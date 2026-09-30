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
            gates.spawn.yaw = route.spawn_yaw;
            gates.spawn.yaw_source = "human".into();
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
        "table-r" => cmd_table_r(rest),
        "split" => cmd_split(rest),
        "tiny-map" => cmd_tiny_map(rest),
        "credit-fit" => cmd_credit_fit(rest),
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
    let human_maps: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
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
                    // and the spawn facing from the humans' first metres (the start block's direction sign is a guess)
                    gates.spawn.yaw = route.spawn_yaw;
                    gates.spawn.yaw_source = if bank_route { "human".into() } else { "human-unverified".into() };
                    io::write_gates(&gp, gates).unwrap_or_else(|e| die(&e));
                    out.push_str(&format!("oriented {n} gate normals from human crossings ({})\n", if bank_route { "verified" } else { "unverified" }));
                }
            }
            let _ = io::write_atomic(&Path::new(&geom).join(&uid).join(format!("consensus{suffix}.txt")), out.as_bytes());
            print!("{}", out.lines().next().unwrap_or(""));
            println!("\t[{} ghosts{}]", ghost_paths.len(), suffix);
            if !c.modal_group_order.is_empty() {
                human_maps.borrow_mut().push(format!("{}\t{}\t{}\t{}\t{}", gates.map_uid, gates.map_name, if suffix.is_empty() { "verified" } else { "unverified" }, if c.agree { "agree" } else { "split" }, j(&c.modal_group_order)));
            }
        };
        run(&exact, "", true, &mut gates);
        if unverified { run(&pending, ".unverified", false, &mut gates); }
    }
    // one file listing the maps that have a human modal order — the exhibit's map set, so plan-r need not walk
    // 600 dirs on the bank mount (16 min at 17:07Z) to find ~30
    let mut lines = vec!["map_uid\tmap_name\tghosts\tconsensus\tmodal_groups".to_string()];
    lines.extend(human_maps.borrow().iter().cloned());
    let _ = io::write_atomic(&Path::new(&geom).join("human-maps.tsv"), (lines.join("\n") + "\n").as_bytes());
    eprintln!("human-maps.tsv: {} rows", lines.len() - 1);
}
fn has_flag(args: &[String], name: &str) -> bool { args.iter().any(|a| a == name) }

/// `tmroute table-r ROUTES_DIR --geom GEOM_DIR [--geo router-plan-cost] [--r router-plan-r] [--also uid,uid]`
/// The M2 exhibit: per map with a human modal order (verified consensus, else unverified — marked), the
/// GEOMETRIC planner's order, the R planner's order (`tmr plan`), and each one's agreement with the humans
/// (EXACT / τ). `--also` adds maps without a human order (the failed / hypothesis maps) with what exists.
pub fn cmd_table_r(args: &[String]) {
    let pos = positionals(args, &["--geom", "--geo", "--r", "--hyb", "--also", "--train", "--held-out", "--title", "--uids"], &[]);
    let root = pos.first().unwrap_or_else(|| die("table-r ROUTES_DIR --geom GEOM_DIR"));
    let geom = flag(args, "--geom").unwrap_or_else(|| die("--geom GEOM_DIR"));
    let geo_src = flag(args, "--geo").unwrap_or_else(|| "router-plan-cost".into());
    let r_src = flag(args, "--r").unwrap_or_else(|| "router-plan-r".into());
    let hyb_src = flag(args, "--hyb").unwrap_or_else(|| "router-plan-hyb".into());
    let (mut hyb_ex, mut n_hyb_plans, mut unseen_hyb_ex, mut hyb_tau, mut n_hyb_tau) = (0usize, 0usize, 0usize, 0.0f64, 0usize);
    let (mut geo_cp, mut r_cp, mut hyb_cp, mut unseen_hyb_cp) = (0usize, 0usize, 0usize, 0usize);
    let (mut hon_hyb_legs, mut hon_hyb_tau, mut hon_hyb_n, mut hon_hyb_wrong) = ((0usize, 0usize), 0.0f64, 0usize, 0usize);
    let (mut hon_geo_legs, mut hon_geo_tau, mut hon_geo_n, mut hon_geo_wrong) = ((0usize, 0usize), 0.0f64, 0usize, 0usize);
    let also: Vec<String> = flag(args, "--also").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
    let list = |k: &str| -> Vec<String> { flag(args, k).map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default() };
    let (train, held) = (list("--train"), list("--held-out"));
    let title = flag(args, "--title");
    let (mut unseen_n, mut unseen_r_ex, mut unseen_geo_ex) = (0usize, 0usize, 0usize);
    // --uids: only these map dirs (the exhibit's ~35 maps) — a walk over all 500+ dirs on the bank mount is minutes
    let only = list("--uids");
    let mut dirs: Vec<PathBuf> = if only.is_empty() {
        std::fs::read_dir(&geom).unwrap_or_else(|e| die(&e.to_string())).filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect()
    } else {
        only.iter().chain(also.iter()).map(|u| Path::new(&geom).join(u)).filter(|p| p.is_dir()).collect()
    };
    dirs.sort();
    dirs.dedup();
    let j = |v: &[u32]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    let mut rows: Vec<(String, String)> = Vec::new();
    let (mut n_h, mut geo_ex, mut r_ex, mut both_have, mut geo_tau, mut r_tau, mut n_tau) = (0usize, 0usize, 0usize, 0usize, 0.0f64, 0.0f64, 0usize);
    let (mut n_geo_plans, mut n_r_plans) = (0usize, 0usize);
    for d in dirs {
        let uid = d.file_name().unwrap().to_string_lossy().to_string();
        let Ok(g) = io::read_gates(&d.join("gates.json")) else { continue };
        let load = |src: &str| io::read_route(&Path::new(root).join(&uid).join(io::route_file_name(src, 0))).ok().map(|r| metrics::to_groups(&r.gate_order(), &g));
        let mut unverified = false;
        let cons_file = if d.join("consensus.txt").exists() { "consensus.txt" } else { unverified = true; "consensus.unverified.txt" };
        let cons = std::fs::read_to_string(d.join(cons_file)).ok().and_then(|s| s.lines().next().map(|l| l.to_string()));
        let field = |l: &str, key: &str| -> Option<String> { l.split('\t').find(|f| f.starts_with(key)).map(|f| f.trim_start_matches(key).to_string()) };
        let human: Option<Vec<u32>> = cons.as_ref().and_then(|l| {
            let m = field(l, "modal_groups [")?.trim_end_matches(']').to_string();
            let v: Vec<u32> = m.split(',').filter_map(|x| x.parse().ok()).collect();
            (!v.is_empty()).then_some(v)
        });
        if human.is_none() && !also.contains(&uid) {
            continue;
        }
        let share = cons.as_ref().map(|l| format!("{} ({} runs{})", field(l, "share ").unwrap_or_default(), field(l, "runs ").unwrap_or_default(), if unverified { ", unverified" } else { "" })).unwrap_or_else(|| "-".into());
        let geo = load(&geo_src);
        let r = load(&r_src);
        let hyb = load(&hyb_src);
        // full order (checkpoints + the finish line chosen) and the CP ORDER alone (everything but the last group):
        // a map with several finish lines (Summer 2026 - 05) can have the human CP order and another finish
        let cmp = |a: &Option<Vec<u32>>| -> (String, Option<(bool, f64, bool, usize, usize)>) {
            match (a, &human) {
                (Some(a), Some(h)) => {
                    let tau = metrics::kendall_tau(a, h);
                    let ex = metrics::exact(a, h);
                    let cp_ex = a.len() == h.len() && a.len() > 1 && a[..a.len() - 1] == h[..h.len() - 1];
                    let (lm, ln) = metrics::leg_agreement(a, h);
                    (format!("{} τ={:.2} legs {lm}/{ln}{}", if ex { "EXACT" } else { "differ" }, tau, if cp_ex && !ex { " (CP order EXACT, other finish)" } else { "" }), Some((ex, tau, cp_ex, lm, ln)))
                }
                _ => ("-".into(), None),
            }
        };
        let (gs, gv) = cmp(&geo);
        let (rs, rv) = cmp(&r);
        let (hs, hv) = cmp(&hyb);
        if human.is_some() {
            n_h += 1;
            if geo.is_some() { n_geo_plans += 1; }
            if r.is_some() { n_r_plans += 1; }
            if let Some((ex, _, cp, _, _)) = gv { if ex { geo_ex += 1; } if cp { geo_cp += 1; } }
            if let Some((ex, _, cp, _, _)) = rv { if ex { r_ex += 1; } if cp { r_cp += 1; } }
            if hyb.is_some() { n_hyb_plans += 1; }
            if let Some((ex, t, cp, _, _)) = hv { if ex { hyb_ex += 1; } if cp { hyb_cp += 1; } hyb_tau += t; n_hyb_tau += 1; }
            if let (Some((_, tg, _, _, _)), Some((_, tr, _, _, _))) = (gv, rv) {
                both_have += 1;
                geo_tau += tg;
                r_tau += tr;
                n_tau += 1;
            }
        }
        let hyp = also.contains(&uid);
        // MODEL's split: fnv1a64(uid) % 10 == 0 is held out of R's training forever
        let seen = if metrics::fnv_held_out(&uid) { "held-out(fnv)" } else if train.contains(&uid) { "train" } else if held.contains(&uid) { "held-out" } else { "unseen" };
        if human.is_some() && seen != "train" {
            unseen_n += 1;
            if let Some((true, _, _, _, _)) = rv { unseen_r_ex += 1; }
            if let Some((true, _, _, _, _)) = gv { unseen_geo_ex += 1; }
            if let Some((true, _, _, _, _)) = hv { unseen_hyb_ex += 1; }
            if let Some((_, _, true, _, _)) = hv { unseen_hyb_cp += 1; }
            // the M2 reading, pooled over the honest rows: legs, τ, exact, τ < 0.4 ("genuinely wrong")
            if let Some((_, t, _, lm, ln)) = hv { hon_hyb_legs.0 += lm; hon_hyb_legs.1 += ln; hon_hyb_tau += t; hon_hyb_n += 1; if t < 0.4 { hon_hyb_wrong += 1; } }
            if let Some((_, t, _, lm, ln)) = gv { hon_geo_legs.0 += lm; hon_geo_legs.1 += ln; hon_geo_tau += t; hon_geo_n += 1; if t < 0.4 { hon_geo_wrong += 1; } }
            // a map with a human order and NO hybrid plan counts as 0 legs matched of its legs
            if hv.is_none() { if let Some(h) = &human { hon_hyb_legs.1 += h.len().saturating_sub(1); } }
        }
        rows.push((g.map_name.clone(), format!("| {}{} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |", g.map_name, if hyp { " †" } else { "" }, g.checkpoint_groups, human.as_ref().map_or("—".into(), |v| j(v)), share, geo.as_ref().map_or("— (no plan)".into(), |v| j(v)), gs, r.as_ref().map_or("— (no plan)".into(), |v| j(v)), rs, hyb.as_ref().map_or("— (no plan)".into(), |v| j(v)), hs, seen)));
    }
    rows.sort();
    if let Some(t) = &title {
        println!("{t}\n");
    }
    println!("| map | CP groups | human modal order (groups) | human share | geometric planner ({geo_src}) | geo vs human | R planner ({r_src}) | R vs human | HYBRID planner ({hyb_src}) | hybrid vs human | R saw the map? |");
    println!("|---|--:|---|---|---|---|---|---|---|---|---|");
    for (_, r) in &rows {
        println!("{r}");
    }
    println!();
    println!("maps with a human order: {n_h}; geometric planner has a plan on {n_geo_plans}, == human on {geo_ex}; R planner has a plan on {n_r_plans}, == human on {r_ex}; over the {both_have} maps both planned: mean τ geometric {:.3}, R {:.3}. HYBRID: plan on {n_hyb_plans}, == human on {hyb_ex}, mean τ {:.3}. CP-ORDER agreement (finish-line choice ignored): geometric {geo_cp}, R {r_cp}, hybrid {hyb_cp}. HONEST ROWS (held-out + unseen by R): {unseen_n} maps, R == human on {unseen_r_ex}, geometric == human on {unseen_geo_ex}, hybrid == human on {unseen_hyb_ex} (CP order {unseen_hyb_cp}). † = added without a human order (failed / hypothesis maps). τ = Kendall tau over the checkpoint groups.", if n_tau > 0 { geo_tau / n_tau as f64 } else { f64::NAN }, if n_tau > 0 { r_tau / n_tau as f64 } else { f64::NAN }, if n_hyb_tau > 0 { hyb_tau / n_hyb_tau as f64 } else { f64::NAN });
    println!("M2 READING over the {unseen_n} honest rows — HYBRID: per-leg agreement {}/{} = {:.1} % (a map with no plan counts all its legs missed), mean τ {:.3} over {hon_hyb_n} planned, exact {unseen_hyb_ex}, τ < 0.4 (genuinely wrong route) {hon_hyb_wrong}; GEOMETRIC: per-leg {}/{} = {:.1} % over its {hon_geo_n} planned maps, mean τ {:.3}, exact {unseen_geo_ex}, τ < 0.4 {hon_geo_wrong}.", hon_hyb_legs.0, hon_hyb_legs.1, if hon_hyb_legs.1 > 0 { 100.0 * hon_hyb_legs.0 as f64 / hon_hyb_legs.1 as f64 } else { f64::NAN }, if hon_hyb_n > 0 { hon_hyb_tau / hon_hyb_n as f64 } else { f64::NAN }, hon_geo_legs.0, hon_geo_legs.1, if hon_geo_legs.1 > 0 { 100.0 * hon_geo_legs.0 as f64 / hon_geo_legs.1 as f64 } else { f64::NAN }, if hon_geo_n > 0 { hon_geo_tau / hon_geo_n as f64 } else { f64::NAN });
}

/// `tmroute split UID...` — MODEL's fnv1a64 % 10 rule: HELD-OUT (== 0) or train, per uid.
fn cmd_split(args: &[String]) {
    for u in args.iter().filter(|a| !a.starts_with("--")) {
        println!("{u}\t{}\t{}", metrics::fnv1a64(u) % 10, if metrics::fnv_held_out(u) { "HELD-OUT" } else { "train" });
    }
}

/// `tmroute tiny-map --full gates.json --tiny gates.json [--order 1,2,0,3]`
/// Gate-for-gate correspondence between a full-size map and its TINY copy (uniform scale 0.5 about a
/// centre c; c is solved from the two spawns: c = 2·tiny_spawn − full_spawn), then every full gate group
/// is mapped to the nearest tiny gate group. With --order (a full-size GROUP order, e.g. the human modal
/// order) prints the same order in tiny group ids.
fn cmd_tiny_map(args: &[String]) {
    let full = io::read_gates(Path::new(&flag(args, "--full").unwrap_or_else(|| die("--full gates.json")))).unwrap_or_else(|e| die(&e));
    let tiny = io::read_gates(Path::new(&flag(args, "--tiny").unwrap_or_else(|| die("--tiny gates.json")))).unwrap_or_else(|e| die(&e));
    let fs = full.spawn.pos;
    let ts = tiny.spawn.pos;
    let c = [2.0 * ts[0] - fs[0], 2.0 * ts[1] - fs[1], 2.0 * ts[2] - fs[2]];
    let to_tiny = |p: [f32; 3]| [c[0] + 0.5 * (p[0] - c[0]), c[1] + 0.5 * (p[1] - c[1]), c[2] + 0.5 * (p[2] - c[2])];
    // group centres
    let centres = |g: &tmroute::gates::GatesFile| -> BTreeMap<u32, [f32; 3]> {
        let mut acc: BTreeMap<u32, (usize, [f32; 3])> = BTreeMap::new();
        for r in &g.gates {
            if r.kind == tmroute::gates::WpKind::Start { continue; }
            let e = acc.entry(r.group).or_insert((0, [0.0; 3]));
            e.0 += 1;
            for a in 0..3 { e.1[a] += r.centre[a]; }
        }
        acc.into_iter().map(|(k, (n, s))| (k, [s[0] / n as f32, s[1] / n as f32, s[2] / n as f32])).collect()
    };
    let fc = centres(&full);
    let tc = centres(&tiny);
    println!("scale centre c = ({:.1}, {:.1}, {:.1}); full groups {}, tiny groups {}", c[0], c[1], c[2], fc.len(), tc.len());
    let mut m: BTreeMap<u32, (u32, f32)> = BTreeMap::new();
    for (fg, fp) in &fc {
        let p = to_tiny(*fp);
        let (best, d) = tc.iter().map(|(tg, tp)| (*tg, ((tp[0] - p[0]).powi(2) + (tp[2] - p[2]).powi(2)).sqrt())).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
        m.insert(*fg, (best, d));
        println!("  full group {fg:>2} → tiny group {best:>2}  (XZ residual {d:.1} m)");
    }
    let worst = m.values().map(|v| v.1).fold(0.0f32, f32::max);
    let distinct: std::collections::BTreeSet<u32> = m.values().map(|v| v.0).collect();
    let ok = distinct.len() == fc.len() && worst < 40.0;
    println!("mapping {}: worst residual {worst:.1} m, {} of {} tiny groups hit", if ok { "OK" } else { "AMBIGUOUS" }, distinct.len(), tc.len());
    if let Some(o) = flag(args, "--order") {
        let order: Vec<u32> = o.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        let mapped: Vec<u32> = order.iter().filter_map(|g| m.get(g).map(|v| v.0)).collect();
        println!("tiny order: {}", mapped.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
        // --geo / --hyb: tiny planner orders to compare with the mapped human order
        for key in ["--geo", "--hyb"] {
            if let Some(p) = flag(args, key) {
                let po: Vec<u32> = p.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if po.is_empty() { println!("{key}: no plan"); continue; }
                let ex = metrics::exact(&po, &mapped);
                let cp_ex = po.len() == mapped.len() && po.len() > 1 && po[..po.len() - 1] == mapped[..mapped.len() - 1];
                println!("{key}: {} τ={:.2}{}", if ex { "EXACT" } else { "differ" }, metrics::kendall_tau(&po, &mapped), if cp_ex && !ex { " (CP order EXACT, other finish)" } else { "" });
            }
        }
    }
}

/// `tmroute credit-fit --gates gates.json --crossings X.tsv`
/// Engine-credited crossings (the INPUT arm's per-gate car states around the CP-counter split, 10 ms ticks) against
/// our gate frames: per credited gate, s = (p − centre)·normal at the last row BEFORE the split and at the split row
/// (the counter increments between them), the lateral offset, our `credit_offset_m`, and the fitted offset (midpoint).
fn cmd_credit_fit(args: &[String]) {
    let g = io::read_gates(Path::new(&flag(args, "--gates").unwrap_or_else(|| die("--gates")))).unwrap_or_else(|e| die(&e));
    let text = std::fs::read_to_string(flag(args, "--crossings").unwrap_or_else(|| die("--crossings"))).unwrap_or_else(|e| die(&e.to_string()));
    let mut rows: Vec<(u32, i32, i32, [f32; 3], [f32; 3])> = Vec::new(); // gate, split, t, pos, vel
    for l in text.lines().skip(2) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 9 { continue; }
        let p = |i: usize| f[i].trim().parse::<f32>().unwrap_or(f32::NAN);
        rows.push((f[0].trim().parse().unwrap_or(0), f[1].trim().parse().unwrap_or(0), f[2].trim().parse().unwrap_or(0), [p(3), p(4), p(5)], [p(6), p(7), p(8)]));
    }
    println!("{}\t{} crossing rows", g.map_name, rows.len());
    println!("gate\tsplit_ms\tour wp\tmodel\tfrom_item\ts_before\ts_at\tlat\tour_offset\tfitted_offset\ttravel·n\tspeed_kmh");
    let mut gates: Vec<u32> = rows.iter().map(|r| r.0).collect();
    gates.sort(); gates.dedup();
    for gi in gates {
        let rs: Vec<_> = rows.iter().filter(|r| r.0 == gi).collect();
        let split = rs[0].1;
        let Some(at) = rs.iter().find(|r| r.2 == split) else { println!("{gi}\t{split}\t-\tno row at the split"); continue };
        let Some(before) = rs.iter().find(|r| r.2 == split - 10) else { println!("{gi}\t{split}\t-\tno row 10 ms before the split"); continue };
        // our gate: nearest gate centre to the crossing row (no group knowledge in the file)
        let (wi, gate) = g.gates.iter().enumerate().filter(|(_, x)| x.kind != tmroute::gates::WpKind::Start).min_by(|a, b| {
            let da = (0..3).map(|k| (a.1.centre[k] - at.3[k]).powi(2)).sum::<f32>();
            let db = (0..3).map(|k| (b.1.centre[k] - at.3[k]).powi(2)).sum::<f32>();
            da.partial_cmp(&db).unwrap()
        }).unwrap();
        let n = gate.normal;
        let s = |p: [f32; 3]| (0..3).map(|k| (p[k] - gate.centre[k]) * n[k]).sum::<f32>();
        let s_b = s(before.3); let s_a = s(at.3);
        let d = [at.3[0] - gate.centre[0], at.3[1] - gate.centre[1], at.3[2] - gate.centre[2]];
        let lat = { let along = s_a; let r = [d[0] - along * n[0], d[1] - along * n[1], d[2] - along * n[2]]; (r[0] * r[0] + r[2] * r[2]).sqrt() };
        let v = at.4; let vl = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        let tn = (v[0] * n[0] + v[1] * n[1] + v[2] * n[2]) / vl;
        println!("{gi}\t{split}\t{}\t{}\t{}\t{s_b:.2}\t{s_a:.2}\t{lat:.2}\t{:.2}\t{:.2}\t{tn:+.2}\t{:.0}", gate.waypoint, gate.model, gate.from_item, gate.credit_offset_m, 0.5 * (s_b + s_a), vl * 3.6);
        let _ = wi;
    }
}
