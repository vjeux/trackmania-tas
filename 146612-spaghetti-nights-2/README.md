# Spaghetti Nights 2 — the world record falls by 0.745 s, and the jump that looked decisive is dead

**Author time 38.530 · human world record 40.223 · best validated 39.478.**

| tape | validated | vs human WR | vs AT | steer values | steer events |
|---|---|---|---|---|---|
| [`TAS_39478`](replays/TAS_39478.Ghost.Gbx) | **39.478** | **−0.745** | +0.948 | — | — |
| [`TAS_39748`](replays/TAS_39748.Ghost.Gbx) | 39.748 | −0.475 | +1.218 | — | — |
| [`BEST_39961_v3`](replays/BEST_39961_v3.Ghost.Gbx) | 39.961 | −0.262 | +1.431 | 76 | 234 |
| [`KEYBOARD_39996_v3`](replays/KEYBOARD_39996_v3.Ghost.Gbx) | **39.996** | −0.227 | +1.466 | **3** | **119** |
| author time | 38.530 | — | — | — | — |
| human WR, jujumasterr *(control)* | 40.223 | — | +1.693 | 226 | 1157 |
| human rank 2 *(control)* | 40.226 | +0.003 | +1.696 | 3 | 114 |

TMX map [146612](https://trackmania.exchange/maps/146612) · uid
`jchzEcocJbNJreH4ebIoUYOt286` · authors **AmpelJoe10 + Wakawukwuk** ·
**181 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

*This map is moving hour by hour and several arms are live. `TAS_39478` and
`TAS_39748` re-validated here on the untouched map (sha256 `c6cca762…`) with
both human records exact in the same batch; their own write-ups had not been
banked when this page was updated, so the provenance below covers the map, not
those two specific tapes.*

---

## The headline this page used to carry, and why it is now wrong

Earlier tonight this page led with a genuinely exciting measurement:

> Sector 4 contains a 190-metre gap jump that all 181 humans either avoid or take
> wrong, and taking it right is worth **1.128 s to checkpoint 5** — two thirds of
> the entire unbeaten gap.

**That measurement is still correct and the route built on it is dead.** The jump
reaches checkpoint 5 at 32.702 against the best human's 33.830, on the untouched
map, with all five real checkpoints collected. And then it gives every
millisecond back within eighty metres:

| station | ≈ m past CP5 | human WR | best jump-lineage tape | Δ |
|---|---|---|---|---|
| st01 | 27 | 33.931 | 33.318 | **−0.613** |
| st02 | 54 | 34.281 | 33.970 | −0.311 |
| st03 | 82 | 34.632 | 34.649 | **+0.017 — break-even** |
| st05 | 135 | 35.282 | 35.753 | +0.471 |
| st08 | 215 | 36.171 | 37.105 | **+0.934** |
| st13 | 351 | 37.588 | 40.158 | **+2.570** |

Loss rate over the stretch where both tapes are on the road: **+183 ms per
station, about +7 ms per metre**, with fourteen stations still to run. **No tape
in this lineage has ever finished the map** — two methods and roughly two
node-hours reached station 14 of 22.

The recombination that made the jump look decisive was `32.702 + 5.828 = 38.530`.
It needed a 5.828 s sector 5. The best sector 5 anyone has driven **from any
state** is 6.147, and from *this* state the measurement puts it past 9. **The
jump is not 7 ms short of the author time. It is about five seconds short.**

## And the exit is not merely unsolved — it is geometrically impossible

The open problem this page posed was "find a launch that lands aligned with the
+z run of sector 5". It cannot be done, and the argument is geometric rather than
empirical.

**A ballistic flight changes horizontal heading by exactly zero.** Measured, not
assumed: across 1.8 s of free flight the heading reads 29.4° at every sample,
0.0° of drift. It has to — there are no horizontal forces on an airborne car. The
chassis can yaw in the air; **the direction the car is travelling cannot change.**
Only ground contact changes it, which is visible as a 29.1° → 41.6° step at a
clip.

And the two headings this map demands are **52.8° apart**:

* the flight must fly the bearing from the ramp to checkpoint 5 — `atan2(144, 226)`
  = **32.5°**, and the jump flies 29.4°. No freedom: reaching CP5 pins it.
* the landing surface runs essentially along +z — the world record's own heading
  through the landing area is **82.2° → 98.1°**.

The flight supplies 0° of the 52.8°, and the landing keeps only `cos(mismatch)` of
the speed. That is the whole story of the 74.5 → 22.6 m/s deceleration that had
been measured but not explained.

> **This is a real result, not a failure.** A geometric invariant closes a route
> that two search methods and two node-hours could not close by exhaustion, and it
> generalises: *before searching for a landing that arrives aligned, check whether
> the flight is allowed to rotate the velocity at all.*

## What is actually true about this map

**Every tape this project has produced here is human rank 2, to the millisecond,
up to checkpoint 4** — 70 % of the lap. Nobody had seen that, because the
instrument that would have shown it reads the ghost *header* and reports the
template's splits.

So the map's real shape is: sectors 0–3 hold about **0.326 s** against the best
human driving that exists, the sector-4 jump cannot be banked, and the map needs
roughly a second of TAS headroom above every human — which is what the current
39.478 is chipping at.

**The keyboard result stands and is the cleanest thing here.** 117 key presses,
three steering values, **0.227 s faster than the analog human world record**, on
the field's own route with no jump.

## The sibling maps: five humans inside their own author times, and none of them transfer

This map is one of a series of seven by the same author. **151734 "Spaghetti
Nights 3" shares 3 475 of 146612's 3 541 block records — 98.1 %** — and a human
(mernama) holds 39.555 on it against that map's author time of 39.840. Five of
the six siblings have a human inside their author time, and **12 of 12 sibling
ghosts tested re-simulate their millisecond exactly.**

One coincidence is worth staring at: **133353's human world record is 38.532 and
146612's author time is 38.530** — the same number to 2 ms, on maps sharing 76 %
of their block records.

**But the tapes do not transfer.** All 21 sibling ghosts DNF on 146612, and the
sharp test is that five of them DNF **before checkpoint 1**, on 146612's own
`seg1`, with two native ghosts exact in the same batch — 1.26 s *inside* a region
that an occupied-cell diff said was identical.

> **An occupied-cell diff is a statement about geometry, not about drivability.**
> Two maps can be byte-identical in every block a car touches for 8.6 s and still
> put an open-loop tape somewhere else inside 7 s, because what diverges first is
> not geometry — it is spawn pose, or one decoration's collision, or a contact
> resolving a tick differently.
>
> **A similarity percentage is never reported alone.** It goes next to a transfer
> test — the sibling ghost against the target's own `seg1`, with a native ghost as
> an in-batch control — and the verdict is *transfers* / *does not transfer*, not
> a number.

That is the same conclusion [YOU LOVE WATER](../284238-you-love-water) reached
and the same one [Pure Wet Icy Wood](../210218-fall-2024-25-pure-wet-icy-wood)
reached independently: **an answer key tells you what to optimise, not what to
copy.**

## Method findings, all reusable

**A ladder makes a plateau searchable.** Same map, same seed, same search:
**0 finishers in 207 000 evaluations** with only the finish as an objective;
**13 of 22 stations climbed** once each station became its own objective.

**But a gate ladder cannot measure the progress of a DNF lineage.** A rung
reports the untouched time when it is silent. For a tape that *finishes*, a
silent rung is obviously a silence. For a tape that **does not finish**, a silent
rung and a dead tape are the same output — `DNF cps=4`. The tell here was a
non-monotone pattern: the best jump tape is `DNF cps=4` at stations 9, 10 and 11
and then fires 12 and 13, **and a dead tape cannot come back**. Every conclusion
above rests only on rungs that fired.

**A lookahead beam beats a greedy crawl, measurably.** On the identical seed and
ladder, a beam-of-3 with 3-station lookahead is **0.725 s better at station 4 and
1.545 s better at station 12** than the greedy crawl — the largest win for any
method change on this map. The greedy crawl's failure mode is documented below;
this is the fix, and it is worth what it claims. It just could not save a route
that was geometrically dead.

**A greedy per-station crawl locks in its own accidents.** Delta to the world
record, per station:

| st02 | st03 | **st04** | st06 | st08 | st10 | st12 | st14 |
|---|---|---|---|---|---|---|---|
| −0.501 | −0.231 | **+1.232** | +1.416 | +1.728 | +1.891 | +2.161 | +2.601 |

**The entire run is decided at one station.** st03 → st04 is 1.813 s for 28 m — a
wall contact. Every station after it inherits a dead run. Nothing in the crawl
notices: each station reported an improvement over its own seed, every result
validated, no error.

**Optimise arrival PAST a checkpoint, never at it.** "Fastest to CP5" bought a
state that could not use its own speed.

**A relocated Goal gate is a PLANE, and its axis is the `dir` byte** — 0/2 fire
on a z-plane, 1/3 on an x-plane. That is the finding that unblocked this map's
sector 5, and it explains the fleet's standing puzzle about silent probe rungs.
It also turned out this map has **four spare Goal gates nobody had looked at**:
the un-baked listing shows nine waypoint blocks where the naive read showed five,
four of them already tagged Goal. Relocating one of those is a three-byte
overwrite — no model swap, no promotion, no hole in the track.

**A wide rung is a decoy generator.** A 4-cell curtain produced a march winner
0.316 s ahead of the best known tape. It was the car **off the right-hand side of
the road, airborne**. Which is also how this page's earlier retraction happened:

> A published claim that the jump was still 0.639 s ahead 26 m past checkpoint 5
> was **wrong — the car was 2.3 m past the road edge and 2.2 m in the air.** A
> relocated gate is a plane, so it fired anyway. Make the rung as narrow as the
> road, and decode every march winner's own trajectory before believing its
> number.

**And on the fleet's reward-shaping notice:** shaping was genuinely live here (the
incumbent does not finish) and the finish rate was still 0 % over 207 000
evaluations. **"Shaping is live" is necessary, not sufficient.**

## Checks

* Field reproduction: **176 of 181 exact.** Five fail, all `DNF cps=1`, **none
  returning a different millisecond** — they re-download byte-identical, and two
  contain mid-run respawns, which an input-replay oracle cannot follow.
* Map sha256 `c6cca762e167eba6e969c07f306798c29c88d0da397b4744d4042c51b21526db`,
  Nadeo-served, 3 824 673 bytes.
* Six segment maps all reproduce their reference ghost exactly.
* **A parser warning worth carrying:** the block-census tool *panicked* on this
  map (`unhandled inline node class 0x40000000`) and on 210218. An empty census
  from a panicking parser is indistinguishable from "this map is built from
  items" — which is exactly how a wrong conclusion gets published. With the fix,
  this map parses to 279 free + 2 601 placed blocks + 661 items.

## Notes

* [`JUMP_CLOSED.md`](notes/JUMP_CLOSED.md) — the station-by-station measurement that closed the jump
* [`EXIT_UNSOLVABLE_ballistic_heading.md`](notes/EXIT_UNSOLVABLE_ballistic_heading.md) — the geometric invariant
* [`RESULT_sector_audit.md`](notes/RESULT_sector_audit.md) — the gate-plane finding and the full sector audit
* [`SIBLING_ANSWER_KEYS.md`](notes/SIBLING_ANSWER_KEYS.md) · [`SIBLING_TRANSFER_TEST_CORRECTION.md`](notes/SIBLING_TRANSFER_TEST_CORRECTION.md) — the sibling sweep and the correction that replaced a similarity number with a transfer test
* [`RESULT.md`](notes/RESULT.md) — the original write-up
* [`CORRECTION_st01_offroad.md`](notes/CORRECTION_st01_offroad.md) — the off-road retraction, in its author's own words
* [`GREEDY_CRAWL_NOTE.md`](notes/GREEDY_CRAWL_NOTE.md) · [`GATE_PLANE_ORIENTATION.md`](notes/GATE_PLANE_ORIENTATION.md) · [`SECTOR_AUDIT.md`](notes/SECTOR_AUDIT.md) · [`EXIT_DIAGNOSIS.md`](notes/EXIT_DIAGNOSIS.md) · [`SIBLING_151734_ANALYSIS.md`](notes/SIBLING_151734_ANALYSIS.md)
