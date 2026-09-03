use std::collections::BTreeMap;
use std::convert::TryInto;
use std::env;
use std::fs;

const RECORD_SIZE: usize = 8352;
const CAR_BASE: usize = 64 + 0x60;
const MAGIC: &[u8; 8] = b"HPLTRC3\0";

#[derive(Clone, Copy, Debug)]
struct Sample {
    sim_ms: u32,
    pos: [f64; 3],
    vel: [f64; 3],
    gas: f64,
    brake: f64,
    steer: f64,
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn f32_at(bytes: &[u8], offset: usize) -> f64 {
    f32::from_bits(u32_at(bytes, offset)) as f64
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}
fn magnitude(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn load(path: &str, position_offset: usize, use_last: bool) -> BTreeMap<u32, Sample> {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(bytes.len() % RECORD_SIZE, 0, "{path}: partial record");
    let mut out = BTreeMap::new();
    for (index, record) in bytes.chunks_exact(RECORD_SIZE).enumerate() {
        assert_eq!(&record[..8], MAGIC, "{path}: bad magic at {index}");
        let sim_ms = u32_at(record, 56);
        let car = &record[CAR_BASE..];
        let sample = Sample {
            sim_ms,
            pos: [
                f32_at(car, position_offset),
                f32_at(car, position_offset + 4),
                f32_at(car, position_offset + 8),
            ],
            vel: [
                f32_at(car, position_offset + 12),
                f32_at(car, position_offset + 16),
                f32_at(car, position_offset + 20),
            ],
            gas: f32_at(car, 0x98),
            brake: f32_at(car, 0x9c),
            steer: f32_at(car, 0xa0),
        };
        if use_last {
            out.insert(sim_ms, sample);
        } else {
            out.entry(sim_ms).or_insert(sample);
        }
    }
    out
}

fn movement_t0(samples: &BTreeMap<u32, Sample>) -> u32 {
    let first = samples.values().next().expect("empty trace").pos;
    samples
        .values()
        .find(|sample| distance(sample.pos, first) > 0.05)
        .map(|sample| sample.sim_ms)
        .unwrap_or_else(|| samples.keys().next().copied().unwrap())
}

fn first_reset_after(samples: &BTreeMap<u32, Sample>, t0: u32) -> Option<u32> {
    let start = samples.get(&t0)?.pos;
    let mut reached = false;
    let mut previous = start;
    for sample in samples.range(t0..).map(|(_, sample)| sample) {
        if distance(sample.pos, start) > 25.0 {
            reached = true;
        }
        if reached && (distance(sample.pos, start) < 2.0 || sample.pos[2] + 100.0 < previous[2]) {
            return Some(sample.sim_ms);
        }
        previous = sample.pos;
    }
    None
}

fn describe(label: &str, samples: &BTreeMap<u32, Sample>) {
    let t0 = movement_t0(samples);
    let reset = first_reset_after(samples, t0);
    let gas_on = samples.values().find(|s| s.gas > 0.5).map(|s| s.sim_ms);
    let gas_last = samples.values().rev().find(|s| s.gas > 0.5).map(|s| s.sim_ms);
    let end = reset.unwrap_or_else(|| *samples.keys().next_back().unwrap());
    let mut max_speed = 0.0f64;
    let mut max_abs_steer = 0.0f64;
    let mut max_brake = 0.0f64;
    let mut max_z = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    for sample in samples.range(t0..=end).map(|(_, sample)| sample) {
        max_speed = max_speed.max(magnitude(sample.vel));
        max_abs_steer = max_abs_steer.max(sample.steer.abs());
        max_brake = max_brake.max(sample.brake);
        max_z = max_z.max(sample.pos[2]);
        min_y = min_y.min(sample.pos[1]);
    }
    println!("RUN {label} t0_ms={t0} reset_ms={reset:?} segment_ms={} gas_on_ms={gas_on:?} gas_last_ms={gas_last:?} max_speed_kmh={:.6} max_abs_steer={:.6} max_brake={:.6} max_z_m={max_z:.6} min_y_m={min_y:.6}", end - t0, max_speed * 3.6, max_abs_steer, max_brake);
    for rel in (0..=22_000u32).step_by(1_000) {
        if let Some(s) = samples.get(&(t0 + rel)) {
            println!("SAMPLE {label} rel_s={:.3} pos={:.6},{:.6},{:.6} vel={:.6},{:.6},{:.6} speed_kmh={:.6} gas={:.3}", rel as f64 / 1000.0, s.pos[0], s.pos[1], s.pos[2], s.vel[0], s.vel[1], s.vel[2], magnitude(s.vel) * 3.6, s.gas);
        }
    }
}

fn compare(a_label: &str, a: &BTreeMap<u32, Sample>, b_label: &str, b: &BTreeMap<u32, Sample>) {
    let a0 = movement_t0(a);
    let b0 = movement_t0(b);
    let a_end = first_reset_after(a, a0).unwrap_or_else(|| *a.keys().next_back().unwrap());
    let b_end = first_reset_after(b, b0).unwrap_or_else(|| *b.keys().next_back().unwrap());
    let duration = (a_end - a0).min(b_end - b0);
    let mut count = 0usize;
    let mut sum_pos = 0.0f64;
    let mut max_pos = 0.0f64;
    let mut sum_vel = 0.0f64;
    let mut max_vel = 0.0f64;
    let mut max_axis = [0.0f64; 3];
    let mut first_1mm = None;
    let mut first_1cm = None;
    let mut first_10cm = None;
    let mut first_1m = None;
    for rel in (0..=duration).step_by(10) {
        let (Some(sa), Some(sb)) = (a.get(&(a0 + rel)), b.get(&(b0 + rel))) else { continue; };
        let pd = distance(sa.pos, sb.pos);
        let vd = distance(sa.vel, sb.vel);
        count += 1;
        sum_pos += pd;
        max_pos = max_pos.max(pd);
        sum_vel += vd;
        max_vel = max_vel.max(vd);
        for axis in 0..3 {
            max_axis[axis] = max_axis[axis].max((sa.pos[axis] - sb.pos[axis]).abs());
        }
        if first_1mm.is_none() && pd > 0.001 { first_1mm = Some(rel); }
        if first_1cm.is_none() && pd > 0.01 { first_1cm = Some(rel); }
        if first_10cm.is_none() && pd > 0.1 { first_10cm = Some(rel); }
        if first_1m.is_none() && pd > 1.0 { first_1m = Some(rel); }
    }
    println!("COMPARE {a_label} vs {b_label} a_t0_ms={a0} b_t0_ms={b0} duration_ms={duration} ticks={count} mean_pos_m={:.9} max_pos_m={max_pos:.9} mean_vel_mps={:.9} max_vel_mps={max_vel:.9} max_abs_xyz_m={max_axis:?} first_gt_1mm_ms={first_1mm:?} first_gt_1cm_ms={first_1cm:?} first_gt_10cm_ms={first_10cm:?} first_gt_1m_ms={first_1m:?}", sum_pos / count.max(1) as f64, sum_vel / count.max(1) as f64);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 7, "usage: compare-three exact.trace exact_pos_off stock.trace stock_pos_off v5.trace v5_pos_off");
    let parse_off = |value: &str| usize::from_str_radix(value.trim_start_matches("0x"), 16).unwrap();
    for policy in [false, true] {
        println!("POLICY {}", if policy { "last" } else { "first" });
        let exact = load(&args[1], parse_off(&args[2]), policy);
        let stock = load(&args[3], parse_off(&args[4]), policy);
        let v5 = load(&args[5], parse_off(&args[6]), policy);
        describe("exact", &exact);
        describe("stock", &stock);
        describe("v5", &v5);
        compare("exact", &exact, "stock", &stock);
        compare("exact", &exact, "v5", &v5);
        compare("stock", &stock, "v5", &v5);
    }
}
