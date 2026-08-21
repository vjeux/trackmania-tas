# YEET Fall 2024 - 04

**Everybody climbs. The reactor fires out of the nose, so where the car points is
where it goes — apex at 82 m and spend the boost going forwards, not at 99 m and
spend a second and a half coming back down.**

**Video — both cars at once:** <https://github.com/vjeux/trackmania-tas/releases/download/videos-v1/203072-keyboard-10743-vs-wr-12083.mp4> — our keyboard flight **10.743** against the human world record **12.083** (Bonobo.e).

| run | time | vs author time | vs human WR | steering |
|---|---|---|---|---|
| **TAS** | **10.640** | **−0.694** | **−1.443** | analog |
| action-key flight | 10.717 | −0.617 | −1.366 | 5 values, 22 events |
| **keyboard flight** | **10.743** | **−0.591** | **−1.340** | **3 values, 14 presses** |
| Author time | 11.334 | — | −0.749 | — |
| Human WR — ayti__ | 12.083 | +0.749 | — | — |

TMX map [203072](https://trackmania.exchange/maps/203072) · **272 recorded runs**
· one checkpoint.

This is an Altered Nadeo copy of **Fall 2024 - 04**, with geometry and surfaces
preserved, so the 600 000 people on the official map are driving the same road.

## What the map is

| phase | race | what |
|---|---|---|
| A | 0 → 4.4 | ground, accelerate to ~200 km/h |
| B | 4.4 → 6.4 | two full-lock direction changes, then the launch |
| C | **6.4 → finish** | **~4.2 s of powered flight**, 228 → 430 km/h |

**71–76% of the boost lies along the car's own nose.** It is not a belly-mounted
reactor pushing you upwards; it is a thrust vector you aim by pointing the car.
That single fact is the map.

## Where the time is: fly flatter

The line is not exotic — it sits a mean of 7.7 m from the world record's own
path, closer than most of the field. What differs is the shape of the arc:

| run | apex height | apex time |
|---|---|---|
| **this run** | **81.8 m** | **8.85** |
| human WR | 99.3 m | 9.70 |
| rank 2 | 97.2 m | 9.40 |
| rank 5 | 120.1 m | 9.47 |
| rank 3 | 130.3 m | 9.25 |

**Every human in the sample climbs higher.** The world record is actually *ahead*
through the middle of the flight and then spends 1.5 seconds converting height
back into progress — it reaches the same point down-track at 9.95 against our
10.69, and still loses 1.2 s by the line, because it bought altitude we never
paid for.

### There is an 840 ms dead zone right after the launch

From about 6.44 to 7.20, steering does *literally* nothing: fifteen different
constant steering values through that window return the identical millisecond,
and a single-tick nudge anywhere inside it changes nothing either. Everywhere
else on this map the same nudge usually ends the run.

The window is tied to the flight rather than to a place or a clock — it starts at
your own takeoff. **The attitude you carry into 6.44 is the attitude you have at
7.20, and nothing you do in between matters.** What you set *going into* it is
worth more than anything inside it.

### The launch is a combination lock

The last three differences that produced the fastest tape are all in the 1.5 s
before the launch: **40 ms off the throttle at 4.82 during the full-right lock**,
**10 ms off the throttle at 5.49 during the full-left lock**, and **holding about
10% left lock into the launch at 6.25 instead of unwinding to centre**. All three
together are worth **0.566**. Every proper subset of them either misses the
finish or is slower — the gradient points away from the answer in all three
directions, which is a fair answer to what 272 people missed: nothing
incremental.

Honestly stated: those three edits are differences within one lineage, not a
recipe to bolt onto someone else's run — grafted onto a human tape they do not
finish. What transfers to a driver is the flat arc and the tape below.

## The run as inputs

The whole 4.2-second flight, on three keyboard values and fourteen presses.
Throttle is full throughout except the two blips above; the brake taps at 3.01
and 4.01 are ordinary ground driving.

```
  6.44   release the wheel   (into the dead zone — nothing matters until 7.20)
  7.20   FULL LEFT
  7.35   release
  7.88   FULL LEFT
  8.35   release
  8.56   FULL LEFT   (brief)
  8.59   FULL RIGHT
  9.23   release     (brief)
  9.25   FULL RIGHT  (brief)
  9.28   release     (brief)
  9.30   FULL RIGHT
  9.98   release
 10.05   FULL RIGHT
 10.26   release
 10.64   finish
```

## How forgiving it is

Mistime one press and re-time the ones after it — what a driver who is late
actually does — and this tape holds up better than the human run it was seeded
from:

| tape | still finishes | still within 0.050 of its own base |
|---|---|---|
| **this one, 14 presses** | 88% | **85%** |
| the human seed's own tape | 96% | 45% |

**Twelve of the fourteen presses have ±30 ms of slack.** The exceptions:

- the release at **6.44** is genuinely tight — one tick either way and you do not
  finish. It is the entry to the dead zone, and it sets the attitude for the next
  three quarters of a second.
- the two full-lefts at **7.20** and **7.35** are fragile on the early side only.
- the cluster at **9.23–9.30** is three flickers inside 70 ms. **That is the part
  that will take real practice.**

## Files

| file | what |
|---|---|
| `replays/KEYBOARD_10743.Ghost.Gbx` | **3 values, 14 presses — the one to study** |
| `replays/ACTIONKEY_10717.Ghost.Gbx` | small action-key ladder |
| `replays/TAS_10640.Ghost.Gbx` | the fastest run |
