//! The supervisor: runs the server in the foreground of its own process,
//! restarts it when it dies (with a backoff that resets after a healthy
//! stretch), pipes its output into the rotating log, and hosts the chat bridge.
//!
//! `tmserverctl start` launches `tmserverctl run --quiet` detached (own session,
//! no terminal) and writes nothing else; `stop` sends SIGTERM to that process,
//! which stops the server gracefully and exits.

use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::logfile::Logger;
use crate::{bridge, procs};

/// Set by SIGTERM/SIGINT.
pub static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_sig: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

pub fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

/// Sleep in small steps so a stop request is honoured quickly.
pub fn wait_stop(d: Duration) -> bool {
    let step = Duration::from_millis(200);
    let deadline = Instant::now() + d;
    while Instant::now() < deadline {
        if STOP.load(Ordering::Relaxed) {
            return true;
        }
        thread::sleep(step.min(deadline.saturating_duration_since(Instant::now())));
    }
    STOP.load(Ordering::Relaxed)
}

pub fn run(cfg: &Config, echo: bool) -> Result<(), String> {
    fs::create_dir_all(cfg.run_dir()).map_err(|e| format!("{}: {e}", cfg.run_dir().display()))?;
    fs::create_dir_all(cfg.log_dir()).map_err(|e| format!("{}: {e}", cfg.log_dir().display()))?;
    if let Some(pid) = procs::read_pid(&cfg.pid_file()) {
        if procs::alive(pid) && pid != std::process::id() as i32 {
            return Err(format!("a supervisor is already running (pid {pid})"));
        }
    }
    fs::write(cfg.pid_file(), format!("{}\n", std::process::id())).map_err(|e| format!("{}: {e}", cfg.pid_file().display()))?;
    install_signal_handlers();

    let log = Logger::start(cfg.log_file(), cfg.log_max_bytes, cfg.log_keep, echo).map_err(|e| format!("log: {e}"))?;
    let bin = cfg.binary();
    if !bin.is_file() {
        let _ = fs::remove_file(cfg.pid_file());
        return Err(format!("server binary not found: {}", bin.display()));
    }
    let server_cfg = cfg.server_cfg()?;
    log.note(&format!(
        "supervisor {} starting: {} {} (xml-rpc :{}, game :{}, login {:?})",
        env!("CARGO_PKG_VERSION"),
        bin.display(),
        cfg.server_args().join(" "),
        server_cfg.xmlrpc_port,
        server_cfg.server_port,
        server_cfg.login
    ));

    let bridge_stop = Arc::new(AtomicBool::new(false));
    let bridge_thread = if cfg.bridge {
        let (port, pw, stop, blog) = (server_cfg.xmlrpc_port, server_cfg.superadmin_password.clone(), bridge_stop.clone(), log.clone());
        Some(thread::Builder::new().name("bridge".into()).spawn(move || bridge::run(port, pw, stop, blog)).map_err(|e| e.to_string())?)
    } else {
        None
    };

    let mut delay = Duration::from_secs(cfg.restart_delay_s.max(1));
    while !STOP.load(Ordering::Relaxed) {
        let started = Instant::now();
        let status = match spawn_server(cfg, &log) {
            Ok(mut child) => {
                let _ = fs::write(cfg.server_pid_file(), format!("{}\n", child.id()));
                log.note(&format!("server started, pid {}", child.id()));
                let status = supervise(&mut child, &log);
                let _ = fs::remove_file(cfg.server_pid_file());
                status
            }
            Err(e) => {
                log.note(&format!("cannot start the server: {e}"));
                None
            }
        };
        let uptime = started.elapsed();
        if STOP.load(Ordering::Relaxed) {
            log.note(&format!("server stopped on request after {} s", uptime.as_secs()));
            break;
        }
        match status {
            Some(s) => log.note(&format!("server exited ({s}) after {} s", uptime.as_secs())),
            None => {}
        }
        // A server that ran for a while earned a fresh, short delay; a crash loop backs off.
        if uptime > Duration::from_secs(600) {
            delay = Duration::from_secs(cfg.restart_delay_s.max(1));
        }
        log.note(&format!("restarting in {} s", delay.as_secs()));
        if wait_stop(delay) {
            break;
        }
        delay = (delay * 2).min(Duration::from_secs(60));
    }

    bridge_stop.store(true, Ordering::SeqCst);
    if let Some(t) = bridge_thread {
        let _ = t.join();
    }
    log.note("supervisor exiting");
    let _ = fs::remove_file(cfg.pid_file());
    // Give the writer thread a moment to flush the last lines.
    drop(log);
    thread::sleep(Duration::from_millis(200));
    Ok(())
}

fn spawn_server(cfg: &Config, log: &Logger) -> std::io::Result<Child> {
    let mut child = Command::new(cfg.binary())
        .args(cfg.server_args())
        .current_dir(&cfg.server_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    for (name, reader) in [("stdout", child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)), ("stderr", child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>))] {
        let Some(reader) = reader else { continue };
        let log = log.clone();
        let prefix = if name == "stderr" { "[stderr] " } else { "" };
        thread::Builder::new().name(format!("server-{name}")).spawn(move || {
            let mut r = BufReader::new(reader);
            let mut line = String::new();
            loop {
                line.clear();
                match r.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let l = line.trim_end_matches(['\r', '\n']);
                        if !l.trim().is_empty() {
                            log.server(&format!("{prefix}{l}"));
                        }
                    }
                }
            }
        })?;
    }
    Ok(child)
}

/// Wait for the child; on a stop request, terminate it gently, then firmly.
fn supervise(child: &mut Child, log: &Logger) -> Option<String> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.to_string()),
            Ok(None) => {}
            Err(e) => return Some(format!("wait failed: {e}")),
        }
        if STOP.load(Ordering::Relaxed) {
            log.note("stopping the server (SIGTERM)");
            procs::terminate(child.id() as i32);
            let deadline = Instant::now() + Duration::from_secs(15);
            while Instant::now() < deadline {
                if let Ok(Some(status)) = child.try_wait() {
                    return Some(status.to_string());
                }
                thread::sleep(Duration::from_millis(100));
            }
            log.note("server did not exit in 15 s, killing it");
            let _ = child.kill();
            let _ = child.wait();
            return Some("killed".to_string());
        }
        thread::sleep(Duration::from_millis(200));
    }
}
