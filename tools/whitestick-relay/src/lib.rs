//! whitestick-relay — the rendezvous point between `whitestick` clients (Meta
//! devservers, reaching out through fwdproxy) and the `whitestick agent` running
//! on a box that is not on any network Meta can reach (the WhiteStick render
//! box, in WSL). Both sides connect OUTBOUND to this Worker over HTTPS; nobody
//! has to accept an inbound connection anywhere.
//!
//! One Durable Object per box name. It holds the agent's WebSocket and
//! multiplexes any number of controller ("ctl") WebSockets over it:
//!
//! ```text
//!   devserver  --wss /v1/ctl/<box>-->  [Worker -> DO "box:<box>"]  <--wss /v1/agent/<box>--  box
//! ```
//!
//! The DO is deliberately dumb. Every message between it and the agent is
//! `[u32 BE channel][u8 kind][payload]`:
//!
//! | kind | direction   | meaning                                          |
//! |------|-------------|--------------------------------------------------|
//! | 0    | DO -> agent | OPEN: a controller attached on this channel      |
//! | 1    | both        | DATA: opaque payload for/from that controller     |
//! | 2    | both        | CLOSE: the controller left / the agent hung up    |
//!
//! Controllers see none of this: what a controller sends arrives at the agent
//! as DATA on its channel, and DATA from the agent arrives at the controller as
//! a bare message. What the payload means (commands, stdin, stdout, exit codes)
//! is between `whitestick` and `whitestick agent` — see `tools/whitestick`.
//!
//! Auth is one pre-shared token in the `Authorization: Bearer` header, checked
//! at the edge before anything reaches a DO. The token lives in the Worker
//! secret `PSK`.
//!
//! The WebSockets use the hibernation API, so an idle agent connection costs
//! nothing while nobody is sending commands. Application-level `ping` text
//! frames are auto-answered with `pong` by the runtime without waking the DO.

use serde::{Deserialize, Serialize};
use worker::*;

const HDR: usize = 5;
const K_OPEN: u8 = 0;
const K_DATA: u8 = 1;
const K_CLOSE: u8 = 2;

/// Close codes the two ends understand.
const CLOSE_AGENT_REPLACED: u16 = 4000;
const CLOSE_AGENT_OFFLINE: u16 = 4002;

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(tag = "role")]
enum Attach {
    #[serde(rename = "agent")]
    Agent { since: f64 },
    #[serde(rename = "ctl")]
    Ctl { chan: u32, since: f64 },
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn frame(chan: u32, kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(HDR + payload.len());
    f.extend_from_slice(&chan.to_be_bytes());
    f.push(kind);
    f.extend_from_slice(payload);
    f
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

fn valid_box_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'.')
}

/// Edge entry point: authenticate, validate the path, hand the request to the
/// box's Durable Object.
#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    if path == "/" || path == "/healthz" {
        return Response::ok("whitestick-relay\n");
    }

    let psk = env.secret("PSK")?.to_string();
    let auth = req.headers().get("authorization")?.unwrap_or_default();
    let presented = auth.strip_prefix("Bearer ").unwrap_or("");
    if psk.is_empty() || !constant_time_eq(presented.as_bytes(), psk.as_bytes()) {
        return Response::error("unauthorized", 401);
    }

    // /v1/<role>/<box>
    let mut it = path.trim_start_matches('/').split('/');
    let (v, role, name) = (it.next(), it.next(), it.next());
    if v != Some("v1") || it.next().is_some() {
        return Response::error("not found", 404);
    }
    let name = match name {
        Some(n) if valid_box_name(n) => n,
        _ => return Response::error("bad box name", 400),
    };
    match role {
        Some("agent") | Some("ctl") | Some("status") => {}
        _ => return Response::error("not found", 404),
    }

    let stub = env
        .durable_object("RELAY")?
        .id_from_name(&format!("box:{name}"))?
        .get_stub()?;
    stub.fetch_with_request(req).await
}

#[durable_object(websocket)]
pub struct Relay {
    state: State,
    #[allow(dead_code)]
    env: Env,
}

impl Relay {
    fn agents(&self) -> Vec<WebSocket> {
        self.state.get_websockets_with_tag("agent")
    }

    /// The agent that connected last. Older ones are being closed (or are
    /// half-dead); anything sent to them would fail or go nowhere.
    fn live_agent(&self) -> Option<WebSocket> {
        let mut best: Option<(f64, WebSocket)> = None;
        for a in self.agents() {
            let since = match a.deserialize_attachment::<Attach>() {
                Ok(Some(Attach::Agent { since })) => since,
                _ => 0.0,
            };
            if best.as_ref().map(|(s, _)| since >= *s).unwrap_or(true) {
                best = Some((since, a));
            }
        }
        best.map(|(_, a)| a)
    }

    fn ctls(&self) -> Vec<WebSocket> {
        self.state.get_websockets_with_tag("ctl")
    }

    fn ctl_on(&self, chan: u32) -> Vec<WebSocket> {
        self.state.get_websockets_with_tag(&format!("c:{chan}"))
    }

    fn alloc_chan(&self) -> u32 {
        loop {
            let c = (js_sys::Math::random() * (u32::MAX as f64)) as u32;
            if c != 0 && self.ctl_on(c).is_empty() {
                return c;
            }
        }
    }

    fn require_upgrade(req: &Request) -> Result<bool> {
        Ok(req
            .headers()
            .get("upgrade")?
            .map(|u| u.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false))
    }

    fn status(&self) -> Result<Response> {
        let agents = self.agents();
        let since = self.live_agent().and_then(|a| {
            a.deserialize_attachment::<Attach>()
                .ok()
                .flatten()
                .and_then(|att| match att {
                    Attach::Agent { since } => Some(since),
                    _ => None,
                })
        });
        let body = serde_json::json!({
            "agent": !agents.is_empty(),
            "agent_since_ms": since,
            "sessions": self.ctls().len(),
            "now_ms": now_ms(),
        });
        Response::from_json(&body)
    }

    fn attach_agent(&self) -> Result<Response> {
        // A new agent connection wins. Whatever was attached to the old one
        // is gone (the box rebooted, or the old socket is half-dead).
        for old in self.agents() {
            let _ = old.close(Some(CLOSE_AGENT_REPLACED), Some("replaced by a newer agent"));
        }
        for c in self.ctls() {
            let _ = c.close(Some(CLOSE_AGENT_OFFLINE), Some("agent replaced"));
        }
        let pair = WebSocketPair::new()?;
        pair.server
            .serialize_attachment(Attach::Agent { since: now_ms() })?;
        self.state.accept_websocket_with_tags(&pair.server, &["agent"]);
        Response::from_websocket(pair.client)
    }

    fn attach_ctl(&self) -> Result<Response> {
        let Some(agent) = self.live_agent() else {
            return Response::error("agent offline", 503);
        };
        let chan = self.alloc_chan();
        // Tell the agent first: if that fails the agent is gone and the
        // controller must hear 503, not get a socket nobody serves.
        if agent.send_with_bytes(frame(chan, K_OPEN, &[])).is_err() {
            return Response::error("agent offline", 503);
        }
        let pair = WebSocketPair::new()?;
        pair.server.serialize_attachment(Attach::Ctl {
            chan,
            since: now_ms(),
        })?;
        let tag = format!("c:{chan}");
        self.state
            .accept_websocket_with_tags(&pair.server, &["ctl", &tag]);
        Response::from_websocket(pair.client)
    }

    /// A socket went away (cleanly or not).
    fn gone(&self, ws: &WebSocket) {
        match ws.deserialize_attachment::<Attach>() {
            Ok(Some(Attach::Agent { .. })) => {
                // Only orphan the controllers if no agent is left: a replaced
                // agent's close event must not kill the new agent's sessions.
                if self.agents().is_empty() {
                    for c in self.ctls() {
                        let _ = c.close(Some(CLOSE_AGENT_OFFLINE), Some("agent offline"));
                    }
                }
            }
            Ok(Some(Attach::Ctl { chan, .. })) => {
                if let Some(a) = self.live_agent() {
                    let _ = a.send_with_bytes(frame(chan, K_CLOSE, &[]));
                }
            }
            _ => {}
        }
    }
}

impl DurableObject for Relay {
    fn new(state: State, env: Env) -> Self {
        if let Ok(pair) = worker_sys::WebSocketRequestResponsePair::new("ping", "pong") {
            state.set_websocket_auto_response(&pair);
        }
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();
        let role = path.trim_start_matches('/').split('/').nth(1).unwrap_or("");
        match role {
            "status" => self.status(),
            "agent" | "ctl" => {
                if !Self::require_upgrade(&req)? {
                    return Response::error("expected a websocket upgrade", 426);
                }
                if role == "agent" {
                    self.attach_agent()
                } else {
                    self.attach_ctl()
                }
            }
            _ => Response::error("not found", 404),
        }
    }

    async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
        let att = match ws.deserialize_attachment::<Attach>()? {
            Some(a) => a,
            None => return Ok(()),
        };
        match att {
            Attach::Agent { .. } => {
                let b = match message {
                    WebSocketIncomingMessage::Binary(b) => b,
                    WebSocketIncomingMessage::String(_) => return Ok(()),
                };
                if b.len() < HDR {
                    return Ok(());
                }
                let chan = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                match b[4] {
                    K_DATA => {
                        for c in self.ctl_on(chan) {
                            let _ = c.send_with_bytes(&b[HDR..]);
                        }
                    }
                    K_CLOSE => {
                        for c in self.ctl_on(chan) {
                            let _ = c.close(Some(1000), Some("closed by agent"));
                        }
                    }
                    _ => {}
                }
            }
            Attach::Ctl { chan, .. } => {
                let payload = match message {
                    WebSocketIncomingMessage::Binary(b) => b,
                    // Text from a controller is not part of the protocol
                    // (pings are auto-answered before we see them).
                    WebSocketIncomingMessage::String(_) => return Ok(()),
                };
                let Some(agent) = self.live_agent() else {
                    let _ = ws.close(Some(CLOSE_AGENT_OFFLINE), Some("agent offline"));
                    return Ok(());
                };
                if agent.send_with_bytes(frame(chan, K_DATA, &payload)).is_err() {
                    let _ = ws.close(Some(CLOSE_AGENT_OFFLINE), Some("agent offline"));
                }
            }
        }
        Ok(())
    }

    async fn websocket_close(&self, ws: WebSocket, _code: usize, _reason: String, _was_clean: bool) -> Result<()> {
        self.gone(&ws);
        Ok(())
    }

    async fn websocket_error(&self, ws: WebSocket, _error: Error) -> Result<()> {
        self.gone(&ws);
        Ok(())
    }
}
