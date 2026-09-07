//! THE SAVESTATE TREE: driver side.
//!
//! # What a tree node is
//!
//! The classic fork server has one fork point. Every candidate is a child that
//! rewrites the tail of the input array, runs to the finish, prints a time and
//! dies. Anything the child learned on the way is lost, so reaching a state
//! 2000 ticks in costs a full re-simulation of those 2000 ticks, every time.
//!
//! A **node** is a fork child that stops after a few ticks and re-enters the
//! fork server itself, on a socket of its own. It is a savestate: a paused
//! simulation that can be forked again. A search over macros then pays the
//! macro, not the prefix.
//!
//! # The three things that make this safe rather than merely fast
//!
//! 1. **Every node probes its own consumed boundary.** The checkpoint
//!    is not a fixed simulation point -- under load it moves in whole chunks of
//!    ~62 calls -- so where a node stopped is a property of that node and of
//!    nothing else. A node whose probe fails is destroyed, never used with an
//!    assumed boundary. (`tm2020-forkserver.md`: a failed probe is a hard
//!    abort, never a fallback.)
//! 2. **`branch` refuses to write at or below that boundary.** A record the
//!    engine has already consumed cannot be un-consumed; rewriting it is a
//!    silent no-op that scores exactly the parent's score, so the mutation
//!    disappears and the lineage is contaminated for free. That is defect 1 of
//!    `tm2020-phantoms.md` and it is the reason this module exists. The refusal
//!    is here, at the lowest level that knows the boundary, and it is not
//!    optional.
//! 3. **Calibration may only push the boundary LATER, never earlier.** A caller
//!    that has its own estimate combines it as `max(estimate, probe)`, never
//!    `min`. `Node::floor` does that arithmetic so no caller has to remember it.
//!
//! # Why a unix socket
//!
//! A branch child inherits its parent's command pipe. If it served that pipe,
//! two processes would race for every command byte and the loser would
//! eventually execute a command assembled from the middle of somebody else's
//! patch payload. So the node gets fresh descriptors: it connects to a listener
//! the driver owns, and the driver matches the connection to the branch it
//! asked for **by the pid the node names in its own handshake**.

use crate::forksrv::{
    parse_probe, payload_branch, payload_probe, payload_run, read_frame, write_frame,
    BranchReq, Rec,
};
use std::os::raw::c_int;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

extern "C" {
    fn kill(pid: c_int, sig: c_int) -> c_int;
}
const SIGKILL: c_int = 9;

/// The driver's listening end. One per fork server.
pub struct Tree {
    listener: UnixListener,
    path: PathBuf,
    /// Every node this tree has handed out and not yet reaped. Dropping the
    /// tree kills all of them: a node is a paused 150 MB engine, and a beam of
    /// orphans is how a box dies.
    live: Vec<i32>,
}

impl Tree {
    /// Bind the listener inside the fork server's own work directory.
    ///
    /// The work directory is already per-process and already locked, so the
    /// socket inherits that isolation rather than inventing its own naming
    /// scheme. Two runs cannot collide on it for the same reason two runs
    /// cannot collide on their replays -- which is the failure mode behind four
    /// separate silent corruptions in this project's history.
    pub fn new(dir: &Path) -> Result<Tree, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join("branch.sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).map_err(|e| {
            format!("binding the branch socket at {}: {}", path.display(), e)
        })?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Tree { listener, path, live: Vec::new() })
    }

    /// The path a `BranchReq` must carry. A `sockaddr_un` path is 107 bytes at
    /// most and the shim refuses a longer one rather than truncating it into a
    /// path that names something else.
    pub fn sock_path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    /// Wait for one node to arrive and complete its handshake.
    ///
    /// Returns the node with its own `base`, `clock` and pid. The caller checks
    /// the pid against what `branch` returned: a node is only the node you
    /// asked for if it says so itself.
    pub fn accept(&mut self, timeout_ms: i32) -> Result<Node, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms.max(0) as u64);
        let stream = loop {
            match self.listener.accept() {
                Ok((s, _)) => break s,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return Err(format!(
                            "no branch node connected within {} ms -- the child either never \
                             reached its stop point or could not reach {}",
                            timeout_ms,
                            self.path.display()
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_micros(200));
                }
                Err(e) => return Err(format!("accept: {}", e)),
            }
        };
        self.handshake(stream)
    }

    /// The READY handshake of a node that just connected.
    fn handshake(&mut self, stream: UnixStream) -> Result<Node, String> {
        stream.set_nonblocking(false).map_err(|e| e.to_string())?;
        let mut n = Node { sock: stream, base: 0, clock: 0, pid: -1, boundary: None, dead: false, sim_ms: 0, race_start: 0 };
        let hello = read_frame(&mut n.sock).ok_or("a branch node connected and said nothing")?;
        let s = String::from_utf8_lossy(&hello).into_owned();
        let ready = crate::forksrv::parse_ready_full(&s)?;
        let (base, clock, pid) = (ready.base, ready.clock, ready.pid);
        n.sim_ms = ready.sim_ms;
        n.race_start = ready.race_start;
        let pid = pid.ok_or_else(|| {
            format!(
                "a branch node handshook without naming its pid ({:?}) -- a node the driver \
                 cannot kill is an orphan holding a 150 MB address space",
                s.trim()
            )
        })?;
        n.base = base;
        n.clock = clock;
        n.pid = pid;
        self.live.push(pid);
        Ok(n)
    }

    /// [`Tree::accept`], but give up EARLY if `pid` -- the child that was told
    /// to connect -- has already exited.
    ///
    /// A branch child stops at an `lroundf` count. When the engine ENDS THE
    /// RUN inside the child's budget (the car fell off the world, the validator
    /// declared the replay invalid, the declared time was reached) it stops
    /// calling `lroundf`, finishes the validation and exits -- and the plain
    /// `accept` waits the whole 120 s watchdog for a node that will never
    /// come. Measured on Summer 2026 - 01: one such child per ~200 random
    /// tapes, each a 120 s hang. Distinguishing "exited" from "not yet" turns
    /// the hang into an episode end.
    pub fn accept_expecting(&mut self, pid: i32, timeout_ms: i32) -> Result<Option<Node>, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms.max(0) as u64);
        let proc_path = format!("/proc/{}", pid);
        let mut polls = 0u32;
        let stream = loop {
            match self.listener.accept() {
                Ok((s, _)) => break s,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return Err(format!(
                            "no branch node connected within {} ms -- the child either never \
                             reached its stop point or could not reach {}",
                            timeout_ms,
                            self.path.display()
                        ));
                    }
                    polls += 1;
                    // /proc/<pid> vanishes when the child is reaped; a zombie
                    // still has one but reads as State: Z.
                    if polls % 50 == 0 {
                        let gone = match std::fs::read_to_string(format!("{}/stat", proc_path)) {
                            Err(_) => true,
                            Ok(s) => s.split(')').nth(1).map(|r| r.trim_start().starts_with('Z')).unwrap_or(true),
                        };
                        if gone {
                            // one last look, in case it connected just before exiting
                            if let Ok((s, _)) = self.listener.accept() {
                                break s;
                            }
                            return Ok(None);
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_micros(200));
                }
                Err(e) => return Err(format!("accept: {}", e)),
            }
        };
        self.handshake(stream).map(Some)
    }

    /// Forget a node the caller has already destroyed.
    pub fn reaped(&mut self, pid: i32) {
        self.live.retain(|p| *p != pid);
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        for p in std::mem::take(&mut self.live) {
            unsafe { kill(p, SIGKILL) };
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

// ---------------------------------------------------------------- headroom

/// One-minute load average.
pub fn load1() -> f64 {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse().ok()))
        .unwrap_or(f64::NAN)
}

/// Is there a core free for every process this batch will run?
///
/// **This replaced an absolute load threshold, and the replacement matters.**
/// The failure a timing batch has to avoid is the engine's wall-clock catch-up
/// branch: when the simulation cannot keep up with real time, `lroundf` moves in
/// whole chunks of ~62 calls, a fixed checkpoint count lands at a different
/// simulation point, and the number becomes a number about the load. That
/// happens when a process does not get a core — which is about **headroom**,
/// not about the absolute load average.
///
/// A flat "refuse above load1 = 2" is wrong in both directions on a big box: it
/// refuses a 176-core machine idling at 3 under its own daemons, and it would
/// permit a 4-core machine at 2 with three of its cores gone. So the test is
/// `load1 + concurrency <= 0.8 * cores`.
///
/// A gate like this is still only a PROXY. The direct control is
/// [`checkpoint_stability`]: ask the engine where it stopped, twice, around the
/// batch.
pub fn require_headroom(concurrency: usize) -> Result<f64, String> {
    let cores = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1);
    let l = load1();
    let need = l + concurrency as f64;
    let have = 0.8 * cores as f64;
    if need > have {
        return Err(format!(
            "not enough headroom for a timing batch: load1 {:.2} + {} concurrent workers = \
             {:.2}, against {} cores (usable {:.1}). Under contention the engine takes its \
             wall-clock catch-up branch, `lroundf` moves in whole ~62-call chunks, and a fixed \
             checkpoint lands at a different simulation point -- the number would be about the \
             load.",
            l, concurrency, need, cores, have
        ));
    }
    Ok(l)
}

/// THE DIRECT CONTROL a load gate is only a proxy for: did the checkpoint stay
/// put while the batch ran?
///
/// Ask the engine where it stopped, before and after. The probe is
/// authoritative about what has been consumed, so two identical answers say the
/// simulation point did not move; a difference says the batch straddled a
/// change and its timings are UNMEASURED rather than merely noisy.
pub fn checkpoint_stability(before: usize, after: usize) -> Result<(), String> {
    if before != after {
        return Err(format!(
            "the root's probed boundary moved from tick {} to tick {} DURING the batch. The \
             checkpoint is not a fixed simulation point under contention, so every timing in \
             this batch is UNMEASURED -- not merely noisy.",
            before, after
        ));
    }
    Ok(())
}

/// A paused simulation that can be forked again.
pub struct Node {
    sock: UnixStream,
    /// The decoded input array, at the address every descendant shares.
    pub base: u64,
    /// The engine's own race tick at which this node stopped; `sim_ms` is its
    /// simulation time and `race_start` what that is measured from.
    pub clock: u64,
    pub pid: i32,
    boundary: Option<usize>,
    dead: bool,
    pub sim_ms: u64,
    pub race_start: u64,
}

impl Node {
    /// THE PROBE. The first tick this node has not consumed, asked of the
    /// engine by taking read access to the input array away from a throwaway
    /// child and catching the fault.
    ///
    /// Every node runs this once, for itself. Inheriting a parent's boundary is
    /// precisely the defect that produced 312 false finishes.
    pub fn probe(&mut self) -> Result<usize, String> {
        let t = self.request(&payload_probe()).and_then(|s| parse_probe(&s))?;
        self.boundary = Some(t);
        Ok(t)
    }

    /// The first tick it is safe to write, combining the probe with an optional
    /// outside estimate.
    ///
    /// `max(estimate, probe)` and never `min`: the probe is the record the
    /// engine is about to read -- the first one it is safe to write -- and
    /// calibration may only push the boundary LATER. Getting that backwards is
    /// what made 23 of 100 candidates silently wrong on a re-verification.
    pub fn floor(&self, calibrated: Option<usize>) -> Result<usize, String> {
        let p = self.boundary.ok_or_else(|| {
            "this node has not probed its own boundary -- a node without a probe is not a node"
                .to_string()
        })?;
        Ok(match calibrated {
            Some(c) => c.max(p),
            None => p,
        })
    }

    pub fn probed_boundary(&self) -> Option<usize> {
        self.boundary
    }

    /// Fork a child that runs the tape to the finish and returns the
    /// validator's JSON. The node itself is untouched and can be forked again.
    pub fn run(&mut self, from: usize, recs: &[Rec]) -> Result<String, String> {
        self.check_forward(from, recs.len())?;
        self.request(&payload_run(from, recs))
    }

    /// Fork a child that appends and becomes a node of its own.
    ///
    /// **Refuses any write BELOW this node's own probed boundary.** That
    /// refusal is the whole safety story of the forward-only regime and there
    /// is no flag to turn it off: a caller who needs to rewrite history has to
    /// re-simulate from a node that has not consumed it yet.
    pub fn branch(&mut self, req: &BranchReq) -> Result<i32, String> {
        self.check_forward(req.from, req.recs.len())?;
        let s = self.request(&payload_branch(req))?;
        crate::forksrv::parse_branched(&s)
    }

    /// **THE NEGATIVE CONTROL'S ONLY DOOR.** Run a candidate whose writes go
    /// BELOW this node's probed boundary.
    ///
    /// It exists because rung 0.5 needs both halves: the positive half (a
    /// forward-only candidate agrees with the plain oracle) is decoration on
    /// its own, since a fork server that happened to be fine for unrelated
    /// reasons passes it. The negative half has to reproduce the known wrong
    /// answer, and it cannot do that through an API that refuses it.
    ///
    /// The name is this long on purpose. There is no flag on [`Node::run`] or
    /// [`Node::branch`] that reaches this behaviour, nothing defaults to it, and
    /// one `grep` over the tree finds every caller. A search that calls it is a
    /// search that typed the words.
    pub fn run_below_boundary_for_the_negative_control(
        &mut self,
        from: usize,
        recs: &[Rec],
    ) -> Result<String, String> {
        self.request(&payload_run(from, recs))
    }

    /// The forward-only rule, enforced.
    ///
    /// An EMPTY write is exempt, and that is not a loophole: a branch that
    /// carries no inputs rewrites no record, so there is nothing that could be
    /// a silent no-op. Refusing it would make `fork` — duplicating a node
    /// without appending anything — impossible, which is how this was found.
    fn check_forward(&self, from: usize, n_recs: usize) -> Result<(), String> {
        if n_recs == 0 {
            return Ok(());
        }
        let p = self.boundary.ok_or_else(|| {
            "refusing to write into a node that has not probed its own consumed boundary"
                .to_string()
        })?;
        if from < p {
            return Err(format!(
                "FORWARD-ONLY VIOLATION: this node has already consumed every tick below {}, \
                 and a write at tick {} would be a silent no-op that scores exactly the \
                 parent's score. Branch from an ancestor that has not consumed it.",
                p, from
            ));
        }
        Ok(())
    }

    fn request(&mut self, p: &[u8]) -> Result<String, String> {
        if self.dead {
            return Err("this node has been destroyed".into());
        }
        write_frame(&mut self.sock, p).map_err(|e| format!("node {}: {}", self.pid, e))?;
        match read_frame(&mut self.sock) {
            Some(v) => Ok(String::from_utf8_lossy(&v).into_owned()),
            None => {
                self.dead = true;
                Err(format!("node {} stopped answering", self.pid))
            }
        }
    }

    /// End this node. Idempotent.
    pub fn destroy(&mut self) {
        if !self.dead && self.pid > 0 {
            unsafe { kill(self.pid, SIGKILL) };
        }
        self.dead = true;
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node with no probe refuses every write. The failure mode this guards
    /// is not a crash, it is a plausible number: a write below the boundary is
    /// a no-op and the candidate scores exactly its parent.
    #[test]
    fn a_node_without_a_probe_refuses_to_be_written_to() {
        let n = Node {
            sock: UnixStream::pair().unwrap().0,
            base: 0,
            clock: 0,
            pid: 1,
            boundary: None,
            dead: false,
            sim_ms: 0,
            race_start: 0,
        };
        let e = n.check_forward(500, 1).unwrap_err();
        assert!(e.contains("has not probed"), "{}", e);
        assert!(n.floor(None).is_err(), "a node with no probe has no floor either");
    }

    /// The forward-only refusal fires exactly BELOW the boundary. The boundary
    /// tick itself is ALLOWED: the probe names the record the engine is about
    /// to read, and nothing of it has been consumed. (It was refused until the
    /// probe stopped reporting one record early -- see
    /// `forkoracle::forksrv::STRIDE`.)
    #[test]
    fn the_forward_only_refusal_fires_below_the_boundary_and_not_at_it() {
        let n = Node {
            sock: UnixStream::pair().unwrap().0,
            base: 0,
            clock: 0,
            pid: 1,
            boundary: Some(171),
            dead: false,
            sim_ms: 0,
            race_start: 0,
        };
        for t in [0usize, 1, 169, 170] {
            let e = n.check_forward(t, 1).unwrap_err();
            assert!(e.contains("FORWARD-ONLY VIOLATION"), "tick {} was allowed: {}", t, e);
        }
        for t in [171usize, 172, 4000] {
            assert!(n.check_forward(t, 1).is_ok(), "tick {} was refused and should not be", t);
        }
    }

    /// An EMPTY macro is exempt from the refusal. This is not a loophole and it
    /// is not cosmetic: `fork` -- duplicating a node without appending anything
    /// -- is an empty branch, and refusing it made `fork` impossible. A write of
    /// no records rewrites no record, so there is nothing that could be a silent
    /// no-op. Found by the depth arm of the cost rig, at generation 2.
    #[test]
    fn an_empty_macro_is_exempt_because_it_writes_nothing() {
        let n = Node {
            sock: UnixStream::pair().unwrap().0,
            base: 0,
            clock: 0,
            pid: 1,
            boundary: Some(171),
            dead: false,
        };
        assert!(n.check_forward(0, 0).is_ok(), "an empty branch from tick 0 must be allowed");
        assert!(n.check_forward(0, 1).is_err(), "a non-empty write at tick 0 must not be");
    }

    /// Calibration may only push the boundary LATER. An earlier estimate is
    /// ignored, not honoured -- the probe is authoritative about what is
    /// already consumed.
    #[test]
    fn calibration_can_only_move_the_floor_later() {
        let n = Node {
            sock: UnixStream::pair().unwrap().0,
            base: 0,
            clock: 0,
            pid: 1,
            boundary: Some(171),
            dead: false,
            sim_ms: 0,
            race_start: 0,
        };
        assert_eq!(n.floor(None).unwrap(), 171);
        assert_eq!(n.floor(Some(100)).unwrap(), 171, "an EARLIER estimate must be ignored");
        assert_eq!(n.floor(Some(171)).unwrap(), 171);
        assert_eq!(n.floor(Some(400)).unwrap(), 400, "a LATER estimate must win");
    }
}

// ---------------------------------------------------------------- memory

/// One process's memory, as the two numbers that mean different things.
///
/// `Private_Dirty` is what a process owns alone. `Pss` — proportional set size —
/// divides each shared page among its sharers, so **summing `Pss` over every
/// process in a tree gives the tree's true unique footprint** and summing
/// `Private_Dirty` does not.
///
/// THEY ANSWER DIFFERENT QUESTIONS AND NEITHER IS "the beam cost".
///
/// `Private_Dirty` is the MARGINAL cost of one more node -- what its own ticks
/// dirtied. Summed `Pss` over the children of one engine is mostly that
/// engine.s own shared pages redistributed among its sharers: 20 children of a
/// 150 MB parent each read ~7 MB of PSS, which is 150/21, and summing them
/// recovers the parent rather than the beam.s extra cost.
///
/// MEASURED on the real engine: ~0.35 MB private per node against ~6.9 MB PSS
/// per node. Quote the first for "what does one more node cost"; quote the
/// second only beside the shared engine it is a share OF. Neither counts the
/// kernel page tables a fork of a 150 MB address space needs, which is why
/// `mem_available_mb` is worth reading as a third, system-wide view -- and on
/// this box the two disagree by 6x, so the marginal figure is a RANGE.
pub struct Mem {
    pub private_dirty_mb: f64,
    pub pss_mb: f64,
}

pub fn mem_of(pid: i32) -> Option<Mem> {
    let s = std::fs::read_to_string(format!("/proc/{}/smaps_rollup", pid)).ok()?;
    let get = |k: &str| -> Option<f64> {
        s.lines().find_map(|l| {
            l.strip_prefix(k)
                .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
                .map(|kb| kb / 1024.0)
        })
    };
    Some(Mem { private_dirty_mb: get("Private_Dirty:")?, pss_mb: get("Pss:")? })
}

/// Free memory the kernel believes is available, in MB.
///
/// A SECOND instrument on the same question, from a different source. Checks
/// that agree prove nothing if they share a source, so the beam's footprint is
/// reported both as a sum of per-process `Pss` and as the fall in
/// `MemAvailable` while the beam is alive.
pub fn mem_available_mb() -> f64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("MemAvailable:")
                    .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<f64>().ok())
                    .map(|kb| kb / 1024.0)
            })
        })
        .unwrap_or(f64::NAN)
}
