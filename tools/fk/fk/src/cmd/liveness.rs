//! `fk liveness` — do the wheel fields of the car's vis state move?
//!
//! The engine keeps several copies of the vehicle state. They hold the same
//! position and pass every structural test — unit quaternion, velocity equal to
//! the position's derivative — but only the `CSceneVehicleVisState` has the
//! surrounding fields alive; the others are bare position copies with dead
//! memory around them, and a regeneration anchored on one of those writes
//! zeroed wheel rotations and gear into a file that passes the whole acceptance
//! gate, because none of those bytes affects the simulation.
//!
//! The copy that carries the fields is no longer a question: it is the
//! post-step vis state at `phy+0x848` of the DERIVED car (`forkoracle::car`,
//! LOCATE.md), whose wheel records the engine's own reflection places at
//! `+0xa8 + 44k` (`VEHICLEVISSTATE.md`). This command is the control for that:
//! at the four wheel-record slots, do the rotation floats MOVE? Four live
//! against four dead, nothing in between, one fork.
//!
//! Offsets are reported relative to the vis state's position triple
//! (`vis + 0x50`), which is the anchor every carrier-bytes table uses.
use crate::locate::gather_ticks;
use crate::session::{Checkpoint, Engine, Session};
use crate::tape::Tape;

/// Wheel records, relative to the vis state's position triple: `vis + 0xa8`
/// is `pos + 88`.
pub const WHEEL0: i64 = 88;
pub const WHEEL_STRIDE: i64 = crate::vislayout::WHEEL_STRIDE;
/// Rotation within a wheel record.
pub const WHEEL_ROT: i64 = crate::vislayout::WHEEL_ROT;

pub struct LivenessOpts {
    /// Extra offsets to report, relative to this anchor.
    pub also: Vec<i64>,
}

pub fn run(engine: &Engine, tape: Tape, at: Checkpoint, o: LivenessOpts) -> Result<(), String> {
    let mut s = Session::start(engine, tape, at)?;
    let probe = s.probe_tick()?;
    let recs = s.tape.tail_records(probe);
    let car = forkoracle::car::locate(&s.srv)?;
    println!("car: {}", car);
    let layout = car.layout();
    let anchor = car.vis() + forkoracle::car::build128182::POS_IN_VIS;

    let mut want: Vec<(String, i64)> = (0..4)
        .map(|k| (format!("wheel{k}_rot"), WHEEL0 + WHEEL_STRIDE * k + WHEEL_ROT))
        .collect();
    for a in &o.also {
        want.push((format!("car{a:+}"), *a));
    }
    let lo = want.iter().map(|w| w.1).min().unwrap() - 4;
    let hi = want.iter().map(|w| w.1).max().unwrap() + 8;
    let segs = vec![
        (layout.clock, 4u32),
        (anchor.wrapping_add(lo as u64), (hi - lo) as u32),
    ];
    let rows = gather_ticks(&mut s.srv, probe, &recs, &segs, 600, 4000, (0, 4));
    if rows.len() < 50 {
        return Err(format!("only {} ticks gathered", rows.len()));
    }
    println!("\nanchor {:#x} (vis state position), {} ticks", anchor, rows.len());
    println!("what\toffset\tdistinct\tmin\tmax\tverdict");
    let mut live = 0;
    for (name, off) in &want {
        let i = (off - lo) as usize + 4;
        let vals: Vec<f64> = rows
            .iter()
            .map(|t| f32::from_le_bytes(t.rec[i..i + 4].try_into().unwrap()) as f64)
            .collect();
        let mut d: Vec<u64> = vals.iter().map(|v| v.to_bits()).collect();
        d.sort_unstable();
        d.dedup();
        let (mn, mx) = vals.iter().fold((f64::MAX, f64::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
        // Live means MOVING, not merely non-zero: a constant is as dead as a
        // zero for this purpose, and a dead slot in this engine is exactly one
        // repeated value.
        let alive = d.len() > 8;
        if name.starts_with("wheel") && alive {
            live += 1;
        }
        println!(
            "{name}\tcar{off:+}\t{}\t{:.4}\t{:.4}\t{}",
            d.len(),
            mn,
            mx,
            if alive { "LIVE" } else { "dead" }
        );
    }
    println!();
    match live {
        4 => println!(
            "VERDICT: all four wheel rotations are live -- this anchor IS the copy that \
             carries the fields, so an offset measured from it is an intra-struct offset."
        ),
        0 => println!(
            "VERDICT: no wheel rotation moves -- this anchor is a BARE POSITION COPY. \
             Anything read around it is dead memory, and a regeneration anchored here \
             writes zeros into a file that will still pass every gate."
        ),
        n => println!(
            "VERDICT: {n} of 4 wheel rotations move. That is neither shape this test \
             expects, and it means the wheel-record stride or base is wrong here rather \
             than that the copy is half alive."
        ),
    }
    Ok(())
}
