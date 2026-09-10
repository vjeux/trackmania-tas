//! `whitestick relay` — the rendezvous point, as a plain process on a box with
//! a public IP. Same job and the same wire protocol as the Cloudflare Worker in
//! `tools/whitestick-relay`, for people who would rather run a VPS than an
//! account: it terminates TLS, checks one shared secret, and pairs each
//! controller with the agent of the box it names.
//!
//! ```text
//!   /v1/agent/<box>   the box connects here (one at a time; newest wins)
//!   /v1/ctl/<box>     a controller connects here, one per command
//!   /v1/status/<box>  plain GET: {"agent":bool,"agent_since_ms":…,"sessions":N}
//!   /healthz          plain GET, no auth
//! ```
//!
//! Frames between the relay and the agent are `[u32 BE channel][u8 kind][body]`
//! with kind OPEN/DATA/CLOSE; a controller's own socket carries the body only.
//! Nothing here reads the body — flow control lives at the two ends.
//!
//! The certificate is normally self-signed (a VPS with a bare IP has no name to
//! get a public certificate for), so clients pin its SHA-256 rather than
//! trusting a CA: `whitestick relay --print-pin` prints the line for their
//! configs.

use crate::proto::*;
use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub struct RelayOpts {
    pub listen: SocketAddr,
    pub cert: String,
    pub key: String,
    pub token: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Agent,
    Ctl,
}

struct Agent {
    id: u64,
    tx: mpsc::Sender<Message>,
    since_ms: u64,
}

/// What the relay sends down a controller's socket.
enum CtlOut {
    Data(Vec<u8>),
    /// The agent is gone: close with a code the client can name.
    Gone,
}

#[derive(Default)]
struct BoxState {
    agent: Option<Agent>,
    ctls: HashMap<u32, mpsc::Sender<CtlOut>>,
}

#[derive(Default)]
struct Relay {
    boxes: Mutex<HashMap<String, BoxState>>,
    next_id: AtomicU64,
    next_chan: AtomicU64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn valid_box_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'.')
}

/// The SHA-256 of a certificate's DER, the way the clients pin it.
pub fn cert_pin(cert_pem_path: &str) -> Result<String> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::CertificateDer;
    let pem = std::fs::read(cert_pem_path)
        .with_context(|| format!("read certificate {cert_pem_path}"))?;
    let first = CertificateDer::pem_slice_iter(&pem)
        .next()
        .ok_or_else(|| anyhow!("no certificate in {cert_pem_path}"))?
        .with_context(|| format!("parse certificate {cert_pem_path}"))?;
    Ok(crate::transport::sha256_hex(first.as_ref()))
}

fn tls_acceptor(cert: &str, key: &str) -> Result<tokio_rustls::TlsAcceptor> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let cert_pem = std::fs::read(cert).with_context(|| format!("read certificate {cert}"))?;
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&cert_pem)
        .collect::<std::result::Result<_, _>>()
        .with_context(|| format!("parse certificates in {cert}"))?;
    if certs.is_empty() {
        bail!("no certificate in {cert}");
    }
    let key_der = PrivateKeyDer::from_pem_file(key)
        .with_context(|| format!("read private key {key}"))?;
    let cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key_der)
        .context("server certificate")?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(cfg)))
}

pub async fn run(opts: RelayOpts) -> Result<()> {
    let acceptor = tls_acceptor(&opts.cert, &opts.key)?;
    let listener = TcpListener::bind(opts.listen)
        .await
        .with_context(|| format!("bind {}", opts.listen))?;
    let relay = Arc::new(Relay::default());
    let token = Arc::new(opts.token);
    crate::log(&format!(
        "whitestick relay {} listening on {} (cert {}, pin {})",
        env!("CARGO_PKG_VERSION"),
        opts.listen,
        opts.cert,
        cert_pin(&opts.cert).unwrap_or_else(|_| "?".into())
    ));

    loop {
        let (tcp, peer) = match listener.accept().await {
            Ok(x) => x,
            Err(e) => {
                crate::log(&format!("accept failed: {e}"));
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
        };
        tcp.set_nodelay(true).ok();
        let acceptor = acceptor.clone();
        let relay = relay.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let tls = match tokio::time::timeout(Duration::from_secs(20), acceptor.accept(tcp)).await
            {
                Ok(Ok(s)) => s,
                // Scanners hit any open port, so this is mostly noise — but it
                // is also exactly what a client with the wrong pin looks like
                // (pinning is checked client-side, so the refusal arrives here
                // as a TLS alert), and that is the one failure nobody can see
                // from either end. Worth the line.
                Ok(Err(e)) => {
                    crate::log(&format!("{peer}: tls handshake failed: {e}"));
                    return;
                }
                Err(_) => {
                    crate::log(&format!("{peer}: tls handshake timed out"));
                    return;
                }
            };
            if let Err(e) = serve_conn(tls, peer, relay, token).await {
                let msg = e.to_string();
                // Connections end all the time; only say something surprising.
                if !msg.contains("Connection reset") && !msg.contains("without closing handshake") {
                    crate::log(&format!("{peer}: {msg}"));
                }
            }
        });
    }
}

/// A stream with some bytes pushed back in front of it: the request head is
/// read by hand (so plain GETs can be answered without tungstenite's upgrade
/// parser rejecting them first), then replayed for the handshake.
struct Rewind<S> {
    prefix: Vec<u8>,
    at: usize,
    inner: S,
}

impl<S> Rewind<S> {
    fn new(prefix: Vec<u8>, inner: S) -> Self {
        Self { prefix, at: 0, inner }
    }
}

impl<S: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for Rewind<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.at < self.prefix.len() {
            let n = buf.remaining().min(self.prefix.len() - self.at);
            let at = self.at;
            buf.put_slice(&self.prefix[at..at + n]);
            self.at += n;
            return std::task::Poll::Ready(Ok(()));
        }
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for Rewind<S> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// The request line and headers, read one byte at a time up to the blank line.
async fn read_head<S: tokio::io::AsyncRead + Unpin>(s: &mut S) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut head = Vec::with_capacity(1024);
    let mut b = [0u8; 1];
    loop {
        if s.read(&mut b).await? == 0 {
            bail!("closed before the request head was complete");
        }
        head.push(b[0]);
        if head.ends_with(b"\r\n\r\n") {
            return Ok(head);
        }
        if head.len() > 16 * 1024 {
            bail!("request head too long");
        }
    }
}

async fn write_http<S: tokio::io::AsyncWrite + Unpin>(
    s: &mut S,
    code: u16,
    reason: &str,
    ctype: &str,
    body: &str,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let resp = format!(
        "HTTP/1.1 {code} {reason}\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(resp.as_bytes()).await?;
    s.flush().await?;
    Ok(())
}

async fn serve_conn<S>(
    mut stream: S,
    peer: SocketAddr,
    relay: Arc<Relay>,
    token: Arc<String>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let head = match tokio::time::timeout(Duration::from_secs(20), read_head(&mut stream)).await {
        Ok(Ok(h)) => h,
        _ => return Ok(()),
    };
    let text = String::from_utf8_lossy(&head).to_string();
    let mut lines = text.lines();
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut parts = request_line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let path = path.split('?').next().unwrap_or("");
    let mut auth = String::new();
    let mut upgrade = false;
    for l in lines {
        let Some((k, v)) = l.split_once(':') else { continue };
        match k.trim().to_ascii_lowercase().as_str() {
            "authorization" => auth = v.trim().to_string(),
            "upgrade" => upgrade = v.trim().eq_ignore_ascii_case("websocket"),
            _ => {}
        }
    }

    if path == "/healthz" || path == "/" {
        return write_http(&mut stream, 200, "OK", "text/plain", "whitestick-relay\n").await;
    }
    if method != "GET" {
        return write_http(&mut stream, 405, "Method Not Allowed", "text/plain", "GET only\n").await;
    }
    let presented = auth.strip_prefix("Bearer ").unwrap_or("");
    if !constant_time_eq(presented.as_bytes(), token.as_bytes()) {
        return write_http(&mut stream, 401, "Unauthorized", "text/plain", "unauthorized\n").await;
    }

    let mut it = path.trim_start_matches('/').split('/');
    let (v, role, name) = (it.next(), it.next(), it.next());
    if v != Some("v1") || it.next().is_some() {
        return write_http(&mut stream, 404, "Not Found", "text/plain", "not found\n").await;
    }
    let name = match name {
        Some(n) if valid_box_name(n) => n.to_string(),
        _ => return write_http(&mut stream, 400, "Bad Request", "text/plain", "bad box name\n").await,
    };
    let role = match role {
        Some("status") => {
            let body = relay.status_json(&name);
            return write_http(&mut stream, 200, "OK", "application/json", &body).await;
        }
        Some("agent") => Role::Agent,
        Some("ctl") => Role::Ctl,
        _ => return write_http(&mut stream, 404, "Not Found", "text/plain", "not found\n").await,
    };
    if !upgrade {
        return write_http(&mut stream, 426, "Upgrade Required", "text/plain", "expected a websocket upgrade\n").await;
    }
    if role == Role::Ctl && !relay.has_agent(&name) {
        return write_http(&mut stream, 503, "Service Unavailable", "text/plain", "agent offline\n").await;
    }

    let ws = match tokio_tungstenite::accept_async(Rewind::new(head, stream)).await {
        Ok(ws) => ws,
        Err(e) => return Err(anyhow!("handshake: {e}")),
    };
    match role {
        Role::Agent => serve_agent(ws, name, peer, relay).await,
        Role::Ctl => serve_ctl(ws, name, peer, relay).await,
    }
}

impl Relay {
    fn has_agent(&self, name: &str) -> bool {
        self.boxes
            .lock()
            .unwrap()
            .get(name)
            .map(|b| b.agent.is_some())
            .unwrap_or(false)
    }

    fn status_json(&self, name: &str) -> String {
        let boxes = self.boxes.lock().unwrap();
        let (online, since, sessions) = match boxes.get(name) {
            Some(b) => (
                b.agent.is_some(),
                b.agent.as_ref().map(|a| a.since_ms),
                b.ctls.len(),
            ),
            None => (false, None, 0),
        };
        format!(
            "{{\"agent\":{online},\"agent_since_ms\":{},\"sessions\":{sessions},\"now_ms\":{}}}",
            since.map(|s| s.to_string()).unwrap_or("null".into()),
            now_ms()
        )
    }

    /// Send one frame to the box's agent. False when there is no agent.
    fn to_agent(&self, name: &str, msg: Message) -> bool {
        let boxes = self.boxes.lock().unwrap();
        match boxes.get(name).and_then(|b| b.agent.as_ref()) {
            Some(a) => a.tx.try_send(msg).is_ok(),
            None => false,
        }
    }
}

async fn serve_agent<S>(
    ws: tokio_tungstenite::WebSocketStream<S>,
    name: String,
    peer: SocketAddr,
    relay: Arc<Relay>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let id = relay.next_id.fetch_add(1, Ordering::Relaxed) + 1;
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

    // Newest agent wins: the box rebooted, or the old socket is half-dead.
    let evicted: Vec<mpsc::Sender<CtlOut>> = {
        let mut boxes = relay.boxes.lock().unwrap();
        let b = boxes.entry(name.clone()).or_default();
        let old = b.agent.replace(Agent {
            id,
            tx: tx.clone(),
            since_ms: now_ms(),
        });
        if old.is_some() {
            b.ctls.drain().map(|(_, c)| c).collect()
        } else {
            Vec::new()
        }
    };
    for c in &evicted {
        let _ = c.try_send(CtlOut::Gone);
    }
    if !evicted.is_empty() {
        crate::log(&format!(
            "{name}: agent #{id} from {peer} replaced the previous one; {} session(s) dropped",
            evicted.len()
        ));
    } else {
        crate::log(&format!("{name}: agent #{id} connected from {peer}"));
    }
    drop(evicted); // dropping the senders closes those controllers

    let reason = loop {
        match stream.next().await {
            Some(Ok(Message::Binary(b))) => {
                let Some((chan, kind, body)) = open_envelope(&b) else { continue };
                let ctl = relay
                    .boxes
                    .lock()
                    .unwrap()
                    .get(&name)
                    .and_then(|s| s.ctls.get(&chan).cloned());
                match (kind, ctl) {
                    (K_DATA, Some(c)) => {
                        if c.send(CtlOut::Data(body.to_vec())).await.is_err() {
                            relay
                                .boxes
                                .lock()
                                .unwrap()
                                .get_mut(&name)
                                .map(|s| s.ctls.remove(&chan));
                        }
                    }
                    (K_CLOSE, _) => {
                        relay
                            .boxes
                            .lock()
                            .unwrap()
                            .get_mut(&name)
                            .map(|s| s.ctls.remove(&chan));
                    }
                    _ => {}
                }
            }
            // The agent's application-level keepalive.
            Some(Ok(Message::Text(t))) if t == "ping" => {
                let _ = tx.send(Message::Text("pong".into())).await;
            }
            Some(Ok(Message::Ping(p))) => {
                let _ = tx.send(Message::Pong(p)).await;
            }
            Some(Ok(_)) => {}
            Some(Err(e)) => break format!("read error: {e}"),
            None => break "stream ended".to_string(),
        }
    };

    // Only tear the box down if this is still the agent of record: a replaced
    // agent's own disconnect must not evict its successor.
    let orphans: Vec<mpsc::Sender<CtlOut>> = {
        let mut boxes = relay.boxes.lock().unwrap();
        match boxes.get_mut(&name) {
            Some(b) if b.agent.as_ref().map(|a| a.id) == Some(id) => {
                b.agent = None;
                b.ctls.drain().map(|(_, c)| c).collect()
            }
            _ => Vec::new(),
        }
    };
    for c in &orphans {
        let _ = c.try_send(CtlOut::Gone);
    }
    crate::log(&format!(
        "{name}: agent #{id} gone ({reason}); {} session(s) dropped",
        orphans.len()
    ));
    drop(orphans);
    drop(tx);
    let _ = tokio::time::timeout(Duration::from_secs(5), writer).await;
    Ok(())
}

async fn serve_ctl<S>(
    ws: tokio_tungstenite::WebSocketStream<S>,
    name: String,
    peer: SocketAddr,
    relay: Arc<Relay>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::channel::<CtlOut>(1024);

    // A channel number the agent has not got open right now.
    let chan = {
        let mut boxes = relay.boxes.lock().unwrap();
        let b = boxes.entry(name.clone()).or_default();
        if b.agent.is_none() {
            return Ok(()); // it went offline between the check and here
        }
        let mut c;
        loop {
            c = (relay.next_chan.fetch_add(1, Ordering::Relaxed) as u32).wrapping_add(1);
            if c != 0 && !b.ctls.contains_key(&c) {
                break;
            }
        }
        b.ctls.insert(c, tx);
        c
    };
    if !relay.to_agent(&name, Message::Binary(envelope(chan, K_OPEN, &[]).into())) {
        relay
            .boxes
            .lock()
            .unwrap()
            .get_mut(&name)
            .map(|b| b.ctls.remove(&chan));
        return Ok(());
    }
    crate::log(&format!("{name}: session {chan:08x} from {peer}"));

    // Controller -> agent, and agent -> controller, until either end stops.
    let up = async {
        while let Some(m) = stream.next().await {
            match m {
                Ok(Message::Binary(b)) => {
                    if !relay.to_agent(&name, Message::Binary(envelope(chan, K_DATA, &b).into())) {
                        break;
                    }
                }
                Ok(Message::Text(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {}
                Ok(Message::Ping(_)) => {}
                Ok(Message::Close(_)) | Err(_) => break,
            }
        }
    };
    let down = async {
        while let Some(out) = rx.recv().await {
            let sent = match out {
                CtlOut::Data(b) => sink.send(Message::Binary(b.into())).await,
                CtlOut::Gone => {
                    use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
                    use tokio_tungstenite::tungstenite::protocol::CloseFrame;
                    let _ = sink
                        .send(Message::Close(Some(CloseFrame {
                            code: CloseCode::Library(CLOSE_AGENT_OFFLINE),
                            reason: "agent offline".into(),
                        })))
                        .await;
                    break;
                }
            };
            if sent.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    };
    tokio::select! {
        _ = up => {}
        _ = down => {}
    }

    let still_ours = {
        let mut boxes = relay.boxes.lock().unwrap();
        boxes
            .get_mut(&name)
            .map(|b| b.ctls.remove(&chan).is_some())
            .unwrap_or(false)
    };
    if still_ours {
        relay.to_agent(&name, Message::Binary(envelope(chan, K_CLOSE, &[]).into()));
    }
    crate::log(&format!("{name}: session {chan:08x} ended"));
    Ok(())
}
