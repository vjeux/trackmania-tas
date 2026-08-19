# 210218 — "Fall 2024 - 25 (Pure Wet Icy Wood)"

AT **94477** · human WR **96281** (iambeeen) · **30** records · gap **1804 ms**
· uid `VHalgyxHqys7loscd1RQhgqp3Fe` · tags Water, Altered Nadeo, Wood
· `atSetByPlugin: true`

*Final. Best validated time 96077 ms; the author time did not fall. A second
agent (w218) continued the map afterwards and reached 96065.*

---

## 0. ACQUISITION.md §8 — field reproduction: **PASS (29/30, zero divergences)**

All 30 leaderboard ghosts (96281 … 440289) re-simulated against the map file
fetched from **Nadeo's own** endpoint
`core.trackmania.nadeo.live/maps/95006d75-3d00-428b-b3be-96190a4e5816/file`
(10 025 757 bytes):

**29 of 30 return their recorded millisecond exactly. Zero runs return a
different finish time** — which is the 203072 failure mode, and it is absent.

The exception, `r23_179463`, does not finish in the oracle. It was
re-downloaded and is **byte-identical** (sha `63f73f12…`), so it is not the
truncated-download trap; its ghost is structurally normal (one player record,
17 checkpoints, 3590 samples).

**Explained, by the second agent on this map (w218), after this section was
first written and credited here:** the server prints, on *stderr*,
`Invalid replay: r23_179463.Ghost.Gbx (#26.48x48Screen155Sunset.Script)` — the
ghost references an embedded custom-item script the validator cannot resolve, so
it **never simulated at all**. `tmtas validate` reports a load failure and a
crash identically, as `DNF`. So field reproduction on this map is **29/29 of the
ghosts that load, and the one that does not load is a loader failure, not a
physics divergence.** §8 is a clean pass.

Second control: the WR tape rebuilt through the search's own encoder
re-validates at **96281**.

## 0b. ACQUISITION.md §9 — the author's embedded ghost: **NOT PRESENT**

The map header says `validated="1"`, but there is no validation ghost in the
file. Decompressing the body (11 916 655 bytes) and searching it gives **zero**
hits for `0x0911F000` (CPlugEntRecordData), `0x0303F000/6` (CGameGhost), and
every CGameCtnGhost chunk id; the only ghost-shaped hits are a monotonically
increasing index table (`0x03092000, 0x04092000, 0x05092000, …`). `tmtraj
decode map.Map.Gbx` reports the chunk missing.

**Positive control, same binary and command, on the 228607 map**: `0x0911F000`
found at body offset 607 759, and `tmtraj` decodes a 20 290 ms run from it. The
tool works; this map really is empty.

### Consequence: what this does and does not say about 94477

Three signals point one way:

1. no author ghost in the map file, though the map is marked validated;
2. `atSetByPlugin: true` on unbeaten.at;
3. **the map's own author, R4igekon, sits 4th on the leaderboard at 105172 —
   10 695 ms slower than the author time attributed to them.**

**I first read this as "94477 was probably never driven". A third agent's
sibling-map census, landed at 01:19 while this document was being finalised
(`key_RESULT_v1_the_author_beats_his_own_ATs.md`), makes that reading wrong and
I am withdrawing it.** R4igekon's author times on **nine of nine** of his other
Pure-Wet-Icy-Wood conversions are beaten by humans, by margins of 36 ms to
19.5 s — and on two of them **the human who beats the author time is the author
himself** (208961: 25377 → 23908; 208800: 47167 → 46566). Those runs re-simulate
exactly. So this author drives his own surfaces to within ~2 % of what a good
human does, and a 1804 ms gap here is an **ordinary** margin for the series, not
a freak.

The corrected statement: **the absence of an embedded ghost means we cannot read
the author's line off this map, not that no such line exists.** Being 4th on the
online board says how much the author replayed *this* conversion, not whether he
could drive its author time. 94477 should be treated as a normal target.

That also means the "how would a human do this" work below is on firmer ground
than I thought when I wrote it: the same agent banked **five validated human
runs on this exact surface**, two by the author, all beating their own maps'
author times, in `210218/key_siblings/`. They are a reference, not a seed — the
census shows every one of them grafted onto 210218 binds and dies at `cps=1`,
because each sibling is a different campaign layout wearing the same surface.

---

## 1. Where the time is: the whole leaderboard is a splice away from the AT

17 checkpoints. Per-sector minima across the 30-run field:

| s | best ms | by | WR ms | WR's rank in the sector |
|---|---|---|---|---|
| 1 | 8752 | r01 | 8752 | 1 |
| 2 | 5266 | r01 | 5266 | 1 |
| 3 | 7582 | r01 | 7582 | 1 |
| 4 | 4193 | r01 | 4193 | 1 |
| 5 | 6805 | r01 | 6805 | 1 |
| 6 | **6397** | **r21** | 6854 | 3 |
| 7 | **2626** | **r21** | 2859 | 2 |
| 8 | **2150** | **r19** | 2573 | **6** |
| 9 | 3266 | r01 | 3266 | 1 |
| 10 | **7416** | **r21** | 7575 | 2 |
| 11 | **11013** | **r21** | 11894 | 3 |
| 12 | **3768** | **r21** | 4342 | 3 |
| 13 | **5189** | **r21** | 5904 | **7** |
| 14 | **5094** | **r21** | 5324 | 2 |
| 15 | **4965** | **r21** | 4965* | 2 |
| 16 | **4667** | **r21** | 4936 | 2 |
| 17 | 2677 | r01 | 2677 | 1 |

**Sum of per-sector minima = 91 826 ms.** That is **2651 ms under the author
time** and **4455 ms under the world record**, and every one of those sector
times has already been driven by a human on this map.

The world record is the fastest run in only **6 of 17 sectors**. In sector 13
six humans are faster than it; in sector 8, five are.

**This is the headline finding and it stands on its own: the author time on this
map is reachable by assembling what the field already does.** No new technique
is required to be 2.6 s under it — only the combination.

### Is the dream lap real, or just inherited entry speed?

The obvious objection to any sum-of-sector-minima is that a sector is easy to
win if you arrive at it faster, so the minima chain together illegitimately.
Checked directly — speed at the entry gate of each sector, best-in-sector run
versus the world record:

| s | best by | gain vs WR | entry speed, best | entry speed, WR | Δ |
|---|---|---|---|---|---|
| 6 | r21 | **457** | 221 km/h | 226 | **−5** |
| 7 | r21 | 233 | 342 | 308 | +34 |
| 8 | r19 | **423** | 334 | 354 | **−20** |
| 10 | r21 | **159** | 200 | 200 | **0** |
| 11 | r21 | **881** | 198 | 206 | **−8** |
| 12 | r21 | 574 | 177 | 143 | +34 |
| 13 | r21 | 715 | 245 | 234 | +11 |
| 14 | r21 | 230 | 237 | 230 | +7 |
| 15 | r21 | **514** | 132 | 136 | **−4** |
| 16 | r21 | 269 | 201 | 182 | +19 |

**The five largest gains are all taken from an entry speed equal to or slower
than the world record's** — sector 11's 881 ms on 8 km/h less, sector 8's 423 ms
on 20 km/h less, sector 6's 457 ms on 5 km/h less. Those five alone are
**2434 ms**, so even discarding every sector that might have inherited speed
from the one before it, the assembled lap is **93 847 ms — 630 ms under the
author time**. The two sectors with a large positive entry delta (7 and 12) are
themselves downstream of sectors won on a slower entry, so the chaining is
legitimate rather than circular.

The dream lap is not an artefact.

## 2. The outlier: one run changes technique mid-lap and beats the WR by 3195 ms

`r21_174673` (SparkSheep, 21st on the leaderboard) is the fastest run in the
field in **8 of the last 12 sectors**. Summed over sectors 6–17 it is **3195 ms
faster than the world record**. It sits 21st only because it lost **81 587 ms**
in sectors 1–5 — 41 050 ms in sector 4 and 34 681 ms in sector 5, where the
telemetry shows the car crawling at 5–90 km/h around x ≈ 320–360, y = 128 for
twenty seconds. (Checked: **not** a respawn loop — there are no position jumps.
It is a driver stuck on a plateau.)

Same route: r21's checkpoints are within a metre or two of the WR's at every
one of the 16 gates.

### What it does differently

| | sectors 1–5 | sectors 6–17 |
|---|---|---|
| r21 mean slip angle | 12.3 – 22.3° (normal for the field) | **0.3 – 2.1°** |
| r21 vs WR | 81 587 ms **slower** | **3195 ms faster** |

The driver drives the first third like everyone else, and then stops sliding —
and that is exactly the stretch where they beat the world record, at **higher**
average speed and on a **shorter** path (sector 6: 476.6 m vs 503.9; s10 408.6
vs 431.2; s11 478.7 vs 486.8; s13 342.6 vs 350.6; s16 309.9 vs 313.9).

Sector 6 side by side — same gates, same entry speed (221 vs 226 km/h):

| | WR (r01) | r21 |
|---|---|---|
| sector time | 6854 ms | **6397 ms** |
| lateral velocity, sampled | +8.9, **−50.6**, −30.6, −30.7, **−40.4**, −2.7, **+35.4** m/s | **−0.15, +0.24, −0.18, −0.43, +0.06, −0.28, +0.40** m/s |
| steering, sampled | −1.000, +0.820, +1.000, −0.969, +1.000, −0.004, −0.537 | −0.004, −1.000, −0.004, −0.608, −0.608, −0.004, −0.004 |
| distinct steer values, whole run | **213** | **10** |

The world record thrashes between full left and full right lock with the car up
to 50 m/s sideways. r21 holds the wheel essentially centred and the car within
half a metre per second of straight.

**The one-sentence finding: the entire top of this leaderboard drives Pure Wet
Icy Wood permanently sideways, and the one person who stops doing that is faster
everywhere they stop.**

### The honest statistics, including what does NOT support it

* Mean slip **angle** over the whole run: WR **23.0°**, the other 28 runs
  **18.3 – 24.3°**, r21 **10.8°**. r21 is the only outlier in the field.
* **There is no field-level correlation between slip angle and sector time, in
  either direction.** Per sector, |r| < 0.30 in 12 of 17, and the five that pass
  disagree on sign (s13 +0.49, s17 +0.40, s4 −0.54, s2 −0.46). The reason is
  that the field is stylistically homogeneous — 29 of 30 runs are within six
  degrees of each other — so there is nothing to correlate against. **w218
  measured what does order this leaderboard: respawn count, Spearman +0.874.
  Slip orders it at +0.099.**
* **A correction against myself, recorded because the trap is general.** My
  first pass used lateral velocity in **m/s** and found r = −0.90 … −0.50 in the
  early sectors, apparently saying *more* slip is *faster*. That was pure speed
  confound: a fast car at a small slip angle carries more lateral m/s than a
  slow car sideways. **Slip angle = atan(|side| / forward) is the right
  variable**; on it the effect disappears.

So this is a **single-outlier contrast, n = 1**, not a correlational result. Its
strength is the internal control: the same driver, in one run, drives one way
for five sectors and another way for twelve, and the result switches with the
technique.

### Two controls added by w218 that close the obvious confounds

Both independently measured on their own build, after this section was written:

* **Sectors 12 and 15 are a controlled experiment, not an anecdote.** Both runs
  at **identical 100 % throttle**, on the same line to within 8 m over
  220–240 m, and in s12 **r21's path is 4.6 m LONGER** — yet r21 carries **17 %
  and 9 % more mean speed, at 3° of slip against the WR's 10–16°.** Throttle and
  route are both held constant; only slip differs. (This also corrects my
  "3–5 % shorter line" above: r21's path is shorter in s6, s10, s11, s13 and
  s16, and *longer* in s12 — so the shorter line is a consequence of the
  technique in most places, not its mechanism.)
* **The input-device confound is closed.** **15 of the 30 runs are 3-value
  keyboard tapes**, and all fourteen non-r21 runs among them sit at
  **23.7 – 30.4°** of slip. r21 alone is at 0.3 – 3.1°. Low slip is the driver,
  not the controller.
* **And my "7 distinct steer values" was telemetry, not inputs.** r21's actual
  input tape holds **three**: `{−127, 0, +127}`. The intermediate values in the
  telemetry are the game's own steering filter. **The fastest back half on this
  map is driven on a keyboard, with three values.** The claim in §6.2 is
  stronger than I wrote it.
* Also worth knowing before anyone chases it: **respawning is arithmetically
  excluded** as a route to the AT — w218 swept the respawn tick over the last
  2.7 s (28 points) and the cheapest finishing respawn is 97865 against 96078, a
  floor of **+1787 ms** against a total gap of 1601.

## 3. Map-shape fact: on this surface, input tapes are not transferable at all

Every attempt to move a good stretch of one tape onto another **DNFs**:

* r21's whole back half onto the WR's front half, spliced at **8 different
  checkpoints** (CP5, CP8, CP10, CP12, CP15, CP16 …): DNF, every one, failing at
  or just after the splice.
* The same, spliced at the **8 best state-matched tick pairs** found by
  minimising a joint distance over position, speed, yaw, roll and slip: DNF,
  every one.
* **Single-sector** transplants of r21 into the WR, one sector at a time for ten
  sectors: DNF, each failing exactly at the transplanted sector.
* The WR's own steering, in one sector, put through a **5-tick moving average**:
  DNF.
* The WR's own steering, in one sector, **scaled by 90 %** (a 10 % reduction):
  DNF. Also 80, 70, 60, 50, 40, 30, 20, 10 and 0 %.

**Local search from a finishing incumbent is the only usable operator on this
map.** That is a property of the surface — pure wet icy wood is chaotically
sensitive — and it is worth knowing before anyone spends hours on splice
machinery here.

It is also why the low-slip regime is hard to *reach* from the WR's tape even
though we can see it exists: damping the WR's steering asks the car to be
somewhere it never was.

## 4. Two defects in the toolchain, found here because this is the first
## many-checkpoint map

Both in the hardened build's scoring, both fixed on this node
(`FINISH_BASE 1e8 → 1e12` in `tmsearch/src/main.rs`, `forksearch.rs`,
`bin/tmtas.rs`; `SEG_UNIT` unchanged):

1. **A deep DNF outscores a real finish from 11 checkpoints on.**
   `score_dnf = cps·SEG_UNIT − cp_time` with `SEG_UNIT = 1e7`, against
   `score_finish = FINISH_BASE − t` with `FINISH_BASE = 1e8`. A DNF at CP11
   scores 1.05e8 and beats a finishing 96281 (9.99e7). On any map with 11+
   checkpoints the search will abandon finishing tapes for deep DNFs **and it
   looks exactly like progress**.
2. **The phantom guard misfires from 6 checkpoints on.** Its test is
   `score > FINISH_BASE/2`, which a CP6 DNF already passes, so it computes a
   negative `want`, declares a PHANTOM and aborts. This killed three of four
   search arms here within 45 seconds of launch (scores −15 000 000,
   −25 000 000, −45 000 000 = DNFs at CP12, CP13, CP15).

**These were FALSE phantoms — nothing unreal was ever banked** — but they abort
the run. Specimens preserved in
`tm-loop/phantoms/m210218-shaped-incumbent-20260818-1908/`.

Also recorded: **`tmmaps` cannot parse this map** ("unhandled inline node class
0x40000000 at 1422140"), so per-checkpoint segment maps — the obvious way to get
15× search throughput on a 94-second map — are not available here without a
parser fix. The map is 10 MB with 197 embedded zlib streams (custom items).

## 5. The search

Full-map plain-oracle evaluation costs ~4.5 s per candidate per worker; with 170
workers the search runs at **~245 candidates/s**.

Structure: a **forward sector ratchet**. Each stage searches one checkpoint-to-
checkpoint window with every core, seeded from the running best tape; the
stage's winner is **re-validated through the plain oracle** before it is adopted
as the next stage's seed, and a mismatch stops the ratchet and preserves the
tape. Repeated passes over sectors 6–17.

*(current status and best time: see §7)*

## 6. The human deliverable: how to drive this map faster than the world record

Two things here are worth a driver's time, and neither needs a TAS.

### 6.1 The dream lap is 91 826 and every sector of it has been driven

A player chasing the author time does not need a new trick. They need sectors
6, 7, 10, 11, 12, 13, 14, 15 and 16 the way **SparkSheep** drives them in the
21st-place run (174673), sector 8 the way **Sompig.** drives it (157145), and
the rest from **iambeeen**'s world record. Even discarding every sector that
could have inherited entry speed, that is 630 ms under the author time.

### 6.2 Stop drifting — and it is EASIER, not harder

This is the part that should change how the map is driven. On a surface the
community treats as drift-only, the fastest existing sector times come from a
run that holds the car straight. And the technique that goes faster is
**strictly simpler to execute**:

| sector | r21 input events / distinct steer values | WR input events / distinct steer values | r21's gain |
|---|---|---|---|
| 6 | **50 / 5** | 73 / 50 | 457 ms |
| 7 | **16 / 3** | 39 / 31 | 233 ms |
| 8 | **10 / 3** | 27 / 24 | (r19 owns this one) |
| 9 | 28 / 2 | 48 / 37 | — |
| 10 | **37 / 5** | 78 / 55 | 159 ms |
| 11 | **47 / 5** | 122 / 82 | 881 ms |
| 12 | **10 / 3** | 39 / 32 | 574 ms |
| 13 | **28 / 3** | 70 / 51 | 715 ms |
| 14 | **16 / 3** | 46 / 33 | 230 ms |
| 15 | **22 / 3** | 59 / 42 | 514 ms |
| 16 | **36 / 3** | 49 / 34 | 269 ms |
| 17 | **4 / 3** | 16 / 14 | — |

Over sectors 6–17 r21 uses **seven** distinct steering values in the telemetry —
`{−127, −76, −50, 0, +50, +76, +127}` — and is at **0 (wheel centred) for 66 %
of the samples**. **In its actual input tape it holds three: `{−127, 0, +127}`
(w218) — r21 is a keyboard run**, and the intermediate telemetry values are the
game's own steering filter. The world record uses **213** distinct values over
the run and sits at full lock, one way or the other, for a third of it.

**So the low-input family for this map was not produced by a quantiser — a
human already drove it, on a keyboard.** Three values, two to three times fewer
input changes per sector than the world record, and faster in eight of the last
twelve sectors. Anyone who has been avoiding this map because "you have to catch
the slides" has it backwards.

*(One caveat on robustness, measured by w218: the keyboard tape is not more
forgiving of mistiming than the analog one — it dies on 55 % of single-unit
steer perturbations against the incumbent's 69 %, the same order. Fewer inputs
here means less to execute, not more slack per input.)*

### 6.3 Sector-by-sector, off visual cues

Coordinates are the game's own; the run descends x 1488 → 660 overall. "Wheel
centred" below means literally zero steering input, which is what r21 holds most
of the time.

| sector | where you are | what r21 does | gain |
|---|---|---|---|
| **6** | leaving the CP5 gate at ~220 km/h, long descent to z ≈ 500 | **five steering values, wheel centred between them, throttle pinned.** Do not catch the car — do not let it start sliding. Exit at 341 km/h against the WR's 308. | **457 ms** |
| **7** | flat-out run to z ≈ 338, the fastest ground stretch (401 km/h) | three values, sixteen changes, full throttle throughout | 233 ms |
| **8** | braking zone into z ≈ 238 | **throttle for only 41 % of the sector** against the field's 57 % — lift earlier and longer, steer with three values | 423 ms (r19) |
| **10** | climbing back out, x 785 → 1005, first air (18 %) | throttle 82 % — hold it *more* than the WR does here (48 %) | 159 ms |
| **11** | the long one: x 1005 → 1265 climbing to y 122, **40 % airborne** | the single biggest gain on the map. Five values, 47 changes, throttle 94 %. The WR spends 122 changes and 82 values fighting this and loses 881 ms | **881 ms** |
| **12** | over the top, dropping y 122 → 82, **43 % airborne** | **ten input changes for the whole sector.** Three values, full throttle, a touch of brake | 574 ms |
| **13** | landing and running to x 1352, y 58 | 28 changes, three values, full throttle | 715 ms |
| **14** | along z 755 → 971 at y ≈ 50 | sixteen changes, three values | 230 ms |
| **15** | the slow hairpin back, x 1400 → 1244, down to 132 km/h at entry | 22 changes, three values, full throttle | 514 ms |
| **16** | back along z ≈ 960 to x 985, up to 290 km/h | 36 changes, three values, **full throttle where the WR is on it only 84 %** | 269 ms |
| **17** | the finish: 58 % airborne, 506 km/h peak | **four input changes**. Nothing to do but hold it | (WR owns this) |

The general instruction, and it is the same everywhere: **arrive with the car
pointed where it is going, hold the wheel still, and do not chase it.** The
lateral-velocity trace is the thing to watch — the world record swings ±50 m/s
sideways in sector 6, r21 stays inside ±0.43 m/s.

### 6.4 What we could NOT do

**We could not get a TAS into that regime.** Every route from the world record's
tape to a lower-slip one DNFs (§3), and local search from the world record
recovers only ~200 ms, all of it in the last three sectors. So the low-slip lap
is evidenced by a human run, sector by sector, and not by a tape we produced.
That is the honest state of it, and the open question this map leaves is a
search question: **how do you get an optimiser from one driving regime into
another on a surface where no tape survives a perturbation?**

## 7. The search result

**Best plain-oracle-validated time: 96077 ms.** That is **204 ms under the human
world record (96281)** and **1600 ms OVER the author time (94477). The author
time did not fall.**

Every step of the ratchet was re-validated through the plain oracle before being
adopted; there was no mismatch at any stage, and the two phantom reports at
19:08 were the false positives of §4, not banked results.

Where the 204 ms came from, and where it did not:

| region searched | worker-minutes | gain |
|---|---|---|
| sectors 1, 2, 3, 4, 5 (7 min each, 170 workers) | ~5 950 | **0** |
| sectors 6, 7, 8, 9 (5 min each, 170 workers) | ~3 400 | **0** |
| sector 11, sectors 12–13 (annealed, temp 60) | ~2 380 | **0** |
| sector 14, 15, 16 | ~3 570 | **0** |
| **ticks 8278–9782 (sectors 15–17) and sub-windows** | ~15 000 | **−204** |

**Eleven of the seventeen sectors were searched with every core on the box and
gave back nothing at all.** The world record's tape is a deep local optimum
everywhere except the last fifteen seconds, and even there the gains decayed
geometrically: −103, −67, −16, −1, −7, −4, −5, −1 and then nothing.

This is the same wall as §3 from the other side. The 2651 ms that the field
demonstrably owns is not reachable by perturbing a tape that is in the wrong
driving regime, and the search cannot cross between regimes because no
perturbation of a tape on this surface survives.

### The regime-crossing experiment

The last hours were spent on the open question rather than on more of the same.
Method: take the best tape, **centre the steering across the whole of sector 11**
(the 881 ms sector) so the incumbent starts inside the low-slip basin rather
than being damped toward it, and let the shaped-DNF search rebuild the sector
forward from there. The incumbent begins as a DNF at CP8 and the score ladder
rewards depth, so the search has a gradient to climb even before it finishes.

*(outcome)* **Negative, and cleanly so.** 170 workers, 75 minutes,
**660 000 candidates**. The search climbed from a DNF at CP8 to a DNF at CP9 in
the first twenty seconds and then **did not improve again for 45 minutes**, and
**not one candidate in 660 000 finished the map** (`finish 0%` throughout).
Rebuilding a single sector forward from a centred-steering start is not
something this optimiser can do here.

So the answer to the open question, on this map, is no: **an optimiser cannot
cross from the drifting regime into the gripping one, in either direction, by
any operator this toolchain has.** Damping fails (§3), transplanting fails (§3),
and rebuilding forward fails here. The low-slip lap exists — a human drove it —
but it is not reachable from a tape that is not already in it.

**Amended by w218, and this is the better statement.** They ran the same
operator at shallower depths, seeding with r21's own inputs at a state-matched
anchor instead of by centring, and graded it by *rungs of the DNF ladder to
cross*:

| destroy from | seed depth | rungs to a finish | outcome |
|---|---|---|---|
| CP16 (tick 9238) | `DNF cps 15` | 2 | **finished in 139 s / 3048 candidates** |
| CP14 (tick 8418) | `DNF cps 14` | 3 | climbed one rung, no finish |
| CP12 (tick 7313) | `DNF cps 12` | 5 | climbed one rung in 20 800 candidates, no finish |
| my sector 11 (centred) | `DNF cps 8` | ~6 | climbed one rung in 660 000, no finish |

**So the operator is not dead — it has a horizon, and the horizon is about two
rungs.** My 660 000 candidates were six rungs out and did exactly what their
five-rung attempt did. "Rebuilding forward fails" is wrong; **"rebuilding
forward has a reach of about two checkpoints"** is right.

The reason the horizon is so short is that **the ladder has only six rungs**:
the oracle returns a checkpoint *count* and no checkpoint *time*, so every DNF
at a given depth scores identically and within a rung the search is a flat
random walk. And w218 calibrated something that bears directly on §7's barren
front third: **on this map the server emits the "reached N of M checkpoints"
clause only from N ≥ 6** (nine stop-tapes; `oracle.rs` claims N ≥ 2 from a
different map). **Below CP6 a shaped search is completely blind, so my zero
result on sectors 1–5 is partly an instrument limit and should not be read as
"there is no time there."**

### The endgame neighbourhood is provably empty

Closing check, `tmsimp --mode kbx` on the 96077 tape over the last 1.8 s
(ticks 9600–9782), every event shifted ±1…4, every segment re-valued to each
alphabet symbol, every event deleted, an insertion at every tick with every
symbol, each adjudicated by the plain oracle:

> `kbx round 0: 161290 candidates, NO improvement on 96077 ms — the whole 1-move
> keyboard neighbourhood is exhausted`

The one region of this map that yielded anything to search is now closed at one
move. (A larger sweep over ticks 9200–9782 at full 255-symbol resolution was
started and did not complete a round inside the remaining lease; the 182-tick
sweep above is the one that finished and the one that is claimed.)

### A second agent, and the better number

A second agent (**w218**) took this map at ~22:58 while my arms were running.
They independently re-validated my 96078 in their own build tree (with a
known-answer human ghost in the batch, 48/48 deterministic), and their own
search has since reached **96068**, eleven milliseconds better than my 96077.
**Their tape is the better one and the map is theirs to finish**; mine is banked
as `best_96077.Ghost.Gbx` beside theirs.

Two of their findings correct or extend mine and are credited above and here:
the `r23` DNF is a **loader failure, not a physics divergence** (§0), and the
car model explains only **1.6 %** of yaw-rate variance on this map, which voids
every steering-based prior, corridor and predicate the project owns. Their
enumeration of `--qlevels` ladders is the sharpest statement of §3's wall that
exists: **every ladder from 1 to 32 levels DNFs at CP1, including a 65-value
ladder whose worst error is ±2 of 127**, with the no-ladder identity control
returning 96078.

## 8. Artefacts

In `~/persistent/private-30d/tm-unbeaten/210218/`: `map.Map.Gbx` (Nadeo copy),
`ghosts/` (all 30 leaderboard ghosts), `csv/` (all 30 decoded per-tick
trajectories), `meta/` (checkpoint splits), `all.txt`, `validate_field.txt`,
`RESULT.md`.
