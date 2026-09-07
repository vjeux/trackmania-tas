#!/bin/bash
# GEOM arm batch: gates → human orders → planner (cost + speed [+ flight]) → tables, for every map that has a
# .Map.Gbx in the cartographer bank or the player corpus. Re-runnable; everything lands in the tm-route bank.
#   scripts/campaign.sh [gates|human|plan|tables|all]
set -u
source /tmp/tmp/env.sh
R=/tmp/tmp/repo/tools/route/target/release
# THE BANK IS NEVER WALKED BY A BATCH (2026-09-07: the FUSE mount stalled a 166-core box to load 6 000 and wedged it).
# Every step reads and writes a LOCAL MIRROR (/tmp/tmp/bank-mirror, `rsync -a` in once, out once per step); the real
# bank paths are only rsync endpoints. Override MIRROR= to point elsewhere; MIRROR=none reads the bank directly.
MIRROR=${MIRROR:-/tmp/tmp/bank-mirror}
if [ "$MIRROR" = none ]; then ROOT=$HOME/persistent/private-30d; else ROOT=$MIRROR; fi
BANK=$ROOT/tm-route
B=$ROOT/tm-autopilot/B-cartographer
V=$ROOT/tm-player/data/v0/maps
REAL=$HOME/persistent/private-30d
# pull DIR... (relative to the bank root) into the mirror; push the tm-route results back. Sequential, one pass each.
pull() { [ "$MIRROR" = none ] && return 0; for d in "$@"; do mkdir -p $ROOT/$(dirname $d); rsync -a --info=stats1 $REAL/$d/ $ROOT/$d/ 2>&1 | grep -E "^Number of created|^Total transferred" | tr '\n' ' '; echo " ← $d"; done; }
push() { [ "$MIRROR" = none ] && return 0; for d in "$@"; do rsync -a --info=stats1 $ROOT/$d/ $REAL/$d/ 2>&1 | grep -E "^Number of created|^Total transferred" | tr '\n' ' '; echo " → $d"; done; }
# the player corpus: maps only (ghosts are GBs — pulled only by the human step)
pull_maps() { [ "$MIRROR" = none ] && return 0; mkdir -p $V; rsync -a --info=stats1 --include='*/' --include='map.Map.Gbx' --exclude='*' $REAL/tm-player/data/v0/maps/ $V/ 2>&1 | grep -E "^Number of created" | tr '\n' ' '; echo " ← player maps"; pull tm-autopilot/B-cartographer/bank/maps tm-autopilot/B-cartographer/packs; }
pull_ghosts() { [ "$MIRROR" = none ] && return 0; rsync -a --info=stats1 $REAL/tm-player/data/v0/maps/ $V/ 2>&1 | grep -E "^Number of created|^Total transferred" | tr '\n' ' '; echo " ← player maps + ghosts"; }
# NO_PULL=1 / NO_PUSH=1 skip the syncs (the mirror is known fresh, or a dry run)
if [ -n "${NO_PULL:-}" ]; then pull() { :; }; pull_maps() { :; }; pull_ghosts() { :; }; fi
if [ -n "${NO_PUSH:-}" ]; then push() { :; }; fi
G=$BANK/geom; RT=$BANK/routes; P=$BANK/plan
step=${1:-all}

# map uid → map file (bank first, then the crawl)
mapfile_of() { local u=$1; if [ -f $B/bank/maps/$u.Map.Gbx ]; then echo $B/bank/maps/$u.Map.Gbx; elif [ -f $V/$u/map.Map.Gbx ]; then echo $V/$u/map.Map.Gbx; fi; }
uids() { { ls $B/bank/maps/*.Map.Gbx 2>/dev/null | xargs -n1 basename | sed 's/.Map.Gbx//'; ls $V 2>/dev/null; } | sort -u; }

if [ $step = gates ] || [ $step = all ]; then
  pull_maps; pull tm-route/geom tm-route/gen/g2
  for u in $(uids); do f=$(mapfile_of $u); [ -n "$f" ] || continue; mkdir -p $G/$u
    pk=$B/packs/$u.pack.json
    # the GEN arm's engine-credited normal signs (gen/g2/flipped-normals.tsv) win over the cartographer tour
    EF=$BANK/gen/g2/flipped-normals.tsv; efarg=""; [ -f $EF ] && efarg="--engine-flips $EF"
    if [ -f $pk ]; then $R/tmroute gates $f --out $G/$u/gates.json --orient-from $pk ${pk%.pack.json}.route.json $efarg > $G/$u/gates.txt 2>&1
    else $R/tmroute gates $f --out $G/$u/gates.json $efarg > $G/$u/gates.txt 2>&1; fi
    head -1 $G/$u/gates.txt | cut -f1,3,4,5,6
  done
  push tm-route/geom
fi
if [ $step = human ] || [ $step = all ]; then
  pull_ghosts; pull tm-route/geom tm-route/routes
  $R/tmroute human-batch --data $V --geom $G --routes $RT --unverified
  # Summer 2026 - 01's 44 tm-pop ghosts (not oracle-verified here) → a control consensus only
  u=buNzfsVlp2NF2oWtHM3729dEylg
  $R/tmroute consensus --gates $G/$u/gates.json --orders $G/$u/human-orders.tm-pop.tsv --out /tmp/tm-pop-route.json $HOME/persistent/private-30d/tm-pop/*.Ghost.Gbx > $G/$u/consensus.tm-pop.txt 2>&1
  head -1 $G/$u/consensus.tm-pop.txt
  push tm-route/geom tm-route/routes
fi
if [ $step = plan ] || [ $step = all ]; then
  pull_maps; pull tm-route/geom tm-route/routes tm-route/plan
  mkdir -p $P/plan-cost $P/plan-geometric $P/human-legs
  # one map (cost + speed [+ flight]); run PAR_PLAN-wide (single-threaded planner, ~20 s per map locally)
  plan_one() { u=$1; f=$(mapfile_of $u); [ -n "$f" ] || return 0; [ -f $G/$u/gates.json ] || return 0
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
  }
  # incremental: a map with plan-cost output is skipped unless FORCE_PLAN=1 (or its gates.json is newer)
  for u in $(uids); do [ -f $G/$u/gates.json ] || continue
    if [ -z "${FORCE_PLAN:-}" ] && [ -f $P/plan-cost/$u.txt ] && [ ! $G/$u/gates.json -nt $P/plan-cost/$u.txt ]; then continue; fi
    echo $u; done > /tmp/plan.uids
  echo "plan over $(wc -l < /tmp/plan.uids) maps"
  export -f plan_one mapfile_of; export R G RT P B V
  cat /tmp/plan.uids | xargs -P ${PAR_PLAN:-8} -n 1 bash -c 'plan_one "$0"'
  cat $P/human-legs/*.tsv | grep -v "^map" | sort > $P/human-legs/ALL.tsv
  push tm-route/routes tm-route/plan
fi
if [ $step = classify ] || [ $step = all ]; then
  pull_maps; pull tm-route/geom tm-route/routes
  # human routes: name each leg's connection from the surface graph (Road, or Jump/Drop = a connection the graph lacks)
  for u in $(uids); do f=$(mapfile_of $u); [ -n "$f" ] || continue; r=$RT/$u/route-router-human-0.json; [ -f $r ] || continue
    grid=$(grep -q "DECORATION" $P/plan-cost/$u.txt 2>/dev/null && echo deco || echo track)
    nice $R/tmplan classify $r $f --gates $G/$u/gates.json --grid $grid --quiet 2>&1 | tail -1
  done
  push tm-route/routes
fi
# plan-r: the MODEL arm's R plugged into the EdgeEstimator seam (`tmr plan`, tools/route/tmr on agentcloud/route-model;
# binary TMR, models model/watch/r-latest.tmw + rl-latest.tmw). Every map with a human order + the hypothesis maps.
# Summer 2026 - 04, 07, 09, 14, Norway 2026, Saudi Arabia 2026, Poland 2026
HYP="3wllzzuIOaf7WnPga5vlnGvu8q7 E3iCwrFqXmr4TbDmkJgQGmNnxhb meaXfi6lw0X01rzXmkGnbg9hdXg v_2z3U5J1xzvZTnDmnXDvvweon3 OPozt3ejdJCjNsvRuhuW2BmBbTe VsTQVXqqByO_qBi1GZtUraFtj35 CEeRuRAw6E4hdRfnZEwn1XMe235"
TMR=${TMR:-$R/tmr}
# floors OFF so a ranking always exists (with the 05:12Z r-latest every leg is below the 0.02/0.05 defaults on
# Summer 2026 - 01; with them off rank 0 is the human order there). Agreed with the MODEL arm: see plan/plan-r/MODELS.txt.
TMR_FLAGS=${TMR_FLAGS:---p-floor 0 --p-step 0.02}
# MODEL arm (13:09Z): gate prior = the geo-dropout variant r-latest-gd.tmw, chain = rl-latest.tmw; p-floor 0 (the gate
# head prices nothing under chained), p-step 0.02 bounds the fan. Both pointers move every ~40 min.
MODEL_R=${MODEL_R:-$BANK/model/watch/r-latest-gd.tmw}; MODEL_RL=${MODEL_RL:-$BANK/model/watch/rl-latest.tmw}
if [ $step = plan-r ]; then
  pull_maps; pull tm-route/geom tm-route/model/watch tm-route/plan/plan-r
  mkdir -p $P/plan-r
  [ -x $TMR ] || { echo "no tmr binary at $TMR"; exit 1; }
  # the watcher rewrites the model pointers every ~40 min and the bank mount can serve a half-written or stale file
  # ("trailer says 1657 bytes of meta, 1059 remain" on 7 of 30 maps, 13:11Z run): remount, then SNAPSHOT both models
  # to /tmp so one table is one model
  # (no remount here: a remount while other pipelines hold files left the mount "Transport endpoint is not connected", 15:22Z)
  SNAP=/tmp/tmr-models-$(date -u +%Y%m%dT%H%MZ); mkdir -p $SNAP
  # copy, then prove the copy LOADS (a stale FUSE read gave a 700776-byte rl.tmw whose trailer said otherwise, 14:54Z)
  snap_model() { local src=$1 dst=$2 i; for i in 1 2 3 4 5; do cp $src $dst && $TMR selftest --model $dst > $dst.selftest.txt 2>&1 && return 0; echo "  snapshot of $src did not load (try $i): $(tail -1 $dst.selftest.txt | cut -c1-100)"; sleep 20; done; return 1; }
  # copy the VERSIONED file LATEST.txt names (r-vN-gd.tmw never changes once written) rather than the moving pointer:
  # the mount served a pointer whose size was v7's and whose bytes were v8's (16:49Z)
  resolve_ptr() { local ptr=$1 dir=$(dirname $1) base=$(basename $1) v; v=$(grep -E "^$base = " $dir/LATEST.txt 2>/dev/null | awk '{print $3}'); if [ -n "$v" ] && [ -f $dir/$v ]; then echo $dir/$v; else echo $ptr; fi; }
  MODEL_R=$(resolve_ptr $MODEL_R); MODEL_RL=$(resolve_ptr $MODEL_RL); echo "models resolved: $MODEL_R $MODEL_RL"
  snap_model $MODEL_R $SNAP/r.tmw || { echo "gate model never loaded whole"; exit 1; }
  snap_model $MODEL_RL $SNAP/rl.tmw || { echo "local model never loaded whole"; exit 1; }
  W=$(dirname $MODEL_R); MODEL_R_PTR=$MODEL_R; MODEL_R=$SNAP/r.tmw; MODEL_RL=$SNAP/rl.tmw
  # which r-v<N> the latest pointer is (by md5), and the maps it trained on (its .md)
  RV=""; for f in $W/r-v*.tmw; do [ "$(md5sum < $f)" = "$(md5sum < $MODEL_R)" ] && RV=$(basename $f .tmw); done
  RLV=""; for f in $W/rl-v*.tmw; do [ "$(md5sum < $f)" = "$(md5sum < $MODEL_RL)" ] && RLV=$(basename $f .tmw); done
  TRAIN=$( [ -n "$RV" ] && grep -E "^  train " $W/$RV.md 2>/dev/null | awk '{print $2}' | tr '\n' ',' )
  HELD=$( [ -n "$RV" ] && grep -E "^  HELD-OUT " $W/$RV.md 2>/dev/null | awk '{print $2}' | tr '\n' ',' )
  echo "gate prior $RV ($(md5sum < $MODEL_R | cut -c1-8)) chain $RLV ($(md5sum < $MODEL_RL | cut -c1-8)) flags $TMR_FLAGS run $(date -u +%Y-%m-%dT%H:%MZ); trained on: $TRAIN held-out: $HELD" | tee $P/plan-r/MODELS.txt
  # the exhibit's map set: geom/human-maps.tsv (written by human-batch) + the hypothesis maps; the old walk over
  # every dir of the bank mount took 16 min
  if [ -f $G/human-maps.tsv ]; then { tail -n +2 $G/human-maps.tsv | cut -f1; for u in $HYP; do echo $u; done; } | sort -u > /tmp/plan-r.uids
  else { for d in $G/*/; do u=$(basename $d); [ -f $d/consensus.txt ] || [ -f $d/consensus.unverified.txt ] || continue; grep -q "modal_groups \[[0-9]" $d/consensus*.txt 2>/dev/null && echo $u; done; for u in $HYP; do echo $u; done; } | sort -u > /tmp/plan-r.uids; fi
  echo "plan-r over $(wc -l < /tmp/plan-r.uids) maps"
  # per map: the HYBRID first (geometric on roads, R where the graph has nothing — fast), then pure R; each under a
  # wall-clock cap (the chained estimator costs ~5 s per edge evaluation: Poland 2026 ran > 90 min un-capped)
  plan_r_one() { u=$1; f=$(mapfile_of $u); [ -n "$f" ] || return 0; [ -f $G/$u/gates.json ] || return 0
    timeout ${CAP_HYB:-900} nice $TMR plan $f --gates $G/$u/gates.json --model $MODEL_R --local $MODEL_RL --estimator hybrid $TMR_FLAGS ${HYB_FLAGS:-} --threads ${TMR_THREADS:-24} --top-k 3 --quiet --out-dir $RT --source router-plan-hyb > $P/plan-r/$u.hyb.txt 2>&1 || echo "TIMEOUT/ERROR after ${CAP_HYB:-900} s" >> $P/plan-r/$u.hyb.txt
    grep -E "cp_groups|rank 0|NO PLAN|TIMEOUT|hybrid pricing" $P/plan-r/$u.hyb.txt | head -3 | cut -c1-160
    [ -n "${SKIP_R:-}" ] && return 0
    timeout ${CAP_R:-1500} nice $TMR plan $f --gates $G/$u/gates.json --model $MODEL_R --local $MODEL_RL --estimator chained $TMR_FLAGS --threads ${TMR_THREADS:-24} --top-k 3 --quiet --out-dir $RT --source router-plan-r > $P/plan-r/$u.txt 2>&1 || echo "TIMEOUT/ERROR after ${CAP_R:-1500} s" >> $P/plan-r/$u.txt
    grep -E "cp_groups|rank 0|NO PLAN|TIMEOUT" $P/plan-r/$u.txt | head -2 | cut -c1-160; }
  CAP_HYB=${CAP_HYB:-900}; CAP_R=${CAP_R:-1500}; TMR_THREADS=${TMR_THREADS:-24}
  export -f plan_r_one mapfile_of; SKIP_R=${SKIP_R:-}; HYB_FLAGS=${HYB_FLAGS:-}; export HYB_FLAGS TMR TMR_FLAGS TMR_THREADS MODEL_R MODEL_RL G RT P B V CAP_HYB CAP_R SKIP_R
  cat /tmp/plan-r.uids | xargs -P ${PAR:-2} -n 1 bash -c 'plan_r_one "$0"'
  # no `index` here (a full walk of the bank mount is > 40 min); table-r reads only the exhibit's maps
  $R/tmroute table-r $RT --geom $G --geo router-plan-cost --r router-plan-r --hyb router-plan-hyb --uids $(cat /tmp/plan-r.uids | tr "\n" ",") --also $(echo $HYP | tr " " ",") --train "$TRAIN" --held-out "$HELD" --title "$(cat $P/plan-r/MODELS.txt)" > $P/table-r.md
  cp $P/table-r.md $P/table-r-$(date -u +%Y%m%dT%H%MZ)-$RV.md
  tail -1 $P/table-r.md
  push tm-route/routes tm-route/plan
fi
if [ $step = tables ] || [ $step = all ]; then
  pull tm-route/geom tm-route/routes tm-route/plan
  $R/tmroute index $RT --names $G
  $R/tmroute table $RT --geom $G --plan-source router-plan-cost > $P/table-cost.md
  $R/tmroute table $RT --geom $G --plan-source router-plan > $P/table-speed.md
  [ -f $RT/routes.tsv ] && grep -q router-plan-r $RT/routes.tsv && $R/tmroute table-r $RT --geom $G --geo router-plan-cost --r router-plan-r --hyb router-plan-hyb --also $(echo $HYP | tr " " ",") > $P/table-r.md
  tail -1 $P/table-cost.md; tail -1 $P/table-speed.md
  echo "human legs the surface graph cannot explain: $(grep -c MISSING $P/human-legs/ALL.tsv) of $(wc -l < $P/human-legs/ALL.tsv)"
  push tm-route/routes tm-route/plan
fi
