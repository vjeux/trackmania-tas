//! Reaching the plugin inside the running game — address resolution and the
//! token every command must carry.
//!
//! # The WSL trap (this cost an afternoon in disguise)
//!
//! The game and its plugin run on Windows, so the plugin's HTTP server is on
//! the WINDOWS loopback. A Windows binary like `curl.exe` reaches it at
//! 127.0.0.1. A Linux binary in WSL has its OWN loopback, where 127.0.0.1 is a
//! different machine: connection refused, while a shell command one line
//! earlier reported the plugin healthy.
//!
//! So the address is probed, in order, and only a WORKING one is remembered.
//! Never cache a guess: an earlier version fell back to 127.0.0.1 when nothing
//! answered and cached that, so a driver probing while the game was still
//! starting locked itself to the wrong machine and dialled it for three
//! minutes while the plugin answered elsewhere.
//!
//! # The token
//!
//! Every command carries the holder's token. The plugin compares it with the
//! one the lock published for the live instance and refuses anything else.
//! That is what makes the lock non-cooperative for in-game commands: a driver
//! without it is turned away by the game, not trusted to check first.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::OnceLock;
use std::time::Duration;

/// The plugin's HTTP port.
pub const PORT: u16 = 29800;

static ADDR: OnceLock<String> = OnceLock::new();

/// Candidate addresses, best first.
pub fn addrs() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(h) = std::env::var("SHOOT_HOST") {
        if !h.trim().is_empty() {
            out.push(format!("{}:29800", h.trim()));
        }
    }
    // Native Windows, or WSL with mirrored networking.
    out.push("127.0.0.1:29800".to_string());
    // WSL2's default NAT: the host is the resolv.conf nameserver.
    if let Ok(rc) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in rc.lines() {
            if let Some(ip) = line.strip_prefix("nameserver ") {
                out.push(format!("{}:29800", ip.trim()));
            }
        }
    }
    // The default gateway, which is the host under some WSL configs.
    if let Ok(rt) = std::fs::read_to_string("/proc/net/route") {
        for line in rt.lines().skip(1) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() > 2 && f[1] == "00000000" {
                if let Ok(v) = u32::from_str_radix(f[2], 16) {
                    // Stored little-endian, so the bytes are already in network
                    // order: 0x010012AC -> 172.18.0.1. Reversing them yielded
                    // 1.0.18.172 and a refusal that looked like "plugin down".
                    let b = v.to_le_bytes();
                    out.push(format!("{}.{}.{}.{}:29800", b[0], b[1], b[2], b[3]));
                }
            }
        }
    }
    out.dedup();
    out
}

/// The first candidate that accepts a connection, remembered only on success.
pub fn addr() -> Option<String> {
    if let Some(a) = ADDR.get() {
        return Some(a.clone());
    }
    for a in addrs() {
        let Ok(sa) = a.parse::<SocketAddr>() else { continue };
        if TcpStream::connect_timeout(&sa, Duration::from_millis(400)).is_ok() {
            let _ = ADDR.set(a.clone());
            return Some(a);
        }
    }
    None
}

/// The working address, or the conventional one when nothing answers.
///
/// Callers that need a string for a message or a connect attempt use this
/// instead of repeating the literal — the port is game knowledge and lives
/// here with the rest of it.
pub fn addr_or_default() -> String {
    addr().unwrap_or_else(|| format!("127.0.0.1:{PORT}"))
}

/// Is the plugin answering anywhere?
pub fn alive() -> bool {
    addr().is_some()
}

/// The token for the lock this process holds.
///
/// Either set by [`crate::GameLock`] in-process, or inherited from
/// `tmdrive run` through `TM_LOCK_TOKEN` when a child process does the work.
/// Absent means this driver never took the lock — the plugin will refuse it,
/// which is the intended outcome.
pub fn current_token() -> Option<String> {
    if let Some(t) = crate::held_token() {
        return Some(t);
    }
    std::env::var("TM_LOCK_TOKEN").ok().filter(|s| !s.trim().is_empty())
}

/// One HTTP GET to the plugin, with the token appended.
pub fn get(route: &str, timeout_s: u64) -> Result<String, String> {
    let a = addr().ok_or_else(|| "the plugin is not answering on any candidate address".to_string())?;
    let token = current_token().ok_or_else(|| {
        "no game lock: take one with tmdrive::acquire (or run under `tmdrive run`) — \
         the plugin refuses untokened commands"
            .to_string()
    })?;
    let sep = if route.contains('?') { '&' } else { '?' };
    let route = format!("{route}{sep}token={token}");

    let mut s = TcpStream::connect(a.as_str()).map_err(|e| format!("connect: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(timeout_s))).ok();
    s.set_write_timeout(Some(Duration::from_secs(timeout_s))).ok();
    let req = format!("GET {route} HTTP/1.1\r\nHost: tm\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut buf = String::new();
    s.read_to_string(&mut buf).map_err(|e| format!("read: {e}"))?;
    let body = buf.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or(&buf).to_string();
    if body.contains("token-refused") {
        return Err(format!(
            "the plugin refused our token on {route}: another session holds the live game"
        ));
    }
    Ok(body)
}
