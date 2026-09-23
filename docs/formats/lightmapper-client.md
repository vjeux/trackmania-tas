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
Color Scale/><Atmo2 …/>`, the gradient textures `<Collection>\Media\Moods\
<Mood>\SkyColor.dds` (TMapGradientV, BC6H_UF16 HDR 2048×1024, 12 mips) and
`SkyClouds.dds` (TMapGradientV1, the clouds layer, `SkyUseClouds`)):

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
&set, N)` picks the set for N; sweep-specific overrides at `lm+0x500 +
sweep·0x10` when present), then every point is rotated by the fixed matrix
`M = Rz(0.313338965) · Ry(0.0599014498) · Rx(0.124326788)` (radians:
17.953°, 3.432°, 7.123°; standard right-handed rotations, `d = M·p`, float32
row·point sums) — and, when `CHmsLightMapMood+0xcc ≠ 0` and
`FUN_14020cde0(lm) == 0`, every direction's y is forced negative (`y = −|y|`,
the set folded onto one hemisphere). A uniform **sphere** set with `Scale =
4/N` makes `Σ_D Scale·max(0,n·D)` exactly 1 for a uniform environment
(downward directions see the peeled ground = bounce, upward ones the sky);
the underside readings (0.37–0.4 of the floor) say the fold is off for the
campaign moods. The sweep order interleaves the list into ss² groups (ss =
the supersample factor, 3 → 9; `FUN_140460270` round-robin) — order only.
`PeelDirInW` = the list direction itself. The LCG
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

**The chart list** (`FUN_140291450`): one 0x18-byte record `{model*, u32
flags|subvisual, u32 blockparam, u32 blockIndex, u32 group}` per 0x58-byte
block record when the model's sub-visual count (`PreLightGen+0x48`) is < 2 —
`flags = 0, group = −1`. A model with n ≥ 2 sub-visuals (the
`Item_Prefab_MultiMesh` case) gets **two** records: `{flags 0x10000 | 0,
group −1}` = sub-visual 0 on its own, and `{flags 0x20000, group g}` = sub-
visuals 1..n−1 merged: `FUN_14028f1d0` packs their uv rects into one rect
(`FUN_141402b00`, spacing 0.015) and stores the average `MeterByUv` and the
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
  12 mips — `TMapGradientV`), `SkyClouds.dds` (`TMapGradientV1`),
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
   directions of the PointsInSphere 256-set (order interleaved in 9 groups),
   each an orthographic depth-peel; a texel that sees no surface along `D`
   sees `Sky_p(D)` = `SkyColor.dds` (u shifted by the sun azimuth) ×
   ScaleGrad0 + clouds layer + the two `Atmo` glow lobes around the sun,
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
