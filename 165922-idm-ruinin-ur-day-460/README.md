# idm ruinin ur day #460 — the author time falls, and a respawn was the crowbar

**Author time 15.643 · the only human record 8790.769 · best validated 15.217.**

| tape | validated | vs AT | what it is |
|---|---|---|---|
| [`TAS_15217_clean`](replays/TAS_15217_clean.Ghost.Gbx) | **15.217** | **−0.426** | the best run on this map — **and its steering is keyboard-only from race 4.56 s** |
| [`TAS_15224_analog`](replays/TAS_15224_analog.Ghost.Gbx) | 15.224 | −0.419 | the pure-analog champion, which the keyboard tape beats |
| [`TAS_15285_keyboard`](replays/TAS_15285_keyboard.Ghost.Gbx) | **15.285** | −0.358 | keyboard from race 2.56 s — **70 input events in the whole run** |
| [`TAS_15382_deep_landing`](replays/TAS_15382_deep_landing.Ghost.Gbx) | 15.382 | −0.261 | forced to land 40–80 m deeper into the pad |
| [`KEYBOARD_16276_tolerant`](replays/KEYBOARD_16276_tolerant.Ghost.Gbx) | 16.276 | +0.633 | **over the author time on purpose** — a coarse keyboard launch, 10 % tolerant where every fast tape is 0 % |
| author time | 15.643 | — | — |
| human record, wschseng *(control)* | 8790.769 | — | 2 h 26 m — see below |

TMX map [165922](https://trackmania.exchange/maps/165922) · uid
`mP8HzG68YxUY6yJcrQFx2inUjtk` · **one recorded run**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

Every tape above carries **zero respawn packets** — audited, not assumed, by
enumerating bit 31 of each input packet's state literal. The human record, run
through the same audit as a positive control, reports **941**.

---

## The leaderboard is one run, and it is not a lap

The single human record on this map reads 2 hours 26 minutes. It is not a slow
lap. It is **one session of 930 attempts and 941 respawn presses** with the clock
running through all of them, and the driver never converted one: their best
attempt landed **45 m short of the first gate row** and spent its last **3.77 s
crawling** into a gate. Everything after that is a retry.

So the classification is **known-but-unheld**. Nobody needs to discover the
technique — one person performed it 930 times in a single sitting. What they
never put together is the two things the author time needs: a clean launch, and
a landing that reaches the pad instead of stopping short of it.

## The map in one paragraph

You start on a platform 1.88 km up, drop through a narrow chute, cross the map's
one checkpoint at about 1.7 s inside the start structure, ride a short booster
ramp (all of it inside x ∈ [409, 690]) and leave it at ~5 s doing 180–200 m/s.
Then there is **nothing** for 1.9 km: an unpowered ballistic glide onto a pad of
132 finish gates on the ground, 88 m × 352 m. The map is a launch and a fall.

## Three things a player can use tonight

**The glide needs no input at all.** Force steering to zero from a given moment
to the end of the fastest tape:

| steer zeroed from | finish |
|---|---|
| race 4.50 s | DNF |
| race 5.50 s | 15.276 (+0.052) |
| race 6.46 s | 15.231 (+0.007) |
| race 8.46 s onward | 15.225 (+0.001) |

**Nine of the 15.2 seconds need no steering.** That is exactly what the human
does: after ~6.4 s their tape is `steer 0, gas held` and never changes again.
Holding the throttle from 4.46 s to the finish is likewise completely free.

**The board's only human plays on a keyboard, and so does our best tape.**
Across all 879 231 ticks of the record, 94.2 % of steering values are exactly
{0, −127, +127}; the winning attempt is 102 input events with the gas held
throughout and one 20 ms brake tap — full lock right 2.7 s, full lock left 1.2 s,
full right, then nothing.

**Land on the pad, long rather than short.** The only finisher on this board lost
3.77 s crawling the last 45 m. Our tape crosses the pad's near edge at the exact
millisecond it finishes.

## The first three seconds decide it — and the human's version is forgiving

This is the most useful measurement on the map, and it needs to be read per
region rather than as one number.

Move one input change by a single tick (10 ms), earlier or later, and
re-simulate. Over the whole run of the 15.224 tape: **1 338 shifts, 1 261 survive
— 94.2 %.** But they are not evenly spread:

| window | shifts | still finish |
|---|---|---|
| race 0.00–2.96 s | 52 | **0** |
| race 2.96–3.96 s | 30 | 9 |
| race 3.96–4.96 s | 54 | 50 |
| after race 4.96 s | 1 202 | **1 202** |

Same shape on all four of our tapes. Inside that opening window the wall is
absolute: **0 of 1 300 two-boundary pairs** survive either.

So the tape is **precision-bound in one two-second window and forgiving
everywhere else** — and the shifted runs get down the chute perfectly well
(52 of 52) before crashing on the **booster ramp**. The sensitive thing is the
ramp entry and the boost sequence, not the chute walls and not the aim.

**But that is a fact about a tape, not about a driver.** Run the same instrument
on the human's own winning attempt:

> **42 boundary shifts tested, 17 survive — 40.5 %.** (Three of them are
> *faster*; one returns 8787.643.)

A launch program with real one-tick tolerance **exists on this map** — theirs —
and none of our fast tapes has any. And tolerance is partly buyable: a coarse
keyboard launch is **10 % tolerant at 16.276**, so we have three points on the
curve — 15.2 s at 0 %, 16.3 s at 10 %, the human's 18.8 s attempt at 40.5 %.

> **The forgiving program exists, and it costs about a second.** The author's
> 15.643 sits between our fast line and that forgiving one, which is exactly
> where a driven validation lap should sit.

That also says what to practise: the chute is a **feel** section — do not try to
memorise a millisecond-exact pattern, ours does not even transfer to itself. The
ramp entry is where the run is won. After that, hands off.

## Low input: the constraint found time rather than costing it

Straight quantisation DNFs here, and a whole-tape constraint from a DNF seed has
no gradient. So the alphabet was applied **under search**, through a windowed
ladder that grows backward from the finish — the chute is the fragile end, so the
incumbent stays a finisher at every rung:

| keyboard steering from | result |
|---|---|
| race 13.56 s | 15.224 (free) |
| race 6.56 s | 15.220 |
| **race 4.56 s** | **15.217 — the session's fastest tape** |
| race 3.56 s | 15.292 |
| race 2.56 s | **15.285** — 70 input events in the whole run |
| race 1.56 s | 16.276 — over the author time |

**The fastest run on this map is a keyboard run.** Constraining the alphabet did
not cost time; it found time. Compare the analog champion at 15.224.

One methodological note worth keeping: a rung that reports "no finisher" is not a
negative until it is resourced. The 1.56 s rung returned nothing at 2 minutes on
60 workers and produced a finisher at 8 minutes on 90.

*Counting convention: an input change event is any tick where steer, gas or brake
differs from the previous tick, counted over the whole tape including the
pre-start ticks.*

## How it was cracked: a respawn is a legal input

Two earlier sessions established the deliverable had to be one clean no-respawn
attempt from tick 0, and that the human's one winning attempt could not be
transplanted there. Both are true, and both left the map stuck.

The step that was missing is that **a respawn is an input, and on this map the
state it restores is canonical**. It rides in bit 31 of the input packet's 34-bit
state literal — a place `ghost::Factory` cannot see, which is why 941 of them
were invisible against 914 telemetry discontinuities. That turns the impossible
transplant into two lines:

```
[ any prefix reaching race t = 1.670 s ] ++ [ respawn packet ] ++ [ the winning attempt ]
```

finishing at exactly `(K + L)·10 − 1540` ms. Swept over 4 700 ticks of prefix the
arithmetic is perfectly linear, including from mid-flight at the speed cap.
Mutate the prefix 3 000 times and **140 finish, every one at exactly the same
millisecond**.

That produced the field's **first finishing clean-start tape on this map**, at
20.519. **It was never the deliverable** — the respawn cannot be armed before
race t = 1.670 s, so the route floors near 16.1, always outside the author time.
It was the *instrument*: having any finishing tape is what made a dense score, a
calibrated gate ladder and a real search possible, and the ladder carried a
genuinely respawn-free tape home.

Whether a respawn's restored state is canonical is a **property of the map** —
on [The Blev Special](../227654-the-blev-special) the same construction works on
the run's own prefix and fails 0-for-31 on any other line. See
[`FINDINGS.md`](../FINDINGS.md).

## Where the 1.2 seconds is

Station-by-station, the clean tape against the same tail on the respawn route:

| station | x | clean tape | respawn route | Δ |
|---|---|---|---|---|
| p1 | 423 | 1.958 | 2.968 | −1.010 |
| s1 | 505 | 4.118 | 5.206 | −1.088 |
| launch | 713 | **5.550** | 6.656 | **−1.106** |
| finish | 2300 | **15.246** | 16.461 | −1.215 |

**−1.106 of it is the start.** The clean run reaches the state the respawn
manufactures in 0.56 s; the respawn costs 1.670 s. **That difference is the
author time.** The author's route was always a clean start.

Two physical facts bound the map: the opening 3.5 s is free fall (168 m from
about 7 m/s against 23.29 m/s² solves to 3.51 s, which is what the human
achieves), and the glide is unpowered and capped at 277.55 m/s over a 2 665 m
path — **≥ 9.6 s at the cap**, against our 9.70 s.

## The ladder, and the trap it sprang on schedule

Return-to-origin control first: rewriting the 132 gates onto their own lattice
reproduces the human record at 8790.769 and the incumbent at 16.461, so the
surgery is faithful and no model is swapped. Every station calibrated against the
human's own winning attempt to ≤ 8 ms.

Then the decoy fired as documented. Scoring on the **mid-course** rung at
x = 1216 drove the crossing from 9.640 to **7.860** — 1.838 s ahead of the human
— and those tapes reached neither the next station nor the finish. Optimising
"time to a rung in the middle" buys a dive. Moving the objective to the **far**
rung at x = 1822 fixed it in one round.

## Validation

Every tape re-validated on the untouched map (md5
`1cc927bbb1d640c665ff69068352d4e6`) through the plain oracle, in a batch with the
human record as a known-answer control:

```
vj4_best_15217.Ghost.Gbx        15217
vj4_best_15224.Ghost.Gbx        15224
vj4_clean_15230.Ghost.Gbx       15230
vj4_keyboard_15285.Ghost.Gbx    15285
vj4_padfar_15382.Ghost.Gbx      15382
vj4_kb310_16276.Ghost.Gbx       16276
rank00001_8790769.Ghost.Gbx   8790769   <- known-answer control, exact
```

Respawn audit on the same files, with the human record as the positive control
that proves the auditor can say yes:

```
every published tape   packets 2109   0 with bit31
human record           packets 879231   941 with bit31
```

The earlier 15.549 result was additionally re-measured on a third machine by a
different agent with its own build fork and staging root.

**Superseded:** this map was published earlier tonight at 15.240, and briefly at
15.230. Both are superseded by 15.217. The tapes are kept.

## Notes

* [`HUMAN_TECHNIQUE.md`](notes/HUMAN_TECHNIQUE.md) — the player-facing write-up
* [`RESULT_v2_tolerance_and_lowinput.md`](notes/RESULT_v2_tolerance_and_lowinput.md) — tolerance by
  region, the human comparison, and the constraint ladder
* [`RESULT_v1_superseded.md`](notes/RESULT_v1_superseded.md) — kept because its correction is
  instructive: a tolerance reading was wrong because the measuring tool named
  its oracle workers without the map name, so a sweep over several maps under one
  root silently validated every map after the first **against the first map**
* [`RESULT.md`](notes/RESULT.md) — the session that first beat the author time
* [`VERIFICATION.md`](notes/VERIFICATION.md) — the independent third-node re-measurement
* [`ORACLE_THROUGHPUT.md`](notes/ORACLE_THROUGHPUT.md) — three oracle defects found here, worth
  ~1000× on any tape cut from a long recording
* [`TOLERANCE_AND_CONSTRAINT_LADDERS.md`](notes/TOLERANCE_AND_CONSTRAINT_LADDERS.md) — the fleet notice
