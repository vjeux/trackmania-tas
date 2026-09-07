//! G1: the engine's own checkpoint counter, located behaviourally.
//!
//! The geometric gate detector does not agree with the plain oracle on Summer
//! 2026 - 01 (RL-agentG §5.1: 20 of 40 tapes credited, no radius / window /
//! plane reproduces the split). The engine knows exactly when it credits a
//! checkpoint, so the counter is read out of it instead -- found the way the
//! race clock was found: as a memory slot whose values over a known run match
//! what the known run did.
//!
//! The known run is a game-recorded ghost whose split times are in its own
//! result chunk. Its tape drives a fork server; a sampled child snapshots
//! every writable window of the paused server's memory at instants spread
//! over the run (the race clock gathered beside each); a slot is a candidate
//! iff at every snapshot it equals the number of splits at or before that
//! instant. Candidates are then traced per tick to find the exact tick they
//! step, and located relative to the validator ownership chain so they can be
//! resolved on any server without a scan.

use crate::forkenv::Rig;
use fk::validator::ValidatorCar;
use forkoracle::forksrv::Rec;
use std::path::Path;

pub const EXIT_ON_BUDGET: u32 = 0x8000_0000;

/// A stage-1 survivor.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub addr: u64,
    /// 4 (u32) or 1 (byte).
    pub width: u8,
    /// Value at the first snapshot (the counter may not start at 0).
    pub base_value: i64,
    /// Offsets from the ownership chain's objects, for a per-server resolve.
    pub rel: Vec<(&'static str, i64)>,
}

/// The ghost's splits (ms, finish last) out of its result chunk.
pub fn ghost_splits(path: &Path) -> Result<Vec<i32>, String> {
    let c = ghost::Container::load(&path.to_string_lossy())?;
    let r = gbx::container::read_result(c.body()).ok_or("the ghost carries no result chunk")?;
    let s = r.checkpoints();
    if s.is_empty() {
        return Err("the ghost's result chunk has no splits".into());
    }
    Ok(s)
}

pub struct Snap {
    pub race_ms: i64,
    pub expect: i64,
}

/// Stage 1 over one window: which 4-byte / 1-byte slots match the expected
/// staircase at every snapshot. `blob` is `nsnap` records of `8 + 4 + len`
/// bytes (the sampler's 8-byte header, the clock word, the window).
fn scan_window(blob: &[u8], len: usize, bias: i64, splits: &[i32], w: u64, out: &mut Vec<Candidate>, snaps_seen: &mut usize) {
    let recsz = 8 + 4 + len;
    let m = blob.len() / recsz;
    if m < 6 {
        return;
    }
    *snaps_seen = (*snaps_seen).max(m);
    let clock = |i: usize| u32::from_le_bytes(blob[i * recsz + 8..i * recsz + 12].try_into().unwrap()) as i64;
    let expect: Vec<i64> = (0..m)
        .map(|i| {
            let race = clock(i) - bias;
            splits.iter().filter(|s| **s as i64 <= race).count() as i64
        })
        .collect();
    // A staircase with at least two distinct levels inside the sampled span,
    // or the test says nothing.
    if expect.first() == expect.last() {
        return;
    }
    let g32 = |i: usize, o: usize| -> i64 {
        u32::from_le_bytes(blob[i * recsz + 12 + o..i * recsz + 16 + o].try_into().unwrap()) as i64
    };
    let g8 = |i: usize, o: usize| -> i64 { blob[i * recsz + 12 + o] as i64 };
    for o in (0..len.saturating_sub(4)).step_by(4) {
        let base = g32(0, o) - expect[0];
        if (0..m).all(|i| g32(i, o) - base == expect[i]) {
            out.push(Candidate { addr: w + o as u64, width: 4, base_value: base, rel: Vec::new() });
        }
    }
    for o in 0..len {
        let base = g8(0, o) - expect[0];
        if base >= 0 && (0..m).all(|i| g8(i, o) - base == expect[i]) {
            // skip bytes that are the low byte of an accepted u32
            if out.iter().any(|c| c.width == 4 && c.addr == w + o as u64) {
                continue;
            }
            out.push(Candidate { addr: w + o as u64, width: 1, base_value: base, rel: Vec::new() });
        }
    }
}

/// Run stage 1 and stage 2 on one ghost. Prints as it goes; returns the
/// candidates whose per-tick step instants match every split within `tol_ms`.
pub fn cpfind(
    server: &Path,
    map: &Path,
    shim: &Path,
    work: &Path,
    ghost_path: &Path,
    every_ticks: u64,
    tol_ms: i64,
    verbose: bool,
) -> Result<(Vec<Candidate>, Vec<(Candidate, Vec<i64>)>), String> {
    let splits = ghost_splits(ghost_path)?;
    println!(
        "ghost      {}  splits {}",
        ghost_path.display(),
        splits.iter().map(|s| format!("{:.3}", *s as f64 / 1000.0)).collect::<Vec<_>>().join(" ")
    );
    let rig = Rig::new(server, map, shim, work, ghost_path)?;
    let mut s = rig.session_clock(crate::control::EARLIEST_CLOCK)?;
    let probe = s.probe_tick()?;
    // ALL records, for the resolver (it scans, it does not drive)...
    let recs: Vec<Rec> = s.tape.tail_records(0);
    let car = crate::control::resolve_car(&mut s.srv, probe, &recs, s.tape.start_offset_ms, verbose)?;
    let layout = car.layout().clone();
    let prov = car.provenance().clone();
    // For a countdown-prefixed real ghost the root probe is exact (measured:
    // 156 on every server, bias 2200 = the in-race calibration), so the
    // root-implied bias is used here; the snapshot tolerance covers a tick.
    let bias = layout.clock_bias;
    let n_ticks = s.tape.n() as u32;
    // ...and the TAIL FROM THE PROBE for every child that must DRIVE the ghost's
    // line: a patch list is written at `from + i`, so handing the whole tape
    // with `from = probe` shifts every input by `probe` ticks AND writes past
    // the end of the input array (heap corruption: "double free or corruption
    // (out)" in every child, 0 samples back).
    let drive: Vec<Rec> = s.tape.tail_records(probe);
    println!(
        "server     probe {probe}, clock {:#x} bias {bias}, car state {:#x}; chain controller {:#x} sim {:#x} playground {:#x} players {:#x} participant {:#x} vehicle {:#x}",
        layout.clock, layout.pos, prov.controller, prov.sim, prov.playground, prov.players, prov.participant, prov.vehicle
    );

    // ---- stage 1: every writable window
    let slice: u32 = 1 << 20;
    let mut wins: Vec<u64> = Vec::new();
    for r in forkoracle::procmem::maps(s.srv.pid()) {
        if !(r.perms.contains('w') && r.perms.contains('r')) || r.path.starts_with("/dev") || r.end <= r.start {
            continue;
        }
        if r.end - r.start > (1u64 << 30) {
            continue;
        }
        let mut a = (r.start + 0xFFF) & !0xFFF;
        while a + slice as u64 <= r.end {
            wins.push(a);
            a += slice as u64;
        }
        if r.end > a + 4096 {
            wins.push(a); // the tail, read as a shorter window below
        }
    }
    // nearest to the participant first: that is where a race counter should live
    wins.sort_by_key(|w| (*w as i64 - prov.participant as i64).unsigned_abs());
    wins.dedup();
    let nsnap = (n_ticks as u64 / every_ticks).max(6) as u32;
    println!("stage 1    {} windows of {} KB, {} snapshots every {} ticks", wins.len(), slice / 1024, nsnap, every_ticks);
    let t0 = std::time::Instant::now();
    let mut cands: Vec<Candidate> = Vec::new();
    let mut snaps_seen = 0usize;
    let regions = forkoracle::procmem::maps(s.srv.pid());
    for (i, w) in wins.iter().enumerate() {
        let end = regions.iter().filter(|r| r.start <= *w && *w < r.end).map(|r| r.end).next().unwrap_or(*w + slice as u64);
        // the shim gathers at most 1 MB per sample record, clock word included
        let len = ((end - *w).min(slice as u64 - 4096) as u32) & !0xFFF;
        if len < 4096 {
            continue;
        }
        let (_j, blob) = s.srv.run_sampled_segs_ex(
            probe,
            &drive,
            &[(layout.clock, 4), (*w, len)],
            every_ticks,
            nsnap | EXIT_ON_BUDGET,
            (0, 0),
            n_ticks + 100,
        );
        if i == 0 || (verbose && blob.is_empty()) {
            println!("  window {:#x} len {}: json {:?}, blob {} bytes = {} records", w, len, _j.trim(), blob.len(), blob.len() / (12 + len as usize));
        }
        let before = cands.len();
        scan_window(&blob, len as usize, bias, &splits, *w, &mut cands, &mut snaps_seen);
        if verbose && cands.len() > before {
            println!("  window {:#x}: {} candidate(s)", w, cands.len() - before);
        }
        if i % 40 == 39 {
            println!("  ... {} of {} windows, {} candidates ({:.1} s)", i + 1, wins.len(), cands.len(), t0.elapsed().as_secs_f64());
        }
    }
    println!(
        "stage 1    {} candidates ({} u32, {} byte) from {} snapshots per window, {:.1} s",
        cands.len(),
        cands.iter().filter(|c| c.width == 4).count(),
        cands.iter().filter(|c| c.width == 1).count(),
        snaps_seen,
        t0.elapsed().as_secs_f64()
    );
    for c in cands.iter_mut() {
        c.rel = vec![
            ("controller", c.addr as i64 - prov.controller as i64),
            ("sim", c.addr as i64 - prov.sim as i64),
            ("playground", c.addr as i64 - prov.playground as i64),
            ("players", c.addr as i64 - prov.players as i64),
            ("participant", c.addr as i64 - prov.participant as i64),
            ("vehicle", c.addr as i64 - prov.vehicle as i64),
            ("state", c.addr as i64 - layout.pos as i64),
        ];
    }

    // ---- stage 2: per-tick trace of each candidate, the tick it steps
    let mut survivors: Vec<(Candidate, Vec<i64>)> = Vec::new();
    for c in &cands {
        let segs = [(layout.clock, 4u32), (c.addr & !3, 4u32)];
        let (_j, blob) = s.srv.run_sampled_segs_ex(
            probe,
            &drive,
            &segs,
            1,
            (n_ticks + 400) | EXIT_ON_BUDGET,
            (0, 8),
            n_ticks + 100,
        );
        let recsz = 8 + 8;
        let m = blob.len() / recsz;
        let shift = (c.addr & 3) * 8;
        let mut steps: Vec<i64> = Vec::new();
        let mut last: Option<i64> = None;
        for i in 0..m {
            let clk = u32::from_le_bytes(blob[i * recsz + 8..i * recsz + 12].try_into().unwrap()) as i64;
            let word = u32::from_le_bytes(blob[i * recsz + 12..i * recsz + 16].try_into().unwrap());
            let v = if c.width == 4 { word as i64 } else { ((word >> shift) & 0xFF) as i64 };
            if let Some(l) = last {
                if v != l {
                    steps.push(clk - bias);
                }
            }
            last = Some(v);
        }
        let ok = steps.len() == splits.len()
            && steps.iter().zip(splits.iter()).all(|(t, s)| (t - *s as i64).abs() <= tol_ms);
        let ok_but_finish = steps.len() + 1 == splits.len()
            && steps.iter().zip(splits.iter()).all(|(t, s)| (t - *s as i64).abs() <= tol_ms);
        println!(
            "  {:#x} w{} base {}  steps at {}  {}  rel participant {:+} playground {:+} sim {:+} controller {:+}",
            c.addr,
            c.width,
            c.base_value,
            steps.iter().map(|t| format!("{:.3}", *t as f64 / 1000.0)).collect::<Vec<_>>().join(" "),
            if ok { "MATCHES every split" } else if ok_but_finish { "matches every CHECKPOINT, not the finish" } else { "no" },
            c.rel[4].1,
            c.rel[2].1,
            c.rel[1].1,
            c.rel[0].1
        );
        if ok || ok_but_finish {
            survivors.push((c.clone(), steps));
        }
    }
    println!("stage 2    {} of {} candidates step at the ghost's split times (±{} ms)", survivors.len(), cands.len(), tol_ms);
    let _ = ValidatorCar::layout;
    Ok((cands, survivors))
}
