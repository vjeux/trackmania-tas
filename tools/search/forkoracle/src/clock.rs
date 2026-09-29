//! THE CLOCK THE FORK SERVER IS KEYED ON: the engine's own race tick.
//!
//! Every stride, budget, deadline and checkpoint the driver sends to the shim
//! is in ticks. The shim hooks the entry of the engine's per-tick function
//! (build 128182: `0x119e060`, called first in the validator's tick loop, once
//! per 10 ms of simulated time) and counts race ticks, so a checkpoint is the
//! same simulation point in every process under any load.
//!
//! It used to count `lroundf` calls (~255 per tick), which is bit-identical on
//! an idle box and NOT under load: the validator's frame loop has a wall-clock
//! budget branch, contention cuts a run into more frames, and each frame costs
//! ~62 more calls -- so a fixed count landed on a different tick per process.
//! That clock is GONE, along with the per-worker calibration it forced. The
//! page-fault probe stays as the CONTROL on the tick, not as the mechanism.
//! `tools/search/TICKHOOK.md` has the hook, the controls and the numbers.

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

/// The clock value at which a server should stop for race time `ms`.
///
/// Tick mode: EXACT -- the start of the tick at race time `ms`, before that
/// tick's input record is read, in whichever process and whatever its race
/// start. (A checkpoint earlier than ~180 ms before the race start fires at
/// the first tick at which the engine has set the start; everything before
/// that is countdown the engine never reads inputs for.)
pub fn ckpt_for_race_ms(ms: i64) -> u64 {
    (ms.div_euclid(10) + RACE_CLOCK_BIAS).max(0) as u64
}

/// The clock value at which a server should stop with tape tick `tick` as the
/// first unconsumed record. Exact.
pub fn ckpt_for_tick(tick: i64, start_offset_ms: i32) -> u64 {
    ckpt_for_race_ms(tick * 10 + start_offset_ms as i64)
}

/// The tape tick a tick-mode server stopped in front of, from the `sim_ms` and
/// `race_start` it reported in its handshake. This is what the page-fault
/// probe must agree with, on every server, every time.
pub fn tape_tick_at(sim_ms: u64, race_start_ms: u64, start_offset_ms: i32) -> i64 {
    (sim_ms as i64 - race_start_ms as i64 - start_offset_ms as i64).div_euclid(10)
}

/// WHICH RECORD THE ENGINE READS AT A GIVEN SIMULATION TIME, exactly as
/// `0x119f0f0` decides it:
///
/// ```text
/// esi = race_start - 10
/// if race_start is unknown (-1) or new_time < race_start - 10:
///         copy RECORD 0 verbatim                       (0x119f127)
/// else    i = (new_time - race_start - start_offset) / 10
///         if i >= record_count: the .rdata default, [out+0x1c] = 2
///         else copy RECORD i                           (0x119f169)
/// ```
///
/// Two consequences that are easy to get wrong, and both were:
///
/// * **Every pre-race tick reads record 0**, so a tape whose `start_offset_ms`
///   is -1580 never has records 1..=156 read at all -- the engine uses record 0
///   as the input for the whole countdown. That is why rewriting the countdown
///   region is physically inert: it was measured that way (a `best_23200` with
///   it replaced still gave 23200) long before anyone could say why.
/// * The first record the INDEX path reads is `(-10 - start_offset) / 10`, at
///   race time -10 ms -- one tick before race 0.
pub fn record_read_at(sim_ms: u64, race_start_ms: u64, start_offset_ms: i32) -> i64 {
    if (sim_ms as i64) < race_start_ms as i64 - 10 {
        return 0;
    }
    tape_tick_at(sim_ms, race_start_ms, start_offset_ms)
}

/// The first record the index path ever reads, at race time -10 ms.
pub fn first_read_tick(start_offset_ms: i32) -> i64 {
    (-10 - start_offset_ms as i64).div_euclid(10).max(0)
}

/// A budget for a child that must run `ticks` more ticks: `ticks + 2`, because
/// the sample for tick k lands at the hook of tick k+1.
pub fn budget_for_ticks(ticks: u32) -> u32 {
    ticks.saturating_add(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checkpoint_is_exact_and_inverts() {
        // map 2, rank 1: start_offset -1580, record 157 is read at race -10 ms
        // (sim 2190 with the usual 2200 start, 2290 with a 2300 one).
        assert_eq!(ckpt_for_tick(157, -1580), 999);
        assert_eq!(tape_tick_at(2190, 2200, -1580), 157);
        assert_eq!(tape_tick_at(2290, 2300, -1580), 157);
        assert_eq!(first_read_tick(-1580), 157);
        assert_eq!(first_read_tick(0), 0);
        // the countdown reads record 0, not the tape's countdown records
        assert_eq!(record_read_at(2020, 2200, -1580), 0);
        assert_eq!(record_read_at(2190, 2200, -1580), 157);
        assert_eq!(record_read_at(2200, 2200, -1580), 158);
        for t in [0i64, 1, 60, 171, 2313, 43000] {
            let c = ckpt_for_tick(t, -1580) as i64;
            let race_ms = (c - RACE_CLOCK_BIAS) * 10;
            assert_eq!(tape_tick_at((2200 + race_ms) as u64, 2200, -1580), t);
        }
    }
}
