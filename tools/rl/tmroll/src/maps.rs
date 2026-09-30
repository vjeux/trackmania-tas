//! The worker's map set: per map uid, the assets an episode needs -- the map
//! file, the template (the rank-1 PLAIN ghost's container with our archive; the
//! donor's own seed and countdown kept, so the donor's tape reproduces in it
//! and seeds the human-line archive -- LEARN's `reference-identity`), DATA's
//! geom.json as the Track, the donor's per-tick actions, and the map's archive
//! file (under the worker's work dir; a `state-archive.json` in the map dir is
//! taken as the starting point). Built on first use, kept.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tmstate::Action;

pub struct MapAssets {
    pub uid: String,
    pub map: PathBuf,
    pub reference: PathBuf,
    pub track: Arc<tmenv::Track>,
    /// Ticks in the template (the episode cap is `ticks - 1`).
    pub ticks: usize,
    /// The donor's tape as per-tick actions (its countdown included: the
    /// template carries the same countdown, so the index shift is 0).
    pub donor_actions: Arc<Vec<Action>>,
    pub donor_ms: i64,
    /// Where this box persists the map's archive.
    pub archive: PathBuf,
    pub template_name: String,
}

pub struct MapSet {
    pub maps_dir: PathBuf,
    pub work: PathBuf,
    loaded: Mutex<HashMap<String, Arc<MapAssets>>>,
}

impl MapSet {
    pub fn new(maps_dir: &Path, work: &Path) -> MapSet {
        MapSet { maps_dir: maps_dir.to_path_buf(), work: work.to_path_buf(), loaded: Mutex::new(HashMap::new()) }
    }

    pub fn loaded(&self, uid: &str) -> Option<Arc<MapAssets>> {
        self.loaded.lock().unwrap().get(uid).cloned()
    }

    /// The assets for `uid`, built on first use. Building (tar extraction, the
    /// template, geom.json) happens UNDER the table lock: sixteen env threads
    /// asked for the same new map at once and their concurrent tar extractions
    /// into one directory failed thirteen of them (2026-09-07). A few seconds
    /// once per map is nothing; a torn extraction is an episode error.
    pub fn get(&self, uid: &str) -> Result<Arc<MapAssets>, String> {
        let mut l = self.loaded.lock().unwrap();
        if let Some(a) = l.get(uid) {
            return Ok(a.clone());
        }
        let a = Arc::new(self.build(uid)?);
        l.insert(uid.to_string(), a.clone());
        Ok(a)
    }

    fn build(&self, uid: &str) -> Result<MapAssets, String> {
        if uid.is_empty() || uid.contains('/') || uid.contains("..") {
            return Err(format!("bad map uid {uid:?}"));
        }
        let map_dir = self.maps_dir.join(uid);
        let map = map_dir.join("map.Map.Gbx");
        if !map.exists() {
            return Err(format!("{}: no map.Map.Gbx", map_dir.display()));
        }
        let work = self.work.join("maps").join(uid);
        std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
        let local_map = work.join("map.Map.Gbx");
        if !local_map.exists() {
            std::fs::copy(&map, &local_map).map_err(|e| format!("{}: {e}", map.display()))?;
        }
        let gj = map_dir.join("geom.json");
        if !gj.exists() {
            return Err(format!("{}: no geom.json (the worker runs only DATA-geometry maps)", map_dir.display()));
        }
        let track = Arc::new(tmenv::Track::load_geom_json(&gj)?);
        // the reference container. Two layouts:
        //  * DATA: the fastest ghost is the donor; the template is its container with our archive, its
        //    tape seeds the human line;
        //  * NO-GHOST (the tiny campaign): `template.Ghost.Gbx` in the map dir is a ready container
        //    (`tmenv from-template --map --declare-ms --cps`, start waypoint set); no human line.
        let reference = work.join("reference.Ghost.Gbx");
        let prebuilt = map_dir.join("template.Ghost.Gbx");
        let (n, donor_actions, donor_ms) = if prebuilt.exists() {
            if !reference.exists() {
                std::fs::copy(&prebuilt, &reference).map_err(|e| format!("{}: {e}", prebuilt.display()))?;
            }
            let tpl = tmenv::template::Template::load(&reference)?;
            (tpl.facts().ticks, Vec::<Action>::new(), 0i64)
        } else {
            let gdir = tmenv::sanity::ghosts_dir(&map_dir, &work)?;
            let (ghost, donor_ms) = tmenv::sanity::fastest_ghost_in(&gdir)?;
            let tpl = tmenv::template::Template::load(&ghost)?;
            let n = tpl.facts().ticks;
            if !reference.exists() {
                tmenv::template::write_identity_reference(&ghost, &reference)?;
            }
            let d = fk::tape::Tape::load(&ghost.to_string_lossy())?;
            let donor_actions: Vec<Action> =
                (0..d.n()).map(|i| Action { steer: d.steer[i] as i8, gas: d.accel[i] != 0, brake: d.brake[i] != 0 }).collect();
            (n, donor_actions, donor_ms)
        };
        // the archive: this box's file, seeded from the map dir's when present
        let archive = work.join("state-archive.json");
        let seed = map_dir.join("state-archive.json");
        if !archive.exists() && seed.exists() {
            let _ = std::fs::copy(&seed, &archive);
        }
        Ok(MapAssets {
            uid: uid.to_string(),
            map: local_map,
            reference: reference.clone(),
            track,
            ticks: n,
            donor_actions: Arc::new(donor_actions),
            donor_ms,
            archive,
            template_name: reference.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        })
    }
}

impl MapSet {
    pub fn all(&self) -> Vec<(String, Arc<MapAssets>)> {
        self.loaded.lock().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}
