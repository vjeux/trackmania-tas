#!/bin/bash
# itemtest ITEM_FILE IDENT AUTHOR SCALE [embed]
# Copies the item into the game's Items folder (or embeds it), builds a
# one-item map beside a stock 16 m arch, loads it, probes what the game kept,
# and pulls a screenshot to /tmp/itemtest.png. ~60 s.
set -u
ITEM=$1; IDENT=$2; AUTHOR=$3; SCALE=$4; MODE=${5:-local}
T=/home/vjeux/trackmania-tas-tiny/tools
W='/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania'
if [ "$MODE" = embed ]; then
  $T/target/release/examples/single_item_map ${HOST:-/tmp/Stadium-Host.Map.Gbx} /tmp/One.Map.Gbx "$IDENT" "$AUTHOR" "$SCALE" "$ITEM" | tail -1
  /home/vjeux/bin/wsx sh "rm -f '$W/Items/$IDENT'; exit 0" >/dev/null
else
  $T/target/release/examples/single_item_map ${HOST:-/tmp/Stadium-Host.Map.Gbx} /tmp/One.Map.Gbx "$IDENT" "$AUTHOR" "$SCALE" | tail -1
  /home/vjeux/bin/wsx push "$ITEM" "$W/Items/${IDENT//\\//}" 2>&1 | grep -v chunk | tail -1
fi
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; if ! timeout 8 "$S" get /ping >/dev/null 2>&1; then echo "game not responding: relaunching"; /mnt/c/Windows/System32/taskkill.exe /F /IM Trackmania.exe >/dev/null 2>&1; sleep 4; "$S" launch 180 2>&1 | tail -1; fi; exit 0'
/home/vjeux/bin/wsx push /tmp/One.Map.Gbx "$W/Maps/_shoot/One.Map.Gbx" 2>&1 | grep -v chunk | tail -1
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; seen=0; for k in 1 2 3 4 5 6 7 8; do c=$("$S" get /ctx); case "$c" in *\"ctx\":0*\"dialog\":null*) break;; *FrameDialogSaveAs*) "$S" get /dismiss >/dev/null; seen=1;; *FrameAskYesNo*) if [ $seen = 1 ]; then "$S" get /yes >/dev/null; else "$S" get /no >/dev/null; fi;; *\"dialog\":null*) "$S" get /back >/dev/null;; *) "$S" get /dismiss >/dev/null;; esac; sleep 2; done; rm -f /mnt/c/Users/vjeux/OpenplanetNext/probe.txt /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv; printf "%s" "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/One.Map.Gbx" > /mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter/editmap.txt; "$S" get /editmap >/dev/null; for i in 1 2 3 4 5 6; do sleep 7; c=$("$S" get /ctx); case "$c" in *\"ctx\":1*) break;; *FrameAskYesNo*) echo "DIALOG: missing items"; "$S" get /yes >/dev/null; sleep 2; "$S" get /yes >/dev/null;; esac; done; echo "$c"; exit 0'
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; sleep 4; printf "%s,%s" "'"${CAM:-176,8,128,150,0.0,-0.5}"'" "'"$RANDOM$$"'" > /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp && mv /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp /mnt/c/Users/vjeux/OpenplanetNext/cam.txt; for t in 1 2 3 4 5 6; do rm -f /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv; printf "p%s%s" "$t" "$RANDOM" > /mnt/c/Users/vjeux/OpenplanetNext/probe.txt; sleep 5; [ -s /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv ] && break; done; echo "--- game kept ($("$S" get /ctx)):"; grep -v "^item.*-1000\|^block\|^kind" /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv | grep -v "^item.*Nadeo"; /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -ExecutionPolicy Bypass -File C:\\Users\\vjeux\\shotdpi.ps1 C:\\Users\\vjeux\\itemtest.png >/dev/null; grep "CAMERA" /mnt/c/Users/vjeux/OpenplanetNext/Openplanet.log | tail -1; exit 0'
/home/vjeux/bin/wsx pull /mnt/c/Users/vjeux/itemtest.png /tmp/itemtest.png 2>&1 | grep -v chunk | tail -1
