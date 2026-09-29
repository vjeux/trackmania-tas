# Tiny Summer 2026 — PIPELINE: one map through, one command per stage

Everything runs on an on-demand box (`envspec=www`); nothing on the laptop.
Times are seconds with a decimal in every report; maps by name.

## 0. Box bring-up

```
persistent-storage mount --all
git clone ssh://devvm42752.vll0.facebook.com/home/vjeux/trackmania-tas repo && cd repo && git checkout agentcloud/tiny-tas
export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080     # cargo + rustup go through it on www boxes
curl -sSf https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init -o /tmp/ri && sh /tmp/ri -y --default-toolchain 1.85.0 --profile minimal
(cd tools && cargo build --release) ; (cd tools/search && cargo build --release) ; (cd tools/fk && cargo build --release)
mkdir server && (cd server && unzip -q /path/TrackmaniaServer_Latest.zip)     # from devvm42752:/tmp or files.v04.maniaplanet.com
export TM_SERVER=$PWD/server FK_SHIM=$PWD/repo/tools/search/target/release/libforkshim.so
grep -rn 'const FINISH_BASE' repo/tools/search repo/tools/fk repo/tools/tmauto   # must print nothing
(cd repo/tools/search && cargo test --release)                                   # the search suite + engine tier (oracle_e2e uses TM_SERVER)
```

Binaries: `T=repo/tools/target/release` (`tinytas`, `tmauto`, `tmmaps`, `ghost`, `mapgeom`),
`X=repo/tools/search/target/release` (`tmsearch`, `tmexplore-real`, `libforkshim.so`),
`FK=repo/tools/fk/target/release/fk`.

## A. Inputs (frozen)

`in/<Map>-Tiny.Map.Gbx` (record md5), `in/<Map>.Map.Gbx` (original), the original's
cartographer pack + route (`~/persistent/private-30d/tm-autopilot/B-cartographer/packs/<uid>.{pack,route}.json`).
The tiny map file is never written to until stage G.

## B. Oracle bring-up

```
$T/tmmaps waypoints in/TINY.Map.Gbx                       # expect: parked blocks at cell (0,0,0) + item waypoints
$T/tmauto synth probe --map in/TINY.Map.Gbx --ticks 600 --raw   # G0: a VERDICT (DNF is fine), not "Can't load map"
$T/tmauto synth write --map in/TINY.Map.Gbx --out g1/auto.Ghost.Gbx --ticks 300   # G1: prints validation_start_index (item rule)
```
G1 control when in doubt (a new map shape): sweep `--validation-u03 0..N` with
`--steer 0 --wobble-prefix 60`, then `tmexplore-real state ... --checkpoint-clock 30000 --csv DIR`
per file and read the car's position at the boundary against `tmmaps waypoints`.
G2: `$FK server check --tape g1/w03_X.Ghost.Gbx --map in/TINY.Map.Gbx --at tick:100 --n 100`
(identity resume EXACT, calibration, 100/100 fork = full).

## C. Scaled reference set (+ the transform control)

```
$T/tinytas scale-ref --pack ORIG.pack.json --route ORIG.route.json --map in/TINY.Map.Gbx --out ref/tiny
```
Writes `ref/tiny/<tinyuid>.{pack,route}.json`. Exit 1 unless every scaled gate
centre lands on a tiny waypoint item (bar 0.5 m). Anchor defaults are Summer
2026 - 01's (`--anchor 1584,16,784 --anchor-to 1584,11.5,784`); pass the map's
own (from the converter's recipe) for another map.

## D. Seed: first certified Finish

```
$X/tmexplore-real template --map in/TINY.Map.Gbx --out ex1/template.Ghost.Gbx --ticks 6000 --declare 60000 --cps <n>
$X/tmexplore-real run --pack ref/tiny/<uid>.pack.json --route ref/tiny/<uid>.route.json --map in/TINY.Map.Gbx \
    --template ex1/template.Ghost.Gbx --server $TM_SERVER --shim $X/libforkshim.so --work ex1/work \
    --threads 80 --budget 100000000 --forktick=-24 --minutes 25 > ex1/run.log
```
`--forktick=-24` puts the fork boundary at ~tick 77 (clock 30000) on this
engine build; the run prints the probed boundary (`TICK FRAME`). The run's
`best.tape.tsv` (in `--work`) carries `# frame N`. Start-position control and
identity control are in the run's own startup (`worker N start OK`).

## E/F. Route hypotheses (`--must`) and tape polish (`tmsearch`)

(filled in as wave 0 proceeds — see FINDINGS.md)

## G. Certification and the validated map

```
$T/tinytas tape assemble --template ex1/template.Ghost.Gbx --tape ex1/work/best.tape.tsv --out cert/full.tsv
$T/tmauto synth write --map in/TINY.Map.Gbx --tape cert/full.tsv --out cert/run.Ghost.Gbx --ticks <len> --declared <ms upper bound>
$T/tmauto verdict cert/run.Ghost.Gbx --map in/TINY.Map.Gbx        # plain oracle, twice, two processes -> the millisecond
$T/ghost declare cert/run.Ghost.Gbx cert/run.declared.Ghost.Gbx --from-oracle --map in/TINY.Map.Gbx
$T/tinytas authorghost embed --map in/TINY.Map.Gbx --ghost cert/run.declared.Ghost.Gbx --out out/TINY-validated.Map.Gbx
$T/tinytas authorghost extract --map out/TINY-validated.Map.Gbx --out cert/reextracted.Ghost.Gbx
$T/tmauto verdict cert/reextracted.Ghost.Gbx --map out/TINY-validated.Map.Gbx   # must equal the declared ms
$T/tmmaps header out/TINY-validated.Map.Gbx --xml                              # authortime = ms, medals, validated="1"
md5sum in/TINY.Map.Gbx out/TINY-validated.Map.Gbx                              # both into the ledger
```
Certification proper (a different box re-simulates the same bytes) is in ARM.md.
