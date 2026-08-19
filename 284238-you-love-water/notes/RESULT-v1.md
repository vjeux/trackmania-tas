# 284238 — "YOU LOVE WATER" — result: **real map, AT not reached, and here is exactly why**

Map uid `65bbJpDhtacfnZLT4E9YGGpRhMg` · MapId 284238 · author **Eating My Wings**
(Praha) · uploaded 2025-12-19 · tags **Water / LOL / Transitional**.
AT **50459** · the single human record **440238** (brick555) · gap **389 779 ms**
(8.7×) · **1 record on the leaderboard**.

Verdict in one line: **the map is healthy and the AT is plausible, but the only
human record is 8.7× off because it contains 31 respawns, and after removing
every removable failure the best tape this session could validate is 97 898 ms —
1.94× the author time. Closing that with the search toolchain is not a
milliseconds problem, it is a route-construction problem, and this toolchain has
never solved one.**

Everything below is measured on this box today; every number that names a
millisecond has been through the plain oracle
(`tmtas validate --map map.Map.Gbx …`).

---

## 1. §9 — the author's embedded ghost: **ABSENT** (verified, with a positive control)

```
tmtraj decode map.Map.Gbx
FAIL map.Map.Gbx: CPlugEntRecordData (0x0911F000) chunk not found
```

Per §9a this was not taken at face value. The LZO body was decompressed and
scanned for class ids as raw bytes:

| class id | 284238 | 228607 (positive control) |
|---|---|---|
| `0x0911F000` CPlugEntRecordData | **0** | **2** |
| `0x0309201D` ghost inputs | 0 | 0 |
| `0x0303F005` | 0 | 0 |

(2 215 156 bytes of decompressed body for 284238, 1 694 918 for the control;
the control also decodes: 406 samples, 20 290 ms.)

So: header says `validated="1"`, and **there is no ghost in the file.**
Medals are the standard Nadeo chain off the AT (gold ×1.070, silver ×1.209,
bronze ×1.506 — the same ratios as 285268 and 267460), so the AT is at least
internally consistent with a real validation; it just did not leave a ghost.

**Build note (per §8b / the 134672 finding): this map was SAVED ON A RECENT
BUILD** — `exever="3.3.0" exebuild="2025-07-04_14_15"` — and still carries no
ghost. That is a counter-example to "the saving build is the discriminator":
recent save, `validated="1"`, no ghost.

## 2. §8 — field reproduction: **PASS, 1/1, exact**

```
rank00001_440238.Ghost.Gbx    sim_time 440238
```

The one record re-simulates to the millisecond. Its build is
**2026-02-02_17_51 (git128149)** — the same build that fails half the time on
203072, and here it is exact, which is one more piece of evidence that 203072's
trouble is a property of that map and not of that build.

The map is therefore **not** a broken-physics curiosity: oracle and leaderboard
agree, and any time produced here would be falsifiable.

## 3. What the 440-second record actually is: 31 respawns

Decoded telemetry: 8805 samples, 23 733 m driven, 43 % airborne, max 351 km/h,
bbox x 424–1062, y 1596–1953, z 478–1075.

**31 position discontinuities** — respawns — split the run into attempts.
Declared splits are 6797 / 24213 / 76228 / 184638 / 440238, but the *last
(successful) attempt* in each sector is what the driving actually costs:

| sector | last-attempt time | what the rest of the sector was |
|---|---|---|
| start → CP1 | 6 797 | clean, one attempt |
| CP1 → CP2 | 13 163 | 1 respawn |
| CP2 → CP3 | 24 428 | 1 respawn |
| CP3 → CP4 | 25 788 | 9 respawns |
| CP4 → finish | 23 738 | 20 respawns |
| **total** | **93 914** | 31 respawns = 346 s of retries |

So the human's *clean-equivalent* pace is ~94 s, not 440 s. The 8.7× headline is
retry cost. The real gap is **94 s vs 50.459 s — 1.86×.**

## 4. NEW TECHNIQUE (transferable): respawn is `word0` bit 5, and respawn splicing is EXACT

`tmtas trace` prints only steer/accel/brake and throws the event away. In the
input archive (`0x0309201D`) a respawn is **one packet with `word0 = 34`**
(mode 2 | bit 5), encoded as the literal `state_seg (0,1),(0,1),(0x80000002,34)`;
the following packet re-establishes `word0 = 2` because the "same as previous"
predictor keeps only `word0 & 0xF`.

A new binary `tmpk` (source banked in `tools/tmpk.rs`, add it to
`tmsearch/src/bin/`) prints these and rebuilds an archive from arbitrary packet
ranges:

```
tmpk stats|dump|raw|changes GHOST
tmpk cut GHOST --keep A:B[,C:D…] --out F
tmpk asm GHOST --ops "keep:A:B,resp:T,keep:C:D,…" --out F
```

Two measured properties, both exact to the millisecond:

1. **Cutting between two respawns at the same checkpoint is exact.**
   Removing 76 140 ms + 228 160 ms of failed attempts (everything between the
   first and last respawn at CP3 and at CP4) predicted 135 938 ms and the oracle
   returned **135 938**. Within one recorded run the per-CP respawn state is
   identical every time (see §6b for WHY, and for the limit of that): post-respawn
   samples at one CP differ only by the 50 ms telemetry phase (≤ 3 m at
   220 km/h).
2. **A synthesised respawn press right after a checkpoint is exact — at 3 of the
   4 CPs.** Inserting `resp` one tick after the CP crossing and resuming from the
   post-respawn state deleted the whole first failed attempt and validated
   exactly at CP2 (412 728), CP3 (433 848) and CP4 (436 588).
   **At CP1 it does not**: presses at 6840…10340 all DNF, presses at
   10440/10540/10840 are exact (`440248 − (11040 − X)`). Mechanism unknown — it
   is not a cooldown (the CP2 insert fires 47 ms after the crossing and is
   exact). **Treat an inserted respawn as needing per-case validation.**

Combining both: **`splice/clean_best.Ghost.Gbx`, validated at 97 898 ms** —
the human's own driving with every removable failure deleted, 4 respawns left,
342 s shorter than the record it came from. It is the seed any future attempt on
this map should start from.

## 5. What the author time requires

Budget from the tape above (97 898):

```
6 797  start→CP1 (clean)
3 743  CP1 flail that cannot be spliced out (see §4.2)
13 163 CP1→CP2
24 438 CP2→CP3      ← ~15 s of it is milling in a bowl at x 730-870, y 1760-1800
25 808 CP3→CP4      ← ~17 s of it is milling in a bowl at x 427-500,  y 1706-1740
23 728 CP4→finish   ← ~7 s of it is milling in a bowl at x 965-1030, y 1650-1665
   40  4 respawn ticks
```

Each sector has the same shape: **fall into a bowl → find the way out → hit a
booster → fly to the next checkpoint.** The traverses are fast (260–307 km/h off
the boosters at (807/840, 1712, 927) and (663/647, 1768, 799/827)); the bowls are
where 40 of the 98 seconds go.

Straight-line CP-to-CP is 268 / 538 / 539 / 540 / ~470 m ≈ 2.4 km, and the AT
over that is 47 m/s point-to-point — entirely consistent with "get every bowl
right first time". **The AT does not need an undiscovered route.** It needs four
clean traverses, and the field's one record contains none of them.

To beat 50 459 from the 97 898 tape you must find **−47 439 ms**, essentially all
of it by replacing 40 s of bowl-flailing with driving that does not exist in any
recorded run of this map.

## 6. Why this toolchain cannot do that here — three measurements

1. **The car model has no purchase on this map.** `tmtas carmodel` fitted on this
   map's telemetry explains **2.7 %** of yaw-rate variance (unmodelled yaw-rate
   RMS 3.68 rad/s). The same fit is **71.2 %** on 173636 (a road map) and
   **39.3 %** on 267460 (floating platforms). The car is rotated by loop and
   half-pipe geometry and by 43 % air time, not by steering. Every steering-based
   prior, corridor and predicate this project owns is calibrated on the wrong
   thing here.
2. **Evaluation is ~100× slower.** The tape is 99 s (9 941 ticks). A full search
   of it sustains **67 evals/s** on 160 workers, against ~6 000/s on 252289's
   3.9 s tape. Exhaustive single-tick enumeration — the operator that broke
   252289 open — is 9 941 × 255 ≈ 2.5 M candidates ≈ **10 hours** for ONE pass.
3. **The measured search yield.** A 25.9-minute search restricted to the final
   sector (ticks 7568–9940) moved the tape from **97 898 → 97 461 ms**: −437 ms,
   **0.45 %**, over 132 120 evaluations at 85 evals/s, and essentially flat after
   the first 5 minutes (97 600 at 3 min, 97 489 at 5 min, 97 461 at 26 min).
   Re-validated cold through the plain oracle on the real map: **97 461**.
   Local search polishes the line it is given; it does not delete a bowl.
   Extrapolated at that yield, the −47 439 ms needed is ~4 500 hours of the same
   search.

## 6a. The sector decomposition DOES work as an instrument (and one new trap)

Recommendation #2 below was tested, not just proposed. `tmmaps build` produces
five exact segment maps for this map (`exact=true` on all five), **and the
respawn-anchored prefix survives them**, because segment map *k* neutralises only
the waypoints at or after the cut — every checkpoint the prefix respawns to is
still a checkpoint.

Measured, all exact, on `map_seg4` (CP4 promoted to the finish):

| tape | predicted CP4 time | oracle |
|---|---|---|
| human record | 184 638 (its declared split) | **184 638** |
| `cut_exact` | 184 638 − 76 140 | **108 498** |
| `t2_cp2` | 184 638 − 27 520 + 10 | **157 128** |
| `t3_cp3` | 184 638 − 6 400 + 10 | **178 248** |
| `upto_cp4` (search template) | 74 108 | **74 108** |

So a sector is an independent search problem with an exactly reproducible start
state, and its time is readable to the millisecond. That is the right instrument
for a 99-second map, and it is now built and banked.

**NEW TRAP, worth adding to §11: a respawn press immediately after crossing the
finish voids the run.** `clean_best` (presses 10 ms after the CP4 gate) and
`t4_cp4` (52 ms after) both come back `DNF cps=3` on `map_seg4`, while the same
tapes are perfect on the real map and the human's own tape — which presses 3.7 s
after the same gate — validates. So when you combine segment maps with respawn
splicing, **the template must END at the cut gate** (`upto_cp4` does; that is why
it works). Threshold unmeasured between 52 ms and 3.7 s.

## 6b. CORRECTION TO §11: the soft-respawn state is YOUR CROSSING STATE, not a per-checkpoint constant

§11 says the respawn state is "deterministic and history-independent". The first
half is right; **the second half is wrong, and this map shows it.**

A sector-3 search on `map_seg4` moved the CP4 split from **74 108 → 74 013**
(−95 ms in 3.8 min before the job was killed; re-validated cold on the segment
map: 74 013). Grafting that optimised sector back into the full tape — same
respawn press tick, same sector-4 tail — gives `DNF cps=4`: all four checkpoints
collected, then the tail dies. The identity graft of the *unmodified* sector
through the very same tool returns **97 898**, so the graft itself is exact.

The only thing that changed is *how the car crossed CP4*. Therefore:

> **A soft respawn (`word0 = 34`) restores the state of YOUR OWN crossing of that
> checkpoint. Two runs that cross the same gate differently respawn to different
> states.**

Everything previously read as "canonical" is consistent with this: in a retry run
the driver crosses the checkpoint **once** and respawns to that one crossing N
times, so every restore in the recorded tape is identical — which is exactly why
respawn-to-respawn cutting is exact, and why inserting a press right after that
same crossing is exact.

Two consequences for anyone using §11's method:

1. **Cuts that preserve the crossing are exact; edits that change the crossing
   are not.** The `finish = base − deleted` identity is an acceptance test for
   the first kind only.
2. **Respawn-anchored sectors are NOT independent.** Optimising sector *k*
   changes the start state of sector *k+1*, so sectors must be done
   left-to-right with everything downstream re-optimised — the usual TAS
   sequencing cost, on top of an already 100×-slow evaluation.

It is NOT the cross-container tape-portability defect found on 286279 (an input
archive moved into a different ghost file DNFs at CP1): every splice here is
single-container — `tmpk` rebuilds this record's own archive inside its own file,
and the one graft that crosses files (`from:`) was between two tapes both derived
from that same container, one of which validates at 97 898.

The re-trigger hypothesis is also **dead**: the car comes within 40 m of the CP1
gate exactly once (6100–7600 ms, closest approach 6.7 m at 6850), so there is no
second crossing to restore a different state from.

What is left is one data point: the presses that work (10 440 / 10 540 / 10 840)
are the ones in the **600 ms immediately before the human's own press at 11 040**,
and every press from +43 ms to +3.5 s after the crossing fails. Unexplained.

## 7. Verdict and recommendation



**Not a target for this toolchain in a session, and the reason is specific and
falsifiable**: the map is fine, the oracle is fine, the AT is fine — the *field*
is one 8.7×-off record, so the seed is 1.94× the target and the missing 47 s is
route construction, not optimisation. This is the cold-start problem
(`tm2020-coldstart.md`: 0 verified finishes from geometry) wearing a new hat, on
a surface where the fitted car model explains 2.7 % of the rotation.

What would change the answer, in order of value:

1. **A second human record.** One more finisher who does not flail in the same
   three bowls would supply the missing traverses directly. There is exactly one
   record today; the map is 8 months old.
2. **Left-to-right segment-map sector search anchored on the respawn states.**
   The instrument is built and proven exact (§6a), but §6b removes the free
   parallelism: sector *k+1* must be re-searched after sector *k* changes,
   because the respawn carries the crossing state. Four sectors × ~26 min per
   0.4 % is the wrong shape of budget for a 47 s deficit; it needs an operator
   that changes the *route*, not the line.
3. Fixing the CP1 inserted-respawn anomaly (worth 3.7 s of the 47).

Even with all three, the arithmetic is a photo finish: the conceivable ceiling of
removing every bowl is ~45 s of savings against 47.4 s needed. **I would spend
the next slot on a different map rather than on this one.**

## 7a. Best validated tapes

| tape | oracle (plain, cold) | note |
|---|---|---|
| human record | 440 238 | 31 respawns |
| `cut_exact` | 135 938 | respawn-to-respawn cuts only, arithmetic exact |
| `clean_best` | 97 898 | + synthesised presses at CP2/CP3/CP4 |
| **`best_97461`** | **97 461** | + 26 min of sector-4 search — **best tape this map has** |
| `upto_cp4` | 74 108 (on `map_seg4`) | search template for sector 3 |
| `best_74013` | 74 013 (on `map_seg4`) | −95 ms sector-3 gain, **does not compose** (§6b) |

Author time **50 459**. Best validated run **97 461** = **1.93× the AT**. Not beaten.

## 8. Artefacts (`~/persistent/private-30d/tm-unbeaten/284238/`)

| file | what |
|---|---|
| `map.Map.Gbx` | the Nadeo-served map (sha256 `ecbeca11…f60d8`, 1 669 261 B) |
| `ghosts/rank00001_440238.Ghost.Gbx` | the single human record (verified complete before use) |
| `wr.csv` | its decoded telemetry, 8805 samples |
| `splice/cut_exact.Ghost.Gbx` | **validated 135 938** — respawn-to-respawn cuts only |
| `splice/clean_best.Ghost.Gbx` | **validated 97 898** — the best tape this map has |
| `best_s4/` | the 25-minute sector-4 search's best tapes (97 585, re-validated) |
| `listall.txt` | every block and item with world positions (new `tmmaps listall`) |
| `carmodel_284238.json` | the 2.7 %-variance car-model fit |
| `tools/tmpk.rs`, `tools/anlz.rs` | the respawn surgery binary and the attempt-splitter |
| `RECON-v1.md` | an earlier agent's recon of the same map, found in place at 19:26 |

**Nothing here was submitted anywhere.** No Nadeo leaderboard interaction of any
kind; the only network calls were the documented read-only trackmania.io and
Nadeo CDN fetches, rate-limited, with a research User-Agent.
