# 267460 `Impossible Mini Trial 2` — vj RESULTS v1 (the launch and the flight)

Agent tag `vj`, second agent on this map, 2026-08-18 22:30–23:40 local.
**Scope: the launch and the flight only.** By agreement with the first agent
(session d52644af) at 22:50, they own the prefix (spawn → the ramp) and the
post-landing endgame; ticks ≥ 1400 are mine. Their recon (`NOTES_20260818_1903`,
`GEOMETRY_20260818_2044`) and their tapes are theirs and untouched.

**Headline: no time improvement from my half. A route that looked open is
closed, and it is closed by the map, not by the search.** The evidence and the
enumerations are below, and one finding is worth more than the negative: the
author time does not decompose into any launch + endgame that either of us can
currently build.

Times in seconds. AT **16.888**; only human record Wirtual **23.068**; partner's
current best **21.918**.

---

## 1. Controls (every batch, on the untouched map)

| control | result |
|---|---|
| human WR re-simulated | **23.068** exact |
| partner's banked tapes, independently | **22.028**, **22.137** |
| identity through my own candidate encoder (`tmprobe prog`, no overrides) | 23.068 / 22.028 |
| **gate surgery origin control** — item 11 rewritten to its own (990,58,656) | **23.068** exact |
| waypoints | Spawn block#0 + Goal item#11 → **NbCheckpoints = 1**, confirmed |
| map | Nadeo's own copy, sha256 `4f0db768…` |

Gate surgery is `tmmaps moveitem` (position floats only; **model and yaw
untouched**), adopted from `285885/bis197047_tmmaps_main_moveitem.rs`, with
`--autocell` writing cell = (⌊x/32⌋, ⌊y/8⌋+8, ⌊z/32⌋) — which reproduces item
11's own stored cell (30,15,20), so §9b's offset is confirmed by the map's own
data rather than assumed.

## 2. The finish trigger, measured

Placing the gate exactly on the WR's own trajectory at 11 known times fires 9/11
within 25 ms (two fail: see below). Then sweeping one axis at a time:

- **x is a thin plane**, not a box: the firing time tracks the gate's x 1:1
  (945.4→17.128, 947→17.152, 949→17.188, 953→17.262, 957→17.345), offset ≈1 m.
  Independent confirmation needing no surgery at all: the human is on the finish
  platform from ~21.4 s at x≈1020 and crosses x=990 at 23.068 — a trigger with
  any real x extent would have fired seconds earlier.
- **y window = [gate_y − ~6, gate_y]**: the car must be at or *below* the gate
  origin. Gate y=58.5/59/60/62/64 all fire for a car at y=58.17; **y=58 does
  not**; y=66 does not.
- **z half-width ≈ 14 m**: gate z=698 fires a car at z=712.1, z=690 does not.

> **To finish, a car must cross the plane x = 990 with y ∈ [~51, 58] and
> z ∈ [642, 670].**

Two consequences that were not previously on the record:

1. **The WR's dive misses the finish in y as well as z, by 0.17 m.** It crosses
   x=990 at y = 58.17 against a ceiling of 58.00. A gate at (990,58,712) does
   **not** fire for the WR; (990,58.5,712) fires at **17.901**.
2. The two P-probes that failed are the two where the gate sat above the car —
   the same 6 m window, seen from the other side. An instrument that fires at
   the right millisecond 9 times and silently fails twice is exactly the
   `ACQUISITION_addendum_controls` shape; the axis sweeps are what pinned it.

## 3. The route question, and the answer

### 3.1 The hypothesis (hole B)

The z=686 screen's y=104 panel row sits at x = 800/832/864/896 in the free-block
chunk, so on any panel-size model there should be an opening around
**x ∈ [912…960] × y ∈ [88,120]**, and the launch at (862.6, 114.0, 710.3) at
62.24 m/s points straight at it. Ballistically, θ ≥ ~17° of southward yaw
crosses the screen inside that band; θ ∈ [12°,17°] crosses it inside the y=72
panel row (y 56–88, x 816–1072) and dies.

### 3.2 The aim is not the limit

Measured 26 m *north* of the screen, where nothing can interfere — gate at
(900,110,Z) fires iff the car's z at x=900 is ≤ Z+14:

| z at x=900 | implied θ | tapes (steer) | tapes (steer+brake) |
|---|---|---|---|
| ≤706 | ≥6.6° | 997 | 615 |
| ≤702 | ≥12.5° | 713 | 415 |
| ≤698 | ≥18.2° | 538 | 292 |
| ≤694 | ≥23.5° | 342 | 166 |
| ≤690 | ≥28.5° | 202 | 48 |
| ≤686 | ≥33.0° | 0 | 0 |

**The launch can be aimed to ~28.5°** — well past what hole B needs.

### 3.3 Nothing reaches the far side. The screen is unbroken.

| probe (air route only, crossing < 19.5 s) | tapes |
|---|---|
| x=940, y[84,96], z ≤ 690 | 243 |
| x=940, y[84,96], z ≤ 688 | 128 |
| **x=940, y[84,96], z ≤ 686** | **0** |
| x=950, y[82,88], z ≤ 690 | 260 |
| **x=950, y[82,88], z ≤ 684** | **0** |
| x=955, y[80,86], z ≤ 686 | 0 |
| x=960, y[78,84], z ≤ 684 | 0 |
| x=940, y[84,102], z ≤ 682 (strict, 5 cells) | 0 |

The frontier is **flat at z ≈ 687–688 at both x=940 and x=950**. A ballistic
tape aimed at 17° loses 3 m of z over those 10 m, so a flat frontier is not
flight — it is **cars stopped by the screen and running along it**. Since the
steeper the aim the further *west* the crossing (x_w = 862.6 + 24.3/tanθ:
28.5°→907, 23.5°→918, 18.2°→936), and all of those die, the y≈90–105 band of the
screen is **solid over at least x ∈ [907, 950]** — and the flight has fallen
below the window's y=88 floor by x≈950. **Hole B is not reachable.**

### 3.4 What the best aimed dive actually does

Crossing the finish plane x=990, air route only:

| y band at x=990 | best z reached | earliest crossing |
|---|---|---|
| y[52,58] | **z ≈ 687** | **17.828** |
| y[46,52] | ≈687 | 17.864 |
| y[40,46] | ≈690 | 17.932 |
| y[16,40] | ≈690 | 17.998–18.202 |
| **required** | **≤ 670** | — |

Right height, **17 m short in z**, at 17.828 — and it then falls into the void
strip between the slope's south edge (z=688) and the finish platform's north
edge (z=672). Landing on the platform is likewise unreachable: detectors at
(1008,56,656), (1008,52,656), (1016,56,664), (998,56,664), (1008,60,650) return
**0 air-route tapes** (207 hits, all of them the slow ground endgame at 22 s).

### 3.5 Enumeration behind the negative

- 2 808 single-hold programs (t ∈ [1450,1760] step 2 ticks, s ∈ {−127,−96,−64},
  d ∈ {8,15,25,40,60,90}) × brake on/off = 5 616, against 60+ relocated-gate
  cells; plus wider variants with s ∈ {−32,+64} and t from 1350.
- ~30 000 multi-segment programs from two beam searches (`tmbeam`, 12–24 ordered
  rungs, up to 4 segments, ~10 000 children/round).
- 882 spawn-dive programs (see §5).
- Every count above is filtered to crossings before 19.5 s, i.e. the air route.
  **Unfiltered counts are dominated by the ordinary 22 s ground endgame, which
  fires deep-z cells too** — this is the single easiest way to fabricate a
  discovery on this map, and it caught me once before I filtered.

## 4. THE ARITHMETIC THAT MATTERS MORE THAN THE NEGATIVE

The fall from the ramp exit (y=114, vy≈0, measured a ≈ 26 m/s²) reaches the
trigger's ceiling y=58 in **2.14 s** and its floor y≈51 in **2.31 s**, *whatever
the launch speed*. Therefore, for **any** air finish:

```
t_finish  >=  t_launch + 2.14
```

and for any ground finish, add the endgame. With the human's launch at 15.70 the
air floor is 17.84; the partner's 21.918 is launch 15.70 + 6.2 s of flight and
endgame; even with their best prefix (turbo gate 0.580 s early → launch ≈15.1)
and their 2.97 s endgame, the floor is **≈21.3**.

> **16.888 does not decompose into any (launch + flight + endgame) either of us
> can currently build.** An AT of 16.888 needs the car to leave the ramp by
> ≈14.7 s *and* an air finish — and the air finish is what §3 rules out.

Two possibilities remain, and they should be stated together:

- **There is a route neither of us has found.** Nothing here rules that out, and
  the fleet rule stands: "not humanly executable" is never a valid conclusion.
- **The AT may not have been driven.** The map is `validated="1"` and carries
  **no embedded ghost of any kind** — neither a `CGameCtnGhost` nor a bare
  `CPlugEntRecordData` (the container agent's 31-map survey lists 267460 under
  "nothing embedded"; ACQUISITION §9a verified it on the *decompressed* body:
  0 occurrences of 0x0911F000, 0x0309201D, 0x0303F005). On every other
  validated map in this project that node held the author's own lap. The
  unbeaten.at record carries `inPlugin: true` and no `atSetByPlugin` field.
  **This is a flag for the coordinator, not a conclusion.**

## 5. Other negatives, with their enumerations

- **Spawn dive through the z=740 gap at x ∈ [909,941]: dead.** 882 programs
  (steer hold from the spawn, t ∈ [155,400] step 5, s ∈ {+127,+64,−127},
  d ∈ {20,40,60,80,120,160}, gas forced 1, tail forced straight) against a
  10-cell detector spanning the whole x=990 plane (y ∈ [52,109] in contiguous
  6 m bands × z ∈ [642,698]): **0 reach it**. The detector gates sit at z=656/684
  with a 14 m half-width and **cannot see z≈749**, which is the north-side fall
  that trapped the first agent's earlier sweep — the trap is excluded by
  construction, not by inspection.
- **Forcing gas through the launch is worse, not better.** The human lifts off
  the gas at 15.670, 30 ms before leaving the ramp. Forcing gas=1 across the
  launch: **99** tapes reach (990, y[48,54], z[670,698]) against **1 156**
  without. The lift is deliberate.
- **The partner's fast-prefix family never reaches the ramp.** All 13 tapes
  (`vjx_prefix_turbogate_14659` + 12 siblings) DNF a gate at (848,116,710) that
  the human fires at 15.413, and the 14659 tape fires **nothing** in a full 2-D
  grid at x=860 covering y ∈ [98,122] × z ∈ [691,759]. With model-preserving
  gates it crosses x=840 at **(840, ≈110, ≈725)** — 4 m below the ramp surface
  and past its north edge: a faller. Reported to them at 23:36; their z=736/740
  firings came from a gate tool that swaps the item model (the 285885 hazard),
  and my model-preserving gates disagree with them (DNF at z=753, and a
  three-gate intersection pinning z≈725).

## 6. Tools banked (`vj_tools/`, Rust, no Python anywhere)

| file | what |
|---|---|
| `vj_tmprobe.rs` | hand-built candidate sweeps: `sweep` (grid over start tick × hold × steer, with `--pre` segments, `--gas/--brake`, `--tail straight`), `prog` (one explicit program), `files` (validate existing ghosts against a map), `info` |
| `vj_tmbeam.rs` | beam search over multi-segment input programs scored by an **ordered ladder of relocated-gate maps** (deepest rung fired, then earliest crossing), with `--max-ms` |
| `vj_tmmaps_main_moveitem_listall.rs` | `tmmaps moveitem` (position only, model/yaw untouched, `--autocell`) and `tmmaps listall` |

`--max-ms` exists because of a real defect in my own instrument: on this map the
deepest rung *is* the real finish, which the seed already fires — slowly — so
without a cap the ladder scores the 22 s endgame and the route search never
starts. **A ladder whose deepest rung is reachable by the boring route is not a
ladder.**

## 7. What I would do next, in order

1. **Find where the screen actually ends.** Every conclusion in §3.3 is a null
   result against a wall whose east edge I inferred from nulls. A direct
   measurement — drive a slow tape *into* the screen at a series of x at
   y≈95 and bisect where it stops — would either open hole B or close it for
   good. I ran out of clock before doing it properly.
2. **The void strip is 16 m wide.** The aimed dive puts a car at
   **(990, ≈55, ≈687) at 17.83 s** — level with the finish, 15 m north of the
   finish platform's north edge, with the low doorway (x>976, y<56) open all
   around it. That is 3.5 s better than where the partner's endgame currently
   starts. Crossing those 15 m is a *ground* problem on the slope, which is
   their half; the handoff state is banked (`vj_airdive_best_x990_at_17752`,
   `…_17728` — both **DNF on the real map, they are route probes, not results**).
3. Test an upward launch (vy>0 off the ramp). The y=136 row at z=686 covers only
   x ∈ [912,1008], so a crossing above y=120 west of x=912 is geometrically open
   and completely untested. It needs ≈+18 m/s of vy, which I do not believe the
   flat ramp can give — but I did not measure it.

## 8. Hygiene

Own build tree `/tmp/tmtas-vj267460` (hardened + v5 overlay; FINISH_BASE left at
1e8 — this map has one checkpoint and the defect bites at 6+), own copy of the
fork tree, every run with its own `--root` under `/dev/shm/`, cleanup by my own
roots and my own PIDs only.

**One incident to record against myself:** unpacking the hardened tarball with
`tar -C /tmp` overwrote the shared `/tmp/tmtas-hard` tree before I moved it
aside; I restored a full copy within a minute. Its `target/` was empty and the
extract was the same tarball the tree came from, so nothing appears to have been
lost — but a tarball extracted into a shared /tmp is exactly the hazard the v4
notice describes, and I walked into it. Extract to a private directory first.

---

## 9. Late addition: the screen's lower edge, measured directly

Ran after §7 was written, because §7.1 called the panel geometry the weak point.
Crossings of x=990 already south of the screen plane (gate z=678, window
z ∈ [664,692]), by height band, air route only:

| height band at x=990 | tapes through |
|---|---|
| y[52,58] | **212** |
| y[58,64] | 0 |
| y[64,70] | 0 |
| y[70,76] | 0 |
| y[76,82] | 0 |

So the low doorway's ceiling is **y ≈ 58**, which matches the *centre*-anchored
panel model exactly (the y=72 row spanning y ∈ [56,88]) and rules out the
corner-anchored one for that row. That in turn sharpens what §3.3 measured: the
deaths at crossings of (936, 92) and west of it are the **y=104 row**, and since
the centre model puts that row's panels at x = 800/832/864/896 → coverage
[784,912], **the map has screen there that the free-block list I read does not
account for** — most likely a panel at (928,104,686) or wider panels than 32 m.

Either way the measurement stands and the inference does not need to: **over
x ∈ [907,950] at y ∈ [88,105] the screen is solid, measured by 5 616 programs
that die against it**, and that is the entire span the flight can use while
still above the y=72 row. Somebody with a proper block-model dump should settle
which panel it is; it changes nothing about the route.
