//! Build candidate ghosts directly, without a shell loop and without a
//! round trip through a text tape per candidate.
//!
//! One process reads the base ghost once, then writes N candidates. Two
//! reasons this is a subcommand rather than a loop over `ghost tape
//! inject`: the base tape is parsed once instead of N times, and the edit
//! that defines each candidate is recorded in the file name, so a result can
//! never be attributed to the wrong candidate.
//!
//! Every candidate is checked the way `pkz2 edit` checks one: a window that
//! changed nothing is a candidate that is not what its name says, and it is
//! reported and skipped rather than written.

use gbx::container::{self, Container};
use gbx::tape::{Encoding, StateEnc, Tape};

pub struct Spec {
    pub name: String,
    pub edits: Vec<crate::edit::Edit>,
}

fn apply(t: &mut Tape, start_offset_ms: i64, edits: &[crate::edit::Edit]) -> Vec<usize> {
    let mut changed = vec![0usize; edits.len()];
    let a = &mut t.archives[0];
    for (i, p) in a.packets.iter_mut().enumerate() {
        let race_ms = i as i64 * 10 + start_offset_ms;
        for (k, e) in edits.iter().enumerate() {
            if race_ms < e.from_ms || race_ms >= e.to_ms {
                continue;
            }
            match e.chan.as_str() {
                "steer" => {
                    let v = (e.val as i8) as u8 as u32;
                    if p.steer != v || p.vsame {
                        p.steer = v;
                        p.vsame = false;
                        changed[k] += 1;
                    }
                }
                "accel" => {
                    let v = e.val as u32;
                    if p.accel != v || p.vsame {
                        p.accel = v;
                        p.vsame = false;
                        changed[k] += 1;
                    }
                }
                "brake" => {
                    let v = e.val as u32;
                    if p.brake != v || p.vsame {
                        p.brake = v;
                        p.vsame = false;
                        changed[k] += 1;
                    }
                }
                "respawn" => {
                    // Bit 31 of the state literal. A repeated word has no
                    // literal, so it is turned into the one the decoder
                    // derives from it -- the same rewrite `ghost tape expand
                    // --state` does for a whole tape.
                    let lit = match p.state {
                        StateEnc::Lit(l) => l,
                        _ => gbx::tape::literal_for(p.word0, p.flags),
                    };
                    let nl = if e.val != 0 { lit | (1u64 << 31) } else { lit & !(1u64 << 31) };
                    if nl != lit || !matches!(p.state, StateEnc::Lit(_)) {
                        changed[k] += 1;
                    }
                    p.state = StateEnc::Lit(nl);
                }
                _ => {}
            }
        }
    }
    changed
}

pub fn run(base: &str, outdir: &str, specs: &[Spec]) -> Result<usize, String> {
    let c = Container::load(base)?;
    let body = c.body().to_vec();
    let base_tape = Tape::from_file(base)?;
    let start_offset_ms = base_tape.archives[0].start_offset_ms as i64;
    std::fs::create_dir_all(outdir).map_err(|e| e.to_string())?;
    let mut n = 0;
    for s in specs {
        let mut t = base_tape.clone();
        let changed = apply(&mut t, start_offset_ms, &s.edits);
        if changed.iter().sum::<usize>() == 0 {
            eprintln!("skip {}: the whole candidate is a no-op against its base", s.name);
            continue;
        }
        if let Some(k) = changed.iter().enumerate().position(|(j, c)| *c == 0 && !s.edits[j].optional) {
            eprintln!(
                "skip {}: window {}:{}:{}:{} changed nothing",
                s.name, s.edits[k].from_ms, s.edits[k].to_ms, s.edits[k].chan, s.edits[k].val
            );
            continue;
        }
        let nb = t.splice_into(&body, Encoding::Explicit)?;
        let out = format!("{}/{}.Ghost.Gbx", outdir, s.name);
        container::write_gbx(&c.gbx, nb, &out)?;
        n += 1;
    }
    Ok(n)
}
