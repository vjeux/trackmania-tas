//! `lmtool harness-gate` — THE FIVE CHECKS + TWO-RUN + GUARDS as one command (baker-5, 2026-09-28): the landing gate
//! that baker-3/4 typed as a shell chain at every base, so it carries its pass criteria every time and prints one table.
//!
//!     lmtool harness-gate --pd DIR --bin LMTOOL --tag NAME [--paks DIR] [--guards DIR] [--skip-guards] [--skip-two-run]
//!                         [--g8-threads 128] [--md OUT.md]
//!
//! `--pd DIR` holds the harness inputs and the reference set (mh/harness-refs-<tip>/ unpacked: INPUTS.md5, COMMON.txt,
//! detp1.Map.Gbx, g8-old.Map.Gbx, dump-ref/, dump-tref/, prb-c3/ + the five inputs INPUTS.md5 names). `--bin` = the
//! candidate binary that BAKES (this driver measures: passdiff / diff / filecheck are run through `--bin` too, so the
//! comparison code is the candidate's own — the same convention as corpus-gate's --bake-with). Every bake is pinned with
//! LMTOOL_BAKE_TIME=1790000000 so a file compare is a byte compare.
//!
//! Checks (each PASS/FAIL, all run even after a failure):
//!   inputs     md5 of the five harness inputs vs INPUTS.md5
//!   detp1      bake t16 --raster --quality 4 --game-peel --max-dirs 12          cmp detp1.Map.Gbx
//!   g8         bake g23 --raster --quality 4 --game-peel --max-dirs 8 (128 thr)  cmp g8-old.Map.Gbx
//!   dump-tref  bake t16 … --max-dirs 6 --dump-passes … --dump-dirs 0 --dump-lightsum-after 5
//!              passdiff dump-tref NEW --stride 1 --floor 0 --tol 0 --require 10  every row max 0 / 100.00 %
//!   dump-ref   bake $(COMMON.txt) --dump-passes …  passdiff dump-ref NEW … --require 10
//!   two-run    the COMMON dump a second time, passdiff run1 run2 (determinism)
//!   probes     LMTOOL_PROBE_DUMP_DIR=… bake $(COMMON.txt) --per-subsample --probes transcribed
//!              cmp probe-colour/updown/skyvis .f32 vs prb-c3/
//!   guards     the three d1 product bakes (stpad Night nocache q3, tiny16 kept q4, tiny04ac Day q4; full pak set) cmp'd
//!              against `--guards DIR`/{stpad,tiny16,tiny04ac}-d1-base.Map.Gbx; on a mismatch `diff` + `filecheck` say
//!              what moved (a FILETIME-only difference = an unpinned baseline, reported as such)
//!
//! A passdiff table row is read as PASS only when its max abs is 0.0000 AND its within-±0 column is 100.00 %; a passdiff
//! that compared nothing fails by `--require` (baker-3's false-pass lesson). Exit 1 if any check fails.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const PIN: &str = "1790000000";

struct Check {
    name: String,
    pass: bool,
    secs: f64,
    note: String,
}

fn flag(args: &[String], k: &str) -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() }
fn has(args: &[String], k: &str) -> bool { args.iter().any(|a| a == k) }

fn pak_key(coll: &str) -> &'static str {
    match coll { "Stadium" => "B773D73047A4104857722366D78D28A6", "Maniaplanet" => "9A93723447347A8CE336CCFC49E65449", _ => "660C4C156B80337E296A1034B0AA05B8" }
}

/// Run `bin args…` with the bake-time pin (+ extra env), stderr+stdout to `log`; Ok(secs) on success.
fn run_bin(bin: &Path, cwd: &Path, args: &[String], env: &[(&str, String)], log: &Path) -> Result<f64, String> {
    let t = Instant::now();
    let f = std::fs::File::create(log).map_err(|e| format!("{}: {e}", log.display()))?;
    let f2 = f.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(bin);
    cmd.args(args).current_dir(cwd).env("LMTOOL_BAKE_TIME", PIN).stdout(f).stderr(f2);
    for (k, v) in env { cmd.env(k, v); }
    let st = cmd.status().map_err(|e| format!("{}: {e}", bin.display()))?;
    if !st.success() {
        let tail = std::fs::read_to_string(log).unwrap_or_default().lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
        return Err(format!("exit {st}: {tail}"));
    }
    Ok(t.elapsed().as_secs_f64())
}

fn files_equal(a: &Path, b: &Path) -> Result<bool, String> {
    let (x, y) = (std::fs::read(a).map_err(|e| format!("{}: {e}", a.display()))?, std::fs::read(b).map_err(|e| format!("{}: {e}", b.display()))?);
    Ok(x == y)
}

/// `bin passdiff REF NEW --stride 1 --floor 0 --tol 0 --require 10`: PASS = exit 0 and every table row identical.
fn passdiff(bin: &Path, cwd: &Path, reference: &Path, new: &Path, log: &Path) -> Result<(bool, String), String> {
    let o = Command::new(bin).args(["passdiff", &reference.to_string_lossy(), &new.to_string_lossy(), "--stride", "1", "--floor", "0", "--tol", "0", "--require", "10"]).current_dir(cwd).output().map_err(|e| format!("passdiff: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    let _ = std::fs::write(log, &text);
    let mut rows = 0usize;
    let mut bad: Vec<String> = Vec::new();
    for l in text.lines() {
        // | pass | buffers | texels | max abs | mean abs | RMSE | mean Δ | within ±0 % | conventions |
        if !l.starts_with("| ") || l.starts_with("| pass ") || l.starts_with("|---") { continue; }
        let cols: Vec<&str> = l.split('|').map(|c| c.trim()).collect();
        if cols.len() < 9 { continue; }
        rows += 1;
        let max_abs = cols[4];
        let within = cols[8];
        if max_abs != "0.0000" || within != "100.00 %" { bad.push(format!("{} max {max_abs} within {within}", cols[1])); }
    }
    let compared = text.lines().rev().find(|l| l.starts_with("passdiff:")).unwrap_or("").trim().to_string();
    let ok = o.status.success() && rows > 0 && bad.is_empty();
    let note = if bad.is_empty() { format!("{rows} rows identical; {compared}") } else { format!("{} of {rows} rows MOVED: {}; {compared}", bad.len(), bad.join(", ")) };
    Ok((ok, if o.status.success() { note } else { format!("passdiff exit {}: {note}", o.status) }))
}

fn tail_of(bin: &Path, args: &[String], n: usize) -> String {
    let o = Command::new(bin).args(args).output();
    match o {
        Ok(o) => { let s = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); s.lines().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n") }
        Err(e) => format!("({e})"),
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let pd = PathBuf::from(flag(args, "--pd").ok_or("--pd DIR (the harness inputs + the reference set)")?);
    let bin = PathBuf::from(flag(args, "--bin").ok_or("--bin LMTOOL (the candidate binary)")?);
    let tag = flag(args, "--tag").ok_or("--tag NAME (the work directory DIR/gate-NAME)")?;
    if !bin.is_file() { return Err(format!("--bin {}: no such file", bin.display())); }
    let paks = PathBuf::from(flag(args, "--paks").unwrap_or_else(|| "/tmp/paks".into()));
    let guards = flag(args, "--guards").map(PathBuf::from).unwrap_or_else(|| pd.join("guard"));
    let g8_threads = flag(args, "--g8-threads").unwrap_or_else(|| "128".into());
    let work = pd.join(format!("gate-{tag}"));
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let t_all = Instant::now();
    let mut checks: Vec<Check> = Vec::new();
    let mut push = |name: &str, r: Result<(bool, f64, String), String>| {
        let c = match r { Ok((p, s, n)) => Check { name: name.into(), pass: p, secs: s, note: n }, Err(e) => Check { name: name.into(), pass: false, secs: 0.0, note: e } };
        println!("{:<10} {}  {:7.1} s  {}", c.name, if c.pass { "PASS" } else { "FAIL" }, c.secs, c.note);
        checks.push(c);
    };
    let s = |p: &Path| p.to_string_lossy().to_string();

    // inputs
    push("inputs", (|| {
        let t = Instant::now();
        let list = std::fs::read_to_string(pd.join("INPUTS.md5")).map_err(|e| format!("INPUTS.md5: {e}"))?;
        let mut bad = Vec::new();
        let mut n = 0;
        for l in list.lines() {
            let mut it = l.split_whitespace();
            let (Some(want), Some(name)) = (it.next(), it.next()) else { continue };
            n += 1;
            let data = std::fs::read(pd.join(name)).map_err(|e| format!("{name}: {e}"))?;
            if crate::corpusgate::md5_hex(&data) != want { bad.push(name.to_string()); }
        }
        Ok((bad.is_empty() && n > 0, t.elapsed().as_secs_f64(), if bad.is_empty() { format!("{n} inputs md5 OK") } else { format!("md5 MISMATCH: {}", bad.join(", ")) }))
    })());

    // detp1
    push("detp1", (|| {
        let out = work.join("detp1.Map.Gbx");
        let secs = run_bin(&bin, &pd, &["bake".into(), "t16.Map.Gbx".into(), "--raster".into(), "--quality".into(), "4".into(), "--game-peel".into(), "--max-dirs".into(), "12".into(), "--out".into(), s(&out)], &[], &work.join("detp1.log"))?;
        let eq = files_equal(&pd.join("detp1.Map.Gbx"), &out)?;
        Ok((eq, secs, if eq { "byte-identical".into() } else { format!("DIFFERS: {}", tail_of(&bin, &["diff".into(), s(&pd.join("detp1.Map.Gbx")), s(&out)], 3).replace('\n', " | ")) }))
    })());

    // g8
    push("g8", (|| {
        let out = work.join("g8.Map.Gbx");
        let secs = run_bin(&bin, &pd, &["bake".into(), "g23.Map.Gbx".into(), "--raster".into(), "--quality".into(), "4".into(), "--game-peel".into(), "--max-dirs".into(), "8".into(), "--out".into(), s(&out)], &[("LMTOOL_THREADS", g8_threads.clone())], &work.join("g8.log"))?;
        let eq = files_equal(&pd.join("g8-old.Map.Gbx"), &out)?;
        Ok((eq, secs, if eq { "byte-identical".into() } else { format!("DIFFERS: {}", tail_of(&bin, &["diff".into(), s(&pd.join("g8-old.Map.Gbx")), s(&out)], 3).replace('\n', " | ")) }))
    })());

    // dump-tref
    push("dump-tref", (|| {
        let d = work.join("dump-tref");
        let _ = std::fs::remove_dir_all(&d);
        let secs = run_bin(&bin, &pd, &["bake".into(), "t16.Map.Gbx".into(), "--raster".into(), "--quality".into(), "4".into(), "--game-peel".into(), "--max-dirs".into(), "6".into(), "--dump-passes".into(), s(&d), "--dump-dirs".into(), "0".into(), "--dump-lightsum-after".into(), "5".into(), "--out".into(), s(&work.join("x-tref.Map.Gbx"))], &[], &work.join("dump-tref.log"))?;
        let (ok, note) = passdiff(&bin, &pd, &pd.join("dump-tref"), &d, &work.join("passdiff-tref.log"))?;
        Ok((ok, secs, note))
    })());

    // dump-ref + two-run
    let common: Vec<String> = std::fs::read_to_string(pd.join("COMMON.txt")).map_err(|e| format!("COMMON.txt: {e}"))?.split_whitespace().map(|x| x.to_string()).collect();
    let run_common = |suffix: &str| -> Result<(PathBuf, f64), String> {
        let d = work.join(format!("dump-ref{suffix}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut a: Vec<String> = vec!["bake".into()];
        a.extend(common.iter().cloned());
        a.extend(["--dump-passes".into(), s(&d), "--out".into(), s(&work.join(format!("ours-ref{suffix}.Map.Gbx")))]);
        let secs = run_bin(&bin, &pd, &a, &[], &work.join(format!("dump-ref{suffix}.log")))?;
        Ok((d, secs))
    };
    let first = run_common("");
    push("dump-ref", (|| { let (d, secs) = first.as_ref().map_err(|e| e.clone())?.clone(); let (ok, note) = passdiff(&bin, &pd, &pd.join("dump-ref"), &d, &work.join("passdiff-ref.log"))?; Ok((ok, secs, note)) })());
    if !has(args, "--skip-two-run") {
        push("two-run", (|| { let (d1, _) = first.as_ref().map_err(|e| e.clone())?.clone(); let (d2, secs) = run_common("-2")?; let (ok, note) = passdiff(&bin, &pd, &d1, &d2, &work.join("passdiff-two-run.log"))?; Ok((ok, secs, note)) })());
    }

    // probes
    push("probes", (|| {
        let d = work.join("prb");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let mut a: Vec<String> = vec!["bake".into()];
        a.extend(common.iter().cloned());
        a.extend(["--per-subsample".into(), "--probes".into(), "transcribed".into(), "--out".into(), s(&work.join("xp.Map.Gbx"))]);
        let secs = run_bin(&bin, &pd, &a, &[("LMTOOL_PROBE_DUMP_DIR", s(&d))], &work.join("probes.log"))?;
        let mut bad = Vec::new();
        for f in ["probe-colour", "probe-updown", "probe-skyvis"] { if !files_equal(&pd.join("prb-c3").join(format!("{f}.f32")), &d.join(format!("{f}.f32")))? { bad.push(f); } }
        Ok((bad.is_empty(), secs, if bad.is_empty() { "3 volumes byte-identical".into() } else { format!("DIFFER: {}", bad.join(", ")) }))
    })());

    // guards: the three d1 product bakes vs the pinned baselines in `guards`
    if !has(args, "--skip-guards") {
        let pak = |c: &str| format!("{}:{}", paks.join(format!("{c}.pak")).to_string_lossy(), pak_key(c));
        let recipes: [(&str, Vec<String>); 3] = [
            ("stpad", vec!["bake".into(), s(&guards.join("stpad-night-0x199a-source.Map.Gbx")), "--raster".into(), "--lm-from-map".into(), "--collection".into(), "Stadium".into(), "--quality".into(), "3".into(), "--pak".into(), pak("Stadium"), "--pak".into(), pak("Maniaplanet"), "--max-dirs".into(), "1".into()]),
            ("tiny16", vec!["bake".into(), s(&guards.join("tiny16-fixedlib-source.Map.Gbx")), "--raster".into(), "--lm-from-map".into(), "--collection".into(), "BlueBay".into(), "--quality".into(), "4".into(), "--pak".into(), pak("BlueBay"), "--pak".into(), pak("Stadium"), "--pak".into(), pak("Maniaplanet"), "--kept".into(), s(&guards.join("tiny16-reduced-kept.txt")), "--max-dirs".into(), "1".into()]),
            ("tiny04ac", vec!["bake".into(), s(&guards.join("tiny04ac-GreenCoast-Day-q4-editor.Map.Gbx")), "--raster".into(), "--lm-from-map".into(), "--collection".into(), "GreenCoast".into(), "--quality".into(), "4".into(), "--pak".into(), pak("GreenCoast"), "--pak".into(), pak("Stadium"), "--pak".into(), pak("Maniaplanet"), "--max-dirs".into(), "1".into()]),
        ];
        for (name, mut a) in recipes {
            let label = format!("guard-{name}");
            push(&label, (|| {
                let base = guards.join(format!("{name}-d1-base.Map.Gbx"));
                if !base.is_file() { return Err(format!("no baseline {}", base.display())); }
                let out = work.join(format!("{name}-d1.Map.Gbx"));
                a.extend(["--records-tsv".into(), s(&work.join(format!("{name}-d1.tsv"))), "--out".into(), s(&out)]);
                let secs = run_bin(&bin, &work, &a, &[], &work.join(format!("{name}-d1.log")))?;
                let eq = files_equal(&base, &out)?;
                if eq { return Ok((true, secs, "byte-identical to the baseline".into())); }
                let d = tail_of(&bin, &["diff".into(), s(&base), s(&out)], 4);
                let filetime_only = d.contains("0x6022013") && d.contains("lossless parts: 16 differing bytes");
                let fc = tail_of(&bin, &["filecheck".into(), s(&out), "--against".into(), s(&base)], 14);
                let images_ok = fc.lines().filter(|l| l.contains("image")).all(|l| l.contains("BYTE-IDENTICAL")) && fc.contains("same bind words") && !fc.lines().any(|l| l.contains("bytes:") && !l.contains("max |Δ| 0"));
                Ok((false, secs, if filetime_only && images_ok { "MOVED in the FILETIME word only (an unpinned baseline? re-cut it with LMTOOL_BAKE_TIME)".into() } else { format!("MOVED — {}", d.replace('\n', " | ")) }))
            })());
        }
    }

    // the table + the verdict
    let all = checks.iter().all(|c| c.pass);
    let mut md = format!("# harness-gate {tag} — {} ({:.0} s)\n\n| check | result | s | note |\n|---|---|---|---|\n", if all { "ALL PASS" } else { "FAILED" }, t_all.elapsed().as_secs_f64());
    for c in &checks { md.push_str(&format!("| {} | {} | {:.1} | {} |\n", c.name, if c.pass { "PASS" } else { "FAIL" }, c.secs, c.note.replace('|', "/"))); }
    md.push_str(&format!("\nbinary {} md5 {}; pd {}; guards {}\n", bin.display(), crate::corpusgate::md5_hex(&std::fs::read(&bin).unwrap_or_default()), pd.display(), guards.display()));
    let md_path = flag(args, "--md").map(PathBuf::from).unwrap_or_else(|| work.join("GATE.md"));
    std::fs::write(&md_path, &md).map_err(|e| format!("{}: {e}", md_path.display()))?;
    println!("harness-gate {tag}: {} in {:.0} s — {}", if all { "ALL PASS" } else { "FAILED" }, t_all.elapsed().as_secs_f64(), md_path.display());
    if all { Ok(()) } else { Err(format!("{} check(s) failed", checks.iter().filter(|c| !c.pass).count())) }
}
