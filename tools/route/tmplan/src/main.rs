//! `tmplan` CLI.
//!
//!   tmplan plan MAP.Map.Gbx --gates gates.json [--out-dir DIR] [--top-k 3] [--beam 4000]
//!                            [--flight none|ballistic|drag] [--grid track|deco] [--time speed|cost] [--drop-penalty X] [--source NAME] [--matrix] [--quiet]
//!        the geometric planner: top-k `router-plan` routes (HYPOTHESES), the distance matrix on request
//!   tmplan legs MAP.Map.Gbx --gates gates.json [--flight ballistic] [--step 4] [--reach 40]
//!        the failed-map characterisation (R4): every leg of the best flight-allowed plan that the
//!        surface graph cannot connect, with the chord profile (what lies beneath, the gaps, the drop)

use std::collections::BTreeMap;
use std::path::Path;
use tmplan::estimator::{EdgeEstimator, EdgeKind, FlightModel, Geometric, StateBucket, TimeModel};
use tmplan::planner;
use tmplan::surface::{Nodes, SurfaceModel};
use tmroute::io;
use tmroute::types::TrackGeomExt;

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
    let leg_specials = surf.leg_specials(&nodes, &fields, &gates);
    if has(args, "--matrix") {
        print_matrix(&nodes, &d, "surface-graph COST (off-road 20x; inf = no path)");
        print_matrix(&nodes, &len, "surface-graph path LENGTH (m)");
    }
    let flight = flight_of(flag(args, "--flight"));
    let est = Geometric { time_model: tm(args), d: &d, len: &len, nodes: &nodes, flight, surface: Some(&surf), dirs: Some(&dirs), drop: Some(&drop), drop_penalty: flag(args, "--drop-penalty").and_then(|s| s.parse().ok()).unwrap_or(0.0), specials: Some((&leg_specials, &gates)) };
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
            // sidecar: the gameplay placements on each leg (transformation gates, boosters …) — proposed as
            // `Leg.specials` in tmstate; until then a file beside the route
            if !gates.specials.is_empty() {
                let mut rows: Vec<serde_json::Value> = Vec::new();
                for (li, w) in p.visit.windows(2).enumerate() {
                    for &si in &leg_specials[w[0]][w[1]] {
                        let s = &gates.specials[si];
                        rows.push(serde_json::json!({"leg": li, "kind": s.kind, "car": s.car, "model": s.model, "centre": s.centre, "half_width": s.half_width}));
                    }
                }
                let sf = f.with_extension("specials.json");
                io::write_atomic(&sf, serde_json::to_string_pretty(&rows).unwrap().as_bytes()).unwrap_or_else(|e| die(&e));
                if !rows.is_empty() {
                    println!("    specials on the route: {}", rows.iter().map(|r| format!("leg {} {}{}", r["leg"], r["kind"].as_str().unwrap_or(""), r["car"].as_str().map_or(String::new(), |c| format!("→{c}")))).collect::<Vec<_>>().join(", "));
                }
            }
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
    let leg_specials = surf.leg_specials(&nodes, &fields, &gates);
    print_matrix(&nodes, &d, "surface-graph COST (off-road 20x; inf = no path)");
    print_matrix(&nodes, &len, "surface-graph path LENGTH (m)");
    let flight = flight_of(flag(args, "--flight").or(Some("ballistic".into())));
    let est = Geometric { time_model: tm(args), d: &d, len: &len, nodes: &nodes, flight, surface: Some(&surf), dirs: Some(&dirs), drop: Some(&drop), drop_penalty: flag(args, "--drop-penalty").and_then(|s| s.parse().ok()).unwrap_or(0.0), specials: Some((&leg_specials, &gates)) };
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
        "classify" => cmd_classify(&args[1..]),
        "local" => cmd_local(&args[1..]),
        "families" => cmd_families(&args[1..]),
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
        let verdict = if !l.is_finite() { "MISSING-CONNECTION (no surface path)" } else if v_h > 150.0 { "MISSING-CONNECTION (graph detour)" } else if v_h > 130.0 { "SUSPECT (130-150 m/s: booster or detour)" } else { "surface" };
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

/// `tmplan classify ROUTE.json MAP.Map.Gbx --gates gates.json [--grid deco]`
/// Fill each leg's `connection` of a human route from the surface graph: Road when the graph path between
/// the two gates could be driven in the humans' best leg time (≤ 130 m/s), else Jump / Drop (Δy < −8 m) —
/// a connection the surface graph lacks (F8). Writes the route back in place.
fn cmd_classify(args: &[String]) {
    let route_path = args.iter().find(|a| a.ends_with(".json") && !a.ends_with("gates.json")).cloned().unwrap_or_else(|| die("classify ROUTE.json MAP.Map.Gbx --gates gates.json"));
    let mut route = io::read_route(Path::new(&route_path)).unwrap_or_else(|e| die(&e));
    let (_map, gates, surf, nodes, _d, len, _drop, _fields) = load(args);
    let node_of_wp = |wp: u32| -> Option<usize> {
        let g = gates.by_waypoint(wp)?.group;
        nodes.groups.iter().position(|x| *x == g)
    };
    let Some(legs) = route.legs.as_mut() else { die("route has no legs") };
    let mut prev = 0usize;
    let mut changed = 0;
    for l in legs.iter_mut() {
        let Some(to) = node_of_wp(l.map_waypoint) else { println!("  leg {}: waypoint {} not a planner node", l.gate_idx, l.map_waypoint); continue };
        let best_ms = match &l.evidence { tmroute::LegEvidence::Human { best_ms, .. } => *best_ms, tmroute::LegEvidence::Driven { ms, .. } => *ms, _ => -1 };
        let lg = len[prev][to];
        let (_, dy) = tmplan::estimator::chord(nodes.pos[prev], nodes.pos[to]);
        let v = if best_ms > 0 { 1000.0 * lg / best_ms as f32 } else { f32::NAN };
        let conn = if lg.is_finite() && (v.is_nan() || v <= 130.0) { tmroute::ConnectionClass::Road } else if dy < -8.0 { tmroute::ConnectionClass::Drop } else { tmroute::ConnectionClass::Jump };
        if l.connection != conn { changed += 1; }
        println!("  leg {} → wp {}: graph {} m, human best {}, implied {:.0} m/s, Δy {:+.0} → {:?}", l.gate_idx, l.map_waypoint, if lg.is_finite() { format!("{lg:.0}") } else { "none".into() }, io::secs(best_ms), v, dy, conn);
        l.connection = conn;
        prev = to;
    }
    io::write_route(Path::new(&route_path), &route).unwrap_or_else(|e| die(&e));
    println!("{}: {} leg connections changed, written", route_path, changed);
}

/// `tmplan local MAP.Map.Gbx --gates gates.json [--cache FILE] [--ghost G.Ghost.Gbx ...] [--ray x,y,z:dx,dy,dz ...]`
/// Build (or load) the map's `LocalScene` (mapgeom::local) and answer queries. With ghosts: the control —
/// for every telemetry sample, the nearest surface straight below the car: how far, what material, what
/// block family; a car sits 0.3–1.2 m over its road, so the P50/P90 of that distance and the share of
/// samples with a surface within 3 m say whether the scene is the world the car drove.
fn cmd_local(args: &[String]) {
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates gates.json required (for yoff)"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let t0 = std::time::Instant::now();
    let scene = match flag(args, "--cache").filter(|c| Path::new(c).exists()) {
        Some(c) => {
            let s = mapgeom::local::LocalScene::load(Path::new(&c)).unwrap_or_else(|e| die(&e));
            println!("loaded {} ({} triangles, {} placements) in {:.1} s", c, s.tri_count(), s.placements.len(), t0.elapsed().as_secs_f32());
            s
        }
        None => {
            let server = std::env::var("TM_SERVER").unwrap_or_else(|_| die("TM_SERVER"));
            let paths: Vec<String> = ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"].iter().map(|n| format!("{server}/Packs/{n}")).filter(|p| Path::new(p).exists()).collect();
            let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).unwrap_or_else(|e| die(&e));
            let m = tmmaps::map::MapFile::load(Path::new(&map));
            let opts = mapgeom::local::BuildOpts { with_deco: !has(args, "--no-deco"), with_baked: !has(args, "--no-baked"), cell: flag(args, "--cell").and_then(|s| s.parse().ok()).unwrap_or(4.0) };
            let s = mapgeom::local::LocalScene::build(&mut store, &m, gates.yoff, &opts);
            println!("built {} triangles, {} placements, grid {}x{}x{} @ {} m in {:.1} s", s.tri_count(), s.placements.len(), s.dims[0], s.dims[1], s.dims[2], s.cell, t0.elapsed().as_secs_f32());
            if let Some(c) = flag(args, "--cache") {
                let t1 = std::time::Instant::now();
                s.save(Path::new(&c)).unwrap_or_else(|e| die(&e.to_string()));
                println!("cached → {c} ({:.1} MB, {:.1} s)", std::fs::metadata(&c).map(|m| m.len() as f64 / 1e6).unwrap_or(0.0), t1.elapsed().as_secs_f32());
            }
            s
        }
    };
    // specials census
    let mut specials: BTreeMap<String, usize> = BTreeMap::new();
    for p in &scene.placements {
        if p.special != mapgeom::local::Special::None {
            *specials.entry(format!("{:?} ({})", p.special, p.name)).or_default() += 1;
        }
    }
    if !specials.is_empty() {
        println!("  gameplay placements: {}", specials.iter().map(|(k, v)| format!("{k}×{v}")).collect::<Vec<_>>().join(", "));
    }
    // nearest items
    if let Some(spec) = flag(args, "--near") {
        let v: Vec<f32> = spec.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() != 3 { die("--near x,y,z") }
        let t1 = std::time::Instant::now();
        let idx = mapgeom::local::ItemIndex::build(&scene, 40.0);
        println!("  item index: {} items in {:.2} s", idx.bounds.len(), t1.elapsed().as_secs_f32());
        for n in idx.nearest(&scene, [v[0], v[1], v[2]], 60.0, 8) {
            let p = &scene.placements[n.placement as usize];
            println!("    {:>6.1} m  rel ({:+.1}, {:+.1}, {:+.1})  r {:.1}  {} [{}] {:?} {:?}", n.dist, n.rel[0], n.rel[1], n.rel[2], n.radius, p.name, n.family_id, n.kind, n.special);
        }
    }
    // rays
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--ray" {
            let spec = args.get(i + 1).cloned().unwrap_or_default();
            let (o, d) = spec.split_once(':').unwrap_or_else(|| die("--ray x,y,z:dx,dy,dz"));
            let p3 = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').filter_map(|x| x.parse().ok()).collect(); if v.len() != 3 { die("--ray x,y,z:dx,dy,dz") } [v[0], v[1], v[2]] };
            match scene.raycast(p3(o), p3(d), 500.0, true) {
                Some(h) => println!("  ray {spec}: hit at {:.2} m ({:.1}, {:.1}, {:.1}) normal ({:.2}, {:.2}, {:.2}) {} {} {:?} {:?}{}", h.dist, h.point[0], h.point[1], h.point[2], h.normal[0], h.normal[1], h.normal[2], h.material_name, h.family, h.kind, h.special, if h.collidable { "" } else { " NOT-COLLIDABLE" }),
                None => println!("  ray {spec}: nothing within 500 m"),
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    // ghost control
    let ghosts: Vec<String> = args.iter().filter(|a| a.ends_with(".Ghost.Gbx")).cloned().collect();
    if !ghosts.is_empty() {
        let mut dists: Vec<f32> = Vec::new();
        let mut n = 0usize;
        let mut within3 = 0usize;
        let mut mats: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut fams: BTreeMap<String, usize> = BTreeMap::new();
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        let mut t_ray = std::time::Duration::ZERO;
        for g in &ghosts {
            let Ok(run) = tmroute::human::load_run(Path::new(g)) else { continue };
            for s in run.samples.iter().filter(|s| s.t_ms >= 0) {
                n += 1;
                let t1 = std::time::Instant::now();
                let (below, _) = scene.layers_below_above(s.pos, 60.0, true);
                t_ray += t1.elapsed();
                match below {
                    Some(h) => {
                        dists.push(h.dist);
                        if h.dist <= 3.0 { within3 += 1; }
                        *mats.entry(h.material_name).or_default() += 1;
                        *fams.entry(h.family.clone()).or_default() += 1;
                        *kinds.entry(format!("{:?}", h.kind)).or_default() += 1;
                    }
                    None => dists.push(f32::INFINITY),
                }
            }
        }
        let mut finite: Vec<f32> = dists.iter().copied().filter(|d| d.is_finite()).collect();
        let none = dists.len() - finite.len();
        println!(
            "  ghost control: {} samples from {} ghosts; surface below within 3 m: {:.1} %; none within 60 m: {} ({:.1} %); dist P50 {:.2} m P90 {:.2} m; {:.1} µs per layers query",
            n, ghosts.len(), 100.0 * within3 as f32 / n.max(1) as f32, none, 100.0 * none as f32 / n.max(1) as f32,
            tmroute::human::percentile(&mut finite.clone(), 50.0), tmroute::human::percentile(&mut finite, 90.0),
            t_ray.as_secs_f64() * 1e6 / (n.max(1) as f64)
        );
        let top = |m: &BTreeMap<_, usize>| -> String { let mut v: Vec<(String, usize)> = m.iter().map(|(k, v)| (format!("{k}"), *v)).collect(); v.sort_by(|a, b| b.1.cmp(&a.1)); v.iter().take(8).map(|(k, v)| format!("{k} {:.0}%", 100.0 * *v as f32 / n.max(1) as f32)).collect::<Vec<_>>().join(", ") };
        println!("  materials under the car: {}", top(&mats.iter().map(|(k, v)| (k.to_string(), *v)).collect()));
        println!("  block families under the car: {}", top(&fams));
        println!("  placement kinds under the car: {}", top(&kinds));
    }
}

/// `tmplan families MAP.Map.Gbx ... --gates-dir GEOM_DIR` — census of placement families over maps
/// (for the stable family id table in mapgeom::local).
fn cmd_families(args: &[String]) {
    let gdir = flag(args, "--gates-dir").unwrap_or_else(|| die("--gates-dir GEOM_DIR"));
    let server = std::env::var("TM_SERVER").unwrap_or_else(|_| die("TM_SERVER"));
    let paths: Vec<String> = ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"].iter().map(|n| format!("{server}/Packs/{n}")).filter(|p| Path::new(p).exists()).collect();
    let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).unwrap_or_else(|e| die(&e));
    let mut census: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // family → (placements, triangles)
    for map in args.iter().filter(|a| a.ends_with(".Map.Gbx")) {
        let uid = Path::new(map).file_name().unwrap().to_string_lossy().replace(".Map.Gbx", "");
        let yoff = io::read_gates(&Path::new(&gdir).join(&uid).join("gates.json")).map(|g| g.yoff).unwrap_or(-40.0);
        let m = tmmaps::map::MapFile::load(Path::new(map));
        let s = mapgeom::local::LocalScene::build(&mut store, &m, yoff, &mapgeom::local::BuildOpts::default());
        let mut tri_per: Vec<usize> = vec![0; s.placements.len()];
        for t in &s.tris { tri_per[t.tag as usize] += 1; }
        for (i, p) in s.placements.iter().enumerate() {
            let e = census.entry(p.family.clone()).or_default();
            e.0 += 1;
            e.1 += tri_per[i];
        }
        eprintln!("{uid}: {} placements", s.placements.len());
    }
    let mut v: Vec<(String, usize, usize)> = census.into_iter().map(|(k, (a, b))| (k, a, b)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("family\tplacements\ttriangles\tid");
    for (f, a, b) in &v {
        println!("{f}\t{a}\t{b}\t{}", mapgeom::local::family_id(f));
    }
}
