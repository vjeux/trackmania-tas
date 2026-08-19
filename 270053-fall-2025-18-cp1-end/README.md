# Fall 2025 - 18 CP1 End — author time equalled, human record beaten

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **4.492** | **±0** | **−0.003** |
| TAS, single-tick variant | 4.493 | +0.001 | −0.002 |
| Author time (never beaten by a human) | 4.492 | — | −0.003 |
| Human WR (five players tied) | 4.495 | +0.003 | — |

TMX map [270053](https://trackmania.exchange/maps/270053) · uid
`6r7HjKPCuImnLMBfqiKwWpGK1U1` · author **in-.-** · **973 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## What this is

The author time exactly, on the most-hunted map in this collection: **973 people
have tried and the best of them is 3 ms short**. Equalling rather than beating
is the honest description — the tape reaches 4.492, the author's own validation
lap reached 4.492, and nobody in 973 recorded attempts has done either.

The interesting part is not the millisecond. It is that **the 3 ms turned out to
be a coarse, forgiving driving change**, not a frame-perfect trick.

## Where the time is: you are not turning hard enough in the first half-second

Established by ablation on the human record's own tape — change exactly one
thing and re-simulate:

| what changes on the human record | time |
|---|---|
| nothing (the human record) | 4.495 |
| **one tick of extra steering lock at 0.42 s** | **4.493** |
| the whole corner-exit release, and nothing else | 4.495 — **nothing** |
| everything the TAS does *except* that early lock | 4.495 — **nothing** |
| the early lock **and** the exit release together | **4.492** |

The search's own story pointed at the spectacular-looking finish. The ablation
says the finish is worth **zero** on its own. The whole margin starts with the
turn-in: the human record holds about 52% lock (−66 of 127) through it, and the
car wants more.

### How forgiving

Every single-tick steering value at every tick of the turn-in was enumerated:

- **Timing** — an extra stab of lock anywhere between **0.24 s and 0.77 s**
  gains a millisecond; six different moments in that window gain two. There is
  no frame you have to hit.
- **Amount** — at the best moment, *any* value from about **−83 to full lock
  −127** pays. A 45-unit window on a 127-unit axis.
- **Shape** — you do not need a stab at all. An extra 3–8 units held for 5–10
  ticks, or a smooth 10-tick swell, also pays. Sharp, smooth, brief, sustained:
  all of them work.

In driver language: **turn in a little harder in the first half-second than
feels natural, and let it breathe back out.** It costs almost nothing at
20–40 km/h and it sets up the whole rest of the lap.

### The exit, worth ~1 ms once the entry is right

The human record holds full left lock to 4.35 s then snaps the counter-steer in.
The fast tape starts unwinding at **4.16 s** and rolls it off progressively —
about two tenths earlier. What the finish clock measures is the part of your
speed pointing *through* the line: while you hold lock the car is still
rotating, and every degree not yet cancelled is speed thrown across the line
instead of through it.

Also forgiving: of **169,793** exit shapes enumerated, **54,777 also produce
4.492**. There is a wide family of correct exits.

### The warning

The finish trigger is narrow with a hard edge on the outside, and the fast line
passes about half a metre from it. **Half a metre wider costs 10 ms; two metres
wider and the run does not finish at all** — no time, no explanation. 25 cm
lower and the car misses the trigger entirely. So read "release earlier" as
*stop turning sooner*, never as *run wider*.

Full driving guide: [`notes/HOW_TO_DRIVE_IT.md`](notes/HOW_TO_DRIVE_IT.md).

## Why not 4.491

~12 million simulations say the map is closed at 4.492. The largest move still
findable is worth **10 µs** against a **870 µs** gap. Exhausted, with an
identity control in every batch: all single-tick values repeatedly, blocks,
scales, retimes, doublets, bumps, every lift and brake, the countdown (inert),
the corner exit as a 5-parameter shape, and bilevel entry×exit with the exit
re-solved for every entry — **4.2 million pairs, zero improving**. The finish
trigger is a local optimum in all three axes.

The true crossing, measured to 2 µs by walking the finish plane, is
**4.49286**. Reporting 4.491 would need 4.49199 — 0.87 ms, or 7.8 cm at the
line.

## Validation

Cold re-validation in a fresh process against the untouched map, alongside
**all 15 downloaded human ghosts as known-answer controls — 15/15 reproduced
their leaderboard millisecond exactly**. Transcript in `notes/`.

No fork server was used anywhere on this map: every candidate went through the
plain oracle, so the phantom class cannot arise here.

## Files

| file | what |
|---|---|
| `replays/tas_4492_v1.Ghost.Gbx` | the run |
| `replays/tas_4493_singletick_v1.Ghost.Gbx` | the 4.493 single-tick variant |
| `replays/ablation_early_only_4493.Ghost.Gbx` | human record + the early lock only |
| `replays/ablation_exit_only_4495.Ghost.Gbx` | human record + the exit release only — worth nothing |
| `inputs/tas_4492_v1.inputs.csv` | per-tick inputs |
| `inputs/human_wr_4495.inputs.csv` | the human world record's inputs, for comparison |
| `notes/HOW_TO_DRIVE_IT.md` | the driving guide |
| `notes/RESULTS.md` | the full write-up |

## This map is an Altered Nadeo copy of **Fall 2025 - 18**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **76 975
players** on this geometry.

This is the best case of the ten. It is a **CP1 End** variant, so the official map's opening **is** our entire race — those humans have all driven exactly our sector, at full commitment, as the start of their own lap.

**Official tapes demonstrably run on this map.** Twenty official human ghosts have been grafted onto altered copies and each returned its own official time or split to the millisecond, so this is a demonstrated pipeline rather than a statement about physics. The graft recipe is map-dependent — carry the inputs chunk only, or all three, and pick whichever one's lossless control passes in the same batch.
