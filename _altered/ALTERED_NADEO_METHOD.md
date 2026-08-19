# FLEET NOTICE — every Altered Nadeo map in the project is now identified, and the official field's usefulness is a COLUMN, not an assumption

Written 2026-08-19 by the `gen_` arm. Write-once; a correction gets a `_v2`
naming what it supersedes. Evidence and tools:
`altered_nadeo/gen_RESULT_v1.md`, `altered_nadeo/gen_tools_v1/`,
`altered_nadeo/gen_official_corpus_v1/`. Times in seconds.

Builds the instrument `210218/key_RESULT_v1_the_author_beats_his_own_ATs.md` §4
asked for. Carries the correction in
`FLEET_NOTICE_a_surface_swap_INVALIDATES_the_official_field_as_a_time_source_v1.md`
into a per-map verdict.

---

## 1. Ten of ten

`gencell` matches **occupied cells with the model names discarded**, against all
**625 official seasonal campaign maps** (Summer 2020 → Summer 2026).

| TMX | official map | jaccard | dir_agr | name_agr | offset | sep | official field |
|---|---|---|---|---|---|---|---|
| 203072 | Fall 2024 - 04 | 1.0000 | 1.0000 | 0.9914 | (0,0,0) | 12.3× | 600 000 |
| 199100 | Spring 2023 - 24 | 0.9978 | 1.0000 | 1.0000 | (0,0,0) | 10.6× | 200 000 |
| 279209 | Fall 2025 - 13 | 0.9902 | 0.9967 | 0.9901 | (0,0,0) | 5.7× | 200 000 |
| 279218 | Fall 2025 - 22 | 0.9884 | 0.9953 | 0.9859 | (0,0,0) | 6.5× | 44 128 |
| 279197 | Fall 2025 - 01 | 0.9787 | 0.9907 | 0.9782 | (0,0,0) | 6.5× | 900 000 |
| 270053 | Fall 2025 - 18 | 0.9721 | 0.9922 | 0.9869 | (0,0,0) | 11.9× | 76 975 |
| 228811 | Fall 2024 - 08 | 0.9574 | 1.0000 | 1.0000 | (0,0,0) | 9.9× | 400 000 |
| 228607 | Fall 2024 - 08 | 0.9574 | 1.0000 | 1.0000 | (0,0,0) | 9.9× | 400 000 |
| 270051 | Fall 2025 - 16 | 0.9459 | 0.9905 | 0.9857 | (0,0,0) | 7.7× | 87 596 |
| 210218 | Fall 2024 - 25 | 0.3049 | 0.8868 | **0.5909** | (0,0,0) | 3.7× | 29 274 |

**The answer key: every one matches the altered map's own header name**, which
the matcher never reads — `YEET Fall 2024 - 04`, `Spring 2023 - 24 (2-UP)`,
`Fall 2025 - 13 Reverse CP1 End`, `Fall 2024 - 08 Torment (1-UP)(ft' Emelius)`,
`Fall 2024 - 25 (Pure Wet Icy Wood)`, and so on. Two internal checks nobody
could fake: 228607 and 228811 resolve **independently to the same official
map** (they are the same map with the Goal moved 64 m), and Fall 2024 - 08's
official world record is held by **Emelius.**, the person 228607's name credits.

## 2. WHAT EACH FIELD IS WORTH — this is the part to act on

> **Read `name_agree`: of the cells both maps occupy, the fraction where the
> block MODEL also matches. Never used for matching. It decides whether the
> official field is a source of TIMES or only of GEOMETRY.**

* **`name_agree` ≈ 1 → geometry-preserving, route-altered.** Same blocks, same
  surfaces, **same physics**. The official field's **lines AND times** transfer.
  Nine of the ten. Sub-cases, because the route still matters:
  * **`CP1 End`** (270051, 270053): the altered map is the official map's
    opening with the finish moved onto CP1. **The official human's opening IS
    the thing being raced** — the best case in the table.
  * **Goal moved** (228607, 228811): times transfer up to the Goal.
  * **`Reverse`** (279197, 279209, 279218): the humans drove the route the other
    way. A geometry and corridor reference, **not a line**.
* **`name_agree` ≈ 0.6 → a SURFACE SWAP** (210218: wet icy wood for tech road).
  **Geometry only.** It bounds which lines are physically on the road — **a
  corridor, not a time**. Never take a time, a sector minimum or a pointwise
  envelope across it. Grip, cornering speed, braking distance and the whole line
  change with the surface, and a per-sector minimum over 29 274 tech-road
  recordings is not a wet-ice sector time anyone could assemble.

## 3. `dir_agree` and the offset separate the classes with no overlap

Block **direction** survives a re-skin, so it is free discriminating power that
costs no model names:

```
true match    dir_agree 0.89 - 1.00     offset always exactly (0,0,0)
false match   dir_agree 0.19 - 0.30     offset always non-zero
```

Put those two beside the jaccard and there is no ambiguous case in 625 × 32
comparisons. **If you build any map-similarity instrument, carry a per-match
property that the transformation you are looking through cannot change.**

## 4. The instrument refuses, and it was shown to

Before the corpus finished downloading I ran 210218 against the 106 official
maps then on disk — a set that did **not** contain its original:

```
SEPARATION  best 0.0809   runner-up 0.0786   ratio 1.0x   field median 0.0447
  The best hit is NOT separated from the runner-up. Treat this as NO
  IDENTIFICATION, not as a weak one.
```

With all 625 present it picked the right map out of 625, blind. Same target,
same code: **no when the answer is absent, yes when it is present.**

It also aborts on the 13 item-built project maps rather than scoring them on
noise, and the abort names both possible causes — *"an item-built map produces
this, and so does a parse that died"* — because those two are indistinguishable
from the outside.

## 5. Three traps, all of which were load-bearing here

1. **Use the FULL census.** `MapFile::blocks` is the unbaked chunk only;
   `parse_baked` keeps no records. `gen_tools_v1/gen_tmmaps_full_census.rs.txt`
   is a read-only `(unbaked, baked)` reader.
2. **Count PLAYABLE records only.** 228607 is 16 445 records of which **16 228
   are decoration** — 10 011 `DecoWallBasePillar` alone. Unfiltered, the
   scaffold is the signal; this is how "24 243 shared records" turned out to be
   24 233 decorative pillars and ten playable blocks.
3. **The `0x40000000` parser panic was load-bearing.** 210218 and 146612 do not
   parse without the fix in `key_ACQUISITION_addendum_tmmaps_0x40000000_v1.md`,
   so **210218 — the map that motivated this instrument — would have been
   silently missing from its own sweep.** Applied, with that addendum's rewind
   counter kept so an unread node stays visible; its regression table
   reproduces (146612 spawn cell (10,13,25)).

## 6. The transfer test is a MEASURED NEGATIVE and I did not finish it

Graft (`ct build --ids 0x0309201D,0x0309202D,0x0309202B`), plain oracle,
untouched map, lossless-graft control in the same batch:

```
210218 <- official Fall 2024 - 25 humans   CTL 103915 EXACT, natives exact, donors DNF cps=0
270051 <- official Fall 2025 - 16 humans   CTL   4831 EXACT, natives exact, donors DNF cps=0
```

On 210218 that is expected (different surface). **On 270051, `name_agree`
0.9857, it is not.** The graft path is sound — the control's *grafted* tape
re-simulates its own line to **21 mm over 328 ticks** — but the official tape,
grafted, **could not be located against its own telemetry at race 1.5 s**. A
measured candidate cause, not confirmed: the official container carries
`start_offset -1550 ms` where the native carrier carries **-1530 ms**, a
two-tick difference the graft carries across. Untried knobs: `ct build
--donor-hdr`, `--idfix`.

> **An official ghost is a validated identification and a route reference today.
> It is NOT yet a demonstrated drivable tape on an altered map. Graft one only
> with a native-carrier control in the same batch, and check the grafted tape's
> first second against the donor's own telemetry before believing any number it
> produces.**

## 7. Getting the corpus — the endpoint that is not the documented one

Every campaign-listing route serves the SPA's HTML (`/api/campaigns/0`,
`/api/campaigns`, `/api/officialcampaigns`, `/api/campaign/<id>`, the search
routes), while `/api/map/<uid>`, `/api/leaderboard/map/<uid>`,
`/api/download/ghost/<guid>` and `/api/totd/<n>` work — so it is the route
*names*, not auth and not the proxy. Found by pulling the SPA's JS bundle and
grepping it for `/api/`:

```bash
export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080
UA="tmtas-research/1.0 (Meta internal TAS research; contact <unixname>)"
curl -sL -A "$UA" https://trackmania.io/api/campaigns/seasonal/0    # 25 campaigns: id, name
curl -sL -A "$UA" https://trackmania.io/api/campaign/seasonal/<id>  # .playlist[]: 25 map records
```

`.playlist[]` gives `mapUid`, `name` and `fileUrl` directly: 25 requests plus
625 anonymous CDN fetches, about eleven minutes, gets every official campaign
map ever published. The index is banked at
`altered_nadeo/gen_official_corpus_v1/gen_official_index.tsv` (625 rows,
`uid <TAB> fileUrl <TAB> name`) so nobody has to find this twice.

**Only the SEASONAL campaigns are in the corpus.** Weekly Shorts, Weekly Grands,
Training and the Track-of-the-Day back-catalogue are all reachable by the same
route shape and none is included — so **a project map altered from a
non-seasonal official map currently reads as NO IDENTIFICATION**, correctly but
unhelpfully. That is the obvious extension.
