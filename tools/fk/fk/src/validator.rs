//! The controlled car, resolved from the validator's own simulation objects.
//!
//! This module is intentionally separate from [`crate::locate`]. The latter is
//! forensic tooling for finding coherent state-shaped records in arbitrary
//! captures. It cannot establish player identity. `ValidatorCar` starts at the
//! callback the `/validatepath` state machine itself invokes and follows only
//! typed ownership fields; there is no candidate enumeration or ranking.

use forkoracle::forksrv::{ForkServer, Rec};
use forkoracle::layout::Layout;
use forkoracle::procmem;

use crate::locate::{qualify2, ClockHit};

/// Build 128182 (`date=2026-05-15_18_00`) validator/player ownership layout.
#[derive(Clone, Copy, Debug)]
struct Offsets {
    controller_sim: u64,
    sim_playground: u64,
    playground_players: u64,
    playground_player_count: u64,
    participant_vehicle_class: u64,
    participant_vehicle: u64,
    vehicle_state_pos: u64,
}

const BUILD_128182: Offsets = Offsets {
    // 0x118c170: `mov [rdi+0x1a70], rcx`.
    controller_sim: 0x1a70,
    // 0x1218e3d: `mov rax,[r14+0x18]`, where r14 is the callback sim.
    sim_playground: 0x18,
    // 0x1218e41/4e: the sole validation-player vector.
    playground_players: 0x660,
    playground_player_count: 0x668,
    // 0x11a9b16..21: after the CGameVehiclePhy class check, store the class id
    // and pointer in the participant's primary vehicle slot.
    participant_vehicle_class: 0x1110,
    participant_vehicle: 0x1118,
    // CGameVehiclePhy: q(wxyz) at pos-16, world position, then velocity.
    // Writes/reads are visible at 0x11f38fe..0x11f3919 and 0x9cdb14 onward.
    vehicle_state_pos: 0x12f0,
};

/// CGameVehiclePhy's class id on build 128182.
/// Registered at 0xc3b62f as `CGameVehiclePhy`.
pub const CGAME_VEHICLE_PHY: u32 = 0x032e_2000;

/// The checkpoint counter's offset from the validator's participant object,
/// build 128182. Measured, not fitted: see `ValidatorCar::resolve`.
/// The `CSceneVehicleVisState` inside a `CGameVehiclePhy` (WHEELS.md §1: its
/// position triple is at +0x50, i.e. phy+0x898 = ValidatorCar.pos - 2728).
pub const VIS_IN_VEHICLE: u64 = 0x848;

/// The simulation time word inside the validation sim object (ms, +10 per tick,
/// car-independent) -- WHEELS.md §6.
pub const SIM_TIME_OFF: u64 = 0x48;

pub const CP_COUNTER_OFF: u64 = 0xc70;
/// Three more slots that step with it on every server measured; read at the
/// root as a cross-check that the offsets hold on this process.
pub const CP_COUNTER_CHECKS: [u64; 3] = [0xc80, 0xc90, 0xc94];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorCarProvenance {
    pub controller: u64,
    pub sim: u64,
    pub playground: u64,
    pub players: u64,
    pub participant: u64,
    pub vehicle: u64,
    pub state_pos: u64,
    /// Which participant slot was live: 0 Stadium, 1 Snow, 2 Rally, 3 Desert.
    pub car: u8,
    /// The live vehicle's `CSceneVehicleVisState` (`vehicle + 0x848`, 0x360 B).
    pub vis: u64,
}

/// A car whose identity came from the validator's controlled-player ownership
/// chain. Its inner `Layout` is private so production callers cannot substitute
/// a state-shaped address found by a scanner.
#[derive(Clone, Debug)]
pub struct ValidatorCar {
    layout: Layout,
    provenance: ValidatorCarProvenance,
}

impl ValidatorCar {
    /// Locate the race clock, then resolve the controlled vehicle from validator
    /// ownership. The clock scan labels samples; it does not participate in car
    /// identity.
    #[allow(clippy::too_many_arguments)]
    pub fn locate(
        srv: &mut ForkServer,
        probe: usize,
        recs: &[Rec],
        start_offset_ms: i32,
        bounds: (f64, f64, f64, f64, f64, f64),
        bias_max: i64,
        verbose: bool,
    ) -> Result<Self, String> {
        // THE CLOCK: the simulation's own time word at `sim + 0x48`, which is
        // car-independent. The race counter `find_clock2` locates sits beside
        // the ROOT car's vis state and FREEZES when a car-switch block re-binds
        // the participant's vehicle (INPUT arm, WHEELS.md §6: every later tick
        // then dedups into one row). The sim word must read within a tick of the
        // handshake's own sim time, or the scan is used as before.
        let clock = match Self::sim_clock(srv, verbose) {
            Some(c) => c,
            None => crate::locate::find_clock2(srv, probe, recs, start_offset_ms, bias_max, verbose)?,
        };
        Self::resolve(srv, probe, recs, clock, bounds, verbose)
    }

    /// `sim + 0x48` as the race clock, labelled by `measured_clock_bias`, if it
    /// reads like the simulation time the handshake reported.
    pub fn sim_clock(srv: &mut ForkServer, verbose: bool) -> Option<ClockHit> {
        if srv.validation_sim < 0x1000 || srv.sim_ms == 0 {
            return None;
        }
        let addr = srv.validation_sim + SIM_TIME_OFF;
        let v = procmem::read_at(srv.pid(), addr, 4).map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))? as i64;
        if (v - srv.sim_ms as i64).abs() > 20 {
            if verbose {
                println!("CLOCK sim+{:#x} reads {} but the handshake says sim {} -- not using it", SIM_TIME_OFF, v, srv.sim_ms);
            }
            return None;
        }
        let bias = forkoracle::layout::measured_clock_bias(srv, addr).ok()?;
        if verbose {
            println!("CLOCK sim+{:#x} = {:#x} reads {} (handshake sim {}), bias {:+} -- car-independent", SIM_TIME_OFF, addr, v, srv.sim_ms, bias);
        }
        Some(ClockHit { addr, bias })
    }

    /// Resolve and behaviorally validate the one controlled vehicle. Every hop
    /// is an exact pointer/field read. Structural physics checks reject a stale
    /// chain, but never choose between candidates.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        srv: &mut ForkServer,
        probe: usize,
        recs: &[Rec],
        clock: ClockHit,
        bounds: (f64, f64, f64, f64, f64, f64),
        verbose: bool,
    ) -> Result<Self, String> {
        let provenance = resolve_with(
            srv.validator_controller,
            srv.validation_sim,
            BUILD_128182,
            |a, n| procmem::read_at(srv.pid(), a, n),
        )?;
        let hit = qualify2(srv, probe, recs, clock.addr, provenance.state_pos, 150, bounds)
            .ok_or_else(|| {
                format!(
                    "validator-owned CGameVehiclePhy state at {:#x} failed the structural trajectory check",
                    provenance.state_pos
                )
            })?;
        if verbose {
            println!(
                "VALIDATOR CAR controller {:#x} -> sim {:#x} -> playground {:#x} -> player {:#x} -> CGameVehiclePhy {:#x} -> state {:#x}; verr {:.4} m/s, |q|-1 {:.2e}",
                provenance.controller,
                provenance.sim,
                provenance.playground,
                provenance.participant,
                provenance.vehicle,
                provenance.state_pos,
                hit.verr,
                hit.qerr
            );
        }
        // THE ENGINE'S OWN CHECKPOINT COUNTER, from the participant.
        //
        // Located behaviourally (tmenv cpfind, 2026-09-06): every writable
        // window of three servers running three different game-recorded
        // Summer 2026 - 01 ghosts was snapshotted 52 times over the run; the
        // 4-byte slots equal to "splits credited so far" at every snapshot
        // were traced per tick, and each stepped at exactly the tick of the
        // ghost's own split times (finish included). Four survive on every
        // server at fixed offsets from the participant: +0xc70, +0xc80, +0xc90
        // and +0xc94. The first is the one read; the other three are its
        // cross-checks at startup. At the root nothing is credited, so the
        // word must read 0 there -- a non-zero reading means the offset does
        // not hold on this build/map and the layout carries NO counter rather
        // than a wrong one.
        let cps_addr = provenance.participant.wrapping_add(CP_COUNTER_OFF);
        let cps = match procmem::read_at(srv.pid(), cps_addr, 4) {
            Some(b) if b.len() == 4 && u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 0 => {
                let agree = CP_COUNTER_CHECKS.iter().all(|o| {
                    procmem::read_at(srv.pid(), provenance.participant.wrapping_add(*o), 4)
                        .map(|c| c.len() == 4 && u32::from_le_bytes([c[0], c[1], c[2], c[3]]) == 0)
                        .unwrap_or(false)
                });
                if agree {
                    cps_addr
                } else {
                    if verbose {
                        println!("CP COUNTER: the cross-check slots do not read 0 at the root; not resolving it");
                    }
                    0
                }
            }
            Some(b) if b.len() == 4 => {
                if verbose {
                    println!(
                        "CP COUNTER: participant{:+#x} reads {} at the root, not 0; not resolving it",
                        CP_COUNTER_OFF,
                        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
                    );
                }
                0
            }
            _ => 0,
        };
        if verbose && cps != 0 {
            println!("CP COUNTER participant{:+#x} = {:#x}, reads 0 at the root, 3 cross-check slots agree", CP_COUNTER_OFF, cps);
        }
        Ok(Self {
            layout: Layout {
                pos: provenance.state_pos,
                clock: clock.addr,
                // ValidatorCar.pos is the PHYSICS object: its label is the vis
                // label + 10 (forkoracle::layout::physics_bias).
                clock_bias: forkoracle::layout::physics_bias(clock.bias),
                rms: hit.verr,
                max_dev: hit.qerr,
                cps,
                vis: provenance.vis,
                car: provenance.car,
            },
            provenance,
        })
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn provenance(&self) -> &ValidatorCarProvenance {
        &self.provenance
    }
}

fn word<const N: usize>(
    read: &mut impl FnMut(u64, usize) -> Option<Vec<u8>>,
    at: u64,
) -> Result<[u8; N], String> {
    let b = read(at, N).ok_or_else(|| format!("unreadable validator pointer hop at {:#x}", at))?;
    b.as_slice().try_into().map_err(|_| {
        format!(
            "short validator pointer read at {:#x}: {} of {} bytes",
            at,
            b.len(),
            N
        )
    })
}

fn ptr(
    read: &mut impl FnMut(u64, usize) -> Option<Vec<u8>>,
    at: u64,
    hop: &str,
) -> Result<u64, String> {
    let v = u64::from_le_bytes(word::<8>(read, at)?);
    if v < 0x1000 {
        return Err(format!(
            "validator pointer hop {} at {:#x} is null/invalid ({:#x})",
            hop, at, v
        ));
    }
    Ok(v)
}

fn u32_at(read: &mut impl FnMut(u64, usize) -> Option<Vec<u8>>, at: u64) -> Result<u32, String> {
    Ok(u32::from_le_bytes(word::<4>(read, at)?))
}

fn resolve_with(
    controller: u64,
    captured_sim: u64,
    o: Offsets,
    mut read: impl FnMut(u64, usize) -> Option<Vec<u8>>,
) -> Result<ValidatorCarProvenance, String> {
    if controller < 0x1000 || captured_sim < 0x1000 {
        return Err(
            "validator simulation callback was not captured; refusing heuristic fallback".into(),
        );
    }
    let sim = ptr(&mut read, controller + o.controller_sim, "controller.sim")?;
    if sim != captured_sim {
        return Err(format!(
            "validator callback sim {:#x} disagrees with controller+{:#x} -> {:#x}",
            captured_sim, o.controller_sim, sim
        ));
    }
    let playground = ptr(&mut read, sim + o.sim_playground, "sim.playground")?;
    let n = u32_at(&mut read, playground + o.playground_player_count)?;
    if n != 1 {
        return Err(format!(
            "validator playground has {} players, expected exactly one for a solo /validatepath run",
            n
        ));
    }
    let players = ptr(
        &mut read,
        playground + o.playground_players,
        "playground.players",
    )?;
    let participant = ptr(&mut read, players, "players[0]")?;
    // THE LIVE VEHICLE, not slot 0. The participant holds four (class, ptr)
    // pairs -- Stadium, Snow, Rally, Desert, in the order the cars joined the
    // game -- at +0x1110/+0x1118, +0x1120/+0x1128, +0x1130/+0x1138,
    // +0x1140/+0x1148; the live one is the slot whose u32 at phy+0x10 is not
    // 0xffffffff (the other three read -1 and hold a frozen position: the
    // spawn, or where they were swapped out). Reading slot 0 unconditionally is
    // why the car "never moved" on Fall 2024 - 14 and froze at the first gate of
    // Winter 2026 - 05 (INPUT arm, WHEELS.md §1, 2026-09-06). Slot 0 is the
    // fallback only when no slot says it is live.
    let mut phys: Vec<(u8, u64, bool)> = Vec::new();
    for k in 0..4u64 {
        let class = match u32_at(&mut read, participant + o.participant_vehicle_class + 0x10 * k) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if class != CGAME_VEHICLE_PHY {
            continue;
        }
        // slot 0 is the chain the build was audited on: its hop failing is a
        // stale chain and must fail LOUDLY; a missing extra slot is just absent.
        let phy = match ptr(&mut read, participant + o.participant_vehicle + 0x10 * k, "participant.vehicle[k]") {
            Ok(p) => p,
            Err(e) if k == 0 => return Err(e),
            Err(_) => continue,
        };
        let live = u32_at(&mut read, phy + 0x10).map(|v| v != 0xffff_ffff).unwrap_or(false);
        phys.push((k as u8, phy, live));
    }
    if phys.is_empty() || phys[0].0 != 0 {
        let class = u32_at(&mut read, participant + o.participant_vehicle_class)?;
        return Err(format!(
            "participant primary vehicle class is {:#x}, expected CGameVehiclePhy {:#x}",
            class, CGAME_VEHICLE_PHY
        ));
    }
    let live: Vec<&(u8, u64, bool)> = phys.iter().filter(|p| p.2).collect();
    let (car, vehicle) = match live.len() {
        1 => (live[0].0, live[0].1),
        0 => (phys[0].0, phys[0].1),
        n => {
            return Err(format!(
                "{n} of the participant's vehicle slots say they are live ({:?}); expected exactly one",
                live.iter().map(|p| p.0).collect::<Vec<_>>()
            ))
        }
    };
    let state_pos = vehicle + o.vehicle_state_pos;
    let state = word::<40>(&mut read, state_pos - 16)?;
    let f = |i: usize| f32::from_le_bytes(state[i..i + 4].try_into().unwrap());
    if !(0..10).all(|i| f(i * 4).is_finite()) {
        return Err(format!(
            "validator-owned state at {:#x} contains non-finite values",
            state_pos
        ));
    }
    let qn = (f(0).powi(2) + f(4).powi(2) + f(8).powi(2) + f(12).powi(2)).sqrt();
    if (qn - 1.0).abs() > 1e-3 {
        return Err(format!(
            "validator-owned state at {:#x} has quaternion norm {}, expected 1",
            state_pos, qn
        ));
    }
    Ok(ValidatorCarProvenance {
        controller,
        sim,
        playground,
        players,
        participant,
        vehicle,
        state_pos,
        car,
        vis: vehicle + VIS_IN_VEHICLE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn fixture() -> (u64, u64, BTreeMap<u64, Vec<u8>>) {
        let (controller, sim, playground, players, participant, vehicle) = (
            0x10000u64, 0x20000u64, 0x30000u64, 0x40000u64, 0x50000u64, 0x60000u64,
        );
        let mut m = BTreeMap::new();
        m.insert(
            controller + BUILD_128182.controller_sim,
            sim.to_le_bytes().to_vec(),
        );
        m.insert(
            sim + BUILD_128182.sim_playground,
            playground.to_le_bytes().to_vec(),
        );
        m.insert(
            playground + BUILD_128182.playground_players,
            players.to_le_bytes().to_vec(),
        );
        m.insert(
            playground + BUILD_128182.playground_player_count,
            1u32.to_le_bytes().to_vec(),
        );
        m.insert(players, participant.to_le_bytes().to_vec());
        m.insert(
            participant + BUILD_128182.participant_vehicle_class,
            CGAME_VEHICLE_PHY.to_le_bytes().to_vec(),
        );
        m.insert(
            participant + BUILD_128182.participant_vehicle,
            vehicle.to_le_bytes().to_vec(),
        );
        let mut state = vec![0u8; 40];
        state[0..4].copy_from_slice(&1.0f32.to_le_bytes());
        m.insert(vehicle + BUILD_128182.vehicle_state_pos - 16, state);
        (controller, sim, m)
    }

    fn resolve_fixture(
        controller: u64,
        sim: u64,
        m: &BTreeMap<u64, Vec<u8>>,
        o: Offsets,
    ) -> Result<ValidatorCarProvenance, String> {
        resolve_with(controller, sim, o, |a, n| {
            m.get(&a).filter(|b| b.len() == n).cloned()
        })
    }

    #[test]
    fn follows_the_validator_owned_chain_without_searching() {
        let (controller, sim, m) = fixture();
        let p = resolve_fixture(controller, sim, &m, BUILD_128182).unwrap();
        assert_eq!(p.participant, 0x50000);
        assert_eq!(p.vehicle, 0x60000);
        assert_eq!(p.state_pos, 0x612f0);
    }

    #[test]
    fn a_perturbed_hop_fails_loudly_instead_of_finding_another_object() {
        let (controller, sim, m) = fixture();
        let mut broken = BUILD_128182;
        broken.participant_vehicle += 8;
        let e = resolve_fixture(controller, sim, &m, broken).unwrap_err();
        assert!(e.contains("unreadable validator pointer hop"), "{e}");
    }

    #[test]
    fn callback_and_controller_must_name_the_same_simulation() {
        let (controller, sim, m) = fixture();
        let e = resolve_fixture(controller, sim + 8, &m, BUILD_128182).unwrap_err();
        assert!(e.contains("disagrees"), "{e}");
    }

    #[test]
    fn a_non_vehicle_primary_slot_is_rejected() {
        let (controller, sim, mut m) = fixture();
        m.insert(
            0x50000 + BUILD_128182.participant_vehicle_class,
            0x0a02_0000u32.to_le_bytes().to_vec(),
        );
        let e = resolve_fixture(controller, sim, &m, BUILD_128182).unwrap_err();
        assert!(e.contains("CGameVehiclePhy"), "{e}");
    }
}

/// The participant's LIVE vehicle, read from a paused process: `(slot, phy)`
/// with slot 0 Stadium, 1 Snow, 2 Rally, 3 Desert (WHEELS.md §1; the live one
/// is the slot whose u32 at phy+0x10 is not -1). `None` when no slot says it is
/// live or the reads fail -- the caller keeps what it had. For following a car
/// switch inside an episode: the participant address is the same in every
/// fork of the server.
pub fn live_vehicle(pid: i32, participant: u64) -> Option<(u8, u64)> {
    let o = BUILD_128182;
    let mut live: Vec<(u8, u64)> = Vec::new();
    for k in 0..4u64 {
        let class = procmem::read_at(pid, participant + o.participant_vehicle_class + 0x10 * k, 4)
            .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))?;
        if class != CGAME_VEHICLE_PHY {
            continue;
        }
        let phy = procmem::read_at(pid, participant + o.participant_vehicle + 0x10 * k, 8)
            .map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))?;
        if phy == 0 {
            continue;
        }
        let flag = procmem::read_at(pid, phy + 0x10, 4).map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))?;
        if flag != 0xffff_ffff {
            live.push((k as u8, phy));
        }
    }
    if live.len() == 1 { Some(live[0]) } else { None }
}

/// Where a `CGameVehiclePhy`'s physics position sits (ValidatorCar.pos).
pub const STATE_POS_IN_VEHICLE: u64 = 0x12f0;
