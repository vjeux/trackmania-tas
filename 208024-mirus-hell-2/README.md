# Miru's Hell 2

**This is not a driving map. It is a wall of 1,173 spinning rotors, and the whole
run is one question: can you arrive in a state the upper row will accept? The
author time is now beaten — 18.160 against 18.806.**

**Miru's Hell 2** — TAS **18.160** (−0.646) | AT 18.806 | WR 21.105 by deeperjungle

https://github.com/user-attachments/assets/f6d9b714-2c3d-4ac8-a39d-6adea8eea3ee

**The author time is beaten, by 0.646.** 18.160 is **0.646 under the 18.806** and
**2.945 inside the human record** — deeperjungle's 21.105, set on 2026-08-20 and
still standing when this board was re-pulled on 2026-08-21. This is the first
author-time beat on this map. It is worth keeping the two margins apart: the
human board has three runs on it and has not converged, so being 2.945 clear of
it says little, and **the 0.646 against the author is the result.**

**One camera, both cars.** Ours is the magenta car the camera follows;
deeperjungle's 21.105 is in the same frame, not in a second pane. They are
within **25 m of each other for the first 15.5 s** — 232 of 364 paired samples
sit in the band where two cars read as two cars — and then ours goes: **42 m
apart at 16 s, 92 m at 17 s, 183 m at 18 s.** deeperjungle leaves the shot in the
last two seconds and does not come back, which is the whole point of the clip.
The earlier 19.427 below is a split screen because *that* pairing separates from
the start; this one does not need one.

**The clip is 18.4 s and the runs are 18.160 and 21.105, which needs saying.**
A MediaTracker camera lives exactly as long as the ghost it is bolted to, and
ours stops sampling at 18.150. The full render runs to the longer block at
21.13 s — and measured frame to frame, **everything after 18.20 s is a frozen
still**, clock stuck at 18.160, deeperjungle not drawn. That is 2.9 s of dead
picture that `blackspans=0` and a duration check both pass. It is cut. Nothing
was lost with it: the last live frame is our car crossing the ring.

**What it cost to get a file worth filming.** Across **52 regeneration attempts
on this tape, 4 landed on the true clock — under 8 %.** The rest cluster at
7.78 m, 24–25 m, 403 m and 1368–1372 m from the run's own route. The two winners
in the final batch are byte-identical to each other, and they were picked by
ranking every attempt against ground truth rather than by taking the answer the
attempts agreed on: **the largest agreeing cluster was one of the wrong ones.**
The filmed file is the second reconstruction, `mh2_WATCHABLE_18160_v2`
(md5 `0f63623a…`), which sits **2.5 mm** from the tape's own route dump where the
first sat 0.865 m.

**Miru's Hell 2** — TAS **19.427** (+0.621) | AT 18.806 | WR 21.105 by deeperjungle

https://github.com/user-attachments/assets/d7f87580-9b89-4ae3-9eed-ea3f0b232053

**The clip above is the 19.427, an earlier tape, against the record.** Ours on
the left, deeperjungle's 21.105 on the right, both clocks from the same start.
They take visibly different lines through the red structure from about 6 s, and
by 12.6 s ours is a whole section ahead. It is two panes rather than one camera
because *those* two runs finish **335 m apart at their widest** — a chase camera
would lose the second car within seconds.

| run | time | vs author time | vs the record |
|---|---|---|---|
| **TAS, the filmed one** | **18.160** | **−0.646** | **−2.945** |
| TAS, previous best | 18.942 | +0.136 | −2.163 |
| TAS, filmed earlier | 19.427 | +0.621 | −1.678 |
| TAS, watchable earlier | 20.296 | +1.490 | −0.809 |
| TAS, earlier still | 20.942 | +2.136 | −0.163 |
| Author time | 18.806 | — | −2.299 |
| **Record — deeperjungle** | **21.105** | +2.299 | — |
| lqpzz | 23.689 | +4.883 | +2.584 |
| Herrlille | 25.681 | +6.875 | +4.576 |

The 18.942 held this page's headline until 18.160 validated and reconstructed;
its clip has been withdrawn so the page shows one best. **18.160 is the fastest
time anyone has recorded on this map**, and the first that reaches the author's
own launch mechanism rather than merely getting near it.

### What was checked before it was filmed

| check | reading |
|---|---|
| gate (`tmtrajcheck --race 18160`) | PUBLISHABLE — 0 failures, 1 warning (C10 geometry) |
| custom car skin | clean — `Skins\Models\CarSport\TAS.zip`, nothing else |
| spawn vs deeperjungle's, as a rotation | **0.001 m**, \|dot\| **1.0000** |
| MediaTracker import name | `Ghost:TAS`, one track, one entity block, end 18.15 |
| donor strings (nickname, GUID, zone, storage URL) | zero occurrences |
| contamination vs the human recording | INDEPENDENT — longest near-identical run 53 samples, under the 100 bar |
| input tapes ours vs theirs | different md5s — two runs, not one lap twice |

## The rotor wall is the launcher — and the map is a gate CHOICE

The map holds **1,173 spinning `ObstacleRotor24mWing90X2Level2`** in two rows of
574 — one at y 197 / z 704, the other at y 207 / z 687, spanning x 919–1207 — plus
38 pushers. An earlier reading of this map called eight "launcher bays" the
mechanism; they are scenery. **Removing the rotor rows removes the launch**, which
is how the real answer was found.

**Every finisher needs the LOW row.** Delete it — 282 movers, with the origin
control passing — and every tape on this map DNFs, the weak-launch ones and the
author-launcher ones alike. It is not an obstacle to be got past; it is the first
link in the chain. What the fast tapes changed is not bypassing it but arriving at
the *upper* row in a state that row accepts, after the low row has done its part.

**And the map has fifteen finish gates, so the run is a choice of which one to
reach.** That is where the time is, and two of our tapes prove it:

| tape | launch speed | gate crossed | time |
|---|---|---|---|
| 19.427 | **779 km/h** | #1031 at (1008, 402, 1360) | 19.427 |
| **18.942** | **696 km/h** | **#1033 at (1104, 394, 1232)** | **18.942** |
| the author | 884 km/h | #1026 at (1008, 474, 1136) | 18.806 |
| 18.160 | not measured | not measured | 18.160 |

**The faster of the two launches at 83 km/h LESS.** It reaches a nearer gate and
wins by 0.485. So peak launch speed is the wrong objective — **where the launch
puts you is the objective**, and an earlier note on this page saying the residue is
"launch quality" was at best incomplete. Which gate the 18.160 takes has not been
read off the tape, so it is not in the argument above; it is filmed and
validated, not characterised.

**The interesting part is a gate nobody has reached.** Geometry says **#1027 at
(1168, 458, 1072)** is the cheapest of the fifteen — 63 m closer to the launcher
row than the author own #1026 — with a predicted run of about **18.45**. Nothing
has ever crossed it. #1024 and #1032 are unvisited too. If that prediction holds,
the remaining margin is sitting in a gate no tape has found, which is a far more
tractable target than out-launching the author.

Three things that are measured and closed:

- **The start cannot be shifted by one tick.** A real shift operator was applied at
  k = 0 (control, passes) and k = 1…260 — every one dies. The author's 1.90 s idle
  is not a copyable knob.
- **Lateral aim is not the blocker.** In-flight steering is worth 0.55 m over 4.3 s,
  and a car placed 0.1 m from a bay centre still crashes.
- **The finish set is measure-zero.** 20,815 exhaustive one-move edits from a
  working tape produced **one** finisher.

## Two corrections this map forced on us

**A bound we published here was wrong, and the reason generalises.** An earlier
analysis concluded, from 253 aimed arrivals, that "the approach's reachable set at
the wall is a one-dimensional curve and the author's state is not on it". The
19.427 is a three-operation candidate — gas cut, brake pulse, late steer — that
the analysis's own parameterisation could not express. The measurement was fine;
the inference was scoped to the wrong object.

> **An exhaustive search measures its parameterisation. Exhaustiveness is a
> property of the grid, never of the map.**

With numbers, from this map: every finisher here bar one came from the random
hunt, while the focused enumeration around this solution returned 48 finishers and
**not one under 19.427**. They explore different objects. Run both.

**And a 1.000 s rotor period published mid-analysis is withdrawn** — binning the
data refutes it.

## Why this page took an extra day: the file, not the run

The 19.427 was a validated *time* for hours before it was a renderable *ghost*,
and both problems are worth recording.

**Its regeneration would not certify** — the telemetry sat 7.81 m from the tape's
own route. The cause was a **clock offset**, not a bad locate: the position error
tracked the regeneration window monotonically, so the right car was being found
every time and attributed to the wrong tick. Fixing it took **24 independent
attempts ranked against ground truth**, and the distribution is the lesson —
**two** landed on the true clock (byte-identical to each other, 2.6 mm), **five
agreed with each other on a wrong answer** 7.81 m out, and eleven were 900–1400 m
away. Any procedure of the form "regenerate a few times and take what agrees"
would have shipped a file a kilometre off its own route with a majority behind it.

**And the file was in somebody else's container.** It was built by bit-patching
Herrlille's recording, and it announced itself to the game as `Ghost:Herrlille`
until it was repaired — while passing a byte census of its declared time
cleanly, because the patcher had fixed the time and nothing else.

There is a pleasing detail in that. **Herrlille is now rank 3 on this board.**
Without checking the live leaderboard before writing this page, we would have
compared our run to a record that had already fallen, using a container borrowed
from the man who lost it.
