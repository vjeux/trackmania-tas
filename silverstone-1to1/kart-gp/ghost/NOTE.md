# Kart Silverstone GP — the ghost lap

**72.577 s, 8/8 checkpoints, 0 rewinds**, validated by the Nadeo dedicated
server on the final map (uid `SilverstoneKartae6c4212bbad`, md5
`8faf45038bd0ef2a8dd165d597289239`), driven in the **SnowCar** (the map's
player model). Driven with `tmexplore-real drive` on the fork engine; the two
ghosts come out of `make-filmable.sh` in this folder.

## Files

| file | what |
|---|---|
| `KartSilverstone.lap.Ghost.Gbx` | the validating ghost: input tape + declared 72.577 (50 ms record grid, one constant sample — films as a parked car) |
| `KartSilverstone.lap.filmable.Ghost.Gbx` | same inputs, plus the per-50 ms car telemetry regenerated from the engine (`ghost regen --carrier layout`, gate passed: kappa 1.000, oracle 72.577 == declared), car model set to `CarSnow`. **Film this one.** |
| `KartSilverstone.lap.tape.tsv` | the 10 ms input tape (`tick steer gas brake`, 7 110 ticks). `# frame 173`: search tick 0 is FILE tick 173; file ticks 0–172 hold the template's own inputs (full gas, ±12 wobble steer) |
| `KartSilverstone.lap.drive.log.tsv` | the controller's decisions every 5 ticks (car state + steer/gas/brake + speed target) |
| `KartSilverstone.lap.drive.out.txt` | the driver's transcript (checkpoint confirmations, the finish) |
| `template.grid.Ghost.Gbx` | the from-scratch container the lap is written into: `tmauto synth write --map MAP --out T --ticks 24000 --record grid --cps 8 --car CarSnow --wobble-prefix 200` |
| `make-filmable.sh` | tape → the two ghosts (Rust tools in the order they go) |

## How it drove

1. **Method**: closed loop on the fork engine (`tmexplore-real drive`): every
   50 ms the car's real state is read out of the paused dedicated server, a
   pure-pursuit law steers at a point 0.6 s of travel ahead (5–30 m) on the
   centreline (`../KartSilverstone.lap.line.tsv`), the speed plan is
   `v = sqrt(11.5 m/s² / |curvature|)` with 10 m/s² braking, capped at 40 m/s,
   coasting inside a 0.3 / 1.2 m/s band around the target. Rewind-on-failure
   was armed and never fired (0 rewinds).
   ```
   tmexplore-real drive --line KartSilverstone.lap.line.tsv --map KartSilverstone.Map.Gbx \
     --template template.grid.Ghost.Gbx --server $TM_SERVER --shim libforkshim.so \
     --v-max 40 --a-lat 11.5 --a-brake 10 --k 5 --band-hi 1.2 --band-lo 0.3 \
     --look-t 0.6 --look-min 5 --look-max 30 --off-margin 0.5 \
     --shrink-back 60 --shrink-fwd 10 --allow-floor 0.5
   ```
2. **Speed**: 129 km/h (36 m/s) at the end of the main straight, 117 km/h
   through the plaza; the double hairpin and the cyan hairpin at 34–35 km/h
   (9.5 m/s), Priory 42 km/h, the loop 49 km/h; mean 19.9 m/s (72 km/h) over
   1 414 m (spawn on the grid, 30 m before the timing line, to the line).
3. **Off-track moments**: none. Closest approach to a painted edge 0.22 m
   inside it (cyan hairpin, 15.7 m/s); never on a kerb.
4. **Where it is slow**: the 11.5 m/s² cornering allowance and the 40 m/s cap
   are conservative for the SnowCar (it holds more before sliding; an 80.750
   lap with 9 m/s² came first). Roughly 8–12 s is available.
5. **Splits (geometric, ±0.2 s)**: CP1 4.2 · CP2 13.2 · CP3 25.6 · CP4 27.7 ·
   CP5 37.7 · CP6 43.7 · CP7 51.6 · CP8 68.3 → finish 72.577 (validator).

## The SnowCar

The map names the car (`<playermodel id="CarSnow"/>` in the header, chunk
`0x0304300D` = `("CarSnow", 10003, "Nadeo")` in the body) and the dedicated
server simulates that car: the same full-gas start on the F1 map reaches
147 km/h in the SnowCar where the stadium car reaches 185. The ghost says
`CarSnow` too (`tmauto synth write --car`, `ghost identity set --model`),
otherwise a borrowed human container films as a stadium car.

## What the driver needed for this map (fixed in `tmexplore-real drive`)

- the finish line is 32 m AFTER the spawn along the lap (grid start), so the
  lap distance is unwrapped past the start and the finish is one lap on;
- the speed allowance is indexed by distance from the spawn, as the planner
  reads it (it was indexed by station — invisible on the F1 map, whose spawn is
  station 5885 of 5887);
- past the driven tape the oracle echo BRAKES instead of inheriting the
  template's full gas (200 s of full throttle took the car off the map and the
  server credited zero checkpoints);
- a car crawling because the plan asked for a crawl is not "stalled";
- the template needs `--wobble-prefix`: with a flat steer-0 tape the fork
  shim cannot locate the input array and the server runs to the end.

## Validation output (verbatim)

```
$ tmsearch validate --map KartSilverstone.Map.Gbx KartSilverstone.lap.Ghost.Gbx KartSilverstone.lap.filmable.Ghost.Gbx
KartSilverstone.lap.filmable.Ghost.Gbx           72.577
KartSilverstone.lap.Ghost.Gbx                    72.577

$ ghost verify KartSilverstone.lap.filmable.Ghost.Gbx --map KartSilverstone.Map.Gbx
PASS V6   tape/record agreement: kappa 1.000 (100.0% of 1452 samples exact, best lag 0 ms)
PASS V7   oracle re-simulated the written file: 72.577 == the declared time
WARN V11  no live non-vehicle record (the same shape as the F1 filmable ghost, which films)
```
