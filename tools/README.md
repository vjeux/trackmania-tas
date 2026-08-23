# tools

The instruments. Everything the findings in this repo were checked with, as
source you can build and run yourself.

```
cd tools && cargo build --release && cargo test --release
```

One workspace, one command to build it, one command to test it. Edition 2021,
one third-party dependency in the whole tree (`miniz_oxide`); `liblzo2` is
reached with `dlopen` at run time, so nothing has to be installed to build.

**There are no shell scripts and no Python here, anywhere in the pipeline.** If
something needs doing, it is a subcommand.

| crate | what it is | its own docs |
|---|---|---|
| `gbx` | the GBX file format, once: container, chunks, the 10 ms input tape, the `CPlugEntRecordData` telemetry record, the 116-byte vehicle sample | in-crate |
| `tmtraj` | read-only analysis of a run: decode, compare, the publish gate, corpus scans, racing-line clustering | [`tmtraj/README.md`](tmtraj/README.md) |
| `tmmaps` | `.Map.Gbx` surgery: the census, region moves, segment maps, ladders | [`tmmaps/U02-AUDIT.md`](tmmaps/U02-AUDIT.md) |
| `ghost` | every mutation of a ghost or replay, the plain oracle, the publish decision | [`../GHOSTS.md`](../GHOSTS.md) |
| `tmsite` | the 3D visualisation page and the TICK input-script export | [`tmsite/README.md`](tmsite/README.md) |
| `clip` | publishing a rendered clip so a logged-out visitor can watch it; the side-by-side shot; the trainer playtest | [`clip/README.md`](clip/README.md) |
| `shootctl` | driving the game to render a clip | `RENDER-PIPELINE.md` |
| `fk` | the live engine: re-simulate a tape and read the car's state per tick | in-crate |
| `forkoracle` | the fork server and its shim | in-crate |
| `testdata` | the shared fixture corpus every crate's tests read | — |

`fk` and `forkoracle` are separate workspaces (they pin `-O3` + LTO for the
engine paths) and are built from their own directories. So is `search`
(`tmsearch` + the fork oracle and its shim), for the same reason: `cd search &&
cargo test --release`.

`tmmaps` **is** a member of this workspace. It was in none — neither a member
nor excluded — which is not a slow build but a hard error: `cargo build` inside
it refused to do anything at all ("current package believes it's in a workspace
when it's not"). One command builds and tests every crate a fresh clone can
build.

## The division of labour, and why it is drawn there

**`tmtraj` reads. `ghost` writes. `gbx` is the format, once.**

A tool that can only read can never be the thing that corrupted the file, and a
format implemented once cannot drift against itself. Both halves of that were
paid for: the format used to be implemented three times — in `tmtraj`, in
`ghost` and in `tmsite` — and `tmtraj` carried nine commands that rewrote a
ghost.

Two implementations of one file format is how a project gets silent corruption:
a fix lands in one reader and the other keeps decoding the old way, and nothing
fails. It is not hypothetical here. `ghost` modelled the recorded pedal bytes as
255-or-0 while `tmtraj`'s field table modelled them as `/255` floats, and the
disagreement was invisible until the two shared a crate — at which point it took
one command to find that 40 samples across 4 files, one of them a downloaded
human recording, carry a pedal byte that is neither.

## Where each question is answered

| question | command |
|---|---|
| what does this file say the car did | `tmtraj show`, `tmtraj export`, `tmtraj fields` |
| what inputs does the record carry | `tmtraj inputs` (the 50 ms echo) · `ghost tape extract` (the 10 ms tape — what the driver pressed) |
| are these two recordings the same run | `tmtraj diff` (`--lag`, `--near`, `--bytes`), `tmtraj spawn` |
| is this a physically coherent run of a car | `tmtraj check` |
| is it publishable | `tmtraj gate` (physics, contamination, provenance) and `ghost verify` (container, tape, identity, the engine) |
| is anything wrong across the whole corpus | `tmtraj corpus splice \| span \| qc \| bytes` |
| what does the racing line look like across a field | `tmtraj lines` |
| what differs between two copies of the same module, or two maps that share one | `tmtraj blockdiff` (per-block three-axis residuals in one frame, and `NO-IMAGE` when a block has no counterpart) |
| change a ghost | `ghost` — trim (which sets a run's length in BOTH directions), declare, identity, map, tape, regen |
| re-simulate a tape | `fk` |
| draw it | `tmsite`, `shootctl`, `clip` |

## Filming

The rules a clip is shot under are in [`../FILMING.md`](../FILMING.md) — camera
always on our car, both runs in one scene, and what makes an asset public. The
render box's own UI automation lives on that box; what is here is the part that
decides whether a clip is published:

| | catches |
|---|---|
| `clip ship` | *a clip that plays for you and 404s for everyone else.* A pushed commit does NOT authorise a GitHub attachment for public serving — only a reference in content GitHub renders at save time does. We shipped 19 clips before learning that and 18 were 404 to everyone but us. The gate is an anonymous fetch under `env -i`: no cookie jar, no `GH_TOKEN`, no netrc. A gate that runs with credentials is not a gate. |
| `clip split` | *a "comparison" that is one car and a caption that lies.* On 276877 the human record is 61.5 m away and on 228607 it is 356.68 m away — behind the camera for the whole run. Only for maps where a chase camera provably cannot hold both cars. |
| `tmtraj diff A B` | the number that decides whether a two-car shot is possible at all: you need a frame where the runs are far enough apart to be two cars and close enough that both are in one chase camera. |

## Recipes that outlived their scripts

Fourteen more shell scripts lived at the root of the pre-repo cargo workspace
(`r165_tools_v5.tgz`): `build.sh`, ten `whl_*.sh` and three `r165_*.sh`. All
fourteen are gone. Thirteen were the batch drivers of two investigations that
have since closed — the wheel/surface field-map arm and the one-tick-late
regeneration repair — and they hard-coded paths (`/tmp/fk/rs`, `/tmp/w`,
`/tmp/maps`) into a tree that no longer exists. `build.sh` ran
`cargo build --release`.

Porting them would have been preserving dead work. What they knew that is still
true is here instead:

| family | what it asks |
|---|---|
| A `C1`–`C10` | is this a physically coherent run of a car |
| B `B-contam` | bit-exact against **every human recording held for the map**, race-windowed |
| C `C-oracle` | does the dedicated server re-simulate **the written bytes** to the declared time |
| `C-header` / `C-ident` | does the file declare its own time, under our login, with no account id |
| `C-spawn` | is the first in-race sample at the map's spawn **and facing the way every run on it faces** |
| E `E-stale` | is this a physics tick behind a second independent generation of the same tape |
| D `D-manifest` | does the file's own account of how it was made hold up |

`C-spawn` is new (2026-08-21). It exists because `fk regen` writes the engine's
rotation in whichever of three layouts the locate happened to find, the choice
varies between runs of the same command, and getting it wrong leaves every
position exact — so C1–C10, the oracle, the tape md5 and the whole contamination
family pass while the car faces the wrong way for the entire clip. Measured on
197047: the withdrawn file reads **179.998°** from the human spawn, its
replacement 0.010°.

### The answer key, per map

`tmtraj` cannot tell you it found the right car. A **downloaded human recording
of the same map, regenerated through the same pipeline and graded against its
own recorded bytes**, can:

| map | position | orientation | tick offset |
|---|---|---|---|
| 197047 | 0.489 mm | 0.0068° | +0.000025 m |
| 228811 | 0.483 mm | 0.0070° | +0.000011 m |

**≈0.5 mm is the answer key's own floor** — the number the pipeline returns when
it *is* pointed at the right car. Run one before believing any verdict about a
file you made, and never carry another map's reading over: winning parameters do
not port.

> **Do not call this "the client-vs-server floor".** That name is a claim about
> two engines differing, and **it has now been measured and it is false — for
> the position.** The ≈0.5 mm is **the distance between two copies of the car in
> the server's own memory, and the pipeline was reading the wrong one.**
>
> Measured 2026-08-22 on map 2, `human_22730`, against the game's own recording,
> one flag apart (`ghost regen --transform-from-fields`):
>
> | | transform from the located copy | transform from the copy with a live wheel block |
> |---|---|---|
> | worst separation | 0.001 m | **0.000 m** |
> | samples reproducing the recorded bytes | **0 of 455** | **227 of 455 (49.9 %)** |
> | position byte 51 | 0 of 455 | 396 of 455 |
>
> Bit-identity goes from zero to half the run. A floor between two engines cannot
> do that. The three maps that "agreed" at 0.489 / 0.511 / 0.501 were three
> readings of one quantity, not three confirmations — and the corpus said so
> independently, because it uses the same ≈0.5 mm for comparisons that cannot all
> be one thing: **270051 reads 0.000000 m ours-vs-ours where 173691 reads
> 0.000497 m on the same comparison.**
>
> This also **confirms a suspicion recorded on 2026-08-20 and not actionable
> then** — *"~0.0005 m is the signature of the shadow, not a measure of accuracy;
> a gather that found the car is bit-identical or ~0.000001 m"*. Cite it as a
> suspicion confirmed two days later, not as something nobody had noticed.
>
> **The ORIENTATION half is open, and got worse under the same change** (byte 60:
> 455 of 455 → 8 of 455). The quaternion is read at the anchor's offset relative
> to the position, which should transfer between copies of one struct and on this
> copy does not; the (x,y,z,w)/(w,x,y,z) order was tested and ruled out. So
> `--transform-from-fields` is **default OFF** and the publish path is unchanged.
> **Do not run the three-map round-trip as a verdict yet** — on the position it
> passes, on the orientation it regresses, and recording a mixed result as either
> would be wrong.

### `corpus dup` was silent for this whole lineage — 2026-08-22

Worth reading as a worked example of the failure this file is full of warnings
about. `tmtraj corpus dup` asks "do two published files of one map carry the
same recorded motion?", and it decides by first asking whether their **input
tapes** differ — if the tapes are identical, identical positions are expected
and the pair is excused.

It asked that question by shelling out to **`fk tapediff`**, which is not a
command this repo's `fk` has. The call failed every time; the failure was
swallowed by `.ok()?`; and `None` from that function means *the tapes are
identical*. So **every pair in the corpus was excused as
`identical-tapes / EXPECTED-SAME-INPUTS`, and the scan exited 0.** The check
that exists to catch one run published twice could not see anything at all.

Caught on 228607, where the scan called `SPLICE_24854` and `TAS_19907`
identical-taped while their trajectories are **357 m** apart and `ghost tape
diff` puts their first input difference at tick 72.

Three things to take from it:

* The `corpuscmd` header in this toolchain says the shell scripts were fragile
  because *"every one of them piped a tool's stdout through awk and discarded
  its stderr"*. **The Rust port reproduced the bug**: `.ok()?` is `2>/dev/null`
  with a nicer spelling.
* It is the **second** time `fk` not being reachable produced a wrong answer
  silently (`tools/search/SEARCH.md` has the first, where 24 attempts "failed to
  find the car"). The first failed toward a null. This one failed toward
  **clean**, which is worse, because a null looks like a result and a pass looks
  like nothing at all.
* Fixed by removing the subprocess: the comparison now runs **in process** on
  `gbx::tape`, which this crate already depends on. It has a positive control as
  a unit test — a tape must be identical to itself **and** two known-different
  runs must come back different — because a comparison that cannot fail is what
  shipped.

**What the repaired scan says, and what it does not.** Over the corpus it now
returns 607 `EXPECTED-SHARED-PREFIX`, 135 `REVIEW-SHORT-OVERSHOOT`, 46
`REFUSE-ONE-RUN-TWICE` and 8 `EXPECTED-SAME-INPUTS`, where before it returned
nothing but the last category.

**Those 46 have since been adjudicated against the engine, and the answer is
zero defects.** 14 fall inside 203330's *measured* per-tick inert window, 3 are
at separation exactly 0.000000 m (two of which the 227654 page already documents
by hand as one trajectory), 5 are the documented 286279 author-ghost provenance,
and the remaining 24 — 38 pairs at the finer verdict — were settled by
re-simulating both tapes: **35 INNOCENT-INERT-INPUTS, 1 inconclusive at
0.001 m, 2 untested, 0 defects** (`tmtraj adjudicate`, `tmtraj
adjudicate-batch`). The hypothesis offered here first — long no-authority
windows — was right, and is now measured rather than assumed. The 2 untested are
the turtle maps 238835 and 267859, where no file locates at any of 14 fork
points; that is a fact about the locate, not the files.

Two known limits of the scan itself: its countdown exclusion (ticks before
race 0, which the car cannot act on) is a modelling choice — including them
produced 35 refusals keyed at `diverge@-1.52s`, which is two drivers holding
different keys during the lights — and the respawn bit **is** compared, which
changed the corpus census by nothing (a checked negative).

### `C-route` — the record against the engine, read by a different instrument
`fk btraj2` re-simulates a ghost's tape and dumps the car's position per tick
without going near the record, so it answers "is this record this run?" from
outside the writer's own instrument.

```
fk btraj2 --template G.Ghost.Gbx --map M.Map.Gbx --shim libfkshim.so \
          --server /tmp/tmoracle/server --tick 2500 --out route.csv
tmtraj intg gate G.Ghost.Gbx --race MS --refs refs.tsv --route route.csv ...
```

**It scans integer tick offsets and reports the best one.** The first version
compared at lag 0 and reported a magnitude, and it convicted an honest file
within an hour of being written: 227654's record reads **0.5485 m at lag 0 and
0.0000 m at lag −1**, because 0.5485 m is exactly how far that car travels in
one 10 ms tick. *A magnitude cannot see which side of a tick a file is on* —
a sentence already in this project's notes about `C11b`, which did not stop it
happening again. **When a comparison produces a suspicious distance, scan the
lag before drawing any conclusion. It is two lines.** A time shift collapses to
zero at some lag; a different trajectory collapses at none.

A non-zero best lag is reported, not punished: tick alignment is a property of
the run, the regenerator is nondeterministic about it, and a solo clip cannot
look wrong from one tick. Judge it against the map's own control.

**`C-route` needs a control per map like everything else.** `fk btraj2` cannot
locate the car on every map — on 197047 it reads **1.7657 m against a file the
game itself wrote**, and on 227654 it will not locate at all on the human's own
download. A map where it fails its own control is `UNMEASURED` on this axis:
not clean, not convicted. That column is never folded into either.

### `tmtraj intg echo` — the record's input channel, and it needs no locate

A ghost's samples carry the steer/gas/brake the car was being given. Compare
that against a `tmtas trace` CSV of a tape and you learn which tape the record
was written alongside — with no fork server and no locate, so it works on maps
where `C-route` cannot run. On 197047: **100.0 % agreement with our tape over
1917 samples, 8.3 % with the human rank-1's.**

**Permanent caveat, do not let this be promoted:** the echo channel is written
from our tape even in a record whose *positions* came from somewhere else. It
answers "was this record written alongside this tape", not "are these positions
this run". It would not have caught the defect `C-route` was built for.

## tools/ghost — the ghost / replay API

```
tools/ghost/      one binary, `ghost`: extract and inject inputs, regenerate the
                  car state from the engine, change the map a recording runs on,
                  trim a run, edit the car skin / name / trigram, and verify any
                  of it
```

Build and run its whole test suite in one command:

```
cd tools/ghost && cargo build --release
TM_SERVER=<dir containing TrackmaniaServer> ./target/release/ghost selftest
```

The API, the file-format facts it is built on, and every trap it now prevents
are in [`GHOSTS.md`](../GHOSTS.md) at the repo root. `ghost --help` lists the
commands.

`ghost regen` — the one operation that needs the real physics engine — shells
out to `fk` (the fork-server state reader) and expects it on `PATH` or at
`$FK_BIN`. Everything else in this crate is self-contained: one dependency,
`tmtraj`, in this same directory.

## tools/tmmaps — the map API

```
tools/tmmaps/     one binary, `tmmaps`: read a map's blocks and items (both
                  chunks), move them by position, empty a region and prove it
                  empty, build segment maps, run arrival ladders, and drive the
                  dedicated server
```

```
cd tools/tmmaps && cargo build --release
TM_SERVER=<dir containing TrackmaniaServer> ./target/release/tmmaps selftest
```

**29 checks, seven checked-in fixtures, ~30 s, no external data.** One
dependency, `tools/ghost` by path — the ghost format is its job, so `tmmaps`
calls in for the reference ghost's declared splits rather than keeping a second
reader. [`MAPS.md`](tmmaps/MAPS.md) is the API and the traps;
[`U02-AUDIT.md`](tmmaps/U02-AUDIT.md) is the audit that produced it.

`tmmaps` and `ghost` do not overlap: `ghost` owns the ghost/replay format,
`tmmaps` owns `.Map.Gbx`, and `tmmaps` **refuses a recording by GBX class**
rather than reaching into a carried map. To edit the map a recording runs on,
compose them — `ghost map extract` → `tmmaps …` → `ghost map set`.

`u02` is deleted; all 24 of its subcommands are covered by one of those two, or
were one-off probe apparatus. `U02-AUDIT.md` says which, and why, with the
control behind each verdict.
* **Regenerating a published ghost from zero, in order** (`whl_regen_corpus.sh`,
  proven on three 203072 files: 0 of 215 samples bit-identical to the donor,
  from 245 of 245; in-race coverage 100 %; oracle exact; tape byte-identical):
  `ghost regen` for the telemetry, then `ghost trim --auto` to drop the donor's
  post-finish tail, then `ghost identity set --anonymise` so our runs carry our
  name. All three now refuse rather than warn, which is what the script was
  hand-rolling around each of them.
* **Fit a field map from at least three answer keys, or do not fit one**
  (`whl_keys*.sh`). With one downloaded recording as the key, `fk whl bytemap`
  reported 6 of 116 sample bytes available at 94–99 % exact; with five keys on
  the same map only 2 survived. The four per-wheel "dampen" hits had fitted one
  recording by coincidence.
* **A one-tick-late record is judged against the map's own control lag, not
  against zero** (`r165_*.sh`). Whatever lag a DOWNLOADED human recording sits
  at against the engine is the convention the format uses, and it is per-map and
  never 0 — the game's own recordings trip a naive stale-buffer test. That scan
  is now `tmtraj gate`'s `C-route`, which reports the best integer lag rather
  than a magnitude, because *a magnitude cannot see which side of a tick a file
  is on*.
* **A regeneration is nondeterministic at tick alignment, and run agreement is
  not an acceptance test.** One tape, one map, one binary, identical flags, five
  runs: four agreed and one was right. Two of the four wrong ones agreed with
  each other to the metre. Acceptance is the path length matching the map's own
  ribbon and the first sample matching the spawn — in that order — never a vote.

## tools/mapgeom — the map as real 3D geometry

```
tools/mapgeom/    one binary, `mapgeom`: read the game's own data pack, place
                  every block and item of a map at its true world position,
                  and write the result as glTF, OBJ or a picture -- then GRADE
                  it by dropping a plumb line from every sample of a run
```

Every earlier geometry result in this project was inferred: a deck height from
plumb probes, a route from the block graph, "the ice IS the road" from deleting
caps and watching what broke. This reads the shapes the game collides against,
with the physics material on every triangle, and lays a driven trajectory over
them in the same frame.

[`MAPGEOM.md`](mapgeom/MAPGEOM.md) is where the geometry lives, how the pack's
hashed asset names resolve, and what is still missing. The one number to know:
across thirty-three maps checked in one batch, **twenty put the car within a quarter
of a metre of the model and sixteen within 0.09 m** — and the maps where it
does not are the model missing a surface, which the same command says out loud.
