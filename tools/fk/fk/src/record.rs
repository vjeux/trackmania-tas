//! `fk clean` -- record a whole run's per-tick state from the PARENT process.
//!
//! WHY NOT A FORK
//! --------------
//! Every trajectory tool here so far resumes the simulation in a forked child.
//! That is right for a search (candidates are compared with each other at one
//! fixed checkpoint) and wrong for regenerating telemetry, because the resumed
//! run is not the clean run: measured on this engine, the same tape resumed at
//! two different checkpoints agrees on 0 of 522 ticks and diverges by metres,
//! and against a human ghost's own recorded path a resume at race -30 ms is
//! 5.578 m out where a resume at race +140 ms is 0.0055 m.
//!
//! So: fork only to LOCATE (a child cannot touch the parent's memory), then
//! hand the addresses back to the parent and let it run to the end of the tape
//! as an ordinary `/validatepath` run, writing one record per 50 ms grid
//! instant. The finish time it prints is the re-verification of the very run
//! whose telemetry was recorded.

use crate::session::{clock_for_tick, start_server_on_file, tail_recs, Ctx};
use crate::locate::locate_v2;
use crate::tape::Tape as Factory;

/// One gathered instant: the race clock and the raw bytes of every segment.
pub struct Rec {
    pub clock: u32,
    pub bytes: Vec<u8>,
}

pub fn read_samples(path: &str, reclen: usize) -> Vec<Rec> {
    read_samples_ex(path, reclen, false)
}

/// `first = true` keeps the FIRST record written in each tick rather than the
/// last. The engine writes the vehicle state several times inside one tick and
/// which of those the game's own recorder captured is a measurable question,
/// not a matter of taste (measured: steer wants the first write, suspension the
/// last).
pub fn read_samples_ex(path: &str, reclen: usize, first: bool) -> Vec<Rec> {
    let b = std::fs::read(path).unwrap_or_default();
    let recsz = 8 + reclen;
    let mut out = Vec::with_capacity(b.len() / recsz.max(1));
    for i in 0..b.len() / recsz {
        let r = &b[i * recsz + 8..i * recsz + 8 + reclen];
        let clk = u32::from_le_bytes(r[0..4].try_into().unwrap());
        match out.last_mut() {
            Some(Rec { clock, bytes }) if *clock == clk => {
                if !first {
                    bytes.copy_from_slice(r)
                }
            }
            _ => out.push(Rec {
                clock: clk,
                bytes: r.to_vec(),
            }),
        }
    }
    out
}

/// Both the FIRST and the LAST write of each tick.
pub fn read_samples_pair(path: &str, reclen: usize) -> Vec<(u32, Vec<u8>, Vec<u8>)> {
    let b = std::fs::read(path).unwrap_or_default();
    let recsz = 8 + reclen;
    let mut out: Vec<(u32, Vec<u8>, Vec<u8>)> = Vec::with_capacity(b.len() / recsz.max(1));
    for i in 0..b.len() / recsz {
        let r = &b[i * recsz + 8..i * recsz + 8 + reclen];
        let clk = u32::from_le_bytes(r[0..4].try_into().unwrap());
        match out.last_mut() {
            Some((c, _, l)) if *c == clk => l.copy_from_slice(r),
            _ => out.push((clk, r.to_vec(), r.to_vec())),
        }
    }
    out
}

/// Parse `--segs` as a comma-separated list of `offset:len`, offsets relative
/// to the located position anchor (so they are struct offsets, not addresses).
pub fn parse_segs(s: &str) -> Vec<(i64, u32)> {
    s.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (a, b) = p.split_once(':').expect("segment must be OFFSET:LEN");
            (
                a.trim().parse::<i64>().expect("segment offset"),
                b.trim().parse::<u32>().expect("segment length"),
            )
        })
        .collect()
}

pub struct CleanOut {
    pub bias: i64,
    pub reclen: usize,
    pub sim_time: Option<i64>,
    pub instants: usize,
    pub first_ms: i64,
    pub last_ms: i64,
    pub probe_ms: i64,
    pub pos: u64,
    /// Byte offsets WITHIN one gathered record.
    pub pos_off: usize,
    pub quat_off: usize,
    /// 0 = (x,y,z,w), 1 = (w,x,y,z), 2 = orthonormal 3x3 row-major
    pub quat_kind: u8,
    pub vel_off: usize,
    /// The mapped region the CHOSEN car sits in, as `(start, end)` in this
    /// server's address space.
    ///
    /// A wide gather reads raw addresses out of the engine with
    /// `copy_nonoverlapping`, so a window that runs off the end of the mapping
    /// takes the whole server down. This is the bound: a second gather clamps
    /// its window to the car's own distance from each edge of the region the
    /// first one found it in, so the read is proved to be inside a mapping
    /// rather than assumed to be.
    pub pos_region: (u64, u64),
    /// arm `whl`: the segments actually gathered, pos-relative, in record
    /// order. A wheel-anchored field map needs them to turn a memory offset
    /// into a record offset.
    pub segs_rel: Vec<(i64, u32)>,
    /// The segments actually gathered, as ADDRESSES, in record order.
    ///
    /// `pos` above is computed as if the record were one contiguous window,
    /// which is true of the production gather and false of any gather with
    /// extra segments — those are separate windows and a record offset inside
    /// one of them is not `pos + something`. Anything that has to turn a record
    /// offset into an address (finding what a pointer would have to point at,
    /// for one) must use [`CleanOut::addr_of`] and this list.
    pub segs_abs: Vec<(u64, u32)>,
}

impl CleanOut {
    /// Where an engine address landed in the gathered record, if it was
    /// gathered at all. The inverse of [`CleanOut::addr_of`], and the only
    /// honest way to find a segment's offset once the clipper has had its way
    /// with the list.
    pub fn off_of(&self, addr: u64) -> Option<usize> {
        let mut off = 0usize;
        for (a, l) in &self.segs_abs {
            if addr >= *a && addr < a + *l as u64 {
                return Some(off + (addr - a) as usize);
            }
            off += *l as usize;
        }
        None
    }

    /// The engine address a byte of a gathered record came from.
    pub fn addr_of(&self, mut off: usize) -> Option<u64> {
        for (a, l) in &self.segs_abs {
            if off < *l as usize {
                return Some(a + off as u64);
            }
            off -= *l as usize;
        }
        None
    }
}

/// The engine clock's offset against race time, and where the vehicle state
/// sits relative to the decoded input array.
///
/// Both are measured at a checkpoint far enough into the tape that the car is
/// moving and the page-fault probe is exact. The early checkpoint the clean run
/// needs can do neither: at race -30 ms the car has not moved, so the position
/// locator's "is this triple moving?" filter drops the true slot and the
/// velocity self-consistency test has nothing to work with (measured on 252289
/// and 126859: "no moving, in-bounds float triple", 1828 windows, 5 minutes).
/// The offsets from the input array base, on the other hand, are stable: four
/// runs on 208024 put the state at base-470768 and the clock at base-478580
/// every time, and the clean run re-derives both from its OWN base and then
/// checks them against the sampled data.
#[derive(Clone, Copy)]
pub struct Anchors {
    pub bias: i64,
    pub pos_delta: i64,
    pub clock_delta: i64,
    pub speed: f64,
    /// Offset of the unit quaternion relative to the position triple.
    pub quat_off: i64,
    /// 0 = (x,y,z,w) quaternion, 1 = (w,x,y,z) quaternion, 2 = orthonormal 3x3
    pub quat_kind: u8,
    /// Offset of the velocity triple relative to the position triple.
    pub vel_off: i64,
}

/// How far either side of the position the layout search looks, and how wide
/// the sampled window is.

pub fn win_back() -> i64 {
    std::env::var("FK_WIN_BACK").ok().and_then(|v| v.parse().ok()).unwrap_or(192)
}
pub fn win_len() -> u32 {
    std::env::var("FK_WIN_LEN").ok().and_then(|v| v.parse().ok()).unwrap_or(448)
}
/// Extra gathered segments, RELATIVE TO THE POSITION ANCHOR, on top of the
/// production window — the ground a field search looks over.
///
/// This was an environment variable (`FK_EXTRA_SEGS`). It is a parameter now,
/// because the one command that wants a wide gather also has to change the
/// dedup key and turn the copy search off in the same breath (see
/// [`GatherOpts`]), and a window widened without those two is not a wider
/// measurement — it is 9.8 GB of disk and a locate that walks off the car.
pub type ExtraSegs = Vec<(i64, u32)>;


/// Convert a 3x3 rotation matrix (row-major, world = M * body) to (x, y, z, w).
pub fn mat_to_quat(m: &[f64; 9]) -> [f64; 4] {
    let tr = m[0] + m[4] + m[8];
    if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [(m[7] - m[5]) / s, (m[2] - m[6]) / s, (m[3] - m[1]) / s, 0.25 * s]
    } else if m[0] > m[4] && m[0] > m[8] {
        let s = (1.0 + m[0] - m[4] - m[8]).sqrt() * 2.0;
        [0.25 * s, (m[1] + m[3]) / s, (m[2] + m[6]) / s, (m[7] - m[5]) / s]
    } else if m[4] > m[8] {
        let s = (1.0 + m[4] - m[0] - m[8]).sqrt() * 2.0;
        [(m[1] + m[3]) / s, 0.25 * s, (m[5] + m[7]) / s, (m[2] - m[6]) / s]
    } else {
        let s = (1.0 + m[8] - m[0] - m[4]).sqrt() * 2.0;
        [(m[2] + m[6]) / s, (m[5] + m[7]) / s, 0.25 * s, (m[3] - m[1]) / s]
    }
}

fn quat_fwd(q: [f64; 4]) -> [f64; 3] {
    let [x, y, z, w] = q;
    // rotate (0,0,1)
    [
        2.0 * (x * z + w * y),
        2.0 * (y * z - w * x),
        1.0 - 2.0 * (x * x + y * y),
    ]
}

fn wrap(a: f64) -> f64 {
    let mut a = a;
    while a > std::f64::consts::PI {
        a -= 2.0 * std::f64::consts::PI;
    }
    while a < -std::f64::consts::PI {
        a += 2.0 * std::f64::consts::PI;
    }
    a
}

/// Find the quaternion and the velocity RELATIVE TO THE POSITION, by
/// measurement.
///
/// `-16` and `+12` are true of the copy the 208024 work landed on and false of
/// the copy the locator lands on elsewhere: on 267859 the reference-matched
/// locator finds the car to 6 mm and that same slot has `|q|-1 = 2.2e-2` at -16
/// and `|d(pos)/dt - v| = 13.2 m/s` at +12.
///
/// THE ORIENTATION NEEDS MORE THAN `|q| = 1`. A first version took the nearest
/// unit quaternion and passed every structural self-check -- and the control
/// against 267859's own human ghost came back with a **142.7 degree** median
/// orientation error, because a unit quaternion belonging to something else is
/// still a unit quaternion. So a candidate must also point roughly where the
/// car is going: the spread of (body heading - velocity heading) over the whole
/// window has to be small. A car can drift, so the test allows a wide constant
/// offset and only rejects on SPREAD.
fn discover_layout(
    srv: &mut forkoracle::forksrv::ForkServer,
    probe: usize,
    recs: &[forkoracle::forksrv::Rec],
    clock: u64,
    pos: u64,
) -> Option<(i64, u8, i64, f64)> {
    let segs = [(clock, 4u32), ((pos as i64 - win_back()) as u64, win_len())];
    let ts = crate::locate::gather_ticks(srv, probe, recs, &segs, 200, 1600, (0, 4 + win_len()));
    if ts.len() < 40 {
        return None;
    }
    let g = |t: &crate::locate::Tick, o: usize| -> f64 {
        f32::from_le_bytes(t.rec[o..o + 4].try_into().unwrap()) as f64
    };
    let p0 = 4 + win_back() as usize;
    let mut best_v: Option<(f64, i64)> = None;
    for o in (4..(4 + win_len() as usize - 12)).step_by(4) {
        let mut ds: Vec<f64> = Vec::new();
        for w in ts.windows(2) {
            let dt = (w[1].clock as i64 - w[0].clock as i64) as f64 / 1000.0;
            if dt <= 0.0 {
                continue;
            }
            let mut d = 0.0;
            for k in 0..3 {
                let dv = (g(&w[1], p0 + k * 4) - g(&w[0], p0 + k * 4)) / dt - g(&w[0], o + k * 4);
                d += dv * dv;
            }
            ds.push(d.sqrt());
        }
        if ds.is_empty() {
            continue;
        }
        ds.sort_by(|a, b| a.total_cmp(b));
        let med = ds[ds.len() / 2];
        if med.is_finite() && best_v.map_or(true, |b: (f64, i64)| med < b.0) {
            best_v = Some((med, o as i64 - p0 as i64));
        }
    }
    let mut speeds: Vec<f64> = Vec::new();
    for w in ts.windows(2) {
        let dt = (w[1].clock as i64 - w[0].clock as i64) as f64 / 1000.0;
        if dt > 0.0 {
            let s: f64 = (0..3)
                .map(|k| ((g(&w[1], p0 + k * 4) - g(&w[0], p0 + k * 4)) / dt).powi(2))
                .sum::<f64>()
                .sqrt();
            speeds.push(s);
        }
    }
    speeds.sort_by(|a, b| a.total_cmp(b));
    let speed = if speeds.is_empty() { 0.0 } else { speeds[speeds.len() / 2] };
    let (verr, voff) = best_v?;
    if verr > (0.15 * speed).max(1.0) {
        return None;
    }
    // ---- orientation: a unit quaternion OR an orthonormal 3x3, and it must
    //      point roughly where the car is going.
    let vyaw: Vec<Option<f64>> = ts
        .iter()
        .map(|t| {
            let vx = g(t, (p0 as i64 + voff) as usize);
            let vz = g(t, (p0 as i64 + voff) as usize + 8);
            if (vx * vx + vz * vz).sqrt() > 3.0 {
                Some(vz.atan2(vx))
            } else {
                None
            }
        })
        .collect();
    let heading_spread = |qs: &[Option<[f64; 4]>]| -> Option<f64> {
        let mut d: Vec<f64> = Vec::new();
        for (i, q) in qs.iter().enumerate() {
            let (Some(q), Some(vy)) = (q, vyaw[i]) else { continue };
            let f = quat_fwd(*q);
            if (f[0] * f[0] + f[2] * f[2]).sqrt() < 0.2 {
                continue;
            }
            d.push(wrap(f[2].atan2(f[0]) - vy));
        }
        if d.len() < 20 {
            return None;
        }
        // circular median, then the spread about it
        let mut s: Vec<f64> = d.clone();
        s.sort_by(|a, b| a.total_cmp(b));
        let med = s[s.len() / 2];
        let mut dev: Vec<f64> = d.iter().map(|x| wrap(x - med).abs()).collect();
        dev.sort_by(|a, b| a.total_cmp(b));
        Some(dev[(dev.len() as f64 * 0.9) as usize])
    };
    let mut best_o: Option<(f64, u8, i64)> = None;
    for o in (4..(4 + win_len() as usize - 16)).step_by(4) {
        // quaternion candidate
        let mut ok = true;
        let mut varies = false;
        let qs: Vec<Option<[f64; 4]>> = ts
            .iter()
            .map(|t| {
                let q = [g(t, o), g(t, o + 4), g(t, o + 8), g(t, o + 12)];
                let n: f64 = q.iter().map(|c| c * c).sum::<f64>().sqrt();
                if !n.is_finite() || (n - 1.0).abs() > 1e-4 {
                    ok = false;
                }
                if q[0] != g(&ts[0], o) {
                    varies = true;
                }
                // the record's convention is (x, y, z, w)
                Some([q[0], q[1], q[2], q[3]])
            })
            .collect();
        if ok && varies {
            for order in 0..2 {
                let qq: Vec<Option<[f64; 4]>> = qs
                    .iter()
                    .map(|q| {
                        q.map(|q| {
                            if order == 0 {
                                q
                            } else {
                                [q[1], q[2], q[3], q[0]] // engine (w,x,y,z)
                            }
                        })
                    })
                    .collect();
                if let Some(sp) = heading_spread(&qq) {
                    if sp < 0.9 && best_o.map_or(true, |b| sp < b.0) {
                        best_o = Some((sp, order, o as i64 - p0 as i64));
                    }
                }
            }
        }
        // orthonormal 3x3 candidate
        if o + 36 <= 4 + win_len() as usize {
            let mut good = true;
            let ms: Vec<Option<[f64; 4]>> = ts
                .iter()
                .map(|t| {
                    let mut m = [0.0f64; 9];
                    for k in 0..9 {
                        m[k] = g(t, o + k * 4);
                    }
                    let row = |i: usize| [m[i * 3], m[i * 3 + 1], m[i * 3 + 2]];
                    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                    for i in 0..3 {
                        let r = row(i);
                        if (dot(r, r).sqrt() - 1.0).abs() > 1e-3 {
                            good = false;
                        }
                    }
                    if dot(row(0), row(1)).abs() > 1e-3
                        || dot(row(0), row(2)).abs() > 1e-3
                        || dot(row(1), row(2)).abs() > 1e-3
                    {
                        good = false;
                    }
                    Some(mat_to_quat(&m))
                })
                .collect();
            if good {
                if let Some(sp) = heading_spread(&ms) {
                    if sp < 0.9 && best_o.map_or(true, |b| sp < b.0) {
                        best_o = Some((sp, 2, o as i64 - p0 as i64));
                    }
                }
            }
        }
    }
    let (spread, kind, ooff) = best_o?;
    if std::env::var("FKDBG").is_ok() {
        println!(
            "    orientation: kind {} at {:+}, heading spread p90 {:.1} deg",
            kind,
            ooff,
            spread.to_degrees()
        );
    }
    Some((ooff, kind, voff, speed))
}

/// Every plausible anchor, fastest candidate first, each with its own measured
/// (position, quaternion, velocity) layout.
pub fn measure_anchors(c: &Ctx, f: &Factory, tick: i64, verbose: bool) -> Result<Vec<Anchors>, String> {

    use std::path::PathBuf;
    let work = PathBuf::from(format!("{}-anch", c.work));
    let _ = std::fs::create_dir_all(&work);
    let ckpt = clock_for_tick(tick, f.start_offset_ms);
    let mut srv =
        start_server_on_file(c, f, &work, ckpt, std::path::Path::new(&c.template))?;
    let probe = srv.probe_tick().map_err(|e| format!("probe {}", e))?;
    // NO INPUT PATCH for the locate probes. The staged ghost is the original
    // file, so the child already has the right tape; patching it with the
    // Factory's decoded values is at best a no-op and at worst wrong -- on
    // 267859 and 227654 the patched child drove at 1.3 m/s and the locate found
    // nothing but decoys. Observing costs nothing and assumes nothing.
    let lrecs: Vec<forkoracle::forksrv::Rec> = Vec::new();
    let bounds = (-64000.0, 64000.0, -1000.0, 4000.0, -64000.0, 64000.0);
    let ck = crate::locate::find_clock2(&mut srv, probe, &lrecs, f.start_offset_ms, 100000, verbose)?;
    let mut cands = crate::locate::locate_candidates(
        &mut srv, probe, &lrecs, ck.addr, bounds, 4000, 6, verbose,
    );
    if cands.is_empty() {
        cands = crate::locate::locate_positions_loose(
            &mut srv, probe, &lrecs, ck.addr, bounds, 4000, 8, verbose,
        );
        if verbose {
            println!("loose position candidates: {}", cands.len());
        }
    }
    let base = srv.base;
    let mut out: Vec<Anchors> = Vec::new();
    for h in &cands {
        let Some((qoff, qkind, voff, speed)) = discover_layout(&mut srv, probe, &lrecs, ck.addr, h.pos)
        else {
            continue;
        };
        if verbose {
            println!(
                "  layout at base{:+}: orient kind {} {:+}, vel {:+}, speed {:.1} m/s",
                h.pos as i64 - base as i64,
                qkind,
                qoff,
                voff,
                speed
            );
        }
        out.push(Anchors {
            bias: ck.bias,
            pos_delta: h.pos as i64 - base as i64,
            clock_delta: ck.addr as i64 - base as i64,
            speed,
            quat_off: qoff,
            quat_kind: qkind,
            vel_off: voff,
        });
    }
    srv.quit();
    let _ = std::fs::remove_dir_all(&work);
    if out.is_empty() {
        return Err("no candidate with a discoverable (position, quaternion, velocity) layout".into());
    }
    out.sort_by(|a, b| b.speed.total_cmp(&a.speed));
    Ok(out)
}

/// The engine clock's offset against race time, read at a checkpoint far enough
/// in for the page-fault probe to be exact.
pub fn measure_bias(c: &Ctx, f: &Factory, tick: i64, verbose: bool) -> Result<i64, String> {

    use std::path::PathBuf;
    let work = PathBuf::from(format!("{}-bias", c.work));
    let _ = std::fs::create_dir_all(&work);
    let ckpt = clock_for_tick(tick, f.start_offset_ms);
    let mut srv = start_server_on_file(c, f, &work, ckpt, std::path::Path::new(&c.template))?;
    let probe = srv.probe_tick().map_err(|e| format!("probe {}", e))?;
    // NO INPUT PATCH for the locate probes. The staged ghost is the original
    // file, so the child already has the right tape; patching it with the
    // Factory'"'"'s decoded values is at best a no-op and at worst wrong -- on
    // 267859 and 227654 the patched child drove at 1.3 m/s and the locate found
    // nothing but decoys. Observing costs nothing and assumes nothing.
    let lrecs: Vec<forkoracle::forksrv::Rec> = Vec::new();
    let _ = tail_recs(&f.steer, &f.accel, &f.brake, probe);
    let hit = crate::locate::find_clock2(&mut srv, probe, &lrecs, f.start_offset_ms, 100000, verbose)?;
    srv.quit();
    let _ = std::fs::remove_dir_all(&work);
    Ok(hit.bias)
}

/// The clean run itself.

#[allow(clippy::too_many_arguments)]
/// How to gather, for the one function that gathers.
///
/// This used to be eight positional parameters, which was survivable while
/// `regen` was the only caller. It is not survivable now that a second caller
/// needs two of the decisions made differently, because those two decisions are
/// exactly the ones that go wrong quietly:
///
/// * **`dedup`** bounds what reaches the disk. The shim emits a record on every
///   `lroundf` call whose gathered key slice differs from the last, so a WIDE
///   gather deduplicated on the whole record never suppresses anything —
///   something in 320 KB of engine memory changes on every call. Measured: a
///   320 KB window at a 50 ms grid wrote **9.8 GB in two minutes** and was still
///   going. Keying the dedup on the production window alone restores the
///   production semantics (one record per distinct vehicle state) and the extra
///   ground rides along for free.
/// * **`choose_copy`** decides whether the gathered record is searched for the
///   live copy of the car. That search is right when the window is 452 bytes of
///   vehicle state and catastrophic when it is 320 KB of anything: on the first
///   wide run it walked off the car and the self-check reported
///   `|q|-1 p99.5 = 1.34e-1`. A caller that has ALREADY chosen the car — by
///   running a narrow gather first and taking [`CleanOut::pos`] — must be able
///   to say so.
pub struct GatherOpts<'a> {
    /// Segments to gather, relative to the located position. Ignored when
    /// `anchors` is given, which uses the production window plus
    /// [`extra_segs`].
    pub segs_rel: &'a [(i64, u32)],
    pub bias_override: Option<i64>,
    pub anchors: Option<&'a Anchors>,
    /// Grid period in ms. 10 = every tick (production), 50 = the record's own
    /// sample grid (what a wide gather can afford).
    pub period: i64,
    pub phase_ms: i64,
    pub dump: &'a str,
    pub verbose: bool,
    /// Dedup key inside the gathered record, `(offset, len)`. `None` = the
    /// whole record, which is only affordable for a narrow gather.
    pub dedup: Option<(u32, u32)>,
    /// Search the gathered record for the live copy of the car.
    pub choose_copy: bool,
    /// Ground to gather BESIDE the production window, relative to the position
    /// anchor. Empty for production.
    pub extra: ExtraSegs,
    /// How far into the gathered record the LIVE-COPY SEARCH may look.
    ///
    /// `None` = the whole record, which is what a bare gather wants. A caller
    /// that gathers extra ground **only to read fields out of it** must cap
    /// this at the production window, because the copy search ranges over
    /// whatever was gathered and every extra byte is another candidate — so
    /// widening the window to reach a field would silently change which copy of
    /// the car the transform is written from. Capping keeps the choice
    /// bit-identical to a run with no extra ground at all.
    pub copy_scan_hi: Option<usize>,
    /// Offsets, relative to the position triple, that a candidate copy must
    /// hold a LIVE f32 at — one that takes more than one value over the run.
    ///
    /// This is the reference-free signature of the vehicle struct, and it is
    /// the only thing that separates it from a BARE POSITION COPY: a copy with
    /// the right position and dead memory around it, which passes every
    /// structural test there is (its position is the car's, so its velocity is
    /// consistent and its quaternion is a unit quaternion) and reads zero for
    /// every field. A regeneration anchored on one writes zeroed wheels and
    /// gear into a file that then passes the whole acceptance gate, because
    /// none of those bytes affects the simulation.
    ///
    /// Pass the wheel-rotation offsets. Four live floats at stride 44 is a car;
    /// four constants is not.
    pub require_live: Vec<i64>,
    /// WHERE THE CAR IS, from something that already knows.
    ///
    /// Called with `(pid, base)` of the started server while it is halted at
    /// the handover, and its answer REPLACES the located position anchor. This
    /// is how a pointer chain (`fk::ptr`) removes the search: the engine's own
    /// pointer to the vehicle state is dereferenced in the live process and the
    /// gather is centred on the answer.
    ///
    /// It changes where the window is and NOTHING about what happens to it —
    /// every guard downstream (the wheel-liveness rule, the path-length test,
    /// the self-check, the caller's comparison against a recording) runs
    /// exactly as it does on a located anchor, so a stale pointer FAILS rather
    /// than producing a file.
    /// Returns `(the position anchor, extra segments relative to it)`. The
    /// second half is what makes a POOL usable: the engine owns four vehicle
    /// objects and which of them is live varies by process, so the resolver
    /// hands back the anchor plus a window on each sibling and the copy rule
    /// downstream chooses between them — the same rule, on four candidates
    /// instead of 300,000.
    pub pos_from: Option<&'a dyn Fn(i32, u64) -> Result<(u64, Vec<(i64, u32)>), String>>,
    /// Look at the live server, halted, just before the run starts.
    ///
    /// Called with `(pid, the position anchor, the segments about to be
    /// gathered)`. `fk ptr find` uses it to snapshot the engine's memory at an
    /// instant when nothing is moving — the pointer and the object it points at
    /// are then read from the same state of the world, so a missing pointer
    /// cannot be a torn read.
    pub before_go: Option<&'a dyn Fn(i32, u64, &[(u64, u32)])>,
    /// Abort when the gathered window is not a self-consistent vehicle state.
    ///
    /// ON for production, where there is nothing better: the structural tests
    /// (unit quaternion, velocity equals the position's derivative) are all a
    /// caller has when it does not already know where the car went. A caller
    /// that holds the answer — a recording the GAME wrote, whose positions ARE
    /// the run — has a stronger control and does not want this one, because a
    /// wide window is centred on whatever the anchor pointed at and the car it
    /// is looking for may be some thousands of bytes away inside it.
    pub self_check: bool,
}

impl<'a> GatherOpts<'a> {
    /// The production gather: narrow window, whole-record dedup, copy search on.
    pub fn production(dump: &'a str) -> Self {
        GatherOpts {
            segs_rel: &[],
            bias_override: None,
            anchors: None,
            period: 10,
            phase_ms: 0,
            dump,
            verbose: false,
            dedup: None,
            choose_copy: true,
            extra: Vec::new(),
            copy_scan_hi: None,
            require_live: Vec::new(),
            pos_from: None,
            before_go: None,
            self_check: true,
        }
    }
}

pub fn run_clean_anch(c: &Ctx, o: &GatherOpts) -> Result<CleanOut, String> {
    let GatherOpts {
        segs_rel,
        bias_override,
        anchors,
        period,
        phase_ms,
        dump,
        verbose,
        dedup,
        choose_copy,
        extra,
        copy_scan_hi,
        require_live,
        pos_from,
        before_go,
        self_check,
    } = o;
    let (segs_rel, bias_override, anchors) = (*segs_rel, *bias_override, *anchors);
    let (period, phase_ms, dump, verbose) = (*period, *phase_ms, *dump, *verbose);
    let (dedup, choose_copy, self_check) = (*dedup, *choose_copy, *self_check);
    let copy_scan_hi = *copy_scan_hi;
    // Does the candidate at record offset `p` hold a live float at every
    // offset the caller named? See `GatherOpts::require_live`.
    let wheels_live = |recs: &[Rec], p: usize, reclen: usize| -> bool {
        require_live.iter().all(|rel| {
            let q = p as i64 + rel;
            if q < 0 || q as usize + 4 > reclen {
                return false;
            }
            let q = q as usize;
            let g = |r: &Rec| f32::from_le_bytes(r.bytes[q..q + 4].try_into().unwrap());
            let first = g(&recs[0]);
            first.is_finite() && recs.iter().any(|r| g(r) != first && g(r).is_finite())
        })
    };

    use std::path::PathBuf;
    let work = PathBuf::from(&c.work);
    let _ = std::fs::create_dir_all(&work);
    let f = Factory::load(&c.template).map_err(|e| e.to_string())?;
    // WHICH CHECKPOINT? Two constraints pull opposite ways: the shim can only
    // hand over at a point where the decoded input array already exists (before
    // that the handshake fails with `notfound`), and the parent must not yet
    // have passed the record's first sample instant, because everything before
    // the handover is unrecordable. The `clock = 36141 + 25.483*race_ms` fit is
    // a rough guide only -- on 252289 it put the handover at race 1.150 s, past
    // 23 of the 78 samples -- so ladder up from the earliest lroundf count that
    // works and stop at the first one that hands shakes.
    let ladder: Vec<u64> = if c.ckpt > 0 {
        vec![c.ckpt]
    } else {
        vec![600, 1000, 1600, 2600, 4200, 7000, 12000, 20000, 34000, 56000]
    };
    let mut srv = None;
    let mut used = 0u64;
    let mut err = String::from("no checkpoint tried");
    for ck in ladder {
        match start_server_on_file(c, &f, &work, ck, std::path::Path::new(&c.template)) {
            Ok(s) => {
                srv = Some(s);
                used = ck;
                break;
            }
            Err(e) => {
                err = format!("ckpt {}: {}", ck, e);
                if verbose {
                    println!("  {}", err);
                }
            }
        }
    }
    let mut srv = srv.ok_or(err)?;
    let probe = srv.probe_tick().map_err(|e| format!("probe {}", e))?;
    let probe_ms = forkoracle::layout::sample_ms(probe, 0, f.start_offset_ms);
    if verbose {
        println!("handover at lroundf {} -> probe tick {} (race {} ms)", used, probe, probe_ms);
    }
    // NO INPUT PATCH for the locate probes. The staged ghost is the original
    // file, so the child already has the right tape; patching it with the
    // Factory'"'"'s decoded values is at best a no-op and at worst wrong -- on
    // 267859 and 227654 the patched child drove at 1.3 m/s and the locate found
    // nothing but decoys. Observing costs nothing and assumes nothing.
    let lrecs: Vec<forkoracle::forksrv::Rec> = Vec::new();
    let _ = tail_recs(&f.steer, &f.accel, &f.brake, probe);
    let bounds = (-64000.0, 64000.0, -1000.0, 4000.0, -64000.0, 64000.0);
    let mut layout = match anchors {
        // The POSITION comes from a checkpoint where the car was moving (the
        // early handover cannot locate it: a stationary car fails both the
        // "moving triple" filter and the velocity test). The CLOCK is located
        // here, in this process, because its address does NOT transfer -- on
        // 252289 the transferred clock address read 0 in the clean process, the
        // grid gate then matched every call, and the whole run collapsed to a
        // single deduplicated instant.
        Some(a) => {
            let ck = crate::locate::find_clock2(
                &mut srv,
                probe,
                &lrecs,
                f.start_offset_ms,
                100000,
                verbose,
            )
            .map_err(|e| format!("clock: {}", e))?;
            forkoracle::layout::Layout {
                pos: (srv.base as i64 + a.pos_delta) as u64,
                clock: ck.addr,
                clock_bias: a.bias,
                rms: 0.0,
                max_dev: 0.0,
            }
        }
        None => locate_v2(&mut srv, probe, &lrecs, f.start_offset_ms, bounds, 2000, 4000, verbose)
            .map_err(|e| format!("locate {}", e))?,
    };
    if let Some(b) = bias_override {
        layout.clock_bias = b;
    }
    // THE POINTER, IF THE CALLER HAS ONE. See `GatherOpts::pos_from`. It
    // replaces the anchor and nothing else: every test below is unchanged, so
    // a chain that has gone stale fails the same way a bad anchor does.
    let mut pool_segs: Vec<(i64, u32)> = Vec::new();
    if let Some(f) = pos_from {
        let (p, ex) = f(srv.pid(), srv.base)?;
        pool_segs = ex;
        if verbose {
            println!(
                "pointer: the car is at {:#x} (the anchor would have said {:#x}, {:+} bytes)",
                p,
                layout.pos,
                p as i64 - layout.pos as i64
            );
        }
        layout.pos = p;
    }
    let gate_phase = if period == 10 {
        // every tick: let the shim take the phase from its own clock
        u32::MAX as i64
    } else {
        (((phase_ms + layout.clock_bias) % period) + period) % period
    };
    // The record's own layout. With anchors, one window around the position
    // covers the quaternion and the velocity wherever they were MEASURED to be;
    // without them, the in-process locator guarantees the classic -16 / +12.
    let (segs, pos_off, quat_off, quat_kind, vel_off): (Vec<(u64, u32)>, usize, usize, u8, usize) = match anchors {
        Some(a) => (
            {
                // arm `whl`: the production window is unchanged, and EXTRA
                // segments are appended AFTER it. The surface and wheel fields
                // do not live inside 448 bytes, but widening the production
                // window itself changes which copy of the car the leader rule
                // picks -- measured on 267460, a 16 KB window chose an object
                // 1091 m from the answer key's own recorded path. So the
                // window that decides the copy stays 448 bytes and the extra
                // ground is carried alongside it.
                let mut s: Vec<(u64, u32)> = vec![
                    (layout.clock, 4),
                    ((layout.pos as i64 - win_back()) as u64, win_len()),
                ];
                for (o, l) in extra.iter().copied().chain(pool_segs.iter().copied()) {
                    s.push(((layout.pos as i64 + o) as u64, l));
                }
                s
            },
            4 + win_back() as usize,
            (4 + win_back() + a.quat_off) as usize,
            a.quat_kind,
            (4 + win_back() + a.vel_off) as usize,
        ),
        None => {
            let mut s: Vec<(u64, u32)> = vec![(layout.clock, 4)];
            for (o, l) in segs_rel {
                s.push(((layout.pos as i64 + *o) as u64, *l));
            }
            // The position's offset in the record follows from where the first
            // segment starts, and it was the constant 20 -- correct for the
            // default `-16:40` and wrong for every other width. A caller that
            // widens `--segs` to give the copy search room would otherwise get
            // a quaternion read 8 KB from the car and a self-check failure that
            // reads like a bad locate.
            let po = 4 + segs_rel.first().map_or(16, |(o, _)| -*o) as usize;
            (s, po, po - 16, 1, po + 12)
        }
    };
    // THE BOUND ON A WIDE READ, taken while the server is still alive: `go`
    // waits for the child, so /proc/<pid>/maps is gone by the time this
    // function returns. See `CleanOut::pos_region`.
    let pos_region = forkoracle::procmem::maps(srv.pid())
        .into_iter()
        .find(|r| layout.pos >= r.start && layout.pos < r.end)
        .map(|r| (r.start, r.end))
        .unwrap_or((layout.pos, layout.pos));
    // ... and it is ENFORCED here, not merely reported. The shim gathers by
    // `copy_nonoverlapping` from raw addresses inside the engine, so a segment
    // that runs off the end of the mapping takes the whole server down with a
    // segfault. Every segment is clipped to the mapping the anchor was found
    // in, less a page at each edge.
    let (blo, bhi) = (pos_region.0 + 4096, pos_region.1.saturating_sub(4096));
    let mut segs: Vec<(u64, u32)> = segs
        .into_iter()
        .filter_map(|(a, l)| {
            let (s, e) = (a.max(blo), (a + l as u64).min(bhi));
            // The clock is not in the vehicle's mapping and is four bytes of a
            // located address rather than a window; it is never clipped.
            if a == layout.clock {
                return Some((a, l));
            }
            (e > s).then_some((s, (e - s) as u32))
        })
        .collect();
    segs.retain(|s| s.1 > 0);
    let reclen: usize = segs.iter().map(|s| s.1 as usize).sum();
    let _ = std::fs::remove_file(dump);
    // The dedup key: see `GatherOpts::dedup`. Defaulting to the whole record is
    // right for the 452-byte production window and writes gigabytes for a wide
    // one, so a wide caller keys on the production window instead.
    let key = dedup.unwrap_or((0, reclen as u32));
    // The last moment the engine is halted and observable. See
    // `GatherOpts::before_go`.
    if let Some(f) = before_go {
        f(srv.pid(), layout.pos, &segs);
    }
    let segs_abs = segs.clone();
    let out = srv
        .go(
            &segs,
            1,
            2_000_000,
            key,
            (layout.clock, period as u32, gate_phase as u32),
            dump,
        )
        .map_err(|e| format!("go {}", e))?;
    let recs = read_samples(dump, reclen);
    // WHICH COPY? The anchor's byte offset does not always transfer: the state
    // is double-buffered and on 252289 the clean process's live copy sat 32
    // bytes from where the anchor process's did. Both copies are
    // self-consistent -- same shape, unit quaternion, matching velocity -- so
    // the structural self-check cannot tell them apart, and the stale one is
    // 84.6 mm off the recorded path where the live one is 0.52 mm.
    //
    // The discriminator needs no reference: among copies that are all valid
    // states of the same car, the LIVE one is the one that is furthest along
    // the direction of travel at the same clock value. Project the difference
    // onto the velocity and take the leader.
    let mut return_choice: Option<usize> = None;
    // The in-process locate hands back FIXED offsets and never searches, which
    // is right when nothing depends on the fields around the position and wrong
    // the moment something does: on this fixture it lands on a bare position
    // copy every time, and with `require_live` set that is a silent write of
    // zeroed wheels. When the caller has named a signature, that path searches
    // too -- its quaternion and velocity offsets relative to the position are
    // the same -16 / +12 the anchored path uses.
    let (pos_off, quat_off, vel_off) = if choose_copy && anchors.is_some() && recs.len() > 20 {
        let g = |r: &Rec, o: usize| f32::from_le_bytes(r.bytes[o..o + 4].try_into().unwrap()) as f64;
        let step = 4usize;
        let mut cands: Vec<usize> = Vec::new();
        let qrel = quat_off as i64 - pos_off as i64;
        let vrel = vel_off as i64 - pos_off as i64;
        // arm `whl`, CORRECTED. This was "only the PRODUCTION window may supply
        // a copy", and that single line cost 90 gathers of 91.
        //
        // Measured, on three maps independently: the car the recording itself
        // records sits EXACTLY 864 BYTES past the copy this rule was allowed to
        // choose from -- 0.000001 m from the recording's own path on 191465,
        // 0.000000 m on 285885, 0.000004 m on 267460, against the 0.930 m,
        // 0.461 m and 0.376 m the restricted scan settled for. The copies are
        // an ARRAY at stride 864 (the same 864 the wheel-block twin rule uses),
        // the locate lands on an arbitrary member, and capping the scan at the
        // 448-byte production window put every other member out of reach. The
        // leader-along-velocity rule was working perfectly on a set of one.
        //
        // The extra segments are still never SEARCHED blindly: the scan runs
        // over whatever was gathered, and the qualifying tests below (velocity
        // self-consistency, unit quaternion, then leader along the direction of
        // travel) are unchanged.
        let copy_hi = copy_scan_hi.unwrap_or(reclen).min(reclen).saturating_sub(12);
        for p in (4..copy_hi).step_by(step) {
            let q = p as i64 + qrel;
            let v = p as i64 + vrel;
            if q < 4 || v < 4 || q as usize + 16 > reclen || v as usize + 12 > reclen {
                continue;
            }
            let (q, v) = (q as usize, v as usize);
            let mut ok = true;
            let mut ds: Vec<f64> = Vec::new();
            let mut sps: Vec<f64> = Vec::new();
            for w in recs.windows(2).step_by((recs.len() / 200).max(1)) {
                let dt = (w[1].clock as i64 - w[0].clock as i64) as f64 / 1000.0;
                if dt <= 0.0 {
                    continue;
                }
                let mut d = 0.0;
                let mut sp = 0.0;
                for k in 0..3 {
                    let a = g(&w[0], p + k * 4);
                    let b = g(&w[1], p + k * 4);
                    let vv = g(&w[0], v + k * 4);
                    if !a.is_finite() || !b.is_finite() || !vv.is_finite() {
                        ok = false;
                    }
                    d += ((b - a) / dt - vv).powi(2);
                    sp += vv * vv;
                }
                ds.push(d.sqrt());
                sps.push(sp.sqrt());
            }
            if !ok || ds.len() < 10 {
                continue;
            }
            let qn: f64 = if quat_kind == 2 {
                (0..3).map(|k| g(&recs[recs.len() / 2], q + k * 4).powi(2)).sum::<f64>().sqrt()
            } else {
                (0..4).map(|k| g(&recs[recs.len() / 2], q + k * 4).powi(2)).sum::<f64>().sqrt()
            };
            if (qn - 1.0).abs() > 1e-3 {
                continue;
            }
            ds.sort_by(|a, b| a.total_cmp(b));
            sps.sort_by(|a, b| a.total_cmp(b));
            let sp = sps[sps.len() / 2];
            if sp < 1.0 || ds[ds.len() / 2] > (0.15 * sp).max(1.0) {
                continue;
            }
            cands.push(p);
        }
        if cands.len() > 1 {
            // WHICH CANDIDATE IS OUR CAR?
            //
            // The leader-along-velocity rule below is correct and stays, but it
            // answers a narrower question than it was being asked: among
            // COPIES OF ONE OBJECT, which is the live one. On 249521 the
            // candidates are not copies -- they are different objects 1080 m
            // apart -- and projecting those onto a velocity and taking the
            // leader is meaningless. It picked an object 1080 m from the car
            // while the car sat in the same window at 0.000000 m.
            //
            // So ask the identifying question FIRST, and only then the
            // live-copy question, which is the ordering the standing fleet
            // notice already prescribes: any test that can IDENTIFY the answer
            // must run before any test that merely ranks survivors.
            //
            // The identifying reference costs nothing and is already in hand:
            // THE TEMPLATE GHOST'S OWN RECORDED TRAJECTORY. Every file this
            // runs on carries one -- a downloaded recording carries the game's,
            // a published ghost carries its own validated positions. It is not
            // a threshold and not a heuristic: it is the run itself.
            //
            // Guarded, because the one class of file that cannot supply it is
            // exactly the class this pipeline was built to repair: a ghost
            // whose position is non-finite or constant. When the reference is
            // unusable the rule abstains and the old behaviour stands.
            // DISABLED BY DEFAULT ON A CONTAMINATED TEMPLATE. FK_NO_CHOOSER=1.
            //
            // Measured on 227654, and predicted in this arm's own addendum §4:
            // the chooser grades candidates against THE TEMPLATE'S OWN recorded
            // positions, so when that record is the DONOR's -- which is exactly
            // the case for every file worth regenerating -- it faithfully picks
            // the object matching the DONOR's path. Four 227654 files with
            // demonstrably different inputs inside the recorded window came out
            // with bit-identical telemetry, 0.000000 m, because all five share
            // one donor record. The guard I wrote was a comment; it is now code.
            let reference: Option<Vec<(i64, [f64; 3])>> = if std::env::var("FK_NO_CHOOSER").is_ok() {
                None
            } else { (|| {
                let (times, raws) = crate::record::targets_from_ghost(&c.template).ok()?;
                let mut out = Vec::new();
                for (i, t) in times.iter().enumerate() {
                    let (p, _, _, _) = gbx::record::read_transform_pub(&raws[i], 47);
                    if !p.iter().all(|v| v.is_finite()) {
                        return None;
                    }
                    out.push((*t, p));
                }
                if out.len() < 20 {
                    return None;
                }
                // a constant trajectory identifies nothing
                let moved = out
                    .windows(2)
                    .map(|w| {
                        (0..3).map(|k| (w[1].1[k] - w[0].1[k]).powi(2)).sum::<f64>().sqrt()
                    })
                    .sum::<f64>();
                if moved < 5.0 {
                    return None;
                }
                Some(out)
            })() };
            if let Some(refr) = reference {
                let by_ms: std::collections::HashMap<i64, [f64; 3]> =
                    refr.iter().map(|(t, p)| (*t, *p)).collect();
                let mut scored: Vec<(f64, usize)> = Vec::new();
                for cd in &cands {
                    let mut e: Vec<f64> = Vec::new();
                    for r in recs.iter() {
                        let ms = r.clock as i64 - layout.clock_bias;
                        let Some(tp) = by_ms.get(&ms) else { continue };
                        let mut d = 0.0;
                        for k in 0..3 {
                            let v = g(r, cd + k * 4) - tp[k];
                            d += v * v;
                        }
                        e.push(d.sqrt());
                    }
                    if e.len() < 10 {
                        continue;
                    }
                    e.sort_by(|a, b| a.total_cmp(b));
                    scored.push((e[e.len() / 2], *cd));
                }
                scored.sort_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((err, cd)) = scored.first().copied() {
                    // 5 cm: far above the 0.5 mm client-vs-server floor and far
                    // below the 0.09 m nearest stale copy ever measured.
                    if err < 0.05 {
                        if verbose {
                            println!(
                                "chooser: record +{} is the car -- {:.6} m from the file's OWN recorded path ({} candidates, runner-up {:.4} m)",
                                cd,
                                err,
                                cands.len(),
                                scored.get(1).map(|s| s.0).unwrap_or(f64::NAN)
                            );
                        }
                        return_choice = Some(cd);
                    } else if verbose {
                        println!(
                            "chooser: no candidate is within 5 cm of the file's own path (best {:.4} m of {}) -- falling back to the live-copy rule",
                            err,
                            cands.len()
                        );
                    }
                }
            }
            // leader along the direction of travel
            let mut best = cands[0];
            let mut bestp = f64::MIN;
            for c in &cands {
                let mut proj: Vec<f64> = Vec::new();
                for r in recs.iter().step_by((recs.len() / 200).max(1)) {
                    let vv: Vec<f64> = (0..3)
                        .map(|k| g(r, (*c as i64 + vrel) as usize + k * 4))
                        .collect();
                    let n = (vv[0] * vv[0] + vv[1] * vv[1] + vv[2] * vv[2]).sqrt();
                    if n < 1.0 {
                        continue;
                    }
                    let mut s = 0.0;
                    for k in 0..3 {
                        s += (g(r, c + k * 4) - g(r, cands[0] + k * 4)) * vv[k] / n;
                    }
                    proj.push(s);
                }
                if proj.is_empty() {
                    continue;
                }
                proj.sort_by(|a, b| a.total_cmp(b));
                let m = proj[proj.len() / 2];
                if m > bestp {
                    bestp = m;
                    best = *c;
                }
            }
            if verbose {
                println!(
                    "live copy: position at record +{} (anchor said +{}), {} candidate copies",
                    best,
                    pos_off,
                    cands.len()
                );
            }
            let best = return_choice.unwrap_or(best);
            (best, (best as i64 + qrel) as usize, (best as i64 + vrel) as usize)
        } else if cands.len() == 1 && cands[0] != pos_off {
            (
                cands[0],
                (cands[0] as i64 + qrel) as usize,
                (cands[0] as i64 + vrel) as usize,
            )
        } else {
            (pos_off, quat_off, vel_off)
        }
    } else {
        (pos_off, quat_off, vel_off)
    };
    // THE COPY WITH A CAR IN IT.
    //
    // Everything above finds A COPY of the vehicle state: the tests are the
    // position moving, the velocity equalling its derivative and the quaternion
    // being a unit one, and a BARE POSITION COPY passes all three -- it holds
    // the car's own position, so its velocity is consistent and the four floats
    // 16 bytes below it are a valid rotation. What it does not hold is any of
    // the FIELDS. Measured on this fixture: the in-process locate lands on one
    // every time, and a regeneration from it writes zeroed wheel rotations,
    // zeroed gear and zeroed suspension into a file that then passes the whole
    // acceptance gate, because none of those bytes affects the simulation.
    //
    // So when a caller has named a signature (`require_live`), step sideways to
    // the copy that has it. A COPY is defined by the thing that makes it one:
    // its position triple equals the located one at every instant. That needs
    // no answer key, no reference and no threshold anyone chose -- the copies
    // are bit-identical on position and everything else in memory is not.
    let (pos_off, quat_off, vel_off) = if !require_live.is_empty() && recs.len() > 20 {
        let sample: Vec<&Rec> = recs.iter().step_by((recs.len() / 40).max(1)).collect();
        let get = |r: &Rec, o: usize| f32::from_le_bytes(r.bytes[o..o + 4].try_into().unwrap());
        let same = |p: usize| -> bool {
            sample.iter().all(|r| {
                (0..3).all(|k| {
                    let (a, b) = (get(r, pos_off + k * 4), get(r, p + k * 4));
                    a == b || (a - b).abs() < 1e-3
                })
            })
        };
        let hi = reclen.saturating_sub(12);
        let mut best: Option<usize> = None;
        for p in (4..hi).step_by(4) {
            if p as i64 - 16 < 4 || !same(p) || !wheels_live(&recs, p, reclen) {
                continue;
            }
            // Nearest to where the locate already was, so a run does not skip
            // over an equally good copy for a further one.
            if best.map_or(true, |b| {
                (p as i64 - pos_off as i64).abs() < (b as i64 - pos_off as i64).abs()
            }) {
                best = Some(p);
            }
        }
        match best {
            Some(p) if p != pos_off => {
                println!(
                    "the copy at record +{} has the named fields; the locate was on +{} \
                     ({:+} bytes), which holds the same position and dead memory",
                    p,
                    pos_off,
                    p as i64 - pos_off as i64
                );
                (p, (p as i64 + quat_off as i64 - pos_off as i64) as usize,
                    (p as i64 + vel_off as i64 - pos_off as i64) as usize)
            }
            Some(_) => (pos_off, quat_off, vel_off),
            None => {
                return Err(format!(
                    "no copy of the located car holds a live value at every named offset {:?} -- \
                     the fields this run was asked to write are not in the gathered window",
                    require_live
                ))
            }
        }
    } else {
        (pos_off, quat_off, vel_off)
    };
    // A run that produced almost no instants is a failed run, not a short one.
    if recs.len() < 8 {
        return Err(format!(
            "self-check: only {} instants sampled -- the grid gate never matched (wrong clock?)",
            recs.len()
        ));
    }
    // And the stream must END where the TAPE ends. If this process's clock is a
    // different counter from the one the anchor phase measured its bias on,
    // every sample is mislabelled by the difference between them -- which looks
    // exactly like a physics divergence. The tape's own length is the check;
    // the page-fault probe is NOT (at the early handover it reads up to a
    // second wrong, which is why the bias comes from a mid-tape checkpoint in
    // the first place).
    {
        let tape_end = (f.steer.len() as i64 - 1) * 10 + f.start_offset_ms as i64;
        let l0 = recs[recs.len() - 1].clock as i64 - layout.clock_bias;
        // ... but "where the tape ends" is only the right landmark when the
        // tape and the run end together. A transplanted ghost's CARRIER is
        // usually a LONGER recording than our run (34 of the 171 published
        // files), and the engine stops simulating shortly after the car
        // crosses the line -- so on those files the stream legitimately ends at
        // the FINISH, ten seconds and more before the tape does. Requiring the
        // tape's end there is a false negative that costs the whole file
        // (measured 2026-08-20 on 285885: stream 50790, tape 61220, run 50.229;
        // every one of 12 runs aborted). The landmark is therefore whichever
        // comes FIRST. A clock bias belonging to another counter still lands
        // the stream end far from both.
        let landmark = match sim_time_of(&out) {
            Some(t) => tape_end.min(t + 1500),
            None => tape_end,
        };
        if (l0 - landmark).abs() > 3000 {
            return Err(format!(
                "self-check: the stream ends at race {} ms but the run ends at {} ms (tape end \
                 {}) -- the clock bias does not belong to this process's counter",
                l0, landmark, tape_end
            ));
        }
    }
    // SELF-CHECK. The anchors may have come from another process's layout, and
    // an address that is merely plausible would produce a plausible file. Two
    // structural tests on the sampled data itself, neither of which needs any
    // reference telemetry: the quaternion 16 B before the position must be a
    // unit quaternion, and the velocity 12 B after it must equal the position's
    // own derivative. Both fail loudly on a wrong address.
    let qerr: f64;
    let verr: f64;
    if self_check && reclen >= 44 && recs.len() > 4 {
        let g = |r: &Rec, o: usize| f32::from_le_bytes(r.bytes[o..o + 4].try_into().unwrap()) as f64;
        let mut qs: Vec<f64> = Vec::new();
        let mut vs: Vec<f64> = Vec::new();
        let mut speeds: Vec<f64> = Vec::new();
        for w in recs.windows(2) {
            let dt = (w[1].clock as i64 - w[0].clock as i64) as f64 / 1000.0;
            if dt <= 0.0 {
                continue;
            }
            // A 3x3 rotation matrix is checked by orthonormality of its first
            // row rather than by a 4-vector norm.
            let q: f64 = if quat_kind == 2 {
                (0..3).map(|k| g(&w[0], quat_off + k * 4) * g(&w[0], quat_off + k * 4)).sum()
            } else {
                (0..4).map(|k| g(&w[0], quat_off + k * 4) * g(&w[0], quat_off + k * 4)).sum()
            };
            qs.push((q.sqrt() - 1.0).abs());
            let mut d = 0.0;
            let mut sp = 0.0;
            for k in 0..3 {
                let dv = (g(&w[1], pos_off + k * 4) - g(&w[0], pos_off + k * 4)) / dt - g(&w[0], vel_off + k * 4);
                d += dv * dv;
                sp += g(&w[0], vel_off + k * 4).powi(2);
            }
            vs.push(d.sqrt());
            speeds.push(sp.sqrt());
        }
        qs.sort_by(|a, b| a.total_cmp(b));
        vs.sort_by(|a, b| a.total_cmp(b));
        speeds.sort_by(|a, b| a.total_cmp(b));
        if !qs.is_empty() {
            qerr = qs[(qs.len() as f64 * 0.995) as usize];
            verr = vs[vs.len() / 2];
            let sp = speeds[speeds.len() / 2];
            if qerr > 1e-3 {
                return Err(format!(
                    "self-check: |q|-1 p99.5 is {:.2e} -- the sampled window is not the vehicle \
                     state (wrong anchor)",
                    qerr
                ));
            }
            // d(pos)/dt is a one-step difference over a 50 ms grid, so a couple
            // of per cent of the speed is normal; ten per cent is not.
            //
            // EXCEPT ON A MAP WHERE THE CAR IS ALWAYS IN CONTACT WITH SOMETHING.
            // A one-step difference averages the 50 ms between two samples,
            // while the stored velocity is the state AT one of them, so the two
            // only agree when the velocity is smooth across the step. On a
            // turtle trial the car rocks on its roof at walking pace and the
            // velocity is discontinuous at every rock: 238835's real car scores
            // 1.41 m/s at a median speed of 7.6, and 0.15 x 7.6 = 1.14 refuses
            // it. That is this project's oldest failure shape -- a threshold
            // that condemns the honest case -- and it is why 186935 and 238835
            // have been unregenerable since the `nan` arm.
            //
            // FK_VERR_FRAC raises the fraction, and it is NOT to be reached for
            // on a hunch: raise it only with a POSITIVE CONTROL on the same map
            // -- regenerate a DOWNLOADED recording of it at the same setting
            // and grade the result against that recording's own bytes. If the
            // known-good run comes back sub-millimetre, the bar was the problem;
            // if it does not, a higher bar is admitting a decoy.
            let frac: f64 = std::env::var("FK_VERR_FRAC")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.15);
            if verr > (frac * sp).max(1.0) {
                return Err(format!(
                    "self-check: median |d(pos)/dt - v| is {:.2} m/s at median speed {:.1} -- \
                     the sampled window is not the vehicle state (wrong anchor)",
                    verr, sp
                ));
            }
        }
    }
    let sim_time = out
        .lines()
        .find(|l| l.trim_start().starts_with("\"Time\""))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|s| s.trim().trim_end_matches(',').parse::<i64>().ok());
    Ok(CleanOut {
        bias: layout.clock_bias,
        reclen,
        sim_time,
        instants: recs.len(),
        first_ms: recs.first().map(|r| r.clock as i64 - layout.clock_bias).unwrap_or(-1),
        last_ms: recs.last().map(|r| r.clock as i64 - layout.clock_bias).unwrap_or(-1),
        probe_ms,
        // arm `whl`: the address the CHOSEN car sits at, not the address the
        // locate proposed. A second gather can then be centred on the car
        // itself -- "find the car, then look around the car" -- instead of on
        // whatever copy the anchor happened to land on.
        pos: (layout.pos as i64 + pos_off as i64 - (4 + win_back())) as u64,
        pos_off,
        quat_off,
        quat_kind,
        vel_off,
        pos_region,
        segs_rel: match anchors {
            Some(_) => {
                let mut v = vec![(-win_back(), win_len())];
                v.extend(extra.iter().copied());
                v
            }
            None => segs_rel.to_vec(),
        },
        segs_abs,
    })
}

/// The anchor checkpoints to try, in order, for a tape of `n` ticks.
///
/// One fixed tick is not enough: a trial map is barely moving at tick 200, a
/// short map has no tick 200 at all, and the locate needs a MOVING car (its
/// whole discriminator is d(pos)/dt against the stored velocity).
///
/// HALVE DOWNWARD, do not sample the tape uniformly. The anchor tick has to
/// land inside the RUN, and the run is usually SHORTER than the tape it is
/// carried in: a transplanted ghost inherits the carrier's input array, so a
/// 9.4 s run can sit in a 50 s tape and n/2, n/4 and 3n/4 are then all past the
/// finish — "server never reached the checkpoint", three times, and the ladder
/// is exhausted (measured 2026-08-20 on TMX 276877: n = 5000, finish at tick
/// 1092). Halving reaches any run length in log2 steps.
///
/// It also puts the probe where the CAR IS DRIVING. The locator qualifies a
/// candidate over the 150 ticks after the probe against a threshold of 2 % of
/// the speed in that window, so an early probe is judged where the car is
/// slowest and where the first collision usually is.
pub fn ladder_ticks(n: i64, biastick: i64) -> Vec<i64> {
    let bt = biastick.min(n / 3).max(60);
    let mut ticks: Vec<i64> = vec![bt];
    let mut k = n / 2;
    while k >= 60 {
        ticks.push(k);
        k /= 2;
    }
    ticks.retain(|t| *t >= 60 && *t < n - 20);
    ticks.dedup();
    ticks
}

/// The record's sample grid: period and phase, read off the ghost.
pub fn grid_of(times: &[i64]) -> (i64, i64) {
    let mut d: Vec<i64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    d.sort_unstable();
    let period = if d.is_empty() { 50 } else { d[d.len() / 2] };
    (period.max(10), times.first().copied().unwrap_or(0))
}


/// The record's OWN sample instants and raw bytes.
///
/// The 50 ms grid a ghost's `CPlugEntRecordData` uses is a property of that
/// file, not a constant: a regenerated sample has to land on the instants the
/// carrier already has, or the record declares a span it does not cover. The
/// vehicle entity is the one with >= 100-byte samples and the most of them.
pub fn targets_from_ghost(path: &str) -> Result<(Vec<i64>, Vec<Vec<u8>>), String> {
    use gbx::record::{find_entrecord_blob, load_body, parse_record_data};
    let body = load_body(path)?;
    let (ver, blob) = find_entrecord_blob(&body)?;
    let rd = parse_record_data(&blob, ver)?;
    let ent = rd
        .ents
        .iter()
        .filter(|e| e.sample_size >= 100 && !e.times.is_empty())
        .max_by_key(|e| e.times.len())
        .ok_or("no vehicle entity with >=100-byte samples")?;
    let ss = ent.sample_size;
    let times: Vec<i64> = ent.times.iter().map(|t| *t as i64).collect();
    let raws: Vec<Vec<u8>> = (0..times.len())
        .map(|i| ent.raw[i * ss..(i + 1) * ss].to_vec())
        .collect();
    Ok((times, raws))
}

// ===========================================================================
// NEUTRALISING THE CARRIER'S OWN BYTES
//
// A regenerated ghost writes the 22 transform bytes (47..69) from engine state
// and the three input-echo bytes (14, 15, 18) from the tape. Everything else in
// the 116-byte sample is still the CARRIER's -- rpm, gear, wheel rotation,
// suspension, surface effects, turbo. On a file grafted into somebody else's
// container those bytes are a stranger's, and the file is quietly part-carrier:
// it re-simulates to the exact millisecond and it is sub-millimetre where it was
// written.
//
// There were two ways out and this project tried the harder one first.
//
// WHAT WAS MEASURED, AND THEN DELETED. `fk fields` swept the whole writable
// address space against a real ghost's own recorded columns; `fk fit` turned
// "this slot correlates with rpm" into an encoding; `fk probe` printed the
// near misses; `fk whl` found the four-wheel rotation block (four f32 at
// stride 44, each accumulating distance / one shared radius, |corr| > 0.9999)
// so the offsets would be a property of the GAME rather than of the run --
// which mattered, because two runs of the same map land on different copies of
// the car state and an offset from the position anchor reproduced rpm on 0.2%
// of samples on a second map. It worked: sample byte 5 is
// `round(0.008489 * slot@pos-240812)`, exact on 439 of 474 recorded samples and
// within one quantisation step on all 474.
//
// None of it is on a live path. Every production recipe in this project runs
// `--fieldmap none` or a "neutral map" that is nothing but a list of byte
// offsets to zero, so ~3,300 lines were maintained to produce encodings nobody
// wrote. The 3,300 lines are deleted; the measurements above are the record.
//
// WHAT IS LEFT is the honest option: zero the per-run bytes we do not write, so
// no per-run byte of the donor survives, and say so. Byte 89 (ground contact)
// and byte 31 bit 7 (reactor) were withdrawn as unreliable and are zeroed with
// the rest. `tmtraj check` C10 fails on every neutralised file for exactly this
// reason, and that is sanctioned rather than fixed.
//
// The offsets NOT in this list are format constants (51, 80, 255, 240, 15, 0)
// that are identical in every ghost of every driver, so leaving them alone
// carries no provenance.

// The list and the writer live in `gbx::record` -- `tmtraj` has to RECOGNISE
// a neutralised record to tell a removed field from a stolen one, and two
// copies of this list would be the oldest bug in this project.
pub use gbx::record::{neutralise, NEUTRALISE};

/// Which sample bytes a run WRITES, for the provenance line.
///
/// `xform` is false when `--keep-transform` was asked for: the file's transform
/// is already regenerated and already validated, and rewriting it would re-run
/// the copy choice and could silently replace a checked trajectory with another
/// one.
pub fn written_bytes(ss: usize, xform: bool, neutral: bool) -> Vec<bool> {
    let mut w = vec![false; ss];
    if xform {
        for b in w.iter_mut().take(69.min(ss)).skip(47) {
            *b = true;
        }
    }
    if neutral {
        for &o in NEUTRALISE {
            if o < ss {
                w[o] = true;
            }
        }
    }
    w
}
pub fn car_path_len(dump: &str, reclen: usize, pos_off: usize) -> Result<f64, String> {
    let recs = read_samples(dump, reclen);
    if recs.len() < 20 {
        return Err(format!("only {} instants gathered", recs.len()));
    }
    let p = |r: &Rec| -> [f64; 3] {
        let g = |o: usize| f32::from_le_bytes(r.bytes[o..o + 4].try_into().unwrap()) as f64;
        [g(pos_off), g(pos_off + 4), g(pos_off + 8)]
    };
    let mut len = 0.0f64;
    let mut moved = false;
    for w in recs.windows(2) {
        let (a, b) = (p(&w[0]), p(&w[1]));
        let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
        if !d.is_finite() {
            return Err("the gathered positions are not finite".into());
        }
        if d > 1e-6 {
            moved = true;
        }
        len += d;
    }
    if !moved {
        return Err(format!(
            "the position never changes over {} instants -- this is a frozen slot, not a car",
            recs.len()
        ));
    }
    if !(1.0..=1.0e6).contains(&len) {
        return Err(format!("path length {:.1} m is not a driven distance", len));
    }
    Ok(len)
}

/// The time the ENGINE simulated for this run, out of the server's own output.
///
/// This used to be `find the first line starting with "Time"`, and it was
/// correct only by the order the server happens to print in. The server prints
/// TWO results per file: `ValidatedResult` (what it simulated) and
/// `DeclaredResult` (what the FILE CLAIMS). On a finishing run they carry the
/// same number, so no test on a passing file can tell a right parser from a
/// wrong one. **On a DNF `"ValidatedResult" : null` carries no `Time` at all,
/// and the first `"Time"` in the output is then the file's own declaration** —
/// so the old reader answered "this run finished at the time written in the
/// file" for a run that did not finish. That is the phantom-result shape with
/// the oracle removed from the loop, and it fed `race_end`, which decides which
/// recorded instants count as inside the race.
///
/// `ghost::oracle` parses the two into separate fields, so the mistake is not
/// expressible here. `tests/suite.rs::oracle_dnf_does_not_report_the_declared_time`
/// pins it against a captured DNF from the real server.
pub fn sim_time_of(out: &str) -> Option<i64> {
    ghost::oracle::parse_many(out).first().and_then(|r| r.time_ms)
}

/// A file's base name without its `.Ghost.Gbx` suffix — the label a key goes
/// into a table under.
pub fn name_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).replace(".Ghost.Gbx", "").replace(".Replay.Gbx", "")
}
