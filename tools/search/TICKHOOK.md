# TICKHOOK.md — the fork server's clock is the engine's own tick

The fork server used to count `lroundf` calls. It now hooks the function the
validator calls once per simulated tick, and every checkpoint, stride, budget
and deadline is in **ticks of race time**.

There is ONE clock and no way to ask for another: the shim installs the hook or
exits 92. What it replaced -- a count of `lroundf` calls -- is gone from the
tree; the numbers it produced are kept below, because they are why the hook
exists.

---

## 1. Why the old clock had to go

`lroundf` is called ~255 times per simulated tick and the total for a given
(map, ghost) is bit-identical **on an idle box**. Under load it is not: the
count moves in whole chunks of ~62 calls (120 concurrent runs gave
615124…615372, 352 gave 614875…615434). 62 calls is about a quarter of a tick,
so a fixed count lands at a different simulation point in every process.

That is not a mystery any more. The validator's frame function
(`0x1218db0`) advances the simulation in a loop of 10 ms ticks until it reaches
the frame's target time **or a wall-clock budget runs out**:

```
1219786:  mov  [rbp-0x30], eax          ; new_time = sim.time + 10
1219789:  mov  rdi, [rbp-0x128]
1219790:  call 19a1a00                  ; now()  (wall clock)
1219795:  sub  eax, [rbp-0xbc]          ; elapsed since the frame began
121979b:  cmp  eax, [rbp-0xc0]
12197a1:  ja   121a150                  ; BUDGET SPENT: leave the loop here
```

Under contention that branch fires, the same run is cut into more frames, and
each extra frame costs a fixed lump of per-frame work — the ~62 calls. So the
drift was never noise in the physics: it is the engine's own frame partition,
and it is invisible to anything measured in ticks.

The consequences are all over the project's notes: per-worker boundary probes,
per-worker calibration, the "probe+1, max over workers" floor, per-worker tick
labels (phantom defect 3), and the mid-tick resume that produced phantom
defect 1. **The tick hook removes the cause rather than compensating for it.**

Measured on this box, 300 fork servers started together and asked for the same
checkpoint (`fk tickhook load --n 300 --at tick:171`):

| clock | distinct stop points | probe agrees with the requested tick |
|---|---|---|
| `lroundf` | **10** (probe 277…287) | — |
| `tick` | **1** (probe 170 on all 300) | 300 / 300 |

---

## 2. The tick loop, build 128182

`TrackmaniaServer`, 30 113 288 bytes, md5 `0f0f4b25f31f80c60c81404366c95e68`,
`date=2026-05-15_18_00 git=128182-0de74ece09e`.

`/validatepath` reaches the frame function through the validation job
(`0x118c6c6: call 0x1218db0` with `edx = 10`, `ecx = -1`). Its body, per tick:

| offset | what |
|---|---|
| `0x1219786` | `[rbp-0x30] = new_time` (`= sim.time + 10`) |
| **`0x12197eb`** | **`call 0x119e060(players, n_players, new_time, dt)` — the tick function, FIRST in the body** |
| `0x1219f89` | per player: `call 0x119f0f0` — copies input record `(new_time − race_start)/10` |
| `0x121a071` | `call 0x119f1b0` — the physics step |
| `0x1219750` | `sim.time (= [sim+0x48]) = new_time`, loop |

`0x119e060` is the hook point, and it is the right one for three reasons:

1. **Once per tick, exactly.** Under gdb, on a 2401-tick run, `0x119e060` was
   entered 2401 times and the loop's clock write executed 2401 times. Ten runs
   across three maps agree (§5).
2. **Before the tick's input is read.** The record for this tick is copied
   later in the body (`0x119f0f0`), so a fork taken at the hook has consumed
   *nothing* of it — the resume boundary is a function of the tick index.
3. **Its prologue is relocatable**: `push rbp; mov rbp,rsp; push r15..rbx;
   sub rsp,0x18` — 17 bytes with no RIP-relative operand, so they can be
   re-executed from a trampoline anywhere in the address space.

### The record, the read, and the two things that were wrong about them

The record is 32 bytes and the engine fetches it whole
(`0x119f165: shl rcx,5; movups xmm0,[rdx+rcx]; movups xmm1,[rdx+rcx+0x10]`),
so element `i` starts at `array + 32i`. Read out of a running server rather
than inferred:

| offset | contents | how it is known |
|---|---|---|
| `+0x00` | `u32` flags, normally 2 | the "no input" path ORs `0x40` into the byte at `+0` (`0x119f1a2`) |
| `+0x04` | `f32` steer = `(i8)steer / 127` | the shim's key match, and it equals the tape at every tick |
| `+0x08` | `f32` gas | same |
| `+0x0c` | `f32` brake | same |
| `+0x10` | `f32` 0 | measured, every record |
| `+0x14` | `u32`, engine-owned | **NOT the constant the old notes called a "device-segment const"**: `0x3576f40e` early on one tape and `0x3576f409` later. Never read, never written |
| `+0x18` | `u32` 0 | measured, every record |
| `+0x1c` | `u32` 2 | the "no input" path writes `2` to the byte here (`0x119f14c`) |

**The base was four bytes into that record until 2026-09-06.** The shim finds
the array by searching for the tape's STEER values, and it used the address of
tick 0's steer as `base`. Every field was still written to the right address —
the patch is 12 bytes at `base + 32t`, which is steer/gas/brake either way —
but the page-fault probe divides a *record-aligned* fault address by 32, and
`(32i − 4) / 32 = i − 1`. **That is where the `probe + 1` every resume carried
came from.** It had been read for a year as "tick p is already partly
consumed"; it was an offset. The base is now the record base, the patch writes
at `+4`, the probe protects exactly `[base, base + 32n)`, and the probe reports
the record the engine reads next — so the resume floor is `probe`, and the
forward-only rule refuses BELOW the boundary and accepts AT it.

**The countdown reads record 0.** Before race time −10 ms — or while the race
start is still `-1` — the engine does not index the tape at all: it copies
record 0 verbatim as that tick's input (`0x119f0fc..0x119f127`). So on a tape
whose `start_offset_ms` is −1580, records 1..156 are **never read**, and record
0 is the input for the whole countdown. That is the mechanism behind a fact the
phantom investigation could only measure: rewriting the countdown region is
inert. `clock::record_read_at` is that rule transcribed, and it is what the
boundary control compares the probe against.

---

## 3. The hook

Before `main`, the shim:

1. resolve the main module's base from `/proc/self/maps`;
2. check that all three signature ranges are inside its **executable** mapping
   (a host that is not the server — `shimhost`, `/usr/bin/time` — is refused
   here rather than segfaulting);
3. compare three byte signatures: the tick function's prologue, the loop's
   call site, and the loop's clock write. The call site's `rel32` must resolve
   to the tick function — the offsets are checked against each other, not just
   against bytes;
4. `mmap` a trampoline: save `rax rcx rdx rsi rdi r8-r11` and `xmm0-7`, call
   `tick_entry(new_time, dt)` with `edx`/`ecx` moved into `edi`/`esi`, restore,
   run the 17 displaced prologue bytes, `jmp [rip+0]` back to
   `0x119e060 + 17`;
5. overwrite those 17 bytes with a 14-byte absolute `jmp [rip+0]` + the
   trampoline address (padded with `int3`), under `mprotect` RWX→RX.

Any failure is `_exit(92)`. **There is no other clock to fall back to**, which
is the point: a silent fallback is how a wrong number gets believed.

`tick_entry` carries what the `lroundf` interposer used to: the sampler/watchdog
dispatch, the SIGSTOP stop point and the fork-server checkpoint, keyed on the
tick.

Two hosts are not the game and are handled by fact rather than by a flag. A
shim LINKED INTO a binary (the crate's own `cargo test`) hooks nothing — its
constructor sees its code inside the main executable. `shimhost`, the engine
stand-in the savestate-tree tests run against, has no tick function to hook, so
it sets `FKSHIM_TEST_HOST=1` and calls the shim's `fkshim_tick(new_ms, dt,
race_start_ms)` seam once per tick; a real server never gets that licence.

### The clock is RACE time, not simulation time

The simulation starts at ~1000 ms and the race — input record 0 of a tape with
`start_offset_ms = 0` — starts later. **That start is not a constant.** It is
usually 2200 ms, and in 1–6 of 300 servers started at once it was 2300: the
spawn is scheduled off the same frame clock whose partition made `lroundf`
drift. Keying on simulation time would have imported exactly the defect this
work removes.

So the shim reads the start out of the engine, exactly where the tick loop
reads it before applying inputs (`0x1219f3b..0x1219f66` → `0x121cf50`):

```
race_start = max( [[playground+0x968] + 0x3c],      // round start
                  [participant + 0x218] )           // participant spawn
             , unknown while either is -1
playground   = [sim + 0x18]                          // sim from the validator
players      = [playground + 0x660], count [+0x668] == 1     // VALIDATOR_CAR.md
participant  = [players][0]
```

It is read until it is known and then frozen, and the clock is

```
clock = (new_time − race_start) / 10 + 1000        // +1000 keeps the countdown positive
```

Every hop is null-checked and count-checked; before the race exists the hook
counts ticks and keys nothing. The handshake carries `tick <sim_ms>
<race_start>` so the driver can convert and check.

### Checkpoints

```
ckpt_for_race_ms(ms)          = ms/10 + 1000
ckpt_for_tick(t, start_off)   = (10t + start_off)/10 + 1000
tape_tick_at(sim, start, off) = (sim − start − off)/10
```

`forkoracle::clock` is the only place these live; `fk::session::clock_for_*`
and `tmsearch::forkeval::clock_for_tick` forward to it.

---

## 4. The controls

The point of a hook is that a wrong one must **fail** a check, not go quiet.

**In-process, every run** (`FKSHIM` lines on stderr):
* `tick_anomalies` — entries whose `dt != 10`. Must be 0.
* `tick_clock_mismatch` — entries where the validation simulation's own clock
  `[sim+0x48]` was not `new_time − 10`. Must be 0. This is the hook asserting
  it sits at a tick boundary *of this simulation*, not of something else.
* `clock_total` vs `(sim_ms_end − race_start)/10 + 1000`.

**Against the engine, every fork server**: `ForkServer::boundary_tick` runs the
page-fault probe and requires it to equal the record that the reported
`sim_ms`/`race_start` say the engine reads next. The two are measured from
opposite sides — one is where the engine says it is, the other is which record
it actually faults on — so agreement is a real check and a disagreement is a
hard error, never a number to choose between. `tmsearch` and `fk` both go
through it.

**Every read of engine memory, on demand**: `fk tickhook reads` stops a server
and compares all five against the engine (§4.1).

**Negative controls, run:**

| control | result |
|---|---|
| hook the physics step (`0x119f1b0`) instead | 2401 `dt != 10`, 2401 clock mismatches, race start never found — **rejected** |
| a host with no server text (`/bin/true`, `/usr/bin/time`) | exit 92, "signature offsets are not inside the main module's text" |
| `FKSHIM_TICK_FN_OFF` without `FKSHIM_TICK_UNSAFE=1` | exit 92, refused |
| an override target whose prologue is not relocatable | exit 92, refused |
| a handshake with no tick clock, or a race start of 0 | the driver refuses the server |
| the probe ≠ the engine's own tick | hard error naming both numbers |
| a write BELOW a node's boundary (savestate tree) | refused; a write AT it is accepted **and changes the verdict** |

### 4.1 Every read of engine memory, and what it is checked against

The oracle reads five things out of a running server. Until `fk tickhook reads`
existed, two of them had ever been checked against the engine's own arithmetic
— and **three of the five were wrong**.

| # | read | checked against | verdict |
|---|---|---|---|
| 1 | the input array's base, and the record the engine reads next | the tick the hook reports, vs the page-fault probe — two independent measurements | **was wrong**: base 4 bytes into the record (`probe + 1`); fixed |
| 2 | the record's fields at `+4/+8/+12` | the tape, tick by tick; and the engine-owned tail against its own writes | right; the tail's `+0x14` is **not** the constant the notes claimed |
| 3 | the u32 the locators call "the race clock" | the race time the hook knows exactly | **was mislabelled**: it counts from the ROUND start (1200 ms), and its bias was FITTED (1000 in one run, 1020 in another). Now measured from the stopped parent: 1000 every time |
| 4 | the car's position | the validator's own ownership chain — a different object, no shared evidence | right, and now quantified: the two are the same car exactly one tick apart |
| 5 | `[sim+0x48]`, the simulation's own clock | `new_time − 10`, on every tick | right; 0 mismatches in every run |

Read 3 is the one with teeth beyond tidiness. The label a trajectory sample
carries is `counter − bias`, and a fitted bias is an assumption about which
tick a child's first sample belongs to: measured, that assumption was two ticks
out between two runs of one tape — which is exactly "the fork child's tick
labelling shifts by a whole tick between workers", recorded as phantom defect
3 and never explained. `layout::measured_clock_bias` reads the counter in the
stopped parent, whose tick the hook knows, so the origin is arithmetic.

Measured at three checkpoints on map 2 (`fk tickhook reads`, all PASS):

| map, checkpoint | probe vs engine | counter − bias | bias | blind car vs validator car |
|---|---|---|---|---|
| map 2, `tick:171` | 171 = 171 | 120 ms = race of the finished tick | 1000 | 0.0175 m at 1.73 m/s = **1.01 ticks** |
| map 2, `tick:1200` | 1200 = 1200 | 10410 ms | 1000 | 0.8360 m at 83.58 m/s = **1.00 ticks** |
| map 2, `tick:2313` | 2313 = 2313 | 21540 ms | 1000 | 0.8523 m at 85.16 m/s = **1.00 ticks** |
| 126859, `tick:300` | 300 = 300 | 1440 ms | 2200 | 0.2248 m at 22.58 m/s = **1.00 ticks** |
| 126859, `tick:1500` | 1500 = 1500 | 13440 ms | 2200 | same object (0.0000 m) |
| 145875, `tick:500` | 500 = 500 | 3450 ms | 2200 | same object (0.0000 m) |

The bias is 1000 on map 2 and 2200 on the other two: the counter's origin is
per race, which is the whole reason it has to be measured per server rather
than fitted once. Note also that the two locators land on the SAME object on
some maps and on two objects a tick apart on others -- so "same address" was
never the right check, and "same car" is.

---

## 5. The measurements

All on `devvm62717`, 166 cores, server md5 `0f0f4b25…`.

**Tick counting** (`fk tickhook count --gdb`) — plain run vs hooked run of the
same tape, plus an independent gdb count with no shim at all:

| map | ghost | ticks (hook) | ticks (gdb) | sim span | `dt≠10` | clock mismatch | time plain / hooked |
|---|---|---|---|---|---|---|---|
| map 2 | rank00001 | 2401 | 2401 | 1000→25010 | 0 | 0 | 22.730 / 22.730 |
| map 2 | rank00100 | 2501 | 2501 | 1000→26010 | 0 | 0 | 22.884 / 22.884 |
| map 2 | rank10000 | 2501 | 2501 | 1000→26010 | 0 | 0 | 23.286 / 23.286 |
| 145875 | r01 | 801 | 801 | 1000→9010 | 0 | 0 | 6.346 / 6.346 |
| 145875 | r05 | 801 | 801 | 1000→9010 | 0 | 0 | 6.380 / 6.380 |
| 145875 | r09 | 801 | 801 | 1000→9010 | 0 | 0 | 6.424 / 6.424 |
| 126859 | rank01 | 2601 | 2601 | 1020→27030 | 0 | 0 | 24.342 / 24.342 |
| 126859 | rank12 | 2901 | 2901 | 1010→30020 | 0 | 0 | 27.449 / 27.449 |
| 126859 | rank20 | 3401 | 3401 | 1020→35030 | 0 | 0 | 32.089 / 32.089 |
| 126859 | rank22 | 4601 | 4601 | 1020→47030 | 0 | 0 | DNF / DNF |

`tick_total == (sim_end − sim_start)/10` on every row. (`fk tickhook count`
reports the DNF row as a FAIL because it insists on a validated time; the tick
criteria all pass.)

**Determinism under load** (`fk tickhook load`, map 2 rank00001, `tick:171`).
The lroundf rows were taken on the same box before that clock was deleted:

| clock | servers | distinct stops | probe |
|---|---|---|---|
| tick | 150 | 1 (`clock 1013`) | 150/150 agree |
| tick | 300 (×3 runs) | 1 (`clock 1013`) | 300/300 agree, every run — including the 1–6 servers whose race start was 2300 |
| lroundf | 300 | **10** (probe 277…289) | n/a |

At a late checkpoint and on another map (150 servers each):

| map, checkpoint | clock | distinct stops | race starts seen |
|---|---|---|---|
| map 2, `tick:2313` | tick | 1 (`clock 3155`, probe 2312) | {2200} — 150/150 agree |
| map 2, `tick:2313` | lroundf | **2** (probe 2327, 2328) | n/a |
| 126859, `tick:1500` | tick | 1 (`clock 2345`, probe 1499) | **{2200, 2300}** — 9 servers started their race 100 ms later and still reported the same race tick and the same probe; 150/150 agree |

That last row is why the clock is keyed on race time and not on simulation
time: those nine servers are at a different `sim_ms` and the same tick.

**Exactness** (`fk server check`, 50 candidates each, fork vs full validation):

| map | ghost | checkpoints | result |
|---|---|---|---|
| map 2 | rank00001 | tick 171, 1200, 2200, 2380, frac 0.994 | 250/250 exact |
| map 2 | rank01000 | tick 400, 1800, 2300 | 150/150 exact |
| 126859 | rank01 | tick 300, 1500, 2500 | 150/150 exact |
| 145875 | r01 | tick 200, 500, 700 | 150/150 exact |

**700 / 700 identical, 0 mismatches** (plus 300/300 more re-run after the read
audit, at three checkpoints on each of the other two maps), oracle self-repeatability 0 disagreements
throughout, identity resume exact everywhere. In every one of the 14 runs the
calibration sweep left the boundary where the probe put it. (Those runs predate
the record-alignment fix, so they read `boundary tick N (probe N−1)`; three of
them were repeated after it -- 150/150 exact at ticks 171/1200/2380 -- and now
read `boundary tick N (probe N)`.)

**Cost.** 15 full validations of map 2 rank00001, median wall time: no shim
**2.350 s**, lroundf shim **2.376 s**, tick shim **2.360 s**. Per candidate
(`fk server bench`, n=60): tick 68.5 / 42.3 / 13.1 ms at ticks 171 / 1200 /
2313, lroundf 68.7 / 40.9 / 12.6 ms at the same *requested* ticks — the lroundf
runs are a hair cheaper only because their stop lands later (probe 2327 vs
2313), i.e. they simulate fewer ticks. The hook itself is free.

**The watchdog and the per-tick sampler.** `fk watch measure`, 60 candidates,
same seed, both clocks resumed at the same boundary (278), taken while both
clocks still existed: identical verdicts —
21 non-tripping candidates bit-identical armed vs unarmed, 0 disagreeing with
the full validation, 0 perturbed by watching, the same 39 trips with the same
per-predicate breakdown, the same false positive (`c0055`), the same progress
figures to 0.1 m, score safety 60/60. The only differences: the abort landed at
tick 2476 under the tick clock and 2477 under lroundf, and the identity run
sampled 2161 ticks vs 2163 — the hook samples at the start of each tick, so an
abort lands at most one tick earlier, which the score-safety invariant
(`progress(aborted) ≤ progress(unarmed)`) allows and 60/60 confirmed.

**A search, end to end.** `tmsearch --fork --forktick 171` on map 2, 24
workers, 10 minutes, seed 42, phantom guard on, and the boundary-stress window
that historically produced phantoms (`--lo 171 --window 60 --stride 400`), run
once under each clock, while both still existed:

| build | evals | eval/s | banked | phantoms | best |
|---|---|---|---|---|---|
| tick clock | 272 910 | 454 | 9 | **0** | 22.711 |
| lroundf clock | 236 130 | 391 | 9 | **0** | 22.711 |
| tick clock, after the read audit (`from = probe`, one tick earlier) | 274 590 | 456 | 9 | **0** | 22.711 |
| final build (bias fix, calibration off the per-run path, 96-tick clock hunt) | 275 760 | 458 | 8 | **0** | 22.711 |

The third row is the one that matters for the audit: the resume floor moved a
tick earlier when the probe stopped reporting one record late, so every
candidate could now edit a tick the old code could not. If record `probe` were
in fact already consumed, those edits would be silent no-ops scoring exactly
the incumbent -- the phantom signature -- and the guard would have caught them.
It caught nothing.

Same answer, same number of confirmed improvements, no phantoms either way (the
guard has been on since the phantom work, and 10 minutes is not a phantom-rate
measurement — the base rate is low, which is exactly why the defect survived
four investigations). The eval/s gap is not a clean speed claim: the two arms
resumed at different boundaries (tick 171 vs the lroundf stop at 278, i.e. 2261
vs 2154 tail ticks) and ran at different times on a shared box. Per-candidate
cost is equal within noise (§ above).

---

## 6. `fk tickhook`

```
fk tickhook check                              do the constants match this binary? (static, reads the ELF)
fk tickhook count --tape G --map M [--gdb]     hooked vs plain run, all the criteria
fk tickhook load  --tape G --map M --at T --n N   N servers at once
fk tickhook reads --tape G --map M --at T      every read of engine memory, vs the engine
fk tickhook cost  --tape G --map M --at T      where a candidate's milliseconds go, measured in the child
fk tickhook finish --tape G --map M --at T      hunt the race result in memory (--chain: backward pointer scan)
fk tickhook finishfind --tape G --map M --at T  which word changes at the finish (--object result)
fk tickhook finishcheck --tape G --map M --at T --n N   the fast finish vs the JSON, same child
fk tickhook find  --tape G --map M             a new build: which function is the tick?
```

## 7. A new server build

`fk tickhook check` fails first — that is the signal. Then:

1. **`fk tickhook find`.** It runs the page-fault probe under the legacy clock
   to get the fault's frame chain (the return address inside the tick loop),
   disassembles nothing: it takes every `call rel32` in a window around that
   return address whose target begins with a relocatable frame-pointer
   prologue, hooks each one in the shim's **finder mode**
   (`FKSHIM_TICK_FN_OFF=0x… FKSHIM_TICK_UNSAFE=1`, prologue-shape check only),
   and keeps the ones that behave as the tick: entered ≥100 times,
   `dt == 10` always, `[sim+0x48] == new_time − 10` always,
   `ticks == (sim_end − sim_start)/10`, and the same validated time as the
   plain run. It prints ready-made constants for the winner.
2. **The clock write** (`TICK_CLOCK_WRITE_*`) is not found automatically:
   disassemble the loop around the reader's return address and take the
   `mov [sim+0x48], new_time` at the end of the body.
3. Update `forkoracle/src/tickhook_sig.rs` — one file, `#[path]`-included by
   the shim (which has no dependencies by design) and used by `fk tickhook
   check`, so the two cannot drift.
4. Re-run `fk tickhook check`, `count --gdb`, `load --n 150`, and
   `fk server check` before trusting a number.

If the race-start hops move (`+0x18`, `+0x660`, `+0x668`, `+0x968`, `+0x3c`,
`+0x218`), the shim will simply never find a race start: `clock_total` stays
`u64::MAX` and `ForkServer::start` refuses the handshake with "the shim stopped
before the engine set a race start". Re-derive them from `VALIDATOR_CAR.md`'s
chain plus `0x1219f3b..0x1219f66`.

## 8. What this does NOT change

* **The regime limit stands.** The fork server is exact for late perturbations
  of a human seed and lied on 312 of 312 reported finishes outside that regime.
  A tick-exact clock does not make a cold-start resume trustworthy.
* **The plain oracle still decides every banked number.** The guard is
  unchanged and still on by default.
* **The page-fault probe is still run**, on every server, every time — as the
  control on the hook rather than as the mechanism. It has been demoted, not
  deleted, and that is deliberate: two independent measurements that must agree
  are worth more than either alone. It is also how the four-byte base error was
  caught, months after every downstream number had been checked.

---

## 9. Where the time goes, and what was done about it

vjeux: *"investigate how to make all these operations and fork way faster? I
feel like there's a lot of wasted overhead."* Measured first, then cut what the
measurement named.

### 9.1 A candidate, phase by phase — measured INSIDE the child

Every earlier estimate of the per-candidate fixed cost was by subtraction
between protocol paths, and that measures the paths (the sampled path's own
transport costs more than the finish path it was meant to price). So the shim
now carries a **MAP_SHARED timing page**: the child stores when it started, its
first tick, its last tick and its tick count (three stores per tick, no
syscalls), and the parent appends them to `FKTIME`. `fk tickhook cost` reads it.
Map 2 rank00001, 40 runs per phase:

| phase | `tick:2313` (119 ticks) | `tick:1200` (1232) | `tick:171` (2261) |
|---|---|---|---|
| fork → child alive | 1.88 ms | 2.66 | 2.81 |
| child → its first tick (COW, shim entry, patch) | 0.55 | 0.58 | 0.57 |
| first tick → last tick | 4.70 (37 µs/tick) | 34.53 (27.9) | 65.38 (28.8) |
| **last tick → first output byte** | **5.82** | **6.09** | **5.79** |
| first byte → answer (pipe, parse, SIGKILL) | 0.06 | — | — |
| total | 12.81 | 43.85 | 74.56 |

Across the three checkpoints the end-to-end model is **9.9 ms fixed +
27.6 µs per tick**, and the fixed part is now itemised: 1.9 ms is the fork
itself (a null fork + reap of the paused engine costs 1.7–2.0 ms — the floor
for a ~150 MB address space), 0.55 ms is the child reaching its first tick,
and **5.8 ms — constant, 44 % of a late-checkpoint candidate, 8 % of an early
one — is spent inside the child between its last simulated tick and its first
byte of output.** The transport after that is 0.06 ms. So the waste is not the
pipe, not the JSON parse, not the kill: it is the validator's own
finish-and-print path, run by every child for an answer that is a single
number.

Only 7 of those ticks are simulation (the engine runs 70–80 ms past the
finish, 0.26 ms); the rest is the validator.

### 9.2 What it would take to skip it, and why it is not done yet

A child that knew its own finish time could `_exit` at the finish tick and
report through the timing page — no validator epilogue, no JSON. The finish
time is a tick boundary (every validated time is a multiple of 10 ms), so it
is exactly the race clock at the tick the finish is *detected*: the missing
piece is the detection, i.e. where the engine writes the race result.

`fk tickhook finish` stops a server past the finish and scans every writable
region for the validated time (as race ms and as sim ms), reporting each hit as
an offset from the typed objects a child can resolve — participant, vehicle,
playground, simulation, input array. Three tapes with different finish times
(22730 / 22884 / 23013) were the control: **no offset holds each tape's own
result in all three runs.** The result lives in per-run heap allocations
(`participant+0x11e48`, `+0x11ff8`, `+0x12058` on the three), so it is reached
by a pointer the participant holds, not by an offset from it. Finding that
pointer is the next step and it is worth **1.8× at late checkpoints**
(12.8 → ~7 ms per candidate); it needs the disassembly around the finish
event, which is why it is a measured lever here rather than a change.

### 9.3 Worker startup: 18.6 s → 10.4 s, and → 2.8 s behind a flag

Timestamping `fk watch measure`'s startup line by line:

| phase | before | now |
|---|---|---|
| server boot to READY | 2.2 s | 2.2 s |
| `calibrate_boundary` (51 forks + a 51-file batch validation) | **8.1 s** | 0 — off by default |
| blind locate of the vis state (37 windows, a fork each) | 3.6 s | 3.6 s (0.2 s with `FK_FAST_LOCATE=1`) |
| clock hunt inside the locate | 4.3 s (of the 3.6/blind path's own; see below) | 0.35 s |

**The calibration sweep is off the per-run path** (`--calibrate` restores it;
`fk server check` still runs it unconditionally as the acceptance test). It
existed because the probe used to be the only word on where the engine had
got to; `boundary_tick` now requires the probe to equal the engine's own next
record, two independent measurements from opposite sides, and across 20 runs
on 3 maps since the hook landed the sweep never once moved the boundary off
the probe. A control that cannot fail is decoration — 8 s of it per run.

**`find_clock` sampled 400 ticks of a 20 KB window and then let the child
simulate the rest of the tape in silence.** 96 ticks pin a strict
+10-every-tick slot just as well (the bias is measured against the engine now,
so the sample count only has to pick *which* slot), and `EXIT_ON_BUDGET` ends
the child when the measurement is over: 4.3 s → 0.35 s.

**`forkoracle::car::locate_fast`** replaces the blind sweep with two steps that
cannot pick a decoy: read the car's position out of the validator's own
`CGameVehiclePhy` (a typed pointer walk, six reads, no simulation), scan the
*paused parent* for every float triple within a tick of travel of it (one pass
over memory, no forks, 0.10 s), then test the candidates seven at a time in one
fork each over 24 ticks — a candidate must stay within a tick of the car,
travel what the car travels, and **lag it by one tick** (the vis state does;
the engine's mirrors of the physics state do not, and picking one would shift
every label by a tick). It locates in 0.15–0.5 s and the object it picks passes
`fk trace`'s control — median 3.5 mm against the ghost's own telemetry,
100 % of ticks within 5 cm, with the trace's quaternion and velocity
self-checks passing.

**It is behind `FK_FAST_LOCATE=1`, not the default, and the reason is a
measurement.** With it, `fk watch measure --tick 171 --n 8 --seed 1` goes
from *4 of 8 candidates tripping the crash predicate, 2 disagreeing with the
full validation* to *0 of 8 tripping, 6 disagreeing*, and the search ran 5.8×
faster with 0 % finishers. Position, attitude and velocity of the chosen
object are right — the 3 mm control says so — so something else the predicate
evaluator reads from `segments()` is not (the wetness word at +180, or the
counter it pairs with the state: `locate_fast`'s clock carries bias +2210
where the sweep's carries +1010, two different counters both correctly
biased for *labelling*, but the watchdog's own arithmetic may assume one of
them). That is a named, reproducible question — same seed, one env var —
and until it is answered the search keeps the locator its numbers were
measured with.

### 9.4 Fixed on the way

* `fk trace` had been dead on map 2 for months ("the chain is stale"):
  `DEFAULT_CHAIN`'s root `mod+0x1d56e48` holds the address of a STACK FRAME
  and is only walkable while that frame is live. `locate_v2` now resolves the
  car from the validator's own object and tries every chain against it, with
  a test that a pointer that merely *resolves* cannot pass: the first version
  accepted a chain 0.1034 m from the car whose object was a **frozen copy**
  parked at the checkpoint position (463 m median error against the
  reference). A candidate must now *track* the car over 24 ticks. The
  bounded sweep is the fallback, bounded by the validator's car rather than
  the map.
* **The clock bias was one tick out.** `fk trace` against the ghost's own
  telemetry read median **0.7966 m** — one tick of travel at 80 m/s — and
  **0.0031 m** with a −10 ms shift. The counter is the round time of the
  *finished* tick, and the vis state that `segments()` gathers lags that by one
  more tick; `measured_clock_bias` now labels the state the sampler actually
  reads (`counter − ((sim − race_start) − 20)`), and `fk tickhook reads`
  checks that instant. 3.1 / 3.5 mm at two checkpoints, 100 % within 5 cm.
* `fk`'s two suite failures and two doctest failures, all pre-existing at the
  branch point: the pool-spec test named `DEFAULT_CHAIN` after the default
  had stopped being a pool; `POINTER.md` quoted the legacy chain as the
  default; two indented blocks in `record.rs` docs compiled as Rust. **38/38.**

---

## 10. Exit at the finish

A candidate no longer waits for the validator to format its answer.

### 10.1 What the 5.8 ms was

§9.1 measured it and could not name it: 5.8 ms per candidate, constant, between
the child's last simulated tick and its first byte of output, and only 0.26 ms
of that is the 7 ticks the engine runs past the finish. The rest is the
validator's finish-and-print path — run to deliver one integer.

The printer is at `0x113b020` and reads a result struct whose fields fall
straight out of the disassembly, each beside its own name string:

| field | offset | how it is known |
|---|---|---|
| `IsValid` | +0x00 | `cmp DWORD PTR [rbx],0x1` / `sete` |
| `Time` | +0x08 | `mov r8d,[rbx+0x8]` beside `"Time"` |
| `Score` | +0x0c | `mov r8d,[rbx+0xc]` beside `"Score"` |
| `NbRespawns` | +0x10 | `mov r8d,[rbx+0x10]` |
| `NbCheckpoints` | +0x20 | `mov r8d,[rbx+0x20]` |

That struct is **no use**: the engine builds it at print time, out of a vector
it hangs at `+0x4870`, so reaching it means paying for the thing being skipped.

### 10.2 Finding the word the engine writes AT the finish

The value is in memory two ticks after the finish and long before anything is
printed — 41 copies of it — but at no fixed offset from anything: **two runs of
the same tape put it 0x50 apart**. So it lives in a per-run allocation, and the
only durable way to it is a pointer some typed object holds.

Forwards that is a 150 MB pointer graph. Backwards it is a short search, and
`fk tickhook finish --chain` does it: snapshot the writable memory, take the
words holding the finish time, find every word that points into their blocks,
repeat, and stop when a hop lands inside an object the shim can already
resolve. Sixteen candidates at depth 1; four survived three tapes with three
different finish times; the shortest is

```text
[[controller + 0x1a88] + 0xa4]     the finish time, in SIMULATION ms
```

It reads `0xffffffff` for the whole race and takes its value one tick after the
tick that detects the crossing. It is exact **including the sub-tick**:
rank00100 finishes at race 22884 ms — not a multiple of 10, so the engine
interpolates within the tick — and the word reads sim 25084, which is 22884 to
the millisecond. (This is also why "detect the finish tick and convert" is not
a solution: the tick is 22880.)

### 10.3 Why it is calibrated per server and not a constant

Hardcoding that offset is wrong, and every way it is wrong is SILENT:

| map | what a hardcoded rule met |
|---|---|
| map 2 | the word at +0xa4, sentinel `0xffffffff`, state beside it `2 -> 3` |
| 145875 | the word at +0xa4, sentinel `0xffffffff`, state beside it **`0 -> 1`** |
| 126859 | sentinel **`0`**, and the record **not in that block at all** |

Each mismatch made the lever quietly not fire — a search would have kept paying
the 5.8 ms while the code said it had been fixed. So the driver calibrates:
one fork of the tape whose finish time it already knows (the incumbent's own
millisecond, from the plain oracle), gathering the block as it changes — the
dedup key is the whole window, so a block that sits still costs nothing — and
it keeps the word that ends at exactly that time and started at one of the two
"nothing yet" markers. It tries the controller's block first, then the
participant, vehicle, playground and simulation.

Children are forked from that process, so **the address is the same in every
one of them** and no offset has to be portable. The shim is told the address
(`'Y'`), reads the sentinel itself, and every tick does one load and one
compare; when the word changes it writes the value to the shared timing page
and `_exit`s. The parent turns it into `FKFINISH race_ms <N>`, which
`parse_result` prefers over the JSON.

`on_clock` runs BEFORE that check, deliberately: leaving first would drop the
last tick from every finisher's watchdog summary — a behaviour change disguised
as a speedup.

### 10.4 The control, and the numbers

`fk tickhook finishcheck` arms `FKSHIM_FINISH_CHECK`, which makes a child
record the fast answer **and print the JSON anyway**, so both numbers come from
one simulation. It fails on any disagreement, and on any candidate where one
path answered and the other did not.

| map, checkpoint | candidates | finished | agree | disagree |
|---|---|---|---|---|
| map 2, `tick:171` | 400 | 93 | 93 | **0** |
| map 2, `tick:2313` | 200 | 200 | 200 | **0** |
| 126859, `tick:1200` | 250 | 57 | 57 | **0** |
| 145875, `tick:400` | 250 | 144 | 144 | **0** |
| **total** | **1100** | **494** | **494** | **0** |

A DNF produces no fast answer and keeps the JSON path — that is the honest
fallback, not a guess, and it is why the table's non-finishers are not a gap.

**Cost, measured** (`fk tickhook cost`, n=40, map 2 rank00001):

| checkpoint | before | after | |
|---|---|---|---|
| `tick:2313` (119 tail ticks) | 13.22 ms | **9.57 ms** | 1.38x |
| `tick:1200` (1232) | 43.19 | 40.78 | 1.06x |
| `tick:171` (2261) | 75.71 | 70.50 | 1.07x |

**Nothing else moved:** `fk watch measure` 50 candidates, same seed — 30 of 50
trips, 20 identical armed vs unarmed, 0 false positives, 50/50 score safety,
the same as with the lever off. `fk server check` **300/300 exact** across two
maps × three checkpoints. The guarded 10-minute stress search: 278 850 evals,
9 improvements, **0 phantoms**, best 22.711.

### 10.5 What is NOT done

* **A DNF still pays the 5.8 ms.** Skipping it needs the checkpoint count the
  driver reads from the `Desc` line, and no stable place for that has been
  found. In a search most candidates that reach the end are DNFs, so this is
  the larger half of the remaining win.
* **`FK_FAST_LOCATE` is still off** — §11.

---

## 11. FK_FAST_LOCATE: named, half fixed, still off

§9.3 reported a locator that answers in 0.2 s where the sweep takes 3.6 s, and
an unexplained difference: with it the watchdog stopped tripping (4 of 8 → 0 of
8) and a search ran 5.8x faster with 0 % finishers. That is now explained, and
the explanation is worth more than the speedup.

**The fast locator was picking a POSITION-ONLY RENDER COPY of the car.**
Identical x/y/z to the real state — which is why every position check passed,
including `fk trace`'s 3 mm control — but **velocity all zeros** and a
quaternion that is not a unit quaternion (|q|−1 = 0.14 against 1.3e-7 for the
real one). A speed-based predicate cannot fire on a car whose speed is
identically zero, so the watchdog went quiet, every candidate ran to the end,
and the search "sped up" by measuring nothing.

The fix is that the test is now the whole state, not the position: a candidate
must track the validator's car, travel what the car travels, carry a unit
quaternion at −16, and carry a velocity at +12 that is the derivative of its
own position. That rejects the copy on every map.

**It is still not the only locator, because a second copy exists.** With the
whole-state tests in place the candidate set still contains an object exactly
ONE TICK out of phase with the vis state. It passes every test above —
`fk trace`'s self-check accepts its quaternion and velocity — and produces a
trajectory 1.2147 m from the reference on 126859, which is one tick of travel
at 128.9 m/s, which the watchdog turns into **21 false positives out of 21
finishers**. The obvious discriminator does not work: the phase between the vis
state and the validator's own car is **not the same on every map** (the vis
state lags by a tick on map 2 and does not on 126859), so any rule of the form
"closer to where the car was" is right on one map and wrong on the next.

So the sweep stays the default and the blind sweep is NOT deleted. What is
known, for whoever finishes it:

* the engine's own pointer chains (`fk::ptr::CAR_CHAINS`, mirrored in
  `forkoracle::car::CAR_CHAINS`) end at the vis state by construction and
  resolve in 0.00 s — they are the right candidate source, and `fk trace`'s
  ladder already uses them to reach 2.9 mm on 126859 where the sweep cannot
  locate at all (`vel_err 1356 m/s: refusing to guess`);
* what is missing is the choice BETWEEN chains on a map where several resolve.
  The one test that is ground truth for it is the reference line the search
  already holds: the chosen object and bias must reproduce the reference
  position at the checkpoint tick. That is one comparison, and it is the next
  step;
* candidates must be vetted in the driver before they reach a child. Gathering
  several into one sample is faster and wrong: one bad address takes the child
  down and the failure reads as "nothing tracks the car".

### 10.6 The DNF half: found, measured, and NOT switched on

A DNF pays the same epilogue — **6.02 ms** after its last simulated tick,
against 5.84 for a finisher (`fk tickhook cost` with the tape steered off the
road) — and in a search DNFs are the majority: 307 of 400 candidates at an
early checkpoint. So this is the larger half of the lever, and both pieces it
needs now exist:

* **the engine says when the tape runs out.** `fk tickhook dnf` steers a
  candidate off the road and asks which words in the participant settle near
  the end: six do, all at the same instant, **one tick after the tape's last
  record** — `participant+0x188` goes `2 -> 0`, on a finisher and on a DNF
  alike. A child that reaches it with no finish recorded knows it did not
  finish;
* **the engine counts checkpoints.** `participant+0xc70` increments exactly at
  the ghosts' split ticks, the finish included — located behaviourally by the
  tm-player project's ENV arm and verified 200/200 against the plain oracle
  (and 1288/1289 over 2453 tapes). This arm's finish hunt had found the same
  word independently: it is the `2 -> 3` at the finish in §10.2.

It is implemented, and it is **off** (`FKSHIM_DNF_FAST=1`), because the two
numbers are not the same number:

| | JSON `Desc` | engine's counter |
|---|---|---|
| 45 of 79 DNFs | agrees | agrees |
| **34 of 79 DNFs** | `"wrong simu"` → **0** | **1** |

`parse_result` derives a DNF's checkpoint count from the `Desc` line, and for a
MUTATED candidate that line is almost always `wrong simu` — the declared result
in the ghost file no longer matches what was simulated — which the driver maps
to 0 checkpoints. The engine's counter says the car really did pass one. **Both
are right about different things**, and the engine's is the more truthful; but
switching to it changes how every non-finisher in the search is scored. That is
a decision about the search, not a speedup to slip in behind a 6 ms saving, so
it ships as a flag with this table attached.

### 10.7 What the finish lever does NOT cover

`fk server check` on **126859 refuses to calibrate** — no word in 32 KB of the
controller's block, the participant, any of the four vehicle slots, the
playground or the simulation ends at that race's finish time — and the server
keeps the JSON path. That is the designed fallback and it is verified, not
assumed: 100/100 exact on that map with the lever refusing. The map has already
shown it keeps this record somewhere else (§10.3), and the search window is
32 KB from each base; widening it is the obvious next step and costs one fork.

### 10.8 The four vehicle slots

Fixed on the way, from the tm-player INPUT arm's finding: the participant holds
**four** vehicle slots (`+0x1118/+0x1128/+0x1138/+0x1148` = Stadium, Snow,
Rally, Desert) and only one is being driven — the one whose `phy+0x10` is not
`0xffffffff`. `validator_chain` read slot 0 unconditionally, which is right on
an ordinary map and resolves to a **parked** vehicle on a transform map. It now
picks the live slot and refuses if the count is not exactly one.

(The same arm places the full vis state at `phy+0x848`. Tried: its position is
exact on 126859 and 145875 and one tick out on map 2, and the quaternion at
`pos-16` is not a unit quaternion there — so its internal layout is not the one
`segments()` describes. Left alone rather than guessed at.)
