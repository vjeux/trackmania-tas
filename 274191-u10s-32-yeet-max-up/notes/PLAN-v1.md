# PLAN — map 274191 `U10S_32 By Everios96 [Yeet] MAX-UP`

Written 2026-08-18 on `105213.od.fbinfra.net` (176 cores), after acquisition,
the identity control, and the 20-minute reconnaissance below. Baseline searches
were launched during the reconnaissance; **every prediction in §4 is written
before any search is aimed at it.**

| | |
|---|---|
| uid | `ieMJgDjCjWkZ45apoiK2DPmQeQg` |
| Nadeo mapId | `dddea8ed-682a-4e42-9566-3035683ddba5` |
| TMX id | 274191 |
| author | TwinsNaA |
| author time | **7704** |
| human online WR | 7893 (`Whatever8319`) |
| records | **3** |
| gap | 189 ms |
| tags | **Reactor**, Remake |
| map md5 | `20a56be3b87b3dfa3ff9d35e523ec25c` (Nadeo CDN, anonymous) |

---

## 1. Controls that already passed

* **Identity control — 3/3.** All three ghosts re-simulate to their exact
  leaderboard millisecond: 7893, 7933, 8597.
* **ACQUISITION.md §8 whole-field check — 3/3 = 100 %.** The leaderboard has
  exactly three records and *all three* were downloaded and re-simulated, so
  this is not a sample: the entire recorded population of this map reproduces
  in our oracle. The map is falsifiable. (Worth stating because 203072 failed
  this at 29 %, and the suspected cause there was *a Reactor item changing
  behaviour between builds* — this map is Reactor-tagged, and it passes.)
* **Codec round-trip — 3/3.** Each ghost rebuilt through `tmsearch --verify`
  re-validates to the same millisecond.

## 2. Is the AT hand-driven or a formula?

| medal | ms |
|---|---|
| author | **7704** |
| gold | 9000 |
| silver | 10000 |
| bronze | 12000 |

Gold/silver/bronze are round to the second; the author time is not. That is the
signature of a **driven validation lap** with formula medals below it — a human
sat down and drove 7704. `atSetByPlugin` is not set on this record. So
"not humanly executable" is excluded by construction, per UNBEATEN.md §A.

## 3. What this map actually is — the reconnaissance that changes everything

482 blocks, 1141 items, **no intermediate checkpoints**: one `PlatformPlasticStart`
at cell (14,13,15) and one `PlatformPlasticFinish` at cell (31,38,27). Start
(464, 42, 496); finish crossed at about **(991, 244, 876)** by all three
runs — 527 m across, 380 m along, and **202 m straight up**. "MAX-UP" is
literal.

**63 % of the lap is airborne**, and the last ground contact of the world record
is at **2.85 s** of a 7.89 s lap. The car then accelerates from 250 km/h to
**772 km/h while in free air.** Something is thrusting.

### 3a. The thruster is bolted to the car's own body axis

`an axes` rebuilds the body frame from each telemetry sample's quaternion,
differentiates the recorded velocity, subtracts gravity, and projects what is
left onto the body axes. (Convention confirmed against the standing start:
at t=0 body-forward = world +X, which is the direction the car drives off the
line, and body-up = world +Y.)

Ground phase: the residual sits on **+forward** — engine and boosters, as it
must.

Airborne phase, world record, every sample from 3.7 s to the finish:

```
  t_ms   |a-g|  onFwd   onUp  onRgt      roll
  4300    64.9   40.3  -50.8    1.9     -2.32
  5000    46.3   18.5  -42.3    2.7     -2.99
  5900    43.7   12.9  -41.7    1.2      2.37
  6500    45.2   11.3  -43.6    3.3      2.28
  7100    44.7   -1.4  -44.6    3.3      2.28
  7500    45.7  -12.5  -44.0    0.1      2.36
```

**`onUp` is pinned at −44 m/s² for four and a half seconds.** Averaged over the
whole airborne phase the non-gravity acceleration is 87 % on the body-up axis
with mean cosine **−0.86**, on all three tapes independently.

So: **the reactor applies a constant ~44 m/s² (4.5 g) along the car's −up axis —
out through the floor — and nothing else.** Where you go is decided entirely by
**where the car is pointing**. The engine is irrelevant after 2.85 s; the
steering wheel is not a steering wheel, it is an attitude jet.

That is also why every run flies **nearly inverted**: roll settles at
±2.3 to ±3.1 rad (130–180°), which is what it takes to aim "out through the
floor" at the sky. The world record flies the whole climb at roll ≈ +2.3 rad;
rank 2 flies it mirrored at ≈ −3.0 rad. Both work.

### 3b. There are three routes in a field of three

This is the rarest thing on the list so far — with three records, the route is
genuinely unsettled.

| | route |
|---|---|
| r1 7893 | accelerate east, booster at 2.05 s, **leave the road at 2.85 s** and fall 32 m to y≈10, hit the launcher at 3.55 s (232→148 km/h), fly |
| r2 7933 | the same route, mirrored roll, and it **touches down again at 7.85 s** just short of the finish |
| r3 8597 | **a different map.** Lifts off the gas at 2.0 s, brakes to 27 km/h, reverses, and at 3.5 s stands the car on its nose (pitch 1.42 rad) against a wall at (511,40,518) and launches **vertically from y=42** — never uses the drop at all. Reaches **942 km/h**, the fastest of the three, and still loses 700 ms because it spent 1.5 s stopped. |

r3 looks like a botched run recovered by a second launcher rather than a
considered line, but it proves a second launch point exists and that the flight
phase can carry far more speed than the WR extracts from it.

### 3c. Where the time is, at 50 ms resolution

r1 and r2 are the same route and differ by 40 ms. r1 is ahead of r2 by ~15 ms
at the drop and the rest appears in the flight. There is no meaningful sector
correlation to compute across a field of three, so the usual "rank sectors by
correlation with finish time" instrument does not apply here — the substitute
is §4, which is a *mechanistic* prediction rather than a statistical one.

---

## 4. THE HYPOTHESIS, STATED BEFORE SEARCHING FOR IT

The brief's attitude hypothesis (227969: the field rolls and pays in exit speed;
203330: roll at the lip orders the field perfectly) is not merely *expressible*
on this map — on this map it is the **entire physics**, in its strongest
possible form. Thrust is a body-axis vector of fixed magnitude. Attitude is the
only thing anyone controls, and every degree of misalignment is multiplied by
44 m/s² for four and a half seconds.

**Prediction 1 (alignment).** Define the thrust-alignment as
`cos θ` between the thrust direction (−body_up) and the unit vector from the car
to the finish gate, averaged over the airborne phase. I predict this quantity
**orders the three human runs**, and that all three are well under 1 — i.e. the
field is spending a measurable fraction of a 200 m/s Δv budget pushing sideways.

**Prediction 2 (the wobble).** The three humans do not hold an attitude; they
fight it. The world record's steering during the flight flips between full lock
left and full lock right (−1, +1, +0.88, −0.86, −1, 0, +1, …, 39 change events
over the lap, 22 distinct values) and its roll visibly oscillates around the
mean. I predict the TAS's edge is **settling** — reaching the right attitude
earlier and then holding it still — and that a *low-input* tape will therefore
be near-free on this map rather than costly, because the optimum is
intrinsically a small number of long holds. This is the opposite of the usual
trade-off, and it is a strong, falsifiable claim.

**Prediction 3 (the timing of the settle, not its value).** Since thrust is
constant, the integral is what matters: getting to the right attitude 200 ms
earlier is worth more than holding a marginally better attitude. So the decisive
inputs are predicted to be **in the first second of flight (3.6–4.6 s)**, not
near the finish, and the last second of the run is predicted to be nearly
inert — the same "the spectacular closing feature is worth nothing" result that
227969 and 270051 both found.

**A contradiction is as valuable as a confirmation and will be reported either
way.** In particular, if the searches put their gains in the last second rather
than the first, Prediction 3 is dead and the story is something else.

---

## 5. The attack, in order

1. **Baseline searches (running).** Four arms, distinct `--root` per process
   (defect 1): two from r1 with different RNG seeds, one from r2, one from r3.
   Purpose: establish whether the seeds' basins merge, which the brief says is
   worth minutes to find out. *Status at +3 min: r1 arms at 7838/7844, r2 arm at
   7880, r3 arm at 8528.*
2. **Test the three predictions against the human population** before spending
   search budget on them (§4).
3. **The route question.** r3 proves a second launcher. Measure whether the
   drop-route is even the right one: the drop costs 32 m of altitude and
   84 km/h at the launcher. If the vertical launcher can be entered at speed,
   it may be a different and better map. This is the "the route may be the open
   question" branch of the brief and it gets an explicit test, not an
   assumption.
4. **Attitude-aware search.** If §4 holds, the productive operator space is
   long holds of steer, not per-tick jitter — i.e. `--quant` / low-input search
   is not a concession here, it is the *right* search space. Run it in parallel
   with the unconstrained arms rather than after them.
5. **Robustness scoring** (`worst time over a ±1–2 tick placement window`) on
   the decisive inputs, per the brief and 270051.
6. **Deliverables per UNBEATEN.md §A and §B.** §B is mandatory here: the gain
   lives in a reactor phase by definition.

## 6. Known defects being respected

1. distinct `--root` per concurrent `tmsearch` — yes, `/dev/shm/sr_<arm>`.
2. fork resume boundary — the classic path is being used for the baseline; any
   fork work will keep the mutation window above the resume tick.
3. fork child clock — per-worker calibration, or don't read absolute times.
4. modelled sub-tick plane — **presumed invalid here until measured.** This map
   finishes airborne at 178 m/s with roll spread over 1.5 rad across the field,
   which is exactly the 227969 configuration that produced a confident wrong
   answer. If sub-millisecond adjudication is needed it will be the **gate
   ladder** (`tmmaps places`), which is adjudicated by the real trigger.
5. relocated gate maps keep the mapUid — one worker root per map.
6. `~/persistent/private-30d` is not read-your-writes — write-once versioned
   filenames, verified by md5 from this node.

## 7. Success criteria

Beating 7704 is half. The other half is a technique a human can practise:
per-input tolerance, a low-input family with event counts and alphabets, and a
sector guide phrased off visual cues. On a map whose whole content is "point the
car at the sky and hold it", the visual cue is likely to be the horizon — which
is, pleasingly, exactly what 227969 landed on.
