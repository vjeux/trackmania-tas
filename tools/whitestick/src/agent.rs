//! `whitestick agent` — the box side. Keeps one WebSocket to the relay and runs
//! whatever the controllers on the other end ask for, one channel per command.
//!
//! Reconnects forever with a capped backoff; there is no state worth keeping
//! across a reconnect except the commands that were running, and those are
//! killed (with their process group) the moment their channel disappears.

use crate::config::Config;
use crate::proto::*;
use crate::transport::{self, Ws};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub struct AgentOpts {
    pub name: String,
    pub cwd: Option<String>,
    pub shell: String,
}

/// Environment the box-side commands must not inherit: a proxy setting that
/// is right for the agent's own connection is wrong for everything it runs
/// (navi-node taught that one the hard way).
const PROXY_VARS: &[&str] = &[
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
];

const PING_EVERY: Duration = Duration::from_secs(20);
const DEAD_AFTER: Duration = Duration::from_secs(75);
/// After the command exits, how long an open-but-silent output pipe is waited for.
const DRAIN_IDLE: Duration = Duration::from_secs(10);

pub async fn run(cfg: Config, opts: AgentOpts) -> Result<()> {
    let ep = cfg.endpoint()?;
    let token = cfg.token()?.to_string();
    let proxy = transport::resolve_proxy(cfg.proxy.as_deref()).await;
    let path = format!("/v1/agent/{}", opts.name);
    let opts = Arc::new(opts);
    crate::log(&format!(
        "whitestick agent {} — box \"{}\", relay {}://{}:{}{}, cwd {}, shell {}",
        env!("CARGO_PKG_VERSION"),
        opts.name,
        if ep.tls { "wss" } else { "ws" },
        ep.host,
        ep.port,
        match &proxy {
            Some(p) => format!(" via proxy {p}"),
            None => String::new(),
        },
        opts.cwd.as_deref().unwrap_or("(inherited)"),
        opts.shell
    ));

    let mut backoff = 1u64;
    loop {
        match transport::websocket(&ep, proxy.as_ref(), &token, &path).await {
            Ok(ws) => {
                crate::log("connected to relay");
                backoff = 1;
                let started = Instant::now();
                let reason = serve(ws, opts.clone()).await;
                crate::log(&format!(
                    "disconnected after {} s: {reason}",
                    started.elapsed().as_secs()
                ));
                if reason.contains("(4000:") {
                    // Another agent took over this box name. Two copies fighting
                    // every second would kill every session on both; stand back.
                    crate::log("another agent is connected under this box name — is a second copy running? retrying in 60 s");
                    backoff = 60;
                }
            }
            Err(e) => {
                crate::log(&format!("connect failed: {e}"));
                // A bad token or a missing relay will not fix itself quickly.
                if matches!(e, transport::WsError::Http { status: 401 | 404, .. }) {
                    backoff = 30;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

/// Drive one relay connection until it dies. Returns why.
async fn serve(ws: Ws, opts: Arc<AgentOpts>) -> String {
    let (mut sink, mut stream) = ws.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(1024);

    let writer = tokio::spawn(async move {
        while let Some(m) = out_rx.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let ping_tx = out_tx.clone();
    let pinger = tokio::spawn(async move {
        let mut t = tokio::time::interval(PING_EVERY);
        t.tick().await;
        loop {
            t.tick().await;
            if ping_tx.send(Message::Text("ping".into())).await.is_err() {
                break;
            }
        }
    });

    let mut chans: HashMap<u32, mpsc::Sender<Vec<u8>>> = HashMap::new();
    let mut last_seen = Instant::now();
    let reason = loop {
        let msg = tokio::select! {
            m = stream.next() => m,
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                if last_seen.elapsed() > DEAD_AFTER {
                    break format!("nothing heard from the relay for {} s", DEAD_AFTER.as_secs());
                }
                continue;
            }
        };
        last_seen = Instant::now();
        match msg {
            Some(Ok(Message::Binary(b))) => {
                let Some((chan, kind, body)) = open_envelope(&b) else { continue };
                match kind {
                    K_OPEN => {
                        let (tx, rx) = mpsc::channel::<Vec<u8>>(256);
                        chans.insert(chan, tx);
                        tokio::spawn(run_channel(chan, rx, out_tx.clone(), opts.clone()));
                    }
                    K_DATA => {
                        if let Some(tx) = chans.get(&chan) {
                            if tx.send(body.to_vec()).await.is_err() {
                                chans.remove(&chan);
                            }
                        }
                    }
                    K_CLOSE => {
                        chans.remove(&chan);
                    }
                    _ => {}
                }
            }
            Some(Ok(Message::Text(_))) => {} // "pong"
            Some(Ok(Message::Ping(p))) => {
                let _ = out_tx.send(Message::Pong(p)).await;
            }
            Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
            Some(Ok(Message::Close(c))) => {
                break match c {
                    Some(cf) => format!("closed by relay ({}: {})", u16::from(cf.code), cf.reason),
                    None => "closed by relay".to_string(),
                }
            }
            Some(Err(e)) => break format!("read error: {e}"),
            None => break "stream ended".to_string(),
        }
    };

    // Dropping the senders ends every channel task, which kills its command.
    drop(chans);
    pinger.abort();
    drop(out_tx);
    let _ = tokio::time::timeout(Duration::from_secs(5), writer).await;
    reason
}

fn data(chan: u32, tag: u8, body: &[u8]) -> Message {
    Message::Binary(envelope(chan, K_DATA, &frame(tag, body)).into())
}

fn kill_group(pid: i32, sig: i32) {
    if pid > 0 {
        unsafe {
            libc::kill(-pid, sig);
            libc::kill(pid, sig);
        }
    }
}

fn summarize(cmd: &str) -> String {
    let one_line: String = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 120 {
        let cut: String = one_line.chars().take(117).collect();
        format!("{cut}... ({} bytes)", cmd.len())
    } else {
        one_line
    }
}

/// One controller session: expect a REQ, run it, stream the result.
async fn run_channel(
    chan: u32,
    mut inbox: mpsc::Receiver<Vec<u8>>,
    out: mpsc::Sender<Message>,
    opts: Arc<AgentOpts>,
) {
    let tag = format!("[{chan:08x}]");
    let req: Req = match inbox.recv().await {
        Some(f) if f.first() == Some(&C_REQ) => match serde_json::from_slice(&f[1..]) {
            Ok(r) => r,
            Err(e) => {
                let _ = out.send(data(chan, A_ERROR, format!("bad request: {e}").as_bytes())).await;
                let _ = out.send(data(chan, A_EXIT, b"{\"code\":2}")).await;
                return;
            }
        },
        Some(_) => {
            let _ = out.send(data(chan, A_ERROR, b"protocol error: expected a request first")).await;
            let _ = out.send(data(chan, A_EXIT, b"{\"code\":2}")).await;
            return;
        }
        None => return,
    };

    let shell = req.shell.clone().unwrap_or_else(|| opts.shell.clone());
    let cwd = req.cwd.clone().or_else(|| opts.cwd.clone());
    let mut cmd = tokio::process::Command::new(&shell);

    // A big command goes to the shell as a FILE, not as `-c <string>`.
    // Linux caps a single argv entry at MAX_ARG_STRLEN (128 KiB, not
    // tunable), and `wsx push` sends half a megabyte of base64 inside the
    // command itself -- exec fails with E2BIG ("Argument list too long")
    // before the shell ever runs. The old navi bridge did not exec this way,
    // so this limit has to stay invisible here too. Small commands keep the
    // fast path: no file, no cleanup.
    let script = if req.cmd.len() > 96 * 1024 {
        let path = std::env::temp_dir().join(format!(
            "whitestick-cmd-{}-{chan:08x}.sh",
            std::process::id()
        ));
        match tokio::fs::write(&path, req.cmd.as_bytes()).await {
            Ok(()) => {
                cmd.arg(&path);
                Some(path)
            }
            Err(e) => {
                let msg = format!(
                    "command is {} bytes, too long for one exec argument, and the script file {} could not be written: {e}",
                    req.cmd.len(),
                    path.display()
                );
                crate::log(&format!("{tag} {msg}"));
                let _ = out.send(data(chan, A_ERROR, msg.as_bytes())).await;
                let _ = out.send(data(chan, A_EXIT, b"{\"code\":126}")).await;
                return;
            }
        }
    } else {
        cmd.arg("-c").arg(&req.cmd);
        None
    };
    if let Some(d) = &cwd {
        cmd.current_dir(d);
    }
    for k in PROXY_VARS {
        cmd.env_remove(k);
    }
    cmd.env("WHITESTICK", "1");
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    cmd.stdin(if req.stdin { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(false);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            if let Some(p) = &script {
                let _ = tokio::fs::remove_file(p).await;
            }
            let msg = format!(
                "cannot start `{shell}`{}: {e}",
                cwd.as_deref().map(|d| format!(" in {d}")).unwrap_or_default()
            );
            crate::log(&format!("{tag} {msg}"));
            let _ = out.send(data(chan, A_ERROR, msg.as_bytes())).await;
            let _ = out.send(data(chan, A_EXIT, b"{\"code\":127}")).await;
            return;
        }
    };
    let pid = child.id().unwrap_or(0) as i32;
    let started = Instant::now();
    crate::log(&format!("{tag} pid {pid}: {}", summarize(&req.cmd)));

    // Output goes through a shared byte window; `progress` counts chunks
    // delivered so the drain below can tell "still flowing" from "stuck".
    let window = Arc::new(Window::new(WINDOW));
    let progress = Arc::new(AtomicU64::new(0));
    let rd_out = tokio::spawn(pump_out(
        child.stdout.take(), chan, A_STDOUT, out.clone(), window.clone(), progress.clone(),
    ));
    let rd_err = tokio::spawn(pump_out(
        child.stderr.take(), chan, A_STDERR, out.clone(), window.clone(), progress.clone(),
    ));
    let mut drain = Box::pin(async move {
        let _ = rd_out.await;
        let _ = rd_err.await;
    });

    // stdin gets its own task so a command that never reads it cannot stall
    // the control loop (signals, acks, the client going away).
    let (stdin_tx, mut stdin_rx) = mpsc::channel::<Option<Vec<u8>>>(256);
    let stdin_pipe = child.stdin.take();
    let stdin_out = out.clone();
    let stdin_task = tokio::spawn(async move {
        let mut pipe = stdin_pipe;
        while let Some(item) = stdin_rx.recv().await {
            match item {
                Some(bytes) => {
                    if let Some(p) = pipe.as_mut() {
                        if p.write_all(&bytes).await.is_err() {
                            pipe = None;
                        }
                    }
                    // Acknowledge even when the pipe is gone: the client's
                    // window must keep moving so it reaches EOF and the exit.
                    let _ = stdin_out.send(data(chan, A_ACK, &(bytes.len() as u32).to_be_bytes())).await;
                }
                None => {
                    pipe = None; // EOF: dropping the handle closes the pipe
                }
            }
        }
    });

    // The loop ends when the command has exited AND its output has drained.
    // ACKs keep being processed the whole time: output after the exit (the
    // tail still in the pipe, or a slow reader on the other end) still needs
    // the window to move. A pipe that stays open after the exit with nothing
    // flowing for a while (a daemon started without setsid) is given up on.
    let mut inbox_open = true;
    let mut exit: Option<Exit> = None;
    let mut drained = false;
    let mut last_progress = (0u64, Instant::now());
    loop {
        if drained && exit.is_some() {
            break;
        }
        tokio::select! {
            status = child.wait(), if exit.is_none() => {
                exit = Some(match status {
                    Ok(s) => {
                        use std::os::unix::process::ExitStatusExt;
                        Exit { code: s.code(), signal: s.signal() }
                    }
                    Err(_) => Exit { code: Some(1), signal: None },
                });
                last_progress = (progress.load(Ordering::Relaxed), Instant::now());
            }
            _ = &mut drain, if !drained => { drained = true; }
            f = inbox.recv(), if inbox_open => match f {
                None => {
                    // Nobody is listening any more: kill the command and stop.
                    inbox_open = false;
                    crate::log(&format!("{tag} controller gone, killing pid {pid}"));
                    kill_group(pid, libc::SIGKILL);
                    drained = true;
                    if exit.is_none() {
                        exit = Some(match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                            Ok(Ok(s)) => {
                                use std::os::unix::process::ExitStatusExt;
                                Exit { code: s.code(), signal: s.signal() }
                            }
                            _ => Exit { code: None, signal: Some(libc::SIGKILL) },
                        });
                    }
                }
                Some(f) => match f.first() {
                    Some(&C_STDIN) => { let _ = stdin_tx.send(Some(f[1..].to_vec())).await; }
                    Some(&C_STDIN_EOF) => { let _ = stdin_tx.send(None).await; }
                    Some(&C_SIGNAL) => {
                        let sig = f.get(1).copied().unwrap_or(15) as i32;
                        crate::log(&format!("{tag} signal {sig} -> pid {pid}"));
                        kill_group(pid, sig);
                    }
                    Some(&C_ACK) => {
                        if let Some(n) = read_ack(&f[1..]) { window.release(n); }
                    }
                    _ => {}
                },
            },
            _ = tokio::time::sleep(Duration::from_secs(1)), if exit.is_some() && !drained => {
                let p = progress.load(Ordering::Relaxed);
                if p != last_progress.0 {
                    last_progress = (p, Instant::now());
                } else if last_progress.1.elapsed() > DRAIN_IDLE {
                    crate::log(&format!(
                        "{tag} pid {pid} exited but its output pipe is still open with nothing flowing for {} s (background child?); finishing without it",
                        DRAIN_IDLE.as_secs()
                    ));
                    drained = true;
                }
            }
        }
    }
    drop(drain);
    drop(stdin_tx);
    stdin_task.abort();

    // Now that the shell has finished with it. (Unlinking right after spawn
    // loses the race: the child has not exec'd, let alone opened the script.)
    if let Some(p) = &script {
        let _ = tokio::fs::remove_file(p).await;
    }

    let exit = exit.unwrap_or_default();
    let body = serde_json::to_vec(&exit).unwrap_or_else(|_| b"{}".to_vec());
    if inbox_open {
        let _ = out.send(data(chan, A_EXIT, &body)).await;
    }
    crate::log(&format!(
        "{tag} pid {pid} exit {} after {:.3} s",
        exit.status(),
        started.elapsed().as_secs_f64()
    ));
}

async fn pump_out(
    r: Option<impl AsyncRead + Unpin>,
    chan: u32,
    tag: u8,
    out: mpsc::Sender<Message>,
    window: Arc<Window>,
    progress: Arc<AtomicU64>,
) {
    let Some(mut r) = r else { return };
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = match r.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        window.acquire(n as u64).await;
        if out.send(data(chan, tag, &buf[..n])).await.is_err() {
            break;
        }
        progress.fetch_add(1, Ordering::Relaxed);
    }
}
