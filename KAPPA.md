# Is each page's headline ghost actually its own run?

**Five published pages carry a recording that is not their tape's run — and the
plain oracle re-simulates every one of those tapes to the time the page claims.
The times are sound. The films show a different car.**

19 pages verify clean, 5 are refused, and 15 this check still cannot reach:
the page states no headline time, or two replays carry it, or the directory has
no replay in it at all. **An unreachable row is not a clean row.**

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
| The Magnet Trial | 793.893 | 1.000 | 793.893 | OK |
| Training 10 long | 13.071 | 0.382 | 13.071 | REFUSED: 1 check(s) failed **CARRIES ANOTHER RUN** |
| Welcome to wiggles | 95.839 | - | - | 2 replays carry 95839: TAS_95839_analog.Ghost.Gbx, gen_TAS_95839_analog.Ghost.Gbx |
| YEET Fall 2024 - 04 | 10.640 | 0.427 | 10.640 | REFUSED: 1 check(s) failed **CARRIES ANOTHER RUN** |
| Get in the hole (impossible) | 13.984 | - | - | 2 replays carry 13984: an330_13984.Ghost.Gbx, kb330_31ev_13984.Ghost.Gbx |
| Miru's Hell 2 | 18.160 | - | - | replays: No such file or directory (os error 2) |
| Fall 2024 - 25 (pure wet icy wood) | 95.575 | - | - | no replay filename carries 95575 |
| The Blev Special | ? | - | - | the page states no headline time |
| Great WTF of what #165 | 7.998 | 0.495 | 7.998 | REFUSED: 1 check(s) failed **CARRIES ANOTHER RUN** |
| Torment (1-UP) | 19.907 | 1.000 | 19.907 | OK |
| Torment (1-DOWN) | 20.237 | 1.000 | 20.237 | OK |
| [Turtle Trial] Angustus | 239.133 | 1.000 | 239.133 | OK |
| impossible at for ssano | 14.289 | 1.000 | 14.289 | OK |
| surely my least cooked at | 3.836 | 1.000 | 3.836 | OK |
| Impossible Mini Trial 2 | 21.022 | - | - | no replay filename carries 21022 |
| bald turtle #35 | 10.758 | 1.000 | 10.758 | OK |
| Fall 2025 - 16 (CP1 end) | 4.830 | 1.000 | 4.830 | OK |
| Fall 2025 - 18 (CP1 end) | 4.492 | 1.000 | 4.492 | OK |
| U10S_32 [Yeet] MAX-UP | 7.463 | 1.000 | 7.463 | OK |
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
