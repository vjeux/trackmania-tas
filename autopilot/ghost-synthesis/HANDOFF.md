# Handoff

## Status

The donor-free writer now selects the semantic RoadTechStart correctly. The causal field is validation chunk `0x0309202D` U03, now named `validation_start_index` in code and `start_checkpoint_index` in manifests.

The value is derived from the map as the count of non-Spawn block waypoints. The five-value Summer sweep maps U03 directly onto the engine checkpoint array, and independent recordings agree on Summer (`3`), the repository test map (`2`), and Training - 10 Long (`1`).

## Proven

- Early stationary Summer state is 0.0005 m from semantic RoadTechStart.
- Moving only RoadTechStart +64 m moves the authoritative vehicle +64 m.
- Training - 10 Long starts from its differently oriented RoadTechStart and responds oppositely to hard-left/right.
- A standalone generated Summer file reaches 137.854 m from the real start and has opposite hard-left/right response.
- U03=0 fails the enforced start check at 394.633 m, at the map's checkpoint-zero location.
- Archive format, field0, start offset, validation seed, other validation fields, result metadata, account id, record samples, and packet compression were independently ruled out as the start selector.

All authoritative state uses `validator job → simulation → controlled participant → CGameVehiclePhy → state`; there is no candidate scan or fallback.

## Not yet achieved

- CP1 from the corrected start.
- Finish from the corrected start.
- Client import/render.

Two five-minute CP1 searches are banked in `evidence/summer01/cp1-search/`. The broad run reached route station 39 / 780 m and the tighter gate run reached station 25 / 500 m, but every plain-oracle check remained `cps0`; neither is a CP1 result. The historical 36.011 file starts at the last checkpoint and must not be reused as a success claim.

## Next command path

Use the corrected template writer (no validation override is needed):

```text
tmauto synth write --map MAP --out template.Ghost.Gbx \
  --ticks 3000 --steer 0 --wobble-prefix 160 \
  --declared 30000 --cps 3 --record grid
```

Then continue the gate search with `tmexplore-real run`, but tighten the actual checkpoint-plane objective rather than merely extending either banked cps0 tape. Every candidate promoted past CP1 must be re-simulated by the plain oracle and report `cps1` or greater.
