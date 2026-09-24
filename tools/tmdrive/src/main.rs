//! `tmdrive` — the command-line face of the game lock.
//!
//! Subcommands that CHANGE the game take the lock first; the read-only ones
//! (`status`, `wait`) do not. A shell script that drives the game goes through
//! `tmdrive run`, which is what keeps the Rust-level guarantee from being
//! side-stepped by a one-line `.sh`.

use tmdrive::{acquire, holder, ops, Error, Host, Identity};

fn usage() -> ! {
    eprint!(
        "{}",
        r#"tmdrive — one game, one driver

READ-ONLY
  status                        who holds the game, and whether it is reclaimable
  wait [--timeout S]            block until the box is free (default 600)

HOLDS THE LOCK
  run --purpose P -- CMD...     hold the box for the whole of CMD (use this for
                                multi-step work: the lock renews automatically
                                and is released when CMD exits)
  launch [--timeout S]          start the game and wait for the process
  kill                          stop the game
  playmap PATH                  load a map
  input KEYS [--hold MS]        send key input
  plugin ENDPOINT [QUERY]       call the in-game plugin

Every lock-taking subcommand accepts --purpose 'what for' (shown to a blocked
driver) and --wait S (wait for a busy box instead of failing).

Identity comes from TM_SESSION (or AGENTCLOUD_SESSION_ID) and TM_SESSION_TITLE,
so a blocked driver can see who holds the box and message that session.

Exit 75 (EX_TEMPFAIL) means BUSY: a live session holds it — retry or ask.
"#
    );
    std::process::exit(2)
}

fn flag(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}
fn num(args: &[String], k: &str, d: u64) -> u64 {
    flag(args, k).and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let host = Host::detect();
    let purpose = flag(&args, "--purpose").unwrap_or_else(|| format!("tmdrive {}", args[0]));

    let code = match args[0].as_str() {
        "status" | "holder" => match holder(&host) {
            Ok(None) => {
                println!("FREE");
                0
            }
            Ok(Some(h)) => {
                println!("{}", h.summary());
                match h.reclaimable {
                    None => {
                        println!(
                            "  ask for it:  agentcloudctl send-message --to {} --body '...'",
                            h.session_id
                        );
                        1
                    }
                    Some(_) => {
                        println!("  (reclaimable: the next driver will take it automatically)");
                        0
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        },

        // Run detached by `acquire_detached`; not for humans.
        "renew-daemon" => {
            tmdrive::renew_daemon(&host);
            0
        }

        "wait" => {            let me = match Identity::from_env() {
                Ok(i) => i.session_id,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1)
                }
            };
            match tmdrive::wait_until_free(&host, &me, num(&args, "--timeout", 600)) {
                Ok(()) => {
                    println!("box is free");
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    75
                }
            }
        }

        "launch" => with_lock(&host, &purpose, &args, |l| {
            let pid = ops::launch(l, num(&args, "--timeout", 300))?;
            println!("game pid {pid}");
            Ok(())
        }),

        "kill" => with_lock(&host, &purpose, &args, |l| {
            ops::kill(l)?;
            println!("killed");
            Ok(())
        }),

        "playmap" => {
            let Some(p) = args.get(1).cloned() else { usage() };
            // VALIDATE BEFORE THE LOCK. A map outside the game's user
            // directory is refused with the fix in the message -- and that
            // refusal must not cost a wait for the box first. Checking the
            // path needs no lock, so it comes first and fails in
            // milliseconds; the game is never asked to load something it
            // would silently ignore.
            let p = match tmdrive::loadable_map_path(&p) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("tmdrive: {e}");
                    std::process::exit(2);
                }
            };
            with_lock(&host, &purpose, &args, |l| {
                println!("{}", ops::play_map(l, &p)?);
                Ok(())
            })
        }

        "input" => {
            let Some(keys) = args.get(1).cloned() else { usage() };
            with_lock(&host, &purpose, &args, |l| {
                println!("{}", ops::input(l, &keys, num(&args, "--hold", 50))?);
                Ok(())
            })
        }

        "plugin" => {
            let Some(ep) = args.get(1).cloned() else { usage() };
            let q = args.get(2).filter(|s| !s.starts_with("--")).cloned().unwrap_or_default();
            with_lock(&host, &purpose, &args, |l| {
                println!("{}", ops::plugin(l, &ep, &q)?);
                Ok(())
            })
        }

        // Hold the box for the duration of any other command. This is how a
        // shell script gets the lock without every script re-implementing it,
        // and how multi-step work keeps the box across many operations.
        "run" => {
            let Some(sep) = args.iter().position(|a| a == "--") else { usage() };
            let cmd: Vec<String> = args[sep + 1..].to_vec();
            if cmd.is_empty() {
                usage();
            }
            // TWO RUNS OF THE SAME SESSION MUST NOT SILENTLY NEST.
            //
            // The lock is re-entrant per session on purpose (a run's script
            // calls other lock-aware tools). But a SECOND `tmdrive run` from
            // the same session — a probe started in another terminal while a
            // publish is mid-flight — joins the first run's hold and then
            // drives the game under its feet: the u10s publisher lost its
            // game twice this way on 2026-09-24. Same session is not the same
            // job. So a run refuses when its own session already holds the
            // box with a LIVE keeper, unless the caller says it means to nest
            // (`--nested`, or TM_LOCK_TOKEN already in the environment, which
            // is what a run's own child processes carry).
            let explicitly_nested =
                args.iter().any(|a| a == "--nested") || std::env::var_os("TM_LOCK_TOKEN").is_some();
            if !explicitly_nested {
                if let (Ok(me), Ok(Some(h))) = (Identity::from_env(), tmdrive::holder(&host)) {
                    if h.session_id == me.session_id && h.reclaimable.is_none() && h.keeper_alive {
                        eprintln!(
                            "tmdrive: this session ALREADY holds the box — '{}' ({}s ago, keeper pid {}).\n\
                             A second run would drive the game under that job's feet. Wait for it, or\n\
                             pass --nested if this run is deliberately part of it.",
                            h.purpose,
                            h.age_s,
                            h.owner_pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
                        );
                        std::process::exit(75);
                    }
                }
            }
            with_lock(&host, &purpose, &args, move |l| {
                let st = std::process::Command::new(&cmd[0])
                    .args(&cmd[1..])
                    .env("TM_LOCK_TOKEN", l.token())
                    .status()
                    .map_err(|e| Error::Op(format!("{}: {e}", cmd[0])))?;
                if !st.success() {
                    return Err(Error::Op(format!("command exited {st}")));
                }
                Ok(())
            })
        }

        _ => usage(),
    };
    std::process::exit(code);
}

/// Acquire, run, release. `--wait S` turns a busy box into a wait instead of
/// an immediate failure.
fn with_lock<F>(host: &Host, purpose: &str, args: &[String], f: F) -> i32
where
    F: FnOnce(&tmdrive::GameLock) -> Result<(), Error>,
{
    let wait_s = num(args, "--wait", 0);
    if wait_s > 0 {
        if let Ok(me) = Identity::from_env() {
            if let Err(e) = tmdrive::wait_until_free(host, &me.session_id, wait_s) {
                eprintln!("{e}");
                return 75;
            }
        }
    }
    match acquire(host.clone(), purpose) {
        Ok(lock) => match f(&lock) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        },
        Err(e) => {
            eprintln!("{e}");
            // BUSY is a different outcome from broken: a caller waiting for
            // the box wants to retry, not to fail the run.
            match e {
                Error::Busy(_) => 75,
                _ => 1,
            }
        }
    }
}
