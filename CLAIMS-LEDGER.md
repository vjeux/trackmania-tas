# Claims audit — ledger

Branch `claims-audit`, node `61734.od.fbinfra.net`, repo
`/data/users/vjeux/trackmania-tas` off `main` = `0836d2c`.
Convention adopted: `CLAIMS.md` at the repo top level — MEASURED (control named)
/ INFERRED (inference stated) / UNKNOWN (open task) / SUPERSEDED (points
forward).

Scope swept: 56 `.md` files (36 map dirs, 6 top-level docs, 9 `tools/` docs,
`trainer/`), plus the project's memory keys.

## Who owns what — three other arms are editing this repo tonight

| files | arm | branch |
|---|---|---|
| 186935, 227654, 286279, 134672, 238835 map READMEs | render | `render-longmaps` (70582) |
| `tools/mapgeom/MAPGEOM.md` | mapgeom | `mapgeom2` (88360) |
| `tools/search/SEARCH.md` | state-objective | `state-objective` (509d54a) |
| **everything else** | **this audit** | **`claims-audit` (61734)** |

Their findings are recorded below anyway, marked `[owned elsewhere]`, so the
ledger is complete even though I do not edit those files.

---

# 0. FINAL STATE — read this, then the sections for the working

Everything below is the audit as it happened, including two claims of mine that
turned out to be wrong. That is deliberate: the wrong turns are the part worth
reading, and rule 4 of this project is that a retraction is published next to
the thing it retracts rather than instead of it. **This section is the settled
tally.**

## Claims touched, by category

| category | found | wrong and corrected | verified sound, left alone | still open |
|---|---|---|---|---|
| **1 — instrument reported as world** | 12 | 10 | — | 2 |
| **2 — null with no positive control** | 3 flagged | 2 restated | **8 verified, control cited** | 1 |
| **3 — n = 1 / scope beyond evidence** | 8 | 7 | 1 (already right) | — |
| **4 — superseded numbers standing** | 8 | 7 | — | 1 (146612, needs the map) |
| **found by running checks, not on the brief** | 4 | 3 | 1 (a checked negative) | — |

**17 documents edited**, **31 of 56 read in full**. The rest were swept by four
pattern searches plus four machine checks that read every published file. §G is
explicit about which is which — the unread pages are **not** claimed clean.

## The two claims of mine that were wrong

1. **210218 "the two files carry one recording" — RETRACTED.** Published as a
   warning box, then settled against the engine: over the **1735 samples where
   the records agree bit for bit, the two cars are 0.0001 m apart**, 1734 of
   them bit-identical. Both files sound. What misled me: two whole-file *rates*
   that agreed, 93.8 % and 94.1 %, which say nothing about whether they are the
   same samples. §I.
2. **267859 "the directory holds only `TAS_10859`" — WITHDRAWN.** All seven
   files are present and each declares its own name. I read it off a truncated
   capture and did not open the directory. §H7.

Plus **six instrument errors caught by controls before they reached a page** —
four in §I, two in §F1.

## The duplicate question, settled

46 `corpus dup` refusals → **14** inside 203330's measured inert window, **3** at
separation exactly 0.000000 m (two already documented by hand on the 227654
page), **5** documented 286279 provenance, **24** settled against the engine as
38 pairs: **35 measured innocent, 1 inconclusive at 0.001 m, 2 untested, ZERO
defects**, over 143 traces. §I.

## What is genuinely still open

| item | why, and what would settle it |
|---|---|
| the ≈0.5 mm **orientation** half | regressed under the same fix; owned by the carrier-bytes arm, flag default-off |
| 238835 + 267859 regeneration | no file locates on either turtle map at any of 14 fork points — **diagnosed, not skipped** (§I) |
| 228607 transform | repaired on `regen-sweep-b`, correctly held for the wrong-copy fix |
| is `g` genuinely per-map | the intercept is fitted on one map; needs a second with a long enough arc |
| 146612's 38.968 / 39.183 | the map is not redistributed and this box has no copy |
| 173691's ground-contact byte | `ghost regen` does not write byte 89; reading it out of memory is an open task |
| the wheel-block offset (§J) | 540 in one arm's frame against 248 in another's, unexplained |

---

# A. CATEGORY 1 — a claim about the world that is really a claim about our instrument

The one that costs days.

### A1. `186935/README.md:8` — "there is no car in the file" — **REFUTED, first-hand** [owned elsewhere]
> `BEST_793893` carries no `CSceneVehicleVis` entity at all

Reproduced on this box against the published file:
```
$ tmtraj check 186935-magnet-trial/replays/BEST_793893.Ghost.Gbx --race 793.893
  PASS C1  every position and velocity component is finite (15533 samples)
  FAIL C2  the car travels 0.0000 m over 1 distinct points -- this is not a run
```
The entity exists, with **15 533 samples**; every position is (0,0,0). A zeroed
memory slot, not a missing entity — and the project's own `tmtraj check` names it
in one line, and did the whole time. "No entity" reads as unrecoverable; "zeroed
slot" reads as regenerable, which it was.
**Action: render arm has regenerated it (15 878 samples, 10 881.6 m). Their fix.**

### A2. `227654/README.md` — "the carrier is truncated" [owned elsewhere]
One car split into **27 entities** at the respawns, tiling 0 → 147.000. Every
reader in the project takes the entity with the most samples, which on a
27-player server replay is one 365-sample fragment. Three attempts concluded
38 s of the race was unrecordable. **Render arm's fix.**

### A3. `README.md` (top level) — the 173691 row is the retired framing, verbatim — **MINE, high cost**
The map's own page was corrected at `0836d2c` and says so loudly. The top-level
page still says:
> That deck is a dead end: 0 finishes in 2 400 fuzzed tapes from it against 515
> from one storey up, and nothing lands between y = 122 and the stand fronts at
> y = 162. **The finish is reachable only from the course's own high road at
> y = 234.**

All three clauses are superseded by `173691-.../ENDGAME-MEASURED.md`: ten finish
gates span **y 130…194 and both rows fire**, the lowest measured firing is
**y = 133.97**, and there is a drivable ledge at 130.2 → 134 that finishes the
map. The target is **15.7 m** above the deck the jump reaches, not 48 — and
"reachable only from y = 234" is false.
**This is the single highest-cost live instance of the error in the repo**: the
top-level page is what a reader meets first, and it still carries the sentence
the map page spent an arm retiring. **Action: rewrite the row. DONE.**

### A4. `285885/README.md:181` — "gravity here is per-map" — **CORRECTED with arithmetic, MINE**
> One more measured local fact: **free fall on this map is −24.308 m/s²**, not
> the −25.20 measured elsewhere. Gravity here is per-map.

This is a constant-`g` fit reported as a property of the map. The engine's own
law (memory key `tm2020-free-fall-is-linear-drag.md`, fleet notice F1994616772,
confirmed on four tapes across three maps) is

    a_y = −g − k·v_y      g = 24.78 ± 0.10 m/s²,  k = 0.032 ± 0.002 /s

under which a scalar "gravity" is just `a_y` sampled at whatever `v_y` the
measurement happened to span. The two numbers this page contrasts reconcile to
three decimals:

| quoted "gravity" | v_y it implies | `−24.78 − 0.032·v_y` |
|---|---|---|
| −25.20 ("measured elsewhere") | +13 m/s | **−25.20** |
| −24.308 (this map) | −16 m/s | **−24.27** |

So the *difference the page reports is real and the conclusion it draws from it
is not*: one law with no per-map term predicts both. Checked on this box against
the page's own published tape — `tmtraj motion TAS_50229.Ghost.Gbx --fit-g`
reports **"longest free-fall stretch: 2 samples, 14.770 .. 14.820 s"**, g =
21.4981. A 2-sample, 0.05 s lever arm cannot identify anything, and `tmtraj`'s
own README already says so: *"a fit whose lever arm is a few m/s of `v_y` cannot
identify a drag term and must not be quoted as if it had."*
**Action: restated as MEASURED (a_y at its v_y) + the reconciliation, with the
open question named. DONE.**

### A5. "~0.5 mm is the client-vs-server floor" — a NAME asserted about the engines, used for three different comparisons — **MINE, project-wide**
Sites: `tools/README.md:129`, `tools/tmtraj/README.md:80-83`,
`173691/README.md:147` and `:150`, `165922/README.md:41`,
`267460/README.md:22`, `276874/README.md:142`, `276877/README.md:140`,
`270051/README.md:20,37`, `279197/README.md:61`, `274191/README.md:33`.

The number is stable and useful. The *name* is a claim about the world (two
engines differing) and the corpus itself contradicts it, because the same
≈0.5 mm is used to name three comparisons that cannot all be the same quantity:

| document | what was compared | reading |
|---|---|---|
| `270051:37` | ours vs **two different humans**, and vs **its own** | 0.000500 m / **0.000000 m** |
| `173691:150` | ours vs **our own second regeneration** | 0.000497 m |
| `276874:142` | ours vs **a different readout path** | 0.48 mm |
| `tools/README.md` | ours vs a **downloaded human answer key** | 0.489 / 0.483 mm |

`270051` reads **0.000000 m** against its own copy where `173691` reads
0.000497 m against its own copy. Those are the same comparison with answers two
orders of magnitude apart, so ≈0.5 mm is not one floor.
The render arm has independently retracted their own version of this (three maps
at 0.489 / 0.511 / 0.501 are three readings of **one** quantity, probably the
fixed distance between two copies of the car struct, not three corroborations;
discriminator at `8ca8c2e7`, pre-registered).
**Action: keep every number, drop the causal name. `tools/tmtraj/README.md`'s use
is sound as-is — it is a *threshold* for `--near`, and a threshold does not need
to know its own cause; that one gets a sentence saying so rather than a
correction. DONE.**

### A6. `134672/README.md:206` — "this map's engine gather contains no wheel block at any window size" [owned elsewhere]
Under-determined, not refuted: `fk whl find` returning 0 is a cheap proxy for a
**bad car-copy pick**, and 267460 — the map where wheels *were* found — also
returns 0 when the chooser lands on a copy whose path is 0.0 m. The honest form
is "we have not found a wheel block on a verified pick here", which is UNKNOWN.

### A7. `134672` — "two cosmetic wheel channels stay the carrier's" [owned elsewhere]
Not cosmetic: another player's run, driving the tyre effects in a published
video. Render arm has rewritten it to say the effects are **absent** (written as
zero), which is a different and honest claim.

### A8. `GHOSTS.md:424-425` — "found the car zero times" — **MINE**
> 8 runs on the default ladder found the car once; 24 runs over seven
> hand-picked ladders found it zero times

"Found the car" is the *chooser's* verdict, and the chooser is the known-weak
part: the information is present in every gather (`fk whl carscan` recovers the
car from a junk run's own dump), so what varies is the pick, not the presence.
**Action: restated. DONE.**

### A9. `228811/README.md` — a necessary condition published as the firing condition — **MINE**
Reported by the state-objective arm with two independent counter-runs. The
published trigger is a conjunction (downward crossing, ≥ 85 m/s body-lateral)
measured over **1343 logged launches** — right about what every launch *has*,
unsupported as what *produces* one:
* Arm K reached side 98.2 m/s with −vz 30.9 on the deck downstream of the x = 80
  checkpoint — both terms clear — and **nothing fired**.
* Arm D closed position to 0.29 m and velocity to 3.60 m/s and stopped **53.8°
  away in attitude**.
The third term is attitude, and `TECHNIQUE.md` already says so in prose two
paragraphs from the table that contradicts it.
**Action: necessary-not-sufficient, with both counter-runs named. DONE.**

### A10. `MAPGEOM.md` — physics material names wrong from id 26 up [owned elsewhere]
27 is `Bumper_Deprecated`, not `RoadIce` (RoadIce is 74); 28 is `NotCollidable`,
not `Bumper` — so triangles the car cannot touch were eligible to be reported as
the surface under it. The 134672 "RoadIce 78 %" headline was carried entirely by
the map's embedded blocks. Conclusion survives at 97 %; the evidence as written
did not support it. Mapgeom arm's fix, and they report the exclusion's measured
effect on the 33-map corpus as **nothing** — a checked negative, correctly
recorded as checked rather than assumed.

---

# B. CATEGORY 2 — a null with no positive control

### B1. `210218/README.md:130-132` — three "0 finishers" rows, no control — **MINE**
| the weld … over every join point and phase | 77 | 0 finishers |
| re-phasing the tail after banking 467 ms in sector 14 | 64 | **0 finishers** |
| the same at the exact tick the run dies | 84 | **0 finishers** |

n = 77 / 64 / 84 with no row saying what *did* finish in the same batch. This map
is also the one where a corrupted objective is known to have been live
(`FINISH_BASE` = 1e8 makes a DNF at CP ≥ 10 outrank every lap, and this map has
16 checkpoints), which is exactly the condition under which a null means
nothing. **Action: marked UNKNOWN pending a control, with the reason named.**

### B2. `285885/README.md:171` — "3483 candidates, 0 improvements" — **MINE**
No control, and "all 78 splice handovers after the divergence are dead" is the
same shape. **Action: control named where one exists (the map's other nulls do
have them — the 424-station sweep fires ~40 on the human record, and the
gate-at-the-crossing-point validation fires at 40.964), these two marked
UNKNOWN.**

### B3. `285885/README.md:161-170` — the 797-probe rotation survey — **MINE**
> A 797-probe fan across the whole approach … found **580 airborne episodes and
> the nearest one comes within 82.6 m of the patch**; coverage was complete at
> 5 m out to ±40 m and 10 m out to ±80 m.

The survey's own coverage statement is the refutation: the rotation source that
exists is the fast line going **airborne at race 34.86–35.43, rolled 74° at
167 km/h**, and the page names that episode itself two paragraphs earlier
("both tapes reach 74° of tilt at 165 km/h, 142 m from the finish"). A fan
covering ±80 m around the patch structurally could not see a source 142 m away.
The lead is still closed, but by the **828 overrides in that window returning 0
tilted arrivals**, not by the fan. **Action: closure re-grounded on the
evidence that can carry it. DONE.**

### B4. Nulls that **do** have their control, and should be left alone
Recording these because "everything is doubtful" is the failure mode I was sent
to avoid. These are the good ones:

| document | the null | the control, as published |
|---|---|---|
| `145875:24,90` | 0 of 120 trials survive ±10 ms | *"The world record fails the same test identically, 0 of 120"* — published beside the number |
| `267460:129` | 0 of ~135 000 evaluations reach the far side | *"A marker 16 m further east, built the same way in the same batch, fires normally"* |
| `173691/ENDGAME:103-104` | `L_low 0 finishes / 208` | `N_none 0 finishes / 208 ← negative control`, plus twelve non-firing runs shown sitting **inside** the trigger window for 28–1131 samples |
| `126859:147` | 312 of 315 inputs have zero timing slack | *"the human world record's own keyboard tape measures 97 of 99 at zero slack on exactly the same test"* — the null is about the map, and the control says so |
| `146612:176` | keyboard tape brittle | 117/144 vs the human's 103/140, same measurement |
| `276874:125` | nothing in a further 2.3 M evaluations | quoted **at a 74 % finish rate** — the search was demonstrably finding finishers |
| `285268:34`, `274191:59` | contamination | "none of 49.291, 49.446, 49.491, 49.282" — the discriminating set is named |
| `tools/tmsite:106` | 0 of 233 real steps | stated as *"the honest outcome — TICK would refuse it too"* |

---

# C. CATEGORY 3 — generalisation from n = 1, or scope beyond the evidence

### C1. `285885/README.md` — the trigger model `y + 0.84·u_y` is a mechanism the page asserts, and it mis-ranks — **MINE**
The page derives the whole "the deficit is height" conclusion from a tested point
0.84 m up the car's body axis. Per `tm2020-map285885-sunken-gate.md`: **six
probes that model scores 0.10–0.17 m better fire nothing on the oracle ladder,
while the incumbent fires at 144.070.** A model that mis-ranks the candidates it
is used to rank is a correlate, not a mechanism.
The *conclusion* survives on separate evidence (the 424-station sweep and the
gate-at-crossing-point control), which is why this is a restatement and not a
retraction. **Action: model demoted to INFERRED-and-known-to-mis-rank; the
conclusion re-attached to the evidence that carries it. DONE.**

### C2. `285885/README.md` — "Rank 2 … Nobody has looked at that approach" — **MINE**
The exact shape of the claim this map already had to retract once ("rank 1's
11.2 s approach has never been searched, ~6 s unclaimed" — wrong on both
halves, that approach *is* our own incumbent). An unsearched-lead claim on this
page needs its own evidence now. **Action: marked UNKNOWN with what would settle
it, not repeated as a lead. DONE.**

### C3. `FK.md:138` vs the `FK_STATE_OFF` doctrine — **MINE, and it is already right**
`FK.md` says the race clock is **"not at a fixed offset"** from the position
(P−7916 / P−11268 / P−14780 on three runs) — *"locate it behaviourally, never by
offset."* That is the correct doctrine and it is already in the repo. The n = 1
generalisation vjeux flagged is in the **memory keys**, not here.
**Action: memory key `tm2020-forkstate.md` corrected; `FK.md` gets a pointer so
the two cannot drift apart again. DONE.**

### C4. `276874/README.md:37` — g calibrated on two tapes — **MINE**
> it accelerates downward at 25.20 m/s² in this engine (calibrated on two tapes…)

Same family as A4: a scalar g with no `v_y` beside it. n = 2 on one map.
**Action: `v_y` range required beside it; pointer to the law. DONE.**

### C5. `227654/README.md:110` — a search ceiling stated as a map property [owned elsewhere]
> 59.912 is the best keyboard finish, and that appears to be the ceiling for
> three steering values on this map **rather than a gap in what has been tried**

The second clause is precisely the inference the evidence cannot support.

### C6. `tools/README.md:129` — "never carry another map's reading over" — **already right**
The answer-key section explicitly says *"winning parameters do not port"* — this
is the n = 1 rule stated correctly, from two maps. Left alone, cited in `CLAIMS.md`.

---

# D. CATEGORY 4 — superseded numbers left standing

### D1. `146612` — **the page's headline time is not in the directory, and two files' names disagree with their own headers.** MINE, and the worst instance found.
`tmtraj corpus qc --root .` on this box:
```
146612  TAS_39183.Ghost.Gbx        OK  39.555  792  3715.3 m
146612  KEYBOARD_39706.Ghost.Gbx   OK  39.555  795  3716.6 m
146612  TAS_39430.Ghost.Gbx        OK  39.430  789  3554.9 m
146612  BEST_39961_v3.Ghost.Gbx    OK  39.961  800  3556.0 m
```
* `TAS_39183` **declares 39.555**, not 39.183. `KEYBOARD_39706` **declares
  39.555**, not 39.706. Two files, two different names, **one declared time**.
* Both carry a path of ~3715 m where every other file in the directory is
  ~3555 m — 160 m longer. They are not the same route as the rest of the family.
* The page's headline is **38.968**; the top-level README says **38.975**; the
  index memory key says **39.185**; the Files table calls `TAS_39183` *"the
  fastest lap in this directory, 39.183"*. **Four numbers, and the fastest thing
  the directory actually declares is 39.430.**
**Action: page and top-level row restated to what the files support, with the
discrepancy named rather than quietly resolved, and 38.968/39.183 marked
UNVERIFIED pending an oracle run on a map copy this box does not have. DONE.**

### D2. Top-level `README.md` — the 267460 row quotes a bound that is VOID — **MINE**
> bounded away three ways: **an energy floor of 17.102** taken at the most
> generous measured gravity and using no constant…

`tm2020-map267460-mini-trial-2.md`: *"⚠ THE EARLIER 'physical floor 17.102'
BOUND FROM THIS MAP IS VOID — its premise 'best ramp exit ever = 15.014' was a
**search record**; the flick broke it within the hour."* The map's own page has
already dropped it and says *"an estimate, not a bound"*; only the top-level page
still calls it a floor. **Action: row rewritten. DONE.**

### D3. Top-level `README.md` — the 285885 row's open question is closed — **MINE**
> The open question is a banked surface: both tapes reach 74° of tilt at
> 165 km/h, 142 m from the finish, and nobody has mapped what is beside the ramp

The map page's own "What is closed" section closes exactly this. **Action: row
rewritten to the current open question (13 m of apex from a coupled move, and
the AT's provenance). DONE.**

### D4. Top-level `README.md` — six stale times — **MINE**
| map | top-level says | the directory / index says |
|---|---|---|
| 227654 | 57.493 | **57.482** (`fu1_recov_k3`, independently validated on a second box and binary; 57.483 itself superseded) |
| 210218 | 95.575 | **95.507** |
| 146612 | 38.975 | see D1 |
| 126859 | 23.416 | 23.416 has a file; index's 23.462 also has a file — **not a defect**, both are real, the page picks the faster |
| 267859 | 10.759 | **NO DEFECT — this row was MY error.** All seven files are present and each declares its own name (`TAS_10758/10759/10768/10769/10859`, `KEYBOARD_10788/10897`). I read it off a truncated `corpus qc` capture and did not check the directory. The page is also already right about 10.758 vs 10.759: it cites 10.759 because that one was rebuilt on three separately compiled binaries and 10.758 on one |
| 191465 | 13.071 | index 13.073 |
| 203072 | 10.640 | index 10.642 |
**Action: the ones with a file win; the ones without are marked. DONE where
settled. The 267859 row was my own mistake and is corrected above.**

### D5. `134672` — the published best has no file in the directory [owned elsewhere]
Directory holds `TAS_67404` and `KEYBOARD_67625`; the published result is
**67.319** (`ksi_67319_watchable_v2`). Render arm.

### D6. `227654/README.md` — the results table and the Files table name disjoint sets — [owned elsewhere]
Table lists `TAS_57493` / `TAS_57573` / `TAS_59912`, all *(no file)*; Files lists
`TAS_57518` / `TAS_57537` / `TAS_57577`, none of which appear in the table. A
reader cannot tell what the map's best time is from its own page.

### D7. 33 of 159 published ghosts declared a span outliving their car [owned elsewhere]
Render arm; `ghost record shorten` repaired 27, 4 correctly refused.

---

# E. What I could not settle, and why

| item | why |
|---|---|
| whether ≈0.5 mm is a client/server difference or two copies of the car struct | needs the render arm's `8ca8c2e7` discriminator to land; both readings are in the corpus (A5) and the test is pre-registered |
| whether `g` is genuinely per-map | the fleet law's intercept is fitted on **one** map (208024). The named test — re-fit another map against `v_y` and compare intercepts — needs arcs long enough to have a lever arm, and 285885's own tape has a 2-sample longest arc |
| 146612's 38.968 / 39.183 | the map file is not redistributed in the repo and this box has no copy; the oracle cannot adjudicate without it |
| ~~267859~~ | **withdrawn — this was my own error, see D4.** All seven files are present and correct |
| 238835's regeneration | render arm states the blocker precisely and **refuses to ship a raised threshold without a positive control on the same map**. Correct call; left as UNKNOWN, not upgraded |

---

# F. FOUND DURING THE AUDIT — not on the brief, and the biggest of the four

### F1. `tmtraj corpus dup` HAD NEVER RUN. It reported the whole corpus clean.
**Category 1, about our own audit toolchain.** `corpus dup` is the check that
catches "one run published twice". It decides whether two files' identical
recorded motion is *expected* by first asking whether their input tapes differ —
and it asked by shelling out to **`fk tapediff`**, which is **not a command this
repo's `fk` has**, at any build:

```
$ ./tools/target/release/fk tapediff --a A.gtape --b B.gtape
fk: ABORT: unknown command "tapediff"
```

The call failed every time. `.ok()?` swallowed it. `None` from that function
means *the tapes are identical*. So every pair in the corpus came back
`identical-tapes / EXPECTED-SAME-INPUTS` and the scan **exited 0**.

Caught on 228607: the scan called `SPLICE_24854` and `TAS_19907` identical-taped
while their trajectories are **357 m** apart and `ghost tape diff` puts their
first input difference at **tick 72**.

Three things worth keeping:
* The module's own header says the shell scripts it replaced were fragile
  because *"every one of them piped a tool's stdout through awk and discarded
  its stderr"*. **The Rust port reproduced the bug**: `.ok()?` is `2>/dev/null`
  with a nicer spelling.
* **Second time `fk` being unreachable produced a silent wrong answer**
  (`SEARCH.md` has the first: 24 attempts "failed to find the car"). The first
  failed toward a **null**, this one toward **clean** — and clean is worse,
  because a null looks like a result and gets questioned.
* **Fixed by deleting the subprocess.** The comparison now runs in-process on
  `gbx::tape`, already a dependency. Positive control as a unit test
  (`intgcmd::tapediff_control`): a tape must read identical **to itself** and two
  known-different runs must read **different** — either half alone passes for a
  broken comparison.

**And I did not believe my own repair either.** Its first output was **35
`REFUSE-ONE-RUN-TWICE`**, nearly all keyed `diverge@-1.52s` — two drivers
holding different keys during the countdown, before the car can act. Pre-race
ticks are now excluded, and that choice is stated in the code. The corpus now
returns 607 shared-prefix / 135 review / **46 refuse** / 8 same-inputs.
**The 46 are NOT adjudicated and are NOT published as defects.** Several sit on
maps with long no-authority windows where inputs can differ for seconds without
moving the car. The established fact is only that the check runs.

### F2. 228607 Torment (1-UP) — eight published files carry a carrier's result chunk
**Category 4 + 1, on a map in the repo's *beaten* table, with three clips
published from it, that nobody had checked.** `ghost inspect`, all eleven files:

| files | header | result chunk | record span |
|---|---|---|---|
| `TAS_19907/19910/19927/19936/20070/20083`, `FORGIVING_19948`, `LOWINPUT_20070_16values` | their own name | **20.034, and 4 respawns** | 0.000 … **24.900** |
| `TAS_20126` | 20.126 | **20.426**, 4 respawns | 0.000 … 24.900 |
| `AUTHOR_LAP_20258_watchable` | 20.258 | 20.258 | 20.290 — **clean** |
| `SPLICE_24854` | 24.854 | 24.854 | 24.900 — **clean** |

Nine runs sharing one carrier's result chunk and record length, for a ~20 s run,
and none of our tapes contains a respawn. **Times unaffected** — the oracle reads
the tape. **Clips affected** — a render reads the record. The two clean files are
the control: the test can come out the other way and on this map it does, twice.
`tmtraj corpus span` and `ghost inspect` both name it. **Action: written up on
the page as a warning box; regeneration left as an open task, not claimed.**

### F3. `tmtraj corpus claims` — new, and what it found
New scan, 4 checks, over all 35 directories (`tools/tmtraj/src/claimscmd.rs`):
name-vs-header, page links a missing file, file the page never names, headline
no file backs. **159 files read, 27 flags** after two false-positive classes were
removed — bolded *deltas* read as times (14 of them, which would have been 14
invitations to hedge pages that were right), and struck-through withdrawals,
which are the behaviour the scan should encourage.
Live findings: 146612's two mis-named files, 228607's nine, and the
headline-unbacked rows for 146612 / 210218 / 238835.

### F4. Claims verified and left alone — the other half of the job
`145875` (0 of 120, control published beside it: *"the world record fails the
same test identically, 0 of 120"*), `267460`'s 135 000-evaluation null (marker
16 m east fires in the same batch), `173691/ENDGAME`'s `N_none` negative control
plus twelve non-firing runs shown sitting **inside** the trigger window,
`126859`'s zero-slack null (controlled against the human's own tape, so the
claim is about the map), `210218`'s four zeros (control in the same table, 203
finishers of 1368), `276874`'s 2.3 M-evaluation null (quoted at a 74 % finish
rate), `tools/README.md`'s *"winning parameters do not port"*, `FK.md`'s *"the
race clock is not at a fixed offset — locate it behaviourally"*, and
`tm2020-map203169-cobalt-cove-video.md`'s `FK_STATE_OFF` note, which **already**
carries the n=1 retraction with its counter-measurement and is a model of the
convention. `279218` withdraws a file by strikethrough with the reason beside
it — also a model.

---

# G. Coverage — what I read, and what I only swept

**Read in full (11):** top-level `README.md`, `CLAIMS.md` (written), `126859`,
`145875`, `146612`, `165922/VERIFICATION.md`, `173691`, `186935`, `227654`,
`267460`, `285885`, plus `tools/README.md` and `tools/tmtraj/README.md` around
the sites edited, and the relevant regions of `228607`, `228811`, `210218`,
`276874`, `GHOSTS.md`, `FK.md`.

**Swept by pattern, not read line by line (the rest):** four greps across all 56
files — absence-as-fact language (303 hits triaged), nulls and the word
"control", scalar gravity, and the 0.5 mm family — plus the four machine checks
(`corpus qc`, `span`, `dup`, `claims`) which read every published file.

**So: I am not claiming the unread pages are clean.** A pattern sweep finds the
phrasings it knows. The two machine checks are the durable part — they will find
this class again on files nobody thought to look at, which is how 228607 turned
up.

---

# H. SECOND PASS — the pages §G said were only swept

Priority order as instructed: player-facing pages, then documents an arm reads
first. Read in full this pass: `173636`, `197047`, `191465`, `203330`,
`267859`, `252289`, `238835`, `276877`, `227969`, `153527`, `284238`, `270051`,
`270053`, `285268`, `249521`, `FILMING.md`, `RENDER-BOX.md`, `tools/LINEAGE.md`,
`trainer/README.md`, `GHOSTS.md` §0–1.

## H1. `284238` — a retracted law still taught as the mechanism. **The biggest find of the second pass.**
The page's section is headed *"What the obstacle actually is: the engagement
point"* and gives `engage_x` as **"one number"** governing the obstacle, with a
27-tape monotone curve and a bracketed target. A later arm **retracted the law**:
the discriminator is the **crossing angle `vz` at the ice kicker**, and
`engage_x` was standing in for it.

Category 3 and 4 at once, on a page an arm reads before spending a day on the
map. The correction was earned the right way round — **by perturbing the run
that WORKS**: Yhomas's own 46.112 on the sibling, one flag at a time, **rides at
vz −23.72, unloads at −22.91**, with his checkpoint arrival falling
69.40 → **22.44 m/s** as vz weakens. Ours engages at **−2.3**.

The mechanism underneath, which is the reusable part: **the water launcher is a
shallow trough (−0.78 °/m) where his tech deck is flat (0.09 °/m)** — loading the
wheels needs roll ≈ 0, *the floor*; the rotation needs vz ≤ −24, *3 m up the
wall*. Same variable, opposite directions.

**Two experiments on that page are additionally VOID**: the 1.00 m kicker is a
**four-block assembly with one free block**, so both "raise/lower the kicker"
matched A/Bs moved a quarter of it and built a step. Worth keeping as a rule —
**a matched A/B is only matched if it moved the whole object**, and the car
arriving bit-identically is exactly what hides the failure.
**Action: measurements kept, law marked retracted above them, open question
restated as the one actually open (the 1.30 s water start, never searched). DONE.**

## H2. `FILMING.md` rule 3 — a live defect cited that has since been repaired
*"On 285268 all eight of our tapes decode to a human's trajectory."* Checked:
`tmtraj corpus splice --root .` now reports all five of ours **CLEAN** against
`HUMAN_rank2_keyboard_49491` — **0 of 986–990 samples identical**, worst
separation 3.55–15.60 m. **Action: rule kept, example marked as a
before-and-after, and the two corpus-wide commands named — with the warning that
any `corpus dup` clearance predating today means nothing. DONE.**

## H3. The 0.5 mm result INVERTS four pages
Not a rename. Each page cited ≈0.5 mm agreement between two of *our own*
readings as evidence the trajectory is right; it is now the signature of two
readings of the **wrong copy**.

| page | what it said |
|---|---|
| `276877` | "two independent regenerations… agree to **0.487 mm**" |
| `276874` | "a completely different readout path agrees to **0.48 mm**" — and its "26 of 33 regenerations agree" is a **reproduction count**, which must never outrank a test that can identify the answer |
| `267460` | "regenerated from engine memory and **accurate to half a millimetre**, so the driving you see is exactly what the simulator validated" |
| `165922` | nine files following wschseng's recording to **0.000563 m** — they replay **his** inputs there, so a correct locate reproduces him *bit-identically* |

**No time is affected** — the oracle reads the tape. What is affected is whether
the **clips** are frame-accurate. **Action: marked on each, with what would
settle it. DONE.**

`270051` is the exception and deserves the credit: it already said *"the offset
is ours"*, two days before anyone could say what caused it. **Action: mechanism
added, its conclusion left standing. DONE.**

## H4. Gravity — a checked negative, and one page fixed
Swept for `9.81`, `9.8`, `sin(26.6`, `4.39`, "gravity alone", J/kg and energy
tables, apex arithmetic. **The repo pages are CLEAN**: the only Earth reference
is already a correction note in `tools/tmtraj/README.md`, and `228607`
correctly uses −24.7. Recording that as **checked, not assumed**.
One real gap: `267460`'s "gravity alone costs 6.4 m" did not name its `g`, and
**with Earth's the gap it rules out would look crossable** (2.6 m against
6.5 m). **Action: named there, with the three-map agreement. DONE.**

## H5. `tmtraj` — the contact bit cannot select samples
On 153527's driver recording the derived `is_ground_contact` bit reads **False
on all 85 811 samples**, including a car standing still on a floor. **Action:
documented why `motion` classifies from the position and only *prints* the byte.
DONE.**

## H6. Pages read and found SOUND — with the passages worth copying
`173636` ("steering through the glide is **provably inert** — zeroing it over the
entire glide returns the identical millisecond": a claim with its test inline),
`197047` (a positive control on the exact defect that caused a withdrawal),
`191465`, `252289`, `227969`, `249521`, `270053`, `153527`, `238835`, `276877`,
`267859` (already right to cite 10.759 over the faster 10.758, because 10.759
was rebuilt on three separately compiled binaries), `trainer`, `RENDER-BOX`,
`GHOSTS` §0–1.

**Four are now cited in `CLAIMS.md` as the model**, all written before the
convention existed: `203330`'s authority map (which separates *"the car does not
respond"* from *"the input has no authority"* and then says the mechanism is
**"measured but not yet attributed"**), `285268`'s catch of its own tool
(*"`nearident` returned `overlap=0` with a mean of 1.8e308 — it compared
**nothing**"*), `270051`'s false positive, and `tools/LINEAGE.md`.

## H7. An error of my own, corrected
Ledger entry D4 claimed 267859's directory held only `TAS_10859`. **All seven
files are present and each declares its own name.** I read it off a truncated
`corpus qc` capture and did not open the directory. Corrected in place.

## H8. `tape_diffs_in_window` now compares the respawn bit
A respawn is an editable input (bit 31 of the state literal) and this project
edits it deliberately. **Checked negative: including it changes the corpus
census by nothing** — 607 / 135 / 38 / 8 / 8 before and after — so no pair was
hiding a respawn-only difference. Stated as checked rather than assumed.

---

# I. THE 38 UNRESOLVED PAIRS, SETTLED AGAINST THE ENGINE

§E said this "cannot be done from a clean checkout". **That was a harness limit
written as a conclusion** — the maps are on the shared store and the oracle runs
on this box. Corrected, and done.

Evidence: `evidence/adjudication_final.txt`. Tools: `tmtraj adjudicate`,
`tmtraj adjudicate-batch` (new), over 143 `fk trace` runs.

## The result

| n | verdict |
|---|---|
| **35** | **INNOCENT-INERT-INPUTS — MEASURED.** Wherever the two records agree bit for bit, the engine agrees bit for bit too, running each file's own tape. Those differing inputs had no authority there. |
| 1 | INCONCLUSIVE at **0.001077 m** (173636) — and that is *below* each file's own trace-vs-record agreement (0.0008 m). It is innocent at the instrument's resolution; saying so is more honest than promoting it. |
| 2 | UNTESTED — 238835 and 267859, and the reason is known (below). |
| **0** | **DEFECTS.** |

**25 of 26 traced files reproduce their own record to ≤ 0.0068 m**, i.e. their
records *are* their own tapes' runs.

## My own defect claim was wrong, and is retracted

I published a warning box on 210218 saying the two files there carry one
recording. The decisive test says the opposite: over the **1735 samples where
the records agree bit for bit**, the engine puts the two cars at most
**0.0001 m** apart, 1734 of them bit-identical. Both files are sound. Page
corrected.

**Why my reading failed**: I compared whole-file *rates* — records agree on
93.8 % of samples, simulations on 94.1 % — which say nothing about whether those
are the **same** samples. They are. That had to be measured.

## Four instrument errors of mine on the way, each caught by a control

1. **One trace is not enough.** 173636 `TAS_22072` read "does NOT match" at fork
   ticks 400 and 700 (0.30 m) and **matched at tick 1000 (0.0008 m)**. Two
   agreeing wrong answers — the reproduction-count trap. Acceptance is now
   best-of-sweep.
2. **Test for a time shift, not a distance.** At shift 0, five sound files read
   MISMATCH at 0.56–1.54 m; scanned over −3…+3 ticks they read 0.005 m. Judging
   at shift 0 would have condemned 210218 ×2 and 267460 ×3.
3. **A pair test must use each trace at its own accepted shift.** Omitting that
   produced **four identical "DEFECT" verdicts at 2.140111 m** on 126859 — one
   tick at 222 m/s — on five files each of which reproduces its own record to
   0.007 m. *An identical number across four independent findings is one
   artefact.*
4. **Picking the best-agreeing trace ignored coverage.** 16 pairs read UNTESTED
   purely because the chosen fork point started after the window under test.
   Among traces that have found the car, more coverage wins.

## The two that stay UNTESTED, and why that is a map fact

238835 and 267859 are the two **turtle** maps in the set — the car is inverted at
walking pace for the whole run. **No file on either map locates**, at any of the
14 fork points tried. That is not a property of the files, and it is
independently diagnosed: the render arm reports the locate compares `d(pos)/dt`
over a 50 ms sample against the stored velocity and demands 15 % of speed, so on
a turtle trial the **real car scores 1.41 m/s against a bar of 1.14** and every
anchor is refused. My failures land on exactly the maps their independent
analysis predicts. **The honest statement is "this instrument cannot see these
two maps", not anything about the files.**

---

# J. THE ANCHOR, AND A DISAGREEMENT BETWEEN TWO ARMS

Contributed first-hand by the carrier-bytes arm (`8ca8c2e7`, node 27628, files
`tools/fk/CARRIER.md` + `carrier-bytes.tsv`, banked at
`tm-unbeaten/_audit/carrierbytes/`) after the coordinator attributed it to
`CLAIMS.md` from a one-line summary and I declined to paraphrase it.

**That refusal was the right call and is itself a ledger entry.** An anchor
definition is exactly the kind of claim that needs its own measurement and
control beside it; writing one from a fragment is the failure this audit exists
to catch. Two of the three items in the summary turned out to be **contested
between two arms**, which a paraphrase would have flattened into a fact.

### J1. The definition, and the cost — CATEGORY 1, fourth instance of "precise, confident and blind"
*"car" = the position triple of the copy whose slots at car+92/136/180/224 hold
**live floats**.* Several copies hold the same position and **all pass every
structural test** (unit quaternion, velocity = d(pos)/dt); only one has the
fields around it. Anchoring on `Layout::pos` **wrote zeroed wheel rotations and
gear into a file that passed the whole `ghost verify` gate** — V1, V6 at kappa
1.000, V7 the oracle re-simulating to 22.730 — because none of those bytes
affects the simulation, and a provenance check sees zeros rather than a donor's
bytes. Now in `CLAIMS.md` §3.

### J2. The criterion does not transport — **entered as contested, both measurements**
| | carrier-bytes | video-reconstruction |
|---|---|---|
| gear | `car+340`, 100.00 % on 8 recordings | `car+748`, 99.43 % |
| wheels / wetness | +92/136/180/224, 99.25–100 % | wetness `car+180`, 95.4–96.0 % |
| the other's liveness test, ported | — | **4 dead slots, 0.0, over 814 ticks** |

Gear reconciles (748 − 408 = 340); **the wheel offset does not** — 1196 implied
from one side against 408 from the other, so the wheel block may not sit at a
fixed offset from the position at all. The second anchor reads as "a bare copy"
under the first's criterion while reproducing the recording at 95–99 %, and dead
memory does not do that. **Written as: sound as a chooser within the frame it was
validated in, not a transportable test.** Open, and in §0.

### J3. Four encoding assumptions, each of which cost a channel
Range (an f32 filter to 0..1 hiding a rotation running to 1607), rounding
(round-to-nearest vs truncation, **17 points**), quantisation (u8 exactness for a
channel that is not quantised), and the **small-integer-lookup trap** (an integer
read as f32 is a denormal, so a fitter returns k = 2.85e45 at a flawless 100 %;
byte 89 scored as a raw byte on eight keys is **0.00 %**).

### J4. That arm's own retractions, recorded as it made them
Dirt is **ABSENT, not zero** (pre-registered and refuted, −7.35 points); byte 89
refused a fourth time; ice held back while it was a one-key result and shipped
only at two independent recordings on two maps; and a quaternion reported "exact,
0.00000 rad" **from a median of a bimodal population** — honest figure 75.0 %
exact, p90 0.00042 rad. *The project has "split before you quote a spread"
written down, and the arm walked into it anyway and said so.*

---

# K. AFTER THE REBASE — the third wrong claim of mine, and 146612 closed

Rebased onto `main` = `dc332a7` (the render arm's ten commits, regen-sweep-B's
work, and both of my own pushes). Re-swept, and re-ran every check.

### K1. **My 146612 finding was wrong. The names are right; the headers are stale.**
`tmtraj corpus claims` flagged `TAS_39183` and `KEYBOARD_39706` as declaring
**39.555** apiece instead of the time in their names. That fact is true. I turned
it into *"neither figure is backed by the file that bears its name"* and rewrote
**two pages** around it.

Then I asked the oracle, with the map from the store:

```
TAS_39183.Ghost.Gbx        PASS V7   oracle re-simulated the written file: 39.183
KEYBOARD_39706.Ghost.Gbx   PASS V7   oracle re-simulated the written file: 39.706
```

**Eight of eight publishable files re-simulate to exactly the time in their
name**, and the ninth returns **DNF at cps 5** as its own name says — the
negative control, in the same batch. A stale declaration inherited from a
carrier, which `ghost declare --from-oracle` fixes and which changes no physics.

**And `tools/LINEAGE.md` had already recorded it** before this audit started:
*"146612 · 9 · 8 + the file named `SEGMENT_cp5_…`, which returns DNF cp5 as its
name says."* I had read that file — I quoted it twice as a model — and did not
search it for the map I was flagging.

Fixed in three places, and the tool now says so in its own output:
`NAME-VS-HEADER … (ASK THE ORACLE: a stale declaration is common and harmless)`.
**Rule added to `CLAIMS.md`: before writing that something is unsupported,
search the repo for the arm that already measured it.**

### K2. `146612/CANNOT-OPEN.md` — two of three open items CLOSED
The page listed *"the repo ships no `.Map.Gbx` to compare against"*, so *"the
staged file is corrupt in a way that keeps the header valid"* stood as
unresolved. **The store has a byte-identical second copy** (same md5, same
3 824 673 bytes) — and the stronger test needs no second copy at all: **the
engine loads this map and runs 40 seconds of physics on it, eight times.** A
file that parses, spawns a car and simulates is not corrupt.
**What is not settled** is the symptom itself: `EditMap()` still returns `ok`
and never opens. Now localised to the editor path rather than the file.

### K3. Ice was being called unavailable on four pages after it shipped
Three map pages and the tools doc say eleven channels *"cannot yet be read out
of the engine — rpm, per-wheel ice and dirt, ground contact, gear"*. The
carrier-bytes arm shipped **ice** hours earlier: `Icing01` at
`car + 88 + 44k + 28`, **100.00 % exact on two independent recordings on two
maps** (462 and 1370 samples) against 71.9 % and 79.0 % constants, no refit.
A harness limit written as permanent — my own category 1, inside the correction
notes for category 1. Count is now **ten**, on all three pages, with dirt marked
**refuted** rather than unfound and byte 89 as refused four times.
On 134672 this is not cosmetic: it is the ice spray on a 2620 m ice ribbon.

### K4. The pages other arms rewrote tonight — checked, and they hold
186935, 227654, 286279, 134672, 238835, 173691, 228607, 285885, 284238, plus the
two pages that did not exist when I swept (`203169-cobalt-cove`, 431 lines, and
`tools/vidread`). **They apply the convention without having been asked to**:
absent-not-zero stated explicitly, controls named inline, mechanisms separated
from correlations. Cobalt Cove is the strongest new page in the repo — it
distinguishes *"the instrument works"* (99.6 % against a 43.7 % runner-up) from
*"it declines when it should"* (no peak on earlier builds) and says outright
that a second channel's meaning *"has not been established"*.
