# Which build of the search produced a number

*2026-08-22. Written because the toolchain silently forked into two lineages,
and the one everybody now builds from is the one without the fixes.*

## The finding

The TM2020 TAS toolchain has two lineages of `tmsearch`, and they never merged.

* **The tools lineage** — `r165_tools_v5.tgz` and every `whl_tools_v*.tgz`, i.e.
  the tarball that has been passed around as "the maintained toolkit" since
  2026-08-20. Its `tmsearch` is **byte-identical to the pre-hardening
  `reliability.tgz`**.
* **The hardened lineage** — `tmtas-rs-hardened.tgz` → the
  `tmtas-rs-hardened-plus-lowinput-v6.x` overlays. This is where the phantom
  investigation's fixes actually landed.

Nothing merged the second back into the first. So an arm that followed the
standing instruction to "restore `r165_tools_v5.tgz`" got a search with **every
one of the four phantom defects open**.

### The evidence, measured rather than remembered

In `r165_tools_v5.tgz`:

```
tmsearch/src/main.rs:55        const FINISH_BASE: i64 = 100_000_000;
tmsearch/src/forksearch.rs:58  pub const FINISH_BASE: i64 = 100_000_000;
tmsearch/src/bin/tmtas.rs:318  const FINISH_BASE: i64 = 100_000_000;
grep -rc 'claim_root|phantom|PHANTOM' tmsearch/src   ->  0 files
```

and, in the same tarball, on the driver side:

```
fk/src/state.rs:52       default_work_dir() -> /tmp/fk/stw-<pid>     (hardened)
fkdrv/src/forksrv.rs:107 pipe2(fds, O_CLOEXEC)                       (hardened)
fkdrv/src/forksrv.rs:119 the .fkowner directory lock                 (hardened)
fkdrv/src/forksrv.rs:148 impl Drop for ForkServer -> SIGKILL          (hardened)
fkdrv/src/layout.rs:194  verify_tape, the tape identity control       (present)
```

**So "restore r165_tools_v5" gives you a hardened oracle driver and a broken
search.** That combination is the reason nobody noticed: the driver's own
controls all pass. The fork server will tell you truthfully that it reproduced
a full validation exactly, while the search above it is scoring candidates on a
constant that lets an eleven-checkpoint DNF outrank a finisher. **Nothing the
driver reports is evidence about the search.**

A census of every archive in the store confirms the split — the `main.rs` in
`forkserver / forkstate / predicates / reliability / tmtas-rs / fk-hardened /
r165_tools_v1..v5 / whl_tools_v7..v17` is 854–921 lines with `FINISH_BASE =
1e8`; the `tmtas-rs-hardened` → `v6.x` line is 1122–1277 lines, and only the
`v6.x` overlays carry `1e12`.

## Check the constant, not the version

This is the reusable lesson and it should outlive the specific bug.

* A tarball's name does not tell you which lineage it is.
* **A tree can hold disagreeing copies of the same constant.** One real tree
  had `main.rs` at `1e9` and `forksearch.rs` at `1e8`. The rule is *equal AND
  correct*, not *not the broken value*.
* The `v6.x` overlays patched two of the three copies; `bin/tmtas.rs` stayed
  at `1e8`, so the analysis tool kept mis-rendering while the search was fixed.

Three copies of one number is the defect underneath the defect. In the rebuilt
search there is **no constant at all**: an outcome is
[`tmsearch::score::Outcome`](search/tmsearch/src/score.rs), a two-variant enum
whose `Ord` puts every finisher above every non-finisher by construction, for
any checkpoint count and any map length. The test
`no_dnf_ever_outranks_a_finisher` sweeps checkpoint depths 0..64 against an
hour-long finisher, and it is not a threshold anyone can tune wrong.

## What was open in the tools lineage, and what each one does

| defect | effect |
|---|---|
| `FINISH_BASE = 1e8` | On a map with **≥10 checkpoints**, a deep DNF outranks a finisher and the search abandons finishing runs for deep failures — silently, and it looks like progress. At **≥6 checkpoints** a DNF *prints* as a time. On a map with fewer checkpoints it cannot bite at all. |
| no phantom guard | A banked `best_*.Ghost.Gbx` was never re-validated. Any of the four phantom mechanisms could put a time in the log and a file on disk that does not achieve it. |
| no root claim | `--root` defaulted to the fixed `/dev/shm/tmsearch`, and worker directories are named by index, so two concurrent searches validate each other's candidates. Measured A/B on one map: shared root → 13 banked bests, **7 phantoms**; distinct roots → 8 of 8 exact. |
| one resume boundary for the whole fleet | The fork resume rewrites input records the engine may already have consumed; a record already consumed cannot be un-consumed, so the rewrite is a **silent no-op**. The boundary was calibrated once in the master, and in one real 150-worker run **135 workers stopped past it**. An invisible mutation scores exactly the incumbent's score, `delta == 0` is accepted, and that worker's lineage is contaminated for free. |

## What this invalidates, concretely

The honest answer has three parts, and the middle one is the important one.

**1. Published headline times are not invalidated by this, and here is the
measurement.** Every published ghost in five map directories was re-simulated
today, on this box, through the plain oracle against the map from the store:

```
tmsearch validate --map <store>/<id>/map.Map.Gbx <repo>/<id>-*/replays/*.Ghost.Gbx
```

| map | files | agree with their own filename |
|---|---|---|
| 126859 Kacky Reloaded #290 | 6 | 6 |
| 146612 Spaghetti Nights 2 | 9 | 8 + the file named `SEGMENT_cp5_..._DO_NOT_PUBLISH`, which returns DNF cp5 as its name says |
| 173636 Tap water 01 | 4 | 4 |
| 249521 impossible at for ssano | 4 | 4 |
| 145875 unlucke get jiggy with it | 6 | 6 |

**29 of 29 publishable files re-simulate to exactly the time in their name.**
Three of them declare a different time in their header — a stale declaration
inherited from the template, which `ghost declare --from-oracle` fixes and
which changes no physics.

That is what one would expect from the mechanism: the `FINISH_BASE` defect
cannot fabricate a validated time. It changes which candidate the search
*chases*, not what the oracle *says*. Nothing in the map surgery or validation
path references it, and every published ghost has been through the publish
gate, one of whose four families is *the oracle re-simulating the written
file* — the check the guard now automates.

**2. Any number quoted from a search log or an unvalidated `best_` file is
worth exactly nothing until re-simulated.** That is the population at risk:
intermediate bests, "we reached X in this arm" claims, per-arm comparisons, and
anything copied out of a run's stderr. There is no way to tell from the number
itself.

**3. "We searched N evaluations and found nothing" is weak evidence on any map
with ten or more checkpoints, if the run came from the tools lineage.** The
search was ranking deep failures above finishers, so a negative result there is
a statement about a corrupted objective. Maps in this project with enough
checkpoints for that to bite include 210218 (16), 186935 (16), 153527, 286279
and 238835. One arm hit exactly this and diagnosed it independently.

### The artefact that settles it for a given map

For a headline: the map's `RESULT*.md` **read by mtime**, plus the publish-gate
row for the ghost. Re-running the check today costs one command and no
reasoning:

```
tmsearch validate --map <map>.Map.Gbx <the published ghost>
```

It prints the time the engine SIMULATED, and — when they differ — the time the
file DECLARES. Agreement is the whole claim. Disagreement is either a stale
header (harmless, fixable with `ghost declare --from-oracle`) or a phantom
(not).

For a search run: the run's `--log`. In the rebuilt search every accepted
improvement writes a line carrying the oracle's own answer and the provenance
of the claim, and every refusal writes a `PHANTOM_` line. A run with no log
cannot be audited after the fact, and that is now the only way to lose the
information.

## What has changed

`tools/search` is one lineage. The guard is not optional: `Bank` owns the
output directory and the only method that puts a file in it validates first, so
there is no code path that banks an unconfirmed time. The resume floor is the
maximum over workers behind a startup barrier, and a test with a fake oracle
fails if any worker edits below it. `--root` is per-pid and claimed with
`O_EXCL`. There is no `FINISH_BASE`.

The rest of the audit, including what was deleted and what was deliberately not
touched, is in [`tools/search/SEARCH.md`](search/SEARCH.md).
