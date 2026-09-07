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
/// 3. **the race clock** -- the u32 the blind locator picks by its `+10 every
///    tick` signature, read at a tick whose race time the hook knows exactly;
/// 4. **the car state** -- the position the blind locator picks vs the one the
///    VALIDATOR's own ownership chain resolves, which share no evidence;
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

    // 3+4. THE CAR AND THE CLOCK, found two ways that share no evidence: the
    //    VALIDATOR'S OWN ownership chain (typed, no search) and the blind
    //    locator the search uses (a float triple whose derivative matches the
    //    velocity 12 bytes later).
    //
    //    The blind one is given a box around the validator car, because that is
    //    how production runs it -- `tmsearch` bounds it by the reference line --
    //    and because UNBOUNDED it does not work: with the whole world allowed it
    //    picked a STATIONARY object 1624 m from the car on 126859 and 1192 m on
    //    145875, both of which pass its own self-consistency test (nothing moves,
    //    so d(pos)/dt matches a zero velocity). A 200 m box does not choose
    //    between candidates 0.8 m apart; it excludes ones a kilometre away.
    let recs = s.tape.tail_records(probe);
    let world = (-64000.0, 64000.0, -1000.0, 4000.0, -64000.0, 64000.0);
    let car = crate::validator::ValidatorCar::locate(&mut s.srv, probe, &recs, off, world, 4000, false)
        .map_err(|e| format!("the validator's ownership chain did not resolve: {}", e))?;
    let cpos = {
        let b = read_at(pid, car.layout().pos, 12).ok_or("cannot read the validator car")?;
        [
            f32::from_le_bytes(b[0..4].try_into().unwrap()) as f64,
            f32::from_le_bytes(b[4..8].try_into().unwrap()) as f64,
            f32::from_le_bytes(b[8..12].try_into().unwrap()) as f64,
        ]
    };
    let r = 200.0;
    let bounds = (cpos[0] - r, cpos[0] + r, cpos[1] - r, cpos[1] + r, cpos[2] - r, cpos[2] + r);
    let layout = forkoracle::blind::locate_blind(&mut s.srv, probe, &recs, off, 1, bounds, false)
        .map_err(|e| format!("the blind locator could not find the car: {}", e))?;
    let clk = read_at(pid, layout.clock, 4)
        .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()) as i64)
        .ok_or("cannot read the located clock")?;
    println!(
        "  located clock {:#x} reads {} ms; race time here is {} ms (difference {})",
        layout.clock, clk, race_ms, clk - race_ms
    );
    // WHICH u32 NEAR THE CAR EQUALS RACE TIME? The locator picks by a `+10
    // every tick` signature, and several counters in the engine step by 10.
    // This says what the neighbourhood actually holds.
    if let Some(win) = read_at(pid, layout.clock - 256, 1024) {
        let hits: Vec<String> = (0..win.len() / 4)
            .map(|i| (i, u32::from_le_bytes(win[i * 4..i * 4 + 4].try_into().unwrap()) as i64))
            .filter(|(_, v)| (*v - race_ms).abs() <= 2000 && *v > 0)
            .map(|(i, v)| format!("{:+}:{}({:+})", i as i64 * 4 - 256, v, v - race_ms))
            .collect();
        println!("  u32 within 2 s of race time in [clock-256, clock+768): {}", hits.join(" "));
    }
    // It is NOT the race clock: it counts from the ROUND start (1200 ms here,
    // 1000 ms before the race). What must hold is that the MEASURED bias turns
    // it into race time exactly -- that is what labels every sample.
    let bias = forkoracle::layout::measured_clock_bias(&s.srv, layout.clock)?;
    let finished = race_ms - 10;
    check(
        clk - bias == finished && bias == layout.clock_bias,
        format!(
            "counter - bias = {} ms = the race time of the tick the server has FINISHED ({}), \
             and the locator carries the same bias ({} vs {})",
            clk - bias,
            finished,
            layout.clock_bias,
            bias
        ),
    );

    {
        {
            let p = car.provenance();
            println!(
                "  validator chain: controller {:#x} -> sim {:#x} -> playground {:#x} -> \
                 participant {:#x} -> vehicle {:#x} -> pos {:#x}",
                p.controller, p.sim, p.playground, p.participant, p.vehicle, p.state_pos
            );
            // The two locators resolve DIFFERENT OBJECTS by design (a vis state
            // and the CGameVehiclePhy), so the check is that they agree about
            // the CAR -- same position, to float precision.
            let xyz = |a: u64| -> Option<[f32; 3]> {
                let b = read_at(pid, a, 12)?;
                Some([
                    f32::from_le_bytes(b[0..4].try_into().unwrap()),
                    f32::from_le_bytes(b[4..8].try_into().unwrap()),
                    f32::from_le_bytes(b[8..12].try_into().unwrap()),
                ])
            };
            let (a, b) = (xyz(layout.pos), xyz(car.layout().pos));
            println!("  blind pos {:#x} = {:?}", layout.pos, a);
            println!("  chain pos {:#x} = {:?}", car.layout().pos, b);
            // They are DIFFERENT OBJECTS -- a vis state and the CGameVehiclePhy --
            // and they hold the same car at instants one tick apart, so the
            // test is that the gap is one tick of travel and not a second car.
            let vel = xyz(layout.pos + 12);
            match (a, b, vel) {
                (Some(a), Some(b), Some(v)) => {
                    let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                    let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    check(
                        d <= speed * 0.01 + 0.05,
                        format!(
                            "the two locators hold the same car one tick apart: {:.4} m apart at \
                             {:.2} m/s, i.e. {:.2} ticks of travel",
                            d,
                            speed,
                            d / (speed * 0.01).max(1e-6)
                        ),
                    );
                }
                _ => check(false, "one of the two car positions could not be read".to_string()),
            }
            let cclk = read_at(pid, car.layout().clock, 4)
                .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()) as i64)
                .unwrap_or(-1);
            println!(
                "  chain clock {:#x} reads {} ms ({:+} vs race)",
                car.layout().clock,
                cclk,
                cclk - race_ms
            );
        }
    }

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
