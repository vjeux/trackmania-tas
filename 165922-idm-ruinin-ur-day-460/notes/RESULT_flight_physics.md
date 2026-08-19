# 165922 — "idm ruinin ur day #460" (AT 15643, one human record of 8 790 769 ms)

Worked 2026-08-18 19:27–21:10 PT on 28518.od.fbinfra.net. **No TAS produced.
Verdict: this IS a real target — the author time is consistent with the map's
physics and sits about 0.5 s (3 %) beyond the one human attempt.** What follows
is the evidence, the measured physics, and the reason the usual seeding route
does not work here.

**Operational note: a second agent was working this same map on this same node
in parallel** (`/tmp/m4`, 168-worker `tmex` searches, curtain maps). I coexisted
at <= 60 jobs with distinct `--root`s and my own scratch. See §8 — their search
shape (mutating the tail of the 8790-second human ghost) *cannot* produce an
AT-beating run, whoever runs it.

---

## 1. The decode said nothing — properly checked

```
tmtraj decode map.Map.Gbx   ->  FAIL: CPlugEntRecordData (0x0911F000) chunk not found
```

Header says `validated="1"`, so per ACQUISITION §9a that is necessary but not
sufficient, and a raw byte scan proves nothing. Checked the decompressed body:

```
tmex chunks --template map.Map.Gbx --csv body.bin
  class 0x03043000  body 1 707 342 bytes (decompressed), 172 skippable chunks
  raw class-id 0x0911f000: 0 occurrences      (telemetry)
  raw class-id 0x0309201d: 0 occurrences      (ghost input bitstream)
  raw class-id 0x0303f005: 0 occurrences
```

**Positive control** (required, per §9a): the same tool on 228607's map decodes
that author's validation lap — `version 10  samples 406  period 50 ms  start 0
end 20290`. So the tool works and the negative is real: **there is no embedded
author ghost on 165922. The AT's route is not readable.**

## 2. §8: the field reproduces, 1/1

The field is one ghost. It re-simulates **exactly**:

```
tmtas validate --map map.Map.Gbx ghosts/rank00001_8790769.Ghost.Gbx
  rank00001_8790769.Ghost.Gbx     8790769
```

8 790 769 declared, 8 790 769 simulated — including all 914 respawns. The
oracle is faithful on this map. (Second control: my own rebuild of the tape
through the `Replay` codec, `tmjoin IN OUT 0:879231`, also returns 8790769.)

## 3. What the map actually is

* Spawn: item#0 `GateStartLeft8m`, car starts at rest at **(421.6, 1880.5, 819.5)** — 1.88 km up.
* One `GateCheckpoint`, in the start structure.
* **132 `GateExpandableFinish` free blocks**, and their positions (chunk
  0x0304305F, records 34..165) are a flat **11 x 12 grid at y = -1**:
  x ∈ {2300, 2308, … 2380} (8 m steps), z ∈ {576, 608, … 928} (32 m steps).
  That is an **88 m x 352 m landing pad, 1.9 km downrange and 1.88 km below the spawn.**
* Block-name table also contains `GateSpecial4mBoost` and `GateSpecial{4,32}mReset` — the map has booster gates (see §5).
* So the map is: fall out of a narrow start chute, ride/bounce a short ramp
  structure (all of it inside x ∈ [409, 690]), then a **1.9 km unpowered
  ballistic glide** onto the pad. Nothing exists between x = 690 and the pad.

## 4. The 2.44-hour record is 914 retries, and its best attempt is 18.85 s

`an jumps wr.csv` (position discontinuities > 30 m) finds **914 respawns**.
Between 3 600 s and 6 600 s the car sits parked at (2294.6, -13.0, 737.7): 50
minutes AFK. The record is not a 2.44-hour run, it is 914 attempts.

The successful (last) attempt:

| event | race time | position | speed |
|---|---|---|---|
| respawn | 8 771.92 s | (421.68, 1879.62, 816.33) | 7.3 m/s |
| leaves the ramp structure | +4.88 s | (687, 1626, 845) | **182.5 m/s** |
| first ground contact | +15.08 s | (2251, ~0, 690) | 277.5 m/s (capped) |
| finish | +18.85 s | (2364, 17.8, 689) | 43 m/s |

**They landed 45 m short of the pad's near edge and spent the last 3.77 s
crawling along the ground into a gate.** That is where the map's time is.

## 5. The flight physics, measured

Fitted to the record's own 9.5 s of free flight, by integrating
`vx' = -k|v|vx , vy' = -g - k|v|vy` with a speed clamp and minimising position
error (`fly fit`):

> **g = 23.29 m/s² · quadratic drag k = 1.404e-4 /m · hard speed cap 277.55 m/s (999.2 km/h)**
> RMS position error **8.56 m over the whole 2 km glide** (0.4 %).
> Controls: same fit with k = 0 → 98 m; with g = 0 → 411 m.

Powered-flight models were tested and **rejected**: gravity + a thrust along the
car's own -up axis (the reactor signature, ~21 m/s² in this project's earlier
maps) fits *worse* than gravity + drag. The glide is unpowered. Note the 50 ms
telemetry *velocity* column carries a ±2 m/s sawtooth artefact — a 50 ms central
difference of it manufactures ±45 m/s² of phantom acceleration. Every derivative
here comes from the position column with a ±250 ms stencil.

Energy (per unit mass, `E = v²/2 + g·y`) over the winning attempt: **+10 500 J/kg
is ADDED during the first 5 s** — equivalent to 417 m of extra height — in two
discrete jumps (+2400 in 200 ms at (460, 1830); +4200 in 600 ms at (600→690,
1660→1626)). That is the booster gates. Afterwards E decays monotonically: drag.
**The boosters are the only lever on launch speed, and launch speed is the only
lever on the flight.**

## 6. Is 15 643 ms reachable? Yes — barely. This is a real target.

Ballistic solve from the human's own launch state (690, 1626), all launch
angles, landing on the pad (`fly reach`):

| launch speed | best flight time to the pad |
|---|---|
| 140 m/s | 13.49 s (the minimum speed that reaches the pad at all) |
| 182.5 m/s (**the human's**) | **10.47 s** |
| 200 m/s | 9.93 s |
| 230 m/s | 9.04 s |

And the start: the respawn state matches the *race-start* trajectory at
**t ≈ 0.78 s** (position agrees to 0.35 m), so a clean run from the start pays
only ~0.78 s to reach the state every one of the 914 attempts started from.

> **Human line, flown perfectly: 0.78 (start) + 4.88 (ramp) + 10.47 (flight) ≈ 16.1 s.**
> **AT = 15.643 s.** The gap is ~0.5 s, and one lever closes it: a launch at
> ~200 m/s instead of 182.5 buys 0.54 s of flight for nothing.

So the author time is not a joke, not a plugin artefact, and not out of reach:
it is the human's own route with a better launch — which is exactly what an
author who knows where the boosters are would drive. The `LOL` tag and the
2.44-hour record describe the *difficulty*, not the legitimacy of the AT.

## 7. Why there is no TAS yet: the tape must be ONE clean attempt, and the start is a chute

A respawn does not stop the clock, so **any tape containing the retries is
bounded below by 8 772 s**. An AT-beating run must be a single attempt from
tick 0. The obvious seed — transplant the winning attempt onto the race start —
fails:

* Built 224 rejoined tapes: `[donor 0..p] ++ [donor 877195..879295]` for
  p ∈ [50, 300] and three tail offsets (`tmjoin`, new tool).
* All 224 DNF on the real map, on a pad re-hung 96 m nearer, and on **all ten
  rungs of a ruler** (the 132 gates re-hung as 88 m tiles at y = 48, tiling
  x ∈ [1600, 2480] with full z coverage): **0 of 224 came down anywhere in that
  788 m window.**
* They die *at the start*: on a "chute" map (the gates re-hung as a net at
  y = 1800 across x ∈ [424, 512]) the human's own opening passes at **3.194 s**
  and a 400-tick cut of it at **3.298 s**, while every rejoined tape DNFs
  before it.

**The first ~3 s is a narrow chute and it is chaotic**: the respawn state and
the race-start-at-0.78 s state differ by 0.9 m/s of speed and 2.5 m/s of vy, and
that is enough. Stage 1 of any search on this map is "get down the chute", not
"fly further".

## 8. For whoever picks this up

1. **Use a short template.** `tmcut IN OUT FROM TO` (new, in `rs/tmsearch/src/bin/`)
   cuts a 2000-tick single-attempt template out of the 879 231-packet record;
   `tmjoin IN OUT A:B,C:D` concatenates ranges. Identity control passes
   (full-range rebuild → 8790769). Searching the full ghost costs 41 s per
   candidate *and cannot beat the AT*, because the retries are inside the tape.
2. **The instruments are built and controlled** (in `165922/maps/`):
   `chute.Map.Gbx` (net at y=1800 over the start chute — stage-1 score),
   `rul{1600..2392}.Map.Gbx` (10 rungs, 88 m each, at y=48 — a downrange ruler),
   `pad{1600..2296}.Map.Gbx` (the pad translated in x), `netfar.Map.Gbx`
   (the pad extended to x ∈ [2296, 2552] — the real objective with a 4x larger
   window). Controls: the human ghost finishes netfar at 8790769 (unchanged),
   netmid at 2 668 325 (an earlier attempt), rul2216 at 7 429 688, chute at 3 194.
   All built with `tmex movegrid` (positions only — no lookback-table surgery).
3. **Do not relocate free-block record 0** thinking it is the checkpoint: moving
   it breaks the human's run (the record ↔ block index mapping is only verified
   for the finish gates, records 34..165).
4. Suggested programme: (a) stage-1 search on `chute.Map.Gbx` for a population
   that gets down the chute from tick 0; (b) extend with the ruler as the score
   ("how far downrange did it come down"); (c) switch to `netfar` (real
   objective, 344 m window) and finally the real map; (d) the ballistic model in
   §5 predicts the landing point from the launch state to ~10 m, so once a
   launch state can be read, the pad can be *aimed* rather than searched for.
   `fk btraj` would give that state but its blind locate fails on this map
   (`exit 2`, "searching 36 mapped 64 KB windows").

## Files

`map.Map.Gbx`, `mapinfo.json`, `lb0.json`, `rank00001_8790769.Ghost.Gbx` (the
one human record), `wr.csv` (its decoded telemetry, 175 598 samples),
`tools/` (tmcut, tmjoin, gen, and the analysis binaries an/phys/fly/react2),
`maps/` (every instrument map above).

## 9. Stage-1 search, measured (added at the end of the session)

To test whether the chute is searchable I built 2 000 perturbations of the
rejoined tape (`gen`, edits confined to ticks 0..90 — time shifts, steer scales,
constant-steer blocks, gas/brake blocks, random offsets) and scored them on the
chute map.

* **0 of 2000 got down the chute.**
* Control against a too-narrow instrument: rebuilt the chute net **4x wider**
  (`chuteW.Map.Gbx`, 44 tiles, x ∈ [344, 696]) and re-ran 400 of them plus both
  bases. The human's own opening still passes (3 298 ms); **all 400 still DNF.**

So the failure is real and early: the tail of the human's winning attempt is
incompatible with a race start, and patching the first 0.9 s does not rescue it.
A seed for this map has to be *searched for* through the chute, not transplanted.

## 10. Cross-check with the other agent's artefact (independently re-validated)

The parallel agent banked `respawn_attempt_optimised_15085.Ghost.Gbx`. I
re-validated it through my own oracle, on the unmodified map:

```
tmtas validate --map map.Map.Gbx sib15085.Ghost.Gbx   ->  8 787 035     (declared 8 790 769)
```

3 734 ms faster than the human record — i.e. they removed the crawl and landed
the last attempt straight on the pad. Measured from the respawn at 8 771 950
that final attempt is **15 085 ms**, which is 265 ms better than the
"human line flown perfectly" figure this document derives from the ballistic
model (4.88 s ramp + 10.47 s flight = 15.35 s), so they improved the launch too.

Putting the two results together, with the 0.78 s start-to-respawn-state offset
measured in §6:

> **an equivalent clean single-attempt run is worth about 15.87 s, against an
> author time of 15.643 s. 0.22 s left to find.**

That is the strongest statement available today about whether this AT is real:
it is real, it is close, and the whole remaining problem is that the tape has to
start at tick 0 — §7 and §9 are about exactly that, and they are unsolved.
