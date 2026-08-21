//! `tmtraj check` -- THE REFUSE-TO-PUBLISH CHECK.
//!
//! One command, run on a ghost before it leaves anybody's hands. It answers a
//! single question: **does this file's telemetry belong to this file's run?**
//!
//! WHY IT EXISTS
//! -------------
//! Every defect this project has shipped had the same shape. A ghost is a tape
//! plus a recorded trajectory, and they are separate payloads that can disagree
//! completely: the oracle reads the tape, a video reads the record. So a file
//! can validate to the exact millisecond four different ways and still play
//! back as a stranger driving another map. It has happened, repeatedly:
//!
//!   * a carrier's whole trajectory, published under our lap time;
//!   * a non-finite position that every gate passed, because `err > tol` is
//!     false for NaN and so ACCEPTS it;
//!   * fourteen files whose car never moved, whose own results table recorded
//!     `moved_m = 0.0000` and nobody read it;
//!   * a carrier's post-finish tail teleporting the car 868 m after the line;
//!   * and the one a viewer finally noticed: dirt thrown up in mid-air.
//!
//! Each was found by a different ad-hoc investigation, after publication. This
//! is those investigations as one gate, run before.
//!
//! WHAT IT CANNOT DO
//! -----------------
//! It cannot prove telemetry is right. It proves a file is not obviously
//! somebody else's, and every check is a relationship the file must satisfy
//! against ITSELF or against the map -- no reference recording, so it runs
//! anywhere on anything. A PASS means "no known defect"; it never means
//! "verified".

use crate::whlcmd::{classify, decode, Cls, G_DEFAULT, R};

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 }
}

struct Out {
    fail: usize,
    warn: usize,
    lines: Vec<String>,
}

impl Out {
    fn ok(&mut self, id: &str, msg: String) {
        self.lines.push(format!("PASS {:<6} {}", id, msg));
    }
    fn bad(&mut self, id: &str, msg: String) {
        self.fail += 1;
        self.lines.push(format!("FAIL {:<6} {}", id, msg));
    }
    fn warn(&mut self, id: &str, msg: String) {
        self.warn += 1;
        self.lines.push(format!("WARN {:<6} {}", id, msg));
    }
    fn na(&mut self, id: &str, msg: String) {
        self.lines.push(format!("n/a  {:<6} {}", id, msg));
    }
}

pub fn cmd(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    // A flag's VALUE is not a filename. Filtering only on the leading "--"
    // made `--race 12759` look like a ghost called "12759" and REFUSE it, which
    // is a checker that fails for the wrong reason -- the exact thing it exists
    // to stop.
    let mut files: Vec<&String> = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        if args[i].starts_with("--") {
            if matches!(args[i].as_str(), "--race" | "--g") {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        files.push(&args[i]);
        i += 1;
    }
    let race: i64 = flag("--race").and_then(|v| v.parse().ok()).unwrap_or(-1);
    let g: f64 = flag("--g").and_then(|v| v.parse().ok()).unwrap_or(G_DEFAULT);
    let quiet = args.iter().any(|a| a == "--quiet");
    if files.is_empty() {
        println!(
            "usage: tmtraj check GHOST... [--race MS] [--quiet]\n\
             \n\
             Refuses a ghost whose telemetry is not its own. --race is the\n\
             file's validated time in milliseconds; without it the declared\n\
             race time in the file is used and C4 is reported as unchecked."
        );
        std::process::exit(2);
    }
    let mut worst = 0;
    for f in &files {
        let (code, o) = check_one(f, race, g);
        worst = worst.max(code);
        let verdict = match code {
            0 => "PUBLISHABLE",
            1 => "PUBLISHABLE WITH WARNINGS",
            _ => "REFUSED",
        };
        println!("=== {}  --  {} ({} fail, {} warn)", f, verdict, o.fail, o.warn);
        if !quiet || code != 0 {
            for l in &o.lines {
                println!("  {}", l);
            }
        }
    }
    std::process::exit(worst);
}

fn check_one(path: &str, race_in: i64, g: f64) -> (i32, Out) {
    let mut o = Out { fail: 0, warn: 0, lines: Vec::new() };
    let r: Vec<R> = match decode(path) {
        Ok(v) => v,
        Err(e) => {
            o.bad("C0", format!("no readable vehicle record: {}", e));
            return (2, o);
        }
    };
    let n = r.len();
    if n < 5 {
        o.bad("C0", format!("{} samples -- nothing to check", n));
        return (2, o);
    }
    let race = if race_in > 0 { race_in } else { -1 };

    // C1 FINITE -- written positively. `err > tol -> reject` accepts NaN, and
    // that spelling is why four non-finite ghosts were published.
    let nf = r.iter().filter(|x| !x.finite()).count();
    if nf == 0 {
        o.ok("C1", format!("every position and velocity component is finite ({} samples)", n));
    } else {
        o.bad("C1", format!("{} of {} samples are NON-FINITE", nf, n));
    }

    // C2 MOVES -- zeroed memory is finite, self-consistent, has a unit
    // quaternion, and passes a continuity test perfectly.
    let mut dist = 0.0;
    for i in 1..n {
        let mut d = 0.0;
        for k in 0..3 {
            let q = r[i].pos[k] - r[i - 1].pos[k];
            d += q * q;
        }
        if d.is_finite() {
            dist += d.sqrt();
        }
    }
    let distinct = {
        let mut v: Vec<[i64; 3]> = r
            .iter()
            .map(|x| [(x.pos[0] * 1e3) as i64, (x.pos[1] * 1e3) as i64, (x.pos[2] * 1e3) as i64])
            .collect();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    if dist > 5.0 && distinct > n / 4 {
        o.ok("C2", format!("the car travels {:.1} m over {} distinct points", dist, distinct));
    } else {
        o.bad("C2", format!("the car travels {:.4} m over {} distinct points -- this is not a run", dist, distinct));
    }

    // C3 NO TELEPORT -- the carrier's post-finish tail jumps hundreds of
    // metres. Any step far beyond the run's own step distribution is one.
    // PER-SECOND, not per-step. A record may have GAPS: 286279 jumps from
    // 46.200 to 46.850 s, and a car at 131 km/h legitimately covers 23.7 m in
    // that 650 ms. Measuring raw step distance called that "the car JUMPS
    // 23.1 m -- a carrier tail or a spliced record" on a file with no splice at
    // all, and it did so on the PUBLISHED original too. A teleport is a
    // distance no SPEED explains, so divide by the elapsed time.
    let mut steps: Vec<(f64, i64)> = Vec::new();
    for i in 1..n {
        let mut d = 0.0;
        for k in 0..3 {
            let q = r[i].pos[k] - r[i - 1].pos[k];
            d += q * q;
        }
        let dt = (r[i].ms - r[i - 1].ms) as f64 / 1000.0;
        if d.is_finite() && dt > 0.0 {
            steps.push((d.sqrt() / dt, r[i].ms));
        }
    }
    let mut sv: Vec<f64> = steps.iter().map(|s| s.0).collect();
    sv.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = sv[sv.len() / 2];
    let worst = steps.iter().cloned().fold((0.0, 0i64), |a, b| if b.0 > a.0 { b } else { a });
    // A RESPAWN IS NOT A TELEPORT. On a Trial map the car is thrown back to
    // its last checkpoint and the engine writes its own speed as 0.0 in that
    // sample -- measured on 286279 at 45.850 s: 10.60 m in 50 ms (212 m/s) with
    // the recorded speed dropping 90.6 km/h -> 0.0. A splice leaves the speed
    // field alone, so the car's OWN speed byte separates the two, and it costs
    // nothing: drop any step whose landing sample reads a dead stop.
    let steps: Vec<(f64, i64)> = steps
        .into_iter()
        .zip(r.iter().skip(1))
        // THE SPEEDOMETER IS THE DISCRIMINATOR, not a threshold on distance.
        // Three cases separate cleanly with no magic constant:
        //   real driving  implied/recorded ~= 1.00
        //   RESPAWN       recorded speed EXACTLY 0.0 while the position moves
        //                 8-11 m in 50 ms (153-213 m/s) -- routine on a Trial
        //                 map, and a 200 m/s bar runs straight through the
        //                 middle of that cluster, refusing a genuine human
        //                 recording (286279 HUMANCUT_236972 at 213.3)
        //   SPLICE        ratio in the THOUSANDS (227654 TAS_57503: 50090 m/s
        //                 implied against 19.2 recorded, ratio 2606)
        .filter(|(_, s)| s.speed > 0.5)
        .map(|(x, _)| x)
        .collect();
    // ...AND A RESPAWN DOES NOT ALWAYS ZERO THE SPEEDOMETER. Measured on
    // 286279 AUTHORMIN_831ev: nine jumps of 74-146 m whose landing sample
    // reads 11.87 m/s -- the speed field is FROZEN at its pre-respawn value,
    // not zeroed, so the speedometer filter above runs straight past them and
    // C3 called a legal Trial respawn "a spliced record". The channel that
    // does separate them is MOTION, which is also the channel we trust most:
    //   RESPAWN  the engine HOLDS the car at the respawn point for exactly
    //            1.00 s -- 20-21 samples of bit-identical position. Measured
    //            over the 1.2 s after each landing: <= 0.50 m on 10 of 12
    //            (the other two are sample-time GAPS, dt 650 and 350 ms,
    //            which the implied-speed divide already handles).
    //   SPLICE   the car keeps driving on the far side. 238835 TAS_239133's
    //            three jumps (186.3 / 268.0 / 96.3 m) are each followed by
    //            ~1.10 m per 50 ms = 22 m/s of continued motion.
    // So: exempt a step whose landing is followed by a hold. This is a
    // NARROW exemption -- it rescues nothing that keeps moving.
    let held = |i: usize| -> bool {
        let t0 = r[i].ms;
        let mut d = 0.0f64;
        let mut j = i + 1;
        while j < n && r[j].ms - t0 <= 1000 {
            let mut q = 0.0;
            for k in 0..3 {
                let e = r[j].pos[k] - r[j - 1].pos[k];
                q += e * e;
            }
            if !q.is_finite() {
                return false;
            }
            d += q.sqrt();
            j += 1;
        }
        // A hold needs samples to be held THROUGH; the tail of a file is not
        // a respawn.
        j > i + 4 && d <= 0.5
    };
    // SECOND RESPAWN SHAPE. 186935 ONE_ATTEMPT_DELETED at 2355.500 s jumps
    // 237 m and then DRIFTS for 0.35 s (speed 4.65 -> 3.31) before coming to
    // rest at exactly (920.0001, 249.0000, 834.5001) with the speedometer
    // reading 0.0000. The car IS brought to a dead stop -- the engine just
    // takes a third of a second to do it -- so the hold test above, which
    // measures the first 1.0 s, sees ~0.8 m and says "not a respawn".
    // This is not a new rule and not a new threshold: it is the SAME
    // speedometer discriminator already used above, widened from "the landing
    // sample reads 0.0" to "the car reaches 0.0 within half a second of
    // landing". NO-CONTROL: no jump in 238835 (6 files) or 227654 TAS_57503
    // reaches a dead stop after landing -- those cars keep driving at 22 m/s.
    let stops = |i: usize| -> bool {
        let t0 = r[i].ms;
        let mut j = i;
        while j < n && r[j].ms - t0 <= 500 {
            if r[j].speed <= 0.5 {
                return true;
            }
            j += 1;
        }
        false
    };
    // THIRD, AND THE ONE THAT ACTUALLY SETTLES IT: A RESPAWN RETURNS THE CAR
    // TO A PLACE IT HAS ALREADY BEEN. A splice does not, and cannot.
    // The two shapes above are symptoms; this is the mechanism. 186935
    // "Magnet Trial" raised the question -- can a map feature legitimately
    // displace a car 73 m while it keeps driving at 20 m/s? -- and the answer
    // is a census, not an opinion. Measured over each file's own samples:
    //   186935 ONE_ATTEMPT: landings recur at fixed points, up to 29 times at
    //     (961,331,768), and the restored SPEED is the same every time
    //     (4.60 m/s x29; 12.10 x3; 9.44 x4; 2.09 x4). The engine restores a
    //     saved checkpoint STATE -- which is also why 286279's speedometer
    //     looked "frozen": it is not frozen, it is RESTORED.
    //   Distance from each landing to the nearest EARLIER sample of the same
    //     run: 238835 0.052-0.174 m, 186935 0.248-1.362 m -- the car is put
    //     back on its own track.
    //   227654 TAS_57503, the known donor seam: 2003.622 m. The car has never
    //     been there and never goes back.
    // Three orders of magnitude between the classes, so the bar is not
    // delicate: 2 m.
    let revisit = |i: usize| -> bool {
        // The test READS POSITIONS, so it needs positions. On a file with
        // non-finite samples (186935 CUT_795034: 15572 of 50293) the earlier
        // track is partly missing and a "revisit" cannot be trusted, so the
        // exemption is withheld there and C1 refuses the file anyway.
        if nf > 0 {
            return false;
        }
        let mut best = f64::INFINITY;
        for j in 0..i.saturating_sub(1) {
            let mut e = 0.0;
            for k in 0..3 {
                let q = r[j].pos[k] - r[i].pos[k];
                e += q * q;
            }
            if e.is_finite() && e < best {
                best = e;
            }
        }
        best.sqrt() <= 2.0
    };
    let respawn_held: Vec<i64> = if std::env::var("TMTRAJ_NO_HOLD").is_ok() {
        // Control switch: disables the respawn-hold exemption so the old and
        // new C3 can be run over the same corpus in one build.
        Vec::new()
    } else {
        (1..n).filter(|&i| held(i) || stops(i) || revisit(i)).map(|i| r[i].ms).collect()
    };
    let steps: Vec<(f64, i64)> = steps
        .into_iter()
        .filter(|(_, ms)| !respawn_held.contains(ms))
        .collect();
    let respawn_only = steps.is_empty();
    if respawn_only {
        o.na("C3", "every step lands on a respawn -- nothing to test".into());
    }
    let mut sv: Vec<f64> = steps.iter().map(|s| s.0).collect();
    if sv.is_empty() { sv.push(0.0); }
    sv.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = sv[sv.len() / 2];
    let worst = steps.iter().cloned().fold((0.0, 0i64), |a, b| if b.0 > a.0 { b } else { a });
    // 200 m/s = 720 km/h, comfortably above anything this game does.
    let bar = (p50 * 8.0).max(200.0);
    if respawn_only {
    } else if worst.0 <= bar {
        o.ok("C3", format!("worst implied speed {:.1} m/s at {:.3} s (median {:.1} m/s)", worst.0, worst.1 as f64 / 1000.0, p50));
    } else {
        o.bad("C3", format!(
            "the car moves at {:.0} m/s at {:.3} s (median {:.1}) -- no speed explains this, so it is a teleport: a carrier tail or a spliced record",
            worst.0, worst.1 as f64 / 1000.0, p50
        ));
    }

    // C4 NO SAMPLES AFTER THE FINISH -- what a viewer sees as the car driving
    // on after the line, in somebody else's direction.
    match race {
        r0 if r0 > 0 => {
            let after = r.iter().filter(|x| x.ms > r0 + 60).count();
            if after == 0 {
                o.ok("C4", format!("no samples after the finish at {:.3} s", r0 as f64 / 1000.0));
            } else {
                o.bad("C4", format!("{} samples AFTER the finish at {:.3} s (last {:.3} s)", after, r0 as f64 / 1000.0, r[n - 1].ms as f64 / 1000.0));
            }
        }
        _ => o.na("C4", "no --race given, so the post-finish tail is unchecked".into()),
    }

    // C5 / C6 THE SURFACE AND CONTACT FIELDS, against free fall.
    //
    // Airborne is decided from the trajectory alone -- second difference of the
    // car's own position against g -- so it cannot be fooled by the flag it is
    // checking. g = 25.20 m/s^2, measured on ten downloaded recordings by
    // splitting THEIR samples on THEIR OWN contact flag.
    let v: Vec<R> = r.into_iter().filter(|x| race < 0 || x.ms <= race).collect();
    let c = classify(&v, g, 2.0, 5.0, 3);
    let ball: Vec<usize> = (0..v.len()).filter(|i| c.cls[*i] == Cls::Ballistic).collect();
    let sup: Vec<usize> = (0..v.len()).filter(|i| c.cls[*i] == Cls::Supported).collect();
    let con_b = ball.iter().filter(|i| v[**i].contact()).count();
    let con_s = sup.iter().filter(|i| v[**i].contact()).count();
    if ball.is_empty() {
        o.na("C5", "the car is never unambiguously airborne on this run".into());
    } else if con_b == 0 {
        o.ok("C5", format!("ground contact is OFF on all {} provably airborne samples", ball.len()));
    } else {
        o.bad("C5", format!(
            "ground contact is ON on {} of {} provably AIRBORNE samples -- this is the dirt-at-altitude defect",
            con_b, ball.len()
        ));
    }
    // C6 is the half a ZEROED field fails. Without it, blanking the byte scores
    // a clean pass on C5 and the file is just as wrong.
    if sup.len() < 5 {
        o.na("C6", format!("only {} unambiguously ground-borne samples -- cannot test", sup.len()));
    } else if pct(con_s, sup.len()) >= 95.0 {
        o.ok("C6", format!("ground contact is ON on {:.1} % of {} ground-borne samples", pct(con_s, sup.len()), sup.len()));
    } else {
        o.bad("C6", format!(
            "ground contact is ON on only {:.1} % of {} provably GROUND-BORNE samples -- a zeroed or foreign flag",
            pct(con_s, sup.len()), sup.len()
        ));
    }
    // C7 surface material must never ACCUMULATE with nothing touching the car.
    // It may hold or decay: a real recording carries ice through a jump.
    let rise = |f: &dyn Fn(&R) -> u8| -> usize {
        ball.iter()
            .filter(|i| **i > 0 && c.cls[**i - 1] == Cls::Ballistic && f(&v[**i]) > f(&v[**i - 1]))
            .count()
    };
    let dr = rise(&|x: &R| x.dirt_max());
    let ir = rise(&|x: &R| x.ice_max());
    if ball.is_empty() {
        o.na("C7", "no airborne stretch".into());
    } else if dr == 0 && ir == 0 {
        o.ok("C7", "no surface material accumulates during free fall".into());
    } else {
        o.bad("C7", format!("surface material RISES during free fall: dirt {} times, ice {} times", dr, ir));
    }

    // C10 A CLAIMED FLIGHT MUST FALL LIKE ONE.
    //
    // Added after C6 passed a file whose contact flag was wrong on 143 of 254
    // samples. C5 and C6 assert on classes derived from the motion, and on a
    // map where the car is mostly held up by something that is neither clean
    // free fall nor level ground, BOTH classes are tiny -- 22 and 8 samples on
    // 276874 -- so neither can see a 143-sample error. Eight samples is not a
    // sample size, and a gate that cannot fail on the bulk of a file is not
    // load-bearing.
    //
    // This one reads the flag's OWN claim and holds it to account over its full
    // extent: take every maximal run of contact-OFF samples and compare the
    // height the car actually loses with the height free fall demands. A car
    // that claims 7.75 s of flight must fall about 757 m; if it descends 30 m,
    // or climbs, it was on a surface. No classification, no thresholds on
    // acceleration, and it covers every sample the flag speaks about.
    {
        let mut worst: Option<(f64, f64, f64, f64)> = None; // dur, actual, predicted, t0
        let mut i = 0usize;
        while i < v.len() {
            if v[i].contact() {
                i += 1;
                continue;
            }
            let mut j = i;
            while j < v.len() && !v[j].contact() {
                j += 1;
            }
            let dur = (v[j - 1].ms - v[i].ms) as f64 / 1000.0;
            if dur >= 0.5 {
                let vy0 = if i + 1 < v.len() {
                    (v[i + 1].pos[1] - v[i].pos[1]) / ((v[i + 1].ms - v[i].ms) as f64 / 1000.0)
                } else {
                    0.0
                };
                let pred = vy0 * dur - 0.5 * g * dur * dur;
                let act = v[j - 1].pos[1] - v[i].pos[1];
                // how far the claim is from the physics, as a fraction of the
                // predicted drop
                let err = (act - pred).abs() / pred.abs().max(1.0);
                if worst.map_or(true, |w| err > (w.1 - w.2).abs() / w.2.abs().max(1.0)) {
                    worst = Some((dur, act, pred, v[i].ms as f64 / 1000.0));
                }
            }
            i = j;
        }
        match worst {
            None => o.na("C10", "no claimed flight lasts 0.5 s -- nothing to hold to account".into()),
            Some((dur, act, pred, t0)) => {
                let err = (act - pred).abs() / pred.abs().max(1.0);
                // WARN, not FAIL. Measured on five downloaded recordings: three of
                // them trip this, one of them by +96.8 m against -18.4 m
                // predicted. So CONTACT-OFF DOES NOT IMPLY FREE FALL in this
                // engine -- the game turns the flag off while geometry is still
                // throwing the car around, and a rule that fails on that
                // condemns real recordings. It stays because the MAGNITUDE
                // still discriminates: the worst genuine case predicts an 18 m
                // drop, while the file this check was written for predicts 693 m
                // over a single 7.7 s claim.
                if err < 0.5 {
                    o.ok("C10", format!(
                        "the longest claimed flight ({:.2} s at {:.3} s) falls {:.1} m against {:.1} m predicted",
                        dur, t0, -act, -pred
                    ));
                } else if pred.abs() > 150.0 {
                    o.bad("C10", format!(
                        "the flag claims {:.2} s of flight at {:.3} s, over which the car changes height by {:.1} m -- free fall demands {:.1} m. Nothing in this engine holds a car up for that long with the flag off; the flag is another run's.",
                        dur, t0, act, pred
                    ));
                } else {
                    o.warn("C10", format!(
                        "the flag claims {:.2} s of flight at {:.3} s over a {:.1} m height change against {:.1} m of free fall -- geometry can do this, so it is a WARNING, not a refusal",
                        dur, t0, act, pred
                    ));
                }
            }
        }
    }

    // C8 THE WHEELS ARE WHEELS. The wheel-rotation bytes imply a radius; if
    // they came from another recording the implied radius is not a wheel's.
    // Measured on four downloaded recordings: 0.3639 / 0.3643 / 0.3648 /
    // 0.3651 m.
    {
        let turns = |x: &R| -> f64 { x.b(7) as f64 + x.b(6) as f64 / 255.0 };
        let mut rr: Vec<f64> = Vec::new();
        for i in 1..v.len() {
            // NOT "class == Supported". That gate produced "0 usable rolling
            // steps -- cannot infer a wheel radius" on 145875, INCLUDING on
            // Nadeo's own downloaded rank-1 ghost of that map -- which reads as
            // "this map's wheels are dead" and is false. The car descends for
            // its whole run there, so it has no unambiguously ground-borne
            // sample, while its wheel bytes carry 88-109 distinct values and
            // imply a 0.35-0.40 m radius on 50 of 125 steps.
            //
            // A wheel that is turning at v/r is evidence of a wheel whatever
            // the chassis is doing, so the only requirement is that the car is
            // MOVING and the wheel angle changed. C5/C6/C7 still assert on the
            // trajectory-derived classes; C8 asks a different question and must
            // not inherit their sample selection.
            if c.cls[i] == Cls::Ballistic {
                continue;
            }
            let mut dt = turns(&v[i]) - turns(&v[i - 1]);
            while dt < -128.0 {
                dt += 256.0;
            }
            while dt > 128.0 {
                dt -= 256.0;
            }
            let mut d = 0.0;
            for k in 0..3 {
                let q = v[i].pos[k] - v[i - 1].pos[k];
                d += q * q;
            }
            let d = d.sqrt();
            // ABSOLUTE value: on 249521 the wheel-turn bytes count DOWN
            // (255 -> 0 -> 255), and a rule that only accepted a positive
            // delta measured noise and reported a 0.0522 m radius -- refusing
            // a downloaded human recording. A wheel rolling backwards is still
            // a wheel.
            let dt = dt.abs();
            if dt > 1e-4 && d > 0.05 {
                rr.push(d / (dt * std::f64::consts::TAU));
            }
        }
        if rr.len() < 5 {
            o.na("C8", format!("only {} usable rolling steps -- cannot infer a wheel radius", rr.len()));
        } else {
            rr.sort_by(|a, b| a.partial_cmp(b).unwrap());
            // NEITHER the median NOR a percentile. Both tails are real driving:
            // WHEELSPIN turns the wheel further than the car travels and reads
            // as a SMALLER radius (median 0.053 m on 249521's own human
            // recording, whose free-rolling steps read 0.36); a LOCKED wheel
            // under braking reads as a LARGER one (p90 0.98 m on 267460's).
            // Tried and rejected in that order, each against all ten downloaded
            // recordings -- the median refused one of them and the p90 refused
            // three.
            //
            // Free rolling is not the average of a run, it is its MODE: the one
            // value the wheel keeps returning to. Bin at 1 cm and take the
            // fullest bin.
            // ... and the mode alone is not enough either: 249521's human
            // recording spins its wheels so persistently that the SPIN is the
            // mode (0.050 m). The question that survives all three failures is
            // not "what is the average implied radius" but "does this file
            // contain a car wheel at all": what SHARE of its rolling steps land
            // in the range a wheel can be. Prior knowledge of the range is not
            // circular here -- the range is not fitted, and a file carrying
            // another run's wheels puts essentially nothing in it.
            let inband: Vec<f64> = rr.iter().cloned().filter(|x| (0.30..=0.45).contains(x)).collect();
            let share = inband.len() as f64 / rr.len() as f64;
            let rad = if inband.is_empty() {
                rr[rr.len() / 2]
            } else {
                let mut b: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
                for x in &inband {
                    *b.entry((x * 200.0).round() as i64).or_insert(0) += 1;
                }
                let k = *b.iter().max_by_key(|(_, v)| **v).unwrap().0;
                let mut c: Vec<f64> = inband
                    .iter()
                    .cloned()
                    .filter(|x| ((x * 200.0).round() as i64) == k)
                    .collect();
                c.sort_by(|a, b| a.partial_cmp(b).unwrap());
                c[c.len() / 2]
            };
            // WETNESS EXEMPTION, measured. 173636 "Tap water 01" is the one map
            // of eleven where the wetness byte varies, and it pins at 255 from
            // 3 s onward. Its own Nadeo rank-2 download puts 329 of 463 rolling
            // steps at an implied radius of 0.10 m -- the wheels turning ~3.6x
            // faster than the road, which is AQUAPLANING, not a small wheel. A
            // share test cannot see the difference between a car spinning its
            // wheels on water and a file carrying a stranger's wheels, so on a
            // soaked run the share is reported and not enforced.
            let soaked = v.iter().filter(|x| x.b(101) > 200).count() * 2 > v.len();
            if soaked && share < 0.15 {
                o.warn("C8", format!(
                    "only {:.0} % of {} rolling steps imply a car wheel (mode {:.4} m) -- but the car is AQUAPLANING (wetness pinned high), which spins the wheels off the road speed, so this is not enforced",
                    100.0 * share, rr.len(), rad
                ));
            } else if share >= 0.15 {
                o.ok("C8", format!(
                    "{:.0} % of {} rolling steps imply a car wheel; the mode is {:.4} m",
                    100.0 * share, rr.len(), rad
                ));
            } else {
                o.bad("C8", format!(
                    "only {:.0} % of {} rolling steps imply a wheel-sized radius (mode {:.4} m) -- these wheel bytes are another run's",
                    100.0 * share, rr.len(), rad
                ));
            }
        }
    }

    // C9 THE INPUT ECHO MATCHES THE MOTION. Bytes 14/15/18 are the tape's own
    // steer/gas/brake. If the record is a stranger's they describe a different
    // drive: a car that is accelerating hard with the throttle byte at zero.
    {
        let mut acc_no_gas = 0usize;
        let mut tested = 0usize;
        for i in 1..v.len().saturating_sub(1) {
            let dt = (v[i + 1].ms - v[i - 1].ms) as f64 / 1000.0;
            if dt <= 0.0 {
                continue;
            }
            if c.cls[i] != Cls::Supported {
                continue;
            }
            let dv = (v[i + 1].speed - v[i - 1].speed) / dt;
            if !dv.is_finite() {
                continue;
            }
            tested += 1;
            if dv > 8.0 && v[i].b(15) == 0 {
                acc_no_gas += 1;
            }
        }
        if tested < 10 {
            o.na("C9", "too few ground-borne samples to test the input echo".into());
        } else if pct(acc_no_gas, tested) < 5.0 {
            o.ok("C9", format!("the throttle echo agrees with the car's acceleration ({} of {} disagree)", acc_no_gas, tested));
        } else {
            o.warn("C9", format!(
                "{:.1} % of ground-borne samples accelerate hard with the throttle byte at zero -- the echo may be another run's",
                pct(acc_no_gas, tested)
            ));
        }
    }

    let code = if o.fail > 0 { 2 } else if o.warn > 0 { 1 } else { 0 };
    (code, o)
}
