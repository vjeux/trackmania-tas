//! The chat-command bridge: the game gives a mode script no access to chat, so
//! this side reads `ManiaPlanet.PlayerChat` through XML-RPC and forwards
//! `/gravity 0.3`-style lines to the LowG mode as
//! `TriggerModeScriptEventArray("LowG.Cmd", [login, command, args...])`.
//!
//! One connection, one thread: requests are answered in-line and callbacks are
//! read between them (see `gbx::Client::call`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::gbx::{self, Callback, Client, Value};
use crate::logfile::Logger;

pub const CMD_EVENT: &str = "LowG.Cmd";
pub const STATUS_EVENT: &str = "LowG.Status";

/// The chat line -> mode command mapping. Returns None for chat that is not for us.
pub fn parse_chat_command(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let body = text.strip_prefix('/')?;
    let mut words = body.split_whitespace();
    let cmd = words.next()?.to_ascii_lowercase();
    let rest: Vec<String> = words.map(|w| w.to_string()).collect();
    let name = match cmd.as_str() {
        "gravity" | "g" | "grav" | "gravite" | "gravité" => "gravity",
        "bots" | "bot" | "balloons" | "ballons" => "bots",
        "collisions" | "collision" | "col" => "collisions",
        "status" | "lowg" | "low-g" | "lowgravity" => "status",
        _ => return None,
    };
    let mut out = vec![name.to_string()];
    // "/gravity 0,3" -- a French keyboard's decimal comma.
    out.extend(rest.into_iter().map(|w| if name == "gravity" { w.replace(',', ".") } else { w }));
    Some(out)
}

/// Run until `stop` is set. Reconnects after every failure.
pub fn run(port: u16, password: String, stop: Arc<AtomicBool>, log: Logger) {
    let mut announced_waiting = false;
    while !stop.load(Ordering::Relaxed) {
        match session(port, &password, &stop, &log) {
            Ok(()) => return,
            Err(e) => {
                let msg = e.to_string();
                // While the server boots, connection refused is the normal state; say it once.
                if msg.contains("Connection refused") || msg.contains("connection refused") {
                    if !announced_waiting {
                        log.note(&format!("bridge: waiting for the XML-RPC port {port}"));
                        announced_waiting = true;
                    }
                } else {
                    log.note(&format!("bridge: {msg}; reconnecting in 3 s"));
                    announced_waiting = false;
                }
            }
        }
        wait(&stop, Duration::from_secs(3));
    }
}

fn wait(stop: &AtomicBool, d: Duration) {
    let step = Duration::from_millis(200);
    let mut left = d;
    while !stop.load(Ordering::Relaxed) && !left.is_zero() {
        let s = step.min(left);
        thread::sleep(s);
        left -= s;
    }
}

fn session(port: u16, password: &str, stop: &AtomicBool, log: &Logger) -> gbx::Result<()> {
    let mut c = Client::connect(("127.0.0.1", port), Duration::from_secs(5))?;
    c.authenticate("SuperAdmin", password)?;
    // The newest API the server knows; older servers refuse unknown versions, which is harmless.
    if let Err(e) = c.call("SetApiVersion", &["2023-04-24".into()]) {
        log.note(&format!("bridge: SetApiVersion 2023-04-24 refused ({e}); staying on the default"));
    }
    c.call("EnableCallbacks", &[true.into()])?;
    let name = c.call("GetServerName", &[]).map(|v| v.to_plain()).unwrap_or_default();
    log.note(&format!("bridge: connected to {name:?} on port {port}, forwarding chat commands"));
    c.set_read_timeout(Some(Duration::from_secs(1)))?;
    for cb in c.take_callbacks() {
        handle(&mut c, cb, log)?;
    }
    while !stop.load(Ordering::Relaxed) {
        if let Some(cb) = c.next_callback()? {
            handle(&mut c, cb, log)?;
        }
    }
    Ok(())
}

fn handle(c: &mut Client, cb: Callback, log: &Logger) -> gbx::Result<()> {
    match cb.method.as_str() {
        "ManiaPlanet.PlayerChat" => {
            let login = cb.params.get(1).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let text = cb.params.get(2).and_then(|v| v.as_str()).unwrap_or("").to_string();
            if login.is_empty() || login == "0" {
                return Ok(()); // the server's own lines carry uid 0
            }
            let Some(cmd) = parse_chat_command(&text) else {
                return Ok(());
            };
            log.note(&format!("chat command from {login}: {text:?} -> {cmd:?}"));
            let mut params = vec![login];
            params.extend(cmd);
            c.call("TriggerModeScriptEventArray", &[CMD_EVENT.into(), Value::from(params)])?;
        }
        "ManiaPlanet.ModeScriptCallbackArray" => {
            let name = cb.params.first().and_then(|v| v.as_str()).unwrap_or("");
            if name == STATUS_EVENT {
                if let Some(lines) = cb.params.get(1).and_then(|v| v.as_array()) {
                    for l in lines {
                        log.note(&format!("lowg: {}", l.to_plain()));
                    }
                }
            }
        }
        "ManiaPlanet.PlayerConnect" => {
            let login = cb.params.first().map(|v| v.to_plain()).unwrap_or_default();
            let spectator = cb.params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
            log.note(&format!("player connected: {login}{}", if spectator { " (spectator)" } else { "" }));
        }
        "ManiaPlanet.PlayerDisconnect" => {
            let login = cb.params.first().map(|v| v.to_plain()).unwrap_or_default();
            let reason = cb.params.get(1).map(|v| v.to_plain()).unwrap_or_default();
            log.note(&format!("player disconnected: {login} ({reason})"));
        }
        "ManiaPlanet.BeginMap" => {
            let map = cb.params.first().map(|v| format!("{} ({})", v.field_str("Name"), v.field_str("UId"))).unwrap_or_default();
            log.note(&format!("map: {map}"));
        }
        "ManiaPlanet.ServerStart" | "ManiaPlanet.ServerStop" => {
            log.note(&format!("server event: {}", cb.method));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_chat_lines() {
        assert_eq!(parse_chat_command("/gravity 0.3"), Some(vec!["gravity".into(), "0.3".into()]));
        assert_eq!(parse_chat_command("/g 0,3"), Some(vec!["gravity".into(), "0.3".into()]));
        assert_eq!(parse_chat_command("  /Gravity all 0.5 "), Some(vec!["gravity".into(), "all".into(), "0.5".into()]));
        assert_eq!(parse_chat_command("/bots 3"), Some(vec!["bots".into(), "3".into()]));
        assert_eq!(parse_chat_command("/collisions off"), Some(vec!["collisions".into(), "off".into()]));
        assert_eq!(parse_chat_command("/lowg"), Some(vec!["status".into()]));
        assert_eq!(parse_chat_command("hello /gravity"), None);
        assert_eq!(parse_chat_command("/help"), None);
        assert_eq!(parse_chat_command("/"), None);
    }
}
