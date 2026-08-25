# Training - 10 Long

**The world record's one genuinely faster sector is the one you have to give
away. It banks a millisecond and a half crossing the reset pad and pays nine for
it over the last 448 m — and the way to go faster is to spend *more* there, not
less.**

**Training - 10 Long** — TAS **13.070** (−0.010) | AT 13.080 | WR 13.081 by in-.-

> ### The clip is the 13.070 now — re-shot 2026-08-24
>
> **Both things this banner used to warn about are fixed.** It read: *"The
> published clip is an older, slower run, and its film was already bad — shot
> from the 13.071 file, whose recorded trajectory is not its own tape's run
> (kappa 0.382)."* The clip above is now the **13.070**,
> `replays/TAS_13070_analog.Ghost.Gbx`, and that file's recording IS its own
> run: **kappa 1.000**, 262 of 262 samples exact, with the plain oracle
> re-simulating the WRITTEN file to **13.070**. Same treatment as before — two
> cars, in-.-'s world record 13.081 as the opponent, chase camera on our car,
> input overlay.
>
> The clip shipped earlier the same day was of the **13.071** and was itself
> already superseded by the time it went up; this replaces it. Corpus-wide
> table: [`KAPPA.md`](../KAPPA.md).

https://github.com/user-attachments/assets/0f1e45fd-bd17-40b7-b265-17240ea68e13

| run | time | vs author time | vs human WR |
|---|---|---|---|
| **TAS, unconstrained** | **13.070** | **−0.010** | **−0.011** |
| TAS, unconstrained (previous) | 13.071 | −0.009 | −0.010 |
| TAS, 5-level pad | 13.074 | −0.006 | −0.007 |
| **TAS, keyboard only** | **13.075** | **−0.005** | **−0.006** |
| Author time | 13.080 | — | −0.001 |
| Human WR — in-.- | 13.081 | +0.001 | — |

TMX map [191465](https://trackmania.exchange/maps/191465) · author **in-.-** ·
**1070 recorded runs** (board 2026-08-24; the field measurements on this page
were taken over the 856 recorded then).

The author time is the author's own editor validation lap, and their best public
attempt sits 0.001 behind it. Nobody has ever got past it.

## The world record, sector by sector — measured at the same PLACE, not the same instant

Comparing two runs at the same *instant* answers the wrong question: at 240 m/s
two cars five milliseconds apart are 1.2 m apart, so "who is faster at t = 11.0"
is mostly "who is further down the road". The table below is the comparison that
means something — each run's interpolated crossing time at a ladder of planes
along the road (`tmtraj splits`), differenced. **Negative = the record is
ahead.**

The map runs from x = 1520 down to the line at x ≈ 29.

| x (m) | what is there | the WR vs our 13.071 | vs our 13.070 |
|---|---|---|---|
| 1504 → 1248 | the launch, the ramp | −0.14 … +0.37 ms | the same |
| 1216 | booster 1 | +0.36 | +0.36 |
| 928 | booster 2 | +0.33 | +0.33 |
| 672 | booster 3 | **+1.70** | **+2.01** |
| 544 | run-in to the reset pad | +1.62 | +2.26 |
| **480** | **the reset pad** | **+0.47** | **−0.30** |
| 448 | road is one lane now | +0.36 | −1.07 |
| 256 | final straight | +2.91 | +2.96 |
| 96 | final straight | +8.66 | +9.71 |
| finish | | **+10** | **+11** |

**The lead vjeux was looking for is not at the beginning.** Through the launch,
the two-level ramp and booster 1 — the first 6.8 seconds and the first 300 m —
the record and our tape are the *same run* to within four tenths of a
millisecond, and the first 186 ticks of the two input tapes are byte-identical.
There is nothing there.

### There is one sector where the record really is faster, and it is the reset pad

Between **x = 552 and x = 456** — the run-in to the reset pad and the pad itself
— the world record takes **1.35 ms** back off our 13.071. That is real, it is the
only place on the map where it is ahead of us, and the mechanism is visible:

| at x = 512 | total speed | speed *along the road* | angle of travel off the road |
|---|---|---|---|
| our 13.071 | 225.18 m/s | 223.01 | 8.0° |
| human WR | 224.84 | **224.03** | **4.9°** |

**The record is slower and gets there sooner.** It is carrying less speed and
pointing more of it down the road: from x = 544 to x = 448 it moves 6.4 m across
the road where we move 11.3 m, and cos(8.0°) against cos(4.9°) over that 96 m is
exactly the millisecond and a half.

### It is a trade, and the record lost it — so did we, by not spending enough

The reason the record is straighter at the pad is that it is **7 m wider** when
it gets to the single-lane section, and it spends the last 448 m putting that
right: 17 m of lateral movement, eight full-lock corrections in the final 1.3 s,
and **8.84 ms**. Bought 1.35, paid 8.84.

Three independent tests say the sector does not transplant:

- **Grafts.** 32 hybrid tapes — ours up to tick *k* then the record's, and the
  reverse, for *k* every 20 ticks across race 9.47 … 12.47 — put through the
  plain oracle. **Every one is slower.** The best is 13.072, and the far end of
  the sweep converges on 13.072 and 13.083, either side of the two runs the
  hybrids are made of, which is the control that says the splicing is sound.
- **A search that could only see a millisecond.** 688 050 evaluations over the
  whole window from booster 3 to the line, on the plain validator, moved nothing.
  Its matched positive control — the same flags, the same budget, seeded from a
  tape one millisecond slower — recovered that millisecond in 9 000 evaluations.
  There was no millisecond-sized gain to find there.
- **The new run went the other way.** The 13.070 gives away **2.38 ms** more than
  the 13.071 does through x = 544 … 416, and takes **2.81 ms** back over the last
  400 m. It arrives at the reset pad *more* crabbed and 6 m further from the
  record's line, not less.

**So the reset pad is a currency you spend, not one you save.** What matters is
not how fast you cross it; it is what attitude you arrive at the last 400 m with.

## Where the time is

Same route, same lanes, no air phase anywhere on the map. Split by split against
the world record, the eleven milliseconds are three numbers:

| where | worth | what it is |
|---|---|---|
| start → x = 544 | **+2.26 ms** | almost all of it the **booster-3 clip**, about a metre tighter than any of the 14 measured human runs (+2.01 ms is already banked by x = 672) |
| x = 544 → 448 | **−3.34 ms** | the reset pad. The record's one faster sector, and we give away more of it than the 13.071 did |
| x = 448 → the line | **+12.1 ms** | the last 448 m, at 810–860 km/h |

The last 448 m is the map. The human field puts in eight full-lock corrections
in the final 1.3 seconds; this run does not, because the work that makes that
possible is done between the booster-3 crossing and the reset pad.

And the last millisecond, 13.071 → 13.070, is the same shape, smaller:
−0.64 ms banked by x = 544 coming out of booster 3, **+2.38 ms given away at
the reset pad**, −2.81 ms
taken back from x = 416 to the line.

So this is a discipline problem, not a secret. It is also why a keyboard tape can
do it: **the time was never hiding in analog resolution.** A 5-level pad tape
matches 13.074 and the pure keyboard tape — steering only ever `{−127, 0, +127}`
— is one millisecond behind it. It is hiding in *what* you steer, not how finely.

For scale on the margins: at the finish speed of 858 km/h, **1 ms is 24 cm of
travel**.

## How the last millisecond was found

The plain validator returns an **integer millisecond**, and on this map that is a
thousand times too coarse to search on: 1 ms is 24 cm of road, so almost every
mutation is invisible to it and the population random-walks a plateau. That is
not a guess — the two runs below are the same seed, the same window and the same
machine:

| objective | evaluations | result |
|---|---|---|
| the plain millisecond | 688 050 | **nothing** |
| `tmsearch --plane 28.90` | 285 000 | 13.071 → **13.070** |

`--plane` scores a finisher by the fork child's own interpolated crossing of a
plane at x = 28.90, in **microseconds**, and it is new in this repo
([`tools/search/SEARCH.md` §6](../tools/search/SEARCH.md)). It never becomes a
result: the guard is unchanged, every banked candidate is re-simulated from the
bytes on disk by the plain oracle, and what is written down is the oracle's
millisecond. Over four searches it confirmed **109 improvements and 0 phantoms**.

Two things it needs, both of which bit somebody before:

- **A per-worker calibration.** The child's tick labelling moves by a whole tick
  between fork servers *and between workers of one run*. Each worker now
  measures the incumbent on its own server, snaps the offset to a whole tick and
  **refuses to join** if the residual is over 2 ms.
- **A repeatable attitude at the line.** The finish trigger is a *body*, not a
  plane through the car's centre. On this map the two ends of the range agree to
  a quarter of a millisecond (our 13.071 crosses x = 28.90 at 13.070 75 and the
  record's 13.081 at 13.080 75); on an airborne finish the same idea produced a
  confident 7.991 that validated at 8.004.

The final file was **not** chosen on that surrogate. Once tapes diverge from the
fork's reference by hundreds of ticks the plane drifts away from the validator —
here by about 1.2 ms while the validator moved 1 ms — so the four candidates were
re-run as **clean, un-resumed full simulations** (`ghost regen`) and ranked by
where the car is at one fixed engine tick. At race 13.040 the published run is
**0.281 m further down the road** than the 13.071, which is 1.18 ms at 238 m/s.

### A defect in `ghost regen` found on the way, and it is reproducible

**`ghost regen` labelled this map's search tapes one whole tick late, and
labelled a downloaded human recording of the same map correctly.** Both controls
are unambiguous:

- The first 186 ticks of our tape and the record's are **byte-identical**, so the
  two runs are the same run there — and at the same label our regenerated record
  was 10 ms behind the record's, exactly one tick.
- Extrapolated to the finish plane, our regenerated record read 13.0807 for a
  tape the oracle calls **13.071**; the correction of −10 ms puts it at 13.0707.

It is **not** jitter: five regenerations of our tape all landed one tick late and
five of the human recording all landed correct, in fresh processes. Anything that
compares two regenerated records — a split table, a two-car clip — has to check
it. `ghost regen-control` does not catch it (it passes at 0.000 5 m, because the
file it checks is *internally* consistent), and `tmtraj splits --shift-ms` exists
so the correction has to be stated out loud rather than hidden in a spreadsheet.

## The run as inputs

The sector-by-sector guide for this map is not written yet, and neither is a
per-input slack table — study the input scripts below against the world record's
own, which is included for exactly that comparison. The published run is full
throttle from the countdown to the line, the brake is never touched, and its
steering is unconstrained analog (403 input events).

## Files

| file | what |
|---|---|
| `replays/TAS_13070_analog.Ghost.Gbx` | **13.070, the fastest tape** — `ghost verify` clean, kappa 1.000, five cold oracle passes |
| `replays/TAS_13071_analog.Ghost.Gbx` | 13.071, the previous best (kappa 0.382 — do not film it) |
| `replays/WIP_keyboard.Ghost.Gbx` | **13.075, keyboard only** — the one worth studying |
| `replays/WIP_pad5.Ghost.Gbx` | 13.074, steering in `{−127, −64, 0, 64, 127}` |
| `replays/TAS_13074_analog.Ghost.Gbx` | 13.074, unconstrained |
| `inputs/TAS_13070_analog.inputs.csv` | per-tick inputs for the fastest run (`ghost tape csv`) |
| `inputs/TAS_13071_analog.inputs.tsv` | per-tick inputs for the previous best |
| `inputs/TAS_13074_analog.inputs.tsv` | per-tick inputs for the 13.074 analog run |
| `inputs/human_WR_13081.inputs.tsv` | the world record's inputs, for comparison |
