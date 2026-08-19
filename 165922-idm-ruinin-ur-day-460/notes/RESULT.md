# 165922 `idm ruinin ur day #460` — **AUTHOR TIME BEATEN**

**AT 15.643 → 15.240 validated on the untouched map.** −0.403 s (−2.6 %).
A single clean attempt from tick 0, **no respawn anywhere in the tape**.

Session 3, 2026-08-18, node 82976.od.fbinfra.net.
Builds on `RESULT.md` (agent 1, characterisation) and
`RESULT_flight_physics_and_seed_problem.md` (agent 2, measured physics + the
seed blocker). Neither is retracted; both were right about what they measured.

| | time | note |
|---|---|---|
| author time | **15.643** | the target |
| human field WR | 8790.769 | one session of 915 attempts, 941 respawn presses |
| the field's best clean attempt | — | never finished; stopped 5 m short of the first gate row |
| **this session** | **15.240** | `v3/AT_BEATER_15240.Ghost.Gbx`, clean start, 0 respawn packets |

Nothing submitted to any Nadeo leaderboard.

**Independently verified.** Agent vj4 re-measured the first AT-beating tape
(`AT_BEATER_15549`) on a **different node** (64455) with their own build fork and
staging root, map md5 `1cc927bbb1d640c665ff69068352d4e6`, and confirmed
**15549** with a known-answer control at 8790769 and the respawn audit at zero —
`vj4_VERIFICATION_v1.md`. They have since carried the line to **15230**
(`vj4_clean_15230.Ghost.Gbx`, 0 respawn packets), which I re-validated here at
**15230** in a batch with the human control. The current best on this map is
therefore **15.230**, and it is theirs; **15.240** is this session's own.

---

## 1. What unlocked it

Two prior sessions concluded the deliverable had to be one clean no-respawn
attempt from tick 0, and that the human's winning attempt could not be
transplanted there. Both are true. The step that was missing is that
**a respawn is a legal INPUT whose state is canonical**, which turns the
impossible transplant into a two-line construction:

```
[ any prefix reaching race t = 1.670 s ] ++ [ respawn packet ] ++ [ the winning attempt ]
```

finishing at exactly `(K + L)·10 − 1540 ms`. That produced the field's **first
finishing clean-start tape on this map**, at 20.519 — and, with agent 1's
optimised attempt in the tail, 16.785 straight away.

That route has a floor: the respawn cannot be armed before race t = 1.670 s
(§3), and it bottoms out near 16.1. It was never the deliverable — it was the
**instrument**. Having a finishing tape is what made a dense score, a calibrated
gate ladder and a real search possible, and the ladder is what finally carried a
genuinely respawn-free tape home.

## 2. The mechanism, and the controls on it

Respawn rides in **bit 31 of the packet's 34-bit state literal** (ACQUISITION,
from 197047), literal `0x80000002`. This record carries **941** of them against
914 telemetry discontinuities — enumerate the packets, not the jumps.

**Canonical-state test.** `[record 0..K) ++ [record 877346..879231)` over a
4700-tick sweep of K returns exactly `(K + 1885)·10 − 1540` every time:

```
K=321 -> 20519   K=324 -> 20549   K=396 -> 21269   K=400 -> 21309
K=1400 -> 31309  K=5000 -> 67309
```

Perfectly linear, including from mid-flight at the 999 km/h cap. The 1885 ticks
after a respawn replay identically regardless of what the car was doing before
it. `"NbRespawns": 1` in the validator's own `ValidatedResult` confirms the count.

## 3. K = 321 is a hard floor — the respawn arms at race t = 1.670 s

```
K = 316..320   DNF cps=1
K = 321..400   finish, linear in K
```

and it is not a property of the human's particular driving. `m165 probe`
re-validates 3 000 random mutations of the prefix window `[154, K)`:

| K | identity | finishers / 3000 |
|---|---|---|
| **321** | **16785** | **140 — and every one of them returns exactly 16785** |
| 310 / 290 / 250 / 200 | DNF | 0 / 0 / 0 / 0 |

The instrument says **yes** at 321 and **no** below it. The 140 finishers all
returning the same millisecond is a second, independent proof that the prefix
contributes nothing but its length.

## 4. THREE ORACLE DEFECTS WORTH ~1000× — fleet-reusable, all fixed

Candidates cost **2.7 s (finisher) / 32 s (DNF)** at the start of this session
and **0.03 s / 0.34 s** at the end; throughput went from **14.5 to ~500
candidates/s** on 150 workers. Without this none of the rest happens.

### 4.1 `tmcut --strip` strips nothing, and never could

The telemetry (`CPlugEntRecordData`, `0x0911F000`) is **not** a top-level PIKS
skippable chunk. It is inline inside CGameCtnGhost chunk `0x03092000` as
`id | version | uncompSize | compSize | zlib`. `gbx::all_skip_chunks` admits only
class-id top bytes `{0x03,0x0B,0x24,0x2E,0x30}` — `0x09` is not among them — so
`tmcut --strip`'s `find(|c| c.0 == ENTREC)` matches nothing and **the flag is a
silent no-op**. The blob inflates to **24 309 292 bytes per candidate**: that is
the 2.7 s.

`m165 telmin` re-encodes the record with the same header, descriptors and
notices but **zero samples** (grammar from `tmtraj::entrec`) and shrinks the
enclosing chunk header. **1 914 181 → 5 425 bytes**, still 16785 to the ms.

**Do not just empty the blob.** `m165 tel0` (uncompSize 0, an 8-byte empty zlib
stream) yields a file the server refuses to load, *silently*: the ghost vanishes
from the batch (`Starting validation of 1 ghosts` instead of 2), no diagnostic,
and the caller reads `sim_time = None` — indistinguishable from a DNF.

### 4.2 A DNF is simulated all the way to the DECLARED race time

A tape cut from the 8 790 769 ms record still **declares** 8 790 769 ms, and a
run that never crosses the line is simulated to that clock — **independent of
the tape's own length** (a 300-packet DNF cost 22.5 s; a 1985-packet DNF
17.7 s). The declared time lives in **four** places, and
`RACE_TIME_CHUNK_ID = 0x03092005` is only one of them:

```
0x03092005        drives the walltime -- changing ONLY this leaves the DNF cost intact
0x0309200B +12
0x0309201B +10
0x0309202B +4 and +32    (the splits chunk)
```

Rewriting all four (`m165 setdecl_all`) takes a DNF from **17.7 s to 0.34 s**;
the finisher still returns 16785. Declaring just above the incumbent also prunes
for free: anything slower reports DNF.

### 4.3 `sweep::evaluate` uses `min(workers, ceil(n/batch))`

`tmex`'s default `--batch 600` runs a 1500-candidate round on **three** workers
however many `--jobs` were asked for. Use ~20–25.

`m165 mktpl IN OUT A:B,C:D DECL_MS` does join → telmin → setdecl_all in one step.

## 5. The gate ladder, its control, and the trap it sprang exactly as documented

**Return-to-origin control first** (as the coordinator required): `tmex movegrid`
rewriting the 132 gates back onto their own lattice
(`34:132:2300:8:11:-1:0:1:576:32:12`) reproduces the human record at **8790769**
and the incumbent at **16461**. The gate model is not being swapped and the
surgery is faithful.

Stations are curtains of the 132 gates at one x, 11 y-levels × 12 z-levels
(dy = 16, dz = 32). Calibration against the human's own winning attempt, whose
trajectory is known from `wr.csv`, matched every station to ≤ 8 ms:

| station | x | predicted | human's tape |
|---|---|---|---|
| s1 | 505 | 5200 | 5198 |
| s2 | 713 | 6700 | 6697 |
| s3 | 1216 | 9700 | 9698 |
| s4 | 1822 | 13700 | 13703 |
| s5 | 2230 | 16700 | 16705 |

**And §0.6 fired, on schedule.** Scoring on s3 (x = 1216) drove the crossing from
9640 to **7860** — 1838 ms ahead of the human — and those tapes reached neither
s4 nor the finish: optimising "time to a mid-course rung" buys a dive. Moving
the objective to the FAR rung (x = 1822, tall window) fixed it in one round, and
the first harvest from that arm finished the real map at **15549**. Every
headline here was re-validated on the untouched map, in a batch containing the
human record as a known-answer control.

## 6. Where the 1.2 s came from — measured, not inferred

Station profile of the winning tape against the same tail on the respawn route:

| station | x | **clean tape** | respawn route | Δ |
|---|---|---|---|---|
| p1 | 423 | 1958 | 2968 | −1010 |
| q4 | 486 | 3763 | 4691 | −928 |
| s1 | 505 | 4118 | 5206 | −1088 |
| launch | 713 | **5550** | 6656 | **−1106** |
| finish | 2300 | **15246** | 16461 | −1215 |

**−1106 ms of it is the start**, and −109 ms is a slightly better glide. The
clean run reaches the state the respawn manufactures in **0.56 s**; the respawn
costs **1.670 s**. That difference *is* the author time.

Two physical facts that bound the map, both measured here and consistent with
agent 2's fit:

* the first 3.5 s after the start state is **free fall** — 168 m from ~7 m/s
  solves to 3.51 s against gravity 23.29 m/s², which is exactly what the human
  achieves. It is not compressible.
* the glide is unpowered and speed-capped at 277.55 m/s; 1 880 m across and
  1 889 m down is a 2 665 m path, i.e. **≥ 9.6 s** at the cap. The tape's glide
  is 9.70 s.

So the respawn route floors at roughly 1.67 + 4.99 + 9.6 ≈ **16.1 s** and could
never have beaten the author. **The author's route is a clean start; there was
never another way.**

## 7. FOLLOW-UP 1 — how does a human do this

The author time is a driven validation lap and this map is the clearest case yet
that *"not humanly executable" is not the question*.

**Classification: known-but-unheld.** The technique is not undiscovered — the
one human on this leaderboard performed it **915 times in a single session**.
What they never did was *finish* one: their best attempt landed 45 m short of
the pad and slid to a stop at **x = 2294.8, five metres short of the first gate
row at x = 2300**, and every attempt after that was a respawn with the clock
still running. The 2.44-hour "record" is not a slow lap, it is a retry grind
that was never converted.

**What the run actually asks of a driver:**

* **13.5 of the 15.2 seconds need no input at all.** Forcing steer = 0 from tick
  954 to the end of the tape changes the time by **1 ms** (15292 → 15293).
  The entire 1.9 km glide is ballistic and uncontrolled — the driver lets go.
* The load-bearing part is **ticks 154–954, eight seconds**: creep off the start
  block, fall, and take the two booster gates. Zeroing any 100-tick window
  before tick 754 is a DNF; every window after 954 is free.
* Of that, the first ~3.5 s is gravity, not skill.

**Precision, honestly stated — and it is a fact about OUR tape.** `tmex tol`
shifts every input-change boundary in the ramp by ±1…8 ticks: **343 of 343
shifts DNF**, except three at ticks 491–502 which survive at +114 to +133 ms.
Our tape is a 10 ms-precision TAS line. That is the cue §0.7 names, and the
forgiving variant is the obvious next piece of work: re-search the ramp with the
per-boundary tolerance as an explicit objective rather than the finish time. The
existence of a forgiving line is not in doubt — a human drove down this chute
915 times.

## 8. FOLLOW-UP 2 — the low-input family

**Converting a finished tape does not work here either**, which makes five maps.
Quantising the whole tape to `{−127, 0, +127}` (or to five levels) DNFs, and
`tmex` refuses to start on it: `IDENTITY CONTROL FAILED`.

So the constraint was applied **under search**, and to the part of the tape that
tolerates it:

* Plain `tmex --alpha` quantises the **whole steer array**, which destroys the
  prefix the search was told not to touch — the 274191 finding. A patched
  `tmexq` quantises **only `[--lo, --hi)`**.
* **Zero-ladder control before use** (§0.4): with `--alpha 0` on `[154,954)` the
  seed DNFs (the constraint bites); with no alpha the same run returns 15293.
  The instrument is pinned from both sides.
* The glide is already input-free, so the march constrains the ramp backwards
  from tick 954: `[904,954) → [854,954) → …`, searching under the constraint at
  every step.

Result so far: pure keyboard steering on **`[604, 954)`** plus **steer = 0 over
the whole glide `[954, 2109)`**, validated at **15290** — inside the author time
by 353 ms. Measured against the analog tape:

| | analog 15240 | low-input 15290 |
|---|---|---|
| input CHANGE events | **611** | **86** |
| distinct steer values | **226** | **39** |
| ticks with no steering at all | 1155 | 1155 (11.55 s) |

**The frontier is tick 604 (race t ≈ 4.5 s), and it is sharp.** Extending the
constrained window even 10 ticks earlier DNFs the seed — not only at three
levels but at five and at **nine** (`{−127,−96,…,96,127}`), tested. Ticks
594–604 are in the booster phase and demand finer than 32/254 of lock in this
line. Going further needs the search to start from an infeasible point and climb
the gate ladder, which is the named next step rather than a limit: the
free-fall window `[394,494)` *does* tolerate keyboard (it finishes at 18769,
3.5 s slower), so the basin is reachable, just not from here for free.

## 9. What did not work, with the enumeration stated

* **The direct clean-start graft, at 1-tick resolution and 25× agent 2's sample
  size.** `[record 0..P) ++ [optimised attempt]`, P = 180…330: **151 of 151
  DNF**. Random mutation of the handover window `[154,300)`: **0 of 50 000**.
  The best state match is genuinely tight — clean t = 0.80 s against donor
  t = 0.10 s agrees to **0.09 m and 0.23 m/s** — but the *attitude* does not:
  pitch differs by 0.049 rad and roll by 0.064 rad, and on a car balancing on a
  chute edge that is everything. The graft never even reaches a curtain at
  x = 423. What worked instead was deleting packets from the *record itself*
  (`[0:P] ++ [877346+P−224+o : end]`), which keeps the clean opening the human
  actually drove; those tapes cross the launch plane 894 ms early and became the
  search seed.
* **A seed library from the other 914 attempts is not better.** Every attempt of
  ≥ 900 packets was rebuilt as a legal respawn tape (137 of them): **only the
  last one finishes**. Their fastest near-miss reached x = 2298.6 — 1.4 m short —
  in 17.15 s, slower than the tail already in hand.
* **`fk btraj` still fails on this map**, and for a *different* reason than the
  brief records. The 274191 clock-window ladder (adopted here, not rebuilt) runs
  all four rungs — 16 K/256 → 64 K → 192 K → **512 K/512 K** — and finds no
  `+10`-per-tick u32 in any of them at 400 samples each. The blind locator also
  settles on a slot whose mean speed is 1.2 m/s where the car should be doing
  tens, so the position lock is weak here too. **Trajectories were not needed:**
  the calibrated gate ladder replaced them completely, and it is cheaper.

## 10. Adopted rather than rebuilt

`tm-unbeaten/274191/fk-274191-clockfix-v1.tgz` already contained the widening
clock-window ladder and the speed-scaled `vel_err` tolerance this session was
briefed to write. Both were adopted verbatim. They are correct; they are simply
not the reason `btraj` fails here.

**Fleet build v4 does not compile against any published `forksearch.rs`** —
`main.rs` sets `FCfg { verify_best, phantom_continue, plane_x, quant }` and no
archive on the box has those fields. Rather than stub them silently (the
`--quant`-parsed-but-ignored trap of §0.5), the fork path in this tree
**refuses to start** when any of the four is set.

## 11. Files — `165922/v3/`

| file | what |
|---|---|
| `AT_BEATER_15240.Ghost.Gbx` | **the result** — 15.240 on the unmodified map, clean start, 0 respawn packets |
| `AT_BEATER_15549.Ghost.Gbx` | the first tape to beat the AT, kept as the provenance record |
| `respawnroute_16461.Ghost.Gbx` | best on the respawn route (legal, finishing, but floored ~16.1) |
| `respawnroute_human_20519.Ghost.Gbx` | the human's own winning attempt as a legal clean-start tape |
| `lowinput_best.Ghost.Gbx` | the low-input march's tape (keyboard ramp tail + zero-steer glide) |
| `m165.rs` | the tool: `respawns mktpl telmin setdecl_all probe holds zerofrom attempttapes findu32 attempts match` |
| `tmexq.rs` | `tmex` with the alphabet applied to `[--lo,--hi)` only |
| `tmjoin_strip.rs` | `tmjoin` with a `--strip` that is no longer a no-op |
| `gmaps/` | the calibrated station curtains + `origin.Map.Gbx` (the return-to-origin control) |
| `VALIDATION.txt` | raw oracle transcripts, every headline with a known-answer control in the batch |
