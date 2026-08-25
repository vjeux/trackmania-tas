# Is each page's headline ghost actually its own run?

**Five published pages carry a recording that is not their tape's run — and the
plain oracle re-simulates every one of those tapes to the time the page claims.
The times are sound. The films show a different car.**

19 pages verify clean, 5 are refused, and 15 this check still cannot reach:
the page states no headline time, or two replays carry it, or the directory has
no replay in it at all. **An unreachable row is not a clean row.**

> **2026-08-24, hand-edited row:** `Training 10 long` was repaired — its page now
> publishes a regenerated 13.070 that reads kappa 1.000 — so the refused count
> above is 4, not 5, until the command below is re-run and regenerates this
> table. Nothing else in it has been touched by hand.

Reproduce with `clip inventory --root . --verify [--markdown]`. It finds each
page's headline replay by the milliseconds in its filename, runs `ghost verify`
against the map on the shared store, and reports V6 (chance-corrected agreement
between the tape and the recording in the same file) beside V7 (the plain
dedicated server re-simulating the WRITTEN file).

**Read the two columns together.** `kappa 0.151` with `oracle 22.072` means: the
input tape really does produce 22.072 on the game's own engine, and the
trajectory stored beside it belongs to somebody else's run. That is a filming
defect, not a result defect. `kappa 1.000` means the recording is the tape's own
run and the page can be filmed as it stands.

The fix for a foreign recording is `ghost regen`, which rebuilds the telemetry
from the tape on the live engine. Three were repaired that way on 2026-08-22
(0.151 → 1.000 on Tap water 01, 0.500 → 1.000, 0.521 → 1.000); Kacky Reloaded
#290 is the first published from a repaired file.

**untitled 01 and untitled 02 came off the "cannot reach" list on 2026-08-23**:
both pages had no `replays/` directory, so the check had nothing to open. Each
now carries the ghost its clip was shot from, regenerated from the tape the page
publishes and verified V1–V11 clean. Their tapes were identified by SIMULATION —
every stored file for those maps declares the search container's 29.286 in its
header, and the oracle finishes exactly one of them at the page's time.

| map | TAS | kappa (V6) | oracle (V7) | verdict |
|---|---|---|---|---|
| Kacky Reloaded #290 | 23.416 | 1.000 | 23.416 | OK |
| KEKL- SAUSAGE ICE | 67.200 | 1.000 | 67.200 | OK |
| unluckE - get jiggy with it | 6.342 | 1.000 | 6.342 | OK |
| Spaghetti Nights 2 | ? | - | - | the page states no headline time |
| P-Found - Pokeuuu | ? | - | - | the page states no headline time |
| idm ruinin ur day #460 | 15.217 | 0.500 | 15.217 | REFUSED: 1 check(s) failed **CARRIES ANOTHER RUN** |
| Tap water 01 | 22.072 | 0.151 | 22.072 | REFUSED: 1 check(s) failed **CARRIES ANOTHER RUN** |
| Spring 2023 - 15 (Underwater) | 36.049 | - | - | no map on the store at /var/svcscm/persistent/private-30d/tm-unbeaten/173691 |
| [object Object] | 793.893 | 1.000 | 793.893 | OK |
| Training 10 long | 13.070 | 1.000 | 13.070 | OK — the published run is now `TAS_13070_analog`, regenerated; the 13.071 beside it still reads 0.382 and must not be filmed |
| Welcome☺to wiggles | 95.839 | - | - | 2 replays carry 95839: TAS_95839_analog.Ghost.Gbx, gen_TAS_95839_analog.Ghost.Gbx |
| YEET Fall 2024 - 04 | 10.640 | 0.427 | 10.640 | REFUSED: 1 check(s) failed **CARRIES ANOTHER RUN** |
| Get in the Hole | 13.984 | - | - | 2 replays carry 13984: an330_13984.Ghost.Gbx, kb330_31ev_13984.Ghost.Gbx |
| Miru's Hell 2 | 18.160 | - | - | replays: No such file or directory (os error 2) |
| Fall 2024 - 25 (pure wet icy wood) | 95.575 | - | - | no replay filename carries 95575 |
| The Blev Special | ? | - | - | the page states no headline time |
| Great WTF of what #165 | 7.998 | 0.495 | 7.998 | REFUSED: 1 check(s) failed **CARRIES ANOTHER RUN** |
| Fall 2024 - 08 Torment (1-UP)(ft' Emelius) | 19.907 | 1.000 | 19.907 | OK |
| Fall 2024 - 08 Torment (1-DOWN) | 20.237 | 1.000 | 20.237 | OK |
| [Turtle Trial] Angustus | 239.133 | 1.000 | 239.133 | OK |
| impossible at for ssano | 14.289 | 1.000 | 14.289 | OK |
| surely my least cooked at | 3.836 | 1.000 | 3.836 | OK |
| Impossible Mini Trial 2 | 21.022 | - | - | no replay filename carries 21022 |
| bald turtle #35 | 10.758 | 1.000 | 10.758 | OK |
| Fall 2025 - 16 (CP1 end) | 4.830 | 1.000 | 4.830 | OK |
| Fall 2025 - 18 (CP1 end) | 4.492 | 1.000 | 4.492 | OK |
| U10S_32 By Everios96 [Yeet] MAX-UP | 7.463 | 1.000 | 7.463 | OK |
| untitled 01 | 12.759 | 1.000 | 12.759 | OK |
| untitled 02 | 9.415 | 1.000 | 9.415 | OK |
| Fall 2025 - 01 Reverse (CP1 end) | 10.594 | 1.000 | 10.594 | OK |
| Fall 2025 - 13 Reverse (CP1 end) | 6.578 | - | - | 2 replays carry 6578: BEST_6578_ratcheted.Ghost.Gbx, champ_6578.Ghost.Gbx |
| Fall 2025 - 22 Reverse (CP1 end) | 5.352 | 1.000 | 5.352 | OK |
| You love water | ? | - | - | the page states no headline time |
| Pain ft Mango & Teuflum | 49.275 | 1.000 | 49.275 | OK |
| finish is on the roof to your right | ? | - | - | the page states no headline time |
| [Turtle Trial] Leto | 218.812 | 1.000 | 218.812 | OK |

37 pages: 19 whose recording IS their tape's run, 5 carrying another run, 13 not checkable.

## The rows that say nothing

`no replay filename carries <ms>` and `N replays carry <ms>` are this tool
declining to guess: the mapping from a page's headline time to the file that
produced it lives only in the filename, and where that is ambiguous, verifying
the wrong file would give a confident answer about a file nobody publishes.
`replays: No such file` is a page with no replays directory at all. None of
those rows is evidence that anything is wrong — they are pages this check
cannot reach, and each one needs a human to name the file.

## The three rows this check could not reach, measured by hand (2026-08-24)

`finish is on the roof to your right`, `YOU LOVE WATER` and `The Blev Special`
read *"the page states no headline time"* above, because their pages had no clip
and no headline line for the parser to find. Run `ghost verify` on the file each
page publishes and they are not blank rows at all:

| map | file | kappa (V6) | oracle (V7) | verdict |
|---|---|---|---|---|
| finish is on the roof to your right | `TAS_50229` | **0.342** | 50.229 | **carries another run** — and it kills the game client on import |
| finish is on the roof to your right | `POKE_1input_50659` | **0.343** | 50.659 | as above |
| finish is on the roof to your right | `TRIGGERPOKE_50469` | **0.346** | 50.469 | as above |
| YOU LOVE WATER | `TAS_97325` | **0.499** | 97.325 | **carries another run**; imports fine |
| The Blev Special | `TAS_57482` | 1.000 | 57.482 | OK |

Both pages now publish a repaired file beside the original and their clips are
shot from it: `285885/replays/TAS_50229_shootable.Ghost.Gbx` (regenerated, then
`graft-scene` for the import crash) and
`284238/replays/TAS_97325_carrier.Ghost.Gbx` (regenerated with `--carrier
layout`). Both verify V1–V11 clean at kappa 1.000.

**So five of the "not checkable" rows were hiding four more foreign
recordings.** A page with no headline time is not a page with nothing to check —
it is a page this tool cannot find the file for, and the files were there.
