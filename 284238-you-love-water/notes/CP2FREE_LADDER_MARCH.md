# 284238 — the CP2-free ladder march: run, and the account survives it

`state_ADDENDUM_v10_cp2free_ladder_march.md`. The experiment
`state_RESULT_v2_closing_account.md` §"still open" named as the one thing that
could break its own conclusion. Sidecar; supersedes nothing. Times in seconds.

Controls, plain oracle, untouched map: record **440.238** exact in every batch.
**No time gain. Best validated tape remains 97.325. A `cps` number from a rung
map is not a time.**

---

## The experiment

Every negative on this map was taken **inside the CP2-collecting basin**: the
trilemma (78 variants), the two-channel search (6500 evaluations, all confined
by a checkpoint term), the brake families, the previous-cycle-exit family. The
rung ladder was built precisely to leave that basin. So: a search scored **only
by rung depth along Yhomas's line**, with no checkpoint term at all.

```
objective = (7 - depth) * 1000 + distance_to_the_next_rung
depth     = how many of his 7 canonical launch points the car passes
            within rtol, in order
window    = ticks 1950-2340 (race 18.0-21.9), steer + throttle, 4 move types
```

Three marches, 10 000 evaluations, `rtol` 8 / 4 / 2 m.

## Result 1 — the tolerance must be smaller than the effect, and 8 m is not

At **rtol 8** the march reached **depth 7 of 7** in 1180 evaluations and the
score went to zero. It is not a success:

* the winner's wall contact is at canonical **z 925.7** — still *our* branch
  (ours 923.4, his 913.9);
* it still returns **`DNF cps=2`** on the untouched map;
* and the reason is arithmetic: **8 m of tolerance is wider than the 9.5 m that
  separates the two branches**, so a tape can "pass" every rung on his line
  while flying our line.

**That is a general instrument rule and I had not seen it stated: a rung's
tolerance must be smaller than the difference it is meant to detect.** A ladder
calibrated by "does the reference fire it" is not calibrated — the reference
fires it from both branches. On this obstacle nothing above ~3 m discriminates.

## Result 2 — at a discriminating tolerance, the march does not move

| rtol | seed depth | best depth after 3000 evals | winner on the untouched map |
|---|---|---|---|
| 8 m | 3 | **7** (spurious, see above) | `DNF cps=2` |
| **4 m** | **0** | **3** | `DNF cps=1` |
| **2 m** | **0** | **1** | `DNF cps=1` |

At 4 m the march gets to R3 (mid-flight) and stops; at 2 m it gets to R1 and
stops. Both winners are *worse* on the real map than the seed — they have left
the basin, as designed, and found nothing on the other side. The distance to the
next rung stalls at 6.1 m (rtol 4) and 11.3 m (rtol 2) and does not improve over
the last 1500 evaluations of either run.

**So the experiment designed to break the account did not break it.** Freed of
the checkpoint constraint entirely, and scored directly on proximity to the
successful branch, the search still cannot get the car onto his line through the
flight and the wall contact — the same R3/R4/R5 gap the ladder calibration
found.

## What this closes

The account in `state_RESULT_v2_closing_account.md` now rests on an experiment
that had every opportunity to contradict it:

> The launch needs a long flat approach and only copy 0 has one; the tube
> delivers copies 1–3 onto the lane 100 m late, and the lateral velocity
> obtainable in the 0.6 s that remains is bounded below what the wall contact
> needs.

**Bound, measured:** copy 0 has ~2 s of flat deck → vz −17.9; copies 1–3 have
~0.6 s → −1.9 (the record) to −15.7 (full lock, the most that fits); Yhomas has
a flat run-up in all four copies → −24/−25. Full lock is worth 13.4 m/s on our
water lane and 13.2 on his tech lane, so it is not grip; 97.2 fails while 90.9
and 99.1 work, so it is not speed.

## Caveat, and it is a real one

The fleet has just found that **`ghost::Factory` cannot see or remove a respawn**
(bit 31 of the state literal), so every search seeded from a respawn-carrying
tape runs under a frozen retry schedule. Our record carries 31 and `best_97325`
carries 4. **That does not affect the launch families in this file or in v1–v9:**
all of them perturb steer/gas/brake inside a 0.4–4.0 s window in cycle 1, between
the record's respawn at 11.040 and the next at 51.780, so no respawn lies inside
any perturbed window and none could have been added or removed. It *would*
affect any whole-tape search, and it is the right caveat to carry into one.

## Enumeration

* 3 marches × 3000–4000 evaluations = 10 000, window ticks 1950–2340, two
  channels, 4 move types, spans 1–40 ticks, rtol ∈ {8, 4, 2} m.
* All three winners validated on the untouched map (`DNF cps=2`, `cps=1`,
  `cps=1`); the rtol-8 winner additionally measured per tick to establish that
  its wall contact is on our branch.
* This is a local-search negative over the arc+lane window at three tolerances,
  not an exhaustive statement about all tapes.

---

## Appendix: steer + brake on the lane — the last channel pair, also closed

The channel matrix was steer alone (60+ variants), steer + throttle (6500
evaluations), and steer + brake untested. 16 variants, gas and brake held
together with lock over the last 0.2–0.6 s of the lane (windows 2281:2341,
2301:2341, 2311:2341, 2321:2341 × −30, −60, −90, −127):

| | wall plane z | one-tick loss | CP2 |
|---|---|---|---|
| best of the 16 (2281:2341, −127) | **916.95** | 1.03 | 5.2 m, 37.9 m/s |
| baseline | 923.35 | 8.71 | 6.5 m, **45.80** |
| target (Yhomas) | **913.9** | 0.75 | **69.40** |

The same trade as every other lane family: the wall contact comes down (923.4 →
917.0, and the slam disappears, 8.71 → 1.03) and the checkpoint crossing speed
falls with it (45.8 → 37.9). Scrubbing buys the contact height by spending the
speed that made it worth having. **No variant improves the checkpoint crossing;
the best of the 16 is 7.9 m/s worse than doing nothing.**

The channel matrix is now complete, and every cell of it is a variant of the
same trade curve.
