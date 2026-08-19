# 203330 — constraint and correctness controls

Companion to `RESULT-v3.md`. Everything here was run on 65139 against
`map.Map.Gbx` with the plain oracle as the adjudicator.

## 1. The keyboard tapes really are keyboard tapes — two independent proofs

**Proof A, from the artefacts themselves.** The distinct steering values in each
delivered tape, read back out of the file:

```
best/kb330_13984.Ghost.Gbx           3 distinct -> [0, left, right]
lowinput/kb330_31ev_13984.Ghost.Gbx  3 distinct -> [0, left, right]
lowinput/kb330_12ev_13986.Ghost.Gbx  3 distinct -> [0, left, right]
best/an330_13984.Ghost.Gbx           192 distinct  (the analog tape, for contrast)
```

This is the proof that matters for the deliverable: whatever the search did, the
published tapes are drivable on three keys.

## 2. The ZERO LADDER — proving the constraint bit, not just that it was passed

The coordinator's control: run the search with a value ladder of `{0}`. Every
steering input becomes zero, the car drives straight, and a constraint that is
actually applied must produce `finish 0%`. Run against the same configuration
that produced the keyboard tapes, over a window where steering demonstrably
matters (`--lo 776`, which contains the launch):

| arm | ladder | result |
|---|---|---|
| fork path, `--quant 0` | `{0}` | **`finish 0%`** |
| fork path, `--quant -127,0,127` | keyboard | `finish 69%` |

**The constraint bites on the fork path**, which is the path every keyboard tape
in this directory came from.

## 3. …and it does NOT bite on the classic path

The same two ladders with `--fork` omitted:

| arm | ladder | result |
|---|---|---|
| classic path, `--quant 0` | `{0}` | `finish 70%`, best 13.984 |
| classic path, `--quant -127,0,127` | keyboard | `finish 70%`, best 13.984 |

Identical, and the zero ladder finishes 70% of the time — so **`--quant` is a
no-op on the classic path in this build** and a classic-path arm given it runs
completely unconstrained while its log looks perfect. This reproduces the fleet
finding independently. Nothing here was affected (all constrained runs were
fork-path), but it is the reason the zero ladder is worth running every time:
without it, §1 alone cannot distinguish "the constraint worked" from "the search
happened not to leave the ladder".

## 4. A caveat on the zero ladder that this map exposes

The first zero-ladder attempt used the production window `--lo 1100` and came
back `finish 100%` — which looks like a broken constraint and is not. On this
map **steering in ticks 1100-1552 has no effect on the finish time at all**
(9113 forced-input variants, best +0 ms): zeroing an axis that is already inert
cannot make the car fail.

So the control has a precondition: **the constrained window must contain inputs
that demonstrably matter.** Check that first — otherwise `finish 100%` under a
zero ladder is a statement about the map, not about the constraint.

## 5. Other controls run on this map

- **Identity control** in every validation batch: all five human ghosts
  re-simulate to their exact recorded millisecond, and `r01` (14.018) was carried
  in every batch that produced a claim.
- **Factory round-trip**: `tmsearch --verify` of the seed re-validates to 14.018.
- **Fork-resume exactness** against full `/validatepath` of the same tapes:
  200/200 at boundary tick 620, 250/250 at 1000, 250/250 at 1200, 200/200 at
  1450. Zero mismatches.
- **Triple validation of the result**: `an330_13984` and `kb330_13984` validated
  in a mixed batch, again in a smaller batch, and again one file at a time.
- **Phantom rate**: zero in ~90 banked tapes. The 35 tapes the guard quarantined
  all validate at 13.984 (see `RESULT-v3.md` §7).
