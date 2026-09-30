//! Per-map HUMAN REFERENCE for the tracking reward (BAR M2-3 piece 1): the map's best exact, non-keyboard, train-split
//! ghost from DATA's manifest + shards, resampled by the map's own geometry arc length (every 2 m): position, speed,
//! yaw, race time. Written as `refs/<uid>.ref` (little-endian: magic b"TRF0", n u32, then n × [s f32, x, y, z, speed,
//! yaw, t_ms f32]) and looked up by s at reward time.

use crate::shard::{read_shard, Record};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use tmstate::TrackGeom;

pub const STEP_M: f32 = 2.0;

#[derive(Clone, Debug)]
pub struct RefTrack {
    pub s: Vec<f32>,
    pub pos: Vec<[f32; 3]>,
    pub speed: Vec<f32>,
    pub yaw: Vec<f32>,
    pub t_ms: Vec<f32>,
}

/// Yaw (heading in the horizontal plane) from the body quaternion (w,x,y,z): the car's forward axis is +z in its
/// frame; yaw = atan2(fwd.x, fwd.z) in world.
pub fn yaw_of(quat: [f32; 4]) -> f32 {
    let [w, x, y, z] = quat;
    // rotate (0,0,1) by q
    let fx = 2.0 * (x * z + w * y);
    let fz = 1.0 - 2.0 * (x * x + y * y);
    fx.atan2(fz)
}

impl RefTrack {
    /// Build from one ghost's per-tick records (sorted by tick) against the geometry.
    pub fn from_records(g: &TrackGeom, recs: &[Record]) -> RefTrack {
        let n = (g.length() / STEP_M).ceil() as usize + 1;
        let mut s = Vec::with_capacity(n);
        let mut pos = Vec::with_capacity(n);
        let mut speed = Vec::with_capacity(n);
        let mut yaw = Vec::with_capacity(n);
        let mut t_ms = Vec::with_capacity(n);
        // Per record: its s (probe with its own cps), then fill the grid by nearest-s (records are dense: 10 ms).
        let mut by_s: Vec<(f32, &Record)> = recs
            .iter()
            .filter(|r| r.state.race_ms >= 0)
            .map(|r| {
                let mut st = r.state.clone();
                st.cps = r.state.cps;
                (tmobs::probe(g, &st).s, r)
            })
            .collect();
        by_s.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut j = 0usize;
        for i in 0..n {
            let target = i as f32 * STEP_M;
            while j + 1 < by_s.len() && (by_s[j + 1].0 - target).abs() <= (by_s[j].0 - target).abs() {
                j += 1;
            }
            if by_s.is_empty() {
                break;
            }
            let (rs, r) = by_s[j];
            if (rs - target).abs() > 10.0 * STEP_M {
                continue; // a gap the human never covered at this s (e.g. a cut); the lookup interpolates over it
            }
            s.push(target);
            pos.push(r.state.pos);
            let v = r.state.vel;
            speed.push(if r.state.speed.is_finite() { r.state.speed } else { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() });
            yaw.push(if r.state.quat.iter().all(|q| q.is_finite()) { yaw_of(r.state.quat) } else { v[0].atan2(v[2]) });
            t_ms.push(r.state.race_ms as f32);
        }
        RefTrack { s, pos, speed, yaw, t_ms }
    }

    /// Nearest sample at arc length `s` (None outside the covered range).
    pub fn at(&self, s: f32) -> Option<(usize, [f32; 3], f32, f32, f32)> {
        if self.s.is_empty() {
            return None;
        }
        let i = match self.s.binary_search_by(|x| x.partial_cmp(&s).unwrap()) {
            Ok(i) => i,
            Err(i) => {
                if i == 0 {
                    0
                } else if i >= self.s.len() {
                    self.s.len() - 1
                } else if (self.s[i] - s).abs() < (s - self.s[i - 1]).abs() {
                    i
                } else {
                    i - 1
                }
            }
        };
        if (self.s[i] - s).abs() > 3.0 * STEP_M {
            return None;
        }
        Some((i, self.pos[i], self.speed[i], self.yaw[i], self.t_ms[i]))
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let mut out = Vec::with_capacity(16 + self.s.len() * 28);
        out.extend_from_slice(b"TRF0");
        out.extend_from_slice(&(self.s.len() as u32).to_le_bytes());
        for i in 0..self.s.len() {
            for v in [self.s[i], self.pos[i][0], self.pos[i][1], self.pos[i][2], self.speed[i], self.yaw[i], self.t_ms[i]] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        std::fs::File::create(path).and_then(|mut f| f.write_all(&out)).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn read(path: &Path) -> Result<RefTrack, String> {
        let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if b.len() < 8 || &b[..4] != b"TRF0" {
            return Err(format!("{}: not a TRF0 file", path.display()));
        }
        let n = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
        if b.len() != 8 + n * 28 {
            return Err(format!("{}: {} bytes for {n} samples", path.display(), b.len()));
        }
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let mut t = RefTrack { s: vec![], pos: vec![], speed: vec![], yaw: vec![], t_ms: vec![] };
        for i in 0..n {
            let o = 8 + i * 28;
            t.s.push(f(o));
            t.pos.push([f(o + 4), f(o + 8), f(o + 12)]);
            t.speed.push(f(o + 16));
            t.yaw.push(f(o + 20));
            t.t_ms.push(f(o + 24));
        }
        Ok(t)
    }
}

/// Tracking term for one car state against the reference at the car's s: exp(−(Δlat/σp)² − (Δv/σv)² − (Δyaw/σy)²),
/// in [0, 1]; None when the reference does not cover this s.
pub fn tracking_term(r: &RefTrack, s: f32, pos: [f32; 3], speed: f32, yaw: f32, sig: (f32, f32, f32)) -> Option<f32> {
    let (_, rp, rv, ry, _) = r.at(s)?;
    let d = ((pos[0] - rp[0]).powi(2) + (pos[2] - rp[2]).powi(2)).sqrt();
    let dv = speed - rv;
    let mut dy = yaw - ry;
    while dy > std::f32::consts::PI {
        dy -= 2.0 * std::f32::consts::PI;
    }
    while dy < -std::f32::consts::PI {
        dy += 2.0 * std::f32::consts::PI;
    }
    Some((-(d / sig.0).powi(2) - (dv / sig.1).powi(2) - (dy / sig.2).powi(2)).exp())
}

/// `tmrl refs`: build every map's reference from the manifest + shards. Picks, per map, the lowest-rank ghost with
/// resim_verdict "exact", keyboard false, split train (the held-out maps get one too — they are only used for
/// diagnostics there, never for training).
pub fn build_all(manifest: &str, shards_dir: &str, geom_dir: &str, out_dir: &str, only_maps: &[String]) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{manifest}: {e}"))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty manifest")?.split('\t').collect();
    let col = |name: &str| header.iter().position(|h| *h == name).ok_or_else(|| format!("manifest has no column {name}"));
    let (c_gid, c_uid, c_rank, c_verdict, c_kb, c_shard) = (col("ghost_id")?, col("map_uid")?, col("rank")?, col("resim_verdict")?, col("keyboard")?, col("shard")?);
    // map uid -> (rank, ghost_id, shard file)
    let mut pick: BTreeMap<String, (u32, u32, String)> = BTreeMap::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() <= c_shard {
            continue;
        }
        if f[c_verdict] != "exact" || f[c_kb] != "false" {
            continue;
        }
        let uid = f[c_uid].to_string();
        if !only_maps.is_empty() && !only_maps.contains(&uid) {
            continue;
        }
        let rank: u32 = f[c_rank].parse().unwrap_or(u32::MAX);
        let gid: u32 = f[c_gid].parse().unwrap_or(u32::MAX);
        // DATA's manifest header is two columns short of its rows (2026-09-07); the shard file is the field ending in ".tmd".
        let shard_file = f.iter().rev().find(|x| x.ends_with(".tmd")).map(|x| x.to_string()).unwrap_or_else(|| f[c_shard].to_string());
        let e = pick.entry(uid).or_insert((u32::MAX, 0, String::new()));
        if rank < e.0 {
            *e = (rank, gid, shard_file);
        }
    }
    println!("{} maps have an exact non-keyboard ghost", pick.len());
    // Group by shard so each shard is read once.
    let mut by_shard: BTreeMap<String, Vec<(String, u32)>> = BTreeMap::new();
    for (uid, (_, gid, sh)) in &pick {
        by_shard.entry(sh.clone()).or_default().push((uid.clone(), *gid));
    }
    let mut n_ok = 0;
    for (sh, maps) in &by_shard {
        let path = Path::new(shards_dir).join(sh);
        let recs = match read_shard(path.to_str().unwrap()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("WARNING {}: {e}", path.display());
                continue;
            }
        };
        for (uid, gid) in maps {
            let mut mine: Vec<Record> = recs.iter().filter(|r| &r.map_uid == uid && r.ghost_id == *gid).cloned().collect();
            if mine.is_empty() {
                eprintln!("WARNING {uid}: ghost {gid} has no records in {sh}");
                continue;
            }
            mine.sort_by_key(|r| r.tick);
            let gp = Path::new(geom_dir).join(uid).join("geom.json");
            let g = match crate::bc::load_geoms(&[gp.to_string_lossy().into_owned()]) {
                Ok(m) => match m.into_values().next() {
                    Some(g) => g,
                    None => continue,
                },
                Err(e) => {
                    eprintln!("WARNING {uid}: {e}");
                    continue;
                }
            };
            let rt = RefTrack::from_records(&g, &mine);
            // Control: coverage and lateral sanity.
            let cover = rt.s.len() as f32 * STEP_M / g.length().max(1.0);
            let mut inside = 0usize;
            for (i, p) in rt.pos.iter().enumerate() {
                let mut st = tmstate::CarState::unknown();
                st.pos = *p;
                st.cps = g.gates.iter().filter(|gt| gt.s < rt.s[i]).count() as u8;
                let pr = tmobs::probe(&g, &st);
                if pr.lateral.abs() <= pr.half_width {
                    inside += 1;
                }
            }
            let frac_in = inside as f32 / rt.pos.len().max(1) as f32;
            rt.write(&Path::new(out_dir).join(format!("{uid}.ref")))?;
            println!("{uid}: ghost {gid} ({} records) → {} samples, coverage {:.2}, inside corridor {:.3}, human time {:.3} s", mine.len(), rt.s.len(), cover, frac_in, rt.t_ms.last().copied().unwrap_or(0.0) / 1000.0);
            n_ok += 1;
        }
    }
    println!("wrote {n_ok} references to {out_dir}");
    Ok(())
}
