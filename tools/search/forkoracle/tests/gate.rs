//! THE STATE OBJECTIVE: the gate box, the key language, and what the child
//! records inside it.
//!
//! The evaluator is `pred_core.rs`, which the LD_PRELOAD shim `#[path]`-includes
//! verbatim, so everything here is testing the code that runs inside the game
//! server -- without a game server.

use forkoracle::pred::{parse_gate, parse_key};
use forkoracle::pred_core::{body_axes, key_eval, Eval, Fire, Gate, KeyOp, St, KEYOP_BYTES};

/// The gate that found the launcher on map 228811: the boost deck at the base
/// of the end wall, and the state that fires it.
const BOX_228811: &str = "xmin=56,xmax=136,ymin=48,ymax=54,zmin=704,zmax=713,minspeed=60";

fn gate(key: &str) -> Gate {
    parse_gate(BOX_228811, key).unwrap_or_else(|e| panic!("{}: {}", key, e))
}

/// A quaternion for a yaw of `deg` about +Y, as `(qw, qx, qy, qz)`.
fn yaw(deg: f32) -> [f32; 4] {
    let h = deg.to_radians() / 2.0;
    [h.cos(), 0.0, h.sin(), 0.0]
}

/// A quaternion for a roll of `deg` about the car's forward (+Z) axis.
fn roll(deg: f32) -> [f32; 4] {
    let h = deg.to_radians() / 2.0;
    [h.cos(), 0.0, 0.0, h.sin()]
}

#[test]
fn the_key_language_reads_every_part_of_the_state() {
    let p = [70.0, 50.0, 709.0];
    let v = [30.0, -4.0, -40.0];
    let q = yaw(90.0); // nose along +X
    let k = |s: &str| key_eval(&parse_key(s).unwrap(), St::at(p, v, q));

    assert!((k("speed") - 50.1597f32).abs() < 1e-3);
    assert_eq!(k("vx"), 30.0);
    assert_eq!(k("vy"), -4.0);
    assert_eq!(k("vz"), -40.0);
    assert_eq!(k("px"), 70.0);
    assert_eq!(k("pz"), 709.0);
    // nose along +X after a 90 degree yaw
    assert!((k("nose(1,0,0)") - 1.0).abs() < 1e-5);
    assert!((k("nose(0,0,1)")).abs() < 1e-5);
    assert!((k("roof(0,1,0)") - 1.0).abs() < 1e-5);
    // the car points +X, so its own forward velocity is the world vx
    assert!((k("bodyfwd") - 30.0).abs() < 1e-4);
    // and its right axis points -Z, so a -40 m/s vz is +40 out of the window
    assert!((k("bodyright") - 40.0).abs() < 1e-4);
    assert!((k("along(0,0,-1)") - 40.0).abs() < 1e-4);
    assert!((k("dist(70,50,709)")).abs() < 1e-4);
    assert!((k("dist(70,50,700)") - 9.0).abs() < 1e-3);
    assert!((k("vdist(30,-4,-40)")).abs() < 1e-4);
    // arithmetic
    assert_eq!(k("2*3+4"), 10.0);
    assert_eq!(k("-vz"), 40.0);
    assert_eq!(k("min(3,4)"), 3.0);
    assert_eq!(k("max(3,4)"), 4.0);
    assert_eq!(k("abs(0-7)"), 7.0);
    assert!((k("min(abs(bodyright), 5*(-vz))") - 40.0).abs() < 1e-4);
}

/// THE POINT OF THE WHOLE FEATURE, as one assertion.
///
/// On the map this was proven on, position and velocity together were not
/// enough: the launcher ignored both and triggered on which way the car was
/// pointing. Two states identical in every metre and every metre per second,
/// differing only in attitude, must score differently -- otherwise the key is
/// not a function of the whole state and the search cannot see the thing it is
/// hunting.
#[test]
fn the_key_can_tell_two_identical_velocities_apart_by_attitude() {
    let p = [70.0, 50.0, 709.0];
    let v = [30.0, -4.0, -40.0];
    let prog = parse_key("bodyright").unwrap();
    let flat = key_eval(&prog, St::at(p, v, yaw(90.0)));
    let rolled = key_eval(&prog, St::at(p, v, roll(90.0)));
    assert!(
        (flat - rolled).abs() > 10.0,
        "the key scored {} and {} for the same position and velocity in two attitudes",
        flat,
        rolled
    );
}

/// The eleven hard-coded `gate_mode` integers the working version of this had,
/// written as expressions -- and checked against the arithmetic they were,
/// transcribed here, over a sweep of states.
///
/// Two implementations agreeing is the only evidence that replacing one with
/// the other changed nothing, and this is the one place in the feature where a
/// silent difference would look exactly like a different map.
#[test]
fn the_key_language_reproduces_the_objectives_it_replaces() {
    let dir = {
        let n = (0.888f32 * 0.888 + 0.451 * 0.451 + 0.086f32 * 0.086).sqrt();
        [0.888 / n, 0.451 / n, -0.086 / n]
    };
    let vt = [45.0f32, -12.0, -70.0];
    let centre = [96.0f32, 51.0, 708.5];

    let m3 = parse_key("bodyright").unwrap();
    let m4 = parse_key("abs(bodyright)").unwrap();
    let m5 = parse_key("speed*nose(0.888,0.451,-0.086)").unwrap();
    let m6 = parse_key("min(abs(bodyright), -vz)").unwrap();
    let m7 = parse_key("nose(0.888,0.451,-0.086)").unwrap();
    let m8 = parse_key("min(abs(bodyright), -vz) * max(nose(0.888,0.451,-0.086), 0)").unwrap();
    let m9 = parse_key("min(abs(bodyright), 5*(-vz))").unwrap();
    let m10 = parse_key(
        "-(dist(96,51,708.5) + vdist(45,-12,-70)/5 + 10*(1 - nose(0.888,0.451,-0.086)))",
    )
    .unwrap();

    // a deterministic spread of states, including inverted and sideways ones
    let mut seed = 12345u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f32 / (1u32 << 31) as f32) * 2.0 - 1.0
    };
    for _ in 0..400 {
        let p = [96.0 + 40.0 * next(), 51.0 + 3.0 * next(), 708.5 + 4.0 * next()];
        let v = [100.0 * next(), 40.0 * next(), 100.0 * next()];
        let qraw = [next(), next(), next(), next()];
        let n = (qraw[0] * qraw[0] + qraw[1] * qraw[1] + qraw[2] * qraw[2] + qraw[3] * qraw[3])
            .sqrt()
            .max(1e-6);
        let q = [qraw[0] / n, qraw[1] / n, qraw[2] / n, qraw[3] / n];

        // ---- the arithmetic the private fork ran, transcribed
        let a = body_axes(q);
        let bx = a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2];
        let fwd = a[2];
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let dot = fwd[0] * dir[0] + fwd[1] * dir[1] + fwd[2] * dir[2];
        let dp = ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2))
            .sqrt();
        let dv = ((v[0] - vt[0]).powi(2) + (v[1] - vt[1]).powi(2) + (v[2] - vt[2]).powi(2)).sqrt();
        let want10 = -(dp + dv / 5.0 + 10.0 * (1.0 - dot));

        let close = |got: f32, want: f32, what: &str| {
            assert!(
                (got - want).abs() <= 1e-3 * want.abs().max(1.0),
                "{}: the expression gives {} where the hard-coded mode gave {}",
                what,
                got,
                want
            );
        };
        close(key_eval(&m3, St::at(p, v, q)), bx, "mode 3, body-lateral speed");
        close(key_eval(&m4, St::at(p, v, q)), bx.abs(), "mode 4");
        close(key_eval(&m5, St::at(p, v, q)), speed * dot, "mode 5, speed x nose");
        close(key_eval(&m6, St::at(p, v, q)), bx.abs().min(-v[2]), "mode 6, the conjunction");
        close(key_eval(&m7, St::at(p, v, q)), dot, "mode 7, pure nose alignment");
        close(key_eval(&m8, St::at(p, v, q)), bx.abs().min(-v[2]) * dot.max(0.0), "mode 8");
        close(key_eval(&m9, St::at(p, v, q)), bx.abs().min(5.0 * -v[2]), "mode 9, the firing condition");
        close(key_eval(&m10, St::at(p, v, q)), want10, "mode 10, the whole 6-D target state");
    }
}

#[test]
fn a_mistake_in_a_key_is_refused_rather_than_scored() {
    assert!(parse_key("bodylateral").is_err(), "an unknown term was accepted");
    assert!(parse_key("min(1)").is_err(), "min with one argument was accepted");
    assert!(parse_key("nose(1,0)").is_err(), "a two-component direction was accepted");
    assert!(parse_key("nose(0,0,0)").is_err(), "a zero direction was accepted");
    assert!(parse_key("1 +").is_err());
    assert!(parse_key("(1").is_err());
    assert!(parse_key("").is_err());
    assert!(parse_key("speed speed").is_err(), "trailing rubbish was accepted");
}

/// Every bound is required. A gate with a side left open is a box that also
/// contains somewhere else on the map, and it would measure a state
/// convincingly in the wrong place.
#[test]
fn a_gate_with_a_missing_or_inverted_bound_is_refused() {
    assert!(parse_gate("xmin=1,xmax=2,ymin=1,ymax=2,zmin=1", "speed").is_err());
    assert!(parse_gate("xmin=1,xmax=1,ymin=1,ymax=2,zmin=1,zmax=2", "speed").is_err());
    assert!(parse_gate("xmin=1,xmax=2,ymin=1,ymax=2,zmin=1,zmax=2,minspead=3", "speed").is_err());
    assert!(parse_gate("xmin=1,xmax=2,ymin=1,ymax=2,zmin=1,zmax=2", "speed").is_ok());
}

#[test]
fn a_key_op_survives_the_crossing_into_the_child() {
    let g = gate("min(abs(bodyright), 5*(-vz)) * nose(0.888,0.451,-0.086)");
    for k in g.prog.iter() {
        let mut b = [0u8; KEYOP_BYTES];
        k.encode(&mut b);
        let q = KeyOp::decode(&b);
        let mut b2 = [0u8; KEYOP_BYTES];
        q.encode(&mut b2);
        assert_eq!(b, b2);
    }
}

/// Drive a car through the box and check what the child recorded.
fn run(g: Gate, path: &[( [f32; 3], [f32; 3], [f32; 4] )]) -> Eval {
    let mut ev = Eval::ZERO;
    ev.reset();
    ev.gate = g;
    for (i, (p, v, q)) in path.iter().enumerate() {
        ev.feed(i as i32, *p, *v, *q);
    }
    ev
}

/// A straight run across the deck, sideways, with one tick where the car is
/// most sideways of all. The record must be that tick's WHOLE state.
#[test]
fn the_gate_records_the_whole_state_at_its_best_tick() {
    let g = gate("abs(bodyright)");
    let mut path = Vec::new();
    for i in 0..40 {
        let x = 40.0 + 3.0 * i as f32; // enters the box at i = 6
        // most sideways at i = 20
        let side = 60.0 + 30.0 * (1.0 - ((i as f32 - 20.0) / 20.0).abs());
        path.push(([x, 50.0, 708.0], [10.0, 0.0, -side], yaw(90.0)));
    }
    let ev = run(g, &path);
    let s = ev.sum;
    assert_eq!(s.gate_tick, 20, "the record is not at the best tick");
    assert_eq!(s.gate_miss, 0.0, "a run that got inside still reports a miss");
    assert!((s.gate_key - 90.0).abs() < 1e-3);
    assert_eq!(s.gate_pos[0], 40.0 + 3.0 * 20.0);
    assert_eq!(s.gate_pos[2], 708.0);
    assert!((s.gate_vel[2] + 90.0).abs() < 1e-3);
    // AND THE ATTITUDE. This is the field position and velocity did not have.
    let a = body_axes(s.gate_quat);
    assert!((a[2][0] - 1.0).abs() < 1e-4, "the recorded quaternion is not the car's");
}

/// The continuous extension outside the box: how close it came, in metres, and
/// exactly zero once anything got in.
#[test]
fn the_miss_is_the_closest_approach_and_stops_at_the_boundary() {
    let g = gate("abs(bodyright)");
    // a run that passes 7 m short of the box in z and never enters
    let path: Vec<_> = (0..40)
        .map(|i| ([40.0 + 3.0 * i as f32, 50.0, 720.0], [80.0, 0.0, 0.0], yaw(90.0)))
        .collect();
    let ev = run(g, &path);
    assert_eq!(ev.sum.gate_tick, -1, "a run that never entered reported a record");
    assert!((ev.sum.gate_miss - 7.0).abs() < 1e-3, "miss was {}", ev.sum.gate_miss);

    // nearer, but still outside
    let path: Vec<_> = (0..40)
        .map(|i| ([40.0 + 3.0 * i as f32, 50.0, 713.5], [80.0, 0.0, 0.0], yaw(90.0)))
        .collect();
    assert!((run(g, &path).sum.gate_miss - 0.5).abs() < 1e-3);
}

/// The gate ignores ticks below its minimum speed, for the key AND for the
/// miss. A car crawling through the box at the standing start has not arrived.
#[test]
fn a_crawl_through_the_box_is_not_an_arrival() {
    let g = gate("abs(bodyright)");
    let path: Vec<_> = (0..40)
        .map(|i| ([40.0 + 3.0 * i as f32, 50.0, 708.0], [1.0, 0.0, -2.0], yaw(90.0)))
        .collect();
    let ev = run(g, &path);
    assert_eq!(ev.sum.gate_tick, -1, "a 2 m/s crawl through the box counted as an arrival");
    assert!(!ev.sum.gate_miss.is_finite(), "a crawl contributed to the miss");
}

/// A run the watchdog kills is a PREFIX of the same run left alone, and the key
/// is a maximum over ticks, so aborting can only lower it. That is what lets a
/// search arm the watchdog and the gate at once without a dead candidate ever
/// displacing a live one.
#[test]
fn aborting_can_only_lower_the_key() {
    let g = gate("abs(bodyright)");
    let full: Vec<_> = (0..40)
        .map(|i| {
            let side = 60.0 + 30.0 * (1.0 - ((i as f32 - 30.0) / 30.0).abs());
            ([40.0 + 3.0 * i as f32, 50.0, 708.0], [10.0, 0.0, -side], yaw(90.0))
        })
        .collect();
    let whole = run(g, &full).sum;
    for cut in 1..full.len() {
        let part = run(g, &full[..cut]).sum;
        if part.gate_tick >= 0 {
            assert!(
                part.gate_key <= whole.gate_key + 1e-4,
                "a run aborted at tick {} scored {} against the full run's {}",
                cut,
                part.gate_key,
                whole.gate_key
            );
        }
        assert!(
            part.gate_miss >= whole.gate_miss - 1e-4,
            "a run aborted at tick {} reported a NEARER miss than the full run",
            cut
        );
    }
}

/// A key that cannot be evaluated must not silently score zero -- zero is a
/// perfectly plausible key.
#[test]
fn a_program_that_does_not_evaluate_is_not_a_score() {
    let bad = [KeyOp { op: forkoracle::pred_core::KOP_ADD, axis: 0, a: [0.0; 3] }, KeyOp::END];
    assert!(key_eval(&bad, St::at([0.0; 3], [0.0; 3], [1.0, 0.0, 0.0, 0.0])).is_nan());
}

// ---------------------------------------------------------------- the event
//
// A gate is a PLACE. A launch is a THING THAT HAPPENS, and on 228811 it is the
// entire point of the place: the state the gate scores is worth having only
// because the map then fires the car from 323 to 751 km/h in one contact.

/// The 228811 launch detector, and the box the launch has to happen in: the
/// deck downstream of the last checkpoint at x = 80.
fn a_launch_clause(after: &str) -> Fire {
    forkoracle::pred::parse_fire(
        "dspeed",
        10.0,
        1,
        "xmin=56,xmax=80,ymin=48,ymax=54,zmin=704,zmax=713",
        after,
        0,
    )
    .unwrap()
}

fn run_fire(g: Gate, f: Fire, path: &[([f32; 3], [f32; 3], [f32; 4])]) -> Eval {
    let mut ev = Eval::ZERO;
    ev.reset();
    ev.gate = g;
    ev.fire = f;
    for (i, (p, v, q)) in path.iter().enumerate() {
        ev.feed(i as i32, *p, *v, *q);
    }
    ev
}

/// A car crossing the deck westward at 90 m/s from x = 130, which at tick
/// `boost` gains `jump` m/s in one tick and is thrown back east down the track.
///
/// 0.9 m a tick, so it reaches the checkpoint at x = 80 around tick 56 and the
/// far end of the deck at x = 56 around tick 82.
fn deck_run(boost: Option<(usize, f32)>) -> Vec<([f32; 3], [f32; 3], [f32; 4])> {
    let mut out = Vec::new();
    let mut x = 130.0f32;
    let mut v = 90.0f32;
    for i in 0..90 {
        if let Some((t, jump)) = boost {
            if i == t {
                v += jump;
            }
        }
        // west along the deck until the boost throws it back east
        let fired = boost.map(|(t, _)| i >= t).unwrap_or(false);
        let dir = if fired { 1.0 } else { -1.0 };
        x += dir * v * 0.01;
        out.push(([x, 50.0, 708.0], [dir * v, 0.0, -60.0], yaw(90.0)));
    }
    out
}

/// THE LAUNCH DETECTOR. A one-tick rise of tens of m/s has exactly one cause on
/// this map; ordinary driving gains about 2 m/s in a tick and a flight gains
/// 0.1.
#[test]
fn a_one_tick_speed_rise_is_a_launch_and_ordinary_driving_is_not() {
    let g = gate("abs(bodyright)");
    // the author's: 323 -> 751 km/h, about 119 m/s in one tick
    let fired = run_fire(g, a_launch_clause(""), &deck_run(Some((62, 119.0)))).sum;
    assert!(fired.fire_tick >= 0, "a 119 m/s one-tick rise was not detected");
    assert_eq!(fired.fire_tick, 62);
    // 109.3, not the 119 added along x: `dspeed` is the rise in the SPEED, and
    // the car also carries 60 m/s across the deck, so the magnitude grows by
    // less than the component does. The detector measures what the car did,
    // not what one axis did.
    assert!((fired.fire_value - 109.28).abs() < 0.1, "value was {}", fired.fire_value);

    // ordinary driving across the same deck: nothing
    let quiet = run_fire(g, a_launch_clause(""), &deck_run(None)).sum;
    assert_eq!(quiet.fire_tick, -1, "ordinary driving fired the launch detector");
}

/// **PEAK SPEED CANNOT DO THIS JOB, and that is measured rather than asserted.**
/// The human world record on this map reaches 151 m/s at the finish under its
/// own power. A detector thresholded on speed catches it; one thresholded on
/// the one-tick RISE does not.
#[test]
fn peak_speed_is_not_a_launch_detector_but_the_rise_is() {
    let g = gate("abs(bodyright)");
    // a run that accelerates hard and smoothly to 151 m/s -- no discontinuity
    let mut path = Vec::new();
    let mut v = 90.0f32;
    let mut x = 130.0f32;
    for i in 0..90 {
        v = (v + 0.7).min(151.0); // 70 m/s^2, harder than the car can do
        x -= v * 0.01;
        path.push(([x, 50.0, 708.0], [-v, 0.0, -60.0], yaw(90.0)));
        let _ = i;
    }
    assert!(
        path.iter().map(|(_, v, _)| v[0].abs()).fold(0.0f32, f32::max) > 150.0,
        "the fixture never reaches the speed the world record does"
    );
    let by_rise = run_fire(g, a_launch_clause(""), &path).sum;
    assert_eq!(by_rise.fire_tick, -1, "a smooth run to 151 m/s tripped the launch detector");

    // the same threshold on SPEED would fire on it, which is the point
    let by_speed = run_fire(
        g,
        forkoracle::pred::parse_fire("speed", 140.0, 1, "", "", 0).unwrap(),
        &path,
    )
    .sum;
    assert!(by_speed.fire_tick >= 0, "the speed-thresholded control did not fire");
}

/// THE BOX AROUND THE EVENT. A launch fired upstream of a checkpoint the run
/// still has to collect flies beautifully and can pass within a metre of the
/// finish, and the run can never validate -- measured on 228811 as 5 of 6
/// checkpoints, DNF. Without the box the band is a trap.
#[test]
fn a_launch_outside_its_box_does_not_count() {
    let g = gate("abs(bodyright)");
    // boost at tick 5, while the car is still at x > 120: upstream of the
    // checkpoint at x = 80, so outside the fire box
    let early = run_fire(g, a_launch_clause(""), &deck_run(Some((5, 119.0)))).sum;
    assert_eq!(early.fire_tick, -1, "a launch upstream of the checkpoint counted");

    // the same launch, later, inside the box
    let late = run_fire(g, a_launch_clause(""), &deck_run(Some((62, 119.0)))).sum;
    assert!(late.fire_tick >= 0, "the in-box control did not fire");
    assert!(late.fire_pos[0] <= 80.0, "fired at x = {}", late.fire_pos[0]);
}

/// THE AFTER-KEY IS MEASURED ONLY AFTER THE EVENT, and that is not a detail.
///
/// The ordinary route on 228811 passes within 99 m of the finish on its way
/// down the track, so "closest approach to the finish" measured from tick 0
/// pins every candidate at 99 m and flattens the objective exactly where it has
/// to bite.
#[test]
fn the_after_key_ignores_everything_before_the_event() {
    let g = gate("abs(bodyright)");
    let f = a_launch_clause("-dist(130,50,708)");
    // the car starts at x = 130, which is 70 m from the point; it drives AWAY
    // to x = 80, launches, and comes back past it
    let ev = run_fire(g, f, &deck_run(Some((62, 119.0)))).sum;
    assert!(ev.fire_tick >= 0);
    assert!(ev.after_tick > ev.fire_tick, "the after-key was taken before the event");
    // it gets much closer after the launch than the 70 m it started at
    assert!(
        ev.after_key > -5.0,
        "the launch carried the car to within {:.1} m and the after-key says {:.1}",
        -ev.after_key,
        ev.after_key
    );

    // and with no launch there is no after-key at all, however close the car
    // came on the way past
    let quiet = run_fire(g, a_launch_clause("-dist(130,50,708)"), &deck_run(None)).sum;
    assert_eq!(quiet.after_tick, -1, "an after-key was measured with no event");
}

/// The event is the FIRST tick the condition holds: a candidate that crosses
/// the threshold twice does not get to choose.
#[test]
fn the_event_is_the_first_tick_and_cannot_be_re_fired() {
    let g = gate("abs(bodyright)");
    let mut path = deck_run(Some((62, 119.0)));
    // a second, bigger jump later
    for (i, p) in path.iter_mut().enumerate() {
        if i > 70 {
            p.1[0] += 300.0;
        }
    }
    let ev = run_fire(g, a_launch_clause(""), &path).sum;
    assert_eq!(ev.fire_tick, 62, "a later, larger jump displaced the first one");
}

/// Aborting can only remove ticks, so it can only LOSE the event and only lower
/// the after-key -- the same property the gate key has, and the reason a search
/// may arm the watchdog and the event clause together.
#[test]
fn aborting_can_only_lose_the_event() {
    let g = gate("abs(bodyright)");
    let f = a_launch_clause("-dist(130,50,708)");
    let full = deck_run(Some((62, 119.0)));
    let whole = run_fire(g, f, &full).sum;
    for cut in 1..full.len() {
        let part = run_fire(g, f, &full[..cut]).sum;
        if part.fire_tick >= 0 {
            assert_eq!(part.fire_tick, whole.fire_tick, "a prefix fired at a different tick");
        }
        if part.after_tick >= 0 {
            assert!(
                part.after_key <= whole.after_key + 1e-4,
                "a run aborted at {} scored {} after the event against the full run's {}",
                cut,
                part.after_key,
                whole.after_key
            );
        }
    }
}

#[test]
fn a_malformed_event_clause_is_refused() {
    assert!(forkoracle::pred::parse_fire("dspeeed", 10.0, 1, "", "", 0).is_err());
    assert!(forkoracle::pred::parse_fire("dspeed", 10.0, 1, "xmin=1,xmax=2", "", 0).is_err());
    assert!(forkoracle::pred::parse_fire("dspeed", 10.0, 1, "", "dist(1,2)", 0).is_err());
    assert!(forkoracle::pred::parse_fire("dspeed", 10.0, 1, "", "", 0).is_ok());
}

/// THE SECOND DECOY FAMILY, and the symptom that catches it.
///
/// The startup decoy test catches "doing less scores more". It does not catch a
/// fast, driven tape that maximises the key SOMEWHERE USELESS -- and that is
/// what happened here, on this map, measured: with the box spanning the whole
/// 80 m deck the search took the firing conjunction to 100.5, above the
/// author's own 86.8, at x = 122.7, forty metres upstream of the checkpoint the
/// run still has to collect. A perfect state in a place where it cannot pay.
///
/// The symptom is NOT "against a face" -- that winner sat 83% of the way across
/// its box, comfortably inside. It is how far the optimum has MIGRATED from
/// where the seed itself crossed.
#[test]
fn a_state_that_migrates_across_its_own_box_is_reported() {
    let g = gate("abs(bodyright)");
    // the real numbers: the seed crossed at x = 58.95, the winner at 122.69,
    // in a box spanning 56..136
    let seed = [58.95, 50.04, 714.92];
    let winner = [122.69, 53.51, 706.17];
    let m = g.migration(seed, winner).expect("the 228811 migration was not reported");
    assert!(m.contains("x +63.7"), "{}", m);
    assert!(m.contains("80%"), "{}", m);

    // AND WHY IT IS A REPORT AND NOT A VERDICT. The author's own contact --
    // the CORRECT answer, in the same box -- migrates 51% of the z span,
    // because that axis is only 9 m thick and he legitimately crosses low.
    let right = g.migration(seed, [71.38, 50.36, 710.34]).unwrap();
    assert!(right.contains("z"), "{}", right);
    assert!(right.contains("51%"), "{}", right);
    // 80% and 51%: any threshold that separates them is a threshold fitted to
    // two points, so there is no threshold here. The numbers are printed and a
    // person decides.
    assert!(!right.contains("80%"));
}

// --------------------------------------------- the load detector (284238)
//
// A car whose wheels have left the ground is a FREE RIGID BODY: its body-frame
// angular rate is then EXACTLY constant, bit-identical for tens of ticks. That
// is measurable and position, velocity and attitude cannot see it -- tapes
// matched to a human reference at 0.13 m, vz -25.13 and omega within 1.4 deg/s
// on all three axes still take the wrong branch.

/// Advance a quaternion by a body-frame rate for one 10 ms tick.
fn spin(q: [f32; 4], rate_deg_s: [f32; 3]) -> [f32; 4] {
    let dt = 0.01f32;
    let w = [
        rate_deg_s[0].to_radians() * dt,
        rate_deg_s[1].to_radians() * dt,
        rate_deg_s[2].to_radians() * dt,
    ];
    let ang = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    let dq = if ang < 1e-12 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        let (s, c) = ((ang / 2.0).sin() / ang, (ang / 2.0).cos());
        [c, w[0] * s, w[1] * s, w[2] * s]
    };
    // q * dq: the increment is in the BODY frame, so it multiplies on the right
    let (aw, ax, ay, az) = (q[0], q[1], q[2], q[3]);
    let (bw, bx, by, bz) = (dq[0], dq[1], dq[2], dq[3]);
    let r = [
        aw * bw - ax * bx - ay * by - az * bz,
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
    ];
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2] + r[3] * r[3]).sqrt();
    [r[0] / n, r[1] / n, r[2] / n, r[3] / n]
}

/// `body_omega` must read back the rate that generated the rotation.
#[test]
fn the_body_rate_reads_back_what_produced_it() {
    for rate in [[0.0, 0.0, 0.0], [250.0, 0.0, 0.0], [0.0, -90.0, 0.0], [40.0, -33.0, 121.0]] {
        let q0 = yaw(37.0);
        let q1 = spin(q0, rate);
        let got = forkoracle::pred_core::body_omega(q0, q1, 0.01);
        for i in 0..3 {
            assert!(
                (got[i] - rate[i]).abs() < 0.5,
                "axis {}: read {:?} for a rate of {:?}",
                i,
                got,
                rate
            );
        }
    }
    // q and -q are the same rotation: the shortest arc, not 359 degrees
    let q0 = yaw(0.0);
    let q1 = spin(q0, [100.0, 0.0, 0.0]);
    let flipped = [-q1[0], -q1[1], -q1[2], -q1[3]];
    let a = forkoracle::pred_core::body_omega(q0, q1, 0.01);
    let b = forkoracle::pred_core::body_omega(q0, flipped, 0.01);
    assert!((a[0] - b[0]).abs() < 0.01, "{:?} vs {:?}", a, b);
}

/// **THE LOAD DETECTOR.** A free rigid body holds its body-frame rate exactly;
/// a loaded wheel changes it every tick. `domega` separates them and nothing
/// else in the language does.
#[test]
fn domega_separates_a_free_rigid_body_from_a_loaded_one() {
    let g = gate("abs(bodyright)");
    // both runs are at the SAME place, the SAME velocity, and are ROTATING at
    // the same rate -- the only difference is whether that rate is constant.
    let make = |loaded: bool| {
        let mut q = yaw(20.0);
        let mut out = Vec::new();
        for i in 0..60 {
            let rate = if loaded {
                // a wheel bites: the rate wanders
                [260.0 + 14.0 * (i as f32 * 0.7).sin(), 3.0, -2.0]
            } else {
                [260.0, 3.0, -2.0]
            };
            q = spin(q, rate);
            out.push(([70.0, 50.0, 708.0], [0.0, 0.0, -80.0], q));
        }
        out
    };
    let free = make(false);
    let loaded = make(true);

    // sanity: both are turning hard, so a rate threshold cannot separate them
    let by_rate = forkoracle::pred::parse_fire("omega", 200.0, 1, "", "", 0).unwrap();
    assert!(run_fire(g, by_rate, &free).sum.fire_tick >= 0);
    assert!(
        run_fire(g, by_rate, &loaded).sum.fire_tick >= 0,
        "the control does not fire, so this fixture cannot show what domega adds"
    );

    // THE DISCRIMINATION: |domega| under half a degree per tick, held three
    // ticks, is a free body.
    let by_load = forkoracle::pred::parse_fire("-domega", -0.5, 3, "", "", 0).unwrap();
    assert!(run_fire(g, by_load, &free).sum.fire_tick >= 0, "a free rigid body did not read as one");
    assert_eq!(
        run_fire(g, by_load, &loaded).sum.fire_tick,
        -1,
        "a loaded car read as a free rigid body"
    );
}

/// `--fire-need` is what makes a load detector a detector: `domega` is near
/// zero for a single tick whenever the car happens not to be turning, and what
/// distinguishes a free body is that it STAYS there.
#[test]
fn the_event_can_require_consecutive_ticks() {
    let g = gate("abs(bodyright)");
    // a rate that is momentarily steady every few ticks and never for long
    let mut q = yaw(0.0);
    let mut path = Vec::new();
    for i in 0..60 {
        let r = if i % 4 == 0 { 100.0 } else { 100.0 + 20.0 * ((i % 4) as f32) };
        q = spin(q, [r, 0.0, 0.0]);
        path.push(([70.0, 50.0, 708.0], [0.0, 0.0, -80.0], q));
    }
    let one = forkoracle::pred::parse_fire("-domega", -0.5, 1, "", "", 0).unwrap();
    let three = forkoracle::pred::parse_fire("-domega", -0.5, 3, "", "", 0).unwrap();
    assert!(run_fire(g, one, &path).sum.fire_tick >= 0, "need=1 found no steady tick");
    assert_eq!(
        run_fire(g, three, &path).sum.fire_tick, -1,
        "need=3 fired on a rate that is never steady for three ticks"
    );
}

/// **A WINDOW WHOSE END THE CANDIDATE CHOOSES IS A DECOY THE INSTRUMENT
/// BUILDS.** `--after-ticks` fixes the window in ticks from the event, so a
/// candidate cannot shorten the interval it is judged over by dying early or by
/// missing whatever the window was keyed on.
#[test]
fn the_after_window_can_be_fixed_in_ticks() {
    let g = gate("abs(bodyright)");
    let f_open = a_launch_clause("py");
    let mut f_fixed = a_launch_clause("py");
    f_fixed.after_ticks = 5;

    // a run that keeps climbing long after the launch
    let mut path = deck_run(Some((62, 119.0)));
    for (i, p) in path.iter_mut().enumerate() {
        if i > 62 {
            p.0[1] = 50.0 + (i - 62) as f32; // 1 m a tick
        }
    }
    let open = run_fire(g, f_open, &path).sum;
    let fixed = run_fire(g, f_fixed, &path).sum;
    assert!(open.after_key > fixed.after_key, "the fixed window did not bound anything");
    assert!(
        (fixed.after_key - 55.0).abs() < 1.5,
        "five ticks of a 1 m/tick climb should be ~55 m, got {}",
        fixed.after_key
    );
}
