# 146612 `Spaghetti Nights 2` — the tail, the instrument, and the sector audit

Agent `w612`, 2026-08-19, node 145855 (80 cores). Map sha256
`c6cca762e167eba6e969c07f306798c29c88d0da397b4744d4042c51b21526db`, uid
`jchzEcocJbNJreH4ebIoUYOt286`. AT **38.530**, human WR **40.223**, 181 records.

Dispatched to build a sector-5 gradient. Built it, then the coordinator moved
sector 5 to another arm and gave me the rest of the map. Everything below is
prefixed `w612_`; I touched nothing of the previous agent's.

---

## Headline

**The author time did not fall. Three things that were unknown at the start of
the night are now measured:**

1. **Sector 5 has a gradient, on the untouched map, with no model swap.** The
   previous agent's blocker is closed, and by a different mechanism than they
   or the fleet expected: **a relocated Goal gate is a PLANE, and its axis is
   the `dir` byte.** That also explains the fleet's standing "roughly a third of
   well-chosen placements are silent".
2. **Every tape this project has produced on this map is human rank 2, to the
   millisecond, up to CP4** — 70 % of the lap. The instrument that would have
   shown this (`tmtas splits`) reads the ghost header and reports the template's
   splits, so nobody had seen it.
3. **Sectors 0–3 hold 326 ms against the best human driving that exists**, and
   the sector-4 jump — worth 1.128 s to CP5 — **cannot be banked**, because a
   ballistic flight changes travel heading by exactly zero. So the map needs
   about a second of TAS headroom above every human, and the first measurements
   of that headroom are in §5.

---

## 1. Controls, first

| control | result |
|---|---|
| all seven inherited tapes + human WR + rank 2 + rank 13, untouched map | **exact**: 39961 / 39973 / 40040 / 39996 / 40001 / 40058 / 40223 / 40226 / 42768 |
| `JUMP_cp5_32702_v1`, untouched map | `DNF cps=5` — the previous agent's central claim, reproduced |
| map sha256 and all seven tape md5s vs `RESULT.md` | identical |
| six segment maps, `--order 439,494,440,633,492`, vs reference ghost | all six `exact=true`: 7.311 / 15.718 / 19.980 / 27.834 / 33.584 / 40.223 |

Every number below was produced with an in-batch known-answer control, and every
tape was cold-validated after the fact (§6).

## 2. The instrument: a Goal gate is a plane, and you can rotate it

The previous agent could not make a gate fire mid-sector-5 and left it as the
map's blocker. Two things were in the way and neither was the cell encoding they
suspected.

**(a) This map has four spare Goal gates and nobody had looked.**
`TMMAPS_NO_BAKED=1 tmmaps list` shows **nine** waypoint blocks, not five: the
four finish road blocks *and* four `GateExpandableFinish` blocks (`#2652`–`#2655`)
sitting on top of them, all tagged `Goal`. Moving one is a **three-byte
overwrite** of its cell in chunk `0x0304301F` — no model swap, no promotion, no
Id-table change, and the road block underneath stays put so there is no hole in
the track. `FLEET_NOTICE_origin_control_insufficient_v1`'s trigger-volume
question never arises because nothing about the gate changes except where it is.

No segment map is needed. Sector 5 is downstream of all five checkpoints, which
is exactly the case
`FLEET_NOTICE_val_gate_relocation_false_negative_v1` says works — and the
previous agent's failed identity probe (park the gate at CP1's position, read
back CP2's time) is that notice's failure mode exactly, because CP1's cell is
before any checkpoint is collected.

**(b) The gate is a plane and it was pointing the wrong way.** Predicted from
the world record's own trajectory *before* the maps were built:

| gate | predicted crossing | ladder said | error |
|---|---|---|---|
| `36,13,25` dir 0 | z=816 at 34.631 | **34.608** | −23 ms |
| `36,13,23` dir 0 | z=752 at 33.815 | **33.791** | −24 ms |
| `36,13,25` dir 3 | x=1168 at 34.869 | **34.802** | −67 ms |
| `36,13,26` dir 3 | x=1168 at 34.869 | **34.818** | −51 ms |

`dir 0/2` is a **z-plane** at `z = 32cz+16`; `dir 1/3` an **x-plane** at
`x = 32cx+16`; the lead is the car's nose. `dir` is the byte immediately before
the cell bytes — a rotation, same model, same volume. **The same cell fires at
one orientation and is silent at the other.** This map's gates ship `dir=3` and
its last straight runs in −x, so every unrotated rung on a north–south straight
was silent, which is what my first uncalibrated ladder showed.

Sidecar: `ACQUISITION_addendum_146612_gate_plane_orientation_v1.md`.

**The resulting ladder** (`w612_ladder_narrow_v2.txt`) is 22 rungs from CP5 to
the finish, control OK for five ghosts, 22 rungs → 22 distinct hashes, monotone
arrivals for all four finishers. Three of its rungs fire with the car
**airborne** (`is_ground_contact=false`, 10 m up, gates resting on nothing) —
which falsified a fleet notice claiming relocated gates never fire in mid-air;
that notice has been retracted.

## 3. The decoy that got past every control

My first rungs were 4-cell curtains. The first march winner crossed the z=816
plane at **34.405, apparently 316 ms ahead of our best tape.**

It was the car **off the right-hand side of the road at x=1208, airborne,
y falling 43 → 12**, having left the track 40 m earlier. The road there is one
cell wide (`cx=36`, `RoadTechNarrowSide`); cells 34/35/37 at that height are wall
and platform.

Origin control: passed. Distinctness: passed. Monotonicity: fine. Plain-oracle
agreement on every full-map claim: fine. **Nothing in the instrument's output
distinguished it.** It was caught by putting the winner through
`fk btraj --allow-dnf` and looking at where the car actually was.

Two rules follow, and the second is the general one: **make the rung as narrow as
the road**, and **`btraj` every march winner before believing its number.** A
ladder measures "did the car cross this plane", which is not "did the car drive
this track".

## 4. The sector audit

Splits **simulated** on the segment maps, never read from ghost headers.

| tape | CP1 | CP2 | CP3 | CP4 | CP5 | finish |
|---|---|---|---|---|---|---|
| human rank 2 (our template) | 7.390 | 15.791 | 20.163 | 28.156 | 33.830 | 40.226 |
| `BEST_39961_v3` | 7.390 | 15.791 | 20.163 | 28.156 | 33.814 | **39.961** |
| `KEYBOARD_39996_v3` | 7.390 | 15.791 | 20.163 | 28.156 | 33.820 | 39.996 |
| `BEST_39973` / `KBD_40001` / `BEST_40040` / `KBD_40058` | 7.390 | 15.791 | 20.163 | 28.156 | 33.82x | … |
| human WR rank 1 | 7.311 | 15.718 | 19.980 | 27.834 | 33.584 | 40.223 |

Six tapes, three searches, two input alphabets — **identical at four consecutive
checkpoints.** All 262 ms of our advantage over the human world record is made
after 33.8 s, and at CP4 we are **322 ms behind** the record we beat.

`tmtas splits` cannot see this: it reads `DeclaredResult`, so all six of our
tapes report `race_time=40226` and rank 2's five splits. Fleet sidecar:
`w612_FLEET_SIDECAR_template_inherited_driving_v1.md`.

| sector | ours (=rank 2) | WR | field min | holder | spread | corr | path/chord |
|---|---|---|---|---|---|---|---|
| 0 →CP1 | 7.390 | 7.311 | **7.295** | r6 | 41.872 | +0.72 | 1.19 |
| 1 CP1→CP2 | **8.401** | 8.407 | 8.401 | **r2 = us** | 23.602 | +0.43 | 1.37 |
| 2 CP2→CP3 | 4.372 | 4.262 | 3.784 | r9 | 13.355 | +0.43 | 1.01 |
| 3 CP3→CP4 | 7.993 | **7.854** | 7.854 | r1 | 13.877 | +0.71 | **3.36** |
| 4 CP4→CP5 | 5.658 | 5.750 | 5.674 | r2 | 18.693 | +0.76 | 1.27 |
| 5 CP5→fin | **6.147** | 6.639 | 6.396 | r2 | 12.827 | +0.87 | 1.21 |

**Sector 2's 588 ms is not real.** Sectors 1 and 2 are bimodal and
anti-correlated; the joint board is what counts:

```
12.669 = 8.407 + 4.262  rank 1   <- the best pair anyone drives
12.773 = 8.401 + 4.372  rank 2 = us
12.882 = 9.098 + 3.784  rank 9   <- the "fast sector 2" variant, and it LOSES
```

So the best jointly-achievable human driving in sectors 0–3 is
`7.295 + 12.669 + 7.854 = 27.818` against our 28.156: **326 ms, and that is all
the human field has to give.** Against a 1.431 s gap.

## 5. The two cuts, and why neither pays

Sector 3 is a **hairpin**: CP3 → 310 m west → 180° turn at (371,13,714) →
380 m east → CP4, and its outbound leg is already a 220 m flight. It is the
obvious place for a second sector-4-style cut, and it is dead.

**A ballistic flight changes horizontal travel heading by exactly zero.**
Measured on that flight, travel heading = `atan2(Δz,Δx)` from position deltas:

```
t_ms    travel_deg   chassis_yaw_deg   ground
20500   -168.073     -106.742          false
21700   -168.052     -129.831          false
21900   -168.045     -131.983          false
22300   -154.513     -136.608          true    <- ground contact
```

**Constant to 0.08° over 1.4 s while the chassis rotates 30.4°**, then 14° of
heading change in two ticks the instant it lands. The `14bbffec` arm measured
the same law independently on the sector-4 jump and closed the exit problem with
it; my sidecar v1 had made the mistake this corrects — it quoted chassis yaw as
"the yaw the flight delivers", when that term is identically zero.

Scored properly: you keep `cos(mismatch)` of your speed, where mismatch is
between the flight's bearing and the landing surface's direction.

| | sector 4 | sector 3 |
|---|---|---|
| path/chord | 1.27 | **3.36** |
| mismatch at touchdown | 52.8° | ~100° |
| `cos(mismatch)` | 0.60 | **−0.17** |
| verdict | 1.128 s to CP5, real, **unbankable** | you land travelling backwards along the road |

**The sector with 2.6× the detour ratio is the more obviously dead one**, which
is the sharpest available statement of the path/chord warning: doubling back is
what makes a cut's two ends point in opposite directions, so the ratio's signal
and the criterion's veto are positively correlated. Sidecars:
`w612_FLEET_SIDECAR_yaw_budget_for_cuts_v1.md` (wrong, kept) and `_v2.md`
(correct).

## 6. What TAS is worth in the sectors nobody had searched

Sectors 0–3 had **never been searched** before tonight. Every arm ran on a
segment map built from the untouched map, own staging root, phantom guard on;
every result below was cold-validated afterwards with human controls in-batch.

| sector | seed | workers | evals | **result** | vs best human | cold-validated |
|---|---|---|---|---|---|---|
| 0 | our tape (rank-2 line) 7.390 | 34 | 134,520 | **7.251** | −44 ms | 7251, r1 7311 / r6 7295 exact |
| 0 (2nd basin) | human rank 6, 7.295 — a different line | 10 | 30,660 | **7.253** | −42 ms | 7253, same batch |
| 1+2 | our tape, 12.773 | 10 | 3,900 | 12.758 | +89 ms | 20148, r1 19980 exact |
| 3 | human WR, 7.854 | 34 | 95,040 | **7.591** | **−263 ms** | 27571, r1 27834 / r2 28156 exact |

**Sector 0 is converged**: two seeds from *different human lines*, 165,000
evaluations, **1 ms apart**. That is worth more than the 44 ms it bought — it is
the standard for deciding when to stop, and it freed 34 workers the minute I
believed it. Sidecar:
`w612_FLEET_SIDECAR_convergence_needs_a_second_basin_v1.md`.

**Sector 3 is not converged** and is the map's live thread: 263 ms under the best
CP4 any human has set, still improving, with cold (T=6) and hot (T=45) arms
re-seeded from it.

## 7. Where the map stands

| bound | value | vs AT 38.530 |
|---|---|---|
| field marginal splice (`RESULT.md` §3) | 39.404 | +0.874 |
| joint-corrected for the s1/s2 pair | 39.888 | +1.358 |
| + our sector 5 (6.147) | 39.639 | +1.109 |
| + the jump's sector 4 (4.558) — **withdrawn**, the exit is unsolvable | 38.523 | −0.007 |
| **+ tonight's TAS sectors 0 and 3** | **39.451** | **+0.921** |

For two hours the 38.523 line was the most interesting number on this map: with
the jump, recombining known driving lands 7 ms *under* the author time. The
`14bbffec` arm then measured the landing and withdrew it — from the jump's CP5
state the car has ~42 m/s of usable speed where rank 2 arrives aligned at 75.3,
so the 6.147 sector 5 is not available from there.

So the honest position: **the map needs ~0.92 s of TAS headroom above every
human, in sectors that had never been searched until tonight and have so far
given 307 ms of it from two sectors, one of which is converged and one of which
is not.** Whether that closes is a compute question and an assembly question,
not a technique question — and assembly is the harder half, because this tail
absorbs nothing (a tape 29 ms faster at CP1 returns `DNF cps=1`) and all 60 of
`RESULT.md` §3's cross-splices DNF. Every number in §6 is a **segment** result.

## 8. Negatives worth the same as the positives

* **0 finishers in 53,640 evaluations** from the jump's CP5 state on the
  untouched map (38 workers, 9 min, `finish 0% shaped 0%` throughout) —
  independent replication of `14bbffec`'s 0/207,000. The DNF basin past CP5 is
  not one mutation deep.
* **The throughput fix does not apply here.** Same server, same map, one worker,
  8 candidates each: **8 finishers 7.57 s, 8 DNFs 7.17 s.** DNFs are marginally
  *cheaper*. `FLEET_NOTICE`'s defect 2 is conditional on a template cut from a
  long recording; our seeds are whole 40-second ghosts declaring 40.226 s. I did
  not patch the declared time, and the reason is a measurement, not a guess.
* **Sibling tapes do not transfer, and the divergence bound over-promises.** The
  answer-key agent reported `151734`'s human line first divergent from this map
  at 8.650 s on 98.1 % identical geometry. All five sibling ghosts **DNF on my
  `seg1`**, which only requires surviving 7.39 s, with `rank1` 7311 and `rank6`
  7295 exact in the same batch. An occupied-cell diff is a statement about
  geometry, not about drivability.
* **Zero phantoms in six arms.** My fork carries both halves of the §8c fix:
  `create_dir_all(&a.bestdir)` at startup, and the swallowed
  `let _ = fs::write(...)` replaced by an abort with **exit 8**, so a failed
  write can never be reported as a phantom.

## 9. Artefacts — `~/persistent/private-30d/tm-unbeaten/146612/`

| file | what |
|---|---|
| `w612_SECTOR_AUDIT_v1.md` | §4 in full, with the raw segment-map splits |
| `w612_segsplits_v1.txt` | raw `tmmaps oracle` output, 6 maps × 9 ghosts |
| `w612_ladder_raw_v1.txt`, `w612_ladder_narrow_v2.txt` | the three ladders, with their controls |
| `w612_tools_v1.tgz` | `tmmaps` (`mladder`/`bladder`/`moveblock`/`movemany`, `move_block_cell`, `set_block_dir`, `coord_off`), the patched `tmsearch/src/main.rs`, and `w612` (`cells`/`path`/`cross`/`heading`/`splits`/`dev`/`selfnear`/`audit`) |
| `w612_final/` | the four validated sector tapes + the map + the control ghosts |
| `w612_PLAN_v1.md` | the plan as written before any measurement |

Fleet-level, in `~/persistent/private-30d/tm-unbeaten/`:
`ACQUISITION_addendum_146612_gate_plane_orientation_v1.md`,
`w612_FLEET_SIDECAR_template_inherited_driving_v1.md`,
`w612_FLEET_SIDECAR_yaw_budget_for_cuts_v1.md` (superseded, kept) and `_v2.md`,
`w612_FLEET_SIDECAR_convergence_needs_a_second_basin_v1.md`.

Nothing was submitted to any Nadeo leaderboard.
