//! THE TICK HOOK'S BUILD CONSTANTS -- one file, two readers.
//!
//! The LD_PRELOAD shim `#[path]`-includes this file (it has no dependencies by
//! design) and checks the bytes in the mapped image before patching; `fk
//! tickhook check` reads the same offsets out of the ELF on disk. One
//! definition of "the tick function", so the two cannot drift.
//!
//! Server build 128182 (`TrackmaniaServer`, 30 113 288 bytes, md5
//! `0f0f4b25f31f80c60c81404366c95e68`). How they were found, and how to find
//! them again on another build: `tools/search/TICKHOOK.md`.

/// The per-tick entry called FIRST in the validator's tick loop, once per
/// 10 ms of simulated time, with `(players, n_players, new_time, dt)`.
pub const TICK_FN_OFF: usize = 0x119e060;
/// Its first 32 bytes. The first `TICK_FN_DISPLACED` are the relocatable
/// prologue (push rbp; mov rbp,rsp; push r15..rbx; sub rsp,0x18) that the
/// trampoline re-executes.
pub const TICK_FN_SIGNATURE: [u8; 32] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x83, 0xec,
    0x18, 0x48, 0x89, 0x7d, 0xc8, 0x85, 0xf6, 0x0f, 0x84, 0xe3, 0x00, 0x00, 0x00, 0x89, 0xf0, 0x48,
];
pub const TICK_FN_DISPLACED: usize = 17;
/// The tick loop's call site: `mov ecx,r14d; call rel32`, and the rel32 must
/// resolve to `TICK_FN_OFF` (checked, not assumed).
pub const TICK_CALL_SITE_OFF: usize = 0x12197e8;
pub const TICK_CALL_SITE_SIGNATURE: [u8; 8] = [0x44, 0x89, 0xf1, 0xe8, 0x70, 0x48, 0xf8, 0xff];
/// The tick loop's clock write at the end of the body:
/// `mov ebx,[rbp-0x30]; mov [r15+0x48],ebx` -- `sim.time = new_time`.
pub const TICK_CLOCK_WRITE_OFF: usize = 0x1219750;
pub const TICK_CLOCK_WRITE_SIGNATURE: [u8; 7] = [0x8b, 0x5d, 0xd0, 0x41, 0x89, 0x5f, 0x48];

/// Check every signature through `read(rva, n)` -- memory in the shim, the
/// ELF file in `fk`. `Err` names the first signature that does not match.
pub fn tick_hook_signatures_match(read: &dyn Fn(usize, usize) -> Vec<u8>) -> Result<(), &'static str> {
    if read(TICK_FN_OFF, TICK_FN_SIGNATURE.len()) != TICK_FN_SIGNATURE {
        return Err("tick function prologue");
    }
    if read(TICK_CALL_SITE_OFF, TICK_CALL_SITE_SIGNATURE.len()) != TICK_CALL_SITE_SIGNATURE {
        return Err("tick loop call site");
    }
    let rel = i32::from_le_bytes(TICK_CALL_SITE_SIGNATURE[4..8].try_into().unwrap()) as i64;
    if (TICK_CALL_SITE_OFF as i64 + 8 + rel) as usize != TICK_FN_OFF {
        return Err("tick loop call site does not target the tick function");
    }
    if read(TICK_CLOCK_WRITE_OFF, TICK_CLOCK_WRITE_SIGNATURE.len()) != TICK_CLOCK_WRITE_SIGNATURE {
        return Err("tick loop clock write");
    }
    Ok(())
}

/// How many leading bytes of `head` form a relocatable frame-pointer prologue
/// of at least 14 bytes: `push rbp; mov rbp,rsp`, then any run of `push r64`
/// and `sub rsp,imm8|imm32`. Position-independent by construction, so the
/// trampoline can re-execute them anywhere. `None` when the shape does not
/// fit -- the hook is then refused.
pub fn prologue_displaced_len(head: &[u8]) -> Option<usize> {
    if head.len() < 14 || head[..4] != [0x55, 0x48, 0x89, 0xe5] {
        return None;
    }
    let mut i = 4;
    while i < head.len().min(48) {
        if i >= 14 {
            return Some(i);
        }
        match head[i] {
            0x50..=0x57 => i += 1,
            0x41 if i + 1 < head.len() && (0x50..=0x57).contains(&head[i + 1]) => i += 2,
            0x48 if i + 3 < head.len() && head[i + 1] == 0x83 && head[i + 2] == 0xec => i += 4,
            0x48 if i + 6 < head.len() && head[i + 1] == 0x81 && head[i + 2] == 0xec => i += 7,
            _ => return None,
        }
    }
    if i >= 14 {
        Some(i)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_displaced_prologue_is_the_signature_head() {
        assert_eq!(prologue_displaced_len(&TICK_FN_SIGNATURE), Some(TICK_FN_DISPLACED));
        // the reader's `push rbp; mov rbp,rsp; mov rax,[rbp+0x10]` is NOT relocatable
        assert_eq!(prologue_displaced_len(&[0x55, 0x48, 0x89, 0xe5, 0x48, 0x8b, 0x45, 0x10, 0, 0, 0, 0, 0, 0, 0, 0]), None);
    }

    #[test]
    fn the_call_site_targets_the_function() {
        let ok = |o: usize, n: usize| -> Vec<u8> {
            match o {
                TICK_FN_OFF => TICK_FN_SIGNATURE[..n].to_vec(),
                TICK_CALL_SITE_OFF => TICK_CALL_SITE_SIGNATURE[..n].to_vec(),
                TICK_CLOCK_WRITE_OFF => TICK_CLOCK_WRITE_SIGNATURE[..n].to_vec(),
                _ => vec![0; n],
            }
        };
        assert_eq!(tick_hook_signatures_match(&ok), Ok(()));
        let bad = |o: usize, n: usize| -> Vec<u8> { if o == TICK_CLOCK_WRITE_OFF { vec![0x90; n] } else { ok(o, n) } };
        assert_eq!(tick_hook_signatures_match(&bad), Err("tick loop clock write"));
    }
}

// ---------------------------------------------------------------- the finish
//
// WHERE THE ENGINE PUTS THE RACE RESULT, and how it was found.
//
// A candidate costs 5.8 ms after its last simulated tick and before its first
// byte of output (`fk tickhook cost`) -- the validator's finish-and-print path,
// run for an answer that is one integer. A child that could read that integer
// could report it and `_exit` instead.
//
// The printer's own struct is no use: it is built at print time, out of a
// vector the engine hangs at `+0x4870`, so reaching it means paying for the
// thing we are trying to skip. (The printer is at `0x113b020`; its fields are
// read straight off the disassembly -- `Time` at struct+0x8, `Score` +0xc,
// `NbRespawns` +0x10, `NbCheckpoints` +0x20, each beside its own name string.)
//
// The RESULT BLOCK is different: the engine fills it AT THE FINISH. It was
// found by a backward pointer scan (`fk tickhook finish --chain`) -- snapshot
// the writable memory, take the words holding the finish time, find every
// pointer into their blocks, and keep the hops that land in an object the shim
// can already resolve. Of the sixteen candidates, exactly two survived three
// tapes with three different finish times, and this is the shorter.
//
//     [[controller + 0x1a88] + 0xa4]   the finish time, in SIMULATION ms
//                            + 0xbc    the race state: 2 racing, 3 finished
//
// The word is `0xffffffff` for the whole race and takes its value ONE TICK
// after the tick that detects the finish. It is exact, sub-tick included:
// rank00100 finishes at race 22884 ms -- not a multiple of 10, so the engine
// interpolates the crossing within the tick -- and this word reads sim 25084,
// which is 22884 to the millisecond.
//
// There is no code signature to check here (it is data, not instructions), so
// the shim checks it the only honest way: the word must read `NO_TIME` while
// the race is running. A build that moved the field fails that at once.

/// `controller + this` -> the block the engine fills at the finish.
pub const RESULT_PTR_IN_CONTROLLER: u64 = 0x1a88;
/// `result_block + this` -> the finish time in SIMULATION ms, or `NO_TIME`.
pub const FINISH_SIM_MS_IN_RESULT: u64 = 0xa4;
/// `result_block + this` -> 2 while racing, 3 once finished.
pub const RACE_STATE_IN_RESULT: u64 = 0xbc;
/// What the finish word holds until the race ends.
pub const NO_TIME: u32 = 0xffff_ffff;
/// What the race-state word holds once the player has finished.
pub const RACE_STATE_FINISHED: u32 = 3;
