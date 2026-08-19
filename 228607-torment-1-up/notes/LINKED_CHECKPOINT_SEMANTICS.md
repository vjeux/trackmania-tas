# FLEET NOTICE — `LinkedCheckpoint` semantics, settled: a SET is satisfied by
# crossing ANY ONE member — and check adjacency before hoping for a new route

Answers the semantics question opened by
`FLEET_NOTICE_order_repaired_and_LinkedCheckpoint_is_INVISIBLE_v1.md`
(md5 `18842bf89316555ef6d2a0a2ea67a4a9`). Settled on 228607 from data already on
disk, no new simulation.

## The geometry

```
item#8    GateCheckpointLeft32m    (736, 82, 752)     one 64 m gate,
item#9    GateCheckpointRight32m   (736, 82, 720)     two 32 m halves
item#10/11 Left/Right32m           ( 80, 50, 752/720)
item#12/13 Left/Right32m           (432, 18, 752/720)
item#0/1   GateCheckpointCenter32mv2 (1102, 93, 720 + 752)
item#1550/1 GateCheckpointCenter32mv2 ( 959, 57, 720 + 752)
item#19   GateCheckpointLeft32m  tag=Checkpoint  (1342, 218, 748)   plain, unpaired
```

**Ten `LinkedCheckpoint`s = five coincident pairs**, each pair identical in x and
y and 32 m apart in z, plus one plain `Checkpoint`. **5 + 1 = 6 logical
checkpoints.**

## Three independent confirmations

1. **Split count.** Three real human ghosts declare **7 splits**
   (`[4687, 8196, 9542, 11781, 15008, 18434, 20034]`) — six checkpoints plus the
   finish. Exactly 5 pairs + 1 single.
2. **The oracle's own ceiling.** Every non-finisher on this map returns `cps=6`,
   never 7 or 10.
3. **Proof by existence, and it is decisive.** A car cannot be at z=720 and
   z=752 at the same instant, yet **15 of 15 official humans finish.** If each
   member were separately required, nobody could ever complete this map.

> **Crossing one member satisfies the set.**

## The test to run per map, before hoping for a hidden route

The tempting inference — "a set of alternative required waypoints means
unexplored routing" — is FALSE where the members are adjacent:

> **For each `LinkedCheckpoint` set, check whether its members are ADJACENT (one
> wide gate assembled from 32 m item pieces — no routing freedom whatsoever) or
> SCATTERED (genuine alternative required waypoints).**

Adjacency is readable straight off `tmmaps list` positions. **No oracle calls, no
simulation.** On 228607 every set is adjacent, so that map has **no hidden
routing** — a confident negative rather than an open gap.

Maps carrying the tag: **228607 (10), 228811 (10), 199100 (5), 285268 (4)**. The
adjacency test is minutes for all four.

## The tooling gap is unchanged

`tag == "Checkpoint"` misses these entirely, so they are never ordered,
neutralised, or mentioned. **A segment map cut before a PAIRED waypoint still
requires its partner and cannot finish** — which is the failure this whole thread
started from. Validated lap times are unaffected on every one of these maps: the
game scores those and sees every waypoint.
