//! Configuration: where the server lives and how it is started.
//!
//! `~/tmserver/tmserverctl.conf` is `key = value` lines (`#` comments). Every key
//! has a default, so an empty or missing file is a valid configuration:
//!
//! ```text
//! server_dir    = server                 # relative to the home directory
//! dedicated_cfg = dedicated_cfg.txt      # under UserData/Config/
//! game_settings = MatchSettings/lowg.txt # under UserData/Maps/
//! title         = Trackmania
//! lan           = false                  # true: /lan, no master-server account (local testing)
//! restart_delay_s = 5                    # first restart delay; doubles up to 60 s
//! log_max_mb    = 20                     # rotate server.log above this
//! log_keep      = 5                      # rotated files kept
//! bridge        = true                   # chat-command bridge for the LowG mode
//! ```
//!
//! The SuperAdmin password and the XML-RPC port are read from the server's own
//! dedicated_cfg.txt, so they are never written twice.

use std::fs;
use std::path::{Path, PathBuf};

use crate::xml;

#[derive(Debug, Clone)]
pub struct Config {
    pub home: PathBuf,
    pub server_dir: PathBuf,
    pub dedicated_cfg: String,
    pub game_settings: String,
    pub title: String,
    pub lan: bool,
    pub restart_delay_s: u64,
    pub log_max_bytes: u64,
    pub log_keep: usize,
    pub bridge: bool,
}

/// What the server's dedicated_cfg.txt says that the tool needs.
#[derive(Debug, Clone)]
pub struct ServerCfg {
    pub superadmin_password: String,
    pub xmlrpc_port: u16,
    pub server_name: String,
    pub login: String,
    pub server_port: u16,
}

impl Config {
    pub fn default_home() -> PathBuf {
        if let Ok(h) = std::env::var("TMSERVER_HOME") {
            if !h.is_empty() {
                return PathBuf::from(h);
            }
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join("tmserver")
    }

    pub fn load(home: &Path) -> Result<Config, String> {
        let mut c = Config {
            home: home.to_path_buf(),
            server_dir: home.join("server"),
            dedicated_cfg: "dedicated_cfg.txt".to_string(),
            game_settings: "MatchSettings/lowg.txt".to_string(),
            title: "Trackmania".to_string(),
            lan: false,
            restart_delay_s: 5,
            log_max_bytes: 20 * 1024 * 1024,
            log_keep: 5,
            bridge: true,
        };
        let path = home.join("tmserverctl.conf");
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(c),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(format!("{}:{}: expected `key = value`", path.display(), n + 1));
            };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            let bad = |what: &str| format!("{}:{}: {what} for {k}: {v:?}", path.display(), n + 1);
            match k {
                "server_dir" => {
                    let p = PathBuf::from(v);
                    c.server_dir = if p.is_absolute() { p } else { home.join(p) };
                }
                "dedicated_cfg" => c.dedicated_cfg = v.to_string(),
                "game_settings" => c.game_settings = v.to_string(),
                "title" => c.title = v.to_string(),
                "lan" => c.lan = parse_bool(v).ok_or_else(|| bad("expected true/false"))?,
                "bridge" => c.bridge = parse_bool(v).ok_or_else(|| bad("expected true/false"))?,
                "restart_delay_s" => c.restart_delay_s = v.parse().map_err(|_| bad("expected seconds"))?,
                "log_max_mb" => c.log_max_bytes = v.parse::<u64>().map_err(|_| bad("expected megabytes"))? * 1024 * 1024,
                "log_keep" => c.log_keep = v.parse().map_err(|_| bad("expected a count"))?,
                other => return Err(format!("{}:{}: unknown key {other:?}", path.display(), n + 1)),
            }
        }
        Ok(c)
    }

    pub fn binary(&self) -> PathBuf {
        self.server_dir.join("TrackmaniaServer")
    }
    pub fn dedicated_cfg_path(&self) -> PathBuf {
        self.server_dir.join("UserData").join("Config").join(&self.dedicated_cfg)
    }
    pub fn game_settings_path(&self) -> PathBuf {
        self.server_dir.join("UserData").join("Maps").join(&self.game_settings)
    }
    pub fn run_dir(&self) -> PathBuf {
        self.home.join("run")
    }
    pub fn log_dir(&self) -> PathBuf {
        self.home.join("logs")
    }
    pub fn pid_file(&self) -> PathBuf {
        self.run_dir().join("tmserverctl.pid")
    }
    pub fn server_pid_file(&self) -> PathBuf {
        self.run_dir().join("server.pid")
    }
    pub fn log_file(&self) -> PathBuf {
        self.log_dir().join("server.log")
    }

    /// The exact server command line.
    pub fn server_args(&self) -> Vec<String> {
        let mut a = vec![
            "/nodaemon".to_string(),
            format!("/dedicated_cfg={}", self.dedicated_cfg),
            format!("/game_settings={}", self.game_settings),
            format!("/title={}", self.title),
        ];
        if self.lan {
            a.push("/lan".to_string());
        }
        a
    }

    pub fn server_cfg(&self) -> Result<ServerCfg, String> {
        let path = self.dedicated_cfg_path();
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let root = xml::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut password = String::new();
        if let Some(levels) = root.child("authorization_levels") {
            for level in levels.children_named("level") {
                let name = level.child("name").map(|n| n.text()).unwrap_or_default();
                if name.trim() == "SuperAdmin" {
                    password = level.child("password").map(|p| p.text().trim().to_string()).unwrap_or_default();
                }
            }
        }
        let sys = root.child("system_config");
        let port = sys
            .and_then(|s| s.child("xmlrpc_port"))
            .map(|p| p.text().trim().to_string())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<u16>().map_err(|_| format!("{}: bad xmlrpc_port {p:?}", path.display())))
            .transpose()?
            .unwrap_or(5000);
        let server_port = sys
            .and_then(|s| s.child("server_port"))
            .map(|p| p.text().trim().to_string())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<u16>().map_err(|_| format!("{}: bad server_port {p:?}", path.display())))
            .transpose()?
            .unwrap_or(2350);
        let server_name = root.child("server_options").and_then(|o| o.child("name")).map(|n| n.text().trim().to_string()).unwrap_or_default();
        let login = root.child("masterserver_account").and_then(|o| o.child("login")).map(|n| n.text().trim().to_string()).unwrap_or_default();
        Ok(ServerCfg { superadmin_password: password, xmlrpc_port: port, server_name, login, server_port })
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_server_cfg() {
        let dir = std::env::temp_dir().join(format!("tmserverctl-cfg-test-{}", std::process::id()));
        let cfgdir = dir.join("server").join("UserData").join("Config");
        fs::create_dir_all(&cfgdir).unwrap();
        fs::write(
            cfgdir.join("dedicated_cfg.txt"),
            "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\" ?>\n<dedicated><authorization_levels><level><name>SuperAdmin</name><password>s3cret</password></level><level><name>User</name><password>u</password></level></authorization_levels><masterserver_account><login>yannexsrv</login><password>x</password></masterserver_account><server_options><name>Yannex low-g</name></server_options><system_config><server_port>2350</server_port><xmlrpc_port>5001</xmlrpc_port></system_config></dedicated>",
        )
        .unwrap();
        fs::write(dir.join("tmserverctl.conf"), "lan = true\nlog_max_mb = 1 # small\n").unwrap();
        let c = Config::load(&dir).unwrap();
        assert!(c.lan);
        assert_eq!(c.log_max_bytes, 1024 * 1024);
        let s = c.server_cfg().unwrap();
        assert_eq!(s.superadmin_password, "s3cret");
        assert_eq!(s.xmlrpc_port, 5001);
        assert_eq!(s.server_name, "Yannex low-g");
        assert_eq!(s.login, "yannexsrv");
        assert_eq!(c.server_args(), vec!["/nodaemon", "/dedicated_cfg=dedicated_cfg.txt", "/game_settings=MatchSettings/lowg.txt", "/title=Trackmania", "/lan"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_key_is_an_error() {
        let dir = std::env::temp_dir().join(format!("tmserverctl-cfg-test2-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("tmserverctl.conf"), "colour = blue\n").unwrap();
        assert!(Config::load(&dir).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
