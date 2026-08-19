# §9h — making a CUT run watchable: record-data sample surgery

Written 2026-08-18 by the container agent, on 286279 `[Turtle Trial] Leto` and
238835 `[Turtle Trial] Angustus`. Companion to §9g (which made the *uncut*
author ghost loadable). Everything below is measured through the plain oracle
with a known-answer control in every batch.

Tool: `rec` — `_container/tools/rec.rs`, a binary in the `mt` crate.

---

## The problem, and how bad it was

Cutting a trial tape edits the INPUT archive (`0x0309201D`) and nothing else.
The ghost's recorded samples (`CPlugEntRecordData`, `0x0911F000`) are left
alone, so a cut file re-simulates to the cut time and **shows the uncut run**.

It is worse than "shows too much". Every cut tape this project has published so
far was built by transplanting inputs into *rank 1's container*, so the replay
a human opens is **the world record holder's 441-second run**, complete with
his ten failed attempts — a different driver, a different time, a different
lap, in a file named for a 220-second one. Measured, on the banked tapes:

| file | validates | its record node says |
|---|---|---|
| `m286279_AUTHORCUT_220391_v6` | 220.391 | start 0 **end 441000**, 8783 samples = rank 1's run |
| `m286279_TAS_235625_v3` | 235.625 | the same 441.002 s samples |
| `m286279_HUMAN_keyboard_236972_v1` | 236.972 | the same |
| `238835/AUTHOR_246602` | 246.602 | rank 1's 1964.933 s samples |

## The result

| file | oracle | `IsValid` | shows |
|---|---|---|---|
| `AUTHORCUT_286279_220391.Ghost.Gbx` | **220.391 s** | true, 3 respawns | the author's own lap, nine failed attempts removed |
| `HUMANCUT_286279_236972.Ghost.Gbx` | **236.972 s** | true, 3 respawns | Bald_tm's own inputs, ten failed attempts removed |
| `AUTHORCUT_238835_246602.Ghost.Gbx` | **246.602 s** | true, 3 respawns | 238835's author, retries removed |

Three cold passes each, fresh server process per pass, controls in every batch
(441.002 / 977.690 / 355.181, and 1964.933 / 462.982). Clean-room rebuild from
banked source reproduced `AUTHORCUT_286279_220391` **byte-identically**
(md5 `9f02098c1ba581d8e57770ebbcffbf4a`).

```bash
rec align UNCUT.Ghost.Gbx CUT.Ghost.Gbx --minrun 500 [--nosteer]   # derive the drops
mt splice I.Ghost.Gbx UNCUT a:b UNCUT c:d ...                       # (or use the cut tape you have)
rec cut I.Ghost.Gbx C.Ghost.Gbx --drop A:B,C:D                      # delete + retime the samples
mt declare C.Ghost.Gbx D.Ghost.Gbx <time> <nresp> <cp1,cp2,...>
mt reencode D.Ghost.Gbx OUT.Ghost.Gbx                               # fixes the walltime field
rec verify UNCUT OUT --drop A:B,C:D                                 # every surviving sample identical?
```

---

## Why deleting samples is EXACT and not an approximation

Every cut this project makes is respawn-anchored, and §1.4 of 286279's
RESULT.md measured the governing property directly: *however long the car has
been failing since the checkpoint, and wherever it is when the respawn fires,
the run afterwards is bit-identical.* So the cut run's trajectory **is** the
uncut trajectory with the deleted intervals removed. The surviving samples are
the right samples, in the right states; only their timestamps move.

That is also why the oracle's arithmetic is the primary acceptance test:
`finish == uncut_finish − dropped` to the millisecond means the post-join
physics matched, which is the same statement.

## The format, and why a byte patch cannot do it

An entity's samples are stored as `numSamples | sampleSize | numSamples × i32
dt` (cumulative) followed by **columnar byte-delta coding**: for each byte index
of the sample, all `numSamples` bytes of that column consecutively, each the
wrapping difference from the previous sample's byte. Deleting one sample
therefore re-derives every column. The blob is zlib; the node lives inside
skippable chunk `0x03092000`, whose length word must move with it.

Sample times are explicit per-sample deltas, so surviving samples do **not**
have to stay on the 50 ms grid — a cut whose length is not a multiple of the
sample period is representable, and that is the normal case (286279's cut is
134 790 ms).

**Positive control, run before anything was believed:** parse + re-encode with
no edit must be BYTE-IDENTICAL. It is, on nine ghosts spanning 63 KB to 5.2 MB
of record data (`rec roundtrip`). Run it after any change to the encoder.

---

## Three defects this shook out, all silent

**1. `zlib` level 9 writes `78 DA`; every reader here keys on `78 9C`.**
The first cut file validated at 220.391 through the oracle and reported
`CPlugEntRecordData chunk not found` to our own decoder — the game accepts any
zlib header, `tmtraj::find_entrecord_blob` does not. `rec` compresses at level 6
for that reason alone. **A file that validates is not thereby readable.**

**2. Tick → millisecond is `start_offset_ms + 10·tick`, and the offset is not
zero.** 286279's author archive starts at **−1570 ms**, 238835's at −1510,
rank 1's at −1520. Assuming zero puts every drop ~1.5 s out of place. This one
nearly shipped: the misplaced windows still produced a file that validated at
220.391 (the drop *lengths* were right, so the arithmetic was right) and still
passed a checkpoint-position test at three of four splits.

What caught it was the *shape* of the joins. With the corrected offsets the
three post-join positions are

```
(939.20, 38.00, 656.00)      (848.0, 18.0, 939.2)      (715.20, 18.00, 976.00)
```

and the first and third are, to the centimetre, the canonical CP1 and CP3
standing-respawn transforms that RESULT.md §1.3 measured independently
(`CP1 (939.20, 38.00, 656.00)`, `CP3 (715.20, 18.00, 976.00)`). With the wrong
offsets they were not. **A cut is right when its joins land on known respawn
destinations** — that is the test to run, and it is sharper than the arithmetic
because the arithmetic is blind to a shift that preserves lengths.

**3. `p.steer` is the DECODED value and a "same as previous" packet inherits
it**, so after a splice the packet at a join decodes to a different steer from
the same packet in its source (measured: cut[13169] decodes −127 where
uncut[33557] decodes 0, because the join changed what "previous" means).
`rec align --nosteer` matches on the state word and pedals only.

---

## `rec align` recovers a cut spec from the two tapes

Nobody should have to trust a remembered tick number. `rec align` greedily
matches the cut tape into its source and prints the kept tick ranges and the
`--drop` spec. It refuses rather than guessing: if the kept count does not equal
the cut tape's packet count it prints `MISMATCH, do not trust`.

Sweep `--minrun` upward until the segmentation stops changing — a repeated
input pattern (a failed attempt looks like the next failed attempt) produces
spurious short matches at low values. On 286279 it converges at `--minrun 200`.

The converged answer on 286279's 236.972 s tape is
`WR[0..13169) ++ WR[33557..38212) ++ WR[38227..end)` and drops
`130170:334050, 380600:380750` — the same segments RESULT.md §2 records by hand,
and `334050` is exactly the number its prediction `130170 + (441002 − 334050) =
237122` was built on. Derived from the bytes, agreeing with the prose.

## The negative: a SEARCHED tape can never be made watchable this way

`m286279_TAS_235625_v3` and `m286279_KEYBOARD_235939_v4` are not cuts. A
mutation search changed steer values, so the tape is **not a subsequence of any
recorded run** and there are no samples anywhere that show what it does — the
trajectory it produces was never recorded by anything. `rec align` reports
`MISMATCH, do not trust` on both, which is correct behaviour, and no sample
surgery can fix it.

So on 286279 the watchable set is exactly the two *pure-input* results —
220.391 (the author's own driving) and 236.972 (Bald_tm's own) — and the
235.625 s floor stays validator-only. Producing samples for a searched tape
needs a simulator that emits telemetry, which is a different instrument
(the fork server's per-tick state readout is the candidate; not attempted here).

## A caveat on reading a cut file at a checkpoint

On a trial map the checkpoint crossing is typically 3–20 ms before the respawn
that follows it, so a split time can land inside the one sample interval that
spans a join. Interpolating there blends across the teleport and gives a
position several metres out — an artefact of interpolation, not of the file.
Every other split matches the uncut run's position exactly (verified at all
4/4, 4/4 and 5/5 splits of the three files; the three near-join splits differ by
2.3, 5.9 and 3.8 m). `rec verify` is the check that does not have this problem:
it compares every surviving sample byte-for-byte. All three files pass it —
8 764, 9 443 and 9 828 samples, times and bytes.

---

## 146612's 25 record nodes: there is no author lap in there

Asked for a rule for picking the right node out of a multi-node map. The rule,
and the answer for 146612:

1. **If the map carries a whole `CGameCtnGhost` blob** (`ct probe` shows
   `CGameGhost 00x` chunks and a `ghost inputs` id), use that — its record node
   is the author's validation lap and every other node in the body is
   something else. 286279 and 238835 are this case.
2. **Otherwise match by END TIME**, allowing a ~2.96 s countdown lead-in:
   `end ≈ AT` (228607: 20.290 vs 20.258; 228811: 20.550 vs 20.555) or
   `end ≈ AT + 2.96` (145875: 9.300 vs 6.343; 203330: 16.950 vs 13.995;
   285268: 52.250 vs 49.282). Those are the only two values observed.
3. **If nothing matches and the nodes start at DIFFERENT positions, they are not
   laps of the map.**

146612 fails 2 and 3. `rec nodes` lists 25 nodes that are 13 distinct
recordings, each of the first twelve appearing twice, at body offsets from
357 k to 2 053 k. They start at thirteen different places, in six clusters —
(335–336, 42, 815), (352, 42, 819), (624–633, 34, 1010), (866–890, 15–18, 787),
(620, 18, 617), (504–524, 15–18, 782–824) — several of them mid-map, and they
end nowhere in common. No node's end time
is the AT (38.530) or AT + 2.96 (41.490); the nearest is 40.730.

**So 146612 embeds no author-time lap**, and the `ATREC_146612.Ghost.Gbx`
banked under §9g — built from node 0 on the assumption that index 0 is the
author's — **has been withdrawn**. It was a 24.400 s recording of something
else. That is the trap the rule exists to prevent, and it caught its own author.

---

## One operational hazard, met live

Mid-session the shared store's `286279/tools/mt_main.rs` changed under me from
826 lines to a truncated 807, and a clean-room build off it failed with an
unclosed delimiter — another agent writing the file while I read it. `_container`
now carries its own copy of every source it needs (`tools/mt_main.rs`,
md5 `999f005c2522e57d681192263641efe9`) so a rebuild here depends on nothing
another agent is editing. **Do not build from another map's directory; copy what
you need first.** Same lesson as ACQUISITION §8a, third occurrence.
