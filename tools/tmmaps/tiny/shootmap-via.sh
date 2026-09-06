#!/bin/bash
# shootmap-via.sh MAP OUT.png "x,y,z,dist,h,v" [NAME]
# Same contract as shootmap.sh, but the render-box bridge is driven from
# another box ($SHOOT_VIA, default devvm64230): the devserver's own user
# certificate expires every 3 days and with it fwdproxy refuses the bridge
# (2026-09-06, 19:53 UTC — the shot that failed was 30 s later). The map is
# copied over by scp, shootmap.sh runs there, the screenshot comes back.
set -u
MAP=$1; OUT=$2; CAM=$3; NAME=${4:-Shot}
VIA=${SHOOT_VIA:-devvm64230.cln0.facebook.com}
SSH="ssh -q -o BatchMode=yes -o StrictHostKeyChecking=no"
T=/home/vjeux/trackmania-tas-tiny/tools/tmmaps/tiny
$SSH "$VIA" "mkdir -p /tmp/tiny3 $T" || { echo "cannot reach $VIA"; exit 2; }
scp -q -o BatchMode=yes -o StrictHostKeyChecking=no "$T/shootmap.sh" "$VIA:$T/shootmap.sh"
scp -q -o BatchMode=yes -o StrictHostKeyChecking=no "$MAP" "$VIA:/tmp/tiny3/via-$NAME.Map.Gbx" || { echo "scp of $MAP failed"; exit 2; }
$SSH "$VIA" "$T/shootmap.sh /tmp/tiny3/via-$NAME.Map.Gbx /tmp/tiny3/via-$NAME.png '$CAM' $NAME"
scp -q -o BatchMode=yes -o StrictHostKeyChecking=no "$VIA:/tmp/tiny3/via-$NAME.png" "$OUT"
ls -la "$OUT" 2>/dev/null | awk '{print $5, $9}'
