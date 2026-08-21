# Miru's Hell 2

**This is not a driving map. It is a wall of 1,173 spinning rotors, and the whole
run is one question: can you arrive in a state the upper row will accept? Our
19.427 is the first tape of ours that does — through the low row, caught by the
upper one at 779 km/h.**

**Miru's Hell 2** — TAS **19.427** (+0.621) | AT 18.806 | WR 21.105 by deeperjungle

https://github.com/user-attachments/assets/d7f87580-9b89-4ae3-9eed-ea3f0b232053

**Ours on the left, deeperjungle's 21.105 on the right, both clocks from the same
start.** They take visibly different lines through the red structure from about
6 s, and by 12.6 s ours is a whole section ahead. It is two panes rather than one
camera because the runs finish **335 m apart at their widest** — a chase camera
would lose the second car within seconds.

| run | time | vs author time | vs the record |
|---|---|---|---|
| **TAS, validated (not yet watchable)** | **18.942** | **+0.136** | **−2.163** |
| **TAS, the filmed one** | **19.427** | +0.621 | −1.678 |
| TAS, watchable earlier | 20.296 | +1.490 | −0.809 |
| TAS, earlier still | 20.942 | +2.136 | −0.163 |
| Author time | 18.806 | — | −2.299 |
| **Record — deeperjungle** | **21.105** | +2.299 | — |
| lqpzz | 23.689 | +4.883 | +2.584 |
| Herrlille | 25.681 | +6.875 | +4.576 |

**The author time is not beaten** — but it is close. A newer tape validates
**18.942**, only **0.136** short, and a second in the same batch reaches 19.276, so
the basin has depth. That tape is not yet certified as a renderable ghost, so the
clip above is still the 19.427; the times below are what the game validates. What the
run is, is the fastest time anyone has recorded on this map — 1.678 under the
current record — and the first evidence that the author's own launch mechanism is
reachable at all.

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
reach.** That is where the time is, and our own two best tapes prove it:

| tape | launch speed | gate crossed | time |
|---|---|---|---|
| 19.427 | **779 km/h** | #1031 at (1008, 402, 1360) | 19.427 |
| **18.942** | **696 km/h** | **#1033 at (1104, 394, 1232)** | **18.942** |
| the author | 884 km/h | #1026 at (1008, 474, 1136) | 18.806 |

**The faster of our two launches at 83 km/h LESS.** It reaches a nearer gate and
wins by 0.485. So peak launch speed is the wrong objective — **where the launch
puts you is the objective**, and an earlier note on this page saying the residue is
"launch quality" was at best incomplete.

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
