//! `tmtraj route` — geometric queries over an exported trajectory CSV.
//!
//! Written for the 173691 landing arm, where every question of the shape "does
//! this run ever come near that lip, and in what state" was being answered with
//! `awk` one-liners that nobody could re-run or check. A query that decides
//! whether an artefact is publishable belongs in the tool.
//!
//! The CSV is whatever `tmtraj export --csv` writes: a header line naming the
//! columns, then one row per sample. Columns are looked up BY NAME, never by
//! index, because two producers in this project disagree on column order and a
//! positional reader silently measures the wrong field. CRLF is stripped: the
//! banked CSVs have it and a naive parse turns the last column into a string.

use crate::cli;

const USAGE: &str = "\
usage: tmtraj route CSV [query...]

  --summary                     rows, span, bbox, path length, first and last
  --near X,Y,Z [--top N]        the N samples closest to a point (default 5)
  --where 'COL OP VALUE'        filter; repeatable, ANDed. OP is < <= > >= == !=
  --first N / --last N          print the first / last N rows of the selection
  --every N                     print every Nth row of the selection
  --cols a,b,c                  which columns to print (default t,x,y,z,km/h,vy)

Times print as seconds. A selection that is empty says so and exits 1, because
'no rows' and 'no output' are different answers.
";

/// A usage/read error: say what is wrong and stop. Never a default.
fn die(msg: &str) -> ! {
    eprintln!("tmtraj route: {}", msg);
    std::process::exit(2)
}

struct Table {
    names: Vec<String>,
    rows: Vec<Vec<f64>>,
}

impl Table {
    fn col(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }
    /// Column index or a fatal error naming what the file does have.
    fn need(&self, name: &str) -> usize {
        self.col(name).unwrap_or_else(|| {
            die(&format!(
                "no column {:?} in this CSV; it has: {}",
                name,
                self.names.join(",")
            ))
        })
    }
}

fn load(path: &str) -> Table {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {}", path, e)));
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_else(|| die("empty CSV"));
    let names: Vec<String> =
        header.trim_end_matches('\r').split(',').map(|s| s.trim().to_string()).collect();
    let mut rows = Vec::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let row: Vec<f64> = line
            .split(',')
            .map(|f| f.trim().parse::<f64>().unwrap_or(f64::NAN))
            .collect();
        if row.len() < names.len() {
            continue;
        }
        rows.push(row);
    }
    Table { names, rows }
}

struct Pred {
    col: usize,
    op: String,
    val: f64,
    text: String,
}

fn parse_pred(t: &Table, s: &str) -> Pred {
    // "y>130", "y > 130", "speed_kmh >= 90"
    let ops = ["<=", ">=", "==", "!=", "<", ">"];
    for op in ops {
        if let Some(i) = s.find(op) {
            let lhs = s[..i].trim().to_string();
            let rhs = s[i + op.len()..].trim();
            let val: f64 = rhs
                .parse()
                .unwrap_or_else(|_| die(&format!("--where {:?}: {:?} is not a number", s, rhs)));
            return Pred { col: t.need(&lhs), op: op.to_string(), val, text: s.to_string() };
        }
    }
    die(&format!("--where {:?}: no comparison operator (< <= > >= == !=)", s))
}

fn holds(p: &Pred, row: &[f64]) -> bool {
    let v = row[p.col];
    if v.is_nan() {
        return false;
    }
    match p.op.as_str() {
        "<" => v < p.val,
        "<=" => v <= p.val,
        ">" => v > p.val,
        ">=" => v >= p.val,
        "==" => v == p.val,
        "!=" => v != p.val,
        _ => unreachable!(),
    }
}

fn fmt_row(t: &Table, row: &[f64], cols: &[usize], tcol: usize) -> String {
    let mut out = Vec::new();
    for &c in cols {
        let v = row[c];
        if c == tcol {
            out.push(format!("{:>10}", crate::fmt::secs(v as i64)));
        } else if v.is_nan() {
            out.push(format!("{:>10}", "-"));
        } else {
            out.push(format!("{:>10.3}", v));
        }
    }
    let _ = t;
    out.join(" ")
}

pub fn cmd(argv: &[String]) -> i32 {
    let a = cli::parse("tmtraj route", argv, &["summary"]);
    if a.positional.is_empty() {
        eprint!("{}", USAGE);
        return 2;
    }
    let path = a.positional[0].clone();
    let near = a.one("near").map(|s| s.to_string());
    let top: usize = a.num("top", 5);
    let wheres: Vec<String> = a.many("where").iter().map(|s| s.to_string()).collect();
    let firstn = a.one("first").map(|s| s.parse::<usize>().unwrap_or(5));
    let lastn = a.one("last").map(|s| s.parse::<usize>().unwrap_or(5));
    let every = a.one("every").map(|s| s.parse::<usize>().unwrap_or(1));
    let want_cols = a.one("cols").map(|s| s.to_string());
    let summary = a.has("summary");
    a.finish(USAGE);

    let t = load(&path);
    let (cx, cy, cz) = (t.need("x"), t.need("y"), t.need("z"));
    let ct = t.col("time_ms").unwrap_or(0);

    let cols: Vec<usize> = match &want_cols {
        Some(s) => s.split(',').map(|n| t.need(n.trim())).collect(),
        None => {
            let mut v = vec![ct, cx, cy, cz];
            for n in ["speed_kmh", "vy", "vz"] {
                if let Some(i) = t.col(n) {
                    v.push(i);
                }
            }
            v
        }
    };
    let head: Vec<&str> = cols.iter().map(|&c| t.names[c].as_str()).collect();

    println!("{}  ({} rows, {} columns)", path, t.rows.len(), t.names.len());

    if summary || (near.is_none() && wheres.is_empty() && firstn.is_none() && lastn.is_none()) {
        let mut len = 0.0;
        let (mut xmin, mut xmax) = (f64::MAX, f64::MIN);
        let (mut ymin, mut ymax) = (f64::MAX, f64::MIN);
        let (mut zmin, mut zmax) = (f64::MAX, f64::MIN);
        for (i, r) in t.rows.iter().enumerate() {
            if i > 0 {
                let p = &t.rows[i - 1];
                len += ((r[cx] - p[cx]).powi(2) + (r[cy] - p[cy]).powi(2) + (r[cz] - p[cz]).powi(2))
                    .sqrt();
            }
            xmin = xmin.min(r[cx]);
            xmax = xmax.max(r[cx]);
            ymin = ymin.min(r[cy]);
            ymax = ymax.max(r[cy]);
            zmin = zmin.min(r[cz]);
            zmax = zmax.max(r[cz]);
        }
        if let (Some(f), Some(l)) = (t.rows.first(), t.rows.last()) {
            println!(
                "  span   {} .. {}",
                crate::fmt::secs(f[ct] as i64),
                crate::fmt::secs(l[ct] as i64)
            );
            println!("  first  ({:.2}, {:.2}, {:.2})", f[cx], f[cy], f[cz]);
            println!("  last   ({:.2}, {:.2}, {:.2})", l[cx], l[cy], l[cz]);
        }
        println!("  bbox   x {:.1}..{:.1}  y {:.1}..{:.1}  z {:.1}..{:.1}", xmin, xmax, ymin, ymax, zmin, zmax);
        println!("  path   {:.1} m", len);
    }

    // --near: rank by distance to a point.
    if let Some(p) = &near {
        let nums: Vec<f64> = p.split(',').map(|s| s.trim().parse().unwrap_or(f64::NAN)).collect();
        if nums.len() != 3 || nums.iter().any(|v| v.is_nan()) {
            die("--near wants X,Y,Z");
        }
        let mut idx: Vec<(f64, usize)> = t
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let d = ((r[cx] - nums[0]).powi(2) + (r[cy] - nums[1]).powi(2) + (r[cz] - nums[2]).powi(2))
                    .sqrt();
                (d, i)
            })
            .collect();
        idx.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        println!("\nnearest to ({:.2}, {:.2}, {:.2}):", nums[0], nums[1], nums[2]);
        println!("{:>10} {}", "dist_m", head.iter().map(|h| format!("{:>10}", h)).collect::<Vec<_>>().join(" "));
        for (d, i) in idx.iter().take(top) {
            println!("{:>10.3} {}", d, fmt_row(&t, &t.rows[*i], &cols, ct));
        }
    }

    // --where: filter, then print.
    if !wheres.is_empty() || firstn.is_some() || lastn.is_some() {
        let preds: Vec<Pred> = wheres.iter().map(|s| parse_pred(&t, s)).collect();
        let sel: Vec<usize> = t
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| preds.iter().all(|p| holds(p, r)))
            .map(|(i, _)| i)
            .collect();
        let what = if preds.is_empty() {
            "all rows".to_string()
        } else {
            preds.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join(" AND ")
        };
        println!("\nselection [{}]: {} of {} rows", what, sel.len(), t.rows.len());
        if sel.is_empty() {
            return 1;
        }
        println!("{}", head.iter().map(|h| format!("{:>10}", h)).collect::<Vec<_>>().join(" "));
        let mut shown: Vec<usize> = Vec::new();
        if let Some(n) = firstn {
            shown.extend(sel.iter().take(n).copied());
        }
        if let Some(n) = lastn {
            shown.extend(sel.iter().rev().take(n).rev().copied());
        }
        if let Some(n) = every {
            let n = n.max(1);
            shown.extend(sel.iter().step_by(n).copied());
        }
        if shown.is_empty() {
            shown.push(sel[0]);
            if sel.len() > 1 {
                shown.push(sel[sel.len() - 1]);
            }
        }
        shown.sort_unstable();
        shown.dedup();
        for i in shown {
            println!("{}", fmt_row(&t, &t.rows[i], &cols, ct));
        }
    }
    0
}
