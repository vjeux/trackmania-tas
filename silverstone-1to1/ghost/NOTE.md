# Silverstone 1:1 — the ghost lap

**127.702 s, 14/14 checkpoints, 0 rewinds**, validated by the Nadeo dedicated
server on the final map (uid `Silverstone1to14ae52765fea5`, md5
`c05993a8cae060380d61e15002fb3d0e`). Driven with `tmexplore-real drive`
(pure pursuit + a 20 m/s² speed plan capped at 55 m/s) on the fork engine;
the two ghosts come out of `make-filmable.sh` in this folder.

Ride height over the whole lap, from the engine's own trace: median 0.019 m,
max 0.103 m. (The first lap, on the first map, left the ground at 14 places,
1.09 m at Chapel Curve: 32 m terrain cells cut through the road. `circuit`
now refuses to write a map whose surface has anything above the tarmac.)

Start: the real start line (OSM `raceway=start`, 0.4 m from our centreline).
Finish: the real finish/timing line, 150 m upstream of it — so this lap is
5737 m, the way a British GP's first lap is.

---

Below: the note the driver wrote for the first lap (130.433 s on the first
map). Method and quirks still apply; times and hashes are the old ones.

# Silverstone 1:1 — the ghost lap

**130.433 s, 14/14 checkpoints, validated by the Nadeo dedicated server on the
frozen map** `Silverstone.final.Map.Gbx` (uid `Silverstone1to12b6f959f2fe7`,
md5 `5e081c677fd26911d65f6ba90cb04c97`; same physics as uid
`Silverstone1to12b6f959f2fe7` without the header change).

## Files

| file | what |
|---|---|
| `Silverstone.lap.Ghost.Gbx` | the validating ghost: input tape + declared 130.433 with 14 measured splits; 50 ms record grid (one constant sample — films as a parked car) |
| `Silverstone.lap.filmable.Ghost.Gbx` | same inputs, plus the per-50 ms `CSceneVehicleVis` samples regenerated from the engine (`ghost regen`, gate passed: V1–V10, kappa 1.000, oracle 130.433 == declared). **Film this one.** |
| `Silverstone.lap.tape.tsv` | the 10 ms input tape (`tick steer gas brake`, 12 910 ticks). `# frame 172`: search tick 0 is FILE tick 172; file ticks 0–171 hold the template's own inputs (full gas, ±12 wobble steer) — see "How it was driven" |
| `Silverstone.lap.trace.tsv` | the engine's per-tick car state for the lap (position, speed, yaw, station, lateral offset) |
| `Silverstone.lap.drive.log.tsv` | the controller's decisions (every 10 ticks: state + steer/gas/brake + speed target) |
| `template.grid.Ghost.Gbx` | the from-scratch container the lap is written into (`tmauto synth write --record grid`, 24 000 ticks, item start) |

## How it drove (5 lines)

1. **Method**: closed loop on the fork engine (`tmexplore-real drive`): every 100 ms the car's real state (position, velocity, attitude) is read out of the paused dedicated server, a pure-pursuit law steers at a point 1 s of travel ahead on the centreline (`Silverstone.Map.line.tsv`), the speed plan is `v = sqrt(20 m/s² / |curvature|)` with 20 m/s² braking, capped at 55 m/s; steer command = wanted curvature / full-lock curvature, with the full-lock curvature measured on this map (`12.5·v^-1.6`, linear in the steer input). Rewind-on-failure was armed and never fired on the final map (0 rewinds).
2. **Speed**: 199–202 km/h (55–56 m/s) on Hamilton, Wellington, Hangar, Copse, Chapel; Abbey 168 km/h; Village 120; Brooklands/Luffield 115–126; Becketts 126–160; Vale 105 (slowest corner); average 45 m/s (161 km/h) over 5 862 m.
3. **Off-track moments**: none. Max lateral offset from the centreline 4.9 m (briefly, Becketts complex), always inside the tarmac edges; the off-track detector (−0.5 m inside the painted edge) never tripped. No respawn, no wall contact.
4. **Where it is slow**: the 55 m/s cap on every straight (the car reaches 78 m/s flat out) and the conservative 20 m/s² cornering allowance (the car holds 35–60 m/s² before sliding) — roughly 25–35 s is available by raising `--v-max` and `--a-lat`; the first 1.72 s are the template's straight-line full throttle (fixed below the fork boundary).
5. **Splits (geometric, ±0.1 s)**: 10.85 20.84 29.61 37.31 45.41 56.00 63.80 70.51 79.09 89.72 96.30 104.37 112.36 121.76 → finish 130.433 (validator).

## Validation output (verbatim)

```
$ tmsearch validate --map Silverstone.final.Map.Gbx Silverstone.lap.Ghost.Gbx Silverstone.lap.filmable.Ghost.Gbx
Silverstone.lap.filmable.Ghost.Gbx              130.433
Silverstone.lap.Ghost.Gbx                       130.433

$ tmexplore-real write --template template.grid.Ghost.Gbx --map Silverstone.Map.Gbx --tape Silverstone.lap.tape.tsv --out ... --confirm
plain oracle: Finish { ms: 130433 } desc "validated time is actually better! (240000 > 130433)" map Silverstone1to12b6f959f2fe7

$ ghost verify Silverstone.lap.filmable.Ghost.Gbx --map Silverstone.final.Map.Gbx
PASS V1   codec identity: a verbatim re-encode of all 24000 ticks reproduces the file's own bitstream
PASS V2   declared-time census: 1 copies (1 in the body, 0 in the header), all 130.433
PASS V5   telemetry: 2609 samples, 0.000 .. 130.400, span 0.000 .. 130.433
PASS V6   tape/record agreement: kappa 1.000 (100.0% of 2609 samples exact, best lag 0 ms)
PASS V7   oracle re-simulated the written file: 130.433 == the declared time
OK
```

## Quirks worth knowing

* **The plain oracle prints the bare `wrong simu` (read as 0 checkpoints) for a partial tape that ends before CP6, and `reached some checkpoints (N out of 15)` from CP6 on.** The checkpoint items credit fine (the finish proves it); the report for short partial runs is blind. Not chased further.
* **The fork boundary moves between server launches (172 or 173 on this box).** A tape is only meaningful at the boundary it was driven at (`# frame` in the TSV); re-validating it one tick off gives a different run (0 cps). `tmexplore-real drive --want-boundary N` relaunches until the server stops at or before N; `--start-from` pads the gap with the container's own inputs.
* `ghost regen`'s closing report says "94 channels not written without --carrier" — that message keys on the USER's flags, but `--carrier layout` is unconditional inside the pipeline; the samples do carry varying gear/rpm/dampers/contact (checked with `tmtraj export`). The decoder reads gear 1..3 over the lap, which looks low for 200 km/h — a decoder/field question, not the lap's.

## Tools (branch `silverstone-ghost`)

* `tmexplore-real drive|trace|write` (new `tools/search/tmexplore-engine/src/drive.rs`): the closed-loop driver, the per-tick tracer, and the tape → container writer.
* `tmexplore-real template --u03 N`: validation start index override (item starts).
* `ForkBranch::advance` returns exactly the states of the macro's ticks (it returned the last k of a stream that runs 4 ticks past the tape, or 56 ticks short of it under a tight `lroundf` budget — a controller read a stale or future car).
* `tmauto synth write`: item-start maps (Spawn item → `validation_start_index` = its index among item waypoints; initial transform from the item).
* `ghost declare --splits`: writes the intermediates + the finish and checks the same thing (its three read-back controls disagreed with each other; no input could pass).
* `tmsearch`: f32/f64 fixes so `tools/search` builds against the current `gbx::record`.
8c8d962d8123e16d3820928000873cdd  Silverstone.lap.Ghost.Gbx
26198529dda43e0ac5e6b8985d40d156  Silverstone.lap.filmable.Ghost.Gbx
5fea40f7d58afaf36a3a20eb1d4c167b  Silverstone.lap.tape.tsv
