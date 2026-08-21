//! `census` -- the reference-free container check.
//!
//! A synthesised tape is built inside a *carrier* -- somebody else's ghost --
//! and the search overwrites the carrier's telemetry with our own. What it does
//! not overwrite is everything the container says about itself, and the
//! cheapest of those to read is the declared time: it is stored several times
//! over as a little-endian u32, so a file can be asked, with no reference
//! recording and no server, whether it carries its own time and nobody else's.
//!
//! The rule this implements, from `DO-NOT-FILM.md`:
//!
//! > N copies of its OWN validated time and ZERO of any other time.
//!
//! Both halves matter and neither is sufficient. A census that asks only
//! "does it contain a foreign time" calls a partially-rewritten container
//! foreign; one that asks only "does it contain its own" calls the same file
//! clean. Eighteen published files hold both answers at once, so this prints
//! both counts and refuses to collapse them into one verdict.
//!
//! Two stated limits, because a clean census is not a clearance:
//!
//!  * It reads the time **as a u32 only**. A value stored as a float, or
//!    split across a struct boundary, is invisible to it.
//!  * It cannot see the nickname, the login, the account id, the skin path or
//!    the split list. Those need the game, the server, and a string read
//!    respectively -- four readers, four fields, none subsuming another.
//!
//! Usage:
//!
//! ```text
//! census GHOST.Gbx --own MS [--other MS]... [--min-count N] [--offsets]
//! ```
//!
//! `--own` is the file's VALIDATED time, not its filename and not its header.
//! `--other` names a time you already suspect -- the human record, the author
//! time, a neighbouring row on the leaderboard -- and is reported explicitly
//! even at a count of zero, because a named zero is evidence and an unnamed
//! one is silence.
//!
//! Everything else in the plausible-time band that occurs at least
//! `--min-count` times (default 2) is listed too: a repeated exact u32 is
//! structural, whereas float noise repeats by accident about never. Isolated
//! hits are suppressed rather than hidden -- the count of them is printed.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::process::exit;

use tmtraj::gbx::Gbx;

/// The band a race time can plausibly live in: half a second to an hour.
/// Below it every small integer in the file is a candidate; above it, nothing
/// in this corpus is a lap.
const T_MIN: u32 = 500;
const T_MAX: u32 = 3_600_000;

struct Region {
    name: &'static str,
    bytes: Vec<u8>,
}

/// Every little-endian u32 in the plausible band, by value, with the region
/// and offset each copy sits at. Overlapping alignments are all scanned: the
/// stride between the sites is structural, not four-byte aligned by promise.
fn scan(regions: &[Region]) -> BTreeMap<u32, Vec<(&'static str, usize)>> {
    let mut hits: BTreeMap<u32, Vec<(&'static str, usize)>> = BTreeMap::new();
    for reg in regions {
        let b = &reg.bytes;
        if b.len() < 4 {
            continue;
        }
        for off in 0..=(b.len() - 4) {
            let v = u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]);
            if (T_MIN..=T_MAX).contains(&v) {
                hits.entry(v).or_default().push((reg.name, off));
            }
        }
    }
    hits
}

fn fmt_secs(ms: u32) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

fn usage() -> ! {
    eprintln!(
        "usage: census GHOST.Gbx --own MS [--other MS]... [--min-count N] [--offsets]\n\
         \n\
         Counts little-endian u32 copies of the declared time in a ghost's\n\
         container. --own is the file's VALIDATED time in milliseconds, never\n\
         its filename. Exit 0 clean, 2 when a foreign time is present or the\n\
         file carries no copy of its own.\n\
         \n\
         A clean census is not a clearance: it reads one field, as a u32."
    );
    exit(64)
}

fn main() {
    let argv: Vec<String> = env::args().skip(1).collect();
    let mut path: Option<String> = None;
    let mut own: Option<u32> = None;
    let mut others: Vec<u32> = Vec::new();
    let mut min_count: usize = 2;
    let mut show_offsets = false;

    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--own" => {
                i += 1;
                own = Some(argv.get(i).unwrap_or_else(|| usage()).parse().unwrap_or_else(|_| usage()));
            }
            "--other" => {
                i += 1;
                others.push(argv.get(i).unwrap_or_else(|| usage()).parse().unwrap_or_else(|_| usage()));
            }
            "--min-count" => {
                i += 1;
                min_count = argv.get(i).unwrap_or_else(|| usage()).parse().unwrap_or_else(|_| usage());
            }
            "--offsets" => show_offsets = true,
            "-h" | "--help" => usage(),
            s if s.starts_with("--") => usage(),
            s => path = Some(s.to_string()),
        }
        i += 1;
    }

    let path = path.unwrap_or_else(|| usage());
    let own = own.unwrap_or_else(|| usage());

    let data = fs::read(&path).unwrap_or_else(|e| {
        eprintln!("census: cannot read {}: {}", path, e);
        exit(66)
    });
    let g = Gbx::parse(&data);

    // Three regions, kept apart in the report because they are repaired by
    // different means: a length change in the header invalidates the size u32
    // at offset 77, and a length change in the body is free.
    let regions = vec![
        Region { name: "header", bytes: g.user_data.clone() },
        Region { name: "reftable", bytes: g.ref_table.clone() },
        Region { name: "body", bytes: g.body.clone() },
    ];
    let hits = scan(&regions);

    println!("file      {}", path);
    println!(
        "regions   header {} B, reftable {} B, body {} B",
        g.user_data.len(),
        g.ref_table.len(),
        g.body.len()
    );
    println!("own       {} ms ({})", own, fmt_secs(own));

    let own_hits = hits.get(&own).map(|v| v.len()).unwrap_or(0);

    // Named values first: a zero against a name you chose is a measurement.
    println!();
    println!("count  value      seconds      sites");
    let mut named: Vec<u32> = vec![own];
    named.extend(others.iter().copied());
    for v in &named {
        let h = hits.get(v);
        let n = h.map(|x| x.len()).unwrap_or(0);
        let sites = match h {
            Some(x) if show_offsets => x
                .iter()
                .map(|(r, o)| format!("{}@{}", r, o))
                .collect::<Vec<_>>()
                .join(" "),
            Some(x) => {
                let mut per: BTreeMap<&str, usize> = BTreeMap::new();
                for (r, _) in x {
                    *per.entry(r).or_insert(0) += 1;
                }
                per.iter().map(|(r, c)| format!("{}x{}", c, r)).collect::<Vec<_>>().join(" ")
            }
            None => String::new(),
        };
        let tag = if *v == own { "OWN " } else { "NAMED" };
        println!("{:>5}  {:<10} {:<12} {} {}", n, v, fmt_secs(*v), tag, sites);
    }

    // Then everything else that repeats. An unnamed foreign time is exactly
    // what a search of the leaderboard would have named, so it must surface
    // without being asked for.
    let mut singles = 0usize;
    let mut repeats: Vec<(u32, usize)> = Vec::new();
    for (v, h) in &hits {
        if named.contains(v) {
            continue;
        }
        if h.len() >= min_count {
            repeats.push((*v, h.len()));
        } else {
            singles += 1;
        }
    }
    repeats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (v, n) in &repeats {
        let sites = if show_offsets {
            hits[v].iter().map(|(r, o)| format!("{}@{}", r, o)).collect::<Vec<_>>().join(" ")
        } else {
            let mut per: BTreeMap<&str, usize> = BTreeMap::new();
            for (r, _) in &hits[v] {
                *per.entry(r).or_insert(0) += 1;
            }
            per.iter().map(|(r, c)| format!("{}x{}", c, r)).collect::<Vec<_>>().join(" ")
        };
        println!("{:>5}  {:<10} {:<12} {} {}", n, v, fmt_secs(*v), "other", sites);
    }
    println!(
        "\n{} further value(s) in {}..{} ms occur fewer than {} times and are not listed",
        singles, T_MIN, T_MAX, min_count
    );

    // The verdict names both halves, and says what it did not read.
    println!();
    let foreign_named: Vec<u32> = others
        .iter()
        .copied()
        .filter(|v| hits.get(v).map(|h| !h.is_empty()).unwrap_or(false))
        .collect();
    let mut rc = 0;
    if own_hits == 0 {
        println!("VERDICT FOREIGN: no copy of its own {} ms anywhere in the container", own);
        rc = 2;
    } else if !foreign_named.is_empty() {
        let l: Vec<String> = foreign_named.iter().map(|v| fmt_secs(*v)).collect();
        println!(
            "VERDICT MIXED: {} copies of its own {} AND a named foreign time ({})",
            own_hits,
            fmt_secs(own),
            l.join(", ")
        );
        rc = 2;
    } else {
        println!(
            "VERDICT CLEAN on this field: {} copies of its own {}, none of any named other",
            own_hits,
            fmt_secs(own)
        );
    }
    println!("        (declared time only, read as a u32 -- says nothing about the");
    println!("         nickname, the login, the account id, the skin or the splits)");
    exit(rc)
}
