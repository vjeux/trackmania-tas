# 249521 `impossible at for ssano` — RESULT

**The author time has fallen: by 299 ms on a keyboard, 359 ms with analog
steering.**

| | ms | vs AT | vs human WR |
|---|---|---|---|
| best human online WR (`in-.-`, rank 1 of 147) | 15039 | +391 | — |
| **author time (AT)** — never beaten by a human | **14648** | — | −391 |
| **this TAS, keyboard alphabet {−127, 0, +127}** | **14349** | **−299** | **−690** |
| this TAS, analog steering | **14289** | **−359** | −750 |

Reproduce the claim:

```bash
tmtas validate --map ~/persistent/private-30d/tm-unbeaten/249521/map.Map.Gbx \
  ~/persistent/private-30d/tm-unbeaten/249521/tapes/TAS_kbd_14349.Ghost.Gbx \
  ~/persistent/private-30d/tm-unbeaten/249521/tapes/CONTROL_humanWR_15039.Ghost.Gbx
```

→ `14349` and `15039`. Three cold passes in `logs/coldval.txt`, sha256 in
`tapes/SHA256SUMS`. Nothing was submitted to any leaderboard.

**What the replay files are.** They are input tapes. The 1661 per-tick
steer/accel/brake values are ours; the `CPlugEntRecordData` telemetry inside is
still the human WR's, inherited from the template. They re-simulate to the times
above in the oracle, but loading one in game as a ghost would replay the human's
motion. To watch this run drive, re-simulate the inputs (`/validatepath`) or
import the TICK script (`tapes/*.tick.txt`).

---

## 1. What this map is

Four blocks in one row on the x axis, plus one item:

| x | block | |
|---|---|---|
| 800..832 | `RoadTechStart` | spawn at x = 816, facing +x |
| 768..800 | `RoadTechSpecialTurbo` | boost pad |
| 736..768 | `RoadTechSpecialTurbo2` | boost pad |
| 704..736 | `RoadTechFinish` | trigger at x ≈ 736 |
| (828, 10, 752) | `GateCheckpointCenter8mv2` | the only checkpoint |

The checkpoint is **12 m in front of the spawn**; the finish is **80 m behind
it**. You nose forward through the gate, then have to get back down 64 m of
boost pad — and the pads push the wrong way. Measured: from a −24 km/h reverse
with the wheel straight, the pads stop the car, reverse its motion and carry it
to +46 km/h in +x, pinning it against the start block at x ≈ 808.

So every one of the 147 human runs crosses the strip **backwards, wagging the
nose ±90-100°**. That is not decoration, it is the only known way to make
ground: 11.4 s of the WR's 15.0 s is that crawl, at a mean −5.0 m/s.

Ruled out by measurement, not assumption:

* **You cannot turn round and let the pads fire you at the line.** Full lock
  settles into a stable 32 km/h orbit with zero net progress; centring the wheel
  as the nose comes past −x flings the car +x at 210 km/h, off the strip
  entirely (`traj/spin_l.csv`, `traj/u_942_gb.csv`).
* **No author ghost is recoverable from the map.** The header says
  `validated="1"`, but the file carries neither a `CPlugEntRecordData` chunk nor
  a `0x0309201D` input chunk. The medals are hand-set round numbers
  (16000/18000/22000) while the AT is not round, and the author is also the WR
  holder — 391 ms slower online than their own validation lap.

**Oracle health (the whole-field check).** All 147 downloaded ghosts
re-simulated against the untouched map: **146 return their exact recorded
millisecond, 1 DNFs** (rank 76, 17986). Every one of the top 75 is exact,
including the WR (`val_all.txt`).

## 2. Where the 391 ms lives

Crossing times at world-x stations, from 10 ms trajectories:

| sector | field mean (top 40) | human WR | TAS kbd |
|---|---|---|---|
| start → x=816 — out to the CP, stop, reverse | 3024 | 3003 | 3004 |
| 816 → 800 — reverse dash at 100 km/h, no pads yet | 693 | 683 | 684 |
| 800 → 784 — pads bite, 87 → 24 km/h | 2110 | 1627 | 1610 |
| 784 → 736 — the wag, four swings | ~8000 | 8143 | 7590 |
| 736 → finish | 525 | 852 | ~700 |

**The gap is diffuse.** The launch is a dead heat (3 ms), the entry is worth
17 ms, and essentially all of it is spread across the four swings. This is
technique-within-a-route, not a feature nobody found — which is what a 391 ms
gap over a 147-run field looks like when everyone already drives the same line.

Per-sector correlation with the final time across the top 40 is weak everywhere
(0.07-0.50), and so are the obvious wag statistics: swing count 0.05, swing
duration 0.30, ground per swing 0.10, peak speed **0.02**, amplitude 0.17. The
field is not sorted by any one of them.

## 3. The technique — verdict: KNOWN BUT MIS-TIMED

Every top human already does the right *shape*: both pedals down through the
whole crawl, and one gas lift per swing. The WR lifts at 7.09, 8.91, 10.82 and
12.76 s; our tape lifts at 7.06, 8.89, 10.82 and 12.76 s — **the same moments in
race time**. The difference is entirely in *what the car is doing when the lift
happens*:

| lift | TAS heading, off → on | WR heading, off → on | TAS speed | WR speed |
|---|---|---|---|---|
| 1 (7.06 s) | 71° → 77° | 69° → 75° | 26.9 → 25.3 | 26.2 → 22.4 |
| 2 (8.9 s) | −89° → **−105°** | −85° → −97° | 30.0 → **55.3** | 29.1 → 39.2 |
| 3 (10.8 s) | +93° → **+101°** | +83° → +95° | 32.1 → **67.0** | 29.3 → 34.9 |
| 4 (12.8 s) | −87° → **−101°** | −80° → −89° | 30.6 → **65.6** | 28.5 → **28.6** |

(heading 0° = nose pointing back up the strip at +x, ±90° = nose square across
the track.)

**The human field lifts before the car is square.** Lift at 80-85° and the pads
give you 0-6 km/h — the WR's fourth lift is worth literally nothing
(28.5 → 28.6). Lift once the nose is past square, 90-105°, and the same pads
give you 25-35 km/h. That speed is then spent on the way back through square,
where our vx peaks at −9.7 m/s against the WR's −8.2, and each swing eats a
little more ground in a little less time:

| swing | human WR | TAS keyboard |
|---|---|---|
| 1 | 1830 ms, −10.02 m, peak 36 km/h | 1850 ms, −10.11 m, peak 48 km/h |
| 2 | 1920 ms, −9.72 m, peak 42 km/h | 1920 ms, −9.74 m, peak 55 km/h |
| 3 | 1960 ms, −9.56 m, peak 35 km/h | 1960 ms, −10.39 m, peak 70 km/h |
| 4 | 2050 ms, −10.26 m, peak 31 km/h | 1830 ms, −10.68 m, peak 68 km/h |

Note the population caveat from §2: peak speed on its own does not sort the
human field (corr 0.02). The teachable statement is not "swing harder" but
**"wait until the nose is square before you lift, and be swinging back as the
speed arrives"** — the humans who reach high peaks reach them at the wrong point
in the swing and throw the speed sideways instead of backwards.

## 4. Is it humanly executable?

The AT was driven by a person, so yes; the question is how tight. Every
steering change event in a tape was moved by ±1..±4 ticks and every variant
re-simulated (`u10cand tolerance`, tables in `logs/tol_*.txt`):

| tape | ms | steer events | perturbations that still finish | events with any slack |
|---|---|---|---|---|
| human WR — a run a person actually drove | 15039 | 23 | 18 % | 9 / 23 |
| **TAS keyboard** | **14349** | 54 | **41 %** | **33 / 54** |
| robustness-polished keyboard | 14479 | 46 | 33 % | 26 / 46 |
| 30-event thinned keyboard | 14608 | 30 | 10 % | 7 / 30 |

**Our tape is more than twice as forgiving as the human world record's own
tape.** Both are brittle as open-loop objects — most single-tick edits DNF —
which is the standing lesson from 270051: open-loop jitter is not evidence about
human executability, because a driver is closed-loop and steers off what the car
is doing. What matters is that this tape does not ask for anything finer than
what the record holder was already holding.

Two negative results worth recording:

* **The robustness polish bought nothing here.** Re-placing each event where the
  worst case over ±1 tick is best (`ssa robust`) traded 130 ms for 2 percentage
  points of survival. The fragility on this map is systemic — an 11 s chaotic
  slide — not a few one-tick stabs, so there is no unteachable lottery ticket to
  convert.
* **Thinning the tape makes it *less* forgiving, not more.** 54 events → 30
  events costs 259 ms *and* drops survival from 41 % to 10 %. The extra events
  are not clutter; they are the corrections that keep the swing on its cycle.
  The deliverable is therefore the 54-event tape, not the minimal one.

Also, straight out of the WR's own tolerance table: shifting **its** fourth-swing
gas lift 3 ticks later is worth **160 ms** to that run. The field's lift timing
is early, and its own data says so.

## 5. The low-input family

| tape | ms | vs AT | steer events | pedal events | alphabet |
|---|---|---|---|---|---|
| `TAS_analog_14289` | 14289 | −359 | 597 | 17 | 224 values |
| `TAS_analog_14329` | 14329 | −319 | 582 | 17 | 191 values |
| **`TAS_kbd_14349`** | **14349** | **−299** | **66** | **25** | **3 (keyboard)** |
| `ROBUST_kbd_14479` | 14479 | −169 | 65 | 25 | 3 |
| `DRIVABLE_kbd30_14608` | 14608 | −40 | 31 | 25 | 3 |
| human WR, for scale | 15039 | +391 | 27 | 13 | 3 (keyboard) |

**Analog steering is worth 20 ms and costs 516 extra steering events.** The
keyboard tape is the deliverable in every sense: the WR is a keyboard run, five
of the top eight humans are keyboard runs, and the keyboard optimum is within
20 ms of the analog one.

(Projecting the finished analog tape onto the keyboard alphabet DNFs at every
minimum hold — third map in a row. Constrained tapes have to be *searched* under
the constraint.)

## 6. Driving guide, off visual cues

Times are race time. "Square" = nose pointing straight across the track.

1. **Countdown.** Hold gas. (Revving earlier or later is worth nothing here —
   measured.)
2. **0 → 1.05 s.** Gas, straight, roll 7 m forward and take the checkpoint.
   Don't try to be clever: the whole field is within 20 ms here and so are we.
3. **1.07 s.** Off gas. **1.09 s: brake, and keep the brake pressed until the
   finish line.** The car stops at ~1.8 s and starts rolling backwards.
4. **1.1 → 3.66 s.** Reverse in a straight line, past the spawn, up to about
   88 km/h. Wheel dead straight — every 10 ms of wobble here is free time lost.
5. **3.66 s, the moment the rear reaches the first boost pad** (x = 800, the
   join between the start block and the first pad): **add gas — both pedals
   down — and start flicking.** For the next 2.1 s alternate full-lock left and
   full-lock right roughly every 100-190 ms, keeping the car pointed straight
   back up the strip. The pads will drag you from 88 km/h down to about 21; the
   flicking is what stops them turning you round. You cover 20 m in this phase.
6. **5.76 s.** Stop flicking. **Hold one lock** (ours holds left) and let the
   car pivot. This is the first swing.
7. **Each swing, four times, alternating direction:**
   * let the nose come round to **square, and a touch beyond — 90 to 105°**,
     i.e. pointing at the side of the strip or slightly back down it;
   * **only then release the gas** (brake still down) for about 250 ms — the
     pads will punch you from ~30 km/h to 55-70;
   * **gas back on** and flip the lock the other way, so the nose swings back
     through square while that speed is pointing down the strip. This is where
     the ground is made: vx peaks near −9.7 m/s as the nose passes straight.
   * Each swing is worth about 10 m and takes ~1.9 s.
8. **~14.35 s.** The finish trigger is the near edge of the last block; you
   cross it mid-swing, at ~32 km/h, nose about 100° across. There is nothing to
   set up for it.

**Which parts are realistic.** Steps 1-4 are free. Step 5 (the flick phase) is a
rhythm, not a set of exact timings — it is the most forgiving part of the tape.
Step 7 is the map: four judgement calls, one per swing, each of them "wait for
square, then lift". That is the whole 391 ms, and it is a cue a driver can see.

## 7. Traps found on this map

* **`--quant` was a silent no-op in the classic search path.** It is implemented
  only in `forksearch.rs`; the classic path accepts the flag, ignores it, and
  reports an unconstrained result as if it were keyboard-legal. Three "keyboard"
  runs were analog before this was caught (176 distinct steer values in the
  output). Fixed by projecting every candidate before evaluation in the classic
  path too — `project()` in `tmsearch/src/main.rs`, plus a `--minhold` that works
  there. **Anyone who has reported a `--quant` result from the classic path
  should re-check it.**
* **The Factory's `accel` field is the real BRAKE, and its `brake` field is the
  real GAS**, on every ghost from this map. Confirmed by experiment, not
  inference: from mid-run, holding only the `brake` field turns a −8.9 m/s
  reverse into forward motion, while holding only the `accel` field accelerates
  the reverse to −27.8 m/s. `tmsite tick` inherits the mislabel, so the exported
  scripts here come in both forms — `*.tick.raw.txt` as the tool writes it and
  `*.tick.txt` with the two keywords exchanged, which is the one to drive.
* **Tape indices are offset by `start_offset_ms` (≈ −1570).** Race ms =
  10·tick + start_offset. The first ~157 ticks are countdown.
* **Every human tape ends at that human's own finish.** A template gives no
  slack: any candidate slower than its template DNFs for want of tape, and that
  DNF is indistinguishable from a crash. Choose an exploration template
  accordingly.
* **A faithful, correctly aligned copy of one human's tape onto another human's
  template DNFs** — every shift in ±8 ticks, both directions, adequate tape
  length, zero frozen slots, and the written file's decoded tape matches the
  source exactly. Something outside the input chunk is per-ghost. Copies within
  one template's lineage are exact. Unresolved; worked around by never mixing
  templates.
* **`fk btraj` needs `--allow-dnf` to measure a non-finishing tape**, and the
  flag is rejected by `state::parse` in the hardened build (patched locally).

## 8. If someone continues

* Both final arms (86 workers, 32 min each, then 70×45 min) sat on 14329/14349
  without moving, from four independent seeds. The plateau looks real for local
  search under a whole-millisecond objective.
* The obvious next lever is a **dense objective for the crawl**: relocate the
  finish gate to x = 792 / 776 / 760 / 744 (`tmmaps probe --at`) and optimise
  the swings stage by stage, since a mutation in swing 2 currently has to
  survive 6 more seconds of chaos before it scores. That is the biggest
  structural improvement available and it was not built here.
* The sub-tick timing plane is **not** applicable: the finish is crossed
  mid-swing with the heading varying by tens of degrees across tapes, which is
  exactly the 227969 failure mode.
* The entry flutter (3.66 → 5.76 s) has never been searched as a *shape* — only
  perturbed. A car that starts swinging earlier, while still carrying 60-80 km/h
  of reverse speed, is the one untested strategic idea on this map.

## 9. Three more controls, run because they change what to teach

**The entry flick is load-bearing, not decoration.** Replace the flick phase
(3.66 → 5.8 s) with a straight wheel and keep everything else: the car enters the
pads at 88 km/h in reverse, is stopped **3.4 m in** (x = 800 → 796.6 by 4.0 s),
and is thrown back out to x ≈ 806. It never reaches the second pad. With the
flick the same 2.1 s covers 20 m. So the first thing to teach a driver is not
the swings, it is *keep flicking on the way in*.

**Gate maps as a dense objective — built, wired in, no gain.** `tmmaps probe
--at X,10,752 --cell C,9,23 --block 3` relocates the checkpoint gate, turns it
into a finish, and leaves a working segment map (the human WR crosses the x=792,
776, 760 and 744 gates at 4059 / 6328 / 9999 / 12938 ms — the gate is a body
trigger, so it fires 200-500 ms before the car's centre reaches the plane, which
is the expected offset). Fed to the search as `--seg 1:gate_760.Map.Gbx`, every
DNF gets a real score instead of a flat one (14-18 % of candidates shaped). It
did not move either incumbent in 35 minutes, and it halves the evaluation rate
because a DNF now costs a second simulation. Worth knowing before someone else
spends the afternoon on it: the plateau here is not caused by DNFs being
unscored.

**Other human seeds do not catch up.** Four keyboard humans (ranks 3, 5, 7, 8)
seeded into identical 42-worker keyboard searches reached 14887 / 14859 / 15631 / 15528 — none of them even reached the author
time — while the WR-seeded line was at 14349. On this map
the basins do **not** merge — the WR's line is the one to work from, and that is
worth ten minutes to establish rather than assume.

## 10. Late addendum — 14289 (analog), and what finally moved

After six flat rounds (four seeds, two with gate-map shaping), a **high
temperature** run finally moved the analog line: `--temp 45 --migrate 0.10`, 86
workers, 40 minutes → **14289 ms** (AT − 359, human WR − 750), validated three
times cold with the human WR as control in every batch. The keyboard line did
not move under the same treatment (`--temp 60`, 919 k evaluations, still 14349).

The lesson is the annealing temperature, not the objective: at T = 15-22 the
search sat on 14329/14349 through roughly 3 million evaluations, and at T = 45
it walked out within 15 minutes. On a map whose whole run is one long chaotic
slide, the useful moves are large and initially costly, so a temperature that
"routinely tolerates 45 ms worse" is the one that finds them.
