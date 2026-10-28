//! `ghost wallclock` — the REAL-TIME forensics of ghost files.
//!
//! Every TM2020 ghost carries a validation chunk (`0x0309202D`) with two unix
//! timestamps: the real-time start and end of the attempt. For an honest run
//!
//! ```text
//!   walltime_end - walltime_start  ==  race time + countdown   (+/- 1 s)
//! ```
//!
//! because the game simulates in real time. A game that ran SLOWER than real
//! time -- a slowdown tool, a freeze -- leaves a longer span, and Nadeo's own
//! dedicated server refuses the file for it: `IsValid false, Desc "unexcepted
//! walltime (39s)"`. A game that ran FASTER than real time leaves a shorter
//! span, which the server does not check at all; the stamps are the only
//! instrument for that direction.
//!
//! Two subcommands:
//!
//! ```text
//!   ghost wallclock files DIR|FILE... [--tsv OUT]
//!   ghost wallclock census WALL.tsv [--records REC.tsv] [--flag-s 10] [--top N]
//! ```
//!
//! `files` reads a pile of ghosts straight from their bytes -- declared time,
//! countdown, wall span, excess, ratio, and the machine fingerprint the client
//! stamped beside the clock (OS/CPU class, exe checksum, settings flags,
//! Openplanet) -- one row a file, sorted by ratio. `census` reads that table
//! (plus, optionally, who set each record, at what rank, when) and writes the
//! report: the distribution, the base rate, who is flagged and whether their
//! ratios are the discrete fractions of a speed SETTING or the scatter of
//! stalls, whether their slow records are their good ones, and whether any two
//! of their runs overlap in real time (two people on one account).
//!
//! `excess` carries +/- 1 s of rounding (the stamps are whole seconds), which
//! is why the default flag is 10 s and never 1. The method, its calibration
//! against the server and the base rates it measured are written up in
//! `forensics/README.md` at the repo root.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cli::{die, flag};
use gbx::container::Container;
use gbx::tape::Tape;

/// The flag used throughout the investigations: a record is "slow" when its
/// real-time span exceeds race + countdown by at least `flag_s` seconds, OR by
/// at least 5 s at a ratio of 1.15 or more (short runs cannot accumulate 10 s
/// even at half speed). Nadeo's own threshold sits between +10.8 s and +15 s
/// depending on run length; this is slightly more inclusive on purpose.
fn is_slow(excess: f64, ratio: f64, flag_s: f64) -> bool {
    excess >= flag_s || (ratio >= 1.15 && excess >= 5.0)
}

/// A game clock that ran FASTER than the wall: no stall produces this.
fn is_fast(excess: f64) -> bool {
    excess <= -5.0
}

// ---------------------------------------------------------------------------
// the manifest JSON, read with a key scanner (no JSON crate in this workspace)
// ---------------------------------------------------------------------------

/// The raw text of `"key": value` in a flat JSON object: a string without its
/// quotes, or the bare token of a number / bool.
fn field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\":", key);
    let i = json.find(&pat)? + pat.len();
    let rest = json[i..].trim_start();
    if let Some(s) = rest.strip_prefix('"') {
        let mut end = 0;
        let b = s.as_bytes();
        while end < b.len() {
            if b[end] == b'\\' {
                end += 2;
                continue;
            }
            if b[end] == b'"' {
                break;
            }
            end += 1;
        }
        Some(&s[..end.min(s.len())])
    } else {
        let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
        Some(rest[..end].trim())
    }
}

fn num(json: &str, key: &str) -> Option<f64> {
    field(json, key)?.parse::<f64>().ok()
}

// ---------------------------------------------------------------------------
// one file
// ---------------------------------------------------------------------------

/// What one ghost says about its own real time.
pub struct WallRow {
    pub file: String,
    pub declared_ms: i64,
    pub countdown_ms: i64,
    pub walltime_start: i64,
    pub walltime_end: i64,
    pub client_build: String,
    pub openplanet: String,
    pub os_kind: String,
    pub cpu_kind: String,
    pub exe_checksum: String,
    pub settings_flags: String,
}

impl WallRow {
    /// Race time plus countdown, in seconds: what the wall span should equal.
    pub fn game_s(&self) -> f64 {
        (self.declared_ms + self.countdown_ms) as f64 / 1000.0
    }
    /// Stamps present and ordered. Identical stamps (a 0-second span) are a
    /// real population -- 0.2% of records, cause undocumented, refused by the
    /// server too -- and carry no timing information, so they are reported as
    /// absent rather than as a ratio of 0.
    pub fn stamps_ok(&self) -> bool {
        self.walltime_start > 1_000_000_000 && self.walltime_end > self.walltime_start
    }
    pub fn wall_s(&self) -> i64 {
        self.walltime_end - self.walltime_start
    }
    pub fn excess(&self) -> Option<f64> {
        if self.stamps_ok() && self.game_s() > 0.0 {
            Some(self.wall_s() as f64 - self.game_s())
        } else {
            None
        }
    }
    pub fn ratio(&self) -> Option<f64> {
        if self.stamps_ok() && self.game_s() > 0.0 {
            Some(self.wall_s() as f64 / self.game_s())
        } else {
            None
        }
    }
}

/// Read one ghost's wall clock and fingerprint. `Err` names what was missing.
pub fn read_file(path: &Path) -> Result<WallRow, String> {
    let p = path.to_str().ok_or("non-UTF-8 path")?;
    let c = Container::load(p)?;
    let tape = Tape::from_file(p)?;
    let so = tape.archives.first().map(|a| a.start_offset_ms as i64).unwrap_or(-1500);
    let declared = c.declared_times().first().map(|(_, v)| *v as i64).unwrap_or(0);
    let vj = gbx::manifest::validation_manifest(c.body()).ok_or("no validation chunk (0x0309202D)")?;
    let s = |k: &str| field(&vj, k).unwrap_or("").to_string();
    let build = s("exe_version").split(' ').nth(1).unwrap_or("").trim_start_matches("date=").to_string();
    let op = s("race_settings").split(" (").next().unwrap_or("").to_string();
    Ok(WallRow {
        file: path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        declared_ms: declared,
        countdown_ms: -so,
        walltime_start: num(&vj, "walltime_start").unwrap_or(0.0) as i64,
        walltime_end: num(&vj, "walltime_end").unwrap_or(0.0) as i64,
        client_build: build,
        openplanet: op,
        os_kind: s("os_kind"),
        cpu_kind: s("cpu_kind"),
        exe_checksum: s("exe_checksum"),
        settings_flags: s("settings_flags"),
    })
}

pub const FILES_HEADER: &str = "file\tdeclared_s\tcountdown_s\twall_s\texcess_s\tratio\twalltime_start\twalltime_end\tclient_build\topenplanet\tos_kind\tcpu_kind\texe_checksum\tsettings_flags";

fn files_line(r: &WallRow) -> String {
    let ok = r.stamps_ok();
    format!(
        "{}\t{:.3}\t{:.2}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        r.file,
        r.declared_ms as f64 / 1000.0,
        r.countdown_ms as f64 / 1000.0,
        if ok { r.wall_s().to_string() } else { "-".into() },
        match r.excess() {
            Some(e) => format!("{:+.2}", e),
            None => "stamps-absent".into(),
        },
        match r.ratio() {
            Some(x) => format!("{:.3}", x),
            None => "-".into(),
        },
        r.walltime_start,
        r.walltime_end,
        r.client_build,
        r.openplanet,
        r.os_kind,
        r.cpu_kind,
        r.exe_checksum,
        r.settings_flags
    )
}

pub fn cmd(rest: &[String]) {
    match rest.first().map(|s| s.as_str()) {
        Some("files") => cmd_files(&rest[1..]),
        Some("census") => cmd_census(&rest[1..]),
        _ => die(
            "usage: ghost wallclock files DIR|FILE... [--tsv OUT]\n       ghost wallclock census WALL.tsv [--records REC.tsv] [--flag-s 10] [--top N]",
        ),
    }
}

fn cmd_files(a: &[String]) {
    let tsv = flag(a, "--tsv").map(|s| s.to_string());
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut skip = false;
    for x in a {
        if skip {
            skip = false;
            continue;
        }
        if x == "--tsv" {
            skip = true;
            continue;
        }
        let p = PathBuf::from(x);
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&p)
                .unwrap_or_else(|e| die(format!("{}: {}", x, e)))
                .flatten()
                .map(|e| e.path())
                .filter(|q| {
                    let s = q.to_string_lossy();
                    s.ends_with(".Ghost.Gbx") || s.ends_with(".Replay.Gbx")
                })
                .collect();
            v.sort();
            paths.extend(v);
        } else {
            paths.push(p);
        }
    }
    if paths.is_empty() {
        die("ghost wallclock files: no files");
    }
    let mut rows: Vec<(f64, String)> = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for p in &paths {
        match read_file(p) {
            Ok(r) => {
                let key = r.ratio().unwrap_or(-1.0);
                rows.push((key, files_line(&r)));
            }
            Err(e) => bad.push(format!("{}: {}", p.display(), e)),
        }
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = String::from(FILES_HEADER);
    out.push('\n');
    for (_, l) in &rows {
        out.push_str(l);
        out.push('\n');
    }
    match tsv {
        Some(t) => {
            std::fs::write(&t, &out).unwrap_or_else(|e| die(format!("{}: {}", t, e)));
            eprintln!("wrote {} ({} rows, {} unreadable)", t, rows.len(), bad.len());
        }
        None => print!("{}", out),
    }
    // An unreadable file is a fact about the corpus, never a silent drop.
    for b in &bad {
        eprintln!("unreadable: {}", b);
    }
}

// ---------------------------------------------------------------------------
// the census
// ---------------------------------------------------------------------------

/// One row of a `files` table, joined with whatever the records table knows.
#[derive(Clone, Debug, Default)]
struct Rec {
    file: String,
    declared_s: f64,
    wall: Option<i64>,
    excess: Option<f64>,
    ratio: Option<f64>,
    ws: i64,
    we: i64,
    build: String,
    openplanet: String,
    // from --records
    player: String,
    rank: Option<i64>,
    set_utc: String,
    map: String,
}

impl Rec {
    fn slow(&self, flag_s: f64) -> bool {
        matches!((self.excess, self.ratio), (Some(e), Some(r)) if is_slow(e, r, flag_s))
    }
    fn fast(&self) -> bool {
        matches!(self.excess, Some(e) if is_fast(e))
    }
    fn absent(&self) -> bool {
        self.excess.is_none()
    }
    fn year(&self) -> String {
        self.set_utc.get(0..4).unwrap_or("?").to_string()
    }
}

fn read_tsv(path: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| die(format!("{}: {}", path, e)));
    let mut lines = text.lines();
    let header: Vec<String> = lines.next().unwrap_or("").split('\t').map(|s| s.trim().to_string()).collect();
    let rows = lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|s| s.to_string()).collect())
        .collect();
    (header, rows)
}

fn col(header: &[String], name: &str) -> Option<usize> {
    header.iter().position(|h| h == name)
}

fn basename(s: &str) -> &str {
    s.rsplit('/').next().unwrap_or(s)
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Which of the speed-setting fractions a ratio sits on, if any: the game at
/// 1/2, 2/5, 1/4, 3/4 or 1/3 speed. A stall gives a ratio that varies with
/// the map; a setting gives one of these on every map.
fn discrete_name(r: f64) -> Option<&'static str> {
    const TABLE: [(f64, &str); 5] = [(2.0, "x2.0"), (2.5, "x2.5"), (4.0, "x4.0"), (1.333, "x1.33"), (3.0, "x3.0")];
    TABLE.iter().find(|(v, _)| (r - v).abs() <= 0.06).map(|(_, n)| *n)
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 {
        "-".into()
    } else {
        format!("{:.2}%", 100.0 * n as f64 / d as f64)
    }
}

fn cmd_census(a: &[String]) {
    let wall = a.first().filter(|s| !s.starts_with("--")).unwrap_or_else(|| die("ghost wallclock census WALL.tsv [--records REC.tsv] [--flag-s 10] [--top N]"));
    let flag_s = flag(a, "--flag-s").and_then(|s| s.parse::<f64>().ok()).unwrap_or(10.0);
    let top = flag(a, "--top").and_then(|s| s.parse::<usize>().ok()).unwrap_or(40);

    let (h, rows) = read_tsv(wall);
    let need = |n: &str| col(&h, n).unwrap_or_else(|| die(format!("{}: no column {:?} (is this a `ghost wallclock files` table?)", wall, n)));
    let (cf, cd, cw, ce, cr, cws, cwe, cb, co) = (
        need("file"),
        need("declared_s"),
        need("wall_s"),
        need("excess_s"),
        need("ratio"),
        need("walltime_start"),
        need("walltime_end"),
        need("client_build"),
        need("openplanet"),
    );
    let g = |r: &Vec<String>, i: usize| -> String { r.get(i).cloned().unwrap_or_default() };
    let mut recs: Vec<Rec> = rows
        .iter()
        .map(|r| Rec {
            file: g(r, cf),
            declared_s: g(r, cd).parse().unwrap_or(0.0),
            wall: g(r, cw).parse().ok(),
            excess: g(r, ce).parse().ok(),
            ratio: g(r, cr).parse().ok(),
            ws: g(r, cws).parse().unwrap_or(0),
            we: g(r, cwe).parse().unwrap_or(0),
            build: g(r, cb),
            openplanet: g(r, co),
            ..Default::default()
        })
        .collect();

    // the records table: who, what rank, when, which map -- joined on the file name
    let mut joined = 0usize;
    if let Some(rp) = flag(a, "--records") {
        let (rh, rrows) = read_tsv(rp);
        let rf = col(&rh, "file").unwrap_or_else(|| die(format!("{}: no column \"file\"", rp)));
        let (rpl, rrk, rts, rmp) = (col(&rh, "player"), col(&rh, "rank"), col(&rh, "set_utc"), col(&rh, "map"));
        let mut by: BTreeMap<String, &Vec<String>> = BTreeMap::new();
        for r in &rrows {
            by.insert(basename(&g(r, rf)).to_string(), r);
        }
        for rec in recs.iter_mut() {
            if let Some(r) = by.get(basename(&rec.file)) {
                joined += 1;
                if let Some(i) = rpl {
                    rec.player = g(r, i);
                }
                if let Some(i) = rrk {
                    rec.rank = g(r, i).parse().ok();
                }
                if let Some(i) = rts {
                    rec.set_utc = g(r, i);
                }
                if let Some(i) = rmp {
                    rec.map = g(r, i);
                }
            }
        }
    }

    let n = recs.len();
    let slow: Vec<&Rec> = recs.iter().filter(|r| r.slow(flag_s)).collect();
    let fast: Vec<&Rec> = recs.iter().filter(|r| r.fast()).collect();
    let absent = recs.iter().filter(|r| r.absent()).count();
    let within = |lo: f64, hi: f64| recs.iter().filter(|r| matches!(r.excess, Some(e) if e >= lo && e < hi)).count();

    println!("WALL-CLOCK CENSUS of {} ({} records{})", wall, n, if joined > 0 { format!(", {} joined with --records", joined) } else { String::new() });
    println!("flag: excess >= {:.0} s, or ratio >= 1.15 with excess >= 5 s; fast: excess <= -5 s\n", flag_s);

    println!("DISTRIBUTION of excess = wall span - (race + countdown)");
    println!("  within +/-1 s        {:>7}  {}", within(-1.0, 1.0), pct(within(-1.0, 1.0), n));
    println!("  within +/-2 s        {:>7}  {}", within(-2.0, 2.0), pct(within(-2.0, 2.0), n));
    println!("  +2 .. +{:<3.0} s          {:>7}  {}", flag_s, within(2.0, flag_s), pct(within(2.0, flag_s), n));
    println!("  SLOW (flagged)       {:>7}  {}", slow.len(), pct(slow.len(), n));
    println!("  FAST (<= -5 s)       {:>7}  {}   <- no stall produces this; Nadeo does not check it", fast.len(), pct(fast.len(), n));
    println!("  stamps absent/0-span {:>7}  {}   (refused by Nadeo as 'unexcepted walltime (0s)')\n", absent, pct(absent, n));

    // ratio histogram of the slow ones, 0.1 bins
    if !slow.is_empty() {
        println!("RATIO HISTOGRAM of the flagged records (wall / game, 0.1 bins) -- peaks at exactly x2.0 / x2.5 / x4.0 are a speed SETTING, scatter is stalls");
        let mut bins: BTreeMap<i64, usize> = BTreeMap::new();
        for r in &slow {
            *bins.entry((r.ratio.unwrap_or(0.0) * 10.0).floor() as i64).or_default() += 1;
        }
        for (b, c) in &bins {
            println!("  x{:<5.1} {:>5} {}", *b as f64 / 10.0, c, "#".repeat((*c).min(60)));
        }
        let disc = slow.iter().filter(|r| r.ratio.and_then(discrete_name).is_some()).count();
        println!("  on a speed-setting fraction (x2.0 x2.5 x3.0 x4.0 x1.33 +/-0.06): {} of {}\n", disc, slow.len());
    }

    // by client build and Openplanet
    println!("BY CLIENT BUILD (records / flagged / rate)");
    let mut bb: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in &recs {
        let e = bb.entry(if r.build.is_empty() { "?".into() } else { r.build.clone() }).or_default();
        e.0 += 1;
        if r.slow(flag_s) {
            e.1 += 1;
        }
    }
    for (b, (t, f)) in &bb {
        println!("  {:<18} {:>7} {:>5}  {}", b, t, f, pct(*f, *t));
    }
    let (mut op_t, mut op_f, mut no_t, mut no_f) = (0, 0, 0, 0);
    for r in &recs {
        let s = r.slow(flag_s) as usize;
        if r.openplanet.contains("Openplanet") {
            op_t += 1;
            op_f += s;
        } else {
            no_t += 1;
            no_f += s;
        }
    }
    println!("\nOPENPLANET present {:>7} records, {} flagged ({});  absent {:>7} records, {} flagged ({})\n", op_t, op_f, pct(op_f, op_t), no_t, no_f, pct(no_f, no_t));

    if joined == 0 {
        print_flagged(&recs, flag_s, top);
        println!("\n(no --records table: rank, year, per-player and overlap sections need one -- columns file, player, rank, set_utc, map)");
        return;
    }

    // by rank
    println!("BY RANK (flag rate) -- a tool pays at the top; stalls hit whatever run was in progress");
    let buckets: [(&str, i64, i64); 6] = [("rank 1", 1, 1), ("rank 2-3", 2, 3), ("rank 4-10", 4, 10), ("rank 11-100", 11, 100), ("rank 101-1000", 101, 1000), ("rank 1001+", 1001, i64::MAX)];
    for (name, lo, hi) in buckets {
        let in_b: Vec<&Rec> = recs.iter().filter(|r| matches!(r.rank, Some(k) if k >= lo && k <= hi)).collect();
        let f = in_b.iter().filter(|r| r.slow(flag_s)).count();
        println!("  {:<14} {:>7} records {:>5} flagged  {}", name, in_b.len(), f, pct(f, in_b.len()));
    }

    // by year
    println!("\nBY YEAR OF THE RECORD");
    let mut yy: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in &recs {
        let e = yy.entry(r.year()).or_default();
        e.0 += 1;
        if r.slow(flag_s) {
            e.1 += 1;
        }
    }
    for (y, (t, f)) in &yy {
        println!("  {:<6} {:>7} records {:>5} flagged  {}", y, t, f, pct(*f, *t));
    }

    // per player
    println!("\nPLAYERS with a flagged record (records / flagged / fast / absent; ratios; the two populations)");
    println!("  CHRONIC = >= 3 flagged and >= 50% of their records: the ratios decide between a setting and a slow machine");
    println!("  SPORADIC = otherwise: compare the flagged records' ranks with the player's own median rank");
    let mut pp: BTreeMap<String, Vec<&Rec>> = BTreeMap::new();
    for r in &recs {
        pp.entry(if r.player.is_empty() { "?".into() } else { r.player.clone() }).or_default().push(r);
    }
    let mut flagged_players: Vec<(&String, &Vec<&Rec>)> = pp.iter().filter(|(_, v)| v.iter().any(|r| r.slow(flag_s) || r.fast() || r.absent())).collect();
    flagged_players.sort_by(|a, b| {
        let fa = a.1.iter().filter(|r| r.slow(flag_s)).count();
        let fb = b.1.iter().filter(|r| r.slow(flag_s)).count();
        fb.cmp(&fa).then(a.0.cmp(b.0))
    });
    let mut chronic_total = 0usize;
    for (name, v) in &flagged_players {
        let t = v.len();
        let f: Vec<&&Rec> = v.iter().filter(|r| r.slow(flag_s)).collect();
        let fast = v.iter().filter(|r| r.fast()).count();
        let abs = v.iter().filter(|r| r.absent()).count();
        let ratios: Vec<String> = f.iter().filter_map(|r| r.ratio).map(|x| format!("{:.2}", x)).collect();
        let on_frac = f.iter().filter(|r| r.ratio.and_then(discrete_name).is_some()).count();
        let chronic = f.len() >= 3 && f.len() * 2 >= t;
        if chronic {
            chronic_total += f.len();
        }
        let mut all_ranks: Vec<f64> = v.iter().filter_map(|r| r.rank).map(|k| k as f64).collect();
        let mut slow_ranks: Vec<f64> = f.iter().filter_map(|r| r.rank).map(|k| k as f64).collect();
        let kind = if chronic {
            format!("CHRONIC ({} of {} flagged ratios on a setting fraction)", on_frac, f.len())
        } else if f.is_empty() {
            "fast/absent only".to_string()
        } else {
            format!("SPORADIC (median rank of flagged {} vs own median {})", median(&mut slow_ranks), median(&mut all_ranks))
        };
        println!("  {:<22} {:>5} / {:>3} / {} / {}  ratios [{}]  {}", name, t, f.len(), fast, abs, ratios.join(" "), kind);
    }
    if !slow.is_empty() {
        println!("  chronic players account for {} of the {} flagged records ({})", chronic_total, slow.len(), pct(chronic_total, slow.len()));
    }

    // overlap test: two runs of one player whose real-time windows intersect
    println!("\nOVERLAP TEST -- two runs of one account whose real-time windows intersect were not driven by one person");
    let mut ov = 0usize;
    for (name, v) in &pp {
        let mut w: Vec<(i64, i64, &str)> = v.iter().filter(|r| r.ws > 1_000_000_000 && r.we > r.ws).map(|r| (r.ws, r.we, r.file.as_str())).collect();
        w.sort();
        for i in 1..w.len() {
            if w[i].0 < w[i - 1].1 {
                ov += 1;
                let same = w[i].0 == w[i - 1].0 && w[i].1 == w[i - 1].1;
                println!(
                    "  OVERLAP {:<20} {} .. {}  vs  {} .. {}   ({} | {}){}",
                    name,
                    w[i - 1].0,
                    w[i - 1].1,
                    w[i].0,
                    w[i].1,
                    w[i - 1].2,
                    w[i].2,
                    if same { "  <- identical window: the same run saved twice, not two drivers" } else { "" }
                );
            }
        }
    }
    println!("  overlapping pairs: {}", ov);

    // selectivity: for players with >= 10 records, does the slow rate rise with run quality?
    println!("\nSELECTIVITY -- flag rate by rank bucket, players with >= 10 records and >= 1 flag (a tool switched on for the good runs shows a gradient)");
    for (name, v) in &flagged_players {
        if v.len() < 10 || !v.iter().any(|r| r.slow(flag_s)) {
            continue;
        }
        let cell = |lo: i64, hi: i64| -> String {
            let b: Vec<&&Rec> = v.iter().filter(|r| matches!(r.rank, Some(k) if k >= lo && k <= hi)).collect();
            let f = b.iter().filter(|r| r.slow(flag_s)).count();
            if b.is_empty() {
                "   -  ".into()
            } else {
                format!("{:>2}/{:<3}", f, b.len())
            }
        };
        println!("  {:<22} rank1 {}  2-3 {}  4-10 {}  11-100 {}  101+ {}", name, cell(1, 1), cell(2, 3), cell(4, 10), cell(11, 100), cell(101, i64::MAX));
    }

    print_flagged(&recs, flag_s, top);
}

fn print_flagged(recs: &[Rec], flag_s: f64, top: usize) {
    let mut v: Vec<&Rec> = recs.iter().filter(|r| r.slow(flag_s) || r.fast() || r.absent()).collect();
    v.sort_by(|a, b| {
        let ka = a.excess.map(|e| e.abs()).unwrap_or(-1.0);
        let kb = b.excess.map(|e| e.abs()).unwrap_or(-1.0);
        kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
    });
    println!("\nFLAGGED RECORDS (top {} by |excess|; {} in all)", top.min(v.len()), v.len());
    println!("  {:<10} {:<20} {:>5} {:>9} {:>6} {:>8} {:>7}  {:<16} {}", "set", "player", "rank", "time_s", "wall", "excess", "ratio", "build", "map / file");
    for r in v.iter().take(top) {
        println!(
            "  {:<10} {:<20} {:>5} {:>9.3} {:>6} {:>8} {:>7}  {:<16} {}",
            r.set_utc.get(0..10).unwrap_or("-"),
            if r.player.is_empty() { "-" } else { &r.player },
            r.rank.map(|k| k.to_string()).unwrap_or("-".into()),
            r.declared_s,
            r.wall.map(|w| w.to_string()).unwrap_or("-".into()),
            r.excess.map(|e| format!("{:+.1}", e)).unwrap_or("absent".into()),
            r.ratio.map(|x| format!("x{:.3}", x)).unwrap_or("-".into()),
            r.build,
            if r.map.is_empty() { &r.file } else { &r.map }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_scanner_reads_strings_and_numbers() {
        let j = r#"{"u01":0,"exe_version":"Trackmania date=2026-02-02_17_51 git=1","exe_checksum":1623925506,"walltime_start":1787685520,"walltime_end":1787685537,"race_settings":"Openplanet 1.29.5 (next, Public, 1234ad9e)"}"#;
        assert_eq!(field(j, "exe_version"), Some("Trackmania date=2026-02-02_17_51 git=1"));
        assert_eq!(num(j, "walltime_end"), Some(1787685537.0));
        assert_eq!(field(j, "exe_checksum"), Some("1623925506"));
        assert_eq!(field(j, "race_settings").map(|s| s.split(" (").next().unwrap()), Some("Openplanet 1.29.5"));
    }

    #[test]
    fn the_flag_is_the_investigations_flag() {
        // deeperjungle's bounty run: 17 s of wall for 16.94 s of race + countdown
        assert!(!is_slow(0.06, 1.003, 10.0));
        // AshenBeast1985's Miru's Hell 2: 39 s for 10.6 s
        assert!(is_slow(28.38, 3.671, 10.0));
        // a 12-second run at half speed never reaches +10 s; the ratio arm catches it
        assert!(is_slow(6.0, 1.5, 10.0));
        // and a 3-second run with +2 s of rounding noise does not
        assert!(!is_slow(2.2, 1.46, 10.0));
        assert!(is_fast(-25.4));
        assert!(!is_fast(-0.9));
    }

    #[test]
    fn discrete_fractions() {
        assert_eq!(discrete_name(2.02), Some("x2.0"));
        assert_eq!(discrete_name(2.49), Some("x2.5"));
        assert_eq!(discrete_name(4.05), Some("x4.0"));
        assert_eq!(discrete_name(1.476), None);
        assert_eq!(discrete_name(5.685), None);
    }

    #[test]
    fn wallrow_arithmetic() {
        let r = WallRow {
            file: "x".into(),
            declared_ms: 9075,
            countdown_ms: 1550,
            walltime_start: 1787345051,
            walltime_end: 1787345090,
            client_build: String::new(),
            openplanet: String::new(),
            os_kind: String::new(),
            cpu_kind: String::new(),
            exe_checksum: String::new(),
            settings_flags: String::new(),
        };
        assert!(r.stamps_ok());
        assert_eq!(r.wall_s(), 39);
        assert!((r.excess().unwrap() - 28.375).abs() < 1e-9);
        assert!((r.ratio().unwrap() - 3.6706).abs() < 1e-3);
        let z = WallRow { walltime_end: 1787345051, ..r };
        assert!(!z.stamps_ok());
        assert!(z.excess().is_none());
    }
}
