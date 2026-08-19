# 270053 — `Fall 2025 - 18 CP1 End` — AUTHOR TIME EQUALLED (4.492 s), human record beaten by 3 ms

AT **4.492** · best human ever **4.495** (five players tied) over **973 records**
· uid `6r7HjKPCuImnLMBfqiKwWpGK1U1` · TMX 270053 · author `in-.-`.

**Validated result: 4.492 s** — `270053/tas_4492_v1.Ghost.Gbx`, md5
`37614b66da394415c26195f903f1afd3`. Re-validated cold through the plain oracle
in a fresh process alongside **all 15 downloaded human ghosts as known-answer
controls, every one reproducing its leaderboard time exactly**
(`270053/validation_transcript_final.txt`).

**Not beaten outright.** True crossing of our tape is **4.49286 s**; reporting
4.491 needs 4.49199 — 0.87 ms, or 7.8 cm of travel at the line, further. Section
"why not 4.491" below says why that is a wall and not a lack of budget.

Driver write-up: `270053/HOW_TO_DRIVE_IT_v2.md`.

## The map

450 race ticks. Full throttle lights to line, **never lifted, never braked**,
always on the ground, and — the decisive fact — **it never slides**: lateral
speed stays under 0.7 km/h all the way round. The long downhill sweeper is taken
at FULL LOCK for ~128 consecutive ticks, so the middle of the corner has no free
parameter at all: it is steering-angle limited, not grip limited. One waypoint
only (a custom `sausagecpfin` item), so no segment maps, no shaping, no
checkpoint ladder — and no need for any, because **100% of single-tick
perturbations finish** here, against the 57-79% DNF rate of the 23 s maps.

Oracle throughput: **2500 candidate simulations/second** on 176 cores. A
whole-tape exhaustive single-tick sweep (450 ticks x 255 steer values = 114,751
simulations) costs **46 seconds**. On a map this short, enumerate — do not
sample.

## Where the time actually is — ablation, not opinion

This is the finding worth carrying to the other CP1-End maps, and it is the
opposite of what the search's own history suggested. Splice one region of the
fast tape into the human record and re-simulate:

| human record, with one thing changed | time |
|---|---|
| nothing | 4.495 |
| **one tick of extra lock at 0.42 s** (−87 instead of −66) | **4.493** |
| the entire corner-exit release, alone | 4.495 — **worth nothing** |
| everything the TAS does *except* that early lock | 4.495 — **worth nothing** |
| both together | **4.492** |

So the spectacular-looking part (releasing the exit two tenths early) pays only
once the early lock is there. **The human field is under-steering the first
half-second.**

And it is a broad effect, not a knife-edge — every single-tick value at every
tick of the turn-in was enumerated:

- **timing**: an extra stab of lock anywhere from **0.24 s to 0.77 s** gains
  1 ms; six separate moments in that window gain 2 ms.
- **amount**: at the best moment, *anything* from −83 to full lock −127 gives
  4.494 (−87 gives 4.493) — a 45-unit window on a 127-unit axis.
- **shape**: a sustained 3-8 units of extra lock over 5-10 ticks, or a smooth
  10-tick raised-cosine swell of −5, anywhere in that window, also gives 4.494.

## Transferable: the sub-tick vernier, four refinements past the basic idea

Relocating the goal ITEM back along the direction of travel (**1 ms = 8.75 cm**
here, 11.4 ms/m) is essential and, on its own, useless. Four things had to be
added before it produced anything:

1. **AUTO-CALIBRATE every round.** A fixed ladder goes blind after one accepted
   edit — the incumbent lands just inside a rung and everything ties again.
   Measured exactly that: 1724 improving candidates in round 0, then **0** in
   round 1. Binary-search the incumbent's own crossing and re-aim at it.
2. **TWO-SIDED** (half the rungs above the incumbent's crossing, half below). A
   one-sided ladder cannot distinguish a worse candidate from an equal one —
   both flip nothing — so a beam search on a plateau sorts by array index and
   marches the whole beam into full lock.
3. **CASCADE it.** A candidate that fails the easiest rung never needs the rest.
   200k extra simulations per pass became a few thousand; a full sweep went from
   160 s to 55 s.
4. **Calibration must be PARALLEL.** 26 sequential one-candidate validations at
   2.2 s of server startup each is 60 s per re-aim — more than the layer it
   serves. One parallel round of 48 offsets gives the same answer.

Resolution reached: 12 rungs over 0.002 m = **1.9 microseconds**.

**And the resolution is what decides whether there is a gradient at all.** Same
incumbent, same 114,751-candidate sweep, three ladder spans:

| rung | improving candidates |
|---|---|
| 28.6 us | 0 |
| 11.4 us | 0 |
| 5.7 us | 600 |
| 1.9 us | 1964 |

A search that reports "converged" has often only run out of ruler.

## Why not 4.491 — the negative results, all measured, all with an identity control in every batch

| tried | outcome |
|---|---|
| every single-tick steer value, whole tape, repeatedly, down to 1.9 us resolution | best remaining move is worth **~10 microseconds** |
| uniform blocks 2-34 ticks; scale; retime; doublets | 0 improving |
| throttle lifts 1-90 ticks anywhere | 0 improving; lifts over 10 ticks finish only 23% |
| braking 1-60 ticks anywhere | 0 improving; finish only 30% |
| **the whole corner exit as a 5-parameter shape**, 169,793 combinations | 0 improving — and **54,777 of them also give 4.492** |
| **bilevel: entry edits x exit shapes, the exit re-solved for every entry, entries admitted even when they lost 2 ms** — 1,190,400 pairs with single-edit entries, then 3,036,000 more with random 3- and 4-edit entries | **0 improving** |
| the countdown, ticks 0-151, every steer value + lifts + brakes | 0 improving; 39,322 of 51,001 candidates bit-identical — pre-start input is inert here |
| multi-start from other human seeds | rank 2 (4.495) is already at a single-tick optimum and gains 0.14 ms in a full round |
| finish-trigger geometry in all three axes | no margin — see below |

The gap is 0.87 ms and the largest move the search can still find is 10 us. That
is not a budget problem; it is a wall. **4.4929 s is the floor of this route
family**, and the author's 4.492x sits inside the same millisecond we reached.

## The finish trigger, measured in all three axes

Worth knowing on every CP1-End map. Slide the goal item, re-time:

- **along travel (x)**: linear, 11.4 ms/m. This is the vernier.
- **laterally (z)**: the run already crosses at the earliest point. 0.5 m either
  way is slower (+2 ms tighter, +10 ms wider) and **2 m wider does not finish at
  all**. No hidden margin here, unlike 279197.
- **vertically (y)**: same story. **0.25 m lower and the car misses the trigger
  entirely**; higher is monotonically later.

The crossing point is a local optimum in all three axes at once: the search
drove the car onto the trigger's earliest corner and the geometry has nothing
left.

## Cross-checks on things other agents raised

- **The "tick-0 throttle idle" lever does not exist on this map.** Every one of
  the 15 human tapes has accel = 1 from its own first recorded tick and never
  lifts; two of them (p00004, p00005) record no countdown at all and still
  validate to 4.495, which independently confirms the pre-start region is inert.
- **`tmmaps segments::move_gate` must not be used on a CP1-End map**: it swaps
  in the stock finish-gate model and thereby deletes the only Goal. `tmmaps
  gateshift` (added here) translates the item **keeping its model**.

## Method note — why the fork-resume phantom class cannot arise here

Every candidate in this whole effort was evaluated through the plain
`/validatepath` oracle. No fork server anywhere. One search process at a time,
each with an explicit distinct `--root`, and **index 0 of every batch is the
untouched incumbent whose exact score must come back or the run aborts**. The
banked tape was then re-validated cold, three times, beside 15 known-answer
controls.

## Tools left behind

`270053/tools/tmlayer-src-v2.tgz`:

- **`tmlayer`** — a whole-tape layered search for short maps. Exhaustive and
  parametric families (single-tick, block, raised-cosine bump, doublet, scale,
  retime, a 5-parameter corner-exit shape), each optionally confined to a tick
  range with an `@LO:HI` suffix; the auto-calibrated two-sided cascaded vernier;
  a beam sweep; a bilevel outer x inner product search; `--splice` for tape
  ablation; and `--table`, which prints every candidate's time so a technique's
  *tolerance* can be measured rather than assumed.
- **`tmmaps gateshift`** (translate the goal item, keeping its model),
  **`tmmaps blocks`**, **`tmmaps items`**.
