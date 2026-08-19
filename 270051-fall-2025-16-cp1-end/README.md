# Fall 2025 - 16 CP1 End — author time beaten, and matched with inputs a human can hold

| | time | vs AT | vs human WR | slack on every input |
|---|---|---|---|---|
| TAS, unconstrained floor | **4.830** | **−1** | −4 | one-tick precision |
| **TAS, human-shaped** | **4.831** | **±0** | **−3** | **±10 ms** |
| TAS, one input change | 4.832 | +1 | −2 | — |
| TAS, keyboard only | 4.834 | +3 | ±0 | — |
| Author time (never beaten by a human) | 4.831 | — | −3 | — |
| Human WR — OriginalCJM | 4.834 | +3 | — | — |

TMX map [270051](https://trackmania.exchange/maps/270051) · uid
`vsiB0RwTzQeq5Loh21CuloIzjf9` · author **in-.-** · **903 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Read this row, not the fastest one

`replays/m270051_human_shaped_4831.Ghost.Gbx` **matches the author time with
±10 ms of slack on every input.** That is the tape worth studying. It is two
small trims on the human world record's own line:

- **a light left brush at 2.90 s** — 7 % of lock, three ticks, with a 30 ms
  window in which it works and a wide band of acceptable strength
- **ease the left trim by 1.5 % at 3.35 s**

That is it. No route change, no different jump.

The 4830 exists and is real, but its last millisecond comes from a **one-tick
75 %-lock stab** — a single 10 ms frame no human places deliberately. It is the
floor, not the lesson.

The difference between those two rows came from changing what the search
optimises. Scoring candidates by their *worst* time over a ±1–2 tick placement
window — rather than their best time — found the **same physical effect** as the
one-tick stab, expressed as a forgiving three-tick brush. **The lottery ticket
and the teachable input were the same discovery all along; only the objective
decided which one came out.**

## What the map is

484 race ticks (4.83 s), one checkpoint — the finish. **Full throttle
throughout, no brake, ever.** The whole map is a steering problem.

The last 620 ms is **ballistic flight**: the car leaves the ground at ~4.210
at 176.8 km/h on a 21° climb and never lands, crossing the finish in mid-air,
still rising.

So the finish time is decided at the takeoff tick. **3.8 of our 4 ms is simply
being 15.6 cm further along the track when the wheels leave the ramp.** Same
jump, same attitude, line within 12 cm of the human world record's — well inside
the field's 1.35 m corridor.

Two consequences, both measured:

1. **Inputs after ~4.360 are worth at most 1 ms.** Overwrite every input from
   tick 436 with "steer 0" and the time changes by 0 or −1 ms. The car is in the
   air; steering only rotates it.
2. **1 ms = 4.55 cm of travel at the finish plane.** The whole 3 ms between the
   human world record and the author time is **13.6 cm** of forward progress.

## Why nobody has done it — verdict: known but unheld

Not a route discovery. **Invisible** is the better word.

Both inputs are small trims on sections that feel like nothing is happening, and
their payoff is 15 cm at a ramp — which nothing in the cockpit shows you. There
is no feedback loop a driver could use to find them.

The sector analysis says the same thing. The closing jump, the dramatic part,
spreads only **5 ms** across the field and correlates **0.07** with finishing
order. The stretch at **2.4–3.7 s**, where both winning inputs sit, correlates
**0.43 / 0.31**. The part of the map that looks decisive is not, and the part
that decides it looks like nothing.

## Validation

A 1 ms margin has to be airtight:

- **24 of 24** downloaded human ghosts re-simulate to their exact leaderboard
  millisecond against this map file.
- **Five cold re-validations** of the banked tape, each in a fresh directory with
  a fresh server process and the rank-1 human ghost as a known-answer control:
  5/5 returned 4830, control 4834 every time, `NbRespawns: 0`, `IsValid: true`.
- A **second independent code path** driving the same server also returns 4.830.
- The map file is **sha256-identical to Nadeo's own copy**.
- No `--fork`, a distinct `--root` per process, every tape re-validated. No
  phantoms.

`sha256 8c7436a4afa4180f68a54b141738f47a759867a73b1c9f165677ef968bc4a579`

Convergence: ~580,000 evaluations across every move class — all 254 steer values
at every tick, spans out to 21 ticks, throttle lifts, brake taps, and 169,216
two-tick pairs — found nothing better at 0.05 ms resolution.

## Instruments built here

- **A vernier of relocated finish-gate maps** (`tmmaps gate`), 2–4 mm apart,
  turning the 1 ms-quantised score into a 0.05 ms objective. Necessary when the
  entire gap is 13.6 cm of travel.
- **A robustness objective** — score by the worst time over a placement window.
  This is what produced the teachable 4.831, and it is the technique most likely
  to transfer to other maps.

One bug found and worth repeating: relocated gate maps keep the **original
mapUid**, so a ladder plus the real map in one worker's `Maps/` directory
silently measures the wrong map. Caught by the batch identity control.

## Files

| file | what |
|---|---|
| `replays/m270051_human_shaped_4831.Ghost.Gbx` | **the author time, with ±10 ms slack on every input** |
| `replays/m270051_4830.Ghost.Gbx` | the unconstrained floor |
| `replays/m270051_one_input_4832.Ghost.Gbx` | one input change from the human WR |
| `replays/m270051_keyboard_4834.Ghost.Gbx` | keyboard only, ties the human WR |
| `inputs/rob4_4831.json` | the robust tape's per-tick inputs |
| `notes/RESULT.md` | the full write-up |

## This map is an Altered Nadeo copy of **Fall 2025 - 16**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **87 596
players** on this geometry.

This is the best case of the ten. It is a **CP1 End** variant, so the official map's opening **is** our entire race — those humans have all driven exactly our sector, at full commitment, as the start of their own lap.

**This is the map where official-tape transfer was demonstrated.** All five of the official top five, grafted onto this map, returned their own official CP1 splits to the millisecond — 4.951 / 4.951 / 4.962 / 4.966 / 4.932, with the lossless control exact at 4.831 in the same batch. Five foreign tapes, five exact predictions that could not have been tuned.

**And it re-scores our own result. Our 4.830 beats every one of them**, against a field of 87 596 players rather than the 903 on the altered board. The recipe on this map is inputs-only (`--ids 0x0309201D`); the three-chunk form breaks it, because carrying the donor's result chunk declares nine official splits onto a map with one waypoint.
