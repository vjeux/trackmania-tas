# ACQUISITION addendum — THE ORACLE IS ~1000× SLOWER THAN IT NEEDS TO BE ON A CUT TAPE

Found on 165922, 2026-08-18. Nothing here is map-specific: it applies to **every
map where the template is cut out of a long recording**, which is most of them.

Measured on a 2206-tick tape cut from an 8 790 769 ms human record:

| | before | after |
|---|---|---|
| a finishing candidate | 2.7 s | **0.03 s** |
| a DNF candidate | 32 s | **0.34 s** |
| throughput, 150 workers | 14.5 cand/s | **~500 cand/s** |

Three independent causes. All three are one-line-ish fixes and all three are
invisible — nothing warns you, the numbers are simply correct and slow.

---

## 1. `tmcut --strip` strips nothing, and never could

The ghost's recorded telemetry (`CPlugEntRecordData`, class `0x0911F000`) is
**not a top-level PIKS skippable chunk**. It is written inline inside the
CGameCtnGhost chunk `0x03092000`, as

```
u32 0x0911F000 | u32 version | u32 uncompressedSize | u32 compressedSize | <zlib>
```

Two consequences:

* `gbx::all_skip_chunks` walks top level only, and its class-id filter admits
  only top bytes `{0x03, 0x0B, 0x24, 0x2E, 0x30}`. `0x0911F000`'s top byte is
  `0x09`. So `tmcut --strip`'s `skips.iter().find(|c| c.0 == ENTREC)` **can
  never match** and the flag is a silent no-op. Check a "stripped" template's
  size before believing it: on 165922 every one was still 1.9 MB.
* The blob inflates to **24 309 292 bytes**. Per candidate. That is the 2.7 s,
  and it is memory bandwidth, so it gets *worse* with more workers.

**Fix (`m165 telmin`):** re-encode the record with the same header, entity
descriptors and notices but **zero samples** — the grammar is fully spelled out
in `tmtraj::entrec::parse_record_data`, and `read_encoded_deltas` accepts `n = 0`
as a bare `i32 0`. Then shrink the enclosing chunk header by the number of bytes
removed. 1 914 181 → **5 425 bytes**, and the tape still validates to the same
millisecond.

**Do NOT just empty the blob.** Setting `uncompressedSize = 0` with an 8-byte
empty zlib stream produces a file the server **refuses to load, silently**: the
ghost disappears from the batch (`Starting validation of 1 ghosts` where you
staged 2), there is no diagnostic line, and the caller reads `sim_time = None`.
That is indistinguishable from a DNF, which is exactly the class of failure this
project keeps getting burned by. The record has to stay grammatical.

## 2. A DNF is simulated all the way to the DECLARED race time

This is the big one, and it explains "a DNF costs ~20 s" on several maps.

A tape cut from an 8 790 769 ms record still **declares** 8 790 769 ms. A run
that never crosses the finish line is simulated to that clock — **independent of
the tape's own length.** Measured on 165922: a 300-packet DNF cost **22.5 s**, a
1985-packet DNF **17.7 s**. The input archive running out does not stop it.

The declared time lives in **four** places in the decompressed body, and
`RACE_TIME_CHUNK_ID = 0x03092005` — the one the codebase knows about — is *not*
the one that governs:

```
0x03092005          drives the walltime field; changing ONLY this leaves the DNF cost intact
0x0309200B  +12
0x0309201B  +10
0x0309202B  +4  and  +32     (the splits chunk: race_time and the finish split)
```

**Fix (`m165 setdecl_all`):** rewrite `0x03092005`, run `fix_walltime()`, then
replace **every remaining u32 in the body equal to the old declared time**.
DNF cost 17.7 s → **0.34 s**, finishers unchanged to the millisecond.

Two bonuses:

* **Free pruning.** Declare just above the incumbent and anything slower comes
  back DNF. For a minimising search that is pure win, and it makes the DNFs
  cheaper still.
* It confirms and sharpens the note in `tm2020-reliability.md` that "the
  validator only simulates as long as the input archive lasts" — that is true of
  the *inputs*, not of the *clock*.

## 3. `sweep::evaluate` silently caps the worker count

```rust
for wi in 0..workers.min((n + batch - 1) / batch).max(1)
```

With `tmex`'s default `--batch 600`, a 1500-candidate round runs on **three**
workers no matter what `--jobs` says. Pass `--batch 20`–`25`. Symptom: `--jobs
100` and a load average of 3.

---

## The check that would have caught all three in a minute

Time ONE candidate through the plain oracle, as a finisher and as a DNF, before
you size a search:

```
ls -la TEMPLATE.Ghost.Gbx                 # > 100 KB on a short tape = defect 1
m165 findu32 TEMPLATE.Ghost.Gbx <declared_ms>   # > 1 site = defect 2
```

A short template that is 1.9 MB, or that declares a race time from the recording
it was cut out of, is costing you two to three orders of magnitude.

`m165 mktpl IN OUT A:B,C:D DECL_MS` does join → telmin → setdecl_all in one
command. Source: `tm-unbeaten/165922/v3/m165.rs`.

---

## Unrelated, same session: `tmex --alpha` projects the WHOLE tape

Already reported from 274191 for `tmsearch --quant`; it is true of `tmex` too.
`quantise()` iterates all of `s.steer`, so `--alpha kb --lo 600 --hi 900`
quantises the prefix the search was told not to touch and the identity control
dies for a reason that has nothing to do with the constraint. Patched copy that
projects only `[--lo,--hi)`: `tm-unbeaten/165922/v3/tmexq.rs`.
