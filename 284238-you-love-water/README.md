# YOU LOVE WATER — the route that beats this author time exists, and one missing run-up is why we cannot drive it

**Author time 50.459 · the only human record 440.238 · best validated 97.325.**

> **The route that beats this author time exists and is 3.0 s better than it.**
> A human's line on the byte-identical sibling map, carried onto our map with the
> launcher penalty paid in full, is **47.4 against an author time of 50.459.**

| tape | validated | note |
|---|---|---|
| [`TAS_97325`](replays/TAS_97325.Ghost.Gbx) | **97.325** | the human's own driving with the retries cut, plus 0.136 s of search |
| human record, brick555 *(control)* | 440.238 | contains **31 respawns** |
| author time | 50.459 | — |
| *the sibling human's line, priced onto this map* | *47.4* | *a construction, not a driven tape — see below* |

TMX map [284238](https://trackmania.exchange/maps/284238) · 4 checkpoints ·
**exactly one recorded run.**

**Not submitted to any Nadeo leaderboard, and it never will be.** No time gain is
claimed here: the best validated tape on this map is 97.325.

This map is published as a **characterisation**. The author time did not fall,
and the reason it did not is now measured end to end rather than guessed at — and
it turns out to be a single missing 100 metres of flat road.

---

## The map is one 40-block module placed four times

Its author (Eating_My_Wings, 486 maps on TMX) reuses that module
byte-identically. **279008 "Keep dropping" is this map with the launchers
changed**: 167 of its 186 block records are identical — same block, same absolute
position, same angles — and its four checkpoint gates sit at the *same world
coordinates*. What differs is 19 records: 15 water ramps swapped for tech ramps,
the start block, three ice blocks nudged under a metre, the finish net — and
**six boost-pad items that 279008 does not have**.

Its author time is 52.461, and:

> **Yhomas_TM holds 46.112 on it — a human, beating that map's author time,
> driving our obstacle, in a clean single-life run.**

That ghost validates at 46.112 on its own map, has **zero respawn packets** and a
single vehicle entity holding 923 of 923 samples. It is an answer key in the
strict sense.

### How the sibling was found — the method is the reusable part

1. The author's map list from the TMX API, paginated → 486 maps.
2. Download each at ~1 request / 1.5 s with a descriptive User-Agent (never a
   browser UA).
3. **Fingerprint by block census** — the count of each block model per file.
4. Confirm geometrically: sort `name,x,y,z,pitch,yaw,roll` and diff the two
   files. Identity of block *records*, not just of counts.

Every instrument used carried a two-sided control, including the important one: a
probe at Yhomas's wall-curve contact point that **says no for our record and yes
for Yhomas at 15.278 on the identical geometry of 279008.**

## What the map turns on: 9.5 metres of lateral position at one wall

Each cycle is kicker → flight → wall curve → checkpoint, and the whole cycle is
decided by the canonical z at which the car meets the wall curve:

| | at the wall's height | one-tick speed loss | checkpoint crossing |
|---|---|---|---|
| our record, cycle 1 | z **923.4**, v 77.4 | **8.71 m/s** | **45.80** |
| our standing start *(works)* | z **915.4**, v 73.1 | — | — |
| Yhomas, all four copies | z **913.9**, v 80.8 | **0.75 m/s** | **69.40** |

**Nine and a half metres.** Everything downstream follows from it: the 1630-vs-311
energy loss, the crossing-speed decay 52.8 → 45.8 → 41.1 → 37.4 as the cycles go
on, and whether the next cycle clears the 71 m gap at all.

**And it is set by lateral velocity built up on the flat before the kicker**,
which is bounded by (time on the flat) × (lateral acceleration available):

| | time on the flat | vz achieved |
|---|---|---|
| copy 0 — start platform, ~100 m of deck | ~2 s | **−17.9** |
| copies 1–3 — fed by the tube | ~0.6 s | −1.9 (our record) … **−15.7** (full lock, the most that fits) |
| Yhomas, every copy — tech-block launchers | a flat run-up in each | **−24 … −25** |

**Only copy 0 has a long flat run-up.** Copies 1–3 are fed by the tube, which — by
construction of the map's screw symmetry — is the *only* connection between one
copy and the next. They arrive on the lane 100 m late with 0.6 s of flat left, and
in 0.6 s the car cannot build the lateral velocity the wall contact needs.

**That is the map.** Not speed, not energy, not grip, not the boost pads.

Three things it is **not**, each measured rather than argued:

* **not grip** — full lock buys 13.4 m/s on our water lane and 13.2 on his tech
  lane;
* **not speed** — the kicker is crossed at 97.2 (ours, fails), 99.1 (his, works)
  and 90.9 (our standing start, works), and copy 0 is not slow at the lane
  either, reaching 90.7 m/s;
* **not the six boost pads** — they sit on the flat *after* the aim is decided,
  restoring speed the arc lost, one second too late to change where the car is
  pointing. *(An earlier version of this page said the pads force too much speed
  into the catch. That was withdrawn on this evidence.)*

## The positive result at the centre of it

**Our own standing start flies Yhomas's launch to within 2–7 metres, point for
point, in order** — 2.78 m, 2.20, 4.06, 5.93, 6.78 at five stations, measured per
tick on the untouched map.

The target line is not exotic, is not beyond this car, and **is already in our own
record at 4.2–5.2 s**. What the map withholds is not the line. It is the approach
that produces it.

That is also what makes the 47.4 construction meaningful rather than arithmetic.
A closed-loop controller fitted on Yhomas's run transfers to our copy 0 at
**0.02 m median lateral error** and crosses CP1 at **64.4 m/s** against our
record's 52.8 — the first time anything on this map has crossed a checkpoint at
the sibling human's speed.

## The launcher, priced exactly

Hold the line fixed and vary only the map. Copy 0 differs from its sibling by
**exactly one block record**, and that one block — the water start — costs
**1.30 s, all of it in the first 1.4 s of the race**: standing acceleration
8.1 m/s² against 19.3, with the gap flat at +1.24…+1.31 from phase 180 onward.
After that our car matches his acceleration exactly.

`46.112 + 1.30 = 47.4`, against an author time of 50.459.

> That is the strongest evidence this map has produced that **50.459 was driven**,
> and that the author's route is essentially the one Yhomas drives on the remix.

## Every lever, and how each one closed

A negative is only worth reading if it is enumerated. These are the families
tried, with their sizes:

| lever | enumeration | outcome |
|---|---|---|
| lane steer, one window | 60+ variants | one locus; the target sits 9.5 m below it |
| two-window pulse + counter-pulse | 36 | every one destroys the run |
| throttle restoration | 6 windows | lane speed up to 109.7, lateral still ≈0, misses by 107–129 m |
| arc steer, phase grid | 78 | a **trilemma** — peak height, peak x, peak speed: any two, never three |
| arc steer + throttle | 6 500 evaluations, 2 seeds | tops out 6 m short inside the CP2-collecting basin |
| steer + brake (scrub) | 16 | buys contact height by spending speed |
| per-copy entry | geometry, no search | the tube **is** the connection; no copy 1–3 entry avoids it |
| respawn delivery | 31 presses, 4 measured per tick | restores the crossing state at full speed; freeze is exactly 1.010 s |
| slow arrival | 12 windows | monotonically worse — and it corrected the hypothesis: copy 0 is slow on the *deck*, not at the lane |
| previous cycle's exit | 20 | 5 inert, 15 destroy the run, 0 improve |
| **a CP2-free rung-ladder march** | 10 000 evaluations at 8 / 4 / 2 m tolerance | **the experiment built to break this account** — at discriminating tolerance it stalls at depth 3 and depth 1 |

**Falsifiable form**, so the next person can attack it directly: *any tape
entering a copy 1–3 launch out of the tube meets the wall above canonical
z ≈ 920 and loses the cycle, because the lateral velocity obtainable in 0.6 s at
~96 m/s is bounded below what the contact needs.*

The two measurements that looked most like counterexamples are consistent with
it: the arc *can* reach the target crossing height, and arrives at the wrong x
with the wrong speed; the yaw *is* available on our lane, and 0.6 s is not enough
of it.

## What would change the answer

**A launcher that gives copies 1–3 a flat run-up.** That is not a hypothetical —
it is the substitution the author himself made in his own remix, and on that
remix a human beats the author time.

If someone picks this map up again, the two things worth doing are: give the
closed-loop policy the copy 1–3 problem (can a controller exceed vz −15.7 inside
0.6 s of flat?), adjudicated by the ladder at ≤ 3 m tolerance; and re-run one
representative family with the respawn stripper, because **`ghost::Factory`
cannot see or remove a respawn**, so a whole-tape search silently inherits its
template's retry schedule.

## Three findings from this map that generalise

**Hold the line fixed and vary the map.** That is how the launcher got a price
instead of an adjective. Two maps differing by one block record, the same tape on
both: 1.30 s, and the phase profile says exactly where it is paid.

**A detector must say yes, say no, *and* resolve finer than the effect it is
measuring.** An 8 m rung ladder on a 9.5 m effect reported depth 7 of 7 with the
winner on the wrong branch of the split. Calibrate a detector against what you
want to **exclude**, not only against what you want to find.

**`ghost::Factory` is blind to respawns.** Any search that writes steer, gas and
brake inherits its template's retry schedule invisibly. It does not affect
anything in this account — every family here perturbs a window inside cycle 1,
between the record's respawns — but it is the caveat to carry into any whole-tape
search.

## A trap this map found that defeats one of our own safeguards

A 14.7-minute search against a segment map reported **−13.975 s**. The winner
collects only CP1 and CP2 on the untouched map: it had found the promoted gate's
**enlarged trigger volume**, not a route.

**And the origin round-trip control passed throughout** — moving the gate back to
its true position reproduced the untouched map exactly, because the *position*
was restored correctly. The defect is in the **volume**, and a control that only
exercises position cannot see it.

> **A control validates the property it exercises, and nothing else.** When you
> adopt one, write down what it cannot see.

Substitutes, both with calibrations: a position-only rung mover, and a finish
placed 15–50 m *beyond* a real checkpoint so the checkpoint the candidate must
satisfy stays the map's own untouched trigger.

## The 8.7× headline is retry cost, not pace

The one human record contains **31 respawns**. On a Trial-family map the clock
runs through them, so a recorded time is clean driving plus every failed attempt.
Taking his own last, successful attempt in each sector gives **93.914 s of clean
driving** — and our 97.325 is that, with the retries cut and 0.136 s of search on
top.

## Files

| file | what |
|---|---|
| `replays/TAS_97325.Ghost.Gbx` | best validated run |
| `notes/WHY_THIS_IS_HARD_final.md` | **the closing account** — the 9.5 m, the bound, every lever |
| `notes/POLICY_TRANSFER.md` | the closed-loop transfer, the launcher price, and the 47.4 |
| `notes/SIBLING_MAP_ANSWER_KEY.md` | how 279008 was found and what Yhomas's cycle demands |
| `notes/COPY0_IS_HIS_LINE.md` | our standing start against his launch, point for point |
| `notes/CP2FREE_LADDER_MARCH.md` | the experiment built to break the account |
| `notes/RUNG_TOLERANCE.md` | a rung must resolve finer than the effect it measures |
| `notes/RESPAWN_CANNOT_DELIVER.md` · `notes/LAUNCH_STATE.md` · `notes/TUBE_REACHABLE_SET.md` | the closed levers |
| `notes/RESULT-symmetry.md` · `notes/RESULT-v1.md` · `notes/GEOMETRY.md` | the four-copy derivation and the original recon |
