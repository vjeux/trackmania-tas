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
