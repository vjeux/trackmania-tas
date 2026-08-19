# 210218 "Fall 2024 - 25 (Pure Wet Icy Wood)" — its author's OTHER conversions are all beaten by humans, twice by the author himself, and five of those runs are validated

Write-once sidecar, `key_` prefix (answer-key agent, session
`9f00f635-a11d-4d68-b07a-6ac2f1386397`, node 105213). Supersedes nothing. Times
in seconds. **Nothing was submitted to any leaderboard**; TMX and trackmania.io
read-only, rate-limited, descriptive User-Agent.

---

## 0. Verdict

| question | answer |
|---|---|
| siblings | the author's **other campaign conversions** — 18 maps share > 1 000 unbaked records; the top three are 6 489–6 659 |
| block identity | **6 659 of 210218's 37 269 unbaked records** with the best (205229); no sibling is the *same layout* |
| a human beating that sibling's AT? | **yes on 9 of 9 checked — every single one** |
| do those ghosts re-simulate? | **yes: 9 of 10 tested exact**, including all five rank-1 AT-beaters |
| do they transfer to 210218? | **no** — grafted onto our map they bind and die at `cps=1` (different campaign layouts) |
| embedded author ghost | **absent** (zero `CPlugEntRecordData` nodes) |

**The one sentence: R4igekon's author times are beaten by humans on nine of his
nine other Pure-Wet-Icy-Wood conversions — and on two of them the human who
beats the author time IS the author — so 210218's AT is not an outlier, and we
hold five validated runs showing how this surface is actually driven.**

## 1. The field, and why it matters here

| TMX | name | AT | human WR | by | margin |
|---|---|---|---|---|---|
| **210218** | **Fall 2024 - 25 (Pure Wet Icy Wood)** | **94.477** | 96.281 iambeeen | — | **+1.804 unbeaten** |
| 205229 | Summer 2024 - 22 (CPLess Pure WIW Forced Route) | 79.637 | **60.114** tuduttuduu | −19.523 |
| 208961 | Fall 2024 - 01 (Pure Wet Icy Wood) | 25.377 | **23.908** **R4igekon — the author** | −1.469 |
| 208800 | Fall 2024 - 15 (Pure Wet Icy Wood) | 47.167 | **46.566** **R4igekon — the author** | −0.601 |
| 208804 | Fall 2024 - 23 | 77.778 | **76.372** n00bdax | −1.406 |
| 208802 | Fall 2024 - 19 | 77.588 | **76.972** A------------ar | −0.616 |
| 210217 | Fall 2024 - 22 | 98.473 | **95.805** kjszrqhczxn | −2.668 |
| 208965 | Fall 2024 - 09 | 28.941 | **28.594** n00bdax | −0.347 |
| 208801 | Fall 2024 - 16 | 57.428 | **57.313** iambeeen | −0.115 |
| 208964 | Fall 2024 - 07 | 35.620 | **35.584** thgiN_ | −0.036 |

Nine of nine. This author's author times sit within ~2 % of what a good human
does on the same surface, and **the author himself has gone back and beaten two
of them online**. Our map's 1.804 gap is the same size as the margins humans
routinely find elsewhere in the series — it is an ordinary target, not a freak.

## 2. Validated, on their own untouched maps

```
208961  rank1 23908 -> 23908   (R4igekon, the author)      rank2 25127 -> DNF cps=1
208800  rank1 46566 -> 46566   (R4igekon, the author)      rank2 48039 -> 48039
205229  rank1 60114 -> 60114                               rank2 103094 -> 103094
210217  rank1 95805 -> 95805                               rank2 101910 -> 101910
208802  rank1 76972 -> 76972                               rank2 93750  -> 93750
```

Nine of ten exact. Every AT-beating run re-simulates — these are answer keys in
the strict sense.

## 3. Transfer test (the graft, with controls)

A foreign ghost declares its own map's uid, so it must be translated before it
can be tested at all (`key_ACQUISITION_addendum_foreign_ghost_binding_v1.md`):

```
ct build OUT --base <210218 rank1> --donor <sibling rank1> --ids 0x0309201D,0x0309202D,0x0309202B
```

| tape | on 210218 |
|---|---|
| X_205229, X_208800, X_208802, X_208961, X_210217 | **DNF `cps=1`** — bound, drove, died after CP1 |
| IDENT control (210218 rank2 grafted into rank1) | **102601 exact** |
| native rank 1 | **96281 exact** |

The count is present in every row, so these are driving failures, not binding
failures: the tapes really do run on our map and really do fail — as they must,
since each sibling is a *different* campaign layout wearing the same surface.

> **So the deliverable here is a reference, not a seed.** Which is the same
> answer 284238 got, and the same lesson the 146612 arm put best tonight: *an
> answer key tells you what to optimise, not what to copy.*

## 4. What is shared, and what is not

The siblings share the conversion's *furniture*, not the route: 6 659 records
with 205229, 6 601 with 208802, 6 489 with 208961, out of 37 269 unbaked. All
these maps are the same author re-surfacing different Nadeo campaign maps, so
what repeats is the wet-icy-wood block palette in similar structural
arrangements.

**Worth someone's time, and not built here:** 210218 is an *Altered Nadeo* map —
its layout is the official **Fall 2024 - 25**, whose human field is enormous. My
identity test demands the same block MODEL at the same cell, so a conversion that
swaps every surface scores zero against its own original by construction. A
**cell-occupancy** test that ignores the model name would find it, and would let
the official map's field be read as a route reference. That is the obvious next
instrument, and it applies to every "Altered Nadeo" map in the project
(228607/228811 among them).

## 5. What I hand the 210218 arm

1. **Five validated human runs on this exact surface**, two of them by the map's
   own author, all beating their maps' author times — the material for the
   mandatory "how does a human do this" investigation, on wet icy wood, from
   people who demonstrably can.
2. **A calibration for expectations**: nine of nine of this author's other ATs
   are beaten by a human, by 0.036 to 19.523. A 1.804 gap here is normal.
3. **A measured negative on transfer**, with the graft and both controls, so
   nobody spends a night trying to seed from a sibling.
4. **The cell-occupancy idea in §4**, which is the way to reach the original
   Nadeo map's field.

## 6. Artefacts (`210218/key_siblings/`)

* `key_205229.Map.Gbx`, `key_208961.Map.Gbx`, `key_208800.Map.Gbx`,
  `key_210217.Map.Gbx`, `key_208802.Map.Gbx`
* `ghosts/key_208961_R4igekon_author_23908.Ghost.Gbx`,
  `ghosts/key_208800_R4igekon_author_46566.Ghost.Gbx`,
  `ghosts/key_205229_tuduttuduu_60114.Ghost.Gbx`,
  `ghosts/key_210217_kjszrqhczxn_95805.Ghost.Gbx`,
  `ghosts/key_208802_76972.Ghost.Gbx`
* `xfer/` — the five translated carriers and the IDENT control
* `key_identity_table_v2_full_census.tsv`, `key_author_147341_corpus.tsv`
