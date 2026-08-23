# POINTER.md — the structure that owns the vehicle state

`fk regen --carrier` used to find the car by **searching**: a 1.25 MB window of
engine memory streamed to disk at every instant of a run, then swept at every
4-byte offset for a float triple matching a position the file already knew. It
is a search for something with one right answer, and the engine has always held
the answer as a pointer.

**It does.** On `TrackmaniaServer` md5 `0f0f4b25f31f80c60c81404366c95e68`
(`date=2026-05-15_18_00 git=128182-0de74ece09e`):

```text
vehicles = *( *(module + 0x1e45148) + 0x148 )      the engine's vehicle array
car[k]   = *(vehicles + 8k)            k = 0..3    four vehicle objects
state    = car[k] + 0x46c                          the simulated car's vis state
position = state + 0x50                            CARRIER.md's `car` anchor
```

written as one spec, which is what `fk` takes and what
[`fk::ptr::DEFAULT_CHAIN`](fk/src/ptr.rs) holds:

```text
mod+0x1e45148:0:+0x148#4x8+0x46c
```

The vehicle objects are **0x1e08 bytes** apart and each vis state is a
**member** of one — which is why the first version of this search found nothing
but stack slots: it looked for pointers INTO the 864-byte struct, and what the
engine points at is the object the struct is inside.

```
fk ptr find  --template REC.Ghost.Gbx --map M.Map.Gbx     one engine run   [PROPOSES]
fk ptr check --template REC.Ghost.Gbx --map M.Map.Gbx --chain SPEC          [DECIDES]
```

---

## 1. What it cost, measured

Same command, same file, one flag apart (`--no-car-chain` is the old blind
window), on this 176-core box with `/tmp` on NVMe:

| | blind window | pointer | |
|---|---|---|---|
| 285885, one regeneration | 96.4 s | 79.5 s | **byte-identical output** |
| 191465, one regeneration | 108.2 s | 104.6 s | **byte-identical output** |
| 191465 TAS transplant, one regeneration | 169.7 s | 139.4 s | **byte-identical output** † |
| 285885, **24 at once** | 125.4 s | **94.4 s** | all 48 files one md5 |
| 285885, 24 at once, bytes written to the device | **8.86 GB** | **0.12 GB** | 74× less |
| what the field gather reads per instant | 1 310 720 B | 1 860 B | |

† On that file one run in four writes no file **either way**: several anchors
land on bare position copies and the gather refuses them. That is the decoy rate
FK.md's G7 describes, it predates this work, and the pointer neither fixes nor
worsens it — when the pointer misses, the run falls back and fails exactly where
it failed before.

**The bit-identity is the control that matters**, not the clock: the pointer
changes WHERE the gather looks and nothing about what is done with it, so the
regenerated file has to come out the same. It does, on two maps, and the 24-way
batch produces one md5 across all 48 files.

The wall-clock win here is modest because this box is not I/O-bound — a
regeneration is three engine runs and the disk was never the limit. On a box
where 24 parallel regenerations *are* I/O-bound (the 11–12 minute regime this
work was sent after), the number that moves is the 74×.

## 2. How it was found, and the controls

One engine run does the whole thing (`fk ptr find`):

1. **Snapshot every writable mapping while the engine is HALTED** at the shim's
   handover — 120–160 MB, 0.07 s. Halted matters: a pointer and the object it
   points at are then read from the same instant, so a missing pointer cannot be
   a torn read.
2. **Run the tape** and identify the car exactly as `gather_fields` does — the
   copy whose position reproduces the recording's own path (0.000000–0.000008 m)
   AND whose four wheel-rotation slots at car+92/+136/+180/+224 hold live
   floats.
3. **Walk the snapshot backwards** from that address: every 8-byte slot holding
   a pointer into the target, then every slot pointing at one of those slots, to
   a depth of four, stopping at slots in the game binary's own writable data.

**The control for the negative.** "Nothing points at the car" and "the scan
cannot see the slot that does" are the same output, so the command plants
nothing and instead draws slots that DO hold pointers, hides their values, and
asks the same scan to find them again: `recall control: 212 of 212`, printed
before any result, and a shortfall aborts. The test
`the_scanner_finds_a_pointer_this_test_planted` runs the same scan against a
needle whose address the test knows, in this process.

**The control for the positive.** A chain is not believed because it was found;
it is resolved in a FRESH server and graded (`fk ptr check`). Measured, three
maps, three processes each independent of the process the chain was found in:

| map | median from the recording | p90 | worst | paired instants | wheels |
|---|---|---|---|---|---|
| 191465 | 0.000000 m | 0.000122 | 0.000136 | 262 | 4 of 4 |
| 267460 | 0.000004 m | 0.000062 | 0.000136 | 462 | 4 of 4 |
| 285885 | 0.000008 m | 0.000122 | 0.000173 | 1225 | 4 of 4 |

**And the guard can fail**, which is the other half of that. Pointing the same
chain at a NEIGHBOURING array element is refused: elements 0 and 2 of the array
hold something that is not a vehicle object, and the gather centred on them
comes back four bytes wide and is rejected rather than graded.

**An independent `find` on each of the three maps produced the same root**, in
three separate processes with three different load addresses.

## 3. The two vis states — and why three maps of evidence could not tell them apart

**Each vehicle object carries two `CSceneVehicleVisState`s, at +0x46c and
+0x848.** On a file whose record IS its own run — every downloaded human
recording — they hold the same state to a micron, and a search picks between
them arbitrarily. The first version of this chain named `+0x848` alone and
passed everything put to it: three maps, six processes, byte-identical
regenerations, the acceptance test green every time.

Then it was run on a **transplanted container** — a published TAS ghost whose
telemetry record is a stranger's — and the two separated:

| | distance from the tape the engine simulated | distance from the record the file carries |
|---|---|---|
| the state at `+0x848` | **977.998 m** | 0.93 m |
| the state at `+0x46c` | **0.000477 m**, 4 of 4 wheels live | — |

So `+0x848` is not the simulated car at all; it follows the file's own recorded
path. On a recording those are the same line, which is why no amount of
evidence from recordings could have said so. **A chain that is right on every
file where the record is the run is not thereby a chain to the car**, and the
acceptance test is what said so out loud: it refused the state, printed
`the state the chain named is 977.997965 m from it`, and the run fell back to
the blind window and still wrote its file.

So the spec names `+0x46c`. Measured, four runs of a recording and four of the
transplant with exactly that spec: the pointer resolved on **7 of 8** (the
eighth fell back to the blind window, which then failed too — that file's own
decoy rate, and it fails the same way without a pointer), and **every file it
wrote is byte-identical to the blind window's** — `5d16e45e…` on the recording,
`ba5f62c1…` on the transplant, on every run.

Naming BOTH members was tried and is worse: one run in six then picked
`+0x848`, which also qualified in that process, and wrote different bytes. The
comma form (`#4x8+0x46c,+0x848`) stays in the spec language because comparing
the two members is how the right one gets chosen on a new build — it is a
calibration tool, not a default.

`fk ptr find --truth engine` is what found this: it runs the clean gather first
and identifies the car against the positions THE ENGINE just measured, instead
of against the file's record. On a transplant that is the only reference that
means anything, and it is the reference `gather_fields` was already using.

## 4. The array — and the index that was a coin flip

The first chain published here was `…:+0x8:+0x848`: array element **1**, fixed.
It resolved to the live car on six runs across three maps. Then element **3**
resolved to a live car too, on a seventh run of the same recording — same
acceptance, 0.000000 m, four live wheels.

Both cannot be a property of the index. **Which element of the array is the live
copy varies by process**, and a fixed index was a coin that had come up heads six
times.

So the spec names the ARRAY (`#4x8+0x46c` — four elements, stride 8, the state
at +0x46c of each) and every element is gathered. The choice between them is made
by the rule that was already making it: the copy that reproduces the run's own
measured path with four live wheels. The pointer's job is not to be right about
which copy; it is to turn 300,000 candidates into four.

Two of the four slots hold a usable vehicle object in a solo validation run and
two do not; unreadable ones are dropped when the spec is resolved (proved by a
read at `state` and at `state+0x358`, not assumed).

## 5. One chain to avoid, written down rather than dropped

The first root this found was `mod+0x1d56e48`, at depth 2, and it works — three
maps, three processes, every acceptance passed.

It is also **not a data structure**. The function at `f20700` does
`lea -0x38(%rbp)` and stores that into the global, so `mod+0x1d56e48` holds the
address of a **dead stack frame**, and the chain only resolves because that
frame's contents are deterministic. The disassembly says so in four
instructions: two writes, both of a stack address, and two reads
(`asmdig xref /tmp/ts.asm 1d56e48`).

A chain can be perfectly reproducible and still be built on nothing. The
default root is instead `mod+0x1e45148`, which the whole binary writes **once**
(`19a96b7 mov %rsi,0x49ba8a(%rip)`) and reads nowhere — a singleton set at
start-up. Two further roots reach the same array on all three maps and are in
the source as second witnesses.

## 6. What the pointer does NOT change

* **The transform.** `fk regen`'s clean run is untouched — same anchor ladder,
  same 452-byte window, same copy. A regenerated trajectory is bit-identical to
  one from a run with no pointer at all. The half-millimetre shadow question of
  `CARRIER.md` §6 is exactly where it was.
* **The acceptance.** `gather_fields` applies the same tests to a pointer window
  as to a blind one: the copy must reproduce the clean run's own measured path
  to 1e-3 m and all four wheel slots must be live, and a failure falls back to
  the blind window and says so on its own line. **A stale chain cannot produce a
  file. It can only cost the time the search would have cost anyway.**
* One check it DOES skip, deliberately and visibly: the orientation hunt, which
  searches ±4 KB for a varying unit quaternion. A pointer window is the struct
  itself, and the struct holds `Loc`'s 3x3 rotation rather than a quaternion, so
  there is nothing there to find. That hunt only feeds `--transform-from-fields`
  (default off), and `fk regen` **refuses** the two flags together rather than
  serving a guess.
  Worth knowing, because it is a difference in behaviour and not only in speed:
  on 267460 the blind path aborts on that hunt's veto after six anchors and
  writes no file, where the pointer path writes one in 101 s.

## 7. Recalibrating for another build

Every number above is a property of one binary. On a new build:

```bash
fk ptr find --template A_DOWNLOADED_RECORDING.Ghost.Gbx --map M.Map.Gbx
fk ptr check --template ANOTHER_RECORDING.Ghost.Gbx --map ANOTHER.Map.Gbx --chain '<spec>'
```

`find` defaults to identifying the car against the FILE's recorded path, so it
must be run on a **recording of the run** — a downloaded human ghost, or a file
whose telemetry is already regenerated. On a transplanted container pass
`--truth engine`: it runs the clean gather first and identifies the car against
the positions the engine measures for the tape, which is the only reference that
is the run. Do both, on the same map, and compare the two answers — that
comparison is what found the second vis state.

Then put the spec in `fk::ptr::DEFAULT_CHAIN` with the binary's md5 beside it,
and check it on a second map before trusting it. Until then `--car-chain SPEC`
takes one for a single run and `--no-car-chain` turns the pointer off.
