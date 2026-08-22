# GHOSTS.md — the TM2020 ghost / replay API

Everything this project does to a `.Ghost.Gbx` or a `.Replay.Gbx`, in one Rust
binary, with a control behind every operation.

```
cd tools/ghost && cargo build --release
TM_SERVER=/path/to/TrackmaniaServer-dir ./target/release/ghost selftest
```

`ghost --help` lists every command. Times print as **seconds with a decimal**
(`22.730`), never as raw milliseconds.

---

## 1. The operations

| | command | what it does |
|---|---|---|
| inspect | `ghost inspect FILE` | container kind, the map it will actually run on, declared time and every copy of it, checkpoints, identity, the input tape, the telemetry record |
| **extract inputs** | `ghost tape extract FILE --out T.gtape` | every tick, every field the packet carries — full fidelity, round-trips byte for byte |
| **inject inputs** | `ghost tape inject IN OUT --tape T.gtape` | writes a tape back; every vehicle field explicit so no tick inherits another's |
| | `ghost tape expand IN OUT` | rewrite every "same as previous tick" packet explicitly; semantically a no-op and the oracle says so |
| | `ghost tape diff A.gtape B.gtape` · `stats` · `bits` | compare two tapes; summarise one; census which bits of the state literal ever vary |
| **car state** | `ghost regen IN OUT --map M` | run the real engine on this file's own inputs, capture per-sample car state, write it in — behind a gate that refuses a bad locate |
| | `ghost regen-control FILE --map M` | the fixed point: regenerate a file that already knows its own answer and require it back |
| **change the map** | `ghost map show FILE` | which map this file will *actually* run on, and whether `--map` is real for it |
| | `ghost map extract FILE --out M.Map.Gbx` | pull the carried map out |
| | `ghost map set IN OUT --map M` | replace the **carried** map — the only thing that moves a recording that has one |
| | `ghost map rebind IN OUT --map M` | rebind a **pure ghost** by uid — refused on a file that carries a map |
| **trim** | `ghost trim IN OUT [--from MS] [--to MS]` | cut head and/or tail keeping tape, telemetry, record span, checkpoints and every copy of the declared time coherent |
| **identity** | `ghost identity show FILE` | skin, display name, trigram, zone, club tag, login, account id, locator URL — with offsets |
| | `ghost identity set IN OUT --name N --trigram XXX --skin S [--anonymise]` | change them, with an oracle no-op control |
| added | `ghost declare IN OUT --from-oracle --map M` | write the time the file **actually does**, asked of the plain oracle, into every copy |
| added | `ghost verify FILE [--map M] [--engine] [--empty-maps]` | the acceptance gate |
| added | `ghost selftest [--engine] [--strict]` | the whole suite, one command |

### Things added beyond the six, and why

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
| checkpoints + race time | skippable chunk `0x0309202B`: `[?, race_time, ?, ?, ?, n, then n pairs of (checkpoint_ms, ?)]` — **not** a bare split vector |
| walltime | `0x0309202D` |
| skin / locator URL / **display name** | the front of skippable chunk `0x03092000`, before the record blob |
| trigram / zone / club tag | the **tail** of the same chunk, after the record blob |
| account id | **inline** chunk `0x0309200F` — no `PIKS`, not in the chunk table |
| map uid (pure ghost) | **inline** chunk `0x03092010`, plus literal copies |
| telemetry | `CPlugEntRecordData`, zlib, 50 ms samples of 116 bytes; the vehicle entity is `CSceneVehicleVis` `0x0A018000` |
| **the carried map** | `0x03093002` + a size word + a whole nested GBX — **inline, no `PIKS`** |

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

34 checks over five checked-in fixtures: two human ghosts, one anonymised
replay that carries its own map, one file this project labelled
`DO_NOT_PUBLISH`, and one map. Three tiers:

* **PURE** — codec identity, tape round trip, bit identity, expansion, respawn
  write and refusal, the steer-byte table, identity reading, the inline-chunk
  account id, embedded-map detection, map-set round trip, trim coherence, the
  kappa separation, and two refusals.
* **ORACLE** — the donor's own time; expansion, injection and identity edits are
  no-ops; an edited tick actually changes the run (the writer is not a no-op);
  the empty-Maps control both ways; the map swap; the trim cases; the rebind
  proved in both directions.
* **ENGINE** — the engine's own run of the fixture's tape against the recording
  in it.

Every fixture that carries a person's identity has been through
`ghost identity set --anonymise`, and the suite proves that pass changed no
physics (7.241 before and after).
