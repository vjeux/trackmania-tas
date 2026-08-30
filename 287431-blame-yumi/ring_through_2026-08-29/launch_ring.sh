#!/bin/bash
# Four arms searching for a lap that finishes THROUGH the ring.
#
# The objective is the stock map's lap time, with a HARD constraint: every
# finisher must also finish on six variant maps, each the stock map with the
# finish ring translated by a known amount. A run that threads a 4 m gap in the
# ring's rim (which is what the 20.852 does) dies the moment the ring moves;
# a run through the middle of the 45 m aperture survives all six. Passing all
# six bounds the crossing to within ~14 m of the aperture centre.
set -u
TS=/tmp/tm/repo/tools/search/target/release/tmsearch
export TM_SERVER=/tmp/tm/server
cd /tmp/tm/work
M=ring/must
MUST="--must $M/m0_yp4.Map.Gbx --must $M/m1_yp9.Map.Gbx --must $M/m2_yp14.Map.Gbx \
      --must $M/m3_zp14.Map.Gbx --must $M/m4_ym14.Map.Gbx --must $M/m5_zm14.Map.Gbx"

launch() { # name seedfile lo hi window stride nops temp extra
  local n=$1 seed=$2 lo=$3 hi=$4 w=$5 st=$6 nops=$7 temp=$8; shift 8
  local D=ring/arms/$n
  mkdir -p $D
  nohup $TS search --template BEST20852.Ghost.Gbx --start-from "$seed" \
    --map stockmap.Map.Gbx $MUST --must-window 0.12 \
    --server /tmp/tm/server --root $D/root --bestdir $D/best --log $D/log.jsonl \
    --workers 20 --batch 200 --ops wide --nops-upto $nops \
    --lo $lo --hi $hi --window $w --stride $st --temp $temp \
    --seed $RANDOM --minutes 600 "$@" > $D/out.txt 2>&1 &
  echo "$n pid $!"
}

launch rA BEST20852.Ghost.Gbx            1700 2768 100 13 14 0.4
launch rB ring/seeds/z2_best_20_792.Ghost.Gbx 1500 2768 250 19 20 1.0 --full-window-every 3
launch rC ring/seeds/y2_best_20_804.Ghost.Gbx 1950 2768  50  7 10 0.15
launch rD BEST20852.Ghost.Gbx            1200 2768 400 29 24 1.8 --full-window-every 3
