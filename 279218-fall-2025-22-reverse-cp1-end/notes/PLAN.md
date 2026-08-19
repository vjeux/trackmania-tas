# PLAN — unbeaten.at 279218, "Fall 2025 - 22 Reverse CP1 End"

uid `_Toadb_vTfXnT7PfAIpHypSJClk` · Nadeo mapId `98556b9d-3c90-402a-95ac-70fef40781a8`
AT **5350** · human WR **5355** (Matik_K) · 339 records · gap **5 ms**

Written 2026-08-18 BEFORE any search, from measurements taken on this map.
Every number below was measured, not assumed. Recon done on
`21462.od.fbinfra.net` (which then died); re-verified on the replacement
`84499.od.fbinfra.net` (176 cores). Scratch `/tmp/m279218`, durable
`~/persistent/private-30d/tm-unbeaten/279218/`.

---

## 1. Acquisition and the identity control — PASSED

ACQUISITION.md's recipe worked verbatim. `map.Map.Gbx` (1,164,721 B, `GBX…BUCR`),
plus **40 finishing human ghosts**: the top 15 (5355–5362) and five each from
leaderboard offsets 25 / 50 / 100 / 200 / 339 (5364 … 5854), i.e. a population
spanning rank 1 to rank 344.

```
tmtas validate --map <abs>/map.Map.Gbx --jobs 25 <abs>/ghosts*/*.Ghost.Gbx
```

**40 of 40 re-simulated to their exact recorded millisecond** (re-run 15/15 on
the replacement box after the rebuild). The candidate factory also round-trips:
`tmsearch --template r001_5355 --verify id.Ghost.Gbx` → validates 5355. That is
the identity control; `ghosts/r001_5355.Ghost.Gbx` travels in every batch.

## 2. The medals say the AT is a driven lap

| | |
|---|---|
| authorScore | **5350** |
| gold / silver / bronze | **6000 / 7000 / 9000** |
| author | `in-.-` (Uruguay), who is also **rank 5 on the board with 5358** |

Gold/silver/bronze are round thousands — placeholders, not Nadeo's generated
ladder and not hand-tuned. The AT is not derived from them. `in-.-` authored
this whole "Reverse CP1 End" series (the same account authored
`Training - 10 Long` in ACQUISITION.md) and their own **public** best here is
5358, eight milliseconds slower than the AT they published.

Conclusion: **5350 is a driven validation lap**, set by the author in the editor
where they could retry indefinitely, and it is 5 ms better than 339 players have
managed online. It is reachable, because someone reached it, and a
human-repeatable technique therefore exists.

## 3. What the map actually is

`tmmaps list` — exactly **two waypoints**:

```
block#6185 RoadTechStart tag=Spawn  cell=(35,15,11) yaw=1.5707964
item#1394  GateFinishCenter16mv2 tag=Goal cell=(30,14,12) pos=(971.2952, 50, 400) yaw≈0
```

No checkpoints (`tmtas validate` prints `cps = -` for every ghost). This is the
"CP1 End" construction: the campaign map's CP1 has been replaced by a
free-standing 16 m finish gate.

From the WR's decoded telemetry, the whole course is **201.5 m and 5.355 s**:

| phase | ms | what happens |
|---|---|---|
| launch | 0 – 1750 | standing start at (1136, 58, 368) facing −x; straight, slightly falling; 0 → 100 km/h |
| descent | 1750 – 3500 | still straight along −x, dropping y 58 → 48.5, 100 → 176 km/h, tiny right corrections (steer +0.20…+0.41) |
| **the corner** | 3500 – 5200 | one continuous **left-hander at full lock**; y dips to 43.2 then climbs back to 50; heading rotates from −x to +z; speed saturates at **203.15 km/h** and stays pinned there ~1 s (grip/drag equilibrium); brief airtime at 4800 |
| exit | 5200 – 5355 | straight along +z, steering released, speed climbs again 209 → 213 km/h into the plane |

**Gas is 1 on every tick and brake is 0 on every tick, for all 40 runs.** Nobody
lifts, nobody brakes. **Steering is the only control that does anything**, and
the run is one corner. The input tape is **559 ticks at 10 ms**; the finish
falls at tick 535.5.

## 4. Where the finish plane is, exactly — and the vernier works

`tmmaps probe … --at 971.2952,50,400 --cell 30,14,12 --yaw 0 --axis z --range -8:4:1`
relocates the goal gate along the direction of travel and re-times three ghosts:

```
z off |  r001_5355 |  r002_5357 |  r015_5362
 -8.0 |       5219 |       5220 |       5226
 -4.0 |       5288 |       5289 |       5294
 +0.0 |       5355 |       5357 |       5362   <- identity: exact, all three
 +4.0 |       5423 |       5425 |       5430
```

1. **The relocation is exact.** At offset 0 the rebuilt map returns each ghost's
   true published time to the millisecond, even though the probe swaps the model
   to `GateFinish32m`. The vernier instrument is trustworthy on this map.
2. **The plane is perpendicular to +z** and the timing is a clean
   **17.0 ms per metre** (58.8 m/s), linear over ±8 m — so the reported time is
   a sub-tick interpolated plane crossing, not a tick index. Corroborated by the
   times themselves: 5355/5357/5358/5359 are not multiples of the 10 ms tick.
3. Therefore **the objective is "how far along +z am I at a given instant"**, and
   **1 ms = 1.7 cm**. The 5 ms we need is **8.5 cm of extra reach**. It is a
   geometry problem, not a timing problem.

The vernier's job is to **resolve ties**: two candidates both reporting 5350 can
differ by 0.99 ms; sweeping the plane in 1 cm steps until each one's reported ms
ticks over separates them to ~0.17 ms. That is how the final replay gets chosen
and how a margin gets stated instead of a coin flip.

## 5. Where the field disagrees — manufactured splits

The map declares no splits, so there is nothing to diff. New Rust subcommand
`tmtraj stations` (source banked at `279218/src/stations.rs`): resample the
reference run into N equally arc-spaced stations, erect the plane perpendicular
to the reference tangent at each, interpolate every run's crossing time. That
manufactures a 200-column split table for a map that publishes none. Referenced
to the WR, over all 40 runs:

```
 stn   s_m    ref_t  mean_dt  sd_dt  min_dt  max_dt  ref_v  sd_lat
  10   10.1   1138.7     0.1    0.4    -0.4     1.1   64.6   0.19
  50   50.6   2617.0     1.2    2.5    -1.2     8.9  130.3   1.45
  90   91.1   3573.5     6.7   11.6    -2.6    52.4  180.9   0.51
 130  131.7   4319.5    25.9   57.9    -1.7   259.9  203.1   2.63
 170  172.2   5043.1    50.6  113.5    -1.8   439.7  206.5   3.45
 190  192.4   5390.8    64.2  139.0     0.0   456.2  210.9   1.75
```

**Through the launch and the descent (0 – 90 m) the entire field, rank 1 to rank
344, is within a few milliseconds and 1 m of each other.** All the dispersion is
in **the corner, 110 m to 190 m**.

Restricted to the top 15:

```
run        final   s=91  s=111  s=132  s=152  s=172  s=182  s=192
r001_5355   5355    0.0    0.0    0.0    0.0    0.0    0.0    0.0   <- WR
r005_5358   5358   -2.6   -2.3   -1.7   -1.9   -1.8    0.3    2.5
r009_5359   5359   -2.5   -1.0   -0.0   -0.6   -0.8    1.5
r014_5361   5361   -1.5   -1.9   -0.7    1.9    4.6    5.4
r003_5357   5357   -0.0    0.8    1.2    1.0    1.0    1.6
```

**Runs 5 and 9 are ~2 ms AHEAD of the world record for most of the corner and
give it all back in the last 20 metres.** The WR is not the fastest run through
the corner; it is the run with the best **exit**. Slow-in/fast-out. So:

> **the time is in the last third of the corner and the exit onto the finish
> straight (ticks ~440–536), traded against how much lock is carried through the
> middle (ticks ~355–440).**

Nothing is to be gained in the first 90 metres.

## 6. Why a TAS should be able to take it

The humans drive a **full-lock** corner: `steer = −1` held for ~165 consecutive
ticks by nearly all of them (r002, r015 and most of the field are pure keyboard,
−1 / 0). The WR is the exception — a pad run using intermediate values (+0.208,
+0.263, −0.467, −0.718, −0.945) — and it wins.

Full lock at 200 km/h scrubs speed. The car is pinned at 203.15 km/h through the
middle of the corner: that is the grip/drag equilibrium *at that steering
angle*. Less lock ⇒ less scrub ⇒ a higher plateau, as long as the car still
makes the exit. The TAS's structural edge is the **252 intermediate steering
values no human uses**, applied to a single 165-tick corner.

## 7. The oracle here is cheap — measured

400 → 12,000 identical candidates through `tmtas validate` on a 176-core box:

| jobs | n | wall | throughput |
|---|---|---|---|
| 44 | 4,000 | 4.21 s | 950/s (2570/s marginal) |
| 60 | 12,000 | 6.41 s | **1875/s (3080/s marginal)** |
| 88 | 4,000 | 5.26 s | 760/s |
| 120 | 4,000 | 7.07 s | 566/s |

Each dedicated server wants ~3–4 cores; ~44–60 concurrent servers saturates the
box. Sustained **≈2000 evals/s ⇒ 7 million evaluations per hour**, against
9.4 eval/s/process on the 22.7 s map 2. A 5.35 s run simulates in ~19 ms.

**This changes which operators are rational.** On map 2, 536 ticks × 20 steer
probes = 10,700 evals would be 19 minutes; here it is 5 seconds.

## 8. What I expect each operator to be worth, stated before measuring

| # | operator | why | expected |
|---|---|---|---|
| A | **exhaustive per-tick coordinate descent on steer**, ticks 340–540, full-tape scoring, several passes | the corner is one control channel; a full pass is ~1.6 s at this throughput. The operator the throughput unlocks, unavailable on long maps | **10–40 ms** — the largest single source |
| B | random `mix3` (doublets, compensated, level, edge-shift) windowed on 340–540 | the proven map-2 move set; finds what axis-aligned steps cannot | 3–10 ms on top of A |
| C | retiming the corner entry (`shift`) | entry timing is one integer; the field's own spread at s=91 is ±2.6 ms | 1–3 ms |
| D | exit-release polish, ticks 500–540 | exactly where r005/r009 lose 4–5 ms to the WR | 1–4 ms, overlapping A |
| E | launch/descent, ticks 0–340 | whole field within 1 m and a few ms; traction-limited | **0–2 ms**, searched last, only to prove it empty |
| F | multi-seed islands (r001, r003, r005, r009) | r005/r009 occupy a measurably different, faster-through-the-middle basin; map-2 says basins do not communicate, so run them, do not splice | a seed slot, not a ms estimate |
| G | movable-plane vernier | resolves 1 ms quantisation to ~0.17 ms | **0 ms of gain**; the measuring instrument for the final claim |
| H | **low-input human strat**, built alongside | gas/brake are constant, so the entire human-facing instruction is a steering script; quantise the TAS steer trace to as few held segments as still beats 5350 | the deliverable, not a time |

**Fork server: I expect it NOT to pay here, and will measure.** Per-eval cost is
~19 ms of physics inside ~52 ms of wall, so more than half of an evaluation is
fixed overhead the fork cannot remove. Forking at tick 340 removes at most 63 %
of the 19 ms → a ceiling around **1.2–1.3×**, against a stack whose known
failure mode (shared scratch, orphaned servers) costs whole runs. On the 22.7 s
map the same fork was worth 3.3–5.7×.

## 9. Order of work

1. Operator A (coordinate descent) as a new `tmsearch` mode — highest
   expectation, cheapest to write.
2. Concurrent A/B per `tm-loop/PROTOCOL.md`: control arm + candidate arms on the
   same box, identity control in every batch, AUC as the statistic.
3. Fork-server measurement (§8) as one arm, briefly, then move on.
4. Multi-seed islands from r001/r003/r005/r009.
5. Vernier the winner, re-validate through the plain oracle, write the route up.
6. Low-input strat in parallel with 4–5, not after.

## 10. Non-negotiables restated

No submission to any Nadeo leaderboard, ever. Every claimed improvement is
re-validated with `tmtas validate --map <ABS> <ABS ghost>` before it is adopted
or reported. A failed re-validation is a STOP: preserve the specimen in
`~/persistent/private-30d/tm-loop/phantoms/`, investigate, report. Rust only.
External APIs rate-limited, descriptive User-Agent, never a browser UA.

**Every `tmsearch` process gets an explicit distinct `--root`** (fleet-wide
phantom bug: processes sharing `/dev/shm/tmsearch` validate each other's tapes
and fabricate improvements). Using the patched tmsearch from
`279218/../145875/tmtas-rs-src-patched.tgz`, which defaults `--root` per-pid and
refuses to wipe a live root.
