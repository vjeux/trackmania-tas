#!/bin/bash
# usage: loadmap.sh LOCAL.Map.Gbx -> pushes as _shoot/Bis.Map.Gbx (verified), loads in editor, prints alive(1)/dead(0)
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; /mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq Trackmania.exe" /NH | tr -d "\r" | grep -q Trackmania || "$S" launch 180 >/dev/null 2>&1; seen=0; for k in 1 2 3 4 5 6; do c=$("$S" get /ctx); case "$c" in *\"ctx\":0*\"dialog\":null*) break;; *FrameDialogSaveAs*) "$S" get /dismiss >/dev/null; seen=1;; *FrameAskYesNo*) if [ $seen = 1 ]; then "$S" get /yes >/dev/null; else "$S" get /no >/dev/null; fi;; *\"dialog\":null*) "$S" get /back >/dev/null;; *) "$S" get /dismiss >/dev/null;; esac; sleep 2; done; exit 0' >/dev/null 2>&1
want=$(md5sum "$1" | cut -d' ' -f1)
/home/vjeux/bin/wsx push "$1" "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/Bis.Map.Gbx" >/dev/null 2>&1
got=$(/home/vjeux/bin/wsx sh 'md5sum /mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/Bis.Map.Gbx | cut -d" " -f1' 2>/dev/null | tail -1)
[ "$want" = "$got" ] || { echo "PUSH FAILED"; exit 2; }
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; printf "%s" "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/Bis.Map.Gbx" > /mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter/editmap.txt; "$S" get /editmap >/dev/null; sleep 60; /mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq Trackmania.exe" /NH | tr -d "\r" | grep -c Trackmania; exit 0' 2>/dev/null | tail -1
