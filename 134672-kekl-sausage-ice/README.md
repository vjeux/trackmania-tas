# KEKL- SAUSAGE ICE — a 2620 m ice ribbon, and the author time still stands

**KEKL- SAUSAGE ICE** — TAS **67.319** (+8.632) | AT 58.687 | WR 68.442 by Robbalobb

TMX 134672 · uid `agH9XtjTZd8iZbuGp_KhC16jMO7` · author `Travis.TM` · 15 records
Nothing here has been or will be submitted to a Nadeo leaderboard.

> **On the world record.** Roevhaal's 63.546 was set on a 2022 game build and
> does not re-simulate on a current one — feed it to Nadeo's own validator today
> and the car is exact for 8.9 s and lost by 9.6 s. So the reference used
> throughout is **68.442 (Robbalobb, rank 2)**, the fastest run on the board that
> today's game reproduces to the millisecond.

---

## The thing worth reading first: the first 12 seconds

**A search that had never seen a human drive this map beat the entire human
field to CP1 by 0.734, and beat the best re-simulable human by 1.431.**

| to CP1 | |
|---|---|
| **this TAS** | **12.475** |
| best CP1 split in all 15 records | 13.209 |
| Robbalobb (rank 2) | 13.906 |

No human input bit is in that tape. It is built forward from the start line, one
gate at a time, on a **three-value steering alphabet** — full left, straight,
full right, exactly what a keyboard gives you — and the constant-throttle,
no-steering tape it starts from carries no information about this map at all.

**And it converged on the keyboard players' own technique.** Across the 15
records, more time at full lock goes with a *faster* lap (correlation −0.77
among the keyboard runs), the top three records are all pure keyboard, and the
fastest pad player is seven seconds back. The cold search, told nothing of this,
settled at **76 % of ticks at full lock** — between Roevhaal's 65 % and
Robbalobb's 81 %. The same search run with a 7-value analog alphabet lands at
36 %, which is where the field's pad players are, and is 1.535 s slower at the
ramp.

**What that means if you drive this map**: full lock is not a mistake here. This
is not a surface where you feather the wheel to keep grip — the fast line is a
committed continuous drift and the steering is there to rotate the car, not to
hold it. Both the record and this TAS spend the lap sideways at **~22.8 m/s of
pure lateral speed**.

## The honest headline: the author time is 8.632 away, and it is not a driving problem

The AT is **58.687**. This TAS is **67.319** — a second inside the best human
that re-simulates, and still nowhere near it. That is not for want of searching,
and the reason is specific enough to state as a number.

**Prefix gain on this map converts at about 3.8 %.**

This lap reaches the last checkpoint **2.239 s earlier** than the previous best
TAS, and finishes **0.085** earlier. The other 2.154 s is eaten in the closing
sector, and it is eaten in one place:

| | CP1 | CP2 | CP3 | CP4 | finish |
|---|---|---|---|---|---|
| **this TAS** | **12.475** | **31.492** | **45.396** | **61.703** | **67.319** |
| previous best TAS | 13.906 | 33.106 | 45.437 | 63.942 | 67.404 |
| Robbalobb 68.442 | 13.906 | 33.106 | 45.437 | 63.812 | 68.442 |

The last hop is the 8 m drop off the raised section into the finish gate. The
previous TAS covers it in **1.484**. This one takes **3.327** — while being
faster than both other runs on *every single hop into* CP4. Arrive at that drop
faster and you arrive at it wrong.

That is why the author time is out of reach from this direction: at 3.8 %
conversion, closing 8.632 s through better driving before the drop would need
something like 227 s of upstream gain.

### Why arriving faster arrives wrong: the lap hits the wall four times

Reading the trajectory back rather than the clock explains it. Counting
one-sample speed losses above 15 km/h — a collision, in other words:

| run | wall hits |
|---|---|
| Robbalobb 68.442 | **0** |
| the previous 67.404 TAS | **0** |
| ranks 1, 3, 4 | 1 · 1 · 2 |
| **this TAS** | **4** |
| the back half of the field | 6–8 |

The count orders the field almost perfectly — and the fourth hit, at **62.25 s**,
is *inside* the closing descent: **122.6 → 86.3 km/h in a single sample.**
**That crash is the 1.843 s.**

So the 3.8 % exchange rate is the symptom and clipping is the cause. At the
descent entry the clean tape carries **22.2 m/s of lateral speed**; ours carries
**0.3** — pointed straight, 15 km/h faster, into the wall. The beam bought its
speed with lines that clip.

**And the clean basin is not reachable from here.** A crash costs time in one
place, so it can be constrained in the ladder without a trajectory: re-score
finishers on a rung before the descent and rank legal-hop first, time second.
Positive control — it admits the clean tape at its true time and demotes both
others, 54 of 175 legal once mutating. Then, matched to the unconstrained arm in
every parameter and RNG seed: **0 legal out of 5342 finishers from our own
state**, against **931 per round** in the clean basin.

**The next lever, for anyone taking it further:** score the *state* at the
pre-descent rung — lateral speed, not arrival time — and chain backward. If it
joins, the ceiling is about **65.7**, which is still 7.0 over the author time.

**And there is no secret route.** Of the map's 117 drivable surface cells, 99
have been driven by at least one of the 15 records. The 18 that have not are
inside-corners of curves, one cell past the finish, and a 128 m dead-end spur
that stops 64 m short of — and 8 m below — the raised section it points at.

**Nor is there a splice that gets there.** Take this TAS's sector 1 and sector 4
(the two the cold search wins outright), the best sector 2 and sector 3 anyone
in the field has driven, and the best closing sector ever recorded here:

```
12.475 + 17.651 + 11.309 + 16.307 + 3.462  =  61.204
```

**Still 2.517 over the author time**, and nobody has ever driven that lap.

## How chaotic this map actually is

The map's reputation is that a 1/127 steering error blows the run up in under a
second. That is the right instinct and the wrong shape, and it is now measured:
41 tapes, each differing from Robbalobb's run by **one steering unit on one
10 ms tick**, scored at 19 gates around the lap.

* A one-unit error takes a median of **6.1 s** to move the lap by more than
  0.100, and a median of **8.1 s** to kill the run outright.
* **Sixteen of those perturbations — from six different places in the lap, at
  four different sizes, in both directions — all die at the same gate.** The
  time to death is simply "when you reach that corner", minus where you made the
  mistake. One steering unit dies exactly where sixty-four do.

So the map is not a uniform amplifier. It is a **sequence of filters** — the
ramp-and-chicane at ~9.6 s, the turbo and CP1 complex at ~13.2–13.9 s, and more
at ~19.8–28.1 s. Between them a small error rides along doing very little. At
one, whatever you have accumulated is cashed in at once.

**For a driver that is the practical bit**: a mistake on the fast sections is
survivable far longer than the map's reputation suggests, and the corners are
not "hard" so much as *unforgiving of everything that happened before them*.

## The ice is the road

The 41 custom `FlinkIceBlocks` on this map are not a skin over the track. Move
them off the grid — leaving every support pillar in place — and the run ends
4 seconds in: Robbalobb, rank 10, a constant-throttle tape and our own best cold
lap all stop dead at the same cell inside the big curve, along with 108 000
search candidates. Those blocks *are* the driving surface.

## The tape

| | |
|---|---|
| steering values | **3** (`−127 / 0 / +127`) — pure keyboard |
| steering changes | 129 over the lap |
| throttle held | 91.0 % of ticks |
| brake | 3.8 % |
| top speed | 272.0 km/h at 20.70 s (the record's peak is 243.5) |
| mean speed | 139.5 km/h |
| mean lateral speed | 22.73 m/s — sideways essentially all lap |

Robbalobb's record, for comparison: 3 steering values, 101 changes, throttle
82.1 %, brake 5.9 %, lateral 22.81 m/s. **This is not an exotic input program.**
It is 28 more steering inputs than a human already makes, on the same three
values, with more throttle held.

## Verification

Every number above is the map's own finish gate through Nadeo's dedicated-server
validator, on the **unmodified** `.Map.Gbx` — md5 `e73cb7b4e201edd176be97566adffb4b`,
and byte-for-byte identical to the copy Nadeo's own CDN serves today.

* The lap re-simulates to **67319** on three cold runs, one tape per invocation,
  against two separately obtained copies of the map.
* Known-answer controls in the same session: 68442, 94940 and the previous best
  67404 all exact; the 2022 world record DNFs, as it does for everyone.
* 5 of the 15 records were set on current game builds and **all 5 re-simulate to
  the millisecond**; the 10 that do not are all from the one 2022 build.

### About the replay file

The searched tape and the watchable ghost are two different things: a search
output carries the *telemetry* of whatever ghost's container it was built in, so
it can time correctly and play back as somebody else's run. The 67.319 ghost has
been **regenerated** — its position, orientation and speed are read out of the
engine sample by sample, and its declared time and checkpoint list are its own
(12.475 / 31.492 / 45.396 / 61.703 / 67.319, not the carrier's).

The fidelity of that regeneration is measured, not assumed: run the same
pipeline on Robbalobb's own ghost, whose true telemetry we already have, and it
reproduces that recording's own position to a **mean of 0.0002 m** and its speed
to 0.07 km/h.

**The tyre effects are now absent rather than wrong.** The old regeneration
rewrote 25 of the 116 bytes per sample and left the rest as the carrier's,
including the per-wheel contact and surface channels — which described
*Robbalobb's* flights, so ice spray fired at his moments and not ours. The
current file zeroes every per-run byte it does not write and **names the eleven
channels it cannot yet produce**: rpm, per-wheel ice and dirt, ground contact,
gear. Those quantities *are* in the engine's memory — fitted against a real
recording, exactly on gear and turbo and 92.6 % on rpm — so this is a missing
anchor that survives a change of map, not a limit of the data. Saying so
precisely is the point: the previous wording called it cosmetic, and it was
somebody else's run.

**The line and the speed in the replay are this run's. The tyre effects are
absent.**

### The impacts are real, and that is measurable

The car visibly bangs off the platform edges, which looks like a rendering
artefact or a debug collision left switched on. It is neither. Two independent
regenerations of this tape — separate engine processes, separate located
addresses, no shared state — produce the same four impacts at **10.400, 22.900,
31.500 and 62.250**, agreeing to 0 ms and 0.0 km/h. Robbalobb's own recording
has zero, as this page says, and rank 3 has one. The map is sha256
`1cc10011a9882145333afcfc4acf2b85e20548e0ec035ccfcfd7e85e9010b703`, identical to
the copy the validator uses. `tmtraj impacts --against` is the census.

## Files

| file | what |
|---|---|
| `replays/TAS_67404.Ghost.Gbx` | the previous best TAS, 67.404 |
| `replays/KEYBOARD_67625.Ghost.Gbx` | keyboard tape, 67.625 |

| `replays/TAS_67319.Ghost.Gbx` | **the 67.319 itself** — regenerated from engine state, no sample byte the donor's, span 0.000 → 67.319 |

Every time and split on this page comes from the validator, not from a file
kept here.
