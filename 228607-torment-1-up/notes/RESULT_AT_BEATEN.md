# 228607 "Fall 2024 - 08 Torment (1-UP)" — the author time is beaten, 20.258 → 19.943, and the map is an official campaign map with a 400 000-record field

Agent `tor`, 2026-08-19, node 31830. Write-once sidecar; a correction gets a new
version number in a new file. Times in **seconds**. **Nothing was submitted to
any Nadeo or official leaderboard**; every network call was a read against
trackmania.io at ~1 req/1.6 s with a descriptive User-Agent.

Companion: `tor_FLEET_NOTICE_release_the_lock_on_1UP_corrects_228811_TECHNIQUE_v1.md`
(md5 `0a70765346c2e8bca65c8bb2d8ba820b`), which corrects an instruction in
`228811/TECHNIQUE.md` that is actively wrong for this map.

Builds on, and supersedes nothing in: `228607/key_RESULT_v1_sibling_228811_is_the_same_map.md`
and `key_RESULT_v2_author_lap_read_the_reactor_flight.md` (answer-key agent),
whose author-lap read is the reference trajectory used throughout.

---

## 0. The claim

| | time | vs AT 20.258 | vs human WR 24.902 |
|---|---|---|---|
| **best validated** | **19.943** | **−0.315** | −4.959 |
| next four | 19.947 · 19.973 · 19.977 · 20.066 | −0.311 … −0.192 | |
| the low-input family member | 20.070 | −0.188 | |
| author time | 20.258 | — | |
| human world record (Falco_TM_, 23 records) | 24.902 | +4.644 | — |

**Twenty tapes and two controls, re-validated with the map, the controls and the
tapes all read from the shared store and nothing from scratch: 22/22 exact.**
Transcript: `tor_bank/tor_VALIDATION_storeonly_v1.txt`. Manifest with sha256 for
every map, control, tape and downloaded ghost: `tor_bank/tor_MANIFEST_v1.sha256`.
An independent auditor reproduced 13/13 on its own build before this was written.

---

## 1. Why the map was hard, and what actually unlocked it

The fleet arrived here holding a bound seed — 228811's `v2_best_20237` grafted —
that cleared every checkpoint and missed only the Goal (`DNF cps=6`). Five
search islands, ~250 000 evaluations, all stalled at exactly the same altitude.

**They stalled at world y = 122. The official map's Goal is at cell y = 23,
which is world y = 122.**

228607 is an Altered Nadeo copy of **Fall 2024 - 08** (uid
`2msqSkfJP683MmDGhfpkJM4q7Sk`, official AT 23.541, ~400 000 records, WR **20.034**
by **Emelius.** — the name 228607's own title credits: *"Torment (1-UP)(ft'
Emelius)"*). The alteration moves the four `GateFinish` blocks:

```
official   cell y=23   world y=122      <- where every search stalled
228811     cell y=19   world y= 90      (1-DOWN)
228607     cell y=27   world y=154      (1-UP)   <- mine
```

Our entire tape lineage descends from 228811's search, which was optimised to
end **low**. Every one of those tapes flies the official line's altitude and
then some. The searches were not stuck on a hard problem; they were sitting on
the altitude the whole lineage was built for.

**The fix was a seed, not more compute.** Rung-profiling the official field
found a human 24 m higher than anything we owned; reseeded from a grafted
official tape, the same searcher produced a finisher **in sixteen seconds** and
was inside the author time within two minutes.

---

## 2. The instrument: a ladder of real Goals

`--seg` cannot shape this map. It keys on `reached_cps`, and every candidate
returns the same one — they all clear all checkpoints and miss the Goal. So:

```bash
tmmaps movemany m228607.Map.Gbx --out rung<CY>.Map.Gbx \
  --move 10905:11,<CY>,24 --move 10906:11,<CY>,21 \
  --move 10907:12,<CY>,22 --move 10908:12,<CY>,23     # CY = 19..27
```

Nine rungs, 8 m apart, **rung 27 is the untouched map**, so the top of the
ladder is not a proxy — it is the finish. Added `--rung idx:map` to `tmsearch`:
a non-finisher is tried at the rung above the incumbent's, else at the
incumbent's own (speed at the same altitude, which on this map is the same
currency as height — the author is both higher *and* 150 km/h faster there).
Two extra oracle calls per round, not one per rung; ~70 eval/s with the ladder
against ~300 without.

### The ladder's controls, and they are the strongest part of this result

| control | result |
|---|---|
| `tmmaps origin` (every waypoint through the mover, byte-identical) | **0 failures** |
| `tmmaps roundtrip` | body 1 694 918 → 1 694 918 **identical** |
| **native yes-control**: 228811's `v2_best_20237`, grafted, on rung y=19 (= the 1-DOWN Goal) | **20.237 — the sibling's own time, to the millisecond** |
| **foreign-container yes-control**: all 15 grafted official humans on rung y=23 (= the official Goal) | **15/15 return their own official times exactly** — 20034, 20188, 20217, 20257, 20260, 20272, 20307, 20337, 20387, 20426, 20430, 20480, 20703, 20738, 20938 |
| field reproduction of those 15 on the official map itself | 15/15 exact |
| lossless-graft control (a native tape's inputs into a different native carrier) | 24.854 exact |
| the ladder saying **no**: the same 15 tapes on the untouched 1-UP map | 15/15 `DNF cps=6` |

Fifteen independent, untunable predictions from foreign containers settle two
things at once: the graft path is exact here, and **228607 is physically the
official map with the Goal moved** — not merely similar to it.

### Displaced Goals as a trajectory readout

A search-produced or grafted tape carries its **carrier's** telemetry, so
decoding one describes the wrong run. Sweeping the Goal over a grid of
(cell x, cell y) and reading which placements fire gives the trajectory from the
oracle alone:

| gate x | author's own lap | our best (pre-official) | 228811's `v2_best_20237` |
|---|---|---|---|
| 192 | 102.6 | 90 | — |
| 256 | ≈127 | 106 | — |
| 320 | ≈147 | 122 | 90 |
| 352 | **160.5** | 122 | 90 |

Read only the **threshold** from such a sweep (`FLEET_NOTICE_ladder_the_moving_axis_v2`);
nothing here converts a gate displacement into a car displacement.

---

## 3. §A — how a human does this. The map splits the technique in two.

### The launcher is known and held — by 400 000 people, and by none of your 23

Every one of the top 15 official records fires the launcher: a single 50 ms
sample of **692 → 997 km/h** at (71–80, 50.4, ~709). On 228607's own 23-record
board, **0 of 23 fire it**; their largest one-sample rise anywhere near the boost
deck is ~10 km/h. The altered board simply never found a move that is standard
practice on the same blocks.

### The coast is held by nobody

The author's flight after ignition is **ballistic**: vy 92 → 51 over 1.55 s is
**−26 m/s², gravity and nothing else**, and speed only falls 769 → 688. The
slope decays 0.386 → 0.332 → 0.302. He fires and coasts.

| | vy at ignition + 0.45 s | y entering the Goal x-band (x=352) |
|---|---|---|
| **the author (AT 20.258)** | **+79.5** | **160.5, climbing** |
| official WR Emelius. 20.034 | +62.1 | 135.0 |
| official #5 KappaRiley 20.260 | +68.6 | 143.6 |
| official #10 NiTech91 20.426 | +61.2 | 140.4 |
| worst of the official top 15 | +49.8 | 127.6 |
| 1-DOWN author (228811) | +30.7 | 95.3, flat |

**The author is 17–33 m above the entire visible top of a 400 000-record field.**

### The move, from the author's own inputs

Both Torment authors are in an identical state at the launcher contact — pitch
0.26 rad, roll 0.06, ~330 km/h, y = 50.4 — and both leave it at vy 92–94. The
launch vector is not what separates them. This is:

* **1-DOWN**: `steer = +1` **held**. Roll runs on to −3.10 (inverted), pitch
  rises to +1.11, the car goes broadside: 751 → 562 km/h, vy 94 → 22 in 0.5 s.
* **1-UP**: holds the lock ~200 ms, **releases to centre at 18.740**, then
  counter-steers to full left by 19.390. Roll stops at −1.61 and returns to
  −0.18; the nose falls in line with the 25°-up flight path; 769 → 720 km/h,
  vy 92 → 68.

Throttle and brake are held **together** throughout, on both laps.

> **Classification: the launcher is known-but-unheld on this board and known-and-held
> on the official one; the post-launch coast is UNDISCOVERED — nobody in either
> field does it.** It is a single, teachable action: *let go of the steering
> about 200 ms after the launch fires, then feed in opposite lock.* "Not humanly
> executable" is not in play: a person drove 20.258 doing exactly this.

---

## 4. §B — the low-input family, searched under the constraint

**Not converted.** Converting the finished analog tape was measured to fail here
exactly as on the other five maps: setting `--qlevels` over the whole lap takes
the 20.070 finisher to `DNF@cp0` immediately.

**The range constrained, named:** ticks **1840–2162**, i.e. race time
**18.40 s to the finish** — the reactor flight. That range is the one where a
projection is defensible, because it is the range measured to be **ballistic**
(vy decaying at −26 m/s², gravity only). Nothing was projected over the launch
itself, where the physics is a contact and the tolerance is one tick. I added
`--qlo/--qhi` to bound the alphabet ladder to a tick range for this.

| member | steer alphabet (flight) | input events (flight) | validated |
|---|---|---|---|
| `FAM_analog_fast` | 100 values | 125 | **19.947** |
| `FAM_analog_20070` | 58 values | 83 | 20.070 |
| **`FAM_lowinput_a8`** (searched under `--qlevels 8`) | **16 values** | **47** | **20.070** |
| `--qlevels 3` (7 values) | — | — | no finisher in 18 000 evaluations |

**Counting convention**, stated because the fleet has two: an event is a tick
whose value differs from the previous tick, counted per axis over the named
range, with no explicit initial value emitted.

**The constraint's own control passed**: the zero ladder (`--qlevels zero`, one
level, every steer forced to 0 over the same range) returned **finish 0 % and no
improvement over 18 920 evaluations**. The instrument can say no.

### Tolerance, per member — and "fewer inputs is easier" is FALSE here

Probe: slide the whole input block from tick 1890 (after the launch has fired,
so this isolates air control from the launcher) by ±1…±6 ticks.

| member | shifts that still finish, of 12 |
|---|---|
| `FAM_analog_fast` (19.947) | **1** — −2 ticks, finishing 20.248 |
| `FAM_analog_20070` | 0 |
| `FAM_lowinput_a8` (16 values, 47 events) | **0** |

**The low-input member is not more forgiving than the analog tape it came from —
it is less.** And the survivor is non-monotone (−2 works, −1 does not), so this
is a knife edge rather than a clean asymmetry; I am not claiming a direction from
one survivor. Shifting from tick 1840 instead (i.e. including the launcher)
kills all three at ±1, reproducing the sibling map's one-tick launcher result.

---

## 5. Two bounded negatives, each with the control that gives it meaning

1. **Transplanting the author's input script onto our launch does not work.**
   130 tapes: the full script, steer-only, release-only and throttle+brake-only
   variants, over 29 alignments of the ignition tick. Every one still `cps=6`
   and **not one fires any rung — including rung 19, which the seed itself
   fires**. So this is "worse than the seed", not "the instrument saw nothing".
2. **`tmcut setdecl` does not bite on this map.** A known 23.562 finisher
   re-declared *below* its own time still returns 23562, so the time-budget
   constraint is unavailable here. `--maxfinish MS` was added to `tmsearch`
   instead (a finish slower than the cap scores just above the top rung, so a
   slow basin cannot swallow the ladder).

And one defect of my own, worth carrying: **a decoded CSV's `steer` is a float in
±1; the tape stores i8 −127..=127.** Writing the float as the byte turns 0.976
into −4 — a mirror image that still simulates plausibly. It voided the first
sweep of 15 transplants. Print the template's own bytes before believing any
transplant.

---

## 6. Reproducing this from the store alone

```bash
B=~/persistent/private-30d/tm-unbeaten/228607/tor_bank
tmtas validate --map $B/maps/tor_m228607_untouched.Map.Gbx --jobs 18 \
      $B/tapes/*.Ghost.Gbx $B/controls/*.Ghost.Gbx
sha256sum -c   # against tor_MANIFEST_v1.sha256, run inside each subdirectory
```

`tor_bank/` holds the untouched map, the official Fall 2024 - 08 map, three
ladder rungs (y=19 the 1-DOWN Goal, y=23 the official Goal, y=26), both native
controls, the lossless-graft control, the grafted 228811 optimum, all 15
downloaded official records and 2 of their grafts, all 20 claim tapes, the
store-only validation transcript, and the two analysis tools (`torflight.rs`,
`torscript.rs`).

**Instrument note for anyone reusing the ladder**: on 228607 the **three-chunk**
graft (`--ids 0x0309201D,0x0309202D,0x0309202B`) is exact for foreign official
donors — 15/15. That is the opposite of 270051, where only inputs-only works.
The rule is the coordinator's: build both ways and keep whichever one's lossless
control passes in the same batch. Note also that on this map the lossless control
passed *regardless*, so it is not a detector for a wrong recipe — only the
foreign donor's own known answer is.

---

## 7. What I did not build

* **The searches were still improving when I wrote this** (19.943 and falling,
  three islands). A later arm should simply continue from `tor_bank/tapes/`.
* **Only the top 15 of 400 000 were pulled.** The rung ladder reads altitude at
  the Goal band straight off the oracle at ~1 s per tape; a deeper sweep of the
  official field is cheap and is the obvious way to look for anyone who coasts.
  My best seed was rank **10**, not rank 1 — the fastest official is not the
  highest, so depth is likely to pay.
* **The `--qlevels 3` family (7 values) has no finisher yet.** It reached no rung
  in 18 000 evaluations; a longer run with the full ladder is the natural next
  step, and 7 values is the alphabet a keyboard player would actually have.
* **A per-action tolerance**, rather than a whole-block shift. The block shift is
  the fleet's comparable probe but it perturbs every input at once; the number a
  player wants is how late the *release* may be, on its own.
