//! Contiguous stretches of a `vidread lamps` table where the overlay is on
//! screen, and what the lamps do inside them.

use std::io::{BufRead, Write};

pub struct Row {
    pub t: f64,
    pub present: bool,
    pub bits: [bool; 5],
}

pub fn read_table(r: impl BufRead) -> Vec<Row> {
    let mut v = Vec::new();
    for line in r.lines() {
        let line = line.expect("read");
        if line.starts_with('t') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 {
            continue;
        }
        let mut bits = [false; 5];
        for k in 0..5 {
            bits[k] = f[2 + k] == "1";
        }
        v.push(Row { t: f[0].parse().unwrap(), present: f[1] == "1", bits });
    }
    v
}

/// Print every run of present frames at least `min_len` frames long, with the
/// number of frames each lamp is lit inside it.
pub fn sections(rows: &[Row], min_len: usize, gap: usize, o: &mut impl Write) {
    let mut i = 0;
    writeln!(o, "start\tend\tsecs\tframes\tbrake\tup\tdown\tleft\tright").unwrap();
    let mut total = 0.0;
    while i < rows.len() {
        if !rows[i].present {
            i += 1;
            continue;
        }
        // extend, tolerating up to `gap` absent frames inside a section
        let start = i;
        let mut end = i;
        let mut j = i;
        let mut miss = 0;
        while j < rows.len() {
            if rows[j].present {
                end = j;
                miss = 0;
            } else {
                miss += 1;
                if miss > gap {
                    break;
                }
            }
            j += 1;
        }
        let n = end - start + 1;
        if n >= min_len {
            let mut c = [0usize; 5];
            for r in &rows[start..=end] {
                for k in 0..5 {
                    if r.bits[k] {
                        c[k] += 1;
                    }
                }
            }
            let secs = rows[end].t - rows[start].t;
            total += secs;
            writeln!(
                o,
                "{:.3}\t{:.3}\t{:.2}\t{}\t{}\t{}\t{}\t{}\t{}",
                rows[start].t, rows[end].t, secs, n, c[0], c[1], c[2], c[3], c[4]
            )
            .unwrap();
        }
        i = j;
    }
    writeln!(o, "# total {:.2} s of overlay", total).unwrap();
}
