# 197047 — `Welcome☺to wiggles` — the author time falls by 4.9 s, and it falls on two keys

**Headline: 95839 ms against an author time of 100784 that no human has ever
beaten — 4945 ms inside it, 5955 ms inside the human world record. The
pure-keyboard version is 96412 ms, 4372 ms inside the AT, and its entire
steering alphabet is two values: full left and full right.**

uid `VFK89hRxW1Wu2gicRvoe3uKCyeb` · author CatBagasm · AT **100784** · human WR
**101794** (Beagle.3) · 22 recorded runs, all 22 downloaded and analysed ·
`isValidated: true`, so the AT is a **driven validation lap**, not a plugin
number. **Nothing here has been or will be submitted to a Nadeo leaderboard.**

| tape | validated | vs AT 100784 | vs WR 101794 | steering alphabet in the wiggle | device |
|---|---|---|---|---|---|
| **`TAS_95839_analog`** | **95839** | **−4945** | −5955 | `{−110, −37, +37, +110}` | pad / action keys |
| **`TAS_kbd_marched`** | **96412** | **−4372** | −5382 | **`{−127, +127}` — two values** | **keyboard** |
| `TAS_analog_marched` | 96454 | −4330 | −5340 | 4 values | pad |
| `TAS_kbd25_marched` | 96719 | −4065 | −5075 | `{−127, 0, +127}` | keyboard |
| `TAS_kbd_metronome` | 96759 | −4025 | −5035 | `{−127, 0, +127}` | keyboard |
| `TAS_96852_v1` | 96852 | −3932 | −4942 | 4 values | pad |
| — human WR, Beagle.3 | 101794 | +1010 | — | 229 values | pad |
| — best human keyboard (rank 9) | 108114 | +7330 | +6320 | `{−127, 0, +127}` | keyboard |

Every one of those six re-validated exactly, three times, in a fresh directory
against a re-downloaded map, with the human world record carried as a
known-answer control returning 101794 every pass. **Zero phantoms this session.**

---

## 1. What this map actually is

Three blocks and 77 items on a `48x48Day` Stadium. The car spawns at
**y = 242 m** — a flat straight in the sky — and stays at exactly that altitude
for the whole race. x runs 1018 → 391 (627 m); z never leaves ±3 m.

There are only **two checkpoints**: an item gate 6 m from the spawn (everybody
crosses it around 770 ms) and a block gate 620 m away. The WR's splits are
`[766, 100215, 101794]`. So the map is one 99.4-second sector and a 1.58 s tail.

From race 1.95 s to 100.4 s the world record holds **gas and brake together**
and flips the steering full-left / full-right at about 2.4 Hz. Speed sits at
**22 km/h — 6.2 m/s — for a hundred seconds.**

**The whole map is the "wiggle": the TM2020 technique for moving a car that
cannot drive, by rocking it left and right with gas+brake held.** That is what
"Welcome to wiggles" and the Educational tag mean. 620 m of it, and nothing else.

Two consequences, and they reshape the entire problem:

* There is no trick hiding at a feature. The objective is the **mean forward
  speed of a periodic steering waveform** — a limit cycle. 1010 ms out of
  100784 is a 1.0 % speed improvement.
* Nothing in the input tape matters except **timing**. Measured: steering
  amplitudes of 70, 90, 110 and 127 (of 127) over the same rhythm produce times
  within **1 ms of each other** over 14 s of wiggling. The engine's steering
  rate limiter eats the difference. **A keyboard is not a handicap on this map;
  it is the whole alphabet you need.**

## 2. The tail is a respawn, and it is worth 1.5 s

After the far checkpoint the car is not driven to the finish. It **respawns** —
teleporting to the start line — and crosses the finish ~1.5 s later. The
validator confirms `"NbRespawns": 1` for every human run.

The respawn is not a steer/accel/brake field, which is why every search in this
project would have left it frozen. It is **bit 31 of the packet's 34-bit state
literal** (unpacking to word0 bit 5), present in exactly two packets per run;
the next "same as previous" packet clears it for free because `pack_prev` masks
word0 with `0xF`. Measured on the WR by moving that pair of packets:

| respawn moved by | finish |
|---|---|
| −7 ticks | 101724 |
| −5 | 101744 |
| −3 | 101764 |
| 0 (as recorded) | 101794 |
| +5 | 101846 |

**finish = (first respawn tick − 154)·10 + ~1504 ms**, exactly, provided CP2 is
already collected. Two respawn packets are required — one alone gives `DNF
cps=2`; the gap between them is irrelevant (1, 10, 25 or 50 ticks all work).

So the human tail decomposes cleanly: **tail = press latency + 1504 ms.** The
WR pressed 75 ms after CP2 and got a 1579 ms tail. Rank 5 pressed 22 ms after
and got 1526 — the best tail in the field — and 1579 − 1526 = 53 = 75 − 22, to
the millisecond. Our tapes press on the **first tick after CP2**.

That is worth ~75 ms against the WR for free, and it is the single easiest thing
in this write-up for a human to copy.

## 3. Correctness

* **§4 identity control: 22/22.** Every downloaded ghost re-simulates to its
  exact leaderboard millisecond, in 2.6 s of wall time for the whole field.
* **§8 field reproduction: 22/22**, spanning the whole leaderboard from the WR
  to the 125 s run. This map is healthy in our oracle.
* **Codec identity**: the WR rebuilt through the search's own encoder returns
  101794. The WR round-tripped through the new packet-level respawn editor with
  the respawn left where it was also returns 101794.
* **Cold re-validation**: all six headline tapes, three passes, fresh
  directory, fresh server process, against a **re-downloaded map** — `curl`
  from Nadeo's own `core.trackmania.nadeo.live/maps/<guid>/file`, sha256
  `6b0c0dce5ab9c4ef88d2e774d90cbc4c4e7fa3f4cfbeeed17b8d988bb178bc4b`,
  byte-identical to the working copy — with the human WR as the control
  returning 101794 in every batch. Every tape returned its filename time
  exactly, every pass.
* `NbRespawns: 1`, `IsValid: true`, correct `MapUid`. The run collects both
  checkpoints, wiggles the same 620 m corridor the field wiggles, and uses the
  same respawn-to-finish the whole field uses. Nothing is skipped.

## 4. Where the time is — the per-sector table

There are no intermediate checkpoints, so I built some: **`tmmaps gate --at
x,y,z`** (new subcommand) writes a map whose only finish is a gate at a chosen
world position with the far checkpoint neutralised. Twenty of them along the
corridor, and every one of the 22 human runs timed through every gate — 440
exact splits.

Sector times, whole field, mostly 40 m sectors (ms):

| sector | field min | field spread | corr with final |
|---|---|---|---|
| 1000→980 | 1154 | 960 | 0.16 |
| 980→960 | 3095 | 976 | 0.37 |
| 960→940 | 3154 | 832 | 0.32 |
| 940→920 | 3163 | 792 | 0.44 |
| 920→900 | 3192 | 650 | 0.65 |
| 900→860 | 6514 | 1071 | 0.54 |
| 860→820 | 6416 | 1347 | 0.69 |
| 820→780 | 6266 | 1439 | 0.71 |
| 780→740 | 6428 | 1007 | 0.74 |
| 740→700 | 6276 | 1437 | **0.80** |
| 700→660 | 6415 | 1461 | 0.70 |
| 660→620 | 6403 | 1223 | 0.77 |
| 620→580 | 6453 | 1038 | 0.65 |
| 580→540 | 6488 | 1101 | 0.67 |
| 540→500 | 6302 | 1188 | 0.66 |
| 500→470 | 4982 | 594 | 0.47 |
| 470→440 | 4712 | 1145 | 0.65 |
| 440→420 | 3211 | 806 | 0.59 |
| 420→400 | 3184 | 663 | 0.72 |

**Every sector correlates 0.44–0.80 with the final time.** That is the opposite
of 227969 and 270051, where one spectacular feature cost everybody the same and
a quiet stretch sorted the field. Here the map is homogeneous: the loss is
uniform, because the map is one continuous act with no features at all.

**The sum of per-sector minima is 95277 to reach x = 400, against the WR's
98483 — the field, assembled, is 3.2 s faster than its own best run**, and the
WR is fastest in only 5 of the 19 sectors. Ranks 2, 3, 4, 5, 7, 8, 9 and 11 each
own sectors. Assembling the field would already have beaten the author time by
roughly 2 s. Nobody on this leaderboard is doing anything special anywhere; they
are all doing the same thing at slightly different quality.

Our tapes on the same instrument, per 40 m sector in the steady state:

| tape | sector time | speed |
|---|---|---|
| human WR | 6403 – 6784 ms, wandering | 5.9 – 6.25 m/s |
| `TAS_kbd_metronome` | **6230 ms every sector, ±3 ms** | 6.42 m/s |
| `TAS_95839_analog` | **6030 ms every sector, ±3 ms** | **6.63 m/s** |

That flatness is itself the finding: a machine holding one rhythm produces a
dead-flat sector profile. The humans do not.

## 5. The technique — verdict: KNOWN BUT MISTIMED

Nobody needs to discover anything. All 22 runs are already doing the right
thing: gas+brake held, full-lock flips. What separates 95.8 s from 101.8 s is
**the rhythm and the metronome quality**, plus the respawn reflex.

Measured half-cycle (one flip to the next) in 10 ms ticks, over each human's own
wiggle:

| run | time | median half-cycle | sd | flips within ±1 tick of own median |
|---|---|---|---|---|
| rank 1 (WR) | 101794 | **21.0** | 2.08 | 53 % |
| rank 2 | 102934 | **22.0** | 2.58 | **71 %** |
| rank 3 | 103244 | 19.0 | 3.19 | 55 % |
| rank 5 | 105174 | 20.0 | 2.48 | 49 % |
| rank 9 (keyboard) | 108114 | 19.0 | 2.20 | 53 % |
| rank 14 (keyboard) | 110344 | 23.0 | 2.50 | 54 % |
| rank 19 | 113104 | 30.0 | 7.74 | 26 % |
| rank 22 | 125161 | 30.0 | 9.46 | 4 % |

And the measured speed of a *perfect* metronome at each rhythm over 120 m of
steady-state corridor (x 860 → 740) — the physics answer, with the entry
transient excluded:

| half-cycle | 120 m in | speed |
|---|---|---|
| 20 ticks (200 ms) | 19244 ms | 6.24 m/s |
| 21 ticks | 18954 | 6.33 |
| 22 ticks (220 ms) | 18692 | 6.42 |
| **25 ticks (250 ms)** | **18128** | **6.62** |
| 29 ticks | 22525 | 5.33 |

**The whole field wiggles too fast.** The world record's median half-cycle is
21 ticks; the fastest limit cycle is **25 ticks — hold each direction 250 ms,
two flips per second instead of the WR's 2.4.** The field's variability costs
more on top: the WR's own tape is slower than a *clean* 21-tick square wave,
which is in turn slower than a clean 25-tick one.

Three things, in order of size:

1. **Slow down to a 250 ms half-cycle** (2.0 Hz). Worth ~3 s over the map.
2. **Be metronomic.** Only 53 % of the WR's flips are within one tick of its own
   median. The tapes that beat the AT are dead constant. Worth ~1.5 s.
3. **Respawn on the frame you touch the far gate.** ~75 ms against the WR.

Things that are *not* the answer, each measured, each a real negative result:

* **Steering harder does nothing.** 70 / 90 / 110 / 127 land within 1 ms of each
  other. Do not buy a pad for this map.
* **Releasing gas or brake, even for one tick per half-cycle, destroys the
  wiggle.** All 325 pulsed-input candidates DNF; only constant-hold survives.
  The field is right to hold both.
* **Asymmetry does not help.** Every asymmetric hold (left ≠ right) that reached
  the end was slower than the matching symmetric one; nearly all died.
* **Starting the wiggle earlier than the WR does is worse.** Synthetic entries
  at every tick from race 3.5 s to 9 s were all slower to x = 940 than taking
  over at 7.46 s. The WR's transition into the wiggle is good; the best gain
  available in the launch is ~124 ms.

## 6. Why the field never found it, and why the run is so hard to hold

Because a 100-second wiggle is **chaotically unstable open-loop**. Replacing
50 ms of the WR's steering at t = 30 s loses the run completely. Of 481
synthetic waveforms swept against the far end of the corridor, **12 reached it**;
of 605 keyboard variants, **2**; and in a final 432-candidate sweep over hold,
ramp, amplitude *and* phase around the known-good rhythm, **exactly one
survived** — the same one. The car drifts sideways and eventually leaves the
corridor, and a fixed periodic input has no restoring force.

This is also why the honest reading of the human data is *not* "the field is
bad". A driver is **closed-loop**: they watch the drift and correct it, and the
±2-tick scatter in their half-cycle lengths *is* that correction. They are
paying for stability with rhythm. Our tape holds the rhythm because a search
found a set of half-cycle lengths that happens to keep the car straight — the
same job a human does with their eyes.

So the advice is not "be a metronome and never correct". It is: **correct as
little and as smoothly as you can, and let your default be 250 ms, not 210 ms.**
Rank 2 — the most metronomic driver in the field, 71 % of flips within one tick,
and the only one already at a 22-tick median — sits 1.1 s off the author time
with a rhythm three ticks short of optimal. That is this leaderboard in one row.

## 7. Tolerance — how tight is it really?

`wig tol`: every one of the 431 half-cycles of the keyboard metronome shifted by
−3, −2, −1, +1, +2, +3 ticks, each re-simulated through the plain oracle — 2587
runs. This is the **recoverable** form: the flip is mistimed and every later
flip keeps its spacing, which is what a driver actually does.

| flips surviving | count |
|---|---|
| all 6 shifts (±30 ms) | **227 of 431 (53 %)** |
| 5 of 6 | 67 |
| 4 of 6 | 58 |
| 3 of 6 | 42 |
| 2 of 6 | 23 |
| 1 of 6 | 12 |
| none | 2 |

And when a shift survives it costs between **−5 and 0 ms** — nothing. Several
mistimings are marginally *faster* than the nominal tape.

The sensitivity decays with how much track is left for an error to grow:

| race time of the flip | mean shifts surviving (of 6) |
|---|---|
| 9 – 18 s | 2.8 |
| 18 – 26 s | 3.7 |
| 27 – 35 s | 3.3 |
| 35 – 44 s | 4.2 |
| 44 – 53 s | 5.0 |
| 53 – 62 s | 5.4 |
| 62 s → finish | **6.0 — every shift survives** |

**The first 35 seconds is the hard part. After the first minute, nothing you can
do to the timing of a single flip loses the run.** For a human that is a
friendly shape: the section that punishes error is the one you practise most.

## 8. The low-input family

Steering **change events** (a value held N ticks is one event) and the alphabet,
measured inside the wiggle:

| tape | time | steer change events | alphabet in the wiggle | what a driver does |
|---|---|---|---|---|
| `TAS_kbd_marched` | **96412** | **~430** | **`{−127, +127}`** | one key swap every 220 ms, no neutral |
| `TAS_kbd_metronome` | 96759 | ~860 | `{−127, 0, +127}` | 210 ms hold, 10 ms neutral, swap |
| `TAS_kbd25_marched` | 96719 | ~860 | `{−127, 0, +127}` | 240 ms hold, 10 ms neutral |
| `TAS_95839_analog` | 95839 | ~860 | `{−110, −37, +37, +110}` | 230 ms hold + a 20 ms ramp |
| human WR | 101794 | 3533 | 229 values | a pad, corrected continuously |
| human rank 9 | 108114 | 616 | `{−127, 0, +127}` | a real keyboard run |

**The two-value tape is keyboard-authentic, and that was checked against the
humans rather than assumed.** Ranks 9 and 14 are pure `{−127, 0, +127}` runs,
and in their own tapes the flip goes **straight from −127 to +127 without
passing through 0 on 338 of 377 flips** (rank 9) and 271 of 298 (rank 14). A
keyboard player releasing one key and pressing the other inside the same 10 ms
tick is exactly what the two-value tape asks for, and it is what they already
do.

**Caveat, stated plainly:** in the keyboard tapes the 7.5 s launch (race
0 → 7.46 s, before the wiggle is established) and the post-respawn roll across
the line are inherited from the human world record's *pad* tape. The wiggle
itself — 92 of the 96 seconds — is pure keyboard. Ranks 9 and 14 drive the whole
map, launch included, with three values, so the launch is demonstrably
keyboard-drivable; I simply did not re-search it, because it is worth ~124 ms.
A march seeded from rank 9's own tape was attempted and failed to find any
surviving continuation in the time available.

## 9. The driving guide, off visual cues

The map has no features to cue off — that is the point of it — so the cues are
the countdown, the gate, and a count in your head.

1. **Start → the checkpoint gate (0 → ~0.8 s).** Full gas, no brake, straight
   down the platform. The gate is 6 m away; you are through it immediately.
2. **The run-up (0.8 → ~1.9 s).** Keep full gas. The car reaches ~100 km/h and
   then the surface takes it away — you feel it stop pulling at about x = 992,
   a car-length or so past the gate structure.
3. **Enter the wiggle (~1.95 s).** **Add the brake and keep the gas.** Both held
   for the next ninety-eight seconds. Never release either — one tick of release
   kills the run.
4. **The wiggle (2 s → ~94 s).** Alternate full left and full right. **Hold each
   side for a quarter of a second — 250 ms — not the fifth of a second the world
   record uses.** Two flips per second: a 120 bpm metronome with a flip on every
   beat and every off-beat. Steer strength is irrelevant — a keyboard's full
   lock and a stick at half deflection give the same speed to the millisecond,
   so use whichever you can time better.
5. **Hold the line.** The corridor is a few metres wide and the car creeps
   sideways. Correct with the *length* of a half-cycle, not with a partial
   steer: lengthen the side you want to come back from by 10–20 ms, then go
   straight back to the rhythm. The first 35 seconds is where a bad correction is
   expensive; after a minute the run is very hard to lose.
6. **The far gate (~94 s).** The instant you touch it — not after you have read
   the split — **press respawn.** You are teleported to the start line and the
   clock from there is fixed at ~1.5 s with nothing left to drive. Every
   millisecond you wait before pressing respawn is a millisecond on your time.

Realistic? The rhythm is the easiest part and half the flips take ±30 ms of
error for free. The demanding part is holding it for 92 seconds without a drift
that walks you off the edge — which is exactly what this leaderboard is already
struggling with, and why it has 22 entries rather than 900.

## 10. Findings that generalise

* **`tmmaps gate --at x,y,z` (new).** On a map with no useful checkpoints you can
  manufacture them. A ladder of relocated finish gates gives exact splits for any
  tape, human or synthetic, turns a 100 s evaluation into a 16 s one, and is what
  the whole per-sector investigation hangs off. One worker root per gate map —
  every gate map keeps the original mapUid.
* **A gate ladder neutralises the real checkpoint, so a tape optimised on it can
  be optimised into a line that never collects it.** The best marched tape here
  reached x = 400 in 92137 ms — 509 ms ahead of the tape that eventually won —
  and then `DNF cps=1` on the real map at every respawn tick, because it passed
  the far gate's plane outside the trigger volume. **Verify every marched
  candidate on the real map, or keep the target checkpoint required in the last
  gate.** Repairing it cost more than it had gained.
* **The respawn is an editable input, and it can silently pin your finish time.**
  It rides in the packet state literal, not the vehicle triple, so
  `ghost::Factory` cannot see it. On any map that ends with a respawn, the finish
  is a function of that tick and nothing else. **Watch for "every candidate
  returns exactly the same time"** — that is the signature of an ending the
  search cannot mutate, and it cost an hour here before a packet dump explained
  it. Worth 1.5 s of otherwise invisible time on this map.
* **On a steady-state map, measure the limit cycle between two far-apart gates,
  not from the start.** Timing to a single near gate ranked a 22-tick rhythm
  fastest; timing x 860 → 740 showed the 25-tick rhythm is 3 % faster and the
  22-tick one only looked better because it settles sooner. Total-time-to-a-gate
  conflates transient with steady state.
* **When every sector correlates with the result, stop looking for a trick.** A
  table where all 19 sectors correlate 0.4–0.8 is telling you the map has no
  features and the answer is a global parameter.
* **Chaos sets the method.** When a 50 ms edit anywhere loses the run, a global
  perturbation search cannot work; a sector-by-sector march can, and it must
  score each sector against the *next* gate rather than its own, or it optimises
  the tape into a state it cannot survive past (measured: the first march did
  exactly that and could not reach the following gate at all).
* **Test amplitude-invariance early on any input-limited map.** Four amplitudes
  agreeing to 1 ms collapses the keyboard-versus-pad question immediately and
  tells you the deliverable can be digital.
* **Ghost telemetry here is 50 ms, but inputs are 10 ms.** Read the rhythm from
  `tmtas trace`, not from the trajectory CSV, or you will mistake the game's
  own steering ramp for analog input.

## 11. Tooling added (Rust, in `tmtas-rs`)

* `tmmaps gate` — write one gate map with the finish at an arbitrary world
  position, optionally keeping named checkpoints required.
* `wig` (`tmsearch/src/bin/wig.rs`) — this map's tool:
  * `info` / `flags` — packet-level dump, including the respawn bit;
  * `retemplate` — move the respawn action to any tick;
  * `field` — per-human waveform table: period, hold, transition, metronome
    quality (sd and share of flips within ±1 tick);
  * `sectors` — per-sector table, per-sector minima and correlations from a
    gate-ladder run;
  * `gen` — synthesise parametric wiggle waveforms and validate them in parallel
    (hold, ramp, amplitude, phase, entry tick, brake/gas pulsing, respawn tick);
  * `march` — sector-by-sector search over half-cycle lengths, with a lookahead
    gate and a forward repair window;
  * `finish` — bisect the respawn tick for a plan and write the final tape;
  * `tol` — per-flip recoverable timing tolerance against the plain oracle;
  * `evalplans` — validate a directory of plans against any map.

## 12. Files

In `~/persistent/private-30d/tm-unbeaten/197047/`:

| file | what |
|---|---|
| `tapes/TAS_95839_analog.Ghost.Gbx` | the fastest validated tape |
| `tapes/TAS_kbd_marched.Ghost.Gbx` | **the drivable one** — two keys, 96412 |
| `tapes/TAS_kbd_metronome.Ghost.Gbx` | the pure 22-tick metronome, 96759 |
| `tapes/TAS_kbd25_marched.Ghost.Gbx`, `tapes/TAS_analog_marched.Ghost.Gbx`, `tapes/TAS_96852_v1.Ghost.Gbx` | the rest of the family |
| `tapes/*.tick.txt` | complete input scripts |
| `tapes/CONTROL_humanWR_101794.Ghost.Gbx` | the control carried in every batch |
| `plan_*.txt` | half-cycle-length plans for the three headline tapes |
| `map197047_v1.Map.Gbx` | the map (sha256 `6b0c0dce5ab9…`, = Nadeo's own copy) |
| `ghosts/` | all 22 human runs |
| `fieldgates_v1.txt` / `ourgates_v1.txt` | 440 exact gate splits for the field; ours |
| `tolerance_kbd_metronome_v1.txt` | the full 2587-run tolerance table |
| `PLAN.md` | the plan, argued from the acquisition evidence |
| `wig_v3.rs`, `tmmaps_main_v1.rs`, `wig_bin_v1` | the tool, source and binary |
