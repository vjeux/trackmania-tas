# Historical Physics Lab

An Openplanet Developer-mode plugin cataloguing official historical profiles. Every historical emulation is fail-closed until matched old-client behavior is certified; static handler or pack hashes are never promoted into fake user-facing “engines”.

For measured player-facing effects rather than implementation addresses, see [`BEHAVIOR_CHANGELOG.md`](BEHAVIOR_CHANGELOG.md).

## Profiles

| Stadium profile | Representative | Status |
|---|---|---|
| **January 2022** | Client build 105899 (2022-01-21); dynamic boundary anchor server build 112349 (2022-03-25) | Catalogued; disabled after independent audit found incomplete field/ABI remapping |
| **Spring 2022** | Dynamic server builds 112449–113135; no matching full client in the public archive | Catalogued; blocked on an external/private Mar. 29–Sep. 20 client |
| **Fall 2022** | Sep. 21 build shipped Sep. 30; Oct. 6 full profile is dynamically equivalent | Catalogued; native adapter fail-closed while exhaustive field-layout remapping is completed |
| **Summer 2023 / Current** | Earliest staged source build 121457 (2023-06-23) | **Selectable through the installed supported game**, no patch |

Snow, Rally, and Desert are separate vehicle families, not Stadium behavior epochs, and are therefore not presented as extra versions of the Stadium engine.

## Confirmed Stadium boundaries and catalog coverage

The player-facing changelog now includes two measured pre-2022 boundaries that are not yet implementable profiles:

- **Between July 23 and September 11, 2020:** 26 of 194 identical-input runs that finish on both builds change lap time; 19 move by at most `0.020 s`, two by more than `0.200 s`, and the largest change is `−1.954 s`. No 2020 full client is publicly archived, so the launch-era side cannot be transplanted faithfully.
- **June 8, 2021:** 92 of 139 common completions (66%) change lap time across 40 maps; median change `0.016 s`, largest `+1.488 s` on a `25.470 s` run. Stadium/shared/title packs are byte-identical, isolating this boundary to executable code. The only archived in-window client is July 8, 2021; closure/remap work has not started and remains fail-closed.

The original 2022+ catalog is retained below while those earlier targets remain research-only:

- The 2022-03-25 server rejects Roevhaal's 63.546 recording; the 2022-03-29 server reproduces it exactly. A 16-way executable/pack matrix makes that boundary follow the executable even though the two builds share the same normalized top-level CarSport handler.
- The public October 2022 ice/water update is already staged in the September 21 executable distributed by the September 30 archive. Full September 30 and October 6 clients produce the same deterministic HPLTRC3 canonical trajectory digest (`5ed96a35…04323`) and no checkpoints on Roevhaal. Their direct closures, measured scalars, tracked physics packs, and decrypted title-pack payloads are also equivalent. Therefore October 1 is a release date, **not** the executable boundary; the exact client boundary lies in the unavailable March 29–September 20 full-client archive gap.
- The Summer 2023 change is staged by June 23. Both the handler and tracked packs changed, so its causal split remains unresolved. No later Stadium force-law change is behaviorally confirmed.

The January profile is only a representative snapshot; the 2020–2021 audit disproved the old catch-all interpretation, and an independent native audit found its current adapter incomplete. It remains catalog-only.

## Observed code and data changes by version

This section records only deltas we actually localized. A changed hash is not treated as a physics change, and an official behavior change is not assigned to a byte range until the byte-level cause is identified.

### Stadium

| Version / boundary | Observed implementation delta |
|---|---|
| **January 2022 representative** | The historical root is `0x1405EDEB0..0x1405EFF40`, with a separate initializer at `0x1405EDE00..0x1405EDEAC`. It reaches a removed helper (`0x1405EDCF0..0x1405EDE00`), a removed averaging helper (`0x1405E7730..0x1405E77F2`), and nine legacy curve wrappers (`0x1413BE660..0x1413BEAD5`). Those wrappers use the old output-pointer interpolation ABI. Four initializer defaults also differ: `5.0`, `5.0`, `25.0`, and disabled. The first generated payload copies 14 regions (12,876 bytes), performs 161 declared field, 105 call, and 83 RIP relocations, adds one ABI adapter, and resolves 41 current-image targets. An independent audit then found 16 additional provably wrong live-object accesses, 10 unresolved accesses, two unresolved ABI risks, and no behavior gate. January is fail-closed until the exact January executable/disassembly completes the audit and matched trajectories pass. |
| **March 29, 2022** | The March 25 server returns `WRONG_SIMU` for Roevhaal while March 29–June 21 servers reproduce `63.546`, yet the March 25/29 builds share the same normalized top-level CarSport handler. The boundary therefore lives outside that normalized root. No complete client exists in the public archive for Mar. 29–Sep. 20, so Spring is intentionally catalog-only rather than borrowing the Fall-staged Sep. 21 closure. |
| **Fall 2022 staged build / public October update** | September 30 and October 6 are one historical build; October 1 is only a release date. V5 targeted code while a later complete Sep. 30 `VehicleTunings` transplant targeted data, but neither is certified. The clientbridge used by every matched client run wrote tape inputs to inherited car offsets that no build uses for input storage; actual axes are frame-local. All Fall client trajectory numbers—including metre-scale separation, one-ULP and first-divergence claims—are withdrawn. Static pack/cipher work remains valid; the mechanism is unresolved and Fall stays fail-closed. |
| **June 23, 2023** | The CarSport handler and tracked packs both change between May and June; June 23 and July 10 then match each other. The exact causal split between native code and data is not yet localized. Current behavior is supplied by the installed supported game. |

### Snow

| Version / boundary | Observed implementation delta |
|---|---|
| **November 2023 release** | Baseline Snow behavior. The November 15 staging build and November 24 release representative have the same measured handler and tracked physics packs. |
| **January 9, 2024** | Only `Trackmania.exe` and the updater manifest changed across the archived boundary; all 18 measured packs/resources are byte-identical. The three `SetPlayer_Delayed_` wrappers are structurally identical, so the bug is downstream. Their pre/post/current wrapper RVAs are `0x12DDA70/0x12DD540/0x13427C0` (Adherence), `0x12DDC10/0x12DD6E0/0x1342950` (Accel), and `0x12DDDB0/0x12DD880/0x1342AE0` (Control); helper RVAs are `0x12DD7A0/0x12DD270/0x1342450`. Historical compatibility suppresses only current call sites `0x1342927`, `0x1342AB7`, and `0x1342C47`, replacing their exact `call` preimages with `31 C0 90 90 90`. This reproduces the confirmed Snow no-effect behavior without falsely claiming the original downstream consumer was recovered. |
| **February 27, 2024** | `CInputDeviceDx8Pad::GatherLatestInputs` moves from January RVA `0x268E70` to February `0x269350` and target `0x2B8650`. February adds `cmp eax,0x18; je skip-first-queue`; the target branch at `0x2B8C4C` is `74 18`, and pre-February rollback is exactly `90 90`. This is input routing, not force law. The root `Vehicles\\Items\\Cars\\CarSnow.Item.Gbx` is byte-identical pre/post (1,900 bytes, SHA-256 `1f7b1bc0…`). The announced collision change is isolated to `Vehicles\\Cars\\CarSnow\\SnowCar.Shape.Gbx`, a `CPlugSurface`: pre-Feb is 1,123 bytes (`82a08222…`), first post-Feb is 1,147 bytes (`ef0ebee2…`), and current is 1,151 bytes (`7ea1385e…`). Its first three compound members changed from spheres with radii `1.195428014`, `0.969449997`, `1.119449973` to ellipsoids `[1.195,1.1,1.195]`, `[0.969,0.8,1.2]`, `[1.119,1.0,1.119]`; the four `0.47` wheel spheres are unchanged. The exact pre-Feb shape is bundled as a pre-launch override; unrelated tuning churn is excluded. |
| **May 22, 2024** | The real smooth-steering fix is in `Xi_GatherLatestInputs`: current RVA `0x2C360E` adds `F3 0F 11 64 8D 74` (`movss [rbp+rcx*4+0x74], xmm4`) to snap a converged stored input to its exact target. Pre-May behavior is exactly six NOP bytes at that site. An earlier DirectInput-dispatch candidate was disproved as equivalent control-flow factoring and is explicitly excluded. |

### Rally

| Version / boundary | Observed implementation delta |
|---|---|
| **February 27, 2024 release** | Baseline uses `CarRally.Item.Gbx` with `CGameItemModel.SkinDirNameCustom = Models\\RallyCar\\` and the pre-May smooth-steering path. |
| **April 2, 2024** | The encrypted Rally physics-model payload is bit-identical across the boundary. The only Rally resource delta is `CarRally.Item.Gbx`, changing a serialized model path from `Models\\RallyCar\\` to `Models\\CarRally\\`. Build 128130 exposes neither path through the reflected loaded-item strings, so release behavior uses the exact historical item as a pre-launch, hash-gated override rather than a guessed memory field; April/current use installed data. |
| **May 22, 2024** | Uses the same `Xi_GatherLatestInputs` six-byte snap-to-target store described above. Release and April profiles NOP it; current restores the exact original bytes. |

### Desert

| Version / boundary | Observed implementation delta |
|---|---|
| **May 22, 2024 release / current** | The closest archived payload is the April 30 “Desert update” staging build. No independently confirmed post-release driving or physics change exists through the 2026 changelog, so no additional Desert epochs are invented from later handler or pack churn. |

The machine-readable provenance and negative controls are in `profiles.json`, `payloads/manifest.tsv`, and `evidence/`.

### Pre-February Snow collision transaction

The retained Snow release/January implementation would patch the already-loaded `CPlugSurface` in memory, but direct installation is fail-closed pending matched old-client behavior. Its transaction requires the exact seven-shape compound, verifies the installed first three ellipsoids and four unchanged `0.47` wheel spheres, then changes only the first three type/radius fields to the historical spheres. It retains the exact current scales and restores them on profile switch or unload.

A live build-128130 control mutated all three ellipsoids to radii `1.195428014`, `0.969449997`, and `1.119449973`, read them back, restored the exact scales, and passed the full Snow release install/rollback self-test. The bundled 1,123-byte historical GBX remains evidence for geometry and verifier controls; it is not installed or redistributed through GameData at runtime.

## Official vehicle families (separate axis)

The plugin also discovers and authors maps for the four official current vehicle families: **Stadium, Snow, Rally, and Desert**. These are not additional Stadium epochs.

The selector enumerates the live `#10003` GlobalCatalog articles and writes the selected official article's model, author, and collection IDs into an open map, following the same mechanism validated by Editor++. Maps must be saved under a new name and contain compatible car gates.

## Official vehicle-family epochs

Snow, Rally, and Desert are separate official vehicle families. Their historical boundaries remain visible in the catalog, but only installed-current profiles are selectable until matched old-client trajectories certify each emulation:

| Family | Profile | Confirmed boundary |
|---|---|---|
| Snow | Release (Nov 2023) | baseline |
| Snow | January 2024 | delayed-player scripting functions fixed |
| Snow | February 2024 | hitbox/collision change plus action-key re-ranging |
| Snow | May 2024 / Current | analog smooth-steering 100% fix |
| Rally | Release (Feb 2024) | baseline, including action-key re-ranging |
| Rally | April 2024 | custom-ice behavior fixed |
| Rally | May 2024 / Current | analog smooth-steering 100% fix |
| Desert | May 2024 / Current | release baseline; no confirmed later behavior change |

The plugin does not infer additional epochs from executable or pack hashes. There are no era-matched Snow/Rally/Desert replay controls; official changelog statements establish the boundaries but do not behavior-certify an emulation. Every historical Snow and Rally entry is therefore catalog-only until matched old-client trajectories pass.

The three Rally implementations are retained as two independent, transactional research axes, but are not selectable in the release build:

- **Custom ice data:** the release and March 19 `CarRally.Item.Gbx` payloads are identical and contain `Models\\RallyCar\\`; the post-fix payload contains `Models\\CarRally\\`. The encrypted Rally physics-model payload itself is bit-identical across the April boundary. Because both reflected loaded-item strings are empty on build 128130, the plugin does not pretend to patch a loaded string. The bundled Rust helper installs the exact 3,056-byte release item before launch; the plugin then requires that exact live FID size. April/current require the exact supported-current 2,058-byte FID. Install/activation/analog rollback passed live on build 128130.
- **Shared analog input:** the staged May build adds one `Xi_GatherLatestInputs` store that snaps a converged smooth-steering value to its exact target. The two pre-May Rally profiles replace only the exact six-byte build-128130 store with NOPs and restore the original bytes transactionally. This is global input behavior while the profile is active, not Rally tire or surface physics.

`payloads/manifest.tsv` records the byte-level provenance. The release-item helper validates exact executable and payload hashes, performs atomic backup/write/readback, and refuses foreign files, but it is retained for research only while Rally release remains fail-closed; it is not part of the supported selector workflow.

No community-restored or hidden vehicle assets are supported or bundled.

## Exact supported target

The native historical previews currently support one executable:

- Trackmania build banner `2026-01-28_13_00`, git/build `128130`, GameVersion `3.3.0`
- executable SHA-256 `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda`
- unique CarSport handler RVA `0x851f00`

Both the build-banner bytes and the 41-byte CarSport entry must match before native activation.

## Safety model

- Openplanet **Developer** category; School Mode/offline use only.
- Never patches automatically.
- Experimental activation must be armed again on every process start.
- Switching is allowed only from the main menu with no playground or editor.
- Target bytes and every native write are checked.
- Rally release activation requires the exact 3,056-byte historical item FID; April/current require the supported 2,058-byte installed FID. The pre-launch helper owns, backs up, and restores the loose item by hash.
- Snow collision activation verifies the exact seven-child loaded compound, applies the sphere/ellipsoid geometry transaction in memory, and restores retained scales on switch/unload.
- Pre-May analog activation requires the exact six-byte build-128130 snap store, verifies the NOP write, and restores only bytes still owned by this plugin.
- Multi-axis switches are transactional: a failure restores every changed axis to its prior state.
- Unknown builds fail closed without changing memory.
- Unload restores only a jump still owned by this plugin; if another plugin changed it, the allocation is retained instead of being freed under a live reference.
- Process restart returns native code and Snow collision memory to stock. The Rally release loose item persists until `rally_item_overlay restore` removes the plugin-owned file, after which another restart returns installed Rally data.

## Using the selector

Open **Historical Physics Lab** in the Openplanet menu:

1. Return to the main menu.
2. Open **Choose engine profile**.
3. Select one of the current installed-game profiles. Historical profiles remain visible for evidence and status but are not executable until certified.
4. Use offline maps only.
5. Catalog-only entries explain the missing certification rather than silently substituting another profile.

## Fall 2022 native payload (disabled pending behavior certification)

- 9,916 native bytes
- 203 originally generated structure-field rewrites; audit is rebuilding this manifest exhaustively
- 155 rel32 relocations
- 40 absolute current-image targets
- current runtime tuning preimage: 28 named entries / active index 27
- Fall runtime tuning view: first 25 measured entries / active index 24; behaviorally identical to stock current on the booster control
- one measured ABI adapter
- zero unresolved direct calls or RIP-relative references
- V5 field/ABI remap: crash-free for map load, 10-second throttle gate, and full 22-second trajectory
- matched-control verdict: V5 is crash-free but farther from exact Sep. 30 than stock current; certification bit remains false

WhiteStick live testing first exposed a second-tick native crash, then a full audit corrected stale field mappings and adapted a current helper’s added output-buffer argument. V5 survives a full 22-second run with no native fault. The matched historical control nevertheless rejects it: V5 diverges from exact Sep. 30 earlier and with greater mean/max error than unmodified current stock, while V5 remains much closer to stock. This is a measured behavioral mismatch, so `PROFILE_FALL2022_BEHAVIOR_CERTIFIED` remains false and the release exposes no Fall installer path.

## Live integration controls

On exact build 128130, automated main-menu tests passed all of these transitions with readback and rollback verification:

- current preimages → January native island → current;
- current → Rally April/pre-May analog → current;
- release Rally item override + release/pre-May analog → analog restore;
- current → Snow February/pre-May analog → current;
- current Snow ellipsoids → release Snow spheres → exact ellipsoid restore.

These are diagnostic transaction tests only. They do not establish semantic equivalence, and every historical profile remains fail-closed until matched old-client trajectories and positive controls pass.

## Verification vocabulary

Every claim names its verification level:

1. **Static verified** — exact source/target hashes; complete relocation or patch manifest; expected-byte preimages; no unresolved references; independent verifier pass.
2. **Live integration verified** — the plugin compiles on exact build 128130, installs, reads back every change, executes without fault, and restores the original state.
3. **Behavior certified** — a deterministic input tape reproduces the reference trajectory and every checkpoint exactly in two fresh processes, while an adjacent negative control fails the same frozen gates.

“Verified” without a qualifier never means behavior-certified. Claims are qualified individually; transaction tests do not upgrade a profile’s behavior status. **No historical profile is behavior-certified or selectable yet.**

## Certification boundary

“Runs without crashing” proves native integration, not exact historical semantics. The HPLTRC3 harness itself now passes a fresh-process identity control: two September 30 runs have identical canonical digest `5ed96a35f518a6a98c00481f25aaa5b973fcaa8fbb7955dc0d1a2e93f8904323`, and a hardware watch proves the injected steering field is consumed at RVA `0x6E2920`.

That full client is nevertheless a measured **negative** against Roevhaal: it exceeds 5 mm at `4.010`, 0.1 m at `4.460`, 1 m at `5.460`, 10 m at `9.910`, and records no checkpoints. The complete October 6 profile produces the same canonical trajectory and no checkpoints. Neither is a Spring positive.

The required positive remains exact client re-simulation of:

**13.492 / 31.143 / 42.452 / 59.582 / 63.546**

The public full-client archive has no snapshot in the exact `2022-03-29..2022-09-20` interval. An exhaustive public search confirmed that the sole Internet Archive TM2020 client collection contains 32 builds and jumps directly from Jan. 21 to Sep. 30; no second client collection appeared among 739 Trackmania archive items. Steam cannot contain a 2022 copy because Trackmania launched there on Feb. 2, 2023. Ubisoft/Wayback, GitHub, Reddit, community indexes, and public torrent metadata were also negative. The most plausible remaining sources are private backups held by the archive curator or Openplanet developers; no one has been contacted. Certification therefore requires an external/private April–August 2022 full-client backup. This is an archive-coverage gap, not a claim that no matching client existed. Imported ghost playback is not accepted because it may display recorded samples rather than re-simulate inputs.

`profiles.json` contains the machine-readable source builds, hashes, evidence labels, and implementation status for all 12 confirmed official profiles.
