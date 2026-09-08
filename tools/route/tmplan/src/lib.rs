//! `tmplan` — the route planner skeleton (BRIEF-GEOM R5).
//!
//! * `surface`   — the cartographer's surface graph over the FULL checkpoint set of a `gates.json`
//! * `estimator` — the `EdgeEstimator` seam; `Geometric` is the first implementation, R is the second
//! * `planner`   — beam search over (visited, gate group, arrival bucket), finish last
//! * `export`    — a plan → `router-plan` route file with `Predicted` legs

pub mod estimator;
pub mod export;
pub mod planner;
pub mod surface;

/// The pak files the geometry comes from: `TM_PAKS` (colon-separated paths) when set, else the dedicated
/// server's `$TM_SERVER/Packs/{dedicated_TMStadium,dedicated,resource}.pak`. The server tree under /tmp was
/// wiped once (2026-09-07 10:26Z, every plan failed with "no .pak"); the game client's `Stadium.pak` in the
/// bank is the fallback that keeps the tools alive.
pub fn pak_paths() -> Result<Vec<String>, String> {
    if let Ok(list) = std::env::var("TM_PAKS") {
        let v: Vec<String> = list.split(':').filter(|s| !s.is_empty() && std::path::Path::new(s).exists()).map(|s| s.to_string()).collect();
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let server = std::env::var("TM_SERVER").map_err(|_| "TM_SERVER not set (and no TM_PAKS)")?;
    let v: Vec<String> = ["dedicated_TMStadium.pak", "dedicated.pak", "resource.pak"].iter().map(|n| format!("{server}/Packs/{n}")).filter(|p| std::path::Path::new(p).exists()).collect();
    if v.is_empty() {
        return Err(format!("no .pak in {server}/Packs (set TM_PAKS=path.pak:path2.pak)"));
    }
    Ok(v)
}

/// Advisory speed (m/s) per centreline point — the player's `speed_hint`: horizontal curvature (lateral grip
/// `a_lat`), vertical convexity (a crest or a dip's exit where following the road needs more than `g_follow`
/// downward acceleration = the car leaves the ground), a ceiling, then braking/acceleration propagation along `s`.
/// Curvatures from 3-point circles over a ±`win` m window. All units SI.
pub fn speed_hints(pts: &[[f32; 3]], s: &[f32], win: f32, a_lat: f32, g_follow: f32, v_cap: f32, a_brake: f32, a_acc: f32) -> Vec<f32> {
    let n = pts.len();
    if n < 3 {
        return vec![v_cap; n];
    }
    let idx_at = |i: usize, d: f32| -> usize {
        let target = s[i] + d;
        if d < 0.0 { (0..=i).rev().find(|&k| s[k] <= target).unwrap_or(0) } else { (i..n).find(|&k| s[k] >= target).unwrap_or(n - 1) }
    };
    let mut v = vec![v_cap; n];
    for i in 0..n {
        let a = idx_at(i, -win);
        let b = idx_at(i, win);
        if a == i || b == i { continue; }
        let (p, q, r) = (pts[a], pts[i], pts[b]);
        // horizontal curvature: circle through the XZ projections
        let kh = {
            let (ax, az, bx, bz, cx, cz) = (p[0], p[2], q[0], q[2], r[0], r[2]);
            let area2 = ((bx - ax) * (cz - az) - (bz - az) * (cx - ax)).abs();
            let l1 = ((bx - ax).powi(2) + (bz - az).powi(2)).sqrt();
            let l2 = ((cx - bx).powi(2) + (cz - bz).powi(2)).sqrt();
            let l3 = ((cx - ax).powi(2) + (cz - az).powi(2)).sqrt();
            if l1 * l2 * l3 < 1e-3 { 0.0 } else { 2.0 * area2 / (l1 * l2 * l3) }
        };
        // vertical: second derivative of y along s; convex (crest / dip exit) when negative
        let (s1, s2) = (s[i] - s[a], s[b] - s[i]);
        let kv = if s1 > 0.5 && s2 > 0.5 { 2.0 * ((r[1] - q[1]) / s2 - (q[1] - p[1]) / s1) / (s1 + s2) } else { 0.0 };
        let mut vi = v_cap;
        if kh > 1e-4 { vi = vi.min((a_lat / kh).sqrt()); }
        if kv < -1e-4 { vi = vi.min((g_follow / -kv).sqrt()); }
        v[i] = vi.max(8.0);
    }
    // braking: a point's speed must be reachable from the next one
    for i in (0..n - 1).rev() {
        let ds = (s[i + 1] - s[i]).max(0.01);
        v[i] = v[i].min((v[i + 1] * v[i + 1] + 2.0 * a_brake * ds).sqrt());
    }
    // acceleration from the spawn (standing start)
    v[0] = v[0].min(8.0);
    for i in 1..n {
        let ds = (s[i] - s[i - 1]).max(0.01);
        v[i] = v[i].min((v[i - 1] * v[i - 1] + 2.0 * a_acc * ds).sqrt());
    }
    v
}
