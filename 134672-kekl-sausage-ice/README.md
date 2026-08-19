# KEKL- SAUSAGE ICE — the author time did not fall, and the map explains itself

**Author time 58.687 · best human on a current build 68.442 · best validated 67.404.**

| tape | validated | vs AT | vs best today-legal human | steer values | steer events |
|---|---|---|---|---|---|
| [`TAS_67404`](replays/TAS_67404.Ghost.Gbx) | **67.404** | +8.717 | **−1.038** | 74 | 193 |
| [`KEYBOARD_67625`](replays/KEYBOARD_67625.Ghost.Gbx) | **67.625** | +8.938 | −0.817 | **3** | 114 |
| author time | 58.687 | — | — | — | — |
| human WR (2022), Roevhaal | 63.546 | — | — | — | does not re-simulate on a current build |
| best today-legal human *(control)* | 68.442 | — | — | — | — |

TMX map [134672](https://trackmania.exchange/maps/134672) · uid
`agH9XtjTZd8iZbuGp_KhC16jMO7` · author **Travis.TM**, uploaded by "KEKL Archive"
· **15 recorded runs** · the map's own TMX comment: *"Built in 15mins for KEKL"*.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## What the map is

A **narrow elevated ice ribbon** — the "sausage" — about 2 620 m long, driven at
30–45 m/s with the car permanently sideways. 252 wall pillars stacked five cells
high with a **custom ice deck** (`FlinkIceBlocks\3-1-*-Ice-Light`) on top, one
turbo gate, four checkpoints, a finish crossed airborne 8 m below the last
checkpoint.

## The measurement that explains everything else: errors e-fold in 0.7 seconds

Take the best today-legal human tape and change **one steer unit on one 10 ms
tick**:

| gate | reference | +1 unit at 2.0 s |
|---|---|---|
| 1.9 s | 1.916 | 1.916 |
| 2.9 s | 2.927 | **2.927 — exact** |
| 8.0 s | 7.973 | 8.037 (+0.064) |
| 9.6 s | 9.634 | **15.716 — the run is gone** |

A 1/127 perturbation is invisible for a second, worth +0.173 four seconds later,
and fatal by six. Everything else about this map follows from that one number:
the 40-second spread across 15 records, the fact that **0 of 319 input events are
deletable** at a 40 ms budget over 83 319 evaluations, and the field-reproduction
result below.

> **Measure a map's Lyapunov time before you choose a method.** It costs five
> perturbed candidates and one gate ladder, and it tells you whether splicing,
> thinning and cross-run transplant are available to you at all. Here they are
> not.

## Why the author time did not fall

Three independent estimates of what this route is worth:

| estimate | value |
|---|---|
| the field's best-sector recombination, all 15 records | 63.263 |
| the 2022 human world record | 63.546 |
| our own per-sector TAS optima, summed | 63.492 |

**They land within 0.283 s of each other, and the author time is 4.8 s below all
three.**

Our sector optima are lower bounds — a local hill climb from a human seed on a
chaotic tape, not a global optimum — so "the author time is unreachable" is
**not** proved and we do not claim it. But three facts sit together
uncomfortably: the author time is 4.8 s beyond everything this route has ever
produced from any source; the author's own online record is **69.522** — 10.8 s
slower than their own validation lap, on a map they built; and the map was built
in fifteen minutes out of stacked wall blocks and *embedded custom ice blocks*,
saved on a 2022 build.

The hypothesis that fits all three, and which we cannot test without the author's
own files, is that **the validation lap was driven on a state of this map that is
not the state that shipped** — most plausibly before the custom ice blocks were
in place, or before they behaved as ice. We tested the two alternatives we
*could* test: the map file is byte-identical to Nadeo's own copy (one TMX
version), and a route cut is refuted. Both negative.

What we record is the measurement, not the conclusion: **on the map as published,
under physics the oracle reproduces to the millisecond for every record set on a
current build, 58.687 is 4.8 s beyond the best line anyone — human or machine —
has produced.**

## A build-correlated reproduction failure is not a broken oracle

10 of 15 ghosts here fail to re-simulate, which on another map would be alarming.
**All ten are from one 2022 build.** All **5 of 5** from three different
2025–2026 builds reproduce exactly, including a 101.259 run, and the state
locator tracks a ghost's own telemetry to rms 0.008 m over 68 s.

On a map with a 0.7 s Lyapunov time, *any* physics-build difference is fatal to a
replay. That is a property of the map, not evidence against the instrument.
**Check the ghosts' `git=` build string before condemning a map** — this is a
different animal from a map where ghosts return *wrong times*.

## Tooling this map produced

* **`tmmaps gateladder`** — park every checkpoint off the track (rename it to a
  finish so it is not required, move it to a corner cell) and relocate the real
  Goal block to any 32 m cell, optionally keeping the first N checkpoints real.
  Turns a DNF into "reached cell (x, z) at t" for the price of one validation.
  Verified with yes-controls: a gate at CP2's own cell returns each ghost's own
  declared CP2 split to the millisecond. Build both `dir` families — 1/3 fires on
  crossings along x, 0/2 along z.
* **A gate with no yes-control proves nothing.** The mid-field "is there a cut
  here" probes on this map are weak negatives for exactly that reason, and are
  recorded as weak.
* **`tmmaps build` can derive the checkpoint order wrong.** It did here —
  243, 165, 170, 261 instead of 165, 170, 243, 261 — which silently makes every
  derived segment map wrong.

## Notes

* [`RESULT.md`](notes/RESULT.md) — the full write-up, sector by sector
* [`SUMMARY.md`](notes/SUMMARY.md) — the short version
* [`NOTES.md`](notes/NOTES.md)
