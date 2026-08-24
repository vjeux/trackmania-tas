//! `tmauto cpladder` — **when** did the car cross each checkpoint, asked with
//! nothing but the dedicated server.
//!
//! # The idea
//!
//! The validator simulates until the **declared time**, not until the tape
//! ends. That is normally a trap (a container declaring 0 stops at race 2.500
//! however long its tape is). Turned around, it is a clock:
//!
//! > Hold the tape byte-identical and sweep only `declared_ms`. The checkpoint
//! > count the server reports at each rung is the number of gates the car had
//! > crossed **by that instant**. The rung where the count steps from `k` to
//! > `k+1` brackets the crossing time of gate `k+1`.
//!
//! Nothing here reads process memory. It shares no source with `fk::locate`,
//! with the fork server, with `progress()`, or with agent B's route — which is
//! the point: every instrument that was available when agent C's `cps 3` was
//! retracted ultimately read the same bytes out of the same address space.
//!
//! # What it can decide
//!
//! | shape of the ladder | reading |
//! |---|---|
//! | count rises in steps spread across the run | the car **drove** — it was somewhere else at each earlier rung |
//! | count is already at its maximum on the first rung | the car began **on top of** the gates; it did not drive to them |
//! | count never rises | the tape collects nothing at any horizon |
//!
//! The first two are the two hypotheses about agent C's retracted run, and they
//! are distinguishable *before* the run. The third is a null and is reported as
//! one.
//!
//! # Controls that ship with it
//!
//! * **negative, same batch** — the identical ladder over a full-throttle
//!   zero-steer tape. It must stay at zero. A ladder that rises for *any* tape
//!   is measuring the declared time, not the driving.
//! * **monotonicity** — checkpoint counts must not fall as the horizon grows.
//!   A non-monotone ladder means the declared time is doing something other
//!   than bounding the simulation, and the instrument is UNMEASURED.
//! * **saturation** — the top rung must be long enough that the count has
//!   stopped rising, or the last step is an artefact of the horizon.
//!
//! # The frame, and why `--prefix` is not optional
//!
//! A banked search tape's tick 0 is **file tick `prefix`**, not file tick 0:
//! the fork server resumes at a probed boundary and everything below it is the
//! container's own inputs. A one-tick shift of a whole tape has already turned
//! a confirmed `cps 3` into `cps 0` in this project. So the frame is part of
//! the artifact, and this command will not guess it: `--prefix sweep` measures
//! it instead, by asking which offset reproduces the banked verdict.

use std::path::{Path, PathBuf};
use tmauto::oracle::{self, Maps};
use tmauto::synth::{self, ChunkSet, GhostMeta};
use tmauto::tape::Input;

/// The explorer's container template: full throttle, and a steer cycle whose
/// 25 values have mean zero. Reproduced here from the same formula rather than
/// read out of a file, so this command depends on no artefact of a past run.
///
/// The throttle is ON deliberately. `locate_blind` finds the vehicle state by
/// velocity consistency and a parked car is indistinguishable from any other
/// constant region of memory — that is why the template drives. It also means
/// the ticks BELOW the search tape's frame are not neutral, which is exactly
/// why the frame has to be recorded.
pub fn template_inputs(ticks: usize) -> Vec<Input> {
    (0..ticks)
        .map(|t| Input {
            steer: ((t as u64 * 7919 + 13) % 25) as i8 - 12,
            gas: true,
            brake: false,
            respawn: false,
        })
        .collect()
}

/// Lay a search tape into the container's tick frame, exactly as the explorer's
/// `EngineOracle::to_inputs` does: the template underneath, the tape written at
/// `prefix`, and the template again past its end.
pub fn lay(tape: &[Input], prefix: usize, ticks: usize) -> Vec<Input> {
    let mut v = template_inputs(ticks);
    for (i, t) in tape.iter().enumerate() {
        let j = prefix + i;
        if j >= ticks {
            break;
        }
        v[j] = *t;
    }
    v
}

/// `tick<TAB>steer<TAB>gas<TAB>brake`, the form the explorer banks.
pub fn read_tape(p: &Path) -> Result<Vec<Input>, String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("tick") {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            return Err(format!("{}:{}: expected 4 columns, got {}", p.display(), n + 1, f.len()));
        }
        // The tick column is not decoration: a tape with a hole in it would
        // otherwise be laid down compacted, which is a different run.
        let tick: usize = f[0].parse().map_err(|_| format!("{}:{}: bad tick", p.display(), n + 1))?;
        if tick != out.len() {
            return Err(format!(
                "{}:{}: tick column says {} but this is row {}. The tape has a gap or is \
                 out of order; laying it down anyway would silently produce a different run.",
                p.display(),
                n + 1,
                tick,
                out.len()
            ));
        }
        out.push(Input {
            steer: f[1].parse().map_err(|_| format!("{}:{}: bad steer", p.display(), n + 1))?,
            gas: f[2] != "0",
            brake: f[3] != "0",
            respawn: f.get(4).map(|s| *s != "0").unwrap_or(false),
        });
    }
    if out.is_empty() {
        return Err(format!("{}: no input rows", p.display()));
    }
    Ok(out)
}

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// Build one container and return its path.
#[allow(clippy::too_many_arguments)]
fn emit(
    dir: &Path,
    name: &str,
    uid: &str,
    inputs: &[Input],
    declared_ms: u32,
) -> Result<PathBuf, String> {
    let mut meta = GhostMeta::probe(uid);
    // The count we author is not evidence about anything, so it is a knob and
    // not a constant: `TMAUTO_DCPS=n` re-runs an identical experiment with a
    // different authored count. A reading that survives that is not a reading
    // of our own declaration.
    let n: usize =
        std::env::var("TMAUTO_DCPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let cps: Vec<i32> = (1..=n)
        .map(|i| (declared_ms as i32 / (n as i32 + 1)) * i as i32)
        .chain(std::iter::once(declared_ms as i32))
        .collect();
    meta.set_declared(declared_ms, cps);
    let bytes = synth::synthesize(inputs, &meta, &ChunkSet::ALL);
    let p = dir.join(format!("{}.Ghost.Gbx", name));
    std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
    Ok(p)
}

/// One rung's answer, kept with its horizon.
struct Rung {
    declared_ms: u32,
    cps: Option<u32>,
    time_ms: Option<i64>,
    desc: String,
}

/// The (authored count) × (tape cut) matrix.
///
/// # Why this exists
///
/// The truncation ladder was cross-checked against the two container knobs
/// that could plausibly move it. Archive length did nothing — 5000 ticks and
/// 9000 ticks give the identical ladder. **The authored checkpoint count did
/// something**: at `TMAUTO_DCPS=5` every intermediate rung collapsed to zero
/// while the endpoints agreed.
///
/// `DeclaredResult.NbCheckpoints` is a number WE write into the file. It is not
/// evidence and it must not be able to change a measurement. That it can is
/// either (a) the server reporting differently, or (b) the server *simulating*
/// differently — and those are very different facts. This matrix is the thing
/// that distinguishes them, because a report-level effect cannot change where
/// the count first becomes non-zero as a function of the DRIVING, whereas a
/// simulation-level effect can.
///
/// Read the rows: if every row has the same *shape* (0 for a while, then
/// rising) and they differ only in level, the count is being filtered. If one
/// row is flat at zero until the very end, that row's container is not
/// simulating the same run.
fn matrix(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let tape_path = PathBuf::from(arg(args, "--tape").ok_or("--tape is required")?);
    let ticks: usize =
        arg(args, "--ticks").unwrap_or_else(|| "9000".into()).parse().map_err(|_| "--ticks")?;
    let prefix: usize =
        arg(args, "--prefix").ok_or("--prefix is required for the matrix")?.parse().map_err(|_| "--prefix")?;
    let out = PathBuf::from(arg(args, "--out").unwrap_or_else(|| "/tmp/c2/cpmatrix".into()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let uid = crate::map_uid(&map)?;
    let tape = read_tape(&tape_path)?;

    let dcps: Vec<usize> = arg(args, "--dcps")
        .unwrap_or_else(|| "0,1,2,3,4,5,6".into())
        .split(',')
        .map(|s| s.trim().parse().map_err(|_| "--dcps"))
        .collect::<Result<_, _>>()?;
    let ks: Vec<usize> = arg(args, "--ks")
        .unwrap_or_else(|| "0,400,800,1200,1600,2000,2400,2800,3200,3600,4000".into())
        .split(',')
        .map(|s| s.trim().parse().map_err(|_| "--ks"))
        .collect::<Result<_, _>>()?;

    println!("MAP   {}  ({})", map.display(), uid);
    println!("TAPE  {} ({} ticks), prefix {}, container {} ticks\n", tape_path.display(), tape.len(), prefix, ticks);

    let mut files = Vec::new();
    let mut cells = Vec::new();
    for d in &dcps {
        for k in &ks {
            let mut inputs = template_inputs(ticks);
            for (i, t) in tape.iter().take(*k).enumerate() {
                if prefix + i < ticks {
                    inputs[prefix + i] = *t;
                }
            }
            for slot in inputs.iter_mut().skip(prefix + *k) {
                *slot = Input { steer: 0, gas: false, brake: false, respawn: false };
            }
            let h = ((prefix + *k) as u32) * 10 + 200;
            let mut meta = GhostMeta::probe(&uid);
            let cps: Vec<i32> = (1..=*d)
                .map(|i| (h as i32 / (*d as i32 + 1)) * i as i32)
                .chain(std::iter::once(h as i32))
                .collect();
            meta.set_declared(h, cps);
            let bytes = synth::synthesize(&inputs, &meta, &ChunkSet::ALL);
            let p = out.join(format!("m_d{}_k{}.Ghost.Gbx", d, k));
            std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
            files.push(p);
            cells.push((*d, *k));
        }
    }
    let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), "cpmatrix")?;
    std::fs::write(
        out.join("transcript_matrix.txt"),
        format!("===== stdout =====\n{}\n===== stderr =====\n{}\n", b.raw, b.err),
    )
    .map_err(|e| e.to_string())?;

    print!("{:>10}", "dcps \\ k");
    for k in &ks {
        print!("{:>7}", k);
    }
    println!();
    let mut rows: Vec<(usize, Vec<u32>)> = Vec::new();
    for d in &dcps {
        print!("{:>10}", d);
        let mut row = Vec::new();
        for k in &ks {
            let i = cells.iter().position(|c| c == &(*d, *k)).unwrap();
            let a = b.by_name(files[i].file_name().unwrap().to_str().unwrap());
            let c = a.map(|a| a.cps.unwrap_or(0)).unwrap_or(0);
            row.push(c);
            print!("{:>7}", c);
        }
        println!();
        rows.push((*d, row));
    }

    println!("\n--- reading ---");
    for (d, row) in &rows {
        let first_nonzero = row.iter().position(|c| *c > 0).map(|i| ks[i]);
        println!(
            "  dcps {}: first non-zero at k = {}, final = {}",
            d,
            first_nonzero.map(|k| k.to_string()).unwrap_or_else(|| "never".into()),
            row.last().unwrap()
        );
    }
    let finals: Vec<u32> = rows.iter().map(|(_, r)| *r.last().unwrap()).collect();
    if finals.windows(2).all(|w| w[0] == w[1]) {
        println!(
            "\n  PASS  every authored count agrees on the FULL tape ({}). The authored number \
             does not change the answer at full length.",
            finals[0]
        );
    } else {
        println!(
            "\n  FAIL  the authored count changes the answer even on the full tape: {:?}. \
             Nothing in this matrix is a measurement of driving.",
            finals
        );
    }
    println!("\ntranscript banked in {}", out.display());
    Ok(())
}

fn ladder(
    dir: &Path,
    map: &Path,
    uid: &str,
    label: &str,
    inputs: &[Input],
    horizons: &[u32],
    tag: &str,
) -> Result<Vec<Rung>, String> {
    let mut files = Vec::new();
    for d in horizons {
        files.push(emit(dir, &format!("{}_{}", label, d), uid, inputs, *d)?);
    }
    let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(map), tag)?;
    std::fs::write(
        dir.join(format!("transcript_{}.txt", label)),
        format!("===== stdout =====\n{}\n===== stderr =====\n{}\n", b.raw, b.err),
    )
    .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (d, f) in horizons.iter().zip(&files) {
        let a = b.by_name(f.file_name().unwrap().to_str().unwrap());
        out.push(Rung {
            declared_ms: *d,
            // `cps` is what the server SIMULATED. On a bare `wrong simu` the
            // count line is absent and the honest reading is zero collected --
            // but that is a parse of prose, so the desc travels with it.
            cps: a.map(|a| a.cps.unwrap_or(0)),
            time_ms: a.and_then(|a| a.time_ms),
            desc: a.map(|a| a.desc.trim().replace('\n', " | ")).unwrap_or_else(|| "NOT READ".into()),
        });
    }
    Ok(out)
}

pub fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--matrix") {
        return matrix(args);
    }
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let tape_path = PathBuf::from(arg(args, "--tape").ok_or("--tape is required")?);
    let ticks: usize =
        arg(args, "--ticks").unwrap_or_else(|| "9000".into()).parse().map_err(|_| "--ticks")?;
    let from: u32 = arg(args, "--from").unwrap_or_else(|| "3000".into()).parse().map_err(|_| "--from")?;
    let to: u32 = arg(args, "--to").unwrap_or_else(|| "60000".into()).parse().map_err(|_| "--to")?;
    let step: u32 = arg(args, "--step").unwrap_or_else(|| "3000".into()).parse().map_err(|_| "--step")?;
    let out = PathBuf::from(arg(args, "--out").unwrap_or_else(|| "/tmp/c2/cpladder".into()));
    let prefix_arg = arg(args, "--prefix").unwrap_or_else(|| "sweep".into());

    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let uid = crate::map_uid(&map)?;
    let tape = read_tape(&tape_path)?;

    println!("MAP      {}  ({})", map.display(), uid);
    println!("TAPE     {}  ({} ticks = {} of race)", tape_path.display(), tape.len(), secs(tape.len() as u32 * 10));
    println!("CONTAINER {} ticks = {} of race\n", ticks, secs(ticks as u32 * 10));

    let horizons: Vec<u32> = (from..=to).step_by(step as usize).collect();

    // ---- the frame ----
    let prefix: usize = if prefix_arg == "sweep" {
        println!("FRAME    --prefix sweep: the banked tape does not carry its frame, so it is");
        println!("         MEASURED. Same tape, same long horizon, only the offset moves.\n");
        let cands: Vec<usize> = vec![0, 60, 74, 100, 150, 152, 153, 154, 155, 200, 300];
        let mut files = Vec::new();
        for p in &cands {
            files.push(emit(&out, &format!("frame_{}", p), &uid, &lay(&tape, *p, ticks), to)?);
        }
        let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), "frame")?;
        std::fs::write(
            out.join("transcript_frame.txt"),
            format!("===== stdout =====\n{}\n===== stderr =====\n{}\n", b.raw, b.err),
        )
        .map_err(|e| e.to_string())?;
        let mut best: Option<(usize, u32)> = None;
        println!("{:>8}  {:>4}  {}", "prefix", "cps", "desc");
        for (p, f) in cands.iter().zip(&files) {
            let a = b.by_name(f.file_name().unwrap().to_str().unwrap());
            let c = a.map(|a| a.cps.unwrap_or(0)).unwrap_or(0);
            println!(
                "{:>8}  {:>4}  {}",
                p,
                c,
                a.map(|a| a.desc.trim().replace('\n', " | ")).unwrap_or_else(|| "NOT READ".into())
            );
            if best.map(|(_, bc)| c > bc).unwrap_or(true) {
                best = Some((*p, c));
            }
        }
        let (p, c) = best.ok_or("the frame sweep read nothing at all")?;
        println!("\n  best frame: prefix {} with cps {}", p, c);
        if c == 0 {
            return Err(
                "no offset in the sweep produced a single checkpoint. Either this tape is not \
                 for this map/template, or the frame is outside the swept range. Reporting \
                 UNMEASURED rather than picking the first row."
                    .into(),
            );
        }
        p
    } else {
        prefix_arg.parse().map_err(|_| "--prefix must be a number or `sweep`")?
    };

    println!("\nFRAME    prefix {} — the tape's tick 0 is file tick {}\n", prefix, prefix);

    let laid = lay(&tape, prefix, ticks);
    let straight: Vec<Input> = vec![Input { steer: 0, gas: true, brake: false, respawn: false }; ticks];

    // ================= the TRUNCATION ladder =================
    //
    // The horizon ladder below moves `declared_ms`, and its monotonicity
    // control caught that doing so does not merely bound the simulation. This
    // one truncates the TAPE and clamps the HORIZON to the truncation point
    // together, so that:
    //
    //  * nothing after tick k is ever simulated — whatever fills the rest of
    //    the archive cannot matter, which removes the padding from the
    //    experiment entirely;
    //  * `cps(k)` is exactly the gates crossed in the first k ticks;
    //  * the ladder is monotone BY CONSTRUCTION, because physics is causal.
    //    So a non-monotone reading is an instrument fault and is reported as
    //    one rather than interpreted.
    //
    // The first version of this padded with FULL BRAKE and its comment said the
    // car "slides to a stop". It does not: brake with no throttle puts a
    // Stadium car into REVERSE, so the residue was forty seconds of driving
    // backwards and the ladder came back non-monotone. That was an invented
    // mechanism, caught by its own control. The padding is now neutral AND
    // never simulated.
    //
    // The reading that matters is **cps(0)**: at k = 0 the search tape
    // contributes nothing at all and only the container's own 1.53 s of
    // template driving has happened. A checkpoint credited there is a gate
    // within a second and a half of wherever the car begins. No memory is read
    // to say so.
    if arg(args, "--no-truncate").is_none() {
        let tt: Vec<usize> = {
            let n = tape.len();
            let st: usize = arg(args, "--tick-step")
                .unwrap_or_else(|| "200".into())
                .parse()
                .map_err(|_| "--tick-step")?;
            let mut v: Vec<usize> = vec![0, 25, 50, 100];
            v.extend((0..=n).step_by(st));
            if *v.last().unwrap() != n {
                v.push(n);
            }
            v.sort_unstable();
            v.dedup();
            v
        };
        let mut files = Vec::new();
        let mut hs = Vec::new();
        for k in &tt {
            let mut inputs = template_inputs(ticks);
            // the container's own prefix stays exactly as the search saw it
            for (i, t) in tape.iter().take(*k).enumerate() {
                if prefix + i < ticks {
                    inputs[prefix + i] = *t;
                }
            }
            // Neutral past the cut, and past the horizon as well: belt and
            // braces, because the two claims ("it is neutral" and "it is never
            // simulated") fail differently.
            for slot in inputs.iter_mut().skip(prefix + *k) {
                *slot = Input { steer: 0, gas: false, brake: false, respawn: false };
            }
            // Stop the validator at the cut. +200 ms of slack so the boundary
            // tick itself is inside the window rather than exactly on it.
            let h = ((prefix + *k) as u32) * 10 + 200;
            hs.push(h);
            files.push(emit(&out, &format!("trunc_{}", k), &uid, &inputs, h)?);
        }
        let b = oracle::validate_raw(&oracle::server_dir(), &files, Maps::One(&map), "trunc")?;
        std::fs::write(
            out.join("transcript_trunc.txt"),
            format!("===== stdout =====\n{}\n===== stderr =====\n{}\n", b.raw, b.err),
        )
        .map_err(|e| e.to_string())?;
        println!(
            "=== TRUNCATION LADDER (tape cut at k, neutral after, horizon clamped to the cut) ==="
        );
        println!("{:>8} {:>9} {:>9}  {:>4}  {}", "k ticks", "= race", "horizon", "cps", "server said");
        let mut cs: Vec<u32> = Vec::new();
        for ((k, h), f) in tt.iter().zip(&hs).zip(&files) {
            let a = b.by_name(f.file_name().unwrap().to_str().unwrap());
            let c = a.map(|a| a.cps.unwrap_or(0)).unwrap_or(0);
            cs.push(c);
            println!(
                "{:>8} {:>9} {:>9}  {:>4}  {}",
                k,
                secs(*k as u32 * 10),
                secs(*h),
                c,
                a.map(|a| a.desc.trim().replace('\n', " | ")).unwrap_or_else(|| "NOT READ".into())
            );
        }
        println!("\n--- truncation controls ---");
        if cs.windows(2).all(|w| w[1] >= w[0]) {
            println!("  PASS  monotone — as it must be if the count is caused by the tape");
        } else {
            let bad: Vec<String> = cs
                .windows(2)
                .enumerate()
                .filter(|(_, w)| w[1] < w[0])
                .map(|(i, w)| format!("k {}→{} : {}→{}", tt[i], tt[i + 1], w[0], w[1]))
                .collect();
            println!(
                "  FAIL  NOT monotone ({}). Extending a tape removed a checkpoint a shorter \
                 prefix of the SAME tape had, with the same horizon rule. That cannot be \
                 caused by driving; UNMEASURED.",
                bad.join(", ")
            );
        }
        match cs.first().copied() {
            Some(0) => println!(
                "  PASS  k = 0 (the search tape contributes nothing) scores nothing — \
                 the car does not begin on or beside a gate"
            ),
            Some(n) => println!(
                "  *** k = 0 SCORES {} CHECKPOINT(S) at a horizon of {}. The search tape \
                 contributes NOTHING and the car is still credited. Either a gate sits within \
                 {} of the start, or the count includes something that is not a checkpoint. ***",
                n,
                secs(hs[0]),
                secs(hs[0])
            ),
            None => println!("  FAIL  k = 0 was not read — UNMEASURED"),
        }
        println!();
    }

    let rows = ladder(&out, &map, &uid, "tape", &laid, &horizons, "cpl")?;
    let ctrl = ladder(&out, &map, &uid, "straight", &straight, &horizons, "cplc")?;

    println!("{:>10}  {:>4}  {:>4}   {}", "horizon", "cps", "ctrl", "server said");
    for (r, c) in rows.iter().zip(&ctrl) {
        println!(
            "{:>10}  {:>4}  {:>4}   {}",
            secs(r.declared_ms),
            r.cps.map(|c| c.to_string()).unwrap_or_else(|| "-".into()),
            c.cps.map(|c| c.to_string()).unwrap_or_else(|| "-".into()),
            r.desc
        );
    }

    // ---- controls ----
    println!("\n--- controls ---");
    let cs: Vec<u32> = rows.iter().map(|r| r.cps.unwrap_or(0)).collect();
    let monotone = cs.windows(2).all(|w| w[1] >= w[0]);
    if monotone {
        println!("  PASS  the count never falls as the horizon grows");
    } else {
        println!(
            "  FAIL  the count FALLS somewhere in this ladder. The declared time is not \
             simply bounding the simulation and every reading here is UNMEASURED."
        );
    }
    if ctrl.iter().all(|r| r.cps.unwrap_or(0) == 0) {
        println!("  PASS  the negative control (full throttle, no steer) stays at zero at every horizon");
    } else {
        println!(
            "  FAIL  the negative control collected a checkpoint. This ladder is measuring \
             the horizon, not the driving — UNMEASURED."
        );
    }
    let saturated = cs.len() >= 2 && cs[cs.len() - 1] == cs[cs.len() - 2];
    if saturated {
        println!("  PASS  the top of the ladder has stopped rising — the last step is not a horizon artefact");
    } else {
        println!("  note  the count was still rising at the top rung; raise --to before reading the last step");
    }

    // ---- the reading ----
    println!("\n--- crossing times, bracketed ---");
    let mut prev = 0u32;
    let mut steps = 0;
    for r in &rows {
        let c = r.cps.unwrap_or(0);
        if c > prev {
            println!(
                "  gate {} .. {} crossed between {} and {}",
                prev + 1,
                c,
                secs(r.declared_ms.saturating_sub(step)),
                secs(r.declared_ms)
            );
            steps += 1;
            prev = c;
        }
    }
    if steps == 0 {
        println!("  the count never rose. This tape collects nothing at any horizon tested.");
    } else if rows.first().map(|r| r.cps.unwrap_or(0)).unwrap_or(0) == prev && prev > 0 {
        println!(
            "\n  READING: the count was ALREADY at its maximum ({}) on the very first rung ({}).\n  \
             The car did not drive to these gates — it began on top of them.",
            prev,
            secs(from)
        );
    } else {
        println!(
            "\n  READING: the count rose in {} separate steps spread across the run, ending at {}.\n  \
             A car that began on top of the gates would have scored them all on the first rung.\n  \
             This one did not: it was somewhere else at each earlier horizon. THE CAR DROVE.",
            steps, prev
        );
    }
    println!("\ntranscripts banked in {}", out.display());
    Ok(())
}
