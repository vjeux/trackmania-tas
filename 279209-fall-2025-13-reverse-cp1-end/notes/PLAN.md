# Map 279209 — "Fall 2025 - 13 Reverse CP1 End" — attack plan

uid `uKd2hMaH4k0KekCMv1rZUbrKFag` · Nadeo mapId `ceca46f4-b9c9-4c95-aca3-f4aef3ae0488`
· TMX id 279209 · author **`in-.-`** (Uruguay) · uploaded 2025-11-17 · AT **6595**
· best human online WR **6604** (`jujumasterr`) · 334 records · gap **9 ms**.

Everything below was measured on this map on this box on 2026-08-18. Nothing is
assumed from the sibling maps; where a sibling result is invoked it is named.

---

## 1. Acquisition and the identity control — PASSED, 105/105

Followed `ACQUISITION.md` exactly: fwdproxy, the descriptive research
User-Agent, 1.6 s between ghost downloads, no auth anywhere.

I pulled a **deep** slice rather than just the top 15: leaderboard offsets
0/15/30/60/100/150/250, i.e. **105 finishing human ghosts spanning ranks 1–265
and 6604–7029 ms** (a 425 ms spread, ~6.4% of the lap). That population is the
instrument for everything in §4 and §5.

```
tmtas validate --map <ABS map> --jobs 20 <ABS 105 ghosts>   ->  105/105 exact
```

Every one re-simulated to its exact recorded millisecond (6604, 6608, 6608,
6612 … 7029). Map loads, ghosts decode, the oracle agrees with the online board
across the whole 425 ms range — not just at the top, which is the part that
would have been easy to fake. **Wall time: 2.7 s for all 105.** This map is
cheap; that shapes the whole plan (see §6).

Candidate factory round-trip (`tmsearch --verify` on r001, then validate):
**6604 exactly**. Template is 813 input ticks of 10 ms; the run finishes at
tick ~660.

Carried as the identity control in every batch from here on: `r001_6604`.

## 2. The medals say the AT is a driven lap — REACHABLE

| medal | ms | gap to AT |
|---|---|---|
| author | **6595** | — |
| gold | 7000 | +405 |
| silver | 8000 | +1405 |
| bronze | 10000 | +3405 |

Gold/silver/bronze are round numbers to the whole second. Nadeo's generated
medals are never that tidy — the author hand-typed them and did not care. In the
TM2020 editor the **author medal is not editable**: it is whatever the author's
own validation run did. So 6595 is a lap `in-.-` physically drove.

Two further observations that matter:

- `in-.-` is **not on the leaderboard at all** in the top 265. The author
  validated at 6595 and never posted a public attempt. So the AT is not "the
  author's best of a thousand tries also visible online" — it is one recorded
  validation lap, 9 ms better than the best of 334 other people.
- `in-.-` is also the author of **191465 `Training - 10 Long`**, another map on
  the unbeaten list, where the same medal signature appears. This author sets
  ATs by driving them.

Verdict: **hand-set, driven, reachable.** Not a formula, not a plugin artefact.
The map also survives unbeaten.at's own curation (`hiddenReason == null`).

## 3. What kind of map this is — decoded, not guessed

`tmmaps list` finds exactly **two waypoints**:

| # | what | where |
|---|---|---|
| 0 | block#2653 `RoadIceStart`, tag Spawn | cell (35, 11, 34), world ≈ (1136, 34, 1101) |
| 1 | item#707 `cp1end` (`blocks\bob.Gbx.Item.Gbx`), tag **Goal** | cell (32, 11, 40) |

2656 blocks, 708 items, **no intermediate checkpoints**. This is a "CP1 End"
conversion: the community author took Fall 2025 – 13, drove it in reverse, and
dropped a custom finish item where CP1 used to be.

Consequences, both of them load-bearing:

1. **A DNF carries no information.** There is no checkpoint ladder, so
   `score_dnf` is a flat plateau and reward shaping is not available for free —
   same situation as 145875. If DNF gradient is ever needed it must come from
   the fork server's arclength-along-the-incumbent measure.
2. **The start is on ICE** (`RoadIceStart`). Ice traction is the most
   perturbation-sensitive surface in the game, and it is where the human field
   is most likely to be leaving something — or, equally, where our fitted car
   model is least trustworthy. Establish which before believing anything from
   the first two seconds.

### The lap, from the human WR's telemetry (r001, 6604)

| phase | t (ms) | what happens | speed km/h |
|---|---|---|---|
| S0 | 0–1700 | standing start on ice, straight down a **steep** −z... +z slope; y falls 34.0 → 28.6, x moves 0.8 m in total | 0 → 98 |
| S1 | 1700–2800 | the slope shallows, gear 1→2, still essentially straight | 98 → 130 |
| S2 | 2800–3600 | **full RIGHT held**; the car turns off the straight, yaw 0.09 → −0.59 | 130 → 161 |
| S3 | 3600–6604 | **full LEFT held for 3.0 s** — one enormous sweeping left-hander, the whole rest of the map; yaw −0.59 → −2.83 (140° of rotation), and the car *accelerates the whole way through it* | 161 → **212** |

Wheels are on the ground for **every tick of the whole run** (no air phase at
all — so §B of UNBEATEN.md's "reactor/boost/flight" trigger does not apply here;
this is a pure grip-and-line map). Gas = 1 on essentially every tick. Brake is
used by **1 of the 16 tapes I inspected, for 6 ticks**.

### The finish plane — located to 15 cm

The Goal is an item, so `tmmaps probe`'s block sweep does not address it
directly. Measured instead from the population: every ghost's telemetry stops at
the last 50 ms sample *before* its declared finish, so extrapolate each run from
its last sample at its last measured velocity to its declared finish
millisecond. If the plane is `x = const`, all 105 must agree on the x they
reach.

```
tmpop endstate --csv traj/top15   ->  x_fin mean 1040.726  sd 0.149  (n=15)
tmpop endstate --csv traj/csv     ->  x_fin mean 1040.885  sd 0.333  (n=105)
```

**The finish plane is `x ≈ 1040.6–1040.7`, normal to x.** (The residual scatter
is exactly the constant-velocity extrapolation error: runs whose last sample is
48 ms from the finish extrapolate high because the car is still accelerating, so
the true plane sits at the low end of the range.) `z_fin` meanwhile spreads over
**7 m** among the top 15 with no timing consequence — the gate is wide and only
x matters.

Crossing conditions for the human WR: **x = 1040.68, vx = −57.8 m/s, vz = −10.8
m/s**, i.e. heading 79° away from... essentially straight down the −x axis at
212 km/h.

**1 ms at the plane = 5.8 cm.** That number governs the endgame: the last input
tick is 58 cm of travel, so the final milliseconds are bought with *speed and x
at the last tick*, not with steering geometry.

## 4. The human input alphabet, read off the tapes — the keyboard ladder is REAL

The brief says establish the action-key alphabet from the human tapes rather
than guessing. Done (`tmpop tapes`, over 16 tapes spanning ranks 1–265):

| run | ms | steer alphabet | change events | brake ticks |
|---|---|---|---|---|
| r001 | 6604 | 70 distinct | 120 | 0 |
| r002 | 6608 | 111 | 159 | 0 |
| **r003** | **6608** | **3 — exactly {−127, 0, +127}** | **17** | **0** |
| r004 | 6612 | 67 | 95 | 0 |
| r005 | 6615 | 78 | 123 | 0 |
| r006 | 6615 | 65 | 97 | 0 |
| **r075** | **6737** | **3 — {−127, 0, +127}** | **14** | **0** |
| r115 | 6768 | 47 | 60 | 6 |
| r265 | 7029 | 106 | 224 (max \|steer\| only 74) | 0 |

Two findings I did not have to assume:

1. **The input tape is 8-bit signed steer, −128..+127**, and the population
   alphabet across all tapes is 238 of the 256 values — but 4611 + 4214 + 1074
   of ~10 000 sampled ticks are exactly −127 / 0 / +127. The field is
   overwhelmingly digital even when the pad allows otherwise.
2. **A pure-keyboard human run is 4 ms off the human WR.** `r003_6608` is rank 3
   with a three-value alphabet and **17 input change events for the entire lap**.
   That is the low-input family's skeleton handed to me for free, and it means
   the "low-input strat" deliverable does not have to be reverse-engineered from
   an analog tape — it can be *seeded* from a human keyboard tape that is
   already within 4 ms of the best human.

r003's 17 events, in full:

```
   30 LEFT     740 centre   800 RIGHT    900 centre  1020 RIGHT  1170 centre
 1250 RIGHT   1440 centre  1500 RIGHT   1600 centre  1730 LEFT   2410 RIGHT
 2560 centre  2790 RIGHT   3620 centre  3680 LEFT   (held to the finish)
```

Read that as: **a wiggle sequence on the ice for the first 1.7 s, a long left,
a stab of right, then RIGHT held 2790→3620, then LEFT held 3680→finish (2.9 s
on one key).** Everything after 3.68 s is a single held key. That is
extraordinarily good news for reproducibility.

## 5. Where the time is — two independent measurements that agree

### 5a. The human population (105 runs, `tmpop sectors` / `diverge`)

Cumulative delta vs r001 at a ladder of planes, top 15:

| station | t (approx) | spread across top 15 |
|---|---|---|
| z=1110 | 2.3 s | −0 … +3 ms |
| z=1150 | 2.7 s | −1 … +8 ms |
| z=1190 | 3.7 s | −3 … +22 ms |
| z=1230 | 4.6 s | −5 … +34 ms |
| z=1250 | 5.1 s | −14 … +54 ms |
| finish | 6.6 s | 0 … +51 ms |

And over the whole field, the correlation between a run's *speed at time t* and
its *finish time* is ≈ 0 until t = 3.2 s and then jumps to **−0.51 … −0.59 from
t = 3.4 s onward**; the correlation between *yaw* and finish time peaks at
**+0.80 at t = 3.8 s**.

> **The entire human field is within 8 ms of each other at 2.7 s and spread over
> 50 ms by 5.1 s.** The ice start and the straight are solved. The time lives in
> the corner: the last of the RIGHT (≈3.2–3.6 s) and the first second of the
> long LEFT. Fast runs are already rotated further and carrying more speed by
> 3.4 s, which means the decisive input is *earlier* than the place the gap
> shows up.

### 5b. Our own neighbourhood (4170 unbiased single moves, `--dump`)

From the human WR r001 (6604), window 100, ticks 0–680:

- **83.3 % of mutations still finish** — a forgiving map with a real gradient.
- **2.54 % of single random moves IMPROVE on the human WR** (106 of 4170).
- **Best single random move: −9 ms → 6595, exactly the AT.** One unbiased
  mutation equals the author time.
- Where they come from:

  | tick range | ms | n | improve | best |
  |---|---|---|---|---|
  | 0–203 | 0–2030 | 1342 | **0.00 %** | — |
  | 203–339 | 2030–3390 | 1048 | 2.0 % | −3 |
  | 339–407 | 3390–4070 | 480 | 3.3 % | −6 |
  | 407–475 | 4070–4750 | 506 | 2.4 % | −4 |
  | **475–543** | **4750–5430** | 544 | **8.27 %** | **−9** |
  | 543–611 | 5430–6110 | 210 | 4.8 % | −3 |
  | 611–679 | 6110–6790 | 40 | 0.0 % | — |

  **Zero improvements in 1342 samples over the first two seconds** — the
  independent confirmation of 5a from a completely different instrument.

- By operator, and this is the headline for the search design:

  | op | n | finish | improve | best |
  |---|---|---|---|---|
  | **edge** (move a steer transition) | 469 | **100 %** | **12.37 %** | **−9** |
  | cos (raised-cosine bump) | 1874 | 71 % | 1.60 % | −9 |
  | lvl (flat analog level) | 1046 | 85 % | 1.53 % | −6 |
  | acc | 372 | 100 % | 0.27 % | −2 |
  | brk | 266 | 100 % | 0.38 % | −2 |

- Big moves die, as everywhere: spans ≥ 80 ticks finish 27–42 % and improved
  **0 of 171**. Spans 0–5 ticks improve 4.76 %.
- Same dump from the keyboard tape r003 (6608): 1.22 % improve, best −6.

**`edge` is eight times better than anything else and never DNFs.** That is not
a coincidence — it is the direct consequence of §4: the optimal tape on this map
is a handful of long full-lock holds, so the only thing that can be optimised is
*where the transitions are*. A raised-cosine bump in the middle of a 300-tick
full-left hold is a no-op if it saturates and a corruption if it does not; moving
the edge of that hold by two ticks is the whole game.

## 6. Therefore — the plan

The oracle costs ~1.3 ms/eval per worker here (105 full validations in 2.7 s at
20 jobs). At 176 cores that is on the order of 10⁵ evaluations per minute. **The
fork server is not obviously worth it on this map** — its 3.3–5.7× comes from
skipping a long prefix, and here the whole run is 6.6 s with gains spread from
tick 200 to tick 610, so the resumable prefix is short and the predicate
watchdog has little tail to save. That is a hypothesis to *measure* against a
concurrent control (PROTOCOL), not to assume in either direction.

1. **Beat the AT and bank it immediately.** A single dump move already reaches
   6595; a real search should go under within a minute. Bank the first
   re-validated sub-6595 tape to `evidence/` before doing anything else, so the
   deliverable exists even if the node dies. *(Done: 6592 at t=17 s, see
   §7.)*
2. **Search, four concurrent arms, distinct `--root` per process** (the known
   bug; `--root` now also defaults to `/dev/shm/tmsearch-<pid>` in the patched
   tool, but pass it explicitly anyway). Metropolis temperature **3 ms** — gains
   here are 1–9 ms, so map 2's T = 25 would be a random walk. Windows biased to
   ticks 300–620.
3. **Operator change I expect to pay: an `edge`-heavy mix.** The stock mix is
   45 % cos / 25 % lvl / 15 % edge / 15 % acc-brk, and on this map cos and lvl
   between them buy 1.5 % improve while edge buys 12.4 %. Predicted worth of
   re-weighting to edge-dominant: **1.5–2.5× the rate of accepted improvements**
   at equal evaluations. Also widen the edge displacement from ±5 to ±8 ticks,
   since 2-tick edge moves are already producing most of the accepted gains and
   the operator is truncating the tail of the distribution. Measure it as an
   A/B against the stock mix, concurrently, same box, PROTOCOL bar.
4. **Do NOT spend the budget on ticks 0–200.** 1342 unbiased samples improved
   zero times and the human field is within 8 ms there. Confirm once with a
   dedicated ice-start arm (the ice is the one place where "nobody in the field
   tried it" is plausible), and if that arm produces nothing, mask it out.
5. **Multi-seed islands.** The field is not on one line — r002 is 39 ms *ahead*
   of the WR at x = 1120 and still loses, r010 is 108 ms behind there and takes
   72 of it back. Those are different basins. Run islands from r001 (analog WR),
   r003 (keyboard), r002 and r004 (the two runs that lead the WR mid-corner).
   Cross-splicing is known not to work; distinct seeds are the way.
6. **The endgame is a speed problem, not a geometry problem.** 1 ms = 5.8 cm at
   the plane and the car is still accelerating through it, so the last
   milliseconds come from arriving at x = 1040.7 faster, which is set by the
   exit of the long left several seconds earlier. Expect the final ms to be
   bought at ticks 450–550, not at 640.
7. **Human-reproducibility work runs in parallel from the start, not after.**
   The keyboard seed r003 makes this concrete: build the low-input family by
   quantizing our best tape to {−127, 0, +127}, merging events, and re-searching
   *under the constraint*, with r003's 17-event structure as the target shape.
   Report per-event tick tolerance by sweeping each transition ±N ticks through
   the plain oracle.

### Predicted worth, stated before measuring

| move | prediction |
|---|---|
| edge-heavy operator mix | 1.5–2.5× accepted-improvement rate vs stock mix |
| T = 3 vs T = 25 | decisive; T = 25 should behave as a random walk here |
| masking ticks 0–200 | ~+25 % effective throughput, 0 ms lost |
| multi-seed islands | insurance; probably 0 unless r001's basin is bad |
| fork server + predicates | **≤ 1.2×, possibly a loss** — short run, no long dead tail |
| sub-tick / final-tick vernier | last 1–3 ms |
| keyboard-only family | costs 5–15 ms vs the unconstrained best |

## 7. Status at the time this plan was written

**The AT has already fallen.** Arm a1 (seed r001, stock mix, T = 3) reached
**6592 ms at t = 17 s**, re-validated through the plain oracle from the banked
copy of the map:

```
tmtas validate --map .../279209/map.Map.Gbx .../evidence/AT_BEATEN_6592_a1.Ghost.Gbx
AT_BEATEN_6592_a1.Ghost.Gbx    6592
```

6592 is **3 ms under the 6595 author time and 12 ms under the human WR**. Within
five minutes the arms were at 6580 / 6583 / 6588 / 6588. So the remaining work
is not "can it be beaten" — it is §6.7, the part that has value to the humans
this project exists to serve, plus finding out how much is actually on the table
so the published replay is not embarrassed a week later.

## 8. Non-negotiables carried into this map

Never submit to a Nadeo leaderboard. Re-validate every claim with
`tmtas validate --map <ABS> <ABS>`; a number from a search log is not evidence.
A failed re-validation is a STOP and the specimen goes to `tm-loop/phantoms/`.
Rust only — the population analysis in §4/§5 is a new Rust binary (`tmpop`) in
the workspace, not a script. Rate-limited, honestly identified HTTP.

---

## CORRECTION (18:05) — the tick-to-millisecond mapping in §5b was wrong

§5b's "where improvements come from" table converted tape ticks to race
milliseconds as `tick x 10`. That is wrong: this map's tapes carry a
`start_offset_ms` of **1580** (the countdown is recorded), so

    race_ms = 10 * tick - 1580

The table's tick ranges are right; its millisecond labels were 1.58 s too late.
Corrected, the improvement hot zone is **race 3170-3850 ms**, not 4750-5430 —
which is the RIGHT-to-LEFT transition and the entry to the sweeper, not its
middle. The corrected reading AGREES with the population measurement
(reference-line stations put the field's spread as created between 2.75 s and
4.95 s), where the original mislabelling did not. Everything downstream in this
directory uses the corrected mapping; the conclusion of §6 (bias the window to
the corner, not the start) is unchanged and if anything strengthened.

The other consequence: ticks 0-203, where 1342 unbiased samples improved zero
times, is race **-1580 to +450 ms** -- i.e. mostly the COUNTDOWN, when inputs
cannot move the car. That is not evidence that the ice start is optimal; it is
mostly evidence that the first 158 ticks of the tape are inert. A later arm did
find an accepted improvement at tick 146 (race -120 ms), which is pre-steering
the wheels during the countdown -- a real lever, and one the dump could not see
because it was averaged in with the inert ticks.
