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

/// The vehicle's PHYSICS copies of the drivetrain state, found by `tmenv phy-scan` on Tiny 20 (Stadium car, 2026-09-09):
/// each value the post-step vis state shows exists once more inside the phy, outside both vis copies --
/// FrontSpeed at phy+0x142c, engine rpm at phy+0x15d8 (and a twin at +0x1608), and per wheel k (stride 0xb8 from
/// phy+0x1780: front-left, front-right, rear-left, rear-right) damper length +0x00, RotSpeed +0x1c, Rot +0x78.
/// Wheel radii from the same scan: RotSpeed/FrontSpeed = 3.258 (front, r = 0.307 m) and 2.869 (rear, r = 0.349 m).
pub const PHY_FRONT_SPEED: u64 = 0x142c;
pub const PHY_RPM: [u64; 2] = [0x15d8, 0x1608];
/// The GEAR (u32) right after each rpm copy (phy-scan diff of an injected car (gear 0, rpm 1310 idle, no drive, steer
/// capped at 0.2) against a driving one, Poland 2026-09-10): the injected body keeps the paused template car's gear 0 =
/// neutral, so the engine never drives it -- THE inject residual (15-20 % slower, 0.2 steer).
pub const PHY_GEAR: [u64; 2] = [0x15dc, 0x160c];
pub const PHY_WHEEL0: u64 = 0x1780;
pub const PHY_WHEEL_STRIDE: u64 = 0xb8;
pub const PHY_WHEEL_ROTSPEED: u64 = 0x1c;
pub const WHEEL_RAD_PER_M: [f32; 4] = [3.258, 3.258, 2.869, 2.869];

/// Write the drivetrain to match a forward speed: wheel angular speeds for rolling without slip, FrontSpeed, and the
/// engine rpm when given (else left as the paused car had it). Call after `write_body_and_phy`.
pub fn write_drivetrain(pid: i32, car: &Car, speed_fwd: f32, rpm: Option<f32>) -> Result<(), String> { write_drivetrain_gear(pid, car, speed_fwd, rpm, None) }
/// `write_drivetrain` plus the gear (u32 at PHY_GEAR, both copies).
pub fn write_drivetrain_gear(pid: i32, car: &Car, speed_fwd: f32, rpm: Option<f32>, gear: Option<u32>) -> Result<(), String> {
    procmem::write_at(pid, car.phy + PHY_FRONT_SPEED, &speed_fwd.to_le_bytes())?;
    if let Some(r) = rpm { for o in PHY_RPM { procmem::write_at(pid, car.phy + o, &r.to_le_bytes())?; } }
    if let Some(g) = gear { for o in PHY_GEAR { procmem::write_at(pid, car.phy + o, &g.to_le_bytes())?; } }
    // TMENV_INJECT_WHEEL_RATIO=r overrides the rad/m ratio (all wheels); "0" skips the wheel-speed write.
    let ratio_override: Option<f32> = std::env::var("TMENV_INJECT_WHEEL_RATIO").ok().and_then(|s| s.parse().ok());
    if ratio_override == Some(0.0) { return Ok(()); }
    for k in 0..4u64 {
        let w = speed_fwd * ratio_override.unwrap_or(WHEEL_RAD_PER_M[k as usize]);
        procmem::write_at(pid, car.phy + PHY_WHEEL0 + k * PHY_WHEEL_STRIDE + PHY_WHEEL_ROTSPEED, &w.to_le_bytes())?;
    }
    Ok(())
}
