#!/bin/bash
# usage: overlay-run.sh TAG BLOCKNAME YAWOFF [SWAP]  -> prints yellow% in a 110 m top-down shot centred on the block
cd "$(dirname "$0")/../.." || exit 1
TAG=$1; NAME=$2; OFF=$3; SWAP=$4
B="target/release/mapgeom tiny-assets /tmp/Summer-2026-01.Map.Gbx --catalog tmmaps/tiny/summer-2026-resolved.tsv --footprints tmmaps/tiny/summer-2026-footprints.tsv --nadeo-zip /tmp/Nadeo.zip --empty-template tmmaps/tiny/empty.Item.Gbx --blue-pak /tmp/BlueBay.pak --stadium-pak /tmp/current-Stadium.pak"
if [ -n "$SWAP" ]; then TINY_SWAP_XZ=1 $B --scale 1 --out /tmp/OvFull.Map.Gbx --library-out /tmp/OvLib.zip >/dev/null 2>&1; else $B --scale 1 --out /tmp/OvFull.Map.Gbx --library-out /tmp/OvLib.zip >/dev/null 2>&1; fi
target/release/tmmaps tiny-catalog /tmp/Summer-2026-01.Map.Gbx --mapping /tmp/OvLib.placements.tsv --library /tmp/OvLib.zip --out /tmp/Ov.Map.Gbx --only "$NAME" --raise 3 --overlay --yaw-offset "$OFF" >/dev/null 2>&1
alive=$(/tmp/loadmap.sh /tmp/Ov.Map.Gbx | tail -1)
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; printf "656,34,672,110,3.14159,1.35,%s" "'"$RANDOM"'" > /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp && mv /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp /mnt/c/Users/vjeux/OpenplanetNext/cam.txt; sleep 6; /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -ExecutionPolicy Bypass -File C:\\Users\\vjeux\\shotdpi.ps1 C:\\Users\\vjeux\\ov.png >/dev/null; exit 0' >/dev/null 2>&1
/home/vjeux/bin/wsx pull /mnt/c/Users/vjeux/ov.png /tmp/ov-$TAG.png >/dev/null 2>&1
echo "$TAG $NAME yaw+$OFF swap=${SWAP:-0} alive=$alive: $(target/release/examples/pixstat /tmp/ov-$TAG.png 1200 500 2640 1660 | tail -1 | cut -d: -f2)"
