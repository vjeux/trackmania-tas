# 156451 "Snowcarmc free 42" — structure, field, and what the author time is worth

Arm `snow`, node 74761, 2026-08-20. Store files all prefixed `snow_`.
Map md5 `eec9f5730e9481d72361066c8d84c60c`, sha256
`28098514cf251e55bd46bc5434d6a77522449f4077efd59148a90e8a93e9a947` —
**the trackmania.exchange copy and the Nadeo CDN copy are byte-identical**
(`snow_MAP_156451_nadeo_v1.Map.Gbx`). uid `7jgWLFiAFRQ09bAnRA6DeGFnm_e`,
TMX 156451, Nadeo mapId `80422065-1f51-434a-afc8-4f91726e0f6f`.

## THE HEADLINE: the AT is soft by a factor of two and every human beats it

Author time **40.074**. The 63-record leaderboard runs **18.810 … 21.830**.
The author, BapdadaTM, is **rank 12 on his own map at 19.610** — less than half
his own author time. Nothing on this map has to be invented to beat the AT; the
question worth answering here is how far under the human WR a TAS can go.

## Structure — no checkpoints, one finish, and the route is enforced by geometry

`tmmaps census` (a subcommand added for this arm, banked in
`snow_tmmaps_census_patch_v1.rs`):

* **610 blocks: 521 `DecoPlatformIceBase`, 84+4 `DecoCliffIce…`, 1
  `GateGameplaySnow`.** All at cell y=9. That is a flat ice base with a cliff
  perimeter, x cells 23..42, z cells 16..41.
* **103 items: 92 `SnowGateGameplay`** (a curtain at y=26 spanning x 0..1376,
  z 512..1340 — this is where the snow car comes from; the header's
  `<playermodel id=""/>` is empty) **and 11 `DirtHill\…` pieces**, which ARE the
  course:

```
DH-Waypoint\DH-Start1        992,16,576     (Spawn)
DH-Transitions\DH-Transi6.b 1024,16,639
DH-Canyon\DHC-Turns\DHC-Turn4   1056,24,703
DH-Canyon\DHC-Turns\DHC-Turn3.c  896,24,831
DH-Canyon\DHC-Transitions\DHC-Transi4.c 928,24,831
DH-Turns\DH-UTurn1           992,32,863
DH-Canyon\DHC-Turns\DHC-Turn3.c 960,32,864
DH-Transitions\DH-UptoFlatR  992,32,864
DH-Turns\DH-UTurn2          1120,40,864
DH-Transitions\DH-Transi6.b 1152,40,864
DH-Waypoint\DH-Finish2      1120,48,800     (Goal)
```

* **`NbCheckpoints` = 0.** Exactly one Spawn and one Goal; nothing enforces the
  route. But unlike 276877, that does not make the AT cuttable: the course is a
  **terraced hill climb** (the car's y goes 21 → 26 → 34 → 42 → 50), the finish
  sits on the top terrace, and the only connections between terraces are the two
  U-turn ramps. The ice base is ~40 m below the finish. Say "the AT is not
  route-enforced"; do **not** claim a cut exists — none was found and the
  geometry argues against one.
* `validated="1"` but **there is no embedded author ghost**: no
  `CPlugEntRecordData` (0x0911F000) and no `0x0309201D` / `0x0309202B` anywhere
  in the decompressed body. The 228607 shortcut is unavailable here.

## The finish is a plane at z = 787.65, measured off 63 runs

Extrapolating each ghost's last telemetry sample by its own velocity to its own
declared finish time (ACQUISITION §5): **all 63 cross at z = 787.6 ± 0.1 m**
(the few outliers are slow-moving runs where a 50 ms extrapolation is coarse),
travelling −z at ~40 m/s, at y = 50.17, with x spread **1133.6 … 1141.8**.
So: the trigger is a plane in z, its lateral aperture is at least 8 m wide, and
**1 ms is worth 4 cm** here.

## The field: speed-saturated, full throttle, one line

* **Not one brake input in the entire field of 63 runs, and no throttle lift in
  the top 6.** Path length 714–769 m at 126–166 km/h; time is essentially path ÷
  ~38 m/s. WR = 716.2 m / 18.810 s.
* Lateral spread is tiny: mean pairwise RMS separation **2.56 m** (max 4.48),
  i.e. one line, a narrow road. The WR is **not** central (rank 49 of 63 by
  centrality) and its nearest neighbour is rank 2 at 0.54 m.
* The WR wins on **speed, not geometry**: +4 to +9 km/h over the field median
  through the whole second half.
* **Best-sector splice = 18.528 s** (20 equal-arclength stations, forward-only
  projection, worst station miss 12.9 m, sectors excluded where a run's station
  miss > 12 m). A bound, not a lap. rank 6 (19.093) owns six of the twenty
  sectors including the last by 56.8 ms.

## §8 field reproduction: the map is FAITHFUL, and the failures are build-correlated

63 ghosts re-simulated on the unmodified map: 44 exact, 10 different, 9 DNF.
Split by the game build the record was set on (`strings … | grep date=`):

| build | exact | diff | DNF |
|---|---|---|---|
| 2026-02-02 `git128149` | **9** | 0 | 0 |
| (no build string) | **3** | 0 | 0 |
| 2025-07-04 | 7 | 5 | 1 |
| 2024-12-12 | 9 | 5 | 5 |
| 2024-01…09 | 10 | 5 | 3 |

**12 of 12 for every record set on the current build, the WR included, to the
millisecond.** Our oracle's own build is `2026-05-15 git128182`. The snow car's
physics changed between 2024 and now; this is the opposite polarity to 203072's
failure and it is **not** a stop — the physics we optimise against is the
physics the recent records were set under. §8 should be read **per build**, not
as a single percentage: a raw 70 % here would have condemned a healthy map.

## The start trick is NOT AVAILABLE on this map — tick 0 is inert

`start_offset_ms = 0`, gas on at every one of the 1882 ticks. Plain-oracle
sweep (`stx eval`, one candidate per row, identity included):

| edit | result |
|---|---|
| identity | 18743 |
| gas off at race 0.000 (**the trick**) | **18743 — bit-for-bit the same time** |
| full steer at 0.000 | 18743 |
| brake on at 0.000 | 18743 |
| gas off at 0.010 | DNF |
| full steer at 0.010 | DNF |
| brake on at 0.010 | DNF |
| gas off at 0.020 / 0.030 | DNF |

The tick-1 row **is** the yes-control: the same three edits one tick later are
each decisive, so delivery works and the tick-0 slot has no effect. On this map
the first packet is consumed before the first simulated step, so "start on the
second tick" is already the only thing you can do. No paired re-search was
needed and none was run.

## Toolchain on this node (measured, not remembered)

`/tmp/fk/rs` = `reliability.tgz` + `fk-hardened`'s `pred_core.rs` +
`tg_tools_v3_final` + `tail_tools_v1` + `nan_tools_v1` (via nan's `build.sh`),
then **v6.6** `tmsearch`/`tmmaps` over it, plus `u02` and `stx` added as
workspace members. `FINISH_BASE = 1_000_000_000_000` in both `main.rs` and
`forksearch.rs`. `readlink -f /tmp/fk/rs` = `/tmp/fk/rs` (no symlink).
Two files v6.6 needs but does not ship: `tmsearch/src/carmodel.rs` (taken from
`tmtas-rs.tgz`'s tree) and `src/bin/ph{diag,repro}.rs` (from `phantoms.tgz`);
its `Cargo.toml` path-depends on the literal `/tmp/fk-hard/fkdrv`, repointed to
`../fkdrv`. `tmtas selftest` 10/10.

**`fk regen` works on this map** — answer-key control (regenerate the downloaded
WR's own telemetry and compare with its own bytes): 377/377 samples, POSITION
median **0.000000 m**, max **0.000137 m**, ORIENT median 0.0055°.

**Fork-mode search is not worth it here** (measured): `--fork` at tick 1000 with
`FK_ANCHOR` set costs **165 ms/candidate/worker** against the plain batched
oracle's **~70 ms**, because an 18.8 s run is short enough that batching 60
candidates per server invocation beats forking a 150 MB address space per
candidate. Without `FK_ANCHOR` every worker aborts with "state not located".
