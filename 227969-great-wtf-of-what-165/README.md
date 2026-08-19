# Great wtf of## The driving guide

The full write-up, including the per-input timing tolerance table and a
sector-by-sector guide with a cue for every input, is in
[`notes/RESULT.md`](notes/RESULT.md).

The short version, if you only read one thing: **after the throw, do not hold
the lock. Three taps — hooold, tap, tick (220 / 80 / 40 ms).** Release the first
one at the bottom of the swing, the instant you stop sliding backwards and the
car starts being flung forward up the corridor (speedo 292 km/h). Commit to the
kicker when the nose stops rising, about two car lengths before the lip.

**And the cue that needs no instrument: the horizon must stay level.** Hold the
lock and the car rides up the curved wall and rolls over — by the kicker the
world record is nose-up 57° and nearly on its side. Our run never rolls past 5°.
If your horizon tips, you are driving the old line.

## Files

| file | what |
|---|---|
| `replays/best_7998#165 — author time beaten by 129 ms

| | time | vs AT | vs human WR |
|---|---|---|---|
| **TAS, unconstrained** | **7998 ms** | **−129** | **−199** |
| **TAS, keyboard only** | **8075 ms** | **−52** | **−122** |
| TAS, 8-level action keys | 8050 ms | −77 | −147 |
| Author time (never beaten by a human) | 8127 ms | — | −70 |
| Human WR — Titoch_tm | 8197 ms | +70 | — |

TMX map [227969](https://trackmania.exchange/maps/227969) · uid
`LtSUTxJ71u7ayvLj57wUdVPyH2h` · author **FrankTheHamster** · 42 recorded runs,
all of them downloaded and analysed.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## The interesting result is the keyboard run

`replays/kb_8075.Ghost.Gbx` uses steering values `{-127, 0, +127}` and nothing
else — exactly what a keyboard produces — with **14 input change events, 12 of
them steering**.

The human world record holder is *also* on keyboard, using the same three
values, with **11 steering events**.

So: same device, one more input than the world record, and **122 ms faster**.
This is not a run that wins on machine precision. It wins on doing a different
thing with roughly the same number of key presses.

## What it does differently

All of the gain is in the last 16 % of the course. Over the first 560 m of
~680 m this run is actually **10 ms slower** than the reference human line —
it inherits that prefix from a human tape. The entire 129–199 ms is made
between 560 m and the finish.

**The final wall-ride is where the field loses the time.** Everybody rolls the
car onto its side (roll 0.9–1.5 rad) and pitches up ~55° through it. The kicker
at the end then eats about a third of their speed — the world record goes
73.6 → 61.3 m/s across it.

This run arrives **flat** (roll 0.06 rad, pitch 30°) and **square**
(lateral velocity 0.35 m/s), and loses only 3 m/s. Vertical speed into the
finish plane: **69.2 m/s, against the best human's 59.8 and the world record's
57.3.**

In the corner before it, the keyboard tape follows the world record's input
script exactly up to 5.24 s and then **pumps** the steering — three taps of
220 / 80 / 40 ms — where the world record holds one continuous 390 ms lock.

If you take one thing from this map: **stop holding the lock through the final
corner, and get the car flat before the kicker.**

## Is it a legitimate run?

Checked geometrically, not just by the clock, because the map declares a single
waypoint pair so a checkpoint count proves nothing:

- The line **never leaves the human racing corridor**: maximum distance from the
  human world record's own trajectory over the whole run is **2.57 m**.
- At the decisive point (z = 855) its speed, vertical speed, pitch, roll and
  lateral position are all **inside the range of the 42-run human field** — and
  two human runs pass that point *faster* than this one does.
- The map contains exactly one collision event, the wall throw at 6.68 s that
  redirects the car ~100°. **Every human run takes it identically.** This run is
  not exploiting it, it is surviving it better.
- No respawns, no skipped geometry, no out-of-bounds flight.

## Validation

Three cold validations in fresh processes against a re-downloaded, byte-identical
copy of the map: **8010, 8010, 8010** for the first banked tape; the later tapes
were each re-validated the same way. A human ghost carried as a known-answer
control returns **8197** every time.

Beyond that, **all 164 tapes any search arm wrote during this work** were
re-validated through the plain oracle: 164/164 returned exactly the time in
their filename. Zero phantoms.

Raw transcript: `notes/validation_8010.txt`. Fuller working notes:
`notes/RESULT.md`.

## Files

| file | what |
|---|---|
| `replays/best_7998.Ghost.Gbx` | fastest run, unconstrained inputs |
| `replays/kb_8075.Ghost.Gbx` | **keyboard only** — the one worth studying |
| `replays/ak8_8050.Ghost.Gbx` | 8-level action-key steering |
| `replays/best_8010.Ghost.Gbx` | the first tape to beat the AT |
| `inputs/m165_TAS_8010ms.tick.txt` | that run as a TICK input script |

Note the ghosts' *declared* header time is inherited from the human tape each
was built from. The number that matters is the **validated** time — what the
server produces when it re-simulates the inputs — which is the number in the
filename.
