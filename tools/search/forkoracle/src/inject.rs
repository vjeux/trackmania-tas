//! CAR-STATE INJECTION (2026-09-09, with the MODEL arm): write a full rigid-body state into the fork server's LIVE dyna
//! body record -- the 88-byte record the solver integrates (`car.rs`: +0x00 3x3 rotation, +0x24 position, +0x30
//! quaternion (w,x,y,z), +0x40 linear velocity, +0x4c angular velocity) -- so a measured car state (vjeux's LaunchedCP
//! crossings) becomes a search SAVESTATE instead of a state the lanes must drive to.
//!
//! What this can and cannot set: the record IS the whole rigid-body state the solver reads; wheel/suspension/contact
//! caches live elsewhere and are not mapped, so after an inject the car settles for a few ticks (the caches belong to
//! the paused fork's own car at that tick). Write the ROOT server at its checkpoint and every fork from it inherits the
//! state; or write a paused child by its pid (same addresses -- a fork shares the address space layout).
//!
//! No dependencies here (the shim compiles part of this crate): the JSON parsing of a state file is the caller's
//! (tmenv `inject`, tmreach `--inject-state`); this module takes plain arrays.

use crate::car::Car;
use crate::procmem;

/// A rigid-body state, world frame, metres / seconds / radians per second; quaternion (w, x, y, z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyState {
    pub pos: [f32; 3],
    pub quat_wxyz: [f32; 4],
    pub vel: [f32; 3],
    pub ang_vel: [f32; 3],
}

impl BodyState {
    /// The 3x3 rotation matrix the record carries beside the quaternion, row-major, from a unit quaternion.
    pub fn rot(&self) -> [f32; 9] {
        let [w, x, y, z] = self.quat_wxyz;
        let n = (w * w + x * x + y * y + z * z).sqrt().max(1e-9);
        let (w, x, y, z) = (w / n, x / n, y / n, z / n);
        [
            1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w),
            2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w),
            2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y),
        ]
    }
    /// The 88-byte record image: rotation, position, quaternion, velocity, angular velocity.
    pub fn record(&self) -> [u8; 88] {
        let mut b = [0u8; 88];
        let mut put = |off: usize, v: &[f32]| { for (i, f) in v.iter().enumerate() { b[off + 4 * i..off + 4 * i + 4].copy_from_slice(&f.to_le_bytes()); } };
        put(0x00, &self.rot());
        put(0x24, &self.pos);
        put(0x30, &self.quat_wxyz);
        put(0x40, &self.vel);
        put(0x4c, &self.ang_vel);
        b
    }
    /// Decode a record image.
    pub fn from_record(b: &[u8]) -> Option<BodyState> {
        if b.len() < 88 { return None; }
        let g = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        Some(BodyState {
            pos: [g(0x24), g(0x28), g(0x2c)],
            quat_wxyz: [g(0x30), g(0x34), g(0x38), g(0x3c)],
            vel: [g(0x40), g(0x44), g(0x48)],
            ang_vel: [g(0x4c), g(0x50), g(0x54)],
        })
    }
    pub fn dist(&self, o: &BodyState) -> f32 {
        ((self.pos[0] - o.pos[0]).powi(2) + (self.pos[1] - o.pos[1]).powi(2) + (self.pos[2] - o.pos[2]).powi(2)).sqrt()
    }
    pub fn speed(&self) -> f32 {
        (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1] + self.vel[2] * self.vel[2]).sqrt()
    }
}

/// Read the live body record of `car` in process `pid`. `None` inside a respawn window (no body).
pub fn read_body(pid: i32, car: &Car) -> Result<BodyState, String> {
    let b = car.body.ok_or("the car has no body right now (respawn window): nothing to read")?;
    let bytes = procmem::read_at(pid, b.addr, 88).ok_or_else(|| format!("cannot read the body record at {:#x} of pid {pid}", b.addr))?;
    BodyState::from_record(&bytes).ok_or_else(|| "short record".into())
}

/// Write `st` into the live body record of `car` in process `pid` and read it back. The write is refused when the car
/// has no body (respawn window) or when the read-back differs (a wrong address would otherwise pass silently).
pub fn write_body(pid: i32, car: &Car, st: &BodyState) -> Result<BodyState, String> {
    let b = car.body.ok_or("the car has no body right now (respawn window): cannot inject")?;
    procmem::write_at(pid, b.addr, &st.record())?;
    let back = read_body(pid, car)?;
    if back.dist(st) > 1e-4 || (back.speed() - st.speed()).abs() > 1e-4 {
        return Err(format!("read-back differs from the write: wrote pos {:?} v {:?}, read pos {:?} v {:?}", st.pos, st.vel, back.pos, back.vel));
    }
    Ok(back)
}

/// EXPERIMENT (2026-09-09): the body-record write alone did not steer the next step on Tiny 20 (the car kept its old
/// trajectory although the read-back matched), so also write the vehicle's own copies -- the phy copy-out fields
/// (quat +0x12e0, pos +0x12f0, vel +0x12fc, angvel +0x1308) -- in case the step seeds the solver from them.
pub fn write_body_and_phy(pid: i32, car: &Car, st: &BodyState) -> Result<BodyState, String> {
    let back = write_body(pid, car, st)?;
    let mut put = |addr: u64, v: &[f32]| -> Result<(), String> { let mut b = Vec::with_capacity(4 * v.len()); for f in v { b.extend_from_slice(&f.to_le_bytes()); } procmem::write_at(pid, addr, &b).map(|_| ()) };
    put(car.quat(), &st.quat_wxyz)?;
    put(car.pos(), &st.pos)?;
    put(car.vel(), &st.vel)?;
    put(car.angvel(), &st.ang_vel)?;
    Ok(back)
}
