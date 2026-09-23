//! The course intro: MK64 opens every race with a fly-over of the course.
//! Three cuts, like the Summer campaign intros: a high overview of the
//! whole course, a low fly-along of a mid-course stretch, and a sweep down
//! onto the start line looking up the road.

use tmmaps::mtauthor::CamKey;

/// Camera pitch/yaw looking from `from` at `to` (pitch positive = down,
/// yaw = atan2(dx, dz)).
fn look(from: [f32; 3], to: [f32; 3]) -> [f32; 3] {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let h = (d[0] * d[0] + d[2] * d[2]).sqrt().max(1e-3);
    [(-d[1]).atan2(h), d[0].atan2(d[2]), 0.0]
}

/// The three shots over `path` (TM frame, closed lap), total ~14 s.
pub fn shots(path: &[[f32; 3]]) -> Vec<Vec<CamKey>> {
    let n = path.len();
    if n < 8 {
        return Vec::new();
    }
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in path {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let extent = ((hi[0] - lo[0]).max(hi[2] - lo[2])).max(50.0);
    let at = |i: usize| path[i % n];
    let start = at(0);
    let ahead = at(n / 40);
    let start_dir = {
        let d = [ahead[0] - start[0], 0.0, ahead[2] - start[2]];
        let l = (d[0] * d[0] + d[2] * d[2]).sqrt().max(1e-3);
        [d[0] / l, 0.0, d[2] / l]
    };
    // 1. the overview: high above, drifting sideways, looking at the course centre
    let h1 = extent * 0.9;
    let a = [centre[0] - extent * 0.7, centre[1] + h1, centre[2] + extent * 0.9];
    let b = [centre[0] + extent * 0.1, centre[1] + h1 * 0.9, centre[2] + extent * 1.0];
    let shot1 = vec![
        CamKey { time: 0.0, position: a, pitch_yaw_roll: look(a, centre), fov: 60.0 },
        CamKey { time: 5.0, position: b, pitch_yaw_roll: look(b, centre), fov: 60.0 },
    ];
    // 2. a fly-along of the stretch at 35..45 % of the lap, 12 m up, looking ahead
    let (i0, i1) = (n * 35 / 100, n * 45 / 100);
    let mut shot2 = Vec::new();
    let steps = 5;
    for s in 0..=steps {
        let i = i0 + (i1 - i0) * s / steps;
        let p = at(i);
        let q = at(i + n / 30);
        let pos = [p[0], p[1] + 12.0, p[2]];
        shot2.push(CamKey { time: 5.0 + 5.0 * s as f32 / steps as f32, position: pos, pitch_yaw_roll: look(pos, [q[0], q[1] + 2.0, q[2]]), fov: 70.0 });
    }
    // 3. the sweep onto the start: from high behind the line down to a chase height
    let back = |d: f32, up: f32| [start[0] - start_dir[0] * d, start[1] + up, start[2] - start_dir[2] * d];
    let target = [start[0] + start_dir[0] * 60.0, start[1] + 1.0, start[2] + start_dir[2] * 60.0];
    let c = back(90.0, 45.0);
    let d = back(12.0, 4.0);
    let shot3 = vec![
        CamKey { time: 10.0, position: c, pitch_yaw_roll: look(c, target), fov: 65.0 },
        CamKey { time: 14.0, position: d, pitch_yaw_roll: look(d, target), fov: 75.0 },
    ];
    let mut out = vec![shot1, shot2, shot3];
    // yaw continuity inside a shot: no long way round the wrap at ±π
    for shot in out.iter_mut() {
        for i in 1..shot.len() {
            let prev = shot[i - 1].pitch_yaw_roll[1];
            let y = &mut shot[i].pitch_yaw_roll[1];
            while *y - prev > std::f32::consts::PI {
                *y -= std::f32::consts::TAU;
            }
            while prev - *y > std::f32::consts::PI {
                *y += std::f32::consts::TAU;
            }
        }
    }
    out
}
