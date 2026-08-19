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

## Times transfer — demonstrated, not merely physical

**Twenty official human tapes have been grafted onto altered copies, and every
one of them returned its own official time or split to the millisecond.**

All five of the official top five on Fall 2025 - 16, grafted onto our
[270051](../270051-fall-2025-16-cp1-end):

| donor | our oracle on the altered map | that human's official CP1 split |
|---|---|---|
| rank 1 (41.475 WR) | **4.951** | 4.951 |
| rank 2 | **4.951** | 4.951 |
| rank 3 | **4.962** | 4.962 |
| rank 4 | **4.966** | 4.966 |
| rank 5 | **4.932** | 4.932 |
| lossless control | 4.831 | 4.831 |
| *our native TAS tape* | *4.830* | — |

and, independently, **all fifteen** grafted official Fall 2024 - 08 humans return
their own official times to the millisecond on
[228607](../228607-torment-1-up)'s geometry.

Twenty foreign tapes, twenty exact, **and untunable** — each is a separate
prediction that could only come out right if the identification, the graft and
the oracle are all correct. It validates the whole chain in one measurement.

### The recipe is map-dependent, and you pick it with a control

An earlier version of this page carried a caveat saying the graft was a measured
negative and undiagnosed. **It was a defect in the recipe, and it is fixed.**

Carrying chunk `0x0309202D` declares the **donor's** race result onto the
carrier — an official ghost's nine splits onto a map with one waypoint — and the
validator rejects the run. The instruction had always been *carry the inputs, not
the container*; the published `--ids` list contradicted it.

```
270051   --ids 0x0309201D                            control exact, official 4.951   WORKS
270051   --ids 0x0309201D,0x0309202D                 control exact, official DNF
270051   --ids 0x0309201D,0x0309202D,0x0309202B      control exact, official DNF
210218   --ids 0x0309201D                            CONTROL ITSELF DNFs
210218   --ids 0x0309201D,0x0309202D,0x0309202B      control exact at 103.915
```

> **Try both the inputs-only form and the three-chunk form, and use whichever
> one's lossless control passes in that same batch. Never assume either.**

**And note why this hid for so long: the lossless control passes in all three
rows on 270051.** A native donor carries compatible metadata, so a
native-into-native graft is fine however many chunks you take. The control was
working correctly and was **blind to this by construction** — another instance of
a control testing a proposition adjacent to the one it was believed to test.

### `name_agree` predicts the transfer, exactly

With the recipe chosen properly, 210218's official humans **still** return
`DNF cps=0` — with its own control exact at 103.915 in the same batch.

That is no longer a mystery. It is a clean measurement of the surface swap:

> **`name_agree` 0.9857 → foreign tapes run and return their own splits.
> `name_agree` 0.5909 → they do not, because the physics changed.**

The classifier and the transfer test agree, which is the strongest form the
classification could take.

## What this changes immediately

Two results follow from the twenty exact grafts, and neither required a new
search:

* **Our [270051](../270051-fall-2025-16-cp1-end) tape at 4.830 beats every one of
  the official top five** (best 4.932), on a field of **87 596 players** rather
  than the 903 on the altered board.
* **On [228607](../228607-torment-1-up), the official top 15 all fire the
  launcher** at 692–997 km/h — while **0 of the 23** players on the altered
  board ever found it. The technique was never undiscovered; it was on a
  leaderboard nobody had connected to the map.

## What else this opens

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
