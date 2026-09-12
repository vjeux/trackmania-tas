# Eras: builds, archives, and what changes between them

Which game build wrote a file, which server can re-simulate it, and how the
formats moved. Sources: `OLDBUILD.md`, `BUILD-ID.md`, `tools/dsprobe`,
`tools/chunkswap`, `tools/strpatch`, memories `tm2020-old-client-archives.md`,
`tm-hpl-pack-crypto.md`. Confidence **[VERIFIED-GAME]** for every server
result, **[FILE]** for the format observations.

## 1. Reading a file's era offline

* A ghost's recording build is in chunk `0x0309202D`:
  `Trackmania date=2026-05-15_18_00 git=128182-… GameVersion=3.3.0`
  (`chunkswap --show FILE 0x0309202D | grep date=`); the server echoes it as
  `GameBuild`. Across 685 held files: 457 are build 128149, all but four are
  126529 or later (2024+); KEKL- SAUSAGE ICE's records carry 113150 (2022),
  one Spring 2023 file 120733.
* A map's or replay's header XML carries `exever="3.3.0"
  exebuild="2026-04-28_18_20"` in the CLEAR — bucket a corpus with no
  decompression.
* Format versions that move with the era: input archive `formatVersion` 11
  (33-bit state literal) → 12 (34-bit) on 2026 files; `CPlugEntRecordData`
  version 11 since 2023; `CSceneVehicleVis` archiver version 30 (103-byte
  samples) → 33 (116 bytes); the ghost's constant `0x0303F006` blob is a
  constant OF THE BUILD; `CGameItemModel` `0x2E00201F` v13 in the current pack
  (GBX.NET documents to 12); `CPlugSolid2Model` v34; `CPlugSurface` mesh v6/7.

## 2. Dedicated-server archives

`http://files.v04.maniaplanet.com/server/TrackmaniaServer_YYYY-MM-DD.zip`: a
HEAD sweep of every date 2018–2026 (3 287 names, `tools/dsprobe`) finds **41
builds from 2020-07-01 to 2022-06-21**, plus `_Latest.zip` (2026-05-15 as of
this writing). Nothing dated between 2022-06-21 and today — the build that
changed the physics after 2022 cannot be bisected from this archive. The 2020-07-01
build prints a flat schema and segfaults on every replay recorded 2020-07-07+;
2020-10-02/10-12/11-04/11-16 ship the SAME exe.

## 3. Physics do not travel across eras

* The 2022-06-21 server re-simulates the 2022 world record of KEKL- SAUSAGE ICE
  (63.546) to the millisecond and to all five splits where the current server
  DNFs it; today's tapes (68.442, our 67.200) die inside the first 13.5 s under
  2022 physics — a symmetric incompatibility, at the lap's first big slide
  (3.99 s divergence measured by `ksi2`).
* The binding is chunk `0x0309202D`, not the build stamp: `strpatch` rewrote
  the stamp both ways and nothing moved; moving `0x0309201D` + `0x0309202D`
  together lets a tape run in any container (ten gates, ten exact splits).
* An EXECUTABLE-only change (2021-05-31 → 2021-06-08, physics packs
  byte-identical) moved 74 % of common completions and turned 468 of 727
  finishes into `wrong simu`; a pack change (2020-07-23 → 2020-09-11) moved no
  time and flipped 6 runs `wrong simu` → finish. "Exe-only cannot move
  physics" is false; behaviour boundaries are not time boundaries.
* The author time of a 2022 map is still 4.6 s below the best sector
  combination anyone drove ON that build; the old server makes the question
  askable under the right physics, it does not answer it (`OLDBUILD.md` §8).

## 4. Client archives (`archive.org/details/tm2020-archive`, 32 clients)

A 252-day hole `2022-01-21 → 2022-09-30`; labels are CAPTURE dates, not build
dates ("2024-02-26" ships post-February Snow data; "2024-04-30" is a staged
May-22 build with the whole Desert car); `Trackmania Chinese/Trackmania_Setup.exe`
wraps a complete 2020-10-02 client. The 2024-01-09 boundary is
executable-only for Snow/Rally; pack FILES change at every boundary, so diff
INSIDE the pack (`pak-nadeopak.md` §7). The Feb-2024 Snow change touched the
`SnowCar.Shape.Gbx`, the Snow `VehiclePhyModel` and the Tunings (chunk
`0x090ED0A5` v1 → v2 drops nine `Keys` curves + six ints).

## 5. What the era does NOT change

The container grammar (`gbx-container.md`) and the chunk ids read here are
stable across every 2020–2026 file met; a 2022 ghost and a 2026 ghost differ in
`formatVersion`, the `0x0303F006` blob and the `0x0309202D` block, and the
server reads both.

## 6. Not known

* Any server binary between 2022-06-21 and 2026-05-15.
* Which `0x0309202D` field the validator gates on (a one-field patch series
  would settle it).
* The exact client build that moved the physics after 2022.
