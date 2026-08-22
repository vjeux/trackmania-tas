# TM2020 `CPlugEntRecordData` (0x0911F000) — decoder, validation, and racing-line analysis

**Map:** *Summer 2026 - 01*. **Date:** 2026-08-17. **Runs decoded:** 51 real human ghosts
(the 44-ghost leaderboard-depth population `p00001_19538` … `p09994_19812`, plus 7 unique
ghosts from the original set of 11).

**Bottom line:** the decoder works and is validated to the metre against independent ground
truth. And the answer to the question you actually asked is **no — the world-record line is
not geometrically distinct. All 51 humans, from world record to position ~10 000, drive the
same line.** The WR is *more* typical than the average run (rank 10 of 51 by centrality);
it wins by carrying 3–4 km/h more speed along that same line. There is no line diversity in
the human population to seed a search from.

---

## 1. The format

`CPlugEntRecordData` sits in the ghost body as chunk `0x0911F000`. Layout at the chunk site
(TM2020 ghosts are chunk **version 11**):

```
u32 chunkId          = 0x0911F000     (often preceded by the same value as the node class id)
u32 version          = 11
u32 uncompressedSize
u32 compressedSize
<zlib stream, starts 78 9C>
```

Versions ≤ 4 store the record inline uncompressed — not seen in TM2020, not implemented.

### Decompressed payload grammar

Derived from GBX.NET (`Src/GBX.NET/Engines/Plug/CPlugEntRecordData.cs`,
`Src/GBX.NET/Engines/Scene/CSceneVehicleVis.EntRecordDelta.cs`,
`Src/GBX.NET/Serialization/GbxReader.cs::ReadTransform`) and confirmed byte-exact here.

```
v>=1 : i32 start_ms ; i32 end_ms
       i32 nDesc ; nDesc x EntRecordDesc
           EntRecordDesc := u32 classId ; i32 ; i32 ; i32 ; data ; i32
v>=2 : i32 nNotice ; nNotice x NoticeRecordDesc
           NoticeRecordDesc := i32 ; i32 ; v>=4: u32 classId
       EntList := u8 hasNext ; while hasNext:
           i32 type              # index into the EntRecordDesc array
           i32 u01 ; i32 u02     # u02 ~ start ms
           i32 u03               # ~ last recorded ms for this entity
           v>=6 : i32 u04
           v>=11: EncodedDeltas          (v<11: while u8: (i32 time ; MwBuffer))
           u8 hasNext
           v>=2 : Deltas2 := while u8: (i32 type ; i32 time ; MwBuffer)
v>=3 : BulkNoticeList := while u8: (i32 ; i32 ; MwBuffer)
       CustomModulesDeltaLists:
           v>=8 : i32 nLists  (else 1)
           each := (while u8: i32 ; MwBuffer ; v>=9: MwBuffer) ; v>=10: i32 period

EncodedDeltas := i32 numSamples
                 if numSamples: i32 sampleSize
                 numSamples x i32 deltaTime        # cumulative -> absolute ms
                 COLUMNAR DELTA CODING:
                   for byteIndex i in [0, sampleSize):
                     read numSamples bytes; running u8 accumulator across the
                     samples (acc += byte, wrapping); sample[b].data[i] = acc.
                   The accumulator RESETS at the start of every column.
```

`MwBuffer` = `i32 length` + raw bytes. Times are `TimeInt32` = plain `i32` milliseconds.

The columnar/delta layout is the interesting part: the data is stored **byte-column major**
(byte 0 of every sample, then byte 1 of every sample, …) and delta-coded horizontally along
each column. That is why it zlib-compresses so hard — the WR's 391 × 116 = 45 356 sample
bytes plus overhead go from 55 306 to 8 605 bytes.

### The vehicle sample (`CSceneVehicleVis`, class `0x0A018000`)

TM2020 records **116 bytes per sample at a fixed 50 ms period** (the record is *not* at
physics-tick rate; 391 samples for a 19.538 s run). Fields at fixed byte offsets:

| Offset | Type | Field |
|---|---|---|
| 2 | u16 | sideSpeed → `((v/65536)-0.5)*2000` |
| 5 | u8 | rpm |
| 6–13 | u8 ×8 | FL/FR/RR/RL wheel rotation + rotation count |
| 14 | u8 | steer → `((v/255)-0.5)*2` |
| 15 | u8 | (gas base; `gas = u15/255 + brake/255`) |
| 18 | u8 | brake |
| 21 | u8 | turboTime |
| 23–30 | u8 ×8 | FL/FR/RR/RL damper length + ground-contact material id |
| 31–33 | u8 ×3 | isTurbo, slipCoef1, slipCoef2 |
| **47–68** | | **transform (22 bytes) — see below** |
| 76 | u8 | vehicleState (bit 0x20 = top contact) |
| 81–84 | u8 ×4 | FL/FR/RR/RL icing |
| 89–91 | u8 ×3 | groundMode, boosterAirControl, **gear** |
| 93/95/97/99 | u8 | FL/FR/RR/RL dirt |
| 101–102 | u8 ×2 | wetness, simulationTimeCoef |

**The transform, 22 bytes at offset 47:**

```
f32 x, f32 y, f32 z          world position in metres, Y is up
u16 angle                    * pi / 65535
i16 axisHeading              * pi / 32767
i16 axisPitch                * pi / 32767 / 2
i16 speedLog                 speed_m_s = exp(v / 1000)
i8  velHeading               * pi / 127
i8  velPitch                 * pi / 127 / 2

quaternion = ( sin(angle)cos(axisPitch)cos(axisHeading),
               sin(angle)cos(axisPitch)sin(axisHeading),
               sin(angle)sin(axisPitch),
               cos(angle) )                                 -> (x, y, z, w)
velocity   = speed * ( cos(vp)cos(vh), cos(vp)sin(vh), sin(vp) )
```

Two things worth knowing that the GBX.NET source does *not* say and I had to establish:

* the velocity tuple as computed above **is** the world (x, y, z) velocity — no axis swap,
  despite the "heading/pitch" naming suggesting a Z-up convention;
* the quaternion's **local +Z is the car's forward axis and local +Y is its up axis**.

**Gear** is stored as `1 + 4*gear`: the byte only ever takes the values 5, 9, 13, 17, 21 →
gears 1–5. **RPM** is a raw 0–255 byte; the absolute rev scale is unknown.

### Bonus: the split times

Checkpoint splits live in a *skippable* ghost chunk `0x0309202B`, which is why a naive
search for the chunk id lands in the wrong place — the id is followed by the marker
`PIKS` (`0x534B4950`, "SKIP" as a LE u32) and a size:

```
u32 0x0309202B ; 'PIKS' ; u32 size(=60) ;
u32 version=1 ; i32 raceTime_ms ; i32 ; i32 ; i32 nbRespawns(=3 for all our ghosts) ;
i32 nCheckpoints ; nCheckpoints x (i32 time_ms, i32 stunts-or-flag) ; i32 -1
```

### Entities present, and what is NOT decoded

A TM2020 ghost record carries 7 entity descriptors; the ones with samples are
`0x0A018000 CSceneVehicleVis` (116 B/sample, 50 ms — the car) and `0x2D001000`
(13 B/sample, 50 ms — not decoded). Also present but empty here: `0x0A019000`,
`0x2F0CB000`, `0x032E3000`, `0x032AC000`, `0x032CB000`. Not decoded: those entities'
byte layouts, the 82 notice records, `Deltas2`, and the custom-module deltas — GBX.NET
models them as opaque buffers too, so there is no reference to transcribe from.

**Watch out:** two of the 44 population ghosts (`p00041_19580`, `p00701_19628`) contain
**two** `CSceneVehicleVis` entities — a decimated one (6–7 samples ~3 s apart) *and* the
real full-rate track. Taking the first match silently yields a 7-point trajectory. The
decoder now always takes the entity with the most samples.

---

## 2. Validation

`entrec.py --selftest` runs all of this. **Every test passes on both the WR and the
last-place ghost.**

| Test | WR `01_19538` | Last place `slow_p10000_19812` |
|---|---|---|
| T0 blob consumed exactly | 55 306 / 55 306 B | 56 168 / 56 168 B |
| T1 splits from chunk `0x0309202B` | 7617/13308/16316/19538 ✔ | 7630/13406/16572/19812 ✔ |
| T2 position at t=0 vs start block centre (1584, 784) | **0.000 m** | **0.000 m** |
| T3 CP1 (1232, 976) | 2.61 m at t=7634 (+17 ms) | 4.23 m at t=7646 (+16 ms) |
| T3 CP2 (1154, 1328) | 9.42 m at t=13351 (+43 ms) | 7.34 m at t=13450 (+44 ms) |
| T3 CP3 (1360, 1104) | 2.25 m at t=16324 (+8 ms) | 5.83 m at t=16579 (+7 ms) |
| T3 finish | last sample (1362.7, 706.8) at 19500; extrapolated to 19538 → (1362.8, **702.1**), inside the finish cell z[672,704] | (1363.3, 702.0), inside |
| T4 continuity | max implied step speed 473.0 km/h | 470.2 km/h |
| T5 speed sanity | v(0)=0.81 km/h, peak **471.0** km/h @16800 | 0.81, peak 468.2 @17000 |
| T6 decoded speed vs \|d(pos)/dt\| | median err **0.046 m/s**, p95 0.391 | 0.045 / 0.398 |
| T7 velocity direction vs path tangent | median cos **0.99995** | 0.99996 |
| T8 quaternion unit + forward axis | \|1−\|q\|\| ≤ 2.2e-16; median cos(+Z, v) 0.9999 | same |
| T9 **your independent gate measurement** | see below | — |
| T10 gear quantisation 1+4k | raw ∈ {5,9,13,17,21} | same |

### The strongest evidence

**Start position.** The WR's first decoded sample is `(1584.000, 18.002, 784.000)`. The
start block is cell (49, 7, 24) → centre `(1584, ·, 784)`. Exact, to the millimetre, with
no fitting. Across all 51 ghosts the largest start-position error is **0.009 m**.

**Your independent finish-gate measurement.** You measured the WR passing six points 4 m
apart at t = 614, 946, 1188, 1388, 1563, 1720 ms. Integrating arc length along the decoded
trajectory and sampling it at exactly those six times gives gaps of

```
4.048, 4.051, 4.043, 4.020, 4.071  metres      (expected 4.000)
```

Five independent intervals, each 4.00 m to within 1.2 %, from a completely different
measurement method. That pins both the position scale and the time base.

*One caveat, stated plainly:* those six times land in the **start** phase of the run,
where the decoded car is at x = 1584 travelling in +z — not at x ≈ 1232 ± as your note
labels them. The *spacing and timing* reproduce perfectly, and the absolute frame is
independently confirmed by the start/CP block geometry, so I believe the x-labels in your
note refer to a different axis or origin in the gate-placement tooling rather than to a
decode error. Worth reconciling if you rely on those absolute numbers elsewhere.

**Checkpoints, whole population.** Over all 51 ghosts, closest approach to the nominal
checkpoint centres at the declared split times: CP1 min 0.68 / median 4.68 / max 8.74 m;
CP2 7.30 / 9.43 / 10.14 m; CP3 0.94 / 3.91 / 10.57 m. Checkpoint blocks are 32 m cells, so
a car passing 5–9 m from the *centre* is simply driving a line through the gate. The
consistent CP2 residual (~9 m, and always ~+43 ms) is a fixed offset between the gate
item's origin and its trigger plane, not decode drift: it is the same for every run.

**Finish.** The record always stops slightly before the finish crossing (`end_ms` 19530 vs
race time 19538). Extrapolating the last sample's velocity to the declared finish time puts
the car at z ≈ 702, i.e. 2 m inside the finish block's near edge (z = 704) — exactly where a
finish trigger fires.

### Field confidence

**VERIFIED** (numerically cross-checked): `time_ms`, `x/y/z`, `speed_ms`/`speed_kmh`,
`vx/vy/vz`, quaternion, derived `yaw/pitch/roll`, `gear`, `rpm_raw` (byte only).

**DERIVED** (GBX.NET reference, internally consistent, no independent check here):
`steer`, `gas`, `brake`, `side_speed`, `turbo_time`, `is_turbo`, `is_ground_contact`,
`is_top_contact`, `wetness`, `sim_time_coef`, wheel rotations, damper lengths.

**GUESS** (byte offset + name only): icing, dirt, `ground_mode_raw`,
`booster_air_control_raw`, `vehicle_state_raw`, ground-contact material ids.

---

## 3. Are the lines different?

Method: every path is resampled at **equal arc length** (so speed cannot masquerade as
geometry), then each run's **signed lateral offset** from the WR line is measured at 300
stations (6.1 m apart) by closest-point projection onto the WR's local normal. Distance
between two runs = RMS of the difference of their lateral profiles, in metres. Clustering
is complete-linkage agglomerative with a metres-valued threshold.

### Number of "distinct lines"

| eps | my projection method | `lines.py` (fraction-matched) | `lines.py --dtw` |
|---|---|---|---|
| 1.0 m | 37 | 50 | — |
| 2.0 m | **18** | 30 | 24 |
| 5.0 m | **3** | 8 | — |

**These cluster counts are meaningless, and that is the finding.** At every eps the
*minimum separation between the resulting clusters* is 0.8–1.0 m while the *within-cluster
spread* is up to 4.6 m. The clustering is slicing a continuum, not finding modes. For
contrast, `lines.py --demo` on two genuinely distinct synthetic lines gives 0.8 m within and
**11.2 m between** — the correct signature, and nothing like what the real population shows.

### The distribution has no gap

1275 pairwise separations, single smooth unimodal hump, mean 3.07 m, sd 1.38 m:

```
   0.0- 0.5 m |                                                               3
   0.5- 1.0 m |######                                                        22
   1.0- 1.5 m |###########################                                   96
   1.5- 2.0 m |##################################################           177
   2.0- 2.5 m |############################################################ 212
   2.5- 3.0 m |##########################################################   205
   3.0- 3.5 m |###########################################                  154
   3.5- 4.0 m |##################################                           122
   4.0- 4.5 m |###########################                                   97
   4.5- 5.0 m |#################                                             63
   5.0- 5.5 m |#############                                                 46
   5.5- 6.0 m |#######                                                       28
   6.0- 6.5 m |#####                                                         18
   6.5- 7.0 m |#####                                                         19
   7.0- 7.5 m |#                                                              5
   7.5- 8.0 m |#                                                              5
   8.0- 8.5 m |                                                               1
   8.5- 9.0 m |                                                               2
```

Largest gap anywhere in the sorted list: **0.48 m**, out in the tail. A population
containing K distinct lines would show a gap of order the between-line separation.

### The world record is not an outlier — it is central

* Mean distance to all other runs: population mean 3.07 m (sd 0.63). **WR: 2.57 m, z = −0.78,
  rank 10 of 51 by centrality.** The WR is *more* typical than the median run.
* Nearest neighbour of the WR: `05_19556` at **0.70 m** RMS — a run 18 ms slower is
  essentially on top of it.
* WR's lateral offset at the checkpoints, versus the whole field's spread there:
  CP1 sd 2.13 m, **CP2 sd 0.71 m**, CP3 sd 3.34 m, finish sd 4.22 m. The entire field —
  world record to position 10 000 — threads CP2 within about a metre of each other.
* Most separated pair in the entire population: `p00004_19556` ↔ `slow_p10000_19812` at
  8.83 m RMS, and most of that is after CP3: restricted to start→CP3 the population's
  pairwise mean drops to 2.41 m and its **max to 6.89 m**.

### Where the field diverges (sd of lateral offset across the 51 runs)

```
  sd (m) vs distance along lap, 0 .. 1820 m; max sd 4.25 m
    4.2 |                                                                                                    |
    3.8 |                                                                     #                         #####|
    3.4 |                                                                  ########              ############|
    3.0 |                                                                ####################################|
    2.5 |                                                               #####################################|
    2.1 |                          #                                   ######################################|
    1.7 |                      ######                                 #######################################|
    1.3 |   ##             ###########        ###                    ########################################|
    0.8 |  ####        ################  ##  #######      ####  #   #########################################|
    0.4 | ###################################################################################################|
    0.0 |####################################################################################################|
                                  CP1                            CP2                 CP3                   F
```

Spread is near zero for the first third, ~2 m around CP1, collapses to 0.7 m at CP2, and
only opens up (3–4 m) after CP3 on the run to the finish — i.e. the divergence is
concentrated in the part of the lap where it costs the least.

### Overhead view — all 51 runs (`W` = WR drawn first, `.` = the other 50)

The other 50 runs almost never occupy a cell the WR did not already occupy:

```
+------------------------------------------------------------------------------------------------+ x:[1594..974] z:[632..1346]  (up = +z, left = +x)
|                           C  ...WWWWWW......                                                   |
|                        .WWWWWWWW......WWWWWWWW...                                              |
|                    .WWWW.                    .WWWW..                                           |
|                WWWWW.                           ..WWW..                                        |
|          .WWWWW..                                  .WWW..                                      |
|       .WWW.                                          .WW..                                     |
|    .WWW                                                WW..                                    |
|   .WW                                                  .WW.                                    |
|  WW                                                     .W.                                    |
| .W                                                      .W.                                    |
| .W                                                      .W.                                    |
| .W                                                      .W..                                   |
| .WW                                                     .W..                                   |
|  .WW                                                    .WC.                                   |
|   .WWW                                                  .W..                                   |
|      WWW..                                              .W.                                    |
|         WWWWWW..                                        .W.                                    |
|               WWWWWW                                    .W.                                    |
|                    .WWWW                                .W.                                    |
|                        WWWWWW.                          ..W                                    |
|                            ..WWWWWWWWWCWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW..              |
|                                                         ..W                    WWWWWW.         |
|                                                         ..W                         WWW.       |
|                                                         ..W                            WWW     |
|                                                         ..W                              WW    |
|                                                         ..W                               W.   |
|                                                         ..W                               WW.  |
|                                                         ..W                                W.  |
|                                                         ..W                                W.  |
|                                                         ..W                                .W  |
|                                                         ..W                                 S  |
|                                                         ..W.                                   |
|                                                         ..W.                                   |
|                                                         ..W.                                   |
|                                                         ..W.                                   |
|                                                         ..F.                                   |
+------------------------------------------------------------------------------------------------+
```

### Speed vs distance along the lap (all 51 overlaid)

```
     472.5 +----------------------------------------------------------------------------------------------------+
           |                                                                   .......................  ...     |
           |                                                        ................            .............   |
           |                                               ..............                                   ..  |
           |                                  ..................                                             .. |
           |              ...          ..................                                                    ...|
           |        .....................                                                                     . |
           |     .....                                                                                          |
           |    .                                                                                               |
           |   ..                                                                                               |
           |   .                                                                                                |
           |  .                                                                                                 |
           |  .                                                                                                 |
           | ..                                                                                                 |
           |..                                                                                                  |
           |.                                                                                                   |
           |.                                                                                                   |
           |.                                                                                                   |
           |.                                                                                                   |
       0.8 +----------------------------------------------------------------------------------------------------+  km/h
             0                                                                                             1820  m along lap
```

A single monotone acceleration curve for the whole field. WR vs field median:

| s (m) | 0 | 164 | 329 | 493 | 657 | 822 | 992 | 1156 | 1321 | 1485 | 1649 | 1820 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| WR | 0.8 | 304.4 | 330.5 | 324.4 | 358.9 | 370.8 | 404.1 | 426.9 | 447.8 | 471.0 | 446.7 | 440.5 |
| field median | 0.8 | 304.7 | 330.9 | 323.7 | 358.7 | 366.4 | 400.9 | 424.2 | 445.0 | 467.7 | 444.1 | 436.6 |
| **WR − median** | +0.0 | −0.3 | −0.4 | +0.7 | +0.3 | **+4.4** | **+3.2** | **+2.7** | **+2.8** | **+3.3** | **+2.6** | **+3.9** |

The WR is identical to everyone else for the first ~800 m and then simply carries 3–4 km/h
more from mid-lap to the flag. That is the entire difference.

### Where the time goes

WR sectors: S1 7617, S2 5691, S3 3008, S4 3222 ms. Field deficit versus the WR:

| sector | median | worst |
|---|---|---|
| S1 start→CP1 | +10 ms | +39 ms |
| **S2 CP1→CP2** | **+59 ms** | **+171 ms** |
| S3 CP2→CP3 | +20 ms | +158 ms |
| S4 CP3→finish | +19 ms | +62 ms |

Two thirds of the field's total loss is in S2 — which is also the sector where the lateral
spread is *smallest*. Same geometry, different carried speed.

---

## 4. What this means for the search

* **The human population contains no line diversity to seed from.** Max geometric
  separation in 51 runs spanning the whole leaderboard is 8.8 m RMS (6.9 m before CP3),
  with a smooth unimodal distribution and no gap. Your search converging on lines that
  cross CP1 within a metre of each other is *not* evidence of a stuck search — it is what
  every human does too (field sd at CP1: 2.1 m; at CP2: 0.7 m).
* **The WR is not hiding a different line.** It is a central member of the cloud that
  carries 3–4 km/h more speed from mid-lap onward.
* If you still want diverse seeds, the population's own extremes are the best available:
  `p00004_19556` and `slow_p10000_19812` are the most separated pair (8.83 m RMS; 16.3 m
  peak lateral). But 8.8 m on a 32 m-wide track is a nudge, not an alternative line — I
  would treat "the map admits a materially different line" as **unsupported by this
  evidence**, and look for the remaining time in speed/inputs (S2 especially) rather than
  in geometry. Note also that you now have **per-sample steer/gas/brake bytes** for all 51
  human runs, at 50 ms — that is a much more direct thing to seed an input search from
  than a path.

### A caveat on `lines.py`

`lines.py` is correct in concept and its `--demo` reproduces the claimed 11.2 m / 0.8 m
separation. But its `rms_separation` compares station *k* of A with station *k* of B, where
stations are at the same **fraction** of each run's own total arc length. Real runs here
differ in total path length by up to **73 m** (1814–1887 m), so fraction-matching displaces
stations *along* the track by tens of metres, and that longitudinal mismatch is counted as
separation. That is why it reports 30 lines at eps=2 where the projection method reports 18,
and why `--dtw` (which matches elastically) drops it to 24. The conclusion is the same
either way — the between-cluster separation is ~1 m in every variant — but for absolute
lateral numbers use the projection method.

---

## 5. Deliverables

| Path | Contents |
|---|---|
| `~/persistent/private-30d/tm-entrec/entrec.py` | decoder + `--selftest` (all validation above) + `--fields` |
| `~/persistent/private-30d/tm-entrec/cluster_lines.py` | arc-length resample, lateral projection, clustering, ASCII plots |
| `~/persistent/private-30d/tm-entrec/paths/*.json` | 51 runs, `{"name","time_ms","checkpoints_ms","samples":[{t,x,y,z,speed,gear,yaw}]}` |
| `~/persistent/private-30d/tm-entrec/csv/*.csv` | same runs, full field set (29 columns incl. steer/gas/brake/quaternion) |
| `~/persistent/private-30d/tm-entrec/reports/` | this report, the raw selftest, cluster and analysis output |

Usage:

```bash
python3 entrec.py --selftest
python3 entrec.py GHOST.Ghost.Gbx --csv out.csv --json out.json
python3 cluster_lines.py --dir paths/ --eps 1 2 5 --ref p00001_19538 --out clusters.json
```
