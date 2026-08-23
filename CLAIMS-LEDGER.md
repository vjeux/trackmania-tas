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
| 267859 | 10.759 | directory holds only `TAS_10859` — **10.759 has no file here** |
| 191465 | 13.071 | index 13.073 |
| 203072 | 10.640 | index 10.642 |
**Action: the ones with a file win; the ones without are marked. DONE where
settled; 267859 raised as an open item.**

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
| 267859's 10.758/10.759 | no such file in the directory; may be unpushed on another arm's branch |
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
