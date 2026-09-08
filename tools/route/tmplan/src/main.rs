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
    planner::BEAM_BUDGET_S.store(flag(args, "--beam-budget-s").and_then(|s| s.parse().ok()).unwrap_or(600), std::sync::atomic::Ordering::Relaxed);
    tmplan::estimator::SPAWN_TURN_M.store(flag(args, "--spawn-turn").and_then(|s| s.parse::<f32>().ok()).unwrap_or(80.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
    human_legs(args, &nodes, &gates, &est, &len, Some(&surf), &fields);
    let plans = planner::beam_laps(&nodes, &nodes.kinds, gates.laps, &est, width, top_k, StateBucket::of_speed(0.0));
    if gates.laps > 1 {
        println!("  lap race: {} laps — checkpoint order planned once from the spawn to the lap line and repeated; last lap ends at {}", gates.laps, if nodes.kinds[nodes.finish_range()].iter().any(|k| *k == tmroute::gates::WpKind::Finish) { "the finish" } else { "the lap line" });
    }
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
    // --note: appended to produced_by (e.g. the tiny converter build + the map md5 and collhash it was planned on)
    let prov = match flag(args, "--note") { Some(n) => format!("{}; {n}", tmroute::provenance("tmplan plan")), None => tmroute::provenance("tmplan plan") };
    let out_dir = flag(args, "--out-dir");
    for (k, p) in plans.iter().enumerate() {
        let (g, w) = order_str(&nodes, &gates, &p.visit);
        let flights = p.edges.iter().filter(|e| e.kind == EdgeKind::Flight).count();
        let len: f32 = p.edges.iter().map(|e| e.length_m).sum();
        let drop: f32 = p.visit.windows(2).map(|w| surf.path_drop(&nodes, &fields, w[0], w[1])).filter(|d| d.is_finite()).sum();
        println!("  rank {k}: predicted {}  P(reach) {:.3}  length {:.0} m  drop {:.0} m  flight legs {}  groups [{}]  waypoints [{}]{}", io::secs(p.total_ms), p.p_reach, len, drop, flights, g, w, if p.capped { "  BEAM-CAPPED" } else { "" });
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
        plans = vec![planner::Plan { visit, edges, total_ms: ms, p_reach: logp.exp(), score: ms as f32, capped: false }];
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
        let tag = match e.kind { EdgeKind::Surface => "surface", EdgeKind::Flight => "NO-SURFACE-PATH", EdgeKind::Learned => "learned", EdgeKind::None => "none" };
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
        "deck-gates" => cmd_deck_gates(&args[1..]),
        "leg-scan" => cmd_leg_scan(&args[1..]),
        "road-centreline" => cmd_road_centreline(&args[1..]),
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
            let paths = tmplan::pak_paths().unwrap_or_else(|e| die(&e));
            let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).unwrap_or_else(|e| die(&e));
            let m = tmmaps::map::MapFile::load(Path::new(&map));
            let opts = mapgeom::local::BuildOpts { with_deco: !has(args, "--no-deco"), with_baked: !has(args, "--no-baked") && !tmroute::gates::is_tiny_map(&gates.map_uid, &gates.map_name), cell: flag(args, "--cell").and_then(|s| s.parse().ok()).unwrap_or(4.0) };
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
    // --materials: triangle count per (placement kind, material) — the reader check the converter asked for
    if has(args, "--materials") {
        // AREA-weighted, up-facing (n.y > 0.7): a road deck is 256 big Asphalt triangles, its lips 4 480 tiny Rubber
        // ones — counting triangles called the tiny roads Rubber (F20)
        let mut hist: BTreeMap<(String, &'static str), (usize, f64)> = BTreeMap::new();
        for t in &scene.tris {
            let e1 = [t.v[1][0] - t.v[0][0], t.v[1][1] - t.v[0][1], t.v[1][2] - t.v[0][2]];
            let e2 = [t.v[2][0] - t.v[0][0], t.v[2][1] - t.v[0][1], t.v[2][2] - t.v[0][2]];
            let nrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
            if l < 1e-6 || nrm[1].abs() / l < 0.7 { continue; }
            let k = format!("{:?}", scene.placements[t.tag as usize].kind);
            let e = hist.entry((k, mapgeom::scene::physics_name(t.mat))).or_insert((0, 0.0));
            e.0 += 1;
            e.1 += (l / 2.0) as f64;
        }
        let mut v: Vec<_> = hist.into_iter().collect();
        v.sort_by(|a, b| b.1 .1.partial_cmp(&a.1 .1).unwrap());
        println!("  up-facing surfaces (kind, physics → area m², triangles):");
        for ((k, m), (n, a)) in v.iter().take(30) {
            println!("    {k:<10} {m:<20} {a:>10.0} m²  {n:>8} tris");
        }
    }
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
    let paths = tmplan::pak_paths().unwrap_or_else(|e| die(&e));
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

/// `tmplan deck-gates MAP.Map.Gbx --gates in.json --out out.json`
/// Re-anchor every ITEM gate on the DECK of its own collision hull: the converter's block-derived items
/// (tiny campaign, AC…) anchor at the block ORIGIN corner with the drivable deck 0–6 m above and, for a
/// diagonal piece, 30 m away in XZ — a ray down from the anchor misses it (coordinator, 22:08Z). For each
/// item gate: the smallest placement whose XZ box contains the anchor (or the nearest small one within 40 m),
/// its upward-facing triangles (n.y > 0.7), area-weighted centroid → the gate's new XZ, top → its road y.
fn cmd_deck_gates(args: &[String]) {
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates in.json"));
    let out = flag(args, "--out").unwrap_or_else(|| die("--out out.json"));
    let mut gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let paths = tmplan::pak_paths().unwrap_or_else(|e| die(&e));
    let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).unwrap_or_else(|e| die(&e));
    let m = tmmaps::map::MapFile::load(Path::new(&map));
    let scene = mapgeom::local::LocalScene::build(&mut store, &m, gates.yoff, &mapgeom::local::BuildOpts { with_deco: false, with_baked: false, cell: 4.0 });
    // per placement: XZ bounds, and the upward-facing deck (area-weighted centroid, top y)
    let n = scene.placements.len();
    let mut lo = vec![[f32::INFINITY; 3]; n];
    let mut hi = vec![[f32::NEG_INFINITY; 3]; n];
    let mut deck: Vec<(f64, [f64; 3], f32)> = vec![(0.0, [0.0; 3], f32::NEG_INFINITY); n]; // (area, Σ area·centroid, top y)
    for t in &scene.tris {
        let k = t.tag as usize;
        for v in &t.v { for a in 0..3 { lo[k][a] = lo[k][a].min(v[a]); hi[k][a] = hi[k][a].max(v[a]); } }
        let e1 = [t.v[1][0] - t.v[0][0], t.v[1][1] - t.v[0][1], t.v[1][2] - t.v[0][2]];
        let e2 = [t.v[2][0] - t.v[0][0], t.v[2][1] - t.v[0][1], t.v[2][2] - t.v[0][2]];
        let nrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if l < 1e-6 { continue; }
        let ny = nrm[1].abs() / l; // either winding
        if ny > 0.7 {
            let area = (l / 2.0) as f64;
            let c = [(t.v[0][0] + t.v[1][0] + t.v[2][0]) / 3.0, (t.v[0][1] + t.v[1][1] + t.v[2][1]) / 3.0, (t.v[0][2] + t.v[1][2] + t.v[2][2]) / 3.0];
            let d = &mut deck[k];
            d.0 += area;
            for a in 0..3 { d.1[a] += area * c[a] as f64; }
            d.2 = d.2.max(c[1]);
        }
    }
    let mut moved = 0;
    let mut lines = Vec::new();
    for g in gates.gates.iter_mut() {
        if !g.from_item { continue; }
        let anchor = [g.centre[0], g.centre[1] - g.half_height, g.centre[2]];
        // the gate's OWN placement: same model name, nearest box to the anchor (a diagonal piece's origin corner
        // lies outside its own deck, so containment is not the test)
        let dist_box = |k: usize| -> f32 { let mut d2 = 0.0f32; for a in [0usize, 2] { let v = anchor[a]; let c = v.clamp(lo[k][a], hi[k][a]); d2 += (v - c) * (v - c); } let vy = anchor[1]; let cy = vy.clamp(lo[k][1], hi[k][1]); d2 += (vy - cy) * (vy - cy); d2.sqrt() };
        let mut cands: Vec<(f32, usize)> = (0..n)
            .filter(|&k| scene.placements[k].kind == mapgeom::local::PlacementKind::Item && deck[k].0 > 1.0 && scene.placements[k].name == g.model)
            .map(|k| (dist_box(k), k))
            .collect();
        cands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let Some(&(dd, k)) = cands.first().filter(|c| c.0 < 40.0) else { lines.push(format!("  wp {:>2} {}: no placement of that model within 40 m of the anchor — left as is", g.waypoint, g.model)); continue };
        let _ = dd;
        let d = &deck[k];
        let c = [(d.1[0] / d.0) as f32, (d.1[1] / d.0) as f32, (d.1[2] / d.0) as f32];
        let top = d.2;
        let old = g.centre;
        // the deck's mean height (a sloped piece) rather than its highest face
        g.centre = [c[0], c[1] + g.half_height, c[2]];
        let _ = top;
        // where the engine credits it, in THIS frame (deck centroid): measured on tiny 02/06 crossings (INPUT arm)
        let mb = g.model.as_bytes();
        if mb.len() > 2 && mb[0] == b'A' && mb[2].is_ascii_digit() {
            // MEASURED on ship8 (real disc triggers; INPUT arm crossings 2026-09-08 12:18Z, 39 gates on 7 maps), deck frame:
            // AC checkpoints −2.1 (n 26, sd 0.36; one slope checkpoint at −3.8), AC finishes −6.5 (n 6, sd 0.27), AI gate
            // items −2.1 (n 7, sd 0.19). The pre-3f5da2a fit (−9.5 / −7.6 / −2.07) measured whole-block triggers.
            g.credit_offset_m = match (mb[1], g.kind) { (b'C', tmroute::gates::WpKind::Checkpoint) => -2.1, (b'C', tmroute::gates::WpKind::Finish) => -6.5, (b'I', _) => -2.1, _ => g.credit_offset_m };
        }
        moved += 1;
        lines.push(format!("  wp {:>2} {}: anchor ({:.1}, {:.1}, {:.1}) → deck centre ({:.1}, {:.1}, {:.1}) top {:.1} (hull {} tris, deck area {:.0} m²)", g.waypoint, g.model, old[0], old[1] - g.half_height, old[2], c[0], c[1], c[2], top, (0..scene.tris.len()).filter(|&i| scene.tris[i].tag as usize == k).count(), d.0));
    }
    // spawn too
    {
        let s = gates.spawn.pos;
        let mut cands: Vec<(f32, usize)> = (0..n).filter(|&k| scene.placements[k].kind == mapgeom::local::PlacementKind::Item && deck[k].0 > 1.0).filter(|&k| s[0] >= lo[k][0] - 2.0 && s[0] <= hi[k][0] + 2.0 && s[2] >= lo[k][2] - 2.0 && s[2] <= hi[k][2] + 2.0 && s[1] >= lo[k][1] - 4.0 && s[1] <= hi[k][1] + 4.0).map(|k| ((hi[k][0] - lo[k][0]) * (hi[k][2] - lo[k][2]), k)).collect();
        cands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if let Some(&(_, k)) = cands.first() {
            let d = &deck[k];
            let c = [(d.1[0] / d.0) as f32, (d.1[1] / d.0) as f32, (d.1[2] / d.0) as f32];
            lines.push(format!("  spawn ({:.1}, {:.1}, {:.1}) → start deck centroid ({:.1}, {:.1}, {:.1})", s[0], s[1], s[2], c[0], c[1], c[2]));
            // the start item anchors at a corner of its deck: the car spawns on the deck, so does the polyline
            gates.spawn.pos = [c[0], c[1] + 0.5, c[2]];
        }
    }
    gates.produced_by = format!("{}; deck-gates: {moved} item gates re-anchored on their own hull deck ({})", gates.produced_by, tmroute::provenance("tmplan deck-gates"));
    for l in &lines { println!("{l}"); }
    io::write_gates(Path::new(&out), &gates).unwrap_or_else(|e| die(&e));
    println!("wrote {out} ({moved} gates moved)");
}

/// `tmplan leg-scan MAP.Map.Gbx --gates gates.json --ghosts DIR [--legs 3-5,5-0]`
/// What the HUMANS do on each consecutive leg of their order (median over ghosts): chord, height delta, time,
/// speeds, airborne share (no surface within 1.5 m below the car), largest fall, wallride share (surface below with
/// |n.y| < 0.5), specials (booster/reactor/turbo…) within 8 m of the trajectory — beside the surface graph's
/// verdict on the leg (path length or MISSING). For the M2 failure list (coordinator, 2026-09-07 23:33Z).
fn cmd_leg_scan(args: &[String]) {
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates gates.json"));
    let gdir = flag(args, "--ghosts").unwrap_or_else(|| die("--ghosts DIR"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let (surf, nodes) = SurfaceModel::build(Path::new(&map), &gates, false, false).unwrap_or_else(|e| die(&e));
    let (d, len, _fields) = surf.distance_matrix(&nodes);
    let node_of_group: BTreeMap<u32, usize> = nodes.groups.iter().enumerate().map(|(i, g)| (*g, i)).collect();
    let paths = tmplan::pak_paths().unwrap_or_else(|e| die(&e));
    let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).unwrap_or_else(|e| die(&e));
    let m = tmmaps::map::MapFile::load(Path::new(&map));
    let scene = mapgeom::local::LocalScene::build(&mut store, &m, gates.yoff, &mapgeom::local::BuildOpts { with_deco: true, with_baked: !tmroute::gates::is_tiny_map(&gates.map_uid, &gates.map_name), cell: 4.0 });
    let specials = tmroute::gates::specials(&m, gates.yoff);
    // ghosts
    let mut runs = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&gdir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map_or(false, |x| x == "Gbx") && p.to_string_lossy().ends_with(".Ghost.Gbx") {
                if let Ok(r) = tmroute::human::load_run(&p) { runs.push(r); }
            }
        }
    }
    if runs.is_empty() { die("no ghosts"); }
    let only: Option<Vec<(u32, u32)>> = flag(args, "--legs").map(|s| s.split(',').filter_map(|l| { let mut it = l.split('-'); Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?)) }).collect());
    // per leg (from group, to group) → per-ghost measurements
    #[derive(Default)]
    struct Acc { chord: Vec<f32>, dy: Vec<f32>, ms: Vec<f32>, vmax: Vec<f32>, vmean: Vec<f32>, air: Vec<f32>, fall: Vec<f32>, wall: Vec<f32>, specials: BTreeMap<String, usize>, path: Vec<f32> }
    let mut legs: BTreeMap<(u32, u32), Acc> = BTreeMap::new();
    for run in &runs {
        let cr: Vec<tmroute::human::Crossing> = tmroute::human::crossings(run, &gates, 40.0).into_iter().flatten().collect();
        // spawn first
        let mut seq: Vec<(u32, i32, [f32; 3])> = vec![(u32::MAX, 0, gates.spawn.pos)];
        for c in &cr { seq.push((c.group, c.ms, c.pos)); }
        for w in seq.windows(2) {
            let (ga, ta, pa) = w[0];
            let (gb, tb, pb) = w[1];
            if let Some(o) = &only { if !o.contains(&(ga, gb)) { continue; } }
            if tb <= ta { continue; }
            let a = legs.entry((ga, gb)).or_default();
            let ch = ((pb[0] - pa[0]).powi(2) + (pb[1] - pa[1]).powi(2) + (pb[2] - pa[2]).powi(2)).sqrt();
            a.chord.push(ch);
            a.dy.push(pb[1] - pa[1]);
            a.ms.push((tb - ta) as f32);
            let seg: Vec<&tmroute::human::Sample> = run.samples.iter().filter(|s| s.t_ms >= ta && s.t_ms <= tb).collect();
            if seg.is_empty() { continue; }
            let mut vmax = 0.0f32; let mut vsum = 0.0f32; let mut air = 0usize; let mut wall = 0usize; let mut fall = 0.0f32; let mut ymax = f32::NEG_INFINITY; let mut plen = 0.0f32;
            for (i, s) in seg.iter().enumerate() {
                vmax = vmax.max(s.speed); vsum += s.speed;
                if i > 0 { let q = seg[i - 1].pos; plen += ((s.pos[0] - q[0]).powi(2) + (s.pos[1] - q[1]).powi(2) + (s.pos[2] - q[2]).powi(2)).sqrt(); }
                ymax = ymax.max(s.pos[1]);
                fall = fall.max(ymax - s.pos[1]);
                let (below, _) = scene.layers_below_above(s.pos, 60.0, true);
                match below {
                    Some(h) if h.dist <= 1.5 => { if h.normal[1].abs() < 0.5 { wall += 1; } }
                    _ => air += 1,
                }
                for sp in &specials {
                    let dd = ((s.pos[0] - sp.centre[0]).powi(2) + (s.pos[1] - sp.centre[1]).powi(2) + (s.pos[2] - sp.centre[2]).powi(2)).sqrt();
                    if dd < 8.0 { *a.specials.entry(sp.kind.clone()).or_default() += 1; }
                }
            }
            a.vmax.push(vmax); a.vmean.push(vsum / seg.len() as f32); a.air.push(air as f32 / seg.len() as f32); a.wall.push(wall as f32 / seg.len() as f32); a.fall.push(fall); a.path.push(plen);
        }
    }
    let med = |v: &Vec<f32>| -> f32 { if v.is_empty() { f32::NAN } else { let mut w = v.clone(); tmroute::human::percentile(&mut w, 50.0) } };
    println!("{}\t{} ghosts\tleg\tn\tchord_m\tdy_m\thuman_s\tv_mean\tv_max\thuman_path_m\tair_share\tmax_fall_m\twall_share\tgraph_path_m\tgraph_cost\tspecials", gates.map_name, runs.len());
    for ((ga, gb), a) in &legs {
        let lab = |g: u32| if g == u32::MAX { "spawn".to_string() } else { format!("g{g}") };
        let (gl, gc) = match (if *ga == u32::MAX { Some(&0usize) } else { node_of_group.get(ga) }, node_of_group.get(gb)) {
            (Some(&i), Some(&j)) => (len[i][j], d[i][j]),
            _ => (f32::NAN, f32::NAN),
        };
        let sp: Vec<String> = a.specials.iter().map(|(k, n)| format!("{k}×{}", (*n as f32 / a.ms.len().max(1) as f32).round() as usize)).collect();
        println!("\t\t{}→{}\t{}\t{:.0}\t{:+.0}\t{:.3}\t{:.0}\t{:.0}\t{:.0}\t{:.2}\t{:.1}\t{:.2}\t{}\t{}\t{}", lab(*ga), lab(*gb), a.ms.len(), med(&a.chord), med(&a.dy), med(&a.ms) / 1000.0, med(&a.vmean) * 3.6, med(&a.vmax) * 3.6, med(&a.path), med(&a.air), med(&a.fall), med(&a.wall), if gl.is_finite() { format!("{gl:.0}") } else { "MISSING".into() }, if gc.is_finite() { format!("{gc:.0}") } else { "∞".into() }, if sp.is_empty() { "-".into() } else { sp.join(" ") });
    }
}

/// `tmplan road-centreline MAP.Map.Gbx --gates deck.json --order 3,1,2,0,4 --out road-centreline.json [--note ...]`
/// A ROAD-FOLLOWING centreline through the gate decks in the given group order (the route's): per leg the road-only
/// shortest path (no off-road cell, no leap), re-centred to the road midline and smoothed, half-width from the
/// lateral road span; where no road path exists the polyline STAYS on the last deck and the segment is a `gap`
/// (never a straight line off the road — Argentina 21, the car fell at s ≈ 60 m). The player's ask, 2026-09-08.
fn cmd_road_centreline(args: &[String]) {
    // platform decks of any physics are road for a centreline that must follow the decks (Summer 2026 - 10: Grass/Concrete
    // decks; Saudi Arabia 2026: Sand) — opt-in here so the planner's exhibit road set is untouched
    if std::env::var("TMPLAN_DECK_PHYSICS").is_err() { std::env::set_var("TMPLAN_DECK_PHYSICS", "Concrete,Grass,Sand"); }
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let mut gates = io::read_gates(Path::new(&flag(args, "--gates").unwrap_or_else(|| die("--gates")))).unwrap_or_else(|e| die(&e));
    // --spawn x,y,z: the ENGINE's tick-0 pose (what the env resets to) instead of the start deck centroid — the INPUT arm
    // measures it per build (tm-player/tiny/gate-crossings/ENGINE-SPAWNS-<build>.tsv); 14's pitched free-placed start
    // item put the centroid 4 m off (12:25Z)
    let mut spawn_note = String::new();
    if let Some(sp) = flag(args, "--spawn") {
        let v: Vec<f32> = sp.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() == 3 {
            spawn_note = format!("; spawn = engine tick-0 pose ({:.1}, {:.1}, {:.1}) (was deck centroid ({:.1}, {:.1}, {:.1}))", v[0], v[1], v[2], gates.spawn.pos[0], gates.spawn.pos[1], gates.spawn.pos[2]);
            gates.spawn.pos = [v[0], v[1], v[2]];
        }
    }
    let out = flag(args, "--out").unwrap_or_else(|| die("--out"));
    let order: Vec<u32> = flag(args, "--order").unwrap_or_else(|| die("--order g,g,g")).split(',').filter_map(|x| x.trim().parse().ok()).collect();
    let (mut surf, nodes) = SurfaceModel::build(Path::new(&map), &gates, false, false).unwrap_or_else(|e| die(&e));
    // --exclude x0,z0,x1,z1[;…] (or --exclusions FILE.tsv: map_stem\tx0\tz0\tx1\tz1\tnote, filtered by --map-stem): XZ boxes
    // whose surfaces are NOT road for this line — Argentina 2026's grandstand tiers (Metal, like the tech decks) stood
    // between the two roads and the line cut across them; the converter's census names such structures
    let mut boxes: Vec<[f32; 4]> = Vec::new();
    if let Some(e) = flag(args, "--exclude") { for b in e.split(';') { let v: Vec<f32> = b.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 4 { boxes.push([v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])]); } } }
    if let (Some(f), Some(stem)) = (flag(args, "--exclusions"), flag(args, "--map-stem")) {
        if let Ok(t) = std::fs::read_to_string(f) { for l in t.lines().filter(|l| !l.starts_with('#')) { let c: Vec<&str> = l.split('\t').collect(); if c.len() >= 5 && c[0] == stem { let v: Vec<f32> = c[1..5].iter().filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 4 { boxes.push([v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])]); } } } }
    }
    if !boxes.is_empty() {
        let mut n_ex = 0usize;
        for k in 0..surf.graph.len() {
            let w = surf.graph.world(&surf.grid, k);
            if boxes.iter().any(|b| w[0] >= b[0] && w[0] <= b[2] && w[2] >= b[1] && w[2] <= b[3]) && surf.graph.node_road[k] { surf.graph.node_road[k] = false; surf.graph.node_prime[k] = false; n_ex += 1; }
        }
        eprintln!("  exclusions: {} boxes, {n_ex} road nodes turned off", boxes.len());
    }
    // the SURFACE-FOLLOWING fallback for gap legs (wallrides, loops, inverted roads, the pool, terrain hills): a walk over
    // the collision triangles themselves (tmplan::walk), built once, lazily; --no-walk disables it. Legs whose verdict
    // is Jump/Drop never get one (air is not a surface).
    let walk_on = !has(args, "--no-walk");
    let mut walk: Option<tmplan::walk::SurfaceWalk> = None;
    let mut build_walk = || -> Option<tmplan::walk::SurfaceWalk> {
        let paths = tmplan::pak_paths().ok()?;
        let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).ok()?;
        let m = tmmaps::map::MapFile::load(Path::new(&map));
        let opts = mapgeom::local::BuildOpts { with_deco: false, with_baked: !tmroute::gates::is_tiny_map(&gates.map_uid, &gates.map_name), cell: 4.0 };
        let scene = mapgeom::local::LocalScene::build(&mut store, &m, gates.yoff, &opts);
        let t0 = std::time::Instant::now();
        let w = tmplan::walk::SurfaceWalk::build(&scene, 1.0, &|mat: u8| mat != 255);
        eprintln!("  surface walk: {} shell voxels of {} m from {} triangles, built in {:.1} s", w.key.len(), w.cell, scene.tris.len(), t0.elapsed().as_secs_f32());
        Some(w)
    };
    // verdict classes by (from label, to group) — a Jump/Drop leg is never walked
    let stem_v = flag(args, "--map-stem").unwrap_or_default();
    let jumpy: Vec<(String, String)> = flag(args, "--verdicts").and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().filter(|l| !l.starts_with('#')).filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); if f.len() >= 4 && f[0] == stem_v && (f[3] == "Jump" || f[3] == "Drop") { Some((f[1].to_string(), f[2].to_string())) } else { None } }).collect()).unwrap_or_default();
    let node_of_group: BTreeMap<u32, usize> = nodes.groups.iter().enumerate().map(|(i, g)| (*g, i)).collect();
    let mut seq: Vec<usize> = vec![0];
    for g in &order { seq.push(*node_of_group.get(g).unwrap_or_else(|| die(&format!("group {g} not in gates")))); }
    let mut pts: Vec<[f32; 3]> = vec![nodes.pos[0]];
    let mut hw: Vec<f32> = vec![surf.road_span(nodes.pos[0], [0.0, 1.0], 24.0).map_or(6.0, |(l, r)| ((l + r) / 2.0).max(3.0))];
    let mut segs: Vec<String> = Vec::new();
    let mut gaps = 0;
    // where the previous leg left the car: 6 m THROUGH the gate along its arrival direction when the road continues
    let mut through: Option<[f32; 3]> = None;
    // a straight STUB along the spawn heading first (12 m, while the road is there): the car starts at rest facing
    // spawn.yaw and an OffRoute check at t = 0 must not see the line leave at 110° (player, 14:36Z); the first leg
    // then starts from the stub's end. --spawn-yaw overrides the heading (radians, TM convention: yaw 0 = +x… as in
    // gates.json), --no-stub disables.
    if !has(args, "--no-stub") {
        let yaw = flag(args, "--spawn-yaw").and_then(|s| s.parse::<f32>().ok()).unwrap_or(gates.spawn.yaw);
        let d = [yaw.sin(), 0.0f32, yaw.cos()];
        let p0 = pts[0];
        let mut stub: Vec<[f32; 3]> = Vec::new();
        for k in 1..=6 {
            let q = [p0[0] + d[0] * 2.0 * k as f32, p0[1], p0[2] + d[2] * 2.0 * k as f32];
            match surf.graph.nearest_window(&surf.grid, q, 1, -8.0, 3.0) {
                Some(n) if surf.graph.node_road[n] => {
                    let w = surf.graph.world(&surf.grid, n);
                    stub.push([q[0], w[1], q[2]]);
                }
                _ => break,
            }
        }
        if stub.len() >= 3 {
            for q in &stub {
                hw.push(surf.road_span(*q, [d[0], d[2]], 24.0).map_or(4.0, |(l, r)| ((l + r) / 2.0).max(3.0)));
                pts.push(*q);
            }
            through = Some(*stub.last().unwrap());
            eprintln!("  spawn stub: {} m along yaw {:.2} ({:.2}, {:.2}) from ({:.1}, {:.1}, {:.1})", 2 * stub.len(), yaw, d[0], d[2], p0[0], p0[1], p0[2]);
        } else {
            eprintln!("  spawn stub: no road along yaw {yaw:.2} from the spawn — none written");
        }
    }
    // --leg-lines FILE.tsv (map_stem \t from_group|spawn \t to_group \t half_width \t x,y,z;x,y,z;… \t note): an explicit
    // drive line for a leg the graph cannot carry (Argentina 21's turbo jump — player/LEARN 16:04Z); the polyline
    // takes these points (resampled 2 m), the segment is "via": "manual", the Leg keeps its verdict class
    let leg_lines: Vec<(String, String, f32, Vec<[f32; 3]>)> = flag(args, "--leg-lines").and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().filter(|l| !l.starts_with('#')).filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 5 && f[0] == stem_v {
            let pts: Vec<[f32; 3]> = f[4].split(';').filter_map(|p| { let v: Vec<f32> = p.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None } }).collect();
            if pts.len() >= 2 { Some((f[1].to_string(), f[2].to_string(), f[3].trim().parse().unwrap_or(8.0), pts)) } else { None }
        } else { None }
    }).collect()).unwrap_or_default();
    for w in seq.windows(2) {
        let (i, j) = (w[0], w[1]);
        let i0 = pts.len() - 1;
        if let Some((_, _, hwid, line)) = leg_lines.iter().find(|(f, t, _, _)| *f == grp(&nodes, i).trim_matches('"') && *t == grp_id_of(&nodes, j)) {
            let mut raw = vec![*pts.last().unwrap()];
            raw.extend_from_slice(line);
            let leg = tmroute::human::resample(&raw, 2.0);
            for p in leg.iter().skip(1) {
                hw.push(*hwid);
                pts.push(*p);
            }
            eprintln!("  manual drive line {} → {}: {} points given, {} pts, half-width {hwid}", grp(&nodes, i), grp(&nodes, j), line.len(), leg.len());
            segs.push(format!("{{\"from_group\": {}, \"to_group\": {}, \"i0\": {i0}, \"i1\": {}, \"gap\": false, \"via\": \"manual\"}}", grp(&nodes, i), grp(&nodes, j), pts.len() - 1));
            through = None;
            continue;
        }
        let path = match through { Some(p) => surf.road_path_from_point(p, &nodes, j).or_else(|| surf.road_path(&nodes, i, j)), None => surf.road_path(&nodes, i, j) };
        through = None;
        match path {
            Some(raw) => {
                let mut leg = tmroute::human::resample(&raw, 2.0);
                if leg.len() > 2 {
                    let orig = leg.clone();
                    for k in 1..orig.len() - 1 {
                        let d = [orig[k + 1][0] - orig[k - 1][0], orig[k + 1][2] - orig[k - 1][2]];
                        leg[k] = surf.recentre(orig[k], d, 24.0);
                    }
                    let c = leg.clone();
                    for k in 1..c.len() - 1 { for a in [0usize, 2] { leg[k][a] = 0.25 * c[k - 1][a] + 0.5 * c[k][a] + 0.25 * c[k + 1][a]; } }
                }
                for (k, p) in leg.iter().enumerate().skip(1) {
                    let d = [leg[k][0] - leg[k - 1][0], leg[k][2] - leg[k - 1][2]];
                    hw.push(surf.road_span(*p, d, 24.0).map_or(4.0, |(l, r)| ((l + r) / 2.0).max(3.0)));
                    pts.push(*p);
                }
                let gate_i = pts.len() - 1;
                // continue 2, 4, 6 m past the gate along the arrival direction while the road is there (the credit plane
                // lies 1–2 m past the deck centroid: the line must CROSS it before it turns)
                if leg.len() >= 3 && j != *seq.last().unwrap() {
                    let a = leg[leg.len() - 3]; let b = leg[leg.len() - 1];
                    let l = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt().max(1e-3);
                    let d = [(b[0] - a[0]) / l, (b[2] - a[2]) / l];
                    for k in [2.0f32, 4.0, 6.0] {
                        let q = [b[0] + d[0] * k, b[1], b[2] + d[1] * k];
                        let Some(n) = surf.graph.nearest_window(&surf.grid, q, 1, -3.0, 3.0) else { break };
                        if !surf.graph.node_road[n] { break; }
                        let qw = surf.graph.world(&surf.grid, n);
                        let qq = [q[0], qw[1], q[2]];
                        hw.push(surf.road_span(qq, d, 24.0).map_or(4.0, |(l2, r2)| ((l2 + r2) / 2.0).max(3.0)));
                        pts.push(qq);
                        through = Some(qq);
                    }
                }
                segs.push(format!("{{\"from_group\": {}, \"to_group\": {}, \"i0\": {i0}, \"i1\": {gate_i}, \"gap\": false}}", grp(&nodes, i), grp(&nodes, j)));
            }
            None => {
                // SURFACE WALK before declaring a gap: over the collision triangles, from around where the line stands
                // (the through-point or the from-gate) to the to-gate deck; accepted when ≤ 4 × chord + 40 m
                // a Jump/Drop verdict INTO this gate from any gate counts: an isolated deck (21's gate 15) is a jump from
                // wherever the order arrives (the order moved 2→15 to 6→15 and the walk built a fake path, 16:11Z)
                let walked = if walk_on && !jumpy.iter().any(|(f, t)| *t == grp_id_of(&nodes, j)) {
                    if walk.is_none() { walk = build_walk(); }
                    walk.as_ref().and_then(|w| {
                        let start = *pts.last().unwrap();
                        let goal = nodes.pos[j];
                        let chord = ((goal[0] - start[0]).powi(2) + (goal[1] - start[1]).powi(2) + (goal[2] - start[2]).powi(2)).sqrt();
                        let from_set = w.near(start, 8.0, 4.0);
                        let to_set = w.near(goal, 8.0, 4.0);
                        w.walk(&from_set, &to_set, 4.0 * chord + 40.0).map(|(p, l)| (p, l, chord))
                    })
                } else { None };
                if let Some((raw, wlen, chord)) = walked {
                    let leg = tmroute::human::resample(&raw, 2.0);
                    eprintln!("  surface walk {} → {}: {:.0} m over the triangles (chord {:.0} m), {} pts", grp(&nodes, i), grp(&nodes, j), wlen, chord, leg.len());
                    for p in leg.iter().skip(1) {
                        hw.push(3.0);
                        pts.push(*p);
                    }
                    segs.push(format!("{{\"from_group\": {}, \"to_group\": {}, \"i0\": {i0}, \"i1\": {}, \"gap\": false, \"via\": \"surface\"}}", grp(&nodes, i), grp(&nodes, j), pts.len() - 1));
                    continue;
                }
                // stay on the last deck: the polyline does not move; the segment is a gap the consumer must bridge
                gaps += 1;
                let road_at = |k: usize| nodes.graph_node[k].map_or("no graph node".to_string(), |n| if surf.graph.node_road[n] { "road".into() } else { "off-road".into() });
                let any = surf.path_points(&nodes, &surf.distance_matrix(&nodes).2, i, j).map_or("none".to_string(), |p| format!("{} pts", p.len()));
                eprintln!("  gap {} → {}: from node {}, to node {}, full-graph path {any}", grp(&nodes, i), grp(&nodes, j), road_at(i), road_at(j));
                // what the full-graph path crosses: consecutive off-road runs > 4 m with the surface under them
                if let Some(fp) = surf.path_points(&nodes, &surf.distance_matrix(&nodes).2, i, j) {
                    let mut cur = 0.0f32;
                    let mut mats: BTreeMap<String, usize> = BTreeMap::new();
                    let mut run_start = [0.0f32; 3];
                    let flush = |cur: f32, mats: &BTreeMap<String, usize>, at: [f32; 3]| {
                        if cur > 4.0 {
                            let m: Vec<String> = mats.iter().map(|(k, v)| format!("{k}×{v}")).collect();
                            eprintln!("      off-road run {cur:.0} m over {} from ({:.0}, {:.1}, {:.0})", m.join(" "), at[0], at[1], at[2]);
                        }
                    };
                    for w in fp.windows(2) {
                        let (a, b) = (w[0], w[1]);
                        let step = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
                        let on = surf.road_span(b, [b[0] - a[0], b[2] - a[2]], 2.0).is_some();
                        if on {
                            flush(cur, &mats, run_start);
                            cur = 0.0;
                            mats.clear();
                        } else {
                            if cur == 0.0 { run_start = a; }
                            cur += step;
                            let m = surf.grid.cell_of(b[0], b[2]).and_then(|(ix, iz)| surf.grid.cells[iz * surf.grid.nx + ix].iter().min_by(|p, q| (p.y - b[1]).abs().partial_cmp(&(q.y - b[1]).abs()).unwrap()).map(|s| format!("{}@{:+.1}", surf.grid.mats[s.mat as usize], s.y - b[1]))).unwrap_or_else(|| "void".to_string());
                            *mats.entry(m).or_default() += 1;
                        }
                    }
                    flush(cur, &mats, run_start);
                }
                segs.push(format!("{{\"from_group\": {}, \"to_group\": {}, \"i0\": {i0}, \"i1\": {i0}, \"gap\": true, \"to_pos\": [{:.2}, {:.2}, {:.2}]}}", grp(&nodes, i), grp(&nodes, j), nodes.pos[j][0], nodes.pos[j][1], nodes.pos[j][2]));
            }
        }
    }
    let mut s = vec![0.0f32];
    for k in 1..pts.len() { let a = pts[k - 1]; let b = pts[k]; s.push(s[k - 1] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()); }
    let note = format!("{}{}", flag(args, "--note").unwrap_or_default(), spawn_note);
    // optional per-point advisory speed (player, 06:44Z): lateral 25 m/s², leave-ground at 2.5 g of required
    // downward acceleration, 80 m/s ceiling, braking 12 m/s², acceleration 7 m/s², ±16 m curvature window
    let hint = tmplan::speed_hints(&pts, &s, 16.0, 25.0, 9.81 * 2.5, 80.0, 12.0, 7.0);
    let js = format!(
        "{{\n  \"map_uid\": \"{}\",\n  \"map_name\": \"{}\",\n  \"order_groups\": [{}],\n  \"pts\": [{}],\n  \"s\": [{}],\n  \"half_width\": [{}],\n  \"speed_hint\": [{}],\n  \"speed_hint_note\": \"m/s, advisory: min of lateral-grip (25 m/s²) curvature limit, leave-ground limit on crests/dip exits (2.5 g of required downward acceleration), 80 m/s ceiling; braking 12 m/s² and acceleration 7 m/s² propagated along s; ±16 m curvature window; standing start\",\n  \"segments\": [{}],\n  \"gaps\": {gaps},\n  \"produced_by\": \"{}{}\"\n}}\n",
        gates.map_uid, gates.map_name,
        order.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(","),
        pts.iter().map(|p| format!("[{:.2},{:.2},{:.2}]", p[0], p[1], p[2])).collect::<Vec<_>>().join(","),
        s.iter().map(|v| format!("{v:.1}")).collect::<Vec<_>>().join(","),
        hw.iter().map(|v| format!("{v:.1}")).collect::<Vec<_>>().join(","),
        hint.iter().map(|v| format!("{v:.1}")).collect::<Vec<_>>().join(","),
        segs.join(", "),
        tmroute::provenance("tmplan road-centreline"), if note.is_empty() { String::new() } else { format!("; {note}") }
    );
    io::write_atomic(Path::new(&out), js.as_bytes()).unwrap_or_else(|e| die(&e));
    // --route-out: the same centreline as a route file (TrackGeom + Leg per gate; a gap leg is ConnectionClass::Unknown
    // with s_start == s_end — the consumer bridges it), source "router-road-centreline"
    if let Some(ro) = flag(args, "--route-out") {
        use tmroute::types::*;
        // --verdicts FILE.tsv (map_stem \t from_group \t to_group \t class \t note), filtered to this map by --map-stem
        let stem = flag(args, "--map-stem").unwrap_or_default();
        let verdicts: Vec<(String, String, String, String)> = flag(args, "--verdicts").and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.lines().filter(|l| !l.starts_with('#')).filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); if f.len() >= 4 && f[0] == stem { Some((f[1].to_string(), f[2].to_string(), f[3].to_string(), f.get(4).unwrap_or(&"").to_string())) } else { None } }).collect()).unwrap_or_default();
        let mut verdict_notes: Vec<String> = Vec::new();
        let mut tg_gates = Vec::new();
        let mut legs = Vec::new();
        let mut gate_order = Vec::new();
        for (li, w) in seq.windows(2).enumerate() {
            let to = w[1];
            let grp_id = nodes.groups[to];
            let (centre, _axis, half) = gates.group_geometry(grp_id).unwrap();
            let rep = gates.group_rep(grp_id).unwrap();
            let seg = &segs[li];
            let gap = seg.contains("\"gap\": true");
            let i1: usize = seg.split("\"i1\": ").nth(1).and_then(|x| x.split(',').next()).and_then(|x| x.trim().parse().ok()).unwrap_or(0);
            let i0: usize = seg.split("\"i0\": ").nth(1).and_then(|x| x.split(',').next()).and_then(|x| x.trim().parse().ok()).unwrap_or(0);
            let heading = if i1 >= 1 && i1 < pts.len() && i1 > i0 { let a = pts[i1 - 1]; let b = pts[i1]; let l = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt().max(1e-3); [(b[0] - a[0]) / l, 0.0, (b[2] - a[2]) / l] } else { rep.normal };
            let kind = if li + 1 == seq.len() - 1 { GateKind::Finish } else { match rep.kind { tmroute::gates::WpKind::Finish => GateKind::Finish, tmroute::gates::WpKind::Multilap => GateKind::Multilap, _ => GateKind::Checkpoint } };
            // the gate's plane is the ITEM's facing (its trigger disc), signed along the line's travel — the polyline's
            // arrival heading can be 30° off on a curve into the gate (ENV's crossing test missed 04 wp12/wp3, 12:23Z)
            let gate_normal = {
                let n = rep.normal;
                let horiz = (n[0] * n[0] + n[2] * n[2]).sqrt();
                if horiz > 0.5 { let s = if n[0] * heading[0] + n[2] * heading[2] >= 0.0 { 1.0 } else { -1.0 }; [s * n[0] / horiz, 0.0, s * n[2] / horiz] } else { heading }
            };
            // the gate's centre in the route file is the CREDIT plane: the geometric centre moved by credit_offset_m along the
            // signed normal (F25 fit on the disc triggers) — the player's crossing test needs the plane the engine credits
            let co = rep.credit_offset_m;
            let centre_c = [centre[0] + gate_normal[0] * co, centre[1], centre[2] + gate_normal[2] * co];
            tg_gates.push(Gate { kind, centre: centre_c, normal: gate_normal, half_width: half, s: s[i1], map_waypoint: rep.waypoint });
            // a gap leg's class from the converter's verdicts (--verdicts TSV: map_stem, from_group, to_group, class, note)
            let from_lab = grp(&nodes, w[0]).trim_matches('"').to_string();
            // exact (from, to) first, else any Jump/Drop verdict INTO this gate (orders move between builds)
            let verdict = verdicts.iter().find(|v| v.0 == from_lab && v.1 == grp_id.to_string()).cloned().or_else(|| verdicts.iter().find(|v| v.1 == grp_id.to_string() && (v.2 == "Jump" || v.2 == "Drop")).cloned());
            // a manual drive line or a gap keeps the verdict class (a Jump stays a Jump even with points to follow)
            let manual = seg.contains("\"via\": \"manual\"");
            let conn = if !gap && !manual { ConnectionClass::Road } else { match verdict.as_ref().map(|v| v.2.as_str()) { Some("Jump") => ConnectionClass::Jump, Some("Drop") => ConnectionClass::Drop, Some("Road") => ConnectionClass::Road, _ => if manual { ConnectionClass::Road } else { ConnectionClass::Unknown } } };
            if let Some(v) = &verdict { verdict_notes.push(format!("{}→{} {}{}", v.0, v.1, v.2, if v.3.is_empty() { String::new() } else { format!(" ({})", v.3) })); }
            legs.push(Leg { gate_idx: li as u32, map_waypoint: rep.waypoint, s_start: s[i0], s_end: s[i1], connection: conn, arrival_speed: [5.0, 80.0], arrival_heading: gate_normal, arrival_heading_tol: 0.5, arrival_height: [centre[1] - rep.half_height - 1.0, centre[1] - rep.half_height + 3.0], p_reach: if gap { 0.0 } else { 1.0 }, expected_ms: -1, evidence: LegEvidence::Predicted });
            gate_order.push(rep.waypoint);
        }
        let tg = TrackGeom {
            geom_version: GEOM_VERSION,
            map_uid: gates.map_uid.clone(),
            pts: pts.clone(),
            half_width: hw.clone(),
            s: s.clone(),
            gates: tg_gates,
            spawn: gates.spawn.pos,
            spawn_yaw: gates.spawn.yaw,
            source: "router-road-centreline".into(),
            legs: Some(legs),
            route: Some(RouteMeta { route_version: ROUTE_VERSION, source: "router-road-centreline".into(), rank: 0, predicted_ms: -1, status: RouteStatus::Hypothesis, gate_order, produced_by: format!("{}{}; road-following centreline, {gaps} gap legs (s_start == s_end; class Unknown unless a converter verdict says Jump/Drop){}", tmroute::provenance("tmplan road-centreline"), if note.is_empty() { String::new() } else { format!("; {note}") }, if verdict_notes.is_empty() { String::new() } else { format!("; verdicts: {}", verdict_notes.join(", ")) }) }),
        };
        io::write_route(Path::new(&ro), &tg).unwrap_or_else(|e| die(&e));
    }
    let on_road = pts.iter().enumerate().filter(|(k, p)| { let d = if *k + 1 < pts.len() { [pts[k + 1][0] - p[0], pts[k + 1][2] - p[2]] } else { [0.0, 1.0] }; surf.road_span(**p, d, 24.0).is_some() }).count();
    // control: a centreline whose first 50 m descend more than 5 m stepped off its start deck (Argentina 2026)
    let y0 = pts[0][1];
    let min50 = pts.iter().zip(s.iter()).filter(|(_, si)| **si <= 50.0).map(|(p, _)| p[1]).fold(y0, f32::min);
    let flag = if y0 - min50 > 5.0 { format!("  FLAG: first 50 m descend {:.1} m", y0 - min50) } else { String::new() };
    println!("{}: {} pts, {:.0} m, {} segments, {gaps} gaps, on-road {:.1} %{flag} → {out}", gates.map_name, pts.len(), s.last().unwrap(), segs.len(), 100.0 * on_road as f32 / pts.len().max(1) as f32);
    fn grp(n: &Nodes, i: usize) -> String { if i == 0 { "\"spawn\"".into() } else { n.groups[i].to_string() } }
    fn grp_id_of(n: &Nodes, i: usize) -> String { n.groups[i].to_string() }
}
