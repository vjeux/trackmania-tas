//! The car frame. Quaternions are `(w, x, y, z)` as the validator reads them
//! (tmstate `CarState::quat`); `rotate` maps a CAR-frame vector to the WORLD
//! frame, `to_car` the inverse. Which local axis is "forward" is MEASURED, not
//! assumed: `tmr frame` reports the mean alignment of the velocity direction
//! with each rotated local axis over a starts.tsv (FEATURES.md §1 records the
//! number). Result on Summer 2026 - 01 (g1-starts, 39 rows): local +Z.

/// 3×3 rotation matrix (row-major) of the unit quaternion (w, x, y, z).
pub fn matrix(q: [f32; 4]) -> [[f32; 3]; 3] {
    let [w, x, y, z] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)],
        [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)],
        [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

/// Car-frame vector → world frame.
pub fn rotate(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// World-frame vector → car frame (the transpose).
pub fn to_car(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

/// Quaternion (w, x, y, z) whose rotation sends local +Z to the horizontal
/// world direction `dir` (XZ) — a yaw-only attitude for a synthetic start.
pub fn yaw_quat(dir: [f32; 2]) -> [f32; 4] {
    let yaw = dir[0].atan2(dir[1]); // angle from +Z towards +X about the Y axis
    let h = 0.5 * yaw;
    [h.cos(), 0.0, h.sin(), 0.0]
}

pub fn norm3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

pub fn unit3(v: [f32; 3]) -> Option<[f32; 3]> {
    let n = norm3(v);
    if n < 1e-6 { None } else { Some([v[0] / n, v[1] / n, v[2] / n]) }
}

/// Mean dot of the velocity direction with each rotated local axis (±X, ±Y,
/// ±Z), over rows moving faster than `min_speed`. The forward axis is the one
/// nearest +1; a wrong quaternion convention shows as no axis near ±1.
pub fn alignment(rows: &[([f32; 3], [f32; 4])], min_speed: f32) -> ([f32; 3], usize) {
    let mut acc = [0f32; 3];
    let mut n = 0usize;
    for (vel, q) in rows {
        let Some(d) = unit3(*vel) else { continue };
        if norm3(*vel) < min_speed {
            continue;
        }
        let m = matrix(*q);
        for a in 0..3 {
            let mut e = [0f32; 3];
            e[a] = 1.0;
            let w = rotate(&m, e);
            acc[a] += w[0] * d[0] + w[1] * d[1] + w[2] * d[2];
        }
        n += 1;
    }
    if n > 0 {
        for a in acc.iter_mut() {
            *a /= n as f32;
        }
    }
    (acc, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn yaw_quat_sends_z_to_dir() {
        for (dx, dz) in [(0.0f32, 1.0f32), (1.0, 0.0), (-1.0, 0.0), (0.6, -0.8)] {
            let q = yaw_quat([dx, dz]);
            let m = matrix(q);
            let f = rotate(&m, [0.0, 0.0, 1.0]);
            assert!((f[0] - dx).abs() < 1e-5 && (f[2] - dz).abs() < 1e-5 && f[1].abs() < 1e-5, "{:?} -> {:?}", (dx, dz), f);
            let back = to_car(&m, f);
            assert!((back[2] - 1.0).abs() < 1e-5);
        }
    }
}
