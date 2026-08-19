# 267859 "bald turtle #35" — the boundary statement CONFIRMED, prospectively

Companion to `PLAN.md` (md5 `e81d076c728718c16eb14b0d24aa237d`), which recorded
the feature classification and the directional predictions **before any
correlation was computed**. Fourth map in the attitude series; the first one
where the rule was tested prospectively rather than discovered.

AT **10768** · human WR **11169** · gap **401 ms** · **all 19 records** · tag Turtle.

---

## 0. ACQUISITION.md §8 field reproduction: PASS, 19/19

Every record re-simulates to its recorded millisecond exactly. (203072 failed
this at 24/34 and was abandoned.)

## 1. Result: every prediction held, and the confounder check strengthened it

| prediction | bar | all 19 records | **top 10 (clean)** |
|---|---|---|---|
| **P1** corr(&#124;roll − top3&#124; at the LAST contact, finish ms) | > +0.30 | **+0.313** ✓ | **+0.748** ✓ |
| **P2** at least 3 of the last 5 contacts above +0.30 | ≥ 3 of 5 | **3 of 5** ✓ | **5 of 5** ✓ |
| **P3** roll-associated spread ≥ 100 ms across the top 10 | ≥ 100 ms | — | **500–700 ms** ✓ |

Per contact, counting back from the finish:

| contact | all 19 | **top 10** | slope, top 10 |
|---|---|---|---|
| −1 (last before the finish) | +0.313 | **+0.748** | +1788 ms/rad |
| −2 | +0.447 | **+0.907** | +314 ms/rad |
| −3 | +0.258 | **+0.787** | +140 ms/rad |
| −4 | +0.063 | +0.512 | +103 ms/rad |
| −5 | +0.373 | +0.527 | +177 ms/rad |

**The confounder check came out the right way.** P4 named the obvious trap:
slow runs are slow for unrelated reasons (crashes, respawns) and drag roll along
with them, manufacturing a correlation. If that were the mechanism, restricting
to the ten clean runs would *weaken* it. It **strengthens** it, at every one of
the five contacts, from +0.31 to +0.75 at the decisive one and to **+0.91** at
contact −2. The association is inside the clean field, not between the clean
field and the crashers.

## 2. The cross-map contrast, which is the actual content

The same measurement, at the boundaries of each map's decisive phase:

| map | decisive feature | momentum transacted with a surface? | corr(roll, finish) |
|---|---|---|---|
| 227969 | wallride into a kicker | **yes** | orders the field; 199 ms, the whole margin |
| 203330 | platform lip at 860 km/h | **yes** | orders the field perfectly |
| 203072 | ballistic launch into a 5.5 s flight | **no** | **+0.14** — nothing |
| **267859** | **15 successive inverted landings** | **yes, everywhere** | **+0.75 to +0.91** |

A rule that only fires on maps where it was discovered is not a rule. This is
the prospective version: the classification was written down first, the
direction was predicted first, and the map that was predicted to show nothing
showed nothing while the map predicted to show it everywhere shows it at five
contacts out of five.

**The statement, now with one prospective confirmation and one prospective
refutation behind it:**

> **Roll at a feature determines the speed retained if and only if the feature
> converts the car's momentum through a surface. Where the decisive phase is
> ballistic, roll costs nothing — orientation cannot move a centre of mass.**

## 3. What this map is, and what it means for a driver

The car is **upside down for the entire run**. Roll at successive contacts
alternates +2.44…+2.89 / −2.56…−2.94 rad: the car is inverted and flops from one
side to the other, contact by contact, 15 times, climbing from 10 to 99 km/h.
19–22 % of the run is airborne in 150–300 ms hops between landings. The last
contact before the finish is the only one where the car comes back near upright
(fast runs −0.64 to −0.71 rad).

So on a turtle map there is no "keep it flat" — flat is not available. The rule's
real content is **presentation**: how squarely the car meets each surface,
measured as deviation from what the fastest runs do. That is why the x variable
here is |roll − top-3 mean| rather than |roll|, and it is the right general form
of the rule on every map: 227969's "arrive flat" is the special case where the
fast presentation happens to be flat.

For a driver on this map: **the last three landings are where the field loses
its time**, at roughly +1800, +310 and +140 ms per radian of deviation from the
fast presentation. The last one is worth the most by a factor of six.

## 4. Limitation, recorded in PLAN.md before the test and still true

This map is almost entirely surface interaction. It tests the "surface ⇒ roll
matters" half of the rule strongly and the "ballistic ⇒ roll is free" half only
weakly — its air phases are too short and too incidental to carry a decisive
event. The second half rests on 203072 alone. **A clean prospective test of both
halves at once needs a map with a decisive feature of each kind**, and that is
the next test worth running.

No search was run here; this map was taken as an experiment, not as a target.
The AT is unbeaten and the gap is 401 ms over 19 records, so it remains a good
TAS target for whoever wants it — and §0 says the oracle can be trusted on it.

## 5. Artefacts

`map.Map.Gbx`, `all.txt` (19 leaderboard entries), `csv/` (all 19 decoded
per-tick trajectories), `PLAN.md` (the pre-registration).
