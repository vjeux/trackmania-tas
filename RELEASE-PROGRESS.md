# Release progress

Durable state for the toolchain release. **Update this file in the same commit
as the work it describes** — a session that loses its context reads this and
knows exactly where it is.

Plan of record: `RELEASE-REVIEW.md` (24 crates surveyed, 10 API findings,
4-tier test plan).

## Phase 0 — preflight

- [x] **Working tree verified against repo HEAD.** 361 source files compared by
      md5; exactly one differed.
- [x] **HEAD DID NOT COMPILE — fixed.** `fk/fk/src/cmd/carrier.rs` still
      referenced `a.pos_delta`, removed when the pointer work replaced it with
      `a.chain`. Four `E0609` errors. The fix existed only in the working tree.

## Phase 1 — plumbing (mechanical, non-breaking)

- [x] **R2 `--version`** — `[workspace.package] version = "1.0.0"` inherited by
      all 23 crates; every binary prints `<binary> <version> (<stamp>)` from
      `CARGO_BIN_NAME` / `CARGO_PKG_VERSION` / `option_env!("TAS_BUILD")`.
      Compile-time only, no build.rs, no dependency.
- [x] **R1 `--help`** — was missing from 14 of 23 binaries, three of which
      (`dsprobe`, `recon`, `rend`) *panicked* with no arguments. All 23 now
      print usage on stdout and exit 0. The `fn usage() -> !` tools were
      refactored so the error path and `--help` share one `USAGE_TEXT`.
- [x] **`gbx/tests/cli_contract.rs`** enforces both, with an explicit binary
      list so a new tool without help is a review comment, not a silent gap.
- [ ] **R3 exit codes** — publish and apply `0 ok / 1 answered NO / 2 usage /
      3 environment`. Today `exit(2)` means both "bad flags" and "gate
      refused" across 69 sites.
- [ ] **Tier 1 tests** — decoder fuzz (truncate/bit-flip, assert `Err` not
      panic), a positive AND negative control for every `ghost` gate.
- [ ] **Tier 2 golden files** — `regen` md5 + coverage + first-sample position
      per corpus map.
- [ ] **Phase 1 gate:** full suite green on a clean checkout with no game.

## Phase 2 — breaking (own version bump)

- [ ] **R6 positional order** — `ghost synth IN OUT` (drop `--from`),
      `ghost swap-samples IN OUT --donor D`. Both breaks are mine, from the
      2026-09-01 session.
- [ ] **R5 flag aliases** — `--ghost(s)`, `--map(s)`, `--tick(s)`,
      `--verbose`/`--v`. Plural iff repeatable.
- [ ] **R7 namespace** — promote ~10 real verbs; forensic probes (`set-u01`,
      `swap-samples`, `car-first`, `poke`, `codeccheck`, `strip-events`,
      `split-car`) under `ghost debug`.
- [ ] **R4 `-o json`** on `ghost inspect`, `tmtraj stats`, `tmmaps census` —
      the three commands other tools scrape.

## Baseline — CORRECTED 2026-09-01

`RELEASE-REVIEW.md` reports 452 pass / 4 fail with three absent fixtures. **That
was measured on the build tree, which has no `testdata/`, and is wrong about
which tests fail.** On the git checkout, after `cargo build --release
--workspace`:

```
59 test binaries · 454 passed · 2 real failures
```

The tmsite fixture failures do not exist: those tests pass on a real checkout
(19/19). The genuine failures are:

| test | crate | status |
|---|---|---|
| `every_decoded_field_is_pinned` | tmtraj | **REAL, pre-existing.** 17 of 45 runs render differently from the committed golden. Verified identical at `7c48fbe`, before any release work, so it is not caused by it. Either the decoder moved and the golden was never re-blessed, or the golden is from a different corpus. **Blocks the release**: a golden that does not match is either a bug or a lie, and until it is diagnosed we do not know which. |
| `split_holds_the_shorter_run_to_the_length_of_the_longer` | clip | Environmental. Writes to `/tmp`, and the Windows `ffmpeg` behind the WSL bridge cannot read a WSL path ("is not on a Windows drive"). Should use `/mnt/<drive>/…` when running on the bridge, or be marked `#[ignore]` as needing the render box. |

**`cargo test` alone is not enough.** `cli_contract` runs the binaries in
`target/`, and `cargo test -p gbx` does not rebuild other crates' binaries — so
it once reported 14 broken tools that had already been fixed. The release
command is:

```
cargo build --release --workspace && cargo test --release --workspace --no-fail-fast
```

The test now says so in its own failure message.

Unit tests by crate — coverage is inverted against risk:

```
haul 166 · clip 68 · tmtraj 54 · tmauto 40 · fk 38 · gbx 25
tmmaps 9 · mapgeom 8 · ghost 5   ← the flagship, 13,581 lines
```

## Working arrangement

Build/test tree: `/tmp/y/rp/tools` on the OD box (**no `testdata/`** — fixture
tests cannot be trusted there). Git checkout: `~/trackmania-tas` on the render
box, reached with `~/bin/wsx` **from devvm42752** (wsx needs navi credentials
that exist only there). Changes travel tarball → Phabricator file → `wsx push`
→ extract → commit.

**Before editing, re-run the md5 manifest comparison of both trees** — it is
what caught the broken HEAD. **Before trusting a test result, check which tree
it ran on.**
