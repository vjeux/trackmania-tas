# The MediaTracker: chunk `0x03043049` and the media classes

Reader/writer: `tools/tmmaps/src/mediatracker.rs` (`tmmaps mediatracker MAP`,
the tiny transform, `--promote-endrace`, `--shift-trigger`, `--trigger-size`),
`tools/tinyctl/src/mtrender.rs`, `camcheck.rs`. Filming rules in `FILMING.md`.
Confidence **[FILE]** for the layouts read in full (the three camera classes,
Triangles, Fog), **[VERIFIED-GAME]** for the trigger and render behaviour.

## 1. Chunk `0x03043049` (NOT skippable)

It follows the baked-blocks chunk `0x03043048` on every Summer map and runs to
the next skippable header; `tmmaps chunks` never lists it. The MediaTracker's
own nodes carry skippable chunks (`0x0307900E` per clip, `0x03029002` per
Triangles block), so a byte scan sees those inside it.

```text
u32 version (2 on the Summer 2026 maps)
noderef ClipIntro          CGameCtnMediaClip       0x03079000
noderef ClipPodium         CGameCtnMediaClip
noderef ClipGroupInGame    CGameCtnMediaClipGroup  0x0307A000
noderef ClipGroupEndRace   CGameCtnMediaClipGroup
noderef ClipAmbiance       CGameCtnMediaClip       (version ≥ 2)
Int3    triggerSize        (version ≥ 1) cells per block: (3, 1, 3)
```

A node ref is the container's (`gbx-container.md` §6.2): `FFFFFFFF` null, an
index seen before = back-reference, a new index followed by `u32 class` and
chunks. The walker leans on "a new node's index is the next free one" to find
the end of a block whose layout it does not know (kept verbatim as an opaque
span). The chunk has no size field, so a rewrite is variable-length
(`MapFile::set_mediatracker` splices it).

## 2. Clips, tracks, groups

```text
CGameCtnMediaClip (0x03079000)
  0x0307900D: u32 version (1), u32 listVersion (10), u32 nTracks, nTracks × Track,
              string name, u32 StopWhenLeave, u32 U02, u32 StopWhenRespawn,
              string U03, f32 U04 (0.2), i32 LocalPlayerClipEntIndex (-1)
  0x0307900E (skippable, 8 B)
  0xFACADE01

CGameCtnMediaTrack (0x03078000)
  0x03078001: string name, u32 listVersion (10), u32 nBlocks, nBlocks × block node, i32 -1
  0x03078004: u32
  0x03078005: u32 v, u32 IsKeepPlaying, u32 IsReadOnly, u32 IsCycling, [v≥1: f32, f32 (-1, -1)]
  0xFACADE01

CGameCtnMediaClipGroup (0x0307A000)
  0x0307A003: u32 listVersion (10), u32 nClips, nClips × Clip node,
              u32 nTriggers (= nClips), nTriggers × Trigger, [skippable chunks], 0xFACADE01
  Trigger: i32 u01, u02, u03, u04, i32 condition, f32 conditionValue,
           u32 nCells, nCells × Int3 cell        — cells of the TRIGGER GRID (block cell × triggerSize)
```

## 3. Blocks read in full (they carry WORLD coordinates the tiny transform moves)

| class | chunk | keys |
|---|---|---|
| CameraCustom `0x030A2000` | `0x030A2006` v4 | N keys of 42 words: time, interpolation, anchorRot, anchor, anchorVis, target, then THREE 12-word camera states — the value, its left tangent, its right tangent — each {position, pitch/yaw/roll, fov, target position, 0.05, 1.0}. Tangents are VECTORS (scaled, not moved) |
| CameraPath `0x030A1000` | `0x030A1003` | N keys of {time, position, pitch/yaw/roll, fov, [v≥3 nearZ], anchorRot, anchor, anchorVis, target, target position, weight, [v≥4 …]}; the per-version tail is derived from the key stride = (bytes to the node end) / N |
| CameraOrbital `0x030A0000` | `0x030A0001` | N keys, raw words with the stride derived from the version (GBX.NET: v0 = 15 words after the time) — layout not pinned |
| Triangles3D `0x0304C000` (base `0x03029001`) | `0x03029001` | N key times, N again, V vertices, N×V Vec3 positions (world space), V RGBA colours, T Int3 triangles, then int, int, int, float, int, long; `0x03029002` skippable |
| Fog `0x03199000` | `0x03199000` v2 | N keys of {time, intensity, skyIntensity, distance, [v≥1 coefficient, colour RGB], [v≥2 clouds opacity, clouds speed]}; distance is metres of world: scaled |

Opaque (kept verbatim, listed in the report): CameraEffectShake `0x030A4000`,
Image `0x030A5000`, MusicEffect `0x030A6000`, Sound `0x030A7000`, Text
`0x030A8000`, Trails `0x030A9000`, TransitionFade `0x030AB000`, Triangles2D
`0x0304B000`, Time, Fx*.

## 4. What the game does with it **[VERIFIED-GAME]**

* The trigger grid: `triggerSize (3,1,3)` splits a block cell into trigger
  cells of 32/3 × 8 × 32/3 m, row 0 at the collection's ground; **the game
  honours the chunk's trigger size** (the tiny converter doubles it to
  (6,2,6) so the half-size cells still resolve; `TINY_TRIGGER_SIZE=keep`).
  **A car SPAWNED INSIDE a trigger does not fire it; a car ENTERING one
  does** — the tiny end-race clip fired within one frame of entry (12.973 s
  vs the jump at 12.959 s) (2026-09-07, 23ff47f).
* Playground timeline on the Summer maps: intro 0–10 s, chase camera from
  10.0 s, race start ≈ 14.7 s after the playground opens (19–20 s on the big
  maps); the chase camera offset (−4.5 m, +2.2 m) is a game constant,
  unscaled. All 25 maps use only CameraCustom, Triangles2D, CameraGame and Fog
  (13/16/17, distance 40000); none has a podium clip (the game's default).
* The intro clip plays when a map opens in PLAY; shot k of the original and of the tiny build are the same
  moment of the same clip, so `tinyctl play` frames compare (`tinyctl camcheck`
  aligns the two clocks on the intro's first camera CUT).
* **An in-game clip runs when the player ENTERS its trigger zone.** A clip
  made by `CreateClip` in the editor has a default zone the car may never
  enter (where it lands depends on the editor camera), so a ghost render of a
  map with no in-game group renders a static frame from the map's thumbnail
  camera (`0x03043036`) with no car. `tinyctl mtrender` therefore moves the
  end-race group's node into the in-game slot and makes its first clip's zone
  every trigger cell the ghost passes through (+ `--pad` cells) — no node is
  duplicated, so every node index stays unique (2026-09-09).
* An end-race clip fires when the race ends; the same clip moved into the
  in-game group fires on trigger entry (`--promote-endrace`).
* A ghost camera block lasts exactly as long as its ghost's SAMPLE STREAM; a
  downloaded human recording keeps sampling ~0.5 s past its own finish and our
  tapes end at the finish, so the camera on our car runs short by that much
  (`FILMING.md` §1; trim the clip, never bolt the camera to the longer car).
* The MediaTracker renders as long as the LONGEST block: a ghost whose record
  declares a span past its car (an inherited carrier span) makes a 7:21 clip
  of a 218 s run with the camera flying to the top of the map when its target
  entity ends (memory `tm2020-ghost-record-and-scene.md`).
* The editor's `/shadows` lightmap pass and the shoot both play the group AS A
  RACE WOULD; live playback in the editor follows the imported ghost
  regardless.
* The "Ref. Ghost: Author ghost" of the MediaTracker is looked up BY MAP NAME
  in `ProgramData\Trackmania\MediaTrackerCache\MTAuthorGhost<name>.Ghost.gbx`
  (`ghost-caches.md`).
* Whether the GAME honours a finer trigger grid (`--trigger-size`) is untested.

## 5. The tiny transform (`transform`)

Every world coordinate (camera positions, targets, triangle vertices) goes
through the items' point transform; every tangent and fog distance through the
scale; every trigger cell through the cell transform. Times, angles and fields
of view stay. The field offsets the transform writes through are re-read from
the body and must equal the parsed values (a self-check on every write).

## 6. Not known

* CameraOrbital key layout; the trailing words of CameraPath keys per version.
* Every opaque block class's payload.
* Trigger `u01..u04`, `condition`, `conditionValue` semantics.
