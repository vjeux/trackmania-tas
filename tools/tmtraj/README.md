# tmtraj — read-only analysis of a Trackmania 2020 run

```
cd tools && cargo build --release
tools/target/release/tmtraj --help
```

Times print as **seconds with a decimal** — `36.049`, never `36049`. A tick
index is a count and stays an integer.

---

## The one-line version

**`tmtraj` reads. `ghost` writes. `gbx` is the format, once.**

That boundary is the design. A tool that can only read can never be the thing
that corrupted the file, and a format implemented once cannot drift against
itself. Before this rebuild the format was implemented three times — here, in
`tools/ghost` and in `tmsite` — and `tmtraj` carried nine commands that
rewrote a ghost.

| where | what it owns |
|---|---|
| `tools/gbx` | the container, the chunks, the 10 ms input tape, the `CPlugEntRecordData` record and the meaning of the 116-byte vehicle sample |
| `tools/tmtraj` | analysis: what a trajectory says, whether two recordings are the same run, whether a file is a coherent run of a car |
| `tools/ghost` | every mutation, the plain oracle, the publish decision |
| `tools/tmsite` | presentation: the 3D page, the TICK input-script export |

---

## The commands

Six groups, and the top level says which question you are asking.

### What does the file say?

| | |
|---|---|
| `show FILE... [--head N]` | span, checkpoints, entities, first samples |
| `export FILE [--csv F] [--json F] [--full-json F]` | the decoded artefacts |
| `export --dir D... [--out-csv D] [--out-json D] [--jobs N]` | the same in bulk, threaded |
| `fields` | every decoded field with its confidence tier |
| `inputs FILE [--events] [--csv F]` | the steer/gas/brake the record carries, recovered exactly |

`show`'s first two lines carry the **sample count** and the **declared
checkpoints** deliberately: that pair is the two-second tell for a synthesised
tape carrying its template's telemetry. A poisoned file had 281 samples and
declared 14.018 where the clean regeneration of the same run had 280 and
13.984.

`fields` is not a convenience — it **is** what this project publishes about the
format, and a test asserts its 18 VERIFIED / 18 DERIVED / 12 GUESS split, so a
change to a claim has to be a deliberate commit rather than a build.

### Are these the same run?

| | |
|---|---|
| `diff A B [--rows] [--csv F]` | per-instant separation on the instants the two files share |
| `diff A B --lag` | the same with the time labels ignored, over every integer sample offset |
| `diff A B --near --control X Y` | a copy that has been through a float re-encode |
| `diff A B --bytes` | per-byte agreement of the raw samples |
| `spawn FILE... --ref R` | same start position **and attitude** |

**One invariant runs through all of it: an empty denominator is not a
measurement.** Every mode reports how many samples it compared and returns
`UNMEASURED` (exit 3) rather than a verdict from nothing. `sep` used to walk two
files index by index and stop at the first mismatched time key, printing to
stderr — and sample times are SESSION times, so two recordings made in
different sessions disagree at index 0. All ten of 228607's published files
were compared against `AUTHOR_LAP_20258`, produced zero rows each, and the audit
recorded ten CLEAN verdicts.

**A shared prefix proves nothing.** The simulation is deterministic; our own
sibling tapes are 67 % bit-identical on one 203072 pair. Only RE-CONVERGENCE —
identical, then more than `--minsep` apart, then exactly identical again —
cannot be driven.

**`--near` refuses without `--control`.** Half a millimetre means "copy" only if
a pair known to be two different runs measures much further apart on this map:
our own writer sits ~0.5 mm from the game's own recording of the same run
(0.482 / 0.483 / 0.489 / 0.518 mm on four maps' answer keys), which is inside
any 1 mm band. A verdict with no control cost four clips.

**`spawn` compares orientation as a rotation, never as bytes.** `q` and `−q` are
the same rotation; five 199100 files read `(−0.7071, 0, 0.7071, 0)` against the
humans' `(0.7071, 0, −0.7071, 0)` and are perfectly correct.

### Is it a coherent run of a car, and is it ours?

| | |
|---|---|
| `check FILE... [--race S]` | C1–C10, exit 0 clean / 1 warn / 2 REFUSED |
| `gate FILE... --race S --refs F --mapid ID [...]` | the publish gate |
| `manifest new\|verify\|show` | the provenance manifest |

`gate` exits **0 publishable, 2 refused, 3 UNMEASURED**, and 3 is never folded
into 0: an input the gate could not read is not a verdict about the ghost. Every
family it could not run says so on its own line.

### What does the trajectory say, as against what a flag claims?

| | |
|---|---|
| `motion FILE [--fit-g] [--per-sample]` | BALLISTIC / SUPPORTED / UNKNOWN from the second difference of position, with the recorded contact, dirt and ice bytes beside it |
| `wheels FILE` | the wheel radius, and separately whether the wheel bytes are alive at all |
| `facing FILE... [--ref R \| --route CSV]` | is the car pointing where it is going |

`motion` has **three** classes, not two: a car held up by a reactor, a boost or
a wall it is scraping is neither in free fall nor ground-borne, and a two-class
rule has to call it one of them. `--fit-g` fits gravity from the file's own
longest free-fall stretch and reports the vertical-speed range it was fitted
over, because a fit whose lever arm is a few m/s of `v_y` cannot identify a drag
term and must not be quoted as if it had.

`wheels` answers two questions separately because conflating them produced a
false refusal of Nadeo's own recording: "is there a wheel radius" needs
ground-supported samples, and a run that descends the whole way has none — an
n/a there is a statement about the CHECK, not about the file.

### Every published file at once

| | |
|---|---|
| `corpus splice --root R [--extra MAPID=FILE]` | telemetry that is another driver's |
| `corpus span --root R [--tol MS]` | a record that stops short of the line, or runs past it |
| `corpus qc --root R` | pre-render QC, and the car skin |
| `corpus bytes --root R` | which of the 116 sample bytes ever vary |

A map with no human recording is `NO-HUMAN-REFERENCE`, which means UNTESTED and
never clean.

### A population of runs

`lines report|matrix|stats|demo --dir D` — clustering and population analysis.
`demo` runs on two synthetic lines and needs no data.

---

## What was deleted, and why

The command surface went from about sixty entry points to twenty. This is the
account of the cull; if something you used is missing, it is here.

### Moved to `ghost`, because `tmtraj` no longer writes

| was | is |
|---|---|
| `tail fix` | `ghost trim --auto` |
| `recspan --end/--trim-all` | `ghost trim` |
| `setdecl` | `ghost declare` |
| `anon`, `hdr setlogin`, `body setlogin` | `ghost identity set --anonymise` |
| `rec info\|reencode\|roundtrip` | `ghost selftest`'s codec-identity check |
| `rectime` (shift the record's instants) | `ghost` — it writes |
| `intg corrupt` (a positive-control writer) | `ghost tape inject` |

### Folded into a command that answers the same question

| was | is |
|---|---|
| `sep`, `nan cmp`, `recdiff`, `intg pair` | `diff` |
| `seplag`, `intg lag` | `diff --lag` |
| `nearident` | `diff --near` |
| `spawnq`, `nan spawn` | `spawn` |
| `ghostqc`, `census`, `skincheck.sh` | `corpus qc` |
| `whl air`, `whl gate`, `ballistic`, `airtime` | `motion` |
| `whl roll`, `whlvar` | `wheels` |
| `whl grav` | `motion --fit-g` |
| `c3speed`, `spdcheck`, `gapdump`, `intg c3` | C3 in `check` (see below) |
| `inputchunk` | `ghost tape extract` — the input chunk is the tape, and the tape is `ghost`'s |
| `inputcount` | `inputs` |
| `cluster`, `compare`, `stats`, `demo` | `lines report\|matrix\|stats\|demo` |
| `tmtrajcheck` (a deployment shim binary) | `check` — one binary now |

### Deleted as one-off probes

`intg sweep`, `intg echo`, `intg selfsim`, `intg qrule`, `intg poison`,
`intg tapecsv`, `intg md5`, `intg stale` (CLI; the engine stays, the gate calls
it), `intg c11b` (see below), `whl dump`, `whl cmp`, `whl bits`, `whl calib`,
`whl surv`, `whl twoway`, `nan pick`, `nan vres`, `nan csvcmp`, `nan lag`,
`tail plan`, `tail verify`, `tail finishcheck`, `tail apply`.

Each was the manual face of one investigation, written so a person could eyeball
one number during one night's work. Where the engine behind one is load-bearing
it is still here and the gate calls it; what went is the command line.

### Three that were worse than unused

* **`intg c12` was a correct check wired to nothing.** It plugs `B-contam`'s
  documented blind spot — the near-copy that is never byte-equal — and no
  pipeline ran it. **It is now a gate check.**
* **`intg c3` was the corrected teleport test, and the gate ran the old one.**
  A bar on distance, or even on implied speed alone, cannot tell a teleport
  from fast driving, and these cars are very fast: it produced a work queue of
  24 files across 8 maps that were mostly never broken. **Its speedometer rule
  is now C3 itself.**
* **`intg c11b`'s CLI could only ever print `NO-VERDICT`**, because it called
  `c11b_verdict` with a hard-coded `control = None`, so the `MATCHES-THE-GAME`
  and `STALE-BUFFER` arms were unreachable from the command line. Its lesson
  survives in `C-route`, which scans the lag: *a magnitude cannot see which side
  of a tick a file is on.* 227654's record reads 0.5485 m at lag 0 and 0.0000 m
  at lag −1, because 0.5485 m is exactly how far that car travels in one 10 ms
  tick, and the first version of the check convicted an honest file inside an
  hour.

### Shell scripts

Seven are gone. Five had jobs that still matter and are now subcommands; two
had jobs that no longer exist.

| script | outcome |
|---|---|
| `ghost-splice-audit.sh` | ported → `corpus splice` |
| `record-stops-short-scan.sh` | ported → `corpus span` |
| `skincheck.sh` | ported → `corpus qc` (and `ghost identity show`) |
| `ship-clip.sh` | ported → `clip ship` |
| `splitscreen.sh` | ported → `clip split` |
| `trainer/playtest.sh` | ported → `clip playtest` |
| `jump-recheck-speedometer.sh` | **deleted as dead work.** It re-graded every C3/C4 refusal against the car's own speedometer, one work queue at a time. The speedometer rule is now C3 itself, so there is no queue of distance-rule refusals to re-grade. |
| `sep-truncation-scan.sh` | **deleted as dead work.** It hunted the silent-comparison bug in our own instrument — a `CLEAN` verdict produced by `sep` falling silent. The instrument no longer has that bug: `diff` and `corpus splice` report coverage per pair and refuse a verdict on zero rows. A scan for a bug you have made impossible is dead work; the lesson is now an invariant with a unit test. |

---

## Tests

```
cd tools && cargo test --release          # everything, ~90 s
```

One command covers `gbx`, `tmtraj`, `ghost`, `tmsite` and `clip`. Fixtures are
checked in under `tools/testdata` and resolved from each crate's manifest
directory, never searched for relative to the cwd.

**A test that skips when its fixture is missing is a test that passes when it
tests nothing.** `golden_full_fields` used to look for `/tmp/entrec_full`, print
`SKIP` and return ok — and that directory could only be regenerated by running
the Python this crate replaced, which is not on any box any more. It had been
green and vacuous for its whole life. `tmtraj selftest` had the same shape: with
none of its four `/tmp` fixture directories present it printed
`SELFTEST: ALL PASS (0 checks, 0 failed)` and exited 0. Both now fail, naming
the file. Where a tier genuinely cannot run here — a live `gh` release, a
headless browser — it **skips loudly to real stderr with a named reason**, and
`ghost selftest --strict` makes a skip a failure.

### What each suite proves

| suite | what it is evidence for |
|---|---|
| `golden_decode` | the 29 CSV columns and the path JSON, **byte-identical** to what a different implementation produced from the same 45 ghosts. This is correctness, not a behaviour lock. |
| `golden_full_fields` | all 48 fields, by digest. A behaviour lock, and it says so: it proves the other 19 cannot change unnoticed, not that they are right. |
| `golden_cluster`, `golden_stats` | the racing-line maths against the same reference implementation |
| `cli` | the command line over checked-in fixtures, including the gate end to end at the no-server tier |
| unit tests | the rules that used to be buried inside a command: the C8b acceptance bar, the C12 growth bar, `outcome`'s empty-denominator rule, C3's three jump classes, `q` vs `−q`, the steer byte's exact inverse, seconds formatting |
| `ghost selftest` | 44 checks against the real dedicated server |

### The controls that say the suite bites

Eight deliberate breakages, each reverted:

| mutation | caught by |
|---|---|
| the gate's wheel acceptance bar `0.15 → 0.95` | `the_c8b_bar_admits_a_drift_map_and_refuses_noise` |
| C12's growth bar `10 → 1e9` (the near-copy check never fires) | `c12_growth_bar_separates_the_two_populations` |
| zero compared rows reads as clean | `an_empty_denominator_is_never_a_clean_result` |
| C3 drops the speedometer witness | `a_teleport_a_respawn_and_fast_driving_are_three_different_things` |
| `is_ground_contact` bit `0x01 → 0x02` | `reproduces_the_51_python_trajectories` |
| `turbo_time` `/255 → /254` | `reproduces_the_51_python_trajectories` |
| orientation compared as bytes instead of as a rotation | `q_and_minus_q_are_the_same_rotation` |
| times printed as raw milliseconds | `seconds_with_a_decimal` |

The first of those is the point of the exercise: **before this work it was
caught by nothing.** The whole eight-test suite stayed green while the publish
gate's acceptance threshold moved from 15 % to 95 %.

---

## Facts the code paid for

Kept here because the code that encoded them is gone or has moved.

**The record locator, and a constant that was right once.** `anon` used to bound
its string scan at a flat 64 KB, on the reasoning that the identity lives at the
front and the compressed record follows it. True of the file it was written for.
On 227654 the identity chunk sits AFTER a 76 KB record, so the login, the
trigram and the zone were all past the limit: the anonymiser replaced four
strings, reported success, and left the account id in the file.

**The entity list is interleaved**, and getting it wrong is the one mistake that
still produces a plausible-looking blob. The reader is
`hasNext; while hasNext { fields; deltas; hasNext = u8; deltas2 }` — the flag for
the NEXT entity sits between this entity's samples and its own deltas2 list.

**Some ghosts carry two `CSceneVehicleVis` entities**, a heavily decimated one
(6–7 samples, ~3 s apart) and the real full-rate track. Take the one with the
most samples — and check the class id, not just the size: 165922's donor carries
175 815 samples of the undecoded `0x2D001000` entity spanning its whole 2.4-hour
session, and two modules used to pick by size alone.

**Encode what the engine holds, sign and all.** `q` and `−q` are the same
rotation and both encodings decode to it, but the game writes the one it has:
143 of one ghost's 474 samples carry `qw < 0`. Forcing `qw >= 0` re-encoded
those to different, still correct bytes — "equivalent but not identical", which
is what hides a real error later.

**The declared time is not one field.** It lives in `0x03092005`, `0x0309200B`,
`0x0309201B` and `0x0309202B` (whose last split is the finish), across five or
six sites. And the compressed telemetry payload must never be scanned for it:
four bytes of a zlib stream that happen to match are not a time.

**`hdr setlogin` must not be used on a file whose user-data size is zero** —
everything is in the body, so the header path misreads the body as a chunk table
and produces a longer, corrupt file (measured: 5263 → 10436 bytes, and it
overwrites the map uid). This is why identity editing is one command in `ghost`
now, with an oracle no-op control.

**NaN accepts every threshold test.** `err > tol → reject` ACCEPTS a non-finite
value, because every comparison involving NaN is false. Four published ghosts
carried one and every gate in the pipeline passed them at "OK, 100 %". And
`is_finite()` is necessary, not sufficient: two of eight regenerations of 270051
produced a trajectory that was exactly (0,0,0) at every instant — finite,
internally consistent, unit quaternion, and 1082 m from where the map starts.
Zeroed memory has the right shape too.

**A fixed threshold on the wrong quantity** is the shape of nearly every
correction in this file. C8's 0.36 m assumed the Stadium wheel; gravity's −22.3
assumed Earth (it is −25.20 here, measured from ten recordings split on their
own contact flag, and −22.3 makes the airborne class come out empty so every
"not airborne" assertion passes vacuously); C3's metres assumed a fixed sample
period, and a 650 ms hole at 131 km/h is 23.7 m of perfectly ordinary driving.

**`tmtraj`'s oracle parser does not have the `"Time"`-prefix bug** that bit the
fk arm. `parse_oracle` checks `"ValidatedResult" : null` explicitly and anchors
its search after that key, so a DNF cannot read back as a finish. Nothing in
`intg` consumes a race end from a parser of that shape — the race window comes
from `--race` or from the file's own `0x0309202B` chunk.

---

## Open, and stated rather than papered over

**91 of the 116 bytes of every sample are still the carrier's** after `ghost
regen` writes the 22 transform bytes and the three input-echo bytes. That is a
harness limit, not a physics limit: the engine computes every one of them and
they are in its memory. `corpus bytes` now enumerates the set instead of
shrugging at it — over the 158 published files, **28 of the 116 bytes are LIVE
and unnamed**: 0, 1, 4, 19, 20, 22, 24, 26, 28, 30, 32, 33, 34, 39, 40, 41, 42,
43, 69, 70, 71, 72, 73, 79, 80, 103, 108, 109. A byte that is constant
across the corpus has nothing to find; a byte that is constant per file is an
identity, not a signal; these are neither.

**Bytes 0 and 1 carry like one little-endian u16** — 86.7 % of 2237 low-byte
wraps are matched by a ±1 step in byte 1 — so one 16-bit quantity is being read
as at most one byte. The controls printed beside it: `side_speed`'s documented
u16 at bytes 2,3 scores 96.4 %, and two byte pairs inside the f32 position field
score 100 % and 99.7 %. **The same table refutes my own first hypothesis**, that
`rpm_raw` at byte 5 is the high half of a u16 rpm: bytes 4 and 5 carry on 10.5 %
of 62 211 wraps, so they are not one u16 and the "unknown rpm scale factor" is
not a missing low byte.

**The recorded steer byte is not always on the tape's i8 grid.** The corpus
model is `floor((steer_i8 + 127) * 255 / 254)`, whose image covers 255 of the
256 byte values; the one it misses is **254**, and 254 occurs 84 times across 27
files — **including one downloaded human recording**, so it is not something our
writer does. 254 is exactly what a `round` produces at `steer_i8 = 126`. Two
readings are open: a second encoder path in the game, or a record written from a
pre-quantisation analogue value. This matters because the fk arm has just taken
a verification statistic from kappa 0.467 to 1.000 by switching `round` to
`floor`; that fix is right for the fixture it was measured on, and this says the
other spelling also occurs. **Next step: read byte 14 out of engine memory
during a regeneration and see which value the engine holds.**

**The pedals are not always digital.** `ghost` models bytes 15 and 18 as 255-or-0
and `tmtraj`'s field table models them as `/255` floats — the two tools
disagreed, which only became visible once they shared a crate. Measured: 40
samples across 4 files carry a pedal byte that is neither 0 nor 255, one of them
a human recording. `inputs` reports these as `ANALOGUE(0x..)` rather than
rounding them to the nearest legal value.

**What `inputs` is not.** It is what the RECORD says the car was given, on a
50 ms resampling of a 10 ms channel. It is not an input count: six
README-stated counts were once checked against telemetry and all six were wrong
(14→17, 23→16, 19→16, 15→59, 2→523, 3→1753). For what the driver pressed, read
the input chunk with `ghost tape extract`.
