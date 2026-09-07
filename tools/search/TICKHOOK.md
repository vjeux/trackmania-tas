# TICKHOOK.md — the fork server's clock is the engine's own tick

The fork server used to count `lroundf` calls. It now hooks the function the
validator calls once per simulated tick, and every checkpoint, stride, budget
and deadline is in **ticks of race time**.

```
FK_CLOCK=tick      (default)  the tick hook; the shim installs it or dies
FK_CLOCK=lroundf              the old clock, kept so the two can be measured
                              against each other
```

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

### The record read, and why every resume needed `probe + 1`

The page-fault probe reports `(fault_addr − base) / 32`, where `base` is where
the shim found the tape's **steer** values. Steer sits 4 bytes into the
engine's 32-byte record, so a fault on the first byte of record `i` reports
`i − 1`. **The probe names the last record the engine finished with; `probe+1`
is the first unconsumed one.** That `+1` had been carried for a year as "tick p
is already partly consumed" — it is not partial consumption, it is an offset,
and now it is checked rather than believed (§4).

The engine also never touches the tape before race time −10 ms
(`0x119f0fc..0x119f11a`), which is why rewriting the countdown region is
physically inert (a fact the phantom investigation found empirically).

---

## 3. The hook

`FKSHIM_CLOCK=tick` makes the shim, before `main`:

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

Any failure is `_exit(92)`. **There is no fallback to the lroundf clock**: a
driver that asked for the tick clock must never silently get the other one.

`tick_entry` is where the old `lroundf` interposer's body now lives: the
sampler/watchdog dispatch, the SIGSTOP stop point and the fork-server
checkpoint, keyed on the tick. `lroundf` is still interposed — it only counts,
so `FKSHIM lroundf_total` stays comparable — and under `FK_CLOCK=lroundf`
nothing about the old path changes.

### The clock is RACE time, not simulation time

The simulation starts at ~1000 ms and the race — input record 0 of a tape with
`start_offset_ms = 0` — starts later. **That start is not a constant.** It is
usually 2200 ms, and in 1–3 of 300 servers started at once it was 2300: the
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
and `tmsearch::forkeval::clock_for_tick` forward to it. Under
`FK_CLOCK=lroundf` the same functions return the old fitted line
`36141 + 25.483·race_ms`.

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
page-fault probe and requires `probe + 1` to equal the tape tick that the
reported `sim_ms`/`race_start` name. The two are measured from opposite sides —
one is where the engine says it is, the other is which record it faults on — so
agreement is a real check and a disagreement is a hard error, never a number to
choose between. `tmsearch` and `fk` both go through it.

**Negative controls, run:**

| control | result |
|---|---|
| hook the physics step (`0x119f1b0`) instead | 2401 `dt != 10`, 2401 clock mismatches, race start never found — **rejected** |
| a host with no server text (`/bin/true`, `/usr/bin/time`) | exit 92, "signature offsets are not inside the main module's text" |
| `FKSHIM_TICK_FN_OFF` without `FKSHIM_TICK_UNSAFE=1` | exit 92, refused |
| an override target whose prologue is not relocatable | exit 92, refused |
| `FKSHIM_CLOCK=bogus` | exit 92 |
| `probe + 1` ≠ the engine's tick | hard error naming both numbers |

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

**Determinism under load** (`fk tickhook load`, map 2 rank00001, `tick:171`):

| clock | servers | distinct stops | probe |
|---|---|---|---|
| tick | 150 | 1 (`clock 1013`, probe 170) | 150/150 agree |
| tick | 300 (×2 runs) | 1 (`clock 1013`, probe 170) | 300/300 agree, both runs — including the 1–3 servers whose race start was 2300 |
| lroundf | 300 | **10** (probe 277…287) | n/a |

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

**700 / 700 identical, 0 mismatches**, oracle self-repeatability 0 disagreements
throughout, identity resume exact everywhere. In every one of the 14 runs the
calibration sweep left the boundary at the requested tick: `boundary tick N
(probe N−1)`.

**Cost.** 15 full validations of map 2 rank00001, median wall time: no shim
**2.350 s**, lroundf shim **2.376 s**, tick shim **2.360 s**. Per candidate
(`fk server bench`, n=60): tick 68.5 / 42.3 / 13.1 ms at ticks 171 / 1200 /
2313, lroundf 68.7 / 40.9 / 12.6 ms at the same *requested* ticks — the lroundf
runs are a hair cheaper only because their stop lands later (probe 2327 vs
2313), i.e. they simulate fewer ticks. The hook itself is free.

**The watchdog and the per-tick sampler.** `fk watch measure`, 60 candidates,
same seed, both clocks resumed at the same boundary (278): identical verdicts —
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
once under each clock:

| clock | evals | eval/s | banked | phantoms | best |
|---|---|---|---|---|---|
| tick | 272 910 | 454 | 9 | **0** | 22.711 |
| lroundf | 236 130 | 391 | 9 | **0** | 22.711 |

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
fk tickhook check                     do the constants match this binary? (static, reads the ELF)
fk tickhook count  --tape G --map M [--gdb]     hooked vs plain run, all the criteria
fk tickhook load   --tape G --map M --at T --n N   N servers at once
fk tickhook find   --tape G --map M [--back N --ahead N]   a new build
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
  control on the hook rather than as the mechanism. It has now been demoted,
  not deleted, and that is deliberate: two independent measurements that must
  agree are worth more than either alone.
