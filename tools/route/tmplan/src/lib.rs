//! `tmplan` — the route planner skeleton (BRIEF-GEOM R5).
//!
//! * `surface`   — the cartographer's surface graph over the FULL checkpoint set of a `gates.json`
//! * `estimator` — the `EdgeEstimator` seam; `Geometric` is the first implementation, R is the second
//! * `planner`   — beam search over (visited, gate group, arrival bucket), finish last
//! * `export`    — a plan → `router-plan` route file with `Predicted` legs

pub mod estimator;
pub mod export;
pub mod planner;
pub mod surface;

/// The pak files the geometry comes from: `TM_PAKS` (colon-separated paths) when set, else the dedicated
/// server's `$TM_SERVER/Packs/{dedicated_TMStadium,dedicated,resource}.pak`. The server tree under /tmp was
/// wiped once (2026-09-07 10:26Z, every plan failed with "no .pak"); the game client's `Stadium.pak` in the
/// bank is the fallback that keeps the tools alive.
pub fn pak_paths() -> Result<Vec<String>, String> {
    if let Ok(list) = std::env::var("TM_PAKS") {
        let v: Vec<String> = list.split(':').filter(|s| !s.is_empty() && std::path::Path::new(s).exists()).map(|s| s.to_string()).collect();
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let server = std::env::var("TM_SERVER").map_err(|_| "TM_SERVER not set (and no TM_PAKS)")?;
    let v: Vec<String> = ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"].iter().map(|n| format!("{server}/Packs/{n}")).filter(|p| std::path::Path::new(p).exists()).collect();
    if v.is_empty() {
        return Err(format!("no .pak in {server}/Packs (set TM_PAKS=path.pak:path2.pak)"));
    }
    Ok(v)
}
