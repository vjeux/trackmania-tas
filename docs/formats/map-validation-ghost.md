# The map's validation ghost: chunk `0x0305B00F`

Reader/writer: `tools/tmmaps/src/map.rs::strip_validation_ghost_to`
(`GhostForm::{Dummy, Skeleton, Remove, Keep}`), the extraction tool `ct
mapghost` (memory `tm2020-embedded-author-ghost.md`), `ghost unwrap` for the
MediaTrackerCache form. Confidence **[FILE]**, oracle results
**[VERIFIED-GAME]**.

## 1. Layout

```text
u32 chunk 0x0305B00F, "PIKS", u32 size
u32 version (0)
u32 byteLength                 of the node that follows
CGameCtnGhost node written WITH its class id and NO node index:
    u32 0x03092000, chunks…, 0xFACADE01
  — or the null node FFFFFFFF (byteLength 4): the 12-byte SKELETON
    [00 00 00 00 | 04 00 00 00 | FF FF FF FF] every unvalidated Nadeo map carries
```

Position: near the FRONT of the body (offset 190 on `testdata/map2.Map.Gbx`,
after `0x0305B00A` and `0x0305B00E`), before the blocks chunk. The chunk
`0x0305B00E` precedes it and is where a `Dummy` ghost is inserted when a map
has no ghost chunk at all.

**Its lookback context is CHUNK-LOCAL** — the ghost node's Ids (`CarSport`,
`Nadeo`, the ghost uid, two empty ones) do NOT share the body's numbering:
proven by the Summer 21 skeleton→real test (2026-09-08), where inserting a
real ghost right after `0x0305B00E` left every later chunk's raw indices
untouched, and by `tmmaps validate` (2026-09-11, bb4c5727), which writes a
standalone `.Ghost.Gbx` BODY verbatim (`file[25..]`, ending `FACADE01`) into
the chunk and the editor's `/authghost` reads it back (`RaceValidateGhost
{17417, TAS}`, `AuthorTime 17417`). An earlier belief (`map.rs`, from the
player project's `authorghost embed`) — that a real ghost's Ids are numbered
BEFORE every other Id of the body, so a ghost could never be INSERTED into a
skeleton map — is superseded by that test. Hence the `Dummy` form: Summer
2026 - 01's own 13 148-byte validation ghost
(`tools/tmmaps/assets/dummy-ghost-summer01.bin`) can stand in for a stripped
one on any map — but it replays the ORIGINAL lap over a tiny map, so the
converter's default became `Remove` (`--dummy-ghost` inserts, `--keep-ghost`
keeps the source's; 2026-09-09, 1b5eeb5e). The header XML's `validated="1"`
is set to `0` in every stripped form; a stripped tiny 02 loads and passes
`startcheck` (2026-09-08, 794011a7).

`tmmaps validate MAP --ghost G --out F [--gold/--silver/--bronze MS]` is the
inverse: it writes the ghost chunk, the author medal words in
`0x0305B004/8/A`, the header chunk `0x03043002` times (byte offsets +5
bronze, +9 silver, +13 gold, +17 author, +37 authorscore), the XML `<times>`
and `validated="1"`; medals Nadeo-style ceil to whole seconds (17.417 →
19/21/27). The game accepts it.

## 2. What is inside

The embedded ghost is the AUTHOR's validation run: a full `CGameCtnGhost` with
its input tape and telemetry (`ghost-cgamectnghost.md`), same chunk stream as
a standalone ghost body except:

1. **the `CPlugEntRecordData` node reference has no INDEX word** — a real
   ghost writes `u32 nodeIndex | u32 classId (0x0911F000) | node`, the embedded
   blob writes the class id alone. A reader that takes `0x0911F000` as the
   index "loads 0 ghosts", whatever `numNodes` it is given. Fix: insert `u32 1`
   before the class id, grow `0x03092000`'s size by 4, `numNodes = 2`;
2. **chunk `0x03092010` holds the map uid AS OF THE VALIDATION SAVE**, not the
   file's; once (1) is fixed the file loads and DNFs with no checkpoints until
   the uid is rewritten from header `0x03043003`.

Extracted that way, 286279's author run validates at **355.181**, 238835's at
**462.982**, telemetry byte-identical to the map's. Of 31 corpus maps, 2 embed a
full ghost, 6 embed a record-data-only ghost (watch-only, never
re-simulatable: 145875, 146612, 203330, 228607, 228811, 285268), 23 nothing.

Replayed over a TINY map the source's ghost is a car driving the full-size line
in the air — which is why the converter strips it.

## 3. The dedicated server and the client

* `/validatepath` validates only what is in `UserData/Replays`; the map's own
  embedded ghost is never simulated by it and is harmless to leave
  (`tools/tmmaps/src/segments.rs`).
* The dedicated server starts a validation run at the waypoint the ghost's
  provenance chunk `0x0309202D` names BY INDEX (the `u32` after the
  settings-flags word = the 0-based waypoint index; with it set to the Spawn
  placement's index the car is on the Spawn on all 25 maps — 2026-09-08,
  e6980597), while the client starts on the Spawn item — so a server-certified
  lap built on a wrong index started 150 m+ away on a checkpoint (measured
  2026-09-07, `tinyctl replay-pull`, `tinyctl startcheck`). `tinyctl
  startcheck` measures the client's resting position against the Spawn
  placement (tolerance 12 m; a start block's own offset is (8, 1, 8) =
  11.4 m; a start GATE item spawns 5.3 m along the gate axis = the pack
  prefab's spawn point halved).
* A finished solo run in play mode is autosaved by the client to
  `Documents\Trackmania\Replays\Autosaves\<login>_<MAP NAME>_PersonalBest_TimeAttack.Replay.Gbx`
  (keyed by the map's declared NAME; a slower run never overwrites a faster
  one) — the only client-made recording of a map with no plugin involved
  (`replay-cgamectnreplayrecord.md` §5).

## 4. Not known

* `0x0305B00A` and `0x0305B00E` payloads (28 and 30 bytes on map2): opaque.
* Whether the client reads anything of the embedded ghost beyond the
  MediaTracker "Author ghost" slot.
