//! `lmtool daytime-sweep SRC --words W1,W2,… --out-dir DIR [--records TSV] -- <bake args…>`: the DayTime sweep of one
//! map — for every word a copy of SRC carrying that word (`lmtool daytime SRC --out COPY --set W`, the same binary),
//! a full bake with the given args (`lmtool bake COPY <args> --out OUT`), then one table row per word: the time of day,
//! the blend line, the sun (or moon) direction, the record MaxHDR, the AddAmbient accumulator, the per-class mean HDR
//! of frame 0 (classcmp's decode) and the sanity flags (non-finite record, all-black image). No oracle is involved: the
//! row checks continuity across the day and the Night ↔ Sunrise / Sunset ↔ Night branch picks.

use crate::classcmp::{ClassAcc, GroupBy, Options, RecRow};

pub struct SweepRow {
    pub word: u32,
    pub blend: String,
    pub sun: String,
    pub moon: bool,
    pub record_maxhdr: Option<f32>,
    pub add_ambient: String,
    pub classes: Vec<(String, ClassAcc)>,
    pub bake_ok: bool,
    pub wall_s: f32,
}

fn grep1<'a>(log: &'a str, key: &str) -> Option<&'a str> {
    log.lines().find(|l| l.contains(key))
}

/// One word: set, bake, measure.
pub fn run_word(exe: &std::path::Path, src: &str, word: u32, out_dir: &str, bake_args: &[String], records: Option<&[RecRow]>) -> Result<SweepRow, String> {
    let copy = format!("{out_dir}/sweep-{word:04x}.Map.Gbx");
    let out = format!("{out_dir}/sweep-{word:04x}-ours.Map.Gbx");
    let log_path = format!("{out_dir}/sweep-{word:04x}.bake.log");
    let st = std::process::Command::new(exe).args(["daytime", src, "--out", &copy, "--set", &format!("{word:#x}")]).output().map_err(|e| format!("daytime: {e}"))?;
    if !st.status.success() { return Err(format!("daytime --set {word:#x} failed: {}", String::from_utf8_lossy(&st.stderr))); }
    let t0 = std::time::Instant::now();
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("bake").arg(&copy).args(bake_args).arg("--out").arg(&out);
    let o = cmd.output().map_err(|e| format!("bake: {e}"))?;
    let wall_s = t0.elapsed().as_secs_f32();
    let log = format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    std::fs::write(&log_path, &log).map_err(|e| format!("{log_path}: {e}"))?;
    let bake_ok = o.status.success();
    let blend = grep1(&log, "mood blend:").map(|l| l.trim_start_matches("mood blend: ").to_string()).unwrap_or_else(|| "pure".into());
    let sun = grep1(&log, "model xml:").and_then(|l| l.split("; sun ").nth(1)).map(|s| s.split(" (direct").next().unwrap_or(s).to_string()).unwrap_or_default();
    let moon = log.lines().any(|l| l.contains("the moon") || l.contains("moon branch") || l.contains("MOON"));
    let add_ambient = grep1(&log, "AddAmbient accumulator after sweep 0").and_then(|l| l.split(": ").last()).map(|s| s.to_string()).unwrap_or_default();
    let (record_maxhdr, classes) = if bake_ok {
        let m = crate::mapio::load(&out)?;
        let opts = Options { frame: 0, lit: 8, lit_hdr: None, by: GroupBy::Class, worst: 0, own_rects: false, peaks: 0, near: None };
        let r = crate::classcmp::compare(&m, &m, records, &opts)?;
        (Some(r.maxhdr_ours), r.classes)
    } else { (None, Vec::new()) };
    Ok(SweepRow { word, blend, sun, moon, record_maxhdr, add_ambient, classes, bake_ok, wall_s })
}

pub fn print_rows(rows: &[SweepRow]) {
    let class_names: Vec<String> = rows.iter().flat_map(|r| r.classes.iter().map(|(k, _)| k.clone())).fold(Vec::new(), |mut acc, k| { if !acc.contains(&k) { acc.push(k); } acc });
    println!("word\ttime\tbake\tblend\tsun / moon\trecord MaxHDR\tAddAmbient (sweep 0)\t{}\tflags\twall s", class_names.iter().map(|k| format!("{k} mean HDR r/g/b (lit %)")).collect::<Vec<_>>().join("\t"));
    for r in rows {
        let t = r.word as f64 / 65536.0 * 24.0;
        let (h, m) = (t.floor() as u32, ((t - t.floor()) * 60.0).round() as u32);
        let mut flags = Vec::new();
        match r.record_maxhdr { Some(v) if !v.is_finite() => flags.push("RECORD NaN".to_string()), Some(v) if v <= 0.0 => flags.push("RECORD 0".to_string()), None => flags.push("NO FILE".to_string()), _ => {} }
        if r.classes.iter().all(|(_, c)| c.lit_ours == 0) && r.bake_ok { flags.push("ALL BLACK".to_string()); }
        if r.classes.iter().any(|(_, c)| c.mean_ours().iter().any(|v| !v.is_finite())) { flags.push("MEAN NaN".to_string()); }
        let cols: Vec<String> = class_names.iter().map(|k| match r.classes.iter().find(|(kk, _)| kk == k) { Some((_, c)) => { let m = c.mean_ours(); format!("{:.4} / {:.4} / {:.4} ({:.0})", m[0], m[1], m[2], 100.0 * c.lit_ours as f64 / c.texels.max(1) as f64) } None => "—".into() }).collect();
        println!("{:#06x}\t{h:02}:{m:02}\t{}\t{}\t{}{}\t{}\t{}\t{}\t{}\t{:.1}", r.word, if r.bake_ok { "ok" } else { "FAILED" }, r.blend, r.sun, if r.moon { " [moon]" } else { "" }, r.record_maxhdr.map(|v| format!("{v}")).unwrap_or_default(), r.add_ambient, cols.join("\t"), flags.join(" "), r.wall_s);
    }
}

/// Parse `--words a,b,c` (hex with 0x or decimal).
pub fn parse_words(s: &str) -> Result<Vec<u32>, String> {
    s.split(',').map(|w| { let w = w.trim(); if let Some(h) = w.strip_prefix("0x") { u32::from_str_radix(h, 16).map_err(|e| format!("{w}: {e}")) } else { w.parse::<u32>().map_err(|e| format!("{w}: {e}")) } }).collect()
}
