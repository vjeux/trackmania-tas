//! `jumprig srvload MAP…` — load maps on the Linux dedicated server (the WSL
//! copy under /home/vjeux/tt-rl/srv/s) and read its verdict: `...Load
//! succeeds`, or `...ERROR: Load failed` with the `Missing Items:` list it
//! prints for embedded items it cannot instantiate. The server side of AR's
//! 2026-10-11 question (a map with a wrapper item referencing a pack prefab:
//! does a dedicated server resolve it?). No game lock: the server is its own
//! process on its own port (3461; the lanes' RL-ball server sits on 3452).

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct ServerVerdict {
    pub map: String,
    pub loaded: bool,
    pub missing: Vec<String>,
    pub log: String,
}

fn server_root() -> PathBuf {
    PathBuf::from(std::env::var("TM_DEDICATED").unwrap_or_else(|_| "/home/vjeux/tt-rl/srv/s".to_string()))
}

const CFG: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<dedicated>
	<authorization_levels>
		<level>
			<name>SuperAdmin</name>
			<password>probe</password>
		</level>
	</authorization_levels>
	<masterserver_account>
	</masterserver_account>
	<server_options>
		<name>prefab reference probe</name>
		<comment></comment>
		<hide_server>1</hide_server>
		<max_players>1</max_players>
		<password>probe</password>
		<callvote_timeout>0</callvote_timeout>
		<disable_horns>True</disable_horns>
	</server_options>
	<system_config>
		<connection_uploadrate>2048000</connection_uploadrate>
		<connection_downloadrate>2048000</connection_downloadrate>
		<xmlrpc_port>0</xmlrpc_port>
		<server_port>3461</server_port>
	</system_config>
</dedicated>
"#;

/// Load one map; the server is started for it and stopped after the verdict.
pub fn load_one(map: &Path, timeout: Duration) -> Result<ServerVerdict, String> {
    let root = server_root();
    if !root.join("TrackmaniaServer").exists() {
        return Err(format!("no dedicated server at {}", root.display()));
    }
    let stem = map.file_name().and_then(|n| n.to_str()).ok_or("map name")?.to_string();
    let maps_dir = root.join("UserData/Maps/Probe");
    fs::create_dir_all(&maps_dir).map_err(|e| e.to_string())?;
    fs::copy(map, maps_dir.join(&stem)).map_err(|e| format!("copy {}: {e}", map.display()))?;
    let ms_dir = root.join("UserData/Maps/MatchSettings");
    fs::create_dir_all(&ms_dir).map_err(|e| e.to_string())?;
    let ms_name = format!("probe-{}.txt", stem.replace(['.', ' '], "_"));
    let playlist = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\" ?>\n<playlist>\n<gameinfos><game_mode>0</game_mode><script_name>Trackmania/TM_TimeAttack_Online</script_name></gameinfos>\n<filter><is_lan>1</is_lan><is_internet>0</is_internet><sort_index>1000</sort_index></filter>\n<startindex>0</startindex>\n<map><file>Probe/{stem}</file></map>\n</playlist>\n"
    );
    fs::write(ms_dir.join(&ms_name), playlist).map_err(|e| e.to_string())?;
    let cfg_dir = root.join("UserData/Config");
    fs::write(cfg_dir.join("dedicated_probe.txt"), CFG).map_err(|e| e.to_string())?;
    let mut child = Command::new("./TrackmaniaServer")
        .args(["/nodaemon", "/dedicated_cfg=dedicated_probe.txt", &format!("/game_settings=MatchSettings/{ms_name}"), "/lan"])
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("launching the server: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let start = Instant::now();
    let mut log = String::new();
    let mut loaded = None;
    let mut missing = Vec::new();
    let mut in_missing = false;
    while start.elapsed() < timeout {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                log.push_str(&line);
                log.push('\n');
                if line.contains("...Load succeeds") {
                    loaded = Some(true);
                    break;
                }
                if line.contains("Load failed") || line.contains("Could not load map") {
                    loaded = Some(false);
                }
                if line.starts_with("Missing Items") {
                    in_missing = true;
                    continue;
                }
                if in_missing {
                    if let Some(name) = line.trim().strip_prefix("- ") {
                        if !missing.contains(&name.to_string()) {
                            missing.push(name.to_string());
                        }
                        continue;
                    }
                    in_missing = false;
                    if loaded == Some(false) {
                        break;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    // drain what is left
    while let Ok(line) = rx.try_recv() {
        log.push_str(&line);
        log.push('\n');
    }
    let _ = fs::remove_file(maps_dir.join(&stem));
    match loaded {
        Some(l) => Ok(ServerVerdict { map: stem, loaded: l, missing, log }),
        None => Err(format!("{stem}: no verdict from the server in {:.0}s; log:\n{log}", timeout.as_secs_f64())),
    }
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let maps: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if maps.is_empty() {
        return Err("srvload MAP.Map.Gbx…".into());
    }
    let mut all_ok = true;
    for m in maps {
        let v = load_one(Path::new(m), Duration::from_secs(180))?;
        if v.loaded {
            println!("{}: LOAD SUCCEEDS on the dedicated server", v.map);
        } else {
            all_ok = false;
            println!("{}: LOAD FAILED on the dedicated server; {} missing item(s):", v.map, v.missing.len());
            for mi in &v.missing {
                println!("  - {mi}");
            }
        }
        if args.iter().any(|a| a == "--log") {
            println!("--- server log ---\n{}", v.log);
        }
    }
    if all_ok { Ok(()) } else { Err("a map did not load on the dedicated server".into()) }
}
