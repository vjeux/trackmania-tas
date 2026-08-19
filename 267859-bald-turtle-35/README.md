# bald turtle #35 — the human record falls by 0.400 s, and the author time survives by one millisecond

**Author time 10.768 · human world record 11.169 · best validated 10.769.**

| tape | validated | vs human WR | vs AT |
|---|---|---|---|
| [`TAS_10769`](replays/TAS_10769.Ghost.Gbx) | **10.769** | **−0.400** | **+0.001** |
| [`TAS_10859`](replays/TAS_10859.Ghost.Gbx) | 10.859 | −0.310 | +0.091 |
| [`KEYBOARD_10897`](replays/KEYBOARD_10897.Ghost.Gbx) | **10.897** | −0.272 | +0.129 |
| author time | 10.768 | — | — |
| human WR, Schmaniol *(control)* | 11.169 | — | +0.401 |
| human rank 2 *(control)* | 11.189 | +0.020 | +0.421 |

TMX map [267859](https://trackmania.exchange/maps/267859) · uid
`auaaMFbt2cKnZPYjP11sySqEb_6` · author **Bald_tm / BALDFROMSPB** · tag **Turtle**
(and only Turtle — no Trial) · **19 recorded runs, all 19 downloaded and
re-simulated exactly**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

**This is the closest miss in the project.** One millisecond, on the wrong side
of an author time that no human has beaten — on a map where the oracle
reproduces all nineteen records to the millisecond. The figure is not a single
lucky run: **four cold measurements across two separately compiled builds agree
on 10.769**, and the whole chain of tapes it descends from reproduces. It has
been re-validated again for this page against the untouched map, with the human
world record and rank 2 exact in the same pass.

A millisecond is one tick of the simulation at 100 Hz. There is no smaller unit
to lose by.

---

## What this map contributed: the attitude rule, tested before it was measured

The interesting thing here is not the time. This is the fourth map in a series
testing one claim, and **the first where the prediction was registered before any
correlation was computed.**

The claim: **where a car transacts momentum with a surface, its roll angle at
that contact orders the field; where it is in free flight, roll does nothing.**

The plan file — frozen, checksummed, and written before the numbers — predicted
that this map, which is fifteen successive inverted landings, would show the
effect everywhere. It did:

| prediction | bar | all 19 records | **top 10 (clean)** |
|---|---|---|---|
| **P1** corr(&#124;roll − top3&#124; at the last contact, finish time) | > +0.30 | **+0.313** ✓ | **+0.748** ✓ |
| **P2** at least 3 of the last 5 contacts above +0.30 | ≥ 3 of 5 | **3 of 5** ✓ | **5 of 5** ✓ |
| **P3** roll-associated spread across the top 10 | ≥ 0.100 | — | **0.500–0.700** ✓ |

And the confounder check came out the right way. The obvious trap is that slow
runs are slow for unrelated reasons — crashes, respawns — and drag roll along
with them, manufacturing a correlation. If that were the mechanism, restricting
to the ten clean runs would **weaken** it. It strengthens it, at every one of the
five contacts, from +0.31 to +0.75 at the decisive one and to **+0.91** at the
one before. The association is inside the clean field.

The cross-map contrast is the actual content:

| map | decisive feature | momentum transacted with a surface? | corr(roll, finish) |
|---|---|---|---|
| [227969](../227969-great-wtf-of-what-165) | wallride into a kicker | **yes** | orders the field; the whole margin |
| [203330](../203330-get-in-the-hole-impossible) | platform lip at 860 km/h | **yes** | orders the field perfectly |
| [203072](../203072-yeet-fall-2024-04) | ballistic launch into a 5.5 s flight | **no** | **+0.14 — nothing** |
| **267859** | **15 successive inverted landings** | **yes, everywhere** | **+0.75 to +0.91** |

A rule that only fires on the maps where it was discovered is not a rule. Here
the map predicted to show nothing showed nothing, and the map predicted to show
it everywhere shows it at five contacts out of five.

The same series produced a **failure** on [228607](../228607-torment-1-up), which
is published alongside this one and is the more instructive of the two.

## Validation

`TAS_10769` sha256
`5211fa9c3cdd82255c7bc1b38c91b0273c2cf0f1094b9a667c384910df534f09`, map sha256
`a2bb4b9eafce011584297a69880f808378ef8fc3e894c19de7f1edadf1599215`.

Re-validated for publication against the untouched map, one file per invocation
in a fresh process, with two human records carried as known-answer controls in
the same pass:

```
rank01_11169.Ghost.Gbx    11169   (control — the world record)
rank02_11189.Ghost.Gbx    11189   (control)
TAS_10769.Ghost.Gbx       10769
```

The tape carries **zero respawn packets**. The map is the same untouched copy on
which all 19 recorded runs re-simulate exactly — which is itself the strongest
available check that it has not been tampered with, since an edited map does not
return nineteen strangers' times to the millisecond.

## Notes

* [`RESULT.md`](notes/RESULT.md) — the pre-registered attitude test and its results
* [`PLAN.md`](notes/PLAN.md) — the search plan written for the TAS attempt
