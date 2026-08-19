# What is in this directory, and how to re-verify it from scratch

Map **186935** `[object Object]` (a magnet trial by Taxonomon), uid
`sOIkPZULktmoT_OoFbT4HlVxpOe`. Author time **2540.641**, best human 2575.154,
**our best 793.893**. Read `RESULTS_186935_v1.md` then `_v2.md`.

**This directory is self-sufficient: the map is here.** `MANIFEST.md5` (v1) had
no map entry, which was a real gap — `MANIFEST_v2.md5` supersedes it and covers
the map, the tapes, all seven human ghosts, the tools and the docs. The v1
manifest is left in place, not deleted.

## Re-verify with two commands, using nothing outside this directory

```bash
md5sum -c <(grep -v '^#\|^sha256\|^$' MANIFEST_v2.md5)
tmtas validate --map "$PWD/map/map186935_sOIkPZULktmoT_OoFbT4HlVxpOe_v1.Map.Gbx" \
               --jobs 5 "$PWD"/tapes/*.Ghost.Gbx
```

Run 2026-08-19 with both paths inside the store, nothing from `/tmp`:

```
CUT_186935_795034.Ghost.Gbx                  795034     the retry cut
LOWINPUT_186935_ev16396_793893.Ghost.Gbx     793893     best; 16397 input events
ONE_ATTEMPT_A_2501894.Ghost.Gbx             2501894     keby minus ONE fall
ONE_ATTEMPT_B_2503644.Ghost.Gbx             2503644     keby minus one other fall
rank00001_2575154.Ghost.Gbx                 2575154     KNOWN-ANSWER CONTROL
```

**`tmtas validate` needs ABSOLUTE paths** — a relative path yields a dangling
symlink, an empty table and no error.

Map provenance: fetched from Nadeo's public
`core.trackmania.nadeo.live/maps/f94c3d93-c41c-4be9-9229-92990e156ea7/file`,
5 036 234 bytes, sha256
`857056bc019318d3b5e47a6de64c93985dbfe5083f1825dcc534120bbe7e488d`, which is
also recorded in `analysis/map.sha256` and in `MANIFEST_v2.md5`.

## Contents

| path | what |
|---|---|
| `map/` | the `.Map.Gbx`. **Every tape here is meaningless without it.** |
| `tapes/` | the results, plus the human record they were cut from |
| `ghosts/` | all 7 leaderboard records — the field, and the known-answer controls |
| `analysis/` | PLAN.md, splits, respawn ticks, sector/obstacle tables, probe and ddmin logs |
| `tools/` | `m935.rs` (sector, obstacle, cut-plan analysis), `mspl.rs` (segment reassembly), `mq.rs` (ladder quantiser + identity control) |

Note the tools are single files from a forked `tmtas-rs-hardened` + fleet-v5
workspace with `FINISH_BASE` patched to `1e12` (this map has 17 waypoints);
they are not standalone crates.
