//! Wire protocol between `whitestick` (the client on a devserver) and
//! `whitestick agent` (on the box). The relay in between never looks inside.
//!
//! Every message on a channel is one binary WebSocket frame: `[tag u8][body]`.
//!
//! client -> agent
//! | tag  | body                         |
//! |------|------------------------------|
//! | 0x01 | REQ: JSON [`Req`]            |
//! | 0x02 | STDIN: bytes for the command |
//! | 0x03 | STDIN_EOF                    |
//! | 0x04 | SIGNAL: one byte, the signal |
//! | 0x05 | ACK: u32 BE bytes consumed   |
//!
//! agent -> client
//! | tag  | body                                   |
//! |------|----------------------------------------|
//! | 0x11 | STDOUT: bytes                          |
//! | 0x12 | STDERR: bytes                          |
//! | 0x13 | EXIT: JSON [`Exit`] -- always the last |
//! | 0x14 | ERROR: utf-8 text (spawn failed, ...)  |
//! | 0x15 | ACK: u32 BE stdin bytes consumed       |
//!
//! Flow control is a byte window per direction: a sender may have at most
//! [`WINDOW`] bytes of STDIN/STDOUT/STDERR unacknowledged. The receiver ACKs
//! bytes once it has written them to the local fd, so a slow reader on either
//! end stalls the producer instead of piling megabytes up inside the relay.
//! Chunks stay well under the relay's 1 MiB per-message limit.
//!
//! Between the relay and the agent, every frame is wrapped as
//! `[u32 BE channel][u8 kind][frame]` (see `whitestick-relay`); the client
//! never sees that layer.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CHUNK: usize = 64 * 1024;
pub const WINDOW: u64 = 4 * 1024 * 1024;

pub const C_REQ: u8 = 0x01;
pub const C_STDIN: u8 = 0x02;
pub const C_STDIN_EOF: u8 = 0x03;
pub const C_SIGNAL: u8 = 0x04;
pub const C_ACK: u8 = 0x05;

pub const A_STDOUT: u8 = 0x11;
pub const A_STDERR: u8 = 0x12;
pub const A_EXIT: u8 = 0x13;
pub const A_ERROR: u8 = 0x14;
pub const A_ACK: u8 = 0x15;

/// Relay <-> agent envelope kinds.
pub const K_OPEN: u8 = 0;
pub const K_DATA: u8 = 1;
pub const K_CLOSE: u8 = 2;
pub const ENVELOPE: usize = 5;

/// Relay close codes the client turns into messages.
pub const CLOSE_AGENT_REPLACED: u16 = 4000;
pub const CLOSE_AGENT_OFFLINE: u16 = 4002;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Req {
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Whether STDIN frames will follow. When false the command gets /dev/null.
    #[serde(default)]
    pub stdin: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct Exit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

impl Exit {
    /// The shell convention: a signal death is 128 + the signal number.
    pub fn status(&self) -> i32 {
        match (self.code, self.signal) {
            (Some(c), _) => c,
            (None, Some(s)) => 128 + s,
            (None, None) => 1,
        }
    }
}

pub fn frame(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(1 + body.len());
    f.push(tag);
    f.extend_from_slice(body);
    f
}

pub fn ack(tag: u8, n: usize) -> Vec<u8> {
    frame(tag, &(n as u32).to_be_bytes())
}

pub fn read_ack(body: &[u8]) -> Option<u64> {
    if body.len() < 4 {
        return None;
    }
    Some(u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as u64)
}

pub fn envelope(chan: u32, kind: u8, body: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(ENVELOPE + body.len());
    f.extend_from_slice(&chan.to_be_bytes());
    f.push(kind);
    f.extend_from_slice(body);
    f
}

pub fn open_envelope(b: &[u8]) -> Option<(u32, u8, &[u8])> {
    if b.len() < ENVELOPE {
        return None;
    }
    Some((u32::from_be_bytes([b[0], b[1], b[2], b[3]]), b[4], &b[ENVELOPE..]))
}

/// A byte window shared between a producer task and the ACK reader.
pub struct Window {
    max: u64,
    used: std::sync::atomic::AtomicU64,
    notify: tokio::sync::Notify,
}

impl Window {
    pub fn new(max: u64) -> Self {
        Self {
            max,
            used: std::sync::atomic::AtomicU64::new(0),
            notify: tokio::sync::Notify::new(),
        }
    }

    /// Reserve `n` bytes, waiting for ACKs if the window is full.
    pub async fn acquire(&self, n: u64) {
        use std::sync::atomic::Ordering::SeqCst;
        loop {
            let notified = self.notify.notified();
            let used = self.used.load(SeqCst);
            if used + n <= self.max
                && self
                    .used
                    .compare_exchange(used, used + n, SeqCst, SeqCst)
                    .is_ok()
            {
                return;
            }
            notified.await;
        }
    }

    pub fn release(&self, n: u64) {
        use std::sync::atomic::Ordering::SeqCst;
        let mut cur = self.used.load(SeqCst);
        loop {
            let next = cur.saturating_sub(n);
            match self.used.compare_exchange(cur, next, SeqCst, SeqCst) {
                Ok(_) => break,
                Err(actual) => cur = actual,
            }
        }
        self.notify.notify_waiters();
    }
}
