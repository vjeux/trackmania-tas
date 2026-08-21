# Spaghetti Nights 2

**This map has four finish gates in a diagonal staircase and only 16 of 181
runs reach the first one; getting to that gate instead of the next one down is
about a second, and it is free to anyone driving in the 43–45 second range.**

The author time has not fallen. The fastest lap here is **1.255 under the
human world record** and still **0.438** short of the author medal.

> ### No video yet — and the reason is *not* the map
>
> This page has no clip. An earlier version of it said "the game will not open
> this map", as though that were a property of the map. **That claim has been
> withdrawn**, and so has the follow-up theory that the map's dependencies were
> to blame. Both were refuted by measurement rather than argument.
>
> A chunk census of the decompressed map body puts 146612 against maps that do
> film:
>
> | | **146612** | 279218 | 133353 | 126859 | 210218 |
> |---|---|---|---|---|---|
> | body | 4.12 MB | 1.43 MB | 2.10 MB | 3.22 MB | **11.92 MB** |
> | GBX nodes | 169 | 84 | 86 | 79 | **254** |
> | embedded objects | 384 KB | 24 B | 5.8 KB | 258 KB | **7.24 MB** |
> | items | 112 KB | 215 KB | 76 KB | **1.23 MB** | 727 KB |
> | header deps | 14 | 8 | 10 | **20** | 17 |
> | films? | **no** | yes | ? | yes | **yes** |
>
> **Everything that only the client processes — embedded objects, skins, item
> count, sheer size — is larger on maps that film.** 210218 carries nineteen
> times the embedded object data, 1.5× the nodes and 2.9× the body, and it
> films. 126859 carries more dependencies and eleven times the items, and it
> films. 228811 has two car-model skins exactly as this map does, and it films.
>
> The container itself is sound on six independent readers — the game engine via
> `/validatepath`, the block graph (2,881 records), the waypoint list, six
> derived segment maps, a live-memory trajectory reader and the ghost decoder —
> and Nadeo's own binary simulated about 1.29 million laps off this file today.
> md5 `16e7220f2128587c0d0018626feacb0f`, sha256
> `c6cca762e167eba6e969c07f306798c29c88d0da397b4744d4042c51b21526db`,
> 3,824,673 bytes.
>
> What is left is **that machine, that copy, or that attempt** — not this map.
>
> **So the honest status: the ghost side is finished and the video side is not.**
> `sn2_LAP_38968_watchable_v1` is regenerated, gate-passing and watchable in
> principle (md5 `6c459ceb546eb8c794084690e4c31cb0`, spawn exact, residual
> 0.000403 m against a same-batch control of 0.000407 m). Nobody has opened this
> map in a client and filmed it. **Until somebody does, the 38.968 is a result we
> hold, not a result we ship** — and this page is not promising a clip.

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS** | **38.968** | +0.438 | **−1.255** | analog |
| TAS, previous | 38.975 | +0.445 | −1.248 | analog |
| TAS, earlier still | 39.183 | +0.653 | −1.040 | analog, 189 steering values |
| **TAS, keyboard only** | **39.706** | +1.176 | **−0.517** | 3 values, 101 presses |
| Author time | 38.530 | — | −1.693 | — |
| Human WR — jujumasterr | 40.223 | +1.693 | — | pad, 1157 events |
| Human rank 2 | 40.226 | +1.696 | +0.003 | keyboard, 114 events |

**Read that table correctly: the author time is not beaten.** 38.968 against an
author medal of 38.530 is still 0.438 short. What this map has is a lap 1.255
under the fastest human — which is worth watching, and is not a record.

TMX map [146612](https://trackmania.exchange/maps/146612) · authors
**AmpelJoe10** and **Wakawukwuk** · 181 recorded runs.

**181 records does not mean the field is settled here.** Only five runs are
within a second of the world record, and there is an 0.849 cliff between rank 2
and rank 3. This is two players who duelled and 179 who did not, which is why a
whole second was still lying on the floor.

## The finish gate — the free second

The map has four finish gates in a diagonal staircase, and the last straight
runs back toward them, so **G1 is the first one you can reach**:

| gate | runs that take it | mean last sector | best last sector | mean speed at CP5 | best final time |
|---|---|---|---|---|---|
| **G1** | **16** | 7.331 | **6.396** | 72.2 m/s | **40.223** |
| G2 | 8 | 7.860 | 7.121 | 65.9 | 41.561 |
| G3 | 101 | 8.886 | 7.421 | 63.2 | 43.616 |
| G4 | 56 | 8.502 | 7.558 | 63.3 | 43.054 |

Eleven of the sixteen runs that reach G1 are in the top 15. **It is not simply a
consequence of speed**: rank 16 arrives at the last checkpoint at 75.2 m/s,
matching the world record, still takes G3, and pays about a second for it. If
you are anywhere in the 43–45 range, this is the cheapest second on the map.

## Sector 4: take the loop, not the ramp

After the last-but-one checkpoint the track branches.

- **The loop line** — the nine fastest sector 4s, including the entire top 7 —
  takes the branch, swings out wide, comes back and climbs a ramp to the last
  checkpoint. Best: 5.674.
- **The ramp line** — 144 of the 181 runs stay straight and hit the up-slope,
  which launches them about 190 m straight down the corridor. They land at
  70 m/s and then crawl round a slow turn at 51–60 m/s. Best: 6.113.

**In the top 40, every single ramp run is slower than every single loop run.**
The field tried the ramp and concluded it was a trap.

It *is* a trap, but not for the reason it looks. Taken at an angle — full lock
across the corridor for the last 80 m of run-up, leaving the lip about 21° across
instead of square — the same ramp carries the car 90 m sideways in flight, onto a
raised platform nobody has ever landed on, 50 m from the checkpoint and already
pointing at it. That reaches the last checkpoint **1.128 earlier than the best
human**, and it holds 0.639 of the lead 26 m past it.

Then it gives every millisecond back inside eighty metres, and the reason it
cannot be fixed is geometric rather than a matter of practice: **a ballistic
flight changes your direction of travel by exactly zero.** Measured across 1.8 s
of free flight, the heading reads the same to 0.0°. The chassis can yaw in the
air; where the car is *going* cannot change until it touches something. The
flight must fly the bearing to the checkpoint, 32.5°, and the surface it lands on
runs 52.8° away from that — so the landing keeps only the cosine of the
mismatch, and the car goes from 74.5 m/s to 22.6 m/s against the outside wall.
**The angled jump is real, and there is no exit from it. Do not chase it.**

The same argument kills the other obvious cut. Sector 3 is a hairpin that
travels 3.36 m of track for every metre of progress — the most inviting shape on
the map — and a flight across it lands travelling roughly backwards along the
road.

## Sector 5: arrive planted, not early

The last sector is a straight into a left sweep. It is a steering sector, not an
acceleration sector, and **arriving at the last checkpoint earliest is not
arriving best**:

| lap | last checkpoint | sector 4 | last sector | finish |
|---|---|---|---|---|
| human WR | 33.584 | 5.750 | 6.639 | 40.223 |
| `BEST_39961_v3` | 33.814 | 5.658 | 6.147 | 39.961 |
| `TAS_39748` | 33.756 | 5.600 | **5.992** | 39.748 |
| **`TAS_39460`** | **33.325** | **5.491** | 6.135 | **39.460** |
| an airborne entry | 33.143 | — | 7.073 | did not hold up |

Going from 33.756 to 33.325 costs 0.143 of the last sector to buy 0.431 of
sector 4 — a good trade. Going the further 0.182 to 33.143 costs the best part
of a second, because the car crosses the checkpoint airborne with no steering
authority and 12° of yaw across a road one cell wide. **The target is the
fastest arrival that is still planted and pointing down the road**, which on
this evidence is around 33.3.

## Where the lap's time comes from

Through the first four checkpoints the fast laps here are the best human driving
that exists, split for split — 7.311 / 15.718 / 19.980 / 27.834. Everything the
machine adds is in the last twelve seconds:

| | last checkpoint | sector 4 | last sector | finish |
|---|---|---|---|---|
| human WR | 33.584 | 5.750 | 6.639 | 40.223 |
| the 39.430 lap | 33.325 | **5.491** | 6.105 | 39.430 |

Field best for sector 4 is 5.674 and for the last sector 6.396. The best sector
4 and the best last sector measured here are not yet on the same tape.

For the rest of the lap the honest number is small: the best jointly achievable
human driving through the first four sectors is 0.326 better than what the
current laps use, and sectors 1 and 2 are anti-correlated — the "fast sector 2"
variant loses on the pair, so do not chase it in isolation.

## This map is keyboard territory

Six of the top 15 humans are pure three-value keyboard runs, including rank 2 at
0.003 off the world record, and the world record itself never lifts the
throttle. The shortest press anyone uses is 10 ms, with a median hold of
110–170 ms.

A keyboard lap here is **39.706 on 101 key presses** — half a second under the
world record, on the field's own route, with no jump. That is the deliverable on
this map, and it is the thing to copy.

## How forgiving it is

On the keyboard line, 117 of 144 steering presses (81 %) still finish the lap if
you mistime them by a 10 ms tick, against 103 of 140 (74 %) for the human
keyboard record measured the same way. **The tape is about as forgiving as a
human's own run** — a 40 s tech map replayed frozen is brittle whoever wrote it,
and a driver corrects on the next frame.

The inputs with real room on that tape are at 21.58 s (10 ticks), 31.84 s
(6 ticks), 33.38 s (8 ticks), 37.84 s (9 ticks), and everything after 38.0 s has
13 ticks — the run-in to the line is free.

**What will take real practice.** There is no single missing technique left in
the remaining 0.653: it is tolerance across the whole lap. The sector-by-sector
guide for this map is not written yet.

## Files

| file | what |
|---|---|
| `replays/TAS_39183.Ghost.Gbx` | **the fastest lap** |
| `replays/KEYBOARD_39706.Ghost.Gbx` | **keyboard only, 101 presses** — the one worth studying |
| `replays/TAS_39460.Ghost.Gbx` | the best sector 4, 5.491, arriving planted at the last checkpoint |
| `replays/TAS_39748.Ghost.Gbx` | carries the best last sector, 5.992 |
| `replays/TAS_39430.Ghost.Gbx` | the lap whose splits are quoted above |
| `replays/BEST_39961_v3.Ghost.Gbx` | earlier analog lap |
| `replays/KEYBOARD_39996_v3.Ghost.Gbx` | earlier keyboard lap, 119 presses |
| `replays/JUMP_cp5_32702_v1.Ghost.Gbx` | the angled ramp jump — reaches the last checkpoint 1.128 early and never finishes |
| `inputs/BEST_39961_v3.tick.csv` · `inputs/KEYBOARD_39996_v3.tick.csv` | those two laps as readable input scripts |
