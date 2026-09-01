# Release progress

Durable state for the toolchain release. **Update this file in the same commit
as the work it describes.**

Plan of record: `RELEASE-REVIEW.md`.

## Status: every review item is done, and the suite is green

```
cargo build --release --workspace && cargo test --release --workspace --no-fail-fast
477 passed · 0 failed          (with TM_SERVER set — see "the engine" below)
```

| item | state |
|---|---|
| **R1** `--help` | done — every binary AND every `tmtraj` subcommand |
| **R2** `--version` | done — one workspace version, `<binary> <version> (<stamp>)` |
| **R3** exit codes | done — `0 ok / 1 refused / 2 usage / 3 environment` |
| **R4** machine-readable | done — `ghost verify -o json`, `clip` audit converted |
| **R5** flag names | assessed — 3 of 4 findings withdrawn, the real one recorded |
| **R6** positional order | done — `synth IN OUT`, `swap-samples IN OUT --donor D` |
| **R7** namespace | done — `ghost debug` for probes, 13 invisible verbs documented |
| Tier 1 hermetic tests | done — fuzz, exit codes, help, namespace, audit_json |
| Tier 2 goldens | done — digest + oracle per reference run |
| Tier 3 engine | done — see below |

## What the plumbing work actually found

Not style. Every one of these was a defect:

- **HEAD did not compile.** `carrier.rs` used a field deleted weeks earlier; the
  fix lived only in a build tree. Nothing in the loop ever built from the repo.
- **Three decoder panics** on input a leaderboard could hand you: a slice using
  a length read from the file, an `assert` on lzo failure, and a bit-reader
  assert that **one flipped bit** could trip.
- **A false pass.** 67 bytes of `GBX` + `0xFF` verified OK, because the only
  gates that passed were absence checks ("no account id", "no embedded map").
  A verifier that passes garbage is worse than one that panics.
- **A control that could not fail.** `gate_runs_with_a_scrubbed_environment`
  poisoned a proxy, but a normal `~/.curlrc` sets `noproxy` for `127.0.0.1`, so
  the control always succeeded. Now poisoned with `CURL_HOME`, which no
  `noproxy` can exempt.
- **`--ticks` meant three different things** — a count (13 files), a range
  (`ghost tape poke`), and a boolean (`ghost inspect`). The range is now
  `--tick-range`, with the old spelling warned for one release.
- **13 of 30 `ghost` verbs were undocumented**, including `film` and `synth`.

## The golden blocker — a real determinism bug

`golden_full_fields` failed in release, passed in debug. **The decoder was not
optimisation-invariant:** `Sample` held `f64` printed at full precision, so a
1-ulp difference in trig became a different md5.

Fixed at the root — **`Sample`'s floats are `f32`, the width the record
actually stores.** Position and velocity are f32 in the file; orientation comes
from three packed integers (~5 decimal digits). The f64 was a Python-port
artefact. Trig still runs in f64 and narrows once, at the boundary.

`Val::F` is f32 too, rendered by `py_repr_f32` **without widening** — widening
asks for the shortest string that round-trips as f64, so `0.1f32` would print
`0.10000000149011612`.

## The engine — how to run it

The dedicated server is a public download, and the shim builds from the tree:

```
curl -sSL -o ts.zip http://files.v04.maniaplanet.com/server/TrackmaniaServer_Latest.zip
cd tools/search && cargo build --release -p forkshim
export TM_SERVER=/path/to/server FK_SHIM=$PWD/target/release/libforkshim.so
```

With those: **56 of 58 selftest checks pass**, and the oracle re-simulates
**45 of 45** reference ghosts to the exact millisecond recorded.

The two engine failures are **pre-existing** — identical counts at `9e52cca~1`,
before the f32 change:

- `engine.determinism` — the dead-channel gate firing on bytes [24,26,28,30].
- `engine.trajectory` — **the message differs between runs** ("did not identify
  the car in 24 attempts" vs "0.7136 m mean"), because the car locate is
  probabilistic. A single run of this check reports a different-looking failure
  each time.

Unproven candidate for the 0.71 m: the fixture was recorded on
`git=128149 (2026-02-02)` and the downloadable server is `git=128182
(2026-05-15)`. `BUILD-ID.md` concludes old physics needs an old binary. Against
it: the oracle validates that same fixture at exactly 22.730 on this server.

## Still not covered

The **carrier gather** in `fk regen` on the five corpus maps (203072 and
friends) — those maps are not in this repository. Everything around it is
verified; the pointer walk and memory gather are not.

## Working arrangement

The render box (`~/bin/wsx`) went **offline** mid-session and never returned;
the git checkout and the only GitHub credential live there. Work moved to a
fresh clone on a devserver:

```
git clone https://github.com/vjeux/trackmania-tas.git    # needs the fwdproxy
env https_proxy=fwdproxy:8080 http_proxy=fwdproxy:8080 cargo build ...
```

**Commits since `de0a4dd` are LOCAL ONLY**, banked as bundles on Phabricator
(`F1995456313`, `F1995457002`, `F1995457067`, `F1995458424`, `F1995458818`,
`F1995459…`). Push them when the render box returns, or copy that credential to
a live box — it is a file.
