# 173636 `Tap water 01` — AUTHOR TIME BEATEN. Result sidecar **v2**.

**Supersedes `RESULTS_173636_v1.md`** (which reported 22.123 mid-search and left
the low-input family open). v1 is not retracted and is not deleted; everything
in it still holds. Write-once — supersede with `_v3`, never edit in place.
**Nothing shared was edited.** Coordinator: merge into `RESULTS.md` /
`UNBEATEN.md`.

| | time |
|---|---|
| Author time (target) | **23.325** |
| Best human, 602 records — Reddnox, who is also the author | 23.638 |
| **This work** | **22.072** |
| Margin under the AT | **−1.253 s** |
| Margin under the human WR | **−1.566 s** |
| Best tape at a human 40 ms input grain | **23.125** (−0.200 under the AT) |
| Fewest inputs still under the AT, 1-minimal | **747 events → 23.183** |

uid `psnXaq_c26TynrBA_sS5_xGj7Xc` · Stadium · tag Underwater · TMX 173636 ·
`inPlugin: true`, **no `atSetByPlugin`** — an ordinary author medal from a
validation lap, whose holder also holds the online record.
Converged: the alternating search plateaued at 22.072 for three rounds.

---

## §0 collision — reported, not redone

`173636/` was created 2026-08-18 18:39 by the **attitude-experiment agent**,
who used the map as a control and wrote `RECON-disqualified-v1.md`. Their
verdict was correctly scoped — *disqualified as a test of the attitude question*
(0.0 % airborne, 7° of roll over the whole run: nothing for roll to order),
explicitly **"not disqualified as a TAS target"** — and there was no `RESULT.md`,
no ghosts and no search. I took the TAS target and inherited their
`map.Map.Gbx`, `leaderboard_sample.txt` (470 ranks) and 31 decoded
trajectories, which removed the acquisition step entirely. Their separate
warning also held up: our airborne detector keys on a −24.7 m/s² plateau that
never occurs at 90 km/h, so I used the ghosts' own `is_ground_contact` and never
the derived detector.

## Controls

1. **Oracle fidelity 30/30** — every downloaded ghost (ranks 1-20, 25, 30, 40,
   60, 80, 120, 160, 200, 300, 470) re-simulates to its exact leaderboard
   millisecond.
2. **Codec identity 30/30** — every tape re-encoded through my own writer, in
   its own container, returns its own time exactly.
3. Every number in this file was produced by the **plain oracle on the untouched
   map**, and every banked tape was re-validated on **two cold passes** in
   batches containing downloaded human ghosts as known-answer controls. Final
   sweep: 15 tapes + 3 human controls, twice, all exact.
4. **No embedded author ghost** — `ct probe` / `ct mapghost` / `rec nodes` all
   negative despite `validated="1"`.
5. **No checkpoints** (`tmmaps list`: one Spawn, one Goal), so the FINISH_BASE
   defect and the 6+-CP phantom-guard misfire are both out of scope.
6. **Every constrained result reads its constraint back off the delivered
   artefact** and prints HELD/VIOLATED. This caught a real defect: the first
   minimum-dwell ladder reported 23.166 at a "30 ms grain" and the artefact said
   `shortest interior run 2 ticks` — the repair function was not enforcing the
   constraint. Those numbers were discarded and the ladder re-run; the table
   below is from the fixed version, every row HELD.

## What the map is

One straight ramp. Spawn cell (1,38,20), Goal (14,14,20); after the drop-in the
surface is a **perfectly constant −0.5000 slope** (1:2, 26.6°) from x ≈ 68 to
x ≈ 448, y 240 → 51. The run is effectively one-dimensional: the whole field's
lateral spread is ~18 m of z over a 400 m descent. Blocks are
`PlatformTechSlope*` between a `RoadTechStart` and a `RoadTechFinish`, no items.

Speed saturates at **89-94 km/h** on a 191 m drop, the gear never leaves 1: a
very high-drag, low-grip surface, which is what the name and the `Underwater`
tag are about.

---

## The finding: this is a throttle DUTY-CYCLE contest, and the field is capped at 67 %

Measured over the glide (canonical tick 460 → finish) on 30 downloaded ghosts.
`duty %` is the fraction of glide ticks with the accelerator down; `medON` /
`medOFF` are the median held-down and released run lengths in 10 ms ticks.

| rank | time | throttle runs | duty % | medON | medOFF |
|---|---|---|---|---|---|
| 1 | 23.638 | 246 | **66.9** | 10 | 5 |
| 2 | 23.754 | 243 | 66.1 | 10 | 5 |
| 3 | 23.833 | 24 | 66.6 | 101 | 54 |
| 4 | 23.852 | 28 | 66.8 | 95 | 51 |
| 5 | 23.917 | 179 | 65.4 | 5 | 7 |
| 8 | 24.459 | 32 | 62.1 | 75 | 49 |
| 9 | 24.547 | 381 | 60.0 | 6 | 4 |
| 20 | 25.464 | 251 | 58.5 | 9 | 7 |
| 40 | 26.588 | 212 | 56.8 | 12 | 9 |
| 80 | 28.569 | 228 | 52.8 | 11 | 9 |
| 120 | 29.979 | 286 | 48.1 | 9 | 9 |
| 200 | 31.029 | 1 | 0.0 | — | 2643 |
| **this work** | **22.072** | 1143 | **68.3** | **2** | **1** |

Duty orders the field almost perfectly. **Tap RATE orders nothing**: rank 3
reaches 66.6 % with 24 throttle runs a lap (a full second held at a time) and
rank 9 only 60.0 % with 381. Two quite different human styles converge on the
same ceiling and **no human in the sample exceeds 67 %**.

### Why they cannot simply hold it down

Forcing the accelerator on over any window of the glide, on the WR's own tape,
does not finish at all:

```
gas forced ON, windows sliding over ticks 400-2363 of the WR's tape
len  20 ticks:  DEAD DEAD DEAD  +78  DEAD  +79 DEAD DEAD DEAD +511
len  50 ticks:  DEAD ... DEAD          (+487 only in the last window)
len 100 ticks:  DEAD ... DEAD          (+407 only in the last window)
len 200 ticks:  DEAD everywhere
```

Forcing it **off** over the same windows is merely slow (off from tick 2000 →
25.713). So throttle is the only live channel, more of it is faster, and too
much in one go ends the run — a traction-limit signature, and the reason the map
is a duty-cycle problem rather than a press-forward one.

**Stated as a lineage property, not a map law** (the trap this project has hit
before): rank 3 holds the key down for 101 ticks at a time and finishes. The
table above is a statement about states reachable on the WR's line.

### Steering is inert through the glide — measured as a composition, not per window

Zeroing the steer channel over the **entire** glide (tick 460 → finish) returns
the identical millisecond on the WR's tape (23.638) and on my optimised tape
(23.112). That is the composition test, so it is not the "locally inert,
globally load-bearing" trap.

It is lineage-specific: substituting the WR's keyboard steering into rank 2's
analog line kills the run at every cut point (DEAD at 200, 500, … 2000), while
substituting his **throttle** into the same line finishes everywhere from tick
500 (24.126 → 23.705). Steering matters through the start; on a settled line it
does nothing. The two channels are **not separable across the start**: swapping
whole channels between the WR's tape and mine DNFs in both directions.

---

## Where the 1.566 s is

| step | time | won |
|---|---|---|
| human WR (rank 1) | 23.638 | — |
| + uniform 2-on-1-off tap over the glide from tick 460 | **23.335** | 0.303 |
| + hill-climb on the glide throttle bits | **23.112** | 0.223 |
| + hill-climb on the start, ticks 0-470, steer + throttle | **22.277** | 0.835 |
| + five alternating glide/start rounds, converged | **22.072** | 0.205 |

**The first 4.7 seconds are worth more than the entire 19-second glide rhythm.**
The optimised start drives the WR's own line — same full-right at 0.15-0.17 s,
same full-left at 1.37 s, same simultaneous **gas + brake scrub** at 1.62 s,
same flip to full-right at 2.37 s — and differs only in throttle modulation
through the drop-in (2.34-3.09 s) and the first second of ramp. A control that
optimises the start with **steering frozen at the WR's** still reaches 22.532,
so ≈0.58 s of the 0.835 is pure throttle timing and ≈0.25 s needs the steering
to move with it.

### The tap that wins

Uniform **two ticks on, one tick off** — 20 ms down, 10 ms up, 33 Hz, duty
66.7 % — applied over the glide from tick 460 at phase offset 1: **23.335**,
straight out of a sweep, already 0.303 under the human WR. Phase is brutal:
offset 2 gives 23.476 and offset 0 does not finish. Where the pattern starts is
just as sharp: 23.335 at tick 460, 23.454 at 430, 23.488 at 490, and nothing
before tick 430 finishes at all.

---

## Mandatory follow-up 1 — how does a human do this

**A human drove 23.325**: Reddnox, the map's author, in a validation lap; the
same person holds the 23.638 online record. "Not humanly executable" is neither
available nor needed.

- **Classification: precision-bound, on a technique nobody is missing.** The
  map's name tells every player what to do — *tap* — and all 30 sampled records
  drive the same line. There is no hidden route, no unfired feature, no
  attitude trick (0.0 % airborne, 7° of roll). What separates 602 people is one
  scalar, the fraction of the descent spent on the accelerator, and the field is
  bunched against a ceiling of about 67 %. The author's validation lap is 313 ms
  better than his own online best because he caught a slightly better duty and
  phase in the editor, not because he did something different.
- **What all 602 missed, in one sentence:** the two thirds of the lap everyone
  is optimising is not where the time is — **the first 4.7 seconds are, and they
  are a throttle problem, not a driving problem.** Every sampled human,
  the author included, takes the same start line with an essentially unmodulated
  throttle through the drop-in; modulating it there is worth 0.835 s against
  0.526 s for perfecting the whole 19-second glide.
- **And the second thing they missed:** duty, not rate. Rank 3 and rank 9 differ
  by sixteen times in tap rate and 0.7 s in time, and the slower tapper is
  faster. Anyone chasing "tap quicker" is optimising the wrong variable.
- **Drivable advice, in descending order of value:** (1) tap through the drop-in
  and the first second of ramp, not only on the ramp; (2) push glide duty toward
  67 % — hold longer, release shorter — at whatever rate you can keep
  phase-stable; (3) if a slow rhythm (~1 s on, ~0.5 s off, rank 3 and 4's style)
  gives you a steadier duty than a fast one, use it, because rate itself buys
  nothing.

## Mandatory follow-up 2 — the low-input family

Searched **under** the constraint in every case; nothing here is a finished
analog tape converted afterwards.

**Alphabet.** Read off the human tapes, not invented: `{−127, 0, +127}`. The WR
and 12 of the 30 sampled records are pure keyboard runs; rank 8's steer alphabet
is `{0, +127}` — no left input at all. Every tape delivered here is keyboard.
Glide steering can be deleted outright at zero cost (measured above), so the
deliverables carry **zero steer events after tick 460**.

**Grain — the honest low-input axis on this map**, because the tape *is* a pulse
train and raw event count mostly measures the pulse rate. Constraint: no
throttle run shorter than N ticks anywhere in the glide, enforced by repairing
every candidate onto the constraint after mutation and verified by reading the
shortest interior run back off the delivered file.

| grain | best uniform (exhaustive sweep) | best searched | vs AT 23.325 |
|---|---|---|---|
| 10 ms | 23.335 | **23.112** | −0.213 |
| 20 ms | 23.783 | **23.272** | −0.053 |
| 30 ms | 24.573 | **23.173** | −0.152 |
| 40 ms | 23.627 | **23.125** | −0.200 |
| 50 ms | 24.555 | 23.578 | +0.253 |
| 60 ms | 24.268 | 23.778 | +0.453 |
| 80 ms | 25.326 | 24.178 | +0.853 |
| 100 ms | **0 of 40 180 finish** | — | — |

The uniform column is exhaustive over every period 1-60, every duty and every
phase (9 050 patterns at grain 1, 40 180 at grain 10). The searched column is
20 hill-climb rounds of 400 candidates each from the best legal uniform seed —
so it is a lower bound on each row, and the non-monotonicity between 20 ms and
40 ms is search budget, not physics.

Read together: **the author time falls to a 40 ms input grain**, which is inside
what the field already does (rank 5's own medians are 50 ms on / 70 ms off).
Beyond that the constraint bites hard, and at a 100 ms grain no uniform rhythm
gets down the ramp at all. Conversely the extra 1.05 s from 23.125 to 22.072
does require 10 ms control: a grain-4 search of the **start** found no
improvement at all over the WR's own start throttle, so the 0.835 s the start is
worth is entirely sub-40 ms work.

**Event count.** `tmsimp --mode ddmin` under budget 23.325, from the 22.072
tape: terminates **1-minimal at 747 events → 23.183**. 1-minimal licenses the
strong statement: *no 746-event version of this line beats the author time*
(within the block-deletion move set, which is what ddmin certifies). From the
23.112 tape the same procedure landed at 830 events → 23.308. For scale the
human WR uses 271 events and 23.638.

---

## Negatives, each with its enumeration

- **No embedded author ghost.** `ct probe`, `ct mapghost`, `rec nodes`, all
  negative. Positive control: the container agent's survey, where the same tools
  find ghosts on 238835 and 286279.
- **Input tapes are NOT portable between containers on this map.** 30/30
  self-identity; **10 of 10** cross-container transplants tried (rank 1's tape
  into the containers of ranks 2, 3, 4, 5, 9, 12, 80, 200, 300, 470) return
  `wrong simu`. Alignment is not the cause — tapes are normalised to the
  race-start timeline first (offsets here run −1580 ms to 0, so index alignment
  alone shifts a tape by up to 158 ticks, which is what this looked like at
  first) and self-identity is exact after normalisation. Not bisected further:
  the workaround (splice inside one container) is free.
- **`fk traj` cannot observe this map.** The vehicle state is located perfectly
  — 1.2 mm RMS against the reference ghost's own telemetry, four LIVE addresses
  agreeing — but **no u32 within ±80 KB of it advances by exactly 10 every
  tick**, and none advances monotonically by 10-or-0 either (relaxed criterion,
  ±36 KB, 400 samples, 0 candidates). The tool hard-aborts, correctly. So this
  map has per-tick positions available but no per-tick *labels*; all the physics
  above was done by ablation through the plain oracle instead. Anyone who needs
  trajectories here should locate the clock behaviourally over a much wider
  window or by a different signature.
- **No gate ruler.** `tmmaps probe` in fleet build v5 panics on this map
  (`probe.rs:62`, index out of bounds), and neither parallel fix named in
  `FLEET_NOTICE_v4_..._v2.md` (`probe --keep-model`, `moveitem`/`ladder`) is in
  v5. Not pursued — ablation answered the same questions.

## Hazards this map added, both of which generalise

- **A container's tail is neither zero slack nor unlimited — measure it.** Every
  ghost here ends within 10 ms of its own finish, so the first reading was "no
  slack at all", and a 174-pattern `tmsimp --mode metro` sweep came back
  **0/174**, which looked exactly like a law of nature. It was the harness: the
  simulation runs on past the end of the input tape holding the last input, and
  rank 1's container in fact tolerates **+2.1 s** (gas off from tick 2000 →
  25.713; from tick 1800 → dead). Measure the slack before believing an all-DNF
  sweep. The same map then produced a *real* all-DNF sweep (40 180 patterns at a
  100 ms grain) that is physics — which is exactly why the first one has to be
  ruled out by measurement rather than by intuition.
- **Normalise tapes to the RACE-START timeline before splicing anything.**
  `start_offset_ms` varies by 158 ticks across one map's own ghosts, and index
  alignment silently produces a wall of DNFs.
- **A constraint that is not read back off the artefact is not a constraint.**
  My first minimum-dwell repair produced tapes that reported "grain 3" and
  contained 2-tick runs. The check that caught it was printing the shortest
  interior run of the *delivered file* next to the constraint, and it cost one
  line.

## Artefacts — `~/persistent/private-30d/tm-unbeaten/173636/`

All tapes are in rank 1's container, all validate on the untouched
`map.Map.Gbx`, all confirmed on two cold passes with human controls in the
batch.

| file | what | oracle |
|---|---|---|
| `tapes/it5s_optfinal_22072.Ghost.Gbx` | **the result** | **22.072** |
| `tapes/it1g_optfinal_22122.Ghost.Gbx` | one round earlier | 22.122 |
| `tapes/s1_optfinal_22277.Ghost.Gbx` | glide + start, first pass | 22.277 |
| `tapes/o2_optfinal_23112.Ghost.Gbx` | glide only, converged | 23.112 |
| `tapes/o1_optfinal_23141.Ghost.Gbx` | glide only, round 1 | 23.141 |
| `tapes/d1_duty_p3_on2_off1_23335.Ghost.Gbx` | WR start + uniform 2-on-1-off | 23.335 |
| `tapes/grain20ms_…23272` … `grain80ms_…24178` | the minimum-dwell ladder, constraint HELD on each | 23.272 / 23.173 / **23.125** / 23.578 / 23.778 / 24.178 |
| `tapes/ddmin_747ev_23183.Ghost.Gbx` | 1-minimal event set under the AT | 23.183 |
| `tapes/ddmin_830ev_23308.Ghost.Gbx` | the same from the 23.112 lineage | 23.308 |
| `tools/tx.rs` | experiment driver: `probe ident hold window duty ablate splice allsplice opt keys tapstats write` | |
| `tools/tw.rs` | trajectory-CSV analysis | |
| `duty60.txt` | the exhaustive uniform sweep, all finishers | |
| `PLAN_v1.md`, `RESULTS_173636_v1.md` | plan, and the superseded sidecar | |

Rust only throughout; no python anywhere in this work. Nothing was submitted to
any Nadeo leaderboard.
