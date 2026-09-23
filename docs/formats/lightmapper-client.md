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
2. The stored lightmap holds **ambient + sky + indirect light only — there is
   no direct sun in it** [DISASSEMBLY]. The runtime material shaders
   (`Tech3/Block_*_p.hlsl`, e.g. `Block_TDSN_COut_DecalMod_p` blob 1, lines
   225–735 of the disassembly) sample the real-time cascaded sun shadow maps
   (`TBindedMapShadowLDir0`, `…_Clip`, `GbxP_Pssm_*`) and add
   `shadow · π · GbxP_LightDirRgbLinear0 · saturate(n·−GbxP_LightDirDirInWorld0)`
   themselves; the lightmap textures are `TBindedMapLM0_LDiffuseAmb0..3`
   ("L diffuse ambient"). The lightmapper renders the sun only as the *input*
   of the bounce passes (`GeomILightIn0_p`, §2.4).
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
| sun (for the bounce input only) | `RenderLightDir` 0x14023809e, `RenderLightDirMulti` 0x140237d37, `RenderLightDir0ToBitmap` 0x14022fd8a | `LmLBumpLDir_Inst_p`, `LmLHBasisDirect_Inst_p`, `ZOnlyParaboloid_Inst_v` (ball-light shadow cubes) | §2.2 |
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

### 2.2 The sun — rendered, not stored [DISASSEMBLY]

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
(`RenderLightDir` 0x1402381ef); the sum over the samples is the sun term.
It feeds the bounce only (§2.4). The bump permutation writes the three RNM
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

**What the sky actually is** [DIFFERENTIAL, baker 2026-09-23 on BlueBay
Sunset-quarter test bakes; DISASSEMBLY for the mechanism]: the stored frame
0 has **no unoccluded ambient** (a wall reads 0.09 HDR where
`LAmbient·(0.8+0.2n.y)` would give ≥ 0.32) and the "sky" is a **narrow
cone around the zenith**: a 16 m plate 15.8 m above a pad darkens the pad
centre to 0.21 of the open value (a hemisphere would leave 0.76), the edge
to 0.45, +4 m outside 0.78, +8 m 0.93; a vertical wall reads 0.15 of the
floor. A cosine-weighted cone of half-angle **30–32°** fits (rms 0.055). An
open horizontal floor reads **1.55 × LAmbient in exactly LAmbient's hue**
(0.61, 0.58, 0.77 HDR for Sunset). This is the §2.2 cone generator applied
to a **zenith-pointing directional light of angular radius ≈ 30° and
LAmbient-hued colour** — the sky is modelled as a wide directional light
with shadow maps, not as a hemisphere. Where its angle and colour scale are
set is the one CPU item still open (candidates: the `CHmsLightMapMood`
defaults, §6, or the mood's `LAmbient` light object); until then use A =
30°, colour = 1.55·LAmbient·SkyFactor. The `LmLBumpAmbient` shader (§2.1)
is therefore not part of the stored frame (the preview/in-gameplay path,
or the runtime's use of the record's `StoreLAmbient`/`LAmbient`).

### 2.4 Bounces [DISASSEMBLY]

`GeomILightIn0_p` builds the peel input radiance of a surface point:

```text
ILightInput = ( C0(lightmap so far, decoded) + LightDirRgb · max(0, n·−LightDirDirW) · shadow_LDir0(P) ) · MDiffuse(P)
```

i.e. (ambient + sky + previous bounces, **plus the direct sun**) × the
material's real diffuse albedo (`TMapLM_MDiffuse`, the material textures
rasterised into lightmap space by `Compute_MDiffuse`) — no constant albedo.
`RenderLightIndirectBounces` ("Lighting bounce %1", "%d/%d") repeats the
dome sweep with this input; `ComputeBounces_SpherePoints` (0x140a9e6b4)
draws the sphere directions: `n = min(n, g_MaxSpherePoints)` (global at
0x14205c850; the caller passes 32 with a 64 budget), two passes of a
stratified sphere-point generator (`FUN_14045fbd0(count·n)` then pick the
subset `pass % n`), so consecutive bounce passes use different direction
subsets. `BounceFactor` (2, 1.6 on BlueBay Sunrise, 1.8 Stadium Sunset, 3
RedIsland Night) is the mood's multiplier on the bounced radiance
[FILE: frame record; the exact multiply site is pending]. **Bounce iterations per quality**
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

**The per-chart byte** (`z4`, one per chart per frame, map-lightmap.md §3.4):
the chart's own maximum relative to the frame's `MaxHDR`
[DIFFERENTIAL §3.4 + the encoding above]: `fb = round(255 · chartMax /
frame.MaxHDR)`, and the chart's pixels are `sqrt(E / (fb/255 · MaxHDR))`
[INFERRED — the per-chart compress dispatch is the pending CPU item; the
per-image maxima buffer `g_In_MaxHdrHBasis` is bound per chart]. Frame
records [FILE, `clientre lmimages`]: 66-byte `SFrame` × 3 right after the
constant head (no count word): `{u32 Bump, u32 0, u32 DayTime, f32
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
2048, 1024, 512, 256, 128}) is read by `RenderLightDirectGetLocalLightDescs`
and the bounce/NLocal code with a per-light class column — the local
lights' shadow-map sizes per quality. The stored `m_LightAmbSampleCount =
256` (the sky cone's direction count) has no per-quality table in the
ranges read so far; the texel budget (`WantedTexelByMeter`) per quality is
still pending.

## 3. Charts, packing, probes

### 3.1 Chart allocation [DISASSEMBLY `AllocateWithScale_BlockSplit` 0x140295516, `AllocateWithScale_` 0x140294860]

`m_AllocMode = 2` = `BestSize+UseFree`: the layout is a fixed 2048×2048
texel space (1024² images, map-lightmap.md §2) and the **texel density is
whatever fills it**: `[+0x1c] = (W·H) / Σ surface` (texels per surface unit
over every chart's surface), a scale `[+0x20] = 1`, and if any chart's side
`sqrt(scale·density)·extent` exceeds the atlas side the scale is cut to
`1/ratio²`; `AllocateWithScale` then packs at `sqrt(scale·density)` and
retries with a smaller scale until the packing succeeds (states 0x3eb). The
per-chart "surface" is the sum of four floats per chart (the visual's uv-1
extent × `PreLightGen.u02` metres-per-uv, [INFERRED]). The measured
"`u02 × 0.5625` layout texels per metre" of map-lightmap.md §5 is the value
this search lands on for the tiny maps' total surface, not a constant of the
game — a map with more lightmapped surface gets smaller charts. Chart sizes
are quantised (even sizes, odd positions, 1-texel gutters — `TinyAlloc_16b`
in `EQualityVer`).

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

## 4. Object order

The chart→object index is the "IdForLightMap" assigned by
`CGameCtnChallenge::AutoSetIdsForLightMap` (0x140b88a1c) and bound to the
scene by `NGameMgrMap::IdsForLightMap_BindToScene` (0x140dc5c4c);
`TransferIdForLightMapFromBakedBlocksToBlocks` (0x140b91f00) moves ids from
the file's baked records to the regenerated blocks, `LightMapGetMostRecentBlock`
(0x140b965d0) is the "MostRecentBlock" timestamp the cache checks. The
measured rule (map-lightmap.md §3.6: `[decoration][authored blocks][generated
blocks][items]`, `base = P + N_authored + (S_x·S_z − replaced) + G`) is the
order these functions enumerate: blocks (authored, then the game's
generated/baked list in generation order), then items. `G` is the count of
generated pieces — the same list `mapgeom bake` simulates (clip fillers,
pillars, aprons) — so predicting `G` offline = running that simulation on
the map; the lightmapper itself just walks the challenge's BakedBlocks
array [DISASSEMBLY: the functions exist and are named so; their bodies are
coroutine-split, exact walk **pending**].

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
  these is **pending** (it only matters for the bounce input).
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
3. **No direct sun term.** lmtool adds `sun·max(0,n·L)·sunVis` to frame 0.
   Client: none (real-time). Fix: drop it from E; keep the sun only inside
   the bounce input (§2.4).
4. **Ambient shape.** lmtool: `ambient + up·(0.5+0.5 n.y) + sky·skyVis`
   (fitted). Client (stored frame): **no unoccluded term at all** — the sky
   cone (item 5) plus the bounces; nothing else. Fix: drop `ambient` and
   `up`.
5. **Sky = a 30° zenith cone light.** lmtool: 64 cosine-weighted hemisphere
   rays × a fitted sky colour. Client: N (= 256) uniform directions inside a
   cone of half-angle ≈ 30° around +y (taken from the game's sphere point
   table), each a shadow-mapped directional light of colour ≈
   1.55·LAmbient·SkyFactor, summed with `max(0, n·D)/N`. Fix: replace the
   hemisphere with the cone (this is why the editor's shadows under plates
   are near-black and lmtool's were grey, and why walls are dark).
6. **Bounce.** lmtool: one bounce, constant albedo 0.5, off by default.
   Client: 2 (Default) / 4 (High) / 6 (Ultra) sweeps with the material
   albedo (`MDiffuse`) and the direct sun on the bounce surfaces, ×
   `BounceFactor` (2 / 1.6 / 1.8 / 3); the sea/ground bounce is what lights
   undersides (0.37 of the open floor). Fix: albedo from the material diffuse
   textures (mean colour per material is enough at lightmap resolution), the
   sun (64 disc samples, its EmittAngularSize) on the bounce input,
   BounceFactor from the quarter-mood XML, 2 iterations for a Default bake.
7. **Direction sets.** lmtool: stratified random. Client: the precomputed
   `Std.PointsInSphere.Gbx` sets (banked under client-re/): the sky cone =
   the points of the 4112-set inside 30° of +y (≈ 275 → the stored 256),
   the sun = a 30°-rotated jittered grid of 64 in the disc of radius sin(A),
   the bounces = the same table through `ComputeBounces_SpherePoints`
   (64·n and 32·n point sets partitioned into n interleaved subsets, one
   subset per pass). Fix: read the table and use the same sets; the noise
   pattern then matches the editor's texel for texel.
8. **Mood parameters by DayTime quarter**, not by the decoration's name
   (§6). Fix: read `0x03043056`, pick the quarter's XML; the frame record's
   MaxHDR_Mood/Bounce/Sky must be that mood's.
9. **Frame record.** lmtool copies a template's. Client: per-bake `MaxHDR =
   min(peak irradiance, Mood.MaxHDR)`, `MaxHDR_HBasisScaled234` = the three
   directional maxima, DayTime = the map's word. Fix: compute them.
10. **Per-chart byte.** lmtool: `fb0 = 255·max/K` with a fitted K. Client:
    K = the frame's `MaxHDR` (§2.5). Fix: K := frame.MaxHDR (drop the fit).
11. **Chart density.** lmtool: `u02 × 0.5625` per metre, fixed. Client:
    fills the 2048² layout — density = budget/Σsurface, shrunk until it packs.
    Fix: keep the constant for the tiny maps (it is what the search yields
    there); for the giants implement the search (§3.1) so charts match the
    editor's sizes.
12. **Probes.** Image 1 = sky-cone visibility fraction, image 2 = the
    unoccluded ambient (inferred), validity = front-face fraction over the
    directions (§3.2); pixels are **sRGB(value/scale)**, not sqrt and not
    linear. lmtool's "inside test by back-face rays" is the same idea; make
    the threshold, the cone and the encoding match.
13. **Local lights.** lmtool: `0.27·I·colour·n·l·(1−(d/R)²)²·spot`. Client:
    `LightRgb · att · spot · n·l · shadow`, `att = max(0, 1 − d²/R²)` (or the
    HN2 form when the light carries it), `spot = smoothstep` on the cosine
    window, half-angle cones. Fix: (1 − x²) not (1 − x²)²; the 0.27 was the
    linear-vs-sqrt mismatch — re-fit after item 1 (expect ≈ 1).
