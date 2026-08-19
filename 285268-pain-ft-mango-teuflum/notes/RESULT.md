# Map 285268 — `Pain ft Mango & Teuflum` — **author time beaten**, and what a human should actually take from it

uid `c94vCARJWKM_FsXQ4dnaqQWUHqf` · **AT 49.282 s** · human WR **49.446 s**
(burntbagels) · 163 records · Stadium **Ice** · Slidelock + mang0tm ·
uploaded 2025-12-26 · gap to beat: 164 ms

**Unconstrained TAS floor: 49.275 s (validated). The author time falls by 7 ms.**

**But the number is the smaller half of this map's result.** Two findings matter
more to the 163 people who play it:

1. **The field's own best sectors sum to 48.733 s — 549 ms under the author
   time.** Nobody has ever put a clean lap together here. The AT is not a wall
   the field is pressed against.
2. **Ssnake01's rank-2 lap (49.491 s) is a *pure keyboard* run that owns four of
   the ten sectors and is dead last of the top twenty in one of them.** With a
   merely median sector 4 that same lap is **49.279 s — under the author
   time, on a keyboard, with 58 key presses.**

| tape | time | vs AT | what it is | file |
|---|---|---|---|---|
| unconstrained TAS | **49.275** | **−7 ms** | human WR's inputs for 42.88 s, then 52 changed ticks | `m285268_49275_BEST.Ghost.Gbx` |
| — earlier bank | 49.278 | −4 ms | same line, one round earlier | `m285268_49278_AT_BEATEN.Ghost.Gbx` |
| **keyboard** | **49.475** | +193 ms | **2 changes to Ssnake01's lap; 59 steer events, alphabet {−127,0,+127}** | `m285268_49475_keyboard.Ghost.Gbx` |
| author's own AT lap | 49.282 | 0 | recovered from inside the .Map.Gbx | `AT_author_telemetry.csv` |
| human WR | 49.446 | +164 ms | analog, 294 steer events | (downloaded) |
| best human keyboard | 49.491 | +209 ms | rank 2, 57 steer events | (downloaded) |

Every time above is the plain oracle's answer to that exact file.

---

## 1. Validation and integrity

* Map pulled anonymously from Nadeo's own
  `core.trackmania.nadeo.live/maps/ad311207-…/file` — 4 650 629 bytes, sha256
  `8264685054d3276694c4a6182783150d0507b352e54c20b36d2c3da9f19b213d`.
* **158 of the 163 downloaded human ghosts re-simulate to their leaderboard
  millisecond exactly.** The five that do not (ranks 103, 105, 126, 153, 161)
  return DNF after 1–7 checkpoints — deep-field runs that used a respawn. Every
  run in the top 100 is exact.
* Every batch of every search carried an **unlabelled identity control** (the
  incumbent, asserted to return its exact known time). ~1.6 M candidates across
  eleven stages, **zero control failures**.
* Every banked tape re-validated with `tmtas validate` against the untouched map
  with a known-answer human ghost alongside. 49275, 49278, 49285, 49311, 49355,
  49475: all exact, no phantoms.
* **Nothing was submitted to a Nadeo leaderboard.**

### 1.1 Why none of the project's five phantom mechanisms can reach this result

All five known silent-corruption mechanisms live in the **fork** path or in
surrogate scoring. This map used neither. `m49` is a map-tailored driver over
`tmsearch::sweep::evaluate` — the classic path: every candidate is written out
as a real `.Ghost.Gbx` and adjudicated by `TrackmaniaServer /validatepath`
against the untouched map. No resume, no sub-tick plane, no gate ladder, one
map per worker root, an explicit distinct `--root` per stage.

Two design choices are worth stating as a **pattern for the project**, because
they make a class of bug impossible rather than unlikely:

* **Never compare a DNF and a finish on one axis.** The score is
  `struct Score { fin: u8, key: i64 }`, compared lexicographically. A finisher
  can never be outranked by a DNF however deep it got. On 2026-08-18 the
  single-scalar encoding used elsewhere in the project
  (`cps·SEG_UNIT − t` against `FINISH_BASE − t`) overflowed into exactly that
  failure at 11 checkpoints, and its guard misfired at 6. **The fix is not a
  bigger constant — it is refusing to put two incomparable things on one axis.**
* **A DNF is structurally ineligible to become the incumbent**: the accept
  filter is `Some(t) if t < best_t`. `reached_cps` is used only for diagnostics
  (§4), never for acceptance.

Cost of that choice, measured: **173–330 candidates/s on 176 cores, 845 ms of
CPU per 49-second validation.** Slower per candidate than the fork server, and
immune to all five mechanisms. On a map where correctness is the bottleneck
rather than throughput, that is the better trade.

---

## 2. What this map is

5098 ticks; `race_ms = −1530 + 10·tick`. **Nine intermediate checkpoints plus
the finish** — the first map in this project with real sector structure. Not a
full-throttle map: the top runs brake for 1.1–2.0 s and lift the gas 5–14 times.
Effectively never airborne (the WR has 4 airborne telemetry samples of 989; the
author has none during the race).

### 2.1 The measurement that decided everything: the dream lap

| sector | race window | WR | field best | owner of the best | top-20 spread | corr. with final |
|---|---|---|---|---|---|---|
| S1 | 0 → 6.522 | 6.522 | 6.501 | rank 6 | 117 ms | +0.44 |
| S2 | 6.522 → 12.349 | 5.827 | 5.738 | rank 3 | 227 ms | +0.75 |
| S3 | 12.349 → 18.492 | 6.143 | 6.052 | **rank 2** | 357 ms | +0.55 |
| S4 | 18.492 → 22.939 | 4.447 | 4.410 | rank 13 | 335 ms | +0.55 |
| S5 | 22.939 → 26.988 | 4.049 | 3.968 | rank 7 | 331 ms | +0.57 |
| S6 | 26.988 → 31.788 | 4.800 | 4.790 | **rank 2** | 255 ms | +0.78 |
| S7 | 31.788 → 38.627 | 6.839 | 6.727 | rank 15 | 409 ms | **+0.85** |
| S8 | 38.627 → 42.972 | 4.345 | 4.270 | **rank 2** | 349 ms | +0.55 |
| S9 | 42.972 → 47.636 | 4.664 | 4.641 | rank 3 | 316 ms | +0.69 |
| S10 | 47.636 → 49.446 | 1.810 | 1.729 | **rank 2** | 83 ms | +0.56 |

**Sum of the field's own best sectors = 48.733 s. Even restricted to the top
ten runs it is 48.866 s.** That is 416–549 ms under the author time.

This is the opposite of every previous map in this project. On 270051 and 227969
the field had ground the route flat and the remaining milliseconds were
sub-tick. Here the AT stands because a 49-second Ice map punishes consistency
and **no human has ever strung ten good sectors together**.

The time is also **diffuse**: every sector correlates 0.44–0.85 with the
finishing order and every sector has 100–400 ms of top-20 spread. There is no
single dramatic feature to attack — the contrast with 279197's sweeper (worth
nothing to practise) and 270051's closing jump (worth nothing) is complete, and
the reason is the map's length, not its surface.

### 2.2 Sector ranks of the top ten — this is the deliverable

1 = fastest through that sector among the top 20.

| run | final | S1 | S2 | S3 | S4 | S5 | S6 | S7 | S8 | S9 | S10 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 burntbagels | 49.446 | 10 | 10 | 4 | 5 | 4 | 2 | 11 | 5 | 3 | **19** |
| **2 Ssnake01 (kbd)** | 49.491 | 5 | 8 | **1** | **20** | 2 | **1** | 14 | **1** | 7 | **1** |
| 3 Tigu. | 49.535 | 12 | **1** | 10 | 10 | 10 | 4 | 5 | 8 | **1** | 3 |
| 4 thgiN_ (kbd) | 49.541 | 6 | 5 | 12 | 16 | 3 | 5 | 2 | 6 | 11 | 5 |
| 5 Jastastic000 | 49.634 | 8 | 3 | 6 | 3 | 7 | 6 | 4 | 18 | 12 | 9 |
| 6 ChooDawn | 49.640 | **1** | 9 | 2 | 15 | 5 | 3 | 6 | 15 | 8 | 8 |
| 7 Shikurima | 49.720 | 3 | 7 | 15 | 13 | **1** | 13 | 9 | 4 | 13 | 2 |
| 13 Slidelock (author) | 50.140 | 14 | 17 | 11 | **1** | 17 | 16 | 13 | 17 | 6 | 10 |
| 15 SimKingTM | 50.304 | 4 | 15 | 14 | 17 | 8 | 17 | **1** | 16 | 19 | 11 |

Two things jump out and both are actionable today, with no TAS involved.

* **Ssnake01 is throwing the lap away in sector 4.** 4.745 s against a top-20
  median of 4.533 s and a best of 4.410 s. Nothing else in that lap is worse
  than 8th. A median S4 puts that run at **49.279 s**, under the author time,
  and it is a **keyboard** lap.
* **The world record's own worst sector is the last one**: 19th of 20, 81 ms
  off the best, on a 1.8-second run to the line. That is where our TAS took its
  time too, and it is the cheapest sector on the map to practise because it is
  short and it is the least chaotic (§4).

### 2.3 The field's input alphabet — measured, not assumed

**72 of the 163 records are pure keyboard** (steer only ever −127, 0, +127),
including **8 of the top 20** and both rank 2 and rank 4. The best keyboard run
is 45 ms off the analog world record, with **57 steer change events across 49
seconds**. Keyboard is not a handicap on this map.

### 2.4 How close is the field really? One sector each.

The dream lap of §2.1 needs ten different drivers' best sectors, so it is a
ceiling, not a prediction. The disciplined version: for each of the top twenty,
take the **top-20 median** for every sector, find the run's single worst sector
against that median, and replace it.

| run | final | worst sector | loss vs median | fix that one | fix two | every sector at or under the median |
|---|---|---|---|---|---|---|
| 1 burntbagels | 49.446 | S10 | 36 | 49.410 | 49.410 | 49.410 |
| **2 Ssnake01 (kbd)** | 49.491 | **S4** | **212** | **49.279** | **49.199** | 49.199 |
| 3 Tigu. | 49.535 | S1 | 3 | 49.532 | 49.532 | 49.532 |
| 4 thgiN_ (kbd) | 49.541 | S4 | 62 | 49.479 | 49.467 | 49.467 |
| 5 Jastastic000 | 49.634 | S8 | 57 | 49.577 | 49.564 | 49.564 |
| 6 ChooDawn | 49.640 | S4 | 58 | 49.582 | 49.533 | 49.533 |
| 7 Shikurima | 49.720 | S3 | 71 | 49.649 | 49.609 | 49.550 |
| 9 Lattack.TM | 49.781 | S9 | 74 | 49.707 | 49.677 | 49.677 |
| 12 TheNedzy | 50.058 | S7 | 145 | 49.913 | 49.795 | 49.693 |
| 17 mgpm | 50.524 | S7 | 297 | 50.227 | 50.049 | 49.875 |

(top-20 median sector times: 6.522 / 5.828 / 6.231 / 4.533 / 4.153 / 4.903 /
6.839 / 4.453 / 4.726 / 1.774; they sum to 49.962.)

**Exactly one run in the field is one sector away from the author time, and it
is the keyboard one.** Everybody else needs three or four. That is the honest
version of §2.1: the 549 ms of dream-lap slack is real but distributed, and the
single actionable target on this leaderboard is **Ssnake01's sector 4**.

Caveat, stated plainly: sectors are not independent — a sector time is bought
partly with entry speed from the one before (§3.1 is the proof). These numbers
are an indicator of where the slack is, not a predicted lap.

---

## 3. The author's own lap, recovered from inside the map file

The map header says `validated="1"`, and `tmtraj decode map.Map.Gbx` returns
**1046 telemetry samples at 50 ms: the author's own validation lap — the one
that set 49.282.**

**(a) `atSetByPlugin: true` is wrong for this map.** unbeaten.at flags the AT as
plugin-written; the embedded validation ghost is direct evidence a person drove
it. (Slidelock does sit 13th on his own leaderboard at 50.140 — that is a normal
gap between a practised validation attempt and a casual leaderboard run, and
note from §2.2 that his leaderboard run owns sector 4 outright.)

**(b) The telemetry `steer` column is the RAW INPUT, exactly.** Verified against
the world record: `round(steer_telemetry × 127) == steer_input` at every matched
50 ms sample, and the same for gas and brake. So the author's *input tape* is
readable on any validated map, not just their trajectory. The only loss is
resolution — a transition is located to within 5 ticks. Reconstructing a 10 ms
tape by hold-forward leaves 880 ambiguous ticks of 5098 and DNFs at CP1: on a
map this chaotic the reconstruction is a **shape, not a tape**.

**(c) The author is on the field's route.** Maximum separation from the WR's
line over the whole lap is **6.0 m**, typically 1–4 m. No launcher, no shortcut,
never airborne during the race. This is technique-within-a-route.

### 3.1 Where the author's 164 ms actually is — and the 100 ms they left behind

| sector | author | WR | Δ |
|---|---|---|---|
| S1 | 6.521 | 6.522 | −1 |
| S2 | 5.754 | 5.827 | **−73** |
| S3 | 6.135 | 6.143 | −8 |
| S4 | 4.543 | 4.447 | **+96** |
| S5 | 4.018 | 4.049 | −31 |
| S6 | 4.829 | 4.800 | +29 |
| S7 | **6.636** | 6.839 | **−203** |
| S8 | 4.371 | 4.345 | +26 |
| S9 | 4.729 | 4.664 | +65 |
| S10 | 1.727 | 1.810 | **−83** |

The author's S7 of 6.636 is **91 ms faster than the entire human field's best
S7**. And the author gives 96 ms back in S4 and 65 ms in S9. **The author time is
one good lap with two bad sectors** — which is the same story as everyone
else's, and is why the dream-lap number in §2.1 is the honest target.

**How the S7 gain is made — and it is not made in S7.** Speed against the WR at
matched positions: **−4.4 km/h** at race 28.2 s (the crest at x ≈ 600, where the
track climbs 42 → 64 m), then +1.4, +2.7, **+6.5, +5.3, +6.4, +5.8, +8.1, +7.8,
+7.1, +7.3 km/h** from 29.6 s continuously to 37.4 s. Textbook slow-in /
fast-out: **give up 4 km/h at the top of the sector-6 rise and carry 6–8 km/h
more for the whole of sector 7.** In §5's terms the author is also using the
least lock of anyone through S6 (27.1 % of ticks at full lock, against the WR's
32.3 % and Ssnake01's 69.8 %).

---

## 4. The constraint that shaped the whole attack: this map is violently chaotic

One tick of steer applied to the WR tape:

| race time of the change | outcome |
|---|---|
| before 0 s (countdown) | 49.446 every time — steering before the lights does nothing |
| 2.47 s | 5 of 6 finish |
| **6.47 s … 36.47 s** | **0 of 6 finish, at every magnitude down to ±1/127** |
| 41.47 s | 3 of 3 finish, best 49.412 |
| 43.47 s | 0 of 3 finish (all reach CP9) |
| 45.47 s | 3 of 3 finish |
| 48.50 s | 3 of 3 finish, best 49.407 |

**Between race 6.5 s and 36.5 s, one tick of ±1/127 — 0.8 % of lock for 10 ms —
kills the run.** The DNF lands one to three sectors downstream of the change
(perturb at 18 s → dies at CP5; at 32 s → dies at CP7). That is the sharpest
possible statement of why 163 people are stacked where they are, and it ruled
out the move-at-a-time greedy on the full tape that solved every previous map
in this project.

The nine checkpoints buy a **diagnostic**, not a search gradient: `reached_cps`
is how the horizon above was measured. It is deliberately not used for
acceptance (§1.1).

---

## 5. The technique: on ice, the field pins full lock — and it is wrong exactly
   where the field is unanimous

Steering saturation over all 163 records, race ticks only:

| class | n | % of ticks at full lock | corr(lock %, finish time) |
|---|---|---|---|
| all | 163 | 74.2 | −0.49 |
| pure keyboard | 72 | 80.8 | −0.75 |
| analog | 91 | 69.0 | −0.49 |
| top 20 | 20 | 85.0 | +0.00 |

Read naively that says *more lock is faster*, and the sister investigation on
134672 (also Ice) measured the same sign. **It is a confound**: slower drivers
steer less because they are correcting more, so the correlation measures
mistakes, not the optimum. Two independent lines say the optimum here is below
full lock:

**(a) Our search.** Every millisecond it found came from holding *slightly less*
than full lock. The 49.275 tape is byte-identical to the human world record for
the first **42.88 seconds**; all 171 ms comes from **52 changed ticks** in the
last 6.4 s, in five blocks, and every one of them is "ease off full left lock":

| race window | duration | WR | ours | ease |
|---|---|---|---|---|
| 42.88 – 42.90 s | 30 ms | −127 | −103 | 19 % |
| **43.41 – 43.80 s** | **400 ms** | −127 | **−123** | **3 %** |
| 45.04 s | 10 ms | −127 | −124 | 2 % |
| 46.32 – 46.37 s | 60 ms | −127 | −126 | 1 % |
| 47.66 – 47.67 s | 20 ms | −127 | −123 | 3 % |

The single largest move on the map is that **400 ms of 97 % lock instead of
100 %, worth 124 ms of lap time on its own** (49.446 → 49.322).

### 5.1 Attribution — every subset of the five blocks, adjudicated by the oracle

All 31 non-empty subsets, applied to the human world record's tape:

| blocks | time | Δ |
|---|---|---|
| ② alone (the 400 ms ease) | 49.322 | **−124** |
| ⑤ alone | 49.415 | −31 |
| ③ alone | 49.421 | −25 |
| ④ alone | 49.421 | −25 |
| **① alone** | **49.621** | **+175** |
| ① ② | 49.290 | −156 |
| ① ② ④ | **49.278** | **−168** |
| ① ② ③ ④ | 49.277 | −169 |
| ① ② ③ ④ ⑤ | **49.275** | **−171** |
| ② ③ · ② ④ · ② ⑤ · ② ③ ④ · ② ④ ⑤ · … | **DNF** | — |

Two things fall out that no attribution heuristic would have found.

* **Three inputs carry 168 of the 171 ms.** The fourth and fifth are worth
  3 ms between them.
* **① is not a time-gainer, it is a stabiliser.** On its own it costs 175 ms.
  But ② combined with *any* of ③④⑤ and **without** ① is a **DNF** — every one
  of the six such subsets. The 30 ms, 19 %-of-lock ease at 42.88 s is what makes
  the car survive the rest of the sector once the long ease is in. That is the
  class of move a one-at-a-time greedy structurally cannot see, and here the
  compounding order found it only because ① happened to be accepted first while
  the tail was still soft.

**(b) The author.** Per sector, % of ticks at full lock:

| sector | author | WR | Ssnake01 (kbd) | author Δ vs WR |
|---|---|---|---|---|
| S6 | **27.1** | 32.3 | 69.8 | +29 ms |
| S7 | 93.4 | 95.6 | 100.0 | **−203 ms** |
| S9 | **100.0** | **100.0** | **100.0** | +65 ms |
| S10 | **91.7** (8.3 % gas lift) | 100.0 (0 % lift) | 100.0 (0 % lift) | **−83 ms** |

Through the long lefts at 27.4–27.9 s and 34.4–35.4 s the WR holds −127 while
the author holds 0, −74, −99, −82, −105, −108.

**Sector 9 is the blind spot, and it is unanimous.** All twenty of the top
twenty runs *and the author* hold full lock for **100.0 %** of sector 9. Our
whole 171 ms lives in it and the first 200 ms of sector 10. Nobody who has ever
driven this map fast, including the person who set the author time, has tried
anything else there.

The bounded statement that survives both ice maps:

> On ice, back off lock where you are trying to keep the car **pointed and
> accelerating**; keep it pinned where you are trying to **rotate**. And do not
> read the field's own lock-vs-time correlation as evidence — it is negative on
> both maps and it only means the drivers who steer less are the ones making
> mistakes.

Two more from the same statistics, both top-20: gas **lift** correlates +0.53
with a slower lap overall, yet the author's 8.3 % lift in S10 is worth 83 ms;
braking correlates **−0.48** overall (more braking, faster), **−0.77** inside
sector 9, and **+0.87** inside sector 10. **Brake in S9. Do not brake in S10.**

---

## 6. Honesty about our tape: it is a chaos exploit, not a technique

Required by the project rules, and it is the most important caveat here.

The decisive input — 400 ms at 97 % lock from race 43.41 s — was swept over
placement (±200 ms) × strength (1–24/127) × duration (100–600 ms), 3 276 cells,
every one adjudicated by the plain oracle. **There is no basin.** The −124 ms
cell at (offset 0, strength 4/127, 400 ms) sits in a field where the immediate
neighbours are +612, +100, +45 and DNF; strength 5 at the same placement DNFs;
duration 600 ms costs +100 to +700 everywhere. Scattered other cells give −131,
−71, −62 with no pattern.

So: **our 49.275 s beats the author time, and its inputs are lottery tickets.**
An open-loop tape in a simulator this chaotic exploits micro-divergences that a
driver cannot aim at. Per the project rule that "a human cannot do this" is
never the answer, the forgiving version of this result is not our tape — it is
§2.2/§2.4 and §7: **the field's own sectors, and the keyboard lap below.**

A second, blunter demonstration of the same thing: take the finished 49.275 tape
and add a *second* ease of the same shape as ① at the same place (30–60 ms,
12–32/127, placement ±80 ms, 144 cells). **106 of the 144 DNF and not one of the
other 38 is faster** — the cheapest of them costs +270 ms. The tape is on a
knife edge in every direction.

What our tape *does* prove, and it is worth having: 49.282 is not a floor, and
the specific place the whole field is leaving time is sector 9's unanimous full
lock.

---

## 7. The drivable deliverable

### 7.1 The keyboard lap — 49.475 s, two changes, both practisable

Seed: Ssnake01's rank-2 lap (49.491 s, pure keyboard). Searched under the
constraint (alphabet never leaves {−127, 0, +127}; edge slides and inserted
presses only). Converged at **49.475 s** after 53 173 further dense candidates
found nothing more.

| # | race | input | worth |
|---|---|---|---|
| ① | **46.00 s** | **release the left key for 20 ms** (2 ticks) in the middle of the long full-left hold | −11 ms |
| ② | **48.39 s** | **tap the brake for 30 ms** | −5 ms |

**59 steer change events, 10 throttle events, 37 brake events, alphabet
{−127, 0, +127}.** This is faster than every one of the 72 keyboard runs on the
leaderboard and would sit 3rd overall.

Tolerance of ①, swept on the plain oracle (placement ±250 ms × duration
10–80 ms):

* **duration matters more than placement.** 20 ms is right; 80 ms costs +130 to
  +600 ms *everywhere*, and 10 ms is mostly worth nothing.
* at 20 ms, the whole band from −50 ms to +30 ms of placement costs at most
  +48 ms and four offsets in it are worth −4 to −11. Mistiming it by 100 ms
  costs under 50 ms — it is a **cheap** input to try, which is the property that
  matters for something you will attempt a hundred times.

Verdict on ①: this is the keyboard expression of §5. You cannot hold 97 % lock
on a keyboard, but you can blip off it — and a 20 ms blip in the right place is
worth 11 ms while an 80 ms one is a disaster.

### 7.2 The sector guide — what to actually practise, off visual cues

Targets are the top-20 sector bests (§2.1). A lap that hits all of them is
48.826 s.

| sector | cue | target | what the fast runs do |
|---|---|---|---|
| **S1** 0 → 6.5 s | standing start to the first gate, still climbing to ~215 km/h | **6.501** | only 53 % full lock — this is the one sector where the whole field is *steering*, not holding. 117 ms of spread and it barely predicts the result. Do not over-drive it. |
| **S2** 6.5 → 12.3 s | 97.6 % full lock, 7.2 % of it braking | **5.738** | the highest-braking sector on the map. rank 3 owns it; the author is 73 ms up on the WR here. Brake earlier, hold the lock. |
| **S3** 12.3 → 18.5 s | slowest section, down to 153 km/h at the exit | **6.052** | Ssnake01 owns it on keyboard. |
| **S4** 18.5 → 22.9 s | 98 % full lock, 7 % braking, exit at 153 km/h | **4.410** | **the sector that costs Ssnake01 the author time.** The author loses 96 ms here too. The WR's 9 % gas lift through it is *good* — the author lifts only 3.4 % and is 96 ms slower. **Lift the throttle here.** |
| **S5** 22.9 → 27.0 s | acceleration to 220 km/h | **3.968** | 331 ms of spread, corr +0.78 with the final time. Worth real practice. |
| **S6** 27.0 → 31.8 s | the crest at 28.2 s: the track climbs 42 → 64 m and turns hard | **4.790** | **the least-lock sector on the map** (author 27 %, WR 32 %, Ssnake01 70 %). **Be 4 km/h slower over the crest.** That single sacrifice is what pays sector 7. |
| **S7** 31.8 → 38.6 s | the long fast sweep, 250–285 km/h, 6.8 s of it | **6.727** (author: 6.636) | the highest-correlation sector on the map (+0.85) and the biggest single prize. You do not gain it *here* — you carry 6–8 km/h through it because of what you did at the S6 crest. |
| **S8** 38.6 → 43.0 s | fast, 270–280 km/h at the gate | **4.270** | Ssnake01 owns it. |
| **S9** 43.0 → 47.6 s | the long left; **everyone holds full lock for all 4.7 s of it** | **4.641** | **the blind spot.** 100.0 % full lock across the whole top 20 and the author. Braking here is *good* (corr −0.77). This is where our TAS found 171 ms; on a keyboard, blip off the left key for 20 ms around 46.0 s. |
| **S10** 47.6 s → finish | the run to the line, 240 km/h | **1.729** | the WR is 19th of 20 here and 81 ms off. The author lifts the gas 8.3 % of it and is 83 ms up. **Do not brake in S10** (corr +0.87); *do* lift. |

**Honest about difficulty.** S6 → S7 is the big one and it is a real technique:
sacrificing 4 km/h at a crest to carry 7 km/h for the next seven seconds is
exactly the thing that feels wrong and pays. S4 is the cheapest fix in the top
twenty for anyone whose profile looks like Ssnake01's. S9 and S10 are the
shortest and least chaotic sectors and are where the field's unanimity means
there is unexplored ground. And the first 36 seconds of this map are chaotic to
the point that a 0.8 %-of-lock error for one frame ends the run in our
simulator — a driver is closed-loop and recovers, but that number is the reason
this map is called Pain and the reason consistency, not peak pace, is what
separates the leaderboard.

---

## 8. Method, and the negative results that are worth as much

Eleven stages, all plain-oracle, identity-controlled, distinct roots. Seed is
the human WR (49.446) unless stated.

| stage | window (race) | seed | result | rounds | candidates |
|---|---|---|---|---|---|
| A | 42.97 s → end | raw WR | **49.285** | 7 | 222 k |
| B | 38.63 s → end | A | 49.285 (**0 improvements**) | 1 | 54 k |
| C | 42.97 s → end, dense | A | 49.285 (**0 improvements**) | 1 | 113 k |
| D | sector 8 only | raw WR | 49.311 | 5 | 272 k |
| E | 42.97 s → end | D | 49.292 | 3 | 223 k |
| F | sector 7 only | raw WR | 49.355 | 3 | 251 k |
| G | 38.63 s → end, ±127 deltas | F | 49.315 | 2 | 319 k |
| H | 41.47 s → end, dense steer + dense gas/brake | A | 49.285 (**0 improvements**) | 1 | 180 k |
| **I** | **38.63 s → end** | **raw WR** | **49.275** | 6 | 796 k |
| **L** | **31.79 s → end** (independent reproduction, wider window) | raw WR | **49.277 → 49.275** | 5 | 1 076 k |
| K/K2 | 31.79 s → end, digital | rank 2 | **49.475** | 5 | 96 k |

**Stage L is a reproducibility control and it is a clean one.** Same seed, same
move set, window opened a further 684 ticks earlier (the whole of sector 7 in
play as well), so it explored 1.08 M candidates against stage I's 796 k — and it
accepted **the identical moves in the identical order**: `4494_40_0_4` (49.322),
`4441_3_0_24` (49.290), `4785_6_0_1` (49.278), `4657_1_0_3` (49.277). Two
searches over different windows landed on the same tape. The 49.275 basin is not
an artefact of one window choice, and opening sector 7 to the search bought
nothing at all.

Four transferable results, three of them negative:

1. **A converged tape is frozen.** After stage A, 113 138 dense single moves over
   the same window and 180 058 over a wider one with the full ±127 delta range
   and per-tick gas/brake returned **zero** improvements. Not diminishing —
   zero. 347 k single-move candidates say 49.285 was 1-move optimal at
   millisecond resolution for that basin.
2. **Upstream gains and downstream gains are SUBSTITUTES, not addends.** Sector 8
   alone is worth 135 ms on the untouched tail (D). The tail alone is worth
   161 ms on the untouched upstream (A). Both in sequence gives 154 ms (E), and
   adding sector 7 first gives 131 ms (G). **Every sequential chain was worse
   than the tail alone.**
3. **The reason is that the human tape is SOFT and an optimised one is HARD.**
   Stage A took 7 rounds (5175 → 428 → 7 → 21 → 192 → 13 → 8 → 0 improvements);
   stage G took 1 (686 → 0). Optimising a prefix removes more downstream
   opportunity than it creates. **On a long chaotic map, do the last sectors
   first, on the softest tape you have, and treat "earliest sector first" as the
   trap it is.**
4. **What finally worked was widening the window on the soft tape, not chaining
   stages.** Stage I is stage A with the window opened from 42.97 s back to
   38.63 s and *nothing else changed* — same seed, same move set. A gave
   49.285; I gave **49.275**. One search over a 1082-tick window beat three
   sequential searches over the same ground.

Also recorded: **`tmmaps splits` on a synthesised ghost returns the TEMPLATE's
checkpoint times**, because the factory copies the declared-splits chunk and
does not recompute it. Sector times for our own tapes must come from telemetry
(`tmtraj decode --csv` + a plane/point crossing), never from `splits`. This
would have produced a very convincing wrong table.

---

## 9. Tooling (Rust only; no Python anywhere)

* **`m49`** (`tmsearch/src/bin/m49.rs`) — the driver.
  `probe` (perturbation survival with checkpoint resolution) ·
  `sens` · `suffix` (windowed compounding greedy with multi-move stack
  acceptance) · `tol` (placement × strength × duration tolerance surface) ·
  `fromtel` (rebuild an input tape from decoded telemetry) · `emit` · `dumptape`.
* **`m49an`** — sector spread, correlation with finishing order, sector
  champions, per-run sector ranks.
* **`m49at`** — the author's embedded validation ghost against the field:
  `splits`, `route`, `inputs`, `lock`.
* **`m49sat`** — steering-saturation statistics over a whole field of decoded
  input tapes, by device class and by sector.

---

## 10. Files

In `~/persistent/private-30d/tm-unbeaten/285268/`:

| file | what |
|---|---|
| `m285268_49275_BEST.Ghost.Gbx` | **the unconstrained floor, 49.275 s** |
| `m285268_49278_AT_BEATEN.Ghost.Gbx` | 49.278 s, the first tape under the AT |
| `m285268_49475_keyboard.Ghost.Gbx` | **the drivable one: 49.475 s, keyboard, 59 steer events** |
| `m285268_49285_stageA_final.*`, `..._49311_stageD_*`, `..._49355_stageF_*` | the intermediate stages |
| `AT_author_telemetry.csv` | **the author's own AT lap**, decoded from the map file |
| `map285268.Map.Gbx` | Nadeo's own file, sha256 `8264685054d3…` |
| `splits_raw.tsv` | the 10-split vector of all 163 human records |
| `val_field_20260818.txt` | the 163-ghost identity control |
| `m49.rs`, `m49an.rs`, `m49at.rs`, `m49sat.rs` | the tooling |
| `NOTES_v1.md`, `PLAN.md` | working notes and the plan, as written before the search |

*(Stage L's fifth accepted move differs — `4918_2_0_64` where stage I took
`4919_2_0_4` — so the two 49.275 tapes are byte-different files that both
validate to 49275. Banked separately as `m285268_49275_BEST.Ghost.Gbx` and
`m285268_49275_stageL_independent.Ghost.Gbx`.)*
