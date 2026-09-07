//! `TMR0` — the reachability dataset (INTERFACES §2), and the private copy of
//! the player's `CarState` (INTERFACES §1: identical to `tmstate`'s text until
//! that crate lands, then deleted).
//!
//! Record layout (little-endian, fixed): `start_id u32, macro_id u16,
//! horizon_ticks u16, outcome u8, [pad 3], end: CarState (100 bytes, the
//! `repr(C)` layout below), gate_tick: [i16; 32], path_len_m f32, min_speed
//! f32, max_speed f32` = 188 bytes. Header: `TMR0`, version u32, count u64,
//! n_gates u8, [pad 7] = 24 bytes. The count in the header is rewritten when
//! the shard is closed; a shard whose count disagrees with its length is
//! refused by `verify`.

use std::io::Write;

pub const TMR_VERSION: u32 = 1;
pub const STATE_VERSION: u32 = 1;

pub const OUTCOME_OK: u8 = 0;
pub const OUTCOME_CRASH_STOP: u8 = 1;
pub const OUTCOME_OFFWORLD: u8 = 2;
pub const OUTCOME_FINISHED: u8 = 3;
pub const OUTCOME_ABORTED: u8 = 4;

/// THE dataset's car state is `tmstate::CarState` itself (repr(C), 100 bytes; the
/// MODEL arm reads it through the same crate), not a copy.
pub use tmstate::CarState;

pub const CARSTATE_BYTES: usize = 100;
pub const RECORD_BYTES: usize = 4 + 2 + 2 + 1 + 3 + CARSTATE_BYTES + 64 + 4 + 4 + 4;
pub const HEADER_BYTES: usize = 24;

/// Building a `CarState` from an engine row.
pub trait FromRow {
    fn from_row(r: &forkoracle::layout::Row, race_ms: i64, cps: u8, finished: bool) -> CarState;
    /// The byte-exact TMR0 layout (100 bytes, little-endian, padding zeroed).
    fn write(&self, o: &mut Vec<u8>);
    fn read(b: &[u8]) -> CarState;
}

impl FromRow for CarState {
    /// From an engine row (pos/vel/quat); everything the readout does not
    /// expose is NaN / u8::MAX. `race_ms` is the TRUE race clock: the row's
    /// label + the worker's measured shift (`Worker::race_of`).
    fn from_row(r: &forkoracle::layout::Row, race_ms: i64, cps: u8, finished: bool) -> CarState {
        let v = [r.vx as f32, r.vy as f32, r.vz as f32];
        CarState {
            race_ms: race_ms as i32,
            pos: [r.x as f32, r.y as f32, r.z as f32],
            vel: v,
            quat: [r.qw as f32, r.qx as f32, r.qy as f32, r.qz as f32],
            ang_vel: [f32::NAN; 3],
            speed: (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
            gear: u8::MAX,
            rpm: f32::NAN,
            wheel_contact: [u8::MAX; 4],
            wheel_material: [u8::MAX; 4],
            wheel_slip: [f32::NAN; 4],
            turbo: f32::NAN,
            cps,
            finished,
        }
    }

    fn write(&self, o: &mut Vec<u8>) {
        let start = o.len();
        o.extend_from_slice(&self.race_ms.to_le_bytes());
        for x in self.pos.iter().chain(&self.vel).chain(&self.quat).chain(&self.ang_vel) {
            o.extend_from_slice(&x.to_le_bytes());
        }
        o.extend_from_slice(&self.speed.to_le_bytes());
        o.push(self.gear);
        o.extend_from_slice(&[0u8; 3]); // pad to align rpm
        o.extend_from_slice(&self.rpm.to_le_bytes());
        o.extend_from_slice(&self.wheel_contact);
        o.extend_from_slice(&self.wheel_material);
        for x in &self.wheel_slip {
            o.extend_from_slice(&x.to_le_bytes());
        }
        o.extend_from_slice(&self.turbo.to_le_bytes());
        o.push(self.cps);
        o.push(self.finished as u8);
        o.extend_from_slice(&[0u8; 2]); // struct tail pad
        debug_assert_eq!(o.len() - start, CARSTATE_BYTES);
    }

    fn read(b: &[u8]) -> CarState {
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        CarState {
            race_ms: i32::from_le_bytes(b[0..4].try_into().unwrap()),
            pos: [f(4), f(8), f(12)],
            vel: [f(16), f(20), f(24)],
            quat: [f(28), f(32), f(36), f(40)],
            ang_vel: [f(44), f(48), f(52)],
            speed: f(56),
            gear: b[60],
            rpm: f(64),
            wheel_contact: b[68..72].try_into().unwrap(),
            wheel_material: b[72..76].try_into().unwrap(),
            wheel_slip: [f(76), f(80), f(84), f(88)],
            turbo: f(92),
            cps: b[96],
            finished: b[97] != 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Record {
    pub start_id: u32,
    pub macro_id: u16,
    pub horizon_ticks: u16,
    pub outcome: u8,
    pub end: CarState,
    /// Tick within the rollout at which map waypoint w was FIRST crossed, -1 if not.
    pub gate_tick: [i16; 32],
    pub path_len_m: f32,
    pub min_speed: f32,
    pub max_speed: f32,
}

impl Record {
    pub fn write(&self, o: &mut Vec<u8>) {
        let start = o.len();
        o.extend_from_slice(&self.start_id.to_le_bytes());
        o.extend_from_slice(&self.macro_id.to_le_bytes());
        o.extend_from_slice(&self.horizon_ticks.to_le_bytes());
        o.push(self.outcome);
        o.extend_from_slice(&[0u8; 3]);
        self.end.write(o);
        for g in &self.gate_tick {
            o.extend_from_slice(&g.to_le_bytes());
        }
        o.extend_from_slice(&self.path_len_m.to_le_bytes());
        o.extend_from_slice(&self.min_speed.to_le_bytes());
        o.extend_from_slice(&self.max_speed.to_le_bytes());
        debug_assert_eq!(o.len() - start, RECORD_BYTES);
    }

    pub fn read(b: &[u8]) -> Record {
        let mut gate_tick = [0i16; 32];
        for (i, g) in gate_tick.iter_mut().enumerate() {
            let o = 12 + CARSTATE_BYTES + 2 * i;
            *g = i16::from_le_bytes(b[o..o + 2].try_into().unwrap());
        }
        let t = 12 + CARSTATE_BYTES + 64;
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        Record {
            start_id: u32::from_le_bytes(b[0..4].try_into().unwrap()),
            macro_id: u16::from_le_bytes(b[4..6].try_into().unwrap()),
            horizon_ticks: u16::from_le_bytes(b[6..8].try_into().unwrap()),
            outcome: b[8],
            end: CarState::read(&b[12..12 + CARSTATE_BYTES]),
            gate_tick,
            path_len_m: f(t),
            min_speed: f(t + 4),
            max_speed: f(t + 8),
        }
    }
}

/// A shard being written. The header's count is patched on `close`.
pub struct Writer {
    f: std::fs::File,
    path: std::path::PathBuf,
    count: u64,
    buf: Vec<u8>,
}

impl Writer {
    pub fn create(path: &std::path::Path, n_gates: u8) -> Result<Writer, String> {
        let mut f = std::fs::File::create(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let mut h = Vec::new();
        h.extend_from_slice(b"TMR0");
        h.extend_from_slice(&TMR_VERSION.to_le_bytes());
        h.extend_from_slice(&0u64.to_le_bytes());
        h.push(n_gates);
        h.extend_from_slice(&[0u8; 7]);
        f.write_all(&h).map_err(|e| e.to_string())?;
        Ok(Writer { f, path: path.to_path_buf(), count: 0, buf: Vec::new() })
    }

    pub fn push(&mut self, r: &Record) -> Result<(), String> {
        r.write(&mut self.buf);
        self.count += 1;
        if self.buf.len() > 1 << 20 {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), String> {
        self.f.write_all(&self.buf).map_err(|e| e.to_string())?;
        self.buf.clear();
        Ok(())
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    /// Flush, patch the count, fsync. Returns the record count.
    pub fn close(mut self) -> Result<u64, String> {
        use std::io::Seek;
        self.flush()?;
        self.f.seek(std::io::SeekFrom::Start(8)).map_err(|e| e.to_string())?;
        self.f.write_all(&self.count.to_le_bytes()).map_err(|e| e.to_string())?;
        self.f.sync_all().map_err(|e| e.to_string())?;
        let _ = &self.path;
        Ok(self.count)
    }
}

pub struct Shard {
    pub version: u32,
    pub n_gates: u8,
    pub records: Vec<Record>,
}

pub fn read_shard(path: &std::path::Path) -> Result<Shard, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    if b.len() < HEADER_BYTES || &b[0..4] != b"TMR0" {
        return Err(format!("{}: not a TMR0 shard", path.display()));
    }
    let version = u32::from_le_bytes(b[4..8].try_into().unwrap());
    let count = u64::from_le_bytes(b[8..16].try_into().unwrap()) as usize;
    let n_gates = b[16];
    let body = &b[HEADER_BYTES..];
    if body.len() != count * RECORD_BYTES {
        return Err(format!(
            "{}: header says {} records ({} bytes) but the body is {} bytes: an unclosed or truncated shard",
            path.display(),
            count,
            count * RECORD_BYTES,
            body.len()
        ));
    }
    let records = body.chunks(RECORD_BYTES).map(Record::read).collect();
    Ok(Shard { version, n_gates, records })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carstate_layout_is_the_repr_c_layout() {
        assert_eq!(std::mem::size_of::<CarState>(), CARSTATE_BYTES);
        assert_eq!(std::mem::offset_of!(CarState, gear), 60);
        assert_eq!(std::mem::offset_of!(CarState, rpm), 64);
        assert_eq!(std::mem::offset_of!(CarState, wheel_contact), 68);
        assert_eq!(std::mem::offset_of!(CarState, wheel_slip), 76);
        assert_eq!(std::mem::offset_of!(CarState, turbo), 92);
        assert_eq!(std::mem::offset_of!(CarState, cps), 96);
        assert_eq!(std::mem::offset_of!(CarState, finished), 97);
    }

    #[test]
    fn record_round_trips() {
        let mut end = CarState::from_row(
            &forkoracle::layout::Row { time_ms: 1230, x: 1.0, y: 2.0, z: 3.0, vx: 4.0, vy: 5.0, vz: 6.0, qx: 0.1, qy: 0.2, qz: 0.3, qw: 0.9, wetness: 0.0, cps: u32::MAX },
            1240,
            2,
            false,
        );
        end.gear = 3;
        let mut gt = [-1i16; 32];
        gt[3] = 77;
        let r = Record { start_id: 9, macro_id: 5, horizon_ticks: 300, outcome: OUTCOME_OK, end, gate_tick: gt, path_len_m: 12.5, min_speed: 1.0, max_speed: 90.0 };
        let mut b = Vec::new();
        r.write(&mut b);
        assert_eq!(b.len(), RECORD_BYTES);
        let q = Record::read(&b);
        assert_eq!(q.start_id, 9);
        assert_eq!(q.gate_tick[3], 77);
        assert_eq!(q.end.race_ms, 1240);
        assert_eq!(q.end.gear, 3);
        assert!(q.end.rpm.is_nan());
        assert_eq!(q.end.pos, end.pos);
    }
}
