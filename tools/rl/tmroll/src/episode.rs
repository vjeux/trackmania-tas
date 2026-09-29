//! One episode, the way `tmrl ppo` runs it (ppo_train.rs `run_episode`, field
//! for field, with the bcnet CHUNK head: one policy draw = k per-tick actions),
//! from a root or an archive state, into an `EpisodeOut`. The worker and the
//! local half of the control call this one function. Also the human-line
//! seeding of a map's archive (LEARN's ppo_bc `seed_human_archive`).
//!
//! THE ARCHIVE is a per-map STORE shared by every env thread of the box: each
//! state is its action prefix plus metadata under a box-wide `state_id` (the
//! file's order; new snapshots append). An env thread that is asked for a state
//! it has not seen imports the prefix (`ForkEnv::import_snapshot`) and
//! re-materialises it on first use; a snapshot it makes goes to the store with
//! the prefix, so any thread can serve it next. Persisted as JSON
//! (`StateArchive`'s `SavedArchive` shape, `origin` included).

use crate::maps::MapAssets;
use crate::policy::{Policy, Rng};
use tmproto::{ArchiveEntry, EpisodeOut, EpisodeReq, StepOut, FLAG_ROWS, FLAG_TAPE};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tmenv::archive::{SavedArchive, SavedEntry};
use tmenv::forkenv::{build_at_start, RootCfg, StateId};
use tmenv::{ActionSpace, CoreCfg, Done, ForkEnv, Rig};
use tmstate::Action;

/// A map's states, box-wide.
#[derive(Default)]
pub struct MapStore {
    pub entries: Vec<StoredState>,
    pub template: String,
    pub dirty: bool,
}

pub struct StoredState {
    pub meta: ArchiveEntry,
    pub prefix: Vec<Action>,
}

pub type Stores = Arc<Mutex<HashMap<String, MapStore>>>;

impl MapStore {
    pub fn load(path: &Path) -> Result<MapStore, String> {
        let j = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let s: SavedArchive = serde_json::from_str(&j).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut m = MapStore { entries: Vec::new(), template: s.template, dirty: false };
        for (i, e) in s.entries.into_iter().enumerate() {
            m.entries.push(StoredState {
                meta: ArchiveEntry { state_id: i as u64, progress_m: e.progress_m, race_ms: e.race_ms, score: e.score, origin: e.origin },
                prefix: e.prefix.iter().map(|t| Action { steer: t[0], gas: t[1] != 0, brake: t[2] != 0 }).collect(),
            });
        }
        Ok(m)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let s = SavedArchive {
            bucket_m: 10.0,
            per_bucket: usize::MAX,
            template: self.template.clone(),
            entries: self
                .entries
                .iter()
                .map(|e| SavedEntry {
                    progress_m: e.meta.progress_m,
                    race_ms: e.meta.race_ms,
                    score: e.meta.score,
                    origin: e.meta.origin,
                    prefix: e.prefix.iter().map(|a| [a.steer, a.gas as i8, a.brake as i8]).collect(),
                })
                .collect(),
        };
        let j = serde_json::to_string(&s).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, j).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
    pub fn push(&mut self, meta: ArchiveEntry, prefix: Vec<Action>) -> u64 {
        let id = self.entries.len() as u64;
        self.entries.push(StoredState { meta: ArchiveEntry { state_id: id, ..meta }, prefix });
        self.dirty = true;
        id
    }
}

/// A worker thread's env for ONE map (rebuilt when the map, k, obs version or margin changes).
pub struct MapEnv {
    pub uid: String,
    pub k_ticks: u16,
    pub obs_version: u32,
    pub margin_m: f32,
    pub env: ForkEnv,
    pub rig: Rig,
    pub tape: fk::tape::Tape,
    pub length_m: f32,
    pub template_name: String,
    /// Store state ids this env has imported or made → its own snapshot ids.
    pub known: HashMap<u64, StateId>,
    pub archive_path: PathBuf,
}

impl MapEnv {
    pub fn build(server: &Path, shim: &Path, work: &Path, a: &MapAssets, k_ticks: u16, obs_version: u32, margin_m: f32) -> Result<MapEnv, String> {
        let cfg = CoreCfg {
            k_ticks: k_ticks as usize,
            obs_version,
            max_ticks: a.ticks.saturating_sub(1).max(100),
            offroute_margin: margin_m,
            ..Default::default()
        };
        let root = RootCfg { require_start: Some((a.track.geom.spawn, 6.0, 4.0)), verbose: false, ..Default::default() };
        let (mut env, rig, tape) = build_at_start(server, &a.map, shim, work, &a.reference, a.track.clone(), ActionSpace::default(), cfg, &root)?;
        // 96 envs a box: a small pin budget each (a pinned node is a paused engine
        // process); an evicted state is re-materialised from its prefix on use
        env.set_pin_budget(8);
        Ok(MapEnv {
            uid: a.uid.clone(),
            k_ticks,
            obs_version,
            margin_m,
            length_m: a.track.geom.length(),
            env,
            rig,
            tape,
            template_name: a.template_name.clone(),
            known: HashMap::new(),
            archive_path: a.archive.clone(),
        })
    }

    /// The env's snapshot for a store state, importing the prefix on first use.
    fn state_for(&mut self, stores: &Stores, state_id: u64) -> Result<StateId, String> {
        if let Some(id) = self.known.get(&state_id) {
            return Ok(*id);
        }
        let prefix = {
            let s = stores.lock().unwrap();
            let m = s.get(&self.uid).ok_or_else(|| format!("map {} has no archive", self.uid))?;
            if m.template != self.template_name {
                return Err(format!("map {}: archive driven in template {:?}, this env runs {:?}", self.uid, m.template, self.template_name));
            }
            m.entries.get(state_id as usize).ok_or_else(|| format!("map {} has no archive state {state_id}", self.uid))?.prefix.clone()
        };
        let id = self.env.import_snapshot(prefix)?;
        self.known.insert(state_id, id);
        Ok(id)
    }

    /// Snapshot the current state into the store (origin 0 human line, 1 policy).
    fn snapshot_to_store(&mut self, stores: &Stores, progress_m: f32, race_ms: i32, origin: u8) -> u64 {
        let id = self.env.snapshot();
        let prefix = self.env.snapshot_prefix(&id).map(|p| p.to_vec()).unwrap_or_default();
        let score = if race_ms > 0 { progress_m / (race_ms as f32 / 1000.0) } else { 0.0 };
        let mut s = stores.lock().unwrap();
        let m = s.entry(self.uid.clone()).or_insert_with(|| MapStore { template: self.template_name.clone(), ..Default::default() });
        let sid = m.push(ArchiveEntry { state_id: 0, progress_m, race_ms, score, origin }, prefix);
        self.known.insert(sid, id);
        sid
    }

    /// LEARN's `seed_human_archive`: the donor's tape through this env with a
    /// snapshot every `snap_every` ticks while the car moves (> 5 m/s). The
    /// template IS the donor's container (identity reference), so index shift 0.
    pub fn seed_human_line(&mut self, stores: &Stores, acts: &[Action], snap_every: usize) -> Result<usize, String> {
        let k = self.k_ticks.max(1) as usize;
        self.env.reset()?;
        let mut n = 0usize;
        let mut since = 0usize;
        let mut steps = 0usize;
        let stop = Action { steer: 0, gas: false, brake: true };
        loop {
            let base = self.env.next_tick()?;
            let chunk: Vec<Action> = (0..k).map(|j| acts.get(base + j).copied().unwrap_or(stop)).collect();
            let (_, _, d, info) = self.env.step_ticks(&chunk)?;
            steps += 1;
            since += k;
            if d.is_some() || steps > acts.len() / k + 50 {
                break;
            }
            if since >= snap_every && info.speed > 5.0 {
                since = 0;
                self.snapshot_to_store(stores, info.best_s, info.state.race_ms, 0);
                n += 1;
            }
        }
        self.env.reset()?;
        Ok(n)
    }
}

/// Write a map's store to its file when it changed.
pub fn save_store(stores: &Stores, uid: &str, path: &Path) -> Result<(), String> {
    let mut s = stores.lock().unwrap();
    if let Some(m) = s.get_mut(uid) {
        if m.dirty {
            m.save(path)?;
            m.dirty = false;
        }
    }
    Ok(())
}

pub fn store_entries(stores: &Stores, uid: &str) -> Vec<ArchiveEntry> {
    let s = stores.lock().unwrap();
    s.get(uid).map(|m| m.entries.iter().map(|e| e.meta.clone()).collect()).unwrap_or_default()
}

fn done_code(d: Option<Done>) -> u8 {
    match d {
        None => 0,
        Some(Done::Finished) => 1,
        Some(Done::OffRoute) => 2,
        Some(Done::NoProgress) => 3,
        Some(Done::RunEnded) => 5,
        Some(Done::TickCap) => 6,
    }
}

pub fn run_episode(me: &mut MapEnv, stores: &Stores, policy: &dyn Policy, req: &EpisodeReq, batch_id: u64, policy_id: u64) -> Result<EpisodeOut, String> {
    let t0 = std::time::Instant::now();
    let mut rng = Rng(req.seed | 1);
    let k = me.k_ticks.max(1) as usize;
    let mut obs = if req.start == 1 {
        let id = me.state_for(stores, req.state_id)?;
        me.env.reset_to(&id)?
    } else {
        me.env.reset()?
    };
    let mut steps: Vec<StepOut> = Vec::with_capacity(req.max_steps as usize);
    let mut done: Option<Done> = None;
    let mut reward_sum = 0.0f32;
    let mut gates = 0u32;
    let mut best_s = 0.0f32;
    let mut finish_ms = -1i32;
    let mut since_snap = 0usize;
    for _ in 0..req.max_steps {
        let (acts, bins, logp) = policy.sample_chunk(&obs, k, req.temperature, &mut rng);
        let value = policy.value(&obs);
        let (next_obs, reward, d, info) = me.env.step_ticks(&acts)?;
        let next_value = policy.value(&next_obs);
        let terminal = matches!(d, Some(Done::Finished) | Some(Done::OffRoute) | Some(Done::NoProgress));
        let truncated = matches!(d, Some(Done::TickCap));
        reward_sum += reward;
        gates = info.gates as u32;
        best_s = info.best_s;
        if let Some(f) = me.env.finish() {
            finish_ms = f as i32;
        }
        steps.push(StepOut { obs: std::mem::replace(&mut obs, next_obs), action: bins, logp, value, reward, terminal, truncated, next_value });
        since_snap += k;
        if req.snap_every > 0 && d.is_none() && since_snap >= req.snap_every as usize && info.speed > 5.0 {
            since_snap = 0;
            me.snapshot_to_store(stores, info.best_s, info.state.race_ms, 1);
        }
        if let Some(d) = d {
            done = Some(d);
            break;
        }
    }
    let mut truncated = false;
    if done.is_none() {
        if let Some(l) = steps.last_mut() {
            l.truncated = true;
        }
        truncated = true;
    }
    let mut rows = Vec::new();
    if req.flags & FLAG_ROWS != 0 {
        let tr = me.env.trace_rows();
        for (i, r) in tr.iter().enumerate() {
            let st = tmenv::core::row_state(r, if i > 0 { Some(&tr[i - 1]) } else { None }, gates.min(255) as u8, finish_ms >= 0);
            rows.extend_from_slice(&st.to_bytes());
        }
    }
    let mut tape = Vec::new();
    if req.flags & FLAG_TAPE != 0 {
        let (s, g, b) = me.env.banked_tape(&me.tape);
        let out = me.rig.engine.work.join(format!("ep-{}.Ghost.Gbx", req.ep_id));
        me.tape.write_candidate(&s, &g, &b, &out)?;
        tape = std::fs::read(&out).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&out);
    }
    Ok(EpisodeOut {
        ep_id: req.ep_id,
        batch_id,
        policy_id,
        map_uid: me.uid.clone(),
        seed: req.seed,
        start: req.start,
        state_id: req.state_id,
        obs_version: me.obs_version,
        obs_dim: policy.obs_dim() as u32,
        k_ticks: me.k_ticks,
        done: done_code(done),
        truncated,
        finish_ms,
        gates,
        reward_sum,
        best_s,
        length_m: me.length_m,
        wall_ms: t0.elapsed().as_millis() as u32,
        worker_ticks: me.env.trace_rows().len() as u32,
        steps,
        rows,
        tape,
    })
}
