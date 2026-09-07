//! `tmplan` CLI.
//!
//!   tmplan plan MAP.Map.Gbx --gates gates.json [--out-dir DIR] [--top-k 3] [--beam 4000]
//!                            [--flight none|ballistic|drag] [--grid track|deco] [--time speed|cost] [--source NAME] [--matrix] [--quiet]
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
        Some(x) => die(&format!("--flight {x}: none|ballistic|drag")),
    }
}

fn order_str(nodes: &Nodes, gates: &tmroute::gates::GatesFile, visit: &[usize]) -> (String, String) {
    let groups: Vec<String> = visit.iter().skip(1).map(|&n| nodes.groups[n].to_string()).collect();
    let wps: Vec<String> = visit.iter().skip(1).map(|&n| gates.group_rep(nodes.groups[n]).unwrap().waypoint.to_string()).collect();
    (groups.join(","), wps.join(","))
}

fn load(args: &[String]) -> (String, tmroute::gates::GatesFile, SurfaceModel, Nodes, Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Option<(Vec<f32>, Vec<u32>)>>) {
    let map = args.iter().find(|a| a.ends_with(".Map.Gbx")).cloned().unwrap_or_else(|| die("MAP.Map.Gbx required"));
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates gates.json required"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let quiet = has(args, "--quiet");
    let deco = flag(args, "--grid").map_or(false, |g| g == "deco");
    let (surf, nodes) = SurfaceModel::build(Path::new(&map), &gates, !quiet, deco).unwrap_or_else(|e| die(&e));
    for n in &surf.notes {
        println!("  note: {n}");
    }
    let (d, len, fields) = surf.distance_matrix(&nodes);
    (map, gates, surf, nodes, d, len, fields)
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
    let (_map, gates, surf, nodes, d, len, fields) = load(args);
    if has(args, "--matrix") {
        print_matrix(&nodes, &d, "surface-graph COST (off-road 20x; inf = no path)");
        print_matrix(&nodes, &len, "surface-graph path LENGTH (m)");
    }
    let flight = flight_of(flag(args, "--flight"));
    let est = Geometric { time_model: tm(args), d: &d, len: &len, nodes: &nodes, flight, surface: Some(&surf) };
    let width: usize = flag(args, "--beam").and_then(|s| s.parse().ok()).unwrap_or(4000);
    let top_k: usize = flag(args, "--top-k").and_then(|s| s.parse().ok()).unwrap_or(3);
    let plans = planner::beam(&nodes, &est, width, top_k, StateBucket::of_speed(0.0));
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
        println!("  rank {k}: predicted {}  P(reach) {:.3}  length {:.0} m  flight legs {}  groups [{}]  waypoints [{}]", io::secs(p.total_ms), p.p_reach, len, flights, g, w);
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
    let (_map, gates, surf, nodes, d, len, _fields) = load(args);
    print_matrix(&nodes, &d, "surface-graph COST (off-road 20x; inf = no path)");
    print_matrix(&nodes, &len, "surface-graph path LENGTH (m)");
    let flight = flight_of(flag(args, "--flight").or(Some("ballistic".into())));
    let est = Geometric { time_model: tm(args), d: &d, len: &len, nodes: &nodes, flight, surface: Some(&surf) };
    let plans = planner::beam(&nodes, &est, 4000, 1, StateBucket::of_speed(0.0));
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
