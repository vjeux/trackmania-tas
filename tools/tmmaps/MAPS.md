# MAPS.md — the TM2020 map API

Everything this project does to a `.Map.Gbx`, in one Rust binary, with a
control behind every operation. Its counterpart is
[`GHOSTS.md`](../../GHOSTS.md) / `tools/ghost`, which owns the ghost and replay
format. **The two never overlap**, and the boundary is enforced in code: hand
`tmmaps` a recording and it refuses, by GBX class, with the composition to use
instead.

```
cd tools/tmmaps && cargo build --release
TM_SERVER=/path/to/TrackmaniaServer-dir ./target/release/tmmaps selftest
```

`tmmaps --help` lists every command. Times print as **seconds with a decimal**
(`16.316`), never as raw milliseconds. The one exception is `segments.json`,
which is machine-readable and keeps integer ms.

---

## 1. The operations

| | command | what it does |
|---|---|---|
| **read** | `tmmaps waypoints MAP` | spawn, checkpoints, goal — the indices every mover takes |
| | `tmmaps census MAP [--filter PAT] [--free]` | **every** block, unbaked *and* baked, with its real position, as TSV |
| | `tmmaps region MAP --box A:B [--filter PAT]` | everything inside a world box. **A gate is a structure, not a block** |
| | `tmmaps chunks MAP` | every skippable body chunk with its size |
| **change** | `tmmaps move MAP --out F --move SPEC…` | position-only surgery: grid cell, free block, baked free block, item |
| | `tmmaps rotate MAP --out F --rot\|--drot BLK:y,p,r · --tilt N,N --about X,Y,Z --dir DEG --angle RAD` | rotate FREE blocks — the only non-position surgery here, and still no model swap |
| | `tmmaps clear MAP --out F --box A:B --to X,Y,Z` | empty a region **and prove it is empty** in the written file |
| | `tmmaps segments MAP --ref-ghost G` | a map per checkpoint, each verified against the ghost's own split |
| **measure** | `tmmaps ladder MAP --spec F --ghosts G…` | arrival-time ladders, with an origin control and a distinctness assert |
| | `tmmaps rungspec TRAJ.csv …` | emit a ladder spec on a reference trajectory |
| | `tmmaps oracle --map M --ghosts G…` | drive the dedicated server over (map, ghosts) batches |
| | `tmmaps cporder MAP TRAJ.csv --splits …` | which waypoint produced which declared split |
| **control** | `tmmaps selftest [--strict]` | the whole suite, one command |
| | `tmmaps roundtrip MAP` | parse and re-emit unchanged, on decompressed bodies |
| | `tmmaps origin MAP` | every mover, at its own placement, must reproduce the file byte for byte |
| | `tmmaps renamecheck MAP` | the rename round trip, which `roundtrip` cannot be |

### The composition with `ghost`

A recording that carries its own map is a `ghost` problem until the map is out
of it:

```bash
ghost map extract R.Replay.Gbx --out m.Map.Gbx
tmmaps clear m.Map.Gbx --out m2.Map.Gbx --box 1260,55,405:1385,130,475 \
                       --to -3000,-3000,-3000 --filter GateExpandable
ghost map set R.Replay.Gbx R2.Replay.Gbx --map m2.Map.Gbx
```

This is the only way to change the map a recording runs on. `ghost map set`
splices the bytes you give it in unchanged, and `tmmaps` never touches a
replay, so there is exactly one implementation of the embedded-map chunk and
exactly one set of block movers in the project.

#### Worked end to end, on 173691, with the numbers

Three maps built from the author's own file, the same landing run swapped onto
each, all through the plain oracle:

| map | `tmmaps region … --filter GateExpandable` | oracle |
|---|---|---|
| **A** the author's map, gate intact | **16** in the box | **37.229** |
| **B** the unbaked anchor moved away — *what the first pass did* | **15** left | **DNF** |
| **C** `tmmaps clear` — all sixteen moved | **0** left | **DNF** |

**B and C are indistinguishable to the oracle.** That is the whole trap, and it
is why it survived: the difference between them is 77.8 m and 249 m of driving,
and the only instrument the project had returned one number per run. `region`
tells them apart before anything is simulated, in a command that costs nothing.

---

## 2. The file format, as measured

| what | where |
|---|---|
| blocks | chunk `0x0304301F` — index, name, `dir`, three cell bytes, flags |
| **baked** blocks | chunk `0x03043048` — **a second list**, indexed from 0 in its own right |
| items (anchored objects) | chunk `0x03043040`, which carries **two** size words that must agree |
| free-block placement | chunk `0x0304305F` — `u32 version`, then **24 bytes per free block**: `Vec3 position`, `Vec3 pitchYawRoll` |
| waypoint property | `CGameWaypointSpecialProperty`, class `0x2E009000`, tag in {Spawn, Checkpoint, Goal} |
| GBX class of a map | `0x03043000` (`CGameCtnChallenge`); a ghost is `0x03092000`, a replay `0x03093000` |

The free-position stream runs **unbaked free blocks first, then baked free
blocks**, and the walk is required to land exactly on the chunk end. That end
assertion matters: an assertion that only counts entries cannot fail on a wrong
ordering, and a walk with no end check cannot fail at all. Both halves used to
be held separately and neither was checked.

**A block's cell is not its position.** A grid block is placed by its three
cell bytes; a **free** block ignores them entirely and lives in `0x0304305F`.
World point of a grid cell: `(32·cx + 16, 8·cy − 62, 32·cz + 16)`.

**Waypoint tags in the file are ignored by the game.** What a waypoint *does*
is decided by the block model (`RoadTechCheckpoint` vs `RoadTechFinish`) or the
item model (`GateCheckpointLeft32m` vs `GateFinish32m`). Retagging a block
Checkpoint → Goal, or deleting its `waypointParams` outright, changes nothing:
four experiments, all four returning the reference ghost's own 19.538 / 4 CPs.
So the surgery is on models, and that is why it is dangerous — see below.

---

## 3. The traps, and what the tool now does about them

### A GATE IS A STRUCTURE, NOT A BLOCK

On 173691 the map's author added a finish gate. Moving what looked like its
anchor produced a map that loaded, an origin control that passed, an oracle
that answered — and a car that drove into the invisible remains and stopped
77.8 m onto the deck. vjeux saw it in the video before any instrument did.

The census says the gate is **sixteen** blocks. (The banked write-up said "an
unbaked anchor plus fifteen baked pieces"; it is **four unbaked and twelve
baked**, and the real vertical extent reaches **y 64**, not the y 96…121 that
was written down — three `…RightVFC` pieces sit at (1271, 64, 41x). A box typed
off the banked numbers misses them.)

→ `region` counts what is in a box. `clear` moves everything in it, **re-reads
the file it wrote**, and exits 3 while anything is left. The suite carries all
three directions on the real map: the structure is 16 pieces; moving one anchor
leaves 15; clearing leaves 0 — and the edited map still round-trips.

And the reason the trap needed a new instrument rather than more care: **the
oracle cannot see it.** Run the same landing tape on the author's map, on the
map with only the anchor moved, and on the cleared map, and the last two return
the identical `DNF cps=0` (§1, worked example). One number per run cannot
distinguish 77.8 m of driving from 249 m.

### Baked blocks move if — and only if — they are FREE

The mover used to refuse every baked index: *"baked terrain is not
relocatable"*. Half right. A baked block's cell bytes **are** dead, and baked
index N is **not** unbaked index N — a bare `2461` pasted from a census row
addresses an unrelated block, and that mover would *succeed*: the map loads,
the origin control passes (the wrong block is restored just as faithfully), and
the ladder quietly measures the wrong thing.

But a baked block that is **free** has six f32 exactly like an unbaked one, and
twelve of 173691's sixteen gate pieces are precisely that. The blanket refusal
is what let a pass move one piece of sixteen and report success.

→ `bN@x,y,z` moves a baked free block. `bN` or `bN:cx,cy,cz` is still refused,
by name, with the spelling that works in the message.

### NEVER PROBE BY SWAPPING A GATE MODEL

The old `gate`, `gateat` and `probe` commands relocated a waypoint by first
swapping its item model to `GateFinish32m`. On 285885 that quadruples the
trigger volume — the origin control then returns 50.589 instead of 61.229, so
the instrument *fabricates discoveries*. On 279197 it deletes a custom Goal
item and everything DNFs.

→ **Deleted.** Every mover here is position-only. `segments` still promotes a
gate, because that is the one place where it is right: a promoted gate is a
fine **ruler** and an unsafe **objective**. And even as a ruler it is checked —
a relocated gate that does not fire falls back to the block rename, and every
segment is re-validated against the reference ghost's own declared split.

### A MAP-SURGERY CONTROL CAN BE INERT

On 173691 a gate-removed map, a deck-removed map and a road-removed map all
re-simulated to the identical 3102 rows. The tempting reading is "the map does
not matter here". The truth was that the surgery never reached the simulation:
the recording carried its own map and the file on disk was decoration. **The
road control is what proved the instrument was dead rather than the maps
identical.**

→ The suite runs the pair, on the real oracle, in both directions: moving the
checkpoint block the reference ghost drives through must change the run (it
DNFs), and moving an off-route block by the same mover must leave the time
**exactly** unchanged (19.538). Either row alone proves nothing — a dead
instrument passes the second, a broken writer passes the first.

### THE SERVER IGNORES A FILE IT CANNOT NAME

A candidate not named `*.Ghost.Gbx` or `*.Replay.Gbx` is skipped, produces no
result row, and reads back as an ordinary DNF. It cost the regen work
**32 consecutive good regenerations** refused. For map A/B it is worse: "the
gate removal broke the run" and "the server never opened the file" are the same
output.

→ `oracle::readable_name` refuses the path before anything is staged.

### THE SERVER PRINTS TWO RESULTS AND THE SECOND IS THE FILE'S OWN CLAIM

A parser that keeps reading `"Time"` to the end of a block returns
`DeclaredResult` — the file's declaration — and so confirms whatever the file
says about itself. On a **DNF** the trap is sharper still: `ValidatedResult` is
`null` and carries no time at all, so the *first* `"Time"` is already the
declaration, and a run that reached 2 of 4 checkpoints reads back as "finished
at 22.730".

→ `parse_output` tracks which block it is in. It is tested on **asymmetric**
fixtures, and that is the whole point: on a passing file the two results are
*equal*, so a fixture built from a passing run cannot fail — a correct parser
and a broken one are indistinguishable. The two fixtures are a finish whose
declared time is a different number, and a DNF whose declaration still carries
one.

### A CARRIED MAP IS NOT SOMETHING TO WALK

A replay carries a whole map, and that map's own chunk can declare a size
running past the map's end. A chunk walk that "corrects" such a size word
writes four bytes into the middle of a map: every string reads back perfectly
and the file then validates to **nothing**.

→ `MapFile::load` refuses any GBX class that is not `0x03043000`, and prints
the `ghost map extract` / `ghost map set` composition. The suite proves the
refusal and — the half that matters — proves a real map still loads through the
same entry point, so the refusal is about the class and not about a broken
loader.

### A CELL MOVE ON A FREE BLOCK IS SILENT

A free block ignores its cell bytes, so a regime-blind cell write produces a
map that loads, an origin control that passes, and a ladder in which **every
rung is silent** — which reads exactly like "the car does not go there".
Measured on 210218: `--cell 20,12,31` puts the gate at y = 34 while the car is
at y ≈ 108; y = 34/58/74 all silent, 82/90/98 all fire.

→ `move` refuses a cell spec on a free block and names the form that works.

### A PER-BLOCK ROTATION SHEARS A SURFACE INSTEAD OF TILTING IT

A block's stored rotation turns it about **its own anchor**, so a road made of
32 m tiles is not tilted by giving every tile the same roll: at 3.4 ° each
tile's far edge lifts 1.9 m above its neighbour's near edge and the road becomes
a staircase. Measured on 279008: the same tilt applied per-block deflected the
human's car from a crossing angle of −25.29 to −8.28 and cost it 3.4 m/s before
it reached the obstacle at all; applied about a common axis 100 m long, it lifted
the far tile into the neighbouring untilted road and **stopped the car dead 100 m
short**. Both read, from a distance, exactly like "the tilt did not matter".

→ `rotate --tilt` writes position and rotation together about one named axis, and
prints that the orientation is a first-order decomposition so the surface is
measured rather than assumed. And the general lesson underneath it, which is the
same one as the gate: **the object you are rotating is not the object you think
you are rotating** — on 284238 the ice kicker is FOUR blocks sharing an anchor to
the millimetre, of which one is free-placed, and two arms ran "raise the kicker
by 1.00 m" as their decisive experiment, raised a quarter of it, built a 1 m step
(entry speed 99.81 → 50.84 m/s) and got a null. `rotate` REFUSES when a free
block within `--group-radius` of the rotation is not in it, and names it.

### READ EVERY RESULT DIRECTORY BY MTIME, NEVER BY FILENAME

→ Nothing in this tool reads a result directory. Every number it prints comes
out of the file in front of it or out of the plain oracle.

---

## 4. The suite

```
tmmaps selftest              # pure + oracle, ~30 s
tmmaps selftest --strict     # a SKIP is a failure
cargo test --release         # the same suite, through cargo
```

**29 checks over seven checked-in fixtures**: two campaign maps with their
reference ghosts, 173691's author map with the added finish gate that taught us
the structure lesson, and a **captured dedicated-server transcript**. Two tiers:

* **PURE** — container round trip on all three maps; the origin control
  (81 094 movers on the big map, 0 failures); the census seeing both block
  chunks; the gate-structure quartet; the mover refusals; the container-class
  refusal with its positive twin; the transcript-driven oracle-parser checks
  **and the mutation control that proves the transcript can catch a wrong
  parser**; the filename guard with its positive twin; time formatting.
* **ORACLE** — the real dedicated server: the identity candidate first (an
  untouched map must give each ghost its own declared time, or nothing below is
  interpretable); every map-1 segment reproducing its declared split to the
  millisecond; the map-2 block-rename fallback firing early by 0.167 and not by
  "some amount"; and the alive-instrument pair.

`--strict` exists because a suite whose fixtures are missing prints green lines
and proves nothing: the previous version returned early from every oracle test
when `/tmp/m1` was absent and reported `7 passed`. `cargo test` proves the flag
is not vacuous by running the tool against a server path that does not exist
and requiring a non-zero exit.

The parser fixture deserves its own note, because it is the shape most easily
got wrong. `testdata/oracle_transcript.json` is the **server's own bytes** from
a real run — not hand-written, so it carries the real spacing, the trailing
`\n` in `Desc`, and `Desc` in a different position on each row. Both of its
rows are deliberately **asymmetric**, because on an ordinary passing file
`ValidatedResult` and `DeclaredResult` are equal and a fixture built from one
cannot fail. Row 1 is a tape edited until it stopped finishing, still declaring
19.538 and four checkpoints; row 2 finishes at 19.538 while declaring 30.000.
And the suite then runs the **wrong** parser — last `"Time"` before each
`"FileName"` — on the same bytes and requires it to answer `19.538, 30.000`. If
that ever agrees with the right answer, the fixture has stopped being a test.

Every ghost fixture has been through `ghost identity set --anonymise`, and the
oracle says the pass changed no physics: 19.538, 19.812 and 22.730 before and
after.

---

## 5. What happened to `u02`

`u02` is deleted. See [`U02-AUDIT.md`](U02-AUDIT.md) for the command-by-command
audit and the controls behind each verdict. In one line: it was a second
implementation of the ghost container, and its one genuinely separate
capability — free-block surgery — is `tmmaps move` composed with
`ghost map extract` / `ghost map set`.

---

## 6. What is NOT safe, and what I could not check

* **"It validated" is still not "it renders".** A re-emitted map loads in the
  dedicated server and has never loaded in the game client — both the map
  extracted from a replay and an edit built from the downloaded file sat
  forever on "loading map". There is no game client on a Linux box, so nothing
  here can prove an import. The honest controls are the byte-level round trip
  and the origin control, and they are both in the suite. **If a scene has to
  be filmed, film it on a map file the game already has and swap it in with
  `ghost map set`.**
* **The origin control cannot see a trigger-volume change.** It is a byte
  identity on a position-only mover. That is why the model-swapping commands
  were deleted rather than controlled: there is no control here that would have
  caught them.
* **`region` is only as good as its box, and a grid block has no position.**
  A grid block is reported at its cell's world point, accurate to 32 m, and the
  output says which rows those are. `clear` refuses to move them rather than
  half-doing it. The suite's guard against a too-small box is to ask the whole
  map the same question and require the same count; on a map where the filter
  is less selective than `GateExpandable`, that guard is weaker.
* **`renamecheck`'s lookback warning is conservative** — it fires on maps whose
  game check then passes. Do not read it as a failure, and do not read it as an
  all-clear.
* **The oracle tier runs on two maps.** Map 1 is Tech road with a relocatable
  gate item; map 2 is Dirt with none, which is why it exercises the
  block-rename fallback. A third regime — a map whose Goal is a *free* block,
  like 210218's `GateExpandableFinish` — is exercised only in the PURE tier
  (through `goth.Map.Gbx`) because no reference ghost for it is checked in.
* **`cporder` and `rungspec` read a `tmtraj` CSV** and are not covered by the
  suite: they need a decoded trajectory, which is a `tmtraj` artefact and not a
  map fixture. They are kept because the ladder workflow needs them, but they
  are the two commands here with no control of their own.
