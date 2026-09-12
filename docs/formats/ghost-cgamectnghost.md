# CGameCtnGhost — the ghost (`.Ghost.Gbx`, class `0x03092000`)

Owner crate: `tools/ghost` (every mutation) on `tools/gbx` (the bytes).
Readers: `tools/gbx/src/container.rs`, `tools/ghost/src/{synth,ident,census,
hdr,unwrap}.rs`; the class reference is `next.openplanet.dev/Game/CGameCtnGhost`.
Companion pages: `ghost-input-tape.md` (chunk `0x0309201D`),
`ghost-telemetry-cplugentrecorddata.md` (the record inside `0x03092000`),
`replay-cgamectnreplayrecord.md` (the map-carrying container),
`ghost-caches.md`. Confidence **[FILE]** (byte-identical re-emission by
`ghost synth` on five maps), **[VERIFIED-GAME]** where the dedicated server or
the client was asked.

## 1. Classes and members

```text
CGameGhost           0x0303F000
CGameCtnGhost        0x03092000   inherits CGameGhost
CPlugEntRecordData   0x0911F000   the telemetry record (nested node)
```

`CGameCtnGhost`'s 28 documented members (2026-01-26 build) are what the chunks
serialise: `Duration, Size, ModelIdentName/Author/Collection(+_Text),
GhostLogin, GhostTrigram, GhostCountryPath, GhostNickname, m_GhostNameLogoType,
GhostAvatarName, RecordingContext, LightTrailColor, RaceTime, NbRespawns,
StuntsScore, Validate_ChallengeUid, Validate_ScopeType, Validate_ScopeId,
Validate_GameMode, Validate_GameModeCustomData, Validate_ExeVersion,
Validate_ExeChecksum, Validate_TitleId, Validate_ExtraTool_Info,
Validate_OsKind, Validate_CpuKind`. Their object offsets from the server's
reflection table are in `BUILD-ID.md` §1 (`Validate_ChallengeUid 0x164`,
`Validate_ExeVersion 0x1c0`, `Validate_ExeChecksum 0x1d0`, …).

## 2. The body, chunk by chunk (`testdata/human_22730.Ghost.Gbx`, 16 034 B, 2026 build)

`ghost chunks` lists the skippable ones; `ghost synth` names every byte between
them and re-emits the body byte-identically (61 bytes it reports "unnamed" are
identified below).

| offset | chunk | framing | size | content |
|---|---|---|---|---|
| 0 | `0x0303F006` | inline | 28 | CGameGhost: `u32 version 1`, `u32 4`, `u32 12`, 12-byte zlib blob `78 9C FB FF FF FF 7F 00 09 FA 03 FD` — BYTE-IDENTICAL in every ghost of five maps (10.6 s to 235.6 s): a constant of the build, not of the run |
| 28 | `0x0303F007` | PIKS | 4 | |
| 44 | `0x03092000` | PIKS | 12 757 | the main chunk: model ident, skins, display name, the **telemetry record** (a nested `CPlugEntRecordData` node), then trigram / zone / club tag — §3 |
| 12 813 | `0x03092005` | PIKS | 4 | `u32 raceTime_ms` — the DECLARED race time (a file can hold more than one copy) |
| 12 829 | `0x03092008` | PIKS | 4 | |
| 12 845 | `0x0309200A` | PIKS | 4 | |
| 12 861 | `0x0309200B` | PIKS | 28 | |
| 12 901 | `0x0309200C` | inline | 4 | scalar |
| 12 909 | `0x0309200E` | inline | 4 | a hash — copied, never derived; zeroing it or flipping every bit still validates |
| 12 917 | `0x0309200F` | inline | 4 + 22 | `string accountId` (`u32 22` + 22 chars, e.g. `I0PKZ6d8R8iDvXT5nG8KNw`) — the server-reported account id; a skippable-chunk walk walks straight past it |
| 12 947 | `0x03092010` | inline | 4 + 4 + 27 | `MwId Validate_ChallengeUid`: `0x40000000` marker, `u32 27`, the map uid in the clear |
| 12 986 | `0x03092013` | PIKS | 8 | |
| 13 006 | `0x03092014` | PIKS | 4 | |
| 13 022 | `0x0309201A` | PIKS | 4 | |
| 13 038 | `0x0309201B` | PIKS | 30 | |
| 13 080 | `0x0309201C` | inline | 32 | a 32-byte hash — copied, never derived; not checked by the server |
| 13 116 | `0x0309201D` | PIKS | 2 179 | the **INPUT TAPE** (`ghost-input-tape.md`) |
| 15 307 | `0x03092022` … `0x0309202A` | PIKS | 8/47/8/167/16/16/0/20/8 | the validation metadata block (`Validate_*`: scope, game mode, exe version string, checksum, title id, OS/CPU kind …). `0x03092028` is empty |
| 15 705 | `0x0309202B` | PIKS | 52 | the **ghost-result chunk** — §4 |
| 15 769 | `0x0309202C` | PIKS | 12 | |
| 15 793 | `0x0309202D` | PIKS | 209–211 | the **provenance block** — §6 |
| 16 014 | `0x0309202E` | PIKS | 4 | |
| 16 026 | `0xFACADE01` | | 4 | body end |

A ghost body is thus ~25 chunks: a run of skippable ones and five inline ones
(`0x0303F006`, `0x0309200C/0E/0F/10/1C`) between them. Everything the SERVER
checks can be emitted from parsed values and constants (`ghost synth`); the
untested surface was the CLIENT loader, which is exactly where synthesised
containers have crashed (§8).

## 3. Chunk `0x03092000`: identity, then the record, then more identity

Structural walk (`tools/ghost/src/ident.rs::scan`), offsets on the fixture:

```text
@72    lookback  "CarSport"            ModelIdentName   (the first lookback carries the version marker)
@92    lookback  "Nadeo"               ModelIdentAuthor
       skin pack descriptors: each { checksum, path, locator URL }
@150   string    "Skins\Models\CarSport\Hans Sub Red v2_<uuid>.zip"      the car SKIN
@232   string    "https://core.trackmania.nadeo.live/storageObjects/<uuid>"  its LOCATOR URL
       (a "Prestige=Yes&Level=1&Year=2026&Mode=Ranked&Medal=Master&SubRank=3" string may sit
        here: the driver's ranked badge — found 2026-08-22 in 16 published files on 5 maps)
@330   string    "hobbi."               the DISPLAY NAME: the string immediately before the record
       u32 nodeIndex (1 in a standalone ghost, 2 inside a replay), u32 class 0x0911F000,
       u32 version 11, u32 uncompressedSize, u32 compressedSize, zlib (78 9C …)   ← the record
@12769 string    "HOB"                  GhostTrigram
@12776 string    "World|Europe|Germany" GhostCountryPath (the zone; the only self-identifying
                                        string of the tail — the trigram is the one BEFORE it,
                                        the club tag the one AFTER)
@12800 string    "$F97$O9II"            the club tag
```

Anonymising a container therefore has to clear: skin path (the storage-object
uuid in it), locator URL, display name, ranked badge, trigram, club tag, and
the account id in `0x0309200F` — and NOT the zone (it is the landmark the tail
is parsed by; blanking it made the trigram and club tag unfindable).
`ghost identity set --anonymise` clears all of them at once and FAILS if any
survives; `ghost verify` V3/V10 re-read them. A REPLAY keeps another copy of
the driver in its HEADER (`replay-cgamectnreplayrecord.md`).

## 4. The ghost-result chunk `0x0309202B` (`gbx::container::GhostResult`)

```text
u32 version = 1
i32 raceTime_ms
i32 u01, i32 u02
i32 word4_unidentified       — called nbRespawns in GBX.NET and it is NOT one (§4.1)
i32 nCheckpoints
nCheckpoints × (i32 time_ms, i32 tag)
i32 -1                       terminator
```

The map-1 WR's chunk is `[1, 19538, 0, 0, 3, 4, 7617, 2, 13308, 4, 16316, 0,
19538, 1, -1]`: FOUR of fifteen words are splits (`7.617 13.308 16.316 19.538`);
the last entry is the finish and equals the race time. `n == 0` is a
well-formed empty list (seven words), what a search carrier declares when it
never knew a checkpoint. This is the project's ONLY decoder of the chunk; four
copies once disagreed and one printed the version word as a `0.001` split.

### 4.1 Word 4

Measured 2026-08-22: all fifteen human records on 134672 read **5** while their
tapes hold 0, 1 and 2 respawn events; on 279218 files whose tapes DO respawn
read **0**; nor is it the checkpoint count (134672: 5 with 5 entries, 249521: 1
with 2, 279218: 0 with 1, the fixture: 2 with 3). Unidentified; nothing may key
on it. The trustworthy respawn count is the tape's respawn bits.

### 4.2 What the server checks about it **[VERIFIED-GAME]**

The declared checkpoint COUNT does not gate validation: 1, 2, 3, 5 declared on a
4-split map and 9 on a 3-split map, with intermediate splits written as
`0.000`, all validated at the right time and the server merely echoed the
wrong count in `DeclaredResult` (`oracle.cps_does_not_gate`). `wrong simu`
means the simulation did not reproduce the DECLARED RESULT; on a partial run
it says how far it got. What the count breaks is this toolchain's segment
builder, which refuses a reference ghost whose split count is not the map's.

## 5. Copies of the declared time and of the map uid

* `0x03092005` (one or more), `0x0309202B` (race time + last split), a replay's
  header `0x03093000` (`u32`) and header XML `<times best=…>`, and the
  `u03`/`end` words of the record. `ghost census FILE` counts every ALIGNED
  word equal to a candidate time outside the opaque blobs (the zlib record, a
  carried map): six declared-time sites on 173636, five on 199100 — N is read,
  never assumed. `ghost declare --from-oracle` writes the time the file DOES
  into every copy; `ghost verify` V2 counts body AND header copies.
* The map uid: `0x03092010` (MwId) plus literal 27-char copies
  (`Container::uids`); a PURE ghost is bound to its map by this uid
  (`ghost map rebind`), a replay by the map it carries.

## 6. The provenance block `0x0309202D` **[VERIFIED-GAME]**

209–211 bytes: the recording build stamp (`date=2026-05-15_18_00
git=128182-… GameVersion=3.3.0` form), a wall-clock pair, the title name, a
~36-byte per-session token, a settings-flags word followed by **the 0-based
INDEX of the waypoint the validator places the car at** (the dedicated server
spawns there; set to the Spawn placement's index the car starts on the Spawn
on all 25 tiny maps — 2026-09-08, e6980597; a wrong index put the server's car
150 m+ away on a checkpoint while the client started on the Spawn), and — in
one leaderboard ghost — `Openplanet 1.28.0 (next, Public, 2025-08-16)`. **A tape only validates in the container
it was built in, and this chunk is the binding**: bisected on the current
server by moving one chunk at a time from file B into file A (`tools/chunkswap`),
every other chunk including the whole 95 KB record and the splits was inert
and `0x0309202D` alone turned a validating file into `wrong simu`
(`OLDBUILD.md` §6; memory `tm2020-tape-container-binding.md`). Moving
`0x0309201D` and `0x0309202D` together lets a tape run in any container of the
map (ten gates, ten exact splits). Which field the validator gates on is
UNKNOWN; the wall-clock pair is the leading candidate (the server prints
`unexcepted walltime (103s)` when a container is trimmed without it). The
build stamp itself is decoration to the server (`tools/strpatch` rewrote it
both ways; nothing moved) — the physics come from the binary.

## 7. Container facts the tools enforce

* **One entity per LIFE**: a run with respawns is stored as a chain of
  `CSceneVehicleVis` entities tiling the recording (227654: 27 of them); every
  reader takes the entity with most samples — `ghost record show` prints them
  all (`ghost-telemetry-cplugentrecorddata.md` §4).
* **The record ends at the finish**: six untouched leaderboard ghosts (177 789
  samples) carry ZERO samples after their own race time, last sample = the 50 ms
  grid point before the finish; no file has a sample at a negative time
  (memory `tm2020-ghost-post-finish-tail.md`). Transplanted tapes inherit a
  carrier's longer record; `ghost trim --auto` cuts it.
* **The record's declared span** (`start..end` words) must not run past the
  car: 33 of 159 published ghosts did, producing a 7:21 clip of a 218 s run
  and a camera flying off when its target entity ended (`ghost record shorten`).
* **A record that runs past its declared time is soft-rejected by the
  client**: `0 -> 0` ghost blocks, a `FrameMessage` dialog, no crash, nothing
  in the log (`tools/ghost/src/film.rs`).
* **Two input channels**: the 10 ms tape and bytes 14/15/18 of every 50 ms
  sample carry the driver's inputs twice (`steer_byte = floor((s+127)·255/254)`,
  pedals 0/255); Cohen's kappa between them is 1.000 on an honest recording
  and 0.12–0.46 on a transplanted tape (`ghost verify` V6).
* **`ghost trim` owns a run's length** in both directions (a tape can be
  LENGTHENED by appending copies of the last packet with the respawn bit
  cleared; 2432 → 7000 ticks still re-simulates to 22.730); `ghost splice`
  owns its middle (Rule R: delete each segment's failed attempts up to its
  last respawn press; the result is a study artefact, not a lap).

## 8. What the client does that the server cannot see **[VERIFIED-GAME]**

* **The client soft-refuses a multi-client carrier** on import (7 entities +
  2368 mode-15 packets, or 9 entities): ghost blocks `0 → 0`, a
  `FrameMessage`, no crash. The container rule for a render is a single-player
  carrier — the CAR + `0x2D001000` + `0x032CB000`, mode-2 packets only, tail
  trimmed (`ghost debug keep-ents`, 2026-09-09, bd33f978).
* A regenerated ghost whose record keeps ONLY the vehicle entity crashed the
  client on `ImportGhosts` (173691 ×3, 285885 ×3); grafting the one live
  `0x2D001000` record back made it import — but `TAS_67319` and 284238's
  `TAS_97325` have no live non-vehicle record and import cleanly, so the shape
  is a suspect, not a verdict (`ghost verify` V11 is a WARNING). Nothing
  headless sees this; the server never reads the scene.
* The chase camera reads sample **byte 32**: a regeneration that wrote 0 there
  put the camera under a ramp for the last second; the constant 128 (what
  game recordings hold on the ground, with 42 at byte 33) fixes it
  (`gbx::record::NEUTRAL_VALUE`).
* A car drawn as a transparent WIREFRAME: sample byte 73 = `(state[0x1bc] &
  0xf) | (state[0x8] << 4)` held `0x12` where every game recording holds
  `0x10` (memory `tm2020-invisible-car-byte73.md`). `ghost verify` looks at no
  pixel; `clip frames` does.
* Dirt thrown where there is no dirt: sample bytes 89/91/93/95/97/99 inherited
  from a carrier (`tmtraj provenance`).
* A crash reads as `read: Connection reset by peer (os error 104)` in
  `shootctl setup` — indistinguishable at the socket from a plugin reload;
  check the game's PID either side.

## 9. Eras

Format versions seen: input archive format 11 (33-bit state literal) and 12
(34-bit); record version 11 (2023+); input chunk version ≤ 4; `0x0303F006`
constant per build. A 2022 recording (`exebuild 2022-07-06 / 113150`) fails on
the 2026 server as `wrong simu` and validates to the millisecond on the
2022-05-04 / 2022-06-21 dedicated servers, and vice versa — the physics differ,
not the container (`OLDBUILD.md`; `dedicated-server-oracle.md` §5).

## 10. Not known

* The payloads of `0x0303F007`, `0x03092008/0A/0B/13/14/1A/1B`, `0x03092022`
  … `0x0309202A` beyond "metadata; copied", `0x0309202C`, `0x0309202E`.
* The two hashes `0x0309200E` / `0x0309201C` (what they hash; the server
  ignores them; the client has not been asked).
* Which field of `0x0309202D` binds the tape (one-field patch experiment
  pending).
* Why the client crashes on some vehicle-only records and not others.
