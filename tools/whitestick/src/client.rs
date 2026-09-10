//! `whitestick '<command>'` — the devserver side. One WebSocket to the relay per
//! command; stdin, stdout, stderr and the exit status flow through it live.

use crate::config::Config;
use crate::proto::*;
use crate::transport::{self, Endpoint, WsError};
use anyhow::{anyhow, bail, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub struct RunOpts {
    pub instance: String,
    pub cmd: String,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub forward_stdin: bool,
    pub json: bool,
    /// How long to wait for the box to come online before giving up.
    pub wait: Duration,
    /// Kill the remote command and exit 124 after this long.
    pub timeout: Option<Duration>,
}

/// Connect a controller session, waiting up to `wait` for an offline box.
async fn connect(cfg: &Config, instance: &str, wait: Duration) -> Result<transport::Ws> {
    let ep = Endpoint::parse(cfg.relay()?)?;
    let token = cfg.token()?;
    let proxy = transport::resolve_proxy(cfg.proxy.as_deref()).await;
    let path = format!("/v1/ctl/{instance}");
    let deadline = Instant::now() + wait;
    let mut told = false;
    loop {
        match transport::websocket(&ep, proxy.as_ref(), token, &path).await {
            Ok(ws) => return Ok(ws),
            Err(WsError::Http { status: 503, .. }) if Instant::now() < deadline => {
                if !told {
                    eprintln!("[whitestick] {instance} is offline, waiting for it...");
                    told = true;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(WsError::Http { status: 503, .. }) => bail!(
                "{instance} is offline: no agent is connected to the relay \
                 (is `whitestick agent` running on the box?)"
            ),
            Err(WsError::Http { status: 401, .. }) => bail!(
                "the relay rejected the token (401): check `token` in {}",
                crate::config::config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ),
            Err(WsError::Http { status, body }) => {
                bail!("relay answered HTTP {status}: {}", body.trim())
            }
            Err(WsError::Other(e)) => {
                return Err(e.context(format!(
                    "cannot reach the relay at {}:{}{}",
                    ep.host,
                    ep.port,
                    match &proxy {
                        Some(p) => format!(" via proxy {p}"),
                        None => " (no proxy)".to_string(),
                    }
                )))
            }
        }
    }
}

pub async fn run(cfg: &Config, opts: RunOpts) -> Result<i32> {
    let ws = connect(cfg, &opts.instance, opts.wait).await?;
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::channel::<Message>(1024);
    let writer = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if sink.send(m).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let req = Req {
        cmd: opts.cmd.clone(),
        cwd: opts.cwd.clone(),
        shell: opts.shell.clone(),
        env: Default::default(),
        stdin: opts.forward_stdin,
    };
    tx.send(Message::Binary(frame(C_REQ, &serde_json::to_vec(&req)?).into()))
        .await
        .map_err(|_| anyhow!("connection closed before the request was sent"))?;

    let in_window = Arc::new(Window::new(WINDOW));
    if opts.forward_stdin {
        tokio::spawn(pump_stdin(tx.clone(), in_window.clone()));
    }

    let ping_tx = tx.clone();
    let pinger = tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(20));
        t.tick().await;
        loop {
            t.tick().await;
            if ping_tx.send(Message::Text("ping".into())).await.is_err() {
                break;
            }
        }
    });

    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut interrupts = 0u32;
    let deadline = opts.timeout.map(|t| tokio::time::Instant::now() + t);
    let far_future = tokio::time::Instant::now() + Duration::from_secs(365 * 24 * 3600);

    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();
    let mut out_buf: Vec<u8> = Vec::new();
    let mut err_buf: Vec<u8> = Vec::new();
    let mut error_text: Option<String> = None;

    let exit: Exit = loop {
        tokio::select! {
            m = stream.next() => match m {
                Some(Ok(Message::Binary(b))) => {
                    let Some((&tag, body)) = b.split_first() else { continue };
                    match tag {
                        A_STDOUT => {
                            if opts.json { out_buf.extend_from_slice(body); }
                            else { stdout.write_all(body).await?; stdout.flush().await?; }
                            let _ = tx.send(Message::Binary(ack(C_ACK, body.len()).into())).await;
                        }
                        A_STDERR => {
                            if opts.json { err_buf.extend_from_slice(body); }
                            else { stderr.write_all(body).await?; stderr.flush().await?; }
                            let _ = tx.send(Message::Binary(ack(C_ACK, body.len()).into())).await;
                        }
                        A_EXIT => {
                            break serde_json::from_slice::<Exit>(body).unwrap_or_default();
                        }
                        A_ERROR => {
                            let text = String::from_utf8_lossy(body).to_string();
                            eprintln!("[whitestick] {text}");
                            error_text = Some(text);
                        }
                        A_ACK => {
                            if let Some(n) = read_ack(body) { in_window.release(n); }
                        }
                        _ => {}
                    }
                }
                Some(Ok(Message::Text(_))) => {} // pong
                Some(Ok(Message::Ping(p))) => { let _ = tx.send(Message::Pong(p)).await; }
                Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
                Some(Ok(Message::Close(c))) => {
                    let code = c.as_ref().map(|cf| u16::from(cf.code)).unwrap_or(0);
                    bail!("{}", match code {
                        CLOSE_AGENT_OFFLINE => format!("{} went offline mid-command", opts.instance),
                        CLOSE_AGENT_REPLACED => format!("{} reconnected mid-command (agent replaced)", opts.instance),
                        _ => format!("the relay closed the connection ({code}: {})",
                                     c.map(|cf| cf.reason.to_string()).unwrap_or_default()),
                    });
                }
                Some(Err(e)) => bail!("connection lost: {e}"),
                None => bail!("connection closed"),
            },
            _ = sigint.recv() => {
                interrupts += 1;
                if interrupts >= 2 {
                    let _ = tx.send(Message::Binary(frame(C_SIGNAL, &[libc::SIGKILL as u8]).into())).await;
                    eprintln!("[whitestick] killed");
                    return Ok(130);
                }
                eprintln!("[whitestick] interrupt sent to the remote command (again to kill)");
                let _ = tx.send(Message::Binary(frame(C_SIGNAL, &[libc::SIGINT as u8]).into())).await;
            }
            _ = sigterm.recv() => {
                let _ = tx.send(Message::Binary(frame(C_SIGNAL, &[libc::SIGTERM as u8]).into())).await;
                return Ok(143);
            }
            _ = tokio::time::sleep_until(deadline.unwrap_or(far_future)), if deadline.is_some() => {
                let _ = tx.send(Message::Binary(frame(C_SIGNAL, &[libc::SIGKILL as u8]).into())).await;
                eprintln!("[whitestick] timeout after {} s, remote command killed",
                          opts.timeout.map(|t| t.as_secs()).unwrap_or(0));
                return Ok(124);
            }
        }
    };

    pinger.abort();
    if opts.json {
        let mut v = serde_json::json!({
            "success": true,
            "stdout": String::from_utf8_lossy(&out_buf),
            "stderr": String::from_utf8_lossy(&err_buf),
            "exitCode": exit.status(),
        });
        if let Some(e) = error_text {
            v["error"] = serde_json::Value::String(e);
        }
        println!("{v}");
    } else {
        stdout.flush().await?;
        stderr.flush().await?;
    }
    drop(tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), writer).await;
    Ok(exit.status())
}

async fn pump_stdin(tx: mpsc::Sender<Message>, window: Arc<Window>) {
    let mut stdin = tokio::io::stdin();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = match stdin.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        window.acquire(n as u64).await;
        if tx
            .send(Message::Binary(frame(C_STDIN, &buf[..n]).into()))
            .await
            .is_err()
        {
            return;
        }
    }
    let _ = tx.send(Message::Binary(frame(C_STDIN_EOF, &[]).into())).await;
}

/// `whitestick status`: is the box connected to the relay?
pub async fn status(cfg: &Config, instance: &str) -> Result<i32> {
    let ep = Endpoint::parse(cfg.relay()?)?;
    let token = cfg.token()?;
    let proxy = transport::resolve_proxy(cfg.proxy.as_deref()).await;
    let (status, body) =
        transport::http_get(&ep, proxy.as_ref(), token, &format!("/v1/status/{instance}")).await?;
    if status != 200 {
        bail!("relay answered HTTP {status}: {}", body.trim());
    }
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| anyhow!("unexpected (non-JSON) answer from {}:{} — is that really the relay?", ep.host, ep.port))?;
    let online = v["agent"].as_bool().unwrap_or(false);
    let since = v["agent_since_ms"].as_f64();
    let now = v["now_ms"].as_f64();
    let sessions = v["sessions"].as_u64().unwrap_or(0);
    if online {
        let up = match (since, now) {
            (Some(s), Some(n)) if n >= s => format!(", connected {} ago", human_secs((n - s) / 1000.0)),
            _ => String::new(),
        };
        println!("{instance}: online{up}, {sessions} session(s)");
        Ok(0)
    } else {
        println!("{instance}: OFFLINE (no agent connected to the relay)");
        Ok(3)
    }
}

fn human_secs(s: f64) -> String {
    let s = s as u64;
    if s < 90 {
        format!("{s} s")
    } else if s < 5400 {
        format!("{} min", s / 60)
    } else if s < 172_800 {
        format!("{:.1} h", s as f64 / 3600.0)
    } else {
        format!("{:.1} days", s as f64 / 86400.0)
    }
}
