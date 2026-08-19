# U10S_32 [Yeet] MAX-UP

**Do not slam the wheel into the edge: feather out of the lock over the last two
tenths of road, because the moment the wheels leave, the steering does nothing
for 1.2 seconds and your launch attitude is already decided.**

| run | time | vs author time | vs human WR | inputs |
|---|---|---|---|---|
| **TAS** | **7.463** | **−0.241** | −0.430 | analog |
| TAS, keyboard climb | 7.474 | −0.230 | −0.419 | 24 presses, 3 steering values |
| **TAS, keyboard climb, 30 ms hold floor** | **7.476** | **−0.228** | −0.417 | **15 presses**, 3 values |
| TAS, four-input climb | 7.514 | −0.190 | −0.379 | 4 inputs, 2 values — the fewest, and the hardest to hold |
| Author time | 7.704 | — | −0.189 | — |
| Human WR — Whatever8319 | 7.893 | +0.189 | — | — |

TMX map [274191](https://trackmania.exchange/maps/274191) · author
**Everios96** · **3 recorded runs**.

"Keyboard" here describes **the climb only**. Every run on this map, ours and
the world record's alike, uses about 48 distinct steering values on the ground
before the launch — restricting the ground phase to a keyboard alphabet does not
finish at any resolution tried.

## The physics everything rests on

This is a **Reactor** map. In the air the car is pushed at a constant
**~44 m/s² (4.5 g) along its own −up axis** and nothing else — 87% of the
non-gravity acceleration on that one body axis, on every human tape. The
thruster is bolted to the car's belly: **where the belly points is where you
go.** That is also why every run flies nearly inverted; that is what it takes to
aim "out through the floor" at the sky.

And there is a **dead zone: 1.2 seconds, from 2.890 to 4.100, in which the
steering wheel does nothing.** Force any constant onto the wheel anywhere inside
that window and the run returns the same millisecond. Both the world record and
rank 2 spend that time holding full lock, which is the clearest possible sign
that nobody knows.

The pedals are *not* dead there — gas and brake are live pitch controls in the
air. This is a three-pedal map, not a steering-only one.

## What the field does wrong

Because the wheel is dead for 1.2 s, **the attitude at which you meet the
launcher is fixed by the rotation you carry off the lip.** Nothing after the lip
can change it.

| | leaves the lip at | arrives at the launcher | keeps |
|---|---|---|---|
| human WR | 3.80 rad/s | nose-up **58°** | 148 km/h |
| this run | 3.58 rad/s, about a different axis | **35°** | **197 km/h** |

**All three humans slam to full lock and pin it into the edge.** The fast line
does the opposite: it peaks just short of the stop and **unwinds about 15 of 127
units of lock over the last 100 ms of road, braking 20 ms later.** That is worth
0.046 directly — and it unlocks 0.288 more, because from a better launch you can
hold the thruster flat: **+17 to +25° above horizontal against the human's
+53°**, which is 2% of the thrust wasted instead of 8%.

At 5.000 the fast line is *lower* than the world record (33 m against 47 m) and
40 km/h faster: **the climb is bought with speed, not with altitude, and the
reactor buys the height back.**

| race | human WR | this run |
|---|---|---|
| 2.890 leaves the road | 257 km/h, y = 33 | 253 km/h, y = 33 |
| **3.600 strikes the launcher** | **148 km/h** | **197 km/h** |
| 5.000 | 391 km/h, y = 47 | **431 km/h, y = 33** |
| 6.000 | 530 km/h, y = 100 | **612 km/h, y = 105** |
| 7.000 | 668 km/h, y = 171 | **767 km/h, y = 195** |

The launcher costs the human 109 km/h and costs this run 56.

## Sector by sector, off what you can see

Full throttle from the line. The brake comes on at 2.810.

**Sector 1 — the road, 0 → 2.240. Drive it exactly like the world record.**
This tape *is* the world record's own inputs here, and one tick of difference
anywhere does not finish. Left off the line at 0.520, straighten, the flicks at
0.900 / 1.380 / 1.750, on to the booster.

**Sector 2 — the last 0.65 s of road, 2.240 → 2.890. The only hard part.**
You are turning left onto the edge at about 260 km/h.

1. **Turn in a fraction harder and earlier** — about 10% more lock from 2.500
   than feels natural.
2. **Do not fully unwind at 2.700.** The world record lets the wheel go to
   centre here; keep a whisker of left in it.
3. **The one that matters: do not slam to full lock into the edge.** Reach peak
   lock at about 2.770, *just short of the stop*, then **feather out of it all
   the way to the lip** — roughly a tenth of the wheel unwound over the last two
   tenths of a second. Brake as you begin unwinding, not before.

**The cue is the edge of the road, not a clock: you should be unwinding as the
front wheels reach the lip, not fighting the stop.**

**The check is a number on the speedo,** at the instant the launcher throws you,
about 3.600:

| speed | what it means |
|---|---|
| 148 km/h | you drove it like the current world record → ≈ 7.893 |
| 170 km/h | about the author time |
| **197 km/h** | the fast line |

**Sector 3 — the fall, 2.890 → 4.100. Hands off the wheel; feet still working.**
Steering is ignored. The fast tapes let the brake off briefly at about 3.530 and
again into 3.830, straddling the launcher strike at 3.600, and those two are
10 ms-tight. If you want the simple version, hold the brake like the world
record does and give up a few hundredths — the 0.228 is not in these.

**Sector 4 — the climb, 4.100 → 5.900.** Control returns at 4.100, the moment
the launcher has finished with you and the reactor takes over. One idea:

> **Stop the car swinging nose-up. Point its belly at the far side of the map,
> not at the sky.**

Belly aimed 50° up and half the thrust goes nowhere; aimed 20° up and it goes
where the gate is. **The horizon is your instrument** — the field lets the nose
keep climbing; you check it and hold it low.

**Sector 5 — 5.900 to the gate. Already decided.** Freeze everything from 7.400
and the time does not change.

## The run, as keys

The 15-press keyboard tape, `replays/KEYBOARD_7476.Ghost.Gbx`. `L` = full left,
`R` = full right, `—` = centre.

```
  4.100  R      as control returns — the catch
  4.410  —
  4.510  L
  4.610  —      <-- the "check the nose" release
  4.840  L      .......... 320 ms
  5.160  —
  5.250  R
  5.310  —
  5.390  L      .......... 490 ms, the long one
  5.880  —      from here on nothing is worth more than a few thousandths
  6.650  L      (30 ms tap)
  6.680  —
  7.150  R      (60 ms tap)
  7.210  —
  7.490  L      hold to the gate
```

**And the fewest-input version, for reference — but do not learn this one.** The
7.514 tape flies the whole climb on four inputs and two values, then holds the
wheel centred for the last 2.3 seconds:

```
  4.100  (R already held from before control returned)
  4.500  L
  4.610  —
  4.840  L      .......... 320 ms
  5.160  —      and CENTRE all the way to the gate
```

It is 0.190 inside the author time, and it is the least forgiving tape on the
map — see below. Fewer inputs is not the same as easier.

## How forgiving it is

Mistime one press in the climb and keep driving — the cost against 7.476.
Anything up to +0.228 still beats the author time, and every number below does.

| press | −50 ms | −30 | −20 | −10 | +10 | +20 | +30 | +50 |
|---|---|---|---|---|---|---|---|---|
| 4.100 / 4.410 / 4.510 | — | +0.091 | — | +0.024 | +0.006 | +0.014 | +0.014 | — |
| 4.610 | — | — | +0.010 | +0.002 | — | +0.041 | — | — |
| 4.840 | — | — | — | +0.034 | +0.006 | +0.010 | +0.014 | +0.028 |
| 5.160 | +0.006 | +0.004 | +0.002 | +0.001 | +0.009 | +0.011 | +0.025 | +0.041 |
| 5.250 | — | +0.006 | +0.009 | +0.000 | +0.001 | +0.001 | +0.002 | +0.004 |
| 5.310 | — | — | — | +0.008 | +0.001 | +0.002 | +0.004 | +0.008 |
| 5.390 | — | +0.042 | +0.012 | +0.000 | +0.001 | +0.002 | +0.002 | +0.005 |
| 5.880 | +0.012 | +0.015 | +0.015 | +0.000 | +0.002 | +0.001 | +0.004 | +0.006 |
| **6.650 / 6.680 / 7.150 / 7.210 / 7.490** | **+0** | **+0** | **+0** | **+0** | **+0** | **+0** | **+0** | **+0** |

(A dash is a mistiming that does not finish.)

**The first three presses return identical rows: they are one 400 ms phrase, and
only the total rotation matters, not which of the three you were late on.** From
5.160 the tape is comfortable, ±30–50 ms. **The last five presses cost literally
nothing however you place them.**

**Which tape to actually hold.** Displacing every input in turn by one tick and
counting what still finishes, with the world record's own tape put through the
identical test:

| tape | time | survives |
|---|---|---|
| **the unconstrained floor** | **7.463** | **70.5 %** |
| human WR (the control) | 7.893 | 71.9 % |
| keyboard climb, 24 presses | 7.474 | 54.7 % |
| four-input climb | 7.514 | **17.8 %** |

**The fastest tape is also the most forgiving, within 1.4 points of the human's
own — there is no speed-versus-forgiveness trade on this map.** And the tape
with the fewest inputs is the worst of the four by a wide margin: stripping a
tape down to its minimum deletes exactly the inputs that were absorbing error,
so it ends up with no slack anywhere. Hand a person `TAS_7463`.

**What will take real practice** is the feather in sector 2. Every input between
2.580 and 2.850 is 10 ms-critical on a frozen tape — but so is the world
record's own run, which dies on 98 % of the same mistimings, because a recorded
tape has no eyes and a driver does. The useful version of that fact: **the last
input in the whole lap a one-tick error can kill is at race 4.530 — all 92
events after it survive being displaced either way.** On a map that is 63 %
airborne, the flight looks like the hard part and is the only part you cannot
get wrong.

The change asked of you on that stretch of road is **not a new input, it is
taking one away**: stop pinning the wheel into the lip.

## Files

| file | what |
|---|---|
| `replays/TAS_7463.Ghost.Gbx` | **the fastest run, and the most forgiving — the one to learn** |
| `replays/KEYBOARD_7476.Ghost.Gbx` | fifteen presses in the climb, 7.476 |
| `replays/KEYBOARD_7474.Ghost.Gbx` | 24 presses, 7.474 |
| `replays/KEYBOARD_4input_7514.Ghost.Gbx` | the whole climb on four inputs, 7.514 — the fewest inputs, the least slack |
