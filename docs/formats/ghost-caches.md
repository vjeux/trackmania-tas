# The client's per-map caches: `LaunchedCP.gbx`, `MTAuthorGhost`, Autosaves, the lightmap cache

Files the game client writes on its own, keyed by the map's NAME, and what
this project reads out of them. Confidence **[FILE]** for the LaunchedCP layout
(`tools/ghost/src/lcp.rs`, measured on tiny 13/20/21/22), **[VERIFIED-GAME]**
for where and when the files appear.

## 1. `C:\ProgramData\Trackmania\LaunchedCheckpointsCache\<map name>.LaunchedCP.gbx`

Class `0x03262000` `CGameSaveLaunchedCheckpoints`, one chunk, uncompressed
body, written at EVERY checkpoint crossing in the editor's test mode so the
player can "start from checkpoint" — the route skeleton of an UNFINISHED run:
which checkpoints, in which order, the full car state at each crossing and the
last ~1.5 s of approach with the inputs. `ghost lcp FILE [--json] [--csv]
[--samples]`.

```text
u32 chunk id 0x03262000
u32 version = 7
u32 33                          constant on every file seen
u32 n                           entries (checkpoints reached, in the order reached)
n × ENTRY (663 B)
u32 total                       approach samples that follow
total × { u8[116] CSceneVehicleVis sample ; u32 t_ms }     t counts up inside each entry's window;
                                the last sample of a window is the frame before the crossing
n × u32                         samples per entry (sums to total)
n × { u32 0x0FF00000 ; MwId ident }     the vehicle model (CarSport / 10003 / Nadeo)
u32 0xFACADE01

ENTRY:
  +0x00 u32   checkpoint landmark index (= the `tmmaps waypoints` index)    +0x04 u32 0
  +0x08 u32   time at the crossing, ms (the test-mode clock: race time on a first run,
              session time after respawns)
  +0x0c f32×3 checkpoint position          +0x18 u32 0x0A020000 (state format tag)
  +0x1c u32   landmark to LAUNCH from (the group's for a linked checkpoint; else the same)
  +0x20 f32×4 orientation quaternion x y z w
  +0x30 f32×3 car position   +0x3c f32×3 velocity m/s   +0x48 f32×3 angular velocity rad/s
  +0x152, +0x195, +0x1d8, +0x21b: four 67-byte WHEEL blocks (FL FR RR RL by the steer field:
              the first two carry the steering angle, the rear two 0):
              +0 f32, +4 u32 1, +8 u32, +22 f32 rotation, +26 f32 steer angle rad
  +0x26b f32  signed forward speed m/s (negative = crossed in reverse)
  the rest: timers and -1 ids, kept as raw hex in the JSON
```

The 116-byte approach samples are exactly the ghost telemetry sample
(`ghost-telemetry-cplugentrecorddata.md`), so the last 1.5 s of steer / gas /
brake before each crossing are in here too, at the render frame rate (~53 ms).
Fixture: `tools/testdata/launchedcp_tiny20.LaunchedCP.gbx` (3 checkpoints, 83
approach samples).

## 2. `C:\ProgramData\Trackmania\MediaTrackerCache\MTAuthorGhost<map name>.Ghost.gbx`

The run a player finished in the editor's test mode, looked up BY MAP NAME as
the MediaTracker's "Ref. Ghost: Author ghost". Despite the extension it is a
`CGameCtnReplayRecord` (`0x03093000`): the whole map in `0x03093002` (9–18 MB
for a tiny map), then `0x03093014` whose one node reference is the ghost.
`ghost unwrap` turns it into a plain ghost with a slice and one node-index
rewrite (`replay-cgamectnreplayrecord.md` §3).

## 3. `Documents\Trackmania\Replays\Autosaves\<login>_<MAP NAME>_PersonalBest_TimeAttack.Replay.Gbx`

A finished solo run in PLAY mode, autosaved with no dialog and no plugin. Keyed
by the declared map NAME (a published tiny map still called "Summer 2026 - 02"
is indistinguishable from the original's autosave) and a PERSONAL BEST (a slower
run does not overwrite a faster one) — `tinyctl replay-pull` moves the folder
aside before the run and pulls the new file with its md5.

## 4. `C:\ProgramData\Trackmania\Cache\…` — the lightmap cache

One `<64-hex>_<16-hex>_<Collection>_<mood-tag>.Bump.LightMap.zip` per computed
lightmap; `ComputeShadows` reuses a matching entry (matched by map CONTENT,
not uid) and reports the request satisfied without ever going busy; deleting
the entries forces a real compute (measured 2026-09-12, coordinator session).
The same folder holds hash-named mod zips (a Nations mod `C0C53E74…zip`) and
`*.jpg|png|dds`. Details in `map-lightmap.md` §2.1.

## 5. Not known

* `LaunchedCP` entry bytes past `+0x26f` (timers, ids) and the meaning of the
  `u32 33`.
* Whether the game reads the LaunchedCP file back for anything other than the
  editor's "start from checkpoint".
