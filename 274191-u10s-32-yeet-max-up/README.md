# U10S_32 By Everios96 [Yeet] MAX-UP — author time beaten by 211 ms

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS, unconstrained** | **7493 ms** | **−211** | **−400** |
| **TAS, pure keyboard climb** | **7494 ms** | **−210** | −399 |
| Author time (never beaten by a human) | 7704 ms | — | −189 |
| Human WR | 7893 ms | +189 | — |

TMX map [274191](https://trackmania.exchange/maps/274191) · **only 3 recorded
runs** · a 4.5 s reactor flight.

**Not submitted to any Nadeo leaderboard, and it never will be.**

The keyboard tape is one millisecond off the unconstrained floor — **20 key
presses on three steering values**. This map does not need a pad.

Field reproduction: **3/3, the entire recorded population**, not a sample.

## The mistake this map nearly produced, and the rule that came out of it

The first write-up said: *the field pins full lock through the fall and we let
go.* Plausible, complete, and **wrong** — because this map has a **1.2-second
dead zone with no air control at all** (race 2890 → 4090 ms). Replace the
steering with **any** constant anywhere inside it and the oracle returns the
same millisecond, exactly. The engine ignores the input entirely.

The real technique is one beat earlier, on the ground.

> **On any map with an air phase, sweep a constant through it before writing a
> word about what the driver is doing.** It costs one minute of machine time and
> it is the difference between a technique and a story.

## Two tooling bugs found here, both fixed

Both were in the trajectory reader, and both had been invisible because every
previous map was slower and laid out differently.

1. **The clock-search window was aimed the wrong way.** It streamed a fixed
   16 KB *below* the vehicle state looking for the counter that ticks +10 —
   which is where one earlier map always put it. Here the clock is *above* the
   state on some tapes and **319 KB below** on others, so the reader aborted
   with "no u32 advances by exactly 10 every tick near the vehicle state", which
   reads like a broken server rather than a mis-aimed window. Fixed by laddering
   the window outward.
2. **The self-check threshold was speed-blind.** It rejected a trajectory when
   `|d(pos)/dt − v|` exceeded a fixed 2.0 m/s — calibrated on a car topping out
   near 90 m/s. This car crosses the line at **215 m/s**, where a one-tick
   central difference legitimately disagrees by about 1 % of speed, so good
   trajectories were being thrown away with an alarming error message. Fixed to
   `max(2.0, 3 % of mean speed)`.

Also confirmed here: the search's `--quant` flag was wired into the fork path
only, so a classic-path arm given `--quant` ran **completely unconstrained**
while its log looked perfect. Now applied in the classic candidate loop, to the
starting state, and **only over the search window** — projecting the whole tape
wrecks a prefix the search was told not to touch.

## Files

| file | what |
|---|---|
| `replays/TAS_7493.Ghost.Gbx` | fastest run |
| `replays/KEYBOARD_7494.Ghost.Gbx` | **20 key presses, three values, one ms slower** |
| `notes/PLAN-v1.md` | the pre-search analysis |
