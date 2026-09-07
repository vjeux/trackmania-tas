//! DEEP FORK POINTS (`forkoracle::ladder`), END TO END, WITH NO GAME.
//!
//! Same rig as `tree.rs`: the real shim, really `LD_PRELOAD`ed, against
//! `shimhost`, whose verdict is a hash of the input records it actually
//! consumed. That hash is what makes these tests mean something: a candidate
//! run from a node 1300 ticks in returns the SAME verdict as the same candidate
//! run from the root only if the node consumed exactly the records the root
//! would have -- the lineage's below its tick, the candidate's above. A ladder
//! that forked from a node of the wrong lineage, or wrote a record the node had
//! already consumed, changes the hash.

use forkoracle::forksrv::{parse_result, rec_of, write_key, ForkServer, Rec};
use forkoracle::inputs::Inputs;
use forkoracle::ladder::Ladder;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TICKS: usize = 3000;

fn ckpt(t: usize) -> u64 {
    forkoracle::clock::ckpt_for_tick(t as i64, 0)
}

fn shim_path() -> PathBuf {
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

/// Refuse a stale shim -- see `tree.rs` for why this is a refusal and not a
/// habit.
fn assert_fresh(so: &Path) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    if let (Ok(so_t), Ok(src_t)) = (
        std::fs::metadata(so).and_then(|m| m.modified()),
        std::fs::metadata(&src).and_then(|m| m.modified()),
    ) {
        assert!(
            so_t >= src_t,
            "{} is OLDER than {}: run `cargo build --release -p forkshim` first",
            so.display(),
            src.display()
        );
    }
}

fn tape(n: usize) -> Vec<u8> {
    (0..n).map(|t| ((t.wrapping_mul(37).wrapping_add(11)) % 251 + 1) as u8).collect()
}

struct Host {
    srv: ForkServer,
    dir: PathBuf,
    reference: Inputs,
}

fn start(tag: &str, ckpt: u64) -> Host {
    let dir = std::env::temp_dir().join(format!("fkshim-ladder-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let steer = tape(TICKS);
    let key = dir.join("key.bin");
    write_key(&key, &steer);
    let mut c = Command::new(env!("CARGO_BIN_EXE_shimhost"));
    c.args([TICKS.to_string()])
        .env("FKSHIM_TEST_HOST", "1")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(std::fs::File::create(dir.join("stdout.log")).unwrap()));
    let srv = ForkServer::start_raw(&dir, c, &key, &shim_path(), ckpt)
        .unwrap_or_else(|e| panic!("shimhost did not reach the checkpoint: {}", e));
    // shimhost's array holds gas 1.0 and brake 0.0 on every record.
    let reference = Inputs::from_arrays(&steer, &vec![1u8; TICKS], &vec![0u8; TICKS]);
    Host { srv, dir, reference }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn tail(c: &Inputs, from: usize) -> Vec<Rec> {
    (from..c.len()).map(|t| rec_of(c.steer[t] as u8, c.gas[t] as u8, c.brake[t] as u8)).collect()
}

fn verdict(out: &str) -> i64 {
    parse_result(out).0.expect("the host must produce a verdict")
}

/// `k` candidates that each change one tick at or after `lo`, all otherwise
/// equal to `base`.
fn batch(base: &Inputs, lo: usize, k: usize) -> Vec<Inputs> {
    (0..k)
        .map(|i| {
            let mut c = base.clone();
            let t = lo + 13 * i;
            c.steer[t] = c.steer[t].wrapping_add(17 + i as i8);
            c
        })
        .collect()
}

/// A batch forks from the deepest grid node below the prefix it shares, the
/// node is made once and reused, and every verdict equals the root's.
#[test]
fn deep_forks_consume_exactly_the_records_the_root_would_have() {
    let mut h = start("deep", ckpt(300));
    let root_tick = h.srv.probe_tick().unwrap();
    assert_eq!(root_tick, 300);
    let mut l = Ladder::new(&h.dir.join("ladder"), root_tick, root_tick, 500, 8, false).unwrap();

    let b1 = batch(&h.reference, 1700, 6);
    let common = Ladder::common_prefix(&b1);
    assert_eq!(common, 1700);
    l.prepare(&mut h.srv, &b1[0], common);
    assert_eq!(l.rung_ticks(), vec![1300], "grid 300 + k*500: the deepest tick <= 1700 is 1300");
    for c in &b1 {
        let (j, from) = l.run(&mut h.srv, c);
        assert_eq!(from, 1300, "the batch must fork from the node it asked for");
        let root = h.srv.run(root_tick, &tail(c, root_tick));
        assert_eq!(verdict(&j), verdict(&root), "a deep fork consumed different records than the root");
    }
    let s = l.stats();
    assert_eq!((s.runs, s.from_rung, s.from_root, s.rungs_made, s.rungs_failed), (6, 6, 0, 1, 0));
    assert_eq!(s.ticks_saved, 6 * 1000);

    // A deeper batch on the same lineage: the new node is made FROM the 1300
    // node, and both stay.
    let b2 = batch(&h.reference, 2400, 4);
    l.prepare(&mut h.srv, &b2[0], Ladder::common_prefix(&b2));
    assert_eq!(l.rung_ticks(), vec![1300, 2300]);
    for c in &b2 {
        let (j, from) = l.run(&mut h.srv, c);
        assert_eq!(from, 2300);
        let root = h.srv.run(root_tick, &tail(c, root_tick));
        assert_eq!(verdict(&j), verdict(&root));
    }
    // And a mutation that changes the verdict at all: the two must agree on
    // being DIFFERENT from the reference, not merely agree with each other.
    let ref_v = verdict(&h.srv.run(root_tick, &tail(&h.reference, root_tick)));
    let (j, _) = l.run(&mut h.srv, &b2[0]);
    assert_ne!(verdict(&j), ref_v, "the candidate's own edit must reach the verdict");
}

/// THE NEGATIVE CONTROL. A node holds one lineage; a candidate of another
/// lineage -- one that differs BELOW the node's tick -- must never be run from
/// it, however deep its own edit is. Then the other lineage gets nodes of its
/// own, and both lineages keep getting the root's answer.
#[test]
fn a_node_of_another_lineage_is_never_used_below_its_own_tick() {
    let mut h = start("lineage", ckpt(300));
    let root_tick = h.srv.probe_tick().unwrap();
    let mut l = Ladder::new(&h.dir.join("ladder"), root_tick, root_tick, 500, 8, false).unwrap();

    let old = batch(&h.reference, 2000, 3);
    l.prepare(&mut h.srv, &old[0], Ladder::common_prefix(&old));
    assert_eq!(l.rung_ticks(), vec![1800]);

    // A new lineage: the incumbent changed at tick 1000, and a candidate of it
    // edits tick 2500. The 1800 node holds the OLD tick 1000 and must be
    // refused; the candidate runs from the root.
    let mut inc = h.reference.clone();
    inc.steer[1000] ^= 0x33;
    let mut c = inc.clone();
    c.steer[2500] ^= 0x44;
    let (j, from) = l.run(&mut h.srv, &c);
    assert_eq!(from, root_tick, "a node of another lineage was used");
    assert_eq!(verdict(&j), verdict(&h.srv.run(root_tick, &tail(&c, root_tick))));

    // The new lineage gets its own node at the SAME grid tick, and the two
    // coexist: each lineage is served by its own.
    let new = batch(&inc, 2000, 3);
    l.prepare(&mut h.srv, &new[0], Ladder::common_prefix(&new));
    assert_eq!(l.rung_ticks(), vec![1800, 1800]);
    for c in new.iter().chain(old.iter()) {
        let (j, from) = l.run(&mut h.srv, c);
        assert_eq!(from, 1800);
        assert_eq!(verdict(&j), verdict(&h.srv.run(root_tick, &tail(c, root_tick))));
    }
    // The two 1800 nodes give DIFFERENT verdicts for the same tail, because
    // they hold different lineages -- which is exactly why one cannot stand
    // in for the other.
    let (jo, _) = l.run(&mut h.srv, &old[0]);
    let (jn, _) = l.run(&mut h.srv, &new[0]);
    assert_ne!(verdict(&jo), verdict(&jn));
}

/// Least-recently-used eviction at the cap, never below one rung, and a batch
/// of one makes nothing.
#[test]
fn the_cap_evicts_the_least_recently_used_node() {
    let mut h = start("cap", ckpt(300));
    let root_tick = h.srv.probe_tick().unwrap();
    let mut l = Ladder::new(&h.dir.join("ladder"), root_tick, root_tick, 500, 2, false).unwrap();

    // one candidate: no node
    let one = batch(&h.reference, 2000, 1);
    l.prepare(&mut h.srv, &one[0], Ladder::common_prefix(&one));
    assert!(l.rung_ticks().is_empty(), "a batch of one shares nothing and gets no node");

    let a = batch(&h.reference, 900, 2);
    l.prepare(&mut h.srv, &a[0], Ladder::common_prefix(&a));
    let b = batch(&h.reference, 1400, 2);
    l.prepare(&mut h.srv, &b[0], Ladder::common_prefix(&b));
    assert_eq!(l.rung_ticks(), vec![800, 1300]);
    // use the 800 node, so 1300 is the least recently RUN from
    for c in &a {
        let (_, from) = l.run(&mut h.srv, c);
        assert_eq!(from, 800);
    }
    // The batch at 2400 is made FROM the 1300 node (its deepest usable one),
    // which marks it used: the victim is 800, the node nobody needs for this.
    let d = batch(&h.reference, 2400, 2);
    l.prepare(&mut h.srv, &d[0], Ladder::common_prefix(&d));
    assert_eq!(l.rung_ticks(), vec![1300, 2300], "the base of the new node must survive; 800 was the LRU");
    assert_eq!(l.stats().rungs_evicted, 1);
    for c in &d {
        let (j, from) = l.run(&mut h.srv, c);
        assert_eq!(from, 2300);
        assert_eq!(verdict(&j), verdict(&h.srv.run(root_tick, &tail(c, root_tick))));
    }
}
