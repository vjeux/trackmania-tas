#!/bin/bash
# water-table.sh NN STEM TS  — ray grid at car height over the map's author-line bbox; Water hits grouped by plane y into
# footprints; item ids from `mapgeom who`; author-line crossings (samples within 1.5 m below..2 m above a plane inside its footprint)
NN=$1; STEM=$2; TS=$3
cd /tmp/tmp/repo/tools/route && source /tmp/tmp/env.sh
B=/tmp/tmp/bank-mirror/tm-route/tiny-builds/ship15-53383427; MAP="/tmp/ship15/Tiny Summer 2026 - $NN.Map.Gbx"
[ -f "$MAP" ] || MAP=$(ls /tmp/ship15/*" $NN.Map.Gbx" /tmp/ship15/*"$NN"*.Map.Gbx 2>/dev/null | head -1)
AL=$B/centreline/$STEM.author-line.json
O=~/persistent/private-30d/tm-route/gen/sections/$NN; mkdir -p $O
PTS=$(grep -o '"pts": \[.*\]' $AL | grep -o '\[[0-9.-]*,[0-9.-]*,[0-9.-]*\]' | tr -d '[]' | tr ',' ' ')
read x0 x1 z0 z1 y0 y1 <<< $(echo "$PTS" | awk 'BEGIN{x0=1e9;x1=-1e9;z0=1e9;z1=-1e9;y0=1e9;y1=-1e9} {if($1<x0)x0=$1; if($1>x1)x1=$1; if($3<z0)z0=$3; if($3>z1)z1=$3; if($2<y0)y0=$2; if($2>y1)y1=$2} END{printf "%d %d %d %d %d %d", x0-60, x1+60, z0-60, z1+60, y0, y1}')
# rays from several heights (the map roof blocks high starts; water can sit at any level): start 3 m above each distinct author level band
HS=$(echo "$PTS" | awk '{printf "%d\n", int($2/8)*8+10}' | sort -un | tr '\n' ' ')
: > /tmp/water-$NN.txt
for H in $HS; do
  rays=$(awk -v x0=$x0 -v x1=$x1 -v z0=$z0 -v z1=$z1 -v h=$H 'BEGIN{for (z=z0; z<=z1; z+=6) for (x=x0; x<=x1; x+=6) printf " --ray %d,%d,%d:0,-1,0", x, h, z}')
  target/release/tmplan local "$MAP" --gates $B/geom/$STEM.deck.json $rays 2>/dev/null | grep "Water" | sed -E 's/  ray ([0-9.-]*),[0-9.]*,([0-9.-]*):[^:]*: hit at [0-9.]* m \([0-9.-]*, ([0-9.-]*), [0-9.-]*\).*/\1 \2 \3/' >> /tmp/water-$NN.txt
done
sort -u /tmp/water-$NN.txt > /tmp/water-$NN.u.txt
{
echo -e "# map $NN water planes (ship15) — ray grid 6 m at car height, GEOM $TS. Water = TM physics 13 (rideable at speed / sinkable nose-down); each converted pool item also carries a coincident NotCollidable (28) plane. Campaign rule: zero water contact."
echo -e "plane_y\tx_min\tx_max\tz_min\tz_max\thits_6m\titems(anchor)\tauthor_line_samples_in_footprint_near_plane"
awk '{k=$3; if (!(k in n)) {x0[k]=$1;x1[k]=$1;z0[k]=$2;z1[k]=$2;n[k]=0} if ($1<x0[k])x0[k]=$1; if ($1>x1[k])x1[k]=$1; if ($2<z0[k])z0[k]=$2; if ($2>z1[k])z1[k]=$2; n[k]++} END {for (k in n) printf "%s\t%d\t%d\t%d\t%d\t%d\n", k, x0[k]-3, x1[k]+3, z0[k]-3, z1[k]+3, n[k]}' /tmp/water-$NN.u.txt | sort -n | while IFS=$'\t' read py xa xb za zb nh; do
  items=$(for pt in "$(( (xa+xb)/2 )),$py,$(( (za+zb)/2 ))" "$xa,$py,$za" "$xb,$py,$zb"; do /tmp/tmp/upstream-target/release/mapgeom who "$MAP" --at $pt --dy 1 --packs /tmp/tmp/server/Packs 2>/dev/null | grep "Water" | grep -o "item i[0-9]* [A-Z0-9]*.Item.Gbx at ([^)]*)"; done | sort -u | tr '\n' ';')
  cross=$(echo "$PTS" | awk -v py=$py -v xa=$xa -v xb=$xb -v za=$za -v zb=$zb '$1>=xa && $1<=xb && $3>=za && $3<=zb && $2>=py-1.5 && $2<=py+2.0 {c++} END {print c+0}')
  echo -e "$py\t$xa\t$xb\t$za\t$zb\t$nh\t$items\t$cross"
done
} > $O/WATER-$TS.tsv
cat $O/WATER-$TS.tsv | cut -c1-260
