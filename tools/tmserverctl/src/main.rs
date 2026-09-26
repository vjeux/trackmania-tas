//! tmserverctl -- run a Trackmania 2020 dedicated server on a host without root.
//!
//! ```text
//! tmserverctl [--home DIR] <command>
//!   start                 start the supervisor in the background (idempotent)
//!   stop                  stop the server and the supervisor
//!   restart
//!   status                pids, uptime, and what the server says over XML-RPC
//!   logs [-n N] [-f]      tail the log (server output + supervisor + bridge notes)
//!   run [--quiet]         the supervisor itself, in the foreground (what `start` launches)
//!   check                 verify the layout and print the server command line
//!   rpc <Method> [args]   one XML-RPC call as SuperAdmin (123 int, true bool, 0.5 double,
//!                         s:123 string, [a,b] array of strings)
//!   lowg <cmd> [args] [--login L]   send a LowG mode command; `lowg status` prints the snapshot
//!   account <login> <pw>  write the dedicated-server account into dedicated_cfg.txt
//!   passwords             fresh random XML-RPC passwords in dedicated_cfg.txt
//!   cron                  print the crontab lines that keep the server alive across reboots
//! ```
//!
//! Home is `--home`, else `$TMSERVER_HOME`, else `~/tmserver`. Layout: `server/`
//! (the unpacked Nadeo archive), `tmserverctl.conf`, `run/`, `logs/`.

mod bridge;
mod clock;
mod config;
mod gbx;
mod logfile;
mod procs;
mod supervisor;
mod xml;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use config::Config;
use gbx::{Client, Value};

fn usage() -> ! {
    eprintln!("usage: tmserverctl [--home DIR] start|stop|restart|status|logs [-n N] [-f]|run [--quiet]|check|rpc <Method> [args...]|lowg <cmd> [args] [--login L]|account <login> <password>|passwords|cron");
    std::process::exit(2);
}

fn main() {
    // A CLI piped into `head` should die quietly on a closed pipe, not panic.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut home: Option<PathBuf> = None;
    while args.first().map(|a| a.as_str()) == Some("--home") {
        args.remove(0);
        if args.is_empty() {
            usage();
        }
        home = Some(PathBuf::from(args.remove(0)));
    }
    if args.is_empty() {
        usage();
    }
    let home = home.unwrap_or_else(Config::default_home);
    // The supervisor changes directory, so a relative --home must be pinned down first.
    let home = if home.is_absolute() { home } else { std::env::current_dir().map(|d| d.join(&home)).unwrap_or(home) };
    let cfg = match Config::load(&home) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tmserverctl: {e}");
            std::process::exit(1);
        }
    };
    let cmd = args.remove(0);
    let result = match cmd.as_str() {
        "start" => start(&cfg),
        "stop" => stop(&cfg),
        "restart" => stop(&cfg).and_then(|_| start(&cfg)),
        "status" => status(&cfg),
        "logs" => logs(&cfg, &args),
        "run" => supervisor::run(&cfg, !args.iter().any(|a| a == "--quiet")),
        "check" => check(&cfg),
        "rpc" => rpc(&cfg, &args),
        "lowg" => lowg(&cfg, &args),
        "cron" => cron(&cfg),
        "account" => account(&cfg, &args),
        "passwords" => passwords(&cfg),
        _ => usage(),
    };
    if let Err(e) = result {
        eprintln!("tmserverctl: {e}");
        std::process::exit(1);
    }
}

fn start(cfg: &Config) -> Result<(), String> {
    if let Some(pid) = procs::read_pid(&cfg.pid_file()) {
        if procs::alive(pid) {
            println!("already running (supervisor pid {pid})");
            return Ok(());
        }
    }
    // A server whose supervisor died would keep the ports; a second one would bind the next
    // port and never be found by players.
    if let Some(pid) = procs::read_pid(&cfg.server_pid_file()) {
        if procs::alive(pid) {
            println!("a server without supervisor is running (pid {pid}); terminating it first");
            procs::terminate(pid);
            let deadline = Instant::now() + Duration::from_secs(20);
            while Instant::now() < deadline && procs::alive(pid) {
                thread::sleep(Duration::from_millis(200));
            }
            if procs::alive(pid) {
                procs::kill9(pid);
            }
        }
        let _ = std::fs::remove_file(cfg.server_pid_file());
    }
    check_layout(cfg)?;
    std::fs::create_dir_all(cfg.log_dir()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(cfg.run_dir()).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let errlog = std::fs::OpenOptions::new().create(true).append(true).open(cfg.log_dir().join("supervisor.err")).map_err(|e| e.to_string())?;
    let mut command = Command::new(exe);
    command.arg("--home").arg(&cfg.home).arg("run").arg("--quiet").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(errlog)).current_dir(&cfg.home);
    // A new session: no controlling terminal, so the shell's exit does not take it down.
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let child = command.spawn().map_err(|e| format!("cannot spawn the supervisor: {e}"))?;
    let pid = child.id() as i32;
    // The supervisor writes its pid file first thing; give it a moment and confirm.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if procs::read_pid(&cfg.pid_file()) == Some(pid) && procs::alive(pid) {
            println!("started (supervisor pid {pid}); `tmserverctl logs -f` to watch, `tmserverctl status` in ~20 s");
            return Ok(());
        }
        if !procs::alive(pid) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!("the supervisor did not come up; see {}", cfg.log_dir().join("supervisor.err").display()))
}

fn stop(cfg: &Config) -> Result<(), String> {
    let Some(pid) = procs::read_pid(&cfg.pid_file()) else {
        println!("not running (no pid file)");
        return stop_orphan(cfg);
    };
    if !procs::alive(pid) {
        println!("not running (stale pid file for {pid})");
        let _ = std::fs::remove_file(cfg.pid_file());
        return stop_orphan(cfg);
    }
    procs::terminate(pid);
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if !procs::alive(pid) {
            println!("stopped");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    procs::kill9(pid);
    Err(format!("supervisor {pid} ignored SIGTERM for 30 s; sent SIGKILL"))
}

/// A server left behind by a dead supervisor.
fn stop_orphan(cfg: &Config) -> Result<(), String> {
    if let Some(pid) = procs::read_pid(&cfg.server_pid_file()) {
        if procs::alive(pid) {
            println!("a server without supervisor is running (pid {pid}); terminating it");
            procs::terminate(pid);
        }
        let _ = std::fs::remove_file(cfg.server_pid_file());
    }
    Ok(())
}

fn check_layout(cfg: &Config) -> Result<(), String> {
    let mut problems = Vec::new();
    if !cfg.binary().is_file() {
        problems.push(format!("missing server binary {}", cfg.binary().display()));
    }
    if !cfg.dedicated_cfg_path().is_file() {
        problems.push(format!("missing {}", cfg.dedicated_cfg_path().display()));
    }
    if !cfg.game_settings_path().is_file() {
        problems.push(format!("missing {}", cfg.game_settings_path().display()));
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    cfg.server_cfg().map(|_| ())
}

fn check(cfg: &Config) -> Result<(), String> {
    check_layout(cfg)?;
    let s = cfg.server_cfg()?;
    println!("home:          {}", cfg.home.display());
    println!("server:        {}", cfg.server_dir.display());
    println!("command:       ./TrackmaniaServer {}", cfg.server_args().join(" "));
    println!("server name:   {}", s.server_name);
    println!("account login: {}", if s.login.is_empty() { "(EMPTY -- the server will not go online)".to_string() } else { s.login });
    println!("game port:     {} (TCP+UDP; must be open in the Scaleway security group)", s.server_port);
    println!("xml-rpc port:  {} (localhost)", s.xmlrpc_port);
    println!("SuperAdmin pw: {}", if s.superadmin_password == "SuperAdmin" || s.superadmin_password.is_empty() { "DEFAULT -- change it in dedicated_cfg.txt" } else { "set" });
    if !cfg.game_settings_path().is_file() {
        return Err("no matchsettings".into());
    }
    let ms = std::fs::read(cfg.game_settings_path()).map_err(|e| e.to_string())?;
    match xml::parse(&ms) {
        Ok(root) => {
            let script = root.find("script_name").map(|e| e.text()).unwrap_or_default();
            let maps: Vec<String> = root.children_named("map").filter_map(|m| m.child("file").map(|f| f.text())).collect();
            println!("mode script:   {script}");
            println!("maps:          {}", maps.len());
            for m in &maps {
                let p = cfg.server_dir.join("UserData").join("Maps").join(m.replace('\\', "/"));
                println!("               {} {}", if p.is_file() { "ok     " } else { "MISSING" }, m);
            }
            let script_path = cfg.server_dir.join("UserData").join("Scripts").join(&script);
            if script.starts_with("Modes/") && !script_path.is_file() {
                println!("               (custom mode script {} not found under UserData/Scripts -- fine only if it is a Nadeo mode)", script);
            }
        }
        Err(e) => return Err(format!("{}: {e}", cfg.game_settings_path().display())),
    }
    Ok(())
}

fn connect(cfg: &Config) -> Result<Client, String> {
    let s = cfg.server_cfg()?;
    let mut c = Client::connect(("127.0.0.1", s.xmlrpc_port), Duration::from_secs(3)).map_err(|e| format!("xml-rpc 127.0.0.1:{}: {e}", s.xmlrpc_port))?;
    c.authenticate("SuperAdmin", &s.superadmin_password).map_err(|e| e.to_string())?;
    // Script callbacks arrive as ModeScriptCallbackArray only on the newer API; the default is the 2011 one.
    let _ = c.call("SetApiVersion", &["2023-04-24".into()]);
    Ok(c)
}

fn status(cfg: &Config) -> Result<(), String> {
    let sup = procs::read_pid(&cfg.pid_file()).filter(|p| procs::alive(*p));
    let srv = procs::read_pid(&cfg.server_pid_file()).filter(|p| procs::alive(*p));
    match sup {
        Some(p) => println!("supervisor: running, pid {p}{}", procs::uptime_secs(p).map(|u| format!(", up {}", human(u))).unwrap_or_default()),
        None => println!("supervisor: not running"),
    }
    match srv {
        Some(p) => println!("server:     running, pid {p}{}", procs::uptime_secs(p).map(|u| format!(", up {}", human(u))).unwrap_or_default()),
        None => println!("server:     not running"),
    }
    let mut c = match connect(cfg) {
        Ok(c) => c,
        Err(e) => {
            println!("xml-rpc:    unavailable ({e})");
            return if sup.is_some() { Ok(()) } else { Err("not running".into()) };
        }
    };
    let st = c.call("GetStatus", &[]).map_err(|e| e.to_string())?;
    println!("status:     {} (code {})", st.field_str("Name"), st.field_str("Code"));
    if let Ok(v) = c.call("GetServerName", &[]) {
        println!("name:       {}", v.to_plain());
    }
    let mut server_login = String::new();
    if let Ok(v) = c.call("GetSystemInfo", &[]) {
        server_login = v.field_str("ServerLogin");
        println!("public ip:  {} port {} (login {})", v.field_str("PublishedIp"), v.field_str("Port"), server_login);
        if !cfg.lan && !server_login.is_empty() {
            println!("join link:  trackmania://#join={server_login}@Trackmania");
        }
    }
    if let Ok(v) = c.call("GetCurrentMapInfo", &[]) {
        println!("map:        {} by {} [{}]", v.field_str("Name"), v.field_str("Author"), v.field_str("FileName"));
    }
    if let Ok(v) = c.call("GetScriptName", &[]) {
        println!("mode:       {}{}", v.field_str("CurrentValue"), if v.field_str("NextValue") != v.field_str("CurrentValue") { format!(" (next: {})", v.field_str("NextValue")) } else { String::new() });
    }
    if let Ok(v) = c.call("GetPlayerList", &[200.into(), 0.into()]) {
        let players: Vec<String> = v
            .as_array()
            .unwrap_or(&[])
            .iter()
            .filter(|p| p.field_str("Login") != server_login)
            .map(|p| format!("{} ({}){}", p.field_str("NickName"), p.field_str("Login"), if p.get("SpectatorStatus").and_then(|s| s.as_i64()).unwrap_or(0) != 0 { " spec" } else { "" }))
            .collect();
        println!("players:    {}{}", players.len(), if players.is_empty() { String::new() } else { format!(": {}", players.join(", ")) });
    }
    if let Ok(v) = c.call("GetMaxPlayers", &[]) {
        println!("max:        {} players", v.field_str("CurrentValue"));
    }
    Ok(())
}

fn human(secs: u64) -> String {
    if secs < 60 {
        format!("{secs} s")
    } else if secs < 3600 {
        format!("{} min", secs / 60)
    } else if secs < 86_400 {
        format!("{} h {:02} min", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{} d {:02} h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

fn logs(cfg: &Config, args: &[String]) -> Result<(), String> {
    let mut n = 50usize;
    let mut follow = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-f" | "--follow" => follow = true,
            "-n" => {
                i += 1;
                n = args.get(i).and_then(|v| v.parse().ok()).ok_or("-n needs a number")?;
            }
            other => return Err(format!("unknown logs option {other}")),
        }
        i += 1;
    }
    let stop = Arc::new(AtomicBool::new(false));
    if follow {
        supervisor::install_signal_handlers();
        let s = stop.clone();
        thread::spawn(move || {
            while !supervisor::STOP.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
            }
            s.store(true, Ordering::Relaxed);
        });
    }
    logfile::tail(&cfg.log_file(), n, follow, stop).map_err(|e| e.to_string())
}

fn rpc(cfg: &Config, args: &[String]) -> Result<(), String> {
    let Some((method, rest)) = args.split_first() else {
        return Err("rpc needs a method name".into());
    };
    let params: Vec<Value> = rest.iter().map(|a| gbx::value_from_cli(a)).collect();
    let mut c = connect(cfg)?;
    let v = c.call(method, &params).map_err(|e| e.to_string())?;
    println!("{}", pretty(&v, 0));
    Ok(())
}

/// Multi-line rendering for structs and arrays of structs.
fn pretty(v: &Value, indent: usize) -> String {
    let pad = " ".repeat(indent);
    match v {
        Value::Struct(m) => m.iter().map(|(k, v)| match v {
            Value::Struct(_) | Value::Array(_) => format!("{pad}{k}:\n{}", pretty(v, indent + 2)),
            _ => format!("{pad}{k}: {}", v.to_plain()),
        }).collect::<Vec<_>>().join("\n"),
        Value::Array(items) => items.iter().enumerate().map(|(i, v)| match v {
            Value::Struct(_) | Value::Array(_) => format!("{pad}- [{i}]\n{}", pretty(v, indent + 2)),
            _ => format!("{pad}- {}", v.to_plain()),
        }).collect::<Vec<_>>().join("\n"),
        other => format!("{pad}{}", other.to_plain()),
    }
}

fn lowg(cfg: &Config, args: &[String]) -> Result<(), String> {
    let mut login = String::from("tmserverctl");
    let mut words: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--login" {
            i += 1;
            login = args.get(i).cloned().ok_or("--login needs a value")?;
        } else {
            words.push(args[i].clone());
        }
        i += 1;
    }
    if words.is_empty() {
        return Err("lowg needs a command: gravity <0..1> | gravity all <0..1> | bots <n> | collisions on|off | status".into());
    }
    let mut c = connect(cfg)?;
    let want_status = words[0] == "status";
    if want_status {
        c.call("EnableCallbacks", &[true.into()]).map_err(|e| e.to_string())?;
    }
    let mut params = vec![login];
    params.extend(words);
    c.call("TriggerModeScriptEventArray", &[bridge::CMD_EVENT.into(), Value::from(params)]).map_err(|e| e.to_string())?;
    if !want_status {
        println!("sent");
        return Ok(());
    }
    c.set_read_timeout(Some(Duration::from_millis(500))).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut pending = c.take_callbacks();
    while Instant::now() < deadline {
        for cb in pending.drain(..) {
            if cb.method == "ManiaPlanet.ModeScriptCallbackArray" && cb.params.first().and_then(|v| v.as_str()) == Some(bridge::STATUS_EVENT) {
                if let Some(lines) = cb.params.get(1).and_then(|v| v.as_array()) {
                    for l in lines {
                        println!("{}", l.to_plain());
                    }
                }
                return Ok(());
            }
        }
        match c.next_callback() {
            Ok(Some(cb)) => pending.push(cb),
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("no LowG.Status callback within 4 s -- is the LowG mode running? (tmserverctl status shows the mode)".into())
}

/// `account <login> <password>`: write the dedicated-server account into dedicated_cfg.txt
/// (the `<masterserver_account>` block), keeping the rest of the file byte for byte.
fn account(cfg: &Config, args: &[String]) -> Result<(), String> {
    let [login, password] = args else {
        return Err("usage: tmserverctl account <server-login> <server-password>".into());
    };
    let path = cfg.dedicated_cfg_path();
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let start = find_outside_comments(&text, "<masterserver_account>").ok_or("no <masterserver_account> block in dedicated_cfg.txt")?;
    let end = text[start..].find("</masterserver_account>").map(|i| start + i).ok_or("unterminated <masterserver_account> block")?;
    let block = &text[start..end];
    let replaced = replace_element(block, "login", login).and_then(|b| replace_element(&b, "password", password)).ok_or("the <masterserver_account> block has no <login>/<password> elements")?;
    let mut out = String::with_capacity(text.len() + 64);
    out.push_str(&text[..start]);
    out.push_str(&replaced);
    out.push_str(&text[end..]);
    let tmp = path.with_extension("txt.new");
    std::fs::write(&tmp, out).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("account written to {} (login {login}); `tmserverctl restart` to apply", path.display());
    Ok(())
}

fn replace_element(block: &str, name: &str, value: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let a = block.find(&open)? + open.len();
    let b = block[a..].find(&close)? + a;
    Some(format!("{}{}{}", &block[..a], xml::escape(value), &block[b..]))
}

/// `passwords`: give the three XML-RPC authorization levels fresh random passwords
/// (they only ever travel on localhost; tmserverctl reads the SuperAdmin one back).
fn passwords(cfg: &Config) -> Result<(), String> {
    let path = cfg.dedicated_cfg_path();
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let start = find_outside_comments(&text, "<authorization_levels>").ok_or("no <authorization_levels> block")?;
    let end = text[start..].find("</authorization_levels>").map(|i| start + i).ok_or("unterminated <authorization_levels>")?;
    let mut block = text[start..end].to_string();
    let mut out_block = String::new();
    let mut n = 0;
    while let Some(a) = block.find("<password>") {
        let b = block[a..].find("</password>").ok_or("unterminated <password>")? + a;
        out_block.push_str(&block[..a]);
        out_block.push_str("<password>");
        out_block.push_str(&random_password(24)?);
        block = block[b..].to_string();
        n += 1;
    }
    out_block.push_str(&block);
    if n == 0 {
        return Err("no <password> elements in <authorization_levels>".into());
    }
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&out_block);
    out.push_str(&text[end..]);
    let tmp = path.with_extension("txt.new");
    std::fs::write(&tmp, out).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{n} XML-RPC password(s) replaced in {}", path.display());
    Ok(())
}

fn random_password(len: usize) -> Result<String, String> {
    use std::io::Read;
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut bytes = vec![0u8; len];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes)).map_err(|e| format!("/dev/urandom: {e}"))?;
    Ok(bytes.iter().map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char).collect())
}

fn cron(cfg: &Config) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    println!("# keep the Trackmania server alive: at boot, and every 2 minutes if it is gone (start is a no-op when it runs)");
    println!("@reboot {} --home {} start >> {}/cron.log 2>&1", exe.display(), cfg.home.display(), cfg.log_dir().display());
    println!("*/2 * * * * {} --home {} start >> {}/cron.log 2>&1", exe.display(), cfg.home.display(), cfg.log_dir().display());
    Ok(())
}

/// First occurrence of `needle` that is not inside an XML comment.
fn find_outside_comments(text: &str, needle: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = text[from..].find(needle) {
        let at = from + i;
        let before = &text[..at];
        let open = before.rfind("<!--");
        let close = before.rfind("-->");
        let in_comment = match (open, close) {
            (Some(o), Some(c)) => o > c,
            (Some(_), None) => true,
            _ => false,
        };
        if !in_comment {
            return Some(at);
        }
        from = at + needle.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_edit_skips_comments() {
        let text = "<!-- fill <masterserver_account> with the account --><dedicated><masterserver_account>\n\t<login>a</login>\n\t<password>b</password>\n</masterserver_account></dedicated>";
        let start = find_outside_comments(text, "<masterserver_account>").unwrap();
        assert!(start > text.find("-->").unwrap());
        let end = text[start..].find("</masterserver_account>").unwrap() + start;
        let block = replace_element(&text[start..end], "login", "srv_1").unwrap();
        let block = replace_element(&block, "password", "p&w").unwrap();
        assert_eq!(block, "<masterserver_account>\n\t<login>srv_1</login>\n\t<password>p&amp;w</password>\n");
    }
}
