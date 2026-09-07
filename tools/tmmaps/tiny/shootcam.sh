#!/bin/bash
# shootcam.sh OUT.png "x,y,z,dist,h,v"  — aim + screenshot the map ALREADY open
# in the editor (the tail of shootmap.sh, for a map that took longer to load
# than shootmap.sh waited, or for a second view of the same map).
set -u
OUT=$1; CAM=$2
WSX=/home/vjeux/bin/wsx
$WSX sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; printf "%s,%s" "'"$CAM"'" "'"$RANDOM$$"'" > /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp && mv /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp /mnt/c/Users/vjeux/OpenplanetNext/cam.txt; for t in 1 2 3 4 5 6; do rm -f /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv; printf "p%s%s" "$t" "$RANDOM" > /mnt/c/Users/vjeux/OpenplanetNext/probe.txt; sleep 5; [ -s /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv ] && break; done; echo "--- game kept: $(grep -c "^item" /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv) item rows, $(grep -c "^block" /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv) block rows"; sleep 3; /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -ExecutionPolicy Bypass -File C:\\Users\\vjeux\\shotdpi.ps1 C:\\Users\\vjeux\\shotmap.png >/dev/null; exit 0' 2>&1 | tail -2
$WSX pull /mnt/c/Users/vjeux/shotmap.png "$OUT" 2>&1 | grep -v chunk | tail -1
