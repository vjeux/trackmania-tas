//! Quaternion and frame arithmetic, and the route probe the observation needs.
//!
//! Everything the policy sees about attitude is expressed **in the car's own
//! frame**. A world-frame velocity is a different number on every corner of the
//! map for the same physical situation, and a network then has to learn the map
//! instead of the car.

pub type V3 = [f32; 3];

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn norm(a: V3) -> f32 {
    dot(a, a).sqrt()
}

pub fn unit(a: V3) -> V3 {
    let n = norm(a);
    if n < 1e-9 {
        [0.0, 0.0, 0.0]
    } else {
        scale(a, 1.0 / n)
    }
}

/// A unit quaternion as the engine stores it: `(x, y, z, w)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat(pub f32, pub f32, pub f32, pub f32);

impl Quat {
    pub fn conj(self) -> Quat {
        Quat(-self.0, -self.1, -self.2, self.3)
    }

    pub fn mul(self, o: Quat) -> Quat {
        let (ax, ay, az, aw) = (self.0, self.1, self.2, self.3);
        let (bx, by, bz, bw) = (o.0, o.1, o.2, o.3);
        Quat(
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz,
        )
    }

    /// Rotate a world vector into the frame this quaternion describes.
    ///
    /// The engine's quaternion takes car-frame to world, so the inverse
    /// rotation is what puts a world vector in the car's frame.
    pub fn world_to_car(self, v: V3) -> V3 {
        self.conj().rotate(v)
    }

    pub fn rotate(self, v: V3) -> V3 {
        let u = [self.0, self.1, self.2];
        let s = self.3;
        let a = scale(u, 2.0 * dot(u, v));
        let b = scale(v, s * s - dot(u, u));
        let c = scale(cross(u, v), 2.0 * s);
        add(add(a, b), c)
    }

    pub fn len(self) -> f32 {
        (self.0 * self.0 + self.1 * self.1 + self.2 * self.2 + self.3 * self.3).sqrt()
    }
}

/// Angular velocity in the car frame, from two consecutive orientations.
///
/// `dt` in seconds. The engine's per-tick readout does not carry an angular
/// velocity channel — that is a readout we have not widened yet, not a quantity
/// the engine lacks — so this is differenced from the quaternions, which are
/// read to ~2e-5.
pub fn ang_vel_car(prev: Quat, cur: Quat, dt: f32) -> V3 {
    if dt <= 0.0 {
        return [0.0, 0.0, 0.0];
    }
    // Relative rotation from prev to cur, expressed in the previous car frame.
    let mut d = prev.conj().mul(cur);
    // Both q and -q are the same rotation; pick the short way round, or a
    // 180-degree flip in the sign convention reads as an enormous spin.
    if d.3 < 0.0 {
        d = Quat(-d.0, -d.1, -d.2, -d.3);
    }
    let s = 2.0 / dt;
    [d.0 * s, d.1 * s, d.2 * s]
}
