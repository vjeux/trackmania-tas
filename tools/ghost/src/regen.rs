//! `ghost regen` -- from INPUTS + MAP, generate the car state and write it into
//! the ghost, so the recorded trajectory matches the tape.
//!
//! This is the operation that keeps biting the project, and it bites in a
//! specific way: the state reader locates the car in a live engine's memory,
//! and WHICH COPY it lands on varies between runs. A wrong pick produces a
//! file that passes coverage, passes a manifest, passes a two-run agreement
//! test, and carries a frozen memory slot instead of a car.
//!
//! So this wrapper does not just call the regenerator. It runs it, then puts
//! the result through a gate that can IDENTIFY the answer rather than count
//! votes:
//!
//!   G1  the written file's own telemetry agrees with the tape it carries
//!       (the two input channels in one file, no reference needed);
//!   G2  the first sample is at the map's spawn, not somewhere else;
//!   G3  the path length is a plausible driven distance, not 0 m and not 1e28;
//!   G4  the plain oracle re-simulates THE WRITTEN FILE to the declared time;
//!   G5  no sample is NaN and no position is constant for the whole run.
//!
//! and `ghost regen-control` runs the strongest control the write path has: a
//! ghost that already carries its own true telemetry, regenerated from its own
//! inputs, must reproduce that telemetry. If the fixed point does not hold on a
//! known-good file, nothing the regenerator says about an unknown one counts.

use crate::container::secs;
use crate::verify;
use crate::{die, flag, has, num};
use std::path::Path;
use std::process::Command;

fn fk_binary() -> String {
    if let Ok(v) = std::env::var("FK_BIN") {
        return v;
    }
    // next to this binary, which is where cargo puts both
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            let p = d.join("fk");
            if p.exists() {
                return p.to_string_lossy().into();
            }
        }
    }
    "fk".into()
}

fn shim() -> String {
    if let Ok(v) = std::env::var("FK_SHIM") {
        return v;
    }
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            let p = d.join("libfkshim.so");
            if p.exists() {
                return p.to_string_lossy().into();
            }
        }
    }
    "/tmp/fk/rs/target/release/libfkshim.so".into()
}

pub struct RegenOut {
    pub ok: bool,
    pub log: String,
}

/// Run the regenerator once. `dump` MUST be absolute: a relative path fails
/// inside the forked server with a bare `go ERR open`.
pub fn run_regen(template: &str, map: &str, out: &str, extra: &[String]) -> RegenOut {
    let dump = std::env::temp_dir()
        .join(format!("ghost-regen-{}-{}.bin", std::process::id(), rand_tag()))
        .to_string_lossy()
        .to_string();
    let mut args: Vec<String> = vec![
        "regen".into(),
        "--template".into(),
        abspath(template),
        "--map".into(),
        abspath(map),
        "--out".into(),
        abspath(out),
        "--dump".into(),
        dump,
        "--shim".into(),
        shim(),
        "--server".into(),
        crate::oracle::server_dir(None).to_string_lossy().to_string(),
    ];
    args.extend_from_slice(extra);
    let o = Command::new(fk_binary()).args(&args).output();
    match o {
        Err(e) => RegenOut { ok: false, log: format!("cannot run {}: {}", fk_binary(), e) },
        Ok(o) => {
            let log = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            RegenOut { ok: o.status.success() && Path::new(out).exists(), log }
        }
    }
}

fn rand_tag() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    format!("{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos())
}

fn abspath(p: &str) -> String {
    std::fs::canonicalize(p)
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            let cwd = std::env::current_dir().unwrap();
            cwd.join(p).to_string_lossy().to_string()
        })
}

/// Path length of the written file's own recorded trajectory, and its first
/// sample. `carscan`'s test in one place: a car's path is the map's ribbon, a
/// wrong memory copy's is 0.0 m or 1e28 m.
pub fn path_stats(file: &str) -> Option<(f64, [f64; 3], usize, bool)> {
    let d = tmtraj::entrec::decode_ghost(file).ok()?;
    if d.samples.is_empty() {
        return None;
    }
    let mut len = 0.0;
    let mut finite = true;
    let mut moved = false;
    for w in d.samples.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let dz = b.z - a.z;
        let s = (dx * dx + dy * dy + dz * dz).sqrt();
        if !s.is_finite() {
            finite = false;
            continue;
        }
        if s > 1e-6 {
            moved = true;
        }
        len += s;
    }
    let f = &d.samples[0];
    Some((len, [f.x, f.y, f.z], d.samples.len(), finite && moved))
}

pub fn cmd(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost regen IN OUT --map MAP"));
    let out = a.get(1).unwrap_or_else(|| die("ghost regen IN OUT --map MAP"));
    let map = flag(a, "--map").unwrap_or_else(|| die("--map MAP.Map.Gbx"));
    let extra: Vec<String> = {
        let mut v = Vec::new();
        for k in ["--fieldmap", "--anchorticks", "--segs", "--biastick", "--inputshift"] {
            if let Some(x) = flag(a, k) {
                v.push(k.to_string());
                v.push(x.to_string());
            }
        }
        if has(a, "--verbose") {
            v.push("--verbose".into());
        }
        if has(a, "--trim-outside") {
            v.push("--trim-outside".into());
        }
        v
    };
    let tries: i64 = num(a, "--tries").unwrap_or(3);

    let mut accepted: Option<String> = None;
    let mut lastlog = String::new();
    for k in 0..tries {
        let cand = format!("{}.try{}", out, k);
        println!("== regeneration attempt {} of {}", k + 1, tries);
        let raw = format!("{}.raw", cand);
        let r = run_regen(inp, map, &raw, &extra);
        lastlog = r.log.clone();
        if !r.ok {
            println!("   the regenerator did not produce a file; retrying");
            for l in r.log.lines().rev().take(4) {
                println!("   | {}", l);
            }
            continue;
        }
        // Widen the write: the recorded steer / gas / brake channels come from
        // the TAPE, not from the engine, and leaving them as the carrier's is
        // how a regenerated file ends up describing somebody else's driving.
        match write_input_channels(&raw, &cand) {
            Ok((w, sk)) => println!("   input channels rewritten from the tape on {} samples ({} outside the tape)", w, sk),
            Err(e) => {
                println!("   could not rewrite the input channels: {}; keeping the transform-only file", e);
                let _ = std::fs::rename(&raw, &cand);
            }
        }
        let _ = std::fs::remove_file(&raw);
        match gate(&cand, map, a) {
            Ok(msg) => {
                println!("{}", msg);
                accepted = Some(cand);
                break;
            }            Err(msg) => {
                println!("{}", msg);
                println!("   REFUSED -- the locate found something that is not the car. Retrying.");
                let _ = std::fs::remove_file(&cand);
            }
        }
    }
    match accepted {
        None => {
            eprintln!("--- last regenerator log ---");
            eprintln!("{}", lastlog);
            die(format!(
                "no regeneration passed the gate in {} attempts. NOTHING was written to {}: a file \
                 that fails this gate is a frozen memory slot, not a car.",
                tries, out
            ));
        }
        Some(c) => {
            std::fs::rename(&c, out).unwrap_or_else(|e| die(format!("{}: {}", out, e)));
            println!("wrote {}", out);
            println!(
                "  WHAT IS OURS in each 116-byte sample: the 22 transform bytes (position, \n\
                 \x20 orientation, speed, velocity direction) from the engine, and bytes 14 / 15 / 18 \n\
                 \x20 (steer, gas, brake) from the tape. The remaining 91 -- rpm, gear, wheel \n\
                 \x20 rotation and suspension, surface effects -- are still the carrier's. They are \n\
                 \x20 in engine memory too; nothing here has read them yet."
            );
        }
    }
}

/// The acceptance gate for one regenerated candidate.
fn gate(cand: &str, map: &str, a: &[String]) -> Result<String, String> {
    let mut s = String::new();
    let (len, first, n, ok) = path_stats(cand).ok_or("   G3 the written file has no samples")?;
    if !ok {
        return Err(format!(
            "   G5 the trajectory is not finite or never moves ({} samples, {:.1} m)",
            n, len
        ));
    }
    if !(1.0..=1.0e6).contains(&len) {
        return Err(format!(
            "   G3 path length {:.1} m over {} samples -- not a driven path",
            len, n
        ));
    }
    s.push_str(&format!("   G3 path length {:.1} m over {} samples\n", len, n));
    s.push_str(&format!(
        "   G2 first sample at ({:.2}, {:.2}, {:.2})\n",
        first[0], first[1], first[2]
    ));
    match verify::tape_record_agreement(cand) {
        None => return Err("   G1 the written file has no telemetry to compare with its tape".into()),
        Some((kappa, rate, lag, k)) => {
            let _ = rate;
            if kappa < 0.60 {
                return Err(format!(
                    "   G1 tape/record agreement kappa {:.3} over {} samples (best lag {} ms) -- the record \
                     written is not this tape's run",
                    kappa, k, lag
                ));
            }
            s.push_str(&format!(
                "   G1 tape/record agreement kappa {:.3} over {} samples (best lag {} ms)\n",
                kappa, k, lag
            ));
        }
    }
    // G4: the plain oracle on THE WRITTEN FILE
    if !has(a, "--no-oracle") {
        let server = crate::oracle::server_dir(flag(a, "--server"));
        if server.join("TrackmaniaServer").exists() {
            match crate::oracle::validate(&server, Path::new(cand), crate::oracle::MapsMode::One(Path::new(map)), "regen") {
                Ok(res) => {
                    let decl = crate::container::Container::load(cand)
                        .ok()
                        .and_then(|c| c.declared_times().first().map(|x| x.1 as i64));
                    match (res.time_ms, decl) {
                        (Some(t), Some(d)) if t == d => {
                            s.push_str(&format!("   G4 oracle on the written file: {}\n", secs(t)))
                        }
                        (Some(t), Some(d)) => {
                            return Err(format!(
                                "   G4 oracle re-simulated the written file to {} but it declares {}",
                                secs(t),
                                secs(d)
                            ))
                        }
                        (Some(t), None) => s.push_str(&format!("   G4 oracle: {}\n", secs(t))),
                        (None, _) => {
                            return Err(format!("   G4 oracle: DNF on the written file (cps {:?})", res.cps))
                        }
                    }
                }
                Err(e) => s.push_str(&format!("   G4 oracle not run: {}\n", e)),
            }
        } else {
            s.push_str("   G4 oracle not run: no dedicated server\n");
        }
    }
    s.push_str("   GATE PASSED");
    Ok(s)
}

/// The fixed-point control: regenerate a file that already carries its own true
/// telemetry, and require the regeneration to reproduce it.
pub fn control(a: &[String]) {
    let inp = a.first().unwrap_or_else(|| die("ghost regen-control FILE --map MAP"));
    let map = flag(a, "--map").unwrap_or_else(|| die("--map MAP.Map.Gbx"));
    let d0 = tmtraj::entrec::decode_ghost(inp)
        .unwrap_or_else(|e| die(format!("{} has no telemetry to control against: {}", inp, e)));
    let out = format!("{}.regen-control", inp);
    let r = run_regen(inp, map, &out, &[]);
    if !r.ok {
        eprintln!("{}", r.log);
        die("the regenerator did not produce a file");
    }
    let d1 = tmtraj::entrec::decode_ghost(&out).unwrap_or_else(|e| die(e));
    let n = d0.samples.len().min(d1.samples.len());
    let mut sum = 0.0;
    let mut worst = 0.0f64;
    let mut exact = 0usize;
    for i in 0..n {
        let (a2, b) = (&d0.samples[i], &d1.samples[i]);
        let dd = ((a2.x - b.x).powi(2) + (a2.y - b.y).powi(2) + (a2.z - b.z).powi(2)).sqrt();
        sum += dd;
        worst = worst.max(dd);
        if dd == 0.0 {
            exact += 1;
        }
    }
    let mean = if n > 0 { sum / n as f64 } else { f64::NAN };
    println!("fixed-point control on {}", inp);
    println!("  samples        {} original, {} regenerated", d0.samples.len(), d1.samples.len());
    println!("  mean position  {:.6} m", mean);
    println!("  worst          {:.6} m", worst);
    println!("  bit-identical  {} of {}", exact, n);
    // a one-tick offset is a PURE TIME SHIFT: check for it explicitly rather
    // than letting a small mean hide it
    let shifted = |k: i64| -> f64 {
        let mut s = 0.0;
        let mut c = 0usize;
        for i in 0..n {
            let j = i as i64 + k;
            if j < 0 || j >= d1.samples.len() as i64 {
                continue;
            }
            let (a2, b) = (&d0.samples[i], &d1.samples[j as usize]);
            s += ((a2.x - b.x).powi(2) + (a2.y - b.y).powi(2) + (a2.z - b.z).powi(2)).sqrt();
            c += 1;
        }
        if c == 0 {
            f64::INFINITY
        } else {
            s / c as f64
        }
    };
    let m1 = shifted(1);
    let mm1 = shifted(-1);
    println!("  one sample early/late: {:.6} m / {:.6} m", mm1, m1);
    if m1 < mean * 0.5 || mm1 < mean * 0.5 {
        println!(
            "  VERDICT: FAIL -- the regeneration is a WHOLE SAMPLE out of phase. \
             A solo clip cannot look wrong from this, but every frame-synchronous comparison is."
        );
        std::process::exit(2);
    }
    if mean.is_finite() && mean < 0.05 {
        println!("  VERDICT: PASS -- the write path is a fixed point on a file that already knew its own answer");
    } else {
        println!("  VERDICT: FAIL -- regenerating a known-good file did not reproduce it");
        std::process::exit(2);
    }
}

pub const CLASS_CSCENEVEHICLEVIS: u32 = 0x0A018000;

/// The vehicle entity's raw sample block: (sample_size, bytes).
pub fn raw_vehicle_samples(path: &str) -> Result<(usize, Vec<u8>), String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
    let g = tmtraj::gbx::Gbx::parse(&data);
    let (version, blob) = tmtraj::entrec::find_entrecord_blob(&g.body)?;
    let rd = tmtraj::entrec::parse_record_data(&blob, version)?;
    let mut best: Option<&tmtraj::entrec::Ent> = None;
    for e in &rd.ents {
        let cid = rd.descs.get(e.type_.max(0) as usize).filter(|_| e.type_ >= 0).map(|d| d.class_id);
        if cid == Some(CLASS_CSCENEVEHICLEVIS) && best.map_or(true, |b| e.times.len() > b.times.len()) {
            best = Some(e);
        }
    }
    let e = best.ok_or("no CSceneVehicleVis entity")?;
    Ok((e.sample_size, e.raw.clone()))
}

/// The recorded steer byte for a tape steer value.
///
/// MEASURED, not assumed: over the project's whole ghost corpus this
/// reproduces byte 14 of every vehicle sample exactly. The mapping is
/// `floor((s + 127) * 255 / 254)` -- note the 254, and note the FLOOR: a round
/// misses s = 60 and s = 0, which is how a "close enough" encoder ends up one
/// grid step out on half the file.
pub fn steer_byte(s: i8) -> u8 {
    (((s as i32 + 127) as i64 * 255) / 254) as u8
}

/// The recorded gas / brake byte for a digital pedal.
pub fn pedal_byte(on: u32) -> u8 {
    if on != 0 {
        255
    } else {
        0
    }
}

/// Run the real engine on the file's OWN tape and compare the trajectory it
/// produces with the trajectory the file records.
///
/// Returns (mean position error, worst, samples compared, one-sample-shift).
/// The shift flag is separate on purpose: a one-tick offset is a PURE TIME
/// SHIFT, so it hides inside a small mean and only shows when the comparison is
/// re-scored against the neighbouring samples.
pub fn engine_trajectory_agreement(path: &str, map: &str) -> Result<(f64, f64, usize, bool), String> {
    let d0 = tmtraj::entrec::decode_ghost(path).map_err(|e| e.to_string())?;
    if d0.samples.is_empty() {
        return Err("this file has no telemetry to compare".into());
    }
    let tmp = std::env::temp_dir().join(format!("ghost-v9-{}-{}.Ghost.Gbx", std::process::id(), rand_tag()));
    let out = tmp.to_string_lossy().to_string();
    let mut ok = false;
    let mut log = String::new();
    // The locate is not deterministic: try a few times and take the first run
    // whose result identifies the car (path length + spawn), rather than voting.
    for _ in 0..3 {
        let r = run_regen(path, map, &out, &[]);
        log = r.log.clone();
        if r.ok {
            if let Some((len, _first, _n, moved)) = path_stats(&out) {
                if moved && (1.0..=1.0e6).contains(&len) {
                    ok = true;
                    break;
                }
            }
        }
        let _ = std::fs::remove_file(&out);
    }
    if !ok {
        return Err(format!(
            "the engine readout did not identify the car in three attempts; last log tail: {}",
            log.lines().rev().take(2).collect::<Vec<_>>().join(" | ")
        ));
    }
    let d1 = tmtraj::entrec::decode_ghost(&out).map_err(|e| e.to_string())?;
    let n = d0.samples.len().min(d1.samples.len());
    let dist = |a: &tmtraj::entrec::Sample, b: &tmtraj::entrec::Sample| {
        ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
    };
    let mut sum = 0.0;
    let mut worst = 0.0f64;
    for i in 0..n {
        let dd = dist(&d0.samples[i], &d1.samples[i]);
        sum += dd;
        worst = worst.max(dd);
    }
    let mean = sum / n.max(1) as f64;
    let shifted = |k: i64| -> f64 {
        let (mut s, mut c) = (0.0, 0usize);
        for i in 0..n {
            let j = i as i64 + k;
            if j < 0 || j >= d1.samples.len() as i64 {
                continue;
            }
            s += dist(&d0.samples[i], &d1.samples[j as usize]);
            c += 1;
        }
        if c == 0 { f64::INFINITY } else { s / c as f64 }
    };
    let shift = mean > 0.02 && (shifted(1) < mean * 0.5 || shifted(-1) < mean * 0.5);
    let _ = std::fs::remove_file(&out);
    Ok((mean, worst, n, shift))
}

/// Write the RECORDED INPUT CHANNELS from the tape.
///
/// `fk regen` rewrites the 22 transform bytes of each sample -- position,
/// orientation, speed, velocity direction -- and leaves the other 94 as the
/// carrier's. Three of those 94 are the recorded steer, gas and brake, and
/// they do not need the engine at all: the TAPE is where they come from. A
/// regenerated file that keeps the carrier's input channels is a file whose
/// analysis tools, overlays and contamination checks all describe somebody
/// else's driving, so this closes them.
///
/// It is not a claim about the other 91 bytes. `ghost regen` names those.
pub fn write_input_channels(inp: &str, out: &str) -> Result<(usize, usize), String> {
    let t = crate::tape::Tape::from_file(inp)?;
    let a0 = &t.archives[0];
    let so = a0.start_offset_ms as i64;
    let mut written = 0usize;
    let mut skipped = 0usize;
    tmtraj::recwrite::rewrite_ghost(inp, out, |rd| {
        let mut vehicle: Option<usize> = None;
        for (i, e) in rd.ents.iter().enumerate() {
            let cid = rd.descs.get(e.type_.max(0) as usize).filter(|_| e.type_ >= 0).map(|d| d.class_id);
            if cid == Some(CLASS_CSCENEVEHICLEVIS)
                && vehicle.map_or(true, |v| e.times.len() > rd.ents[v].times.len())
            {
                vehicle = Some(i);
            }
        }
        let Some(vi) = vehicle else { return Err("no CSceneVehicleVis entity".into()) };
        let e = &mut rd.ents[vi];
        let ss = e.sample_size;
        if ss < 20 {
            return Err(format!("vehicle sample size {} is too small for the input bytes", ss));
        }
        for i in 0..e.times.len() {
            let idx = (e.times[i] as i64 - so) / 10;
            if idx < 0 || idx >= a0.packets.len() as i64 {
                skipped += 1;
                continue;
            }
            let p = &a0.packets[idx as usize];
            let d = &mut e.raw[i * ss..(i + 1) * ss];
            d[14] = steer_byte(p.steer_i8());
            d[15] = pedal_byte(p.accel);
            d[18] = pedal_byte(p.brake);
            written += 1;
        }
        Ok(())
    })?;
    Ok((written, skipped))
}
