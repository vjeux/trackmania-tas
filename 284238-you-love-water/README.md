# YOU LOVE WATER — not beaten, but the map is one module repeated four times

**Author time 50.459 · the only human record 440.238 · best validated 97.325.**

| tape | validated | note |
|---|---|---|
| [`TAS_97325`](replays/TAS_97325.Ghost.Gbx) | **97.325** | the human's own driving with the retries cut, plus 0.136 s of search |
| human record, brick555 *(control)* | 440.238 | contains **31 respawns** |
| author time | 50.459 | — |

TMX map [284238](https://trackmania.exchange/maps/284238) · 4 checkpoints ·
**exactly one recorded run.**

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## The 8.7× headline is retry cost, not pace

The one human record contains **31 respawns**. On a Trial-family map the clock
runs through them, so a recorded time is clean driving plus every failed
attempt. Taking his own last, successful attempt in each sector:

| sector | last attempt | respawns in that sector |
|---|---|---|
| start → CP1 | 6.797 | 0 — clean |
| CP1 → CP2 | 13.163 | 1 |
| CP2 → CP3 | 24.428 | 1 |
| CP3 → CP4 | 25.788 | 9 |
| CP4 → finish | 23.738 | 20 |
| **total** | **93.914** | 31 = 346 s of pure retry |

So the real comparison is **93.914 against a 50.459 author time — 1.86×**, not
8.7×.

## The map is one module, placed four times

All 186 blocks are **free-placed**, so their positions live in chunk
`0x0304305F` rather than in the block records — which is why every block lister
showed an empty map and why nobody had read the layout.

Read properly, the map is **one module repeated four times: each copy rotated
−120° about a vertical axis at (772.286, 821.428) and dropped 56 m.** 129 of 186
blocks map onto a same-model block in another copy (worst case 0.85 m); the 57
that do not are exactly the final copy plus the start furniture. The four
checkpoints are exact images of one another, to 0.00 m.

Two agents derived this independently, by different methods, and agreed.

## So the lap is four attempts at the same problem — and it is lossy

Checkpoint crossing speeds decay **52.8 → 45.8 → 41.1 → 37.4 m/s**, while each
copy sits 56 m lower, which is worth **+10.5 m/s** of free height. A break-even
lap arrives *faster* at each copy. The human's loses about **16 m/s per cycle**,
drops below the jump threshold, mills in the bowl for 15–20 s, and enters the
next copy slower still.

The decisive feature is one **71 m gap, 32 m down**, between two loop blocks:

- **≥300 km/h at the lip** → flies it, **5.3 s** to the exit lane. Two attempts
  out of 23 manage this.
- **61–255 km/h** → falls into the closed end of the tube, **14.5–20.4 s** to
  climb out. Twenty-one attempts.

Of the record's 23 gap approaches in copies 1–3, **the fastest is 255 km/h
against the ~300 needed.** The standing start's two approaches are 299 and 302 —
the only ones that clear it.

**And the loss is in the launch, not the chute.** The standing start crosses its
water run at 89.8 m/s and CP1 at 52.8; cycle 1 crosses the same run at 96.5 and
CP2 at 45.8. A 14 m/s swing, entirely in the ramp → wall-curve → crossing.

## What the author time needs

`6.797 + 3F + (F−3) = 50.459` → **F = 11.67 s per cycle**, against **13.163 that
a human has already driven** — and he drove that entering at 218 km/h off a
respawn and dawdling to 190 in the chute. So the author time is **−11.4% on one
13-second obstacle, solved once and transported by an exact transform.**

## Why "just replay the good cycle three times" does not work

The obvious attack, and it was tested properly: **115 tapes** — every phase pair,
attitude-matched phases, with and without a respawn press, a 10 ms press-phase
sweep, and the best S-matched period in the whole record. **All died at the same
checkpoint.**

Measured reason: at the best positional handover phase the state is off by
**10.65 m, 2.11 m/s and 1.758 rad of yaw**. Position matching is not state
matching, and the transplant leaves the target line two to three seconds later,
inside the chute.

The next lever, stated with its blocker: **search the 150-tick launch for a state
match against the previous crossing, including attitude.** That needs per-tick
state, and the trajectory reader's blind locate aborts on this map — it refuses
to guess, twice, at about 5.5 minutes each.

## The 0.136 s that did validate

An exhaustive 10 ms prefix sweep at CP1 (375 tapes; only three phases survive,
364 dead — so the "one non-periodic phase" cutting rule rescues nothing here)
gave −0.100 s, and 49 minutes of search on the untouched map's own finish gave
−0.036 s.

**231,000 evaluations bought 0.036 s.** Evaluation rate is not the constraint
here (75–137 evals/s) — local search is. It polishes a line; it cannot delete a
bowl.

## The obstacle is reused on four other maps, and on one of them a human beats the author time

**This is the most useful thing found on this map, and it did not come from
searching it.**

284238's obstacle — the 40-block module placed four times — is not unique to this
map. Its author (Eating_My_Wings, 486 maps on TMX) reuses it byte-identically.
**279008 "Keep dropping" is this map with the launchers changed**: 167 of its 186
block records are identical — same block, same absolute position, same angles —
and its four checkpoint gates sit at the *same world coordinates*. What differs
is 19 records: 15 water ramps swapped for tech ramps, the start block, three ice
blocks nudged under a metre, the finish net — and **six boost-pad items that
279008 does not have**.

Its author time is 52.461, and:

> **Yhomas_TM holds 46.112 on it — a human, beating that map's author time,
> driving OUR obstacle, in a clean single-life run.**

That ghost is an answer key. It validates at 46.112 on its own map, it has **zero
respawn packets** and a single vehicle entity holding 923 of 923 samples, so
there is nothing to mis-decode. And it answers the questions this map had been
stuck on:

* it clears the 71 m gap **four times out of four**, at 314.2 / 323.1 / 300.9 /
  304.0 km/h — confirming the ">= 300 km/h at the lip" figure from a source with
  no connection to our decode of the 440.238 record;
* its checkpoint crossings are 65.4 / 69.4 / 68.3 / 69.1 m/s against our record's
  52.8 / 45.7 / 40.3 / 36.3. **His cycle is a high-energy fixed point; ours
  decays.**
* his cycle times are 11.257 / 10.417 / 10.870 — so the ~11.67 s per cycle that
  50.459 requires **is a thing a human does, repeatedly, on this geometry.**

### And it says what our field is doing wrong

**The launch fails on sideways velocity, not on speed.** All three launches we
can measure hit the ice kicker at 91–99 m/s. The two that work cross it drifting
toward −z (vz −17.9 and −25.1) and land on the wall curve **low**, where the
surface is tangent. The one that fails crosses at vz −3.2, flies past the tangent
part and slams into the wall 21 m higher, losing 1 630 of specific energy instead
of ~320.

**Our record never steers on the launcher lane at all.** Yhomas holds steer −1
for about 60 % of it.

An earlier claim on this page — that the six extra boost pads force too much
speed into a catch that wants to be met slowly — **is withdrawn**. The three
measurable launches sit at 90.9 (works), 97.2 (fails) and 99.1 (works). Speed
does not separate them; vz does. What survives is weaker: the pads deliver
+24 m/s inside about a second of lane, so the car spends the second half of the
lane ~20 m/s faster than Yhomas's does at the same x, which leaves **less
distance in which to yaw the car** before the kicker. And the counterfactual is
built and controlled — with the pads parked elsewhere, our record cannot even
reach CP2, so it *needs* them.

### It is already worth time on the untouched map

Sweeping a *constant, gentle* steer over the last 0.3–0.55 s of the water lane
(190 tapes) puts our car on Yhomas's wall-contact line **on the untouched map,
with the boost pads in place** — a probe that fires at 22.842–22.898 against his
equivalent 22.68. So **93.914 s of clean driving is not a surface limit on this
map; it is a line nobody drove.**

### How the sibling was found — the method is the reusable part

1. TMX v2 API by author id, paginated → 486 maps.
2. Download each at ~1 request / 1.5 s with a descriptive User-Agent (never a
   browser UA).
3. **Fingerprint by block census** — the count of each block model in the file.
   This map's module signature is distinctive enough to match on its own.
4. Confirm geometrically: sort `name,x,y,z,pitch,yaw,roll` and diff the two
   files. Identity of block *records*, not just of counts.

Every instrument used here carried a two-sided control, including the important
one: a probe at Yhomas's wall-curve contact point that **says no for our record
and yes for Yhomas at 15.278 on the identical geometry of 279008.**

> **A map author who reuses modules has published an answer key for every map
> that contains one. Before concluding an obstacle cannot be driven a certain
> way, look for a sibling map where somebody drives it well.** This one broke
> open a map we had written off.

## A trap this map found that defeats one of our own safeguards

A 14.7-minute search against a segment map reported **−13.975 s**. The winner
collects only CP1 and CP2 on the untouched map: it had found the promoted gate's
**enlarged trigger volume**, not a route.

**And the origin round-trip control passed throughout** — moving the gate back
to its true position reproduced the untouched map exactly, because the *position*
was restored correctly. The defect is in the **volume**, and a control that only
exercises position cannot see it.

> **A control validates the property it exercises, and nothing else.** When you
> adopt one, write down what it cannot see.

Substitutes, both with calibrations: a position-only rung mover, and a finish
placed 15–50 m *beyond* a real checkpoint so the checkpoint the candidate must
satisfy stays the map's own untouched trigger.

## Files

| file | what |
|---|---|
| `replays/TAS_97325.Ghost.Gbx` | best validated run |
| `notes/SIBLING_MAP_ANSWER_KEY.md` | **the sibling-map answer key** — how 279008 was found, Yhomas's cycle folded into module coordinates, and the launch diagnosis |
| `notes/LAUNCH_STATE.md`, `notes/TUBE_REACHABLE_SET.md` | the launch-state work that followed from it |
| `notes/RESULT-symmetry.md` | the four-copy derivation, the 115 transplants, the lossy-lap analysis |
| `notes/RESULT-v1.md` | the original recon: respawn splicing, the car-model fit, the search-yield measurement |
| `notes/GEOMETRY.md` | the independent block-graph derivation |
