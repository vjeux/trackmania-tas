//! `tmroute` CLI.
//!
//!   tmroute show ROUTE.json                       gate order, per-leg bands, status (seconds with a decimal)
//!   tmroute agree A.json B.json [--gates G.json]  order agreement: exact + Kendall tau (by checkpoint group when --gates)
//!   tmroute gates MAP.Map.Gbx --out gates.json [--orient-from PACK.pack.json ROUTE.route.json]
//!                                                 gates.json (INTERFACES §3) with the declared-count control
//!   tmroute from-cartographer PACK.pack.json ROUTE.route.json --gates gates.json --out route.json
//!   tmroute human-orders --gates gates.json --out human-orders.tsv GHOST...
//!   tmroute consensus --gates gates.json --orders human-orders.tsv --out route.json [--gates-out gates.json] [--certified-by BOX] GHOST...
//!   tmroute index ROUTES_DIR [--names gates_dir]  rebuild routes.tsv
//!   tmroute validate ROUTE.json

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tmroute::gates::GatesFile;
use tmroute::human;
use tmroute::io;
use tmroute::metrics;
use tmroute::types::*;

fn die(msg: &str) -> ! {
    eprintln!("tmroute: {msg}");
    std::process::exit(2)
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}
fn flag_n(args: &[String], name: &str, n: usize) -> Option<Vec<String>> {
    let i = args.iter().position(|a| a == name)?;
    let v: Vec<String> = args[i + 1..].iter().take(n).cloned().collect();
    (v.len() == n).then_some(v)
}
/// Positional arguments: everything not a flag or a flag's value(s).
fn positionals(args: &[String], flags1: &[&str], flags2: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if flags1.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if flags2.contains(&a.as_str()) {
            i += 3;
            continue;
        }
        if a.starts_with("--") {
            i += 1;
            continue;
        }
        out.push(a.clone());
        i += 1;
    }
    out
}

fn f3(v: [f32; 3]) -> String {
    format!("({:.2}, {:.2}, {:.2})", v[0], v[1], v[2])
}
fn band(v: [f32; 2], unit: &str) -> String {
    if v[0].is_nan() {
        "-".into()
    } else {
        format!("{:.1}..{:.1} {unit}", v[0], v[1])
    }
}

fn cmd_show(args: &[String]) {
    let p = args.first().unwrap_or_else(|| die("show: ROUTE.json"));
    let g = io::read_route(Path::new(p)).unwrap_or_else(|e| die(&e));
    let errs = g.validate();
    println!("{}  source {}  geom_version {}", g.map_uid, g.source, g.geom_version);
    println!("  centreline {} pts, {:.1} m, {} gates, spawn {} yaw {:.3}", g.pts.len(), g.s.last().copied().unwrap_or(0.0), g.gates.len(), f3(g.spawn), g.spawn_yaw);
    if let Some(r) = &g.route {
        let st = match &r.status {
            RouteStatus::Hypothesis => "HYPOTHESIS".to_string(),
            RouteStatus::Certified { ms, ghost_md5, oracle_box } => format!("CERTIFIED {} ghost {} on {}", io::secs(*ms), ghost_md5, oracle_box),
        };
        println!("  route v{} {} rank {}  predicted {}  {}", r.route_version, r.source, r.rank, io::secs(r.predicted_ms), st);
        println!("  order (map waypoints, finish last): {}", r.gate_order.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" → "));
        println!("  produced_by: {}", r.produced_by);
    }
    if let Some(legs) = &g.legs {
        println!("  {:>3} {:>4} {:>8} {:>8} {:<8} {:<16} {:<28} {:>6} {:<16} {:>7} {:>9}  evidence", "leg", "wp", "s_start", "s_end", "conn", "speed", "heading (tol)", "", "height", "p_reach", "exp");
        for l in legs {
            let ev = match &l.evidence {
                LegEvidence::Human { runs, best_ms } => format!("Human runs={runs} best={}", io::secs(*best_ms)),
                LegEvidence::Rollout { reached, tried } => format!("Rollout {reached}/{tried}"),
                LegEvidence::Predicted => "Predicted".into(),
                LegEvidence::Driven { ms, ghost_md5 } => format!("Driven {} {}", io::secs(*ms), ghost_md5),
            };
            println!(
                "  {:>3} {:>4} {:>8.1} {:>8.1} {:<8} {:<16} {:<28} {:>6} {:<16} {:>7} {:>9}  {}",
                l.gate_idx,
                l.map_waypoint,
                l.s_start,
                l.s_end,
                format!("{:?}", l.connection),
                band(l.arrival_speed, "m/s"),
                f3(l.arrival_heading),
                if l.arrival_heading_tol.is_nan() { "-".to_string() } else { format!("±{:.0}°", l.arrival_heading_tol.to_degrees()) },
                band(l.arrival_height, "m"),
                if l.p_reach.is_nan() { "-".to_string() } else { format!("{:.2}", l.p_reach) },
                io::secs(l.expected_ms),
                ev
            );
        }
    }
    if errs.is_empty() {
        println!("  valid: yes");
    } else {
        println!("  valid: NO");
        for e in errs {
            println!("    - {e}");
        }
        std::process::exit(1);
    }
}

fn cmd_validate(args: &[String]) {
    let mut bad = 0;
    for p in args {
        match io::read_route(Path::new(p)) {
            Ok(g) => {
                let e = g.validate();
                if e.is_empty() {
                    println!("{p}: ok");
                } else {
                    bad += 1;
                    println!("{p}: INVALID");
                    for x in e {
                        println!("  - {x}");
                    }
                }
            }
            Err(e) => {
                bad += 1;
                println!("{p}: {e}");
            }
        }
    }
    if bad > 0 {
        std::process::exit(1);
    }
}

fn cmd_agree(args: &[String]) {
    let pos = positionals(args, &["--gates"], &[]);
    if pos.len() != 2 {
        die("agree A.json B.json [--gates gates.json]");
    }
    let a = io::read_route(Path::new(&pos[0])).unwrap_or_else(|e| die(&e));
    let b = io::read_route(Path::new(&pos[1])).unwrap_or_else(|e| die(&e));
    let (mut oa, mut ob) = (a.gate_order(), b.gate_order());
    let mut by = "waypoint";
    if let Some(gp) = flag(args, "--gates") {
        let g = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
        oa = metrics::to_groups(&oa, &g);
        ob = metrics::to_groups(&ob, &g);
        by = "checkpoint group";
    }
    let tau = metrics::kendall_tau(&oa, &ob);
    let (only_a, only_b) = metrics::symmetric_difference(&oa, &ob);
    let j = |v: &[u32]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    println!("A {}  {}", a.route.as_ref().map(|r| r.source.clone()).unwrap_or(a.source.clone()), j(&oa));
    println!("B {}  {}", b.route.as_ref().map(|r| r.source.clone()).unwrap_or(b.source.clone()), j(&ob));
    println!("by {by}: exact {}  kendall_tau {:.3}  shared {}  only_A [{}]  only_B [{}]", metrics::exact(&oa, &ob), tau, oa.iter().filter(|x| ob.contains(x)).count(), j(&only_a), j(&only_b));
}

fn cartographer_dirs(pack: &Path, route: &Path, gates: &GatesFile) -> BTreeMap<u32, [f32; 3]> {
    // Orient gates.json normals from the cartographer's tour tangents.
    let mut dirs = BTreeMap::new();
    let Ok(imp) = tmroute::cartographer::import(pack, route, gates, "orient") else { return dirs };
    for l in imp.geom.legs.unwrap_or_default() {
        let g = gates.by_waypoint(l.map_waypoint).map(|g| g.group).unwrap();
        for r in gates.gates_of_group(g) {
            dirs.insert(r.waypoint, l.arrival_heading);
        }
    }
    dirs
}

fn cmd_gates(args: &[String]) {
    let pos = positionals(args, &["--out", "--engine-flips"], &["--orient-from"]);
    let map = pos.first().unwrap_or_else(|| die("gates MAP.Map.Gbx --out gates.json [--orient-from PACK ROUTE] [--engine-flips flipped-normals.tsv]"));
    let prov = tmroute::provenance("tmroute gates");
    let mut g = tmroute::gates::build(Path::new(map), &prov).unwrap_or_else(|e| die(&e));
    let mut oriented = 0;
    // the GEN arm's engine-credited sign FIRST (it is relative to the placement normal) and it wins:
    // `orient` never touches an "engine" gate
    if let Some(fp) = flag(args, "--engine-flips") {
        let flips = tmroute::gates::read_flips(Path::new(&fp)).unwrap_or_else(|e| die(&e));
        if let Some(w) = flips.get(&g.map_uid) {
            let n = tmroute::gates::orient_engine(&mut g, w);
            oriented = g.gates.len() - 1;
            eprintln!("engine sign: {} gates, {n} flipped against placement", oriented);
        }
    }
    if let Some(v) = flag_n(args, "--orient-from", 2) {
        let dirs = cartographer_dirs(Path::new(&v[0]), Path::new(&v[1]), &g);
        oriented += tmroute::gates::orient(&mut g, &dirs, "cartographer");
    }
    let ctl = if g.control_ok() { "OK" } else { "FAIL" };
    println!(
        "{}\t{}\tdeclared {}\tcp_groups {}\tfinish_groups {}\tcontrol {}\tyoff {}\tresid {:+.1}\tgates {}\toriented {}",
        g.map_name, g.map_uid, g.declared_checkpoints, g.checkpoint_groups, g.finish_groups, ctl, g.yoff, g.yoff_residual, g.gates.len() - 1, oriented
    );
    for r in &g.gates {
        println!(
            "  wp {:>2} {:<10} grp {:>3} link {:>2} {:<32} c {} n {} hw {:>4.1} hh {:.1} {} {}",
            r.waypoint,
            format!("{:?}", r.kind),
            if r.group == u32::MAX { "-".to_string() } else { r.group.to_string() },
            r.link_order,
            r.model,
            f3(r.centre),
            f3(r.normal),
            r.half_width,
            r.half_height,
            if r.from_item { "item" } else { "block" },
            r.normal_source
        );
    }
    for s in &g.specials {
        println!("  special {:<16} {:<36} c {} axis {} hw {:>4.1} {}{}", s.kind, s.model, f3(s.centre), f3(s.axis), s.half_width, if s.from_item { "item" } else { "block" }, s.car.as_ref().map_or(String::new(), |c| format!(" → car {c}")));
    }
    if let Some(out) = flag(args, "--out") {
        io::write_gates(Path::new(&out), &g).unwrap_or_else(|e| die(&e));
        println!("wrote {out}");
    }
    if !g.control_ok() {
        std::process::exit(1);
    }
}

fn cmd_from_cartographer(args: &[String]) {
    let pos = positionals(args, &["--gates", "--out"], &[]);
    if pos.len() != 2 {
        die("from-cartographer PACK.pack.json ROUTE.route.json --gates gates.json --out route.json");
    }
    let gp = flag(args, "--gates").unwrap_or_else(|| die("--gates required"));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let prov = tmroute::provenance("tmroute from-cartographer");
    let imp = tmroute::cartographer::import(Path::new(&pos[0]), Path::new(&pos[1]), &gates, &prov).unwrap_or_else(|e| die(&e));
    let errs = imp.geom.validate();
    println!(
        "{}\t{}\tlegs {}\tlength {:.1} m\tmissing_groups {:?}\tvalid {}",
        imp.map_name,
        imp.geom.map_uid,
        imp.geom.legs.as_ref().map_or(0, |l| l.len()),
        imp.geom.s.last().copied().unwrap_or(0.0),
        imp.missing_groups,
        errs.is_empty()
    );
    for e in &errs {
        println!("  - {e}");
    }
    if let Some(out) = flag(args, "--out") {
        io::write_route(Path::new(&out), &imp.geom).unwrap_or_else(|e| die(&e));
        println!("wrote {out}");
    }
}

fn load_runs(paths: &[String]) -> Vec<human::Run> {
    let mut runs = Vec::new();
    for p in paths {
        match human::load_run(Path::new(p)) {
            Ok(r) => runs.push(r),
            Err(e) => eprintln!("skip {p}: {e}"),
        }
    }
    runs
}

fn make_rows(runs: &[human::Run], gates: &GatesFile) -> Vec<human::OrderRow> {
    let mut idx: Vec<usize> = (0..runs.len()).collect();
    idx.sort_by_key(|&i| if runs[i].declared_ms > 0 { runs[i].declared_ms } else { i32::MAX });
    let mut rank_of = vec![0u32; runs.len()];
    for (r, &i) in idx.iter().enumerate() {
        rank_of[i] = r as u32 + 1;
    }
    runs.iter()
        .enumerate()
        .map(|(i, run)| {
            let cr = human::crossings(run, gates, 40.0);
            let unmatched = cr.iter().filter(|c| c.is_none()).count();
            let max_d = cr.iter().flatten().map(|c| c.dist_xz).fold(0.0f32, f32::max);
            human::OrderRow {
                md5: run.md5.clone(),
                rank: rank_of[i],
                ms: run.declared_ms,
                order_wp: cr.iter().map(|c| c.as_ref().map_or(u32::MAX, |c| c.waypoint)).collect(),
                order_group: cr.iter().map(|c| c.as_ref().map_or(u32::MAX, |c| c.group)).collect(),
                respawns: run.respawns,
                cp_ms: run.splits_ms.clone(),
                unmatched,
                max_dist_xz: max_d,
                file: run.path.clone(),
            }
        })
        .collect()
}

fn cmd_human_orders(args: &[String]) {
    let ghosts = positionals(args, &["--gates", "--out"], &[]);
    let gp = flag(args, "--gates").unwrap_or_else(|| die("human-orders --gates gates.json --out human-orders.tsv GHOST..."));
    let gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let runs = load_runs(&ghosts);
    let rows = make_rows(&runs, &gates);
    let mut lines = vec![human::ORDERS_HEADER.to_string()];
    let mut ok = 0;
    let mut bad_count = 0;
    let mut unmatched = 0;
    for r in &rows {
        lines.push(human::order_row(r));
        if r.unmatched > 0 {
            unmatched += 1;
        }
        if gates.declared_checkpoints > 0 && r.cp_ms.len() as i32 != gates.expected_splits() {
            bad_count += 1;
        } else if r.unmatched == 0 {
            ok += 1;
        }
    }
    println!("{}", lines.join("\n"));
    let mut dists: Vec<f32> = rows.iter().map(|r| r.max_dist_xz).collect();
    eprintln!(
        "{}: {} ghosts; split count == declared ({}) on {}; wrong split count {}; with unmatched crossings {}; max XZ residual to gate centre P50 {:.1} m P90 {:.1} m max {:.1} m",
        gates.map_name,
        rows.len(),
        gates.expected_splits(),
        ok + unmatched.min(0),
        bad_count,
        unmatched,
        human::percentile(&mut dists.clone(), 50.0),
        human::percentile(&mut dists.clone(), 90.0),
        human::percentile(&mut dists, 100.0)
    );
    if let Some(out) = flag(args, "--out") {
        io::write_atomic(Path::new(&out), (lines.join("\n") + "\n").as_bytes()).unwrap_or_else(|e| die(&e));
        eprintln!("wrote {out}");
    }
}

fn cmd_consensus(args: &[String]) {
    let ghosts = positionals(args, &["--gates", "--out", "--gates-out", "--orders"], &[]);
    let gp = flag(args, "--gates").unwrap_or_else(|| die("consensus --gates gates.json --out route.json [--gates-out gates.json] [--orders human-orders.tsv] GHOST..."));
    let mut gates = io::read_gates(Path::new(&gp)).unwrap_or_else(|e| die(&e));
    let runs = load_runs(&ghosts);
    let rows = make_rows(&runs, &gates);
    if let Some(o) = flag(args, "--orders") {
        let mut lines = vec![human::ORDERS_HEADER.to_string()];
        lines.extend(rows.iter().map(human::order_row));
        io::write_atomic(Path::new(&o), (lines.join("\n") + "\n").as_bytes()).unwrap_or_else(|e| die(&e));
    }
    let prov = tmroute::provenance("tmroute consensus");
    let cert = flag(args, "--certified-by");
    let c = human::consensus(&gates, &runs, &rows, &prov, cert.as_deref());
    let j = |v: &[u32]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    println!(
        "{}\t{}\truns {}\tusable {}\tdistinct_orders {}\tmodal_groups [{}]\tmodal_n {}\tshare {:.2}\tagree {}",
        gates.map_name, gates.map_uid, c.n_runs, c.n_finished_ok, c.distinct_orders, j(&c.modal_group_order), c.n_modal, c.share, if c.agree { "YES" } else { "no" }
    );
    for n in &c.notes {
        println!("  note: {n}");
    }
    // every distinct order, with its count
    let mut counts: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
    for r in &rows {
        if r.unmatched == 0 && r.cp_ms.len() as i32 == gates.expected_splits() {
            *counts.entry(r.order_group.clone()).or_default() += 1;
        }
    }
    for (k, v) in &counts {
        println!("  order [{}] x{}", j(k), v);
    }
    if let Some(route) = &c.route {
        let errs = route.validate();
        println!("  route: {} pts, {:.1} m, {} legs, predicted {}, valid {}", route.pts.len(), route.s.last().copied().unwrap_or(0.0), route.legs.as_ref().map_or(0, |l| l.len()), io::secs(route.route.as_ref().unwrap().predicted_ms), errs.is_empty());
        for e in &errs {
            println!("    - {e}");
        }
        if let Some(out) = flag(args, "--out") {
            io::write_route(Path::new(&out), route).unwrap_or_else(|e| die(&e));
            println!("wrote {out}");
        }
        // human crossing directions orient the gates.json normals
        if let Some(go) = flag(args, "--gates-out") {
            let mut dirs = BTreeMap::new();
            for l in route.legs.as_ref().unwrap() {
                let g = gates.by_waypoint(l.map_waypoint).map(|g| g.group).unwrap();
                for r in gates.gates_of_group(g) {
                    dirs.insert(r.waypoint, l.arrival_heading);
                }
            }
            let n = tmroute::gates::orient(&mut gates, &dirs, "human");
            io::write_gates(Path::new(&go), &gates).unwrap_or_else(|e| die(&e));
            println!("oriented {n} gate normals from human crossings → {go}");
        }
    }
}

fn cmd_index(args: &[String]) {
    let pos = positionals(args, &["--names"], &[]);
    let root = pos.first().unwrap_or_else(|| die("index ROUTES_DIR [--names GEOM_DIR]"));
    let names_dir = flag(args, "--names").map(PathBuf::from);
    let names = |uid: &str| -> String {
        names_dir
            .as_ref()
            .and_then(|d| io::read_gates(&d.join(uid).join("gates.json")).ok())
            .map(|g| g.map_name)
            .unwrap_or_else(|| uid.to_string())
    };
    let (n, out) = io::rebuild_index(Path::new(root), &names).unwrap_or_else(|e| die(&e));
    println!("{n} routes → {}", out.display());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprintln!("usage: tmroute show|validate|agree|gates|from-cartographer|human-orders|consensus|index ... (see src/main.rs)");
        std::process::exit(2);
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "show" => cmd_show(rest),
        "validate" => cmd_validate(rest),
        "agree" => cmd_agree(rest),
        "gates" => cmd_gates(rest),
        "from-cartographer" => cmd_from_cartographer(rest),
        "human-orders" => cmd_human_orders(rest),
        "consensus" => cmd_consensus(rest),
        "index" => cmd_index(rest),
        "table" => cmd_table(rest),
        "human-batch" => cmd_human_batch(rest),
        other => die(&format!("unknown command {other}")),
    }
}

/// `tmroute table ROUTES_DIR --geom GEOM_DIR [--plan-source router-plan]`
/// The campaign table (BRIEF R6): per map, cartographer vs human modal vs
/// planner order, agreement by checkpoint GROUP, human share from consensus.txt.
pub fn cmd_table(args: &[String]) {
    let pos = positionals(args, &["--geom", "--plan-source"], &[]);
    let root = pos.first().unwrap_or_else(|| die("table ROUTES_DIR --geom GEOM_DIR"));
    let geom = flag(args, "--geom").unwrap_or_else(|| die("--geom GEOM_DIR"));
    let plan_source = flag(args, "--plan-source").unwrap_or_else(|| "router-plan".into());
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&geom).unwrap_or_else(|e| die(&e.to_string())).filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    let mut rows: Vec<(String, String)> = Vec::new();
    let j = |v: &[u32]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    let mut n_carto_cmp = 0;
    let mut n_carto_exact = 0;
    let mut n_human_cmp = 0;
    let mut n_human_exact = 0;
    dirs.sort();
    for d in dirs {
        let uid = d.file_name().unwrap().to_string_lossy().to_string();
        let Ok(g) = io::read_gates(&d.join("gates.json")) else { continue };
        let load = |src: &str| io::read_route(&Path::new(root).join(&uid).join(io::route_file_name(src, 0))).ok().map(|r| metrics::to_groups(&r.gate_order(), &g));
        let carto = load("cartographer");
        let mut human = load("router-human");
        let mut unverified = false;
        let plan = load(&plan_source);
        // no verified route: fall back to the UNVERIFIED consensus (ghosts without a resim verdict) — marked
        let cons_file = if d.join("consensus.txt").exists() { "consensus.txt" } else { unverified = human.is_none(); "consensus.unverified.txt" };
        if human.is_none() {
            human = std::fs::read_to_string(d.join(cons_file)).ok().and_then(|s| {
                let l = s.lines().next()?.to_string();
                let m = l.split('\t').find(|f| f.starts_with("modal_groups ["))?.trim_start_matches("modal_groups [").trim_end_matches(']').to_string();
                let v: Vec<u32> = m.split(',').filter_map(|x| x.parse().ok()).collect();
                (!v.is_empty()).then_some(v)
            });
        }
        // human share from consensus
        let share = std::fs::read_to_string(d.join(cons_file)).ok().and_then(|s| {
            let l = s.lines().next()?.to_string();
            let sh = l.split('\t').find(|f| f.starts_with("share "))?.trim_start_matches("share ").to_string();
            let n = l.split('\t').find(|f| f.starts_with("runs "))?.trim_start_matches("runs ").to_string();
            Some(format!("{sh} ({n} runs{})", if unverified { ", UNVERIFIED" } else { "" }))
        }).unwrap_or_else(|| "-".into());
        let cmp = |a: &Option<Vec<u32>>, b: &Option<Vec<u32>>| -> String {
            match (a, b) {
                (Some(a), Some(b)) => {
                    let (oa, ob) = metrics::symmetric_difference(a, b);
                    let tau = metrics::kendall_tau(a, b);
                    let ex = metrics::exact(a, b);
                    format!("{}{} τ={:.2}{}", if ex { "EXACT" } else { "differ" }, if oa.is_empty() && ob.is_empty() { "" } else { "*" }, tau, if oa.is_empty() && ob.is_empty() { String::new() } else { format!(" (only A {:?}, only B {:?})", oa, ob) })
                }
                _ => "-".into(),
            }
        };
        let pc = cmp(&plan, &carto);
        let ph = cmp(&plan, &human);
        if plan.is_some() && carto.is_some() {
            n_carto_cmp += 1;
            // exact over the checkpoints the cartographer knew: compare the plan restricted to carto's set
            let p = plan.as_ref().unwrap();
            let c = carto.as_ref().unwrap();
            let pr: Vec<u32> = p.iter().copied().filter(|x| c.contains(x)).collect();
            if pr == *c {
                n_carto_exact += 1;
            }
        }
        if plan.is_some() && human.is_some() {
            n_human_cmp += 1;
            if plan == human {
                n_human_exact += 1;
            }
        }
        rows.push((
            g.map_name.clone(),
            format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                if uid.starts_with("Tin") { format!("{} (tiny)", g.map_name) } else { g.map_name.clone() },
                g.checkpoint_groups,
                carto.as_ref().map_or("—".into(), |v| j(v)),
                human.as_ref().map_or("—".into(), |v| j(v)),
                share,
                plan.as_ref().map_or("— (no plan)".into(), |v| j(v)),
                pc,
                ph
            ),
        ));
    }
    rows.sort();
    println!("| map | CP groups | cartographer order (groups) | human modal order | human share | planner ({plan_source}) order | planner vs cartographer | planner vs human |");
    println!("|---|--:|---|---|---|---|---|---|");
    for (_, r) in &rows {
        println!("{r}");
    }
    println!();
    println!("planner order == cartographer order over the checkpoints the cartographer knew: {n_carto_exact}/{n_carto_cmp}; planner == human modal: {n_human_exact}/{n_human_cmp}. τ = Kendall tau over shared groups; * = the two orders cover different checkpoint sets (F1).");
}

/// `tmroute human-batch --data DATA/v0/maps --geom GEOM_DIR --routes ROUTES_DIR [--unverified]`
/// For every map dir with ghosts: verified ghosts (sidecar `verdict == "exact"`) → human-orders.tsv +
/// consensus.txt + routes/<uid>/route-router-human-0.json (+ gates.json normals oriented "human").
/// With --unverified, ghosts whose verdict is null are ALSO run, into the *.unverified.* files only.
pub fn cmd_human_batch(args: &[String]) {
    let data = flag(args, "--data").unwrap_or_else(|| die("--data DIR (…/v0/maps)"));
    let geom = flag(args, "--geom").unwrap_or_else(|| die("--geom GEOM_DIR"));
    let routes = flag(args, "--routes").unwrap_or_else(|| die("--routes ROUTES_DIR"));
    let unverified = has_flag(args, "--unverified");
    // who certified the verified ghosts: the DATA arm's plain-oracle resim (its box, from its STATUS.md)
    let oracle_box = flag(args, "--oracle-box").unwrap_or_else(|| "tm-player DATA resim (devvm62680)".into());
    let prov = tmroute::provenance("tmroute human-batch");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&data).unwrap_or_else(|e| die(&e.to_string())).filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for d in dirs {
        let uid = d.file_name().unwrap().to_string_lossy().to_string();
        let gp = Path::new(&geom).join(&uid).join("gates.json");
        if !gp.exists() {
            // a map the crawl brought that GEOM has not seen: build its gates.json from the crawled map file
            let mf = d.join("map.Map.Gbx");
            if mf.exists() {
                // a reader assertion on one odd map (tmmaps refusing to guess a free-block chunk) must not
                // take the whole batch down: catch the panic, report, move on
                let built = std::panic::catch_unwind(|| tmroute::gates::build(&mf, &prov)).unwrap_or_else(|_| Err("map reader panicked (see stderr)".to_string()));
                match built {
                    Ok(g) => {
                        io::write_gates(&gp, &g).unwrap_or_else(|e| die(&e));
                        let ctl = if g.control_ok() { "OK" } else { "FAIL" };
                        let line = format!("{}\t{}\tdeclared {}\tcp_groups {}\tfinish_groups {}\tcontrol {}\tyoff {}\tresid {:+.1}\tgates {}\toriented 0\n", g.map_name, g.map_uid, g.declared_checkpoints, g.checkpoint_groups, g.finish_groups, ctl, g.yoff, g.yoff_residual, g.gates.len() - 1);
                        let _ = io::write_atomic(&Path::new(&geom).join(&uid).join("gates.txt"), line.as_bytes());
                        print!("NEW gates.json: {line}");
                    }
                    Err(e) => eprintln!("{uid}: gates.json build failed: {e}"),
                }
            }
        }
        let Ok(mut gates) = io::read_gates(&gp) else { eprintln!("{uid}: no gates.json, skipped"); continue };
        let gdir = d.join("ghosts");
        let Ok(rd) = std::fs::read_dir(&gdir) else { continue };
        let mut exact = Vec::new();
        let mut pending = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map_or(true, |x| x != "Gbx") || !p.to_string_lossy().ends_with(".Ghost.Gbx") { continue; }
            let side = p.with_extension("").with_extension("json");
            let verdict = std::fs::read_to_string(&side).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v.get("verdict").and_then(|x| x.as_str().map(|s| s.to_string())));
            match verdict.as_deref() {
                Some("exact") => exact.push(p.to_string_lossy().to_string()),
                Some(_) => {}
                None => pending.push(p.to_string_lossy().to_string()),
            }
        }
        let run = |ghost_paths: &[String], suffix: &str, bank_route: bool, gates: &mut GatesFile| {
            if ghost_paths.is_empty() { return; }
            let runs = load_runs(ghost_paths);
            let rows = make_rows(&runs, gates);
            let mut lines = vec![human::ORDERS_HEADER.to_string()];
            lines.extend(rows.iter().map(human::order_row));
            let _ = io::write_atomic(&Path::new(&geom).join(&uid).join(format!("human-orders{suffix}.tsv")), (lines.join("\n") + "\n").as_bytes());
            let c = human::consensus(gates, &runs, &rows, &prov, if bank_route { Some(oracle_box.as_str()) } else { None });
            let j = |v: &[u32]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
            let mut out = format!(
                "{}\t{}\truns {}\tusable {}\tdistinct_orders {}\tmodal_groups [{}]\tmodal_n {}\tshare {:.2}\tagree {}\n",
                gates.map_name, gates.map_uid, c.n_runs, c.n_finished_ok, c.distinct_orders, j(&c.modal_group_order), c.n_modal, c.share, if c.agree { "YES" } else { "no" }
            );
            for n in &c.notes { out.push_str(&format!("  note: {n}\n")); }
            let mut counts: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
            for r in &rows { if r.unmatched == 0 && r.cp_ms.len() as i32 == gates.expected_splits() { *counts.entry(r.order_group.clone()).or_default() += 1; } }
            for (k, v) in &counts { out.push_str(&format!("  order [{}] x{}\n", j(k), v)); }
            if let Some(route) = &c.route {
                out.push_str(&format!("  route: {} pts, {:.1} m, {} legs, predicted {}, valid {}\n", route.pts.len(), route.s.last().copied().unwrap_or(0.0), route.legs.as_ref().map_or(0, |l| l.len()), io::secs(route.route.as_ref().unwrap().predicted_ms), route.validate().is_empty()));
                if bank_route && c.agree {
                    let f = Path::new(&routes).join(&uid).join(io::route_file_name("router-human", 0));
                    io::write_route(&f, route).unwrap_or_else(|e| die(&e));
                    out.push_str(&format!("wrote {}\n", f.display()));
                }
                // the SIGN of every gate normal from the humans' travel — verified or not: 19 unanimous runs settle a
                // sign; the cartographer's tour tangent got it wrong on 6/7 gates of Summer 2026 - 10 (GEN arm,
                // engine-credited rows). Verified runs override an unverified orientation, never the reverse.
                let already_human = gates.gates.iter().any(|g| g.normal_source == "human");
                if c.agree && (bank_route || !already_human) {
                    let mut dirs = BTreeMap::new();
                    for l in route.legs.as_ref().unwrap() {
                        let g = gates.by_waypoint(l.map_waypoint).map(|g| g.group).unwrap();
                        for r in gates.gates_of_group(g) { dirs.insert(r.waypoint, l.arrival_heading); }
                    }
                    let n = tmroute::gates::orient(gates, &dirs, if bank_route { "human" } else { "human-unverified" });
                    io::write_gates(&gp, gates).unwrap_or_else(|e| die(&e));
                    out.push_str(&format!("oriented {n} gate normals from human crossings ({})\n", if bank_route { "verified" } else { "unverified" }));
                }
            }
            let _ = io::write_atomic(&Path::new(&geom).join(&uid).join(format!("consensus{suffix}.txt")), out.as_bytes());
            print!("{}", out.lines().next().unwrap_or(""));
            println!("\t[{} ghosts{}]", ghost_paths.len(), suffix);
        };
        run(&exact, "", true, &mut gates);
        if unverified { run(&pending, ".unverified", false, &mut gates); }
    }
}
fn has_flag(args: &[String], name: &str) -> bool { args.iter().any(|a| a == name) }
