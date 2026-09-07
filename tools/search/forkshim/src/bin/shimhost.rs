//! `shimhost` — a stand-in engine, so the savestate tree can be tested and
//! partly calibrated with no game, no map and no `.Ghost.Gbx` at all.
//!
//! # Why this exists
//!
//! The fork shim is an `LD_PRELOAD` interposer on `lroundf`. Everything it does
//! — count calls, stop at a checkpoint, fork, patch the decoded input array,
//! probe the consumed boundary, re-enter as a branch node on a fresh socket —
//! is about **process mechanics and one array in memory**. None of it is about
//! Trackmania.
//!
//! So the mechanism can be exercised against a program that merely *behaves
//! like* the engine in the three ways the shim depends on:
//!
//! 1. it calls `lroundf` a fixed number of times per simulated tick;
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
//! # The two knobs that make it a CALIBRATION rig as well as a test rig
//!
//! `--heap MB` and `--dirty KB`. The published cost of a fork child splits into
//! the fork itself, the copy-on-write faults as the child touches its working
//! set, and the validator's finish-and-print path. The first two are functions
//! of **address-space size** and **how much memory a tick dirties**, not of
//! physics — so a stand-in with the engine's memory shape measures them on this
//! box, and the engine's own per-tick simulation cost adds on top.
//!
//! That is a MODEL, not a measurement of the engine, and anything built on it
//! says so. What it removes is the softest term in the prediction: how much of
//! the ~10 ms fixed cost is copy-on-write.
//!
//! **Per-tick CPU here is deliberately near zero**, so a branch timed against
//! this host is fork + COW and nothing else. Do not read a per-tick slope off
//! it.

use std::io::Write;

const STRIDE: usize = 32;

fn arg(a: &[String], name: &str) -> Option<String> {
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned()
}
fn num(a: &[String], name: &str, d: usize) -> usize {
    arg(a, name).map(|v| v.parse().expect("a number")).unwrap_or(d)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n = num(&a, "--ticks", 3000);
    let per_tick = num(&a, "--per-tick", 255);
    let heap_mb = num(&a, "--heap", 0);
    let dirty_kb = num(&a, "--dirty", 0);
    let key_path = std::env::var("FKSHIM_KEY").expect("shimhost needs FKSHIM_KEY");

    // A resident working set of the engine's order, so fork() has to copy page
    // tables for a comparable address space. Touched once, here, before the
    // checkpoint: it is the PARENT's memory, and what a fork costs to copy.
    let mut heap: Vec<u8> = vec![0; heap_mb * 1024 * 1024];
    for i in (0..heap.len()).step_by(4096) {
        heap[i] = (i / 4096) as u8;
    }

    // The decoded input array, exactly the shape the engine holds: one 32-byte
    // record per tick, `f32 steer, f32 gas, f32 brake`, in tick order, one
    // allocation, in an anonymous rw mapping.
    let steer = key_steer(&key_path, n);
    let mut arr = vec![0u8; n * STRIDE];
    for t in 0..n {
        arr[t * STRIDE..t * STRIDE + 4].copy_from_slice(&steer[t].to_le_bytes());
        arr[t * STRIDE + 4..t * STRIDE + 8].copy_from_slice(&1.0f32.to_le_bytes());
        arr[t * STRIDE + 8..t * STRIDE + 12].copy_from_slice(&0.0f32.to_le_bytes());
        // A distinguishable tail, so a stray 12-byte patch is visible.
        arr[t * STRIDE + 16..t * STRIDE + 20].copy_from_slice(&(t as u32).to_le_bytes());
    }

    // The "simulation". The order inside a tick is the load-bearing part: the
    // clock advances FIRST and the record is read AFTER, so a checkpoint that
    // fires inside tick `t` leaves record `t` unconsumed -- which is exactly
    // what the probe should report.
    let mut hash: u64 = 1469598103934665603;
    let dirty = dirty_kb * 1024;
    for t in 0..n {
        for i in 0..per_tick {
            // A value whose rounding depends on the tick, so nothing can be
            // constant-folded away.
            unsafe { lroundf((t as f32) * 0.001 + i as f32 * 1e-6) };
        }
        let rec =
            unsafe { std::ptr::read_volatile(arr.as_ptr().add(t * STRIDE) as *const [u8; 12]) };
        for b in rec {
            hash ^= b as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        // WHAT A TICK DIRTIES. Scattered across the heap on a stride that is
        // not a multiple of the page size, so the pages faulted are spread the
        // way a physics working set's are rather than being one contiguous run
        // the kernel can fault cheaply.
        if dirty > 0 && !heap.is_empty() {
            let len = heap.len();
            for j in (0..dirty).step_by(4096) {
                let o = (t.wrapping_mul(1_000_003).wrapping_add(j)) % len;
                heap[o] = heap[o].wrapping_add(1);
            }
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
    // Keep both allocations alive to the very end; without this LLVM is free to
    // drop them once the loop is done, and the shim's cached base would be
    // verifying freed memory.
    std::hint::black_box((&arr, &heap));
}

extern "C" {
    fn lroundf(x: f32) -> i64;
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
