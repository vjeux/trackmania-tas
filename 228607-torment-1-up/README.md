# Torment (1-UP) — the within-map control that failed, and why that is worth publishing

**Author time 20.258 · human world record 24.902 · best known validated 24.854.**

TMX map [228607](https://trackmania.exchange/maps/228607) · **23 recorded runs**
· field reproduction 22/23 · sibling of
[Torment (1-DOWN)](../228811-torment-1-down), where the author time *did* fall.

**No replay is published here.** The 24.854 tape re-simulates on the untouched
map, but no human ghost for this map was banked, so our re-validation could not
carry a known-answer control in the same batch — and this project's rule is that
a number without a control does not get a replay next to it. The figure is
recorded, attributed, and left as a lower bound for whoever picks the map up.

**Nothing here has been or will be submitted to a Nadeo leaderboard.**

---

## Why this map is in the index at all

It is the fifth map in the attitude series — the claim that *where a car
transacts momentum with a surface, its roll angle at that contact orders the
field* — and it is the one where **the pre-registered prediction failed.**

| prediction | bar | top 10 clean | all 22 |
|---|---|---|---|
| **P1** roll deviation at the decisive last contact | r > +0.30 | **+0.327** | **−0.131** |
| P1 at contact −2 | — | −0.044 | +0.441 |
| P1 at contact −3 | — | +0.242 | −0.064 |
| **P2** free-ballistic phases stay below +0.30 | ≤ 1 of 5 above | **0 of 5** ✓ | — |
| **P6** powered-air phase correlates like a surface | r > +0.30 | **−0.381** (wrong sign) | −0.116 |

**The signs flip between populations at every contact.** +0.327 on the top ten
becomes −0.131 on all 22; −0.044 becomes +0.441. That is the signature of *no
effect*, not of a weak one. P1 fails. P6 fails with the wrong sign. P2 "passes",
but a null arm carries no information when the positive arm is null too.

One post-hoc test was run, is labelled as such, and also failed: a single roll
sample cannot describe a rotating car under body-fixed thrust, so the world-up
component of the car's own up-axis was integrated over the powered phase.
r = −0.382 on the top ten, and inspection shows it is carried entirely by one
outlier of a different run shape. **We stopped there rather than keep fishing.**

## Why the map could not answer the question

The sector table explains the failure better than the correlations do. Sector
durations across the top ten:

| sector | mean | sd |
|---|---|---|
| 1 | 4.749 | 0.044 |
| 2 | 3.558 | 0.064 |
| 3 | 1.457 | 0.125 |
| 4 | 2.458 | 0.417 |
| 5 | 3.506 | 0.410 |
| 6 | 3.429 | 0.149 |
| **7 (CP6 → finish)** | **7.220** | **0.936** |

The last sector carries the spread, as on every map in this series. But **the
last sector time does not order the finish here.** Rank 11 has the *fastest*
closing sector in the whole field — 5.249 against the world record's 6.269 — and
finishes eleventh, because it lost 2.5 s in sectors 3–5.

So the decisive contact is not decisive, and there is nothing for roll at that
contact to order. That is a property of the map, and it is why a within-map
control matters: **the same measurement, on a map that cannot express the effect,
correctly reports nothing.**

## The by-product

A splice into the world record over the divergence window (ticks 1500–2000, 210
windows) produced 9 finishers, best **24.854**, −0.048 on the world record, using
no launcher at all.

## Notes

* [`RESULT.md`](notes/RESULT.md) — the pre-registered predictions, the results, and the
  structural reason the map could not answer

## This map is an Altered Nadeo copy of **Fall 2024 - 08**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **400 000
players** on this geometry.

Geometry and surface are identical (`name_agree` 1.0000); the alteration is the Goal moved 64 m. Its sibling [Torment (1-DOWN)](../228811-torment-1-down) resolves independently to the same official map, which is right — they are one map with the finish in two places. A detail no matcher could have arranged: the official world record on Fall 2024 - 08 is held by **Emelius.**, the person this map's own header name credits.

*No time here is claimed from that field.* Grafting an official tape onto one of
our maps is a measured negative on 2 of 2 maps tried and undiagnosed, so "times
transfer" is a statement about physics rather than a demonstrated pipeline.
