# impossible at for ssano — the author time falls by 299 ms, on a keyboard

| | time | vs AT | vs human WR | alphabet |
|---|---|---|---|---|
| **TAS, keyboard** | **14.349** | **−0.299** | −0.690 | **3 values** |
| TAS, analog | **14.289** | −0.359 | −0.750 | analog |
| robust keyboard variant | 14.479 | −0.169 | −0.560 | 3 values |
| 30-event variant | 14.608 | −0.040 | −0.431 | 3 values |
| Author time (never beaten by a human) | 14.648 | — | −0.391 | — |
| Human WR — `in-.-`, rank 1 of 147 | 15.039 | +0.391 | — | — |

TMX map [249521](https://trackmania.exchange/maps/249521) · **147 recorded
runs**, all re-simulated.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## The map is four blocks, driven backwards

The checkpoint is **12 m in front of the spawn**. The finish is **80 m behind
it**. So the whole race is 64 m of boost pad taken *backwards*, with the pads
pushing the wrong way — a car reversing straight down the strip gets stopped,
turned round, and carried forwards to +46 km/h back at the start block.

Every human on the leaderboard solves it the same way: **wag the nose ±90–100°
across the strip**, so the pads catch the car at an angle and push it
*backwards* along its own heading. That wag is **11.4 of the world record's 15.0
seconds**.

The tempting shortcut is ruled out by measurement, not by intuition: full lock
settles into a 32 km/h orbit with zero progress, and centring out of it flings
the car sideways at 210 km/h off the strip.

## Where the 391 ms is — the attitude at the lift, not the timing of it

The margin is **diffuse**: the launch is a dead heat, and all of it is in the
four swings. And the field already has the *shape* — both pedals down, one gas
lift per swing — with the world record's lifts falling at the same race times as
ours **to within 30 ms**.

The difference is what the car is doing at the moment of the lift:

| | heading at the lift | what the pads give |
|---|---|---|
| the field, including the WR | **80–85°** — before the nose is square | **0–6 km/h** |
| this run | **90–105°** — past square | **25–35 km/h** |

That speed converts into backwards ground as the nose swings back through
straight. **Verdict: known but mis-timed.** Nobody needs a new technique — they
need to hold the swing a fraction longer before lifting.

The world record's own tolerance table agrees: moving *its* fourth lift three
ticks later is worth **160 ms** to that run.

## It is more forgiving than the run a human actually drove

Against the right control — the human world record's own driven tape, put
through the identical test:

| tape | survives ±1–4 tick mistimings |
|---|---|
| **our keyboard tape** | **41%** |
| the human world record | **18%** |

## Three results that cut against instinct

- **Thinning made it worse in both senses.** Reducing 54 events to 30 cost time
  *and* dropped survival from 41% to 10%. Fewer inputs is not automatically more
  drivable.
- **The robustness re-placement pass bought 2 percentage points of survival for
  130 ms** — a bad trade, and worth knowing before running one.
- **Peak swing speed correlates 0.02 with finishing order** across the whole
  field. The flashy difference between runs is noise; "swing harder" would have
  been exactly the wrong lesson to publish.

## Validation

**All 147 ghosts — the entire leaderboard — were re-simulated: 146 exact, one
mid-field DNF at rank 76. The top 75 are all exact.** Every reported tape passed
3+ cold passes in fresh processes with the human world record carried as a
known-answer control, returning 15.039 every time.

**Then the two headline tapes were re-verified independently**, on a separately
compiled build fed only from the archive: **14.289 and 14.349, both exact**, with
three human records as controls (15.039 / 15.196 / 15.199, all exact) and zero
respawns under both respawn keys. The control could have failed and was shown to
be capable of failing — the brake anchor was deliberately broken at three depths
and the run died each time. Re-checked once more against the untouched map for
publication, and the two files published here are **byte-identical** to the ones
that were verified.

**A trap found here and patched:** `--quant` is a **silent no-op in the classic
search path** (it is implemented only in the fork path). Three runs here were
reported as keyboard while actually being unconstrained analog before it was
caught. Any pre-fix `--quant` result from the classic path is suspect — the
classic-path ladder is `--qlevels`.

**The keyboard claim survived that trap the only way a claim can**: the verifier
read the alphabet **off the tape** instead of trusting the flag that produced it,
and all three keyboard tapes are exactly `{−127, 0, +127}`. The no-op never
reached anything shipped here.

And the comparison on this map is unusually clean, because **the human world
record is itself a pure-keyboard run** — three values, 38 change events. Like
against like: same device, same alphabet, 0.690 s apart.

## Files

| file | what |
|---|---|
| `replays/KEYBOARD_14349.Ghost.Gbx` | **the one to study — 3 values, 0.299 inside the author time** |
| `replays/TAS_14289.Ghost.Gbx` | the analog floor |
| `replays/ROBUST_KEYBOARD_14479.Ghost.Gbx` | the robustness-optimised variant |
| `replays/DRIVABLE_30ev_14608.Ghost.Gbx` | 30 events — slower *and* less forgiving, kept as the counter-example |
| `notes/RESULT.md` | full write-up, sector guide off visual cues, tolerance tables |
