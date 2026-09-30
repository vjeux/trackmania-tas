//! The per-tick inputs a candidate is made of, and the operators that change
//! them.
//!
//! TM2020 admits steering in `-127..=127` only -- 255 values, not the +-65536
//! analog range TMNF/TMInterface exposes -- and gas and brake are one bit each.
//! The engine reads one such triple per 10 ms tick.

/// A deterministic RNG, seeded per worker so a run is reproducible.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).max(1))
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0; // xorshift64*
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
    #[inline]
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            lo
        } else {
            lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i64
        }
    }
    #[inline]
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    #[inline]
    pub fn sign(&mut self) -> f64 {
        if self.next_u64() & 1 == 0 {
            -1.0
        } else {
            1.0
        }
    }
}

/// One candidate's inputs, one entry per 10 ms tick.
#[derive(Clone, PartialEq)]
pub struct Inputs {
    pub steer: Vec<i8>,
    pub gas: Vec<bool>,
    pub brake: Vec<bool>,
    /// The RESPAWN channel per tick (the tape's state-literal bit 31). Empty = no respawns (every older constructor).
    /// Carried by every candidate so a fork evaluation respawns where the tape does (2026-09-09: without it every
    /// candidate whose suffix held a respawn DNF'd in the fork -- Norway: 750k candidates, 0 finishers).

    pub respawn: Vec<bool>,
}

impl Inputs {
    pub fn len(&self) -> usize {
        self.steer.len()
    }
    pub fn is_empty(&self) -> bool {
        self.steer.is_empty()
    }

    /// The three arrays as the fork protocol and the tape writer want them:
    /// steering as the raw byte, pedals as 0/1.
    pub fn steer_u8(&self) -> Vec<u8> {
        self.steer.iter().map(|&v| v as u8).collect()
    }
    pub fn gas_u8(&self) -> Vec<u8> {
        self.gas.iter().map(|&v| v as u8).collect()
    }
    pub fn brake_u8(&self) -> Vec<u8> {
        (0..self.brake.len()).map(|t| self.brake_byte(t)).collect()
    }

    /// From the three arrays a decoded tape hands over.
    pub fn from_arrays(steer: &[u8], gas: &[u8], brake: &[u8]) -> Inputs {
        Inputs {
            steer: steer.iter().map(|&v| v as i8).collect(),
            gas: gas.iter().map(|&v| v != 0).collect(),
            brake: brake.iter().map(|&v| v & 1 != 0).collect(),
            respawn: brake.iter().map(|&v| v & 2 != 0).collect(),
        }
    }

    /// The brake byte the engine-side record builders take: bit 0 brake, bit 1 respawn (`rec_of` decodes it).
    pub fn brake_byte(&self, t: usize) -> u8 {
        (self.brake[t] as u8) | ((self.respawn.get(t).copied().unwrap_or(false) as u8) << 1)
    }

    /// How far this tape is from the tape a fork server checkpointed on.
    ///
    /// **The fork oracle is not trustworthy far from its reference** -- 0 of
    /// 312 fork-reported finishes survived a plain re-validation when the tape
    /// was not a small, late perturbation of the reference. So every result the
    /// search reports carries this, and a reader can see for themselves which
    /// regime it came from.
    pub fn distance_from(&self, reference: &Inputs) -> Distance {
        let n = self.len().min(reference.len());
        let mut first = None;
        let mut diff = 0usize;
        let mut max_steer = 0i32;
        for t in 0..n {
            let d = self.steer[t] != reference.steer[t]
                || self.gas[t] != reference.gas[t]
                || self.brake[t] != reference.brake[t];
            if d {
                if first.is_none() {
                    first = Some(t);
                }
                diff += 1;
                max_steer = max_steer.max((self.steer[t] as i32 - reference.steer[t] as i32).abs());
            }
        }
        Distance { first_diff_tick: first, diff_ticks: diff, ticks: n, max_steer_delta: max_steer }
    }
}

/// How far a candidate sits from the reference its evaluation was taken near.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Distance {
    pub first_diff_tick: Option<usize>,
    pub diff_ticks: usize,
    pub ticks: usize,
    pub max_steer_delta: i32,
}

impl std::fmt::Display for Distance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.first_diff_tick {
            None => write!(f, "identical to the reference"),
            Some(t) => write!(
                f,
                "{} of {} ticks differ, first at tick {}, largest steer move {}",
                self.diff_ticks, self.ticks, t, self.max_steer_delta
            ),
        }
    }
}

#[inline]
fn clamp_i8(v: f64) -> i8 {
    v.round().clamp(-127.0, 127.0) as i8
}

/// What one mutation did, for the improvement log and for `tmsearch analyze`.
#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    pub kind: &'static str,
    pub at: usize,
    pub span: i64,
    pub val: i64,
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{} span={} val={}", self.kind, self.at, self.span, self.val)
    }
}

/// The operator set. Named because naming one is what makes an A/B of the move
/// set possible -- and retuning a stalled search from its own improvement log
/// (tally `kind@tick`, restrict to the productive range) has broken plateaus
/// that four parameter configurations did not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpSet {
    /// The local set: raised-cosine steer, flat analog level, transition shift,
    /// gas/brake window.
    Local,
    /// `Local`, plus the three non-local moves below at 45% total.
    Wide,
    /// One named operator, alone.
    Only(Kind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Doublet,
    Retime,
    Scale,
}

impl std::str::FromStr for OpSet {
    type Err = String;
    fn from_str(s: &str) -> Result<OpSet, String> {
        Ok(match s {
            "local" => OpSet::Local,
            "wide" => OpSet::Wide,
            "doublet" => OpSet::Only(Kind::Doublet),
            "retime" => OpSet::Only(Kind::Retime),
            "scale" => OpSet::Only(Kind::Scale),
            _ => {
                return Err(format!(
                    "unknown operator set {:?}: local | wide | doublet | retime | scale",
                    s
                ))
            }
        })
    }
}

/// A STEERING DOUBLET: +A then -A, so the two lobes cancel.
///
/// A plain raised-cosine bump changes the car's HEADING, and every input after
/// it was tuned for the old heading, so the rest of the run falls apart --
/// measured: a 160-tick bump finishes 1% of the time. A doublet integrates to
/// zero, so it moves the car sideways and hands the tail back a car pointing
/// the same way. That is the difference between editing the plan and
/// invalidating it.
fn doublet(s: &mut Inputs, rng: &mut Rng, lo: usize, hi: usize) -> Op {
    let w = hi - lo;
    let r = rng.range(3, (w / 4).max(4) as i64) as usize;
    let a = lo + r + rng.below(w.saturating_sub(4 * r).max(1));
    let b = a + 2 * r;
    if b + r >= hi {
        return Op { kind: "nop", at: a, span: 0, val: 0 };
    }
    let amp = rng.sign() * rng.range(3, 100) as f64;
    for (c, sign) in [(a, 1.0), (b, -1.0)] {
        for i in c.saturating_sub(r).max(lo)..(c + r).min(hi) {
            let w = 0.5 * (1.0 + (std::f64::consts::PI * (i as f64 - c as f64) / r as f64).cos());
            s.steer[i] = clamp_i8(s.steer[i] as f64 + sign * amp * w);
        }
    }
    Op { kind: "doublet", at: a, span: r as i64, val: amp as i64 }
}

/// RETIME THE TAIL: shift every input from tick `a` onward by `d` ticks.
///
/// The downstream plan is preserved exactly, just executed earlier or later.
/// "Brake a tick sooner and keep everything else" is a move a human TASer makes
/// constantly and one the local operators cannot express at all.
fn retime(s: &mut Inputs, rng: &mut Rng, lo: usize, hi: usize) -> Op {
    let n = s.len();
    let a = lo + rng.below((hi - lo).max(1));
    let mut d = rng.range(-4, 4);
    if d == 0 {
        d = 1;
    }
    let (st, ga, br) = (s.steer.clone(), s.gas.clone(), s.brake.clone());
    for i in a..n {
        let j = (i as i64 - d).clamp(a as i64, (n - 1) as i64) as usize;
        s.steer[i] = st[j];
        s.gas[i] = ga[j];
        s.brake[i] = br[j];
    }
    Op { kind: "retime", at: a, span: d, val: 0 }
}

/// Scale the steering in a window toward or away from centre: a gentler,
/// shape-preserving alternative to adding a bump.
///
/// Softening the steer has paid three times running on one map (127 -> 96 ->
/// 32), which is why it is a first-class operator and not a special case.
fn scale_win(s: &mut Inputs, rng: &mut Rng, lo: usize, hi: usize) -> Op {
    let a = lo + rng.below((hi - lo).max(1));
    let b = (a + rng.range(5, 120) as usize).min(hi);
    let f = 0.60 + 0.80 * rng.unit();
    for i in a..b {
        s.steer[i] = clamp_i8(s.steer[i] as f64 * f);
    }
    Op { kind: "scale", at: a, span: (b - a) as i64, val: (100.0 * f) as i64 }
}

/// Apply one random operator to `s`, confined to ticks `[lo, hi)`.
pub fn mutate(s: &mut Inputs, rng: &mut Rng, lo: usize, hi: usize, set: OpSet) -> Op {
    let hi0 = hi.min(s.len());
    let lo0 = lo.min(hi0.saturating_sub(1));
    if hi0 > lo0 + 1 {
        match set {
            OpSet::Only(Kind::Doublet) => return doublet(s, rng, lo0, hi0),
            OpSet::Only(Kind::Retime) => return retime(s, rng, lo0, hi0),
            OpSet::Only(Kind::Scale) => return scale_win(s, rng, lo0, hi0),
            OpSet::Wide => {
                let u = rng.unit();
                if u < 0.25 {
                    return doublet(s, rng, lo0, hi0);
                } else if u < 0.35 {
                    return retime(s, rng, lo0, hi0);
                } else if u < 0.45 {
                    return scale_win(s, rng, lo0, hi0);
                }
            }
            OpSet::Local => {}
        }
    }
    local(s, rng, lo0, hi0)
}

fn local(s: &mut Inputs, rng: &mut Rng, lo: usize, hi: usize) -> Op {
    if hi <= lo + 1 {
        return Op { kind: "nop", at: lo, span: 0, val: 0 };
    }
    let pick = rng.unit();
    let a = lo + rng.below(hi - lo);

    if pick < 0.45 {
        // raised-cosine steer deformation: coloured noise, far more
        // sample-efficient than per-tick white jitter
        let r = rng.range(3, ((hi - lo) / 2).max(4) as i64) as usize;
        let amp = rng.sign() * rng.range(3, 100) as f64;
        for i in a.saturating_sub(r).max(lo)..(a + r).min(hi) {
            let w = 0.5 * (1.0 + (std::f64::consts::PI * (i as f64 - a as f64) / r as f64).cos());
            s.steer[i] = clamp_i8(s.steer[i] as f64 + amp * w);
        }
        Op { kind: "cos", at: a, span: r as i64, val: amp as i64 }
    } else if pick < 0.70 {
        // a flat analog level: a keyboard template only ever uses -127/0/+127,
        // so the other 252 values are unexplored space
        let b = (a + rng.range(2, 60) as usize).min(hi);
        let base = s.steer[a] as f64;
        let tgt = if rng.unit() < 0.30 {
            rng.range(-127, 127) as f64
        } else {
            base * (0.30 + 0.70 * rng.unit()) + rng.range(-20, 20) as f64
        };
        let t = clamp_i8(tgt);
        for i in a..b {
            s.steer[i] = t;
        }
        Op { kind: "level", at: a, span: (b - a) as i64, val: t as i64 }
    } else if pick < 0.85 {
        // shift a steering transition: preserves the digital structure of a
        // keyboard run, which per-tick noise destroys
        let mut edges = Vec::new();
        for i in lo.max(1)..hi {
            if s.steer[i] != s.steer[i - 1] {
                edges.push(i);
            }
        }
        if edges.is_empty() {
            return Op { kind: "nop", at: a, span: 0, val: 0 };
        }
        let e = edges[rng.below(edges.len())];
        let mut d = rng.range(-5, 5);
        if d == 0 {
            d = 1;
        }
        let (p, q) = if d < 0 {
            ((e as i64 + d).max(lo as i64) as usize, e)
        } else {
            (e, ((e as i64 + d) as usize).min(hi))
        };
        let fill = if d < 0 { s.steer[e - 1] } else { s.steer[e] };
        for i in p..q {
            s.steer[i] = fill;
        }
        Op { kind: "edge", at: e, span: d, val: fill as i64 }
    } else {
        let b = (a + rng.range(1, 15) as usize).min(hi);
        let is_gas = rng.unit() < 0.6;
        let v = rng.next_u64() & 1 == 1;
        for i in a..b {
            if is_gas {
                s.gas[i] = v;
            } else {
                s.brake[i] = v;
            }
        }
        Op {
            kind: if is_gas { "gas" } else { "brake" },
            at: a,
            span: (b - a) as i64,
            val: v as i64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(n: usize) -> Inputs {
        Inputs { steer: vec![0; n], gas: vec![true; n], brake: vec![false; n], respawn: Vec::new() }
    }

    /// Every operator must stay inside its window. A mutation below the resume
    /// boundary is invisible to a fork evaluator and present in the written
    /// file -- the exact shape of the phantom-improvement defect.
    #[test]
    fn no_operator_ever_writes_outside_its_window() {
        let sets = [
            OpSet::Local,
            OpSet::Wide,
            OpSet::Only(Kind::Doublet),
            OpSet::Only(Kind::Scale),
        ];
        for set in sets {
            for seed in 0..400u64 {
                let base = flat(600);
                let mut s = base.clone();
                let mut rng = Rng::new(seed);
                mutate(&mut s, &mut rng, 200, 400, set);
                for t in 0..600 {
                    if (200..400).contains(&t) {
                        continue;
                    }
                    assert!(
                        s.steer[t] == base.steer[t]
                            && s.gas[t] == base.gas[t]
                            && s.brake[t] == base.brake[t],
                        "{:?} seed {} wrote tick {} outside [200,400)",
                        set,
                        seed,
                        t
                    );
                }
            }
        }
    }

    /// `retime` deliberately rewrites the whole tail, so it is excluded above
    /// and pinned here instead: it may not touch anything BEFORE the window.
    #[test]
    fn retime_rewrites_the_tail_and_nothing_before_it() {
        for seed in 0..200u64 {
            let base = flat(600);
            let mut s = base.clone();
            let mut rng = Rng::new(seed);
            let op = mutate(&mut s, &mut rng, 200, 400, OpSet::Only(Kind::Retime));
            for t in 0..op.at {
                assert_eq!(s.steer[t], base.steer[t], "retime touched tick {} < {}", t, op.at);
            }
        }
    }

    #[test]
    fn a_seed_reproduces_its_run() {
        let mut a = flat(300);
        let mut b = flat(300);
        let (mut r1, mut r2) = (Rng::new(7), Rng::new(7));
        for _ in 0..50 {
            mutate(&mut a, &mut r1, 0, 300, OpSet::Wide);
            mutate(&mut b, &mut r2, 0, 300, OpSet::Wide);
        }
        assert!(a == b);
    }

    #[test]
    fn distance_reports_where_a_candidate_left_the_reference() {
        let r = flat(100);
        let mut c = r.clone();
        c.steer[40] = 60;
        c.steer[41] = -3;
        let d = c.distance_from(&r);
        assert_eq!(d.first_diff_tick, Some(40));
        assert_eq!(d.diff_ticks, 2);
        assert_eq!(d.max_steer_delta, 60);
        assert_eq!(r.distance_from(&r).first_diff_tick, None);
    }
}

/// A HUMAN-SHAPED CONSTRAINT on a candidate: an input alphabet for steer and a
/// minimum hold per input change, applied to every candidate AFTER mutation and
/// before evaluation. Quantising an optimised analog tape afterwards never
/// survives (a tape is a closed loop around its own trajectory: 64 levels
/// still DNF'd), so the tape has to be SEARCHED under the constraint -- every
/// candidate the search ever scores is already legal, and so is everything it
/// banks.
///
/// `alphabet`: the steer values a candidate may hold (`kb` = -127, 0, +127);
/// every steer byte snaps to the nearest one. `minhold`: the fewest ticks any
/// input may hold before it changes again; a change that comes sooner is
/// suppressed (the previous value is held through it), on steer, gas and brake
/// alike. Ticks outside `[lo, hi)` are left exactly as they are.
#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    pub alphabet: Option<Vec<i8>>,
    pub minhold: usize,
    /// GAS HELD: every tick in the window keeps the throttle down. A keyboard
    /// driver (and the rig's key injection) re-engages the throttle slowly --
    /// measured in the client on 2026-09-29: a 50 ms lift cost ~300 ms of flat
    /// speed and 6 km/h at the ramp -- so a tape meant to be DRIVEN controls
    /// speed with the brake alone.
    pub gas_held: bool,
}

impl Constraint {
    pub fn parse_alphabet(s: &str) -> Result<Vec<i8>, String> {
        if s == "kb" || s == "keyboard" {
            return Ok(vec![-127, 0, 127]);
        }
        let mut out = Vec::new();
        for part in s.split(',') {
            let v: i32 = part.trim().parse().map_err(|_| format!("--alphabet: {:?} is not an i8 level", part))?;
            if !(-127..=127).contains(&v) {
                return Err(format!("--alphabet: level {} is outside -127..=127", v));
            }
            out.push(v as i8);
        }
        if out.len() < 2 {
            return Err("--alphabet needs at least two levels (or `kb`)".into());
        }
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }

    pub fn is_noop(&self) -> bool {
        self.alphabet.is_none() && self.minhold <= 1
    }

    fn snap(levels: &[i8], v: i8) -> i8 {
        let mut best = levels[0];
        let mut bd = (v as i32 - best as i32).abs();
        for &l in &levels[1..] {
            let d = (v as i32 - l as i32).abs();
            // ties go to the level nearer zero: a released key, not a pressed one
            if d < bd || (d == bd && (l as i32).abs() < (best as i32).abs()) {
                best = l;
                bd = d;
            }
        }
        best
    }

    fn hold<T: Copy + PartialEq>(v: &mut [T], lo: usize, hi: usize, n: usize) {
        if n <= 1 || hi <= lo + 1 {
            return;
        }
        // The run in force when the window opens is the one holding at `lo - 1`,
        // counted from where it really started: a change AT the window edge is
        // judged like any other, so the edge is not a free place to change.
        let (mut run_val, mut run_len, first) = if lo > 0 {
            let mut j = lo - 1;
            let mut n = 1usize;
            while j > 0 && v[j - 1] == v[lo - 1] {
                j -= 1;
                n += 1;
            }
            (v[lo - 1], n, lo)
        } else {
            (v[0], 1usize, 1)
        };
        for i in first..hi {
            if v[i] != run_val {
                if run_len < n {
                    v[i] = run_val;
                    run_len += 1;
                } else {
                    run_val = v[i];
                    run_len = 1;
                }
            } else {
                run_len += 1;
            }
        }
    }

    /// Make `s` legal inside `[lo, hi)`.
    pub fn apply(&self, s: &mut Inputs, lo: usize, hi: usize) {
        let hi = hi.min(s.len());
        if lo >= hi {
            return;
        }
        if let Some(levels) = &self.alphabet {
            for v in &mut s.steer[lo..hi] {
                *v = Self::snap(levels, *v);
            }
        }
        if self.minhold > 1 {
            Self::hold(&mut s.steer, lo, hi, self.minhold);
            Self::hold(&mut s.gas, lo, hi, self.minhold);
            Self::hold(&mut s.brake, lo, hi, self.minhold);
        }
        if self.gas_held {
            for g in &mut s.gas[lo..hi] {
                *g = true;
            }
        }
    }

    /// Does `s` already satisfy the constraint over `[lo, hi)`?
    pub fn holds(&self, s: &Inputs, lo: usize, hi: usize) -> bool {
        let mut t = s.clone();
        self.apply(&mut t, lo, hi);
        t.steer == s.steer && t.gas == s.gas && t.brake == s.brake
    }

    /// The number of input CHANGES (steer, gas or brake) in `[lo, hi)` -- the
    /// thing a low-input tape is measured by.
    pub fn events(s: &Inputs, lo: usize, hi: usize) -> usize {
        let hi = hi.min(s.len());
        let mut n = 0;
        for i in lo.max(1)..hi {
            if s.steer[i] != s.steer[i - 1] || s.gas[i] != s.gas[i - 1] || s.brake[i] != s.brake[i - 1] {
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod constraint_tests {
    use super::*;

    fn tape(steer: &[i8]) -> Inputs {
        Inputs {
            steer: steer.to_vec(),
            gas: vec![true; steer.len()],
            brake: vec![false; steer.len()],
            respawn: Vec::new(),
        }
    }

    #[test]
    fn the_keyboard_alphabet_snaps_to_the_nearest_key_and_ties_release() {
        let c = Constraint { alphabet: Some(Constraint::parse_alphabet("kb").unwrap()), minhold: 1, gas_held: false };
        let mut s = tape(&[-127, -90, -64, -63, -20, 0, 20, 63, 64, 90, 127]);
        let n = s.len();
        c.apply(&mut s, 0, n);
        // 64 is nearer 127 (63) than 0 (64); 63 is nearer 0
        assert_eq!(s.steer, vec![-127, -127, -127, 0, 0, 0, 0, 0, 127, 127, 127]);
    }

    #[test]
    fn a_change_sooner_than_the_hold_is_suppressed_and_the_run_continues() {
        let c = Constraint { alphabet: None, minhold: 3, gas_held: false };
        let mut s = tape(&[0, 0, 0, 5, 0, 0, 0, 7, 7, 7, 9, 9]);
        let n = s.len();
        c.apply(&mut s, 0, n);
        // HOLD-FORWARD semantics: a value, once it changes, is held for at least
        // the minimum; a change inside that hold is dropped and the held value
        // continues. The lone 5 becomes a 3-tick press (a real move for the
        // search, not a no-op), which delays the 0 and then swallows the 9s.
        assert_eq!(s.steer, vec![0, 0, 0, 5, 5, 5, 0, 0, 0, 7, 7, 7]);
        assert!(c.holds(&s, 0, n));
    }

    #[test]
    fn the_window_edge_is_not_a_free_place_to_change() {
        let c = Constraint { alphabet: None, minhold: 4, gas_held: false };
        // the run of 1s started at tick 1; editing [3, 8) must still see it as
        // 2 ticks old at tick 3
        let mut s = tape(&[0, 1, 1, 2, 2, 2, 2, 2, 3]);
        c.apply(&mut s, 3, 8);
        assert_eq!(s.steer, vec![0, 1, 1, 1, 1, 2, 2, 2, 3]);
    }

    #[test]
    fn events_count_every_channel_change_once_per_tick() {
        let mut s = tape(&[0, 0, 127, 127, 0]);
        s.brake[2] = true;
        s.brake[3] = true;
        let n = s.len();
        assert_eq!(Constraint::events(&s, 0, n), 2);
    }
}
