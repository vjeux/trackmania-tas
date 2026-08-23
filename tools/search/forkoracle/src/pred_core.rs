//! The predicate core: one copy of the condition language, shared verbatim by
//! the LD_PRELOAD shim (where it runs INSIDE the fork child, once per tick) and
//! by the `fk` driver (where it parses the command line, builds the wire
//! config, and re-evaluates the very same conditions offline against a
//! trajectory CSV).
//!
//! It is included by both crates with `#[path = ...] mod`, not linked as a
//! library, for one reason: `fkshim` is a `cdylib` that exports `lroundf`, and
//! linking it into `fk` would interpose the hook on the driver itself. A shared
//! source file gives one implementation with no linkage risk.
//!
//! # Rules the child imposes on this code
//!
//! * **No allocation, no syscalls, no panics on the hot path.** Everything is
//!   fixed-size: the evaluator is a `static` in the child, the ring buffers are
//!   arrays, the reference trajectory is a raw pointer into memory the parent
//!   leaked before the first fork (so every child inherits it copy-on-write and
//!   nobody pays to copy it).
//! * **No `std::fmt`, no `Vec`, in anything reachable from `feed`.**
//! * The evaluator may only READ the simulation's memory. A watchdog that
//!   perturbs the physics is worse than no watchdog at all.

#![allow(dead_code)]

pub const MAXP: usize = 8;
/// Longest window any predicate may look back over, in ticks (10 ms each).
pub const RINGW: usize = 1024;

pub const K_NONE: u32 = 0;
pub const K_SPEEDDROP: u32 = 1;
pub const K_FLOOR: u32 = 2;
pub const K_BOX: u32 = 3;
pub const K_OFFREF: u32 = 4;
pub const K_NOPROG: u32 = 5;

pub fn kind_name(k: u32) -> &'static str {
    match k {
        K_NONE => "none",
        K_SPEEDDROP => "speeddrop",
        K_FLOOR => "floor",
        K_BOX => "box",
        K_OFFREF => "offref",
        K_NOPROG => "noprog",
        _ => "unknown",
    }
}

pub fn kind_of(s: &str) -> Option<u32> {
    match s {
        "speeddrop" => Some(K_SPEEDDROP),
        "floor" => Some(K_FLOOR),
        "box" => Some(K_BOX),
        "offref" => Some(K_OFFREF),
        "noprog" => Some(K_NOPROG),
        _ => None,
    }
}

/// One armed condition. Fixed size and `Copy` so the child can hold an array of
/// them in `.bss` and never allocate.
///
/// `p` is the per-kind parameter block:
///
/// | kind | win | need | p[0] | p[1..] |
/// |---|---|---|---|---|
/// | speeddrop | look-back ticks | consecutive ticks | fraction of the window peak | p[1] = minimum peak that arms it (m/s) |
/// | floor | - | consecutive ticks | speed threshold (m/s) | |
/// | box | - | consecutive ticks | xmin | xmax ymin ymax zmin zmax |
/// | offref | - | consecutive ticks | metres from the reference line | |
/// | noprog | look-back ticks | consecutive ticks | metres of net displacement | |
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pred {
    pub kind: u32,
    pub win: u32,
    pub need: u32,
    /// First tick index at which this predicate may fire (absolute tape tick).
    pub after: i32,
    /// Last tick index at which it may fire; `i32::MAX` for "to the end".
    pub until: i32,
    pub p: [f32; 6],
}

pub const PRED_BYTES: usize = 4 * 5 + 4 * 6;

impl Pred {
    pub const ZERO: Pred = Pred {
        kind: 0,
        win: 0,
        need: 1,
        after: 0,
        until: i32::MAX,
        p: [0.0; 6],
    };
    pub fn encode(&self, out: &mut [u8]) {
        out[0..4].copy_from_slice(&self.kind.to_le_bytes());
        out[4..8].copy_from_slice(&self.win.to_le_bytes());
        out[8..12].copy_from_slice(&self.need.to_le_bytes());
        out[12..16].copy_from_slice(&self.after.to_le_bytes());
        out[16..20].copy_from_slice(&self.until.to_le_bytes());
        for i in 0..6 {
            out[20 + 4 * i..24 + 4 * i].copy_from_slice(&self.p[i].to_le_bytes());
        }
    }
    pub fn decode(b: &[u8]) -> Pred {
        let g4 = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let mut p = [0.0f32; 6];
        for i in 0..6 {
            p[i] = f32::from_bits(g4(20 + 4 * i));
        }
        Pred {
            kind: g4(0),
            win: g4(4),
            need: g4(8),
            after: g4(12) as i32,
            until: g4(16) as i32,
            p,
        }
    }
}

/// What the child reports back about a run. Written into a `MAP_SHARED` page,
/// so the fork server's parent can read it after the child is gone -- including
/// when the child exited early and printed no JSON at all.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Summary {
    pub magic: u32,
    /// Ticks the watchdog actually evaluated.
    pub nticks: u32,
    /// Last tape tick index seen (-1 if none).
    pub last_tick: i32,
    /// Index of the predicate that tripped, or -1.
    pub trip_pred: i32,
    pub trip_tick: i32,
    pub trip_value: f32,
    /// Furthest point reached along the reference line, in metres of its
    /// arclength. 0 when no reference is armed.
    pub progress: f32,
    /// Path length actually driven since the resume, metres.
    pub travelled: f32,
    pub max_speed: f32,
    pub last_speed: f32,
    /// Largest distance from the reference line over the run, metres.
    pub off_max: f32,
    /// Reference index the progress corresponds to.
    pub refidx: i32,
    /// SUB-TICK FINISH TIMING (`Eval::plane_x`). The tape tick BEFORE the run
    /// crossed the armed plane x = plane_x going in -x, or -1 if it never did.
    pub cross_tick: i32,
    /// Where inside that tick the crossing fell, 0..1, by linear interpolation
    /// of the car's own position. `cross_tick + cross_frac` is a continuous
    /// arrival time; the validator's integer millisecond is a floor of the
    /// same quantity and is 1000x coarser.
    pub cross_frac: f32,

    /// THE STATE OBJECTIVE. The tape tick at which the gate key reached its
    /// best value inside the armed box, or -1 if the run never qualified.
    ///
    /// Everything below is meaningless when this is -1 except `gate_miss`, and
    /// the driver's types make that unrepresentable rather than a rule to
    /// remember: see `tmsearch::score::GateState`.
    pub gate_tick: i32,
    /// The key at that tick. Bigger is better, always.
    pub gate_key: f32,
    /// CLOSEST APPROACH to the box, in metres, over the ticks that met the
    /// gate's minimum speed: 0 once anything got inside. This is what keeps
    /// the objective continuous for a candidate that never reaches the gate at
    /// all -- without it every run outside the box scores the same and the
    /// search is a random walk (measured on 228811: 160k evaluations, no
    /// gradient, nothing found).
    pub gate_miss: f32,
    /// THE WHOLE STATE at `gate_tick`: position, velocity AND attitude.
    ///
    /// Position and velocity together were not enough on 228811 -- the trigger
    /// was which way the car was pointing -- so the record carries the
    /// quaternion, and the seed identity control checks all three.
    pub gate_pos: [f32; 3],
    pub gate_vel: [f32; 3],
    /// `(qw, qx, qy, qz)`, the car's orientation.
    pub gate_quat: [f32; 4],
}

pub const SUMMARY_BYTES: usize = 108;
pub const SUMMARY_MAGIC: u32 = 0x464B5057; // "FKPW"

impl Summary {
    pub const ZERO: Summary = Summary {
        magic: 0,
        nticks: 0,
        last_tick: -1,
        trip_pred: -1,
        trip_tick: -1,
        trip_value: 0.0,
        progress: 0.0,
        travelled: 0.0,
        max_speed: 0.0,
        last_speed: 0.0,
        off_max: 0.0,
        refidx: -1,
        cross_tick: -1,
        cross_frac: 0.0,
        gate_tick: -1,
        gate_key: 0.0,
        gate_miss: f32::INFINITY,
        gate_pos: [0.0; 3],
        gate_vel: [0.0; 3],
        gate_quat: [0.0; 4],
    };
    pub fn encode(&self, o: &mut [u8]) {
        let w = |o: &mut [u8], i: usize, v: u32| o[i..i + 4].copy_from_slice(&v.to_le_bytes());
        w(o, 0, SUMMARY_MAGIC);
        w(o, 4, self.nticks);
        w(o, 8, self.last_tick as u32);
        w(o, 12, self.trip_pred as u32);
        w(o, 16, self.trip_tick as u32);
        w(o, 20, self.trip_value.to_bits());
        w(o, 24, self.progress.to_bits());
        w(o, 28, self.travelled.to_bits());
        w(o, 32, self.max_speed.to_bits());
        w(o, 36, self.last_speed.to_bits());
        w(o, 40, self.off_max.to_bits());
        w(o, 44, self.refidx as u32);
        w(o, 48, self.cross_tick as u32);
        w(o, 52, self.cross_frac.to_bits());
        w(o, 56, self.gate_tick as u32);
        w(o, 60, self.gate_key.to_bits());
        w(o, 64, self.gate_miss.to_bits());
        for i in 0..3 {
            w(o, 68 + 4 * i, self.gate_pos[i].to_bits());
            w(o, 80 + 4 * i, self.gate_vel[i].to_bits());
        }
        for i in 0..4 {
            w(o, 92 + 4 * i, self.gate_quat[i].to_bits());
        }
    }
    pub fn decode(b: &[u8]) -> Option<Summary> {
        if b.len() < SUMMARY_BYTES {
            return None;
        }
        let g = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        if g(0) != SUMMARY_MAGIC {
            return None;
        }
        let f = |i: usize| f32::from_bits(g(i));
        Some(Summary {
            magic: g(0),
            nticks: g(4),
            last_tick: g(8) as i32,
            trip_pred: g(12) as i32,
            trip_tick: g(16) as i32,
            trip_value: f32::from_bits(g(20)),
            progress: f32::from_bits(g(24)),
            travelled: f32::from_bits(g(28)),
            max_speed: f32::from_bits(g(32)),
            last_speed: f32::from_bits(g(36)),
            off_max: f32::from_bits(g(40)),
            refidx: g(44) as i32,
            cross_tick: g(48) as i32,
            cross_frac: f32::from_bits(g(52)),
            gate_tick: g(56) as i32,
            gate_key: f(60),
            gate_miss: f(64),
            gate_pos: [f(68), f(72), f(76)],
            gate_vel: [f(80), f(84), f(88)],
            gate_quat: [f(92), f(96), f(100), f(104)],
        })
    }
}

// ------------------------------------------------------------ the gate key
//
// THE KEY IS A PROGRAM, NOT A MODE NUMBER.
//
// The version of this that worked on 228811 had eleven hard-coded `gate_mode`
// integers -- `-vz`, body-lateral speed, `min(|side|, 5*-vz)`, a signed
// distance to a 6-D target state, and so on -- each one a match arm added to
// the child the day it was wanted. Two of them turned out to be decoys and one
// was a sign error; none of them could be tried without rebuilding the shim
// that runs inside the game server.
//
// So the key is compiled in the parent from an expression the operator writes
// (`min(abs(bodyright), 5*(-vz))`) into a fixed-size postfix program, and the
// child runs that program on a tiny stack. No allocation, no branching on
// strings, nothing to add to this file the next time a map wants a different
// quantity. The parser is `crate::pred::parse_key`, in the driver, where it can
// have a tokenizer and an error message; this side only executes.

/// Most operations one key may have. A program that does not fit is refused by
/// the parser rather than truncated.
pub const MAXKOPS: usize = 32;
const KSTACK: usize = 16;

pub const KOP_END: u32 = 0;
/// push a[0]
pub const KOP_CONST: u32 = 1;
/// push a component of the velocity / position: a[0] = 0|1|2 for x|y|z
pub const KOP_VEL: u32 = 2;
pub const KOP_POS: u32 = 3;
/// push |v|
pub const KOP_SPEED: u32 = 4;
/// push the velocity in the car's own frame: a[0] = 0 right, 1 up, 2 forward
pub const KOP_BODYVEL: u32 = 5;
/// push (body axis a[0]) . (world direction in a[1..], already a unit vector,
/// carried in the NEXT op's `a` -- see `KeyOp::dir`)
pub const KOP_AXISDOT: u32 = 6;
/// push v . dir
pub const KOP_ALONG: u32 = 7;
/// push |p - point|
pub const KOP_DIST: u32 = 8;
/// push |v - target|
pub const KOP_VDIST: u32 = 9;
pub const KOP_ADD: u32 = 10;
pub const KOP_SUB: u32 = 11;
pub const KOP_MUL: u32 = 12;
pub const KOP_DIV: u32 = 13;
pub const KOP_MIN: u32 = 14;
pub const KOP_MAX: u32 = 15;
pub const KOP_NEG: u32 = 16;
pub const KOP_ABS: u32 = 17;

/// One instruction. `a` carries a constant, an axis index, a world direction,
/// or a point, depending on `op`; `axis` picks a body axis for `KOP_AXISDOT`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyOp {
    pub op: u32,
    pub axis: u32,
    pub a: [f32; 3],
}

pub const KEYOP_BYTES: usize = 20;

impl KeyOp {
    pub const END: KeyOp = KeyOp { op: KOP_END, axis: 0, a: [0.0; 3] };
    pub fn encode(&self, o: &mut [u8]) {
        o[0..4].copy_from_slice(&self.op.to_le_bytes());
        o[4..8].copy_from_slice(&self.axis.to_le_bytes());
        for i in 0..3 {
            o[8 + 4 * i..12 + 4 * i].copy_from_slice(&self.a[i].to_le_bytes());
        }
    }
    pub fn decode(b: &[u8]) -> KeyOp {
        let g = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        KeyOp {
            op: g(0),
            axis: g(4),
            a: [f32::from_bits(g(8)), f32::from_bits(g(12)), f32::from_bits(g(16))],
        }
    }
}

/// The car's own axes in world coordinates, from `(qw, qx, qy, qz)`: the
/// columns of the body-to-world rotation. `0` right, `1` up, `2` forward.
#[inline]
pub fn body_axes(q: [f32; 4]) -> [[f32; 3]; 3] {
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

#[inline]
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Run a compiled key over one state. Total, allocation-free, and identical in
/// the child and in the driver's tests -- it is the same function.
///
/// A malformed program (stack underflow, no result) returns `f32::NAN`, which
/// the caller must treat as "this tick did not score". The parser makes that
/// unreachable; the check is here because this runs inside the game server.
pub fn key_eval(prog: &[KeyOp], pos: [f32; 3], vel: [f32; 3], quat: [f32; 4]) -> f32 {
    let mut st = [0.0f32; KSTACK];
    let mut sp = 0usize;
    let axes = body_axes(quat);
    for k in prog {
        if k.op == KOP_END {
            break;
        }
        // unary and binary operators pop before they push
        let v = match k.op {
            KOP_CONST => k.a[0],
            KOP_VEL => vel[(k.axis as usize) % 3],
            KOP_POS => pos[(k.axis as usize) % 3],
            KOP_SPEED => (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt(),
            KOP_BODYVEL => dot(axes[(k.axis as usize) % 3], vel),
            KOP_AXISDOT => dot(axes[(k.axis as usize) % 3], k.a),
            KOP_ALONG => dot(vel, k.a),
            KOP_DIST => {
                let d = [pos[0] - k.a[0], pos[1] - k.a[1], pos[2] - k.a[2]];
                dot(d, d).sqrt()
            }
            KOP_VDIST => {
                let d = [vel[0] - k.a[0], vel[1] - k.a[1], vel[2] - k.a[2]];
                dot(d, d).sqrt()
            }
            KOP_NEG | KOP_ABS => {
                if sp < 1 {
                    return f32::NAN;
                }
                sp -= 1;
                let x = st[sp];
                if k.op == KOP_NEG {
                    -x
                } else {
                    x.abs()
                }
            }
            _ => {
                if sp < 2 {
                    return f32::NAN;
                }
                sp -= 2;
                let (x, y) = (st[sp], st[sp + 1]);
                match k.op {
                    KOP_ADD => x + y,
                    KOP_SUB => x - y,
                    KOP_MUL => x * y,
                    KOP_DIV => x / y,
                    KOP_MIN => {
                        if x < y {
                            x
                        } else {
                            y
                        }
                    }
                    KOP_MAX => {
                        if x > y {
                            x
                        } else {
                            y
                        }
                    }
                    _ => return f32::NAN,
                }
            }
        };
        if sp >= KSTACK {
            return f32::NAN;
        }
        st[sp] = v;
        sp += 1;
    }
    if sp == 1 {
        st[0]
    } else {
        f32::NAN
    }
}

/// The armed gate: a box, a speed condition, and a key.
///
/// This is a `box` predicate that RECORDS instead of aborting. It shares the
/// box's overshoot arithmetic exactly -- `over <= 0` is inside -- which is what
/// makes the miss and the membership test the same measurement.
#[derive(Clone, Copy)]
pub struct Gate {
    pub armed: bool,
    /// xmin xmax ymin ymax zmin zmax
    pub bounds: [f32; 6],
    /// Ticks slower than this are ignored entirely, for the key AND for the
    /// miss: on a map where the car crawls through the box at the start, the
    /// standing start is not an arrival.
    pub minspeed: f32,
    pub prog: [KeyOp; MAXKOPS],
}

impl Default for Gate {
    fn default() -> Gate {
        Gate::NONE
    }
}

impl Gate {
    pub const NONE: Gate = Gate {        armed: false,
        bounds: [0.0; 6],
        minspeed: 0.0,
        prog: [KeyOp::END; MAXKOPS],
    };
    /// How far outside the box, in metres; `<= 0` is inside.
    #[inline]
    pub fn over(&self, pos: [f32; 3]) -> f32 {
        (self.bounds[0] - pos[0])
            .max(pos[0] - self.bounds[1])
            .max(self.bounds[2] - pos[1])
            .max(pos[1] - self.bounds[3])
            .max(self.bounds[4] - pos[2])
            .max(pos[2] - self.bounds[5])
    }
}

/// The reference line: one position per tape tick, plus its cumulative
/// arclength. Raw pointers, because in the child this points at memory the
/// fork server's parent leaked before the first fork.
#[derive(Clone, Copy)]
pub struct RefLine {
    pub n: usize,
    /// 3 * n f32: x, y, z per tick.
    pub xyz: *const f32,
    /// n f32: cumulative arclength in metres.
    pub s: *const f32,
    /// Only count progress while within this many metres of the line.
    pub corridor: f32,
    /// How far ahead of the last match to look for the nearest point.
    pub ahead: i32,
    /// How far back.
    pub back: i32,
}

impl RefLine {
    pub const NONE: RefLine = RefLine {
        n: 0,
        xyz: core::ptr::null(),
        s: core::ptr::null(),
        corridor: 40.0,
        ahead: 250,
        back: 60,
    };
    #[inline]
    unsafe fn pt(&self, i: usize) -> (f32, f32, f32) {
        (
            *self.xyz.add(3 * i),
            *self.xyz.add(3 * i + 1),
            *self.xyz.add(3 * i + 2),
        )
    }
}

/// The per-run evaluator. One instance lives as a `static mut` in the child, so
/// arming costs nothing and evaluating allocates nothing.
pub struct Eval {
    pub np: usize,
    pub preds: [Pred; MAXP],
    pub cons: [u32; MAXP],
    pub rl: RefLine,
    /// Arclength at which the reference crossed the FINISH. Past it the
    /// candidate has already finished (the engine keeps simulating for a
    /// fraction of a second afterwards, and the reference line's own tail runs
    /// past the finish too), so nothing may fire there: an abort after the
    /// finish would throw away a valid time. 0 disables the guard.
    pub finish_s: f32,
    /// World-x of a virtual timing plane. When non-zero, the first tick at
    /// which the car's x goes from above it to at-or-below it is recorded in
    /// the summary with sub-tick interpolation. Nothing is aborted by it: it is
    /// pure instrumentation, and it is what turns the validator's 1 ms integer
    /// into a continuous objective.
    pub plane_x: f32,
    /// THE STATE OBJECTIVE: a box that records the car's whole state instead
    /// of aborting the run. See `Gate`.
    pub gate: Gate,
    ring_sp: [f32; RINGW],
    ring_x: [f32; RINGW],
    ring_y: [f32; RINGW],
    ring_z: [f32; RINGW],
    head: usize,
    filled: usize,
    /// Reference index the car is nearest to right now (the progress index is
    /// a running maximum and may lag behind this one).
    cur: usize,
    prev_valid: bool,
    prev: [f32; 3],
    pub sum: Summary,
}

impl Eval {
    pub const ZERO: Eval = Eval {
        np: 0,
        preds: [Pred::ZERO; MAXP],
        cons: [0; MAXP],
        rl: RefLine::NONE,
        finish_s: 0.0,
        plane_x: 0.0,
        gate: Gate::NONE,
        ring_sp: [0.0; RINGW],
        ring_x: [0.0; RINGW],
        ring_y: [0.0; RINGW],
        ring_z: [0.0; RINGW],
        head: 0,
        filled: 0,
        cur: 0,
        prev_valid: false,
        prev: [0.0; 3],
        sum: Summary::ZERO,
    };

    /// Re-arm for a new run without touching the armed predicates.
    pub fn reset(&mut self) {
        self.cons = [0; MAXP];
        self.head = 0;
        self.filled = 0;
        self.cur = 0;
        self.prev_valid = false;
        self.prev = [0.0; 3];
        self.sum = Summary::ZERO;
        self.sum.magic = SUMMARY_MAGIC;
    }

    #[inline]
    fn at(&self, back: usize) -> (f32, f32, f32, f32) {
        // back = 0 is the newest sample
        let i = (self.head + RINGW - 1 - back) % RINGW;
        (self.ring_sp[i], self.ring_x[i], self.ring_y[i], self.ring_z[i])
    }

    /// Feed one finished tick. Returns the index of the predicate that tripped,
    /// or -1. Everything the caller needs afterwards is in `self.sum`.
    ///
    /// `tick` is the tape tick index whose input produced this state. `quat` is
    /// the car's orientation as `(qw, qx, qy, qz)` -- the state objective is the
    /// only thing that reads it, and it is here because position and velocity
    /// together were not enough on the one map this feature was proven on.
    pub fn feed(&mut self, tick: i32, pos: [f32; 3], vel: [f32; 3], quat: [f32; 4]) -> i32 {
        let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
        // ---- running totals
        if self.prev_valid {
            let d = dist(pos, self.prev);
            if d.is_finite() && d < 100.0 {
                self.sum.travelled += d;
            }
            // ---- sub-tick timing plane (see `plane_x`). First crossing only,
            // and it must be a real transition, so a car sitting on the plane
            // cannot register.
            if self.plane_x != 0.0 && self.sum.cross_tick < 0 {
                let (a, b) = (self.prev[0], pos[0]);
                if a > self.plane_x && b <= self.plane_x {
                    let f = (a - self.plane_x) / (a - b);
                    self.sum.cross_tick = tick - 1;
                    self.sum.cross_frac = if f.is_finite() { f } else { 0.0 };
                }
            }
        }
        self.prev = pos;
        self.prev_valid = true;
        self.ring_sp[self.head] = speed;
        self.ring_x[self.head] = pos[0];
        self.ring_y[self.head] = pos[1];
        self.ring_z[self.head] = pos[2];
        self.head = (self.head + 1) % RINGW;
        if self.filled < RINGW {
            self.filled += 1;
        }
        self.sum.nticks += 1;
        self.sum.last_tick = tick;
        self.sum.last_speed = speed;
        if speed > self.sum.max_speed {
            self.sum.max_speed = speed;
        }

        // ---- THE STATE OBJECTIVE.
        //
        // Read-only, allocation-free, and deliberately a MAXIMUM over ticks:
        // like `progress`, aborting a run can only remove ticks, so an aborted
        // candidate's key is <= the key it would have shown had it run on. A
        // search may therefore arm the watchdog without ever ranking a dead
        // candidate above a live one.
        //
        // The miss is tracked on exactly the same ticks the key would be, so
        // "how close did it come" and "did it arrive" are one measurement and
        // cannot disagree.
        if self.gate.armed && speed >= self.gate.minspeed {
            let over = self.gate.over(pos);
            if over > 0.0 {
                if over < self.sum.gate_miss {
                    self.sum.gate_miss = over;
                }
            } else {
                self.sum.gate_miss = 0.0;
                let key = key_eval(&self.gate.prog, pos, vel, quat);
                if key.is_finite() && (self.sum.gate_tick < 0 || key > self.sum.gate_key) {
                    self.sum.gate_key = key;
                    self.sum.gate_tick = tick;
                    self.sum.gate_pos = pos;
                    self.sum.gate_vel = vel;
                    self.sum.gate_quat = quat;
                }
            }
        }

        // ---- where are we along the reference line?
        //
        // Nearest point, hill-descended from the last match rather than argmin
        // over a window: the car advances about one reference index per tick,
        // so this costs two or three distance evaluations instead of three
        // hundred, and it stays LOCAL, which is what you want on a track that
        // passes near itself. Squared distances throughout; one sqrt per tick.
        //
        // The measure is a perpendicular deviation from the LINE, not a
        // distance to where the reference was at the same millisecond: a
        // candidate that is simply 100 ms ahead of the incumbent has not left
        // the line and must not be scored as if it had.
        let mut off = f32::INFINITY;
        if self.rl.n > 0 {
            unsafe {
                // The first tick of a run anchors globally -- the resume can
                // start anywhere on the line -- and every tick after it
                // searches a short window around the last match. A pure
                // hill-descent is not usable here: the line has plateaus (the
                // standing start, any tick the car does not move) and a
                // descent stops dead on the first of them.
                let (lo, hi) = if self.sum.nticks <= 1 {
                    (0usize, self.rl.n - 1)
                } else {
                    (
                        self.cur.saturating_sub(self.rl.back as usize),
                        (self.cur + self.rl.ahead as usize).min(self.rl.n - 1),
                    )
                };
                let mut j = lo;
                let mut d = f32::INFINITY;
                for i in lo..=hi {
                    let dd = d2(pos, self.rl.pt(i));
                    // `<=` breaks ties towards the LATER index on purpose: the
                    // line has stretches of exactly-repeated points (the
                    // standing start, any tick the reference did not move) and
                    // taking the earliest of them pins the match at the front
                    // of the plateau for the rest of the run. A tie means zero
                    // arclength between the two, so preferring the later one
                    // cannot inflate progress.
                    if dd <= d {
                        d = dd;
                        j = i;
                    }
                }
                self.cur = j;
                off = d.sqrt();
                if off > self.sum.off_max {
                    self.sum.off_max = off;
                }
                if off <= self.rl.corridor {
                    let s = *self.rl.s.add(j);
                    if s > self.sum.progress {
                        self.sum.progress = s;
                        self.sum.refidx = j as i32;
                    }
                }
            }
        }

        // ---- the armed conditions, in order; first to trip wins
        if self.finish_s > 0.0 && self.sum.progress >= self.finish_s {
            // already at the finish line: whatever happens now, the time is
            // banked. Keep measuring, stop judging.
            return -1;
        }
        for i in 0..self.np {
            let p = self.preds[i];
            if tick < p.after || tick > p.until {
                self.cons[i] = 0;
                continue;
            }
            let (hit, val) = match p.kind {
                K_SPEEDDROP => {
                    let w = (p.win as usize).min(self.filled).max(1);
                    let mut peak = 0.0f32;
                    for b in 0..w {
                        let s = self.at(b).0;
                        if s > peak {
                            peak = s;
                        }
                    }
                    (peak >= p.p[1] && speed < p.p[0] * peak, speed)
                }
                K_FLOOR => (speed < p.p[0], speed),
                K_BOX => {
                    let over = (p.p[0] - pos[0])
                        .max(pos[0] - p.p[1])
                        .max(p.p[2] - pos[1])
                        .max(pos[1] - p.p[3])
                        .max(p.p[4] - pos[2])
                        .max(pos[2] - p.p[5]);
                    (over > 0.0, over)
                }
                K_OFFREF => {
                    if self.rl.n == 0 {
                        (false, 0.0)
                    } else {
                        (off > p.p[0], off)
                    }
                }
                K_NOPROG => {
                    let w = p.win as usize;
                    if self.filled <= w {
                        (false, 0.0)
                    } else {
                        let a = self.at(0);
                        let b = self.at(w);
                        let d = dist([a.1, a.2, a.3], [b.1, b.2, b.3]);
                        (d < p.p[0], d)
                    }
                }
                _ => (false, 0.0),
            };
            if hit {
                self.cons[i] += 1;
                if self.cons[i] >= p.need.max(1) {
                    self.sum.trip_pred = i as i32;
                    self.sum.trip_tick = tick;
                    self.sum.trip_value = val;
                    return i as i32;
                }
            } else {
                self.cons[i] = 0;
            }
        }
        -1
    }
}

#[inline]
fn d2(a: [f32; 3], b: (f32, f32, f32)) -> f32 {
    let (dx, dy, dz) = (a[0] - b.0, a[1] - b.1, a[2] - b.2);
    dx * dx + dy * dy + dz * dz
}

#[inline]
fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}
