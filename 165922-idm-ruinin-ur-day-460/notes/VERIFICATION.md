# 165922 — INDEPENDENT VERIFICATION of `v3/AT_BEATER_15549.Ghost.Gbx`

Agent vj4, 2026-08-19 00:04 PT, node **64455.od.fbinfra.net** (a different node
from every prior agent on this map), own build fork `/tmp/tmtas-vj165`
(tmtas-rs-hardened + lowinput v5 + tmsimp v5), own staging root `/tmp/vj165`.

## The claim, re-measured on the untouched map

```
tmtas validate --map /tmp/vj165/map.Map.Gbx --jobs 4 ...
file                                       sim_time      cps
CONTROL_human_8790769.Ghost.Gbx             8790769        -     <- known-answer control, exact
v3_atbeater.Ghost.Gbx                         15549        -     <- THE CLAIM, confirmed
v3_rr16461.Ghost.Gbx                          16461        -
v3_rrhuman20519.Ghost.Gbx                     20519        -
```

* Map md5 `1cc927bbb1d640c665ff69068352d4e6`, identical to the banked
  `map_mP8HzG68YxUY6yJcrQFx2inUjtk.Map.Gbx` download — the untouched map, no
  relocated gates, no probe surgery.
* Tape md5 `e094dfecdc97edb2b94ca6d3a48a9d1e`.
* Codec identity control: `tmsearch --template <tape> --verify` → rebuild
  re-validates at **15549**.

**Author time 15.643. This tape is 15.549. The AT is beaten by 0.094 s.**

## It is a LEGAL single attempt from tick 0 — checked, not assumed

The standing worry on this map was that any tape carrying respawns is bounded
below by the record's 8790.769 s. This tape carries **none**:

```
m165 respawns v3_atbeater.Ghost.Gbx
archive 0: fmt 12  start_offset -1540  packets 2109  (1 state literal)  0 with bit31
```

Zero packets with bit 31 of the 34-bit state literal set (the respawn bit, the
enumeration that `ghost::Factory` cannot see). `start_offset -1540` is the
ordinary 1.54 s countdown prefix a normal human tape carries, so the race
begins at tape tick 154 and the run ends 15.549 s later. For contrast, the two
sibling artefacts in the same directory (`respawnroute_*`) DO each carry exactly
one bit-31 packet at tape packet 321 — those are the respawn-route experiments,
and both are SLOWER than the AT (16.461 and 20.519). **The fast one is the clean
one.**

## Status

Map 165922's author time is beaten by a clean, legal, single-attempt tape from
tick 0, independently re-validated on a second node with a known-answer control.
The v3 agent had not yet written a result document when I picked this up; this
file records only my verification of their artefact. Provenance and method for
the tape itself are theirs to write up.

Continuing work by me (margin, technique classification, low-input family) will
be in `vj4_RESULTS_165922_v*.md`.
