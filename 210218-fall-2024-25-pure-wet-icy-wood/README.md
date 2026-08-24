# Fall 2024 - 25 (Pure Wet Icy Wood)

**The whole map is slip angle. The field slides through the middle sectors at
21–30°; rank 21 drives the same corners at 0.3–3.1° and takes 881 ms out of the
world record in sector 11 alone. Arrive pointed where you are going and keep the
wheel still.**

**Fall 2024 - 25 (pure wet icy wood)** — TAS **UNKNOWN — 95.507, 95.575 or 96.068, see below** | AT 94.477 | WR 96.281 by iambeeen

> ### ⚠️ UNKNOWN 2026-08-24 — this map is published with three different "our best" numbers and no file for two of them
>
> This is not a rounding disagreement. Three numbers are in print for the same
> thing, none of them retracted, and **the honest state is that nobody here can
> currently say which is this project's best validated lap on this map.**
>
> | number | where it is published | what backs it |
> |---|---|---|
> | **95.507** | the root [`../README.md`](../README.md), *"this TAS"* | **nothing in this directory.** `tmtraj corpus shipped` reads it `UNSHIPPED` |
> | **95.575** | this page's own caption and table, below | **nothing in this directory** |
> | **96.068** | `replays/TAS_96068.Ghost.Gbx` | its own header states it, and it is the tape the clip on this page was shot from |
>
> **MEASURED:** `tmtraj corpus shipped --root .` reads this directory's two
> files and the fastest header among them is **96.068** — 0.561 slower than the
> front page's claim and 0.493 slower than this page's. *Control:* the same
> scan reads 26 other maps' claims as `BACKED` by a file that states the time,
> so it is capable of finding a backing file when one exists.
>
> **What is NOT in doubt.** The author time is not beaten here on any of the
> three numbers — 94.477 is under all of them — and 96.281 is the human world
> record. Nothing in the rest of this page depends on which of 95.507 and
> 95.575 is right: the sector analysis, the slip-angle result and the
> spend-it-backward argument are all measured against the world record's lap
> and against the field, not against our headline.
>
> **What would settle it, and it is a small job.** Re-simulate the search tape
> through the plain oracle and read the time it returns; then either publish
> the file it produces in `replays/` and print that number in both places, or —
> if the tape cannot be found — retract both 95.507 and 95.575 in favour of the
> 96.068 that has a file. Until one of those happens **the only number on this
> map a reader can check for themselves is 96.068**, and that is the one to
> quote if you need a defensible figure today.
>
> A note on how this happened, because it is the reusable part: a *search tape*
> is a validated time before it is a renderable ghost, and this project has
> published the time in one place, a refinement of it in another, and a file
> for neither. `tmtraj corpus shipped` exists to catch exactly that, and it did.

https://github.com/user-attachments/assets/5a22e94e-20ee-44eb-b8a1-f76042d0dc56

*The clip is the 96.068 tape — the fastest run this directory publishes as a
file, and 0.493 slower than the 95.575 the page below describes. See the UNKNOWN
banner: it has not been re-filmed, and there is nothing here to re-film it from.*

> ### The two files in `replays/` were suspected of sharing a recording. They do not — MEASURED, and the suspicion is retracted.
>
> **What raised it (2026-08-22).** `tmtraj diff` on the pair reads:
>
> ```
> A 1922 samples, declared 96.068   B 1922 samples, declared 96.078
> compared 1922 shared instants: 1803 bit-identical (93.8 %), worst separation 2.414 m
> VERDICT IS-THE-REFERENCE: this file's telemetry is that recording
> ```
>
> and the two input tapes **genuinely differ at 731 ticks**, spread right across
> the run (110 in race 0–10 s, 98 in 10–20, 88 in 20–30, 190 in 50–60, 140 in
> 70–80). Identical positions across differing inputs looks exactly like one
> file carrying the other's recording, and this page said so for about an hour.
>
> **What settles it.** Re-simulate *each file's own tape* through the engine and
> ask how far apart the two cars are **on the samples where the two records
> agree bit for bit** (`fk trace` ×2, then `tmtraj adjudicate`):
>
> ```
> THE DECISIVE ONE -- over the 1735 samples where the two RECORDS are
> bit-identical, the engine puts the two cars at most 0.0001 m apart
> (1734 of those 1735 are bit-identical in the simulation too, 99.9 %)
> ```
>
> **The records agree because the car really is in the same place.** Those 731
> differing inputs have no authority where they differ — which on a map driven
> at 21–30° of slip, with the wheel doing very little for long stretches, is
> exactly what one would expect. Both files are sound; the 96.078 is a genuine
> 1-minimal variant of the 96.068 and its record is its own.
>
> **Why the summary numbers could not settle it, which is the reusable part.**
> The records agree on 93.8 % of samples and the simulations on 94.1 % — two
> rates that look like confirmation and say nothing, because they do not say
> whether those are the *same* samples. They are, but that had to be measured.
> `tmtraj adjudicate` prints the restricted statistic for that reason.

TMX [210218](https://trackmania.exchange/maps/210218) · author time **94.477** ·
world record **96.281** (iambeeen) · **42 recorded runs** (board 2026-08-24; the
field measurements on this page were taken over the 36 recorded then)

| | time | vs AT | vs WR |
|---|---|---|---|
| **our best lap — 95.507 or 95.575, UNKNOWN which** | **95.507 / 95.575** | **+1.030 / +1.098** | **−0.774 / −0.706** |
| our fastest **published file**, `replays/TAS_96068.Ghost.Gbx` | 96.068 | +1.591 | −0.213 |
| world record, iambeeen | 96.281 | +1.804 | — |
| author time | 94.477 | — | −1.804 |

**The author time is not beaten on this map**, on any of those numbers. Our best
validated lap is 1.030–1.098 s **over** the author time and 0.706–0.774 s under
the human world record — fastest ever driven here, and still short of the medal.
The reason is unusual enough to be the subject of this page: on 210218 the time
exists and cannot be spent.

> *Retracted, kept in place:* this table used to read *"our TAS — 95.575 —
> +1.098 — −0.706"* as a single settled row. See the UNKNOWN banner at the top
> of the page: the front page says 95.507, this page said 95.575, and there is
> no file for either.

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
somebody. Nobody has assembled one.

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

One conversion has been made end to end, and it is the exchange rate as a
number: 115 ms banked at checkpoint 15, then a full rebuild of the last 7
seconds to recover from it. **Net gain: −124 ms.** Everything else the search
found upstream was paid straight back to the tail.

## What is closed, and what it cost to close

| we tried | candidates | result |
|---|---|---|
| **the weld** — our first five sectors, then rank 21's entire tail, over every join point and phase | 77 | 0 finishers |
| **re-phasing the tail** after banking 467 ms in sector 14 (slide the remaining inputs earlier by 0–55 ticks) | 64 | **0 finishers** |
| the same at the exact tick the run dies | 84 | **0 finishers** |
| **blending** our tape toward the faster one, from 10 % to 95 % | 10 | 0 reach the next checkpoint — even a 10 % blend is fatal |
| **an exhaustive structural sweep** — every steering bias, gain and phase shift over a grid of windows across the last 24 seconds | **1 368** | 203 finishers, best **−1 ms** |

**The last row is the positive control for the first four**, and it is worth
saying so out loud: the same machinery, on the same map, returns **203
finishers** out of 1 368. So the four zeros above are results about those
operators and not about a search that had stopped working. Two limits on how far
to push them: the control is *not* budget-matched (77 / 64 / 84 candidates
against 1 368), and this map has **16 checkpoints**, which is where the old
`FINISH_BASE = 1e8` bug made a deep DNF outrank a finished lap — so any null
here taken from a search log predating `tools/LINEAGE.md`'s single lineage
should be re-run before it is leaned on.

The first four say the same thing in four ways: **a fast arrival is not a fast
lap, and no amount of sliding the tail around converts one into the other.**
The fifth says the endgame is genuinely converged — combined with an earlier
session's exhaustive enumeration of all 470 016 single-input changes, this map's
last 24 seconds have absorbed nearly half a million deliberate edits and given
back 2 milliseconds.

The negative has a positive control on both sides. Against the **real finish
line** the same search resolves single milliseconds and kept doing so —
**95.604 → 95.603 → 95.598 → 95.591 → 95.588 → 95.586 → 95.575**, every step
written to disk and re-validated. Pointed one checkpoint upstream, the same
binary, tape and operators find **926, 371 and 467 ms** in twelve minutes
apiece — and every one of those upstream winners reproduces our time exactly at
the checkpoint before the sector it edited and is a did-not-finish on the real
map. So the failure to reach 94.477 is a statement about the map.

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

## Files

| file | what |
|---|---|
| `replays/TAS_96068.Ghost.Gbx` | the tape the clip above is filmed from — the same line, 96.068 |
| `replays/TAS_96078_1minimal.Ghost.Gbx` | the 1-minimal variant, 96.078 |
