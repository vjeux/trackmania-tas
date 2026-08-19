# Fall 2025 - 18 CP1 End — author time equalled, human record beaten

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS** | **4492 ms** | **±0** | **−3** |
| TAS, single-tick variant | 4493 ms | +1 | −2 |
| Author time (never beaten by a human) | 4492 ms | — | −3 |
| Human WR | 4495 ms | +3 | — |

TMX map [270053](https://trackmania.exchange/maps/270053) · uid
`6r7HjKPCuImnLMBfqiKwWpGK1U1` · author **in-.-** · **973 recorded runs**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## What this is

The author time exactly, on a map where **973 people have tried and the best of
them is 3 ms short**. This is the most-hunted map in the collection.

Equalling rather than beating is the honest description, and it is worth
stating plainly: the tape reaches 4492, the author's own validation lap reached
4492, and nobody in 973 recorded attempts has done either.

## Validation

Re-validated through the plain oracle against the untouched map, with a
downloaded human ghost as a known-answer control in the same batch (returns
4495 exactly). Transcript in `notes/validation_transcript_v1.txt`.

## Files

| file | what |
|---|---|
| `replays/tas_4492_v1.Ghost.Gbx` | the run |
| `replays/tas_4493_singletick_v1.Ghost.Gbx` | a 4493 variant |
| `inputs/tas_4492_v1.inputs.csv` | per-tick inputs |
| `inputs/human_wr_4495.inputs.csv` | the human world record's inputs, for comparison |
| `notes/PLAN.md` | the pre-search analysis |

The human-reproducibility work on this map — where the field's 3 ms of spread
is created, and a low-input family read off the human alphabet — is still in
progress.
