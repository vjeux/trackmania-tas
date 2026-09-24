# NadeoPak (`.pak`) — the game's data archives

What we open: `dedicated_TMStadium.pak` (the dedicated server's collision-only
pack, key `870FBE770EE4909C714B18B04D914C17`), the client's
`Stadium.pak` (1 754 026 248 B, key `B773D73047A4104857722366D78D28A6`),
`BlueBay.pak`/`RedIsland.pak`/`WhiteShore.pak`/`GreenCoast.pak` (shared key
`660C4C156B80337E296A1034B0AA05B8`), `Maniaplanet.pak` / `Maniaplanet_Core.pak`
/ `Maniaplanet_ModelsSport.pak` (key `9A93723447347A8CE336CCFC49E65449`; the
physics packs). Reader: `tools/mapgeom/src/{store,pak,pakfile,blowfish,
lz4dict,md5,names,parents,keyhunt}.rs`; the HPL lab's `pak_extract` /
`paksumverify` (memories `tm-hpl-pack-crypto.md`,
`tm-hpl-pack-cipher-SOLVED.md`, `nadeo-pak-container-digests.md`). Confidence
**[FILE]**/**[EXE]**; every claim below decoded real entries to their declared
size. Note: `tools/pkz2` is NOT a pak tool (it is the 153527 map's analysis
crate; "pkz" is that map's nickname) — the pak code is all under `mapgeom`.

## 1. File layout (v18 on the current packs; the readers handle v6+)

```text
0x00  "NadeoPak"
0x08  i32 version
0x0C  u8[32] contentChecksum        = SHA-256(pack[44..EOF])   — over the whole file, in the clear
0x2C  u32 headerFlags               (7 on the retail packs; 0 = header entirely in the clear)
0x30  u32 headerMaxSize             (v≥15; a fixed 0x300000 arena; slack past the table is in no digest)
      v≥7: u32 authorVersion, 4 strings (author…), string manialinkURL, [v≥13 string downloadURL],
           u64 creationDate, string comments, [v≥12 string XML, string titleId], string usageSubdir,
           string creationBuildInfo, 16 bytes, [v≥10 includedPacks[] { 32 B, string, u32, 5 strings, 8 B,
           string, [v≥11 u32] }]
      → the ENCRYPTED private header starts here (`store.rs::pak_encrypted_header_start`)
```

Private header (Blowfish-CBC with the HEADER key = `key XOR
56EECBBBDEB6BC90A17DFCEB761D59CE`, IV = the first 8 bytes at the start;
`pak.rs::read_pak`):

```text
u8[16] headerMD5, u32 gbxHeadersStart, [v<15 i32], i32 gbxHeadersSize, i32 gbxHeadersComprSize,
[v≥14: 16 B unused (= MD5(pack[gbxHeadersStart..EOF]) — not unused at all), [v≥16 u32 size]], 16 B, u32 flags,
i32 nFolders, nFolders × { i32 parent, string name }   ← after folder 2 its UTF-16 name perturbs the cipher
i32 nFiles, nFiles × { i32 folderIndex, string name, i32, i32 uncompressedSize, i32 compressedSize, u32 offset,
                       u32 classId, [v≥17 i32 sizeField], [v≥14 u128 checksum], u64 flags }
```

Entry flags: `& 0x3C` compressed; bit 32 (`0x1_0000_0000`) `DontUseDummyWrite`;
`0x2000000000000` public file; `0x4000000000000` `ForceNoCrypt`; encrypted ⇔
neither. Nearly everything carries ForceNoCrypt; the decoration `.Map.Gbx`
files are ENCRYPTED. Entry framing in the data region: `[u16 compressedLen][LZ4
block]`, each block emitting exactly 4096 bytes except the last; a stored slot
is 64-aligned with an 8-byte IV prefix + payload + zero padding.

## 2. Keys and ciphers

* **Pack key = `MD5(<32-char UPPER-hex base> ++ "NadeoPak")`**; the 32-char
  bases are PLAIN ASCII STRINGS IN THE EXE (`strings -n 32 Trackmania.exe |
  grep -E '^[0-9A-F]{32}$'` → 18 bases): BlueBay/RedIsland/WhiteShore/GreenCoast
  share base `105C22EB2CFAC821DF2282E634B453BD`, Stadium
  `5B985F4CC43013C458A4C99A4B285E1E`. `mapgeom pak-keyhunt DUMP PAK` recovers
  a key from a full memory dump of the running game instead (the pack's SHA-256
  anchors search windows; every 16-byte window nearby is tried as the key and
  as the header-XORed form against the private header's first fields).
* **Blowfish constants are just π**: the 18 P + 4×256 S words are the first
  1042 32-bit words of π's fractional part (P[0] = `0x243f6a88`, S4[255] =
  `0x3ac372e6`); generate them, never hunt for the C# file.
* **The LZ4 preset dictionary is 1006 bytes**, sha256 `49091f00…f18d2`,
  byte-identical in every 2023–2026 exe (`tools/mapgeom/src/lz4dict.rs`;
  found by CONSENSUS of matches reaching before the output start).
* The gbx-headers blob decodes with the header key, then the LZ4 block
  stream with the dictionary.
* **Entries with flag `0x40` use a COUNTER-mode Blowfish, not CBC** **[DISASSEMBLY
  0x14055a590 → cipher kind 4, vtable 0x141bc4ac8; opener 0x1413b0a10, reader
  0x1413b1090 through the 8-round wrapper 0x1413b1cd0]**: the same key as every
  other entry and the same 8-round schedule (`InitBlowfish` 0x140128240 with
  `P[i] ^= key_le_word[i & 3]`, i < 10), but mode 2 — the P array is NOT
  reversed as the CBC kinds do — and no chaining: the entry begins with an
  8-byte IV; for body byte offset `o`, batch `b = o/256`, block `k = (o%256)/8`,
  the keystream is the little-endian block encryption of the little-endian
  counter `IV + 8 + 256·b + k` and `plain = cipher ^ keystream`. The dummy-write
  fold is never applied (random-access reader). `blowfish::counter_decrypt`;
  the only encrypted `0x40` entries in the six packs are the five
  `CPlugMoodBlender` XMLs (`GameCtnDecoration\\<hash>`, 0x0911A000) — the
  Maniaplanet shader/material `0x…041` entries carry bit 50 (ForceNoCrypt) and
  were always plain.

## 3. Hashed file names (`names.rs`) **[FILE]** — every prefab in the pack resolves

`CPlugPrefab`, `CPlugSurface`, `CPlugSolid`, the `.Light.Gbx` and the gate
`*_Trigger.Shape.Gbx` files live under 34 hex characters
(`Stadium\Media\Prefab\42F328BF947AC905A1D4FECB9A40E4C6F7`). The hash is
`MD5.Compute136` with two details that each look like a wrong answer:

```text
h[0]    = the BYTE LENGTH of the lowercased UTF-8 path          (not 0x00)
h[1..]  = md5 of those bytes
hex     = each byte written LOW NIBBLE FIRST                     (not normal hex)
```

What is hashed is a SUFFIX of the path and the entry lives under the remaining
prefix: `A\B\C\name` may be stored as `A\B\C\<h(name)>`, `A\B\<h(C\name)>`,
`A\<h(B\C\name)>` or `<h(A\B\C\name)>` — resolution walks the four split
points. Where the names come from: a block model's GBX REFERENCE TABLE names
its prefabs in plain text (folder index 1-based, 0 = the ancestor directory).

## 4. The "dummy write" — the cipher is perturbed from INSIDE the GBX parse **[DISASSEMBLY]**

Read off `Trackmania.exe` (Aug 2025 client, md5 `4a28c00429c6f75c894cf7bc4378a8a2`)
on 2026-09-23 with `tools/asmdig` over an `objdump` listing; the addresses are
that build's. Until then the rule was GBX.NET's guess (its `inherits:` lines),
which failed on every class the C# corpus does not carry — the decoration
`Scene3d` among them.

### 4.1 Who folds, and what

`CMwNod::Archive` (`0x1402d0720`, vtable slot 14 of every node class — the
chunk loop that reads or writes a node body) starts, in BOTH directions, with:

```text
info = this->GetClassInfo()                       // vtable slot 2
if (info->parent != NULL) {                       // CMwClassInfo +0x20; only CMwNod and the primitive types have none
    archive->buffer->dummy = 1                    // the TOP buffer's +0x10 flag
    switch (info->classId) {                      // +0x18, the ENGINE id
        0x0A003000 (CSceneLayout)              → v = 0x0A001000 (CScene)
        0x090BF000                             → v = 0x0804B000
        0x0917E000, 0x09184000                 → v = 0x05010000
        0x09185000                             → v = 0x05002000
        default                                → v = REMAP(info->parent->classId)     // 0x1402f3570
                                                 if (v == 0x07031000) v = 0x07001000   // a CControlText child folds CControlBase
    }
    if (v == 0) v = 0xFFFFFFFF                    // 0x1402d1d00 (never taken)
    archive->Write4(&v)                           // 0x14012bbe0 → the buffer chain's Write, in dummy mode
    archive->buffer->dummy = 0
}
```

* `REMAP` (`0x1402f3570`, 161 entries) is the id the engine WRITES for a class
  — the CGame `0x03xxxxxx` ids files carry as `0x24xxxxxx`
  (`0x03043000 → 0x24003000`, `0x0304E000 → 0x24005000`, …); identity for
  everything else. Its read-side inverse plus the legacy aliases
  (`0x0301A000 → 0x2E001000`, the `0x0805xxxx → 0x090Bxxxx` particle classes)
  is `0x1402f2610` (191 entries), applied to every class id read from a file
  before the class registry lookup (`0x1402f20a0`). Both tables and the whole
  hierarchy (1905 classes, from the two `CMwClassInfo::Register` spellings at
  `0x1402d52e0` / `0x1402ea9e0`) are generated into
  `tools/mapgeom/src/engine_classes.rs` by `asmdig classtree` / `cmptree`.
* The buffer chain: the archive's top buffer is the LZ4 decoder for a
  compressed entry (ctor `0x1413ac4c0`, read core `0x1413ac620`, write
  `0x1413ac900`), the Blowfish buffer for a raw one (`0x1413b0430`; VT_A
  `0x141bc4a18` = 8 rounds for pak v18, read `0x1413b0b90`, write
  `0x1413b0e00`). A Write in dummy mode passes straight through the LZ4 buffer
  (`0x1413ac934`: it sets the underlying's flag and forwards) and, in the
  crypt buffer, folds instead of writing: per byte `iv_xor = rotl64(iv_xor, 13)
  ^ (b | 0xAA)` (`0x1413b0e72..0e8f`). `+0x18` of the crypt buffer = pak entry
  flag bit 32 (`DontUseDummyWrite`, set in `CreateFileBuffer` `0x14055a590`)
  turns the fold into a no-op. A memory buffer's dummy Write is a no-op
  (`0x140123c80`).
* The fold lands when the crypt buffer next refills its 0x100-byte batch
  (`0x1413b0c02`: `iv ^= iv_xor; iv_xor = 0` before decrypting the 32 blocks);
  the writer applies it after encrypting a batch (`0x1413b0ff3`), which is the
  same boundary. Block chaining: `plain = BF(cipher) ^ iv; iv = (iv >> 47) ^
  9·iv ^ cipher`.

### 4.2 Which nodes fold — and which do not

* Every node body the chunk loop reads: the main node (its `Archive` is
  called from `LoadGbx_Body` `0x1409031d0` via `0x140900560`) and every inline
  node `ArchiveNodRef` (`0x140905cb0`) creates: index not yet in the node
  table → read class id → `CreateByMwClassId` (`0x1402cf380`) → `Archive`.
  A `-1` reference, a back-reference and an external (reference-table) node
  fold nothing.
* NOT a class whose own `Archive` never reaches `CMwNod::Archive` — a
  plain-struct body: `CPlugVegetTreeModel`, `CPlugDynaObjectModel`,
  `CPlugPrefab`, `CPlugStaticObjectModel`, the `NPlug*::S*` structs, the
  `CPlugFile*` loaders (92 of 1857 vtables; `engine_classes::NO_FOLD_CLASSES`,
  from `asmdig vtables`: slot 14 ≠ `0x1402d0720` and no call/jmp to it). The
  172 `.VegetTreeModel.Gbx` and 24 `.DynaObject.Gbx` entries of the five
  packs decode without a main-node fold and garble with one. The CPlugVisual
  family (`0x1404031e0`), CPlugBitmap and CPlugMaterial override `Archive`
  but tail-call the base before writing anything: they fold at the body start.
* NOT a node inside a SKIPPABLE chunk: the loop reads `PIKS` + size, pulls
  the whole chunk into a memory buffer, swaps it in as the archive's buffer
  (`0x1402d0a30..0x1402d0aa8`) and parses from there — the inline nodes'
  dummy writes hit the memory buffer and vanish.
* Three explicit per-class dummy writes exist besides the generic one:
  `CPlugVehiclePhyTuning` / `CPlugVehicleCarPhyTuning` fold the first four
  bytes of their name (`0x1405fbcca`, `0x1405da97d`) and `CPlugSurfaceGeom`
  chunk `0x0900F004` folds `f32(box.X − box.X2)` (`0x14051ed4e`). GBX.NET
  knew these three.

### 4.3 Where the fold lands in a compressed entry

The LZ4 read core pulls the next `[u16 len][block]` from the crypt buffer only
when a read finds its window empty (`0x1413ac686..0x1413ac6fa`): with `p`
plain bytes consumed, the crypt stream stands at the start of chunk
`ceil(p / 4096)` — chunk k+1 for a body that begins strictly inside chunk k,
chunk k for one that begins exactly on a 4096 boundary. The fold then applies
at the next 0x100 boundary of the compressed stream from there (`pakfile.rs::
fold_position`). Folds between two boundaries accumulate in read order.

### 4.4 What this unlocked

`BlueBay\GameCtnDecoration\Scene3d\Base64x64.Scene3d.Gbx` (CSceneLayout
`0x0A003000`, 75 325 B, 44 nodes, 19 LZ4 chunks) needed 43 folds; the one no
table had was the main node's — `0x0A003000` is a hard-coded special case
(folds `0x0A001000`), not derivable from any hierarchy. It now decodes whole,
as do the other collections' layouts; `mapgeom scene3d` exports the island /
sea / shadow-caster solids (`scene3d.rs`). Over the 734 encrypted
dummy-written entries of the five packs the engine table decodes 10 files
GBX.NET's guess could not (CPlugDecalModel, CGameCtnDecorationMood,
CGameItemPlacementParam, CPlugGameSkin(AndFolder) — all `CMwNod`-parented
classes GBX.NET's list lacked) and loses none. Test vectors (uncompressed):
Snow `CarSnow\E100BAE2` `0x700 → 0x5B0525E6FA585C38` after 12 armings,
`0xD00 → 0x00005502E81540AB`. `mapgeom pak-foldhunt` remains the fallback
for a class the table does not know (it accepts a fold under which a chunk
decodes to exactly 4096 plain bytes).
## 5. The four integrity fields a repacked pack must carry **[VERIFIED-GAME]**

A pack that gets ANY of these wrong is refused by the client (the process
EXITS after "Initializing meta…", no crash event):

| field | construction |
|---|---|
| per-entry 16-byte checksum | `MD5(pack[headerMax + offset .. + sizeField])` — the whole 64-aligned SLOT: IV prefix + payload + padding |
| `pack[12..44]` | `SHA-256(pack[44..EOF])` |
| private header `[28..44]` ("unused") | `MD5(pack[gbxHeadersStart..EOF])` |
| private header `[0..16]` (`HeaderMD5`) | `MD5(pack[52 .. encryptedOffset−8] ++ headerImage[0..tableEnd] with headerImage[0..16] ZEROED)` — the plaintext metadata block spliced onto the DECRYPTED table, which is why ~11 sweeps over the file missed it |

Enforced in the binary (`sep30.exe 0x14042a8f0` re-serialises the header into
an MD5 sink; reader at `0x14042a843` compares both 8-byte halves). Write order
when repacking: data + header fields (offsets, sizes, `md5(slot)`) →
`header[28..44]` → `header[0..16]` LAST → encrypt → `pack[12..44]`.

## 6. What we extract, and with what

* `mapgeom scene3d <Coll>\GameCtnDecoration\Scene3d\Base64x64.Scene3d.Gbx --out X.obj`:
  the decoration's island / sea / shadow-caster solids (CSceneLayout → CPlugSolid
  → CPlugTree → visuals; `scene3d.rs`), grouped by material name — the baker's
  `--decoration` input. Stadium256's layout holds only the sky dome.
* `mapgeom` (`store.rs`): block infos, prefabs, static objects, Solid2s,
  surfaces, materials, lights, FxSystems, particle models, veget models,
  decorations — by logical path through the hash resolver, with an OVERLAY
  for files read from a map's embedded zip. `mapgeom dump PATH`, `mapgeom
  model … --out X.obj`, `mapgeom refs FILE --deep`, `blockinfo`, `veget-info`,
  `fx-dump`, `tex-stats`, `skins`, `surf`, `prefab-ents`.
* The dedicated server's pack ships COLLISION, not appearance (`mesh = -1` on
  every road block); a rendering-grade model needs the client packs.
* Textures: ffmpeg reads the pak `.dds` directly; the `.Texture.gbx` is a
  363-byte wrapper (`textures-dds-skins.md`).
* Physics packs (`Maniaplanet_ModelsSport.pak` → `Vehicles\…` `CPlugVehiclePhyTuning`
  `0x090EC000`/`0x090ED000`): decoded by the HPL lab's grammar-driven parser
  (`tools/tune/grammar_inc.rs`, GBX.NET's 464 `.chunkl` → 1 678 chunk
  grammars); the three `Keys` archives are `{ i32 u02; i32 count; match u02
  {1 → u8, 2 → u32, 3 → u16}; Vec2[count] }`; chunk `0x090ED0A5` v1 → v2 drops
  nine `Keys` curves + six ints per tuning (the real Fall-2022 difference).

## 7. Archive-era traps

The public client archive (`archive.org/details/tm2020-archive`, 32 clients)
has a 252-day hole `2022-01-21 → 2022-09-30`; its labels are CAPTURE dates,
not build dates ("2024-02-26" ships post-February Snow data; "2024-04-30" is a
staged May-22 build); `Trackmania Chinese/Trackmania_Setup.exe` inside it
wraps a complete 2020-10-02 client. Pack FILES change at every boundary, so
file-level hashes localise nothing — diff INSIDE the pack (memory
`tm2020-old-client-archives.md`).

## 8. Not known

* The header-flag bits' individual meaning; `headerFlags 7` packs are
  encrypted, 0 in the clear.
* The remaining `.pak` cases the reader fails on: `LightRay.DynaObject.Gbx`
  (`bad match offset 49599`), one LZ4 case; two 250–440 KB VegetTreeModels
  (`BlueBay\Media\A21207AF…`, `RedIsland\Media\A2EBE59E…`) still need the fold hunt.
* The pack keys of `Skins_Stadium`, `Titles`, `Resource`, `Title.Pack`
  (a title-pack key seen in lab evidence: `a6a873d36485d18b99454be644dcf0ac`).
