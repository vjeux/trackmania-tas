# GHOSTS.md — the TM2020 ghost / replay API

Everything this project does to a `.Ghost.Gbx` or a `.Replay.Gbx`, in one Rust
crate, with a control behind every operation.

```
cd tools/ghost && cargo build --release
TM_SERVER=/path/to/TrackmaniaServer-dir ./target/release/ghost selftest
```

`ghost --help` lists every command. Times print as **seconds with a decimal**
(`22.730`), never as raw milliseconds.

## 0. It is a library first

`tools/ghost` owns the ghost and replay format for the whole toolchain.
Everything else calls in here rather than keeping its own reader, because every
bug this crate exists to prevent was a second copy of one of these readers
disagreeing with the first.

| module | what it gives you |
|---|---|
| `tape` | the input codec: `Tape::from_file` / `from_text` / `to_text` / `splice_into` / `verbatim_is_identity`, and decoded per-tick slices (`steer_i8s`, `accels`, `brakes`, `respawns`, `race_ms`) |
| `container` | chunks, the embedded map, every copy of the declared time, the ghost-result chunk, uid literals, framed string edits, `set_embedded_map` |
| `ident` | every identity string with its role and offset |
| `oracle` | `validate_many` (the server validates in BATCHES and the per-launch cost dominates), `validate`, `MapsMode::{One, Empty}` |
| `verify` | the acceptance gate, `tape_record_agreement`, and `SimResult::declaration_holds()` on its own |
| `regen` | engine-regenerated car state behind a gate, `write_input_channels`, `engine_trajectory_agreement` |

The library surface returns `Result` and never exits. The `cmd` functions are
the CLI's entry points and are the only things that call `cli::die`.

`SimResult` keeps **both** times the server reports — what it simulated and what
the file claims — plus the server's own `Desc`, its `Inputs` echo of the tape it
decoded, `IsValid`, and the account id and login it read out of the file.

---

## 1. The operations

| | command | what it does |
|---|---|---|
| inspect | `ghost inspect FILE` | container kind, the map it will actually run on, declared time and every copy of it, checkpoints, identity, the input tape, the telemetry record |
| **extract inputs** | `ghost tape extract FILE --out T.gtape` | every tick, every field the packet carries — full fidelity, round-trips byte for byte |
| **inject inputs** | `ghost tape inject IN OUT --tape T.gtape` | writes a tape back; every vehicle field explicit so no tick inherits another's |
| | `ghost tape expand IN OUT` | rewrite every "same as previous tick" packet explicitly; semantically a no-op and the oracle says so |
| | `ghost tape diff A.gtape B.gtape` · `stats` · `bits` | compare two tapes; summarise one; census which bits of the state literal ever vary |
| | `ghost tape sync-record IN OUT` | rewrite the telemetry's recorded steer / gas / brake from the tape — they are fully determined by it and need no engine |
| **car state** | `ghost regen IN OUT --map M` | run the real engine on this file's own inputs, capture per-sample car state, write it in — behind a gate that refuses a bad locate |
| | `ghost regen-control FILE --map M` | the fixed point: regenerate a file that already knows its own answer and require it back |
| | `ghost trajdiff A B` | two files' recorded trajectories, at every shift from −3 to +3 samples |
| | `ghost engine classinfo/idsites/vtable` | what the server binary says about its own classes — the evidence behind the locate |
| **change the map** | `ghost map show FILE` | which map this file will *actually* run on, and whether `--map` is real for it |
| | `ghost map extract FILE --out M.Map.Gbx` | pull the carried map out |
| | `ghost map set IN OUT --map M` | replace the **carried** map — the only thing that moves a recording that has one |
| | `ghost map rebind IN OUT --map M` | rebind a **pure ghost** by uid — refused on a file that carries a map |
| **length** | `ghost trim IN OUT [--from MS] [--to MS]` | set the run's window: cut head and/or tail, or **lengthen** it (`--to` past the end appends ticks holding the last input), keeping tape, telemetry, record span, checkpoints and every copy of the declared time coherent |
| **identity** | `ghost identity show FILE` | skin, display name, trigram, zone, club tag, login, account id, locator URL — with offsets |
| | `ghost identity set IN OUT --name N --trigram XXX --skin S [--anonymise]` | change them, with an oracle no-op control |
| **the header** | `ghost header show FILE` | the replay HEADER's chunk table, its driver fields, the map's own attribution, and every copy of the race time it holds |
| added | `ghost declare IN OUT --from-oracle --map M` | write the time the file **actually does**, asked of the plain oracle, into every copy |
| added | `ghost declare IN OUT --time MS --cps N` | and the NUMBER of checkpoint entries, for a container borrowed from another map |
| added | `ghost verify FILE [--map M] [--engine] [--empty-maps]` | the acceptance gate |
| added | `ghost selftest [--engine] [--strict]` | the whole suite, one command |

### Things added beyond the six, and why

* **`ghost trim` lengthens.** `u02 extend` — append copies of the last packet —
  had 15 callsites, and when `u02` was deleted the capability went with it: the
  one regression of the audit. The 173691 landing work needed a **7000-tick**
  tape to give the car room to brake after touchdown. `--to` past the end of the
  tape appends ticks that hold the last recorded input, with the respawn bit
  cleared — a respawn is an input, and repeating a respawn tick five thousand
  times would hold the key down for the whole extension. It lives in `trim`
  rather than in `ghost tape` because one command should own a run's LENGTH:
  the coherence obligations are one set, and `tape inject` deliberately refuses
  a length change so that a tape and its container can never disagree.
  An extension does not touch the telemetry, the declared time or the splits —
  no samples exist past the end of the recording, and the appended ticks are
  after the finish. **The control for that claim is the world**: the lengthened
  file must re-simulate to the time the original did (`oracle.extend_keeps_finish`,
  2432 → 7000 ticks, 22.730 → 22.730), and `ghost verify` must still pass on it.
* **`ghost declare --cps N`** — `--time` writes the declared time into every
  copy, but a container **borrowed from another map** also declares the *donor
  map's* checkpoint list, and the count cannot be changed without changing the
  chunk's length. The intermediate entries are written as `0.000`, deliberately:
  see "the ghost-result chunk" below.
* **`ghost declare --from-oracle`** — after the inputs change, the declared time
  is the *old* run's. Every "the file says X, the file does Y" check then
  compares a stale number. This asks the plain oracle what the file does and
  writes that, so the number is never typed by a human and never copied out of a
  search log.
* **`ghost map rebind`** — a pure ghost is bound to a map by the uid it
  declares, not by an embedded copy. Two different operations, one for each
  container kind, and each refuses the other's case.
* **`ghost verify`** — one command that runs every control at once, so
  "publishable" is a thing you can ask rather than a thing you remember.
* **`ghost tape bits`** — a census of which bits of the state literal ever vary
  across a corpus, so "unnamed" is an enumerated set instead of a shrug.
* **`ghost inspect`** and **`ghost chunks`** — because most of the bugs below
  were invisible, and the first fix for an invisible bug is to print it.

---

## 2. The file format, as measured

### The input chunk `0x0309201D`

One or more *archives*; each is a bit-packed stream, one packet per **10 ms**
tick. A packet is:

1. **the state word** — three codings:
   `1` repeat the previous word · `0 1 x y` repeat it overriding flag bits 0
   and 1 · `0 0 <literal>` an explicit **33-bit** (format 11) or **34-bit**
   (format 12) literal.
   The literal unpacks into `word0` and 22 `flags` bits; `word0 & 0xF` is the
   packet **mode**; **bit 31 of the literal is the RESPAWN input** (it lands in
   `word0` bit 5, and a repeated word always reads 0 there).
2. **the mouse segment** — `1` for none, or `0` plus two 16-bit axes.
3. **the vehicle fields**, per mode:
   `2, 4` same-bit else steer:8, accel:1, brake:1 ·
   `12` same-bit else steer:32, accel:1, brake:1 ·
   `13` same-bit else steer:32 ·
   `0` nothing · anything else: same-bit else four 2-bit trigger fields.

Steer is an **i8 over 127**. A real bitstream is **11–33 bytes longer** than its
packets need; those trailing bytes are part of the file and a verbatim
re-encode carries them through.

### Where everything else lives

| what | where |
|---|---|
| declared race time | skippable chunk `0x03092005` (a file can hold more than one copy) |
| checkpoints + race time | skippable chunk `0x0309202B` — the **ghost-result chunk**, and not a bare split vector: see below |
| walltime | `0x0309202D` |
| skin / locator URL / **display name** | the front of skippable chunk `0x03092000`, before the record blob |
| trigram / zone / club tag | the **tail** of the same chunk, after the record blob |
| account id | **inline** chunk `0x0309200F` — no `PIKS`, not in the chunk table |
| map uid (pure ghost) | **inline** chunk `0x03092010`, plus literal copies |
| telemetry | `CPlugEntRecordData`, zlib, 50 ms samples of 116 bytes; the vehicle entity is `CSceneVehicleVis` `0x0A018000` |
| **the carried map** | `0x03093002` + a size word + a whole nested GBX — **inline, no `PIKS`** |

### The ghost-result chunk `0x0309202B`, and why `splits()` was a trap

```text
u32 version = 1 · i32 raceTime_ms · i32 u01 · i32 u02 · i32 nbRespawns
i32 nCheckpoints · nCheckpoints × (i32 time_ms, i32 tag) · i32 -1
```

On the map-1 WR that is fifteen words —
`[1, 19538, 0, 0, 3, 4, 7617, 2, 13308, 4, 16316, 0, 19538, 1, -1]` — of which
**four are splits**. `Container::splits()` returned the whole array, and
`ghost inspect` printed it through the seconds formatter, so its `splits` line
opened `0.001 19.538 0.000 0.000 0.003 0.004 7.617 …`: a version number as
`0.001`, a per-entry tag as `0.002`. The real answer is
`7.617 13.308 16.316 19.538`.

The chunk is now decoded and written in exactly one place,
`gbx::container::GhostResult` (`decode` / `encode` / `checkpoints`), reached as
`Container::result()`, `Container::splits()` (the checkpoint list) and
`Container::splits_raw()` (the words, for forensics). Three other readers of
this chunk are gone with it: `gbx::record`'s needle-based `read_ghost_result`,
`ghost::trim`'s inline writer, and `tmmaps`'s own `decode`. The defect was found
exactly that way — `tmmaps` deleted its copy, called this one, and the segment
builder refused on the spot: *"the map declares 3 checkpoints so the ghost should
declare 4 splits; it declares 15"*.

**What the server does and does not check, measured rather than assumed.**
`ghost declare --cps N` exists because a borrowed container declares the donor
map's checkpoints; the audit that asked for it believed the server refused such
a file as `wrong simu` *without simulating it*. It does not. On two maps and six
counts — 1, 2, 3, 5 declared on a 4-split map and 9 on a 3-split map, plus
intermediate splits written as `0.000` — the server validated every one of them
at the right time, and simply echoed the wrong count back in `DeclaredResult`.
`wrong simu` is what it says when the simulation does not reproduce the DECLARED
RESULT, and on a partial run it says how far it got (`wrong simu, but reached
some checkpoints (1 out of 2)`) — a simulation, not a pre-check. `ghost selftest`
pins this as `oracle.cps_does_not_gate` so the belief cannot come back.

What the count really breaks is **this toolchain**: `tmmaps segments` refuses a
reference ghost whose split count is not the map's, because it verifies every
segment against a declared split. That refusal is why the intermediate entries
are written as `0.000` rather than carried over from the donor: with zeros, the
order measurement REFUSES (*"declared split #2 is 0.000"*); with the donor's
plausible numbers it would have produced an order — a fabricated result instead
of a stop.

### The recorded input channels

Inside each 116-byte telemetry sample:

```
byte 14 = floor((steer_i8 + 127) * 255 / 254)      the FLOOR and the 254 both matter
byte 15 = 255 when the gas is down, else 0
byte 18 = 255 when the brake is down, else 0
```

Measured against the corpus, not assumed: a `round` instead of a `floor` misses
`steer = 0` and `steer = 60`. This is what makes a ghost carry its driver's
inputs **twice**, which is the cheapest contamination check there is.

---

## 3. The traps, and what the tool now does about them

**THE MAP IS INSIDE THE REPLAY.**
A `.Replay.Gbx` carries the whole map in chunk `0x03093002` and the server
simulates *that copy*; `--map` and `UserData/Maps` are decoration.
→ `ghost inspect` and `ghost map show` say which case a file is, in the first
line of output. `ghost verify --empty-maps` proves it by validating with a Maps
directory containing **zero files** (fixture: 7.241, with no map on disk).
`ghost map rebind` **refuses** on a file that carries a map, because rewriting
the uid there makes the file claim one map and run another. `ghost map set`
replaces the carried map and the suite proves the swap changes the answer *on a
box where the original map does not exist at all*.

**A synthesised tape carries its TEMPLATE's telemetry.**
`tmtas splits` reads the header, so a grafted tape reports the donor's splits.
→ `ghost tape inject` says, every time, that the telemetry still describes the
old inputs. `ghost verify` V6 compares the file's *two* input channels — the
10 ms input chunk and byte 14 of every 50 ms sample — as **Cohen's kappa on the
exact byte**: 1.000 on a recording that belongs to its tape, 0.120 on the file
this project itself named `SEARCHTAPE_..._DO_NOT_PUBLISH`.
**Its limit, measured:** a search tape that differs from its template by a few
per cent of ticks still agrees on the other 97 %, and three such files score
0.83 — the same as a human recording. So V6 catches wholesale contamination and
**not** a small graft. What catches that is V9.

**V9: the engine re-simulates the tape and the recording must match it.**
`ghost verify --map M --engine` runs the real engine on the file's own inputs
and compares the trajectory it produces with the trajectory the file claims.
On the fixture: **0.0005 m mean, 0.0008 m worst over 455 samples.** It also
tests separately for a **whole-sample phase shift**, because a one-tick offset
is a pure time shift that hides inside a small mean and only corrupts
frame-synchronous comparisons.

**"Same as previous tick" packets are one bit with no fields.**
They must be expanded before the inputs are writable.
→ the codec always expands on write (`Encoding::Explicit`), `ghost tape expand`
does it as an operation, and the type is called `SAME_INPUTS`, never "frozen":
"frozen" reads as a claim about the game, and a stretch of them then looks like
a physics constraint instead of a gap in the encoder.
Control: `extract -> inject -> extract` is byte-identical, and the verbatim
re-encode reproduces the file's own bitstream **on 307 of 307 files in the
project's corpus** — including every replay, every search tape and every
regenerated ghost.

**A respawn is an editable input** — bit 31 of the state literal.
→ it is a named, editable field on every line of the tape format. Asking for
`respawn=1` on a repeated word is **refused with the reason**, not silently
dropped, because the bit does not exist in that coding.

**A re-emitted map loads in the dedicated server but never in the game client,
and a replay whose embedded map the client cannot parse silently fails to
import.** So "it validated" is not "it renders".
→ **not fixed, and the tool does not pretend otherwise.** `ghost map set` never
re-emits a map: it splices the *bytes you give it* in unchanged, so a map that
loaded before still loads. There is no game client on a Linux box, so nothing
here can prove an import; the honest control is the round-trip (put the carried
map back → the body is byte-identical) plus the empty-Maps validation, and both
are in the suite.

**Read every result directory by mtime, never by filename.**
→ nothing in this tool reads a result directory. Every number it prints comes
out of the file in front of it or out of the plain oracle.

**Never report a harness limit as a physics limit.**
→ `ghost regen` writes the 22 transform bytes from engine memory **and bytes
14 / 15 / 18 from the tape**, and then prints exactly which of the 116 bytes are
ours and which are still the carrier's. The remaining 91 (rpm, gear, wheel
rotation, suspension, surface effects) are in engine memory too; nothing here
has read them yet, and that is a task, not a conclusion.

**A container can be ours and the file still somebody else's.**
→ `ghost identity show` lists every identity string with its offset, including
the account id in the **inline** chunk `0x0309200F` that a skippable-chunk walk
walks straight past. `--anonymise` clears skin, locator URL, display name,
trigram, club tag and account id in one pass and **fails if any of them
survive**.

**AND THE HEADER IS A SECOND CONTAINER, WHICH NOTHING READ FOR A DAY.**
Found 2026-08-22 on 173691, after `--anonymise` and `declare --from-oracle` had
both reported success and `ghost verify` had passed the file:

```
V2  declared-time census: 1 copies, all 36.049          <- body only
V3  container identity: (nothing foreign)               <- body only
```

while its header still said `GothMommyTM`, `3Awx2_MzSdaCJZjZOht51A` and
`<times best="49958">`. **"1 copies" was a count of a set the check could not
see the rest of**, which is worse than no count.

A `.Replay.Gbx` keeps, in its header user-data: the driver's nickname and login
in chunk `0x03093000`, the race time as a **raw u32** in the same chunk, the
race time again as `best=` in the XML of `0x03093001`, and the driver's login,
nickname and zone in `0x03093002`. There is a fourth driver block in the
**body**, chunk `0x03093018`, past the nested ghost node where the identity
walk stops.

→ `gbx::header` parses the chunk table and edits string frames with their
length words; `ghost::hdr` parses `0x03093000` and `0x03093002` structurally;
`identity set --anonymise` and `declare` cover all of it; `verify` V2 counts
body **and** header copies and says where each is, V3 reports the header driver
fields, and **V10 is a raw-bytes backstop** that greps the finished file for
anything shaped like a person's identity or another run's time.

**Legitimate versus foreign is decided by POSITION, never by value.** On 173691
the map's author and the replay's driver are the same person and the same 22
bytes. The map's own attribution — the meta triple in `0x03093000`, `author=`
and `authorzone=` in the header XML, and everything inside the embedded map's
byte range — is left alone, and V10 excludes it by offset.

**Scope, measured: 0 of the 158 recordings published in this repo have a
replay header chunk table at all.** A plain `.Ghost.Gbx` has none, so none of
them can carry this. It bites the map-carrying containers. The positive control
for that sweep is the replay fixture, which does have one — and, worth saying
because the sentence at the bottom of this document claims otherwise, **that
fixture's header still carries `Ibozz91` and his account id.** It is kept that
way on purpose: `header.anonymise` needs a foreign value to start from. The
"every fixture has been anonymised" line below is true of bodies only.

**A cosmetic edit that quietly breaks the file.**
Found here, by the control: renaming the driver inside a **replay** from
`Ibozz91` to `TAS` produces a file where every string reads back perfectly and
which validates to **nothing** (7.241 → DNF). Two separate causes, both real:
a replay carries a map whose own chunk `0x0304305F` declares a size that runs
*past* the end of the carried map, so a naive chunk walk "corrects" a size word
**inside the map**; and the driver's strings sit in a nested node whose offsets
something else depends on.
→ the carried map is protected from chunk fixups; `ghost identity set` runs an
**oracle no-op control** (same time before and after) and **deletes its own
output** if the time changed; and with no server available it refuses a
length-changing edit it cannot frame rather than guessing. `--anonymise` pads to
the original byte length when it cannot shorten.

**A trimmed run that declares a time it does not achieve.**
→ `ghost trim` re-reads what it wrote and checks every claim: tape inside the
window, telemetry inside the window in **every** entity, record span, declared
time in every copy, checkpoints. A head cut says in plain words that the tape
now starts mid-run and is not something to hand the oracle. `ghost verify`
fails a DNF against a declared finish unless you say `--expect dnf`.

**A check that reads one operand off the command line.**
A manifest verifier once compared `--declared 8050` to `--oracle 8050` and
printed PASS without reading the file.
→ every check in `ghost verify` takes both operands from the file or from the
world. `--expect-ms` only ever *adds* a constraint; it cannot satisfy one.

**THE SERVER PRINTS TWO RESULTS AND THE SECOND IS THE FILE'S OWN CLAIM.**
Found here, by a control, in this tool's own code. Per file the server prints
`ValidatedResult` — the time it just simulated — and then `DeclaredResult`, the
result the file DECLARES, in the same shape with another `"Time"` line. A parser
that keeps reading `"Time"` lines takes the second one, so **"the oracle said
22.730" was the file saying 22.730**. Measured on a tape that simulates 22.738
and declares 22.730: the naive parse returned 22.730 and made a stale
declaration look correct — the exact failure mode every other check here exists
to prevent, inside the thing doing the checking.
→ the parser tracks WHICH block it is inside and keeps both, so the
disagreement is a value you can read (`SimResult::declaration_holds()`) rather
than a bug you can have. **Two fixtures pin it, and they have to be ASYMMETRIC
to pin anything**: on a file that passes, the two numbers are equal, so no
equal-number fixture can fail whatever the parser does.
`oracle.reads_the_world` simulates 22.738 and declares 22.730;
`oracle.dnf_with_declared_time` is the other shape — `ValidatedResult: null`
with a `DeclaredResult` of 15.000, which a careless parser reports as a 15.000
finish for a run that never finished. `ghost verify` refuses both.

**How many copies of that parser there were, and how many there are.** Six, in
one tree: here, `fk`'s regen (which took the first `"Time"` line and read a DNF
as a finish), `tmtraj`'s integrity gate (which read the TIME correctly and then
scanned forward past a null for `NbCheckpoints`, reporting four validated
checkpoints for a run the server refused), `tmmaps`'s map-surgery driver,
`tmsearch`'s, and `forkoracle`'s. Five of the six are gone: `fk`, `tmsearch`,
`tmtraj` and `tmmaps` all call `ghost::oracle::parse_many`. Each merge paid for
itself immediately — `tmmaps`'s brought two behaviours with it, the
`wrong simu → cps 0` sentinel (kept local, where its meaning is local) and the
huge-u32 "never crossed" time, which is now `sane_time` in the shared parser
because a 4 294 967.295 s "finish" is a bug for every caller
(`oracle.parse.sentinel`).

**`forkoracle`'s copy is the one that is still there, deliberately.** It reads
the fork server's TRUNCATED stream, which stops at `"IsValid"` and never prints
`"FileName"` — and `parse_many` completes a record only at `FileName`, so it
would return nothing at all on that input. Merging it needs a flush-on-EOF entry
point plus its `cps = Some(0)` sentinel preserved, and its consumer is the
search's scoring hot path: the one place in this project that has already paid
for a phantom. It is a copy with a reason and a note, not an oversight.

**A regenerated file whose locate found something that is not the car.**
The car used to be found by scanning memory for a self-consistent
(position, quaternion, velocity) triple. That is a DESCRIPTION of a car, not an
identification of one, and it has three failure modes at once: the engine holds
several objects that satisfy it, a frozen memory slot satisfies it *trivially*
(a constant position has a consistent zero velocity), and which one the scan
lands on varies between runs.

Three things were done about it, in order of how much they mattered.

1. **THE RIGHT LOCATE WAS ALREADY THERE AND WAS NOT BEING USED FIRST.** There
   are two: one forks, hunts the child for an object that moves like a car, and
   hands an address back to the parent; the other locates in the clean process
   itself and needs no cross-process assumption at all. The forking one ran
   first and usually won — with a decoy. Measured on the fixture map: **six runs
   through the in-process locate produced BIT-IDENTICAL trajectories, 13.7 s
   each; six runs on the default path took about 90 s and disagreed.**
   `ghost regen` now tries the in-process locate first, alone.
   It is not universal — it cannot see a car that is barely moving at the
   handover, and on map 279218's 5.355 s tape it finds nothing — so the search
   is still there behind it, and on that map it is what produces the file.
2. **The searching locate's own self-check could not see a frozen slot**, for
   the reason above. It now requires the candidate to trace a finite, moving,
   plausible path, and it collects candidates from every checkpoint in the
   ladder instead of stopping at the first that yields any.
3. **The gate identifies the car instead of describing it.** The decisive check
   is `G2`: *the run must start where this map's runs start*. The template is a
   recording of the same map from the same spawn, so its own first sample is the
   answer key — free, already in the file, no reference needed. On the fixture
   map the car starts 0.001 m from it and the surviving decoy — which traces a
   perfectly plausible 1.6 km path — starts 1629 m from it.

Two things measured along the way that are worth not repeating:
*diversifying the anchor tick makes it worse* (8 runs on the default ladder
found the car once; 24 runs over seven hand-picked ladders **had the chooser
land on the car zero times** — the car was in every one of those gathers, which
`fk whl carscan` recovers from a junk run's own dump, so what the ladder changed
was the pick and not what was there);
and *the "1 in 8" failure rate was partly an illusion* — five of six runs were
writing bit-identical trajectories and only looked different because the record
offsets are reported relative to differently-sized gather windows. **Comparing
the written trajectories is the measurement; comparing the log lines is not.**

What was tried and does NOT work on this binary, so nobody repeats it: the
clean type-directed route. `TrackmaniaServer` is stripped and has no RTTI, so
there is no typeinfo to walk. Its class names *are* in it as plain strings
(`CSceneVehicleVis`, `CSceneVehicleVisState`, `CGameCtnApp`) and its class ids
*are* in it as constants — `ghost engine idsites --class-id 0x0A018000` finds 63
of them, including `mov esi, 0x0A018000; call …` where the engine queries by
class — but the names live in a merged string pool referenced only by
RIP-relative code, so there is no descriptor table to read, and there is no
`mov eax, <classid>; ret` getter, so there is no vtable to find the instances
by. `ghost engine classinfo` and `ghost engine idsites` are in the tool so the
next attempt starts from the evidence rather than from scratch.

`fk regen --anchor <bias:pos:clock:quat:kind:vel>` was added as the escape
hatch: for a given binary and map the car sits at a fixed offset from the module
base, so an address established once can simply be supplied. It is not wired
into `ghost` because the in-process locate made it unnecessary.

If no attempt passes the gate, **nothing is written**.

**THE SERVER ONLY VALIDATES FILES WITH THE RIGHT EXTENSION, AND SAYS DNF
OTHERWISE.** Found here, and it cost 32 wrongly-refused regenerations before the
diagnostic went in. A candidate written as `out.try3` is not read at all, and
the answer is indistinguishable from a run that did not finish — so the gate
above rejected *good* regenerations as bad locates, twice in a row, with a
plausible story attached each time.
→ `ghost`'s oracle always links a file into `UserData/Replays` under a name the
server will read, whatever it is called on disk. A DNF now means a DNF.

---

## 4. The tape format

```
#gtape 1
#source /path/to/donor.Ghost.Gbx
#chunk_version 4
@archive 0 format_version=12 field0=0 start_offset_ms=-1580 packets=2432 bitstream_bytes=2151 bits_used=17203
@tail 0e00…
t=0 mode=2 w=lit:0x0000000E2 respawn=0 mouse=none vsame=0 steer=0 accel=1 brake=0 flags=0x000007
t=1 mode=2 w=prev respawn=0 mouse=none vsame=1 steer=0 accel=1 brake=0 flags=0x000007
```

* `t` is the tick index. **Race time ms = `t * 10 + start_offset_ms`** — check
  `start_offset_ms` before interpreting any tick-0 edit: most of this project's
  incumbents are countdown-prefixed, so writing `accel=0` at race 0 there is a
  one-tick *lift inside an existing hold*, not a change of onset.
* `w` records how the state word was coded, so the round trip is exact.
* `respawn` is authoritative: set it and the writer puts it in the literal.
* `vsame` records the original coding. Change a value and the writer expands
  that packet automatically — an edit is never dropped to keep a same-bit.
* `steer` is the signed i8 for an 8-bit field; a 32-bit steer field prints as
  `steer32=0x…`.
* `@tail` carries the bytes past the last packet, so a verbatim injection
  reproduces the original bitstream **exactly**.

---

## 5. The suite

```
ghost selftest              # pure + oracle, ~30 s
ghost selftest --engine     # + the real engine, several minutes
ghost selftest --strict     # a SKIP is a failure
cargo test --release        # the same suite, through cargo
```

50 checks over five checked-in fixtures: two human ghosts, one anonymised
replay that carries its own map, one file this project labelled
`DO_NOT_PUBLISH`, and one map. Three tiers:

* **PURE** — codec identity, tape round trip, bit identity, expansion, respawn
  write and refusal, the steer-byte table, identity reading, the inline-chunk
  account id, embedded-map detection, map-set round trip, trim coherence, the
  kappa separation, two refusals, a deterministic uncompressed whole-file image,
  and the oracle parser against canned server transcripts -- which need no
  server, no map and no 30 MB binary, so they are the checks that never get
  skipped.
* **ORACLE** — the donor's own time; expansion, injection and identity edits are
  no-ops; an edited tick actually changes the run (the writer is not a no-op);
  **the oracle reads the world and not the file's claim** on both asymmetric
  shapes, and `ghost verify` refuses the file that declares one time and does
  another; a three-file BATCH in one server launch, each result keyed to its own
  file; the empty-Maps control both ways; the map swap; the trim cases in both
  directions -- a cut that keeps the finish, a cut that honestly DNFs, and a
  tape LENGTHENED from 2432 to 7000 ticks that still re-simulates to 22.730;
  the declared checkpoint count measured against the server rather than
  believed; the rebind proved in both directions.
* **ENGINE** — the engine's own run of the fixture's tape against the recording
  in it (0.0005 m mean over 455 samples), and two independent regenerations of
  the same file agreeing to 0.000000 m.

### A worked round trip

```
ghost tape extract donor.Ghost.Gbx --out a.gtape     # 2432 ticks, codec identity OK
<edit a.gtape: one steer unit for 300 ms>
ghost tape inject donor.Ghost.Gbx edited.Ghost.Gbx --tape a.gtape
ghost declare edited.Ghost.Gbx declared.Ghost.Gbx --from-oracle --map map2.Map.Gbx
                                                     # the oracle says 22.738
ghost regen declared.Ghost.Gbx final.Ghost.Gbx --map map2.Map.Gbx
   [6] accepted
   G3 path length 1620.1 m over 455 samples
   G2 first sample at (1552.00, 34.00, 560.00)
   G1 tape/record agreement kappa 1.000 over 455 samples (best lag 0 ms)
   G4 oracle on the written file: 22.738
ghost verify final.Ghost.Gbx --map map2.Map.Gbx      # V1..V7 PASS
```

Every fixture that carries a person's identity has been through
`ghost identity set --anonymise`, and the suite proves that pass changed no
physics (7.241 before and after).
