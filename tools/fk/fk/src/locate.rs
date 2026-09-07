//! Gathering the car's state per tick out of a fork child.
//!
//! The car itself is DERIVED, not located: `forkoracle::car::locate` walks the
//! pointers the physics step follows to the dyna body the solver integrates,
//! and its `Layout` names the addresses the sampler gathers and the clock word
//! that stamps them (`sim+0x48`, the tick loop's own). What lives here is only
//! the gather: one record per tick, keyed on the clock so a tick in which the
//! car does not move (a respawn freeze, the countdown) can never be dropped and
//! several samples inside one tick collapse to the last.
//!
//! Three sweeps used to live in this file -- a clock hunt by the `+10 every
//! tick` signature, a candidate scan for position-shaped triples and a grader
//! for one address. They found copies (`LOCATE.md` names every one) and could
//! not tell them from the car; the derivation can, and they are gone.

use forkoracle::forksrv::{ForkServer, Rec};
use forkoracle::layout::{Layout, Row, R_POS, R_QUAT, R_VEL, REC_LEN};

/// Bit 31 of the sample budget: the child exits when the budget is spent.
pub const EXIT_ON_BUDGET: u32 = 0x8000_0000;


/// A budget for a child that must run `ticks` more ticks, in the shim's clock
/// units (ticks).
pub fn budget_for(ticks: u32) -> u32 {
    forkoracle::clock::budget_for_ticks(ticks)
}

fn getf32(b: &[u8], o: usize) -> f64 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64
}


/// One sampled tick: the race-clock value and the gathered record.
pub struct Tick {
    pub clock: u32,
    pub rec: Vec<u8>,
}

/// Sample `segs` for `ticks` ticks with the clock as segment 0, and return one
/// record per distinct clock value (the LAST one written in that tick).
///
/// `key` is the dedup key within the record; pass `(0, reclen)` (the whole
/// record) when the clock is inside it, which is the only configuration that
/// cannot silently drop a tick.
pub fn gather_ticks(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    segs: &[(u64, u32)],
    ticks: u32,
    max_samples: u32,
    key: (u32, u32),
) -> Vec<Tick> {
    let reclen: usize = segs.iter().map(|s| s.1 as usize).sum();
    // Only the ticks the child will actually simulate are sent. The patch list
    // used to be the whole remaining tape -- 43 000 records, 688 KB, written
    // down a pipe and copied into the child's input array FOR EVERY PROBE --
    // which is most of what made a locate on a long tape unaffordable.
    let keep = ((ticks as usize) + 64).min(recs.len());
    let (_j, blob) = srv.run_sampled_segs_ex(
        probe,
        &recs[..keep],
        segs,
        1,
        max_samples | EXIT_ON_BUDGET,
        key,
        budget_for(ticks),
    );
    let recsz = 8 + reclen;
    let m = if recsz > 0 { blob.len() / recsz } else { 0 };
    let mut out: Vec<Tick> = Vec::with_capacity(m);
    for i in 0..m {
        let b = &blob[i * recsz + 8..i * recsz + 8 + reclen];
        let clk = u32::from_le_bytes(b[0..4].try_into().unwrap());
        match out.last_mut() {
            Some(t) if t.clock == clk => t.rec.copy_from_slice(b),
            _ => out.push(Tick {
                clock: clk,
                rec: b.to_vec(),
            }),
        }
    }
    out
}

/// A located clock: its address and `value - race_ms`.
/// Extract the whole trajectory with a located layout, one row per tick.
pub fn trajectory(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    l: &Layout,
    ticks: u32,
) -> Vec<Row> {
    let segs = forkoracle::layout::segments(l);
    let ts = gather_ticks(srv, probe, recs, &segs, ticks, 200_000, (0, REC_LEN as u32));
    ts.iter()
        .map(|t| Row {
            time_ms: t.clock as i64 - l.clock_bias,
            x: getf32(&t.rec, R_POS),
            y: getf32(&t.rec, R_POS + 4),
            z: getf32(&t.rec, R_POS + 8),
            vx: getf32(&t.rec, R_VEL),
            vy: getf32(&t.rec, R_VEL + 4),
            vz: getf32(&t.rec, R_VEL + 8),
            qw: getf32(&t.rec, R_QUAT),
            qx: getf32(&t.rec, R_QUAT + 4),
            qy: getf32(&t.rec, R_QUAT + 8),
            qz: getf32(&t.rec, R_QUAT + 12),
            wetness: getf32(&t.rec, forkoracle::layout::R_WET),
        })
        .collect()
}

/// `gather_ticks`, but keeping only every `stride`-th tick.
///
/// The child still samples every tick (the clock is the dedup key); the driver
/// thins the result. That costs a little pipe traffic and buys a phase-1 window
/// wide enough for the car to have moved, which is what the "is this a position
/// triple" filter needs.
pub fn gather_ticks_stride(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    segs: &[(u64, u32)],
    ticks: u32,
    max_samples: u32,
    key: (u32, u32),
    stride: u32,
) -> Vec<Tick> {
    if stride <= 1 {
        return gather_ticks(srv, probe, recs, segs, ticks, max_samples, key);
    }
    let all = gather_ticks(srv, probe, recs, segs, ticks, max_samples * stride, key);
    all.into_iter()
        .enumerate()
        .filter(|(i, _)| i % stride as usize == 0)
        .map(|(_, t)| t)
        .collect()
}

// ---------------------------------------------------------------------------
