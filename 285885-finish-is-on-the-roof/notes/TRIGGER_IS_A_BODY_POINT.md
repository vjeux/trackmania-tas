# 285885 — the finish tests a point on the car's ROOF, not its origin

**Sidecar v2. Supersedes `att_TRIGGER_IS_BODY_POINT_v1.md`** (same session, four
hours earlier). v1 §4 claimed the fast route "hits something, hard, at
(417.5, 1704.6)". **That was wrong** — an artefact of reading a gate-only
reconstruction past the point where a different face of the trigger volume
becomes the binding one; §5 corrects it from a live engine readout. v1's §3
located the binding point at the footprint's down-ramp corner; §3 below replaces
that with what the ruler actually says. v1 is kept, not deleted.

*Third agent on this map. All my files are `att_`-prefixed; I changed nothing
belonging to the previous two. Every number is from the plain oracle on the
untouched map, or from `tmmaps moveitem`/`ladder` position-only surgery whose
return-to-origin control reproduces 61229 / 88209 / 97769 to the millisecond.
~78 000 oracle runs. Rust only; no Python anywhere in this work.*

---

## 1. The correction, and the control that forces it — no model required

The model on the record — "fire iff `car_y ≤ gate_y + 1.25` inside a footprint"
— is **wrong**. It was fitted entirely on tapes that cross the patch **upside
down**, which is exactly why it looked consistent.

One ghost, one gate, two crossings of the same footprint, 20 mm of gate height
apart (201-rung y-ladder at the true x/z, `att_ycurve.log`):

| ghost | gate y | fires at | car ORIGIN y at the fire | body-up·world-up | attitude |
|---|---|---|---|---|---|
| rank 2 | **144.34** | 68.608 | **145.521** | −0.889 | upside down |
| rank 2 | **144.36** | 51.019 | **143.937** | +0.980 | upright |

The crossing that fires with its origin **1.585 m higher** is the inverted one.
That is a single tape against a single gate, so no model, no telemetry
interpolation and no surgery enters it: **the trigger cannot be a function of
the car's origin.**

What fits every rung on all three ghosts:

> **fire iff `y + h·u_y ≤ ceiling`**, where `u_y = 1 − 2(q_x² + q_z²)` is the
> world-vertical component of the car's **body-up** axis, and **h ≈ 0.84 m**.

The trigger tests a point **fixed in the car's body frame, ~0.84 m above the
origin** — the car's roof. Upright it rides 0.84 m above the origin; inverted it
hangs 0.84 m below it. **Turning the car over moves the tested point 1.7 m
down.** The old "+1.25 m ceiling" is this same surface read off inverted cars.

Least squares over 38 ladder transitions × 3 ghosts: `h = 0.83`, rms 0.24 m,
against **rms 0.48 m for the best origin-only model**. `h = 0.84` makes the two
rank-2 crossings above agree exactly.

**All three humans finish upside down. That is not a fumble — on this map it is
the only way in.**

## 2. The trigger volume, in 3-D

936 stations against rank 2's straight 150 km/h pass, plus 81 stations against
all three ghosts (`att_3d_r2.log`, `att_xz148.log`; 29 s of wall clock):

| axis | extent relative to the item origin | evidence |
|---|---|---|
| x | **[−5.5, ≈ +5.9]** | entry points; independently the fast tape's own entry at −5.44 |
| z | **[≈ −1.0, +1.61]** | 324 of 433 entry points sit exactly on `dz = +1.614` |
| y | floor ≈ −6.3; **ceiling NOT horizontal** (§3) | y-ladder; a gate at 152 stops firing |

An 11 m × 2.6 m slab, offset from the item, ~7 m deep.

## 3. The roof is a plane; the ceiling is not

**Roof.** 263 grounded human samples, x ∈ [411,424], z ∈ [1701,1708], one
per-orientation offset:

> **contact plane y = 410.5518 + 0.09211·x − 0.17895·z**, residual **rms 38 mm**, max 87 mm

— an **11.4°** plane descending toward (−x,+z), upright ride height **0.278 m**
above it (reproduces rank 2's telemetry to 5 mm). Sampled over the whole
40 × 40 m region the same plane holds to ±0.6 m. **It is one very large sloping
roof with nothing on it**: no lip, no edge, no bump, nothing to rotate off.

**Ceiling.** Feeding the fast route's y-ladder through its live trajectory
(`att_yfast.log` × `att_fastb.csv`) shows the trigger's top surface is *tilted*,
and more steeply than the roof:

| gate y | fires at | fire position | tested point `y + 0.84·u_y` | implied ceiling − gate_y |
|---|---|---|---|---|
| 144.30 | 41.037 | (413.59, 1705.17) | 144.594 | **0.294** |
| 144.12 | 41.039 | (413.69, 1705.14) | 144.609 | 0.489 |
| 144.10 | 41.044 | (413.94, 1705.06) | 144.645 | 0.545 |
| **144.08** | 41.069 | (415.18, 1704.68) | 144.829 | **0.749** |

Lower the gate and the fire moves **later and further up-ramp** — impossible for
a horizontal ceiling. So there is no exploitable "down-ramp corner": the
clearance is a function of position with an **interior minimum**, and on this
route it sits at **x ≈ 415.2, z ≈ 1704.7**, which is exactly where the tape
fires its lowest rung.

**The deficit there is 70 mm** — the tape fires a gate at 144.070 and refuses
144.069, against a real gate at 144.000. Everything else about the map is
downstream of that number.

## 4. What the map requires: ~26° of body tilt

`u_y ≤ 0.895` at the binding point, against the **0.978** the ramp itself
imposes (which is just the plane's own 11.4°). Three sources, all measured:

**Suspension — dead, quantitatively.** Regressing the origin's height above the
plane on the damper channel gives 0.278 − 0.301·dampen, ~59 mm across the
damper's whole range; but the resting value is 2/255 and **no grounded sample in
any of the three human runs is ever more than 2/255 below rest**, i.e. **≤ 5 mm
of compression is ever available**. The "suspension dip" hypothesis is now
closed by measurement rather than by argument.

**Air control — dead.** The car is not airborne anywhere near the finish (§5),
and accel/brake overrides across the earlier flight move the ladder by nothing.

**Rotation off a feature — the only live one.** The two humans who finish
"early" both flip at the same place, **(295, 122, 1772)** — a steep narrow climb
that pitches the car 45° and throws it into a continuous 1.7 rad/s barrel roll
— at 39.5 s (rank 1) and 41.3 s (rank 3), and then need **10.6 s** for the last
130 m on the roof at 20–45 km/h. That is the whole gap between the TAS arrival
at the patch (41.0 s) and the best human finish (50.2 s). Rank 2 instead flips
on a **wall at (405, 149, 1666)** hit at 270 km/h, which puts it on its roof in
0.6 s — but that wall is 39 m up-ramp of the patch and costs all the speed.

> **So the map is: acquire ≥26° of tilt within the last ~120 m for less than the
> ~2 s the author time allows.** Nothing on that stretch supplies it. The author
> had something; it is not on the route as currently driven.

## 5. The fast route, measured live — no crash, no flight, no room

`fk btraj` (reference-free; template = the tape itself — one line added to
`state.rs` so `--allow-dnf` parses) on `bis_418.6138_best`. Self-check clean:
|q|−1 max 1.6e−7, |d(pos)/dt − v| mean 0.6 m/s, 0 clock gaps. **This is the
first honest telemetry that tape has ever had** (`att_fastb.csv`, 100 Hz).

```
t_ms         x        y        z    km/h    v_y      u_y   y − roof plane
40950   409.31  143.148  1706.48   186.0   7.13   0.9754     0.269
41030   413.25  143.722  1705.27   187.9   7.25   0.9782     0.264
41069      -- fires the 144.08 gate at (415.18, 1704.68) --
41100   416.72  144.235  1704.21   189.6   7.48   0.9809     0.267
41400   432.08  146.452  1699.95   196.6   7.67   0.9785     0.307
```

From 38.5 s to beyond 41.4 s the car is **glued to the plane at nominal ride
height** (0.26–0.31 m), **upright** — `u_y` 0.974–0.985 is the plane's own tilt
and nothing else — accelerating 138 → 197 km/h up 120 m of ramp. It does not
jump, does not land, does not hit anything. **Its attitude at the finish IS the
ramp's attitude; there is no other term in it.** (It runs out of roof much
later: a wall at (507, 159, 1660) at 43.0 s at 210 km/h, then falls off.)

**Why v1 said "crash":** the gate reconstruction reads `x(t) = station_x − 5.588`
off the fire time, valid only while the `−x` face is binding; past 41.10 the
`−z` face takes over and the apparent x-velocity collapses from 50 m/s to
11 m/s. **A gate reconstruction must be checked for which face is binding before
it is read as a trajectory.**

## 6. The one real gradient — and why it is too late

Score the phase **after** the binding point by moving the ruler up-ramp (gate at
x = 423, so its footprint is entered only at x ≥ 417.4), under a time budget so
a late fire does not count:

| tape | lowest firing rung with t ≤ 43.079 |
|---|---|
| `bis_418.6138_best` (identity) | 144.90 |
| + full-lock steer, 30 ticks from slot 4230 | **144.50** at 41.114 |
| after two hill-climb rounds | **144.45** at 41.154 |

**Hard steering is worth 0.40–0.45 m of tested height** — six times the 70 mm
needed. Confirmed on the live readout: the poked tape is 46 mm lower at 41.00,
68 mm at 41.05, 93 mm at 41.10. It is simply not there yet at the binding point:
the same input scored on the real gate's ladder is 144.16, *worse* than the
identity's 144.08. **The lever exists; it develops ~0.3 s after the input and the
finish arrives 0.25 s in.**

## 7. Negatives, with their enumerations

All against `bis_418.6138_best`, scored on a ladder wide enough that every
candidate gets a number (143.85 → 146.50) — the previous 11 × 5 mm ladder was
55 mm long, so anything worse than that read as "no fire", which is exactly how
a landscape with a wall in it looks flat.

| perturbation | candidates × rungs | best |
|---|---|---|
| accel/brake over the earlier flight, slots 4150–4270 | 221 × 6 | no gain |
| steering flicks ±48…±127, 2–12 ticks, slots 4180–4260 | 271 × 8 | no gain |
| full lock ±64…±127, 20–80 ticks, slots 4140–4245 | 421 × 10 | no gain |
| brake / lift, 5–55 ticks, slots 4160–4252 | 321 × 10 | no gain |
| negative-lock sweep, 5 mm rungs 143.90–144.10 | 401 × 8 | no gain |
| **lane-change pulse PAIRS** (steer out, then back — the only shape that shifts the line without turning the car), 6 starts × 4 widths × 3 gaps × 4 magnitudes × 3 cancel ratios | **865 × 8** | **no gain** |
| line shifts, steer ±4…±16, slots 3800–4200 | 127 × 17 | worse or DNF |
| two tail hill-climbs past the binding point | ~50 000 | 144.45 up-ramp, nothing at the finish |

The 144.070 threshold has now survived ~1.6 M evaluations from two agents and
~78 000 from me, including every input channel at full authority and the one
perturbation *shape* nobody had tried. **Treat it as a proven bound on this
line, not a stubborn number.**

## 8. For whoever picks this up

1. **Stop trying to arrive lower. Arrive TILTED.** The quantity to maximise is
   the car's tilt at the binding point, and the shaping instrument is the
   up-ramp ladder of §6 under a time budget.
2. **The rotation source is the whole problem.** It has to be within the last
   ~120 m and cost under ~2 s. The roof there is a bare plane, so it must come
   from *off* the plane: an approach that arrives from a higher structure
   already rotating, or a feature not on the current route. The author's 43.079
   is about 2 s over the fastest known arrival — one flip's worth.
3. **Do not reuse the old trigger model.** Anything computed from
   `car_y ≤ gate_y + 1.25` on an upright tape is out by up to 1.7 m.
4. Banked and working: the **body-point ladder** (a y-sweep at fixed x/z
   measures `y + 0.84·u_y`, not `y`), the **gate-as-trajectory reconstructor**
   (§5, with the binding-face caveat), and **`fk btraj` on this map** with the
   one-line `--allow-dnf` patch — its self-check passes and its output matches a
   human ghost's own decoded telemetry exactly.

## 9. Files

`att_TRIGGER_IS_BODY_POINT_v2.md` (this) · `att_TRIGGER_IS_BODY_POINT_v1.md`
(superseded; §3–4 wrong) · `att_ycurve.log` (201-rung y ladder, 3 ghosts) ·
`att_yfast.log` (71-rung ladder, fast route) · `att_3d_r2.log` + `att_3d_r2.off`
(3-D volume sweep and its entry points) · `att_xz148.log` · `att_surf.txt` (263
surface samples) · `att_C.log` (x/y profile) · **`att_fastb.csv`** (the fast
route's live 100 Hz trajectory) · `att_d1b.csv` (the same for the best
steering-poked tape) · `att_tools.tgz` (all Rust sources: `att.rs` —
override/lane sweeps and ladder ranking — plus `fit`, `plane`, `ceil`, `surf2`,
`offs`, `flight`, `susp`, `thr`, `gridgen`, `pick`, and the climb driver).
