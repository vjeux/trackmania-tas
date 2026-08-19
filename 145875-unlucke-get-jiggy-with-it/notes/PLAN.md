# Map 145875 — "unluckE - get jiggy with it" — attack plan

uid `_GsJKvxawnKoIgkiWCpy9tRIMM0` · Nadeo mapId `56c24403-891e-4ffc-a9f0-2bd9ff98ae27`
· author **InfTM** (Koblenz, DE) · uploaded 2023-12-09 · AT **6343** · best human
online WR **6346** (xeap-.-) · 46 records on the board.

Everything below is measured on this map on 2026-08-18, not assumed.

## 1. Acquisition and the identity control — PASSED

Followed `ACQUISITION.md` exactly (fwdproxy, descriptive UA, 1.7 s between ghost
downloads). Map file + top-15 ghosts.

`tmtas validate --map <ABS map> --jobs 15 <ABS ghosts>`: **15/15 re-simulated to
their exact recorded millisecond** (6346, 6350, 6360, 6373, 6380, 6385, 6408,
6413, 6424, 6440, 6442, 6448, 6452, 6474, 6478). Map loads, ghosts decode, the
oracle agrees with the online board.

Candidate factory round-trip (`tmsearch --verify`, then validate): 6346 exactly.
Template is 789 input ticks (10 ms each); the finish falls at tick ~635.

## 2. The medals say the AT is a driven lap — REACHABLE

| medal | ms | gap to AT |
|---|---|---|
| author | **6343** | — |
| gold | 7000 | +657 |
| silver | 8000 | +1657 |
| bronze | 10000 | +3657 |

Gold/silver/bronze are round numbers to the second: the author hand-typed them
and did not care. In the TM2020 editor the **author medal is not editable** — it
is whatever the author's own validation run did. So 6343 is a lap InfTM
physically drove, and it is 3 ms better than the best of 46 online attempts by
other people. Nothing formula-generated, nothing impossible. This is the good
case: a human already did it, so the physics admits it.

(Contrast: the maintainers of unbeaten.at dismiss 387 of 420 unbeaten ATs as
cheated/plugin/impossible. This one survives their curation, and the medal
pattern is consistent with that.)

## 3. What kind of map this is — decoded, not guessed

**No checkpoints at all.** `tmmaps list` finds exactly two waypoints: block#0
`PlatformTechStart` (Spawn) and the Goal — block#2612 `GateExpandableFinish`
plus items #130/#131 `GateFinishCenter8mv2` at (1230,158,820) and (1232,158,820).
2620 blocks, 132 items. Every ghost declares a single split == its finish time.

Consequence for search: **a DNF carries no information from the validator** —
there is no checkpoint ladder to fall back on, so `score_dnf` is a flat plateau.
Reward shaping via segment maps is not available for free. If DNF gradient is
needed it must come from the fork-server progress measure (arclength along the
incumbent's own measured line), not from checkpoints.

Trajectory of the WR (r01, 6346), decoded from `CPlugEntRecordData`:

| phase | t (ms) | what happens | speed km/h | y (up) |
|---|---|---|---|---|
| S0 | 0–1200 | standing start on a very steep downslope, full LEFT held, wheels on the ground | 1 → 98 | 137 → 120 |
| S1 | 1200–2200 | airborne, free-fall, steer flips to full RIGHT at 1.4 s | 98 → 233 | 120 → 76 |
| S2 | 2200–3400 | back on a surface, gear 1→4, steering unwinds to ~0 | 233 → 292 | 76 → 42 |
| S3 | 3400–3500 | the low point, brief contact, the car turns onto the exit line | 292 → 270 | 42 (min) |
| S4 | 3500–6346 | **airborne the whole way, rolled ~180° (inverted), CLIMBING and ACCELERATING** | 270 → **601** | 43 → 158 |

Gas is 1 and brake is 0 for **every tick of the whole run** in the human WR.
Steering is at or near full lock almost everywhere (a keyboard-like run): the
252 intermediate steer values are unused, which is exactly the structural edge
this toolchain has exploited before.

The S4 signature — wheels fully extended (`*_dampen` pinned at +0.196 = no
load), roll ≈ ±π, and speed rising from 270 to 601 km/h while gaining 115 m of
altitude — is not ballistic. The car is under continuous thrust along its own
forward axis for the last 2.8 s. **So in S4 the car's orientation IS the
throttle direction**, and steering (yaw/roll in the air) points the thrust
vector. That is where a TAS wins: aiming 2.8 s of thrust slightly better.

## 4. Where the time is — measured, 930 unbiased samples

`tmsearch --dump 930` from the human WR (unbiased mutations, ticks 0..660,
window 140), scored by `tmtas analyze-dump`:

- **41.4 % of mutations still finish.** This map is far more forgiving than
  map 2 (21–43 % there from a *converged* run). There is a real gradient.
- **1.51 % of single moves IMPROVE on the human WR** (14 of 930). For
  comparison, a converged incumbent on map 2 improved on 0.01 % of 15,900
  samples. This map is nowhere near flat — the human field of 46 has not ground
  it down.
- **Best single random move: −8 ms.** One unbiased mutation already produces
  6338, i.e. **below the 6343 AT**. The AT is not the hard part.
- Improvements come from ONE place:

  | tick range | ms | n | improve rate | best |
  |---|---|---|---|---|
  | 0–459 | 0–4590 | 792 | **0.00 %** | — |
  | 459–525 | 4590–5250 | 98 | 6.1 % | −8 ms |
  | 525–591 | 5250–5910 | 27 | 22.2 % | −3 ms |
  | 591–657 | 5910–6570 | 13 | 15.4 % | −3 ms |

  Every single improving move is in **S4 after 4.59 s** — the last third of the
  flight, the part that is pure thrust-vector aiming. Nothing before it moved
  the clock at all in 792 samples.

- By operator: `edge` (move a steer transition) 5.05 % improve / best −8;
  `lvl` 2.24 %; `cos` 0.90 %; `acc`/`brk` (throttle/brake toggles) **0 % of
  128** — consistent with gas-always-on being already optimal, and with brake
  in the air only costing thrust.
- By span: short moves win (0–5 ticks: 7.7 % improve). Spans ≥ 80 ticks finish
  4–9 % of the time and never improve — same "big moves die" law as map 2.
- Expected best-of-batch: 20 → −0.7 ms, 100 → −3.5, 400 → −6.4, 800 → −7.4 ms.
  Diminishing but not saturated at 800.

## 5. Therefore — the plan

The 3 ms gap is *not* the problem here; one random move clears it. The problem
is to find how much is actually on the table and to land it validated. So:

1. **Beat the AT first and bank it** (a single confirmed sub-6343 replay), so
   the deliverable exists early. Expect this in minutes, not hours.
2. **Search hard, seeded from r01 (6346)**, with the window biased to S4
   (ticks 430–660) where 100 % of the measured gains live, plus a slower global
   arm so nothing is assumed. Metropolis temperature must be **small** — gains
   here are 1–8 ms, so map 2's T = 25 ms would be a random walk; T ≈ 2–4 ms.
   Short spans, `edge`/`lvl`-heavy operators.
3. **Population seeding.** The 15 human runs spread 0.47–10.21 m RMS laterally
   (median pair 3.69 m) — unlike map 1/2 they do *not* all drive one line. Run
   independent islands from several distinct seeds (r01, r03, r04, r15 are the
   extremes of the geometric spread) in case the WR's line is not the best
   basin. Cross-splicing is known not to work; distinct seeds are the way.
4. **Sub-tick vernier last.** Once the tick-level plan is optimal, the finish
   time is decided by *where inside the crossing tick* the car passes the plane.
   At 601 km/h the car covers 1.67 m per tick, so 1 ms = 0.056 m. `tmmaps probe`
   (movable finish plane, ~0.7 µs) measures the crossing geometry directly and
   tells us how much of the remaining fraction is sub-tick positioning rather
   than a better line.
5. **Fork server**: all gains are at tick ≥ 459 of 635, so resuming at tick
   ~420 skips 66 % of the simulation. That is the right shape for `fk fs`, and
   the DNF-progress measure also replaces the missing checkpoint ladder. Adopt
   it only after a measured head-to-head against the classic loop on this map
   (PROTOCOL: concurrent control, same box).

Predicted worth, stated before measuring: S4-focused window ≈ 2–3× the global
arm's rate of improvement; multi-seed islands ≈ insurance, probably 0 unless
r01's basin is bad; fork+predicates ≈ 1.5–2.5× throughput; sub-tick vernier
≈ the last 1–3 ms.

## 5b. INCIDENT, ROOT-CAUSED: "phantom finishes" here were a shared staging root

First two arms launched (a1, b1, concurrent, 15 min) produced five sub-AT bests
in arm a1. Re-validated through the plain oracle: **4 of 5 were phantoms**
(three `DNF`/"wrong simu", and `best_6343` re-simulated to 6346 — the template's
own time, i.e. an unmutated tape credited with a mutated tape's score). Arm b1's
bests were fine. Specimens preserved in
`~/persistent/private-30d/tm-loop/phantoms/m145875_20260818/`.

**Cause: I ran two `tmsearch` processes without `--root`, so both used the
default `/dev/shm/tmsearch`.** Every worker directory is named by index, so
a1's `w007/UserData/Replays` and `c007/c0000.Ghost.Gbx` are also b1's: each
process's server validated whichever tape the other had just written, and the
resulting time came back attached to the wrong state. The process that starts
second also `remove_dir_all`s the root out from under the first, which is why
one arm takes nearly all the damage.

Controlled, on this box, same two configurations, 3 min, 40 workers each:

| staging root | bests written | re-validate exactly | phantoms |
|---|---|---|---|
| **shared** `/dev/shm/tmsearch` | 13 | 6 | **7** |
| **distinct** `/dev/shm/tms_a`, `/dev/shm/tms_b` | 8 | **8** | **0** |

So the classic (non-fork) oracle path on this map is trustworthy *provided each
concurrent process owns its staging root*. This is operator error, not an oracle
fault — and it is the exact failure `fk`'s `default_work_dir()` was changed to
prevent on 2026-08-18; the search side had been left behind.

**Fixed in the tool** so it cannot recur: `tmsearch`'s `--root` now defaults to
`/dev/shm/tmsearch-<pid>`, and `claim_root()` writes a `.owner` pid file and
*aborts* rather than wiping a root owned by a live process. Patched sources:
`tmtas-rs-src-patched.tgz` in this directory.

Note for the map-2 phantom investigation: `tm-loop/bench.sh`, `benchc.sh` and
`benchg.sh` all pass `--root "$OUT/root_${name}_${s}"`, so the *scripted*
map-2 benchmarks did not have this defect. Any ad-hoc concurrent `tmsearch`
invocation there did.

## 6. Non-negotiables carried into this map

Never submit to a Nadeo leaderboard. Re-validate every claim with
`tmtas validate --map <ABS> <ABS>`. A failed re-validation is a STOP and the
specimen goes to `tm-loop/phantoms/`. Rust only. Rate-limited, honestly
identified HTTP.
