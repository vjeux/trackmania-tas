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

## 4. The "dummy write" — the cipher is perturbed from INSIDE the GBX parse **[EXE]**

While the game reads an ENCRYPTED, UNCOMPRESSED pak entry it arms the
Blowfish IV perturbation at the start of every NODE body (the main node after
the header; each inline node after its class id) with the four LE bytes of
that node's PARENT class id, folded as `iv_xor ← rotl64(iv_xor, 13) ^ (byte |
0xAA)` per byte; a `CPlugVehiclePhyTuning`-derived node arms again with the
first four ASCII bytes of its name; `CPlugSurfaceGeom` chunk `0x0900F004` with
`BitConverter.GetBytes(box.X − box.X2)`. The accumulated fold lands at the
next `0x100` boundary of the COMPRESSED read and chains through every later
block; a read spanning no boundary carries its armings forward. Exempt: LZ4
entries and entries with flag bit 32 (`DontUseDummyWrite`). The parent table
is GBX.NET's `inherits:` lines (251 classes) with the engine fallback (a
`0x09xxxxxx` Plug class → `CPlug 0x0902B000`, else `CMwNod 0x01001000`;
`GxLightSpot → GxLightBall 0x04002000`); the fold exists only for a class WITH
a declared parent (a blanket fallback garbled the DynaObject files). This is
how all 73 Stadium `.Light.Gbx` (275–485 B) read garbage past byte 0x100
until 2026-09-07, and how the Sport/Snow/Rally/Desert tuning entries
(`0x090EC000`) decode to their exact declared sizes with every boundary
PREDICTED (test vectors: Snow `CarSnow\E100BAE2` `0x700 →
0x5B0525E6FA585C38` after 12 armings, `0xD00 → 0x00005502E81540AB`).
Compressed dummy-written files (flags 0x5, no bit 50: every `VegetTreeModel`,
48 in Stadium) fold the parent class id of every node body starting inside
plain chunk k into the cipher at the compressed offset where chunk k+1 begins;
`mapgeom pak-foldhunt` accepts a fold under which the chunk decodes to exactly
4096 plain bytes.

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
  (`bad match offset 49599`), one LZ4 case.
* The pack keys of `Skins_Stadium`, `Titles`, `Resource`, `Title.Pack`
  (a title-pack key seen in lab evidence: `a6a873d36485d18b99454be644dcf0ac`).
