#!/bin/bash
# usage: bisect.sh lo hi  -> prints alive(1)/dead(0); TINY_* env knobs pass through to the generator
cd /home/vjeux/trackmania-tas-tiny/tools
TINY_ONLY="$1-$2" target/release/mapgeom tiny-assets /tmp/Summer-2026-01.Map.Gbx --out /tmp/Bis.Map.Gbx --library-out /tmp/Bis.zip --catalog tmmaps/tiny/summer-2026-resolved.tsv --footprints tmmaps/tiny/summer-2026-footprints.tsv --nadeo-zip /tmp/Nadeo.zip --empty-template tmmaps/tiny/empty.Item.Gbx --blue-pak /tmp/BlueBay.pak --stadium-pak /tmp/current-Stadium.pak 2>&1 | grep -E "TINY_ONLY|panick|NOT" >&2
# leave the editor first: the game holds the open map file, and a push into a held file fails silently
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; /mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq Trackmania.exe" /NH | tr -d "\r" | grep -q Trackmania || "$S" launch 180 >/dev/null 2>&1; seen=0; for k in 1 2 3 4 5 6; do c=$("$S" get /ctx); case "$c" in *\"ctx\":0*\"dialog\":null*) break;; *FrameDialogSaveAs*) "$S" get /dismiss >/dev/null; seen=1;; *FrameAskYesNo*) if [ $seen = 1 ]; then "$S" get /yes >/dev/null; else "$S" get /no >/dev/null; fi;; *\"dialog\":null*) "$S" get /back >/dev/null;; *) "$S" get /dismiss >/dev/null;; esac; sleep 2; done; exit 0' >/dev/null 2>&1
want=$(md5sum /tmp/Bis.Map.Gbx | cut -d' ' -f1)
/home/vjeux/bin/wsx push /tmp/Bis.Map.Gbx "/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/Bis.Map.Gbx" >/dev/null 2>&1
got=$(/home/vjeux/bin/wsx sh 'md5sum /mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/Bis.Map.Gbx | cut -d" " -f1' 2>/dev/null | tail -1)
if [ "$want" != "$got" ]; then echo "PUSH FAILED (md5 $got != $want)"; exit 2; fi
/home/vjeux/bin/wsx sh 'S="$HOME/trackmania-tas/tools/target/release/shootctl"; printf "%s" "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/Bis.Map.Gbx" > /mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter/editmap.txt; "$S" get /editmap >/dev/null; sleep 60; /mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq Trackmania.exe" /NH | tr -d "\r" | grep -c Trackmania; exit 0' 2>/dev/null | tail -1
