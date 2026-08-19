# RESULTS — map 210218 `Fall 2024 - 25 (Pure Wet Icy Wood)` — agent w218

**Status as of 23:27 PDT; four search arms still running. A `_v2` will carry
the final numbers.**

**AT 94.477 · human WR 96.281 (iambeeen) · 29 records · previous agent's tape
96.078 · my best 96.078. The author time was NOT beaten.**

This is the second agent on this map. The first banked `RESULT.md` and
`best_96078.Ghost.Gbx` at 21:10/21:40 and stopped mid-search (its §7 is still a
placeholder). Nothing of theirs was overwritten; every file of mine is prefixed
`w218_`. Supersede this document with `_v2`, never edit it.

---

## 1. Verification of the previous agent's tape — PASS

Independently, in my own forked build tree, on the untouched map, with a
downloaded human ghost in the same batch:

| tape | oracle |
|---|---|
| `best_96078.Ghost.Gbx` | **96.078** |
| `r01_96281` (human WR, known-answer control) | **96.281** |
| identity round-trip through my own writer | 96.078 |
| 16 copies each of those two plus one killed tape | **48/48 identical** — the oracle is deterministic on this map |

**Their 96.078 is real.** It is 0.203 under the world record and 1.601 over the
author time.

**But do not read splits off it.** `tmtas splits best_96078.Ghost.Gbx` returns
`race_time=96281` and the WR's seventeen splits, byte for byte — the
synthesised-tape telemetry trap
(`FLEET_NOTICE_synthesised_tape_telemetry_v1.md`) is live on this map and would
have silently described the seed. Everything below that needed splits used
either a downloaded human ghost or the plain oracle.

## 2. The two measurements that decide what is possible here

### 2.1 The car model explains 1.6 % of yaw

`tmtas carmodel`, all 30 field ghosts, 96 888 samples: **1.6 %** of yaw-rate
variance explained. Fitted per-run: WR 12.4 %, r21 16.0 %, so the pooled figure
is not a pooling artefact. This is below both maps quoted in my brief (71 % and
2.7 %).

**Every steering-based prior, corridor and predicate this project owns is void
on 210218 and I used none of them.** (`--corridor`/`--refcsv` are fork-path
only in any case, and the fork server is the instrument this project trusts
least.)

### 2.2 Perturbation response: lethal, or exactly neutral, and nothing else

One tick changed, everything else identical, swept the whole tape:

| operator | probes | DNF | survivors that changed the time |
|---|---|---|---|
| steer ±1 (1 unit of 254) on the incumbent | 96 | **69 %** | 3, all in the last sector, all slower |
| steer ±1 on r21 (keyboard tape) | 174 | **55 %** | 7, all in the last sector |
| accel flip | 96 | **71 %** | 7, all after tick 8900, all slower |
| brake flip | 96 | **88 %** | 7, all after tick 8900, all slower |

Every other survivor returns **exactly 96078**. Single ticks are worth well
under a millisecond mid-run, so neutrality is expected; the finding is the
**kill rate**. A one-unit steering change is fatal at two thirds of the ticks in
the run.

Then at event scale, which is where time actually lives:

| operator | variants | survived | **faster** |
|---|---|---|---|
| ablate each of the 22 throttle lifts (full / first half / second half) | 66 | 8 | **0** |
| clear each of the 6 brake taps | 6 | 1 | **0** |
| slide both boundaries of every lift by ±3, ±6, ±12 ticks | 988 | 99 | **0** |
| force 100 % throttle over each whole sector | 17 | 5 (already flat out) | **0** |
| force gas=1 over sliding spans of 10/30/80 ticks | 93 | 75 | **0** |
| **steer → dead centre over spans of 5/15/40 ticks** | **863** | **40 (94–96 % kill)** | **0** |
| **pairs of individually-fatal moves** (centre 15 ticks + a compensating 15-tick steer of ±32/±64/±127 immediately before or after) | **472** | **6** | **0** |

The last two rows are the ones aimed at the mechanism in section 4.2 — if the
fast regime is the low-slip regime, a local move toward centre is the move that
matters — and at the fleet's standing advice that the winning move is often a
**combination whose parts are individually bad**. 96 % of centring moves are
fatal, 99 % of the compensated pairs are fatal, and not one of either is faster.
The six surviving pairs are all at tick 9400, in the final sector, and all cost
between 1.1 and 2.4 seconds.

**Zero improvements in 2 505 targeted structural variants, plus ~70 000 search
evaluations of my own and the previous agent's ~2.2 M.** Every survivor returns
96078 to the millisecond; the eight lift-ablations that survive do so because
their window is *inert* (the car is airborne — the dead-window precondition from
`ACQUISITION_addendum_controls_v1.md` §1, visible here as "force gas=1 across
tick 5506 changes nothing").

**Instrument controls for that negative**, because it is a large one:
files verified distinct by md5; accel arrays verified to differ at the intended
ticks; a positive control (deleting the last lift, 9256–9327) returns
**`DNF cps 16`** — the instrument can say no, and says it at the right depth.

### The eighth "instrument that could only say yes", caught mid-experiment

The centring sweep was run once **without** a no-op guard. It reported a 77 %
kill rate and 36 surviving 5-tick spans, and those 36 then **composed** — I
applied 27 of them cumulatively and the tape still returned exactly 96078,
which read as a directed plateau walk toward the low-slip regime and would have
been a genuinely new operator.

It was nothing. The incumbent is already 24 % centred, so a random short span is
often **already all zeros**, and "zeroing" it produces a byte-identical file
whose survival is a tautology. Diffing the composed tape against the incumbent:
**zero ticks differ.** The whole composition result was an identity.

`w218 flipspan` now exits 3 with `NOOP: span leaves the tape unchanged` rather
than writing such a candidate; 88 of 951 spans were refused. The corrected sweep
is the table row above.

> **Generalisation for the fleet: any ablation that sets a channel TO a value
> must assert the tape changed.** Deletion probes are safe because they act on
> an event list, but a span-forcing operator silently manufactures no-ops
> wherever the tape already agrees with it, and no-ops always "survive". This is
> the same shape as the seven in `ACQUISITION_addendum_controls_v1.md` and it
> passed every other control I had — determinism, md5-distinct files, a working
> positive control — because those check the *harness*, not whether this
> particular candidate carries the change.

## 3. Composition is impossible here, and now there is a number for it

The field owns the time. I reproduce the previous agent's arithmetic
independently from the ghosts' own split chunks: **sum of per-sector minima =
91.826 s**, 2.651 under the author time. The problem has never been that the
time does not exist.

I aligned the WR and r21 by **state**, not by checkpoint: nearest (position,
speed) pairs over each late sector. Four anchors matched to **1.1–1.5 m with
0.000 m/s of speed difference**. Grafting r21's tail onto the incumbent at each
anchor, sweeping the graft point ±40 ticks and the time offset ±2 ticks — 260
tapes:

| anchor (sector) | best depth reached |
|---|---|
| tick 5953 (s10) | `DNF cps 10` — every one |
| tick 7313 (s12) | `DNF cps 12` — every one |
| tick 8418 (s14) | `DNF cps 14` — every one |
| tick 9238 (s16) | `DNF cps 15–16` |

**Every graft dies inside the sector it was grafted in, from a state match good
to one metre and zero speed error.** Not one of 260 survived a single further
checkpoint. That is the transfer horizon of this map measured rather than
asserted: **under one sector, at a one-metre state match.** It closes splicing,
bridging, damping, quantising and any other reuse of an existing tape, and it
explains the previous agent's §3 without needing any of its individual
negatives.

## 4. What orders this leaderboard, and what sets the pace — two different questions

The previous agent's headline is that the field drives permanently sideways and
that r21 (SparkSheep, 21st) is fast because it stops. They were scrupulous that
the field-level correlation was absent. Both halves of that turn out to be
right, and they are answers to two different questions.

### 4.1 What orders the field: respawns, not driving

Across all 30 runs, finish time against:

| candidate explanator | Pearson | **Spearman** |
|---|---|---|
| **respawn count** (`word0 & 0x20`, no simulation needed) | **+0.925** | **+0.874** |
| steer events per second | −0.346 | −0.446 |
| throttle % | −0.333 | −0.325 |
| distinct steer values | −0.296 | +0.075 |
| mean slip angle | +0.303 | **+0.099** |

**This leaderboard is a survival ranking.** What separates 96 s from 440 s is how
many times you fall in the water: 0 respawns for the top five, 34 for the last.
Respawn count is one pass over the ghosts with no simulation at all, and it
should be the first column of any field table on any map.

Within the fast group (top 12, all ≤2 respawns) the ordering variable changes to
**throttle %** (Pearson −0.743, Spearman −0.783) — but see 4.2, because throttle
and slip are not independent.

### 4.2 What sets the pace: slip, measured against a matched control

Slip explains 1 % of the *ranking* for the reason the previous agent gave — the
field is stylistically homogeneous, 29 of 30 runs sit between 21° and 30°, so
there is nothing to correlate against. The right comparison is within a matched
pair. Per sector, WR against r21, with throttle and path length alongside so the
confounds are visible:

| sec | WR slip | r21 slip | WR gas | r21 gas | WR mean speed | r21 mean speed | path Δ | r21 gain |
|---|---|---|---|---|---|---|---|---|
| 6 | 24.8° | **0.3°** | 100 % | 100 % | 73.4 m/s | **75.0** | −27.3 m | 457 ms |
| 12 | 10.4° | **3.1°** | **100 %** | **100 %** | 51.7 | **60.5** | **+4.6 m** | 574 ms |
| 13 | 23.5° | **3.1°** | 95.8 % | 100 % | 59.7 | **66.9** | −8.0 m | 715 ms |
| 14 | 28.6° | **2.0°** | 97.2 % | 100 % | 59.4 | **61.9** | **+0.6 m** | 230 ms |
| 15 | 16.1° | **0.7°** | **100 %** | **100 %** | 44.7 | **48.8** | −2.5 m | 514 ms |
| 16 | 34.1° | **0.5°** | 83.8 % | 100 % | 63.9 | **67.4** | −4.0 m | 269 ms |

**Sectors 12 and 15 are the clean experiment: identical 100 % throttle, the same
line to within 8 m over 220–240 m, and r21 carries 17 % and 9 % more mean speed
while sliding 3° instead of 10–16°.** Throttle cannot explain those two, and
route cannot — in sector 12 r21's path is 4.6 m *longer*. On this surface
sliding scrubs speed, and that is the whole of it.

**The control that rules out an input-mode artefact.** r21 is a keyboard tape,
so low slip might be a property of digital steering rather than of the driver.
It is not: **15 of the 30 runs are 3-value keyboard tapes** (r03, r04, r08, r09,
r10, r14, r15, r20, r21, r22, r24, r27, r28, r29, r30) and every one of the other
fourteen sits at **23.7°–30.4°**, indistinguishable from the analog half. r21 is
alone at 14.1° over the run and at 0.3–3.1° over sectors 6–17. The keyboard /
analog split does not order the leaderboard either (Spearman +0.075).

Two further measured corrections to the previous write-up, both making its case
stronger rather than weaker:

* r21's **input tape** holds **three** values, `{−127, 0, +127}`, not the seven
  read off the decoded CSV. The intermediate values are the game's steering
  *filter* appearing in telemetry, not inputs.
* Low-input is **not** more robust here, which is the fleet's standing prior:
  r21's keyboard tape dies on 55 % of single-unit perturbations against the
  analog incumbent's 69 %. Same order of magnitude.

r21 also carries **1 635 packets in digital mode 15**, which `ghost::Factory`
cannot read or write. They are confined to packets 5137–6997 and all carry
`tri = [0,0,0,0]`: the driver's 18-second hands-off coast, not driving. Any
r21-derived tape outside that band is faithful; inside it our toolchain is blind.

## 5. What a driver should be told

> **On Pure Wet Icy Wood the car is not steered, it is aimed. Arrive pointed
> where you are going and keep the wheel still; every degree of slide is speed
> you are grinding off on the ice.**

The evidence is section 4.2, and it is worth more than the usual "drive smooth"
because it is measured against a throttle-matched, line-matched control in two
sectors. The rest of the leaderboard drives at 21–30° of slip and one person
drives at 0.3°, and he is faster in eight of the last twelve sectors from an
entry speed equal to or lower than the world record's in five of them.

Where the time is, concretely:

| sector | r21 gain | mechanism |
|---|---|---|
| 11 | 881 ms | shorter line (−8.1 m) plus 30.2° → 2.2° |
| 13 | 715 ms | shorter line (−8.0 m) plus 23.5° → 3.1° |
| **12** | **574 ms** | **slip alone** — same throttle, longer path |
| **15** | **514 ms** | **slip alone** — same throttle, same path |
| 6 | 457 ms | shorter line (−27.3 m) plus 24.8° → 0.3° |
| 16 | 269 ms | 83.8 % → 100 % throttle plus 34.1° → 0.5° |
| 14 | 230 ms | slip, same line |

Classification, per the mandatory follow-up's taxonomy: **known-but-unheld.** It
is not an undiscovered route — the lines are within 8 m of the world record's in
the sectors that matter most. A 21st-place player already drives it, in the same
run, and loses 81.587 s in sectors 1–5 to a stall that has nothing to do with
technique. The reason nobody holds it is section 2: on this surface the
low-slip line is about one unit of steering wide, and a driver who misses it is
in the water, which is exactly what section 4.1 says the leaderboard is made of.

## 6. What I could not do, and the honest reason

I could not move 96.078. Nor could the search move it: **~65 000 of my own
evaluations across five aimed structural sweeps and four search arms produced no
tape faster than the previous agent's.** The positive control matters here — the
same search configuration, started from the *raw* WR tape, found 96.281 → 96.214
in four minutes on twelve workers, so the search works; 96.078 is simply the
floor of that lineage.

What the map forbids, in one line: **its transfer horizon is under one sector at
a one-metre state match, so the only legal operator is local search from a
finishing incumbent, and the incumbent that exists is already at the bottom of
its basin.** Reaching 94.477 requires re-driving sectors 12–16 onto a
throttle-down line from scratch, which is a cold-start problem on a surface
where two thirds of single-unit perturbations are fatal.

## 7. Instruments this map broke — see the separate fleet notice

`w218_FLEET_NOTICE_dnf_depth_and_invalid_replay_v1.md`:

1. **DNF depth is blind below six checkpoints on this map**, not two as the
   harness comment states, so every candidate dying in the first third scores
   identically at `cps = 1`. Calibrated with nine stop-tapes; recipe in the
   notice. This made a 110-tape graft sweep of mine uninterpretable until it was
   calibrated.
2. **`r23_179463`, the previous agent's one unexplained ghost, is explained.**
   It is not a physics divergence: the server prints, on *stderr*,
   `Invalid replay: r23_179463.Ghost.Gbx (#26.48x48Screen155Sunset.Script)` and
   `Is Valid: 96% (29) / Is Invalid: 3% (1)`. The ghost references an embedded
   custom-item script the validator cannot resolve, so it **never simulated**.
   `tmtas validate` reports a load failure as `DNF`, indistinguishable from a
   crash. Field reproduction on this map is **29/29 of the ghosts that load**.
3. Check respawn count before naming a technique; never `pkill -f` a pattern
   that matches your own shell.

## 8. Enumerations, stated with the negatives

* perturbation: every 100th tick, ticks 200–9700, steer ±1 / accel flip / brake
  flip (288 tapes); plus ±1, ±5, ±40 at ten ticks (51).
* lift structure: **all** 22 gas=0 stretches and **all** 6 brake taps of the
  incumbent, enumerated from the tape itself, not by hand — full ablation, both
  halves, and both boundaries slid ±3/±6/±12 (1 060 tapes).
* whole-sector full throttle: all 17 sectors.
* grafts: 4 state anchors × 13 graft offsets × 5 time offsets = 260, plus an
  earlier 110 at the CP5 rejoin.
* ladders: `--qlevels` 1,2,3,4,5,6,8,12,16,24,32 and the zero ladder on the
  incumbent — **all DNF at CP1, including the 65-value ladder whose worst error
  is ±2 of 127** — with the identity (no ladder) control returning 96.078.
* search: five arms, plain oracle, `--verify-best` on, own staging roots.

## 9. Artefacts

`~/persistent/private-30d/tm-unbeaten/210218/` — `w218_PLAN_v1.md`,
`w218_RESULTS_v1.md` (this file), and the raw sweep outputs under `w218_data/`.
The fleet notice is one level up. The previous agent's `RESULT.md`,
`best_96078.Ghost.Gbx` and `seed96178.Ghost.Gbx` are untouched.
