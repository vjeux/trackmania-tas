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
    # the GEN arm's engine-credited normal signs (gen/g2/flipped-normals.tsv) win over the cartographer tour
    EF=$BANK/gen/g2/flipped-normals.tsv; efarg=""; [ -f $EF ] && efarg="--engine-flips $EF"
    if [ -f $pk ]; then $R/tmroute gates $f --out $G/$u/gates.json --orient-from $pk ${pk%.pack.json}.route.json $efarg > $G/$u/gates.txt 2>&1
    else $R/tmroute gates $f --out $G/$u/gates.json $efarg > $G/$u/gates.txt 2>&1; fi
    head -1 $G/$u/gates.txt | cut -f1,3,4,5,6
  done
fi
if [ $step = human ] || [ $step = all ]; then
  $R/tmroute human-batch --data $V --geom $G --routes $RT --unverified
  # Summer 2026 - 01's 44 tm-pop ghosts (not oracle-verified here) → a control consensus only
  u=buNzfsVlp2NF2oWtHM3729dEylg
  $R/tmroute consensus --gates $G/$u/gates.json --orders $G/$u/human-orders.tm-pop.tsv --out /tmp/tm-pop-route.json $HOME/persistent/private-30d/tm-pop/*.Ghost.Gbx > $G/$u/consensus.tm-pop.txt 2>&1
  head -1 $G/$u/consensus.tm-pop.txt
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
if [ $step = classify ] || [ $step = all ]; then
  # human routes: name each leg's connection from the surface graph (Road, or Jump/Drop = a connection the graph lacks)
  for u in $(uids); do f=$(mapfile_of $u); [ -n "$f" ] || continue; r=$RT/$u/route-router-human-0.json; [ -f $r ] || continue
    grid=$(grep -q "DECORATION" $P/plan-cost/$u.txt 2>/dev/null && echo deco || echo track)
    nice $R/tmplan classify $r $f --gates $G/$u/gates.json --grid $grid --quiet 2>&1 | tail -1
  done
fi
# plan-r: the MODEL arm's R plugged into the EdgeEstimator seam (`tmr plan`, tools/route/tmr on agentcloud/route-model;
# binary TMR, models model/watch/r-latest.tmw + rl-latest.tmw). Every map with a human order + the hypothesis maps.
HYP="3wllzzuIOaf7WnPga5vlnGvu8q7 E3iCwrFqXmr4TbDmkJgQGmNnxhb izk69D9FxOr6GzMU534ykPCeepb sAhLhKUwHH9V95xk3mlm9hAxun4 OPozt3ejdJCjNsvRuhuW2BmBbTe VsTQVXqqByO_qBi1GZtUraFtj35 CEeRuRAw6E4hdRfnZEwn1XMe235"
TMR=${TMR:-/tmp/tmp/model-target/release/tmr}
# floors OFF so a ranking always exists (with the 05:12Z r-latest every leg is below the 0.02/0.05 defaults on
# Summer 2026 - 01; with them off rank 0 is the human order there). Agreed with the MODEL arm: see plan/plan-r/MODELS.txt.
TMR_FLAGS=${TMR_FLAGS:---p-floor 0 --p-step 0.02}
# MODEL arm (13:09Z): gate prior = the geo-dropout variant r-latest-gd.tmw, chain = rl-latest.tmw; p-floor 0 (the gate
# head prices nothing under chained), p-step 0.02 bounds the fan. Both pointers move every ~40 min.
MODEL_R=${MODEL_R:-$BANK/model/watch/r-latest-gd.tmw}; MODEL_RL=${MODEL_RL:-$BANK/model/watch/rl-latest.tmw}
if [ $step = plan-r ]; then
  mkdir -p $P/plan-r
  [ -x $TMR ] || { echo "no tmr binary at $TMR"; exit 1; }
  # which r-v<N> the latest pointer is (by md5), and the maps it trained on (its .md)
  W=$(dirname $MODEL_R); RV=""; for f in $W/r-v*.tmw; do [ "$(md5sum < $f)" = "$(md5sum < $MODEL_R)" ] && RV=$(basename $f .tmw); done
  RLV=""; for f in $W/rl-v*.tmw; do [ "$(md5sum < $f)" = "$(md5sum < $MODEL_RL)" ] && RLV=$(basename $f .tmw); done
  TRAIN=$( [ -n "$RV" ] && grep -E "^  train " $W/$RV.md 2>/dev/null | awk '{print $2}' | tr '\n' ',' )
  HELD=$( [ -n "$RV" ] && grep -E "^  HELD-OUT " $W/$RV.md 2>/dev/null | awk '{print $2}' | tr '\n' ',' )
  echo "gate prior $RV ($(md5sum < $MODEL_R | cut -c1-8)) chain $RLV ($(md5sum < $MODEL_RL | cut -c1-8)) flags $TMR_FLAGS run $(date -u +%Y-%m-%dT%H:%MZ); trained on: $TRAIN held-out: $HELD" | tee $P/plan-r/MODELS.txt
  { for d in $G/*/; do u=$(basename $d); [ -f $d/consensus.txt ] || [ -f $d/consensus.unverified.txt ] || continue; grep -q "modal_groups \[[0-9]" $d/consensus*.txt 2>/dev/null && echo $u; done; for u in $HYP; do echo $u; done; } | sort -u > /tmp/plan-r.uids
  echo "plan-r over $(wc -l < /tmp/plan-r.uids) maps"
  plan_r_one() { u=$1; f=$(mapfile_of $u); [ -n "$f" ] || return 0; [ -f $G/$u/gates.json ] || return 0
    nice $TMR plan $f --gates $G/$u/gates.json --model $MODEL_R --local $MODEL_RL --estimator chained $TMR_FLAGS --top-k 3 --quiet --out-dir $RT --source router-plan-r > $P/plan-r/$u.txt 2>&1
    grep -E "cp_groups|rank 0|NO PLAN" $P/plan-r/$u.txt | head -2 | cut -c1-160; }
  export -f plan_r_one mapfile_of; export TMR TMR_FLAGS MODEL_R MODEL_RL G RT P B V
  cat /tmp/plan-r.uids | xargs -P ${PAR:-6} -n 1 bash -c 'plan_r_one "$0"'
  $R/tmroute index $RT --names $G
  $R/tmroute table-r $RT --geom $G --geo router-plan-cost --r router-plan-r --also $(echo $HYP | tr " " ",") --train "$TRAIN" --held-out "$HELD" --title "$(cat $P/plan-r/MODELS.txt)" > $P/table-r.md
  tail -1 $P/table-r.md
fi
if [ $step = tables ] || [ $step = all ]; then
  $R/tmroute index $RT --names $G
  $R/tmroute table $RT --geom $G --plan-source router-plan-cost > $P/table-cost.md
  $R/tmroute table $RT --geom $G --plan-source router-plan > $P/table-speed.md
  [ -f $RT/routes.tsv ] && grep -q router-plan-r $RT/routes.tsv && $R/tmroute table-r $RT --geom $G --geo router-plan-cost --r router-plan-r --also $(echo $HYP | tr " " ",") > $P/table-r.md
  tail -1 $P/table-cost.md; tail -1 $P/table-speed.md
  echo "human legs the surface graph cannot explain: $(grep -c MISSING $P/human-legs/ALL.tsv) of $(wc -l < $P/human-legs/ALL.tsv)"
fi
