//! `fk tickhook` — the engine's own tick as the fork server's clock: check it,
//! count it, load-test it, and find it again on a new build.
//!
//! Four verbs, four different questions:
//!
//! * `check` — *do the build constants match this server binary?* Static, reads
//!   the ELF on disk through the same `tickhook_sig` the shim uses. Cheap, and
//!   the first thing to run on a new `TrackmaniaServer`.
//! * `count` — *is the hook entered exactly once per tick, and does it change
//!   nothing?* One plain run and one hooked run of the same tape: the hooked
//!   run must report `tick_total == (sim_end - sim_start) / 10`, zero `dt != 10`
//!   entries, zero disagreements with the simulation's own clock, and the same
//!   validated time as the plain run. `--gdb` adds an independent count of the
//!   loop's clock write under a debugger.
//! * `load` — *is the stop the same simulation point on N servers started at
//!   once?* Under the lroundf clock 104 of 150 were one tick late. Under the
//!   tick hook every server must report the same `sim_ms` AND the page-fault
//!   probe (the control, measured from the other side) must agree on every
//!   one.
//! * `find` — *on a build with different offsets, which function is the tick?*
//!   Dynamic first: the probe's fault log gives the return address inside the
//!   tick loop; the `call rel32` targets near it are the candidates; each is
//!   hooked in the shim's finder mode and kept only if it behaves as the tick
//!   (`count`'s criteria). Prints the constants for `tickhook_sig.rs`.

use crate::session::{Checkpoint, Engine, Session};
use crate::tape::Tape;
use forkoracle::clock;
use forkoracle::tickhook_sig::{
    prologue_displaced_len, tick_hook_signatures_match, TICK_CALL_SITE_OFF, TICK_CLOCK_WRITE_OFF,
    TICK_FN_OFF, TICK_FN_SIGNATURE,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ------------------------------------------------------------------ the ELF

/// The server binary with its PT_LOAD map, so an RVA can be read off disk.
pub struct Elf {
    bytes: Vec<u8>,
    /// (vaddr, file offset, filesz, executable)
    loads: Vec<(usize, usize, usize, bool)>,
}

impl Elf {
    pub fn open(path: &Path) -> Result<Elf, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        if bytes.len() < 64 || &bytes[..4] != b"\x7fELF" || bytes[4] != 2 {
            return Err(format!("{} is not a 64-bit ELF", path.display()));
        }
        let u16at = |o: usize| u16::from_le_bytes(bytes[o..o + 2].try_into().unwrap()) as usize;
        let u32at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let u64at = |o: usize| u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap()) as usize;
        let (phoff, phentsize, phnum) = (u64at(0x20), u16at(0x36), u16at(0x38));
        let mut loads = Vec::new();
        for i in 0..phnum {
            let p = phoff + i * phentsize;
            if p + 56 > bytes.len() {
                break;
            }
            if u32at(p) == 1 {
                let flags = u32at(p + 4);
                loads.push((u64at(p + 16), u64at(p + 8), u64at(p + 32), flags & 1 != 0));
            }
        }
        if loads.is_empty() {
            return Err("no PT_LOAD segments".into());
        }
        Ok(Elf { bytes, loads })
    }

    /// File offset of an RVA, if it is inside a loaded segment.
    pub fn file_off(&self, rva: usize) -> Option<usize> {
        self.loads
            .iter()
            .find(|(va, _, sz, _)| rva >= *va && rva < va + sz)
            .map(|(va, off, _, _)| off + (rva - va))
    }

    pub fn is_text(&self, rva: usize) -> bool {
        self.loads.iter().any(|(va, _, sz, x)| *x && rva >= *va && rva < va + sz)
    }

    /// `n` bytes at `rva`, zero-filled where unmapped (so a signature check
    /// against an unmapped offset FAILS rather than panics).
    pub fn read(&self, rva: usize, n: usize) -> Vec<u8> {
        match self.file_off(rva) {
            Some(o) if o + n <= self.bytes.len() => self.bytes[o..o + n].to_vec(),
            _ => vec![0; n],
        }
    }

    /// The executable segments as (rva, bytes).
    pub fn text_segments(&self) -> Vec<(usize, &[u8])> {
        self.loads
            .iter()
            .filter(|l| l.3)
            .map(|(va, off, sz, _)| (*va, &self.bytes[*off..(*off + *sz).min(self.bytes.len())]))
            .collect()
    }
}

// ---------------------------------------------------------------- fk tickhook check

pub fn check(server: &Path) -> Result<(), String> {
    let bin = server.join("TrackmaniaServer");
    let elf = Elf::open(&bin)?;
    let read = |o: usize, n: usize| elf.read(o, n);
    println!("binary: {} ({} bytes)", bin.display(), elf.bytes.len());
    println!(
        "tick function  {:#x}: {}",
        TICK_FN_OFF,
        hex(&elf.read(TICK_FN_OFF, TICK_FN_SIGNATURE.len()))
    );
    println!("loop call site {:#x}: {}", TICK_CALL_SITE_OFF, hex(&elf.read(TICK_CALL_SITE_OFF, 8)));
    println!("loop clock wr  {:#x}: {}", TICK_CLOCK_WRITE_OFF, hex(&elf.read(TICK_CLOCK_WRITE_OFF, 7)));
    match tick_hook_signatures_match(&read) {
        Ok(()) => {
            println!("SIGNATURES MATCH: the shim will hook this build");
            Ok(())
        }
        Err(what) => Err(format!(
            "SIGNATURE MISMATCH at the {} -- the shim will refuse this build (exit 92). \
             Run `fk tickhook find` and update forkoracle/src/tickhook_sig.rs",
            what
        )),
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- the shim's report

/// What the shim prints on stderr at exit.
#[derive(Debug, Default, Clone)]
pub struct ShimReport {
    pub lroundf_total: u64,
    pub tick_total: u64,
    pub sim_ms0: u64,
    pub sim_ms_end: u64,
    pub anomalies: u64,
    pub clock_mismatch: u64,
    pub fn_off: u64,
    pub race_start: u64,
    pub clock_total: u64,
    pub refused: Option<String>,
}

pub fn parse_shim_report(stderr: &str) -> ShimReport {
    let mut r = ShimReport::default();
    for l in stderr.lines() {
        let get = |k: &str| l.strip_prefix(k).and_then(|v| v.trim().parse::<u64>().ok());
        if let Some(v) = get("FKSHIM lroundf_total ") {
            r.lroundf_total = v;
        } else if let Some(v) = get("FKSHIM tick_total ") {
            r.tick_total = v;
        } else if let Some(v) = get("FKSHIM sim_ms0 ") {
            r.sim_ms0 = v;
        } else if let Some(v) = get("FKSHIM sim_ms_end ") {
            r.sim_ms_end = v;
        } else if let Some(v) = get("FKSHIM tick_anomalies ") {
            r.anomalies = v;
        } else if let Some(v) = get("FKSHIM tick_clock_mismatch ") {
            r.clock_mismatch = v;
        } else if let Some(v) = get("FKSHIM tick_fn_off ") {
            r.fn_off = v;
        } else if let Some(v) = get("FKSHIM race_start ") {
            r.race_start = v;
        } else if let Some(v) = get("FKSHIM clock_total ") {
            r.clock_total = v;
        } else if l.contains("FKSHIM tickhook: REFUSED") {
            r.refused = Some(l.trim().to_string());
        }
    }
    r
}

impl ShimReport {
    /// The tick-hook consistency criteria. A wrong function fails at least one.
    pub fn consistent(&self) -> Result<(), String> {
        if let Some(r) = &self.refused {
            return Err(r.clone());
        }
        if self.tick_total < 100 {
            return Err(format!("only {} ticks", self.tick_total));
        }
        let span = self.sim_ms_end.saturating_sub(self.sim_ms0);
        if span / 10 != self.tick_total || span % 10 != 0 {
            return Err(format!(
                "tick_total {} != (sim_end {} - sim_start {}) / 10",
                self.tick_total, self.sim_ms_end, self.sim_ms0
            ));
        }
        if self.anomalies != 0 {
            return Err(format!("{} entries with dt != 10", self.anomalies));
        }
        if self.clock_mismatch != 0 {
            return Err(format!(
                "{} entries where the simulation's own clock was not new_time - 10",
                self.clock_mismatch
            ));
        }
        if self.race_start == u64::MAX || self.race_start == 0 || self.race_start < self.sim_ms0 || self.race_start > self.sim_ms_end {
            return Err(format!("race start {} not inside the simulation", self.race_start));
        }
        let want_total = ((self.sim_ms_end as i64 - self.race_start as i64).div_euclid(10) + clock::RACE_CLOCK_BIAS) as u64;
        if self.clock_total != want_total {
            return Err(format!("clock_total {} != (sim_end - race_start)/10 + bias = {}", self.clock_total, want_total));
        }
        Ok(())
    }
}

/// One full validation of `ghost` on `map` in a fresh directory, with `envs`
/// added; returns (validated time, stderr, wall seconds). `timeout` kills a
/// hooked server that wedged.
pub fn run_once(
    dir: &Path,
    server: &Path,
    map: &Path,
    ghost: &Path,
    envs: &[(&str, String)],
    timeout: Duration,
) -> Result<(Option<i64>, String, f64), String> {
    let replays = dir.join("UserData/Replays");
    let maps = dir.join("UserData/Maps");
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(&replays).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&maps).map_err(|e| e.to_string())?;
    let link = |t: PathBuf, a: PathBuf| {
        let _ = std::os::unix::fs::symlink(t, a);
    };
    link(server.join("Packs"), dir.join("Packs"));
    link(server.join("TrackmaniaServer"), dir.join("TrackmaniaServer"));
    let m = map.canonicalize().map_err(|e| format!("{}: {}", map.display(), e))?;
    link(m.clone(), maps.join(m.file_name().unwrap()));
    std::fs::copy(ghost, replays.join("g.Ghost.Gbx")).map_err(|e| e.to_string())?;
    let out = dir.join("stdout.log");
    let err = dir.join("stderr.log");
    let mut c = Command::new("./TrackmaniaServer");
    c.args(["/nodaemon", "/validatepath=."])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&out).map_err(|e| e.to_string())?)
        .stderr(std::fs::File::create(&err).map_err(|e| e.to_string())?);
    for (k, v) in envs {
        c.env(k, v);
    }
    let t0 = Instant::now();
    let mut child = c.spawn().map_err(|e| format!("spawn: {}", e))?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if t0.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("server did not finish within {:?}", timeout));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(e.to_string()),
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let stdout = std::fs::read_to_string(&out).unwrap_or_default();
    let stderr = std::fs::read_to_string(&err).unwrap_or_default();
    let (t, _) = forkoracle::forksrv::parse_result(&stdout);
    Ok((t, stderr, secs))
}

// ---------------------------------------------------------------- fk tickhook count

pub struct CountOpts {
    pub gdb: bool,
}

/// Plain run vs hooked run of the same tape; PASS only if every criterion holds.
pub fn count(engine: &Engine, tape: Tape, o: CountOpts) -> Result<(), String> {
    engine.check()?;
    std::fs::create_dir_all(&engine.work).map_err(|e| e.to_string())?;
    let g = engine.work.join("count.Ghost.Gbx");
    tape.write_reference(&g)?;
    let shim = engine.shim.canonicalize().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
    let to = Duration::from_secs(600);

    let (plain_t, _, plain_s) = run_once(&engine.work.join("plain"), &engine.server, &engine.map, &g, &[], to)?;
    let (hook_t, err, hook_s) = run_once(
        &engine.work.join("hooked"),
        &engine.server,
        &engine.map,
        &g,
        &[
            ("LD_PRELOAD", shim.clone()),
            ("FKSHIM_CLOCK", "tick".into()),
            ("FKSHIM_VALIDATOR_CAR", "1".into()),
        ],
        to,
    )?;
    let r = parse_shim_report(&err);
    println!(
        "plain run: {}  [{:.2}s]   hooked run: {}  [{:.2}s]",
        crate::secs_opt(plain_t),
        plain_s,
        crate::secs_opt(hook_t),
        hook_s
    );
    println!(
        "hook: {} ticks, sim {} -> {} ms, race start {}, dt!=10: {}, sim-clock disagreements: {}, clock_total {}, lroundf calls: {}",
        r.tick_total, r.sim_ms0, r.sim_ms_end, r.race_start, r.anomalies, r.clock_mismatch, r.clock_total, r.lroundf_total
    );
    let mut fails = Vec::new();
    if let Err(e) = r.consistent() {
        fails.push(e);
    }
    if plain_t != hook_t || plain_t.is_none() {
        fails.push(format!(
            "validated time differs or is missing: plain {} vs hooked {}",
            crate::secs_opt(plain_t),
            crate::secs_opt(hook_t)
        ));
    }
    // The tape covers race time [start_offset, start_offset + 10 n); the engine
    // simulates from sim 1000 to a little past the last record. Report the
    // relation rather than assert a tail length nobody has measured.
    let last_rec_sim = r.race_start as i64 + tape.race_ms(tape.n().saturating_sub(1));
    println!(
        "tape: {} records, last record at sim {} ms; the engine ran {} ms past it",
        tape.n(),
        last_rec_sim,
        r.sim_ms_end as i64 - last_rec_sim
    );
    if o.gdb {
        let n = gdb_count(&engine.work.join("gdb"), &engine.server, &engine.map, &g, &[TICK_CLOCK_WRITE_OFF, TICK_FN_OFF])?;
        println!(
            "gdb (no shim): loop clock write hit {} times, tick function entered {} times",
            n[0], n[1]
        );
        if n[0] != r.tick_total || n[1] != r.tick_total {
            fails.push(format!(
                "gdb counts {} / {} disagree with the hook's {}",
                n[0], n[1], r.tick_total
            ));
        }
    }
    if fails.is_empty() {
        println!("PASS");
        Ok(())
    } else {
        Err(format!("FAIL: {}", fails.join("; ")))
    }
}

/// Hit counts at `rvas` over one full run under gdb with randomization off
/// (PIE base 0x555555554000). Independent of the shim entirely.
pub fn gdb_count(dir: &Path, server: &Path, map: &Path, ghost: &Path, rvas: &[usize]) -> Result<Vec<u64>, String> {
    let replays = dir.join("UserData/Replays");
    let maps = dir.join("UserData/Maps");
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(&replays).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&maps).map_err(|e| e.to_string())?;
    let _ = std::os::unix::fs::symlink(server.join("Packs"), dir.join("Packs"));
    let _ = std::os::unix::fs::symlink(server.join("TrackmaniaServer"), dir.join("TrackmaniaServer"));
    let m = map.canonicalize().map_err(|e| e.to_string())?;
    let _ = std::os::unix::fs::symlink(m.clone(), maps.join(m.file_name().unwrap()));
    std::fs::copy(ghost, replays.join("g.Ghost.Gbx")).map_err(|e| e.to_string())?;
    let mut script = String::from(
        "set pagination off\nset confirm off\nset disable-randomization on\nstarti\nset $b = 0x555555554000\n",
    );
    for (i, r) in rvas.iter().enumerate() {
        script.push_str(&format!("break *($b+{:#x})\nignore {} 100000000\n", r, i + 1));
    }
    script.push_str("continue\ninfo breakpoints\nquit\n");
    let sp = dir.join("count.gdb");
    std::fs::write(&sp, script).map_err(|e| e.to_string())?;
    let out = Command::new("gdb")
        .args(["-q", "-batch", "-x"])
        .arg(&sp)
        .args(["--args", "./TrackmaniaServer", "/nodaemon", "/validatepath=."])
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("gdb: {}", e))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut hits = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("breakpoint already hit ") {
            let n: u64 = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            hits.push(n);
        }
    }
    if hits.len() != rvas.len() {
        return Err(format!("gdb reported {} hit counts for {} breakpoints:\n{}", hits.len(), rvas.len(), text));
    }
    Ok(hits)
}

// ---------------------------------------------------------------- fk tickhook load

pub struct LoadOpts {
    pub n: usize,
}

/// N fork servers started at once, all asked for the same checkpoint. Reports
/// the distribution of (clock, sim_ms, probe). Under the tick hook every one
/// must agree, and `probe + 1` must be the tick the engine reported.
pub fn load(engine: &Engine, tape: Tape, at: Checkpoint, o: LoadOpts) -> Result<(), String> {
    engine.check()?;
    std::fs::create_dir_all(&engine.work).map_err(|e| e.to_string())?;
    let n = o.n.max(1);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(n));
    let mut handles = Vec::new();
    let t0 = Instant::now();
    for i in 0..n {
        let e = Engine {
            server: engine.server.clone(),
            map: engine.map.clone(),
            shim: engine.shim.clone(),
            work: engine.work.join(format!("w{:03}", i)),
            work_is_temporary: false,
        };
        let tp = tape.clone();
        let b = barrier.clone();
        handles.push(std::thread::spawn(move || -> Result<(u64, u64, usize, bool, u64), String> {
            b.wait();
            let mut s = Session::start(&e, tp, at)?;
            // The raw probe, NOT `boundary_tick`: this command wants to SEE a
            // disagreement, not abort on the first one.
            let p = s.srv.probe_tick()?;
            let r = (s.srv.clock, s.srv.sim_ms, p, s.srv.tick_mode, s.srv.race_start);
            s.srv.quit();
            Ok(r)
        }));
    }
    let mut rows = Vec::new();
    let mut errs = Vec::new();
    for h in handles {
        match h.join().map_err(|_| "worker panicked".to_string()) {
            Ok(Ok(r)) => rows.push(r),
            Ok(Err(e)) | Err(e) => errs.push(e),
        }
    }
    let wall = t0.elapsed().as_secs_f64();
    // Keyed on (clock, probe, race_start): sim_ms legitimately differs between
    // a 2200-start and a 2300-start process, and the point is that the RACE
    // tick and the probe do not.
    let mut by: BTreeMap<(u64, usize, u64), usize> = BTreeMap::new();
    for r in &rows {
        *by.entry((r.0, r.2, r.4)).or_default() += 1;
    }
    println!(
        "{} servers started together at {:?} ({}), {} answered, {} failed, {:.1}s wall",
        n,
        at,
        if clock::tick_mode() { "tick clock" } else { "lroundf clock" },
        rows.len(),
        errs.len(),
        wall
    );
    for e in errs.iter().take(5) {
        println!("  failure: {}", e);
    }
    for ((c, p, rs), k) in &by {
        println!("  clock {:>8}  race_start {:>5}  probe {:>5}  (first unconsumed {:>5})  x{}", c, rs, p, p + 1, k);
    }
    let tick_mode = rows.first().map(|r| r.3).unwrap_or(false);
    if tick_mode {
        // Per server: probe + 1 must be the tape tick its OWN sim_ms/race_start
        // name; across servers: one clock value and one probe.
        let want_of = |r: &(u64, u64, usize, bool, u64)| clock::tape_tick_at(r.1, r.4, tape.start_offset_ms).max(clock::first_read_tick(tape.start_offset_ms));
        let probe_ok = rows.iter().filter(|r| r.2 as i64 + 1 == want_of(r)).count();
        let clocks: std::collections::BTreeSet<u64> = rows.iter().map(|r| r.0).collect();
        let probes: std::collections::BTreeSet<usize> = rows.iter().map(|r| r.2).collect();
        let starts: std::collections::BTreeSet<u64> = rows.iter().map(|r| r.4).collect();
        println!(
            "race starts seen: {:?}; distinct clock values: {}; distinct probes: {}; probe agrees with its own server's tick on {} of {}",
            starts, clocks.len(), probes.len(), probe_ok, rows.len()
        );
        if !errs.is_empty() || clocks.len() != 1 || probes.len() != 1 || probe_ok != rows.len() {
            return Err(format!(
                "FAIL: {} distinct clocks, {} distinct probes, probe agrees on {} of {}, {} failures",
                clocks.len(), probes.len(), probe_ok, rows.len(), errs.len()
            ));
        }
        println!("PASS: one race tick on all {} servers; the probe agrees on every one", rows.len());
    } else {
        println!(
            "lroundf clock: {} distinct stop points across {} servers (this is the A/B baseline, not a failure)",
            by.len(),
            rows.len()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------- fk tickhook find

pub struct FindOpts {
    /// How far below the tick-loop return address to look for call sites.
    pub back: usize,
    pub ahead: usize,
}

/// Find the tick function on a build whose constants do not match.
pub fn find(engine: &Engine, tape: Tape, o: FindOpts) -> Result<(), String> {
    engine.check()?;
    std::fs::create_dir_all(&engine.work).map_err(|e| e.to_string())?;
    let elf = Elf::open(&engine.server.join("TrackmaniaServer"))?;
    let read = |a: usize, n: usize| elf.read(a, n);
    match tick_hook_signatures_match(&read) {
        Ok(()) => println!("note: the current constants already match this binary; searching anyway"),
        Err(w) => println!("current constants do not match this binary ({}); searching", w),
    }

    // 1. DYNAMIC: where does the engine read an input record from, and who
    //    called it? The page-fault probe logs the fault RIP and the frame
    //    chain. Run it under the LEGACY clock (the tick hook may not install
    //    on this build) in a child `fk`, so this process's clock mode is left
    //    alone.
    let pw = engine.work.join("probe");
    let g = engine.work.join("find.Ghost.Gbx");
    tape.write_reference(&g)?;
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let st = Command::new(&me)
        .args(["server", "probe", "--tape"])
        .arg(&g)
        .arg("--map")
        .arg(&engine.map)
        .arg("--server")
        .arg(&engine.server)
        .arg("--shim")
        .arg(&engine.shim)
        .arg("--work")
        .arg(&pw)
        .args(["--at", "frac:0.5"])
        .env("FK_CLOCK", "lroundf")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| e.to_string())?;
    if !st.success() {
        return Err("the legacy-clock probe run failed; cannot get the fault frame chain".into());
    }
    let log = std::fs::read_to_string(pw.join("srv/server.log")).map_err(|e| e.to_string())?;
    let mut base = 0u64;
    let mut rip = 0u64;
    let mut frames = Vec::new();
    for l in log.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() >= 4 && f[0] == "FKSHIM" && f[1] == "input_fault" {
            match f[2] {
                "module_base" => base = f[3].parse().unwrap_or(0),
                "rip" => rip = f[3].parse().unwrap_or(0),
                "frame" if f.len() >= 5 => frames.push(f[4].parse::<u64>().unwrap_or(0)),
                _ => {}
            }
        }
    }
    if base == 0 || rip == 0 || frames.is_empty() {
        return Err("no input_fault frame chain in server.log".into());
    }
    let reader_rip = (rip - base) as usize;
    let ret_in_loop = frames[0].wrapping_sub(base) as usize;
    println!(
        "record read at {:#x}; returns into {:#x} (frames: {})",
        reader_rip,
        ret_in_loop,
        frames.iter().map(|f| format!("{:#x}", f.wrapping_sub(base))).collect::<Vec<_>>().join(" ")
    );
    if !elf.is_text(ret_in_loop) {
        return Err("the return address is not in the binary's text".into());
    }

    // 2. STATIC: every `call rel32` in [ret - back, ret + ahead) whose target
    //    is text and begins with a relocatable frame-pointer prologue.
    let lo = ret_in_loop.saturating_sub(o.back);
    let hi = ret_in_loop + o.ahead;
    let win = elf.read(lo, hi - lo);
    let mut cands: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..win.len().saturating_sub(5) {
        if win[i] != 0xe8 {
            continue;
        }
        let rel = i32::from_le_bytes(win[i + 1..i + 5].try_into().unwrap()) as i64;
        let target = (lo + i + 5) as i64 + rel;
        if target <= 0 || !elf.is_text(target as usize) {
            continue;
        }
        let t = target as usize;
        if prologue_displaced_len(&elf.read(t, 64)).is_none() {
            continue;
        }
        cands.entry(t).or_default().push(lo + i);
    }
    println!("{} candidate functions with a relocatable prologue called from the loop window", cands.len());

    // 3. DYNAMIC AGAIN: hook each candidate in finder mode and keep the ones
    //    that behave as the tick.
    let shim = engine.shim.canonicalize().map_err(|e| e.to_string())?.to_string_lossy().into_owned();
    let (plain_t, _, _) = run_once(&engine.work.join("plain"), &engine.server, &engine.map, &g, &[], Duration::from_secs(600))?;
    let mut winners = Vec::new();
    for (t, sites) in &cands {
        let dir = engine.work.join(format!("cand_{:x}", t));
        let res = run_once(
            &dir,
            &engine.server,
            &engine.map,
            &g,
            &[
                ("LD_PRELOAD", shim.clone()),
                ("FKSHIM_CLOCK", "tick".into()),
                ("FKSHIM_TICK_UNSAFE", "1".into()),
                ("FKSHIM_TICK_FN_OFF", format!("{:#x}", t)),
                ("FKSHIM_VALIDATOR_CAR", "1".into()),
            ],
            Duration::from_secs(120),
        );
        let verdict = match res {
            Err(e) => Err(e),
            Ok((tm, err, _)) => {
                let r = parse_shim_report(&err);
                match r.consistent() {
                    Err(e) => Err(e),
                    Ok(()) if tm != plain_t => Err(format!("validated time {} != plain {}", crate::secs_opt(tm), crate::secs_opt(plain_t))),
                    Ok(()) => Ok(r),
                }
            }
        };
        match verdict {
            Ok(r) => {
                println!("  {:#x}: TICK ({} ticks, sim {}..{}), called from {}", t, r.tick_total, r.sim_ms0, r.sim_ms_end, sites.iter().map(|s| format!("{:#x}", s)).collect::<Vec<_>>().join(","));
                winners.push((*t, sites.clone()));
            }
            Err(e) => println!("  {:#x}: no ({})", t, e),
        }
    }
    if winners.is_empty() {
        return Err("no candidate behaves as the tick function; widen --back/--ahead or read TICKHOOK.md".into());
    }
    // The tick function is the FIRST call in the loop body: among winners
    // (several per-tick functions may qualify), prefer the lowest call site
    // above the reader's return address, i.e. the earliest in the body.
    winners.sort_by_key(|(_, sites)| sites.iter().copied().min().unwrap_or(usize::MAX));
    for (t, sites) in &winners {
        let head = elf.read(*t, 32);
        let disp = prologue_displaced_len(&head).unwrap_or(0);
        let site = sites.iter().copied().min().unwrap_or(0);
        println!("\ncandidate {:#x} (call at {:#x}):", t, site);
        println!("  pub const TICK_FN_OFF: usize = {:#x};", t);
        println!("  pub const TICK_FN_SIGNATURE: [u8; 32] = [{}];", head.iter().map(|b| format!("{:#04x}", b)).collect::<Vec<_>>().join(", "));
        println!("  pub const TICK_FN_DISPLACED: usize = {};", disp);
        println!("  pub const TICK_CALL_SITE_OFF: usize = {:#x};  // the 8 bytes from 3 before the call", site.saturating_sub(3));
        println!("  pub const TICK_CALL_SITE_SIGNATURE: [u8; 8] = [{}];", elf.read(site.saturating_sub(3), 8).iter().map(|b| format!("{:#04x}", b)).collect::<Vec<_>>().join(", "));
    }
    println!("\nThe loop's clock write (TICK_CLOCK_WRITE_*) is not found automatically: disassemble the\nloop around the reader's return address and take the `mov [sim+0x48], new_time` at the\nend of the body (TICKHOOK.md §4). Then `fk tickhook check` and `fk tickhook count --gdb`.");
    Ok(())
}
