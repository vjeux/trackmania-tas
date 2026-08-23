# You love water

**The whole map is one 71 m gap, jumped four times. Cross the lip at about
300 km/h and you fly it; cross at 255 and you fall into the tube below and lose
9–15 s. The only recorded human run misses it three times out of four.**

| run | time | vs author time | note |
|---|---|---|---|
| **TAS** | **97.325** | +46.866 | one life, no respawns |
| Human record — brick555 | 440.238 | +389.779 | contains 31 respawns |
| Author time | 50.459 | — | never beaten |

TMX map [284238](https://trackmania.exchange/maps/284238) · author
**Eating_My_Wings** · 4 checkpoints · **exactly one recorded run**.

The author time still stands. What follows is what the map is and where its time
goes, not a route that beats it — **the sector-by-sector guide for this map is
not written yet.**

## The map is one obstacle, built four times

186 blocks: the same 40-block module placed four times, plus the finish. Route
order is copy 0 → copy 3 → copy 1 → copy 2 → finish, and each copy is the same
cycle:

```
  launcher lane, two boost pads   ->  ~300 km/h
      |
      v  ~3.1 s of flight
  CHECKPOINT
      |  a curved chute, dropping ~100 m over ~3.5 s
      v
  THE LIP
      |
      |   *** THE GAP: 71 m across, 32 m down ***
      v
  FAR LIP  -> ride the half-pipe tube down and out onto the next launcher
```

Every one of the four timed cycles launches off a **water ramp**. Fall short of
the gap and you land in the closed end of the tube — the "bowl" — and have to
climb back out.

## Where the time goes: the lip

Across the 23 attempts inside the human record:

| speed at the lip | attempts | outcome |
|---|---|---|
| **302 and 305 km/h** | 2 | cleared the gap; reached the exit lane 5.300 s later |
| 61–255 km/h | 21 | fell in; when they reached the exit lane at all it took 14.5–20.4 s |

The threshold is **bracketed between 255 and 302 km/h**, not measured. Missing
the gap costs 9–15 s, and the record misses it in three of the four copies. That
is the 40 s of the run right there, before any respawns.

Lip speeds in that record, by copy: **305 km/h** (copy 0, cleared), then 240, 251
and 210 km/h — all three fell in at 89–129 km/h.

## Why copies 1–3 are so much harder than copy 0

Whether you arrive at the lip fast is decided far earlier, at the **wall curve**
after the launch — specifically by how far along the wall you meet it:

| | where the car meets the wall | speed lost in one tick | checkpoint crossing |
|---|---|---|---|
| the recorded run, cycle 1 | z 923.4, 77.4 m/s | 8.71 m/s | 45.8 m/s |
| off the start platform, copy 0 (works) | z 915.4, 73.1 m/s | — | — |
| a human on the author's remix | z 913.9, 80.8 m/s | 0.75 m/s | 69.4 m/s |

**Nine and a half metres of lateral position** separates a cycle that keeps its
speed from one that throws it away, and checkpoint speeds decay
52.8 → 45.8 → 41.1 → 37.4 m/s as the cycles go on because each bad contact feeds
the next.

**The obstacle is a surface to be ridden, not a gap to be jumped.** The human
crosses the ice kicker and the wall curve with his wheels **loaded the whole
way**, rolled 90° onto the car's side. Copies 1–3 launch our car off the kicker
instead, and from the instant the suspension fully unloads its angular velocity
is exactly constant and every input is inert — so the wall contact is decided by
the rotation at the ramp exit and by nothing afterwards.

Body-frame angular velocity from the ramp exit, in °/s:

| | exit | +0.10 s | +0.20 s | +0.30 s | +0.50 s |
|---|---|---|---|---|---|
| the human's 46.112 (works) | 222 | 150 | 92 | 84 | 68 |
| ours, copy 0 (works) | 275 | 142 | 81 | 57 | 46 |
| ours, copy 1 (fails) | **284.1** | **284.1** | **284.1** | **284.1** | **284.1** |
| his inputs grafted, copy 1 | **267.7** | **267.7** | **267.7** | **267.7** | **267.7** |

Identical to every printed digit for 40 consecutive ticks: a free rigid body.
His dampers read 0.039 at the first no-contact sample and extend *gradually* to
0.180 over 450 ms, never reaching full extension — he never leaves the surface.

The decisive test: grafting the human's own steer, gas and brake from the kicker
onto a launch matched to his state (2.61 m, 98.31 against 98.10 m/s, slip/pitch/
roll within 1.5°) gives roll at the wall **−150 against his +86**, flat across a
±60 ms phase sweep. **This is not a missing input sequence; it is a missing
state, and the state is angular velocity.**

Steering forced over the true flight window returns bit-identical state at the
wall for −1, −0.3, +0.3 and +1 — on copy 1 *and* on copy 0 (0.6° across the full
range).

Position, by contrast, is essentially solved — the human's crossing point is
reached to **0.58 m**, from 9.73 m earlier the same day. Roughly forty
coordinate-descent runs across eight objective variants, windows from tick 2000
to 2365, seeded both randomly and from the incumbent's own inputs, put roll at
the wall in **−131 … +176** and never within 100° of the human's +86.

The kicker's geometry is also cleared, on all three axes at once: applying
copies 1–3's *whole* offset from the sibling map (+0.70, −1.00, −0.29) to copy
0's kicker, in a matched A/B so the car arrives bit-identically, leaves the
working launch working — roll within 2.5° of control throughout. It is not what
separates the copies.

So the remaining lever is narrow and upstream: the ramp does the same thing to
whatever it is handed, so the target is the state at the **ramp entry** — the
tube and the arc, upstream of anything searched on this map — aiming to unload
at ~95 °/s instead of ~270.

Lateral position on the flat before the kicker is still what separates the
cycles, and copies 1–3 do not have that flat:

| | time on the flat | sideways speed achieved |
|---|---|---|
| copy 0 — start platform, ~100 m of deck | ~2 s | −17.9 m/s |
| copies 1–3 — fed by the tube | ~0.6 s | −1.9 as driven … −15.7 at full lock |

The tube is the only connection between one copy and the next, so copies 1–3
always arrive on the lane 100 m late with 0.6 s of flat left — and in 0.6 s the
car cannot build the sideways speed the wall contact wants.

It is **not** grip (full lock buys 13.4 m/s of sideways speed on this lane), not
speed (the kicker is crossed at 97.2 m/s on the attempt that fails and 90.9 on
the standing start that works), and not the six boost pads on the lane (they sit on the flat
*after* the aim is decided). It is also **not** the kicker's height: lowering
copy 0's kicker by 1.00 m in a matched A/B — one block's f32 `y`, so the car
arrives in a bit-identical state — leaves the roll profile essentially unchanged
(+105.2 against +108.4, both locking at +89.5 and driving the wall).

The one encouraging measurement: a standing start off copy 0 flies the good line
to within 2–7 m of it, point for point. The line is not exotic and this car can
drive it — copy 0 has the run-up that produces it and the others do not.

The author made exactly this substitution himself on his remix **279008 "Keep
dropping"**, which shares 167 of its 186 blocks with this map but replaces the
water ramps with tech blocks that give every copy a flat run-up. A human beats
that map's author time. Nobody has beaten this one.

## What the obstacle actually is: the state at the kicker is matched, and it is not enough

*Claims below are tagged **MEASURED** (with the control named), **INFERRED**
(with the inference stated), **UNKNOWN** (an open task, never "there is no X")
and **SUPERSEDED** (pointing forward).*

> **SUPERSEDED — the "engagement point law" that used to stand here.** It said
> the frozen roll rate is a monotone function of `engage_x` — 912.5 → 145 °/s,
> 914.6 → 126, 918.0 → 71 — measured over 27 tapes. The 27 rows are correct and
> the law is not. Move the *kicker* instead of the car, one f32 on the map with
> the same tape and the same inputs, and `engage_x` travels 3.6 m for **12 °/s**:
>
> | map | engage_x | frozen roll rate | the law predicted |
> |---|---|---|---|
> | untouched | 910.76 | **284.1** | — |
> | kicker −0.50 m | 912.85 | **258.7** | ≈ 140 |
> | kicker −1.00 m | 914.34 | **272.7** | ≈ 126 |
>
> It was a within-family correlation of the brake sweep — the only family in
> that table with any variation, and the one in which the engagement point and
> the crossing angle move together.

### The number that decides it, measured on the human's own tape

**MEASURED.** The control is inside the experiment: these are perturbations of
the run that WORKS, on its own map, so the positive case is not assumed.

The discriminator is **`vz` at the kicker: the crossing angle.** It is not
inferred from our failures — it is measured by perturbing the run that works.
Twenty-four one-flag perturbations of Yhomas_TM's 46.112, on his own map,
through one readout:

| crossing angle at the kicker | outcome | speed at the checkpoint |
|---|---|---|
| −26.34 | rides | 65.26 |
| −25.00 (unperturbed) | rides | **69.40** |
| −23.72 | rides | 71.70 |
| **−22.91** | **unloads** | 72.34 |
| −22.09 | unloads | 55.47 |
| −19.77 | unloads | 43.83 |
| −18.37 | unloads | **22.44** |

Every row at −23.72 or steeper keeps its wheels loaded; every row at −22.91 or
shallower launches. **The threshold is a 0.8 m/s window at 99.4 m/s** and a
6/127 steer offset held half a second crosses it. His tolerance in lateral
position is the same size: +0.26 m still rides, +0.43 m does not.

**284238's own record engages the kicker at vz −2.3**, and the best synthesised
shot at −18.2. The perturbation that puts *him* at −18.4 arrives at the
checkpoint at 22.44 m/s against his own 69.40. That is this map's whole deficit,
in one number, on his map, with his car.

### What our lane can deliver: everything, and it is not enough

**MEASURED.** Every restart was replayed through the readout and scored on the
OUTCOME, not on the objective it minimised — the argmin of a proxy is not the
population that does the thing.

Six searches, 60 restarts, aimed at his kicker state shifted by the launcher
assembly's own offset so it is the same place relative to the kicker. The last
two put the crossing angle and the speed on **hard bars** instead of into a
weighted sum, which is what a weighted sum could never do — let a candidate buy
attitude with crossing angle is exactly the trade the surface is trying to force:

| | target | best reached on our lane |
|---|---|---|
| position | 0.00 | **0.07 – 0.19 m** |
| speed | 99.22 | **100.00** |
| crossing angle | −25.20 | **−25.13** |
| body angular velocity | (−54.0, +15.2, −57.3) | **(−55.4, +15.3, −58.7)** |
| roll | +1.23 | **+1.13** |

**Every one of them is reachable, and the car still launches.** Across that whole
frontier the frozen rotation only falls from 284 to **208 °/s** — against the
~90–100 the wall contact needs, and against a human tape that never freezes at
all. Every restart was replayed and scored on the **outcome** rather than on the
objective it minimised; none of them rides.

### Where the two runs actually part: 17 m after the kicker

**MEASURED** (both traces from the live engine at 10 ms, aligned on canonical x).
**INFERRED**: that the 4.3 m/s our car is down at that point is WHY contact ends
there. That is one pair, not a control. The causal test is a tape at his speed
AND his attitude 17 m up the curl, and it is **UNKNOWN** — a search for exactly
that is running as this is written, and had reached his place and his attitude
at 68.93 m/s against his 92.84.

Aligned on canonical x, his tape against our best:

| x | his height above the deck | his roll | his ω_z | his speed | our height | our roll | our ω_z | our speed |
|---|---|---|---|---|---|---|---|---|
| 913.0 | 2.735 | +2.8 | +96 | 98.17 | 1.63 | +3.2 | +37 | 94.09 |
| 925.9 | 8.682 | +16.7 | +157 | 94.83 | 7.97 | +16.4 | +157 | 90.65 |
| 933.3 | 14.450 | +40.6 | **+242** | 92.84 | 13.29 | +38.0 | **+240** | 88.54 |
| 936.9 | 17.305 | +52.9 | +189 | 91.94 | 15.88 | +49.5 | **+240 frozen** | 87.69 |
| 940.5 | 20.083 | +61.2 | +129 | 91.05 | — airborne — | | **+240 frozen** | |
| 950.6 | 27.480 | +76.2 | **+87** | 88.65 | — airborne — | | **+240 frozen** | |

The two cars cross the ice kicker within 2.6° of roll and 0.7 m of height of one
another and **both reach about 240 °/s**. At x 933.7 ours goes rigid and holds
240.3 to the last printed digit. His keeps a wheel loaded for another 17 m and
the rate bleeds off — 242 → 189 → 129 → **87** — arriving at the wall at exactly
the rate the contact needs.

**So the obstacle is not decided by the state at the kicker.** It is decided by
whether contact survives the last 17 m of the curl, and by then our car is
4.3 m/s slower, having paid that on the water lane.

### The lane is a trough, and the trough is the block

**MEASURED** (the profile, 25 tapes; the stored rotations, read off both maps).
**INFERRED**: that the car's ROLL is the proximate cause rather than some other
property of the water block. It is the residual by elimination and the direct
trend is weak — across tapes holding vz in [−26,−22] at ≥ 93 m/s the frozen rate
against arrival roll reads +2.2 → 217, +2.8 → 238, +3.5 → 242, +3.6 → 217,
+4.5 → 249, +4.8 → 245, which does not extrapolate to a ride at +1.2.

The launcher deck on copies 1–3 is a shallow **valley** — the car's own height on
it, at canonical x 902.5, over ten tapes at 99–100 m/s:

```
z 921.7  y 1873.258  roll +4.8      z 927.5  y 1873.088  roll -0.1   <- the floor
z 924.9  y 1873.104  roll +2.4      z 933.2  y 1873.428  roll -4.2
```

**−0.78 ° of roll per metre across the lane**, level only at z ≈ 927.4, where the
sibling's tech deck and our own copy 0's start platform are flat (0.09 °/m). It
is a real coupling between roll and lateral position and it is why the naive
searches could not hold both — but it is **not a bound**, because a barred search
beats it.

And it is not a placement. Every launcher deck block on both maps is stored with
**pitch = roll = 0**; the trough is the shape of `PlatformWaterRampBase`, which is
a channel, against `PlatformTechBase`, which is a flat platform. The author did
not tilt anything — he changed the block. So "is it the roll or is it the water?"
is not a question that can be asked here: they are one property of one block, and
neither the position mover nor the new rotator can take them apart.

### The matched pair, on one map

**MEASURED.** One map, one car, one kicker model, the crossing angle matched to
0.30 m/s.

| | deck | speed | crossing angle | roll | frozen ω | checkpoint |
|---|---|---|---|---|---|---|
| our copy 0 | `PlatformTechBase` | 91.06 | −17.94 | **+0.82** | **55.8** | 52.85 |
| our copy 1 | `PlatformWaterRampBase` | 99.81 | −18.24 | **+4.96** | **284.1** | 37.10 |
| 279008 copy 1 | `PlatformTechBase` | 99.35 | −25.00 | +1.80 | never freezes | 69.40 |

Same map, same car, same kicker model, the same crossing angle to 0.30 m/s — and
the flat deck rides while the trough does not. **And 4° of roll is not
driveable**: eighteen steer deltas on copy 0's flat deck span the whole reachable
band, −5.90 … +1.62, so copy 1's +4.96 is the deck and not the driving.

### What this closes

Each line is **MEASURED**, with its control named.

* **The 1.00 m kicker.** A rigid offset on a **four-block assembly**
  (`tmtraj blockdiff`; found independently by the claims audit on the same day), of which exactly one block is free-placed — so the two
  earlier experiments that "raised the kicker by 1.00 m" raised a quarter of it
  and built a step: entry speed 99.81 → **50.84**. And the deck already pays for
  it: the car's height above its own kicker matches the human's to **8 mm**.
* **Copies 2 and 3.** The vector from a water ramp's anchor to the ice kicker is
  (32.572, −1.000, −0.164) on copy 1, copy 2 and copy 3 — identical to 1 mm.
* **An airborne approach.** The only candidate crest is the floor of the trough,
  and a floor is not a crest: `y` never rises above the local surface on any of
  the ten tapes that cross it.
* **Tilting a deck under a run that works.** Rotating a tiled road makes a step:
  a 1.15° tilt of four tiles about a common axis stops the human's car dead
  100 m short of the kicker.

**What is left**: the last 17 m of the curl — aim at his state at canonical
x 933.3 rather than at the kicker plane; the post-fix re-run reaches one
checkpoint further than its template but is still 10.75 m short at ~50 m/s —
and the water start (1.30 s, never searched, independent of all of this).

**A WRITER DEFECT, found by tracing the one tape that looked like progress —
then isolated, and it was not the writer.**
Every file this arm wrote with `fk pol grid --outdir` FAILED TO REPRODUCE THE
EVALUATION THAT SELECTED IT: re-run the identical command on the written file and
a wall miss of 4.81 m becomes 66.36 m, a checkpoint approach of 1.88 m becomes
126.93 m. Traced tick by tick, those tapes never climb the wall curve at all —
they are below it and descending, and they respawn 53 m under the deck. The plain
oracle said `DNF cps 1` about all of them and was right; the in-memory numbers
never had standing. The sentence I nearly published — that a graft improves the
lap and dies downstream of the checkpoint — is withdrawn: it never reaches the
checkpoint.

**The cause was the edit window, not the writer and not chaos.** Two candidate
explanations were on the table: `--outdir` emits a different tape than it
evaluated, or two fork servers stop at different probe ticks and this obstacle is
chaotic across one tick 4 s upstream. Three tests killed both.

1. **The writer is byte-honest.** Drive the template through the whole factory
   with zero edits (`--v id=id`) and the output has the *same md5* as the
   template: `b01129775d1a5708b5fb525ff8acf9ff`. Not one bit differs.
2. **It is not the file.** The same spline measured in process and read back
   off disk: 4.81 m vs 66.36 m — but that is not chaos either, because
3. **It is the window's lower bound.** The fork resumes at probe tick 1941. A
   window of `1900:2320` writes 41 edited ticks *below the resume point* into
   the file, where they are simulated on replay and were never simulated during
   the search. Move the bound above the probe — `2000:2320` — and the same tape
   reproduces from disk **to the last printed digit** (20.06 m, 81.52 both
   ways). `1900:2320` is wrong by 46 m. One flag, one boundary, deterministic
   on either side.

`fk pol shoot2` now **refuses** the illegal window rather than producing a
plausible number, naming the two ticks. **This generalises to every search in
the fleet**: any run whose window starts below its fork's probe has banked tapes
that were scored on inputs they do not contain. In finish-time mode the phantom
guard catches it; in state-objective mode nothing does. (`24322ed6` checked
`tmsearch` on this warning and found it safe by construction — a fleet-max
resume floor — but the read-back check that warning prompted then caught a
*different* live defect in it, `retime` reaching past the window.)

**The re-run above the probe moves.** MEASURED, plain oracle, one invocation,
files on disk:

| tape | plain oracle |
|---|---|
| the human's own downloaded recording (positive control) | **440.238, full lap** |
| the template every search here edits | DNF, **cps 1** |
| the template through the writer, zero edits — the do-nothing tape | DNF, **cps 1** |
| the pre-fix winner, window starting 41 ticks below the probe | DNF, **cps 1** |
| `fx1`, post-fix, legal window 1950:2320, aim = the state 17 m up the curl | DNF, **cps 2** |
| `fx2`, post-fix, legal window 1950:2320, aim = wall + checkpoint | DNF, **cps 2** |

The do-nothing tape is printed first and on purpose: **the laziest way to
maximise this objective scores worse than both winners**, so it is a proxy and
not a decoy. The illegal-window winner scores exactly what doing nothing scores
— its whole apparent gain *was* the 46 m of disagreement. And the human's file
validating in the same invocation is the positive control that says the DNFs are
the tapes rather than the oracle.

`fx1` and `fx2` are the first tapes on this map whose in-process numbers and
their own read-back agree exactly. **They are not laps**: DNF at checkpoint 2 of
4, arriving 10.75 m and 15.33 m out at ~50 m/s. Nothing here is a time and
nothing is claimed.

One trap re-confirmed live while measuring them: `tmtas splits` prints
`race_time=440238` for all three of `fx1`, `fx2` and the do-nothing tape,
because a synthesised tape carries its **template's** telemetry and `splits`
reads the header. On this map that header is a real record *for this exact map*,
which makes it maximally seductive. Only `tmtas validate`'s `sim_time` and `cps`
columns are the simulator speaking.

Full account, tables and md5s: `tm-unbeaten/284238/RESULT.md`, which points at
`wtr_CORRECTION_v2_roll_is_reachable_and_it_is_not_enough.tsv` and
`wtr_RESULT_v1_the_cross_slope_couples_roll_to_the_line.md`.


## The record's time is mostly retries

On a Trial-family map the clock runs through respawns, so a recorded time is
clean driving plus every failed attempt. Take the human's own last, successful
attempt in each sector and his clean driving is **93.914**. Our 97.325 is that
driving with the retries cut.

## Files

| file | what |
|---|---|
| `replays/TAS_97325.Ghost.Gbx` | the best validated run — one life, no respawns |

Not in the repo (too large, and none of them is a lap): the two post-fix tapes
`wtr_fx1_…_DNF_cps2_state17m.Ghost.Gbx` and `wtr_fx2_…_DNF_cps2_wall_plus_cp.Ghost.Gbx`,
the byte-identical do-nothing tape, and the full tables live in
`tm-unbeaten/284238/` on the shared store — `RESULT.md` first, then
`wtr_legal_window_v4_the_first_tapes_that_survive_their_own_readback.tsv`.
32 artefacts, md5s in `wtr_MANIFEST.md5`, all verifying.
