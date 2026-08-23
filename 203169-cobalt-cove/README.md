# Cobalt Cove — reconstructing somebody else's tool-assisted run from video

**This page is about a run that is not ours.** On 2026-08-22 WirtualTV
published *[I Made A Theoretically Perfect Trackmania
Run](https://www.youtube.com/watch?v=F8tbqE2wV08)*: a tool-assisted run of
**1:12.589** on Nadeo's *Cobalt Cove*, built with Acepter's **Trackmania Input
Control Kit**. The run, the route and every idea in it are Wirtual's. Nothing
here is a claim to it. The question this page asks is a different one:

> Given only the published video — no ghost, no replay, no inputs, no telemetry
> — how much of that run can be recovered as a tape a simulator will accept?

TMX map [203169](https://trackmania.exchange/maps/203169) · Ubisoft Nadeo ·
"Map 12 of the Platform Discovery campaign" · uid `mA834Z8Nip9VGmiSn1I3mL2KAVa`
· internal name `PlatformWaterHFCInsideShort` · TM_Platform, 9 checkpoints ·
best replay uploaded to TMX **88.898** (Sapi, 8 respawns), 12 replays in all.

## The short version

| | |
|---|---|
| map identified | yes, exactly — from the narration, then confirmed on TMX by name, author and uid |
| the run's clock, in video time | race 0 at video **522.662 s ± 0.002**, playback rate **1.000** |
| speed of the run, at 60 Hz | **3341 of 4380 frames** read over race 0.005–73.0 s (76.3 %) |
| the run's inputs, recovered | **674 ticks — 6.7 s of 72.6** (race ≈55.4–58.1 and 58.2–64.4) |
| the map in our oracle | exact: the dedicated server re-simulates Sapi's 88.898 to the millisecond |
| per-tick engine state on this map | **located, and exact** — reproduces a ghost's own telemetry to a median **0.000 m** |
| reconstruction from race 0 | speed only: race **12.938 s**, five seeds inside a 0.45 s band. **With a positional gate: 12.380 s** — about half a second of the larger number was bought by driving off the track |
| where it fails | it leaves the pipe past CP1 at race ≈12.2 and falls 8 m; the speed objective keeps paying it while it falls |

The honest headline is the last line. Everything upstream of the search works,
and is controlled; the search itself gets a few seconds in and stops.

## 1. What the video actually gives you

The video is 14:12 at 2560×1440/60. The finished run is played once, in real
time and uncut, from 8:43 to 9:55. Everything before that is the run being
*built*, in clips at playback speeds between 0.1× and 1×.

Four readouts, in descending order of what they were worth:

**The map's own trackside scoreboard.** *Cobalt Cove* has a stadium screen that
displays the live race clock in milliseconds, and it is in frame at the end of
the run. Read at three frames — video 594.30 → 01:11.636, 594.50 → 01:11.838,
594.70 → 01:12.039 — it puts race 0 at video **522.662 s** with a spread of
2 ms across the three, and confirms the replay plays at exactly 1.000×. That
one measurement is what makes every other number in this page indexable by race
time. (It also means the finish, 72.589, falls at video 595.251.)

**The speed readout**, bottom right, three right-aligned digits, no leading
zeros. Readable on 76.3 % of the run's frames. The other quarter is not faint —
it is *absent*: white text over the map's pale surfaces, with nothing behind
it. That is a property of the recording, not of the reader.

**A five-lamp key overlay** — BRAKE and the four arrows — composited by the
video editor at a fixed screen position, in two flat greys. It is on screen for
163.4 s across 14 clips. Steering is two lamps and never anything in between,
so this is a digital tape. BRAKE and the down arrow carry the same signal on
**1582 of 1635** overlay frames; where they differ it is in sustained stretches
of a third of a second and more, not at transitions, so they are two channels
and not one signal drawn twice — what the second one is has not been
established.

**The game's own race clock**, bottom centre. Usable, but only on clips the
editor did not reframe, and only where the background is not white.

What the video does **not** give you: the car's position. There is no overlay,
no minimap, no coordinates. Position has to come from the physics.

## 2. Which clips are even the same run

He built the run over about six hours, so a clip at race time *T* is usually an
*earlier* version of the run at *T*. Deciding which clips show the finished run
is the whole difficulty of using the construction footage, and it is decided
physically: read the clip's own speed readout, place it against the finished
replay's speed trace by searching (rate, offset), and see whether a placement
exists that fits.

* **The instrument works.** Fed the reference's own table it returns rate
  1.000, the right offset, 99.6 % agreement, and the best competing placement
  at 43.7 %.
* **It declines when it should.** On clips of earlier builds it finds no peak —
  fit 30–32 % against a 28–30 % runner-up — and the direct speed comparison at
  those clips' own clock times shows a median disagreement of tens of km/h.
* **Two instruments, different pixels, same answer.** On the clip at video
  405.1–419.0 the clock OCR gives rate 0.698 with race 50.13 at video 405.2;
  the speed aligner, which reads a different corner of a different frame,
  independently gives rate 0.695 and race 50.20. Neither can borrow from the
  other.

Only two clips place on the finished run with a distinct peak:

| clip (video s) | playback rate | race window | fit | best rival placement |
|---|---|---|---|---|
| 494.5–497.6 | 0.619–0.670 | 55.34–58.11 | **91.6 %** | 53.5 % |
| 498.4–506.5 | 0.766 | 58.15–64.35 | 56.6 % | 43.8 % |

and one is contradicted: 405.1–419.0 places at 58.7 % against a 34.1 % rival,
which is a peak — but where it overlaps the 494.5 clip in race time, the two
records' steering does not reconcile at any shift (gas and brake agree on
100 %, `left` never above 55 %, `right` never above 80 %). Two records of the
same instant that disagree are two different runs; the 494.5 clip has by far
the better speed fit, so **405.1–419.0 is an earlier build** and its keys are
not in the recovered record.

## 3. What was recovered

`video/kt_493.tsv` and `video/kt_498.tsv` in the bank: **674 ticks** of the
finished run's key states, race ≈55.4–58.1 and 58.2–64.4, with no
frame-to-frame disagreement inside either record. As an event list
(`ev_493.txt`, `ev_498.txt`) they read the way a TAS input script reads.

That is **6.7 seconds of a 72.6-second run — 9 %**. It is the ceiling this
video supports, not a ceiling of the method: the finished replay, the only
continuous recording of the run, carries no input overlay at all.

One physical corroboration, on the second record: its brake ticks lose
**2.52 km/h per 100 ms** on average and not one of the 21 gains more than
2 km/h. On the first record the corroboration is unavailable rather than
negative — there are no brake ticks in it, and it covers the plastic-bounce
section where the speed is set by collisions and not by the pedals.

## 4. The oracle, and the thing that had to be fixed to use it

The dedicated server re-simulates *Cobalt Cove* exactly, moving blocks,
water, respawns and all: `ghost verify` on Sapi's replay passes every check and
the server returns 88.898 against a declared 88.898.

The per-tick engine state — the readout a search has to score against — did
**not** work here at first, and the diagnosis is worth keeping:

1. `fk trace` refused: *"best candidate is not self-consistent enough (median
   |d(pos)/dt − v| 25.80 m/s at mean speed 52.2)"*. The car on this map at that
   instant is doing 23.7 m/s, so the candidate was not the car.
2. `FK_ANCHOR` — accept only a slot whose first sample is within *r* of a
   position the car is known to occupy — fixed it, but only at a **tight**
   radius. At r = 4 m it still adopted a decoy sitting within four metres of the
   car and spinning (the ranking prefers angular travel, which is exactly what a
   wheel does). At r = 1.2 m the candidate becomes verr **0.78 m/s**, |q|−1
   9.9e−8, mean speed 25.4 m/s — the car.
3. It was still refused, by a gate calibrated elsewhere: acceptance needs
   `verr < max(0.02·speed, 0.25)`. On this map the position-versus-velocity
   residual is 0.19–1.01 m/s depending on where you fork. `FK_VERR_MAX=1.0`
   accepts it.
4. **The control.** Fork at race 9.25, trace to the end, and compare against the
   ghost's own recorded telemetry: 1595 shared instants, position median
   **0.000 m**, p95 0.000 m, **max 0.010 m**; speed median 0.06 km/h. The slot
   is the car. (`tmtraj csvdiff`, added for this.)
5. The slot sits at a **fixed offset from the server's base — `base − 8183260`
   — on this map**: the same address on every fork of every probe tick, and for
   two different ghosts. `FK_STATE_OFF` takes it directly, which removes both
   the 50-second sweep and the need for an anchor on a tape whose positions you
   do not yet know. A full-run per-tick trajectory then costs **3.3 s**.
   **It is not a property of the binary.** On a different map under the same
   server the honest locate returns `base − 602416 / −602640 / −603104 /
   −602912` at different probe ticks — not constant even within that map — and
   Cobalt Cove's offset puts the car 3.5e9 m away there. What does transfer is
   the safety: a wrong offset is caught loudly by `fk`'s own self-check
   (`|q|−1 = 3.5e9`, "not a unit quaternion") and no trajectory is handed back,
   so trying one costs nothing and can never silently lie.
6. Forking early is less exact than forking late: from race 0.25 the same
   comparison gives a median **0.361 m** (p95 0.850 m) rather than 0.000 m. The
   reconstruction below runs on early forks and inherits that.

## 5. The reconstruction, and where it stops

With the video's speed trace as the objective and the engine as the simulator,
the reconstruction is a search: grow a digital tape forward for as long as its
simulated speed stays within 8 km/h of the video's, mutating only around the
race time where it currently stops tracking. `tools/recon` does that.

**The scorer had to be fixed before any of it meant anything.** Off the start
line this car gains 255 km/h per second, so a ten-millisecond difference in
timing is four km/h of apparent error. Under a nearest-instant comparison, two
traces of the *same tape* scored 2.638 s and 11.005 s. Comparing against the
closest VALUE inside a ±50 ms window — the sub-tick and frame-quantisation
uncertainty — makes it deterministic: five traces of one tape now score
identically, and the traces themselves are bit-identical.

**Calibrate the floor.** How long does a run that is emphatically *not*
Wirtual's keep his speed? Every human replay on TMX, simulated from race 0:

| run | tracks the video to |
|---|---|
| Sapi 88.898 | 11.021 s |
| Zai 105.385 | 10.888 s |
| Elya 114.975 | 10.288 s |
| Bren 105.569, RCinCHgamer01 116.065 | 10.005 s |
| roaSone9 107.444 | 3.521 s |
| *our gas-only tape, no steering* | *3.538 s* |

So **the first ten seconds of Cobalt Cove are forced**: everyone drives the
launch the same way, and speed alone cannot tell them apart. Anything at or
below ~11 s is free.

**From an empty tape the search cannot even reach the floor.** Sixty rounds of
64 candidates take a gas-only seed from 3.538 s to **4.055 s** and stop — worse
than five of the six humans. That is the honest measure of how little a speed
trace guides a search that has to invent the driving: the launch ramp carries
no information, so the search wanders on it.

**Seeded from a real run it goes past the floor — and every seed hits the same
wall.** Give the search a public human replay's opening (`--keep-before`, that
driver's inputs to race 8.0 s) and let it search forward. Five drivers, five
RNG seeds, independent runs:

| seed | that human alone | after the search |
|---|---|---|
| Bren 105.569 | 10.005 s | **12.488 s** |
| Sapi 88.898 | 11.021 s | 12.671 s |
| Zai 105.385 | 10.888 s | **12.938 s** |
| Elya 114.975 | 10.288 s | 12.738 s |
| RCinCHgamer01 116.065 | 10.005 s | 12.588 s |

The seeds span a full second and finish inside a **0.45 s band**. The search
adds 1.5–2.5 s to whatever it is given and then stops — for 110 rounds in the
first run and 60 in each of the others. **The wall is a property of the
objective, not of the seed or the RNG**, which is a much stronger statement
than any one run's number.

Best on the speed objective alone: **race 12.938 s of 72.589**, mean |diff|
3.97 km/h — but see §6, where about half a second of that turns out to have
been bought by driving off the track.

Both an independent scorer (`vidread enginecmp`) and `recon`'s own agree
exactly on the same trace, which is worth saying because they are two
implementations of one statistic and this project has been bitten by that
before.

**Where the wall is, on the track.** Just past checkpoint 1 (9.401 s on the
human reference), in the pipe turn Wirtual describes as "at real speed this
just looks ridiculous the way it's driven". The video's car holds 172–176 km/h
through it and then *accelerates* to 188; every candidate we can build bleeds
away — 174 → 154 → 130 → 99 → 67 over the next two seconds. §6 says what is
actually happening there, and it is not a grip problem.

**Two things it is not.** It is not Wirtual's tape: it is a tape whose speed
matches his for 12.9 s, seeded by somebody else's driving, and where the two
agree in speed they may still be metres apart. And it is not something more
compute fixes — five independent searches say so.

**Why it stops.** The objective is a scalar per instant. Speed cannot tell a
car that is on the line from one three metres left of it, so the search buys
tenths of tracking with a line that is going somewhere else, and in a pipe turn
that is immediately fatal. The missing observable is position.

## 6. The wall, diagnosed: the candidate is off the track

"Bleeds speed in the corner" was the wrong description, and the right one was a
plumb line away. The pipe turn past CP1 is a **Metal** surface at y ≈ 40–42
(`mapgeom plumb` at ten points along a human's line through it: Metal at every
point that has a triangle at all). Here is where the best speed-only candidate
actually is, against Sapi's line at the same instants:

| race | distance from the human line | height difference |
|---|---|---|
| 11.000 | 1.6 m | −0.3 m |
| 12.000 | 3.5 m | +0.1 m |
| 12.500 | **16.0 m** | −3.8 m |
| 13.000 | 42.1 m | −12.2 m |
| 14.000 | 106.6 m | −10.8 m |

It leaves the pipe at about race 12.2 and falls eight metres onto the concrete
below — `plumb` at its 12.75 position finds no surface until Concrete at
y 33.0, where the car is at y 33.7. **And the speed objective kept paying it
for another 0.7 s while it fell**, because a falling car passes through the
right speeds on the way down. The plateau was never a search-budget problem and
it is not really about corner grip either: the objective admits cars that are
not on the track.

### A positional constraint that needs no camera pose

The video does not say where the car was. **Four human replays do say where a
car can be**, and on the stretch where this run follows the normal route — CP0
to CP2, which Wirtual's own commentary confirms — a candidate outside the
envelope of the human lines has left the track. `recon --corridor` adds exactly
that: distance to the nearest human position at the same instant, and the score
becomes the earlier of "stopped tracking the speed" and "left the corridor".
`--corridor-to` is where the caller states the route stops being shared, and
past it the corridor makes no claim.

Three controls, and the middle one is the one that matters:

* **A human's own run is not penalised.** Zai's untouched tape scores 10.771 s
  with the corridor and 10.771 s without it.
* **A HELD-OUT human is not penalised.** Bren, scored against a corridor built
  from the other three drivers only, gets 9.605 s with the corridor and 9.605 s
  without — at tube radii of 3, 5 and 8 m alike. So the corridor is not merely
  memorising the runs it was built from.
* **The off-track candidate is caught, and caught earlier as the tube tightens**:
  12.938 → 12.270 (8 m) → 12.130 (5 m) → 11.980 (3 m). A gate that responded to
  its own parameter in the wrong direction, or not at all, would be measuring
  something else.

### What the honest number is

Re-running the search with the corridor active (tube 3 m, two seeds, 90 rounds
of 48):

| | speed only | speed + corridor |
|---|---|---|
| Zai-seeded | 12.938 s | **12.130 s** |
| Sapi-seeded | 12.671 s | 12.380 s |

**So about 0.5 s of the previous best was bought by leaving the track.** The
honest figure for this arm is **race 12.380 s of 72.589**, and it is a smaller
number that means more than the larger one did.

Two things this does not do. It does not extend the reconstruction — the
corridor can only ever lower a score, never raise one. And it stops being
evidence where Wirtual reroutes; everything above is inside CP0–CP2, where his
own narration says he drives the normal way.

## 7. Position — where the geometry stands

`tools/mapgeom` is the positional observable in the long run. Graded here
against four human replays it covered **60.3–65.1 %** of driven samples with a
0.25–0.53 m vertical fit — the weak half of its range. The geometry arm has
since reported that its current head reaches **80.3 % raw / 87.9 % of the
samples the model owes**, at a **0.101 m** median gap on this exact map and
replay, which is comfortably enough for a pose solve; that work is not on
`main` at the time of writing and the numbers in the table above are what this
box could measure.

Two corrections to what I concluded from the older tool, both from that arm and
both worth carrying:

* **The moving blocks were not the cause.** Opening `CPlugDynaObjectModel`
  moved this map's coverage by 0.2 points, and exactly 1 of 1457 samples rests
  on one. They are drawn at rest pose and reported separately, never averaged
  in — a swept hull is worse than useless as a ride-height probe.
* **What was missing is water, and it is physics rather than a hole.** A car on
  water sits **0.900 m under** the plane (measured on this map's own 8 m-boundary
  water planes, four times out of four), so a downward plumb read every water
  sample as uncovered. `PlatformWaterHFCInsideShort` is this map's internal
  name and a third of the run is on water — **any position observable derived
  from this geometry has to carry the same 0.900 m convention**, or a pose solve
  sits a metre high over every water section.

**But the pipe turn is not a water problem, and I checked rather than assuming**:
ten plumb points along the human line through it return Metal at y 40–42 at
every point that has a triangle, and two of the ten have none. The failure
there is the car being off the surface entirely, not on the wrong one.

## 8. What is banked

`~/persistent/private-30d/tm-wirtual-perfect/`

| | |
|---|---|
| `video/wirtual_speed_race.tsv` | the run's speed at 60 Hz, indexed by race time — the headline artefact |
| `video/kt_493.tsv`, `kt_498.tsv`, `ev_*.txt` | the recovered key states, per tick and as events |
| `video/tmpl_eye.txt`, `tmpl_timer.txt` | the digit templates, and `lampsurvey2.tsv`, the overlay census |
| `map/` | the map, all 12 TMX replays, and Sapi's decoded telemetry |
| `engine/` | full-run engine traces of Sapi's run, forked at race 0.25 and 9.25 |
| `recon/` | the baseline tape, its trace, and its comparison against the video |

Tools added for this, all in `tools/`: **`vidread`** (read a run off a screen
recording), **`recon`** (grow a tape against a speed trace), `ghost tape
script` (an event list to a tape) and `tmtraj csvdiff` (two trajectories on the
instants they share).

## 9. Attribution

The run is **Wirtual's**, on **Nadeo's** map, made with **Acepter's**
Trackmania Input Control Kit. Video:
<https://www.youtube.com/watch?v=F8tbqE2wV08>. No file produced here carries
his login, account id or nickname, and none of it has been or will be uploaded
to a leaderboard.
