//! `tmtraj` command line. Arguments are parsed by hand, in the style of the
//! sibling `tmsearch` crate.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use tmtraj::entrec::{self, Decoded};
use tmtraj::json::fmt_g6;
use tmtraj::lines::{self, Analysis, Metric, Sort};
use tmtraj::selftest;

const USAGE: &str = "\
tmtraj -- TM2020 ghost trajectory decoder and racing-line analysis

  tmtraj decode GHOST.Gbx [--csv OUT.csv] [--json OUT.json] [--full-json OUT.json]
                          [--head N]
        Decode one ghost; print the header and the first N samples.
        --csv       the 29-column CSV entrec.write_csv produced
        --json      the compact per-run path JSON decode_all.py produced
        --full-json every field of every sample

  tmtraj decode-all DIR... [--out-json DIR] [--out-csv DIR] [--jobs N]
        Decode every *.Ghost.Gbx under the given directories, in parallel,
        and write the same JSON/CSV artefacts decode_all.py did.

  tmtraj fields
        Print the field confidence table (VERIFIED / DERIVED / GUESS).

  tmtraj selftest
        Validate the decoder against independent ground truth.

  tmtraj cluster --dir DIR [--stations N] [--eps E...] [--ref NAME]
                 [--metric projection|station|dtw] [--sort time|name]
                 [--out FILE] [--no-plots]
        Full racing-line report: per-run lateral summary, pairwise distance
        distribution, clusters + seed per line at each eps, ASCII plots.

  tmtraj compare --dir DIR [--stations N] [--metric M] [--ref NAME]
        Just the pairwise distance matrix and its distribution.

  tmtraj stats --dir DIR [--stations N] [--ref NAME]
        Population analysis: separation histogram, centrality of the
        reference run, lateral spread along the lap, most separated pair,
        sector times, speed profile vs the field median.

  tmtraj demo [--eps E...]
        Run the clustering on lines.py's two synthetic lines (sanity check:
        ~0.8 m within a line, ~11 m between).

  tmtraj tail scan GHOST... [--tsv OUT] [--thr M] [-v]
        Physical-continuity scan of the vehicle record: per step, metres moved
        vs metres the record's OWN speed field allows. A carrier tail left
        behind by a transplant shows up as metres of unexplainable
        displacement. Also counts samples before 0.000 and after the finish.

  tmtraj tail fix GHOST --out OUT (--cut MS | --keep N | --auto [--thr M])
        Truncate the vehicle entity's sample list. --auto cuts at the last
        sample before the first step whose excess is over --thr.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print!("{}", USAGE);
        std::process::exit(2);
    }
    let cmd = args[0].clone();
    let rest = &args[1..];
    let code = match cmd.as_str() {
        "decode" => cmd_decode(rest),
        "decode-all" => cmd_decode_all(rest),
        "fields" => {
            entrec::print_field_confidence();
            0
        }
        "selftest" => {
            let r = selftest::selftest(true);
            if !r.skipped.is_empty() {
                println!("(skipped: {})", r.skipped.join(", "));
            }
            i32::from(!r.ok)
        }
        "tail" => cmd_tail(rest),
        "nan" => { tmtraj::nancmd::cmd(rest); 0 }
        "whl" => { tmtraj::whlcmd::cmd(rest); 0 }
        "facing" => tmtraj::facingcmd::cmd(rest),

        "rectime" => match rest.first().map(|s| s.as_str()) {
            Some("cmp") => tmtraj::rectimecmd::cmd_cmp(&rest[1..]),
            Some("lag") => tmtraj::rectimecmd::cmd_lag(&rest[1..]),
            _ => { tmtraj::rectimecmd::cmd(rest); 0 }
        },

        "recspan" => { tmtraj::recspancmd::cmd(rest); 0 }

        "setdecl" => { tmtraj::setdeclcmd::cmd(rest); 0 }

        "anon" => { tmtraj::anoncmd::cmd(rest); 0 }

        "intg" => { tmtraj::intgcmd::cmd(rest); 0 }

        "check" => { tmtraj::checkcmd::cmd(rest); 0 }
        "cluster" => cmd_cluster(rest, true),
        "rec" => cmd_rec(rest),
        "recdiff" => cmd_recdiff(rest),
        "hdr" => cmd_hdr(rest),
        "body" => cmd_body(rest),
        "compare" => cmd_cluster(rest, false),
        "stats" => cmd_stats(rest),
        "demo" => cmd_demo(rest),
        "-h" | "--help" | "help" => {
            print!("{}", USAGE);
            0
        }
        other => {
            eprintln!("unknown command {:?}\n", other);
            print!("{}", USAGE);
            2
        }
    };
    std::process::exit(code);
}

// ---------------------------------------------------------------------------
// tiny hand-rolled flag parsing
// ---------------------------------------------------------------------------

struct Args {
    flags: BTreeMap<String, Vec<String>>,
    positional: Vec<String>,
}

fn parse_args(a: &[String], valueless: &[&str]) -> Args {
    let mut flags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut positional = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if let Some(name) = a[i].strip_prefix("--") {
            let name = name.to_string();
            i += 1;
            let mut vals = Vec::new();
            if !valueless.contains(&name.as_str()) {
                while i < a.len() && !a[i].starts_with("--") {
                    vals.push(a[i].clone());
                    i += 1;
                }
            }
            flags.entry(name).or_default().extend(vals);
        } else {
            positional.push(a[i].clone());
            i += 1;
        }
    }
    Args { flags, positional }
}

impl Args {
    fn has(&self, k: &str) -> bool {
        self.flags.contains_key(k)
    }
    fn one(&self, k: &str) -> Option<&str> {
        self.flags.get(k).and_then(|v| v.first()).map(|s| s.as_str())
    }
    fn many(&self, k: &str) -> Vec<String> {
        self.flags.get(k).cloned().unwrap_or_default()
    }
    fn usize_or(&self, k: &str, d: usize) -> usize {
        self.one(k).map(|v| v.parse().expect("integer")).unwrap_or(d)
    }
}

// ---------------------------------------------------------------------------
// decode
// ---------------------------------------------------------------------------

fn cmd_decode(a: &[String]) -> i32 {
    let a = parse_args(a, &[]);
    let Some(path) = a.positional.first() else {
        eprintln!("usage: tmtraj decode GHOST.Gbx [--csv f] [--json f] [--full-json f]");
        return 2;
    };
    let dec = match entrec::decode_ghost(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("FAIL {}: {}", path, e);
            return 1;
        }
    };
    println!(
        "version {}  samples {}  period {} ms  sample_size {}  start {} end {}",
        dec.version,
        dec.samples.len(),
        dec.sample_period_ms
            .map_or("None".into(), |v| v.to_string()),
        dec.sample_size,
        dec.start_ms,
        dec.end_ms
    );
    println!("checkpoints (ms): {:?}", dec.checkpoints_ms);
    let ents: Vec<String> = dec
        .ents
        .iter()
        .map(|e| {
            format!(
                "('0x{:08X}', {}, {})",
                e.class_id.unwrap_or(0),
                e.n_samples,
                e.sample_size
            )
        })
        .collect();
    println!("entities: [{}]", ents.join(", "));
    println!(
        "{:>8} {:>10} {:>8} {:>10} {:>9} {:>6} {:>5}",
        "t", "x", "y", "z", "km/h", "gear", "rpm"
    );
    let head = a.usize_or("head", 10);
    for s in dec.samples.iter().take(head) {
        println!(
            "{:>8} {:>10.3} {:>8.3} {:>10.3} {:>9.2} {:>6.1} {:>5}",
            s.time_ms, s.x, s.y, s.z, s.speed_kmh, s.gear, s.rpm_raw
        );
    }
    if let Some(f) = a.one("csv") {
        std::fs::write(f, entrec::csv_string(&dec)).expect("write csv");
        println!("wrote {}", f);
    }
    if let Some(f) = a.one("json") {
        std::fs::write(f, entrec::path_json_string(&dec)).expect("write json");
        println!("wrote {}", f);
    }
    if let Some(f) = a.one("full-json") {
        std::fs::write(f, entrec::full_json_string(&dec)).expect("write json");
        println!("wrote {}", f);
    }
    0
}

// ---------------------------------------------------------------------------
// decode-all  (decode_all.py, parallel)
// ---------------------------------------------------------------------------

fn cmd_decode_all(a: &[String]) -> i32 {
    let a = parse_args(a, &[]);
    let dirs: Vec<String> = if a.positional.is_empty() {
        a.many("dir")
    } else {
        a.positional.clone()
    };
    if dirs.is_empty() {
        eprintln!("usage: tmtraj decode-all DIR... [--out-json DIR] [--out-csv DIR] [--jobs N]");
        return 2;
    }
    let mut ghosts: Vec<String> = Vec::new();
    for d in &dirs {
        let Ok(rd) = std::fs::read_dir(d) else {
            eprintln!("cannot read dir {}", d);
            continue;
        };
        let mut here: Vec<String> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path().to_string_lossy().to_string())
            .filter(|p| p.ends_with(".Ghost.Gbx"))
            .collect();
        here.sort();
        ghosts.extend(here);
    }
    let out_json = a.one("out-json").map(|s| s.to_string());
    let out_csv = a.one("out-csv").map(|s| s.to_string());
    for d in [&out_json, &out_csv].into_iter().flatten() {
        std::fs::create_dir_all(d).expect("mkdir");
    }
    let jobs = a.usize_or(
        "jobs",
        std::thread::available_parallelism().map_or(8, |v| v.get()),
    );

    let next = AtomicUsize::new(0);
    let rows: Mutex<Vec<(String, String, usize, String, usize, bool, Vec<i32>)>> =
        Mutex::new(Vec::new());
    let fails: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..jobs.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= ghosts.len() {
                    break;
                }
                let p = &ghosts[i];
                if std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) == 0 {
                    println!("SKIP empty {}", p);
                    continue;
                }
                let nm = entrec::name_for(p);
                match entrec::decode_ghost(p) {
                    Err(e) => fails.lock().unwrap().push(format!("FAIL {}: {}", nm, e)),
                    Ok(dec) => {
                        if let Some(d) = &out_json {
                            std::fs::write(
                                format!("{}/{}.json", d, nm),
                                entrec::path_json_string(&dec),
                            )
                            .expect("write json");
                        }
                        if let Some(d) = &out_csv {
                            std::fs::write(format!("{}/{}.csv", d, nm), entrec::csv_string(&dec))
                                .expect("write csv");
                        }
                        rows.lock().unwrap().push((
                            nm,
                            dec.race_time_ms.map_or("None".into(), |v| v.to_string()),
                            dec.samples.len(),
                            dec.sample_period_ms
                                .map_or("None".into(), |v| v.to_string()),
                            dec.sample_size,
                            dec.bytes_consumed == dec.bytes_total,
                            dec.checkpoints_ms.clone(),
                        ));
                    }
                }
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    for f in fails.into_inner().unwrap() {
        println!("{}", f);
    }
    println!(
        "{:<22} {:>7} {:>6} {:>5} {:>5} {:>5} {}",
        "run", "time", "nsamp", "per", "ssz", "exact", "checkpoints"
    );
    for r in &rows {
        println!(
            "{:<22} {:>7} {:>6} {:>5} {:>5} {:>5} {:?}",
            r.0,
            r.1,
            r.2,
            r.3,
            r.4,
            if r.5 { "True" } else { "False" },
            r.6
        );
    }
    println!(
        "decoded {} ghosts{}",
        rows.len(),
        match &out_json {
            Some(d) => format!(" -> {}", d),
            None => String::new(),
        }
    );
    0
}

// ---------------------------------------------------------------------------
// cluster / compare  (cluster_lines.py + lines.py, unified)
// ---------------------------------------------------------------------------

fn cmd_cluster(a: &[String], full: bool) -> i32 {
    let a = parse_args(a, &["no-plots"]);
    let Some(dir) = a.one("dir").map(|s| s.to_string()).or_else(|| a.positional.first().cloned())
    else {
        eprintln!("usage: tmtraj cluster --dir DIR [--eps E...] [--metric M] [--stations N]");
        return 2;
    };
    let runs = match lines::load_dir(&dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    if runs.len() < 2 {
        eprintln!("need at least 2 runs (got {})", runs.len());
        return 1;
    }
    let metric = Metric::parse(a.one("metric").unwrap_or("projection")).expect("bad --metric");
    let sort = match a.one("sort").unwrap_or("time") {
        "name" => Sort::Name,
        _ => Sort::Time,
    };
    let stations = a.usize_or("stations", if metric == Metric::Projection { 400 } else { 300 });
    let eps: Vec<f64> = {
        let v: Vec<f64> = a.many("eps").iter().map(|s| s.parse().unwrap()).collect();
        if v.is_empty() {
            vec![1.0, 2.0, 5.0]
        } else {
            v
        }
    };
    let an = lines::analyse(runs, metric, stations, a.one("ref"), sort);
    report(&an, &eps, full, !a.has("no-plots"), a.one("out"));
    0
}

fn cmd_stats(a: &[String]) -> i32 {
    let a = parse_args(a, &[]);
    let Some(dir) = a
        .one("dir")
        .map(|s| s.to_string())
        .or_else(|| a.positional.first().cloned())
    else {
        eprintln!("usage: tmtraj stats --dir DIR [--stations N] [--ref NAME]");
        return 2;
    };
    let runs = match lines::load_dir(&dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let an = lines::analyse(
        runs,
        Metric::Projection,
        a.usize_or("stations", 300),
        a.one("ref"),
        Sort::Time,
    );
    tmtraj::stats::print_stats(&an);
    0
}

fn cmd_demo(a: &[String]) -> i32 {
    let a = parse_args(a, &[]);
    let eps: Vec<f64> = {
        let v: Vec<f64> = a.many("eps").iter().map(|s| s.parse().unwrap()).collect();
        if v.is_empty() {
            vec![2.0]
        } else {
            v
        }
    };
    let metric = Metric::parse(a.one("metric").unwrap_or("station")).expect("bad --metric");
    let an = lines::analyse(lines::demo(), metric, a.usize_or("stations", 300), None, Sort::Name);
    report(&an, &eps, true, false, a.one("out"));
    0
}

fn report(an: &Analysis, eps_list: &[f64], full: bool, plots: bool, out: Option<&str>) {
    let n = an.names.len();
    println!(
        "{} runs, {} .. {} ms; reference = {}   [metric: {}]",
        n,
        an.runs.iter().map(|r| r.time_ms).min().unwrap(),
        an.runs.iter().map(|r| r.time_ms).max().unwrap(),
        an.ref_name(),
        an.metric.name()
    );
    println!(
        "reference path length {:.1} m over {} stations ({:.2} m spacing)",
        an.ref_total,
        an.stations,
        an.ref_total / (an.stations - 1) as f64
    );

    let cps = an.cp_stations();
    let cp = |k: &str| cps.iter().find(|(n, _)| *n == k).unwrap().1;

    if full {
        println!();
        println!(
            "PER-RUN SUMMARY (lateral offset vs {}, + = left of the reference heading)",
            an.ref_name()
        );
        println!(
            "{:<18} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "run", "time", "pathlen", "rms_lat", "max_lat", "CP1", "CP2", "CP3", "vmax"
        );
        for r in &an.runs {
            let p = &an.profiles[&r.name];
            let max_lat = p
                .iter()
                .cloned()
                .fold(0.0f64, |acc, v| if v.abs() > acc.abs() { v } else { acc });
            println!(
                "{:<18} {:>7} {:>8.1} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.1}",
                r.name,
                r.time_ms,
                an.totals[&r.name],
                lines::rms(p),
                max_lat,
                p[cp("CP1")],
                p[cp("CP2")],
                p[cp("CP3")],
                r.v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            );
        }
    }

    println!();
    println!("PAIRWISE SEPARATION (m) -- distribution");
    let allp = an.pair_distances();
    if !allp.is_empty() {
        println!(
            "  min {:.2}  p25 {:.2}  median {:.2}  p75 {:.2}  max {:.2}  ({} pairs)",
            allp[0],
            allp[allp.len() / 4],
            allp[allp.len() / 2],
            allp[3 * allp.len() / 4],
            allp[allp.len() - 1],
            allp.len()
        );
        let ri = an.ref_idx;
        let mut vs: Vec<f64> = (0..n).filter(|&j| j != ri).map(|j| an.d[ri][j]).collect();
        vs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "  vs {} specifically: min {:.2}  median {:.2}  max {:.2}",
            an.ref_name(),
            vs[0],
            vs[(n - 1) / 2],
            vs[vs.len() - 1]
        );
        let mean = allp.iter().sum::<f64>() / allp.len() as f64;
        let sd = (allp.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / allp.len() as f64).sqrt();
        let gap = allp
            .windows(2)
            .map(|w| w[1] - w[0])
            .fold(0.0f64, f64::max);
        println!(
            "  mean {:.2}  sd {:.2}  largest gap anywhere in the sorted list {:.3} m",
            mean, sd, gap
        );
    }

    let mut cl_out: Vec<(f64, Vec<Vec<usize>>)> = Vec::new();
    for &eps in eps_list {
        let mut cl = lines::cluster(&an.d, eps);
        cl.sort_by_key(|c| c.iter().map(|&i| an.runs[i].time_ms).min().unwrap());
        println!();
        println!(
            "=== eps = {:.1} m : {} distinct line{} ===",
            eps,
            cl.len(),
            if cl.len() == 1 { "" } else { "s" }
        );
        for (ci, c) in cl.iter().enumerate() {
            let mut mem: Vec<usize> = c.clone();
            mem.sort_by_key(|&i| an.runs[i].time_ms);
            let names: Vec<&str> = mem.iter().map(|&i| an.names[i].as_str()).collect();
            println!(
                "  line {} (n={:2}, internal spread {:.2} m, seed {} @ {} ms): {}",
                ci + 1,
                c.len(),
                lines::spread(&an.d, c),
                an.names[mem[0]],
                an.runs[mem[0]].time_ms,
                names.join(", ")
            );
        }
        if cl.len() > 1 {
            let mut seps = Vec::new();
            for ci in 0..cl.len() {
                for cj in (ci + 1)..cl.len() {
                    let mut m = f64::INFINITY;
                    for &i in &cl[ci] {
                        for &j in &cl[cj] {
                            m = m.min(an.d[i][j]);
                        }
                    }
                    seps.push(m);
                }
            }
            println!(
                "  min separation between distinct lines: {:.2} m",
                seps.iter().cloned().fold(f64::INFINITY, f64::min)
            );
        }
        cl_out.push((eps, cl));
    }

    if plots && full {
        println!();
        println!("OVERHEAD VIEW (x-z), all runs; C=CP, F=finish, S=start");
        let labels: Vec<(String, char)> = an
            .names
            .iter()
            .map(|nm| {
                (
                    nm.clone(),
                    if nm == an.ref_name() { 'W' } else { '.' },
                )
            })
            .collect();
        println!(
            "{}",
            lines::ascii_xz(&an.stations_by_run, &labels, lines::MAP_GEOM, 96, 40)
        );

        println!();
        println!(
            "SPEED vs DISTANCE ALONG LAP (km/h vs m):  W = {}, . = others",
            an.ref_name()
        );
        let mut series: Vec<(char, Vec<(f64, f64)>)> = an
            .names
            .iter()
            .filter(|nm| *nm != an.ref_name())
            .map(|nm| {
                (
                    '.',
                    an.stations_by_run[nm].iter().map(|p| (p.0, p.4)).collect(),
                )
            })
            .collect();
        series.push((
            'W',
            an.stations_by_run[an.ref_name()]
                .iter()
                .map(|p| (p.0, p.4))
                .collect(),
        ));
        println!("{}", lines::ascii_series(&series, 100, 18, "km/h", "m along lap"));

        println!();
        println!("LATERAL OFFSET vs DISTANCE ALONG LAP (m):  0 = the reference line");
        let series: Vec<(char, Vec<(f64, f64)>)> = an
            .names
            .iter()
            .filter(|nm| *nm != an.ref_name())
            .map(|nm| {
                (
                    '.',
                    (0..an.stations)
                        .map(|i| (an.ref_stations[i].0, an.profiles[nm][i]))
                        .collect(),
                )
            })
            .collect();
        println!(
            "{}",
            lines::ascii_series(&series, 100, 18, "m (+ = left of ref)", "m along lap")
        );
    }

    if let Some(f) = out {
        std::fs::write(f, lines::clusters_json(an, eps_list, &cl_out)).expect("write out");
        println!("\nwrote {}", f);
    }
    let _ = fmt_g6(0.0);
}

#[allow(dead_code)]
fn unused(_: &Decoded) {}

// ---------------------------------------------------------------------------
// `tmtraj rec` -- inspect and rewrite the CPlugEntRecordData node
// ---------------------------------------------------------------------------

/// `rec info GHOST` / `rec roundtrip GHOST [--out F]`.
///
/// `roundtrip` is the control that licenses every telemetry edit: decode the
/// record, re-encode it with no change, and require the payload to come back
/// BYTE-IDENTICAL. An encoder that cannot reproduce an untouched record cannot
/// be trusted to write a changed one.
fn cmd_rec(args: &[String]) -> i32 {
    use tmtraj::entrec::{find_entrecord_blob, load_body, parse_record_data};
    use tmtraj::recwrite::{encode_record_data, find_rec_site, rewrite_ghost};
    if args.len() < 2 {
        eprintln!("usage: tmtraj rec info|roundtrip GHOST [--out OUT]");
        return 2;
    }
    let sub = args[0].as_str();
    let path = args[1].clone();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let body = match load_body(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}", e);
            return 3;
        }
    };
    let site = match find_rec_site(&body) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            return 3;
        }
    };
    let (ver, blob) = match find_entrecord_blob(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            return 3;
        }
    };
    let rd = match parse_record_data(&blob, ver) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return 3;
        }
    };
    match sub {
        "info" => {
            println!(
                "body {} B, record node at {:#x}: version {}, raw {} B, zlib {} B",
                body.len(),
                site.hdr,
                site.version,
                site.usize_,
                site.csize
            );
            match site.skip_chunk {
                Some((cid, coff, poff, sz)) => println!(
                    "  framed by skippable chunk {:#010x} at {:#x} (payload {:#x}, {} B)",
                    cid, coff, poff, sz
                ),
                None => println!("  NOT inside a skippable chunk"),
            }
            println!(
                "  {} .. {} ms, {} descs, {} entities, consumed {} of {} B",
                rd.start_ms,
                rd.end_ms,
                rd.descs.len(),
                rd.ents.len(),
                rd.bytes_consumed,
                rd.bytes_total
            );
            for (i, e) in rd.ents.iter().enumerate() {
                println!(
                    "  ent[{}] type {} class {:#010x} samples {} sample_size {} deltas2 {}",
                    i,
                    e.type_,
                    rd.descs.get(e.type_ as usize).map(|d| d.class_id).unwrap_or(0),
                    e.times.len(),
                    e.sample_size,
                    e.deltas2.len()
                );
            }
            0
        }
        "reencode" => {
            // Decode every sample's transform and write it straight back. The
            // encoder's rounding conventions are right only if the bytes come
            // back identical; this is the control that has to pass BEFORE any
            // regenerated value is written into a published file.
            use tmtraj::entrec::read_transform_pub;
            use tmtraj::recwrite::{write_transform, Xform};
            let mut n = 0usize;
            let mut same = 0usize;
            let mut worst: Vec<(usize, Vec<u8>, Vec<u8>)> = Vec::new();
            for e in rd.ents.iter().filter(|e| e.sample_size >= 100) {
                let ss = e.sample_size;
                for i in 0..e.times.len() {
                    let d = &e.raw[i * ss..(i + 1) * ss];
                    let (pos, quat, _sp, vel) = read_transform_pub(d, 47);
                    let mut out = d.to_vec();
                    write_transform(
                        &mut out,
                        47,
                        &Xform {
                            pos: [pos[0] as f32, pos[1] as f32, pos[2] as f32],
                            quat,
                            vel,
                        },
                    );
                    n += 1;
                    if out[47..69] == d[47..69] {
                        same += 1;
                    } else if worst.len() < 5 {
                        worst.push((i, d[47..69].to_vec(), out[47..69].to_vec()));
                    }
                }
            }
            println!(
                "re-encode of the transform: {} of {} samples byte-identical ({:.2}%)",
                same,
                n,
                100.0 * same as f64 / n.max(1) as f64
            );
            for (i, a, b) in &worst {
                println!("  sample {}\n    rec {:02x?}\n    gen {:02x?}", i, a, b);
            }
            i32::from(same != n)
        }
        "roundtrip" => {
            let re = encode_record_data(&rd);
            let same = re == blob;
            println!(
                "roundtrip: {} B in, {} B out, {}",
                blob.len(),
                re.len(),
                if same { "BYTE-IDENTICAL" } else { "DIFFERS" }
            );
            if !same {
                let n = re.len().min(blob.len());
                let first = (0..n).find(|i| re[*i] != blob[*i]);
                println!("  first difference at {:?}", first);
                return 1;
            }
            if let Some(o) = out {
                match rewrite_ghost(&path, &o, |_| Ok(())) {
                    Ok((a, b)) => println!("wrote {} (record {} -> {} B)", o, a, b),
                    Err(e) => {
                        eprintln!("{}", e);
                        return 3;
                    }
                }
            }
            0
        }
        other => {
            eprintln!("unknown rec subcommand {:?}", other);
            2
        }
    }
}

/// `tmtraj recdiff A B` -- per-byte agreement between two ghosts' vehicle
/// samples, with the "does this byte carry information at all" column that
/// decides whether an agreement is meaningful.
fn cmd_recdiff(args: &[String]) -> i32 {
    use tmtraj::entrec::{find_entrecord_blob, load_body, parse_record_data};
    if args.len() < 2 {
        eprintln!("usage: tmtraj recdiff A.Ghost.Gbx B.Ghost.Gbx [--csv OUT]");
        return 2;
    }
    let get = |p: &str| -> (Vec<i32>, Vec<u8>, usize) {
        let body = load_body(p).unwrap();
        let (v, blob) = find_entrecord_blob(&body).unwrap();
        let rd = parse_record_data(&blob, v).unwrap();
        let e = rd
            .ents
            .iter()
            .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
            .max_by_key(|e| e.times.len())
            .expect("no vehicle entity");
        (e.times.clone(), e.raw.clone(), e.sample_size)
    };
    let (ta, ra, sa) = get(&args[0]);
    let (tb, rb, sb) = get(&args[1]);
    if sa != sb {
        println!("sample sizes differ: {} vs {}", sa, sb);
        return 1;
    }
    let n = ta.len().min(tb.len());
    if ta[..n] != tb[..n] {
        println!("WARNING: sample times differ");
    }
    let mut same = vec![0usize; sa];
    let mut near = vec![0usize; sa];
    let mut varies = vec![std::collections::HashSet::new(); sa];
    for i in 0..n {
        for b in 0..sa {
            let x = ra[i * sa + b];
            let y = rb[i * sa + b];
            if x == y {
                same[b] += 1;
            }
            if (x as i32 - y as i32).abs() <= 1 {
                near[b] += 1;
            }
            varies[b].insert(x);
        }
    }
    println!("{} samples compared, sample size {}", n, sa);
    println!("byte  distinct(A)  identical%  within1%");
    let mut nvary = 0usize;
    let mut nvary_same = 0usize;
    for b in 0..sa {
        let d = varies[b].len();
        if d > 1 {
            nvary += 1;
            if same[b] == n {
                nvary_same += 1;
            }
        }
        println!(
            "{:>4}  {:>11}  {:>9.2}  {:>8.2}",
            b,
            d,
            100.0 * same[b] as f64 / n as f64,
            100.0 * near[b] as f64 / n as f64
        );
    }
    println!(
        "{} of {} bytes carry information in A; {} of those are identical in B",
        nvary, sa, nvary_same
    );
    // The DUPLICATE-TELEMETRY test, in one line. Two ghosts that encode
    // different runs must not carry the same recorded motion; before this fix,
    // 17 of the repo's 29 multi-ghost maps did, which is why two of our own
    // tapes rendered as a single car.
    let total = n * sa;
    let ident: usize = same.iter().sum();
    println!(
        "VERDICT {} ({} of {} sample bytes identical, {:.2}%)",
        if ident == total { "IDENTICAL-TELEMETRY" } else { "DIFFERENT-RUNS" },
        ident,
        total,
        100.0 * ident as f64 / total as f64
    );
    0
}

/// `tmtraj hdr info|setlogin` -- the GBX header's user-data block.
///
/// The recorded login sits there as a plain length-prefixed string in an
/// UNCOMPRESSED part of the file (`strings` finds it), so it can be rewritten
/// without touching the body at all -- but the chunk's own size field and the
/// user-data total both have to move with it, which is why this is a tool and
/// not a byte poke.
fn cmd_hdr(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: tmtraj hdr info FILE | tmtraj hdr setlogin FILE OUT NAME");
        return 2;
    }
    let sub = args[0].as_str();
    let path = &args[1];
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: {}", path, e);
            return 3;
        }
    };
    let g4 = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    if &data[0..3] != b"GBX" {
        eprintln!("not a GBX file");
        return 3;
    }
    let version = u16::from_le_bytes(data[3..5].try_into().unwrap());
    // "GBX"(3) + u16 version + u8 format + u8 refcomp + u8 bodycomp = 8
    let mut o = 8usize;
    if version >= 4 {
        o += 1;
    }
    o += 4; // class id
    if version < 6 {
        println!("no user data (version {})", version);
        return 0;
    }
    let ud_size_at = o;
    let ud_size = g4(&data, o) as usize;
    o += 4;
    let ud_start = o;
    let nch = g4(&data, o) as usize;
    o += 4;
    let mut chunks: Vec<(u32, usize, usize)> = Vec::new(); // id, size, payload offset
    let mut poff = ud_start + 4 + nch * 8;
    for i in 0..nch {
        let id = g4(&data, o + i * 8);
        let sz = (g4(&data, o + i * 8 + 4) & 0x7FFF_FFFF) as usize;
        chunks.push((id, sz, poff));
        poff += sz;
    }
    match sub {
        "info" => {
            println!("version {}, user data {} B, {} header chunks", version, ud_size, nch);
            for (id, sz, off) in &chunks {
                let mut strs: Vec<String> = Vec::new();
                let end = (off + sz).min(data.len());
                let mut p = *off;
                while p + 4 <= end {
                    let n = g4(&data, p) as usize;
                    if n > 0 && n < 256 && p + 4 + n <= end {
                        if let Ok(s) = std::str::from_utf8(&data[p + 4..p + 4 + n]) {
                            if s.chars().all(|c| !c.is_control()) {
                                strs.push(format!("@{} {:?}", p - off, s));
                                p += 4 + n;
                                continue;
                            }
                        }
                    }
                    p += 1;
                }
                println!("  chunk {:#010x} {:>6} B: {}", id, sz, strs.join(" "));
            }
            0
        }
        "setlogin" => {
            if args.len() < 4 {
                eprintln!("usage: tmtraj hdr setlogin FILE OUT NAME");
                return 2;
            }
            let out = &args[2];
            let name = args[3].as_bytes();
            // Every plain length-prefixed string in the header that looks like a
            // login or nickname is replaced. Which ones those are is decided by
            // POSITION, not by guessing the chunk's schema: the strings that
            // equal the file's own recorded login.
            let mut hits: Vec<(usize, usize)> = Vec::new(); // (offset of length field, old len)
            for (_, sz, off) in &chunks {
                let end = (off + sz).min(data.len());
                let mut p = *off;
                while p + 4 <= end {
                    let n = g4(&data, p) as usize;
                    if n > 0 && n < 64 && p + 4 + n <= end {
                        if let Ok(s) = std::str::from_utf8(&data[p + 4..p + 4 + n]) {
                            if !s.is_empty()
                                && s.chars().all(|c| c.is_ascii_graphic())
                                && !s.contains('/')
                                && !s.contains('\\')
                                && !s.starts_with("http")
                                && s != "CarSport"
                                && s != "Nadeo"
                                && s != "Stadium"
                            {
                                hits.push((p, n));
                                p += 4 + n;
                                continue;
                            }
                        }
                    }
                    p += 1;
                }
            }
            if hits.is_empty() {
                eprintln!("no login-shaped string found in the header");
                return 1;
            }
            for (p, n) in &hits {
                println!(
                    "  replacing {:?} at {} with {:?}",
                    std::str::from_utf8(&data[p + 4..p + 4 + n]).unwrap_or("?"),
                    p,
                    std::str::from_utf8(name).unwrap()
                );
            }
            // rebuild
            let mut outb: Vec<u8> = Vec::with_capacity(data.len() + 64);
            outb.extend_from_slice(&data[..ud_start]);
            let mut newud: Vec<u8> = Vec::new();
            newud.extend_from_slice(&(nch as u32).to_le_bytes());
            let tbl_at = newud.len();
            newud.extend_from_slice(&data[ud_start + 4..ud_start + 4 + nch * 8]);
            let mut sizes: Vec<usize> = Vec::new();
            for (_, sz, off) in &chunks {
                let end = (off + sz).min(data.len());
                let mut body: Vec<u8> = Vec::with_capacity(*sz + 16);
                let mut p = *off;
                while p < end {
                    if let Some((hp, hn)) = hits.iter().find(|(hp, _)| *hp == p) {
                        body.extend_from_slice(&(name.len() as u32).to_le_bytes());
                        body.extend_from_slice(name);
                        p = hp + 4 + hn;
                    } else {
                        body.push(data[p]);
                        p += 1;
                    }
                }
                sizes.push(body.len());
                newud.extend_from_slice(&body);
            }
            for (i, sz) in sizes.iter().enumerate() {
                let flag = g4(&data, ud_start + 4 + i * 8 + 4) & 0x8000_0000;
                let v = (*sz as u32) | flag;
                newud[tbl_at + i * 8 + 4..tbl_at + i * 8 + 8].copy_from_slice(&v.to_le_bytes());
            }
            outb[ud_size_at..ud_size_at + 4].copy_from_slice(&(newud.len() as u32).to_le_bytes());
            outb.extend_from_slice(&newud);
            outb.extend_from_slice(&data[ud_start + ud_size..]);
            if let Err(e) = std::fs::write(out, &outb) {
                eprintln!("{}: {}", out, e);
                return 3;
            }
            println!(
                "wrote {} ({} -> {} B, user data {} -> {} B, {} string(s) replaced)",
                out,
                data.len(),
                outb.len(),
                ud_size,
                newud.len(),
                hits.len()
            );
            0
        }
        other => {
            eprintln!("unknown hdr subcommand {:?}", other);
            2
        }
    }
}

/// `tmtraj body login FILE` / `tmtraj body setlogin FILE OUT NAME`
///
/// The recorded driver name lives in the ghost node in the BODY (these files
/// carry an uncompressed body, which is why `strings` finds it). Replacing it
/// changes a length-prefixed string, so any enclosing SKIPPABLE chunk's size
/// field has to move with it -- the same fixup the record rewrite already does.
fn cmd_body(args: &[String]) -> i32 {
    use tmtraj::gbx::{all_skip_chunks, Gbx};
    if args.len() < 2 {
        eprintln!("usage: tmtraj body login FILE | tmtraj body setlogin FILE OUT NAME");
        return 2;
    }
    let sub = args[0].as_str();
    let path = &args[1];
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: {}", path, e);
            return 3;
        }
    };
    let g = Gbx::parse(&data);
    let body = &g.body;
    let g4 = |o: usize| u32::from_le_bytes(body[o..o + 4].try_into().unwrap());
    // plain length-prefixed strings in the first part of the body
    let known = [
        "CarSport", "CarSnow", "CarRally", "CarDesert", "Nadeo", "Stadium", "Trackmania",
    ];
    let mut cands: Vec<(usize, String)> = Vec::new();
    let mut p = 0usize;
    let lim = body.len().min(4096);
    while p + 4 <= lim {
        let n = g4(p) as usize;
        if n > 0 && n < 64 && p + 4 + n <= body.len() {
            if let Ok(s) = std::str::from_utf8(&body[p + 4..p + 4 + n]) {
                if s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
                    && !s.contains('\\')
                    && !s.starts_with("http")
                    && !known.contains(&s)
                {
                    cands.push((p, s.to_string()));
                    p += 4 + n;
                    continue;
                }
            }
        }
        p += 1;
    }
    match sub {
        "login" => {
            for (o, s) in &cands {
                println!("  body@{:<6} {:?}", o, s);
            }
            if cands.is_empty() {
                println!("  (no login-shaped string in the first {} B of the body)", lim);
            }
            0
        }
        "setlogin" => {
            if args.len() < 4 {
                eprintln!("usage: tmtraj body setlogin FILE OUT NAME");
                return 2;
            }
            let out = &args[2];
            let name = args[3].as_bytes();
            let Some((_, old)) = cands.first().cloned() else {
                eprintln!("no login-shaped string found");
                return 1;
            };
            // every occurrence of that exact string, as a length-prefixed string
            let mut hits: Vec<usize> = Vec::new();
            let ob = old.as_bytes();
            let mut p = 0usize;
            while p + 4 + ob.len() <= body.len() {
                if g4(p) as usize == ob.len() && &body[p + 4..p + 4 + ob.len()] == ob {
                    hits.push(p);
                    p += 4 + ob.len();
                } else {
                    p += 1;
                }
            }
            let skips = all_skip_chunks(body);
            let mut nb: Vec<u8> = Vec::with_capacity(body.len() + 64);
            let mut last = 0usize;
            for h in &hits {
                nb.extend_from_slice(&body[last..*h]);
                nb.extend_from_slice(&(name.len() as u32).to_le_bytes());
                nb.extend_from_slice(name);
                last = h + 4 + ob.len();
            }
            nb.extend_from_slice(&body[last..]);
            // fix the size of every skippable chunk that contained a hit
            let delta = name.len() as i64 - ob.len() as i64;
            for (_, coff, poff, sz) in &skips {
                let inside = hits.iter().filter(|h| **h >= *poff && **h < *poff + *sz).count() as i64;
                if inside > 0 {
                    // the chunk header sits at the same offset in the new body only
                    // if no earlier hit shifted it
                    let shift: i64 =
                        hits.iter().filter(|h| **h < *coff).count() as i64 * delta;
                    let at = (*coff as i64 + shift) as usize;
                    let cur = u32::from_le_bytes(nb[at + 8..at + 12].try_into().unwrap()) as i64;
                    let nv = (cur + inside * delta) as u32;
                    nb[at + 8..at + 12].copy_from_slice(&nv.to_le_bytes());
                }
            }
            let mut file = g.header_bytes_u();
            file.extend_from_slice(&nb);
            if let Err(e) = std::fs::write(out, &file) {
                eprintln!("{}: {}", out, e);
                return 3;
            }
            println!(
                "wrote {}: {:?} -> {:?}, {} occurrence(s), body {} -> {} B",
                out,
                old,
                std::str::from_utf8(name).unwrap(),
                hits.len(),
                body.len(),
                nb.len()
            );
            0
        }
        other => {
            eprintln!("unknown body subcommand {:?}", other);
            2
        }
    }
}

// ---------------------------------------------------------------------------
// tail -- the carrier's post-tape telemetry tail
// ---------------------------------------------------------------------------

fn cmd_tail(args: &[String]) -> i32 {
    use tmtraj::tailcmd;
    if args.is_empty() {
        eprintln!("usage: tmtraj tail scan|fix ...");
        return 2;
    }
    let sub = args[0].as_str();
    let a = parse_args(&args[1..], &["v", "verbose", "auto", "dry-run"]);
    let thr: f64 = a.one("thr").map(|v| v.parse().expect("float")).unwrap_or(0.5);
    match sub {
        "scan" => {
            if a.positional.is_empty() {
                eprintln!("usage: tmtraj tail scan GHOST... [--tsv OUT] [--thr M] [-v]");
                return 2;
            }
            tailcmd::cmd_scan(
                &a.positional,
                a.one("tsv"),
                thr,
                a.has("v") || a.has("verbose"),
                a.one("at").map(|v| v.parse().expect("integer")),
            )
        }
        "plan" => {
            let Some(covp) = a.one("cov") else {
                eprintln!("tail plan needs --cov tg_coverage_v3.tsv");
                return 2;
            };
            tailcmd::cmd_plan(&a.positional, covp, thr, a.one("tsv"))
        }
        "verify" => {
            let (Some(br), Some(ar)) = (a.one("before"), a.one("after")) else {
                eprintln!("tail verify needs --before --after");
                return 2;
            };
            let rel: f64 = a.one("rel").map(|v| v.parse().expect("float")).unwrap_or(10.0);
            tailcmd::cmd_verify(
                &a.positional,
                br,
                ar,
                a.one("times"),
                a.one("abs").map(|v| v.parse().expect("float")).unwrap_or(0.30),
                rel,
                a.one("tsv"),
            )
        }
        "finishcheck" => {
            let Some(covp) = a.one("cov") else {
                eprintln!("tail finishcheck needs --cov");
                return 2;
            };
            tailcmd::cmd_finishcheck(&a.positional, covp, a.one("tsv"))
        }
        "apply" => {
            let (Some(covp), Some(inr), Some(outr)) = (a.one("cov"), a.one("in"), a.one("out")) else {
                eprintln!("tail apply needs --cov --in --out");
                return 2;
            };
            let rel: f64 = a.one("rel").map(|v| v.parse().expect("float")).unwrap_or(10.0);
            tailcmd::cmd_apply(
                &a.positional,
                inr,
                outr,
                covp,
                a.one("ours"),
                a.one("times"),
                a.one("abs").map(|v| v.parse().expect("float")).unwrap_or(0.30),
                rel,
                a.one("tsv"),
            )
        }
        "fix" => {
            let Some(path) = a.positional.first() else {
                eprintln!("usage: tmtraj tail fix GHOST --out OUT (--cut MS | --keep N | --auto)");
                return 2;
            };
            let Some(out) = a.one("out") else {
                eprintln!("tail fix needs --out");
                return 2;
            };
            let sc = match tailcmd::scan_file(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{}: {}", path, e);
                    return 3;
                }
            };
            // decide the keep count
            let keep: usize = if let Some(k) = a.one("keep") {
                k.parse().expect("integer")
            } else if let Some(c) = a.one("cut") {
                let cut: i32 = c.parse().expect("integer ms");
                sc.steps
                    .iter()
                    .take_while(|s| s.t0 <= cut)
                    .count()
                    .max(1)
                    .min(sc.n)
                    + usize::from(sc.t_first <= cut && sc.n > 0)
                    - usize::from(sc.t_first <= cut && sc.n > 0)
                    + 0
            } else if a.has("auto") {
                let over = sc.over(thr);
                match over.first() {
                    // the jump is the step i -> i+1, so the last genuine
                    // sample is index i, i.e. keep i+1 samples
                    Some(s) => s.i + 1,
                    None => sc.n,
                }
            } else {
                eprintln!("tail fix needs one of --cut MS, --keep N, --auto");
                return 2;
            };
            // --cut is easier to compute directly from the times
            let keep = if let Some(c) = a.one("cut") {
                let cut: i32 = c.parse().expect("integer ms");
                let body = tmtraj::entrec::load_body(path).unwrap();
                let (v, blob) = tmtraj::entrec::find_entrecord_blob(&body).unwrap();
                let rd = tmtraj::entrec::parse_record_data(&blob, v).unwrap();
                let e = tmtraj::tailcmd::vehicle_ent(&rd).unwrap();
                tailcmd::keep_count(&e.times, cut)
            } else {
                keep
            };
            if keep >= sc.n {
                println!(
                    "{}\tNOCHANGE\t{} samples, nothing to cut",
                    path, sc.n
                );
                if let Err(e) = std::fs::copy(path, out) {
                    eprintln!("{}: {}", out, e);
                    return 3;
                }
                return 0;
            }
            let r = tmtraj::recwrite::rewrite_ghost(path, out, |rd| {
                tailcmd::truncate_vehicle(rd, keep).map(|_| ())
            });
            match r {
                Ok((a0, b0)) => {
                    println!(
                        "{}\tCUT\t{} -> {} samples\trecord {} -> {} B\t{}",
                        path, sc.n, keep, a0, b0, out
                    );
                    0
                }
                Err(e) => {
                    eprintln!("{}: {}", path, e);
                    3
                }
            }
        }
        other => {
            eprintln!("unknown tail subcommand {:?}", other);
            2
        }
    }
}
