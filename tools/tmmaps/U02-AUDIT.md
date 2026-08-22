# U02-AUDIT.md — what happened to `u02`, and to the three `tmmaps`

**Verdict: `u02` is deleted, all 24 subcommands. `tmmaps` survives as
`tools/tmmaps` at 12 subcommands from 25, with `u02`'s one genuinely separate
capability folded in as a composition with `tools/ghost`.**

Two implementations of one file format is how this project got silent
corruption before, so this was never a stylistic question. The controls below
are the point; the table is the summary.

---

## 0. Two findings before the table

### The `u02` in the workspace was not the newest, and there are THREE `tmmaps`

`u02`'s newest copy is `173691/drive_20260822/` (1 110 + 353 + 39 = 1 502
lines, 24 subcommands); the copy in the banked workspace tarball is a day older
and is missing `mapof`, `setmap` and `movefree` — the three that matter most.

`tmmaps` is worse. **Three divergent forks are banked, and the one wired into
the `rs/` workspace is the smallest and oldest of them:**

| fork | lines | subcommands | has |
|---|---|---|---|
| `r165_tools_v5/rs/tmmaps` (the workspace member) | 2 306 | 9 | — |
| `prs_tmmaps_v3` | 4 042 | 24 | `moveitem`, `ladder`, `origin`, baked census |
| **`ord_tmmaps_order_v1`** (newest, 2026-08-19) | 4 232 | 25 | all of the above + a verified `--order` |

The underwater arm's own scripts call `tmmaps moveblock`, `tmmaps allblocks2`
and `tmmaps origin` — commands the workspace member **does not have**. Across
the store's write-ups, `tmmaps gate` is cited 88 times and `moveitem` 38: the
forbidden probe and its replacement, in one corpus, because the fix lived in a
fork nobody adopted.

So the base for this work is `ord`, not the workspace member. Adopting it is
controlled: it passes the workspace member's own acceptance suite with
**byte-identical numbers** (map 1 8/8 exact, map 2 seg1 early by 0.167,
seg2/seg3 exact). Keeping the workspace member instead would have shipped a
tool whose only probe path is the one the fleet notes forbid.

### `u02`'s `rebind` produces a file that claims one map and runs another

The brief asked for this to be enforced by the tool rather than remembered.
Measured, on the checked-in Kacky replay:

```
$ u02 rebind r.Replay.Gbx out.Replay.Gbx --map map1.Map.Gbx
rebind BMWE8nGL9v6ho1B9nmYt6ijf7p8 -> buNzfsVlp2NF2oWtHM3729dEylg: 4 occurrences
$ ghost map show out.Replay.Gbx
carried map uid: Some("buNzfsVlp2NF2oWtHM3729dEylg")     # map1's uid
$ tmmaps oracle --map map1.Map.Gbx --ghosts out.Replay.Gbx
map1.Map.Gbx  out.Replay.Gbx  7.241                       # Kacky's time
```

Exit 0, no warning. It rewrote the uid **inside the carried map as well**, so
the file now declares map 1 everywhere, would pass any "does the uid match"
check, and still simulates Kacky. `ghost map rebind` on the same file:

```
REFUSED: this file carries an embedded map, so the server will simulate THAT
copy whatever uid the file declares. Rewriting the uid here would produce a
file that claims one map and runs another -- the exact failure this API exists
to prevent. Use `ghost map set` to replace the carried map.
```

That is the whole argument for the merge in one command.

---

## 1. `u02`, command by command

24 subcommands. **0 survive as `u02`.**

### Deleted — `tools/ghost` owns this (12)

| `u02` | replacement | control |
|---|---|---|
| `info` | `ghost inspect` | `inspect` prints everything `info` did plus the codec identity, the declared-time census, the identity strings with offsets and the telemetry span |
| `splits` | `ghost inspect` | same chunk `0x0309202B`, printed as seconds |
| `mapuid`, `finduid` | `ghost map show` / `ghost inspect` | `map show` prints the carried uid **and** every uid literal, and says which one the server will use |
| `rebind` | `ghost map rebind` | the measurement above: `u02`'s writes a file that claims one map and runs another; `ghost`'s refuses that case by name |
| `mapof` | `ghost map extract` | same chunk `0x03093002` |
| `setmap` | `ghost map set` | `ghost`'s is proven by a swap that changes the oracle's answer **with zero maps on disk**, and a put-back that is byte-identical |
| `totxt`, `fromtxt`, `tape` | `ghost tape extract` / `inject` | **decisive**: round-tripping the map-1 WR through `u02 totxt`→`fromtxt` changes **4 151 of 2 119 tape lines**, expands every `vsame` packet (bitstream 861 → 3 453 bytes) and **drops the `@tail 0f` byte**. `ghost tape extract`→`inject --verbatim` changes **0**. Both re-simulate to 19.538 — which is exactly why the loss survived: the oracle cannot see it |
| `truncate` | `ghost trim --to` | **measured**: `u02 truncate --ticks 1000` on the map-1 WR cut the tape to 1 000 ticks and left the file declaring **19.538**, all four splits, and **391 telemetry samples spanning 0.000 .. 19.530** — a file whose tape and whose record describe different runs. `ghost trim --to 10000` on the same file: tape 2 109 → 1 156, declared 10.000 in every copy, checkpoints `["7.617"]` from `["7.617", "13.308", "16.316", "19.538"]`, telemetry 401 kept / 380 dropped, and it re-reads what it wrote. (`u02 truncate` also silently expanded all 2 074 same-as-previous packets on the way through) |
| `declare` | `ghost declare` | `u02 declare --time 70000` set the `0x0309202B` race time and left the header declaring 19.538 — the "declared time lives at N offsets" trap, in the tool. `ghost declare` writes every copy and reads back. (One capability does not survive: see §3) |
| `events` | `ghost tape stats` | tick count, input events, packet modes, respawns |
| `walltime` | `ghost` writes it | `ghost trim` and `ghost declare` keep `0x0309202C`/`0x0309202D` coherent as a consequence of the edit, rather than as a separate command a human must remember |

### Deleted — one-off probe apparatus whose question is answered (5)

`patchbytes`, `setu32` — a generic "copy this byte range of this chunk from
that donor", built to answer *which field binds a container to its map*. The
answer is known and is now structural: the uid is inline chunk `0x03092010`,
the map is `0x03093002`, and a file carrying the second ignores the first.
Keeping a byte-poker so that the question can be re-asked is how the second
implementation starts.

`sweep` — a parameter grid over four switch ticks of one speed-keyed policy for
map 276877. Single-map, single-policy; `tmsearch` is the search.

`jitter` — ±1-tick variants of every input change. A real measurement, but a
search operator, not a container tool, and nothing on a live path calls it.

`climb` (353 lines) — the 276877 climb search. Same.

### Deleted — belongs to a tool that already owns the data (3)

`mapview` — an ASCII occupancy view of a block-census TSV. Superseded by
`tmmaps census` + `tmmaps region`, which read the map itself rather than a TSV
somebody generated, see the **baked** chunk that `mapview`'s input never
contained, and answer "what is in this box" instead of "draw me a picture and
count by eye". Its priority table (`R`/`N`/`B`/`S`/`G` for reset/no-steer/
boost/slow/gate) is carried into `census --filter`.

`trajscore`, `trajstats` — trajectory reduction, with map-276877 target
coordinates as defaults. `tmtraj` owns decoded trajectories, and `fk land`'s
banded scorer superseded `trajscore` outright. The one fact worth keeping is in
`tmtraj`'s docs already: a one-tick displacement of metres is a teleport, and
that is the cheapest test for "is this a car driving or a memory slot".

### Moved into `tmmaps` (1)

`movefree` — move a free block by rewriting its three f32 in place, reaching
inside a replay's carried map. The capability is real and separate; the reach
is not. It becomes:

```bash
ghost map extract R.Replay.Gbx --out m.Map.Gbx
tmmaps move m.Map.Gbx --out m2.Map.Gbx --move 68150@-3000,-3000,-3000
ghost map set R.Replay.Gbx R2.Replay.Gbx --map m2.Map.Gbx
```

Three reasons this is better than the command it replaces, all of them things
`movefree` got wrong on the map it was written for:

1. **`movefree` addresses a block by its coordinate triple** and refuses on more
   than one match. Twelve of 173691's sixteen gate pieces are baked, and several
   share coordinates with unrelated blocks; `--at` cannot express "the baked
   one". `tmmaps move` takes the index the census prints, and `bN` for baked.
2. **`movefree` moves one thing.** A gate is a structure. `tmmaps clear` moves a
   region and proves the region is empty afterwards.
3. **`movefree` reaches into the replay itself**, which means a chunk walk over
   a container that carries a map — the walk that, in the ghost arm's hands,
   "corrected" a size word *inside* a carried map and produced a file that reads
   back perfectly and validates to nothing. `tmmaps` now refuses any GBX class
   that is not `0x03043000`.

### Deleted — dead on arrival (1)

`hud` (a second binary, 39 lines) — no callsite anywhere in the workspace or
the store.

---

## 2. `tmmaps`: 25 subcommands to 12

Started from `ord_tmmaps_order_v1`, 4 232 lines.

| kept | why |
|---|---|
| `waypoints` (was `list`) | the indices every mover takes; 102 citations |
| `census` (was `allblocks2`) | the only listing that sees **both** block chunks; 52.7 % of blocks are baked and eleven maps read as near-empty without it |
| **`region`** *(new)* | a gate is a structure |
| **`clear`** *(new)* | …and this is the enforced form of it |
| `segments` (was `build`) | the segment maps; 66 citations |
| `move` (was `movemany`) | one mover, all four regimes |
| `ladder` (was `mladder`) | curtain rungs; subsumes `ladder`/`bladder` exactly |
| `oracle` | the server driver |
| `rungspec`, `cporder` | the ladder workflow's two analysis steps |
| `roundtrip`, `origin`, `renamecheck` | the three controls |
| `selftest` *(new)* | all of it, one command |

| deleted | why |
|---|---|
| `gate`, `gateat`, `probe` | **they swap the gate model before relocating it.** On 285885 that quadruples the trigger volume and the origin control returns 50.589 instead of 61.229 — the instrument fabricates discoveries. On 279197 it deletes a custom Goal item and everything DNFs. There is no control here that catches it, which is why these are deleted rather than fixed |
| `ladder`, `bladder` | `mladder`'s spec covers both: a curtain of one gate *is* a single-gate rung, and a single-cell rung is silent for about a third of well-chosen placements anyway. ~380 lines of near-duplicate |
| `moveitem`, `moveblock` | special cases of `movemany`, with their own copies of the free-vs-grid refusal |
| `blocks`, `freeblocks2` | strict subsets of the census (one chunk / one filter) |
| `splits` | `ghost inspect` owns the ghost format. The library function stays; the command goes |
| `renametest`, `bodydiff`, `dump` | debug scaffolding. `renamecheck` is the real rename control; `chunks` is the real structural view |

Also deleted: `OrderRound`/`OrderReport::rounds` and `OrderProbe::matched`
(built, never read), `move_waypoint_block` (its only caller was `moveblock`),
and `probe.rs` entire. **The crate builds with zero warnings.**

Net: 4 232 → 5 183 lines, but 657 of those are the new suite and 368 the new
region/clear module; the pre-existing surface shrank by roughly a third while
gaining the two commands the traps needed.

---

## 3. Gaps — things that should exist and do not

> **Asks 1 and 2 were closed on 2026-08-22** (branch `gapfix-20260822`).
> `ghost trim --to` past the end of the tape now LENGTHENS it, and
> `ghost declare --cps N` sets the number of split entries.
>
> **Ask 2's diagnosis did not survive its own control, and that is worth more
> than the command.** The claim here — that the server refuses a count mismatch
> as `wrong simu` *without simulating it at all* — is false. Measured on two
> maps and six counts (1, 2, 3, 5 declared on a 4-split map, 9 on a 3-split
> map, intermediate splits zeroed): the server validated every one of them at
> the right time and echoed the wrong count back in `DeclaredResult`.
> `wrong simu` is what it says when the simulation does not reproduce the
> DECLARED RESULT; on a partial run it even reports the depth (`wrong simu, but
> reached some checkpoints (1 out of 2)`). Correcting the count does not make a
> borrowed container simulate. What the count really breaks is **this
> toolchain**: `tmmaps segments` refuses a reference ghost whose split count is
> not the map's — verified both ways, and now the reason `--cps` exists.
> Asks 3, 4 and 5 are still open.

These are the concrete asks. Two are for `tools/ghost`, whose arm has settled;
they are small and they are load-bearing.

1. **`ghost tape` cannot lengthen a tape.** `u02 extend` (append copies of the
   last packet) is `u02`'s second-most-cited command — 15 callsites — and
   `ghost tape inject` refuses a longer tape outright:
   *"tape has 2159 ticks, a.Ghost.Gbx has 2109 … use `ghost trim` to change the
   length"*, and `trim` only cuts. This is not academic: the 173691 landing work
   needed a 7 000-tick tape to give the car room to brake. **Ask: `ghost trim`
   grows as well as cuts, or `tape inject --extend`.** Until then this
   capability is simply gone, and that is the one regression in this audit.
2. **`ghost declare` cannot change the number of split entries.** `--time` sets
   the declared time in every copy correctly (which `u02 declare` did not), but
   leaves the split list at its old length. A container borrowed from another
   map declares *that* map's checkpoints, and the server then refuses the file
   with `wrong simu` **without simulating it at all** — 0 s elapsed,
   `ValidatedResult` null — which through any tool that reads only the summary
   line is indistinguishable from a tape that drove badly. **Ask:
   `ghost declare --cps N`**, ideally derived from the map rather than typed.
3. **A `--map`-inert container has no cheap tell.** On 173691, gate-removed,
   deck-removed and road-removed maps all re-simulated identically because the
   recording carried its own map. `ghost inspect` says `EMBEDDED MAP: …` in its
   first lines, and `tmmaps` now refuses recordings, so the two halves exist —
   but nothing *joins* them. **Worth building: `tmmaps oracle` warning when a
   candidate carries its own map and `--map` was passed**, because that is the
   exact command where the mistake is made.
4. **`region` cannot ask "what is near this trajectory".** The box is typed by
   hand, and the suite's own guard against a too-small box (ask the whole map,
   require the same count) only works when the filter is selective. Landing
   zones come from trajectories; `rungspec` already reads one. **Worth
   building: `tmmaps region --along TRAJ.csv --radius R`.**
5. **No fixture exercises a map whose Goal is a free block against the
   oracle.** 210218's `GateExpandableFinish` is the regime where a cell move is
   silent and the origin control still passes; the PURE tier covers the parsing
   through `goth.Map.Gbx`, but no reference ghost for such a map is checked in,
   so the ORACLE tier has never seen one.

---


> `tmmaps/src/oracle.rs` keeps the (map, ghosts) driver — one worker directory
> per pair, because every segment map keeps the original mapUid — but **not a
> parser**: it projects `ghost::oracle`'s result onto the four fields map
> surgery uses. That was the sixth copy of the server parser in the tree. The
> merge changed no answer (the ORACLE tier reproduces 7.617 / 13.308 / 16.316 /
> 19.538 exactly, as before) and moved one behaviour up into the shared reader:
> the huge-u32 "never crossed" sentinel, which `ghost::oracle` now refuses to
> report as a finish.

## 4. One more thing for `tools/ghost`: `Container::splits()` is raw

> **CLOSED 2026-08-22.** `Container::splits()` returns the checkpoint list, the
> words are `splits_raw()`, and `ghost inspect` prints
> `7.617 13.308 16.316 19.538`. The decode AND the write live once, in
> `gbx::container::GhostResult`; this file's `src/ghost.rs` keeps no decoder
> (only the "a missing chunk is `None`, not an empty list" rule that is
> genuinely tmmaps'), and its two unit tests moved into `gbx` with the layout.
> Two more readers went with them: `gbx::record`'s needle-based
> `read_ghost_result` and `ghost::trim`'s inline writer. The third consumer of
> the raw array was `tmsearch`'s `--seg` check, which compared a segment map's
> answer against `splits[k-1]` — the chunk's version word for checkpoint 1.

Not a bug, but an API shape that invites one, and it caught me. `splits()`
returns the chunk's **raw u32 array**, not the checkpoint list. On the map-1 WR
that is

```
[1, 19538, 0, 0, 3, 4, 7617, 2, 13308, 4, 16316, 0, 19538, 1, 4294967295]
```

— fifteen words, of which four are splits; the rest are a version, a race time,
a count and per-entry tags. `ghost inspect` prints that array through the
seconds formatter, so its `splits` line reads

```
splits  0.001 19.538 0.000 0.000 0.003 0.004 7.617 0.002 13.308 …
```

which renders a version number as `0.001` and a per-entry tag as `0.002`. The
real answer is `7.617 13.308 16.316 19.538`.

I found this by *replacing* `tmmaps`'s own copy of the chunk walk with the call
— and the segment builder refused on the spot: *"the map declares 3 checkpoints
so the ghost should declare 4 splits; it declares 15"*. That refusal is the
control for the change and it fired first time, which is the argument for
having made the change at all: two implementations agree until they do not, and
deleting one is how you find out which day it is.

`tmmaps` now decodes the array locally (`src/ghost.rs`, with the layout written
down and two unit tests, one of them a refusal on a short array) and its
segment output is byte-identical to the pre-switch run. **Ask:
`Container::splits()` returns the checkpoint list, the raw array gets a name
that says so (`splits_raw`), and `ghost inspect` prints the decoded one.**
Until then every caller of `splits()` is one plausible assumption away from
verifying its segments against the wrong checkpoint.

## 5. What I deliberately did not touch

* **`tools/ghost`** — owned by another arm, settled at `2edf257`. Everything
  above is a call into it or a deletion.
* **`tools/fk`, `tools/forkoracle`, `tools/shootctl`, `tools/openplanet-plugin`**
  — other arms', live.
* **`tmsearch` / `tmtas`, `tmtraj`, `tmsite`, `fkcount`** — not in scope, but I
  read the two parsers while building the fixture and one of them is wrong.

  `tmsearch`'s `oracle.rs` is a **fourth** copy of the dedicated-server driver.
  Its result parser is **correct** on both paths (it gates the `"Time"` read on
  an `in_validated` flag and clears it after the first hit). It has **no
  filename guard**: `stage` links whatever it is handed, so a candidate the
  server skips comes back as an ordinary DNF.

  `tmtraj`'s integrity gate has a **fifth**, and it has the bug. `parse_oracle`
  handles the time correctly — an explicit `"ValidatedResult" : null` test
  first — and then does this:

  ```rust
  let cps = grab_i("NbCheckpoints", "\"ValidatedResult\"");
  ```

  `grab_i` finds `"ValidatedResult"` and scans **forward** for the key. On a
  DNF there is no `NbCheckpoints` inside `ValidatedResult` — it is `null` — so
  the scan runs on into `DeclaredResult` and returns the file's own claim. The
  captured transcript now checked in as `testdata/oracle_transcript.json` has
  exactly that row: a run the server refused (`"ValidatedResult" : null`,
  `"Desc" : "wrong simu"`) whose declaration says `"NbCheckpoints" : 4`, so
  **the gate reports four validated checkpoints for a run that reached none**.
  Verified rather than read: `parse_oracle`'s two closures were compiled
  verbatim and run on that transcript. Row 1 comes back
  `sim_time=None, cps=Some(4), declared=Some(19538)` — **the time is right and
  the checkpoint count is the file's own claim.** Same family as the DNF-time
  bug the fk arm found, one field over. Not mine to fix; the fixture that
  catches it is checked in and is two lines to reuse.
* **The `rs/` workspace itself.** `tmmaps` now lives in the repo at
  `tools/tmmaps` and builds standalone (`cargo build --release`; one path
  dependency, `tools/ghost`); the workspace copy is superseded and should be
  deleted when whoever owns `rs/` next touches it. I did not delete it because
  four other members still build against that `Cargo.toml`.
* **The shell scripts under `tools/`** (`ghost-splice-audit.sh`,
  `ship-clip.sh`, `skincheck.sh`, …). Nine of them are in the repo root's
  `tools/`, they violate the Rust-only rule, and none of them are mine. Flagged,
  not touched.
