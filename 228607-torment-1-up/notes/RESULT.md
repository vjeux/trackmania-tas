# 228607 "Torment (1-UP)" — the within-map control: SURFACE arm FAILS here, and I think the map is the reason

Companion to `PLAN.md` (md5 `e924f48f016d6eac3e9627c377c6809a`, frozen with the
amendment before any correlation was computed). Fifth map in the attitude
series, run in parallel with another agent on the sibling 228811.

AT 20258 · human WR 24902 · gap 4644 ms · 23 records · §8 = **22/23** (rank 10
excluded, provenance verified — see PLAN §A).

---

## 1. Result: P1 and P6 both fail. P2 holds, but trivially.

| prediction | bar | top 10 clean | all 22 |
|---|---|---|---|
| **P1** roll deviation at the decisive last contact | r > +0.30 | **+0.327** (per-run anchored) | **−0.131** |
| P1 at contact −2 | — | −0.044 | +0.441 |
| P1 at contact −3 | — | +0.242 | −0.064 |
| **P2** free-ballistic phases stay below +0.30 | ≤1 of 5 above | **0 of 5** ✓ | — |
| **P6** powered-air phase correlates like a surface | r > +0.30 | **−0.381** (wrong sign) | −0.116 |

**The signs flip between populations at every contact.** +0.327 on the top 10
becomes −0.131 on all 22; −0.044 becomes +0.441. That is the signature of no
effect, not of a weak one, and P1's bar is not met on both populations as
required. **P1 fails. P6 fails, with the wrong sign.**

P2 "passes" — the five free-ballistic phases give −0.03, −0.02, −0.30, +0.26,
−0.03 — but a null arm is only informative when the other arm is positive, and
here it is not. The within-map control did not fire.

One post-hoc test, labelled as such and reported because it also failed: a
single roll sample cannot describe a rotating car under a body-fixed thrust, so
I integrated the world-up component of the car's own −up axis over the powered
phase (i.e. how much of the reactor's 22 m/s² went into lifting). r = **−0.382**
on the top 10, and inspection shows it is carried entirely by one outlier
(rank 11 at 0.392 against 0.85–0.99 for the other nine, and rank 11 is a
different run shape). Within the tight group there is no ordering. **I stopped
there rather than keep fishing.**

## 2. Why this map could not answer the question — the structural reason

The sector table explains the failure better than the correlations do.

Sector durations across the top 10, and their spread:

| sector | mean (ms) | sd (ms) |
|---|---|---|
| 1 | 4749 | 44 |
| 2 | 3558 | 64 |
| 3 | 1457 | 125 |
| 4 | 2458 | 417 |
| 5 | 3506 | 410 |
| 6 | 3429 | 149 |
| **7 (CP6 → finish)** | **7220** | **936** |

The last sector carries the spread, as on every other map in this series. But:

**the last sector time does not order the finish.** rank 11 has the *fastest*
last sector in the entire field (5249 ms against the WR's 6269) and finishes
**11th**, because it lost 2.5 s in sectors 3–5. rank 6 is fastest to CP6 and
finishes 6th. The correlation between last-sector time and finish time is
therefore weak by construction, and any statistic computed *inside* the last
sector inherits that.

In other words this map's field is not separated by one technique executed
better or worse. It is separated by **where each run loses a chunk** — sector 4
has an sd of 417 ms on a 2458 ms mean, sector 5 the same. That is a map with
several independent places to make a mistake, and the AT being 4644 ms below the
WR (18% of the run, the largest relative gap in the whole target list) says the
same thing: nobody is close to clean.

**The attitude hypothesis needs a field that agrees on a route and differs in
execution at one feature.** 227969 (42 runs, all one route), 203330 (5 runs, "on
rails" to 8.5 s) and 267859 (19 runs, one flopping sequence) are that. This map
is not, and I could have predicted that from the sector spread *before*
correlating — sd of 417 ms in a mid-run sector is the tell. **That is the
selection criterion I would add for the next test: check that the decisive
sector dominates the variance AND that its time orders the finish.**

## 3. What DID replicate here, and it is the more valuable finding

Before any correlation, the sibling agent measured that 228811's final air phase
is **powered, not ballistic**: a ≈21 m/s² force along the car's own −up axis. He
predicted 1-UP would mirror it at **+21**. I ran the same fit on 228607:

| phase | 228607 body-up (top 10 clean) | 228811 |
|---|---|---|
| early air 10.3–12.1 s | **−1.37 ± 0.25** | ≈0 (A1–A5: +0.6 … −3.4) |
| mid air 16.3–18.5 s | −1.0 … −1.8 | ” |
| **final air, 20.9 s → finish** | **−22.33 ± 1.99** (all 22: −21.72 ± 2.32) | **−21.05 ± 4.63** |

**Same sign, same magnitude — his prediction of a flip is refuted, and that is
the better outcome.** A sign flip would have made the force a property of the
map variant; identical values across two independently-measured variants make it
a property of the **reactor block**, i.e. a reusable constant:

> **A firing reactor applies ≈21–22 m/s² along the car's own −up (belly) axis.**
> Orientation therefore *aims* it: inverted, it cancels gravity; upright, it
> doubles it; on edge, it is horizontal thrust.

The free-ballistic control at −1.37 ± 0.25 against −22 in the *same runs* with
the *same statistic* is the cleanest thing on this page.

Conditioning, reported the way it should be rather than as a box score: body-fit
beats world-fit on 13 of 22 runs, but that number is meaningless because a run
holding one attitude makes the two models mathematically degenerate. The
evidence is the well-conditioned runs — `rank00006` rotates a full 360° and
gives body **0.099** against world 0.543; `rank00003` and `rank00005` also rotate
360°. The "world wins" cases are concentrated in the slow tail (ranks 15–23, sd
15–70, i.e. no stable coefficient at all — those runs are crashing, not flying).
Body-up itself is stable at −22 ± 2 across all 22 regardless.

## 4. Consequence for the rule

The series now stands at: **227969 surface → holds** (discovered), **203330
surface → holds** (discovered), **203072 ballistic → correctly predicts null**
(pre-registered), **267859 surface → holds** (pre-registered, r = +0.75 to
+0.91), **228607 surface → FAILS** (pre-registered).

The failure is real and is recorded as such. My reading is that it is a failure
of the *test*, not of the rule — this map's field does not isolate one feature —
but that reading is post-hoc and I am not entitled to it as evidence. What I am
entitled to say is:

* the rule is **not universal across maps that contain a surface feature**;
* the extra condition it appears to need is about the FIELD, not the map: the
  decisive sector must dominate the variance *and* order the finish;
* **the reactor constant is the finding that survives this map**, and it makes
  the rule's name questionable in a way I pre-registered: what plausibly matters
  is not "a surface" but **any force whose direction is fixed in the car's
  frame**. P6 failing here does not settle that — it was tested on a field that
  cannot answer anything — so it stays open.

The sibling's 228811 result, on a 48/48 field, is the one that will actually
decide the within-map control. Mine should be read as: **a null, on a field that
turned out to be the wrong instrument, plus a replicated force constant.**

## 5. Artefacts

`map.Map.Gbx`, `all.txt`, `csv/` (23 decoded per-tick trajectories),
`PLAN.md` (pre-registration + amendment), `verify/` (the re-downloaded rank 10
with its sha256 and DNF transcript). No search was run.

---

## 6. ADDENDUM — the author's own AT ghost decodes, and it changes the map's story

The 228811 agent found that **the author's validation ghost is embedded in the
`.Map.Gbx` and decodes with `tmtraj decode map.Map.Gbx`**. On 1-UP it does:
`validated="1"`, 406 samples at 50 ms, 0…20290 ms — the author time's own
trajectory, with its steer/gas/brake columns. `atSetByPlugin: true` on
unbeaten.at does **not** mean nobody drove it. Banked as `csv/AT_author.csv`.

**The launcher replicates on 1-UP, exactly as they predicted.**

| | route through the last feature | y reached | peak speed | finish |
|---|---|---|---|---|
| **AT (author)** | **hits a launcher at (78, 50, 709)** | 173 | **769 km/h** | **20258** |
| human WR | climbs the wall to y≈144 | 143.8 | 435 | 24902 |
| all 21 other finishers | climb the wall | 142.0–146.7 | 373–497 | 25219+ |

**Not one of the 23 records touches the launcher.** Every finisher climbs the
wall; the two that do not (ranks 11 and 23, max y 83 and 76) simply fail
elsewhere. The AT's contact at 18490→18540 takes it from **339.7 to 768.9 km/h
in a single 50 ms sample** and fires it at the finish: 1768 ms of flight to the
line, against the WR's 6269 ms last sector.

The AT's inputs into it, from its own telemetry — a driver could copy this:

```
race 17990   steer +0.33  gas 1  brake 0     turn in, 361 km/h
race 18040   steer +0.86  gas 1  brake 0
race 18090   steer +0.95  gas 1  brake 0
race 18140   steer +0.96  gas 1  BRAKE 1     <- gas AND brake together, held
race 18140-18490   steer ~+0.97, gas 1, brake 1   slides ~30 m, 358 -> 340 km/h
race 18540   ... 769 km/h                    <- the launcher fires
```

Full lock with **gas and brake held together**, scrubbing sideways across the
floor into the launcher — the same technique the 1-DOWN agent documented, at the
same kind of feature, on the sibling map.

The AT is *not* faster through the earlier map: at x = 1400 the human WR is
**69 ms ahead** of it, and the AT's lead at x = 82 is only 187 ms. **All 4644 ms
come from the launcher.**

### What this does to this map's contribution to the series

It makes the null in §1 unsurprising and mostly uninteresting: the human field
varies its attitude at a feature *the fast route does not use*. Recording the
general caution, which the 228811 agent put well:

> **The feature the field varies at is not necessarily the feature the fast run
> uses.** Before running an attitude test, check that the fast reference and the
> field take the same route through the feature being measured.

That is a second, independent selection criterion alongside the one in §2 (the
decisive sector must dominate the variance *and* order the finish). Both are
cheap, both are checkable before any correlation, and this map fails both.

**And the practical instruction for anyone taking 228607 as a TAS target: do not
seed from the human WR.** The whole gap is a route the field has not found, the
route is visible in the map file itself, and the AT's own tape is the
description of it.

---

## 7. REACHABILITY, first attempt: the AT's telemetry is NOT a replayable tape

Recorded because it is a clean negative about a *tool*, and it would otherwise
look like evidence about the map.

**The embedded AT ghost carries telemetry but no inputs.** Confirmed by scanning
the decompressed map body for the input chunk `0x0309201D`: **absent** (`fk
layout` on the map: "no input chunk"). So the author's run cannot be replayed
directly — only its per-sample steer/gas/brake, at **50 ms**, against a tape
that runs at **10 ms**.

I built `tmsimp --mode fromcsv` / `--mode splicecsv` to rebuild a tape from that
telemetry (hold each sample for 5 ticks) and to splice it into the human WR's
tape over a window, then swept the window.

| experiment | result |
|---|---|
| full reconstruction of the AT from its telemetry | **DNF** |
| splice into the WR over the divergence window, ticks 1500–2000, 210 windows | 9 finish; best **24854 ms** (−48 on the WR), **no launcher** |
| **splice over an EARLY, UNCONTESTED stretch (race 2–7 s), 15 windows** | **0 of 15 finish** |

That last row is the decisive one. Over a stretch where the author and the WR
are driving the *same line at the same speed*, replaying the author's own inputs
still DNFs — so the reconstruction is **unfaithful by construction**, and the
50 ms sampling is the reason. A change of input between samples is invisible,
and on a 20-second run in a chaotic simulator that is fatal within a second.

**Therefore: this experiment says nothing about whether the launcher is
reachable.** It says only that telemetry-replay is not the instrument. Anyone
attacking this should use the waypoint approach the 228811 agent specified —
relocate the finish gate to the launcher so arrival time *is* a finish time, and
search the drop (ticks ~1700–1990) with `tmsearch` unmodified — because a
finish-time objective is blind across the valley: every candidate that starts
toward the launcher and misses is slower or a DNF.

(Byproduct, validated and banked as `m228607_splice_24854.Ghost.Gbx`: **24854 ms**,
48 ms under the human WR, from splicing the author's inputs over race
16.50–17.00 s. It is not a result — it is 4.6 s off the AT and on the field's
route — but it does show the author's line is marginally better even in the part
where the two agree.)

---

## 8. The launcher is a VELOCITY-DIRECTION trigger — the objective, specified

Established jointly with the 228811 agent, each of us measuring on our own map.
They drove a spliced tape through their launcher's exact coordinates **73 km/h
faster than the author** and it did not fire. On 228607 the same conclusion
falls out of the field: the launcher sits inside an existing checkpoint gate
(items 10/11, `GateCheckpointLeft/Right32m` at (80, 50, 720) and (80, 50, 752)),
so **every finisher already drives through it**, and none of them trigger it.

State at the x = 80 plane:

| run | z | speed | vx | vz | velocity off the −x axis | launcher |
|---|---|---|---|---|---|---|
| **AT (author)** | 713.2 | 340.7 | −76.7 | **−57.5** | **+36.9°** | **FIRES → 769 km/h** |
| rank 6 | 714.2 | **442.5** | −122.4 | 0.0 | −0.0° | no |
| rank 2 | 741.0 | 377.3 | −104.7 | +2.6 | −1.4° | no |
| human WR | 743.3 | 365.7 | −100.4 | +8.8 | −5.0° | no |
| rank 8 | 753.4 | 401.8 | −111.6 | 0.0 | −0.0° | no |

**rank 6 passes within one metre of the author in x and z, 100 km/h faster, and
nothing happens.** Position is not the trigger and speed is not the trigger.

The author's scrub, tick by tick — the velocity vector rotating off the track
axis while the speed barely falls:

```
t=18140  x=107.9  v=358.1   vx=-95.4  vz=-25.6   off-axis 15.0 deg   <- gas+brake on
t=18240  x= 98.3  v=352.8   vx=-90.3  vz=-37.8            22.7
t=18340  x= 89.7  v=347.6   vx=-83.8  vz=-47.9            29.8
t=18390  x= 85.7  v=345.1   vx=-76.7  vz=-57.5            36.9
t=18440  x= 81.9  v=342.0   vx=-68.4  vz=-65.9            43.9   <- |vz| > |vx|
t=18540                     v=768.9                              <- fires
```

18 km/h lost over 400 ms, and 44° of velocity gained. That is what "full lock
with **gas and brake held together**" buys: it is not a turn, it is a scrub —
the only way to rotate the velocity vector out of the direction the car points.

**The free sanity check the 228811 agent proposed, run on all 23 records:**

| run | best velocity-off-axis angle anywhere near the launcher |
|---|---|
| **AT** | **43.9°** |
| best human (rank 22, a 101-second run) | 14.9° |
| rank 13 | 12.1° |
| rank 19 | 9.9° |
| everyone else | ≤ 3.6° |

The three humans above 9° are all wrecks. **Every clean run in the field is
within 4° of straight.** The objective separates the author from the entire
population by a factor of three, and separates him from the *clean* population
by a factor of twelve.

### The objective for whoever searches this

> Maximise the **velocity angle off the −x axis** (equivalently −vz) at the last
> ground contact near x ≈ 75–85, y ≈ 50, subject to keeping ≥ ~320 km/h.
> Target: **44°, vz ≈ −66 m/s at 340 km/h.** Mutation window race 15.9–18.3 s
> (the drop before the wall base), gas/brake ops included.

It needs the fork state reader per candidate (~65 ms), **not** gate relocation —
and note `tmmaps probe` cannot relocate a gate on this map anyway
(`no checkpoint block to probe`: the checkpoints here are *items*, and the
probe path only handles blocks). Unlike arrival time, this objective is smooth
and monotone, so a hill-climb can follow it across the valley that a finish-time
objective cannot see.

### Joint statement, agreed with the 228811 agent

> **On both Torment remixes the author time is not a better-driven version of
> the human line — it is a different line, taken once, by the person who built
> the map.** Zero of 71 recorded human runs across the two leaderboards touch
> the feature that produces it, and on both maps the human WR is level with or
> ahead of the author everywhere before it. **And the feature is not merely
> unvisited, it is guarded: passing through it at speed does nothing unless the
> car arrives sideways** — which is why 71 drivers who have all been within a
> few metres of it have never seen it fire.
