//! One pass over every banked result, and the verdict for each.

use crate::{find_map, is_ghost, map_id, map_name, provenance, published_ms, Provenance};
use haul::log::Log;
use haul::rec::Rec;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// The oracle simulated exactly the millisecond this tape is published at.
    Holds { ms: i64 },
    /// It simulated something else. This is the one that matters.
    Differs { simulated: i64, published: i64 },
    /// It did not finish.
    Dnf { cps: u32, published: Option<i64> },
    /// Simulated, but the name carries no number to check it against.
    Unpublished { ms: i64 },
    /// Simulated, but its car did not start at the map's start line.
    ///
    /// A separate verdict from `Differs`, because it is a different kind of
    /// wrong: the time may be exactly what the tape claims and the run still
    /// not be a run of the map. The position is telemetry-derived — see
    /// `maps::telemetry_start_xz` for what that does and does not establish.
    StartedElsewhere { dev_m: f64, ms: Option<i64> },
    /// Not read, on purpose: a human drove it.
    RefusedHumanGhost,
    /// Could not be measured, with the reason. Never folded in with a pass.
    Unmeasured { why: String },
}

impl Verdict {
    pub fn tag(&self) -> &'static str {
        match self {
            Verdict::Holds { .. } => "holds",
            Verdict::Differs { .. } => "DIFFERS",
            Verdict::Dnf { .. } => "dnf",
            Verdict::Unpublished { .. } => "unpublished",
            Verdict::StartedElsewhere { .. } => "STARTED-ELSEWHERE",
            Verdict::RefusedHumanGhost => "refused-human-ghost",
            Verdict::Unmeasured { .. } => "UNMEASURED",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Verdict::Holds { ms } => haul::time::ms_as_seconds(*ms),
            Verdict::Differs { simulated, published } => format!(
                "simulated {} but published as {}",
                haul::time::ms_as_seconds(*simulated),
                haul::time::ms_as_seconds(*published)
            ),
            Verdict::Dnf { cps, published } => match published {
                Some(p) => format!("DNF at cp{cps}, published as {}", haul::time::ms_as_seconds(*p)),
                None => format!("DNF at cp{cps}"),
            },
            Verdict::Unpublished { ms } => format!("{} (no published number to check)", haul::time::ms_as_seconds(*ms)),
            Verdict::StartedElsewhere { dev_m, ms } => format!(
                "its telemetry starts {dev_m:.1} m from the map's start line{}",
                ms.map(|m| format!(", simulating {}", haul::time::ms_as_seconds(m))).unwrap_or_default()
            ),
            Verdict::RefusedHumanGhost => "a human drove it; not read".to_string(),
            Verdict::Unmeasured { why } => why.clone(),
        }
    }

    /// Is this a result the project can stand behind? `Unmeasured` is
    /// deliberately not `false` — it is neither, and folding it in with either
    /// is how a broken instrument becomes a clean report.
    pub fn is_clean(&self) -> Option<bool> {
        match self {
            Verdict::Holds { .. } | Verdict::Unpublished { .. } => Some(true),
            Verdict::Differs { .. } | Verdict::Dnf { .. } | Verdict::StartedElsewhere { .. } => {
                Some(false)
            }
            Verdict::RefusedHumanGhost | Verdict::Unmeasured { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub map: String,
    pub file: String,
    pub verdict: Verdict,
}

/// Decide a verdict from what the oracle said. Pure, and therefore testable
/// without an engine — which matters, because the interesting cases (a DNF
/// that is published as a finish) are the ones hardest to produce on demand.
pub fn judge(sim_ms: Option<i64>, cps: Option<u32>, published: Option<i64>) -> Verdict {
    match (sim_ms, published) {
        (Some(ms), Some(p)) if ms == p => Verdict::Holds { ms },
        (Some(ms), Some(p)) => Verdict::Differs { simulated: ms, published: p },
        (Some(ms), None) => Verdict::Unpublished { ms },
        (None, p) => Verdict::Dnf { cps: cps.unwrap_or(0), published: p },
    }
}

pub struct Sweep {
    pub repo: PathBuf,
    pub corpus: Vec<PathBuf>,
    pub server: PathBuf,
    pub progress: Option<PathBuf>,
    pub results: Option<PathBuf>,
    pub only_map: Option<String>,
    /// The map registry, for each map's start line. Absent means the check
    /// cannot run — which is reported as UNKNOWN per tape, never skipped.
    pub registry: Vec<crate::maps::MapRow>,
    pub start_dev_max_m: f64,
}

impl Sweep {
    /// Every (map directory, ghost) pair in the repo, in a stable order.
    pub fn inventory(&self) -> Result<Vec<(String, Vec<PathBuf>)>, String> {
        let mut out = Vec::new();
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&self.repo)
            .map_err(|e| format!("{}: {e}", self.repo.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for d in dirs {
            let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
            if map_id(&name).is_none() {
                continue;
            }
            if let Some(only) = &self.only_map {
                if &name != only {
                    continue;
                }
            }
            let replays = d.join("replays");
            if !replays.is_dir() {
                continue;
            }
            let mut ghosts: Vec<PathBuf> = std::fs::read_dir(&replays)
                .map_err(|e| format!("{}: {e}", replays.display()))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| is_ghost(p))
                .collect();
            ghosts.sort();
            if !ghosts.is_empty() {
                out.push((name, ghosts));
            }
        }
        Ok(out)
    }

    /// One full pass. `evals` counts tapes actually put through the oracle.
    pub fn run(&self, start_evals: u64) -> Result<(Vec<Row>, u64), String> {
        let mut rows = Vec::new();
        let mut evals = start_evals;
        let plog = self.progress.as_ref().map(Log::at);
        let rlog = self.results.as_ref().map(Log::at);
        let mut clean = 0u64;

        for (dirname, ghosts) in self.inventory()? {
            let human_name = map_name(&dirname);
            let id = map_id(&dirname).unwrap_or_default();

            // The gate runs before anything is opened.
            let mut allowed: Vec<PathBuf> = Vec::new();
            for g in &ghosts {
                let base = g.file_name().unwrap_or_default().to_string_lossy().to_string();
                match provenance(&base) {
                    Ok(Provenance::Ours) => allowed.push(g.clone()),
                    Ok(Provenance::Human) => rows.push(Row {
                        map: human_name.clone(),
                        file: base,
                        verdict: Verdict::RefusedHumanGhost,
                    }),
                    Err(why) => rows.push(Row {
                        map: human_name.clone(),
                        file: base,
                        verdict: Verdict::Unmeasured { why },
                    }),
                }
            }
            if allowed.is_empty() {
                continue;
            }

            let Some(mapfile) = find_map(&self.corpus, &id) else {
                for g in &allowed {
                    rows.push(Row {
                        map: human_name.clone(),
                        file: g.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        verdict: Verdict::Unmeasured {
                            why: format!("no .Map.Gbx for {id} in the corpus roots given"),
                        },
                    });
                }
                continue;
            };

            let refs: Vec<&Path> = allowed.iter().map(|p| p.as_path()).collect();
            let sims = match ghost::oracle::validate_many(
                &self.server,
                &refs,
                ghost::oracle::MapsMode::One(&mapfile),
                "resim",
            ) {
                Ok(s) => s,
                Err(e) => {
                    // An oracle failure is UNMEASURED for every tape in the
                    // batch. It is never a pass and never a failure.
                    for g in &allowed {
                        rows.push(Row {
                            map: human_name.clone(),
                            file: g.file_name().unwrap_or_default().to_string_lossy().to_string(),
                            verdict: Verdict::Unmeasured { why: format!("oracle: {e}") },
                        });
                    }
                    continue;
                }
            };

            for g in &allowed {
                let base = g.file_name().unwrap_or_default().to_string_lossy().to_string();
                let found = sims.iter().find(|s| s.file == base || base.starts_with(&s.file));
                let mut verdict = match found {
                    None => Verdict::Unmeasured {
                        why: "the server never mentioned this file".to_string(),
                    },
                    Some(s) => judge(s.time_ms, s.cps, published_ms(&base)),
                };
                // A run that did not start at the start line is not a run of
                // this map, whatever time it posts. Checked against the
                // registry's spawn; only overrides a verdict that was
                // otherwise clean, so a DIFFERS stays a DIFFERS.
                if verdict.is_clean() == Some(true) {
                    if let Some(dev) = self.start_dev(&id, g) {
                        if dev > self.start_dev_max_m {
                            let ms = found.and_then(|s| s.time_ms);
                            verdict = Verdict::StartedElsewhere { dev_m: dev, ms };
                        }
                    }
                }
                evals += 1;
                if verdict.is_clean() == Some(true) {
                    clean += 1;
                }
                if let Some(rl) = &rlog {
                    let _ = rl.append(
                        &Rec::new("resim")
                            .f("map", &human_name)
                            .f("file", &base)
                            .f("verdict", verdict.tag())
                            .f("detail", verdict.detail()),
                    );
                }
                if let Some(pl) = &plog {
                    let _ = pl.append(&Rec::new("progress").f("evals", evals).f("best", clean));
                }
                println!("{:<28} {:<48} {:<20} {}", human_name, base, verdict.tag(), verdict.detail());
                rows.push(Row { map: human_name.clone(), file: base, verdict });
            }
        }
        Ok((rows, evals))
    }
}

pub fn summarise(rows: &[Row]) -> String {
    let mut holds = 0;
    let mut differs = 0;
    let mut dnf = 0;
    let mut unpublished = 0;
    let mut refused = 0;
    let mut elsewhere = 0;
    let mut unmeasured = 0;
    for r in rows {
        match r.verdict {
            Verdict::Holds { .. } => holds += 1,
            Verdict::Differs { .. } => differs += 1,
            Verdict::Dnf { .. } => dnf += 1,
            Verdict::Unpublished { .. } => unpublished += 1,
            Verdict::StartedElsewhere { .. } => elsewhere += 1,
            Verdict::RefusedHumanGhost => refused += 1,
            Verdict::Unmeasured { .. } => unmeasured += 1,
        }
    }
    format!(
        "{holds} hold · {differs} differ · {dnf} DNF · {unpublished} unpublished · \
         {elsewhere} STARTED-ELSEWHERE · {refused} refused (human) · {unmeasured} UNMEASURED"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tape_that_does_what_it_says_holds() {
        assert_eq!(judge(Some(12_759), Some(3), Some(12_759)), Verdict::Holds { ms: 12_759 });
    }

    #[test]
    fn a_tape_that_does_something_else_is_caught() {
        assert_eq!(
            judge(Some(12_800), Some(3), Some(12_759)),
            Verdict::Differs { simulated: 12_800, published: 12_759 }
        );
    }

    #[test]
    fn a_dnf_published_as_a_finish_is_not_a_pass() {
        // The failure that would matter most, and the one a careless
        // implementation reports as clean because `None == None`.
        let v = judge(None, Some(5), Some(12_759));
        assert_eq!(v, Verdict::Dnf { cps: 5, published: Some(12_759) });
        assert_eq!(v.is_clean(), Some(false));
    }

    #[test]
    fn unmeasured_is_neither_clean_nor_convicted() {
        assert_eq!(Verdict::Unmeasured { why: "no server".into() }.is_clean(), None);
        assert_eq!(Verdict::RefusedHumanGhost.is_clean(), None);
    }

    #[test]
    fn verdicts_print_times_as_seconds_with_a_decimal() {
        assert_eq!(Verdict::Holds { ms: 23_144 }.detail(), "23.144");
        assert!(Verdict::Differs { simulated: 12_800, published: 12_759 }
            .detail()
            .contains("12.759"));
    }

    #[test]
    fn the_summary_counts_every_bucket_separately() {
        let rows = vec![
            Row { map: "m".into(), file: "a".into(), verdict: Verdict::Holds { ms: 1 } },
            Row { map: "m".into(), file: "b".into(), verdict: Verdict::RefusedHumanGhost },
            Row {
                map: "m".into(),
                file: "c".into(),
                verdict: Verdict::Unmeasured { why: "x".into() },
            },
        ];
        let s = summarise(&rows);
        assert!(s.contains("1 hold"), "{s}");
        assert!(s.contains("1 refused"), "{s}");
        assert!(s.contains("1 UNMEASURED"), "{s}");
    }
}

impl Sweep {
    /// Metres between where this tape's telemetry starts and where the map's
    /// registry row says the start line is. `None` when either half is
    /// unavailable — an absent answer, not a passing one.
    pub fn start_dev(&self, map_id: &str, ghost: &Path) -> Option<f64> {
        let spawn = self
            .registry
            .iter()
            .find(|r| r.id == map_id || r.uid == map_id)
            .and_then(|r| r.spawn)?;
        let start = crate::maps::telemetry_start_xz(ghost).ok()?;
        Some(crate::maps::start_deviation_m(spawn, start))
    }
}
