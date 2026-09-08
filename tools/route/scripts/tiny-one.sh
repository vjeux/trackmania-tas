#!/bin/bash
# tiny-one.sh <map.Map.Gbx> [--build NAME] [--manifest MANIFEST.txt] [--order g,g,g]
# The whole tiny pipeline for ONE map on the mirror: collhash (verified against the MANIFEST when given, else recorded
# as measured) → gates → deck-gates (+ measured credit offsets) → geometric + hybrid Hypothesis routes → road-following
# centreline (+ route file, verdicts, exclusions) → a per-map tarball in the bank's drops/ with a MANIFEST line.
# Everything stamped with the build name, map md5 and collhash. ~1–3 min per map.
set -u
source /tmp/tmp/env.sh
cd /tmp/tmp/repo/tools/route
MAP=$(readlink -f "$1"); shift
BUILD=out2; MANI=""; ORDER=""
while [ $# -gt 0 ]; do case "$1" in --build) BUILD=$2; shift 2;; --manifest) MANI=$2; shift 2;; --order) ORDER=$2; shift 2;; *) shift;; esac; done
R=target/release; M=/tmp/tmp/bank-mirror; REAL=$HOME/persistent/private-30d/tm-route
W=$M/tm-route/model/watch; COLLHASH=/tmp/tmp/upstream-target/release/mapgeom
b=$(basename "$MAP" .Map.Gbx)
OUT=$M/tm-route/tiny-builds/$BUILD; GT=$OUT/geom; RT=$OUT/routes; P=$OUT/plan; C=$OUT/centreline; mkdir -p $GT $RT $P $C
md5=$(md5sum < "$MAP" | cut -d' ' -f1)
coll=$($COLLHASH collhash "$MAP" 2>&1 | grep -o "collision [0-9a-f]*" | cut -d' ' -f2)
if [ -n "$MANI" ]; then want=$(awk -v m="$(basename "$MAP")" '$1==m {print $3}' "$MANI"); [ "$coll" = "$want" ] || { echo "$b: collhash $coll != MANIFEST $want — REFUSED"; exit 2; }; cver="verified against $(basename $MANI)"; else cver="measured"; fi
note="tiny build $BUILD; map md5 $md5; collhash $coll ($cver, mapgeom collhash @ upstream-main 69164def)"
# models: the frozen exhibit pair when snapshotted, else LATEST
S=$(ls -d /tmp/tmr-models-* 2>/dev/null | tail -1)
if [ -n "$S" ] && [ -f $S/r.tmw ]; then MODEL_R=$S/r.tmw; MODEL_RL=$S/rl.tmw; else MODEL_R=$W/$(grep "r-latest-gd.tmw" $W/LATEST.txt | awk '{print $3}'); MODEL_RL=$W/$(grep "^rl-latest.tmw" $W/LATEST.txt | awk '{print $3}'); fi
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
# 3. centreline in the route order (hybrid, else geometric, else --order)
order=${ORDER:-${hyb:-$geo}}; src=hybrid; [ -z "$hyb" ] && src=geometric
if [ -n "$order" ]; then
  nice $R/tmplan road-centreline "$MAP" --gates $GT/$b.deck.json --order $order --out $C/$b.road-centreline.json --route-out $C/$b.route-router-road-centreline-0.json --verdicts $M/tm-route/tiny/gap-verdicts.tsv --exclusions $M/tm-route/tiny/road-exclusions.tsv --map-stem $b --note "order from the $src route; $note; gates $b.deck.json" > $C/$b.centreline.txt 2>&1
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
