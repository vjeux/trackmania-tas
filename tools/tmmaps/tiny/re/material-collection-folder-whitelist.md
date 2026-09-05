# Trackmania.exe — BlueBay "unknown collection folder" static RE

Binary: /tmp/crash/Trackmania.exe, ImageBase 0x140000000. Scratch + tool (`pex`, Rust) in /tmp/mat-re/.
Note: .pdata / unwind info is scrambled by the protector; function bounds below were taken from int3 padding.

## 1. Where the error is emitted and how the "known collection folders" set is built

* `"Material: unknown collection folder %s"` — .rdata 0x141c992a0. Single xref:
  `lea rdx,[rip+..]` at **0x140f4bf49**, inside **NGameItemUtils::LoadPlugCrystalMaterials**
  (function 0x140f4b600–0x140f4c6c8; name string 0x141c991e8 used as profiling scope at 0x140f4b639).
  Args: rcx = CPlugCrystal (materials array +0x48, count +0x50, stride 0x20; material at [entry+0x00],
  resolved CPlugMaterial cached at [entry+0x18]), edx = collection id (CMwId), r9 = error logger (r15).

* CPlugMaterialUserInst layout used here: `Link` CFastString at +0x30 (heap flag +0x3b, length +0x3c),
  `MaterialId` (CMwId) at +0x40, PhysicsId/GameplayId bytes at +0x148/+0x149, u32 at +0x14c.

* The list is built once per call at the top of the function:
  ```
  140f4b810  lea rcx,[rbp-0x10]      ; SArray<CFastString> folders
  140f4b814  call 0x140103b60        ; array init
  140f4b819  lea rdx,[rbp-0x10]
  140f4b81d  mov ecx,ebx             ; collection id
  140f4b81f  call 0x140ae6b40        ; <-- GetMaterialCollectionFolders(collection, &folders)
  ```

* The gate, per material with non-empty Link (0x140f4be41–0x140f4bf89):
  ```
  140f4be96  call 0x14010db20        ; tmp = Link (copy)
  140f4be9b  cmp dword [rbp+0x10c],9 ; len==9 &&
  140f4bea2  lea rax,[0x141bb8e58]   ;   tmp == "ERROR_MAT"  -> accepted, skip gate
  140f4bef1  mov dl,0x5c
  140f4befa  call 0x14010ec50        ; tmp.TruncateAtFirst('\\')  (FindChar + SetLength)
  140f4bf30  call 0x14087bb40        ; IndexOf(folders, tmp): exact length+content match, else -1
  140f4bf35  cmp eax,-1
  140f4bf38  jne 0x140f4bf85         ; found -> go resolve the link (0x140f4a8d0)
  140f4bf49  lea rdx,[0x141c992a0]   ; "Material: unknown collection folder %s"
  140f4bf57  call 0x14011a810        ; logger->Printf  (this is the UGCErrorsLog line)
  ```
  i.e. `Link.Split('\\')[0] ∈ folders(collection)`; nothing else is consulted.

* **0x140ae6b40 (0x140ae6b40–0x140ae7014) builds the set — it is a hard-coded, compiled-in table**:
  ```
  140ae6b6a  lea rax,[0x141bbc428]   "Test"      -> push
  140ae6b9a  lea rax,[0x141bb8de8]   "Editors"   -> push
  140ae6bbb  lea rax,[0x141b5dde8]   "Effects"   -> push
  ; then: cmp ebx (collection id) against g_CollectionIds[] entries and add per family:
  140ae6bd2..6bf9  Canyon | Canyon4 | Canyon256   -> je 0x140ae6f51: + "Canyon"(0x141bcebd4), name(Canyon256), name(Canyon4)
  140ae6bff..6c26  Lagoon | Lagoon4 | Lagoon256   -> je 0x140ae6eb1: + "Lagoon"(0x141bcec4c), name(Lagoon256), name(Lagoon4)
  140ae6c2c        == [0x141ec73a8] (runtime alias id) -> 0x140ae6e11: + "Meteor"(0x141bcec54), name, name
  140ae6c38..6c5f  Stadium | Stadium4 | Stadium256 -> je 0x140ae6d71: + "Stadium"(0x141b67db0), name(Stadium256), name(Stadium4)
  140ae6c65        == StadiumMP4                  -> + "StadiumMP4"(0x141c299a0)
  140ae6c87        == Storm                       -> + "Storm"(0x141c29974)
  140ae6ca9..6cc8  Valley | Valley4 | Valley256   -> + "Valley"(0x141bcec44), name(Valley256), name(Valley4)
  140ae6cc8  jne 0x140ae7000                      ; anything else: return with only Test/Editors/Effects
  ```
  The `g_CollectionIds` int array is at **0x141e71130** (33 entries; the pointers 0x141ecc228 etc. in the
  code point into it). Index → name table at 0x141ecf7c0 (33 × {char*,len}):
  0 Canyon, 1 Canyon4, 2 Canyon256, 3 Valley, 4 Valley4, 5 Valley256, 6 Lagoon, 7 Lagoon4, 8 Lagoon256,
  9 Stadium, 10 Stadium4, 11 Stadium256, 12 Storm, 13 TMCommon, 14 SMCommon, 15 Vehicles, 16 Common,
  17 GreenCoast, 18 RedIsland, **19 BlueBay**, 20 WhiteShore, 21 Desert, 22 Snow, 23 Rally, 24 Island,
  25 Bay, 26 Coast, 27 StadiumMP4, 28 Deprecated_Arena, 29–31 QMTest1–3, 32 _Unassigned.
  Raw MwId values assigned at init 0x140ae6380 (via 0x140ae6310): Canyon=0x0c, Valley=0x0b, Lagoon=0x0d,
  Stadium=0x1a, Stadium4=0x18, Stadium256=0x19, Storm=0xca, GreenCoast=0x0f, RedIsland=0x10, **BlueBay=0x1c**,
  WhiteShore=0x1d, TMCommon=0x11, SMCommon=0x12b, Vehicles=0x2710("Vehicles"), Common=0x2713, ...

  **BlueBay is a known collection (idx 19) but has no branch in 0x140ae6b40**, so its folder set is exactly
  {"Test","Editors","Effects"}. Hence both `Stadium\...` and `BlueBay\...` are rejected, while in a Stadium
  map the set is {"Test","Editors","Effects","Stadium","Stadium256","Stadium4"}. Nothing is read from a pak,
  the title, or the collection Gbx.

## 2. Is there a link syntax that reaches a Stadium material from a BlueBay map? — No.

After the gate the Link is resolved by 0x140f4a8d0 → 0x1404c5510 → **0x1404c4190** (link branch at
0x1404c4c51):
* `Link == "ERROR_MAT"` → returns the global error material ([0x141fa9108]+0x498) (0x1404c4c69–0x1404c4cb1).
* otherwise: root folder(s) from the fid manager ([0x141fbbee0], 0x1408f1750 at 0x1404c4cbd), copy Link,
  if no extension (0x1408fcc00) append `".Material.gbx"` (0x141bb8d80), then
  `0x1404bfee0(roots, "", link, ext)` → 0x1408fa730 → for each root: 0x1408fa390(root, path, 1, …).
  Not found → `"Material not found : "` (0x141bb8d90, 0x1404c4ed2).
* Path walking (0x1408fa560 / tokenizer 0x1408fec00 / component lookup 0x1408fa100):
  * separator is `'\\'` only; leading `"\\\\"` skipped, repeated backslashes collapsed; `'/'` is not a separator.
  * component `"..\\"` (0x141c0660c) → **returns NULL, lookup fails** (0x1408fa131–0x1408fa164).
    `".\\"` (0x141c06610) → same folder. So `Editors\..\Stadium\...` cannot work.
  * the `:resource:\`, `:shared:\`, `:user:\`, `:data:\`, `:temp:\` root prefixes (table 0x141ebdb20, parsed by
    0x1408f9090) are only honoured when the start folder is NULL (0x1408fa390 rcx==0). The material lookup
    always passes explicit root folders, so no prefix syntax is available on this path.
* The gate itself compares the raw first path component by exact length + content (0x14087bb40), so there is
  no alternate spelling. `Editors`/`Effects` are accepted purely because they are in the fixed base list;
  after the gate they are ordinary GameData-relative paths like `Stadium\...` would be. The only
  "Editors"-specific code is: 0x140f4a8f6–0x140f4a95c swaps the (primary, fallback) path pair, and
  0x1404c4de4–0x1404c4e34 picks loader 0x1409069b0 (Editors link or `[+0x14c] > 0`) vs 0x1409009a0.
* MaterialId path (Link empty, `MaterialId != -1`, 0x140f4bb6a → 0x140f4bb82): `name(MaterialId)+".Material.Gbx"`
  is searched (0x1408fa390 at 0x140f4bcc3) in folders from 0x140ae7420(collection): for each id of the
  collection's family (0x140ae68e0) `"<Name>\Media\Material_BlockCustom\"` and `"<Name>\Media\Material\"`
  (built by 0x140ae61e0 with 0x141c299e8 / 0x141c29a08), plus `"Effects\Media\Material\"` (0x141c29ad8 at
  0x140ae74d7) and 0x140ae7340 extras. For BlueBay the family is just BlueBay → `BlueBay\Media\Material\<id>`.
  Failure logs `"Material: could not find id %s"` (0x141c99280 at 0x140f4bcf2). No Stadium reach either.
* `IsUsingGameMaterial`, `Collection`, `LinkFull` do not enter the gate. Bytes +0x148/+0x149
  (PhysicsId/GameplayId) are only copied onto the loaded material (0x1404c4e5e–0x1404c4eab; 0x140f4b729–0x140f4b76e).

Conclusion: in a BlueBay map the only material links that pass are `Test\…`, `Editors\…`, `Effects\…` and the
literal `ERROR_MAT`. Reaching `Stadium\Media\Material\*` requires patching 0x140ae6b40 (add a BlueBay branch
pushing "Stadium") or the compare at 0x140f4bf35.

## 3. DisableLightmap

* `"DisableLightmap"` (0x141c207d0) is referenced once, at **0x140058863**, inside the static initialiser
  (0x1400577c0…) that fills CGameItemModel's declarative chunk/member table in .data (0x141ec2660…0x141ec3340).
  The record written at 0x140058863–0x1400588b0:
  ```
  [0x141ec32dc] = 0x2E00202A        ; chunk id
  [0x141ec32e8] = 0x278             ; member offset in CGameItemModel
  [0x141ec32f0] = [0x141ec32f8] = "DisableLightmap"
  [0x141ec3300] = 3, [0x141ec3304] = 0x380, [0x141ec3308] = 0 (fn slot, r10 == 0)
  [0x141ec32d4] = 1                 ; type code (1 = Bool; cf. 6 = float for the records below)
  ```
  Neighbouring records from the same table, all single-value "auto" chunks:
  0x2E002024 GroundPoint Vec3 @+0x1b8 (type 0x14), 0x2E002025 PainterGroundMargin @+0x1d4 (6),
  0x2E002026 OrbitalCenterHeightFromGround @+0x1c4 (6), 0x2E002027 OrbitalRadiusBase @+0x1c8 (6),
  0x2E002028 OrbitalPreviewAngle @+0x1cc (6), 0x2E002029 IconMacroBlockInfo noderef @+0x2a8 (0x1f),
  0x2E00202B DisableAutoCreateSound (offset -1, accessor-based).
* So: **chunk 0x2E00202A, body = one Gbx bool (u32) = CGameItemModel.DisableLightmap (+0x278)**, same
  encoding as the known single-float chunks 0x2E002025–0x2E002028.
* No other per-item-model lightmap switch/allocation field exists in that table. Per-placed-item lightmap
  quality lives on the map side (`MapElemLightmapQuality` / `LightmapQuality` on the anchored object), not in
  the item's 0x2E002xxx chunks.
