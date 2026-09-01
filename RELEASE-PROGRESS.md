# Release progress

Durable state for the toolchain release. **Update this file in the same commit
as the work it describes** — a session that loses its context reads this and
knows exactly where it is.

Plan of record: `RELEASE-REVIEW.md` (24 crates surveyed, 10 API findings,
4-tier test plan).

## Phase 0 — preflight

- [x] **Working tree verified against repo HEAD.** 361 files compared by md5.
- [x] **HEAD DID NOT COMPILE — fixed.** `fk/fk/src/cmd/carrier.rs` still used
      `Anchors.pos_delta`, deleted when the pointer work replaced it with
      `chain`. The fix existed only in a build tree and had never shipped.

## Phase 1 — plumbing

- [x] **R2 `--version`** — one workspace version, inherited by all 23 crates;
      every binary prints `<binary> <version> (<stamp>)` from `CARGO_BIN_NAME`
      (not the crate name: `haul` ships `tmhaul`, `clip` also ships
      `playtest`). Compile-time only, no build.rs, no dependency.
- [x] **R1 `--help`** — was missing from 14 of 23 binaries, three of which
      *panicked* with no arguments. Then found broken again one level down:
      6 of 20 `tmtraj` SUBCOMMANDS failed in four different ways. Fixed in
      `cli::finish` (covers 14 at once), `corpuscmd`, `checkcmd`, `intgcmd`,
      `manifest`, and a post-dispatch fallback in `run()`.
      **Contract: asking for help is exit 0; forgetting arguments is exit 2.**
- [x] **R3 exit codes** — `0 ok / 1 answered NO / 2 usage / 3 environment`,
      documented at `cli::die`, with `cli::refuse()` for the verdict path.
- [x] **R4 machine-readable output** — `ghost verify -o json`, and `clip`'s
      audit converted off prose-scraping (it split on the literal strings
      `"kappa "`, `"file: "`, `"re-simulated"`).
- [x] **Tier 1 tests** — `decoder_fuzz` (13 truncations, 160 bit-flips, 6
      garbage inputs), `exit_codes`, `cli_contract`, `help_contract`,
      `audit_json`.
- [ ] **Tier 2 golden files** — regen md5 per corpus map. NOT DONE.
- [ ] **Phase 1 gate:** full suite green on a clean checkout with no game.

## Phase 2 — breaking

- [x] **R6 positional order** — `ghost synth IN OUT` (was `OUT --from DONOR`,
      which inverted the convention) and `ghost swap-samples IN OUT --donor D`
      (was three positionals). Both breaks were mine, from 2026-09-01.
- [ ] **R5 flag aliases** — `--ghost(s)`, `--map(s)`, `--tick(s)`,
      `--verbose`/`--v`. Plural iff repeatable.
- [ ] **R7 namespace** — promote ~10 real verbs; forensic probes (`set-u01`,
      `swap-samples`, `car-first`, `poke`, `codeccheck`, `strip-events`,
      `split-car`) under `ghost debug`.

## The golden blocker — SOLVED, and it was a real bug

`golden_full_fields` failed in release and passed in debug. Not a stale golden:
**the decoder was not optimisation-invariant.** `Sample` held `f64`, printed at
full precision, so a 1-ulp difference in trig became a different md5 — three
bytes across a 454 KB render.

Fixed at the root: **`Sample`'s floats are now `f32`, the width the record
actually stores.** Position and velocity are f32 in the file; orientation comes
from three packed integers (a u16 and two i16) carrying ~5 decimal digits. The
f64 was a Python-port artefact. Trig still runs in f64 and narrows once, at the
boundary. ~20 files across six crates updated.

Goldens re-blessed from the Rust decoder, and the test is **strict again** —
byte for byte, now holding under BOTH profiles.

## Baseline

```
cargo build --release --workspace && cargo test --release --workspace --no-fail-fast
469 passed · 1 failed
```

The one failure is `clip`'s `gate_runs_with_a_scrubbed_environment`, which
needs network egress. On the render box the equivalent failure was
`split_holds_the_shorter_run…` (a Windows ffmpeg that cannot read a WSL path).
**Both are environmental and neither has been triaged.**

Coverage is still inverted against risk: `haul` 166 unit tests, `ghost` 5.

## Not yet verified

**The engine regen has NOT been re-run since the f32 change.** It needs a
dedicated server and no box in this session has one. What IS verified on the
45 corpus ghosts:

```
ghost inspect    45 / 45 clean
tmtraj check     45 / 45 pass     (C8/C9/C10 recompute wheel rotation,
                                   throttle echo and flight distance FROM
                                   the narrowed fields)
ghost synth      45 / 45 byte-identical bodies
```

That covers decode and compare. It does **not** cover the carrier gather, which
is where `regen.rs`'s distance arithmetic is exercised for real.

## Working arrangement — CHANGED

The render box (`~/bin/wsx`) went **offline** mid-session; the git checkout
lives there. Work moved to a fresh clone on a devserver:

```
git clone https://github.com/vjeux/trackmania-tas.git   # needs the fwdproxy
env https_proxy=fwdproxy:8080 http_proxy=fwdproxy:8080 cargo build ...
```

Cargo needs the proxy for its one dependency. **That box has no GitHub
credential**, so commits since `de0a4dd` are LOCAL ONLY and banked as bundles:

- `F1995456313` — through `d0fb705`
- `F1995457002` — through `d337305`

Unpushed: `d0fb705`, `d337305`, `f178ba3`. Push them when the render box
returns (its credential is the only one that reaches GitHub), or copy that
credential to a live box — it is a file, and a credential that lives on machine
A is not a property of machine A.
