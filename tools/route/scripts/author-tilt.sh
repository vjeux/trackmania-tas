#!/bin/bash
# author-tilt.sh NN STEM TS — the author's supporting surface per 100 ms sample (closest hit of 5 short rays: down, ±x, ±z, 2.5 m),
# its normal's angle to vertical = the tilt of what he rides; stretches with tilt >= 70° (wall rides) -> AUTHOR-TILT70-<TS>.tsv
NN=$1; STEM=$2; TS=$3
cd /tmp/tmp/repo/tools/route && source /tmp/tmp/env.sh
B=/tmp/tmp/bank-mirror/tm-route/tiny-builds/ship15-53383427; MAP="/tmp/ship15/Tiny Summer 2026 - $NN.Map.Gbx"
O=~/persistent/private-30d/tm-route/gen/sections/$NN; mkdir -p $O
grep -o '"pts": \[.*\]' $B/centreline/$STEM.author-line.json | grep -o '\[[0-9.-]*,[0-9.-]*,[0-9.-]*\]' | tr -d '[]' | tr ',' ' ' > /tmp/at-pts.txt
rays=$(awk '{for (d=0; d<5; d++) { split("0,-1,0 1,0,0 -1,0,0 0,0,1 0,0,-1", D, " "); printf " --ray %.2f,%.2f,%.2f:%s", $1, $2+0.3, $3, D[d+1] }}' /tmp/at-pts.txt)
target/release/tmplan local "$MAP" --gates $B/geom/$STEM.deck.json $rays 2>/dev/null | grep "ray " | sed -E 's/  ray ([0-9.-]*),[0-9.-]*,([0-9.-]*):[^:]*: hit at ([0-9.]*) m \([^)]*\) normal \([^,]*, ([^,]*), [^)]*\) ([A-Za-z]*).*/\1 \2 \3 \4 \5/; s/  ray ([0-9.-]*),[0-9.-]*,([0-9.-]*):.*nothing.*/\1 \2 99 1 none/' > /tmp/at-hits.txt
# 5 rays per sample in order; pick the closest hit within 2.5 m
awk -v ts=$TS -v nn=$NN 'BEGIN {print "# map " nn " — the AUTHOR'"'"'s supporting surface per 100 ms sample (closest of 5 short rays), tilt = angle of its normal to vertical; stretches with tilt >= 70° = wall rides the author himself does (allowance for the >= 70° publish rule). GEOM " ts; print "t_start\tt_end\tdur_s\tx\ty\tz\ttilt_deg_max\tmaterial"}
{ n++; i=int((n-1)/5); if (!(i in best) || $3+0 < best[i]+0) { best[i]=$3; ny[i]=$4; mat[i]=$5 } }
END { for (i=0; i<=int((n-1)/5); i++) { tilt = (best[i] < 2.5) ? atan2(sqrt(1-ny[i]*ny[i]), (ny[i]<0?-ny[i]:ny[i]))*57.2958 : -1; T[i]=tilt } 
  inrun=0; for (i=0; i<=int((n-1)/5); i++) { if (T[i] >= 70) { if (!inrun) { s=i; mx=T[i]; m=mat[i]; inrun=1 } else if (T[i] > mx) mx=T[i] } else if (inrun) { printf "%.1f\t%.1f\t%.1f\t%s\t%.1f\t%s\n", s/10, (i-1)/10, (i-s)/10, P[s], mx, m; inrun=0 } } if (inrun) printf "%.1f\t%.1f\t%.1f\t%s\t%.1f\t%s\n", s/10, (i-1)/10, (i-s)/10, P[s], mx, m }
' /tmp/at-hits.txt > /tmp/at-out.txt
# positions for the stretch starts
awk 'NR==FNR {P[NR-1]=$1"\t"$2"\t"$3; next} FNR<=2 {print; next} {split($0,a,"\t"); s=int(a[1]*10+0.5); print a[1]"\t"a[2]"\t"a[3]"\t"P[s]"\t"a[5]"\t"a[6]}' /tmp/at-pts.txt /tmp/at-out.txt > $O/AUTHOR-TILT70-$TS.tsv
echo "samples $(wc -l < /tmp/at-pts.txt); tilt>=70 stretches: $(($(wc -l < $O/AUTHOR-TILT70-$TS.tsv)-2))"
awk 'NR>2' $O/AUTHOR-TILT70-$TS.tsv
# also the distribution
awk '{n++; i=int((n-1)/5); if (!(i in best) || $3+0 < best[i]+0) { best[i]=$3; ny[i]=$4 }} END {for (i=0; i<=int((n-1)/5); i++) { if (best[i]<2.5) { t=atan2(sqrt(1-ny[i]*ny[i]), (ny[i]<0?-ny[i]:ny[i]))*57.2958; b=int(t/10)*10; c[b]++ } else c["air"]++ } for (k in c) printf "%s:%d ", k, c[k]; print ""}' /tmp/at-hits.txt
