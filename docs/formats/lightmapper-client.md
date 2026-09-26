# The client's lightmapper (`NHmsLightMap`) — what "Compute shadows" computes, and how

Reverse-engineered from the retail client (Trackmania.exe, 45 467 720 B, md5
4a28c00429c6f75c894cf7bc4378a8a2, the 2025-08-24 build on the render box) and
from the game's own shader cache (`Packs/GpuCache_D3D11_SM5.zip`), so that
`lmtool bake` (docs/formats/map-lightmap.md §5) can be built from what the game
does instead of fitted against its output. Instruments: `tools/clientre`
(GpuCache → DXBC, an SM5 disassembler with constant-buffer reflection, and
`lmimages`, which splits a map's lightmap blobs into their RIFF sub-images and
decodes the frame records), `tools/asmdig` over an `objdump` listing of `.text`,
Ghidra 12.1 headless for the CPU side (`RangeDecomp.java` — decompile every
`.pdata` function in the lightmapper's address ranges without a full analysis;
the exception directory is relocated by the protector to RVA 0x2b09b80, the
section named `.pdata` is junk). Working notes, the disassembled shaders and the
decompiled functions are banked under `tm-player/tiny/lightmap-re/client-re/`
(NOTES.md, shaders/, decomp/).

Every claim is tagged: **[DISASSEMBLY]** (shader bytecode or decompiled exe
code, address given), **[RUNTIME]** (read from the running game), **[FILE]**
(read off editor bakes / pack data), **[DIFFERENTIAL]** (a bake experiment),
**[INFERRED]** (consistent with the above, not directly read).

## 0. The headline

1. The lightmapper is a **GPU program**: every stage is a pixel/compute shader
   under `Lightmap/*.hlsl` (75 entries, 1–8 permutations each); the CPU
   (`NHmsLightMap::*`, `.text` 0x14020d000–0x1402a0000 and
   0x140a3a000–0x140aa0000) allocates charts, builds direction sets, runs the
   passes as a background coroutine (`NGlobal::ComputeLoop`, states resumed
   frame by frame) and packs/compresses the result.
2. The stored lightmap holds **the sky and the bounces for static
   geometry** — the direct sun is **not** in frame 0. RE child 1's first
   reading (from the runtime material shaders, which add the PSSM sun
   themselves) was right; its 2026-09-23 "correction" came from a wall
   reading 1.10 HDR "in LDirSun's orange" at the Sunset quarter — that term
   is the **sky itself**: the dome pass renders the mood's sky dome
   (`Tech3/Sky_p`: the `SkyColor` gradient shifted to the sun's azimuth plus
   the `Atmo1/Atmo2` sun-glow lobes), so at low sun the horizon toward the
   sun is a bright pink patch and at noon nothing directional remains
   [DISASSEMBLY §2.3, DIFFERENTIAL baker 2026-09-23: Day-quarter bakes are
   azimuth-flat with blue brightest verticals, Sunset-quarter bakes carry a
   1–2° ESE pink term]. The sun enters only the **bounce** input (the peeled
   surfaces are lit by `LightDirRgb·max(0,n·−L)·shadow` in `GeomILightIn0`,
   §2.4). The cache flag `IsOnlyIndirectDir0` says the same.
3. The stored values are **H-basis coefficients** (4 per texel: one constant,
   three directional), **sqrt-encoded and normalised per chart, written as
   BT.601 studio-swing YCbCr planes** — a WEBP decode gives the encoded values
   directly, and the game squares them at load or in the shader. Frame-0
   image 0 is the colour constant term; frame-0 image 1 is **three
   concatenated grey WEBPs**, the directional terms [FILE, DISASSEMBLY]. Frames
   1 and 2 hold the local lights (§2.6).

## 1. The pipeline of a bake

`CGameEditorPluginMap::ComputeShadows` (0x140f91100, the script/plugin entry;
`ComputeShadows1(EShadowsQuality)` at 0x1400a7980) →
`CGameCtnApp::HmsLightMapComputeCurrentChallenge` (0x140c53928) →
`CGameCtnApp::HmsLightMapCompute` (0x140c53cfc, 5.4 KB, a coroutine) →
`CHmsLightMap::ComputeLighting` (0x14021a9b0) → `NHmsLightMap::ComputeLighting`
(0x14021db9f) → `RenderLighting_Frames` (0x14021e390: one pass set per frame,
"Frame %u/%u") → `RenderLighting` (0x140222d19). Quality enum
`EHmsLightMapQuality` = {0 VFast, 1 Fast, 2 Default, 3 High, 4 Ultra}
(table at 0x141e70f58) [DISASSEMBLY]; the map's own quality byte per item
(chunk `0x03043068`, "MapElemLightmapQuality") biases its chart.

The stages, in the order the function/shader names and the coroutine states
give [DISASSEMBLY; names are the game's own, the strings sit next to each
function]:

| stage | CPU | GPU | what it does |
|---|---|---|---|
| id binding | `CGameCtnChallenge::AutoSetIdsForLightMap` 0x140b88a1c, `TransferIdForLightMapFromBakedBlocksToBlocks` 0x140b91f00, `NGameMgrMap::IdsForLightMap_BindToScene` 0x140dc5c4c | — | every block/item gets an "IdForLightMap" (§4) |
| chart allocation | `NHmsLightMap::AllocateBlocks(_)` 0x14028f709/0x1402901e5, `AllocateWithScale(_)` 0x140291270/0x140294860, `AllocateWithScale_BlockSplit` 0x140295516, `CHmsLightMapAllocT3::AllocateBlocks` 0x14028f600 | — | one chart per lightmappable visual, sized from its surface; the atlas scale is searched (§3) |
| geometry | `InitGeometryAndTransfos` 0x1402212b5, `ConvertAllBlocks_FromVertexToTexel` 0x140216c40, `DynaUV_UpdateMapper_UvTransfos_FromCache` | `LmRasterPosNrm_Inst_v`, `LmLBump_Inst_v` | instanced rasterisation of every object in lightmap space: per instance quaternion + translation + uniform scale (TEXCOORD5/6) and the chart's ST (TEXCOORD7, or a per-vertex chart index in BLENDINDICES → `g_TcLM_ST_LM01[]`) |
| coverage / ids | — | `LmCoverage_Inst_*`, `LmSSGid_SetId_Inst_p`, `LmSSGid_MergeId_p`, `LmSSGid_UpSampleY_p` | which supersampled texel belongs to which geometry id (gutter filling later) |
| albedo | `Compute_MDiffuse` 0x1402246e3/0x140224f15, `RenderLM_MDiffuse` | `Block_*_PeelDiff_p`, `SetWaterId_*` | the materials' diffuse colour per lightmap texel (`TMapLM_MDiffuse`) — the bounce albedo |
| ambient | `RenderLightDirect` 0x140229669 | `LmLBumpAmbient_Inst_p` | §2.1 |
| sun (shadow-mapped, into the frame) | `RenderLightDir` 0x14023809e, `RenderLightDirMulti` 0x140237d37, `RenderLightDir0ToBitmap` 0x14022fd8a | `LmLBumpLDir_Inst_p`, `LmLHBasisDirect_Inst_p`, `ZOnlyParaboloid_Inst_v` (ball-light shadow cubes) | §2.2 |
| local lights | `RenderLightBall(Multi)`, `RenderLightSpot(Multi)`, `RenderLightIndex_NoBump_Inst`, `ComputeLightIndex` 0x14024240f, `LightId_Share1_SetLists` | `LmLBumpDirect_Inst_p`, `LmLIndex_Inst_p`, `LmLIndex_Sort_c` | §2.6 |
| supersample resolve | `SuperSampleNormalize_LightSums` 0x14023e269, `RenderAddAlphaSSAA` | `LmSSResolve_LBump_p`, `LmSSResolve_Light_p`, `LmSSNormWithA_p`, `LmSSNormOrGutterWithA_p`, `LmSSResolve_Spread*` | box-average the cSamplePerAxe² sub-texels, divide by the accumulated weight, spread into gutters |
| sky + bounces | `RenderLightIndirectDome` 0x140233b94, `RenderLightIndirectPeel` 0x140234e45, `RenderLightIndirectBounces` 0x140230b12 ("Lighting bounce %1"), `ComputeBounces_SpherePoints` 0x140a9e6b4 | `PeelZDiffuse_*`, `GeomILightIn0_*`, `LmILightDir_Set_p`, `LmLBumpILighting_Inst_p`, `LmLHBasisILighting_Inst_p`, `LmILightDir_AddAmbient_c` | §2.3–2.4 |
| probes | `ProbeCpt_SafetyOffset_Compute` 0x1402285e0, `ProbeGrid_ComputeIsValidSS` 0x1402217c7 | `ProbeGrid_*` | §3 (probe volume) |
| bump average / HDR statistics | `LightSumBumpGetAverage` 0x14022e7c0, `LightSumRenderToStatic` 0x14022be72 | `LmLightSumBumpAvg_p`, `LmLightSumCopy_p` | (Tx+Ty+Tz)/3; the frame record's maxima |
| encode | `BitmapSetStaticCompress` 0x14022abb0, `ImageRealToNat8` 0x14022ca00, `YCbCr_to_RGB_Down2x2` 0x14022af60 (load side) | `LmCompress_HBasis_YCbCr4_c` | §2.5 |
| write | `CHmsLightMapCache` archive 0x14027da50 (chunks 0x0602200B…1A), `SHmsLightMapCacheMapping::UpdateBlocks` | — | the `.Bump.LightMap.zip` cache and the map chunk (map-lightmap.md) |

Cache constants written into every bake and read back at load [FILE,
DISASSEMBLY 0x14027da50, chunk 0x0602201A v13]: `m_LightAmbSampleCount = 256`,
`m_LightDirSampleCount = 64`, `m_LightPntSampleCount = 25`, `m_SortMode = 1`,
`m_AllocMode = 2`, `m_CompressMode = 3`, `m_Bump = 4` (`HBasis_Intens`),
`m_Version = 8` (`lightmap="8"` in the header XML), `m_QualityVer = 14`
(`Item_Prefab_MultiMesh`, the newest of the 14 `EQualityVer` values). The three
sample counts are identical on every file we have (Nadeo's, editor q=3, q=4),
so the quality level does not change them — it changes the texel budget
(`WantedTexelByMeter`/`m_AllocatedTexelByMeter`, §3) and the supersampling.

## 2. The light model

Notation: `n` = surface normal, `P` = texel world position, `L` = unit vector
towards the light, `D` = a direction of the dome/sphere set. All colours are
linear HDR in the mood's units (`LDirSun HdrColor` ≈ 2.9 is the sun).

### 2.1 Ambient [DISASSEMBLY `LmLBumpAmbient_Inst_p`]

```text
E_amb(n) = AmbientAtTop · (0.6 + 0.4 · (0.5 + 0.5 · n.y)) = AmbientAtTop · (0.8 + 0.2 · n.y)
```

No visibility term in this pass. The dome passes (§2.3) add the sky on top,
so the "AO look" of shadowed texels comes from the sky term going to zero
under occluders, not from the ambient. `AmbientAtTop` is the mood's
`LAmbient HdrColor` (the frame record stores a per-frame `LAmbient` too,
§2.5). Local-light passes preload the same shape as `LightToAdd ·
lerp(LightToAdd_ScaleYm, LightToAdd_ScaleYp, 0.5 + 0.5 n.y)`.

### 2.2 The directional-light machinery (sun): sample set and shaders [DISASSEMBLY]

`LmLBumpLDir` is the generic directional-light pass; for frame 0 the sun is
**not** drawn into the texels (§0, §2.3) — it lights the peeled bounce
surfaces through `GeomILightIn0` (§2.4) with a single shadow map
(`TMapShadowLDir0`), and this section describes the pass that other
directional lights (and the NLocal/preview paths) use.

`LmLBumpLDir_Inst_p` (no-bump permutation):

```text
E_sun = shadow(P) · max(0, n · −DirInWorld) · LightRgb · OutScale ;  alpha += OutScale
shadow(P) = sample_c_lz(TMapShadow, (WorldPw01Shadow · P).xy, depth)   — one hardware comparison
```

**Sample set of a directional light** (`FUN_140237330`, the generator
`RenderLightDirMulti` runs before its passes; N from the per-quality table
at 0x141e6f338 = {VFast 1, Fast 16, Default 64, High 64, Ultra 64} unless the
light carries its own count at +0x120; the light's angular radius A =
`GxLightDirectional.EmittAngularSize` (degrees, field +0x80 of the light):

* A > 19.5°: draw `N / ((1 − cos A)/2)` points of the **precomputed sphere
  point table** (`Techno\Media\PointsInSphere\Std.PointsInSphere.Gbx`,
  class 0x09066000: 135 unit-vector sets of n = 4…132, 256, 512, 1032, 2040,
  4112, 8192 points, 24 916 vectors; the first set is the tetrahedron) and
  keep those with `dot(dir, axis) ≥ cos A` — **N uniform directions inside
  the cone of half-angle A** around the light's axis;
* A ≤ 19.5° (the sun): a regular grid of ≈N points inside the disc of radius
  `sin A` around the axis, step `2/sqrt(4N/π)`, each point jittered by a hash
  of its cell (`FUN_1418f6a04`) and the whole grid **rotated by 30°**
  (`0.5235988` at 0x140227146 → the 2×2 rotation at light+0x4e8).

Each sample is one orthographic shadow-map render of the scene and one
`LmLBumpLDir` pass with `OutScale = LightRgb·intensity / (N / (batch))`
(`RenderLightDir` 0x1402381ef); the sum over the samples is the sun term
**accumulated into the frame** (and, through the lit frame × albedo, the
bounce input, §2.4). Shadow casters are filtered by the solids'
`CastShadowGrp0..3` flags (`NPlugSolid2::GetShadedGeoms_CastShadow_IsOk`,
material `ShadowCasterCond`/`NoShadow`) — Nadeo's ground tiles barely shadow
a pad below them [DIFFERENTIAL], i.e. they are receivers only [INFERRED]. The bump permutation writes the three RNM
targets `Tx/Ty/Tz`: `target_i = NdotL · 3 · w_i / (Σw + 1e-4)`, `w_i =
max(0, L_tan · BumpInTgts[i])`; `LmLightSumBumpAvg` = their mean.

### 2.3 Sky and indirect light — a directional depth-peel raycast [DISASSEMBLY]

For each direction `D` of a direction set (`RenderLightIndirectDome`, N =
`m_LightAmbSampleCount` = 256 [FILE]; `PeelDirInW_Scale.w = 4/N` for the RNM
path and `1/N` for the H-basis path, read at 0x140233b94: `[+0x54] = 1/N`,
`fVar24·4.0`):

1. `PeelZDiffuse_p`: render the whole scene orthographically along `D`,
   depth-peeled layer by layer (`TMapDepthToPeel` = the previous layer,
   discard what is not behind it). Each pixel = `TMapILightInput` sampled in
   the surface's **lightmap UV** = the surface's current outgoing radiance
   (§2.4) for **front** faces, **black for back faces**
   (`and o0.xyz, rgb, isFrontFace`).
2. `LmILightDir_Set_p`: for every lightmap texel with `n·D ≥ 0` (others
   discard), project `P` into the layer's view; where the texel is behind
   the layer (`sample_c_lz` on the layer's depth), write the layer's colour
   into `TMapILightDir[texel]`. Texels no layer covers keep the value the
   target was cleared to = the **sky radiance in direction D**.
3. `LmLBumpILighting_Inst_p`: `E += Scale · max(0, n·D) · TMapILightDir[texel]`,
   `alpha += Scale · 0.25`. With `Scale = 4/N` over a uniform sphere set the
   sum over all directions of `Scale·max(0,n·D)` → 1, i.e. **E is the
   cosine-weighted mean incoming radiance** (a uniform sky of radiance L
   gives exactly L; alpha ends at 1). The H-basis permutation
   (`LmLHBasisILighting_Inst_p`) projects the same sample into the four
   coefficients instead (§2.5).

**What the sky actually is** [DISASSEMBLY `RenderLightIndirectPeel`
0x140234df0, `Tech3/Sky_p`+`Sky_v`; DIFFERENTIAL baker 2026-09-23]: the
peel is an ordinary orthographic scene render along `D`, and the scene
includes the mood's **sky dome**, which peels as the farthest layer with the
radiance the runtime sky shader gives it. Nothing is cleared to a constant
"sky colour"; texels that see no surface see the dome. `Tech3/Sky_p`
(constants from `Mood.MoodSetting.xml` `<Atmo><HdrSun Power><Atmo1 Power
Color Scale/><Atmo2 …/>`; the two gradient textures are the two moods the
blender mixes — `TMapGradientV` = mood A's `<Collection>\Media\Moods\<Mood>\
SkyColor.dds` (BC6H_UF16 HDR 2048×1024, 12 mips), `TMapGradientV1` = mood
B's, with ScaleGrad0 = (1 − t)·s0, ScaleGrad1 = t·s1 (t = the blender weight,
s0 = s1 = 1; filler `0x1409f7a40`, NOTES.md §(3)). CAPTURE (pwc-day, BlueBay
Day, DayTime01 0.644 = the Day key → t = 1): t0 = texture 16801 is BlueBay-
**Sunrise**-SkyColor.dds mip 0 and t1 = 16803 is BlueBay-**Day**-SkyColor.dds
mip 0, both BYTE-IDENTICAL to the banked mood files (md5 over the 2 097 152
mip-0 bytes), ScaleGrad0 = 0, ScaleGrad1 = 1. There is NO clouds term in the
dome: PS 16774's 29 instructions are the gradient pair, the sun disc (off),
the two Atmo lobes, the fog lerp, GlobalScale and the 16375 clamp; the mood's
`SkyClouds.dds` (DXT5 1024², a runtime sky-layer texture) is bound in no
lightmapper draw (port engineer G, 2026-09-26; §11 for the cloud sprites)):

```text
uv     = dome vertex uv ; u = GradientV_ForceX ≥ 0 ? GradientV_ForceX : u − LightDirAngle_m11Zx     (azimuth RELATIVE TO THE SUN)
         v = GradientV_InvertY ? 1 − v : v
rgb    = TMapGradientV(uv)·ScaleGrad0  [+ TMapGradientV1(uv)·ScaleGrad1 if ScaleGrad1 > 1e-6]
c      = max(0, dir · −LightDirDirInWorld0)                                    (cosine to the sun)
rgb   += SunIsVisible ? c^SunPower · SunPower · LightDirRgbLinear0 : 0        (the disc: Power 500000 → 0.1°, never hit by a dome direction)
rgb   += c^Atmo1.Power · Atmo1.Scale · Atmo1.RgbLinear + c^Atmo2.Power · Atmo2.Scale · Atmo2.RgbLinear     (the glow; Sunset: 200/3.00042/ff7713 and 3/0.50007/ffaa69)
rgb    = lerp(rgb, Fog_LinearRGB, FogIntens) · GlobalScale ; min 16375
```

During the **first** dome sweep (bounce counter 0, bit 0 of pipeline+0x258)
the peel renders surfaces with the lightmap decode scale forced to 0 and a
sun-visibility scale set to `CHmsLightMapMood+0x24` (1.0), so the peeled
surfaces show `LightDirRgb·max(0,n·−L)·shadow·MDiffuse` — the sun's first
bounce — and the dome shows the sky; both restored to 1.0 afterwards
(`FUN_140234df0` lines 480–530). What the baker measured is exactly that: a
pad under a 16 m plate reads 12 % of an open pad at the Sunset quarter
(the sky radiance is zenith-heavy / the horizon sees dark peeled terrain),
verticals read 0.8 of floors at the Day quarter (bounce). The dome mesh's
uv convention, `LightDirAngle_m11Zx`, `ScaleGrad0/1`, `GlobalScale` and
`FogIntens` at bake time are **pending** (runtime sky constants; the
lightmapper locates the dome object by its material having both the
`GradientV` and `GradientV1` texture slots, `FUN_14028aea0`). Game compass
[DISASSEMBLY, icon-shooter reflection strings]: azimuth 0 = North (+Z),
90 = East (−X), 180 = South (−Z), 270 = West (+X); altitude 0 = horizon.

**The dome direction set** [DISASSEMBLY `RenderLightIndirectBounces`
0x140230ac0 line 917 → `FUN_140236da0(lm, N, sweep)`, `FUN_140234c80`]: the
list at `CHmsLightMap+0x4c8` (count `+0x4d0` = N) is rebuilt **per sweep**
with `N = table 0x141e6f290[quality·6 + sweep]` (§2.7: Default 256 then 128;
High 1024, 512, 256, 128; Ultra 2048, 1024, 1024, 512, 256, 128) from the
`Std.PointsInSphere.Gbx` table object at `lm+0x330` (`FUN_14045fbd0(table,
&set, N)`: binary search of the table's {count, offset} index for N, then
the **nearest** count wins — ties to the smaller — so 1024 → the 1032-point
set, 2048 → 2040, 4096 → 4112, and 256/512/128/64/32 exactly; the set's own
count becomes `lm+0x4d0` and the `Scale = 4/N` denominator; sweep-specific
overrides at `lm+0x500 + sweep·0x10` when present), then every point is rotated by the fixed matrix
`M = Rz(0.313338965) · Ry(0.0599014498) · Rx(0.124326788)` (radians:
17.953°, 3.432°, 7.123°; standard right-handed rotations, `d = M·p`, float32
row·point sums) — and, when `CHmsLightMapMood+0xcc ≠ 0` and
`FUN_14020cde0(lm) == 0`, every direction's y is forced negative (`y = −|y|`,
the set folded onto one hemisphere). A uniform **sphere** set with `Scale =
4/N` makes `Σ_D Scale·max(0,n·D)` exactly 1 for a uniform environment
(downward directions see the peeled ground = bounce, upward ones the sky);
the underside readings (0.37–0.4 of the floor) say the fold is off for the
campaign moods.

**The ISSUE ORDER of a sweep** [DISASSEMBLY, RE child 5 2026-09-25;
`SPlugGroupOfPointInSphere::Compute` 0x140460270 and
`::InsertPointAt_Group_IndexInGroup` 0x14045fdc0; transcribed as
`lightmap::dome::group_issue_order`, verified against the pwc-day capture]:
`RenderLightIndirectBounces` (0x140230ac0 l.926–940) builds one
`SPlugGroupOfPointInSphere` at `lm+0x918` per sweep over the ROTATED list
(`+0x8` = `lm+0x4c8`, `+0x10` = N = `lm+0x4d0`, `+0` = G = ss² from
`FUN_140201580`, ss = 0x141e6f260[q] = {1,2,3,3,3,3} unless
`CHmsLightMapParam+0x128` overrides; nothing is built when ss < 2), and
`RenderLightIndirectDome` (0x140233b50 l.238–246) issues, at issue index
`i = lm+0x4a0`, the direction `list[order[i]]` with `order = grp+0x18` (the
identity when `lm+0x48c < 2 && lm+0x490 < 2`). `Compute` fills `order` by
rounds: `k = 0, 1, …`, within a round the groups `g = 0..G` (a group's size
is `N/G + (g < N mod G)`), slot `G·k + g` — so issue index `i` is group
`i mod G`, rank `i div G`. `InsertPointAt(g, k)` picks, among the points
not yet placed (`grp+0x28` = point → slot, −1 free): `(k=0, g=0)` → point 0;
`(k=0, g>0)` → the point with the LARGEST dot with point 0 (strict `>`,
first wins: the first round is point 0 and its G−1 nearest neighbours);
`(k≥1)` → the point whose largest dot with the points already in group g
is SMALLEST (`maxss` over the members, strict `<`, first wins: a
farthest-point greedy per group, so round 1 starts with the antipode of
point 0). The dots are f32 `fl(fl(y·Y + x·X) + z·Z)` on the rotated
values, and the rotation itself is `M = I; Rx·M; Ry·M; Rz·M` in f32 with
the CRT `sinf/cosf` (`dome::rotation_matrix`, bit patterns pinned in its
test) — near-ties (the 128-set's points 49/116 at round 0, 8/104 at
round 2) flip with anything less exact. Checked: pwc2's 124 sweep-0 draws
are an ordered subsequence of the true order (the capture's counter
skipped 42 of the first 166 directions — the H-basis MRT alpha = the
1/N count says so at all 59 banked snapshots: capture index 9 is issue
10, 29 is 41, 123 is 165), pwc6's last ten sweep-0 directions are issues
246..255 exactly, and pwc6's 127 sweep-1 directions are issues 1..127 of
the 128-set's order exactly (its first block, set point 0, sits unrecorded
in frame 7533's eid gap 13903–28208; the alpha count there starts at
2/128). pwc1's frame 40648 saw set points 9 then 89 back to back = issues
20, 21 of the order. **The 9 raster sub-samples index by the issue
index**: `FUN_14023dde0` (top of every direction) sets `lm+0x494/+0x498 =
(i mod ss², …) = (g mod ss, g div ss)` with `g = i mod ss²` — the group of
the direction — and `FUN_14023de40` turns them into the ST offset
`2·off/W, 2·off/H` with `off` = `FUN_140436200` (the ROTATED grid `a =
(ix − (nx−1)/2)/nx, b = …, x = a + b/nx, y = b − a/nx`; in ninths of a
texel g = 0..8 → (−4,−2) (−1,−3) (2,−4) (−3,1) (0,0) (3,−1) (−2,4) (1,3)
(4,2), the capture's LM01_Trans_RasterSS cycle with y flipped) when the
supersampled raster bitmap `lm+0x708` is absent (H-basis moods: `mood+0xbc`
= 3 releases it, 0x140217e10 l.209–235), else `FUN_140436170` (the plain
grid `(ix − (nx−1)/2)/nx`). The LCG block below is also chained by the
issue index. `PeelDirInW` = the list direction itself. The LCG
`FUN_140238b60` (seed 0x7d3fb6ac at index 0; `x' = (0x3039 − x·0x3e39b193)
mod 2³² & 0x7fffffff`, `r = (x'>>16)/32767`, index k > 0 re-seeds from the
saved state plus one discarded draw) produces per direction a 6-float
block `{0.5u, 0.5v, −(0.05 + 0.45w), c1, c2, −sqrt(1 − c1² − c2²)}` with
(u,v,w) uniform in the unit half-ball (w ≥ 0, rejection) and (c1,c2)
uniform in the unit disc; `FUN_140234c80` adds `CHmsLightMap+0x4a8/+0x4ac`
to its 3rd/4th floats and uploads it as the 32-byte `SetILightDir` vertex
constant — the peel camera's raster jitter, not the direction. The earlier
"30° zenith cone" reading of this document was a fit to the plate
measurements and is withdrawn.

**How layer k excludes layer k−1** [DISASSEMBLY, RE child 4 2026-09-23; peel
camera = a `CHmsVolumeShadow` (scene+0x1b0, class 0x0603C000), matrices
0x140a4c9a0, render 0x140a4fab0]: the compare sampler is GREATER_EQUAL and
`WorldPw01Shadow.z` is bit-identical to the projection z of
`GbxV_WorldPrCamera` (the bias row of `Pw01 = Bias·Proj·View` is `(0,0,1,0)`
for the lightmapper's shadow group: its texel-size z term is gated on
`CHmsShadowGroup+0x4c & 0xc < 4` and the lightmapper sets that field to 8).
The exclusion is the **D3D11 rasterizer depth bias of the layer render**: the
volume-shadow render calls `SetDepthBias(sign·DepthBiasConst,
sign·DepthBiasSlope, 0)` from the lightmapper's `CHmsShadowGroup` (lm+0x628:
the peel function sets Const = 1, Slope = 1.0 — `CHmsLightMapParam+0xfc`, a
default-1 switch — around its layer loop) and the backend (0x140a81220)
negates the three into `D3D11_RASTERIZER_DESC`; `sign = −1` when the volume
shadow has a colour target (the peel), so the layers rasterize with
**DepthBias = +1, SlopeScaledDepthBias = +1.0 on a D32_FLOAT target**:
`d_stored = z + 2^(exponent(max z of the triangle) − 23) + 1.0·max(|∂z/∂x|,
|∂z/∂y|)`, pushed **toward the camera** (reversed z: 1 = near). Layer k+1
keeps fragments with `z ≥ d_k` — nearer to the camera than layer k by more
than the bias, so the same surface fails and the peel is **far-to-near**
(the depth test keeps the farthest survivor). `LmILightDir_Set`'s texel test
`z_texel ≥ d_k` therefore selects the last layer still farther than the
texel: the first surface along `D` beyond it; the texel's own layer is
excluded by the same bias, the 1-pixel slope term covering the
interpolation difference between the LM raster and the layer raster. The
depth lookup is POINT-sampled after a **one-texel inset**: `u_lookup = 0.5 +
(u_render − 0.5)·(w−2)/w` (Bias rows `sx = −0.5·(w−2)/w`, `sy =
−0.5·(h−2)/h`, offsets 0.5). One depth unit = the peel frustum's full depth
(`2·halfD`, frustum copy at vs+0x2d4). The sun shadow maps (§2.2) go through
the same code with `sign = +1` (no colour target): DepthBias −1, slope −1.0,
stored depth pushed away from the light, so `lit = z_recv ≥ z_stored` never
self-shadows.

**The sky dome's radiance** [DISASSEMBLY 0x1409f7a40 (the `Tech3/Sky_p`
ShaderP filler) and 0x1402694c0 (mood → vision constants)]: `SunPower =
HdrSun.Power`, `PowScale1_2 = (Atmo1.Power, Atmo1.Scale, Atmo2.Power,
Atmo2.Scale)`, `RgbLinear1/2 = Atmo1/2.Color` (all from the mood's `<Atmo>`),
`SunIsVisible = HdrSun.Power > 1` **but forced 0 for the lightmapper's dome
pass** (0x140234df0 l.480–522: no sun disc in the bake; the two atmo lobes
stay), `GlobalScale = CHmsLightMapParam+0x24` (default 1.0) for that pass,
`ScaleGrad0 = (1−t)·1.0`, `ScaleGrad1 = t·1.0` with `t` the mood blender's
blend weight between the two gradient textures = the two moods being
blended (a single mood → the SkyColor.dds at scale 1), and `FogIntens =
Fog.Enabled ? <Fog><SkyClouds GlobalIntens> : 0` (BlueBay Day 0.414; the
MediaTracker Fog block's "Sky intensity" is this same field). Shader: `out =
min(16375, GlobalScale · lerp(FogRGB, grad·ScaleGrad + atmo, 1 − FogIntens))`
with `atmo = Σ cos^Power_i · Scale_i · Rgb_i` around the sun direction.


### 2.4 Bounces [DISASSEMBLY]

`GeomILightIn0_p` builds the peel input radiance of a surface point:

```text
ILightInput = ( C0(lightmap so far, decoded) + LightDirRgb · max(0, n·−LightDirDirW) · shadow_LDir0(P) ) · MDiffuse(P)
```

i.e. (the lit frame so far — sun, sky, previous bounces) × the
material's real diffuse albedo (`TMapLM_MDiffuse`, the material textures
rasterised into lightmap space by `Compute_MDiffuse`) — no constant albedo.
`RenderLightIndirectBounces` ("Lighting bounce %1", "%d/%d") repeats the
dome sweep with this input; `ComputeBounces_SpherePoints` (0x140a9e6b4)
draws the sphere directions: `n = min(n, g_MaxSpherePoints)` (global at
0x14205c850; the caller passes 32 with a 64 budget), two passes of a
stratified sphere-point generator (`FUN_14045fbd0(count·n)` then pick the
subset `pass % n`), so consecutive bounce passes use different direction
subsets. `BounceFactor` (2, 1.6 on BlueBay Sunrise, 1.8 Stadium Sunset, 3
RedIsland Night) enters at `RenderLightIndirectBounces` 0x140230ac0 lines
270–278 [DISASSEMBLY]: the four lightmap decode scales at scene+0x8f8..+0x904
(`GbxP_LightGenPHdrScale`, `HBasisHdrScales3`, saved to the state and
restored afterwards) are multiplied by `1/frame.BounceFactor` for the bounce
sweeps — the intermediate lightmap is read back by `GeomILightIn0` through a
scale divided by BounceFactor. How that yields a net ×BounceFactor on the
stored result (the frame's own normalisation) is not yet reconciled —
verify the sign on the baker's underside ratio before porting. The peel's
texture LOD bias is `−0.5·log2(N)` (`FUN_140237fd0`, N = the sweep's
direction count) and `MDiffuse` is the material's diffuse texture rasterised
once per lightmap texel by the `Block_*_PeelDiff` shaders, so the albedo is
a per-texel texture sample, not a material mean [DISASSEMBLY]. **Bounce iterations per quality**
= the table at 0x141e6f248 = {VFast 0, Fast 2, Default 2, High 4, Ultra 6,
Ultra2 6} (`RenderLightIndirectBounces` compares its bounce counter
`[+0x158]` against it) [DISASSEMBLY]. The baker's undersides read 0.37 of
the open floor = BounceFactor 2 × a sea albedo ≈ 0.18 [DIFFERENTIAL].
`DialogComputeShadowsQuality_CheckSaveBounces` is the editor's "save
bounces" checkbox.

### 2.5 Encoding — H-basis, sqrt, YCbCr [DISASSEMBLY `LmCompress_HBasis_YCbCr4_c`, `LightSumRenderToStatic`; runtime `Block_TDSN_COut_DecalMod_p`]

Per texel the bake ends with four HDR H-basis coefficient images
`g_In_HdrRgbHBasis[0..3]` (Habel–Wimmer hemispherical basis, normal axis =
y: H1 = 1/√(2π) constant, H2..H4 = √(3/(2π))·{x, 2y−1, z}). Projection of a
light of colour `c` from tangent-space direction `s` (`sz = n·L`)
[`LmLHBasisDirect_Inst_p`]:

```text
k  = shadow · att · spot · max(0, sz) · 128π / (45 sz² + 64 sz + 17) · ScaleRGB · Rgb
C0 += k · (0.09350571·(3sz²−1) + 0.39892766·sz + 0.19947205)
C1 += k · (−0.23033048·s_a − 0.1619514·s_a·sz)          s_a = first tangent axis
C2 += k · (−0.23033048·sz  − 0.10796642·(3sz²−1))
C3 += k · (−0.23033048·s_b − 0.1619514·s_b·sz)          s_b = second tangent axis
```

(For a light along the normal: `C0 = √(2π)·Rgb`, so `C0·H1 = Rgb`.)

Frame statistics (`LightSumRenderToStatic` 0x14022c3d8): the four image
maxima `max0..3` are reduced on the GPU; then
`s = max(1, max0 / (Mood.MaxHDR · 2.5066283))`, all four maxima ÷ s; the
frame record stores `MaxHDR = max0 · 0.39894226` (= max0/√(2π), i.e. the
frame's peak *irradiance*, capped at `Mood.MaxHDR` — Tiny 16: 2.381 < 3;
Tiny 11: 3.0 = capped) and `MaxHDR_HBasisScaled234 = max1..3 · 0.6909883`
(f16). Compression:

```text
m_i   = max_i / s                                   (per image)
coef 0:  p = min(1, sqrt(max(1e-9, c / m_0)))       per channel
coef 1–3: p = clamp(sign(c)·sqrt(|c| / m_i)·0.5 + 0.5, 0, 1)   (128 = zero)
Y  = 0.2568 R + 0.5041 G + 0.0979 B + 16/255
Cb = −0.1482 R − 0.2910 G + 0.4392 B + 128/255
Cr =  0.4392 R − 0.3678 G − 0.0714 B + 128/255
```

Y per texel for the four coefficients (`Y4`), Cb/Cr per 2×2 block weighted
by each texel's Y/maxY (0.5 where all four are zero). These are exactly the
planes of a VP8 WEBP (`LightMap%u_HSH%c.webp`), so a plain WEBP decode
yields the sqrt-encoded RGB. Runtime decode (`GbxP_LmBumpTxTyTz == 2`,
`LmHBasis_IsFloat == 0`): `rgb_i = YCbCr→RGB` (R = 1.1644Y + 1.5960Cr −
0.8742, G = 1.1644Y − 0.8130Cr − 0.3918Cb + 0.5317, B = 1.1644Y + 2.0172Cb
− 1.0856); `C0 = rgb_0² · LightGenPHdrScale`; `C_i = sign(2rgb−1)·(2rgb−1)² ·
HBasisHdrScales3[i]`; with the tangent-space bump normal `n` (its y raised
to `Bumpiness`, then renormalised):

```text
E(n) = max(0, C0 − C1·n.x + C2·(1 − n.y) − C3·n.z)        flat normal ⇒ E = C0
```

**From the compress shader to the stored images** [DISASSEMBLY
`LightSumRenderToStatic` 0x14022be30 → `FUN_14022b370` → `FUN_14029c450`
(NHmsLightMapBlender, per frame) → `FUN_14029c830`/`FUN_14029add0`/
`FUN_14029bc10`/`FUN_14029bf40`; 2026-09-23]. The compress shader
normalises the **whole frame** by one value — `t4` holds the four frame
maxima, there is no per-chart buffer — `m_0 = min(max0, Mood.MaxHDR·2.5066283)`
(the CPU also scales the four maxima by `1/max(1, max0/(Mood.MaxHDR·2.5066283))`
and stores `MaxHDR = max0·0.39894226`, `HBasis234 = max1..3·0.6909883` in
the 0x44-byte in-memory `SFrame`: +0xc MaxHDR, +0x10 HBasis234, +0x1c
Bounce, +0x20 Sky, +0x24 Clouds, +0x28 StoreLAmbient, +0x2c Storage, +0x30
Switch, +0x34 LAmbient, +0x40 Bump). Its outputs are 8-bit `Y4` (Y of the
four coefficients, at the working resolution) and `Cb4`/`Cr4` at half
resolution. The CPU then builds the stored images:

```text
colour image (frame image 0) = YCbCr_to_RGB_Down2x2 (0x14022af60), size = the chroma texture's:
    Y' = (Y4[2x,2y].c0 + Y4[2x+1,2y].c0 + Y4[2x,2y+1].c0 + Y4[2x+1,2y+1].c0) · 0.25 · 1.1643835         (8-bit values)
    R = Y' − 222.92155 + Cr·1.5960268 ;  G = Y' + 135.5753 − Cb·0.3917623 − Cr·0.81296766 ;  B = Y' − 276.83585 + Cb·2.0172322
    each: v = (int)R ; v < 1 → 0 ; v > 254 → 255                                                       (truncation, no rounding)
grey images (frame-0 image 1, three of them) = channels 1..3 of Y4 at Y4's size:  g = (int)(Y·1.1643835 − 18.630136), same clamp
```

Both stored at 1024² in every file we have; the code as read makes the
colour half the size of the greys, so either the greys are halved on a path
not yet located or the Y4 surface the CPU reads is already 1024² — **open**;
a 1-texel checker item bake decides it (the colour would show a 2×2 box
blur relative to the greys).

**The per-chart byte** (`z4`, one per chart per frame, map-lightmap.md
§3.4) is computed by `FUN_14029add0` on the 8-bit colour image **before**
its WEBP encode, and the image is modified by it:

```text
for every chart i with w,h ≠ 0 (mapping version ≥ 9):
    x0 = floor(x·imgW/W) ; y0 = floor(y·imgH/H) ; x1 = ceil((x+w)·imgW/W) − 1 ; y1 = ceil((y+h)·imgH/H) − 1   (clamped ≥ x0/y0)
        → with imgW = 1024, W = 2048, x odd, w even this is exactly the packer node: node.x/2 … (node.x+node.w)/2 − 1
    byte[i] = max over the rect of R, G and B                                            (empty rect → 0xff)
charts whose position is exactly (x_j, y_j + h_j) of another chart j (same x, starting on j's bottom edge — only the
    contiguous identical-chart groups of big maps; impossible for two packer nodes) form a vertical chain that shares
    the chain's max and excludes its bottom row from the rescale
for every chart with 1 ≤ byte ≤ 254:  every R,G,B in the rect = (uint8)(int)((float)v · (255.0f / (float)byte))      (0 and 255: untouched)
```

So the stored chart is stretched to a 255 maximum in the sqrt domain and
the byte is its pre-stretch maximum: `chartMax_E = (byte/255)² · m_0`
(irradiance `(byte/255)²·MaxHDR`) — the baker's measured rule. Frames 1
and 2 run the same code on their own image 0. The greys are not touched.

**The WEBP encoder** [DISASSEMBLY 0x1404613e0 (the plane export),
`FUN_14029bc10` (colour), `FUN_14029bf40` (greys)]: `libwebp64.dll` is
libwebp **1.6.0** (`WebPGetEncoderVersion` → 0x010600, SharpYuv 0.4.2; the
import table is virtualised by the protector, only `WebPPictureImportRGBX`
shows by name). Both callers build the same 16-byte request `{quality
(float), lossless = quality > 100, 1, 100}` on top of
`WebPConfigInit(WEBP_PRESET_DEFAULT, quality)` (method 4, sns_strength 50,
filter_strength 60, sharpness 0, strong filter, 4 segments, 1 partition, 1
pass, no sharp-yuv). The **colour** image (and frame 1, frame 2, the probe
images) goes through `FUN_140460dd0` = the RGB bitmap import — libwebp does
its own RGB→YUV — at quality **91** (`0x5b`, literal in `FUN_14029c640`/
`FUN_14029c450`). The three **greys** go through `FUN_14029bf40`: Y plane =
channel k of the grey bitmap, U = V = 0x80 planes of ((w+1)/2, (h+1)/2), fed
directly (no conversion), concatenated with their end offsets recorded, at
the quality held in the runtime global `DAT_14205c7fc` (read at
0x14029c51f; a .bss variable with no visible initialiser — the baker's
VP8-header match gives **30**; when `DAT_14205c7f8` is 0 the game writes
one q-80 RGB webp of the three coefficients instead, which no file of ours
shows). `cwebp -q 91` on the decoded Tiny-16 colour image reproduces its
VP8 header exactly (segment quantizers 11/8/6/4, filter strengths 3/2/0/0,
level 3, one partition). Byte-identical WEBPs need libwebp 1.6.0 exactly
plus identical planes.

Frame records [FILE, `clientre lmimages`]: 66-byte `SFrame` × 3 right after
the constant head (no count word): `{u32 Bump, u32 0, u32 DayTime, f32
ReplayTime = −FLT_MAX, f32 MaxHDR_Mood, f32 MaxHDR, f32 BounceFactor, f32
SkyFactor, u32 SkyUseClouds, f16×3 MaxHDR_HBasisScaled234, u32
StoreLAmbient, u32 LocalLight_Storage {0 None, 1 All, 2 OnlyRgbAccum}, u32
LocalLight_Switch {0 On, 1 Off, 2 Unknown}, f32×3 LAmbient}`;
`EHmsLightMapBump = {0 TxTyTz, 1 TxTyTz_Intens, 2 None, 3 HBasis_Color,
4 HBasis_Intens}` (0x141a67120). `DayTime` is the map's own
`0x03043056` word (0xdaab on Tiny 16 = 0.854 of the day).

### 2.6 Local lights (frames 1 and 2) [DISASSEMBLY `LmLBumpDirect_Inst_p`, `ProbeGrid_LightAcc_p`]

Up to 8 lights per pass, each `{IsSpot, IsAtt_HN2, InvCosRange, CosOuter,
PosInWorld, InvRadius, LightRgb, InvRadius2, SpotDirNegInWorld, AttHN2}`:

```text
d = |Pos − P|
att  = IsAtt_HN2 ? max(0, AttHN2.w + 1 / (AttHN2.x + AttHN2.y·d + AttHN2.z·d²))
                 : max(0, 1 − d²·InvRadius2)
spot = IsSpot ? smoothstep(saturate((dir·SpotDirNeg − CosOuter) · InvCosRange)) : 1   (s²(3−2s))
E   += LightRgb · att · spot · max(0, n·dir) · shadow_cube_or_projector(P)      (only where att·spot > 0)
```

`CosOuter`/`InvCosRange` come from the light's cone (the baker's "half
angles" finding stands: `CosOuter = cos(outer/2)`). Frame 1 = `LocalLight_Storage
All`, `Bump HBasis_Color` (only the constant term is written, images 1–2
absent); frame 2 = `OnlyRgbAccum`, `Bump None` (an rgb accumulation used by
the light-index/switch machinery: `LmLIndex_*`, `TBindedMapLM_LListIndex/W/IsLit`,
`GbxP_LmLocals`). At runtime frame 1 is `TBindedMapLM_LocalDirect`, added as
`albedo · LocalDirect · LocalDirectScale01` (no square in the shader — its
load path `CacheLocal_LoadLightMapDiffuse` 0x140213c6f converts; the stored
frame-1 pixels are sqrt-encoded like frame 0 (the same `HBasis_Color`
compress): the baker's frame-1 fit on a linear read gave `(1 − (d/R)²)²`,
which is `att = 1 − d²/R²` read through the sqrt, with the hard cutoff at
`d = R` the shader's `max(0, ·)` produces [DIFFERENTIAL + DISASSEMBLY]).

### 2.7 Quality levels [DISASSEMBLY, tables in .data indexed by `EHmsLightMapQuality`]

| quality | dir (sun) samples 0x141e6f338 | point-light samples 0x141e6f320 | bounce iterations 0x141e6f248 | supersampling per axis 0x141e6f260 | 0x141e6f278 |
|---|---|---|---|---|---|
| 0 VFast | 1 | 1 | 0 | 1 | 1 |
| 1 Fast | 16 | 7 | 2 | 2 | 1 |
| 2 Default | 64 | 25 | 2 | 3 | 1 |
| 3 High | 64 | 25 | 4 | 3 | 1 |
| 4 Ultra | 64 | 25 | 6 | 3 | 2 |
| 5 Ultra2 | 64 | 25 | 6 | 3 | 2 |

The supersampled render target is the atlas size × the per-axis factor
(0x140217e10: `param+0x128` overrides the table). The 2-D table at
0x141e6f290 (6 words per quality: {0…}, {64, 32, 0…}, {256, 128, 0…},
{1024, 512, 256, 128, 0, 0}, {2048, 1024, 1024, 512, 256, 128}, {4096,
2048, 1024, 512, 256, 128}) is the **dome direction count per sweep**
[DISASSEMBLY `RenderLightIndirectBounces` line 917 → `FUN_140236da0`;
`NLocal::ComputeInGameplay3` 0x140a40ae0 line 284 sums the same row]: the
"bounce iterations" column above is the number of sweeps and this row gives
each sweep's N — a Default bake is one 256-direction sky+bounce sweep plus
one 128-direction bounce sweep; High is 1024 + 512 + 256 + 128 (the Tiny 16
q4 reference). The stored `m_LightAmbSampleCount = 256` is Default's first
entry. `NLocal::ComputeInGameplay3` also sets `lm+0x4c = 10` and runs the
same tables with the client's quality setting; the texel budget
(`WantedTexelByMeter`) per quality is still pending. Floats/ints after the
tables at 0x141e6f350: 3.1e-05, 1.0, 32767, 5000, 1000, 200, 100, 0.2, … (the
in-gameplay time budgets, not read).

## 3. Charts, packing, probes

### 3.1 Chart allocation — the walk [DISASSEMBLY, typed decompile banked as client-re/decomp2-typed.tgz; every constant below read off the code, 2026-09-23 RE child 2]

Call chain: `NHmsLightMap::UpdateMapping` 0x14020f510 → `AllocateBlocks`
0x14028f6e0 → `AllocateBlocks_` 0x1402901a0 → `AllocateWithScale_`
0x140294830 → (chart list `FUN_140291450`, grouping `FUN_1402938c0`) →
`AllocateWithScale_BlockSplit` 0x1402954f0 → `TryPack` 0x140295d30 → the
binary-tree rect packer 0x140492750/0x1404927b0/0x1404928b0/0x140492a80 →
write-back `SetUvTransfo` 0x1402923b0 (or `FUN_140291f20` in simple mode).
(The probe/sprite atlases go through the same `AllocateBlocks_` with alloc
mode 2 → 0x140296000: per-chart (w,h) = the model's stored `spriteCount`
pair, stable sort by area, largest first, atlas side = `ceil(sqrt(Σwh +
0.5)·1.01) + 1` grown by 1 until everything fits — that is why the probe
images are 198×194-ish, not powers of two.)

**The layout size, granularity and pad are not passed in — they are
derived** (RE child 1 read `UpdateMapping`'s 5th argument as dims; it is the
block array `{ptr, count}` at `CHmsLightMap+0xd8/+0xe0`):

```text
W = H = {1024, 2048, 4096}[T[quality]]      T = table 0x141e6f278 = {VFast 1, Fast 1, Default 1, High 1, Ultra 2, Ultra2 2}
                                            (FUN_14020dd80 → FUN_14020dd90, stored at SGlobal+0x478 by RenderLighting_Frames 0x14021e340;
                                             ComputeLighting 0x14021a9b0 lowers the index to 0 (1024) when VRAM < 0xC400000 B and to 1 when
                                             VRAM < 0x2BC00000 B; a re-bake with a valid cache reuses the cache mapping's stored W,H)
k  = (max(W,H) | 1024) >> 10 ;  lg = bsr(k) + 1          (FUN_14028f190, called by AllocateBlocks_ right after the dims copy)
g  = 1 << (lg − 1)                                       granularity   → 2048: 2   (1024: 1, 4096: 4)
pad = 1 << max(0, lg − 2)                                border        → 2048: 1   (1024: 1, 4096: 2)
m  = roundup(max(4·pad, 6), g)                           minimum chart side (FUN_140295ce0) → 6
```

The layout unit is therefore **half a stored texel** at 2048 (the images are
1024²): `pad = 1` is the half-texel centre inset, `g = 2` makes every chart a
whole number of stored texels, and `m = 6` = 3 stored texels.

**Per chart** (`FUN_1402917e0` + `GetPreLightGenMeterByUv` 0x14028f480):

```text
f      = PreLightGen.MeterByUv (float +0, our "u02") × blockScale     (uv set 0 bounds at PreLightGen+4..+0x10 = u0,v0,u1,v1;
                                                                       alloc mode 1 uses the second set at +0x14..+0x20;
                                                                       per-sub-visual table at PreLightGen+0x40 (stride 0x14: {f, u0,v0,u1,v1}) when flags & 0x10000;
                                                                       the merged group's bounds and average f when flags & 0x20000)
blockScale = (kind 0 block: BlockInfo float at model+0x98→+0x3c ; items: 1.0) × qualityByte/255       (FUN_14021d8b0, FUN_14021d8e0)
ext    = ((u1−u0)·f, (v1−v0)·f)          metres; non-finite → (0,0)
area   = ext.x · ext.y                    m²
```

The quality byte is the per-element `MapElemLightmapQuality` (chunk
`0x03043068`, block+0x9d / item+0xc2) through `FUN_140dcc1c0` =
`(√2)^e · G`, `e = {0 Normal:0, 1:+1, 2:+2, 3:+3, 4:−1, 5:−2, 6:−3, other:0}`
(FUN_140dcc160), `G` = 1.0 for the map's own objects, 0.0625 for the
decoration challenge's objects on the Stadium-family collection id, 0.5 on
the others (`CGameCtnApp::HmsLightMapUpdateBlocksAndItemsQuality`
0x140dcc290); `byte = clamp(int(f·255), 1, 255)` (values < 2 → 1) into the
scene-bound lm record +0x38. A terrain-class block also gets an 8-byte
neighbour mask at +0x40 (its quality blended with each of the 8 neighbouring
tiles: `(q_self + q_nb)/2`, 0 when either < 0.01) — the packed-geometry
charts.

**Which placed things get a record at all** [DISASSEMBLY, RE 7 2026-09-25; `tools/lightmap/src/itemrule.rs`]:
the three record builders append to ONE array (CHmsLightMap+0xa8/+0xb0, 0x58-byte records) that `AllocateBlocks_`
packs and `SHmsLightMapCacheMapping` (FUN_140288530 ← FUN_14027f1d0 ← RenderLighting_Frames) writes to the file —
every record, in radix-key order, so a thing without a record is neither packed nor in the chart table:

* kind 2 — a STATIC item (CPlugStaticObjectModel → the zone's static pool, FUN_1401e9a50 → FUN_14020e7b0): the
  Solid2Model's PreLightGen must exist with `u01` ≠ 0 (the file's i32, kept as the byte PLG+0x50), uv-set-0 bounds
  non-degenerate (u0 < u1 and v0 < v1) and the model's LIGHTMAP GEOMETRY non-empty (FUN_14045b730: the LOD-selected
  indexed-triangle visuals whose material has a lightmap texcoord set (compiled material +0x40 bits 15..19 / material
  +0x34 ≠ −1), positions ≤ that set's count) — else the warning "Mesh has invalid LightMap texture coordinates,
  lighting may be wrong." and NO record (tiny 16's AC16497076 with one uv set: kept in the editor bake, no chart).
  The model entry is also skipped when a material carries CPlugMaterial+0x18 & 0x6000 while the BSS knob
  DAT_14205c82c is on ("CustomLM"; no checked game material file has those bits — chunk 0x09079016 flags = 0x240).
  The record's quality (+0x50) is the pool entry's FLOAT (entry+0x34 ← FUN_1401eab80), NOT a byte.
* kind 0 — a CHmsItem mobil (a CPlugSolid: legacy items, dyna/moving solids, the forest's per-tree solids via
  FUN_14026dd00; the Solid2Model → CPlugSolid conversion FUN_1401fcc30 copies the PreLightGen whole): record iff the
  solid's PreLightGen exists with u01 ≠ 0 (FUN_14020e3c0). Quality = `clamp(int(f·255), 1, 255) / 255`.
* kind 1 — the CHmsZone+0x1f8 manager's per-model entries (FUN_14020eea0): PreLightGen with u01 ≠ 0 → a record
  with quality 1.0, flags 9, centre 0 / half −1; a stored sprite size w·h ≠ 0 only gets a caster record (+0xb8).

Quality per item (FUN_140dccde0): `f = powf(√2, e)·G`, e from the MapElemLightmapQuality byte {0:0, 1:+1, 2:+2,
3:+3, 4:−1, 5:−2, 6:−3}; a static-pool item stores f as the FLOAT (its chart scale), a CHmsItem the byte.
Area per record = `ext.y·ext.x` with `ext = (Δu·f, Δv·f)`, `f = MeterByUv × blockScale` (no placement scale;
non-finite f → (0, 0) → the `m·mins` chart; f < 0 → 0.1). The 501 chart-less items of tiny 16 are NOT a game
rule: they are the items `tmmaps keepitems` dropped before the editor bake (refs/tiny16-reduced-kept.txt; the
"light-carrying" correlation is that filter) — only the two 1-uv-set items are the game's exclusion.

**The chart list** (`FUN_140291450`): one 0x18-byte record `{model*, u32
flags|subvisual, u32 blockparam, u32 blockIndex, u32 group}` per 0x58-byte
block record when the model's sub-visual count (`PreLightGen+0x48`) is < 2 —
`flags = 0, group = −1`. A model with n ≥ 2 sub-visuals (the
`Item_Prefab_MultiMesh` case) gets **two** records: `{flags 0x10000 | 0,
group −1}` = sub-visual 0 on its own (its own `uvGroups[0]` = {f, u0, v0, u1, v1}: the file's uvGroups are
count × 20 bytes, not [i32;4]), and `{flags 0x20000, group g}` = sub-
visuals 1..n−1 merged: `FUN_14028f1d0` packs their uv rects into one rect
(`FUN_141402b00`, seed constant 1, spacing 0.015) and stores the average of the FINITE `MeterByUv`s (Σf/n, f32) and the
merged bounds in the group record (stride 0x28; +0x18 → the per-sub-visual
ST table used by `SetUvTransfo`). Tiny/campaign items are single-visual:
one chart per item.

**Simple mode** (`AllocateWithScale_` sets state+0x138 when no chart has a
group or the 0x30000 flags; always true for our maps): the charts are first
grouped by `FUN_1402938c0`: charts with the same `(model, blockScale)` and
`ext.x·D ≤ 100 && ext.y·D ≤ 100` (D = W·H/Σarea, in (layout/m)²; a
dimensionally odd test, but that is the code) share a group laid out as a
`cols × rows` grid (`FUN_140293d70`; contiguous cells, only the last column/
row inset by 2·pad, `FUN_140291f20`); the Stadium `DecoWall`
`Base_VFCMiddle_Air.Prefab.Gbx` pieces are strip-merged (`FUN_14028ff90`
entry+0x24). On the tiny maps D is in the hundreds, so **every chart is its
own group** and the packing below sees one rect per chart.

**Density and scale search** (`AllocateWithScale_BlockSplit`):

```text
D = W·H / Σ area                      (layout units/m)²  (fixed-density mode instead uses WantedTexelByMeter² — alloc mode 0 with dims+0x10 > 1e-9; not the editor)
scale_hi = 1.0 ; if sqrt(D)·max_ext.x / W > 1 or sqrt(D)·max_ext.y / H > 1: scale_hi = 1 / max(ratio)²
if (W/m)·(H/m)·0.9 < N: scale_hi = 0.01                               (integer divisions)
iter = 0
loop:  iter++ ; scale_lo = scale_hi·0.9 ; s = sqrt(scale_lo·D)
       if TryPack(s) succeeds: break
       scale_hi = scale_lo
       if the largest-area chart has ext.x·s < m or ext.y·s < m: break   (nothing left to shrink)
iter = min(iter, 1)
bisect while iter < maxIter[quality] = {VFast 1, Fast 3, Default 6, High 8, Ultra 10, Ultra2 10}:
       iter++ ; mid = (scale_lo + scale_hi)/2 ; s = sqrt(mid·D)
       TryPack(s) into the spare packer: success → scale_lo = mid, swap packers ; failure → scale_hi = mid
result: s_final = sqrt(scale_lo·D) (result+0x14, "AllocatedTexelByMeter" in layout units per metre), the last successful packer
```

**TryPack(s)** (0x140295d30). The order array is the state of the stable
LSD radix sorter 0x14012c850 (4 byte passes over the float bits, sign-aware,
a pass skipped when the keys are already ordered); `AllocateBlocks_` feeds
it four keys with N = block count — `|halfExtent|²` (block +0x44..+0x4c),
then block +0x38, +0x3c, +0x40 — and `BlockSplit` adds the chart **area**
last, so the order is **ascending area with ties broken by +0x40 (world
centre z), then +0x3c (centre y), +0x38 (centre x), |halfExtent|², then the
block-array index** (when the chart count differs from the block count the
sorter resets: area then index only). TryPack walks `order[N−1] … order[0]`,
i.e. largest area first and, among equal areas, the chart with the largest
centre z first (then largest y, largest x, largest index) [DIFFERENTIAL
baker 2026-09-23: all 34 items of the BlueBay test bake match size and
position with these keys].

The record fields are the **world AABB of the solid model's own bbox**
[DISASSEMBLY `FUN_14020e3c0` (CHmsLightMap add-from-mobil) →
`FUN_140185f70(&rec+0x38, solid+0xc8 = {centre, halfExtent}, mobil+0x20 +
k·0x30 = the 4×3 world matrix, k = FUN_140941ec0())`]:

```text
centre_w = (m00·c.x + m01·c.y + m02·c.z + T.x,  m10·c.x + m11·c.y + m12·c.z + T.y,  m20·c.x + m21·c.y + m22·c.z + T.z)   → +0x38, +0x3c, +0x40
half_w   = (|m00|·h.x + |m01|·h.y + |m02|·h.z,  |m10|·h.x + |m11|·h.y + |m12|·h.z,  |m20|·h.x + |m21|·h.y + |m22|·h.z)   → +0x44, +0x48, +0x4c
+0x50 = qualityByte/255 ; +0x54 flags: 0x10 = PreLightGen uv-set-1 bbox non-degenerate, 0x8 = no PreLightGen (else 0x20), 0x4 = material class DAT_141e7c768
```

float32 in that order (the abs is an `andps` with 0x7fffffff). For a zone
tile the solid is the tile mesh, so centre.y is the mesh's mid-height (sea,
shore and land tiles at one z sort by height before x) and a mesh that
overhangs its cell shifts the centre. The record is appended through
`FUN_140248710(mobil+0xd0, lm+0x90, lm+0xa8)` in bind order, which is the
final tie-break.

```text
reset packer to (W, H); fail if m²·N ≥ W·H
carry = 0
for k = N−1 … 0:  i = order[k]
    if ext.x == 0 or ext.y == 0: (w,h) = m·mins[i]                       (mins = (1,1) for every chart in simple mode)
    else:
        a  = ((ext.y·s)·ext.x)·s                                          float32, in that order
        Fit(A): t = sqrtf(A / (ext.x·ext.y)); w = (int)floorf(ext.x·t); h = (int)floorf(ext.y·t)     ← FLOOR (FUN_14028f570 / floorf 0x14195c7b8), not ceil
        (w0,h0) = Fit(a) ;  (w1,h1) = Fit(a + max(carry, 0))
        w' = w0 + (w0 < w1) ; if w' % g: { w = w' − w' % g ; if w' < w1: w += g } else w = w' ; w = max(w, m·mins[i].x)
        h' = h0 + (h0 < h1) ; if h' % g: { h = h' − h' % g ; if h' < h1: h += g } else h = h' ; h = max(h, m·mins[i].y)
        carry += a − float(w·h)                                         (the floor leaves a POSITIVE deficit that the next chart pays back with +1)
    if !insert((w,h), i+1): fail
success
```

This is the origin of the ±1/±2 bumps the baker measured (26 identical pads
at 302 and one at 304): the first charts of a run of equal areas are floored
and a later one absorbs the accumulated deficit.

**The packer** (0x140492a80, the classic binary-tree lightmap packer; node
= `{u16 x, y, w, h; ptr user; i32 child0, child1}`, root = (0, 0, W, H)):

```text
insert(node, w, h):
    loop: if w > node.w or h > node.h: return −1
          if node has children: r = insert(child0, w, h); if r ≠ −1: return r; node = child1; continue
          if node.user: return −1
          if w == node.w and h == node.h: return node                   (caller sets user, used += w·h)
          dw = node.w − w ; dh = node.h − h ; allocate child0, child1
          if dw > dh: child0 = (x, y, w, node.h),   child1 = (x + w, y, dw, node.h)       (vertical cut)
          else:       child0 = (x, y, node.w, h),   child1 = (x, y + h, node.w, dh)       (horizontal cut)
          node = child0
```

**Write-back** (`SetUvTransfo` 0x1402923b0; simple mode `FUN_140291f20`,
identical for singleton groups): for every packer node with a user,
`pad = result+0x20` (set by FUN_14028f190; TryPack forces 1 when 0):

```text
x = node.x + pad ;  y = node.y + pad ;  w = node.w − 2·pad ;  h = node.h − 2·pad      (+ a sub-atlas origin when given)
layout[idx] = {i16 x, i16 y, i16 w, i16 h}                     ← the per-chart rect of the mapping (map-lightmap.md §3.3): x,y odd, w,h even
uvTransfo[idx] (FUN_140200970): e = W/16384 ;  ox = (x + e)/W ; oy = (y + e)/H ; sx = (w − 2e)/W ; sy = (h − 2e)/H
                                composed with uv → (uv − u0)/(u1 − u0)          (charts with w or h = 0 get {0, 0, −1, −1})
```

In stored-texel terms the uv range [u0,u1] maps to
`[node.x/2 + 0.5 + 1/16, node.x/2 + node.w/2 − 0.5 − 1/16]`: the chart's
edges sit on the **centres** of its first and last texels (the classic
bilinear inset); the node IS the chart's texel footprint, there is no gutter
between charts beyond the packer's own free space.

### 3.2 The probe volume [DISASSEMBLY `ProbeGrid_*`, FILE map-lightmap.md §3.8–3.9]

Probes live on the 16 m (or 32 m) cell grid described in map-lightmap.md
§3.8; each probe is offset by `TMapProbeSafetyOffset` (a 3D texture computed
by `ProbeCpt_SafetyOffset_Compute` 0x1402285e0 — pushes probes out of nearby
geometry). Per dome direction `D` (the same peel layers as §2.3):

* `ProbeGrid_AddSkyVisibility_p`: `+= OutScale` when the probe passes the
  direction's shadow test → **image 1 = the fraction of directions that see
  the sky** (the "occlusion" image).
* `ProbeGrid_SetILightDir_p`: `+= OutScale · colour of the first surface in
  front of the probe` (w += 1 where that colour is non-zero) → the incoming
  radiance → **image 0 (sky + bounce irradiance at the probe)** and the
  `ProbeGridCpt.Bitmap_ILightDir` accumulation.
* `ProbeGrid_SetIsValid_p`: `+= OutScale` where the first surface in front
  of the probe is **front-facing** (its colour sum > 1e-6; back faces are
  black) — a probe that mostly sees back faces is inside geometry →
  **the validity mask** (`ProbeGrid_ComputeIsValidSS` thresholds it).
* `ProbeGrid_LightAcc_p`: `+= shadow · att · spot · LightRgb` per light, no
  cosine → **image 3 (the local lights)**; for the sun the same shader with
  `IsLightPos = 0` gives the sun visibility.
* `ProbeGrid_LightWeightMax_p`: the max light weight over cSamplePerAxe³
  jittered positions in the cell (for the light list).

**Encoding of the four probe images** [DISASSEMBLY 0x14022d8f0, the probe
grid download ("!! ProbeGrid download crash helpers")]: the f16 probe
textures are converted on the CPU through the **sRGB** table at
0x141a64760 (4096 entries, the exact sRGB curve; inverse table of 256 floats
at 0x141a64360): `byte = sRGB(value / scale_k)`, `scale_k` = the image's
maximum = `frame_info[k].scale` of the trailer; the fourth channel of the
first image is the linear f16 alpha rounded to a byte, and a validity byte
is set where that alpha ≥ 0.5 (zero pixels otherwise). Not sqrt, not linear.

Image 2 (pale bluish, grey inside geometry): the remaining accumulation is
`LmILightDir_AddAmbient_c` — the ambient/sky **without** occlusion (the
`AmbientAtTop`-shaped term) — **[INFERRED]**; runtime consumers:
`ProbeGrid_Sample_c`, `ProbeGrid_Sample_CbTrans_c`, `DynaBox_*` (the car's
lighting: `DynaBox_SetLightAmb_FromHandle_c`, `DynaBox_ILightDir_SetFromPeel_c`).
Sprites (vegetation) get their own SH: `LmSpriteSS_LightAtt_p` (9 jittered
positions in the sprite's volume) → `LmSpriteSS_ResolveAndProjSH_p`.

## 4. Object order [DISASSEMBLY `CGameCtnChallenge::AutoSetIdsForLightMap` 0x140b88a1c, `NGameMgrMap::IdsForLightMap_BindToScene` 0x140dc5c4c]

The chart→object index is the object's `IdForLightMap` (`block+0x98`,
`item+0x168`), assigned by `AutoSetIdsForLightMap` in one pass:

```text
challenges = [the decoration's own map (Stadium: Deco48x48*.Map.Gbx) if the decoration has one, then the map]
id = 0
for each challenge C:
    for b in C.Blocks (+0x278):              if lightmappable(C, b): b.id = id++
    for b in C.BakedBlocks (+0x288):         b.id = id++                    (no filter)
    for a in C.AnchoredObjects/items (+0x2a8): a.id = id++
    if C is the decoration map (+0x828 != 0): id = max(id, 0x4000)         → the map starts at 16384
```

`lightmappable(C, b)` (0x140b88500): a block with no BlockInfo counts;
otherwise **no id** for a `CGameCtnBlockInfoClip` (0x03053000) or
`CGameCtnBlockInfoFrontier` (0x03055000) block, and no id for a
terrain-class block (three BlockInfo classes, 0x140f31840/850/860) whose
cell holds another block that passes `FUN_140d2b3d0` (a ground block
replacing the tile). Everything else in the authored list counts, embedded
custom blocks included only when they have a BlockInfo of a counted class.

`IdsForLightMap_BindToScene` writes the bind words the mapping stores
(map-lightmap.md §3.6): for every visual (mobil) of a block or item
`word1 = id·4 | (mobil & 3)`, `word0 = 0`; for the terrain **packed
geometry** of a baked block (`CHmsZoneVPacker`, blocks merged per zone)
`word1 = id·4`, `word0 = geomIndex | 0x10000000` — **bit 28 marks a
packed-geometry chart**, which is why Nadeo's maps have them (15 %) and
tiny/item-only maps have none. Baked blocks flagged `0x1000` at +0x90 are
skipped in the bind (they keep their id).

Consequences: `base(items) = P + |Blocks counted| + |BakedBlocks|` with
P = 16384 when the decoration ships a map — exactly the measured rule
(`P + N_authored + (S_x·S_z − replaced) + G`): the generated ground tiles
and the clip fillers/pillars/aprons are the game's `BakedBlocks` list at
bake time, in generation order. `G` is therefore predicted by whatever
reproduces that list (`mapgeom bake` for the clip fillers; the Stadium apron
and pillar generators are the remaining part), not by anything in the
lightmapper. `TransferIdForLightMapFromBakedBlocksToBlocks` (0x140b91f00)
carries ids from a file's baked records onto regenerated blocks so a stored
cache survives a regeneration; `LightMapGetMostRecentBlock` (0x140b965d0)
feeds the `MostRecentBlock` timestamp the cache compares.

## 5. At load

`CGameCtnChallenge` load (0x1413651ac): a map whose header `lightmap` version
is **< 8** is refused with "Map lightmap is not up to date." unless the
client runs with `/allowoutdatedlightmaps` ("will still load for now")
[DISASSEMBLY]. A stored cache is then validated against the scene; every
reason is a string at 0x141b68e08–0x141b693b0: `Version is not up to date`,
`AllocMode/SortMode/MapCount/IdCollection/IdDecoration/ChallengeJointId/
Bump/BumpMode/TimeOfDay/CompressMode has changed`, `Quality is not enough`,
`LDirQ/LPntQ has changed`, `Mood.BounceFactor/MaxHDR/SkyUseClouds/SkyFactor
has changed (was %.2f, expects %.2f)`, `Dynamic DayTimes count changed`,
`One DayTime has changed`, `TotalLmSurfaceMeter has changed`,
`TimeWriteMostRecentSolid has changed`, `WantedTexelByMeter has changed`,
`Spread in Invalids`, plus the `EQualityVer` regressions (`Bounce shadow was
filtered`, `Ultra (>2048) mapper was bugged`, `Local lights were wrongly
rotated`…). A rejected cache → the coarse load-time recompute
(`NHmsLightMap::NLocal::ComputeInGameplay2/3`, the "Local" path with its own
`AllocLmBlockTexels`/`RenderLightIndirect`) — its parameters **pending**.
`hasLightmaps = 0` takes the same path.

**The editor crashes on some of our maps — five distinct sites in the box's
Application event log (Event 1000, module Trackmania.exe 2026.2.2.1751,
read 2026-09-23; offsets are exe-relative, +0x140000000):**

| when (UTC) | code | site | what it is [DISASSEMBLY] |
|---|---|---|---|
| 09-22 21:01–21:22, 9× (the WhiteShore tiny copies / maps 20, 24 window) | c0000005 | 0x280bf4 in `FUN_140280190` | the frame → texture upload: per chart rect it reads 3-byte pixels from the frame's image 0 and image 2 (`FUN_1401d5a50` slices the three images by the frame's end offsets [0x15..0x17], `FUN_140460940` = WebP decode) and blends them with the frame weights; the rect comes from **image 0's** size (`local_170`) and the faulting read is image 2's pixel — an image 1/2 **smaller than image 0** (or a chart rect outside them) reads past the bitmap. A frame whose three images do not share image 0's dimensions crashes at upload, i.e. ~9 s after the map opens, before any compute. Fix in the converter: never emit a frame whose images differ in size (or drop the chunk, `hasLightmaps = 0`, and let the coarse load-time bake stand in). The chart **sort** cannot be it — it is a stable radix sort with no data-dependent branch. |
| 09-15 05:08–16:59, 9× | c0000005 | 0x456513 | generic container/renderer code, not lightmapper |
| 09-22 22:44–23:31, 4× | c000001d | 0x11da01 (`ud2`) | a deliberate fatal trap (assert) |
| 09-23 07:03–07:43, 3× | c000001d | 0x2d1d14 (`ud2` in `FUN_1402d1d00`) | fatal: `FUN_1402e7e00(classId, obj)` returned 0 — a class-id lookup failed while loading a Gbx (an unknown/unsupported node class in a hand-built file) |
| 09-23 06:09–06:58 | c0000005 | 0xa51d4c (`Shadow_RenderDelayed`), 0x5385e3 (array grow), 0x18d768c (CRT copy), 0xa0d308 (refcount on obj+0x100) | dangling pointers in the renderer during/after a compute — use-after-free of a render pipeline object; not reproducible from file content |

The event log is read with one PowerShell call through the bridge
(`Get-WinEvent -FilterHashtable @{LogName="Application"; Id=1000}`), no
render lock needed; do that first for any new crash instead of guessing.

## 6. What the game reads from the mood

* `Mood.MoodSetting.xml` (pack `Media\Moods\<Mood>\`): `LAmbient HdrColor`,
  `LDirSun HdrColor`, `T3LightMap MaxHDR BounceFactor SkyFactor SkyUseClouds`
  — the **effective mood is the quarter of the map's DayTime word**
  (`0x03043056`: [0,¼) Night, [¼,½) Sunrise, [½,¾) Day, [¾,1) Sunset)
  [DIFFERENTIAL, baker: the 25 sources' frame triples; Tiny 16's record
  carries the Sunset triple although its decoration is "Sunrise64"].
* `<Mood>.DecorationMood.Gbx` chunk 0x0303A000 `{Latitude, Longitude,
  DeltaGMT, TimeSunRise, TimeSunFall}` — the sun path parameters (BlueBay/
  RedIsland Day (35, 2, 1, 6:00, 18:00), Sunrise (63, 0, 0, 10:00, 14:00),
  Sunset (30, 2, 1, 10:00, 14:00), Night (48, 0, 0, 6:00, 18:00); Stadium
  (45,2,0,6–18), (48,0,0,6–18), (45,0,0,10–14), (65,2,1,10–14)) and
  0x0303A012 `RemappedStartDayTime` [FILE]. The XML `Latitude` (20/36/45) is
  not the one the sun uses. The sun direction as a function of DayTime through
  these is **pending**; the baker measures the sun from the east at el ≈ 1–2°
  at DayTime 0.854 on BlueBay (Sunset quarter) [DIFFERENTIAL].
* **The sun's inputs, pinned** [DISASSEMBLY 0x14028cd20 (the lightmapper's
  mood → sun direction), 0x140494690/0x140494810, RE children 3+4]: the day
  fraction `b = clamp((t − SunRise)/(SunFall − SunRise))` comes from the
  **decoration's `CPlugMoodBlender`** curve (`GameCtnDecoration\<hash>`, class
  0x0911A000, an XML stored as the pack's only counter-mode-encrypted entry —
  pak-nadeopak.md §2), and the direction `D = (cos πb, −cos(lat)·sin πb,
  −sin(lat)·sin πb)` uses **the mood XML's `<Light Latitude>`** (`mood+0x1c`:
  20 for the BlueBay moods), NOT the blender's. The decoded blenders [FILE,
  banked in tm-player/tiny/lightmap-re/moods/]: BlueBay, RedIsland, Stadium
  `Latitude="47.5" SunRise="06:00:00" SunFall="21:00:00"
  LocalLight_SwitchOff="06:30:00" LocalLight_SwitchOn="18:30:00"`; GreenCoast
  the same with `SwitchOn="19:10:00"`; WhiteShore `Latitude="55"`; all with
  `<MoodWeights>` keys at X 0.08/0.2/0.55/0.69, weight 0. The blender's
  Latitude is stored in the curve (+0) but not read by the two functions
  above.
* `LightMap\HmsPackLightMap\Tech3_HDR_PSSM.PackLightMap.Gbx` (Maniaplanet.pak,
  class 0x06021000) is the lightmapper's pack configuration: it references
  the sphere point table, `LightMap\HmsPackLightMapMood\Tech3
  MoodSettings.PackLightMapMood.Gbx` (`CHmsLightMapMood`, class 0x06023000:
  chunk 0x06023000 = five floats (1, 1, 0, 1, 1) at +0x18…+0x28; chunk
  0x06023004 v0 = MaxHDR 3, BounceFactor 1, SkyFactor 1, SkyUseClouds 1,
  then 4.0, 0.01, 3.0 at +0x40/+0x44/+0x48 — the defaults the XML
  `T3LightMap` overrides; the last three are lightmap-only constants whose
  use is pending), the working textures (ColorPeeled, ILightInput,
  ILightDir, Shadow1, DepthToPeel, ProbeBBoxInvHDiag, ProbeGrid…) and the
  sprite shaders [FILE].
* `Techno3\MotionManagerWeathers\DayTime.MotionManagerWeathers.Gbx` ("Sunny")
  binds the lights' day curves (`LightAmbSunny.tga`, `LightSunSunny.tga`,
  `LightMoonSunny.tga`), `Techno3\Func\FuncDayTime\Default.FuncDayTime.Gbx`
  (class 0x09181000: 50000, 50, 0.25, 5, 0.125; 0.75; …) and the default
  mood XML (`Latitude 35, DayTime01 0.644, LAmbient cce2ff × 0.815, LDirSun
  fffef1 × 2.86, T3LightMap MaxHDR 5 Bounce 3 Sky 0.3`) [FILE].
* The sky the dome pass renders comes from the mood folder
  `<Collection>\Media\Moods\<Mood>\`: `SkyColor.dds` (BC6H_UF16 2048×1024,
  12 mips — `TMapGradientV`; the other blended mood's is `TMapGradientV1`, §6e), `SkyClouds.dds` (a runtime sky-layer texture, in no lightmapper draw),
  `AmbCubeP.dds`, `EnvCubicHdr.dds`, `Clouds.tga`, `Mood.MoodSetting.xml`
  (`<Atmo><HdrSun Power><Atmo1/2 Power Color Scale>` = the `Sky_p` lobes,
  `<Fog Color IntensMax …>`). All 20 collection×mood sets are banked under
  `client-re/moods/<Collection>-<Mood>-<file>` (`mapgeom raw <pack path>
  --out F`). `Techno3\MotionManagerWeathers\Default.MoodSetting.xml` and
  `Techno3\Media\Texture\Image\DefaultSkyGradV.dds/.exr`,
  `DefaultEnvCubicHdrScaleA2.dds`, `Techno2\…\DefaultCubeAmbientP.dds`,
  `Clouds\…\Cumulus02.tga` are the fallbacks the blender loads
  (`FUN_14028aea0`) when a scene has no mood [DISASSEMBLY].
* The **surroundings** the peel sees (BlueBay's island, sea, cliffs) are the
  decoration's `Scene3d`: `<Coll>\GameCtnDecoration\Base64x64<Mood>.Decoration.Gbx`
  → `Size\64x64.DecorationSize.Gbx` (0x0303B000, chunk 0x0303B002 = {4, 64,
  64, 64, 1, 1}) → `Scene3d\Base64x64.Scene3d.Gbx` (class 0x0A003000, 75 KB
  in BlueBay.pak). That entry **fails the pak reader's fold hunt** ("bad
  match offset 7755 (hist 5273) in the chunk at compressed offset 1749",
  budget 3 M tries) — the one file the baker needs for the horizon
  geometry is not extractable with mapgeom as it stands; a fix to the LZ4
  fold reader (pakfile.rs) is the way to it.
* Casters [DISASSEMBLY `NPlugSolid2::GetShadedGeoms_CastShadow_IsOk`
  0x1401fd390]: each shaded geom carries u32 group bits (the material's
  `CastShadowGrp0..3`, names at 0x141b62720), masked by `(1 << groups) − 1`;
  a visual with flag `+0x144 & 0x40000` never casts; a material of type 7 or
  with a special pass becomes a *conditional* caster (`ShadowCasterCond`
  shader: `ShadowCasterAlphaRef/AlphaCut/IgnoreAlpha`). The render lists
  (`RenderLightAddBlocksOrPackedGeoms` 0x14023e950) hold the packed zone
  geometry (lm+0x600, stride 0x48) plus every kind ≠ 0 block record; kind-0
  blocks only through their packed geom. Which list the peel colour pass
  filters by (all geoms vs casters only) is not read; the baker's 0.93–1.02
  under raised terrain-tile items says tile items do not occlude the dome —
  a material without cast groups is the likely reason. Test: flip a tile
  material's CastShadowGrp bits and re-bake.
* `CPlugDayTime` = class 0x09181000 (`FuncDayTime`; constructor 0x140592ca0,
  0x138 bytes): chunk 0x09181000 = {50000, 50, 0.25, 5, 0.125}, 0x09181001 =
  {1, 1×6}, 0x09181002 = {0.75}, 0x09181004…08 as banked. The DayTime →
  sun-direction function is **still not located** (no sinf/cosf call in the
  mood-XML parser 0x1405143c0 or the DecorationMood class; candidates: the
  CPlugDayTime methods 0x140592000–0x140596000, `CPlugWeather`). Until then
  the baker's differential (bakes of one map at several DayTimes inside one
  quarter, azimuth of the sky glow) is the way to the formula. Working
  hypothesis [INFERRED]: the sun is the quarter mood's **own** authored sun
  — `Mood.MoodSetting.xml` `Latitude φ`, `DayTime01 t` — with `el =
  asin(cos φ · cos(360°·(t − ½)))` and the azimuth from the hour angle (Sunset
  t = 0.75 → hour 18 → el ≈ 0°, due West = +X in game axes; Day t = 0.644 →
  el ≈ 35°); RE child 1 found this reproduces Stadium Day (34.9° vs 35°
  fitted) but not BlueBay Sunrise (69° vs 45°), so the 64×64 decorations'
  `DecorationMood` (Latitude 30, Longitude 2, DeltaGMT 1, TimeSunRise 10:00,
  TimeSunFall 14:00 on BlueBay Sunset) may remap `t` for those; the map's
  DayTime word only selects the quarter.
* The stored `LAmbient` of frame 0 is not the XML colour (Tiny 16: (0.751,
  1.833, 1.116); Tiny 11: (1.581, 1.792, 1.614)) — a derived reference the
  runtime scales by; treat as opaque.

## 6a. THE MOOD BLENDER — the DayTime word is a TIME, the blend is the MoodWeights smoothstep [DISASSEMBLY, RE child 10, 2026-09-26; 5 frame-record oracles; `moods::blend_weights`]

Corrects §6's first bullet and the "quarter" model: the game blends the moods
for every map, with a weight that is PURE (t ≤ 1.2e-7) at each mood's
default word and a real blend for custom words.

* **The map's DayTime word is the time of day**, `t = word · 2^-16`
  (`RenderLighting_Frames` 0x14021e340 l.312 → zone+0x754; zone+0x750 =
  lroundf(t·65536)). The moods' default words are `key_to_time(DayTime01)`
  (FUN_14020d170 lists each mood as that word): BlueBay Sunrise 0.515 →
  07:20:24 = 0x4e4b, Day 0.644 → 14:34 = 0x9b59, Sunset 0.75 → 20:30 =
  0xdaab, Night 0.15 → 02:24 = 0x199a; Stadium 0.52/0.6/0.73 → 0x5148 /
  0x8111 / 0xceb8. The editor's day-time slider is a key whose quarters are
  the four moods (0x1407b1d50: < ¼ Night, < ½ Sunrise, < ¾ Day, else
  Sunset), stored as `key_to_time(slider)` — the origin of the quarter rule.
* **Time → blend key** (FUN_140494690, `BlenderCurve::time_to_key`): the
  blender curve is CPlugMoodBlender+0x18 `{Latitude, w = 1/48 (0x3caaaaab,
  not in the XML), SunRise t_r, LocalLight_SwitchOff, LocalLight_SwitchOn,
  SunFall t_s}` (decoration blenders: 06:00 / 06:30 / 18:30 (GreenCoast
  19:10) / 21:00; defaults FUN_140494530 = 48, 1/48, 06:00, 06:30, 17:00,
  18:00). Day (t_r ≤ t ≤ t_s): `b = clamp((t − t_r)/(t_s − t_r))`; t < t_r +
  w → `key = 0.25 + ((t − t_r)·¼)/w`; t < t_s − w → `key = 0.5 + (((t − t_r)
  − w)·¼)/((t_s − t_r) − 2w)`; else `key = 0.75 + (((t − t_s) + w)·¼)/w`.
  Night: `mid = (t_s + t_r)/2`, `span = (t_r + 1) − t_s`; t > mid → b = 1,
  `key = ((t − t_s)·¼)/span`; else b = 0, `key = (((1 − t_s) + t)·¼)/span`.
  FUN_140494560 (`key_to_time`) is the inverse. `b` is the sun-arc parameter
  of FUN_140494810 (D = (cos πb, −cos lat·sin πb, −sin lat·sin πb)).
* **The weight curve** (CPlugMoodCurve = CPlugMoodBlender+0x48, keys
  `{X, Weight, moodIndex}` stride 0xc; ctor 0x1404baad0 sets +0x38 = 1 =
  SMOOTHSTEP, nothing turns it off): the XML `<MoodWeights><Key X Weight>`
  entries (moodIndex −1; 0.08 / 0.2 / 0.55 / 0.69, Weight 0 on all five
  decoration blenders) MERGED (FUN_1404bab70, a mood before an equal XML
  key) with the collection's moods sorted by DayTime01 (CHmsMoodBlender
  build 0x14028aea0: CRT qsort by +4; with 4 moods the base setting = the
  sorted 3rd), each mood `{DayTime01, Weight = (k & 1), k}` (k = sorted
  index). BlueBay: `0.08₀ 0.15 Night(k0,W0) 0.2₀ 0.515 Sunrise(k1,W1) 0.55₀
  0.644 Day(k2,W0) 0.69₀ 0.75 Sunset(k3,W1)`.
* **Evaluation** (FUN_1404bae10 ← 0x14028d0b0 l.147): bracket the key
  (FUN_14018c4b0 with period 1: wrap into [0,1); below the first key → +1;
  past the last → the wrap interval (n−1 → 0) with `frac = (key − last)/
  ((first + 1) − last)`; inside: FUN_14018c240 scans consecutive pairs from
  0 for `X[i] − 1e-5 ≤ key ≤ X[i+1] + 1e-5` — an exact or near hit on a key
  is the END of the previous interval; FUN_14018c3a0 `frac = (key −
  X[i0])/(X[i1] − X[i0])`, 0 when i0 == i1 or |ΔX| < 1e-5); `s = fl(fl(f·3)·f)
  − fl(fl(fl(f·2)·f)·f)` (0 / 1 outside (0,1)); `w = fl(fl(1 − s)·W[i0]) +
  fl(s·W[i1])` (FUN_1404bace0); A = the first MOOD walking backward
  (cyclic) from i0, B = the first mood walking forward from i1 (i0 + 1 when
  i0 == i1); **t toward B = (B.k odd) ? w : 1 − w**. Between two moods the
  weight runs W_A → 0 at the XML key → W_B, so the blend is PINNED at the
  XML key: BlueBay [0.55, 0.69] = pure Day, [0.15, 0.2] ∪ [1.08, 1.15] =
  pure Night, [0.2, 0.515] Night→Sunrise, [0.515, 0.55] Sunrise→Day, [0.69,
  0.75] Day→Sunset, [0.75, 1.08] Sunset→Night (all smoothstep).
* **The blended CPlugMoodSetting** (CHmsMoodBlender::Update 0x14028d0b0 ←
  FUN_14028d040(key of zone+0x754) ← FUN_1402692b0): every field
  `a + fl(fl(b − a)·t)` — Latitude (+0x1c: the MOODS' XML latitude; the
  blender XML's 47.5 only with CPlugMoodBlender+0x30 ≠ 0, which nothing
  sets), LocalLightX, HelperHdrX, LAmbient rgb + scale (+0x2c/+0x38),
  LDirSun (+0x3c/+0x48), LDirMoon (+0x4c/+0x58), T3SpecularLocal
  (+0x68/+0x6c), T3LightMap MaxHDR/BounceFactor/SkyFactor (+0x70/+0x74/
  +0x78; SkyUseClouds +0x7c = the NEARER mood's, t < 0.5 → A),
  HdrScales.Player (+0x8c), the whole <Atmo> block (+0x90..+0x158: HdrSun
  Power, Atmo1/Atmo2 Power/Scale/Color — colours are stored LINEAR (hex →
  LUT 0x141a64360 = client-re/srgb_to_linear_f32_256.bin), so the lerp is
  linear), the FogMatter entries; the <Fog> block (+0x1ac) by FUN_141418090
  as `fl(1 − t)·a + t·b` (DepthMin/Max, Exponant, Intens, Height, Clouds,
  SkyClouds.GlobalIntens = Sky_p's FogIntens, Color, WaterFog, Noise).
  DayTime01 = the key; EnableStars = near's. Applied to the zone: the sky
  material = A's (GradientV = A's SkyColor, GradientV1 = B's — the NEXT
  mood's texture is the material's second slot), vc+0xf8 = t → `ScaleGrad0
  = 1 − t`, `ScaleGrad1 = t` (pwc-day at 14:34: key 0.6439972 → t = 1 →
  (0, 1) as captured); FUN_1402697e0 = sun/moon/ambient from the blended
  fields (sun colour = LDirSun·scale, or the moon when |LDirSun|·s ≤
  |LDirMoon|·s' with the fixed direction +0x5c; direction FUN_140494810
  (blended Latitude, b); vc+8 = **local lights on iff time < SwitchOff ||
  time > SwitchOn** — on the TIME, the word itself: 0x5148 07:37 off,
  0x9b59 off, 0xdaab 20:30 on; the lightmapper bakes frame 1 whenever the
  map has lamps regardless — stpad at 07:37 is lit; the flag is the runtime
  toggle of the colourless light list); FUN_1402694c0 = the Sky_p lobes
  verbatim; FUN_14020d370 → FUN_14020d170: the compute params' MaxHDR /
  Bounce / Sky / Clouds = the BLENDED mood's → the frame record's
  MaxHDR_Mood / BounceFactor / SkyFactor / SkyUseClouds.
* **Oracles** (`clientre lmimages` on refs/): np-tk3 0x5000 (07:30) → key
  0.5178571 in [Sunrise, 0.55]: frac 0.0816, s 0.0189 → A Sunrise, B Day
  (even) → t = 0.0189037: MaxHDR_Mood 1.0378075 = 1 + 2t ✓, Bounce
  1.6075615 = 1.6 + 0.4t ✓, Sky 1 ✓ (a linear weight would give 0.0816 ✗).
  0x9b59 → key 0.6439972 → t = 1 pure Day ✓ (the captured dome). 0xC000
  (18:00) → key 0.7053571 → Day → Sunset t 0.163. 0xdaab (20:30) → key
  0.750061 (the word's 1/65536) → wrap interval, frac 1.85e-4, s 1.03e-7 →
  w = 0.99999988 → t = 1.19e-7 toward Night: MaxHDR 3 + (1.7 − 3)t =
  2.9999998 ✓, Sky 1 + 2t = 1.0000002 ✓, Bounce 2 ✓ — hill4 and np-tk3's
  "lerp artefacts" are this 1e-7; hill4 is a PURE Sunset bake. stpad 0x5148
  (Stadium, 07:37) → key 0.5200021, 2e-6 above Sunrise's 0.52 → the
  lookup's tolerance puts it at the end of [0.2, 0.52] → t = 1 toward
  Sunrise → MaxHDR_Mood 2.2 ✓. `cargo test -p lightmap --lib moods` holds
  all of these (the 0xdaab record values to the bit).
* **The port** (`moods.rs`): `blend_weights(word, collection)` → `MoodBlend
  {a, b, t, time, key, sun_arc}` (+ `near()`, `scale_grad()`, `pure()`),
  `blended_xml`/`blended_xml_of` (every field `lerp_field`, MaxHDR included),
  `lerp_fog_field` for the <Fog> block, `nearest_mood_name` (FUN_14028ccb0:
  the mood the editor calls current for a key), `BlenderCurve::time_to_key /
  key_to_time / local_lights_on / default_word`, `word_of_time` /
  `time_of_word`, `merge_mood_curve` / `curve_lookup` / `curve_weight` /
  `curve_moods`. `lmtool bake` blends by default (`--no-mood-blend` = the
  nearer mood alone; identical at the default words to 1e-7).

## 6b. What the attribute pre-pass reads from the COLLECTION PACK [DISASSEMBLY + FILE, RE child 8, 2026-09-25; every rule bit-exact vs pwc-day frame 127447]

The pre-pass (`Lightmap/…` PS 8401 = `Tech3/Block_PyPxz_ids_p`, PS 17025 = the
single-texture Py/Pxz shader, the water tint PS 17018) rasterises every LM
chart with `GbxVisualToWorld = 0`: the world position and normal are 0, the
normal's normalisation is NaN, both `mul_sat` blend weights land on 0, and a
material's colour collapses to ONE texture sample at a constant uv. Its inputs
are collection data — `mapgeom::terrain` + `lightmap::paktables` build them
from the pack (`lmtool pak-tables --pak F:K … --collection C --material LINK …`):

* **Terrain materials** (`<Coll>\Media\Material\<Name>.Material.Gbx`, parent
  `Techno3\…\Tech3 Block PyPxz_Ids`): the inline `CPlugMaterialCustom` chunk
  0x0903A015 v2 holds the layer NAMES (u32 0, then Pxz, Py, X2, H2 — SeaFloor:
  `"SeaFloor","SeaFloor","SeaFloor",""`; reader 0x1404415c0 into +0xe8/+0xd8/
  +0xf8/+0x108); chunk 0x0903A013 the slots `BaseColor RoughMetal Normal PyX2
  PyH2` → `Texture Array\Terrain_{D,R,N,X2,H2}.TextureArray.Gbx`. The
  `CPlugBitmap` array file: chunk 0x09011034 = `{v4, ref ImageArray, string
  suffix "_D", n, slice refs[n], ref, 8 B}` — the slices IN GPU ORDER, each
  uploaded VERTICALLY FLIPPED (BC1 block rows reversed, the four index bytes
  reversed; 6/6 slices of 5354, 4/4 of 5363, all 13 mips of 14627 identical);
  chunk 0x09011030 → an inline `CPlugFileGen` kind 0x1d `{4096, 4096, 6, 1, 3,
  13, 1, 1, 0}` (the generated array's size / slices / mips; archive
  0x1404179a0: version 0x80000006, kind, u32[], float4[], f32[], string, nodes,
  refs — no chunk framing). `Image Array\TerrainLayers_DNRM.ImageArray.Gbx`
  (`CPlugImageArray` 0x0914C000, chunk 0x0914C000 v7, reader 0x1404d4b00):
  folder + layers `{name, Py size xy, Py offset xy, Pxz size xy, Pxz offset xy,
  rotation°, side blend start/end°, top blend start/end°, id, id}` (stride 0x50;
  BlueBay: Land 52/40 m rot 74° blendPy 40–60°, CliffPxz 128, HillPxz 64, Sand
  32, SeaFloor 64, RocksTop 40; ids 100/110/120/30/54/115).
* **The per-slice buffers** `g_WorldPosToTcPyPxz` / `PyX2` / `PyH2` (5352 / 5361
  / 5365) live on the texture-array object (+0x1c0/+0x1c8; the binder
  0x1409fc550 hands them to the shader by name) and are filled by 0x1409f2e50
  + 0x1404d49d0, 4 float4 per layer: `a = (rot·3.1415927f)/180f`, `s, c =
  CRT sinf/cosf(a)`; `[0] = (c, 0, s, offU)·(1/sizeX)`, `[1] = (s, −0, −c,
  −offV)·(1/sizeY)` (mul by the reciprocal); `[2] = (1/PxzX, 1/PxzY,
  PxzOffY/PxzY, bits(id[1]))`; `[3] = (cos(sideEnd·k), cos(sideStart·k),
  cos(topEnd·k), cos(topStart·k))`, `k = f32 0x3c8efa36` — BlueBay's 24 float4
  reproduce 5352 to the bit (`cos 60° = 0x3efffffc`). `mapgeom::terrain::
  world_pos_to_tc`.
* **The ids** `g_CBufferP_Shader.{iPy,iPxz,iPyX2,iPyH2}` = the material's
  `SubIndexPyPxz` (0x1404424e0 → 0x1404429f0 / 0x140442d80 / 0x140442e00,
  packed `(iPyH2|0x80)<<24 | iPyX2<<16 | iPxz<<8 | iPy`): iPy / iPxz = the Py /
  Pxz name's index among the BaseColor array's ImageArray layers (0x1404d4550:
  first match, else −1), iPyX2 / iPyH2 likewise in the PyX2 / PyH2 arrays', −1
  for an empty name or a slot without an ImageArray (WhiteShore's PyX2 is the
  Techno3 `DisabledModX2` texture). SeaFloor → (4, 4, 1, −1), Land → (0, 0, 0,
  −1) = the captured cbuffers. Quirk: BlueBay's X2 TEXTURE has 4 slices (Land,
  Dirt2, Dirt, SeaFloor) but its ImageArray 2 layers → iPyX2 = 1 samples
  Dirt2_X2 with SeaFloor's mapping; irrelevant at the zero matrix (X2 only
  modulates the Py term, weight 0).
* **The constant** = `TMapBaseColor` slice iPxz at uv `(0, −[2].z)` through
  `SGbxWrap_Aniso` with zero derivatives = mip 0, bilinear, wrap; with every
  BlueBay PxzOffY = 0 that is the mean of the FOUR CORNER TEXELS of the slice
  image (BC1 Expand8Round, IEC sRGB→linear): SeaFloor_D (0.85107964,
  0.73398048, 0.32585403), Land_D (0.11575814, 0.17534077, 0.04276802) — the
  captured 0.8510797 / 0.7339804 / 0.325854 (the last digit is the GPU's
  filter order). `paktables::terrain_constant`. RedIsland CliffEndsPxz
  (PxzOff (0, 70)) and WhiteShore WaterBottomPxz (Pxz (80, 7)) leave the
  corner rule; the formula covers them.
* **PS 17025 materials** (`…\Modifier\StadiumOnTerrain\TrackWallInWorld`,
  parent `Tech3 Block PyPxzDiff_Spec_Norm_LM1`): `PyBaseColor = PxzBaseColor =
  Stadium\Media\Texture\TrackWallPxzInWorld_D.Texture.gbx` (Stadium.pak) whose
  chunk 0x09011025 `(1/32, 1/32, 0, 0, 0, 0xff000000)` is
  `GbxSamplerTcScaleTrans_PxzBaseColor` / the `GbxWorldPosToTexCoord` scale;
  `TMapACosSmoothPy` (5459) = the Techno3 default `ACosSmoothDefaultPyPxz`
  (Maniaplanet.pak, NOT in a collection pack): texel 0 = 65535 → the Py term's
  weight `1 − 1 = 0` → the constant = the Pxz image at (0, −trans.y) = its
  corner mean (0.43576217, 0.40357280, 0.34829098) = the captured (0.436,
  0.404, 0.348). `paktables::projected_constant`.
* **Water** (`Collections\<Coll>.Collection.Gbx` chunk 0x03033038 v8, reader
  0x140d0c580 → 0x14040c720 into the collection +0xf0): `{i32 −1, u32 0, Id
  name, f32 WaterTop, f32 WaterFloor, f32 FogMaxDepth, ref fog TGA, ref
  WaterTransmittance.ImageGen, ref normal texture, f32×4, u32, f32, u32, f32}`:
  BlueBay `Sea 7.0 / 4.0 / 3.5`, RedIsland `Deep 7.7 / 2.0 / 6.0`, GreenCoast
  `Deep 7.2 / 0.0 / 5.0`, WhiteShore `Deep 7.0 / 2.0 / 6.0`, Stadium `Shallow
  7.0 / 4.0 / 50.0`. The lightmapper (0x1402255a0): `g_WaterTop_ByPlanes[p]` =
  the zone's water plane heights (= WaterTop; pwc-day 7.0),
  `g_WaterDepth_FogMaxDepthInv_ByIds[id−1]` = `(WaterTop − WaterFloor,
  1/FogMaxDepth)` = (3.0, 0x3e924925) from the zone vision constants' water
  table (`SHmsZoneVisionCst+0x60`, 4 × `{id, fogMaxDepth, depth}`); the id map
  = water type + 1 (PS 17012; one type per collection). `TMapWaterFog` (15075,
  256 BGRA8 sRGB) = COLUMN 0 of the descriptor's fog image
  (`<Coll>\Media\Texture\Image\WaterSea_Fog.dds`: a 32×256 bottom-up TGA
  despite the name; = the Day mood's `WaterColor.tga`) read TOP-DOWN, 256/256;
  the other collections' `WaterFog.dds` are 256×32 → the top row, INFERRED.
  `TMapWaterTransmittance` (15078, 2048 RGBA8 sRGB) = `WaterTransmittance.
  ImageGen.Gbx` (`CPlugFileGen` kind 0x33: u32 `{2048, curve}`, f32 `{c.r, c.g,
  c.b, depth, −1, −1}`; BlueBay (0.15, 0.18, 0.1, 4.0)) generated by
  0x140418310: `t = i/2048` (f32), `t = (t² + t) − t²·t` when `curve`, `d =
  t·depth`, `rgb = c^d` (CRT powf), each channel → an sRGB byte through the
  engine's 4096-entry table (0x141a64760 = `round(255·IEC(i/4095))`, index
  `trunc(x·4095 + 0.5)` clamped; 0x14018cdf0), alpha 0xff — 2048/2048.
  `paktables::water_tables`.
* **The Techno3 defaults** (Maniaplanet.pak, key `9A93…`, banked under
  tm-paks/ 2026-09-25): the parent materials are one shader reference each
  (`Tech3 Block PyPxz_Ids.Material.gbx` → `…\Shader\Tech3 Block PyPxz_Ids.
  Shader.Gbx`; `paktables::material_constant` reads the family off the shader
  name when the pack is in the store). `ACosSmoothDefaultPyPxz.Texture.Gbx`
  (5459) is a GENERATED R16 1024×1 LUT — `CPlugFileGen` kind 0x21 `{1024,
  smooth 1, degrees 0}`, f32 `{0.45, 0.7}` — by 0x14041b290: `t = acos(i/1024)
  / (π/2)`, `(t − 0.45)/(0.7 − 0.45)` clamped, smoothstep `3t² − 2t³`,
  `× 65535` truncated (`paktables::acos_smooth_lut`, 1024/1024 vs the
  capture; texel 0 = 65535 → the Py term's weight 0 at the zero matrix).
  `DisabledModX2.Texture.gbx` (5468) → `Image\DisabledModX2.tga`, a 4×4
  24-bit RLE TGA of one colour 0x7f7f7f (`paktables::constant_tga_rgba`).
  `SeaWaterFog.Texture.gbx` → `Image\SeaWaterFog.tga`, a 256×256 GREY default
  (245,245,240,3 → 52,51,49,253): the bitmap object the weather model hands to
  `CHmsZone+0x3b8` carries the COLLECTION's fog image at runtime — the
  default's pixels never reach the lightmapper.
* **Which fog table for a non-Day mood** [DISASSEMBLY 0x1406a3400,
  0x1407b1d50, FILE `mapgeom who-refs`]: `CHmsZone+0x3b8` is written from the
  decoration layout's env-bitmap 0 (−1 in every collection's Scene3d) and from
  `CPlugWeatherModel::BitmapWaterFog` of the ONE `Techno3\MotionManagerWeathers
  \DayTime.MotionManagerWeathers.Gbx` (all collections, all moods). Nothing
  references `Moods\<Mood>\WaterColor.tga` (0 of 5593 GBX files in BlueBay.pak;
  no exe string): Night/Sunrise/Sunset use the same collection image. OPEN
  (bounded): the image is 32 columns wide and the runtime texture is named
  `WaterFogDayTimed`; the Day bake took column 0; other columns differ by
  ≤ 13/255 (alpha at depth) — one Sunset capture settles the column rule.

## 6c. The ENVIRONMENT BLOCK from the collection pack [DISASSEMBLY + FILE, RE child 9, 2026-09-25; bit-exact vs pwc-day frame 127448]

Layer 0 of every peel (and, a subset, the sun shadow map) is the DECORATION's
own geometry: the mobil solids of `<Coll>\GameCtnDecoration\Scene3d\Base64x64.
Scene3d.Gbx` (`scene3d-cscenelayout.md`), drawn through the legacy `CPlugSolid`
→ `CPlugTree` render with their own materials — `mapgeom envblock --collection C
[--out peel.obj] [--sky dome.obj] [--shadow sun.obj] [--gpu DIR]
[--compare-capture PASSCAP [--frame N]]` (`mapgeom::envblock`) rebuilds it and
checks it against a capture.

* **What the capture holds** (frame 127448, direction 0's world peel, then the
  sun shadow map eids 347–410): the peel's layer 0 = draw 1009 = the `ShadowCaster64`
  solid (272 vertices / 262 triangles, `Effects\…\InvisibleShadowCaster.Material`,
  an inverted skirt x/z −198..2246 × y −1020..4 — the "sea box"), draws 1028 /
  1033 / 1071 / 1076 = the four `Warp-C00x_1` leaves of the `Square64Water`
  solid (184 vertices / 280 triangles each, `WarpSand.Material` — the "terrain
  patches" are a MESH of the pack, not a heightmap: the two R16 1024×1 textures
  of those draws are the Warp shader's `ACosSmooth` LUTs, §6b), draw 1051 = the
  SKY DOME (`Sky\Media\Solid\SkyDomeMirror.Solid.Gbx` of Maniaplanet.pak: 2143
  vertices in 33 rings × 65, 3968 triangles, radii 22265.23 (x, z) × 9751.16 (y),
  material `Tech3 Sky`), then the 177 cloud sprites (a separate system). The sun
  shadow map holds the ShadowCaster64 alone of these (draw 377, VS 1142 / PS 937).
  NOT drawn anywhere: the four `Warp-C00x_0` Water leaves (83 vertices / 84
  triangles each, `Water.Material` → `Tech3_Water_MultiH`).
  `mapgeom envblock … --compare-capture` reproduces all seven draws' vertex
  buffers (positions f32×3, normals snorm16×4, the dome's uv f32×2) and index
  lists BIT FOR BIT; the normals' re-encoding is `trunc((s/511) · 32767)` per
  Dec3N component (a float round trip, not a shift: −16 → −1025).
* **The selection rule** (`CVisionViewport::Shadow_RenderDelayed` 0x140a4fab0 →
  per CHmsItem 0x140970070 → per leaf 0x14096fee0 → 0x1409514c0 → shader pick
  0x140950f80):
  1. `CPlugTree` flags (chunk `0x0904F01A`, in memory +0xa8 = file word | 0x2000):
     the ROOT renders when `& 0x8` (IsVisible) or, in a SHADOW-MODE render
     (`ctx+0xb0 & 4` — the lightmapper's sun shadow map AND its peels), when
     `& 0x4000` (IsShadowCaster); a LEAF is submitted in shadow mode only when
     ITS `& 0x4000`. Square64Water root 0x1e88a, WarpSand leaves 0x1e80a,
     Water leaves 0x1a80a (visible, not a caster → out of every lightmapper
     render), ShadowCaster64 0x1e802 (caster, invisible → in every lightmapper
     render, never in the game view), the dome's `Desert` tree 0x1e88a.
  2. The material's compiled shader (`CPlugShaderApply`; `.Shader.Gbx` chunk
     `0x09002020` v3 = {u32 A → +0x140, u32 B → +0x144, u32 → +0x150, f32, ref,
     u16 PASS BITS → +0x154}, reader 0x1403da430): 0x1409514c0 draws the leaf
     iff `(passBits & ctx.mask) == ctx.required`, default mask 0x8130 /
     required 0 (0x140408a70) — ShadowCaster 0x0401, Tech3 Warp PyPxzDiff 0x0441,
     Tech3_Water_MultiH 0x0041 all pass; 0x140950f80 refuses in shadow mode a
     shader whose `B & 0x40000` (never casts): Water's B = 0x004c0020 has it,
     WarpSand's 0x0018fff0 and the ShadowCaster's 0x0008ff00 do not.
  3. The PASS KIND separates the sun shadow map from the peel: the shadow-map
     render asks 0x1403de0d0(shader, kind 6 | 0xb) for the shader's SHADOW
     variant and draws nothing without one (0x140950f80, `ctx+0xbc` = no
     fallback). A pure-shader material (no `CPlugMaterialCustom`:
     InvisibleShadowCaster) gets the default caster program (VS 1142 / PS 937);
     a material WITH a custom part only when its pass-0 program declares itself a
     caster (program object +0x248 bit 0 — the bit's source is not located; the
     `IsShadowCaster` / `ShadowCasterCond` strings are its likely annotations) or
     carries an alpha parameter (0x1403de6f0: a pass-0 parameter of kind 0x77 →
     the alpha-cut `ShadowCasterCond` variant = PS 1147 `discard
     TMapAlpha01.a − GbxShadowAlphaThreshold < 0`, threshold 0.50196 = 128/255,
     which the capture applies to the vegetation card's leaf textures 14579 /
     14585, not to any terrain). Tech3 Warp PyPxzDiff has neither → WarpSand is
     absent from the sun shadow map; it has NO alpha texture and its peel shader
     PS 16752 (= `Tech3/Warp_PyPxz_p`) has no discard — no cut-out anywhere.
  4. The peel draws each caster with its OWN colour program under the
     `RenderPath_DblSideBlackBack` permutation: `ShadowCaster.PHlsl` blob 1 =
     PS 17316 `discard_nz is_front_face; o0 = 0` (the box is black, back faces
     only), `Warp_PyPxz_p` = PS 16752 (`and o0.xyz, colour, isfrontface`: the
     WarpSand's forward-lit colour — `PyDiffuse` = `WarpSand_D` (tc scale 0.0005
     from its texture chunk 0x09011025), `PxzDiffuse`/`PxzNormal` = `CliffPxz_D`
     / `_N`, the two `ACosSmooth` LUTs, `PxzScaleTrans` = (0.0005, 0.0005, 0.5)
     from the material's `CPlugMaterialCustom` chunk 0x0903A00A GpuFx parameter,
     lit by `GbxP_LightDirRgbLinear0/DirInWorld0` = the mood sun, ambient 0,
     × the clouds shadow, × 2, fog lerp — on black back faces).
* **Per collection**: BlueBay (inline solids), GreenCoast, RedIsland, WhiteShore
  (external `<Coll>\Media\Solid\Warp\{Square64Water,ShadowCaster64}.Solid.Gbx`)
  all follow the rule: the dome (`SkyDomeMirror`, at the origin on BlueBay /
  GreenCoast, at (1024, 0, 1024) on RedIsland / WhiteShore — `GbxSkyV0.
  VisualToWorld` = the mobil pose), four `WarpGround` quadrants in the peel only
  (RedIsland 221 vertices / 354 triangles each, WhiteShore 222 / 355, GreenCoast
  223 / 356; `PxzScaleTrans` 0.001 on RedIsland / WhiteShore), four Water
  quadrants excluded, one ShadowCaster64 (262) in the peel and the sun map.
  Stadium: EVERY Stadium decoration (Base48x48*, NoStadium48x48*, Screen155*
  — one `CGameCtnDecorationSize` E2159DF0…) references `Stadium256\GameCtnDecoration
  \Scene3d\Base16x12.Scene3d.Gbx` (hashed D16267E8…): ONE mobil `SkyDome` at
  (0, 3000, 0) drawing `Sky\Media\Solid\SkyDomeDouble.Solid.Gbx` — tree `Desert` >
  `Snow` = a dome of the SkyDomeMirror radii (2143 vertices / 3968 triangles, its
  own vertex order, u ∈ [−1, 1], v ∈ [0, 1], closed: every edge on two triangles;
  world y −6751..12751) + `Stars` = a `CPlugVisualSprite` (8952 star sprites,
  material `Tech3 Stars`, pass bits 0x0401; its 24-byte vertices = position + three
  floats — `CPlugVisual3D::ArchiveChunk` 0x14049a160 never packs a sprite's normal;
  sprite chunks 0x09010005/6/8/9 per 0x14045bec0). No ground solids: the NoStadium
  difference is the decoration MAP (RE 7), not the Scene3d. No Stadium capture exists;
  the dome is proved structurally (`stadium_dome_is_the_double_dome_at_3000`).
* **The WarpSand's texture transform**: `GbxWorldPosToTexCoord_MapPyDiffuse` =
  0.0005·R(15°) in the capture; the 15° is `CPlugBitmap` member `DefaultTexCoordRotate`
  (0x0901101D, degrees) = the 5th word of `WarpSand_D.Texture.gbx`'s chunk 0x09011025
  {Vec2 scale → +0x30, Vec2 trans → +0x38, f32 rotate → +0x40, u32 colour → +0x1c}
  (reader 0x1403f7550). `envblock::TexCoordTransform::world_pos_to_texcoord`: a =
  deg·(π/180 = 0x3c8efa35), rows x = (cos a·su, sin a·sv), z = (sin a·su, −cos a·sv),
  w = trans — the captured rows 0x39fd3630 / 0x3907b21b bit for bit (the constant
  provider is case 0xca of 0x1409f87a0 → 0x1409f8570 from an Iso4 the texture binding
  holds; that Iso4's builder was not located — the arithmetic is transcribed from the
  reproduced bits). `EnvLeaf.texcoord` carries it per texture slot.
* **What is not the environment block**: the zone tiles (the Sea quad, draw 365,
  4096 instances — E's tile path), the items (draws 347–410), the clouds
  (`GbxClouds3dInst0`, `clouds.rs`), and the WarpSand's colour (A's shading).

## 6e. CLOUDS NEVER REACH THE LIGHTMAP [CAPTURE pwc-day f127448 + stpad f4788; port engineer G, 2026-09-26; `lmtool clouds-reach`]

Two places a cloud could enter a bake, both closed on captured data:

* **The dome has no clouds term.** PS 16774 (29 instructions, transcribed in
  `skygrad::sky_ps` / `domecheck::ps_16774`) reads two BC6 2048×1024 gradients:
  `TMapGradientV` × ScaleGrad0 + `TMapGradientV1` × ScaleGrad1, then the (off) sun
  disc, the two Atmo lobes, the fog lerp, GlobalScale, min 16375. The two textures
  are the TWO BLENDED MOODS' `SkyColor.dds` — in pwc-day (BlueBay Day, t = 1)
  t0 = 16801 = BlueBay-Sunrise-SkyColor.dds mip 0, t1 = 16803 = BlueBay-Day-
  SkyColor.dds mip 0, byte-identical to the banked mood files (md5 of the 2 097 152
  mip-0 bytes; the other 18 moods' gradients differ), ScaleGrad0 = 0, ScaleGrad1 = 1
  (the filler `0x1409f7a40`: (1 − t)·s0, t·s1, s0 = s1 = 1). `SkyClouds.dds` (DXT5
  1024², 11 mips) matches no texture bound in any lightmapper draw of the capture
  (neither the gradients nor the sprite atlas 14508's four slices). The mood's
  `SkyUseClouds` (1 for BlueBay Day, the frame record's `Clouds` word) therefore
  adds nothing to the dome; with it ON the dome was cloud-free.
* **The cloud SPRITES cannot produce a fragment.** They are the environment block's
  177 draws of VS 14514 / PS 14515 (One/InvSrcAlpha, GreaterEqual, no depth write,
  the same draws in every peel of every direction: 531 = 3 × 177 in frame 127448).
  Each draw is one `GbxClouds3dInst0` instance (VisualToWorld = a pure translation;
  the instances tile every 16 km over ±64 km in x/z at y 2 101–3 000 m) holding
  1–201 camera-facing sprites (8 113 in all): centres y 574–5 706 m, half extents up
  to 2 241 m, opacity 1. For the ORTHOGRAPHIC peel camera the sprite is a quad
  perpendicular to D: corner = centre + R·(a·size)·(±½) + U·size·(±½) with R/U =
  `GbxV_WorldToCamera` columns 0/1 (= `AxeXinV`/`AxeYinV`; GlobalDir_Branch.w = 0,
  IsRadial = 0, pivots −0, vortex off on all 177 draws) — so the test against the
  frustum (the light-space AABB of the world box, `lightcam::fit_camera`, +5 m far,
  ×1.0001) is exact per axis: `lmtool clouds-reach PASSCAP [--box …] [--quality Q]
  [--scan-ymax …]`. RESULT on pwc-day's world box (x/z 0…2048, y 4.0…138.0) over the
  game's q4 sets (1032 + 512 + 256 + 128 directions; q3's 256 + 128 are subsets of
  the same tables): **0 sprites inside in 0 directions**; the least separation is
  245.5 m (sweep 1, direction (0.5212, 0.5966, −0.6103), a sprite of eid 1848 — the
  instance over the map at (0, 3000, 0) — centre (−45, 3042, 682), half 504 × 1482);
  the frustum's highest world point over all directions is 1 588 m, the sprites'
  lowest corner over all camera orientations −405 m (a far instance, outside the
  frustum in x/z). Engineer D's clip test on the captured post-VS positions of
  direction 0 (568 triangles in depth, 12 in x/y, none in both) is the same fact for
  one direction. THE MARGIN IS GEOMETRIC, NOT LARGE: raising the world box's top
  (`--scan-ymax`) the least separation falls 263.5 → 93.6 (350 m) → 53.1 (400) →
  16.5 (450) m and at **500 m one sprite enters one direction** (600 m: 22 sprites in
  17 directions of the 1032-set) — a BlueBay-family scene whose world box (scene ∪
  probe-grid) tops ≈ 470 m would peel cloud sprites, and PS 14515 (83 instructions,
  unexercised in every capture) would then need transcribing. No product map does:
  **Stadium's peel has no environment block at all** — stpad f4788 (Sunrise) holds
  11 584 draws, every one an item layer (VS 9163/9166, depth LESS, viewport
  (1, 1, 4094, 4094)), no GreaterEqual draw, no dome, no terrain, no cloud shader;
  the 4096² peel targets are CLEARED per direction (144 colour + 150 depth clears)
  and the sky enters as that clear colour (E; the (b) export carries the values).
* Consequence for the port: `skygrad.rs` is complete for the dome (gradient pair,
  lobes, fog, scale — its "clouds layer" comment was stale); no cloud code belongs in
  the peel for any bake whose world box stays under the threshold above. The
  world-box top is the one quantity to watch: `clouds-reach --box` re-runs the proof
  for any box.

## 6d. THE UNDERWATER TERM — `BlendWaterFog` on the pre-pass MDiffuse [CAPTURE pwc-day f127447 + the port's own atlas, RE child 11, 2026-09-26]

The game attenuates submerged surfaces in ONE place: the attribute pre-pass. After
each of the nine jittered material runs (§6b) it draws, into that run's RGBA16F
MDiffuse scratch, `SetWaterId` (VS 17011 / PS 17012: every water quad of the
scene rasterised TOP-DOWN into an R8G8_UINT map at one texel per metre over the
map's world XZ — `(BLENDINDICES.x + 1, TEXCOORD7.x)` = the water type + 1 and
the PLANE index; no depth test, the last quad wins) and then `BlendWaterFog`
(VS 17017 / PS 17018) over EVERY LM object with the dual-source blend
`One / Src1Color`:

```text
(id1, plane) = WaterIdMap[(x, size_z − z)]          id1 == 0 → untouched (no water over this xz)
top = g_WaterTop_ByPlanes[plane]                     top < P.y → untouched (above the surface)
(depth, inv) = g_WaterDepth_FogMaxDepthInv_ByIds[id1 − 1]
P.y < top − depth − 0.1 → untouched (more than 10 cm under the water FLOOR)
u = 2·(top − P.y)·inv                                twice the fragment's own depth: the round trip, no eye ray
fog = TMapWaterFog.SampleBilinearClamp(u)            256 texels, rgb sRGB → linear, alpha LINEAR = the fog amount α
T   = TMapWaterTransmittance.Sample(u)               2048 texels, sRGB → linear, per channel
dst.rgb = α·fog.rgb·ScaleOut + dst.rgb·T·(1 − α)     ScaleOut = 1/9 per run; alpha untouched
```

Over the nine runs: `MDiffuse = α·fog + T·(1 − α)·mean(albedo)`. It multiplies
the ALBEDO of the submerged texels — the light they bounce (the peel colour,
`ILightInput = sun/w × MDiffuse`, §2.4) — and nothing else: the direct sun, the
sky through the un-peeled water plane (§6c: Water never casts, never peels) and
the local lights on a submerged wall are not attenuated. Bilinear at texel
`u·W − 0.5`, clamped: pwc-day's seabed at depth 3 m under a 3.5 m FogMaxDepth has
`u = 1.71` → the deepest texel, `T ≈ 0`: the seabed's albedo is REPLACED by the
fog colour (A's `prepass_check` reproduces the captured atlas after the tint,
1 654 008 texels changed on both sides).

The constants: `g_WaterDepth_FogMaxDepthInv_ByIds` and the two LUTs are the
COLLECTION's (§6b; Stadium Shallow: `(3.0, 0.02)`, the transmittance generator
`(0.84, 0.95, 0.97)^(50·curve(u))`, the fog image `WaterFog.dds` whose row the
port reads carries alpha 0 on all 256 texels — Stadium's tint is then
transmittance only, pending the stpad pre-pass capture's t1 export for the
256 × 32 image's axis); `g_WaterTop_ByPlanes` are the zone's water PLANES in
WORLD y, not the collection's block-local WaterTop 7: stpad's pools sit at
23, 119 and 231 (156 / 12 / 12 blocks), and a fragment at 20–23 under a plane
"7" would never qualify. The water quad = the shaded geom whose material's
shader is the water shader (`Tech3_Water_MultiH`, B 0x004c0020: `never_casts`
+ "water" in the shader path; Stadium's `Water\Base_Air.Prefab` entity 0
visual 1 — 9 vertices at local y 7, POSITION + BLENDINDICES(Int32, the type
in the low byte) + NORMAL + COLOR0, no uv), placed by the record's Iso4.

Port: `lightmap::waterid` (the id map + plane table from the records;
`lmtool water-ids MAP --pak … --collection C [--out ids.png] [--quads]`),
`setupmap::water_tables_from_records` (the inputs on both setup paths),
`prepass_check::tint_from_map` over every LM mesh, `prepass::{vs_17017,
ps_17018, run_water_draws}` (A's transcription). On stpad the pool floor's
MDiffuse becomes Waterground × T(3 m) = (0.0344, 0.2544, 0.3765) — once the
record materials carry their own constant (the per-link map of RE11/0003;
before it every block / clip record was black on Stadium, so the tint had
nothing to act on and the port's Stadium bake had no first bounce at all).

## 7. Alignment list for `lmtool bake` (what lmtool does → what the client does → fix)

1. **Encoding is sqrt, not linear.** lmtool `synth.rs from_hdr`: `pixel =
   255·E/enc_max`. Client: `pixel = 255·sqrt(E/enc_max)` per channel, and the
   game squares at load. Fix: sqrt in `from_hdr`/`from_hdr2` (frame 0 and
   frame 1). This is the "Sunrise black-shadowed / Day darker and harder"
   defect: every mid-tone was displayed at p².
2. **Frame-0 image 1 is three WEBPs, not one.** lmtool writes one grey
   image; the client reads three concatenated RIFF files (coefs 1–3). Fix:
   write three 128-grey images (zero directional terms) — or the real
   coefficients from §2.5 with the item's tangent frame; store their maxima
   in the frame record's f16×3.
3. **Direct sun: NOT in frame 0** (re-corrected 2026-09-23, §0). lmtool
   adds `sun·max(0,n·L)·sunVis` to the texel; the client lights only the
   *peeled bounce surfaces* with the sun (`GeomILightIn0`: one shadow map,
   `LightDirRgb·max(0,n·−L)·shadow·MDiffuse`) and the texel receives it
   through the dome directions that hit those surfaces. Fix: drop the
   direct term; put `LDirSun` on the bounce-surface radiance only.
4. **Ambient shape.** lmtool: `ambient + up·(0.5+0.5 n.y) + sky·skyVis`
   (fitted). Client (stored frame): **no unoccluded term at all** — the
   dome sweep (item 5) plus the bounces; nothing else. Fix: drop `ambient`
   and `up`.
5. **Sky = the rendered sky dome over a uniform sphere set.** lmtool: 64
   cosine-weighted hemisphere rays × a fitted sky colour. Client: N = 256
   directions of the PointsInSphere 256-set (in the game's issue order:
   `dome::group_issue_order`, §2.3),
   each an orthographic depth-peel; a texel that sees no surface along `D`
   sees `Sky_p(D)` = `SkyColor.dds` (u shifted by the sun azimuth) ×
   ScaleGrad0 + the other blended mood's gradient × ScaleGrad1 (no clouds term, §6e) + the two `Atmo` glow lobes around the sun,
   fogged, × GlobalScale (§2.3); accumulation `Σ_D (4/N)·max(0,n·D)·L(D)`
   (H-basis: 1/N with the projection weights). Fix: decode the mood's BC6H
   `SkyColor` (banked under client-re/moods/), implement `Sky_p`, use the
   256 sphere directions; the "30° cone" is withdrawn.
6. **Bounce.** lmtool: one bounce, constant albedo 0.5, off by default.
   Client: 2 (Default) / 4 (High) / 6 (Ultra) sweeps with the per-texel
   material albedo (`MDiffuse`, LOD bias −0.5·log2 N) and the direct sun on
   the bounce surfaces, decode scale ÷ `BounceFactor` during the sweeps
   (§2.4 — reconcile the sign against the 0.37 underside ratio before
   porting); the sea/ground bounce is what lights undersides. Fix: albedo
   from the material diffuse textures, the sun on the bounce input, the
   BounceFactor handling as read, 2 iterations for a Default bake.
7. **Direction sets.** lmtool: stratified random. Client: the 256-point
   sphere set for sky and bounces (§2.3), the per-direction LCG jitter block
   (seed 0x7d3fb6ac) for the peel camera, the disc grid (§2.2) only for
   directional-light passes that frame 0 does not run. Fix: read
   `Std.PointsInSphere.Gbx` (banked), take its 256-set, reproduce the LCG;
   the noise pattern then matches the editor's texel for texel.
8. **Mood parameters by DayTime quarter**, not by the decoration's name
   (§6). Fix: read `0x03043056`, pick the quarter's XML; the frame record's
   MaxHDR_Mood/Bounce/Sky must be that mood's.
9. **Frame record.** lmtool copies a template's. Client: per-bake `MaxHDR =
   min(peak irradiance, Mood.MaxHDR)`, `MaxHDR_HBasisScaled234` = the three
   directional maxima, DayTime = the map's word. Fix: compute them.
10. **Per-chart byte.** lmtool: `fb0 = 255·max/K` with a fitted K. Client
    (§2.5, `FUN_14029add0`): on the finished 8-bit colour image, `byte =
    max(R,G,B)` over the chart's node rect, then the rect's pixels ×
    `255/byte` truncated (bytes 1..254 only); frames 1, 2 alike. Fix: port
    it verbatim after the compress emulation; drop the fit.
11. **Chart density and packing.** lmtool: `u02 × 0.5625` per metre, fixed,
    own packer. Client: §3.1 exactly — W = H = 2048 with g = 2, pad = 1, m =
    6 derived from W; `D = W·H/Σarea`; shrink ×0.9 until it packs, then
    5/7/9 bisection steps (Default/High/Ultra); charts sized `floor(ext·t)`
    with the positive area-deficit carry and the +1 bumps; stable radix order
    by area with the +0x40/+0x3c/+0x38/|extent|² tie-breaks, largest first;
    the binary-tree packer with the `dw > dh` cut rule; x = node.x + 1, w =
    node.w − 2. Fix: port §3.1 verbatim (~200 lines); the 0.5625 was
    `s_final/2` for the tiny maps' Σarea.
12. **Probes.** Image 1 = sky (dome-direction) visibility fraction, image 2 = the
    unoccluded ambient (inferred), validity = front-face fraction over the
    directions (§3.2); pixels are **sRGB(value/scale)**, not sqrt and not
    linear. lmtool's "inside test by back-face rays" is the same idea; make
    the threshold, the cone and the encoding match.
13. **Local lights.** lmtool: `0.27·I·colour·n·l·(1−(d/R)²)²·spot`. Client:
    `LightRgb · att · spot · n·l · shadow`, `att = max(0, 1 − d²/R²)` (or the
    HN2 form when the light carries it), `spot = smoothstep` on the cosine
    window, half-angle cones. Fix: (1 − x²) not (1 − x²)²; the 0.27 was the
    linear-vs-sqrt mismatch — re-fit after item 1 (expect ≈ 1).
