//! Tick-aligned sampling and structural checks for validator-owned state.
//!
//! Controlled-car identity does not live here. [`crate::validator::ValidatorCar`]
//! follows the validator's ownership path and then uses these helpers to locate
//! the race clock and reject stale or malformed state. The clock scan labels
//! samples; it never chooses a car.
//!
//! The race clock is found from its exact +10-per-tick signature. Including that
//! clock in every sampled record prevents stationary ticks from disappearing and
//! lets repeated writes within one tick collapse onto the final state.

use forkoracle::forksrv::{ForkServer, Rec};
use forkoracle::layout::{sample_ms, Layout, Row, REC_LEN, R_POS, R_QUAT, R_VEL};

/// Bit 31 of the sample budget: the child exits when the budget is spent.
pub const EXIT_ON_BUDGET: u32 = 0x8000_0000;

/// `lroundf` calls the engine makes per 10 ms tick, with margin. Measured:
/// clock = 36141 + 25.483 * race_ms, i.e. ~255 per tick.
pub fn budget_for(ticks: u32) -> u32 {
    ticks.saturating_mul(340).saturating_add(12000)
}

fn getf32(b: &[u8], o: usize) -> f64 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64
}

fn median(v: &mut Vec<f64>) -> f64 {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return f64::MAX;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
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
#[derive(Clone, Copy, Debug)]
pub struct ClockHit {
    pub addr: u64,
    pub bias: i64,
}

/// Every writable, readable region of the paused parent, largest first cut off
/// at `cap` bytes total, in address order.
fn writable_regions(pid: i32) -> Vec<forkoracle::procmem::Region> {
    forkoracle::procmem::maps(pid)
        .into_iter()
        .filter(|r| {
            r.perms.contains('w')
                && r.perms.contains('r')
                && r.end > r.start
                && r.end - r.start
                    <= std::env::var("FK_REGION_CAP")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1u64 << 30)
                && !r.path.starts_with("/dev")
        })
        .collect()
}

/// STEP 1: the race clock, with no reference and no assumed offset.
///
/// A value scan of the paused parent for u32 slots within `bias_max` ms of the
/// known race time, then the +10-every-tick test on the survivors, eight at a
/// time (the gather takes up to 8 segments, so eight candidates cost one fork).
/// STEP 1: the race clock, with no reference, no assumed offset, and no
/// assumption about what the clock READS.
///
/// The first version of this filtered candidates by value (a u32 within a
/// couple of seconds of the known race time). That failed on 284238: nothing
/// within 2000 ms of the race time ticks by 10, so the engine's clock is not
/// simply "race milliseconds" on this build/map, and a value prior is exactly
/// the kind of assumption that turns a locator into a map-specific tool.
///
/// What replaces it assumes only the SIGNATURE. Each mapped window is sampled
/// at four instants a few ticks apart (four snapshots, no dedup, then the child
/// exits), and a slot survives if every one of its deltas is a positive
/// multiple of 10 of the right order of magnitude. Survivors then face the
/// strict per-tick test.
pub fn find_clock2(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    start_offset_ms: i32,
    _bias_max: i64,
    verbose: bool,
) -> Result<ClockHit, String> {
    let race0 = sample_ms(probe, 0, start_offset_ms);
    let base = srv.base;
    // Snapshot spacing: ~10 ticks between samples, four samples.
    const GAP_TICKS: u64 = 10;
    const NSNAP: u32 = 4;
    let slice: u32 = 1 << 20;
    let mut wins: Vec<u64> = Vec::new();
    for r in writable_regions(srv.pid()) {
        let mut a = (r.start + 0xFFF) & !0xFFF;
        while a + slice as u64 <= r.end {
            wins.push(a);
            a += slice as u64;
        }
        // the tail of a region that is not a whole slice wide
        if r.end > r.start + 0x1000 && (r.end - ((r.start + 0xFFF) & !0xFFF)) % slice as u64 > 4096
        {
            let a = (r.end - ((r.end - ((r.start + 0xFFF) & !0xFFF)) % slice as u64)) & !0xFFF;
            if a >= r.start && a + 4096 <= r.end {
                wins.push(a);
            }
        }
    }
    wins.sort_by_key(|w| (*w as i64 - base as i64).unsigned_abs());
    wins.dedup();
    if verbose {
        println!(
            "clock scan: {} mapped windows of {} KB, nearest to the input array first",
            wins.len(),
            slice / 1024
        );
    }
    let t0 = std::time::Instant::now();
    let mut cands: Vec<u64> = Vec::new();
    let mut scanned = 0usize;
    for w in &wins {
        scanned += 1;
        let ticks = (GAP_TICKS as u32) * NSNAP + 8;
        let keep = ((ticks as usize) + 64).min(recs.len());
        let (_j, blob) = srv.run_sampled_segs_ex(
            probe,
            &recs[..keep],
            &[(*w, slice)],
            255 * GAP_TICKS,
            NSNAP | EXIT_ON_BUDGET,
            (0, 0), // no dedup: a fixed number of snapshots
            budget_for(ticks),
        );
        let recsz = 8 + slice as usize;
        let m = blob.len() / recsz;
        if m < 3 {
            continue;
        }
        let g = |i: usize, o: usize| -> u32 {
            u32::from_le_bytes(
                blob[i * recsz + 8 + o..i * recsz + 12 + o]
                    .try_into()
                    .unwrap(),
            )
        };
        for o in (0..slice as usize - 4).step_by(4) {
            let mut ok = true;
            let mut dmin = u32::MAX;
            let mut dmax = 0u32;
            for i in 0..m - 1 {
                let d = g(i + 1, o).wrapping_sub(g(i, o));
                if d % 10 != 0 || d < 10 || d > 40 * 10 * (GAP_TICKS as u32) {
                    ok = false;
                    break;
                }
                dmin = dmin.min(d);
                dmax = dmax.max(d);
            }
            if ok && dmax - dmin <= 40 {
                cands.push(*w + o as u64);
            }
        }
        if cands.len() >= 8 {
            break;
        }
    }
    if verbose {
        println!(
            "clock scan: {} slots step by a multiple of 10 across {} windows ({:.1}s)",
            cands.len(),
            scanned,
            t0.elapsed().as_secs_f64()
        );
    }
    if cands.is_empty() {
        return Err("no slot advances by a multiple of 10 per tick: race clock not located".into());
    }
    confirm_clock(srv, probe, recs, race0, base, &cands, verbose)
}

#[allow(clippy::too_many_arguments)]
fn confirm_clock(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    race0: i64,
    base: u64,
    cands: &[u64],
    verbose: bool,
) -> Result<ClockHit, String> {
    let mut cands: Vec<u64> = cands.to_vec();
    cands.sort_by_key(|a| (*a as i64 - base as i64).unsigned_abs());
    if cands.is_empty() {
        return Err("no clock candidate to confirm".into());
    }
    let ticks: u32 = 40;
    let cap: usize = std::env::var("FK_CLOCK_MAX_PROBES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4000);
    let mut confirmed: Vec<ClockHit> = Vec::new();
    let mut nonempty = 0usize;
    let mut maxticks = 0usize;
    let t0 = std::time::Instant::now();
    for (ci, chunk) in cands.chunks(8).enumerate() {
        if ci >= cap {
            return Err(format!(
                "clock not found in the nearest {} candidates ({} scanned of {})",
                ci * 8,
                ci * 8,
                cands.len()
            ));
        }
        if verbose && ci > 0 && ci % 500 == 0 {
            println!(
                "  clock probes {} of {} ({:.1}s)",
                ci * 8,
                cands.len(),
                t0.elapsed().as_secs_f64()
            );
        }
        let segs: Vec<(u64, u32)> = chunk.iter().map(|a| (*a, 4u32)).collect();
        let reclen = (segs.len() * 4) as u32;
        // No dedup key on the clock (we do not know which slot it is yet):
        // key on the whole record, so a tick can only be missed if NOTHING in
        // the record changes -- and then there is no clock in it either.
        let ts = gather_ticks(srv, probe, recs, &segs, ticks, ticks * 8, (0, reclen));
        maxticks = maxticks.max(ts.len());
        if ts.len() >= 20 {
            nonempty += 1;
        } else {
            continue;
        }
        for (j, a) in chunk.iter().enumerate() {
            let vals: Vec<u32> = ts
                .iter()
                .map(|t| u32::from_le_bytes(t.rec[j * 4..j * 4 + 4].try_into().unwrap()))
                .collect();
            let mut tens = 0usize;
            let mut ok = true;
            for w in vals.windows(2) {
                let d = w[1].wrapping_sub(w[0]);
                if d == 10 {
                    tens += 1;
                } else if d != 0 {
                    ok = false;
                    break;
                }
            }
            if ok && tens >= 15 {
                confirmed.push(ClockHit {
                    addr: *a,
                    bias: vals[0] as i64 - race0,
                });
            }
        }
        if !confirmed.is_empty() {
            break;
        }
    }
    let hit = *confirmed.first().ok_or_else(|| {
        format!(
            "no u32 advances by exactly +10 per tick: race clock not located \
             ({} of {} probe forks returned >=20 ticks, deepest {} ticks)",
            nonempty,
            cands.len().div_ceil(8),
            maxticks
        )
    })?;
    // Deep confirm: 400 ticks, nothing but 0 and +10.
    let ts = gather_ticks(srv, probe, recs, &[(hit.addr, 4)], 400, 4000, (0, 4));
    let vals: Vec<u32> = ts
        .iter()
        .map(|t| u32::from_le_bytes(t.rec[0..4].try_into().unwrap()))
        .collect();
    let bad = vals
        .windows(2)
        .filter(|w| {
            let d = w[1].wrapping_sub(w[0]);
            d != 10 && d != 0
        })
        .count();
    if vals.len() < 50 || bad > 0 {
        return Err(format!(
            "clock candidate {:#x} failed the deep confirm: {} samples, {} bad steps",
            hit.addr,
            vals.len(),
            bad
        ));
    }
    if verbose {
        println!(
            "CLOCK {:#014x} (base{:+}) bias {:+} ms, confirmed over {} ticks",
            hit.addr,
            hit.addr as i64 - base as i64,
            hit.bias,
            vals.len()
        );
    }
    Ok(hit)
}

/// A vehicle-state candidate and the evidence for it.
#[derive(Clone, Debug)]
pub struct PosHit {
    pub pos: u64,
    /// median |d(pos)/dt - v| over the window, m/s -- robust to one impact
    pub verr: f64,
    /// max |q| - 1 over the window: an independent structural signature
    pub qerr: f64,
    /// Fraction of consecutive TICKS on which the quaternion actually CHANGES.
    ///
    /// RULE 3. A norm-only test cannot tell an identity rotation from a zeroed
    /// slot, and cannot tell the live vehicle state from a shadow object that
    /// carries no attitude at all -- both satisfy |q| = 1. Measured on three
    /// maps (267460, 191465, 227969), the live car is the ONLY candidate whose
    /// quaternion both has unit norm and varies; the back buffer, the bare
    /// position copy and the two 0.494 mm shadows all fail it. See
    /// `whl_BUFFER_v3` / `v7`.
    ///
    /// Counted across genuine 10 ms tick steps, never across raw records: the
    /// engine writes the state ~53 times inside one tick, so counting raw rows
    /// makes a varying quaternion look constant on 96 % of them.
    pub qvary: f64,
    /// Total angular travel over the window, radians. On a map where the car
    /// barely rotates the quaternion drifts only in the fifth decimal and
    /// `qvary` passes by a hair (measured on 191465), so RANK by how far the
    /// attitude actually moved rather than thresholding on "did it change".
    pub qtravel: f64,
    pub mean_speed: f64,
    pub ticks: usize,
    pub first: (f64, f64, f64),
}

/// Deep test of one candidate: gather `[clock | q(16) | pos(12) | vel(12)]` for
/// `ticks` ticks, one row per clock value, and judge it on two independent
/// signatures.
pub fn qualify2(
    srv: &mut ForkServer,
    probe: usize,
    recs: &[Rec],
    clock: u64,
    pos: u64,
    ticks: u32,
    bounds: (f64, f64, f64, f64, f64, f64),
) -> Option<PosHit> {
    let segs = [(clock, 4u32), (pos - 16, 40u32)];
    let ts = gather_ticks(
        srv,
        probe,
        recs,
        &segs,
        ticks,
        ticks * 8,
        (0, REC_LEN as u32),
    );
    if ts.len() < 20 {
        return None;
    }
    let (xlo, xhi, ylo, yhi, zlo, zhi) = bounds;
    // FK_ANCHOR="x,y,z,r" -- accept only a slot whose FIRST usable sample sits
    // within r metres of a position we KNOW the car occupies at the fork tick
    // (on a map with no ghosts, the map's own Spawn item). Self-consistency
    // alone cannot tell the car from a falling prop; this can, and it is
    // ground truth rather than a threshold.
    let anchor: Option<(f64, f64, f64, f64)> = std::env::var("FK_ANCHOR").ok().and_then(|s| {
        let v: Vec<f64> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() == 4 {
            Some((v[0], v[1], v[2], v[3]))
        } else {
            None
        }
    });
    let mut first_pos: Option<(f64, f64, f64)> = None;
    let mut verrs: Vec<f64> = Vec::with_capacity(ts.len());
    let mut qerrs: Vec<f64> = Vec::with_capacity(ts.len());
    let mut qmoved = 0usize;
    let mut qsame = 0usize;
    let mut qtrav = 0.0f64;
    let mut speed = 0.0;
    let mut n = 0usize;
    let mut outliers = 0usize;
    let mut rows = 0usize;
    for w in ts.windows(2) {
        if (w[1].clock.wrapping_sub(w[0].clock)) != 10 {
            continue;
        }
        rows += 1;
        let dt = 0.01;
        let p = |t: &Tick, k: usize| getf32(&t.rec, R_POS + k);
        let (x, y, z) = (p(&w[0], 0), p(&w[0], 4), p(&w[0], 8));
        // OUTLIER, NOT DISQUALIFICATION. A single tick can legitimately be a
        // teleport: a RESPAWN moves the car tens of metres in one tick, and
        // the record this project works from has 31 of them. The first version
        // of this test returned None on any step over 400 m/s, so on a
        // respawning map the real vehicle state was thrown away and only the
        // lagged copies -- which do not follow the teleport -- survived. Bad
        // rows are counted and skipped; too many of them condemn the slot.
        let bad_pos = !(x.is_finite() && y.is_finite() && z.is_finite())
            || x < xlo
            || x > xhi
            || y < ylo
            || y > yhi
            || z < zlo
            || z > zhi;
        if bad_pos {
            outliers += 1;
            continue;
        }
        if first_pos.is_none() {
            first_pos = Some((x as f64, y as f64, z as f64));
        }
        let (dx, dy, dz) = (p(&w[1], 0) - x, p(&w[1], 4) - y, p(&w[1], 8) - z);
        let step = (dx * dx + dy * dy + dz * dz).sqrt();
        if !step.is_finite() || step / dt > 400.0 {
            outliers += 1;
            continue;
        }
        let v = |t: &Tick, k: usize| getf32(&t.rec, R_VEL + k);
        // The integration convention is not assumed: the derivative over the
        // step is compared with the velocity at BOTH ends and the better one
        // taken, so a hard acceleration inside the tick cannot condemn a slot.
        let e0 = ((dx / dt - v(&w[0], 0)).powi(2)
            + (dy / dt - v(&w[0], 4)).powi(2)
            + (dz / dt - v(&w[0], 8)).powi(2))
        .sqrt();
        let e1 = ((dx / dt - v(&w[1], 0)).powi(2)
            + (dy / dt - v(&w[1], 4)).powi(2)
            + (dz / dt - v(&w[1], 8)).powi(2))
        .sqrt();
        if !e0.is_finite() && !e1.is_finite() {
            outliers += 1;
            continue;
        }
        verrs.push(if e0.is_finite() && e1.is_finite() {
            e0.min(e1)
        } else {
            e0.max(e1)
        });
        speed += step / dt;
        n += 1;
        let q = |k: usize| getf32(&w[0].rec, R_QUAT + k);
        let qn = (q(0) * q(0) + q(4) * q(4) + q(8) * q(8) + q(12) * q(12)).sqrt();
        let qe = (qn - 1.0).abs();
        qerrs.push(if qe.is_finite() { qe } else { 1e9 });
        // ...and whether it MOVED between these two ticks. Compared across a
        // real tick step (the 10 ms filter above already guarantees that), so
        // the ~53 writes the engine makes inside one tick cannot make a
        // varying quaternion look constant.
        let q1 = |k: usize| getf32(&w[1].rec, R_QUAT + k);
        let moved = (0..4).any(|i| {
            let (a0, b0) = (q(i * 4), q1(i * 4));
            (a0 - b0).abs() > 1e-9
        });
        qsame += 1;
        if moved {
            qmoved += 1;
        }
        // Angular travel between the two ticks: 2*acos(|dot|) of the unit
        // quaternions, summed. Ranks a car that really turns above one whose
        // attitude merely jitters in the fifth decimal.
        let n1 = (q1(0) * q1(0) + q1(4) * q1(4) + q1(8) * q1(8) + q1(12) * q1(12)).sqrt();
        if qn > 0.5 && n1 > 0.5 {
            let dot = ((q(0) * q1(0) + q(4) * q1(4) + q(8) * q1(8) + q(12) * q1(12)) / (qn * n1))
                .abs()
                .min(1.0);
            let da = 2.0 * dot.acos();
            if da.is_finite() {
                qtrav += da;
            }
        }
    }
    // A quarter of the window may be teleports and freezes before the slot is
    // judged untrustworthy; below that they are simply not evidence.
    if n < 20 || outliers * 4 > rows {
        return None;
    }
    if let (Some(a), Some(p)) = (anchor, first_pos) {
        let d = ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2) + (p.2 - a.2).powi(2)).sqrt();
        if d > a.3 {
            return None;
        }
    }
    qerrs.sort_by(|a, b| a.total_cmp(b));
    let q95 = qerrs[(qerrs.len() * 95) / 100];
    Some(PosHit {
        pos,
        verr: median(&mut verrs),
        qerr: q95,
        qvary: if qsame > 0 {
            qmoved as f64 / qsame as f64
        } else {
            0.0
        },
        qtravel: qtrav,
        mean_speed: speed / n as f64,
        first: first_pos.unwrap_or((0.0, 0.0, 0.0)),
        ticks: ts.len(),
    })
}

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
