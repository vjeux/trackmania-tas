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
      referenced `a.pos_delta`, the field removed when the pointer-chain work
      replaced it with `a.chain`. Four `E0609: no field pos_delta` errors.
      The fix existed only in the working tree and had never been shipped.
      Verified: `cargo build --release` in `tools/fk` now finishes clean.

## Phase 1 — plumbing (mechanical, non-breaking)

- [ ] **R2 `--version`** on every binary; one workspace version inherited by
      all crates; git hash stamped at build time.
      *(19 of 24 crates are still at the `cargo new` default 0.1.0.)*
- [ ] **R1 `--help`** on every binary, `ghost`'s grouped style as the model.
      *(Missing entirely from `tmtraj`, `tmauto`, `shootctl`, `mapgeom`,
      `recon`.)*
- [ ] **R3 exit codes** — publish and apply `0 ok / 1 answered NO / 2 usage /
      3 environment`. *(Today `exit(2)` means both "bad flags" and "gate
      refused" across 69 sites.)*
- [ ] **Fixtures** committed; a missing fixture SKIPS instead of panicking.
      *(3 of the 4 current failures are absent fixtures panicking at
      `tmsite/tests/common/mod.rs:30`.)*
- [ ] **`clip` network test** behind `--ignored`.
- [ ] **Tier 1 tests** — round-trip identity, decoder fuzz (truncate/bit-flip,
      assert `Err` not panic), a positive AND negative control for every
      `ghost` gate, `--help`/`--version` smoke test for every binary.
- [ ] **Tier 2 golden files** — `regen` md5 + coverage + first-sample position
      per corpus map.
- [ ] **Phase 1 gate:** full suite green on a clean checkout with no game.

## Phase 2 — breaking (own version bump)

- [ ] **R6 positional order** — `ghost synth IN OUT` (drop `--from`),
      `ghost swap-samples IN OUT --donor D`. Both breaks were introduced in
      the 2026-09-01 session and are mine.
- [ ] **R5 flag aliases** — `--ghost(s)`, `--map(s)`, `--tick(s)`,
      `--verbose`/`--v`. Plural iff repeatable; minority spelling hidden alias
      for one release, then dropped.
- [ ] **R7 namespace** — promote ~10 real verbs; move forensic probes
      (`set-u01`, `swap-samples`, `car-first`, `poke`, `codeccheck`,
      `strip-events`, `split-car`) under `ghost debug`.
- [ ] **R4 `-o json`** on the three commands other tools scrape:
      `ghost inspect`, `tmtraj stats`, `tmmaps census`.

## Measured baseline (2026-09-01)

```
cargo test --release --workspace --no-fail-fast
58 test binaries · 452 passed · 4 failed

failures, all environmental:
  clip    gate_runs_with_a_scrubbed_environment          needs live network
  tmsite  export_then_verify_is_exact_on_every_ghost…    fixture absent
  tmsite  the_respawn_bit_coincides_with_a_teleport…     fixture absent
  tmsite  the_respawn_fixture_really_contains_respawns   fixture absent

unit tests by crate (inverted against risk):
  haul 166 · clip 68 · tmtraj 54 · tmauto 40 · fk 38 · gbx 25
  tmmaps 9 · mapgeom 8 · ghost 5   ← the flagship, 13,581 lines
```

## Working arrangement

The build/test tree is `/tmp/y/rp/tools` on the OD box; the git checkout is
`~/trackmania-tas` on the render box, reached with `~/bin/wsx` **from
devvm42752** (wsx needs navi credentials that exist only there). Changes travel
tarball → Phabricator file → `wsx push` → extract → commit.

**Before editing, re-run the md5 manifest comparison of both trees.** It is
what caught the broken HEAD above, and the two trees drift silently whenever a
file is edited locally and not shipped.
