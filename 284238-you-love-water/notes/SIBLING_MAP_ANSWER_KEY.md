# 284238 "YOU LOVE WATER" — the obstacle is REUSED on four other maps, and on one of
# them a human beats the author time. That run is this map's answer key.

Write-once sidecar, `cold_` prefix (cold-start agent). Supersedes nothing.
`RESULT-v1.md`, `RESULT-v2-symmetry-and-transplant.md` and `GEOMETRY_v1.md` are
all still correct on what they measured; this adds an outside reference they did
not have. Times in seconds. Nothing was submitted to any leaderboard.

---

## 0. Summary

284238's obstacle — the 40-block module placed four times — is **not unique to
this map**. Its author (Eating_My_Wings, TMX user 152751, 486 maps) reuses it
byte-identically. **279008 "Keep dropping"** is 284238 with the launchers
changed: **167 of its 186 block records are identical to ours** (same block, same
absolute position, same angles) and its four checkpoint gates are at the *same
world coordinates*. Its author time is 52.461 (ours 50.459) and

> **Yhomas_TM holds 46.112 on it — a human, beating that map's author time,
> driving OUR obstacle, in a clean single-life run.**

That ghost answers the questions this map has been stuck on:

* it clears the 71 m gap **four times out of four**, at **314.2 / 323.1 / 300.9 /
  304.0 km/h** — so the ">=300 km/h at the lip" figure is confirmed by a source
  that has nothing to do with our decode of the 440.238 record;
* its checkpoint crossings are **65.4 / 69.4 / 68.3 / 69.1 m/s** against our
  record's 52.8 / 45.7 / 40.3 / 36.3 — his cycle is a **high-energy fixed
  point**, ours decays;
* its cycle times are **11.257 / 10.417 / 10.870** — the ~11.67 s/cycle that
  50.459 requires is a thing a human does, repeatedly, on this geometry.

And it explains why 284238's field is stuck at 93.914 of clean driving:

> **The launch fails on SIDEWAYS velocity, not on speed.** All three launches we
> can measure hit the ice kicker at 91–99 m/s. The two that work cross it drifting
> toward −z (vz −17.9 and −25.1) and land on the wall curve LOW, where the surface
> is tangent; the one that fails crosses it at vz −3.2, flies past the tangent
> part and slams into the wall 21 m higher, losing 1630 of specific energy instead
> of ~320. **284238's record never steers on the launcher lane at all.**

---

## 1. How the siblings were found (method, reusable)

1. `meta`-free: TMX v2 API, `https://trackmania.exchange/api/maps?authoruserid=152751&count=100&fields=MapId,Name,MapUid,UploadedAt,DisplayCost`, paginated with `&after=<last MapId>`; 486 maps (`cold_author_152751_maps.tsv`).
2. Download each with `https://trackmania.exchange/maps/download/<MapId>` at ~1 req/1.5 s with a descriptive User-Agent (never a browser UA).
3. Fingerprint by **block census**: `tmmaps freeblocks MAP | awk '{print $2}' | sort | uniq -c`. 284238's module signature is
   `PlatformTechLoopStart:84  PlatformPlasticLoopOutStartCurve1:32  PlatformTechLoopStartCurve0OutFull:16  PlatformTechWallCurve3x4:8  PlatformTechSlope2Start:8  PlatformTechLoopStartCurve1In:8  PlatformIceLoopStartCurve0Out:4`.
   (A map whose blocks are all *placed* rather than free needs `tmmaps allblocks`, added this session; a map built from ITEMS shows an empty census — several of this author's do.)
4. Confirm geometrically by sorting `name,x,y,z,pitch,yaw,roll` and `comm`-ing the two files: identity of block records, not just of counts.

**Result over the 312 maps downloaded so far (of 486):**

| TMX id | name | module | AT | human record | note |
|---|---|---|---|---|---|
| **279008** | **Keep dropping** | **167/186 identical** | **52.461** | **46.112 Yhomas_TM** (2nd 92.018) | tech launchers, **no boost pads** |
| 284238 | YOU LOVE WATER | (this map) | 50.459 | 440.238 brick555 | water launchers + 6 boost pads |
| 299968 | Water Crimes pt1 | 161/186 identical | 226.362 | 214.879 ILLUSIUUM | tech launchers + water deco, slow route |
| 287506 | Banger banging u | same census | 339.804 | 227.039 Patriam | |
| 300007 | Climb up special edition | same census + extra | 1378.505 | 1258.025 Woshus | |

**What differs between 279008 and 284238, exhaustively** (19 block records):
15 `PlatformWaterRampBase` → `PlatformTechBase`; `PlatformWaterStart` →
`PlatformTechStart`; 3 `PlatformIceLoopStartCurve0Out` nudged < 1 m; the finish
net. Plus **284238 adds 6 `GateSpecial32mTurbo2` boost-pad ITEMS that 279008 does
not have** (items 198/199, 235/236, 239/240). Everything else — the chute, both
gap lips, the tube, the wall curves, the ice kicker, the four checkpoint gate
items — is identical, at identical coordinates.

## 2. Controls (an instrument that can only say yes is not an instrument)

* `tmtas validate` on their own maps: **46.112 / 92.018 / 214.879 exact**.
* **Entity-selection defect does not apply to Yhomas**: `tmtraj decode` reports
  one vehicle entity `0x0A018000` holding **923 of 923 samples**, and the input
  archive has **zero respawn packets** (4766 packets, all mode 2, one flags
  value). Single life, single entity, nothing to mis-select.
  Splits 5.238 / 16.495 / 26.912 / 37.782 / 46.112.
* **Our own record's decode was refereed against the oracle** at three points
  with honest `finrung` probes on the untouched map: predicted 16.2 → **16.183**,
  28.65 → **28.513**, 43.9 → **43.839**. For t < 44 s the decoded entity is the
  player's car. (The 153527 agent has since retracted the entity concern for this
  ghost independently.)
* `tmmaps moveitem` (new, position-only, no model swap) round-trip: moving boost
  pad item 239 — and separately 240 — onto its own position reproduces
  **440.238** and **97.325** exactly.

## 3. The answer key, in canonical module coordinates

Full 50 ms tables banked as `cold_answerkey_cycle1_canonical.tsv` (his CP1→CP2)
and `cold_answerkey_cycle2_canonical.tsv`. The load-bearing rows, with our
record's cycle 1 beside them (canonical frame, g = 24.6 m/s², linear air drag
k ≈ 0.038 s⁻¹, both fitted from the telemetry):

| phase (canonical) | Yhomas 46.112 | our record cycle 1 |
|---|---|---|
| lane entry, x ≈ 782 | 92.0 m/s, vz +41 | 78.1 m/s, vz +33 |
| steering on the lane | **steer −1 for ~60 % of it** | **steer ≈ 0 throughout** |
| ice kicker, x ≈ 903 | 99.1 m/s, **vz −25.1** | 97.2 m/s, **vz −3.2** |
| kick exit | (936.9,1889.3,917.9) v (72.4,56.1,−8.0) 91.9 | (933.9,1887.4,917.2) v (68.9,54.1,+9.0) 88.1 |
| wall-curve contact | **(980.1,1917.9,913.9) at 80.8, smooth** | **(1009,1930.5,927.2) at 72.1, SLAM** |
| energy lost, kick→CP | **311** | **1630** |
| CP crossing | (1048.7,1940.1,958.9) v (34.7,−8.6,59.5) **69.4** | (1047.6,1939.6,959.2) v (22.7,−4.0,39.5) **45.7** |
| fall entry, y=1918 | (1061.4,·,1007.6) v (−9.2,−44.6,56.8) 72.8 | (1060.2,·,1008.3) v (−7.8,−30.5,33.9) 46.2 |
| crossing y=1848 | z **1063.9** at **91.4** — caught | z **1049.0** at 34.4 — clipped the lip |
| lip | 323.1 km/h, gap cleared | fell into the bowl, +14.5 s |

Two things in that table are new and neither needed a search to find:

**(a) The chute is ballistic and lossless in both runs.** Between y=1918 and
y=1848 both cars are in free flight and both cover Δz ≈ +41 m. The 15 m
difference in where they land comes from where they *enter* the fall, which comes
from the CP crossing, which comes from the launch. The gap is therefore a
**landing-position** criterion, not a speed threshold: cross y=1848 at
canonical z ≈ 1057–1065 and the loop curve catches you; z ≈ 1049 hits the lip
block; the record's slow attempts that drift to z ≈ 1070–1075 are already dead.

**(b) The wall curve is the only thing on the map that converts +x motion into
+z motion**, and it is what the CP crossing needs (the fall must carry ~100 m of
+z). You cannot skip it by flying straight to the checkpoint: that arrives with
vz ≈ +8 and lands ~80 m short in z.

## 4. The boost pads: what the evidence does and does not say

I said earlier that the six extra pads force ~97 m/s into a catch that wants to
be met slowly. **That is not what the numbers say and I withdraw it.** The three
measurable launches sit at 90.9 (our standing start, works), 97.2 (our record
cycle 1, fails) and 99.1 (Yhomas, works). Speed does not separate them; vz does.

What survives is weaker and still worth knowing: the pads deliver +24 m/s inside
~1 s of lane, so the car spends the second half of the lane ~20 m/s faster than
Yhomas's car does at the same x, which leaves **less distance in which to yaw the
car** before the kicker. The pads make the required steering harder, they do not
make the launch speed wrong.

**The counterfactual is built and controlled** (`cold_nopads` construction:
`tmmaps moveitem --items 239,240 --at 500,1000,500`): on it the record cannot
even reach CP2 (`DNF cps=1`), i.e. the record's launch *needs* the pads. A full
no-pads vs pads comparison of a steered launch is the obvious next experiment.

## 5. Instruments built this session (all with two-sided controls)

| instrument | what it measures | control |
|---|---|---|
| `V1` finrung at Yhomas's kick exit, S-imaged into copy 1, `--gate 244 --keep 120` | did the car leave the kicker on his line | our record **22.128** (predicted 22.1); Yhomas on *his* map, same world point, **14.686** (predicted 14.70) |
| **`V2`** finrung at his **wall-curve contact** (748.44,1861.93,595.27, yaw 3.5578) | did the car meet the wall where he does | **says NO for our record**, and **YES for Yhomas at 15.278** on the identical geometry of 279008 — a probe that can say both |
| `Z1035…Z1085` ladder across the y=1848 plane | where the fall lands in z | record fires **Z1055** only, consistent with its measured z = 1049 |
| `pC` finrung at canonical (1007.9,1822.4,1065.5) | time from CP2 into the bowl/tube | fires for **both** branches: clean = CP+2.9, bowl-faller = CP+4.3 |
| `tmmaps moveitem` | park any item (boost pads) elsewhere, no model swap | identity round-trip exact (§2) |
| `tmpk asm steer:A:B:V` | force an arbitrary steer value over an ms window | zero-window is a no-op reproducing the base time |

Note on finrung geometry, learned the hard way: **a relocated finish is a
doorway, not a sphere** — 32 m wide, thin along its normal. Its yaw must be the
travel direction (a gate at ψ ± 90° is edge-on and never fires), and a probe
placed *before* a kept checkpoint voids the run instead of timing it, so probes
upstream of CP2 must relocate **gate 244** and keep only **120**.

## 6. What I got working, and where it still stops

Sweeping a *constant, gentle* steer over the last 0.3–0.55 s of the water lane
(`steer:21500:21950:−30` and neighbours, 190 tapes) puts our car **on Yhomas's
wall-contact line on the untouched map, with the boost pads in place**:

```
V2 fires at 22.842 – 22.898   (his equivalent phase: 22.68)
```

So the water lane **can** deliver the launch geometry the map wants; the record's
93.914 of clean driving is not a surface limit, it is a line nobody drove.

Grafting Yhomas's inputs from the wall contact onward (30 composite tapes,
graft phase swept ±0.3 s) now **collects CP1 and CP2** (`cps=2`, where blind
grafts previously lost CP2) but does not yet reach the bowl: the CP2 crossing
lands 0.26 s later than the record's and the fall misses the catch window. The
remaining gap is a state match at the CP, which is the other agent's lane and
which now has an exact target (§3).

Best honest measured gain from touching the launch alone: the copy-1 chute probe
`pC` **28.513 → 27.369**.

## 7. What I would do next, in order

1. **Search the launch against `V2` and then against a `Z1060` landing probe** —
   both are honest, both have two-sided controls, and the objective is now a
   geometric target rather than a finish time. The seed exists (§6).
2. **Re-drive the start.** Our spawn block is `PlatformWaterStart` where 279008's
   is `PlatformTechStart`, and everything else on the start lane is shared. Our
   record reaches CP1 at 52.8; Yhomas reaches it at 65.4 on the same lane. That
   segment has **no upstream coupling at all** — it is the cheapest 13 m/s on the
   map.
3. **Mine the remaining 174 maps** of the catalogue for the module, and pull
   every leaderboard ghost from all five sibling maps. Each is another clean
   single-life read of this obstacle.
4. **Do not** transplant copy 0's own cycle (115 tapes say no, `RESULT-v2` §3) and
   **do not** search against a promoted-gate objective without re-validating on
   the untouched map (`RESULT-v2` §5).

## 8. Artefacts

`~/persistent/private-30d/tm-unbeaten/284238/cold_siblings/` (with
`cold_SHA256SUMS.txt`):

* `cold_279008_keepdropping.Map.Gbx` — the sibling map
* `cold_279008_yhomas_46112.Ghost.Gbx` — **the answer key**, validates 46.112
* `cold_279008_ashura_92018.Ghost.Gbx`, `cold_299968_illusiuum_214879.Ghost.Gbx`,
  `cold_299968_watercrimes.Map.Gbx`
* `cold_279008_yhomas_46112_telemetry.csv` — 923 samples, single entity
* `cold_author_152751_maps.tsv` — the 486-map catalogue
* `cold_block_census.tsv` — block census per downloaded map
* `cold_answerkey_cycle*_canonical.tsv` — his cycles folded into module coordinates
* `cold_tools_v1.tgz` — `cold` (the fold/energy/fall/lip analyser, Rust, no deps)
  plus the `tmmaps moveitem` / `tmmaps allblocks` / `tmmaps allitems` /
  `tmpk asm steer:` patches used above

Node-local working tree (dies with the node): `/tmp/wcold` on
125408.od.fbinfra.net.
