//! `tmplan` CLI.
//!
//!   tmplan plan MAP.Map.Gbx --gates gates.json [--out-dir DIR] [--top-k 3] [--beam 4000]
//!                            [--flight none|ballistic|drag] [--grid track|deco] [--time speed|cost] [--drop-penalty X] [--source NAME] [--matrix] [--quiet]
//!        the geometric planner: top-k `router-plan` routes (HYPOTHESES), the distance matrix on request
//!   tmplan legs MAP.Map.Gbx --gates gates.json [--flight ballistic] [--step 4] [--reach 40]
//!        the failed-map characterisation (R4): every leg of the best flight-allowed plan that the
//!        surface graph cannot connect, with the chord profile (what lies beneath, the gaps, the drop)

use std::path::Path;
use tmplan::estimator::{EdgeEstimator, EdgeKind, FlightModel, Geometric, StateBucket, TimeModel};
use tmplan::planner;
use tmplan::surface::{Nodes, SurfaceModel};
use tmroute::io;

fn die(msg: &str) -> ! {
    eprintln!("tmplan: {msg}");
    std::process::exit(2)
}
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}
fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn tm(args: &[String]) -> TimeModel {
    match flag(args, "--time").as_deref() {
        None | Some("speed") => TimeModel::Speed,
        Some("cost") => TimeModel::Cost,
        Some(x) => die(&format!("--time {x}: speed|cost")),
    }
}

fn flight_of(s: Option<String>) -> Option<FlightModel> {
    match s.as_deref() {
        None | Some("none") => None,
        Some("ballistic") => Some(FlightModel::ballistic()),
        Some("drag") => Some(FlightModel::drag()),
        Some("any") => Some(FlightModel::any()),
        Some(x) => die(&format!("--flight {x}: none|ballistic|drag")),
    }
}

fn order_str(nodes: &Nodes, gates: &tmroute::gates::GatesFile, visit: &[usize]) -> (String, String) {
    let groups: Vec<String> = visit.iter().skip(1).map(|&n| nodes.groups[n].to_string()).collect();
    let wps: Vec<String> = visit.iter().skip(1).map(|&n| gates.group_rep(nodes.groups[n]).unwrap().waypoint.to_string()).collect();
    (groups.join(","), wps.join(","))
}

fn load(args: &[String]) -> (String, tmroute::gates::GatesFile, SurfaceModel, Nodes, Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Option<(Vec<f32>, Vec<u32>)>>) {
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates gates.json required"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let quiet = has(args, "--quiet");
    let deco = flag(args, "--grid").map_or(false, |g| g == "deco");
    let (surf, nodes) = SurfaceModel::build(Path::new(&map), &gates, !quiet, deco).unwrap_or_else(|e| die(&e));
    for n in &surf.notes {
        println!("  note: {n}");
    }
    let (d, len, drop, fields) = surf.distance_matrix_full(&nodes);
    (map, gates, surf, nodes, d, len, drop, fields)
}

fn print_matrix(nodes: &Nodes, d: &[Vec<f32>], title: &str) {
    println!("  {title}. 0 = spawn, then checkpoint groups, then finish groups:");
    print!("      {:>7}", "");
    for j in 0..nodes.pos.len() {
        print!("{:>8}", if j == 0 { "spawn".to_string() } else { format!("g{}", nodes.groups[j]) });
    }
    println!();
    for (i, row) in d.iter().enumerate() {
        let lbl = if i == 0 { "spawn".to_string() } else { format!("g{}", nodes.groups[i]) };
        print!("  {:>5} {:>7}", lbl, format!("{:?}", nodes.kinds[i]).chars().take(7).collect::<String>());
        for v in row {
            print!("{:>8}", if v.is_finite() { format!("{:.0}", v) } else { "inf".into() });
        }
        println!();
    }
}

fn cmd_plan(args: &[String]) {
    let (_map, gates, surf, nodes, d, len, drop, fields) = load(args);
    let dirs = surf.directions(&nodes, &fields);
    if has(args, "--matrix") {
        print_matrix(&nodes, &d, "surface-graph COST (off-road 20x; inf = no path)");
        print_matrix(&nodes, &len, "surface-graph path LENGTH (m)");
    }
    let flight = flight_of(flag(args, "--flight"));
    let est = Geometric { time_model: tm(args), d: &d, len: &len, nodes: &nodes, flight, surface: Some(&surf), dirs: Some(&dirs), drop: Some(&drop), drop_penalty: flag(args, "--drop-penalty").and_then(|s| s.parse().ok()).unwrap_or(0.0) };
    let width: usize = flag(args, "--beam").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let top_k: usize = flag(args, "--top-k").and_then(|s| s.parse().ok()).unwrap_or(3);
    human_legs(args, &nodes, &gates, &est, &len, Some(&surf), &fields);
    let plans = planner::beam(&nodes, &est, width, top_k, StateBucket::of_speed(0.0));
    if has(args, "--exact") {
        match planner::exact_cost(&nodes, &d) {
            Some((visit, cost)) => {
                let (g, w) = order_str(&nodes, &gates, &visit);
                let beam0 = plans.first().map(|p| p.visit.clone());
                println!("  exact Held–Karp on COST: cost {:.0}  groups [{}]  waypoints [{}]  beam rank 0 {}", cost, g, w, if beam0.as_ref() == Some(&visit) { "== exact" } else { "DIFFERS" });
            }
            None => println!("  exact Held–Karp: not run (> 16 checkpoints or no tour)"),
        }
    }
    let inf_pairs = d.iter().flatten().filter(|v| !v.is_finite()).count();
    println!(
        "{}\t{}\tcp_groups {}\tfinish_groups {}\testimator {}\tbeam {}\tinf_pairs {}/{}\tplans {}",
        gates.map_name, gates.map_uid, nodes.n_cp, nodes.n_fin, est.name(), width, inf_pairs, d.len() * d.len(), plans.len()
    );
    let prov = tmroute::provenance("tmplan plan");
    let out_dir = flag(args, "--out-dir");
    for (k, p) in plans.iter().enumerate() {
        let (g, w) = order_str(&nodes, &gates, &p.visit);
        let flights = p.edges.iter().filter(|e| e.kind == EdgeKind::Flight).count();
        let len: f32 = p.edges.iter().map(|e| e.length_m).sum();
        let drop: f32 = p.visit.windows(2).map(|w| surf.path_drop(&nodes, &fields, w[0], w[1])).filter(|d| d.is_finite()).sum();
        println!("  rank {k}: predicted {}  P(reach) {:.3}  length {:.0} m  drop {:.0} m  flight legs {}  groups [{}]  waypoints [{}]", io::secs(p.total_ms), p.p_reach, len, drop, flights, g, w);
        if let Some(dir) = &out_dir {
            let source = flag(args, "--source").unwrap_or_else(|| "router-plan".into());
            let mut route = tmplan::export::export(&gates, &nodes, &surf, &fields, p, k as u32, &est.name(), &prov);
            route.source = source.clone();
            if let Some(r) = route.route.as_mut() { r.source = source.clone(); }
            let errs = route.validate();
            if !errs.is_empty() {
                println!("    INVALID: {:?}", errs);
            }
            let f = Path::new(dir).join(&gates.map_uid).join(io::route_file_name(&source, k as u32));
            io::write_route(&f, &route).unwrap_or_else(|e| die(&e));
            println!("    wrote {}", f.display());
        }
    }
    if plans.is_empty() {
        println!("  NO PLAN: no complete tour under this estimator (surface graph{}). Not a verdict — the exploration budget is the GEN arm's.", if flight.is_some() { " + flight" } else { "" });
        std::process::exit(1);
    }
}

fn cmd_legs(args: &[String]) {
    let (_map, gates, surf, nodes, d, len, drop, fields) = load(args);
    let dirs = surf.directions(&nodes, &fields);
    print_matrix(&nodes, &d, "surface-graph COST (off-road 20x; inf = no path)");
    print_matrix(&nodes, &len, "surface-graph path LENGTH (m)");
    let flight = flight_of(flag(args, "--flight").or(Some("ballistic".into())));
    let est = Geometric { time_model: tm(args), d: &d, len: &len, nodes: &nodes, flight, surface: Some(&surf), dirs: Some(&dirs), drop: Some(&drop), drop_penalty: flag(args, "--drop-penalty").and_then(|s| s.parse().ok()).unwrap_or(0.0) };
    let mut plans = planner::beam(&nodes, &est, 4000, 1, StateBucket::of_speed(0.0));
    if let Some(o) = flag(args, "--order") {
        // characterise a GIVEN group order (e.g. the human modal order) instead of the best tour
        let mut visit = vec![0usize];
        for g in o.split(',') {
            let g: u32 = g.parse().unwrap_or_else(|_| die("--order g,g,g (group ids, finish last)"));
            visit.push(nodes.groups.iter().position(|x| *x == g).unwrap_or_else(|| die(&format!("group {g} is not a node"))));
        }
        let mut edges = Vec::new();
        let mut bucket = StateBucket::of_speed(0.0);
        for li in 0..visit.len() - 1 {
            let e = est.estimate(bucket, if li == 0 { None } else { Some(visit[li - 1]) }, visit[li], visit[li + 1]);
            bucket = e.arrival;
            edges.push(e);
        }
        let ms: i32 = edges.iter().map(|e| e.expected_ms.max(0)).sum();
        let logp: f32 = edges.iter().map(|e| e.p_reach.max(1e-6).ln()).sum();
        plans = vec![planner::Plan { visit, edges, total_ms: ms, p_reach: logp.exp(), score: ms as f32 }];
    }
    let step: f32 = flag(args, "--step").and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let reach: f32 = flag(args, "--reach").and_then(|s| s.parse().ok()).unwrap_or(40.0);
    let Some(p) = plans.first() else {
        println!("{}: no tour even with {}", gates.map_name, est.name());
        std::process::exit(1);
    };
    let (g, w) = order_str(&nodes, &gates, &p.visit);
    println!("{}\tbest tour under {}: groups [{}] waypoints [{}] predicted {} P(reach) {:.3}", gates.map_name, est.name(), g, w, io::secs(p.total_ms), p.p_reach);
    for (li, wv) in p.visit.windows(2).enumerate() {
        let (a, b) = (wv[0], wv[1]);
        let e = p.edges[li];
        let pa = nodes.pos[a];
        let pb = nodes.pos[b];
        let prof = surf.chord_profile(pa, pb, step, reach);
        let mats: Vec<String> = prof.materials().iter().map(|(m, f)| format!("{m} {:.0}%", f * 100.0)).collect();
        let tag = match e.kind { EdgeKind::Surface => "surface", EdgeKind::Flight => "NO-SURFACE-PATH", EdgeKind::None => "none" };
        println!(
            "  leg {li}: g{} → g{} [{}]  surface path {}  chord {:.1} m horiz, dy {:+.1} m  gaps {} (longest {:.1} m)  beneath the chord: {}",
            if a == 0 { "spawn".into() } else { nodes.groups[a].to_string() },
            nodes.groups[b],
            tag,
            if d[a][b].is_finite() { format!("{:.0} m", d[a][b]) } else { "NONE".into() },
            prof.horiz,
            prof.dy,
            prof.gaps.len(),
            prof.longest_gap(),
            mats.join(", ")
        );
        if e.kind == EdgeKind::Flight {
            println!("      from ({:.1}, {:.1}, {:.1}) to ({:.1}, {:.1}, {:.1})", pa[0], pa[1], pa[2], pb[0], pb[1], pb[2]);
            for (s, e) in &prof.gaps {
                println!("      gap {:.1}..{:.1} m along the chord", s, e);
            }
            // the profile, coarsely: every 4th row
            for r in prof.rows.iter().step_by(4) {
                match &r.surface {
                    Some((y, m)) => println!("      t {:.2} ({:.0},{:.0},{:.0}) surface y {:.1} ({:+.1} below chord) {}", r.t, r.pos[0], r.pos[1], r.pos[2], y, y - r.pos[1], m),
                    None => println!("      t {:.2} ({:.0},{:.0},{:.0}) NOTHING within {} m below", r.t, r.pos[0], r.pos[1], r.pos[2], reach),
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprintln!("usage: tmplan plan|legs MAP.Map.Gbx --gates gates.json ... (see src/main.rs)");
        std::process::exit(2);
    };
    match cmd.as_str() {
        "plan" => cmd_plan(&args[1..]),
        "legs" => cmd_legs(&args[1..]),
        other => die(&format!("unknown command {other}")),
    }
}

/// `--human-orders FILE`: evaluate the human modal order through the estimator
/// and print per-leg length / predicted / human-best — the speed model's
/// calibration data. Rows with unmatched crossings are skipped.
fn human_legs(args: &[String], nodes: &Nodes, gates: &tmroute::gates::GatesFile, est: &dyn EdgeEstimator, len: &[Vec<f32>], surf: Option<&SurfaceModel>, fields: &[Option<(Vec<f32>, Vec<u32>)>]) {
    let Some(f) = flag(args, "--human-orders") else { return };
    let Ok(txt) = std::fs::read_to_string(&f) else { eprintln!("cannot read {f}"); return };
    let mut rows: Vec<(Vec<u32>, Vec<i32>, i32)> = Vec::new();
    for (i, line) in txt.lines().enumerate() {
        if i == 0 { continue; }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 8 || c[7] != "0" { continue; }
        let groups: Vec<u32> = c[6].split(',').filter_map(|x| x.parse().ok()).collect();
        let cp: Vec<i32> = c[5].split(',').filter_map(|x| x.parse().ok()).collect();
        let ms: i32 = c[2].parse().unwrap_or(-1);
        if groups.len() == cp.len() && ms > 0 { rows.push((groups, cp, ms)); }
    }
    if rows.is_empty() { println!("  human-orders: no usable row"); return; }
    let mut counts: std::collections::BTreeMap<Vec<u32>, usize> = Default::default();
    for r in &rows { *counts.entry(r.0.clone()).or_default() += 1; }
    let (modal, n) = counts.iter().max_by_key(|(_, v)| **v).map(|(k, v)| (k.clone(), *v)).unwrap();
    let node_of = |g: u32| nodes.groups.iter().position(|x| *x == g);
    let mut visit = vec![0usize];
    for g in &modal { match node_of(*g) { Some(k) => visit.push(k), None => { println!("  human-orders: group {g} not a planner node"); return; } } }
    let best_lap = rows.iter().filter(|r| r.0 == modal).map(|r| r.2).min().unwrap();
    println!("  human modal order (groups [{}], {n}/{} runs, best lap {}): per leg", modal.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","), rows.len(), io::secs(best_lap));
    println!("    {:>3} {:>8} {:>9} {:>7} {:>10} {:>10} {:>9} {:>9}", "leg", "to", "length_m", "drop_m", "pred_ms", "human_ms", "v_pred", "v_human");
    let mut bucket = StateBucket::of_speed(0.0);
    let mut tot_pred = 0i32;
    let mut tsv = vec!["map\tleg\tfrom_group\tto_group\tto_waypoint\tgraph_len_m\tdrop_m\tpred_ms\thuman_best_ms\tv_implied\tverdict".to_string()];
    for li in 0..modal.len() {
        let (a, b) = (visit[li], visit[li + 1]);
        let e = est.estimate(bucket, if li == 0 { None } else { Some(visit[li - 1]) }, a, b);
        let human_ms = rows.iter().filter(|r| r.0 == modal).map(|r| r.1[li] - if li == 0 { 0 } else { r.1[li - 1] }).min().unwrap();
        let l = len[a][b];
        let drop = surf.map_or(f32::NAN, |s| s.path_drop(nodes, fields, a, b));
        let v_h = 1000.0 * l / human_ms.max(1) as f32;
        // A human leg whose surface-graph path implies > 130 m/s, or has no path at all, was
        // NOT driven along the graph: the humans used a connection the surface reader lacks.
        let verdict = if !l.is_finite() { "MISSING-CONNECTION (no surface path)" } else if v_h > 130.0 { "MISSING-CONNECTION (graph detour)" } else { "surface" };
        println!("    {:>3} {:>8} {:>9.0} {:>7.1} {:>10} {:>10} {:>9.1} {:>9.1}  {}", li, format!("g{}", modal[li]), l, drop, e.expected_ms, human_ms, if e.expected_ms > 0 { 1000.0 * l / e.expected_ms as f32 } else { f32::NAN }, v_h, verdict);
        tsv.push(format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.1}\t{}\t{}\t{:.1}\t{}", gates.map_name, li, if a == 0 { "spawn".to_string() } else { nodes.groups[a].to_string() }, modal[li], gates.group_rep(modal[li]).map_or(u32::MAX, |g| g.waypoint), l, drop, e.expected_ms, human_ms, v_h, verdict));
        tot_pred += e.expected_ms.max(0);
        bucket = e.arrival;
    }
    println!("    lap: predicted {} vs human best {}", io::secs(tot_pred), io::secs(best_lap));
    if let Some(out) = flag(args, "--legs-out") {
        std::fs::write(&out, tsv.join("\n") + "\n").unwrap_or_else(|e| die(&e.to_string()));
    }
}
