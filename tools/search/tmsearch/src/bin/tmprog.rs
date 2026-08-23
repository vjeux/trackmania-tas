//! `tmprog` -- enumerate EXPLICIT multi-phase input programs.
//!
//! # Why this exists
//!
//! The search's operators are *perturbations of an incumbent*. They cannot
//! express "turn the car around, then drive back the other way", because every
//! tape on the path between those two behaviours is a crash and the annealer
//! never crosses the valley. A named technique -- a Scandinavian flick, a
//! bug slide, a wiggle -- is a **program**: a short sequence of constant
//! (steer, gas, brake) phases with free durations. Enumerating the family
//! directly is how you find out whether the technique is available at all,
//! and it is the shape this map's earlier arms used to find the dirt ice
//! flick (`imt2_tmprog_v1.rs`, 2026-08-20).
//!
//! This is that tool, ported to the current `tape::Patcher` / `Inputs` API and
//! with one thing it did not have: **`--tail template`**, which hands the wheel
//! back to the template's own inputs after the last phase instead of holding a
//! constant to the end of the tape. A constant tail can only express "do the
//! trick and then coast"; a template tail expresses "do the trick and then
//! drive the rest of the lap the way we already know how", which is what a
//! route change in the MIDDLE of a lap needs.
//!
//! # Usage
//!
//! ```text
//! tmprog --template G.Ghost.Gbx --out DIR [--start TICK] [--limit N]
//!        --p "steer/gas/brake/dur" [--p ...]      (repeatable, in order)
//!        [--tail "steer/gas/brake" | --tail template]
//! ```
//!
//! Every field of a `--p` is a comma-separated list and the full cross-product
//! is enumerated. `dur` is in ticks (10 ms). Ticks before `--start` keep the
//! template's own inputs. `index.tsv` records every candidate's whole parameter
//! vector, so a hit can be read back to the program that produced it.
//!
//! # The trap this tool has, stated
//!
//! A candidate here is written by *bit-patching the template*, so a tick whose
//! packet has no 8-bit steering field cannot be written -- `Patcher` names
//! those and this binary **refuses** rather than silently dropping the edit,
//! which is the defect `SEARCH.md` §2 records against the old writer.

use forkoracle::inputs::Inputs;
use std::path::PathBuf;
use tmsearch::tape::Patcher;

fn flag(a: &[String], n: &str) -> Option<String> {
    a.iter().position(|x| x == n).and_then(|i| a.get(i + 1)).cloned()
}

fn flags(a: &[String], n: &str) -> Vec<String> {
    let mut v = Vec::new();
    for (i, x) in a.iter().enumerate() {
        if x == n {
            if let Some(y) = a.get(i + 1) {
                v.push(y.clone());
            }
        }
    }
    v
}

fn nums(s: &str) -> Vec<i64> {
    s.split(',')
        .filter(|x| !x.trim().is_empty())
        .map(|x| x.trim().parse().unwrap_or_else(|_| panic!("not a number: {x:?}")))
        .collect()
}

#[derive(Clone)]
struct Phase {
    steer: Vec<i64>,
    gas: Vec<i64>,
    brake: Vec<i64>,
    dur: Vec<i64>,
}

fn parse_phase(s: &str) -> Phase {
    let f: Vec<&str> = s.split('/').collect();
    assert_eq!(f.len(), 4, "--p wants steer/gas/brake/dur, got {s:?}");
    let p = Phase { steer: nums(f[0]), gas: nums(f[1]), brake: nums(f[2]), dur: nums(f[3]) };
    for v in [&p.steer, &p.gas, &p.brake, &p.dur] {
        assert!(!v.is_empty(), "empty list in --p {s:?}");
    }
    for &s in &p.steer {
        assert!((-128..=127).contains(&s), "steer {s} is not an i8");
    }
    p
}

#[derive(Clone, Copy)]
enum Tail {
    Hold(i64, i64, i64),
    Template,
    /// Rejoin the template's inputs, but `advance` ticks EARLIER in the
    /// template's own clock: tick `i` of the candidate takes the template's
    /// tick `i + advance`.
    ///
    /// This is the tail a shortcut needs, and `Template` is the wrong one for
    /// it. A trick whose whole point is to reach a place SOONER hands back to a
    /// reference that is, at that moment, still somewhere behind — so replaying
    /// the reference's inputs at the same clock replays the inputs for a car
    /// that has not arrived yet. Aligning by POSITION rather than by clock is
    /// what makes the rest of a known-good lap reusable, and the advance is the
    /// time the trick saved. The tape is padded at the end by holding the
    /// template's last tick, so the candidate is still `n` ticks long.
    Rejoin { advance: usize },
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 || a.iter().any(|x| x == "--help" || x == "-h") {
        eprint!("{}", HELP);
        return;
    }
    let template = flag(&a, "--template").expect("--template G.Ghost.Gbx");
    let out = PathBuf::from(flag(&a, "--out").expect("--out DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let start: usize = flag(&a, "--start").map(|s| s.parse().unwrap()).unwrap_or(0);
    let limit: usize = flag(&a, "--limit").map(|s| s.parse().unwrap()).unwrap_or(usize::MAX);
    let phases: Vec<Phase> = flags(&a, "--p").iter().map(|s| parse_phase(s)).collect();
    assert!(!phases.is_empty(), "at least one --p");
    let tail = match flag(&a, "--tail").as_deref() {
        None => Tail::Hold(0, 1, 0),
        Some("template") => Tail::Template,
        Some(s) if s.starts_with("rejoin:") => {
            Tail::Rejoin { advance: s[7..].parse().expect("--tail rejoin:N wants a tick count") }
        }
        Some(s) => {
            let v = nums(&s.replace('/', ","));
            assert_eq!(
                v.len(),
                3,
                "--tail wants steer/gas/brake, `template`, or `rejoin:N`"
            );
            Tail::Hold(v[0], v[1], v[2])
        }
    };

    let pat = Patcher::build(&template).unwrap_or_else(|e| panic!("{template}: {e}"));
    let n = pat.n();

    // The furthest tick any program in the family can write, so an unwritable
    // slot inside the family's reach is refused BEFORE anything is generated.
    let mut reach = start;
    for p in &phases {
        reach += *p.dur.iter().max().unwrap() as usize;
    }
    let reach = reach.min(n);
    let hi = match tail {
        // `Template` leaves the tail bytes exactly as the template had them, so
        // nothing past the last phase is written and an unwritable slot there
        // is harmless. Every other tail writes to the end of the tape.
        Tail::Template => reach,
        Tail::Hold(..) | Tail::Rejoin { .. } => n,
    };
    if let Err(e) = pat.check_window(start, hi) {
        panic!("this family would write a tick the codec cannot express: {e}");
    }

    let base: Inputs = pat.template.clone();

    // the cross product as a mixed-radix counter
    let mut radix: Vec<usize> = Vec::new();
    for p in &phases {
        radix.push(p.steer.len());
        radix.push(p.gas.len());
        radix.push(p.brake.len());
        radix.push(p.dur.len());
    }
    let total: usize = radix.iter().product();

    let mut idx = String::from("file");
    for i in 0..phases.len() {
        idx.push_str(&format!("\ts{i}\tg{i}\tb{i}\td{i}"));
    }
    idx.push_str("\tend\n");

    let mut k = 0usize;
    for code in 0..total {
        if k >= limit {
            break;
        }
        let mut c = code;
        let mut pick: Vec<usize> = Vec::with_capacity(radix.len());
        for &r in &radix {
            pick.push(c % r);
            c /= r;
        }
        let mut st = base.clone();
        let mut t = start;
        let mut row = String::new();
        for (pi, p) in phases.iter().enumerate() {
            let s = p.steer[pick[pi * 4]];
            let g = p.gas[pick[pi * 4 + 1]];
            let b = p.brake[pick[pi * 4 + 2]];
            let d = p.dur[pick[pi * 4 + 3]] as usize;
            row.push_str(&format!("\t{s}\t{g}\t{b}\t{d}"));
            let end = (t + d).min(n);
            for i in t..end {
                st.steer[i] = s as i8;
                st.gas[i] = g != 0;
                st.brake[i] = b != 0;
            }
            t = end;
        }
        if let Tail::Hold(s, g, b) = tail {
            for i in t..n {
                st.steer[i] = s as i8;
                st.gas[i] = g != 0;
                st.brake[i] = b != 0;
            }
        }
        if let Tail::Rejoin { advance } = tail {
            for i in t..n {
                let j = (i + advance).min(n - 1);
                st.steer[i] = base.steer[j];
                st.gas[i] = base.gas[j];
                st.brake[i] = base.brake[j];
            }
        }
        // Tail::Template needs no work: `st` started as the template's inputs
        // and only the phase ticks were overwritten.

        let name = format!("p{k:06}.Ghost.Gbx");
        std::fs::write(out.join(&name), pat.file(&st)).unwrap();
        idx.push_str(&name);
        idx.push_str(&row);
        idx.push_str(&format!("\t{t}\n"));
        k += 1;
    }
    std::fs::write(out.join("index.tsv"), idx).unwrap();
    let tailname = match tail {
        Tail::Template => "template".to_string(),
        Tail::Hold(s, g, b) => format!("{s}/{g}/{b}"),
        Tail::Rejoin { advance } => format!("rejoin:{advance}"),
    };
    println!(
        "wrote {k} candidates of {total} ({n} ticks each, start tick {start}, tail {tailname}) to {}",
        out.display()
    );
}

const HELP: &str = "\
tmprog -- enumerate EXPLICIT multi-phase input programs.

  tmprog --template G.Ghost.Gbx --out DIR [--start TICK] [--limit N]
         --p \"steer/gas/brake/dur\" [--p ...]
         [--tail \"steer/gas/brake\" | --tail template]

Every --p field is a comma-separated list; the cross-product is enumerated.
`dur` is in ticks (10 ms). Ticks before --start keep the template's inputs.

  --tail template   after the last phase, hand back to the TEMPLATE's own
                    inputs. Use this for a route change in the middle of a
                    lap, where the rest of the run should still be driven.
  --tail s/g/b      hold one triple to the end of the tape (default 0/1/0).
  --tail rejoin:N   rejoin the template N ticks EARLIER in its own clock:
                    candidate tick i takes template tick i+N. This is the
                    tail a SHORTCUT needs -- a trick that reaches a place
                    sooner must rejoin the reference by POSITION, not by
                    clock, and N is the time the trick saved.

index.tsv records each candidate's whole parameter vector.
";
