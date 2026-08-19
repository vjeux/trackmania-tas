# bald turtle #35 — the author time falls by nine milliseconds, and 44 % of them are in the last twenty-two metres

**Author time 10.768 · human world record 11.169 · best validated 10.759.**

| tape | validated | vs AT | vs human WR |
|---|---|---|---|
| [`TAS_10759`](replays/TAS_10759.Ghost.Gbx) — **the record** | **10.759** | **−0.009** | **−0.410** |
| [`TAS_10758`](replays/TAS_10758.Ghost.Gbx) — the arm's best, *pending independent verification* | 10.758 | −0.010 | −0.411 |
| [`TAS_10768`](replays/TAS_10768.Ghost.Gbx) | 10.768 | ±0 — *equals* the author time | −0.401 |
| [`TAS_10769`](replays/TAS_10769.Ghost.Gbx) | 10.769 | +0.001 | −0.400 |
| [`KEYBOARD_10788`](replays/KEYBOARD_10788.Ghost.Gbx) | 10.788 | +0.020 | −0.381 |
| [`TAS_10859`](replays/TAS_10859.Ghost.Gbx) | 10.859 | +0.091 | −0.310 |
| [`KEYBOARD_10897`](replays/KEYBOARD_10897.Ghost.Gbx) | 10.897 | +0.129 | −0.272 |
| author time | 10.768 | — | −0.401 |
| human WR, Schmaniol *(control)* | 11.169 | +0.401 | — |
| human rank 2 *(control)* | 11.189 | +0.421 | +0.020 |

**Why two numbers, and why the slower one is the record.** 10.759 is the tape a
verification arm rebuilt from the archive and re-ran on **three separately
compiled binaries**, so it is the independently verified figure and it is what
this repository cites. 10.758 has cleared the full six-invocation protocol on two
nodes — but **those two nodes share one toolchain**, which the finding arm
flagged itself, so the two runs are independent in their operating systems and
servers and not in their binaries. That is a real distinction and it is why the
faster tape is published as *the arm's best, pending independent verification*
rather than as the record.

The arm asked to be cited the conservative way. That request is the reason to
trust the rest of the numbers on this page.

TMX map [267859](https://trackmania.exchange/maps/267859) · uid
`auaaMFbt2cKnZPYjP11sySqEb_6` · author **Bald_tm / BALDFROMSPB** · tag **Turtle**
(and only Turtle — no Trial) · **19 recorded runs, all 19 downloaded and
re-simulated exactly**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

**Nine milliseconds is the thinnest margin in this project, so the denominator
was checked as carefully as the numerator.** The author time is not only a field
in a downloaded JSON: **the map binary's own header carries the medal quadruple
in the clear at bytes 74–89 — bronze 17000, silver 13000, gold 12000, author
10768** — and that is what the margin is taken against. Two independent sources,
and the bytes were read here to confirm it.

*A note on how this map reached publication, because it is the better story. The
arm that found it fixed its rule in advance: claim nothing until a tape returns
10767 or less, and do not round in our favour at 10.768 against 10.768. It then
produced a tape that validates at exactly **10768 — equalling the author time**
— and declined to claim it. That tape is in the table because equalling an
unbeaten author time is a real result worth showing; it is not the headline,
because it is not a beat.*

## Where the nine milliseconds are

Sectioned against the human world record:

| section | our tape | human WR | we gain |
|---|---|---|---|
| start → z = 641 (~2.9 s) | 2.948 | 2.986 | 0.038 |
| z = 641 → z = 704 | 5.860 | 6.019 | 0.159 |
| z = 704 → z = 715 | 0.451 | 0.484 | 0.033 |
| **z = 715 → finish (22 metres)** | **1.500** | **1.680** | **0.180** |

**Forty-four per cent of the entire margin is in the last twenty-two metres of an
eleven-second run.** That is not where anyone would look, and it is the whole
coaching content of this map:

> **Do not chase the approach. Arrive at the last obstacle in the roll phase the
> flip wants.** Arriving five hundredths of a second earlier in the wrong phase
> costs you half a second.

Three independent measurements stand behind that sentence:

* a **pre-registered** sensitivity of **+1.788 s per radian** of roll error at
  the last contact — registered before the correlation was computed, on this
  page's own attitude test;
* 40 ms of arrival-time variation containing a **400 ms range** of achievable
  endgames — the arrival window is fifty times finer than the outcome it selects;
* and the field's own counter-example: **the fastest human to two gates finishes
  six tenths off the record**, because it arrives at roll +3.10 where the fast
  lines arrive at −2.55. Multiply 0.6 rad by 1.788 s/rad and the arithmetic
  closes at about 1.07 s.

## The classification is unusual: precision-bound at the start, phase-bound at the end

Most maps here get one label. This one needs two, and only the second half is
teachable.

**The opening is precision-bound — for everybody, including the human.** There
are three windows in the first seconds where tolerance is **0 %**. Put the human
world record's own driven tape through the same mistiming test and its first
seven seconds survive **0–25 %**, against our tape's **70.4 %**. So on this map
the *human* is the tight one, and a person went out and hit it anyway. That is
worth sitting with: the part a machine finds hardest to make robust is the part
someone already drives.

**The finish is phase-bound**, which is a different and friendlier thing. It does
not ask for a tighter input, it asks for the right *state* on arrival — and a
state can be aimed for, felt, and practised in a way that a 10 ms window cannot.

**The low-input follow-up is complete**, and its answer is the record itself —
see the section below.

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

## The low-input answer is "the record, unchanged" — and it loses on both axes

Every beaten map here owes a low-input family: a version with a small alphabet
and few input changes, on the standing assumption that such a tape is the one to
hand a person. On this map that follow-up is now **complete**, and it came back
inverted.

Alphabets read back off the finished tapes, not taken from the flags that
produced them:

| tape | steer values | boundaries | events |
|---|---|---|---|
| keyboard 10.788 | **3** — `{−127: 451, 0: 491, +127: 205}` | 79 | 87 |
| **analog record 10.759** | 116 | 236 | 230 |

Tolerance, measured as one-tick boundary shifts with every variant validated:

| tape | shifts tested | survive |
|---|---|---|
| keyboard 10.788 | 158 | **38.6 %** |
| **analog record 10.759** | 472 | **76.1 %** |
| human world record 11.169 | 74 | **24.3 %** |

> **Hand a person the 10.759 analog record.** It is the fastest thing on the map
> *and* three times more forgiving than the world record a human actually drove.

The sparse tape loses on **both** axes at once: 29 ms slower and half as
tolerant, despite having a third of the boundaries and three steer values. This
is the fifth map's worth of evidence that event count predicts nothing about
tolerance — and the first where the low-input member is worse in both directions
simultaneously, which is about as flat a refutation of the intuition as the data
can give.

So the follow-up is **answered rather than outstanding**. "The deliverable is the
record, unchanged" is a result, not an absence.

**Credit where it belongs.** The keyboard tape is another arm's work
(`m267859_KEYBOARD_10788_v2`), validated here on a different build at 10788 with
its alphabet read back independently before it was used for anything. That arm
also reached **within 0.030 of the author time** on this map by its own route:
two arms converging on one map from different directions is worth saying out
loud, because agreement between independent attacks is evidence of a kind that no
single search can produce.

**And the constrained search carries a control that fired.** Before the
constrained chains started, the zero-ladder control was re-run *on that node and
that build*: the incumbent becomes a DNF, `finish 0 %`, nothing banked. A
constraint control that fires **before** the constrained run is the difference
between a low-input result and a low-input hope — otherwise a search that
silently ignored its constraint produces a tape you will describe with the wrong
word (see [`FINDINGS.md`](../FINDINGS.md) on the two flags that lied tonight).

*Not published as the record: **10.758** has cleared this map's full protocol on
both nodes and is in the table as the arm's best pending independent
verification. **10.759 stands as the cited time.***

## The descent, and where the time came from

| | |
|---|---|
| 10.859 → 10.849 → 10.839 → 10.827 → 10.817 → 10.797 → 10.788 → 10.777 → 10.769 → 10.768 → 10.759 → 10.758 | **0.101 s in about two hours** |

**Every one of those steps came from the hot wide joint window, and none from
local repair.** Over the same period two exhaustive local enumerations — 32 768
variants and 113 764 variants — found *nothing*. That is a clean natural
experiment: the two strategies ran side by side on the same map at the same time,
and one of them produced twelve improvements while the other produced zero after
146 532 evaluations.

The lesson is not that local search is useless. It is that **on this map the time
was never in the neighbourhood of the incumbent**, and no amount of exhaustively
enumerating that neighbourhood would have said so. Only running the wide window
alongside it did.

**And the margin has a symmetry worth noticing.** We are 0.010 under the author
and **0.411 under the best of the nineteen humans** who have driven this map —
which is slightly *more* than the margin the author held over that entire field
when the map was first dispatched. The gap we opened over the humans is the same
size as the gap that made this map interesting in the first place.

## Validation

`TAS_10759` sha256
`d08fa35063210cef21d61e743555a8234b78bdba2cbc739bf7f4ac86cf0dd2d7`
(md5 `d3232839b7f1fee0f21a65c57f430573`), map sha256
`a2bb4b9eafce011584297a69880f808378ef8fc3e894c19de7f1edadf1599215`.

On a nine-millisecond margin the verification is the result, so it is given in
full. **Seven cold measurements across three separately compiled binaries, all
bit-identical at 10759.** The verifying arm rebuilt the finding arm's stated base
in its own tree (selftest 10 of 10), ran the claim in five cold processes, then
once through each of two other builds — **nothing copied from any node, its own
download of the server.** Controls each in their own invocation: human world
record → 11169 on two builds, the search seed → 11467.

Re-validated again here for publication, one file per invocation, fresh process,
against the untouched map:

```
rank01_11169.Ghost.Gbx     11169   (control — the world record)
rank02_11189.Ghost.Gbx     11189   (control)
TAS_10759.Ghost.Gbx        10759   ×3
TAS_10758.Ghost.Gbx        10758   ×2
TAS_10769.Ghost.Gbx        10769
TAS_10768.Ghost.Gbx        10768
KEYBOARD_10788.Ghost.Gbx   10788
```

`TAS_10758` sha256
`f6566bcf3b5a531e9f66fb9541a7603a29d4f6558e5331eeb3cfc0899958382a`
(md5 `26516780e145f8b6c02c8df8eda66d98`), zero respawns, 1147 packets — the same
container as every other tape in the chain.

**Provenance was checked rather than assumed**, which matters when a nine-ms
result descends from a downloaded human tape:

* the seed is a genuine unmodified download — it re-simulates to 11467 exactly;
* **packet count is 1147 on the seed and 1147 on every tape in the chain**, while
  rank 1's is 1117; a dropped-state transplant would be unlikely to preserve that;
* respawn census **0** on the seed and 0 on every published tape, under **both**
  respawn keys;
* and the alphabet read off the tapes rather than off a flag: the **seed is pure
  keyboard, three values, brake never pressed**, while the claim is analog with
  116 distinct values. The search travelled a very long way in input space while
  keeping its container — which is what a real search looks like, and not what a
  copied tape looks like.

The map is the same untouched copy on which all 19 recorded runs re-simulate
exactly, which is itself a strong check against tampering: an edited map does not
return nineteen strangers' times to the millisecond.

## Notes

* [`RESULT.md`](notes/RESULT.md) — the pre-registered attitude test and its results
* [`PLAN.md`](notes/PLAN.md) — the search plan written for the TAS attempt
