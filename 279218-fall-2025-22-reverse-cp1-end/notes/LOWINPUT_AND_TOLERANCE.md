# 279218 — MANDATORY FOLLOW-UP §B: a KEYBOARD tape equals the author time, and it is the human record plus two blips

Auditor agent, 2026-08-19. Prefix `aud_`; **supersedes nothing**. `RESULT.md`'s
5.347 and `ROUTE.md`'s analog account stand untouched. This adds the §B half
that neither contained, and the tolerance measurement `ROUTE.md`'s
precision-bound finding required.

Times in seconds. Map `279218/map.Map.Gbx`, sha256
`883818fa4b8acec7e8fdebbfd2921b5bdbc849c8b9e66646a5b1fcecd38c5343`, untouched.

---

## The headline, stated exactly

**`kb279218_5350` validates at 5.350 — equal to the author time, and 5 ms
inside the human world record of 5.355 — on three steer values and fifteen
input change events.**

**It EQUALS the author time; it does not beat it.** The project's existing beat
on this map is the analog 5.347 in `RESULT.md`. What is new is that the author
time turns out to be reachable **on a keyboard**, which nobody had established,
and by a route a person can be told in two sentences.

## Provenance: this is a human's own lap plus two blips

The tape descends from **`r005_5358`, a real human keyboard run** (rank 5,
5.358, three steer values, 11 events). Searched under `--qlevels 1`, never
converted. And the search kept almost all of it: our 5.350 differs from that
human's own lap in **exactly two places**.

| | r005 (human, 5.358) | ours (5.350) |
|---|---|---|
| 2.76 s | hold RIGHT, 64 ticks unbroken | hold RIGHT 37 ticks, **release for 1 tick at 3.13**, hold 26 more |
| 3.91 s | (nothing — full throttle throughout the left) | **lift the throttle for 2 ticks**, steering still full left |

Everything else — the launch, all three early right taps, the turn-in at 3.50 s,
the final release at 5.36 s — is byte-for-byte the human's.

### Ablation, with the instrument's own identity control

Each blip reverted to the human's inputs individually, then both:

| tape | validated | vs r005 5.358 |
|---|---|---|
| **both blips (ours)** | **5.350** | **−8 ms** |
| blip 1 only — the 1-tick right release | 5.355 | −3 ms |
| blip 2 only — the 2-tick throttle lift | **5.376** | **+18 ms — worse alone** |
| both reverted (control) | **5.358** | **0 — reproduces r005 exactly** |

**They are a coupled pair.** The throttle lift is a 18 ms mistake on its own and
becomes worth 5 ms once the right release precedes it. That is the same shape as
252289's "the right tap is catastrophic alone; it only makes sense once the lift
follows it", and it is the reason nobody stumbles onto these: each half is
punished when tried by itself.

The `revert_both` row is the ablation instrument's identity control — the mixer
reproduces the human's own millisecond from our tape, so the two rows above it
are differences in the blips and nothing else.

## Validation — singly, one file per invocation

Because 37/37-in-a-batch is exactly the shape the corrected batch rule asks us
to distrust, every decisive tape was re-validated **alone**, `--jobs 1`, one
file per invocation, from the store:

| tape | sha256 | singleton | batch |
|---|---|---|---|
| **`kb279218_5350`** | `414d40cabcfb6898…` | **5350** | 5350 |
| `kb279218_5352` | `614024d5f4f20a48…` | 5352 | 5352 |
| `d5_279218_5351` | `7b487fcaf941b642…` | 5351 | 5351 |
| `d3_279218_5354` | `9309982f5020f5e3…` | 5354 | 5354 |
| control `r001_5355` (human WR) | `60f3677a6933fef9…` | **5355** | 5355 |
| control `r005_5358` (the seed) | `9e98eb89d8f36135…` | **5358** | 5358 |
| control `final5347` (banked analog) | `d9f2ddcf2ccd68aa…` | **5347** | 5347 |
| ablations `only_blip1` / `only_blip2` / `revert_both` | see `aud_lowinput.sha256` | 5355 / 5376 / 5358 | — |

Singleton and batch agree on every row. The batch additionally carried all 37
human ghosts, **37/37 exact**. Batch basenames were unique (the trigger
condition for the mis-attribution defect is a duplicate basename), and no row
was a `LOADFAIL`.

## The family

| tape | validated | vs AT 5.350 | vs human WR 5.355 | events | values | alphabet |
|---|---|---|---|---|---|---|
| analog floor (`RESULT.md`, not mine) | 5.347 | −0.003 | −0.008 | 114 | 60 | analog |
| **`kb279218_5350`** | **5.350** | **0.000** | **−0.005** | **15** | **3** | `{−127, 0, +127}` |
| `d5_279218_5351` | 5.351 | +0.001 | −0.004 | 19 | 7 | 5 detents |
| `kb279218_5352` | 5.352 | +0.002 | −0.003 | 11 | 3 | keyboard |
| `d3_279218_5354` | 5.354 | +0.004 | −0.001 | 16 | 5 | 3 detents |
| human WR `r001` | 5.355 | +0.005 | — | 107 | 59 | analog |
| human `r005` (our seed) | 5.358 | +0.008 | +0.003 | 11 | 3 | keyboard |

**Four constrained members all beat the human world record**, and the two
smallest alphabets are the two fastest of them. On this map the alphabet is
nearly free: keyboard costs 3 ms against the analog floor, where on 270053 it
cost 38.

Counting convention: a change event is a tick differing from the previous tick,
race window only, first tick not counted (`aud alphabet`, reading the same bits
the search writes).

Controls before any of it: **identity** — unconstrained from the 5.347 tape,
`best=5347` over 33 570 evaluations, invents nothing. **Zero ladder** — every
steer forced to 0, **finish 0 %** over 33 540 evaluations. The constraint bites.

## Tolerance — and this is the measurement `ROUTE.md` needed

`ROUTE.md` says "a human will not reproduce the percentages… almost any
single-tick deviation is slower", which is a precision-bound finding with no
tolerance number and no forgiving variant. Here is the number, on the same
pessimistic measure used on 270053 (move one boundary ±1 tick, no compensation,
still finish within +0.050 of that tape's own base):

| tape | events | ±1 tick OK **both** sides | at least one side |
|---|---|---|---|
| analog `final5347` | 114 | **71 (62 %)** | 92 (81 %) |
| human WR `r001_5355` | 107 | 26 (24 %) | 91 (85 %) |
| `d5_279218_5351` | 19 | 8 (42 %) | 18 (95 %) |
| **`kb279218_5350`** | 15 | 1 (7 %) | 9 (60 %) |
| `kb279218_5352` | 11 | 0 (0 %) | 9 (82 %) |
| human `r005_5358` | 11 | 3 (27 %) | 5 (45 %) |

Two things fall out, and the second one corrects a natural reading of my own
270053 result:

* **The analog TAS tape is the most forgiving thing on this map (62 %), more
  than twice the human world record (24 %).** So "our tape is a knife-edge" is
  simply not true here — measured, with the field's own tapes as the controls.
* **Fewer events is again not safer.** 15 events at 7 % against 114 at 62 %. And
  the honest comparison for the keyboard tape is not the analog floor but **its
  own human seed**: r005 scores 27 % on 11 events, ours 7 % on 15. A keyboard
  lap on this map is intrinsically twitchy — for the human too.

This is the second map where event count and tolerance move in opposite
directions, in the opposite direction from each other. On 270053 cutting to 10
events made the tape six times *more* forgiving; here the low-event tapes are
the *least* forgiving. **Neither direction is a rule. Measure it per tape.**

### Which way to fail — the asymmetry, and here it is one-sided

Every event of the 5.350 tape shifted ±1 tick:

| moment | 10 ms EARLY | 10 ms LATE |
|---|---|---|
| 0.66 s release right | 5.733 | **DNF** |
| 0.85 s tap right | **DNF** | 5.728 |
| 0.92 s release | 5.744 | **DNF** |
| 1.64 s tap right | **DNF** | 5.781 |
| 2.76 s hold right | 5.391 | **DNF** |
| 3.13 s the 1-tick blip | 5.709 | 5.376 |
| 3.40 s release right | **DNF** | 5.386 |
| 3.50 s turn in left | 6.288 | 5.371 |
| 3.91 s the throttle lift | 6.018 | 5.371 |
| 5.36 s final release | 5.350 | 5.350 — free |

Unlike 270053, **the direction alternates**: the three early right taps want to
be late, the holds want to be early. There is no single "when in doubt" rule
here, and saying so is the honest answer — but the **last two decisions**
(turn-in and lift) both cost ~0.65 s early and only ~0.02 s late, so *at the
corner, late is cheap and early is ruinous.* That is the half worth carrying
into muscle memory.

## What a person actually does — in order

Full throttle from the lights, brake never touched, three keys total.

1. **0.12 s — hold RIGHT** for about half a second, through the opening bend.
   Release at 0.66 s.
2. **0.85 s — a short right tap** (about 70 ms), then let go.
3. **1.64 s — the shortest tap of the run** (50 ms right). Then hands off for a
   full second down the descent.
4. **2.76 s — hold RIGHT again** into the approach… and here is the first of the
   two things nobody does: **at 3.13 s let the key go for a single frame and
   press it straight back.** One flicker in the middle of a long hold.
5. **3.40 s — release**, coast 100 ms.
6. **3.50 s — FULL LEFT, and keep it there to the line.** This is the corner the
   whole leaderboard is fighting over.
7. **3.91 s — the second thing nobody does: lift the throttle for two frames**,
   ~400 ms into that left hold, steering unchanged. Then back on the gas.
8. **5.36 s — release everything.** Free either way.

Steps 4 and 7 are worth 8 ms together and are a package: **the flicker alone is
worth 3 ms, the lift alone loses 18.** Practise them as one move or not at all.

Honest difficulty: this is a 5.35-second sprint where a mistimed frame costs
0.4 s or the run. The two blips are one and two frames long. **A human who
cannot hit them still has r005's own 5.358 lap**, which is 11 events, comfortably
inside the field, and already on the board — the blips are the last 8 ms, not
the technique.

**Classification for this map — precision-bound, and now with its forgiving
variant measured**: `d5_279218_5351` is 42 % two-sided tolerant on 19 events and
still beats the human world record by 4 ms. That is the tape to hand someone who
wants the record without the frame-perfect blips.

## Files, in `279218/`

```
aud_lowinput/kb279218_5350.Ghost.Gbx        equals the AT, 15 events, 3 values
aud_lowinput/d5_279218_5351.Ghost.Gbx       the forgiving one — 42 %, beats the human WR
aud_lowinput/kb279218_5352.Ghost.Gbx        11 events
aud_lowinput/d3_279218_5354.Ghost.Gbx       3 detents
aud_lowinput/only_blip{1,2}.Ghost.Gbx       the ablation
aud_lowinput/revert_both.Ghost.Gbx          the ablation's identity control (= 5.358)
aud_lowinput/aud_VALIDATION_family_batch_v1.txt   43 rows, 37/37 human controls exact
aud_lowinput/tol_*.csv                      six tolerance sweeps
aud_lowinput.sha256
```

Fleet build v6 (`457ed76b8ff0af79adcea32f00a94e4e`) over a pristine hardened
checkout, my own tree. Nothing was submitted to any Nadeo leaderboard.
