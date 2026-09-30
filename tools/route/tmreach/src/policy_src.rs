//! Policy proposals for `tmreach lap` (MODEL arm, coordinator 2026-09-08 22:00Z): the player's per-map
//! tmrl policy rolled forward in closed loop as ONE more macro type beside the 59 open-loop macros and
//! the FOLLOW macros — "diversity, not processes" for the long tiny maps.
//!
//! The loop mirrors `tmenv`: engine row → `CarState` (`tmr::FromRow`) → `tmobs::observe_version(v)` →
//! `tmrl::bcnet::Weights::forward` → categorical steer (21 bins) / Bernoulli pedals sampled at temperature T
//! (`sample`) or the mode (`decode`, T = 0) → k ticks of inputs (`shape.k`, 10) → the fork worker steps them
//! and hands back the rows → repeat until h ticks. Inside the worker, no separate process.
//!
//! Interface (behind flags; GEN owns lap.rs): `--policy P.tmw --policy-geom geom.json [--obs-version V]
//! [--policy-temp T] [--policy-n N]`. V defaults to the version stored in the .tmw; T = 0.6; N = 3 proposals
//! per fan (independent samples; with T = 0 one is enough).

use crate::tmr::FromRow;
use forkoracle::forksrv::Rec;
use forkoracle::layout::Row;
use tmstate::{Action, CarState, TrackGeom};

pub struct PolicySrc {
    pub w: tmrl::bcnet::Weights,
    pub obs_version: u32,
    pub geom: TrackGeom,
    pub temp: f32,
    pub n: usize,
    pub label: String,
}

impl PolicySrc {
    pub fn load(policy: &str, geom: &std::path::Path, obs_version: Option<u32>, temp: f32, n: usize) -> Result<PolicySrc, String> {
        let (w, v_file) = tmrl::policy::read_any(policy)?;
        let v = obs_version.unwrap_or(v_file);
        let want = tmobs::obs_dim(v);
        if want == 0 {
            return Err(format!("OBS_VERSION {v} unknown"));
        }
        if w.shape.obs_dim != want {
            // a policy trained on obs 100 (v2) drives a v3 env: tmrl pads — we do the same by observing at the
            // version whose width matches the net
            let fits = (1..=4).find(|vv| tmobs::obs_dim(*vv) == w.shape.obs_dim);
            match fits {
                Some(vv) if obs_version.is_none() => {
                    let track = tmenv::Track::load_geom_json(geom)?;
                    eprintln!("policy {policy}: obs_dim {} = OBS v{vv} (file says v{v_file}); observing at v{vv}", w.shape.obs_dim);
                    return Ok(PolicySrc { w, obs_version: vv, geom: (*track.geom).clone(), temp, n: n.max(1), label: std::path::Path::new(policy).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() });
                }
                _ => return Err(format!("policy {policy}: obs_dim {} but OBS v{v} has {want} floats", w.shape.obs_dim)),
            }
        }
        let track = tmenv::Track::load_geom_json(geom)?;
        Ok(PolicySrc { w, obs_version: v, geom: (*track.geom).clone(), temp, n: n.max(1), label: std::path::Path::new(policy).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() })
    }

    /// Ticks per decision (the net's action chunk).
    pub fn k(&self) -> usize {
        self.w.shape.k.max(1)
    }

    /// One decision from an engine row: the next k ticks of inputs, and the actions to remember as `prev`.
    pub fn propose(&self, row: &Row, race_ms: i64, cps: u8, prev: &[Action], unit: &mut dyn FnMut() -> f32) -> (Vec<Rec>, Vec<Action>) {
        let st: CarState = CarState::from_row(row, race_ms, cps, false);
        let obs = tmobs::observe_version(self.obs_version, &self.geom, &st, prev);
        let raw = self.w.forward(&obs);
        let acts = if self.temp > 0.0 { self.w.sample(&raw, self.temp, unit) } else { self.w.decode(&raw).0 };
        let recs = acts.iter().map(|a| Rec { steer: a.steer as f32 / 127.0, gas: a.gas as u8 as f32, brake: a.brake as u8 as f32 }).collect();
        (recs, acts)
    }
}

/// Keep the last `tmobs::N_PREV` actions, most recent LAST (tmobs reverses when it reads them).
pub fn push_prev(prev: &mut Vec<Action>, acts: &[Action]) {
    prev.extend_from_slice(acts);
    let keep = tmobs::N_PREV.max(1);
    if prev.len() > keep {
        let cut = prev.len() - keep;
        prev.drain(..cut);
    }
}
