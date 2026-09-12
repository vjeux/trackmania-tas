# CGameCtnReplayRecord — the replay (`.Replay.Gbx`, class `0x03093000`)

A replay is a MAP plus one or more ghosts in one file. Readers:
`tools/gbx/src/{container,header}.rs`, `tools/ghost/src/{hdr,unwrap}.rs`;
`ghost header show`, `ghost map show|extract|set`, `ghost unwrap`.
Confidence **[FILE]** (fixture `testdata/replay_kacky_7241.Replay.Gbx`, 781 814 B),
**[VERIFIED-GAME]** for what the dedicated server simulates.

## 1. THE MAP IS INSIDE THE REPLAY

Body chunk `0x03093002` — **inline, no `PIKS`, not in any chunk table** — is
`u32 chunk id, u32 size, <a whole nested GBX file>` (`GBX` at body offset 8 on
the fixture, 771 380 B). The dedicated server simulates THIS copy; `--map`,
`UserData/Maps` and the uid in the header are decoration (`ghost verify
--empty-maps` validates the fixture at 7.241 with a Maps directory containing
zero files). A scan that only knows skippable chunks reports "no embedded map"
on every real replay.

Consequences enforced in code:

* `ghost map rebind` (uid rewrite) REFUSES a file that carries a map; `ghost
  map set` replaces the carried bytes (`gbx::container::set_embedded_map`:
  splice the new map in, fix the size word in front of it, rewrite the 27-char
  uid literals OUTSIDE the map — length-preserving);
* `tmmaps` refuses any GBX whose class is not `0x03043000`, so a map is edited
  by `ghost map extract` → `tmmaps …` → `ghost map set`, never in place;
* the carried map's byte range is PROTECTED from every chunk-size fixup: its
  own `0x0304305F` declares a size running past the map's end, and a walk that
  "corrects" it turns 7.241 into DNF while every string reads back fine.

## 2. Header user data (three chunks; 745 B on the fixture)

| chunk | layout | who |
|---|---|---|
| `0x03093000` (137 B) | `u32 version (8)`, `u32 lookback version (3)`, `meta uid` (lookback string), `meta environment` (lookback index, 26 = Stadium), `meta author` (lookback string = the MAP AUTHOR's login), `u32 raceTime` (a copy of the declared time), `string nickname` (the DRIVER), `string login` (the DRIVER), `u8`, lookback title id (`TMStadium`) | map's, then run's, then driver's |
| `0x03093001` (482 B, heavy) | one GBX string: XML `<header type="replay" …><map uid= name= author= authorzone=/><desc envir= mood= maptype= mapstyle= displaycost= mod=/><times best="7241" respawns= stuntscore= validable="1"/><checkpoints cur= onelap=/><playermodel id=/></header>` | `best=` is the run's claim; `author=`/`authorzone=` are the map's |
| `0x03093002` (98 B) | `u32 version`, `u32 authorVersion`, `string login`, `string nickname`, `string zone`, `string extra` | the DRIVER |

**Legitimate versus foreign is decided by POSITION, never by value**: on 173691
the map's author and the replay's driver are the same person and the same 22
bytes. `ghost identity set --anonymise` rewrites the driver fields and the
run's time copies and leaves the map's attribution alone; `ghost verify` V10
greps the finished file for identity-shaped strings and excludes the map's
range by offset. The header was a second container nothing read for a day:
after a successful body anonymise + declare, the header still said
`GothMommyTM`, `3Awx2_MzSdaCJZjZOht51A`, `<times best="49958">`
(`tools/gbx/src/header.rs`).

## 3. Body

```text
0x03093002   inline: u32 size + the nested map GBX
0x03093014   two words and a node index → the ghost (CGameCtnGhost node, class id + chunks)
             — the ghost's chunk stream is BYTE-IDENTICAL in shape to a standalone ghost body
             (opens with the constant 0x0303F006, closes with 0x0309202E + 0xFACADE01); the ONE
             difference is the telemetry node's index: 2 here, 1 in a standalone ghost
0x03093018   a fourth driver block, past the nested ghost node (where the identity walk stops)
0xFACADE01
```

`ghost unwrap IN.Replay.Gbx OUT.Ghost.Gbx` slices the ghost node byte-for-byte
and rewrites that one node index; the control requires exactly that and the
same tape / result / declared time back. A multi-car replay (227654's 27-player
carrier) carries the OTHER cars in the same record and a render would draw
them.

## 4. Extensions matter to the server **[VERIFIED-GAME]**

`/validatepath` reads only files named `*.Ghost.Gbx` or `*.Replay.Gbx`; a
candidate named `out.try3` is skipped, produces no result row and reads back
as a DNF (32 good regenerations were refused before the diagnostic went in;
`ghost::oracle::readable_name`). Note `MTAuthorGhost<map>.Ghost.gbx` is a
REPLAY by class despite its extension (§5).

## 5. Game-written replays

* `Documents\Trackmania\Replays\Autosaves\<login>_<MAP NAME>_PersonalBest_TimeAttack.Replay.Gbx`:
  a finished solo run in play mode, autosaved with no dialog; keyed by the map's
  declared NAME; a slower run never overwrites a faster one (`tinyctl
  replay-pull` moves the folder aside first).
* `C:\ProgramData\Trackmania\MediaTrackerCache\MTAuthorGhost<map name>.Ghost.gbx`:
  the run a player finished in the EDITOR's test mode, kept by map NAME as the
  MediaTracker's "Ref. Ghost: Author ghost"; a `CGameCtnReplayRecord` (the
  whole map in `0x03093002`, 9–18 MB for a tiny map, then `0x03093014` → the
  ghost) (`ghost-caches.md`).
* trackmania.exchange serves replays at
  `https://trackmania.exchange/recordgbx/<ReplayId>`; the header's `exebuild`
  buckets them by era with no decompression.

## 6. Not known

* `0x03093014`'s two words; `0x03093018`'s layout beyond "a driver block".
* The `u8` and the trailing fields of header `0x03093000`.
