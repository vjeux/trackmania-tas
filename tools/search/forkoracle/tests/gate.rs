//! THE STATE OBJECTIVE: the gate box, the key language, and what the child
//! records inside it.
//!
//! The evaluator is `pred_core.rs`, which the LD_PRELOAD shim `#[path]`-includes
//! verbatim, so everything here is testing the code that runs inside the game
//! server -- without a game server.

use forkoracle::pred::{parse_gate, parse_key};
use forkoracle::pred_core::{body_axes, key_eval, Eval, Gate, KeyOp, KEYOP_BYTES};

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
    let k = |s: &str| key_eval(&parse_key(s).unwrap(), p, v, q);

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
    let flat = key_eval(&prog, p, v, yaw(90.0));
    let rolled = key_eval(&prog, p, v, roll(90.0));
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
        close(key_eval(&m3, p, v, q), bx, "mode 3, body-lateral speed");
        close(key_eval(&m4, p, v, q), bx.abs(), "mode 4");
        close(key_eval(&m5, p, v, q), speed * dot, "mode 5, speed x nose");
        close(key_eval(&m6, p, v, q), bx.abs().min(-v[2]), "mode 6, the conjunction");
        close(key_eval(&m7, p, v, q), dot, "mode 7, pure nose alignment");
        close(key_eval(&m8, p, v, q), bx.abs().min(-v[2]) * dot.max(0.0), "mode 8");
        close(key_eval(&m9, p, v, q), bx.abs().min(5.0 * -v[2]), "mode 9, the firing condition");
        close(key_eval(&m10, p, v, q), want10, "mode 10, the whole 6-D target state");
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
    assert!(key_eval(&bad, [0.0; 3], [0.0; 3], [1.0, 0.0, 0.0, 0.0]).is_nan());
}
