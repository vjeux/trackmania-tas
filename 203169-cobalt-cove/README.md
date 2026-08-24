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
Live Nadeo board, fetched 2026-08-24 03:32–03:37 UTC: **564 records, best
98.857 by SmallPumpk1n** — nothing on this page is measured against it, and
none of it is ours.

## The short version

| | |
|---|---|
| map identified | yes, exactly — from the narration, then confirmed on TMX by name, author and uid |
| the run's clock, in video time | race 0 at video **522.662 s ± 0.002**, playback rate **1.000** |
| speed of the run, at 60 Hz | **3341 of 4380 frames** read over race 0.005–73.0 s (76.3 %) |
| the run's inputs, recovered | **674 ticks — 6.7 s of 72.6** (race ≈55.4–58.1 and 58.2–64.4) |
| the map in our oracle | exact: the dedicated server re-simulates Sapi's 88.898 to the millisecond |
| per-tick engine state on this map | **located, and exact** — reproduces a ghost's own telemetry to a median **0.000 m** |
| reconstruction from race 0 | speed only: race **12.938 s**, five seeds inside a 0.45 s band. With a positional gate: 12.380 s. **With the wetness objective as well: race 12.480 s** (§9.4), measured under three observables instead of two, against 12.330 for the previous best tape re-measured the same way |
| where it fails | it leaves the pipe past CP1 at race ≈12.2 and falls 8 m; the speed objective keeps paying it while it falls |
| the next observable | **tyre wetness** — a positional integral, on screen, and now READ OUT OF THE ENGINE at `car+180` (95.4–96.0 % exact, negative control declines at 44 %) |
| the wetness reader | **finished** (§9.1): 574 readings, **zero** violations of the dry-out law; the run is 100 % wet to race 9.9, resets at 9.95, and crosses water again at 22.4, 39.4 and 50.0 |
| what the wetness objective bought (§9.2, §9.4) | it **confirms** the old number rather than lowering it, dates the reroute, and — once the dry window the HUD does not draw is asserted (§9.3) — takes the reconstruction to **12.480 s** under three observables |
| the readout after that | **`! Slip`**, 40 % of the run's frames and present in every five-second bin — located, not calibrated (§9.3) |

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

## 7. Position — where the geometry stands, and one gate that does NOT work

`tools/mapgeom` is the positional observable in the long run. My first grading
of it here (60.3–65.1 % of driven samples, 0.25–0.53 m fit) was of an older
build. Re-measured **on this box** against the geometry arm's `mapgeom2`
(`15db6bf`, fetched over ssh — `origin/main` is stuck at `0836d2c` while the
render box is down):

| replay | over a surface, raw | of the samples the model owes | median gap |
|---|---|---|---|
| Sapi 88.898 | **80.3 %** | **87.9 %** | **0.101 m** |
| Bren 105.569 | 79.3 % | 85.1 % | 0.090 m |

That is comfortably enough to fit a pose solve on, and it supersedes the table
I published earlier. Two corrections from that arm, both worth carrying:

* **The moving blocks were not the cause.** Opening `CPlugDynaObjectModel`
  moved this map by 0.2 points; 1 of 1457 samples rests on one. They are drawn
  at rest pose and reported separately — a swept hull is worse than useless as
  a ride-height probe.
* **What was missing is water, and it is physics rather than a hole.** A car on
  water sits **0.900 m under** the plane. `PlatformWaterHFCInsideShort` is this
  map's internal name and 9 % of a human run here is over water, so **any
  position observable derived from this geometry must carry that convention**
  or it sits a metre high over every water section.

The residue is now inflatables (`InflatableMat1mCurve2`, `InflatableTube*`) and
`RoadWater*` — all of which the map has and the model draws, so they are
placement gaps rather than missing readers.

### The general on-surface gate, and why it is a checker and not an objective

The obvious generalisation of §6 is "a candidate with no triangle under it is
dead", enforced every tick. I built it (`recon onsurface`) and **it fails its
own control on this map**, in two independent ways:

* **A held-out human fails it.** Bren, plumbed at 24 instants where Sapi is on
  a surface, is off one at 4 of them — because the model still has holes. A
  gate that kills 17 % of a real human run is not a gate.
* **Falling off the track lands you on another surface.** The candidate that
  leaves the pipe reads `on Concrete` from race 13.8 onward: it is on the floor
  of the stadium, eight metres below the road, and perfectly "on a surface".

So the honest statement is the narrow one: **on this map the human corridor is
the working positional constraint and the on-surface test is not.** The
checker is committed because it is the right test to run before believing a
geometric gate anywhere — and because a negative that reproduces is worth
more than an untested idea. On a map with tighter coverage and no floor beneath
the track it may well work; here it does not, and I would not have known
without the control.

## 8. What else is in the frames — and the answer for past CP2

The corridor works to CP2 and is silent after it, which is most of the run. So:
what else does a frame contain that a candidate could be scored against?

The bottom-right HUD carries four readouts, not one. Speed is the one this arm
used. Beside it are a **gear** digit in a ring, an **RPM arc**, and — in a small
box — a **tyre-wetness percentage**, seen at 91 %, 43 % and a `Slip` warning at
three different points of the run.

**Wetness is the one that matters, and it is the answer to the CP2 problem.**

* **It is a positional integral.** Wetness is not a function of the car's state;
  it is a function of *where the car has been*. Two candidates that agree in
  speed at an instant but drove different routes to get there have different
  wetness, and it does not wash out — it persists for seconds and decays.
* **It is decisive on this map, measured.** Between two human replays of Cobalt
  Cove, wetness differs by more than 10 percentage points at **870 of 1780
  shared instants — 49 %**. Speed does not come close to separating them that
  often. It cycles 0 → 100 → 0 several times per run as the route crosses water.
* **The run's author says it is the point.** Wirtual's own commentary: *"going
  into the ending pipe, we do not have wet tyres… whereas if you do it the same
  way that the world record does, you will cross this water pool on your way to
  the pipe. So the last part is a wet pipe. That's why he cannot really do the
  skip that I did."* The reroute this reconstruction cannot yet reach is chosen
  *for its wetness*.
* **The simulator has it.** `wetness` is byte 101 of the 116-byte telemetry
  sample, already decoded (`tmtraj fields`), already exported.
* **And it is on screen**, so it can be read for the whole run the same way the
  speed was.

**One thing had to be built, and it is now built.** `fk trace`'s per-tick
readout gathered 44 bytes — clock, quaternion, position, velocity — and wetness
was outside it. That was a decision, not a limit: the engine computes it, since
the recording carries it.

`fk probe` (new) finds a named channel by asking the recording. Gather a wide
window around the located car, take the game's own series for the channel, and
report every offset whose bytes reproduce it — a search with a ground-truth
answer key, which cannot talk itself into a wrong answer the way a
self-consistency argument can.

**Result: `wetness` is an f32 at `car+180`.**

| | |
|---|---|
| exact on steady ticks | **95.63 %** and 95.95 % (two probe ticks, Sapi), **95.37 %** (Bren, a different ghost) |
| correlation | **0.9997** |
| next-best offset anywhere in 2 KB | 31–47 % |
| **negative control** | the same tape against **another run's** answer key: **44.10 %, NOT FOUND** — the probe declines |
| end to end | `fk trace` now emits a `wetness` column agreeing with the recordings to a mean \|diff\| of **0.00104** (Sapi) and 0.00079 (Bren) — inside the record's own u8 quantisation of 1/255 = 0.0039 |
| the widening disturbs nothing | the same tape traced before and after: 8060 shared instants, position max difference **0.000 m** |

Three things the probe had to be taught, each of which produced a wrong answer
first: a channel that barely varies matches everywhere (so it states the
reference's own variation and refuses to rank without enough of it);
mid-transition ticks are not comparable, because the record is on a 50 ms grid
and the fork reports every 10 ms; and **which rounding the record uses is not
something to assume** — round-to-nearest and truncation differ by 17 percentage
points here, so both are scored. An unscaled `u8` encoding is tried too, after
scoring a gear as a 0..1 quantity gave 0.00 % exact beside a 0.9953 correlation,
which is the shape of a right answer being told the wrong question.

**What remains** is to make it the objective: read the wetness percentage off
the video's HUD the way the speed was read, and score candidates on it. That is
the same work as §1 and §5, on a channel that constrains the route rather than
the speed.

## 9. The wetness readout on screen: located, then decoded

**SUPERSEDED in part.** §9.1 below finishes this reader and corrects one of its
findings: the largest of the three "digit count" modes is not a percentage at
all. Everything in §9 up to §9.1 is what was known before that; the numbers in
it are real measurements and the conclusion drawn from the largest one was
wrong. Read §9.1 for the state of the reader.

The simulator side is done (§8). The video side is where this arm stops, and
precisely where matters.

**Located.** The bottom-right status box draws a **droplet icon at x 2119–2130,
y 1230–1242** (2560×1440 master) followed by the percentage: digit cells about
9 px apart from x ≈ 2135, glyph box ≈ 10×15, then a `%`. Read by eye off ASCII
dumps: **43 %** at video 565, **20 %** at 567 and 569.

**A trap for whoever finishes it.** The same box draws a **`! Slip` line at the
same y**, so the line's content varies and a reader that assumes digits will
decode letters as numbers. `S`, `l`, `i`, `p` at 540 is what that looks like.

**Available on 62.2 % of the run.** Over all 4380 frames of the final run the
droplet is on screen on **2726** of them (`wetness/wetpresence.tsv` lists the
stretches). Detected by **contrast** — p95 − p05 of `min(r,g,b)` over the icon
rect ≥ 45 — not by an absolute level, because the box sits over everything from
a dark tunnel to a white wall. Controls: present at 540/565/567/569 where the
icon is visible in the dump; absent at 556 and 571 where it is not.

**Not done: decoding the digits.** They are ~9×13 px over wildly varying
backgrounds. `vidread wetread` is written — it anchors on the `%` glyph and
reads right-aligned digits leftwards from it, which is both what the variable
field needs and the guard against the `! Slip` trap — but its **templates are
not trained**, because with the labelling budget I had the honest options were
a reader I could not control or no reader. Four glyphs (0, 2, 3, 4) are legible
in the frames I read; the other six need eye readings I did not get to, and one
frame I tried to read produced a string no percentage can be, which is exactly
the failure mode a half-trained alphabet has.

### The acceptance test that reader will need, measured

Whoever finishes it does not have to trust it. Over three human replays (283
decreasing steps) the dry-out law is exact:

* **Every decrease is an integer number of 1/255 units — 0 of 283 are not.**
  The channel is a u8 and nothing between samples is interpolated.
* Decreases come in exactly **two kinds**: gradual dry-out at **1 or 2 units
  per 50 ms** (213 of 283), and an **instant reset to 0.000 in 100 ms** when
  the car leaves the water.
* Gradual stretches run at **0.098, 0.099 and 0.101 /s** on the three
  replays — 10 percentage points per second, so a soaked car dries in about
  ten seconds.

So a decoded series can be checked **without any ground truth for the run being
read**: every decrease is 1–2 units per 50 ms or a reset; gradual stretches run
at 0.10 /s; and it never rises except in water. A mis-decoded tens digit is a
10–30 point step with no reset, and a mis-decoded units digit breaks the
quantisation. Three independent checks, none of which needs the run simulated.

### The labelling attempt: the circle, and the geometric way out of it

Rather than read six more glyphs by eye, I clustered: extract every digit box
from every frame where the droplet is up, cluster the 10×15 bitmaps, and the
alphabet falls out with only the *names* unknown — which the dry-out law above
can then pin down, since a descending dry-out stretch spells its own labels.
`vidread wetcluster` does that.

**The first run returned 72 clusters, not 10**, and that named the blocker
exactly: **the digit cells cannot be cut until the `%` is found, and the `%`
cannot be found without a template.** The field is left-aligned after the icon,
so its cells move with the value; cutting at a fixed x — all an untrained
reader can do — samples a different part of the field on every frame, and what
clusters is the background.

**The way out is geometric, and it works.** The `%` is the rightmost ink in the
box, so a per-frame **ink profile** gives the field's right edge with no
template at all. `vidread wetedge` measures it: ink per column over the glyph
*rows only* (the `! Slip` line shares this y, and a full-height profile mixes
it in), thresholded against the band's own dark level rather than an absolute
one, anchored on the droplet so the whole measurement rides with the HUD.

Over the run's 2726 droplet frames it finds an edge on **2725**, and the
distribution is exactly what a right-aligned `%` after a 1-, 2- or 3-digit
number should give — **three sharp modes**:

| right edge | frames | |
|---|---|---|
| 2159 | **1334** | |
| 2165 | **480** | |
| 2174 | **169** | |
| 2193 | 402 | the band's own right end — a saturated background, i.e. the detector failing *loudly* |
| everything else | ~340 | scattered singletons |

So the edge is measurable on about 85 % of the frames the icon is up, and the
15 % where it is not announce themselves by pinning to the band edge.

**Re-clustering on those per-frame cuts took 72 → 45**, with membership roughly
tripled — a real cut finds three digits where a fixed cut found background.
Bucketing by edge (the sub-pixel phase test) took it further: **16, 13 and 11
clusters** in the three modes. Still not ten, and the reason turned out not to
be phase at all.

**Pooling the three cells is what was left.** A 1-digit value has one digit and
*two cells of background*; clustering all three together mixes glyphs with
scenery no matter how well the cells are cut. Clustering **one cell within one
edge bucket** — the only combination in which every sample is the same kind of
thing — gives:

| edge (frames) | clusters in the units cell |
|---|---|
| 2159 (1334) | **9** |
| 2165 (480) | **2** |
| 2174 (169) | **6** |

And at edge 2159 the units cell is dominated by **one shape with 889 of 1334
members**, whose averaged bitmap is plainly a `0`; the smaller clusters are the
same glyph over different backgrounds. That is a reader working, not a reader
failing — 1334 frames of `0%` is what a run that spends most of its length dry
should look like.

**What is left, precisely.** Merge the background variants (a
contrast-normalised template already ignores background, so this is a clustering
radius question, not a new idea), name the merged shapes with the dry-out law,
and run the law as the gate. The instrument is built and the cells are right;
what remains is bookkeeping I did not have the session for.

**Do not read 72, or 45, as noise.** A cluster count far from ten, on data where
ten is the answer, is the instrument reporting that its cells are wrong, which
is what it is for. **Nine would have been the dangerous outcome**: two glyphs
merged, unseparable afterwards, and a reader subtly wrong on one digit forever.
A result that fails visibly beats one that fails invisibly.

## 9.1 The reader, finished — and the mode that was not a number

The reader is done. It reads **574 frames** of the final run and the dry-out
law refuses **none of them**. Getting there meant correcting the previous
section's central inference, and the correction is the interesting part.

### The `! Slip` line has an icon too

§9 detects the readout by **contrast** over the icon rectangle — p95 − p05 of
`min(r,g,b)` — and that is the right way to find the box over backgrounds that
run from a dark tunnel to a white wall. But contrast says the slot is *drawn*.
It cannot say **what is in it**, and this box draws two things there: a droplet
when the line is a percentage, and the **`!` of `! Slip`** when it is not.

`vidread weticon` clusters the icon slot itself, and the two shapes fall out —
an exclamation mark (a bar, a gap, a dot) and a droplet (narrow at the top,
round at the bottom). Their cross-check is that the icon shape and the field's
right edge are **two independent measurements** — one a template correlation on
12×15 pixels, the other a template-free ink profile 20 px to the right — and
they agree:

| icon in the slot | right edge 2156 | 2159 | 2165 | 2174 | band end (failed) | other |
|---|---|---|---|---|---|---|
| `!` (1753 frames) | 0 | **1326** | 14 | 2 | 270 | 140 |
| droplet (973) | 55 | 8 | **466** | 167 | 132 | 145 |

So the 2159 mode — **1334 frames, the largest, the one §9 read as `0 %`** — is
`! Slip`. Its "right edge" is the right edge of the **`p`**, and the 889-member
cluster whose averaged bitmap looked "plainly a `0`" is that `p`'s bowl. The
sentence *"1334 frames of `0 %` is what a run that spends most of its length dry
looks like"* was a good story fitted to a real measurement of the wrong thing.

> **A guard you removed to break a circle is still a guard.** §9 dropped the
> `%` anchor — the stated defence against `! Slip` — because finding the `%`
> needed a template and the template needed cells. The geometric right edge
> broke that circle beautifully and *silently took the guard with it*. What
> replaced it had to be an independent reading of the same question, not an
> assumption that the question had gone away.

### The cell grid, measured

With the Slip frames out, `vidread wetgeom` takes the **median ink profile per
edge bucket** over every clean droplet frame — backgrounds differ frame to
frame and glyphs do not, so the median is the field's own geometry with the
scenery removed. It says:

* the field is **left-aligned at x 2136**, not right-aligned against the `%`;
* the digit pitch is **exactly 9 px** — an integer, so every cell shares one
  sub-pixel phase and the per-phase template banks the speed field needed are
  not needed here;
* the `%` is **11 px** wide and follows the last digit.

Hence `edge = 2136 + 9n + 11`: **2156, 2165, 2174 for one, two and three
digits.** The previous cells — pitch 9.6, cut leftwards from the edge — were
sampling across glyph boundaries even on the frames that were percentages.

### The alphabet, with no eye-labelling at all

Three digits can only ever be **100**. So the 151 three-digit frames hand over
a `1` (cell 0) and two `0`s (cells 1 and 2) for nothing, and the rest of the
alphabet follows from the dry-out law: a gradual dry-out steps the units digit
down by one about every six frames, so the **temporal succession** of the
units-cell clusters spells `1, 0, 9, 8, …`. `vidread wetalpha` does both.

* Seed: `0` from 302 samples (self-correlation median 0.967), `1` from 151
  (0.974), and the two templates correlate at **0.039** — they are not the same
  shape being split.
* Clustering all 1313 cells at radius 0.82 gives 14 clusters; **ten of them
  form a single 10-cycle** under "which cluster follows this one after a dwell
  of at least four frames": `1 → 2 → 3 → 7 → 5 → 4 → 11 → 10 → 6 → 9 → 1`.
* **The check.** The chain is anchored only on the seeded `0`. Walking it names
  cluster 1 as `1` — and the seed, which never saw the chain, calls cluster 1 a
  `1` at correlation 0.999. Two derivations, one from the value `100` and one
  from the passage of time, agree.
* The four leftover clusters (8, 7, 7 and 13 members) are background variants:
  each joins the named shape it correlates best with, at 0.786 – 0.925. That is
  the whole of "merge the background variants" — a radius question, as §9 said.

The ten averaged bitmaps are in `wetness/alpha.txt` and read as the digits they
are named.

### The acceptance gate, and what it decided

`vidread wetlaw` is §9's dry-out law as an instrument. Two things had to be
right before it could be believed, and both were wrong first:

* **The sample period, not the frame period.** The engine recomputes wetness
  every 50 ms and the HUD holds the last value, so at 60 Hz a whole sample's
  change lands on one frame boundary. Dividing by the frame period turns the
  largest real rise in the human data — 4 points — into a nonsensical 240 pt/s
  and rejects it. The bound is per **sample**.
* **A reset is two steps, and the second one is where the zero is.** Asking
  whether *this* step landed on nil misses the first half of a `202 + 53` fall.
  The test looks ahead — and looks for the fall **completing**, not for the
  zero itself, because the HUD box empties as the readout does and the last
  frame of a real reset in this video reads 8. What separates a reset from a
  mis-decoded tens digit is not the destination but the shape: a reset keeps
  falling, a bad digit comes straight back up, and the return trip is caught as
  a rise by the same gate.

| series | pairs | violations |
|---|---|---|
| **positive control** — human replay 218053, from its own telemetry | 5337 | **0** |
| 222683 | 6333 | **0** |
| 235245 | 6321 | **0** |
| **negative control** — the same three with 2 % of readings digit-flipped | — | **1.18 – 1.27 %** |
| **the decoded video** | 551 | **0** |

The negative control is the one that makes the row above it mean something: the
gate is not vacuous, and injecting exactly the defect it claims to catch turns
it on.

**And the gate decided a parameter rather than being tuned to pass.** The right
edge lands within a pixel or two of a mode on frames the strict test refuses;
letting it in is free in principle, because the cells are anchored at x 2136
and not at the edge, so a wrong edge moves no cell. The law says how free:

| edge tolerance | readings | violations |
|---|---|---|
| **0 px** | **574** | **0** |
| 1 px | 591 | 2 (0.35 %) |
| 2 px | 600 | 3 (0.52 %) |
| 3 px | 609 | 5 (0.85 %) |

Seventeen more readings cost two violations, so the reader ships at zero. The
same test settled the digit **margin**: a cell that correlates 0.95 with `9` and
0.92 with `0` is not a reading of `9`, and every wrong digit the law caught sat
under a margin of 0.05 while the bulk of the readings sit at 0.18. Refusing
below 0.06 drops 7 readings and takes the violations from 5 to 1.

### What it reads

574 readings, race 3.605 – 55.571 s, in the four stretches the box is legible:

| race | what the readout does |
|---|---|
| 3.605 – 9.921 | **100 %** throughout — the car is in water for the whole launch |
| 9.921 – 10.038 | 96 → 73 → 8: **a reset.** The car leaves the water at ≈ 9.95 |
| 22.355 – 27 | back to 100 %, then a gradual dry-out to 90 % over 23.4 – 24.3 |
| 39.355 – 46.7 | 0 → 5 (entering water), 44 at 41.7, drying 43 → 20 by 45.9 |
| 50.021 – 55.571 | **100 %** again |

Coverage is 574 of 4380 frames — 13.1 %. That is the honest figure and it is
much smaller than §9's 62.2 %, because 62.2 % was the frames on which the
*slot* was drawn and most of those are the Slip line.

## 9.2 What the wetness objective bought

`recon --wet` scores a candidate on the decoded series exactly as `--corridor`
scores it on the human lines: the run's score is the **earliest** of "stopped
tracking the speed", "left the corridor" and "wrong wetness", because a
candidate is right until it is wrong and any one of them can be what is wrong
first.

### It dates the reroute, and it corrects `--corridor-to`

Two controls first, because a gate that cannot tell two runs apart is not a
gate and one that disagrees with the recording is not reading the channel:

* **It discriminates.** Human 218053 against human 222683, over the 1780
  instants they share: within 10 points on **51.9 %** of them, mean |diff|
  **34.95** points; against 235245, 65.8 % and 19.64. So two real runs of this
  map differ by more than 10 points at about half of their shared instants —
  an independent reimplementation reproducing §8's 870-of-1780.
* **Engine and recording are the same channel.** 218053's recorded telemetry
  against a full-run `fk trace` of 218053's own tape: 1777 shared instants,
  **100.0 % within 10 points, mean |diff| 0.08 points.** Candidates only ever
  produce the engine side, so this is the one that licenses the gate.

Held against the human replays (`recon wetcmp`), the video's wetness:

* is **indistinguishable through the launch** — at a 10-point tolerance the
  last agreed instant with 218053 is race **10.038**, and the reset instant
  matches 218053's to within 40 ms and 222683's to within 130 ms;
* **first differs measurably at race 22.355**, where the video reads 100 % and
  Sapi is at 55.7 % and still rising — this run was back in the water about
  0.45 s before the human route puts a car there;
* and **diverges decisively from race 41.8 s** — 44 % and 37 % where Sapi reads
  0 — with 100 % against Sapi's 25 % at 50.2 s.

The video has no readable frame between 10.038 and 22.355, so **the divergence
began somewhere inside that window and this instrument cannot say where.** What
it does say is that the shared-route assumption is *supported* to race 10.038
and *refuted* by 22.355. `--corridor-to 30000` was set at "CP2" by assumption;
nothing here supports a value past **22355**, and the honest reading is that
past race 10 s the corridor is asserting something the wetness does not back.
It changes no number in this arm — no candidate reaches race 11 — but it is the
first evidence anyone has had about where that parameter belongs.

### On the incumbent, it confirms rather than lowers

| tape | speed + corridor (3 m) | + wetness |
|---|---|---|
| `best_corridor.events` (the arm's best) | **12.330 s** | **12.330 s** |
| `best_corr.events` | 10.621 s | 10.621 s |
| `seed_218053.events` | 11.170 s | 11.170 s |

Unchanged — and unlike the corridor, which took 12.938 down to 12.380 by
catching a car that had left the track, **the wetness gate has nothing to
object to.** The incumbent is 100 % wet from 3.6 to 9.9 and resets at 10.0,
exactly as the video does; its first wetness disagreement is at race **24.005**,
twelve seconds past where its speed already failed.

**The gate can fire.** Shifting the video's series in race time — the same
tape, the same code, a series it cannot satisfy — takes the same 12.330 to
**8.405** at +2 s. (At −2 s it does not fire: that shift lands the reset in a
stretch with fewer than the six consecutive readings the gate requires before
it will call a divergence. The asymmetry is the gate's `run` parameter, not an
inconsistency.)

### Why it cannot extend the reconstruction yet, precisely

The reconstruction stops at race 12.380. The video's wetness readings stop at
10.038 and do not resume until **22.355**. *There is no wetness reading in the
window where the search is working.* This is not a property of the objective —
it is that the HUD box in that window is showing the other line.

So the honest statement of what wetness bought is three things, and none of
them is a larger number:

1. it **confirmed** that the 12.330 was not bought by a wrong water history,
   which is the failure mode the corridor caught for position;
2. it **dated the reroute** — indistinguishable from the human route to race
   10.038, measurably different by 22.355, decisively so by 41.8 — which is the
   first evidence about where `--corridor-to` belongs;
3. it is **ready** for the region past CP2, where nothing else this arm has
   built makes any claim, and where 543 of its 574 readings live.

Seeding a search at race 20 s to reach that region does **not** work, and the
reason is worth writing down: the humans are 16 seconds slower over the run, so
at race 20 s a human tape and this run are at completely different points of
the track. A seed has to be matched by **position**, not by clock, and no tape
we have reaches race 20 s of *this* run.

## 9.3 What looking at the frames gave that measuring them did not

Everything above was built by instruments: ink profiles, template correlation,
clustering. That is how you get a *number* out of a frame. It is not how you
answer "what is happening", and four things fell out in twenty minutes of
actually looking at the pictures — labelled here as **INFERRED-from-looking**,
because an eye on a frame is evidence about the world and is not a measurement.

**The decoder, confirmed from outside itself.** Six frames the reader called
`100`, `98`, `90`, `5`, `44` and `21`, cropped at full resolution and enlarged,
read exactly that on screen — droplet, digits and `%`. Every control in §9.1 is
internal to the reader; this one is not, and it took one command.

**The 10 – 22 s hole is not an absence of information.** One frame per second
across it shows the car on dry blue surfaces the whole way: the pipe at 11 – 13,
airborne off it at 15, a yellow-striped block at 17, a fast descent at 18, a
wall-ride at 19 – 20. Frames every 0.3 s put the **water entry between race 20.6
and 20.9** — the car is on the blue wall above the pool at 20.6 and in the pool,
throwing a wake, at 20.9.

**The run is narrated, and nobody in this project had read it.** The video's own
captions place the route against the clock (video time minus 522.662):

| race | what he says |
|---|---|
| 1.9 | "we start out in the water pool inconspicuously, trying to keep a lot of speed" |
| 6.8 | "and then we head to checkpoint one" |
| 9.0 | "this pipe at real speed just looks ridiculous the way it's driven" |
| 13.7 | "we jump" |
| 14.7 | "get for the most part a clean wall hug" |
| 17.8 | "do the wall touch into flat lining into" |
| **20.3** | **"the water pool close to the corner"** |
| 22.7 | "close to the jump here, carry a lot of speed" |
| 25.9 | "and then towards the moving blocks" |
| 33.7 | "the shortcut across two keeps decent speed" |
| 36.6 | "jump through the checkpoint" |
| 39.3 | "Shaft's kiss as we crash into the plastic double Beyblade" |
| **48.8** | **"carry all that speed into the water"** |
| 50.1 | "go past the green arrow" |
| 54.4 | "plastic bounce down to the last checkpoint" |
| 59.6 | "into nose bug through the second last checkpoint" |
| 62.6 | "no slide up the hill" |
| 64.3 | "aim for a pipe blindly that you can't really see" |
| 70.9 | "touch the pipe into smooth landing for a final trick" |

**Three of those water entries are in the decoded series, at the soak rate the
law measures.** "Into the water pool" at 20.3 → the reader's 100 % at 22.355.
"Carry all that speed into the water" at 48.8 → the reader's 100 % at 50.021,
**1.2 s later, which is exactly 0 → 100 at the 80 pt/s wetting rate §9.1
measured from the human replays.** The author's own commentary is an external
control on the reader, and it agrees three times.

He also states the mechanism the whole reroute is built on, in a different part
of the video: *"with wet tyres, you have no air control, and so you are
effectively spinning."* That is why the dry approach to the final pipe is worth
the detour — not grip, **air control**.

### The dry window, and the constraint it buys

So the hole in the readings is a hole in the *display*, not in the run: between
the reset and the pool the car is dry, and the HUD draws nothing because there
is nothing to draw. `recon --wet-zero 10100:20600` asserts that, and the
assertion has **four independent supports**:

* the reset **measured** at race 10.038;
* the frames, which show a dry car for the whole window;
* the run's author, placing the entry at 20.3;
* the **soak arithmetic** — 100 % at 22.355 at 80 pt/s puts the entry no
  earlier than race 21.1, which brackets the eye's 20.6 – 20.9 from the other
  side.

It is still an assertion. `recon` prints `ASSERTING 0 % … -- not read, inferred`
on every run that uses it, and the banked artefact stays the 574 measured
readings.

**And it bites where nothing else does.** The incumbent's own trace leaves the
asserted band at race **13.313**: it is in the water from 13.3 and soaked to
100 % by 15.2, while the video's car is dry until 20.6. So the wetness channel
now catches this candidate's off-track excursion **independently of the
corridor and of the map geometry**, a full second past the point where the
speed objective gives up — and it is the only one of the three detectors that
still works past CP2.

It does not move the 12.330, because the speed objective already fails there.
What it does is close the escape route: a candidate that buys tracking past
12.330 by falling into the water, which is precisely what this one does, is now
refused on a third and independent ground.

## 9.4 The number, with the wetness objective on

The joint objective does move the reconstruction, and it moves it while
satisfying more constraints than the old number did. Three arms, one seed
(11111), one budget (48 candidates a round), one base tape (Sapi's inputs to
race 8.0 s):

| arm | objective | rounds | best, re-measured on a quiet box |
|---|---|---|---|
| A | speed + corridor 3 m | 60 | **12.030 s** (plateaued at round 24) |
| B | + wetness as read | 24 | 12.380 s (stopped early to free the box) |
| E | + wetness + the asserted dry window | 70 | **12.480 s** (plateaued at round 30) |

And the two tapes held against each other under identical gates:

| tape | speed only | + corridor 3 m | + wetness | + dry window |
|---|---|---|---|---|
| the previous arm's `best_corridor.events` | 12.388 | 12.330 | 12.330 | 12.330 |
| **this arm's `best_joint_dry.events`** | 12.588 | **12.480** | **12.480** | **12.480** |

**Race 12.480 of 72.589** is the honest figure, and it is a stronger claim than
the 12.330 it replaces because it is measured under three observables instead
of two. The new tape is also better on every observable separately, not only on
the score: against Sapi's line it is 4.13 m out at race 12.5 where the old one
is 6.02, and 13.31 m at 13.0 where the old one is 25.02; its wetness leaves the
asserted dry band at 13.636 rather than 13.313. It still leaves the track — this
is not Wirtual's line past about 12.5 s — but it leaves it later and less.

### The number the search reported was not the number

**Arm E's log says 12.580. The tape re-measures at 12.480, five times out of
five on a quiet box.** The difference is exactly the corridor: 12.588 is that
tape's *speed-only* score, so in the round that scored it, the positional gate
did not run.

The cause is load. Three searches were running at once and the box sat at a
load average of 460; a trace written under that can come back short, and
`evaluate` was reading the corridor and wetness gates through `if let
Some(...)` — so a gate whose data could not be loaded was silently skipped and
the candidate kept its speed-only score. Worse, `score` itself skips video
instants with no engine sample nearby, which means **a trace that runs out
looks exactly like a trace that tracks perfectly.**

Both are fixed: a configured gate that cannot be evaluated now REFUSES the
candidate, and the score is capped at the last instant the engine actually
reported. And the general rule this is another instance of —

> **A search that keeps the best score keeps the luckiest measurement.** If the
> evaluator is not reproducible, the search is a maximiser of its noise. Every
> number in the table above is a re-measurement of a banked event list on an
> idle box, not a line from a search log.



Classifying the icon turned the Slip line from a trap into a census. It is on
screen for **1753 of 4380 frames — 40.0 %** — and unlike wetness it appears in
every five-second bin of the run, including race 10–20 s, which is exactly
where wetness is silent and where the reconstruction's frontier sits.

| race (5 s bins) | 0 | 5 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 | 60 | 65 | 70 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `! Slip` frames | 141 | 130 | 51 | 271 | 68 | 168 | 49 | 44 | 148 | 30 | 238 | 204 | 124 | 55 | 32 |
| droplet frames | 118 | 108 | 8 | 4 | 138 | 112 | 5 | 10 | 149 | 136 | 53 | 88 | 18 | 10 | 16 |

**UNKNOWN: which engine quantity it is.** `fk probe` cannot find it the way it
found wetness, because that method needs the channel in the game's own
recording as an answer key and no recorded ghost carries a slip flag. What
would settle it: **render a human replay through this project's own clip
pipeline and read its HUD with this same reader**, which gives a slip series
for a run that can be simulated — the answer key `fk probe` needs. The render
box is not reachable from this node, so it is a task and not a result.

Two smaller notes for whoever picks it up. The contrast presence test flickers
frame to frame over hard backgrounds (0/1/0/1 at 60 Hz around race 22), so
"the box was not drawn" is a **refusal**, not an observation — do not read an
absent box as a wetness of zero. And the presence test is not what limits the
wetness reader's coverage: dropping `--span-min` from 45 to 0 changes the
readings not at all, from 574 to 574. What limits coverage is the edge landing
in a mode and the digits being legible.


## 9.5 The third readout, located and not calibrated: `! Slip`

Classifying the icon turned the Slip line from a trap into a census. It is on
screen for **1753 of 4380 frames — 40.0 %** — and unlike wetness it appears in
every five-second bin of the run, including race 10–20 s, which is exactly
where wetness is silent and where the reconstruction's frontier sits.

| race (5 s bins) | 0 | 5 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 | 60 | 65 | 70 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `! Slip` frames | 141 | 130 | 51 | 271 | 68 | 168 | 49 | 44 | 148 | 30 | 238 | 204 | 124 | 55 | 32 |
| droplet frames | 118 | 108 | 8 | 4 | 138 | 112 | 5 | 10 | 149 | 136 | 53 | 88 | 18 | 10 | 16 |

**UNKNOWN: which engine quantity it is.** `fk probe` cannot find it the way it
found wetness, because that method needs the channel in the game's own
recording as an answer key and no recorded ghost carries a slip flag. What
would settle it: **render a human replay through this project's own clip
pipeline and read its HUD with this same reader**, which gives a slip series
for a run that can be simulated — the answer key `fk probe` needs. The render
box is not reachable from this node, so it is a task and not a result.

Two smaller notes for whoever picks it up. The contrast presence test flickers
frame to frame over hard backgrounds (0/1/0/1 at 60 Hz around race 22), so
"the box was not drawn" is a **refusal**, not an observation — do not read an
absent box as a wetness of zero. And the presence test is not what limits the
wetness reader's coverage: dropping `--span-min` from 45 to 0 changes the
readings not at all, from 574 to 574. What limits coverage is the edge landing
in a mode and the digits being legible.


## 10. What is banked

`~/persistent/private-30d/tm-wirtual-perfect/`

| | |
|---|---|
| `video/wirtual_speed_race.tsv` | the run's speed at 60 Hz, indexed by race time — the headline artefact |
| `video/kt_493.tsv`, `kt_498.tsv`, `ev_*.txt` | the recovered key states, per tick and as events |
| `video/tmpl_eye.txt`, `tmpl_timer.txt` | the digit templates, and `lampsurvey2.tsv`, the overlay census |
| `map/` | the map, all 12 TMX replays, and Sapi's decoded telemetry |
| `engine/` | full-run engine traces of Sapi's run, forked at race 0.25 and 9.25 |
| `recon/` | the baseline tape, its trace, and its comparison against the video |
| `recon2/` | this arm's tape (`best_joint_dry.events`, its gtape and replay), both traces, and the five search logs |
| `wetness/` | the reader: `icon.tmpl`, `digits.tmpl`, `wet_video.tsv` (the 574 readings), `alpha.txt`, `wetedge2.tsv`, `wetgeom.tsv`, `READER.md`, the run's captions, and `frames/` — the three contact sheets §9.3 is written from |

Tools added for this, all in `tools/`: **`vidread`** (read a run off a screen
recording), **`recon`** (grow a tape against a speed trace), `ghost tape
script` (an event list to a tape) and `tmtraj csvdiff` (two trajectories on the
instants they share).

## 11. Attribution

The run is **Wirtual's**, on **Nadeo's** map, made with **Acepter's**
Trackmania Input Control Kit. Video:
<https://www.youtube.com/watch?v=F8tbqE2wV08>. No file produced here carries
his login, account id or nickname, and none of it has been or will be uploaded
to a leaderboard.
