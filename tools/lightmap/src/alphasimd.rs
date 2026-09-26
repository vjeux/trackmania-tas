//! THE ALPHA TEST SIXTEEN FRAGMENTS AT A TIME (perf engineer 4). `alphatex::AlphaTex::passes_planned` runs one
//! fragment through 2–3 anisotropic taps × 2 mip levels of dependent scalar arithmetic — ~55 instructions per
//! bilinear tap on a critical path of ~40 cycles, out-of-order execution finding little to overlap because
//! the raster's visit loop branches on every answer. Here the raster QUEUES the card fragments of a band
//! (`AlphaQueues::push`: the fragment's uv and the payload the raster needs back — pixel, depth, triangle)
//! and the queue runs the test for 16 of them at once (`flush`), one fragment per SIMD lane, then hands the
//! PASSING payloads back in push order. Fragments are queued by their triangle's TAP COUNT (one queue per
//! count), so a batch's lanes run the same number of taps and no lane computes a tap its triangle has not.
//!
//! EXACTNESS. The kernel performs, lane by lane, the very same sequence of IEEE operations as
//! `sample_planned_clamp` — clamp, `·wf − 0.5`, floor, `floor((fx − x0)·256)·2⁻⁸`, the four texels a tap
//! reads, the bilinear in the written order, `a + (b − a)·t`, the taps summed in tap order, `sum / n`,
//! `α − threshold ≥ 0` — with vector instructions whose per-lane results are the scalar ones (vmulps /
//! vaddps / vsubps / vdivps are correctly rounded like their scalar forms, vrndscaleps(9) is floor,
//! vcvttps2dq is `as i32` on the in-range values the clamps guarantee, no FMA anywhere: Rust never contracts
//! and the kernel spells every multiply and add separately). Two representation changes, both exact:
//! (1) the four texels of a tap come from the level's QUAD TABLE (`AlphaLevel::quad`: the 2×2 neighbourhood
//! as four bytes, ClampEdge baked in) — one 32-bit gather per tap instead of four f32 gathers (the gathers
//! were the whole cost of the first version); (2) the byte → `a as f32 / 255.0` conversion is done without a
//! division: with m = a·0x01010101 (a's byte four times), a/255 = m·2⁻³² + a·2⁻³²/255, so fl(a/255) is m
//! ROUNDED UP to 24 significant bits, times 2⁻³² (the discarded bits of m are a's own bits, whose top bit is
//! set, so the discarded part is at least half — a tie exactly when a is a power of two, which the positive
//! remainder breaks upward): a byte-broadcast shuffle, a round-toward-+∞ conversion, a multiply by a power
//! of two — checked against the division for all 256 bytes in the tests (a product by the rounded
//! reciprocal differs for 126 of them). The per-tap offsets `(i + 0.5)/n − 0.5` are computed once per
//! triangle in scalar f32, the same expression. Lanes may come from different triangles (their own levels,
//! lod fraction, tap axis — `LanePlan`) and different textures (64-bit per-lane texel pointers gathered with
//! vpgatherqd; no shared arena). The one input the vector min/max would treat differently from Rust's
//! `clamp` is a NaN coordinate: a batch with a NaN uv goes through the scalar test lane by lane. The
//! early-outs of `passes_planned` are exact, so the kernel's always-sample answer is the same answer; the
//! unit tests check the kernels against `passes_planned` and `passes_sampled` on random textures, plans and
//! coordinates, ties at the threshold included.
//!
//! The raster's order of fragment insertion changes with the queue (a passing card fragment is emitted at
//! the flush, after the plain fragments visited meanwhile); every consumer of the fragments sorts by
//! (pixel, z, triangle) — see the (z, tri) key on the sparse A-buffer's per-pixel sort in peel.rs.

use crate::alphatex::{Address, AlphaLevel, AlphaTex, TapPlan};

/// The lanes.
pub const LANES: usize = 16;

/// What the raster needs back for a passing fragment.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pend {
    pub x: u32,
    pub y: u32,
    pub z: f32,
    pub ti: u32,
    /// The fragment takes a count record (an item fragment of a counting band).
    pub count: bool,
}

/// The per-triangle constants of the test in the form the lanes read: the two levels' quad-table pointers
/// and sizes, the lod fraction, the tap axis and count.
#[derive(Clone, Copy, Debug)]
pub struct LanePlan {
    base0: *const u32,
    base1: *const u32,
    w0: u32,
    h0: u32,
    w1: u32,
    h1: u32,
    two: bool,
    t: f32,
    ax: f32,
    ay: f32,
    n: u32,
}

impl LanePlan {
    /// The lane form of `plan` for `tex` (whose level buffers the plan's pointers reference).
    pub fn of(tex: &AlphaTex, p: &TapPlan) -> LanePlan {
        let l0: &AlphaLevel = &tex.levels[p.l0];
        let l1: &AlphaLevel = &tex.levels[p.l1];
        // (the planned sampler averages the taps only when both the anisotropy and the tap count exceed one;
        // `plan` gives n = 1 when aniso ≤ 1, so `n > 1` alone selects the tap loop)
        let n = if p.aniso > 1 { p.n.max(1) as u32 } else { 1 };
        LanePlan { base0: l0.quad.as_ptr(), base1: l1.quad.as_ptr(), w0: l0.w as u32, h0: l0.h as u32, w1: l1.w as u32, h1: l1.h as u32, two: p.two, t: p.t, ax: p.axis[0], ay: p.axis[1], n }
    }
}

/// The per-slot tables the kernel permutes from (one slot per distinct triangle in the batch; a batch of 16
/// fragments references at most 16 triangles, and a triangle's fragments are consecutive in the raster's
/// visit order, so a slot is opened whenever the pushed triangle changes).
#[repr(C, align(64))]
struct SlotTable {
    base0: [u64; LANES],
    base1: [u64; LANES],
    wf0: [f32; LANES],
    hf0: [f32; LANES],
    /// The quad table's row stride (w + 1).
    qs0: [i32; LANES],
    wf1: [f32; LANES],
    hf1: [f32; LANES],
    qs1: [i32; LANES],
    t: [f32; LANES],
    two: [i32; LANES],
    ax: [f32; LANES],
    ay: [f32; LANES],
    /// The tap count as f32 and as i32.
    nf: [f32; LANES],
    n: [i32; LANES],
}

/// Which kernel runs the batches (LMTOOL_ALPHA_SIMD=scalar|avx2|avx512 overrides the detection — the A/B
/// switch; the results are identical whichever runs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    Scalar,
    Avx2,
    Avx512,
}

pub fn kernel() -> Kernel {
    static K: std::sync::OnceLock<Kernel> = std::sync::OnceLock::new();
    *K.get_or_init(|| {
        let forced = std::env::var("LMTOOL_ALPHA_SIMD").ok();
        let detected = {
            #[cfg(target_arch = "x86_64")]
            {
                if std::arch::is_x86_feature_detected!("avx512f") && std::arch::is_x86_feature_detected!("avx512bw") && std::arch::is_x86_feature_detected!("avx512vl") && std::arch::is_x86_feature_detected!("avx512dq") {
                    Kernel::Avx512
                } else if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
                    Kernel::Avx2
                } else {
                    Kernel::Scalar
                }
            }
            #[cfg(not(target_arch = "x86_64"))]
            {
                Kernel::Scalar
            }
        };
        match forced.as_deref() {
            Some("scalar") => Kernel::Scalar,
            Some("avx2") if detected != Kernel::Scalar => Kernel::Avx2,
            Some("avx512") if detected == Kernel::Avx512 => Kernel::Avx512,
            Some(other) if !other.is_empty() && other != "avx2" && other != "avx512" => panic!("LMTOOL_ALPHA_SIMD={other}: scalar | avx2 | avx512"),
            _ => detected,
        }
    })
}

/// A slot's texture and levels for the scalar fallback (the plan is rebuilt from the slot's fields).
#[derive(Clone, Copy)]
struct SlotRef {
    tex: *const AlphaTex,
    l0: u8,
    l1: u8,
    /// The slot's triangle and whether its fragments take a count record (per triangle, not per fragment).
    ti: u32,
    count: bool,
}

/// THE queue: up to 16 pending fragments (uv + payload, SoA) of up to 16 triangles with their own tap counts,
/// and the slot table of their plans — one per thread, ~2 KB, so it stays in L1 beside the job's tables
/// (one queue per tap count kept 4–6 of them warm: measured slower).
///
/// The slots hold raw pointers into the textures' level tables: a queue must be flushed (or dropped) before
/// the textures it was fed are — the raster flushes at the end of every job, and the textures live for the
/// whole bake.
/// (`repr(C, align(64))`, the lane arrays first: their 512-bit loads never split a cache line — see raster::Bary16.)
#[repr(C, align(64))]
pub struct AlphaQueue {
    slot_of: [i32; LANES],
    u: [f32; LANES],
    v: [f32; LANES],
    px: [u32; LANES],
    py: [u32; LANES],
    pz: [f32; LANES],
    len: usize,
    slots: usize,
    last_key: u64,
    /// The largest tap count among the slots (the kernel's tap loop runs to it; lanes past their own count
    /// are masked).
    nmax: u32,
    table: Box<SlotTable>,
    refs: [SlotRef; LANES],
    kernel: Kernel,
    /// Batches flushed, fragments tested, fragments passed.
    pub stats: [u64; 3],
}

impl AlphaQueue {
    pub fn new() -> Self {
        let table = Box::new(SlotTable { base0: [0; LANES], base1: [0; LANES], wf0: [1.0; LANES], hf0: [1.0; LANES], qs0: [2; LANES], wf1: [1.0; LANES], hf1: [1.0; LANES], qs1: [2; LANES], t: [0.0; LANES], two: [0; LANES], ax: [0.0; LANES], ay: [0.0; LANES], nf: [1.0; LANES], n: [1; LANES] });
        AlphaQueue { len: 0, slots: 0, last_key: u64::MAX, nmax: 1, slot_of: [0; LANES], u: [0.0; LANES], v: [0.0; LANES], px: [0; LANES], py: [0; LANES], pz: [0.0; LANES], table, refs: [SlotRef { tex: std::ptr::null(), l0: 0, l1: 0, ti: 0, count: false }; LANES], kernel: kernel(), stats: [0; 3] }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Queue one fragment: `key` identifies its triangle (consecutive pushes with one key share a slot),
    /// `plan` is that triangle's plan for `tex`. Returns true when the batch is full — the caller flushes.
    #[inline(always)]
    pub fn push(&mut self, tex: &AlphaTex, plan: &TapPlan, key: u64, u: f32, v: f32, payload: Pend) -> bool {
        if key != self.last_key {
            self.open_slot(tex, plan, key, payload.ti, payload.count);
        }
        let i = self.len;
        self.slot_of[i] = (self.slots - 1) as i32;
        self.u[i] = u;
        self.v[i] = v;
        self.px[i] = payload.x;
        self.py[i] = payload.y;
        self.pz[i] = payload.z;
        self.len = i + 1;
        self.len == LANES
    }

    #[inline(never)]
    fn open_slot(&mut self, tex: &AlphaTex, plan: &TapPlan, key: u64, ti: u32, count: bool) {
        let s = self.slots;
        debug_assert!(s < LANES);
        let lp = LanePlan::of(tex, plan);
        let tb = &mut self.table;
        tb.base0[s] = lp.base0 as u64;
        tb.base1[s] = lp.base1 as u64;
        tb.wf0[s] = lp.w0 as f32;
        tb.hf0[s] = lp.h0 as f32;
        tb.qs0[s] = lp.w0 as i32 + 1;
        tb.wf1[s] = lp.w1 as f32;
        tb.hf1[s] = lp.h1 as f32;
        tb.qs1[s] = lp.w1 as i32 + 1;
        tb.t[s] = lp.t;
        tb.two[s] = if lp.two { -1 } else { 0 };
        tb.ax[s] = lp.ax;
        tb.ay[s] = lp.ay;
        tb.nf[s] = lp.n as f32;
        tb.n[s] = lp.n as i32;
        self.refs[s] = SlotRef { tex: tex as *const AlphaTex, l0: plan.l0 as u8, l1: plan.l1 as u8, ti, count };
        self.nmax = self.nmax.max(lp.n);
        self.slots = s + 1;
        self.last_key = key;
    }

    /// The payload of pending fragment `i`.
    #[inline]
    fn pend(&self, i: usize) -> Pend {
        let r = &self.refs[self.slot_of[i] as usize];
        Pend { x: self.px[i], y: self.py[i], z: self.pz[i], ti: r.ti, count: r.count }
    }

    /// Run the test on the pending fragments and call `emit` with every PASSING payload, in push order;
    /// the queue is empty afterwards. Returns (fragments tested, fragments passed).
    #[inline]
    pub fn flush(&mut self, threshold: f32, mut emit: impl FnMut(&Pend)) -> (u32, u32) {
        let len = self.len;
        if len == 0 {
            return (0, 0);
        }
        let mut mask = self.test_mask(threshold);
        let passed = mask.count_ones();
        while mask != 0 {
            let i = mask.trailing_zeros() as usize;
            emit(&self.pend(i));
            mask &= mask - 1;
        }
        self.len = 0;
        self.slots = 0;
        self.last_key = u64::MAX;
        self.nmax = 1;
        self.stats[0] += 1;
        self.stats[1] += len as u64;
        self.stats[2] += passed as u64;
        (len as u32, passed)
    }

    /// `flush` (the end of a job).
    pub fn flush_all(&mut self, threshold: f32, emit: impl FnMut(&Pend)) {
        self.flush(threshold, emit);
    }

    /// The pass bits of the pending fragments (bit i = fragment i), the queue untouched.
    #[inline(never)]
    fn test_mask(&mut self, threshold: f32) -> u32 {
        let len = self.len;
        // the unused lanes of a partial batch read slot 0 at uv (0, 0) — valid texels, an ignored answer
        for i in len..LANES {
            self.slot_of[i] = 0;
            self.u[i] = 0.0;
            self.v[i] = 0.0;
        }
        let mask: u32 = match self.kernel {
            #[cfg(target_arch = "x86_64")]
            Kernel::Avx512 => match unsafe { avx512::test16(&self.table, &self.slot_of, &self.u, &self.v, self.nmax, threshold) } {
                Some(m) => m as u32,
                None => self.scalar_mask(threshold),
            },
            #[cfg(target_arch = "x86_64")]
            Kernel::Avx2 => match unsafe { avx2::test16(&self.table, &self.slot_of, &self.u, &self.v, self.nmax, threshold) } {
                Some(m) => m as u32,
                None => self.scalar_mask(threshold),
            },
            _ => self.scalar_mask(threshold),
        };
        mask & if len >= 32 { u32::MAX } else { (1u32 << len) - 1 }
    }

    /// A slot's plan for the scalar test, rebuilt from the slot's fields (the same levels, fraction, axis and
    /// tap count; `aniso` only decides the tap loop together with n > 1; the early-outs off).
    fn slot_plan(&self, s: usize) -> TapPlan {
        let tb = &self.table;
        let n = tb.n[s].max(1) as usize;
        TapPlan { lod: 0.0, l0: self.refs[s].l0 as usize, l1: self.refs[s].l1 as usize, two: tb.two[s] != 0, t: tb.t[s], axis: [tb.ax[s], tb.ay[s]], n, aniso: if n > 1 { 2 } else { 1 }, try_early: false }
    }

    /// The scalar test lane by lane (the fallback, and the reference of the tests).
    fn scalar_mask(&self, threshold: f32) -> u32 {
        let mut m = 0u32;
        for i in 0..self.len {
            let s = self.slot_of[i] as usize;
            // SAFETY: the queue's contract — the textures outlive the pending fragments
            let tex = unsafe { &*self.refs[s].tex };
            if tex.passes_planned(self.u[i], self.v[i], &self.slot_plan(s), threshold, Address::ClampEdge) {
                m |= 1 << i;
            }
        }
        m
    }

}

impl Default for AlphaQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// The raster hook's queues for one thread: THREE, BY TAP COUNT (n ≤ 4 / ≤ 8 / ≤ 16), so a batch's `nmax` — the kernel
/// loops to the batch's largest tap count — stays close to every lane's own. PERF 4.3 measured a per-tap-count set as
/// slower in situ, on the unaligned tree and under the screen-axis footprint where N was 2–6 almost everywhere; with the
/// ellipse (0004) the grazing cards take up to 16 taps and used to share batches with 2-tap ones. Exactness untouched: a
/// fragment's decision never depends on its batch; only the emission order changes, which every consumer sorts away.
/// LMTOOL_ALPHA_QUEUES=1 keeps one queue (the A/B).
#[repr(C, align(64))]
pub struct AlphaQueues {
    q: [AlphaQueue; 3],
    single: bool,
}

impl AlphaQueues {
    pub fn new() -> Self {
        AlphaQueues { q: [AlphaQueue::new(), AlphaQueue::new(), AlphaQueue::new()], single: single_queue() }
    }
    /// The queue a plan's fragments go to.
    #[inline(always)]
    pub fn of(&mut self, plan: &TapPlan) -> &mut AlphaQueue {
        let c = if self.single { 0 } else if plan.n <= 4 { 0 } else if plan.n <= 8 { 1 } else { 2 };
        &mut self.q[c]
    }
    /// Every queue flushed (the job's end).
    pub fn flush_all(&mut self, threshold: f32, mut emit: impl FnMut(&Pend)) {
        for q in self.q.iter_mut() {
            q.flush_all(threshold, &mut emit);
        }
    }
    /// A recycled set from this thread's pool (the raster runs one job at a time per thread).
    pub fn take() -> Box<AlphaQueues> {
        POOL.take().unwrap_or_else(|| Box::new(AlphaQueues::new()))
    }
    /// Back to the pool (empty — the caller flushed), the totals banked.
    pub fn give(mut self: Box<AlphaQueues>) {
        for q in self.q.iter_mut() {
            debug_assert!(q.is_empty());
            ALPHA_TOTALS[0].fetch_add(q.stats[0], std::sync::atomic::Ordering::Relaxed);
            ALPHA_TOTALS[1].fetch_add(q.stats[1], std::sync::atomic::Ordering::Relaxed);
            ALPHA_TOTALS[2].fetch_add(q.stats[2], std::sync::atomic::Ordering::Relaxed);
            q.stats = [0; 3];
        }
        POOL.set(Some(self));
    }
}

fn single_queue() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("LMTOOL_ALPHA_QUEUES").map(|v| v == "1").unwrap_or(false))
}

thread_local! {
    static POOL: std::cell::Cell<Option<Box<AlphaQueues>>> = const { std::cell::Cell::new(None) };
}

/// Bake-wide totals: batches, fragments tested through the queues, fragments passed (`alpha_queue_report`).
pub static ALPHA_TOTALS: [std::sync::atomic::AtomicU64; 3] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

/// LMTOOL_ALPHA_STATS: the queues' totals since the last report.
pub fn alpha_queue_report() {
    if crate::alphatex::alpha_stats_on() {
        let g = |i: usize| ALPHA_TOTALS[i].swap(0, std::sync::atomic::Ordering::Relaxed);
        let (b, t, p) = (g(0), g(1), g(2));
        eprintln!("alpha queue ({:?}): {b} batches, {t} fragments ({:.1} per batch), {p} passed ({:.1} %)", kernel(), t as f64 / b.max(1) as f64, 100.0 * p as f64 / t.max(1) as f64);
    }
}

#[cfg(target_arch = "x86_64")]
mod avx512 {
    use super::{SlotTable, LANES};
    use std::arch::x86_64::*;

    /// fl(a / 255) for the 16 byte values at byte position K of each dword of `q`: the byte broadcast to the
    /// dword (m = a·0x01010101), converted rounding toward +∞, × 2⁻³² (see the module doc; exhaustively
    /// checked in the tests).
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl,avx512dq")]
    unsafe fn unorm8<const K: i32>(q: __m512i) -> __m512 {
        let c = 0x0101_0101i32 * K;
        let ctrl = _mm512_set4_epi32(0x0c0c_0c0c + c, 0x0808_0808 + c, 0x0404_0404 + c, c);
        let m = _mm512_shuffle_epi8(q, ctrl);
        let f = _mm512_cvt_roundepu32_ps::<{ _MM_FROUND_TO_POS_INF | _MM_FROUND_NO_EXC }>(m);
        _mm512_mul_ps(f, _mm512_set1_ps(1.0 / 4294967296.0))
    }

    /// 16 quads (u32) at per-lane 64-bit addresses `base + 4·idx` (two 8-lane gathers).
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl,avx512dq")]
    unsafe fn gather_u32(idx: __m512i, base_lo: __m512i, base_hi: __m512i) -> __m512i {
        let lo = _mm512_add_epi64(base_lo, _mm512_slli_epi64::<2>(_mm512_cvtepu32_epi64(_mm512_castsi512_si256(idx))));
        let hi = _mm512_add_epi64(base_hi, _mm512_slli_epi64::<2>(_mm512_cvtepu32_epi64(_mm512_extracti64x4_epi64::<1>(idx))));
        let plo: __m256i = _mm512_i64gather_epi32::<1>(lo, std::ptr::null::<i32>());
        let phi: __m256i = _mm512_i64gather_epi32::<1>(hi, std::ptr::null::<i32>());
        _mm512_inserti64x4::<1>(_mm512_castsi256_si512(plo), phi)
    }

    /// One bilinear sample of a level for 16 lanes at the CLAMPED coordinates (cu, cv):
    /// `sample_level_clamp`'s arithmetic, lane-wise, the four texels from the quad table.
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl,avx512dq")]
    unsafe fn bilinear(cu: __m512, cv: __m512, wf: __m512, hf: __m512, qs: __m512i, base_lo: __m512i, base_hi: __m512i) -> __m512 {
        let one = _mm512_set1_ps(1.0);
        let half = _mm512_set1_ps(0.5);
        let c256 = _mm512_set1_ps(256.0);
        let inv256 = _mm512_set1_ps(1.0 / 256.0);
        let one_i = _mm512_set1_epi32(1);
        // fx = u.clamp(0, 1)·wf − 0.5
        let fx = _mm512_sub_ps(_mm512_mul_ps(cu, wf), half);
        let fy = _mm512_sub_ps(_mm512_mul_ps(cv, hf), half);
        let x0 = _mm512_roundscale_ps::<0x09>(fx);
        let y0 = _mm512_roundscale_ps::<0x09>(fy);
        let tx = _mm512_mul_ps(_mm512_roundscale_ps::<0x09>(_mm512_mul_ps(_mm512_sub_ps(fx, x0), c256)), inv256);
        let ty = _mm512_mul_ps(_mm512_roundscale_ps::<0x09>(_mm512_mul_ps(_mm512_sub_ps(fy, y0), c256)), inv256);
        // the quad at (xi + 1, yi + 1): xi = floor(fx) ∈ [−1, w − 1] after the clamp, so no index clamps
        let xi1 = _mm512_add_epi32(_mm512_cvttps_epi32(x0), one_i);
        let yi1 = _mm512_add_epi32(_mm512_cvttps_epi32(y0), one_i);
        let q = gather_u32(_mm512_add_epi32(_mm512_mullo_epi32(yi1, qs), xi1), base_lo, base_hi);
        let paa = unorm8::<0>(q);
        let pba = unorm8::<1>(q);
        let pab = unorm8::<2>(q);
        let pbb = unorm8::<3>(q);
        let omtx = _mm512_sub_ps(one, tx);
        let omty = _mm512_sub_ps(one, ty);
        let top = _mm512_add_ps(_mm512_mul_ps(paa, omtx), _mm512_mul_ps(pba, tx));
        let bot = _mm512_add_ps(_mm512_mul_ps(pab, omtx), _mm512_mul_ps(pbb, tx));
        _mm512_add_ps(_mm512_mul_ps(top, omty), _mm512_mul_ps(bot, ty))
    }

    /// The 16 lanes' pass bits (each lane its slot's tap count, up to `nmax`), or None when a coordinate is
    /// NaN (the scalar path decides those).
    #[target_feature(enable = "avx512f,avx512bw,avx512vl,avx512dq")]
    pub unsafe fn test16(tb: &SlotTable, slot_of: &[i32; LANES], us: &[f32; LANES], vs: &[f32; LANES], nmax: u32, threshold: f32) -> Option<u16> {
        let idx = _mm512_loadu_si512(slot_of.as_ptr() as *const _);
        let u = _mm512_loadu_ps(us.as_ptr());
        let v = _mm512_loadu_ps(vs.as_ptr());
        if (_mm512_cmp_ps_mask::<_CMP_UNORD_Q>(u, u) | _mm512_cmp_ps_mask::<_CMP_UNORD_Q>(v, v)) != 0 {
            return None;
        }
        let pf = |a: &[f32; LANES]| _mm512_permutexvar_ps(idx, _mm512_loadu_ps(a.as_ptr()));
        let pi = |a: &[i32; LANES]| _mm512_permutexvar_epi32(idx, _mm512_loadu_si512(a.as_ptr() as *const _));
        let idx_lo = _mm512_cvtepu32_epi64(_mm512_castsi512_si256(idx));
        let idx_hi = _mm512_cvtepu32_epi64(_mm512_extracti64x4_epi64::<1>(idx));
        let p64 = |a: &[u64; LANES], ix: __m512i| _mm512_permutex2var_epi64(_mm512_loadu_si512(a.as_ptr() as *const _), ix, _mm512_loadu_si512(a.as_ptr().add(8) as *const _));
        let (b0_lo, b0_hi) = (p64(&tb.base0, idx_lo), p64(&tb.base0, idx_hi));
        let (b1_lo, b1_hi) = (p64(&tb.base1, idx_lo), p64(&tb.base1, idx_hi));
        let (wf0, hf0, qs0) = (pf(&tb.wf0), pf(&tb.hf0), pi(&tb.qs0));
        let (wf1, hf1, qs1) = (pf(&tb.wf1), pf(&tb.hf1), pi(&tb.qs1));
        let t = pf(&tb.t);
        let two_v = pi(&tb.two);
        let two = _mm512_test_epi32_mask(two_v, two_v);
        let ax = pf(&tb.ax);
        let ay = pf(&tb.ay);
        let nf = pf(&tb.nf);
        let n = pi(&tb.n);
        let zero = _mm512_setzero_ps();
        let one = _mm512_set1_ps(1.0);
        let half = _mm512_set1_ps(0.5);
        let single = _mm512_cmpeq_epi32_mask(n, _mm512_set1_epi32(1));
        let mut sum = zero;
        for i in 0..nmax {
            // s = (i + 0.5)/n − 0.5 per lane (the scalar sampler's expression); the tap at (u + ax·s, v + ay·s) —
            // the fragment's own uv when n = 1; the clamp to [0, 1] once for both levels (the same value either way)
            let s = _mm512_sub_ps(_mm512_div_ps(_mm512_add_ps(_mm512_set1_ps(i as f32), half), nf), half);
            let uu = _mm512_mask_blend_ps(single, _mm512_add_ps(u, _mm512_mul_ps(ax, s)), u);
            let vv = _mm512_mask_blend_ps(single, _mm512_add_ps(v, _mm512_mul_ps(ay, s)), v);
            let cu = _mm512_max_ps(_mm512_min_ps(uu, one), zero);
            let cv = _mm512_max_ps(_mm512_min_ps(vv, one), zero);
            let a = bilinear(cu, cv, wf0, hf0, qs0, b0_lo, b0_hi);
            let b = bilinear(cu, cv, wf1, hf1, qs1, b1_lo, b1_hi);
            let ab = _mm512_add_ps(a, _mm512_mul_ps(_mm512_sub_ps(b, a), t));
            let val = _mm512_mask_blend_ps(two, a, ab);
            let active = _mm512_cmpgt_epi32_mask(n, _mm512_set1_epi32(i as i32));
            sum = _mm512_mask_add_ps(sum, active, sum, val);
        }
        // `sum / n` — the scalar sampler divides only when it averaged taps; x / 1.0 = x exactly, so one form
        let res = _mm512_div_ps(sum, nf);
        let pass = _mm512_cmp_ps_mask::<_CMP_GE_OQ>(_mm512_sub_ps(res, _mm512_set1_ps(threshold)), zero);
        Some(pass)
    }

    /// fl(a/255) of the 16 bytes in `a` (for the exhaustive test of `unorm8`).
    #[target_feature(enable = "avx512f,avx512bw,avx512vl,avx512dq")]
    pub unsafe fn unorm8_probe(a: &[u32; 16]) -> [[f32; 16]; 4] {
        let q = _mm512_loadu_si512(a.as_ptr() as *const _);
        let mut out = [[0f32; 16]; 4];
        _mm512_storeu_ps(out[0].as_mut_ptr(), unorm8::<0>(q));
        _mm512_storeu_ps(out[1].as_mut_ptr(), unorm8::<1>(q));
        _mm512_storeu_ps(out[2].as_mut_ptr(), unorm8::<2>(q));
        _mm512_storeu_ps(out[3].as_mut_ptr(), unorm8::<3>(q));
        out
    }
}

#[cfg(target_arch = "x86_64")]
mod avx2 {
    use super::{SlotTable, LANES};
    use std::arch::x86_64::*;

    /// A 16-entry table permute for 8 lanes of 32-bit indices 0..15.
    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn perm16_ps(idx: __m256i, a: &[f32; LANES]) -> __m256 {
        let lo = _mm256_permutevar8x32_ps(_mm256_loadu_ps(a.as_ptr()), idx);
        let hi = _mm256_permutevar8x32_ps(_mm256_loadu_ps(a.as_ptr().add(8)), idx);
        // bit 3 of the index selects the high half: move it to the sign bit for blendv
        let sel = _mm256_castsi256_ps(_mm256_slli_epi32::<28>(idx));
        _mm256_blendv_ps(lo, hi, sel)
    }
    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn perm16_epi32(idx: __m256i, a: &[i32; LANES]) -> __m256i {
        let lo = _mm256_permutevar8x32_epi32(_mm256_loadu_si256(a.as_ptr() as *const _), idx);
        let hi = _mm256_permutevar8x32_epi32(_mm256_loadu_si256(a.as_ptr().add(8) as *const _), idx);
        let sel = _mm256_castsi256_ps(_mm256_slli_epi32::<28>(idx));
        _mm256_castps_si256(_mm256_blendv_ps(_mm256_castsi256_ps(lo), _mm256_castsi256_ps(hi), sel))
    }
    /// The 4 lanes' 64-bit table entries at indices 0..15 (4 lanes of 32-bit indices in `idx4`).
    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn perm16_epi64(idx4: __m128i, a: &[u64; LANES]) -> __m256i {
        let mut ix = [0i32; 4];
        _mm_storeu_si128(ix.as_mut_ptr() as *mut _, idx4);
        _mm256_setr_epi64x(a[ix[0] as usize] as i64, a[ix[1] as usize] as i64, a[ix[2] as usize] as i64, a[ix[3] as usize] as i64)
    }

    /// fl(a / 255) for the byte SHIFT bits up in each dword: the division itself (correctly rounded, as the
    /// scalar `as f32 / 255.0`; AVX2 has no rounding-mode conversion).
    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn unorm8<const SHIFT: i32>(q: __m256i) -> __m256 {
        let a = _mm256_and_si256(_mm256_srli_epi32::<SHIFT>(q), _mm256_set1_epi32(0xff));
        _mm256_div_ps(_mm256_cvtepi32_ps(a), _mm256_set1_ps(255.0))
    }

    /// 8 quads at per-lane 64-bit addresses (two 4-lane gathers).
    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn gather_u32(idx: __m256i, base_lo: __m256i, base_hi: __m256i) -> __m256i {
        let lo = _mm256_add_epi64(base_lo, _mm256_slli_epi64::<2>(_mm256_cvtepu32_epi64(_mm256_castsi256_si128(idx))));
        let hi = _mm256_add_epi64(base_hi, _mm256_slli_epi64::<2>(_mm256_cvtepu32_epi64(_mm256_extracti128_si256::<1>(idx))));
        let plo: __m128i = _mm256_i64gather_epi32::<1>(std::ptr::null::<i32>(), lo);
        let phi: __m128i = _mm256_i64gather_epi32::<1>(std::ptr::null::<i32>(), hi);
        _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(plo), phi)
    }

    #[inline]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn bilinear(cu: __m256, cv: __m256, wf: __m256, hf: __m256, qs: __m256i, base_lo: __m256i, base_hi: __m256i) -> __m256 {
        let one = _mm256_set1_ps(1.0);
        let half = _mm256_set1_ps(0.5);
        let c256 = _mm256_set1_ps(256.0);
        let inv256 = _mm256_set1_ps(1.0 / 256.0);
        let one_i = _mm256_set1_epi32(1);
        let fx = _mm256_sub_ps(_mm256_mul_ps(cu, wf), half);
        let fy = _mm256_sub_ps(_mm256_mul_ps(cv, hf), half);
        let x0 = _mm256_round_ps::<0x09>(fx);
        let y0 = _mm256_round_ps::<0x09>(fy);
        let tx = _mm256_mul_ps(_mm256_round_ps::<0x09>(_mm256_mul_ps(_mm256_sub_ps(fx, x0), c256)), inv256);
        let ty = _mm256_mul_ps(_mm256_round_ps::<0x09>(_mm256_mul_ps(_mm256_sub_ps(fy, y0), c256)), inv256);
        let xi1 = _mm256_add_epi32(_mm256_cvttps_epi32(x0), one_i);
        let yi1 = _mm256_add_epi32(_mm256_cvttps_epi32(y0), one_i);
        let q = gather_u32(_mm256_add_epi32(_mm256_mullo_epi32(yi1, qs), xi1), base_lo, base_hi);
        let paa = unorm8::<0>(q);
        let pba = unorm8::<8>(q);
        let pab = unorm8::<16>(q);
        let pbb = unorm8::<24>(q);
        let omtx = _mm256_sub_ps(one, tx);
        let omty = _mm256_sub_ps(one, ty);
        let top = _mm256_add_ps(_mm256_mul_ps(paa, omtx), _mm256_mul_ps(pba, tx));
        let bot = _mm256_add_ps(_mm256_mul_ps(pab, omtx), _mm256_mul_ps(pbb, tx));
        _mm256_add_ps(_mm256_mul_ps(top, omty), _mm256_mul_ps(bot, ty))
    }

    /// Eight lanes (lanes `off..off+8` of the batch), each its slot's tap count up to `nmax`.
    #[target_feature(enable = "avx2,fma")]
    unsafe fn test8(tb: &SlotTable, slot_of: &[i32; LANES], us: &[f32; LANES], vs: &[f32; LANES], off: usize, nmax: u32, threshold: f32) -> Option<u8> {
        let idx = _mm256_loadu_si256(slot_of.as_ptr().add(off) as *const _);
        let u = _mm256_loadu_ps(us.as_ptr().add(off));
        let v = _mm256_loadu_ps(vs.as_ptr().add(off));
        let nan = _mm256_or_ps(_mm256_cmp_ps::<_CMP_UNORD_Q>(u, u), _mm256_cmp_ps::<_CMP_UNORD_Q>(v, v));
        if _mm256_movemask_ps(nan) != 0 {
            return None;
        }
        let (b0_lo, b0_hi) = (perm16_epi64(_mm256_castsi256_si128(idx), &tb.base0), perm16_epi64(_mm256_extracti128_si256::<1>(idx), &tb.base0));
        let (b1_lo, b1_hi) = (perm16_epi64(_mm256_castsi256_si128(idx), &tb.base1), perm16_epi64(_mm256_extracti128_si256::<1>(idx), &tb.base1));
        let (wf0, hf0, qs0) = (perm16_ps(idx, &tb.wf0), perm16_ps(idx, &tb.hf0), perm16_epi32(idx, &tb.qs0));
        let (wf1, hf1, qs1) = (perm16_ps(idx, &tb.wf1), perm16_ps(idx, &tb.hf1), perm16_epi32(idx, &tb.qs1));
        let t = perm16_ps(idx, &tb.t);
        let two = _mm256_castsi256_ps(perm16_epi32(idx, &tb.two));
        let ax = perm16_ps(idx, &tb.ax);
        let ay = perm16_ps(idx, &tb.ay);
        let nf = perm16_ps(idx, &tb.nf);
        let n = perm16_epi32(idx, &tb.n);
        let zero = _mm256_setzero_ps();
        let one = _mm256_set1_ps(1.0);
        let half = _mm256_set1_ps(0.5);
        let single = _mm256_castsi256_ps(_mm256_cmpeq_epi32(n, _mm256_set1_epi32(1)));
        let mut sum = zero;
        for i in 0..nmax {
            let s = _mm256_sub_ps(_mm256_div_ps(_mm256_add_ps(_mm256_set1_ps(i as f32), half), nf), half);
            let uu = _mm256_blendv_ps(_mm256_add_ps(u, _mm256_mul_ps(ax, s)), u, single);
            let vv = _mm256_blendv_ps(_mm256_add_ps(v, _mm256_mul_ps(ay, s)), v, single);
            let cu = _mm256_max_ps(_mm256_min_ps(uu, one), zero);
            let cv = _mm256_max_ps(_mm256_min_ps(vv, one), zero);
            let a = bilinear(cu, cv, wf0, hf0, qs0, b0_lo, b0_hi);
            let b = bilinear(cu, cv, wf1, hf1, qs1, b1_lo, b1_hi);
            let ab = _mm256_add_ps(a, _mm256_mul_ps(_mm256_sub_ps(b, a), t));
            let val = _mm256_blendv_ps(a, ab, two);
            let active = _mm256_castsi256_ps(_mm256_cmpgt_epi32(n, _mm256_set1_epi32(i as i32)));
            sum = _mm256_blendv_ps(sum, _mm256_add_ps(sum, val), active);
        }
        let res = _mm256_div_ps(sum, nf);
        let pass = _mm256_cmp_ps::<_CMP_GE_OQ>(_mm256_sub_ps(res, _mm256_set1_ps(threshold)), zero);
        Some(_mm256_movemask_ps(pass) as u8)
    }

    #[target_feature(enable = "avx2,fma")]
    pub unsafe fn test16(tb: &SlotTable, slot_of: &[i32; LANES], us: &[f32; LANES], vs: &[f32; LANES], nmax: u32, threshold: f32) -> Option<u16> {
        let lo = test8(tb, slot_of, us, vs, 0, nmax, threshold)?;
        let hi = test8(tb, slot_of, us, vs, 8, nmax, threshold)?;
        Some(lo as u16 | ((hi as u16) << 8))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::alphatex::Footprint;

    pub fn chain(w: usize, mut f: impl FnMut(usize, usize) -> u8) -> AlphaTex {
        let mut a0 = vec![0u8; w * w];
        for y in 0..w {
            for x in 0..w {
                a0[y * w + x] = f(x, y);
            }
        }
        let mut levels = vec![AlphaLevel::with_blocks(w, w, a0.clone())];
        let (mut cur, mut cw) = (a0, w);
        while cw > 1 {
            let nw = cw / 2;
            let mut n = vec![0u8; nw * nw];
            for y in 0..nw {
                for x in 0..nw {
                    let s = cur[2 * y * cw + 2 * x] as u32 + cur[2 * y * cw + 2 * x + 1] as u32 + cur[(2 * y + 1) * cw + 2 * x] as u32 + cur[(2 * y + 1) * cw + 2 * x + 1] as u32;
                    n[y * nw + x] = (s / 4) as u8;
                }
            }
            levels.push(AlphaLevel::with_blocks(nw, nw, n.clone()));
            cur = n;
            cw = nw;
        }
        AlphaTex { levels, flipped: false }
    }

    pub fn textures() -> Vec<AlphaTex> {
        let mut seed = 0x9e3779b97f4a7c15u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 100_000) as f32 / 100_000.0
        };
        let blotch = |x: usize, y: usize, w: f32, r: &mut dyn FnMut() -> f32| {
            let d = (((x as f32 - w / 2.0).powi(2) + (y as f32 - w / 2.0).powi(2)).sqrt() / (w / 2.2)).min(1.0);
            ((1.0 - d) * 255.0 + (r() - 0.5) * 60.0).clamp(0.0, 255.0) as u8
        };
        vec![
            // a card-like blotch (64²), a threshold-hugging noise field (64²), a texture of exact 128s and
            // 127/129 (ties), a 256² leaf-like field, a non-square 128×32 texture
            chain(64, |x, y| blotch(x, y, 64.0, &mut rnd)),
            chain(64, |x, y| (128.0 + (rnd() - 0.5) * 40.0 + ((x + y) % 2) as f32 * 3.0) as u8),
            chain(32, |x, y| match (x / 4 + y / 4) % 3 { 0 => 128, 1 => 127, _ => 129 }),
            chain(256, |x, y| blotch(x % 128, y % 128, 128.0, &mut rnd)),
            {
                let (w, h) = (128usize, 32usize);
                let mut a0 = vec![0u8; w * h];
                for y in 0..h { for x in 0..w { a0[y * w + x] = ((x * 2 + y * 7) % 256) as u8; } }
                let mut levels = vec![AlphaLevel::with_blocks(w, h, a0.clone())];
                let (mut cur, mut cw, mut ch) = (a0, w, h);
                while cw > 1 || ch > 1 {
                    let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
                    let mut n = vec![0u8; nw * nh];
                    for y in 0..nh { for x in 0..nw { let (sx, sy) = ((2 * x).min(cw - 1), (2 * y).min(ch - 1)); let (sx1, sy1) = ((2 * x + 1).min(cw - 1), (2 * y + 1).min(ch - 1)); n[y * nw + x] = ((cur[sy * cw + sx] as u32 + cur[sy * cw + sx1] as u32 + cur[sy1 * cw + sx] as u32 + cur[sy1 * cw + sx1] as u32) / 4) as u8; } }
                    levels.push(AlphaLevel::with_blocks(nw, nh, n.clone()));
                    cur = n; cw = nw; ch = nh;
                }
                AlphaTex { levels, flipped: false }
            },
        ]
    }

    fn set_kernel(qs: &mut AlphaQueues, k: Kernel) {
        for q in qs.q.iter_mut() {
            q.kernel = k;
        }
    }

    /// Random plans (footprints from lod −1 to the last level, isotropic and anisotropic) and coordinates
    /// (uniform, on texel centres and edges, outside [0, 1]) through the queues, against the scalar tests.
    fn check(kernel: Kernel) {
        let texs = textures();
        let thr = crate::peel::ALPHA_THRESHOLD;
        let mut seed = 0x2545f4914f6cdd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 1_000_003) as f32 / 1_000_003.0
        };
        let mut qs = AlphaQueues::new();
        set_kernel(&mut qs, kernel);
        let mut n_pass = 0usize;
        let mut n_all = 0usize;
        let mut key = 0u64;
        for round in 0..20000 {
            // a band: a few triangles (consecutive fragments share a plan), each on a random texture; the
            // fragments go to the queues, full batches flush as the raster would, the rest at the end
            let mut expected: Vec<(u32, bool)> = Vec::new();
            let mut got: Vec<u32> = Vec::new();
            let mut pushed = 0u32;
            let band = 1 + (rnd() * 40.0) as usize;
            while (pushed as usize) < band {
                let tex = &texs[(rnd() * texs.len() as f32) as usize % texs.len()];
                let scale = 2f32.powf(rnd() * 9.0 - 1.0);
                let stretch = 2f32.powf(rnd() * 5.0);
                let ang = rnd() * 6.2832;
                let (c, s) = (ang.cos(), ang.sin());
                let fp = Footprint { dx: [c * scale * stretch, s * scale * stretch], dy: [-s * scale, c * scale], w: tex.w() as f32, h: tex.h() as f32 };
                let aniso = if rnd() < 0.8 { 16 } else { 1 };
                let plan = tex.plan(&fp, aniso);
                key += 1;
                let frags = 1 + (rnd() * 6.0) as usize;
                for _ in 0..frags {
                    if pushed as usize >= band {
                        break;
                    }
                    let mode = rnd();
                    let lw = tex.levels[plan.l0].w as f32;
                    let lh = tex.levels[plan.l0].h as f32;
                    let (u, v) = if mode < 0.5 {
                        (rnd(), rnd())
                    } else if mode < 0.75 {
                        // texel centres / edges of the plan's level (ties on the 8-bit weights)
                        (((rnd() * lw).floor() + if rnd() < 0.5 { 0.5 } else { 0.0 }) / lw, ((rnd() * lh).floor() + if rnd() < 0.5 { 0.5 } else { 0.0 }) / lh)
                    } else if mode < 0.9 {
                        (rnd() * 1.4 - 0.2, rnd() * 1.4 - 0.2)
                    } else {
                        (f32::from_bits((rnd() * 4.0e9) as u32 & 0x3fff_ffff), rnd())
                    };
                    let want = tex.passes_planned(u, v, &plan, thr, Address::ClampEdge);
                    assert_eq!(want, tex.passes_sampled(u, v, &fp, thr, Address::ClampEdge, aniso), "planned vs sampled disagree");
                    expected.push((pushed, want));
                    let q = qs.of(&plan);
                    if q.push(tex, &plan, key, u, v, Pend { x: pushed, ..Pend::default() }) {
                        q.flush(thr, |p| got.push(p.x));
                    }
                    pushed += 1;
                }
            }
            qs.flush_all(thr, |p| got.push(p.x));
            got.sort_unstable();
            let want: Vec<u32> = expected.iter().filter(|(_, w)| *w).map(|(i, _)| *i).collect();
            assert_eq!(got, want, "round {round} kernel {kernel:?}");
            n_pass += want.len();
            n_all += expected.len();
        }
        assert!(n_pass > n_all / 10 && n_pass < n_all * 9 / 10, "{n_pass} of {n_all} pass: a degenerate test");
        eprintln!("{kernel:?}: {n_pass} of {n_all} fragments pass, all as the scalar test");
    }

    #[test]
    fn the_queue_answers_as_the_scalar_test_scalar() {
        check(Kernel::Scalar);
    }
    #[test]
    fn the_queue_answers_as_the_scalar_test_avx2() {
        if kernel() == Kernel::Scalar {
            eprintln!("no AVX2 here");
            return;
        }
        check(Kernel::Avx2);
    }
    #[test]
    fn the_queue_answers_as_the_scalar_test_avx512() {
        if kernel() != Kernel::Avx512 {
            eprintln!("no AVX-512 here");
            return;
        }
        check(Kernel::Avx512);
    }

    #[test]
    fn the_quad_table_holds_the_clamped_neighbourhood() {
        for tex in textures() {
            for l in &tex.levels {
                assert_eq!(l.quad_stride as usize, l.w + 1);
                assert_eq!(l.quad.len(), (l.w + 1) * (l.h + 1));
                for yi in -1..l.h as i64 {
                    for xi in -1..l.w as i64 {
                        let q = l.quad[(yi + 1) as usize * (l.w + 1) + (xi + 1) as usize];
                        let cx = |x: i64| x.clamp(0, l.w as i64 - 1) as usize;
                        let cy = |y: i64| y.clamp(0, l.h as i64 - 1) as usize;
                        let want = [l.a[cy(yi) * l.w + cx(xi)], l.a[cy(yi) * l.w + cx(xi + 1)], l.a[cy(yi + 1) * l.w + cx(xi)], l.a[cy(yi + 1) * l.w + cx(xi + 1)]];
                        assert_eq!(q.to_le_bytes(), want, "level {}×{} at ({xi}, {yi})", l.w, l.h);
                    }
                }
            }
        }
    }

    #[test]
    fn the_byte_conversion_is_the_division_for_every_byte() {
        if kernel() != Kernel::Avx512 {
            eprintln!("no AVX-512 here");
            return;
        }
        for base in (0u32..256).step_by(16) {
            // every byte position of the dword carries a different value
            let mut a = [0u32; 16];
            for i in 0..16 {
                let b0 = base + i as u32;
                a[i] = b0 | ((255 - b0) << 8) | (((b0 * 7) % 256) << 16) | (((b0 * 13 + 5) % 256) << 24);
            }
            let got = unsafe { avx512::unorm8_probe(&a) };
            for i in 0..16 {
                for k in 0..4 {
                    let byte = (a[i] >> (8 * k)) & 0xff;
                    let want = byte as f32 / 255.0;
                    assert_eq!(got[k][i].to_bits(), want.to_bits(), "byte {byte} at position {k}: {} vs {}", got[k][i], want);
                }
            }
        }
    }

    #[test]
    fn a_nan_coordinate_takes_the_scalar_path() {
        let texs = textures();
        let thr = crate::peel::ALPHA_THRESHOLD;
        let tex = &texs[0];
        let fp = Footprint { dx: [2.0, 0.0], dy: [0.0, 2.0], w: 64.0, h: 64.0 };
        let plan = tex.plan(&fp, 16);
        for k in [Kernel::Scalar, Kernel::Avx2, Kernel::Avx512] {
            if k != Kernel::Scalar && kernel() == Kernel::Scalar || k == Kernel::Avx512 && kernel() != Kernel::Avx512 {
                continue;
            }
            let mut q = AlphaQueue::new();
            q.kernel = k;
            q.push(tex, &plan, 1, f32::NAN, 0.5, Pend { x: 1, ..Pend::default() });
            q.push(tex, &plan, 1, 0.5, 0.5, Pend { x: 2, ..Pend::default() });
            q.push(tex, &plan, 1, 0.5, f32::NAN, Pend { x: 3, ..Pend::default() });
            let mut got = Vec::new();
            q.flush(thr, |p| got.push(p.x));
            let mut want = Vec::new();
            for (u, v, i) in [(f32::NAN, 0.5, 1), (0.5, 0.5, 2), (0.5, f32::NAN, 3)] {
                if tex.passes_planned(u, v, &plan, thr, Address::ClampEdge) {
                    want.push(i);
                }
            }
            assert_eq!(got, want, "{k:?}");
            assert!(q.is_empty());
        }
    }

    fn bench_plans(tex: &AlphaTex, rnd: &mut impl FnMut() -> f32) -> Vec<TapPlan> {
        (0..64)
            .map(|_| {
                let scale = 2f32.powf(4.0 + rnd() * 1.5);
                let stretch = 1.0 + rnd() * 2.5;
                let ang = rnd() * 6.2832;
                let (c, s) = (ang.cos(), ang.sin());
                let fp = Footprint { dx: [c * scale * stretch, s * scale * stretch], dy: [-s * scale, c * scale], w: 256.0, h: 256.0 };
                tex.plan(&fp, 16)
            })
            .collect()
    }

    /// cargo test --release -p lightmap alpha_bench -- --nocapture --ignored: the kernels' throughput on the
    /// giant's regime (a 256² texture at levels 4–5, 2–3 taps, five fragments per triangle), whole queue and
    /// its parts (the push path alone, the kernel alone on one fixed batch).
    #[test]
    #[ignore]
    fn alpha_bench() {
        let texs = textures();
        let tex = &texs[3];
        let thr = crate::peel::ALPHA_THRESHOLD;
        let mut seed = 0x1234_5678_9abc_def1u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 1_000_003) as f32 / 1_000_003.0
        };
        let plans = bench_plans(tex, &mut rnd);
        let n = 4_000_000usize;
        let frags: Vec<(u32, f32, f32)> = (0..n).map(|i| ((i / 5) as u32 % 64, rnd(), rnd())).collect();
        eprintln!("mean taps {:.2}, two-level {:.2}", plans.iter().map(|p| p.n as f32).sum::<f32>() / 64.0, plans.iter().filter(|p| p.two).count() as f32 / 64.0);
        let t0 = std::time::Instant::now();
        let mut pass = 0usize;
        for &(pi, u, v) in &frags {
            pass += tex.passes_planned(u, v, &plans[pi as usize], thr, Address::ClampEdge) as usize;
        }
        let scalar = t0.elapsed();
        eprintln!("scalar passes_planned: {:.1} ns per fragment ({pass} pass)", scalar.as_nanos() as f64 / n as f64);
        for k in [Kernel::Scalar, Kernel::Avx2, Kernel::Avx512] {
            if k != Kernel::Scalar && kernel() == Kernel::Scalar || k == Kernel::Avx512 && kernel() != Kernel::Avx512 {
                continue;
            }
            let mut qs = AlphaQueues::new();
            set_kernel(&mut qs, k);
            let t0 = std::time::Instant::now();
            let mut pass2 = 0usize;
            for (g, group) in frags.chunks(5).enumerate() {
                let pi = group[0].0 as usize;
                let plan = &plans[pi];
                let q = qs.of(plan);
                for (j, &(_, u, v)) in group.iter().enumerate() {
                    if q.push(tex, plan, g as u64, u, v, Pend { x: (g * 5 + j) as u32, ..Pend::default() }) {
                        q.flush(thr, |_| pass2 += 1);
                    }
                }
            }
            qs.flush_all(thr, |_| pass2 += 1);
            let dt = t0.elapsed();
            assert_eq!(pass, pass2);
            // the push path alone
            let mut qs = AlphaQueues::new();
            set_kernel(&mut qs, k);
            let t1 = std::time::Instant::now();
            for (g, group) in frags.chunks(5).enumerate() {
                let pi = group[0].0 as usize;
                let plan = &plans[pi];
                let q = qs.of(plan);
                for (j, &(_, u, v)) in group.iter().enumerate() {
                    if q.push(tex, plan, g as u64, u, v, Pend { x: (g * 5 + j) as u32, ..Pend::default() }) {
                        q.len = 0;
                        q.slots = 0;
                        q.last_key = u64::MAX;
                        q.nmax = 1;
                    }
                }
            }
            let push_ns = t1.elapsed().as_nanos() as f64 / n as f64;
            // the kernel alone on one full batch of three-tap fragments
            let mut q = AlphaQueue::new();
            q.kernel = k;
            let p3: Vec<usize> = (0..64).filter(|&i| plans[i].n == 3).collect();
            for (i, &(_, u, v)) in frags[..16].iter().enumerate() {
                let pi = p3[(i / 5) % p3.len()];
                q.push(tex, &plans[pi], (i as u64 / 5) * 100, u, v, Pend { x: i as u32, ..Pend::default() });
            }
            let reps = 250_000;
            let t2 = std::time::Instant::now();
            let mut acc = 0u32;
            for _ in 0..reps {
                acc = acc.wrapping_add(q.test_mask(thr));
                q.u[0] = f32::from_bits(q.u[0].to_bits() ^ (acc & 1));
            }
            let kern_ns = t2.elapsed().as_nanos() as f64 / (reps * 16) as f64;
            eprintln!("queue {k:?}: {:.1} ns per fragment ({pass2} pass); push alone {push_ns:.1} ns, kernel alone (3 taps) {kern_ns:.1} ns ({acc})", dt.as_nanos() as f64 / n as f64);
        }
    }
}
