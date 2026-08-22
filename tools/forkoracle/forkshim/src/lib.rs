//! fkshim -- LD_PRELOAD fork server for the TM2020 dedicated server.
//!
//! # What it does
//!
//! The server simulates a ghost's input tape tick by tick. Re-simulating from
//! tick 0 for every candidate wastes the whole shared prefix. This shim stops
//! the process *inside* the simulation, holds that state as a checkpoint, and
//! forks a child per candidate: the child rewrites the still-unread part of the
//! decoded input array and runs only the tail.
//!
//! # The three mechanisms
//!
//! 1. **A deterministic clock.** The engine calls `lroundf` ~25.5 times per
//!    simulated millisecond, and the total for a given (map, ghost) is bit-exact
//!    across runs. Interposing it gives a reproducible cursor into the middle of
//!    a simulation with no debugger, no disassembly and no ptrace.
//!
//! 2. **The decoded input array.** The ghost's bitstream is decoded up front (it
//!    is *not* read during the simulation) into one 32-byte record per 10 ms
//!    tick: `f32 steer = (i8)steer/127`, `f32 gas`, `f32 brake`, then engine
//!    fields. The shim finds it by searching its own address space for the
//!    reference ghost's steer sequence at stride 32.
//!
//! 3. **fork() from the checkpoint.** The engine simulates on the main thread
//!    (the only other thread, `NetPoll`, sleeps), so the fork child is a
//!    complete, self-consistent simulator. The parent never advances.
//!
//! # Protocol
//!
//! Driven over two inherited fds (`FKSHIM_CMD_FD`, `FKSHIM_RES_FD`). Frames are
//! `u32 len` + payload. The shim announces `READY <base> <clock>`, then serves
//! commands until `Q`:
//!   `R <n> [tick steer gas brake]*n`  -- fork, patch, run, return the child's
//!                                        stdout (the validator's JSON block)

use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};

#[path = "../../forkoracle/src/pred_core.rs"]
pub mod pred_core;
use pred_core::{Eval, Pred, RefLine, Summary, MAXP, PRED_BYTES, SUMMARY_BYTES};

const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;

extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn getenv(name: *const c_char) -> *const c_char;
    fn atoll(s: *const c_char) -> i64;
    fn raise(sig: c_int) -> c_int;
    fn fork() -> c_int;
    fn _exit(code: c_int) -> !;
    fn pipe(fds: *mut c_int) -> c_int;
    fn dup2(old: c_int, new: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, opts: c_int) -> c_int;
    fn signal(sig: c_int, h: usize) -> usize;
    fn fflush(f: *mut c_void) -> c_int;
    fn setvbuf(f: *mut c_void, buf: *mut c_char, mode: c_int, size: usize) -> c_int;
    fn poll(fds: *mut PollFd, n: u64, timeout: c_int) -> c_int;
    fn open(path: *const c_char, flags: c_int, mode: c_int) -> c_int;
    static mut stdout: *mut c_void;
}

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

const SIGSTOP: c_int = 19;
const SIGKILL: c_int = 9;
const SIGCHLD: c_int = 17;
const SIG_IGN: usize = 1;
const IONBF: c_int = 2;
const POLLIN: i16 = 0x001;

static N_LROUNDF: AtomicU64 = AtomicU64::new(0);
static STOP_AT: AtomicU64 = AtomicU64::new(u64::MAX);
static CKPT_AT: AtomicU64 = AtomicU64::new(u64::MAX);
static ARMED: AtomicUsize = AtomicUsize::new(0);
static IS_CHILD: AtomicUsize = AtomicUsize::new(0);
static INIT: AtomicUsize = AtomicUsize::new(0);
static REAL_LROUNDF: AtomicUsize = AtomicUsize::new(0);
static REAL_WRITE: AtomicUsize = AtomicUsize::new(0);
static CMD_FD: AtomicI32 = AtomicI32::new(-1);
static RES_FD: AtomicI32 = AtomicI32::new(-1);

// ------------------------------------------------------------------ sampling
//
// The point of the whole exercise: a forked child does not just produce a
// finish time, it can report the car's state as it simulates. `lroundf` is
// already on the simulation's hot path (~255 calls per 10 ms tick), so the hook
// doubles as a sampling clock: every `SAMPLE_STRIDE` calls, copy a window of
// the vehicle struct out to a dedicated pipe.
//
// Dedup is what makes this tick-accurate without knowing where the engine keeps
// its tick counter: the physics integrator writes the state once per tick, so
// sampling several times per tick and emitting only on change yields exactly
// the distinct states, in order.

static SAMPLE_ADDR: AtomicUsize = AtomicUsize::new(0);
static SAMPLE_LEN: AtomicUsize = AtomicUsize::new(0);
/// Up to 8 (addr, len) segments gathered into one contiguous record. The
/// engine's race clock sits ~8 KB away from the vehicle state, and streaming
/// the 8 KB between them every tick would cost more than the simulation does;
/// gathering two small segments costs nothing.
const MAX_SEG: usize = 8;
static SEG_N: AtomicUsize = AtomicUsize::new(0);
static SEG_ADDR: [AtomicUsize; MAX_SEG] = [
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
];
static SEG_LEN: [AtomicUsize; MAX_SEG] = [
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
];
static SAMPLE_FD: AtomicI32 = AtomicI32::new(-1);
static SAMPLE_STRIDE: AtomicU64 = AtomicU64::new(0);
static SAMPLE_NEXT: AtomicU64 = AtomicU64::new(u64::MAX);
static SAMPLE_LEFT: AtomicU64 = AtomicU64::new(0);
static SAMPLE_DEDUP: AtomicUsize = AtomicUsize::new(0); // key length, 0 = off
static SAMPLE_KEYOFF: AtomicUsize = AtomicUsize::new(0);
/// Absolute `lroundf` count past which the child stops simulating (0 = never).
///
/// WHY: a locate probe wants 6 or 150 TICKS, and without this the child
/// simulates the whole remaining tape -- on a 440 s record that is 43 000 ticks
/// for six ticks of data, which is what made the blind locate cost 5.5 minutes
/// per attempt and put a general fix out of reach on long tapes.
static SAMPLE_DEADLINE: AtomicU64 = AtomicU64::new(0);
/// Exit the child the moment the sample budget is spent (1 = yes).
static SAMPLE_EXIT: AtomicUsize = AtomicUsize::new(0);
/// GRID GATE. Sample only when the engine's race clock is on the record's own
/// 50 ms grid: `(clock_value - GATE_PHASE) % GATE_MOD == 0`.
///
/// WHY: a ghost sample exists only every 50 ms, so a snapshot taken at any
/// other tick can be compared with a recorded value only by tolerating a slop
/// of up to 25 ms -- and at 70 m/s that slop is 1.75 m, which drags the
/// correlation of a slot that IS the position down below the correlation of
/// junk. Gating in the shim makes every snapshot land on an instant the answer
/// key actually has, so the comparison can be exact.
static GATE_ADDR: AtomicUsize = AtomicUsize::new(0);
static GATE_MOD: AtomicU64 = AtomicU64::new(0);
static GATE_PHASE: AtomicU64 = AtomicU64::new(0);
static SAMPLE_PREV: AtomicUsize = AtomicUsize::new(0); // *mut u8, len SAMPLE_LEN
static SAMPLE_BUF: AtomicUsize = AtomicUsize::new(0); // *mut u8, len 8 + SAMPLE_LEN

/// Gather the watched segments out, if the key slice changed. Called from the
/// `lroundf` hook in the child only.
///
/// Dedup is on a *key* slice of the gathered record rather than the whole
/// thing: the record has to be wide enough to catch the neighbouring fields,
/// but the engine touches some of those several times per tick, and only the
/// race clock marks a tick boundary unconditionally.
#[inline(never)]
unsafe fn do_sample(clock: u64) {
    // The simulated-time deadline is checked FIRST and before the budget test
    // below, which parks the hook (`SAMPLE_NEXT = MAX`) and would otherwise
    // stop this from ever being reached again.
    let dl = SAMPLE_DEADLINE.load(Ordering::Relaxed);
    if dl != 0 && clock > dl {
        _exit(0)
    }
    let stride = SAMPLE_STRIDE.load(Ordering::Relaxed);
    let gm = GATE_MOD.load(Ordering::Relaxed);
    if gm != 0 {
        // A REJECTED gate check must re-arm for the NEXT call, not for the next
        // stride: advancing by a whole stride here means each sample needs the
        // one lroundf call `stride` ahead to land on the grid by luck, which
        // turned a request for 64 snapshots into 17.
        let ga = GATE_ADDR.load(Ordering::Relaxed) as *const u32;
        if ga.is_null() {
            SAMPLE_NEXT.store(clock + stride, Ordering::Relaxed);
            return;
        }
        let c = std::ptr::read_volatile(ga) as u64;
        // PHASE u32::MAX means "whatever this process's clock is congruent to".
        // `find_clock2` returns a CLASS of counters and two processes can land
        // on ones with different absolute offsets, so a phase computed in the
        // anchor process can match NOTHING here -- measured as "0 instants
        // sampled" on a third of the corpus.
        if GATE_PHASE.load(Ordering::Relaxed) == u32::MAX as u64 {
            GATE_PHASE.store(c % gm, Ordering::Relaxed);
        }
        if c.wrapping_sub(GATE_PHASE.load(Ordering::Relaxed)) % gm != 0 {
            SAMPLE_NEXT.store(clock + 1, Ordering::Relaxed);
            return;
        }
    }
    SAMPLE_NEXT.store(clock + stride, Ordering::Relaxed);
    let n = SAMPLE_LEFT.load(Ordering::Relaxed);
    if n == 0 {
        if SAMPLE_EXIT.load(Ordering::Relaxed) != 0 {
            _exit(0)
        }
        SAMPLE_STRIDE.store(0, Ordering::Relaxed);
        SAMPLE_NEXT.store(u64::MAX, Ordering::Relaxed);
        return;
    }
    let len = SAMPLE_LEN.load(Ordering::Relaxed);
    let buf = SAMPLE_BUF.load(Ordering::Relaxed) as *mut u8;
    if buf.is_null() {
        return;
    }
    let nseg = SEG_N.load(Ordering::Relaxed);
    let mut o = 0usize;
    for s in 0..nseg {
        let a = SEG_ADDR[s].load(Ordering::Relaxed) as *const u8;
        let l = SEG_LEN[s].load(Ordering::Relaxed);
        std::ptr::copy_nonoverlapping(a, buf.add(8 + o), l);
        o += l;
    }
    let klen = SAMPLE_DEDUP.load(Ordering::Relaxed);
    let koff = SAMPLE_KEYOFF.load(Ordering::Relaxed);
    let prev = SAMPLE_PREV.load(Ordering::Relaxed) as *mut u8;
    if klen != 0 && !prev.is_null() {
        if std::slice::from_raw_parts(prev, klen)
            == std::slice::from_raw_parts(buf.add(8 + koff), klen)
        {
            return;
        }
        std::ptr::copy_nonoverlapping(buf.add(8 + koff), prev, klen);
    }
    std::ptr::copy_nonoverlapping(clock.to_le_bytes().as_ptr(), buf, 8);
    SAMPLE_LEFT.store(n - 1, Ordering::Relaxed);
    write_all(
        SAMPLE_FD.load(Ordering::Relaxed),
        std::slice::from_raw_parts(buf, 8 + len),
    );
}

// ----------------------------------------------------------------- watchdog
//
// STOP SIMULATING A CANDIDATE THE MOMENT IT IS CLEARLY DEAD.
//
// Cost is linear in simulated time, and a third to a half of the candidates a
// search generates crash, stop or leave the track long before the finish. The
// state needed to notice that is already in hand -- `do_sample` above proves
// the child can read the car's position and velocity every tick for a couple
// of milliseconds -- so the condition costs almost nothing to evaluate and the
// saving is the whole remaining tail.
//
// Three properties are non-negotiable:
//
//  1. A candidate that does NOT trip must produce a bit-identical answer to
//     one run with no watchdog. The evaluator therefore only ever READS the
//     simulation's memory, never allocates (the `Eval` is a `static`, the
//     reference line is memory the parent leaked before the first fork), and
//     never makes a syscall until it decides to abort.
//  2. Per-tick semantics must match `decode_rows` in the driver exactly: the
//     record is deduplicated on its whole content, and a tick is evaluated
//     when the RACE CLOCK moves on, using the last record carrying the old
//     clock. That is the same "last sample per clock value" rule the CSV path
//     uses, which is the rule that was validated to 3.4 mm.
//  3. The parent has to learn what happened even though the child exits
//     without printing anything. A one-page MAP_SHARED region carries the
//     summary out; the child updates it every tick, so the parent can read
//     progress even for candidates that ran to the end.

extern "C" {
    fn mmap(
        addr: *mut c_void,
        len: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        off: i64,
    ) -> *mut c_void;
}
const PROT_RW: c_int = 3;
const MAP_SHARED: c_int = 1;
const MAP_ANONYMOUS: c_int = 0x20;

const MAXREC: usize = 256;

/// Armed configuration, set once by the `A` command in the PARENT, inherited
/// by every child through the fork.
struct WatchCfg {
    np: usize,
    preds: [Pred; MAXP],
    rl: RefLine,
    clock0: i64,
    finish_s: f32,
    fast: u32,
    plane_x: f32,
    off_clock: usize,
    off_pos: usize,
    off_vel: usize,
    rec_len: usize,
    nseg: usize,
    seg: [(usize, usize); MAXP],
    /// MAP_SHARED page: the child's report, readable by the parent afterwards.
    out: *mut u8,
}

static mut WCFG: WatchCfg = WatchCfg {
    np: 0,
    preds: [Pred::ZERO; MAXP],
    rl: RefLine::NONE,
    clock0: 0,
    finish_s: 0.0,
    fast: 1,
    plane_x: 0.0,
    off_clock: 0,
    off_pos: 20,
    off_vel: 32,
    rec_len: 44,
    nseg: 0,
    seg: [(0, 0); MAXP],
    out: core::ptr::null_mut(),
};

static mut EVAL: Eval = Eval::ZERO;
static mut WREC: [u8; MAXREC] = [0; MAXREC];
static mut WPREV: [u8; MAXREC] = [0; MAXREC];
static WPREV_VALID: AtomicUsize = AtomicUsize::new(0);
static WLAST_CLOCK: AtomicU64 = AtomicU64::new(u64::MAX);
static WATCH_ON: AtomicUsize = AtomicUsize::new(0);

#[inline]
unsafe fn rec_u32(b: *const u8, o: usize) -> u32 {
    let mut v = [0u8; 4];
    std::ptr::copy_nonoverlapping(b.add(o), v.as_mut_ptr(), 4);
    u32::from_le_bytes(v)
}

#[inline]
unsafe fn rec_f32(b: *const u8, o: usize) -> f32 {
    f32::from_bits(rec_u32(b, o))
}

/// Evaluate one finished tick and, if a predicate trips, end the child.
///
/// `clock` labels the tick being judged; `rec` is the gathered record.
#[inline(never)]
unsafe fn watch_eval(rec: *const u8, clock: i64) {
    let cfg = &*core::ptr::addr_of!(WCFG);
    let ev = &mut *core::ptr::addr_of_mut!(EVAL);
    let tick = ((clock - cfg.clock0) / 10) as i32;
    let pos = [
        rec_f32(rec, cfg.off_pos),
        rec_f32(rec, cfg.off_pos + 4),
        rec_f32(rec, cfg.off_pos + 8),
    ];
    let vel = [
        rec_f32(rec, cfg.off_vel),
        rec_f32(rec, cfg.off_vel + 4),
        rec_f32(rec, cfg.off_vel + 8),
    ];
    let trip = ev.feed(tick, pos, vel);
    if !cfg.out.is_null() {
        let mut buf = [0u8; SUMMARY_BYTES];
        ev.sum.encode(&mut buf);
        std::ptr::copy_nonoverlapping(buf.as_ptr(), cfg.out, SUMMARY_BYTES);
    }
    if trip >= 0 {
        // dead candidate: stop paying for it. The parent sees EOF on the JSON
        // pipe and reads the verdict out of the shared page.
        _exit(0)
    }
}

#[inline(never)]
unsafe fn watch_gather(rec: *mut u8) {
    let cfg = &*core::ptr::addr_of!(WCFG);
    let mut o = 0usize;
    for s in 0..cfg.nseg {
        let (a, l) = cfg.seg[s];
        std::ptr::copy_nonoverlapping(a as *const u8, rec.add(o), l);
        o += l;
    }
}

/// The child's per-`lroundf` hook when the watchdog is armed.
///
/// Two modes, and they were measured against each other over hundreds of
/// candidates before the cheap one became the default:
///
/// * **full** -- gather the whole record on every call (255 per tick), dedup on
///   its content, and judge the last record carrying a given clock value. This
///   is exactly the rule `decode_rows` uses on the driver side, which is the
///   rule that was validated to 3.4 mm against ghost telemetry.
/// * **fast** -- read only the 4-byte race clock on every call, and gather the
///   state only when it moves. Measured, not assumed: the engine writes the
///   car's state for tick T *before* it advances the clock to T, so the first
///   call carrying clock T already sees tick T's finished state -- the same
///   bytes the full path judges at the END of clock T's span. (Judging one
///   sample earlier is exactly what makes it worth doing: an abort lands a
///   tick sooner.) It costs a load and a compare instead of a 44-byte gather
///   250 times a tick, which is 10 ms a candidate. `fk pred --mode equiv`
///   checks the two paths field by field.
#[inline(never)]
unsafe fn do_watch(clock: u64) {
    SAMPLE_NEXT.store(clock + SAMPLE_STRIDE.load(Ordering::Relaxed), Ordering::Relaxed);
    let cfg = &*core::ptr::addr_of!(WCFG);
    let rec = core::ptr::addr_of_mut!(WREC) as *mut u8;
    let prev = core::ptr::addr_of_mut!(WPREV) as *mut u8;
    if cfg.fast != 0 {
        let c_now = *((cfg.seg[0].0) as *const u32) as i64;
        let last = WLAST_CLOCK.load(Ordering::Relaxed) as i64;
        if c_now == last {
            return;
        }
        watch_gather(rec);
        watch_eval(rec, c_now);
        WLAST_CLOCK.store(c_now as u64, Ordering::Relaxed);
        WPREV_VALID.store(1, Ordering::Relaxed);
        return;
    }
    watch_gather(rec);
    let n = cfg.rec_len;
    if WPREV_VALID.load(Ordering::Relaxed) != 0 {
        if std::slice::from_raw_parts(rec, n) == std::slice::from_raw_parts(prev, n) {
            return;
        }
        let cprev = rec_u32(prev, cfg.off_clock) as i64;
        if rec_u32(rec, cfg.off_clock) as i64 != cprev {
            watch_eval(prev, cprev);
        }
    }
    std::ptr::copy_nonoverlapping(rec, prev, n);
    WPREV_VALID.store(1, Ordering::Relaxed);
}

type WriteFn = unsafe extern "C" fn(c_int, *const c_void, usize) -> isize;type ReadFn = unsafe extern "C" fn(c_int, *mut c_void, usize) -> isize;

unsafe fn real_write() -> WriteFn {
    let mut p = REAL_WRITE.load(Ordering::Relaxed);
    if p == 0 {
        p = dlsym(RTLD_NEXT, b"write\0".as_ptr() as *const c_char) as usize;
        REAL_WRITE.store(p, Ordering::Relaxed);
    }
    std::mem::transmute(p)
}

unsafe fn real_read() -> ReadFn {
    static P: AtomicUsize = AtomicUsize::new(0);
    let mut p = P.load(Ordering::Relaxed);
    if p == 0 {
        p = dlsym(RTLD_NEXT, b"read\0".as_ptr() as *const c_char) as usize;
        P.store(p, Ordering::Relaxed);
    }
    std::mem::transmute(p)
}

fn log(s: &[u8]) {
    unsafe {
        real_write()(2, s.as_ptr() as *const c_void, s.len());
    }
}

fn logn(prefix: &[u8], v: u64) {
    let mut o = Vec::with_capacity(64);
    o.extend_from_slice(prefix);
    utoa(v, &mut o);
    o.push(b'\n');
    log(&o);
}

pub fn utoa(mut v: u64, out: &mut Vec<u8>) {
    let mut d = [0u8; 24];
    let mut k = 0;
    if v == 0 {
        d[0] = b'0';
        k = 1;
    }
    while v > 0 {
        d[k] = b'0' + (v % 10) as u8;
        v /= 10;
        k += 1;
    }
    while k > 0 {
        k -= 1;
        out.push(d[k]);
    }
}

unsafe fn env_i64(name: &[u8]) -> Option<i64> {
    let e = getenv(name.as_ptr() as *const c_char);
    if e.is_null() {
        None
    } else {
        Some(atoll(e))
    }
}

unsafe fn env_str(name: &[u8]) -> Option<String> {
    let e = getenv(name.as_ptr() as *const c_char);
    if e.is_null() {
        return None;
    }
    let mut n = 0;
    while *e.add(n) != 0 {
        n += 1;
    }
    Some(String::from_utf8_lossy(std::slice::from_raw_parts(e as *const u8, n)).into_owned())
}

unsafe fn init() {
    if INIT.swap(1, Ordering::SeqCst) != 0 {
        return;
    }
    if let Some(v) = env_i64(b"FKSHIM_STOP_LROUNDF\0") {
        if v > 0 {
            STOP_AT.store(v as u64, Ordering::SeqCst);
        }
    }
    if let Some(v) = env_i64(b"FKSHIM_CKPT\0") {
        if v > 0 {
            CKPT_AT.store(v as u64, Ordering::SeqCst);
        }
    }
    if let Some(v) = env_i64(b"FKSHIM_CMD_FD\0") {
        CMD_FD.store(v as i32, Ordering::SeqCst);
    }
    if let Some(v) = env_i64(b"FKSHIM_RES_FD\0") {
        RES_FD.store(v as i32, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------- input array

/// Horspool substring search (see fk/src/procmem.rs for the rationale).
struct Horspool {
    needle: [u8; 4],
    shift: [usize; 256],
}
impl Horspool {
    fn new(needle: [u8; 4]) -> Horspool {
        let mut shift = [4usize; 256];
        for i in 0..3 {
            shift[needle[i] as usize] = 3 - i;
        }
        Horspool { needle, shift }
    }
    fn find_from(&self, hay: &[u8], from: usize) -> Option<usize> {
        let n = 4;
        if hay.len() < n {
            return None;
        }
        let mut i = from;
        while i + n <= hay.len() {
            let c = hay[i + 3];
            if c == self.needle[3] && hay[i..i + 4] == self.needle {
                return Some(i);
            }
            i += self.shift[c as usize];
        }
        None
    }
}

const STRIDE: usize = 32;

/// The reference ghost's steer axis, one f32 per tick, plus the offset of the
/// most distinctive window. Written by the driver, read here.
struct Key {
    t0: usize,
    m: usize,
    steer: Vec<f32>,
}

unsafe fn read_key(path: &str) -> Option<Key> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 12 {
        return None;
    }
    let n = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let t0 = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let m = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    if data.len() < 12 + 4 * n {
        return None;
    }
    let steer = (0..n)
        .map(|i| f32::from_le_bytes(data[12 + 4 * i..16 + 4 * i].try_into().unwrap()))
        .collect();
    Some(Key { t0, m, steer })
}

/// Scan our own address space for the decoded input array.
unsafe fn locate(key: &Key) -> Option<usize> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    let want: Vec<[u8; 4]> = key.steer.iter().map(|v| v.to_le_bytes()).collect();
    let hs = Horspool::new(want[key.t0]);
    let n = key.steer.len();
    for line in maps.lines() {
        let mut it = line.split_whitespace();
        let range = it.next()?;
        let perms = it.next().unwrap_or("");
        if !perms.starts_with("rw") {
            continue;
        }
        let path = line.split_whitespace().nth(5).unwrap_or("");
        if path.starts_with("/dev") || path == "[vvar]" || path == "[vsyscall]" || path == "[stack]"
        {
            continue;
        }
        let (a, b) = range.split_once('-')?;
        let start = usize::from_str_radix(a, 16).ok()?;
        let end = usize::from_str_radix(b, 16).ok()?;
        let hay = std::slice::from_raw_parts(start as *const u8, end - start);
        let mut i = 0usize;
        while let Some(p) = hs.find_from(hay, i) {
            i = p + 1;
            let last = key.t0 + key.m - 1;
            if p + (key.m - 1) * STRIDE + 4 > hay.len() {
                continue;
            }
            if hay[p + (key.m - 1) * STRIDE..p + (key.m - 1) * STRIDE + 4] != want[last] {
                continue;
            }
            if p < key.t0 * STRIDE {
                continue;
            }
            let base = p - key.t0 * STRIDE;
            if base + n * STRIDE > hay.len() {
                continue;
            }
            let mut ok = true;
            for t in 0..n {
                if hay[base + t * STRIDE..base + t * STRIDE + 4] != want[t] {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(start + base);
            }
        }
    }
    None
}

// ------------------------------------------------------- boundary tick probe
//
// The one thing that can make a resumed run silently wrong is rewriting a tick
// the simulation has ALREADY read. Guessing that boundary from the clock is not
// good enough -- at a 98.9% checkpoint a four-tick probe mis-read it and two of
// thirty candidates came back 2-3 ms off. So ask the engine instead: fork a
// throwaway child, take away read access to the input array, and see which
// record it faults on next. That address IS the tick the simulation is about to
// consume, so every tick from there on is safe to rewrite.

extern "C" {
    fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
    fn sigaction(sig: c_int, act: *const SigactionT, old: *mut SigactionT) -> c_int;
    fn getpagesize() -> c_int;
}

#[repr(C)]
struct SigactionT {
    handler: usize,
    mask: [u64; 16],
    flags: c_int,
    restorer: usize,
}

const SA_SIGINFO: c_int = 4;
const SIGSEGV: c_int = 11;
const PROT_NONE: c_int = 0;
const PROT_READ_WRITE: c_int = 3;

static PROBE_FD: AtomicI32 = AtomicI32::new(-1);
static PROBE_BASE: AtomicUsize = AtomicUsize::new(0);
static PROBE_END: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn segv_handler(_sig: c_int, info: *const u8, _ctx: *mut c_void) {
    let addr = *(info.add(16) as *const usize);
    let base = PROBE_BASE.load(Ordering::Relaxed);
    let end = PROBE_END.load(Ordering::Relaxed);
    let fd = PROBE_FD.load(Ordering::Relaxed);
    if addr < base || addr >= end {
        // a neighbour sharing one of the two edge pages: give that page back
        // and let the instruction retry, so the probe keeps waiting for a real
        // input-array read.
        let ps = getpagesize() as usize;
        mprotect((addr / ps * ps) as *mut c_void, ps, PROT_READ_WRITE);
        return;
    }
    let mut o = Vec::with_capacity(64);
    o.extend_from_slice(b"TICK ");
    utoa(((addr - base) / STRIDE) as u64, &mut o);
    o.push(b'\n');
    write_all(fd, &o);
    _exit(0)
}

/// In a forked child: revoke read access to every page the input array touches
/// (including the two partial edge pages -- the tail of the array lives in the
/// last one, and a checkpoint late in the run reads only there) and let the
/// simulation walk into it.
unsafe fn arm_probe(base: usize, n: usize, fd: c_int) {
    let ps = getpagesize() as usize;
    let lo = base / ps * ps;
    let hi = (base + n * STRIDE + ps - 1) / ps * ps;
    PROBE_FD.store(fd, Ordering::SeqCst);
    PROBE_BASE.store(base, Ordering::SeqCst);
    PROBE_END.store(base + n * STRIDE, Ordering::SeqCst);
    let act = SigactionT {
        handler: segv_handler as usize,
        mask: [0; 16],
        flags: SA_SIGINFO,
        restorer: 0,
    };
    sigaction(SIGSEGV, &act, std::ptr::null_mut());
    if hi > lo {
        mprotect(lo as *mut c_void, hi - lo, PROT_NONE);
    }
}

// ------------------------------------------------------------------- protocol

unsafe fn read_exact(fd: c_int, buf: &mut [u8]) -> bool {
    let mut got = 0usize;
    while got < buf.len() {
        let r = real_read()(fd, buf.as_mut_ptr().add(got) as *mut c_void, buf.len() - got);
        if r <= 0 {
            return false;
        }
        got += r as usize;
    }
    true
}

unsafe fn write_all(fd: c_int, buf: &[u8]) -> bool {
    let mut sent = 0usize;
    while sent < buf.len() {
        let r = real_write()(fd, buf.as_ptr().add(sent) as *const c_void, buf.len() - sent);
        if r <= 0 {
            return false;
        }
        sent += r as usize;
    }
    true
}

unsafe fn send_frame(fd: c_int, payload: &[u8]) {
    let mut hdr = (payload.len() as u32).to_le_bytes().to_vec();
    hdr.extend_from_slice(payload);
    write_all(fd, &hdr);
}

/// The fork server. Entered once, in the parent, at the checkpoint; never
/// returns there. Returns in the *child*, so the simulation resumes with the
/// patched tail.
unsafe fn forkserver() {
    let cmd = CMD_FD.load(Ordering::SeqCst);
    let res = RES_FD.load(Ordering::SeqCst);
    let keypath = match env_str(b"FKSHIM_KEY\0") {
        Some(p) => p,
        None => {
            log(b"FKSHIM: no FKSHIM_KEY\n");
            _exit(97)
        }
    };
    let key = match read_key(&keypath) {
        Some(k) => k,
        None => {
            log(b"FKSHIM: bad key file\n");
            _exit(97)
        }
    };
    let base = match locate(&key) {
        Some(b) => b,
        None => {
            log(b"FKSHIM: input array NOT FOUND\n");
            send_frame(res, b"ERR notfound");
            _exit(96)
        }
    };
    let mut hello = Vec::new();
    signal(SIGCHLD, SIG_IGN);
    hello.extend_from_slice(b"READY ");
    utoa(base as u64, &mut hello);
    hello.push(b' ');
    utoa(N_LROUNDF.load(Ordering::Relaxed), &mut hello);
    send_frame(res, &hello);

    loop {
        let mut lenb = [0u8; 4];
        if !read_exact(cmd, &mut lenb) {
            _exit(0);
        }
        let len = u32::from_le_bytes(lenb) as usize;
        let mut payload = vec![0u8; len];
        if len > 0 && !read_exact(cmd, &mut payload) {
            _exit(0);
        }
        if len == 0 || payload[0] == b'Q' {
            _exit(0);
        }
        if payload[0] == b'N' {
            // null fork: how much of the per-candidate cost is fork + child
            // startup alone, with no simulation at all?
            let t_start = now_us();
            let mut fds = [0i32; 2];
            if pipe(fds.as_mut_ptr()) != 0 {
                send_frame(res, b"ERR pipe");
                continue;
            }
            let pid = fork();
            if pid < 0 {
                close(fds[0]);
                close(fds[1]);
                send_frame(res, b"ERR fork");
                continue;
            }
            if pid == 0 {
                close(fds[0]);
                write_all(fds[1], b"x");
                _exit(0);
            }
            let t_forked = now_us();
            close(fds[1]);
            let mut b = [0u8; 8];
            real_read()(fds[0], b.as_mut_ptr() as *mut c_void, 8);
            let t_got = now_us();
            kill(pid, SIGKILL);
            close(fds[0]);
            let mut o = Vec::new();
            o.extend_from_slice(b"NULLFORK fork_us ");
            utoa(t_forked - t_start, &mut o);
            o.extend_from_slice(b" roundtrip_us ");
            utoa(t_got - t_start, &mut o);
            o.push(b'\n');
            send_frame(res, &o);
            continue;
        }
        if payload[0] == b'P' {
            // boundary probe
            fflush(std::ptr::null_mut());
            let mut fds = [0i32; 2];
            if pipe(fds.as_mut_ptr()) != 0 {
                send_frame(res, b"ERR pipe");
                continue;
            }
            let pid = fork();
            // A FAILED FORK MUST NOT BECOME kill(-1). `fork()` returns -1 under
            // EAGAIN, and the handlers below all end in `kill(pid, SIGKILL)`:
            // with pid = -1 that is "signal every process this user can reach",
            // which on a box running five searches means killing all of them.
            if pid < 0 {
                close(fds[0]);
                close(fds[1]);
                send_frame(res, b"ERR fork");
                continue;
            }
            if pid == 0 {
                IS_CHILD.store(1, Ordering::SeqCst);
                close(fds[0]);
                dup2(fds[1], 1);
                arm_probe(base, key.steer.len(), fds[1]);
                return;
            }
            close(fds[1]);
            let mut out: Vec<u8> = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let mut pfd = PollFd {
                    fd: fds[0],
                    events: POLLIN,
                    revents: 0,
                };
                if poll(&mut pfd, 1, 20000) <= 0 {
                    out.extend_from_slice(b"PROBE-TIMEOUT");
                    break;
                }
                let r = real_read()(fds[0], buf.as_mut_ptr() as *mut c_void, buf.len());
                if r <= 0 {
                    break;
                }
                out.extend_from_slice(&buf[..r as usize]);
                if out.windows(5).any(|w| w == b"TICK ") {
                    break;
                }
            }
            kill(pid, SIGKILL);
            let mut st = 0i32;
            waitpid(pid, &mut st, 0);
            close(fds[0]);
            send_frame(res, &out);
            continue;
        }
        if payload[0] == b'S' {
            // Sample-and-run: like 'R', but the child also gathers segments of
            // memory out on a second pipe as it simulates. Two frames come
            // back: the validator's JSON, then the raw sample blob.
            //   u32 n_patch | u32 n_seg | u64 stride | u32 max
            //   | u32 key_len | u32 key_off
            //   | n_seg * (u64 addr, u32 len)
            //   | n_patch * (u32 tick, f32 steer, f32 gas, f32 brake)
            let np = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
            let nseg = u32::from_le_bytes(payload[5..9].try_into().unwrap()) as usize;
            let sstride = u64::from_le_bytes(payload[9..17].try_into().unwrap());
            let smax_raw = u32::from_le_bytes(payload[17..21].try_into().unwrap());
            let sexit = (smax_raw & 0x8000_0000) != 0;
            let smax = (smax_raw & 0x7fff_ffff) as u64;
            let sdedup = u32::from_le_bytes(payload[21..25].try_into().unwrap()) as usize;
            let skeyoff = u32::from_le_bytes(payload[25..29].try_into().unwrap()) as usize;
            let mut segs = [(0usize, 0usize); MAX_SEG];
            let mut slen = 0usize;
            for s in 0..nseg.min(MAX_SEG) {
                let o = 29 + s * 12;
                segs[s] = (
                    u64::from_le_bytes(payload[o..o + 8].try_into().unwrap()) as usize,
                    u32::from_le_bytes(payload[o + 8..o + 12].try_into().unwrap()) as usize,
                );
                slen += segs[s].1;
            }
            let poff = 29 + nseg * 12;
            // OPTIONAL trailing u32 (older drivers do not send it): how many
            // more `lroundf` calls this child may simulate before it exits.
            // ~255 calls to the tick, so 6 ticks is ~1530.
            let sbudget = {
                let o = poff + np * 16;
                if payload.len() >= o + 4 {
                    u32::from_le_bytes(payload[o..o + 4].try_into().unwrap()) as u64
                } else {
                    0
                }
            };

            // OPTIONAL trailing (u64 gate_addr, u32 gate_mod, u32 gate_phase)
            let (gaddr, gmod, gphase) = {
                let o = poff + np * 16 + 4;
                if payload.len() >= o + 16 {
                    (
                        u64::from_le_bytes(payload[o..o + 8].try_into().unwrap()) as usize,
                        u32::from_le_bytes(payload[o + 8..o + 12].try_into().unwrap()) as u64,
                        u32::from_le_bytes(payload[o + 12..o + 16].try_into().unwrap()) as u64,
                    )
                } else {
                    (0, 0, 0)
                }
            };
            fflush(std::ptr::null_mut());
            let t_start = now_us();
            let mut fds = [0i32; 2];
            let mut sfds = [0i32; 2];
            if pipe(fds.as_mut_ptr()) != 0 || pipe(sfds.as_mut_ptr()) != 0 {
                // TWO frames, because that is what the driver reads for 'S'.
                // Answering with one desynchronises the stream permanently:
                // every later response is read as the answer to the previous
                // command, and the server eventually executes a command byte
                // taken from the middle of a patch payload.
                send_frame(res, b"ERR pipe");
                send_frame(res, b"");
                continue;
            }
            let pid = fork();
            if pid < 0 {
                close(fds[0]);
                close(fds[1]);
                close(sfds[0]);
                close(sfds[1]);
                send_frame(res, b"ERR fork");
                send_frame(res, b"");
                continue;
            }
            if pid == 0 {
                IS_CHILD.store(1, Ordering::SeqCst);
                close(fds[0]);
                close(sfds[0]);
                dup2(fds[1], 1);
                close(fds[1]);
                setvbuf(stdout, std::ptr::null_mut(), IONBF, 0);
                for i in 0..np {
                    let o = poff + i * 16;
                    let tick = u32::from_le_bytes(payload[o..o + 4].try_into().unwrap()) as usize;
                    let rec = (base + tick * STRIDE) as *mut u8;
                    std::ptr::copy_nonoverlapping(payload.as_ptr().add(o + 4), rec, 12);
                }
                // buffers are allocated BEFORE the hook can fire, so the hot
                // path never allocates
                let mut b = vec![0u8; 8 + slen];
                let mut p = vec![0xFFu8; slen.max(1)];
                SAMPLE_BUF.store(b.as_mut_ptr() as usize, Ordering::SeqCst);
                SAMPLE_PREV.store(p.as_mut_ptr() as usize, Ordering::SeqCst);
                std::mem::forget(b);
                std::mem::forget(p);
                SAMPLE_ADDR.store(segs[0].0, Ordering::SeqCst);
                SAMPLE_LEN.store(slen, Ordering::SeqCst);
                SEG_N.store(nseg.min(MAX_SEG), Ordering::SeqCst);
                for s in 0..nseg.min(MAX_SEG) {
                    SEG_ADDR[s].store(segs[s].0, Ordering::SeqCst);
                    SEG_LEN[s].store(segs[s].1, Ordering::SeqCst);
                }
                SAMPLE_FD.store(sfds[1], Ordering::SeqCst);
                SAMPLE_LEFT.store(smax, Ordering::SeqCst);
                SAMPLE_EXIT.store(if sexit { 1 } else { 0 }, Ordering::SeqCst);
                SAMPLE_DEADLINE.store(
                    if sbudget == 0 {
                        0
                    } else {
                        N_LROUNDF.load(Ordering::Relaxed) + sbudget
                    },
                    Ordering::SeqCst,
                );
                SAMPLE_DEDUP.store(sdedup, Ordering::SeqCst);
                SAMPLE_KEYOFF.store(skeyoff, Ordering::SeqCst);
                GATE_ADDR.store(gaddr, Ordering::SeqCst);
                GATE_MOD.store(gmod, Ordering::SeqCst);
                GATE_PHASE.store(gphase, Ordering::SeqCst);
                SAMPLE_STRIDE.store(sstride.max(1), Ordering::SeqCst);
                SAMPLE_NEXT.store(0, Ordering::SeqCst);
                return;
            }
            let t_forked = now_us();
            close(fds[1]);
            close(sfds[1]);
            let mut out: Vec<u8> = Vec::with_capacity(4096);
            let mut samples: Vec<u8> = Vec::with_capacity(1 << 20);
            let mut buf = [0u8; 65536];
            let mut json_done = false;
            let mut samples_eof = false;
            let mut t_first = 0u64;
            loop {
                let mut pfds = [
                    PollFd { fd: if json_done { -1 } else { fds[0] }, events: POLLIN, revents: 0 },
                    PollFd { fd: if samples_eof { -1 } else { sfds[0] }, events: POLLIN, revents: 0 },
                ];
                if json_done && samples_eof {
                    break;
                }
                let pr = poll(pfds.as_mut_ptr(), 2, 60000);
                if pr <= 0 {
                    out.extend_from_slice(b"\nFKSHIM-TIMEOUT\n");
                    break;
                }
                if !samples_eof && pfds[1].revents != 0 {
                    let r = real_read()(sfds[0], buf.as_mut_ptr() as *mut c_void, buf.len());
                    if r <= 0 {
                        samples_eof = true;
                    } else {
                        samples.extend_from_slice(&buf[..r as usize]);
                    }
                }
                if !json_done && pfds[0].revents != 0 {
                    let r = real_read()(fds[0], buf.as_mut_ptr() as *mut c_void, buf.len());
                    if r <= 0 {
                        json_done = true;
                    } else {
                        if t_first == 0 {
                            t_first = now_us();
                        }
                        out.extend_from_slice(&buf[..r as usize]);
                        if out.windows(9).any(|w| w == b"\"IsValid\"") {
                            json_done = true;
                            // the child is finished simulating, so every sample
                            // it will ever write is already in the pipe; kill it
                            // and drain what is buffered.
                            kill(pid, SIGKILL);
                        }
                    }
                }
            }
            if !samples_eof {
                // drain whatever the pipe still holds
                loop {
                    let mut pfd = PollFd { fd: sfds[0], events: POLLIN, revents: 0 };
                    if poll(&mut pfd, 1, 200) <= 0 {
                        break;
                    }
                    let r = real_read()(sfds[0], buf.as_mut_ptr() as *mut c_void, buf.len());
                    if r <= 0 {
                        break;
                    }
                    samples.extend_from_slice(&buf[..r as usize]);
                }
            }
            kill(pid, SIGKILL);
            close(fds[0]);
            close(sfds[0]);
            out.extend_from_slice(b"\nFKTIME fork_us ");
            utoa(t_forked - t_start, &mut out);
            out.extend_from_slice(b" first_us ");
            utoa(t_first.saturating_sub(t_forked), &mut out);
            out.extend_from_slice(b" done_us ");
            utoa(now_us() - t_start, &mut out);
            out.push(b'\n');
            send_frame(res, &out);
            send_frame(res, &samples);
            continue;
        }
        if payload[0] == b'G' {
            // GO: sample the rest of THIS process's run and return to the
            // simulation. No fork, no resume, no input patching.
            //
            // WHY THIS EXISTS: a forked child is NOT the clean run. Measured on
            // this engine (banked as `hl_RESULT_v13`, and reproduced here on
            // 208024): the same tape resumed from two different checkpoints
            // agrees on 0 of 522 ticks and diverges by up to 2.9 m, and against
            // a human ghost's own recorded path a resume at race -30 ms is
            // 5.578 m out where a resume at race 140 ms is 0.0055 m. Telemetry
            // regenerated from a resumed child would therefore describe a
            // slightly different run from the one the oracle validates. The
            // parent, on the other hand, is an ordinary /validatepath run: it
            // has forked children (which cannot touch its memory) and is
            // otherwise untouched.
            //
            //   u32 n_seg | u64 stride | u32 max | u32 key_len | u32 key_off
            //   | u64 gate_addr | u32 gate_mod | u32 gate_phase
            //   | u32 path_len | path | n_seg * (u64 addr, u32 len)
            let nseg = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
            let sstride = u64::from_le_bytes(payload[5..13].try_into().unwrap());
            let smax = u32::from_le_bytes(payload[13..17].try_into().unwrap()) as u64;
            let sdedup = u32::from_le_bytes(payload[17..21].try_into().unwrap()) as usize;
            let skeyoff = u32::from_le_bytes(payload[21..25].try_into().unwrap()) as usize;
            let gaddr = u64::from_le_bytes(payload[25..33].try_into().unwrap()) as usize;
            let gmod = u32::from_le_bytes(payload[33..37].try_into().unwrap()) as u64;
            let gphase = u32::from_le_bytes(payload[37..41].try_into().unwrap()) as u64;
            let plen = u32::from_le_bytes(payload[41..45].try_into().unwrap()) as usize;
            let mut path: Vec<u8> = payload[45..45 + plen].to_vec();
            path.push(0);
            let mut segs = [(0usize, 0usize); MAX_SEG];
            let mut slen = 0usize;
            for s in 0..nseg.min(MAX_SEG) {
                let o = 45 + plen + s * 12;
                segs[s] = (
                    u64::from_le_bytes(payload[o..o + 8].try_into().unwrap()) as usize,
                    u32::from_le_bytes(payload[o + 8..o + 12].try_into().unwrap()) as usize,
                );
                slen += segs[s].1;
            }
            let fd = open(path.as_ptr() as *const c_char, 577, 0o644);
            if fd < 0 {
                send_frame(res, b"ERR open");
                continue;
            }
            let mut b = vec![0u8; 8 + slen];
            let mut pv = vec![0xFFu8; slen.max(1)];
            SAMPLE_BUF.store(b.as_mut_ptr() as usize, Ordering::SeqCst);
            SAMPLE_PREV.store(pv.as_mut_ptr() as usize, Ordering::SeqCst);
            std::mem::forget(b);
            std::mem::forget(pv);
            SAMPLE_ADDR.store(segs[0].0, Ordering::SeqCst);
            SAMPLE_LEN.store(slen, Ordering::SeqCst);
            SEG_N.store(nseg.min(MAX_SEG), Ordering::SeqCst);
            for s in 0..nseg.min(MAX_SEG) {
                SEG_ADDR[s].store(segs[s].0, Ordering::SeqCst);
                SEG_LEN[s].store(segs[s].1, Ordering::SeqCst);
            }
            SAMPLE_FD.store(fd, Ordering::SeqCst);
            SAMPLE_LEFT.store(smax as u64, Ordering::SeqCst);
            SAMPLE_EXIT.store(0, Ordering::SeqCst);
            SAMPLE_DEADLINE.store(0, Ordering::SeqCst);
            SAMPLE_DEDUP.store(sdedup, Ordering::SeqCst);
            SAMPLE_KEYOFF.store(skeyoff, Ordering::SeqCst);
            GATE_ADDR.store(gaddr, Ordering::SeqCst);
            GATE_MOD.store(gmod, Ordering::SeqCst);
            GATE_PHASE.store(gphase, Ordering::SeqCst);
            SAMPLE_STRIDE.store(sstride.max(1), Ordering::SeqCst);
            SAMPLE_NEXT.store(0, Ordering::SeqCst);
            send_frame(res, b"GO");
            return;
        }
        if payload[0] == b'A' {
            // Arm the watchdog. Parsed and stored in the PARENT, once, so every
            // later fork inherits it for free -- in particular the reference
            // line, which is 30 KB and must not be re-sent per candidate.
            //
            //   'A' | u32 np | np * Pred
            //       | i64 clock0 | u32 off_clock off_pos off_vel rec_len
            //       | u32 nseg | nseg * (u64 addr, u32 len)
            //       | f32 corridor | i32 ahead | i32 back
            //       | u32 nref | nref * 3 f32 xyz | nref * f32 arclength
            let cfg = &mut *core::ptr::addr_of_mut!(WCFG);
            let mut o = 1usize;
            let g4 = |o: usize| u32::from_le_bytes(payload[o..o + 4].try_into().unwrap());
            let np = (g4(o) as usize).min(MAXP);
            o += 4;
            for i in 0..np {
                cfg.preds[i] = Pred::decode(&payload[o..o + PRED_BYTES]);
                o += PRED_BYTES;
            }
            cfg.np = np;
            cfg.clock0 = i64::from_le_bytes(payload[o..o + 8].try_into().unwrap());
            o += 8;
            cfg.off_clock = g4(o) as usize;
            cfg.off_pos = g4(o + 4) as usize;
            cfg.off_vel = g4(o + 8) as usize;
            cfg.rec_len = (g4(o + 12) as usize).min(MAXREC);
            o += 16;
            let nseg = (g4(o) as usize).min(MAXP);
            o += 4;
            cfg.nseg = nseg;
            for s in 0..nseg {
                cfg.seg[s] = (
                    u64::from_le_bytes(payload[o..o + 8].try_into().unwrap()) as usize,
                    g4(o + 8) as usize,
                );
                o += 12;
            }
            let corridor = f32::from_bits(g4(o));
            let ahead = g4(o + 4) as i32;
            let back = g4(o + 8) as i32;
            cfg.finish_s = f32::from_bits(g4(o + 12));
            cfg.fast = g4(o + 16);
            o += 20;
            let nref = g4(o) as usize;
            o += 4;
            if nref > 0 {
                let mut xyz: Vec<f32> = Vec::with_capacity(3 * nref);
                for i in 0..3 * nref {
                    xyz.push(f32::from_bits(g4(o + 4 * i)));
                }
                o += 12 * nref;
                let mut s: Vec<f32> = Vec::with_capacity(nref);
                for i in 0..nref {
                    s.push(f32::from_bits(g4(o + 4 * i)));
                }
                o += 4 * nref;
                let _ = o;
                cfg.rl = RefLine {
                    n: nref,
                    xyz: xyz.as_ptr(),
                    s: s.as_ptr(),
                    corridor,
                    ahead,
                    back,
                };
                // leaked on purpose: every child must see it at the same
                // address, and it is written once per server
                std::mem::forget(xyz);
                std::mem::forget(s);
            } else {
                cfg.rl = RefLine::NONE;
            }
            // trailing sub-tick timing plane, if this driver sent one
            cfg.plane_x = if payload.len() >= o + 4 {
                f32::from_bits(g4(o))
            } else {
                0.0
            };
            if cfg.out.is_null() {
                let p = mmap(
                    std::ptr::null_mut(),
                    4096,
                    PROT_RW,
                    MAP_SHARED | MAP_ANONYMOUS,
                    -1,
                    0,
                );
                if p as isize == -1 {
                    send_frame(res, b"ERR mmap");
                    continue;
                }
                cfg.out = p as *mut u8;
            }
            let mut ack = Vec::new();
            ack.extend_from_slice(b"ARMED ");
            utoa(np as u64, &mut ack);
            ack.extend_from_slice(b" preds, ");
            utoa(nref as u64, &mut ack);
            ack.extend_from_slice(b" reference points");
            send_frame(res, &ack);
            continue;
        }
        if payload[0] == b'W' {
            // Run one candidate with the watchdog armed. Same wire shape as
            // 'R'; two frames come back: the validator's JSON (empty if the
            // child was aborted) and the fixed-size summary.
            let n = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
            let cfg = &mut *core::ptr::addr_of_mut!(WCFG);
            if cfg.out.is_null() || cfg.nseg == 0 {
                send_frame(res, b"ERR not armed");
                send_frame(res, b"");
                continue;
            }
            // clear the shared report before the fork, so a child that dies
            // without writing cannot be mistaken for one that reported
            std::ptr::write_bytes(cfg.out, 0, SUMMARY_BYTES);
            fflush(std::ptr::null_mut());
            let t_start = now_us();
            let mut fds = [0i32; 2];
            if pipe(fds.as_mut_ptr()) != 0 {
                send_frame(res, b"ERR pipe");
                send_frame(res, b"");
                continue;
            }
            let pid = fork();
            if pid < 0 {
                close(fds[0]);
                close(fds[1]);
                send_frame(res, b"ERR fork");
                send_frame(res, b"");
                continue;
            }
            if pid == 0 {
                IS_CHILD.store(1, Ordering::SeqCst);
                close(fds[0]);
                dup2(fds[1], 1);
                close(fds[1]);
                setvbuf(stdout, std::ptr::null_mut(), IONBF, 0);
                for i in 0..n {
                    let o = 5 + i * 16;
                    let tick = u32::from_le_bytes(payload[o..o + 4].try_into().unwrap()) as usize;
                    let rec = (base + tick * STRIDE) as *mut u8;
                    std::ptr::copy_nonoverlapping(payload.as_ptr().add(o + 4), rec, 12);
                }
                let ev = &mut *core::ptr::addr_of_mut!(EVAL);
                ev.reset();
                ev.np = cfg.np;
                ev.preds = cfg.preds;
                ev.rl = cfg.rl;
                ev.finish_s = cfg.finish_s;
                ev.plane_x = cfg.plane_x;
                WPREV_VALID.store(0, Ordering::SeqCst);
                WLAST_CLOCK.store(u64::MAX, Ordering::SeqCst);
                WATCH_ON.store(1, Ordering::SeqCst);
                SAMPLE_STRIDE.store(1, Ordering::SeqCst);
                SAMPLE_NEXT.store(0, Ordering::SeqCst);
                return; // resume the simulation, watched
            }
            let t_forked = now_us();
            let mut t_first = 0u64;
            close(fds[1]);
            let mut out: Vec<u8> = Vec::with_capacity(4096);
            let mut buf = [0u8; 4096];
            let mut done = false;
            while !done {
                let mut pfd = PollFd {
                    fd: fds[0],
                    events: POLLIN,
                    revents: 0,
                };
                let pr = poll(&mut pfd, 1, 20000);
                if pr <= 0 {
                    out.extend_from_slice(b"\nFKSHIM-TIMEOUT\n");
                    break;
                }
                let r = real_read()(fds[0], buf.as_mut_ptr() as *mut c_void, buf.len());
                if r <= 0 {
                    break; // the child aborted itself, or finished and closed
                }
                if t_first == 0 {
                    t_first = now_us();
                }
                out.extend_from_slice(&buf[..r as usize]);
                if out.windows(9).any(|w| w == b"\"IsValid\"") {
                    done = true;
                }
            }
            kill(pid, SIGKILL);
            close(fds[0]);
            out.extend_from_slice(b"\nFKTIME fork_us ");
            utoa(t_forked - t_start, &mut out);
            out.extend_from_slice(b" first_us ");
            utoa(t_first.saturating_sub(t_forked), &mut out);
            out.extend_from_slice(b" done_us ");
            utoa(now_us() - t_start, &mut out);
            out.push(b'\n');
            let mut sum = [0u8; SUMMARY_BYTES];
            std::ptr::copy_nonoverlapping(cfg.out, sum.as_mut_ptr(), SUMMARY_BYTES);
            send_frame(res, &out);
            send_frame(res, &sum);
            continue;
        }
        // 'R' u32 n, then n * (u32 tick, f32 steer, f32 gas, f32 brake)
        let n = u32::from_le_bytes(payload[1..5].try_into().unwrap()) as usize;
        fflush(std::ptr::null_mut()); // no half-written stdio in the child
        let t_start = now_us();
        let mut fds = [0i32; 2];
        if pipe(fds.as_mut_ptr()) != 0 {
            send_frame(res, b"ERR pipe");
            continue;
        }
        let pid = fork();
        if pid < 0 {
            close(fds[0]);
            close(fds[1]);
            send_frame(res, b"ERR fork");
            continue;
        }
        if pid == 0 {
            // ---- child: becomes the candidate's simulator
            IS_CHILD.store(1, Ordering::SeqCst);
            close(fds[0]);
            dup2(fds[1], 1);
            close(fds[1]);
            setvbuf(stdout, std::ptr::null_mut(), IONBF, 0);
            for i in 0..n {
                let o = 5 + i * 16;
                let tick = u32::from_le_bytes(payload[o..o + 4].try_into().unwrap()) as usize;
                let rec = (base + tick * STRIDE) as *mut u8;
                std::ptr::copy_nonoverlapping(payload.as_ptr().add(o + 4), rec, 12);
            }
            return; // resume the simulation, with the tail rewritten
        }
        // ---- parent: collect the child's JSON, then stop it dead
        let t_forked = now_us();
        let mut t_first = 0u64;
        close(fds[1]);
        let mut out: Vec<u8> = Vec::with_capacity(4096);
        let mut buf = [0u8; 4096];
        let mut done = false;
        while !done {
            let mut pfd = PollFd {
                fd: fds[0],
                events: POLLIN,
                revents: 0,
            };
            let pr = poll(&mut pfd, 1, 20000);
            if pr <= 0 {
                out.extend_from_slice(b"\nFKSHIM-TIMEOUT\n");
                break;
            }
            let r = real_read()(fds[0], buf.as_mut_ptr() as *mut c_void, buf.len());
            if r <= 0 {
                break;
            }
            if t_first == 0 { t_first = now_us(); }
            out.extend_from_slice(&buf[..r as usize]);
            // everything we need is in ValidatedResult/Desc, which precede IsValid;
            // stopping here skips the DeclaredResult block and the Inputs RLE
            if out.windows(9).any(|w| w == b"\"IsValid\"") {
                done = true;
            }
        }
        if pid > 0 {
            // SIGKILL and walk away: tearing down a 150 MB address space costs
            // milliseconds, and SIGCHLD=SIG_IGN makes the kernel reap for us, so
            // the next candidate does not have to wait for it.
            kill(pid, SIGKILL);
        }
        close(fds[0]);
        out.extend_from_slice(b"\nFKTIME fork_us ");
        utoa(t_forked - t_start, &mut out);
        out.extend_from_slice(b" first_us ");
        utoa(t_first.saturating_sub(t_forked), &mut out);
        out.extend_from_slice(b" done_us ");
        utoa(now_us() - t_start, &mut out);
        out.push(b'\n');
        send_frame(res, &out);
    }
}

fn now_us() -> u64 {
    #[repr(C)]
    struct Ts {
        s: i64,
        ns: i64,
    }
    extern "C" {
        fn clock_gettime(id: c_int, t: *mut Ts) -> c_int;
    }
    let mut t = Ts { s: 0, ns: 0 };
    unsafe {
        clock_gettime(1, &mut t);
    }
    (t.s as u64) * 1_000_000 + (t.ns as u64) / 1000
}

// ---------------------------------------------------------------------- hooks

#[no_mangle]
pub unsafe extern "C" fn lroundf(x: f32) -> i64 {
    let mut p = REAL_LROUNDF.load(Ordering::Relaxed);
    if p == 0 {
        init();
        p = dlsym(RTLD_NEXT, b"lroundf\0".as_ptr() as *const c_char) as usize;
        REAL_LROUNDF.store(p, Ordering::Relaxed);
    }
    let n = N_LROUNDF.fetch_add(1, Ordering::Relaxed) + 1;
    if n >= SAMPLE_NEXT.load(Ordering::Relaxed) {
        if WATCH_ON.load(Ordering::Relaxed) != 0 {
            do_watch(n);
        } else {
            do_sample(n);
        }
    }
    if n >= STOP_AT.load(Ordering::Relaxed) && ARMED.swap(1, Ordering::SeqCst) == 0 {
        log(b"FKSHIM stop\n");
        raise(SIGSTOP);
    }
    if n >= CKPT_AT.load(Ordering::Relaxed)
        && IS_CHILD.load(Ordering::Relaxed) == 0
        && ARMED.swap(1, Ordering::SeqCst) == 0
    {
        forkserver();
    }
    let f: unsafe extern "C" fn(f32) -> i64 = std::mem::transmute(p);
    f(x)
}

fn emit() {
    if IS_CHILD.load(Ordering::Relaxed) == 0 {
        logn(b"FKSHIM lroundf_total ", N_LROUNDF.load(Ordering::Relaxed));
    }
}

#[used]
#[link_section = ".fini_array"]
static FINI: extern "C" fn() = {
    extern "C" fn f() {
        emit()
    }
    f
};
