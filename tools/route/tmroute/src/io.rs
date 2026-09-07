//! Files: one route = one JSON `TrackGeom`; the index `routes.tsv`.
//!
//! `~/persistent/private-30d/tm-route/routes/<mapUid>/route-<source>-<rank>.json`
//! `routes/routes.tsv`: `map_uid  map_name  source  rank  status  predicted_ms
//! certified_ms  gate_order(comma-sep waypoints)  file`.

use crate::types::{RouteStatus, TrackGeom, TrackGeomExt};
use std::path::{Path, PathBuf};

pub fn read_route(p: &Path) -> Result<TrackGeom, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn write_route(p: &Path, g: &TrackGeom) -> Result<(), String> {
    let s = serde_json::to_string(g).map_err(|e| e.to_string())?;
    write_atomic(p, s.as_bytes())
}

/// Write to `<name>.tmp` then rename: a reader on ANOTHER box (the bank is a network
/// store that publishes bytes on close) never sees a half-written file — the MODEL
/// arm read four truncated gates.json while a batch was rewriting them (2026-09-07).
pub fn write_atomic(p: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension(format!("{}.tmp", p.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, p).map_err(|e| format!("{} → {}: {e}", tmp.display(), p.display()))
}

pub fn read_gates(p: &Path) -> Result<crate::gates::GatesFile, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn write_gates(p: &Path, g: &crate::gates::GatesFile) -> Result<(), String> {
    let s = serde_json::to_string_pretty(g).map_err(|e| e.to_string())?;
    write_atomic(p, s.as_bytes())
}

/// The canonical file name of a route inside `routes/<mapUid>/`.
pub fn route_file_name(source: &str, rank: u32) -> String {
    format!("route-{source}-{rank}.json")
}

pub const INDEX_HEADER: &str =
    "map_uid\tmap_name\tsource\trank\tstatus\tpredicted_ms\tcertified_ms\tgate_order\tfile";

/// One index row for a route file.
pub fn index_row(g: &TrackGeom, map_name: &str, file: &str) -> String {
    let (source, rank, pred, status, cert) = match &g.route {
        Some(r) => {
            let (st, cert) = match &r.status {
                RouteStatus::Hypothesis => ("Hypothesis".to_string(), -1),
                RouteStatus::Certified { ms, .. } => ("Certified".to_string(), *ms),
            };
            (r.source.clone(), r.rank, r.predicted_ms, st, cert)
        }
        None => (g.source.clone(), 0, -1, "Geometry".to_string(), -1),
    };
    let order: Vec<String> = g.gate_order().iter().map(|x| x.to_string()).collect();
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        g.map_uid,
        map_name,
        source,
        rank,
        status,
        pred,
        cert,
        order.join(","),
        file
    )
}

/// Rebuild `routes.tsv` from every `<root>/<uid>/route-*.json`. Map names come
/// from `names` (uid → name) or the uid when unknown.
pub fn rebuild_index(root: &Path, names: &dyn Fn(&str) -> String) -> Result<(usize, PathBuf), String> {
    let mut rows = vec![INDEX_HEADER.to_string()];
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut n = 0;
    for d in dirs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&d)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().map_or(false, |f| { let f = f.to_string_lossy(); f.starts_with("route-") && f.ends_with(".json") && !f.ends_with(".specials.json") }))
            .collect();
        files.sort();
        for f in files {
            let g = read_route(&f)?;
            let rel = format!(
                "{}/{}",
                d.file_name().unwrap().to_string_lossy(),
                f.file_name().unwrap().to_string_lossy()
            );
            rows.push(index_row(&g, &names(&g.map_uid), &rel));
            n += 1;
        }
    }
    let out = root.join("routes.tsv");
    write_atomic(&out, (rows.join("\n") + "\n").as_bytes())?;
    Ok((n, out))
}

/// Seconds with a decimal (`23.144`), `-` for a missing value.
pub fn secs(ms: i32) -> String {
    if ms < 0 {
        return "-".into();
    }
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
