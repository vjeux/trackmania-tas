# Prefabs, moving parts, effects, lights, vegetation

The classes between an item or block model and its triangles that are not the
mesh itself: `CPlugPrefab`, `CPlugStaticObjectModel`, `CPlugDynaObjectModel`,
`NPlugDyna_SKinematicConstraint`, `CPlugFxSystem` and the particle chain,
`CPlugLight`/`GxLight*`, `CPlugVegetTreeModel`. Readers:
`tools/mapgeom/src/static_item/{prefab,dyna,particle,light}.rs`,
`tools/mapgeom/src/{geom,node,classes,veget,rescale}.rs`. Confidence **[FILE]**
(byte-exact round trips on the Stadium pack), **[VERIFIED-GAME]**/**[EXE]**
where marked.

## 1. `CPlugPrefab` (`0x09145000`) and `CPlugStaticObjectModel` (`0x09159000`)

A prefab is a list of ENTITIES, each `{ ref model, quaternion rot, Vec3 pos,
instance params (raw words sized per GBX.NET), … }`. Models met: inline
`CPlugStaticObjectModel` trees (parse fully), external nodes (a `.Prefab.Gbx`,
`.StaticObject.Gbx`, `.DynaObject.Gbx`, `.FxSys.Gbx`, `.Mesh.Gbx`,
`.HitShape.Gbx`), `NPlugTrigger_*` records (`item-cgameitemmodel.md` §5),
`CPlugSpawnModel`. A prefab tree is deep and crosses files: `RoadDirtTiltCurve3`
is two prefabs, one holding 51 entities; the walk touches 85 files for 24 064
triangles of one 3×3 block. Every transform on the way down must be composed
for the leaf triangles to land in the block's frame.

`CPlugStaticObjectModel` (archive class): `u32 version, ref mesh (Solid2), u8
isMeshCollidable, [!collidable] ref shape (CPlugSurface)`. In the DEDICATED
SERVER's Stadium pack every road block reports `mesh = -1` (collision only,
no `CPlugSolid2Model`); the client packs carry the visuals.

Traps measured: a `GbxLoc` in a prefab's placement-group parameters is 28
bytes (position + quaternion), not a 48-byte `Iso4` (131 prefabs failed that
way); `CGameItemModel`'s `0x2E00201F` is v13 in this pack; the decoration
`.Map.Gbx` files are ENCRYPTED in the pack (the same key opens them).

Pack items are WRAPPERS: `entity model → external .Prefab.Gbx /
.StaticObject.Gbx (+ external .Mesh.Gbx / .HitShape.Gbx)`; a variant-list
item (`NPlugItem_SVariantList`) = its externals in REFERENCE ORDER, indexed by
the placement's variant byte (`Show*` rigs: one item with 37 externals — 0
RigStraight2m … 4 RigStraight32m, 23 Light4Spots, 25 LightRamp8m, 26 Speaker,
28 Fogger16M; `PalmForest` v11..24 = the palm species).

## 2. Moving parts: `CPlugDynaObjectModel` (`0x09144000`) + `NPlugDyna_SKinematicConstraint` (`0x2F0CA000`)

Neither has chunk framing — the class IS the struct (`dyna.rs`):

```text
DynaObject v13:  u32 version, bool IsStatic, bool DynamizeOnSpawn, ref Mesh, ref DynaShape (the
                 MoveShape, moves with the object), ref StaticShape (HitShape), 43 bytes verbatim
                 (break speed 100 km/h, mass 10, light durations 5/7, 1/1, u16 4, 1/10/0/0, u8 0),
                 ref LocAnim, 8 bytes, ref WaterModel
KinematicConstraint (127 B): u32 version 0, u32 sub 3,
                 TransAnimFunc { u32 1, N × { u8 ease, u8 reverse, u32 durationMs } },   ease: Constant|Linear|
                 RotAnimFunc   { same },                                                  EaseInQuad|EaseOutQuad|EaseInOutQuad
                 u32 ShaderTcType, u32 ShaderTcVersion, N × { u32 ms, u32 subTexture } (the countdown light
                 strip: 6 frames over the pusher's 4.8 s), [type 1: NbSubTexture, PerLine, PerColumn, TopToBottom],
                 u8 TransAxis, f32 TransMin, f32 TransMax, u8 RotAxis, f32 AngleMinDeg, f32 AngleMaxDeg
prefab entity params:  0x2F0B6000 (dyna instance, 32 B) = { v2, PeriodSc 8, TextureId 0, IsKinematic 0/1,
                       PeriodScMax 16, Phase01 0, Phase01Max 1, CastStaticShadow 0/1 };
                       0x2F0C8000 (constraint params) = { Ent1 -1 (the world), Ent2 = the RANK of the dyna
                       object among the prefab's DYNA entities }
```

The prefab's own `KinematicConstraints\ObstaclePusher8m` has a ZERO range (the
OFF pusher); the item's Level modifier (game skin `ItemObstacleLevelN`) swaps
in `Modifier\ItemObstacle\AnimPusher8mLevel0/1/2` = Z 0..8 m over 8/4/2 s
(Level1: 2 s out / 2 s back, linear; the rotor: Z 180..−180° over 4 s).
Travel scales with the item, period does not (Nadeo's 4m/8m/16m Level1 all use
2 s + 2 s). MoveShape of a pusher = `CPlugSurface` ConvexPolyhedron (20
vertices, 18 faces); a rotor's a Compound of nine.

**[VERIFIED-GAME]** (Summer 15 lineups, 2026-09-07/08):

* Kinematic parts ANIMATE in an embedded item exactly like stock ones
  (pushers travel, rotors turn, screen textures cycle; the stroke IS halved; a
  half pusher shoves the car 19 m vs 25 m); the layout the game renders is the
  prefab DIRECTLY under `CGameItemModel`, entities `[0] DynaObject (inline
  scaled Solid2 + MoveShape + HitShape), [1] StaticObject (the box), [2]
  KinematicConstraint (inline)`; wrapped in a `CGameCommonItemEntityModel` the
  item is dropped silently.
* A dyna object with a NULL `DynaShape` CRASHES the client at load
  (`Trackmania.exe+0xb7088c`, `NULL+0x38`: the constraint dereferences its
  target's shape); a mesh, a convex polyhedron or a compound all serve.
* The phase of a kinematic ITEM is the map's per-placement `AnimPhaseOffset`
  byte (`map-items.md` §4); `Phase01` and the in-record `0x03101005` word do
  nothing.
* The race clock in play mode starts ~13 s after the playground opens; the
  kinematic clock runs in the editor too (`tinyctl motion`).

### The vertex-tween cloth — a wall **[EXE]** (`tm2020-tween-anim-re.md`, `TINY.md` "Animated items")

The flag cloth is a visual-only dyna (kind `0x914E000`; the kinematic kind is
`0x914F000`) whose `Flag.Mesh.Gbx` is a Solid2 with 5 LOD visuals × 86
animation FRAMES stacked in inline vertices (12 384 = 86 × 144, one 726-index
list, sub-visual table `0x09006005` = per-frame `{vertex base, index start,
count}`; `VisCstType 2`). The spawner (`0x140b72d20`) tests
`CPlugSolid2Model+0x1f0 & 1` ("some visual has ≥ 2 sub-visual frames"; our
embedded cloth passes) and packs a u32 anim handle (11-bit log period
0.25..60 s, 12-bit phase, 8-bit TextureId) for `NHmsMgrInstDyna2`. What an
embedded cloth LACKS is `CPlugVisual+0x24` bit 27 (the VMorph/tween mark):
set in exactly one place at run time (`0x1404059b0`, from
`CPlugSolid2Model::OnNodLoaded`) and ONLY when the geom's material, taken from
the plain `Materials` list (`+0xc8`: `CPlugMaterial` refs, i.e. external
`.Material.Gbx` files) has the vertex-tween property; the `CustomMaterials`
user-inst list — the only material form an embedded item can hold — is never
consulted, and the file cannot carry the bit (the chunk readers mask it).
Without it the VertexTween shader draws nothing on its own and whatever a
stock flag's draw left bound (the "borrowing": a stock flag in view drives our
cloth, shards across LOD bands, bare poles). **Verdict: no embedded item can
carry a self-animating vertex-tween cloth**; the shipped tiny flags are the
still frame-0 cloth (`ItemFlagNoAnim`), or the STOCK `Flag8m` whose
`FlagSmall.Mesh` is the 16 m cloth at exactly ×0.5 (the pole is not). A
mechanical banner of kinematic strips (one item per strip, phase per
placement) is designed and not built.

## 3. Effects: `CPlugFxSystem` (`0x0915C000`) and the particle chain

```text
chunk 0x0915C000: u32 version 1, u32 10, root node, Id ContextClassId, Id ExtraContextClassId,
                  u32 varCount, u32 55, vars
node:  u32 type, Id Name, then per type
  0 Parallel         : u32 count, children
  1 Condition        : string ConditionExpr, child
  3 UpdateVar        : Id VarName, u32 ResetToDefaultIfInactive, string UpdateVarExpr
  4 ParticleEmitter  : ref Model (CPlugParticleEmitterModel 0x090B3000), Id JointName, 10 expression strings,
                       u32 DOVAndUpAreLocalSpace, 2 expression strings
var:   Id name, u8 type; type 2 (Real): f32, u8, f32, f32; type 6: u8, u8
```

Emitter string order (the exe's own serialiser `exe+0x62098c`): 1 LocalOffset
(`float3(0,0,5)` on `SparklerEnd8m`), 2 WorldOffset, 3 LinearVelInW, 4
SpawnFreqModifier (`cos(Time/800-1)` on the pulsing sparklers), 5 Scale, 6
LAmbient, 7 Up, 8 DOV, 9 Opacity, 10 WaterTop, [bool], 11 LinearHue01 (`-1.0`
or the `Hue` var), 12 HueLightness. Chain: `CPlugParticleEmitterModel
0x090B3000` (`*.ParticleModel.Gbx`) → `CPlugParticleEmitterSubModel
0x090B2000` (chunk `0x090B2036 = {version, u32, u32, u32}`) → render node
`0x090B5000`, `CPlugParticleGpuSpawn 0x090C5000`, `CPlugParticleGpuModel
0x090C6000`. Parse → write of every FxSys and ParticleModel of the Stadium pack
is identical (`mapgeom fx-dump --check`).

**[VERIFIED-GAME]**: an embedded item CANNOT carry a live particle emitter in
this build — any emitter with a model (inline or pack path, with or without
`ContextClassId 0x2F0DD000`) makes the game silently DROP the whole item at
instantiation (`CGameCtnChallenge::InitAllAnchoredObjects → exe+0xB8F680`
returns 0), geometry included; an FxSystem with NO emitters is kept. Emitters
live only in `Fogger16M/8M`, `FoggerWithLight16m/8m`, `Sparkler16m/8m`,
`Torch/TorchSmall` (`TorchSmoke.FxSys`), the `Show` rig's variant 28 and the
Podium's 8 `PodiumSparkler`s; the rest of the Show family carries none. The
stock SMALL siblings give the half-reach effect for free (`ShowFogger16M →
ShowFogger8M`: the 16M/8M in the name is the effect's REACH). Special gates
have no particle system. Crash forms fenced by `item-check FX-01/02/03`: a
NULL texture ref, an inline `CPlugBitmap` (the engine reads an inline bitmap
with a different chunk set from a `.Texture.gbx` file: user-file readers
dispatch chunks `0x2B–0x2E, 0x30, 0x32–0x3A`, pack files carry `0x19..0x2D`),
a NULL particle model.

## 4. Lights: `CPlugLight` (`0x0901D000`) + `GxLight*`

```text
0x0901D003: u32 v1, ref ImageAnim, f32 periodMin, f32 periodMax, Id animTimer
0x0901D004: u32 v0, ref GxLight (INLINE GxLightSpot 0x0400B000 / GxLightBall 0x04002000; also GxLightFrustum
            0x0400A000, Directional 0x04007000, Ambient 0x04005000), ref FuncLight, ref BitmapFlare,
            ref BitmapProjector (the lamp's ItemLamp_I texture), u32 flags, ref ColorTargetTable
GxLight 0x0400100A: v, colour, flags (0x6d), intensity 1.1, diffuse, shadow, flare 0.348, shadowRGB
GxLightPoint 0x04003004: flare size 12, biasZ 5
GxLightBall 0x04002008: flags 0x12, Radius 50, specular 50, shadow 16, flare 200, EmittingRadius 0.2,
            CylinderLenZ 0.2, attHTnLR [0, 8.12], ambient 0, hyper2 [−0.96, 0.206]; 009 = 50; 00A = 1/64
GxLightSpot 0x0400B003: v1, flags 0, inner 80, outer 130, flare 130, innerShadow 80, outerShadow 130,
            falloff 0, bytes [1,1]  (ItemLampSpot; the GateGameplay spot 140/170)
```

All 73 `.Light.Gbx` of the Stadium pack (hash-named `Stadium\Media\<32hex>`,
275–485 B) decode — only once the pak "dummy write" was emulated
(`pak-nadeopak.md` §4). A Solid2 `lights[]` socket names one; the bake embeds
the light INLINE with ball radii ×0.5 (chunk `0x04002009`) and intensity
unchanged, drops the pack refs (projector/flare bitmaps, colour table).
**[VERIFIED-GAME]**: the falloff is a function of d/R (half radii = the
original irradiance: night top-2 % patch (137,135,45) stock vs (138,137,46)
ours; ×0.5 dims, ×0.25 is unlit); GxLight flag bits 0x02/0x04/0x08/0x40/0x80
or ball 0x400 dim a lit item ~3×; `IsNatural` does nothing; block lights
(`StageTechnicsLight*`) go through the same MergedLight path; ANIMATED lights
(`is_animated`: chunk 003 ImageAnim set or a `CFuncLight` ref —
`SpeedometerCP.Light`) and gameplay-gate lights are NOT embedded (they burned
gates white). Beam along local −Z; inside a prefab the light's rotation
composes with the entity chain's rotation INVERTED (`R_entityᵀ·R_socket`)
while its position follows the geometry. The item-editor light form
(`CPlugLightUserModel 0x090F9000` + `light_insts`) crashes the client
(`+0x4c9062`); no pack Solid2 has one.

## 5. Vegetation: `CPlugVegetTreeModel` (`0x2F086000`, `.VegetTreeModel.Gbx`)

A table-less struct (`veget.rs`; 191–193 of the 193 species files of the five
collections parse to the byte):

```text
u32 version (21), u32 h1 (3|4), u32 lodCount, u32 lodCount−1, u32 materialCount
material × n: [i32;3] .Texture.gbx refs (D, N, R; −1 none), [i32;3] .dds refs, [i32;3] unused, u8 leaf
u32 materialCount, u32 3; material × n: f32 2.0, string name
lod group × lodCount: u32 visualCount; visual × count: u16 materialIndex, u32 nodeIndex, u32 class 0x0901E000,
    CPlugVisualIndexedTriangles body (to FACADE; opens with CPlugVisual chunk 0x09006001; the index buffer
    inside 0x0906A001 is a node), u8 0
f32 × (lodCount−1) switch distances (50, 100), u8 1, f32 farDistance (100 | 150), u8 3, u8 2, u64 FILETIME,
f32 1.0, f32 0.1, u32, u32 1, u32 7, u32 7,
u32 hullVertexCount, Vec3 × n, u32 hullTriangleCount, (u32 a, b, c, u32 surfaceMaterialId — 14 Wood) × n
… wind / impostor parameters (kept raw)
```

Materials are INLINE (name + D/N/R images + a leaf flag), not pack
`.Material.Gbx` files (the vegetation renderer has its own shaders,
`Tech3/Trees/Tree_SelfAO_*`, `NPlugVeget::SMaterial {Color, Normal, Roughness,
Variation, SubSurface}`); leaf normals are a shell round the crown; the
varying colour byte = alpha = self-AO on level 0; leaf atlases are DXT5, ~80 %
transparent (the big oak 1024×512). The trunk hull (kind 7) is a few dozen
Wood triangles; −1 = none (grass, flowers, BlueBay's `JungleForestA/B/C` are
not collidable; trees are). **[VERIFIED-GAME]**: a `VegetTreeModel` placement
IGNORES the placement scale (species at 1/0.5/0.25 render one size); the stock
palms are trunk only (crown procedural); what an item bake cannot give a tree:
translucency/subsurface, per-leaflet shading, wind, impostor/LOD morph, soft
alpha (the item shader's alpha test is binary), dynamic lighting of the crown
(`TINY.md` "Trees").

## 6. Not known

* The 43 + 8 verbatim bytes of `DynaObject`; `LocAnim`, `WaterModel`.
* The wind/impostor trailer of `VegetTreeModel`.
* `CPlugSkel` (`0x090BA000`) skinned meshes: `GateExpandableFinish`'s
  inflatable horns are a 14 568-vertex visual with all y = 0 and an Int32
  bone-index element — posing it needs the skeleton's joints; not read.
* `LightRay.DynaObject.Gbx` (fails in the pak reader, `bad match offset`),
  two crystal layer types (13, 18) never met.
