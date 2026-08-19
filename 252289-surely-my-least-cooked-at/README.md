# surely my least cooked at — author time beaten by 15 ms, and by 3 ms with two key presses

| tape | validated | vs AT | vs human WR | input changes | steer alphabet |
|---|---|---|---|---|---|
| human WR — KevinMagPizza | 3867 | +16 | — | 8 | `{-127,0,+127}` **keyboard** |
| **WR + two key actions** | **3848** | **−3** | −19 | 12 | keyboard |
| TAS, keyboard | **3844** | **−7** | −23 | 18 | keyboard |
| TAS, 8-value | **3836** | **−15** | **−31** | 18 | 8 values |

TMX map [252289](https://trackmania.exchange/maps/252289) · uid
`eetemRii0Hscd6vEudBsy4mbMK3` · author **in-.-** · **706 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The headline is not the 3836

> **Take KevinMagPizza's world-record run and add two keyboard actions in the
> last corner — a single-tick right tap at 2.63 s and a 50 ms throttle lift at
> 2.89 s — and it finishes in 3848 ms. That beats the author time by 3 ms with
> nothing a human cannot do.**

Nothing before 2.63 s changes at all. The lift on its own, added to the world
record, is already worth 9 ms.

And the record holder is himself **on keyboard** — exactly three steering values,
eight input changes for the whole lap. So this is not a machine out-precising a
human. It is two extra key presses in a corner nobody treated as a corner.

## Where the 31 ms is

All of it is in the final left sweep from 2.68 s.

The human holds **full lock and full throttle** through it and loses **6.2 km/h
between 3.02 and 3.12 s**, crossing the seam between the two lanes with the
suspension loaded. The TAS gives up 2 km/h on turn-in, **lifts the throttle for
50 ms at 2.89 s**, loses only 2.2 km/h, and is 5 km/h faster from 3.1 s to the
line — **1.11 m further down the road** when the clock stops.

Slow in, fast out, on a corner the whole field drove flat out.

## Tolerance, measured the honest way

Frozen, a one-tick error on our tape costs 280–1900 ms — which sounds
unrepeatable until you run the same test on **the human world record's own
tape**, where it costs 276–2871 ms. A person drove that. So frozen tolerance
measures open-loop replay, not human executability.

The fair measure is tolerance **with repair** — mistime an input, then re-drive
what follows, which is what a driver actually does:

**40 of 40 mistimings recover to exactly 3836**, and every one tried on the
keyboard tape recovers to exactly 3844.

**The line is robust; the tape is not.** The only input that genuinely needs
precision is the lift (start within ±1 tick, at least 50 ms long), and it has
both a speed cue and a visual cue.

## Method, and why it suited a 3.9 s map

The fork server was **measured and rejected**: a full re-simulation here costs
8.86 ms per candidate against the fork's ~11.5 ms floor. On a map this short,
resuming is slower than starting over.

So: the plain oracle, and **enumerate instead of sampling**. A full single-tick ×
255-value sweep of the entire 387-tick tape is 98,298 candidates in 16 seconds.

Both tapes are **1-move optimal at millisecond resolution** — 5.9 M and 6.3 M
closing candidates across single-tick, block-constant at every offset/length/
value, accel-brake, ramps, quadratics, segment values, boundary shifts, splits,
joint pairs, triples and two-tick moves, with **zero improving**. That is a
different statement from "the search stopped improving".

## One incident, reported rather than buried

Two keyboard tapes banked at 3844/3846 re-simulated at **4006/3848**.

Cause: the alphabet restriction quantised each candidate *inside the evaluator*,
but the search stored the **un-quantised** state as its incumbent — so the tape
written to disk was not the tape that had been scored. The scores were sound;
the artefacts were not.

This is a **fourth, independent class** of silent corruption in this project,
with nothing to do with fork servers or shared directories, and it is invisible
to any static scan — only re-validation catches it. The general rule:

> **Any transform applied inside the evaluator — quantise, clamp, minimum-hold —
> must also be applied wherever a state becomes the incumbent, and to the seed.**

Specimens preserved; the keyboard line was re-run from scratch after the fix.

## Files

| file | what |
|---|---|
| `replays/tas_twoinputs_3848.Ghost.Gbx` | **the world record plus two key presses** — the one worth studying |
| `replays/tas_keyboard_3844.Ghost.Gbx` | keyboard only, 18 inputs |
| `replays/tas_3836.Ghost.Gbx` | fastest, 8-value alphabet |
| `inputs/*.tick.txt` | each run as a readable input script |
| `notes/RESULT.md` | the full write-up and the driver's guide |
| `notes/validation.txt`, `notes/tol*.txt` | oracle transcripts and the tolerance tables |
