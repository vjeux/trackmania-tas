# Fall 2024 - 25 (Pure Wet Icy Wood)

**Fall 2024 - 25 (pure wet icy wood)** — TAS **95.575** (+1.098) | AT 94.477 | WR 96.281 by iambeeen

https://github.com/user-attachments/assets/5a22e94e-20ee-44eb-b8a1-f76042d0dc56

*The clip is an earlier 96.068 tape — the same line, 0.493 slower than the
95.575 this page describes; it has not been re-filmed yet.*

TMX [210218](https://trackmania.exchange/maps/210218) · author time **94.477** ·
world record **96.281** (iambeeen) · **36 recorded runs**

| | time | vs AT | vs WR |
|---|---|---|---|
| **our TAS** | **95.575** | **+1.098** | **−0.706** |
| world record, iambeeen | 96.281 | +1.804 | — |
| author time | 94.477 | — | −1.804 |

**The author time is not beaten.** We are 0.706 s under the human world record
and 1.098 s over the author. This page is about why — and the why turns out to
be a genuinely unusual thing, which is that on this map the time exists and
cannot be spent.

---

## If you drive this map: sector 11 is where the time is

Sector 11 is the long one — 11.9 seconds, CP10 to CP11, and the single biggest
block of unclaimed time on the track.

**926 milliseconds are available there**, and we know that in two independent
ways that agree:

* a machine search of that sector alone, starting from the world record's own
  entry state, found **−926 ms** in twelve minutes;
* **rank 21 on the leaderboard already drives it 881 ms faster than the world
  record does.**

Those two numbers landing on top of each other is the interesting part. A TAS
optimum on its own tells you nothing about whether a person can hold the line —
machines routinely find time in places no human hand can reach. When the
machine's answer and a human's answer arrive at the same place with the same
number, the gain is a real property of the corner and not an artefact of
perfect inputs.

**What rank 21 does differently, and it is the whole map:** he does not slide.
On this surface the field drives at 21–30° of slip. Through sectors 6 to 17 he
is at **0.3–3.1°** — the slip angle of a world record on the *grippy* version
of this same layout. Same corners, same throttle, and in sector 11 he is
carrying the speed everybody else is grinding off sideways.

> **Arrive pointed where you are going and keep the wheel still.** Every degree
> of slide on this ice is speed you are throwing away.

The catch, and the reason nobody holds this time: that low-slip line is about
one unit of steering wide, and the driver who misses it is in the water. This
is a survival leaderboard — across all 30 runs, finishing position tracks
*respawn count* at +0.874. The top five have zero. Last place has 34.

## The state of the run

Our lap **is** the world record's lap — literally, to within a centimetre — for
its first 83.2 seconds. We re-simulated our tape inside the engine and compared
it to iambeeen's own recorded telemetry tick by tick:

| race second | distance between the two lines |
|---|---|
| 0 … 82 | ≤ 0.003 m |
| 84 | 0.199 m |
| 90 | 14.007 m |
| 95 | 95.836 m |

Everything we have ever won on this map — 706 ms across three sessions — was
won in the **last 12.4 seconds**. Sectors 1 through 14 have never been improved
by anything.

And the field's own per-sector bests, added up, come to **91.826** — 2.651
under the author time. Every sector of a winning lap has been driven by
somebody. Nobody has assembled one, and this page is the account of finding out
why.

## Why the time cannot be spent: the exchange rate

Here is the measurement that explains this map.

We ran the same search five times. Same starting tape, same operators, same
budget of about 24 minutes, same window of the lap open to editing. **The only
thing that changed was where we put the finish line.**

| time is measured at | how much the search finds |
|---|---|
| checkpoint 12 | **−947 ms** |
| checkpoint 13 | −251 ms |
| checkpoint 14 | −365 ms |
| checkpoint 15 | −90 ms |
| **the real finish** | **−10 ms** |

Nearly a second of real, driveable time exists at checkpoint 12. Ninety
milliseconds of it are still there three sectors later. **Ten survive to the
line.**

The same thing measured a different way: searching each sector on its own, from
the state we actually arrive in, finds **1.814 seconds** in 48 minutes of
compute — against a deficit of 1.098. Then 1.52 million evaluations aimed at the
real finish line bought **23 milliseconds**.

> **210218 is not short of time. It is short of a way to spend it.**

The mechanism is not mysterious, it is just brutal. This is an open-loop input
tape in a chaotic simulation: change one steering unit on one tick and the run
dies 69 % of the time, and the survivors come back to the same millisecond. Any
edit invalidates every input after it. So a gain in sector 11 is only worth
anything if the *next twenty-five seconds of unchanged driving happen to still
work* — and they almost never do.

We did convert one, end to end, which is worth stating as a number because it
is the exchange rate as a lived experience: 115 ms banked at checkpoint 15,
then a full rebuild of the last 7 seconds to recover from it. **Net gain:
−124 ms.** Everything else the search found upstream was paid straight back to
the tail.

## What is closed, and what it cost to close

Each of these is a resourced experiment with a control, not an impression.

| we tried | candidates | result |
|---|---|---|
| **the weld** — our first five sectors, then rank 21's entire tail, over every join point and phase | 77 | 0 finishers |
| **re-phasing the tail** after banking 467 ms in sector 14 (slide the remaining inputs earlier by 0–55 ticks) | 64 | **0 finishers** |
| the same at the exact tick the run dies | 84 | **0 finishers** |
| **blending** our tape toward the faster one, from 10 % to 95 % | 10 | 0 reach the next checkpoint — even a 10 % blend is fatal |
| **an exhaustive structural sweep** — every steering bias, gain and phase shift over a grid of windows across the last 24 seconds | **1 368** | 203 finishers, best **−1 ms** |

The first four say the same thing in four ways: **a fast arrival is not a fast
lap, and no amount of sliding the tail around converts one into the other.**
The fifth says the endgame is genuinely converged — combined with an earlier
session's exhaustive enumeration of all 470 016 single-input changes, this map's
last 24 seconds have absorbed nearly half a million deliberate edits and given
back 2 milliseconds.

## How you can tell this is the map and not our instrument

A negative result is only worth reading if the thing that produced it could
have detected a positive. This one could, and did, all night:

* Against the **real finish line**, the search resolved single milliseconds and
  kept doing so: **95.604 → 95.603 → 95.598 → 95.591 → 95.588 → 95.586 →
  95.575**, every step written to disk and re-validated.
* The **same binary, same starting tape, same operators**, pointed one
  checkpoint upstream, found **926, 371 and 467 ms** in twelve minutes apiece.
* Every one of those upstream winners reproduces our tape's time *exactly* at
  the checkpoint before the sector it edited, and is a **did-not-finish** on the
  real map. The instrument agrees where it must and diverges where it should.

So the failure to reach 94.477 is a statement about the map, not about the
detector.

One thing that went right and is worth recording: the search's own guard fired
once, refusing to bank a tape whose time it could not reproduce on a second
look. Re-checked by hand afterwards the tape was fine — it had hit the known,
rare case where the simulator returns a different answer on the same input — but
**a search that stops rather than bank a result it cannot reproduce is doing
exactly its job**, and that guard exists because an earlier session on another
map nearly published a time that was not real.

## Verification

Everything is re-simulated by Nadeo's own dedicated server on an untouched copy
of the map, one file per invocation, with a downloaded human ghost in the same
sweep as a control:

```
r01_96281  (world record, control)   96281
r02_103915 (rank 2, control)        103915
TAS                                  95575   ×3
```

The published ghost carries its own telemetry, regenerated out of the running
engine, so it plays back as the run it records rather than as the tape it was
built from. Nadeo's validator on the published file:

```
"Time" : 95575,   "IsValid" : true,   Can't load 0%,   Unvalidable 0%
```

## What would take this map

Not more compute on the endgame — that is settled to two milliseconds. The
prize is sector 11, it is worth 926 ms, and the obstacle is not finding the time
but carrying it home through twenty-five seconds of tail. That needs a repair
tool that works over that distance; the best one we have works over seven
seconds. Whoever builds it gets the author time, because the time is already
there and measured.
