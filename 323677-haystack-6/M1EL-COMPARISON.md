# Comparison: m1el Haystack 6 graph vs driven oracle results

Compared 2026-08-26 against [gist `fe744797fc68771de43362902bdca9c6`](https://gist.github.com/m1el/fe744797fc68771de43362902bdca9c6)
and the stock map MD5 `440c2bc538d9fcff2bde3f595c0d2f21`.

## Bottom line

The gist is a strong **waypoint / authoring-graph extraction** and agrees with
ours on the map census and most special structures. It is **not a faithful
vehicle transition graph**. Its largest forced structure—eight burnable
TrapBox escapes—is physically impossible from the state where an ordinary
edge actually places the car. Its clean Eulerian/exact-once reduction also omits
the measured facts that one physical move can emit several checkpoint events,
events for already-held groups are suppressed, and the last *fresh* event
selects the respawn destination.

Use its corridor/funnel constraints and special-LID census as solver hints after
re-validating them against the driven transition catalog. Do not use its
TrapBox/family model or its exact-once Eulerian reduction as correctness claims.

## Exact agreements

- Census: 6,396 gate items, 323 LIDs including finish 0, 1,176 lattice rooms,
  18 special sealed-lid rooms.
- Its eight `double take` LIDs are exactly our eight driven bonus groups:
  `1052392961`, `1192721329`, `1284970017`, `1324517265`, `1387778849`,
  `1875528577`, `1973621873`, `2123535873`.
- Its geometry for them is right: each bonus `CPBox` is one meter above a
  directional ordinary gate. In the fresh case our driven event order is bonus
  first, primary second; the ordinary respawn destination follows.
- Its eight `trapbox-escape` LIDs are exactly our eight formerly-open groups:
  `1055171172`, `1298681604`, `1399226756`, `1458121476`, `1843038516`,
  `1885379892`, `2111633252`, `2133528403`.
- Its four starter/hub LIDs map exactly to four driven center groups:
  `1298681601`, `1492404801`, `1795403329`, `2085715921`.
- The center/funnel is highly one-way, not an all-to-all free teleport. Our
  driven center graph has 206 measured states, 345 arcs, and 198 SCCs.
- The gist's solver wall—roughly 10 missing / 15 duplicate—resembles ours:
  local searches repeatedly produce long walks that are not legal once-only
  runs.
- The gist's low-confidence RNG/label hunch is consistent with our controlled
  permutation test: the map's `Order` values carry no measurable route
  adjacency signal.

## Material differences

### 1. The TrapBox escape edges are not drivable

The gist models each of 18 TrapBoxes as five one-way entries plus a burnable
`CPBox_U` escape to its host room, then concludes every solution must choose
exactly eight boxes (576 box choices) and avoid the other ten.

Driven result:

- Every ordinary entry places the car on the lower sealed lid at plane+7.
- The upside-down `CPBox_U` trigger begins 0.265 m above the car.
- 1,800 input probes over all 18 rooms never fired it and reached at most
  0.109 m of air. The same grid at ordinary lids fires 90–91/100 and reaches
  0.455 m.
- A second mirror of 540 respawn-press probes produced one outcome every time:
  counter 0, return to the same landmark.
- The upper plane+15 story is solid and drivable, but unreachable from the
  lower lid. From above, 15–17/80 inputs leave; from below, none do.

Therefore those 18 rooms are dead **origins**. They are valid destination-only
sinks and can appear only as the last move. The gist's `burnable_box_escape`
edges are authoring targets, not reachable vehicle transitions. Consequently
its 576 box combinations, hub↔escape coupling, 32 families, and related forced
assignments are not valid physical constraints.

The same eight LIDs are collected elsewhere: each has a plain `CPBox` on the
center's y=116 ring. All eight have direct driven proofs.

### 2. The Eulerian exact-once formulation is stricter than the engine

The gist says to choose exactly one edge per LID and obtain a connected Eulerian
trail of 322 graph edges. The engine rule is:

```text
fresh = [g in ordered_events(physical_move) if g not in held]
legal iff fresh is non-empty
held' = held union fresh
destination = respawn_state(last(fresh))
```

A physical move can emit two or three group events. A move may cross an
already-held primary and remain legal if another event is fresh. Thus
transitions are set-labelled and history-dependent, not single-color static
edges.

The gist's Virtual pass-through nodes are a useful way to represent bundled
fresh events, but only for the all-fresh branch. They do not represent event
suppression or the changed destination when an earlier event was already held.

### 3. The eight hub cascade event orders swap their first two events

Example H:

- gist virtual chain: `1458121473` (ordinary) → `1055171172` (open) →
  `1795403329` (center)
- driven trigger order: `1055171172` (open) → `1458121473` (ordinary) →
  `1795403329` (center)

The same ordinary/open swap occurs in all eight A–H cascades. With all three
fresh, coverage and final destination happen to agree because the center event
is last. Under held history, order changes which event is last-fresh and hence
where the car respawns.

### 4. Static destinations fail under held history

At one driven bonus pocket, fresh order is bonus `2123535873`, then primary
`2034130465`. Fresh run: both fire and respawn follows the primary to the
neighboring lid. With the primary collected earlier: only the bonus emits, and
respawn ends at `(953.077, 63.078, 915.002)` inside the bonus pocket.

The gist has one static target per event/virtual edge, so it cannot express this.
The bonus-only destination is terminal: all outgoing attempts cross only the
now-held `{bonus,primary}` pair.

### 5. Its "known valid run" does not validate this graph

The map ships `validated=1` and author time 911.615, so a retail-build author run
existed. But the current dedicated server freezes the vehicle at the map's
Rally gate, and the current retail `/validatepath` also cannot reproduce the
start. More importantly, an author run does not prove that the gist's extracted
TrapBox exits or event ordering are correct. Those need vehicle-oracle controls;
the TrapBox controls fail.

### 6. Minor internal count drift

The notes still say 1,251 rooms / 54 ExitFunnels, describing an older graph. The
current CSV has 1,274 rooms: 1,176 Lattice + Start + Hub + Finish + 53
ExitFunnels + 18 TrapBoxes + 24 Virtual rooms. This is documentation drift, not
a conceptual issue.

## Useful pieces to import

- One-way corridor/funnel locking and endpoint balance constraints.
- The 24 non-generic LID inventory and two two-gate chokepoints, after checking
  each against the driven catalog.
- The eight bonus-pass-through locations and four start/center LIDs.
- CPBoxB's authoring-order anomaly as a heuristic only; “therefore intended
  route” remains conjecture.
- The full gate-item census and source-file ordering as cross-checks.

## Current driven ground truth

- All 13 formerly unexplained groups have direct driven proofs; all 322 are
  individually collectable.
- Lattice catalog: 22,268 usable-origin physical arcs; 29 multi-label arcs, all
  and only the eight known bonus pockets; 1,751 boundary arcs driven; 20,517
  geometry-confirmed; zero label mismatches.
- Center: all eight open groups are collected in three-event cascades on the
  y=116 ring; five center groups have direct proofs.
- Best complete driven route remains 278 moves / 278 groups. The old 301-move
  walk reaches 291 groups but has ten moves with no fresh event and is invalid.
