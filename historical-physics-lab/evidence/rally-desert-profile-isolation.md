# RallyCar and DesertCar profile isolation

## Behavioral boundaries

The profile catalog follows official behavior evidence, not executable or pack hash churn:

| Profile | Official range | Custom-ice axis | Analog-input axis |
|---|---|---|---|
| Rally release | 2024-02-27 through 2024-04-01 | release | pre-May 22 |
| Rally post custom-ice fix | 2024-04-02 through 2024-05-21 | corrected | pre-May 22 |
| Rally May 2024 / current | 2024-05-22 onward | corrected | current |
| Desert release / current | 2024-05-22 onward | not applicable | current release baseline |

Nadeo's April 2 release note confirms the Rally custom-ice boundary. Nadeo's May 22 release note confirms the global smooth-steering input boundary. No later Desert driving-behavior change is confirmed through the checked 2026 changelog.

## Custom-ice data isolation

The `Maniaplanet_ModelsSport.pak` private header key was recovered independently from each representative executable as `9A93723447347A8CE336CCFC49E65449`. The Rally-specific file table was extracted from the February 26, March 19, and April 30 snapshots.

- February 26 and March 19 `CarRally.Item.Gbx` are byte-identical: SHA-256 `a1d5cdcd21ed4b152ae18b9f94dd8fa4f3eb4375d0035a83c20923a251bccd9a`.
- April 30 changes the item payload to SHA-256 `7cf6976abe68c8910d2cda6b504aec4c40ec90fc1561e27446b97313c9c80868`.
- The semantic item delta is `SkinDirNameCustom`: `Models\RallyCar\` becomes `Models\CarRally\`.
- The encrypted Rally physics-model entry `B191D2177AEA3017BE17C0D1694BF68515` is bit-identical on March 19 and April 30: stored size 5,696, SHA-256 `a0a4c1fe0e771ca75053badd1b0b78de0dfe0f98fedcd2cb73d583020700734e`.

The identical physics-model ciphertext is a negative control: the April profile does not claim a changed tire, engine, or force-law blob. The official note supplies the behavior evidence; the item-path delta identifies the delivered Rally-specific data change.

At runtime the plugin resolves the one official `CarRally` GlobalCatalog article, preloads its `CollectorFid`, obtains reflected `CGameItemModel.SkinDirNameCustom`, and requires the exact current `Models\CarRally\` preimage before writing the release value. It retains the original string and restores it only while the historical value is still owned.

## Shared analog-input isolation

The rejected candidate at current RVA `0x2b8f2a` only reflects equivalent input-event dispatch factoring and is not shipped.

The actual May boundary is in `Xi_GatherLatestInputs`:

- Pre-fix convergence branch at 2024-03-19 VA `0x14027429d` exits once the smoothed/raw delta is within epsilon, leaving the stored value short of its exact target.
- The staged May build adds `movss [rbp+rcx*4+0x74], xmm4` at 2024-04-30 VA `0x1402747ce`, snapping the stored value to the exact target before exit.
- The same six-byte instruction is present on build 128130 at RVA `0x2c360e`: `F3 0F 11 64 8D 74`.

The two pre-May Rally profiles replace only those six bytes with NOPs after matching the full surrounding signature. Rally current and Desert current retain the installed instruction. This patch is explicitly a shared input mapping behavior, not custom-ice physics.

## Safety and controls

- Exact target: build 128130, executable SHA-256 `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda`.
- Activation requires the existing per-process experimental arm and a main-menu state with no playground or editor.
- Both axes have exact preimage checks, post-write readback, ownership checks, and rollback.
- Multi-axis switches restore the previous axis states if either write fails.
- The runtime self-check verifies the loaded Rally path and analog patch bytes without changing them.
- `verify_profiles.rs` checks all four representative executable hashes, the old/post/current convergence bytes, both item payload hashes and paths, the unchanged encrypted physics-model control, and the source transaction guards.
- There are no era-matched Rally or Desert replay controls. These profiles are statically isolated and remain experimental until live runtime controls and behavior-specific map tests pass.
