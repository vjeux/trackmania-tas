# 267460 — `Impossible Mini Trial 2` — the author time did NOT fall

**Best validated: 21.918 s.** Author time **16.888 s**. Human world record
(Wirtual, the map's only record) **23.068 s**.

| | s | vs AT | vs human WR |
|---|---|---|---|
| author time (unbeaten) | 16.888 | — | −6.180 |
| **our best, validated** | **21.918** | **+5.030** | **−1.150** |
| event-thinned, 82 inputs | 22.290 | +5.402 | −0.778 |
| low-input, 76 inputs, 9 values | 22.698 | +5.810 | −0.370 |
| earlier banked | 22.028, 22.137 | | |
| human WR (Wirtual) | 23.068 | +6.180 | — |

Map uid `KLiIUnR3oNZTnfJwL3GImI1VtOl`, author **Mattlightning**, tags
Trial + Mini, 1 recorded run. Nothing here has been or will be submitted to a
Nadeo leaderboard.

---

## 1. The headline finding, which is not the time

**This is not a respawn map, and the brief's premise for trial maps does not
hold here.** The evidence is one line of the validator's own output:

```
"ValidatedResult" : { "NbCheckpoints" : 1, "NbRespawns" : 0, "Time" : 23068 },
"IsValid" : true
```

* **`NbRespawns: 0`** — the human world record contains no respawns at all.
* **`NbCheckpoints: 1`** — the map's only waypoints are the Spawn block and the
  Goal gate. There are **no intermediate checkpoints**, so there is nothing to
  respawn *to*: a respawn returns the car to the start line with the clock
  running, which is worse than any recovery. Confirmed independently from
  telemetry — 462 samples at 50 ms, no position discontinuity anywhere.

So the 6.180 s gap is **not** retry waste. `Trial` here is a building style
(small floating platforms) and not a checkpoint mechanic.

**Transferable answer on respawn semantics.** `NbRespawns` is a first-class
field in BOTH the `DeclaredResult` and the `ValidatedResult` blocks the
validator prints, and `IsValid` is the comparison of the two — so
`NbRespawns: 0` on every accepted run in this project is a property of the runs
we have fed it, not a rule of `/validatepath=`. (The 238835 agent has since
proven the rest empirically: a respawn is a bit in the packet's `word0`, `0x22`
soft / `0x1002` hard, and the validator exactly re-simulates a run with 198 of
them.) **The thing to check on any trial map before assuming retries can be
deleted is `NbCheckpoints`.**

---

## 2. What the map actually is

31 blocks, 17 items, 838 m, 23 s. **22 of the 31 blocks are
`CanopyCenterFlatBase`** — the big flat stadium screens, rotated vertical.
They are not scenery: they are two solid walls, at **z = 740** and **z = 686**,
and every route question on this map is "which hole, in which wall".

Free blocks keep their world position in chunk **`0x0304305F`** (24 bytes each:
Vec3 pos + Vec3 rotation), *not* in the block record — the record carries cell
`(-1,0,-1)` for every one of them. The first 24 records of that chunk are the 24
free blocks **in block order**, which is what lets them be named. Full
panel-by-panel tables in `GEOMETRY.md`; the short version:

* **z=740 wall** blocks the direct drop from the start platform onto the turbo
  gate. Its y=119 row covers x 717–909 and the start platform sits at x 845–925.
* **z=686 wall** separates the flight corridor from the finish. Its y=72 row
  covers y 56–88 for x 816–1072. The only doorway near the flag is
  **y < 56, x > 976**.

Driving surfaces: an ice run at y≈135 (x 845–925), a cluster of 30°-rolled dirt
platforms at y 96–112 ("the pit"), a run-up platform at (830,112,736), a grass
landing L at y=40 (x 1056–1120), and the finish platform at y=48
(x 992–1024, z 640–672) carrying four `ObstaclePillar2m` and the flag.
Three special gates: turbo on the start line, **`GateSpecial32mTurbo` at
(846,114,720)** which throws the car across the map, and
**`GateSpecial32mNoEngine` at (1056,49,672)** which kills the engine for the
last stretch.

## 3. Where the human's 23.068 s goes

Splits from relocated finish gates, each its own map, one map per worker root:

| gate | human | ours | what |
|---|---|---|---|
| (835.5,135.7,749.8) | 1.985 | 1.985 | flat out west on the ice |
| (758.4,133,749) | 3.946 | 3.946 | off the west end, airborne |
| (716.3,109.4,728.4) | 5.979 | 5.979 | bottom of the pit, 69 km/h |
| (740.6,108.8,727.5) | 12.969 | 12.969 | back out, charging east |
| (840,114.3,711) | 15.239 | 15.239 | **through the big turbo gate** |
| (995.7,57.8,712.4) | 18.018 | 18.015 | mid-dive, 257 km/h |
| (1000,52,662) — x=1000 plane | 22.239 | **21.660** | on the finish platform |
| finish | 23.068 | **21.918** | |

**Nine of the 23 seconds are the pit** (3.9 → 12.9): 151 m at 45–100 km/h on
tilted dirt. **Four more are the endgame** (19.0 → 23.068): land on the grass,
overshoot to x=1091, U-turn, cross the no-engine gate, jump the 32 m gap up to
the finish platform, thread the pillars, and coast in. The human crosses the
line at **8.5 km/h**, having gone 75.9 → 8.5 km/h in the final half second —
it arrives into the flag structure rather than through it.

## 4. Our 1.150 s, and where it is

**All of it is after 18.0 s.** Our tape is bit-identical to the human's through
every gate up to the mid-dive; the prefix was never improved in the banked tape.
The endgame splits into two halves of almost equal size:

* **0.576 s between the dive and the x=1000 plane** (3.645 s vs 4.221 s): a
  tighter landing and turn-around, and a different line through the gap jump —
  our tape does not fire a gate at (1020,52,680) that the human fires at 21.408,
  so it comes back on a different line.
* **0.574 s in the final 10 m.** The human takes **829 ms** to cover the last
  10 m (≈43 km/h average); ours takes **258 ms** (≈140 km/h average). The human
  brakes from 20.0 to 21.4 s (`brake=1` held through the jump and the pillars)
  and arrives at the flag nearly stopped. **Carrying speed across the finish
  platform instead of braking onto it is the single biggest thing a driver can
  take from this run.**

## 5. Three routes to the author time that do not exist (all measured)

The 6.180 s has to be somewhere, and the obvious candidates are all closed.
Recording the negatives because each cost real box time.

**(a) Fly through the flag mid-dive.** At 18.018 the car is at
(995.7, 57.83, 712.37) at 257 km/h and the flag is at (990, 58, 656) — same x,
same y to 0.2 m, 56 m adrift in z. It looks like the whole answer. It is not:
a gate ratchet pulled the flight corridor from z≈712 to **z≈688** and then
stalled — **414 000 evals, 0 finishers, 4 m further**. The z=686 wall is at
z=686 and the corridor was already against it. 49 tapes out of a launch sweep
DO get **through** the low doorway, at (1010, 46, 680) at 17.955 s — and then
hit nothing at all (7 probe gates beyond, 0/49): the doorway is at x>976 and the
flag is at x=990, so a car crossing the wall eastbound is already past it and
four metres below the finish platform. To fire the trigger in the air it would
have to cross z=686 at x≈980 and then make −34 m of z for +10 m of x — near due
south, which the launch cannot produce.

**(b) Drop south off the start platform straight into the turbo gate.** The
turbo gate is 70 m from the spawn and the human takes 15.2 s to reach it. It is
behind the z=740 screen. **2600 hand-built tapes** (human line to tick T, then
hold steer S for D ticks; T ∈ [60,320], S ∈ ±{32…127}, D ∈ {15…120}) — 1114
still reach the first progress gate, **0 reach any gate within 32 m of the turbo
gate**. A partner agent reproduced the negative independently with 882 programs
against a 10-gate detector for the x=990 column: 0/882.

**(c) Land on the dirt slope north of the finish platform.** The partner agent
measured that an aimed dive puts the car at (990, ~55, ~687) at 17.83 s — level
with the flag, with the low doorway open around it, and only ~15 m north of the
finish platform. My own aimed tapes confirm the state: 1831 of a 5940-tape
launch sweep reach (995.7, 57.8, 704) and 1608 reach (1000, 56, 698), at
17.77 s, keeping the human's height. **But nothing crosses the last 16 m.**
0 of 5940 reach any gate on the finish platform, and a ladder-shaped search
from the best slope seed (62 workers, 31 min, **408 300 evals, 0 finishers**)
never got past the slope rung. The strip between the slope's south edge
(z=688) and the platform's north edge (z=672) is void, and the car arrives
there with no way to cross it.

> **CORRECTION, and it is a methodological one worth more than the negative.**
> Those three platform detectors were **broken instruments**: gates at
> (1005,50,665), (1012,50,660) and (1000,52,668) **cannot be fired by our own
> 21.918 tape**, which drives across that spot. Verified here: at x=1005 the
> gate fires only at **y=54** (21.546); y=46, y=50 and y=58 all DNF. The car
> crosses x=1005 at y ∈ (50,52) and the two detectors sat on either side of the
> asymmetric 6 m y window without containing it. **So "0 of 5940" was measured
> by an instrument that could only ever say no.**
> The partner agent re-ran it properly — detectors at y=54, z=656, with the
> 21.918 tape as a **yes-control** first (it fires all four: 21.660 / 21.479 /
> 21.306 / 21.132), then ~1400 programs per detector in both mutation windows —
> and **the negative survives**: no arrival earlier than the incumbent's own
> path. The conclusion stands; the original evidence for it did not.
>
> **The rule this yields, which applies to every probe in this document:
> a detector gate needs a YES-CONTROL before its zero means anything.** Fire it
> first with a tape known to pass through that point. A gate is a small
> asymmetric box (±14 m in z, `[y−6, y]` in y), so a detector four metres out in
> y is not approximately right — it is silent. On this map the same class of
> error produced three false positives (§5d and the two gate traps) and now one
> false negative; §7's model-swap caveat is a third variety of the same disease.
> **The two negatives in this document never re-measured with a yes-controlled
> detector are the hole-A doorway tolerance and the aim ceiling.** Treat them as
> provisional.

**(d) A faster line through the pit.** Tapes DO reach (740.6,108.8,727.5) at
4.9 s against the human's 12.969 — and it is an artefact: that gate has a 14 m
z half-width and the car is *falling* through the same volume on the way down,
not driving out of it eastbound. Every such tape then reaches nothing.
**A relocated gate is only a valid objective if reaching it implies the route.**

## 6. What the remaining 5.030 s probably is

Honest answer: **the pit, and we could not crack it.** It is the only place with
that much time in it — nine seconds to descend 16 m and come back out 24 m east
of where you fell in. A turbo-gate-objective search did find a genuinely faster
prefix — **14.659 at (840,114.3,711), −0.580 s**, and it arrives 12–18 m further
north on the run-up ramp than the human (it fires ramp gates out to z=740 that
the human's run does not reach) — but that arrival state kills the flight: 110
workers × 17 minutes of ladder-shaped search never got it past the turbo-gate
rung again. That 0.580 s is real, unclaimed and handed to the partner agent in
`prefix_handoff/`, where it may be the enabling term for the "hole B" route
(a second, higher gap in the z=686 wall at x ∈ [912,1040], y ∈ [88,120]) that
needs about +3 % launch speed.

## 7. Validation

* **Cold validation of the headline**, fresh directory, fresh server process,
  map **copied** (not symlinked), human ghost alongside as a known-answer
  control: `"Time": 21918`, `"NbCheckpoints": 1`, `"NbRespawns": 0`,
  `"Desc": "validated time is actually better! (23068 > 21918)"`, correct
  `MapUid`; control returns **23068** exactly. (`IsValid: false` is the standard
  signature of a run faster than the time its file declares.)
* Identity control: the human WR re-simulates to **23.068** exactly — that is
  the whole field on this map, 1/1.
* Candidate-encoder round trip: `tmsearch --verify` on the human tape → 2462
  ticks → **23.068**.
* Gate surgery no-op: the Goal put back at its own position (990,58,656),
  cell (30,15,20) → **23.068**.
* Map provenance: fetched from Nadeo's own
  `core.trackmania.nadeo.live/maps/<guid>/file` (307 → Ubisoft CDN, no auth),
  sha256 `4f0db768139245d9b2066b08d9f471cc057718e8c43c2df0f1bbe679ee64c55f`.
* **Not exposed to the known phantom defects.** Every `tmsearch` carried an
  explicit distinct `--root`; the `--fork` path was never used anywhere in this
  work (classic full-simulation oracle throughout), so neither the shared-root
  nor the fork-resume defect can apply; no sub-tick plane surrogate was used.
  A second agent independently re-validated the 22.028 and 22.137 tapes.
* **CAVEAT ON THE PROBE GEOMETRY — `tmmaps gate` swaps the gate MODEL.**
  `segments::move_gate` rewrites the Goal item's model as well as its position:
  the untouched map carries `GateFinishCenter32mv2` and every relocated-gate map
  built here carries **`GateFinish32m`**. The identity control still returns
  **23068** exactly, so the substitute trigger is compatible on the human's own
  crossing, and **every time reported in this document was measured on the
  untouched map** — the headline results are unaffected. But the trigger
  tolerances in §4 (±14 m in z, the asymmetric `[y−6, y]` window) are properties
  of `GateFinish32m`, and so are the route negatives in §5 that rest on probes.
  They should be read as "measured with a 32 m finish gate of a slightly
  different model at that position". Two things support them anyway: a partner
  agent using a model-preserving `moveitem` port measured the same ±14 m z
  half-width and the same asymmetric y window independently, and reproduced
  every one of the splits in §3 exactly. The fleet's standing advice is to
  prefer `moveitem`/`ladder` over `move_gate` for exactly this reason.
* **No phantoms.** Nothing failed re-validation, so nothing was written to
  `tm-loop/phantoms/`.

## 8. §9 check — no embedded author ghost

The header says `validated="1"`, so the AT is a driven validation lap, but
**the map carries no ghost**. `tmtraj decode map.Map.Gbx` reports no
`CPlugEntRecordData`; because a `.Map.Gbx` body is LZO-compressed I decompressed
it (595 653 bytes) and scanned the decompressed body for the class ids directly:
**0 occurrences of `0x0911F000`, 0 of `0x0309201D`, 0 of `0x0303F005`**. The
single `0x03092000` hit is coincidental bytes inside the baked-blocks chunk
`0x03043048`. New `tmmaps chunks` / `tmmaps body` do this check properly.

## 9. Driving guide — what a person should take from this

The route is forced; there is no secret line. What is worth practising is the
**last four seconds**, which is where the entire TAS margin is and where a human
is throwing away more than a second.

1. **The pit (3.9 → 12.9 s).** Unavoidable. You have to go west past x≈717 —
   that is the only gap in the near screen — drop into the dirt cluster, and
   climb back out to reach the run-up platform. Nine seconds is what it costs
   the WR; it is also where the author's remaining margin must be, so it repays
   practice more than anything else on the map.
2. **The turbo gate and the dive (15.2 → 19.0 s).** Ballistic. Nothing you do in
   the air changes where you land. Do not try to steer toward the flag — you can
   see it out of the window and you cannot reach it; the screen is in the way.
3. **The landing (≈19.0 s).** The WR lands at x≈1066 still pointed east, runs on
   to x≈1091 and turns around. **That U-turn is about half a second.** Land
   already turning.
4. **THE ONE THAT MATTERS: do not brake onto the finish platform.** The WR holds
   the brake from 20.0 s through the no-engine gate, the gap jump and the
   pillars, and crosses the line at 8.5 km/h — 829 ms for the last 10 m. Our
   tape crosses the same 10 m in 258 ms. The engine is dead after (1056,49,672),
   so **every km/h you brake away before that gate is gone for good**: speed is
   the only thing you can still spend on the far side. Carry it through the gap
   jump, thread the pillars at speed, and let the flag stop you.
5. **The drivable versions.** The unconstrained tape is 515 steer change events
   over 214 distinct values — per-tick noise no person could reproduce. It
   simplifies a very long way:

   | tape | s | vs human WR | change events | distinct steer values |
   |---|---|---|---|---|
   | unconstrained | **21.918** | −1.150 | 515 | 214 |
   | event-thinned | **22.290** | −0.778 | **82** | 30 |
   | low-input / near-keyboard | **22.698** | −0.370 | **76** | 9 (`±127`, `0`, and six small residues) |

   All three validated through the plain oracle in one batch with the human
   ghost returning 23.068 as the control. Greedy event deletion alone removes
   **433 of the 515 events for nothing at all** — the tape's real structure is
   about 80 held segments, and most of what looks like precision is search
   noise. Complete per-event scripts are in the `.script.txt` files.
   **And point 4 above needs no tape at all: it is a decision about the brake,
   and it is worth roughly half a second on its own.**

## 10. Files

In `~/persistent/private-30d/tm-unbeaten/267460/`:

| file | what |
|---|---|
| `m267460_TAS_analog_21918ms.Ghost.Gbx` | **the result** (md5 `c4cf7484…`) |
| `m267460_TAS_analog_21918ms.tick.csv` | its complete per-tick input trace |
| `m267460_TAS_thinned_82inputs.Ghost.Gbx` + `.script.txt` | 22.290 s, 82 change events, 30 steer values |
| `m267460_TAS_lowinput_76inputs.Ghost.Gbx` + `.script.txt` | 22.698 s, 76 change events, 9 steer values |
| `m267460_validated_22028.Ghost.Gbx`, `…_22137.Ghost.Gbx` | earlier banked, both re-validated by a second agent |
| `human_WR_23068_Wirtual.Ghost.Gbx` + `…_trajectory.csv` | the reference run and its decoded telemetry |
| `map_267460.Map.Gbx`, `mapinfo.json`, `thumb.jpg` | the map as Nadeo serves it |
| `map_block_positions.txt` | every world position in the map body |
| `GEOMETRY.md` | the walls, their holes, and the three closed routes |
| `prefix_handoff/vjx_prefix_turbogate_14659.Ghost.Gbx` + 12 siblings | the −0.580 s prefix, tails unusable |

## 11. Tooling added (Rust)

* `tmmaps listall` — every block and item with name, cell, flags, world position.
* `tmmaps gate MAP --at x,y,z [--yaw] [--cell] --out M` — relocate the Goal gate
  anywhere. `tmmaps probe` cannot be used on this map: it hard-requires a
  Checkpoint block and this map has none.
* `tmmaps scanpos` / `chunks` / `body` — find world positions in the body, list
  every skippable chunk of the DECOMPRESSED body, dump it.
* `tmgen` — enumerate tapes from an explicit plan (`--t0 LO:HI:STEP --steer …
  --dur … [--start-from]`) instead of perturbing one. **This found things the
  search could not**: the low doorway in the z=686 wall, and the two negatives
  in §5. On a map where the incumbent's route is the question, enumerate.
* `tmsearch --ladder` — score a DNF by the deepest rung of a gate ladder it
  still fires. The stock `--seg` shaping is dead on a one-waypoint map because
  `reached_cps` is always 0, and a blind search there is *completely* blind:
  measured 414 000 evals with zero movement.
* `mutate::redrive` (`--ops mixR`, `--ops redrive`) — replace a whole window
  with a fresh piecewise-constant plan. Every other operator perturbs the
  incumbent, which cannot propose a different route.

### Bugs found and fixed

1. **The item y cell is `floor(y/8) + 8`, not `floor(y/8)`** — the map's vertical
   origin sits 8 cells (64 m) below y=0. A wrong y cell still loads and still
   usually fires, so a relocated gate built with it is a *silently inconsistent
   instrument*: the same z-tolerance sweep run with each convention gave
   mutually contradictory answers.
2. **`FINISH_BASE` at 1e8 lets a deep ladder rung outrank a real finish** once
   the ladder passes 10 rungs (rung score is `k·1e7 − t`). Raised to 1e9.
3. **The ladder scan order must start near the incumbent's own depth.** A gate
   is a band, not a half-space — a deeper candidate stops firing the shallow
   rungs — so a naive deepest-first scan costs one server run per rung and drops
   throughput 12×.
4. **`--simplify` phase 2b loops forever** on a degenerate ramp whose two holds
   carry the same value (267460: span 345..349, `127 -> 127`), and the tool
   writes its output only at the end, so a looping run produces nothing at all.

### The methodological lesson

**A detector gate needs a YES-CONTROL before its zero means anything** — fire it
first with a tape known to pass through that point. And: **A relocated gate is only a valid search objective if reaching it implies the
route.** Otherwise it is a probe, valid only on tapes independently known to be
on the route. This map produced three separate false positives from that one
mistake — a "shortcut" to the pit exit 8 s early, 124 hits "at the turbo gate",
and a 3.309 s "northern ramp arrival" — every one of them a car falling through
the gate's 14 m half-width on the wrong side of a screen. Cross-check every new
hit against a second gate the same tape must also fire.
