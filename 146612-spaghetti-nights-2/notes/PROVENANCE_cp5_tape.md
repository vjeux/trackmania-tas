# Provenance — the upstream half of `s5_LAP_39478_v1` (and of `s5_LAP_39460`'s target)

Written 2026-08-19 by `w612` (node 145855) at the coordinator's request, after
the publisher flagged two published tapes on 146612 with no banked provenance.

**Neither `s5_LAP_39478_v1.Ghost.Gbx` nor `s5_LAP_39748_v1.Ghost.Gbx` is mine** —
both carry the `s5_` prefix and belong to the sector-5 arm (session
`800133d6`, node 46836), which is the session that must document how they were
driven home. This sidecar documents only the part I can attest to: **where
`s5_LAP_39478_v1`'s first 33 seconds came from**, because it is my tape.

## The identification

`s5_LAP_39478_v1.Ghost.Gbx` is 28 431 bytes. So is
`w612_final/w612_CP5_33158_v1.Ghost.Gbx` (md5
`4ab9b195c892cf924b55d3172b6e0c50`), which I handed to the sector-5 arm at
08:11Z and which it confirmed verifying by size and md5 before running. The
sector-5 arm's own answer sidecar (`s5_ANSWER_to_w612_fast_cp5_v1.md`) describes
seeding a sectors-4-and-5 window from exactly that tape. So 39.478 is, to the
best of my knowledge, **my CP5 delivery vehicle with its last two sectors
re-driven by them**; they should confirm and state the search that did it.

## Provenance of the upstream part

| | |
|---|---|
| **what** | `w612_CP5_33158_v1.Ghost.Gbx` — reaches CP5 at **33.158** on `seg5` |
| **seed** | human rank 1 (`rank00001_40223.Ghost.Gbx`, native, 40.223) |
| **map** | `w612_map_seg5.Map.Gbx`, built by `tmmaps build --ref-ghost rank00001_40223 --order 439,494,440,633,492` from the untouched map (sha256 `c6cca762…`); all six segment maps `exact=true` against the reference ghost |
| **search** | `tmsearch`, classic path, 26 workers, `--lo 1990 --hi 3360` (the joint window over sectors 3 **and** 4), `--temp 12 --migrate 0.04 --window 110 --stride 55 --nops 2 --seed 91034`, own staging root `/dev/shm/w612-s34a`, own `--bestdir` |
| **scored how** | **time at CP5**, not at CP4. This is the whole point of the tape: the same window scored at CP4 produces a tape 263 ms faster to CP4 that returns `DNF cps=4`, and repairing sector 4 from it found **0 finishers in 21 870 evaluations**. Scored at CP5 the first improvement arrived in **60 evaluations**. |
| **phantom guard** | on (default). Every banked incumbent re-validated through the plain oracle from the file before acceptance. Zero phantoms in every arm I ran tonight. |
| **`btraj`-clean?** | **Yes.** `fk btraj --allow-dnf --tick 1800`, self-check `\|q\|−1 max 1.57e−7`, `\|d(pos)/dt − v\|` mean 0.457 m/s, 0 clock gaps. Passes CP4 at `(766.8, 18.0, 587.8)` — 2.66 m from the gate — at 113.1 m/s; tracks the human world record's own line through the whole of sector 4 within **0.6–6.7 m**; crosses CP5 at `(1179.6, 42.0, 736.9)`, inside the road surface (`cx=36`, x ∈ [1152, 1184]), at **87.8 m/s** against the record's 75.3. Trajectory banked as `146612/w612_S34_cp5_trajectory-v1.csv`. |
| **what it is NOT** | a lap. It is a `seg5` tape: past CP5 its inputs are rank 1's, driven from a state 426 ms and 12.5 m/s away from rank 1's, and it stalls. It was handed over explicitly as a **CP5 delivery vehicle**. |
| **sectors 0–2** | untouched rank 1: CP1 7.311, CP2 15.718, CP3 19.980. None of my sector-0 or sector-1+2 work is in this tape. |

## The one number worth carrying out of it

The sector-5 arm has now measured what that entry is worth, and **it is negative
at the finish**: from CP5 33.143 airborne, sector 5 costs 7.073 against 5.992
from a planted 33.756. So `w612_CP5_33158_v1` is a **correct measurement of how
early CP5 can be reached and a bad way to reach it** — 317 ms earlier at CP5,
756 ms slower at the line. Their `s5_LAP_39460_v1` at **39.460** is the map's
best lap and it arrives at 33.325 planted.

That is not a retraction of the tape, it is its result: it established the left
edge of sector 5's entry optimum. The corrected objective for my sectors 3+4 is
**not** "earliest CP5" but **"fastest CP5 that still arrives planted and pointing
down the road"**, which on their curve is around 33.3.

## Files

```
146612/w612_final/w612_CP5_33158_v1.Ghost.Gbx   28431 B  md5 4ab9b195c892cf924b55d3172b6e0c50
146612/w612_S34_cp5_trajectory-v1.csv           the btraj trace above
146612/w612_final/w612_map.Map.Gbx              the untouched map, sha256 c6cca762…
146612/w612_final/rank00001_40223.Ghost.Gbx     the seed, and the control in every batch
146612/w612_final/rank00002_40226.Ghost.Gbx     the second control
```
