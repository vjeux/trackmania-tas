# SnowCar static localization for build 128130

Target executable: Trackmania build 128130 (`2026-01-28_13_00`), SHA-256 `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda`.

## 2024-01-09: delayed-player scripting

The script-facing wrappers and their common enqueue helper are structurally identical across the last staged pre-fix client and first staged post-fix client. Therefore the official Snow fix is downstream of script registration/enqueue; wrapper churn is not treated as the fix.

| Function | 2023-12-21 RVA | 2024-01-10 RVA | build-128130 RVA |
|---|---:|---:|---:|
| `SetPlayer_Delayed_AdherenceCoef` | `0x12DDA70` | `0x12DD540` | `0x13427C0` |
| `SetPlayer_Delayed_AccelCoef` | `0x12DDC10` | `0x12DD6E0` | `0x1342950` |
| `SetPlayer_Delayed_ControlCoef` | `0x12DDDB0` | `0x12DD880` | `0x1342AE0` |
| common delayed-event enqueue helper | `0x12DD7A0` | `0x12DD270` | `0x1342450` |

The three target wrappers enqueue event discriminants `0x0B`, `0x0D`, and `0x0C` respectively. The compatibility payload reproduces the confirmed release-era Snow no-effect at the narrowest verified target boundary: it replaces only each wrapper's call to the common helper with `xor eax,eax` and padding.

| Call site | Current preimage | Release replacement |
|---:|---|---|
| `0x1342927` adherence | `E8 24 FB FF FF` | `31 C0 90 90 90` |
| `0x1342AB7` acceleration | `E8 94 F9 FF FF` | `31 C0 90 90 90` |
| `0x1342C47` control | `E8 04 F8 FF FF` | `31 C0 90 90 90` |

This is an API-path compatibility shim, not a claim that those bytes existed in the historical executable and not a Snow force-law patch.

## 2024-02-27: analog action-key routing

`CInputDeviceDx8Pad::GatherLatestInputs` maps as follows:

- 2024-01-10: `0x268E70..0x269F38`
- 2024-02-26: `0x269350..0x26A418`
- 2024-03-19: `0x269300..0x26A3C8`
- build 128130: `0x2B8650..0x2B9708`

The February build introduces an event-type `0x18` discriminator before the first action queue:

```text
cmp eax, 0x18
je  skip_first_queue
```

The target retains this as `83 F8 18 74 18` at RVA `0x2B8C49`; the two-byte conditional branch is at RVA `0x2B8C4C`. Replacing `74 18` with `90 90` exactly restores the pre-Feb route while preserving the second queue and every other event type.

This is an input-path change, not a vehicle force-law change.

## 2024-02-27: Snow collision data

Offline NadeoPak v18 decryption identified the root item at `Vehicles\Items\Cars\CarSnow.Item.Gbx`. The January and February root files are byte-identical (1,900 bytes, SHA-256 `1f7b1bc03a67d7cfde6917857f81f6e4ef9046385cbe3dc9bccf093a0e65e64c`), so the root is a negative control, not the hitbox payload. Its reference table names `PhyModelSnow.VehiclePhyModel.Gbx`, which in turn names `SnowCar.Shape.Gbx`.

The minimal collision payload is `Vehicles\Cars\CarSnow\SnowCar.Shape.Gbx`, packed as hash entry `11509106C787A37122602217B5C84AAEA8`, class `0x0900C000` (`CPlugSurface`). It changed from 1,123 bytes / SHA-256 `82a0822220468e50f78b372c840fe2c01fee9cc017a712b3f926548668841661` to 1,147 bytes / SHA-256 `ef0ebee29e98faec02c5e563c99688fb664b9b76314eee20c77aef8c9c048d9d` at the February boundary. The current supported client carries a 1,151-byte descendant (SHA-256 `7ea1385e37ecaa3005939bd1d38608f5a36e276da4f81d3efc9079cfa13a68cb`).

A structure-aware Rust decoder shows the exact geometry change. The first three members of the seven-member compound changed from spheres with radii `1.195428014`, `0.969449997`, and `1.119449973` to ellipsoids with axes `[1.195, 1.1, 1.195]`, `[0.969, 0.8, 1.2]`, and `[1.119, 1.0, 1.119]`. The four radius-`0.47` wheel spheres are unchanged. This directly identifies collision geometry rather than inferring physics from pack hashes.

Other CarSnow children, including `TuningsSnow.VehicleTunings.Gbx`, also changed across the pack boundary. They are excluded from the collision payload because pack churn is not evidence of a Snow force-law change. `tools/snow_collision_overlay.rs` installs only the historical `SnowCar.Shape.Gbx` as a pre-launch loose override, with exact target/payload checks, backup, atomic replacement, readback, and restore.

## 2024-05-22: analog smooth-steering 100%

`Xi_GatherLatestInputs` maps as follows:

- 2024-03-19: `0x273E00..0x2745EC`
- 2024-04-30: `0x274350..0x274B1C`
- build 128130: `0x2C3190..0x2C395C`

Post-May code snaps the stored analog value to the target after the threshold test:

```text
RVA 0x2C360E: F3 0F 11 64 8D 74
                  movss [rbp+rcx*4+0x74], xmm4
```

Pre-May flow has no corresponding store. Six NOPs at `0x2C360E` restore the pre-May behavior while the following jump preserves the old flow.

A nearby apparent change in `CInputDeviceDx8Pad::GatherLatestInputs` was rejected: pre-May directly calls a tiny argument-swapping wrapper that tail-jumps the dispatcher, while post-May joins an equivalent common block that calls the same dispatcher once with the same effective arguments.

## Controls

`tools/verify_snow_payload.rs` checks:

1. exact target executable SHA-256;
2. all five target preimages;
3. January lacks, while February/current contain, the event-`0x18` action-routing branch;
4. March lacks, while April/current contain, the Xi snap store;
5. successful February and Release transaction plans;
6. injected multi-site failure restores every exact backup in reverse order;
7. one-bit target corruption is rejected;
8. Release/January require the 1,123-byte historical Shape FID, while February/current require the 1,151-byte installed Shape FID.
