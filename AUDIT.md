# The 2026-08-22 ghost audit: thirteen map directories, seventy-one files

Every published `.Ghost.Gbx` in these thirteen directories, what was wrong with
it, and what was done. Nothing here is remembered: each column is a measurement,
and the command that produced it is named at the bottom.

## The columns

| column | what it is |
|---|---|
| **kappa before** | Cohen's kappa between the file's two channels of inputs — the 10 ms input chunk, and byte 14 of every 50 ms telemetry sample. A recording that belongs to its own tape scores near 1; a carrier's recording scores near 0. `ghost verify` V6 refuses below 0.60. |
| **regen moves it** | how far the record moves when it is rewritten from the engine driving the file's own tape. 0.000000 m means the positions were already the engine's. |
| **instrument zero** | how far the same operation moves a recording **the game made itself** — a downloaded human ghost on the same map. This is the regenerator's own error, and it is not 0. Mode of five runs, because the tick alignment jitters run to run. |
| **one tick** | a fifth of the median distance between two recorded samples on that map. A record is sampled every 50 ms and the physics runs at 10 ms, so a one-tick labelling error shows up as this much constant separation at zero shift, which no whole-sample shift removes. |
| **verdict** | on the game's tick, or one tick from it. Read by comparing the two columns before it: a file that the instrument does not move sits where the instrument puts things, which is the game's tick only when the instrument's zero is itself ~0. |

## The table

| map | file | published | kappa before | regen moves it (m) | instrument zero (m) | one tick (m) | verdict | repair |
|---|---|---|---|---|---|---|---|---|
| 228607 | `AUTHOR_LAP_20258_watchable` | 20.258 | - | - | 0.0005 | 0.9204 | - | untouched |
| 228607 | `FORGIVING_19948` | 19.948 | 0.358 | 0.000000 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `LOWINPUT_20070_16values` | 20.070 | 0.376 | 0.000000 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `SPLICE_24854` | 24.854 | 0.402 | 0.000000 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `TAS_19907` | 19.907 | 0.331 | 0.000000 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `TAS_19910` | 19.910 | 0.341 | 0.000000 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `TAS_19927` | 19.927 | 0.359 | 0.000039 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `TAS_19936` | 19.936 | 0.361 | 0.000039 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `TAS_20070` | 20.070 | 0.374 | 0.000000 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `TAS_20083` | 20.083 | 0.385 | 0.000039 | 0.0005 | 0.9204 | on the game tick | regen |
| 228607 | `TAS_20126` | 20.126 | 0.396 | 0.000000 | 0.0005 | 0.9204 | on the game tick | regen |
| 228811 | `TAS_20237` | 20.237 | 0.266 | 0.000000 | 0.0005 | 0.9988 | on the game tick | regen |
| 228811 | `TAS_20237_regenerated` | 20.237 | 0.266 | 0.000000 | 0.0005 | 0.9988 | on the game tick | regen |
| 249521 | `DRIVABLE_30ev_14608` | 14.608 | 0.642 | 0.098054 | 0.0005 | 0.0788 | WAS ONE TICK OFF -- corrected | regen |
| 249521 | `KEYBOARD_14349` | 14.349 | 0.629 | 0.099234 | 0.0005 | 0.0788 | WAS ONE TICK OFF -- corrected | regen |
| 249521 | `ROBUST_KEYBOARD_14479` | 14.479 | 0.632 | 0.099390 | 0.0005 | 0.0788 | WAS ONE TICK OFF -- corrected | regen |
| 249521 | `TAS_14289` | 14.289 | 0.528 | 0.098132 | 0.0005 | 0.0788 | WAS ONE TICK OFF -- corrected | regen |
| 252289 | `tas_3836` | 3.836 | 0.790 | 0.000000 | 0.0005 | 0.0808 | on the game tick | regen |
| 252289 | `tas_keyboard_3844` | 3.844 | 0.808 | 0.000000 | 0.0005 | 0.0808 | on the game tick | regen |
| 252289 | `tas_twoinputs_3848` | 3.848 | 0.824 | 0.000000 | 0.0005 | 0.0808 | on the game tick | regen |
| 267460 | `TAS_21918_analog` | 21.918 | 0.751 | 0.000000 | 0.0005 | 0.3768 | on the game tick | regen |
| 267460 | `TAS_22290_thinned` | 22.290 | 0.737 | 0.000000 | 0.0005 | 0.3768 | on the game tick | regen |
| 267460 | `TAS_22698_lowinput` | 22.698 | 0.773 | 0.000000 | 0.0005 | 0.3768 | on the game tick | regen |
| 267859 | `KEYBOARD_10788` | 10.788 | 0.491 | 0.000000 | 0.1363 | 0.1282 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 267859 | `KEYBOARD_10897` | 10.897 | 0.395 | 0.000000 | 0.1363 | 0.1282 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 267859 | `TAS_10758` | 10.758 | 0.597 | 0.000480 | 0.1363 | 0.1282 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 267859 | `TAS_10759` | 10.759 | 0.595 | 0.000477 | 0.1363 | 0.1282 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 267859 | `TAS_10768` | 10.768 | 0.597 | 0.000000 | 0.1363 | 0.1282 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 267859 | `TAS_10769` | 10.769 | 0.582 | 0.000000 | 0.1363 | 0.1282 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 267859 | `TAS_10859` | 10.859 | 0.407 | 0.000000 | 0.1363 | 0.1282 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 270051 | `m270051_4830` | 4.830 | 0.383 | 0.000000 | 0.3190 | 0.3272 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 270051 | `m270051_human_shaped_4831` | 4.831 | 0.383 | 0.000000 | 0.3190 | 0.3272 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 270051 | `m270051_keyboard_4834` | 4.834 | 0.549 | 0.000000 | 0.3190 | 0.3272 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 270051 | `m270051_one_input_4832` | 4.832 | 0.372 | 0.000000 | 0.3190 | 0.3272 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 270053 | `ablation_early_only_4493` | 4.493 | 0.764 | 0.328413 | 0.0005 | 0.3822 | WAS ONE TICK OFF -- corrected | regen |
| 270053 | `ablation_exit_only_4495` | 4.495 | 0.769 | 0.000000 | 0.0005 | 0.3822 | on the game tick | regen |
| 270053 | `tas_4492_v1` | 4.492 | 0.769 | 0.328452 | 0.0005 | 0.3822 | WAS ONE TICK OFF -- corrected | regen |
| 270053 | `tas_4493_singletick_v1` | 4.493 | 0.764 | 0.328389 | 0.0005 | 0.3822 | WAS ONE TICK OFF -- corrected | regen |
| 274191 | `KEYBOARD_4input_7514` | 7.514 | 0.374 | 0.000000 | 0.0005 | 0.7096 | on the game tick | regen |
| 274191 | `KEYBOARD_7474` | 7.474 | 0.473 | 0.000000 | 0.0005 | 0.7096 | on the game tick | regen |
| 274191 | `KEYBOARD_7476` | 7.476 | 0.473 | 0.000000 | 0.0005 | 0.7096 | on the game tick | regen |
| 274191 | `TAS_7463` | 7.463 | 0.571 | 0.000000 | 0.0005 | 0.7096 | on the game tick | regen |
| 279197 | `ACTIONKEY_5detent_10643` | 10.643 | 0.298 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `best_10596` | 10.596 | 0.240 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `best_10597` | 10.597 | 0.240 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `best_10598` | 10.598 | 0.225 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `DETENT16_10602` | 10.602 | 0.247 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `KEYBOARD_10636` | 10.636 | 0.258 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `KEYBOARD_35ev_10646` | 10.646 | 0.291 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `real_10594` | 10.594 | 0.238 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279197 | `real_10595` | 10.595 | 0.230 | 0.000000 | 0.0005 | 0.6198 | on the game tick | regen |
| 279209 | `AK5_6595` | 6.595 | 0.538 | 0.000000 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279209 | `AK7_6591` | 6.591 | 0.507 | 0.000000 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279209 | `BEST_6578_ratcheted` | 6.578 | 0.539 | 0.000000 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279209 | `champ_6578` | 6.578 | 0.504 | 0.000000 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279209 | `kb20` | (none) | 0.793 | 0.000073 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279209 | `kb2_best_6595` | 6.595 | 0.792 | 0.000000 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279209 | `kb_gasfull` | (none) | 0.793 | 0.361323 | 0.3604 | 0.4388 | on the game tick | sync-record |
| 279209 | `KB_SIMPLE_6595` | 6.595 | 0.792 | 0.000000 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279209 | `ms_r002_6608_best_6585` | (none) | 0.637 | 0.000000 | 0.3604 | 0.4388 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279218 | `best_pC_5348_32098` | 5.348 | 0.511 | 0.356144 | 0.3653 | 0.3852 | on the game tick | sync-record |
| 279218 | `best_pF_5347_32087` | 5.347 | 0.268 | 0.000000 | 0.3653 | 0.3852 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 279218 | `DRIVABLE_5351_5detents` | 5.351 | 0.360 | 0.358095 | 0.3653 | 0.3852 | on the game tick | sync-record |
| 279218 | `KEYBOARD_5350_equals_AT` | 5.350 | 0.502 | 0.357922 | 0.3653 | 0.3852 | on the game tick | sync-record |
| 279218 | `KEYBOARD_5352_11events` | 5.352 | 0.476 | 0.000000 | 0.3653 | 0.3852 | ONE TICK OFF THE GAME -- not repairable here | sync-record |
| 285268 | `HUMAN_rank2_keyboard_49491` | 49.491 | 0.863 | - | 0.5877 | 0.5952 | - | untouched |
| 285268 | `KEYBOARD_49475` | 49.475 | 0.862 | 0.586663 | 0.5877 | 0.5952 | on the game tick | sync-record |
| 285268 | `TAS_49275` | 49.275 | 0.802 | 0.586669 | 0.5877 | 0.5952 | on the game tick | sync-record |
| 285268 | `TAS_49275_independent` | 49.275 | 0.802 | 0.586640 | 0.5877 | 0.5952 | on the game tick | sync-record |
| 285268 | `TAS_49275_regenerated` | 49.275 | 0.802 | 0.586791 | 0.5877 | 0.5952 | on the game tick | sync-record |
| 285268 | `TAS_49278` | 49.278 | 0.802 | 0.000000 | 0.5877 | 0.5952 | ONE TICK OFF THE GAME -- not repairable here | sync-record |

## What each repair was

**regen** — `ghost regen`: every sample's 22 transform bytes read out of the
dedicated server's engine while it drove that file's own input tape, and bytes
14 / 15 / 18 written from the tape. Then `ghost identity set --anonymise` and
`ghost declare --from-oracle`. Used only on the eight maps whose instrument zero
is 0.0005 m.

**sync-record** — `ghost tape sync-record`: bytes 14 / 15 / 18 rewritten from the
file's own input chunk and **nothing else touched**. They are fully determined by
the tape and need no engine. Used on the five maps where the regenerator is
itself one physics tick off the game, because rewriting the transform there
replaces a correct record with a wrong one and every check stays green.

**untouched** — the two files this project publishes *as* a person's own
recording: 228607's `AUTHOR_LAP_20258_watchable` and 285268's
`HUMAN_rank2_keyboard_49491`. Carrying that person's identity and trajectory is
what the page says the file is.

## What is still wrong, and it is named rather than tuned away

**Byte 89, the ground-contact flag, is the carrier's on every file.** `ghost
regen` writes 22 of a sample's 116 bytes from the engine and three from the tape;
the other 91 — rpm, gear, wheel rotation, suspension, surface effects — are the
donor container's. Every manifest here declares it, and C5 / C6 / C7 / C10 now
report UNMEASURED with that reason instead of failing. Reading it out of engine
memory is an open task, not a conclusion: it is in there.

**C-route is unmet on all seventy-one.** Every other check reads the record, so
none of them can disagree with it about where the car was. The instrument that
would — `fk trace`, an engine trajectory produced without touching the record —
does not locate the car on these maps.

**Twenty-two files are one physics tick out of step with the run they claim**, on
the five maps where the regenerator is too. Their times are real, they
re-simulate to the millisecond in their names, and nothing in them is a
stranger's driving. But a frame-synchronous two-car comparison from those files
will show a 0.1-0.6 m phantom gap, which is the defect that took 270053's clip
down. Each page names its own.

**The record node's declared span outlives the run on twenty files**, by 0.5 to
5.0 s — the donor container's `end_ms`, with the donor's non-vehicle entities
still inside at full length. `ghost record shorten` fixes exactly this without
touching a trajectory; it was not on `main` when this pass ran.

| map | file | run | record span declares |
|---|---|---|---|
| 228607 | `FORGIVING_19948` | 19.948 | 24.900 |
| 228607 | `LOWINPUT_20070_16values` | 20.070 | 24.900 |
| 228607 | `TAS_19907` | 19.907 | 24.900 |
| 228607 | `TAS_19910` | 19.910 | 24.900 |
| 228607 | `TAS_19927` | 19.927 | 24.900 |
| 228607 | `TAS_19936` | 19.936 | 24.900 |
| 228607 | `TAS_20070` | 20.070 | 24.900 |
| 228607 | `TAS_20083` | 20.083 | 24.900 |
| 228607 | `TAS_20126` | 20.126 | 24.900 |
| 228811 | `TAS_20237` | 20.237 | 22.670 |
| 249521 | `KEYBOARD_14349` | 14.349 | 15.000 |
| 249521 | `ROBUST_KEYBOARD_14479` | 14.479 | 15.000 |
| 249521 | `TAS_14289` | 14.289 | 15.000 |
| 267460 | `TAS_21918_analog` | 21.918 | 23.050 |
| 267460 | `TAS_22290_thinned` | 22.290 | 23.050 |
| 267859 | `KEYBOARD_10788` | 10.788 | 11.450 |
| 267859 | `TAS_10758` | 10.758 | 11.450 |
| 267859 | `TAS_10759` | 10.759 | 11.450 |
| 267859 | `TAS_10768` | 10.768 | 11.450 |
| 267859 | `TAS_10769` | 10.769 | 11.450 |

## The commands

```
tmtraj gate FILE --race MS --refs refs.tsv --mapid ID --server DIR --map M --require-manifest
ghost verify FILE --map M --server DIR          # V1-V10
ghost phase --map M --control <a human download of this map> --runs 5 [FILE...]
ghost trajdiff A B                              # at every shift from -3 to +3 samples
tmtraj corpus dup --root .                      # two files of one map with the same recorded motion
tmtraj tapediff A B                             # every tick two tapes ask for different inputs
ghost census FILE --expect-ms MS --other MS,... # every millisecond stored as a time
```

The reference table is every human recording this project holds for each of the
thirteen maps: 434 downloads over 13 maps. Where a map's only reference is one
file, the contamination test says so rather than passing.
