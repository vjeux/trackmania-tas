//! PRIVATE stand-in for `tmdata`'s shard reader/writer — the `.tmd` format of INTERFACES.md §Dataset.
//!
//! **To be replaced by the DATA arm's `tmdata` crate the day it lands**; the byte layout below is the proposal
//! recorded in INTERFACES.md so the two agree. Everything little-endian.
//!
//! ```text
//! header  magic b"TMD0" | STATE_VERSION u32 | count u64                                   16 bytes
//! record  map_uid [u8;32] zero-padded | ghost_id u32 | tick u32 | CarState 100 B | Action 3 B + 1 pad | weight f32
//!                                                                                          148 bytes
//! ```
//! `CarState` is written field by field in its `repr(C)` order with the C padding zeroed (3 bytes after `gear`,
//! 2 after `finished`), so the 100 bytes are exactly the in-memory layout — a reader may `transmute`, this one
//! does not.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use tmstate::{Action, CarState, STATE_VERSION};

pub const MAGIC: &[u8; 4] = b"TMD0";
/// Record and state sizes per STATE_VERSION: v1/v2 carry the 100-byte CarState (148-byte records, INTERFACES.md), v3
/// the 120-byte one (168-byte records: + effects/reactor/boost bytes at 99..104, reactor_air 104..116, sim_time_coef 116..120).
pub const RECORD_SIZE: usize = 168;
pub const STATE_SIZE: usize = 120;
pub const RECORD_SIZE_V2: usize = 148;
pub const STATE_SIZE_V2: usize = 100;
pub fn sizes_for(ver: u32) -> (usize, usize) {
    if ver >= 3 { (RECORD_SIZE, STATE_SIZE) } else { (RECORD_SIZE_V2, STATE_SIZE_V2) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub map_uid: String,
    pub ghost_id: u32,
    pub tick: u32,
    pub state: CarState,
    pub action: Action,
    pub weight: f32,
}

fn put_f32s(out: &mut Vec<u8>, xs: &[f32]) {
    for x in xs {
        out.extend_from_slice(&x.to_le_bytes());
    }
}

pub fn encode_state(s: &CarState, out: &mut Vec<u8>) {
    let start = out.len();
    out.extend_from_slice(&s.race_ms.to_le_bytes());
    put_f32s(out, &s.pos);
    put_f32s(out, &s.vel);
    put_f32s(out, &s.quat);
    put_f32s(out, &s.ang_vel);
    put_f32s(out, &[s.speed]);
    out.push(s.gear);
    out.extend_from_slice(&[0, 0, 0]);
    put_f32s(out, &[s.rpm]);
    out.extend_from_slice(&s.wheel_contact);
    out.extend_from_slice(&s.wheel_material);
    put_f32s(out, &s.wheel_slip);
    put_f32s(out, &[s.turbo]);
    out.push(s.cps);
    out.push(s.finished as u8);
    out.push(s.car); // byte 98 (v2; v1 wrote 0 = Stadium, the only car v1 data has)
    out.push(s.effects); // byte 99 (v3; v2 wrote 0 = unknown)
    out.push(s.reactor_lvl);
    out.push(s.reactor_type);
    out.push(s.boost_enum);
    out.push(0);
    put_f32s(out, &s.reactor_air);
    put_f32s(out, &[s.sim_time_coef]);
    debug_assert_eq!(out.len() - start, STATE_SIZE);
}

struct Rd<'a>(&'a [u8], usize);
impl<'a> Rd<'a> {
    fn f32(&mut self) -> f32 {
        let v = f32::from_le_bytes(self.0[self.1..self.1 + 4].try_into().unwrap());
        self.1 += 4;
        v
    }
    fn f3(&mut self) -> [f32; 3] {
        [self.f32(), self.f32(), self.f32()]
    }
    fn f4(&mut self) -> [f32; 4] {
        [self.f32(), self.f32(), self.f32(), self.f32()]
    }
    fn u8(&mut self) -> u8 {
        let v = self.0[self.1];
        self.1 += 1;
        v
    }
    fn u8x4(&mut self) -> [u8; 4] {
        [self.u8(), self.u8(), self.u8(), self.u8()]
    }
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.0[self.1..self.1 + 4].try_into().unwrap());
        self.1 += 4;
        v
    }
}

pub fn decode_state(b: &[u8]) -> CarState {
    assert!(b.len() >= STATE_SIZE_V2);
    let mut r = Rd(b, 0);
    let race_ms = r.u32() as i32;
    let pos = r.f3();
    let vel = r.f3();
    let quat = r.f4();
    let ang_vel = r.f3();
    let speed = r.f32();
    let gear = r.u8();
    r.1 += 3;
    let rpm = r.f32();
    let wheel_contact = r.u8x4();
    let wheel_material = r.u8x4();
    let wheel_slip = r.f4();
    let turbo = r.f32();
    let cps = r.u8();
    let finished = r.u8() != 0;
    let car = r.u8();
    let mut s = CarState { race_ms, pos, vel, quat, ang_vel, speed, gear, rpm, wheel_contact, wheel_material, wheel_slip, turbo, cps, finished, car, ..CarState::unknown() };
    if b.len() >= STATE_SIZE {
        s.effects = r.u8();
        s.reactor_lvl = r.u8();
        s.reactor_type = r.u8();
        s.boost_enum = r.u8();
        r.1 += 1;
        s.reactor_air = r.f3();
        s.sim_time_coef = r.f32();
    }
    s
}

pub fn encode_record(rec: &Record, out: &mut Vec<u8>) {
    let start = out.len();
    let mut uid = [0u8; 32];
    let b = rec.map_uid.as_bytes();
    assert!(b.len() <= 32, "map uid longer than 32 bytes: {}", rec.map_uid);
    uid[..b.len()].copy_from_slice(b);
    out.extend_from_slice(&uid);
    out.extend_from_slice(&rec.ghost_id.to_le_bytes());
    out.extend_from_slice(&rec.tick.to_le_bytes());
    encode_state(&rec.state, out);
    out.push(rec.action.steer as u8);
    out.push(rec.action.gas as u8);
    out.push(rec.action.brake as u8);
    out.push(0);
    out.extend_from_slice(&rec.weight.to_le_bytes());
    debug_assert_eq!(out.len() - start, RECORD_SIZE);
}

pub fn decode_record(b: &[u8]) -> Record {
    // Accepts a v2 (148 B) or v3 (168 B) record slice; the state size follows from the slice length.
    assert!(b.len() == RECORD_SIZE || b.len() == RECORD_SIZE_V2);
    let ss = b.len() - 48;
    let n = b[..32].iter().position(|&c| c == 0).unwrap_or(32);
    let map_uid = String::from_utf8_lossy(&b[..n]).into_owned();
    let ghost_id = u32::from_le_bytes(b[32..36].try_into().unwrap());
    let tick = u32::from_le_bytes(b[36..40].try_into().unwrap());
    let state = decode_state(&b[40..40 + ss]);
    let a0 = 40 + ss;
    let action = Action { steer: b[a0] as i8, gas: b[a0 + 1] != 0, brake: b[a0 + 2] != 0 };
    let weight = f32::from_le_bytes(b[a0 + 4..a0 + 8].try_into().unwrap());
    Record { map_uid, ghost_id, tick, state, action, weight }
}

pub fn write_shard(path: &str, recs: &[Record]) -> Result<(), String> {
    let f = File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut w = BufWriter::new(f);
    let mut hdr = Vec::with_capacity(16);
    hdr.extend_from_slice(MAGIC);
    hdr.extend_from_slice(&STATE_VERSION.to_le_bytes());
    hdr.extend_from_slice(&(recs.len() as u64).to_le_bytes());
    w.write_all(&hdr).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(RECORD_SIZE * 1024);
    for (i, r) in recs.iter().enumerate() {
        encode_record(r, &mut buf);
        if buf.len() >= RECORD_SIZE * 1024 || i + 1 == recs.len() {
            w.write_all(&buf).map_err(|e| e.to_string())?;
            buf.clear();
        }
    }
    w.flush().map_err(|e| e.to_string())
}

pub fn read_shard(path: &str) -> Result<Vec<Record>, String> {
    let f = File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut r = BufReader::new(f);
    let mut hdr = [0u8; 16];
    r.read_exact(&mut hdr).map_err(|e| format!("{path}: header: {e}"))?;
    if &hdr[..4] != MAGIC {
        return Err(format!("{path}: bad magic {:?}", &hdr[..4]));
    }
    let ver = u32::from_le_bytes(hdr[4..8].try_into().unwrap());
    // v1 shards read fine under v2: the layout is unchanged (v2 put `car` into a v1 padding byte, which v1 wrote as 0
    // = Stadium — the only car v1 data has).
    if !(1..=STATE_VERSION).contains(&ver) {
        return Err(format!("{path}: STATE_VERSION {ver}, this reader is {STATE_VERSION} (and reads 1..={STATE_VERSION})"));
    }
    let (rec_size, _) = sizes_for(ver);
    let count = u64::from_le_bytes(hdr[8..16].try_into().unwrap()) as usize;
    let mut body = Vec::with_capacity(count * rec_size);
    r.read_to_end(&mut body).map_err(|e| e.to_string())?;
    if body.len() != count * rec_size {
        // A shard seen through a stale / mid-sync ~/persistent view can be shorter than its header says (seen
        // 2026-09-06: header 1,001,681 records, body 1,000,623). Read the whole records that ARE there and say so;
        // a LONGER body than the header is a different fault and is refused.
        if body.len() > count * rec_size || body.len() % rec_size != 0 {
            return Err(format!("{path}: header says {count} records ({} B), body is {} B", count * rec_size, body.len()));
        }
        eprintln!("WARNING {path}: header says {count} records, body holds {} — reading the body (stale or partial copy?)", body.len() / rec_size);
    }
    Ok(body.chunks_exact(rec_size).map(decode_record).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_exact_including_nans() {
        let mut st = CarState::unknown();
        st.race_ms = -1550;
        st.pos = [1584.0, 18.002, 784.0];
        st.quat = [1.0, 0.0, 0.0, 0.0];
        st.gear = 3;
        st.cps = 2;
        st.finished = true;
        let rec = Record {
            map_uid: "buNzfsVlp2NF2oWtHM3729dEylg".into(),
            ghost_id: 7,
            tick: 12345,
            state: st,
            action: Action { steer: -127, gas: true, brake: false },
            weight: 0.75,
        };
        let mut b = Vec::new();
        encode_record(&rec, &mut b);
        assert_eq!(b.len(), RECORD_SIZE);
        let back = decode_record(&b);
        // PartialEq on NaN fails, so compare the bit patterns.
        let mut b2 = Vec::new();
        encode_record(&back, &mut b2);
        assert_eq!(b, b2);
        assert_eq!(back.map_uid, rec.map_uid);
        assert_eq!(back.action, rec.action);
        assert_eq!(back.state.race_ms, -1550);
        assert!(back.state.rpm.is_nan());
        assert_eq!(std::mem::size_of::<CarState>(), STATE_SIZE);
    }
}
