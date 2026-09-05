# Trackmania.exe — CPlugMaterialUserInst `Color` / `UserTextures` / Editors materials, static RE

Binary: /tmp/crash/Trackmania.exe (TM2020, ImageBase 0x140000000). Tools/scratch in /tmp/color-re/
(`pex` Rust helper, `dis.sh`, `full.asm` = whole .text disassembly, `archive_090fd.asm`,
`editortable.asm`, `f_67e390.asm`, `f_9069b0.asm`, `loadmatlib.asm`, `resload.asm`).
Read-only; the game was not run. Class/field names below come from the Mw reflection tables in
.data and from the Openplanet dump; everything else is from disassembly.

## 0. Ground truth: the class layout and the chunk 0x090FD000 reader

Reflection member table for class 0x090FD000 (records at 0x141ea8700…0x141ea9110, 0x38 bytes each:
`{u32 type, u32 id}, fn, offset, name`):

| member | offset | notes |
|---|---|---|
| `_Name` (id) | +0x28 | `MaterialName` in the chunkl |
| `_LinkFull` (string, CFastString) | +0x30 (heap flag +0x3b, len +0x3c) | the `string Link` |
| `Link` / `_Link_OldCompat` (id) | +0x40 | the `id Link`, also `MaterialId` |
| `Model` (id) | +0x48 | |
| `BaseTexture` (string) | +0x50 | |
| `TexturesDiffuse … TexturesRoughMetal` | accessor (off -1) | 13 texture slots, see §2 |
| `TilingU`, `TilingV` | +0x138, +0x13c | chunk 0x090FD001 |
| `TextureSizeInMeters` | +0x140 | chunk 0x090FD001 |
| `IsNatural` | +0x144 | chunk 0x090FD001 |
| `HidingGroup` (id) | +0x220 | |
| `IsUsingGameMaterial` | +0x224 | |
| (not reflected) SurfacePhysicId byte | +0x148 | |
| (not reflected) SurfaceGameplayId byte | +0x149 | |
| (not reflected) `Csts` count / entries | +0x14c / +0x150 (inline, ≤8 × 0x10) | |
| (not reflected) `Color` values | +0x1d0 ptr / +0x1d8 count (int array) | |
| (not reflected) `UvAnims` | +0x1e0 count / +0x1e4 entries (inline, ≤2 × 0x18) | |
| (not reflected) `id[]` | +0x214 count / +0x218 (inline, ≤2) | |
| (not reflected) `UserTextures` | +0x60 count / +0x68 entries (inline, ≤8 × 0x18: `{int Slot; CFastString Texture}`) | |
| chunk 0x090FD002 int | +0x128 | |
| chunk 0x090FD001 CPlugBitmapAtlas | +0x130 | |
| runtime-resolved CPlugMaterial cache | +0x240 | read at 0x14067e4bb |

Chunk dispatcher `CPlugMaterialUserInst::ArchiveChunk(this=rsi, archive=rdi, id=r8d)` =
**0x1404fced0–0x1404fd701** (0x090FD000 → 0x1404fd16f, 0x090FD001 → 0x1404fcf5f, 0x090FD002 → 0x1404fcf2d).
The 0x090FD000 body matches the chunkl exactly and pins down the two fields asked about:

```
1404fd173  version (v11 written)                         [rbp-0x60]
1404fd1ab  v>=9: boolbyte IsUsingGameMaterial -> ebx     (written as: LinkFull.len != 0)
1404fd1d2  id  +0x28 MaterialName
1404fd1de  id  +0x48 Model
1404fd1ea  string +0x50 BaseTexture
1404fd1f8  v<10: byte +0x148 (remapped via 0x14040db70) ; v>=10: byte +0x148 PhysicId, byte +0x149 GameplayId
1404fd252  id  +0x40 Link          (v1..8, or v>=11 && !IsUsingGameMaterial)
1404fd313  string +0x30 LinkFull   (v9..10, or v>=11 && IsUsingGameMaterial)
1404fd334  v>=12 -> ud2 (unsupported)
--- v>=2 : Csts + Color ---
1404fd357  call 0x1404fde40 (rdi,&+0x14c)   ; count, REJECTED if > 8 (archive error flag [rdi+0x28]=1, count:=0)
1404fd374..3b7  per Cst (stride 0x10 from +0x150): id Name(+0), id Type(+4), int Count(+8), then
                Offset(+0xc) := running sum of Counts (on read it is COMPUTED, on write it is verified: 1404fd3a3)
1404fd3e1  call 0x14014d0c0 (&+0x1d0)       ; int[] Color  == the packed constant VALUES buffer
1404fd3ec  if Color.count < sum(Cst.Count) -> archive error, Csts:=0, Color cleared      (1404fd3f1..416)
1404fd448..4ff  read-side fix-up: for each Cst whose Type id == "Real" (0x141b598b8): for each value in
                [Offset, Offset+Count): if 1 <= (u32)v <= 255  ->  v := (float)v * 1/255  (0x3b808081 @0x141d1edd0)
--- v>=3 UvAnims (+0x1e0, stride 0x18: id,id,u32,u64, v>=5 id) ; v>=4 id[] (+0x214) ---
--- v>=6 UserTextures ---
1404fd5eb  WRITE side: if v>=9 && IsUsingGameMaterial && LinkFull.len!=0  -> UserTextures.count := 0 (not saved)
1404fd5f6  call 0x1404fde40 (&+0x60)         ; count, rejected if > 8
1404fd618..6b6  per entry (stride 0x18 from +0x68): int Slot (+0)  [v<8: old enum remapped by jump table
                0x1404fd704: 0,1,2,3,4,5,6,7,8,9 -> 0,1,4,5,6,7,8,9,10,11], string Texture (+8)
1404fd6cc  v>=7: id +0x220 HidingGroup
```

So, on the file format itself:

* **`Csts` = shader-constant overrides** `{id Name, id Type, int Count, int Offset}`; **`Color` is not a colour
  field at all — it is the flat value buffer of those constants** (Count dwords per Cst, at Offset).
  Values are raw 32-bit words; for Type `"Real"` they are IEEE floats, and a legacy encoding of small
  integers 1..255 is converted to `v/255.0` on load (so the old "RGB 0-255, one int per channel" form is
  still accepted for `Real` constants). A Cst of Count 3 named like the shader's colour constant is an
  RGB float triple; nothing in the reader knows about palettes or indices.
* `UserTextures` = `{int Slot, string Texture}` with Slot indexing the reflected texture slots
  (0 TexturesDiffuse, 1 TexturesDiffuseO, 2 TexturesBaseColor, 3 TexturesBaseColorO, 4 Specular,
  5 Normal, 6 Energy, 7 TeamMask, 8 SelfIllum, 9 Damage, 10 Dirt, 11 Shield, 12 RoughMetal).
* **A UserInst that uses a game material (Link non-empty) is written with an empty `UserTextures` array**
  (0x1404fd5dc–0x1404fd5eb) — the writer already considers textures meaningless for a Link material.

## 1. Does the runtime use `Color` to tint an `Editors\MeshEditorMedia\Materials\…` material?

**No, not on the path that loads items into a map.** Two different code paths exist and only the mesh
editor's one ever looks at the constants:

### 1a. Item loading (game / map editor): `NGameItemUtils::LoadPlugCrystalMaterials` 0x140f4b600

Signature: `(CPlugCrystal* rcx, int collectionId edx, int skipEditorTable r8d, logger r9)`.
All NGameItemUtils callers pass **r8d = 0**:
`0x140f52874` (in `GenerateCommonItemEntityModel`, 0x140f52600–0x140f53822; `xor r8d,r8d` at 0x140f52850),
`0x140f53181 / 0x140f5320f / 0x140f532f0` (same function), `0x140f5a005` (0x140f59f80, `xor r8d,r8d` at 0x140f59ff2).
Only `CGameEditorMesh` (0x1410a2e6a, 0x1410a4c30, 0x1410a4cf9, 0x1410a6996, 0x1410a7632, 0x1410a8027,
collection at `[this+0x4f4]`) passes r8d = 1.

With r8d == 0 the function first runs an **"Editors" pre-pass** (0x140f4b6a6–0x140f4b7ad):

```
140f4b6a6  test edi,edi ; jne 140f4b7ca        ; r8d != 0 -> skip pre-pass (mesh editor)
140f4b6d0  rsi = crystal.Materials[i].UserInst (+0x00 of the 0x20 entry)
140f4b6e0  if UserInst.LinkFull.len (+0x3c) == 0 -> next
140f4b6ea..727  if len < 7 || memcmp(LinkFull, "Editors", 7) != 0 -> next     ("Editors" = 0x141bb8de8)
140f4b730  ecx = UserInst.byte[+0x148]          ; SurfacePhysicId
140f4b737  rdi = g_EditorMaterials[ecx]          ; [0x142021eb0] = CPlugMaterial*[0x50]
140f4b729/747  if UserInst.byte[+0x149] (GameplayId) != 0:
140f4b74e      rdi = clone(rdi)  (0x1409059d0 = serialize+deserialize deep copy)
140f4b75f..76e  clone.SurfaceIds (+0x28) = {PhysicId, GameplayId}
140f4b79c  crystal.Materials[i].Material (+0x18) = rdi   (refcounted swap)
```

Then it falls through into the full path (0x140f4b7ca…), where every entry that already has a resolved
material is **skipped**: `140f4ba55: cmp [entry+0x18],0 ; jne 140f4c011` (0x140f4c011 just records the remap
index). Consequently, for `Editors\…` links loaded through NGameItemUtils:

* the material is selected **solely by the PhysicsId byte (+0x148)**; the text after `Editors` is never
  parsed (the only test is the 7-byte prefix). `Editors\MeshEditorMedia\Materials\Grass` with PhysicId=Metal
  gives the Metal material;
* the GameplayId byte (+0x149) is the only other input (it forces a private clone and is stamped into the
  material's surface-id word);
* `Csts`/`Color`, `UserTextures`, `BaseTexture`, `Model`, `TilingU/V`, `TextureSizeInMeters`, `IsNatural`,
  `HidingGroup` are **never read** on this path — no code touches +0x14c/+0x1d0/+0x60/+0x50 between the
  pre-pass and `CreateSolid2Model`, and the resulting `Solid2Model.Materials[]` holds the shared table
  material (see the "all Editors" copy loop in /tmp/crash-re/REPORT.md, 0x140f562c4–0x140f56358).

`g_EditorMaterials` (0x142021eb0, count at 0x142021eb8) is built once at **0x14040f4d0–0x14040f576**:
80 (`0x50`, one per `EPlugSurfaceMaterialId`) freshly constructed `CPlugMaterial` objects
(`new(0x140)` 0x14054ed90 + ctor 0x14040d070) whose only non-default state is `SurfaceIds.PhysicId = i`
(remapped through 0x14040db70, written at 0x14040f548). They have **no shader variants** (`+0x40 == 0`,
`+0x38` empty), no base material (+0x48), no `CPlugMaterialCustom` (+0x50), no textures, no constants,
no fid. Files reference them by index: the CPlugMaterial-array archive at 0x1404deb00 writes
`bool isNodeRef = (0x1404e1770(mat) == -1)`, and for table materials a `u16 = PhysicId` which is resolved
back through `g_EditorMaterials[u16]` at 0x1404decf9–0x1404ded2c. (Side note: index 0x2e/TechWall coming
from a file named `PlatformDetailsToPlatformPxz.Material.Gbx` is patched to 0x16/ResonantMetal at
0x1404debf8–0x1404decbd.) The inverse — turning a table material back into a UserInst for saving — is at
0x140f4d140–0x140f4d247: it constructs a UserInst with
`LinkFull = "Editors\MeshEditorMedia\Materials\" + PhysicsName[PhysicId]` (name table 0x141ea78e0,
16-byte `{ptr,len}` per id) and `+0x148/+0x149 = material.SurfaceIds`. That is where the
`Editors\MeshEditorMedia\Materials\<PhysicsName>` spelling comes from; it is a label for humans, the
byte is what the loader uses.

### 1b. The full path (mesh editor, r8d = 1) — the only place `Csts` are consumed

Link gate (folders {Test, Editors, Effects} for BlueBay, /tmp/mat-re/REPORT.md) → 0x140f4a8d0 →
0x1404c5510 → **0x1404c4190**, link branch 0x1404c4c51:

```
1404c4cbd  roots = fid manager root(s) ([0x141fbbee0])
1404c4d58  if no extension -> append ".Material.gbx"
1404c4dcc  rbx = 0x1404bfee0(roots, "", link)          ; find the .Material.Gbx fid
1404c4de4  if UserInst.Csts.count (+0x14c) > 0        -> loader B (0x1409069b0)
1404c4df1..e34  else if LinkFull starts with "Editors" -> loader B
1404c4e49  else loader A: 0x1409009a0(out, fid)       ; normal cached load; SurfaceIds patched in place or via clone 1404c4e57..eab
1404c4eb7  loader B: 0x1409069b0(out, fid) ; then 0x140410370(mat)
```

`0x1409069b0` (0x140906930–0x140906a8d) does **not** apply any debug/checker shader. It is
"load an uncached private instance of this fid": it detaches whatever nod is currently cached on the fid
(0x140912480 / 0x1408fb560), calls the normal loader 0x1409009a0, detaches the new nod from the fid
(0x1408fb560 at 0x140906a48) so it is not shared, and re-attaches the previously cached nod
(0x1408fb5d0). `0x140410370` just re-validates the material's shader-variant list (`[mat+0x38]`, count
`[mat+0x40]`, stride 0x38, per-entry 0x14040eae0). So loader B is chosen precisely because the instance is
about to be mutated per UserInst; the material content is whatever the pak's `.Material.Gbx` contains.

Constants are then applied at **0x1404c5035–0x1404c5259** (only if the global "no graphics" flag
`[0x141fbbf08]` is 0; on a server it is skipped):

```
1404c5041  r8d = UserInst.Csts.count (+0x14c)
1404c5055  r12 = mat->Custom (+0x50)  (CPlugMaterialCustom); its constant table at [r12+0x70], count [r12+0x78], stride 0x30
1404c5090  for each material constant j: edx = decl.NameId ([entry+0x18])
1404c50a8    find Cst i with Cst.Name (+0x150 + i*0x10) == edx          ; MATCH BY NAME ID
1404c50c7    esi = decl size in dwords (packed type word at [entry+0x1c]); nRegs = (esi+3)/4
1404c512c    src = UserInst.Color (+0x1d0)[ Cst.Offset (+0x15c+i*0x10) + reg*4 .. ]  (copied 4 dwords at a time into a float4)
1404c5212    0x140443210(custom, &decl, j, &float4, reg)   ; write the constant value
1404c5259  0x140443380(custom)                            ; commit
```

I.e. a `Cst{Name="<shader constant name>", Type="Real", Count=3}` with 3 floats in `Color` overrides that
constant of the *material's own shader* — no fixed "Color" semantics; the names must match constants
declared by the `.Material.Gbx` in the pak. Then, for `Editors` links only (0x1404c5268–0x1404c545f):
the material's `SurfaceIds` are set from +0x148/+0x149 and the texture
`Editors\MeshEditorMedia\Texture\<GameplayIdName>.Texture.Gbx` (folder string 0x141bb8df0, names table
0x141eb2f60, suffix 0x141bb8e18) is bound to every sampler whose name contains `"BaseColor_Over"`
(0x141bb8e28; loop 0x1404c5410–0x1404c5448, setter 0x140443980). This is the gameplay-icon overlay
(Turbo arrows etc.) and shows that the pak's Editors materials do have a `BaseColor` + `BaseColor_Over`
shader; but none of this runs for a map item.

**Bottom line for Q1:** in a BlueBay map the item loader ignores `Color`; the visible material for an
`Editors\…` link is `g_EditorMaterials[PhysicId]` (+ optional GameplayId clone). There is no
per-instance tint reachable from the .Item.Gbx on that path. `CGameEditorMesh.MaterialBaseColors`
(+0x9d4) / `MaterialLastUsedColors` (+0x998) / `CurrentColorForSpecialMaterials` are UI state of the mesh
editor (records 0x141f33fc8 / 0x141f33f60 / 0x141f34030 of the 0x0328B000 member table); the only native
writer found is the initialiser 0x1410a1585/0x1410a159b (empty arrays), plus the swatch palette
0x141aec770 (14 named palettes × 9 vec3, resolved at 0x1412430d0). They do not feed the renderer.

## 2. Are `UserTextures` / `BaseTexture` honoured for such materials?

**No.** In the resolver 0x1404c4190 the very first branch decides the mode:

```
1404c4214  cmp [UserInst+0x40], -1  ; jne 1404c4828      ; MaterialId / id Link set  -> link/id mode
1404c422f  cmp [UserInst+0x3c], 0   ; jne 1404c4828      ; LinkFull non-empty         -> link/id mode
1404c423d  ecx = UserInst.Model (+0x48) ; 0x1404c2530(models, Model) -> index, -1 -> link/id mode too
1404c42ae  r10d = UserTextures.count (+0x60) ; rcx = &BaseTexture (+0x50) ; r14 = &UserTextures[0] (+0x68)
1404c4390..1404c4733  per texture slot of the model (0x230-stride model entry, slot list at +0x30):
           look for a UserTexture with matching Slot (1404c43d0), else derive the name from BaseTexture +
           per-slot suffix table (0x141e6fde0), try ".dds"/".tga"/".png" (0x141ba45f0/0x141ba45e8/0x141bb8894)
```

`BaseTexture`/`UserTextures` are read **only in the "custom model" mode** (Link empty, MaterialId = -1,
`Model` id found in the model table). In the link/id branch (0x1404c4828…0x1404c5502) the UserInst is
touched only at +0x40, +0x3c/+0x30, +0x148, +0x149, +0x14c/+0x150/+0x15c, +0x1d0 — never +0x50/+0x60/+0x68.
And for a map item the pre-pass (§1a) never reaches 0x1404c4190 at all. The other consumer of
`UserTextures` is 0x1404fdbb0 (callers 0x140439b9c, 0x1405275af, 0x140ab1322): a dependency collector
that appends `<Texture>.dds` fids (class 0x09025000 lookup at 0x1404fdca6) to a list for embedding/preload —
it does not affect rendering.

So an embedded item cannot supply a diffuse texture for an `Editors\…` material in BlueBay: with a Link
the textures are ignored (and the writer even drops them, §0); without a Link the only other options are a
`Model`+`BaseTexture` custom material — which needs the `Model` id to exist in the model table handed to
0x1404c5510 (built from `crystal+0x58` via 0x140523450; whether TM2020 still populates it was not
established here) — or a `MaterialId`, which BlueBay resolves only under `BlueBay\Media\Material\`
(/tmp/mat-re/REPORT.md §2). In every case the folder gate applies, so a texture path could only be
`Editors\…`/`Effects\…`/`Test\…`-relative anyway.

## 3. Flat colour vs world-space checker

What the code establishes:

* **At 0x1409069b0 nothing debug-specific happens** (§1b): it is an unshared load of the same
  `Editors\MeshEditorMedia\Materials\<PhysicsName>.Material.Gbx` the cached loader would load. Whatever
  pattern the mesh editor shows for a given physics id is defined inside that material/shader in
  Maniaplanet.pak; the exe only (a) overrides constants by name, (b) binds the GameplayId texture to
  `*BaseColor_Over*` samplers. The material files are addressed by the exact path
  `Editors\MeshEditorMedia\Materials\` + `PhysicsName` (the EPlugSurfaceMaterialId enum name:
  Concrete, Pavement, Grass, Ice, Metal, Sand, Dirt, …, table 0x141ea78e0) + `.Material.gbx`
  (0x141bb8d80), so the set of files is exactly one per physics id, and the exe never creates a
  checker/grid shader for them.
* **In game (item path) the object rendered is `g_EditorMaterials[PhysicId]`**, a bare `CPlugMaterial`
  with no shader variants, no textures, no constants — the .Material.Gbx from the pak is not even
  opened. Its appearance can therefore only be a function of the PhysicId (and GameplayId) byte and of
  the renderer's handling of a shader-less material (`+0x88` material-data block initialised to
  shader id `-3` / texture indices `-1` at 0x14040d3e0 and never filled because `+0x40 == 0` at
  0x14040d7e4). Which physics ids the renderer paints as a flat colour and which as a world-space
  checker is decided in that fallback, which I did not locate in this pass (the engine-wide resources
  loaded by 0x1403dfba0 include `BitmapChecker` @[0x141fa9108]+0x260, `BitmapInvisible` +0x270,
  `MwTexture_MeshDefault` +0x370, `MwTexture_HotGrid` +0x368, `MwTexture_NoBitmap` +0x378,
  `Material_ShowInvalid` +0x218, `ShaderFillColor_ShowInvalid` +0x208; the checker/grid textures used
  for the fallback are presumably among these — not traced).

Practical consequence: the item author controls exactly one bit of appearance per Editors material —
`SurfacePhysicId` — plus the gameplay overlay via `SurfaceGameplayId`; the Link tail, `Color`, textures,
tiling and the rest of chunk 0x090FD000/0x090FD001 have no effect in a BlueBay map.

## Addresses index

| what | address |
|---|---|
| CPlugMaterialUserInst member table | 0x141ea8700–0x141ea9110 |
| ArchiveChunk dispatcher / 0x090FD000 body | 0x1404fced0 / 0x1404fd16f–0x1404fd6d1 |
| Csts count reader (≤8) | 0x1404fde40 |
| `"Real"` id string / 1/255 constant | 0x141b598b8 / 0x141d1edd0 |
| LoadPlugCrystalMaterials, Editors pre-pass | 0x140f4b600, 0x140f4b6a6–0x140f4b7ad |
| already-resolved skip in full path | 0x140f4ba55 → 0x140f4c011 |
| g_EditorMaterials table / count / builder | 0x142021eb0 / 0x142021eb8 / 0x14040f4d0–0x14040f576 |
| table index lookup / u16 archive | 0x1404e1770 / 0x1404deb00 |
| UserInst from table material (Link spelling) | 0x140f4d140–0x140f4d247, "Editors\MeshEditorMedia\Materials\" 0x141c99388 |
| PhysicsName table / GameplayName table | 0x141ea78e0 / 0x141eb2f60 |
| resolver (link vs custom-model branch) | 0x1404c4190; branch at 0x1404c4214/0x1404c422f; link body 0x1404c4c51 |
| loader choice (Csts>0 or Editors → unique) | 0x1404c4de4–0x1404c4eb7 |
| unique loader / cached loader | 0x1409069b0 / 0x1409009a0 |
| Csts → material constants by name | 0x1404c5035–0x1404c5259 (0x140443210 set, 0x140443380 commit) |
| GameplayId overlay texture binding | 0x1404c5268–0x1404c545f ("BaseColor_Over" 0x141bb8e28, ".Texture.Gbx" 0x141bb8e18, folder 0x141bb8df0) |
| UserTextures dependency collector | 0x1404fdbb0 |
| CGameEditorMesh MaterialBaseColors/LastUsed/CurrentColor records | 0x141f33fc8 (+0x9d4) / 0x141f33f60 (+0x998) / 0x141f34030 |
| mesh-editor colour swatch palettes | 0x141aec770 (14×9 vec3), lookup 0x1412430d0 |
| engine resource loader (BitmapChecker etc.) | 0x1403dfba0–0x1403e271a |
