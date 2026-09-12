# Zone genealogy: chunk `0x03043043` (`CGameCtnZoneGenealogy`, `0x0311D000`)

Reader/writer: `tools/tmmaps/src/map.rs` (`genealogy_full`,
`clear_genealogy_file`, `fill_genealogy_file`), `tools/tmmaps/src/cmd/inspect.rs`
(`tmmaps genealogy`), policies in `tools/tmmaps/src/tiny.rs`. Confidence
**[FILE]** for the layout, **[VERIFIED-GAME]** for what the game regenerates.

## 1. Layout

```text
u32 version
u32 innerLength           byte length of what follows
u32 count                 one record per cell in the chunk's own order (64 × 64 = 4096; no
                          coordinates stored)
count × record:               one per cell; records are indexed x*64 + z (`tmmaps genealogy-cells --cells`)
    u32 class 0x0311D000
    u32 chunk 0x0311D002
    u32 n                 length of the zone chain
    [u32 3]               lookback version, once (the chunk has its OWN table)
    n × Id  ZoneIds       the chain, root first ("Lake>LakeShore>Grass")
    u32 CurrentIndex
    u32 Dir
    Id  CurrentZoneId     the terrain block the game regenerates for this cell
    u32 0xFACADE01
```

The first record defines the lookback strings; later records of the same zone
are the short form `count, refs 1..=count, CurrentIndex, Dir, ref count+1,
FACADE` (`fill_genealogy_file` requires the source's own second record to have
exactly that shape before it writes copies). Cleared form: version, inner
length 4, count 0 (12-byte payload).

`tmmaps genealogy MAP` prints the histogram of `CurrentZoneId`, as
`Lake>LakeShore>Grass @1 d2 = LakeShore`.

## 2. What the game does with it **[VERIFIED-GAME]**

At load the game REGENERATES the terrain blocks the records name — Land, Beach,
LandHill*, LandCliff*, Water, Lake, LakeShore, GrassCliff2, Sea… — as full-size
32 m zone blocks at the collection's fixed plane. On Summer 2026 - 01 that is
1656 Land + 852 Beach + … blocks that showed up in the loaded map with every
authored block parked. The zone water (`Sea`, `Lake`) is one of these: a
one-unit block info with `surface "Water"` and a prefab holding an 8-triangle
`Water` (physics 13) quad at local +7 (Lake +7.2) over a floor at +4, 32 × 32 m
— which is why a car floats/sinks in it the way it never does on an item plate
(`collision-cplugsurface.md` §5, `TINY.md` "Water: the engine's native
representations").

Fixed water/ground planes per collection (`tiny::fixed_plane`), measured:
BlueBay 7.0 (Sea), RedIsland −0.5 (lake: Water prefab local +7.5 at cell 14),
WhiteShore −1.0 (Water zone at cell 14: 14·8 − 120 + 7), GreenCoast −0.8 (Lake
zone at cell 4: 4·8 − 40 + 7.2), Stadium 10.0 (Grass at cell 9, plane local +2).

**A regenerated zone block also hides nothing and is hidden by nothing except
the unit rule**: a terrain tile is never drawn in a cell one of a block's UNITS
occupies, authored or generated (`map-blocks.md` §8; `TINY.md` "Terrain tiles
under blocks", final 2026-09-11).

## 3. The three policies the tiny converter applies (`tiny.rs`)

| collection | policy | why |
|---|---|---|
| BlueBay (0x1C) | `clear` | the sea around the island is decoration, so no zone at all leaves plain sea under the half-size map |
| RedIsland (0x10), WhiteShore (0x1D), GreenCoast (0xF) | `fill` — every cell gets a copy of the map's first record, which must also be the most common zone (`Water` 2568/4096 of Summer 02, `Water` 3148/4096 of 03, `Lake` 2418/4096 of 04) | the ambient terrain is a zone BLOCK, so the game regenerates the full-size lake/sea around and under the tiny island, whose water items sit on the same surface |
| Stadium | `keep` | |

`TINY_GENEALOGY=clear|fill|keep` overrides; what the game regenerates under the
island is only visible in the game.

## 4. Related: auto-terrain on block infos

A block variant's ground form can declare `auto_terrains` (`CGameCtnAutoTerrain`
`0x03120000`: `Int3 offset` + a genealogy node) — the terrain the editor bakes
under/around the block (`blockinfo-cgamectnblockinfo.md` §4). The 2026-09-08/11
tile rule was first written against those declarations and was wrong: the tile
is hidden wherever a UNIT covers the cell, declared auto terrain or not.

## 5. Not known

* `Dir`'s meaning beyond "the record's direction word" and `CurrentIndex`'s
  role when it is not the chain's last entry.
* Whether the game reads anything of the chain other than `CurrentZoneId` at
  load (the fill writes the whole chain because the source does).
