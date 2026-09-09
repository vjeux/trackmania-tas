#!/bin/bash
# vjeux's client runs (ghost exports recorded ON the tiny map, ship15 frame) → <stem>.vjeux-line.json (loop-free, 100 ms) +
# a centreline/route file on HIS line (<stem>.road-centreline.vjeux.json, <stem>.route-router-road-centreline-vjeux-0.json)
source /tmp/tmp/env.sh; cd /tmp/tmp/repo/tools/route
BUILD=${1:-ship15-53383427}; M=/tmp/tmp/bank-mirror; B=$M/tm-route/tiny-builds/$BUILD; T=/tmp/${BUILD%%-*}
IN=$HOME/persistent/private-30d/tm-player/tiny/incoming/vjeux-runs
V=$M/tm-route/tiny/gap-verdicts-ship7-3adf1f85.tsv; X=$M/tm-route/tiny/road-exclusions.tsv; LL=$M/tm-route/tiny/leg-lines.tsv
for g in "$IN"/*.Gbx; do
  [ -f "$g" ] || continue
  # which map: the ghost's map uid → our stem (tiny uid in the deck.json)
  uid=""
  b=""
  if [ -z "$b" ]; then nn=$(basename "$g" | grep -oE "(^|[^0-9])(0[1-9]|1[0-9]|2[0-5])([^0-9]|$)" | grep -oE "[0-9]{2}" | head -1); b=$(ls $B/centreline/ | grep "^$nn-" | grep "\.road-centreline.json$" | sed 's/.road-centreline.json//'); fi
  [ -n "$b" ] || { echo "SKIP $(basename "$g"): map not identified"; continue; }
  echo "== $(basename "$g") → $b"; /tmp/tmp/playerwt/tools/target/release/ghost inspect "$g" 2>&1 | grep -iE "input|tape|control|samples|map" | head -4 | cut -c1-140
  target/release/tmplan author-line "$g" --anchor 0,0,0:0,0,0 --scale 1 --centreline $B/centreline/$b.road-centreline.json --gates $B/geom/$b.deck.json --out $B/centreline/$b.vjeux-line.json --row "$b (vjeux)" 2>&1 | grep -E "author order|respawn|^\|" | cut -c1-200
  order=$(target/release/tmplan author-line "$g" --anchor 0,0,0:0,0,0 --scale 1 --centreline $B/centreline/$b.road-centreline.json --gates $B/geom/$b.deck.json 2>/dev/null | grep "author order" | sed 's/.*: //')
  nn=${b:0:2}; sp=$(awk -F'\t' -v n="$nn" '$1==n {print $2","$3","$4}' $M/tm-player/tiny/gate-crossings/ENGINE-SPAWNS-*.tsv 2>/dev/null | head -1); SPAWN=""; [ -n "$sp" ] && SPAWN="--spawn $sp"
  [ -n "$order" ] && target/release/tmplan road-centreline "$T/Tiny Summer 2026 - $nn.Map.Gbx" --gates $B/geom/$b.deck.json --order $order --out $B/centreline/$b.road-centreline.vjeux.json --route-out $B/centreline/$b.route-router-road-centreline-vjeux-0.json --verdicts $V --exclusions $X --leg-lines $LL --map-stem $b $SPAWN --author-line $B/centreline/$b.vjeux-line.json --note "POLYLINE = vjeux's own client run on $BUILD ($(basename "$g")); order = his crossings" 2>&1 | grep -E "author line|pts," | cut -c1-140
done
