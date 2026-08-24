//! `tmauto evalbug` — the two-line experiment behind the claim in
//! [`tmauto::oracle::evaluate_declared`]'s doc comment.
//!
//! That comment says `oracle::evaluate` truncates every candidate at race
//! 2.500 because it builds its container from `GhostMeta::probe`, whose
//! `declared_ms` is 0. **A statement of that shape is exactly the kind this
//! project has been burned by**: it is a tidy causal story about someone
//! else's code, and the rule is to cite it or measure it. So it is measured,
//! here, and the comment quotes this command's output rather than the other way
//! round.
//!
//! Same tape, same map, same box, same batch. One field.

use std::path::PathBuf;
use tmauto::oracle;

fn secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}
fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let map = PathBuf::from(arg(args, "--map").ok_or("--map is required")?);
    let apath = PathBuf::from(arg(args, "--artifact").ok_or("--artifact is required")?);
    let work = PathBuf::from(arg(args, "--work").unwrap_or_else(|| "/tmp/c2/evalbug".into()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

    let (h, inputs) = crate::artifact::read_artifact(&apath)?;
    let tapes = vec![inputs];
    let n = tapes[0].len();

    let a = oracle::evaluate(&map, &tapes, n, 1, &work.join("a"))?;
    let b = oracle::evaluate_declared(&map, &tapes, n, 1, 600, h.declared_ms, &work.join("b"))?;

    println!("tape          {} ({} ticks)", apath.display(), n);
    println!("map           {}", map.display());
    println!();
    println!("evaluate           (declared_ms = 0, from GhostMeta::probe)  -> {:?}", a[0].as_ref().map(|e| e.verdict));
    println!("evaluate_declared  (declared_ms = {})                    -> {:?}", h.declared_ms, b[0].as_ref().map(|e| e.verdict));
    println!();

    match (a[0].as_ref().map(|e| e.verdict), b[0].as_ref().map(|e| e.verdict)) {
        (Some(tmauto::verdict::Verdict::Finish { .. }), _) => {
            println!(
                "  NO DEFECT FOUND. `evaluate` reported a finish, so the declared time is not \
                 truncating this run and the doc comment claiming it does is WRONG and must be \
                 removed."
            );
        }
        (x, Some(tmauto::verdict::Verdict::Finish { ms })) => {
            println!(
                "  CONFIRMED. The same tape finishes in {} with a stated declared time and \
                 comes back {:?} without one. `oracle::evaluate` cuts every candidate off at \
                 race 2.500, and on a candidate that could have finished that is \
                 indistinguishable from bad driving.",
                secs(ms),
                x
            );
        }
        (_, y) => {
            println!(
                "  UNMEASURED. Neither path finished ({:?}); this experiment cannot say \
                 anything about the declared time. Do not quote it.",
                y
            );
        }
    }
    Ok(())
}
