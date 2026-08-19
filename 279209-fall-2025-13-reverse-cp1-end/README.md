# Fall 2025 - 13 Reverse CP1 End — author time beaten by 17 ms, and matched on a keyboard

| | time | vs AT | vs human WR | inputs |
|---|---|---|---|---|
| **TAS, unconstrained** | **6.578** | **−17** | **−26** | analog |
| TAS, 7-value action keys | 6.593 | −2 | −11 | — |
| **TAS, keyboard only** | **6.595** | **±0** | **−9** | **19 change events, 3 values** |
| Author time (never beaten by a human) | 6.595 | — | −9 | — |
| Human WR — jujumasterr | 6.604 | +9 | — | — |

TMX map [279209](https://trackmania.exchange/maps/279209) · uid
`uKd2hMaH4k0KekCMv1rZUbrKFag` · author **in-.-** · **334 recorded runs**.

The largest margin over an author time on any heavily-hunted map in this
collection.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The whole finding, in one instruction

> **On the ice run-down, about 1.7 seconds in, where the slope flattens out and
> the nose goes light — lift off the throttle for four ticks (40 ms), then back
> on. That is worth 12 milliseconds, it is the easiest input in the entire lap
> to get right, and nobody in a field of 334 does it.**

The lift has a **70 ms window of acceptable start times** — the most forgiving
input in the lap is also the decisive one. Add turning in 40 ms earlier at the
same place and the author time falls.

Everything else in the keyboard tape is what the rank-3 human already drives.

## The map

Two waypoints, no checkpoints, 6.6 s, on the ground for every tick, gas held
essentially throughout, brake almost never used. A standing start **on ice**
down a steep straight (0 → 130 km/h in 2.5 s, under 1 m of lateral movement), a
stab of full right, then **one 77 m-radius left-hand sweeper held at full lock
for 3.0 seconds** to the flag, accelerating from 161 to 212 km/h.

The finish plane is normal to x at x ≈ 1040.68 and the car meets it at 58 m/s:
**1 ms = 5.8 cm.**

## The keyboard alphabet was read off the human tapes, not guessed

`r003` is **rank 3 on the leaderboard and a pure keyboard run**: steer alphabet
exactly `{-127, 0, +127}`, **17 input change events for the whole lap**, 4 ms
off the world record. `r075` is the same shape with 14. So a three-value tape
was already within 4 ms of the best human in the world before anyone searched
anything.

r003's entire lap:

```
   30 LEFT     740 centre   800 RIGHT    900 centre  1020 RIGHT  1170 centre
 1250 RIGHT   1440 centre  1500 RIGHT   1600 centre  1730 LEFT   2410 RIGHT
 2560 centre  2790 RIGHT   3620 centre  3680 LEFT   (held to the flag)
```

Everything after 3.68 s is one held key. Across 16 sampled tapes spanning ranks
1 to 265, the brake appears in **one** tape, for six ticks.

## Validation

- **105 of 105 human ghosts** (ranks 1–45, 61–75, 101–115, 151–165, 251–265,
  spanning 6.604–7.029) re-simulate to their exact leaderboard millisecond.
- The candidate factory round-trips rank 1 to 6.604.
- **Zero failed re-validations in the whole session**, so nothing was written to
  the project's phantom directory.
- **Gate machinery identity control**: a relocatable-gate map with the item put
  back exactly where it already is reproduces 6604 / 6608 / 6655 / 6757 — the
  surgery is a no-op when it should be.
- Distinct search root per process throughout; the fork-resume and sub-tick
  plane paths were never used here, so none of the corruption defects found
  elsewhere in this project apply.

## Files

| file | what |
|---|---|
| `replays/champ_6578.Ghost.Gbx` | fastest run, unconstrained |
| `replays/kb2_best_6595.Ghost.Gbx` | **keyboard only, 19 inputs — matches the author time** |
| `replays/KB_SIMPLE_6595.Ghost.Gbx`, `kb20.Ghost.Gbx`, `kb_gasfull.Ghost.Gbx` | the rest of the low-input family |
| `notes/RESULT.md` | the full write-up, including the tolerance tables |
| `notes/NOTES.md`, `notes/PLAN.md` | instrument work and the pre-search analysis |

## This map is an Altered Nadeo copy of **Fall 2025 - 13**

Identified blind by cell occupancy against all 625 official seasonal campaign
maps — see [`_altered/`](../_altered). The official map has a field of **200 000
players** on this geometry.

This is a **Reverse** variant: same physics, but those humans drove the route **backwards**. The official field gives you geometry and a corridor, **not a line** — and nobody has yet tried reversing it into one.

*No time here is claimed from that field.* Grafting an official tape onto one of
our maps is a measured negative on 2 of 2 maps tried and undiagnosed, so "times
transfer" is a statement about physics rather than a demonstrated pipeline.
