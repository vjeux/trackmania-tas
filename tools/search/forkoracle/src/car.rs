//! THE CAR, BY DERIVATION: the rigid body the physics step integrates, reached
//! by the pointers the engine itself follows to get there. No scan, no fork, no
//! candidate, no heuristic -- and nothing to choose between.
//!
//! # What the physics step does, per tick (build 128182, `LOCATE.md`)
//!
//! The validator's tick loop (`0x1218db0`) calls the playground update
//! `0x119f1b0(playground, new_time, dt)` once per tick. Inside it, for the
//! vehicle-physics manager `vehmgr` it fetches from the playground's scene:
//!
//! 1. `0x11a14da: call 0xa53ce0(vehmgr)` -- the SOLVER. It integrates the
//!    dyna body record of every vehicle with a body: the record's position is
//!    written twice (`0xa549f7` predict, `0xa54ba4` correct) and its velocity
//!    once (`0xa54c1e`), measured with hardware watchpoints.
//! 2. `0x11a17ac: call 0x9cd8c0(vehmgr)` -- the COPY-OUT. For each vehicle it
//!    does not skip (`u32[phy+0x128c] & 0xf != 2` and `u32[phy+0x1c90]` in
//!    {0, 3}), if the body handle `u32[phy+0x10]` is not `0xffffffff` it looks
//!    the record up (`0x934980`: `[dyna+0x70] + 0x58 * u32[[dyna+0xb0] +
//!    4*handle]`) and copies quaternion, position, velocity and angular
//!    velocity into the `CGameVehiclePhy` at `+0x12e0/+0x12f0/+0x12fc/+0x1308`.
//!
//! Before the solver the step also refreshes the vis state at `phy+0x4e8` from
//! the pre-step body (a 1 mm-quantised copy, so it lags one tick), and after
//! the copy-out it refreshes the one at `phy+0x848` from the post-step body.
//! Everything else that looks like the car in memory -- the participant's copy
//! at `+0xe24` that the old sweep used to pick, the vis states, the `Iso4` at
//! `phy+0x27c` -- is a copy made by other code at another instant, and that is
//! the whole story of the one-tick phase differences the sweep kept running
//! into: they were properties of which copy it had picked, never of the map.
//!
//! # The path
//!
//! ```text
//! controller (captured at 0x118c170)  +0x1a70 -> sim        (== captured rcx)
//! sim         +0x18  -> playground                             sim+0x48 = the tick loop's clock
//! playground  +0x660 -> players, +0x668 count == 1 -> participant
//! playground  +0x7c8 -> scene;  vehmgr = [scene + 0x10 + 8 * u32[mod + 0x1cc18b0]]
//! vehmgr      +0x110 -> dyna;   +0xb0/+0xb8 = the vehicle array the step iterates
//! participant +0x1118/+0x1128/+0x1138/+0x1148 -> the four CGameVehiclePhy slots
//!             the DRIVEN one is the one the step does not skip; it has a body
//!             (u32[phy+0x10] != 0xffffffff) except inside a respawn window
//! body        = [dyna+0x70] + 0x58 * u32[[dyna+0xb0] + 4*handle]
//!             +0x00 3x3 rotation, +0x24 position, +0x30 quaternion (w,x,y,z),
//!             +0x40 velocity, +0x4c angular velocity                (88 bytes)
//! ```
//!
//! Every hop is checked as it is taken, and the walk is cross-checked against
//! the engine at no cost: the phy must be one of the vehicles `vehmgr` iterates
//! (so the global index really named the vehicle manager), and, when a body
//! exists, the phy's copy-out must be byte-identical to it (so the record the
//! lookup named is the one the step copied from, one tick ago). Any failure is
//! an error with the hop's name in it. There is no fallback to a search.
//!
//! # Respawns
//!
//! A respawn REMOVES the body (`0xe873b1` writes `0xffffffff` into `phy+0x10`)
//! and re-creates it 101 ticks later (`0x9c9ba9`), with the same handle and the
//! same record: 31 respawns on the 440 s ghost of 284238 never moved the record
//! and never changed the handle. Inside the window there is no body to
//! integrate; the phy already holds the checkpoint's saved pose. So the
//! locator names the car with or without a body, and the SAMPLER reads the
//! copy-out -- defined at every tick, bit-identical to the record at every tick
//! boundary where a body exists (`fk locate check` verifies that identity tick
//! by tick), and the object the game layer itself reads.
//!
//! # What "one tick" means here
//!
//! The fork server stops in the tick hook, at the START of the tick whose
//! `new_time` is `sim_ms`, so memory holds the state at the END of the tick
//! before it -- the one stamped `[sim+0x48] == sim_ms - 10`. Its race time is
//! `[sim+0x48] - race_start`, both read from the engine. That is the `Layout`:
//! clock word `sim+0x48`, bias `race_start`, and nothing measured.

use crate::forksrv::ForkServer;
use crate::layout::Layout;
use crate::procmem;

/// Where the build keeps things. One place, so `LOCATE.md`, the tests and the
/// code cannot drift apart; `fk locate check` verifies every one of them on a
/// live server.
pub mod build128182 {
    /// `[rdi+0x1a70] = rcx` in the validation callback `0x118c170`.
    pub const SIM_IN_CONTROLLER: u64 = 0x1a70;
    /// `0x1218e3d: mov rax,[r14+0x18]` -- the playground, per tick.
    pub const PLAYGROUND_IN_SIM: u64 = 0x18;
    /// `0x1219750: mov [r15+0x48], ebx` -- the tick loop's own clock write.
    pub const TIME_IN_SIM: u64 = 0x48;
    /// `0x1218e41/0x1218e4e` and `0x119f5a5/0x119f5b8` -- the player array.
    pub const PLAYERS_IN_PLAYGROUND: u64 = 0x660;
    pub const NPLAYERS_IN_PLAYGROUND: u64 = 0x668;
    /// `0x119f1fa: mov r15,[rdi+0x7c8]` -- the scene the step fetches its
    /// managers from.
    pub const SCENE_IN_PLAYGROUND: u64 = 0x7c8;
    /// `0x9c3714: mov eax,[rip+..] # 1cc18b0` -- the vehicle-physics
    /// manager's registration index, a process global set at start-up.
    pub const VEHMGR_INDEX_GLOBAL: u64 = 0x1cc18b0;
    /// `0xa47c96: mov rax,[rdi+rax*8+0x10]` -- manager `idx` of a scene.
    pub const MANAGERS_IN_SCENE: u64 = 0x10;
    /// `0x9cd8e1/0x9cd8e8` -- the vehicles the copy-out iterates.
    pub const VEHICLES_IN_VEHMGR: u64 = 0xb0;
    pub const NVEHICLES_IN_VEHMGR: u64 = 0xb8;
    /// `0x9cdbd5: mov rdi,[rax+0x110]` -- the dyna world of the manager.
    pub const DYNA_IN_VEHMGR: u64 = 0x110;
    /// `0x934984..0x934994`: `[dyna+0x70] + 0x58 * u32[[dyna+0xb0] + 4*h]`.
    pub const BODIES_IN_DYNA: u64 = 0x70;
    pub const REMAP_IN_DYNA: u64 = 0xb0;
    pub const BODY_STRIDE: u64 = 0x58;
    /// The record the solver integrates (`0xa549f7`, `0xa54ba4`, `0xa54c1e`).
    pub const ROT_IN_BODY: u64 = 0x00;
    pub const POS_IN_BODY: u64 = 0x24;
    pub const QUAT_IN_BODY: u64 = 0x30;
    pub const VEL_IN_BODY: u64 = 0x40;
    pub const ANGVEL_IN_BODY: u64 = 0x4c;
    /// `0x11a9b16..0x11a9b21` stores the class id and the pointer.
    pub const CLASS_IN_PARTICIPANT: u64 = 0x1110;
    pub const CGAME_VEHICLE_PHY: u32 = 0x032e_2000;
    /// Stadium, Snow, Rally, Desert (tm-player INPUT arm, `WHEELS.md`).
    pub const VEHICLE_SLOTS: [u64; 4] = [0x1118, 0x1128, 0x1138, 0x1148];
    /// `0x9cdaf1: mov eax,[r13+0x128c]; and eax,0xf; cmp eax,2; je skip` --
    /// a parked vehicle reads 2 here and the step never touches it.
    pub const STATE_IN_PHY: u64 = 0x128c;
    pub const STATE_PARKED: u32 = 2;
    /// `0x9cdb00: mov eax,[r13+0x1c90]; cmp eax,3 / test eax,eax` -- the
    /// step processes a vehicle in mode 0 or 3 only.
    pub const MODE_IN_PHY: u64 = 0x1c90;
    /// `0x9cdb10: mov r15d,[r13+0x10]` -- the body handle the copy-out looks
    /// up; `0xffffffff` (`0x9cdbcb`) means this vehicle has no body right
    /// now: parked, or inside a respawn window.
    pub const BODY_HANDLE_IN_PHY: u64 = 0x10;
    pub const NO_BODY: u32 = 0xffff_ffff;
    /// `0x9cdc27`: the simulation time the body comes back at after a
    /// respawn; `-1` until the first respawn. Informational.
    pub const RESPAWN_TIME_IN_PHY: u64 = 0x12dc;
    /// The copy-out's destinations (`0x9cdbe8..0x9cdc1e`).
    pub const QUAT_IN_PHY: u64 = 0x12e0;
    pub const POS_IN_PHY: u64 = 0x12f0;
    pub const VEL_IN_PHY: u64 = 0x12fc;
    pub const ANGVEL_IN_PHY: u64 = 0x1308;
    /// The two `CSceneVehicleVisState`s (0x360 bytes each) inside the phy:
    /// refreshed from the body BEFORE the solver (`+0x4e8`, so one tick
    /// behind) and AFTER the copy-out (`+0x848`). Both quantise to 1 mm.
    pub const VIS_PRE_IN_PHY: u64 = 0x4e8;
    pub const VIS_POST_IN_PHY: u64 = 0x848;
    /// `Loc.translation` and `WetnessValue01`, from the engine's reflection
    /// of the class (`VEHICLEVISSTATE.md`).
    pub const POS_IN_VIS: u64 = 0x50;
    pub const WETNESS_IN_VIS: u64 = 0x328;
    /// The copy the old blind sweep picked on map 2: quaternion, position,
    /// velocity of the PREVIOUS tick, inside the participant. Named so the
    /// census can label it; never read as the car.
    pub const STATE_COPY_IN_PARTICIPANT: u64 = 0xe24;
}

use build128182::*;

/// The dyna body of the car, while it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Body {
    /// `u32[phy+0x10]`.
    pub handle: u32,
    /// `u32[[dyna+0xb0] + 4*handle]`, the record's index.
    pub index: u32,
    /// The 88-byte record: THE state the solver reads and writes.
    pub addr: u64,
}

/// The car, and every object on the way to it.
#[derive(Clone, Debug)]
pub struct Car {
    pub controller: u64,
    pub sim: u64,
    pub playground: u64,
    pub participant: u64,
    pub scene: u64,
    pub vehmgr: u64,
    pub dyna: u64,
    /// Which of the four vehicle slots holds the car (0 Stadium, 1 Snow,
    /// 2 Rally, 3 Desert).
    pub slot: usize,
    /// All four slot pointers, driven or parked (the finish record lives in
    /// one of them and it is not always the driven one -- `finish.rs`).
    pub vehicles: Vec<u64>,
    /// The driven `CGameVehiclePhy`.
    pub phy: u64,
    /// Its dyna body; `None` inside a respawn window.
    pub body: Option<Body>,
    /// The tick loop's clock, `sim+0x48`.
    pub sim_time: u64,
    /// The race start in simulation ms, as the engine set it in this process.
    pub race_start: u64,
}

impl Car {
    /// The copy-out inside the phy: equal to the body at every tick boundary
    /// where a body exists, and the frozen checkpoint pose where none does.
    pub fn pos(&self) -> u64 {
        self.phy + POS_IN_PHY
    }
    pub fn quat(&self) -> u64 {
        self.phy + QUAT_IN_PHY
    }
    pub fn vel(&self) -> u64 {
        self.phy + VEL_IN_PHY
    }
    pub fn angvel(&self) -> u64 {
        self.phy + ANGVEL_IN_PHY
    }
    /// The solver's own record, when the car has a body.
    pub fn body_pos(&self) -> Option<u64> {
        self.body.map(|b| b.addr + POS_IN_BODY)
    }
    pub fn body_quat(&self) -> Option<u64> {
        self.body.map(|b| b.addr + QUAT_IN_BODY)
    }
    pub fn body_vel(&self) -> Option<u64> {
        self.body.map(|b| b.addr + VEL_IN_BODY)
    }
    pub fn body_angvel(&self) -> Option<u64> {
        self.body.map(|b| b.addr + ANGVEL_IN_BODY)
    }
    pub fn body_rot(&self) -> Option<u64> {
        self.body.map(|b| b.addr + ROT_IN_BODY)
    }
    /// The post-step vis state (1 mm quantised; where the wheel, gear, rpm and
    /// wetness fields live).
    pub fn vis(&self) -> u64 {
        self.phy + VIS_POST_IN_PHY
    }
    /// The pre-step vis state (one tick behind).
    pub fn vis_pre(&self) -> u64 {
        self.phy + VIS_PRE_IN_PHY
    }
    pub fn wetness(&self) -> u64 {
        self.vis() + WETNESS_IN_VIS
    }
    /// The participant's previous-tick copy -- a decoy, kept for the census.
    pub fn participant_copy_pos(&self) -> u64 {
        self.participant + STATE_COPY_IN_PARTICIPANT
    }

    /// What the sampler gathers: the copy-out, stamped with the tick loop's
    /// own clock, labelled from the engine's own race start.
    pub fn layout(&self) -> Layout {
        Layout {
            pos: self.pos(),
            quat: self.quat(),
            vel: self.vel(),
            wet: self.wetness(),
            clock: self.sim_time,
            clock_bias: self.race_start as i64,
            rms: 0.0,
            max_dev: 0.0,
            cps: 0,
            vis: 0,
            car: self.slot as u8,
        }
    }

    /// [`Car::layout`] plus the two engine words the environment gathers per
    /// tick: the checkpoint counter (`participant + 0xc70`) and the driven
    /// vehicle's post-step vis state (`phy + 0x848`, 0x360 bytes: gear, rpm,
    /// wheels, turbo). Seven segments; the fk tools keep the five-segment base.
    pub fn layout_with_engine(&self) -> Layout {
        Layout { cps: self.participant + CP_COUNT_IN_PARTICIPANT, vis: self.vis(), ..self.layout() }
    }
}

impl std::fmt::Display for Car {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "controller {:#x} -> sim {:#x} -> playground {:#x} -> participant {:#x}; scene {:#x} -> \
             vehmgr {:#x} -> dyna {:#x}; slot {} phy {:#x} (pos {:#x}); {}; clock sim+0x48 {:#x}, \
             race start {}",
            self.controller,
            self.sim,
            self.playground,
            self.participant,
            self.scene,
            self.vehmgr,
            self.dyna,
            self.slot,
            self.phy,
            self.pos(),
            match self.body {
                Some(b) => format!("body handle {} -> record[{}] {:#x}", b.handle, b.index, b.addr),
                None => "no body (respawn window)".to_string(),
            },
            self.sim_time,
            self.race_start
        )
    }
}

/// The main module's load address, from `/proc/<pid>/maps`.
pub fn module_base(pid: i32) -> Option<u64> {
    procmem::maps(pid)
        .into_iter()
        .filter(|r| r.path.ends_with("TrackmaniaServer"))
        .map(|r| r.start)
        .min()
}

/// Locate the car in a stopped fork server. Thirty-odd small reads of
/// `/proc/<pid>/mem`, no simulation, no fork; well under a millisecond.
pub fn locate(srv: &ForkServer) -> Result<Car, String> {
    let pid = srv.pid();
    let module = module_base(pid).ok_or("cannot find the server module in /proc/<pid>/maps")?;
    resolve_with(
        srv.validator_controller,
        srv.validation_sim,
        module,
        srv.sim_ms,
        srv.race_start,
        |a, n| procmem::read_at(pid, a, n),
    )
}

fn word<const N: usize>(
    read: &mut impl FnMut(u64, usize) -> Option<Vec<u8>>,
    at: u64,
    what: &str,
) -> Result<[u8; N], String> {
    let b = read(at, N).ok_or_else(|| format!("{}: cannot read {} bytes at {:#x}", what, N, at))?;
    b.as_slice()
        .try_into()
        .map_err(|_| format!("{}: short read at {:#x} ({} of {} bytes)", what, at, b.len(), N))
}

fn ptr(
    read: &mut impl FnMut(u64, usize) -> Option<Vec<u8>>,
    at: u64,
    what: &str,
) -> Result<u64, String> {
    let v = u64::from_le_bytes(word::<8>(read, at, what)?);
    if v < 0x1000 {
        return Err(format!("{}: null pointer at {:#x} ({:#x})", what, at, v));
    }
    Ok(v)
}

fn u32_at(
    read: &mut impl FnMut(u64, usize) -> Option<Vec<u8>>,
    at: u64,
    what: &str,
) -> Result<u32, String> {
    Ok(u32::from_le_bytes(word::<4>(read, at, what)?))
}

/// The walk itself, over any memory reader, so the tests can hand it an image.
///
/// `sim_ms` is the tick the stopped server is about to run (the hook's
/// `new_time`); `race_start` is the engine's, from the handshake. Both are
/// only used for the clock cross-check and the label.
pub fn resolve_with(
    controller: u64,
    captured_sim: u64,
    module: u64,
    sim_ms: u64,
    race_start: u64,
    mut read: impl FnMut(u64, usize) -> Option<Vec<u8>>,
) -> Result<Car, String> {
    if controller < 0x1000 || captured_sim < 0x1000 {
        return Err("this server did not capture the validator's callback (no controller/sim)".into());
    }
    // 1. the validation: the captured argument must agree with the object's
    //    own field, or the capture is not describing this simulation.
    let sim = ptr(&mut read, controller + SIM_IN_CONTROLLER, "controller.sim")?;
    if sim != captured_sim {
        return Err(format!(
            "the captured simulation {:#x} is not the one the controller holds ({:#x})",
            captured_sim, sim
        ));
    }
    // 2. the clock the tick loop writes: at a stop in the hook it must read
    //    exactly one tick behind the tick being entered.
    let sim_time = sim + TIME_IN_SIM;
    let t = u32_at(&mut read, sim_time, "sim.time")? as u64;
    if sim_ms != 0 && t + 10 != sim_ms {
        return Err(format!(
            "[sim+0x48] = {} but the hook stopped entering tick {} -- not stopped at a tick boundary",
            t, sim_ms
        ));
    }
    // 3. the playground and its one participant.
    let playground = ptr(&mut read, sim + PLAYGROUND_IN_SIM, "sim.playground")?;
    let n = u32_at(&mut read, playground + NPLAYERS_IN_PLAYGROUND, "playground.nplayers")?;
    if n != 1 {
        return Err(format!("{} participants; a validation has exactly 1", n));
    }
    let players = ptr(&mut read, playground + PLAYERS_IN_PLAYGROUND, "playground.players")?;
    let participant = ptr(&mut read, players, "players[0]")?;
    let class = u32_at(&mut read, participant + CLASS_IN_PARTICIPANT, "participant.vehicle_class")?;
    if class != CGAME_VEHICLE_PHY {
        return Err(format!(
            "participant's vehicle class id is {:#x}, not CGameVehiclePhy ({:#x})",
            class, CGAME_VEHICLE_PHY
        ));
    }
    // 4. the vehicle-physics manager, exactly as 0x119f1b0 fetches it.
    let scene = ptr(&mut read, playground + SCENE_IN_PLAYGROUND, "playground.scene")?;
    let idx = u32_at(&mut read, module + VEHMGR_INDEX_GLOBAL, "vehmgr registration index")?;
    if idx > 1024 {
        return Err(format!("vehicle-manager index {} is not a registration index", idx));
    }
    let vehmgr = ptr(&mut read, scene + MANAGERS_IN_SCENE + 8 * idx as u64, "scene.managers[idx]")?;
    let nveh = u32_at(&mut read, vehmgr + NVEHICLES_IN_VEHMGR, "vehmgr.nvehicles")?;
    if nveh == 0 || nveh > 64 {
        return Err(format!(
            "manager {} at {:#x} holds {} vehicles -- not the vehicle-physics manager",
            idx, vehmgr, nveh
        ));
    }
    let varr = ptr(&mut read, vehmgr + VEHICLES_IN_VEHMGR, "vehmgr.vehicles")?;
    let mut managed: Vec<u64> = Vec::with_capacity(nveh as usize);
    for i in 0..nveh as u64 {
        managed.push(ptr(&mut read, varr + 8 * i, "vehmgr.vehicles[i]")?);
    }
    let dyna = ptr(&mut read, vehmgr + DYNA_IN_VEHMGR, "vehmgr.dyna")?;
    // 5. WHICH SLOT, by the step's own tests. A parked vehicle is skipped by
    //    the copy-out (`state & 0xf == 2`); the driven one is processed, and
    //    has a body except inside a respawn window. `0xffffffff` is the value
    //    the engine chose for "no body" (0x9cdbcb), so it is demanded exactly.
    let mut vehicles = Vec::with_capacity(4);
    let mut processed: Vec<(usize, u64, u32)> = Vec::new();
    for (k, off) in VEHICLE_SLOTS.iter().enumerate() {
        let Ok(v) = ptr(&mut read, participant + off, "participant.vehicle[k]") else {
            continue;
        };
        vehicles.push(v);
        let state = u32_at(&mut read, v + STATE_IN_PHY, "phy.state")?;
        let mode = u32_at(&mut read, v + MODE_IN_PHY, "phy.mode")?;
        let h = u32_at(&mut read, v + BODY_HANDLE_IN_PHY, "phy.body_handle")?;
        if state & 0xf != STATE_PARKED && (mode == 0 || mode == 3) {
            processed.push((k, v, h));
        }
    }
    let with_body: Vec<&(usize, u64, u32)> = processed.iter().filter(|(_, _, h)| *h != NO_BODY).collect();
    let (slot, phy, handle) = match (with_body.len(), processed.len()) {
        (1, _) => *with_body[0],
        (0, 1) => processed[0],
        (0, 0) => return Err("the step processes none of the four vehicle slots -- no car".into()),
        (0, n) => {
            return Err(format!(
                "{} vehicle slots are processed and none has a body (a respawn window on a \
                 transform map?) -- cannot tell which is driven",
                n
            ))
        }
        (n, _) => return Err(format!("{} vehicle slots have a dyna body; exactly one drives", n)),
    };
    if !managed.contains(&phy) {
        return Err(format!(
            "the participant's vehicle {:#x} is not among the {} vehicles manager {} iterates -- \
             the manager index does not name the vehicle-physics manager on this build",
            phy, nveh, idx
        ));
    }
    // 6. the body record, exactly as 0x934980 looks it up -- when there is one.
    let body = if handle == NO_BODY {
        None
    } else {
        if handle > 4096 {
            return Err(format!("body handle {} is not an index", handle));
        }
        let remap = ptr(&mut read, dyna + REMAP_IN_DYNA, "dyna.remap")?;
        let index = u32_at(&mut read, remap + 4 * handle as u64, "dyna.remap[handle]")?;
        if index > 4096 {
            return Err(format!("remap[{}] = {} is not a record index", handle, index));
        }
        let bodies = ptr(&mut read, dyna + BODIES_IN_DYNA, "dyna.bodies")?;
        let addr = bodies + BODY_STRIDE * index as u64;
        // 7. THE CROSS-CHECK THAT COSTS NOTHING: the copy-out the step made
        //    from this record one tick ago must still be byte-identical to it.
        //    If the lookup named the wrong record, or the phy is not the
        //    vehicle this body belongs to, they differ in the first tick the
        //    car moves.
        let rec = word::<{ BODY_STRIDE as usize }>(&mut read, addr, "dyna body record")?;
        let copy = word::<0x34>(&mut read, phy + QUAT_IN_PHY, "phy copy-out")?;
        let same = rec[0x30..0x40] == copy[0x00..0x10]
            && rec[0x24..0x30] == copy[0x10..0x1c]
            && rec[0x40..0x4c] == copy[0x1c..0x28]
            && rec[0x4c..0x58] == copy[0x28..0x34];
        if !same {
            return Err(format!(
                "body record {:#x} (index {} for handle {}) is not what the step copied into phy \
                 {:#x}: not the same body",
                addr, index, handle, phy
            ));
        }
        Some(Body { handle, index, addr })
    };
    // 8. the state itself must be a state.
    let copy = word::<0x34>(&mut read, phy + QUAT_IN_PHY, "phy copy-out")?;
    let f = |o: usize| f32::from_le_bytes(copy[o..o + 4].try_into().unwrap());
    if !(0..0x34).step_by(4).all(|o| f(o).is_finite()) {
        return Err(format!("phy {:#x} holds non-finite state", phy));
    }
    let qn = (f(0).powi(2) + f(4).powi(2) + f(8).powi(2) + f(12).powi(2)).sqrt();
    if (qn - 1.0).abs() > 1e-3 {
        return Err(format!("phy {:#x}: |q| = {} -- not an attitude", phy, qn));
    }
    Ok(Car {
        controller,
        sim,
        playground,
        participant,
        scene,
        vehmgr,
        dyna,
        slot,
        vehicles,
        phy,
        body,
        sim_time,
        race_start,
    })
}

/// The checkpoint counter, in the participant: a u32 that increments exactly
/// at the ghosts' split ticks, the finish included, on every server.
///
/// Located behaviourally by the tm-player project's ENV arm and verified
/// against the plain oracle 200/200 and 1288/1289 over 2453 tapes; the finish
/// hunt in `TICKHOOK.md` §10 found the same word independently.
pub const CP_COUNT_IN_PARTICIPANT: u64 = 0xc70;

pub fn read_xyz(pid: i32, at: u64) -> Option<[f32; 3]> {
    let b = procmem::read_at(pid, at, 12)?;
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    Some([f(0), f(4), f(8)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A memory image with every object on the path, laid out exactly as the
    /// build does it.
    struct Image {
        m: BTreeMap<u64, Vec<u8>>,
    }
    const CONTROLLER: u64 = 0x10000;
    const SIM: u64 = 0x20000;
    const PLAYGROUND: u64 = 0x30000;
    const PLAYERS: u64 = 0x40000;
    const PARTICIPANT: u64 = 0x50000;
    const PHY: [u64; 4] = [0x60000, 0x62000, 0x64000, 0x66000];
    const SCENE: u64 = 0x70000;
    const VEHMGR: u64 = 0x80000;
    const VARR: u64 = 0x81000;
    const DYNA: u64 = 0x90000;
    const REMAP: u64 = 0x91000;
    const BODIES: u64 = 0x92000;
    const MODULE: u64 = 0x5000_0000;
    const IDX: u32 = 11;
    const HANDLE: u32 = 0;
    const INDEX: u32 = 0;
    const SIM_MS: u64 = 12620;
    const RACE_START: u64 = 2200;

    impl Image {
        fn put(&mut self, at: u64, b: &[u8]) {
            self.m.insert(at, b.to_vec());
        }
        fn p64(&mut self, at: u64, v: u64) {
            self.put(at, &v.to_le_bytes());
        }
        fn p32(&mut self, at: u64, v: u32) {
            self.put(at, &v.to_le_bytes());
        }
        fn read(&self, at: u64, n: usize) -> Option<Vec<u8>> {
            // byte-addressable over the sparse map
            let mut out = Vec::with_capacity(n);
            for a in at..at + n as u64 {
                let (base, blob) = self.m.range(..=a).next_back()?;
                let o = (a - base) as usize;
                out.push(*blob.get(o)?);
            }
            Some(out)
        }
    }

    fn body_bytes(pos: [f32; 3], q: [f32; 4], v: [f32; 3], w: [f32; 3]) -> Vec<u8> {
        let mut b = vec![0u8; 0x58];
        for (i, x) in pos.iter().enumerate() {
            b[0x24 + 4 * i..0x28 + 4 * i].copy_from_slice(&x.to_le_bytes());
        }
        for (i, x) in q.iter().enumerate() {
            b[0x30 + 4 * i..0x34 + 4 * i].copy_from_slice(&x.to_le_bytes());
        }
        for (i, x) in v.iter().enumerate() {
            b[0x40 + 4 * i..0x44 + 4 * i].copy_from_slice(&x.to_le_bytes());
        }
        for (i, x) in w.iter().enumerate() {
            b[0x4c + 4 * i..0x50 + 4 * i].copy_from_slice(&x.to_le_bytes());
        }
        b
    }

    fn copy_of(rec: &[u8]) -> Vec<u8> {
        let mut copy = Vec::new();
        copy.extend_from_slice(&rec[0x30..0x40]);
        copy.extend_from_slice(&rec[0x24..0x30]);
        copy.extend_from_slice(&rec[0x40..0x4c]);
        copy.extend_from_slice(&rec[0x4c..0x58]);
        copy
    }

    fn fixture() -> Image {
        let mut im = Image { m: BTreeMap::new() };
        im.p64(CONTROLLER + SIM_IN_CONTROLLER, SIM);
        im.p64(SIM + PLAYGROUND_IN_SIM, PLAYGROUND);
        im.p32(SIM + TIME_IN_SIM, (SIM_MS - 10) as u32);
        im.p64(PLAYGROUND + PLAYERS_IN_PLAYGROUND, PLAYERS);
        im.p32(PLAYGROUND + NPLAYERS_IN_PLAYGROUND, 1);
        im.p64(PLAYGROUND + SCENE_IN_PLAYGROUND, SCENE);
        im.p64(PLAYERS, PARTICIPANT);
        im.p32(PARTICIPANT + CLASS_IN_PARTICIPANT, CGAME_VEHICLE_PHY);
        for (k, off) in VEHICLE_SLOTS.iter().enumerate() {
            im.p64(PARTICIPANT + off, PHY[k]);
            // slot 0 drives; the others are parked, exactly as measured
            im.p32(PHY[k] + BODY_HANDLE_IN_PHY, if k == 0 { HANDLE } else { NO_BODY });
            im.p32(PHY[k] + STATE_IN_PHY, if k == 0 { 0 } else { STATE_PARKED });
            im.p32(PHY[k] + MODE_IN_PHY, 0);
        }
        im.p32(MODULE + VEHMGR_INDEX_GLOBAL, IDX);
        im.p64(SCENE + MANAGERS_IN_SCENE + 8 * IDX as u64, VEHMGR);
        im.p64(VEHMGR + VEHICLES_IN_VEHMGR, VARR);
        im.p32(VEHMGR + NVEHICLES_IN_VEHMGR, 4);
        for k in 0..4 {
            im.p64(VARR + 8 * k as u64, PHY[k]);
        }
        im.p64(VEHMGR + DYNA_IN_VEHMGR, DYNA);
        im.p64(DYNA + REMAP_IN_DYNA, REMAP);
        im.p64(DYNA + BODIES_IN_DYNA, BODIES);
        im.p32(REMAP + 4 * HANDLE as u64, INDEX);
        let pos = [1552.2075, 34.01432, 564.96521];
        let q = [0.998972, -0.004001, 0.045115, 0.001857];
        let v = [1.189158, -0.024525, 12.521759];
        let w = [0.006746, 0.259617, 0.010426];
        let rec = body_bytes(pos, q, v, w);
        im.put(BODIES + BODY_STRIDE * INDEX as u64, &rec);
        im.put(PHY[0] + QUAT_IN_PHY, &copy_of(&rec));
        im
    }

    fn run(im: &Image) -> Result<Car, String> {
        resolve_with(CONTROLLER, SIM, MODULE, SIM_MS, RACE_START, |a, n| im.read(a, n))
    }

    #[test]
    fn the_walk_reaches_the_body_the_solver_writes() {
        let im = fixture();
        let car = run(&im).expect("resolves");
        assert_eq!(car.phy, PHY[0]);
        assert_eq!(car.slot, 0);
        assert_eq!(car.body, Some(Body { handle: HANDLE, index: INDEX, addr: BODIES }));
        assert_eq!(car.body_pos(), Some(BODIES + 0x24));
        let l = car.layout();
        assert_eq!(l.pos, PHY[0] + 0x12f0);
        assert_eq!(l.quat, PHY[0] + 0x12e0);
        assert_eq!(l.vel, PHY[0] + 0x12fc);
        assert_eq!(l.clock, SIM + 0x48);
        assert_eq!(l.clock_bias, RACE_START as i64);
        assert_eq!(l.wet, PHY[0] + VIS_POST_IN_PHY + WETNESS_IN_VIS);
    }

    #[test]
    fn the_driven_slot_is_the_one_the_step_processes_whichever_it_is() {
        let mut im = fixture();
        // the car is the Rally one: parked flag and body move to slot 2
        im.p32(PHY[0] + BODY_HANDLE_IN_PHY, NO_BODY);
        im.p32(PHY[0] + STATE_IN_PHY, STATE_PARKED);
        im.p32(PHY[2] + BODY_HANDLE_IN_PHY, HANDLE);
        im.p32(PHY[2] + STATE_IN_PHY, 0);
        let copy = im.read(PHY[0] + QUAT_IN_PHY, 0x34).unwrap();
        im.put(PHY[2] + QUAT_IN_PHY, &copy);
        let car = run(&im).expect("resolves");
        assert_eq!(car.slot, 2);
        assert_eq!(car.phy, PHY[2]);
    }

    #[test]
    fn inside_a_respawn_window_the_car_has_no_body_and_is_still_named() {
        let mut im = fixture();
        im.p32(PHY[0] + BODY_HANDLE_IN_PHY, NO_BODY);
        let car = run(&im).expect("resolves without a body");
        assert_eq!(car.slot, 0);
        assert_eq!(car.body, None);
        assert_eq!(car.body_pos(), None);
        assert_eq!(car.layout().pos, PHY[0] + 0x12f0);
    }

    #[test]
    fn two_bodies_are_refused() {
        let mut im = fixture();
        im.p32(PHY[1] + STATE_IN_PHY, 0);
        im.p32(PHY[1] + BODY_HANDLE_IN_PHY, HANDLE);
        assert!(run(&im).unwrap_err().contains("2 vehicle slots have a dyna body"));
    }

    #[test]
    fn two_processed_slots_and_no_body_is_an_honest_refusal() {
        let mut im = fixture();
        im.p32(PHY[0] + BODY_HANDLE_IN_PHY, NO_BODY);
        im.p32(PHY[1] + STATE_IN_PHY, 0);
        let e = run(&im).unwrap_err();
        assert!(e.contains("2 vehicle slots are processed and none has a body"), "{}", e);
    }

    #[test]
    fn a_zero_handle_is_a_body_not_a_marker() {
        // 0 is a valid handle (it is the one the car gets); only the engine's
        // chosen 0xffffffff means none
        let mut im = fixture();
        im.p32(PHY[1] + STATE_IN_PHY, 0);
        im.p32(PHY[1] + BODY_HANDLE_IN_PHY, 0);
        assert!(run(&im).unwrap_err().contains("2 vehicle slots have a dyna body"));
    }

    #[test]
    fn a_wrong_record_is_caught_by_the_copy_out_cross_check() {
        let mut im = fixture();
        // remap now names index 1, whose record is not what the phy holds
        im.p32(REMAP + 4 * HANDLE as u64, 1);
        let other = body_bytes([1.0, 2.0, 3.0], [1.0, 0.0, 0.0, 0.0], [0.0; 3], [0.0; 3]);
        im.put(BODIES + BODY_STRIDE, &other);
        assert!(run(&im).unwrap_err().contains("not the same body"));
    }

    #[test]
    fn a_manager_index_that_does_not_own_the_vehicle_is_refused() {
        let mut im = fixture();
        for k in 0..4 {
            im.p64(VARR + 8 * k as u64, 0xdead_000 + k as u64 * 0x100);
        }
        assert!(run(&im).unwrap_err().contains("not among"));
    }

    #[test]
    fn a_stop_off_the_tick_boundary_is_refused() {
        let mut im = fixture();
        im.p32(SIM + TIME_IN_SIM, (SIM_MS - 20) as u32);
        assert!(run(&im).unwrap_err().contains("tick boundary"));
    }

    #[test]
    fn a_broken_hop_names_itself() {
        let mut im = fixture();
        im.m.remove(&(VEHMGR + DYNA_IN_VEHMGR));
        let e = run(&im).unwrap_err();
        assert!(e.contains("vehmgr.dyna"), "{}", e);
    }

    #[test]
    fn the_captured_sim_must_be_the_controllers() {
        let im = fixture();
        let e = resolve_with(CONTROLLER, SIM + 8, MODULE, SIM_MS, RACE_START, |a, n| im.read(a, n))
            .unwrap_err();
        assert!(e.contains("not the one the controller holds"));
    }

    #[test]
    fn the_segments_gather_the_record_in_the_consumers_order() {
        let car = run(&fixture()).unwrap();
        let segs = crate::layout::segments(&car.layout());
        assert_eq!(
            segs,
            vec![
                (SIM + 0x48, 4),
                (PHY[0] + 0x12e0, 16),
                (PHY[0] + 0x12f0, 12),
                (PHY[0] + 0x12fc, 12),
                (PHY[0] + 0x848 + 0x328, 4),
            ]
        );
        assert_eq!(segs.iter().map(|s| s.1 as usize).sum::<usize>(), crate::layout::REC_LEN);
    }
}
