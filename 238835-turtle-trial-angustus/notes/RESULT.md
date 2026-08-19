# 238835 — `[Turtle Trial] Angustus` — AUTHOR TIME BEATEN by 3m 20s (43.2 %)

**uid** `KHmZyqOJ9oTOvTFIRkxGNTTK1D8` · author **Bald_tm** (BALDFROMSPB) · tags
**Trial, Turtle** · 5 checkpoints (4 intermediate + finish)

| run | time | vs AT |
|---|---|---|
| author time (AT) | 462.982 s | — |
| only human record (Quantiks, 1 of 1) | 1 964.933 s (32m 45s) | +1 501.951 s |
| **this work, validated** | **262.907 s** | **−200.075 s (−43.2 %)** |

All times are the plain oracle's (`TrackmaniaServer /nodaemon /validatepath=`)
against the untouched Nadeo-served `map.Map.Gbx`
(sha256 `57bcdd8c…78b0`), with the human record carried as a known-answer
control in every batch (1 964 933, every time).

---

## 1. THE HEADLINE FINDING: what a respawn actually does

Established empirically, not assumed. **Other trial-map agents should read this
section and nothing else if they are short of time.**

### 1.1 A respawn is IN THE INPUT TAPE and the oracle replays it

The human's 32-minute record re-simulates to **1 964 933 ms exactly** — and it
contains **198 respawn presses**. So:

* **respawns are expressible in the `.Ghost.Gbx` input bitstream**, and
* **`/validatepath=` accepts and re-simulates a run containing them.**

"`NbRespawns: 0` on every accepted run in this project" is a property of the
runs the project happened to feed it, **not a rule**. The validator prints
`NbRespawns` in *both* the `DeclaredResult` and the `ValidatedResult`, and
`IsValid` compares the two (so a tape whose respawn count differs from its
container's declaration is still *simulated* and still reports a
`ValidatedResult` — `IsValid: false` is not a DNF).

### 1.2 Where it lives in the bitstream

One packet per 10 ms tick. The packet's `word0` carries the input KIND, and
`word0 & 0xF` is the mode this project already decoded:

| `word0` | meaning | count in the human record | count in the author's own AT |
|---|---|---|---|
| `2` | ordinary vehicle packet | 196 296 | 46 430 |
| `34` = `0x22` (bit 5) | vehicle packet **+ respawn action** | 198 | 3 |
| `4098` = `0x1002` (bit 12) | a **second, different respawn action** | 0 | 17 |

Both bits produce a respawn; they are two different bound keys (see §1.4).
The existing `tmtraj`/`tmtas trace` decoders show only steer/gas/brake and are
blind to this, which is why the project had not seen it. `tmtas packets` (added
here) dumps `word0` and finds it in one command.

### 1.3 The two respawn kinds, measured from telemetry

The human's 198 presses produce exactly **198 position discontinuities**, of two
kinds:

**SOFT respawn — one press.** The car is restored to **the state it had when it
crossed the last checkpoint: position, speed AND attitude.** On this map that
means being put back at 62.4 km/h, **upside down** (roll −2.66 rad), because
that is how the checkpoint was crossed. Restores are bit-repeatable: every soft
respawn after CP2 puts the car at `(826.9, 99.1, 538.0)` at 62.4 km/h with the
same roll, forever.

**HARD respawn — two presses ~100–640 ms apart.** The second press converts it
into a placement **at the checkpoint block's own respawn transform, at a dead
standstill, perfectly upright and square**: `(843.2, 98.0, 528.0)`, v = 0, roll
= 0, pitch = 0, yaw = π/2 exactly, followed by a freeze of ~0.5–0.9 s. Also
bit-repeatable.

> **So the answer to "state at crossing, or standstill?" is BOTH, and the driver
> chooses.** One press keeps your speed and your attitude; a quick second press
> throws all of it away for a clean, upright, stationary start. On a turtle map,
> where the checkpoint is crossed inverted, the second press is usually the one
> you want — 91 of the human's 198 presses are the second half of a hard
> respawn.
>
> The validator counts a two-press hard respawn as **one** respawn
> (`NbRespawns: 4` for a tape of mine containing 6 presses = 2 soft + 2 hard).

### 1.4 The author uses a different key for the same thing

The author's own AT run (§4) contains **17 `word0=4098` presses and only 3
`word0=34`**, and *every one of its 20 respawns is a single-press teleport
straight to the standstill transform*. So `0x1002` is a **one-press hard
respawn** — a separately bound "respawn at standstill" key. The human is
double-tapping `0x22` to get what the author gets with one press of `0x1002`.

### 1.5 The consequence: the map decomposes exactly

Because a respawn's restored state is deterministic and history-independent, a
whole failed attempt can be **spliced out of the tape** and the run is unchanged
except for being shorter by exactly the deleted duration.

The first test predicted the answer before running it: delete ticks
28729–29404 (one failed attempt, 6 760 ms) from the human record →
**predicted 1 958 173, oracle returned 1 958 173.** Exact, first try.

That identity — *finish time = base − deleted duration, to the millisecond* — is
a **self-validating splice test**. Use it as the acceptance criterion for every
cut; any deviation means the two respawn states were not the same and the splice
is invalid. It caught every mistake made here.

**Caveat that cost real time (worth knowing):** splices are exact
*individually* but do **not** freely compose. Sixty-nine cuts that each
validated exactly, applied together, DNF'd. Two causes: (a) merging respawn
actions of *different kinds* (a 1-press soft into a 2-press hard) splices
incompatible states, and (b) the splice has a **phase**: the tail must start at
the same offset from the *first* press of the respawn action it is being grafted
onto. Fix: **one splice per obstacle cluster (first respawn action → last), and
sweep the deletion length by a few ticks until the arithmetic is exact.** After
that everything composed on the first attempt.

---

## 2. THE DIFFICULTY PROFILE — the respawn histogram

The single human record contains **106 respawn actions**. Where they cluster is
this map's difficulty profile, and it is extremely concentrated. Clustered by
the position at which each attempt died (single-link, 10 m):

| # | segment | where the attempt dies | attempts | wall time burned | share of the 32-min run |
|---|---|---|---|---|---|
| **1** | CP2→CP3 | **(974, 95, 518)** — 132 m past the respawn | **39** | **664 s** | **33.8 %** |
| 2 | CP3→CP4 | (1048, 135, 456) — 71 m | 8 | 177 s | 9.0 % |
| 3 | CP4→fin | (914, 96, 603) — 133 m | 12 | 140 s | 7.1 % |
| 4 | CP2→CP3 | (899, 89, 557) — 65 m | 18 | 121 s | 6.2 % |
| 5 | CP2→CP3 | (966, 89, 536) | 4 | 66 s | 3.4 % |
| 6 | CP3→CP4 | (1090, 135, 443) | 2 | 82 s | 4.1 % |
| — | 19 further spots | | 1–3 each | 3–70 s each | |

**Four places account for 55 % of a 32-minute run.** Obstacles 1 and 4 are the
same climb approached at two stages: 57 of the 70 attempts in CP2→CP3 die on
one feature.

Per segment, the human record splits as:

| segment | human elapsed | respawn actions | of which hard |
|---|---|---|---|
| start→CP1 | 58.9 s | **0** | — |
| CP1→CP2 | 200.8 s | 3 | 0 (all soft) |
| **CP2→CP3** | **1 072.6 s (17m 53s)** | **70** | **70** |
| CP3→CP4 | 372.0 s | 14 | 1 |
| CP4→finish | 260.7 s | 20 | 20 |

### What the obstacles are

Every death position has the car at **roll ±2.3 to ±3.1 rad — inverted — and
0–35 km/h**. That is the turtle signature: the car is driven on its roof, and
failure means *coming to rest upside down*, not falling off.

* **Obstacle 1 (974, 95, 518)** is the crest of an inverted climb: the car runs
  a corridor at y ≈ 89, then has to carry momentum up **8 m of vertical rise**
  (y 89 → 97) between z 540 and z 512 while upside down. 26 of the 70 attempts
  get within 3 m of the crest and **still** die there — they crest and slide
  back. Entry speed does **not** discriminate: the successful pass enters the
  ramp at 68.2 km/h, which is the *median* of the 47 measured entries
  (range 37–84). Whatever separates success here is line and attitude over the
  last two metres, not run-up speed.
* **Obstacle 4 (899, 89, 557)** is the entry to that same corridor, 65 m out.
* **Obstacle 3 (914, 96, 603)** is the map's real boss — see §4; it is what the
  *author* lost their run to.

---

## 3. HOW THE RUN WAS BUILT

No driving search was needed to beat the author time. The whole margin is
retries.

| step | what was done | time |
|---|---|---|
| 0 | the human record, as downloaded | 1 964 933 |
| 1 | splice out every failed attempt (5 splices, one per cluster, each length-swept until the arithmetic was exact) | **347 003** |
| 2 | **inject a respawn on the tick after each checkpoint fires**, so the human's own first failed attempt in each segment disappears too | **276 393** |
| 3 | delete slack ticks: 5.0 s where the car sits still at (1036, 137, 522), plus two smaller stalls | **268 554** |
| 4 | automated tape decimation (`tmdec`), plain-oracle scored, 14 rounds | **262 907** |

**Step 2 is worth spelling out.** A checkpoint's saved state exists on the very
next tick after the checkpoint fires — measured: injecting a respawn at tick
5891 works, at 5890 it does not, and CP1 fires at 58 906 ms. So you can cross a
checkpoint and *immediately* claim a clean standstill start, skipping the
approach the human wasted. For a hard respawn the second press must sit at the
right offset (k = 16 or 17 ticks here) so the grafted tail lines up with the
freeze. This alone was worth **70.6 s**.

**The author does exactly this.** Their AT contains a respawn 70 ms after
crossing CP2 and another 273 ms after crossing CP4. It is a known trial
technique, and it is the single most valuable thing in this write-up for a
human: *on a turtle-trial map, take the checkpoint however you can, then
immediately hard-respawn to start the hard part upright and stationary.*

### Tooling added (Rust, in `tmtas-rs2`)

* **`tmtas packets`** — dump every input packet with its `word0`, so respawns
  and any other non-vehicle action are visible. `--nonveh` filters to them.
* **`tmcut`** — arbitrary tape surgery: `del A:B`, `trunc`, `pad`, **`rsp T` /
  `norsp T`** (write or clear a respawn event at a tick), `set A:B:S:G:K`,
  `--start-offset`, `--field0`, and **`--inputs-from FILE`** — transplant
  another GBX's input archives into a working container, which is how an
  embedded author ghost becomes a tape the validator will replay.
* **`tmdec`** — the searcher this map actually wanted. Its operator is *delete
  ticks*, not *steer differently*: on a trial map the driver is at walking pace
  and time is spent, not lost. Every candidate is scored by the plain oracle;
  each round proposes one deletion at every (position, length) on a grid, then
  accepts a **ladder of prefix combinations in one parallel batch** (composition
  is not free, so "how many of these survive together" is the question, and it
  is one oracle batch, not a bisect).
* **`t38`** — analysis: `gaps` / `tele` (find respawns in telemetry),
  `clusters`, `attempts`, **`obstacles`** (the difficulty profile above),
  `approach`, `slow`, `chase`.

---

## 4. THE AUTHOR'S OWN AT IS ITSELF A RETRY RUN — and it is embedded in the map

The map header says `validated="1"`, and the author's author-time run is
embedded in the `.Map.Gbx`. `tmtraj decode map.Map.Gbx` reads it directly:
8 926 telemetry samples, splits **47 578 / 111 430 / 176 467 / 265 677 /
462 982**, matching `authortime="462982"` in the header.

Decoding its respawns gives the AT's own anatomy:

| the author's 462 982 ms | |
|---|---|
| clean driving to CP2 | 111.4 s |
| 2 failed attempts at obstacles 1 and 4, then a clean CP2→CP3 | 65.0 s |
| 1 failed attempt at obstacle 2, then a clean CP3→CP4 | 89.2 s |
| **14 failed attempts at obstacle 3 (914, 97, 604)**, then a clean CP4→finish | **197.3 s** |

**The author time is not a fast lap. It is a competent lap plus 19 crashes, and
~160 s of it — a third of the AT — is fourteen attempts at one obstacle.** The
same obstacle cost the only human record 140 s in 12 attempts. That is the
single most useful fact on this map for a driver: *if you can take (914, 97,
604) first time, you beat the author time with ordinary driving.*

Stripping the author's retries the way we stripped the human's would give a
clean author line of roughly **246 s**, so their *driving* is about 22 s better
than ours — almost all of it in segment 1 (author 47.6 s, ours 58.9 s; see §6).

### INCIDENT / caveat on the §9 embedded-ghost technique

**The embedded author ghost decodes but does NOT re-simulate.** Transplanted
into a working container (`tmcut --inputs-from`), its input archive round-trips
**byte-exactly** (20 250 bytes in, 20 250 out, `EXACT`), the chunk versions
match (4 and 4), the decoded steer/gas/brake agree with the ghost's own
telemetry — and the plain oracle still says `wrong simu`, failing **before
CP2**. It was not the container: the *human's* archive transplanted into a
*different* container reproduces 1 964 933 exactly, so the mechanism is proven
sound. Tried and rejected: `--raw` (byte-identical bitstream),
`start_offset` −1510 / −1500 / 0 / +10, dropping 0–170 leading countdown
packets, `field0` 2 574 490 → 0, and every combination.

So: **an embedded author ghost is a first-class source of telemetry, splits and
respawn structure — but do not assume it will validate.** Ours did not, and the
cause is still unknown (the container's `GameBuild` field belongs to the
container, not to the transplanted archive, so the recording build cannot be
read off a transplant).

---

## 5. FOR A HUMAN DRIVING THIS MAP

The map is not hard to *drive*; it is hard to *survive*. Four things, in order
of value:

1. **Hard-respawn the instant you take a checkpoint.** The saved state is live
   on the next tick. On a turtle map you cross checkpoints upside down, so a
   soft respawn (one press) hands you back a car on its roof at 60 km/h; the
   second press gives you the block's own respawn transform — upright, square,
   stationary. The author does this at CP2 and CP4. Worth 70 s on our tape and
   it is free.
2. **Learn (914, 97, 604) before anything else.** 12 human attempts, 14 author
   attempts, ~300 s burned between them. It is the last obstacle before the
   finish and it is where the author time was set — or rather, where it was
   lost.
3. **Then (974, 95, 518)**, the inverted climb in CP2→CP3: 39 of 70 attempts.
   Run-up speed is not the answer — the successful pass entered at the median
   68 km/h and plenty of faster entries failed. Everything is in the last two
   metres of the crest; more than a third of the attempts reach it and slide
   back.
4. **Nothing before CP1 matters much.** Zero respawns there in either run, and
   the whole segment is 48–59 s of ordinary driving.

---

## 6. WHERE OUR REMAINING TIME IS

Our 267 646 (v7) splits as **58.9 / 68.8 / 48.1 / 56.9 / 35.9 s**. Against the
author's *retry-stripped* driving (47.6 / 63.9 / ~43 / ~55.6 / 35.9 s) we are:

* **level** in the last segment,
* within a second or two in CP3→CP4,
* ~5 s down in each of CP1→CP2 and CP2→CP3,
* **11.3 s down in segment 1**, and it is localised: a `chase` of the two
  trajectories shows our line *ahead* by up to 2.75 s until 18 s, then losing
  8 s in one place — the human is nearly stationary (1.75 km/h for 3.0 s) at
  **(912, 104, 785)** where the author passes at 34 km/h. That is a physical
  stall, so tape deletion cannot fix it; it needs a driving search, and it is
  the obvious next 8 s on this map.

---

## 7. INTEGRITY

* Field-reproduction check (ACQUISITION.md §8): the field here is **one** record
  and it reproduces to the millisecond, **1/1**. It was re-validated as a
  known-answer control in every batch run for this map (dozens of batches).
* Every reported time is the plain oracle's on the untouched Nadeo map file.
* No fork-server or in-child surrogate score was used at any point for anything
  that is reported — `tmdec` scores every candidate through
  `/validatepath=` itself. (The cold-start workstream's warning about
  fork-reported finishes on self-derived tapes therefore does not apply here.)
* The run is legitimate: it is the human's own inputs with failed attempts
  removed. It skips no geometry, takes all 5 checkpoints in order
  (`NbCheckpoints: 5` in the `ValidatedResult`), and its respawns are the same
  mechanic the human and the author both use, in the same places.
* Nothing was submitted to any Nadeo leaderboard.

## 8. FILES

`~/persistent/private-30d/tm-unbeaten/238835/`

| file | what |
|---|---|
| `map.Map.Gbx` | the Nadeo-served map, sha256 `57bcdd8c…78b0` |
| `tapes/rank00001_1964933.Ghost.Gbx` | the only human record (control) |
| `tapes/TAS_407463_noretry.Ghost.Gbx` | step 1a — first tape under the AT |
| `tapes/TAS_347003_noretry_v4.Ghost.Gbx` | step 1 complete |
| `tapes/TAS_276393_v5.Ghost.Gbx` | step 2, checkpoint-respawn injection |
| `tapes/TAS_268554_v6.Ghost.Gbx` | step 3 |
| `tapes/TAS_267646_v7.Ghost.Gbx` | step 4, first decimation pass |
| `tapes/TAS_262907_v8.Ghost.Gbx` | **step 4 complete — the reported result, 262.907 s** |
| `ops_final_v5.txt`, `ops_final_v3.txt` | the `tmcut` op lists that build them |
| `analysis/` | difficulty profile, attempt tables, author decode |
| `tools/` | `tmcut.rs`, `tmdec.rs`, `tmtas packets`, `t38` |

---

## 9. FINAL NUMBERS (all times in seconds, as validated)

| tape | validated | vs AT |
|---|---|---|
| author time (AT) | 462.982 | — |
| only human record | 1 964.933 | +1 501.951 |
| `TAS_407463_noretry` | 407.463 | −55.519 |
| `TAS_347003_noretry_v4` | 347.003 | −115.979 |
| `TAS_276393_v5` | 276.393 | −186.589 |
| `TAS_268554_v6` | 268.554 | −194.428 |
| `TAS_267646_v7` | 267.646 | −195.336 |
| **`TAS_262907_v8`** | **262.907** | **−200.075 (−43.2 %)** |

Validation transcript: `VALIDATION.txt` — three cold passes over all seven
tapes in fresh processes (identical every pass), the human record carried as the
known-answer control in every batch (1 964.933 every time), plus a fourth pass
against a **re-downloaded, sha256-identical** Nadeo map file. sha256 of every
tape is in the same file.

**Positive control:** distinct from the identity control, and satisfied here by
construction — the evaluator is the unmodified plain oracle, seven different
tapes validate through it, and 363–853 of every ~11 800 `tmdec` candidates per
round return a real finish. It is not an instrument that can only say "no".

### Corrections adopted from sibling agents (2026-08-18, same evening)

* **Only the HARD respawn state is a per-checkpoint constant.** The SOFT state
  is *your own* crossing state, so respawn-anchored sectors cannot be optimised
  in parallel and recombined — work left to right. Cut-only work (all of this
  result) is unaffected: a cut changes nothing upstream of itself, which is
  exactly why `finish = base − deleted` stays valid.
* **Input tapes carry a dependency on their ghost container.** This is the
  leading explanation for §4's embedded-author-ghost failure, and it means the
  "control" quoted there was weaker than stated: the container I transplanted
  the human's archive into was itself derived from the human's own file, so it
  was a round-trip control, not a cross-container one. Getting an embedded
  author lap to run probably needs a container synthesised around it.
* A synthesised respawn press is **not** portable between checkpoints in
  general — it DNF'd at CP1 on another map. On 238835 it was exact at all four.
  Validate each one individually.

### The open item, and it is worth taking

The author's *retry-stripped* driving on this map is worth **≈246 s** against
our 262.907 s, and 11.3 s of that gap is one place in segment 1 where the human
sits at 1.75 km/h for 3.0 s and the author passes at 34 km/h. Deleting tape
cannot fix it (1 194 candidates, cut lengths to 3 s, **zero finishers** — it is
a physical stall). It needs either a driving search over ticks ~1 700–5 890, or
the author's own tape made to re-simulate.
