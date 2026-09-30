//! `tinyctl token --from-game [--pid N] [--token-dir DIR]`
//!
//! The Nadeo web-service tokens WITHOUT touching the game: the running client holds both
//! `Authorization` header values (`nadeo_v1 t=<JWT>`) in its own memory, so a READ-ONLY
//! scan of the process recovers them — no render lock, no launch, no restart, no plugin
//! installed in a game someone is playing (vjeux's order, 2026-09-27 20:27: "mint the token
//! on a live version of Trackmania without requesting the lock or restarting the game").
//!
//! The read is done by the windowcap lane's bounded scanner on the box
//! (`C:/Users/vjeux/mk64qa/re/tm-memscan.exe`, PROCESS_VM_READ|QUERY only, never writes):
//!   `tm-memscan find PID "nadeo_v1 t=eyJ"` → candidate addresses
//!   `tm-memscan dump PID ADDR LEN`         → the bytes, hex
//! Each candidate is decoded as a JWT and kept when its `aud` names an audience we want and
//! its `exp` is still ahead; the pair is verified against the live APIs before it is used.
//!
//! Three checks keep a STALE hit (a valid token left in a freed buffer — the windowcap
//! lane's warning) from being taken for the current one: the NEWEST `iat` of each audience
//! wins, both must carry the same `sid` (one login session), and the pair is proved against
//! the live APIs. Owner confirmation through the holding object (`tm-memscan refs`/`objhead`
//! → CGameUserService / CTrackManiaNetwork field, then a stable chain instead of a scan) is
//! the next step; it needs a running game to derive and is tracked in state-notes.
//!
//! Verified 2026-09-27 20:33 on vjeux's own running game: 5 candidates, both audiences,
//! HTTP 200 from core and live.

use std::path::Path;

use crate::wsx::Wsx;

const SCANNER: &str = "/mnt/c/Users/vjeux/mk64qa/re/tm-memscan.exe";
/// The exe this scanner and its offsets were built for; a different build is refused.
const EXE_SHA_PREFIX: &str = "3fc7d8cda542beda";

fn b64url(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// (audience, expiry) of a JWT's payload, read without a JSON dependency.
fn claims(jwt: &str) -> Option<(String, u64)> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = b64url(payload)?;
    let s = String::from_utf8_lossy(&bytes);
    let field = |k: &str| -> Option<String> {
        let i = s.find(&format!("\"{k}\""))?;
        let rest = &s[i + k.len() + 2..];
        let rest = rest.trim_start_matches([':', ' ']);
        if let Some(r) = rest.strip_prefix('"') {
            Some(r[..r.find('"')?].to_string())
        } else {
            let end = rest.find([',', '}'])?;
            Some(rest[..end].trim().to_string())
        }
    };
    Some((field("aud")?, field("exp")?.parse().ok()?))
}

/// `iat` (issued at) and `sid` (login-session id) of a JWT: the staleness check. A freed
/// buffer can still hold a VALID token from an earlier login — the windowcap lane's warning,
/// 2026-09-27 — so the newest `iat` wins and the two audiences must share one `sid`: tokens
/// of the current session are issued together under the same login.
fn iat_sid(jwt: &str) -> Option<(u64, String)> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = b64url(payload)?;
    let s = String::from_utf8_lossy(&bytes);
    let field = |k: &str| -> Option<String> {
        let i = s.find(&format!("\"{k}\""))?;
        let rest = &s[i + k.len() + 2..];
        let rest = rest.trim_start_matches([':', ' ']);
        if let Some(r) = rest.strip_prefix('"') {
            Some(r[..r.find('"')?].to_string())
        } else {
            let end = rest.find([',', '}'])?;
            Some(rest[..end].trim().to_string())
        }
    };
    Some((field("iat")?.parse().ok()?, field("sid")?))
}

/// The game's pid on the box, or None when no game runs.
pub fn game_pid(wsx: &Wsx) -> Option<u32> {
    let out = wsx
        .sh("/mnt/c/Windows/System32/tasklist.exe /FO CSV /NH 2>/dev/null | grep -i trackmania | head -1 | cut -d, -f2 | tr -d '\"'")
        .ok()?;
    out.trim().parse().ok()
}

/// Read both tokens out of the RUNNING game. Returns (core, live) as full header values.
pub fn tokens_from_game(wsx: &Wsx, pid: u32) -> Result<(String, String), String> {
    // the scanner is built against one exe; refuse a different build rather than read junk
    let sha = wsx
        .sh("sha256sum \"/mnt/c/Program Files (x86)/Ubisoft/Ubisoft Game Launcher/games/Trackmania/Trackmania.exe\" 2>/dev/null | cut -c1-16; true")
        .unwrap_or_default();
    let sha = sha.trim();
    if !sha.is_empty() && sha != EXE_SHA_PREFIX {
        return Err(format!("token --from-game: the game exe is {sha}, the scanner's offsets are for {EXE_SHA_PREFIX} — refusing"));
    }
    let hits = wsx.sh(&format!("cd /mnt/c/Users/vjeux/mk64qa/re && {SCANNER} find {pid} \"nadeo_v1 t=eyJ\" 2>&1 | grep '^hit' | awk '{{print $3}}' | head -24; true"))?;
    let addrs: Vec<String> = hits.lines().map(|l| l.trim().to_string()).filter(|l| l.starts_with("0x")).collect();
    if addrs.is_empty() {
        return Err("token --from-game: no token strings in that process (is it the game, and logged in?)".into());
    }
    // (iat, sid, header) per audience: newest issue wins, and the pair must share one sid
    let (mut core, mut live) = (None::<(u64, String, String)>, None::<(u64, String, String)>);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    for a in &addrs {
        let hex = wsx
            .sh(&format!("cd /mnt/c/Users/vjeux/mk64qa/re && {SCANNER} dump {pid} {a} 900 2>/dev/null | awk -F'\\t' '{{print $4}}' | tr -d '\\n'; true"))
            .unwrap_or_default();
        let hex = hex.trim();
        if hex.is_empty() || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()).collect();
        let text = String::from_utf8_lossy(&bytes);
        let text = text.split('\0').next().unwrap_or("");
        let Some(start) = text.find("eyJ") else { continue };
        let jwt: String = text[start..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-').collect();
        if jwt.split('.').count() != 3 {
            continue;
        }
        let Some((aud, exp)) = claims(&jwt) else { continue };
        if exp <= now + 60 {
            continue;
        }
        let header = format!("nadeo_v1 t={jwt}");
        let Some((iat, sid)) = iat_sid(&jwt) else { continue };
        let slot = match aud.as_str() {
            "NadeoServices" => &mut core,
            "NadeoLiveServices" => &mut live,
            _ => continue,
        };
        // the NEWEST issued candidate of each audience (a stale copy in a freed buffer can
        // still be unexpired, so `exp` alone does not decide)
        if slot.as_ref().map(|(i, _, _)| iat > *i).unwrap_or(true) {
            *slot = Some((iat, sid, header));
        }
    }
    match (core, live) {
        (Some((ci, csid, c)), Some((li, lsid, l))) => {
            if csid != lsid {
                return Err(format!(
                    "token --from-game: the two tokens come from different login sessions ({csid} vs {lsid}) — one is stale; retry, or mint with --standalone"
                ));
            }
            let _ = (ci, li);
            println!("  token --from-game: read from pid {pid}, one login session ({csid}), newest issue of each audience");
            let _ = now;
            Ok((c, l))
        }
        (c, l) => Err(format!(
            "token --from-game: incomplete ({} core, {} live) — is the game logged in?",
            c.map(|_| 1).unwrap_or(0),
            l.map(|_| 1).unwrap_or(0)
        )),
    }
}

/// Write the pair where every publishing command looks for it.
pub fn write_tokens(dir: &Path, core: &str, live: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("token-NadeoServices.txt"), core).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("token-NadeoLiveServices.txt"), live).map_err(|e| e.to_string())?;
    Ok(())
}

/// A read-only probe of both APIs: the tokens are only good if the servers accept them.
pub fn verify(core: &str, live: &str) -> Result<(), String> {
    let probe = |auth: &str, url: &str| -> Result<u32, String> {
        let out = std::process::Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "-H", &format!("Authorization: {auth}"), url])
            .output()
            .map_err(|e| e.to_string())?;
        String::from_utf8_lossy(&out.stdout).trim().parse().map_err(|_| "curl: no status".to_string())
    };
    let c = probe(core, "https://prod.trackmania.core.nadeo.online/maps/?mapUidList=EKiVTd52zbo7MHkU0HPT0qLXhan")?;
    let l = probe(live, "https://live-services.trackmania.nadeo.live/api/token/club/43788/campaign/157851")?;
    if c == 200 && l == 200 {
        println!("  token --from-game: verified against both APIs (core {c}, live {l})");
        Ok(())
    } else {
        Err(format!("token --from-game: the APIs refused the tokens (core {c}, live {l})"))
    }
}
