//! `ghost tape shifts` — the +/- k-tick BOUNDARY-SHIFT family of a tape.
//!
//! The question behind it: how fragile is this run? Take every input event of
//! the template (every tick where steer / accel / brake changes), move that one
//! boundary by -k..+k ticks (10 ms a tick), leave everything else identical,
//! and ask the dedicated server which of those neighbours still finish, and in
//! what time. A run with no slack -- a landing 3 m from the pad edge -- loses
//! every neighbour; a comfortable one keeps most of them. On the bounty run of
//! September 2026 the field's ghosts kept 3-52% of their +/-10 ms neighbours
//! and the record kept 0 of 26, which is exactly what a 3-metre margin
//! predicts and is therefore NOT evidence of anything (`forensics/README.md`).
//!
//! ```text
//!   ghost tape shifts --template RUN.Ghost.Gbx --out DIR --map MAP.Map.Gbx
//!                     [--server DIR] [--max-shift 1] [--until-ms MS]
//!                     [--no-validate] [--stations NAME=MAP.Map.Gbx,...]
//! ```
//!
//! Every candidate is one whole file in DIR (`IDENT.Ghost.Gbx` is the
//! template re-encoded explicitly, the control), and the whole family is
//! validated in ONE server launch, because the launch is what costs. The
//! optional station ladder validates the family against relocated-finish
//! copies of the map (`NAME=MAP`, in track order) and reports, per candidate,
//! the last station it reached -- WHERE each perturbation died, not only that
//! it did.

use std::path::{Path, PathBuf};

use crate::cli::{die, flag, need};
use crate::oracle::{self, MapsMode, SimResult};
use gbx::container::Container;
use gbx::tape::{Encoding, Tape};

/// One input event of the template: the first tick carrying the new inputs.
struct Event {
    idx: usize,
    tick: usize,
    race_ms: i64,
    what: String,
}

fn channels(t: &Tape) -> Vec<(i8, u8, u8)> {
    let s = t.steer_i8s();
    let a = t.accels();
    let b = t.brakes();
    (0..s.len()).map(|i| (s[i], a[i], b[i])).collect()
}

fn describe(from: (i8, u8, u8), to: (i8, u8, u8)) -> String {
    let mut v = Vec::new();
    if from.0 != to.0 {
        v.push(format!("steer {}->{}", from.0, to.0));
    }
    if from.1 != to.1 {
        v.push(format!("accel {}->{}", from.1, to.1));
    }
    if from.2 != to.2 {
        v.push(format!("brake {}->{}", from.2, to.2));
    }
    v.join(", ")
}

/// The template's events, in tick order, optionally only those before `until_ms`.
fn events(t: &Tape, until_ms: Option<i64>) -> Vec<Event> {
    let ch = channels(t);
    let mut out = Vec::new();
    for i in 1..ch.len() {
        if ch[i] != ch[i - 1] {
            let ms = t.race_ms(i);
            if let Some(u) = until_ms {
                if ms > u {
                    break;
                }
            }
            out.push(Event { idx: out.len(), tick: i, race_ms: ms, what: describe(ch[i - 1], ch[i]) });
        }
    }
    out
}

/// A copy of the tape with ONE boundary moved by `d` ticks.
///
/// `d < 0`: the new inputs start `|d|` ticks earlier (ticks `t+d..t` take the
/// post-event packet's vehicle fields). `d > 0`: they start `d` ticks later
/// (ticks `t..t+d` keep the pre-event packet's). The vehicle fields are copied
/// RAW from the neighbouring packet -- steer keeps whatever width the tape's
/// mode uses -- so the only thing that changes is which tick the change lands on.
fn shifted(t: &Tape, tick: usize, d: i64) -> Option<Tape> {
    let mut out = t.clone();
    let a = out.archives.first_mut()?;
    let n = a.packets.len();
    let (lo, hi, src) = if d < 0 {
        let lo = tick as i64 + d;
        if lo < 1 {
            return None;
        }
        (lo as usize, tick, tick)
    } else {
        let hi = tick as i64 + d;
        if hi as usize >= n {
            return None;
        }
        (tick, hi as usize, tick - 1)
    };
    let (steer, accel, brake) = (a.packets[src].steer, a.packets[src].accel, a.packets[src].brake);
    for i in lo..hi {
        let p = &mut a.packets[i];
        p.steer = steer;
        p.accel = accel;
        p.brake = brake;
        p.vsame = false;
    }
    Some(out)
}

struct Candidate {
    name: String,
    path: PathBuf,
    event: usize,
    d: i64,
}

fn outcome(r: Option<&SimResult>, ident_ms: Option<i64>) -> String {
    match r {
        None => "no answer from the server".into(),
        Some(r) => match r.time_ms {
            Some(t) => {
                let delta = ident_ms.map(|i| format!(" ({:+.3} s vs IDENT)", (t - i) as f64 / 1000.0)).unwrap_or_default();
                format!("FINISH {}{}", gbx::container::secs(t), delta)
            }
            None => format!("DNF cps {}{}", r.cps.map(|c| c.to_string()).unwrap_or("?".into()), if r.desc.is_empty() { String::new() } else { format!(" -- {}", r.desc) }),
        },
    }
}

/// Results keyed by the base name the server used (see `oracle::validate_many`).
fn by_name(res: &[SimResult]) -> std::collections::BTreeMap<String, &SimResult> {
    let mut m = std::collections::BTreeMap::new();
    for r in res {
        // the server may prefix a duplicate base name with "<i>_"; our names are unique
        let key = r.file.rsplit('/').next().unwrap_or(&r.file).to_string();
        m.insert(key, r);
    }
    m
}

fn lookup<'a>(m: &'a std::collections::BTreeMap<String, &'a SimResult>, name: &str) -> Option<&'a SimResult> {
    m.get(name).copied().or_else(|| m.iter().find(|(k, _)| k.ends_with(name)).map(|(_, v)| *v))
}

pub fn cmd(a: &[String]) {
    let template = need(a, "--template");
    let out = PathBuf::from(need(a, "--out"));
    let max_shift: i64 = flag(a, "--max-shift").and_then(|s| s.parse().ok()).unwrap_or(1);
    let until_ms: Option<i64> = flag(a, "--until-ms").and_then(|s| s.parse().ok());
    let validate = !a.iter().any(|x| x == "--no-validate");
    if max_shift < 1 {
        die("--max-shift must be >= 1");
    }
    let c = Container::load(template).unwrap_or_else(|e| die(format!("{}: {}", template, e)));
    let tape = Tape::from_file(template).unwrap_or_else(|e| die(format!("{}: {}", template, e)));
    let declared = c.declared_times().first().map(|(_, v)| *v as i64);
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(format!("{}: {}", out.display(), e)));

    let evs = events(&tape, until_ms);
    if evs.is_empty() {
        die("the template has no input events in the requested window");
    }
    println!(
        "template {}: {} ticks, start offset {} ms, declared {}; {} events{}; shifts +/-{} tick(s)",
        template,
        tape.n(),
        tape.archives[0].start_offset_ms,
        declared.map(gbx::container::secs).unwrap_or("-".into()),
        evs.len(),
        until_ms.map(|u| format!(" up to race {} ms", u)).unwrap_or_default(),
        max_shift
    );

    // IDENT: the template re-encoded explicitly -- the control every candidate is measured against
    let ident = out.join("IDENT.Ghost.Gbx");
    let bytes = tape.inject_into(&c, Encoding::Explicit).unwrap_or_else(|e| die(format!("IDENT: {}", e)));
    std::fs::write(&ident, &bytes).unwrap_or_else(|e| die(format!("{}: {}", ident.display(), e)));

    let mut cands: Vec<Candidate> = Vec::new();
    for e in &evs {
        for d in -max_shift..=max_shift {
            if d == 0 {
                continue;
            }
            let Some(t2) = shifted(&tape, e.tick, d) else { continue };
            let name = format!("e{:02}_t{}_{}{}.Ghost.Gbx", e.idx, e.tick, if d < 0 { "m" } else { "p" }, d.abs());
            let p = out.join(&name);
            let b = t2.inject_into(&c, Encoding::Explicit).unwrap_or_else(|err| die(format!("{}: {}", name, err)));
            std::fs::write(&p, &b).unwrap_or_else(|err| die(format!("{}: {}", p.display(), err)));
            cands.push(Candidate { name, path: p, event: e.idx, d });
        }
    }
    println!("wrote IDENT + {} candidates to {}", cands.len(), out.display());
    for e in &evs {
        println!("  e{:02}  tick {:>5}  race {:>8.3} s  {}", e.idx, e.tick, e.race_ms as f64 / 1000.0, e.what);
    }
    if !validate {
        return;
    }

    let map = Path::new(need(a, "--map"));
    let server = oracle::server_dir(flag(a, "--server"));
    let mut files: Vec<&Path> = vec![ident.as_path()];
    files.extend(cands.iter().map(|c| c.path.as_path()));
    let res = oracle::validate_many(&server, &files, MapsMode::One(map), "shifts").unwrap_or_else(|e| die(format!("validator: {}", e)));
    let m = by_name(&res);
    let ident_res = lookup(&m, "IDENT.Ghost.Gbx");
    let ident_ms = ident_res.and_then(|r| r.time_ms);
    println!("\nIDENT: {}{}", outcome(ident_res, None), match (ident_ms, declared) {
        (Some(t), Some(dcl)) if t == dcl => "  == declared".to_string(),
        (Some(_), Some(dcl)) => format!("  != declared {} -- the template itself does not re-simulate; the family below is measured against a broken control", gbx::container::secs(dcl)),
        _ => String::new(),
    });

    println!("\nPER EVENT (finishing shifts / shifts tried):");
    let mut fin_total = 0usize;
    for e in &evs {
        let mine: Vec<&Candidate> = cands.iter().filter(|c| c.event == e.idx).collect();
        let fin = mine.iter().filter(|c| lookup(&m, &c.name).and_then(|r| r.time_ms).is_some()).count();
        fin_total += fin;
        println!("  e{:02}  race {:>8.3} s  {:<28} {}/{}", e.idx, e.race_ms as f64 / 1000.0, e.what, fin, mine.len());
        for c in &mine {
            println!("        {:+2} tick  {}", c.d, outcome(lookup(&m, &c.name), ident_ms));
        }
    }
    println!("\nSURVIVAL: {} of {} neighbours finish ({:.0}%)", fin_total, cands.len(), if cands.is_empty() { 0.0 } else { 100.0 * fin_total as f64 / cands.len() as f64 });

    // the station ladder
    if let Some(st) = flag(a, "--stations") {
        let stations: Vec<(String, PathBuf)> = st
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let (n, p) = s.split_once('=').unwrap_or_else(|| die(format!("--stations wants NAME=MAP, got {:?}", s)));
                (n.to_string(), PathBuf::from(p))
            })
            .collect();
        println!("\nSTATION LADDER ({} stations, in the order given):", stations.len());
        let mut reached: Vec<Vec<bool>> = vec![vec![false; stations.len()]; cands.len() + 1];
        for (si, (name, mp)) in stations.iter().enumerate() {
            let r = oracle::validate_many(&server, &files, MapsMode::One(mp), &format!("station{}", si)).unwrap_or_else(|e| die(format!("station {}: {}", name, e)));
            let sm = by_name(&r);
            reached[0][si] = lookup(&sm, "IDENT.Ghost.Gbx").and_then(|x| x.time_ms).is_some();
            for (ci, c) in cands.iter().enumerate() {
                reached[ci + 1][si] = lookup(&sm, &c.name).and_then(|x| x.time_ms).is_some();
            }
        }
        let header: Vec<String> = stations.iter().map(|(n, _)| format!("{:>8}", n.chars().take(8).collect::<String>())).collect();
        println!("  {:<24} {}  last station reached", "candidate", header.join(" "));
        let row = |name: &str, r: &[bool]| {
            let cells: Vec<String> = r.iter().map(|b| format!("{:>8}", if *b { "yes" } else { "-" })).collect();
            let last = r.iter().rposition(|b| *b).map(|i| stations[i].0.clone()).unwrap_or("(none)".into());
            println!("  {:<24} {}  {}", name, cells.join(" "), last);
        };
        row("IDENT", &reached[0]);
        for (ci, c) in cands.iter().enumerate() {
            row(c.name.trim_end_matches(".Ghost.Gbx"), &reached[ci + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_names_every_changed_channel() {
        assert_eq!(describe((0, 1, 0), (-127, 1, 0)), "steer 0->-127");
        assert_eq!(describe((0, 1, 0), (0, 0, 1)), "accel 1->0, brake 0->1");
        assert_eq!(describe((5, 1, 0), (5, 1, 0)), "");
    }
}
