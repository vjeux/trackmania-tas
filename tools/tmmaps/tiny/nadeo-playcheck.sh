#!/bin/sh
# nadeo-playcheck.sh MAP_UID [OUT.png]
# Runs ON THE RENDER BOX (WSL). Plays the map Nadeo Services STORES for a uid
# — the same bytes the club-campaign menu downloads — through the title API
# (PlayMap with the signed CDN url the core /file route redirects to), waits
# for the playground, and screenshots it. Proves "the published map loads and
# plays" without driving the club menus by hand.
set -eu
UID_=$1; OUT=${2:-/mnt/c/Users/vjeux/playcheck.png}
ST=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter
S="$HOME/trackmania-tas/tools/target/release/shootctl"
"$S" get "/nadeotoken?aud=NadeoServices" >/dev/null
CORE=$(cat $ST/token-NadeoServices.txt)
REC=$(curl -s -H "Authorization: $CORE" "https://prod.trackmania.core.nadeo.online/maps/?mapUidList=$UID_")
FILE=$(echo "$REC" | grep -o '"fileUrl":"[^"]*"' | head -1 | cut -d'"' -f4)
NAME=$(echo "$REC" | grep -o '"name":"[^"]*"' | head -1 | cut -d'"' -f4)
[ -n "$FILE" ] || { echo "no record for $UID_"; exit 2; }
CDN=$(curl -s -o /dev/null -w '%{redirect_url}' -H "Authorization: $CORE" "$FILE")
echo "map \"$NAME\" file $FILE -> $(echo "$CDN" | cut -c1-70)..."
# back to the menu
seen=0; for k in 1 2 3 4 5 6 7 8; do c=$("$S" get /ctx); case "$c" in *\"ctx\":0*\"dialog\":null*) break;; *FrameDialogSaveAs*) "$S" get /dismiss >/dev/null; seen=1;; *FrameAskYesNo*) if [ $seen = 1 ]; then "$S" get /yes >/dev/null; else "$S" get /no >/dev/null; fi;; *\"dialog\":null*) "$S" get /back >/dev/null;; *) "$S" get /dismiss >/dev/null;; esac; sleep 2; done
printf "%s" "$CDN" > $ST/editmap.txt
echo "playmap: $("$S" get '/playmap?mode=' | cut -c1-80)"
for i in $(seq 1 24); do sleep 5; c=$("$S" get /ctx); case "$c" in *\"playground\":true*) break;; esac; done
echo "ctx after $((i*5)) s: $c"
sleep 8
/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -ExecutionPolicy Bypass -File C:\\Users\\vjeux\\shotdpi.ps1 "$(echo "$OUT" | sed 's|/mnt/c/|C:\\|; s|/|\\|g')" >/dev/null
ls -la "$OUT" | awk '{print $5, $9}'
