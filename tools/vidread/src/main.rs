//! vidread -- read a Trackmania run off a screen recording.
//!
//! Frames arrive as raw rgb24 on stdin; ffmpeg is the decoder:
//!
//!   ffmpeg -v error -ss 500 -t 25 -i v.webm -f rawvideo -pix_fmt rgb24 - \
//!     | vidread lamps --fps 60 --t0 500
//!
//! Every frame-reading subcommand prints a TSV whose first column is the
//! frame's time in the source video, in seconds.

mod align;
mod digits;
mod enginecmp;
mod frame;
mod ink;
mod keyphys;
mod keytape;
mod lamps;
mod sections;
mod trace;
mod xcheck;

use digits::{Field, Patch, Templates};
use frame::Frame;
use std::collections::BTreeMap;
use std::io::{BufWriter, Write};

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned()
}

fn need(args: &[String], k: &str) -> String {
    arg(args, k).unwrap_or_else(|| die(&format!("missing {k}")))
}

fn num<T: std::str::FromStr>(args: &[String], k: &str, dflt: T) -> T {
    match arg(args, k) {
        Some(v) => v.parse().unwrap_or_else(|_| die(&format!("bad value for {k}"))),
        None => dflt,
    }
}

fn die(m: &str) -> ! {
    eprintln!("vidread: {m}");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_default();
    let w: usize = num(&args, "--w", 2560);
    let h: usize = num(&args, "--h", 1440);
    let fps: f64 = num(&args, "--fps", 60.0);
    let t0: f64 = num(&args, "--t0", 0.0);
    let sx: i32 = num(&args, "--sx", 3);
    let sy: i32 = num(&args, "--sy", 2);

    let stdin = std::io::stdin();
    let mut r = std::io::BufReader::with_capacity(1 << 22, stdin.lock());
    let out = std::io::stdout();
    let mut o = BufWriter::new(out.lock());
    let mut f = Frame::new(w, h);
    let at = |i: u64| t0 + i as f64 / fps;

    match cmd.as_str() {
        "lamps" => {
            let lo: f32 = num(&args, "--lo", 0.60);
            let hi: f32 = num(&args, "--hi", 0.75);
            let border_min: f32 = num(&args, "--border-min", 150.0);
            let glyph_min: f32 = num(&args, "--glyph-min", 30.0);
            let raw = args.iter().any(|a| a == "--raw");
            write!(o, "t\tpresent\t{}", lamps::NAMES.join("\t")).unwrap();
            if raw {
                write!(o, "\tfill0\tfill1\tfill2\tfill3\tfill4\tbord0\tbord1\tbord2\tbord3\tbord4").unwrap();
            }
            writeln!(o).unwrap();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                let rd = lamps::Reading::of(&f);
                let p = rd.present(border_min, lo, hi, glyph_min);
                let b = rd.bits(lo, hi);
                write!(o, "{:.4}\t{}", at(i), p as u8).unwrap();
                for k in 0..5 {
                    write!(o, "\t{}", (p && b[k]) as u8).unwrap();
                }
                if raw {
                    for k in 0..5 {
                        write!(o, "\t{:.1}", rd.fill[k]).unwrap();
                    }
                    for k in 0..5 {
                        write!(o, "\t{:.1}", rd.border[k]).unwrap();
                    }
                }
                writeln!(o).unwrap();
                i += 1;
            }
        }

        "sections" => {
            let min_len: usize = num(&args, "--min-len", 5);
            let gap: usize = num(&args, "--gap", 2);
            let rows = sections::read_table(&mut r);
            sections::sections(&rows, min_len, gap, &mut o);
        }

        "ink" => {
            let n: Vec<usize> =
                need(&args, "--rect").split(',').map(|s| s.parse().unwrap()).collect();
            let thresh: f32 = num(&args, "--thresh", 200.0);
            let mut p = ink::Profile::new(frame::Rect::new(n[0], n[1], n[2], n[3]));
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                p.add(&f, thresh);
            }
            p.print(&mut o);
        }

        // A PGM contact sheet of cell patches: one row of cells per sampled
        // frame. Labelling is done by eye off this sheet, which is the only
        // way that has ever worked on these fonts.
        "patches" => {
            let fd = Field::parse(&need(&args, "--field"));
            let every: u64 = num(&args, "--every", 1);
            let rows: usize = num(&args, "--rows", 40);
            let path = need(&args, "--out");
            let gap = 3usize;
            let rw = fd.cells() * fd.pw + (fd.cells() + 1) * gap;
            let rh = fd.ph + gap;
            let mut img = vec![0u8; rw * rh * rows];
            let mut i = 0u64;
            let mut n = 0usize;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                if i % every == 0 && n < rows {
                    for k in 0..fd.cells() {
                        let ox = gap + k * (fd.pw + gap);
                        for y in 0..fd.ph {
                            for x in 0..fd.pw {
                                let px = fd.xs[k].round() as usize + x;
                                let py = fd.y0.round() as usize + y;
                                img[(n * rh + y) * rw + ox + x] = f.minc(px, py) as u8;
                            }
                        }
                    }
                    eprintln!("row {n} = frame {i} t {:.4}", at(i));
                    n += 1;
                }
                i += 1;
            }
            let mut fh = std::fs::File::create(&path).unwrap();
            write!(fh, "P5\n{rw} {}\n255\n", rh * rows).unwrap();
            fh.write_all(&img).unwrap();
            eprintln!("wrote {path}: {rw}x{}", rh * rows);
        }

        // Build templates. Either from frames whose value I read by eye
        //   --labels "12=179,60=253"     (one glyph per cell, '.' skips a cell)
        // or by bootstrapping from a previous pass's own confident, temporally
        // consistent readings
        //   --from-read speed.tsv --min-score 0.78
        // The bootstrap can only sharpen glyphs the eye-labelled pass already
        // gets right; it is checked against a held-out eye reading, never
        // trusted on its own.
        "train" => {
            let fd = Field::parse(&need(&args, "--field"));
            let per_cell = args.iter().any(|a| a == "--per-cell");
            let cap: usize = num(&args, "--max-per-glyph", 40);
            let mut want: BTreeMap<u64, Vec<char>> = BTreeMap::new();
            if let Some(l) = arg(&args, "--labels") {
                for spec in l.split(',') {
                    let (i, v) =
                        spec.split_once('=').unwrap_or_else(|| die("labels are IDX=DIGITS"));
                    let cs: Vec<char> = v.chars().collect();
                    if cs.len() != fd.cells() {
                        die(&format!("label {v}: {} chars for {} cells", cs.len(), fd.cells()));
                    }
                    want.insert(i.parse().unwrap(), cs);
                }
            }
            if let Some(p) = arg(&args, "--from-read") {
                let min_score: f32 = num(&args, "--min-score", 0.78);
                let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| die(&e.to_string()));
                let rows: Vec<(String, f32)> = txt
                    .lines()
                    .skip(1)
                    .filter_map(|l| {
                        let f: Vec<&str> = l.split('\t').collect();
                        (f.len() >= 3).then(|| (f[1].to_string(), f[2].parse().unwrap_or(0.0)))
                    })
                    .collect();
                for i in 1..rows.len().saturating_sub(1) {
                    if rows[i].1 >= min_score
                        && rows[i].0 == rows[i - 1].0
                        && rows[i].0 == rows[i + 1].0
                        && !rows[i].0.contains('?')
                    {
                        want.insert(i as u64, rows[i].0.chars().collect());
                    }
                }
            }
            if want.is_empty() {
                die("train needs --labels or --from-read");
            }
            let mut samples: BTreeMap<(usize, char), Vec<Patch>> = BTreeMap::new();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                if let Some(cs) = want.get(&i) {
                    for (k, c) in cs.iter().enumerate() {
                        if *c != '.' {
                            let key = (if per_cell { k } else { 0 }, *c);
                            let v = samples.entry(key).or_default();
                            if v.len() < cap {
                                v.push(Patch::cut(&f, &fd, k, 0, 0));
                            }
                        }
                    }
                }
                i += 1;
            }
            for (k, v) in &samples {
                eprintln!("glyph {}:{} -- {} samples", k.0, k.1, v.len());
            }
            Templates::from_samples(fd.pw, fd.ph, per_cell, &samples).write(&mut o);
        }

        // Read the field on every frame. A right-aligned field pads with
        // BLANK, not with a zero, and a blank cell has no glyph to match: it
        // is called blank when its own best score collapses while the cells to
        // its right stay legible.
        "read" => {
            let fd = Field::parse(&need(&args, "--field"));
            let blank_max: f32 = num(&args, "--blank-max", 0.55);
            let legible_min: f32 = num(&args, "--legible-min", 0.70);
            let t = Templates::read(
                &std::fs::read_to_string(need(&args, "--templates"))
                    .unwrap_or_else(|e| die(&e.to_string())),
            );
            let with_lamps = args.iter().any(|a| a == "--with-lamps");
            write!(o, "t\tvalue\tworst\tmargin\tdx\tdy").unwrap();
            if with_lamps { write!(o, "\tpresent\t{}", lamps::NAMES.join("\t")).unwrap(); }
            writeln!(o).unwrap();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                let (cells, gdx, gdy) = t.read_field(&f, &fd, sx, sy);
                let mut s = String::new();
                let mut worst = 2.0f32;
                let mut margin = 2.0f32;
                for (k, (c, best, second)) in cells.iter().enumerate() {
                    let rest_ok = cells[k + 1..].iter().all(|x| x.1 >= legible_min);
                    if *best < blank_max && k + 1 < fd.cells() && rest_ok {
                        s.push('_');
                        continue;
                    }
                    s.push(*c);
                    worst = worst.min(*best);
                    margin = margin.min(best - second);
                }
                write!(o, "{:.4}\t{}\t{:.3}\t{:.3}\t{}\t{}", at(i), s, worst, margin, gdx, gdy).unwrap();
                if with_lamps {
                    let rd = lamps::Reading::of(&f);
                    let p = rd.present(150.0, 0.60, 0.75, 30.0);
                    let b = rd.bits(0.60, 0.75);
                    write!(o, "\t{}", p as u8).unwrap();
                    for k in 0..5 { write!(o, "\t{}", (p && b[k]) as u8).unwrap(); }
                }
                writeln!(o).unwrap();
                i += 1;
            }
        }

        "trace" => {
            let min_score: f32 = num(&args, "--min-score", 0.65);
            let tol: f64 = num(&args, "--tol", 18.0);
            let half: usize = num(&args, "--half", 5);
            let mut s = trace::parse(&mut r, min_score);
            let d = trace::despike(&mut s, half, tol);
            eprintln!("despike dropped {d} readings");
            trace::print(&s, &mut o);
        }
        "keytape" => {
            let from: f64 = num(&args, "--from", f64::MIN);
            let to: f64 = num(&args, "--to", f64::MAX);
            let min_score: f32 = num(&args, "--min-score", 0.72);
            let half: usize = num(&args, "--half", 4);
            let tol_ms: f64 = num(&args, "--tol-ms", 25.0);
            let rows = match arg(&args, "--align") {
                Some(spec) => {
                    let v: Vec<f64> = spec.split(',').map(|x| x.parse().expect("rate,ms0,t0")).collect();
                    keytape::parse_aligned(&mut r, v[0], v[1], v[2], from, to)
                }
                None => keytape::parse(&mut r, min_score)
                    .into_iter()
                    .filter(|x| x.t >= from && x.t <= to)
                    .collect(),
            };
            let keep = keytape::monotone_filter(&rows, half, tol_ms);
            eprintln!("{} clock-legible frames with the overlay up, {} survive the monotone filter", rows.len(), keep.len());
            let m = keytape::collapse(&rows, &keep);
            keytape::print(&m, &mut o);
        }
        "xcheck" => {
            let cl = std::fs::File::open(need(&args, "--clock")).unwrap_or_else(|e| die(&e.to_string()));
            let sp = std::fs::File::open(need(&args, "--speed")).unwrap_or_else(|e| die(&e.to_string()));
            let rf = std::fs::File::open(need(&args, "--reference")).unwrap_or_else(|e| die(&e.to_string()));
            let reference = xcheck::load_reference(std::io::BufReader::new(rf));
            let p = xcheck::pairs(
                std::io::BufReader::new(cl),
                std::io::BufReader::new(sp),
                &reference,
                num(&args, "--min-clock", 0.72f32),
                num(&args, "--min-speed", 0.65f32),
                num(&args, "--win-ms", 20i64),
            );
            xcheck::report(&p, num(&args, "--near", 5.0f64), &mut o);
        }
        "align" => {
            let rf = std::fs::File::open(need(&args, "--reference")).unwrap_or_else(|e| die(&e.to_string()));
            let reference = xcheck::load_reference(std::io::BufReader::new(rf));
            let lo: f64 = num(&args, "--from", 0.0);
            let hi: f64 = num(&args, "--to", 1e9);
            let sp = std::fs::File::open(need(&args, "--speed")).unwrap_or_else(|e| die(&e.to_string()));
            let all = align::load_clip(std::io::BufReader::new(sp), num(&args, "--min-score", 0.65f32));
            let clip: Vec<align::Obs> = all.into_iter().filter(|c| c.t >= lo && c.t <= hi).collect();
            let f = align::fit(
                &clip,
                &reference,
                (num(&args, "--rate-lo", 0.08), num(&args, "--rate-hi", 1.05), num(&args, "--rate-step", 0.005)),
                (num(&args, "--off-lo", 0.0), num(&args, "--off-hi", 73000.0), num(&args, "--off-step", 20.0)),
                num(&args, "--win-ms", 25i64),
                num(&args, "--near", 4.0f64),
            );
            align::print(&arg(&args, "--name").unwrap_or_else(|| format!("{lo}-{hi}")), &f, &mut o);
        }
        "ktevents" => {
            let m = keytape::load_record(&need(&args, "--record"));
            keytape::events(&m, &mut o);
        }
        "ktcompare" => {
            let a = keytape::load_record(&need(&args, "--a"));
            let b = keytape::load_record(&need(&args, "--b"));
            keytape::compare(&a, &b, &mut o);
            if let Some(m) = arg(&args, "--shifts") {
                keytape::compare_shifts(&a, &b, m.parse().unwrap(), &mut o);
            }
        }
        "keyphys" => {
            let rec = keytape::load_record(&need(&args, "--record"));
            let sf = std::fs::File::open(need(&args, "--trace")).unwrap_or_else(|e| die(&e.to_string()));
            let sp = xcheck::load_reference(std::io::BufReader::new(sf));
            keyphys::report(&rec, &sp, num(&args, "--window-ms", 100i64), num(&args, "--tol-ms", 25i64), &mut o);
        }
        "enginecmp" => {
            let vf = std::fs::File::open(need(&args, "--video")).unwrap_or_else(|e| die(&e.to_string()));
            let video = xcheck::load_reference(std::io::BufReader::new(vf));
            let engine = enginecmp::load_engine(&need(&args, "--engine")).unwrap_or_else(|e| die(&e));
            enginecmp::report(&video, &engine, num(&args, "--tol", 8.0f64), num(&args, "--run", 6usize), num(&args, "--tol-ms", 10i64), &mut o);
        }
        _ => die("usage: vidread lamps|sections|ink|patches|train|read|trace ..."),
    }
}
