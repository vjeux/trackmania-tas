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
        "author-line" => cmd_author_line(&args[1..]),
        "author-ground" => cmd_author_ground(&args[1..]),
        "arrival-bands" => cmd_arrival_bands(&args[1..]),
        "leg-plot" => cmd_leg_plot(&args[1..]),
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
    let scene = mapgeom::local::LocalScene::build(&mut store, &m, gates.yoff, &mapgeom::local::BuildOpts { with_deco: true, with_baked: std::env::var("TMPLAN_BAKED").is_ok() || !tmroute::gates::is_tiny_map(&gates.map_uid, &gates.map_name), cell: 4.0 });
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
        let opts = mapgeom::local::BuildOpts { with_deco: false, with_baked: std::env::var("TMPLAN_BAKED").is_ok() || !tmroute::gates::is_tiny_map(&gates.map_uid, &gates.map_name), cell: 4.0 };
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
    // --author-line author.json: the HUMAN line replaces the polyline wholesale (the coordinator's lap directive, 17:23Z:
    // "the human line is a better guide than the planner polyline everywhere"). Points resampled at 2 m and smoothed
    // (5-point), half-width = the road span where the point is on road, else 4 m; the segments are re-cut at each gate's
    // first pass (finish: last pass) in `seq` order; a leg the human never passes within reach of stays as it was.
    let mut author_note = String::new();
    let mut author_used = false;
    // with --author-line the speed hint is the HUMAN's measured speed (100 ms sample spacing × 10, 5-sample smoothed),
    // not the curvature estimate (11's plateau: the estimate said 63 m/s, the human drives it at 96–114 — 16:47Z)
    let mut human_speed: Option<Vec<f32>> = None;
    if let Some(ap) = flag(args, "--author-line") {
        let txt = std::fs::read_to_string(&ap).unwrap_or_else(|e| die(&format!("{ap}: {e}")));
        let k = "\"pts\": [";
        let i = txt.find(k).unwrap_or_else(|| die(&format!("{ap}: no pts")));
        let rest = &txt[i + k.len()..];
        let end = rest.find("]]").map(|e| e + 1).unwrap_or(rest.len());
        let flat: Vec<f32> = rest[..end].split(|c: char| c == ',' || c == '[' || c == ']').filter_map(|x| x.trim().parse::<f32>().ok()).collect();
        let raw: Vec<[f32; 3]> = flat.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect();
        if raw.len() >= 10 {
            let mut line = tmroute::human::resample(&raw, 2.0);
            {
                let mut vraw: Vec<f32> = (0..raw.len()).map(|i| if i + 1 < raw.len() { let (a, b) = (raw[i], raw[i + 1]); ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt() * 10.0 } else { 0.0 }).collect();
                if vraw.len() > 1 { let n = vraw.len(); vraw[n - 1] = vraw[n - 2]; }
                let sm: Vec<f32> = (0..vraw.len()).map(|i| { let lo = i.saturating_sub(2); let hi = (i + 2).min(vraw.len() - 1); vraw[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32 }).collect();
                human_speed = Some(line.iter().map(|p| { let mut best = (f32::INFINITY, 0usize); for (k, q) in raw.iter().enumerate() { let d = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2); if d < best.0 { best = (d, k); } } sm[best.1] }).collect());
            }
            let orig = line.clone();
            for k in 2..orig.len().saturating_sub(2) {
                let mut acc = [0.0f32; 3];
                for j in k - 2..=k + 2 { acc[0] += orig[j][0]; acc[1] += orig[j][1]; acc[2] += orig[j][2]; }
                line[k] = [acc[0] / 5.0, acc[1] / 5.0, acc[2] / 5.0];
            }
            // gate passes along the human line → segment cuts
            let mut cuts: Vec<(usize, usize)> = Vec::new(); // (index in line, seq position)
            let mut ok = true;
            for (si, &nd) in seq.iter().enumerate().skip(1) {
                let grp_id = nodes.groups[nd];
                let is_fin = gates.gates.iter().any(|x| x.group == grp_id && matches!(x.kind, tmroute::gates::WpKind::Finish));
                // the cut = the CROSSING, not the first sample within reach: within the (first, or for a finish the last) run
                // of in-range samples take the one nearest the gate plane (min lateral distance to a gate of the group) —
                // the first-in-range sample sat 10–12 m before every credit on 15 certified laps (INPUT tables, 05:20Z)
                let mut found: Option<usize> = None;
                let mut run_best: Option<(usize, f32)> = None;
                let mut best_run: Option<(usize, f32)> = None;
                let mut in_run = false;
                let (mut lo, mut hi) = (0usize, 0usize);
                let _ = (lo, hi);
                // scan from the previous gate's cut: the pass AFTER the previous gate (GEN's 22:59Z rule; 23's overlapping runs)
                let start = cuts.last().map(|c| c.0 + 1).unwrap_or(0);
                for (li, p) in line.iter().enumerate().skip(start) {
                    let dmin = gates.gates.iter().filter(|x| x.group == grp_id).filter(|x| { let dy = p[1] - x.centre[1]; dy >= -9.0 && dy <= 3.0 }).map(|x| ((x.centre[0] - p[0]).powi(2) + (x.centre[2] - p[2]).powi(2)).sqrt() - x.half_width).fold(f32::INFINITY, f32::min);
                    let hit = dmin <= 6.0;
                    if hit {
                        if !in_run { in_run = true; run_best = None; lo = li; }
                        hi = li;
                        if run_best.map(|(_, d)| dmin < d).unwrap_or(true) { run_best = Some((li, dmin)); }
                    } else if in_run {
                        in_run = false;
                        if is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true) { best_run = run_best; }
                    }
                }
                if in_run && (is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true)) { best_run = run_best; }
                found = best_run.map(|(i, _)| i);
                match found { Some(li) => cuts.push((li, si)), None => { ok = false; eprintln!("  author line never passes group {grp_id} — author line NOT used"); break; } }
            }
            let monotone = cuts.windows(2).all(|w| w[1].0 > w[0].0);
            if ok && monotone {
                pts = line;
                hw = Vec::with_capacity(pts.len());
                for k in 0..pts.len() {
                    let d = if k + 1 < pts.len() { [pts[k + 1][0] - pts[k][0], pts[k + 1][2] - pts[k][2]] } else { [pts[k][0] - pts[k - 1][0], pts[k][2] - pts[k - 1][2]] };
                    hw.push(surf.road_span(pts[k], d, 24.0).map_or(4.0, |(l, r)| ((l + r) / 2.0).max(3.0)));
                }
                segs.clear();
                gaps = 0;
                let mut i0 = 0usize;
                for (li, si) in &cuts {
                    let from = if *si == 1 { "\"spawn\"".to_string() } else { nodes.groups[seq[*si - 1]].to_string() };
                    segs.push(format!("{{\"from_group\": {}, \"to_group\": {}, \"i0\": {i0}, \"i1\": {li}, \"gap\": false, \"via\": \"author\"}}", from, nodes.groups[seq[*si]]));
                    i0 = *li;
                }
                author_note = format!("; POLYLINE = the human line ({ap}), {} pts, gates cut at the human's passes; speed_hint = the human's measured speed", pts.len());
                eprintln!("  author line used as the centreline: {} pts, {} legs", pts.len(), cuts.len());
                author_used = true;
            } else if ok {
                eprintln!("  author line passes the gates out of the given order — author line NOT used");
            }
        }
    }
    let mut s = vec![0.0f32];
    for k in 1..pts.len() { let a = pts[k - 1]; let b = pts[k]; s.push(s[k - 1] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()); }
    let note = format!("{}{}{}", flag(args, "--note").unwrap_or_default(), spawn_note, author_note);
    // optional per-point advisory speed (player, 06:44Z): lateral 25 m/s², leave-ground at 2.5 g of required
    // downward acceleration, 80 m/s ceiling, braking 12 m/s², acceleration 7 m/s², ±16 m curvature window
    let hint = match &human_speed { Some(h) if h.len() == pts.len() => h.clone(), _ => tmplan::speed_hints(&pts, &s, 16.0, 25.0, 9.81 * 2.5, 80.0, 12.0, 7.0) };
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
            // on a human-line leg (author or manual) the class comes from the line itself: ≥ 12 m with nothing under the car within
            // 3 m = a Jump (20 wp3→wp5: the author flies 45 m from the asphalt end onto the wooden road — the route said Road)
            let airborne_m = if author_used || manual { let mut tot = 0.0f32; for k in (i0 + 1)..=i1.min(pts.len() - 1) { let prof = surf.chord_profile(pts[k - 1], pts[k], 1.0, 3.0); for (a, b) in &prof.gaps { tot += b - a; } } tot } else { 0.0 };
            let air_jump = airborne_m >= 15.0;
            let conn = if air_jump && verdict.as_ref().map(|v| v.2 != "Drop").unwrap_or(true) { ConnectionClass::Jump } else if !gap && !manual { ConnectionClass::Road } else { match verdict.as_ref().map(|v| v.2.as_str()) { Some("Jump") => ConnectionClass::Jump, Some("Drop") => ConnectionClass::Drop, Some("Road") => ConnectionClass::Road, _ => if manual { ConnectionClass::Road } else { ConnectionClass::Unknown } } };
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
            speed_hint: Some(hint.clone()),
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

/// `author-line SRC.Map.Gbx --anchor sx,sy,sz:tx,ty,tz [--scale 0.5] --centreline X.road-centreline.json [--out author.json]`
/// The ORIGINAL author's validation ghost mapped into the tiny frame (converter anchors: tiny = ta + (src − sa) × scale)
/// against our polyline: lateral offset per sample (nearest polyline point, XZ), summary and worst stretch. The
/// route-sanity oracle the coordinator asked for (16:06Z); the author line itself is written as points for the player.
fn cmd_author_line(args: &[String]) {
    let src = args.iter().find(|a| a.ends_with(".Map.Gbx") || a.ends_with(".Ghost.Gbx") || a.ends_with(".Replay.Gbx")).cloned().unwrap_or_else(|| die("SRC.Map.Gbx / Ghost.Gbx required"));
    let anchor = flag(args, "--anchor").unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz"));
    let (sa, ta) = {
        let mut it = anchor.split(':');
        let p = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() != 3 { die("--anchor sx,sy,sz:tx,ty,tz") } [v[0], v[1], v[2]] };
        (p(it.next().unwrap_or("")), p(it.next().unwrap_or("")))
    };
    let scale: f32 = flag(args, "--scale").and_then(|s| s.parse().ok()).unwrap_or(0.5);
    let cl = flag(args, "--centreline").unwrap_or_else(|| die("--centreline X.road-centreline.json"));
    let txt = std::fs::read_to_string(&cl).unwrap_or_else(|e| die(&format!("{cl}: {e}")));
    let grab = |key: &str| -> Vec<f32> {
        let k = format!("\"{key}\": [");
        let i = txt.find(&k).unwrap_or_else(|| die(&format!("{cl}: no {key}")));
        let rest = &txt[i + k.len()..];
        // pts is an array of arrays: take everything up to "]]," ; scalars up to "]"
        let end = if key == "pts" { rest.find("]]").map(|e| e + 1).unwrap_or(rest.len()) } else { rest.find(']').unwrap_or(rest.len()) };
        rest[..end].split(|c: char| c == ',' || c == '[' || c == ']').filter_map(|x| x.trim().parse::<f32>().ok()).collect()
    };
    let flat = grab("pts");
    let pts: Vec<[f32; 3]> = flat.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect();
    let s = grab("s");
    let hw = grab("half_width");
    // every vehicle entity merged (a car-switch or multi-entity ghost keeps only a stretch per entity)
    let d = gbx::record::decode_ghost_all_vehicles(&src).unwrap_or_else(|e| die(&format!("{src}: no ghost ({e})")));
    // 100 ms samples in the tiny frame
    let mut samples: Vec<([f32; 3], i32)> = Vec::new();
    let mut last_t = i32::MIN;
    for smp in &d.samples {
        if smp.time_ms < last_t + 100 { continue; }
        last_t = smp.time_ms;
        samples.push(([ta[0] + (smp.x - sa[0]) * scale, ta[1] + (smp.y - sa[1]) * scale, ta[2] + (smp.z - sa[2]) * scale], smp.time_ms));
    }
    // LOOP-FREE (coordinator 12:02Z, 24's WR: fell at the ramp, respawned to CP 13, redid the finish ramp — the line had the
    // ramp twice): when a later stretch coincides with an earlier one (≤ 0.5 m for ≥ 6 consecutive samples, ≥ 20
    // samples apart), the samples between are a respawn loop — dropped and spliced
    let mut respawns: Vec<(i32, i32, usize)> = Vec::new();
    {
        let mut i = 0usize;
        while i < samples.len() {
            let mut cut: Option<usize> = None;
            'search: for j in (i + 20)..samples.len() {
                if j + 6 > samples.len() { break; }
                let mut ok = true;
                for k in 0..6 { if i + k >= samples.len() { ok = false; break; } let a = samples[i + k].0; let b = samples[j + k].0; if ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() > 0.5 { ok = false; break; } }
                // a RESPAWN shows as a teleport (the sample before j is far from j) or a standstill at j — a track that legitimately
                // re-passes the same road (a figure-8) is continuous and must stay
                let jump = { let a = samples[j - 1].0; let b = samples[j].0; ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() };
                let still = (j + 3 < samples.len()) && (0..3).all(|k| { let a = samples[j + k].0; let b = samples[j + k + 1].0; ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() < 0.3 });
                if ok && (jump > 8.0 || still) { cut = Some(j); break 'search; }
            }
            if let Some(j) = cut {
                respawns.push((samples[i].1, samples[j].1, j - i));
                samples.drain(i + 1..=j);
            }
            i += 1;
        }
        // second form (a human respawn re-drives a DIFFERENT line): a teleport (> 12 m in 100 ms, or > 5 m followed by a
        // standstill) landing within 4 m of an EARLIER sample = respawn to that checkpoint — cut the loop between them
        let mut j = 1usize;
        while j < samples.len() {
            let a = samples[j - 1].0; let b = samples[j].0;
            let jump = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            let still = (j + 3 < samples.len()) && (0..3).all(|k| { let p = samples[j + k].0; let q = samples[j + k + 1].0; ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt() < 0.3 });
            if jump > 12.0 || (jump > 5.0 && still) {
                let mut best: Option<(usize, f32)> = None;
                for i in 0..j.saturating_sub(10) { let p = samples[i].0; let d = ((p[0] - b[0]).powi(2) + (p[1] - b[1]).powi(2) + (p[2] - b[2]).powi(2)).sqrt(); if d < 4.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) { best = Some((i, d)); } }
                if let Some((i, _)) = best {
                    respawns.push((samples[i].1, samples[j].1, j - i));
                    samples.drain(i + 1..=j);
                    j = i + 1;
                    continue;
                }
            }
            j += 1;
        }
    }
    for (t0, t1, n) in &respawns { eprintln!("  respawn loop removed: {n} samples, t {:.1}–{:.1} s (the ghost re-drives the same stretch)", *t0 as f32 / 1000.0, *t1 as f32 / 1000.0); }
    let mut rows: Vec<(f32, f32, f32, [f32; 3], i32)> = Vec::new(); // (s_nearest, lateral, dy, tiny pos, t)
    let mut author: Vec<[f32; 3]> = Vec::new();
    for &(q, t_ms) in &samples {
        author.push(q);
        let mut best = (f32::INFINITY, 0usize);
        for (k, p) in pts.iter().enumerate() {
            let dd = (p[0] - q[0]).powi(2) + (p[2] - q[2]).powi(2);
            if dd < best.0 { best = (dd, k); }
        }
        let k = best.1;
        rows.push((s.get(k).copied().unwrap_or(0.0), best.0.sqrt(), q[1] - pts[k][1], q, t_ms));
    }
    if rows.is_empty() { die("no ghost samples"); }
    let mut lat: Vec<f32> = rows.iter().map(|r| r.1).collect();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = lat[lat.len() / 2];
    let p90 = lat[(lat.len() as f32 * 0.9) as usize];
    let within: usize = rows.iter().filter(|r| { let k = s.iter().position(|v| *v >= r.0).unwrap_or(0); r.1 <= hw.get(k).copied().unwrap_or(4.0) + 2.0 }).count();
    let worst = rows.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
    // the longest stretch (in ghost time) more than 12 m off the line
    let mut run_start: Option<i32> = None; let mut best_run = (0i32, 0i32, [0.0f32; 3]);
    for r in &rows {
        if r.1 > 12.0 { if run_start.is_none() { run_start = Some(r.4); } let len = r.4 - run_start.unwrap(); if len > best_run.0 { best_run = (len, run_start.unwrap(), r.3); } } else { run_start = None; }
    }
    println!("author line vs polyline: {} samples ({:.3} s), lateral median {med:.1} m, p90 {p90:.1} m, within half-width+2 m {:.0} %, worst {:.1} m at s {:.0} (author at ({:.0}, {:.0}, {:.0}), t {:.3} s); longest off-line (> 12 m) stretch {:.1} s from t {:.3} s at ({:.0}, {:.0}, {:.0})",
        rows.len(), d.end_ms as f32 / 1000.0, 100.0 * within as f32 / rows.len() as f32, worst.1, worst.0, worst.3[0], worst.3[1], worst.3[2], worst.4 as f32 / 1000.0, best_run.0 as f32 / 1000.0, best_run.1 as f32 / 1000.0, best_run.2[0], best_run.2[1], best_run.2[2]);
    // --gates deck.json: the AUTHOR's gate order = the order in which the ghost first comes within 10 m (XZ) and 6 m (y)
    // of each gate group's centre — the ground truth of a finishing lap on the tiny map
    let mut author_order: Vec<u32> = Vec::new();
    if let Some(gp) = flag(args, "--gates") {
        let g = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
        let mut firsts: Vec<(i32, u32)> = Vec::new();
        let mut groups: Vec<u32> = g.gates.iter().filter(|r| r.group != u32::MAX).map(|r| r.group).collect();
        groups.sort_unstable();
        groups.dedup();
        for grp in groups {
            // FIRST pass for a checkpoint; LAST pass for a finish group (the lap ends there — a finish tower passed under
            // earlier must not be ordered early; tiny 13)
            let is_fin = g.gates.iter().any(|x| x.group == grp && matches!(x.kind, tmroute::gates::WpKind::Finish));
            // the CROSSING time: within the first (finish: last) in-range run, the sample nearest the gate plane — the same
            // rule as road-centreline's cuts, so the order and the cuts agree (23: two gates with overlapping runs)
            let mut t_first: Option<i32> = None;
            let mut run_best: Option<(i32, f32)> = None;
            let mut best_run: Option<(i32, f32)> = None;
            let mut in_run = false;
            for r in &rows {
                let dmin = g.gates.iter().filter(|x| x.group == grp).filter(|x| { let dy = r.3[1] - x.centre[1]; dy >= -9.0 && dy <= 3.0 }).map(|x| ((x.centre[0] - r.3[0]).powi(2) + (x.centre[2] - r.3[2]).powi(2)).sqrt() - x.half_width).fold(f32::INFINITY, f32::min);
                if dmin <= 6.0 {
                    if !in_run { in_run = true; run_best = None; }
                    if run_best.map(|(_, d)| dmin < d).unwrap_or(true) { run_best = Some((r.4, dmin)); }
                } else if in_run {
                    in_run = false;
                    // checkpoints: the DEEPEST run (smallest lateral distance = through the ring; 23's helix passes 4 m under
                    // gate 11 before crossing it); finish: the last run
                    if is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true) { best_run = run_best; }
                }
            }
            if in_run && (is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true)) { best_run = run_best; }
            t_first = best_run.map(|(t, _)| t);
            if let Some(t) = t_first { firsts.push((t, grp)); } else {
                let (mut dmin, mut at) = (f32::INFINITY, [0.0f32; 3]);
                for r in &rows { for x in g.gates.iter().filter(|x| x.group == grp) { let dd = ((x.centre[0] - r.3[0]).powi(2) + (x.centre[1] - r.3[1]).powi(2) + (x.centre[2] - r.3[2]).powi(2)).sqrt(); if dd < dmin { dmin = dd; at = r.3; } } }
                eprintln!("  author never at group {grp}: nearest {dmin:.1} m at ({:.0}, {:.0}, {:.0})", at[0], at[1], at[2]);
            }
        }
        firsts.sort();
        author_order = firsts.iter().map(|f| f.1).collect();
        println!("author order (groups, first pass): {}", author_order.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(","));
    }
    if let Some(label) = flag(args, "--row") {
        let resp = if respawns.is_empty() { String::new() } else { format!(" RESPAWN ×{}: {}", respawns.len(), respawns.iter().map(|(a, b, n)| format!("t {:.1}–{:.1} s ({n} samples dropped)", *a as f32 / 1000.0, *b as f32 / 1000.0)).collect::<Vec<_>>().join(", ")) };
        println!("| {label} | {} ({:.1} s) | {med:.1} | {p90:.1} | {:.0} % | {:.1} @ s {:.0} (author at ({:.0}, {:.0}, {:.0}), t {:.1}) | {:.1} s from t {:.1} at ({:.0}, {:.0}, {:.0}){resp} |", rows.len(), d.end_ms as f32 / 1000.0, 100.0 * within as f32 / rows.len() as f32, worst.1, worst.0, worst.3[0], worst.3[1], worst.3[2], worst.4 as f32 / 1000.0, best_run.0 as f32 / 1000.0, best_run.1 as f32 / 1000.0, best_run.2[0], best_run.2[1], best_run.2[2]);
    }
    if let Some(out) = flag(args, "--out") {
        let js = format!("{{\n  \"source\": \"{}\",\n  \"anchor\": \"{anchor}\",\n  \"scale\": {scale},\n  \"pts\": [{}],\n  \"lateral_to_centreline\": [{}],\n  \"note\": \"the ORIGINAL author's validation ghost mapped into the tiny frame (100 ms samples, ground contact point); lateral = XZ distance to the nearest centreline point\"\n}}\n",
            src, author.iter().map(|p| format!("[{:.2},{:.2},{:.2}]", p[0], p[1], p[2])).collect::<Vec<_>>().join(","), rows.iter().map(|r| format!("{:.1}", r.1)).collect::<Vec<_>>().join(","));
        io::write_atomic(Path::new(&out), js.as_bytes()).unwrap_or_else(|e| die(&e));
    }
}

/// `author-ground A.Map.Gbx B.Map.Gbx --gates deck.json --author-line author.json`: for every author-line sample, the
/// ground under it (downward ray from y+1, 8 m) in build A and in build B; rows where A has a surface within 1.5 m
/// below the car and B has none within 3 m = a surface the author drove that B no longer has (ship10 dropped the
/// tiny-15 pool shelf, 22:43Z). Prints the runs of such samples with their extent.
fn cmd_author_ground(args: &[String]) {
    // optional third map = the SOURCE (full size) with --anchor sx,sy,sz:tx,ty,tz and --source-gates gates.json: a stretch
    // with no surface under the raw ghost sample in the original either is a FLIGHT, not a converter loss (04's apex, 23:02Z)
    let maps: Vec<String> = args.iter().filter(|a| a.ends_with(".Map.Gbx")).cloned().collect();
    if maps.len() < 2 || maps.len() > 3 { die("two maps: A.Map.Gbx B.Map.Gbx [SOURCE.Map.Gbx --anchor sx,sy,sz:tx,ty,tz --source-gates gates.json]"); }
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates deck.json"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let ap = flag(args, "--author-line").unwrap_or_else(|| die("--author-line author.json"));
    let txt = std::fs::read_to_string(&ap).unwrap_or_else(|e| die(&format!("{ap}: {e}")));
    let k = "\"pts\": [";
    let i = txt.find(k).unwrap_or_else(|| die("no pts"));
    let rest = &txt[i + k.len()..];
    let end = rest.find("]]").map(|e| e + 1).unwrap_or(rest.len());
    let flat: Vec<f32> = rest[..end].split(|c: char| c == ',' || c == '[' || c == ']').filter_map(|x| x.trim().parse::<f32>().ok()).collect();
    let pts: Vec<[f32; 3]> = flat.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect();
    let paths = tmplan::pak_paths().unwrap_or_else(|e| die(&e));
    let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).unwrap_or_else(|e| die(&e));
    let opts = mapgeom::local::BuildOpts { with_deco: true, with_baked: std::env::var("TMPLAN_BAKED").is_ok() || !tmroute::gates::is_tiny_map(&gates.map_uid, &gates.map_name), ..Default::default() };
    let ground = |mp: &str, store: &mut mapgeom::store::DataStore| -> Vec<Option<(f32, &'static str)>> {
        let m = tmmaps::map::MapFile::load(Path::new(mp));
        let s = mapgeom::local::LocalScene::build(store, &m, gates.yoff, &opts);
        // water planes are not solid for the car (tiny 15: the car fell through the Water at 46 once the Concrete shelf
        // under it was gone) — skip Water/Sea/Lake hits and keep casting below them
        pts.iter().map(|p| {
            // from 0.3 m above the ghost's contact point: a non-solid plane 0.5 m up (tiny 15's pool surface, Rubber at 46 over the shelf at 45.5) must not count
            let mut o = [p[0], p[1] + 0.3, p[2]];
            for _ in 0..4 {
                match s.raycast(o, [0.0, -1.0, 0.0], 9.0 - (p[1] + 0.3 - o[1]), true) {
                    Some(h) if matches!(h.material_name, "Water" | "Sea" | "Lake" | "WaterSurface") => { o = [h.point[0], h.point[1] - 0.05, h.point[2]]; }
                    Some(h) => return Some((p[1] - h.point[1], h.material_name)),
                    None => return None,
                }
            }
            None
        }).collect()
    };
    let ga = ground(&maps[0], &mut store);
    let gb = ground(&maps[1], &mut store);
    // the original: sample back to source coordinates, rays on the source scene (baked terrain on)
    let gs: Option<Vec<Option<(f32, &'static str)>>> = if maps.len() == 3 {
        let anchor = flag(args, "--anchor").unwrap_or_else(|| die("--anchor sx,sy,sz:tx,ty,tz with a source map"));
        let mut it = anchor.split(':');
        let pa = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect(); if v.len() != 3 { die("--anchor sx,sy,sz:tx,ty,tz") } [v[0], v[1], v[2]] };
        let (sa, ta) = (pa(it.next().unwrap_or("")), pa(it.next().unwrap_or("")));
        let scale: f32 = flag(args, "--scale").and_then(|s| s.parse().ok()).unwrap_or(0.5);
        let sg = flag(args, "--source-gates").map(|p| io::read_gates(Path::new(&p)).unwrap_or_else(|e| die(&e)));
        let yoff = sg.as_ref().map(|g| g.yoff).unwrap_or(gates.yoff);
        let m = tmmaps::map::MapFile::load(Path::new(&maps[2]));
        let s = mapgeom::local::LocalScene::build(&mut store, &m, yoff, &mapgeom::local::BuildOpts::default());
        Some(pts.iter().map(|p| {
            let q = [sa[0] + (p[0] - ta[0]) / scale, sa[1] + (p[1] - ta[1]) / scale, sa[2] + (p[2] - ta[2]) / scale];
            let mut o = [q[0], q[1] + 0.6, q[2]];
            for _ in 0..4 {
                match s.raycast(o, [0.0, -1.0, 0.0], 18.0 - (q[1] + 0.6 - o[1]), true) {
                    Some(h) if matches!(h.material_name, "Water" | "Sea" | "Lake" | "WaterSurface") => { o = [h.point[0], h.point[1] - 0.05, h.point[2]]; }
                    Some(h) => return Some(((q[1] - h.point[1]) * scale, h.material_name)),
                    None => return None,
                }
            }
            None
        }).collect())
    } else { None };
    let mut run_start: Option<usize> = None;
    let mut n_bad = 0usize;
    let mut flush = |a: usize, b: usize| {
        let (p0, p1) = (pts[a], pts[b]);
        println!("  MISSING in B: samples {a}..{b} ({} pts, t {:.1}–{:.1} s) from ({:.1}, {:.1}, {:.1}) to ({:.1}, {:.1}, {:.1}); A ground {} at {:+.1} m, B {}",
            b - a + 1, a as f32 / 10.0, b as f32 / 10.0, p0[0], p0[1], p0[2], p1[0], p1[1], p1[2],
            ga[a].map(|g| g.1).unwrap_or("-"), ga[a].map(|g| -g.0).unwrap_or(0.0),
            gb[a].map(|g| format!("{} at {:+.1} m", g.1, -g.0)).unwrap_or_else(|| "nothing within 8 m".into()));
        if let Some(g) = &gs { println!("      original under the source sample: {}", g[a].map(|g| format!("{} at {:+.1} m (tiny scale)", g.1, -g.0)).unwrap_or_else(|| "nothing within 18 m — a flight OR a generated filler the source scene does not build (converter plumb decides)".into())); }
    };
    for i in 0..pts.len() {
        let a_ok = ga[i].map(|g| g.0 >= -0.3 && g.0 <= 1.5).unwrap_or(false);
        let b_bad = gb[i].map(|g| g.0 > 3.0).unwrap_or(true);
        // with a source map: only a stretch the ORIGINAL supports counts (no surface under the source sample = flight)
        let src_ok = gs.as_ref().map(|g| g[i].map(|g| g.0 >= -0.6 && g.0 <= 1.5).unwrap_or(false)).unwrap_or(true);
        // the source filter is ADVISORY: the full-size LocalScene has no generated fillers (the original's water-road floor
        // under 05 reads as "no surface" although the converter's plumb finds it), so a "flight" verdict is printed, not applied
        let _ = src_ok;
        if a_ok && b_bad { n_bad += 1; if run_start.is_none() { run_start = Some(i); } }
        else if let Some(rs) = run_start.take() { if i - rs >= 2 { flush(rs, i - 1); } }
    }
    if let Some(rs) = run_start { flush(rs, pts.len() - 1); }
    let a_air = ga.iter().filter(|g| g.map(|g| g.0 > 1.5).unwrap_or(true)).count();
    println!("{}: {} samples, A airborne/unsupported {} ({:.0} %), A-supported-but-B-missing {}", Path::new(&maps[1]).file_name().unwrap().to_string_lossy(), pts.len(), a_air, 100.0 * a_air as f32 / pts.len().max(1) as f32, n_bad);
}

/// `leg-plot MAP.Map.Gbx --gates deck.json --author-line author.json (--leg K | --s A:B) [--chains rows.tsv]... --out X.png [--ppm 2]`
/// One picture of a leg: top-down surfaces by physics (walls dark), the human line by speed, the chains by death cause
/// with a marker where each dies, the gates; below it the side elevation along the human's arc length (surface under the
/// line, the human's height, the chains' heights) and the speeds. Coordinator 15:56Z.
fn cmd_leg_plot(args: &[String]) {
    use tmplan::plot::*;
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates deck.json"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let ap = flag(args, "--author-line").unwrap_or_else(|| die("--author-line author.json"));
    let txt = std::fs::read_to_string(&ap).unwrap_or_else(|e| die(&format!("{ap}: {e}")));
    let k = "\"pts\": [";
    let i = txt.find(k).unwrap_or_else(|| die("no pts"));
    let rest = &txt[i + k.len()..];
    let end = rest.find("]]").map(|e| e + 1).unwrap_or(rest.len());
    let flat: Vec<f32> = rest[..end].split(|c: char| c == ',' || c == '[' || c == ']').filter_map(|x| x.trim().parse::<f32>().ok()).collect();
    let line: Vec<[f32; 3]> = flat.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect();
    // arc length and speed (100 ms samples) along the human line
    let mut s = vec![0.0f32];
    for i in 1..line.len() { let (a, b) = (line[i - 1], line[i]); s.push(s[i - 1] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()); }
    let vh: Vec<f32> = (0..line.len()).map(|i| if i == 0 { 0.0 } else { (s[i] - s[i - 1]) * 10.0 }).collect();
    // the gate crossings along the line (same rule as the route files: deepest in-range run, finish = last run)
    let mut groups: Vec<u32> = gates.gates.iter().filter(|g| g.group != u32::MAX).map(|g| g.group).collect();
    groups.sort_unstable();
    groups.dedup();
    let mut crossings: Vec<(usize, u32)> = Vec::new();
    for grp in &groups {
        let is_fin = gates.gates.iter().any(|x| x.group == *grp && matches!(x.kind, tmroute::gates::WpKind::Finish));
        let (mut best_run, mut run_best, mut in_run): (Option<(usize, f32)>, Option<(usize, f32)>, bool) = (None, None, false);
        for (li, p) in line.iter().enumerate() {
            let dmin = gates.gates.iter().filter(|x| x.group == *grp).filter(|x| { let dy = p[1] - x.centre[1]; dy >= -9.0 && dy <= 3.0 }).map(|x| ((x.centre[0] - p[0]).powi(2) + (x.centre[2] - p[2]).powi(2)).sqrt() - x.half_width).fold(f32::INFINITY, f32::min);
            if dmin <= 6.0 { if !in_run { in_run = true; run_best = None; } if run_best.map(|(_, d)| dmin < d).unwrap_or(true) { run_best = Some((li, dmin)); } }
            else if in_run { in_run = false; if is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true) { best_run = run_best; } }
        }
        if in_run && (is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true)) { best_run = run_best; }
        if let Some((li, _)) = best_run { crossings.push((li, *grp)); }
    }
    crossings.sort();
    // the window: --leg K (gate K-1 → K in the human order; K = 0 is spawn → first gate) or --s A:B
    let (i0, i1, title) = if let Some(k) = flag(args, "--leg").and_then(|v| v.parse::<usize>().ok()) {
        let a = if k == 0 { 0 } else { crossings.get(k - 1).map(|c| c.0).unwrap_or(0) };
        let b = crossings.get(k).map(|c| c.0).unwrap_or(line.len() - 1);
        (a, b, format!("LEG {k}: {} > GROUP {}", if k == 0 { "SPAWN".to_string() } else { format!("GROUP {}", crossings[k - 1].1) }, crossings.get(k).map(|c| c.1.to_string()).unwrap_or("END".into())))
    } else if let Some(w) = flag(args, "--s") {
        let mut it = w.split(':');
        let a: f32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        let b: f32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(f32::INFINITY);
        let ia = s.iter().position(|&v| v >= a).unwrap_or(0);
        let ib = s.iter().position(|&v| v >= b).unwrap_or(line.len() - 1);
        (ia, ib, format!("S {a:.0}-{b:.0}"))
    } else { die("--leg K or --s A:B") };
    let seg = &line[i0..=i1];
    let mut chains: Vec<Chain> = args.iter().enumerate().filter(|(_, a)| *a == "--chains").filter_map(|(i, _)| args.get(i + 1)).flat_map(|p| read_chains(p).unwrap_or_else(|e| die(&e))).collect();
    // GEN's archive dumps (gen/plots/FORMAT.md): best-rows (tick race_ms x y z speed vy cps) = one chain "best";
    // deaths (cause x y z v s macro) = one marker each; cells (x y z v cps s macro) = alive end states, small dots
    let tsv = |p: &str| -> Vec<Vec<String>> { std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{p}: {e}"))).lines().skip(1).map(|l| l.split('\t').map(|x| x.trim().to_string()).collect()).filter(|f: &Vec<String>| f.len() >= 4).collect() };
    if let Some(p) = flag(args, "--gen-best") {
        let rows: Vec<ChainRow> = tsv(&p).iter().filter_map(|f| Some(ChainRow { t: f.get(1)?.parse::<f32>().ok()? / 1000.0, p: [f.get(2)?.parse().ok()?, f.get(3)?.parse().ok()?, f.get(4)?.parse().ok()?], v: f.get(5)?.parse().ok()? })).collect();
        let rows: Vec<ChainRow> = rows.into_iter().enumerate().filter(|(i, _)| i % 10 == 0).map(|(_, r)| r).collect();
        chains.push(Chain { id: "best".into(), rows, cause: "alive".into() });
    }
    if let Some(p) = flag(args, "--gen-deaths") {
        for f in tsv(&p) { if let (Ok(x), Ok(y), Ok(z), Ok(v)) = (f[1].parse::<f32>(), f[2].parse::<f32>(), f[3].parse::<f32>(), f.get(4).map(|s| s.parse::<f32>()).unwrap_or(Ok(0.0))) { chains.push(Chain { id: format!("death{}", chains.len()), rows: vec![ChainRow { t: 0.0, p: [x, y, z], v }], cause: f[0].to_lowercase() }); } }
    }
    let mut cells: Vec<([f32; 3], f32)> = Vec::new();
    if let Some(p) = flag(args, "--gen-cells") {
        for f in tsv(&p) { if let (Ok(x), Ok(y), Ok(z), Ok(v)) = (f[0].parse::<f32>(), f[1].parse::<f32>(), f[2].parse::<f32>(), f[3].parse::<f32>()) { cells.push(([x, y, z], v)); } }
    }
    // clip every chain to the leg: rows within 30 m (XZ) of a leg sample (a whole-lap replay would frame the whole map)
    let near_leg = |p: [f32; 3]| -> bool { seg.iter().any(|q| (q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2) < 900.0) };
    for c in chains.iter_mut() { if c.rows.len() > 1 { c.rows.retain(|r| near_leg(r.p)); } }
    chains.retain(|c| !c.rows.is_empty() && (c.rows.len() > 1 || near_leg(c.rows[0].p)));
    cells.retain(|(p, _)| near_leg(*p));
    // bounding box: the leg ± 25 m, plus the chains
    let (mut xmin, mut xmax, mut zmin, mut zmax) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    let mut ymax = f32::NEG_INFINITY;
    for p in seg.iter().chain(chains.iter().flat_map(|c| c.rows.iter().map(|r| &r.p))) { xmin = xmin.min(p[0]); xmax = xmax.max(p[0]); zmin = zmin.min(p[2]); zmax = zmax.max(p[2]); ymax = ymax.max(p[1]); }
    let margin = 25.0;
    xmin -= margin; xmax += margin; zmin -= margin; zmax += margin;
    let ppm: f32 = flag(args, "--ppm").and_then(|v| v.parse().ok()).unwrap_or(2.0);
    let (tw, th) = (((xmax - xmin) * ppm) as usize + 1, ((zmax - zmin) * ppm) as usize + 1);
    let (tw, th) = (tw.clamp(200, 2400), th.clamp(200, 2400));
    let ppm = ((tw as f32 - 1.0) / (xmax - xmin)).min((th as f32 - 1.0) / (zmax - zmin));
    let side_h = 320usize;
    let legend_h = 40usize;
    let mut cv = Canvas::new(tw.max(900), th + side_h + legend_h, [250, 250, 250]);
    // scene
    let paths = tmplan::pak_paths().unwrap_or_else(|e| die(&e));
    let mut store = mapgeom::store::DataStore::open(&paths, mapgeom::store::STADIUM_KEY).unwrap_or_else(|e| die(&e));
    let m = tmmaps::map::MapFile::load(Path::new(&map));
    let opts = mapgeom::local::BuildOpts { with_deco: true, with_baked: std::env::var("TMPLAN_BAKED").is_ok() || !tmroute::gates::is_tiny_map(&gates.map_uid, &gates.map_name), ..Default::default() };
    let scene = mapgeom::local::LocalScene::build(&mut store, &m, gates.yoff, &opts);
    let ymin_leg = seg.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    // start the rays just above the leg (a terrain roof over a cavity — 20's deck pit — would otherwise hide it); --top Y overrides
    let top_y = flag(args, "--top").and_then(|v| v.parse::<f32>().ok()).unwrap_or(ymax + 6.0);
    // top-down: one downward ray per pixel; heights shade the colour (higher = lighter), walls (ny < 0.5) dark
    let to_px = |x: f32, z: f32| -> (f32, f32) { ((x - xmin) * ppm, (zmax - z) * ppm) };
    let mut ground_under: Vec<Option<f32>> = Vec::with_capacity(seg.len());
    for py in 0..th {
        for px in 0..tw {
            let x = xmin + px as f32 / ppm;
            let z = zmax - py as f32 / ppm;
            let mut o = [x, top_y, z];
            let mut col = [250u8, 250, 250];
            for _ in 0..3 {
                match scene.raycast(o, [0.0, -1.0, 0.0], top_y - (ymin_leg - 60.0), true) {
                    Some(h) if matches!(h.material_name, "Water" | "Sea" | "Lake" | "WaterSurface") => { o = [h.point[0], h.point[1] - 0.05, h.point[2]]; col = [200, 220, 245]; continue; }
                    Some(h) => {
                        let mut c = material_colour(h.material_name);
                        // height shading: ±25 m around the leg → ±25 % brightness
                        let sh = ((h.point[1] - (ymin_leg + ymax) * 0.5) / 25.0).clamp(-1.0, 1.0) * 0.25;
                        for k in 0..3 { c[k] = ((c[k] as f32) * (1.0 + sh)).clamp(0.0, 255.0) as u8; }
                        if h.normal[1].abs() < 0.5 { c = [60, 60, 70]; }
                        col = c;
                        break;
                    }
                    None => break,
                }
            }
            cv.set(px as i64, py as i64, col);
        }
    }
    // gates
    for g in &gates.gates {
        if g.group == u32::MAX { continue; }
        let (px, pz) = to_px(g.centre[0], g.centre[2]);
        let fin = matches!(g.kind, tmroute::gates::WpKind::Finish);
        cv.ring(px, pz, (g.half_width * ppm).max(4.0), if fin { [20, 120, 40] } else { [230, 30, 30] });
        cv.text(px as i64 + 4, pz as i64 - 12, &format!("G{}", g.group), [0, 0, 0], 1);
    }
    // human line (whole map faint, the leg bright by speed)
    for i in 1..line.len() { let (a, b) = (to_px(line[i - 1][0], line[i - 1][2]), to_px(line[i][0], line[i][2])); cv.line(a.0, a.1, b.0, b.1, [120, 120, 120], 1); }
    for i in (i0 + 1)..=i1 { let (a, b) = (to_px(line[i - 1][0], line[i - 1][2]), to_px(line[i][0], line[i][2])); cv.line(a.0, a.1, b.0, b.1, speed_colour(vh[i]), 3); }
    // --cp-states NN.json: vjeux's checkpoint-crossing states (launched-cp): position + velocity vector + speed, magenta
    // markers with an arrow (any JSON: every object with x,y,z and vx,vy,vz or speed is a state)
    let mut cp_states: Vec<([f32; 3], [f32; 3], f32, String)> = Vec::new();
    if let Some(p) = flag(args, "--cp-states") {
        let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| die(&format!("{p}: {e}")));
        let v: serde_json::Value = serde_json::from_str(&txt).unwrap_or_else(|e| die(&format!("{p}: {e}")));
        fn walk(v: &serde_json::Value, out: &mut Vec<([f32; 3], [f32; 3], f32, String)>, label: String) {
            let num = |o: &serde_json::Map<String, serde_json::Value>, k: &str| o.get(k).and_then(|x| x.as_f64()).map(|x| x as f32);
            match v {
                serde_json::Value::Object(o) => {
                    let pos = if let (Some(x), Some(y), Some(z)) = (num(o, "x"), num(o, "y"), num(o, "z")) { Some([x, y, z]) } else { o.get("pos").or(o.get("position")).and_then(|a| a.as_array()).filter(|a| a.len() == 3).map(|a| [a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32]) };
                    if let Some(pp) = pos {
                        let vel = if let (Some(x), Some(y), Some(z)) = (num(o, "vx"), num(o, "vy"), num(o, "vz")) { [x, y, z] } else { o.get("vel").or(o.get("velocity")).and_then(|a| a.as_array()).filter(|a| a.len() == 3).map(|a| [a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32]).unwrap_or([0.0; 3]) };
                        let sp = num(o, "speed").or(num(o, "v")).unwrap_or((vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt());
                        let lab = o.get("cp").or(o.get("checkpoint")).or(o.get("waypoint")).or(o.get("landmark")).map(|x| x.to_string().trim_matches('"').to_string()).unwrap_or(label.clone());
                        if o.get("approach").is_none() && o.get("samples").is_none() || o.contains_key("cp") || o.contains_key("landmark") { out.push((pp, vel, sp, lab)); }
                    }
                    for (k, x) in o { if k != "approach" && k != "samples" { walk(x, out, k.clone()); } }
                }
                serde_json::Value::Array(a) => { for (i, x) in a.iter().enumerate() { walk(x, out, format!("{label}{i}")); } }
                _ => {}
            }
        }
        walk(&v, &mut cp_states, String::new());
        cp_states.retain(|(p, _, _, _)| near_leg(*p));
        eprintln!("  {} vjeux CP states in the window", cp_states.len());
    }
    // alive cells: small dots by speed
    for (p, v) in &cells { let (px, pz) = to_px(p[0], p[2]); cv.disc(px, pz, 1.5, speed_colour(*v)); }
    // chains
    for c in &chains {
        let col = cause_colour(&c.cause);
        for i in 1..c.rows.len() { let (a, b) = (to_px(c.rows[i - 1].p[0], c.rows[i - 1].p[2]), to_px(c.rows[i].p[0], c.rows[i].p[2])); cv.line(a.0, a.1, b.0, b.1, col, 2); }
        if let Some(r) = c.rows.last() { let (px, pz) = to_px(r.p[0], r.p[2]); if c.rows.len() > 1 { cv.disc(px, pz, 5.0, col); cv.ring(px, pz, 7.0, [0, 0, 0]); } else { cv.disc(px, pz, 3.0, col); } }
    }
    // vjeux CP states: magenta disc + velocity arrow (1 s of travel) + label
    for (p, vel, sp, lab) in &cp_states {
        let (px, pz) = to_px(p[0], p[2]);
        let (qx, qz) = to_px(p[0] + vel[0], p[2] + vel[2]);
        cv.line(px, pz, qx, qz, [200, 0, 200], 3);
        cv.disc(px, pz, 6.0, [200, 0, 200]);
        cv.ring(px, pz, 8.0, [255, 255, 255]);
        cv.text(px as i64 + 9, pz as i64 + 4, &format!("V{lab} {sp:.0}", ), [120, 0, 120], 1);
    }
    // ground under the human line (for the elevation)
    for p in seg {
        let mut o = [p[0], p[1] + 0.3, p[2]];
        let mut found = None;
        for _ in 0..3 {
            match scene.raycast(o, [0.0, -1.0, 0.0], 40.0, true) {
                Some(h) if matches!(h.material_name, "Water" | "Sea" | "Lake" | "WaterSurface") => { o = [h.point[0], h.point[1] - 0.05, h.point[2]]; }
                Some(h) => { found = Some(h.point[1]); break; }
                None => break,
            }
        }
        ground_under.push(found);
    }
    // side elevation: x = s along the leg, y = height; chains projected onto the leg by nearest sample
    let (sx0, sx1) = (s[i0], s[i1]);
    let panel_y0 = th as i64 + 10;
    let panel_h = (side_h - 20) as i64;
    let elev_h = (panel_h as f32 * 0.62) as i64;
    let spd_y0 = panel_y0 + elev_h + 12;
    let spd_h = panel_h - elev_h - 12;
    cv.rect(0, th as i64, cv.w as i64 - 1, th as i64 + side_h as i64 + legend_h as i64 - 1, [255, 255, 255]);
    let all_y: Vec<f32> = seg.iter().map(|p| p[1]).chain(ground_under.iter().filter_map(|g| *g)).chain(chains.iter().flat_map(|c| c.rows.iter().map(|r| r.p[1]))).collect();
    let (ymn, ymx) = (all_y.iter().cloned().fold(f32::INFINITY, f32::min) - 3.0, all_y.iter().cloned().fold(f32::NEG_INFINITY, f32::max) + 3.0);
    let w = cv.w as f32 - 80.0;
    let sx = |sv: f32| -> f32 { 60.0 + (sv - sx0) / (sx1 - sx0).max(1.0) * w };
    let ey = |y: f32| -> f32 { panel_y0 as f32 + (1.0 - (y - ymn) / (ymx - ymn).max(1.0)) * elev_h as f32 };
    let vy = |v: f32| -> f32 { spd_y0 as f32 + (1.0 - (v / 90.0).clamp(0.0, 1.0)) * spd_h as f32 };
    // axes + labels
    cv.line(60.0, panel_y0 as f32, 60.0, (panel_y0 + elev_h) as f32, [0, 0, 0], 1);
    cv.line(60.0, (panel_y0 + elev_h) as f32, 60.0 + w, (panel_y0 + elev_h) as f32, [0, 0, 0], 1);
    cv.text(2, panel_y0, &format!("{ymx:.0}M"), [0, 0, 0], 1);
    cv.text(2, panel_y0 + elev_h - 8, &format!("{ymn:.0}M"), [0, 0, 0], 1);
    cv.line(60.0, spd_y0 as f32, 60.0, (spd_y0 + spd_h) as f32, [0, 0, 0], 1);
    cv.line(60.0, (spd_y0 + spd_h) as f32, 60.0 + w, (spd_y0 + spd_h) as f32, [0, 0, 0], 1);
    cv.text(2, spd_y0, "90 M/S", [0, 0, 0], 1);
    cv.text(2, spd_y0 + spd_h - 8, "0", [0, 0, 0], 1);
    for k in 0..=8 { let sv = sx0 + (sx1 - sx0) * k as f32 / 8.0; cv.text(sx(sv) as i64 - 8, spd_y0 + spd_h + 3, &format!("S{sv:.0}"), [0, 0, 0], 1); cv.line(sx(sv), (panel_y0 + elev_h) as f32 - 3.0, sx(sv), (panel_y0 + elev_h) as f32 + 3.0, [0, 0, 0], 1); }
    // surface under the line
    for i in 1..seg.len() { if let (Some(a), Some(b)) = (ground_under[i - 1], ground_under[i]) { cv.line(sx(s[i0 + i - 1]), ey(a), sx(s[i0 + i]), ey(b), [140, 140, 140], 3); } }
    // human height (by speed) and speed
    for i in (i0 + 1)..=i1 { cv.line(sx(s[i - 1]), ey(line[i - 1][1]), sx(s[i]), ey(line[i][1]), speed_colour(vh[i]), 2); cv.line(sx(s[i - 1]), vy(vh[i - 1]), sx(s[i]), vy(vh[i]), [0, 0, 0], 2); }
    // gate crossings as vertical ticks
    for (li, grp) in &crossings { if *li >= i0 && *li <= i1 { cv.line(sx(s[*li]), panel_y0 as f32, sx(s[*li]), (spd_y0 + spd_h) as f32, [230, 30, 30], 1); cv.text(sx(s[*li]) as i64 + 3, panel_y0, &format!("G{grp}"), [230, 30, 30], 1); } }
    // vjeux CP states on the elevation and speed panels
    for (p, _, sp, _) in &cp_states {
        let mut best = (f32::INFINITY, i0); for (j, q) in seg.iter().enumerate() { let d = (q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2); if d < best.0 { best = (d, i0 + j); } }
        let sv = s[best.1];
        cv.disc(sx(sv), ey(p[1]), 5.0, [200, 0, 200]); cv.disc(sx(sv), vy(*sp), 5.0, [200, 0, 200]);
    }
    // chains: project each row to the nearest leg sample (XZ) → s
    for c in &chains {
        let col = cause_colour(&c.cause);
        let proj = |p: [f32; 3]| -> f32 { let mut best = (f32::INFINITY, i0); for (j, q) in seg.iter().enumerate() { let d = (q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2); if d < best.0 { best = (d, i0 + j); } } s[best.1] };
        let mut prev: Option<(f32, f32, f32)> = None;
        for r in &c.rows {
            let sv = proj(r.p);
            if let Some((ps, py, pv)) = prev { if (sv - ps).abs() < 40.0 { cv.line(sx(ps), ey(py), sx(sv), ey(r.p[1]), col, 1); cv.line(sx(ps), vy(pv), sx(sv), vy(r.v), col, 1); } }
            prev = Some((sv, r.p[1], r.v));
        }
        if let Some(r) = c.rows.last() { let sv = proj(r.p); let rr = if c.rows.len() > 1 { 4.0 } else { 2.5 }; cv.disc(sx(sv), ey(r.p[1]), rr, col); cv.disc(sx(sv), vy(r.v), rr, col); }
    }
    // legend
    let ly = (th + side_h) as i64 + 8;
    let mut lx = 10i64;
    for (name, col) in [("HUMAN BY SPEED", speed_colour(40.0)), ("FELL", cause_colour("fell")), ("OFFROUTE", cause_colour("offroute")), ("STOPPED", cause_colour("stopped")), ("ALIVE", cause_colour("alive")), ("FINISH", cause_colour("finish")), ("WALL", [60, 60, 70]), ("GATE", [230, 30, 30]), ("VJEUX CP STATE", [200, 0, 200])] {
        cv.rect(lx, ly, lx + 14, ly + 10, col);
        cv.text(lx + 18, ly + 2, name, [0, 0, 0], 1);
        lx += 18 + 6 * name.len() as i64 + 16;
    }
    cv.text(10, 4, &format!("{} {title} S {sx0:.0}-{sx1:.0} ({} CHAINS)", gates.map_name.to_ascii_uppercase(), chains.len()), [0, 0, 0], 2);
    let out = flag(args, "--out").unwrap_or_else(|| die("--out X.png"));
    cv.write_png(Path::new(&out)).unwrap_or_else(|e| die(&e.to_string()));
    // a one-line reading for the caller
    let g_min = ground_under.iter().filter_map(|g| *g).fold(f32::INFINITY, f32::min);
    let g_max = ground_under.iter().filter_map(|g| *g).fold(f32::NEG_INFINITY, f32::max);
    let air = ground_under.iter().zip(seg.iter()).filter(|(g, p)| g.map(|g| p[1] - g > 1.5).unwrap_or(true)).count();
    let vmin = vh[i0 + 1..=i1].iter().cloned().fold(f32::INFINITY, f32::min);
    let vmax = vh[i0 + 1..=i1].iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    println!("{out}: {title}, s {sx0:.0}–{sx1:.0} ({:.0} m), human {vmin:.0}–{vmax:.0} m/s, height {:.1}–{:.1} m, surface under the line {g_min:.1}–{g_max:.1} m, airborne samples {air}/{}, chains {} ({})", sx1 - sx0, seg.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min), seg.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max), seg.len(), chains.len(), { let mut m: BTreeMap<&str, usize> = BTreeMap::new(); for c in &chains { *m.entry(c.cause.as_str()).or_default() += 1; } m.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", ") });
}

/// `arrival-bands --gates deck.json --route ROUTE.json --author-line author.json [--lcp NN.csv] [--build B] --out X.json`
/// Per gate in the route's order: the HUMAN's crossing state (position in the credit plane, heading, speed) — from vjeux's
/// LaunchedCP crossing where one exists (landmark = map waypoint), else from the author line's crossing sample — plus a band
/// (lateral / height / speed / heading tolerances) and the credit-plane definition. For the player's leg-wise PPO (03:30Z).
fn cmd_arrival_bands(args: &[String]) {
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates deck.json"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let rp = flag(args, "--route").unwrap_or_else(|| die("--route route.json"));
    let rtxt = std::fs::read_to_string(&rp).unwrap_or_else(|e| die(&format!("{rp}: {e}")));
    let rv: serde_json::Value = serde_json::from_str(&rtxt).unwrap_or_else(|e| die(&format!("{rp}: {e}")));
    // the route file is an array: [gate…, {route meta}] (see cmd_road_centreline) or an object with "gates"
    let (gate_rows, meta): (Vec<serde_json::Value>, serde_json::Value) = match &rv {
        serde_json::Value::Array(a) => { let mut g = a.clone(); let m = g.iter().position(|x| x.get("gate_order").is_some()).map(|i| g.remove(i)).unwrap_or(serde_json::Value::Null); (g.into_iter().filter(|x| x.get("map_waypoint").is_some()).collect(), m) }
        serde_json::Value::Object(o) => (o.get("gates").and_then(|g| g.as_array()).cloned().unwrap_or_default(), o.get("route").cloned().unwrap_or(serde_json::Value::Null)),
        _ => die("route: unexpected json"),
    };
    let order_wp: Vec<u32> = gate_rows.iter().filter_map(|g| g.get("map_waypoint").and_then(|w| w.as_u64()).map(|w| w as u32)).collect();
    // author line + arc length + speed
    let ap = flag(args, "--author-line").unwrap_or_else(|| die("--author-line author.json"));
    let txt = std::fs::read_to_string(&ap).unwrap_or_else(|e| die(&format!("{ap}: {e}")));
    let k = "\"pts\": [";
    let i = txt.find(k).or_else(|| txt.find("\"pts\":[").map(|i| i - 1)).unwrap_or_else(|| die("no pts"));
    let rest = &txt[i + k.len()..];
    let end = rest.find("]]").map(|e| e + 1).unwrap_or(rest.len());
    let flat: Vec<f32> = rest[..end].split(|c: char| c == ',' || c == '[' || c == ']').filter_map(|x| x.trim().parse::<f32>().ok()).collect();
    let line: Vec<[f32; 3]> = flat.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect();
    let vh: Vec<f32> = (0..line.len()).map(|i| if i + 1 < line.len() { let (a, b) = (line[i], line[i + 1]); ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt() * 10.0 } else { 0.0 }).collect();
    // vjeux's LaunchedCP crossings: entry,landmark,kind,time_ms,t_window_ms,x,y,z,qx,qy,qz,qw,vx,vy,vz,speed_fwd_ms,...
    let mut lcp: std::collections::BTreeMap<u32, (f32, [f32; 3], [f32; 3], f32)> = std::collections::BTreeMap::new();
    if let Some(lp) = flag(args, "--lcp") {
        let t = std::fs::read_to_string(&lp).unwrap_or_else(|e| die(&format!("{lp}: {e}")));
        for l in t.lines().skip(1) {
            let f: Vec<&str> = l.split(',').collect();
            if f.len() < 16 || f[2] != "crossing" { continue; }
            let lm: u32 = match f[1].parse() { Ok(v) => v, Err(_) => continue };
            let p = [f[5].parse().unwrap_or(0.0), f[6].parse().unwrap_or(0.0), f[7].parse().unwrap_or(0.0)];
            let v = [f[12].parse().unwrap_or(0.0), f[13].parse().unwrap_or(0.0), f[14].parse().unwrap_or(0.0)];
            let sp: f32 = f[15].parse().unwrap_or(0.0);
            lcp.entry(lm).or_insert((f[3].parse::<f32>().unwrap_or(0.0) / 1000.0, p, v, sp));
        }
    }
    let heading_of = |v: [f32; 3]| -> (Vec<f32>, f32) { let n = (v[0] * v[0] + v[2] * v[2]).sqrt().max(1e-6); (vec![v[0] / n, v[2] / n], (v[0] / n).atan2(v[2] / n).to_degrees()) };
    let mut out_gates = Vec::new();
    let mut cursor = 0usize;
    for (idx, wp) in order_wp.iter().enumerate() {
        let g = match gates.gates.iter().find(|x| x.waypoint == *wp) { Some(g) => g, None => continue };
        let grp = g.group;
        let is_fin = matches!(g.kind, tmroute::gates::WpKind::Finish);
        // author crossing: deepest in-range run after the previous crossing (same rule as the route files)
        let (mut best_run, mut run_best, mut in_run): (Option<(usize, f32)>, Option<(usize, f32)>, bool) = (None, None, false);
        for (li, p) in line.iter().enumerate().skip(cursor) {
            let dmin = gates.gates.iter().filter(|x| x.group == grp).filter(|x| { let dy = p[1] - x.centre[1]; dy >= -9.0 && dy <= 3.0 }).map(|x| ((x.centre[0] - p[0]).powi(2) + (x.centre[2] - p[2]).powi(2)).sqrt() - x.half_width).fold(f32::INFINITY, f32::min);
            if dmin <= 6.0 { if !in_run { in_run = true; run_best = None; } if run_best.map(|(_, d)| dmin < d).unwrap_or(true) { run_best = Some((li, dmin)); } }
            else if in_run { in_run = false; if is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true) { best_run = run_best; } if !is_fin && best_run.is_some() { /* keep scanning for a deeper run only if none yet */ } }
        }
        if in_run && (is_fin || best_run.map(|(_, d)| run_best.map(|(_, rd)| rd < d).unwrap_or(false)).unwrap_or(true)) { best_run = run_best; }
        let author = best_run.map(|(li, _)| {
            cursor = li + 1;
            let p = line[li];
            // heading = the velocity direction over ±0.3 s (7 samples), not two neighbours
            let (a, b) = (li.saturating_sub(3), (li + 3).min(line.len() - 1));
            let (h, hdg) = heading_of([line[b][0] - line[a][0], 0.0, line[b][2] - line[a][2]]);
            let sp = vh[li.saturating_sub(1)..=li.min(vh.len() - 1)].iter().sum::<f32>() / (li - li.saturating_sub(1) + 1) as f32;
            serde_json::json!({"source": "author-line", "sample": li, "t_s": li as f32 * 0.1, "pos": [p[0], p[1], p[2]], "heading_xz": h, "heading_deg": hdg, "speed_mps": sp})
        });
        let vj = lcp.get(wp).map(|(t, p, v, sp)| { let (h, hdg) = heading_of(*v); serde_json::json!({"source": "vjeux-launched-cp", "t_s": t, "pos": [p[0], p[1], p[2]], "vel": [v[0], v[1], v[2]], "heading_xz": h, "heading_deg": hdg, "speed_mps": sp.abs(), "speed_fwd_mps": sp}) });
        let human = vj.clone().or(author.clone()).unwrap_or(serde_json::Value::Null);
        let speed = human.get("speed_mps").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
        let pos = human.get("pos").and_then(|p| p.as_array()).map(|a| [a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32]);
        // a group can hold several records (linked rings, stacked tower gates): the credit plane is the record the human crossed
        let g = match pos { Some(p) => gates.gates.iter().filter(|x| x.group == grp).min_by(|a, b| { let da = (a.centre[0] - p[0]).powi(2) + (a.centre[1] - p[1]).powi(2) + (a.centre[2] - p[2]).powi(2); let db = (b.centre[0] - p[0]).powi(2) + (b.centre[1] - p[1]).powi(2) + (b.centre[2] - p[2]).powi(2); da.partial_cmp(&db).unwrap() }).unwrap_or(g), None => g };
        // lateral offset of the human in the plane: signed distance along the plane's in-plane axis (normal × up)
        let lat = pos.map(|p| { let n = g.normal; let ax = [n[2], 0.0, -n[0]]; (p[0] - g.centre[0]) * ax[0] + (p[2] - g.centre[2]) * ax[2] });
        // the deck (where the car sits when it credits) is 4 m under the ring centre on every ring model (F25 measurements)
        let dy = pos.map(|p| p[1] - g.centre[1]); // relative to the credit-plane CENTRE (ring heights differ per model; the human's own offset is the reference)
        // speed band from the AUTHOR when vjeux crossed slowly (a struggle, e.g. 22 wp3 at 6 m/s): his position is exact, his speed is not a target
        let author_speed = author.as_ref().and_then(|a| a.get("speed_mps")).and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
        let (band_speed, speed_source) = if vj.is_some() && author_speed > 0.0 && speed < 0.6 * author_speed { (author_speed, "author-line (vjeux crossed at a struggle speed)") } else { (speed, if vj.is_some() { "vjeux-launched-cp" } else { "author-line" }) };
        let plane = serde_json::json!({"centre": g.centre, "normal": g.normal, "half_width": g.half_width, "half_height": g.half_height,  "credit_offset_m": g.credit_offset_m, "kind": format!("{:?}", g.kind), "model": g.model});
        // a hairpin apex (the author's heading turns > 120° within ±3 s of the crossing) or an angled crossing (> 35° off the plane
        // normal) gets a wide heading band; a crossing anywhere in the ring credits, the band is guidance
        let hdg_c = human.get("heading_deg").and_then(|h| h.as_f64()).unwrap_or(0.0) as f32;
        let turn = author.as_ref().and_then(|a| a.get("sample")).and_then(|s| s.as_u64()).map(|li| { let li = li as usize; let (a0, a1) = (li.saturating_sub(30), li.saturating_sub(15)); let (b0, b1) = ((li + 15).min(line.len() - 1), (li + 30).min(line.len() - 1)); let h0 = (line[a1][0] - line[a0][0]).atan2(line[a1][2] - line[a0][2]).to_degrees(); let h1 = (line[b1][0] - line[b0][0]).atan2(line[b1][2] - line[b0][2]).to_degrees(); let mut d = (h1 - h0).abs(); if d > 180.0 { d = 360.0 - d; } d }).unwrap_or(0.0);
        let hx = hdg_c.to_radians().sin(); let hz = hdg_c.to_radians().cos();
        let incidence = { let dot = (hx * g.normal[0] + hz * g.normal[2]).abs().clamp(0.0, 1.0); dot.acos().to_degrees() };
        let hdg_tol = if turn > 80.0 || incidence > 35.0 { 60.0 } else { 20.0 };
        let band = serde_json::json!({
            "lateral_m": {"centre": lat.unwrap_or(0.0), "tol": (g.half_width - 1.0).max(2.0).max(lat.map(|l| l.abs() + 1.0).unwrap_or(0.0)), "note": "signed in-plane offset from the credit-plane centre, axis = normal × up; a crossing anywhere within ± half_width credits"},
            "height_rel_centre_m": {"centre": dy.unwrap_or(0.0), "lo": dy.unwrap_or(0.0) - 1.5, "hi": dy.unwrap_or(0.0) + 3.0, "note": "car y minus credit-plane centre y at the human's crossing; the human sits on the deck, so this is the deck offset of that ring"},
            "speed_mps": {"centre": band_speed, "lo": (band_speed * 0.75).round(), "hi": (band_speed * 1.15).round(), "source": speed_source},
            "heading_deg": {"centre": hdg_c, "tol": hdg_tol, "turn_within_3s_deg": turn, "incidence_to_plane_normal_deg": incidence, "crossing_dir_vs_normal": if hx * g.normal[0] + hz * g.normal[2] >= 0.0 { "along" } else { "against" }, "note": "a crossing anywhere within the ring credits in either direction; the band is the human's heading — wide (60) at hairpin apexes and angled crossings"}
        });
        out_gates.push(serde_json::json!({"idx": idx, "map_waypoint": wp, "group": grp, "s_on_route": gate_rows.get(idx).and_then(|r| r.get("s")).cloned().unwrap_or(serde_json::Value::Null), "credit_plane": plane, "human": human, "author": author, "vjeux": vj, "band": band}));
    }
    let out = flag(args, "--out").unwrap_or_else(|| die("--out X.json"));
    let doc = serde_json::json!({
        "schema": "arrival-bands/1",
        "map_name": gates.map_name, "map_uid": gates.map_uid, "build": flag(args, "--build").unwrap_or_default(),
        "gates_file": gp, "route_file": rp, "author_line": ap, "launched_cp": flag(args, "--lcp").unwrap_or_default(),
        "gate_order_map_waypoint": order_wp, "route_meta": meta,
        "frame": "tiny map frame, metres; heading_xz = unit (x, z) of travel, heading_deg = atan2(x, z) in degrees",
        "gates": out_gates,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap()).unwrap_or_else(|e| die(&e.to_string()));
    let nv = doc["gates"].as_array().unwrap().iter().filter(|g| !g["vjeux"].is_null()).count();
    println!("{out}: {} gates, {} from vjeux's LaunchedCP, {} from the author line", order_wp.len(), nv, order_wp.len() - nv);
}
