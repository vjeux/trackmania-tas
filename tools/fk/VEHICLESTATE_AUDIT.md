# VEHICLESTATE_AUDIT.md — everything that touches `CSceneVehicleVisState`

Written 2026-08-23, after `--carrier layout` gathered 99 channels out of engine
memory, reported them, named which of them varied, and wrote a file in which all
99 were the donor container's constants.

That was one bug. This is the audit that asked whether it was **one** bug, and
the answer is no: it is one instance of a shape that the code around this
structure reproduces in at least five places. The shape is

> **a fact about the structure, stated more than once, with nothing forcing the
> statements to agree — and a reporting path that reads one statement while the
> artefact is built from another.**

Nothing in this file is a style complaint. Every item below is either a defect
that was live when it was written, or the mechanism by which a defect of this
class stays invisible.

---

## 0. Why this class is invisible here, specifically

Three properties of this subsystem combine into a blind spot:

1. **None of these bytes affects the simulation.** The oracle re-simulates the
   written file and gets the right time whether the wheel rotations are the
   car's, the donor's, or zero. So the strongest check the project has —
   "the real engine agrees" — is silent on the entire subject.
2. **Zero is a legal value for almost every one of them.** A dead read and an
   honest rest look identical in one sample, and telling them apart needs the
   whole run.
3. **Every report is built from the GATHER, not from the FILE.** `fk regen`
   printed "99 channels over 260 instants", listed which never move, and printed
   a per-byte coverage line — all of it computed from `carrier_vals`, the
   in-memory gather result, which was correct. The file was not. No statement in
   the tool was false; the tool simply never looked at its own output.

The general rule this yields, and the one worth carrying out of here:

> **A tool must verify the artefact it produced, not the intermediate it produced
> it from.** Everything else is bookkeeping about bookkeeping.

---

## 1. The defect found, and where the same shape sits elsewhere

### 1.1 `ci` bound the transform choice AND the field write — FIXED

`fk regen`: `let ci = if xform_from_fields { carrier_vals.get(&ms) } else { None }`,
and `ci` then fed both the transform selection (which is what
`--transform-from-fields` is about) *and* the loop that writes the carrier
bytes (which it is not). Without the flag the carrier loop iterated `&[]`.

The pointer path made it total: `fk regen` **refuses** `--transform-from-fields`
together with a pointer chain, so once the pointer landed, that path could
never write a single carrier byte.

*Fixed:* the two bindings are separate. *Guarded:* a read-back after the write
decodes the file and requires every channel the gather saw **vary** to vary in
it.

### 1.2 The write mask and the write are two parallel truths — PARTLY GUARDED

`fk regen` builds `w: Vec<bool>` ("which bytes we wrote") from
`written_bytes()` plus the carrier's returned channels, and separately writes
bytes in the sample loop. Nothing ties them. `w` is what the final coverage line
reports, so the two can — and did — disagree.

The new read-back closes this for channels that *vary*. It cannot close it for a
byte that is legitimately constant: those are still claimed on the strength of
the mask alone.

*Recommended:* have the sample loop **record** what it wrote and derive `w` from
that, rather than predicting it. The prediction has no reason to exist.

### 1.3 `ghost::finish`'s three channel lists had drifted — FIXED

`must_be_live()`, `may_rest()` and `unwritten_channels()` were hand-maintained
in a crate that cannot see the crate that does the writing (`ghost` cannot
depend on `fk`; `fk` depends on `ghost`). On the day of the audit:

* `unwritten_channels()` announced bytes 5, 81–84, 89 and 91 as "zeroed rather
  than inherited" **in the acceptance report of every run**, months after the
  carrier began writing all seven from engine memory.
* Byte 91 (gear) was in `may_rest()` *and* `unwritten_channels()` — written and
  not written at once.
* Bytes 19, 20, 34 and 108–111, which genuinely are not written, were in
  neither list.
* `must_be_live()`/`may_rest()` knew nothing of bytes 76, 89 and 90 — the
  reactor members — so a regression back to constant reactor bytes would not
  have been reported at all.

Nothing failed, because a report cannot fail.

*Fixed:* `gbx::sample` is now the single statement of the sample-byte
vocabulary (`UNPREDICTED`, `DEAD_IN_SERVER`, `TRANSFORM`, and the DERIVED
`written_by_carrier()` / `not_written_by_carrier()`). `fk::vislayout`
re-exports it; `ghost::finish` derives from it. Two tests refuse a
contradiction: `a_channel_is_never_both_written_and_unwritten` and
`every_claimed_channel_is_one_the_writer_actually_writes`.

### 1.4 The report did not depend on whether the carrier ran — FIXED

`ghost regen` printed the same "UNWRITTEN (11 channels)" list whether
`--carrier` was passed or not. Without the carrier, only the transform is ours
and ~94 bytes are the donor's. The report described a run that had not happened.

---

## 2. Repetition: the same fact, stated N times

### 2.1 The four wheel-rotation slots — was FIVE statements, now one

`[92, 136, 180, 224]` — the reference-free signature that separates the real
vehicle struct from a bare position copy. It was stated as:

| where | as |
|---|---|
| `fk/cmd/ptr.rs` | `const WHEEL_REL: [i64;4] = [92,136,180,224]` |
| `fk/cmd/carrier.rs` (copy search) | `const WHEELS: [usize;4] = [92,136,180,224]` |
| `fk/cmd/carrier.rs` (layout mode, ~900 lines later) | `(0..4).map(\|k\| 92 + 44*k)` |
| `fk/vislayout.rs` | `0xac + 44*k` (state-relative) |
| `fk/cmd/liveness.rs` | `WHEEL0 + WHEEL_STRIDE*k + WHEEL_ROT`, against a **different anchor** |

Plus a sixth, in a different vocabulary: `fk regen`'s liveness guard names the
same four wheels as sample channels, `"u16@6" | "u16@8" | "u16@10" | "u16@12"`.

*Fixed:* `vislayout::wheel_rot_rel()` derives them from `WHEEL0`,
`WHEEL_STRIDE`, `WHEEL_ROT` and `POS_IN_STATE`; the other four sites call it. A
test asserts the derivation reproduces the measured `[92,136,180,224]` exactly —
because deriving them from the class descriptor's `0x88` instead puts them 32
bytes low, which scores **3 of 4** against neighbouring live floats. That is the
near-miss that looks like a result, and a derivation is only an improvement if
it is checked against the measurement.

### 2.2 `POS_IN_STATE` (0x50) and `STATE_SIZE` (0x360) — now one each

`0x50` appeared as a literal in six places in `carrier.rs` alone while
`cmd/ptr.rs` had it as a named constant. Both now live in `vislayout` — the
module that IS the structure — and `cmd/ptr.rs` re-exports.

### 2.3 Five independent implementations of "which copy is the car" — OPEN

Not consolidated, and this is the largest remaining item.

| where | how it decides |
|---|---|
| `carrier::find_car` | position match + wheel liveness, over a recording |
| `carrier::gather_fields` | position match vs the clean run + wheel liveness |
| `cmd/ptr.rs` (`find`) | position match vs the file's record, or vs the engine |
| `cmd/ptr.rs` (`check`) | position match + wheel liveness, grading a chain |
| `record.rs` `require_live` | steps sideways to a copy with live slots |

They agree today because they were each written against the same two rules. They
are not the same code, so nothing keeps them agreeing — and the two rules are
subtle enough that `cmd/ptr.rs` needed a **third** (`--truth engine`) once a
transplanted container separated the two vis states 978 m apart.

*Recommended:* one `CopyRule` that takes (candidate offsets, reference
positions, reader) and returns the ranked copies with their scores. Every caller
above is that function with a different reference. This is a real refactor, not
a rename, and it should be done with the `fk ptr check` grading table as the
control: the consolidated rule must reproduce 0.000000 / 0.000004 / 0.000008 m
and 4-of-4 wheels on the three maps in `POINTER.md`.

---

## 3. The silent-zero reader, and the partial read that is worse

Both `impl vislayout::State` (`Gathered`, `GatheredRec`) answer **0** for a byte
outside the gathered window. That is the right default — panicking mid
transcription would be worse — but it is exactly the mechanism that turned a
124-byte-short window into 116 bytes of confident zero that passed every
acceptance test.

Worse than the all-zero case: `f32()` composes **four** `byte()` calls, so a
float straddling the edge returns two real bytes and two zeros — a finite,
small, entirely plausible number. All-zero at least looks like nothing.

*Fixed:* `State::covers_state()` (default `true`, overridden by both real
readers) and `vislayout::pack_checked()`, which refuses to transcribe a state
the reader cannot see all of. The production path calls `pack_checked`. In
layout mode the copy search additionally **rejects as a candidate** any copy
whose whole 864-byte state is not inside the record, so the window's size is a
measured property rather than an assumption.

---

## 4. `fk liveness` is calibrated to one fixture — OPEN, now named

`cmd/liveness.rs` states `WHEEL0 = 496` "relative to `Layout::pos`". That is
`0xa8 + 408`: it bakes in a **408-byte** distance between the locator's anchor
and the copy of the car that holds the fields.

That distance is not a constant. On untitled 01 (2026-08-23) it is **124
bytes**. Run against any fixture with a different shadow, `fk liveness` reads
four unrelated floats and reports 0 of 4 live for a car whose wheels are
turning — the precise false negative that reads like "the window does not reach
the car", and the one that cost most of a day.

Left in place and **named in the source** (`ASSUMED_SHADOW`) rather than
silently patched: the constant is honest about the fixture it was measured on,
and the fix is not a better guess. The fix is to take the pointer.

*Recommended:* `fk liveness` should resolve `fk::ptr::DEFAULT_CHAIN` like
`fk regen` now does, and fall back to the search — at which point `WHEEL0` and
`ASSUMED_SHADOW` both disappear.

---

## 5. `FK_FIELD_REL` — deleted, and why it is worth remembering

A calibration knob that cached the field copy's offset **from the anchor**, so a
search with one right answer need not be re-run per fork.

The anchor is chosen per fork. The same file on the same binary picked
`base-1574780` on one attempt and `base-872608` on the next; re-running with the
printed value gathered a 1332-byte record with 0 of 4 wheel slots live.

The guards caught it every time, so it never produced a bad file — but a knob
whose value is only valid inside the process that printed it is a footgun, not a
cache. It is deleted. The pointer is the real form of what it reached for: an
address the engine itself holds, resolved fresh in every fork.

**The general lesson:** before caching a measurement, ask what it is measured
*relative to*, and whether that thing is stable across the boundary you intend
to cache over.

---

## 6. What is still open, in the order worth doing it

1. **Derive the write mask from the write** (§1.2). Small, and it closes the
   last gap the read-back cannot.
2. **One `CopyRule`** (§2.3). The big one. Control: `POINTER.md`'s grading table.
3. **`fk liveness` takes the pointer** (§4). Removes the last fixture constant.
4. **One byte-name table.** The 116 sample bytes are named in `gbx::record`
   (`FIELD_DOCS`, the decoder), in `fk::traj` (a `&[&str]` for CSV columns), in
   `ghost::finish` (human strings in the three lists) and in `tmtraj`'s corpus
   report. `gbx::sample` is now the right home; the classification moved there
   and the names should follow.
5. **A recording-based regression for the carrier**, not just for the transform.
   `ghost roundtrip` regenerates a recording and requires its own *trajectory*
   back. Nothing does that for the other 99 channels, and the machinery exists:
   `fk carrier layout` already scores a transcription byte-for-byte against a
   recording the game wrote. Wire that into the test suite with a checked-in
   fixture and the whole class of defect in this document becomes a red test
   rather than a published clip with no reactor.
