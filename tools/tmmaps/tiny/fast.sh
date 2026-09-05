#!/bin/bash
# fast.sh PREFAB ID [envs...]  -> builds the crystal via the generator path, tests with COPIES placements, prints copper or DEAD
cd /home/vjeux/trackmania-tas-tiny/tools
env "${@:3}" target/release/mapgeom --pak /tmp/BlueBay.pak:660C4C156B80337E296A1034B0AA05B8 --pak /tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6 crystal-item "$1" --template /tmp/crystal.Item.Gbx --out /tmp/$2.Item.Gbx --ident $2.Item.Gbx --collection 28 2>&1 | grep -v '^warning' | tail -1 >&2
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; /mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq Trackmania.exe" /NH | tr -d "\r" | grep -q Trackmania || "$S" launch 180 >/dev/null 2>&1; exit 0'
out=$(cd /tmp && COPIES=${COPIES:-300} HOST=/tmp/Summer-2026-01.Map.Gbx HOST_ANCHOR="1024,400,1024" CAM="1120,400,1024,120,0.3,-0.3" /home/vjeux/bin/itemtest /tmp/$2.Item.Gbx $2.Item.Gbx $2.Item.Gbx 0.5 embed 2>&1 | grep -E "^copper" | cut -f2)
alive=$(/home/vjeux/bin/wsx sh '/mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq Trackmania.exe" /NH | tr -d "\r" | grep -c Trackmania; exit 0' 2>/dev/null | tail -1)
echo "$2: copper=${out:-?} alive=$alive"
