//! THE HUGE-PAGE ALLOCATOR (perf 6.4): a `#[global_allocator]` in front of the system allocator that
//! serves every allocation of `HUGE_MIN` (64 MB) or more from its own 2 MB-aligned mmap with
//! `madvise(MADV_HUGEPAGE)`, so the bake's large tables — the scene's triangle array (27 M × 64 B on the
//! giant: random access per band-triangle pair, a 4 KB page walk each with the kernel's `madvise` THP
//! policy and glibc's arenas), the per-frame layer tables, the wanted bitmaps and indices, the CSR — sit
//! on 2 MB pages: 512× fewer page faults when first touched, one dTLB entry per 2 MB instead of per 4 KB
//! (`perf stat` on the giant's 4-direction bench: 412 M dTLB load misses, 2.5 M page faults with the
//! system allocator alone). Small allocations go to the system allocator untouched (glibc, with
//! main's `mallopt` tuning). `alloc_zeroed` of a huge block returns the fresh mapping as is (the kernel's
//! pages are zero: no memset pass over 200 MB). Freed huge blocks are kept in a bounded free list by
//! rounded size (the same block comes back page-faulted and huge-paged: the recycled per-frame tables
//! stay warm even through `Vec` growth), the rest is unmapped.
//!
//! MEASURED NEGATIVE on the Genoa guest and therefore OFF BY DEFAULT (LMTOOL_HUGEPAGES=1 turns it on; the
//! switch is read once): on the giant's 4-direction bench a 2 MB threshold cost raster 1.41 → 1.65 s and
//! the wanted index 0.08 → 0.27 (fresh huge mappings per frame: the kernel's fault-time compaction and the
//! 2 MB zeroing on the faulting thread, THP defrag = madvise), a 64 MB threshold still +2.8 % raster /
//! +1.4 % directions; the dTLB potential was ~1 % (412 M page walks of 2 089 G cycles). Kept as the opt-in
//! experiment for a bare-metal box with free, unfragmented memory. Off the x86-64 Linux target the type is
//! a plain pass-through.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

pub struct HugeAlloc;

/// Allocations at or above this go to the huge path.
pub const HUGE_MIN: usize = 64 * 1024 * 1024;
const HUGE_PAGE: usize = 2 * 1024 * 1024;

/// The bounded free list: (ptr, mapped length) of unmapped-but-kept blocks; up to `KEEP` entries and
/// `KEEP_BYTES` bytes in total. A spin lock guards it (the path is rare: one op per large table).
const KEEP: usize = 128;
const KEEP_BYTES: usize = 6 << 30;

struct FreeList {
    lock: AtomicBool,
    n: AtomicUsize,
    bytes: AtomicUsize,
    ptrs: [AtomicUsize; KEEP],
    lens: [AtomicUsize; KEEP],
}

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicUsize = AtomicUsize::new(0);
static FREE: FreeList = FreeList { lock: AtomicBool::new(false), n: AtomicUsize::new(0), bytes: AtomicUsize::new(0), ptrs: [ZERO; KEEP], lens: [ZERO; KEEP] };

/// Counters for the report: huge allocations served, of which from the free list; bytes mapped now.
pub static HUGE_ALLOCS: AtomicU64 = AtomicU64::new(0);
pub static HUGE_REUSED: AtomicU64 = AtomicU64::new(0);
pub static HUGE_MAPPED_BYTES: AtomicUsize = AtomicUsize::new(0);
pub static HUGE_PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

static DISABLED: AtomicBool = AtomicBool::new(false);
static CHECKED: AtomicBool = AtomicBool::new(false);

#[inline]
fn enabled() -> bool {
    if !CHECKED.load(Ordering::Relaxed) {
        // (read once, off the raw environ: the allocator cannot allocate while deciding how to allocate)
        let on = raw_env_has("LMTOOL_HUGEPAGES=");
        DISABLED.store(!on, Ordering::Relaxed);
        CHECKED.store(true, Ordering::Relaxed);
    }
    !DISABLED.load(Ordering::Relaxed)
}

/// Does the process environment carry a variable starting with `prefix` (e.g. "NAME=")? Reads glibc's
/// `environ` directly: no allocation (the allocator cannot allocate while deciding how to allocate).
fn raw_env_has(prefix: &str) -> bool {
    extern "C" {
        static environ: *const *const u8;
    }
    unsafe {
        let mut p = environ;
        if p.is_null() {
            return false;
        }
        while !(*p).is_null() {
            let s = *p;
            let mut k = 0usize;
            let pb = prefix.as_bytes();
            while k < pb.len() && *s.add(k) == pb[k] {
                k += 1;
            }
            if k == pb.len() {
                return true;
            }
            p = p.add(1);
        }
        false
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod sys {
    extern "C" {
        pub fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut u8;
        pub fn munmap(addr: *mut u8, len: usize) -> i32;
        pub fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
    }
    pub const PROT_READ: i32 = 1;
    pub const PROT_WRITE: i32 = 2;
    pub const MAP_PRIVATE: i32 = 2;
    pub const MAP_ANONYMOUS: i32 = 0x20;
    pub const MADV_HUGEPAGE: i32 = 14;
    pub const MAP_FAILED: *mut u8 = !0usize as *mut u8;
}

#[inline]
fn round_up(n: usize, to: usize) -> usize {
    (n + to - 1) / to * to
}

impl FreeList {
    fn lock(&self) {
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
    }
    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
    /// A kept block of exactly `len` mapped bytes, if any (the mapping's length must match: `dealloc` derives
    /// it from the layout again).
    fn take(&self, len: usize) -> Option<*mut u8> {
        self.lock();
        let n = self.n.load(Ordering::Relaxed);
        let mut found = None;
        for i in 0..n {
            if self.lens[i].load(Ordering::Relaxed) == len {
                let p = self.ptrs[i].load(Ordering::Relaxed);
                // swap-remove
                let last = n - 1;
                self.ptrs[i].store(self.ptrs[last].load(Ordering::Relaxed), Ordering::Relaxed);
                self.lens[i].store(self.lens[last].load(Ordering::Relaxed), Ordering::Relaxed);
                self.n.store(last, Ordering::Relaxed);
                self.bytes.fetch_sub(len, Ordering::Relaxed);
                found = Some(p as *mut u8);
                break;
            }
        }
        self.unlock();
        found
    }
    /// Keeps the block when the list has room; returns false when the caller must unmap it.
    fn give(&self, p: *mut u8, len: usize) -> bool {
        self.lock();
        let n = self.n.load(Ordering::Relaxed);
        let kept = if n < KEEP && self.bytes.load(Ordering::Relaxed) + len <= KEEP_BYTES {
            self.ptrs[n].store(p as usize, Ordering::Relaxed);
            self.lens[n].store(len, Ordering::Relaxed);
            self.n.store(n + 1, Ordering::Relaxed);
            self.bytes.fetch_add(len, Ordering::Relaxed);
            true
        } else {
            false
        };
        self.unlock();
        kept
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn huge_map(len: usize) -> *mut u8 {
    // over-map by one huge page to align the start, drop the slack around it
    let want = len + HUGE_PAGE;
    let p = sys::mmap(std::ptr::null_mut(), want, sys::PROT_READ | sys::PROT_WRITE, sys::MAP_PRIVATE | sys::MAP_ANONYMOUS, -1, 0);
    if p == sys::MAP_FAILED {
        return std::ptr::null_mut();
    }
    let start = round_up(p as usize, HUGE_PAGE);
    let head = start - p as usize;
    if head > 0 {
        sys::munmap(p, head);
    }
    let tail = want - head - len;
    if tail > 0 {
        sys::munmap((start + len) as *mut u8, tail);
    }
    sys::madvise(start as *mut u8, len, sys::MADV_HUGEPAGE);
    start as *mut u8
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn huge_alloc(layout: Layout, zeroed: bool) -> *mut u8 {
    let len = round_up(layout.size(), HUGE_PAGE);
    HUGE_ALLOCS.fetch_add(1, Ordering::Relaxed);
    if let Some(p) = FREE.take(len) {
        HUGE_REUSED.fetch_add(1, Ordering::Relaxed);
        if zeroed {
            std::ptr::write_bytes(p, 0, layout.size());
        }
        return p;
    }
    let p = huge_map(len);
    if !p.is_null() {
        let now = HUGE_MAPPED_BYTES.fetch_add(len, Ordering::Relaxed) + len;
        HUGE_PEAK_BYTES.fetch_max(now, Ordering::Relaxed);
    }
    p
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn huge_free(p: *mut u8, layout: Layout) {
    let len = round_up(layout.size(), HUGE_PAGE);
    if !FREE.give(p, len) {
        sys::munmap(p, len);
        HUGE_MAPPED_BYTES.fetch_sub(len, Ordering::Relaxed);
    }
}

#[inline]
fn is_huge(layout: &Layout) -> bool {
    layout.size() >= HUGE_MIN && layout.align() <= HUGE_PAGE
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe impl GlobalAlloc for HugeAlloc {
    // (a huge request that the mapping cannot serve returns null — the allocation error handler aborts —
    // rather than falling back to the system allocator: `dealloc` decides the path from the layout alone,
    // so a huge-sized block must always be a mapping)
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if is_huge(&layout) && enabled() {
            return huge_alloc(layout, false);
        }
        System.alloc(layout)
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if is_huge(&layout) && enabled() {
            return huge_alloc(layout, true);
        }
        System.alloc_zeroed(layout)
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if is_huge(&layout) && enabled() {
            huge_free(ptr, layout);
        } else {
            System.dealloc(ptr, layout);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let (old_h, new_h) = (is_huge(&layout) && enabled(), is_huge(&new_layout) && enabled());
        if !old_h && !new_h {
            return System.realloc(ptr, layout, new_size);
        }
        if old_h && new_h && round_up(layout.size(), HUGE_PAGE) == round_up(new_size, HUGE_PAGE) {
            // the same mapping covers it
            return ptr;
        }
        let np = self.alloc(new_layout);
        if !np.is_null() {
            std::ptr::copy_nonoverlapping(ptr, np, layout.size().min(new_size));
            self.dealloc(ptr, layout);
        }
        np
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
unsafe impl GlobalAlloc for HugeAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 { System.alloc(layout) }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { System.alloc_zeroed(layout) }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) { System.dealloc(ptr, layout) }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 { System.realloc(ptr, layout, new_size) }
}

/// One line for `--profile`: huge allocations served / reused, bytes mapped now and at the peak.
pub fn report() -> String {
    format!(
        "hugepages: {} — {} huge allocations ({} from the free list), {:.1} GB mapped now, {:.1} GB at the peak",
        if enabled() { "on (LMTOOL_HUGEPAGES)" } else { "off" },
        HUGE_ALLOCS.load(Ordering::Relaxed),
        HUGE_REUSED.load(Ordering::Relaxed),
        HUGE_MAPPED_BYTES.load(Ordering::Relaxed) as f64 / 1e9,
        HUGE_PEAK_BYTES.load(Ordering::Relaxed) as f64 / 1e9
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_blocks_round_trip() {
        // through the global allocator (this crate's binary installs it; the test harness uses System unless
        // the lib does — so exercise the type directly)
        let a = HugeAlloc;
        unsafe {
            let l = Layout::from_size_align(HUGE_MIN + 5 * 1024 * 1024 + 17, 8).unwrap();
            let p = a.alloc_zeroed(l);
            assert!(!p.is_null());
            assert!(std::slice::from_raw_parts(p, l.size()).iter().all(|&b| b == 0));
            std::ptr::write_bytes(p, 0xab, l.size());
            let p2 = a.realloc(p, l, HUGE_MIN + 9 * 1024 * 1024);
            assert!(!p2.is_null());
            assert!(std::slice::from_raw_parts(p2, l.size()).iter().all(|&b| b == 0xab));
            a.dealloc(p2, Layout::from_size_align(HUGE_MIN + 9 * 1024 * 1024, 8).unwrap());
            // the block comes back from the free list, zeroed on request
            let l3 = Layout::from_size_align(HUGE_MIN + 9 * 1024 * 1024, 8).unwrap();
            let p3 = a.alloc_zeroed(l3);
            assert!(std::slice::from_raw_parts(p3, l3.size()).iter().all(|&b| b == 0));
            a.dealloc(p3, l3);
            // a small one is the system's
            let s = Layout::from_size_align(64, 8).unwrap();
            let q = a.alloc(s);
            assert!(!q.is_null());
            a.dealloc(q, s);
        }
    }
}
