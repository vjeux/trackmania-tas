#!/bin/bash
# shootmap.sh MAP.Map.Gbx OUT.png "x,y,z,dist,h,v" [NAME]
# Pushes the map to the render box as _shoot/NAME.Map.Gbx, opens it in the
# editor (launching the game if needed, answering the dialogs), aims the
# free camera through the probe plugin's cam.txt, screenshots, pulls OUT.png,
# and prints what the game kept (items) so a silently dropped item shows.
# ~60-120 s. Requires the WhiteStick bridge (~/bin/wsx).
set -u
MAP=$1; OUT=$2; CAM=$3; NAME=${4:-Shot}
W='/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania'
WSX=/home/vjeux/bin/wsx
# game up?
$WSX sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; if ! timeout 8 "$S" get /ping >/dev/null 2>&1; then echo "game not responding: relaunching"; /mnt/c/Windows/System32/taskkill.exe /F /IM Trackmania.exe >/dev/null 2>&1; sleep 4; "$S" launch 180 2>&1 | tail -1; fi; exit 0' 2>&1 | grep -v "^$" | tail -2
# push (md5-checked by wsx)
$WSX push "$MAP" "$W/Maps/_shoot/$NAME.Map.Gbx" 2>&1 | grep -v chunk | tail -1
# back to the menu, then open the map in the editor
$WSX sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; seen=0; for k in 1 2 3 4 5 6 7 8; do c=$("$S" get /ctx); case "$c" in *\"ctx\":0*\"dialog\":null*) break;; *FrameDialogSaveAs*) "$S" get /dismiss >/dev/null; seen=1;; *FrameAskYesNo*) if [ $seen = 1 ]; then "$S" get /yes >/dev/null; else "$S" get /no >/dev/null; fi;; *\"dialog\":null*) "$S" get /back >/dev/null;; *) "$S" get /dismiss >/dev/null;; esac; sleep 2; done; rm -f /mnt/c/Users/vjeux/OpenplanetNext/probe.txt /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv; printf "%s" "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/'"$NAME"'.Map.Gbx" > /mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter/editmap.txt; "$S" get /editmap >/dev/null; for i in 1 2 3 4 5 6 7 8 9 10; do sleep 7; c=$("$S" get /ctx); case "$c" in *\"ctx\":1*) break;; *FrameAskYesNo*) echo "DIALOG: missing items"; "$S" get /yes >/dev/null; sleep 2; "$S" get /yes >/dev/null;; esac; done; echo "ctx: $c"; exit 0' 2>&1 | tail -2
# camera + probe + screenshot
$WSX sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; sleep 4; printf "%s,%s" "'"$CAM"'" "'"$RANDOM$$"'" > /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp && mv /mnt/c/Users/vjeux/OpenplanetNext/cam.tmp /mnt/c/Users/vjeux/OpenplanetNext/cam.txt; for t in 1 2 3 4 5 6; do rm -f /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv; printf "p%s%s" "$t" "$RANDOM" > /mnt/c/Users/vjeux/OpenplanetNext/probe.txt; sleep 5; [ -s /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv ] && break; done; echo "--- game kept:"; grep -v "^item.*-1000\|^item.*-900\|^block\|^kind" /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv | head -40; grep "^copper" /mnt/c/Users/vjeux/OpenplanetNext/probe-out.tsv; sleep 3; /mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -ExecutionPolicy Bypass -File C:\\Users\\vjeux\\shotdpi.ps1 C:\\Users\\vjeux\\shotmap.png >/dev/null; exit 0' 2>&1 | tail -45
$WSX pull /mnt/c/Users/vjeux/shotmap.png "$OUT" 2>&1 | grep -v chunk | tail -1
ls -la "$OUT" 2>/dev/null | awk '{print $5, $9}'
