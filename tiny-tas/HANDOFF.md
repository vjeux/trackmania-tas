# HANDOFF — tiny-tas wave 0 (Summer 2026 - 01 tiny), stopped 2026-09-06 17:54 PDT on vjeux's order

Coordinator session `5bfa1d26-99a7-4f59-9a1f-ef3d5640809b`. Parent: `5ef7ca0a-06ac-43ee-bd28-21e64329f79f`.
Box 0: **`devvm48849.odn0.facebook.com`** (od lease `e43031cf-2a62-4acc-bab1-73ba3af10d4c`, envspec www, 96 cores,
expires 2026-09-07 17:45 UTC) — NOT released; handed to the parent. Working tree `/tmp/tiny-tas/`.

## What was done (1h07 of box time)

1. **Bootstrap** of a www box: rust 1.85 via rustup through fwdproxy:8080 (cargo fetched crates.io through the
   same proxy — no stunnel needed on this box); all four workspaces build (`tools`, `tools/search`, `tools/fk`,
   and `tools/tinytas` new); `tools/search` did **not compile at main** (f32/f64 after gbx `9e52cca`) — fixed.
   `grep 'const FINISH_BASE'` → none. Search suite: every crate green (150+ checks, oracle_e2e on the engine).
2. **G0 PASS** — the dedicated server loads the all-embedded-items tiny map and returns verdicts; the live engine
   shows the car on the scaled decks (12.5 / 8.5 m), the turbo pad accelerating 27→66 m/s, the curve border colliding.
3. **G1 MEASURED + codified** — item-start validator index = position among item waypoints (parked converter
   blocks are not in the engine's array; u03 5..8 = no car). `tmauto::synth::{spawn_waypoint, is_parked_waypoint,
   item_start_index}` + unit test; `tmauto::oracle::evaluate` and `tmexplore-real template` fixed to use it.
4. **Stage C** — `tinytas scale-ref`: scaled pack+route; transform control 5/5 gates at 0.000 m.
5. **Stage D** — first certified Finish **31.769** (explorer, 51 s of search); reproduced 3× by the plain oracle
   from a container written from tick 0 (`tmauto synth write --tape --declared 31769`, IsValid true).
6. **Stage G** — `tinytas authorghost embed`: the VALIDATED MAP `out/Summer-01-Tiny-validated.Map.Gbx`
   (md5 `7f5ff34a64a5fc30fe80ea97394fc7f9`), AT 31.769, medals 35/39/48 s, validated="1"; proof: the ghost
   re-extracted from the output map re-simulates to 31.769 on the output map. Reader control: Nadeo's author
   ghost out of the original → 23.144 (= its AT).

Details, every number beside its control: `FINDINGS.md`. Commands per stage: `PIPELINE.md`.

## Code

Branch **`agentcloud/tiny-tas`** (from `agentcloud/tiny-campaign` @ `7f59351`), pushed to the devvm42752 mirror
`/home/vjeux/trackmania-tas`; bundle `tiny-tas.bundle` in this directory (`git bundle verify`; contains
`agentcloud/tiny-campaign..agentcloud/tiny-tas`). NOT on GitHub (no credential on the box).
- `tools/tmauto/src/synth.rs` — spawn resolver / item start rule / item `initial_state_for_map`
- `tools/tmauto/src/main.rs` — `tmauto verdict` (plain-oracle CLI; was in help, unimplemented), `--validation-u03` bypass
- `tools/tmauto/src/oracle.rs` — `evaluate` uses `complete_meta_for_map`
- `tools/tinytas/` — new crate: `scale-ref`, `authorghost probe|extract|embed|dumphead`, `tape assemble`
- `tools/search/tmsearch/src/{refline,seedstate}.rs`, `tests/seed_state.rs` — f32 casts
- `tools/search/tmexplore-engine/src/main.rs` — `state --csv DIR --tape-tsv`, `--boundary-margin`, template start index
- `tiny-tas/{FINDINGS,PIPELINE,HANDOFF}.md`

## Bank (this directory, `~/persistent/private-30d/tm-tiny-tas/wave0/`)

`MANIFEST.md5` lists everything. Key files: `in/` (frozen tiny map md5 `1b9318c8e1507295485bc9db2fc0fd26`,
original, lib.zip, placements, recipe), `ref/tiny/` (scaled pack+route), `ex1/run.log` + `ex1/work/best.tape.tsv`
+ `ex1/template.Ghost.Gbx` (the seed search), `cert/` (full.tsv, run.declared.Ghost.Gbx, reextracted), `out/`
(the validated map), `g1/` (the u03 sweep + `state-sweep.txt` + traces), `ag/` (extracted author ghosts),
`PLAN.md` (the parent's plan), build/test logs.

## Unfinished

- Client load of the validated map (render box) — never attempted.
- The embedded ghost's record is the minimal first-sample telemetry: validates, not watchable. `fk regen`/`ghost regen`.
- Second-box re-simulation of 31.769 (certification proper); G2 with a checkpoint-reaching reference.
- Physics parity probes, feature census, `--must` route hypotheses, tape polish, `ARM.md`, GitHub push.

## Resume (on any www box)

Follow `PIPELINE.md` §0 (bootstrap), then §G with `cert/full.tsv` to rebuild the container and the validated map
from the bank. Re-simulate first: `tmauto verdict cert/run.declared.Ghost.Gbx --map in/Summer-01-Tiny.Map.Gbx` → 31.769.
