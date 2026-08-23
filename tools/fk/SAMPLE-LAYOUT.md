# SAMPLE-LAYOUT.md — all 116 bytes, read out of the writer instead of fitted

**The dedicated server contains the ghost's telemetry writer, and this is it.**
Every byte below is a transcription of the code that produces it, not a fit
against a recording. The transcription lives in `tools/fk/fk/src/vislayout.rs`
and is scored, byte for byte and with nothing to tune, by
`fk carrier layout`.

Binary: `TrackmaniaServer_Latest`, 30 113 288 B,
`date=2026-05-15_18_00 git=128182-0de74ece09e GameVersion=3.3.0`,
md5 `0f0f4b25f31f80c60c81404366c95e68` — the same binary
`BUILD-ID.md` disassembled, restored per `tmoracle/RESTORE.md`. Nothing was
patched. Addresses are objdump/file addresses (Ghidra's listing is `+0x100000`,
per `BUILD-ID.md`); **no decompiler was used here** — `tools/asmdig` reads the
call sites straight out of the objdump text.

---

## 0. The answer to the cheap first question

**Yes — the dedicated server carries the whole thing**, and the reading is
closed, not partial:

* the class `CSceneVehicleVisState` (`0x0A00C000`) is registered at `0x9d2ea0`
  with **every member name and byte offset** (`VEHICLEVISSTATE.md`);
* `sizeof` is **864**, which is both the stride of the array of copies
  `CARRIER.md` measured and the `u01` field of a real ghost's `EntRecordDesc`;
* the versioned archiver at **`0x9cfed0`** serialises the class, and its version
  gate `cmp r15d, 0x21` is **33** — the `u03` field of that same
  `EntRecordDesc`;
* on the version-33 path it emits **exactly 116 bytes**.

The client was not needed and was not touched.

### The arithmetic control, which is what makes this a reading and not a story

The archiver emits an 85-byte packed block and then 31 bytes field by field,
each field gated on the version. Adding the field widths up:

| version | fields emitted | bytes |
|---|---|---|
| 30 | the block, `state+0x15c`, the bit dword, 4 wheels × 2, wetness, sim-time | **103** |
| 31 | + `state+0x80` | 107 |
| 32 | + `state+0x344`, the countdown | 112 |
| **33** | + `state+0x348` | **116** |

116 is the sample size TM2020 ghosts carry. **103 is the number the project's
own decoder already documents as the floor** (`gbx::record`, *"sampleSize >=
103"*), written down from GBX.NET before any of this. Two independent sources
landing on the same two numbers is the control that says no field was missed
and none was invented. A test pins the ladder
(`the_field_sizes_add_up_to_the_sample_size`).

### Where it is

```
0x9cfed0  CSceneVehicleVis archiver, version-gated       [reads and writes]
  0xaca280 -> 0xac9e20 -> 0xacb110 -> 0xacb230 -> 0xacb520
            fill the flat 85-byte block = sample bytes 0..84
  0x1ab31b0 copy a vec3          0x1ab3480 pack a quaternion into 6 bytes
  0x1ab32d0 pack a vector as i16 1000·ln|v| + heading + pitch
  0x1af5f40 rotation matrix -> quaternion
```

`rdi` is the sample buffer, `rsi` the `CSceneVehicleVisState`. The block writer
does 64-bit read-modify-writes at `[rdi]`, `[rdi+8]`, `[rdi+0x10]`,
`[rdi+0x12]`, `[rdi+0x1a]`, which is why the fields do not sit on tidy
boundaries.

---

## 1. The measurement

`fk carrier layout` rebuilds all 116 bytes from engine memory and compares them
with the recording the game wrote. **There is no coefficient to fit, no offset
to choose and no `--refit` flag**; a byte is right or it is wrong.

**Nine game-written recordings on eight maps, plus one negative control.**

| key | map | instants | the live-wheeled copy is … from the recorded path | hold / fail |
|---|---|---|---|---|
| `map2_22730` | map2 | 455 | **0.000001 m** | 49 / 10 |
| `map2_23013` | map2 | 461 | **0.000001 m** | 49 / 10 |
| `m285885_poke` | 285885 | 1013 | **0.000000 m** | 46 / 12 |
| `m134672_kb` | 134672 | 1353 | 0.000485 m | 55 / 6 |
| `m145875_kb` | 145875 | 127 | 0.000490 m | 42 / 14 |
| `m191465_kb` | 191465 | 262 | 0.000509 m | 46 / 8 |
| `m203072_kb` | 203072 | 215 | 0.000507 m | 46 / 8 |
| `m270051_kb` | 270051 | 97 | 0.000499 m | 41 / 6 |
| `m279209_kb` | 279209 | 132 | 0.000497 m | 47 / 6 |
| **`TAS_67200`** — telemetry is a **stranger's** | 134672 | 1345 | 0.000495 m | **17 / 44** |

**The last row is the control that makes the other nine mean something.**
`TAS_67200` is a search tape carried on a donor's container: its telemetry was
never written for its inputs. Run through the identical rig it collapses to
17 hold and 44 fail. The instrument is not one that passes everything.

The half-millimetre in column four is `CARRIER.md` §6's own number — the
distance between the copy that holds the fields and the copy the transform came
from. On those keys the engine state and the recorded sample are **not the same
instant**, and the fields that suffer are the fast ones (the low byte of a wheel
rotation wraps twice between samples). Every byte still beats its constant
there; the byte-exact claim is made where the two coincide.

### On the two map2 keys, where the copy IS the recorded instant: 38 bytes exact on every sample

100.00 % on all 455 and all 461 samples of both keys, with nothing fitted:

```
0  1  2  3  4  5      FrontSpeed, the lateral speed, rpm
6  7  8  9 10 11 12 13   the four wheel rotations
14 15 18              InputSteer, InputGasPedal (twice -- see below)
22 23 25 27 29 31     SteerAngle, the four dampers, the turbo/enum byte
48 49 53 54 56 57     the high bytes of the position
65 66 68              WorldVel
69 70 71 72           the SECOND vector, at state+0x68
89 91                 the reactor dword, and CurGear
```

and just below, worst of the two:

| byte | worst | what the gap is |
|---|---|---|
| 24, 26, 28, 30 | 99.78 % | one sample in 455; the ground-material substitution |
| 32 | 97.58 % | `SlipCoef > 0.1` on a threshold the state crosses between writes |
| 33 | 96.53 % | the same, three wheels' worth |
| 67 | 93.06 % | the `WorldVel` heading byte, ±1 |
| 47, 51, 55 | 71–84 % | the LOW byte of each position f32 — the known transform issue, §6, not this reading |

Across all nine keys the worst-key figures are in `raw/rollup9.txt`; every byte
that is not on the dead list below holds on every key that gives it power.
Raw per-key reports: `raw/L_*.tsv` and `raw/L_*.txt`.

### The positive control: it reproduces another arm's numbers to the sample

`CARRIER.md` §6 measured, independently and two days earlier, how many of
`human_22730`'s 455 samples reproduce the recorded position bytes when the
transform is read from the live-wheeled copy: **byte 47: 335, byte 51: 396,
byte 55: 350**. This rig, on the same file, scores those three bytes at
**73.63 %, 87.03 % and 76.92 %** of 455 — which is **335, 396 and 350**. Three
for three, to the sample. The gather and the anchor are the same instrument
that produced the existing table, so a disagreement in §3 is a disagreement
about the ENCODING and not about the rig.



---

## 2. The reactor, which is the thing that was missing

**All five reactor members of `CSceneVehicleVisState` are recorded, and they are
in bytes 89, 90, 91 and 76.**

| member | where it lives in the state | where it lands in the sample |
|---|---|---|
| `IsReactorGroundMode` | flags bit 19 | **byte 89 bit 1** |
| `ReactorInputsX` | flags bit 18 | **byte 89 bit 2** |
| `ReactorBoostType` | `state+0x178`, car+296 | **byte 89 bits 3-4** (2 bits), and **byte 76 bit 4** = `!= 0` |
| `ReactorBoostLvl` | `state+0x174`, car+292 | **byte 89 bits 5-6** (2 bits) |
| `ReactorAirControl` | `state+0x180..0x188`, car+304 | a tri-state per component: **byte 90 bits 4-5** (x), **bits 6-7** (y), **byte 91 bits 0-1** (z) — 0 negative, 1 zero, 2 positive |

### And they are CONFIRMED, on a key that fires them

`fk carrier layout` scores the packed fields individually, because a byte can
pass on the strength of one of the six quantities in it — byte 89 is 100.00 % on
a run with no reactor in it purely because `IsGroundContact` is.

**`m203072_kb`** — the game's own keyboard recording of YEET Fall 2024-04, a map
with three boost gates — fires them, and the transcription reproduces them:

| field | exact | a constant scores | distinct values |
|---|---|---|---|
| `ReactorBoostType` | **100.00 %** | 52.09 % | 2 |
| `ReactorBoostLvl` | **100.00 %** | 52.09 % | 2 |
| `ReactorBoostType != 0` (byte 76 bit 4) | **100.00 %** | 52.09 % | 2 |
| `ReactorInputsX` | **100.00 %** | 67.44 % | 2 |
| `ReactorAirControl.x` | **100.00 %** | 67.44 % | 2 |
| `ReactorAirControl.y` | 89.30 % | 68.84 % | 3 |
| `IsReactorGroundMode` | 99.07 % | 66.05 % | 2 |
| `CurGear`, in the same dword | 97.67 % | 70.70 % | 3 |

A second boost-gate map, **`m145875_kb`**, agrees on the two that matter —
`ReactorBoostType` 99.21 % and `ReactorBoostLvl` 99.21 % against a 79.53 % and a
44.88 % constant — and disagrees on `ReactorInputsX` (35.43 %) and
`ReactorAirControl.x/.y`. That key's anchor is 0.000490 m off, i.e. the engine
state and the sample are not the same instant, and single bits are exactly what
that costs. **Stated as it is: two of the five are confirmed on two independent
keys, three are confirmed on one, and `ReactorAirControl.z` never moves on any
of the nine.**

`ReactorAirControl` is stored as a **tri-state per component**, not a value:
0 negative, 1 zero (`|v| < 1e-5`), 2 positive. `CurGear` occupies bits 18-21 of
the same dword, i.e. byte 91 bits 2-5, and bits 0-1 are
`ReactorAirControl.z` — which reads **1** whenever the reactor is idle. That is
exactly why the decoder's `gear_raw = 4·gear + 1` works, and it means **byte 91
stops satisfying `4g+1` the moment a reactor fires**. The +1 is not a constant,
it is a reactor channel.

### Why byte 89 was "closed"

`CARRIER.md` records byte 89 as `is_ground_contact`, refused a fourth time at
0.00 % as a raw byte, with three earlier arms failing on it from three
directions. It is refused because **it is not a byte**: it is bit 0 of a 32-bit
field packed across bytes 89, 90 and 91, carrying six quantities and the gear.
No per-byte affine fit can represent that, so no sweep of that shape could ever
have found it. Read as the field it is, byte 89 is 100.00 % on both map2 keys
and holds on seven of the nine.

### Byte 34 is NOT the reactor, and the reason matters

The sibling arm's correlation (byte 34 live on the 9-boost-gate map, constant on
201 files across four maps) pointed at byte 34. The writer says byte 34 is
`floor(state[0x224] · 255)` — a car-level normalised float, exactly the shape
predicted, three floats away from `GroundDist`. But:

**`state+0x224` is identically zero in the dedicated server for the whole run,
on every key, while the recording moves.** On `map2_22730` the recorded
byte is 255 for 49.89 % of samples and 0 for 19.56 %, and the prediction is 0
for 100 % of them. The same is true of `state+0x228` and `state+0x22c` (sample
bytes 19 and 20) and of the four "dirt" slots at `wheel+0x18` (bytes 93, 95, 97,
99).

That is a **fourth verdict** and `fk carrier layout` prints it as one:
*the source slot is dead in this binary*. It is not a wrong offset — a wrong
offset reads noise, and this reads a constant zero while every neighbouring
offset in the same struct reproduces the recording exactly. These are fields the
server does not compute because it runs the simulation and not the
presentation.

**So the empirical route to byte 34 was never blocked for want of an answer key.
It is blocked because the quantity does not exist in the dedicated server's
memory.** No sweep of that server, on any recording, could ever have found it.
The only route to bytes 19, 20, 34 and the dirt is the client.

---

## 3. Where this contradicts the frozen table

Five places. `tools/fk/carrier-bytes.tsv` is **left exactly as it is**; these are
reported, not applied. In each case the disassembled form scores 100.00 % on
both map2 keys where the table's form has a worse worst-key in `CARRIER.md`.

| # | the table says | the writer says | table's worst of 8 | this reading, worst of the 2 map2 keys |
|---|---|---|---|---|
| 1 | `b22` = `floor(v·40.743044 + 127.5)` at car+100 | `SteerAngle` over **[-π, π]**: `floor((v+π)/(2π)·255)`, i.e. **k = 255/(2π) = 40.5845** | 92.31 % | **100.00 %** |
| 2 | `u16@4` rpm = `floor(v·2.1844886 + 0.1)` | `floor(v/30000·65535)` — **the exact fraction 65535/30000 = 2.18450, and no offset** | 96.91 % | **100.00 %** |
| 3 | `b31` `is_turbo` = the raw engine byte at car+332 | byte 31 = `(state[0x19c] & 7) | (IsTurbo << 7)` — three bits of an enum plus **one bit from the flag word at car+56**, not a byte copy | 91.60 % | **100.00 %** |
| 4 | `b24/26/28/30` = the engine byte, verbatim | the engine byte **unless the wheel's flag bit 1 is set, when the writer emits 13** | 97.44 % | **99.78 %** |
| 5 | dirt (93, 95, 97, 99) is **refuted** as "a ×255 float in the wheel's 44-byte record" | it is exactly that — `wheel+0x18 · 255` — and `TireWear01` is at **94, 96, 98, 100**, which nobody has tested | −7.35 pts below a constant | source slot dead |

On (1): the table's coefficient is the WHEEL constant, 40.743044, borrowed onto
a field that does not use it. `SteerAngle` never leaves ±0.5236, where the two
slopes differ by at most 0.085 of a byte — enough to lose 8 % of samples to a
truncation and not enough for a fitter to notice it had the wrong number.

On (2): `CARRIER.md` says *"65535/30000 = 2.184500 is 5e-6 away and is a guess,
so the table carries the measured number."* The guess was right and the
measurement was absorbing the truncation.

On (5): the refutation tested the right hypothesis at the right offsets. It
failed because the slot is dead on the server, not because the hypothesis was
wrong — and the pre-registration searched *the eight slots the three placed
fields leave*, which would have found `wheel+0x18` if it had had a signal to
find.

### And one about a writer, not a reader

`gbx::recwrite::write_transform` **rounds** where the game **truncates**
(`cvttss2si`, every time, in all three packers). That is invisible when
re-encoding a value that was already on the grid — which is what §6's
453-of-455 round-trip control measured — and it is not invisible when writing a
fresh value out of engine memory. The transcription truncates and scores
100.00 % on `WorldVel` bytes 65, 66, 68 and on all four of the second vector's
bytes.

---

## 4. The layout

`fk carrier bytes` prints this without running anything.
Offsets are into `CSceneVehicleVisState`; `car+N` is `state + 0x50 + N`.
Everything marked **new** is a byte no previous table named.

| bytes | field | encoding | status |
|---|---|---|---|
| 0-1 | `FrontSpeed` | `u16 = (min(v,10000)+1000)/11000·65535`, 0 below −1000 | named (was "an unnamed 16-bit quantity") |
| 2-3 | the lateral speed, `state+0x78` | `u16 = (min(v,1000)+1000)/2000·65535` | confirmed |
| 4-5 | rpm, `state+0x198` | `u16 = v/30000·65535` | **coefficient corrected** |
| 6-13 | `Wheels[k].Rot` | `u16 = v/(2π·256)·65535`, and **`0xcdcd` is bumped to `0xcdce`** | confirmed |
| 14 | `InputSteer` | `(v+1)/2·255` | **new** |
| 15 | `InputGasPedal` | `v·255`, **zero while `InputIsBraking`** | **new** |
| 16-17 | — | a literal zero word | **new (dead)** |
| 18 | `InputGasPedal` | `v·255`, **only while `InputIsBraking`** | **new** — this is why the decoder's `gas = b15/255 + b18/255` works |
| 19 | `state+0x228` | `(v+1)/2·255` | **new**, source slot dead on the server |
| 20 | `state+0x22c` | `(v+1)/2·255` | **new**, source slot dead |
| 21 | `TurboTime` | `v·255` | confirmed |
| 22 | `Wheels[0].SteerAngle` | `(v+π)/(2π)·255` | **coefficient corrected** |
| 23,25,27,29 | `Wheels[k].DamperLength` | `(v+2)/4·255` | confirmed |
| 24,26,28,30 | `Wheels[k]` ground material | the raw byte, **or 13 when the wheel's flag bit 1 is set** | **rule corrected** |
| 31 | `state+0x19c` and `IsTurbo` | bits 0-2 = enum & 7; bits 3-6 = 0; bit 7 = flag bit 24 | **decomposed** |
| 32 | `Wheels[0]` | bits 0-5 = 0; bit 6 = `SlipCoef > 0.1`; bit 7 = wheel flag bit 2 | **new** |
| 33 | `Wheels[1..3]`, `IsWheelsBurning` | six wheel bits, bit 6 = `state+0x1a0 > 0`, bit 7 = flag bit 5 | **new** |
| 34 | `state+0x224` | `v·255` | **new**, source slot dead |
| 35-38 | — | a literal zero dword | **new (dead)** |
| 39-40 | `state+0x244 … 0x260` | eight 2-bit codes: 0 / `<0.5` / `<0.99` / else | **new** |
| 41 | `state+0x264`, `state+0x84` | bits 0-1 a 2-bit code; bits 2-4 = 0; bits 5-7 = `round(v·7)` | **new** |
| 42 | `state+0x30c..0x310` | five bools, bits 5-7 = 0 | **new** |
| 43 | gas, `state+0x1bc`, `state+0x24`, `DiscontinuityCount` | 2 + 1 + 1 + 4 bits | **new** (see below) |
| 44 | `state+0x7c` | `v/5·255` | **new** |
| 45 | flag bit 12 | a bool | **new** |
| 46 | `state+0x1c0` | `v·255` | **new** |
| 47-58 | `Loc.translation` | three raw f32 | confirmed |
| 59-64 | `Loc` rotation | quaternion from the 3×3 at `state+0x2c`, then `acos(qw)/π·65535` and two angles, **truncated** | **located** |
| 65-68 | `WorldVel` | `i16 = 1000·ln‖v‖` truncated, then heading and pitch bytes | **new** |
| 69-72 | `state+0x68`, an unnamed vec3 | the same 4-byte pack | **new** |
| 73 | `state+0x1bc`, `state+0x8` | two nibbles | **new** |
| 74 | `state+0x158` | `v/(2π)·255` | **new** |
| 75 | `state+0x8` | the raw byte | **new** |
| 76 | flag bits 4,6,7,8,9,10,17 + `ReactorBoostType != 0` | eight bits; bit 5 is `IsTopContact`, as the decoder had it | **decomposed** |
| 77-80 | `state+0x338` | a raw u32 | **new**; bytes 79-80 disagree — see below |
| 81-84 | `Wheels[k].Icing01` | `v·255` | confirmed |
| 85-88 | `state+0x15c` | a raw f32 | **new** |
| 89-92 | the reactor dword | see §2; byte 92 is always 0 | **new** |
| 93,95,97,99 | `Wheels[k]+0x18` (dirt) | `v·255` | **new**, source slot dead |
| 94,96,98,100 | `Wheels[k].TireWear01` | `v·255` | **new** |
| 101 | `WetnessValue01` | `v·255` | confirmed |
| 102 | `SimulationTimeCoef` | `v·255` | confirmed |
| 103-106 | `state+0x80` | a raw f32 | **new** |
| 107 | `state+0x344` | a raw byte | **new** |
| 108-111 | `state+0x340` | `-2 - min(now - t, 3000)`, a countdown | **new**, needs the archiver's clock |
| 112-115 | `state+0x348` | a raw u32 | **new** |

**Nothing in the 116 bytes is unaccounted for**: every one of them names a
source in the writer. 106 of them are predicted and scored by
`fk carrier layout`; the 10 that are not are the six orientation words, which
are located but not re-scored here, and the four countdown bytes, which need
the archiver's caller-supplied clock.

### The two that are right and still disagree

* **Byte 43** is 0.00 % on every key with the recorded byte 100 % constant. The
  low nibble matches exactly (gas code and two bools); the high nibble is
  `DiscontinuityCount`, and the server's counter is 1 where the recordings carry
  3, 10, 11, 13 and 15. The layout is right and the *value* is a client-side
  counter the server does not reproduce. Same class as the dead slots, one
  nibble wide.
* **Bytes 79-80** are the high half of the u32 at `state+0x338`. Bytes 77-78
  agree; the server holds `0x0ff0` where every recording holds 0. A field the
  server fills with something of its own.

---

## 5. What to do next, in order

1. **A third boost-gate key, with a micron anchor, closes
   `ReactorInputsX`, `IsReactorGroundMode` and `ReactorAirControl`.** Two of the
   five reactor fields are confirmed on two keys and three on one; the gap is
   an anchor at half a millimetre, not a missing hypothesis.
2. **`fk regen --carrier` should write from this layout, not from the table.**
   Five of its 23 rows are wrong in a way that costs whole samples, and 55 bytes
   it does not write are now writable. That is a change to the publish path and
   wants a corpus re-run.
3. **Byte 91 is not `4·gear+1`.** Anything that assumes it — the decoder does —
   is right only while the reactor is idle.
4. **Bytes 19, 20, 34, 93/95/97/99 and the high nibble of 43 need the client.**
   The dedicated server cannot source them, measured, and that is a property of
   the binary rather than of the corpus.
5. **The orientation words (59-64)** now have their source: a 3×3 rotation
   matrix at `state+0x2c` (car−36) with `Left/Up/Dir` as its columns, converted
   by `0x1af5f40` and packed by `0x1ab3480` with truncation. `CARRIER.md` §6
   asks "find where the quaternion lives on the live-wheeled copy" — it does not
   live there as a quaternion at all, which is why searching for a unit
   quaternion found something 2632 bytes away.

## 6. Reproducing it

```bash
objdump -d --no-show-raw-insn -M intel TrackmaniaServer > cur.asm
tools/asmdig calls cur.asm TrackmaniaServer 9d2ea0   # the member table
tools/asmdig fn    cur.asm TrackmaniaServer acb520   # the packed block
tools/asmdig consts        TrackmaniaServer 1608.4954,30000,11000
cd tools/fk && cargo build --release
TM_SERVER=... FK_SHIM=... ./target/release/fk carrier layout \
    --template KEY.Ghost.Gbx --map M.Map.Gbx --tag KEY --out layout_KEY.tsv
./target/release/fk carrier rollup --tables layout_A.tsv,layout_B.tsv,...
```

`tools/asmdig` is new: it resolves the argument registers at every call in a
function against the ELF, which is what turns a stripped descriptor table into
a member list. It replaces the `h.sh` grep helpers of `BUILD-ID.md`.
