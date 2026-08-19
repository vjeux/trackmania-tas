# 199100 — `Spring 2023 - 24 (2-UP)` — the author time falls, and it falls on a keyboard

uid `zFw7p8IFpSWwZcZMxLy4rTpX7o2` · TMX / unbeaten.at id 199100 · author `.ar`
**AT 51602** · human WR **52202** (JuntaoTM) · 6 recorded runs · gap 600 ms
Tags: Reactor · Plastic · Altered Nadeo

**Nothing here has been or will be submitted to a Nadeo leaderboard.**

## The family

| tape | validated | vs AT | vs human WR | device | steer alphabet | input events | added actions vs the human it is built on |
|---|---|---|---|---|---|---|---|
| `A6_49778` | **49778** | **−1824** | −2424 | TAS analog | 244 values | 1084 | rewritten flight |
| `K7_51062` | **51062** | **−540** | −1140 | **keyboard** | **3** | 219 | ~15 |
| `K4_51107` | 51107 | −495 | −1095 | keyboard | 3 | 214 | ~14 |
| `K1a_51575` | 51575 | −27 | −1024 | keyboard | 3 | 204 | **7** |
| `R4468_51598` | 51598 | −4 | −604 | keyboard | 3 | ~200 | 1 tap + a 20-minute re-aim |
| — human WR, JuntaoTM | 52202 | +600 | — | pad | 223 values | 896 | — |
| — human #3, uelen. | 52599 | +997 | +397 | **keyboard** | 3 | 190 | — |

Every number came out of the plain oracle (`TrackmaniaServer /nodaemon
/validatepath=`), with a human ghost carried as a known-answer identity control
in the same batch. The headline tapes were additionally re-validated **cold** —
fresh directory, fresh server processes, against the **separately downloaded
trackmania.exchange copy** of the map (sha256-identical to Nadeo's): same
milliseconds, with 52202 and 52599 as controls in the batch. No search arm ever
produced a phantom: the guard was on for every arm and fired zero times.

---

## 1. Controls, before anything else

### §4 identity control
All six ghosts were downloaded to `.part`, size- and `GBX`-checked, renamed only
on success, into a directory this session created (§8a discipline). Five of six
re-simulate to their exact leaderboard millisecond.

### §8 field reproduction — 5/6, and the sixth is characterised

| rank | time | re-simulated | respawns | game build |
|---|---|---|---|---|
| 1 | 52202 | **52202** | 0 | 2026-01-18 git 128114 |
| 2 | 52495 | **52495** | 0 | 2026-01-18 git 128114 |
| 3 | 52599 | **52599** | 0 | 2026-02-02 git 128149 |
| 4 | 54918 | **54918** | **1** | 2026-02-02 git 128149 |
| 5 | 57358 | **57358** | **1** | 2026-02-02 git 128149 |
| 6 | 113448 | `wrong simu, 7/10 cps` | declared `4294967295` (unset) | **2023-03-31 git 120733** |

The single failure is the only pre-2026 record on the board — set on the game
build of 2023-03-31, two days before the map was uploaded to Nadeo and ~7 400
build numbers older than every other record. It is a 113-second run whose input
stream carries `_` tokens no other tape has (71.8 s of it inside the last sector
alone), in a ghost format so old it does not even record a respawn count.

It is **not** the 203072 failure mode:

* **respawns are not the cause** — ranks 4 and 5 each contain a real respawn
  (`NbRespawns: 1`) and both reproduce exactly;
* **build 128149 is not the cause** — that is the build 9 of 18 records failed
  on in 203072; here **3 of 3** records set on it reproduce exactly;
* **the file is not truncated** — re-downloaded, sha256-identical, 118 338
  bytes, `GBX` magic (§8a).

Everything this work seeds from (ranks 1–5, including the world record and the
keyboard run) re-simulates exactly. The user was told about the partial
mismatch when it was found.

### Map provenance
The Nadeo-served `.Map.Gbx` (1 997 303 bytes, sha256
`89a2415fbff56a2ec9e730ed916338a27f9b0419a5e5762ee57782002c0f547b`) is
**byte-identical to the trackmania.exchange copy**.

### §9 — the author's validation ghost is NOT in this map
The header says `validated="1"`, `authortime="51602"`, `exebuild=2023-03-31`.
The decompressed body (2 810 537 bytes) contains **no `CPlugEntRecordData`
(0x0911F000) and no ghost input chunk (0x0309201D)** — established by
byte-scanning the whole body, not by a parser that might have skipped them.

Positive control, same tool and command: **228607's map body does contain
0x0911F000** (2 hits). 227969, 270051 and 191465 do not. And independently:
the container agent's `ct probe` survey of all 31 maps in `tm-unbeaten/`
(`ACQUISITION_addendum_embedded_author_ghost.md`) puts 199100 in its "nothing
embedded" group — two tools, written separately, agree.

So for this map there is **no evidence in the file that a human ever drove
51602 on this layout**, and the author `.ar` appears nowhere on the map's own
six-run leaderboard. (`inPlugin` on unbeaten.at is *not* `atSetByPlugin`: it is
`true` for all 1 756 records in the dataset and carries no information.) The map
is an **Altered Nadeo** copy of an official campaign map, which is the obvious
channel for an author time to arrive without a lap being driven on the altered
layout. **I do not claim the AT here is a driven lap.** It does not matter for
the deliverable: the technique below is demonstrated by running it, and its
human-sized version is measured rather than asserted.

---

## 2. What the map is

Nine checkpoints (ten waypoints with the finish), measured from the world
record's own telemetry:

| sector | race | dur | what happens | km/h in→out | airborne |
|---|---|---|---|---|---|
| 1 | 0→4.2 s | 4211 | drop off the start platform, y 158→83 | 0→238 | 73 % |
| 2 | →11.1 s | 6915 | long descent, y 82→50 | 242→395 | 0 % |
| 3 | →17.0 s | 5872 | fast section, peak 511 | 393→444 | 8 % |
| 4 | →23.7 s | 6739 | jump complex, y up to 119 | 445→353 | 20 % |
| 5 | →29.9 s | 6125 | climb back to the y = 74 deck | 358→415 | 0 % |
| 6 | →32.1 s | 2231 | flat deck | 416→461 | 0 % |
| 7 | →35.0 s | 2950 | flat deck, peak 562 | 462→532 | 0 % |
| 8 | →38.2 s | 3122 | **fastest stretch of the map, peak 589** | 533→574 | 0 % |
| 9 | →40.6 s | 2391 | climb and slow into the launcher approach, y 74→137 | 570→359 | 0 % |
| 10 | →52.2 s | **11646** | **the launcher and the powered flight** | 356→320 | 82 % |

### Sector 10 is the whole map
At race ≈ 42.6 s a **launcher captures the car**: horizontal speed collapses
(300 → 121 km/h in two 50 ms samples), the car ends up inverted (roll ≈ ±3 rad),
and from there it is **under continuous thrust while airborne** for about seven
seconds — climbing y 160 → 342 m while *gaining* speed back to ~400 km/h — then
arcs over and dives through a finish gate hanging in the air at ≈
(148, 237, 1263), 660 m down-axis from the launcher, crossed at ~88 m/s with
vy ≈ −69 m/s.

Three consequences shaped everything:

* **The flight is powered and steerable, not ballistic** — the opposite of
  270051's jump. Steering and pitch aim the thrust, so air control is worth
  seconds.
* **The finish is a gate you must hit, in the air.** A miss is a DNF, not a
  slower time. This is why *every* open-loop perturbation anywhere in the run
  DNFs (§5) — the map is what is fragile, not our tape.
* **The launcher is the field's filter**: 2 of the 5 valid records **respawn**
  there (ranks 4 and 5, +3.2 s and +4.7 s), and the 6th record spent 71.8 s in
  this one sector.

### The field's own sector table (declared splits, ms)

| sector | r1 52202 | r2 52495 | r3 52599 | r4 54918 | r5 57358 | best |
|---|---|---|---|---|---|---|
| 1 | **4211** | 4230 | 4230 | 4230 | 4228 | r1 |
| 2 | 6915 | **6864** | 6916 | 6981 | 7146 | r2 |
| 3 | 5872 | 5814 | 5982 | **5720** | 6016 | r4 |
| 4 | 6739 | 6708 | 6917 | **6577** | 6836 | r4 |
| 5 | 6125 | 5977 | **5943** | 6008 | 6082 | r3 |
| 6 | 2231 | 2181 | 2193 | 2201 | **2130** | r5 |
| 7 | 2950 | 2904 | **2887** | 2891 | 2889 | r3 |
| 8 | 3122 | 3062 | **2982** | 2995 | 3025 | r3 |
| 9 | **2391** | 2506 | 2698 | 2485 | 2692 | r1 |
| 10 | **11646** | 12249 | 11851 | 14830 | 16314 | r1 |

**Sum of sector bests = 51351 — 251 ms inside the author time**, out of sectors
five humans have already driven. The world record is the *slowest of the top
four* to cp9 (40556 vs r4's 40088) and wins the map purely in sector 10.

---

## 3. Where our time comes from: all of it is air control

The 49778 tape's inputs are **byte-identical to the human world record's for the
first 4 383 ticks** — the entire ground run, the approach and the launcher
entry. The first differing tick is race **42330 ms**, and the difference there
is a steering value of 1/127 instead of 0.

Everything — 2 424 ms against the WR, 1 824 ms against the author time — is
produced by inputs **after 42.33 s**.

Value curve (our inputs before tick T, the WR's after; `tapetool mix`, one file
image, so the mix is exactly as faithful as any search candidate):

| cut at race | result | note |
|---|---|---|
| 42330 (before our first change) | 52202 | identity control ✓ |
| 42750 | 51948 | 42 ticks of entry nudges: **−254 ms** |
| 43000 | 51822 | −380 ms |
| 43250 … 47500 | **DNF** | the two flights have separated; they cannot be rejoined |
| 48000 | 50647 | |
| 49000 | 50352 | |
| 49500 | 50258 | |
| 50500 | 50224 | inputs after the finish are inert |

The mirror image (WR first, ours after) DNFs at **every** interior cut. The
flight is one committed trajectory, chosen at the capture and corrected only in
its own frame.

---

## 4. The technique a human can practise

The keyboard result is the deliverable, because it is stated as a modification
of a run a human already drove: **uelen.'s rank-3 keyboard run (52599)** — three
steering values, 190 input events, no respawn.

### 4a. The seven-action version — 51575, 27 ms inside the AT

Keep every one of uelen.'s inputs. Add:

| # | race time | action | duration |
|---|---|---|---|
| 1 | **43.23 s** | **tap brake** | 10 ms |
| 2 | 43.65 s | release gas | 110 ms |
| 3 | 43.82 s | tap brake | 30 ms |
| 4 | 47.80 s | tap right | 20 ms |
| 5 | 48.28 s | tap right | 50 ms |
| 6 | 49.17 s | hold brake | 90 ms |
| 7 | 49.27 s | hold right | 470 ms |

What each is worth (cumulative, `tapetool mix` against rank 3):

| through action | validated | gain |
|---|---|---|
| none (control) | 52599 | — |
| **#1 alone (one 10 ms brake tap)** | **51869** | **−730 ms** |
| #1–#3 | 51724 | −875 |
| #1–#5 | 51638 | −961 |
| #1–#7 | 51575 | −1024 |

**Three quarters of the gain is one brake tap, 0.6 s into the reactor climb.**

### 4b. The full keyboard tape — 51062, 540 ms inside the AT

Same run, ~15 added actions, all in the flight. The shape of it: **pump the
brake through the first 900 ms of the climb**, then aim in the last two seconds.

| race | action |
|---|---|
| 43.23 | brake 10 ms |
| 43.48 | brake 20 ms |
| 43.58 | brake 70 ms |
| 43.65 | **gas off 110 ms** |
| 43.79 | brake 60 ms |
| 43.93 | brake 120 ms |
| 44.10 | brake 20 ms |
| 44.82 | release brake 10 ms earlier than uelen. does |
| 47.41 | let go of left 30 ms earlier |
| 47.80 | right 20 ms |
| 48.14→48.37 | right (uelen. holds 40 ms here; hold 230) |
| 48.43 | left |
| 48.57 | brake 40 ms |
| 49.17 | brake 90 ms |
| 49.37→49.85 | **hold right ~480 ms** |
| 49.85 | left into the gate |
| 49.88 | brake 70 ms |

### 4c. Is the decisive tap a lottery ticket? No — measured two ways

**A 217-point grid** (`tapetool tap`), one brake tap applied to the untouched
human keyboard run, start times 42.95→43.55 s × durations 10–200 ms:

* every variant that finishes lands between **51 764 and 52 529 ms** — a tap
  anywhere in a ~400 ms window is worth 100–800 ms, typically ~700;
* 10–30 ms taps are what work (13 of 31 start times finish at 10 ms); 50 ms and
  longer nearly always miss the gate;
* the misses are misses, not slow runs.

**A mistimed tap is recoverable — the decisive test.** Four tap times were
taken from that grid, two that finish and **two that miss the gate outright**,
and each was given a small keyboard search allowed to change only inputs
**after 44.0 s** (6 workers, 20 minutes, ~10 400 evaluations — about 2 % of the
effort behind the 51062 tape):

| tap at | tap alone | after re-aiming later inputs only |
|---|---|---|
| 43.07 s | 51949 | **51744** |
| 43.13 s | **DNF** | **51598 (inside the AT)** |
| 43.31 s | 52029 | **51795** |
| 43.37 s | **DNF** | **51748** |

All four land within 51 ms of each other, and the two that missed recover as
well as the two that hit. **The exact tap time inside a ~300 ms window does not
decide the run; the aiming afterwards does** — which is precisely the part a
driver does closed-loop and a tape cannot.

### 4d. The same tap on the pad world record does nothing
Applied to rank 1 (52202) across the same window it costs 0–60 ms and **never**
DNFs — a flat, boring response. uelen. enters the capture holding **full left**;
the brake tap perturbs a rotation that is already running. **The trick belongs
to the keyboard entry, not to the map in general** — which may be exactly why
nobody found it: the two fastest humans are on a pad, where it does nothing.

---

## 5. Tolerance, with the human's own tape as the control

Blunt version first: **in open loop, one 10 ms shift of one input anywhere in
this run makes the car miss the finish gate.** That is the map. The control
proves it — `tmsimp --mode tol` on **uelen.'s untouched 52599 run**: essentially
every input reads `−1 tick: DNF, +1 tick: DNF`.

That same control produced the sharpest number in this write-up: **shifting one
of the human's own key presses by a single tick is worth up to 733 ms**
(`44580 −1: −733`, `45320 −1: −714`, `45640 +1: −724`, `47440 −1: −552`,
`44520 −1: −300`). Their flight is nowhere near its own optimum, and the field
could never have discovered that by feel, because a 10 ms difference reads as a
DNF rather than as a gain.

Where our tape is tolerant (`tmsimp --mode tol`, ±6 ticks, on the 51575 tape):

| action | usable window | cost at ±1 tick |
|---|---|---|
| #6 brake 49.17 s | **±6 ticks (±60 ms), zero cost** | 0 / 0 |
| #7 right hold 49.27→49.74 s | **±6 ticks (±60 ms), zero cost** | 0 / 0 |
| #4 right tap 47.80 s | 8 ticks | +19 / +39 |
| #5 right tap 48.28 s | 1 tick | +77 / +432 |
| uelen.'s own flight presses | 1–2 ticks | mostly DNF |

So the aiming inputs — the last second and a half — are genuinely forgiving,
and the early climb is where the practice goes. Combined with §4c, the honest
summary is: **the timing of what you do is loose; the requirement to correct
afterwards is absolute.**

### Input alphabet and hold floor, from the human data
* Keyboard alphabet, read off uelen.'s tape: exactly `{-127, 0, +127}`.
* Hold floor, at tape resolution: uelen.'s shortest steering hold is **10 ms**,
  p10 60 ms, median 140 ms; her two brake presses are 70 and 310 ms. The pad WR
  steers with a 20 ms median. **A 10–30 ms tap is inside the alphabet the field
  already uses**, so the decisive input is not sub-human.

---

## 6. Sector-by-sector guide (keyboard)

Sectors 1–9 are **exactly uelen.'s rank-3 run**, unmodified — 5, 28, 26, 30, 37,
4, 6, 12, 13 input events per sector. Nothing in this work found a millisecond
in them (§7), so the instruction for the first 40.7 s is "drive the keyboard run
that already exists", with the caveat that r4 does the same line 468 ms faster
to cp9, so that time is there for a human even though no edit we could make
composed it.

What is new starts at cp9. Cues are what the driver sees:

1. **cp9 → the launcher (40.6 → 42.6 s).** Off the fast deck, up the climb,
   slowing to ~350 km/h. Nothing changes here.
2. **The capture (42.6 s).** The launcher takes the car: speed collapses to
   ~120 km/h and the car rolls inverted. uelen. is already holding **full left**
   from 42.63 s — keep holding. That rotation is what the next input acts on.
3. **The brake tap (≈43.2 s — about six tenths after you feel the grab, while
   the nose is coming down through level and before the car starts swinging
   toward the tower).** One short tap, 1–3 ticks. This is the 730 ms. Anywhere
   in 43.05–43.45 s works; if it feels early or late, do not abandon the run —
   §4c says you can still make the gate.
4. **Pump it (43.5 → 44.1 s).** For the full version: gas off ~43.65 for a
   tenth, and three or four more short brake taps through 44.1. This is pitch
   control — you are choosing where the thrust points for the whole climb.
5. **The climb (44 → 47.4 s).** As uelen. drives it: left releases, right at
   44.16 and 44.77, the brake/gas pair at 44.52–44.83, then the alternating
   left/right through the apex (y ≈ 342 m at ~49 s).
6. **The aim (47.4 → 49.9 s).** Two short rights (47.80, 48.14–48.37), a left,
   brake at 48.57 and again at 49.17, then **hold right for about half a second
   from 49.37** while the car is diving at the gate, flicking left into it. These
   are the tolerant inputs: ±60 ms costs nothing. This is the part you steer by
   eye.
7. **Finish** — airborne, nose-down, ~320 km/h.

---

## 7. What did NOT work (measured, not assumed)

* **The prefix is unimprovable by local search.** 22 workers, mutations confined
  to ticks 0–4250, wide windows (400/200), 2 ops per candidate, `--temp 40`:
  **0 improvements in 15 000 evaluations**, finish rate 5 %. Every ground-section
  mutation misses the finish gate.
* **The field's faster prefix does not transfer.** Rank 4 is on the same line
  (never more than 13 m from the WR) and reaches cp9 **468 ms** earlier,
  carrying 20–30 km/h more from 15 s on. Splicing its prefix onto a good flight
  DNFs at cp9 for every cut tried (40.0, 40.6, 41.0, 41.5, 42.0, 42.4 s). That
  468 ms is real and remains on the table for a human.
* **Other human seeds do not merge** (independent confirmation of 270051): an
  r2-seeded flight arm converged to 51675 and an r4-seeded one to 54533 while
  the r1-seeded arm was already at 50236. r4's tape carries a respawn the search
  cannot remove.
* **Two independent keyboard arms converged to 51107 and 51118** from different
  RNG seeds — the keyboard optimum on this entry is real and close.
* **Event thinning does nothing on this map.** `tmsimp` greedy deletion at a
  0 ms budget removes nothing from the keyboard tape and at a **30 ms** budget
  removes nothing from the 1 084-event analog tape. On 227969 the same pass cut
  185 steer events to 62. Here every event is load-bearing, for the same reason
  everything else is: a gate in the air.
* **A wider action-key ladder buys nothing in this basin.** An arm seeded from
  the keyboard tape with 4 detents per side (9 values) found **no improvement in
  24 500 evaluations** — the keyboard tape is already a local optimum on the
  richer ladder, so the "action keys" rung of the family is simply the keyboard
  rung.
* **The sub-tick plane was never used.** Airborne finish, attitude spread across
  the field: presumed invalid (227969's failure mode). With a 600 ms gap it was
  never needed.
* **`tmmaps` cannot parse this map** (`unhandled inline node class 0x40000000`),
  so segment maps and the gate ladder were unavailable. Sector timing was done
  geometrically from telemetry instead (`an199 sect`, reproduces the declared
  splits to ≤20 ms).
* **`fk btraj` cannot locate the vehicle state on this map/build.** Five
  attempts (ticks 100/1500/2000/4300/4600, with and without reference bounds,
  tolerances 0.3–3.0): either 0 shortlisted float triples, or 139–182
  shortlisted and none self-consistent. So **no trajectory exists for any TAS
  tape here**, and every claim in this document is made from inputs, splits and
  validated times — never from an unverified state read.

---

## 8. Defects found and tooling added

### `tmtas splice` is not faithful for a cross-splice (new)
Head and tail tapes that are **bit-identical over the splice region** produced a
different answer: at 42400 ms, head = our tape / tail = rank 1 gave **52121**
where the tapes are identical before 43670 ms and the answer must be 52202; the
reverse gave DNF. Cuts at 40000–42000 ms were correct, so it is cut-point
dependent, and **the built-in diagonal identity control does not catch it**
(a tape spliced with itself is fine). Use one file image instead:
`tapetool mix` takes tape A's own `Factory` and writes only different values
into the steer/accel/brake slots — the same operation the search performs on
every candidate.

### `FINISH_BASE` (reported by the 17-CP agent) — applied
Patched 1e8 → 1e12 in `main.rs`, `forksearch.rs`, `bin/tmtas.rs` before the
second half of the campaign. On a 9-CP map the defect cannot fire (a cp9 DNF
scores 9e7 against a finisher's ~9.995e7, and the guard's `> FINISH_BASE/2`
test needs a cp6+ DNF that has also beaten a finishing incumbent), but the fix
is free.

### New tools (Rust; no Python anywhere in this work)
* **`tapetool`** (`tmsearch/src/bin/tapetool.rs`) — `info` (events, alphabet,
  first differing tick between two tapes), `cmp` (per-tick side by side), `mix`
  (faithful head/tail composition + batch validation), **`tap`** (apply a
  synthetic brake/gas/steer input over a grid of start times × durations and
  validate the whole grid), `zero`.
  **`tap` is the one worth copying**: it measures the *basin* of a decisive
  input on a human's own tape, which is what "how hard is this to drive"
  actually means. It turned "a one-tick brake tap" from a suspicious lottery
  ticket into a measured 400 ms window.
* **`an199`** — sector timing by nearest approach to geometric gates (`sect`,
  `gates`; reproduces the declared splits to ≤20 ms with no map surgery),
  cumulative-delta comparison along a reference line (`cmp`), per-sector summary
  (`sectsum`), thrust-vs-attitude (`thrust`), hold-duration distributions
  (`holds`), profiles (`prof`, `fin`, `cps`).
* **`probe199` / `body199`** (`tmtraj/src/bin/`) — decompress a map body and
  count ghost/telemetry class ids in it: the §9 check, with a positive control.

### Method notes that generalise
* **Ask the mix, not the splice.** A value curve built by composing two tapes in
  one file image told the whole story of this map in 40 evaluations.
* **On a map with an airborne finish gate, "±1 tick = DNF" is not a statement
  about your tape.** Measure the human's own tape and you get the same table;
  what distinguishes a drivable technique is whether a *later* input can
  recover, and that is a search question you can answer in 20 minutes on 6
  cores.
* **Run the decisive trick on a second human's run before writing it up.** Here
  it gains 730 ms on the keyboard run and nothing at all on the pad WR, which is
  half the explanation of why the AT stood.

---

## 9. Files

In `~/persistent/private-30d/tm-unbeaten/199100/`:

| file | what |
|---|---|
| `tapes/A6_49778.Ghost.Gbx` | the unconstrained floor, **49778** |
| `tapes/K7_51062.Ghost.Gbx` | **keyboard, 51062** |
| `tapes/K1a_51575.Ghost.Gbx` | **the teachable one: uelen. + 7 actions, 51575** |
| `tapes/R44*.Ghost.Gbx` | the four mistimed-tap recoveries (§4c) |
| `tapes/t04*d001.Ghost.Gbx` | single-tap probes on the human keyboard run |
| `tapes/*` | the whole lineage, each validated |
| `tick/*.txt` | tick scripts. **race time = script time − 1500 ms** (−1550 for rank-3-derived tapes) |
| `ghosts/` | all six human ghosts as downloaded |
| `csv/` | decoded telemetry for the five valid human runs |
| `map.Map.Gbx` | the map, sha256 `89a2415f…`, identical to the TMX copy |
| `logs/` | every search log, tolerance run and validation transcript |
| `tools/` | `an199`, `tapetool`, `probe199` sources |

---

## 10. Convergence

The last two arms were seeded from the final tapes and given 30 minutes each on
a converged incumbent:

| arm | seed | workers | evaluations | result |
|---|---|---|---|---|
| A7 (analog, flight window) | 49778 | 32 | 94 320 | **no improvement** |
| K8 (keyboard, flight window) | 51062 | 28 | 80 640 | **no improvement** |

Total for the campaign: ~800 000 oracle evaluations across 13 arms on 80 cores,
every banked improvement re-validated by the guard through the plain oracle at
the moment it was banked, and every surviving tape re-validated again at the end
from its durable copy (20 tapes, all exact, two human ghosts as controls in the
same batch). **Zero phantoms.**
