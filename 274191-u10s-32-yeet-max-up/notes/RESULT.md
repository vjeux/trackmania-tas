# 274191 — `U10S_32 By Everios96 [Yeet] MAX-UP`

## The author time falls by 0.241 s, and the four-and-a-half-second reactor flight turns out to be four inputs

**For a driver, before any of the detail:**

> This map looks like it is about the reactor flight. It is not. **The flight is
> the easy part** — the simplest tape that beats the author time flies the whole
> four-and-a-half-second climb on **four steering inputs and two values**, then
> holds the wheel centred for the last 2.3 seconds. All the precision lives in
> the last quarter-second of road before the drop, and what you have to do there
> is *take an input away*: **stop slamming the wheel into the edge — ease out of
> the lock over the last two tenths.** That sets the spin you fall with, and the
> spin is everything, because **the wheel does nothing at all for the 1.2 seconds
> between leaving the road and half a second after the launcher.** Pin the lock
> to the lip like all three humans do and you meet the launcher rotated 58°
> nose-up and keep 148 km/h. Ease out and you meet it at 35° and keep 197.

| | time | vs AT | what it is |
|---|---|---|---|
| human #3 `Doslydoo` | 8.597 s | +0.893 | a different, slower route |
| human #2 `Flopper_TM` | 7.933 s | +0.229 | same route, mirrored roll |
| **human online WR** `Whatever8319` | **7.893 s** | **+0.189** | best of a field of three |
| **author time (AT)** | **7.704 s** | — | `TwinsNaA`'s driven validation lap |
| our best unconstrained TAS | **7.463 s** | **−0.241** | §2 |
| **keyboard climb, 24 presses** | **7.474 s** | **−0.230** | §6 |
| **keyboard climb, 13 presses, 30 ms hold floor** | **7.474 s** | **−0.230** | §6 |
| **the simplest tape that beats it: 33 live inputs, FOUR of them in the flight** | **7.514 s** | **−0.190** | **the deliverable** |

uid `ieMJgDjCjWkZ45apoiK2DPmQeQg` · Nadeo mapId `dddea8ed-682a-4e42-9566-3035683ddba5`
· TMX 274191 · map md5 `20a56be3b87b3dfa3ff9d35e523ec25c` (Nadeo's own CDN,
anonymous). **Nothing here has been or will be submitted to a Nadeo
leaderboard.**

---

## 1. Correctness first, difficulty second

Every number came out of the plain oracle — `TrackmaniaServer /nodaemon
/validatepath=` re-simulating the input bitstream — and every headline tape was
re-validated **in a second process with a different binary**, carrying the human
world record in the same batch as a known-answer control returning 7.893 s.

* **Identity control: 3/3.** All three ghosts re-simulate to their exact
  leaderboard millisecond (7.893, 7.933, 8.597).
* **ACQUISITION.md §8 whole-field check: 3/3 = 100 %.** The leaderboard has
  exactly three records; all three were downloaded and all three reproduce. This
  is not a sample — it is the *entire recorded population* of the map. Worth
  saying because 203072 failed this check at 29 % and the suspected cause there
  was a Reactor item changing behaviour between game builds. This map is
  Reactor-tagged and passes outright.
* **Codec round-trip: 3/3.**
* **Search guard on throughout** (the hardened build's default: every banked
  improvement re-validated through the plain oracle before acceptance), plus an
  independent ratchet process re-validating every new champion with the *other*
  binary before banking it, with the human WR as a control in every batch.
  **Zero phantoms in the entire session**; nothing was written to
  `tm-loop/phantoms/`.
* **Raw server verdict on the fast tape:** `"NbRespawns": 0`,
  `"NbCheckpoints": 1`, `"MapUid": "ieMJgDjCjWkZ45apoiK2DPmQeQg"`. No respawn, no
  skipped geometry, right map.

**Is it the same route a human drives?** Distance to the *nearest point* of the
human world record's own path, over the whole run:

| tape | mean | max |
|---|---|---|
| **our TAS** | **8.4 m** | **22.0 m** |
| human #2 (`Flopper_TM`, 7.933) | 19.4 m | 45.3 m |
| human #3 (`Doslydoo`, 8.597) | 49.1 m | 113.4 m |

**Our line is twice as close to the world record's line as the second-place
human's is**, and the first 2.24 s of our tape are the world record's own
inputs, essentially unmodified.

---

## 2. What the map is, and the physics everything rests on

482 blocks, 1141 items, **no intermediate checkpoints**: one
`PlatformPlasticStart`, one `PlatformPlasticFinish`. Start (464, 42, 496); the
gate is crossed at about **(991, 244, 876)** — 527 m across, 380 m along, **202 m
straight up**. "MAX-UP" is literal. Medals: author **7.704**, gold 9.000, silver
10.000, bronze 12.000. The medals below are round to the second and the author
time is not: the signature of a **driven validation lap**. A person drove 7.704.

**63 % of the lap is airborne.** The world record's last ground contact is at
**2.85 s** of a 7.89 s lap, after which the car accelerates from 250 km/h to
772 km/h in free air.

### 2a. The thruster is bolted to the car's own body axis

`an axes` rebuilds the body frame from each telemetry sample's quaternion,
differentiates the recorded velocity, subtracts gravity and projects the
remainder onto the body axes. (Convention checked against the standing start: at
t = 0 body-forward is world +X, which is where the car drives off the line, and
body-up is world +Y.)

On the ground the residual sits on **+forward** — engine and boosters. In the
air, on all three human tapes independently:

```
  t_ms   |a-g|  onFwd   onUp  onRgt     roll
  4300    64.9   40.3  -50.8    1.9    -2.32
  5000    46.3   18.5  -42.3    2.7    -2.99
  5900    43.7   12.9  -41.7    1.2    +2.37
  6500    45.2   11.3  -43.6    3.3    +2.28
  7100    44.7   -1.4  -44.6    3.3    +2.28
```

**`onUp` is pinned at −44 m/s² for four and a half seconds.** Over the whole
airborne phase, 87 % of the non-gravity acceleration lies on the body-up axis,
mean cosine −0.86, on every tape.

> **The reactor applies a constant ~44 m/s² (4.5 g) along the car's −up axis —
> out through the floor — and nothing else. Where you go is decided entirely by
> where the car is pointing.**

Which is why every run flies **nearly inverted** (roll ±2.2 to ±3.1 rad): that is
what it takes to aim "out through the floor" at the sky. The world record rolls
one way, rank 2 rolls the other; both work.

### 2b. THE DEAD ZONE — 1.2 seconds in which the wheel does nothing

This is the finding that reorganised the investigation, and I nearly published
the opposite of it.

**Replace the steering over any window inside race 2.890 → 4.100 s with any
constant whatsoever and the run returns its base time, exactly.** Measured at
**20 ms resolution**: 61 two-tick windows × 3 constants (−127 / 0 / +127) =
183 substitutions **on each of two independent lineages** (the human world
record, and our own 7.490 tape). **Every one returns the base millisecond**:
7.893 and 7.490 respectively. Replacing the *entire* 1.21 s window with any of
15 constants: same, both lineages. The first tick at which steering has any
effect at all is tick 561 = **race 4.110 s**, where a 20 ms block of full lock
already changes the time or DNFs.

**But the dead zone is STEERING-ONLY, and this is where it becomes
lineage-dependent** — a correction I owe to the 203330 agent, who found two of
their own "dead" inputs were really 70 ms tolerances read at 100 ms resolution:

* The **human world record holds gas and brake constant** through the whole
  window, so for its lineage the zone is inert on all three axes: shifting its
  (non-existent) pedal transitions by ±100 ms changes nothing.
* **Our tape is not inert there.** Our search discovered brake transitions
  inside the dead zone — at race 3.53 s and 3.83 s, bracketing the launcher
  strike at 3.60 s — and they are **10 ms-critical**: forcing the pedals to any
  constant DNFs, and shifting them by a single tick either way DNFs.

So the honest statement is: **on this map the wheel is dead from 2.89 s to
4.10 s for every lineage tested, and the pedals are not. Our lineage put
load-bearing brake taps into a window where the humans put nothing.**

### 2c. The map, in four parts

| race | what you control |
|---|---|
| 0 → 2.890 s | **on the ground. Everything that matters is decided here.** |
| 2.890 → 4.100 s | **the wheel does nothing.** Pedals still live. |
| 4.100 → 7.400 s | the powered climb: steer, gas and brake all steer the attitude |
| 7.400 s → finish | **nothing.** Freeze every input from 7.400 s and the time is unchanged. |

### 2d. Three records, three routes

| | route |
|---|---|
| r1 7.893 | accelerate east, booster at 2.05 s, **leave the road at 2.85 s**, fall 24 m to y ≈ 9, strike the launcher at 3.60 s, fly |
| r2 7.933 | same route, mirrored roll, and it **touches down again at 7.85 s** just short of the gate |
| r3 8.597 | **a different map**: lifts off at 2.0 s, brakes to 27 km/h, reverses, stands the car on its nose against a wall at (511, 40, 518) at 3.5 s and launches **vertically from y = 42**, never using the drop. Reaches **942 km/h**, the fastest of the three, and still loses 0.7 s because it spent 1.5 s stopped. |

r3's launch point is 636 m from the gate against r1's 553 m, so the drop route
is better and we kept it. (An arm seeded from r3 ran 25 minutes, reached 8.502,
and was retired.)

---

## 3. Where the time is

### 3a. Against the human world record, station by station

Planes normal to the world record's own velocity, so this is a progress measure
and not an axis artefact. `dt` = ours − theirs.

| ref | dt | WR km/h | TAS km/h | WR thrust elev | TAS thrust elev |
|---|---|---|---|---|---|
| 2.08 – 3.48 s | **0 … +5 ms (we are BEHIND)** | | | | |
| 3.48 s | −6 ms | 233 | 245 | +58° | +35° |
| 3.68 s | **−38 ms** | 169 | 179 | +36° | +62° |
| 4.08 s | **−46 ms** | 254 | 246 | +23° | +11° |
| 4.48 s | −23 ms | 309 | 324 | +47° | +31° |
| 4.88 s | −45 ms | 375 | 398 | **+53°** | **+24°** |
| 5.28 s | −72 ms | 432 | 469 | +51° | **+17°** |
| 5.88 s | −128 ms | 514 | 568 | +45° | +19° |
| 6.48 s | −186 ms | 601 | 661 | +41° | +24° |
| 7.08 s | −247 ms | 681 | 741 | +40° | +30° |
| 7.68 s | −289 ms | 759 | 820 | +48° | +36° |
| finish | **−349 ms** | | | | |

Read with §2b in hand, that decomposes exactly:

* **−46 ms is banked by race 4.08 s — before a single steering input in the air
  can have done anything.** Those 46 ms are bought *on the ground* and delivered
  through 1.2 seconds of a dead wheel.
* **The remaining −303 ms is bought in the steered climb**, and the mechanism is
  the last two columns: the human lets the reactor swing up to **+53° above
  horizontal** where we hold it at **+17 to +25°**.

*(This table is against our 7.544 tape, the one whose trajectory was captured for
the A/B; the final 7.463 extends the same curve.)*

### 3b. The factorial ablation: 98 % is committed by 5.5 s

`tmsearch --ablate` grafts our inputs into the human world record over subsets of
six time ranges and measures all 64 combinations against the oracle.

| grafted | result | vs WR |
|---|---|---|
| nothing (base) | 7.893 | — |
| race 0 → **5.500 s** | **7.550** | **−343 ms (98 % of the gain)** |
| + 5.500 → 6.700 | 7.547 | −346 ms |
| + 6.700 → finish | 7.544 | −349 ms |
| race 5.500 → 6.700 only | 7.890 | −3 ms |
| race 6.700 → finish only | 7.891 | −2 ms |
| **every other one of the 64 subsets** | **DNF** | — |

1. **98 % of the gain is committed by the 5.5-second mark**, 1.4 s into the
   steered climb. Hand the world record our first 5.5 seconds and *its own
   hands* fly the rest to within 6 ms.
2. **Every non-prefix subset DNFs.** The lap is one chain: attitude is the
   integral of every input before it, so you cannot import a middle section.
   That is why single-operator search stalled and multi-operator search did not
   (§10).

### 3c. Speed and altitude at the moments that matter

| race | | human WR | our TAS |
|---|---|---|---|
| 2.890 s | leaves the road | 257 km/h, y = 33 | 253 km/h, y = 33 |
| 3.400 s | mid-fall | 241 km/h | 249 km/h |
| **3.600 s** | **strikes the launcher** | **148 km/h** | **197 km/h** |
| 4.000 s | climb begins | 238 km/h, y = 13 | 247 km/h, y = 13 |
| 5.000 s | | 391 km/h, y = 47 | **431 km/h, y = 33** |
| 6.000 s | | 530 km/h, y = 100 | **612 km/h, y = 105** |
| 7.000 s | | 668 km/h, y = 171 | **767 km/h, y = 195** |
| 7.500 s | | 733 km/h, y = 212 | **837 km/h, y = 241** |

The launcher costs the human **109 km/h** and costs us **56**. At 5 s we are
*lower* than the human (33 m against 47) and 40 km/h faster: **the climb is
bought with speed, not with altitude, and the reactor buys the height back.** By
6 s we are level in height and 82 km/h up.

---

## 4. The technique — what nobody was doing

### 4a. The launch spin, and why it is decided on the ground

Angular velocity from consecutive quaternions (`an spin`), rad/s, world frame:

```
              t_ms   |w|      wx     wy     wz    thrust elev
human WR      2790   0.77    0.42   0.57   0.29        -65
              2890   3.80    3.54   0.57   1.25        -64   <-- wheels leave
              3090   3.80    3.55   0.57   1.25        -22
              3290   3.80    3.55   0.57   1.25        +20
              3490   3.80    3.55   0.57   1.25        +58   <-- hits launcher
ours          2780   0.85    0.37   0.71   0.28        -66
              2880   3.58    3.04   1.15   1.49        -69   <-- wheels leave
              3080   3.58    3.04   1.15   1.49        -32
              3280   3.58    3.04   1.15   1.49         +4
              3480   3.58    3.04   1.15   1.49        +35   <-- hits launcher
```

The spin is **constant to two decimal places for 600 ms** in both cases — direct
confirmation of the dead zone. The attitude at which you meet the launcher is
fixed entirely by **the rotation you carry off the lip**.

The world record leaves spinning at **3.80 rad/s** and arrives nose-up **58°**.
We leave at **3.58 rad/s** about a different axis (less wx, more wy and wz) and
arrive at **35°**. The launcher pays accordingly: 148 km/h against 197.

And this is exactly what separates our own tapes:

| tape | finish | \|ω\| at the lip | (wx, wy, wz) |
|---|---|---|---|
| human WR | 7.893 | 3.80 | (3.54, 0.57, 1.25) |
| our 7.832 | 7.832 | **3.80** | **(3.54, 0.57, 1.25)** — the human's exact spin |
| our 7.690 | 7.690 | 3.62 | (3.06, 1.20, 1.50) |
| our 7.558 | 7.558 | 3.58 | (3.04, 1.15, 1.49) |
| our 7.544 | 7.544 | 3.58 | (3.04, 1.15, 1.49) |

**Flying the human's own launch spin better is worth 61 ms. Changing the launch
spin is worth another 288.** The launch is not merely worth its own 46 ms — it
is the enabler for everything after it.

### 4b. The input that sets it

Our 7.832 tape (which carries the human's launch spin, and whose ground phase
*is* the human's) against our 7.544 tape, over the last 440 ms of road. Steering
is the raw 8-bit tape value; −127 is full left.

```
 race     7.832   7.544     what is happening
  2.50 s   -110    -119     both turn in
  2.62 s   -112    -126     7.544 carries ~12 more lock
  2.70 s      0     -17     7.832 fully releases; 7.544 only eases
  2.77 s   -105    -126     both re-apply
  2.79 s   -127    -124  <-- 7.832 SLAMS to full lock and pins it. Brake ON.
  2.81 s   -127    -122     7.544 has already started UNWINDING.  Brake ON.
  2.85 s   -127    -115  <-- last tick on the ground
  2.88 s   -127    -109
  2.89 s   -127     -74  <-- wheels leave. The wheel is dead for 1.2 s.
```

**Where the field slams to full lock and pins it into the edge, our tape peaks
just short of full lock 20 ms earlier and unwinds steadily out of it — about
15/127 of lock over the last 100 ms — and brakes 20 ms later.** That unwinding is
the whole difference in launch spin. Everything before race 2.24 s is the world
record's own tape.

### 4c. Arresting the swing once control returns

From 4.10 s the job is to stop the rotation you launched with and park the
thruster flat. We hold **+17 to +25° above horizontal** through the middle of the
climb; the human lets the rotation run and the thruster swings to **+53°**, then
drifts back to +39°. Cosine between the thrust vector and the straight line to
the gate, averaged 3.8 – 7.0 s:

| tape | finish | alignment | thrust wasted |
|---|---|---|---|
| human WR | 7.893 | 0.922 | 8 % |
| our 7.832 | 7.832 | 0.925 | 8 % |
| our 7.690 | 7.690 | 0.972 | 3 % |
| our 7.544 | 7.544 | **0.981** | **2 %** |
| **the keyboard tape** | 7.543 | **0.981** | **2 %** |

### 4d. Verdict: UNDISCOVERED

Not "known but unheld" — the field is not attempting this and losing it. All
three runs pin the lock into the lip; two then hold it for a further 700 ms
*through the dead zone*, which says plainly that nobody has realised the wheel is
dead there, let alone that the decisive moment had already passed. On a map with
three recorded attempts that is unsurprising; on a map with nine hundred it would
have been found years ago.

---

## 5. The predictions from PLAN.md, scored honestly

PLAN.md was written before any search was aimed at the mechanism.

**Prediction 1 — thrust alignment orders the field. HALF CONFIRMED.** Within our
own lineage it is perfectly monotone and tracks the whole gain (0.925 → 0.981 as
the time falls 7.832 → 7.544, §4c). It does **not** rank the three humans —
h8.597 has the best alignment of anyone (0.956) and is 0.9 s slower, because it
flies a longer route where alignment is easy. A field of three, two of which fly
different lines, cannot support a rank-ordering claim, and I am not going to
pretend otherwise. The mechanism the prediction named is real; the statistical
form it was stated in is untestable here.

**Prediction 2 — the edge is *settling*: reach the right attitude and hold it
still, so a low-input tape would be near-free. WRONG MECHANISM, RIGHT
CONCLUSION.** Attitude churn over the flight:

| tape | finish | d\|roll\|/s | steer change events in flight |
|---|---|---|---|
| human WR | 7.893 | 2.88 | 47 |
| human #2 | 7.933 | 2.59 | 103 |
| our unconstrained | 7.544 | **3.36** | **250** |

The fast analog tape is *more* active than the human's, because the wheel
commands a rotation rate and holding an attitude is itself an active task. So
"settle and hold" is wrong. But the conclusion — that a low-input tape would be
cheap — is spectacularly right for a different reason (§6): the climb tolerates a
three-value alphabet for 11 ms.

**Prediction 3 — the decisive inputs are early in the flight and the last second
is inert. CONFIRMED, with a distinction I should have drawn in advance.** The
ablation puts 98 % of the gain before 5.5 s and 6 ms after. But freezing the
inputs any earlier than 7.4 s does not finish at all. **An input can be
load-bearing without being where the time is.**

**And the thing no prediction anticipated: 1.2 s of the lap ignores the wheel.**
Nothing in the playbook looks for a dead zone, and it inverted the story — I had
a complete, plausible, *wrong* write-up of "the field holds the lock through the
fall and we let go" before the range probe showed the fall accepts no steering
at all.

---

## 6. The low-input family — §B, and the surprise

The obvious thing does not work. **Project the whole tape onto any restricted
alphabet or minimum hold and it dies**: keyboard, 5-level, 9-level, 17-level;
20 / 30 / 40 / 50 ms minimum hold — all sixteen projections DNF, **and so do all
sixteen of the same projections applied to the human world record's own tape.**
Keep that measurement: this map kills a human's own run if you round its inputs.

Project **only the steered climb** (from race 4.100 s) and leave the ground
alone, and it inverts:

| projection of the climb only | result | vs AT |
|---|---|---|
| none (the 7.544 tape) | 7.544 | −0.160 |
| 13-level ladder | 7.546 | −0.158 |
| 5-level ladder | 7.547 | −0.157 |
| 9-level ladder | 7.564 | −0.140 |
| **keyboard, `{−127, 0, +127}`** | **7.577** | **−0.127** |
| 20 ms minimum hold | 7.578 | −0.126 |
| 30 ms minimum hold | 7.666 | −0.038 |

**A pure keyboard climb beats the author time by 0.127 s with no search at all.**
Search under the constraint and it gets better. The final family, every tape
re-validated in one batch with the human WR as control:

| tape | time | vs AT | ground events / values | climb events / values | min hold |
|---|---|---|---|---|---|
| unconstrained floor | **7.463** | −0.241 | 52 / 39 | 100 / 73 | 10 ms |
| keyboard climb | **7.474** | −0.230 | 55 / 42 | 24 / **3** | 10 ms |
| keyboard climb + 30 ms hold floor | **7.474** | −0.230 | 55 / 42 | **13 / 3** | **30 ms** |
| 4-value climb + 30 ms hold floor | 7.481 | −0.223 | 47 / 34 | 17 / 4 | 30 ms |
| event-thinned analog | 7.508 | −0.196 | 30 / 21 | 9 / 9 | 10 ms |
| **THE SIMPLEST ONE — 33 live inputs in the whole lap** | **7.514** | **−0.190** | **27 / 20** | **4 / 2** | 10 ms |
| fewest climb inputs, slower | 7.558 | −0.146 | 25 / 19 | 7 / 3 | 10 ms |
| — human WR, for scale | 7.893 | +0.189 | 28 / 19 | 36 / 26 | 10 ms |

Three things to notice.

* **The 13-press keyboard tape is not an approximation of the analog one — it
  flies the identical attitude program.** Mid-climb thrust elevation +21.3°
  against +21.2°, alignment 0.981 against 0.981, speed at y = 150 m of 697 km/h
  against 697.
* **The simplest tape does the entire four-and-a-half-second reactor flight in
  FOUR steering inputs using TWO values, and then holds the wheel centred for
  the last 2.3 seconds.** It has *fewer ground inputs than the human world
  record* (27 against 28) and is **0.379 s faster**.
* Going from 13 climb presses to 4 costs 40 ms. Both are inside the author time
  by a comfortable margin, so the choice is purely how much you want to be
  doing with your hands.

Free reductions found along the way, worth knowing generally:

* **The dead zone's steering is free junk.** Collapsing all of race 2.890 –
  4.100 s to one constant removed **71 change events at zero cost**. (The pedals
  in that window are *not* free — §2b.)
* **The tail freeze is free**: everything from race 7.560 s on.

**What is NOT reducible: the ground phase.** ~50 change events over race
0 – 2.890 s, of which the first 2.24 s are the world record's own inputs. Every
attempt to quantise or hold-floor it DNFs. That is where the map's difficulty
lives — and it is the same difficulty the three humans already handle.

---

## 7. Per-input tolerance, with the human's own tape as the control

"Recoverable" tolerance: mistime **one** input and let every later input move
with it — what a driver who is a beat late actually does. Cost against 7.476;
**anything up to +228 ms still beats the author time.**

### The presses of the climb (the 15-press tape this was measured on; the 13-press tape is its direct descendant)

| race | −50 | −30 | −20 | −10 | +10 | +20 | +30 | +50 |
|---|---|---|---|---|---|---|---|---|
| 4.10 / 4.41 / 4.51 s | DNF | +91 | DNF | +24 | +6 | +14 | +14 | DNF |
| 4.61 s | DNF | DNF | +10 | +2 | DNF | +41 | DNF | DNF |
| 4.84 s | DNF | DNF | DNF | +34 | +6 | +10 | +14 | +28 |
| 5.16 s | +6 | +4 | +2 | +1 | +9 | +11 | +25 | +41 |
| 5.25 s | DNF | +6 | +9 | +0 | +1 | +1 | +2 | +4 |
| 5.31 s | DNF | DNF | DNF | +8 | +1 | +2 | +4 | +8 |
| 5.39 s | DNF | +42 | +12 | +0 | +1 | +2 | +2 | +5 |
| 5.88 s | +12 | +15 | +15 | +0 | +2 | +1 | +4 | +6 |
| **6.65 / 6.68 / 7.15 / 7.21 / 7.49 s** | **+0** | **+0** | **+0** | **+0** | **+0** | **+0** | **+0** | **+0/+1** |

The first three presses (4.10 / 4.41 / 4.51 s) return *identical* rows: they are
one 400 ms phrase and only the total rotation they produce matters, not which of
the three you were late on. From 5.16 s the tape is comfortable, ±30–50 ms. **The
last five presses cost literally nothing however you place them, ±50 ms.**

Every single non-DNF entry in that table still beats the author time, most by
more than 0.2 s.

### The ground phase, and the control that puts it in perspective

Every input between race 2.58 and 2.85 s is **10 ms-critical**: one tick either
way and the run does not finish. That reads like a verdict of "impossible" until
you run the identical test on the human world record's own tape:

| tape, one input mistimed and everything after re-timed | overall DNF | ground | air |
|---|---|---|---|
| **human WR (the control)** | 46 % | **98 %** | 16 % |
| **our keyboard tape** | — | ~100 % | **3 %** |

**The world record's own run also dies on 98 % of ground-phase mistimings.** An
open-loop tape in a chaotic simulator is fragile whoever wrote it; a driver is
closed-loop, watching the road and the edge. The ground phase is not
extraordinarily hard — it is as hard as the lap three humans already drive,
because it essentially *is* their lap.

And in the air, where the open-loop measure is meaningful because there is
nothing to hit: **our keyboard climb DNFs on 3 % of mistimings against the world
record's 16 %. Our flight is five times more forgiving than theirs.**

---

## 8. The driving guide, off visual cues

Full throttle from the line. The brake comes on at 2.81 s and there are two
brake releases during the fall (below). Gas and brake are live pitch controls in
the air — this is a three-pedal map, not a steering-only one.

### Sector 1 — the road, 0 → 2.24 s. Drive it exactly like the world record.

Nothing to gain and everything to lose: our tape is the world record's own
inputs, and one tick of difference anywhere in here does not finish. Left off the
line at 0.52 s, straighten, the flicks at 0.90 / 1.38 / 1.75 s, on to the
booster.

### Sector 2 — THE ONLY HARD PART. The last 0.65 s of road, 2.24 → 2.89 s.

You are turning left onto the edge at about 260 km/h. Three things, and they are
the whole map:

1. **Turn in a fraction harder and earlier** — carry roughly 10 % more lock from
   2.5 s than feels natural.
2. **Do not fully unwind at 2.70 s.** The world record lets the wheel go to
   centre here; keep a whisker of left in it.
3. **THE ONE THAT MATTERS: do not slam to full lock into the edge.** Reach peak
   lock at about 2.77 s, *just short of the stop*, then **feather out of it all
   the way to the lip** — roughly a tenth of the wheel unwound over the last two
   tenths of a second. Brake as you begin unwinding, not before.

   **The cue is the edge of the road, not a clock:** you should be *unwinding* as
   the front wheels reach the lip, not fighting the stop.

**What you are actually doing:** setting the spin you fall with. From the moment
the wheels leave, **the steering wheel does nothing for 1.2 seconds** — there is
no saving it and no steering it. Hold the lock to the edge and you tumble faster,
meet the launcher nose-up 58°, and it eats 109 km/h. Feather out and you meet it
at 35° and it eats 56.

**The check, and it is a number on the speedo:** at the instant the launcher
throws you, about 3.60 s —

* **148 km/h** — you drove it like the current world record. ≈ 7.89.
* **170 km/h** — about the author time.
* **197 km/h** — the fast line.

### Sector 3 — the fall, 2.89 → 4.10 s. Hands off the wheel; feet still working.

Steering is ignored — measured, at 20 ms resolution, on two independent tapes.
(Both the world record and rank 2 spend this time holding full lock, which is the
clearest sign nobody knows.) **The pedals are not ignored**: the fast tapes let
the brake off briefly at about 3.53 s and again into 3.83 s, straddling the
strike at 3.60 s, and those are 10 ms-tight. If you want the simple version, hold
the brake like the world record does and give up a few hundredths — the 0.228 s
is not in these.

### Sector 4 — the climb, 4.10 → 5.90 s. Four inputs, and all of the time.

Control returns at **4.10 s — the moment the launcher has finished with you and
the reactor takes over.** One idea:

> **Stop the car swinging nose-up. Point its belly at the far side of the map,
> not at the sky.**

The reactor pushes out through the floor of the car at a constant 4.5 g. Belly
aimed 50° up and you spend half of it going nowhere; aimed 20° up and you spend
it going where the gate is. **The horizon is your instrument: the field lets the
nose keep climbing; you have to check it and hold it low.**

The complete keyboard script for the 13-press version. `L` = full left,
`R` = full right, `—` = centre.

```
  4.10 s  R      as control returns — the catch
  4.12 s  —
  4.34 s  R
  4.41 s  —
  4.51 s  L
  4.61 s  —      <-- the "check the nose" release
  4.84 s  L      .......... 320 ms
  5.16 s  —
  5.25 s  R
  5.32 s  —
  5.39 s  L      .......... 480 ms, the long one
  5.87 s  —      from here on nothing you do is worth more than a few ms
  7.54 s  L      hold to the gate
```

**And the version to learn first, because it is barely a script at all.** The
7.514 tape flies the whole climb on four inputs and two values:

```
  4.10 s  (R already held from before control returned)
  4.50 s  L
  4.61 s  —
  4.84 s  L      .......... 320 ms
  5.16 s  —      and CENTRE for the last 2.3 seconds to the gate
```

That is 0.190 s inside the author time and 0.379 s faster than the world record.
Its climb inputs tolerate ±30 ms; several of the mistimings come out *faster*
than the tape itself, which is the clearest sign it is not a knife edge.

Timing notes: the presses around 4.1–4.5 s are one phrase — only the total
rotation they produce matters, not which of them you were late on. From 5.16 s
everything has ±30–50 ms of room, and **from 5.9 s the lap is over: the closing
presses cost zero however you place them.**

### Sector 5 — 5.90 s to the gate. Already decided.

98 % of the gain was banked at 5.5 s. Freeze everything from 7.40 s and the time
does not change at all.

### Is this humanly realistic?

**Yes, and the hard part is the part they already do.** Sector 1 is the world
record's own driving. Sector 3 needs no steering. Sector 4 is four inputs on quarter-second holds
with tens of milliseconds of slack (thirteen if you want the extra 40 ms), five
times more forgiving than the world record's own flight. **All the difficulty is a
650 ms stretch of road that all three humans already negotiate at 260 km/h — and
the change asked for there is not a new input, it is taking one away: stop
pinning the wheel into the lip.**

---

## 9. Files

| file | what |
|---|---|
| `tapes/m274191_x1_7463_*.Ghost.Gbx` | the unconstrained floor, 7.463 s |
| `tapes/m274191_simpR1_*.Ghost.Gbx` | **the deliverable** — 33 live inputs, FOUR of them in the flight, 7.514 s |
| `tapes/m274191_L2_7474_*.Ghost.Gbx` | keyboard climb, 13 presses, 30 ms hold floor, 7.474 s |
| `tapes/m274191_L1_7474_*.Ghost.Gbx` | keyboard climb, 24 presses, 7.474 s |
| `tapes/m274191_L4_7481_*.Ghost.Gbx` | 4-value climb, 7.481 s |
| `tapes/m274191_simpR2_*.Ghost.Gbx` | event-thinned analog, 45 events, 7.508 s |
| `tapes/m274191_simpP2_*.Ghost.Gbx` | 32 live events, 7.558 s |
| `tapes/m274191_TAS_*` | the whole improvement ladder, each re-validated when banked |
| `ghosts/` | the three human runs, as served by trackmania.io |
| `map_274191_v1.Map.Gbx` | the map, from Nadeo's own CDN |
| `btraj/f_*.csv` | per-tick position / velocity / quaternion for every tape |
| `evidence/analysis-v1.txt` | thrust-axis, elevation, alignment, station tables |
| `evidence/tolerance-v1.txt` | tolerance tables including the human control |
| `VALIDATION-v1.txt`, `VALIDATION-final.txt` | raw oracle transcripts and sha256s |
| `PLAN-v1.md` | the plan and its predictions, written before the search |
| `tools/an-v2/` | the analysis binary (Rust, no deps) |
| `tools/tmprobe.rs`, `tools/project.rs` | the new probes and the projection module |
| `tools/ratchet.sh` | the harvest-and-re-validate loop |
| `tmtas-274191-src-v1.tgz`, `fk-274191-clockfix-v1.tgz` | the patched toolchain |

---

## 10. Transferable findings

### The one that generalises furthest

**A property measured on one tape's neighbourhood is not a property of the map.**
I told the 203330 agent that the sub-tick plane has to be armed per lineage; they
came back and told me dead zones do too, and they were right — my own dead zone
is steering-only, and the pedals inside it are inert for the human's lineage and
10 ms-critical for ours, because our search put load-bearing inputs into a window
the humans left empty. Two independent instances of the same fact. **Re-measure
per lineage, and say which lineage a claim was measured on.**

Three rules attached to it, from that exchange:

1. **Sweep finer than the tolerance you are claiming.** A 70 ms tolerance sampled
   at 100 ms spacing looks like indifference.
2. **Test the null the other way.** "Any constant works here" and "this input
   does nothing" are different claims. Verify both.
3. **Dead zones are a lineage property.** Mine reproduced on two lineages at
   20 ms resolution *for steering*, and failed to reproduce *for the pedals*.

### Instruments

* **`fk btraj`'s clock locator was looking the wrong way.** It streams a fixed
  16 KB *below* the vehicle state for the `u32` that ticks +10 — where every
  map-2 run put it. Here the clock is **above** the state on some tapes and
  **319 KB below** on others, and the failure presents as
  `no u32 advances by exactly 10 every tick near the vehicle state`, which reads
  like a broken server. Fixed by laddering the window:
  `(16 K, 256) → (64 K, 64 K) → (192 K, 192 K) → (512 K, 512 K)`.
* **`fk btraj`'s self-check threshold was speed-blind.** It rejected any
  trajectory whose position derivative disagreed with the velocity triple by more
  than a fixed **2.0 m/s** — calibrated on a map topping out near 90 m/s. This
  car crosses the line at **215 m/s**, where a one-tick central difference
  legitimately disagrees by ~1 % of speed. Now `max(2.0, 3 % of mean speed)`.
* **`--quant` was only wired into the FORK path.** In the hardened build a
  classic-path arm given `--quant` runs completely unconstrained and its log
  looks perfect. Added `tmsearch::project` (quantise + min-hold, unit-tested),
  applied in the classic candidate loop, to the starting state, **and only over
  `[--lo, --hi)`** — projecting the whole tape destroys a prefix the search was
  told not to touch, which on this map is the entire difficulty.
* **`simplify`'s ramp-collapse phase can loop for ever.** A successful collapse
  can leave a structure that still parses as a ramp at the same place; two runs
  printed the same "collapsed ramp 431..434" line for twenty minutes and never
  reached step 3. Capped.
* **New probes in `tmprobe`**: `range` / `rangesweep` (force a constant over a
  window — this is what found the dead zone), `hold`, `jitter`, `slide` /
  `recover` (raw and recoverable tolerance, filtered to a tick range), `bmove`
  (shift the pedal transitions only), `gasrange`, `cmp` (side-by-side input dump
  of two tapes), `slew`, `events`, `blocks`.

### Method

* **On any map with an air phase, sweep a constant through it before writing a
  word about what the driver is doing.** One minute of box time; it was the
  difference between the correct write-up and a completely plausible wrong one
  that I had already written.
* **Read the physics before searching.** Twenty minutes with a quaternion and a
  finite difference established that the map's whole content is "where is the car
  pointing". On a map tagged `Reactor`, do this first.
* **Constrain only the part of the tape that tolerates it.** Whole-tape
  projection said "no low-input tape exists here" — sixteen projections, all DNF,
  including the human's own tape. Projecting only the steered climb produced a
  keyboard tape 0.127 s inside the author time *with no search at all*. The
  difference between "impossible" and "free" was one flag.
* **Always run the human's own tape through the fragility test.** 98 % of
  ground-phase mistimings kill the world record too. Without that control our own
  98 % looks like "not humanly executable", which UNBEATEN.md correctly forbids.
* **A three-record leaderboard is a different problem.** No field to mine for
  correlations, no keyboard run to read an alphabet off, no consensus route. What
  replaced it: a mechanistic prediction stated in advance and tested against our
  own improvement ladder — which turned out to be a *better* instrument, because
  every rung differs from the next by one thing.

### Search

* **Multi-operator, high-temperature moves break a chain-dependent map.** Four
  single-operator arms at T = 3 sat between 7.828 and 7.856 for fifteen minutes.
  One arm at **T = 8 with `--nops -3` and a 240-tick window** went 7.690 → 7.558
  in six minutes and every arm re-seeded from it followed. Where each input's
  effect is conditioned on all the previous ones, a single perturbation is almost
  always worse; you have to move a whole phrase.
* **A plateau at ten minutes is not a plateau.** The first arm sat at 7.832 for
  five minutes, then found 7.690 in the next three. Re-seeding every arm from the
  global best every ten minutes, with 2 % island migration, beat any operator
  tuning.
* **Seeds:** all arms seeded from the human WR converge to one basin; the rank-2
  seed (mirrored roll) stayed ~40 ms behind throughout; the rank-3 seed is a
  different, worse route. Seed choice was worth nothing *within* a route and
  everything *between* routes.
* **Constrained arms need re-seeding carefully.** A keyboard projection that
  finishes on one champion's climb DNFs on the next champion's — the constrained
  arm has to be re-seeded from a tape whose projection is *checked* to finish,
  not from the global best on principle.
