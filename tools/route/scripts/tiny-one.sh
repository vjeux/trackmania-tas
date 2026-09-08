#!/bin/bash
# tiny-one.sh <map.Map.Gbx> [--build NAME] [--manifest MANIFEST.txt] [--order g,g,g]
# The whole tiny pipeline for ONE map on the mirror: collhash (verified against the MANIFEST when given, else recorded
# as measured) → gates → deck-gates (+ measured credit offsets) → geometric + hybrid Hypothesis routes → road-following
# centreline (+ route file, verdicts, exclusions) → a per-map tarball in the bank's drops/ with a MANIFEST line.
# Everything stamped with the build name, map md5 and collhash. ~1–3 min per map.
set -u
source /tmp/tmp/env.sh
cd /tmp/tmp/repo/tools/route
MAP=$(cd "$(dirname "$1")" && pwd)/$(basename "$1"); shift
BUILD=out2; MANI=""; ORDER=""
while [ $# -gt 0 ]; do case "$1" in --build) BUILD=$2; shift 2;; --manifest) MANI=$2; shift 2;; --order) ORDER=$2; shift 2;; *) shift;; esac; done
R=target/release; M=/tmp/tmp/bank-mirror; REAL=$HOME/persistent/private-30d/tm-route
W=$M/tm-route/model/watch; COLLHASH=/tmp/tmp/upstream-target/release/mapgeom
b=$(basename "$MAP" .Map.Gbx)
OUT=$M/tm-route/tiny-builds/$BUILD; GT=$OUT/geom; RT=$OUT/routes; P=$OUT/plan; C=$OUT/centreline; mkdir -p $GT $RT $P $C
md5=$(md5sum < "$MAP" | cut -d' ' -f1)
coll=$($COLLHASH collhash "$MAP" 2>&1 | grep -o "collision [0-9a-f]*" | cut -d' ' -f2)
# MANIFEST formats: out2 (file  md5  collhash  moving) or ship6+ (NN  MB  items  md5-8  collhash) keyed by the map number
real=$(readlink -f "$MAP"); rb=$(basename "$real" .Map.Gbx); nn=""; [[ "$rb" =~ ([0-9][0-9])$ ]] && nn=${BASH_REMATCH[1]}
if [ -n "$MANI" ]; then
  row=$(awk -F'\t' -v m="$(basename "$real")" -v nn="$nn" '$1==m || (nn!="" && $1==nn)' "$MANI" | head -1)
  if [ -z "$row" ]; then row=$(awk -v m="$(basename "$real")" '$1==m' "$MANI" | head -1); fi
  wmd5=$(echo "$row" | grep -oE "\b[0-9a-f]{8,32}\b" | head -1); wcoll=$(echo "$row" | grep -oE "\b[0-9a-f]{16}\b" | tail -1)
  [ -n "$wmd5" ] && [ "${md5:0:${#wmd5}}" = "$wmd5" ] || { echo "$b: md5 $md5 != MANIFEST $wmd5 — REFUSED"; exit 2; }
  if [ "$coll" = "$wcoll" ]; then cver="verified against $(basename $MANI)"; else cver="MANIFEST says $wcoll (the converter's collhash build), measured $coll with mapgeom collhash @ upstream-main 49da4bf7 — tool versions differ, md5 verified"; fi
else cver="measured"; fi
note="tiny build $BUILD; map md5 $md5; collhash $coll ($cver)"
# models: the frozen exhibit pair when snapshotted, else LATEST
S=$(ls -d /tmp/tmr-models-* 2>/dev/null | tail -1)
if [ -n "$S" ] && [ -f $S/r.tmw ]; then MODEL_R=$S/r.tmw; MODEL_RL=$S/rl.tmw; else MODEL_R=$W/$(grep "^r-latest.tmw" $W/LATEST.txt | awk '{print $3}'); MODEL_RL=$W/$(grep "^rl-latest.tmw" $W/LATEST.txt | awk '{print $3}'); fi
# 1. gates + deck gates
$R/tmroute gates "$MAP" --out $GT/$b.gates.json > $GT/$b.gates.txt 2>&1
nice $R/tmplan deck-gates "$MAP" --gates $GT/$b.gates.json --out $GT/$b.deck.json > $GT/$b.deck.txt 2>&1
ctl=$(head -1 $GT/$b.gates.txt | cut -f6)
# 2. routes (geometric cost + hybrid), Hypothesis, provenance in produced_by
nice $R/tmplan plan "$MAP" --gates $GT/$b.deck.json --out-dir $RT --top-k 3 --matrix --quiet --time cost --exact --source router-plan-cost --note "$note" > $P/$b.cost.txt 2>&1
grep -q "NO PLAN" $P/$b.cost.txt && nice $R/tmplan plan "$MAP" --gates $GT/$b.deck.json --out-dir $RT --top-k 3 --quiet --time cost --exact --grid deco --source router-plan-cost --note "$note" > $P/$b.cost.txt 2>&1
timeout 900 nice $R/tmr plan "$MAP" --gates $GT/$b.deck.json --model $MODEL_R --local $MODEL_RL --estimator hybrid --p-floor 0 --p-step 0.02 --threads 16 --top-k 3 --quiet --out-dir $RT --source router-plan-hyb --note "$note" > $P/$b.hyb.txt 2>&1 || echo "EXIT $?" >> $P/$b.hyb.txt
geo=$(grep -E 'rank 0' $P/$b.cost.txt | head -1 | grep -o 'groups \[[0-9,]*\]' | tr -d 'groups []')
hyb=$(grep -E 'rank 0' $P/$b.hyb.txt | head -1 | grep -o 'groups \[[0-9,]*\]' | tr -d 'groups []')
VERD=$M/tm-route/tiny/gap-verdicts-$BUILD.tsv; [ -f $VERD ] || VERD=$M/tm-route/tiny/gap-verdicts.tsv
# 3. centreline in the route order (hybrid, else geometric, else --order)
order=${ORDER:-${hyb:-$geo}}; src=hybrid; [ -z "$hyb" ] && src=geometric
if [ -n "$order" ]; then
  # the engine's tick-0 pose when the INPUT arm has measured it for this build (ENGINE-SPAWNS-<build-short>.tsv)
  nn=${b:0:2}; short=${BUILD%%-*}; ES=$M/tm-player/tiny/gate-crossings/ENGINE-SPAWNS-$short.tsv; [ -f $ES ] || ES=$(ls -t $M/tm-player/tiny/gate-crossings/ENGINE-SPAWNS-*.tsv 2>/dev/null | head -1); sp=$(awk -F'\t' -v n="$nn" '$1==n {print $2","$3","$4}' "$ES" 2>/dev/null | head -1); SPAWN=""; [ -n "$sp" ] && SPAWN="--spawn $sp"
  nice $R/tmplan road-centreline "$MAP" --gates $GT/$b.deck.json --order $order $SPAWN --out $C/$b.road-centreline.json --route-out $C/$b.route-router-road-centreline-0.json --verdicts $VERD --exclusions $M/tm-route/tiny/road-exclusions.tsv --map-stem $b --note "order from the $src route; $note; gates $b.deck.json" > $C/$b.centreline.txt 2>&1
  cl=$(grep "pts," $C/$b.centreline.txt | sed -E 's/.*: ([0-9]+) pts, ([0-9]+) m, ([0-9]+) segments, ([0-9]+) gaps, on-road ([0-9.]+) %(.*)→.*/\1 pts \2 m, \4 gaps of \3 legs, on-road \5 %\6/')
else cl="no route order — no centreline"; fi
# 4. one tarball per map into the bank
ts=$(date -u +%Y%m%dT%H%MZ)
tgz=/tmp/tiny-$BUILD-$b-$ts.tgz
uid=$(head -1 $GT/$b.gates.txt | cut -f2)
files="geom/$b.gates.json geom/$b.gates.txt geom/$b.deck.json geom/$b.deck.txt plan/$b.cost.txt plan/$b.hyb.txt centreline/$b.road-centreline.json centreline/$b.route-router-road-centreline-0.json centreline/$b.centreline.txt"
[ -d $RT/$uid ] && files="$files routes/$uid"
tar czf $tgz -C $M/tm-route/tiny-builds/$BUILD $files 2>/dev/null
mkdir -p $REAL/drops && cp $tgz $REAL/drops/ && echo -e "$ts\t$(basename $tgz)\ttiny $BUILD $b: gates ($ctl), deck gates, routes, centreline; md5 $md5 collhash $coll\t$(stat -c %s $tgz)\t$(md5sum < $tgz | cut -c1-12)" >> $REAL/drops/MANIFEST.tsv
echo "$b [$BUILD md5 ${md5:0:8} collhash $coll $cver] gates $ctl | geo ${geo:-none} | hyb ${hyb:-none} | centreline: $cl → drops/$(basename $tgz)"
