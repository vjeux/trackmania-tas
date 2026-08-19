# Fall 2025 - 22 Reverse CP1 End — author time beaten by 3 ms, and equalled on a keyboard

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **5.347** | **−0.003** | **−0.008** |
| TAS, earlier tape | 5.348 | −0.002 | −0.007 |
| **TAS, keyboard — 15 inputs, 3 values** | **5.350** | **±0** | **−0.005** |
| **the drivable one — 19 inputs, 42 % tolerant** | **5.351** | +0.001 | **−0.004** |
| Author time (never beaten by a human) | 5.350 | — | −0.005 |
| Human WR — Matik_K | 5.355 | +0.005 | — |

TMX map [279218](https://trackmania.exchange/maps/279218) · uid
`_Toadb_vTfXnT7PfAIpHypSJClk` · author **in-.-** · **339 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The map

201.5 m, 5.355 s, a 559-tick tape. Two waypoints — a road start and a
free-standing finish gate at (971.3, 50, 400) — and **no checkpoints**, so a
failed run returns no information at all.

**Gas is on and the brake is off on every tick of all 40 downloaded human
runs.** Steering is the only control. The map is one long full-lock left-hander
into a short straight.

Two measurements that shaped everything:

- **The finish-gate vernier is exact and linear at 17.0 ms/m**, so **1 ms =
  1.7 cm** and the whole 5 ms gap to the author time is **8.5 cm of reach**.
  That converts a timing problem into a geometry problem.
- **All the dispersion is in the corner, 110–190 m.** Ranks 1 through 344 are
  within a few milliseconds and about a metre of each other for the first 90 m.
  Among the top 15, ranks 5 and 9 are ~2 ms *ahead* of the world record through
  the corner and lose it all in the last 20 m — **the world record wins on
  exit**, not on the corner.

## The author time is a driven lap

Medals are 5350 / 6000 / 7000 / 9000 — the gold, silver and bronze are round
thousands, i.e. placeholders, so the author time is not derived from them.
Author `in-.-` also sits **rank 5 on their own board with 5358**, eight
milliseconds slower than the time they published.

So 5350 was driven, in the editor, by a person who could retry indefinitely —
and it is 5 ms better than 339 players have managed online. A human-repeatable
technique exists, because a human executed it.

## Validation

**40 of 40 human ghosts** — the top 15 plus five each from leaderboard offsets
25 / 50 / 100 / 200 / 339, spanning rank 1 to rank 344 — re-simulate to their
exact recorded millisecond. The candidate factory round-trips rank 1 to 5355.
Rank 1 travels in every batch as the identity control, and every published tape
re-validates on the untouched map.

Worth recording: the node doing this work **died mid-investigation** with two
hours of lease left. Everything survived because the map, the ghosts and the
analysis had been banked to shared storage; the work resumed on a replacement
box and the identity control was re-run from scratch there before anything was
trusted.

## The author time is reachable on a keyboard — 15 inputs, three values

**This does not beat the author time. It equals it.** The project's beat on this
map is the analog 5.347 above; what is new is that **5.350 is reachable on
`{−127, 0, +127}`**, which nobody had established.

| tape | validated | vs AT 5.350 | vs human WR 5.355 | events | values |
|---|---|---|---|---|---|
| analog floor (above) | 5.347 | −0.003 | −0.008 | 114 | 60 |
| [`KEYBOARD_5350_equals_AT`](replays/KEYBOARD_5350_equals_AT.Ghost.Gbx) | **5.350** | **±0** | **−0.005** | **15** | **3** |
| [`DRIVABLE_5351_5detents`](replays/DRIVABLE_5351_5detents.Ghost.Gbx) | 5.351 | +0.001 | −0.004 | 19 | 7 |
| [`KEYBOARD_5352_11events`](replays/KEYBOARD_5352_11events.Ghost.Gbx) | 5.352 | +0.002 | −0.003 | 11 | 3 |
| human WR, Matik_K | 5.355 | +0.005 | — | 107 | 59 |
| human rank 5 — **our seed** | 5.358 | +0.008 | +0.003 | 11 | 3 |

**Four constrained members all beat the human world record**, and the two
smallest alphabets are the two fastest of them. On this map the alphabet is
nearly free: keyboard costs 3 ms against the analog floor.

### It is a real human's own lap plus two blips

The keyboard tape descends from **rank 5's run — a human keyboard lap** — and
differs from it in exactly two places:

| | the human (5.358) | ours (5.350) |
|---|---|---|
| 2.76 s | hold RIGHT, 64 ticks unbroken | hold 37, **release for one tick**, hold 26 more |
| 3.91 s | full throttle throughout the left | **lift the throttle for two ticks**, steering unchanged |

Everything else — the launch, all three early right taps, the turn-in, the final
release — is byte-for-byte theirs.

**And the two blips are a coupled pair, which is why nobody finds them:**

| | validated | vs the human's 5.358 |
|---|---|---|
| both blips (ours) | **5.350** | −0.008 |
| the one-tick release alone | 5.355 | −0.003 |
| the throttle lift alone | **5.376** | **+0.018 — worse alone** |
| both reverted *(control)* | **5.358** | reproduces the human exactly |

The throttle lift is an 18 ms **mistake** by itself and becomes worth 5 ms only
once the release precedes it. Each half is punished when tried alone — the same
shape as [252289](../252289-surely-my-least-cooked-at), and the reason a human
grinding this map never stumbles onto the pair.

## Tolerance — and the tape to actually hand a person is neither of those

Measured on the pessimistic rule: move one input boundary ±1 tick, no
compensation, still finish within +0.050 of that tape's own time.

| tape | events | survives **both** directions | at least one |
|---|---|---|---|
| **analog `5.347`** | 114 | **71 (62 %)** | 81 % |
| **`DRIVABLE_5351`** | 19 | **8 (42 %)** | 95 % |
| human WR | 107 | 26 (24 %) | 85 % |
| human rank 5 (our seed) | 11 | 3 (27 %) | 45 % |
| `KEYBOARD_5350` | 15 | 1 (7 %) | 60 % |
| `KEYBOARD_5352` | 11 | 0 (0 %) | 82 % |

Two things fall out, and both correct something a reader might reasonably assume.

**The analog TAS tape is the most forgiving thing on this map — 62 %, more than
twice the human world record's 24 %.** An earlier note on this page called our
line a knife-edge that "a human will not reproduce"; that was written without a
tolerance measurement, and the measurement says the opposite. **Retracted.**

**Fewer events is *less* safe here.** 15 events at 7 % against 114 at 62 %, and
the fair comparison for the keyboard tape is its own human seed — 27 % on 11
events against our 7 % on 15. **A keyboard lap on this map is intrinsically
twitchy, for the human too.**

So the tape to hand a person is **`DRIVABLE_5351`**: 42 % two-sided on 19 events,
still 4 ms inside the human world record.

### Which way to fail — and it is *not* the same rule as 228607

| moment | 10 ms EARLY | 10 ms LATE |
|---|---|---|
| 0.66 s release right | 5.733 | **DNF** |
| 0.85 s tap right | **DNF** | 5.728 |
| 2.76 s hold right | 5.391 | **DNF** |
| 3.40 s release right | **DNF** | 5.386 |
| 3.50 s turn in left | 6.288 | 5.371 |
| 3.91 s throttle lift | 6.018 | 5.371 |
| 5.36 s final release | 5.350 | 5.350 — free |

> **The direction alternates.** The early right taps want to be *late*; the holds
> want to be *early*. There is no single "when in doubt" rule on this map — do
> **not** carry [Torment (1-UP)](../228607-torment-1-up)'s *early is free, late is
> fatal* across to it.

What *is* usable: the last two decisions — the turn-in and the lift — both cost
about 0.65 s early and only about 0.02 s late. **At the corner, late is cheap and
early is ruinous.**

## Files

| file | what |
|---|---|
| `replays/best_pF_5347_32087.Ghost.Gbx` | the run |
| `replays/best_pC_5348_32098.Ghost.Gbx` | an independent arm's 5348 |
| `replays/KEYBOARD_5350_equals_AT.Ghost.Gbx` | **the author time on three steer values** — a human's own lap plus two blips |
| `replays/DRIVABLE_5351_5detents.Ghost.Gbx` | **the one to hand a person** — 42 % two-sided tolerance, still inside the human WR |
| `replays/KEYBOARD_5352_11events.Ghost.Gbx` | the smallest tape that still beats the human WR |
| `notes/LOWINPUT_AND_TOLERANCE.md` | the keyboard family, the ablation, and the tolerance table |
| `notes/VALIDATION_family.txt` | the oracle transcript |
| `notes/PLAN.md` | the full pre-search analysis, all measured on this map |

## This map is an Altered Nadeo copy of **Fall 2025 - 22**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **44 128
players** on this geometry.

This is a **Reverse** variant: same physics, but those humans drove the route **backwards**. The official field gives you geometry and a corridor, **not a line** — and nobody has yet tried reversing it into one.

**Official tapes demonstrably run on this map.** Twenty official human ghosts have been grafted onto altered copies and each returned its own official time or split to the millisecond, so this is a demonstrated pipeline rather than a statement about physics. The graft recipe is map-dependent — carry the inputs chunk only, or all three, and pick whichever one's lossless control passes in the same batch.
