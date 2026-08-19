# Which recording in a multi-node ghost is the author's lap — and how to tell when none of them is

Standalone rule, 2026-08-18, container agent. Cheap to apply, and it has
already caught one wrong file — mine.

A `.Map.Gbx` or `.Ghost.Gbx` body can hold **more than one**
`CPlugEntRecordData` node. 146612 holds 25. Taking index 0 because it is the
first one you find is a silent error: the file you build loads, decodes, and
shows a run that is not the one you named it after.

```bash
rec nodes M.Map.Gbx      # every node: offset, version, end time, sample count, first/last position
ct  probe --map M.Map.Gbx   # does the map hold a whole CGameCtnGhost, or only record data?
```

## The rule, in order

**1. If the map carries a whole `CGameCtnGhost` blob, use that one.**
`ct probe` shows `CGameGhost 00x` chunk ids and a `ghost inputs` (`0x0309201D`)
id when it does. That blob's record node is the author's validation lap, and
any other node in the body is something else. Use `ct mapghost`, which finds it
structurally rather than by scanning for the class id. 286279 and 238835 are
this case, and both re-simulate to the author time exactly.

**2. Otherwise match the node's END TIME to the author time**, allowing a
countdown lead-in of ~2.96 s. Only two values have ever been observed:

| map | node ends | AT | delta |
|---|---|---|---|
| 228811 | 20.550 | 20.555 | −0.005 |
| 228607 | 20.290 | 20.258 | +0.032 |
| 145875 | 9.300 | 6.343 | **+2.957** |
| 203330 | 16.950 | 13.995 | **+2.955** |
| 285268 | 52.250 | 49.282 | **+2.968** |

So `end ≈ AT` or `end ≈ AT + 2.96`, and nothing in between. The +2.96 s is
recording that continues past the finish line; the run is the same run.

**3. If no node matches, and the nodes start at DIFFERENT positions, they are
not laps of this map.** A lap starts at the map's spawn. Several recordings
beginning in different places, mid-map, are not attempts at the same thing.

## 146612 fails 2 and 3: it embeds no author lap

25 nodes = 13 distinct recordings (each of the first twelve appears twice), at
body offsets 357 k to 2 053 k. They begin at thirteen positions in six clusters
— (335–336, 42, 815), (352, 42, 819), (624–633, 34, 1010), (866–890, 15–18,
787), (620, 18, 617), (504–524, 15–18, 782–824) — several of them mid-map. End
times run 24.400 to 134.830 with nothing in common. The AT is 38.530; no node
ends at 38.530 or at 41.490, and the nearest is 40.730.

**`ATREC_146612.Ghost.Gbx`, banked earlier the same evening under §9g, was built
from node 0 on the assumption that index 0 is the author's. It is a 24.400 s
recording of something else. It has been withdrawn from `_container/ghosts/`.**

## Why the rule is worth the two minutes

The failure is silent in every direction that matters: the file loads, the
telemetry decodes, the sample count looks sane, and the only thing wrong is
*which run it is*. Nothing downstream can catch that for you — not the
validator (a watch-only file is unvalidatable by construction), not the
decoder, not a checksum. The end-time match is the only cheap discriminator,
and the start-position spread is the tell when the match fails.

Corollary for a map that passes rule 1: check it anyway. `ct mapghost` prints
the node range it took and `rec nodes` will show you whether there were others.
