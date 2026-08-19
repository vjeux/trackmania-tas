# 165922 `idm ruinin ur day #460` — vj4 continuation: margin, tolerance, low input

Agent vj4, 2026-08-19 00:00–01:2x PT, node **64455.od.fbinfra.net**, build fork
`/tmp/tmtas-vj165` (tmtas-rs-hardened + lowinput v5 + tmsimp v5), staging root
`/tmp/vj165`. Author time **15.643**. This file continues, and does not replace,
`RESULT.md` (agent 1), `RESULT_v3_AT_BEATEN.md` + `v3/` (agent 3) and my
`vj4_VERIFICATION_v1.md`.

Every number below is from the **untouched map**
(md5 `1cc927bbb1d640c665ff69068352d4e6`) with the human record 8790.769 as a
known-answer control in the same batch, unless a relocated-gate instrument is
named explicitly.

---

## 1. Headline

| tape | time | what it is |
|---|---|---|
| v3's banked beater | 15.549 | re-validated by me on a second node, control exact |
| **vj4 best** | **15.217** | 0.426 s under the AT; keyboard steering from race 4.56 s on |
| vj4 analog | 15.224 | pure hill-climb from v3's tape, 12 min of search |
| **vj4 keyboard-from-2.56 s** | **15.285** | 3 steering values after 2.56 s, **70 input events in the whole run** |
| vj4 deep-landing | 15.382 | forced to land in the far half of the pad (see §4) |
| keyboard-from-1.56 s | 16.293 | finishes, but over the AT — the ladder's current floor |

Time came cheaply at first (15.549 → 15.230 in 2 minutes, 74 % of mutations
still finishing) and then stopped: three 25-minute streams at three
temperatures all converged on 15.224–15.227 and sat there. The last 7 ms came
from the keyboard ladder, not from the analog search.

## 2. The tolerance claim, corrected and localised

The other arm reported "precision-bound: 343 of 343 boundary shifts DNF". That
is true of the **launch** and false of the tape. Measured with `vj4tol sweep`,
which moves each input-change boundary one tick (10 ms) earlier and one tick
later and re-simulates every variant on the real map:

**Whole race window, our 15.224: 1338 shifts tested, 1261 survive — 94.2 %.**

By region (same tape):

| window | shifts | survive |
|---|---|---|
| race 0.00–2.96 s | 52 | **0 %** |
| race 2.96–3.96 s | 30 | 30 % |
| race 3.96–4.96 s | 54 | 93 % |
| race 4.96–15.26 s | 1202 | **100 %** |

The keyboard tape (15.285) has the same shape — 0 % before 3.96 s (60 shifts),
100 % after (76 shifts) — and so does the deep-landing tape (15.382): 0 % before
3.96 s, 99 % after. **Three independently searched tapes, 112 fragile shifts
between them, zero survivors: the single-tick wall is a property of the map's
start chute, not of any one tape.**

Two further perturbation families, same tape, same window (race 0.00-2.96 s):

| perturbation | tested | survive |
|---|---|---|
| one boundary moved one tick | 52 | 0 |
| **two** boundaries moved one tick, every pair and every direction | **1300** | **0** |
| one tick, steering changed by ONE unit (1/127 of full lock) | 352 | 45 (12.8 %) |

So there are no compensating pairs at one-tick resolution, and even the smallest
representable steering change usually kills the run. For contrast the same
one-unit perturbation over race 2.96-4.96 s survives 97.7 % of the time and over
4.96-7.46 s, 500 of 500. The first three seconds are not "precise", they are
CHAOTIC: differences are amplified, not tracked.

And the failure is immediate, not a missed landing. Scoring the same 52 shifted
variants on the instruments: **0 of 52 reach the y = 1800 chute net**, 0 of 52
reach the pad's near edge, 0 of 52 finish on the 4× wider `netfar` pad. A 10 ms
difference in the first three seconds does not move the landing point — it
crashes the car in the chute. That is consistent with the only human on this
board needing **930 attempts in one session** to get through it once.

### Two ways I tried to buy tolerance, and what happened

* **Tolerance as the search objective** (`vj4tol search2/search3`). Pass/fail
  tolerance has no gradient here — every candidate scores 0 — so the objective
  is graded: a jittered replica scores 3 if it finishes, 2 if it reaches the
  pad's near edge (x = 2300 curtain), 1 if it clears the chute net, 0 if it dies.
  A first version sampled 10 random boundary shifts per candidate and **froze at
  18/30 in 13 seconds**: a sampled objective is noisy and a hill climber banks a
  lucky draw. The deterministic version (every boundary, both directions —
  53 simulations per candidate, 70 workers, ~7500 evals/min) moved to 52/156 and
  then sat there for 30 minutes. **No tolerant launch was found.**
* **Landing deep on purpose.** If the tape clipped the pad's near edge, aiming
  at the middle would buy margin for free. Built `vj4_padfar` (the 132 finish
  gates re-hung on the far half of the pad's own positions — position-only,
  origin control passes) and searched on it: 15.384 there, **15.382 on the real
  map**, i.e. a legal AT-beater that lands ~40–80 m deeper. Its launch tolerance
  is **unchanged at 0 %**. So the fragility is not the aim; it is the chute.

**What I would try next**: the fragile window is only ~26 boundaries wide. It is
small enough to ENUMERATE rather than anneal — every ±1 tick perturbation of
every boundary is 52 tapes, and the full 3^26 is not, but a two-boundary joint
sweep (~1350 tapes, ~4 minutes) would say whether the wall is one-tick-exact in
every direction or whether pairs of compensating shifts exist. If pairs exist,
tolerance is a *coupling* problem and the objective should be over pairs.

## 3. The low-input family (§0.7.2), searched under the constraint

The board's only human is **on a keyboard**: over all 879 231 ticks of the
record, 94.2 % of steering values are exactly {0, −127, +127} and the throttle
is held 100 % of the time inside the successful attempt (102 input events in
18.8 s). So the alphabet was read off the human tape, not invented:
`--qlevels 1` = {−127, 0, +127} is that board's alphabet.

Straight quantisation of a finished analog tape DNFs (as on every other map in
this project). So the constraint was put **inside** the search, with two
patches to my fork:

* `tmsearch --qlo/--qhi` — apply the alphabet ladder only to ticks in a window,
  so the window can be grown while the incumbent stays a finisher.
  Controls: an empty window reproduces the seed exactly (15224); `--qlevels
  zero` over ticks 400–500 DNFs. The instrument can say both yes and no.
* `tmsearch --gaslo/--gashi` — the same for "gas held, no brake".

The ladder grows the keyboard window **backward from the finish**, because the
chute is the fragile end:

| keyboard from | result |
|---|---|
| race 13.56 s | 15.224 (free) |
| race 9.56 s | 15.221 |
| race 6.56 s | 15.220 |
| race 4.56 s | **15.217 — the session's best time** |
| race 3.56 s | 15.292 |
| race 2.56 s | **15.285**, 43 events after the boundary, 70 in the whole run |
| race 1.56 s | 16.293 so far — over the AT |

Two things worth carrying to other maps. **The keyboard constraint did not cost
time — it found time**: the best tape of the session is a keyboard tape from
4.56 s onward, beating the pure-analog champion by 7 ms. And **a two-minute rung
that reports "no finisher" is not a negative**: rung 310 failed at 2 minutes and
60 workers, and produced a finisher at 8 minutes and 90 workers.

### The other two channels

* **Steering is inert in the glide.** Forced to zero from race 6.46 s: +7 ms.
  From 8.46 s: +1 ms. From 4.50 s: DNF. So ~9 of the 15.2 seconds need no
  steering input at all.
* **The throttle is load-bearing only in the launch.** Forcing `accel=1,
  brake=0` from race 4.46 s onward is completely free (15.224 unchanged); doing
  it over any 1-second window before that DNFs. Our tape lifts the throttle where
  the human never does, and only in the first 4.5 s.

## 4. Instruments built (all position-only, all with controls)

`tmex movegrid` on chunk `0x0304305F`, first free-block record at body offset
1674168, gates at records 34..165. **Origin control**: re-writing the 132 gates
at their own positions reproduces the human record at 8790.769 and v3's tape at
15.549 exactly, so the surgery is faithful.

| map | shape | control |
|---|---|---|
| `vj4_origin` | the pad rebuilt where it is | 8790769 / 15549 exact |
| `vj4_curtC` | curtain at x = 2300, y −1..159, z 576..928 (32 m) | human 8788.358, ours fires at its own finish tick |
| `vj4_curtF/Zlo/Zhi` | the same at 16 m z-spacing, three z bands | locates the arrival z |
| `vj4_curtWIDE` | x = 2300, y −1..79, z 400..1072 (22 columns) | fires for all three tapes |
| `vj4_padfar/padfar2` | only the far 6 (or 4) columns of the pad's own positions | human 8790769 unchanged |

**Where our line lands.** It arrives at the pad's near edge inside z ∈ [752, 928]
and **not** inside [560, 736]; the human's successful attempt arrives at z ≈ 690.
So our tape and the human's use different lateral corners of the pad. Our tape
crosses the x = 2300 curtain at **exactly its finish millisecond** — no ground
crawl at all, against 3.77 s of crawling in the human's attempt and 0.463 s in
v3's 15.549.

## 5. Throughput

The coordinator's ~34× applies to a template that still carries its recording's
telemetry and declared time. **Both were already fixed in the tape I inherited**:
5263 bytes, and `m165 findu32 <tape> 16000` reports the declared time at all five
sites (`0x03092005`, `0x0309200B+12`, `0x0309201B+10`, `0x0309202B+4/+32`).
Measured here: ~350 candidates/s at 60 workers, ~600–730/s at 70–100 workers,
70 % of them finishers. I did not re-declare just above the incumbent: with the
tape already declaring 16.0 s against a 15.2 s incumbent, the remaining prune is
~5 % and not worth another set of artefacts.

## 6. Artefacts

In `~/persistent/private-30d/tm-unbeaten/165922/`, all prefixed `vj4_`:

| file | what |
|---|---|
| `vj4_best_15217.Ghost.Gbx` | the session's fastest, keyboard from race 4.56 s |
| `vj4_clean_15230.Ghost.Gbx` | the first improvement over v3's tape (banked early) |
| `vj4_best_15224.Ghost.Gbx` | the pure-analog champion |
| `vj4_keyboard_15285.Ghost.Gbx` | keyboard steering from race 2.56 s, 70 input events |
| `vj4_padfar_15382.Ghost.Gbx` | lands deep in the pad; 0.261 s under the AT |
| `vj4_maps/` | every instrument map above, with its control in this file |
| `vj4_tools/` | `vj4in`, `vj4patch`, `vj4tol`, and the `--qlo/--qhi/--gaslo/--gashi` patch |
| `vj4_HUMAN_TECHNIQUE_v1.md` | the §0.7.1 write-up, for a player |
| `vj4_VERIFICATION_v1.md` | my independent re-validation of v3's beater |
