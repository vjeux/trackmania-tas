//! The job spec: `autopilot/config/job.rec`.
//!
//! Everything the supervisor needs in order to run the right work, in the
//! repo, so a fresh box reads it out of the checkout rather than being told by
//! an agent who may be gone. Plain `key = value` lines with `#` comments —
//! a config a person edits should be readable when they open it.

use crate::alarms;
use crate::budget;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Job {
    /// The work itself. Run with `sh -c` on the box.
    pub worker_cmd: String,
    pub worker_dir: String,
    /// Where the worker appends its progress records. The supervisor only
    /// ever reads this file; it never asks the worker anything.
    pub progress_file: String,
    pub sample_s: i64,
    pub bank_s: i64,
    pub claim_ttl_s: i64,
    /// Bank and stand down this long before the box's lease runs out.
    pub lease_bank_lead_s: i64,
    pub restart_max: i64,
    pub restart_backoff_s: i64,
    pub mirror: String,
    pub mirror_dir: String,
    pub push: String,
    pub branch: String,
    pub objective: String,
    pub map_name: String,
    pub rung: String,
    /// Which budget this job's work spends. The pre-committed switch condition
    /// belongs to the archive search; a job that is not that search must not
    /// spend it.
    pub budget_key: String,
    pub alarms: alarms::Config,
    pub budget: budget::Policy,
    pub extra: BTreeMap<String, String>,
}

impl Default for Job {
    fn default() -> Self {
        Job {
            worker_cmd: String::new(),
            worker_dir: "/tmp/tmtas".into(),
            progress_file: "/tmp/tmhaul/progress.rec".into(),
            sample_s: 30,
            bank_s: 900,
            claim_ttl_s: 3600,
            lease_bank_lead_s: 1800,
            restart_max: 5,
            restart_backoff_s: 60,
            mirror: "paste".into(),
            mirror_dir: String::new(),
            push: "auto".into(),
            branch: "main".into(),
            objective: "furthest station on our own route".into(),
            map_name: "Summer 2026 - 01".into(),
            rung: "1 — reach CP1".into(),
            budget_key: "archive-search".into(),
            alarms: alarms::Config::default(),
            budget: budget::Policy::default(),
            extra: BTreeMap::new(),
        }
    }
}

fn parse_i(v: &str, name: &str, errs: &mut Vec<String>, slot: &mut i64) {
    match v.parse() {
        Ok(n) => *slot = n,
        Err(_) => errs.push(format!("{name}: {v:?} is not a number")),
    }
}

impl Job {
    pub fn parse(text: &str) -> Result<Job, String> {
        let mut j = Job::default();
        let mut errs = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                errs.push(format!("line {}: {raw:?} has no '='", i + 1));
                continue;
            };
            let (k, v) = (k.trim(), v.trim().to_string());
            match k {
                "worker_cmd" => j.worker_cmd = v,
                "worker_dir" => j.worker_dir = v,
                "progress_file" => j.progress_file = v,
                "sample_s" => parse_i(&v, k, &mut errs, &mut j.sample_s),
                "bank_s" => parse_i(&v, k, &mut errs, &mut j.bank_s),
                "claim_ttl_s" => parse_i(&v, k, &mut errs, &mut j.claim_ttl_s),
                "lease_bank_lead_s" => parse_i(&v, k, &mut errs, &mut j.lease_bank_lead_s),
                "restart_max" => parse_i(&v, k, &mut errs, &mut j.restart_max),
                "restart_backoff_s" => parse_i(&v, k, &mut errs, &mut j.restart_backoff_s),
                "mirror" => j.mirror = v,
                "mirror_dir" => j.mirror_dir = v,
                "push" => j.push = v,
                "branch" => j.branch = v,
                "objective" => j.objective = v,
                "map_name" => j.map_name = v,
                "rung" => j.rung = v,
                "budget_key" => j.budget_key = v,
                "alarm_zero_window_s" => parse_i(&v, k, &mut errs, &mut j.alarms.zero_window_s),
                "alarm_collapse_recent_s" => {
                    parse_i(&v, k, &mut errs, &mut j.alarms.collapse_recent_s)
                }
                "alarm_collapse_baseline_s" => {
                    parse_i(&v, k, &mut errs, &mut j.alarms.collapse_baseline_s)
                }
                "alarm_collapse_frac" => match v.parse() {
                    Ok(f) => j.alarms.collapse_frac = f,
                    Err(_) => errs.push(format!("{k}: {v:?} is not a number")),
                },
                "alarm_no_progress_evals" => match v.parse() {
                    Ok(n) => j.alarms.no_progress_evals = n,
                    Err(_) => errs.push(format!("{k}: {v:?} is not a number")),
                },
                "alarm_box_silence_s" => parse_i(&v, k, &mut errs, &mut j.alarms.box_silence_s),
                "alarm_queue_window_s" => parse_i(&v, k, &mut errs, &mut j.alarms.queue_window_s),
                "alarm_disk_min_free_mb" => {
                    parse_i(&v, k, &mut errs, &mut j.alarms.disk_min_free_mb)
                }
                "alarm_bank_max_gap_s" => parse_i(&v, k, &mut errs, &mut j.alarms.bank_max_gap_s),
                "alarm_start_dev_max_m" => match v.parse() {
                    Ok(f) => j.alarms.start_dev_max_m = f,
                    Err(_) => errs.push(format!("{k}: {v:?} is not a number")),
                },
                "worker_drives" => {
                    j.alarms.worker_drives = matches!(v.as_str(), "yes" | "true" | "1");
                    if !matches!(v.as_str(), "yes" | "true" | "1" | "no" | "false" | "0") {
                        errs.push(format!("{k}: {v:?} is not yes or no"));
                    }
                }
                "max_boxes" => match v.parse() {
                    Ok(n) => j.alarms.max_boxes = n,
                    Err(_) => errs.push(format!("{k}: {v:?} is not a number")),
                },
                "budget_switch_evals" => match v.parse() {
                    Ok(n) => j.budget.switch_evals = n,
                    Err(_) => errs.push(format!("{k}: {v:?} is not a number")),
                },
                "budget_switch_productive_s" => {
                    parse_i(&v, k, &mut errs, &mut j.budget.switch_productive_s)
                }
                "budget_has_switch" => {
                    j.budget.has_switch = matches!(v.as_str(), "yes" | "true" | "1");
                    if !matches!(v.as_str(), "yes" | "true" | "1" | "no" | "false" | "0") {
                        errs.push(format!("{k}: {v:?} is not yes or no"));
                    }
                }
                other => {
                    j.extra.insert(other.to_string(), v);
                }
            }
        }
        if errs.is_empty() {
            Ok(j)
        } else {
            // A config that half-parsed is worse than one that refused: the
            // run would proceed with a threshold nobody set.
            Err(errs.join("; "))
        }
    }

    pub fn load(path: &Path) -> Result<Job, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Job::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn default_text() -> String {
        let d = Job::default();
        format!(
            "# The long-haul job. The supervisor reads this out of the repo, so a
# fresh box needs no instructions from anybody — change it here, commit, and
# the next box picks it up.

# What we are working on, for the status page a human reads.
map_name = {}
rung = {}
objective = {}

# The work. Run with `sh -c` in worker_dir. It must append progress records to
# progress_file: `<iso8601>\\tprogress\\tevals=<cumulative>\\tbest=<objective>`.
worker_cmd =
worker_dir = {}
progress_file = {}

# Cadences, in seconds.
sample_s = {}
bank_s = {}
claim_ttl_s = {}
lease_bank_lead_s = {}
restart_max = {}
restart_backoff_s = {}

# Durability. mirror: paste|dir|none   push: auto|direct|whitestick|none
mirror = {}
push = {}
branch = {}

# Alarms. Every one of these has a test that makes it fire: `tmhaul alarms selftest`.
alarm_zero_window_s = {}
alarm_collapse_recent_s = {}
alarm_collapse_baseline_s = {}
alarm_collapse_frac = {}
alarm_no_progress_evals = {}
alarm_box_silence_s = {}
alarm_queue_window_s = {}
alarm_disk_min_free_mb = {}
alarm_bank_max_gap_s = {}
alarm_start_dev_max_m = {}

# Does this job's worker DRIVE a car? A sweep over already-written tapes does
# not. Defaults to yes, so a worker that says nothing about where its car
# started sets off the alarm — only a line a human committed can switch that
# check off.
worker_drives = yes

# The most boxes this project may hold at once. The heartbeat provisions
# replacements without a human; a bug in that logic must not run away.
max_boxes = {}

# WHICH budget this job spends. The pre-committed switch condition below was
# agreed for the ARCHIVE SEARCH; a job that is not that search gets its own key
# so it cannot spend a threshold the project never meant it to.
budget_key = {}

# Does this budget have a SWITCH, or is it just a meter? The threshold below is
# a pre-committed decision about the ARCHIVE SEARCH; for any other workload it
# is a countdown to a decision nobody can act on. Say no and the numbers are
# still reported, without pretending they mean something.
budget_has_switch = yes

# The pre-committed switch condition (DESIGN.md 3.2). Productive seconds only:
# a stall never spends this.
budget_switch_evals = {}
budget_switch_productive_s = {}
",
            d.map_name,
            d.rung,
            d.objective,
            d.worker_dir,
            d.progress_file,
            d.sample_s,
            d.bank_s,
            d.claim_ttl_s,
            d.lease_bank_lead_s,
            d.restart_max,
            d.restart_backoff_s,
            d.mirror,
            d.push,
            d.branch,
            d.alarms.zero_window_s,
            d.alarms.collapse_recent_s,
            d.alarms.collapse_baseline_s,
            d.alarms.collapse_frac,
            d.alarms.no_progress_evals,
            d.alarms.box_silence_s,
            d.alarms.queue_window_s,
            d.alarms.disk_min_free_mb,
            d.alarms.bank_max_gap_s,
            d.alarms.start_dev_max_m,
            d.alarms.max_boxes,
            d.budget_key,
            d.budget.switch_evals,
            d.budget.switch_productive_s,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_spec_parses_back_to_the_defaults() {
        // Otherwise the file we write on `init` is not the configuration we
        // think we are running.
        let j = Job::parse(&Job::default_text()).unwrap();
        let d = Job::default();
        assert_eq!(j.sample_s, d.sample_s);
        assert_eq!(j.bank_s, d.bank_s);
        assert_eq!(j.alarms, d.alarms);
        assert_eq!(j.budget, d.budget);
        assert_eq!(j.map_name, d.map_name);
    }

    #[test]
    fn a_bad_number_refuses_the_whole_file() {
        let e = Job::parse("sample_s = soon\n").unwrap_err();
        assert!(e.contains("sample_s"), "{e}");
    }

    #[test]
    fn comments_and_blank_lines_are_fine() {
        let j = Job::parse("# a note\n\n  sample_s = 5 # trailing\n").unwrap();
        assert_eq!(j.sample_s, 5);
    }

    #[test]
    fn unknown_keys_are_kept_rather_than_dropped() {
        // A key the current binary does not know is more likely to be a newer
        // config than a typo, and silently dropping it is how a setting stops
        // taking effect with nobody noticing.
        let j = Job::parse("future_knob = 7\n").unwrap();
        assert_eq!(j.extra.get("future_knob").map(String::as_str), Some("7"));
    }
}
