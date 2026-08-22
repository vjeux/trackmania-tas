# tmsite — the TM2020 TAS toolchain's presentation end, in Rust

Ports three Python tools out of `tmtas/`:

| Python | here |
|---|---|
| `site/build_site.py` (= `code/build_site.py`, identical copy) | `tmsite site` |
| `code/build_compact.py` | `tmsite compact` |
| `code/to_tick_script.py` (= `results/to_tick_script.py`) | `tmsite tick` |

The GBX container, the ghost input tape and the bit codec are **not** in this
crate: they live once, in the workspace's [`gbx`](../gbx) crate, and tmsite
depends on it (`gbx = { path = "../gbx" }`). liblzo2 is reached through `dlopen`
inside `gbx`, so nothing has to be installed to build.

```
cargo build --release
cargo test  --release        # 42 tests, fixtures under testdata/

tmsite site     [--dir D] [--out F] [--stride N]            # default stride 1
tmsite compact  [--dir D] [--out F] [--stride N] [--pick K] # default stride 3
tmsite tick     --ghost G [--out F] [--archive N] [--raw] [--seed N]
tmsite verify   --ghost G [--script F] [--archive N] [--raw]
tmsite stats    --html F [--html F2 ...]
tmsite serve    --root D [--port N] [--requests N]
```

`--dir` defaults to `/tmp/entrec/paths` (the decoded trajectories).

## Faithfulness

Two independent controls, both re-run on 2026-08-22 against the current build:

**Against CPython's own output.** `tmsite site` (stride 1, the 51-path corpus in
`/tmp/entrec/paths`) reproduces the shipped `tmtas/site/tm_lines.html` **byte for
byte**, 542 463 bytes; `tmsite compact --stride 4` reproduces the shipped
`tmtas/site/tm_compact.html` **byte for byte**, 53 120 bytes. (The shipped
compact page was built at stride 4, not at this tool's default 3 — a page built
at the default is a different, equally valid page, not a mismatch.) The earlier
claim of "30 of 30 configurations" and "13 of 13 ghosts" rested on a CPython
that is no longer on this box and cannot be re-run; it is replaced by the two
artefacts above, which can.

**Against the tool's own previous decoder.** Before the private GBX/ghost/bits
copies were deleted in favour of `gbx`, the pre-change binary exported TICK
scripts for 233 ghosts (every ghost in `/tmp/tmtas/tmtas/ghosts`, `/tmp/m1`,
`/tmp/m2` and every `<map>/replays/` directory in the repo), in both plain and
`--raw` mode. After the switch **all 466 scripts are byte-identical**, as are
site pages at strides 1 and 7 and compact pages at (3, no pick) and (4, pick 8).
The only differences on stderr are the two deliberate changes below (respawn
lines, seconds). Both decoders agree on every ghost we have.

`tmsite verify` re-imports each exported script and replays it tick by tick:
**227 of 227 ghosts EXACT MATCH** (the 228th file, `AUTHOR_LAP_20258_watchable`,
carries no `0x0309201D` chunk at all and is an error, not a mismatch). 30 of
those ghosts contain respawn inputs.

Three CPython behaviours are modelled explicitly in `src/pyfmt.rs` because
byte-identity depends on them, and each has direct unit tests in
`tests/pyfmt_cpython.rs` with cases a naive implementation fails:
`repr(float)` (shortest round-trip, trailing `.0`, and CPython's exponent
thresholds, which Rust's `{}` does not have), `round(x, n)` (correctly rounded,
ties to even, against the *exact* binary value), and `%`-formatting of a
template dict.

## Times are seconds

`36.049`, never `36049`. Every number tmsite prints that is a time is seconds
with three decimals, including the TICK script's header comment
(`# 2432 ticks, ghost start offset -1.580 s, declared 22.730 s`) and
`tmsite stats`' time range. Tick indices and counts stay integers.

Two places keep milliseconds because the format is not ours to change, and both
are machine-read, never displayed:

* **the TICK grammar** — `<ms> accel 1` is what TICK's parser accepts, and `<ms>`
  must be an integer multiple of 10;
* **the page payloads** — `{"time":19538}` in the full page and `[...,19538,...]`
  in the compact page's META are parsed by the page's own JavaScript, which
  formats them for display (`(r.time/1000).toFixed(3)`).

## Verification tools that ship with it

* `tmsite stats --html <page>` re-reads a built page — either variant, including
  the ones Python built — parses its payload with a **strict** JSON parser that
  rejects the bare `NaN`/`Infinity` CPython emits, unpacks the compact page's
  base64 blob, and prints path count, sample count, coordinate ranges, speed
  range and the colour scheme.
* `tmsite verify --ghost <g>` exports a TICK script and replays it back tick by
  tick against the ghost's own decoded inputs — steer, accel, brake **and
  respawn**. The replay enforces TICK's grammar (10 ms alignment, steer in
  -127..127, known actions, no action past the end of the run), so a clean
  verify is also a grammar-conformance check.
* `tools/pagecheck.js` runs a built page's **own JavaScript** headless against a
  stub DOM/canvas and reports what the page computed from its own payload.

## Deliberate differences from the Python

1. **`0 seed` was dead code.** `to_tick_script.py` read
   `getattr(t, "validation_seed", None)`; `Template` has no such attribute and
   never did, so the line never fired. Replaced with an explicit `--seed N`.
2. **`-128` steer is clamped.** The ghost's steer byte is signed, so `0x80`
   decodes to -128, which TICK's parser rejects (*"Steer must be an integer from
   -127 through 127, left, or right"*). `tmsite tick` clamps to -127 and warns;
   `--raw` emits it verbatim, and then `verify --raw` refuses the script, which
   is the honest outcome — TICK would refuse it too. Measured: **0 of 233 real
   ghosts contain one**, so the test builds a ghost that does, by editing one
   packet through the `gbx` codec.
3. **Respawns are encoded, not dropped.** The Python exporter, and this port
   until now, silently dropped them with a warning. A respawn is a real,
   editable input — bit 31 of the packet's state literal, which unpacks into
   `word0` bit 5 — and it can pin a run's finish time, so a script without it
   does not reproduce the run. `tmsite tick` emits TICK's `respawn` action, and
   `word0 & 0x1000` as `srespawn`; `tmsite verify` compares both per tick.
   Within a tick the respawn line is written **before** that tick's
   steer/accel/brake: the car is put back on the ground and then the tick's
   inputs apply to it.
   *Evidence that the bit is what we say it is:* the ghost's telemetry record is
   a separate data path (one 116-byte vehicle sample per 50 ms, written by the
   simulation). On `238835/replays/NORETRY_407463_watchable`, 8 of the 9 respawn
   ticks sit within 100 ms of a >= 8 m jump between consecutive samples, while
   only 10 of 8103 steps (0.12 %) jump that far at all —
   `tests/respawn_is_real.rs`. **`srespawn` is mapped by name only**: no ghost in
   the corpus separates it from `respawn` cleanly enough to prove it from
   telemetry.
4. **The compact page's note is no longer hard-coded to 51.** It said
   "all 51 lines overlap at 1x" even when `--pick` had cut the field to 8. The
   literal is now `%(n)d`; with the 51-run corpus the byte-identity check is
   unaffected.
5. **Packing saturation is reported.** The compact format stores y in one byte
   of decimetres (25.5 m of elevation) and speed in one byte of 2 km/h
   (510 km/h); the Python clamped silently. A saturating build now warns.
6. `--pick 1` raised `ZeroDivisionError` in the Python; it is a clean error here.

## Layout

```
src/lib.rs     the crate as a library, so tests/ can reach it
src/pyfmt.rs   CPython-compatible float repr, round-half-even, %-templating, base64
src/json.rs    strict JSON reader (rejects NaN/Infinity, as JSON.parse does)
src/traj.rs    loading /tmp/entrec/paths/*.json
src/site.rs    full page      + templates/site.html    (verbatim from the Python)
src/compact.rs packed page    + templates/compact.html (verbatim + fix 4)
src/tick.rs    TICK export, script replay, round-trip diff  (ghost I/O: gbx::tape)
src/stats.rs   measure a built page
src/serve.rs   ~60-line static file server for the fetch check
tests/         cargo test --release; fixtures in testdata/, goldens in testdata/golden/
```

## Tests

`cargo test --release`, no network, no fixtures outside the repo except two
ghosts from the corpus (`238835-turtle-trial-angustus/replays/`). **A missing
fixture fails the test that needs it and names the file** — a test that passes
because its input vanished is worse than no test.

| file | what it pins |
|---|---|
| `tests/tick_script.rs` | TICK export byte-identical to `testdata/golden/*.tick`; export → verify exact on three ghosts; respawn ticks, their millisecond and their order within a tick; a script with the respawn lines stripped FAILS; the -128 clamp, on a ghost built for it; the grammar's rejections; seconds in the header |
| `tests/pages.rs` | `site`/`compact` byte-identical to `testdata/golden/*.html`; `stats` numbers on those goldens; `--pick` spread; run order; NaN payload refused; empty directory is an error |
| `tests/pyfmt_cpython.rs` | the three CPython behaviours, each with a case the naive implementation gets wrong |
| `tests/respawn_is_real.rs` | the respawn bit against the telemetry (see difference 3) |
| `src/json.rs` | the strict parser's NaN/Infinity refusal |

The fixtures are small on purpose: five trajectories out of the 51 (the two that
tie on 19.556 s are both in, so the tie order is pinned), a 15 KB human ghost, a
28 KB ghost carrying 6 respawns and 5 standing-respawns.
