# untitled 02

**The author time on this map is not route-enforced. There are no checkpoints,
so the only thing you have to touch is the finish gate — and 29 % of this run
is spent airborne, riding the lattice's own launches rather than driving it.**

| run | time | vs author time |
|---|---|---|
| **TAS** | **8.898** | **−22.501** |
| TAS, earlier rung | 9.415 | −21.984 |
| TAS, the intended-ish line | 32.219 | +0.820 |
| Author time | 31.399 | — |
| Human record — Its_Cam. | 14.959 | −16.440 |

TMX map [276877](https://trackmania.exchange/mapshow/276877) · author
**DugonGOD** · **1 recorded run** — the first human time on this map was set on 2026-08-20.

**untitled 02** — TAS **9.415** (−21.984) | AT 31.399 | WR 14.959 by Its_Cam.

https://github.com/user-attachments/assets/fb996518-80a2-4f6a-b200-3c26f7a01ce3

The clip is the **9.415**, the earlier rung of the same lattice skip — that is
the tape that has been rebuilt on a clean carrier so far, trajectory 189 of 189
positions bit-identical to its own original. **Ground contact and wheel rotation
are still the carrier's**, because zeroing them would claim the car was airborne
all run with wheels that never turned, so the dirt and spark effects may be wrong
while the path is ours. Watching it, the middle of the run looks as though it
leaves the track entirely; it does not — see below.

The split screen against Its_Cam.'s record was filmed from the 8.898 and came
down with it. It returns when that tape is rebuilt.

**Somebody finally drove this map.** Its_Cam. set the first human time here on
20 August 2026 — **14.959** — and took the same shortcut we did, reaching the
same gate at 2.583 s. On a map with no checkpoints, that is the thing worth
noting: a human independently found the same way to cheat it.

## What is going on here

This is a gimmick map — 31 reactor and reset gates, 18 no-steering gates, boost
pads and slow-motion blocks — and it has **no checkpoints at all** and **two**
separate finish gates. Nothing forces you round the course. The author time of
31.399 is what it costs to drive the track as built; the finish gate can be
reached in **8.898** by not driving it.

**Measured, on the 9.415 sibling rung** (same line, same lattice; the 8.898
differs only in how hard it is thrown): 100.0 % of samples sit inside an occupied
block cell, the run never leaves the lattice's own volume, and it never exceeds
its own spawn altitude — but **29.0 % of it is airborne**, 2.78 s in total, with
the longest continuous stretch **1.51 s**. So it is not flying away from the map;
it is being thrown along it by the lattice's own launches, which is why the line
stays inside the cells while barely touching anything.

For contrast, the sibling map's [untitled 01](../276874-untitled-01) run is
airborne 19.3 % of the time with a longest stretch of 1.13 s. **This one flies
more.**

So read this page as a demonstration that the map's time is soft, not as a
driving lesson. **32.219 is the honest number** — that is the first finish
anybody has recorded here, following roughly the intended line, and it is still
0.820 over the author time.

## The line

Off the start platform, straight down into the dark descending section, which is
where all the speed comes from — **325 km/h** by the bottom. There is a hard
contact at **1.86** that the run simply absorbs. Launch at **4.76**, and the
reactor-up zone lifts the car about **21 m** while it decelerates; the cloud
layer is below the whole lattice, so from the chase camera the car reads as
flying above the clouds even though it is inside the map's own structure the
entire time. Thread the gate columns at x ≈ 752 / 688 / 624 around y 281–287,
come down onto the curved ramp, land at **8.5**, and roll into the Goal item at
(580, 284, 713) at **41 km/h**.

## The run, as inputs

**Throttle is held from the countdown to the line and never released — not once
in 8.898.** Everything else is steering, plus a handful of brake taps *with the
gas still down*.

```
race 0.000–0.180  small wiggle   | ±21 either way, settling straight
race 0.310–0.660  full RIGHT     | −127 held for a third of a second
race 0.860–1.420  full RIGHT     | held, and brake TAPPED through it —
                                 |   14 taps of 10–60 ms, gas never lifted
race 1.560–2.770  straight       | 1.2 s of nothing: this is the descent,
                                 |   accelerating to 325 km/h
race 2.770–2.820  full LEFT stab | +127 for 50 ms
race 2.820–4.080  straight       | another 1.3 s of nothing
race 4.440–4.490  full RIGHT stab| −126 for 50 ms — this is the launch input
race 4.920–5.430  the flight     | continuous analog steering, ±80, correcting
                                 |   attitude while airborne
race 5.470–5.640  brake in air   | 110 ms + 30 ms, gas still down
race 6.220–7.750  gate threading | the busiest part: sustained analog work
                                 |   between ±90, one brake burst at 6.310
race 7.750–8.898  landing + run-in| settling onto the ramp and into the gate
```

Counted properly: **349 input change events**, **158 distinct steering values**,
**63 ticks of brake** across 19 separate applications, and **zero throttle
lifts**. The two long straights (1.2 s and 1.3 s of literally no input) are the
descent and the top of the flight — the run is doing nothing at all for a
quarter of its length.

## Can a human do this?

Not this tape. It is **349 input events across 158 distinct steering values** —
an analog run, nothing like the low-input tapes elsewhere in this repo. What a
human can take from it is the *idea*: the launch off the descending section
carries far enough to reach the finish, and no checkpoint stops you.

If you want a human-shaped target on this map, **32.219 is the one to chase** —
that is the intended line, and it is not far off the author time.

## How it was found

There is no author ghost embedded in the map, and when the search ran there was
no human record either, so there was no reference line at all. The starting point
was a video of somebody's near miss, with the input overlay read frame by frame
and the strategy — steering keyed to *speed*, not time: right until 119 km/h,
off, right again from 125 to 152, then straight — reconstructed from it and
confirmed against the video's own speed readout.

From there: 32.219 → 15.247 → 9.515 → 9.415 → 8.899 → **8.898**, each rung
re-simulated from the written tape.

## Verification

Nadeo's own dedicated server, on the unmodified map:

```
"ValidatedResult" : { "NbCheckpoints" : 1, "Time" : 8898 }
"DeclaredResult"  : { "NbCheckpoints" : 1, "Time" : 8898 }
"IsValid" : true
```

The ghost carries its own regenerated telemetry — read out of engine memory
while replaying the tape's inputs — so it plays back as itself rather than as
the container it was built in. Two independent regenerations, on different
machines by different code paths, agree on the trajectory to **0.487 mm**.

> **Read that 0.487 mm as a WARNING, not as a certificate — corrected
> 2026-08-22.** ≈0.5 mm is now measured to be **the distance between two copies
> of the car in the server's own memory**, so two regenerations agreeing at that
> figure are consistent with both having transformed from the wrong copy. A
> gather that has actually found the car agrees **bit-identically, or at
> ~0.000001 m** — transforming from the copy with a live wheel block took
> bit-identity from 0 of 455 samples to 227 of 455 on the map-2 answer key.
> This does not overturn anything else on this page: the **time** is the
> oracle's, read off the tape, and the tape is not in question. It means the
> trajectory in the rebuilt file should be re-checked against the right copy
> before the clip is treated as frame-accurate. See `tools/README.md`.

The run is also more physically self-consistent than the human recordings we
hold: position and velocity disagree by 1.25% of speed across the run, against
2.3% and 1.7% for two downloaded leaderboard ghosts. The only place they
genuinely part company is a 150 ms burst at 1.84–1.98 — the contact, which the
human's video shows too.

## Files

| file | what |
|---|---|
| *(no replay committed)* | The 8.898 tape was withdrawn — it reported a stranger's account, the same one as its sibling on [untitled 01](../276874-untitled-01) — and a rebuilt one is owed here. The rebuilt 9.415 the clip is shot from is not committed either. |
| `inputs/TAS_8898.inputs.csv` | per-tick inputs — **the run itself**: this is what the oracle validates at 8.898 |
