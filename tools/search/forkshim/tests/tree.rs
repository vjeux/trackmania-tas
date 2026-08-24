//! THE SAVESTATE TREE, END TO END, WITH NO GAME AND NO GHOST.
//!
//! These tests run the real shim, in a real `LD_PRELOAD`, against `shimhost` —
//! a stand-in that behaves like the engine in the only three ways the shim
//! depends on (see `src/bin/shimhost.rs`). Everything the tree is made of is
//! exercised: the checkpoint, the fork server, the page-fault probe, the branch
//! command, the socket re-entry on fresh fds, the cached-base verification, the
//! forward-only refusal, and generations of nodes.
//!
//! # Why bother, when the real measurement needs the real engine
//!
//! Because the real engine was not available when this was written, and because
//! the two questions are genuinely separable. **Whether the mechanism works is
//! a question about processes; how much it costs is a question about the
//! engine.** Answering the first here means the first run against the real
//! engine measures cost instead of debugging fd inheritance.
//!
//! It also gives the semantics a permanent test with a *known answer*. The
//! host's verdict is a hash of the records it actually consumed, so:
//!
//! * a patch ABOVE the boundary must change the verdict — the write took;
//! * a patch BELOW the boundary must NOT change it — the record was already
//!   consumed and the write was a silent no-op.
//!
//! That is the defect this whole component exists for, reproduced in
//! milliseconds, both sides, on every `cargo test`.

use forkoracle::forksrv::{
    parse_result, rec_of, write_key, BranchReq, ForkServer, Rec, STRIDE,
};
use forkoracle::tree::Tree;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TICKS: usize = 3000;
const PER_TICK: usize = 255;

fn shim_path() -> PathBuf {
    // The test binary lives in target/<profile>/deps/; the cdylib is one up.
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    for n in ["libforkshim.so", "libfkshim.so"] {
        let c = p.join(n);
        if c.exists() {
            assert_fresh(&c);
            return c;
        }
    }
    panic!("no libforkshim.so beside the test binary at {}", p.display());
}

/// REFUSE A STALE SHIM.
///
/// This cost real time on the first run of these tests. `cargo test` rebuilds
/// the test binary and the rlib, but the **cdylib on disk** — the file that is
/// actually `LD_PRELOAD`ed — is not always refreshed by it. So a fix to
/// `lib.rs` was compiled, the tests ran, and they exercised the PREVIOUS build:
/// the failure looked like the fix not working, and a pass would have looked
/// like the fix working. Either way the test would have been reporting on a
/// file nobody had just written.
///
/// That is this project's "stale hand-built toolchain" pattern, in miniature,
/// and the honest answer to it is a refusal rather than a habit of remembering
/// to `cargo build` first.
fn assert_fresh(so: &Path) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let (a, b) = (
        std::fs::metadata(so).and_then(|m| m.modified()),
        std::fs::metadata(&src).and_then(|m| m.modified()),
    );
    if let (Ok(so_t), Ok(src_t)) = (a, b) {
        assert!(
            so_t >= src_t,
            "{} is OLDER than {} -- `cargo test` does not always refresh the cdylib, and these \
             tests LD_PRELOAD the file on disk. Run `cargo build --release -p forkshim` first. \
             Refusing rather than silently testing the previous build.",
            so.display(),
            src.display()
        );
    }
}

/// A deterministic pseudo-tape. Values are distinct enough that the shim's
/// Horspool key search has something to lock onto, which is the same property
/// `write_key` looks for in a real tape.
fn tape(n: usize) -> Vec<u8> {
    (0..n).map(|t| ((t.wrapping_mul(37).wrapping_add(11)) % 251 + 1) as u8).collect()
}

struct Host {
    srv: ForkServer,
    dir: PathBuf,
    steer: Vec<u8>,
}

/// Start `shimhost` under the shim, stopped at `ckpt` lroundf calls.
fn start(tag: &str, ckpt: u64) -> Host {
    let dir = std::env::temp_dir().join(format!("fkshim-tree-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let steer = tape(TICKS);
    let key = dir.join("key.bin");
    write_key(&key, &steer);

    let mut c = Command::new(env!("CARGO_BIN_EXE_shimhost"));
    c.args([TICKS.to_string(), PER_TICK.to_string()])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(std::fs::File::create(dir.join("stdout.log")).unwrap()));
    let srv = ForkServer::start_raw(&dir, c, &key, &shim_path(), ckpt)
        .unwrap_or_else(|e| panic!("shimhost did not reach the checkpoint: {}", e));
    Host { srv, dir, steer }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn recs(steer: &[u8], from: usize) -> Vec<Rec> {
    (from..steer.len()).map(|t| rec_of(steer[t], 1, 0)).collect()
}

/// The host's verdict for a tail: `Time` is a hash of the consumed records, so
/// two runs agree iff they consumed the same inputs.
fn verdict(out: &str) -> Option<i64> {
    parse_result(out).0
}

fn branch_req<'a>(sock: &'a str, from: usize, recs: &'a [Rec], k: u64) -> BranchReq<'a> {
    BranchReq {
        from,
        recs,
        stop_after_lroundf: (k * PER_TICK as u64).max(1),
        sock,
        trace_path: "",
        segs: &[],
        sample_stride: 1,
        sample_max: 0,
        key: (0, 1),
    }
}

/// The checkpoint fires, the input array is found, and the probe names a tick
/// consistent with where the clock stopped. Without this nothing below means
/// anything.
#[test]
fn the_host_reaches_a_checkpoint_and_can_be_probed() {
    let mut h = start("probe", (500 * PER_TICK) as u64);
    assert_ne!(h.srv.base, 0, "the shim did not find the input array");
    let p = h.srv.probe_tick().expect("the page-fault probe must answer");
    assert!(
        (495..=505).contains(&p),
        "probe reported tick {}, which is nowhere near the tick the clock stopped in",
        p
    );
}

/// **THE TWO-SIDED CONTROL ON THE FORWARD-ONLY RULE, with a known answer.**
///
/// Above the boundary a patch changes the verdict. Below it, the identical
/// patch does not — the record was already consumed and the write is a silent
/// no-op. Either half alone would pass for a broken test: a rig that ignored
/// every patch satisfies the second, and one that applied them all satisfies
/// the first.
#[test]
fn a_write_above_the_boundary_takes_and_a_write_below_it_is_silently_dropped() {
    let mut h = start("bothsides", (500 * PER_TICK) as u64);
    let p = h.srv.probe_tick().unwrap();
    let base = verdict(&h.srv.run(p + 1, &recs(&h.steer, p + 1)))
        .expect("the identity resume must produce a verdict");

    // ABOVE: change one tick the host has not read yet.
    let mut above = h.steer.clone();
    above[p + 5] ^= 0x55;
    let va = verdict(&h.srv.run(p + 1, &recs(&above, p + 1))).unwrap();
    assert_ne!(va, base, "a patch ABOVE the boundary did not change the verdict");

    // BELOW: the same change, at a tick already consumed.
    let mut below = h.steer.clone();
    below[p.saturating_sub(5)] ^= 0x55;
    let vb = verdict(&h.srv.run(p.saturating_sub(5), &recs(&below, p.saturating_sub(5)))).unwrap();
    assert_eq!(
        vb, base,
        "a patch BELOW the boundary changed the verdict -- the defect this component exists \
         for does not behave as recorded, which is far more interesting than if it did"
    );
}

/// **Q1's MECHANISM.** A child consumes more ticks and comes back as a fork
/// point on fresh fds.
#[test]
fn a_branch_child_re_enters_the_fork_server_as_a_new_node() {
    let mut h = start("branch", (500 * PER_TICK) as u64);
    let p0 = h.srv.probe_tick().unwrap();
    let mut tree = Tree::new(&h.dir).unwrap();
    let sock = tree.sock_path();

    let pid = h.srv.branch(&branch_req(&sock, 0, &[], 20)).expect("branch was refused");
    let mut node = tree.accept(20_000).expect("the node never arrived on the socket");
    assert_eq!(node.pid, pid, "a different process connected than the one we branched");
    assert_eq!(node.base, h.srv.base, "the node's input array moved, which it must not");

    let p1 = node.probe().expect("a node must be able to probe its own boundary");
    assert!(
        p1 > p0,
        "the node's boundary ({}) is not past its parent's ({}) -- it consumed nothing",
        p1,
        p0
    );
    assert!(
        (p1 as i64 - p0 as i64 - 20).abs() <= 2,
        "asked for 20 more ticks and the node advanced {}",
        p1 as i64 - p0 as i64
    );
}

/// The refusal fires on a real node, not just in a unit test with a fake
/// socket.
#[test]
fn a_live_node_refuses_a_write_at_or_below_its_own_boundary() {
    let mut h = start("refuse", (500 * PER_TICK) as u64);
    h.srv.probe_tick().unwrap();
    let mut tree = Tree::new(&h.dir).unwrap();
    let sock = tree.sock_path();
    h.srv.branch(&branch_req(&sock, 0, &[], 20)).unwrap();
    let mut node = tree.accept(20_000).unwrap();
    let p = node.probe().unwrap();

    let r = node.run(p, &recs(&h.steer, p));
    assert!(
        r.unwrap_err().contains("FORWARD-ONLY VIOLATION"),
        "a node accepted a write at its own boundary"
    );
    let r = node.branch(&branch_req(&sock, p - 10, &recs(&h.steer, p - 10), 5));
    assert!(r.unwrap_err().contains("FORWARD-ONLY VIOLATION"));
    // ...and the escape hatch the negative control needs still works, so the
    // refusal is a policy of the ordinary API rather than an inability.
    assert!(node
        .run_below_boundary_for_the_negative_control(p - 10, &recs(&h.steer, p - 10))
        .is_ok());
}

/// **A TREE, not a branch.** Descend `d` generations, each appending its own
/// macro and probing its own boundary, and require the leaf's verdict to equal
/// the verdict of the same tape run flat from the root.
///
/// This is the depth control in miniature and it is the reason it exists: a
/// one-tick boundary error per generation is a `d`-tick error at the leaf, each
/// one individually invisible, and a depth-1 test cannot see any of it. The
/// first version of this test found a real defect on its first run — the branch
/// re-entry verified its cached base against the KEY, which stops being what
/// the array holds the moment a generation patches anything.
fn descend_and_compare(d: usize) {
    let tag = format!("depth{}", d);
    let mut h = start(&tag, (400 * PER_TICK) as u64);
    let p0 = h.srv.probe_tick().unwrap();
    let mut tree = Tree::new(&h.dir).unwrap();
    let sock = tree.sock_path();

    // Descend, mutating as we go, keeping the full tape in step so the flat run
    // below is the SAME tape by construction rather than by re-derivation.
    let mut full = h.steer.clone();
    let mut node: Option<forkoracle::tree::Node> = None;
    let mut from = p0 + 1;
    for g in 0..d {
        let k = 8usize;
        assert!(from + k + 2 < TICKS, "generation {} ran off the end of the tape", g);
        for t in from..from + k {
            full[t] = ((g * 31 + t) % 251 + 1) as u8;
        }
        let macro_recs: Vec<Rec> = (from..from + k).map(|t| rec_of(full[t], 1, 0)).collect();
        let req = branch_req(&sock, from, &macro_recs, k as u64);
        let pid = match node.as_mut() {
            None => h.srv.branch(&req).unwrap(),
            Some(n) => n.branch(&req).unwrap_or_else(|e| panic!("generation {}: {}", g, e)),
        };
        let mut child = tree
            .accept(20_000)
            .unwrap_or_else(|e| panic!("generation {} never arrived: {}", g, e));
        assert_eq!(child.pid, pid, "generation {}: the wrong process connected", g);
        let pb = child.probe().unwrap_or_else(|e| panic!("generation {}: {}", g, e));
        assert!(
            pb >= from + k - 1,
            "generation {} consumed too little: boundary {} after writing through {}",
            g,
            pb,
            from + k - 1
        );
        from = pb + 1;
        node = Some(child);
    }
    let mut leaf = node.unwrap();
    let deep = verdict(&leaf.run(from, &recs(&full, from)).unwrap()).unwrap();

    // The same tape, evaluated in ONE fork from the root.
    let flat = verdict(&h.srv.run(p0 + 1, &recs(&full, p0 + 1))).unwrap();
    assert_eq!(
        deep, flat,
        "{} generations of branching did not produce the tape a single flat resume produces",
        d
    );
}

#[test]
fn one_generation_produces_the_same_tape_as_a_flat_run() {
    descend_and_compare(1);
}

#[test]
fn ten_generations_produce_the_same_tape_as_a_flat_run() {
    descend_and_compare(10);
}

/// Fifty generations. Depth-1 exactness does not establish a tree, and this is
/// the shallowest depth at which "the mechanism survives being used the way the
/// explorer will use it" is a claim about a tree at all.
#[test]
fn fifty_generations_produce_the_same_tape_as_a_flat_run() {
    descend_and_compare(50);
}

/// A node that is released is really gone. A beam of orphans each holding a
/// whole address space is how a box dies, and `Drop` is the only thing standing
/// between a search and that.
#[test]
fn releasing_a_node_ends_its_process() {
    let mut h = start("reap", (500 * PER_TICK) as u64);
    h.srv.probe_tick().unwrap();
    let mut tree = Tree::new(&h.dir).unwrap();
    let sock = tree.sock_path();
    h.srv.branch(&branch_req(&sock, 0, &[], 20)).unwrap();
    let pid = {
        let mut n = tree.accept(20_000).unwrap();
        n.probe().unwrap();
        n.pid
    }; // dropped here
    for _ in 0..200 {
        if !Path::new(&format!("/proc/{}", pid)).exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("node {} was still alive two seconds after being dropped", pid);
}

/// The record stride the shim patches with is the one the host lays out. A
/// mismatch here would corrupt neighbouring ticks silently.
#[test]
fn the_record_stride_is_the_one_both_sides_use() {
    assert_eq!(STRIDE, 32);
}
