# Welcome to wiggles — author time beaten by 3.9 seconds

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **95839 ms** | **−4945** | **−5955** |
| earlier validated tape | 96852 ms | −3932 | −4942 |
| Author time (never beaten by a human) | 100784 ms | — | −1010 |
| Human WR | 101794 ms | +1010 | — |

TMX map [197047](https://trackmania.exchange/maps/197047) · tags **Endurance,
Race, Educational** · 21 recorded runs · **100 seconds**, the longest map in
this collection.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The map

A hundred seconds of "wiggling" — a long repeated motif, which is what makes the
margin large: a technique found once applies many times over. The field wiggles
in a way that costs them a little on every repetition, and small per-repetition
gains compound across the length of the map.

Keyboard variants are included (`TAS_kbd_marched`, `TAS_kbd_metronome`) — on a
map built from a repeating motif, a metronomic input pattern is both a natural
search constraint and a natural thing for a human to practise.

## Method note

At 100 seconds this map is far too long to search end to end. The work is
per-sector, using the map's checkpoints for shaping signal, with every stage's
winner re-validated through the plain oracle before it seeds the next.

## Files

| file | what |
|---|---|
| `replays/TAS_95839_analog.Ghost.Gbx` | fastest run |
| `replays/TAS_96852_v1.Ghost.Gbx` | the first tape under the author time |
| `replays/TAS_kbd_marched.Ghost.Gbx`, `TAS_kbd_metronome.Ghost.Gbx` | keyboard-constrained variants |
| `replays/CONTROL_humanWR_101794.Ghost.Gbx` | the human world record, carried as the identity control |
| `notes/PLAN.md` | the pre-search analysis |

Work on this map is ongoing; the write-up and driving guide will follow.
