# untitled 02

**The author time on this map is not route-enforced. There are no checkpoints,
so the only thing you have to touch is the finish gate — and you can reach it by
launching off the track and flying most of the way there.**

| run | time | vs author time |
|---|---|---|
| **TAS** | **8.898** | **−22.501** |
| TAS, earlier rung | 9.415 | −21.984 |
| TAS, the intended-ish line | 32.219 | +0.820 |
| Author time | 31.399 | — |
| Human record | *none — nobody has a time on this map* | — |

TMX map [276877](https://trackmania.exchange/mapshow/276877) · author
**DugonGOD** · **0 recorded runs**.

Video of the run: <https://pxl.cl/cv5jm> (the 9.415 rung; the line is the same).

## What is going on here

This is a gimmick map — 31 reactor and reset gates, 18 no-steering gates, boost
pads and slow-motion blocks — and it has **no checkpoints at all** and **two**
separate finish gates. Nothing forces you round the course. The author time of
31.399 is what it costs to drive the track as built; the finish gate can be
reached in **8.898** by not driving it.

So read this page as a demonstration that the map's time is soft, not as a
driving lesson. **32.219 is the honest number** — that is the first finish
anybody has recorded here, following roughly the intended line, and it is still
0.820 over the author time.

## The line

Off the start platform, straight down into the dark descending section, which is
where all the speed comes from — **325 km/h** by the bottom. There is a hard
contact at **1.86** that the run simply absorbs. Launch at **4.76**, and the
reactor-up zone lifts the car about **21 m** while it decelerates; from there it
is above the cloud layer with the map's own geometry passing underneath. Thread
the gate columns at x ≈ 752 / 688 / 624 around y 281–287, come down onto the
curved ramp, land at **8.5**, and roll into the Goal item at (580, 284, 713) at
**41 km/h**.

The whole middle of the run is off the built track. That is the trick, and it is
visible in the video within a couple of seconds.

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
| `replays/TAS_8898.Ghost.Gbx` | the run |
| `inputs/TAS_8898.inputs.csv` | per-tick inputs |
