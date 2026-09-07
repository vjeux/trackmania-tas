# Tiny Summer 2026 — TAS pipeline FINDINGS (wave 0: *Summer 2026 - 01* tiny)

Every measurement beside its control. Times in seconds with a decimal. Maps by
name. Box 0 = `devvm48849.odn0.facebook.com` (96 cores, envspec www), lease
from 2026-09-06 16:47 PDT.

## Inputs (frozen copies on box 0, `/tmp/tiny-tas/in/`)

| file | md5 | note |
|---|---|---|
| `Summer-01-Tiny.Map.Gbx` | `1b9318c8e1507295485bc9db2fc0fd26` | build of 2026-09-06 16:43 from `/tmp/tiny3` on devvm42752; still being refined by the visual-QA session — the pipeline must be re-runnable on a newer file |
| `Summer-2026-01.Map.Gbx` (original) | `1563b24baad901364e4f86ce76b3f8f6` | uid `buNzfsVlp2NF2oWtHM3729dEylg`, AT 23.144, human WR 19.538 |
| `lib.zip` | `c2c9c2289783e8f6f0e0e3969affebda` | the tiny item library |
| `placements.tsv` | `da53cda235cf922b8a7deb8790a56f99` | |
| `recipe.env` | `ae294b53791a69c9fb51f42c94b3575e` | |
| repo | `agentcloud/tiny-campaign` @ `7f59351` → branch `agentcloud/tiny-tas` | |

Tiny map header: uid `Tin2buNzfsVlp2NF2oWtHM3729d`, `validated="1"`,
`authortime="23144"` (inherited from the original — to be replaced by our
ghost's millisecond in stage G), envir BlueBay, 2430 blocks (all parked or
foundation) + 4376 items.

## Toolchain (box 0)

- rustc 1.85.0 (rustup, via fwdproxy:8080 — cargo fetches crates.io through
  the same proxy on this box; the 407 note in the brief did not reproduce).
- `grep FINISH_BASE tools/search tools/fk tools/tmauto` → only doc comments
  naming the retired constant; **no `const FINISH_BASE`** anywhere. PASS.
- `tools/search` did **not compile** at `7f59351`/`main`: `gbx` commit
  `9e52cca` (2026-09-01) made `Sample` floats f32 and `tmsearch`
  (`refline.rs`, `seedstate.rs`) still expected f64. Fixed with explicit
  casts; `tmsearch`, `libforkshim.so`, `tmexplore-real` build.
- `tmauto verdict` was in the help text and unimplemented; implemented
  (plain oracle, one launch per batch, prints verdict + the server's words).

## G0 — does the dedicated server load the all-items tiny map? **PASS**

| probe | result | control |
|---|---|---|
| `tmauto synth probe` (600-tick tape) on the tiny map | server read the file, `Inputs "600C"`, `wrong simu`, DNF cps 0 — a VERDICT, not `Can't load map` | same probe shape on the original map gives the same class of answer |
| live engine (`tmexplore-real state`, fork at clock 30000 → boundary tick 76) with u03=3 | car at (1584.0, 12.51, 788.3) doing 11.7 m/s +z on a deck at **y 12.5** = the scaled start deck (original 18.0 → 11.5 + 0.5·(18−16) = 12.5) | u03=0 puts the car on the CP0 item deck at **y 8.51** = scaled 10.0 deck; both decks are where the transform puts them |
| straight-run trace (`g1/trace3_30000/straight.csv`) | z 792→808: descent 12.5→8.5 (the scaled `RoadTechOnLandHillSlopeBase` item); z 808→844: **27 → 66 m/s in 1.0 s** (the `RoadTechSpecialTurbo` item fires); z≈848: hard contact at x=1584 (the outer border of the scaled `RoadTechCurve4`), car slides along the border and exits at z≈885 heading −x | original map, same tape: at 1.52 s the car is at (1584.0, 17.96, 801.6) on the 18.0 deck, still before its slope (z 800) — same shape, doubled |

So: the server loads the map, the items carry collision at the right heights,
the turbo pad has gameplay, the curve border collides. Not yet measured: per-
block-family parity of the *magnitudes* (turbo gain, slope, grip) — stage 5.

## G1 — validator checkpoint order for ITEM starts. **MEASURED, codified**

Sweep of `--validation-u03` 0..8 (pure-throttle tape, 60-tick wobble prefix
so the input locator has a key; live engine read at the fork boundary):

| u03 | car at the boundary (x, y, z), heading | = which waypoint |
|---|---|---|
| 0 | (1387.8, 8.51, 1056.0) +x | item#1056 `AI00000001` CP at (1369, 8.5, 1056) |
| 1 | (1472.0, 8.51, 927.3) −z | item#2321 `AC00000086` CP at (1480, 7.5, 952) |
| 2 | (1472.0, 8.54, 724.9) −z | item#2374 `AC00000092` **Goal** at (1480, 7.5, 744) |
| **3** | **(1584.0, 12.51, 788.3) +z** | **item#2385 `AC00000096` Spawn at (1576, 11.5, 776)** |
| 4 | (1391.3, 8.51, 880.0) −x | item#2387 `AC00000086` CP at (1416, 7.5, 872) |
| 5..8 | **no car** — "validator-owned CGameVehiclePhy state failed the structural trajectory check" | the array has exactly 5 entries |

Rule: **items in item order; the four converter-parked waypoint blocks
(renamed `RoadTechStraight`, cell (0,0,0), tags kept in the file) are NOT in
the engine's array.** Codified in `tmauto::synth::item_start_index` (index =
live non-Spawn block gates + live block spawns + position among item
waypoints) with `spawn_waypoint`/`is_parked_waypoint`; unit test
`item_start_index_is_position_among_items_after_live_blocks`. Also fixed:
`tmauto::oracle::evaluate` built its containers with `meta_for_map` (u03 = 0
= the wrong-start defect) — now `complete_meta_for_map`.
Second-map confirmation owed when map 02 is converted (the rule is the same
"blocks first, then items" the block rule already encodes; on this map the
block half is vacuous).

Spawn transform inside the start item: item pos + (8, 1, 8) → (1584, 12.5, 784),
heading +z (identity quaternion at the boundary). The car's first sample at
the earliest boundary reached so far (tick 76, clock 30000) is 4.26 m along +z
from that point at 11.7 m/s — consistent with a standing start there.

## Open / next

- G2: start-position control at tick 0 (fork earlier than clock 30000),
  identity control, fork-server calibration (51 forks + 51 full validations),
  100-candidate agreement at two checkpoints.
- `fk trace` (the pointer-chain locator `mod+0x1d56e48`) resolves a STACK
  address on a synthesized container and fails its self-check; the
  validator-owned locator in `tmexplore-engine::fork::ForkBranch` works. Use
  the latter (`tmexplore-real state --csv DIR`, added) for traces.
- `tmexplore-real state` needs a `--route`; the original's cartographer route
  was used as a placeholder for the locator bounds. The scaled route is stage 6.
