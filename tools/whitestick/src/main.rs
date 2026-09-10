//! whitestick — run commands on the WhiteStick box from a Meta devserver.
//!
//! The box (a Windows PC with WSL, at home) is on no network Meta can reach, and
//! a devserver can only get out through fwdproxy over HTTPS. Both sides
//! therefore dial OUT to a tiny relay (a Cloudflare Worker, `whitestick-relay`)
//! and meet there. Nothing depends on Meta-internal services staying up.
//!
//! ```text
//!   whitestick '<cmd>'        run it on the box; stdin/stdout/stderr stream live,
//!                             exit status is the remote command's
//!   echo '<cmd>' | whitestick same, command read from stdin (the wsx contract)
//!   whitestick agent          the box side: connect to the relay and serve
//!   whitestick status         is the box connected right now?
//! ```
//!
//! Config lives in `~/.whitestick/config.toml` (see `config.rs`).

mod agent;
mod client;
mod config;
mod proto;
mod transport;

use anyhow::{bail, Context, Result};
use std::io::IsTerminal;
use std::time::Duration;

fn usage() -> &'static str {
    "usage:\n  \
     whitestick [--instance NAME] [--json] [--cwd DIR] [--shell SH] [--no-stdin]\n             \
     [--wait SECS] [--timeout SECS] [--] '<command>'\n  \
     echo '<command>' | whitestick [flags]        (command from stdin)\n  \
     whitestick agent [--name NAME] [--cwd DIR] [--shell SH]\n  \
     whitestick status [--instance NAME]\n  \
     whitestick --version | --help\n\n\
     With a positional command, a non-terminal stdin is forwarded to the remote\n\
     command (--no-stdin turns that off). Exit status is the remote command's;\n\
     124 = --timeout hit, 130 = interrupted twice, 1 = bridge error."
}

/// A UTC timestamp for log lines, without pulling in a date crate.
pub fn log(msg: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Howard Hinnant's civil-from-days.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    eprintln!("[{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z] {msg}");
}

fn parse_secs(flag: &str, v: Option<String>) -> Result<Duration> {
    let v = v.with_context(|| format!("{flag} needs a number of seconds"))?;
    let n: f64 = v.parse().with_context(|| format!("{flag}: not a number: {v}"))?;
    Ok(Duration::from_secs_f64(n.max(0.0)))
}

fn main() {
    let code = match real_main() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[whitestick] error: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

fn real_main() -> Result<i32> {
    let mut argv: Vec<String> = std::env::args().skip(1).collect();

    if argv.first().map(String::as_str) == Some("agent") {
        argv.remove(0);
        return run_agent(argv);
    }
    if argv.first().map(String::as_str) == Some("status") {
        argv.remove(0);
        return run_status(argv);
    }

    let cfg = config::Config::load()?;
    let mut instance = cfg.instance().to_string();
    let mut json = false;
    let mut cwd = None;
    let mut shell = None;
    let mut no_stdin = false;
    let mut wait = Duration::ZERO;
    let mut timeout = None;
    let mut positional: Vec<String> = Vec::new();

    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--instance" => instance = it.next().context("--instance needs a box name")?,
            "--json" => json = true,
            "--cwd" => cwd = Some(it.next().context("--cwd needs a directory")?),
            "--shell" => shell = Some(it.next().context("--shell needs a path")?),
            "--no-stdin" => no_stdin = true,
            "--wait" => wait = parse_secs("--wait", it.next())?,
            "--timeout" => timeout = Some(parse_secs("--timeout", it.next())?),
            "-h" | "--help" => {
                println!("{}", usage());
                return Ok(0);
            }
            "-V" | "--version" => {
                println!("whitestick {}", env!("CARGO_PKG_VERSION"));
                return Ok(0);
            }
            "--" => {
                positional.extend(it);
                break;
            }
            other if other.starts_with("--") => bail!("unknown flag: {other}\n{}", usage()),
            _ => positional.push(a),
        }
    }

    if positional.len() > 1 {
        bail!(
            "got {} positional arguments; quote the whole command as one string",
            positional.len()
        );
    }

    let stdin_is_tty = std::io::stdin().is_terminal();
    let (cmd, forward_stdin) = match positional.pop() {
        Some(c) if !c.trim().is_empty() => (c, !no_stdin && !stdin_is_tty),
        _ => {
            // The legacy contract: no positional means the command is on stdin.
            if stdin_is_tty {
                println!("{}", usage());
                return Ok(2);
            }
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            if buf.trim().is_empty() {
                println!("{}", usage());
                return Ok(2);
            }
            (buf, false)
        }
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()?;
    rt.block_on(client::run(
        &cfg,
        client::RunOpts {
            instance,
            cmd,
            cwd,
            shell,
            forward_stdin,
            json,
            wait,
            timeout,
        },
    ))
}

fn run_status(argv: Vec<String>) -> Result<i32> {
    let cfg = config::Config::load()?;
    let mut instance = cfg.instance().to_string();
    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--instance" => instance = it.next().context("--instance needs a box name")?,
            other => bail!("status: unknown argument {other}"),
        }
    }
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(client::status(&cfg, &instance))
}

fn run_agent(argv: Vec<String>) -> Result<i32> {
    let cfg = config::Config::load()?;
    let mut name = cfg.agent_name().to_string();
    let mut cwd = cfg.agent.cwd.clone();
    let mut shell = cfg.agent.shell.clone().unwrap_or_else(|| "/bin/sh".to_string());
    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--name" => name = it.next().context("--name needs a box name")?,
            "--cwd" => cwd = Some(it.next().context("--cwd needs a directory")?),
            "--shell" => shell = it.next().context("--shell needs a path")?,
            other => bail!("agent: unknown argument {other}"),
        }
    }
    if let Some(d) = &cwd {
        if !std::path::Path::new(d).is_dir() {
            log(&format!("warning: cwd {d} does not exist; commands will fail to start until it does"));
        }
    }
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(agent::run(cfg, agent::AgentOpts { name, cwd, shell }))?;
    Ok(0)
}
