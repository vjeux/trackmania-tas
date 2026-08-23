# Fall 2025 - 16 CP1 End

**The author time, matched with two small trims on the world record's own line —
each with ±10 ms of slack. 903 people have tried; the best of them is 0.003
short.**

**Fall 2025 - 16 (CP1 end)** — TAS **4.830** (−0.001) | AT 4.831 | WR 4.834 by OriginalCJM

https://github.com/user-attachments/assets/8c9001d7-61df-4dab-923f-cda6a5f6c81f

**Our 4.830 and OriginalCJM's world record, both in one camera — and they are
never two cars.** Maximum separation over the whole run is **0.184 m**, mean
**0.055 m**, so the two vehicles render inside one another from the lights to
the last frame. That is the map: the entire 0.004 between us is 15.6 cm of
forward progress at the takeoff tick, and the page's own arithmetic says
0.001 = 4.55 cm. A clip in which you cannot see the gap is an honest picture of
a gap you cannot see.

Measured, so the sameness is not left to impression: our trajectory sits inside
**0.5 mm** of his for the first **2.05 s** — 41 consecutive samples — then
departs smoothly, past 1 cm at 2.20 s and 10 cm at 3.60 s, to 0.184 m. That is
the page's claim confirmed from the telemetry: this is his route with two
inputs trimmed, and the trims are what the last two seconds are made of.

**Both recordings stop at 4.800**, 30 ms before our crossing and 34 ms before
his, and by then the car has been airborne since 4.21 — so the clip ends with
both cars in flight, still climbing, just short of a finish line neither file
contains.

*One instrument reading that needs its context.* `nearident`, the contamination
check, reads **COPY** on this pairing — 41 consecutive samples inside its 1 mm
band. It is a false positive here, and the reason is measurable: **every ghost
our pipeline writes places the car ~0.0005 m from where the game places it.**
The cleanest demonstration is 279209, a map where our tape inherits nothing —
4 samples in the band, mean separation 0.81 m — and where **three pairs of
downloaded human recordings sit at exactly 0.000000 m at the spawn while our
tape sits at 0.000500 m from two different humans and at 0.000000 m from its own
sibling.** The offset is ours, it is the same magnitude on every map measured,
and it does not depend on any donor.

> **And as of 2026-08-22 we know what it is, which this page got right in
> attribution and could not explain at the time.** The ~0.0005 m is **the
> distance between two copies of the car in the server's own memory**, and the
> regenerator was transforming from the wrong one: on the map-2 answer key,
> transforming from the copy with a live wheel block takes bit-identity from
> **0 of 455 samples to 227 of 455**. So it is ours, as this page says — but it
> is a *fixable defect in the chooser*, not a permanent property of the writer,
> and the honest expectation once fixed is bit-identity or ~0.000001 m rather
> than half a millimetre. The conclusion below is unaffected: while the offset
> is there, the 1 mm band cannot resolve a close following line from a copy.
> (The orientation half of that fix is still open; see `tools/README.md`.)

So the 1 mm band is only twice our own
writer's noise floor, and it cannot resolve a close following line from a copy
on any of-ours-versus-a-download pairing. The control that does resolve it is
per-map and human-versus-human: here three unrelated pairs of human recordings
read **3, 4 and 6** samples inside the band, against our 41 — and our input
archive is 1073 bytes of dense analog steering
against his 605 bytes of sparse keyboard, so the two tapes are not one tape.

| run | time | vs author time | vs human WR | slack |
|---|---|---|---|---|
| **TAS, human-shaped** | **4.831** | **±0** | **−0.003** | **±10 ms on every input** |
| TAS, one input change | 4.832 | +0.001 | −0.002 | 40 ms window |
| TAS, keyboard only | 4.834 | +0.003 | ±0 | 18 key presses |
| TAS, unconstrained floor | 4.830 | −0.001 | −0.004 | one-tick precision |
| Author time | 4.831 | — | −0.003 | — |
| Human WR — OriginalCJM | 4.834 | +0.003 | — | — |

TMX map [270051](https://trackmania.exchange/maps/270051) · author **in-.-** ·
903 recorded runs.

## What the map is

484 race ticks, about 4.83 s, one waypoint — the finish. **Full throttle throughout, no
brake, ever.** The whole map is a steering problem.

The last 620 ms is **ballistic flight**: the car leaves the ground at about
4.210 at 176.8 km/h and never lands, crossing the finish in mid-air, still
rising. So the finish time is decided at the takeoff tick — **3.8 of our 4 ms is
simply being 15.6 cm further along the track when the wheels leave the ramp.**
Same jump, same arc, same attitude; the line stays within 12 cm of the world
record's, well inside the field's own 1.35 m corridor.

Two consequences:

1. **Inputs after about 4.360 are worth at most 0.001.** Overwrite every input
   from there with "steer 0" and the time changes by 0 or −0.001. The car is in
   the air; steering only rotates it.
2. **0.001 = 4.55 cm of travel at the finish.** The whole 0.003 between the
   world record and the author time is 13.6 cm of forward progress.

Steering during the countdown does nothing at all.

## The run, as inputs

Negative = left, ±127 = full lock. The route is the world record's route; only
the two marked inputs differ.

| race | what the car is doing |
|---|---|
| 0.0–1.8 | standing start, one long right-hand sweep at ~50–60 % lock |
| 1.85–2.05 | left countersteer to ~72 % to straighten out of the sweep |
| **2.90–2.93** | **① light left brush, ~7 % lock, 30 ms — on what looks like dead straight road at 157 km/h** |
| 2.9–3.3 | long descent, 100 → 183 km/h, small left trim |
| **3.35–3.38** | **② ease the left trim by ~1.5 % for 30 ms** |
| 3.55–4.1 | progressive left to full lock, held about 300 ms |
| 4.1–4.21 | unwind and climb the ramp at 177 km/h |
| 4.21–4.83 | airborne, full right lock, crossing the finish still climbing |

**① The light left brush at 2.90** — about 7 % of lock for three ticks, on top
of the world record's line. Worth 0.002 on its own (4.834 → 4.832), and it is
not a knife edge:

* **placement**: three consecutive tick offsets (2.890 / 2.900 / 2.910) all give
  4.832 — a 30 ms window, and several other placements in the same second are
  also worth −0.002
* **strength**: anything from 5 % to 11 % of lock works, flat across the band
* being outside the window costs 0 to +0.003, not a crash

**② The trim release at 3.35** — 1.5 % less left for three ticks. Worth nothing
alone and −0.001 on top of ①. Four consecutive placements across 40 ms all give
4.831.

**If you only learn one thing:** carry 4 % more left for 12 ticks from 3.470.
One input, 4.834 → 4.832, and four consecutive placements within 40 ms all give
it. That is the single most forgiving 0.002 on the map.

The unconstrained 4.830 replaces ① with a **single-tick 75 %-lock stab** and
adds two more trims that pay only at one exact tick — neighbouring placements
cost +0.004. Keep it as the floor; do not practise it.

## How forgiving it is

Both decisive inputs are 30 ms long with 30–40 ms of placement slack and a wide
band of acceptable strength. That is a practisable input, not a frame-perfect
one, and the author drove 4.831, so a human-sized path exists.

What will take real practice is not the inputs, it is **noticing**. Both trims
sit on sections that feel like nothing is happening — ① on apparently straight
road at 157 km/h, ② a 1.5 % release in the middle of a long trim. Neither is at
a corner, a landing, or anywhere a driver's attention naturally goes, and the
payoff is 15 cm at a ramp, which nothing in the cockpit shows you. A driver who
happens to brush left at 2.9 gets 4.832 and has no way of knowing which of their
thousand micro-inputs did it.

The sector numbers say the same thing. The closing jump — the dramatic part —
spreads only 0.005 across the field and correlates 0.07 with finishing order.
The stretch at 2.4–3.7, where both winning inputs sit, correlates 0.43 and 0.31.
The part of the map that looks decisive is not, and the part that decides it
looks like nothing.

That is the honest reason 903 records stopped at 4.834: not a missing skill, a
missing target. Supporting evidence that the field really is at the wall: of
every single-tick steering change to the world record's tape (5172 of them),
95.3 % are slower or fatal, 4.6 % give −0.001, and 5 give −0.002.

## Keyboard

Read off the ghosts: **ranks 7, 9 and 12 are pure keyboard runs** — three steer
values, 11–15 key presses, running 4.843 / 4.845 / 4.847. Searched in that space
directly, a keyboard tape reaches **4.834 with 18 key presses — the human world
record, on a keyboard.**

But three independent keyboard searches all stall at 4.834. **The author time
does not appear to be reachable on keyboard; 4.831 needs the analog trims**,
which is consistent with both decisive inputs being 5–7 % of lock.

## The same geometry as an official campaign map

This is a **CP1 End** cut-down of the official **Fall 2025 - 16**, whose field is
**87 596 players**. The official map's opening *is* this entire race, so all of
those players have driven exactly this sector, at full commitment, as the start
of their own lap. Grafted onto this map, all five of the official top five
return their own official CP1 splits to the millisecond: 4.951 / 4.951 / 4.962 /
4.966 / 4.932. Every one of them is slower than the runs on this page.

## Files

| file | what |
|---|---|
| `replays/m270051_human_shaped_4831.Ghost.Gbx` | **the author time, with ±10 ms of slack on every input** |
| `replays/m270051_one_input_4832.Ghost.Gbx` | one input change from the world record |
| `replays/m270051_keyboard_4834.Ghost.Gbx` | keyboard only, ties the world record |
| `replays/m270051_4830.Ghost.Gbx` | the unconstrained floor |
| `inputs/rob4_4831.json` | the human-shaped tape's per-tick inputs |
