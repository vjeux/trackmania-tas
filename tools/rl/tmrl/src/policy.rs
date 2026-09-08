//! `policy.tmw` — the trained-policy artefact (INTERFACES.md §Trained-policy artefact). LEARN owns it; `tmrl eval`
//! and `tmenv rollout` both load it through this one loader.
//!
//! ```text
//! magic b"TMW0" | file_version u32 = 1 | OBS_VERSION u32 | OBS_DIM u32 | head_kind u32 | steer_bins u32 | chunk_k u32 | hidden u32
//! | n_tensors u32 | per tensor: name_len u32, name, ndim u32, dims u32×ndim, f32 × prod(dims)
//! ```
//! Little-endian throughout. The loader refuses a file whose OBS_VERSION/OBS_DIM are not the ones this binary
//! observes with, and whose tensor shapes do not match the header's network shape — a policy loaded against
//! the wrong observation is a policy that drives on noise.

use crate::bcnet::{tensor_names, HeadKind, Shape, Weights};
use std::fs;

pub const MAGIC: &[u8; 4] = b"TMW0";
pub const FILE_VERSION: u32 = 1;

pub fn write(path: &str, w: &Weights, obs_version: u32) -> Result<(), String> {
    let mut b = Vec::new();
    b.extend_from_slice(MAGIC);
    for x in [FILE_VERSION, obs_version, w.shape.obs_dim as u32, w.shape.head as u32, w.shape.bins as u32, w.shape.k as u32, w.shape.hidden as u32, (2 * w.shape.layers + 2) as u32] {
        b.extend_from_slice(&x.to_le_bytes());
    }
    for (name, (vals, dims)) in tensor_names(w.shape.layers).iter().zip(w.tensors()) {
        b.extend_from_slice(&(name.len() as u32).to_le_bytes());
        b.extend_from_slice(name.as_bytes());
        b.extend_from_slice(&(dims.len() as u32).to_le_bytes());
        for d in &dims {
            b.extend_from_slice(&(*d as u32).to_le_bytes());
        }
        for v in vals {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    fs::write(path, &b).map_err(|e| format!("{path}: {e}"))
}

pub struct Loaded {
    pub weights: Weights,
    pub obs_version: u32,
}

pub fn read(path: &str) -> Result<Loaded, String> {
    let b = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut p: usize;
    let u32_at = |p: &mut usize| -> Result<u32, String> {
        if *p + 4 > b.len() {
            return Err(format!("{path}: truncated at byte {p}"));
        }
        let v = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
        *p += 4;
        Ok(v)
    };
    if b.len() < 4 || &b[..4] != MAGIC {
        return Err(format!("{path}: not a policy.tmw (bad magic)"));
    }
    p = 4;
    let fv = u32_at(&mut p)?;
    if fv != FILE_VERSION {
        return Err(format!("{path}: file version {fv}, this loader is {FILE_VERSION}"));
    }
    let obs_version = u32_at(&mut p)?;
    let obs_dim = u32_at(&mut p)? as usize;
    let head = HeadKind::from_u32(u32_at(&mut p)?).ok_or_else(|| format!("{path}: unknown head kind"))?;
    let bins = u32_at(&mut p)? as usize;
    let k = u32_at(&mut p)? as usize;
    let hidden = u32_at(&mut p)? as usize;
    let n_t = u32_at(&mut p)? as usize;
    // n_t = 2 × hidden layers + 2 (head weight + bias).
    if n_t < 4 || n_t % 2 != 0 {
        return Err(format!("{path}: {n_t} tensors — not 2 × layers + 2"));
    }
    let layers = (n_t - 2) / 2;
    let shape = Shape { obs_dim, hidden, layers, k, head, bins };
    if head == HeadKind::Categorical && (bins < 3 || bins % 2 == 0) {
        return Err(format!("{path}: categorical head with {bins} bins (odd >= 3 required)"));
    }
    let mut tensors = Vec::with_capacity(n_t);
    for want_name in tensor_names(layers).iter() {
        let nl = u32_at(&mut p)? as usize;
        let name = std::str::from_utf8(&b[p..p + nl]).map_err(|e| e.to_string())?.to_string();
        p += nl;
        if name != *want_name {
            return Err(format!("{path}: tensor {name} where {want_name} was expected"));
        }
        let nd = u32_at(&mut p)? as usize;
        let mut dims = Vec::with_capacity(nd);
        for _ in 0..nd {
            dims.push(u32_at(&mut p)? as usize);
        }
        let n: usize = dims.iter().product();
        if p + 4 * n > b.len() {
            return Err(format!("{path}: tensor {name} truncated"));
        }
        let vals: Vec<f32> = b[p..p + 4 * n].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        p += 4 * n;
        tensors.push((dims, vals));
    }
    if p != b.len() {
        return Err(format!("{path}: {} trailing bytes", b.len() - p));
    }
    // Shapes must match the header's network shape.
    let w = Weights::from_tensors(shape, tensors.iter().map(|(_, v)| v.clone()).collect())?;
    for ((dims, _), (_, want)) in tensors.iter().zip(w.tensors()) {
        if *dims != want {
            return Err(format!("{path}: tensor dims {:?} vs header-implied {:?}", dims, want));
        }
    }
    Ok(Loaded { weights: w, obs_version })
}

/// Load and refuse anything this binary cannot observe for.
pub fn read_checked(path: &str, obs_version: u32, obs_dim: usize) -> Result<Weights, String> {
    let l = read(path)?;
    if l.obs_version != obs_version {
        return Err(format!("{path}: OBS_VERSION {} but this binary observes v{obs_version}", l.obs_version));
    }
    if l.weights.shape.obs_dim != obs_dim {
        return Err(format!("{path}: OBS_DIM {} but this binary observes {obs_dim}", l.weights.shape.obs_dim));
    }
    Ok(l.weights)
}

/// Load a policy of ANY observation version this binary can produce (tmobs v1..=v3), returning the weights and the
/// version the env must be configured with (`CoreCfg.obs_version`). Refuses a dim/version mismatch.
pub fn read_any(path: &str) -> Result<(Weights, u32), String> {
    let l = read(path)?;
    if !(1..=4).contains(&l.obs_version) {
        return Err(format!("{path}: OBS_VERSION {} is not one this binary observes (1..=4)", l.obs_version));
    }
    let want = tmobs::obs_dim(l.obs_version);
    if l.weights.shape.obs_dim != want {
        return Err(format!("{path}: OBS_DIM {} but OBS_VERSION {} observes {want}", l.weights.shape.obs_dim, l.obs_version));
    }
    Ok((l.weights, l.obs_version))
}
