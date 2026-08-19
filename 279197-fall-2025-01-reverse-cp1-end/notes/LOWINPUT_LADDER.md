# 279197 — the low-input ladder

**Addendum to `RESULT.md`, written by a second agent on 2026-08-18.** It
replaces one section of that document and leaves the rest standing.

The time result in `RESULT.md` is correct and I re-verified it before doing
anything else: `best/real_10594.Ghost.Gbx` → **10.594 s** on the untouched map
(md5 `f62e7c61872ed27e81352937198ff384`), with the human WR in the same batch
returning **10.602 s** as an identity control. Their gate no-op control
(place the Goal item back at its own position) also reproduces exactly on a
fresh node: 10.602 / 10.598 / 10.800 for three different runs. The AT of
**10.598 s** is beaten and stays beaten. None of that is in question here.

## What this addendum corrects

`RESULT.md` says:

> For the same reason there is **no low-input family to publish**. Every
> simplification of the tape — coarser steering, held inputs — fails. That is a
> result, not an omission.

That conclusion was reached by **converting** the finished analog tape: coarser
steering, held inputs, sample-and-hold. Converting is the one method the project
has repeatedly established cannot produce a low-input tape anywhere — on 145875
(quantising the champion DNFs at 3, 5 and 9 levels; replacing its analog sweeps
with the instant steps a keyboard physically produces gave 0 finishes in 82
placements), in `LOWINPUT-ADDON.md`, and independently on 227969 (DNFs at every
resolution, down to a 64-level ladder whose maximum change is ±1/127 per tick).

The method that does work is to **search under the constraint** (`--qlevels N`,
which hooks `mutate_ctx` so the constraint binds both search paths and the seed
is quantised too, so the incumbent is legal from tick 0).

Both halves were measured here, in the same experiment:

* **Conversion reproduces their negative exactly.** `--qlevels 1` or
  `--qlevels 4` seeded from the analog champion or from the human WR: **0 %
  finish**, DNF immediately. Not "slower" — it does not complete the map.
* **Constrained search does not.** From a compatible seed the same constraints
  finish 22–57 % of the time and improve steadily for 45 minutes.

So a low-input family exists on this map. It just cannot be reached from an
analog tape, and it does not reach the author time — see the honest summary at
the bottom.

## The alphabet, read off the human tapes

Not invented. `tmtraj slew` splits the 27-ghost sample cleanly into pad and
digital by steer change per 10 ms tick, and the distinct values every digital
human actually uses are multiples of **20 % of full lock**:

| run | rank | time | distinct steer values used |
|---|---|---|---|
| r502, r503 | 501+ | 10.800 s | −1.0, 0, +1.0 |
| r301 | 301 | 10.724 s | −1.0, −0.4, 0, +1.0 |
| r302 | 302 | 10.724 s | −0.2, 0, +0.2, +1.0 |
| r501 | 501 | 10.798 s | −0.6, 0, +0.6, +0.8 |
| r053 | 53 | 10.628 s | −0.8, −0.4, 0, +0.8 |
| r152 | 152 | 10.658 s | −1.0, 0, +0.8, +1.0 |

(0.7961 and 0.4039 are the byte encodings of 0.8 and 0.4.)

So on this map:

* **`--qlevels 1`** = `{−127, 0, +127}` = **pure keyboard**, the alphabet r502 /
  r503 drive.
* **`--qlevels 5`** = `{0, ±25, ±51, ±76, ±102, ±127}` = **exactly the 20 %
  action-key ladder** the digital humans use. This is the rung that corresponds
  to a real, bindable setup — it is what r053, r152, r301, r302 and r501 are
  all drawing from.
* `--qlevels 8` and `--qlevels 16` are finer than anything a human is observed
  to use here. They are included because they bound the cost of the alphabet,
  not because anyone can press them. Treat them as measurement, not advice.

## The ladder

All times validated on the untouched map through the plain oracle. Six 45-minute
arms plus two extensions, 30–42 workers each, **each with its own `--root`**;
zero failed re-validations, nothing in `phantoms/`. "Events" is input CHANGE
events; a value held 40 ticks is one event.

| rung | seed | time | vs AT | events | alphabet |
|---|---|---|---|---|---|
| analog (`RESULT.md`) | — | **10.594 s** | **−4 ms** | — | 255 |
| human WR, for scale | — | 10.602 s | +4 ms | — | 255 |
| 16 detents | analog champion | **10.602 s** | +4 ms | 264 | 32 |
| 8 detents | analog champion | 10.608 s | +10 ms | 137 | 17 |
| 8 detents, event-reduced | " | 10.618 s | +20 ms | **85** | 17 |
| **5 detents = action keys** | human r152 | 10.643 s | +45 ms | 76 | 11 |
| keyboard | human r301 | **10.636 s** | +38 ms | 66 | 3 |
| keyboard, event-reduced | " | 10.640 s | +42 ms | 57 | 3 |
| keyboard | human r152 | 10.646 s | +48 ms | **35** | 3 |
| 5 detents | analog champion | 10.702 s | +104 ms | 162 | 10 |
| keyboard or 4 detents | analog champion / WR | **0 % finish** | — | — | — |

Two things fall out of that table beyond the headline numbers.

**The seed matters more than the constraint.** Five detents from the analog
champion is 10.702 s; the same five detents from a human action-key run is
10.643 s. A 59 ms swing with the alphabet held fixed, decided entirely by which
basin the search started in. This is the same "independent searches occupy
disjoint basins" law that killed cross-splicing on maps 1 and 2, and it is the
practical reason conversion fails: quantising drops the tape outside the basin
its own inputs were written for.

**Fewer events is nearly free within a rung, and expensive across rungs.**
Reducing the keyboard tape from 66 to 57 events costs 4 ms; the r152 keyboard
tape is 35 events for 10 ms more. But going from 30 values to 3 costs 33 ms.
If you want a tape a person can hold in their head, the value alphabet is what
you pay for, not the number of inputs.

## Does it close? No — and here is why, measured

**Keyboard does not reach the author time on this map, and I do not believe more
search changes that.** The field says why, and it is the opposite of 145875:

* On 145875 eight of the thirteen fastest humans were on a keyboard, and a
  keyboard tape landed 1 ms off the unconstrained floor.
* Here, **the top of the board is all pad**. Ranks 1–15 have a median non-zero
  steer change of 0.02–0.08 per tick. Every digital human sits at rank 152 or
  worse: 10.658, 10.724, 10.798, 10.800. The best keyboard human is 60 ms off
  the AT before we start.
* The digital deficit is **diffuse carry, not one mistake**. Sampling speed at
  1 s intervals, the digital runs are 1–4 km/h down at every station from
  t = 8 s onward (WR 286 / 309 / 332 km/h at 8 / 9 / 10 s; r152 285 / 308 / 331;
  r301 282 / 305 / 328). There is no single corner to fix — a coarse alphabet
  simply cannot hold the small steady steering angles the sweeper rewards.

The 16-detent rung gets within 4 ms, and that is close enough that the
millisecond-quantisation plateau described in `RESULT.md` becomes the obstacle
— so I checked it with their own vernier rather than guessing. Sweeping the
finish plane back in 5 cm steps:

```
        gate |  real_10594 | z16a_10603 | r001_10602 (human WR)
  z=768.0000 |       10594 |      10603 |      10602
  z=768.2000 |       10596 |      10606 |      10604
  z=768.4000 |       10598 |      10608 |      10606
  z=768.6000 |       10601 |      10610 |      10608
```

The 16-detent tape is **genuinely ~0.85 m behind** the analog champion along the
finish axis. That is a real distance deficit, not a reporting artefact — so the
gap is not a plateau illusion and will not fall to a vernier ratchet the way the
analog 4 ms did.

A further 2.7 hours on that rung — three more arms from the 10.603 s tape, 55 min
each at three different window widths, 1.66 M evaluations — moved it exactly
1 ms, to **10.602 s** (264 events, 32 values, validated). That is dead level
with the human pad world record and still 4 ms short of the author time, with
the rate of improvement down to 1 ms per 50 minutes. I am calling that converged
rather than grinding it further.

## Honest summary for a driver

If you are on a pad, `RESULT.md` is your document: the author time is beatable
and the run is there.

If you are on a keyboard or action keys, this map is not currently winnable at
the author time by anything we can find, and you should know that before
grinding it. What is available:

* **Action keys (20 % detents), 76 inputs: 10.643 s.** This is the rung that
  matches a real bindable setup, and it is 15 ms faster than the best keyboard
  human on the board (r152, 10.658 s) — so it is a genuine target even though
  it does not touch the AT.
* **Pure keyboard, 35 inputs: 10.646 s** — 12 ms faster than the best keyboard
  human, in half the inputs.

Both would be top-150 runs on a 561-record board. Neither is the author time,
and no amount of the search we have applied brought a 3-value alphabet within
38 ms of it.

## Method notes for whoever reads this next

* **`tmtraj decode` on a synthetic candidate reads the TEMPLATE's stale
  telemetry, not the tape's inputs.** Decoding the quantised tapes showed an
  86-value analog alphabet, which is the template's, and I nearly reported it.
  Use `tmsimp` — it reads the real input archive. On these tapes it confirms
  17 values for the 8-detent arms and 3 for keyboard, i.e. `--qlevels` really
  does constrain the stored incumbent and the fourth phantom class does not
  apply here.
* Every arm ran with its own `--root` and its own `--bestdir`, and the tape
  clock offset was checked before setting `--hi` (0 ms here; the template is
  1061 ticks and the finish is tick 1060 — unlike 145875, where it was
  −1540 ms).
* Their vernier (`tools/rank.sh`, `tmmaps places --rank`) rebuilds and runs
  correctly on a fresh node once `.cargo/config.toml` and `vendor/` are copied
  in from another checkout; the tarball ships without them.
