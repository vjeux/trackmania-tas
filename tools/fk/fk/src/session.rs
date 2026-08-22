//! Starting a fork server, and the four things every command needs to say.
//!
//! # The god-struct this replaces
//!
//! Every one of `fk`'s ~45 subcommands used to take the same `Cfg`: twenty-four
//! fields (`template map server work shim csv tick ckpt tol mode out every n
//! addr span len tape difftick obs obstag steerdiv diffmag nth`) parsed by one
//! function, of which any given command read four or five. `--obstag` was
//! accepted by the memory-poker; `--difftick` by the tape cutter. A flag being
//! accepted said nothing about whether it did anything, and the only way to
//! find out was to read the command's body.
//!
//! What is actually shared is small and it is all here: which engine, which
//! map, which tape, where to work, and where to stop. Everything else is an
//! argument to one command and is parsed by that command.

use crate::tape::Tape;
use forkoracle::forksrv::{write_key, ForkServer};
use std::path::{Path, PathBuf};

/// Where the engine is and where to work. Five fields, and every command needs
/// all five.
#[derive(Clone, Debug)]
pub struct Engine {
    /// The dedicated-server install (the directory holding `TrackmaniaServer`).
    pub server: PathBuf,
    /// The map to put in `UserData/Maps`.
    ///
    /// For a `.Replay.Gbx` this is **decoration**: the replay carries the whole
    /// map in chunk `0x03093002` and the server simulates that copy. Proven by
    /// validating with an empty Maps directory and getting the identical time.
    /// `ghost map show FILE` says which case a file is.
    pub map: PathBuf,
    /// The `LD_PRELOAD` shim that counts `lroundf` and services the fork
    /// protocol.
    pub shim: PathBuf,
    /// Scratch root. **Per process by default.** A shared one is the single
    /// root cause behind four separate silent-corruption defects in this
    /// project: two runs pointed at one directory swap replays, and the loser
    /// measures another tape's prefix with its own tail patched in — a real,
    /// self-consistent trajectory of a car that drove somewhere else, which
    /// every internal check passes. The directory lock refuses sharing rather
    /// than racing.
    pub work: PathBuf,
    /// Delete `work` on exit. Off when the caller asked for a specific `--work`,
    /// because then they want to look at it.
    pub work_is_temporary: bool,
}

impl Engine {
    pub fn default_work() -> PathBuf {
        PathBuf::from(format!("/tmp/fk/work-{}", std::process::id()))
    }

    /// Fail now, with the reason, rather than inside a forked child where the
    /// only symptom is a bare `ERR`.
    pub fn check(&self) -> Result<(), String> {
        for (what, p) in [
            ("dedicated server", self.server.join("TrackmaniaServer")),
            ("shim", self.shim.clone()),
            ("map", self.map.clone()),
        ] {
            if !p.exists() {
                return Err(format!("no {} at {}", what, p.display()));
            }
        }
        Ok(())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if self.work_is_temporary {
            let _ = std::fs::remove_dir_all(&self.work);
        }
    }
}

/// Where to stop the simulation.
///
/// The engine has no notion of "tick 2323"; it has an `lroundf` call count. The
/// two are related by a line fitted on three segment maps, and the fit is only
/// ever used to CHOOSE a checkpoint — never to label anything, because the
/// count is not a fixed simulation point (see [`Session::probe_tick`]).
#[derive(Clone, Copy, Debug)]
pub enum Checkpoint {
    /// A raw `lroundf` call count.
    Clock(u64),
    /// A tape tick, converted through the fitted line.
    Tick(i64),
    /// A fraction of the tape's own length.
    Fraction(f64),
}

/// `clock = 36141 + 25.483 * race_ms`, fitted on three segment maps.
///
/// Linear in SIMULATED time, so it transfers between maps. It is an estimate:
/// use [`Session::probe_tick`] to learn where the server actually stopped.
pub fn clock_for_race_ms(ms: i64) -> u64 {
    (36141.0 + 25.483 * ms as f64).max(1000.0) as u64
}

impl Checkpoint {
    pub fn to_clock(self, tape: &Tape) -> u64 {
        match self {
            Checkpoint::Clock(c) => c,
            Checkpoint::Tick(t) => clock_for_race_ms(t * 10 + tape.start_offset_ms as i64),
            Checkpoint::Fraction(f) => {
                let last = tape.race_ms(tape.n().saturating_sub(1));
                clock_for_race_ms((last as f64 * f) as i64)
            }
        }
    }
}

/// A started fork server, plus the tape it is running.
///
/// Holding the two together is not tidiness. The one question no measurement of
/// a simulated trajectory should be trusted without is *is the simulator running
/// the tape I asked about*, and answering it needs both.
pub struct Session {
    pub srv: ForkServer,
    pub tape: Tape,
    pub checkpoint_clock: u64,
}

impl Session {
    /// Start a server, stop it at `at`, and prove it is running `tape`.
    ///
    /// The identity check is not optional and there is no flag to skip it. It
    /// costs one 70 KB read of `/proc/<pid>/mem` and it is the only thing that
    /// can see a swapped replay: two runs sharing a work directory produce a
    /// genuine, self-consistent trajectory of the wrong car, and no internal
    /// consistency test can tell.
    pub fn start(engine: &Engine, tape: Tape, at: Checkpoint) -> Result<Session, String> {
        engine.check()?;
        std::fs::create_dir_all(&engine.work).map_err(|e| e.to_string())?;
        let clock = at.to_clock(&tape);
        let refp = engine.work.join("reference.Ghost.Gbx");
        tape.write_reference(&refp)?;
        let key = engine.work.join("key.bin");
        write_key(&key, &tape.steer);
        let srv = ForkServer::start(
            &engine.work.join("srv"),
            &engine.server,
            &engine.map,
            &refp,
            &key,
            &engine.shim,
            clock,
        )?;
        let s = Session { srv, tape, checkpoint_clock: clock };
        s.assert_running_our_tape()?;
        Ok(s)
    }

    /// THE IDENTITY CONTROL. See [`forkoracle::layout::verify_tape`].
    pub fn assert_running_our_tape(&self) -> Result<(), String> {
        forkoracle::layout::verify_tape(
            self.srv.pid(),
            self.srv.base,
            &self.tape.steer,
            &self.tape.accel,
            &self.tape.brake,
        )
    }

    /// The first tick the simulation has NOT yet consumed, asked of the engine
    /// itself.
    ///
    /// Everything downstream depends on this being right, and every cheaper way
    /// of getting it is wrong. Clock- or behaviour-derived estimates were wrong
    /// on 2 of 30. The engine is asked instead: `mprotect` every page the input
    /// array touches to `PROT_NONE` in a throwaway child, catch `SIGSEGV`, and
    /// read `si_addr` — the record it was about to read.
    ///
    /// **A failed probe is a hard abort, never a fallback.** A resume that
    /// rewrites an already-consumed record is a silent no-op that scores exactly
    /// the incumbent's score, so `delta == 0` is accepted and that lineage is
    /// contaminated for free.
    pub fn probe_tick(&mut self) -> Result<usize, String> {
        self.srv.probe_tick()
    }
}

// ---------------------------------------------------------------------------
// The recorder's view of the same five things.
//
// `fk regen` runs the ORIGINAL FILE, never a re-encode of it: its whole job is
// to rewrite a ghost's telemetry while leaving the tape exactly as stored, and
// staging a re-encoded reference would put a second encoder in the path of the
// one operation that must not touch the inputs. So it needs a server started on
// a file rather than on a `Tape`, and `Ctx` is the flat form of `Engine` that
// the recorder threads through its ladder of checkpoints.

#[derive(Clone, Debug)]
pub struct Ctx {
    pub template: String,
    pub map: String,
    pub server: String,
    pub work: String,
    pub shim: String,
    /// 0 = "use the ladder", any other value pins the checkpoint.
    pub ckpt: u64,
}

impl Ctx {
    pub fn from_engine(e: &Engine, template: &str, ckpt: u64) -> Ctx {
        Ctx {
            template: template.to_string(),
            map: e.map.to_string_lossy().into(),
            server: e.server.to_string_lossy().into(),
            work: e.work.to_string_lossy().into(),
            shim: e.shim.to_string_lossy().into(),
            ckpt,
        }
    }
}

/// `clock = 36141 + 25.483 * race_ms`, addressed by tape tick.
pub fn clock_for_tick(tick: i64, start_offset_ms: i32) -> u64 {
    clock_for_race_ms(tick * 10 + start_offset_ms as i64)
}

/// Start a fork server that runs `ghost` verbatim.
///
/// The shim is keyed on the tape's steer sequence so it can find the decoded
/// input array in the engine's heap; the key comes from the same tape the file
/// carries, so a mismatch here means the file's inputs are not what was
/// decoded.
pub fn start_server_on_file(
    c: &Ctx,
    tape: &crate::tape::Tape,
    work: &Path,
    ckpt: u64,
    ghost: &Path,
) -> Result<ForkServer, String> {
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    crate::tape::check_oracle_readable_name(ghost)?;
    let key = work.join("key.bin");
    write_key(&key, &tape.steer);
    ForkServer::start(
        &work.join("srv"),
        Path::new(&c.server),
        Path::new(&c.map),
        ghost,
        &key,
        Path::new(&c.shim),
        ckpt,
    )
}

/// One input record per tape tick from `from` to the end.
pub fn tail_recs(
    steer: &[u8],
    accel: &[u8],
    brake: &[u8],
    from: usize,
) -> Vec<forkoracle::forksrv::Rec> {
    (from..steer.len())
        .map(|t| forkoracle::forksrv::rec_of(steer[t], accel[t], brake[t]))
        .collect()
}
