#!/bin/bash
# section-spec.sh NN STEM "g_a:g_b:label:read" ...   (gates 1-based in the human order; g 0 = spawn)
NN=$1; STEM=$2; shift 2
M=/tmp/tmp/bank-mirror/tm-route; B=$M/tiny-builds/ship15-53383427; A=$B/arrival-bands/$STEM.arrival-bands.v3.json; L=/tmp/lapbands/$NN-lap.bands.json
AL=$B/centreline/$STEM.author-line.json; OUT=~/persistent/private-30d/tm-route/gen/sections/$NN; mkdir -p $OUT
cert=$(grep -o "([0-9.]* s)" /tmp/lapbands/$NN.log | head -1 | tr -d '( s)')
{
echo "# SECTIONS — map $NN ($STEM) — certified lap $cert s (INPUT ghosts-for-video README-0722), author $(jq -r '.gates[-1].author.t_s' $A) s"
echo "Method: bounded section tests forked from the IMMUTABLE certified lap. Fork = our lap's crossing of the start gate (tick = 10 ms from race 0); window = start gate → end gate; the lane must re-credit every gate in the window and beat the certified crossing time at the end gate; the rest of the lap is the certified inputs (state-matched refit at the end). Gates are 1-based in the human order (g0 = spawn). Pursue files: x y z speed-cap (author speed + 2), the author's 100 ms samples."
echo
for spec in "$@"; do
  IFS=: read ga gb label read <<< "$spec"
  if [ "$ga" = 0 ]; then ft=0; fpos="spawn"; fv=0; at_a=0; else
    ft=$(jq -r ".gates[$((ga-1))].author.t_s" $L); fpos=$(jq -r ".gates[$((ga-1))].author.pos|map(.*10|round/10)|tostring" $L); fv=$(jq -r ".gates[$((ga-1))].author.speed_mps|floor" $L); at_a=$(jq -r ".gates[$((ga-1))].author.t_s" $A); fi
  ot=$(jq -r ".gates[$((gb-1))].author.t_s // \"n/a\"" $L); at_b=$(jq -r ".gates[$((gb-1))].author.t_s" $A)
  wpa=$([ "$ga" = 0 ] && echo spawn || jq -r ".gates[$((ga-1))].map_waypoint" $A); wpb=$(jq -r ".gates[$((gb-1))].map_waypoint" $A)
  pfile=sec-g$ga-g$gb-author-pursue.tsv
  grep -o '"pts": \[.*\]' $AL | grep -o '\[[0-9.-]*,[0-9.-]*,[0-9.-]*\]' | awk -F'[][,]' -v a=$at_a -v c=$at_b 'NR>1 {t=(NR-1)/10; d=sqrt(($2-px)^2+($3-py)^2+($4-pz)^2); if (t>=a-0.2 && t<=c+0.3) printf "%.1f %.1f %.1f %.0f\n", $2, $3, $4, d*10+2} {px=$2;py=$3;pz=$4}' > $OUT/$pfile
  gates=$(jq -r ".gates[$((ga>0?ga-1:0)):$gb][] | \"g\(.idx+1) wp\(.map_waypoint) author t\(.author.t_s|.*10|round/10) v\(.author.speed_mps|floor) hdg\(.band.heading_deg.centre|floor)\"" $A | tr '\n' ';')
  ours=$(jq -r ".gates[$((ga>0?ga-1:0)):$gb][] | \"g\(.idx+1) t\(.author.t_s|.*10|round/10) v\(.author.speed_mps|floor)\"" $L | tr '\n' ';')
  vmax=$(awk 'BEGIN{m=0} {if ($4>m) m=$4} END {print m-2}' $OUT/$pfile); vmin=$(awk 'BEGIN{m=999} NR>3 {if ($4<m) m=$4} END {print m-2}' $OUT/$pfile)
  printf "## Section g%s → g%s (%s → wp%s): %s\n" "$ga" "$gb" "$wpa" "$wpb" "$label"
  printf -- "- Fork: our lap at g%s, t = %s s (tick %d), pos %s, %s m/s. Window end: our g%s at %s s; the author does the window in %.1f s (ours %.1f s) → target ≤ %.1f s.\n" "$ga" "$ft" "$(echo "$ft*100/1" | bc)" "$fpos" "$fv" "$gb" "$ot" "$(echo "$at_b - $at_a" | bc)" "$(echo "$ot - $ft" | bc 2>/dev/null || echo n/a)" "$(echo "($at_b - $at_a)*1.15" | bc)"
  printf -- "- Author through the window: %s speeds %s–%s m/s. Ours: %s\n" "$gates" "$vmin" "$vmax" "$ours"
  printf -- "- Read: %s\n" "$read"
  printf -- "- Pursue file: %s (%d rows). Probes: every gate in the window re-credited; end-gate time ≤ target; end-gate speed ≥ %d m/s (author − 15 %%).\n\n" "$pfile" "$(wc -l < $OUT/$pfile)" "$(jq -r ".gates[$((gb-1))].author.speed_mps*0.85|floor" $A)"
done
} > $OUT/SPEC.md
echo "$OUT/SPEC.md: $(grep -c '^## ' $OUT/SPEC.md) sections"
