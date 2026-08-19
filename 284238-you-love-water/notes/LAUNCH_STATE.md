# 284238 — the launch, measured per tick, against a human who drives it right

`state_RESULT_v1_launch_state.md`. Write-once sidecar; a correction gets a `_v2`.
Times in seconds. Everything here is measured on the **untouched map** with the
plain oracle as the outer control (`rank00001_440238` → **440.238**,
`best_97325` → **97.325**, `cold_279008_yhomas_46112` on its own map →
**46.112**, all exact, this session, this node).

**Every state number below comes from RE-SIMULATING the tape and reading the
engine's own vehicle struct** — never from a ghost file's telemetry. That is the
point of the reader fix (`FLEET_NOTICE_trajectory_reader_clock_first_v1.md`,
code in `state_fk_locate2_v1.tgz`), and it is what makes a synthesised tape
measurable at all.

---

## 0. The instrument, and its two controls

`fk btraj2` locates the car's state blind and extracts one row per tick;
`fk sweep` perturbs a tape in one window and measures a family of variants
against marks in **canonical module coordinates** (it undoes the −120° / −56 m
screw, so a copy-k measurement is directly comparable with copy 0 and with the
sibling map).

* **Known-answer control, our record:** whole 440.8 s trajectory versus the
  ghost's own telemetry — **median 0.0075 m, p90 0.033 m, 91.1 % of ticks within
  5 cm**; the residual is the 31 respawn transitions.
* **Known-answer control, map 2:** median 0.0068 m, p99 0.031 m, 99.34 % within
  5 cm.
* **Baseline agreement with the published table:** the re-simulated record
  crosses CP2 at **45.80 m/s**, the number in `RESULT-v2` §6a. The instrument is
  not inventing a different run.

## 1. The launch is a LATERAL AIM problem, and it is one number

Both launches in canonical coordinates, same instrument, same frame:

| | our record, cycle 1 | Yhomas 46.112 on the sibling map |
|---|---|---|
| lane, before the kick | v 97.3, **vz −2.3** | v 98.2, **vz −24.4** |
| kick exit | (926.9, 1881.9, 916.5) v 90.0, vy 53.5 | (936.9, 1889.3, 917.9) v 91.9, vy 55.3 |
| during the flight | vx ≈ 69, **vz +8.3** | vx ≈ 72, **vz −8.6** |
| **at the wall plane y = 1918** | **x 980.2, z 923.35, v 77.43** | **x 980.2, z 913.88, v 80.81** |
| what happens there | nothing — flies on | **catches the wall** |
| wall contact | (1015.2, 928.0), one-tick loss **8.71 m/s** | (980.1, 917.9, 913.9), loss **0.75** |
| checkpoint crossing | **45.80 m/s** | **69.40 m/s** |

Read the fourth row twice. **At the height where he catches the wall we are at
the same x (980.2) and nearly the same speed (77.4 against 80.8) — and 9.5 m out
in z.** That is the entire difference between the two launches, and everything
downstream of it (the 1630-versus-311 energy loss, the 24 m/s at the checkpoint,
which side of the 71 m gap the next cycle lands on) follows from those 9.5 m.

The cause is upstream and equally simple: **during the ballistic ride our lateral
velocity is +8.3 m/s and his is −8.6 m/s.** He crosses the lane yawed; our
record's driver never steers on the lane at all. Over the 0.9 s flight that
17 m/s difference is 15 m of z, and the wall curve rises with z, so we meet it
21 m higher and slam.

**Speed is not the variable.** Kicker crossings are 97.2 (ours, fails) against
99.1 (his, works) and 90.9 (our own standing start, works). This confirms the
withdrawal of the "boost pads are an overspeed trap" reading — the pads compress
the distance available to yaw, they do not make the car too fast.

## 2. Yawing the lane recovers the energy — and loses the redirect

`fk sweep --var steer` over the last 0.3–0.6 s of the lane, on the untouched
map, pads in place, measured at the wall plane:

| lane steer | y=1918 crossing (x, z, v) | one-tick loss | CP2 |
|---|---|---|---|
| 0 (the record) | 980.2, **923.35**, 77.43 | 8.71 | 45.80 |
| −15 | 969.1, 920.30, 76.79 | 2.68 | 43.98 |
| −25 | 958.4, 917.61, 75.92 | 1.12 | misses by 23.3 m |
| −35 | 948.3, 915.30, 75.23 | 1.19 | misses by 55.9 m |
| −45 | 935.5, **913.14**, 74.36 | 1.17 | misses by 116.5 m |
| −70 | 972.3, 895.03, 74.52 | 1.01 | misses by 83.6 m, **64.4 m/s** |
| **target (Yhomas)** | **980.2, 913.88, 80.81** | 0.75 | **69.40** |

Two things are now measured rather than argued:

1. **The energy loss is not a property of the map.** Yaw the exit of the lane and
   the 8.71 m/s slam disappears (1.1–1.2, i.e. no impact at all) and the car
   keeps up to 26 m/s more downstream. The wall is only expensive when it is met
   high.
2. **A lane-steer knob cannot reach the target state.** It rotates the flight
   about the kick, so z and x move together along a line of slope ≈ 0.23:
   getting z from 923.4 to 913.9 costs ~45 m of x, and the car reaches the wall
   plane 0.3 s early, 45 m short, and never gets the catch. The one-parameter
   family passes *through* the target's z and *through* the target's x, but not
   through both at once.

So the remaining problem is **not** "can our car be yawed" — the cold-start
agent proved it can, and their honest probe (a checkpoint-model gate at
Yhomas's own contact point, S-imaged into copy 1) fires for their steered tapes
at 22.842–22.898 against his 22.68. The remaining problem is that the state
which arrives there is not yet his state, and the catch is what converts it.

## 3. What the next lever is, precisely

The target is now a state, not a time, and it is dense: Yhomas's per-tick
canonical trajectory is banked (`cold_siblings/cold_answerkey_cycle1_canonical.tsv`,
and re-measured through our own reader in this session). The launch reduces to

> reach **y 1918 at x ≈ 980 with z ≈ 914 and v ≈ 80**, with the flight's lateral
> velocity **negative** (−8 to −9) rather than positive.

and the one-parameter lane sweep says a second degree of freedom is needed: the
yaw has to be put in **without** rotating the whole flight — i.e. a lateral
displacement on the lane (approach the kicker from ~9 m further out in +z and
yaw back), or a yaw applied and *released* before the kick so the kick's
direction is preserved while the body is turned.

That is a two-parameter search over (lane offset, yaw timing) with a scalar
objective — |z − 913.9| at the wall plane subject to x ≥ 975 and v ≥ 78 — and
every candidate is measurable per tick before it is ever validated.

## 4. Negative results from this session, with their enumeration

* **A constant steer held across the whole lane (ticks 2201–2341, race
  20.5–21.9) is fatal at every level tried** — 0, ±20, ±40, ±60, ±80, ±100,
  ±127: the car leaves the lane and no variant reaches the checkpoint. The lane
  is diagonal and the record's own steering is load-bearing; only a *delta* on
  it is a meaningful family.
* **A steer delta over the last 0.6 s (ticks 2281–2341)**, −10 … −80 and +10 …
  +40: the CP crossing speed never improves on 45.80. The best CP-collecting
  variant is −10 at 43.13; everything that reaches Yhomas's contact height at
  his z misses the gate by 55–143 m.
* Both families were swept at 10 ms resolution in the window position (five
  window starts × three ends) and at 5–10 units of steer; the negative is
  exhaustive in those two parameters and in nothing else.

## 5. THE YAW IS AVAILABLE ON OUR LANE — the water and the pads are not the barrier

The story I was handed was that 284238's six extra boost pads force ~97 m/s into
a catch that wants to be met low and yawed. That reading was already withdrawn
by its author on speed grounds (all three measurable launches cross the kicker
at 91–99 m/s). Here is the input-space half of the answer, measured on both maps
with the same instrument:

**Full lock held for the last N seconds of the lane, and what it buys in lateral
velocity at the lane mark:**

| held for | ours (water lane, 6 boost pads) | Yhomas's (tech lane, no pads) |
|---|---|---|
| his own inputs | — | vz **−23.95** |
| 0 (steer neutral) | vz **−2.34** | vz −1.47 (his lock removed over 0.7 s) |
| 0.20 s | −2.34 | — |
| 0.30 s | −4.29 | — |
| 0.40 s | −11.18 | — |
| 0.50 s | **−15.70** | −28.98 (against −15.77 with no steer: a **13.2 m/s** swing) |
| 0.70 s | car leaves the lane | −30.24 |

**Full lock over the last 0.5 s is worth 13.4 m/s of lateral velocity on our
water lane and 13.2 m/s on his tech lane.** The surfaces respond the same. Our
record's driver simply never steers there — with his lock removed over 0.7 s,
Yhomas's own lateral velocity collapses from −23.95 to **−1.47**, which is our
record's number exactly.

So "the human never carries the yaw into the jump" is a statement about the
driver, not about the map, and the yaw our launch needs is inside the input
space of our own geometry.

## 6. And yet one lane-yaw window still cannot reach the target state

With the yaw available, the obvious move is to use it — and the reachable set is
the obstacle. Every single-window family I swept (absolute steer and delta
steer; window starts 2251–2331, ends 2321–2356; magnitudes 10–127; plus a
gas-and-brake scrub) lands on the same one-parameter locus at the wall plane:

```
z(at y=1918) = 923.4 - 0.224 * (980.2 - x)      +- 1 m over 60 variants
target                980.2, 913.9        <- 9.5 m below the locus
```

Push harder and the locus does not bend, it **breaks**: 0.5 s of full lock gives
(982.7, 892.8) — x restored, z 21 m past the target, no catch at all, because
the car now misses the wall curve entirely instead of glancing it. The two
regimes are

* **the expensive catch** (ours): meet the wall high at z 928, lose 8.71 m/s in
  one tick, arrive at the checkpoint at 45.80 — and it IS a redirect: the slam
  is what turns the car toward the gate;
* **the cheap tangent** (his): meet it at z 914, lose 0.75, arrive at 69.40;
* and between them, **no catch at all**: the yaw variants that reach z ≈ 914 at
  x ≈ 938–952 fly past the wall and miss the gate by 55–143 m.

The gap between the first and the second is not energy and not speed. It is
**9.5 m of lateral position at a single height**, and no single-window input
change on the lane can produce it, because every knob that moves z also moves x
along that locus.

## 7. What I would do next, in order

1. **Two-window family: displace, then re-aim.** A yaw pulse followed by a
   counter-pulse translates the car across the lane without rotating the flight;
   that is the degree of freedom the one-window families are missing. Objective:
   |z − 913.9| at y = 1918 subject to x ≥ 975 and v ≥ 78, all measurable per tick
   before anything is validated.
2. **The lane ENTRY.** Our car lands into the lane out of the tube and is aligned
   by the landing; his enters it already crossing. The entry state is set at
   race 19.5–20.5 and is a legitimate search window that no one has touched.
3. **Ask copy 0 why it works.** Our one launch that clears the gap (299 and
   302 km/h) is the standing start, and copy 0's launcher is the START PLATFORM,
   not a water ramp — the same substitution the sibling map makes in all four
   copies. Its vz at the kicker is −17.9 and its contact is at z 915: it is
   already most of the way to Yhomas's launch. **The standing start is a worked
   example of the target state on our own map**, and matching it may be easier
   than matching his.
