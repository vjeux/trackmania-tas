# unluckE - get jiggy with it — author time beaten by 21 ms, and by 20 on a keyboard

| | time | vs AT | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, unconstrained** | **6322 ms** | **−21** | **−24** | analog |
| **TAS, pure keyboard** | **6323 ms** | **−20** | −23 | **23 change events, 3 values** |
| **human WR + ONE changed input** | **6342 ms** | **−1** | −4 | keyboard |
| Author time (never beaten by a human) | 6343 ms | — | −3 | — |
| Human WR | 6346 ms | +3 | — | — |

TMX map [145875](https://trackmania.exchange/maps/145875) · author **InfTM** ·
**46 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The cheapest possible advice for the field

> **Take the human world record's own tape and change ONE input: fire the final
> flick about 0.1 s earlier and hold ~80 % instead of 100 %. That validates at
> 6342 — one millisecond under the author time.**

Full enumeration of that single input shows a **6342 plateau spanning 50 ms of
timing and most of the top half of the stick**. It is not a knife-edge; it is a
wide, forgiving target that nobody in 46 runs has aimed at.

## The keyboard result

**6323 ms with 23 input change events**, steering only ever −127/0/+127,
throttle held from the countdown to the line, brake never touched. That is one
millisecond off the unconstrained floor — **this map does not need a pad.**

And the field already knows it: **8 of the 13 fastest humans are on a keyboard**
(measured — two of them move −127 → +127 inside a single tick, so TM2020 does
not ramp a held key). The best of them is 37 ms slower than a keyboard can go.

## What nobody is doing

Verdict: **known-but-unheld, plus one undiscovered detail.** The field already
drives this line, on a keyboard. What none of them do is **aim the last climb
high into the finish trigger, which is tilted** — every metre of height trips
the clock about a metre further back in x. The entire margin over the world
record is **1.0 m of height and 4 km/h at the gate**.

## Both bests are 1-move optimal

Exhaustive single-move neighbourhoods against the plain oracle at full tick ×
full 255-value resolution: **249,747 candidates found nothing better than the
analog floor**, and 2,448 found nothing better than the keyboard 6323. That is a
different statement from "the search stopped improving".

## On difficulty, measured honestly

This first looked precision-bound: the analog margin is a non-separable
interaction between two stick positions 2 % off the stops, a second apart,
either of which *alone* is worse (one by 27 ms). Two measurements overturned
that reading:

- **The open-loop brittleness is the map's, not ours.** Sliding one gesture by a
  single tick DNFs 5 of 6 gestures on the **human world record's own tape**, and
  9 of 11 on the best human keyboard run. Ours is slightly *less* brittle than
  theirs. An open-loop tape is simply the wrong instrument for the first 3.4 s —
  46 people finish this map by reacting.
- **A keyboard reaches 6323.** Whatever the analog margin looks like, the
  drivable alphabet gets to within a millisecond of it.

## Negatives worth banking

Multi-op versus single-op A/B, concurrent on the same box, 45 min each from
6330: both 6330, zero improvements, ~540k evaluations each. Seeding from a
different basin converged 23 ms worse. Searching only the last 1.9 s: 285k
evaluations, 95 % finish rate, nothing. Quantising a *finished* analog tape DNFs
at 3, 5 and 9 levels, and replacing its four analog sweeps with the instant step
a keyboard physically produces gave **0 finishes in 82 placements** — keyboard
strategies must be searched *under* the constraint, never projected afterwards.

## Files

| file | what |
|---|---|
| `replays/BEST_KEYBOARD_6323.Ghost.Gbx` | **keyboard only, 23 inputs** — the one worth studying |
| `replays/HUMANWR_plus_early_flick_6342.Ghost.Gbx` | **the world record with one input changed** — beats the AT |
| `replays/BEST_6322.Ghost.Gbx` | fastest, unconstrained |
| `replays/KEYBOARD_23ev_6323.Ghost.Gbx` | the event-minimised keyboard tape |
| `notes/RESULT.md` | the full write-up, ablation table, trigger geometry, and a 22-step sector guide |
