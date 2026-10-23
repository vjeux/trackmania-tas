//! `dmdel CHANNEL MESSAGE_ID…` — delete our own Discord messages (the duplicates a retried
//! relay command posted, 2026-10-23: `wsx` re-ran a three-send command when its last send
//! hung, so two messages went out three times each). The account token is the one
//! `discordctl` uses (`~/.discordctl/token`); only the account's own messages can go.
//! Each deletion is confirmed by re-reading the channel: a message still listed after
//! the DELETE is reported as NOT deleted and the exit code is 1.
use std::process::Command;

fn token() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let p = format!("{home}/.discordctl/token");
    std::fs::read_to_string(&p).map(|s| s.trim().to_string()).map_err(|e| format!("{p}: {e}"))
}

fn curl(args: &[&str]) -> Result<(String, u16), String> {
    let out = Command::new("curl").args(["-s", "-w", "\n%{http_code}"]).args(args).output().map_err(|e| format!("curl: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, code) = text.rsplit_once('\n').unwrap_or((&text, "0"));
    Ok((body.to_string(), code.trim().parse().unwrap_or(0)))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: dmdel CHANNEL MESSAGE_ID…");
        std::process::exit(2);
    }
    let tok = match token() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("dmdel: {e}");
            std::process::exit(2);
        }
    };
    let chan = &args[0];
    let auth = format!("Authorization: {tok}");
    let mut failed = 0;
    for id in &args[1..] {
        let url = format!("https://discord.com/api/v10/channels/{chan}/messages/{id}");
        match curl(&["-X", "DELETE", "-H", &auth, &url]) {
            Ok((_, 204)) => {}
            Ok((body, code)) => {
                eprintln!("dmdel: {id}: HTTP {code} {}", &body[..body.len().min(200)]);
                failed += 1;
                continue;
            }
            Err(e) => {
                eprintln!("dmdel: {id}: {e}");
                failed += 1;
                continue;
            }
        }
        // the control: the message must be gone from the channel's recent list
        std::thread::sleep(std::time::Duration::from_millis(800));
        let list = format!("https://discord.com/api/v10/channels/{chan}/messages?limit=50");
        match curl(&["-H", &auth, &list]) {
            Ok((body, 200)) if body.contains(&format!("\"id\":\"{id}\"")) => {
                eprintln!("dmdel: {id}: still listed after DELETE — NOT deleted");
                failed += 1;
            }
            Ok((_, 200)) => println!("deleted {id}"),
            Ok((body, code)) => eprintln!("dmdel: {id}: deleted (HTTP 204) but the control read failed: HTTP {code} {}", &body[..body.len().min(120)]),
            Err(e) => eprintln!("dmdel: {id}: deleted (HTTP 204) but the control read failed: {e}"),
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
