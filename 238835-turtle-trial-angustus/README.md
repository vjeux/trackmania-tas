# [Turtle Trial] Angustus — author time beaten by 223.849 s

| run | time | vs AT |
|---|---|---|
| author time | 462.982 | — |
| the only human record (Quantiks, 1 of 1) | 1964.933 | +1501.951 |
| **the author's lap, retries cut, then optimised** — [`TAS_239133`](replays/TAS_239133.Ghost.Gbx) | **239.133** | **−223.849 (−48.3%)** |
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

## Deletion is exhausted — and the proof is the *shape* of the failures

The obvious question about a result that came entirely from cutting failed
attempts out of a tape is whether the cutting is finished. It is, and the way
that was established is worth more than the answer.

The published tape carries **three** respawn presses. Two of them are the
author's standing-respawn key (`0x1002`), which the project's respawn census
does not count — the tape had been reported as carrying one:

| packet | race time | key |
|---|---|---|
| 11290 | 112.900 | `0x1002` standing |
| 15634 | 156.340 | `0x22` respawn |
| 20808 | 208.080 | `0x1002` standing |

If any of those three ends a *failed attempt*, then deleting the attempt in
front of it must return exactly `239.133 − deleted`, because this map's deletion
identity is exact to the millisecond. That makes the test self-validating. It
was run twice, the second time at full resolution:

| sweep | cut lengths | candidates | finishers |
|---|---|---|---|
| coarse | 1 / 3 / 6 / 10 / 20 / 40 s before each press | 18 | **0** |
| **fine** | **every cut start from 40.0 s before each press to the tick before it, at one-tick (10 ms) resolution** | **12 000** | **0** |
| identity control | rebuild with zero edits | 1 | **239.133 exact** |

**Cutting before respawn *k* kills the run at checkpoint *k*** — and not
approximately: `cps = 1` for all 4 000 candidates before the first press,
`cps = 2` for all 4 000 before the second, `cps = 3` for all 4 000 before the
third. Even a one-second cut does it. So each checkpoint crossing lies inside a
one-second window before its press, which means these are not retries at all:
they are the **deliberate post-checkpoint hard respawn** that this page names as
the map's whole technique, and every one of them is load-bearing.

**The published tape therefore contains zero failed attempts.** The −48.3 % was
not the easy part of this map — it was *all* of that part.

The graded failure pattern is also what makes the negative trustworthy. A broken
harness or a poisoned worker flattens every result to the same answer; three
independent populations of 4 000 partitioning perfectly by which press they cut
in front of, with the identity control exact in the same batch, is the signature
of real physics rather than a fault. The coarse sweep stated its own falsifier —
*a retry hiding between two of my offsets* — and the fine sweep closed it.

**CLASSIFICATION: known-but-unheld.** The technique is *hard-respawn the instant
you take a checkpoint*; the author already does it at CP2 and CP4; nothing about
the driving is undiscovered. What nobody has done is string the obstacles
together without the falls.

**And the low-input question is already answered, by construction.** On a trial
map the input axis that matters is not steer events, it is respawns — and this
tape descends from the author's own keyboard lap *by deletion only*, so no
TAS-only value was ever introduced. Measured off the bits, our tape and the
author's both use exactly three steer values, `{−127, 0, +127}`; the human record
uses 229 and is analog. There is no alphabet left to reduce.

### One correction, and one genuinely open problem

An earlier version of the notes said our segment 1 was 11.3 s behind the
author's retry-stripped driving because of a stall at (912, 104, 785) where the
human sits at 1.75 km/h for three seconds. **That belongs to a lineage this
result no longer uses.** The published tape descends from the author's lap, not
the human's, and its segment 1 *is* the author's 34 km/h pass. The gap was the
gap *to* the author, and changing seed is how it was collected.

What is genuinely open is narrower and better posed. This map has had a driving
search — 14 rounds of about 9 500 scored candidates, 246.602 → 239.133,
terminating on an **empty neighbourhood**. So it is *local-search exhausted from
that seed*, which is not the same claim as *unexamined*. What it has never had
is a **segment map**: every search here has been over the whole 239-second tape
at once, and nobody has tried per-obstacle search. The question that instrument
would answer is whether the author's own driving through each obstacle is
improvable — a harder question than the one that has been closed, with no
negative standing against it.

## Files

| file | what |
|---|---|
| `replays/TAS_239133.Ghost.Gbx` | **the result — 239.133**, the author's own lap with every failed attempt deleted and then optimised; re-validated against the untouched map with the human record (1964.933) and the author-cut lap (246.602) exact in the same pass |
| `replays/AUTHORCUT_246602_watchable.Ghost.Gbx` | **the watchable one** — the author's validation lap with their fourteen failures cut out, no TAS driving at all |
| `replays/TAS_262907.Ghost.Gbx` | the earlier human-derived line — 262.907, kept because the correction above is about it |
| `replays/TAS_268554_v6.Ghost.Gbx` | the previous stage |
| `replays/TAS_347003_noretry_v4.Ghost.Gbx` | an earlier, more conservative cut |
| `notes/RESULT.md` | the full write-up, including the respawn bitstream analysis |
