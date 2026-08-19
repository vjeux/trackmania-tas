# [Turtle Trial] Angustus — author time beaten by 223.849 s

| run | time | vs AT |
|---|---|---|
| author time | 462.982 | — |
| the only human record (Quantiks, 1 of 1) | 1964.933 | +1501.951 |
| **the author's lap, retries cut, then optimised** | **239.133** | **−223.849 (−48.3%)** |
| the author's own lap, retries cut only — **watchable** | 246.602 | −216.380 (−46.7%) |
| our human-derived search tape | 262.907 | −200.075 (−43.2%) |

**The 246.602 is watchable** — `replays/AUTHORCUT_246602_watchable.Ghost.Gbx`
loads in the game. It is the map author's own author-time validation lap,
recovered from inside the `.Map.Gbx`, with the fourteen attempts they failed at
one obstacle cut out of it. No TAS driving at all.

TMX map [238835](https://trackmania.exchange/maps/238835) · author **Bald_tm** ·
tags **Trial, Turtle** · 5 checkpoints · **1 recorded run**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The insight: on a trial map, the clock runs through your failures

The recorded time on a trial map is **clean driving plus every failed attempt**.
The human's 32-minute record contains **198 respawns**. The author's own
465-second lap contains **20** — and roughly **160 of those 463 seconds are
fourteen failed attempts at a single obstacle**. Their actual clean driving is
about 246 s.

So a TAS does not have to drive better than a human here. **It has to not fail.**
This result involved *no driving search at all* — the entire 43 % came from
cutting failed attempts out of the tape.

## What a respawn actually is

Established empirically, and it turned out the whole project had been wrong
about it. A respawn lives **in the input bitstream**, and the oracle replays it:

| `word0` | meaning | human record | author's AT lap |
|---|---|---|---|
| `2` | ordinary vehicle packet | 196 296 | 46 430 |
| `34` = `0x22` (bit 5) | vehicle packet **+ respawn action** | 198 | 3 |
| `4098` = `0x1002` (bit 12) | a **second, different** respawn action | 0 | 17 |

`tmtraj` and `tmtas trace` decode only steer/gas/brake and are **blind** to
these bits, which is why every run this project had ever inspected reported
`NbRespawns: 0`. That was a property of the runs we fed the validator, not a
rule — `/validatepath=` accepts and exactly re-simulates a run containing 198 of
them.

The two kinds, measured from telemetry:

- **SOFT (one press of `0x22`)** — restores the state at the checkpoint
  crossing: position, speed **and attitude**. On a turtle map that hands you
  back a car doing 62 km/h upside down.
- **HARD (two presses of `0x22` within ~100–640 ms, or one press of `0x1002`)**
  — the checkpoint block's own transform: dead standstill, upright, square, then
  a ~0.5–0.9 s freeze. The validator counts the pair as **one** respawn. The
  author used the direct key for 17 of their 20.

## The method, and why it validates itself

Respawn state is **deterministic and history-independent**, so a failed attempt
can be spliced out of the tape and

> **`finish = base − deleted`, exactly, to the millisecond.**

That identity is a **self-validating acceptance test for every cut**: if the
arithmetic does not come out exact, the splice is wrong and you know immediately.

Two traps found the hard way:

- **Splices are exact individually but do not compose freely** — never merge
  respawn actions of different kinds.
- **A splice has a phase.** The grafted tail must start at the same offset from
  the *first* press of the action it lands on; sweep the deletion length a few
  ticks until the arithmetic is exact.

And a free 70 seconds: **a checkpoint's saved state is live on the very next
tick after it fires** (measured — tick 5891 works, 5890 does not). So you can
cross a checkpoint and immediately hard-respawn for a clean upright start,
deleting the entire approach. The author does exactly this at two checkpoints.

## For a human

The interesting statement about this map is not the time. It is that **the
unbeaten author time was never a display of driving** — it is someone failing
fourteen times at one obstacle, and the 25-minute difference between the author
and the only human to finish is almost entirely how many attempts each of them
needed.

The obstacle-by-obstacle guide is in `notes/RESULT.md`.

## Files

| file | what |
|---|---|
| `replays/TAS_262907.Ghost.Gbx` | the run — 262.907, independently re-validated with the human record as a known-answer control in the same batch |
| `replays/TAS_268554_v6.Ghost.Gbx` | the previous stage |
| `replays/TAS_347003_noretry_v4.Ghost.Gbx` | an earlier, more conservative cut |
| `notes/RESULT.md` | the full write-up, including the respawn bitstream analysis |
