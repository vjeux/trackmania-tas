//! Mutation operators over a per-tick input state, plus a small deterministic
//! RNG (no external crates, and reproducible from a seed).

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).max(1))
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
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

/// One candidate's inputs. `steer` is the signed steering value per tick;
/// TM2020 admits only -127..=127 (255 values), not the +-65536 analog range
/// TMNF/TMInterface uses.
#[derive(Clone)]
pub struct State {
    pub steer: Vec<i8>,
    pub accel: Vec<u8>,
    pub brake: Vec<u8>,
}

impl State {
    pub fn len(&self) -> usize {
        self.steer.len()
    }
    pub fn steer_u8(&self) -> Vec<u8> {
        self.steer.iter().map(|&v| v as u8).collect()
    }
}

#[inline]
fn clamp_i8(v: f64) -> i8 {
    v.round().clamp(-127.0, 127.0) as i8
}

#[derive(Clone, Debug)]
pub struct Op {
    pub kind: &'static str,
    pub at: usize,
    pub span: i64,
    pub val: i64,
}

/// Apply one random operator to `s`, confined to ticks [lo, hi).
pub fn mutate(s: &mut State, rng: &mut Rng, lo: usize, hi: usize, amp_scale: f64) -> Op {
    mutate_kind(s, rng, lo, hi, amp_scale, "mix")
}

/// A STEERING DOUBLET: +A then -A, so the two lobes cancel. A plain
/// raised-cosine bump changes the car's HEADING, and every input after it was
/// tuned for the old heading, so the rest of the run falls apart -- measured:
/// a 160+ tick bump finishes 1% of the time. A doublet integrates to zero, so
/// it moves the car sideways and hands the tail back a car pointing the same
/// way. That is the difference between "edit the plan" and "invalidate it".
fn doublet(s: &mut State, rng: &mut Rng, lo: usize, hi: usize, amp_scale: f64) -> Op {
    let w = hi - lo;
    let r = rng.range(3, (w / 4).max(4) as i64) as usize;
    let a = lo + r + rng.below(w.saturating_sub(4 * r).max(1));
    let b = a + 2 * r;
    if b + r >= hi {
        return Op { kind: "nop", at: a, span: 0, val: 0 };
    }
    let amp = rng.sign() * rng.range(3, (100.0 * amp_scale).max(4.0) as i64) as f64;
    for (c, sign) in [(a, 1.0), (b, -1.0)] {
        for i in c.saturating_sub(r).max(lo)..(c + r).min(hi) {
            let w = 0.5 * (1.0 + (std::f64::consts::PI * (i as f64 - c as f64) / r as f64).cos());
            s.steer[i] = clamp_i8(s.steer[i] as f64 + sign * amp * w);
        }
    }
    Op { kind: "dbl", at: a, span: r as i64, val: amp as i64 }
}

/// RETIME THE TAIL: shift every input from tick `a` onward by `d` ticks. The
/// downstream plan is preserved exactly, just executed earlier or later --
/// "brake a tick sooner and keep everything else" is a move a human TASer
/// makes constantly and one the local operators cannot express at all.
fn retime(s: &mut State, rng: &mut Rng, lo: usize, hi: usize) -> Op {
    let n = s.len();
    let a = lo + rng.below((hi - lo).max(1));
    let d = {
        let mut d = rng.range(-4, 4);
        if d == 0 {
            d = 1;
        }
        d
    };
    let (st, ac, br) = (s.steer.clone(), s.accel.clone(), s.brake.clone());
    for i in a..n {
        let j = i as i64 - d;
        let j = j.clamp(a as i64, (n - 1) as i64) as usize;
        s.steer[i] = st[j];
        s.accel[i] = ac[j];
        s.brake[i] = br[j];
    }
    Op { kind: "shift", at: a, span: d, val: 0 }
}

/// Scale the steering in a window toward or away from centre: a gentler,
/// shape-preserving alternative to adding a bump.
fn scale_win(s: &mut State, rng: &mut Rng, lo: usize, hi: usize) -> Op {
    let a = lo + rng.below((hi - lo).max(1));
    let b = (a + rng.range(5, 120) as usize).min(hi);
    let f = 0.60 + 0.80 * rng.unit();
    for i in a..b {
        s.steer[i] = clamp_i8(s.steer[i] as f64 * f);
    }
    Op { kind: "scale", at: a, span: (b - a) as i64, val: (100.0 * f) as i64 }
}

/// `kind` selects one operator by name, or "mix" for the search's own
/// distribution. Naming one is what makes an A/B of the move set possible.
pub fn mutate_kind(
    s: &mut State,
    rng: &mut Rng,
    lo: usize,
    hi: usize,
    amp_scale: f64,
    kind: &str,
) -> Op {
    let hi0 = hi.min(s.len());
    let lo0 = lo.min(hi0.saturating_sub(1));
    if hi0 > lo0 + 1 {
        match kind {
            "dbl" => return doublet(s, rng, lo0, hi0, amp_scale),
            "shift" => return retime(s, rng, lo0, hi0),
            "scale" => return scale_win(s, rng, lo0, hi0),
            "mix2" => {
                let u = rng.unit();
                if u < 0.25 {
                    return doublet(s, rng, lo0, hi0, amp_scale);
                } else if u < 0.35 {
                    return retime(s, rng, lo0, hi0);
                } else if u < 0.45 {
                    return scale_win(s, rng, lo0, hi0);
                }
            }
            _ => {}
        }
    }
    mutate_mix(s, rng, lo, hi, amp_scale)
}

fn mutate_mix(s: &mut State, rng: &mut Rng, lo: usize, hi: usize, amp_scale: f64) -> Op {
    let hi = hi.min(s.len());
    let lo = lo.min(hi.saturating_sub(1));
    if hi <= lo + 1 {
        return Op { kind: "nop", at: lo, span: 0, val: 0 };
    }
    let pick = rng.unit();
    let a = lo + rng.below(hi - lo);

    if pick < 0.45 {
        // raised-cosine steer deformation -- colored noise, far more
        // sample-efficient than per-tick white jitter (iCEM)
        let r = rng.range(3, ((hi - lo) / 2).max(4) as i64) as usize;
        let amp = rng.sign() * rng.range(3, (100.0 * amp_scale).max(4.0) as i64) as f64;
        let from = a.saturating_sub(r).max(lo);
        let to = (a + r).min(hi);
        for i in from..to {
            let w = 0.5 * (1.0 + (std::f64::consts::PI * (i as f64 - a as f64) / r as f64).cos());
            s.steer[i] = clamp_i8(s.steer[i] as f64 + amp * w);
        }
        Op { kind: "cos", at: a, span: r as i64, val: amp as i64 }
    } else if pick < 0.70 {
        // flat analog level over a sub-window: a keyboard template only ever
        // uses -127/0/+127, so the other 252 values are unexplored space
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
        Op { kind: "lvl", at: a, span: (b - a) as i64, val: t as i64 }
    } else if pick < 0.85 {
        // shift a steering transition -- preserves the digital structure of a
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
            ((e as i64 + d).max(0) as usize, e)
        } else {
            (e, ((e as i64 + d) as usize).min(s.len()))
        };
        let fill = if d < 0 { s.steer[e - 1] } else { s.steer[e] };
        for i in p..q {
            s.steer[i] = fill;
        }
        Op { kind: "edge", at: e, span: d, val: fill as i64 }
    } else {
        let b = (a + rng.range(1, 15) as usize).min(hi);
        let is_accel = rng.unit() < 0.6;
        let v = (rng.next_u64() & 1) as u8;
        for i in a..b {
            if is_accel {
                s.accel[i] = v;
            } else {
                s.brake[i] = v;
            }
        }
        Op {
            kind: if is_accel { "acc" } else { "brk" },
            at: a,
            span: (b - a) as i64,
            val: v as i64,
        }
    }
}
