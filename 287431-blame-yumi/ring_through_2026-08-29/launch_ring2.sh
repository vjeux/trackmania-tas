#!/bin/bash
# Round 2: seeded on the COMPLIANT laps found in round 1.
#
# A compliant tape already passes the +y4/+y9 staircase rungs, so those two
# launches per batch buy nothing; the four axis constraints at 14 m are what
# bounds the crossing (|delta| <= 15.7 m inside a 45 m aperture). rD keeps the
# full six-rung ladder because it starts from the NON-compliant fast line and
# has to climb.
set -u
TS=/tmp/tm/repo/tools/search/target/release/tmsearch
export TM_SERVER=/tmp/tm/server
cd /tmp/tm/work
M=ring/must
TIGHT="--must $M/m2_yp14.Map.Gbx --must $M/m3_zp14.Map.Gbx --must $M/m4_ym14.Map.Gbx --must $M/m5_zm14.Map.Gbx"
FULL="--must $M/m0_yp4.Map.Gbx --must $M/m1_yp9.Map.Gbx $TIGHT"

launch() { # name seed musts lo hi window stride nops temp extra...
  local n=$1 seed=$2 musts=$3 lo=$4 hi=$5 w=$6 st=$7 nops=$8 temp=$9; shift 9
  local D=ring/arms2/$n
  mkdir -p $D
  nohup $TS search --template BEST20852.Ghost.Gbx --start-from "$seed" \
    --map stockmap.Map.Gbx $musts --must-window 0.12 \
    --server /tmp/tm/server --root $D/root --bestdir $D/best --log $D/log.jsonl \
    --workers 20 --batch 200 --ops wide --nops-upto $nops \
    --lo $lo --hi $hi --window $w --stride $st --temp $temp \
    --seed $RANDOM --minutes 600 "$@" > $D/out.txt 2>&1 &
  echo "$n pid $!"
}

S1=ring/seeds/ring20831.Ghost.Gbx
S2=ring/seeds/ring20945.Ghost.Gbx

launch sA "$S1" "$TIGHT" 1700 2768  90 11 14 0.25
launch sB "$S1" "$TIGHT" 1300 2768 260 19 20 0.9  --full-window-every 3
launch sC "$S2" "$TIGHT" 1900 2768  50  7 12 0.10
launch sD BEST20852.Ghost.Gbx "$FULL" 1200 2768 400 29 24 1.6 --full-window-every 3
