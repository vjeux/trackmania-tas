# 286279 `[Turtle Trial] Leto` — low-input work, v2

Sidecar to `RESULT-lowinput-v1.md` (md5 `671798b0e9152039aea67f1af014988e`).
v1 stands except where corrected below. Two corrections, both from the map
owner, both narrowing my claims; plus the tolerance measurement, which is the
number that actually tells a human whether to attempt this map.

Times are seconds. Author time **355.181**. Owner's run **218.812**.

---

## A. Correction 1 — my rigidity probe measures MARGINAL freedom, not joint

v1 §2 reports that single and 3-event deletions almost never survive, and v1 §4
reports 19 survivors on the author's lap. Both are **one-at-a-time** results, and
the owner produced the counterexample that shows they cannot be read as joint
statements. On the author's lap, inside the causally-erased stretch:

```
blank ticks 25290..25399  (110 ticks)  -> 355181   free
blank ticks 25400..25834  (435 ticks)  -> 355181   free
blank ticks 25290..25834  (the union)  -> DNF at cp2
p_AT (control)                         -> 355181
```

Neither half moves the time by a millisecond; the union kills the run. This is
the "individually-exact cuts do not compose freely" phenomenon from 238835,
reproduced in *blanking* rather than cutting, and inside a stretch that is
supposed to be causally dead.

**What is and is not affected.**

- **`ddmin` is safe.** It evaluates the *cumulative prefixes* of the passing
  blocks and keeps only the longest prefix that still meets the budget, so a
  combination that fails is simply never accepted. The 1-minimal certificate is
  also a marginal claim by construction — "no single remaining event can be
  deleted" — and that is exactly what it says.
- **`probe` output must be read as marginal.** "6 of 177 single deletions are in
  budget" does **not** say those six are jointly removable, and by symmetry a
  position that looks dead may be alive in company. I have said so in the v5
  add-on so it does not get quoted as joint by the next reader.

**A partial bound on the joint question**, since it is cheap: 119 *adjacent
pairs* on the 1-minimal tape, both members individually fatal — **all 119 dead,
0 over budget.** So on this lineage the marginal result is not hiding an obvious
pairwise win. That is a bound, not a settlement: pairs at distance, and larger
non-contiguous sets, are untested.

## B. Correction 2 — "erased" may be conditional on the car staying put

v1 §4 says anything inside a stretch that ends in a respawn is causally erased.
The owner's candidate mechanism for the boundary in §A, **unconfirmed and
labelled as such by them**: a respawn restores the state at the *last* checkpoint
crossing, and a car left hands-off can wander back across a checkpoint and
**overwrite that saved state**. If the union-blanked car re-crosses cp2 where the
half-blanked ones do not, the respawn restores a different state and the winning
attempt starts from the wrong place — which is the cp2 DNF observed.

If that is right it is worth more than the boundary itself: **the checkpoint
state is mutable during a retry loop**, so erasure holds only while the car stays
on the far side of the checkpoint it will respawn to. Named suspect, not a
finding. It is the first thing to test on the next trial map.

**A correction in my favour, also from the owner:** the ~1.02 s respawn freeze
windows really are inert. Throttle off, throttle held, full lock, brake — all
four return 355181. The advice that those ticks are free stands.

## C. Tick tolerance — the difficulty number

Method: shift one input by ±1 and ±2 ticks and ask whether the run still beats
the author time (`tmsimp --mode tol --maxshift 2 --worst 135599 --every 8`),
sampling every 8th event of the 1-minimal tape. 104 events sampled, 416
evaluations, plain oracle throughout.

| surviving window | inputs | share |
|---|---|---|
| **1 tick** (only the exact tick works) | **94** | **90 %** |
| 2 ticks | 2 | 2 % |
| 3 ticks | 3 | 3 % |
| 4 ticks | 2 | 2 % |
| 5 ticks | 3 | 3 % |

**Median tolerance is zero: 90 % of inputs die if you are 10 ms early or 10 ms
late**, and dying is literal — the failures are DNFs, not slower finishes,
consistent with v1 §2. The generous end of the distribution is 5 ticks, i.e.
±20 ms, on three of 104.

Two things follow, and they are the honest summary of this map:

1. **The tape is not a human deliverable and no amount of searching will make it
   one.** 832 inputs, 90 % of them on a single-tick window, over 218 seconds.
   There is no simplification left to find: v1 established you cannot delete
   them, and this establishes you cannot mistime them either.
2. **What transfers is the method and the map knowledge, not the tape** — the
   three sentences in v1 §6, of which *respawn early* is the one that is worth
   real time to a human.

## D. The author's own lap, minimised — where their time went

A minimiser run on the **author's own validation lap** at budget 355181 (ties
allowed), i.e. "how much of the author's input actually does anything":

```
1453 events, 355.181  ->  831 events, 354.781   1-MINIMAL
```

Note the time: **the minimiser deletes its way past the author time.** No new
driving, no search, no line change — just not pressing **622 of the author's own
1453 inputs**, and the lap comes home **400 ms faster** than the author time it
was built from. The first two steps were single blocks of 364 and 181 events,
worth exactly 0 ms: the retry loop of v1 §4, deleted wholesale, because it was
already erased.

Cold-validated with two controls exact in the same batch:

```
AUTHOR_AT_355181_PLAYABLE.Ghost.Gbx   355181   (control)
m286279_BEST_218812_v7.Ghost.Gbx      218812   (control)
authormin.Ghost.Gbx                   354781
```

That is the cleanest available statement of where the 136 s went: **almost all of
the author's input in the middle of that lap does nothing at all**, and the
handful of deletions that do change the time make it *better*.

One symmetry worth noticing. The author's 1453-input lap and our 885-input run
minimise to **831 and 832 events**. Two lineages 136 seconds apart in finishing
time, arrived at by completely different routes, bottom out within one input of
each other — which is what you would expect if the irreducible part of this map
is the balancing itself, and everything above that floor is retries on one side
and optimisation slack on the other. It is the same shape of convergent evidence
as 203330's two lineages both landing on twelve, and I would treat it the same
way: suggestive, from an independent starting point, and not a proof of a map
constant.

## E. What v1 got right and I would still publish

- 885 → 832 events, **1-minimal**, 219.581, cold-validated twice with controls.
- k=1: 6 in budget, **0 over budget**, 171 dead — on this map a missing input
  never costs time, it kills you. Read as marginal, per §A.
- Deaths are local: every dead run reached exactly the checkpoints passed before
  the deletion point, never one more.
- Metronome refuted: 174 square-wave throttle patterns, zero finishes; measured
  first at 4 % of gaps within a tick of the median.
- Rests: 4 of 55 half-second hands-off windows survive, and all four are the
  respawn freezes or the run-out.

**And the bound on the generalisation, which came from another map:** the
0-of-177 "deletions never merely cost time" figure is *not* a general property of
TAS tapes. On 203330, exhaustive singles give 2 of 11 (and 3 of 30 on its
31-input lineage) that merely cost time. That map degrades lethally in its spine
and gracefully in its endgame; Leto degrades lethally everywhere because its
headroom had already been harvested out of it by the author-cut. The Leto figure
is about erased retry time, not about tapes.

## E2. The dead-zone question, answered by the probe rather than by a strip

I owed the owner a dead-zone measurement made **on this lineage** rather than
inherited from another one. The exhaustive single-deletion enumeration already
is that measurement, and it is more direct than a strip: on the 1-minimal tape
there are exactly **six inert inputs**, all of them at a respawn boundary or in
the run-out, each costing 0 ms. There is no dead zone on this lineage in the
sense the strip mode looks for — no axis band whose inputs can be removed
wholesale — and per §A that is a marginal statement like the rest.

## F. Artefacts

In `286279/lowinput/`:

| file | md5 | what |
|---|---|---|
| `kb286279_1minimal_832ev_219581.Ghost.Gbx` | `b241b28c9a2886bd66c2bdf444aad71f` | 1-minimal, 219.581 |
| `kb286279_authormin_831ev_354781.Ghost.Gbx` | `e022950acdf5595aa74c466d1166d99a` | the author's own lap, 1-minimal, 354.781 |
| `log_tolerance_v1.txt`, `log_probe_pairs_v1.txt`, `log_ddmin_authorlap_v1.txt` | — | §A, §C, §D enumerations |
| `kb286279_thin_842ev_220348.Ghost.Gbx` | `d7766b8b254f21da8e284adb05c5736b` | greedy thin of the author-cut, 220.348 |
| `log_ddmin_relaxed_v1.txt`, `log_probe_block1_v1.txt`, `log_probe_block3_v1.txt` | — | the enumerations |
| `RESULT-lowinput-v1.md` | `671798b0e9152039aea67f1af014988e` | v1 |

Fleet build: `tm-map2/tmtas-rs-hardened-plus-lowinput-v5.tgz` md5
`342de2f47ee25cd7127bd90272ec2837`; `LOWINPUT-ADDON-v5.md` md5
`1c1cacaf1ad866c63d92616b03dc1916`; `tmsimp-v5.rs` md5
`d4292bf345790eb8a5efee4794132ae6`.

## G. A diagnostic worth keeping, from the 203330 exchange

Their observation: the validator reports `ceil(t_true)`, so it is a 1 ms ruler —
but if you know the incumbent's true crossing sits 0.304 ms above the integer
boundary, only an improvement of at least 0.304 ms can change the integer, and
the coarse ruler becomes a sharp one. *A coarse ruler is precise when you know
where on it you are standing.*

The inversion, which is the cheap diagnostic: **before trusting a plateau, work
out where the incumbent sits inside its millisecond.** That fixes the smallest
improvement the integer ruler is capable of displaying. A tape sitting at .02 of
a millisecond is blind to anything under 0.98 ms, and a flat search log there is
not evidence of a floor at all.

**Precondition, from my own incident on 227969:** knowing the true crossing needs
an instrument other than the validator, and on that map the sub-tick plane was
wrong by 19 ms because the finish is crossed airborne with 1.5 rad of roll
variation. So the diagnostic is only usable as *plane-verified crossing, or no
claim* — gated on the crossing-coordinate spread against the `v × 1 ms`
quantisation budget. Where the precondition fails, the ruler stays 1 ms wide and
a flat log is uninformative in both directions.
