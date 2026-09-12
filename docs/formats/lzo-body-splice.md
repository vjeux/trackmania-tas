# Editing a compressed GBX body in place: the LZO1X splice

How `tmmaps` writes an edited `.Map.Gbx` without recompressing it. Source:
`tools/tmmaps/src/splice.rs`; measurements in `tools/tmmaps/MAPS.md` §1a.
Confidence **[FILE]** for the stream grammar (checked against liblzo2 on 285
maps), **[VERIFIED-GAME]** for what the client and server accept.

## Why

The game ships maps compressed by an LZO variant stronger than
`lzo1x_1_compress`; re-emitting 173691 produces a body 29 763 bytes longer
sharing nothing after the header with the download. The dedicated server accepts
a re-emitted map, and — measured 2026-08-23 with four `EditMap` loads in one
session — so does the game client (`MAPS.md` §6). The splice is not what makes an
edited map loadable; it is what keeps it **attributable**: a file that differs
from one the client already loads only in the bytes of the edit cannot be
suspected of anything a rebuild might have done.

## The LZO1X stream as the decoder reads it (ported from `lzo1x_d.ch`, LZO 2.10)

```text
first byte > 17           a starting literal run of (b - 17) bytes
t < 16   (top of loop)    a literal run of t + 3 (t == 0: long form)
t < 16   (after literals) a 3-byte match
t in 16..31               a long-distance match; distance 0 is END OF STREAM
t in 32..63               a match, length in t & 31 (long form when 0)
t >= 64                   a short match, length (t >> 5) + 1
…and the low 2 bits of the last distance byte are the number of literals
that follow the match with no opcode of their own.
```

The furthest back any LZO1X match can reach is `0xBFFF` bytes.

## The four write methods

| method | the output file is | when |
|---|---|---|
| **literal** | the stock file byte for byte, edited bytes overwritten INSIDE the compressed stream; same length | every edited byte is a literal in the stock stream and no later match copies from one |
| **middle** | the stock stream either side of one short recompressed stretch | an edited byte sits inside a match |
| **tail** | the stock stream to a cut, then a recompressed tail | no instruction boundary far enough past the edit (in practice an edit at the very end) |
| **re-emit** | the whole body recompressed | ONLY when the body's LENGTH changed — a rename, an item-model swap, a chunk that grew |

Resuming the stock stream after a spliced stretch is sound because a match
names a DISTANCE, not an address: the output either side sits at its original
offsets. Two conditions are enforced, not assumed: the resumed opcode must be a
MATCH (the one opcode class that reads the same from a literal run or from the
top of the loop), and no later match may reach back into the edited bytes —
guaranteed by a resume point more than `0xBFFF` bytes past the edit, tried when
the near ones do not verify.

**Every write is verified before it is a file**: the produced stream is
decompressed with liblzo2 and required to equal the intended body over its
whole length. `splice.scan_agrees` also requires the module's own stream walk
(which decides which bytes are literals) to reconstruct the body identically to
liblzo2.

Measured: one cell byte moved carries 97–99.99 % of the stock stream verbatim;
a no-edit write is byte-identical to the input (`splice.no_edit_is_byte_identical`);
a recompressed body shares 0 bytes with the stock stream
(`splice.reemit_shares_nothing`, the negative control).

## The refusal

A body whose length changed with NO rename in play is a writer bug, and the
writer refuses. That refusal exposed a real defect: `route_170035_roseshaft`'s
body came back 1010 bytes longer with no edit asked for — the Id-table
re-encoder did not reproduce a map with a 268-entry lookback table
(`MAPS.md` §1a).

## Lessons

* LZO recompression is not bit-reproducible; never compare compressed bytes,
  compare decompressed bodies (`tmmaps roundtrip`, `tmmaps bodydiff`).
* A `'C'` body can be written with no compressor at all: a literal-only stream
  is legal (memory `gbx-map-surgery-lookback-tables.md`).
* Ghost writers in this tree write `'U'` bodies on purpose so every control can
  be a byte comparison (`tools/gbx/src/container.rs::write_gbx`); maps cannot,
  because the dedicated server refuses a `'U'` map.
