# Training - 10 Long

**Stop sawing at the wheel over the last 1.3 seconds, and clip booster 3 about a
metre tighter. That is the whole map — and it is enough to beat an author time
856 people have chased, on a keyboard.**

**Training 10 long** — TAS **13.071** (−0.010) | AT 13.081 | WR 13.081

https://github.com/user-attachments/assets/486d39eb-0222-4dee-a6a4-4854a5a4c3bb

Both cars are on screen: eight milliseconds apart, but the two drivers take genuinely different lines and only converge at the finish, so both are clearly visible.

| run | time | vs author time | vs human WR |
|---|---|---|---|
| **TAS, unconstrained** | **13.071** | **−0.009** | **−0.010** |
| TAS, 5-level pad | 13.074 | −0.006 | −0.007 |
| **TAS, keyboard only** | **13.075** | **−0.005** | **−0.006** |
| Author time | 13.080 | — | −0.001 |
| Human WR — in-.- | 13.081 | +0.001 | — |

TMX map [191465](https://trackmania.exchange/maps/191465) · author **in-.-** ·
**856 recorded runs**.

The author time is the author's own editor validation lap, and their best public
attempt sits 0.001 behind it. Nobody has ever got past it.

## Where the time is

Same route, same lanes, no air phase anywhere on the map. The gain is two
things, and neither of them is a line nobody knows:

- **≈0.0013** — clipping **booster 3 about one metre tighter** than any of the
  14 measured human runs.
- **≈0.0049** — being **quieter on the wheel through the last 448 m**. The human
  field puts in *eight full-lock corrections in the final 1.3 seconds*. This run
  does not.

So this is a discipline problem, not a secret. It is also why a keyboard tape can
do it: **the time was never hiding in analog resolution.** A 5-level pad tape
matches 13.074 and the pure keyboard tape — steering only ever `{−127, 0, +127}`
— is one millisecond behind it. It is hiding in *what* you steer, not how finely.
The unconstrained floor is 13.071, and those last three milliseconds are the only
thing analog buys here.

For scale on the margins: at the finish speed of 858 km/h, **1 ms is 24 cm of
travel**.

## The run as inputs

The sector-by-sector guide for this map is not written yet, and neither is a
per-input slack table — study the input scripts below against the world record's
own, which is included for exactly that comparison.

## Files

| file | what |
|---|---|
| `replays/WIP_keyboard.Ghost.Gbx` | **13.075, keyboard only** — the one worth studying |
| `replays/WIP_pad5.Ghost.Gbx` | 13.074, steering in `{−127, −64, 0, 64, 127}` |
| `replays/TAS_13074_analog.Ghost.Gbx` | 13.074, unconstrained |
| `replays/TAS_13071_analog.Ghost.Gbx` | 13.071, the fastest tape |
| `inputs/TAS_13071_analog.inputs.tsv` | per-tick inputs for the fastest run |
| `inputs/TAS_13074_analog.inputs.tsv` | per-tick inputs for the 13.074 analog run |
| `inputs/human_WR_13081.inputs.tsv` | the world record's inputs, for comparison |
