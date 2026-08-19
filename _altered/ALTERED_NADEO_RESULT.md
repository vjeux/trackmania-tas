# Altered Nadeo maps: which official campaign map each one IS — ten of ten, by cell occupancy

Worked 2026-08-19 on node 34881 by the `gen_` arm (the 140614 contact-gain arm,
continuing). Prefix `gen_`. Write-once; a correction gets a `_v2` naming what it
supersedes. Times in seconds.

**No time claimed on any map, nothing searched, nothing submitted anywhere.**

> Builds the instrument `210218/key_RESULT_v1_the_author_beats_his_own_ATs.md`
> §4 specified and could not write:
>
> > *"210218 is an Altered Nadeo map — its layout is the official Fall 2024 - 25,
> > whose human field is enormous. My identity test demands the same block MODEL
> > at the same cell, so a conversion that swaps every surface scores zero
> > against its own original by construction. A cell-occupancy test that ignores
> > the model name would find it."*

---

## 0. Summary

| | |
|---|---|
| the instrument | `gencell` — Jaccard of **occupied-cell sets**, model names discarded; full census; playable-only; translation vote |
| the corpus | **all 625 official seasonal campaign maps**, Summer 2020 → Summer 2026, downloaded and banked by index |
| **the result** | **10 of 10 project maps identified**, separations 3.7×–12.3× |
| **the answer key** | every one matches the altered map's **own header name**, which the matcher never reads |
| the discriminator | **`dir_agree`** (block direction survives a re-skin): true 0.89–1.00, false 0.19–0.30, **no overlap** — and every true hit is at offset **(0,0,0)**, every false hit needs a non-zero one |
| the classification | **`name_agree`** splits *geometry-preserving* (times transfer) from *surface swap* (geometry only) |
| what it opens | official fields of **29 274 to 900 000** players on identical geometry |
| the honest limit | the graft transfer test is a **measured negative on 2 of 2 maps tried** and I did **not** isolate why — §6 |

---

## 1. The table

`--census full --playable on`, 625 official maps each time.

| TMX | official map | jaccard | dir_agr | **name_agr** | offset | sep | official field | class |
|---|---|---|---|---|---|---|---|---|
| 203072 | **Fall 2024 - 04** | 1.0000 | 1.0000 | 0.9914 | (0,0,0) | 12.3× | 600 000 | geometry-preserving |
| 199100 | **Spring 2023 - 24** | 0.9978 | 1.0000 | 1.0000 | (0,0,0) | 10.6× | 200 000 | geometry-preserving |
| 279209 | **Fall 2025 - 13** | 0.9902 | 0.9967 | 0.9901 | (0,0,0) | 5.7× | 200 000 | geometry-preserving, **route reversed** |
| 279218 | **Fall 2025 - 22** | 0.9884 | 0.9953 | 0.9859 | (0,0,0) | 6.5× | 44 128 | geometry-preserving, **route reversed** |
| 279197 | **Fall 2025 - 01** | 0.9787 | 0.9907 | 0.9782 | (0,0,0) | 6.5× | 900 000 | geometry-preserving, **route reversed** |
| 270053 | **Fall 2025 - 18** | 0.9721 | 0.9922 | 0.9869 | (0,0,0) | 11.9× | 76 975 | geometry-preserving, **CP1 End** |
| 228811 | **Fall 2024 - 08** | 0.9574 | 1.0000 | 1.0000 | (0,0,0) | 9.9× | 400 000 | geometry-preserving, **Goal moved** |
| 228607 | **Fall 2024 - 08** | 0.9574 | 1.0000 | 1.0000 | (0,0,0) | 9.9× | 400 000 | geometry-preserving, **Goal moved** |
| 270051 | **Fall 2025 - 16** | 0.9459 | 0.9905 | 0.9857 | (0,0,0) | 7.7× | 87 596 | geometry-preserving, **CP1 End** |
| 210218 | **Fall 2024 - 25** | 0.3049 | 0.8868 | **0.5909** | (0,0,0) | 3.7× | 29 274 | **SURFACE SWAP** |

### 1.1 The answer key — names the matcher never sees

| TMX | identified | the altered map's own header name |
|---|---|---|
| 203072 | Fall 2024 - 04 | `YEET Fall 2024 - 04` |
| 199100 | Spring 2023 - 24 | `Spring 2023 - 24 (2-UP)` |
| 279209 | Fall 2025 - 13 | `Fall 2025 - 13 Reverse CP1 End` |
| 279218 | Fall 2025 - 22 | `Fall 2025 - 22 Reverse CP1 End` |
| 279197 | Fall 2025 - 01 | `Fall 2025 - 01 Reverse CP1 End` |
| 270053 | Fall 2025 - 18 | `Fall 2025 - 18 CP1 End` |
| 228607 | Fall 2024 - 08 | `Fall 2024 - 08 Torment (1-UP)(ft' Emelius)` |
| 228811 | Fall 2024 - 08 | `Fall 2024 - 08 Torment (1-DOWN)` |
| 270051 | Fall 2025 - 16 | `Fall 2025 - 16 CP1 End` |
| 210218 | Fall 2024 - 25 | `Fall 2024 - 25 (Pure Wet Icy Wood)` |

**Ten for ten.** Two further internal checks that could not have been faked:
228607 and 228811 resolve **independently to the same official map**, which is
right — they are the same map with the Goal moved 64 m. And the official world
record on Fall 2024 - 08 is held by **Emelius.** (20.034), the person 228607's
own name credits.

## 2. Why the existing instruments could not do this

Every sibling test the project owns keys on `(cell, model name)`. An Altered
Nadeo map is the official layout with surfaces swapped, so it scores **zero
against its own original by construction**. Drop the name and the layout is
still there. `gencell` matches on cells alone and then *reports* name agreement
as a separate column, which is what makes §5's classification possible.

## 3. Controls — four, and the instrument refuses

**Identity.** A map against itself: **jaccard 1.0000, dir_agree 1.0000**.

**A known-identical pair.** 228607 ↔ 228811 (same map, Goal moved 64 m):
**0.9574, dir_agree 1.0000, offset (0,0,0)**.

**Unrelated maps.** The other project maps against 228607: **≤ 0.047**.

**THE REFUSAL, which is the one that matters.** Before the full corpus had
finished downloading I ran 210218 against the 106 official maps then on disk —
a set that did **not** contain its original. The tool printed:

```
 jaccard dir_agree   shared    cells           offset  map
  0.0809    0.2344      529     4705       (5,-11,-8)  cHy30Gfs2sYzRy56TL2F48Adqb5.Map.Gbx
  0.0786    0.2847      685     7037        (7,-5,-4)  2ophIF0vwNu53QbQ_tUTiOuFGZ2.Map.Gbx
SEPARATION  best 0.0809   runner-up 0.0786   ratio 1.0x   field median 0.0447
  The best hit is NOT separated from the runner-up. Treat this as NO
  IDENTIFICATION, not as a weak one.
```

With all 625 present it then picked the correct map **out of 625, blind**. An
instrument that says no on the same target when the answer is absent, and yes
when it is present, is pinned from both sides (ACQUISITION §0.4).

It also refuses on **13 item-built project maps** (165922, 284238, 249521,
197047, 145875, 252289, 267859, 267460, 274191, 191465, 227969, 285885 and one
more) with

```
ABORT: the target has only N occupied cells after filtering -- below --min-cells 50.
An item-built map produces this, and so does a parse that died; check the census
counts above before reading any score.
```

rather than scoring them on noise. **An empty census and a dead parse look
identical**, which is the trap `key_ACQUISITION_addendum_tmmaps_0x40000000_v1.md`
documents, so the abort names both causes.

## 4. Both census traps, and a parser fix that was load-bearing

**(a) The full census.** `MapFile::blocks` is the unbaked chunk only;
`parse_baked` exists for re-encoding and keeps no records. I added a read-only
`full_census()` returning `(unbaked, baked)`. Without the baked chunk most of
some maps' terrain is invisible.

**(b) Playable only.** 228607 is **16 445 records of which 16 228 are
decoration** — 10 011 `DecoWallBasePillar`, 2 304 `Grass`, 1 668
`DecoWallBaseVFC`. Unfiltered, that scaffold *is* the signal, and this is
exactly how one arm published "24 243 shared records" that were 24 233
decorative pillars and ten playable blocks
(`153527/key_RESULT_v3_CORRECTION_shared_records_are_decoration.md`). The
classifier is a stated keyword list applied to each map **in its own palette**,
before names are discarded; `--playable off` shows the raw figure.

**(c) The `0x40000000` panic was load-bearing here.** 210218 and 146612 do not
parse without the fix in
`key_ACQUISITION_addendum_tmmaps_0x40000000_v1.md`, so **210218 — the map that
motivated this whole instrument — would have been silently absent from its own
sweep.** I applied that addendum's rewind fix, kept its rewind counter so an
unread node stays visible rather than becoming a silent mis-parse, and re-ran
its regression table: 153527, 228607 and 267460 unchanged; 146612 spawn cell
**(10,13,25)**, exactly as published.

## 5. WHAT EACH FIELD IS GOOD FOR — read `name_agree`, not just `jaccard`

This section exists because the coordinator's first framing was over-broad and
the 210218 arm was right to refuse it
(`FLEET_NOTICE_a_surface_swap_INVALIDATES_the_official_field_as_a_time_source_v1.md`).

> **`name_agree` = of the cells both maps occupy, the fraction where the block
> MODEL also matches. It is never used for matching. Read it after a match, to
> decide what the official field is worth.**

* **`name_agree` ≈ 1 — geometry-preserving, route-altered** (CP1 End, Reverse,
  Goal moved). Same blocks, same surfaces, **same physics**: the official
  field's **lines *and times*** transfer. Nine of the ten. For a `CP1 End` map
  the official human's opening *is* the thing being raced — the best case in the
  table.
* **`name_agree` ≈ 0.6 — a SURFACE SWAP** (210218: wet icy wood for tech road).
  **Geometry only.** The field bounds which lines are physically on the road —
  **a corridor, not a time**. Never take a time, a sector minimum or a pointwise
  envelope across it: grip, cornering speed, braking distance and the whole line
  change with the surface, and a per-sector minimum of 29 274 tech-road
  recordings is not a wet-ice sector time anybody could assemble.

And within the geometry-preserving class the route matters: a **Reverse** map's
official field is a geometry and corridor reference, not a line, because the
humans drove the route the other way.

## 6. The transfer test — a measured negative I did NOT finish diagnosing

Grafting the donor's inputs into a carrier of the target map
(`ct build --ids 0x0309201D,0x0309202D,0x0309202B`), plain oracle, untouched
map, lossless-graft control in the same batch:

```
210218 <- official Fall 2024 - 25 humans (surface swap)
  CTL_r02_into_r01                        103915   <- rank2 inputs, rank1 carrier: EXACT
  nat210218_r01_96281                      96281   <- native
  X_off_rank001_86977 / 002 / 003        DNF cps=0  (count PRESENT: bound and simulated)

270051 "Fall 2025 - 16 CP1 End" <- official Fall 2025 - 16 humans (name_agree 0.9857)
  CTL_b_into_a                              4831   <- EXACT
  nat_a_4830                                4830
  X_off rank001..005                     DNF cps=0
```

On 210218 `cps=0` is expected — different surface, different physics. **On
270051 it is not**, and I am reporting it unresolved rather than explaining it
away. What is established:

* **The graft path is sound.** The lossless control's *grafted* tape
  re-simulates its own recorded line to **21 mm median over 328 ticks** through
  `fk rtraj`, and returns its own time to the millisecond.
* **The official tape, grafted, could not be located against its own telemetry
  at race 1.5 s** — `fk rtraj` aborted at three pause ticks (100, 200, 300).
* **A candidate cause, measured and not confirmed:** the official container
  carries `start_offset -1550 ms` where the native carrier carries **-1530 ms** —
  a **two-tick** difference that the graft carries across with the input chunk.
  Whether that shifts the tape relative to the countdown, or whether the tape
  genuinely diverges immediately, I could not separate.
* `ct build --donor-hdr` and `--idfix` are the untried knobs.

> **So: an official ghost is a validated identification and a route reference
> today. It is NOT yet a demonstrated drivable tape on an altered map, and
> anybody who grafts one must carry a native-carrier control in the same batch
> and check the grafted tape's first second against the donor's own telemetry
> before believing any number it produces.**

## 7. Getting the corpus — the endpoint nobody could find

Every documented campaign-listing route silently serves the SPA's HTML through
the proxy (`/api/campaigns/0`, `/api/campaigns`, `/api/officialcampaigns`,
`/api/campaign/<id>`, the search routes). `/api/map/<uid>`,
`/api/leaderboard/map/<uid>`, `/api/download/ghost/<guid>` and `/api/totd/<n>`
all work, so it is the route *names*, not auth or the proxy. The working pair,
found by pulling the SPA's JS bundle and grepping it for `/api/`
(`this.$route.params.club == "seasonal"` is the giveaway):

```bash
export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080
UA="tmtas-research/1.0 (Meta internal TAS research; contact <unixname>)"
curl -sL -A "$UA" https://trackmania.io/api/campaigns/seasonal/0    # 25 campaigns: id, name
curl -sL -A "$UA" https://trackmania.io/api/campaign/seasonal/<id>  # .playlist[]: 25 map records
```

`.playlist[]` carries `mapUid`, `name` and `fileUrl`, so **25 requests plus 625
anonymous CDN fetches** gets every official campaign map ever published.
Rate: 1.7 s between trackmania.io calls, 0.55 s between Nadeo CDN fetches,
descriptive User-Agent, read-only. 624 of 625 came down first pass; the last one
succeeded on retry. Index and campaign JSON are banked.

## 8. What I did not do

No search, no candidate, no time claimed on any map. I did not extend the corpus
past the **seasonal** campaigns — Weekly Shorts, Weekly Grands, Training and the
Totd back-catalogue are all reachable by the same route shape and none is
included, so **a project map that is an alteration of a non-seasonal official
map would currently read as NO IDENTIFICATION, correctly but unhelpfully.** That
is the obvious extension and it is maybe twenty minutes.

I did not resolve §6. I did not test whether a **Reverse** map's official field
can be reversed into a usable line.

---

## Files

```
altered_nadeo/gen_RESULT_v1.md                      this file
altered_nadeo/gen_FLEET_NOTICE_altered_nadeo_identification_v1.md
altered_nadeo/gen_official_corpus_v1/
  gen_official_index.tsv          625 rows: uid <TAB> fileUrl <TAB> name
  gen_seasonal_campaigns.tsv      25 rows: campaign id, name, mapcount
  gen_seasonal_campaign_json.tgz  the raw campaign JSON, so the index is re-derivable
altered_nadeo/gen_tools_v1/
  gencell.rs                      the matcher
  gennames.rs                     model-name histogram of a map's full census
  gen_tmmaps_read_node_ref_FIXED.rs.txt   the 0x40000000 fix, with a rewind counter
  gen_tmmaps_full_census.rs.txt           read-only (unbaked, baked) census
altered_nadeo/gen_evidence_v1/
  gen_cell_210218.tsv             210218 vs all 625, every row
  gen_sweep_all_project_maps_v1.tgz   the full sweep, 32 project maps x 625
  gen_graft_210218.txt            raw validator JSON for the graft test
```

The 625 map bodies themselves are **not** banked (1.5 GB); the index re-fetches
them anonymously in about eleven minutes. They are on node 34881 at
`/tmp/tmtas-gen/work/official/<uid>.Map.Gbx` while that node lives.

## Reproduce

```
# the fix and the census reader must be in tmmaps first (both banked above)
cargo build --release --offline --bin gencell

gencell --target <altered>.Map.Gbx --against official/ --top 8 --tsv out.tsv
gencell --target <altered>.Map.Gbx --against official/<uid>.Map.Gbx --top 1   # name_agree
gencell --target X.Map.Gbx --against X.Map.Gbx --self-test                    # identity control
```

Rust only; no Python was written at any point. Shell drives curl, the oracle and
the file plumbing, nothing else.
