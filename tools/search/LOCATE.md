# LOCATE.md — the car, by derivation

Where the fork server's car is, why that is the right object, what every other
copy of it in memory is, and what it costs to know. Server build 128182
(`TrackmaniaServer`, md5 `0f0f4b25f31f80c60c81404366c95e68`,
`date=2026-05-15_18_00 git=128182-0de74ece09e`). Code: `forkoracle::car`
(the derivation), `fk locate` (the controls). Written 2026-09-07 on
`agentcloud/locate`.

vjeux, 2026-09-06: *"This is the most important part of the project and taking
3s is just unacceptable."* And an hour later: *"It is likely that the existing
locate is wrong as well. Please do the work of figuring out what the actual
right object in memory is. Everyone has been using heuristics so far which have
all proven very brittle."* Both were right. The sweep took 3.6 s per worker
and was one tick wrong on two of the four maps below; the "validator chain" it
was checked against was reading a copy without knowing it.

---

## 1. The answer

**The state the physics step integrates is an 88-byte dyna body record**, and
the `CGameVehiclePhy` the validator owns holds a per-tick **copy** of it.

Per tick, inside the playground update `0x119f1b0(playground, new_time, dt)`
that the validator's tick loop calls once per 10 ms (TICKHOOK.md §2):

| order | site | what it does to the car |
|---|---|---|
| 1 | `0x11a14da: call 0xa53ce0(vehmgr)` | **the vehicle solver.** Integrates the body record: position written at `0xa549f7` (predict) and `0xa54ba4` (correct), velocity at `0xa54c1e`. Before it, the pre-step vis state `phy+0x4e8` is refreshed from the record (copy `0x1ae9636`, quantise to 1 mm `0x9cf60f`) |
| 2 | `0x11a14fe: call 0x933a20(dynamgr, params, dt)` | **the dyna world update** (the engine's own profiler names the boundary `PhysicsStep_BeforeMgrDynaUpdate`). Resolves the bodies against the world; on the ground it writes the record again (`0x935e82` on map 2; `0x93dd41`/`0x94215c` on the maps with other dyna bodies) |
| 3 | `0x11a17ac: call 0x9cd8c0(vehmgr)` | **the copy-out.** For each vehicle the loop does not skip and whose body handle is not `0xffffffff`, look the record up and copy quaternion, position, velocity and angular velocity into the phy (`0x9cdbe4..0x9cdc1e`, the position at `0x9cdbfb`). Then refresh the post-step vis state `phy+0x848` from it (same copy + quantise) |

Nothing else writes the car's position. Measured with hardware watchpoints on
four maps (`fk locate watch`, §6.4): every write of the body's position sits in
frame 1 or 2, the copy-out writes the phy exactly once per tick after the last
of them, and the vis states are written exactly where the table says.

So the **copy-out at `phy+0x12e0..0x1314`** — quaternion (w,x,y,z), position,
velocity, angular velocity — is the physics state at every tick boundary, bit
for bit (§6.2: 100 % of ticks with a body, five maps), and the record it is
copied from is THE object. The locator resolves both.

### The record

```text
body = [dyna+0x70] + 0x58 * u32[[dyna+0xb0] + 4*handle]      (0x934980, the lookup)
  +0x00  3x3 rotation matrix (9 f32)
  +0x24  position x y z
  +0x30  quaternion w x y z
  +0x40  linear velocity
  +0x4c  angular velocity                                  88 bytes
handle = u32[phy+0x10]        0xffffffff = this vehicle has no body right now
```

`phy+0x10` is the word the earlier "live slot" rule keyed on without knowing
what it was: it is the **dyna body handle**. On map 2 and the tiny map it is 0;
on 126859 it is 6 and on 145875 it is 16 — those maps hold other dyna bodies,
which is exactly why the remap table exists and why a locator must go through
it rather than assume the car is body 0.

---

## 2. The path

Every hop is one the engine itself takes, with the instruction it takes it at:

```text
controller (rdi at the validation callback 0x118c170)
  +0x1a70 -> sim                          == the rcx captured at that callback
sim   +0x48  = the tick loop's clock      0x1219750: mov [sim+0x48], new_time
sim   +0x18  -> playground                0x1218e3d
playground +0x660 -> players, +0x668 == 1 -> participant     0x1218e41 / 0x119f5a5
playground +0x7c8 -> scene                0x119f1fa: mov r15,[rdi+0x7c8]
vehmgr = [scene + 0x10 + 8 * u32[module + 0x1cc18b0]]
                                          0x9c3714 (the registration index, a global)
                                          0xa47c96: mov rax,[rdi+rax*8+0x10]
vehmgr +0xb0/+0xb8 = the vehicle array the copy-out iterates (0x9cd8e1/0x9cd8e8)
vehmgr +0x110 -> dyna                     0x9cdbd5
participant +0x1110 == 0x032e2000 (CGameVehiclePhy), +0x1118/+0x1128/+0x1138/+0x1148 -> the four slots
  DRIVEN = the slot the copy-out loop does not skip:
           u32[phy+0x128c] & 0xf != 2  (0x9cdaf1)  and  u32[phy+0x1c90] in {0, 3}  (0x9cdb00)
           it has a body (u32[phy+0x10] != 0xffffffff) except inside a respawn window
body = the record above;  copy-out = phy+0x12e0 (q) +0x12f0 (pos) +0x12fc (vel) +0x1308 (angvel)
```

Two cross-checks ride along at no cost: the driven phy must be one of the
vehicles `vehmgr` iterates (so the global index really named the vehicle
manager), and the record must be byte-identical to the copy-out (so the lookup
named the body this phy was copied from, one tick ago). Every hop is checked as
it is taken and every failure names its hop. There is no fallback to a search.

`forkoracle::car::locate` does this in **thirty-odd reads of `/proc/<pid>/mem`
of the stopped parent**: 132 µs median over 1000 repeats (first call 180 µs).
The sweep it replaces took 3.1 s after the boundary probe, so a fork worker is
ready in **2.25 s instead of 5.4 s** (§7) — what remains is the engine boot.

---

## 3. The label

The fork server stops in the tick hook, at the START of the tick whose
`new_time` is `sim_ms`. Memory then holds the state at the END of the previous
tick, the one the loop stamped `[sim+0x48] == sim_ms − 10` (the shim checks
that identity on every tick; 0 mismatches ever). Its race time is
`[sim+0x48] − race_start`, and `race_start` is what the shim already reads out
of the engine where the loop reads it (TICKHOOK.md §3).

So the `Layout` is: clock word `sim+0x48`, bias `race_start`, position/
quaternion/velocity the copy-out, wetness `WetnessValue01` of the post-step vis
state (`phy+0x848+0x328`, the engine's own name for it). **Nothing is measured
and nothing is fitted.** `measured_clock_bias`, the `−10`/`−20` conventions, and
the round-time counter the sweep used to hunt for are gone: the bias belongs to
the object, and the object is now known.

Against the ghosts' own telemetry, at the telemetry's own 50 ms instants (no
interpolation; `tmtraj csvdiff --tol-ms 0`):

| map | ghost | shared instants | median | p95 | max |
|---|---|---|---|---|---|
| Summer 2026 - 02 (map 2) | rank00001 22.730 | 452 | 0.000 m | 0.010 | 0.010 |
| | rank00010 22.798 | 453 | 0.000 | 0.010 | 0.010 |
| | rank00100 22.884 | 423 | 0.000 | 0.010 | 0.014 |
| Kacky Reloaded #290 (126859) | rank01 24.342 | 278 | 0.000 | 0.010 | 0.010 |
| | rank02 24.634 | 283 | 0.000 | 0.010 | 0.010 |
| | rank03 25.379 | 300 | 0.000 | 0.010 | 0.014 |
| 145875 | r01 6.346 | 97 | 0.000 | 0.001 | 0.010 |
| | r02 6.350 | 98 | 0.000 | 0.001 | 0.010 |
| | r03 6.360 | 98 | 0.000 | 0.001 | 0.001 |
| YOU LOVE WATER (284238), 31 respawns | rank00001 440.238 | 8635 | 0.000 | 13.3 * | 54.5 * |

\* the respawn windows, §5. Outside them the residual is the telemetry's own
1 mm quantisation, on every map, on every ghost. The `fk trace` control against
a 1 ms interpolation of the same telemetry reads median 3.1 mm / max 6.8 mm on
map 2 and median 9 mm on 126859 with a tail to 0.87 m — that tail is the chord
error of interpolating a 50 ms telemetry at 118 m/s, not the readout, which is
why the table above is measured at the instants themselves.

The Summer 2026 - 01 tiny map (items, not blocks) has no ghost with telemetry
— its containers are synthesised — so its rows are the engine-internal
controls of §6 only. The derivation is the same code path and every control
passes there.

---

## 4. Every copy of the car in memory, and its phase

`fk locate census` scans the stopped parent for every float triple within 3 m
of the car and, for each, gathers it beside the copy-out over 64 ticks and asks
at which lag `copy[t] == out[t−k]` bit for bit (or, for a quantised copy,
within its quantum). The same picture on all four maps:

| where | phase vs the copy-out | what it is |
|---|---|---|
| `body+0x24` | lag 0, bit-identical 60/60 | **the dyna body record** (§1) |
| `phy+0x12f0` | lag 0 | **the copy-out** (§1) — the object the sampler reads |
| `phy+0x38` | lag 0, within 1 mm (bit-identical on 126859/145875, 45/60 on map 2 and the tiny map) | the vehicle's current `Iso4` translation |
| `phy+0x848+0x50` | lag 0, quantised to 1 mm | the post-step `CSceneVehicleVisState` (wheels, gear, rpm, wetness live here; `WHEELS.md`) |
| `phy+0x4e8+0x50` | lag **+1**, quantised | the pre-step vis state: refreshed before the solver, so it holds the previous tick. A render copy — and 1–2 cm off on three ticks of 2261 on map 2 (none of them checkpoint ticks; most likely the validator's frame boundaries re-extracting it) |
| `phy+0x27c+0x24` | lag **+1**, bit-identical | the previous tick's `Iso4` (`0x1ae9610` builds it from the previous quaternion and position before the solver) |
| `participant+0xe24` | lag **+1**, bit-identical | q/pos/vel of the previous tick, in the participant. **This is what the blind sweep picked on map 2** (7916 bytes past the round counter — the "P−7916" of the first forkstate notes) |
| one unnamed heap object | lag **+1**, bit-identical | present on all four maps |
| two unnamed heap objects | lag 0, bit-identical | present on 126859 and 145875 only |
| wheel/contact points, a frozen trail on the tiny map | not the car | within the radius, never the car |

Five objects carry the sweep's whole signature — a unit quaternion 16 bytes
before a moving position with its derivative 12 bytes after — at two different
phases, and which of them fall inside a 64 KB window varies by map. The sweep
ranked them by self-consistency and took a lag-1 copy on map 2 (and labelled it
with its lag-1 formula: right by luck), a lag-0 copy on 126859 and 145875 (and
labelled it with the same formula: **one tick wrong**), and on the tiny map an
object with no attitude at all (self-check failure, `|q|−1 = 2e-2`). **The
"phase that is not map-invariant" of TICKHOOK.md §11 was never a property of
the map. It was the property of whichever copy had won the ranking.**

---

## 5. Respawns

A respawn REMOVES the body: `phy+0x10` goes to `0xffffffff` (written at
`0xe873b1`) and comes back 101 ticks later (`0x9c9ba9`) with the same handle,
the same remap entry and the same record address — 31 respawns on the 440 s
ghost of 284238 under gdb, zero moves. Inside the window the copy-out loop skips
the vehicle (40 969 copy-outs over 44 201 ticks), the vis states freeze at the
crash position, and the phy already holds the checkpoint's saved pose;
`phy+0x12dc` holds the simulation time the body comes back at.

Two consequences:

* the locator names the car with or without a body (a stop inside a window
  resolves; `body` is `None`), and the **sampler reads the copy-out**, which is
  defined at every tick and bit-identical to the record at every tick boundary
  where a body exists;
* during a window the ghost telemetry is the game's fly-back animation — it
  moves linearly from the crash point to the checkpoint over the second — and
  disagrees with the copy-out by up to 54 m (§3's asterisks; the residual
  shrinks 54 → 0 m across exactly the 11.05–12.05 s window the gdb log names).
  There is no physics state during a respawn; the copy-out holds the pose the
  physics resumes from.

The parked slots on a single-car map read `u32[phy+0x128c] & 0xf == 2` and
`phy+0x12dc == −1`, and sit at the spawn.

---

### Before the spawn there is no car

The four vehicle objects are CREATED two ticks before the race starts (map 2:
`vehmgr` holds 0 vehicles and the participant's slots are null until race
−20 ms; at −20 ms the four appear with `+0x128c = 1`, and the driven one turns
0 at the race start). A checkpoint in the countdown therefore has nothing to
derive from — and nothing to locate either; the sweep only appeared to manage
it because its probe children ran past the spawn and searched for an object
that had come into existence in the meantime. `fk trace` and `fk regen` now
say so and move to the first checkpoint that has the car (race +0.2 s for
trace, the next rung of regen's own ladder — race 0 — for regen); a checkpoint
before the car exists is the ONE case where the checkpoint is not the user's
to choose, and the tool prints why.

---

## 6. The controls — each one a test a wrong answer would fail

### 6.1 `fk locate` — derive and time it
Prints every hop, the record, the copy-out, the clock word and its label, and
the derivation time: **132 µs median, 146 µs p90, 184 µs max** over 1000
repeats (map 2, tick 1200).

### 6.2 `fk locate check` — the identities, tick by tick, over a whole run
Gathers clock, handle, body record, copy-out and both vis states per tick.

| map | ticks with a body | body == copy-out (q, pos, vel), bit for bit | post-step vis = copy-out to 1 mm (worst) | clock steps by 10 | unit q |
|---|---|---|---|---|---|
| map 2, from tick 171 | 2261 | **2261 / 2261** | 0.0009 m | 2261 rows, 0 gaps | 0 off |
| 126859, from tick 300 | 2338 | **2338 / 2338** | 0.0009 | 0 gaps | 0 off |
| 145875, from tick 200 | 589 | **589 / 589** | 0.0008 | 0 gaps | 0 off |
| tiny, from tick 300 | 2877 | **2877 / 2877** | 0.0009 | 0 gaps | 0 off |
| 284238, tick 1000 + 3000 | 2901 (+101 in a respawn window) | **2901 / 2901** | 0.0008 | 0 gaps | 0 off |

### 6.3 `fk locate mirror` — which object answers the steering, and when
Three forks from one stop: the tape, hard left, hard right, for five ticks.
Sample 0 is the state after the first steered tick (the child resumes into it).

| object | left and right first differ at | map 2 · 126859 · 145875 · tiny |
|---|---|---|
| copy-out (q/pos/vel) | **sample 0** | all four |
| dyna body record (pos/q/vel) | **sample 0** | all four |
| post-step vis state (rot/pos/vel) | sample 0 | all four |
| pre-step vis state | sample 1 | all four |
| participant copy `+0xe14` | sample 1 | all four |

Yaw after 7 ticks relative to the unsteered run: map 2 `+0.135° / −0.300°`,
126859 `+0.316° / −0.637°`, 145875 `+0.075° / −0.003°`, tiny `+0.377° /
−0.377°` — antisymmetric on all four. The copies answer a tick late, which is
the mechanism behind every "answers at N+2" observation in the earlier notes.

### 6.4 `fk locate watch` — who writes, in what order (gdb, no shim)
Hardware watchpoints on the record's position, the copy-out's position and both
vis states' positions for two ticks after tick 500, with the backtrace of each
write. On all four maps, every tick:

```text
vis_pre  x2  (0x1ae9636 copy, 0x9cf60f quantise)
body     x4..x8 -- every one inside the vehicle solver call (return 0x11a14df)
                    or the dyna world update (return 0x11a1503)
copy-out x1  at 0x9cdbfb, inside 0x9cd8c0 (return 0x11a17b1), after the last body write
vis_post x2  (0x1ae9636, 0x9cf60f)
```

map 2: body ×6 at `0x935e82/0xa549f7/0xa54ba4`; 126859: ×6 at
`0x93dd41/0x94215c/0xa549f7/0xa54ba4`; 145875: ×8 at the same four; tiny: ×4 at
`0xa549f7/0xa54ba4` only (no other dyna body). This is the write set of the
physics step, and the copy-out is the only thing that follows it.

### 6.5 The watchdog's verdicts, old locator vs new (`fk watch measure`, 50 candidates, seed 1)

| map | locator | trips | breakdown | false positives | non-trippers identical armed/unarmed | score safety |
|---|---|---|---|---|---|---|
| map 2, tick 171 | sweep | 32 / 50 | crash 16, off 11, stuck 5 | 0 | 18 / 18 | 50 / 50 |
| | **derived** | **32 / 50** | **crash 16, off 11, stuck 5** | **0** | **18 / 18** | **50 / 50** |
| 145875, tick 200 | sweep | 24 / 50 | crash 11, off 13 | 0 | 26 / 26 | 50 / 50 |
| | **derived** | **24 / 50** | **crash 11, off 13** | **0** | **26 / 26** | **50 / 50** |
| 126859, tick 600 | sweep | — | *cannot locate: `vel_err 1356 m/s, refusing to guess`* | | | |
| | **derived** | 35 / 50 | off 35 | **0** | 15 / 15 | 50 / 50 |

Identical verdicts, identical per-predicate breakdowns, identical progress
figures on the two maps where the sweep could run — the precondition the
`FK_FAST_LOCATE` attempt failed. On 126859 the sweep never ran. (With
`crash:speeddrop` armed there, the reference tape itself trips at 21.5 s: the
car goes 188.9 → 62.5 m/s into a wall and free-falls to the finish. The ghost's
own telemetry shows the same numbers to 0.1 m/s. A predicate fact about a
Kacky finish, not a locator fact; the row above is measured without it.)

---

### 6.6 The guarded 10-minute stress search
`tmsearch search --fork --forktick 171` on map 2, 24 workers, seed 42, phantom
guard on, the boundary-stress window (`--lo 171 --window 60 --stride 400`) —
the same run TICKHOOK.md §5 tabulates for every build of the clock:

| build | evals | eval/s | banked | phantoms | best |
|---|---|---|---|---|---|
| tickhook, past-the-end guard fixed (sweep locator) | 275 400 | 458 | 8 | 0 | 22.712 |
| tickhook, finish word without calibration fork (sweep locator) | 282 630 | 470 | 10 | 0 | 22.711 |
| **this branch: the derived car** | **280 050** | **465** | **8** | **0** | **22.713** |

Every banked improvement was re-validated by the plain oracle; nothing was
refused as a phantom. The search does not know the locator changed.

---

## 7. Cost

| | before | after |
|---|---|---|
| locate, `fk` (after the boundary probe, map 2) | 3.1–3.2 s (sweep) | **0.004 s** (derivation 0.13 ms + prints) |
| fork worker ready (`fk watch measure` startup, map 2, 3 runs) | 5.37–5.43 s | **2.18–2.25 s** (= the engine boot) |
| `fk trace` locate line (5 maps) | 1.4–2.4 s, or a refusal | 0.0002 s |
| maps the locator works on | 2 of 4 (one tick wrong on those 2 of 4 it "worked" on…) | 4 of 4, plus the respawning one |

The evaluator's per-candidate cost is unchanged (`fk watch measure` throughput
within noise: 55.8 vs 56.7 ms/cand armed on map 2), as it should be — nothing
about a candidate's run changed, only where the driver looks.

---

## 8. What each quantity is derived from (vjeux: "we shouldn't have to do 51 fork boundary calibrations or any of this kind of things")

| quantity | derived from | verified by (a CONTROL, not the mechanism) | cost per worker |
|---|---|---|---|
| **car state** (q, pos, vel, angvel) | the pointer path of §2 to the copy-out of the dyna body the solver integrates | `fk locate check` (body == copy-out every tick, 5 maps), `mirror`, `watch`, `census`, telemetry at its own instants (§3) | ~30 reads, 0.13 ms, no fork |
| **tick index** | the engine's tick function `0x119e060`, hooked; `new_time` is its argument (TICKHOOK.md) | `fk tickhook count --gdb`: hook entries == loop clock writes == ticks | 0 |
| **race start** | read where the loop reads it: `max([[playground+0x968]+0x3c], [participant+0x218])` | it is the engine's own value; 300 servers agree on race tick and probe (TICKHOOK.md §5) | 0 |
| **clock word / label** | `sim+0x48`, the loop's own clock write; label = word − race start | `fk tickhook reads`: `[sim+0x48] + 10 == sim_ms` at every stop; the telemetry table of §3 | 0 (was a scan + a fitted or measured bias) |
| **input boundary** | record index `(new_time − race_start − start_offset)/10` from the tick (TICKHOOK.md §2) | the page-fault probe, kept as a cross-check at `boundary_tick`; `fk server check --calibrate` (the 51 forks) is a control run on demand, not on the path | probe: one mprotect fork (tickhook arm's) |
| **finish time** | `[[controller+0x1a88]+0xa4]`, sim ms, sentinel `0xffffffff` demanded | `fk tickhook finishcheck` 1100 candidates / 0 disagreements | 0 where the sentinel is structural (map 2, 145875); one calibration fork where it is not (126859) — the remaining measured piece, tickhook arm's |
| **checkpoint count** | `participant+0xc70` — a located word (behavioural, tm-player ENV arm), not yet a disassembled path | 200/200 vs the plain oracle, 1288/1289 over 2453 tapes | 0 |
| **tape exhausted** | arithmetic on the tick index (`last_tape_clock`, tickhook `b8d0fa6`) | — (it is the driver's own knowledge) | 0 |
| **wetness / wheels / gear / rpm** | `phy+0x848` = the post-step `CSceneVehicleVisState`, fields by the engine's reflection (`VEHICLEVISSTATE.md`) | `fk wheels` byte-for-byte vs ghost samples (tm-player), `fk liveness` | 0 |

The two rows that are still located rather than derived — the checkpoint
counter and the finish word on maps without the structural sentinel — belong to
the tickhook arm; both are verified against the plain oracle rather than
trusted.

---

## 9. What the old locators were, and what they got wrong

| | what it did | where it was wrong |
|---|---|---|
| `forkoracle::blind::locate_blind` (the sweep) | forked once per 64 KB window (~37 forks, 3.6 s), kept the most self-consistent moving triple with a unit quaternion at −16 | picked `participant+0xe24` (lag 1) on map 2, a lag-0 copy on 126859/145875 (**one tick wrong** with its lag-1 label: median 0.85–0.97 m vs the telemetry, 0 % within 5 cm, self-abort), a non-attitude on the tiny map; refused 126859 outright with the reference-line box |
| `forkoracle::car::locate_fast` / `FK_FAST_LOCATE` | the validator chain's position as a needle, then chains and a scan, then a tracking test | the tracking test could not choose between the lag-0 and lag-1 copies, because nothing about the copies says which is the state |
| `CAR_CHAINS` / `fk ptr` chains | pointer chains derived by a backward scan to the vis state at `phy+0x4e8` | the vis state is a 1 mm-quantised **render copy one tick behind**, with a matrix where the sampler expected a quaternion; the roots were sometimes stack frames |
| `fk::locate::find_clock2` / `layout::find_clock` | scan for a u32 stepping by 10 near the car | found the round counter (`sim+0x40`) and labelled it with a bias measured against an assumed phase (`measured_clock_bias`: −20 for "the vis state") |
| `fk::validator::ValidatorCar` | the validator chain to `phy+0x12f0` | right object, wrong label (the −20 bias of a lag-1 copy applied to a lag-0 one), and a behavioural `qualify2` that could refuse a real car at a slow checkpoint |

All of them are deleted. `Layout` carries explicit `quat`/`vel`/`wet`
addresses now, so no consumer assumes the `q@−16` shape, and `segments()`
gathers the copy-out in the record order every consumer already decodes.

---

## 10. Re-deriving on a new build

Every constant lives in `forkoracle::car::build128182`. On a new binary:

1. **The tick loop and the playground update** come from `fk tickhook find`
   (TICKHOOK.md §7): the input-record reader `0x119f0f0` by its shape, the tick
   loop as its caller, the tick function and the playground update as the
   calls in the loop body.
2. **The copy-out** is the callee of the playground update that contains the
   lookup shape `mov eax,[rax+rcx*4]; imul rax,rax,0x58; add rax,[rdi+0x70]`
   (the body lookup, `0x934980`) followed by `movups` from `[rax+0x30]` and
   `mov [r13+0x12f0]` — the offsets `0x10` (handle), `0x128c`/`0x1c90` (the
   skip tests), `0x12e0..0x1310` (the destinations), `0x110` (the dyna world)
   and `0xb0/0xb8` (the vehicle array) are read straight off it.
3. **The vehicle manager index** is the global read by the small getter the
   playground update calls right before `a47c90(scene, id)`
   (`mov eax,[rip+X]; ret`); `a47c90` is `[scene + 0x10 + 8*id]`.
4. **The solver** is the callee whose frame owns the writes to the record —
   `fk locate watch` prints the return addresses; put them in `SOLVER_RET`
   and `DYNA_RET`.
5. Then run, in this order: `fk locate` (resolves, or names the broken hop),
   `fk locate check` on one whole run, `fk locate census` (every copy named,
   phases as in §4), `fk locate mirror`, `fk locate watch`, `fk trace
   --reference` against a human ghost and `tmtraj csvdiff --tol-ms 0`
   (median 0.000 m or the label is wrong), `fk tickhook reads`.

A derivation that fails names its hop; a derivation that resolves the wrong
object fails `check` on the first tick the car moves. Neither is a guess.

---

## 11. Evidence

`~/persistent/private-30d/tm-locate/` holds the raw output of every run quoted
here (`proof/`), the gdb scripts and logs of the first write-watch experiments
(`exp/`), the disassembly listing of the server (`ts.asm`, 5.5 M lines), and
the old-tree binaries' outputs for the comparisons.

---

## 12. The suites, and what was deleted

`tools/search`: **163 tests listed, all green** (151 before: the sweep's own
tests left with it, 12 tests of the derivation came in — every hop, both slot
rules, the respawn window, the copy-out cross-check, the sentinel). `tools/fk`
with the engine tier strict: **34 / 34** (38 before: the four unit tests of
`ValidatorCar` went with the file). `fk tickhook reads` PASS on the maps above.

Deleted, not flagged off: `forkoracle::blind` (the sweep), `car::locate_fast`
and `FK_FAST_LOCATE`, the `CAR_CHAINS` mirror and `resolve_chain`/`scan_near`,
`layout::find_clock`, `measured_clock_bias`, `WET_OFF` and the `q@−16` shape
assumption in `segments()`, `fk::locate::{find_clock2, confirm_clock,
locate_v2, qualify2, locate_pos2, locate_candidates, locate_positions_loose,
PosHit, ClockHit}`, `fk::validator::ValidatorCar`, `record::discover_layout`,
the reference-line bounding boxes every caller built to fence the sweep,
`fk trace`'s and `fk resync`'s retry ladders (kept only as the pre-spawn walk
above), and `fk liveness`'s fixture constant (`WHEEL0 = 496`, "408 bytes above
the anchor on this fixture") — the wheel block is at `vis+0xa8`, by the
engine's reflection, and that is what it reads now.

`fk ptr` (the chain finder) and regen's chain-anchor pool remain as regen's
own machinery; they no longer locate anything on the oracle's path.
