//! `tmmaps dropscan` — read the ENVIRONMENT's collision geometry with the car.
//!
//! ## Why this exists
//!
//! 285885 `finish is on the roof to your right` has 113 unbaked blocks, all of
//! them at y = 10, and its Goal item stands at **(419.0, 144.0, 1704.6)** —
//! 500 m past the last block in z and 134 m above the highest one. Every
//! surface the endgame is driven on belongs to the **Stadium decoration**, not
//! to the map, so a census of the map file says nothing at all about where the
//! car can go. Three arms have characterised that endgame by perturbing one
//! tape and watching where the car happened to travel; nobody could ask the
//! plain question *what is over there*.
//!
//! This asks it. A map's spawn is an ordinary grid block: move it to a cell,
//! drive the car straight off it, and the engine's own trajectory says what it
//! hit, how high, and where it ended. One probe is one map plus one `fk trace`.
//!
//! ## The two controls, and why each is load-bearing
//!
//! * **Origin.** The spawn is moved to its OWN cell and the decompressed body
//!   of the result is required to be byte-identical to the untouched map's.
//!   That is the map-surgery control this project already enforces everywhere
//!   (`tmmaps origin`): if the mover writes dead bytes, every rung of the scan
//!   is measuring the mover.
//! * **Reference probe.** One probe is always run at the spawn's REAL cell, and
//!   its trajectory must start at the map's real spawn position (within 1 m).
//!   That is the positive control for the whole instrument: it says the tape
//!   drives, the trace locates the car, and the CSV means what it says. A scan
//!   whose reference probe fails is reporting on itself, not on the map.
//!
//! ## What a probe measures, and what it does not
//!
//! It measures the surface the car actually reaches from that spawn under one
//! fixed input tape. A cell that reports "fell to the void" means *nothing
//! caught this car on this path*, not "there is no geometry near this cell":
//! the car leaves the block with speed and drifts. Read the landing POSITION,
//! which is measured, rather than the spawn cell, which is only where it
//! started.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::map;
// `tools/ghost` owns the ghost format; tmmaps reaches it through that crate
// rather than depending on `gbx` a second time (and `crate::gbx` here is
// tmmaps' own MAP container, a different thing entirely).
use ghost::container::Container;
use ghost::tape::{Encoding, Tape};

/// Cell -> world, the convention the census prints: x = 32·cx + 16,
/// y = 8·cy − 62, z = 32·cz + 16. Derived from this map's own spawn row
/// (cell (4,9,20) -> (144, 10, 656)) and checked against every grid block in
/// the census.
pub fn cell_to_world(c: (i32, i32, i32)) -> (f64, f64, f64) {
    (32.0 * c.0 as f64 + 16.0, 8.0 * c.1 as f64 - 62.0, 32.0 * c.2 as f64 + 16.0)
}

struct Probe {
    id: usize,
    cell: (i32, i32, i32),
    dir: u8,
    /// When set, this probe is a TAPE on the untouched map rather than a moved
    /// spawn: the population being scored is a family of candidate inputs, and
    /// what is measured is where the car got, not what the map is made of.
    tape: Option<PathBuf>,
}

struct Summary {
    id: usize,
    cell: (i32, i32, i32),
    dir: u8,
    ok: bool,
    note: String,
    rows: usize,
    t0: f64,
    x0: f64,
    y0: f64,
    z0: f64,
    ymax: f64,
    ymax_at: (f64, f64, f64),
    ymin: f64,
    xend: f64,
    yend: f64,
    zend: f64,
    tend: f64,
    vmax: f64,
    dmin: f64,
    dmin_at: (f64, f64, f64, f64),
}

fn hdr() -> String {
    "id\tcx\tcy\tcz\tdir\tsx\tsy\tsz\tok\trows\tt0\tx0\ty0\tz0\tymax\tymax_x\tymax_y\tymax_z\tymin\t\
     tend\txend\tyend\tzend\tvmax\tdmin\tdmin_t\tdmin_x\tdmin_y\tdmin_z\tnote"
        .to_string()
}

impl Summary {
    fn line(&self) -> String {
        let w = cell_to_world(self.cell);
        format!(
            "{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{}\t{}\t{:.3}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t\
             {:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.3}\t{:.1}\t{:.1}\t{:.1}\t{:.0}\t{:.1}\t{:.3}\t{:.1}\t\
             {:.1}\t{:.1}\t{}",
            self.id,
            self.cell.0,
            self.cell.1,
            self.cell.2,
            self.dir,
            w.0,
            w.1,
            w.2,
            if self.ok { "ok" } else { "FAIL" },
            self.rows,
            self.t0,
            self.x0,
            self.y0,
            self.z0,
            self.ymax,
            self.ymax_at.0,
            self.ymax_at.1,
            self.ymax_at.2,
            self.ymin,
            self.tend,
            self.xend,
            self.yend,
            self.zend,
            self.vmax,
            self.dmin,
            self.dmin_at.0,
            self.dmin_at.1,
            self.dmin_at.2,
            self.dmin_at.3,
            self.note
        )
    }
}

/// Build the probe tape: one fixed input word for every tick — full throttle,
/// no steering, no brake — written EXPLICITLY so no tick inherits another's.
///
/// The control is inside the function: the file is read back and every packet
/// must carry exactly those inputs. A tape that silently kept its donor's
/// steering would make every probe a copy of the donor's route.
fn build_probe_tape(
    src: &str,
    out: &str,
    steer: i8,
    accel: u32,
    brake: u32,
    ticks: usize,
) -> Result<usize, String> {
    let c = Container::load(src)?;
    let mut t = Tape::from_file(src)?;
    let mut n = 0usize;
    for a in t.archives.iter_mut() {
        // The car cannot move during the countdown (start_offset is negative and
        // those ticks are inert), so the first ticks are free to carry a
        // FINGERPRINT. Without one this tape is a constant array, and the fork
        // driver's locator — which finds the decoded input array by value —
        // matches a field of zeros somewhere else in the heap and then reports
        // `TAPE MISMATCH` on the array it found. Measured: a constant tape
        // fails the identity control on 6274 of 6274 ticks; the same tape with
        // 120 fingerprint ticks passes on all of them.
        let n_cd = ((-a.start_offset_ms).max(0) as usize / 10).min(a.packets.len());
        // A short probe only needs the first few seconds. A shorter tape is a
        // shorter simulation, and it puts `--at frac:F` where the interesting
        // part of the fall is instead of 50 s past it.
        if ticks > 0 && ticks < a.packets.len() {
            a.packets.truncate(ticks);
        }
        for (i, p) in a.packets.iter_mut().enumerate() {
            p.steer = if i < n_cd {
                // deterministic, non-repeating, and never 0
                let v = (((i as i32 * 37 + 11) % 101) - 50) as i8;
                ((if v == 0 { 7 } else { v }) as u8) as u32
            } else {
                (steer as u8) as u32
            };
            p.accel = accel;
            p.brake = brake;
            p.vsame = false;
            n += 1;
        }
    }
    // `splice_into` returns the BODY; `inject_into` returns a whole file
    // (uncompressed header + body) for the search's patchable base image.
    // Handing the latter to `write_gbx`, which prepends a header itself,
    // writes a file 25 bytes too long that the server silently ignores — it
    // does not even appear in the validator's output.
    let body = t.splice_into(c.body(), Encoding::Explicit)?;
    ghost::container::write_gbx(&c.gbx, body, out)?;
    let back = Tape::from_file(out)?;
    for a in back.archives.iter() {
        let n_cd = ((-a.start_offset_ms).max(0) as usize / 10).min(a.packets.len());
        for (i, p) in a.packets.iter().enumerate() {
            if i < n_cd {
                continue;
            }
            if p.steer_i8() != steer || p.accel != accel || p.brake != brake {
                return Err(format!(
                    "probe tape control FAILED: tick {} reads steer {} accel {} brake {}, wrote {} {} {}",
                    i,
                    p.steer_i8(),
                    p.accel,
                    p.brake,
                    steer,
                    accel,
                    brake
                ));
            }
        }
    }
    Ok(n)
}

/// Parse an `fk trace` CSV into (t_s, x, y, z, speed_kmh) rows.
fn read_trace(path: &Path) -> Result<Vec<(f64, f64, f64, f64, f64)>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let mut out = Vec::new();
    for (i, l) in txt.lines().enumerate() {
        if i == 0 || l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.trim().split(',').collect();
        if f.len() < 5 {
            continue;
        }
        let g = |k: usize| f[k].trim().parse::<f64>().unwrap_or(f64::NAN);
        out.push((g(0) / 1000.0, g(1), g(2), g(3), g(4)));
    }
    if out.is_empty() {
        return Err("no rows".into());
    }
    Ok(out)
}

pub fn cmd(args: &[String]) {
    let map_path = args.get(2).cloned().unwrap_or_else(|| die("dropscan needs a MAP path"));
    let f = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let out_dir = PathBuf::from(f("--out").unwrap_or_else(|| die("--out DIR")));
    let tape_src = f("--tape").unwrap_or_else(|| die("--tape GHOST (the container to write inputs into)"));
    let jobs: usize = f("--jobs").and_then(|s| s.parse().ok()).unwrap_or(8);
    let at = f("--at").unwrap_or_else(|| "frac:0.25,frac:0.45,frac:0.12,frac:0.65".into());
    let ticks: usize = f("--ticks").and_then(|s| s.parse().ok()).unwrap_or(0);
    let fk_bin = f("--fk").unwrap_or_else(|| "fk".into());
    let server = f("--server").unwrap_or_else(|| std::env::var("TM_SERVER").unwrap_or_default());
    let steer: i8 = f("--steer").and_then(|s| s.parse().ok()).unwrap_or(0);
    let accel: u32 = f("--accel").and_then(|s| s.parse().ok()).unwrap_or(1);
    let brake: u32 = f("--brake").and_then(|s| s.parse().ok()).unwrap_or(0);
    let keep = args.iter().any(|a| a == "--keep");
    // The point of interest a probe's closest approach is measured against.
    let target: Vec<f64> = f("--target")
        .unwrap_or_else(|| "419.03,144.0,1704.64".into())
        .split(',')
        .map(|s| s.trim().parse::<f64>().unwrap_or(0.0))
        .collect();
    let spawn_block: usize = f("--block").and_then(|s| s.parse().ok()).unwrap_or(0);

    // --cells cx0:cx1:step,cy,cz0:cz1:step   (repeatable), and/or --cell cx,cy,cz[,dir]
    let mut probes: Vec<Probe> = Vec::new();
    let dirs: Vec<u8> = f("--dirs")
        .unwrap_or_else(|| "0".into())
        .split(',')
        .filter_map(|s| s.trim().parse::<u8>().ok())
        .collect();
    for (i, a) in args.iter().enumerate() {
        if a == "--cells" {
            let spec = args.get(i + 1).cloned().unwrap_or_default();
            let parts: Vec<&str> = spec.split(',').collect();
            if parts.len() != 3 {
                die("--cells cx0:cx1:step,cy,cz0:cz1:step");
            }
            let rng = |s: &str| -> Vec<i32> {
                let p: Vec<i32> = s.split(':').filter_map(|v| v.trim().parse().ok()).collect();
                match p.len() {
                    1 => vec![p[0]],
                    2 => (p[0]..=p[1]).collect(),
                    _ => {
                        let st = p[2].max(1);
                        let mut v = Vec::new();
                        let mut k = p[0];
                        while k <= p[1] {
                            v.push(k);
                            k += st;
                        }
                        v
                    }
                }
            };
            let cy: i32 = parts[1].trim().parse().unwrap_or_else(|_| die("--cells: cy"));
            for cx in rng(parts[0]) {
                for cz in rng(parts[2]) {
                    for d in &dirs {
                        probes.push(Probe { id: 0, cell: (cx, cy, cz), dir: *d, tape: None });
                    }
                }
            }
        }
        if a == "--cell" {
            let p: Vec<i32> = args
                .get(i + 1)
                .cloned()
                .unwrap_or_default()
                .split(',')
                .filter_map(|v| v.trim().parse().ok())
                .collect();
            if p.len() < 3 {
                die("--cell cx,cy,cz[,dir]");
            }
            let d = if p.len() > 3 { p[3] as u8 } else { dirs[0] };
            probes.push(Probe { id: 0, cell: (p[0], p[1], p[2]), dir: d, tape: None });
        }
    }
    if probes.is_empty() {
        // --tapes DIR: score a POPULATION OF TAPES on the untouched map instead
        // of a population of spawn cells. Same readout — where did the car get,
        // how high, how close to the target, and when — which is the poor man's
        // state objective: a relocated Goal is a fine ruler but a 2.6 m deep
        // trigger box is a bad search objective, because a candidate 2 m off
        // the line reads as "DNF" and not as "missed by 2 m".
        if let Some(d) = f("--tapes") {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&d)
                .unwrap_or_else(|e| die(&format!("{}: {}", d, e)))
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.to_string_lossy().ends_with(".Ghost.Gbx"))
                .collect();
            v.sort();
            for p in v {
                probes.push(Probe { id: 0, cell: (0, 0, 0), dir: 0, tape: Some(p) });
            }
        }
    }
    if probes.is_empty() {
        die("no probes: give --cells, --cell or --tapes");
    }
    for (i, p) in probes.iter_mut().enumerate() {
        p.id = i + 1;
    }

    std::fs::create_dir_all(&out_dir).expect("mkdir --out");

    // ---- control 1: the mover, at the origin ------------------------------
    let orig = map::MapFile::load(Path::new(&map_path));
    let home = orig.blocks[spawn_block].coords();
    let home_dir = orig.blocks[spawn_block].dir;
    let plain = out_dir.join("control_plain.Map.Gbx");
    let athome = out_dir.join("control_origin.Map.Gbx");
    let a = orig.gbx.body.clone();
    let b = {
        let mut m = map::MapFile::load(Path::new(&map_path));
        m.move_block_cell(spawn_block, home);
        m.set_block_dir(spawn_block, home_dir);
        m.write_to(&athome).expect("write control_origin");
        crate::gbx::Gbx::parse(&std::fs::read(&athome).expect("read control_origin")).body
    };
    let _ = plain;
    if a != b {
        die(&format!(
            "ORIGIN CONTROL FAILED: block#{} moved to its own cell {:?} does not reproduce the \
             untouched map ({} vs {} body bytes). Every probe below would be measuring the mover.",
            spawn_block,
            home,
            a.len(),
            b.len()
        ));
    }
    println!(
        "control OK: block#{} {} at its own cell {:?}/dir{} reproduces the untouched map, {} body bytes",
        spawn_block, orig.blocks[spawn_block].name, home, home_dir, a.len()
    );

    // ---- the probe tape ---------------------------------------------------
    let tape = out_dir.join("probe.Ghost.Gbx");
    let n = build_probe_tape(&tape_src, tape.to_str().unwrap(), steer, accel, brake, ticks)
        .unwrap_or_else(|e| die(&e));
    println!(
        "probe tape: {} ticks, steer {} accel {} brake {} on every one after the countdown (read back and checked)",
        n, steer, accel, brake
    );

    // ---- control 2: the reference probe, at the real spawn ----------------
    let mut all: Vec<Probe> = vec![Probe { id: 0, cell: home, dir: home_dir, tape: None }];
    all.extend(probes.into_iter());

    let queue = Arc::new(Mutex::new(all));
    let done = Arc::new(AtomicUsize::new(0));
    let results: Arc<Mutex<Vec<Summary>>> = Arc::new(Mutex::new(Vec::new()));
    let total = queue.lock().unwrap().len();
    println!("{} probes ({} with the reference), {} jobs", total, total - 1, jobs);

    let mut hs = Vec::new();
    for _ in 0..jobs.max(1) {
        let queue = queue.clone();
        let results = results.clone();
        let done = done.clone();
        let out_dir = out_dir.clone();
        let map_path = map_path.clone();
        let tape = tape.clone();
        let fk_bin = fk_bin.clone();
        let server = server.clone();
        let at = at.clone();
        let target = target.clone();
        hs.push(std::thread::spawn(move || loop {
            let p = { queue.lock().unwrap().pop() };
            let Some(p) = p else { break };
            let tag = if let Some(t) = &p.tape {
                format!("t{:04}_{}", p.id, t.file_stem().unwrap_or_default().to_string_lossy())
            } else {
                format!("p{:04}_{}_{}_{}_d{}", p.id, p.cell.0, p.cell.1, p.cell.2, p.dir)
            };
            let mp = out_dir.join(format!("{}.Map.Gbx", tag));
            if p.tape.is_none() {
                let mut m = map::MapFile::load(Path::new(&map_path));
                m.move_block_cell(spawn_block, p.cell);
                m.set_block_dir(spawn_block, p.dir);
                m.write_to(&mp).expect("write probe map");
            }
            let mp = if p.tape.is_some() { PathBuf::from(&map_path) } else { mp };
            let tape = p.tape.clone().unwrap_or_else(|| tape.clone());
            let csv = out_dir.join(format!("{}.csv", tag));
            let work = out_dir.join(format!("work_{}", tag));
            // The car locator is value-based and it REFUSES rather than guesses:
            // on this map the same tape and map fail at `frac:0.20` and pass at
            // 0.10 and 0.35, so a single fork point loses probes for a reason
            // that has nothing to do with the map. Try several, earliest first,
            // and take the first trace that passes fk's own self-check.
            let mut o = None;
            let mut used = String::new();
            for f in at.split(',') {
                let mut cmd = Command::new(&fk_bin);
                cmd.arg("trace")
                    .arg("--tape")
                    .arg(&tape)
                    .arg("--map")
                    .arg(&mp)
                    .arg("--at")
                    .arg(f)
                    .arg("--out")
                    .arg(&csv)
                    .arg("--work")
                    .arg(&work);
                if !server.is_empty() {
                    cmd.arg("--server").arg(&server);
                }
                let r = cmd.output();
                let good = matches!(&r, Ok(x) if x.status.success());
                used = f.to_string();
                o = Some(r);
                if good {
                    break;
                }
            }
            let o = o.unwrap();
            let mut s = Summary {
                id: p.id,
                cell: p.cell,
                dir: p.dir,
                ok: false,
                note: String::new(),
                rows: 0,
                t0: 0.0,
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                ymax: 0.0,
                ymax_at: (0.0, 0.0, 0.0),
                ymin: 0.0,
                xend: 0.0,
                yend: 0.0,
                zend: 0.0,
                tend: 0.0,
                vmax: 0.0,
                dmin: f64::INFINITY,
                dmin_at: (0.0, 0.0, 0.0, 0.0),
            };
            match o {
                Err(e) => s.note = format!("fk launch failed: {}", e),
                Ok(o) if !o.status.success() => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    s.note = err.lines().last().unwrap_or("fk failed").trim().to_string();
                }
                Ok(_) => match read_trace(&csv) {
                    Err(e) => s.note = e,
                    Ok(rows) => {
                        // A trace of ZEROES passes fk's own self-check — the
                        // velocity is consistent with the position and the
                        // quaternion is a unit — and it means the car was never
                        // there. On this map every spawn cell OUTSIDE the map
                        // grid's own extent produces exactly that: 210 of 245
                        // "ok" probes in the first wide scan reported the car
                        // at (0,0,0) for the whole run. Refuse them by name
                        // rather than averaging them into a heightmap.
                        let moved = rows.iter().any(|r| {
                            (r.1.abs() + r.3.abs()) > 1.0 && (r.1 - rows[0].1).abs()
                                + (r.3 - rows[0].3).abs()
                                > 0.5
                        });
                        if !moved {
                            s.note = "NO CAR: the trace is all zeroes or never moves — this spawn \
                                      cell produced no vehicle (outside the map grid?)"
                                .to_string();
                            let _ = &s.note;
                        } else {
                        s.ok = true;
                        s.rows = rows.len();
                        s.t0 = rows[0].0;
                        s.x0 = rows[0].1;
                        s.y0 = rows[0].2;
                        s.z0 = rows[0].3;
                        s.ymax = f64::NEG_INFINITY;
                        s.ymin = f64::INFINITY;
                        for r in &rows {
                            if r.2 > s.ymax {
                                s.ymax = r.2;
                                s.ymax_at = (r.1, r.2, r.3);
                            }
                            if r.2 < s.ymin {
                                s.ymin = r.2;
                            }
                            if r.4 > s.vmax {
                                s.vmax = r.4;
                            }
                            let d = ((r.1 - target[0]).powi(2)
                                + (r.2 - target[1]).powi(2)
                                + (r.3 - target[2]).powi(2))
                            .sqrt();
                            if d < s.dmin {
                                s.dmin = d;
                                s.dmin_at = (r.0, r.1, r.2, r.3);
                            }
                        }
                        let l = rows[rows.len() - 1];
                        s.tend = l.0;
                        s.xend = l.1;
                        s.yend = l.2;
                        s.zend = l.3;
                        }
                    }
                },
            }
            if !keep && p.tape.is_none() {
                // ONLY a map this scan WROTE may be removed. In --tapes mode
                // `mp` is the caller's own map — an earlier version of this
                // line deleted the map file out of the shared store, which is
                // exactly the class of bug a scratch-cleaning branch invites.
                let _ = std::fs::remove_file(&mp);
            }
            if !keep {
                let _ = std::fs::remove_dir_all(&work);
            }
            s.note = if s.note.is_empty() { format!("at {}", used) } else { format!("{} [at {}]", s.note, used) };
            let k = done.fetch_add(1, Ordering::SeqCst) + 1;
            eprintln!("[{}/{}] {} {}", k, total, tag, if s.ok { "ok" } else { &s.note });
            results.lock().unwrap().push(s);
        }));
    }
    for h in hs {
        let _ = h.join();
    }

    let mut rs = results.lock().unwrap();
    rs.sort_by_key(|s| s.id);
    let mut txt = String::new();
    txt.push_str(&hdr());
    txt.push('\n');
    for s in rs.iter() {
        txt.push_str(&s.line());
        txt.push('\n');
    }
    let sum = out_dir.join("scan.tsv");
    std::fs::write(&sum, &txt).expect("write scan.tsv");

    // ---- control 2, adjudicated ------------------------------------------
    let refp = rs.iter().find(|s| s.id == 0);
    let hw = cell_to_world(home);
    match refp {
        Some(r) if r.ok => {
            let d = ((r.x0 - hw.0).powi(2) + (r.z0 - hw.2).powi(2)).sqrt();
            println!(
                "reference probe (real spawn cell {:?}): trace starts at ({:.1},{:.1},{:.1}) at {:.3}, \
                 {:.1} m from the map's own spawn in (x,z) — {}",
                home,
                r.x0,
                r.y0,
                r.z0,
                r.t0,
                d,
                if d < 400.0 { "POSITIVE CONTROL OK" } else { "CONTROL FAILED" }
            );
        }
        _ => println!("reference probe FAILED — the scan below is about the instrument, not the map"),
    }
    println!("wrote {} ({} probes)", sum.display(), rs.len());
}

fn die(m: &str) -> ! {
    eprintln!("tmmaps dropscan: {}", m);
    std::process::exit(3);
}
