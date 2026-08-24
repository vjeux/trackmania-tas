# KEKL- SAUSAGE ICE — a 2620 m ice ribbon, and an author time set by a different car

**KEKL- SAUSAGE ICE** — TAS **67.200** (+8.513) | AT 58.687 | WR 68.442 by Robbalobb

https://github.com/user-attachments/assets/68a1ee6d-9117-4c5a-a030-0c7c658e7b84

The TAS in magenta, Robbalobb's 68.442 as the opponent, chase camera on our car.
Both cars are in one scene; he is behind for most of it and out of frame for
much of that, which is the divergence rather than a framing fault.

The panel is this run's own inputs, drawn from the 10 ms input chunk — what the
driver pressed, not the 50 ms echo in the telemetry samples. On a synthesised
tape those are two different runs' worth of steering, and the fast one is ours.
Its timing is measured rather than eyeballed: the two channels describe one run,
so they agree at exactly one shift, and on this file that shift is 0 ms.

TMX 134672 · uid `agH9XtjTZd8iZbuGp_KhC16jMO7` · author `Travis.TM` ·
**16 records** (board 2026-08-24 03:32–03:37 UTC). Every "fifteen records"
below is the field as it stood when it was measured; **the sixteenth has not
been read** — UNKNOWN which build it was set on and whether it replays, which
`ghost inspect` plus a plain-oracle run would settle. It is not a new record:
the board's best is still Roevhaal's 63.546.
Nothing here has been or will be submitted to a Nadeo leaderboard.

> **On the world record — and on the author time.** Roevhaal's 63.546 was set on
> the 2022 build `113150`, and it does not re-simulate today. That has been read
> here as the map amplifying a recording quantum. It is measured now, and it is
> not that. All fifteen records have now been measured against their own
> recorded lines, and the split is perfect: **the ten that do not replay leave
> their line in the same corner within a third of a second of each other — race
> 4.04-4.39 for nine of the ten — and all ten leave it on the same side. The
> five that do replay never leave it at all** (0.0002 m over a whole lap).
> Today's car runs wide there; the old car turned more.
>
> The author time is `authorScore: 58687` inside a map file Nadeo stamped on
> **2022-07-31**, so it belongs to that build too. **Beating it means beating a
> time set by a car that corners better than the one we search with.** The 4.9 s
> nobody could explain between the two populations has a mechanism, and it is in
> the car, not in the map. Section *Why the 2022 half of the field does not
> replay* has the measurement and its controls.
>
> **Confirmed against the old engine itself, 2026-08-23.** Nadeo still serves
> dated dedicated-server archives, and on the **2022-06-21** server Roevhaal's
> tape validates at **63.546** with all five of its splits to the millisecond —
> as do all ten of this map's build-113150 records, and none of the five modern
> ones. Run the same 2022 tape on the 2022-01-01 or 2022-03-19 server and it
> fails, so the instrument distinguishes builds rather than passing everything.
> His run is real driving, not a recording artefact. `../OLDBUILD.md`.
>
> **And it still does not explain the author time.** Under that build's own
> physics the field's best-sector sum is **63.263**, and a 200-minute search
> from Roevhaal's own tape reached **63.074** — both more than 4 s above
> **58.687**. The build explains why the record does not replay; it does not
> explain the medal.
>
> The reference used throughout for anything that must re-simulate on a current
> build is still **68.442 (Robbalobb, rank 2)**.

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

## The honest headline: the author time is 8.513 away, and it is not a driving problem

The AT is **58.687**. This TAS is **67.200** — a second inside the best human
that re-simulates, and still nowhere near it. That is not for want of searching,
and the reason is specific enough to state as a number.

**Prefix gain on this map converts at about 3.8 %.**

This lap reaches the last checkpoint **2.239 s earlier** than the previous best
TAS, and finishes **0.085** earlier. The other 2.154 s is eaten in the closing
sector, and it is eaten in one place:

| | CP1 | CP2 | CP3 | CP4 | finish |
|---|---|---|---|---|---|
| **this TAS** | **12.475** | **31.492** | **45.396** | **61.703** | **67.200** |
| the 67.319 it supersedes | **12.475** | **31.492** | **45.396** | **61.703** | 67.319 |
| the 67.404 before that | 13.906 | 33.106 | 45.437 | 63.942 | 67.404 |
| Robbalobb 68.442 | 13.906 | 33.106 | 45.437 | 63.812 | 68.442 |

The last hop is the 8 m drop off the raised section into the finish gate. The
previous TAS covers it in **1.484**. This one takes **3.327** — while being
faster than both other runs on *every single hop into* CP4. Arrive at that drop
faster and you arrive at it wrong.

That is why the author time is out of reach from this direction: at 3.8 %
conversion, closing 8.513 s through better driving before the drop would need
something like 224 s of upstream gain.

**The 0.119 s from 67.319 to 67.200 is all in the closing sector** (5.616 →
5.497), with CP1-CP4 unchanged to the millisecond: 135 450 evaluations over the
last 9.5 s of the tape, 24 improvements confirmed by the plain oracle and 0
phantoms. The five endgame searches the 67.319 arm ran had settled; what moved
it was giving the failures a gradient with a verified `--seg 3` ladder.

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
pre-descent rung — and it is a bigger difference than "lateral speed". Measured
at matched DISTANCE rather than matched time, 75 m before the descent entry:

| at 2400 m along the route | km/h | lateral m/s | on the ground? | yaw |
|---|---|---|---|---|
| **this TAS** | 149.6 | **1.28** | **airborne** | +127.9° |
| Roevhaal 63.546 | 134.5 | 37.32 | yes | −97.8° |
| Robbalobb 68.442 | 125.2 | 29.14 | yes | −67.8° |
| rank 3 69.522 | 133.9 | −21.64 | yes | +164.4° |

We arrive **off the ground, straight, and 130–220° from every human's heading**,
and 75 m later our lateral speed is +34.67 where all three of them are at −28 to
−32 — sliding the other way. That is the state a chained search has to hit, and
"0.3 against 22.2 m/s of lateral speed" understated it.

**And there is no secret route — but the old argument for that was the wrong
test.** This page used to count *undriven cells*: 99 of the map's 117 drivable
surface cells have been driven by some record, and the 18 that have not are
inside-corners, one cell past the finish, and a 128 m dead-end spur that stops
64 m short of — and 8 m below — the raised section it points at.

**A shortcut does not need an undriven cell. It needs to SKIP driven ones.** The
right question is where the line comes back near itself after a long interval,
and over the whole 2615 m lap, at a 40 m bar, there is **exactly one** place:

| from | to | it would save | how close | verdict |
|---|---|---|---|---|
| 34.600 | 55.200 | **20.600** | 9.73 m | **skips CP3 — void** |
| 60.500 | 67.250 | 6.750 | 53.67 m | **skips CP4 — void** (only at a 90 m bar) |

Both folds of the sausage cross a checkpoint, and nothing else on the lap comes
within 90 m of itself except the trivial neighbours of the current point. The
route is forced, and its length with it: every clean run in the field travels
**2615–2623 m** (the four that travel 2650–3850 m are the ones that spun).

**What the route is worth, measured at 10 m instead of at five checkpoints.**
The bound this page used to quote — 63.263 — is a sum of best SECTORS, five
numbers on a 67 s lap, so it can only see a swap between whole sectors. Project
every run onto one centreline by monotone alignment, so a run that spins is
charged for the detour instead of being credited with speed somewhere it was
not, and price each 10 m at the shortest time anyone has ever taken to cross it:

```
RAW ENVELOPE      = 50.978      (human recordings only, without this TAS: 52.589)
FEASIBLE ENVELOPE = 51.567      (after a forward-backward pass under this field's own accel limits)
```

against an author time of 58.687. The control that licenses those numbers: run
the identical pipeline on ONE run's own data and it must return that run's own
lap — **16 of 16 do**, to 0.006–0.36 s. It is an optimistic bound and cannot be
driven, because it stitches together cars in states that cannot be reached from
one another; that is exactly what makes it useful as a negative. **The author
time does not require a speed nobody has reached anywhere on this route.**

**Nor is there a splice that gets there.** Take this TAS's sector 1 and sector 4
(the two the cold search wins outright), the best sector 2 and sector 3 anyone
in the field has driven, and the best closing sector ever recorded here:
(the two the cold search wins outright), the best sector 2 and sector 3 anyone
in the field has driven, and the best closing sector ever recorded here:

```
12.475 + 17.651 + 11.309 + 16.307 + 3.462  =  61.204
```

**Still 2.517 over the author time**, and nobody has ever driven that lap.

## The uncomfortable number: a human drove this map 3.773 s faster than the TAS

Roevhaal's 63.546 is real recorded telemetry of a real drive over the same
2621 m. Whatever the build difference costs us, it is not an excuse for this:

| | S1 | S2 | S3 | S4 | S5 | lap |
|---|---|---|---|---|---|---|
| this TAS | **12.475** | 19.017 | 13.904 | **16.307** | 5.497 | 67.200 |
| Roevhaal | 13.492 | **17.651** | **11.309** | 17.130 | **3.964** | 63.546 |

We are the best in the field in two sectors and worse than *every* top human in
sector 3. At a 25 m grain every loss is the same event — the lap arrives too
fast and craters. Through 1550–1700 m our speed goes 120 → **72** → 101 → **77**
km/h where Roevhaal holds 181 → 177 → 155 → 132; our peak is 272.0 km/h and our
troughs are 59–77, where his peak is ~254 and his troughs 109–135.

**And it is not reachable by searching near our own line.** Two windowed
searches aimed at exactly those places, 2.5 hours and 40 workers each, scored at
the next real checkpoint through a verified segment map, bought **0.222** and
**0.051** against deficits of 2.595 and 1.366. Two further 100-minute searches
that took the sector-3 winners and rewrote the whole tail produced **zero
finishers** between them — so a faster line into CP3 does not merely convert
badly, it does not reconnect at all inside a budget that comfortably finds 24
improvements when the tail is searched from the incumbent itself. Reseeding on
new basins is what broke 208024 open; here it does not.

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

* The current lap re-simulates to **67.200** from two separate processes against
  two separately obtained copies of the map, and the 67.319 before it to 67.319
  on three cold runs, one tape per invocation.
* Known-answer controls in the same batches: 68.442, 94.940 and the previous
  best 67.404 all exact; the 2022 world record DNFs, as it does for everyone.
* 5 of the 15 records were set on current game builds and **all 5 re-simulate to
  the millisecond**; the 10 that do not are all from the one 2022 build.

### About the replay file

The searched tape and the watchable ghost are two different things: a search
output carries the *telemetry* of whatever ghost's container it was built in, so
it can time correctly and play back as somebody else's run. Both published
ghosts have been **regenerated** — position, orientation and speed read out of
the engine sample by sample — and both declare their own time and their own
checkpoint list (12.475 / 31.492 / 45.396 / 61.703, then 67.200 or 67.319).

> **The split list was a real gap until this arm, and it is worth naming.**
> `ghost declare --time` rewrites the race time and the LAST checkpoint entry
> and leaves every intermediate one alone — so a regenerated file on a borrowed
> container declared its own finish beside **four of the donor's splits**, in
> one list, with nothing in the file to say which was which. The deleted `u02
> declare --splits` could write them and its replacement could not.
> `ghost declare --splits` is that capability back, with a read-back control and
> a refusal if the last split is not the declared time.
>
> The same chunk holds a word this toolchain called `nb_respawns`, and it is
> **not one**: all fifteen human records here read **5** while their tapes hold
> 0, 1 and 2 respawn events, and on 279218 the files whose tapes DO respawn read
> **0**. It is not the checkpoint count either (5 with 5 entries here, 1 with 2
> on 249521, 3 with 4 on the format crate's own fixture). It is now called
> `word4_unidentified`, so that nothing keys on a guess.

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

> **Update, 2026-08-22 — ice is no longer one of the eleven, and on this map
> that matters.** The carrier-bytes arm has located it: `Icing01`, per wheel, at
> `car + 88 + 44k + 28`, encoded `floor(v × 255)`. It reads **100.00 % exact on
> 1370 samples of this map's own rank-2 recording** — against a 79.0 % constant
> baseline — and **100.00 % on 462 samples of 267460**, with no refit between the
> two. So the count is **ten**, and *the ice spray on a 2620 m ice ribbon is a
> regeneration away rather than unavailable.*
>
> **Dirt is a different verdict and should not be lumped in with it: refuted,
> not unfound.** Pre-registered across all eight remaining slots of the wheel
> record and beaten by a constant (best worst-key lift **−7.35 points**). Byte
> 89, ground contact, has now been refused four times — most recently at
> `car+58`, where an affine fit scored 91–100 % on five keys because *an integer
> read as an f32 is a denormal*; scored as a raw byte on eight keys with no refit
> it is **0.00 %**.

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
| `replays/TAS_67200.Ghost.Gbx` | **the current best, 67.200** — regenerated from engine state, `tmtraj check` **PUBLISHABLE (0 fail, 0 warn)**, `ghost verify` V1–V10 pass, span 0.000 → 67.200. Its declared splits are **its own** (12.475 / 31.492 / 45.396 / 61.703 / 67.200), measured on segment maps that reproduce two independent runs' splits exactly, and written with the `ghost declare --splits` this arm added — before it, a regenerated file declared its own finish beside four of the container donor's checkpoint times. |
| `replays/TAS_67319.Ghost.Gbx` | the 67.319 it supersedes — regenerated from engine state, no sample byte the donor's, span 0.000 → 67.319 |
| `replays/TAS_67404.Ghost.Gbx` | the TAS before that, 67.404 |
| `replays/KEYBOARD_67625.Ghost.Gbx` | keyboard tape, 67.625 |
| `ARM-ksi2-RESULT.md` | the 2026-08-22 arm in full: the envelope, the shortcut census, the divergence measurement, and every control behind them |

Every time and split on this page comes from the validator, not from a file
kept here.

---

## Why the 2022 half of the field does not replay

*Arm `ksi2`, 2026-08-22. This replaces "the map amplifies a recording quantum"
with a measurement that has a direction.*

`fk trace` runs a tape through the real engine and reports the car's own state
per tick; comparing that against the trajectory the same file records — **with
the whole-tick lag scanned rather than assumed** — measures the divergence
directly. The lag is not optional: at 150 km/h one 10 ms tick is 0.42 m, so at
lag 0 a file that replays perfectly reads as 0.42 m of "drift" at every point of
its lap. Scan it and the same file reads 0.0002 m.

That is the instrument's floor, and it is set by a current-build recording:
**Robbalobb's 68.442 sits 0.0002 m from its own recorded line for the whole
68 s.** Two different fork points give bit-identical divergence curves, so the
resume is not what is being measured.

Against that floor — **all fifteen records on the board, measured**:

| ghost | replays today? | leaves its own recorded line at | lateral sign at 5.06 |
|---|---|---|---|
| **Roevhaal 63.546** | no | **4.040** | −0.866 |
| rank 5 73.922 | no | **4.090** | −0.856 |
| rank 12 87.676 | no | **4.090** | −1.087 |
| rank 3 69.522 | no | **4.190** | −0.890 |
| rank 6 74.859 | no | **4.190** | −0.832 |
| rank 7 76.689 | no | **4.190** | −0.685 |
| rank 9 79.967 | no | **4.240** | −0.570 |
| rank 4 70.543 | no | **4.290** | −0.525 |
| rank 15 103.785 | no | **4.390** | −0.505 |
| rank 8 76.919 | no | **4.690** | −0.164 |
| **Robbalobb 68.442** | **yes** | never | +0.000 |
| rank 10 80.534 | **yes** | never | +0.000 |
| rank 11 84.366 | **yes** | never | +0.000 |
| rank 13 94.940 | **yes** | never | +0.000 |
| rank 14 101.259 | **yes** | one sample at 85.890, 0.0002 m either side | +0.000 |

(5 cm bar. The onsets shift by a few hundredths with the fork tick — a resume is
not the same run — so read the *clustering*, not the third decimal.)

**Ten of ten and five of five: perfect separation, and nine of the ten land
inside 0.35 s of each other.** They land in one corner — cell (27, 14, 23),
`RoadBumpCurve1`, a **stock** block, at the onset of the lap's first big slide.
Not on custom ice: there is no `FlinkIceBlock` within four cells of it.

**It has a sign, and the sign is the same in all ten.** About 90 % of each
divergence is lateral, and in every one of them today's car ends up on the
**outside** of the corner — it rotates less than the old car did on the same
inputs, whether that recording was holding full lock (Roevhaal, rank 15: steer
+127) or no steering at all (ranks 5 and 8: steer 0). A chaotic map amplifying a
rounding seed would take a random side; ten of ten agreeing is 1 in 512, and
they agree on the place as well.

**It is a step, not a drift.** Roevhaal's discrepancy crosses 5 cm within
**0.05 s** of being born. A whole steering unit, at one tick, on this map at
that moment, does **not** move the car 5 cm off its own line in the next 3.6 s.
So this is not a small input-scale difference accumulating; it is a step in the
car's state at a single contact — which is what a bump-road curve taken at
138 km/h and 15 m/s of slip is made of.

**It cannot be repaired, and the control says that is about the tape.**
`fk resync` puts up one fork server, locates once, then tries thousands of
candidate corrections against the recorded line, scored on the *sync horizon* —
the race time at which the engine's run of the candidate first leaves the
recording. The positive control: take Robbalobb's tape, which tracks its own
recording for 68.390, break it by ten steering units at one tick (the horizon
collapses to 8.370), and repair it with the same machinery —
**8.370 → 20.390 → 35.760 → 55.410 in 4 000 evaluations, and still climbing.**
The subject, over six configurations and up to 45 108 evaluations, including the
brake and throttle channels because Roevhaal is at full lock throughout:
**5.060 → 8.670**, and both of the moves that helped were brake taps — the
compensation the measurement above predicts, since braking on ice rotates the
car. Two independent searches from different windows with different move sets
both stall at 8.35–8.67 out of 63.5.

### What it means for the author time

`authorScore: 58687` sits inside a map file Nadeo stamped **2022-07-31**; the ten
records that do not replay are from build `113150`, dated 2022-07-06. **The
author time and the whole non-replaying half of the field are the same build**,
and that build's car rotates more than ours in this map's defining manoeuvre.

Two things follow and they point in opposite directions, so both are said:

* **Against reaching it:** every "the route is worth X" number anchored on 2022
  driving — the 63.546 world record, the old field-best-sector sum of 63.263 —
  is a number about a car that corners better than ours.
* **For reaching it:** the route's own envelope, computed at 10 m from runs the
  current engine reproduces, is **50.978**, and the splice bound of 61.204 is
  assembled from parts that all re-simulate today. **58.687 is inside what the
  current car has already done, metre by metre.** What stands in the way is the
  search, which is still 3.773 s slower than a human.

**One experiment would make this a fact about the game rather than about this
map**, and it is cheap: one pre-2023 recording taken at high slip on a **stock**
bump curve, on any map. The sibling-map control that read "the 2022 build's
physics are ours" (134682, 41.5 s reproduced exactly) has a mean side speed of
**5.68 m/s**; this map's field slides at **18.13**. If the divergence reproduces
off 134672, it is a build-level change in car rotation, and every pre-2023
reference this project uses is affected.

**It was attempted here and it is UNMEASURED, not negative — the instrument
refused.** Map **134525** (same author, same week, 15 records) is the natural
experiment: six of its records DNF and nine replay, and the split is nearly by
date — every DNF is from 2023-05 to 2023-08 and every pass from 2023-08-29 on,
with one pair **fifty-four seconds apart** on either side of the line. On that
map the locate works on a passing ghost and says the right thing (**r01, a
2023-08-29 pass: 0.0003 m, never diverges past 5 cm**), which is the positive
control. On the failing ones it **refuses**: at fork ticks 120, 400, 600 and 750
`fk trace` reports "best candidate is not self-consistent enough … refusing to
guess", and at tick 900 the run has already left its line so the reading is
meaningless (175 m). A locate that will not qualify is not evidence about the
recording, and the honest column for 134525's failures is UNMEASURED.

That leaves the sibling test open and worth someone's hour: it needs either a
locate that qualifies earlier on that map, or a third map with the same
date-split and a friendlier locate.
