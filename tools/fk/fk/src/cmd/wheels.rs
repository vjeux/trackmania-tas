//! `fk wheels` — where the full vehicle state (gear, rpm, wheels, materials,
//! turbo) lives relative to the validator's car, and whether it TRACKS.
//!
//! The per-tick readout the env gets (`forkoracle::layout::segments`) is the
//! clock, the quaternion, the position, the velocity, the wetness and the
//! checkpoint counter. The game's 116-byte telemetry sample carries much more,
//! and `vislayout::pack` is the game's own writer for it, transcribed from the
//! binary: give it a `CSceneVehicleVisState` and it produces the sample. So the
//! question "where are gear / rpm / wheel contact / material / slip / turbo" is
//! the question "where is the vis state whose `pack` reproduces the ghost's own
//! samples, tick after tick" — an answer key exists for every ghost.
//!
//! Method (INPUT arm, 2026-09-06): resolve the validator's car (typed chain),
//! scan the parent for every copy of its position triple, treat each as
//! `state + POS_IN_STATE`, gather the whole 0x360-byte struct of up to seven
//! candidates per fork over the run, `pack` each per tick and score every byte
//! against the ghost's raw sample at the same race time. The copy that scores
//! is the state; a frozen copy or a render double-buffer does not TRACK.
//! Everything is printed relative to the validator's CGameVehiclePhy object
//! and to its position triple, so the offset is transportable.

use crate::locate::gather_ticks;
use crate::session::{Checkpoint, Engine, Session};
use crate::tape::Tape;
use crate::vislayout::{self, State, POS_IN_STATE, STATE_SIZE};
use forkoracle::procmem;
use std::collections::BTreeMap;

pub struct WheelsOpts {
    /// Compare against this many sample-time shifts (ms) and report the best.
    pub shifts: Vec<i64>,
    /// Print the per-byte table for the winner.
    pub bytes: bool,
    /// Also dump the winner's per-tick decoded fields to this CSV.
    pub out: Option<String>,
}

struct Win<'a> {
    b: &'a [u8],
}
impl State for Win<'_> {
    fn f32(&self, off: usize) -> f32 {
        self.b.get(off..off + 4).map(|x| f32::from_le_bytes(x.try_into().unwrap())).unwrap_or(0.0)
    }
    fn u32(&self, off: usize) -> u32 {
        self.b.get(off..off + 4).map(|x| u32::from_le_bytes(x.try_into().unwrap())).unwrap_or(0)
    }
    fn u8(&self, off: usize) -> u8 {
        self.b.get(off).copied().unwrap_or(0)
    }
    fn covers_state(&self) -> bool {
        self.b.len() >= STATE_SIZE as usize
    }
}

fn scan_triple(pid: i32, want: [f32; 3], tol: f64) -> Vec<u64> {
    let mut out = Vec::new();
    for r in procmem::maps(pid) {
        if !r.perms.starts_with("rw") || r.path == "[vvar]" || r.path == "[vsyscall]" {
            continue;
        }
        let Some(buf) = procmem::read_at(pid, r.start, (r.end - r.start) as usize) else { continue };
        let f = |o: usize| f32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
        let mut o = 0usize;
        while o + 12 <= buf.len() {
            if (f(o) - want[0]).abs() as f64 <= tol
                && (f(o + 4) - want[1]).abs() as f64 <= tol
                && (f(o + 8) - want[2]).abs() as f64 <= tol
            {
                out.push(r.start + o as u64);
            }
            o += 4;
        }
    }
    out
}

/// The named fields of a sample, decoded the way `gbx::record` does, for the
/// per-field agreement table. Bytes only: the comparison is in the wire domain.
const NAMED: &[(&str, &[usize])] = &[
    ("rpm (u16@4)", &[4, 5]),
    ("fl_wheel_rot (u16@6)", &[6, 7]),
    ("fr_wheel_rot (u16@8)", &[8, 9]),
    ("rr_wheel_rot (u16@10)", &[10, 11]),
    ("rl_wheel_rot (u16@12)", &[12, 13]),
    ("steer echo b14", &[14]),
    ("gas/brake echo b15,16", &[15, 16]),
    ("turbo_time b21", &[21]),
    ("steer angle b22", &[22]),
    ("fl_dampen b23", &[23]),
    ("fl material b24", &[24]),
    ("fr_dampen b25", &[25]),
    ("fr material b26", &[26]),
    ("rr_dampen b27", &[27]),
    ("rr material b28", &[28]),
    ("rl_dampen b29", &[29]),
    ("rl material b30", &[30]),
    ("is_turbo/contact bits b31", &[31]),
    ("wheel slip/flag bits b32,33", &[32, 33]),
    ("icing b81..84", &[81, 82, 83, 84]),
    ("reactor/gear word b89..91", &[89, 90, 91]),
    ("gear (b91)", &[91]),
];

pub fn run(engine: &Engine, tape: Tape, at: Checkpoint, o: WheelsOpts) -> Result<(), String> {
    let ghost_path = tape.path.clone();
    let mut s = Session::start(engine, tape, at)?;
    let probe = s.probe_tick()?;
    let recs = s.tape.tail_records(probe);
    let _ = &recs;
    // The car is DERIVED (LOCATE.md), not located: the same objects the old
    // validator chain named, without the sweep.
    let car = forkoracle::car::locate(&s.srv).map_err(|e| format!("the car did not derive: {}", e))?;
    let lay = car.layout();
    let prov = Prov { participant: car.participant, playground: car.playground, sim: car.sim, controller: car.controller, vehicle: car.phy };
    let pid = s.srv.pid();
    println!(
        "validator car: CGameVehiclePhy {:#x}, position {:#x} (vehicle+{:#x}); clock {:#x} bias {:+}; probe tick {} (race {})",
        prov.vehicle,
        lay.pos,
        lay.pos - prov.vehicle,
        lay.clock,
        lay.clock_bias,
        probe,
        crate::secs(s.tape.race_ms(probe))
    );
    let want = forkoracle::car::read_xyz(pid, lay.pos).ok_or("cannot read the car")?;
    let hits = scan_triple(pid, want, 2.0);
    println!("{} position-triple copies within 2 m of the car in the parent", hits.len());
    // every copy is a candidate state at hit - POS_IN_STATE; plus the vis state
    // (phy+0x848) of each of the participant's four vehicle slots, so a
    // transformed car far from the stale validator car is still a candidate
    let mut cands: Vec<u64> = hits.iter().map(|h| h - POS_IN_STATE as u64).collect();
    for k in 0..4u64 {
        if let Some(b) = procmem::read_at(pid, prov.participant + 0x1118 + 0x10 * k, 8) {
            let phy = u64::from_le_bytes(b[..8].try_into().unwrap());
            let live = procmem::read_at(pid, phy + 0x10, 4).map(|b| u32::from_le_bytes(b[..4].try_into().unwrap())).unwrap_or(0);
            println!("  slot {k}: phy {:#x}, phy+0x10 = {:#x}{}", phy, live, if live != 0xffff_ffff { "  (registered = LIVE by the +0x10 rule)" } else { "" });
            if phy != 0 && !cands.contains(&(phy + 0x848)) {
                cands.push(phy + 0x848);
            }
        }
    }

    // the answer key: the ghost's raw samples by race time
    let dec = gbx::record::decode_ghost(&ghost_path).map_err(|e| e.to_string())?;
    let mut key: BTreeMap<i64, Vec<u8>> = BTreeMap::new();
    for (i, smp) in dec.samples.iter().enumerate() {
        if let Some(raw) = dec.raw_sample(i) {
            key.insert(smp.time_ms as i64, raw.to_vec());
        }
    }
    println!("answer key: {} samples of {} bytes", key.len(), dec.sample_size);

    let ticks = (s.tape.n() - probe + 100) as u32;
    let mut scored: Vec<(u64, f64, i64, Vec<(usize, usize)>, Vec<(i64, [u8; 116])>)> = Vec::new();
    for group in cands.chunks(7) {
        let mut segs = vec![(lay.clock, 4u32)];
        for c in group {
            segs.push((*c, STATE_SIZE as u32));
        }
        let rows = gather_ticks(&mut s.srv, probe, &recs, &segs, ticks, 200_000, (0, 4));
        for (k, c) in group.iter().enumerate() {
            let off = 4 + k * STATE_SIZE as usize;
            let mut best: Option<(f64, i64, Vec<(usize, usize)>, Vec<(i64, [u8; 116])>)> = None;
            for shift in &o.shifts {
                let mut agree = vec![(0usize, 0usize); 116];
                let mut packed = Vec::new();
                for t in &rows {
                    let race = t.clock as i64 - lay.clock_bias + shift;
                    let Some(want) = key.get(&race) else { continue };
                    let w = Win { b: &t.rec[off..off + STATE_SIZE as usize] };
                    let p = vislayout::pack(&w);
                    for b in 0..116.min(want.len()) {
                        agree[b].1 += 1;
                        if p[b] == want[b] {
                            agree[b].0 += 1;
                        }
                    }
                    packed.push((race, p));
                }
                // score = mean exact rate over the predicted bytes
                let (mut sum, mut n) = (0.0, 0);
                for b in (0..116usize).filter(|b| !gbx::sample::UNPREDICTED.contains(b) && !gbx::sample::DEAD_IN_SERVER.contains(b)) {
                    if agree[b].1 > 0 {
                        sum += agree[b].0 as f64 / agree[b].1 as f64;
                        n += 1;
                    }
                }
                let score = if n > 0 { sum / n as f64 } else { 0.0 };
                if best.as_ref().map(|b| score > b.0).unwrap_or(true) {
                    best = Some((score, *shift, agree, packed));
                }
            }
            if let Some((score, shift, agree, packed)) = best {
                scored.push((*c, score, shift, agree, packed));
            }
        }
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("\ncandidate states, best shift, mean exact rate over the predicted bytes (116 minus UNPREDICTED minus DEAD_IN_SERVER):");
    for (c, score, shift, _, packed) in scored.iter().take(12) {
        println!(
            "  state {:#x} = vehicle{:+#x} = pos{:+} : {:.2} % (shift {:+} ms, {} paired instants)",
            c,
            *c as i64 - prov.vehicle as i64,
            *c as i64 - lay.pos as i64,
            100.0 * score,
            shift,
            packed.len()
        );
    }
    let Some((c, score, shift, agree, packed)) = scored.first() else {
        return Err("no candidate".into());
    };
    println!(
        "\nWINNER state {:#x} (vehicle{:+#x}, pos{:+}): {:.2} % mean exact at shift {:+} ms",
        c,
        *c as i64 - prov.vehicle as i64,
        *c as i64 - lay.pos as i64,
        100.0 * score,
        shift
    );
    find_owner(pid, &prov, *c);
    if let Some(b) = procmem::read_at(pid, *c, 0x360) {
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        println!(
            "  winner state now: +0x8 u8 {} | +0xa u8 {} | +0x88 flags {:#x} | +0x198 rpm {:.1} | +0x1a4 gear {} | +0x1bc {} | +0x328 wetness {:.3} | +0x344 u8 {} | +0x348 {:#x}",
            b[0x8], b[0xa], u(0x88), f32::from_bits(u(0x198)), u(0x1a4), u(0x1bc), f32::from_bits(u(0x328)), b[0x344], u(0x348)
        );
    }
    slots_report(pid, &prov, c.wrapping_sub(0x848));
    live_flag_candidates(pid, &prov, c.wrapping_sub(0x848));
    if std::env::var("FK_WHEELS_HOLDERS").is_ok() {
        pointer_holders(pid, &prov, c.wrapping_sub(0x848), "live phy");
        pointer_holders(pid, &prov, *c, "live vis state");
    }
    println!("\n{:<28} {:>8} {:>7}", "field (sample bytes)", "exact %", "n");
    for (name, bytes) in NAMED {
        // a field agrees when every one of its bytes agrees on that instant: use the min over bytes
        let n = bytes.iter().map(|b| agree[*b].1).min().unwrap_or(0);
        let ok = bytes.iter().map(|b| agree[*b].0).min().unwrap_or(0);
        println!("{:<28} {:>7.2}% {:>7}", name, if n > 0 { 100.0 * ok as f64 / n as f64 } else { 0.0 }, n);
    }
    if o.bytes {
        println!("\nper byte:");
        for b in 0..116 {
            let (ok, n) = agree[b];
            let doc = vislayout::DOC.iter().find(|d| d.byte == b).map(|d| format!("{} {}", d.field, d.encoding)).unwrap_or_default();
            println!("  b{:<3} {:>7.2}%  {}", b, if n > 0 { 100.0 * ok as f64 / n as f64 } else { 0.0 }, doc);
        }
    }
    if let Some(out) = &o.out {
        let mut s = String::from("race_ms,gear,rpm_u16,fl_mat,fr_mat,rr_mat,rl_mat,b31,fl_rot,fr_rot,rr_rot,rl_rot,turbo_time\n");
        for (race, p) in packed {
            let u16 = |i: usize| u16::from_le_bytes([p[i], p[i + 1]]);
            s.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                race,
                (p[91] as i32 - 1) / 4,
                u16(4),
                p[24],
                p[26],
                p[28],
                p[30],
                p[31],
                u16(6),
                u16(8),
                u16(10),
                u16(12),
                p[21]
            ));
        }
        std::fs::write(out, s).map_err(|e| e.to_string())?;
        println!("wrote {out}");
    }
    Ok(())
}

/// Who points at the winning state / its presumed CGameVehiclePhy? Scans the
/// validator's participant, playground and sim objects (and a window around
/// the stale vehicle) for 8-byte pointers equal to `state - 0x848` (the phy
/// object, if the layout is the stadium car's) or to `state` itself.
pub fn find_owner(pid: i32, prov: &Prov, state: u64) {
    let phy = state.wrapping_sub(0x848);
    let targets = [("state", state), ("state-0x848 (phy?)", phy)];
    let regions: [(&str, u64, usize); 4] = [
        ("participant", prov.participant, 0x1400),
        ("playground", prov.playground, 0x1000),
        ("sim", prov.sim, 0x400),
        ("stale vehicle", prov.vehicle, 0x1400),
    ];
    for (name, base, len) in regions {
        let Some(buf) = procmem::read_at(pid, base, len) else { continue };
        for (tn, tv) in targets {
            let mut o = 0;
            while o + 8 <= buf.len() {
                let v = u64::from_le_bytes(buf[o..o + 8].try_into().unwrap());
                if v == tv {
                    println!("  owner: {}+{:#x} holds {} ({:#x})", name, o, tn, tv);
                }
                o += 8;
            }
        }
    }
    // does the presumed phy hold a moving position at +0x12f0?
    if let Some(p) = forkoracle::car::read_xyz(pid, phy + 0x12f0) {
        println!("  phy? {:#x} +0x12f0 reads ({:.3}, {:.3}, {:.3})", phy, p[0], p[1], p[2]);
    }
}

/// Dump the participant's vehicle-slot region as pointers/ids: what changes at a
/// car transform. `participant+0x1100 .. +0x1200`.
pub fn dump_slots(pid: i32, prov: &Prov) {
    if let Some(buf) = procmem::read_at(pid, prov.participant + 0x1100, 0x100) {
        let mut o = 0;
        while o + 8 <= buf.len() {
            let v = u64::from_le_bytes(buf[o..o + 8].try_into().unwrap());
            let lo = u32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
            let hi = u32::from_le_bytes(buf[o + 4..o + 8].try_into().unwrap());
            if v != 0 {
                let mark = if v == prov.vehicle { "  <- validator's vehicle (slot 0x1118)" } else { "" };
                println!("  participant+{:#x}: {:#018x}  (u32 {:#x} {:#x}){}", 0x1100 + o, v, lo, hi, mark);
            }
            o += 8;
        }
    }
}

/// Which of the four vehicle slots is live: the one whose vis state (phy+0x848)
/// packs to the ghost's samples is known by then; this prints, for every slot,
/// the phy pointer and its position, and every u32 in the participant that
/// equals a slot index (0..3) — the candidates for the "current car" field.
pub fn slots_report(pid: i32, prov: &Prov, live_phy: u64) {
    let mut live_slot: Option<usize> = None;
    for k in 0..4usize {
        let at = prov.participant + 0x1118 + 0x10 * k as u64;
        let Some(b) = procmem::read_at(pid, at, 8) else { continue };
        let phy = u64::from_le_bytes(b[..8].try_into().unwrap());
        let p = forkoracle::car::read_xyz(pid, phy + 0x12f0).unwrap_or([f32::NAN; 3]);
        let live = phy == live_phy;
        if live {
            live_slot = Some(k);
        }
        println!("  slot {k}: phy {:#x} pos ({:.3}, {:.3}, {:.3}){}", phy, p[0], p[1], p[2], if live { "  <- LIVE" } else { "" });
    }
    if let (Some(k), Some(buf)) = (live_slot, procmem::read_at(pid, prov.participant, 0x1400)) {
        let mut hits = Vec::new();
        let mut o = 0;
        while o + 4 <= buf.len() {
            let v = u32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
            if v == k as u32 && !(0x1100..0x1150).contains(&o) {
                hits.push(o);
            }
            o += 4;
        }
        println!("  u32 == live slot index {k} at participant+{:?}", hits.iter().map(|h| format!("{:#x}", h)).collect::<Vec<_>>());
    }
}

/// Every 8-byte-aligned word in writable memory equal to `target`, printed
/// relative to the validator objects when inside one of them.
pub fn pointer_holders(pid: i32, prov: &Prov, target: u64, label: &str) {
    let mut n = 0;
    for r in procmem::maps(pid) {
        if !r.perms.starts_with("rw") || r.path == "[vvar]" || r.path == "[vsyscall]" || r.path == "[stack]" {
            continue;
        }
        let Some(buf) = procmem::read_at(pid, r.start, (r.end - r.start) as usize) else { continue };
        let mut o = 0;
        while o + 8 <= buf.len() {
            if u64::from_le_bytes(buf[o..o + 8].try_into().unwrap()) == target {
                let a = r.start + o as u64;
                let rel = [("participant", prov.participant), ("playground", prov.playground), ("sim", prov.sim), ("controller", prov.controller), ("live phy", target), ("slot0 phy", prov.vehicle)]
                    .iter()
                    .map(|(n, b)| format!(" {}{:+#x}", n, a as i64 - *b as i64))
                    .collect::<Vec<_>>()
                    .join("");
                println!("  {label} held at {:#x}{}", a, rel);
                n += 1;
            }
            o += 8;
        }
    }
    println!("  {n} holder(s) of {label}");
}

/// Offsets inside the CGameVehiclePhy where the LIVE slot's u32 differs from
/// the other three slots (which agree among themselves): candidates for an
/// "active" flag that would identify the live car without a position test.
pub fn live_flag_candidates(pid: i32, prov: &Prov, live_phy: u64) {
    let mut phys = Vec::new();
    for k in 0..4usize {
        let at = prov.participant + 0x1118 + 0x10 * k as u64;
        let Some(b) = procmem::read_at(pid, at, 8) else { return };
        phys.push(u64::from_le_bytes(b[..8].try_into().unwrap()));
    }
    let bufs: Vec<Vec<u8>> = phys.iter().map(|p| procmem::read_at(pid, *p, 0x1400).unwrap_or_default()).collect();
    if bufs.iter().any(|b| b.len() < 0x1400) {
        return;
    }
    let live = phys.iter().position(|p| *p == live_phy).unwrap_or(0);
    let mut out = Vec::new();
    let mut o = 0;
    while o + 4 <= 0x1400 {
        let v: Vec<u32> = bufs.iter().map(|b| u32::from_le_bytes(b[o..o + 4].try_into().unwrap())).collect();
        let others: Vec<u32> = (0..4).filter(|k| *k != live).map(|k| v[k]).collect();
        if others.iter().all(|x| *x == others[0]) && v[live] != others[0] && (o < 0x12f0 || o >= 0x1320) {
            out.push(format!("+{:#x}: live {:#x} others {:#x}", o, v[live], others[0]));
        }
        o += 4;
    }
    println!("  live-only u32 fields in the phy ({} of them): {}", out.len(), out.join("; "));
}

/// The validator objects the reports below print addresses relative to
/// (what `ValidatorCarProvenance` used to carry; now read off `forkoracle::car::Car`).
pub struct Prov {
    pub participant: u64,
    pub playground: u64,
    pub sim: u64,
    pub controller: u64,
    /// The driven vehicle's `CGameVehiclePhy`.
    pub vehicle: u64,
}
