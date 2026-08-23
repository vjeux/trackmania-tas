# 267460 `Impossible Mini Trial 2` — the ending is not a route with a tolerance, and it is repairable

Arm `ending`, 2026-08-23, node `102237.od.fbinfra.net`, branch `ending`.
Times in **seconds**, speeds in **m/s**.

AT **16.888** · human WR **23.068** (Wirtual) · incumbent **21.022**, unchanged.

---

## 0. The three headlines, and two of them replace the last arm's

`CREST.md` closed with *"the whole thing reduces to one number: 2.35 m/s at the
turbo pad"* and handed over a converge-and-graft. Both halves are wrong, and
each has a measurement that says so — and the third headline is the way out.

> **1. The runway defect does not exist.** The searched pit was said to apex
> 1.6 m high, land at x = 832 against the incumbent's 815, and have 12 m of deck
> instead of 30. **The incumbent lands at x = 830.9.** Its "815.2" was read off
> its own APEX — 1.46 m above the deck, clearing the wall, not touching it.
> The two tapes touch down **1.2 m apart** and both have ~12 m of deck. The
> corrected objective was correcting a defect that is not there.
>
> **2. The ending does not tolerate 2.35 m/s. It does not tolerate 0.45.**
> A **one-tick** throttle lift — `accel` 1 → 0 for a single 10 ms tick —
> anywhere between race 11.95 and 14.51 makes the lap DNF. Six ticks tested,
> six DNFs. A **one-unit** steer change (1/127 of lock, one tick) at 14.51 DNFs.
> The failure is not the flight: it is a **wall-ride hairpin at x ≈ 1086, four
> seconds later**, whose tolerance is about a metre.
>
> **3. And the ending can be re-driven, which is what makes the whole route
> live again.** Take that broken lap — 0.45 m/s short at the pad, DNF — freeze
> everything up to the drop, and re-search only the hairpin window (400 ticks)
> against arclength along the incumbent's own measured line. In **78 000
> evaluations, four minutes**, it finishes: **21.617, plain oracle, on the file
> as written.**

So 105 grafts DNF'd not because the pit was 2.35 m/s short but because **no
graft inherits this ending at all.** The task was never "converge the pit". It
is "carry a state to the pad, then re-drive the last four seconds", and the
second half is now demonstrated on a real DNF tape.

## 1. Controls

| control | result |
|---|---|
| plain oracle re-simulates the two references | human **23.068**, incumbent **21.022**, exact |
| `ghost tape graft` incumbent head + own tail at its own pad tick 1630, injected, re-simulated | **21.022** — the graft pipeline is still a no-op when it should be |
| **`ghost tape poke` that changes nothing** (`--set brake=0` where brake is already 0, over 20 ticks) | **21.022** — the new tool is a no-op when it should be |
| a 4-tick throttle lift at ticks 2300 and 2400 (**past the 21.022 finish**) | **21.022** twice — a poke after the flag is inert, so a DNF upstream is physics and not my editor |
| a 1-unit steer change at ticks 1300 / 1500 / 1700 / 1800 | **21.022** four times — the intolerance is LOCALISED, not universal |
| the repaired lap of §5, `ghost verify` on the written file | **V7 PASS: the oracle re-simulated the written file at 21.617.** (V6 and V10 fail: the container still carries the donor's telemetry record and Wirtual's identity — a `ghost regen` + `--anonymise` away, and irrelevant to a tape that is not a publishable lap) |
| every trace quoted here | `fk trace`'s own self-check ok |

Rows 4 and 5 are the ones that matter. Without them, "everything I poke DNFs"
is a statement about the tool.

## 2. Where the incumbent actually lands

`fk trace`, incumbent, over the wall and onto the deck:

| race | x | y | speed | what |
|---|---|---|---|---|
| 14.08 | 815.6 | **115.481** | 43.65 | **apex**, `vy` +0.06 — 1.46 m ABOVE the deck |
| 14.41 | 829.6 | 114.190 | 44.15 | still falling, `vy` −8.04 |
| **14.44** | **830.9** | 114.089 | **42.32** | **first contact** — 1.8 m/s gone in one tick |
| 14.47 | 832.1 | 114.014 | 41.75 | settled |
| 14.74 | 843.5 | 114.016 | **43.76** | pad entry |

The deck puts the car's origin at **y = 114.02**. The 115.4 bar is a **wall at
x ≈ 815** that the car jumps; the landing is 15 m past it.

The last arm's searched pit on the same axes: apex **116.15** at x = 815.9,
first contact **832.1**, pad entry **39.78**. **1.2 m of runway different and
4.0 m/s slower.** The speed did not go into runway.

## 3. Where it did go: 9 m of energy height, bought on the spiral

Energy height `y + |v|²/(2·24.8)` — the airborne acceleration this map was
fitted at — measured at x = 786, the same point on both lines:

| | y | \|v\| | energy height |
|---|---|---|---|
| incumbent | 109.39 | **47.28** | **154.5** |
| searched pit | 108.48 | 42.81 | 145.4 |

**9.0 m short, and the map says where it is bought.** The incumbent starts its
final descent at (722.1, **122.0**, 750.0) at race 11.37. The searched pit
starts its run-out at (713.6, **113.9**, 736.3) at race 7.53 — **8.1 m lower on
the spiral.** The pit is 3.8 s early because it skipped a turn, and a turn of
the spiral is the 9 m. That is the same sentence as *"the loop is there for the
drop"*, with a number on it.

## 4. The pad-speed ceiling of the pit line as routed

`min(vx, 100·(arc − 115.45))` is a `min`, so it drives the arc to
`115.45 + vx/100` — it *buys* 0.4 m of overshoot rather than refusing it. With
the runway defect gone there is no reason to score the arc at all: put the gate
at the pad and the arc becomes implicit, because a tape that fails the wall
never reaches the gate.

```
--gate xmin=841,xmax=843,ymin=113,ymax=116,zmin=700,zmax=712
--gate-key 'vx + 4*min(0, 0.6 - abs(pz-705.1)) - abs(bodyright)'
```

`xmax = 843` matters: the pad's boost begins between x = 843.5 and 843.9 on
both tapes, so a box reaching 844 measures boosted ticks and rewards "get
further into the pad".

Seeded from `cr_PAD_1154_at_4365`, **730 050 evaluations at 1030 eval/s**:

| | `vx` at x = 843 | z | slip | crosses at race |
|---|---|---|---|---|
| seed | 39.78 | 707.79 | −0.07 | 11.52 |
| **searched** | **40.71** | **705.18** | **0.04** | **11.48** |
| incumbent | **43.76** | 705.20 | 0.06 | 14.74 |

The z corridor was solved inside the first 270 evaluations and was free
thereafter; the speed plateaued. **The pit line as routed tops out ~3 m/s short
of the pad, which is the 9 m of §3 and not a search failure.**

## 5. The measurement that ends the graft strategy — and the one that reopens it

`ghost tape poke` (new, §7) makes a one-variable probe one command. On the
incumbent:

| probe | ticks | race | result |
|---|---|---|---|
| throttle lift, 1 tick | 1350 / 1400 / 1450 / 1500 / 1550 / 1580 | 11.95 – 14.25 | **DNF ×6** |
| throttle lift, 1 tick | 1606 | 14.51 | **DNF** |
| throttle lift, 2 – 21 ticks | from 1606 | 14.51 | **DNF ×9** |
| brake + lift, 2 – 34 ticks | from 1590 | 14.35 | **DNF ×10** |
| steer +1 of 127, 1 tick | 1606 | 14.51 | **DNF** |
| steer +1 of 127, 1 tick | 1300 / 1500 / 1700 / 1800 | — | **21.022 ×4** |
| brake pressed with throttle HELD, 2 ticks | 1590 | 14.35 | 21.022 — no effect |

**One tick of lifted throttle on the deck is 0.45 m/s at the pad, and 0.45 m/s
is fatal.**

### Where it dies, traced

`p2` — the incumbent with a 2-tick lift at race 14.51, so **43.32 at the pad
against 43.76** — against the incumbent, tick for tick:

| race | p2 | incumbent | apart |
|---|---|---|---|
| 14.74 (pad) | 43.32 | 43.76 | **0.45 m/s** |
| 17.14 (end of the drop) | (977.32, 63.45) 73.73 | (977.51, 63.31) 73.76 | 0.19 m |
| 17.36 (grass) | (992.45, 57.98) 72.85 | (992.74, 57.99) 73.51 | 0.29 m |
| 18.26 | (1057.3, 44.38, 699.3) | (1058.3, 44.32, 699.8) | 1.0 m |
| **18.41** | `vz` **−10.6**, still straight | `vz` **−21.9**, already turning | **the hairpin** |
| 18.86 | x **1091.6** | x **1087.3** | 4.3 m |
| 19.46 | (1087.1, 42.1, 649.9) | (1066.5, 48.2, 652.2) | 21 m |
| 22.1 | y = 10, off the map | finished at 21.022 | — |

The whole 60 m drop and the grass touch are reproduced to **0.3 m**. The lap is
lost in the **hairpin at x ≈ 1086**, where the car turns against a wall: the
incumbent reaches it a metre earlier, bites, and comes back; p2 arrives a metre
late, runs 7 m further east, and comes back on a line that falls off the map.

**Retiming does not fix it.** Ten tapes grafting p2's head onto the incumbent's
tail shifted −6 … +6 ticks: **all DNF.** The error is positional, not temporal.

### And re-driving it does

Same tape, everything up to race 17.45 frozen, editing **ticks 1900–2300 only**,
fork at 1800, ranked by arclength along `cr_inc_trace.csv`:

```
decoy test: the do-nothing tape (400 editable ticks blanked) scores DNF 440 m
of 560 (79%); the incumbent scores DNF 518 m of 560 (93%)
```

The seed beats do-nothing, so this is not the `CLAIMS.md` §3 trap. Then:
518 m → 542 m in 10 000 evaluations, **first finish at 22.05 by 35 000**, and
**21.617 at 78 000 (four minutes)**, still improving when it was stopped for the
next experiment.

> **A 0.45 m/s deficit at the pad costs ≤ 0.60 s once the ending is re-driven,
> and costs the entire lap if it is not.** That is the exchange rate the next
> arm needs, and it is the number `CREST.md` was missing.

## 6. The real graft, and why matching the pad state is not enough either

Grafting the §4 pit (pad at race **11.44** at 40.71, **3.31 s** ahead) onto the
incumbent's tail at tick 1630 gives `cv_GRAFT.Ghost.Gbx`: DNF, and traced it
fails for a **new** reason that is not speed:

| race | x | y | z | speed |
|---|---|---|---|---|
| 11.81 | 863.6 | 114.01 | 705.20 | **60.67** — the pad boost worked |
| 11.89 | 868.4 | 113.87 | 705.39 | 59.07 — above the 58 the flight wants |
| 14.14 | 992.3 | 57.31 | **710.95** | 69.2 — where the incumbent TOUCHES the grass |
| 15.14 | 1049.5 | **8.20** | 714.0 | it went straight past and fell |

At the launch the incumbent has `vz` **+0.12**; this graft has **+2.4**, and
over the 130 m flight that is **6.1 m of lateral drift**. It reaches the grass
slope's height at the grass slope's x and misses it **sideways**.

**So I put the launch velocity into the pad gate and it made things worse**,
which is the finding:

```
--gate-key 'vx + 4*min(0, 0.6-abs(pz-705.1)) - abs(bodyright)
            - abs(vy) - abs(vz + 1.53)'     # the incumbent's own pad velocity
```

646 530 evaluations. Pad entry (842.80, 114.01, **705.18**) v (**40.59**,
**−0.14**, **−1.53**) — z, `vy` and `vz` all on the incumbent's own numbers,
slip 0.02. Grafted (`cv_GRAFT2`), its launch `vz` is **+3.7**, worse than the
+2.4 of the graft that did *not* match.

The reason is in the tail. The incumbent's inputs at ticks **1633–1637** are
**five ticks of full lock** (`steer` 123, 127, 127, 127, 127) — a flick applied
while the pad is accelerating it through 46 m/s. That flick nets the incumbent
`vz` −1.53 → +0.12. The same five ticks applied to a car doing **40.6 instead
of 43.7** turn it much harder, and net **+3.7**.

> **The incumbent's tail is not a set of inputs, it is a set of inputs
> calibrated to the incumbent's own speed.** A graft that matches position,
> velocity, attitude and slip at the handover STILL diverges, because the
> steering that follows was tuned against a speed it no longer has. That is the
> mechanism under §5's one-tick result, and it is why the handover plane cannot
> be pushed earlier or later out of trouble.

An ending search over the frozen first graft ran the arclength metric to
**471 of 471 m (100%)** in 103 000 evaluations and still DNF. Repeating it on
`cv_GRAFT2` with everything after the handover editable (ticks 1311–2100) and
`--corridor 10` — which does bite: it drops the seed from 448 m to **311 m of
470 (66%)**, so there is a real gradient to climb — went 311 → **470 m (100%)**
in 400 000 evaluations, and still DNF.

> On `p2` the finish arrived at **97%** of the reference line. Here a candidate
> exhausts **100%** of it and never crosses. **The arclength gradient saturates
> before the flag, and once it does the ladder needs a second rung** — a gate
> on the finish plane, an `--after-key` measured from the hairpin — or the
> search is climbing a hill whose top it is already standing on.

## 7. What the next arm should do

1. **Give the ending search a second rung.** The arclength gradient takes the
   graft from 66% to 100% of the reference line and then has nothing left, and
   100% is not the flag. Add a finish-plane gate or an `--after-key` opened at
   the hairpin, run the decoy test the tool prints, and only then spend hours.
   Everything before that rung already works: `p2` proves a DNF lap re-drives
   to a plain-oracle finish in four minutes.
2. **Re-drive from the PAD, not from the top of the drop.** §6 is the reason:
   the incumbent's five-tick flick at ticks 1633–1637 is calibrated to 43.7 m/s,
   so those ticks have to be re-driven for any car that arrives slower. Editing
   only the hairpin (which is what worked for `p2`, whose speed was almost
   right) cannot fix a launch that is already 6 m off line.
3. **Or make the pit fast enough that the incumbent's tail is still valid** —
   buy the 9 m of §3, one more turn of the spiral, ~1.8 s on the human's own
   timings. That still leaves this pit ~1.5 s ahead of the incumbent's, so a
   **19.x** lap, against the **18.x** the re-driven ending would be worth.
4. **Do not re-run the crest arc objective, and do not quote 2.35 m/s.** The
   overshoot costs 1.2 m of runway, not 17, and the ending's tolerance is under
   0.45 m/s, not 2.35.

## 8. Tooling

| what | why |
|---|---|
| **`ghost tape poke IN --out T.gtape --ticks A..B --set steer=..,accel=..,brake=..`** | Override the vehicle inputs over a tick range and leave every other tick identical. Every row of §5 is one invocation. Until now a one-variable probe meant hand-editing `t=` lines, which is how an off-by-one gets into a number nobody can reproduce. It refuses a range the tape is not that long for, refuses a field that is not a vehicle input, and reads the tape back before writing it. |

Nothing else needed adding: the pad gate, the graft, the trim and the ending
search are all commands that were already there.

## 9. Artefacts

Banked to `~/persistent/private-30d/tm-unbeaten/267460/en_20260823/`, with
`en_MANIFEST_v1.md5`.

| file | what |
|---|---|
| `en_REPAIRED_21617.Ghost.Gbx` | **the repaired lap: a DNF tape re-driven to 21.617, `ghost verify` V7 PASS on the written file** |
| `en_p2_DNF_pad4332.Ghost.Gbx`, `en_p2_trace.csv` | the 2-tick lift: 0.45 m/s short at the pad, DNF, and its trace through the hairpin |
| `en_PAD_4071.Ghost.Gbx` + `.state.json` | §4's pad search: `vx` 40.71 at x = 843, z 705.18, race 11.48 |
| `en_PAD2_vmatched.Ghost.Gbx` + `.state.json` | §6's: (40.59, −0.14, −1.53) at z 705.18 — the incumbent's own pad velocity |
| `en_GRAFT_1299_1630.*`, `en_GRAFT2_1297_1630.*` + traces | the two grafts, and the 6.1 m / worse lateral miss |
| `en_GRAFT2_endsearch_best.Ghost.Gbx` | 100% of the reference line, still DNF |
| `en_bestpit_trace.csv` | the trace §2 and §3 read the pit off (the incumbent's own is the previous arm's `cr_inc_trace.csv`) |
| `en_padsearch*.log`, `en_endingsearch.log`, `en_graftsearch*.log` | all four searches, decoy lines included |

**None of these is a lap under 21.022. The incumbent is still 21.022.**
