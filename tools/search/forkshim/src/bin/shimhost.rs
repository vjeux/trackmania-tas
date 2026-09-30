//! `shimhost` — a stand-in engine, so the savestate tree can be tested with no
//! game, no map and no `.Ghost.Gbx` at all.
//!
//! # Why this exists
//!
//! Everything the shim does — advance a clock, stop at a checkpoint, fork,
//! patch the decoded input array, probe the consumed boundary, re-enter as a
//! branch node on a fresh socket — is about **process mechanics and one array
//! in memory**. None of it is about Trackmania.
//!
//! So the mechanism can be exercised against a program that merely *behaves
//! like* the engine in the three ways the shim depends on:
//!
//! 1. it announces one tick of simulated time at a time;
//! 2. it holds one decoded input array, 32 bytes per tick, in an `rw` heap
//!    mapping, and reads it strictly **in tick order, one record per tick** —
//!    which is what makes the page-fault probe meaningful;
//! 3. it prints a verdict containing `"IsValid"` when it finishes.
//!
//! That is enough to test the whole tree end to end, including the thing that
//! matters most: **a record already consumed cannot be un-consumed.** The
//! host's verdict is a hash of the records it actually consumed, so a write
//! that lands above the boundary changes the answer and a write that lands
//! below it does not — the exact signature of the defect the forward-only rule
//! exists for, reproducible in milliseconds with no engine.
//!
//! # How it announces a tick
//!
//! In the real server the shim patches the entry of the engine's own per-tick
//! function and reads `new_time`, `dt` and the race start out of it. This host
//! has no such function and no engine to read a race start from, so it calls
//! the shim's `fkshim_tick(new_ms, dt, race_start_ms)` seam directly — the same
//! code path from there on. Its launcher must set `FKSHIM_TEST_HOST=1`, which
//! is the shim's only licence to run without hooking a real tick function.
//!
//! # What it does NOT establish
//!
//! Nothing about cost. This host has a ~1 MB address space and no physics; the
//! Q1 numbers are about forking a ~150 MB engine and simulating real ticks, and
//! they can only be measured on the real thing. It also says nothing about
//! whether the real engine reads its input array the way this one does — that
//! is what the page-fault probe measures on the real engine, and it is why the
//! probe is asked of the engine rather than assumed.

use std::io::Write;

const STRIDE: usize = 32;
/// The engine's own numbers: the simulation starts at 1000 ms, the race a
/// little later, and a tick is 10 ms. Only the SHAPE matters here — the tests
/// convert ticks to clock values through `forkoracle::clock`, exactly as the
/// real driver does.
const SIM_START_MS: u32 = 1000;
const RACE_START_MS: u32 = 2200;
const DT: u32 = 10;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n: usize = a.first().and_then(|v| v.parse().ok()).unwrap_or(2000);
    let key_path = std::env::var("FKSHIM_KEY").expect("shimhost needs FKSHIM_KEY");
    assert_eq!(
        std::env::var("FKSHIM_TEST_HOST").ok().as_deref(),
        Some("1"),
        "shimhost must be launched with FKSHIM_TEST_HOST=1; without it the shim \
         refuses to run on a host it cannot hook, which is the point of that check"
    );

    // The decoded input array, exactly the shape the engine holds: one 32-byte
    // record per tick, `f32 steer, f32 gas, f32 brake`, in tick order, one
    // allocation, in an anonymous rw mapping.
    let steer = key_steer(&key_path, n);
    let mut arr = vec![0u8; n * STRIDE];
    for t in 0..n {
        // THE ENGINE'S LAYOUT, field for field: flags, steer, gas, brake, then
        // the tail the engine fills and the tape never touches. A host with a
        // different layout would test the shim against a record shape that does
        // not exist, which is exactly how the base ended up four bytes off.
        arr[t * STRIDE..t * STRIDE + 4].copy_from_slice(&2u32.to_le_bytes());
        arr[t * STRIDE + 4..t * STRIDE + 8].copy_from_slice(&steer[t].to_le_bytes());
        arr[t * STRIDE + 8..t * STRIDE + 12].copy_from_slice(&1.0f32.to_le_bytes());
        arr[t * STRIDE + 12..t * STRIDE + 16].copy_from_slice(&0.0f32.to_le_bytes());
        // A distinguishable tail so a stray patch outside the three input
        // fields is visible.
        arr[t * STRIDE + 24..t * STRIDE + 28].copy_from_slice(&(t as u32).to_le_bytes());
        arr[t * STRIDE + 28..t * STRIDE + 32].copy_from_slice(&2u32.to_le_bytes());
    }

    // The "simulation". The order inside a tick is the load-bearing part, and
    // it is the engine's: the tick is ANNOUNCED first and the record is read
    // AFTER, so a checkpoint that fires on tick `t` leaves record `t`
    // unconsumed -- which is exactly what the probe should report. The read is
    // of the WHOLE 32-byte record, from its first byte, because that is what
    // the engine's `movups [rdx+rcx]` pair does and it is what the page-fault
    // probe sees.
    let tick = tick_fn();
    let mut hash: u64 = 1469598103934665603;
    // The countdown: ticks the engine runs before the race exists. They must
    // not move the clock, and the shim must survive them.
    let countdown = ((RACE_START_MS - SIM_START_MS) / DT) as usize;
    for i in 0..countdown {
        unsafe { tick(SIM_START_MS + (i as u32 + 1) * DT, DT, u32::MAX) };
    }
    for t in 0..n {
        unsafe { tick(RACE_START_MS + t as u32 * DT, DT, RACE_START_MS) };
        let rec = unsafe {
            std::ptr::read_volatile(arr.as_ptr().add(t * STRIDE) as *const [u8; STRIDE])
        };
        // Hash the three input fields only: the rest is engine-owned and a
        // patch must never move it.
        for b in &rec[4..16] {
            let b = *b;
            hash ^= b as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
    }

    // The verdict, in the shape `forkoracle::forksrv::parse_result` reads.
    let mut out = std::io::stdout().lock();
    let _ = write!(
        out,
        "{{\n  \"ValidatedResult\": {{\n    \"Time\": {},\n    \"NbCheckpoints\": {}\n  }},\n  \"IsValid\": true\n}}\n",
        hash % 1_000_000,
        n
    );
    let _ = out.flush();
    // Keep the array alive to the very end; without this LLVM is free to drop
    // the allocation once the loop is done and the shim's cached base would be
    // verifying freed memory.
    std::hint::black_box(&arr);
}

type TickFn = unsafe extern "C" fn(u32, u32, u32);

extern "C" {
    fn dlsym(handle: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
}

/// Resolve the shim's tick seam AT RUNTIME, out of whatever `LD_PRELOAD`
/// loaded. Linking it statically would give this process its own copy of the
/// shim's statics -- a second clock nobody drives -- so the lookup is
/// deliberate, and its absence is a hard failure rather than a quiet no-op.
fn tick_fn() -> TickFn {
    let p = unsafe { dlsym(std::ptr::null_mut(), b"fkshim_tick\0".as_ptr()) };
    assert!(
        !p.is_null(),
        "shimhost found no `fkshim_tick`: it must run under LD_PRELOAD of libforkshim.so"
    );
    unsafe { std::mem::transmute::<*mut std::ffi::c_void, TickFn>(p) }
}

/// Read the steer sequence the shim will search for out of the key file the
/// driver wrote, so the host's array and the shim's key agree by construction.
fn key_steer(path: &str, n: usize) -> Vec<f32> {
    let d = std::fs::read(path).expect("key file");
    let m = u32::from_le_bytes(d[0..4].try_into().unwrap()) as usize;
    assert_eq!(m, n, "the key file and the host disagree about the tape length");
    (0..n)
        .map(|i| f32::from_le_bytes(d[12 + 4 * i..16 + 4 * i].try_into().unwrap()))
        .collect()
}
