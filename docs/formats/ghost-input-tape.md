# The input tape: chunk `0x0309201D`

The 10 ms record of what the driver pressed — the one thing the dedicated
server re-simulates. Codec: `tools/gbx/src/tape.rs` (`Tape`, `Archive`,
`Packet`, `Encoding::{Verbatim, Explicit}`), bit I/O `tools/gbx/src/bits.rs`
(LSB-first). Editors: `ghost tape extract|inject|expand|diff|stats|bits|sweep|csv`,
`ghost trim`, `ghost splice`, `pkz2 edit`, `ghost script`. Confidence **[FILE]**:
the verbatim re-encode reproduces the file's own bitstream on 307 of 307 files
in the corpus, and `tmsite verify` replays 227 of 227 exported scripts back
exactly.

## 1. Chunk layout

```text
u32 chunk 0x0309201D, "PIKS", u32 size
u32 chunkVersion                 ≤ 4 (4 on every file seen)
u32 nArchives                    1 on every file seen (readers take the first; `--nth` exists)
nArchives × Archive:
    u32 formatVersion            11 → 33-bit state literal; 12 → 34-bit (2026 files are 12)
    u32 field0
    i32 startOffset_ms           race time of tick 0: NEGATIVE on a countdown-prefixed tape
                                 (−1580 on the fixture; −120 on the replay fixture; −1570/−1510/
                                 −1520 on others). NEVER assume 0.
    u32 packetCount
    u32 bitstreamLength          bytes
    u8[bitstreamLength] bitstream
```

`race_ms = tick × 10 + startOffset_ms`. A real bitstream is **11–33 bytes
longer** than its packets need; the tail past the last packet's bits is part
of the file and a verbatim re-encode carries it through (`Archive::tail`).

A ghost can carry MORE than one input chunk (`find_inputs_chunks`); a file with
none (`AUTHOR_LAP_20258_watchable`, record-data only) is an error, not a run.

## 2. One packet per 10 ms tick

```text
1. STATE WORD, three codings:
     1                      repeat the previous packet's word
     0 1 x y                repeat it, overriding flag bits 0 and 1
     0 0 <literal>          an explicit 33- or 34-bit literal
   literal → (word0, flags):
     flags = (lit >> 5) & 0x3FFFFF                              (22 bits)
     word0 = ((((hi & 2) << 5) | (hi & 1)) << 6) | ((lo as i32 >> 20) & 1920) | ((lit >> 26) & 0x20) | (lit & 0x1F)
     (hi = lit >> 32, lo = lit & 0xFFFFFFFF)
   mode = word0 & 0xF
   RESPAWN = bit 31 of the literal → word0 bit 5 (0x20). Only an explicit literal carries it; a
   repeated word ALWAYS reads 0, so writing a respawn onto a repeated packet forces an expansion.
2. MOUSE:  1 → none;  0 → two 16-bit axes
3. VEHICLE FIELDS, per mode:
     2, 4       same-bit (1 = same as previous); else steer:8, accel:1, brake:1
     12         same-bit; else steer:32, accel:1, brake:1
     13         same-bit; else steer:32
     0          nothing
     otherwise  same-bit; else four 2-bit trigger fields
```

Steer for an 8-bit field is a **signed i8 over 127** (`-127..=127`; `0x80` =
−128 is what TICK refuses and 0 of 233 real ghosts contain). Modes seen: `[2]`
on keyboard/pad recordings, `[2, 15]` on the replay fixture. The decoder starts
from a BLANK packet (mode 2, all zero), so a first packet coded "same as
previous" inherits zeros — the encoder must agree or every byte after it shifts.

**"Same as previous tick" is one bit with no fields behind it — not a frozen
tick.** To write a different input there the packet is EXPANDED into its
explicit form; `Encoding::Explicit` always does, `Encoding::Verbatim` keeps the
original coding decisions (byte-identical when nothing was edited). A packet
whose value differs from the previous one is always written explicitly whatever
the encoding asks — an edit is never dropped to keep a same-bit.

## 3. The gtape text form (`ghost tape extract`)

```text
#gtape 1
#source /path/to/donor.Ghost.Gbx
#chunk_version 4
@archive 0 format_version=12 field0=0 start_offset_ms=-1580 packets=2432 bitstream_bytes=2151 bits_used=17203
@tail 0e00…                                   bytes past the last packet, hex
t=0 mode=2 w=lit:0x0000000E2 respawn=0 mouse=none vsame=0 steer=0 accel=1 brake=0 flags=0x000007
t=1 mode=2 w=prev respawn=0 mouse=none vsame=1 steer=0 accel=1 brake=0 flags=0x000007
```

`t` tick index; `w` how the word was coded (`lit:<hex>` | `prev` |
`prev2:<x>,<y>`) so the round trip is exact; `respawn` authoritative (set it
and the writer puts it in the literal); `vsame` the original coding — change a
value and the packet expands; `steer` the signed i8 (a 32-bit field prints as
`steer32=0x…`); `mouse=none` or `mouse=a,b`; `tri=a,b,c,d` for the trigger
modes. `extract → inject → extract` is byte-identical.

Other text forms of the same data:

* `ghost tape csv` → `race_ms,steer,accel,brake` (what every map page
  publishes under `inputs/`);
* `tmsite tick` → a TICK input script: `<ms> <action> <value>` with `<ms>` a
  multiple of 10, actions `steer` (−127..127, `left`, `right`), `accel`,
  `brake`, `respawn`; header `# 2432 ticks, ghost start offset -1.580 s,
  declared 22.730 s`;
* `ghost script` reads an EVENT script (`1230 press left`, `1310 release
  left`; keys `gas brake left right`; `vidread ktevents` produces it off a
  video) and builds a tape by holding the last state between events.

## 4. Facts the codec is built on

* The tape carries the driver's inputs a SECOND time inside the telemetry
  (`ghost-telemetry-cplugentrecorddata.md` §6): byte 14 = `floor((steer_i8 +
  127)·255/254)` (FLOOR, and 254: a `round` misses steer 0 and 60), byte 15
  = 255 with the gas down, byte 18 = 255 with the brake. Kappa between the two
  channels is the cheapest contamination check there is.
* **A tape only validates in the container it was built in** (`0x0309202D`,
  `ghost-cgamectnghost.md` §6). Moving a tape between containers needs both
  chunks moved (`tools/chunkswap`).
* A tape edit changes what the file DOES but not what it DECLARES: after
  `tape inject`, `ghost declare --from-oracle` must rewrite the time, or every
  "says X / does Y" check compares a stale number.
* Respawn semantics **[VERIFIED-GAME]**: on a map with checkpoints a respawn is
  a SOFT respawn — the car is restored to its state at the last checkpoint
  crossing (bit-identical after it: 286279 RESULT §1.4); the dedicated server
  refuses `using unsupported respawns (N)` on some builds/modes
  (`dedicated-server-oracle.md`).
* A 32-bit steer field (modes 12/13) is an analogue axis; nothing here has
  measured its scale against the game — `steer32` is printed raw.
* Tick alignment of a REGENERATION is nondeterministic (~1 in 7 runs lands a
  tick out); the tape itself is exact (`ghost phase`, `tmtraj gate C-route`).

## 5. Not known

* `field0`'s meaning (0 on every file; part of the tape for `fk tapeswap`).
* The 22 `flags` bits' meaning beyond bits 0/1 (overridable by the `0 1 x y`
  coding); `ghost tape bits` censuses which ever vary.
* The trigger modes' four 2-bit fields (seen only on the replay fixture,
  mode 15).
