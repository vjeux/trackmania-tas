# 285885 — THE TRIGGER: the sunken gate CAN be fired early, and the world record misses it by under 5 mm

*Sidecar, map 285885 `finish is on the roof to your right`. Written by the
trigger-half agent (previously 197047) under the mechanism split: the route and
the roof crawl belong to the other agent, whose `FINDINGS.md` in this directory
is theirs and which I have not touched. Every file I have written here is
prefixed `bis197047_`.*

**Answer to the assigned question: YES, and by a wider margin than anyone
expected. One steering input, held 0.18 s, at race 50.91 s, turns the world
record's 61.229 s into 51.059 s — a 10.170 s gain, validated on the untouched
map. The best I have is 50.509 s. This does NOT beat the author time (43.079 s);
it removes the whole 10.2 s of "drive past the gate, turn round and come back"
and leaves the remaining ~7.4 s squarely in the route.**

## 1. Controls first

| control | result |
|---|---|
| map md5 vs the one in this directory | `1c902574afff5e48193928c4c3188ee8`, identical |
| §4/§8 field reproduction, my own copy | **3/3 exact** (61229 / 88209 / 97769) |
| gate surgery at the ORIGINAL position | **61229 / 88209 exact** — see §2, this took two attempts |
| `wig poke` identity (no override) | 61229 |
| headline tapes, 3 cold passes, WR as known-answer control | every tape its filename time, control 61229 every pass |

## 2. A correction to the instrument, and it matters for anyone reusing it

My first gate-surgery sweep put the gate back at its **original** position and
got **50589**, not 61229. The instrument was wrong, and it was wrong in the
direction that manufactures a discovery.

Cause: `tmmaps gate` (and `segments::move_gate` under it) does
`set_item_model(FINISH_GATE)` **before** moving — it swaps
`GateFinishCenter8mv2` for `GateFinish32m`. On this map that replaces an 8 m
gate with a 32 m one, so the trigger volume quadruples and everything fires
early. Every number from such a sweep is an artefact.

Fix: a **position-only** patch — write the three pos floats, leave model, cell
and yaw exactly as the author placed them. Added as `tmmaps moveitem --item N
--at x,y,z --out F` (source banked as `bis197047_tmmaps_main_moveitem.rs`).
With that, the original position reproduces 61229 / 88209 / 97769 exactly, and
the other agent's published numbers reproduce too (gate y=145 → 50639 / 51009).

**Rule for the fleet: a gate probe is only an instrument if moving the gate back
to where it started reproduces the untouched map to the millisecond.** This is
§0.4's "an instrument that can only say yes" in a new costume — the broken
version says "fires early" for every input, including the null one.

## 3. The trigger volume, measured

Gate origin `(419.0277, 144, 1704.6367)`, item 0, `GateFinishCenter8mv2`, yaw 0.

**Vertical — the decisive axis.** Sweeping the gate's y and reading the world
record's finish time:

| gate y | WR finish | gain |
|---|---|---|
| **144.000** (true) | **61229** | — |
| 144.005 | 51059 | **10.170 s** |
| 144.010 – 144.030 | 51039 | 10.190 s |
| 144.035 | 50679 | 10.550 s |
| 144.050 | 50659 | 10.570 s |
| 144.150 – 146.000 | 50639 | 10.590 s |

**Five millimetres of gate height is worth 10.17 seconds.** The trigger's top
face sits exactly **1.25 m above the gate origin** (top at y = 145.25), and on
the upward pass the world record's lowest point inside the horizontal extent is
y ≈ 145.2506 — it grazes the ceiling of the trigger volume and misses by **less
than 5 mm**, drives on up the roof, turns around, and comes back down to fire it
at 61.229 s from the other side.

The other agent's discriminator (car origin y ≤ 145.25 fires, ≥ 145.27 does not)
is confirmed exactly; this pins the boundary to the millimetre and identifies it
as gate_y + 1.25.

The same sweep on the other two runs — their margins are larger:

| run | fires early at gate y | implied miss |
|---|---|---|
| rank 1 (61229) | **144.005** | **< 5 mm** |
| rank 2 (88209) | 144.015 (→ 68749) | ~15 mm |
| rank 3 (97769) | 144.020 (→ 96599) | ~20 mm |

All three humans miss the early fire by **under two centimetres**. This is not a
route difference; it is the same near-miss three times.

**Horizontal.** With the gate lifted clear (y = 148) so height never binds, and
sweeping the gate against the WR's fixed line, fires occur for
dx ∈ [−8, +4] m and dz ∈ [−6, 0] m, DNF outside. Those bounds are the volume
convolved with the car's path, so treat them as "the line passes through the box
over roughly that offset range", not as the box's own dimensions.

## 4. There is no earlier opportunity, and this is worth knowing

I checked whether the gate could be fired during the big launch — the obvious
"different solution shape". It cannot, and not marginally:

**The world record's closest approach to the gate at any time before race 45 s
is 966 metres.** The flight, the highway blast and the climb all happen on the
far side of the map. The first time the car is within 3 m of the gate is
race 51.2 s. There is exactly one window in the entire run, and it is the roof
arrival the other agent is already optimising.

So the trigger cannot substitute for the route. It removes a fixed 10.2 s of
overshoot-and-return, and nothing more.

## 5. The input, and it is absurdly forgiving

Found by a windowed search on the untouched map (`--lo 4900 --hi 5310`), which
hit it on **evaluation 1**. Reproduced exactly by a rectangular override, so it
is one input and not a search artefact:

**At race 50.91 s, hold the steering at −9/127 (7 % left) for 0.18 s.**
Where the world record instead snaps to full right lock.

Validated: **51059 ms**, against the WR's 61229.

Then the tolerance sweep, and this is the striking part. Sweeping the held value
over the full range at the same instant:

| value held | result |
|---|---|
| −127, −100, −80, −60, −40, −30 | DNF (run lost) |
| **−20, −15, −12, −9, −6** | **51059 – 51069** |
| −3 | DNF (chaotic hole) |
| **0** (just don't steer) | **51049** |
| **+3, +6, +10, +15, +20, +30, +40, +60, +80, +127** | **51039 – 51069** |
| the world record's own input | 61229 |

**Sixteen of the twenty-three values tried fire the gate ten seconds early, and
one of the ones that does not is what the world record actually did.** Steering
neutral works. Full right lock works. Only hard *left* (≤ −30) loses the run.

Timing window, at a representative value (+60):

| start (race) | result |
|---|---|
| 50.44 s | **50659** (best of the simple family) |
| 50.49 – 50.69 s | 51019 – 51039 |
| 50.99 s | 51059 |
| 51.04 s and later | DNF |

So a **~550 ms window** of opportunity, with a couple of chaotic holes in it,
and hold durations from 0.08 s to 0.5 s all work. For a driver this is not a
trick at all — it is "arrive and do almost anything except what the record did".

Best simple one-input tape: **50659 ms**, `+60` held 0.25 s from race 50.44 s.
Best from the free search: **50509 ms**.

## 6. What this means for the route half

- The finish patch does **not** need to be reached in the crawling, come-to-rest
  sense. **First contact fires it**, provided the car is under y = 145.25 while
  inside the horizontal extent.
- Your target is therefore purely **earliest arrival at the patch with the car
  origin below 145.25**, and the vertical condition is nearly free on the
  approach from the west (the roof rises through it — the car is *below* 145.25
  for the whole approach and only climbs out of it around x ≈ 416).
- The floor on the existing line is ~**50.5 s**. The author time is 43.079 s, so
  the route must find **~7.4 s**, all of it before the patch.
- A landing, a bounce or a dip is **not required** — that hypothesis is dead.
  The car is already low enough on approach; what the record does wrong is climb
  out of the volume 4 m too early by turning right.
- If your route arrives at the patch from a different direction or at speed,
  check the horizontal bounds in §3: the volume is offset **west and slightly
  negative-z** of the gate origin, so an approach from the east arrives above it.

## 7. Files

All in this directory, all written by me, all prefixed `bis197047_`:

| file | what |
|---|---|
| `bis197047_bisTAS_poke_1input.Ghost.Gbx` | **50659** — one input, +60 held 0.25 s from race 50.44 s |
| `bis197047_bisTAS_search_best.Ghost.Gbx` | **50509** — windowed search best |
| `bis197047_CONTROL_humanWR_61229.Ghost.Gbx` | the control carried in every batch |
| `bis197047_tmmaps_main_moveitem.rs` | `tmmaps moveitem` — position-only item surgery (§2) |
| `bis197047_wig_poke.rs` | `wig poke` — rectangular input override + tolerance sweeps |

`FINDINGS.md`, `variants/`, `csv/`, `ghosts/`, `seed*`, `lat_*`, `bis_418*` in
this directory are the route agent's and I have not read, moved or written any
of them beyond reading `FINDINGS.md` once.

## 8. Findings that generalise

* **A gate probe needs a return-to-origin control.** `move_gate` swaps the item
  model; on a map whose gate is not the standard 32 m finish that silently
  changes the trigger size. Position-only surgery plus an origin control is the
  only safe form. (§2 — and it produced a confident wrong answer first.)
* **Sub-centimetre misses are worth double-digit seconds on a sunken-gate map.**
  Bisect the gate offset rather than reasoning about the car's suspension: five
  millimetres of gate height was the entire question here, and the telemetry at
  50 ms could never have resolved it.
* **Before hypothesising an exotic early trigger, compute the closest approach.**
  966 m at 45 s killed the "fire it during the flight" idea in one pass over a
  CSV, and saved a search that could not have succeeded.
* **When a search finds its answer on evaluation 1, re-derive it as a single
  rectangular override.** It converts "the search found something" into "this
  one input does it", which is what a human can be handed, and it doubles as the
  instrument for the tolerance sweep.
