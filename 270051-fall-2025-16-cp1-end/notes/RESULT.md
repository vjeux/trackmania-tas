# Map 270051 — `Fall 2025 - 16 CP1 End` — author time beaten, and a human-shaped route to it

**Unconstrained TAS floor: 4830 ms (validated).** Author time 4831 ms, never
beaten by a human. Human world record 4834 ms (OriginalCJM), 913 records.

**The deliverable a person should practise is not that one.** A tape built out
of inputs that each tolerate a ±10 ms timing error reaches **4831 ms — the
author time — with only two changes to the human WR line.** That is the useful
result: the author drove 4831, so a human-sized path exists, and this is one.

| tape | time | vs AT | vs human WR | what it is | file |
|---|---|---|---|---|---|
| unconstrained | **4830** | −1 | −4 | 3 changes; one is a single-tick stab | `m270051_4830.Ghost.Gbx` |
| **human-shaped** | **4831** | 0 | −3 | **2 changes, both 30 ms gentle trims, each ±10 ms tolerant** | `m270051_human_shaped_4831.Ghost.Gbx` |
| one-input | **4832** | +1 | −2 | **1 change, a 120 ms trim, ±20 ms tolerant** | `m270051_one_input_4832.Ghost.Gbx` |
| keyboard | **4834** | +3 | 0 | 18 press events, alphabet {−127, 0, +127} | `m270051_keyboard_4834.Ghost.Gbx` |
| human WR | 4834 | +3 | — | analog, 152 distinct steer values | (downloaded) |
| best keyboard human | 4843 | +12 | +9 | 11 press events | (rank 7) |

Every one of those times is the plain oracle's answer to that exact file.

---

## 1. Validation

* **Ground truth.** All 24 downloaded human ghosts (ranks 1–24) re-simulate to
  their leaderboard millisecond **exactly, 24/24**.
* **The map file is Nadeo's.** `map16.Map.Gbx` came from trackmania.exchange by
  exchange id, and is **byte-identical (sha256 `c8025f2d…`) to the file behind
  Nadeo's own `core.trackmania.nadeo.live/maps/<guid>/file`** — that endpoint
  307-redirects to `trackmania-prod-storage-map-s3.cdn.ubi.com` and the
  redirect target needs **no authentication**. (New: ACQUISITION.md said the
  Nadeo URL needs a token. Following the redirect, it does not. Recorded there.)
* **Five cold-process validations** of the banked 4830, each in a fresh
  directory with a fresh server and the rank-1 human ghost alongside as a
  known-answer control: 5/5 `"Time": 4830, "IsValid": true, NbRespawns 0`,
  `"Desc": "validated time is actually better! (4834 > 4830)"`, control 4834
  every time. Transcript: `validation_transcript.txt`.
* **A second independent code path** (`tmmaps oracle`) agrees.
* **Identity control in every batch** of every search — the incumbent, unlabelled,
  asserted to return its known time. This caught a real bug (§6).
* **Every banked and intermediate tape re-validated** through the plain oracle
  after the fleet-wide phantom warnings: g0 4831, g1 4830, f0 4830, f3 4830,
  f4 4830, f5 4830, f6 4830, f7 4831, h5 4833, rob 4832, rob2 4832, rob4 4831,
  kb 4839, kb2 4839, kb_p009 4834, kb_p012 4837. **No phantoms.**
* **Not exposed to the three known phantom defects.** Every search process was
  launched with an explicit distinct `--root`; the `--fork` resume path was
  never used (this search is the classic full-simulation oracle throughout), so
  the consumed-record defect cannot apply; the `edge` operator was never used.
* Nothing was submitted to a Nadeo leaderboard.

---

## 2. What this map is

* **484 race ticks (4.83 s), one checkpoint — the finish.** No intermediate
  split: a DNF returns no information whatsoever.
* **Full throttle from lights-out to the finish, no brake, ever.** The whole run
  is a steering problem.
* **The last 620 ms is ballistic flight.** The car leaves the ramp at 4210 ms at
  176.95 km/h on a 19.90° climb and crosses the finish in mid-air, still rising.
* **The finish is a near-vertical plane at z ≈ 750.30.** 1 ms = **4.55 cm** of
  travel there.
* Inputs after ~4360 ms are worth 0 or −1 ms: the car is in the air.
* **Steering during the countdown does nothing** (ticks −1530…0 ms swept over
  the full ±127 range: 4830 every time). Negative result, cheap, now recorded.
* **The engine rate-limits the wheel.** A single-tick steer command of −32 and
  one of −127 give *byte-identical* results; only below ~−16 does the magnitude
  start to matter. So "how hard you flick" is meaningless for a one-tick input —
  only "how long" is.

---

## 3. Where the time comes from — measured, tick by tick

`fk btraj` trajectories (position to 3.4 mm) for the human WR and for our tape,
projected onto the WR's own racing line:

| distance along track | t_WR | t_ours | Δ | v_WR | v_ours | lateral offset |
|---|---|---|---|---|---|---|
| 30 m | 1996 | 1996 | 0.00 | 101.6 | 101.6 | 0.000 m |
| 45 m | 2494 | 2493 | −0.35 | 124.3 | 124.5 | 0.032 |
| 75 m | 3206 | 3205 | −0.59 | 176.4 | 176.5 | 0.048 |
| 95 m | 3598 | 3597 | −0.86 | 182.5 | 182.7 | 0.083 |
| 115 m | 3992 | 3990 | −2.38 | 178.6 | 179.0 | 0.123 |
| 135 m | 4401 | 4397 | −3.72 | 171.7 | 172.2 | 0.123 |
| 145 m | 4615 | 4611 | −4.04 | 166.6 | 166.8 | 0.145 |

**One discrete spot, or diffuse? Diffuse, then compounding.** Nothing happens for
the first 35 m. From there we carry **+0.2 to +0.5 km/h** for the rest of the
run, and the lead grows smoothly — half of it in the last 40 m, through the big
left-hander and up the ramp.

**Where the speed was bought:** at the exit of the first sweep, and at the entry
to the big left. Both are classic slow-in/fast-out: at 35 m we are *0.1 km/h
slower* than the WR, and from 40 m on we are faster and stay faster.

**The whole margin is banked before the wheels leave the ground.** State at the
takeoff tick (4250 ms):

| | human WR | ours | Δ |
|---|---|---|---|
| position z | 723.787 | 723.943 | **+0.156 m** |
| speed | 176.954 km/h | 177.149 km/h | +0.195 |
| v_z | 46.0256 m/s | 46.0559 m/s | +0.030 |
| launch angle | 19.9024° | 19.9429° | +0.04° |

15.6 cm further along at the same instant ÷ 46 m/s = 3.4 ms, plus 0.03 m/s over
620 ms of flight = 0.4 ms. **3.8 ms of the 4 ms is pure position at takeoff.**
Same jump, same arc, same attitude — we simply arrive 15.6 cm earlier.

**Is the line inside the human corridor?** Deeply. Maximum lateral deviation
from the WR line is **12.3 cm**, against a human field spread of up to **1.35 m**
at the same points. Inside at every station. This is **not** a route discovery
and **not** an exploit.

**Ground contact:** identical. Same takeoff tick (4250 ms), same number of
airborne ticks, same attitude through the flight (yaw/pitch/roll within 0.005
rad of the WR at every tick of the jump). No earlier takeoff, no different
landing, no attitude trick.

### Where the FIELD's spread is created

The same question asked of the 24 human ghosts, by timing each one across
z-planes at 640/660/680/700/720/740/750 (from their own telemetry — a CP1-End
map has no checkpoints, but a plane crossing is a split all the same):

| sector | race time | spread over the field | correlation with the final time |
|---|---|---|---|
| z 640→660 | ~1.52→2.44 s | **69.8 ms** | **0.05** |
| z 660→680 | ~2.44→3.14 s | 44.7 ms | **0.43** |
| z 680→700 | ~3.14→3.71 s | 33.9 ms | **0.31** |
| z 700→720 | ~3.71→4.17 s | 16.4 ms | 0.06 |
| z 720→740 | ~4.17→4.61 s | 7.0 ms | 0.26 |
| z 740→finish (airborne) | ~4.61→4.83 s | **5.1 ms** | **0.07** |

Two things fall out. **The jump costs everyone the same** — a 20 ms field spread
compresses to 5 ms over the final 225 ms, uncorrelated with who wins; the
visible, dramatic end of the map is not where the time is. And the sectors that
actually *predict* the finishing order are **z 660→700, i.e. race 2.4–3.7 s** —
the fast descent and the entry to the big left. The largest raw spread (the
640→660 acceleration phase, 70 ms) predicts nothing: drivers trade it straight
back.

Both of our decisive inputs (§4) sit inside z 660→700. The sector that separates
the human field is the sector where the remaining time was.

---

## 4. The technique — what to actually do

Sign convention: negative = left, positive = right, ±127 = full lock. The route
is the human WR's route; only the marked inputs differ.

| time | what the car is doing |
|---|---|
| 0.0–1.8 s | standing start, one long right-hand sweep at ~50–60 % lock |
| 1.85–2.05 s | left countersteer to ~72 % to straighten out of the sweep |
| **2.90–2.93 s** | **① light left brush, ~7 % lock, 30 ms — on what looks like dead straight road at 157 km/h** |
| 2.9–3.3 s | long descent, 100 → 183 km/h, small left trim |
| **3.35–3.38 s** | **② ease the left trim by ~1.5 % for 30 ms** |
| 3.55–4.1 s | progressive left to full lock, held ~300 ms |
| 4.1–4.21 s | unwind and climb the ramp at 177 km/h |
| 4.21–4.83 s | airborne, full right lock, crossing the finish still climbing |

### ① The light left brush at 2.90 s — the whole technique, and it is real

`steer −9/127 for three ticks at race 2900–2930 ms`, on top of the WR line.
**Worth 2 ms on its own (4834 → 4832).**

Tolerance, measured by sweeping placement and strength on the plain human WR
tape:

* **placement: three consecutive tick offsets (2890/2900/2910 ms) all give
  4832** — a 30 ms window. Several other placements in the same second also
  give −2 (e.g. 2 ticks of −8 at 2820–2840, 2 ticks of −14 at 2920).
* **strength: −6 to −14/127 all work**; the map of outcome vs strength is flat
  across that band.
* Outside the window the cost is small and graded: neighbouring placements give
  0 to +3 ms, not a crash.

This corrects an earlier reading of ours. Our *unconstrained* search had found
the same effect as a **one-tick, 75 %-lock stab** at the same instant, and we
first wrote it up as a chaotic lottery ticket. It is not. Run the robustness
search instead of the speed search and the same 2 ms comes back as a **gentle,
sustained, ±10 ms-tolerant touch**. The stab and the brush are the same
technique; the stab is just the twitchiest way to express it.

### ② The trim release at 3.35 s

`steer +2/127 (i.e. 1.5 % less left) for three ticks at race 3350–3380 ms`.
Worth **0 ms alone**, and **−1 ms on top of ①** (4832 → 4831). Four consecutive
placements (3350/3360/3370/3380 ms) all give 4831 — a **40 ms window**.

### The one-input version, if you only learn one thing

`steer −5/127 (4 % more left) for 12 ticks at race 3470–3590 ms` — carry a
little more left through the entry to the big left-hander. **4834 → 4832, one
input, four consecutive placements within 40 ms all give 4832.** This is the
single most forgiving 2 ms on the map.

### What our 4830 adds on top, and why it is not the thing to practise

The unconstrained 4830 replaces ① with a **single-tick** −96 stab and adds two
more trims. Its extra millisecond over the human-shaped 4831 comes from inputs
that pay only at one exact tick — neighbouring placements give +4 ms. Keep it as
the floor; do not practise it.

---

## 5. Verdict on the technique

**Known but unheld, expressed at a scale the field is not looking at.**

Not undiscovered: the line is the field's own line to within 12 cm, and "carry a
touch more left here" is not a new idea to anyone. Not un-executable: the author
drove 4831, and the two inputs that get us there are 30 ms long with 30–40 ms of
placement slack and a wide band of acceptable strength — that is a practisable
input, not a frame-perfect one.

What makes it fragile is that **it is invisible**. Both decisive inputs are
small steering trims on sections that feel like nothing is happening — ① is on
apparently straight road at 157 km/h, and ② is a 1.5 % release in the middle of
a long trim. Neither is at a corner, a landing, or any place a driver's
attention naturally goes. And their payoff is a **15 cm position gain at the
ramp**, which nothing in the cockpit shows you. A driver who happens to brush
left at 2.9 s gets 4832 and has no way of knowing which of their thousand
micro-inputs did it.

That is the honest reason 913 records stopped at 4834: not a missing skill, a
missing *target*. Now there is one.

Supporting evidence that the field really is at the wall: enumerating **every
single-tick steering change** to the human WR tape (5172 candidates), **95.3 %
make it slower or kill it** (10.1 % DNF, 29.6 % no change), only **4.6 %** give
−1 ms and only **0.10 %** (5 of 5172) give −2 ms. The WR is a genuine local
optimum at tick resolution.

### Keyboard is viable here, and it is not the author's route

Read off the ghosts rather than assumed: **ranks 7, 9 and 12 are pure keyboard
runs** — exactly three steer values {−127, 0, +127}, 11–15 change events —
running 4843, 4845, 4847. The WR itself is analog (152 distinct values, 262
change events).

Optimising in the digital space directly (edges slid in time, presses inserted;
alphabet never leaves {−127, 0, +127}) from all three keyboard seeds:

| seed | events | result |
|---|---|---|
| rank 7 (4843) | 10 | 4839 |
| rank 12 (4847) | 15 | 4837 |
| rank 9 (4845) | **18** | **4834** |

So a **pure keyboard tape matches the human world record** — that is a real
finding for the field, and `m270051_keyboard_4834.Ghost.Gbx` is 18 keypress
events long. But three independent digital searches all stall at 4834+, 3 ms
short of the AT. **The author time is not reachable on keyboard as far as we can
measure; 4831 needs analog trims**, which is consistent with both decisive
inputs being 5–7 % of lock.

### Low-input variants, and an honest warning about them

Simplifying the validated 4830 tape (`m16 simplify`), events counted over the
race window:

| variant | events | alphabet | time | cost |
|---|---|---|---|---|
| validated tape | 270 | 154 | 4830 | 0 |
| deadband 8/127 + quantise to 8 | 99 | 32 | 4842 | +12 |
| quantise to 32 + deadband 24 | 27 | 8 | 4871 | +41 |
| deadband 1/127 | 207 | 133 | 5649 | +819 |
| pure digital {−127,0,127} | 10 | 3 | DNF | — |

**These numbers do not mean what they look like, and the control proves it.**
Run the identical simplification on the *human world record's own tape* and it
behaves the same way: deadband 2 → DNF, every digital variant → DNF, and the
best simplification is +13 ms. The same is true of noise: with correlated
steering noise of σ = 0.5/127 (0.4 % of lock, 50 ms correlation), the human WR
tape DNFs 36 % of the time with a median of +6 ms; ours DNFs 52 % with a median
of +19 ms.

An **open-loop input tape** in a chaotic simulator is fragile no matter who
wrote it. A driver is a closed-loop controller and corrects continuously, so
open-loop noise sensitivity says nothing about human difficulty. The meaningful
tolerance measure is the per-input one in §4, and by that measure the technique
is comfortable. This is worth flagging fleet-wide: **do not report open-loop
jitter as evidence about human executability without running the human's own
tape as the control.**

---

## 6. Method, and what earned its place

1. **Acquisition** followed `ACQUISITION.md` unchanged and worked first time.
2. **Near-exhaustive beats clever, on a 484-tick tape.** The whole single-tick
   neighbourhood (484 ticks × all 254 steer values = **122 936 candidates**) is
   about **4 minutes of box time** at ~500 evals/s on 176 cores. So the search
   is a compounding greedy over an *enumerated* neighbourhood, verified by the
   oracle at every stack size, not a sampler. Single-tick moves alone took the
   human WR to 4831 in 54 s; adding span moves reached 4830 in 97 s.
3. **The sub-millisecond vernier — the plateau breaker.** The validator quantises
   to 1 ms, so nearly every neighbour of a good tape reports the same
   millisecond and the search stalls on a tread. Fix: a **ladder of maps whose
   finish gate is relocated by 1/10 (then 1/20) of a millisecond of travel each**
   — 4.5 mm and 2.3 mm apart — built with a new `tmmaps gate` subcommand.
   `Σ_k T_k` over the ladder falls by exactly 1 for every 1/K ms of true
   improvement, *everywhere*, so it is a globally valid objective at 0.05 ms
   resolution while the real map still decides the answer.
4. **Robustness as the objective, not speed.** `m16 robust` scores a candidate
   by the **worst** time over a ±1 or ±2 tick placement window instead of its
   own. That one change turned a "lottery ticket" into a teachable technique and
   produced the 4831 and 4832 tapes. On a map where the field is 3 ms behind a
   driven AT, this is the search that matters.
5. **Convergence is real.** From the 4830 tape: 122 936 single-tick candidates,
   71 021 span-2…21 moves, 91 844 moves including throttle lifts and brake taps,
   a second 122 936-candidate pass against a 0.05 ms ladder, and **169 216
   two-tick PAIRS** (the one class a one-move-at-a-time search structurally
   cannot see, since it includes pairs whose halves are individually useless) —
   about 578 000 evaluations, **none better than 4830**, none even better at
   0.05 ms resolution.

Negatives worth recording:

* **Other human seeds are worse.** Full greedy from rank 5 (4838) converged to
  4833, 3 ms behind the same treatment of rank 1. The basins do not merge.
* **The heavy machinery earns nothing here.** Segment maps are impossible (one
  checkpoint, and it is the finish); the fork server's mid-simulation resume
  saves little when a whole run is ~2 ms of simulation; the early-abort
  predicates have almost no tail to skip. A plain batch oracle is the right tool
  for a 484-tick tape — and, given the resume-consumption phantom defect found
  later, also the safe one.

### Vernier ladder vs the in-child sub-tick plane

The 191465 agent's in-child plane surrogate needs no map surgery and is cheaper.
**On this map it would be wrong, and there is a measured reason.** The finish
trigger is body-based, and this map's finish is crossed **airborne**: taking
each of the 24 human ghosts at its own validated finish millisecond, the
crossing position spreads **0.88 m** over the field — **19 ms** at 4.55 cm/ms —
and even over the 17 clean flying finishes it still spreads **6.0 cm ≈ 1.3 ms**.
A plane through the car's centre cannot represent that.

The gate ladder does not have the problem, because **every ladder map is still
adjudicated by the real trigger against the real car body**. It is not free of
model error either: two human tapes that differ by 1 ms on the real finish can
tie on a ladder placed 1.7 m further down the track. That is why the ladder is
used strictly as a **tie-break underneath a hard filter on the real map's
millisecond** — a move is only ever considered if the real map says it is not
slower, so ladder error can never make the reported time worse.

**Recommendation for a flying-finish map: use the gate ladder.**

### The bug this run contributed

The ladder gave a perfectly self-consistent wrong answer the first time it ran:
all ten planes reported exactly the real map's time. `oracle::Worker` links its
map into the worker's `UserData/Maps` and never removes it, and **every gate map
keeps the original mapUid**, so ten ladder maps plus the real map ended up in
one `Maps/` directory and the server bound the uid to whichever it found first.
Fix: one root per map. The tell was the ladder control being *too* clean —
exactly 10 × the real time.

That is the fifth silent-corruption defect in this project caught by an identity
control, and the second caused by the one-map-per-directory rule.

---

## 7. Transfer to the sister map

`Fall 2025 - 18 CP1 End` (270053) is the same cut-down of a Fall 2025 campaign
map, same 3 ms gap, same era. Three things here should transfer directly:

1. **Check for a flying finish first.** If the run crosses the gate airborne,
   the whole problem collapses to "position along the track at the takeoff tick"
   and the tail of the tape is nearly free. Test it in two minutes: overwrite
   every input after tick T with a constant and see where the finish time stops
   caring.
2. **Build the gate ladder, not the in-child plane** (§6), and give every map
   its own worker root.
3. **Run the robustness search, not just the speed search.** The best speed move
   and the best *teachable* move were the same physical effect here, expressed
   at completely different scales, and only the robustness objective found the
   teachable form.

---

## Files

| file | what |
|---|---|
| `m270051_4830.Ghost.Gbx` | unconstrained floor, 4830 ms |
| `m270051_human_shaped_4831.Ghost.Gbx` | **the one to study**: 4831 ms, 2 tolerant inputs |
| `m270051_one_input_4832.Ghost.Gbx` | 4832 ms from a single 120 ms trim |
| `m270051_keyboard_4834.Ghost.Gbx` | pure keyboard, 18 events, ties the human WR |
| `map16.Map.Gbx` | the map (= Nadeo's own file, sha256 `c8025f2d…`) |
| `g1.json`, `rob4_4831.json`, … | raw input tapes, 10 ms per entry |
| `validation_transcript.txt` | five cold validations, full server JSON |
