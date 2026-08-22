//! `site` and `compact` build byte-identical pages from a committed set of
//! trajectories, and `stats` measures the built page rather than trusting the
//! builder's own bookkeeping.

mod common;

use common::*;

fn paths_dir() -> String {
    let d = fixture("testdata/paths");
    let n = std::fs::read_dir(&d).unwrap().count();
    assert_eq!(n, 5, "the trajectory fixture set is {} files, expected 5", n);
    d.to_string_lossy().into_owned()
}

fn build(cmd: &str, extra: &[&str], name: &str) -> (std::path::PathBuf, Run) {
    let d = scratch(name);
    let out = d.join(format!("{}.html", name));
    let mut args: Vec<String> = vec![
        cmd.into(),
        "--dir".into(),
        paths_dir(),
        "--out".into(),
        out.to_string_lossy().into_owned(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    let r = tmsite(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    r.ok(cmd);
    (out, r)
}

#[test]
fn site_page_is_byte_identical_to_the_golden() {
    let (out, r) = build("site", &[], "site-stride1");
    assert_eq!(
        r.stdout.trim_end().split("  ").nth(1),
        Some("(5 runs, 1966 samples, 61.0 KB)"),
        "builder summary changed: {:?}",
        r.stdout
    );
    assert_golden("testdata/golden/site_stride1.html", &std::fs::read(out).unwrap());
}

#[test]
fn compact_page_is_byte_identical_to_the_golden() {
    let (out, _) = build("compact", &[], "compact-stride3");
    assert_golden("testdata/golden/compact_stride3.html", &std::fs::read(out).unwrap());
}

#[test]
fn compact_pick_keeps_the_fastest_the_slowest_and_a_spread() {
    let (out, r) = build("compact", &["--pick", "3", "--stride", "5"], "compact-pick3");
    assert!(r.stdout.contains("3 runs"), "{}", r.stdout);
    let html = std::fs::read_to_string(&out).unwrap();
    let s = tmsite::stats::analyse(&html).unwrap();
    assert_eq!(s.runs, 3);
    assert_eq!(
        s.names,
        vec!["p00001_19538", "p00004_19556", "slow_p10000_19812"],
        "pick must keep the fastest and the slowest"
    );
    // --pick 1 has no spread to compute; it must be an error, not a panic
    let d = scratch("compact-pick1");
    let r = tmsite(&[
        "compact",
        "--dir",
        &paths_dir(),
        "--out",
        d.join("x.html").to_str().unwrap(),
        "--pick",
        "1",
    ]);
    r.failed("compact --pick 1");
    assert!(r.stderr.contains("--pick must be >= 2"), "{}", r.stderr);
}

#[test]
fn runs_are_ordered_by_time_and_ties_keep_filename_order() {
    // 05_19556.json and p00004_19556.json declare the SAME time, and the
    // filename order (05 before p00004) is what CPython's stable sort keeps.
    // This pins the ORDER the page's legend is built in. Note what it does NOT
    // pin: swapping the stable `sort_by_key` for `sort_unstable_by_key` leaves
    // this five-run set in the same order, so the test does not discriminate
    // stable from unstable -- it catches a reordering, not the mechanism.
    let (out, _) = build("site", &[], "site-order");
    let html = std::fs::read_to_string(&out).unwrap();
    let s = tmsite::stats::analyse(&html).unwrap();
    assert_eq!(
        s.names,
        vec!["p00001_19538", "05_19556", "p00004_19556", "p03001_19698", "slow_p10000_19812"]
    );
}

#[test]
fn stride_subsamples_without_changing_the_run_set() {
    let (out, r) = build("site", &["--stride", "7"], "site-stride7");
    assert!(r.stdout.contains("(5 runs, 282 samples"), "{}", r.stdout);
    let s = tmsite::stats::analyse(&std::fs::read_to_string(out).unwrap()).unwrap();
    assert_eq!(s.runs, 5);
    assert_eq!(s.samples, 282);
}

#[test]
fn stats_measures_the_golden_site_page() {
    let g = fixture_str("testdata/golden/site_stride1.html");
    let r = tmsite(&["stats", "--html", &g]);
    r.ok("tmsite stats");
    let o = r.stdout;
    for want in [
        "[full variant]",
        "  bytes            62424",
        "  paths            5",
        "  samples          1966",
        "  x range          986.4 .. 1584.0",
        "  y range          10.0 .. 27.9",
        "  z range          702.7 .. 1335.7",
        "  speed range      0.8 .. 471.0 km/h (legend VMAX 500)",
        // seconds with a decimal, not 19538 .. 19812
        "  time range       19.538 .. 19.812 s",
        "  first/last run   p00001_19538 / slow_p10000_19812",
    ] {
        assert!(o.contains(want), "stats output is missing {:?}:\n{}", want, o);
    }
    assert!(!o.contains(" ms"), "a raw-millisecond time survived in stats:\n{}", o);
}

#[test]
fn stats_measures_the_golden_compact_page() {
    let g = fixture_str("testdata/golden/compact_stride3.html");
    let r = tmsite(&["stats", "--html", &g]);
    r.ok("tmsite stats");
    let o = r.stdout;
    for want in [
        "[compact variant]",
        "  paths            5",
        "  samples          658",
        "  time range       19.538 .. 19.812 s",
    ] {
        assert!(o.contains(want), "stats output is missing {:?}:\n{}", want, o);
    }
    // The packed page is lossy by construction: 1 dm of position, 2 km/h of
    // speed. Same runs, coarser numbers -- if these ever match the full page
    // exactly, the packing stopped packing.
    let s = tmsite::stats::analyse(&std::fs::read_to_string(fixture("testdata/golden/compact_stride3.html")).unwrap()).unwrap();
    let f = tmsite::stats::analyse(&std::fs::read_to_string(fixture("testdata/golden/site_stride1.html")).unwrap()).unwrap();
    assert_eq!(s.runs, f.runs);
    assert!((s.xmin - f.xmin).abs() < 0.2, "{} vs {}", s.xmin, f.xmin);
    assert!((s.vmax_data - f.vmax_data).abs() <= 2.0, "{} vs {}", s.vmax_data, f.vmax_data);
    assert_eq!(s.vmax_legend, f.vmax_legend);
}

#[test]
fn stats_rejects_a_payload_the_browser_would_reject() {
    // CPython's json.dumps writes bare NaN/Infinity; JSON.parse in the page
    // would throw on them, so the measuring tool must not accept them either.
    let good = std::fs::read_to_string(fixture("testdata/golden/site_stride1.html")).unwrap();
    let bad = good.replacen("[986.4,", "[NaN,", 1);
    assert_ne!(good, bad, "the substitution did not fire; test is not testing anything");
    let d = scratch("stats-nan");
    let p = d.join("nan.html");
    std::fs::write(&p, bad).unwrap();
    let r = tmsite(&["stats", "--html", p.to_str().unwrap()]);
    r.failed("stats on a NaN payload");
    assert!(r.stderr.contains("not valid JSON"), "{}", r.stderr);
}

#[test]
fn stats_needs_a_page_and_says_so() {
    tmsite(&["stats"]).failed("stats with no --html");
    let d = scratch("stats-empty");
    let p = d.join("empty.html");
    std::fs::write(&p, "<html>nothing here</html>").unwrap();
    let r = tmsite(&["stats", "--html", p.to_str().unwrap()]);
    r.failed("stats on a non-page");
    assert!(r.stderr.contains("not a tmsite page"), "{}", r.stderr);
}

#[test]
fn an_empty_directory_is_an_error_not_an_empty_page() {
    let d = scratch("site-empty-dir");
    let src = d.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let r = tmsite(&[
        "site",
        "--dir",
        src.to_str().unwrap(),
        "--out",
        d.join("x.html").to_str().unwrap(),
    ]);
    r.failed("site on an empty dir");
    assert!(r.stderr.contains("no paths in"), "{}", r.stderr);
}
