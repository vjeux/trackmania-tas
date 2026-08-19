# 285885 — the next lever, measured: 14 seconds of the run has never been searched

*Third agent, closing file. Companion to `att_MAP_CLOSED_v1.md`, which prices
every route through the trigger and finds none inside the budget. This one
tests my own recommendation — "attack the arrival time, not the trigger" —
rather than leaving it as an opinion.*

---

## 1. The assumption on the record

`FINDINGS.md` (agent 1):

> "The first 35 s (highway, ramp, launch) is within a metre of the WR's line —
> **the WR drives that part essentially optimally.**"

The first clause is a measurement. **The second does not follow from it**, and
nobody tested it: matching a human's line is evidence about the *line*, not
about whether the line is fast.

## 2. The measurement

For each sample of the fast route, the nearest point of the human WR's whole
path (any time), and the WR's time there — giving the TAS's **lead along the
route**, which is immune to the two runs being at different places at the same
instant:

| TAS t | nearest WR point | WR's time there | **lead** |
|---|---|---|---|
| 2.000 | 3.15 m | 2.080 | +0.08 |
| 6.000 | 0.27 m | 6.210 | +0.21 |
| 12.000 | 8.43 m | 12.360 | +0.36 |
| 14.000 | 3.04 m | 14.340 | +0.34 |
| 16.000 | 54.79 m | — | *(different line through the loop)* |
| 18.000 | 96.04 m | — | *(different line)* |
| **24.000** | **1.06 m** | 27.740 | **+3.74** |
| **28.000** | **0.64 m** | 31.760 | **+3.76** |
| **32.000** | **1.85 m** | 35.870 | **+3.87** |
| 34.000 | 9.06 m | 38.030 | +4.03 |
| 38.000 | 50.33 m | 47.540 | +9.54 |
| 40.000 | 8.61 m | 72.860 | +32.86 |

Three regimes, and the middle one is the finding:

* **0–14 s (the highway):** the TAS is on the human's line to within 0–11 m and
  gains **0.36 s in fourteen seconds.** Agent 1 was right about this stretch.
* **14–20 s (the loop):** the TAS takes a genuinely different line, 55–96 m off
  the human's, and gains **3.4 s**. This is where the whole upstream advantage
  comes from.
* **20–34 s (the westbound run):** the TAS is back on the human's line —
  **nearest approach 0.6 to 1.9 m** — and the lead is **flat at 3.74 → 3.87 s
  across the entire fourteen seconds.** It is driving a human's line at a
  human's pace and gaining nothing.

## 3. What that is worth

That 14-second stretch covers ~1075 m at an average of **276 km/h**, on a car
that reaches **639 km/h** on the highway and is still only at 411 km/h when it
leaves. **43 % of every sample before 36 s is below half of peak speed.**

The section is acceleration-limited rather than skill-limited — the TAS climbs
steadily 282 → 411 km/h across it — which means the lever is not the straight
itself but **the speed it is entered with**: the loop spits the car out at
144 km/h at race 20 s. Every km/h of exit speed is paid back over 1075 m.

A crude bound: carrying 400 km/h into the straight instead of 144 would cover
those 1075 m in roughly 8 s instead of 14. **That is ~6 s, from the one part of
the run the route search never touched** — and the loop (14–20 s), which is the
only place a TAS has *ever* beaten this human, is right next to it.

## 4. Why this is the right thing to do next

The budget arithmetic on this map is `43.079 − arrival`. Every result in
`att_MAP_CLOSED_v1.md` is a price in seconds:

| tilt source | cost | needs the arrival at |
|---|---|---|
| rank 1's flip (the humans' own route) | **11.2 s** | ≤ 31.9 s |
| rank 2's wall | 19.4 s | ≤ 23.7 s |

The trigger side is closed: no upright crossing fires, and no cheap tilt exists.
But **the flip route is a known, human-demonstrated, fully-validated way to
finish this map** — and it needs nothing new at all if the approach can be
driven in ~31.9 s instead of 41.0. That is a 9.1 s saving against a stretch
where ~6 s is sitting unclaimed and 43 % of the run is below half speed.

It is also a *normal* search problem — arrival time at the patch, an objective
that already works, on a section with no trigger subtleties, no attitude, and no
gate surgery. Everything difficult about this map is in the last 40 m, and the
last 40 m is now known to be impossible; the time is in the first 35 s, and the
first 35 s has never been searched.

**My recommendation to the fleet is to reopen 285885 on the approach, not the
finish.** It is the only map I know of where the hard half is provably closed
and the easy half is untouched.

## 5. Files

`att_upstream.txt` (the two comparisons in §2, and the speed census) ·
`att_tools.tgz` (adds `upstream.rs`, `pathcmp.rs`).
