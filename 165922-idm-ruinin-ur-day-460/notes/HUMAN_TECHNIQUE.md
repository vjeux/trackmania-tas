# 165922 `idm ruinin ur day #460` — how a HUMAN drives this under the author time (v3)

Agent vj4, 2026-08-19, node 64455. **Supersedes v1 and v2**, which both said a
one-tick error "does not even get down the chute". That was an instrument defect
of mine (worker directories reused across maps — see the fleet notice); the
corrected reading is in §3 and it makes the human story *better*, not worse.
Written against ACQUISITION §0.7.1: the author time is a driven validation lap,
so "not humanly executable" is never the answer.

**Classification: KNOWN-BUT-UNHELD.** The route is the one the board's only
human already drives. The author time needs two things they did not put
together in 930 attempts: a clean launch, and a landing that reaches the pad
instead of stopping 45 m short of it.

---

## 1. The map in one paragraph

You start on a platform 1.88 km up, drop through a narrow start chute, cross the
map's one checkpoint at ~1.7 s inside the start structure, ride a short booster
ramp (all of it inside x ∈ [409, 690]) and leave it at ~5 s doing about
180–200 m/s. Then there is **nothing** for 1.9 km: an unpowered ballistic glide
onto a pad of 132 finish gates on the ground, 88 m × 352 m, at x 2300–2380,
z 576–928. The map is a launch and a fall.

## 2. Three things a player can use

**The glide needs no input at all.** Force the steering to zero from a given
moment to the end of our fastest tape:

| steer zeroed from | finish |
|---|---|
| race 4.50 s | DNF |
| race 5.50 s | 15.276 (+0.052) |
| race 6.46 s | 15.231 (+0.007) |
| race 8.46 s onward | 15.225 (+0.001) |

Nine of the 15.2 seconds need no steering. That is exactly what the human does:
after ~6.4 s their tape is `steer 0, gas held` and never changes again. Same for
the throttle — holding it from 4.46 s to the finish costs nothing at all.

**The board's only human plays on a keyboard.** Across all 879 231 ticks of the
2.44-hour record, 94.2 % of steering values are exactly {0, −127, +127}; the
winning attempt is **102 input events**, gas held 100 % of the time, one 20 ms
brake tap: full lock right 2.7 s, full lock left 1.2 s, full right, then nothing.
Our own best tape is a keyboard tape from race 4.56 s onward, and the keyboard
constraint made it *faster*, not slower.

**Their problem was the ramp, and then the pad.** The record is one session of
930 attempts (929 respawns). The one that got through landed **45 m short of the
first gate row** and spent its last **3.77 s crawling** into a gate — 18.85 s for
that attempt. Our tape crosses the pad's near edge at the exact millisecond it
finishes. That crawl is the whole difference.

## 3. The first three seconds decide it — and the human's version is forgiving

Move one input change by a single tick (10 ms), earlier or later, and
re-simulate. On our fastest tape:

| window | shifts | still finish |
|---|---|---|
| race 0.00–2.96 s | 52 | **0** |
| race 2.96–3.96 s | 30 | 9 |
| race 3.96–4.96 s | 54 | 50 |
| after race 4.96 s | 1202 | **1202** |

Same shape on all four tapes we have. The shifted runs **get down the chute
fine — 52 of 52 — and then crash on the booster ramp**: only 2 of 52 are still
alive at x = 900, none at x = 1400. So the sensitive thing is the ramp entry and
the boost sequence, not the chute walls and not the aim (a tape deliberately
aimed 40–80 m deeper into the pad, still 0.26 s under the AT, is just as
sensitive).

**But this is a fact about a TAPE, not about a driver.** The same measurement
run on the human's own winning attempt: **17 of 42 boundary shifts survive —
40.5 %.** Their launch tolerates a 10 ms error; ours tolerates none. A tape is
open loop and cannot notice it is 30 cm off; a player closes the loop by eye. And
between those two extremes we measured a middle point: a coarser, keyboard-only
launch is 10 % tolerant, at a cost of about a second.

That is the real shape of this map: **the fast launch and the forgiving launch
are different programs, and the author's 15.643 is somewhere between them.**

## 4. What to practise

* The chute is a **feel** section. Do not try to memorise a millisecond-exact
  pattern — ours does not even transfer to itself.
* The ramp entry and the boosters are where the run is won or lost. Everything
  after ~5 s is ballistic and forgiving.
* Hold the gas. After the ramp, **hands off the steering** — any input in the
  glide is worth at most a thousandth of a second and can kill the run.
* Land **on** the pad, long rather than short. The only finisher on this board
  lost 3.77 s crawling the last 45 m.

## 5. Why 15.643 is believable

It is our 15.22 plus four tenths: the same route flown a little less perfectly,
landing a little deeper into the pad instead of clipping its near edge. No
undiscovered route, no respawn trick, no input a keyboard cannot make. The `LOL`
tag and the 2.44-hour "record" describe the difficulty, not the legitimacy of
the time.
