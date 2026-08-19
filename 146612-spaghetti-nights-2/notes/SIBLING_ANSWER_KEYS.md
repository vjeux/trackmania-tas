# 146612 "Spaghetti Nights 2" — six siblings, five with a human INSIDE the author time, and the best one is 98.1 % identical

Write-once sidecar, `key_` prefix (answer-key agent, the sibling-map sweep).
Supersedes nothing; the 146612 arm's own `RESULTS-146612-*.md`,
`CORRECTION-146612-*`, `EXIT-DIAGNOSIS-146612-v1.md` and the gate-plane addendum
stand unchanged — this adds an outside reference none of them had. Times in
seconds. **Nothing was submitted to any leaderboard**; TMX and trackmania.io
were read-only, rate-limited, with a descriptive User-Agent.

---

## 0. Verdict

| question | answer |
|---|---|
| siblings found | **six**, the whole "Spaghetti" series by AmpelJoe10 (TMX 55041, 66 maps) |
| best block-identity fraction | **151734 "Spaghetti Nights 3" — 3 475 of 146612's 3 541 records identical (98.1 %)** |
| a human beating that map's author time? | **yes, on five of the six** |
| do those ghosts re-simulate? | **yes — 12 of 12 tested reproduce their millisecond exactly** |
| do they finish on 146612 itself? | **no — all 21 DNF** (measured, with 146612's own record as the control) |
| embedded author ghost | **25 `CPlugEntRecordData` nodes, no input archive** — watch-only, and still unidentified against the AT (§4) |

## 1. The siblings

| TMX | name | identical / 3 541 | AT | human WR | human − AT |
|---|---|---|---|---|---|
| **151734** | **Spaghetti Nights 3** | **3 475 (98.1 %)** | 39.840 | **39.555** mernama | **−0.285** |
| 164965 | Spaghetti Nights 4 | 2 952 (83.4 %) | 39.960 | **39.433** cremconnoisseur | **−0.527** |
| 151831 | Spaghetti 3 | 2 808 (79.3 %) | 44.910 | **44.824** Mareng. | −0.086 |
| 170690 | Spaghetti 4 | 2 778 (78.5 %) | 44.960 | **44.681** ShcrTM | −0.279 |
| 146199 | Spaghetti 2 | 2 759 (77.9 %) | 43.610 | **43.466** Gazorpalse. | −0.144 |
| 133353 | Spaghetti Nights | 2 705 (76.4 %) | 39.910 | **38.532** Spaghett37 | **−1.378** |
| 132380 | Spaghetti | 2 540 (71.7 %) | 44.960 | **43.610** Spaghett37 | −1.350 |
| — | *146612 (this map)* | — | **38.530** | 40.223 jujumasterr | **+1.693 (unbeaten)** |

Everything below rank 8 in the 66-map catalogue drops to ≤ 25 identical records,
i.e. editor boilerplate. Full table: `key_siblings/key_identity_table_v1.tsv`.

**Note the last two rows against each other.** 133353's human WR is **38.532**
and 146612's author time is **38.530** — the same number to 2 ms, on maps that
share 76 % of their block records. Whatever the author did here, a human named
Spaghett37 has driven its equivalent on the predecessor map.

## 2. Controls

* **Identity instrument**: 146612 vs itself = 3 541/3 541; 267460 vs 44 maps of
  its author = 0; 285885 vs 170 maps of its author = 0. It can say no.
* **PARSER — read this before trusting any earlier "empty census" on this map.**
  `tmmaps allblocks/allitems/list` **panicked** on 146612
  (`unhandled inline node class 0x40000000 at 263292`) and on 210218. An empty
  census from a panicking parser is indistinguishable from "this map is built
  from items", which is exactly the failure ACQUISITION §0.4 warns about. Cause
  and two-line fix are in `key_ACQUISITION_addendum_tmmaps_0x40000000_v1.md`
  (top level); with it, 146612 parses to 279 free + 2 601 placed + 661 items,
  every map that already parsed is unchanged to the record, and
  `tmmaps list` now returns this map's waypoints.
* **Oracle**: every ghost below was validated on my own server copy and my own
  staging root, with a known-answer control in the same batch.

## 3. The ghosts — validated, and what they do on 146612

**On their own untouched maps (this is the answer-key test):**

```
151734  rank1 39555 -> 39555     rank2 39814 -> 39814     rank3 41136 -> 41136
164965  rank1 39433 -> 39433     rank2 40205 -> 40205     rank3 40283 -> 40283
133353  rank1 38532 -> 38532     rank2 38925 -> 38925     rank3 39628 -> 39628
146612  rank1 40223 -> 40223     rank2 40226 -> 40226     rank3 41075 -> 41075   (control)
```

12 of 12 exact. These are answer keys in the strict sense: a human, beating that
map's author time, on geometry we share, re-simulating on our oracle.

**Transferred to the untouched 146612 map:** all 21 sibling tapes **DNF**;
146612's own rank 1 returns **40223** in the same batch. So the series cannot be
replayed across maps — the shared 98 % is not the whole route.

## 4. Where each sibling's line stops being about our map

New instrument (`key sectors`): turn both maps' record sets into occupied cells,
then walk the sibling human's decoded telemetry and report the first sample in a
cell where the two maps differ. Cell convention calibrated on a map that states
cells and world positions side by side: `cx = ⌊x/32⌋`, `cy = ⌊(y+64)/8⌋`,
`cz = ⌊z/32⌋`.

| sibling | first divergent cell | divergent samples |
|---|---|---|
| **151734 SN3** | **t = 8.650 s**, world (739.3, 18.0, 1008.9) | **119 / 792 (15 %)** |
| 133353 SN1 | t = 3.810 s, world (478.3, 39.9, 858.4) | 317 / 771 |
| 151831, 146199, 132380, 164965, 170690 | t ≈ 0 (their spawn cell already differs) | 509–668 / ~870 |

Honest caveat: the test flags a cell when **any** record differs there,
decoration included, so it is a conservative bound rather than a driving-relevant
one. It is still decisive as a ranking: **mernama's 39.555 on Spaghetti Nights 3
is a valid reference for 146612's first 8.65 s and for 85 % of the cells its
line touches.**

## 5. What I hand the 146612 arms

1. **A per-sector human reference that beats an author time on our geometry.**
   `key_siblings/ghosts/key_151734_mernama_39555.Ghost.Gbx` (validated 39.555,
   its own map's AT is 39.840). Sector-by-sector comparison against our best tape
   over the first 8.65 s is now a like-for-like comparison, not an analogy.
2. **The 38.532 coincidence** (§1). If our AT of 38.530 looks unreachable, note
   that the same author's earlier map has a *human* at that number over 76 %
   shared blocks: `key_133353_spaghett37_38532.Ghost.Gbx`, validated.
3. **What differs**, exactly: 66 records exist only on 146612 and 331 only on
   SN3 — mostly `PlatformDirt*` / `RoadTechBranch*` in cells x 17–37, y 6–15,
   z 15–32. The re-laid section is where our map is genuinely its own problem.
4. **The parser fix**, which also unblocks relocated-gate work on this map.

## 6. Embedded author ghost — 25 record nodes, none yet matched to the AT

`ct probe` counts **25** `CPlugEntRecordData` nodes and `ct mapghost` reports
`NO EMBEDDED GHOST` (no `CGameCtnGhost` blob, so no input archive: watch-only,
never re-simulatable). This reproduces the survey in
`ACQUISITION_addendum_embedded_author_ghost.md`, including its open item — the
first node ends at 24.400 against an AT of 38.530, so **the first node is not
the author's lap and the map is still unidentified**. `ct recghost` selects the
FIRST record node only; matching a node to the AT needs a node index argument.
Not built here — flagged as the cheapest open lever on this map's map file.

## 7. Artefacts (`146612/key_siblings/`)

* `key_151734.Map.Gbx`, `key_164965.Map.Gbx`, `key_133353.Map.Gbx`,
  `key_151831.Map.Gbx`, `key_146199.Map.Gbx`, `key_170690.Map.Gbx`
* `ghosts/key_151734_mernama_39555.Ghost.Gbx`,
  `ghosts/key_164965_cremconnoisseur_39433.Ghost.Gbx`,
  `ghosts/key_133353_spaghett37_38532.Ghost.Gbx`,
  `ghosts/key_151831_mareng_44824.Ghost.Gbx`,
  `ghosts/key_146199_gazorpalse_43466.Ghost.Gbx`
* `key_identity_table_v1.tsv` (all 66 maps), `key_author_55041_corpus.tsv`

Anything else re-fetches deterministically:
`https://trackmania.exchange/maps/download/<MapId>` (~1 req/1.5 s) and
`https://trackmania.io/api/leaderboard/map/<uid>` (~1 req/1.6 s), descriptive
User-Agent, never a browser UA.
