//! The policy the worker runs. ONE forward implementation is the rule
//! (ROLLOUT-WORKER.md §4): the `.tmw` loader, the bcnet categorical head and
//! `bcnet::sample_chunk` are LEARN's, in `tmrl`; until tmrl's library split
//! lands this module has the trait and a deterministic stand-in so the worker,
//! the protocol and the controls are built and proven now. Swapping the
//! stand-in for `tmrl::bcnet` is one `impl`.

use tmstate::Action;

/// The steer grid of the keyboard-legal categorical head (bins = 3).
pub const STEER_BINS_3: [i8; 3] = [-127, 0, 127];

pub trait Policy: Send + Sync {
    /// Sample one CHUNK of `k` per-tick actions from `obs`; returns the actions,
    /// their bin choices `[steer_bin, gas, brake]` per tick (what the master
    /// recomputes the log-prob from) and the chunk log-prob under this policy.
    /// `temperature` 0 = argmax.
    fn sample_chunk(&self, obs: &[f32], k: usize, temperature: f32, rng: &mut Rng) -> (Vec<Action>, Vec<[u8; 3]>, f32);
    /// The value estimate.
    fn value(&self, obs: &[f32]) -> f32;
    fn obs_version(&self) -> u32;
    fn obs_dim(&self) -> usize;
    /// The chunk length the policy was trained for, when it has one.
    fn chunk_k(&self) -> Option<usize> {
        None
    }
}

/// A deterministic stand-in: per-tick categorical logits from a hash of the
/// observation and the tick index, so episodes are as sensitive to every
/// observation byte as a real policy's and the byte-identity control has
/// teeth. Not a policy anyone should train on.
pub struct HashPolicy {
    pub obs_version: u32,
    pub obs_dim: usize,
}

fn hash_obs(obs: &[f32]) -> u64 {
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
    for v in obs {
        h ^= v.to_bits() as u64;
        h = h.wrapping_mul(0x100_0000_01B3).rotate_left(17);
    }
    h
}

fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

impl Policy for HashPolicy {
    fn sample_chunk(&self, obs: &[f32], k: usize, temperature: f32, rng: &mut Rng) -> (Vec<Action>, Vec<[u8; 3]>, f32) {
        let h = hash_obs(obs);
        let mut acts = Vec::with_capacity(k);
        let mut bins = Vec::with_capacity(k);
        let mut logp = 0.0f32;
        for t in 0..k {
            let ht = h.wrapping_add((t as u64 + 1).wrapping_mul(0x9E37_79B9)).rotate_left(13);
            // steer: 3 logits, biased to straight so the car goes somewhere
            let mut ls = [unit(ht) * 2.0 - 1.0, unit(ht.rotate_left(7)) * 2.0 + 1.0, unit(ht.rotate_left(29)) * 2.0 - 1.0];
            let steer = pick(&mut ls, temperature, rng, &mut logp);
            // gas: mostly on; brake: mostly off
            let mut lg = [unit(ht.rotate_left(41)) * 2.0 - 1.5, unit(ht.rotate_left(47)) * 2.0 + 1.0];
            let gas = pick(&mut lg, temperature, rng, &mut logp);
            let mut lb = [unit(ht.rotate_left(53)) * 2.0 + 1.5, unit(ht.rotate_left(59)) * 2.0 - 1.5];
            let brake = pick(&mut lb, temperature, rng, &mut logp);
            acts.push(Action { steer: STEER_BINS_3[steer], gas: gas == 1, brake: brake == 1 });
            bins.push([steer as u8, gas as u8, brake as u8]);
        }
        (acts, bins, logp)
    }
    fn value(&self, obs: &[f32]) -> f32 {
        ((hash_obs(obs) >> 20) as u32 as f32 / u32::MAX as f32) - 0.5
    }
    fn obs_version(&self) -> u32 { self.obs_version }
    fn obs_dim(&self) -> usize { self.obs_dim }
}

/// One categorical draw (temperature 0 = argmax); adds its log-prob to `logp`.
pub fn pick(logits: &mut [f32], temperature: f32, rng: &mut Rng, logp: &mut f32) -> usize {
    if temperature <= 0.0 {
        let i = logits.iter().enumerate().fold((0usize, f32::NEG_INFINITY), |b, (i, v)| if *v > b.1 { (i, *v) } else { b }).0;
        softmax(logits);
        *logp += logits[i].max(1e-8).ln();
        return i;
    }
    if temperature != 1.0 {
        for l in logits.iter_mut() {
            *l /= temperature;
        }
    }
    softmax(logits);
    let i = sample(logits, rng.f32());
    *logp += logits[i].max(1e-8).ln();
    i
}

/// The `.tmw` header is public (INTERFACES.md §Trained-policy artefact):
/// `TMW0 | file_version | OBS_VERSION | OBS_DIM | ...`. Read only what the
/// worker needs to REFUSE a policy it cannot observe with; the tensors are
/// LEARN's loader's.
pub fn tmw_header(b: &[u8]) -> Result<(u32, u32), String> {
    if b.len() < 16 || &b[0..4] != b"TMW0" {
        return Err("not a policy.tmw (bad magic)".into());
    }
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    if u(4) != 1 {
        return Err(format!("policy.tmw file version {}, this worker reads 1", u(4)));
    }
    Ok((u(8), u(12)))
}

/// Build the worker's policy from `SetPolicy` bytes. Empty bytes = the hash
/// stand-in for the given observation version (controls, bring-up). A real
/// `.tmw` is refused until the tmrl library split lands (§4).
pub fn load_policy(tmw: &[u8], obs_version: u32) -> Result<Box<dyn Policy>, String> {
    if !(1..=4).contains(&obs_version) {
        return Err(format!("unknown obs version {obs_version}"));
    }
    if tmw.is_empty() {
        return Ok(Box::new(HashPolicy { obs_version, obs_dim: tmobs::obs_dim(obs_version) }));
    }
    let (ov, od) = tmw_header(tmw)?;
    if !(1..=3).contains(&ov) {
        return Err(format!("policy observes with unknown version {ov}"));
    }
    if od as usize != tmobs::obs_dim(ov) {
        return Err(format!("policy.tmw OBS_DIM {od} but version {ov} observes {} floats", tmobs::obs_dim(ov)));
    }
    // LEARN's loader reads a path: the bytes go through a private file
    let dir = std::env::temp_dir().join(format!("tmroll-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(format!("policy-{:x}.tmw", tmw.len() as u64 ^ tmw.iter().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(*b as u64))));
    std::fs::write(&p, tmw).map_err(|e| e.to_string())?;
    let (w, ov2) = tmrl::policy::read_any(p.to_str().unwrap())?;
    let _ = std::fs::remove_file(&p);
    if ov2 != ov {
        return Err(format!("policy.tmw: header says obs version {ov}, loader {ov2}"));
    }
    Ok(Box::new(TmwPolicy { w, obs_version: ov }))
}

/// `tmrl`'s rollout RNG and sampler, byte for byte (ppo_train.rs `Rng`,
/// net.rs `softmax`/`sample`) -- the byte-identity control depends on the
/// worker drawing the same numbers the master would.
pub struct Rng(pub u64);
impl Rng {
    pub fn f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / ((1u32 << 24) as f32)
    }
}

pub fn softmax(logits: &mut [f32]) {
    let m = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut s = 0.0;
    for l in logits.iter_mut() {
        *l = (*l - m).exp();
        s += *l;
    }
    for l in logits.iter_mut() {
        *l /= s;
    }
}

pub fn sample(p: &[f32], u: f32) -> usize {
    let mut acc = 0.0;
    for (i, q) in p.iter().enumerate() {
        acc += q;
        if u < acc {
            return i;
        }
    }
    p.len() - 1
}

/// LEARN's policy: the `.tmw` weights, `Weights::forward`, `sample_chunk` --
/// the one forward the master trains (ROLLOUT-WORKER.md §4).
pub struct TmwPolicy {
    pub w: tmrl::Weights,
    pub obs_version: u32,
}

impl Policy for TmwPolicy {
    fn sample_chunk(&self, obs: &[f32], k: usize, temperature: f32, rng: &mut Rng) -> (Vec<Action>, Vec<[u8; 3]>, f32) {
        debug_assert_eq!(k, self.w.shape.k);
        let raw = self.w.forward(obs);
        let mut trng = tmrl::Rng(rng.0);
        let (acts, ch, logp) = if temperature <= 0.0 {
            let (acts, _v) = self.w.decode(&raw);
            let bins: Vec<u32> = acts.iter().map(|a| tmrl::bin_centres(self.w.shape.bins).iter().position(|c| *c == a.steer).unwrap_or(0) as u32).collect();
            let ch = tmrl::Chunk { bins, gas: acts.iter().map(|a| a.gas as u8 as f32).collect(), brake: acts.iter().map(|a| a.brake as u8 as f32).collect() };
            (acts, ch, 0.0)
        } else {
            tmrl::sample_chunk(&self.w, &raw, temperature, &mut trng)
        };
        rng.0 = trng.0;
        let bins: Vec<[u8; 3]> = (0..acts.len()).map(|i| [ch.bins[i] as u8, (ch.gas[i] > 0.5) as u8, (ch.brake[i] > 0.5) as u8]).collect();
        (acts, bins, logp)
    }
    fn value(&self, obs: &[f32]) -> f32 {
        let raw = self.w.forward(obs);
        let (_, _, _, vo) = self.w.shape.offsets();
        raw[vo]
    }
    fn obs_version(&self) -> u32 { self.obs_version }
    fn obs_dim(&self) -> usize { self.w.shape.obs_dim }
    fn chunk_k(&self) -> Option<usize> { Some(self.w.shape.k) }
}
