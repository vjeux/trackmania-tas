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
//!   once?* When the clock was a count of `lroundf` calls, 104 of 150 were one
//!   tick late. Now every server must report the same race tick AND the
//!   page-fault probe (the control, measured from the other side) must agree
//!   on every one.
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
        if let Some(v) = get("FKSHIM tick_total ") {
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
        "hook: {} ticks, sim {} -> {} ms, race start {}, dt!=10: {}, sim-clock disagreements: {}, clock_total {}",
        r.tick_total, r.sim_ms0, r.sim_ms_end, r.race_start, r.anomalies, r.clock_mismatch, r.clock_total
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
/// must agree, and the probe must be the record the engine's own tick names.
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
        handles.push(std::thread::spawn(move || -> Result<(u64, u64, usize, u64), String> {
            b.wait();
            let mut s = Session::start(&e, tp, at)?;
            // The raw probe, NOT `boundary_tick`: this command wants to SEE a
            // disagreement, not abort on the first one.
            let p = s.srv.probe_tick()?;
            let r = (s.srv.clock, s.srv.sim_ms, p, s.srv.race_start);
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
        *by.entry((r.0, r.2, r.3)).or_default() += 1;
    }
    println!(
        "{} servers started together at {:?}, {} answered, {} failed, {:.1}s wall",
        n,
        at,
        rows.len(),
        errs.len(),
        wall
    );
    for e in errs.iter().take(5) {
        println!("  failure: {}", e);
    }
    for ((c, p, rs), k) in &by {
        println!("  clock {:>8}  race_start {:>5}  first unconsumed record {:>5}  x{}", c, rs, p, k);
    }
    // Per server: probe + 1 must be the tape tick its OWN sim_ms/race_start
    // name; across servers: one clock value and one probe.
    let want_of = |r: &(u64, u64, usize, u64)| clock::record_read_at(r.1, r.3, tape.start_offset_ms);
    let probe_ok = rows.iter().filter(|r| r.2 as i64 == want_of(r)).count();
    let clocks: std::collections::BTreeSet<u64> = rows.iter().map(|r| r.0).collect();
    let probes: std::collections::BTreeSet<usize> = rows.iter().map(|r| r.2).collect();
    let starts: std::collections::BTreeSet<u64> = rows.iter().map(|r| r.3).collect();
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

    let g = engine.work.join("find.Ghost.Gbx");
    tape.write_reference(&g)?;

    // 1. STATIC: find the INPUT-RECORD READER, then the loop that calls it.
    //
    //    The reader is the one function in the binary that indexes a 32-byte
    //    array by a tick number and copies a whole record out of it, so its
    //    body carries `shl rcx,5` immediately followed by the two `movups`
    //    that fetch the 32 bytes. That shape is about what the code DOES, not
    //    where it sits, so it survives a rebuild in a way an address never
    //    does. It is still only a lead: what decides is the behaviour test in
    //    step 3.
    let readers = find_record_readers(&elf);
    if readers.is_empty() {
        return Err("no input-record reader found (no `shl rcx,5` + `movups [rdx+rcx]` pair in \
                    the text); the record layout of this build is not what TICKHOOK.md \
                    describes, and the hook needs re-deriving by hand"
            .into());
    }
    println!(
        "input-record reader(s): {}",
        readers.iter().map(|(f, s)| format!("{:#x} (copy at {:#x})", f, s)).collect::<Vec<_>>().join(", ")
    );

    // 2. Every `call rel32` targeting a reader is inside the tick loop; take a
    //    window around each such call site and collect every OTHER function it
    //    calls whose prologue is relocatable.
    let mut call_sites: Vec<usize> = Vec::new();
    for (f, _) in &readers {
        call_sites.extend(callers_of(&elf, *f));
    }
    if call_sites.is_empty() {
        return Err("nothing calls the record reader with a direct `call rel32`; the tick loop \
                    reaches it some other way on this build"
            .into());
    }
    println!(
        "called from {}",
        call_sites.iter().map(|s| format!("{:#x}", s)).collect::<Vec<_>>().join(", ")
    );
    let ret_in_loop = call_sites[0];
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
                    ("FKSHIM_TICK_UNSAFE", "1".into()),
                ("FKSHIM_TICK_FN_OFF", format!("{:#x}", t)),
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

/// Every `(function entry, copy site)` that looks like the input-record reader:
/// `shl rcx,5` (index * 32) followed within 8 bytes by
/// `movups xmm0,[rdx+rcx*1]` — the 32-byte record fetch.
///
/// This is a shape, not an address, which is the point: it is what the reader
/// DOES. It is a lead and never a verdict — `find`'s behaviour test is what
/// decides, and a build where this returns nothing is one where the record
/// layout itself changed.
pub fn find_record_readers(elf: &Elf) -> Vec<(usize, usize)> {
    const SHL_RCX_5: [u8; 4] = [0x48, 0xc1, 0xe1, 0x05];
    const MOVUPS_RDX_RCX: [u8; 4] = [0x0f, 0x10, 0x04, 0x0a];
    let mut out = Vec::new();
    for (va, bytes) in elf.text_segments() {
        let mut i = 0usize;
        while i + 16 < bytes.len() {
            if bytes[i..i + 4] == SHL_RCX_5 {
                if let Some(k) = (4..12).find(|k| bytes[i + k..i + k + 4] == MOVUPS_RDX_RCX) {
                    if let Some(f) = function_entry(bytes, i) {
                        out.push((va + f, va + i + k));
                    }
                }
            }
            i += 1;
        }
    }
    out.dedup_by_key(|(f, _)| *f);
    out
}

/// The entry of the function containing `off`: the nearest `push rbp; mov
/// rbp,rsp` above it that is preceded by padding (`int3`/`nop`), which is how
/// this compiler separates functions.
fn function_entry(bytes: &[u8], off: usize) -> Option<usize> {
    let lo = off.saturating_sub(4096);
    (lo..off).rev().find(|&i| {
        bytes[i..i + 4] == [0x55, 0x48, 0x89, 0xe5]
            && i > 0
            && (bytes[i - 1] == 0xcc || bytes[i - 1] == 0x90)
    })
}

/// Every `call rel32` site whose target is `target`.
pub fn callers_of(elf: &Elf, target: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for (va, bytes) in elf.text_segments() {
        for i in 0..bytes.len().saturating_sub(5) {
            if bytes[i] != 0xe8 {
                continue;
            }
            let rel = i32::from_le_bytes(bytes[i + 1..i + 5].try_into().unwrap()) as i64;
            if (va + i + 5) as i64 + rel == target as i64 {
                out.push(va + i);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- fk tickhook reads

/// AUDIT EVERY READ THE ORACLE MAKES OF ENGINE MEMORY, against the engine.
///
/// The fork oracle reads five things out of the running server, and until this
/// command existed only two of them were ever checked against the engine's own
/// arithmetic. Two of the other three turned out to be wrong (the input array's
/// base was four bytes into the record; the countdown reads record 0, not the
/// tape's countdown records). So: one command, one stopped server, every read
/// stated and compared with what the engine says it should be.
///
/// 1. **the input array** -- the address the shim found, the record it says the
///    engine reads next, and the page-fault probe's independent answer;
/// 2. **the record layout** -- the 32 bytes at that record, field by field,
///    against `forksrv::STRIDE`'s documented layout and against the tape;
/// 3. **the car** -- derived (`forkoracle::car`), with the body record vs its
///    copy-out, and every copy the engine keeps beside it with its phase;
/// 4. **the label** -- `[sim+0x48] - race_start` against the race time the
///    hook knows exactly;
/// 5. **the simulation clock** -- `[sim+0x48]`, which the hook checks on every
///    tick anyway, reported here for completeness.
pub fn reads(engine: &Engine, tape: Tape, at: Checkpoint) -> Result<(), String> {
    use forkoracle::forksrv::{REC_BRAKE, REC_GAS, REC_STEER, STRIDE};
    use forkoracle::procmem::read_at;

    let mut s = Session::start(engine, tape, at)?;
    let pid = s.srv.pid();
    let (sim_ms, race_start) = (s.srv.sim_ms, s.srv.race_start);
    let race_ms = sim_ms as i64 - race_start as i64;
    let off = s.tape.start_offset_ms;
    println!(
        "server {} stopped at sim {} ms, race start {} ms -> race {} ms (clock {}), tape start_offset {} ms",
        pid, sim_ms, race_start, race_ms, s.srv.clock, off
    );

    let mut fails: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String| {
        println!("  [{}] {}", if ok { "ok" } else { "FAIL" }, what);
        if !ok {
            fails.push(what);
        }
    };

    // 1. THE INPUT ARRAY, and the two independent answers about the boundary.
    let want = clock::record_read_at(sim_ms, race_start, off);
    let probe = s.srv.probe_tick()?;
    check(
        probe as i64 == want,
        format!(
            "input array at {:#x}; the engine's own tick says it reads record {} next, the \
             page-fault probe says {}",
            s.srv.base, want, probe
        ),
    );

    // 2. THE RECORD, field by field, against the tape and the documented shape.
    let t = probe.min(s.tape.n() - 1);
    let raw = read_at(pid, s.srv.base + (t as u64) * STRIDE as u64, STRIDE)
        .ok_or("cannot read the record out of the stopped server")?;
    let f32at = |o: usize| f32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
    let u32at = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
    let w = forkoracle::forksrv::rec_of(s.tape.steer[t], s.tape.accel[t], s.tape.brake[t]);
    println!(
        "  record {} = [{:#010x}] steer {} gas {} brake {} | {} {:#010x} {:#010x} {:#010x}",
        t,
        u32at(0),
        f32at(REC_STEER),
        f32at(REC_GAS),
        f32at(REC_BRAKE),
        f32at(16),
        u32at(20),
        u32at(24),
        u32at(28)
    );
    check(
        f32at(REC_STEER) == w.steer && f32at(REC_GAS) == w.gas && f32at(REC_BRAKE) == w.brake,
        format!(
            "steer/gas/brake at +{}/+{}/+{} match the tape ({}, {}, {})",
            REC_STEER, REC_GAS, REC_BRAKE, w.steer, w.gas, w.brake
        ),
    );
    // The tail, over several records, so "constant" is measured and not assumed.
    let tail = |k: usize| -> Option<(f32, u32, u32, u32)> {
        let r = read_at(pid, s.srv.base + (k as u64) * STRIDE as u64, STRIDE)?;
        let f = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let u = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        Some((f(16), u(20), u(24), u(28)))
    };
    let ks: Vec<usize> = [0usize, 1, 157, 158, t, t + 1, s.tape.n() - 1].iter().copied().filter(|k| *k < s.tape.n()).collect();
    for k in &ks {
        if let Some((a, b, c, d)) = tail(*k) {
            println!("  tail[{}] = {} {:#010x} {:#010x} {:#010x}", k, a, b, c, d);
        }
    }
    let tails: Vec<(f32, u32, u32, u32)> = ks.iter().filter_map(|k| tail(*k)).collect();
    check(
        tails.iter().all(|t| t.0 == 0.0 && t.2 == 0 && t.3 == 2),
        "the engine-owned tail is +0x10 = 0.0, +0x18 = 0, +0x1c = 2 in every record".to_string(),
    );
    // +0x14 is NOT constant (0x3576f40e early on this tape, 0x3576f409 later),
    // so the old note calling it a "device-segment const" is wrong. It is
    // engine-owned either way: the check is that we never touch it.
    println!(
        "  +0x14 takes {} distinct value(s) across those records -- engine-owned, never written",
        tails.iter().map(|t| t.1).collect::<std::collections::BTreeSet<_>>().len()
    );

    // 3+4. THE CAR, DERIVED, and its copies. `forkoracle::car::locate` walks
    //    the pointers the physics step itself follows -- controller, sim,
    //    playground, scene, vehicle manager, dyna world, and the participant's
    //    driven slot -- to the body record the solver integrates and the
    //    copy-out in the CGameVehiclePhy. Every hop is checked as it is taken
    //    and the record is required to be byte-identical to the copy-out; here
    //    the copies the engine ALSO keeps are read beside it, so their phases
    //    are stated rather than assumed (LOCATE.md).
    let car = forkoracle::car::locate(&s.srv)
        .map_err(|e| format!("the car did not derive: {}", e))?;
    println!("  car: {}", car);
    let xyz = |a: u64| -> Option<[f64; 3]> {
        let b = read_at(pid, a, 12)?;
        Some([
            f32::from_le_bytes(b[0..4].try_into().unwrap()) as f64,
            f32::from_le_bytes(b[4..8].try_into().unwrap()) as f64,
            f32::from_le_bytes(b[8..12].try_into().unwrap()) as f64,
        ])
    };
    let dist = |a: [f64; 3], b: [f64; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let pos = xyz(car.pos()).ok_or("cannot read the car's position")?;
    let vel = xyz(car.vel()).ok_or("cannot read the car's velocity")?;
    let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
    let tick_m = speed * 0.01;
    println!(
        "  car pos = ({:.4}, {:.4}, {:.4}), speed {:.2} m/s ({:.4} m per tick)",
        pos[0], pos[1], pos[2], speed, tick_m
    );
    match car.body {
        Some(b) => {
            let bpos = xyz(b.addr + forkoracle::car::build128182::POS_IN_BODY).ok_or("cannot read the body")?;
            check(
                bpos == pos,
                format!(
                    "the dyna body record {:#x} (handle {}) and the phy copy-out hold the same position bit for bit",
                    b.addr, b.handle
                ),
            );
        }
        None => println!("  (no dyna body at this tick: inside a respawn window; the copy-out holds the checkpoint pose)"),
    }
    // the copies, each with its measured phase
    let vis_post = xyz(car.vis() + forkoracle::car::build128182::POS_IN_VIS).ok_or("cannot read the post-step vis state")?;
    let vis_pre = xyz(car.vis_pre() + forkoracle::car::build128182::POS_IN_VIS).ok_or("cannot read the pre-step vis state")?;
    let pcopy = xyz(car.participant_copy_pos()).ok_or("cannot read the participant's copy")?;
    check(
        dist(vis_post, pos) <= 0.002,
        format!(
            "the post-step vis state (phy+0x848) is the car's position quantised to 1 mm: {:.4} m off",
            dist(vis_post, pos)
        ),
    );
    println!(
        "  the pre-step vis state (phy+0x4e8) sits {:.4} m from the car = {:.2} ticks of travel (expected 1: it is refreshed BEFORE the solver)",
        dist(vis_pre, pos),
        dist(vis_pre, pos) / tick_m.max(1e-6)
    );
    println!(
        "  the participant's copy (+0xe24, the old sweep's object on map 2) sits {:.4} m from the car = {:.2} ticks of travel",
        dist(pcopy, pos),
        dist(pcopy, pos) / tick_m.max(1e-6)
    );
    // THE LABEL: the clock word is the tick loop's own, the bias the race start
    // it set; the state present is the one stamped [sim+0x48].
    let l = car.layout();
    let clk = read_at(pid, l.clock, 4)
        .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()) as i64)
        .ok_or("cannot read [sim+0x48]")?;
    check(
        clk - l.clock_bias == race_ms - 10,
        format!(
            "[sim+0x48] - race_start = {} ms = the race time of the state present (the tick before the one being entered, {})",
            clk - l.clock_bias,
            race_ms - 10
        ),
    );

    // 5. THE SIMULATION CLOCK the hook cross-checks on every tick.
    let simclk = read_at(pid, s.srv.validation_sim + 0x48, 4)
        .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()) as u64)
        .ok_or("cannot read [sim+0x48]")?;
    check(
        simclk + 10 == sim_ms,
        format!(
            "[sim+0x48] = {} ms, one tick behind the tick being entered ({} ms)",
            simclk, sim_ms
        ),
    );

    s.srv.quit();
    if fails.is_empty() {
        println!("PASS: every read agrees with the engine");
        Ok(())
    } else {
        Err(format!("{} of the oracle's reads disagree with the engine", fails.len()))
    }
}

// ---------------------------------------------------------------- fk tickhook cost

/// WHERE A FORK EVALUATION'S TIME ACTUALLY GOES, phase by phase.
///
/// Per-candidate cost is `fixed + per_tick * remaining_ticks`, and at a late
/// checkpoint the fixed part is most of it. This measures the fixed part's
/// composition against the protocol itself rather than by subtraction:
///
/// * `null` — `'N'`: fork the paused engine and reap it. Nothing simulated.
///   This is the floor: page tables for a ~150 MB address space.
/// * `k ticks` — `'S'` with a simulated-time budget, which makes the child
///   `_exit` the moment the budget is spent. Two budgets give the slope
///   (µs/tick) and, extrapolated back, what a child pays before its first tick.
/// * `full` — `'R'`: the whole path, which is what a candidate really costs —
///   every remaining tick, the engine's own run past the finish, the
///   validator's finish and print path, the JSON down a pipe, and the parent's
///   parse.
///
/// The gap between `full` and `fork + ticks` is the part no candidate needs:
/// the validator's post-race work and the transport.
pub fn cost(engine: &Engine, tape: Tape, at: Checkpoint, n: usize) -> Result<(), String> {
    let mut s = Session::start(engine, tape, at)?;
    let probe = s.srv.boundary_tick(s.tape.start_offset_ms)?;
    let recs = s.tape.tail_records(probe);
    let tail = recs.len();
    // FK_FINISH_FAST=1 calibrates the engine's finish word and lets children
    // leave the moment it is written, so the same measurement prices the lever.
    if std::env::var("FK_FINISH_FAST").is_ok() {
        let d = s.tape.declared_ms.ok_or("the tape declares no finish time")? as i64;
        let (addr, _) = forkoracle::finish::calibrate(&mut s.srv, probe, &recs, d)?;
        println!("exit-at-finish armed on {:#x}", addr);
    }
    let seg = [(s.srv.base, 4u32)]; // a 4-byte gather: the transport, not the data

    let time = |f: &mut dyn FnMut()| -> f64 {
        let t = Instant::now();
        for _ in 0..n {
            f();
        }
        t.elapsed().as_secs_f64() * 1000.0 / n as f64
    };

    let mut srv = &mut s.srv;
    let null_ms = {
        let t = Instant::now();
        for _ in 0..n {
            srv.null_fork();
        }
        t.elapsed().as_secs_f64() * 1000.0 / n as f64
    };
    // SPARSE sampling (one gather every 8 ticks, 4 bytes): the child still exits
    // the moment its simulated-time budget is spent, but the sampler is not
    // what is being timed. Gathering every tick down a pipe doubles the
    // apparent per-tick cost -- measured, and the reason this is not stride 1.
    let mut sampled = |ticks: u32| -> f64 {
        // the SAME patch list the full run sends, so the two differ only in what
        // the child does after its last tick
        let keep = recs.len();
        let t = Instant::now();
        for _ in 0..n {
            srv.run_sampled_segs_ex(
                probe,
                &recs[..keep],
                &seg,
                8,
                // enough samples that the BUDGET is what ends the child, never
                // the sample count
                (ticks / 8 + 8) | crate::locate::EXIT_ON_BUDGET,
                (0, 4),
                forkoracle::clock::budget_for_ticks(ticks),
            );
        }
        t.elapsed().as_secs_f64() * 1000.0 / n as f64
    };
    let s8 = sampled(8);
    let s_tail = sampled(tail as u32);
    let s208 = sampled(208.min(tail as u32));
    // FK_COST_DNF=1: measure a candidate that does NOT finish, by steering it
    // off the road. A finisher and a DNF pay different epilogues and the
    // difference decides whether the DNF half of the lever is worth building.
    let recs = if std::env::var("FK_COST_DNF").is_ok() {
        let mut r = recs.clone();
        for x in r.iter_mut().take(40) {
            x.steer = 1.0;
        }
        r
    } else {
        recs.clone()
    };
    // The full path, and the child's own timeline out of the shim's shared
    // timing page -- so every phase below is measured inside the child rather
    // than inferred by differencing two protocol paths.
    let mut acc = [0f64; 6];
    let full_ms = {
        let t = Instant::now();
        for _ in 0..n {
            let out = srv.run(probe, &recs);
            for (i, k) in ["child_us ", "tick1_us ", "tickN_us ", "ticks ", "first_us ", "fork_us "].iter().enumerate() {
                if let Some(v) = out.split(*k).nth(1).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok()) {
                    acc[i] += v / n as f64;
                }
            }
        }
        t.elapsed().as_secs_f64() * 1000.0 / n as f64
    };
    let (child_us, tick1_us, ticknus, ticks) = (acc[0], acc[1], acc[2], acc[3]);
    // `first_us` is measured from the fork, not from t_start: add the fork.
    let first_byte_us = acc[4] + acc[5];
    let _ = time;

    let span = (208.min(tail as u32) as f64 - 8.0).max(1.0);
    let per_tick_us = (s208 - s8) / span * 1000.0;
    let child_start = s8 - 8.0 * per_tick_us / 1000.0;
    let sim_ms = s_tail - child_start;
    // MEASURED, not extrapolated: the same tail, once with the child exiting at
    // the last tick and once through the whole validator path.
    let after = full_ms - s_tail;
    println!(
        "checkpoint tick {} -- {} ticks of tail, {} runs of each phase\n\
         \n\
         null fork + reap            {:8.2} ms   the paused engine's page tables, nothing simulated\n\
         child up to its first tick  {:8.2} ms   fork + COW + the shim's entry (extrapolated)\n\
         per simulated tick          {:8.2} us   from {} vs {} ticks\n\
         the tail itself             {:8.2} ms   {} ticks (measured: a child that exits at the last one)\n\
         ---------------------------------------\n\
         a candidate, end to end     {:8.2} ms\n\
         of which AFTER the last tick{:8.2} ms   the engine's run past the finish, the validator's\n\
                                                 finish and print path, the JSON, the pipe, the parse\n",
        probe, tail, n,
        null_ms, child_start, per_tick_us, 8, 208, sim_ms, tail, full_ms, after
    );
    // THE CHILD'S OWN TIMELINE (µs from the parent's pre-fork instant).
    if ticks > 0.0 {
        let sim = (ticknus - tick1_us) / 1000.0;
        let before = tick1_us / 1000.0;
        let post = full_ms - ticknus / 1000.0;
        println!(
            "measured INSIDE the child ({} ticks simulated):\n\
             \x20 fork -> child alive      {:8.2} ms\n\
             \x20 child -> its first tick  {:8.2} ms   COW faults, the shim's entry, the patch\n\
             \x20 first tick -> last tick  {:8.2} ms   {:.1} us/tick\n\
             \x20 last tick -> first byte  {:8.2} ms   the engine past the finish, then the\n\
             \x20                                       validator's finish and print path\n\
             \x20 first byte -> answer     {:8.2} ms   the JSON down the pipe, the parent's read and\n\
             \x20                                       scan, the SIGKILL\n\
             \x20 total                    {:8.2} ms",
            ticks,
            child_us / 1000.0,
            before - child_us / 1000.0,
            sim,
            sim * 1000.0 / ticks,
            (first_byte_us - ticknus) / 1000.0,
            full_ms - first_byte_us / 1000.0,
            full_ms
        );
        println!(
            "  => {:.0}% of this candidate is fixed cost: {:.2} ms before the first tick and \
             {:.2} ms after the last.",
            100.0 * (before + post) / full_ms,
            before,
            post
        );
    }
    let _ = after;
    s.srv.quit();
    Ok(())
}

// -------------------------------------------------------------- fk tickhook finish

/// FIND THE RACE RESULT IN MEMORY, so a child never has to run the validator's
/// print path to report it.
///
/// `fk tickhook cost` measures ~5.8 ms per candidate AFTER its last simulated
/// tick — constant, and 44 % of a candidate at a late checkpoint. Almost none
/// of it is simulation (7 ticks, 0.26 ms): it is the validator's finish
/// handling, the JSON, the pipe and the parse. A child that could read its own
/// finish time would skip all of it.
///
/// So: stop a server a few ticks PAST the finish, scan every writable region
/// for the finish time the plain oracle reported, and report each hit as an
/// OFFSET from something a child can resolve for itself — the validator's
/// participant and vehicle, the playground, the simulation, the input array.
/// An address is worthless (the heap is bimodal); an offset from a typed object
/// is not.
///
/// Two tapes with DIFFERENT finish times are the control: a slot that holds
/// each tape's own finish, at the same offset, is the race result. A slot that
/// holds 22730 in both is a coincidence of one number.
pub fn finish(engine: &Engine, tape: Tape, at: Checkpoint) -> Result<(), String> {
    use forkoracle::procmem;

    let mut s = Session::start(engine, tape, at)?;
    let pid = s.srv.pid();
    let race_ms = s.srv.sim_ms as i64 - s.srv.race_start as i64;
    println!(
        "server {} stopped at race {} ms (sim {}, race start {})",
        pid, race_ms, s.srv.sim_ms, s.srv.race_start
    );
    let declared = s.tape.declared_ms.map(|v| v as i64);
    let want: Vec<(String, i64)> = declared
        .into_iter()
        .flat_map(|d| {
            vec![
                (format!("race ms {}", d), d),
                (format!("sim ms {}", d + s.srv.race_start as i64), d + s.srv.race_start as i64),
            ]
        })
        .collect();
    if want.is_empty() {
        return Err("the tape declares no finish time to look for".into());
    }
    if race_ms < declared.unwrap_or(0) {
        return Err(format!(
            "this checkpoint (race {} ms) is BEFORE the finish ({} ms) -- the result cannot be \
             in memory yet; use --at tick:N past it",
            race_ms,
            declared.unwrap_or(0)
        ));
    }

    // The typed objects a child can resolve for itself, to express hits against.
    // The POINTER WALK ONLY: past the finish the engine reads no more input
    // records, so neither the page-fault probe nor any check that simulates can
    // run here -- and neither is needed to say where an object is.
    let word = |a: u64| -> Option<u64> {
        procmem::read_at(pid, a, 8).map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
    };
    let mut anchors: Vec<(&str, u64)> = vec![("input array", s.srv.base)];
    let (controller, sim) = (s.srv.validator_controller, s.srv.validation_sim);
    if controller != 0 && sim != 0 {
        anchors.push(("controller", controller));
        anchors.push(("sim", sim));
        if let Some(pg) = word(sim + 0x18).filter(|v| *v != 0) {
            anchors.push(("playground", pg));
            let count = procmem::read_at(pid, pg + 0x668, 4)
                .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
                .unwrap_or(0);
            if let (Some(players), 1) = (word(pg + 0x660).filter(|v| *v != 0), count) {
                if let Some(part) = word(players).filter(|v| *v != 0) {
                    anchors.push(("participant", part));
                    if let Some(veh) = word(part + 0x1118).filter(|v| *v != 0) {
                        anchors.push(("vehicle", veh));
                    }
                }
            }
        }
    }
    println!(
        "anchors: {}",
        anchors.iter().map(|(n, a)| format!("{} {:#x}", n, a)).collect::<Vec<_>>().join(", ")
    );

    println!("\nstructural probe of the printer's result vector:");
    probe_result_vector(pid, &anchors, declared.unwrap_or(0));

    if std::env::var("FK_CHAIN").is_ok() {
        println!("\nbackward pointer scan from the value to a typed object:");
        let d = declared.unwrap_or(0);
        match chain_to_value(pid, &anchors, &[d, d + s.srv.race_start as i64], 3) {
            Some(c) => println!("  CHAIN: {}", c),
            None => println!("  no chain within 3 hops of any object the shim can resolve"),
        }
    }

    for (what, v) in &want {
        let needle = (*v as u32).to_le_bytes();
        let mut hits: Vec<u64> = Vec::new();
        for r in procmem::maps(pid) {
            if !r.perms.starts_with("rw") {
                continue;
            }
            let Some(buf) = procmem::read_at(pid, r.start, (r.end - r.start) as usize) else {
                continue;
            };
            let mut i = 0usize;
            while i + 4 <= buf.len() {
                if buf[i..i + 4] == needle {
                    hits.push(r.start + i as u64);
                    if hits.len() > 4000 {
                        break;
                    }
                }
                i += 4;
            }
        }
        println!("\n{} -> {} slot(s) hold it:", what, hits.len());
        for h in hits.iter().take(40) {
            let near = anchors
                .iter()
                .map(|(n, a)| (*n, *h as i64 - *a as i64))
                .min_by_key(|(_, d)| d.abs())
                .map(|(n, d)| format!("{}{:+#x}", n, d))
                .unwrap_or_default();
            println!("  {:#014x}  {}", h, near);
        }
        if hits.len() > 40 {
            println!("  ... {} more", hits.len() - 40);
        }
    }
    s.srv.quit();
    println!(
        "\nRun this on a second tape with a different finish time and keep the offsets that hold \
         each tape's OWN result. That offset is what the shim reads to end a candidate at the \
         finish instead of running the validator's print path."
    );
    Ok(())
}

/// THE RESULT VECTOR, probed structurally.
///
/// The JSON printer at `0x113b020` walks a vector the engine hangs off one
/// object at `+0x4870`: the first dword is the count, and each element carries
/// `IsValid` at +0x00, **`Time` at +0x08**, `Score` at +0x0c, `NbRespawns` at
/// +0x10 and `NbCheckpoints` at +0x20 (read straight off the `mov r8d,[rbx+..]`
/// beside each field-name string in the disassembly). This asks every typed
/// object the shim can resolve whether it is that object -- and, failing that,
/// every pointer those objects hold, one level deep.
fn probe_result_vector(pid: i32, anchors: &[(&str, u64)], want_time: i64) {
    use forkoracle::procmem::read_at;
    let w32 = |a: u64| read_at(pid, a, 4).map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()));
    let w64 = |a: u64| read_at(pid, a, 8).map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()));
    for (name, a) in anchors {
        for off in [0x4870u64, 0x4878] {
            let Some(vec_ptr) = w64(a + off) else { continue };
            if vec_ptr < 0x1000 {
                continue;
            }
            let Some(count) = w32(vec_ptr) else { continue };
            if count == 0 || count > 64 {
                continue;
            }
            println!("  {}+{:#x} -> {:#x}, count {}", name, off, vec_ptr, count);
        }
    }
    // Every pointer the anchors hold, one level deep, asked the same question:
    // does the thing it points at carry this run's Time at +8?
    for (name, a) in anchors {
        for off in (0..0x5000u64).step_by(8) {
            let Some(p) = w64(a + off) else { continue };
            if p < 0x1000 {
                continue;
            }
            let Some(t) = w32(p + 8) else { continue };
            if t as i32 as i64 == want_time {
                if let (Some(cp), Some(valid)) = (w32(p + 0x20), w32(p)) {
                    println!(
                        "  [{}+{:#x}] -> {:#x}: Time {} NbCheckpoints {} first-word {}",
                        name,
                        off,
                        p,
                        t as i32,
                        cp,
                        valid
                    );
                }
            }
        }
    }
}

// ------------------------------------------------------- fk tickhook finishfind

/// FIND THE ENGINE'S OWN "THE RACE IS OVER" WORD, by watching it happen.
///
/// A candidate costs 5.8 ms after its last simulated tick (`fk tickhook cost`)
/// and none of that is transport: it is the validator's finish-and-print path,
/// run for an answer that is one integer. A child that knew the race was over
/// -- the tick it happened -- could report it through the timing page and
/// `_exit`, skipping all of it.
///
/// The result STRUCT is not reachable from any object the shim holds (probed:
/// no pointer within 0x5000 of the controller, simulation, playground,
/// participant or vehicle points at it, and the Time word itself lands in
/// per-run heap blocks at no fixed offset). But the engine must also record, in
/// its own state, THAT the player finished -- and that word is what this looks
/// for.
///
/// The method is a differential, not a guess: gather a window of the
/// participant (or vehicle, or playground) every tick across the finish, and
/// keep the words that are constant before, constant after, and change EXACTLY
/// at the finish tick. Run it on two tapes with different finish times and only
/// the words that flip at each tape's own finish survive.
pub fn finishfind(engine: &Engine, tape: Tape, at: Checkpoint, what: &str) -> Result<(), String> {
    let mut s = Session::start(engine, tape, at)?;
    let probe = s.srv.boundary_tick(s.tape.start_offset_ms)?;
    let recs = s.tape.tail_records(probe);
    let chain = forkoracle::car::locate(&s.srv)?;
    let (name, base) = match what {
        "participant" => ("participant", chain.participant),
        "vehicle" => ("vehicle", chain.phy),
        "playground" => ("playground", chain.playground),
        "sim" => ("sim", chain.sim),
        // the RESULT BLOCK: the printer's own struct is built too late to help a
        // child, but the engine records the race's outcome in a block the
        // validation controller holds at +0x1a88 -- the backward pointer scan
        // named it, and it is the same offset with the value at the same +0xa4
        // on three tapes with three different finish times.
        "result" => {
            let p = forkoracle::procmem::read_at(s.srv.pid(), chain.controller + 0x1a88, 8)
                .map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
                .unwrap_or(0);
            if p < 0x1000 {
                return Err(format!("controller+0x1a88 is {:#x} -- no result block here", p));
            }
            ("result", p)
        }
        _ => return Err("--object participant|vehicle|playground|sim".into()),
    };
    let declared = s
        .tape
        .declared_ms
        .ok_or("the tape declares no finish time")? as i64;
    let finish_tick = forkoracle::clock::ckpt_for_race_ms(declared) as i64;
    let start_clock = s.srv.clock as i64;
    println!(
        "{} at {:#x}; the tape finishes at race {} ms = clock {}; server is at clock {}",
        name, base, declared, finish_tick, start_clock
    );

    // 8 KB of the object, in the shim's 8 segments, every tick.
    let chunk: u32 = std::env::var("FK_FF_CHUNK").ok().and_then(|v| v.parse().ok()).unwrap_or(1024);
    let from: u64 = std::env::var("FK_FF_FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let segs: Vec<(u64, u32)> = (0..8).map(|i| (base + from + i * chunk as u64, chunk)).collect();
    let width = 8 * chunk as usize;
    let want_from = finish_tick - 12;
    let ticks = (finish_tick + 8 - start_clock).max(16) as u32;
    let (_j, blob) = s.srv.run_sampled_segs_ex(
        probe,
        &recs,
        &segs,
        1,
        (ticks + 4) | crate::locate::EXIT_ON_BUDGET,
        // NO DEDUP (klen 0): one sample per tick, even when nothing changes --
        // which is the whole point when watching for the one tick where
        // something does.
        (0, 0),
        forkoracle::clock::budget_for_ticks(ticks + 8),
    );
    let recsz = 8 + width;
    let n = blob.len() / recsz;
    println!("{} samples of {} bytes", n, width);
    if n < 8 {
        return Err("too few samples".into());
    }
    let clock_of = |i: usize| u64::from_le_bytes(blob[i * recsz..i * recsz + 8].try_into().unwrap()) as i64;
    let word = |i: usize, o: usize| {
        u32::from_le_bytes(blob[i * recsz + 8 + o..i * recsz + 8 + o + 4].try_into().unwrap())
    };
    // index of the first sample at or after the finish tick
    let Some(fi) = (0..n).find(|&i| clock_of(i) >= finish_tick) else {
        return Err(format!(
            "the run never reached the finish tick (last clock {})",
            clock_of(n - 1)
        ));
    };
    println!(
        "the finish tick is sample {} of {} (clock {} .. {})",
        fi,
        n,
        clock_of(0),
        clock_of(n - 1)
    );
    // ONE TRANSITION IN THE WHOLE WINDOW, and where. Requiring the change to
    // land exactly on the finish tick found nothing on either object -- so ask
    // the looser question and read the answer: a word that settles once, near
    // the finish, is the candidate; a word that changes every tick is physics.
    let lo = (0..n).find(|&i| clock_of(i) >= want_from).unwrap_or(0);
    let mut once: Vec<(usize, usize, u32, u32)> = Vec::new();
    for o in (0..width - 4).step_by(4) {
        let mut trans = 0usize;
        let mut at = 0usize;
        for i in lo + 1..n {
            if word(i, o) != word(i - 1, o) {
                trans += 1;
                at = i;
                if trans > 1 {
                    break;
                }
            }
        }
        if trans == 1 {
            once.push((at, o, word(at - 1, o), word(at, o)));
        }
    }
    once.sort_by_key(|(at, _, _, _)| (*at as i64 - fi as i64).abs());
    println!(
        "{} word(s) change exactly ONCE in the window [{}..{}]; nearest the finish first:",
        once.len(),
        clock_of(lo),
        clock_of(n - 1)
    );
    for (at, o, b, a) in once.iter().take(12) {
        println!(
            "  {}+{:#06x}  at clock {} ({:+} from the finish): {} -> {}   (i32 {} -> {}; f32 {:.4} -> {:.4})",
            name,
            o,
            clock_of(*at),
            clock_of(*at) - finish_tick,
            b,
            a,
            *b as i32,
            *a as i32,
            f32::from_bits(*b),
            f32::from_bits(*a)
        );
    }

    // THE TIME ITSELF. The finish is NOT a tick boundary -- rank00100 finishes
    // at 22884 ms, and the tick that detects it is 22880 -- so the engine
    // interpolates the crossing within the tick and stores the answer
    // somewhere. Any word whose value lands within one tick of the declared
    // time (as race ms, or as sim ms) is a candidate for where.
    println!("\nwords holding something within a tick of the declared finish:");
    let mut near = 0;
    for o in (0..width - 4).step_by(4) {
        let v = word(n - 1, o) as i64;
        let as_race = (v - declared).abs();
        let as_sim = (v - declared - s.srv.race_start as i64).abs();
        if (as_race <= 10 || as_sim <= 10) && near < 20 {
            near += 1;
            println!(
                "  {}+{:#06x} = {} (declared {} race, {} sim; first at sample {})",
                name,
                from as usize + o,
                v,
                declared,
                declared + s.srv.race_start as i64,
                (0..n).find(|&i| word(i, o) as i64 == v).unwrap_or(0)
            );
        }
    }
    if near == 0 {
        println!("  (none in this window)");
    }

    // AND THE COUNTERS: a word that only ever goes up, in steps of one, ending
    // small. That is what a checkpoint count looks like, and a DNF needs it --
    // a child that exits early still has to say how far it got.
    println!("\nmonotone small counters over the same window:");
    let mut shown = 0;
    for o in (0..width - 4).step_by(4) {
        let last = word(n - 1, o);
        if last == 0 || last > 32 {
            continue;
        }
        let mut up = 0usize;
        let mut ok = true;
        for i in lo + 1..n {
            let (p, c) = (word(i - 1, o), word(i, o));
            if c == p {
                continue;
            }
            if c != p + 1 {
                ok = false;
                break;
            }
            up += 1;
        }
        if ok && up >= 1 && shown < 12 {
            shown += 1;
            println!(
                "  {}+{:#06x}: {} -> {} in {} step(s) of one",
                name,
                o,
                word(lo, o),
                last,
                up
            );
        }
    }
    s.srv.quit();
    Ok(())
}

/// FIND THE POINTER CHAIN from a typed object to the finish time, backwards.
///
/// The time is in memory two ticks after the finish and long before anything is
/// printed -- 18 to 21 copies of it -- but at no fixed offset from anything:
/// two runs of the SAME tape put it 0x50 apart. So it lives in a per-run
/// allocation, and the only durable way to it is a POINTER some typed object
/// holds.
///
/// Searching forwards is hopeless (a pointer graph of a 150 MB heap); searching
/// backwards is not. Snapshot the writable memory once, take the addresses that
/// hold the value, find every word that points INTO the block containing one of
/// them, then repeat -- and stop the moment a hop lands inside an object the
/// shim can already resolve. What comes back is a chain of offsets a child can
/// walk in nanoseconds.
fn chain_to_value(
    pid: i32,
    anchors: &[(&str, u64)],
    values: &[i64],
    depth: usize,
) -> Option<String> {
    use forkoracle::procmem;
    let mut mem: Vec<(u64, Vec<u8>)> = Vec::new();
    for r in procmem::maps(pid) {
        if !r.perms.starts_with("rw") || r.path == "[vvar]" || r.path == "[vsyscall]" {
            continue;
        }
        if let Some(b) = procmem::read_at(pid, r.start, (r.end - r.start) as usize) {
            mem.push((r.start, b));
        }
    }
    let total: usize = mem.iter().map(|(_, b)| b.len()).sum();
    println!("  snapshot: {} regions, {:.1} MB", mem.len(), total as f64 / 1e6);

    // every word that points into [target - SLACK, target]
    const SLACK: u64 = 0x1000;
    let mut level: Vec<u64> = Vec::new();
    for (start, buf) in &mem {
        let mut o = 0usize;
        while o + 4 <= buf.len() {
            let v = i32::from_le_bytes(buf[o..o + 4].try_into().unwrap()) as i64;
            if values.contains(&v) {
                level.push(start + o as u64);
            }
            o += 4;
        }
    }
    println!("  {} word(s) hold the value", level.len());
    let mut edges: Vec<(u64, u64)> = Vec::new();
    for d in 1..=depth {
        let mut next: Vec<u64> = Vec::new();
        for (start, buf) in &mem {
            let mut o = 0usize;
            while o + 8 <= buf.len() {
                let p = u64::from_le_bytes(buf[o..o + 8].try_into().unwrap());
                if p >= 0x1000 {
                    if let Some(t) = level.iter().find(|t| **t >= p && **t - p <= SLACK) {
                        let here = start + o as u64;
                        next.push(here);
                        edges.push((here, *t));
                    }
                }
                o += 8;
            }
        }
        next.sort_unstable();
        next.dedup();
        println!("  depth {}: {} pointer(s) into those blocks", d, next.len());
        // did any of them land inside an object we can already name?
        let mut found: Vec<String> = Vec::new();
        for (name, a) in anchors {
            if *name == "input array" {
                // the tape's own allocation: a hit just past its end is heap
                // adjacency (the array is 32 bytes x records), not structure
                continue;
            }
            for p in &next {
                if *p >= *a && *p - *a < 0x20_000 {
                    let t = edges.iter().find(|(f, _)| f == p).map(|(_, t)| *t).unwrap_or(0);
                    let base = u64::from_le_bytes(
                        procmem::read_at(pid, *p, 8).unwrap()[..8].try_into().unwrap(),
                    );
                    found.push(format!(
                        "{}+{:#x} -> {:#x}, value at +{:#x} (depth {})",
                        name,
                        p - a,
                        base,
                        t - base,
                        d
                    ));
                }
            }
        }
        if !found.is_empty() {
            for f in found.iter().take(20) {
                println!("    {}", f);
            }
            return Some(format!("{} candidate chain(s) at depth {}", found.len(), d));
        }
        if next.is_empty() || next.len() > 200_000 {
            break;
        }
        level = next;
    }
    None
}

// ------------------------------------------------------ fk tickhook finishcheck

/// THE CONTROL ON THE EXIT-AT-FINISH LEVER: both numbers, from the same child.
///
/// `FKSHIM_EXIT_AT_FINISH=check` makes a child record the engine's result word
/// and then carry on and print the JSON anyway, so one run produces the fast
/// answer and the slow one for the same simulation. They must be equal on every
/// candidate -- finishers and DNFs alike, where a DNF must produce no fast
/// answer at all rather than a wrong one.
pub fn finishcheck(
    engine: &Engine,
    tape: Tape,
    at: Checkpoint,
    n: usize,
    seed: u64,
) -> Result<(), String> {
    // THIS TOOL ARMS ITS OWN CONTROL. In the normal mode the child LEAVES at the
    // finish, so there is no JSON to compare against and every candidate reads
    // as a disagreement -- a check that fails because it was not set up is
    // worse than no check. Check mode records the fast answer and prints the
    // JSON anyway, from one simulation.
    std::env::set_var("FKSHIM_FINISH_CHECK", "1");
    let mut s = Session::start(engine, tape, at)?;
    let probe = s.srv.boundary_tick(s.tape.start_offset_ms)?;
    let recs = s.tape.tail_records(probe);
    let tail = recs.len();
    let declared = s.tape.declared_ms.ok_or("the tape declares no finish time")? as i64;
    let (addr, sentinel) = forkoracle::finish::calibrate(&mut s.srv, probe, &recs, declared)?;
    println!(
        "checkpoint tick {}, {} tail ticks, {} candidates, seed {}\n\
         the finish word is {:#x} (holds {} until the race ends)",
        probe, tail, n, seed, addr, sentinel as i32
    );
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let mut next = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (rng >> 33) as u64
    };
    // THE TAPE ENDS AT A TICK, NOT AT A MILLISECOND.
    //
    // The last record is consumed by tick `probe + tail - 1`, and a crossing
    // PARTWAY THROUGH that tick is still driven by the tape -- it just carries a
    // timestamp a few ms past the record's own. Comparing milliseconds called
    // 111 of 200 legitimate finishes "past the end" on 145875, where most
    // candidates cross during the very tick the tape ends on.
    let last_record_tick = probe as i64 + tail as i64 - 1;
    let finish_tick_of = |ms: i64| (ms - s.tape.start_offset_ms as i64).div_euclid(10);
    let (mut agree, mut fast_only, mut slow_only, mut disagree) = (0usize, 0, 0, 0);
    let (mut cps_ok, mut cps_bad, mut cps_off) = (0usize, 0usize, 0usize);
    let mut cps_above = 0usize;
    let mut past = 0usize;
    let mut past_bad = 0usize;
    let (mut late, mut late_unflagged) = (0usize, 0usize);
    let (mut finishers, mut dnfs) = (0usize, 0usize);
    let mut worst: Vec<String> = Vec::new();
    for c in 0..n {
        let mut r = recs.clone();
        // a handful of steer nudges in the tail: enough to make some candidates
        // finish with a different time and some not finish at all
        let k = 1 + (next() % 4) as usize;
        for _ in 0..k {
            let i = (next() as usize) % r.len();
            let d = ((next() % 2001) as f32 - 1000.0) / 1000.0;
            r[i].steer = (r[i].steer + d).clamp(-1.0, 1.0);
        }
        let out = s.srv.run(probe, &r);
        let fast = out
            .lines()
            .find_map(|l| l.trim().strip_prefix("FKFINISH race_ms "))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|v| v.parse::<i64>().ok());
        // the JSON's own answer, read the way the driver reads it
        let mut slow = None;
        let mut in_validated = false;
        for line in out.lines() {
            let t = line.trim();
            if t.starts_with("\"ValidatedResult\"") {
                in_validated = !t.contains("null");
            } else if in_validated && t.starts_with("\"Time\"") {
                slow = t
                    .split(':')
                    .nth(1)
                    .and_then(|v| v.trim().trim_end_matches(',').parse::<i64>().ok());
                in_validated = false;
            }
        }
        // the DNF half: the child that ran out of tape reports its checkpoint
        // count, and the JSON says the same thing in prose
        let past_end = out.lines().any(|l| l.trim().starts_with("FKPASTEND "));
        if past_end {
            past += 1;
            // AND CHECK THE CLASSIFICATION, do not just count it: a genuine
            // finish is at or before the tape's own last race ms, so a
            // PAST-END verdict on a time inside the tape would be this guard
            // mislabelling a real finish -- the failure mode that made its
            // first version a regression.
            if let Some(t) = slow {
                if finish_tick_of(t) <= last_record_tick {
                    past_bad += 1;
                    if worst.len() < 8 {
                        worst.push(format!(
                            "c{:04}: called PAST-END but the JSON finished at {} ms = tick {}, \
                             inside the tape (last record tick {})",
                            c,
                            t,
                            finish_tick_of(t),
                            last_record_tick
                        ));
                    }
                }
            }
        }
        // Is the class even PRESENT in this sample? A flag that reads 0 because
        // nothing crossed late is not the same as a flag that is broken, and
        // only the JSON's own time can tell the two apart.
        if let Some(t) = slow {
            if finish_tick_of(t) > last_record_tick {
                late += 1;
                if !past_end {
                    late_unflagged += 1;
                    if worst.len() < 8 {
                        worst.push(format!(
                            "c{:04}: the JSON finished at {} ms = tick {}, past the tape's last \
                             record tick {}, and the guard did NOT flag it",
                            c,
                            t,
                            finish_tick_of(t),
                            last_record_tick
                        ));
                    }
                }
            }
        }
        let engine_cps = out
            .lines()
            .find_map(|l| l.trim().strip_prefix("FKCPS "))
            .and_then(|r| r.split_whitespace().next())
            .and_then(|v| v.parse::<u32>().ok());
        let slow_cps = out.lines().find_map(|l| {
            let t = l.trim();
            if !t.starts_with("\"Desc\"") {
                return None;
            }
            if let Some(p) = t.find("reached some checkpoints (") {
                t[p + "reached some checkpoints (".len()..]
                    .split(' ')
                    .next()
                    .and_then(|s| s.trim().parse::<u32>().ok())
            } else if t.contains("wrong simu") {
                Some(0)
            } else {
                None
            }
        });
        // THE ORDERING CONDITION on adopting the engine's count: wherever the
        // JSON is a MEASUREMENT (it named a number >= 2) the two must be
        // EQUAL, and everywhere else the engine's must be >= the JSON's --
        // never lower. The JSON is a lower bound (a lone checkpoint is
        // invisible to it), so "engine below JSON" would mean the counter is
        // wrong, while "engine above" is the bound being loose.
        if fast.is_none() && slow.is_none() {
            match (engine_cps, slow_cps) {
                (Some(e), Some(j)) if j >= 2 && e != j => {
                    cps_bad += 1;
                    if worst.len() < 10 {
                        worst.push(format!(
                            "c{:04}: DNF, the JSON MEASURED {} checkpoints and the engine says {}",
                            c, j, e
                        ));
                    }
                }
                (Some(e), Some(j)) if e < j => {
                    cps_bad += 1;
                    if worst.len() < 10 {
                        worst.push(format!(
                            "c{:04}: DNF, the engine says {} which is BELOW the JSON's bound {}",
                            c, e, j
                        ));
                    }
                }
                (Some(e), Some(j)) => {
                    if e > j {
                        cps_above += 1;
                    }
                    cps_ok += 1;
                }
                _ => cps_off += 1,
            }
        }
        match (fast, slow) {
            (Some(a), Some(b)) if a == b => {
                agree += 1;
                finishers += 1;
            }
            (Some(a), Some(b)) => {
                disagree += 1;
                finishers += 1;
                if worst.len() < 10 {
                    worst.push(format!("c{:04}: fast {} vs JSON {}", c, a, b));
                }
            }
            (Some(a), None) => {
                fast_only += 1;
                if worst.len() < 10 {
                    worst.push(format!("c{:04}: fast {} but the JSON reported no time", c, a));
                }
            }
            (None, Some(b)) => {
                slow_only += 1;
                finishers += 1;
                if worst.len() < 10 {
                    worst.push(format!("c{:04}: the JSON says {} and the fast path saw nothing", c, b));
                }
            }
            (None, None) => dnfs += 1,
        }
    }
    println!(
        "\n{} candidates: {} finished, {} did not\n\
         \x20 agree                {}\n\
         \x20 DISAGREE             {}\n\
         \x20 fast answered, JSON did not   {}\n\
         \x20 JSON answered, fast did not   {}\n\
         \x20 DNF cps: engine agrees with the JSON   {}\n\
         \x20 DNF cps: engine ABOVE the JSON bound   {}\n\
         \x20 DNF cps: engine WRONG (below, or != a measured >=2)  {}\n\
         \x20 DNF cps: no engine count                {}\n\
         \x20 finished PAST the tape's end (guard fired)  {}\n\
         \x20 of those, WRONGLY (the JSON finished inside the tape)  {}\n\
         \x20 the JSON itself finished past the tape's end   {}\n\
         \x20 of those, NOT flagged by the guard             {}",
        n, finishers, dnfs, agree, disagree, fast_only, slow_only, cps_ok, cps_above, cps_bad,
        cps_off, past, past_bad, late, late_unflagged
    );
    for w in &worst {
        println!("  {}", w);
    }
    s.srv.quit();
    if disagree + fast_only + slow_only + cps_bad + past_bad + late_unflagged > 0 {
        return Err(format!(
            "{} of {} candidates disagree -- the fast path is NOT the validator's answer",
            disagree + fast_only + slow_only + cps_bad + past_bad + late_unflagged,
            n
        ));
    }
    println!("\nPASS: every candidate's fast answer is the validator's own");
    Ok(())
}

// -------------------------------------------------------------- fk tickhook dnf

/// WHAT THE ENGINE DOES WHEN A CANDIDATE DOES NOT FINISH.
///
/// A DNF pays the same 6.02 ms epilogue as a finisher (`fk tickhook cost
/// --dnf`), and in a search DNFs are the majority — 307 of 400 candidates at an
/// early checkpoint. The finish lever cannot help them: their finish word is
/// never written.
///
/// So this asks the same question the finish hunt asked, on a candidate steered
/// off the road: which word in the participant changes ONCE, near the end, and
/// what does the checkpoint counter do? The counter's location is known
/// (`forkoracle::car::CP_COUNT_IN_PARTICIPANT`, verified 200/200 against the
/// plain oracle by the tm-player project's ENV arm); what is not known is
/// whether the engine records "this run is over" anywhere a child could read.
pub fn dnf(engine: &Engine, tape: Tape, at: Checkpoint) -> Result<(), String> {
    let mut s = Session::start(engine, tape, at)?;
    let probe = s.srv.boundary_tick(s.tape.start_offset_ms)?;
    let mut recs = s.tape.tail_records(probe);
    let tape_ticks = recs.len();
    // steer it off the road: this candidate will not finish
    for r in recs.iter_mut().take(40) {
        r.steer = 1.0;
    }
    let chain = forkoracle::car::locate(&s.srv)?;
    let cp_addr = chain.participant + forkoracle::car::CP_COUNT_IN_PARTICIPANT;
    println!(
        "participant {:#x}, cp counter {:#x}; the tape has {} tail ticks from tick {}",
        chain.participant, cp_addr, tape_ticks, probe
    );

    let chunk = 512u32;
    let segs: Vec<(u64, u32)> = (0..8).map(|i| (chain.participant + i * chunk as u64, chunk)).collect();
    let width = 8 * chunk as usize;
    let (json, blob) = s.srv.run_sampled_segs_ex(
        probe,
        &recs,
        &segs,
        1,
        8192,
        (0, 0), // no dedup: one sample per tick
        forkoracle::clock::budget_for_ticks(tape_ticks as u32 + 400),
    );
    let recsz = 8 + width;
    let n = blob.len() / recsz;
    let clock_of =
        |i: usize| u64::from_le_bytes(blob[i * recsz..i * recsz + 8].try_into().unwrap()) as i64;
    let word = |i: usize, o: usize| {
        u32::from_le_bytes(blob[i * recsz + 8 + o..i * recsz + 8 + o + 4].try_into().unwrap())
    };
    if n < 8 {
        return Err(format!("only {} samples", n));
    }
    let last_tape_clock = clock_of(0) + tape_ticks as i64 - 1;
    println!(
        "{} samples, clock {} .. {} (the tape's own last tick is {}, so the engine ran {} ticks \
         past it)",
        n,
        clock_of(0),
        clock_of(n - 1),
        last_tape_clock,
        clock_of(n - 1) - last_tape_clock
    );
    println!(
        "the JSON says: {}",
        json.lines()
            .find(|l| l.trim().starts_with("\"Desc\""))
            .unwrap_or("(no Desc line)")
            .trim()
    );
    println!(
        "the cp counter went {} -> {}",
        word(0, forkoracle::car::CP_COUNT_IN_PARTICIPANT as usize),
        word(n - 1, forkoracle::car::CP_COUNT_IN_PARTICIPANT as usize)
    );
    // Which words settle once, and when relative to the tape's end?
    let mut once: Vec<(usize, usize, u32, u32)> = Vec::new();
    for o in (0..width - 4).step_by(4) {
        let mut trans = 0usize;
        let mut at = 0usize;
        for i in 1..n {
            if word(i, o) != word(i - 1, o) {
                trans += 1;
                at = i;
                if trans > 2 {
                    break;
                }
            }
        }
        if (1..=2).contains(&trans) && clock_of(at) > last_tape_clock - 200 {
            once.push((at, o, word(at - 1, o), word(at, o)));
        }
    }
    once.sort_by_key(|(at, _, _, _)| *at);
    println!(
        "\n{} word(s) in the participant settle in the last 200 ticks:",
        once.len()
    );
    for (at, o, b, a) in once.iter().take(24) {
        println!(
            "  participant+{:#06x}  at clock {} ({:+} from the tape's end): {} -> {}",
            o,
            clock_of(*at),
            clock_of(*at) - last_tape_clock,
            *b as i32,
            *a as i32
        );
    }
    s.srv.quit();
    Ok(())
}
