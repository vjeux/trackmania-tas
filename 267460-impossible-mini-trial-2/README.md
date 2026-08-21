# Impossible Mini Trial 2

**Our best run is 21.022 — 2.046 faster than the only human who has ever driven
this map. Do not brake onto the finish platform. The record holder is on the brake from
20.0 s and crosses the line at 8.5 km/h, taking 0.829 for the last ten metres;
carrying speed instead takes 0.258, and that decision alone is worth more than
half a second.**

**Video: withdrawn, and a replacement is blocked.** The clip published here was
filmed from `TAS_21918_analog`,
whose telemetry is its carrier's rather than its own — measured against a live
re-simulation of its own inputs, it sits a median of 17.05 m and a maximum of
53.19 m from where the car actually goes. A regenerated file sits 0.000523 m
away. The run and the time are unaffected; the video was of the wrong car's
path.

A regenerated ghost of the 21.022 now exists and its positions are right — median
0.5 mm against a live re-simulation — but the render gate still refuses it: the
ground-contact byte is the carrier's, so contact reads ON for 15 of 39 provably
airborne samples. The map's own downloaded human recording passes all ten checks,
which is what makes that a real defect here rather than a false alarm. A record-
internal rule (contact iff min(dampen) < 0.057) scores 96.97 % against a 72.51 %
baseline on this map — eleven times better than the best engine slot found
elsewhere — but it does not transfer, going negative on two of four other maps,
so it is recorded as a bounded negative rather than shipped.

| run | time | vs human WR | vs author time | steer values | input events |
|---|---|---|---|---|---|
| **TAS** | **21.022** | **−2.046** | +4.134 | — | — |
| TAS, earlier | 21.024 | −2.044 | +4.136 | — | — |
| TAS, earlier | 21.090 | −1.978 | +4.202 | — | — |
| TAS, earlier | 21.417 | −1.651 | +4.529 | — | — |
| TAS, earlier | 21.652 | −1.416 | +4.764 | — | — |
| TAS, analog (previously published as best) | 21.918 | −1.150 | +5.030 | 214 | 515 |
| TAS, thinned | 22.290 | −0.778 | +5.402 | 31 | 84 |
| TAS, low-input | 22.698 | −0.370 | +5.810 | **10** | **78** |
| Author time | 16.888 | −6.180 | — | — | — |
| Human WR — Wirtual | 23.068 | — | +6.180 | 3 | 87 |

TMX map [267460](https://trackmania.exchange/maps/267460) · author
**Mattlightning** · **exactly one recorded run**.

**The author time does not fall here.** 16.888 is more than four seconds faster
than anything that can be built on this route, and where those five seconds
would come from is an open question — see the end of this page.

## It is not a respawn map

The obvious read of a 23-second run on a map called *Mini Trial* is that most of
it is failed attempts. It is not: the map has **one checkpoint**, which is the
finish itself, so there is nowhere to respawn *to* except the start line with the
clock running. The world record contains no respawns at all. "Trial" here is a
building style — small floating platforms — not a checkpoint mechanic.

## What the map is, and where the time goes

Twenty-two of the map's thirty-one blocks are big flat stadium screens rotated
vertical into **two solid walls**, one behind the start and one between the
flight and the finish. Every route question on this map is "which hole in which
wall", and the answer is that there is only one of each. **The route is forced;
there is no secret line.**

The world record, gate by gate, with our run alongside:

| point on the route | human | ours |
|---|---|---|
| flat out west on the ice, 167 km/h | 1.985 | 1.985 |
| off the west end, airborne | 3.946 | 3.946 |
| bottom of the pit, 69 km/h | 5.979 | 5.979 |
| top of the climb back out | 9.825 | — |
| back down, charging east | 12.969 | 12.969 |
| **through the big turbo gate** | 15.239 | 15.239 |
| mid-dive, 257 km/h | 18.018 | 18.015 |
| on the finish platform | 22.239 | **21.660** |
| finish | 23.068 | **21.918** |

**Nine of the twenty-three seconds are the pit** — 151 m at 45–100 km/h on
30°-rolled dirt, dropping in and climbing back out. **Four more are the
endgame.** Everything we found is after 18.0 s, and it splits into two almost
equal halves: 0.576 in a tighter landing and turn-around, and **0.574 in the
final ten metres.**

## The run, sector by sector

1. **The ice, west (0 → 3.9 s).** Flat out to the west end and off it. You have
   to go west past the wall's edge — that is the only gap — before you can get
   anywhere.
2. **The pit (3.9 → 12.9 s).** Unavoidable, and nine seconds long. Drop into the
   tilted dirt cluster, cross the bottom, climb back out and come east onto the
   run-up platform. It is the largest single block of time on the map and it is
   the least explored.
3. **The turbo gate and the dive (15.2 → 19.0 s).** Ballistic. Nothing you do in
   the air changes where you land. **Do not try to steer toward the flag** — you
   can see it out of the window and you cannot reach it; the screen is in the
   way, and the only doorway through that wall puts you past the flag and four
   metres below it.
4. **The landing (about 19.0 s).** The record lands still pointed east, runs on
   to the far end of the grass and turns around there. **That U-turn is about
   half a second.** Land already turning.
5. **The last ten metres — this is the whole map.** The engine dies as you cross
   the no-engine gate, so **every km/h you brake away before that gate is gone
   for good**: speed is the only thing you can still spend on the far side.
   Carry it through the 32 m gap jump up onto the finish platform, thread the
   four pillars at speed, and let the flag stop you. The record brakes from
   20.0 s, holds it through the jump and the pillars, and arrives *into* the flag
   structure at 8.5 km/h rather than through it — 0.829 for ten metres against
   our 0.258.

## How forgiving it is

Per-input timing slack has not been measured on this map, so there is no honest
table to give. What can be said:

- **The one thing that matters is a decision, not a timing** — whether you brake
  onto the finish platform. It needs no tape at all and it is worth roughly half
  a second.
- **The line simplifies a very long way.** The fastest tape is per-tick noise no
  person could reproduce, but deleting 433 of its 515 input changes costs
  nothing at all; the real structure is about eighty held segments. The
  low-input version gets within **0.370** of the world record on ten steering
  values, against the record holder's three. That part is teachable.
- The pit is where the remaining time must be, and it repays practice more than
  anything else on the map.

## Where the missing five seconds are not

The author time needs another five seconds, and five seconds here means a
different route, not a better line. Three candidate routes were measured and
closed:

- **Flying through the flag mid-dive.** At 18.018 the car is level with the flag
  and 56 m adrift of it in the wrong axis, with a solid screen between. Tapes do
  get through the low doorway in that wall, but the doorway is east of the flag
  and four metres below the platform, so they arrive already past it.
- **Dropping straight off the start platform into the turbo gate.** It is 70 m
  from the spawn and behind the near screen. Nothing reaches it.
- **Landing on the dirt slope north of the finish platform.** Tapes reach the
  slope in quantity, but the strip between its south edge and the platform is
  void and nothing crosses the last 16 m.
- **Launching upward** over the far wall is closed too: the flat ramp does not
  produce upward velocity.

The best construction anyone has assembled out of a launch, a flight and an
endgame is about 21.3, against a best actual of 21.918. So either there is a
route nobody has found, or the author time was not driven. The map carries no
author ghost of any kind, and with a single human record there is no field to
cross-check it against.

## Files

| file | what |
|---|---|
| `replays/TAS_21918_analog.Ghost.Gbx` | the fastest run |
| `replays/TAS_22290_thinned.Ghost.Gbx` | the same line at 84 input changes |
| `replays/TAS_22698_lowinput.Ghost.Gbx` | **ten steering values — the one worth studying** |
| `inputs/m267460_TAS_lowinput_76inputs.script.txt` | the low-input run as a readable script |
| `inputs/m267460_TAS_thinned_82inputs.script.txt` | the thinned run |
