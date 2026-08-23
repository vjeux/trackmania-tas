//! `tmsearch` -- the command line.
//!
//! ```text
//! tmsearch search   --template G --map M [--fork ...] [flags]
//! tmsearch dump     --template G --map M --n N [--out F.jsonl]
//! tmsearch analyze  --log F.jsonl --base SECONDS
//! tmsearch validate --map M GHOST...
//! ```
//!
//! Times print as seconds with a decimal, and every flag that takes a time
//! takes seconds too (`--temp 0.030`, `--base 23.000`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tmsearch::analyze;
use tmsearch::batch::BatchEval;
use tmsearch::forkeval::{calibrate_boundary, clock_for_tick, ForkEval, ForkSetup};
use tmsearch::guard::{Bank, Provenance};
use forkoracle::inputs::{mutate, Inputs, OpSet, Rng};
use tmsearch::report::secs;
use tmsearch::root::Root;
use tmsearch::score::{Outcome, Progress};
use tmsearch::search::{Config, Evaluator, OpsPerCandidate};
use tmsearch::tape::Patcher;

const USAGE: &str = r#"tmsearch -- the TAS search for Trackmania 2020

  search    --template G.Ghost.Gbx --map M.Map.Gbx [flags]
            Islands of independent incumbents, annealed, over the oracle.
            EVERY banked improvement is re-validated by the plain oracle
            before it is accepted; a disagreement stops the run and keeps
            the tape as PHANTOM_*.

  dump      --template G --map M --n N [--out F.jsonl]
            Evaluate N candidates from a FIXED incumbent and record what
            each operator earned. Nothing is accepted; the sample is
            unbiased, which a live search's log is not.

  analyze   --log F.jsonl --base SECONDS
            What one search step buys: by operator, by tick, and the
            best-of-k curve that sizes --batch.

  validate  --map M GHOST...
            Ask the plain oracle. Prints the time it SIMULATED and, when
            they differ, the time the file DECLARES.

WHERE
  --server DIR        the dedicated server (default $TM_SERVER)
  --root DIR          candidate scratch; per-pid by default, and claimed
  --bestdir DIR       where confirmed improvements land (default ./best)
  --log FILE          JSONL: every confirmation and every phantom

SEARCH
  --start-from G      begin from this tape instead of the template's own
  --seg K:MAP         segment map ending at checkpoint K (repeatable)
  --workers N         default: all cores
  --batch N           candidates per oracle call (default 30)
  --nops N            operators per candidate, or --nops-upto N
  --ops SET           local | wide | doublet | retime | scale
  --lo T --hi T       tick range the search may edit
  --window N --stride N --full-window-every N
  --minutes M --seed S
  --temp SECONDS      Metropolis temperature; 0 is improvement-only
  --migrate P         chance a worker reseeds from the global best
  --max-drift N       stop once a banked result is N ticks from the fork's
                      reference, so it can be re-anchored (0 = never)

FORK MODE (a gradient, never a result)
  --fork              evaluate on mid-simulation fork servers
  --forktick T        where to checkpoint
  --refghost G        the reference line from G's OWN telemetry, accepted only
                      if that telemetry can be shown to belong to G's tape
  --refcsv F          the reference line as a measured trajectory (fk btraj)
  --shim F            libforkshim.so
  --pred SPEC         watchdog condition (repeatable)
  --finishmargin M    disarm predicates within M metres of the finish
  --corridor M        how far off the line still counts as progress

THE STATE OBJECTIVE (--fork only: the plain oracle cannot see the car)
  --gate SPEC         xmin=..,xmax=..,ymin=..,ymax=..,zmin=..,zmax=..[,minspeed=..]
                      A box the car's whole state is recorded in. Score the
                      STATE at a place when finish time cannot cross the valley.
  --gate-key EXPR     what to maximise inside it, over the WHOLE state:
                        speed vx vy vz px py pz
                        bodyright bodyup bodyfwd      (velocity in the car's frame)
                        omega omegax omegay omegaz    (BODY-frame rate, deg/s)
                        domega                        (its change per tick --
                                                       a free rigid body holds
                                                       omega exactly constant,
                                                       so this is a LOAD detector)
                        along(x,y,z) nose(x,y,z) roof(x,y,z) flank(x,y,z)
                        dist(x,y,z) vdist(vx,vy,vz)
                        abs() min() max()  + - * /
                      e.g. --gate-key 'min(abs(bodyright), 5*(-vz))'
  --gate-min-key K    how good the state must be before the bands above
                      "reached" are available at all. Without it, a tape that
                      clips the box and finishes tops the ranking whatever it
                      did there — and on a gate that sits on a line everybody
                      drives, that is the seed, and no state hunt can beat it.
                      `auto` derives a floor from the seed's own measured key,
                      which is a FLOOR and not a target: see below.
  --gate-seed-state G check the fork's measured gate state for the seed against
                      G's own decoded telemetry -- position, velocity AND
                      attitude. In gate mode this replaces the millisecond
                      identity check, and it is stronger.

THE EVENT (a gate is a place; some things are events)
  --fire EXPR         a condition over the same terms, plus `dspeed` -- the
                      one-tick rise in speed, which is what a launch is. The
                      FIRST tick it holds is the event.
                      e.g. --fire dspeed --fire-at 10
  --fire-at K         the value EXPR must reach
  --fire-where SPEC   a box the event must happen inside, same six bounds as
                      --gate. A launch upstream of a checkpoint you still have
                      to collect is a launch that cannot validate.
  --fire-need N       consecutive ticks the condition must hold. A load
                      detector needs this: `domega` is near zero for one tick
                      whenever the car happens not to be turning, and what
                      makes a free rigid body is that it STAYS there.
  --after-key EXPR    what to maximise AFTER the event, over the ticks strictly
                      after it. Measured only after, which is the point:
                      "closest approach to the finish" measured from tick 0
                      pins every candidate at whatever the ordinary route
                      already passes within.
                      e.g. --after-key '-dist(366,50,736)'
  --after-ticks N     bound that window to N ticks. A window whose end the
                      CANDIDATE chooses is a decoy the instrument builds.
  --after-from S      open that window at the event (`start`, default) or at
                      the END of the run that fired (`end`) -- "where did it
                      come back", not "what happened next".
"#;

fn die(m: impl AsRef<str>) -> ! {
    eprintln!("tmsearch: {}", m.as_ref());
    std::process::exit(2)
}

struct Args {
    cmd: String,
    free: Vec<String>,
    template: String,
    start_from: Option<String>,
    map: Option<String>,
    segs: Vec<(u32, PathBuf)>,
    server: Option<String>,
    root: Option<String>,
    bestdir: String,
    log: Option<String>,
    workers: usize,
    batch: usize,
    nops: OpsPerCandidate,
    ops: OpSet,
    lo: usize,
    hi: usize,
    window: usize,
    stride: usize,
    full_window_every: u64,
    minutes: f64,
    seed: u64,
    temp_s: f64,
    migrate: f64,
    max_drift: usize,
    n: usize,
    out: String,
    base_ms: i64,
    fork: bool,
    forktick: i64,
    refcsv: String,
    refghost: String,
    shim: String,
    preds: Vec<String>,
    finishmargin: f32,
    corridor: f32,
    gate: String,
    gate_key: String,
    gate_min_key: f32,
    gate_min_key_auto: bool,
    gate_seed_state: String,
    fire: String,
    fire_at: f32,
    fire_need: u32,
    fire_where: String,
    after_key: String,
    after_ticks: u32,
    after_from_end: bool,
}

fn parse() -> Args {
    let v: Vec<String> = std::env::args().skip(1).collect();
    if v.is_empty() {
        eprint!("{}", USAGE);
        std::process::exit(2);
    }
    let mut a = Args {
        cmd: v[0].clone(),
        free: Vec::new(),
        template: String::new(),
        start_from: None,
        map: None,
        segs: Vec::new(),
        server: None,
        root: None,
        bestdir: "best".into(),
        log: None,
        workers: std::thread::available_parallelism().map(|v| v.get()).unwrap_or(8),
        batch: 30,
        nops: OpsPerCandidate::Exactly(1),
        ops: OpSet::Local,
        lo: 0,
        hi: usize::MAX,
        window: 140,
        stride: 70,
        full_window_every: 8,
        minutes: 120.0,
        seed: 1,
        temp_s: 0.0,
        migrate: 0.0,
        max_drift: 0,
        n: 0,
        out: "/tmp/tmsearch-dump.jsonl".into(),
        base_ms: 0,
        fork: false,
        forktick: 60,
        refcsv: String::new(),
        refghost: String::new(),
        shim: String::new(),
        preds: Vec::new(),
        finishmargin: 250.0,
        corridor: 40.0,
        gate: String::new(),
        gate_key: String::new(),
        gate_min_key: f32::NEG_INFINITY,
        gate_min_key_auto: false,
        gate_seed_state: String::new(),
        fire: String::new(),
        fire_at: 0.0,
        fire_need: 1,
        fire_where: String::new(),
        after_key: String::new(),
        after_ticks: 0,
        after_from_end: false,
    };
    let mut i = 1;
    let num = |s: &str, k: &str| -> f64 { s.parse().unwrap_or_else(|_| die(format!("{} wants a number, got {:?}", k, s))) };
    while i < v.len() {
        let k = v[i].as_str();
        let next = |i: &mut usize| -> String {
            *i += 1;
            v.get(*i).cloned().unwrap_or_else(|| die(format!("{} wants a value", k)))
        };
        match k {
            "--template" => a.template = next(&mut i),
            "--start-from" => a.start_from = Some(next(&mut i)),
            "--map" => a.map = Some(next(&mut i)),
            "--server" => a.server = Some(next(&mut i)),
            "--root" => a.root = Some(next(&mut i)),
            "--bestdir" => a.bestdir = next(&mut i),
            "--log" => a.log = Some(next(&mut i)),
            "--workers" => a.workers = num(&next(&mut i), k) as usize,
            "--batch" => a.batch = num(&next(&mut i), k) as usize,
            "--nops" => a.nops = OpsPerCandidate::Exactly(num(&next(&mut i), k) as usize),
            "--nops-upto" => a.nops = OpsPerCandidate::UpTo(num(&next(&mut i), k) as usize),
            "--ops" => a.ops = next(&mut i).parse().unwrap_or_else(|e| die(e)),
            "--lo" => a.lo = num(&next(&mut i), k) as usize,
            "--hi" => a.hi = num(&next(&mut i), k) as usize,
            "--window" => a.window = num(&next(&mut i), k) as usize,
            "--stride" => a.stride = num(&next(&mut i), k) as usize,
            "--full-window-every" => a.full_window_every = num(&next(&mut i), k) as u64,
            "--minutes" => a.minutes = num(&next(&mut i), k),
            "--seed" => a.seed = num(&next(&mut i), k) as u64,
            "--temp" => a.temp_s = num(&next(&mut i), k),
            "--migrate" => a.migrate = num(&next(&mut i), k),
            "--max-drift" => a.max_drift = num(&next(&mut i), k) as usize,
            "--n" => a.n = num(&next(&mut i), k) as usize,
            "--out" => a.out = next(&mut i),
            "--base" => a.base_ms = (num(&next(&mut i), k) * 1000.0).round() as i64,
            "--fork" => a.fork = true,
            "--forktick" => a.forktick = num(&next(&mut i), k) as i64,
            "--refcsv" => a.refcsv = next(&mut i),
            "--refghost" => a.refghost = next(&mut i),
            "--shim" => a.shim = next(&mut i),
            "--pred" => a.preds.push(next(&mut i)),
            "--finishmargin" => a.finishmargin = num(&next(&mut i), k) as f32,
            "--corridor" => a.corridor = num(&next(&mut i), k) as f32,
            "--gate" => a.gate = next(&mut i),
            "--gate-key" => a.gate_key = next(&mut i),
            "--gate-min-key" => {
                let v = next(&mut i);
                if v == "auto" {
                    a.gate_min_key_auto = true;
                } else {
                    a.gate_min_key = num(&v, k) as f32;
                }
            }
            "--fire" => a.fire = next(&mut i),
            "--fire-at" => a.fire_at = num(&next(&mut i), k) as f32,
            "--fire-need" => a.fire_need = num(&next(&mut i), k) as u32,
            "--fire-where" => a.fire_where = next(&mut i),
            "--after-ticks" => a.after_ticks = num(&next(&mut i), k) as u32,
            "--after-from" => {
                let v = next(&mut i);
                a.after_from_end = match v.as_str() {
                    "end" => true,
                    "start" => false,
                    _ => die("--after-from wants `start` or `end`"),
                };
            }
            "--after-key" => a.after_key = next(&mut i),
            "--gate-seed-state" => a.gate_seed_state = next(&mut i),
            "--seg" => {
                let s = next(&mut i);
                let (kk, p) = s.split_once(':').unwrap_or_else(|| die("--seg wants K:/path/map.Map.Gbx"));
                a.segs.push((kk.parse().unwrap_or_else(|_| die("--seg K must be a number")), PathBuf::from(p)));
            }
            "-h" | "--help" => {
                print!("{}", USAGE);
                std::process::exit(0)
            }
            other if other.starts_with("--") => die(format!("unknown flag {}", other)),
            other => a.free.push(other.to_string()),
        }
        i += 1;
    }
    a.segs.sort_by_key(|s| s.0);
    a
}

fn server_dir(a: &Args) -> PathBuf {
    ghost::oracle::server_dir(a.server.as_deref())
}

fn need_map(a: &Args) -> PathBuf {
    PathBuf::from(a.map.clone().unwrap_or_else(|| die("--map is required")))
}

/// The incumbent's own time, from the plain oracle. This is also the first
/// positive control of every run: if the starting tape does not reproduce, the
/// wiring is wrong and nothing after it means anything.
fn measure(server: &Path, map: &Path, p: &Patcher, inputs: &Inputs, scratch: &Path) -> Outcome {
    let f = scratch.join("incumbent.Ghost.Gbx");
    std::fs::write(&f, p.file(inputs)).unwrap_or_else(|e| die(format!("{}: {}", f.display(), e)));
    match ghost::oracle::validate(server, &f, ghost::oracle::MapsMode::One(map), "incumbent") {
        Ok(r) => match r.time_ms {
            Some(ms) => {
                if let Some(d) = r.declared_ms {
                    if d != ms {
                        eprintln!(
                            "note: the template DECLARES {} and DOES {}. The declaration is the \
                             donor's; `ghost declare --from-oracle` writes the real one.",
                            secs(d),
                            secs(ms)
                        );
                    }
                }
                Outcome::Finish { ms }
            }
            None => Outcome::Dnf(Progress::Checkpoints { cps: r.cps.unwrap_or(0), seg_ms: None }),
        },
        Err(e) => die(format!("the oracle could not measure the incumbent: {}", e)),
    }
}

fn build(a: &Args) -> (Arc<Patcher>, Inputs) {
    if a.template.is_empty() {
        die("--template is required");
    }
    let p = Patcher::build(&a.template).unwrap_or_else(|e| die(e));
    eprintln!(
        "template {}: {} ticks, tick 0 at race {}, declares {}",
        a.template,
        p.n(),
        secs(p.start_offset_ms as i64),
        p.declared_ms.map(secs).unwrap_or_else(|| "nothing".into())
    );
    if !p.unwritable.is_empty() {
        eprintln!(
            "note: {} tick(s) carry a packet this search cannot write ({}); a window \
             containing one is refused rather than searched with the edits dropped",
            p.unwritable.len(),
            p.unwritable
                .iter()
                .take(4)
                .map(|(t, w)| format!("{}: {}", t, w))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let start = match &a.start_from {
        Some(s) => {
            let q = Patcher::build(s).unwrap_or_else(|e| die(e));
            if q.n() != p.n() {
                die(format!("--start-from has {} ticks, the template has {}", q.n(), p.n()));
            }
            q.template
        }
        None => p.template.clone(),
    };
    (Arc::new(p), start)
}

/// Build the gate-mode identity control, if one was asked for.
///
/// The offline half is done HERE, once, before any server starts: if the
/// recording cannot say what the fork should measure -- because the box is not
/// on that tape's line, or the file is not what it claims -- that is a fact
/// about the setup and it should stop the run in the first second, not in the
/// first minute.
#[allow(clippy::type_complexity)]
type SeedCheck = std::sync::Arc<
    dyn Fn(&forkoracle::pred::GateRecord) -> Result<String, String> + Send + Sync,
>;

fn seed_state_check(a: &Args, p: &Patcher) -> (Option<SeedCheck>, f32, Option<[f32; 3]>) {
    if a.gate_seed_state.is_empty() {
        if a.gate_min_key_auto {
            die("--gate-min-key auto needs --gate-seed-state: the floor is derived from the \
                 seed's OWN key at the gate, and without the seed's recording there is nothing \
                 to derive it from.");
        }
        return (None, a.gate_min_key, None);
    }
    if a.gate.is_empty() {
        die("--gate-seed-state without --gate: there is no gate for the seed to be checked at.");
    }
    let gate = forkoracle::pred::parse_gate(&a.gate, &a.gate_key).unwrap_or_else(|e| die(e));
    let expect = tmsearch::seedstate::from_ghost(&a.gate_seed_state, &gate, p.start_offset_ms)
        .unwrap_or_else(|e| die(e));
    eprintln!(
        "gate: {}'s own telemetry reaches the box at {} and scores {:+.4} there\n  ghost   {}",
        a.gate_seed_state,
        secs(expect.tick as i64 * 10 + p.start_offset_ms as i64),
        expect.key,
        expect
    );
    let path = a.gate_seed_state.clone();
    let off = p.start_offset_ms;

    // --- THE BAR, and the control that says when it is wrong.
    //
    // `--gate-min-key` is the one knob in this feature that is a number
    // somebody has to choose, and choosing it wrong turns gate mode quietly
    // back into a finish-time search. It is not fully derivable -- the RIGHT
    // bar is near the key of the thing you are hunting, and if you knew that
    // you would be most of the way there -- but its FAILURE is derivable, and
    // that is worth more than a number in a document.
    //
    // The rule: **the seed must not clear the bar.** If it does, the seed
    // already occupies the top bands, nothing the search finds can outrank it
    // except a faster ordinary lap, and the moat the mode exists to cross is
    // still there. That is checkable here, before anything runs, against the
    // seed's own recording.
    let bar = if a.gate_min_key_auto {
        // A FLOOR, not a target: the smallest bar that keeps the seed out.
        let b = expect.key + f32::EPSILON * expect.key.abs().max(1.0);
        eprintln!(
            "gate: --gate-min-key auto -> {:+.6}, just above the seed's own {:+.4}. This is a \
             FLOOR and not a target: it guarantees only that the seed cannot sit in the top \
             bands. A bar near the key of the thing you are hunting is much stronger, and if \
             you know that number, pass it.",
            b, expect.key
        );
        b
    } else {
        a.gate_min_key
    };
    if bar.is_finite() && expect.key >= bar {
        die(format!(
            "--gate-min-key {:+.4} but the SEED's own state scores {:+.4} at this gate, which \
             clears it. The seed then sits in the top bands and nothing the search finds can \
             outrank it except a faster ordinary lap -- which is a finish-time search wearing a \
             state objective's clothes, and it is the exact local optimum this mode exists to \
             cross. Raise the bar above {:+.4}, or pass `--gate-min-key auto` for the smallest \
             bar that excludes the seed.",
            bar, expect.key, expect.key
        ));
    }
    if !bar.is_finite() {
        eprintln!(
            "gate: NO BAR. The seed scores {:+.4} here, so a tape that merely clips this box and \
             finishes takes the top band -- if that describes the seed's own route, the search \
             cannot beat it. Pass --gate-min-key auto, or a number.",
            expect.key
        );
    }

    let check: SeedCheck = std::sync::Arc::new(move |measured| {
        let ag = tmsearch::seedstate::check(&path, &gate, measured, off)?;
        if ag.passed() {
            Ok(ag.report())
        } else {
            Err(format!(
                "{}\nSTOPPING. The fork's own measurement of the SEED does not match the seed's \
                 own recording. Nothing measured by these servers means anything until that is \
                 explained: the record layout, the car locator, the clock labelling and the gate \
                 arithmetic are all upstream of this number.",
                ag.report()
            ))
        }
    });
    (Some(check), bar, Some(expect.pos))
}

fn main() {
    let a = parse();
    match a.cmd.as_str() {
        "search" => cmd_search(&a),
        "dump" => cmd_dump(&a),
        "analyze" => cmd_analyze(&a),
        "validate" => cmd_validate(&a),
        other => die(format!("unknown command {:?}\n\n{}", other, USAGE)),
    }
}

fn cmd_search(a: &Args) {
    let (p, start) = build(a);
    let map = need_map(a);
    let server = server_dir(a);
    let root = Root::claim(
        &a.root.clone().map(PathBuf::from).unwrap_or_else(Root::default_path),
    )
    .unwrap_or_else(|e| die(e));
    root.reset();

    let hi = a.hi.min(p.n());
    p.check_window(a.lo, hi).unwrap_or_else(|e| die(e));

    // THE STATE OBJECTIVE NEEDS THE FORK. The plain oracle reports a time and
    // a checkpoint count; it never sees where the car was or which way it was
    // pointing, so a gate armed on it would score every candidate as a miss
    // and the run would look like a search that simply found nothing.
    if !a.gate.is_empty() && !a.fork {
        die("--gate needs --fork: the plain oracle returns a time and a checkpoint count and \
             cannot see the car's state at all, so there is nothing for a gate to record.");
    }
    if !a.gate.is_empty() && a.gate_key.is_empty() {
        die("--gate needs --gate-key: a box with no key records a state and ranks nothing.");
    }
    if a.gate.is_empty() && !a.gate_key.is_empty() {
        die("--gate-key without --gate: there is no box to score inside.");
    }
    if !a.fire.is_empty() && a.gate.is_empty() {
        die("--fire needs --gate. The bands are cumulative: an event that fires \
             somewhere the search was not pointed at is not progress towards \
             anything, and without a gate there is no band below it to climb from.");
    }
    if a.fire.is_empty() && !a.after_key.is_empty() {
        die("--after-key without --fire: there is no event for it to be after.");
    }
    if a.fire.is_empty() && !a.fire_where.is_empty() {
        die("--fire-where without --fire: there is no event to place.");
    }
    if !a.fire.is_empty() && a.fire_at == 0.0 {
        die("--fire needs --fire-at K: the value the condition must reach. A \
             threshold of zero is almost never what anyone means and is too easy \
             to leave out by accident.");
    }

    let plain_outcome = measure(&server, &map, &p, &start, &root.path);
    eprintln!("incumbent: {}", plain_outcome);
    check_segment_maps(a, &server, &p, &start, &root.path);

    // IN GATE MODE THE INCUMBENT IS NOT A TIME. The plain measurement above
    // stays: it is the run's first positive control, and if the seed does not
    // reproduce nothing after it means anything. But the ladder the search
    // ranks on is the gate's, so the incumbent enters at the bottom of it and
    // worker 0 measures the seed's real band before the first candidate --
    // which is the same evaluation the decoy test needs, so it is free.
    let start_outcome = if a.gate.is_empty() {
        plain_outcome
    } else {
        eprintln!(
            "gate mode: the ranking above is the plain oracle's and is a control, not the \
             objective. The state objective scores the seed itself in the decoy line below."
        );
        Outcome::Gate(tmsearch::score::GateState::Missed { miss_m: f64::INFINITY })
    };

    let mut bank = Bank::new(
        Path::new(&a.bestdir),
        &server,
        &map,
        a.log.as_ref().map(Path::new),
    )
    .unwrap_or_else(|e| die(e));

    let (seed_check, gate_bar, seed_gate_pos) = seed_state_check(a, &p);
    let cfg = Config {
        workers: a.workers,
        batch: a.batch,
        ops_per_candidate: a.nops,
        opset: a.ops,
        lo: a.lo,
        hi,
        window: a.window,
        stride: a.stride,
        full_window_every: a.full_window_every,
        minutes: a.minutes,
        seed: a.seed,
        temp_s: a.temp_s,
        migrate: a.migrate,
        max_drift: a.max_drift,
        check_seed_gate: seed_check,
    };

    if a.fork {
        run_fork(
            a, &cfg, p, start, start_outcome, plain_outcome, gate_bar, seed_gate_pos, &mut bank,
            &root, &server, &map,
        );
    } else {
        let (pp, rootp, serverp, mapp, segs, refr) =
            (Arc::clone(&p), root.path.clone(), server.clone(), map.clone(), a.segs.clone(), start.clone());
        tmsearch::search::run(&cfg, Arc::clone(&p), start, start_outcome, &mut bank, move |wi| {
            BatchEval::new(Arc::clone(&pp), &rootp, &serverp, &mapp, &segs, wi, refr.clone())
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn run_fork(
    a: &Args,
    cfg: &Config,
    p: Arc<Patcher>,
    start: Inputs,
    start_outcome: Outcome,
    // What the PLAIN oracle said about the seed. In gate mode `start_outcome`
    // is on the gate's ladder and carries no time, but the reference line's
    // finish still has to be found somewhere.
    plain_outcome: Outcome,
    // The bar, after `auto` has been resolved against the seed.
    gate_bar: f32,
    // Where the seed's own state sat in the gate box, if a recording was given.
    seed_gate_pos: Option<[f32; 3]>,
    bank: &mut Bank,
    root: &Root,
    server: &Path,
    map: &Path,
) {
    if a.refcsv.is_empty() && a.refghost.is_empty() {
        die("--fork needs a reference line: --refghost G (the incumbent's own \
             telemetry, checked to belong to its tape) or --refcsv F (a trajectory \
             measured out of the engine). Progress and `offref` are measured against it.");
    }
    if a.shim.is_empty() {
        die("--fork needs --shim /path/to/libforkshim.so");
    }
    let refp = root.join("ref.Ghost.Gbx");
    std::fs::write(&refp, p.file(&start)).unwrap_or_else(|e| die(format!("{}", e)));
    let key = root.join("key.bin");
    let steer_u8: Vec<u8> = start.steer.iter().map(|&v| v as u8).collect();
    forkoracle::forksrv::write_key(&key, &steer_u8);

    let ckpt = clock_for_tick(a.forktick, p.start_offset_ms);
    let mut cal = forkoracle::forksrv::ForkServer::start(
        &root.join("calibrate"),
        server,
        map,
        &refp,
        &key,
        Path::new(&a.shim),
        ckpt,
    )
    .unwrap_or_else(|e| die(format!("the calibration fork server did not start: {}", e)));
    let probe = cal.probe_tick().unwrap_or_else(|e| {
        die(format!("the boundary probe failed ({}) -- a resume cannot be trusted without it", e))
    });
    let boundary = calibrate_boundary(&mut cal, server, map, &p, &root.path, probe, p.n())
        .unwrap_or_else(|e| die(e));
    cal.quit();
    eprintln!(
        "fork: checkpoint tick {} stopped at probe {}, calibrated boundary {} (race {})",
        a.forktick,
        probe,
        boundary,
        secs(boundary as i64 * 10 + p.start_offset_ms as i64)
    );

    let refline = if !a.refghost.is_empty() {
        let g = tmsearch::refline::from_ghost(
            &a.refghost,
            map.to_str().unwrap_or_default(),
            p.start_offset_ms,
            p.n(),
        )
        .unwrap_or_else(|e| die(e));
        eprintln!(
            "fork: reference line from {}'s own telemetry: {} samples, and the engine's own run \
             of its tape sits {:.4} m from it (kappa {:.3})",
            a.refghost, g.samples, g.engine_error_m, g.kappa
        );
        g.line
    } else {
        forkoracle::pred::RefLineData::from_csv(&a.refcsv, p.start_offset_ms, p.n())
            .unwrap_or_else(|e| die(format!("--refcsv: {}", e)))
    };
    let mut watch = forkoracle::pred::Watch::new();
    watch.corridor = a.corridor;
    watch.refline = refline;
    watch.finish_s = match plain_outcome.finish_ms() {
        Some(t) => {
            let tick = ((t - p.start_offset_ms as i64) / 10).max(0) as usize;
            (watch.refline.s_at_tick(tick) - a.finishmargin).max(1.0)
        }
        None => 0.0,
    };
    for s in &a.preds {
        watch.preds.push(forkoracle::pred::parse_spec(s).unwrap_or_else(|e| die(e)));
    }
    if !a.gate.is_empty() {
        watch.gate = forkoracle::pred::parse_gate(&a.gate, &a.gate_key).unwrap_or_else(|e| die(e));
    }
    if !a.fire.is_empty() {
        watch.fire =
            forkoracle::pred::parse_fire(
                &a.fire, a.fire_at, a.fire_need, &a.fire_where, &a.after_key, a.after_ticks,
                a.after_from_end,
            )
                .unwrap_or_else(|e| die(e));
    }
    eprintln!(
        "fork: reference line {:.0} m, predicates disarmed after {:.0} m",
        watch.refline.s_at_tick(usize::MAX),
        watch.finish_s
    );
    print!("{}", watch.describe());
    if watch.gate.armed && gate_bar.is_finite() {
        eprintln!(
            "  gate: a state scoring under {:+.4} does not count as having done the thing, \
             so a tape that clips the box and finishes still ranks as a state",
            gate_bar
        );
    }

    let setup = Arc::new(ForkSetup {
        server: server.to_path_buf(),
        map: map.to_path_buf(),
        reference_ghost: refp,
        key,
        shim: PathBuf::from(&a.shim),
        checkpoint_clock: ckpt,
        calibrated: boundary,
        gate_min_key: gate_bar,
        gate_seed_pos: seed_gate_pos,
        start_offset_ms: p.start_offset_ms,
    });
    let watch = Arc::new(watch);
    let rootp = root.path.clone();
    let refr = start.clone();
    tmsearch::search::run(cfg, Arc::clone(&p), start, start_outcome, bank, move |wi| {
        ForkEval::start(&rootp.join(format!("w{:03}", wi)), &setup, &watch, refr.clone())
    });
}

/// Check every `--seg K:MAP` before the search trusts it as a ladder rung.
///
/// A segment map is the same map with the finish moved to checkpoint K, and
/// **it is a fine ruler and an unsafe objective**: swapping a
/// `GateCheckpointLeft32m` for a `GateFinish32m` is not a faithful trigger --
/// one map paid 0.206 s of phantom gain for exactly that -- and the
/// reference-ghost identity control cannot catch it, because the reference
/// line passes through both volumes.
///
/// Two things are checkable here for the price of one validation each:
///
/// * the incumbent must FINISH on the segment map. If it does not, either the
///   map is not what it claims or the incumbent never reaches that checkpoint,
///   and in both cases every DNF the search re-scores there is scored on
///   nothing.
/// * when the template is a real recording, its own split for checkpoint K is
///   recorded in the container, and the segment map should return **that
///   number**. A gate in a different place, or with a different trigger
///   volume, shows up here as a disagreement of tens of milliseconds.
fn check_segment_maps(a: &Args, server: &Path, p: &Patcher, start: &Inputs, scratch: &Path) {
    if a.segs.is_empty() {
        return;
    }
    let f = scratch.join("segcheck.Ghost.Gbx");
    std::fs::write(&f, p.file(start)).unwrap_or_else(|e| die(format!("{}", e)));
    // The template's own DECLARED checkpoint list. `Container::splits()` used
    // to return the result chunk's RAW WORD ARRAY, so `splits[k - 1]` for
    // checkpoint 1 read the chunk's version word -- and this comparison then
    // announced a 7.616 s "trigger difference" that was an array index. It
    // returns the decoded list now; the raw words are `splits_raw()`.
    let splits: Vec<i32> = ghost::Container::load(&a.template)
        .map(|c| c.splits())
        .unwrap_or_default();
    let own_splits = a.start_from.is_none();

    for (k, m) in &a.segs {
        let r = ghost::oracle::validate(server, &f, ghost::oracle::MapsMode::One(m), "segcheck")
            .unwrap_or_else(|e| die(format!("--seg {}: {}", k, e)));
        match r.time_ms {
            None => die(format!(
                "--seg {}:{} -- the incumbent does not finish on this segment map (it returns \
                 {}). Either the map's finish is not where checkpoint {} is, or the incumbent \
                 never reaches it. Every failure the search re-scores there would be scored on \
                 nothing.",
                k,
                m.display(),
                r.desc.trim(),
                k
            )),
            Some(ms) => {
                // A 0 is not a split. `ghost declare --cps N` writes 0.000 for
                // the intermediate entries of a container borrowed from another
                // map -- "this file does not know its splits" -- and comparing
                // a segment map's answer against that would report the entire
                // time as a trigger difference.
                let expect = if own_splits {
                    splits.get((*k as usize).saturating_sub(1)).filter(|w| **w > 0)
                } else {
                    None
                };
                match expect {
                    Some(&want) if want as i64 != ms => eprintln!(
                        "WARNING --seg {}: the segment map returns {} where the template's own \
                         recorded split for checkpoint {} is {} ({}). A promoted gate is a fine \
                         ruler and an unsafe objective; that difference is the trigger, not the \
                         driving.",
                        k,
                        secs(ms),
                        k,
                        secs(want as i64),
                        report_delta(ms - want as i64)
                    ),
                    Some(&want) => eprintln!(
                        "--seg {}: {} -- and the template's own recorded split agrees exactly ({})",
                        k,
                        secs(ms),
                        secs(want as i64)
                    ),
                    None => eprintln!("--seg {}: the incumbent reaches it at {}", k, secs(ms)),
                }
            }
        }
    }
    let _ = std::fs::remove_file(&f);
}

fn report_delta(ms: i64) -> String {
    tmsearch::report::delta(ms)
}

fn cmd_dump(a: &Args) {
    let (p, start) = build(a);
    let map = need_map(a);
    let server = server_dir(a);
    let root = Root::claim(&a.root.clone().map(PathBuf::from).unwrap_or_else(Root::default_path))
        .unwrap_or_else(|e| die(e));
    root.reset();
    if a.n == 0 {
        die("dump needs --n N");
    }
    let hi = a.hi.min(p.n());
    p.check_window(a.lo, hi).unwrap_or_else(|e| die(e));

    let mut rng = Rng::new(a.seed);
    let mut ev = BatchEval::new(
        Arc::clone(&p),
        &root.path,
        &server,
        &map,
        &a.segs,
        0,
        start.clone(),
    )
    .unwrap_or_else(|e| die(e));

    let mut out = String::new();
    let mut done = 0usize;
    while done < a.n {
        let k = a.batch.min(a.n - done);
        let mut cands = Vec::with_capacity(k);
        let mut ops = Vec::with_capacity(k);
        for _ in 0..k {
            let mut s = start.clone();
            let op = mutate(&mut s, &mut rng, a.lo, hi, a.ops);
            cands.push(s);
            ops.push(op);
        }
        for (o, r) in ops.iter().zip(ev.evaluate(&cands)) {
            let (ms, cps) = match r {
                Outcome::Finish { ms } => (format!("{}", ms), 0),
                Outcome::Dnf(Progress::Checkpoints { cps, .. }) => ("null".into(), cps),
                Outcome::Dnf(Progress::Metres { .. }) => ("null".into(), 0),
                // `dump` is the plain evaluator only, which cannot produce one.
                Outcome::Gate(_) => unreachable!("dump does not run a state objective"),
            };
            out.push_str(&format!(
                "{{\"kind\":\"{}\",\"at\":{},\"span\":{},\"val\":{},\"ms\":{},\"cps\":{}}}\n",
                o.kind, o.at, o.span, o.val, ms, cps
            ));
        }
        done += k;
        eprintln!("dump {}/{}", done, a.n);
    }
    std::fs::write(&a.out, out).unwrap_or_else(|e| die(format!("{}: {}", a.out, e)));
    eprintln!("wrote {}", a.out);
}

fn cmd_analyze(a: &Args) {
    let log = a.log.clone().unwrap_or_else(|| die("analyze needs --log F.jsonl"));
    let txt = std::fs::read_to_string(&log).unwrap_or_else(|e| die(format!("{}: {}", log, e)));
    let rows = analyze::parse_dump(&txt);
    if a.base_ms == 0 {
        die("analyze needs --base SECONDS (the incumbent the dump was drawn from)");
    }
    print!("{}", analyze::report(&rows, a.base_ms, a.window.max(1) as i64));
}

fn cmd_validate(a: &Args) {
    let map = need_map(a);
    let server = server_dir(a);
    if a.free.is_empty() {
        die("validate needs one or more ghosts");
    }
    let paths: Vec<PathBuf> = a.free.iter().map(PathBuf::from).collect();
    let refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
    let rows = ghost::oracle::validate_many(&server, &refs, ghost::oracle::MapsMode::One(&map), "validate")
        .unwrap_or_else(|e| die(e));
    for r in &rows {
        let declared = match r.declared_ms {
            Some(d) if Some(d) != r.time_ms => format!("   (the file declares {})", secs(d)),
            _ => String::new(),
        };
        println!(
            "{:<44} {:>10}{}",
            r.file,
            match r.time_ms {
                Some(ms) => secs(ms),
                None => format!("DNF cp{}", r.cps.unwrap_or(0)),
            },
            declared
        );
    }
    let disagree = rows.iter().filter(|r| !r.declaration_holds()).count();
    if disagree > 0 {
        eprintln!(
            "\n{} of {} files do not do what they declare. A synthesised tape carries its \
             template's header until `ghost declare --from-oracle` writes the real one.",
            disagree,
            rows.len()
        );
    }
}

/// Unused-import guard: `Provenance` and `Evaluator` are part of the public
/// surface this binary documents, and the compiler should say so if they move.
#[allow(dead_code)]
fn _surface(_: &Provenance, _: &dyn Fn() -> Box<dyn Evaluator>) {}
