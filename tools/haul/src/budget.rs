//! The budget that counts work, not wall-clock.
//!
//! The failure this exists for: on 2026-08-24 a search ran three times logging
//! zero evals per second, and the agreed switch threshold (8,000,000 evals /
//! 10 hours, after which a learned ordering over archive bins gets added)
//! burned its wall-clock the whole time. A budget that a stall consumes does
//! not mean what it says.
//!
//! So the clock only advances across a sampling interval in which the eval
//! counter actually moved. Stalled time is counted too, separately, because it
//! is diagnostic — but it never spends the budget.
//!
//! The counters are grow-only and sharded per box (`state/budget/<node>.rec`),
//! so the total is the sum over shards and two boxes never conflict.

use crate::log::{self, Log};
use crate::rec::Rec;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Counters {
    pub evals: u64,
    pub productive_s: i64,
    pub stalled_s: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Policy {
    pub switch_evals: u64,
    pub switch_productive_s: i64,
}

impl Default for Policy {
    fn default() -> Self {
        // The pre-committed switch condition from DESIGN.md §3.2.
        Policy { switch_evals: 8_000_000, switch_productive_s: 10 * 3600 }
    }
}

/// Fold one sampling interval into the counters.
///
/// `delta_evals` is the eval counter's movement across `dt_s` seconds. This is
/// the whole rule, in one place, so it can be tested without a box, a search
/// or an engine.
pub fn fold(c: &mut Counters, delta_evals: u64, dt_s: i64) {
    if dt_s <= 0 {
        return;
    }
    c.evals += delta_evals;
    if delta_evals > 0 {
        c.productive_s += dt_s;
    } else {
        c.stalled_s += dt_s;
    }
}

impl Counters {
    pub fn spent_fraction(&self, p: &Policy) -> f64 {
        let by_evals = self.evals as f64 / p.switch_evals.max(1) as f64;
        let by_time = self.productive_s as f64 / p.switch_productive_s.max(1) as f64;
        by_evals.max(by_time)
    }

    pub fn switch_reached(&self, p: &Policy) -> bool {
        self.evals >= p.switch_evals || self.productive_s >= p.switch_productive_s
    }
}

/// Append one interval to this box's shard.
pub fn record(dir: &Path, node: &str, delta_evals: u64, dt_s: i64) -> std::io::Result<()> {
    let log = Log::at(dir.join(format!("{node}.rec")));
    log.append(
        &Rec::new("budget_interval")
            .f("delta_evals", delta_evals)
            .f("dt_s", dt_s)
            .f("productive", if delta_evals > 0 { 1 } else { 0 }),
    )
}

/// Subtract work that was counted and should not have been.
///
/// The log is append-only and is never rewritten, so a miscount is corrected
/// by recording the correction — with its reason, which is the part that still
/// matters when somebody reads this in three months and wonders why the
/// numbers step backwards. It exists because a real miscount happened: a
/// restarted supervisor whose eval baseline started at zero counted its whole
/// resume point as fresh work.
pub fn correct(dir: &Path, node: &str, evals: u64, productive_s: i64, why: &str) -> std::io::Result<()> {
    let log = Log::at(dir.join(format!("{node}.rec")));
    log.append(
        &Rec::new("budget_correction")
            .f("evals", evals)
            .f("productive_s", productive_s)
            .f("why", why),
    )
}

/// Total across every box that has ever run, reconstructed from the repo alone.
pub fn total(dir: &Path) -> Result<Counters, String> {
    let recs = log::read_all(dir)?;
    let mut c = Counters::default();
    for r in &recs {
        match r.kind.as_str() {
            "budget_interval" => {
                let de = r.get_u64("delta_evals").unwrap_or(0);
                let dt = r.get_i64("dt_s").unwrap_or(0);
                fold(&mut c, de, dt);
            }
            "budget_correction" => {
                c.evals = c.evals.saturating_sub(r.get_u64("evals").unwrap_or(0));
                c.productive_s = (c.productive_s - r.get_i64("productive_s").unwrap_or(0)).max(0);
            }
            _ => {}
        }
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stall_does_not_spend_the_budget() {
        // The exact failure of 2026-08-24: three runs, zero evals per second,
        // hours of wall-clock. The clock must not move.
        let mut c = Counters::default();
        for _ in 0..(6 * 60) {
            fold(&mut c, 0, 60); // six hours of nothing
        }
        assert_eq!(c.productive_s, 0, "a stall spent productive budget");
        assert_eq!(c.stalled_s, 6 * 3600, "and it must still be visible as a stall");
        assert_eq!(c.evals, 0);
        assert!(!c.switch_reached(&Policy::default()));
        assert_eq!(c.spent_fraction(&Policy::default()), 0.0);
    }

    #[test]
    fn real_work_does_spend_it() {
        // The positive control: the same clock, with the counter moving.
        let mut c = Counters::default();
        for _ in 0..(10 * 60) {
            fold(&mut c, 1_000, 60);
        }
        assert_eq!(c.productive_s, 10 * 3600);
        assert_eq!(c.stalled_s, 0);
        assert!(c.switch_reached(&Policy::default()), "10 productive hours is the threshold");
    }

    #[test]
    fn the_eval_arm_can_reach_the_threshold_on_its_own() {
        let mut c = Counters::default();
        fold(&mut c, 8_000_000, 60);
        assert!(c.switch_reached(&Policy::default()));
        assert_eq!(c.productive_s, 60, "and it took one minute of clock, not ten hours");
    }

    #[test]
    fn a_mixed_run_counts_only_the_moving_intervals() {
        let mut c = Counters::default();
        for i in 0..100 {
            fold(&mut c, if i % 2 == 0 { 10 } else { 0 }, 10);
        }
        assert_eq!(c.productive_s, 500);
        assert_eq!(c.stalled_s, 500);
        assert_eq!(c.evals, 500);
    }

    #[test]
    fn a_miscount_is_corrected_by_appending_not_by_rewriting() {
        let d = std::env::temp_dir().join(format!("haul-budget-fix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        record(&d, "boxA", 500, 60).unwrap();
        record(&d, "boxA", 194, 60).unwrap();   // the double-counted resume point
        assert_eq!(total(&d).unwrap().evals, 694);
        correct(&d, "boxA", 194, 0, "a restarted supervisor counted its resume point as fresh work").unwrap();
        assert_eq!(total(&d).unwrap().evals, 500);
        // and the record of both the miscount and the correction survives
        let text = std::fs::read_to_string(d.join("boxA.rec")).unwrap();
        assert!(text.contains("budget_correction"));
        assert!(text.contains("resume point as fresh work"));
        assert_eq!(text.lines().count(), 3, "nothing was rewritten");
    }

    #[test]
    fn a_correction_cannot_drive_the_budget_below_zero() {
        let d = std::env::temp_dir().join(format!("haul-budget-neg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        record(&d, "boxA", 10, 60).unwrap();
        correct(&d, "boxA", 999, 999, "over-correction").unwrap();
        let t = total(&d).unwrap();
        assert_eq!(t.evals, 0);
        assert_eq!(t.productive_s, 0);
    }

    #[test]
    fn totals_reconstruct_from_shards_written_by_different_boxes() {
        let d = std::env::temp_dir().join(format!("haul-budget-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        record(&d, "boxA", 100, 60).unwrap();
        record(&d, "boxB", 0, 60).unwrap();
        record(&d, "boxA", 50, 60).unwrap();
        let t = total(&d).unwrap();
        assert_eq!(t.evals, 150);
        assert_eq!(t.productive_s, 120);
        assert_eq!(t.stalled_s, 60);
    }
}
