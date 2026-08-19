# 279209 — "Fall 2025 - 13 Reverse CP1 End" — RESULT

**The author time is beaten, and the interesting half is that a keyboard can do
it.**

| | ms | vs AT | what it is |
|---|---|---|---|
| human online WR (`jujumasterr`, 334 records) | 6604 | +9 | best of the field |
| **author time (AT)** | **6595** | — | `in-.-`'s driven validation lap |
| our keyboard-only tape, **19 input events**, 3 steer values | **6595** | **0** | §3 — a human could drive this |
| our 7-value action-key tape | 6591 | −4 | §3 |
| **our best, unconstrained** | **6578** | **−17** | §2 |

All times re-validated with `tmtas validate --map <ABS map> <ABS ghost>` against
the banked copy of the map, with human ghosts carried in the same batch as
identity controls returning 6604 / 6608 / 7029 exactly. The closing batch
re-validated **all 25 banked tapes at once**; every one returned its claimed
millisecond. **Zero failed re-validations in the whole session**, so nothing was
written to `tm-loop/phantoms/`.

Artefacts: `evidence/` (tapes, dumps, tolerance tables), `lowinput/` (the
drivable family), `PLAN.md` (the attack plan and its corrections), `NOTES.md`
(the instrument work), `tmtas-rs-279209-src.tgz` (the whole Rust workspace
including the new `tmpop` binary and `tmmaps places`).

---

## 1. The headline for a driver, before any of the detail

> **On the ice run-down, about 1.7 seconds in, where the slope flattens out and
> the nose goes light — lift off the throttle for four ticks (40 ms), then back
> on. That is worth 12 milliseconds, it is the easiest input in the entire lap
> to get right, and nobody in a field of 334 does it.**

Everything else in our keyboard tape is what the rank-3 human already drives.
The AT falls to that one lift plus turning in 40 ms earlier at the same place.

---

## 2. The unconstrained result: 6578 ms

### How it was reached

Seeded from the human WR (`r001`, 6604), `tmsearch` on the plain headless-server
oracle, Metropolis at T = 3 ms, stock operator mix, window 140 ticks, one island
group per arm with 2 % migration, explicit distinct `--root` per process.

- **6592 at t = 17 s** of the first arm — the AT fell almost immediately.
- 6578 after ~10 minutes across four arms, reached **independently twice** (two
  RNG seeds) and later a **third time from a completely different seed**
  (`r002`, a run that is 39 ms ahead of the WR at mid-corner and still
  loses). Three independent lineages converging on the same millisecond is the
  strongest evidence available that this is the route's optimum, not one arm's
  luck.
- After that: **271,530 further evaluations across three arms found nothing.**
  The integer-millisecond objective is exhausted at 6578.

### Where our 17 ms comes from, against the human WR

Timed through planes taken normal to the WR's own velocity (`tmpop refstations`
on `fk btraj` per-tick trajectories, so the comparison is a progress measure and
not an axis artefact):

| station (ms of the WR's lap) | our cumulative delta |
|---|---|
| 330 … 1981 | **0** |
| 2311 | +1 |
| 2972 | −2 |
| 3632 | −4 |
| 4293 | −6 |
| 4623 | −10 |
| 4953 | −13 |
| 5613 | −20 |
| 5944 | −20 |
| finish | **−26** (vs the WR; −17 vs the AT) |

**Diffuse, not a trick.** We are dead level with the human WR for the first two
seconds and then take one to four milliseconds per half-second all the way
through the sweeper. Classified per UNBEATEN.md §A: **better carry the field
could in principle match**, not an undiscovered route.

Side by side (`tmpop abcmp`), the mechanism is visible and small:

- at 2.5–2.75 s we hold **+15/127 of right lock** where the human holds zero;
- from 4.25 s on we are **+2 to +3 km/h** through the whole sweeper;
- through the middle of the corner we run up to **1.1 m wider in z**, converging
  back onto the human's line by the flag;
- both tapes are on the ground for every tick and at full lock (−127) from
  4.0 s to 6.5 s. There is no air phase anywhere on this map, so
  UNBEATEN.md §B's "reactor/boost/flight" trigger does not apply.

### Sub-millisecond progress after the plateau

The oracle reports whole milliseconds and the car crosses the finish plane at
58 m/s, so **1 ms = 5.8 cm** (17.2 ms/m — measured, and almost identical to the
17.0 ms/m the 279218 agent measured on its sibling map). Two tapes 5 cm apart in
true progress report the same number.

Because the Goal on a CP1-End map is a *relocatable item*, the finish plane is
ours to place. `tmmaps places` (new) slides it along the direction of travel and
`ratchet_loop.sh` re-aims it at the champion's own staircase edge after every
step, so the smallest real gain reads as a whole millisecond. Head to head from
the same 6578 champion, same cores, same box, same 10 minutes:

| arm | result |
|---|---|
| plain map, 3 arms, 271,530 evals | **0 improvements** |
| vernier ratchet, 3 arms | 3 adopted steps, champion **20 mm further along** (0.34 ms) |

The ratchet's adoptions still read 6578 on the untouched map — as they must —
but they are measurably further along on a 5 mm ladder. Run to nine rounds it
moved the champion from **12 cm to 7 cm** short of the next millisecond, i.e.
**0.86 ms of real, oracle-adjudicated progress that the integer millisecond
cannot show**, and then it too slowed down. Measured on the fine ladder at the
end (`tmmaps places --rank 6577`), all three of our 6578 tapes ranked:

| tape | reads | gate offset still reporting 6577 | continuous time |
|---|---|---|---|
| ratcheted champion | 6578 | **+0.0700 m** | ≈ **6578.20 ms** |
| first tape to reach 6578 | 6578 | +0.1200 m | ≈ 6579.06 ms |
| independent lineage (from `r002`) | 6578 | +0.1250 m | ≈ 6579.15 ms |

**Honest reading: 6578 is very close to this route's floor.** What remains is
tenths of a millisecond, and the two independent lineages landing 0.09 ms apart
after hours of separate search says the route itself is out of ideas, not that
the search is.

---

## 3. The low-input family — the part that matters to a human

### 3a. The keyboard alphabet was read off the human tapes, not guessed

`r003_6608` is **rank 3 on the leaderboard and a pure keyboard run**: steer
alphabet exactly {−127, 0, +127}, **17 input change events for the entire lap**,
4 ms off the human WR. `r075_6737` is the same shape with 14. Across 16 sampled
tapes spanning ranks 1–265 the modal steer values are −127 (46 %), 0 (42 %) and
+127 (11 %), and brake appears in **one** tape for **six ticks**.

So the ladder below is the real one for this map, and the keyboard rung is not a
theoretical construct — a human is already on it, near the top of the board.

### 3b. The family

Every tape re-validated on the untouched map. "Events" counts input CHANGE
EVENTS, not ticks (a value held 300 ticks is one event).

| rung | steer alphabet | events | ms | cost vs floor | file |
|---|---|---|---|---|---|
| unconstrained (the floor) | 111 values | 198 | **6578** | — | `evidence/BEST_6578_ratcheted.Ghost.Gbx` |
| 7-value action keys | ±127, ±85, ±42, 0 | 45 | **6591** | +13 | `lowinput/AK7_6591.Ghost.Gbx` |
| 5-value action keys | ±127, ±64, 0 | 38 | 6595 | +17 | `lowinput/AK5_6595.Ghost.Gbx` |
| **keyboard** | **−127, 0, +127** | **19** | **6595** | **+17** | **`lowinput/KB_SIMPLE_6595.Ghost.Gbx`** |
| the best human keyboard run | −127, 0, +127 | 17 | 6608 | +30 | `ghosts/r003_6608.Ghost.Gbx` |

Three things worth saying plainly:

- **The keyboard rung equals the author time exactly.** 6595, three steer
  values, 19 decisions, no brake.
- **The 5-value rung is not better than the 3-value rung** — same 6595, and it
  needs 38 events instead of 19 to get there, so it is strictly worse to drive.
  The 7-value rung does find 4 ms (6591) but needs 45 events. Intermediate
  steering values buy almost nothing here unless you have essentially all of
  them: the gap from 7 values to 111 values is 13 ms, the gap from 3 to 7 is 4.
  That is a measured negative result (four arms, ~40,000 evaluations each,
  seeded both from the keyboard tape and from the analog champion quantised
  down), not an assumption.
- **On this map the alphabet is not what costs you; the event count is.** 19
  digital decisions get within 17 ms of a 198-event analog tape.

### 3c. The keyboard tape, in full

`race_ms` is the on-screen clock. Everything before 0 is the countdown.

| # | race ms | input | held |
|---|---|---|---|
| 1 | **30** | full LEFT | 710 ms |
| 2 | 740 | centre | 60 |
| 3 | 800 | full RIGHT | 100 |
| 4 | 900 | centre | 120 |
| 5 | 1020 | full RIGHT | 150 |
| 6 | 1170 | centre | 80 |
| 7 | 1250 | full RIGHT | 190 |
| 8 | 1440 | centre | 60 |
| 9 | 1500 | full RIGHT | 100 |
| 10 | 1600 | centre | 90 |
| 11 | **1690** | full LEFT | (held through the lift) |
| 12 | **1760** | **THROTTLE OFF** (steer stays full LEFT) | 40 |
| 13 | 1800 | throttle back ON | 610 |
| 14 | **2410** | full RIGHT | 150 |
| 15 | 2560 | centre | 230 |
| 16 | 2790 | full RIGHT | 830 |
| 17 | 3620 | centre | 60 |
| 18 | 3680 | full LEFT | 2930 ms, to the flag |

Throttle is on for every other tick of the lap; the brake is never touched.

### 3d. Where its 13 ms over the rank-3 human comes from — decomposed

| tape | ms |
|---|---|
| `r003` as driven | 6608 |
| `r003`'s steering with the turn-in at 1690 instead of 1730 | 6607 |
| ... plus the 40 ms throttle lift at 1760 | **6595** |

**One millisecond is the earlier turn-in. Twelve are the lift.**

*Why the lift works.* The rank-3 human's own telemetry shows the front
suspension unloading right there: `fl_dampen`/`fr_dampen` go from 0.008
(compressed) at 1650 ms to 0.118 (fully extended) at 1850 ms while the rears
stay loaded, and the car's altitude stops falling — the downhill flattens out and
the nose goes light over the transition. Closing the throttle for 40 ms puts
weight back on the front wheels exactly as the left-hander is being asked for,
the front bites, and the car takes a better line into the corner. It is ordinary
weight transfer, on the one part of the track where the surface is ice and the
front is unloaded at the same time.

*It was found, not designed.* The search discovered it on its own (`op=acc@327
val=0` — a throttle toggle), as two separate 20 ms lifts; the single 40 ms lift
in the delivered tape is the simplification, and it is worth the same 12 ms.

### 3e. Tolerance: how much slack each decision really has

Every change event in the keyboard tape was moved ±1…±6 ticks, one at a time,
and re-validated through the plain oracle (263 variants, identity control exact).
Cost in ms of being off by one tick (10 ms):

| # | race ms | input | 1 tick early | 1 tick late | verdict |
|---|---|---|---|---|---|
| 1 | 30 | LEFT | +5 | **0** | very forgiving; late is free out to +30 ms |
| 2–10 | 740–1600 | the wiggle | 10–19 | 10–22 | ~1–2 ms per ms; recoverable |
| 11 | 1690 | LEFT | **+62** | +11 | never early |
| **12** | **1760** | **THROTTLE OFF** | **+7** | **+7** | **the most forgiving input in the lap** |
| **13** | **1800** | **throttle ON** | **+7** | **+6** | same |
| **14** | **2410** | **full RIGHT** | **+30** | **+116** | **the crux — see below** |
| 15 | 2560 | centre | +106 | +29 | tight |
| 16 | 2790 | RIGHT | +12 | +154 | never late |
| 17 | 3620 | centre | +73 | +9 | never early |
| 18 | 3680 | LEFT | +64 | +13 | never early |

And the two things a driver most needs to know:

**The lift is easy.** A separate 2-D sweep (569 variants: every start tick ×
every duration) shows a **40 ms lift starting anywhere between race 1730 and
1790 ms gives 6595** — a 70 ms window — and any lift of 10–40 ms anywhere in
1690–1990 ms is worth at least 7 of the 12 ms. Being 60 ms out costs under
10 ms. There is no way to hurt yourself with it.

**The flick at 2410 is the crux, and it always was.** Five ticks early or six
ticks late and **the run does not finish at all**; one tick late costs 116 ms.
A 2-D sweep of its start and duration (231 variants) finds no forgiving
alternative: the best off-nominal setting costs 6 ms and everything else costs
9 ms or more. This is not something our tape introduced — it is in `r003` and in
every other human tape, with the same ±1 tick window. **It is the reason 334
people are stacked between 6604 and 7029.**

*(For contrast: 3007 variants adding a second throttle lift anywhere else in the
lap improved nothing. The one lift is the whole story.)*

### 3f. Is the unconstrained 6578 tape drivable? No — and that is a fact about
our tape

198 change events and 111 distinct steer values, with **31 of its 79 movable
events having zero tick of slack**. Individually each is only worth 3–50 ms and
none of them DNFs, so it is not fragile — it is simply a dense analog ramp no
person can reproduce. Where the human tape has a digital flick at 2410, ours has
a smooth ramp 15→49→0 over the same 120 ms.

Per UNBEATEN.md §A, "precision-bound" is the start of the work, not the end, and
§3b–3e is the work: the forgiving version exists, it is 17 ms slower, and it
equals the author time.

---

## 4. Sector-by-sector guide, off visual cues

Speeds and positions from the delivered keyboard tape's own per-tick trajectory.

**S1 — the ice run-down (0 → 1.7 s, 0 → 99 km/h).**
You spawn on ice pointing down a steep straight; the road falls about 5.5 m over
the first 20 m of travel. Throttle to the floor and leave it there.
**Immediately at the lights, full LEFT and hold it for about seven tenths of a
second.** Being late with this is free; being early is not. The car barely
moves sideways in the whole section — under a metre — so the steering here is
holding the car straight on ice, not aiming it.

**S2 — the wiggle (0.74 → 1.6 s, 32 → 96 km/h).**
Four short right stabs with the wheel centred between them, roughly at 0.74,
1.02, 1.25 and 1.50 seconds, each about a tenth of a second. This is
counter-steering on ice: the field all does something like it and the exact
pattern is not critical — each stab is worth 10–20 ms if you are a tick out, and
nothing DNFs. Copy the rhythm, not the ticks.

**S3 — the crest, and the twelve free milliseconds (1.69 → 1.80 s, 99 km/h).**
**This is the discovery.** Watch for the moment the downhill stops falling away
and the road goes flat — the horizon steadies and the nose comes up. Right there:
**full LEFT, and about half a tenth of a second later blip off the throttle for
four ticks, then back on.** You have a 70 ms window for the lift and it costs
under 10 ms to be anywhere in a 300 ms window around it. Do not turn in *early*
here — 10 ms early costs 62 ms.

**S4 — the crux flick (2.41 → 2.56 s, 113 → 120 km/h).**
Still on the run-out, before the corner proper. **Full RIGHT for exactly 150 ms,
then centre.** This is the hardest input on the map and the one that separates
the leaderboard. Five ticks early and you do not finish; one tick late costs
116 ms. Practise this one on its own. If you are early, shorten the flick; if
late, lengthen it — the diagonal keeps you within about 10 ms.

**S5 — the long right (2.79 → 3.62 s, 130 → 160 km/h).**
**Full RIGHT, held for eight tenths of a second.** Release it as the road starts
to swing left. Never late on the release: 10 ms late costs 154 ms.

**S6 — the sweeper (3.68 s → flag, 160 → 214 km/h).**
**Full LEFT and hold it for the remaining 2.9 seconds.** One input. The road
rises about 11 m through here and the car accelerates the whole way. Do not
correct, do not lift. Our unconstrained tape gains its whole advantage in this
section by carrying 2–3 km/h more and running about a metre wider through the
middle — worth trying, but it is carry, not a line change you can point at.

**The flag.** The finish plane is at world x ≈ 1040.68, crossed at 214 km/h.
1 ms is 5.8 cm. The gate's trigger window is 49 m wide laterally and every human
crosses 17–18 m inside its near edge, so — unlike some maps in this family —
there is **no invisible boundary to shave here** and nothing to gain by
tightening the exit. Aim for speed, not for the edge.

### What will take real practice

The flick at 2.41 s, and nothing else. S1, S2, S3, S5 and S6 are all
±20 ms-or-better tolerant, and S6 is a single held key for 2.9 seconds.

---

## 5. Negative results, so nobody re-runs them

1. **An `edge`-heavy operator mix is worse, not better.** Predicted at
   1.5–2.5× because `edge` improves 12.4 % of unbiased single moves from the
   human seed against 1.6 % for `cos`. Screened per PROTOCOL — four arms
   concurrently, two RNG seeds each, 15 min, AUC primary:

   | arm | AUC (mean best-so-far, ms) | finals |
   |---|---|---|
   | stock `mix` | **6581.88** | 6578, 6578 |
   | `edgy` | 6584.58 | 6582, 6582 |

   **+2.70 ms, i.e. worse, on both seeds** (control seed-to-seed spread here is
   only 1.2 ms, so it is well outside noise). REJECTED. The lesson is general:
   the dump measured the neighbourhood of the *human seed*, and a minute into
   the search the incumbent is no longer a human tape.

2. **The finish gate's trigger window is not the limiter on this map.** Its low
   edge is at about `item_z − 24.5 m` and every human crosses 17–18 m inside it;
   the corner's centre is at (1052, 1206) so a tighter line means *lower* z, yet
   the faster runs cross *higher*. Nobody is being held off a tighter line.

3. **The fork server's sub-tick plane objective would have been unusable here,
   and the cheap self-test says so.** Six tapes' positions at their own
   validated finish millisecond span **1.1 m** — 19 ms at this map's exchange
   rate, far larger than the gains being chased — because the trigger is
   body-based and the tapes differ in attitude at the flag (the outlier is the
   one whose yaw differs by 1.2 rad). The gate-relocation vernier is adjudicated
   by the real trigger against the real car body and cannot have this failure
   mode. The fork server was not used for scoring at any point; `fk btraj` was
   used off the hot path for per-tick trajectories, where it works (664 ticks,
   validated time matches).

4. **Naive quantisation of the analog champion does not produce a drivable
   tape**: to {−127,0,127} it DNFs, to 5 values 6852, to 7 values 6966. The
   low-input family has to be *searched under the constraint*, and seeded from a
   human keyboard tape rather than from the analog optimum.

5. **A second throttle lift is worth nothing** — 3007 placements across the
   whole lap, best gain 0 ms.

## 6. Caveats on the instruments

- **Negative gate offsets are invalid.** The Goal item sits at x = 1024.0, the
  low edge of its declared cell; sliding it below that leaves the cell while the
  record still says otherwise, and the readings go non-monotone. Only slide
  toward the oncoming car.
- **One rung of a gate ladder can jump.** At exactly +1.0000 m one of our tapes
  read 6568 where its neighbours read 6564 and 6563 — reproducible byte for
  byte, and not present for five other tapes at the same placement. The
  registration is a car-box overlap and that tape is the most yawed at the flag.
  Read the vernier over three consecutive rungs, never one; `ratchet_loop.sh`
  enforces it.
- The `PLAN.md` §5b tick-to-millisecond mapping was wrong by the tape's 1580 ms
  countdown offset and is corrected in place at the end of that file.

## 7. Rules observed

Nothing was submitted to any Nadeo leaderboard. Every reported number was
re-validated through the plain oracle against the banked map with human ghosts
as identity controls in the same batch. Rust only — the population analysis,
the tolerance machinery and the gate vernier are new Rust binaries
(`tmpop`, `tmmaps places`) in the workspace, not scripts; shell is used only to
drive processes. All HTTP was rate-limited (1.6 s between ghost downloads) and
carried the descriptive research User-Agent.

## 8. Toolchain: a deliberate non-adoption

The hardened build (`tm-map2/tmtas-rs-hardened.tgz`) was offered near the end of
the session and **was not adopted here, on purpose**. Its four fixes are: the
per-worker fork resume tick, the atomic root claim, a guard that re-validates
every banked improvement through the plain oracle, and the merged sub-tick
plane. On this map's configuration none of them bind:

- nothing was ever scored through the fork path or through `--plane`, so fixes
  1 and 4 have no surface here (and §5.3 is the measurement saying the plane
  objective would have been *wrong* on this map anyway);
- this tree already carries the per-pid default root with a live-owner refusal;
- the guard's behaviour is what was done externally on every artefact all
  session — ~40 validations, all exact, plus a 105-ghost identity control.

Merging a divergent branch over this session's local work (the new `tmpop`
crate, `tmmaps places`, `--quant` in the classic search path) with under an hour
of lease remaining was the riskier move. Recorded as a decision with reasons
rather than left as an omission. **Any future fork-scored arm on this map should
take the hardened build first**, and should run the boundary-stress acceptance
test (`--lo <boundary> --window 60 --stride 400`, twelve minutes) rather than
trusting a quiet run.

## 9. What the next agent should try

In rough order of expected value:

1. **Nothing on the unconstrained line, probably.** Two independent lineages and
   ~1.5 M evaluations put it at 6578 with 0.09 ms between them. If someone does
   push, the ratchet in `ratchet_loop.sh` is the only instrument that still
   shows a gradient, and it wants longer rounds (240 s was still too short to
   fill a round with candidates by round 4).
2. **A keyboard tape below 6595.** Two arms from `KB_SIMPLE_6595` at T = 3 and
   T = 6, 30 minutes each, found nothing. The obvious unexplored direction is
   the crux flick's *structure* rather than its timing — every human and every
   tape here uses one full-lock right pulse at 2.41 s, and no one has tried
   splitting it.
3. **The countdown.** The whole first 158 ticks of the tape are inert and the
   pre-steer the search left there is worth exactly zero (five separate deletion
   tests, 0 ms each). But one accepted improvement did land at tick 146 in an
   earlier arm, so the countdown is not completely dead and has not been
   systematically swept.
4. **Do not re-run** the edge-heavy operator mix (§5.1), the gate-window
   hypothesis (§5.2), the fork/plane objective (§5.3), naive quantisation
   (§5.4), or a second throttle lift (§5.5). Each has a number attached.
