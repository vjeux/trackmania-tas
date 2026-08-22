//! Golden-data verification of the racing-line comparison and clustering
//! against everything the Python left behind in `/tmp/entrec/reports`:
//!
//!   * `mycluster.json`        -- full-precision distance matrix, lateral
//!                                profiles, reference stations and the cluster
//!                                memberships at eps 1/2/5 from cluster_lines.py
//!   * `lines_eps1.txt`        -- lines.py's station-matched distance matrix
//!                                (2 dp) and its cluster count at eps 1
//!   * `lines_eps2/5.txt`      -- the same at eps 2 and 5
//!   * `lines_dtw2.txt`        -- lines.py --dtw matrix and count at eps 2
//!
//! Run with output: `cargo test --release --test golden_cluster -- --nocapture`

use tmtraj::json;
use tmtraj::lines::{self, Metric, Sort};
use std::collections::BTreeSet;

/// The member names of every group in a `lines.py` report, as a set of sets.
/// Its listing is `      NAME   TIME ms   D m from this line's seed`.
fn parse_printed_groups(txt: &str) -> BTreeSet<BTreeSet<&str>> {
    let mut out = BTreeSet::new();
    let mut cur: BTreeSet<&str> = BTreeSet::new();
    let mut in_groups = false;
    for l in txt.lines() {
        if l.contains("distinct line") {
            in_groups = true;
            continue;
        }
        if !in_groups {
            continue;
        }
        if l.starts_with("  line ") {
            if !cur.is_empty() {
                out.insert(std::mem::take(&mut cur));
            }
        } else if l.starts_with("      ") && l.contains(" ms ") {
            cur.insert(l.split_whitespace().next().unwrap());
        } else if l.trim().is_empty() && !cur.is_empty() {
            out.insert(std::mem::take(&mut cur));
            in_groups = false;
        }
    }
    if !cur.is_empty() {
        out.insert(cur);
    }
    out
}

const PATHS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/decoder-goldens/paths");
const REPORTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/decoder-goldens/reports");

fn max_dev(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f64, f64::max)
}

/// Parse the `%-6s` + `%8.2f`... matrix lines.py prints.
fn parse_printed_matrix(path: &str, n: usize) -> Vec<Vec<f64>> {
    let txt = std::fs::read_to_string(path).unwrap();
    let mut rows = Vec::new();
    for line in txt.lines().skip(2).take(n) {
        let b: Vec<char> = line.chars().collect();
        let mut row = Vec::with_capacity(n);
        for j in 0..n {
            let s: String = b[6 + j * 8..6 + (j + 1) * 8].iter().collect();
            row.push(s.trim().parse::<f64>().unwrap());
        }
        rows.push(row);
    }
    assert_eq!(rows.len(), n);
    rows
}

fn cluster_count(txt: &str) -> usize {
    for l in txt.lines() {
        if let Some(rest) = l.split(" distinct line").next() {
            if l.contains("distinct line") {
                return rest.trim().parse().unwrap();
            }
        }
    }
    panic!("no cluster count in report");
}

#[test]
fn projection_metric_matches_cluster_lines_py() {
    let runs = lines::load_dir(PATHS).unwrap();
    assert_eq!(runs.len(), 51);
    // cluster_lines.py was run with --stations 300 and the default reference
    // (the fastest run), sorting the runs by time.
    let an = lines::analyse(runs, Metric::Projection, 300, None, Sort::Time);

    let gold = json::parse(&std::fs::read_to_string(format!("{}/mycluster.json", REPORTS)).unwrap())
        .unwrap();
    let gnames: Vec<&str> = gold.get("names").unwrap().arr().iter().map(|v| v.str()).collect();
    assert_eq!(an.names, gnames, "run order differs");
    assert_eq!(an.ref_name(), gold.get("result").unwrap().get("reference").unwrap().str());

    // distance matrix
    let gd = gold.get("distance_matrix").unwrap().arr();
    let mut dmax = 0.0f64;
    for (i, row) in gd.iter().enumerate() {
        let g: Vec<f64> = row.arr().iter().map(|v| v.num()).collect();
        dmax = dmax.max(max_dev(&an.d[i], &g));
    }

    // lateral profiles
    let gp = gold.get("lateral_profiles").unwrap().obj_map();
    let mut pmax = 0.0f64;
    for (name, prof) in &an.profiles {
        let g: Vec<f64> = gp[name.as_str()].arr().iter().map(|v| v.num()).collect();
        pmax = pmax.max(max_dev(prof, &g));
    }

    // reference stations (s, x, y, z)
    let gr = gold.get("ref_stations").unwrap().arr();
    let mut smax = 0.0f64;
    for (i, p) in gr.iter().enumerate() {
        let g: Vec<f64> = p.arr().iter().map(|v| v.num()).collect();
        let mine = [
            an.ref_stations[i].0,
            an.ref_stations[i].1,
            an.ref_stations[i].2,
            an.ref_stations[i].3,
        ];
        smax = smax.max(max_dev(&mine, &g));
    }

    println!("\n=== PROJECTION METRIC vs cluster_lines.py (mycluster.json) ===");
    println!("  51 runs, 300 stations, reference {}", an.ref_name());
    println!("  reference path length {:.4} m (report says 1819.9)", an.ref_total);
    println!("  max |dev| distance matrix (1275 pairs) : {:.3e} m", dmax);
    println!("  max |dev| lateral profiles (15300 pts) : {:.3e} m", pmax);
    println!("  max |dev| reference stations (300x4)   : {:.3e} m", smax);

    // clusters
    let gcl = gold.get("result").unwrap().get("clusters").unwrap().obj_map();
    for (eps, key) in [(1.0, "1.0"), (2.0, "2.0"), (5.0, "5.0")] {
        let mut cl = lines::cluster(&an.d, eps);
        cl.sort_by_key(|c| c.iter().map(|&i| an.runs[i].time_ms).min().unwrap());
        let mine: Vec<Vec<String>> = cl
            .iter()
            .map(|c| {
                let mut m = c.clone();
                m.sort_by_key(|&i| an.runs[i].time_ms);
                m.into_iter().map(|i| an.names[i].clone()).collect()
            })
            .collect();
        let theirs: Vec<Vec<String>> = gcl[key]
            .arr()
            .iter()
            .map(|c| {
                c.get("members")
                    .unwrap()
                    .arr()
                    .iter()
                    .map(|v| v.str().to_string())
                    .collect()
            })
            .collect();
        let spread_dev = cl
            .iter()
            .zip(gcl[key].arr())
            .map(|(c, g)| (lines::spread(&an.d, c) - g.get("spread_m").unwrap().num()).abs())
            .fold(0.0f64, f64::max);
        println!(
            "  eps {:.1}: {} lines (python {}), memberships {}, max |dev| internal spread {:.3e} m",
            eps,
            mine.len(),
            theirs.len(),
            if mine == theirs { "IDENTICAL" } else { "DIFFER" },
            spread_dev
        );
        assert_eq!(mine, theirs, "cluster membership differs at eps {}", eps);
        assert!(spread_dev < 1e-12);
    }

    assert!(dmax < 1e-12, "distance matrix deviates by {}", dmax);
    assert!(pmax < 1e-12, "lateral profiles deviate by {}", pmax);
    assert!(smax < 1e-12, "reference stations deviate by {}", smax);
}

#[test]
fn station_and_dtw_metrics_match_lines_py() {
    let runs = lines::load_dir(PATHS).unwrap();
    // lines.py keeps the runs in filename order and defaults to 300 stations.
    let st = lines::analyse(runs.clone(), Metric::Station, 300, None, Sort::Name);
    let dt = lines::analyse(runs, Metric::Dtw, 300, None, Sort::Name);

    let g_st = parse_printed_matrix(&format!("{}/lines_eps1.txt", REPORTS), 51);
    let g_dtw = parse_printed_matrix(&format!("{}/lines_dtw2.txt", REPORTS), 51);
    let mut smax = 0.0f64;
    let mut dmax = 0.0f64;
    for i in 0..51 {
        smax = smax.max(max_dev(&st.d[i], &g_st[i]));
        dmax = dmax.max(max_dev(&dt.d[i], &g_dtw[i]));
    }
    println!("\n=== STATION / DTW METRICS vs lines.py ===");
    println!(
        "  max |dev| station-matched matrix vs the printed 2-dp table: {:.4} m (printing floor 0.005)",
        smax
    );
    println!(
        "  max |dev| DTW matrix vs the printed 2-dp table            : {:.4} m",
        dmax
    );
    assert!(smax <= 0.005 + 1e-9);
    assert!(dmax <= 0.005 + 1e-9);

    for (eps, file) in [(1.0, "lines_eps1.txt"), (2.0, "lines_eps2.txt"), (5.0, "lines_eps5.txt")] {
        let txt = std::fs::read_to_string(format!("{}/{}", REPORTS, file)).unwrap();
        let want = cluster_count(&txt);
        let cl = lines::cluster(&st.d, eps);
        let mine: BTreeSet<BTreeSet<&str>> = cl
            .iter()
            .map(|c| c.iter().map(|&i| st.names[i].as_str()).collect())
            .collect();
        let theirs = parse_printed_groups(&txt);
        println!(
            "  station metric, eps {:.1}: {} lines (python {}), memberships {}",
            eps,
            cl.len(),
            want,
            if mine == theirs { "IDENTICAL" } else { "DIFFER" }
        );
        assert_eq!(cl.len(), want);
        assert_eq!(mine, theirs, "membership differs at eps {}", eps);
    }
    let dtw_txt = std::fs::read_to_string(format!("{}/lines_dtw2.txt", REPORTS)).unwrap();
    let want = cluster_count(&dtw_txt);
    let cl = lines::cluster(&dt.d, 2.0);
    let mine: BTreeSet<BTreeSet<&str>> = cl
        .iter()
        .map(|c| c.iter().map(|&i| dt.names[i].as_str()).collect())
        .collect();
    println!(
        "  dtw metric,     eps 2.0: {} lines (python {}), memberships {}",
        cl.len(),
        want,
        if mine == parse_printed_groups(&dtw_txt) { "IDENTICAL" } else { "DIFFER" }
    );
    assert_eq!(cl.len(), want);
    assert_eq!(mine, parse_printed_groups(&dtw_txt));
}

/// `lines.py --demo`: two synthetic lines that really are distinct, so the
/// clustering has a known-good signature to reproduce (report: ~0.8 m within a
/// line, ~11.2 m between).
#[test]
fn demo_reproduces_the_synthetic_signature() {
    let an = lines::analyse(lines::demo(), Metric::Station, 300, None, Sort::Name);
    let cl = lines::cluster(&an.d, 2.0);
    assert_eq!(cl.len(), 2, "demo should split into exactly two lines");
    let within = cl.iter().map(|c| lines::spread(&an.d, c)).fold(0.0f64, f64::max);
    let mut between = f64::INFINITY;
    for &i in &cl[0] {
        for &j in &cl[1] {
            between = between.min(an.d[i][j]);
        }
    }
    println!(
        "\n=== DEMO (two synthetic lines) ===\n  within {:.2} m, between {:.2} m (python: 0.8 / 11.2)",
        within, between
    );
    assert!(within < 1.0);
    assert!(between > 10.0);
}
