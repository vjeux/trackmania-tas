//! WHICH CLOCK THE FORK SERVER IS KEYED ON, and the units the driver owes it.
//!
//! Every stride, budget, deadline and checkpoint the driver sends to the shim
//! is in *clock units*. There are two clocks:
//!
//! * **`tick`** (the default): the shim hooks the entry of the engine's own
//!   per-tick function (build 128182: `0x119e060`, called first in the
//!   validator's tick loop, once per 10 ms of simulated time). One unit is one
//!   tick, and the reading is the engine's simulation time divided by ten --
//!   the same number in every process, under any load. See
//!   `tools/search/TICKHOOK.md`.
//! * **`lroundf`** (`FK_CLOCK=lroundf`, kept for A/B): the count of `lroundf`
//!   calls, ~255 per tick. Bit-identical on an idle box and NOT under load: the
//!   validator's frame loop has a wall-clock budget branch, contention cuts a
//!   run into more frames, and each frame costs ~62 more calls. A fixed count
//!   therefore lands on a different tick per process, which is what forced the
//!   per-worker probes and floors in the search.
//!
//! The conversions below are the ONLY place the two are told apart. Callers
//! say "N ticks" and get clock units back.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockMode {
    Tick,
    Lroundf,
}

/// `FK_CLOCK=lroundf` selects the legacy clock; anything else is the tick hook.
pub fn mode() -> ClockMode {
    static M: OnceLock<ClockMode> = OnceLock::new();
    *M.get_or_init(|| match std::env::var("FK_CLOCK").as_deref() {
        Ok("lroundf") => ClockMode::Lroundf,
        Ok("tick") | Err(_) => ClockMode::Tick,
        Ok(other) => panic!("FK_CLOCK must be `tick` or `lroundf`, not `{}`", other),
    })
}

pub fn tick_mode() -> bool {
    mode() == ClockMode::Tick
}

/// The value the shim's `FKSHIM_CLOCK` must be given for this mode.
pub fn shim_env() -> &'static str {
    match mode() {
        ClockMode::Tick => "tick",
        ClockMode::Lroundf => "lroundf",
    }
}

/// THE RACE START IS NOT A CONSTANT. The validator's simulation starts at
/// ~1000 ms and the race (input record 0 of a tape with `start_offset_ms = 0`)
/// usually starts at 2200 -- but 1 of 150 servers started at once put it at
/// 2300: the spawn is scheduled off the engine's frame clock, the same
/// load-dependent frame partition that made lroundf drift. So the shim reads
/// the start out of the engine (`max(round start, participant spawn)`, exactly
/// as the tick loop does before applying inputs) and keys its clock on RACE
/// time: `clock = (sim_ms - race_start) / 10 + RACE_CLOCK_BIAS`. The bias keeps
/// countdown ticks positive. Typical value, for reading logs only:
pub const TYPICAL_RACE_START_MS: i64 = 2200;
pub const RACE_CLOCK_BIAS: i64 = 1000;

/// Clock units per simulated tick.
pub fn per_tick() -> u64 {
    match mode() {
        ClockMode::Tick => 1,
        ClockMode::Lroundf => 255,
    }
}

/// The lroundf-clock fit, `clock = 36141 + 25.483 * race_ms` (three segment
/// maps of one ghost; another map fitted `5431 + 26.49 * race_ms`). Only the
/// legacy mode uses it, and only to place a checkpoint roughly.
pub fn lroundf_for_race_ms(ms: i64) -> u64 {
    (36141.0 + 25.483 * ms as f64).max(1000.0) as u64
}

/// The clock value at which a server should stop for race time `ms`.
///
/// Tick mode: EXACT -- the start of the tick at race time `ms`, before that
/// tick's input record is read, in whichever process and whatever its race
/// start. (A checkpoint earlier than ~180 ms before the race start fires at
/// the first tick at which the engine has set the start; everything before
/// that is countdown the engine never reads inputs for.)
pub fn ckpt_for_race_ms(ms: i64) -> u64 {
    match mode() {
        ClockMode::Tick => (ms.div_euclid(10) + RACE_CLOCK_BIAS).max(0) as u64,
        ClockMode::Lroundf => lroundf_for_race_ms(ms),
    }
}

/// The clock value at which a server should stop with tape tick `tick` as the
/// first unconsumed record (tick mode: exactly; lroundf mode: approximately).
pub fn ckpt_for_tick(tick: i64, start_offset_ms: i32) -> u64 {
    ckpt_for_race_ms(tick * 10 + start_offset_ms as i64)
}

/// The tape tick a tick-mode server stopped in front of, from the `sim_ms` and
/// `race_start` it reported in its handshake. This is what the page-fault
/// probe must agree with, on every server, every time.
pub fn tape_tick_at(sim_ms: u64, race_start_ms: u64, start_offset_ms: i32) -> i64 {
    (sim_ms as i64 - race_start_ms as i64 - start_offset_ms as i64).div_euclid(10)
}

/// The first tape tick the engine ever reads. The input application only
/// looks at the tape from race time -10 ms on (`0x119f0fc..0x119f11a`), so every record before that -- the
/// countdown -- is never consumed at all, which is why rewriting the
/// countdown region is physically inert.
pub fn first_read_tick(start_offset_ms: i32) -> i64 {
    (-10 - start_offset_ms as i64).div_euclid(10).max(0)
}

/// A simulated-time budget for a child that must run `ticks` more ticks.
///
/// Tick mode: `ticks + 2` (the sample for tick k lands at the hook of tick
/// k+1). Lroundf mode: the old `340 * ticks + 12000`, generous because the
/// count drifts.
pub fn budget_for_ticks(ticks: u32) -> u32 {
    match mode() {
        ClockMode::Tick => ticks.saturating_add(2),
        ClockMode::Lroundf => ticks.saturating_mul(340).saturating_add(12000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_mode_checkpoint_is_exact_and_inverts() {
        if !tick_mode() {
            return;
        }
        // map 2, rank 1: start_offset -1580, record 157 is read at race -10 ms
        // (sim 2190 with the usual 2200 start, 2290 with a 2300 one).
        assert_eq!(ckpt_for_tick(157, -1580), 999);
        assert_eq!(tape_tick_at(2190, 2200, -1580), 157);
        assert_eq!(tape_tick_at(2290, 2300, -1580), 157);
        assert_eq!(first_read_tick(-1580), 157);
        assert_eq!(first_read_tick(0), 0);
        for t in [0i64, 1, 60, 171, 2313, 43000] {
            let c = ckpt_for_tick(t, -1580) as i64;
            let race_ms = (c - RACE_CLOCK_BIAS) * 10;
            assert_eq!(tape_tick_at((2200 + race_ms) as u64, 2200, -1580), t);
        }
    }
}
