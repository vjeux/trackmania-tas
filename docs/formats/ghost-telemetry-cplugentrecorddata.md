# The telemetry record: `CPlugEntRecordData` (`0x0911F000`) and the 116-byte vehicle sample

The car's recorded state every 50 ms — what a render PLAYS (the dedicated
server never reads it; it re-simulates the tape). Decoder `tools/gbx/src/record.rs`
(port of GBX.NET `CPlugEntRecordData.cs`, `CSceneVehicleVis.EntRecordDelta.cs`,
`GbxReader.ReadTransform`), encoder `tools/gbx/src/recwrite.rs` (byte-exact
round trip), vocabulary `tools/gbx/src/sample.rs`, the writer TRANSCRIBED from
the dedicated server binary in `tools/fk/fk/src/vislayout.rs`
(`tools/fk/SAMPLE-LAYOUT.md`, `VEHICLEVISSTATE.md`, `CARRIER.md`). Tools:
`ghost record show|rebuild|graft-scene|channels|dumpbytes|countdown|shorten`,
`tmtraj show|export|fields|bytes|provenance`, `ghost regen`.

Confidence tiers used below: **VERIFIED** = cross-checked numerically against
independent ground truth; **DERIVED** = GBX.NET's reference, self-consistent
here; **GUESS** = byte position and name from GBX.NET only; **EXE** = read out
of the server's own archiver (`0x9cfed0`, version gate 33 → 116 bytes; the
field widths sum to 103 at v30 and 116 at v33, matching the decoder's floor).

## 1. Where it sits

Inside the ghost's main chunk `0x03092000`, after the display name:

```text
u32 nodeIndex        1 in a standalone ghost, 2 inside a replay (the ONLY difference
                     `ghost unwrap` rewrites); ABSENT in a map's embedded validation ghost
u32 class 0x0911F000
u32 chunk 0x0911F000
u32 version          11 on TM2020 2023+ ghosts (versions < 5 store the data uncompressed
                     inline — not seen, not supported)
u32 uncompressedSize
u32 compressedSize
zlib stream          starts 78 9C. Level 9 writes 78 DA and every reader here keys on 78 9C
                     (a file that validates is not thereby readable): compress at level 6
```

`find_entrecord_blob` locates it by needle; the class id often appears twice in
a row (node class, then chunk id).

## 2. Grammar (version ≥ 11 path)

```text
i32 start_ms; i32 end_ms                          the record's declared SPAN
i32 nDesc; nDesc × EntRecordDesc { u32 classId; i32 u01; i32 u02; i32 u03; data u04; i32 u05 }
v≥2: i32 nNotice; nNotice × { i32 u01; i32 u02; v≥4: u32 classId }
EntList := u8 hasNext; while hasNext:
    i32 type            index into the desc array
    i32 u01; i32 u02    (u02 ≈ start ms)
    i32 u03             ≈ end ms of this entity
    v≥6: i32 u04
    v≥11: EncodedDeltas  (v<11: a plain list of (time, MwBuffer))
    u8 hasNext
    v≥2: Deltas2 := while u8: (i32 type; i32 time; data)
v≥3: BulkNoticeList := while u8: (i32; i32; data)
     CustomModulesDeltaLists: v≥8: i32 nLists (else 1);
        each := (while u8: i32 u01; data; v≥9: data); v≥10: i32 period

EncodedDeltas := i32 numSamples; if numSamples: i32 sampleSize;
                 numSamples × i32 deltaTime (cumulative → absolute ms)
                 then COLUMNAR delta coding: for each byte index i in [0, sampleSize): numSamples
                 bytes; a running u8 accumulator across samples (acc += byte) gives
                 sample[b].data[i]; the accumulator resets per column.
```

The parse consumes the blob to its exact last byte on every test ghost — a
strong check on every width above — and the encoder reproduces it
byte-for-byte (`tmtraj rec roundtrip`), which is what licenses every edit.

### 2.1 The descriptors on a 2026 recording (`ghost record show`)

| desc | class | u01 | u03 | meaning |
|---|---|---|---|---|
| 0 | `0x0A019000` | 588 | 19 | undecoded |
| 1 | `0x2F0CB000` | 4 | 0 | undecoded |
| 2 | `0x0A018000` **CSceneVehicleVis** | **864** | **33** | the car: `u01` = `sizeof(CSceneVehicleVisState)`, `u03` = the archiver version (33 → 116-byte samples) |
| 3 | `0x032E3000` | 68 | 1 | undecoded |
| 4 | `0x032AC000` | 48 | 1 | undecoded |
| 5 | `0x2D001000` | 2156 | 0 | undecoded; a LIVE one per recording (454 samples × 13 B on the fixture) — the record the client crashes without (§7) |
| 6 | `0x032CB000` | 40 | 3 | a 0-sample placeholder carrying the checkpoint blocks in its Deltas2 |

Entities on the fixture: ent 0 = desc 2, 455 samples × 116 B, t 0..22700,
`u01 33554438`, 12 Deltas2 blocks (LIVE, the car); ent 1 = desc 5, 454 × 13 B;
ent 2 = desc 6, 0 samples, `u03 22730`, 3 Deltas2 blocks (placeholder).

## 3. The 116-byte `CSceneVehicleVis` sample — as the decoder reads it (GBX.NET view)

`decode_vehicle_sample` (offsets are bytes into the sample):

| bytes | field | decode | tier |
|---|---|---|---|
| 2–3 | side_speed | `u16`: `((v/65536) − 0.5)·2000` | DERIVED |
| 5 | rpm_raw | `u8`, 0..255, monotone with load (the u16 at 4–5 is the real rpm, §4) | VERIFIED (raw) |
| 6–7, 8–9, 10–11, 12–13 | fl/fr/rr/rl wheel rotation | `b/255·2π + b'·2π` (rotation + turn count), radians | DERIVED |
| 14 | steer | `((v/255) − 0.5)·2` — exactly: `steer_byte(s) = floor((s+127)·255/254)`, injective, skips values | DERIVED → exact inverse `steer_i8_from_byte` |
| 15 | gas | 255 down / 0 up (digital) | DERIVED |
| 18 | brake | 255 / 0 | DERIVED |
| 21 | turbo_time | `b/255` | DERIVED |
| 23, 25, 27, 29 | fl/fr/rr/rl dampen | `((b/255) − 0.5)·4` metres (a zero decodes as −2 = wheels fully extended) | DERIVED |
| 31 | is_turbo | `b & 0x82 != 0` | DERIVED |
| 47–58 | x, y, z | three `f32`, world metres, Y up | VERIFIED (start = start block centre to < 0.01 m) |
| 59–60 | angle | `u16 · π/65535` | VERIFIED |
| 61–62 | axisHeading | `i16 · π/32767` | VERIFIED |
| 63–64 | axisPitch | `i16 · π/32767/2` | VERIFIED |
| 65–66 | speedLog | `i16`: `speed = exp(v/1000)` m/s (matches `|dpos/dt|` to ~0.02 m/s) | VERIFIED |
| 67 | velHeading | `i8 · π/127` | VERIFIED |
| 68 | velPitch | `i8 · π/127/2` | VERIFIED |
| 76 | vehicle_state_raw; bit 5 = is_top_contact | | DERIVED |
| 81–84 | fl/fr/rr/rl ice | `b/255` | GUESS |
| 89 | ground_mode_raw; bit 0 = is_ground_contact | on 153527, 286279, 284238 the bit reads `false` on every sample of a car on a road (rejected per recording when the "airborne" population does not fall at gravity, `mapgeom coverage`) | DERIVED |
| 90 | booster_air_control_raw | | GUESS |
| 91 | gear_raw | `(b − 1)/4` → gear; the byte takes 1+4k ONLY while the reactor is idle (§4) | VERIFIED (idle) |
| 93, 95, 97, 99 | fl/fr/rr/rl dirt | `b/255` | GUESS |
| 101 | wetness | `b/255` | DERIVED |
| 102 | sim_time_coef | `b/255` | DERIVED |

Quaternion from the 22-byte transform at 47: `q = (sin a·cos p·cos h,
sin a·cos p·sin h, sin a·sin p, cos a)` → `(x, y, z, w)`, unit norm to 1e-7;
velocity = `speed·(cos vp·cos vh, cos vp·sin vh, sin vp)` IS the world (x, y, z)
velocity; local +Z is the car's forward axis; yaw 0 = facing +Z, right-handed
about +Y. Floats are stored as `f32` because the file's are (an `f64` store made
a golden test pass in debug and fail in release on 17 of 45 runs, one digit
apart). `gbx::recwrite::write_transform` ROUNDS where the game TRUNCATES
(`cvttss2si`), which is invisible when re-encoding grid values and not when
writing fresh engine values.

Undecoded: the non-vehicle entities (§2.1), the notices, the Deltas2 blocks.

## 4. All 116 bytes — as the server's writer produces them **[EXE]**

Transcribed from `TrackmaniaServer_Latest` (30 113 288 B, `2026-05-15_18_00
git=128182`, md5 `0f0f4b25…`): the archiver at `0x9cfed0` emits an 85-byte
packed block (`0xaca280 → 0xac9e20 → 0xacb110 → 0xacb230 → 0xacb520`, with
64-bit read-modify-writes at `[rdi]`, `[rdi+8]`, `[rdi+0x10]`, `[rdi+0x12]`,
`[rdi+0x1a]` — why fields do not sit on tidy boundaries) then 31 bytes field by
field, each gated on the version. Offsets are into `CSceneVehicleVisState`
(`state`), `car+N` = `state + 0x50 + N`. Scored byte-for-byte with nothing to
tune by `fk carrier layout`: 38 bytes exact on every sample of both map2 keys.

| bytes | field | encoding | status |
|---|---|---|---|
| 0–1 | `FrontSpeed` | `u16 = (min(v,10000)+1000)/11000·65535`, 0 below −1000 | named |
| 2–3 | the lateral speed, `state+0x78` | `u16 = (min(v,1000)+1000)/2000·65535` | confirmed |
| 4–5 | rpm, `state+0x198` | `u16 = v/30000·65535` (exactly 65535/30000, no offset) | corrected |
| 6–13 | `Wheels[k].Rot` | `u16 = v/(2π·256)·65535`, and `0xcdcd` is bumped to `0xcdce` | confirmed |
| 14 | `InputSteer` | `(v+1)/2·255` | new |
| 15 | `InputGasPedal` | `v·255`, ZERO while `InputIsBraking` | new |
| 16–17 | — | literal zero word | dead |
| 18 | `InputGasPedal` | `v·255`, only while `InputIsBraking` — why `gas = b15/255 + b18/255` works | new |
| 19 | `state+0x228` | `(v+1)/2·255` | source slot DEAD in the server |
| 20 | `state+0x22c` | `(v+1)/2·255` | dead in the server |
| 21 | `TurboTime` | `v·255` | confirmed |
| 22 | `Wheels[0].SteerAngle` | `(v+π)/(2π)·255` (k = 255/(2π) = 40.5845; the frozen table's 40.743044 was the WHEEL constant borrowed onto it) | corrected |
| 23, 25, 27, 29 | `Wheels[k].DamperLength` | `(v+2)/4·255` | confirmed |
| 24, 26, 28, 30 | `Wheels[k]` ground material | the raw byte, or **13** when the wheel's flag bit 1 is set | corrected |
| 31 | `state+0x19c` and `IsTurbo` | bits 0–2 = enum & 7; bits 3–6 = 0; bit 7 = flag bit 24 (car+56) | decomposed |
| 32 | `Wheels[0]` | bits 0–5 = 0; bit 6 = `SlipCoef > 0.1`; bit 7 = wheel flag bit 2. **The chase camera reads this byte**: game recordings hold 128 on the ground (with 42 at byte 33); a zero put the camera under a ramp | new |
| 33 | `Wheels[1..3]`, `IsWheelsBurning` | six wheel bits, bit 6 = `state+0x1a0 > 0`, bit 7 = flag bit 5 | new |
| 34 | `state+0x224` | `v·255` — NOT the reactor (refuted on a 7-gate recording) | dead in the server |
| 35–38 | — | literal zero dword | dead |
| 39–40 | `state+0x244…0x260` | eight 2-bit codes: 0 / `<0.5` / `<0.99` / else | new |
| 41 | `state+0x264`, `state+0x84` | bits 0–1 a 2-bit code; 2–4 = 0; 5–7 = `round(v·7)` | new |
| 42 | `state+0x30c..0x310` | five bools, bits 5–7 = 0 | new |
| 43 | gas, `state+0x1bc`, `state+0x24`, `DiscontinuityCount` | 2 + 1 + 1 + 4 bits; the high nibble is a CLIENT counter (recordings hold 3, 10, 11, 13, 15; the server's is 1) | layout right, value client-side |
| 44 | `state+0x7c` | `v/5·255` | new |
| 45 | flag bit 12 | a bool | new |
| 46 | `state+0x1c0` | `v·255` | new |
| 47–58 | `Loc.translation` | three raw `f32` | confirmed |
| 59–64 | `Loc` rotation | quaternion from the 3×3 at `state+0x2c` (`Left/Up/Dir` columns; `0x1af5f40`), then `acos(qw)/π·65535` and two angles, TRUNCATED (`0x1ab3480`) | located |
| 65–68 | `WorldVel` | `i16 = 1000·ln‖v‖` truncated, then heading and pitch bytes (`0x1ab32d0`) | new |
| 69–72 | `state+0x68`, an unnamed vec3 | the same 4-byte pack | new |
| 73 | `state+0x1bc`, `state+0x8` | two nibbles: `(state[0x1bc] & 0xF) \| (state[0x8] << 4)`; every game recording holds `0x10` — `0x12` draws the car as a transparent WIREFRAME | new |
| 74 | `state+0x158` | `v/(2π)·255` | new |
| 75 | `state+0x8` | raw byte | new |
| 76 | flag bits 4,6,7,8,9,10,17 + `ReactorBoostType != 0` | eight bits; bit 5 = `IsTopContact` | decomposed |
| 77–80 | `state+0x338` | raw u32; bytes 79–80: the server holds `0x0ff0` where every recording holds 0 | new |
| 81–84 | `Wheels[k].Icing01` | `v·255` | confirmed |
| 85–88 | `state+0x15c` | raw `f32` | new |
| 89–92 | the reactor dword | bit 0 `IsGroundContact`; bit 1 `IsReactorGroundMode`; bit 2 `ReactorInputsX`; bits 3–4 `ReactorBoostType`; bits 5–6 `ReactorBoostLvl`; byte 90 bits 4–5 / 6–7 and byte 91 bits 0–1 = `ReactorAirControl` x/y/z as tri-states (0 negative, 1 zero, 2 positive); byte 91 bits 2–5 = `CurGear`; byte 92 always 0. Confirmed on `m203072_kb` (three boost gates): BoostType/BoostLvl/InputsX/AirControl.x 100.00 % | new |
| 93, 95, 97, 99 | `Wheels[k]+0x18` (dirt) | `v·255` | dead in the server |
| 94, 96, 98, 100 | `Wheels[k].TireWear01` | `v·255` | new |
| 101 | `WetnessValue01` | `v·255` | confirmed |
| 102 | `SimulationTimeCoef` | `v·255` | confirmed |
| 103–106 | `state+0x80` | raw `f32` | new |
| 107 | `state+0x344` | raw byte | new |
| 108–111 | `state+0x340` | `−2 − min(now − t, 3000)`: the COUNTDOWN, needs the race clock (`ghost record countdown`: −1 on the spawn segment, then that formula; reproduces the WR's 484 samples) | new |
| 112–115 | `state+0x348` | raw u32 | new |

Wheel records in the state: 44 bytes at `car + 88 + 44k`: DamperLen +0,
WheelRot +4, GroundContactMaterial +16, Icing01 +28 (memory
`tm2020-carrier-bytes.md`). Byte 89 could never be found by a per-byte affine
fit because it is bit 0 of a 32-bit field carrying six quantities and the gear.
**Byte 91 stops satisfying `4·gear + 1` the moment a reactor fires.**

## 5. Which bytes a regeneration writes (`gbx::sample`)

`SAMPLE_SIZE 116`; `TRANSFORM 47..69` (the transform encoder's);
`UNPREDICTED [59..65, 108..112]` (orientation words come from the transform
encoder, the countdown from the race clock); `DEAD_IN_SERVER [19, 20, 34, 93,
95, 97, 99]` (identically zero in the dedicated server whatever the car does —
the client computes them). A `--carrier layout` regeneration writes everything
else: `NOT WRITTEN: [19, 20, 34, 93, 95, 97, 99, 108, 109, 110, 111]`.
`NEUTRALISE` (49 per-run bytes the older transform-only path did not write) is
overwritten with 0 except byte 32 → 128 (`NEUTRAL_VALUE`).

## 6. The recorded input channel

`byte14 = floor((steer_i8 + 127)·255/254)`, `byte15 = 255 iff gas`, `byte18 =
255 iff brake` — measured against the corpus (a `round` misses steer 0 and 60;
the error took one kappa from 0.467 to 1.000). This makes the run's inputs
readable from the telemetry alone (`tmtraj intg echo`), and the agreement
between tape and record is the cheapest contamination test (kappa 1.000 honest,
0.120 on a `SEARCHTAPE_…_DO_NOT_PUBLISH`, 0.83 on a tape differing by a few
percent of ticks — so it catches wholesale contamination, not a small graft).

## 7. Record-level rules, all measured

* **One entity per life, tiling the recording**; take the entity with most
  samples and know it is an assumption (`ghost record show`).
* A zeroed slot (every position `(0,0,0)`) is not a missing entity: 186935's
  `BEST_793893`, 15 533 samples (`tmtraj check` C2).
* A record can be GROWN on its own grid (`tmtraj recgrow` 365 → 1125 slots;
  the grown container still validates) and REBUILT (`ghost record rebuild
  --span`: every sample a copy of one template, keeps the non-vehicle entities
  clipped to the span).
* The vehicle-only rebuild is what crashed the client on import (173691,
  285885); `graft-scene --from` a human recording of the map repairs it
  (`ghost-cgamectnghost.md` §8).
* The record's span words must not outlive the car (`ghost record shorten`).
* The floor of a regeneration against a game recording is **not** a
  client-vs-server physics floor: 0.48–0.52 mm was the distance between two
  copies of the car struct in the server's memory; reading the live-wheeled
  copy takes bit-identity from 0 to 227 of 455 samples (`tools/README.md`).
  Regenerated files are judged by `ghost regen`'s gate G1–G5 and `ghost
  verify` V1–V11, never by a two-run vote.
* Ground truth for a layout claim is a DOWNLOADED game recording regenerated
  from its own inputs (`ghost roundtrip`), with the class balance of every bit
  checked first (286279's contact bit is 77/23, not 99.95/0.05).

## 8. Not known

* Semantics of the non-vehicle entity classes (`0x0A019000`, `0x2F0CB000`,
  `0x032E3000`, `0x032AC000`, `0x2D001000`, `0x032CB000`), the notices, the
  Deltas2 payloads, `Ent.u01/u04` (u01 33554438 on the car).
* The unnamed state fields (`state+0x68`, `+0x78`, `+0x7c`, `+0x80`, `+0x8`,
  `+0x158`, `+0x15c`, `+0x1c0`, `+0x224…0x22c`, `+0x338…0x348`): located, not
  named.
* The client-only bytes (19, 20, 34, dirt, the high nibble of 43): the server
  cannot source them; only the client can.
