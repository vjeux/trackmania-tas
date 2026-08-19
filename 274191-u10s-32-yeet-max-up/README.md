# U10S_32 [Yeet] MAX-UP — the reactor flight is fifteen keyboard presses

**Author time 7.704 · human world record 7.893 · this run 7.463 — the author
time beaten by 0.241 s, and a 15-press keyboard tape is still 0.228 inside it.**

| tape | time | vs AT | inputs | alphabet |
|---|---|---|---|---|
| [`TAS_7463`](replays/TAS_7463.Ghost.Gbx) | **7.463** | **−0.241** | analog | 255 values |
| [`KEYBOARD_7474`](replays/KEYBOARD_7474.Ghost.Gbx) | 7.474 | −0.230 | 24 presses | **3 — keyboard** |
| [`KEYBOARD_7476`](replays/KEYBOARD_7476.Ghost.Gbx) | **7.476** | **−0.228** | **15 presses**, 30 ms hold floor | **3 — keyboard** |
| fewest inputs of all | 7.558 | −0.146 | **32 live events for the whole lap** | keyboard |
| human WR — Whatever8319 *(control)* | 7.893 | +0.189 | — | — |

TMX map [274191](https://trackmania.exchange/maps/274191) · author **Everios96**
· **3 recorded runs** — all three re-simulate exactly, so the field check here
covers the entire recorded population.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The physics everything rests on

This is a **Reactor** map. In the air the car is pushed at a constant **~44 m/s²
along its own −up axis** and nothing else — 87% of the acceleration on one body
axis, cosine −0.86, consistent across all three human tapes.

So **attitude is the only control**. The thruster is bolted to the car's belly:
where the belly points is where you go.

And there is a **dead zone — 1.2 seconds in which the steering wheel does
nothing.** Measured at 20 ms resolution, 183 substitutions on each of two
independent tapes, all returning the base millisecond. Both the world record and
rank 2 spend that time holding full lock, which is the clearest possible sign
that nobody knows.

**The pedals are not dead there.** Gas and brake are live pitch controls in the
air — this is a three-pedal map, not a steering-only one. (That distinction
matters generally: the dead zone is a property of a *channel on a lineage*, not
of the map. The fast tapes put brake taps inside the window and made it partly
alive.)

## The technique — undiscovered, and it happens on the ground

Because the wheel is dead for 1.2 s, **the attitude at which you meet the
launcher is fixed by the rotation you carry off the lip.** Nothing after the lip
can change it.

| | leaves at | arrives | keeps |
|---|---|---|---|
| human WR | 3.80 rad/s | nose-up **58°** | 148 km/h |
| this run | 3.58 rad/s, different axis | **35°** | **197 km/h** |

**All three humans slam to full lock and pin it into the edge.** The fast line
does the opposite: **unwind ~15/127 of lock over the last 100 ms of road, and
brake 20 ms later.** Worth 46 ms directly — and it unlocks 288 ms more, because
from a better launch you can hold the thruster flat (+17…+25° against the
human's +53°).

## The driving guide

Full throttle from the line.

**Sector 1 — the road, 0 → 2.24 s.** Drive it exactly like the world record.
Our tape *is* the world record's own inputs here, and one tick of difference
anywhere does not finish.

**Sector 2 — the last 0.65 s of road, 2.24 → 2.89 s. The only hard part.**
You are turning left onto the edge at about 260 km/h.

1. Turn in a fraction harder and earlier — about 10% more lock from 2.5 s than
   feels natural.
2. Do not fully unwind at 2.70 s; the world record goes to centre here, keep a
   whisker of left in it.
3. **Do not slam to full lock into the edge.** Reach peak lock at about 2.77 s,
   *just short of the stop*, then **feather out of it all the way to the lip** —
   roughly a tenth of the wheel unwound over the last two tenths of a second.
   Brake as you begin unwinding, not before.

**The cue is the edge of the road, not a clock: you should be unwinding as the
front wheels reach the lip, not fighting the stop.**

**The check is a number on the speedo.** At the instant the launcher throws you,
about 3.60 s:

| speed | what it means |
|---|---|
| 148 km/h | you drove it like the current world record → ≈ 7.89 |
| 170 km/h | about the author time |
| **197 km/h** | the fast line |

**Sector 3 — the fall, 2.89 → 4.10 s.** Hands off the wheel; feet still working.
Steering is ignored. The fast tapes let the brake off briefly at ~3.53 s and
again into 3.83 s, straddling the launcher strike at 3.60 s, and those are
10 ms-tight — if you want the simple version, hold the brake like the world
record does and give up a few hundredths. The 0.228 s is not in these.

**Sector 4 — the climb, 4.10 → 5.90 s. Fifteen presses, and all of the time.**
One idea:

> **Stop the car swinging nose-up. Point its belly at the far side of the map,
> not at the sky.**

The reactor pushes out through the floor at a constant 4.5 g. Belly aimed 50° up
and half of it goes nowhere; aimed 20° up and it goes where the gate is. **The
horizon is your instrument** — the field lets the nose keep climbing; you check
it and hold it low.

```
  4.10 s  R      as control returns — the catch
  4.41 s  —
  4.51 s  L
  4.61 s  —      <-- the "check the nose" release
  4.84 s  L      .......... 320 ms
  5.16 s  —
  5.25 s  R
  5.31 s  —
  5.39 s  L      .......... 490 ms, the long one
  5.88 s  —      from here on nothing is worth more than a few ms
  6.65 s  L      (30 ms tap)
  6.68 s  —
  7.15 s  R      (60 ms tap)
  7.21 s  —
  7.49 s  L      hold to the gate
```

The first four are one phrase — *catch, check* — and only the total rotation
matters, not which of them you were late on. From 5.16 s everything has ±30–50 ms
of room. **From 5.88 s the lap is over: the last five presses cost zero however
you place them.**

**Sector 5 — already decided.** 98% of the gain is banked by 5.5 s. Freeze
everything from 7.40 s and the time does not change.

## Is this humanly realistic? Yes — and the hard part is the part they already do

Sector 1 is the world record's own driving. Sector 3 needs no steering at all.
Sector 4 is fifteen keyboard presses on quarter-second holds with tens of
milliseconds of slack. The single demanding moment is the feather in sector 2,
and it is a change of *habit* rather than of precision: stop pinning the wheel
into the stop.

## A false negative worth knowing about

The first answer to "is there a low-input family here?" was **no** — sixteen
configurations tried (keyboard, 5/9/17-level, hold floors from 20 to 50 ms), all
sixteen DNF.

That conclusion was an artefact. **All sixteen also DNF when applied to the
human world record's own tape** — the instrument could not say yes, so its "no"
meant nothing. Projecting only the *steered climb* rather than the whole tape
produced a keyboard tape **0.127 s inside the author time with no search at
all.**

The difference between "impossible" and "free" was one flag, and the diagnostic
was sitting in plain sight: when your test destroys a run three humans have on
the board, you are measuring the test.

## Validation

Whole family re-validated in one batch on two independent binaries with the
human world record as a known-answer control; 18 champions banked through an
independent ratchet; **zero phantoms all session**. Field reproduction **3/3 —
100% of the recorded population** (7.893 / 7.933 / 8.597, all exact).

## Files

| file | what |
|---|---|
| `replays/KEYBOARD_7476.Ghost.Gbx` | **fifteen presses, 7.476** — the one to learn |
| `replays/TAS_7463.Ghost.Gbx` | the unconstrained floor |
| `replays/KEYBOARD_7474.Ghost.Gbx` | 24 presses, 7.474 |
| `notes/RESULT.md` | the full write-up: physics, ablation, tolerance tables, negatives |
| `notes/PLAN-v1.md` | pre-search analysis, with its predictions scored honestly afterwards |
