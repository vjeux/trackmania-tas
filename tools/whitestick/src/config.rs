//! Configuration: `~/.whitestick/config.toml`, with environment overrides.
//!
//! ```toml
//! relay = "https://whitestick-relay.example.workers.dev"
//! token = "<the shared secret, also the Worker's PSK>"
//! instance = "WhiteStick"          # box the client talks to by default
//! # proxy = "http://fwdproxy:8080" # "none" forces a direct connection
//!
//! [agent]                          # only read by `whitestick agent`
//! name = "WhiteStick"              # the box's name at the relay
//! cwd = "/mnt/c/Users/vjeux"       # where commands start
//! shell = "/bin/sh"
//! ```
//!
//! Environment: `WHITESTICK_CONFIG` (path), `WHITESTICK_RELAY`,
//! `WHITESTICK_TOKEN`, `WHITESTICK_INSTANCE`, `WHITESTICK_PROXY`.

use anyhow::{bail, Context, Result};
use serde::Deserialize;

pub const DEFAULT_INSTANCE: &str = "WhiteStick";

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub relay: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub instance: Option<String>,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub agent: AgentConfig,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct AgentConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub shell: Option<String>,
}

pub fn config_path() -> Result<std::path::PathBuf> {
    if let Ok(p) = std::env::var("WHITESTICK_CONFIG") {
        if !p.is_empty() {
            return Ok(p.into());
        }
    }
    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(std::path::PathBuf::from(home).join(".whitestick").join("config.toml"))
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let mut cfg = match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str::<Config>(&raw)
                .with_context(|| format!("parse {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
        };
        if let Some(v) = env_nonempty("WHITESTICK_RELAY") {
            cfg.relay = Some(v);
        }
        if let Some(v) = env_nonempty("WHITESTICK_TOKEN") {
            cfg.token = Some(v);
        }
        if let Some(v) = env_nonempty("WHITESTICK_INSTANCE") {
            cfg.instance = Some(v);
        }
        if let Some(v) = env_nonempty("WHITESTICK_PROXY") {
            cfg.proxy = Some(v);
        }
        Ok(cfg)
    }

    pub fn relay(&self) -> Result<&str> {
        match self.relay.as_deref() {
            Some(r) if !r.trim().is_empty() => Ok(r.trim().trim_end_matches('/')),
            _ => bail!(
                "no relay configured: set `relay = \"https://...\"` in {} (or WHITESTICK_RELAY)",
                config_path().map(|p| p.display().to_string()).unwrap_or_default()
            ),
        }
    }

    pub fn token(&self) -> Result<&str> {
        match self.token.as_deref() {
            Some(t) if !t.trim().is_empty() => Ok(t.trim()),
            _ => bail!(
                "no token configured: set `token = \"...\"` in {} (or WHITESTICK_TOKEN)",
                config_path().map(|p| p.display().to_string()).unwrap_or_default()
            ),
        }
    }

    pub fn instance(&self) -> &str {
        self.instance.as_deref().unwrap_or(DEFAULT_INSTANCE)
    }

    pub fn agent_name(&self) -> &str {
        self.agent
            .name
            .as_deref()
            .or(self.instance.as_deref())
            .unwrap_or(DEFAULT_INSTANCE)
    }
}
