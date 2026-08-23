# 267460 `Impossible Mini Trial 2` — AT NOT BEATEN, and the map is a different shape than we thought

Arm `imt3`, 2026-08-22. Times in **seconds**, speeds km/h.
AT **16.888** · human WR **23.068** (Wirtual, the only record) · incumbent
**21.022**, unchanged. Artefacts in `imt3_20260822/`; controls in
`imt3_20260822/imt3_CONTROLS.txt`.

---

## 1. The finding

**The whole endgame of this map is one wall, and the wall has a hole in it.**

The `z = 686` screen is not something the route must go around. It is a wall
with an **opening at x > 922, y > 108** — and a line through that opening is
worth **~17.5** from a prefix no better than the human's. That is 3.5 s under
our incumbent and 5.5 s under the human, and it is the first thing found on this
map that is the right SIZE to explain an author time 37 % faster than the only
human record.

**We cannot yet fly through it, and the shortfall now has a number.** Stated as
geometry rather than as a search record: **the screen has to be displaced 4.4 m
diagonally before our best tape gets past it.** This morning the best tape
anyone had needed **14 m**.

---

## 2. What was believed, and what was wrong with it

> *"END: the U-turn is not a choice. Solid X 826–1002, Y 45–135. The only
> reachable opening (X > 1018, Y 45–69) puts the car PAST the flag."*

The grid that sentence rests on (`imt2_wallmap_z686_v1.txt`) is **right**. I
re-read it and everything measured today agrees with it. What was wrong is the
**reading**: that grid's own rows show the solid band at Y 110.8–134.8 stopping
at **X = 922**, and everything east of 922 at deck height is open. The opening
was never named because no tape had ever been near it — every tape on this map
is 30 m below it by the time it reaches x = 922.

**"The only reachable opening" was a statement about the six tapes we had, not
about the map.** That is the third bound on this map to come from a search
record read as a property of the geometry.

---

## 3. The route, measured end to end

| | |
|---|---|
| the human | deck → launch → fly east to x ≈ 1090 → loop right → back west through the `GateSpecial32mNoEngine` at (1056, 49, 672) → **coast, engine dead, 162 → 8 km/h over the last 2.5 s** → finish |
| through the hole | deck → launch **south-east** → cross z = 686 at x > 922, y > 108 → dive straight into the flag at ~290 km/h |

The eastern loop and the dead-engine coast are **5.5 s** of the human's 23.068.
The hole skips both.

None of this is inferred. `ml_probe_air_17518_on_SCREENREMOVED_map` — a tape an
earlier arm built on a map with the screen deleted — finishes at **17.518** by
crossing z = 686 at **(926.6, 100.0)** and diving onto the flag. On the real map
that same tape **bounces off the screen at (917, 105, 690)** and flies away
north (`imt3_20260822/tr_air_ON_REAL.csv`, the tick the z-velocity flips from
−19.7 to +9.6). The difference between the two is about eight metres.

---

## 4. Where the hole is: three methods, one number

**(a) Map surgery, with controls.** `tmmaps shift` (built here) displaces the 28
screen blocks by a known amount and re-reads the written map to require every
one to have moved. Sliding the screen and asking the plain oracle which
displacement first lets a tape through maps the opening in the CAR's own
coordinates, hull and attitude included:

| tape | passes when the screen moves |
|---|---|
| `air_17518` (this morning's best) | west ≥ 14 m **and** down ≥ 3 m |
| `V36_T1550_W-127` (best hand-built) | west ≥ 6 m **and** down ≥ 9 m |

The two requirements are independent — west 12 fails at every down, down 2 fails
at every west — because they are two different pieces of the screen: the upper
band's **east edge** and the lower band's **top**. Collapsing that to one number
— the smallest DIAGONAL displacement that lets a tape through — is what §1
quotes. Grids: `imt3_wallshift_grid_*.tsv`, `imt3_diagonal_relief_by_tape.tsv`.

Controls, in the same batches: the screen 2000 m down returns the tape's own
17.518, so the instrument is alive; the screen at its own place returns DNF; and
**the human WR is 23.068 exactly on every relieved map**, so the relief is not a
general speed-up — it only helps a tape that uses the hole.

**(b) The state objective.** Six independent searches (different seeds, fork
points, temperatures, operator sets; 0.6–7.5 M evaluations each) converge on a
worst margin of **−5.2 to −6.4 m** against the corner.

**(c) By hand.** 358 tapes over a three-parameter family (deck steer, turn tick,
turn hardness), built with `ghost tape set`: best margin **−6.15 m**, and the 20
that still finish with the screen removed have their best at exactly that.

---

## 5. Why it is metres and not zero — the mechanism

The car leaves the deck at y = 114 and must reach z ≈ 690 with **x ≥ 922 and
y ≥ 108**. Ballistically those pull against each other: more southward velocity
gets you there sooner and higher but not far enough east; less gets you east but
too low. At ~21 m/s² the whole flight is 0.7–0.9 s, so there is no third option.

The one thing that relaxes both at once is **speed**, and that is measured, not
argued:

| deck exit | wall margin |
|---|---|
| with the 32 m turbo (240 km/h) | **−6.15 m** |
| turbo removed (193 km/h) | **−11.55 m** |

**0.115 m of margin per km/h.** Closing the remaining 4.4 m needs roughly
**+38 km/h at the wall — about 280 km/h**, against 294.7, the fastest speed ever
observed anywhere on this map by any tape.

And the map is built so that it will not give it: on this deck the only source
of that speed is falling, and falling is exactly what puts the car under the
opening. That is the shape of the negative — not "the search did not find it"
but **"the two things the opening needs are bought with the same currency."**

*(The slow-line hypothesis — brake, turn tighter, exit further east — is refuted
by the same table: less speed is monotonically worse, and 90 braked variants are
all worse than every unbraked one. The turn is real, though: at 65 m/s a car
needs ~120 m to turn 28°, which is why every tape leaves the 12 m-wide deck at
x ≈ 862–890 instead of at its east end at 909.)*

---

## 6. The descent, which is both the attempt and its own control

Searching the real map directly is a valley: everything DNFs against the screen.
So the search was run against a LADDER of maps with the screen displaced by k
metres diagonally, each stage seeded from the tape that won the stage above it.
Every stage that lands is a positive control that the search finds the route
when the geometry allows; the stage that will not land is the frontier.

| relief k | outcome | time on that map | banked as |
|---|---|---|---|
| 14 m | this morning's best tape passes | 17.518 | (`ml_probe_air_17518_*`) |
| 9 m | best hand-built tape passes | 17.554 | `imt3_SEED_V36_T1550_W-127_needs_9m_relief` |
| 8 m | search lands it, 64 % of candidates finish | 17.515 | `imt3_relief_8m_win_k8` |
| 7 m | lands, 62 % | 17.518 | `imt3_relief_7m_win_k7` |
| 6 m | lands, 29 % | 17.768 | `imt3_relief_6m_win_k6` |
| 5.6 / 5.4 / 5.2 / 5.0 | lands each time | 17.795 → 18.092 | `imt3_relief_5.*m_*` |
| 4.8 m | lands only with wide operators and a hotter chain | 18.223 | `imt3_relief_4.8m_*` |
| 4.6 m | lands, 5 % | 19.480 | `imt3_relief_4.6m_19480` |
| 4.4 m | lands | 19.644 | `imt3_relief_4.4m_d4_k4.4` |
| **4.2 m** | **7.5 M evaluations, 170 workers, 55 minutes — nothing** | — | — |
| 0 m (the real map) | nothing, in every configuration tried | — | — |

Three things that table says beyond the frontier.

* The finish rate collapses from 64 % to 5 % over four metres: this is a closing
  aperture, not a plateau.
* **The last tapes are not clearing the wall, they are CLIPPING it.** The 4.4 m
  winner does 19.644 on that map and **17.618 on the same map with the screen
  deleted** — same inputs, two seconds of difference, all of it paid at the
  contact. So the frontier is where a tape stops surviving the graze, which is
  strictly easier than clearing, and the honest reading of "4.4 m" is *the
  displacement at which a clipping tape still finishes*.
* **It is not monotone.** The 4.8 m winner passes at 4.8 and **fails at 5.0**:
  more relief moves the car's landing, and the flag is small enough to miss.
  Passing is a conjunction of two needles, not one.

---

## 7. The second needle

Of 358 hand-built candidates, only **20 finish even with the screen deleted**.
The flag fires at (991.6, 56.6, 664.9) and does not fire at (995.8, 55.4,
671.7) — 8.7 m away and 7 m too far north.

Any wall-margin ranking that does not first filter on "finishes with the screen
removed" is ranking tapes that were never going to finish. Two searches spent an
hour learning that: they maximised the margin by flying a line that cleared the
wall and sailed past the flag.

Also measured, and unexplained: **remove the `GateSpecial32mNoEngine` and the
human's own run DNFs.** The gate that costs him 2.5 s of dead-engine coasting is
somehow load-bearing for his finish.

---

## 8. What this changes for the next arm

* **The run-up is still the lever, and now it has a price.** Arm 6 said "we reach
  the launch 50 km/h down" and priced it against a route that could never pay.
  The hole prices it exactly: **+38 km/h at the deck exit and the map is
  beaten**, because the endgame behind the hole is worth ~17.5 from a mediocre
  prefix and the flick has 5.65 s in hand at D767.
* **Do not attack the wall from the deck's south edge.** The exit point is the
  whole problem and it is set by the turn radius, not by the flight.
* **Keep the relief ladder.** It converts "the search found nothing" into
  "4.4 m", which the next idea can be measured against in ten minutes instead of
  a day. `diag/k*.Map.Gbx` is rebuilt by one `tmmaps shift` per rung;
  `imt3_map_relief9m.Map.Gbx` is banked as a worked example.
* **THE LEDGE IS REAL — see §11. `mapgeom plumb`, calibrated against this
  map's own recording to 0.1 m, finds a continuous ASPHALT surface at
  **y = 125.5** spanning x 782-846 and z 724-756 -- the canopy row at
  (765..893, 119, 740). That is **11.5 m above the launch deck**, against a
  height deficit of 5-6 m. Nothing has ever been on it, and my attempt to put
  a car there died on a FAILED YES-CONTROL, so it is unresolved rather than
  negative. Settle it first.

---

## 9. Instruments built here (all in the repo, branch `imt3-267460`)

| | |
|---|---|
| `tmsearch --gate/--key` | the STATE OBJECTIVE from `SEARCH.md` §5.1, built as specified: a third `Outcome` variant, a recording box in the child, `-(500 + miss)` outside it, and the **decoy test printed before the first candidate** (here: −6.2 for the seed against −87.5 for the parked car) |
| `--key corner:x=..,y=..` | worst margin against per-axis thresholds. `near:` is a **decoy with a mechanism** on this map: its level sets trade easting for height, and the search duly bought 55 m of easting with 46 m of height and flew into the wall lower down |
| `tmmaps shift` | displace a structure by a known amount, then re-read the written map and require every object to have moved. Turns "is it in the way" into "by how many metres" |
| `ghost tape set` | hold steer/accel/brake over a tick range; same-as-previous packets made explicit first, and the file read back and checked |
| `tmtraj route` | where the car was, EVERY interpolated crossing of a plane, closest approach, and `--margin` — the frontier of a family at a plane, on the same number `corner:` scores |

Two repo defects found on the way, both since fixed independently by another
arm: `tools/search` did not compile against the post-audit `ghost`/`tmtraj`, and
`tools/tmmaps` was in no cargo workspace at all.

**A caveat on my own instrument:** the gate key at z = 692 is a usable gradient
and **not** a faithful proxy for the relief requirement. The tape needing the
least relief (5.8 m) has a WORSE key (−8.44) than one needing 9 m (−6.15). The
objective that actually descends is "finish on a relieved map"; the key is only
what gets a search out of band 0.

---

## 10. Honest accounting

| | |
|---|---|
| incumbent | **21.022**, unchanged — nothing published, nothing submitted |
| author time | 16.888 — not beaten |
| what the hole is worth, from a mediocre prefix | **~17.5** |
| the gap, as geometry | **4.4 m of diagonal screen displacement** (14 m this morning) |
| the gap, as speed | **+38 km/h at the deck exit**, against a map maximum of 294.7 |
| my own hypotheses killed by my own measurement | 4 — the opening is a height problem; the slow line; `near:` as a key; the gate key as a proxy for the relief |
| prior claims corrected | 1 — "the only reachable opening puts the car past the flag" |

Nothing was submitted to any leaderboard. The map is **OPEN**.

---

## 11. THE LEDGE IS REAL, AND I COULD NOT PUT A CAR ON IT

The lead named in §8 is confirmed as GEOMETRY and unresolved as a ROUTE.

`mapgeom plumb` reads collision surfaces straight out of the server's own
packs, with no engine and no car. Calibrated twice against this map's own
recording, at `yoff 0`:

| where | the car's own y | the model's surface |
|---|---|---|
| (762, 722), race 13.5 | 105.83 | **105.649** Asphalt |
| the spawn, (909, 755) | 140.73 | **140.828** Asphalt |

0.1–0.2 m on both. The model is the map.

**And it says there is a second deck.** Above the run-up deck (Asphalt at
**y = 114.0**) there is a continuous **Asphalt** surface at **y = 125.5**:

| | |
|---|---|
| in x | continuous 782 → 846 at 125.514–125.515, then nothing at 862 |
| in z | continuous 724 → 756 (with a step up to 129.8 / 133.8 at z 744–748) |
| height over the deck | **11.5 m** |

That is the row of `CanopyCenterFlatBase` blocks at (765/797/829/861/893, 119,
740) — a 64 m × 32 m roof, **Asphalt, not decoration**, sitting eleven and a
half metres above the launch deck. The height half of the wall deficit is
5–6 m. Nothing in this project has ever been on it.

**Why this is not a result.** I tried to put a car there — moved the spawn block
above the roof, drove forward with a throttle-only tape — and the answer is
about my instrument, not the roof:

* `fk trace` **could not locate the car**: `verr 1.0000 m/s at mean speed 13.9`,
  then a readout with a constant 3.6 km/h, which is the known 267460 locate
  defect and not a car.
* So I switched to an oracle-only test — a relocated Goal high enough that a car
  on the roof fires it and a car that fell to the deck does not — and **the
  yes-control failed**: the gate placed directly on top of the spawn returns
  DNF, at five placements.

**When a control fails the null is about the instrument.** Both the roof test
and its deck control returned DNF, and with a dead yes-control neither of those
DNFs means anything. The roof is UNRESOLVED, and it is the first thing the next
arm should settle.

Two ways in, for whoever picks it up. `tmmaps dropscan` (main) drives a car off
a moved spawn and reads the landing from the live engine, which is the exact
case here. Or fix the locate first: this map's documented recipe is
`FK_QERR_MAX=0.5 FK_MIN_SPEED=8 FK_BOUNDS="600,1300,10,260,560,860"` — I passed
the first two and not the bounds box, and the bounds box is the one that makes
this map's locate work.
