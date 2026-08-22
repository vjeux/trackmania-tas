//! Where the fork evaluator's reference line comes from.
//!
//! Progress -- the only thing an aborted candidate can be ranked by -- is
//! arclength along a line, and `offref` is distance from it. So the line has to
//! be the line the incumbent actually drove.
//!
//! # The trap this module exists to make unrepresentable
//!
//! **A synthesised tape carries its TEMPLATE's telemetry.** A search output is
//! the seed file with its input bits patched: the recorded trajectory inside it
//! still belongs to the seed, byte for byte, and reading it back describes a
//! run that never happened. Taking the reference line from such a file measures
//! every candidate against the wrong line, silently, and the search then
//! optimises towards a trajectory nothing in this run ever drove.
//!
//! There are two honest sources:
//!
//! * `--refghost FILE` -- the file's own telemetry, accepted only after the
//!   real engine has re-simulated that file's own tape and produced the same
//!   trajectory. That is a decisive test: it compares the file's claim against
//!   the world, not against a statistic.
//! * `--refcsv FILE` -- a trajectory measured out of the engine (`fk btraj`),
//!   which is the only source for a tape nobody has ever driven.
//!
//! # A cheaper test that did not work, and the measurement that says so
//!
//! A ghost holds its driver's inputs TWICE -- the 10 ms input chunk and byte 14
//! of every 50 ms telemetry sample -- and `ghost::verify::tape_record_agreement`
//! scores their agreement as chance-corrected Cohen's kappa. The published
//! separation is 1.000 for a recording of its own run against 0.120 for a
//! wholesale-contaminated file, which looks like a free gate.
//!
//! It is not. **Measured here on `human_23013.Ghost.Gbx`, a plain game
//! recording: kappa 0.919 over 461 samples** -- while search tapes that differ
//! from their template by a few per cent of ticks score around 0.83. The two
//! populations are 0.09 apart with a sample of one on the good side, so no
//! threshold on this statistic separates them. It is reported as context and
//! it decides nothing.

use forkoracle::pred::RefLineData;

/// How far the engine's own re-simulation of a file's tape may sit from the
/// trajectory that file records, in metres, before the recording is refused as
/// belonging to a different run.
///
/// A file whose telemetry is its own scores under a millimetre (0.0005 m mean
/// on the project's fixture). A tape carrying its template's trajectory is
/// wrong by whole metres from the first place the two runs diverge. There is no
/// middle ground to calibrate against, so this is set two orders of magnitude
/// above the good case rather than halfway between two populations.
pub const MAX_MEAN_ERROR_M: f64 = 0.05;

pub struct FromGhost {
    pub line: RefLineData,
    /// Mean distance between the recorded trajectory and the engine's own
    /// re-simulation of the same tape.
    pub engine_error_m: f64,
    pub kappa: f64,
    pub samples: usize,
}

/// Build the reference line out of a ghost's own recorded trajectory, after
/// proving that trajectory is what its tape produces.
pub fn from_ghost(
    path: &str,
    map: &str,
    start_offset_ms: i32,
    nticks: usize,
) -> Result<FromGhost, String> {
    let kappa = ghost::verify::tape_record_agreement(path).map(|t| t.0).unwrap_or(f64::NAN);

    let (mean, worst, n, shifted) = ghost::regen::engine_trajectory_agreement(path, map)
        .map_err(|e| {
            format!(
                "{}: could not check whether this file's telemetry is its own ({}). \
                 Measure the line with `fk btraj` and pass --refcsv.",
                path, e
            )
        })?;
    if shifted {
        return Err(format!(
            "{}: its telemetry is a whole sample out of step with what its tape produces \
             (mean {:.4} m, and the neighbouring sample fits better). A one-tick offset is a \
             pure time shift, so it hides inside a small mean and corrupts exactly the \
             frame-synchronous comparison a reference line is.",
            path, mean
        ));
    }
    if mean > MAX_MEAN_ERROR_M {
        return Err(format!(
            "{}: its telemetry is not what its tape produces -- the engine's own run of this \
             file's inputs is {:.3} m from the recorded trajectory on average ({:.3} m at worst, \
             {} samples). This is what a search output looks like: the recording belongs to the \
             template it was patched from. Measure the line with `fk btraj` and pass --refcsv.",
            path, mean, worst, n
        ));
    }

    let d = tmtraj::entrec::decode_ghost(path).map_err(|e| format!("{}: {}", path, e))?;
    let rows: Vec<(i64, f64, f64, f64)> =
        d.samples.iter().map(|s| (s.time_ms as i64, s.x, s.y, s.z)).collect();
    let line = RefLineData::from_samples(&rows, start_offset_ms, nticks)
        .map_err(|e| format!("{}: {}", path, e))?;
    Ok(FromGhost { line, engine_error_m: mean, kappa, samples: rows.len() })
}
