#!/bin/bash
# opening-surfaces.sh NN STEM T0 T1 [CHAIN.tsv] — the collidable items under the author's line (and a chain) in [T0,T1],
# and which of them the ship16-a3d59cb4 colldiff re-dresses (physics/material change) → one line per changed item
NN=$1; STEM=$2; T0=$3; T1=$4; CH=$5
W=/tmp/tmp/upstream-target/release/mapgeom; P="--packs /tmp/tmp/server/Packs"
B=/tmp/tmp/bank-mirror/tm-route/tiny-builds/ship15-53383427; MAP="/tmp/ship15/Tiny Summer 2026 - $NN.Map.Gbx"
D=~/persistent/private-30d/tm-player/tiny/incoming/ship16-a3d59cb4/colldiff/$NN.txt
pts() { grep -o '"pts": \[.*\]' $B/centreline/$STEM.author-line.json | grep -o '\[[0-9.-]*,[0-9.-]*,[0-9.-]*\]' | tr -d '[]' | awk -F, -v a=$T0 -v c=$T1 'NR>1 {t=(NR-1)/10; if (t>=a && t<=c && NR%3==0) printf "%.1f,%.1f,%.1f\n", $1, $2, $3}'; }
{ pts; [ -n "$CH" ] && awk -F'\t' -v a=$T0 -v c=$T1 'NR>1 && $2>=a && $2<=c && NR%3==0 {printf "%.1f,%.1f,%.1f\n", $3, $4, $5}' $CH; } | sort -u > /tmp/os-pts.txt
: > /tmp/os-items.txt
while read p; do $W $P who "$MAP" --at $p --dy 1.5 2>/dev/null | grep "^  y" | grep -v NotCollidable | head -2 | grep -o "AC${NN}[0-9]*\.Item\.Gbx\|AI${NN}[0-9]*\.Item\.Gbx" >> /tmp/os-items.txt; done < /tmp/os-pts.txt
sort /tmp/os-items.txt | uniq -c | sort -rn > /tmp/os-count.txt
echo "# map $NN [$T0,$T1] s: $(wc -l < /tmp/os-pts.txt) probe points, $(wc -l < /tmp/os-count.txt) distinct items under the line(s)"
while read n it; do row=$(grep -m1 " $it " $D); if [ -n "$row" ]; then ch=$(echo "$row" | grep -o "RE-DRESSED.*" | cut -c1-140); blk=$(echo "$row" | awk -F'= ' '{print $2}' | awk '{print $1}'); echo "CHANGED  $it ($n probes) $blk: $ch"; else echo "same     $it ($n probes)"; fi; done < /tmp/os-count.txt
