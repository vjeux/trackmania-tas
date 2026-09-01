# The TAS toolchain — release review

24 crates, ~120,000 lines of Rust, one workspace, one third-party dependency
(`miniz_oxide`). Every measurement below was taken from the tree, not recalled.

---

## Part 1 — What is actually in here

The toolchain is five layers. Nothing in the upper layers reimplements the
format; that discipline is the tree's best structural property and is worth
protecting in the release.

### Layer 0 — the format

| crate | lines | what it owns |
|---|---:|---|
| `gbx` | 4,345 | The GBX container, the chunk table, the 10 ms input tape, `CPlugEntRecordData`, the 116-byte vehicle sample. **The single decoder.** |

### Layer 1 — artifact surgery

| crate | lines | what it owns |
|---|---:|---|
| `ghost` | 13,581 | Every mutation of a ghost/replay: inspect, tape extract/inject, trim, splice, declare, regen, film, verify, synth. The flagship. |
| `tmmaps` | 8,495 | `.Map.Gbx` surgery: census, region moves, segment maps, ladders, the plain oracle. |
| `chunkswap`, `strpatch` | 251 | Single-purpose byte surgery. |

### Layer 2 — the engine

| crate | lines | what it owns |
|---|---:|---|
| `fk` | 17,933 | Re-simulate a tape in a live engine and read car state per tick. Pointer chains, the carrier gather. |
| `forkoracle` | — | The fork server and its `LD_PRELOAD` shim. |
| `resim` | 1,349 | The standing re-simulation sweep as a long-haul worker. |
| `dsprobe`, `asmdig`, `asmshape` | 716 | Binary/ABI reconnaissance. |

### Layer 3 — analysis

| crate | lines | what it owns |
|---|---:|---|
| `tmtraj` | 17,469 | Read-only analysis: decode, compare, publish gate, corpus scans, racing-line clustering. |
| `mapgeom` | 7,626 | A map as real 3D geometry. |
| `uwlab`, `pkz2` | 4,491 | Per-map analysis arms. |
| `recon` | 1,149 | Grow an input tape forward while it keeps matching. |
| `vidread` | 3,257 | Read a run off a screen recording. |

### Layer 4 — presentation and orchestration

| crate | lines | what it owns |
|---|---:|---|
| `tmauto` | 6,679 | The autopilot's oracle, provenance, container CLI. |
| `haul` | 9,790 | Job queue, ledger, leases, credentials. |
| `clip` | 4,625 | Publish a rendered run; side-by-side. |
| `tmsite` | 4,575 | 3D visualisation, TICK export, leaderboard capture. |
| `shootctl` | 2,237 | Drive the game to render a clip. |
| `rend`, `wsx`, `pkz2`, `wincrash` | 2,687 | Render box, file bridge, crash capture. |

---

## Part 2 — API review

### The measurements

```
--map        90 uses      --ghost      24        --tick        2
--maps        3           --ghosts      4        --ticks      36
--out        85           --verbose    11        --v           2

--help present in:  ghost, tmmaps, clip, haul, tmsite       (5 of 10 checked)
--help ABSENT in:   tmtraj, tmauto, shootctl, mapgeom, recon (5 of 10)
--version present:  nowhere

exit(2) 69 sites · exit(1) 43 · exit(3) 20 · exit(0) 9 · exit(9x) 8
machine-readable output: --json 2 sites · --tsv 3 · --csv 6
crate versions: 19 crates at 0.1.0, 4 at 1.0.0, 1 at 0.2.0
```

### R1. Half the tools cannot tell you how to use them ★ blocker

`tmtraj` (17k lines, the analysis workhorse) and `tmauto` (6.7k) have no
`--help` at all. `ghost`'s help, by contrast, is genuinely excellent — grouped
by operation, with prose explaining *why* each gate exists.

**Fix.** Adopt `ghost`'s help as the house style and make it mandatory. A
`cli::usage(&str)` helper in a shared crate, one `--help` arm per binary, and a
test that asserts every binary exits 0 on `--help` and prints its own name.

### R2. No `--version`, and versions are meaningless ★ blocker for a *release*

You cannot ship a release whose binaries cannot say what they are. 19 of 24
crates are still at the `cargo new` default.

**Fix.** One workspace-level `version` inherited by every crate
(`version.workspace = true`), stamped with the git hash at build time, and a
`--version` arm alongside `--help`. This is a prerequisite for every bug report
you will get after release.

### R3. Exit codes are undocumented and overloaded

`exit(2)` is used at 69 sites for both "bad usage" and "gate refused" — which
means no caller can distinguish *"you typed it wrong"* from *"the file is not
publishable"*. `shootctl` already uses a richer scheme (0/1/2/3) informally.

**Fix.** Publish and enforce one table:

```
0  success
1  the operation ran and the answer is NO   (gate refused, verify failed)
2  usage error — bad flags, missing file
3  environment error — no server, no engine, no network
4+ reserved
```

This matters more than it looks: the whole pipeline is scripted, and today a
script cannot tell a refusal from a typo.

### R4. Nothing is machine-readable

Across 120k lines: 2 `--json` sites, 3 `--tsv`, 6 `--csv`. `ghost manifest`
emits deterministic JSON and is the model — everything else prints prose that
callers scrape. Several tools in this tree scrape *each other*.

**Fix.** `-o human|json` on every read-only command, human default. Prose stays
exactly as it is; JSON becomes the contract. Retrofit `ghost inspect`,
`tmtraj stats`, `tmmaps census` first — those three are what everything else
scrapes.

### R5. Singular/plural and synonym drift

`--ghost` (24) vs `--ghosts` (4); `--map` (90) vs `--maps` (3); `--tick` (2)
vs `--ticks` (36); `--verbose` (11) vs `--v` (2).

**Fix.** Plural iff repeatable. Keep the minority spelling as a hidden alias
for one release, warn on it, then drop it.

### R6. Positional-argument order is inconsistent — including two I added today

`ghost` has a clear convention: `FILE` for reads, `IN OUT` for mutations. It
holds across `trim`, `splice`, `declare`, `regen`, `split-car`, `strip-events`,
`car-first`. Two commands break it, and both are mine from this session:

```
ghost synth OUT --from DONOR        input arrives as a FLAG, output is positional
ghost swap-samples IN DONOR OUT     three positionals, donor in the middle
```

**Fix.** `ghost synth IN OUT` (drop `--from`); `ghost swap-samples IN OUT
--donor D`. Rule: **at most two positionals, always `IN OUT`, every other
input is a named flag.** I'd rather flag my own inconsistency now than ship it.

### R7. `ghost` has 38 subcommands in one flat namespace

`inspect codeccheck trajdiff engine manifest chunks dump tape map trim splice
synth swap-samples car-first split-car set-u01 strip-events declare identity
header census phase record film regen regen-control roundtrip verify selftest
poke script graft set extract inject expand diff stats recinputs sync-record`

Several are debugging probes from specific investigations (`set-u01`,
`swap-samples`, `car-first`, `poke`, `codeccheck`) sitting at the same level as
`film` and `verify`, which are the actual product.

**Fix.** Two tiers. Promote the ~10 real verbs; move the probes under
`ghost debug <verb>` and say plainly in help that they are forensic tools with
no compatibility promise. This is the single biggest usability win available.

### R8. The dispatch tables leak flags into the command namespace

`tmmaps` and `tmsite` match `--map`, `--ghosts`, `--out` in the same `match`
that dispatches subcommands, so a mistyped flag is reported as an unknown
*command*.

**Fix.** Parse flags before dispatch; reserve the command namespace for commands.

### R9. `die()` vs `eprintln!` is 580 vs 467

Two error paths with no rule about which to use, so some failures print to
stdout and some to stderr — which corrupts the output of any command being
piped.

**Fix.** All diagnostics to stderr, all data to stdout, one `die()`.

### R10. What is genuinely good — keep it

Worth naming so a refactor does not destroy it:

- **One decoder.** Nothing above `gbx` re-parses the format.
- **Gates that refuse to write.** "Every command that WRITES a file runs a
  control first" is a rare and valuable property.
- **Prose that explains why.** The comments record measured failures and
  reverted experiments. That is the institutional memory of the project.
- **No shell, no Python.** Every capability is a subcommand.

---

## Part 3 — Testing plan

### The baseline, measured

```
cargo test --release --workspace --no-fail-fast
58 test binaries · 452 passed · 4 failed
```

Unit tests by crate — coverage is **inverted against risk**:

```
haul      166      clip       68      tmtraj     54      tmauto  40
fk         38      gbx        25      tmmaps      9      mapgeom  8
ghost       5   ← the flagship, 13,581 lines
```

`ghost` produces every publishable artifact and has 5 tests. `haul`, a job
queue, has 166.

### The 4 current failures — all environmental, and that is the bug

| test | crate | cause |
|---|---|---|
| `gate_runs_with_a_scrubbed_environment` | clip | needs live network via a proxy |
| `export_then_verify_is_exact_on_every_ghost_fixture` | tmsite | fixture missing |
| `the_respawn_bit_coincides_with_a_teleport…` | tmsite | fixture missing |
| `the_respawn_fixture_really_contains_respawns` | tmsite | fixture missing |

Three panic at `tmsite/tests/common/mod.rs:30` — the fixture loader — with a
panic that reads like a logic failure. **A missing fixture must fail as
`SKIPPED: fixture X not present`, never as a panic.** Otherwise every new
contributor's first `cargo test` reports four bugs that do not exist.

### Tier 1 — hermetic unit tests (must pass everywhere, no game, no network)

1. **Round-trip identity, property-based.** For every fixture: decode → encode
   → compare bytes. Extend to `ghost synth`, which already proves this on five
   ghosts and should be a test rather than a manual command.
2. **`gbx` decoder fuzz.** Truncate and bit-flip fixtures; assert the decoder
   returns `Err`, never panics. Zero such tests exist today.
3. **Gate unit tests for `ghost`** — the largest gap. Each gate (V5/V6/V7,
   carrier, dead-channel, tape-identity) needs a *positive control* it passes
   and a *negative control* it must refuse. This session found a gate that had
   never fired in a test and a gate whose refusal message named nothing.
4. **Every binary answers `--help` and `--version` with exit 0.**

### Tier 2 — golden-file tests (fixtures, still no game)

`tmtraj/tests/golden` exists; generalise it. For each of the 5 corpus maps:
`regen` → assert md5 against a committed golden, plus coverage and first-sample
position. This session shipped a file whose every coordinate was one field
late while coverage read 100% — a golden md5 would have caught it instantly.

**Fixtures must be in the repo.** They are not in the source tarball today,
which is why three tests fail here.

### Tier 3 — engine tests (need the dedicated server; CI-capable)

5. **Corpus regen sweep**, the release gate:

```
map      regen    md5         coverage   first sample == source
203072   5.5 s    eb1b8a7c    533/533    yes
294446   5.4 s                582/582    yes
287431  14.1 s                416/416    yes
286279  21.1 s                oracle 235.625 cps=4
126859           pointer 3/3  469/469    yes
```

6. **Chain-resolution hit rate.** Chains resolve probabilistically; assert
   ≥N/M over repeated runs, never a single run. Two claims were retracted this
   session for exactly this.
7. **Oracle agreement**: every produced file re-simulates to its declared time.

### Tier 4 — client tests (need Windows + the game; manual or nightly)

8. **Import smoke test.** `shootctl setup` for each corpus map; assert
   `scene ready` and that the process survives. This is the only tier that
   catches loader crashes, and it caught one this session that every offline
   gate passed.
9. **Render smoke test**, one map end-to-end.

### What this plan would have caught, from this session alone

- the position-shift corruption (Tier 2 golden md5)
- the wrong-chain carrier bug (Tier 3 corpus timings — 41 s vs 5 s)
- the client import crash (Tier 4)
- the unexercised dead-channel gate (Tier 1 negative control)

### Release checklist

- [ ] R1 `--help` on every binary · R2 `--version` + git stamp
- [ ] R3 documented exit codes · R6 fix the two positional-order breaks
- [ ] R7 promote 10 verbs, demote the probes to `ghost debug`
- [ ] Fixtures committed; missing fixture ⇒ skip, not panic
- [ ] `clip` network test moved behind `--ignored`
- [ ] Tier 1 + 2 green on a clean checkout with no game installed
- [ ] Tier 3 green on the corpus; Tier 4 run once by hand and recorded

### Sequencing

Ship the release in two steps. **First** the plumbing — help, version, exit
codes, fixtures, hermetic tests — which is mechanical and low-risk. **Then**
the namespace change (R7) and flag renames (R5, R6), which are breaking and
deserve their own version bump. Do not do both at once; the plumbing is what
makes the breaking change safe to review.
