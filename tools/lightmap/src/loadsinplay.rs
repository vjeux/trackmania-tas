//! LOADS IN PLAY — the trust matrix's per-cell play-load column (V8, 2026-10-02; the coordinator's row class of 10-01 16:10 PT).
//!
//! The Fall night (10-01/02) showed that EVERY port-lit file the lane had ever written hung the client's PlayMap on its loading
//! frame — frame record 0's KIND word (mapping head offset 60) was the template's 3 where the game writes [2, 3] for two image
//! frames and [3, 3, 2] for three — while the editor accepted the same files (the FILETIME word made them "valid" there) and
//! the matrix compared their images to the digit. A cell whose product output does not load in play is NOT closed, whatever
//! its images say. This module feeds one `loads` column per cell from two sources, by the FILE's md5 (the bytes are the identity,
//! not the cell name — a cell inherits a verdict only when its banked bytes ARE the tested bytes):
//!
//! * STATIC — the words the client validates at load, read from the banked `ours.Map.Gbx` in-process (`static_read`): the frame
//!   record kinds vs the frame count (the Fall hang), the cache FILETIME word vs TimeWriteMostRecentSolid (the lightmap is DROPPED
//!   at load when off — `filetimecheck`); or baker-9's `lmtool loadcheck` JSON beside the bake (`loadcheck.json`:
//!   `{"verdict":"ok"|"fail","failed":[…],"md5":"…"}`), which wins when present.
//! * PLAY — the real play-loads on the game box: append-only per-writer TSVs `playload-<writer>.tsv` (`md5 TAB file TAB verdict
//!   TAB when TAB box TAB note`; verdict PLAYS | HANG | LM-REJECTED; `#` comments; a header line is skipped), read with
//!   `read_playload` from a file or a directory of them; the LAST row for an md5 wins. `lmtool playload add` appends rows.
//!
//! The column text is `**TAG** detail` with TAG ∈ PLAYS · HANGS · LM-REJECTED · STATIC ok · STATIC FAIL · untested; `state_of`
//! parses it back and `gated_label` demotes a CLOSED light verdict without a PLAYS verdict to "CLOSED (…) · UNPLAYED" (or
//! "· HANGS IN PLAY" / "· LM REJECTED IN PLAY" / "· STATIC FAIL").

use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadState { Plays, Hangs, LmRejected, StaticOk, StaticFail, Untested }

impl LoadState {
    pub fn tag(&self) -> &'static str {
        match self { LoadState::Plays => "PLAYS", LoadState::Hangs => "HANGS", LoadState::LmRejected => "LM-REJECTED", LoadState::StaticOk => "STATIC ok", LoadState::StaticFail => "STATIC FAIL", LoadState::Untested => "untested" }
    }
}

/// One play-load result row.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayRow {
    pub md5: String,
    pub file: String,
    /// normalised: PLAYS | HANG | LM-REJECTED (anything else is kept verbatim and reads as untested)
    pub verdict: String,
    pub when: String,
    pub host: String,
    pub note: String,
}

pub fn normalise_verdict(v: &str) -> String {
    match v.trim().to_ascii_uppercase().as_str() {
        "PLAYS" | "PLAY" | "PASS" | "LOADS" | "LOADED" | "OK" => "PLAYS".to_string(),
        "HANG" | "HANGS" | "HUNG" | "TIMEOUT" => "HANG".to_string(),
        "LM-REJECTED" | "LM_REJECTED" | "REJECTED" | "LMREJECTED" | "NO-LIGHTMAP" => "LM-REJECTED".to_string(),
        other => other.to_string(),
    }
}

fn parse_rows(txt: &str, source: &str) -> Result<Vec<PlayRow>, String> {
    let mut out = Vec::new();
    for (ln, line) in txt.lines().enumerate() {
        let t = line.trim_end_matches('\r');
        if t.trim().is_empty() || t.starts_with('#') { continue; }
        let f: Vec<&str> = t.split('\t').collect();
        if f[0].trim().eq_ignore_ascii_case("md5") { continue; } // the header
        if f.len() < 3 { return Err(format!("{source}:{}: a play-load row needs md5 TAB file TAB verdict (got {} field(s))", ln + 1, f.len())); }
        let md5 = f[0].trim().to_ascii_lowercase();
        if md5.len() < 8 || !md5.chars().all(|c| c.is_ascii_hexdigit()) { return Err(format!("{source}:{}: `{}` is not an md5 (≥ 8 hex digits)", ln + 1, f[0].trim())); }
        let g = |i: usize| f.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
        out.push(PlayRow { md5, file: g(1), verdict: normalise_verdict(&g(2)), when: g(3), host: g(4), note: g(5) });
    }
    Ok(out)
}

/// Every play-load row of a TSV file, or of every `*.tsv` in a directory (sorted by name), in file order.
pub fn read_playload(p: &Path) -> Result<Vec<PlayRow>, String> {
    if p.is_dir() {
        let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(p).map_err(|e| format!("{}: {e}", p.display()))?.filter_map(|e| e.ok().map(|e| e.path())).filter(|q| q.extension().map(|x| x == "tsv").unwrap_or(false) && q.file_name().map(|n| n.to_string_lossy().starts_with("playload")).unwrap_or(false)).collect();
        names.sort();
        let mut out = Vec::new();
        for q in names { out.extend(read_playload(&q)?); }
        return Ok(out);
    }
    let txt = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    parse_rows(&txt, &p.display().to_string())
}

/// The row for an md5: the LAST matching row wins (append-only logs — a re-test supersedes); a short md5 (≥ 8 hex) on either
/// side matches as a prefix.
pub fn lookup<'a>(rows: &'a [PlayRow], md5: &str) -> Option<&'a PlayRow> {
    let m = md5.trim().to_ascii_lowercase();
    rows.iter().rev().find(|r| r.md5 == m || (r.md5.len() >= 8 && m.starts_with(&r.md5)) || (m.len() >= 8 && r.md5.starts_with(&m)))
}

/// Append rows to a play-load TSV (the header is written when the file is new).
pub fn append(path: &Path, rows: &[PlayRow]) -> Result<(), String> {
    let mut txt = if path.exists() { std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))? } else { String::from("md5\tfile\tverdict\twhen\tbox\tnote\n") };
    if !txt.ends_with('\n') { txt.push('\n'); }
    for r in rows { txt.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\n", r.md5, r.file, r.verdict, r.when, r.host, r.note.replace(['\t', '\n'], " "))); }
    std::fs::write(path, txt).map_err(|e| format!("{}: {e}", path.display()))
}

// ---------------------------------------------------------------- the static read

/// The frame record kinds of a mapping head (60 constants + 66·k records + a 12-byte tail; a record's first u32 is its kind).
pub fn record_kinds(head: &[u8]) -> Vec<u32> {
    let n = head.len().saturating_sub(72) / 66;
    (0..n).map(|i| { let o = 60 + 66 * i; u32::from_le_bytes([head[o], head[o + 1], head[o + 2], head[o + 3]]) }).collect()
}

/// THE KIND RULE (the 19 fixer's read of 77 game files, 10-02 05:22Z): two image frames → records [2, 3]; three → [3, 3, 2].
pub fn kinds_rule(frames: usize, kinds: &[u32]) -> Result<(), String> {
    let want: &[u32] = match frames { 2 => &[2, 3], 3 => &[3, 3, 2], n => return Err(format!("{n} image frames (the game writes 2 or 3)")) };
    if kinds == want { Ok(()) } else { Err(format!("record kinds {kinds:?} with {frames} image frames — the game writes {want:?} (record 0's kind = the Fall play-load hang)")) }
}

#[derive(Clone, Debug)]
pub struct StaticRead {
    pub md5: String,
    pub frames: usize,
    pub kinds: Vec<u32>,
    /// `filetimecheck::verdict` text
    pub filetime: String,
    pub fails: Vec<String>,
}

/// The static read of one lit map: md5, the kind rule, the FILETIME rule.
pub fn static_read(path: &str) -> Result<StaticRead, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let md5 = crate::corpusgate::md5_hex(&bytes);
    let m = crate::mapio::load(path)?;
    let d = m.chunk.data.as_ref().ok_or_else(|| format!("{path}: no lightmap chunk"))?;
    let mp = d.cache.mapping().ok_or_else(|| format!("{path}: no mapping chunk"))?;
    let kinds = record_kinds(&mp.head);
    let frames = d.frames.len();
    let mut fails = Vec::new();
    if let Err(e) = kinds_rule(frames, &kinds) { fails.push(e); }
    let ft = crate::filetimecheck::read(path)?;
    let filetime = crate::filetimecheck::verdict(&ft);
    if filetime.starts_with("OFF") || filetime.starts_with("no 0x06022013") { fails.push(format!("FILETIME {filetime}")); }
    Ok(StaticRead { md5, frames, kinds, filetime, fails })
}

/// baker-9's `lmtool loadcheck` sidecar: (ok, failed rules, md5) — None when the file is absent or unreadable.
pub fn read_loadcheck_json(p: &Path) -> Option<(bool, Vec<String>, String)> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
    let ok = match v["verdict"].as_str() { Some(s) => s.eq_ignore_ascii_case("ok") || s.eq_ignore_ascii_case("pass"), None => v["ok"].as_bool()? };
    let failed = v["failed"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    Some((ok, failed, v["md5"].as_str().unwrap_or("").to_string()))
}

fn short(md5: &str) -> &str { &md5[..md5.len().min(8)] }

/// The column text for one cell. `loadcheck` wins over the in-process static read when present; a play row decides the tag.
pub fn column_text(stat: Option<&StaticRead>, loadcheck: Option<&(bool, Vec<String>, String)>, play: Option<&PlayRow>) -> String {
    let md5 = stat.map(|s| s.md5.as_str()).or(loadcheck.map(|l| l.2.as_str())).unwrap_or("");
    let static_txt = match (loadcheck, stat) {
        (Some((true, _, _)), _) => "loadcheck ok".to_string(),
        (Some((false, failed, _)), _) => format!("loadcheck FAIL {}", if failed.is_empty() { "(unnamed rule)".to_string() } else { failed.join("; ") }),
        (None, Some(s)) if s.fails.is_empty() => format!("kinds {:?}/{} frames · FILETIME {}", s.kinds, s.frames, s.filetime.split(" (").next().unwrap_or(&s.filetime)),
        (None, Some(s)) => format!("static FAIL: {}", s.fails.join("; ")),
        (None, None) => String::new(),
    };
    let static_ok = match (loadcheck, stat) { (Some((ok, _, _)), _) => *ok, (None, Some(s)) => s.fails.is_empty(), (None, None) => false };
    let tail = if md5.is_empty() { String::new() } else { format!(" (md5 {})", short(md5)) };
    match play {
        Some(r) => {
            let (tag, verb) = match r.verdict.as_str() { "PLAYS" => (LoadState::Plays, "play-loaded"), "HANG" => (LoadState::Hangs, "HUNG the client's PlayMap"), "LM-REJECTED" => (LoadState::LmRejected, "loaded WITHOUT its lightmap"), _ => (LoadState::Untested, "unknown verdict") };
            let whenbox = [r.when.as_str(), r.host.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" on ");
            let mut s = format!("**{}** {verb}{}{tail}", tag.tag(), if whenbox.is_empty() { String::new() } else { format!(" {whenbox}") });
            if !r.note.is_empty() { s.push_str(&format!(" — {}", r.note)); }
            if !static_txt.is_empty() { s.push_str(&format!(" · {static_txt}")); }
            s
        }
        None => match (static_txt.is_empty(), static_ok) {
            (true, _) => "—".to_string(),
            (false, true) => format!("**STATIC ok** {static_txt}{tail} — play untested"),
            (false, false) => format!("**STATIC FAIL** {static_txt}{tail}"),
        },
    }
}

/// The tag parsed back from a column text.
pub fn state_of(text: &str) -> LoadState {
    let t = text.trim();
    let Some(tag) = t.strip_prefix("**").and_then(|r| r.split("**").next()) else { return LoadState::Untested };
    match tag.trim() {
        "PLAYS" => LoadState::Plays,
        "HANGS" => LoadState::Hangs,
        "LM-REJECTED" => LoadState::LmRejected,
        "STATIC ok" => LoadState::StaticOk,
        "STATIC FAIL" => LoadState::StaticFail,
        _ => LoadState::Untested,
    }
}

/// THE GATE: a CLOSED light verdict renders as CLOSED only with a PLAYS verdict on the cell's bytes; otherwise the label carries
/// what stands between it and the game. RESIDUE / OPEN / no oracle labels are returned unchanged.
pub fn gated_label(light: &str, loads: &LoadState) -> String {
    if !light.starts_with("CLOSED") { return light.to_string(); }
    match loads {
        LoadState::Plays => light.to_string(),
        LoadState::Hangs => format!("{light} · HANGS IN PLAY"),
        LoadState::LmRejected => format!("{light} · LM REJECTED IN PLAY"),
        LoadState::StaticFail => format!("{light} · STATIC FAIL"),
        LoadState::StaticOk | LoadState::Untested => format!("{light} · UNPLAYED"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_follow_the_frame_count() {
        assert!(kinds_rule(2, &[2, 3]).is_ok());
        assert!(kinds_rule(3, &[3, 3, 2]).is_ok());
        // the Fall hang: two frames, record 0 left at the template's 3
        assert!(kinds_rule(2, &[3, 3]).unwrap_err().contains("[2, 3]"));
        assert!(kinds_rule(3, &[2, 3]).is_err());
        assert!(kinds_rule(1, &[2]).is_err());
        let mut head = vec![0u8; 60 + 66 * 2 + 12];
        head[60..64].copy_from_slice(&3u32.to_le_bytes());
        head[126..130].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(record_kinds(&head), vec![3, 3]);
        assert_eq!(record_kinds(&[0u8; 10]), Vec::<u32>::new());
    }

    #[test]
    fn playload_rows_parse_and_the_last_wins() {
        let rows = parse_rows("md5\tfile\tverdict\twhen\tbox\tnote\n# a comment\n2a797dd5a7314e71b7a33f2e3e8492cb\tFall-01-Giant.Map.Gbx\tPASS\t2026-10-02T07:50Z\twhitestick\tstart check\nDEADBEEF\tx.Map.Gbx\thang\t\t\t\ndeadbeef\tx.Map.Gbx\tPLAYS\t2026-10-02T09:00Z\tws\tre-test\n", "t").unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].verdict, "PLAYS");
        assert_eq!(rows[1].verdict, "HANG");
        assert_eq!(lookup(&rows, "2a797dd5").unwrap().file, "Fall-01-Giant.Map.Gbx");
        assert_eq!(lookup(&rows, "deadbeef00000000").unwrap().verdict, "PLAYS");
        assert!(lookup(&rows, "0123456789abcdef").is_none());
        assert!(parse_rows("zz\tf\tPLAYS\n", "t").is_err());
        assert!(parse_rows("deadbeef\tf\n", "t").is_err());
    }

    #[test]
    fn column_text_and_gate() {
        let s = StaticRead { md5: "abcdef0123456789".into(), frames: 2, kinds: vec![2, 3], filetime: "EQUAL to the max solid time (all embedded = placed)".into(), fails: vec![] };
        let t = column_text(Some(&s), None, None);
        assert!(t.starts_with("**STATIC ok**") && t.contains("md5 abcdef01") && t.contains("play untested"), "{t}");
        assert_eq!(state_of(&t), LoadState::StaticOk);
        assert_eq!(gated_label("CLOSED (class)", &state_of(&t)), "CLOSED (class) · UNPLAYED");
        let bad = StaticRead { fails: vec!["record kinds [3, 3] with 2 image frames".into()], ..s.clone() };
        let t = column_text(Some(&bad), None, None);
        assert_eq!(state_of(&t), LoadState::StaticFail, "{t}");
        assert_eq!(gated_label("CLOSED (texel)", &state_of(&t)), "CLOSED (texel) · STATIC FAIL");
        let play = PlayRow { md5: s.md5.clone(), file: "ours.Map.Gbx".into(), verdict: "PLAYS".into(), when: "2026-10-02T15:00Z".into(), host: "whitestick".into(), note: String::new() };
        let t = column_text(Some(&s), None, Some(&play));
        assert_eq!(state_of(&t), LoadState::Plays, "{t}");
        assert!(t.contains("2026-10-02T15:00Z on whitestick"));
        assert_eq!(gated_label("CLOSED (class)", &state_of(&t)), "CLOSED (class)");
        let hang = PlayRow { verdict: "HANG".into(), ..play.clone() };
        assert_eq!(gated_label("CLOSED (class)", &state_of(&column_text(Some(&s), None, Some(&hang)))), "CLOSED (class) · HANGS IN PLAY");
        // baker-9's loadcheck sidecar wins over the in-process read
        let lc = (false, vec!["kind".to_string()], s.md5.clone());
        let t = column_text(Some(&s), Some(&lc), None);
        assert_eq!(state_of(&t), LoadState::StaticFail, "{t}");
        assert_eq!(gated_label("RESIDUE", &LoadState::Untested), "RESIDUE");
        assert_eq!(column_text(None, None, None), "—");
        assert_eq!(state_of("—"), LoadState::Untested);
    }

    #[test]
    fn append_then_read_round_trip() {
        let dir = std::env::temp_dir().join(format!("loadsinplay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("playload-test.tsv");
        let r = PlayRow { md5: "2a797dd5a7314e71b7a33f2e3e8492cb".into(), file: "Fall-01-Giant.Map.Gbx".into(), verdict: "PLAYS".into(), when: "2026-10-02T07:50Z".into(), host: "whitestick".into(), note: "a\tnote".into() };
        append(&p, &[r.clone()]).unwrap();
        append(&p, &[PlayRow { verdict: "HANG".into(), ..r.clone() }]).unwrap();
        let rows = read_playload(&dir).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].note, "a note");
        assert_eq!(lookup(&rows, &r.md5).unwrap().verdict, "HANG");
        std::fs::remove_dir_all(&dir).ok();
    }
}
