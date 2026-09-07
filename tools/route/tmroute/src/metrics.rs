//! Order-agreement metrics between two routes.

/// Exact match of two order lines.
pub fn exact(a: &[u32], b: &[u32]) -> bool {
    a == b
}

/// Kendall tau-b over the elements both orders contain, ranked by position.
/// 1.0 = same order, -1.0 = reversed, NaN when fewer than two shared elements.
/// Elements only one side has are reported by `symmetric_difference`.
pub fn kendall_tau(a: &[u32], b: &[u32]) -> f64 {
    let common: Vec<u32> = a.iter().copied().filter(|x| b.contains(x)).collect();
    let n = common.len();
    if n < 2 {
        return f64::NAN;
    }
    let pos = |seq: &[u32], x: u32| seq.iter().position(|y| *y == x).unwrap() as i64;
    let mut conc = 0i64;
    let mut disc = 0i64;
    for i in 0..n {
        for j in i + 1..n {
            let da = pos(a, common[i]) - pos(a, common[j]);
            let db = pos(b, common[i]) - pos(b, common[j]);
            if da.signum() == db.signum() {
                conc += 1;
            } else {
                disc += 1;
            }
        }
    }
    (conc - disc) as f64 / (n * (n - 1) / 2) as f64
}

/// Elements in exactly one of the two orders: (only in a, only in b).
pub fn symmetric_difference(a: &[u32], b: &[u32]) -> (Vec<u32>, Vec<u32>) {
    (
        a.iter().copied().filter(|x| !b.contains(x)).collect(),
        b.iter().copied().filter(|x| !a.contains(x)).collect(),
    )
}

/// Map an order of waypoints onto an order of checkpoint GROUPS (so two routes
/// that cross different pieces of one gate row agree). Unknown waypoints are kept
/// as `u32::MAX - wp` so they never collide with a group id.
pub fn to_groups(order: &[u32], gates: &crate::gates::GatesFile) -> Vec<u32> {
    order
        .iter()
        .map(|wp| gates.by_waypoint(*wp).map(|g| g.group).unwrap_or(u32::MAX - *wp))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tau() {
        assert_eq!(kendall_tau(&[1, 2, 3, 4], &[1, 2, 3, 4]), 1.0);
        assert_eq!(kendall_tau(&[1, 2, 3, 4], &[4, 3, 2, 1]), -1.0);
        assert!((kendall_tau(&[1, 2, 3, 4], &[2, 1, 3, 4]) - (4.0 / 6.0)).abs() < 1e-9); // 6 pairs, 1 discordant
        assert!(kendall_tau(&[1], &[1]).is_nan());
        // a shared subset only
        assert_eq!(kendall_tau(&[1, 2, 3, 9], &[1, 2, 3, 7]), 1.0);
        assert_eq!(symmetric_difference(&[1, 2, 9], &[1, 2, 7]), (vec![9], vec![7]));
    }
}

/// FNV-1a 64 of a map uid — the MODEL arm's split rule: `fnv1a64(uid) % 10 == 0` is HELD OUT of R's
/// training forever, so those maps are the honest reserve for the exhibit.
pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
pub fn fnv_held_out(uid: &str) -> bool {
    fnv1a64(uid) % 10 == 0
}
