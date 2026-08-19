# Spring 2023 - 24 (2-UP) — the author time falls, and it falls on a keyboard

| | time | vs AT | vs human WR | device |
|---|---|---|---|---|
| **TAS, unconstrained** | **49.778** | **−1.824** | −2.424 | analog |
| **keyboard** | **51.062** | **−0.540** | −1.140 | **3 values** |
| **a human keyboard run + seven actions** | **51.575** | **−0.027** | −0.627 | **keyboard** |
| one brake tap + a re-aim | 51.598 | −0.004 | −0.604 | keyboard |
| Author time (never beaten by a human) | 51.602 | — | −0.600 | — |
| Human WR — JuntaoTM | 52.202 | +0.600 | — | pad |

TMX map [199100](https://trackmania.exchange/maps/199100) · tags **Reactor,
Plastic, Altered Nadeo** · **6 recorded runs** · 51 seconds.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## What the map is

42.5 s of ordinary ground driving, then a **launcher captures the car** —
300 → 121 km/h, inverted — and thrusts it airborne for about 7 seconds,
climbing 180 m while re-accelerating to 400 km/h, into a finish gate hanging in
the air.

Our fastest tape is **byte-identical to the world record for the first 4,383
ticks**. **100% of the gain is air control.**

## The technique: one brake tap, 0.6 s into the climb

The deliverable is stated as a modification of a run a human already drove —
**uelen.'s rank-3 keyboard run, 52.599**, three steering values, 190 input
events, no respawn. Keep every one of their inputs and add seven actions:

| # | race time | action | duration |
|---|---|---|---|
| 1 | **43.23 s** | **tap brake** | 10 ms |
| 2 | 43.65 s | release gas | 110 ms |
| 3 | 43.82 s | tap brake | 30 ms |
| 4 | 47.80 s | tap right | 20 ms |
| 5 | 48.28 s | tap right | 50 ms |
| 6 | 49.17 s | hold brake | 90 ms |
| 7 | 49.27 s | hold right | 470 ms |

Cumulative, each validated:

| through action | validated | gain |
|---|---|---|
| none (control) | 52.599 | — |
| **#1 alone — one 10 ms brake tap** | **51.869** | **−0.730** |
| #1–#3 | 51.724 | −0.875 |
| #1–#5 | 51.638 | −0.961 |
| #1–#7 | **51.575** | −1.024 |

**Three quarters of the gain is a single 10 ms brake tap**, 0.6 seconds into the
reactor climb. The full keyboard tape (51.062) is the same idea taken further:
**pump the brake through the first 900 ms of the climb**, then aim in the last
two seconds.

## Is that tap a lottery ticket? No — measured two ways

**A 217-point grid** (start × duration, applied to the untouched human run):
every finishing variant lands between 51.764 and 52.529, so **a tap anywhere in
a ~400 ms window is worth 100–800 ms**, typically about 700.

**And a miss is recoverable.** Four different tap times — **including two that
miss the gate outright** — all recover to 51.598–51.795 when only the inputs
after 44.0 s are re-searched. Timing is loose; **re-aiming afterwards is
mandatory**, which is exactly what a driver does and a recorded tape cannot.

## There are two routes through sector 3, and every human here drives the slow one

This is the most useful thing the project has found on this map, and it needed no
search at all — only reading 88 recordings on the identical geometry.

Sector 3 is CP2 → CP3, and the two checkpoints are **241 m apart in a straight
line**. Split the 88 official records by whether they climb over the tower:

| line | n | mean sector 3 | best | mean path length |
|---|---|---|---|---|
| over the top | 27 | 5.664 | 5.516 | **710 m** |
| **low and short** | **61** | **4.721** | **3.901** | **306 m** |

**All five humans who have ever driven this map take the long one.** So does
every tape in our own lineage. That is not a difference in driving quality — it
is a different road.

**The long line looks like the good one.** Off CP2 it turns hard left, drives
*away* from the checkpoint, climbs a wall to y = 139, arcs over the top and drops
back down the far side. It never goes below 350 km/h and averages **430 km/h**.
It also covers **701 m** to travel 241 m of map.

**The short line carries straight on**, leaves the ground and *falls* 21 m,
lands already braking, scrubs down to **150 km/h** through a tight left, and then
takes a boost from **150 to 501 km/h in 0.8 s**. Its average speed is
**276 km/h** — far slower — over **299 m**. It wins by **1.971 s**.

> **A wider line reads faster and is slower.** Our whole lineage optimised speed
> along a route nobody re-examined.

### What it is worth, and why the number is credible

Substitute the field's short-line sector 3 into our own world record's lap and
change nothing else:

| | sector 3 | lap |
|---|---|---|
| our human WR as driven | 5.872 | 52.202 |
| WR + a **median** short line | 4.721 | **51.051** |
| WR + the **best** short line | 3.901 | **50.231** |
| author time | | 51.602 |

> **A median execution of a route that 61 of 88 sampled players already drive
> puts the human world record 0.551 INSIDE the author time** — no TAS, no
> reactor trick, no frame-precise input, and nothing changed after CP3.

Three things make that publishable as advice rather than as analysis:

1. **The slow phase is a braking phase**, which is the most forgiving input there
   is. This is the opposite of a precision-bound finding — 61 different players
   hit it, and an *ordinary* execution of the short line still beats the *best*
   execution of ours by 0.8 s.
2. **The two lines converge in state at CP3** — same position, same ~500 km/h
   (501 against our 510). Nothing downstream changes, so the gain is **additive
   rather than borrowed**. That is the separability question, answered in advance
   rather than discovered later.
3. **The route difference is categorical, not a skill difference.** 400 m of path
   length is not driving quality, and the 61-vs-27 split is a fork in the road.

### Stated plainly: this is a prediction, not a validated lap

**It is a sector sum measured from recordings, and the weld that would prove it
is exactly the one that fails on this map** — 2 829 splice attempts, 0 finishers.
So nobody has driven a tape that does this, and the number above is two humans'
sectors added together. It carries the standard caveat that a sector sum is a
bound until someone drives it.

What makes this one unusually credible is point 2 above: the handoff that
normally breaks a sector sum is not being asked to do anything, because the two
lines arrive at CP3 in the same state. And a few tenths of the 1.971 will be
ordinary skill difference between drivers — but the route part is not.

One practical note for anyone trying it: **the decision is made at CP2.** The low
line's landing and brake begin about 1.9 s later, and by then it is too late to
choose.

**CLASSIFICATION: known-but-unheld**, in its purest form. 200 000 people drive
this route on the unaltered copy of the map. It has never been driven *here*,
because the six people who have played this version all learned it from each
other.

## Why the author time stood

**The same brake tap on the pad world record does nothing** — 0 to 60 ms, never
a DNF. The two fastest humans are on pads, and on a pad the technique is
invisible. It only pays on a keyboard, and the fastest keyboard player is rank 3.

## Tolerance, honestly

On this map ±1 tick is a DNF for essentially every input — **including the
human's own tape**, where shifting one of her presses by a single tick is worth
up to 733 ms. That is a statement about replaying a recording blind, not about
human capability; a driver is a closed loop. The aiming inputs in our tape are
the forgiving ones (±60 ms at zero cost).

## Field reproduction, and one honest caveat

**5 of 6 records reproduce exactly.** The sixth is the only pre-2026 record
(build 2023-03-31), and the failure is build-correlated rather than
map-correlated: both respawn-heavy runs reproduce exactly, and **3/3 records on
the build that failed elsewhere reproduce here**. Everything seeded from
re-simulates to the millisecond.

**There is no author ghost in this map** — no `0x0911F000` and no `0x0309201D`
in the 2.8 MB decompressed body, with another map's author lap decoding in the
same tool as a positive control. The author is also absent from their own
six-run leaderboard, and this is an *Altered Nadeo* copy. **So unlike most maps
in this collection, we do not claim the author time here was ever driven** — it
may be inherited from the original map.

## Validation

Every tape through the plain oracle with a human ghost as a known-answer control
in every batch, plus a cold re-validation against a separately downloaded
sha256-identical copy of the map. 20 banked tapes re-validated from their
durable copies at the end. **Zero phantoms**, guard on throughout, ~800,000
evaluations over 13 arms.

A defect found here and fixed: **`tmtas splice` is not faithful for a
cross-splice** — 52.121 where two bit-identical tapes must give 52.202 — and its
own diagonal control does not catch it. Replaced with a one-file-image mixer.

## Files

| file | what |
|---|---|
| `replays/HUMANPLUS7_51575.Ghost.Gbx` | **the deliverable — a human's keyboard run plus seven actions** |
| `replays/KEYBOARD_51062.Ghost.Gbx` | the full keyboard tape |
| `replays/TAS_49778.Ghost.Gbx` | the unconstrained floor |
| `replays/A2_50738.Ghost.Gbx`, `A3_50224.Ghost.Gbx` | validated intermediates |
| `notes/RESULT.md` | full write-up: sector guide, tolerance study, negatives |

## This map is an Altered Nadeo copy of **Spring 2023 - 24**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **200 000
players** on this geometry.

Geometry **and** surface are preserved (`name_agree` 1.0000), so those humans drove the same car over the same road: their times are directly comparable and their lines are usable as references.

**Official tapes demonstrably run on this map.** Twenty official human ghosts have been grafted onto altered copies and each returned its own official time or split to the millisecond, so this is a demonstrated pipeline rather than a statement about physics. The graft recipe is map-dependent — carry the inputs chunk only, or all three, and pick whichever one's lossless control passes in the same batch.
