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
mod glyphs;
mod ink;
mod keylag;
mod keyphys;
mod keytape;
mod lamps;
mod sections;
mod trace;
mod wetlaw;
mod wetread;
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

/// The frame selector every wetness reader shares, from the common flags.
fn wetsel(args: &[String]) -> (wetread::Select, Templates) {
    let icons = Templates::read(
        &std::fs::read_to_string(need(args, "--icons")).unwrap_or_else(|e| die(&e.to_string())),
    );
    let sel = wetread::Select {
        span_min: num(args, "--span-min", 45.0),
        min_ink: num(args, "--min-ink", 0.25),
        max_gap: num(args, "--max-gap", 3),
        icon_min: num(args, "--icon-min", 0.70),
        edge_tol: num(args, "--edge-tol", 0),
    };
    (sel, icons)
}

fn main() {
    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    if std::env::args().any(|x| x == "--help" || x == "-h") {
        // Usage on STDOUT, exit 0 -- see gbx/tests/cli_contract.rs.
        print!("{}", r#"
vidread: usage: vidread lamps|sections|ink|patches|train|read|trace|weticon|wetedge|wetgeom|wetalpha|wetread|wetlaw ...
"#);
        std::process::exit(0);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_default();
    let w: usize = num(&args, "--w", 2560);
    let h: usize = num(&args, "--h", 1440);
    let fps: f64 = num(&args, "--fps", 60.0);
    let t0: f64 = num(&args, "--t0", 0.0);
    let sx: i32 = num(&args, "--sx", 3);
    let sy: i32 = num(&args, "--sy", 2);
    // The stream may be a crop: --ox/--oy give the frame coordinates of its
    // pixel (0,0), so every constant in this crate stays in full-frame pixels.
    let ox: usize = num(&args, "--ox", 0);
    let oy: usize = num(&args, "--oy", 0);

    let stdin = std::io::stdin();
    let mut r = std::io::BufReader::with_capacity(1 << 22, stdin.lock());
    let out = std::io::stdout();
    let mut o = BufWriter::new(out.lock());
    let mut f = Frame::with_origin(w, h, ox, oy);
    let at = |i: u64| t0 + i as f64 / fps;
    let at2 = at;

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

        // Per-frame ink in a rectangle, as a series. `ink` sums over every frame
        // and answers "where are the cells"; this answers "on which frames is
        // there anything to read at all", which is the question that decides
        // whether a readout can be an objective.
        "inkseries" => {
            let n: Vec<usize> =
                need(&args, "--rect").split(',').map(|s| s.parse().unwrap()).collect();
            // Contrast, not level: this text is white over backgrounds that run
            // from a dark tunnel to a white wall, so an absolute threshold
            // measures the scenery. The span between the rectangle's brightest
            // and darkest pixels is what a glyph adds.
            let span_min: f32 = num(&args, "--span-min", 45.0);
            writeln!(o, "t\tp95\tp05\tspan\tpresent").unwrap();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                let mut v: Vec<f32> = Vec::with_capacity(n[2] * n[3]);
                for y in n[1]..n[1] + n[3] {
                    for x in n[0]..n[0] + n[2] {
                        v.push(f.minc(x, y));
                    }
                }
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let hi = v[(v.len() - 1) * 95 / 100];
                let lo = v[(v.len() - 1) * 5 / 100];
                writeln!(
                    o,
                    "{:.4}\t{:.1}\t{:.1}\t{:.1}\t{}",
                    at(i),
                    hi,
                    lo,
                    hi - lo,
                    ((hi - lo) >= span_min) as u8
                )
                .unwrap();
                i += 1;
            }
        }

        // A rectangle as TEXT. The contact-sheet PGM is the right tool when a
        // human can look at it; this is the right tool when the readout is 30
        // pixels wide and the labeller is working down a pipe.
        "ascii" => {
            let n: Vec<usize> =
                need(&args, "--rect").split(',').map(|s| s.parse().unwrap()).collect();
            let at: Vec<u64> = arg(&args, "--frames")
                .unwrap_or_else(|| "0".into())
                .split(',')
                .map(|s| s.parse().unwrap())
                .collect();
            let ramp: Vec<char> = " .:-=+*#%@".chars().collect();
            let mut lo: f32 = num(&args, "--lo", 60.0);
            let mut hi: f32 = num(&args, "--hi", 230.0);
            // The HUD box sits over everything from a dark tunnel to a white
            // wall, so a fixed ramp shows an empty box on most frames. --auto
            // takes the ends from the rectangle's own 2nd and 98th percentiles,
            // which is the same "measure against the band's own level" rule the
            // ink profile is built on.
            let auto = args.iter().any(|a| a == "--auto");
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                if at.contains(&i) {
                    if auto {
                        let mut v: Vec<f32> = Vec::with_capacity(n[2] * n[3]);
                        for y in n[1]..n[1] + n[3] {
                            for x in n[0]..n[0] + n[2] {
                                v.push(f.minc(x, y));
                            }
                        }
                        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        lo = v[(v.len() - 1) * 2 / 100];
                        hi = v[(v.len() - 1) * 98 / 100].max(lo + 1.0);
                    }
                    writeln!(o, "# frame {} t {:.4}  rect {:?}  ramp {lo}..{hi}", i, at2(i), n)
                        .unwrap();
                    for y in n[1]..n[1] + n[3] {
                        let mut s = String::new();
                        for x in n[0]..n[0] + n[2] {
                            let v = ((f.minc(x, y) - lo) / (hi - lo)).clamp(0.0, 1.0);
                            s.push(ramp[(v * (ramp.len() - 1) as f32).round() as usize]);
                        }
                        writeln!(o, "{}", s).unwrap();
                    }
                }
                i += 1;
            }
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
            if let Some((a, b)) = keytape::window(&m) {
                writeln!(o, "# authoritative over race {a}..{b} ms only", ).unwrap();
            }
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
        "keylag" => {
            let rec = keytape::load_record(&need(&args, "--record"));
            let sf = std::fs::File::open(need(&args, "--trace")).unwrap_or_else(|e| die(&e.to_string()));
            let sp = xcheck::load_reference(std::io::BufReader::new(sf));
            keylag::report(&rec, &sp, num(&args, "--lag-max", 1000i64), num(&args, "--lag-step", 20i64), num(&args, "--window-ms", 100i64), num(&args, "--tol-ms", 25i64), &mut o);
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
            enginecmp::report(&video, &engine, num(&args, "--tol", 8.0f64), num(&args, "--run", 6usize), num(&args, "--tol-ms", 50i64), &mut o);
        }
        // The ink profile and the right edge it yields, per frame. The check
        // before any glyph exists.
        "wetedge" => {
            let span_min: f32 = num(&args, "--span-min", 45.0);
            let min_ink: f32 = num(&args, "--min-ink", 0.25);
            let max_gap: usize = num(&args, "--max-gap", 3);
            let show = args.iter().any(|a| a == "--profile");
            // With an icon bank, every row also says WHICH icon is in the slot.
            // Without one the column is empty, which is the state this reader
            // was in when it believed `! Slip` frames were readings of 0 %.
            let icons = arg(&args, "--icons").map(|p| {
                Templates::read(
                    &std::fs::read_to_string(&p).unwrap_or_else(|e| die(&e.to_string())),
                )
            });
            writeln!(o, "t\tpresent\ticon\ticon_score\tright_edge\tprofile").unwrap();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                let p = wetread::icon_present(&f, span_min);
                let e = if p { wetread::right_edge(&f, min_ink, max_gap) } else { None };
                let k = match (&icons, p) {
                    (Some(t), true) => {
                        let (c, s) = wetread::icon_kind(&f, t);
                        format!("{c}\t{s:.3}")
                    }
                    _ => "\t".to_string(),
                };
                write!(
                    o,
                    "{:.4}\t{}\t{}\t{}",
                    at(i),
                    p as u8,
                    k,
                    e.map(|v| v.to_string()).unwrap_or_default()
                )
                .unwrap();
                if show && p {
                    let prof = wetread::ink_profile(&f);
                    write!(o, "\t").unwrap();
                    for v in prof {
                        write!(o, "{}", (v * 9.0).round() as u8).unwrap();
                    }
                }
                writeln!(o).unwrap();
                i += 1;
            }
        }

        // The icon slot holds TWO shapes, and the presence test cannot tell
        // them apart: a droplet when the line is a percentage, and the `!` of
        // `! Slip` when it is not. Cluster the slot, and cross-check each
        // cluster against the right edge the SAME frames give -- a fixed
        // string has one edge, a variable-width number has three.
        "weticon" => {
            let span_min: f32 = num(&args, "--span-min", 45.0);
            let radius: f32 = num(&args, "--radius", 0.82);
            let min_members: usize = num(&args, "--min-members", 8);
            let min_ink: f32 = num(&args, "--min-ink", 0.25);
            let max_gap: usize = num(&args, "--max-gap", 3);
            // --names 0=D,1=! writes a template bank from the named clusters.
            let names: BTreeMap<usize, char> = arg(&args, "--names")
                .map(|s| {
                    s.split(',')
                        .map(|kv| {
                            let (k, v) = kv.split_once('=').unwrap_or_else(|| die("K=CHAR"));
                            (k.parse().unwrap(), v.chars().next().unwrap())
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut cl = glyphs::Clusters::new(wetread::ICON.w, wetread::ICON.h);
            let mut edge: BTreeMap<u64, usize> = BTreeMap::new();
            let mut patch: BTreeMap<u64, Patch> = BTreeMap::new();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                if wetread::icon_present(&f, span_min) {
                    if let Some(e) = wetread::right_edge(&f, min_ink, max_gap) {
                        edge.insert(i, e);
                    }
                    let p = wetread::icon_cut(&f);
                    cl.add(&p, (i, 0), radius);
                    patch.insert(i, p);
                }
                i += 1;
            }
            let dropped = cl.prune(min_members);
            eprintln!("{} clusters ({dropped} pruned below {min_members})", cl.c.len());
            for (k, c) in cl.c.iter().enumerate() {
                let mut h: BTreeMap<usize, usize> = BTreeMap::new();
                for (fi, _) in &c.members {
                    if let Some(e) = edge.get(fi) {
                        *h.entry(*e).or_default() += 1;
                    }
                }
                let mut v: Vec<(usize, usize)> = h.into_iter().collect();
                v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
                v.truncate(5);
                writeln!(
                    o,
                    "# cluster {k}: {} members, top edges {}",
                    c.n,
                    v.iter().map(|(e, n)| format!("{e}x{n}")).collect::<Vec<_>>().join(" ")
                )
                .unwrap();
            }
            cl.print_ascii(&mut o);
            if let Some(path) = arg(&args, "--templates") {
                let mut samples: BTreeMap<(usize, char), Vec<Patch>> = BTreeMap::new();
                for (k, c) in cl.c.iter().enumerate() {
                    let Some(name) = names.get(&k) else { continue };
                    for (fi, _) in &c.members {
                        samples.entry((0, *name)).or_default().push(patch[fi].clone());
                    }
                }
                for (k, v) in &samples {
                    eprintln!("icon {}: {} samples", k.1, v.len());
                }
                let mut w = std::fs::File::create(&path).unwrap_or_else(|e| die(&e.to_string()));
                Templates::from_samples(wetread::ICON.w, wetread::ICON.h, false, &samples)
                    .write(&mut w);
            }
        }

        // The cell grid, measured. Per edge bucket, the MEDIAN ink profile over
        // every clean droplet frame: backgrounds differ frame to frame and the
        // glyphs do not, so the median profile is the field's own geometry with
        // the scenery taken out of it. This is what replaces a guessed pitch.
        "wetgeom" => {
            let (sel, icons) = wetsel(&args);
            let mut acc: BTreeMap<usize, Vec<Vec<f32>>> = BTreeMap::new();
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                if let Some(e) = sel.edge_of(&f, &icons) {
                    acc.entry(e).or_default().push(wetread::ink_profile(&f));
                }
            }
            writeln!(o, "edge\tframes\tx\tmedian_ink").unwrap();
            for (e, rows) in &acc {
                for k in 0..(wetread::BAND.1 - wetread::BAND.0) {
                    let mut v: Vec<f32> = rows.iter().map(|p| p[k]).collect();
                    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    writeln!(
                        o,
                        "{e}\t{}\t{}\t{:.3}",
                        rows.len(),
                        wetread::BAND.0 + k,
                        v[v.len() / 2]
                    )
                    .unwrap();
                }
            }
        }

        // Build the digit alphabet with no eye-labelling at all.
        //
        // Three digits can only be `100`, so the 3-digit frames hand over a `1`
        // and two `0`s for free. Everything else is named by the dry-out law:
        // a gradual dry-out steps the units digit down by one about every six
        // frames, so the temporal succession of the units-cell clusters spells
        // 1, 0, 9, 8, ... and the seeded `0` says where the chain starts. The
        // seeded `1` is then a CHECK, not an input: the chain must land on it.
        "wetalpha" => {
            let (sel, icons) = wetsel(&args);
            let radius: f32 = num(&args, "--radius", 0.82);
            let min_members: usize = num(&args, "--min-members", 6);
            let seed_min: f32 = num(&args, "--seed-min", 0.60);
            let dwell: u64 = num(&args, "--dwell", 4);
            let mut cells: BTreeMap<(u64, usize), Patch> = BTreeMap::new();
            let mut shape: BTreeMap<u64, (usize, usize)> = BTreeMap::new();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                if let Some(e) = sel.edge_of(&f, &icons) {
                    let n = wetread::digits_at(e, sel.edge_tol).unwrap();
                    shape.insert(i, (e, n));
                    for k in 0..n {
                        cells.insert(
                            (i, k),
                            Patch::cut(&f, &wetread::cell_at(k), 0, 0, 0),
                        );
                    }
                }
                i += 1;
            }
            let n3 = shape.values().filter(|(_, n)| *n == 3).count();
            let n2 = shape.values().filter(|(_, n)| *n == 2).count();
            let n1 = shape.values().filter(|(_, n)| *n == 1).count();
            writeln!(o, "# readable frames: {n1} one-digit, {n2} two-digit, {n3} three-digit").unwrap();
            if n3 == 0 {
                die("no three-digit frames: nothing to seed the alphabet with");
            }
            // Seed. Every 3-digit frame is `100`.
            let mut seed: BTreeMap<(usize, char), Vec<Patch>> = BTreeMap::new();
            for (fi, (_, n)) in shape.iter() {
                if *n != 3 {
                    continue;
                }
                seed.entry((0, '1')).or_default().push(cells[&(*fi, 0)].clone());
                seed.entry((0, '0')).or_default().push(cells[&(*fi, 1)].clone());
                seed.entry((0, '0')).or_default().push(cells[&(*fi, 2)].clone());
            }
            let st = Templates::from_samples(wetread::CELL_W, wetread::CELL_H, false, &seed);
            // The seed's own control: how tightly its samples agree with the
            // template they were averaged into, and how far apart the two
            // templates are. A `1` and a `0` that correlate highly would mean
            // the seed cannot tell them apart and nothing downstream can.
            for (k, list) in &seed {
                let q = &st.g[k];
                let mut c: Vec<f32> = list.iter().map(|p| p.dot(q)).collect();
                c.sort_by(|a, b| a.partial_cmp(b).unwrap());
                writeln!(
                    o,
                    "# seed '{}': {} samples, self-correlation min {:.3} median {:.3}",
                    k.1,
                    c.len(),
                    c[0],
                    c[c.len() / 2]
                )
                .unwrap();
            }
            writeln!(o, "# seed '1' vs '0': {:.3}", st.g[&(0, '1')].dot(&st.g[&(0, '0')])).unwrap();
            // Cluster every cell of every readable frame.
            let mut cl = glyphs::Clusters::new(wetread::CELL_W, wetread::CELL_H);
            for ((fi, k), p) in cells.iter() {
                cl.add(p, (*fi, *k), radius);
            }
            let dropped = cl.prune(min_members);
            writeln!(o, "# {} clusters over {} cells ({dropped} pruned below {min_members})", cl.c.len(), cells.len()).unwrap();
            // Which cluster is which, as far as the seed can say.
            let mut named: Vec<Option<char>> = vec![None; cl.c.len()];
            for (ci, c) in cl.c.iter().enumerate() {
                let p = Patch { w: cl.w, h: cl.h, v: c.mean.clone() };
                let mut best = ('?', -2.0f32);
                for ((_, ch), q) in st.g.iter() {
                    let s = p.dot(q);
                    if s > best.1 {
                        best = (*ch, s);
                    }
                }
                if best.1 >= seed_min {
                    named[ci] = Some(best.0);
                }
                writeln!(o, "# cluster {ci}: {} members, closest seed '{}' {:.3}", c.n, best.0, best.1).unwrap();
            }
            // The units cell, in time order: cluster of the LAST digit.
            let mut of_cell: BTreeMap<(u64, usize), usize> = BTreeMap::new();
            for (ci, c) in cl.c.iter().enumerate() {
                for m in &c.members {
                    of_cell.insert(*m, ci);
                }
            }
            let mut units: Vec<(u64, usize)> = Vec::new();
            for (fi, (_, n)) in shape.iter() {
                if let Some(ci) = of_cell.get(&(*fi, n - 1)) {
                    units.push((*fi, *ci));
                }
            }
            units.sort();
            // Successions, counted only where the source cluster had held for
            // `dwell` frames. A dry-out holds each units digit for about six
            // frames; a car entering water sweeps through them in one, so the
            // dwell is what separates the descending chain from the noise.
            let mut succ: BTreeMap<(usize, usize), usize> = BTreeMap::new();
            let mut held = 0u64;
            for w in units.windows(2) {
                let (fa, ca) = w[0];
                let (fb, cb) = w[1];
                if fb != fa + 1 {
                    held = 0;
                    continue;
                }
                if ca == cb {
                    held += 1;
                    continue;
                }
                if held + 1 >= dwell {
                    *succ.entry((ca, cb)).or_default() += 1;
                }
                held = 0;
            }
            let mut top: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
            for ((a, b), n) in &succ {
                let e = top.entry(*a).or_insert((*b, 0));
                if *n > e.1 {
                    *e = (*b, *n);
                }
            }
            for (a, (b, n)) in &top {
                writeln!(o, "# succession: cluster {a} -> {b} ({n} times)").unwrap();
            }
            // Walk the chain from the seeded `0`: 0, 9, 8, ... back to 0.
            // The chain's anchor is the cluster the seed matches BEST, not the
            // first one over the bar: a background variant of `0` clears the
            // bar too, and starting the walk on one puts every name one step
            // out.
            let mut zero: Option<(usize, f32)> = None;
            for (ci, c) in cl.c.iter().enumerate() {
                if named[ci] != Some('0') {
                    continue;
                }
                let s = Patch { w: cl.w, h: cl.h, v: c.mean.clone() }.dot(&st.g[&(0, '0')]);
                if zero.map_or(true, |(_, b)| s > b) {
                    zero = Some((ci, s));
                }
            }
            let Some((zero, _)) = zero else {
                die("the seed named no cluster '0'; nothing to anchor the chain on")
            };
            let mut chain: Vec<usize> = vec![zero];
            let mut at = zero;
            for _ in 0..9 {
                let Some((nx, _)) = top.get(&at).copied() else { break };
                if chain.contains(&nx) {
                    break;
                }
                chain.push(nx);
                at = nx;
            }
            let mut label: BTreeMap<usize, char> = BTreeMap::new();
            for (step, ci) in chain.iter().enumerate() {
                let d = (10 - step) % 10;
                label.insert(*ci, char::from_digit(d as u32, 10).unwrap());
            }
            writeln!(
                o,
                "# chain from cluster {zero}: {}",
                chain.iter().enumerate().map(|(s, c)| format!("{c}='{}'", (10 - s) % 10)).collect::<Vec<_>>().join(" ")
            )
            .unwrap();
            // The check the seed is for: the chain's `1` must be the cluster
            // the seed independently called `1`.
            let chain_one = label.iter().find(|(_, c)| **c == '1').map(|(k, _)| *k);
            let seed_one = named.iter().position(|c| *c == Some('1'));
            writeln!(
                o,
                "# CHECK chain '1' = cluster {:?}, seed '1' = cluster {:?} -- {}",
                chain_one,
                seed_one,
                if chain_one.is_some() && chain_one == seed_one { "AGREE" } else { "DISAGREE" }
            )
            .unwrap();
            // Merge the leftovers. A cluster outside the chain is the same
            // glyph over a background the radius would not forgive, so it
            // joins the named shape it correlates best with -- or, if nothing
            // is close, it stays unnamed and its frames go unread. That is the
            // whole of "merge the background variants": a radius question, not
            // a new idea.
            let merge_min: f32 = num(&args, "--merge-min", 0.75);
            let base: Vec<(char, Vec<f32>)> =
                label.iter().map(|(ci, ch)| (*ch, cl.c[*ci].mean.clone())).collect();
            for ci in 0..cl.c.len() {
                if label.contains_key(&ci) {
                    continue;
                }
                let p = Patch { w: cl.w, h: cl.h, v: cl.c[ci].mean.clone() };
                let mut best = ('?', -2.0f32);
                for (ch, mean) in &base {
                    let s = p.dot(&Patch { w: cl.w, h: cl.h, v: mean.clone() });
                    if s > best.1 {
                        best = (*ch, s);
                    }
                }
                writeln!(
                    o,
                    "# leftover cluster {ci} ({} members): closest named '{}' {:.3} -- {}",
                    cl.c[ci].n,
                    best.0,
                    best.1,
                    if best.1 >= merge_min { "merged" } else { "UNNAMED, its frames go unread" }
                )
                .unwrap();
                if best.1 >= merge_min {
                    label.insert(ci, best.0);
                }
            }
            for (ci, c) in cl.c.iter().enumerate() {
                writeln!(o, "# cluster {ci} named {:?} ({} members)", label.get(&ci), c.n).unwrap();
            }
            cl.print_ascii(&mut o);
            if let Some(path) = arg(&args, "--templates") {
                let mut samples: BTreeMap<(usize, char), Vec<Patch>> = BTreeMap::new();
                for (ci, c) in cl.c.iter().enumerate() {
                    let Some(ch) = label.get(&ci) else { continue };
                    for m in &c.members {
                        samples.entry((0, *ch)).or_default().push(cells[m].clone());
                    }
                }
                for (k, v) in &samples {
                    eprintln!("glyph {}: {} samples", k.1, v.len());
                }
                let mut w = std::fs::File::create(&path).unwrap_or_else(|e| die(&e.to_string()));
                Templates::from_samples(wetread::CELL_W, wetread::CELL_H, false, &samples).write(&mut w);
            }
        }

        "wetread" => {
            let (sel, icons) = wetsel(&args);
            let t = Templates::read(
                &std::fs::read_to_string(need(&args, "--templates"))
                    .unwrap_or_else(|e| die(&e.to_string())),
            );
            let digit_min: f32 = num(&args, "--digit-min", 0.55);
            let margin_min: f32 = num(&args, "--margin-min", 0.06);
            writeln!(o, "t\tpct\ttext\tedge\tworst\tmargin").unwrap();
            let mut i = 0u64;
            while f.read_from(&mut r).unwrap_or_else(|e| die(&e.to_string())) {
                match wetread::read(&f, &icons, &t, &sel, digit_min, margin_min) {
                    None => writeln!(o, "{:.4}\t\t\t\t\t", at(i)).unwrap(),
                    Some(rd) => writeln!(
                        o,
                        "{:.4}\t{}\t{}\t{}\t{:.3}\t{:.3}",
                        at(i),
                        rd.value.map(|v| v.to_string()).unwrap_or_default(),
                        rd.text,
                        rd.edge,
                        rd.worst,
                        rd.margin
                    )
                    .unwrap(),
                }
                i += 1;
            }
        }

        // The acceptance gate. Reads percentage series -- decoded ones and, as
        // the control, human replays converted from telemetry -- and reports
        // what fraction of adjacent pairs the dry-out law refuses.
        "wetlaw" => {
            let max_gap: f64 = num(&args, "--max-gap", 0.06);
            let g = wetlaw::Gate {
                drop_rate: num(&args, "--drop-rate", 10.5),
                rise_rate: num(&args, "--rise-rate", 80.0),
                max_gap,
                slack: num(&args, "--slack", 1),
                sample: num::<f64>(&args, "--sample-ms", 50.0) / 1000.0,
                reset_win: num::<f64>(&args, "--reset-ms", 100.0) / 1000.0,
                reset_frac: num(&args, "--reset-frac", 0.15),
            };
            let show: usize = num(&args, "--show", 10);
            let corrupt: f64 = num(&args, "--corrupt", 0.0);
            let truncate = args.iter().any(|a| a == "--truncate");
            let tcol: usize = num(&args, "--tcol", 0);
            let vcol: usize = num(&args, "--vcol", 1);
            let fps_out: f64 = num(&args, "--series-fps", 60.0);
            let mut any = false;
            for spec in args.iter().skip(1) {
                let (kind, path) = match spec.split_once(':') {
                    Some(("tel", p)) => ("tel", p),
                    Some(("pct", p)) => ("pct", p),
                    _ => continue,
                };
                let text = std::fs::read_to_string(path).unwrap_or_else(|e| die(&e.to_string()));
                let mut s = match kind {
                    "tel" => wetlaw::from_telemetry(&text, fps_out, truncate),
                    _ => wetlaw::load(&text, tcol, vcol),
                };
                if corrupt > 0.0 {
                    s = wetlaw::corrupt(&s, corrupt, 0x5eed_1234);
                }
                let name = format!("{path}{}", if corrupt > 0.0 { " CORRUPTED" } else { "" });
                wetlaw::print_envelope(&name, &wetlaw::envelope(&s, max_gap, g.reset_win, g.reset_frac), &mut o);
                wetlaw::print_report(&name, &wetlaw::check(&s, &g), show, &mut o);
                any = true;
            }
            if !any {
                die("wetlaw wants one or more of tel:FILE (race ms + fraction) or pct:FILE (t + percent)");
            }
        }

        _ => die("usage: vidread lamps|sections|ink|patches|train|read|trace|weticon|wetedge|wetgeom|wetalpha|wetread|wetlaw ..."),
    }
}
