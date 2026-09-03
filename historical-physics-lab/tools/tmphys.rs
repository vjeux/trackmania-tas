// tmphys — deterministic measurement + extraction tooling for the
// Trackmania 2020 official physics behavior changelog.
//
// Rust only, std only (no network, no crates), so it builds offline on any box.
//
// Subcommands
//   tunings   <bin>...            extract Nadeo physics-tuning names (ordered by file offset)
//   ledger    <dir-of-exes>       first-appearance ledger of tuning names across dated builds
//   strings   <bin> <regex-lite>  raw ASCII string grep with offsets (substring, case-insensitive)
//   validate  <spec.tsv>          run the dedicated-server oracle matrix (build x tape)
//   f32scan   <bin> <lo> <hi>     enumerate plausible f32 constants in a byte range
//
// Every subcommand writes TSV to stdout: stable, diffable, machine-readable.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: tmphys <tunings|ledger|strings|validate|f32scan> ...");
        std::process::exit(2);
    }
    let rc = match args[1].as_str() {
        "tunings" => cmd_tunings(&args[2..]),
        "ledger" => cmd_ledger(&args[2..]),
        "strings" => cmd_strings(&args[2..]),
        "validate" => cmd_validate(&args[2..]),
        "f32scan" => cmd_f32scan(&args[2..]),
        "find" => cmd_find(&args[2..]),
        "poke" => cmd_poke(&args[2..]),
        "matrix" => cmd_matrix(&args[2..]),
        "deltas" => cmd_deltas(&args[2..]),
        other => {
            eprintln!("unknown subcommand: {other}");
            2
        }
    };
    std::process::exit(rc);
}

// ---------------------------------------------------------------- strings ---

/// Extract printable-ASCII runs of length >= min_len with their file offsets.
fn ascii_strings(buf: &[u8], min_len: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut cur: Vec<u8> = Vec::new();
    for (i, &b) in buf.iter().enumerate() {
        let printable = (0x20..0x7f).contains(&b);
        if printable {
            if cur.is_empty() {
                start = i;
            }
            cur.push(b);
        } else {
            if cur.len() >= min_len {
                out.push((start, String::from_utf8_lossy(&cur).into_owned()));
            }
            cur.clear();
        }
    }
    if cur.len() >= min_len {
        out.push((start, String::from_utf8_lossy(&cur).into_owned()));
    }
    out
}

/// UTF-16LE strings (Windows PE builds store many names as wide strings).
fn utf16_strings(buf: &[u8], min_len: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut cur: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i + 1 < buf.len() {
        let lo = buf[i];
        let hi = buf[i + 1];
        if hi == 0 && (0x20..0x7f).contains(&lo) {
            if cur.is_empty() {
                start = i;
            }
            cur.push(lo);
            i += 2;
        } else {
            if cur.len() >= min_len {
                out.push((start, String::from_utf8_lossy(&cur).into_owned()));
            }
            cur.clear();
            i += 1;
        }
    }
    if cur.len() >= min_len {
        out.push((start, String::from_utf8_lossy(&cur).into_owned()));
    }
    out
}

/// Is this string one of Nadeo's physics tuning-table entry names?
///
/// The tuning table is Nadeo's own dated ledger of vehicle force-law tweaks.
/// Two shapes occur, both verified present in shipped binaries:
///   * a dated tweak name: <Topic><YYMMDD>            e.g. IceDrift200624
///   * a named tweak with an explicit date:            e.g. 06/12/2019_TurboAirControl_Ice
///   * a bare topic name:                              e.g. WallRepulse, IceDrift, AirControlStab
fn is_tuning_name(s: &str) -> bool {
    // Bare/known topic names, exact match only (avoid sweeping in unrelated text).
    const EXACT: &[&str] = &[
        "20fev2013",
        "WallRepulse",
        "IceDrift",
        "IceDriftV1",
        "IceDriftV2",
        "IceDriftV3",
        "IceDriftV4",
        "IceDriftV5",
        "IceDriftV6",
        "AirControlStab",
        "SlowDownOnWater",
        "NoSlowDownOnIce",
    ];
    if EXACT.contains(&s) {
        return true;
    }
    // "06/12/2019_TurboAirControl_Ice" shape: starts with dd/dd/dddd_
    let b = s.as_bytes();
    if b.len() > 11
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b'/'
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && b[5] == b'/'
        && b[6..10].iter().all(|c| c.is_ascii_digit())
        && b[10] == b'_'
    {
        return true;
    }
    // "<Topic><YYMMDD>" shape: >=6 trailing digits, alphabetic prefix of >=4 chars,
    // and the digits must parse as a plausible date 2005-01-01 .. 2035-12-31.
    if b.len() >= 10 {
        let digits_start = b.len() - 6;
        let tail = &s[digits_start..];
        let head = &s[..digits_start];
        if tail.chars().all(|c| c.is_ascii_digit())
            && head.len() >= 4
            && head.chars().all(|c| c.is_ascii_alphanumeric())
            && head.chars().next().map(|c| c.is_ascii_uppercase()) == Some(true)
            && !head.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(true)
        {
            let yy: u32 = tail[0..2].parse().unwrap_or(99);
            let mm: u32 = tail[2..4].parse().unwrap_or(99);
            let dd: u32 = tail[4..6].parse().unwrap_or(99);
            if yy <= 35 && (1..=12).contains(&mm) && (1..=31).contains(&dd) {
                return true;
            }
        }
    }
    false
}

/// Decode a tuning name's embedded date, if it carries one, as YYYY-MM-DD.
fn tuning_date(s: &str) -> Option<String> {
    let b = s.as_bytes();
    if b.len() > 11 && b[2] == b'/' && b[5] == b'/' && b[10] == b'_' {
        let dd = &s[0..2];
        let mm = &s[3..5];
        let yyyy = &s[6..10];
        return Some(format!("{yyyy}-{mm}-{dd}"));
    }
    if b.len() >= 10 {
        let tail = &s[b.len() - 6..];
        if tail.chars().all(|c| c.is_ascii_digit()) {
            let yy: u32 = tail[0..2].parse().ok()?;
            let mm: u32 = tail[2..4].parse().ok()?;
            let dd: u32 = tail[4..6].parse().ok()?;
            if yy <= 35 && (1..=12).contains(&mm) && (1..=31).contains(&dd) {
                return Some(format!("20{yy:02}-{mm:02}-{dd:02}"));
            }
        }
    }
    None
}

fn read_file(p: &str) -> Vec<u8> {
    match fs::read(p) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cannot read {p}: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_tunings(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("usage: tmphys tunings <binary>...");
        return 2;
    }
    println!("binary\tindex\toffset\tencoding\tname\tembedded_date");
    for path in args {
        let buf = read_file(path);
        let name = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        let mut hits: Vec<(usize, String, &'static str)> = Vec::new();
        for (off, s) in ascii_strings(&buf, 6) {
            if is_tuning_name(&s) {
                hits.push((off, s, "ascii"));
            }
        }
        for (off, s) in utf16_strings(&buf, 6) {
            if is_tuning_name(&s) {
                hits.push((off, s, "utf16le"));
            }
        }
        hits.sort();
        for (i, (off, s, enc)) in hits.iter().enumerate() {
            println!(
                "{name}\t{i}\t0x{off:x}\t{enc}\t{s}\t{}",
                tuning_date(s).unwrap_or_else(|| "-".to_string())
            );
        }
    }
    0
}

fn cmd_strings(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: tmphys strings <binary> <substring> [min_len]");
        return 2;
    }
    let buf = read_file(&args[0]);
    let needle = args[1].to_lowercase();
    let min_len: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(6);
    println!("offset\tencoding\tstring");
    for (off, s) in ascii_strings(&buf, min_len) {
        if s.to_lowercase().contains(&needle) {
            println!("0x{off:x}\tascii\t{s}");
        }
    }
    for (off, s) in utf16_strings(&buf, min_len) {
        if s.to_lowercase().contains(&needle) {
            println!("0x{off:x}\tutf16le\t{s}");
        }
    }
    0
}

/// Build a first-appearance ledger: for each tuning name, the earliest dated
/// build whose binary contains it, plus the full presence bitmap.
fn cmd_ledger(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("usage: tmphys ledger <dir-with-<date>/<exe> layout> [exe-name]");
        return 2;
    }
    let root = PathBuf::from(&args[0]);
    let exe_name = args.get(1).cloned().unwrap_or_else(|| "TrackmaniaServer".to_string());
    let mut builds: Vec<String> = Vec::new();
    let entries = match fs::read_dir(&root) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("cannot list {}: {e}", root.display());
            return 1;
        }
    };
    for ent in entries.flatten() {
        if ent.path().is_dir() {
            let d = ent.file_name().to_string_lossy().into_owned();
            if ent.path().join(&exe_name).exists() {
                builds.push(d);
            }
        }
    }
    builds.sort();

    // build -> ordered names
    let mut per_build: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for b in &builds {
        let p = root.join(b).join(&exe_name);
        let buf = read_file(&p.to_string_lossy());
        let mut hits: Vec<(usize, String)> = Vec::new();
        for (off, s) in ascii_strings(&buf, 6) {
            if is_tuning_name(&s) {
                hits.push((off, s));
            }
        }
        for (off, s) in utf16_strings(&buf, 6) {
            if is_tuning_name(&s) {
                hits.push((off, s));
            }
        }
        hits.sort();
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();
        for (_, s) in hits {
            if seen.insert(s.clone()) {
                ordered.push(s);
            }
        }
        per_build.insert(b.clone(), ordered);
    }

    // Ledger: first build containing each name.
    let mut first: BTreeMap<String, String> = BTreeMap::new();
    for b in &builds {
        for n in &per_build[b] {
            first.entry(n.clone()).or_insert_with(|| b.clone());
        }
    }
    println!("# tuning first-appearance ledger over {} dated builds ({} .. {})",
        builds.len(),
        builds.first().cloned().unwrap_or_default(),
        builds.last().cloned().unwrap_or_default());
    println!("tuning_name\tembedded_date\tfirst_build_containing\tn_builds_present\ttotal_builds");
    for (name, fb) in &first {
        let n = builds.iter().filter(|b| per_build[*b].contains(name)).count();
        println!(
            "{name}\t{}\t{fb}\t{n}\t{}",
            tuning_date(name).unwrap_or_else(|| "-".to_string()),
            builds.len()
        );
    }
    // Per-build count + change points.
    println!("#");
    println!("# per-build tuning-set summary");
    println!("build\tn_tunings\tset_changed_vs_previous\tadded\tremoved");
    let mut prev: Option<&Vec<String>> = None;
    for b in &builds {
        let cur = &per_build[b];
        let (changed, added, removed) = match prev {
            None => ("START".to_string(), String::from("-"), String::from("-")),
            Some(p) => {
                let ps: BTreeSet<&String> = p.iter().collect();
                let cs: BTreeSet<&String> = cur.iter().collect();
                let add: Vec<String> = cs.difference(&ps).map(|s| (*s).clone()).collect();
                let rem: Vec<String> = ps.difference(&cs).map(|s| (*s).clone()).collect();
                let ch = if add.is_empty() && rem.is_empty() { "=" } else { "CHANGED" };
                (
                    ch.to_string(),
                    if add.is_empty() { "-".into() } else { add.join(",") },
                    if rem.is_empty() { "-".into() } else { rem.join(",") },
                )
            }
        };
        println!("{b}\t{}\t{changed}\t{added}\t{removed}", cur.len());
        prev = Some(cur);
    }
    0
}

// --------------------------------------------------------------- f32scan ---

fn cmd_f32scan(args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("usage: tmphys f32scan <binary> <lo_offset> <hi_offset> [stride=1]");
        return 2;
    }
    let buf = read_file(&args[0]);
    let lo = parse_num(&args[1]).unwrap_or(0);
    let hi = parse_num(&args[2]).unwrap_or(buf.len());
    // Stride 1 by default: a 4-byte stride only ever sees one alignment phase and
    // silently misses every float not congruent to `lo` modulo 4. A control run
    // caught exactly that, so the default is now exhaustive.
    let stride = args.get(3).and_then(|s| parse_num(s)).unwrap_or(1).max(1);
    let hi = hi.min(buf.len());
    println!("offset\tf32\tf64");
    let mut i = lo;
    while i + 4 <= hi {
        let v = f32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
        let plausible = v.is_finite() && v != 0.0 && v.abs() >= 1e-9 && v.abs() <= 1e6;
        if plausible {
            let d = if i + 8 <= hi {
                let mut a = [0u8; 8];
                a.copy_from_slice(&buf[i..i + 8]);
                let dv = f64::from_le_bytes(a);
                if dv.is_finite() && dv != 0.0 && dv.abs() >= 1e-4 && dv.abs() <= 1e6 {
                    format!("{dv}")
                } else {
                    "-".to_string()
                }
            } else {
                "-".to_string()
            };
            println!("0x{i:x}\t{v}\t{d}");
        }
        i += stride;
    }
    0
}

/// Write a little-endian f32 at an offset, producing a NEW file.
///
/// This exists for deliberate-perturbation positive controls: change a value a
/// decoder claims to read, and require the decoder's output to change with it.
/// A decoder that reports the same numbers either way is not reading the bytes
/// you think it is.
fn cmd_poke(args: &[String]) -> i32 {
    if args.len() < 4 {
        eprintln!("usage: tmphys poke <in> <out> <offset> <f32_value>");
        return 2;
    }
    let mut buf = read_file(&args[0]);
    let off = match parse_num(&args[2]) {
        Some(v) => v,
        None => {
            eprintln!("bad offset");
            return 2;
        }
    };
    let val: f32 = match args[3].parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("bad f32");
            return 2;
        }
    };
    if off + 4 > buf.len() {
        eprintln!("offset out of range");
        return 1;
    }
    let before = f32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
    buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    if let Err(e) = fs::write(&args[1], &buf) {
        eprintln!("write failed: {e}");
        return 1;
    }
    println!("offset\tbefore\tafter\tout");
    println!("0x{off:x}\t{before}\t{val}\t{}", args[1]);
    0
}

/// Locate an exact byte pattern (hex, whitespace allowed) in a binary and print
/// every file offset. Used to pin a documented RVA to a real file offset without
/// trusting a section-header calculation.
fn cmd_find(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: tmphys find <binary> <hex-pattern> [context_bytes]");
        return 2;
    }
    let buf = read_file(&args[0]);
    let hex: String = args[1].chars().filter(|c| !c.is_whitespace()).collect();
    if hex.len() % 2 != 0 {
        eprintln!("hex pattern must have an even number of digits");
        return 2;
    }
    let pat: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    let ctx: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    println!("offset\tcontext_hex");
    let mut n = 0;
    for i in 0..buf.len().saturating_sub(pat.len()) {
        if &buf[i..i + pat.len()] == pat.as_slice() {
            n += 1;
            let lo = i.saturating_sub(ctx);
            let hi = (i + pat.len() + ctx).min(buf.len());
            let hexs: String = buf[lo..hi].iter().map(|b| format!("{b:02x}")).collect();
            println!("0x{i:x}\t{hexs}");
        }
    }
    eprintln!("{n} match(es)");
    0
}

// ---------------------------------------------------------------- matrix ---

/// Analyse a build x probe verdict matrix at a boundary.
///
/// The question a probe-count flip does NOT answer: *which* runs changed, and do
/// they share a property? A boundary where every map flips is a global change to
/// the car; one where only maps carrying a given feature flip is a change to that
/// feature. This computes that distinction from the matrix, per map.
///
/// Input: the audit's matrixW_all.tsv (build_date, build_banner, replay,
/// recording_build, map_uid, declared_ms, declared_cps, validated_ms,
/// validated_cps, is_valid, desc).
fn cmd_matrix(args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("usage: tmphys matrix <matrix.tsv> <build_a> <build_b>");
        return 2;
    }
    let raw = read_file(&args[0]);
    let text = String::from_utf8_lossy(&raw);
    let (ba, bb) = (args[1].as_str(), args[2].as_str());

    // (build, replay) -> (verdict, map_uid, declared_ms, recording_build)
    let mut cell: BTreeMap<(String, String), (String, String, String, String)> = BTreeMap::new();
    let mut hdr_seen = false;
    for line in text.lines() {
        if !hdr_seen {
            hdr_seen = true;
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 11 {
            continue;
        }
        let (build, replay, recbuild, map, decl, valid, desc) =
            (c[0], c[2], c[3], c[4], c[5], c[9], c[10]);
        // Verdict taxonomy must keep NOLOAD apart from WRONG: a container-format
        // failure is not a physics fact and must never manufacture an epoch.
        let d = desc.to_lowercase();
        let verdict = if d.contains("can't load") || d.contains("cant load") || d.contains("noload")
        {
            "NOLOAD"
        } else if valid.eq_ignore_ascii_case("true") {
            "EXACT"
        } else {
            "WRONG"
        };
        cell.insert(
            (build.to_string(), replay.to_string()),
            (
                verdict.to_string(),
                map.to_string(),
                decl.to_string(),
                recbuild.to_string(),
            ),
        );
    }

    // Probes loaded by BOTH builds: the only physics-clean comparison basis.
    let mut flips: Vec<(String, String, String, String, String, String)> = Vec::new();
    let mut same: Vec<(String, String, String)> = Vec::new();
    let mut probes: BTreeSet<String> = BTreeSet::new();
    for (b, r) in cell.keys() {
        if b == ba || b == bb {
            probes.insert(r.clone());
        }
    }
    for r in &probes {
        let a = cell.get(&(ba.to_string(), r.clone()));
        let b = cell.get(&(bb.to_string(), r.clone()));
        let (a, b) = match (a, b) {
            (Some(a), Some(b)) => (a, b),
            _ => continue,
        };
        if a.0 == "NOLOAD" || b.0 == "NOLOAD" {
            continue;
        }
        if a.0 != b.0 {
            flips.push((
                r.clone(),
                a.0.clone(),
                b.0.clone(),
                a.1.clone(),
                a.2.clone(),
                a.3.clone(),
            ));
        } else {
            same.push((r.clone(), a.1.clone(), a.0.clone()));
        }
    }

    let maps_flipped: BTreeSet<&String> = flips.iter().map(|f| &f.3).collect();
    let maps_same: BTreeSet<&String> = same.iter().map(|s| &s.1).collect();
    let maps_all: BTreeSet<&String> = maps_flipped.union(&maps_same).cloned().collect();
    // A map is "fully flipped" when every one of its probes flipped: that is the
    // signature of a change the map's whole surface feels, as opposed to a change
    // only some runs on it happen to expose.
    let mut fully = 0usize;
    let mut partly = 0usize;
    for m in &maps_flipped {
        let has_same = same.iter().any(|s| &&s.1 == m);
        if has_same {
            partly += 1;
        } else {
            fully += 1;
        }
    }

    println!("# boundary {ba} -> {bb}");
    println!(
        "common_basis_probes\t{}",
        flips.len() + same.len()
    );
    println!("probes_flipped\t{}", flips.len());
    println!(
        "  exact_to_wrong\t{}",
        flips.iter().filter(|f| f.1 == "EXACT" && f.2 == "WRONG").count()
    );
    println!(
        "  wrong_to_exact\t{}",
        flips.iter().filter(|f| f.1 == "WRONG" && f.2 == "EXACT").count()
    );
    println!("distinct_maps_in_basis\t{}", maps_all.len());
    println!("distinct_maps_with_a_flip\t{}", maps_flipped.len());
    println!("  maps_where_every_probe_flipped\t{fully}");
    println!("  maps_where_only_some_probes_flipped\t{partly}");
    println!(
        "maps_with_no_flip\t{}",
        maps_all.len() - maps_flipped.len()
    );
    // Recording-era breakdown: does the flip track WHEN the run was recorded?
    let mut by_era: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for f in &flips {
        let e = by_era.entry(era_of(&f.5)).or_default();
        if f.1 == "EXACT" {
            e.0 += 1; // lost validity
        } else {
            e.1 += 1; // gained validity
        }
    }
    for s in &same {
        if let Some(c) = cell.get(&(ba.to_string(), s.0.clone())) {
            by_era.entry(era_of(&c.3)).or_default().2 += 1;
        }
    }
    println!("#");
    println!("recording_month\texact_to_wrong\twrong_to_exact\tunchanged");
    for (k, (l, g, s)) in &by_era {
        println!("{k}\t{l}\t{g}\t{s}");
    }
    0
}

/// Extract YYYY-MM from a recording-build banner like
/// `Trackmania date=2021-02-15_13_23 git=... GameVersion=3.3.0`.
fn era_of(banner: &str) -> String {
    for tok in banner.split_whitespace() {
        let t = tok.trim_start_matches("date=").trim_start_matches("Date=");
        if t.len() >= 7 && t.as_bytes()[4] == b'-' && t.as_bytes()[..4].iter().all(|c| c.is_ascii_digit()) {
            return t[..7].to_string();
        }
    }
    "unknown".to_string()
}

/// Parse a quantified re-simulation result out of a validator `desc`.
///
/// The dedicated server does not only say pass/fail. When a run still COMPLETES
/// but under different physics, it reports both numbers:
///   "race finished, time is worse. (5017 < 5033)"        declared 5017, resim 5033
///   "validated time is actually better! (25091 > 25082)" declared 25091, resim 25082
/// Returns (declared_ms, resimulated_ms). This is the only channel in the whole
/// instrument that yields a MAGNITUDE rather than a verdict.
fn parse_time_delta(desc: &str) -> Option<(i64, i64)> {
    let open = desc.rfind('(')?;
    let close = desc[open..].find(')')? + open;
    let inner = &desc[open + 1..close];
    let (a, b) = if let Some(i) = inner.find('<') {
        (&inner[..i], &inner[i + 1..])
    } else if let Some(i) = inner.find('>') {
        (&inner[..i], &inner[i + 1..])
    } else {
        return None;
    };
    let a: i64 = a.trim().parse().ok()?;
    let b: i64 = b.trim().parse().ok()?;
    Some((a, b))
}

/// Per-boundary distribution of re-simulated time differences.
///
/// For every probe that still finishes on both sides, report resim - declared in
/// milliseconds. A boundary where completing runs come back systematically
/// slower or faster is a boundary with a measurable magnitude, not just a
/// verdict flip.
fn cmd_deltas(args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("usage: tmphys deltas <matrix.tsv> <build_a> <build_b>");
        return 2;
    }
    let raw = read_file(&args[0]);
    let text = String::from_utf8_lossy(&raw);
    let (ba, bb) = (args[1].as_str(), args[2].as_str());

    // build -> replay -> (declared, resim_or_none, map, desc)
    let mut per: BTreeMap<String, BTreeMap<String, (i64, Option<i64>, String, String)>> =
        BTreeMap::new();
    let mut hdr = false;
    for line in text.lines() {
        if !hdr {
            hdr = true;
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 11 {
            continue;
        }
        let (build, replay, map, decl, val, valid, desc) =
            (c[0], c[2], c[4], c[5], c[7], c[9], c[10]);
        let declared: i64 = match decl.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        // A run's re-simulated time comes either from the validated column (when
        // it validated) or out of the desc (when it finished at a different time).
        let resim = if valid.eq_ignore_ascii_case("true") {
            val.parse::<i64>().ok()
        } else {
            parse_time_delta(desc).map(|(_, r)| r)
        };
        per.entry(build.to_string())
            .or_default()
            .insert(replay.to_string(), (declared, resim, map.to_string(), desc.to_string()));
    }

    let (ea, eb) = match (per.get(ba), per.get(bb)) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            eprintln!("one of the builds is absent from the matrix");
            return 1;
        }
    };

    let mut rows: Vec<(String, i64, i64, i64, i64, String)> = Vec::new();
    for (replay, (decl_a, resim_a, map, _)) in ea {
        let (decl_b, resim_b, _, _) = match eb.get(replay) {
            Some(v) => v,
            None => continue,
        };
        if decl_a != decl_b {
            continue;
        }
        // Both sides must have completed the lap for a time difference to mean
        // anything; a DNF has no time to compare.
        if let (Some(ra), Some(rb)) = (resim_a, resim_b) {
            if *ra > 0 && *rb > 0 {
                rows.push((replay.clone(), *decl_a, *ra, *rb, rb - ra, map.clone()));
            }
        }
    }
    rows.sort_by_key(|r| r.4);

    println!("# completing-run time differences, {ba} -> {bb}");
    println!("# delta_ms = resim({bb}) - resim({ba}); positive = the same inputs finish SLOWER on {bb}");
    println!("replay\tdeclared_ms\tresim_a_ms\tresim_b_ms\tdelta_ms\tmap_uid");
    for r in &rows {
        println!("{}\t{}\t{}\t{}\t{}\t{}", r.0, r.1, r.2, r.3, r.4, r.5);
    }
    if rows.is_empty() {
        println!("# no probe completes on both sides");
        return 0;
    }
    let n = rows.len();
    let changed: Vec<&(String, i64, i64, i64, i64, String)> =
        rows.iter().filter(|r| r.4 != 0).collect();
    let slower = rows.iter().filter(|r| r.4 > 0).count();
    let faster = rows.iter().filter(|r| r.4 < 0).count();
    let mut mags: Vec<i64> = changed.iter().map(|r| r.4.abs()).collect();
    mags.sort();
    println!("#");
    println!("completing_on_both\t{n}");
    println!("identical_time\t{}", n - changed.len());
    println!("slower_on_b\t{slower}");
    println!("faster_on_b\t{faster}");
    if !mags.is_empty() {
        let sum: i64 = mags.iter().sum();
        println!("abs_delta_min_ms\t{}", mags[0]);
        println!("abs_delta_median_ms\t{}", mags[mags.len() / 2]);
        println!("abs_delta_max_ms\t{}", mags[mags.len() - 1]);
        println!("abs_delta_mean_ms\t{:.1}", sum as f64 / mags.len() as f64);
    }
    0
}

fn parse_num(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x") {
        usize::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

// -------------------------------------------------------------- validate ---

/// Run the dedicated-server oracle across a matrix of builds and tapes.
///
/// spec.tsv columns (tab separated, '#' comments ignored):
///   build_id \t server_dir \t map_file \t ghost_file
///
/// For each row this stages an isolated working tree
///   <work>/<build_id>__<tape>/{TrackmaniaServer, GameData/, UserData/Maps, UserData/Replays}
/// and runs `./TrackmaniaServer /nodaemon /validatepath=.` from inside it
/// (invoking by absolute path makes the server validate the WRONG directory).
fn cmd_validate(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: tmphys validate <spec.tsv> <workdir> [timeout_s]");
        return 2;
    }
    let spec = read_file(&args[0]);
    let spec = String::from_utf8_lossy(&spec).into_owned();
    let work = PathBuf::from(&args[1]);
    let timeout: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(180);
    let _ = fs::create_dir_all(&work);

    println!("build_id\ttape\tvalidated_ms\tvalidated_s\tdeclared_ms\tis_valid\tcheckpoints_reached\tverdict\tstdout_sha_prefix");
    for line in spec.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 {
            eprintln!("bad spec row: {line}");
            continue;
        }
        let (build_id, server_dir, map_file, ghost_file) = (cols[0], cols[1], cols[2], cols[3]);
        let tape = Path::new(ghost_file)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| ghost_file.to_string());
        let cell = work.join(format!("{build_id}__{}", tape.replace('/', "_")));
        let _ = fs::remove_dir_all(&cell);
        if let Err(e) = stage_cell(&cell, server_dir, map_file, ghost_file) {
            println!("{build_id}\t{tape}\t-\t-\t-\t-\t-\tSTAGE_FAIL:{e}\t-");
            continue;
        }
        let out = run_server(&cell, timeout);
        let p = parse_server_output(&out);
        println!(
            "{build_id}\t{tape}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            p.validated_ms.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            p.validated_ms
                .map(|v| format!("{:.3}", v as f64 / 1000.0))
                .unwrap_or_else(|| "-".into()),
            p.declared_ms.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            p.is_valid.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            p.checkpoints.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            p.verdict,
            &sha_prefix(&out)
        );
        // Keep the raw log next to the cell for traceability.
        let _ = fs::write(cell.join("validate_stdout.txt"), out.as_bytes());
    }
    0
}

fn stage_cell(cell: &Path, server_dir: &str, map_file: &str, ghost_file: &str) -> Result<(), String> {
    fs::create_dir_all(cell.join("UserData/Maps")).map_err(|e| e.to_string())?;
    fs::create_dir_all(cell.join("UserData/Replays")).map_err(|e| e.to_string())?;
    let sd = Path::new(server_dir);
    // Symlink the heavy immutable parts, copy the executable.
    for item in ["GameData", "Packs", "CommonData", "RemoteControlExamples"] {
        let src = sd.join(item);
        if src.exists() {
            let _ = std::os::unix::fs::symlink(&src, cell.join(item));
        }
    }
    let exe_src = sd.join("TrackmaniaServer");
    if !exe_src.exists() {
        return Err("no TrackmaniaServer in server_dir".into());
    }
    fs::copy(&exe_src, cell.join("TrackmaniaServer")).map_err(|e| e.to_string())?;
    let mut perms = fs::metadata(cell.join("TrackmaniaServer"))
        .map_err(|e| e.to_string())?
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    let _ = fs::set_permissions(cell.join("TrackmaniaServer"), perms);

    if !map_file.is_empty() && map_file != "-" {
        let mn = Path::new(map_file).file_name().ok_or("bad map name")?;
        fs::copy(map_file, cell.join("UserData/Maps").join(mn)).map_err(|e| e.to_string())?;
    }
    let gn = Path::new(ghost_file).file_name().ok_or("bad ghost name")?;
    fs::copy(ghost_file, cell.join("UserData/Replays").join(gn)).map_err(|e| e.to_string())?;
    Ok(())
}

fn run_server(cell: &Path, timeout_s: u64) -> String {
    // `timeout` guarantees a wedged build cannot stall the whole matrix.
    let out = Command::new("timeout")
        .arg(format!("{timeout_s}"))
        .arg("./TrackmaniaServer")
        .arg("/nodaemon")
        .arg("/validatepath=.")
        .current_dir(cell)
        .output();
    match out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            s
        }
        Err(e) => format!("SPAWN_FAIL: {e}"),
    }
}

#[derive(Default)]
struct Parsed {
    validated_ms: Option<i64>,
    declared_ms: Option<i64>,
    is_valid: Option<bool>,
    checkpoints: Option<u32>,
    desc: String,
    verdict: String,
}

/// Parse the dedicated server's /validatepath report.
///
/// Guard (learned the hard way on 2022 builds): a DNF is reported as a PRESENT
/// ValidatedResult with Time: -1. An unguarded parser scores that as the best
/// finish possible. Any non-positive time is a DNF, never a result.
fn parse_server_output(out: &str) -> Parsed {
    let mut p = Parsed::default();
    // The report is JSON-ish: quoted keys, " : " separators, nested
    // ValidatedResult / DeclaredResult objects, then a plain-text run log.
    //   { "ValidatedResult" : { "Time" : 26448, ... }, "IsValid" : true,
    //     "DeclaredResult" : { "Time" : 26448 }, "Desc" : "wrong simu", ... }
    // Matching bare `Time:` (no quotes) silently parsed nothing at all, which is
    // how a genuine finish fell through to the summary-counter text below.
    let mut block = 0u8; // 1 = validated, 2 = declared
    let mut depth = 0i32;
    for raw in out.lines() {
        let l = raw.trim();
        if l.contains("\"ValidatedResult\"") {
            block = 1;
            depth = 0;
        } else if l.contains("\"DeclaredResult\"") {
            block = 2;
            depth = 0;
        }
        depth += l.matches('{').count() as i32;
        depth -= l.matches('}').count() as i32;
        if let Some(v) = json_int(l, "Time") {
            match block {
                1 => p.validated_ms = Some(v),
                2 => p.declared_ms = Some(v),
                _ => {}
            }
        }
        if let Some(v) = json_int(l, "NbCheckpoints") {
            if block == 1 {
                p.checkpoints = Some(v as u32);
            }
        }
        if let Some(b) = json_bool(l, "IsValid") {
            p.is_valid = Some(b);
            block = 0;
        }
        if let Some(s) = json_str(l, "Desc") {
            p.desc = s;
        }
        if depth <= 0 && block != 0 && l.contains('}') {
            block = 0;
        }
        // Legacy flat schema (launch-era servers): bare `Time:` / `IsValid:`.
        if let Some(rest) = l.strip_prefix("Time:") {
            if let Ok(v) = rest.trim().parse::<i64>() {
                if p.validated_ms.is_none() {
                    p.validated_ms = Some(v);
                }
            }
        }
        if let Some(rest) = l.strip_prefix("IsValid:") {
            let t = rest.trim();
            if p.is_valid.is_none() {
                p.is_valid = Some(t.eq_ignore_ascii_case("true") || t == "1");
            }
        }
        if l.contains("reached some checkpoints") {
            if let Some(open) = l.rfind('(') {
                let inner = &l[open + 1..];
                let n: String = inner.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(v) = n.parse::<u32>() {
                    p.checkpoints = Some(v);
                }
            }
        }
    }

    // Verdict, in priority order. The summary block at the end of a run prints
    // counter lines ("Wrong Simu :   0% (  0)") containing these very phrases,
    // so a naive substring test over the whole log misreads a PASSING run as a
    // failure. Only non-counter lines may set a verdict.
    let mut saw_wrong_simu = false;
    let mut saw_cant_load_replay = false;
    let mut saw_cant_load_map = false;
    let mut saw_unvalidable = false;
    for raw in out.lines() {
        let l = raw.trim();
        let is_counter = l.contains('%') && l.contains('(') && l.contains(':');
        if is_counter {
            continue;
        }
        let low = l.to_lowercase();
        if low.contains("wrong simu") {
            saw_wrong_simu = true;
        }
        if low.contains("can't load replay") || low.contains("can't load ghost") {
            saw_cant_load_replay = true;
        }
        if low.contains("can't load map") || low.contains("can't load challenge") {
            saw_cant_load_map = true;
        }
        if low.contains("unvalidable") {
            saw_unvalidable = true;
        }
    }
    // A refusal to validate because the RECORDING client is blacklisted is a
    // validation-policy outcome, not a physics disagreement. Conflating the two
    // turns a policy change into a fake physics epoch, so it gets its own verdict.
    let policy_refusal = p.desc.contains("known-flawed game exe")
        || out.contains("using known-flawed game exe");

    p.verdict = if out.contains("SPAWN_FAIL") {
        "SPAWN_FAIL".into()
    } else if matches!(p.is_valid, Some(true)) && matches!(p.validated_ms, Some(v) if v > 0) {
        "FINISH".into()
    } else if policy_refusal {
        "POLICY_KNOWN_FLAWED_EXE".into()
    } else if saw_cant_load_replay {
        "CANT_LOAD_REPLAY".into()
    } else if saw_cant_load_map {
        "CANT_LOAD_MAP".into()
    } else if saw_unvalidable {
        "UNVALIDABLE".into()
    } else if p.desc.contains("time is worse") || p.desc.contains("actually better") {
        "FINISH_DIFFERENT_TIME".into()
    } else if saw_wrong_simu {
        "WRONG_SIMU".into()
    } else if matches!(p.validated_ms, Some(v) if v > 0) {
        "FINISH".into()
    } else if p.validated_ms.is_some() {
        "DNF_TIME_NEGATIVE".into()
    } else if out.trim().is_empty() {
        "NO_OUTPUT".into()
    } else {
        "NO_RESULT".into()
    };
    p
}

/// `"Key" : 123` -> 123
fn json_int(line: &str, key: &str) -> Option<i64> {
    let pat = format!("\"{key}\"");
    let i = line.find(&pat)?;
    let rest = &line[i + pat.len()..];
    let colon = rest.find(':')?;
    let val: String = rest[colon + 1..]
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    val.parse().ok()
}

/// `"Key" : true` -> true
fn json_bool(line: &str, key: &str) -> Option<bool> {
    let pat = format!("\"{key}\"");
    let i = line.find(&pat)?;
    let rest = &line[i + pat.len()..];
    let colon = rest.find(':')?;
    let v = rest[colon + 1..].trim().trim_end_matches(',').trim();
    if v.starts_with("true") {
        Some(true)
    } else if v.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// `"Key" : "some text"` -> some text
fn json_str(line: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = line.find(&pat)?;
    let rest = &line[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Cheap stable digest of the raw log so a row can be traced back to its output
/// without shipping megabytes of logs (FNV-1a 64, hex, first 12 chars).
fn sha_prefix(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")[..12].to_string()
}

#[allow(dead_code)]
fn flush() {
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_name_recognition() {
        assert!(is_tuning_name("IceDrift200624"));
        assert!(is_tuning_name("AntiWallHit201021"));
        assert!(is_tuning_name("Reactors200605"));
        assert!(is_tuning_name("06/12/2019_TurboAirControl_Ice"));
        assert!(is_tuning_name("WallRepulse"));
        assert!(is_tuning_name("20fev2013"));
        assert!(!is_tuning_name("SomeRandomString"));
        assert!(!is_tuning_name("libstdc++.so.6"));
        // A plain number tail that is not a date must not qualify.
        assert!(!is_tuning_name("Buffer999999"));
    }

    #[test]
    fn tuning_dates() {
        assert_eq!(tuning_date("IceDrift200624").unwrap(), "2020-06-24");
        assert_eq!(
            tuning_date("06/12/2019_TurboAirControl_Ice").unwrap(),
            "2019-12-06"
        );
        assert_eq!(tuning_date("WallRepulse"), None);
    }

    #[test]
    fn dnf_negative_time_is_not_a_finish() {
        let out = "ValidatedResult\n  Time: -1\n  IsValid: true\n";
        let p = parse_server_output(out);
        assert_eq!(p.verdict, "DNF_TIME_NEGATIVE");
        assert_eq!(p.validated_ms, Some(-1));
    }

    #[test]
    fn finish_is_parsed() {
        // Real nested schema, as emitted from 2020-07-17 onward.
        let out = "{\n  \"ValidatedResult\" : {\n    \"Time\" : 63546\n  },\n  \"IsValid\" : true,\n  \"DeclaredResult\" : {\n    \"Time\" : 63546\n  }\n}\n";
        let p = parse_server_output(out);
        assert_eq!(p.verdict, "FINISH");
        assert_eq!(p.validated_ms, Some(63546));
        assert_eq!(p.declared_ms, Some(63546));
    }

    #[test]
    fn checkpoint_count_is_parsed() {
        let out = "wrong simu, but reached some checkpoints (3 out of 9)\n";
        let p = parse_server_output(out);
        assert_eq!(p.verdict, "WRONG_SIMU");
        assert_eq!(p.checkpoints, Some(3));
    }

    #[test]
    fn summary_counters_do_not_override_a_valid_run() {
        // Regression: the end-of-run summary block prints "Wrong Simu :   0% (  0)".
        // Reading that as a verdict turned a genuine finish into a failure and
        // would have inverted every boundary in the matrix.
        let out = "{\n  \"ValidatedResult\" : {\n \"Time\" : 26448\n},\n  \"IsValid\" : true,\n}\n\
                   Validating x.Replay.Gbx...\n\
                   ---------------- 1 replays parsed --------------\n\
                   Can't load :   0% (  0)\n\
                   Is Valid   : 100% (  1)\n\
                   Wrong Simu :   0% (  0)\n";
        let p = parse_server_output(out);
        assert_eq!(p.verdict, "FINISH");
        assert_eq!(p.validated_ms, Some(26448));
    }

    #[test]
    fn a_real_wrong_simu_line_still_registers() {
        let out = "Validating x.Replay.Gbx...\nwrong simu, but reached some checkpoints (3 out of 9)\n\
                   Wrong Simu : 100% (  1)\n";
        let p = parse_server_output(out);
        assert_eq!(p.verdict, "WRONG_SIMU");
        assert_eq!(p.checkpoints, Some(3));
    }

    #[test]
    fn time_delta_parsing() {
        assert_eq!(
            parse_time_delta("race finished, time is worse. (5017 < 5033)"),
            Some((5017, 5033))
        );
        assert_eq!(
            parse_time_delta("validated time is actually better! (25091 > 25082)"),
            Some((25091, 25082))
        );
        assert_eq!(parse_time_delta("wrong simu"), None);
    }

    #[test]
    fn ascii_and_utf16_extraction() {
        let mut buf = b"xx\x00IceDrift200624\x00".to_vec();
        let a = ascii_strings(&buf, 6);
        assert!(a.iter().any(|(_, s)| s == "IceDrift200624"));
        buf.clear();
        for c in "WallRepulse".bytes() {
            buf.push(c);
            buf.push(0);
        }
        let u = utf16_strings(&buf, 6);
        assert!(u.iter().any(|(_, s)| s == "WallRepulse"));
    }
}

#[cfg(test)]
mod real_format_tests {
    use super::*;

    const REAL_VALID: &str = r#"[
{
  "ValidatedResult" : {
    "NbCheckpoints" : 5,
    "NbRespawns" : 0,
    "Time" : 26448,
    "Score" : 0
  },
  "IsValid" : true,
  "DeclaredResult" : {
    "NbCheckpoints" : 5,
    "NbRespawns" : -1,
    "Time" : 26448,
    "Score" : 0
  },
  "GameBuild" : "Trackmania date=2020-07-07_20_18 Svn=105914 GameVersion=3.3.0",
  "FileName" : "2020-07-07_20_18__51307.Replay.Gbx"
}
]
Starting validation of 1 ghosts (in 1 maps)...
---------------- 1 replays parsed --------------
Can't load :   0% (  0)
Is Valid   : 100% (  1)
Wrong Simu :   0% (  0)
"#;

    const REAL_POLICY: &str = r#"[
{
  "IsValid" : false,
  "Desc" : "using known-flawed game exe 'Trackmania date=2020-07-07_20_18 Svn=105914 GameVersion=3.3.0'",
  "DeclaredResult" : { "Time" : 26448 }
}
]
Wrong Simu : 100% (  1)
"#;

    const REAL_WORSE: &str = r#"[
{
  "IsValid" : false,
  "Desc" : "race finished, time is worse. (5017 < 5033)",
  "DeclaredResult" : { "Time" : 5017 }
}
]
"#;

    #[test]
    fn real_valid_report_is_a_finish() {
        let p = parse_server_output(REAL_VALID);
        assert_eq!(p.verdict, "FINISH");
        assert_eq!(p.validated_ms, Some(26448));
        assert_eq!(p.declared_ms, Some(26448));
        assert_eq!(p.checkpoints, Some(5));
    }

    #[test]
    fn policy_refusal_is_not_a_physics_verdict() {
        let p = parse_server_output(REAL_POLICY);
        assert_eq!(p.verdict, "POLICY_KNOWN_FLAWED_EXE");
    }

    #[test]
    fn finish_at_a_different_time_is_its_own_verdict() {
        let p = parse_server_output(REAL_WORSE);
        assert_eq!(p.verdict, "FINISH_DIFFERENT_TIME");
        assert_eq!(parse_time_delta(&p.desc), Some((5017, 5033)));
    }
}
