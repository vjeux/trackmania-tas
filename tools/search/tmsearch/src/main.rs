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

    let start_outcome = measure(&server, &map, &p, &start, &root.path);
    eprintln!("incumbent: {}", start_outcome);

    let mut bank = Bank::new(
        Path::new(&a.bestdir),
        &server,
        &map,
        a.log.as_ref().map(Path::new),
    )
    .unwrap_or_else(|e| die(e));

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
    };

    if a.fork {
        run_fork(a, &cfg, p, start, start_outcome, &mut bank, &root, &server, &map);
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
    watch.finish_s = match start_outcome.finish_ms() {
        Some(t) => {
            let tick = ((t - p.start_offset_ms as i64) / 10).max(0) as usize;
            (watch.refline.s_at_tick(tick) - a.finishmargin).max(1.0)
        }
        None => 0.0,
    };
    for s in &a.preds {
        watch.preds.push(forkoracle::pred::parse_spec(s).unwrap_or_else(|e| die(e)));
    }
    eprintln!(
        "fork: reference line {:.0} m, predicates disarmed after {:.0} m",
        watch.refline.s_at_tick(usize::MAX),
        watch.finish_s
    );
    print!("{}", watch.describe());

    let setup = Arc::new(ForkSetup {
        server: server.to_path_buf(),
        map: map.to_path_buf(),
        reference_ghost: refp,
        key,
        shim: PathBuf::from(&a.shim),
        checkpoint_clock: ckpt,
        calibrated: boundary,
        start_offset_ms: p.start_offset_ms,
    });
    let watch = Arc::new(watch);
    let rootp = root.path.clone();
    let refr = start.clone();
    tmsearch::search::run(cfg, Arc::clone(&p), start, start_outcome, bank, move |wi| {
        ForkEval::start(&rootp.join(format!("w{:03}", wi)), &setup, &watch, refr.clone())
    });
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
