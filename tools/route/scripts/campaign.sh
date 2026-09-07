#!/bin/bash
# GEOM arm batch: gates → human orders → planner (cost + speed [+ flight]) → tables, for every map that has a
# .Map.Gbx in the cartographer bank or the player corpus. Re-runnable; everything lands in the tm-route bank.
#   scripts/campaign.sh [gates|human|plan|tables|all]
set -u
source /tmp/tmp/env.sh
R=/tmp/tmp/repo/tools/route/target/release
BANK=$HOME/persistent/private-30d/tm-route
B=$HOME/persistent/private-30d/tm-autopilot/B-cartographer
V=$HOME/persistent/private-30d/tm-player/data/v0/maps
G=$BANK/geom; RT=$BANK/routes; P=$BANK/plan
step=${1:-all}

# map uid → map file (bank first, then the crawl)
mapfile_of() { local u=$1; if [ -f $B/bank/maps/$u.Map.Gbx ]; then echo $B/bank/maps/$u.Map.Gbx; elif [ -f $V/$u/map.Map.Gbx ]; then echo $V/$u/map.Map.Gbx; fi; }
uids() { { ls $B/bank/maps/*.Map.Gbx 2>/dev/null | xargs -n1 basename | sed 's/.Map.Gbx//'; ls $V 2>/dev/null; } | sort -u; }

if [ $step = gates ] || [ $step = all ]; then
  for u in $(uids); do f=$(mapfile_of $u); [ -n "$f" ] || continue; mkdir -p $G/$u
    pk=$B/packs/$u.pack.json
    if [ -f $pk ]; then $R/tmroute gates $f --out $G/$u/gates.json --orient-from $pk ${pk%.pack.json}.route.json > $G/$u/gates.txt 2>&1
    else $R/tmroute gates $f --out $G/$u/gates.json > $G/$u/gates.txt 2>&1; fi
    head -1 $G/$u/gates.txt | cut -f1,3,4,5,6
  done
fi
if [ $step = human ] || [ $step = all ]; then
  $R/tmroute human-batch --data $V --geom $G --routes $RT --unverified
  # Summer 2026 - 01 also has the 44 tm-pop ghosts: use every verified + tm-pop ghost for its route
  u=buNzfsVlp2NF2oWtHM3729dEylg
  $R/tmroute consensus --gates $G/$u/gates.json --orders $G/$u/human-orders.tsv --out $RT/$u/route-router-human-0.json --gates-out $G/$u/gates.json \
    $HOME/persistent/private-30d/tm-pop/*.Ghost.Gbx $(for j in $V/$u/ghosts/*.json; do grep -q '"verdict": "exact"' $j && echo ${j%.json}.Ghost.Gbx; done) > $G/$u/consensus.txt 2>&1
  head -1 $G/$u/consensus.txt
fi
if [ $step = plan ] || [ $step = all ]; then
  mkdir -p $P/plan-cost $P/plan-geometric $P/human-legs
  for u in $(uids); do f=$(mapfile_of $u); [ -n "$f" ] || continue; [ -f $G/$u/gates.json ] || continue
    ho=$G/$u/human-orders.tsv; [ -f $ho ] || ho=$G/$u/human-orders.unverified.tsv; hoarg=""; [ -f $ho ] && hoarg="--human-orders $ho --legs-out $P/human-legs/$u.tsv"
    for grid in track deco; do
      nice $R/tmplan plan $f --gates $G/$u/gates.json --out-dir $RT --top-k 3 --matrix --quiet --time cost --exact --grid $grid --source router-plan-cost $hoarg > $P/plan-cost/$u.txt 2>&1
      grep -q "NO PLAN" $P/plan-cost/$u.txt || break
    done
    grid=$(grep -q "DECORATION" $P/plan-cost/$u.txt && echo deco || echo track)
    nice $R/tmplan plan $f --gates $G/$u/gates.json --out-dir $RT --top-k 3 --quiet --grid $grid --source router-plan > $P/plan-geometric/$u.txt 2>&1
    if grep -q "NO PLAN" $P/plan-cost/$u.txt; then
      nice $R/tmplan plan $f --gates $G/$u/gates.json --out-dir $RT --top-k 3 --quiet --flight ballistic --source router-plan-flight > $P/plan-geometric/$u.flight.txt 2>&1
    fi
    grep -E "cp_groups|rank 0|NO PLAN" $P/plan-cost/$u.txt | head -2 | cut -c1-160
  done
  cat $P/human-legs/*.tsv | grep -v "^map" | sort > $P/human-legs/ALL.tsv
fi
if [ $step = tables ] || [ $step = all ]; then
  $R/tmroute index $RT --names $G
  $R/tmroute table $RT --geom $G --plan-source router-plan-cost > $P/table-cost.md
  $R/tmroute table $RT --geom $G --plan-source router-plan > $P/table-speed.md
  tail -1 $P/table-cost.md; tail -1 $P/table-speed.md
  echo "human legs the surface graph cannot explain: $(grep -c MISSING $P/human-legs/ALL.tsv) of $(wc -l < $P/human-legs/ALL.tsv)"
fi
