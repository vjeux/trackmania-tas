# Decoration Scene3d: `CSceneLayout` (`0x0A003000`) chunk `0x0A00301C`

The decoration's world outside the blocks: its light rig and the solids that
are the island, the sea and the invisible shadow casters
(`<Coll>\GameCtnDecoration\Scene3d\Base64x64.Scene3d.Gbx`, stored in the
pack under a hashed name, LZ4 + dummy-write fold — `pak-nadeopak.md` §4).
Reader: `tools/mapgeom/src/classes.rs` (`0x0A00301C`), types `node::Layout`,
drawn by `geom.rs` (`mapgeom model <path> --out X.obj`; `mapgeom scene3d` is
the older scan-based export of the same solids). Confidence
**[DISASSEMBLY]** for the layout (`CSceneLayout::ArchiveChunk` 0x1407efc20,
case `0x1c` at 0x1407f0554, Trackmania.exe Aug 2025), **[FILE]** on the five
collections' layouts (all v5).

```text
u32 version                              5 (< 3 takes a legacy reader 0x1407ef190, not met)
u32 nLights
nLights × {
    Id   name                            lookback string (empty on the packs)
    Vec3 pos
    Quat rot (x, y, z, w)                the sun: (0.3584, 0, 0, 0.9336) = 42° about X
    u32  version (0)
    ref  bitmaps[3]                      CPlugBitmap: DefaultAmbientCube, DefaultAmbientGrad / SunFlare
    ref  light                           inline GxLightAmbient (0x04005000) / GxLightDirectional (0x04007000)
    u32
}
u32 nMobils
nMobils × {
    Id   name                            "Warp" on the island solid, empty otherwise
    Vec3 pos                             (1024, 0, 1024) for RedIsland/WhiteShore's sky dome, else 0
    Quat rot
    u16                                  0x401 on the sky dome, 1 on the solids
    u64  flags
    ref  solid                           CPlugSolid (0x09005000): inline (BlueBay) or the external
                                         <Coll>\Media\Solid\Warp\Square64Water.Solid.Gbx / ShadowCaster64.Solid.Gbx;
                                         the sky dome is Sky\Media\Solid\SkyDomeMirror.Solid.Gbx (not in the packs)
    v≥4: ref  (class 0x090BB000)         −1 on the packs
    v≥5: ref  CPlugPrefab                −1 on the packs
}
ref  (class 0x0A040000)                  −1
v≥2: ref  weather                        Techno3\MotionManagerWeathers\DayTime.MotionManagerWeathers.Gbx
     u32 n, n × u32                      0 on the packs
     ref  bitmaps[3]                     −1, −1, Techno3\Media\Texture\DefaultEnvCubicHdrScaleA2.Texture.gbx
     f32 × 11                            0x141406680: 6 + 5 words; BlueBay (1711.5, 125.8, 204.6, −0.224, −10.24, 0, 100, 60, 3, 1, …) — meaning not pinned
     u32 × 4
     ref  (class 0x0A03A000)
```

Leaf readers, for the record: `0x1402d4eb0` = the Id (lookback, version word
3 once), `0x141462f40` + `0x140194a20` = Vec3 then Quat (w read last),
`0x14012c330` = `Read2`, `0x14012c410(…, 8)` = eight raw bytes,
`0x140155240` / `0x1404b8570` / `0x1407f0d90` / `0x1407f0c90` / `0x1407df9c0`
= typed node refs (a new node still carries its class id and body inline).

The solids are `CPlugSolid` → `CPlugTree` trees (`0x0904F006` children,
`0x0904F00D` name, `0x0904F016` visual/shader/surface, `0x0904F01A` flags +
Iso4): BlueBay's "Square64Water" tree holds 8 leaves — `Warp-C00x_0` (Water
material, 83 verts) and `Warp-C00x_1` (WarpSand, 184 verts) per quadrant — and
the third mobil is the InvisibleShadowCaster skirt. Vertices are world
metres; every mobil of the packs sits at the identity except the sky dome.
The lightmapper draws these through the generic scene render, so they are
peel and shadow casters like blocks.

The light rig here is the DECORATION's default sun (a GxLightDirectional at
42° about X, white) — the bake replaces it with the mood's `LDirSun`
(`lightmapper-client.md`, the sun direction from `DayTime` via the
`CPlugMoodBlender` curve); the GxLightAmbient colour (BlueBay 0.373, 0.396,
0.439) is likewise the editor default, not the mood's `LAmbient`.
