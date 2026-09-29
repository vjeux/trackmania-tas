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

## G2 (partial) — fork server on the tiny map

`fk server check --tape g1/w03_3.Ghost.Gbx --map tiny --at tick:100 --n 50`:
identity resume EXACT (DNF = DNF), boundary tick 194 (probe 194), oracle
repeatability 0/50 differ, exactness **50/50** (all DNF — a weak control until a
reference that reaches checkpoints is used; the 31.769 tape below is that
reference; not yet run against it).

## Stage C — scaled reference set. **TRANSFORM CONTROL PASS**

`tinytas scale-ref` with anchor (1584,16,784)→(1584,11.5,784), k 0.5: every
scaled ORIGINAL gate centre lands on a tiny waypoint item centre (origin +
R(yaw)·(8,·,8), yaw convention +1) at **0.000 m** — Spawn, 3 Checkpoints (one
an item gate, kept as an item), Goal. Output `ref/tiny/Tin2buNz….{pack,route}.json`
(route 950.1 m, gates at s = 255.8 / 557.3 / 741.6 / 950.1).

## Stage D — seed: FIRST CERTIFIED FINISH **31.769** (2026-09-06 17:53 PDT)

`tmexplore-real run` (the ghost-free archive explorer) on the tiny map with
the scaled pack+route, 80 threads, fork boundary tick 77, 25 min:
- its own two-sided identity control PASS (do-nothing vs full-throttle tapes
  decode to different echoes; map uid confirmed); start-position control
  PASS on every worker (first sample 4.6 m from the spawn cell base at 0.77 s);
- first plain-oracle **FINISH 31.769 after 51 s / 29 107 evals**; 413 486
  evals in 1517 s; the archive's "best" ordering later reported 33.989 while
  the confirmed best stayed 31.769 (`best.tape.tsv`, 2553 search ticks, frame 77).
- Route-ladder note: the fork's gate ladder collects a gate within 8 m of its
  centre; the item gate `AI00000001` is a 32 m-wide gate kept unscaled, so
  the ladder saturated at station ~57 while the engine counted cps 3 — the
  "FINISH at station 68" is that saturation, not a physics anomaly.

Certification chain (all on box 0; a second box has NOT re-simulated it yet):
| step | result |
|---|---|
| `tinytas tape assemble` (template[0..77] + 2553 search ticks + template tail to 6000) | 6000-tick whole-file tape, md5 `3473fbd8c0f10765f48a571a4301b353` |
| `tmauto synth write --tape full.tsv --declared 31769` (fresh container from tick 0) | `cert/run.declared.Ghost.Gbx` md5 `8fb2ecf50e4bac8885db98370efbbddb` |
| `tmauto verdict` ×3 (three server processes) | **31.769, 4 cps, IsValid true** every time |
| control: the same search tape WITHOUT the template tail (last input repeated) | **DNF cps 3** — the finish depends on the template's wobble inputs after tick 2630; `tape assemble` now always appends the tail |
| control: `ghost declare --from-oracle` alone leaves the walltime pair at 60 s → server says 31.769 but "unexcepted walltime (60s)" (IsValid false) | use `synth write --declared MS` (moves the walltime pair) |

## Stage G — the VALIDATED MAP writer works end to end

`tinytas authorghost embed --map in/Summer-01-Tiny.Map.Gbx --ghost cert/run.declared.Ghost.Gbx`:
- skeleton = the map's existing embedded Nadeo ghost (13 148-byte 0x0305B00F
  payload: `u32 0 | u32 len | 0x03092000 | chunks | FACADE01`); ours replaces
  0x03092000 (record node-index word dropped), 005, 00F, 010, 014, 01D, 02B,
  02D; skeleton keeps the rest with 0x0309201B redeclared;
- Id literals `["CarSport","Nadeo",<uid>]` → same count/order, uid slot only;
- times [35000,28000,25000,23144] → author **31.769**, gold 35.000, silver
  39.000, bronze 48.000 (1.08/1.20/1.50 ceil-to-second; calibrated: 23.144 →
  25/28/35 s exactly), in header 0x03043002 (×1), header XML, body
  0x0305B004 + 0x0305B00A (×2); `validated="1"`;
- **proof**: `tinytas authorghost extract` from the OUTPUT map → the dedicated
  server, loading the OUTPUT map, re-simulates it to **31.769, IsValid true**.
- Reader control: Nadeo's own author ghost extracted from the ORIGINAL map
  re-simulates to **23.144** (= the AT); the same ghost against the tiny map
  DNFs cps 0 (the maps differ).

| file | md5 |
|---|---|
| `in/Summer-01-Tiny.Map.Gbx` (frozen input) | `1b9318c8e1507295485bc9db2fc0fd26` |
| `out/Summer-01-Tiny-validated.Map.Gbx` | `7f5ff34a64a5fc30fe80ea97394fc7f9` |
| `cert/run.declared.Ghost.Gbx` | `8fb2ecf50e4bac8885db98370efbbddb` |

Unfinished (stop order 2026-09-06 17:54 PDT): client load of the validated
map; regenerated telemetry for the embedded ghost (it carries the minimal
first-sample record, so it validates but is not a watchable car — `fk regen`
/ `ghost regen` is the tool); second-box re-simulation; parity probes;
`--must` route hypotheses; tape polish; ARM.md.
