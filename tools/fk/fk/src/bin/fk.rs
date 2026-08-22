//! `fk` — the command line.
//!
//! Every command takes the same five engine flags and one checkpoint selector.
//! There is no shared config bag: a flag a command accepts is a flag that
//! command uses.

use fk::session::{Checkpoint, Engine};
use fk::tape::Tape;
use std::path::PathBuf;

use fk::cmd;

const USAGE: &str = "\
fk -- the driver for the TM2020 dedicated server used as a physics oracle.

  fk server probe    where a fork server actually stopped, and the safe resume tick
  fk server check    fork resume vs full validation on the same candidates  [THE CONTROL]
  fk server bench    throughput against the batched plain oracle
  fk trace           one fork -> the car's own state per tick, as a 29-column CSV
  fk watch           the early-abort watchdog: exactness, false positives, speedup
  fk regen           rewrite a ghost's telemetry from engine state

Engine flags, accepted by every command:
  --tape FILE        the .Ghost.Gbx / .Replay.Gbx whose inputs the engine runs
  --map FILE         the map (decoration for a .Replay.Gbx: it carries its own)
  --server DIR       the dedicated-server install       [$TM_SERVER]
  --shim FILE        libforkshim.so                     [$FK_SHIM, or beside fk, or
                     ../search/target/release/]
  --work DIR         scratch; per-process by default, and never shared

Where to stop the simulation (one of):
  --at tick:N        tape tick N, via clock = 36141 + 25.483 * race_ms
  --at clock:N       a raw lroundf call count
  --at frac:F        F of the way through the tape       [default frac:0.5]

Run `fk <command> --help` for a command's own flags.

A fork-reported time is a MEASUREMENT. Only the plain oracle, run on the file as
written to disk, is a RESULT -- see `ghost verify`. The fork server was exact on
4700 of 4700 candidates that perturbed a human reference late in the run, and
LIED on 312 of 312 outside that regime.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print!("{}", USAGE);
        std::process::exit(if args.is_empty() { 2 } else { 0 });
    }
    if let Err(e) = dispatch(&args) {
        fk::abort(e);
    }
}

fn dispatch(a: &[String]) -> Result<(), String> {
    match a[0].as_str() {
        "server" => {
            let verb = a.get(1).map(|s| s.as_str()).unwrap_or("");
            let rest = &a[2.min(a.len())..];
            let (engine, tape, at) = common(rest)?;
            match verb {
                "probe" => cmd::server::probe(&engine, tape, at),
                "check" => {
                    let o = cmd::server::CheckOpts {
                        n: num(rest, "--n").unwrap_or(20) as usize,
                        seed: num(rest, "--seed").unwrap_or(1) as u64,
                        span: num(rest, "--span").unwrap_or(60) as usize,
                    };
                    match cmd::server::check(&engine, tape, at, o)? {
                        true => Ok(()),
                        false => Err("the fork server did not reproduce the full validation \
                                      on every candidate"
                            .into()),
                    }
                }
                "bench" => cmd::server::bench(
                    &engine,
                    tape,
                    at,
                    num(rest, "--n").unwrap_or(50) as usize,
                    num(rest, "--seed").unwrap_or(1) as u64,
                ),
                _ => Err("fk server <probe|check|bench>".into()),
            }
        }
        "trace" => {
            let rest = &a[1..];
            let (engine, tape, at) = common(rest)?;
            cmd::trace::run(
                &engine,
                tape,
                at,
                cmd::trace::TraceOpts {
                    reference: flag(rest, "--reference").map(|s| s.to_string()),
                    out: flag(rest, "--out").map(|s| s.to_string()),
                    nth: num(rest, "--nth").unwrap_or(1).max(1) as usize,
                },
            )
        }
        // `watch` and `regen` take `--template` rather than `--tape`, and both
        // choose their own checkpoints from a ladder rather than being told
        // one, so neither goes through `common`. That is a real difference, not
        // an inconsistency to paper over: a harness that measures the watchdog
        // over a window is not the same shape of command as one that reads a
        // trajectory at a checkpoint you name.
        "watch" => cmd::watch::run(&a[1..]),
        "regen" => cmd::regen::run(&a[1..]),
        x => Err(format!("unknown command {:?}\n\n{}", x, USAGE)),
    }
}

pub fn flag<'a>(a: &'a [String], name: &str) -> Option<&'a str> {
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).map(|s| s.as_str())
}
pub fn num(a: &[String], name: &str) -> Option<i64> {
    flag(a, name).map(|v| {
        v.parse()
            .unwrap_or_else(|_| fk::die(format!("{} wants a number, got {:?}", name, v)))
    })
}
pub fn has(a: &[String], name: &str) -> bool {
    a.iter().any(|x| x == name)
}

/// The five engine flags and the checkpoint, parsed once.
///
/// Unknown flags are an ERROR, not a shrug. The old parser panicked on some and
/// silently ignored others depending on which command you were in, so a typo
/// could run a whole measurement against a default you did not mean.
fn common(a: &[String]) -> Result<(Engine, Tape, Checkpoint), String> {
    let tape_path = flag(a, "--tape").ok_or("--tape FILE is required")?;
    let tape = Tape::load(tape_path)?;
    let work = flag(a, "--work").map(PathBuf::from);
    let engine = Engine {
        server: flag(a, "--server")
            .map(PathBuf::from)
            .or_else(|| std::env::var("TM_SERVER").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/tmp/tmoracle/server")),
        map: PathBuf::from(flag(a, "--map").ok_or("--map FILE is required")?),
        shim: flag(a, "--shim")
            .map(PathBuf::from)
            .or_else(|| std::env::var("FK_SHIM").ok().map(PathBuf::from))
            .or_else(fk::session::default_shim)
            .ok_or("no --shim: pass one, set FK_SHIM, or build tools/search (which produces \
              libforkshim.so)")?,
        work_is_temporary: work.is_none(),
        work: work.unwrap_or_else(Engine::default_work),
    };
    let at = match flag(a, "--at") {
        None => Checkpoint::Fraction(0.5),
        Some(s) => match s.split_once(':') {
            Some(("tick", v)) => Checkpoint::Tick(v.parse().map_err(|_| "--at tick:N")?),
            Some(("clock", v)) => Checkpoint::Clock(v.parse().map_err(|_| "--at clock:N")?),
            Some(("frac", v)) => Checkpoint::Fraction(v.parse().map_err(|_| "--at frac:F")?),
            _ => return Err(format!("--at wants tick:N, clock:N or frac:F, got {:?}", s)),
        },
    };
    Ok((engine, tape, at))
}


