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

**untitled 02** — TAS **8.898** (−22.501) | AT 31.399 | WR 14.959 by Its_Cam.

> ### Video withdrawn, then partly replaced — the tape had reported another player's identity
>
> The clip that was here has been taken down. It played, and there is nothing
> wrong with the driving in it: the run is ours, the time re-simulates on the
> game's own oracle to the millisecond in its name, and the declared time in the
> file agrees with what the server validates.
>
> What is wrong is whose file it is. Read by the game's own parser, this tape
> reports account `4c3537f3-381d-46d5-879a-45eca500dd4d`, login
> `TDU38zgdRtWHmkXspQDdTQ` — **a real player, not us.** Our own files report
> login `TAS` and carry no account at all. The same stranger's identity appears
> on this map and on its sibling, so it is a person rather than an artefact.
>
> A searched tape is built inside a *carrier* — an existing ghost — and inherits
> that carrier's container unless every field is rewritten. Identity is one of
> those fields, and on these two tapes it was never rewritten.
>
> **This map has no human recording to compare against** (or, for untitled 02,
> exactly one, set the day this page was written). Every trajectory-based check
> we have is therefore blind here, and the identity read is not one check among
> several — it is the whole verdict. That is why this took until now to find.
>
> The run stands and the time stands. A replacement clip will be filmed from a
> tape rebuilt on a clean carrier.
>
> **Replaced 2026-08-21**, and note the clip below is a **different tape**: the
> withdrawn one was the 8.898, and what has been rebuilt on a clean carrier so
> far is the **9.415**, the earlier rung of the same lattice skip. Login `TAS`,
> no account id, the donor's custom livery and its download URL replaced,
> trajectory 189 of 189 positions bit-identical to its own original, imports as
> `Ghost:TAS`. **Ground contact and wheel rotation are still the carrier's** —
> zeroing them would claim the car was airborne all run with wheels that never
> turned — so the dirt and spark effects may be wrong while the path is ours.
>
> The split screen against Its_Cam.'s record was filmed from the 8.898 and came
> down with it; it returns when that tape is rebuilt.

**untitled 02** — TAS **9.415** (−21.984) | AT 31.399 | WR 14.959 by Its_Cam.

https://github.com/user-attachments/assets/fb996518-80a2-4f6a-b200-3c26f7a01ce3

*(The paragraph below describes the withdrawn split screen; it returns with the rebuilt 8.898 tape.)*

**Somebody finally drove this map, and here is our run beside theirs.** Its_Cam.
set the first human time here on 20 August 2026 — **14.959** — and took the
same shortcut we did: watch the two panes at 2.583 s and they are at the same
gate. Then ours goes through the purple lattice while theirs is still working up
to it, and the left pane parks at the flag at 8.898 while the right keeps flying
for six more seconds.

It is a split screen rather than one camera because the two runs end up **61.5 m**
apart with the human 6.061 s behind: in a single chase shot the second car is
simply not on screen, which would make a two-car caption a claim the picture
cannot support. Side by side, the difference is legible — and on a map with no
checkpoints, what is worth watching is that a human independently found the same
way to cheat it.

## What is going on here

This is a gimmick map — 31 reactor and reset gates, 18 no-steering gates, boost
pads and slow-motion blocks — and it has **no checkpoints at all** and **two**
separate finish gates. Nothing forces you round the course. The author time of
31.399 is what it costs to drive the track as built; the finish gate can be
reached in **8.898** by not driving it.

**Measured, on the 9.415 sibling rung** (same line, same lattice; the committed
8.898 tape differs only in how hard it is thrown): 100.0 % of samples sit inside an
occupied block cell, the run never leaves the lattice's own volume, and it never
exceeds its own spawn altitude — but **29.0 % of it is airborne**, 2.78 s in
total, with the longest continuous stretch **1.51 s**. So it is not flying away
from the map; it is being thrown along it by the lattice's own launches, which is
why the line stays inside the cells while barely touching anything.

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

**A correction, because the video misleads:** watching it, the middle of the run
looks like it leaves the track entirely. It does not. Measured against the map's
block census, every sample is inside an occupied cell and the run never exceeds
its own spawn altitude. What is true is that it is *airborne* for 29 % of the
run — thrown along the lattice by its own reactor zones rather than driven — so
the car is barely touching anything even while it stays within the structure.

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

There is no human record and no author ghost embedded in the map, so there was
no reference line at all. The starting point was a video of somebody's near
miss, with the input overlay read frame by frame and the strategy — steering
keyed to *speed*, not time: right until 119 km/h, off, right again from 125 to
152, then straight — reconstructed from it and confirmed against the video's own
speed readout.

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

The run is also more physically self-consistent than the human recordings we
hold: position and velocity disagree by 1.25% of speed across the run, against
2.3% and 1.7% for two downloaded leaderboard ghosts. The only place they
genuinely part company is a 150 ms burst at 1.84–1.98 — the contact, which the
human's video shows too.

## Files

| file | what |
|---|---|
| ~~`replays/TAS_8898.Ghost.Gbx`~~ | **withdrawn.** It reported the same stranger's account as its sibling on [untitled 01](../276874-untitled-01) — account `4c3537f3-…`, a real player. The time stands and re-simulates on the oracle; the *file* was the carrier's. A rebuilt tape is owed here. |
| `inputs/TAS_8898.inputs.csv` | per-tick inputs — **the run itself, unaffected**: this is what the oracle validates at 8.898 |
