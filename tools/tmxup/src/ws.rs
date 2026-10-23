//! A WebSocket client just big enough for Chrome's DevTools endpoint on the loopback:
//! RFC 6455 handshake, masked client frames, text/binary/continuation/ping/close from the
//! server. No TLS (DevTools is plain HTTP on 127.0.0.1), no extensions, no deps.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct WebSocket {
    s: TcpStream,
    buf: Vec<u8>,
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for ch in data.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if ch.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if ch.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A weak but sufficient nonce: the key only has to be unguessable-ish to the server
/// for cache-busting, and Chrome does not check it beyond echoing the accept hash.
fn nonce() -> [u8; 16] {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let p = std::process::id() as u128;
    let mut x = t ^ (p << 64) ^ 0x9e37_79b9_7f4a_7c15_f39c_c060_5ced_c834u128;
    let mut out = [0u8; 16];
    for b in out.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x >> 24) as u8;
    }
    out
}

impl WebSocket {
    /// `ws://127.0.0.1:9222/devtools/page/<id>`
    pub fn connect(url: &str, timeout: Duration) -> Result<WebSocket, String> {
        let rest = url.strip_prefix("ws://").ok_or_else(|| format!("{url}: not a ws:// url"))?;
        let (hostport, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let addr: std::net::SocketAddr = hostport.parse().map_err(|e| format!("{hostport}: {e}"))?;
        let s = TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("connect {hostport}: {e}"))?;
        s.set_read_timeout(Some(timeout)).ok();
        s.set_write_timeout(Some(timeout)).ok();
        s.set_nodelay(true).ok();
        let mut ws = WebSocket { s, buf: Vec::new() };
        let key = b64(&nonce());
        let req = format!("GET {path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n");
        ws.s.write_all(req.as_bytes()).map_err(|e| format!("handshake write: {e}"))?;
        // read the response head
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = ws.s.read(&mut byte).map_err(|e| format!("handshake read: {e}"))?;
            if n == 0 {
                return Err("handshake: connection closed".into());
            }
            head.push(byte[0]);
            if head.ends_with(b"\r\n\r\n") {
                break;
            }
            if head.len() > 65536 {
                return Err("handshake: response head too long".into());
            }
        }
        let text = String::from_utf8_lossy(&head);
        if !text.starts_with("HTTP/1.1 101") {
            return Err(format!("handshake refused: {}", text.lines().next().unwrap_or("")));
        }
        Ok(ws)
    }

    #[allow(dead_code)]
    pub fn set_timeout(&mut self, t: Duration) {
        self.s.set_read_timeout(Some(t)).ok();
    }

    fn fill(&mut self, need: usize) -> Result<(), String> {
        let mut tmp = [0u8; 65536];
        while self.buf.len() < need {
            let n = self.s.read(&mut tmp).map_err(|e| format!("ws read: {e}"))?;
            if n == 0 {
                return Err("ws: connection closed".into());
            }
            self.buf.extend_from_slice(&tmp[..n]);
        }
        Ok(())
    }

    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<(), String> {
        let mut f = vec![0x80 | opcode];
        let n = payload.len();
        if n < 126 {
            f.push(0x80 | n as u8);
        } else if n < 65536 {
            f.push(0x80 | 126);
            f.extend_from_slice(&(n as u16).to_be_bytes());
        } else {
            f.push(0x80 | 127);
            f.extend_from_slice(&(n as u64).to_be_bytes());
        }
        let mask = nonce();
        f.extend_from_slice(&mask[..4]);
        f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.s.write_all(&f).map_err(|e| format!("ws write: {e}"))
    }

    pub fn send_text(&mut self, text: &str) -> Result<(), String> {
        self.send_frame(1, text.as_bytes())
    }

    /// The next complete text message (binary comes back as lossy text); pings are
    /// answered, a close frame is an error.
    pub fn recv_text(&mut self) -> Result<String, String> {
        let mut message: Vec<u8> = Vec::new();
        loop {
            self.fill(2)?;
            let b0 = self.buf[0];
            let b1 = self.buf[1];
            let fin = b0 & 0x80 != 0;
            let opcode = b0 & 0x0f;
            let masked = b1 & 0x80 != 0;
            let mut len = (b1 & 0x7f) as usize;
            let mut hdr = 2;
            if len == 126 {
                self.fill(4)?;
                len = u16::from_be_bytes([self.buf[2], self.buf[3]]) as usize;
                hdr = 4;
            } else if len == 127 {
                self.fill(10)?;
                let mut a = [0u8; 8];
                a.copy_from_slice(&self.buf[2..10]);
                len = u64::from_be_bytes(a) as usize;
                hdr = 10;
            }
            if masked {
                hdr += 4;
            }
            self.fill(hdr + len)?;
            let mut payload: Vec<u8> = self.buf[hdr..hdr + len].to_vec();
            if masked {
                let mask = [self.buf[hdr - 4], self.buf[hdr - 3], self.buf[hdr - 2], self.buf[hdr - 1]];
                for (i, b) in payload.iter_mut().enumerate() {
                    *b ^= mask[i % 4];
                }
            }
            self.buf.drain(..hdr + len);
            match opcode {
                0x8 => return Err("ws: server closed the connection".into()),
                0x9 => {
                    self.send_frame(0xA, &payload)?;
                    continue;
                }
                0xA => continue,
                0x0 | 0x1 | 0x2 => {
                    message.extend_from_slice(&payload);
                    if fin {
                        return Ok(String::from_utf8_lossy(&message).into_owned());
                    }
                }
                other => return Err(format!("ws: unexpected opcode {other}")),
            }
        }
    }
}
