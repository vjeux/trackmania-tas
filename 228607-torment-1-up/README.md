# Torment (1-UP)

**The launcher at the base of the end wall only fires if you arrive sideways —
and on this map, unlike its low-finish twin, you must release the lock about
200 ms after it fires and then counter-steer, a touch early rather than a touch
late.**

**Torment (1-UP)** — TAS **19.907** (−0.351) | AT 20.258 | WR 24.512 by surms41

https://github.com/user-attachments/assets/8c17c104-ce3d-4dfe-bfb4-c1e6b3cc8d8b

Single car: this map's human record is 24.512, over four and a half seconds slower, so a
side-by-side in one camera would show one car finishing while the other is still
most of a straight behind. Two more of our tapes are filmed for comparison —
[19.927](https://github.com/user-attachments/assets/dab645ee-4ec9-4958-b894-b24373ff4c67)
and the more forgiving
[19.948](https://github.com/user-attachments/assets/0282a92c-5227-4516-821f-2ae50123991d),
which is the one to copy if you are driving it yourself.

**Torment (1-UP)** — TAS **19.907** (−0.351) | AT 20.258 | WR 24.512 by surms41

https://github.com/user-attachments/assets/7a263da5-a1b2-41c7-b110-4e36c9f97a63

**The comparison, as a split screen: our 19.907 on the left, surms41's world
record on the right, both clocks running from the same start.** A single camera
cannot hold this pairing — the two runs are **356 m apart** at the widest point,
so the opponent spends the run behind the lens — but side by side the whole
difference is legible. Watch the middle of the run: we are already through the
green gate while the record is still climbing to it, and by 18.250 we are on the
final yellow ramp with the record two sections back. When our car parks at the
flag, his is still driving; that held frame *is* the 4.605 s.

| run | time | vs author time | vs human WR |
|---|---|---|---|
| **TAS** — [`TAS_19907`](replays/TAS_19907.Ghost.Gbx) | **19.907** | **−0.351** | −4.995 |
| **the forgiving one** — [`FORGIVING_19948`](replays/FORGIVING_19948.Ghost.Gbx) | 19.948 | −0.310 | −4.954 |
| low-input, 16 steer values | 20.070 | −0.188 | −4.832 |
| Author time (never beaten by a human) | 20.258 | — | −4.254 |
| Human WR — surms41 | 24.512 | +4.254 | — |

TMX map [228607](https://trackmania.exchange/maps/228607) · author
**Bernkastel_.**, the title crediting **Emelius.** · **27 recorded runs**.

> **These tapes are one family, not independent attacks.** A near-identity sweep
> finds every pair of our runs on this map agreeing to within a millimetre for the
> **first 18.2 seconds of a 20-second run**, diverging only in the last two. They
> carry *different* input tapes, so they are branches off a shared parent rather
> than copies of one another — 79 such pairs on this map alone, the largest family
> in the project.
>
> Nothing about the times or the driving is in question: each validates on the
> game's own oracle. But the rows above are variations on one solution's ending,
> and a reader is entitled to know that before reading them as four ways of
> attacking the map.


This map is the same map as [Torment (1-DOWN)](../228811-torment-1-down) with the
finish moved 64 m higher, and it is a remix of an official campaign map that
400 000 people have driven. Both fields matter below.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Where the time is

At the base of the end wall the floor is 96 m of boost platform, and running
through it at z ≈ 709 there is a trigger about a metre wide. Cross it correctly
and the game fires the car along its own nose: **340 → 769 km/h in a single
contact**. That one contact is the whole 4.254 s between the author and the
board.

**Not one of the 27 recorded runs on this board ever fires it.** They all climb
the end wall to y ≈ 142–147 at 373–497 km/h and fly back to the line. It is not
that the feature is hidden — every finisher drives straight over it, because it
sits inside a checkpoint gate everybody has to pass through.

**The launcher is guarded by attitude, not by position or speed.** What it wants
is the car's *velocity* turned across the track axis:

| run | speed at the x = 80 plane | velocity off the track axis | launcher |
|---|---|---|---|
| **the author** | 341 km/h | **43.9°** | **fires → 769 km/h** |
| rank 6 | 443 km/h | 0° | no |
| human WR | 366 km/h | 5° | no |
| rank 8 | 402 km/h | 0° | no |
| best of the rest (all three are wrecks) | — | 9.9–14.9° | no |

Rank 6 passes within a metre of the author, 100 km/h faster, and nothing
happens. **Every clean run in the field is within 4° of straight.**

### And then the part nobody does at all

On the official version of this map the launcher is common knowledge: the top 15
records all fire it, at 692–997 km/h. What separates the author from those
400 000 players is what he does in the second *after* ignition. Because this
map's finish is 64 m higher than the original's, the currency here is height.

| | vertical speed at ignition + 0.45 s | height crossing the finish band |
|---|---|---|
| **the author** | **+79.5 m/s** | **160.5 m, still climbing** |
| official WR | +62.1 | 135.0 |
| best of the official top 15 | +68.6 | 143.6 |
| worst of the official top 15 | +49.8 | 127.6 |
| the low-finish twin's author | +30.7 | 95.3, flat |

**The author is 17–33 m above the entire visible top of a 400 000-record field.**

Height at the band comes from two things, and everybody trades one against the
other: how steeply the launcher throws you, and how much of the climb you keep.

| | vertical speed at ignition | mean vertical acceleration to the band | height at the band |
|---|---|---|---|
| **the author** | **92.0** | **−27.1** | **160.5** |
| the field's steepest launch | 101.6 | −45.5 | 134.3 |
| the field's best coast | 74.6 | −26.3 | 136.8 |
| official WR | 85.1 | −34.3 | 135.0 |

Gravity alone is −24.7, so the author is coasting on gravity and almost nothing
else. Both of his numbers sit inside the field's own demonstrated range; **he is
simply the only person who does both at once.** The reason nobody else does is
physical: the steep launch is a nose-up attitude at the contact, and the same
attitude keeps the car rolling until it goes past inverted, presents its flank to
800 km/h of airflow and pays 90 km/h in the first tenth of a second. **The runs
that keep their climb are the runs whose roll never gets there.**

## The run, as inputs

Everything before race 17.9 is the ordinary line down the map and the field
already drives it — the whole gap is the last two and a half seconds. These
timings are the author's own lap, which is the reference for this technique.

```
race 17.990   steer right 0.33, gas          turn in, 361 km/h
race 18.040   steer right 0.86
race 18.090   steer right 0.95
race 18.140   full right lock, GAS AND BRAKE HELD TOGETHER
              held to 18.490 — the car scrubs ~30 m across the floor,
              358 → 340 km/h, velocity rotating 15° → 44° off the axis
race 18.540   the launcher fires: 340 → 769 km/h, climbing at 92 m/s
race 18.740   RELEASE to centre  (~200 ms after ignition)
race 18.74 → 19.390  counter-steer progressively to full left
```

The scrub is the move: 18 km/h lost over 400 ms, and 44° of velocity gained.
Full lock with both pedals down is not a turn, it is the only way to rotate the
car's velocity out of the direction it is pointing.

The release is the other move. Hold the lock instead and the roll runs on past
inverted; release and counter-steer and the roll stops at −1.61 and comes back to
−0.18, the nose falls in line with the 25°-up flight path, and the car holds
769 → 720 km/h and +92 → +68 m/s of climb all the way to the gate. The official
world record already performs this release, 10 ms later than the author does — he
is 25 m lower only because his launch was flatter.

**Cues rather than a clock:** commit to the scrub as the car comes off the last
drop toward the wall base, with both pedals down and full lock; the launch is
unmistakable when it fires; let go about a fifth of a second after it, and feed
in opposite lock as the car swings.

## How forgiving it is

Slip the steering by one tick and re-drive from there:

| the slip happens at | 10 ms early | 10 ms late |
|---|---|---|
| race 18.70 … 19.20 (the release and the counter-steer) | **19.936 — keeps the run, and is 0.011 faster** | **loses the finish** |
| race 19.40 … 19.70 | — | 20.065 — survives, merely slower |
| race 19.80 … 19.90 | 19.946 / 19.941 | 20.172 |
| after race 20.10 | no change | no change — the flight is committed |

> **Release the lock a touch early rather than a touch late.**

It follows from the mechanism: the flight is ballistic after ignition, so
stopping the roll sooner leaves more of the launch's climb intact, while
stopping it later has already spent it.

**The budget is one tick, not more** — two ticks early loses the finish from
every point tested. These numbers come from slipping a frozen tape, so they are
the worst case; a driver correcting as they go has more room than that, but the
*direction* of the rule is the part to take.

The one input with no slack at all is the launcher contact itself: perturb the
run from before ignition and it dies at ±10 ms either way.

**What will take real practice** is the scrub — arriving at the deck at 340 km/h
with 44° of velocity across the axis, having lost almost no speed getting there.
The release after it is a single, teachable action, and a person has driven it.

**One thing not to copy:** the low-input tape below is the same time as its
analog parent on a third of the inputs, and it is *less* forgiving, not more — it
survives no mistiming in either direction. On this map the simpler-looking tape
is the harder one to drive.

## Files

| file | what |
|---|---|
| `replays/FORGIVING_19948.Ghost.Gbx` | **the one to study** — the same tolerance as the record run with its window re-centred, 18 ticks of slack early and 15 late, for 0.041 |
| `replays/TAS_19907.Ghost.Gbx` | the fastest validated run |
| `replays/LOWINPUT_20070_16values.Ghost.Gbx` | 16 steer values — the counter-example: same time as its parent, no tolerance |
| `replays/AUTHOR_LAP_20258_watchable.Ghost.Gbx` | the author's own author-time lap, watchable; it is a recording, so it can be watched but never re-driven |

Do not carry [Torment (1-DOWN)](../228811-torment-1-down)'s closing instruction
across to this map. It says to keep the lock held after the launcher, which is
right for a low finish and sends you broadside at 562 km/h here.
