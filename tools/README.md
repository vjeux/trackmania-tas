# tools

The instruments. Everything the findings in this repo were checked with, as
source you can build and run yourself.

```
tools/tmtraj/     the Rust crate: `tmtraj` plus 14 single-purpose binaries
tools/*.sh        the corpus-audit and video/render shell scripts
```

## Building

```
cd tools/tmtraj && cargo build --release
```

Edition 2021, one dependency (`miniz_oxide`). Binaries land in
`target/release/`. `gbx.rs` `dlopen`s the system `liblzo2` at first use to
decompress GBX bodies, so you need that library present; nothing else is
required.

Built from a clean checkout of exactly these files on a stock Linux box
(stable toolchain, `cargo build --release --locked`): compiles with no errors
and produces all 15 binaries.

### Running the self-test

```
TMTRAJ_GHOST_DIRS=/path/to/your/ghosts tmtraj selftest
```

The fixtures are game-recorded `.Ghost.Gbx` files and are **not** in this repo,
so without that variable the self-test finds nothing. It then prints
`NO CHECKS RAN` and exits non-zero — a run with an empty denominator is not a
pass.

Everything below is named by **the failure it catches**, because that is why
each one exists. A tool that reports OK is saying "no known defect", never
"verified".

### One change from the working copy

The crate as it stood on the render box **did not build**: `src/lib.rs` had
`pub mod recwrite;` commented out, so `main.rs`'s `use tmtraj::recwrite::…`
failed to resolve and the `tmtraj` binary never compiled (the 14 `src/bin/`
binaries did). `recwrite.rs` had been left behind on `flate2`, which is not a
dependency of this crate. Published here with the module re-enabled and its two
zlib calls moved to `miniz_oxide` — the same compressor the rest of the crate
already uses, level 6 as before — so the dependency list stays at one entry and
everything compiles. Nothing else was touched.

## The publish gate

| | catches |
|---|---|
| **`tmtrajcheck`** | *A file that validates to the exact millisecond and still plays back as a stranger driving another map.* The refuse-to-publish gate (`tmtraj check` as a standalone binary): ten checks C1–C10 that a ghost's telemetry must satisfy against itself and against the map — no reference recording needed. Non-finite positions (`err > tol` is false for NaN, so every naive gate *accepts* one), a car that never moved, samples after the finish line, a contact flag that disagrees with the trajectory's own ballistics, surface material accumulating in mid-air, a wheel radius that is not a car's, a throttle echo that contradicts the acceleration. Exit 0 publishable, 1 warnings, 2 REFUSED. Give it `--race <validated_ms>`; without it C4 is checked against the file's own declared time and reported as unchecked. |
| **`ghostqc`** | *Spending a render on a ghost that draws nothing.* A tape that validates as a time is not a tape that draws a car — validation reads the input chunk, the video reads the `CPlugEntRecordData` telemetry. Flags NAN / STATIC (position never moves) / ORIGIN (starts at the world origin, where no real start block is) / SHORT (<10 samples) / CREEP (<5 m of path). One line per file with start position, path length and top speed, so the numbers can be eyeballed rather than trusted. |

## Is this run really ours, and really independent?

| | catches |
|---|---|
| **`spawnq`** | *Telemetry that belongs to a different map, and a car that faces the wrong way for the whole clip.* Compares the first sample against a downloaded human recording of the same map (free on every map — every run spawns identically). Position > 2 m ⇒ different map (276874's first roof candidate began on one, with a clean container and a clean identity). Orientation `|dot| < 0.99` ⇒ wrong facing — 197047's filmed tape carried the identity quaternion where all 26 human recordings read `(3.39e-05, −0.7071, 0, 0.7071)`; its positions matched 1917 of 1917 samples and the car was sideways for the entire 100-second clip, because position lives at +208 and the quaternion at +192 of a 452-byte record. **It compares as a rotation, not as bytes**: `q` and `−q` are the same rotation, and a byte comparison condemns five perfectly correct 199100 files. A check that cries wolf gets switched off, and then it is not a check. |
| **`nearident`** | *A copy that has been through a float re-encode.* Flags two ghosts whose positions stay within `--mm` (default 1 mm) for `--run` (default 100) consecutive samples, at any integer lag. This replaced the exact-equality test, which is structurally blind to a re-encode: on 199100 that test returned "INDEPENDENT — no identical position at any lag" for a pair that is one run (mean 0.000476 m over its first 800 samples, byte-identical input tapes). **Calibrate before trusting**: the defaults were set against a known positive (199100 `51_TAS_47483_clean` vs `91_HUMAN_uelen_47838`, must flag) and a known negative (two of our own independent runs on the same map, must pass). Re-run both whenever you change them. |
| **`seplag`** | *Nothing, any more — this is the superseded exact-equality test.* It scans every integer lag for a run of exactly-zero distances, which is the right shape and the wrong tolerance; use `nearident` instead. Kept because it is what the published-ghost audit ran. Its own lesson is still live: `sep` walks two files index by index and bails out when the recorded sample times differ, printing to stderr — and sample times are *session* times, so two recordings from different sessions share no instants at all. All ten of 228607's files produced ZERO compared rows against `AUTHOR_LAP_20258`, and the pipeline read that silence as CLEAN. |
| **`sep`** | *"Not drawn" and "out of frame" being indistinguishable.* Per-sample 3D distance between two runs at the same race time, plus which one is ahead along its own path length. Written to make the two-car render test decisive: you need a frame where the cars are far enough apart to be two cars and close enough to both be in a chase cam trained on the first. Note the index-alignment caveat above. |

## Input tapes

| | catches |
|---|---|
| **`inputcount`** | *An overlay that treats video frame 0 as race 0.* Counts input events — samples where `(steer, gas, brake)` changes — and can emit the per-tick tape as CSV on the **race** time base. A recording begins before the countdown and ends after the line (126859's 23.545 run is a 27.85 s record), and `Decoded::start_ms` is 0 for every ghost here, so the lead-in is *measured* from the telemetry: the first sample more than 0.05 m from where the car was sitting. **`--meta` reports the SAMPLE span, and cannot see a foreign record span.** Its steer/gas/brake come from the 50 ms telemetry, not the 10 ms input chunk, which is known unreliable for ranking tapes by input count — six known counts, six mismatches. |
| **`inputchunk`** | *Quoting a telemetry-derived input count as if it were what the driver pressed.* Reads the real input archive, chunk `0x0309201D`, at 10 ms. The packed word is a union discriminated by its high bits; rather than guessing the bitfield — which is how wrong numbers get published — it prints the distribution and counts only what it can name. `--dump` for the raw entries. |

## Motion, read from the trajectory itself

| | catches |
|---|---|
| **`ballistic`** | *Believing the recorded contact flag.* A flag is a claim; the trajectory is evidence. In free fall vertical acceleration is G and horizontal velocity is constant, so it classifies every sample from central differences of its own position and cross-tabulates that against what the flag says. **G is −25.20 m/s², the fleet measurement from ten recordings split on their own contact flag — not Earth's.** With −22.3 the airborne class comes out empty and every "not airborne" assertion passes vacuously. Tolerance 6.0 m/s², which is generous because 50 ms sampling of a 25 m/s² field is genuinely noisy. |
| **`airtime`** | *A run that leaves the built track.* Answers "does the car fly off the map" from the recording rather than from squinting at frames: per-50 ms ground-contact flag, altitude and speed. Prints a profile plus fraction of the run on the ground, longest unbroken airborne stretch, and altitude range. |
| **`c3speed`** | *A fixed threshold on the wrong quantity condemning a published original.* C3 refused a file when consecutive positions were far apart, in metres — but a record can carry a gap: 286279 has a 650 ms hole, and at 131 km/h that is 23.7 m of ordinary driving. The quantity that separates a splice from a gap is distance **divided by elapsed time**, because the sample period is not constant. Bar: 200 m/s (720 km/h); the fastest thing in these recordings is a 546 km/h reactor run, and a genuine graft seam reads tens of thousands of m/s. |
| **`spdcheck`** | *Arguing about a big position step without asking the car.* `CSceneVehicleVis` records scalar speed per sample, independently of position. If position says 212 m/s and the speedometer agrees, it drove there; if the speedometer says 40 m/s, the position moved without the car. Prints the top steps with implied speed, recorded speed, and the ratio (verdict at 1.5×). |
| **`gapdump`** | *A check that straddles its own threshold depending on implementation detail.* Written to settle a disagreement where two tools read 212 m/s and 196 m/s on the same file across a 200 m/s bar. Lists every non-nominal sample interval with the speed it implies, and separately the worst step at the nominal period, so a gap and a teleport are told apart by inspection. |
| **`whlvar`** | *A false refusal caused by an "n/a" being read as a verdict.* "Can we infer a wheel radius" and "are the wheel bytes present" are different questions, and conflating them refused Nadeo's own recording: C8 needs ground-supported samples to classify, and a run that descends the whole way has none. This asks the direct question — do the bytes vary? Dead or donor-blanked telemetry is constant; 145875's download carries 88–109 distinct values per wheel, a zeroed field carries 1. Reports the **least** varying of the four wheels, because one dead wheel renders wrongly. Prints `<distinct> <samples> <changes> <span_ms>`. |

## The main binary

`tmtraj` is the decoder and racing-line analysis. Documented subcommands:

- `decode GHOST.Gbx [--csv|--json|--full-json OUT] [--head N]` — one ghost, header and samples
- `decode-all DIR... [--out-json DIR] [--out-csv DIR] [--jobs N]` — parallel bulk decode
- `fields` — per-field confidence table (VERIFIED / DERIVED / GUESS)
- `cluster`, `compare`, `stats`, `demo` — racing-line clustering and population analysis
- `selftest` — see below

Four more subcommands dispatch but are absent from its `--help`; their usage
strings are all the documentation there is:

- `rec info|roundtrip GHOST [--out OUT]` — read / re-encode / rewrite the record payload
- `recdiff A.Ghost.Gbx B.Ghost.Gbx [--csv OUT]` — raw per-sample byte diff of the two records' largest entity
- `hdr info FILE` / `hdr setlogin FILE OUT NAME` — GBX header fields, including the recorded login
- `body login FILE` / `body setlogin FILE OUT NAME` — the same in the compressed body

`src/whlcmd.rs` is the surface-and-contact instrument — it classifies each sample
BALLISTIC / SUPPORTED / UNKNOWN from the second difference of its own position,
then asks *two* questions of the contact flag, because a zeroed field also stops
the dirt-in-mid-air effect and so passes any single-sided test. In this tree it
is a library module behind `tmtrajcheck` (C5–C8), not a `tmtraj whl` subcommand:
the `whl` CLI lives in the upstream `whl_tools_v1` workspace, which is not here.
`checkcmd.rs` is likewise reachable only through the `tmtrajcheck` binary, not as
`tmtraj check`.

### `tmtraj selftest` needs its fixtures

It validates the decoder against independent ground truth (map geometry, the
ghost's own split-time chunk, and a hand measurement of six 4 m gates) using two
reference ghosts, `01_19538.Ghost.Gbx` and `slow_p10000_19812.Ghost.Gbx`, looked
up in `/tmp/m1/ghosts`, `/tmp/m1`, `/tmp/tmtas/tmtas/ghosts`, `/tmp/entrec/ghosts`
(`GHOST_DIRS` in `src/selftest.rs`).

**Without them it prints `SELFTEST: ALL PASS (0 checks, 0 failed)` and exits 0.**
That is a vacuous pass, not a passing test — read the check count, not the word
PASS. The fixtures are not in this repo and were not preserved on the machine the
crate came from.

## Shell scripts

| | catches |
|---|---|
| `ship-clip.sh` | *A clip that plays for you and 404s for everyone else.* Publishes one rendered mp4 so a **logged-out visitor** can watch it: settle and probe the file, upload the original to the `videos-v1` release, upload to GitHub's user-attachments store, register the asset URL in the release body — then fetch it with no credential at all and require 200 and playable bytes. Every step refuses rather than warns; only the anonymous fetch decides that it is published. |
| `skincheck.sh` | *A custom car skin in the video.* Refuses any ghost carrying one. Every other identity field is metadata; this is the paint on the car. 276874's two WATCH tapes read login `TAS`, carried no account id and imported as `Ghost:TAS` — clean on all three readers — while carrying a `Skins\Models\CarSport\...` zip and its Nadeo storage URL. |
| `splitscreen.sh` | *A "comparison" that is one car and a caption that lies.* Two runs side by side, for maps where a chase camera cannot hold both: on 276877 the human record is 61.5 m away and on 228607 it is 356.68 m away, i.e. behind the camera for the whole run. Runs on the render box — the Mac's ffmpeg has no libfreetype, so `drawtext` is unavailable there. |
| `ghost-splice-audit.sh` | *A published ghost whose telemetry is another driver's.* A corpus-wide audit, one TSV row per file. Three tests, and it says which of them is proof: a shared PREFIX is worthless (our own sibling tapes are 67 % bit-identical on 203072), wholesale identity with a human recording means the file simply *is* that recording, and **re-convergence** — identical, then more than 5 m apart, then exactly identical again — is the only one that can only be a splice. Files whose names declare them human-derived (`AUTHORCUT`, `AUTHOR_LAP`, …) are reported `DERIVED-AS-LABELLED`, and a map with no human recording is `NO-HUMAN-REFERENCE`, which means UNTESTED and never clean. Its exact-zero tests predate `nearident` and are blind to a float re-encode; the header says so. |
| `sep-truncation-scan.sh` | *A CLEAN verdict produced by an instrument falling silent.* `sep` bails out when two files' recorded sample times differ and says so only on stderr, which every pipeline here discards — and sample times are *session* times. All ten of 228607's files produced ZERO compared rows against `AUTHOR_LAP_20258` and the pipeline read that silence as clean. Scans every our-file/reference pair and flags any comparison whose row count falls short of `min(samples)`. The absent-signal bug, hunted in our own instrument. |
| `jump-recheck-speedometer.sh` | *A distance threshold condemning a published original.* Re-grades every C3/C4 refusal against `CSceneVehicleVis`'s own recorded speed: ratio under 1.5 is driving and the refusal was false, a recorded speed of exactly 0 is a respawn, and the thousands are a splice (227654: 50090 m/s implied against 19.2 recorded). This is the pass that emptied a 24-file work queue across 8 maps. Also prints the last sample's offset from the declared race time, because a record can stop short of the line. |
| `record-stops-short-scan.sh` | *Filming a finish that is not in the file.* The tail past the line is the familiar shape; this is its opposite, and nothing else looks for it. 126859's published files end 95 ms short of their declared race time, so the crossing is simply not in the record. Flags any file more than 60 ms either side, across the whole corpus. |
| `gamebot-drive.sh` | *A click that was never delivered looking exactly like a click that was ignored.* One action per call — click (1280×720 coordinates, scaled ×3), key, screenshot, plugin state. `powershell.exe` is a Windows binary and rejects `/mnt/c/...` paths including its own `-File`; with stderr discarded, ffmpeg then re-encoded the PREVIOUS screenshot, so the screen looked frozen and every click looked ignored. `shot` prints the PNG's timestamp beside the wall clock: freshness is stated, never assumed. |

The five above were rescued from the render box's `/tmp` on 2026-08-21, out of
112 loose scripts written over one night. Fifty-nine more were kept but not
published — one-off sweeps whose output mattered, and the working copies of the
render box's own UI automation — and are banked with a manifest at
**F1994652705**, together with the eight `.tsv` outputs including the corpus
audit table. The remaining forty-eight were
scaffolding and are gone.

## Filming

The rules a clip is shot under are in [`../FILMING.md`](../FILMING.md) — camera
always on our car, both runs in one scene, and what makes an asset public.

| | catches |
|---|---|
| `render.sh` | a one-car clip: no OCR, nothing judged by eye; refuses a camera that targets nobody |
| `render2.sh` | **the two-car shot**. Imports the TAS ghost first so the camera follows ours, measures the separation and refuses a pairing that would silently look like one car, and reads each imported track's nickname — a tape built on someone else's recording comes back wearing their name |
| `splitscreen.sh` | only for maps where a chase camera provably cannot hold both cars |
| `ship-clip.sh` | a clip that is 404 to everyone but us: registers the URL in the release body, then fetches it back under `env -i` with no credential |

## `tmtraj intg` — the publish gate

The gate is now **source in this crate**, not a tarball. `tmtraj intg gate
GHOST --race MS --refs refs.tsv --mapid ID [--server DIR --map MAP.Map.Gbx]
[--source SECOND_GENERATION] [--require-manifest]` exits 0 publishable,
2 refused, 3 unmeasured — and **3 is never folded into 0**: an input the gate
could not read is not a verdict about the ghost.

It is the thing that decides publishable. `tmtrajcheck` is the weaker check and
it is the one that passed a contaminated file.

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

~0.5 mm is the client-vs-server floor. Run one before believing any verdict
about a file you made, and never carry another map's reading over: winning
parameters do not port.

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
