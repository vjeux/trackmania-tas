# Ten of these maps are Altered Nadeo copies — and we can now name the official map each one is

Nine of the maps in this repository, plus one more, are **Altered Nadeo** maps:
someone took an official Nadeo campaign map and modified it — swapped a surface,
moved the finish, reversed the route — and published it as a new map with a new,
much smaller leaderboard.

That means each of them has a **second field**, on identical geometry, of between
29 274 and 900 000 players.

| our map | is officially | jaccard | `name_agree` | official field | what the field is worth |
|---|---|---|---|---|---|
| [203072](../203072-yeet-fall-2024-04) | Fall 2024 - 04 | 1.0000 | 0.9914 | 600 000 | geometry **and** surface — times comparable |
| [199100](../199100-spring-2023-24-2up) | Spring 2023 - 24 | 0.9978 | 1.0000 | 200 000 | times comparable |
| [279209](../279209-fall-2025-13-reverse-cp1-end) | Fall 2025 - 13 | 0.9902 | 0.9901 | 200 000 | **route reversed** — geometry only |
| [279218](../279218-fall-2025-22-reverse-cp1-end) | Fall 2025 - 22 | 0.9884 | 0.9859 | 44 128 | **route reversed** — geometry only |
| [279197](../279197-fall-2025-01-reverse-cp1-end) | Fall 2025 - 01 | 0.9787 | 0.9782 | **900 000** | **route reversed** — geometry only |
| [270053](../270053-fall-2025-18-cp1-end) | Fall 2025 - 18 | 0.9721 | 0.9869 | 76 975 | **CP1 End** — the official opening *is* the race |
| [270051](../270051-fall-2025-16-cp1-end) | Fall 2025 - 16 | 0.9459 | 0.9857 | 87 596 | **CP1 End** — the official opening *is* the race |
| [228811](../228811-torment-1-down) | Fall 2024 - 08 | 0.9574 | 1.0000 | 400 000 | Goal moved 64 m |
| [228607](../228607-torment-1-up) | Fall 2024 - 08 | 0.9574 | 1.0000 | 400 000 | Goal moved 64 m |
| [210218](../210218-fall-2024-25-pure-wet-icy-wood) | Fall 2024 - 25 | 0.3049 | **0.5909** | 29 274 | **surface swap — a corridor, never a time** |

**No time is claimed from any of this**, and nothing here changes a published
result. What it changes is what a future attempt on these maps has to work with.

---

## How they were identified, and why it is believable

Every sibling-matching instrument this project owned keys on `(cell, block
model)`. An Altered Nadeo map is the official layout with its **surfaces
swapped**, so it scores **zero against its own original by construction** — which
is why nine of these sat unrecognised for a night.

The instrument that worked throws the model names away and matches on **occupied
cells alone**, then reports name agreement afterwards as a separate column. It
was swept blind against **all 625 official seasonal campaign maps**, Summer 2020
to Summer 2026.

**The answer key is what makes it publishable.** Every one of the ten matches the
altered map's **own header name** — which the matcher never reads:

| identified as | the altered map's own name |
|---|---|
| Fall 2024 - 04 | `YEET Fall 2024 - 04` |
| Spring 2023 - 24 | `Spring 2023 - 24 (2-UP)` |
| Fall 2025 - 13 | `Fall 2025 - 13 Reverse CP1 End` |
| Fall 2024 - 08 | `Fall 2024 - 08 Torment (1-UP)(ft' Emelius)` |
| Fall 2024 - 25 | `Fall 2024 - 25 (Pure Wet Icy Wood)` |

Ten for ten, on a signal the matcher has no access to. Two further checks could
not have been arranged:

* **228607 and 228811 resolve independently to the same official map.** That is
  correct — they are one map with the Goal moved 64 m — and nothing in a
  cell-occupancy matcher knows they are related.
* **Fall 2024 - 08's official world record is held by Emelius.**, the person
  228607's own header name credits.

**And the instrument can refuse.** Run against 106 official maps that did *not*
include the original, it reported:

```
SEPARATION  best 0.0809   runner-up 0.0786   ratio 1.0x   field median 0.0447
  The best hit is NOT separated from the runner-up. Treat this as NO
  IDENTIFICATION, not as a weak one.
```

With all 625 present it then picked the right map out of 625, blind. Unrelated
project maps score ≤ 0.047 against each other, and it aborts on item-built maps
rather than scoring noise. **It says no when the answer is absent and yes when it
is present** — which is the standard this project holds every instrument to.

## What each field is actually worth — read `name_agree`, not `jaccard`

`jaccard` says *this is the same layout*. **`name_agree` — the fraction of shared
cells where the block model also matches — says what the official field can be
used for**, and the two answer different questions.

* **≥ 0.978, geometry-preserving.** Same layout, same surfaces, same physics. The
  official field is a field of humans driving the same car over the same road.
* **`Reverse` variants (279197, 279209, 279218).** Same physics, but the humans
  drove it *backwards*. The field gives you geometry and a corridor, **not a
  line**.
* **`CP1 End` variants (270051, 270053).** The best case of all: the official
  map's opening **is** our entire race, so hundreds of thousands of humans have
  driven exactly our sector, at full commitment, as the start of their own lap.
* **0.5909, 210218.** A surface swap. The road was re-skinned and the structure
  kept, which is exactly why it scores 0.59 rather than ~0. **The official field
  is a corridor and never a time** — wet icy wood is not the surface those humans
  drove.

## The caveat, and it must travel with the result

**Grafting an official tape onto our map is a measured negative on 2 of 2 maps
tried, and it is undiagnosed.**

```
210218 <- official Fall 2024 - 25 humans   control 103.915 EXACT, natives exact, donors DNF cps=0
270051 <- official Fall 2025 - 16 humans   control   4.831 EXACT, natives exact, donors DNF cps=0
```

210218 failing is expected — different surface, different physics. **270051 is
not**: `name_agree` 0.9857, same surfaces, and it still failed. The graft
machinery itself is sound (the control's grafted tape re-simulates its own line
to 21 mm over 328 ticks), but the official tape could not be located against its
own telemetry at race 1.5 s. The measured candidate, unconfirmed, is a two-tick
`start_offset` difference between the foreign and native containers — −1550 ms
against −1530 — which the graft carries across.

> **"Times transfer" is a statement about physics, not a demonstrated pipeline.**
> Nothing in this repository has yet driven an official tape on one of our maps.
> Anyone who tries should run it **only** with a native-carrier control in the
> same batch and a first-second telemetry check.

## What this opens

Not a time — a source of **answer keys**, which this project has repeatedly found
worth more than a seed. The pattern is established elsewhere in this repo: on
[YOU LOVE WATER](../284238-you-love-water) a sibling map's human settled what an
obstacle demands and produced the diagnosis; on
[Spaghetti Nights 2](../146612-spaghetti-nights-2) a sibling human's line
transferred as a **seed for a window** and beat our own lineage by 0.126 s in a
sixth of the evaluations.

These fields are one to four orders of magnitude larger than the leaderboards we
have been mining. Two cheap extensions nobody has taken: the corpus is seasonal
campaigns only, so Weekly Shorts, Weekly Grands, Training and the TOTD
back-catalogue would come down the same route; and nobody has tried reversing a
`Reverse` map's official field into a usable line.

## Notes

* [`ALTERED_NADEO_RESULT.md`](ALTERED_NADEO_RESULT.md) — the full sweep, the controls, the refusal
* [`ALTERED_NADEO_METHOD.md`](ALTERED_NADEO_METHOD.md) — how to run the identification on a new map
* [`ALTERED_NADEO_ADDENDUM_classification_and_graft.md`](ALTERED_NADEO_ADDENDUM_classification_and_graft.md) — `name_agree` as a
  classifier, and the graft negative in full
