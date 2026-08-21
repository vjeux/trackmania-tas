# 156451 "Snowcarmc free 42" — AUTHOR TIME BEATEN, and the human WR with it

**This is v2 and it SUPERSEDES `snow_RESULT_v1.md` (md5
`c5bfe8f245a416e9ace772c2329d96a3`), which is kept and says 18.691: the search
found one more millisecond after v1 was written, and the whole verification was
re-run on the new tape. Every number below is the 18.690 tape.**

Arm `snow`, node 74761.od.fbinfra.net, 2026-08-20. Store prefix `snow_`.
**Read `snow_FINDINGS_v1.md` first** (md5 `d349ed6fc21cc4e257096515f91de5a4`) —
the map's structure, the field, and the §8 build-split live there.

## THE RESULT

| | time | vs AT | vs human WR |
|---|---|---|---|
| Author time | 40.074 | — | +21.264 |
| Human WR (Roquett, 63 records) | 18.810 | −21.264 | — |
| **Our TAS** | **18.690** | **−21.384** | **−0.120** |

**Report this honestly: the author time on this map was already beaten by all 63
human records, by about a factor of two** — the author himself is rank 12 on his
own map at 19.610. Beating 40.074 is not an achievement here. The number that
means something is the human world record, and our tape is 0.120 under it.

## The deliverable

```
snow_TAPE_18690_FINAL_watchable_clean_v1.Ghost.Gbx
    md5 31f589d0622855424c99be2720e36797   F1994617155   13306 B
raw search tape (what the search produced, before regeneration)
    md5 96b52afde287b558646e8524db540648   F1994617161   13809 B
the 18.691 predecessor, kept: F1994615453 (clean) / F1994615454 (raw)
map (TMX copy == Nadeo CDN copy, byte-identical)
    md5 eec9f5730e9481d72361066c8d84c60c
    sha256 28098514cf251e55bd46bc5434d6a77522449f4077efd59148a90e8a93e9a947
tools + every sweep spec, with a README naming its base   F1994615462
    md5 abf5c17d589fa290606424f16063b154
```

**Provenance of 18.690, in the order it was established:**

* raw search tape, plain oracle, 3 runs on the working map copy: 18690 three times;
* deliverable (regenerated telemetry, declared time rewritten, identity
  scrubbed), **3 runs on a map copy freshly downloaded from the Nadeo CDN during
  the verification and 3 on the copy banked to the store**: 18690 six times;
* the raw dedicated server on that fresh copy:
  `"ValidatedResult": {"NbCheckpoints": 1, "NbRespawns": 0, "Time": 18690}`,
  `"IsValid": true`, `"DeclaredResult"` identical, `Can't load 0 %`,
  `Unvalidable 0 %`;
* `u02 info` reads back `declared_ms = Some(18690)`, `splits = Some([18690])` —
  both of the two places, agreeing with the header.

The tape is watchable: its telemetry is its own run, read out of engine memory
sample by sample. `tmtraj check`: C1 finite, C2 travels 713.2 m over 374
distinct points, C3 worst step 2.322 m, C4 no samples after the finish, C6
ground contact on 100 % of 191 ground-borne samples, C9 the throttle echo agrees
with the car's acceleration. **C8 fails and is a known false alarm on this map** —
see below. Identity: nickname `TAS bot`, trigram `TAS`, no club tag, no zone
flag, login `tmtas-research`, default car (the carrier's skin FileRef dropped);
`Login` in the server's own output reads `tmtas-research`.

**Tolerance — and a COUNTEREXAMPLE to "the fastest tape is also the most
forgiving".** `tmtol single`, +/-1 tick on every input event, no-op-corrected,
same instrument and settings on both tapes of one lineage on one map:

| tape | events | REAL survival | raw | no-op share |
|---|---|---|---|---|
| 18.700 | 839 | **68.9 %** (737/1069) | 80.2 % | 36.3 % |
| **18.690 (shipped)** | 797 | **48.5 %** (502/1036) | 66.5 % | 35.0 % |

So the 10 ms faster tape is **20 points LESS** forgiving. Four earlier maps
(286279, 274191, 145875, 227654) agreed that the fastest tape was also the most
forgiving; this pair does not. n = 1 pair on one map, both numbers oracle-read,
logs banked (`snow_TOLERANCE_single_1870{0,90}_v1.log`, F1994617166). 48.5 % is
still mid-range for this project (145875 29.8 %, 286279 9.6 %), so the shipped
tape remains a plausible human target — but "trade time for reproducibility is
backwards" should not be quoted as settled.

## How it was found

Seed: the downloaded human WR, which our oracle re-simulates to its exact
recorded millisecond. **18.810 → 18.753 → 18.750 → 18.743 → 18.730 → 18.725 →
18.711 → 18.702 → 18.700 → 18.691 → 18.690.** ~25 M oracle evaluations in ~5 hours on 176
cores. Four things mattered, in descending order:

1. **`--batch 480`, not the default 60 — a free 1.7×.** Matched pair, 20
   workers, same seed, same incumbent: 60 → 161 eval/s, 240 → 273, 480 → 299.
   Server init is ~40 % of per-candidate cost at batch 60 on an 18.8 s run.
2. **`--nops -3` (1–3 mutations per candidate) is what broke the final
   plateau.** Three chains sat at exactly 18.700 for over an hour — 3.6 M, 3.1 M
   and 1.4 M evaluations with **zero** improvement between them — and the
   multi-move chain then found 18.699 → 18.698 → 18.697 → 18.695 → 18.694 →
   18.693 → 18.691 → 18.690 over the next three hours. Single-move search was provably
   exhausted at that point (below), and this is the operator that answers it.
   Predicted by
   `aud_FLEET_NOTICE_coupled_pairs_invisible_to_single_change_iteration_v1.md`.
3. **Fork mode is a LOSS here**, measured: 165 ms/candidate/worker against the
   plain batched oracle's ~70. An 18.8 s run is too short to amortise forking a
   150 MB address space. (And without `FK_ANCHOR` every fork worker aborts with
   "state not located", while `fk btraj2` with the same anchor succeeds.)
4. **Window-restricted chains were worse than the same compute spent globally**,
   every time: hairpin (ticks 820–1160) 4 ms in 27 min at a 16 % finish rate,
   mid (1090–1540) 3 ms in 8 min, tail (1500–1882) 7 ms in 40 min, and the one
   stretch no chain had covered (600–900) 0 ms in 51 min. A second basin seeded
   from rank 6 (19.093, which owns 6 of 20 sectors in the splice) reached 18.810
   in 33 minutes and stopped; an independent lineage from the same WR seed with
   a different RNG reached 18.725 in 92 minutes. Neither threatened the main
   line.

## The single-move neighbourhood is EMPTY, measured two ways

`tmsearch --dump 40000` from the 18.702 incumbent, unbiased single moves:

```
49440 candidates      finish 24.4 %      improve 0.00 %  (0 of 49440)
cos 0/12193 · lvl 0/6752 · edge 0/4050 · acc 0/2480 · brk 0/1584
dbl 0/12111 · shift 0/4994 · scale 0/4952 · nop 0/324
0 improvements at every span from 0-5 ticks to 160+, and at every amplitude
from 0-5 units to 80+
```

Independently, **1 116 systematic single-window steer biases** — every 200 ms of
the lap × a 200 ms and a 500 ms window × ±3/±6/±12 units — produced **no
improvement**: 251 finishers, 866 DNF, mean loss 237 ms, identity present and
returning the incumbent exactly.

## Three negatives, each with the control that would have detected a positive

**1. The start trick is not available: race tick 0 is INERT on this map.**
`start_offset_ms = 0`, gas on at every one of the 1882 ticks. Gas off / full
steer / brake on at race 0.000 each return **the identity's own time to the
millisecond**; the same three edits at race 0.010 each **DNF**. The tick-1 row is
the yes-control — the edits are delivered and decisive one tick later, so the
tick-0 slot has no effect and "start on the second tick" is already the only
thing this map does. No re-search was needed and none was run.

**2. No throttle or brake input helps, anywhere.** 748 candidates (a 20 ms and a
50 ms pulse of brake-on and of gas-off at every 100 ms of the lap): 318
finishers, 431 DNF, **best = the identity**. The three ties are brake pulses at
9.4–9.6 s, in the hairpin, where the car is grip-limited rather than
power-limited. At fine resolution over the launch — a gas lift of 0/10/20 ms at
each of the first 150 ticks, 453 candidates — **only the tick-0 no-op survives;
every other lift DNFs.**

**3. There is no shortcut, though nothing enforces the route.** The map has no
checkpoints, so this was tested rather than assumed: 78 diverted tapes (full
lock left and right at ±40/±83/±127 units, held to the end, from each of 13
divert times spanning 11.5–17.5 s) — **every one DNF** except two late 40-unit
diverts that wander and rejoin (20.840, 18.959); identity in the same batch
returns 18.702. The structure agrees: a terraced hill climb (y 21 → 26 → 34 → 42
→ 50), the finish on the top terrace, the only links between terraces being the
two U-turn ramps, and the ice base ~40 m below the finish. Publish it as **"the
AT is not route-enforced"**, not as "a cut exists" and not as "we proved none
does".

## Two tool defects found here, both cross-map

**`u02 truncate` corrupts LIVE ticks on a tape with little tail.** It
unconditionally rewrites the last 40 packets with its locator signature. These
tapes are 1882 ticks for an 18.7 s run — ~10 ticks of headroom — so the
signature lands on 28 live ticks: **raw 18702, truncated 18703**, three runs
each, deterministic, bisected to that step, with the decoded-input diff showing
exactly the signature. On a sibling tape finishing at 18750 the same corruption
cost **0 ms**, because the garbage landed in the last 330 ms of a straight —
which is why it is easy to ship. Check `finish_tick + 40 <= n_packets` first.

**`tmtraj check` C8's 0.36 m wheel radius is a per-car constant.** This map's car
is a snow car (swapped in by `SnowGateGameplay` items). C8 REFUSES the
**downloaded** human WR at mode **0.4700 m**, and `fk whl`, locating the wheel
block in engine memory during our own regeneration, independently reports
**0.4701 m** — two sources 0.2 mm apart, 31 % above the constant. On this map a
C8 failure means "not a Stadium car", not "another run's wheels".

## What is left on the table

The splice bound over the human field is **18.528** (20 equal-arclength
stations; a bound, not a lap, and biased toward runs that cut inside a station
pair). Our 18.690 is best-in-field in 8 of the 20 sectors. The remaining 162 ms
sits mostly in sectors 12–16 and 20, all owned by rank 6, whose line through the
last U-turn is **wider and 3–5 km/h faster** than ours all the way to the line
(apex z 892.6 vs 888.1, max x 1166.3 vs 1162.2). A search seeded from rank 6
does not get there — it plateaus 120 ms behind — so if anyone picks this map up
again, the question worth attacking is how to move our line onto rank 6's U-turn
without losing the first third, and `--nops -3` or larger is the operator that
has been shown to move this tape at all.
