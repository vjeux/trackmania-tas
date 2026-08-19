# 153527 — the seed hunt: **no geometry reuse, but the dead-build diagnosis is REFUTED**

Written 2026-08-19 by the 153527 agent (node 145855), following the 284238
cold-start finding that its obstacle is reused byte-identically across four maps
by the same author. Write-once sidecar, prefix `route_`. Nothing here modifies
`RESULT.md` or `route_RESULT_v1.md`.

**Two results, and the second is the important one.**

1. **The seed-by-geometry-reuse route is closed for 153527.** Its author reused
   nothing. Reported with three controls, one of which reproduces the published
   284238 answer to the block.
2. **`RESULT.md` §2's explanation for why 153527's only record does not
   re-simulate is wrong.** A sibling ghost with the **byte-identical build
   string** and the **byte-identical corrupt `NbRespawns`** validates exactly.
   Neither variable explains 153527, so **the map is a live suspect again.**

---

## 1. The author and the sibling set

153527 is by **PokeuuuTM** (TMX user 146507). `authoruserid=146507` on the TMX v2
API returns **39 maps**; 11 are Race maps with a leaderboard, of which **8 carry
the same RPG and/or Pathfinding tags**. The closest sibling by every axis is
**152940 `Distingue - by Pokeuuu`**, same tags, uploaded **2024-02-02 — four days
before 153527**.

## 2. Every sibling has a live board, and on every one of them humans beat the author

```
   MapId  Name                                     AT  records    bestHuman    ratio
  153527  P-Found - Pokeuuu                   939.283        1     5661.335    6.03x  <- THIS MAP
  223218  Think Deeply - Pokeuuu              900.000        8      731.209    0.81x
  201931  Galaxy Light - GCUP1                 63.192       15       61.734    0.98x
  187799  AwPath - Babylone ft Pokeuuu        750.069       15      608.680    0.81x
  184482  CUTCUP S1#6                          72.461       15       24.166    0.33x
  184478  CUTCUP S1#4                          27.811       15       17.306    0.62x
  184477  CUTCUP S1#3                          43.618       15       21.959    0.50x
  184475  CUTCUP S1#2                          36.411       15       20.834    0.57x
  184473  CUTCUP S1#1                          45.324       15       26.088    0.58x
  170035  Rose Shaft - Pokeuuu                460.000        5      449.666    0.98x
  152940  Distingue - by Pokeuuu             1193.844        6      886.277    0.74x
  140614  Pathfinding - Legacy                558.117       13      433.228    0.78x
```

**On 11 of 11 siblings a human is FASTER than the author time**, by 2 % to 67 %.
On 153527 the single human is **6.03× slower**. This author's author times are
routinely beaten, including on a **1193.844 s** RPG/Pathfinding marathon
uploaded four days earlier and beaten by 25.8 %.

This is independent, field-side support for `route_RESULT_v1.md`: 939.283 is not
an implausible time for this author's maps. **153527's AT is unbeaten because
one person has played it once, not because it is extreme.**

## 3. Geometry reuse: **none**, and here is why you can believe the negative

Census over the block records: for every (model, dir) pair, vote on the integer
cell offset that would map a sibling's block onto one of 153527's, and take the
largest agreeing set. A reused module shows up as a large count under **one**
offset.

```
map               best  offset (dx,dy,dz)   of playable
152940              13          (3,9,6)          1.1%
170035               4      (-23,20,10)          0.0%
223218              11       (-13,16,9)          2.5%
140614              43     (16,-11,-17)          4.1%
187799              58        (0,21,-3)          0.3%
201931               4    (-24,-10,-17)          0.1%
184473..184482    5-10                          7-24%   (tiny maps, 29-69 playable blocks)
```

58 of 20 227 is noise. The palette overlap is a healthy 0.10–0.38 Jaccard —
they are the same author's RPG blocks — but **no module is placed the same way
twice.** The CUTCUP percentages look high only because those maps have 29–69
playable blocks in total; their absolute counts are 5–10.

### Three controls, because a negative without one is worth nothing

**A — self-match.** 153527 against itself: **735 / 735 = 100.0 % at offset
(0,0,0)**. The census can say yes.

**B — synthetic reuse.** 153527's blocks translated by a known (+7, +3, −11) and
fed back in: recovers exactly **(−7, −3, +11) at 100.0 %**. It finds a
translated module when one exists.

**C — the published answer key, in the other placement regime.** 284238 and its
siblings are **100 % free-placed** (positions in chunk `0x0304305F`), so the grid
census sees nothing at all on them — it returned 0, which was *my instrument
failing*, not a result. Repeating the census over free-block absolute positions:

```
reference 284238: 186 free blocks
map      blocks  matched   offset          of ref
279008      186      167   (0.00,0.00,0.00)  89.8%
```

**167 of 186 — the exact number the cold-start agent published independently.**
So the method detects real reuse when it exists, in the regime where it exists.

Applied to 153527's siblings in that same free regime: **best match 2 of 104.**

Placement regimes for the record: 153527 and every Pokeuuu sibling are
**99.5–100 % grid-placed** (49–203 free blocks each, all gates and springs);
284238's family is 100 % free-placed. Two different building styles, and the
census needs both, which is worth knowing before anyone runs it on a third map.

## 4. **The dead-build diagnosis for 153527 does not survive contact with the siblings**

`RESULT.md` §2 concludes that 153527's record fails because it is a dead-build
ghost (`2024-01-10_12_53 git=126731`, `NbRespawns = 4294967295`), citing 126859's
r22 with the same build and the same corrupt count. The remedy implied — "the map
is fine, seed from a recent ghost" — has nothing to work with here, which is what
made the map unseedable.

I ran every sibling's top ghost through the plain oracle on its own untouched map:

| map | ghost build | declared NbRespawns | result |
|---|---|---|---|
| 170035 | 2026-02-02 git=128149 | 5 | **exact 449.666** |
| 223218 | 2026-02-02 git=128149 | 9 | **exact 731.209** |
| **152940** | **2024-01-10 git=126731** | **4294967295** | **exact 886.277** (simulated NbRespawns 22) |
| 140614 | 2023-11-15 git=126529 | 4294967295 | `wrong simu` + hazard clause, DNF cps=0 |
| 187799 | 2024-04-30 git=127012 | 4294967295 | `wrong simu`, DNF cps=0 |
| **153527** | **2024-01-10 git=126731** | **4294967295** | **`wrong simu` + hazard, DNF cps=0** |

**152940 is the one that matters.** Same author, same genre, same era, an
886.277 s marathon with respawns, on the **byte-identical build string** and the
**byte-identical corrupt `NbRespawns = (u32)−1`** — and it validates to the
millisecond.

So on the evidence now available:

- **the build string does not determine the outcome** — the same build validates
  on 152940 and fails on 153527;
- **the corrupt `NbRespawns` does not determine it either** — 152940, 140614 and
  187799 all declare `4294967295`, and 152940 validates;
- **age is a tendency, not a rule** — old ghosts fail 2 of 3 here, recent ones
  pass 2 of 2, but the exception is precisely the controlled comparison.

**`RESULT.md` §2's mechanism is therefore refuted, and its conclusion "the map is
probably healthy" loses its support.** I am *not* reinstating
`RESULT_v1_RETRACTED.md` — its reasoning (the hazard clause is a map-level
refusal) was independently wrong, and 152940 prints no hazard clause while
140614 does and both are old. But the question **"is 153527 itself simulable?"**
is open again, and nobody has ever obtained a `ValidatedResult` from that map.

### Controls on the oracle itself

The harness must be able to say both things, and it does, in the same session on
the same server build (`2026-05-15_18_00 git=128182`):

- **says yes**: 449.666 / 731.209 / 886.277, three exact times;
- **says no**: 153527's record → `DNF cps=0`, reproducing the previous agent's
  result exactly, plus two more DNFs.

Raw server output for all six is in `route_evidence_seed/rawval_*.txt`. Note the
earlier summary rows print the *declared* time on success, which on its own would
be an instrument that can only say yes — the raw `ValidatedResult` blocks are
what distinguish a real simulation from an echo, and they are what the table
above is read from.

## 5. What this means for seeding 153527

**Closed:** no sibling shares geometry, so there is no transplantable module and
no "same obstacle, different map" answer key of the kind 284238 got.

**Still closed, but for a different reason than we thought:** the map has no
working tape. It is not that the only tape is from a dead build — that build
works elsewhere. Something about **this ghost or this map** defeats the
simulator.

**The one experiment I would run next, and did not have the lease for:**
`RESULT.md` §1 and §2 never established that 153527 can produce a
`ValidatedResult` at all. §12's remedy applies exactly — relocate 153527's finish
gates onto the spawn (`0x0304305F` is a pure float rewrite, the safest surgery
available) and try the only tape that exists. If the record still says
`wrong simu` when the finish is two seconds away, the defect is in the ghost;
if it suddenly returns a time, the map is fine and the record's later stretch is
what diverges — and a segment-wise bisection of the tape becomes possible.
Either answer is worth more than another pass over the geometry.

Second, cheaper: 152940 is now a **working, same-author, same-genre,
886.277 s, 22-respawn positive control** for any tooling aimed at 153527. Nothing
on this project had one before.

## Files

```
route_SEED_v1.md                     this file
route_evidence_seed/
  sibling_maps.tsv                   the 11 sibling maps (TMX id, uid, name)
  sibling_leaderboards.txt           AT / records / best human / ratio
  ghost_builds.txt                   GameBuild of each top ghost
  census_grid.txt                    grid census + controls A and B
  census_free.txt                    free census + control C (167/186)
  rawval_152940.txt                  THE decisive one: same build, same corrupt
                                     NbRespawns, ValidatedResult 886277
  rawval_153527.txt  rawval_140614.txt  rawval_187799.txt  rawval_170035.txt
  rawval_223218.txt
```
Sibling artefacts (map + validating ghost, sha256 in `route_siblings/route_SHA256SUMS.txt`):
`route_siblings/route_152940_distingue.Map.Gbx` + `route_152940_gawliet_886277.Ghost.Gbx`
(the dead-build ghost that VALIDATES) and `route_170035_roseshaft.Map.Gbx` +
`route_170035_449666.Ghost.Gbx` (a live-build control).

Tools: `../153527/route_tools/` (`route sig`, `route fsig`, `route tmxlist`) plus
two read-only binaries added to the tmmaps workspace, `blocklist` and `freelist`
— both refuse to write a map, because the block reader they use is made
fail-soft and must never touch the surgery path.
