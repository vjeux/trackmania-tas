# Trackmania.exe crash @0x140F5633C — static RE report

Binary: Trackmania 2020 3.3.0 (2026-02-02), ImageBase 0x140000000. `.pdata` in this
build is garbage (protector), so function bounds were found from int3 padding.

## 1. The function: `NGameItemUtils::CreateSolid2Model` @ 0x140F558D0

Bounds 0x140F558D0 .. 0x140F5648A (ret), int3 padding on both sides. Identified by
the profiler-scope string passed at entry:

```
140f5590c: lea rdx,[rip+0xd43c9d]   # 0x141c995b0 = "NGameItemUtils::CreateSolid2Model"
140f5591a: lea rcx,[rbp-0x30]
140f5591e: call 0x140117690         ; scope/profile marker (same idiom in LoadPlugCrystalMaterials)
```

Frame: `rbp = entry_rsp - 0x118`, so `[rbp+0x118]`=retaddr, `[rbp+0x120]`=home of
arg1 (rcx), `[rbp+0x128]`=home of arg2 (edx), `[rbp+0x148]`, `[rbp+0x150]`,
`[rbp+0x158]` = stack args 6/7/8 (the arg8 slot is reused as a local flag).

Signature (recovered):
```
CPlugSolid2Model* NGameItemUtils::CreateSolid2Model(
    CPlugCrystal* crystal /*rcx -> r15, later rdi*/, int mode /*edx -> r14d*/,
    int r8d, CPlugCrystal_Mesh* mesh /*r9 -> rsi*/, ..., arg7 /*[rbp+0x150]*/, ...)
```

### Objects
* **rdi at the crash = `crystal` = `CPlugCrystal*`** (reloaded from the home slot:
  `140f562e2: mov rdi,[rbp+0x120]`). Fields used:
  * `+0x48` Materials array (32-byte entries), `+0x50` Materials count.
    Entry layout (from LoadPlugCrystalMaterials @0x140F4B600 and the ArenaBank fixup
    @0x140F54DAE): `+0x00 CPlugMaterialUserInst*`, `+0x08 CFastString MaterialName`
    (ptr `+0x08`, len `+0x14`), `+0x18 CPlugMaterial*` (resolved material).
  * `+0x60` list header used with 0x1413f1750 (iterator) — this is the mesh's face
    list when `crystal` is passed as the "mesh" (see `140f5625f`).
  * `+0x98/+0xa0` Vec2 array (lightmap UVs, chunk 0x09003006) copied per face
    vertex into face `+0x44..` UV slot 1 (`140f55af0..140f55c1a`), guarded by
    `sum(face.vertexCount) == count` (`140f55a58`).
  * `+0xa8/+0xb0`, `+0xb8/+0xc0` per-face int arrays (0x09003007 data): the second
    is written into `face+0xc` only if `count == faceCount` (`140f55ca7..140f55d04`).
  * `+0xf0/+0xf8` Layers (pointer array, `layer+0x8` = type; type 0 Geometry
    (`layer+0x38` = mesh), 0xd Cubes, 0xe Trigger, 0x11 light-like
    (pos/color at +0x48.., node +0x70 → Solid2Model +0x168 list), 0x12 node
    instances (+0x48/+0x50 nodes, +0x58/+0x60 placements → Solid2Model
    +0x178/+0x188)).
* **r13 = the freshly created `CPlugSolid2Model*`**: returned by
  `140f55e41: call 0x14065d060` = CPlugModelTree→CPlugSolid2Model export (that
  function's log strings: `"CPlugModelTree=>CPlugSolid2Model : BBox joint not
  found!"` @0x141be2538, `"...Lod distance fusion not supported yet"` @0x141be2580).
  Fields: `+0xc8/+0xd0` **Materials** (`CPlugMaterial*[]`), `+0xd8/+0xe0`
  MaterialIds, `+0xf8/+0x100` **CustomMaterials** (24-byte `{CPlugMaterialUserInst*,
  CFastString name}`), `+0x158/+0x160` ShadedGeoms (`+4` = material index),
  `+0x208` visuals, `+0x344` flag.
* The `mesh` (r9/rsi) is the crystal geometry: `+0x60` face list (face: `+0x10`
  UV-set count, `+0x44..` UV sets stride 0x20, `+0x60` vertex count, `+0xf0`
  **material index**, `+0xf4` flags — bit0 clear = skipped, test 0x140F53C10),
  `+0x8/+0x10` groups (`group+0 == 1` → ArenaBank "Damage" special case).

### How the crash-loop operands are computed

```
140f55d10: call 0x140f53be0                 ; r14d(mode) -> {0x2841,0x861,0x8e3,0x841} flag word -> [rbp-0x70]
140f55d1b: lea r9,[rbp-0x80]                ; &map  (CFastArray<int>: ptr -0x80, count -0x78)
140f55d22: lea r8,[rbp-0x70]                ; &flags
140f55d26: call 0x140f549a0                 ; (crystal, mesh, &flags, &map) -> CPlugModelTree*  (rdi)
...
140f55e41: call 0x14065d060                 ; CPlugModelTree => CPlugSolid2Model   -> r13
```

`[rbp-0x80]` / `[rbp-0x78]` (index map & its count) are filled inside
0x140f549a0 → 0x140f54bf0 ("build model tree from crystal"):

```
140f54fe5: mov  r14d,[r13+0x50]             ; r14d = crystal.Materials.Count
140f54fed: call 0x14010be60                 ; map.Clear()
140f54ff5: call 0x14012a250 (rdx=r14d)      ; map.SetCount(Materials.Count)   <-- [rbp-0x78]
140f5500a: call 0x1401ff5d0 (rdx=&-1)       ; map.Fill(-1)
; for each face f of mesh (list at mesh+0x60, iterator 0x1413f1750):
140f55041: call 0x140f53c10                 ; skip face if (~f.flags(+0xf4)) & 1
140f5504a: mov  eax,[rcx+0xf0]              ; m = face.MaterialIndex
140f55050: cmp  eax,-1 ; je skip
140f55055: cmp  eax,r14d ; jae skip         ; m >= Materials.Count -> ignored (no map entry!)
140f5505f: cmp  [map+m*4],-1 ; jne skip
140f55065: mov  [map+m*4],edi ; inc edi     ; map[m] = next sequential "used material" index
140f5508b: call 0x140569bc0 (rbx, edi)      ; tree.SetMaterialCount(usedCount)
; for i in 0..map.Count:  if map[i]!=-1:
140f550bc: mov  rax,[r12+0x48] ; rdi=[rax+i*0x20]      ; crystal.Materials[i].MaterialUserInst
140f550ca..: if null -> new CPlugMaterialUserInst(0x248) with Id "ErrorMat"
140f55121: call 0x140569c90 (rbx, map[i], rdi)         ; tree.Materials[map[i]].UserInst = it
```

So `count = crystal.Materials.Count`, `map[i] ∈ {-1} ∪ [0, usedCount)` where
`usedCount` = number of DISTINCT crystal material slots referenced by at least one
non-skipped face.

The destination size `[r13+0x100]` = `Solid2Model.CustomMaterials.Count`, produced by
the model-tree export (0x14065B890, per geom-material group):

```
14065bbd5: cmp  [r13+0x18],0 ; je ...        ; group has a MaterialUserInst
14065bbfa: mov  esi,[rbp+0x100]              ; existing CustomMaterials count
14065bc10: mov  rax,[rbp+0xf8]
14065bc1d: mov  rdx,[r13+0x18]
14065bc21: mov  rcx,[rdi+rax]                ; existing[j].UserInst   (stride 0x18)
14065bc25: call 0x1404fd870 (rcx, rdx, 1)    ; CPlugMaterialUserInst::IsEqual
14065bc2c: jne  -> reuse index j             ; <== DEDUPLICATION
14065bc43: ... call 0x1406636b0 (&[rbp+0xf8]) ; else append new CustomMaterial
14065b9c3: mov  [rax+4],ebx                  ; ShadedGeom.MaterialIndex = j
```

Mixing checks in the same function: a group with a CPlugMaterial (`+0x10`) requires
`CustomMaterials.Count==0 && MaterialIds.Count==0`, etc., else
`"Can't mix Materials with MaterUserInsts or MaterialIds..."` (0x141be2360).

### The crash loop itself (0x140F561B1 .. 0x140F56379)

```
140f561b1: cmp  [rbp-0x78],0                 ; map.Count != 0
140f561b5: mov  [rbp+0x128],1                ; flag = "all used materials are Editors\ materials with a loaded CPlugMaterial"
140f561c5: mov  eax,[r15+0x50]               ; Materials.Count
; for i: if map[i] != -1:
140f56203: mov  rcx,[Materials + i*0x20] ; add rcx,0x30   ; UserInst->Link (CFastString @+0x30, len @+0x3c)
140f5620b..140f5622a: len>=7 && memcmp(Link, "Editors", 7)==0   (0x141bb8de8 = "Editors")
140f56237: cmp  [Materials + i*0x20 + 0x18],0 ; je -> flag=0   ; resolved CPlugMaterial must be non-null
140f56258: mov  [rbp+0x128],0
...
140f562ad: cmp  [rbp+0x128],0 ; je skip
140f562ba: cmp  [rbp-0x78],0  ; je skip
140f562c4: mov  edx,[r13+0x100]              ; CustomMaterials.Count
140f562cb: lea  rcx,[r13+0xc8]
140f562d2: call 0x14017fb30                  ; Solid2Model.Materials.SetCount(CustomMaterials.Count)  (new slots zeroed only when realloc'd)
; for i in 0..map.Count:
140f56307: mov  ecx,[map + i*4]  ; k
140f5630f: mov  r15,[r13+0xc8]               ; dst = Materials.data
140f56319: mov  rax,[rdi+0x48]
140f5631d: mov  rcx,[r15+k*8]                ; old = dst[k]          <-- NO BOUND CHECK vs [r13+0x100]
140f56321: mov  rsi,[rax+i*0x20+0x18]        ; new = crystal.Materials[i].Material
140f56330: inc  [rsi+0x10]                   ; new->AddRef
140f5633c: add  [rcx+0x10],-1                ; old->Release   <== CRASH (rcx = garbage read past the array)
140f56358: mov  [r15+k*8],rsi                ; dst[k] = new
```

`0x14017fb30` (SetCount for a node array) releases+nulls trailing elements on
shrink and tail-calls 0x14017fd00; on growth beyond capacity 0x14015cd60 →
0x14015cfb0 allocates and memsets to zero, but reads at `k >= count` are outside
the array (or, when within spare capacity, stale). Hence `rcx` = heap garbage
(0x140003C0D / 0x8807F6F6F61D7DA1 in the two logs).

## 2. Input data that triggers it

Precondition for the loop to run at all: every crystal material slot used by a face
has a `CPlugMaterialUserInst` whose `Link` starts with `"Editors"` (mesh-modeler
library materials, e.g. `Editors\MeshEditorMedia\...`) and has a resolved
`CPlugMaterial` (`entry+0x18`, filled by `NGameItemUtils::LoadPlugCrystalMaterials`
@0x140F4B600 from the global editor-material table `[0x142021eb0][UserInst+0x148]`).

Mismatch: `usedCount` (distinct **slots** used, from 0x140F54FDF) vs
`CustomMaterials.Count` (distinct **materials by value**, from 0x14065B890). The
copy loop indexes the second array with indices from the first.

**Trigger**: in chunk `0x09003003` (CPlugCrystal materials list) two or more
entries that are used by faces (chunk `0x09003005` geometry-layer face material
indices) and whose `CPlugMaterialUserInst`s compare equal under
`CPlugMaterialUserInst::IsEqual` @0x1404FD870 — trivially true when two list entries
reference the **same node** (same GBX node-ref index: `1404fd90d: cmp rdi,rbx` →
equal), or two distinct nodes with identical content (`+0x28` Id, `+0x30` Link,
`+0x40`, `+0x48`, `+0x50` string, `+0x60/+0x68` array, `+0x14c/+0x150` array,
`+0x1d0`, `+0x220/+0x224`, bytes `+0x148` (editor material index), `+0x149`
(physics id)). Each duplicate that is used by ≥1 face makes `usedCount` exceed
`CustomMaterials.Count` by one, so the highest `map[i]` values read past the
destination array → garbage `Release()`.

Face material indices themselves are NOT the trigger: `m == -1` or
`m >= Materials.Count` is ignored by the remap (`140f55050/140f55055`) and by the
geometry pass (`140f553c5/140f553cb` → geom material -1). Also
`LoadPlugCrystalMaterials` phase 3/4 (0x140F4C04F..0x140F4C3B0) redirects such
faces to an appended fallback material before we get here.

Secondary (unverified) path with the same effect: a used material slot whose faces
all get dropped by the geometry export (no geom group for it) would also make
`CustomMaterials.Count < usedCount`.

## 3. Other invariants enforced on crystal materials

* `LoadPlugCrystalMaterials` (0x140F4B600):
  * `Link` starting with `"Editors"` → `entry.Material` (+0x18) := global table
    `[0x142021eb0][UserInst.byte+0x148]`, cloned with the physics id
    (`byte+0x149`, table `"Test/Rock/Water/Sand/Turbo_Deprecated/DirtRoad/Rubber/
    WetPavement..."` @0x141bbc428) if non-zero (`140f4b6d0..140f4b7a1`).
  * Non-Editors: entry with no name AND no UserInst → dropped (map -1, flag
    cleared, `140f4ba61..140f4ba78`); otherwise resolved via 0x1404FDAE0; failure
    logs `"Material: could not load "` / `"Material: could not find id %s"` /
    `"Material: unknown collection folder %s"` (0x141c99218/0x141c99280/0x141c992a0).
  * Materials are compacted with a remap (`[rbp+0xb8]`, `140f4b9b3..140f4c035`), and
    every Geometry/Cubes/Trigger/type-0x13 layer face with material `-1` or
    `>= Count` sets `esi=1` → a fallback material is appended and all such faces
    are redirected to it (`140f4c360..140f4c37f`: `face.mat = valid ? map[m] :
    map[last]`). No de-duplication of equal UserInsts here — duplicates survive.
* `CreateSolid2Model` build (0x140F54BF0):
  * A used slot with null `MaterialUserInst` gets a fresh `CPlugMaterialUserInst`
    with Id `"ErrorMat"` (`140f550ca..140f5510a`); its empty Link then fails the
    "Editors" test, so the crash loop is skipped.
  * ArenaBank special case: if any mesh group has `+0 == 1` and a material named
    `"\Projects\Lagoon\Media\Material\ArenaBankTest.Material.Gbx"` lacks a
    resolved material, it is replaced by a UserInst with Link
    `"Lagoon\Media\Material\ArenaBankTest"` (`140f54cdb..140f54fb1`); with flags&2
    nine `"Damage%u"` names are generated (`140f54a2a..140f54ac7`).
  * Lightmap UVs (`+0x98/+0xa0`) are only applied when their count equals the
    total face-vertex count (`140f55a58`); the per-face int array (`+0xb8/+0xc0`)
    only when its count equals the face count (`140f55cb2`).
* Model-tree export (0x14065B890): a geom group must have exactly one of
  {CPlugMaterial, CPlugMaterialUserInst, MaterialId} kind consistent with the rest of
  the model, else `"Can't mix Materials with MaterUserInsts or MaterialIds. if from
  max, check your materials props."`; a group with none of the three aborts the export
  of that node (`14065bd3c`, returns 0). Ordering of the materials list is otherwise
  irrelevant (indices are remapped); the only ordering-sensitive thing is the dedup
  above.

## 4. Class identification cross-checked with /tmp/OpenplanetNext.json (object sizes)

The Openplanet dump has no member offsets, but its class sizes match every
allocation seen (`0x14054ed90` = operator new with size in ecx, followed by the ctor):

| alloc size | ctor | class (dump) | role here |
|---|---|---|---|
| 0x100 (256) | — | `CPlugCrystal` 0x09003000 | `rdi`/`r15` (highest field used: `+0xf8`, fits exactly) |
| 0x390 (912) | via 0x140662080 | `CPlugSolid2Model` 0x090BB000 | `r13` (fields up to `+0x344` used) |
| 0x2b0 (688) | 0x1405694c0 | `CPlugModelMesh` 0x09073000 | `rbx` in 0x140F54BF0: the mesh builder (materials `+0x140/+0x148`, 32-byte slots) |
| 0x178 (376) | 0x140565a30 / 0x1406625a0 | `CPlugModelTree` 0x09072000 | wraps the CPlugModelMesh, input of 0x14065D060 |
| 0x248 (584) | 0x1404fcc20 | `CPlugMaterialUserInst` 0x090FD000 | "ErrorMat" / ArenaBank replacements, chunk 0x09003003 v<1 inline insts |
| 0x190 (400) | 0x1404d96f0 | `CPlugSkel` 0x090BA000 | created at 0x140F5607B, stored at Solid2Model `+0x78` |
| 0x58 | 0x140522930 | GeometryLayer (crystal layer type 0) | `+0x38` = mesh, `+0x18` = ?, `+0x40` string |

`CPlugSolid2Model+0xf8/+0x100` = **CustomMaterials** is independently confirmed by the
Solid2Model archive at 0x140437A20: it iterates a 0x18-stride array at `+0xf8`,
reading a string at `+0x8` (len `+0x14`) and, when the name is empty, a node ref of
class `0x090FD000` (CPlugMaterialUserInst) at `+0x0` — exactly the
`Material {string MaterialName; CPlugMaterialUserInst}` archive in
CPlugSolid2Model.chunkl. `+0xc8/+0xd0` = **Materials** (`CPlugMaterial*[]`, 8-byte).
CPlugMaterialUserInst field offsets (from IsEqual @0x1404FD870 and the loaders):
`+0x28` Model (MwId), `+0x30` Link (string, len `+0x3c`), `+0x148` editor material
index byte, `+0x149` PhysicsID byte, `+0x14c/+0x150` textures array, `+0x1d0` ?,
`+0x220/+0x224` TilingU/V or similar.

Chunk 0x09003003 reader (0x140524201..0x1405242C1): per 32-byte entry — v<1: create a
CPlugMaterialUserInst inline and call its archive (`vtbl+0x70`); v≥1: read string
`MaterialName` into `+0x8`, and if its length is 0 read a node ref into `+0x0`
(0x14043e7d0). `+0x18` is never read from the file — it is the runtime-resolved
CPlugMaterial. GBX.NET's `Material` archive matches.

## 5. The crystal `U02`/`U03` fields (mesh archive 0x1413D05C0)

The Crystal mesh archive is `0x1413D05C0` (called from the GeometryLayer archive
0x140532EE0 via `vtbl+0x8`). After the groups (pointer array `mesh+0x8/+0x10`; each group object: `+0`
U01 int, `+4` U02 byte (v≥35) / int, `+0x18` U03, `+0x8` Name, `+0x30` U04 int
(v≥23; **reset to -1 unless `< [mesh+0x20]`**), `+0x20` U05 int[]) come the
embedded flags at `mesh+0x98` (3 ints for v∈[25,29), 1 int for v∈[29,35), 1 byte
for v≥35), then for v≥33 two ints:

```
1413d0964: cmp  [rsi+0x10],0 ; je read      ; writer path: compute them
1413d0990: eax = face.MaterialIndex ([face+0xf0]) ; r11d = max(r11d, eax)  -> [rsp+0x70]
1413d09a9: eax = face.GroupIndex    ([face+0x8])  ; ebx  = max(ebx,  eax)  -> [rsp+0x6c]
1413d09c8: archive int [rsp+0x70]               ; "U02"
1413d09d5: archive int [rsp+0x6c]               ; "U03"
```

So **U02 = max face material index over all faces, U03 = max face group index**
(not a count, not group-related). They are consumed only as the width selector
for the per-face optimized ints:

```
1413d124a: edx=[rsp+0x70] ; call 0x1413d04c0(archive, U02, &face+0xf0, allowMinus1=1)  ; material index
1413d12c6: edx=[rsp+0x6c] ; call 0x1413d04c0(archive, U03, &face+0x8,  1)              ; group index
```

`0x1413D04C0`: `max < 0xFF` → 1 byte (0xFF ⇒ -1); `max < 0xFFFF` → 2 bytes (0xFFFF ⇒
-1); else 4 bytes. U02 = 7 with 2 groups simply means the item's faces use material
indices up to 7 (8 materials) — it is unrelated to the group count. Note the game
uses the stored U02/U03 for the width, whereas GBX.NET uses `Materials.Count` /
`Groups.Length`; they agree only when both sides of the 0xFF/0xFFFF thresholds
match. The reader does **not** range-check the face material index against
Materials.Count here (that happens later in LoadPlugCrystalMaterials).

## Files
* /tmp/crash-re/func.asm       — CreateSolid2Model 0x140F558D0..0x140F5648A
* /tmp/crash-re/remap.asm      — 0x140F549A0 / 0x140F54BF0 (map construction) + helpers
* /tmp/crash-re/f65b890.asm    — model-tree export material handling (dedup)
* /tmp/crash-re/loadmats*.asm  — NGameItemUtils::LoadPlugCrystalMaterials
