//! Getting a socket to the relay: TCP (directly, or through an HTTP CONNECT
//! proxy such as Meta's `fwdproxy`), then TLS, then the WebSocket handshake.
//!
//! Two proxy flavours:
//! - `http://host:port` — plain CONNECT (fwdproxy:8080 with transparent auth;
//!   dead on some devservers, e.g. devvm42752 resets it);
//! - `https://host:port` — TLS to the proxy first, authenticated with the
//!   host's x509 identity, then CONNECT inside that (fwdproxy:8082, what
//!   `fwdproxy-config` recommends), then the relay's own TLS inside THAT.
//!
//! Proxy choice, first match wins: `WHITESTICK_PROXY` (`none` forces a direct
//! connection), the config's `proxy`, `https_proxy` / `HTTPS_PROXY`, and
//! finally — when the name `fwdproxy` resolves, i.e. on any devserver or OD —
//! `https://fwdproxy:8082` if a usable certificate is on disk, else
//! `http://fwdproxy:8080`. At home nothing resolves and the box dials direct.

use anyhow::{anyhow, bail, Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::WebSocketStream;
use url::Url;

pub trait AsyncRw: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncRw for T {}
pub type Stream = Box<dyn AsyncRw>;
pub type Ws = WebSocketStream<Stream>;

/// Certificates that authenticate this host to fwdproxy:8082, in order of
/// preference: the host identity (world-readable, what fwdproxy-config uses),
/// then the user's own.
const PROXY_CERTS: &[&str] = &["/var/facebook/x509_identities/server.pem"];
const PROXY_CA: &str = "/var/facebook/rootcanal/ca.pem";

/// Where the relay lives, with the pieces every connection needs.
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl Endpoint {
    pub fn parse(relay: &str) -> Result<Self> {
        let url = Url::parse(relay).with_context(|| format!("relay url {relay}"))?;
        let tls = match url.scheme() {
            "https" | "wss" => true,
            "http" | "ws" => false,
            s => bail!("relay url scheme {s}: use https:// (or http:// for a local wrangler dev)"),
        };
        let host = url
            .host_str()
            .ok_or_else(|| anyhow!("relay url has no host: {relay}"))?
            .to_string();
        let port = url.port().unwrap_or(if tls { 443 } else { 80 });
        Ok(Self { host, port, tls })
    }

    fn ws_url(&self, path: &str) -> String {
        format!(
            "{}://{}:{}{}",
            if self.tls { "wss" } else { "ws" },
            self.host,
            self.port,
            path
        )
    }
}

#[derive(Debug, Clone)]
pub enum Proxy {
    /// Plain HTTP CONNECT.
    Http { host: String, port: u16 },
    /// TLS to the proxy (client certificate), CONNECT inside.
    Https { host: String, port: u16, cert: String },
}

impl std::fmt::Display for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Proxy::Http { host, port } => write!(f, "{host}:{port}"),
            Proxy::Https { host, port, .. } => write!(f, "{host}:{port} (tls)"),
        }
    }
}

fn user_cert_path() -> Option<String> {
    let user = std::env::var("USER").ok()?;
    Some(format!("/var/facebook/credentials/{user}/x509/{user}.pem"))
}

fn find_proxy_cert() -> Option<String> {
    let mut candidates: Vec<String> = PROXY_CERTS.iter().map(|s| s.to_string()).collect();
    if let Some(u) = user_cert_path() {
        candidates.push(u);
    }
    candidates
        .into_iter()
        .find(|p| std::fs::read(p).map(|b| b.windows(11).any(|w| w == b"PRIVATE KEY")).unwrap_or(false))
}

fn parse_proxy(v: &str) -> Option<Proxy> {
    let v = v.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("none") || v.eq_ignore_ascii_case("direct") {
        return None;
    }
    let (tls, rest) = if let Some(r) = v.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = v.strip_prefix("http://") {
        (false, r)
    } else {
        (false, v)
    };
    let rest = rest.trim_end_matches('/');
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().ok()?),
        None => (rest.to_string(), if tls { 8082 } else { 8080 }),
    };
    if tls {
        let cert = std::env::var("WHITESTICK_PROXY_CERT")
            .ok()
            .filter(|c| !c.is_empty())
            .or_else(find_proxy_cert)?;
        Some(Proxy::Https { host, port, cert })
    } else {
        Some(Proxy::Http { host, port })
    }
}

/// The proxy to use, if any.
pub async fn resolve_proxy(configured: Option<&str>) -> Option<Proxy> {
    if let Ok(v) = std::env::var("WHITESTICK_PROXY") {
        return parse_proxy(&v);
    }
    if let Some(v) = configured {
        return parse_proxy(v);
    }
    for key in ["https_proxy", "HTTPS_PROXY"] {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                return parse_proxy(&v);
            }
        }
    }
    // The corp default: present on every devserver/OD, absent everywhere else.
    if tokio::net::lookup_host(("fwdproxy", 8082)).await.is_ok() {
        if let Some(cert) = find_proxy_cert() {
            return Some(Proxy::Https {
                host: "fwdproxy".to_string(),
                port: 8082,
                cert,
            });
        }
        return Some(Proxy::Http {
            host: "fwdproxy".to_string(),
            port: 8080,
        });
    }
    None
}

async fn read_connect_response<S: AsyncRead + Unpin>(s: &mut S, what: &str) -> Result<()> {
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        let n = s.read(&mut byte).await?;
        if n == 0 {
            bail!("{what} closed the connection during CONNECT");
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
        if buf.len() > 16 * 1024 {
            bail!("{what}: CONNECT response too long");
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let status = head.lines().next().unwrap_or("").to_string();
    let code = status.split_whitespace().nth(1).unwrap_or("");
    if code != "200" {
        bail!("{what} refused CONNECT: {status}");
    }
    Ok(())
}

async fn send_connect<S: AsyncWrite + Unpin>(s: &mut S, ep: &Endpoint) -> Result<()> {
    let req = format!(
        "CONNECT {h}:{p} HTTP/1.1\r\nHost: {h}:{p}\r\nProxy-Connection: Keep-Alive\r\nUser-Agent: whitestick\r\n\r\n",
        h = ep.host,
        p = ep.port
    );
    s.write_all(req.as_bytes()).await?;
    Ok(())
}

fn public_tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: std::sync::OnceLock<Arc<rustls::ClientConfig>> = std::sync::OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Arc::new(
                rustls::ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            )
        })
        .clone()
}

/// TLS config for talking to fwdproxy:8082: Meta's CA bundle, our certificate.
fn proxy_tls_config(cert_path: &str) -> Result<Arc<rustls::ClientConfig>> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let mut roots = rustls::RootCertStore::empty();
    let ca = std::fs::read(PROXY_CA).with_context(|| format!("read proxy CA {PROXY_CA}"))?;
    for c in CertificateDer::pem_slice_iter(&ca) {
        roots.add(c.context("parse proxy CA")?).ok();
    }
    let pem = std::fs::read(cert_path).with_context(|| format!("read proxy cert {cert_path}"))?;
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&pem)
        .collect::<std::result::Result<_, _>>()
        .with_context(|| format!("parse certificates in {cert_path}"))?;
    let key = PrivateKeyDer::from_pem_slice(&pem)
        .with_context(|| format!("parse private key in {cert_path}"))?;
    let cfg = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_client_auth_cert(certs, key)
        .context("proxy client certificate")?;
    Ok(Arc::new(cfg))
}

/// A stream that ends at the relay, TLS included when the relay is https.
pub async fn stream(ep: &Endpoint, proxy: Option<&Proxy>) -> Result<Stream> {
    let inner: Stream = match proxy {
        None => {
            let tcp = TcpStream::connect((ep.host.as_str(), ep.port))
                .await
                .with_context(|| format!("connect {}:{}", ep.host, ep.port))?;
            tcp.set_nodelay(true).ok();
            Box::new(tcp)
        }
        Some(Proxy::Http { host, port }) => {
            let mut tcp = TcpStream::connect((host.as_str(), *port))
                .await
                .with_context(|| format!("connect proxy {host}:{port}"))?;
            tcp.set_nodelay(true).ok();
            send_connect(&mut tcp, ep).await?;
            read_connect_response(&mut tcp, &format!("proxy {host}:{port}")).await?;
            Box::new(tcp)
        }
        Some(Proxy::Https { host, port, cert }) => {
            let tcp = TcpStream::connect((host.as_str(), *port))
                .await
                .with_context(|| format!("connect proxy {host}:{port}"))?;
            tcp.set_nodelay(true).ok();
            let name = rustls::pki_types::ServerName::try_from(host.clone())
                .with_context(|| format!("proxy server name {host}"))?;
            let mut tls = tokio_rustls::TlsConnector::from(proxy_tls_config(cert)?)
                .connect(name, tcp)
                .await
                .with_context(|| format!("tls handshake with proxy {host}:{port} (cert {cert})"))?;
            send_connect(&mut tls, ep).await?;
            read_connect_response(&mut tls, &format!("proxy {host}:{port}")).await?;
            Box::new(tls)
        }
    };
    if !ep.tls {
        return Ok(inner);
    }
    let name = rustls::pki_types::ServerName::try_from(ep.host.clone())
        .with_context(|| format!("tls server name {}", ep.host))?;
    let tls = tokio_rustls::TlsConnector::from(public_tls_config())
        .connect(name, inner)
        .await
        .with_context(|| format!("tls handshake with {}", ep.host))?;
    Ok(Box::new(tls))
}

/// Why a WebSocket handshake did not happen.
#[derive(Debug)]
pub enum WsError {
    /// The relay answered with an HTTP status instead of upgrading.
    Http { status: u16, body: String },
    Other(anyhow::Error),
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsError::Http { status, body } => write!(f, "relay answered HTTP {status}: {}", body.trim()),
            WsError::Other(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for WsError {}

impl From<anyhow::Error> for WsError {
    fn from(e: anyhow::Error) -> Self {
        WsError::Other(e)
    }
}

/// Open an authenticated WebSocket to `path` on the relay.
pub async fn websocket(
    ep: &Endpoint,
    proxy: Option<&Proxy>,
    token: &str,
    path: &str,
) -> std::result::Result<Ws, WsError> {
    let s = stream(ep, proxy).await?;
    let mut req = ep
        .ws_url(path)
        .into_client_request()
        .map_err(|e| anyhow!("build websocket request: {e}"))?;
    req.headers_mut().insert(
        "authorization",
        format!("Bearer {token}")
            .parse()
            .map_err(|e| anyhow!("token is not a valid header value: {e}"))?,
    );
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(16 << 20))
        .max_frame_size(Some(16 << 20))
        .max_write_buffer_size(64 << 20);
    match tokio_tungstenite::client_async_with_config(req, s, Some(cfg)).await {
        Ok((ws, _resp)) => Ok(ws),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            let status = resp.status().as_u16();
            let body = resp
                .body()
                .as_ref()
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_default();
            Err(WsError::Http { status, body })
        }
        Err(e) => Err(WsError::Other(anyhow!("websocket handshake: {e}"))),
    }
}

/// A plain HTTP GET to the relay (for `/v1/status/<box>`). Returns (status, body).
pub async fn http_get(
    ep: &Endpoint,
    proxy: Option<&Proxy>,
    token: &str,
    path: &str,
) -> Result<(u16, String)> {
    let mut s = stream(ep, proxy).await?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {token}\r\nUser-Agent: whitestick\r\nConnection: close\r\n\r\n",
        ep.host
    );
    s.write_all(req.as_bytes()).await?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.ok();
    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| anyhow!("malformed HTTP response from relay"))?;
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| anyhow!("malformed HTTP status line from relay"))?;
    // Chunked bodies: join the chunks. Cloudflare sends short JSON bodies with a
    // content-length, but a local wrangler does chunk them.
    let body = if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        dechunk(body)
    } else {
        body.to_string()
    };
    Ok((status, body))
}

fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    loop {
        let Some((size_line, after)) = rest.split_once("\r\n") else { break };
        let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
        if size == 0 || after.len() < size {
            break;
        }
        out.push_str(&after[..size]);
        rest = after[size..].trim_start_matches("\r\n");
    }
    out
}
