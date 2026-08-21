//! `tmtraj recspan` -- rewrite the record's own declared time span.
//!
//! WHY (arm `r165`, 2026-08-20)
//! ---------------------------
//! `CPlugEntRecordData` declares a start and an end in milliseconds, separately
//! from the samples. A file whose record was imported from a container inherits
//! the CONTAINER's span: on 165922 that is `start 0 end 8790760` -- a 2.4-hour
//! span on a 15-second run, 578x too long, because the donor is a session
//! recording. `tmtraj tail fix` truncates the SAMPLES and leaves the span
//! alone (227654's published files ship with the donor's 147.030 s span for a
//! 57.493 s run, which is the same defect at 2.5x and has not been noticed).
//!
//! It refuses to cut the span shorter than the last sample it would then be
//! describing, because that is a record that disagrees with itself.

use crate::recwrite::rewrite_ghost;

pub fn cmd(args: &[String]) {
    let flag = |n: &str| -> Option<String> {
        args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
    };
    let inp = flag("--in").expect("--in GHOST.Ghost.Gbx");
    let out = flag("--out").expect("--out OUT.Ghost.Gbx");
    let end: i32 = flag("--end").expect("--end MS").parse().expect("--end MS");
    let start: Option<i32> = flag("--start").map(|v| v.parse().expect("--start MS"));
    let mut before = (0i32, 0i32);
    let trim_all = args.iter().any(|a| a == "--trim-all");
    let mut trimmed: Vec<(i32, usize, usize)> = Vec::new();
    let mut last = i32::MIN;
    let r = rewrite_ghost(&inp, &out, |rd| {
        before = (rd.start_ms, rd.end_ms);
        for e in rd.ents.iter() {
            if let Some(t) = e.times.last() {
                if *t > last {
                    last = *t;
                }
            }
        }
        if trim_all { last = i32::MIN; }
        if last > end {
            return Err(format!(
                "the record's last sample is at {} ms and the span would end at {} ms",
                last, end
            ));
        }
        // arm `r165`: the vehicle entity is not the only one the container brings.
        // 165922's donor also carries 175815 samples of the undecoded 0x2D001000
        // entity spanning its whole 2.4-hour session; `tail fix` trims only the
        // vehicle. --trim-all keeps, in every entity, only the samples inside
        // [0, end], which is the window this run actually happened in.
        if trim_all {
            for e in rd.ents.iter_mut() {
                if e.times.is_empty() || e.sample_size == 0 {
                    continue;
                }
                let ss = e.sample_size;
                let keep: Vec<bool> = e.times.iter().map(|t| *t >= 0 && *t <= end).collect();
                let dropped = keep.iter().filter(|k| !**k).count();
                if dropped == 0 {
                    continue;
                }
                let mut nt: Vec<i32> = Vec::new();
                let mut nr: Vec<u8> = Vec::new();
                for (i, k) in keep.iter().enumerate() {
                    if *k {
                        nt.push(e.times[i]);
                        nr.extend_from_slice(&e.raw[i * ss .. (i + 1) * ss]);
                    }
                }
                trimmed.push((e.type_, e.times.len(), nt.len()));
                e.times = nt;
                e.raw = nr;
            }
            last = rd.ents.iter().filter_map(|e| e.times.last().copied()).max().unwrap_or(i32::MIN);
        }
        rd.end_ms = end;
        if let Some(s) = start {
            rd.start_ms = s;
        }
        Ok(())
    });
    match r {
        Ok((a, b)) => {
            for (c, before_n, after_n) in &trimmed {
                println!("  entity type {}: {} -> {} samples", c, before_n, after_n);
            }println!(
            "recspan: {} -> {}: span {}..{} ms -> {}..{} ms (last sample {} ms), record {} -> {} B",
            inp, out, before.0, before.1,
            start.unwrap_or(before.0), end, last, a, b
        ); },
        Err(e) => {
            println!("ABORT: {}", e);
            std::process::exit(3);
        }
    }
}
