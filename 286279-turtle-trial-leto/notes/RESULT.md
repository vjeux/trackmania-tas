# [Turtle Trial] Leto — the author time falls by 38 %

**AT 355.181 s · human WR 441.002 s · validated TAS **218.812 s** — the author
time beaten by 136.369 s (38.4 %), and the human world record by 222.190 s.**

> **READ THE ADDENDUM AT THE END FIRST if you are here for the method.** The
> body of this document records the first result (235.625 s, built from the
> human world record) and one WRONG conclusion in section 1.5. The addendum
> corrects it, and the correction is what produced 218.812 s — built from the
> map author's own author-time ghost, recovered from inside the .Map.Gbx.

unbeaten.at MapId **286279**, uid `p0tVjdmb1DfkCVrDE_DfQN84kq8`, author
**BALDFROMSPB / Bald_tm** (one account, `5f6148ac-…`, who also holds the human
world record), tags **Trial, Turtle**, **5** recorded runs — all five downloaded
and analysed, plus the author's own author-time ghost recovered from inside the
map file. Nothing here has been or will be submitted to a Nadeo leaderboard.

| tape | validated | vs AT | vs human WR | steer alphabet | events |
|---|---|---|---|---|---|
| **`m286279_BEST_218812_v7`** | **218.812 s** | **−136.369** | −222.190 | **3 — keyboard** | 904 |
| `m286279_KEYBOARD_218877_v7` | 218.877 s | −136.304 | −222.125 | 3 — keyboard | 918 |
| `m286279_AUTHORCUT_220391_v6` | 220.391 s | −134.790 | −220.611 | 3 — keyboard | the author's own driving, cuts only |
| `m286279_TAS_235625_v3` | **235.625 s** | **−119.556** | −205.377 | 26 values | 964 |
| `m286279_KEYBOARD_235939_v4` | **235.939 s** | **−119.242** | −205.063 | **3 — keyboard** | **941** |
| `m286279_HUMAN_keyboard_236972_v1` | **236.972 s** | −118.209 | −204.030 | 3 — keyboard | 943 |
| — human WR, Bald_tm (control) | 441.002 s | +85.821 | — | 3 — keyboard | 1811 |

The top three came from the AUTHOR'S OWN author-time ghost (addendum); the
four below them from the human world record.

The last of those is the interesting one: **every input in it is Bald_tm's own,
unmodified, in his own order.** Nothing was driven better than the human world
record. Ten of his failed attempts were deleted.

---

## 1. Respawn semantics — the answer other trial-map agents need

Established empirically before anything was built on it.

### 1.1 The tape expresses respawn and the oracle simulates it

The ghost input chunk `0x0309201D` carries **one respawn bit per 10 ms tick** in
the packet's state word: `word0 & 0x20` = respawn, `word0 & 0x1000` = standing
respawn.

`TrackmaniaServer /nodaemon /validatepath=` **accepts and simulates them**, and
prints `NbRespawns` in both `DeclaredResult` and `ValidatedResult`. The human WR
carries 13 respawn ticks and re-validates to 441002 exactly; strip the bits and
the identical tape DNFs at cp1. **`NbRespawns: 0` everywhere else in this
project is a property of the runs fed to it, not a rule of the validator.**

Editing them is not free. The state word is emitted three ways by the codec
("same as previous", "previous + 2 flag bits", 33/34-bit literal) and only the
literal carries the respawn bits, so setting one means forcing a literal — **and
forcing every following packet to a literal too**, or the next "same as
previous" packet inherits the edited word and the car respawns on every tick for
ever. `mt setrespawn` / `make_explicit` in `tools/mt_main.rs` do this; the
identity control is that re-encoding the WR with every state word forced to a
literal returns 441002, and clearing then re-injecting its own CP1 respawn at
the same tick also returns 441002.

### 1.2 A normal respawn restores the state the car had WHEN IT CROSSED the checkpoint

Position, velocity **and** attitude — not a standstill, and not a canned
per-checkpoint state. Measured at the same checkpoint across five runs, each
returns to its **own** crossing state:

| run | respawn destination at CP1 | speed |
|---|---|---|
| rank 2 | (930.7, 45.4, 652.1) | **26.7** m/s |
| rank 3 | (929.8, 45.8, 647.7) | **22.2** m/s |
| rank 4 | (931.8, 38.9, 655.0) | **16.8** m/s |
| rank 5 | (929.9, 46.5, 648.3) | **23.4** m/s |

Within a run it is exact and repeatable: rank 1's ten sector-3 respawns, spread
over 165 seconds of race time, all put the car at (854.9 ± 0.4, 19.4 ± 0.3,
936.4) at 16.5–16.6 m/s, pitch −0.51 rad, and the trajectory that follows is
identical to the centimetre every time.

### 1.3 A standing respawn is a complete, canonical reset

Produced either by the explicit `0x1000` bit or by a **double tap** (respawn
again within ~150 ms). The car is placed at the checkpoint block's own spawn
transform at rest, and the state is **bit-identical for every run**:

```
CP1  (939.20, 38.00, 656.00)  yaw 1.5708  pitch -0.00006  roll -0.24491
CP3  (715.20, 18.00, 976.00)  yaw 1.5707  pitch -0.00005  roll -0.00005
     speed 0, gear 1, rpm 0, all four dampers -0.00784314, ground contact False
```

verified identical across runs 1/2/3/5 (CP1) and 1/3/5 (CP3) on all 29
telemetry columns. **The car is then FROZEN for ~800–850 ms** — position, speed
and attitude do not move even with the accelerator held — and only then
released. That freeze is the price of the reset, and it is why the human WR
loses 1.25 s at CP1 and 1.20 s at CP3 doing this deliberately.

### 1.4 Respawn is history-free — proved by a linearity sweep

Splice the tape as `WR[0 .. X) ++ WR[33557 .. end)`, where tick 33557 carries
the sector-3 respawn, and sweep X across a 200-second range:

| X | finish |
|---|---|
| 13169 (the CP2 crossing tick) | **237122** |
| 13200 | 237432 |
| 13500 | 240432 |
| 14000 | 245432 |
| 16000 | 265432 |
| 20000 | 305432 |
| 33000 | 435432 |

`finish = 237122 + 10·(X − 13169)`, exactly, every time. **However long the car
has been failing since the checkpoint, and wherever it is when the respawn
fires, the run afterwards is bit-identical.** The same holds at the CP3 standing
respawn (`WR[0..X) ++ WR[38227..end)` for X = 38212…38227 gives
441002 − 10·(38227 − X) exactly, so the whole 60-second sector 4 reproduces).

That is the decomposition the brief predicted, and it is what the result is
built on.

**But a CUT is safe and an OPTIMISATION upstream is not.** The restored state
is the run's OWN crossing state, so deleting ticks that lie entirely after the
crossing changes nothing the respawn depends on — which is why the arithmetic
above is exact. Change anything BEFORE the checkpoint and the crossing state
moves with it, and every input after the respawn was tuned for the old one.
Respawn-anchored sectors are therefore NOT independent and cannot be optimised
in parallel and recombined: work left to right, and re-derive everything
downstream of a change. (Independently measured on map 284238 by another agent,
where a sector optimised to cross CP4 95 ms earlier made the unchanged tail
DNF.) Every search arm here was confined to ticks >= 20500, entirely inside
sector 4 and downstream of the CP3 respawn, so nothing in this result is
exposed to it.

### 1.5 THE HARD LIMIT: input tapes are NOT portable between ghost containers

**This is the most important negative finding here, and it is not about
respawns at all.**

| tape | container | result |
|---|---|---|
| rank 2's input archive | rank 2's own ghost file | **977690 — exact** |
| rank 4's input archive | rank 4's own ghost file | **1371430 — exact** |
| rank 2's input archive | rank 1's ghost file | **DNF, cp 1** |
| rank 4's input archive | rank 1's ghost file | **DNF, cp 1** |
| the author's AT archive (from the map) | rank 1's ghost file | **DNF, cp 1** |

The transplant machinery is provably correct (the two identity rows). Copying
the archive's `start_offset_ms` alignment does not help, and neither does
copying **all fourteen** of the small `0x03092xxx` ghost chunks
(`…08, 0A, 13, 14, 1A, 22, 23, 24, 25, 26, 27, 29, 2A, 2C`) from the donor. The
carrier is therefore one of the large ones — `0x03092000` (the recorded
`CPlugEntRecordData` samples), `0x0309202D`, or `0x0309202E`, which is 4 bytes
in rank 1 and 69 in rank 2.

**Consequence for the fleet: a "best-of-field" splice is impossible.** Every
cross-run splice I tried failed — rank 2's sector 1 onto rank 1, rank 2/3/5's
sector 4 onto rank 1's canonical CP3 standing spawn, at seven tick offsets and
with the source run's own respawn pair — and this is why, not anything to do
with respawn state. Splice **within one ghost file** only.

### 1.6 One measured anomaly, unexplained

Deleting the ticks between a checkpoint crossing and the standing respawn that
follows it works perfectly at **CP3** (15 ticks deleted → exactly −150 ms, the
rest of the run bit-identical) and fails at **CP1** for *every* deletion tried:
1, 2, 4, 8, 12, 16, 20, 22 and 24 ticks, deleted as a prefix, a suffix or a
middle of the interval — all DNF. Inserting idle ticks before the CP1 respawn
fails too, at all of 1…100 ticks, so it is not an absolute-time period either.
Same container, same canonical destination, opposite outcome. Recorded here so
the next agent does not spend the hour I did on it.

---

## 2. How the run was built

The world record's own tape, with its ten failed sector-3 attempts deleted and
the CP3 dead ticks trimmed:

```
WR[0 .. 13169) ++ WR[33557 .. 38212) ++ WR[38227 .. end)
```

* tick 13168 is the CP2 crossing (race 130162 ms);
* tick 33557 is the respawn that began the one sector-3 attempt that succeeded;
* ticks 38212…38226 are the 150 ms between the CP3 crossing and its standing
  respawn.

The first join is exact **by construction**: that respawn restores the CP2
crossing state, which is precisely the state at the end of the first segment.
No injected respawn is needed — and injecting one at the join DNFs.

`237122 ms` on the first try (predicted beforehand as
130170 + (441002 − 334050) = 237122; measured 237122 — the prediction being
exact *is* the proof that respawn restores the crossing state), `236972` with
the CP3 trim, and `235625` after a mutation search over the final
sector.

---

## 3. What the map actually is

3316 m, three checkpoints, ~237 s clean. **The car spends 154.5 s of 235.3 s —
66 % of the run — driving upside down**, and that is not a mistake, it is the
map. "Turtle" here means the car is deliberately flipped onto its roof and
driven there, rocking between roll +2.4 and −2.9 rad at 6–15 m/s with wheels
leaving the ground on each swing.

| phase | race time | attitude | what happens |
|---|---|---|---|
| A | 0 → 11.3 s | upright | the only genuinely fast part: 446 m at 44.6 m/s, a long jump up to y = 67 |
| B | 11.3 → 41.3 s | **inverted 30 s** | first turtle section, on the y ≈ 65 deck |
| C | 41.3 → 53.2 s | upright | drop to y ≈ 39; **CP1 at 45.6 s**; standing respawn; blast to 36 m/s |
| D | 53.2 → 117.9 s | **inverted 58 s** | the long one, y ≈ 35, a loop out to x = 1105 and back |
| E | 117.9 → 134.1 s | upright | descend to the map's low point y ≈ 9; **CP2 at 130.2 s**; accelerate to 28 m/s and flip |
| F | 134.1 → 172.4 s | **inverted 38 s** | third turtle section, y ≈ 25, the z ≈ 1056 corridor |
| G | 172.4 → 187.7 s | upright | **CP3 at 176.6 s**; standing respawn; 58 m/s run and a launch to y = 74 |
| H | 187.7 → 203.2 s | upright | the high deck |
| I | 203.2 → 225.6 s | **inverted 22 s** | final turtle section, descending y 65 → 51 |
| J | 225.6 → finish | upright | **flip back onto the wheels**, crawl to the booster, 66 m/s to the line |

Only 37.6 s of the run is above 20 m/s; 94.7 s is 10–20 m/s and 98.2 s is under
10.

---

## 4. Where the field dies

Across the five recorded runs: **272 failed attempts costing 4 607 170 ms
(76.8 minutes) of race time**, in **41 obstacle clusters**
(`analysis/obstacles.txt`).

| sector | failed attempts | race time lost |
|---|---|---|
| 1 | **0 — nobody has ever failed sector 1** | 0 |
| 2 | 110 | 1 686 220 ms |
| 3 | 100 | 1 709 940 ms |
| 4 | 62 | 1 211 010 ms |

Per run — this is the whole leaderboard:

| run | time | failed attempts (S1/S2/S3/S4) |
|---|---|---|
| rank 1 Bald_tm | 441002 | 0 / 0 / **10** / 0 |
| rank 2 Quantiks | 977690 | 0 / 4 / 18 / 18 |
| rank 3 Ta__Da | 1271692 | 0 / 9 / 42 / 1 |
| rank 4 Schmaniol | 1371430 | 0 / **45** / 2 / 5 |
| rank 5 Max_heyu | 1961645 | 0 / 52 / 28 / 38 |
| — the AUTHOR's own AT run | 355181 | 0 / 0 / **9** / 0 |

**The leaderboard is a ranking of how few times you failed, not of how fast you
drove.** No two runs differ meaningfully in speed while the car is moving.

### The ten obstacles that cost the field the most

`prog_m` = metres along the clean line, `cleanT` = when a clean run gets there,
`roll` = mean magnitude at the moment of failure (π = fully inverted).

| # | sec | prog_m | cleanT | field ms lost | tries | where | speed | roll |
|---|---|---|---|---|---|---|---|---|
| 1 | 3 | 2207 | 159.9 s | **606 350** | 17 | (769.9, 22.6, 1055.9) | 10.0 | 2.41 |
| 2 | 2 | 1574 | 99.8 s | 486 280 | 9 | (956.7, 33.7, 769.4) | 5.5 | 2.41 |
| 3 | 3 | 2028 | 140.6 s | 364 220 | 29 | (895.2, 25.9, 1046.8) | 8.4 | 2.57 |
| 4 | 3 | 2321 | 170.4 s | 338 370 | 7 | (749.3, 18.9, 1022.1) | 12.2 | 1.73 |
| 5 | 3 | 1971 | 134.8 s | 216 460 | **40** | (867.6, 26.8, 1006.9) | 6.9 | 2.39 |
| 6 | 2 | 1105 | 54.8 s | 214 570 | 24 | (1083.0, 41.2, 609.6) | 6.6 | 1.69 |
| 7 | 2 | 1045 | 52.4 s | 187 990 | **30** | (1055.5, 26.4, 578.5) | **43.0** | 0.26 |
| 8 | 4 | 3160 | 214.1 s | 178 120 | 4 | (572.1, 48.8, 595.2) | 11.6 | 1.89 |
| 9 | 4 | 2892 | 189.3 s | 147 680 | 12 | (639.6, 69.5, 709.8) | 20.2 | 2.05 |
| 10 | 3 | 2101 | 149.0 s | 129 030 | 6 | (862.1, 26.2, 1058.1) | 8.3 | 2.55 |

Two kinds of death, wanting opposite things:

* **roll 2.4–2.9 rad — eight of the top ten.** The car is on its roof and the
  failure is *losing the roof*: it rocks past the balance point and lands back
  on its wheels where there is no road under wheels, or rocks the other way and
  drops off the edge. All at 5–12 m/s. These are **slow-speed balance failures,
  not crashes**.
* **roll 0.26 at 43 m/s — obstacle 7, 30 attempts.** The only fast one: upright,
  quick, and it misses a landing.

**The author's own nine failures are all in sector 3, and five of them are at
(895, 26–28, 1033–1049) — obstacle #3.** The best player on the map fails at the
same place as everyone else.

---

## 5. What the successful approach looks like

### Obstacle 5 — the flip-in (40 attempts, the most-attempted spot on the map)

Race 132.6–135.0 s, (857 → 869, y 18 → 27, z 973 → 1008), just after CP2. This
is where you *deliberately turn the car over*, and it is the skill the map is
built around.

```
132.6 s  (856.9, 18.7,  972.9)  28.8 m/s  pitch 0.23  roll 0.07  gas 1   hit the ramp square and flat, full throttle
133.0 s  (860.1, 23.3,  983.0)  27.4       pitch 0.53  roll 0.22  gas 0   RELEASE at the crest, nose 30 deg up
133.4 s  (862.8, 27.2,  990.9)  19.4       pitch 0.42  roll 1.14  gas 0   airborne, rolling over
133.8 s  (865.4, 27.3,  997.3)  15.4       pitch 0.18  roll 1.77  steer +0.80   feed the roll in
134.2 s  (867.4, 27.2, 1002.0)  12.1       pitch -0.10 roll 2.30  steer +1.00   full lock as the nose drops
134.4 s  (868.4, 27.1, 1004.0)   9.8                   roll 2.74  ground        land on the roof
134.8 s  (869.4, 27.1, 1006.8)   5.3                   roll -2.55               settled; now drive
```

The recipe: **arrive at 28–29 m/s dead flat (roll < 0.1), full throttle to the
crest, release the throttle exactly at the crest, then feed progressively more
steering lock into the air phase** — 0.80 of lock at pitch 0.18, full lock as
the nose comes down — and land on the roof at ~10 m/s. Throttle stays off from
the crest until the car has settled.

Both hard parts are in the first 0.4 s: the entry has to be square, because any
roll at the ramp becomes a bad landing 1.5 s later and there is no correction
available in the air; and the throttle release is what sets the pitch. Note
that the 29 failures at obstacle 3 are **48.5 m past this flip** — the field
mostly lands on the roof and then loses it, rather than failing the flip itself.

### The inverted crawl — obstacles 1, 3, 10 (52 attempts between them)

Once you are on the roof the technique is a rocking oscillation: roll swinging
between +2.4 and −2.9 with the wheels leaving the ground at each swing. **The
successful pattern is a steady rhythm, not a hold** — full lock one way for
0.4–0.6 s, neutral through the rock, full lock the other way, throttle pulsed on
the down-swing and off through the inversion — and the roll magnitude never
drops below about 2.3. Dropping below that is the car starting to come back onto
its wheels, and that is exactly what the 52 failures look like.

Through the z ≈ 1056 corridor (obstacle 1 — the single most expensive spot on
the map, 606 s of field time) the clean run holds **full left lock almost
continuously from 160.1 s to 162.7 s** and reaches 16.0 m/s, the fastest
sustained inverted speed anywhere in the run. All 17 failures there are at
10 m/s. **Speed is what keeps you on the roof**; creeping is what tips you over.
That is the single most useful sentence in this document for a driver.

### The flip-back — the last obstacle, and the 4 s the TAS still loses

Race 225.6–231.5 s at (608, 51, 538). The car arrives inverted at 9.5 m/s, the
roll unwinds from −2.95 to −0.6 over about a second, and then the run spends
**4.0 seconds rocking on the spot under 3 m/s** before it gets going again to
the finishing booster. That is the largest single piece of dead time left, and
it is where runs 2, 4 and 5 lost 83.8 s, 64.3 s and 59.2 s on single attempts.
The mutation search in §7 recovered 1347 ms of it and it is
where any further work should go.

### The two standing respawns are a technique, not a mistake

At CP1 and CP3 the world record crosses at 25 and 35 m/s, respawns ~250 ms
later, and sits frozen for ~850 ms. It looks like waste. It is not: the
alternative is braking to a controlled standstill, which costs more, and the
standing respawn hands you a **perfectly known** entry state — square, level,
stationary — for a section where attitude is everything. Every run on the
leaderboard does it. Copy it.

---

## 6. The drivable tape: §B is satisfied by construction

The usual §B question — how to trade TAS precision for something a human can
execute — barely arises here.

`m286279_HUMAN_keyboard_236972_v1.Ghost.Gbx` is **236.972 s of Bald_tm's own
inputs**, in his own order, at his own resolution: three steer values
(`left / 0 / right` — the human world record is a pure keyboard run), 943 input
change events, and **not one tick of TAS mutation**. The only edit is the
deletion of 10 711 ticks in which he was failing.

So the deliverable to the field is not a technique. It is this:

> **Bald_tm has already driven a 237-second lap of this map. He drove it in
> eleven pieces, over seven and a half minutes. Every piece of a sub-four-minute
> run exists inside his own recorded world record.**

`m286279_KEYBOARD_235939_v4` is the same thing after a keyboard-constrained
search over the final sector — steer snapped onto `{−127, 0, +127}` before every
evaluation — **235.939 s, still exactly three steer values, 941 events**, two
fewer than the human tape it came from. `m286279_TAS_235625_v3` lifts the
alphabet restriction and reaches 235.625 s with 26 values and 964 events.

**Keyboard costs 314 ms out of 236 seconds — 0.13 %.** On this map the input
device is irrelevant; only failing is expensive.

### A defect worth carrying: a constraint that silently does not bite

`--quant` in the hardened build is fork-path only. Adding it to the classic
path, I got it wrong twice, and both mistakes are the same shape as the phantom
problem inverted — an instrument that quietly reports success it has not earned:

1. **I patched `run_dump`, not the search loop in `main()`.** Both contain a
   near-identical `mutate_ctx` → `fac.apply` sequence. The constraint did
   nothing for 140 000 candidates and produced a "keyboard" tape with 150+
   distinct steer values (`m286279_TAS_analog26_235814_v3`, kept as the
   specimen — it is a *valid* 235.814 s tape, it is just not keyboard).
2. Even in the right place, snapping only `[lo, hi)` **leaks**: `retime`/`shift`
   moves values from beyond `hi` into the window, so the ladder must be applied
   to the **whole steer array** after mutation.

**The 90-second control that catches both**, and which should be run against
any new alphabet constraint: search with a **one-level ladder** (`--quant 0`).
Every steer becomes zero, the car drives straight, and a healthy constraint
gives `finish 0%` and no improvement. Before the fix this printed `finish 64%`
and a new best; after it, `finish 0%`, best unchanged at the incumbent. Paired
with the ordinary identity run (no ladder → the template's own 237.122 s) that
pins the instrument from both sides: it can say yes, and the constraint really
bites.

---
## 7. And the author's own run says the same thing, louder

The map header says `validated="1"`, and **the author's author-time ghost is
embedded in the `.Map.Gbx`** (ACQUISITION §9). It decodes: 6918 samples, splits
`[42036, 122182, 302761, 355181]`, and **eleven respawns**. Decomposed the same
way:

| | ms |
|---|---|
| S1, start → CP1, clean | 42 036 |
| CP1 → standing respawn | 264 |
| S2, respawn → CP2, clean, zero respawns | 79 882 |
| **S3 — nine failed attempts** | **134 618 — WASTED** |
| S3, the attempt that worked | 45 961 |
| CP3 → standing respawn | 339 |
| S4, respawn → finish, clean | 52 081 |
| **AT as recorded** | **355 181** |
| **AT minus its own failed attempts** | **220 563** |

Two things follow.

1. **The author time is a genuine driven lap, not a plugin fabrication.**
   unbeaten.at flags it `inPlugin: true`, but a fabricated AT does not contain
   nine failed attempts at the same obstacle everybody else fails at. 135 of its
   355 seconds are retries.
2. **220 563 ms is the next target**, and it is the same driver's own driving.
   Their S1 (42 036 vs 45 597), S2 (79 882 vs 84 322) and S4 (52 081 vs 60 409)
   are all faster than the world record's — 16.2 s of it — while their winning
   S3 (45 961) and the WR's (46 543) agree to within 0.6 s.

That tape could not be re-simulated here: it is a different ghost container, and
§1.5 is exactly why. Recovering it needs the container carrier identified
(`0x03092000` / `0x0309202D` / `0x0309202E`) or the embedded ghost rebuilt as a
loadable `.Ghost.Gbx` — the extracted body is byte-correct
(`map body [242, 193679)`, chunk-for-chunk identical in structure to a real
ghost) but the game will not load it with a synthesised header, at any
`num_nodes` from 1 to 100 000, with either file's reference table.

**Why the search only works at the end of the tape.** A mutation anywhere before
the last ~15 seconds perturbs a chaotically sensitive inverted crawl and the
rest of the run diverges; arms aimed at sectors 1–3 produced nothing in 20
minutes each, while arms on the last 1700 ticks produced steadily. The `shift`
operator (retime the tail) was the most productive single operator, which fits:
the dead time there is a stall, and what a stall wants is to be shortened.

---

## 8. Correctness

* **Field reproduction (ACQUISITION §8): 5 / 5 exact** — 441002, 977690,
  1271692, 1371430, 1961645, every one to the millisecond. The map is healthy
  for this oracle (ghosts recorded on `git 128149`, oracle `git 128182`).
* The map is Nadeo's own file, md5 `0e6e5b7366feed92e93c6d4d4dff25c2`,
  1 497 655 bytes; every number here was produced against it.
* **Tape-editor identity controls**, run before any edit was trusted: the WR
  re-encoded with every state word forced to a literal → 441002; its CP1
  respawn cleared and re-injected at the same tick → 441002; the WR's inputs
  transplanted into its own container → 441002; rank 2's and rank 4's into
  their own → 977690 and 1371430.
* **Cold validation of the headline tapes: three passes, fresh throwaway
  directory, fresh server process each time, against a RE-DOWNLOADED
  byte-identical copy of the map, with the human world record as a
  known-answer control in every batch.** 235625 x3, 235939 x2, 235814 x3,
  236972 x3, control 441002 every pass, all `"IsValid": true`,
  `"NbRespawns": 3`.
* Every reported tape was re-validated through the plain oracle against the
  untouched map with the human WR as a known-answer control in the batch.
  No phantom was produced or banked. The hardened build's
  re-validate-every-improvement guard was on throughout and never fired; each
  search arm had its own `--root`.
* The map has **4 checkpoints**, so neither the `FINISH_BASE` DNF-shaping defect
  (bites at 11 CPs) nor the phantom-guard misfire (6 CPs) applies here.
* The delivered tapes carry a corrected `DeclaredResult`, so the validator
  reports **`IsValid: true`** with `NbRespawns: 3`. The declared summary lives
  in chunk **`0x0309201B`** — `u32 version | u32 nbCheckpoints | u16 nbRespawns
  | u32 time | u32 sector_time[nbCheckpoints]` — with `0x0309202B` the
  cumulative split list and `0x03092005` the race time; all three must agree and
  the walltime field must be fixed as well, or the validator answers
  `unexcepted walltime`.
* **Legitimacy.** The run crosses all three checkpoints in order and the finish.
  It is the human world record holder's own input stream. No geometry is
  skipped, nothing goes out of bounds, and the three respawns it uses are three
  of the thirteen he used himself — on a Trial map respawning is the intended
  and only recovery mechanic.

## 9. Files

```
tapes/    m286279_HUMAN_keyboard_236972_v1.Ghost.Gbx  + .tick.txt   pure human, pure keyboard
          m286279_TAS_235625_v3.Ghost.Gbx              + .tick.txt   the floor
          m286279_KEYBOARD_235939_v4.Ghost.Gbx        + .tick.txt   keyboard only
          m286279_TAS_analog26_235814_v3.Ghost.Gbx    + .tick.txt   see §6 warning
          m286279_TAS_237122_v1 / _236554_v2.Ghost.Gbx              the lineage
          AUTHOR_AT_355181_inputs_from_map.Ghost.Gbx                decodes, does NOT re-simulate
ghosts/   the five human leaderboard ghosts as downloaded
analysis/ obstacles.txt   41 obstacle clusters ranked by field time lost
          attempts_all.txt  every attempt of every run, with entry/exit state
          sectors.txt     each run's successful traversal per sector
          respawns.txt    every respawn event with its destination state
          best237122.csv  per-tick telemetry of the clean run
          AUTHOR_AT_355181.csv  the author's own AT run, decoded from the map
tools/    mt_main.rs      the tape surgery used here (Rust; splice, setrespawn,
                          clearrespawn, insert, pad, declare, transplant,
                          extract, attempts, deaths, obstacles, sectors)
```

---

# ADDENDUM (same session, two hours later): 218.812 s — the AT falls by 38 %

Everything above is correct and still validates. It is also **superseded**, by a
finding that began as a correction to my own §1.5.

## A1. §1.5 was WRONG. Input tapes ARE portable — two small chunks carry it

The bisection §1.5 asks for costs three validations, and I should have run it
before writing that section. Transplanting rank 2's input archive into rank 1's
container, one candidate chunk at a time:

| carried with the archive | result |
|---|---|
| nothing | DNF cp1 |
| `0x0309202E` (the 4-vs-69-byte suspect I named) | DNF cp1 |
| `0x03092000` (the recorded telemetry) | DNF cp1 |
| `0x0309202D` | DNF **cp2** — progress |
| **`0x0309202D` + `0x0309202B`** | **EXACT** |

`0x0309202B` is the checkpoint-split list; `0x0309202D` is 209 bytes and **the
same size in every ghost on this map** — which is exactly why the equal-size
`copychunk` sweep in §1.5 missed it. I enumerated fourteen chunks by hand and
the answer was two I had not listed.

**The general lesson is worth more than the fact: a negative from a
hand-enumerated list is worth nothing.** Bisect.

With that pair carried, every foreign tape re-simulates exactly in rank 1's
container — 977690, 1271692, 1371430, 1961645 — and so does **the author's own
author-time ghost, extracted from the map: 355181, exact.** The §7 "third §9
outcome" (decodes but will not validate) was never a property of the ghost; it
was two missing chunks.

> **CAVEAT for anyone reusing a transplanted tape: `0x03092000` stays the HOST's.**
> The recorded telemetry in the container is the host ghost's, not the
> simulation the transplanted inputs produce, so `tmtraj decode` on such a file
> describes the wrong run. Decode the donor for telemetry; use the transplant
> only to obtain a time.

## A2. Which unlocked the author's own run — and it is much faster driving

`tapes/AUTHOR_AT_355181_PLAYABLE.Ghost.Gbx` re-simulates to 355181. Its clean
sectors are faster than the world record holder's in all four:

| sector | the AUTHOR | human WR | delta |
|---|---|---|---|
| S1 start → CP1 | **42.036 s** | 45.597 | −3.561 |
| S2 → CP2 | **79.882 s** | 84.322 | −4.440 |
| S3 → CP3 (each one's successful attempt) | **45.961 s** | 46.543 | −0.582 |
| S4 → finish | **52.081 s** | 60.409 | **−8.328** |

Cutting their nine failed attempts out, then trimming the dead ticks at each of
the three respawns, then a 25-minute search on the final sector:

| tape | validated | vs AT 355.181 |
|---|---|---|
| author's AT as recorded | 355.181 s | — |
| cut of the nine failed attempts | 220.821 s | −134.360 |
| + CP3 trim | 220.511 s | −134.670 |
| + CP1 trim (`m286279_AUTHORCUT_220391_v6`) | 220.391 s | −134.790 |
| **+ search on the last sector (`m286279_BEST_218812_v7`)** | **218.812 s** | **−136.369 (38.4 %)** |
| keyboard-constrained arm (`m286279_KEYBOARD_218877_v7`) | 218.877 s | −136.304 |

Cold-validated three times, fresh processes, re-downloaded byte-identical map,
human WR as control in every batch, all `IsValid: true`, `NbRespawns: 3`.

## A3. The two respawn rules the cutting produced

**A3.1 — You can cut TO a soft respawn but NOT to a standing one.** This, and
not the checkpoint index, is what "CP1 is special" has been on three maps across
three agents. rank 1's CP3 trim that worked lands on a soft respawn; its CP1
trim that never worked, at every deletion length 1…24 and every insertion
1…100, lands on a standing one. Same on the author's tape: the cut to its single
soft respawn worked first try.

**A3.2 — A cut to a standing respawn works at ONE EXACT PHASE, and the phase is
not periodic.** Holding the graft at the respawn press and sliding the cut
point:

```
ticks 12376…12398   DNF
tick  12399         WORKS  -> 220821
tick  12400         WORKS  -> 220831
ticks 12401…12499   DNF
tick  12500         WORKS  -> 221831
```

Once on phase the arithmetic is exact again: `220821 + 10·(X − 12399)`.
**Sweep the CUT POINT, not the graft point** — I swept the graft first, got
fifteen DNFs, and wrongly concluded a standing respawn could not be cut to at
all. The same sweep at the other two respawns then paid 310 ms and 120 ms.

## A4. The unconstrained search chose the keyboard alphabet by itself

The final search arm was given the whole ±127 steering range, 100 080
evaluations over 25 minutes, no ladder. Its winning tape's steer alphabet:

```
{left, 0, right}
```

The keyboard-constrained arm, run in parallel, finished 65 ms behind — the
constraint costing nothing, the arm simply converging slightly worse.

So the cost of restricting this map to a keyboard is not merely small: **an
unconstrained optimiser, free to use 254 steering values, declines to.** Every
human tape on the map is 3-valued too — the author's AT ghost, the world record,
and our best. Analog steering is not where the time is on a low-speed technical
map, and this is the strongest form of that evidence in the project so far.

## A5. What is left

* **72 % of the run is still driven upside down**, and 9.45 s of it is still
  under 3 m/s. The 4.0 s flip-back stall at (608, 51, 538) survives into this
  lineage.
* The three respawns are structural and each hard one costs an ~850 ms freeze
  that no input can shorten.
* The theoretical floor for pure cutting on this lineage was 220.563 s
  (the author's AT minus its own retries); we are 1.75 s under it, all of it
  from the last sector.
* Sector-by-sector search on S1–S3 remains unproductive for the reason in §7:
  perturbing a chaotically sensitive inverted crawl diverges everything after
  it.
