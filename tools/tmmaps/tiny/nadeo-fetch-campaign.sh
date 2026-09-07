#!/bin/sh
# nadeo-fetch-campaign.sh "Campaign name" OUTDIR
# Runs ON THE RENDER BOX (WSL). Downloads every map of an OFFICIAL Nadeo
# campaign (live: /api/token/campaign/official lists them with their
# playlists; core: /maps/?mapUidList= gives the file urls; the file route
# redirects to a signed CDN url). Files land as OUTDIR/NN-<name>.Map.Gbx.
# Auth = the game's own session through the GhostShooter /nadeotoken route.
set -eu
CNAME=$1; OUT=$2
mkdir -p "$OUT"
ST=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter
S="$HOME/trackmania-tas/tools/target/release/shootctl"
"$S" get "/nadeotoken?aud=NadeoServices" >/dev/null
"$S" get "/nadeotoken?aud=NadeoLiveServices" >/dev/null
CORE=$(cat $ST/token-NadeoServices.txt); LIVE=$(cat $ST/token-NadeoLiveServices.txt)
ALL=$(curl -s -H "Authorization: $LIVE" "https://live-services.trackmania.nadeo.live/api/token/campaign/official?length=20&offset=0")
# the campaign object = from its "name" to the next campaign's "name"
# (playlist entries carry id/position/mapUid only, never a name)
CAMP=$(echo "$ALL" | awk -v n="\"name\":\"$CNAME\"" '{ i = index($0, n); if (!i) exit; s = substr($0, i + length(n)); j = index(s, "\"name\":\""); print (j ? substr(s, 1, j) : s) }')
[ -n "$CAMP" ] || { echo "no official campaign named $CNAME; have: $(echo "$ALL" | grep -o '"name":"[^"]*"' | sort -u | tr '\n' ' ')"; exit 2; }
UIDS=$(echo "$CAMP" | grep -o '"mapUid":"[^"]*"' | cut -d'"' -f4)
echo "$CNAME: $(echo "$UIDS" | wc -l) maps"
LIST=$(echo "$UIDS" | tr '\n' ',' | sed 's/,$//')
REC=$(curl -s -H "Authorization: $CORE" "https://prod.trackmania.core.nadeo.online/maps/?mapUidList=$LIST")
i=0
for u in $UIDS; do
  i=$((i+1))
  R=$(echo "$REC" | sed 's/{"author"/\n{"author"/g' | grep "\"mapUid\":\"$u\"" | head -1)
  URL=$(echo "$R" | grep -o '"fileUrl":"[^"]*"' | cut -d'"' -f4)
  NAME=$(echo "$R" | grep -o '"name":"[^"]*"' | head -1 | cut -d'"' -f4 | sed 's/\$[0-9a-fA-F]\{3\}//g; s/\$[a-zA-Z]//g; s/[^A-Za-z0-9 _-]//g; s/ /-/g')
  F="$OUT/$(printf '%02d' $i)-$NAME.Map.Gbx"
  if [ -s "$F" ]; then echo "have $F"; continue; fi
  curl -s -L -H "Authorization: $CORE" -o "$F" "$URL"
  echo "$F $(stat -c %s "$F") bytes uid $u"
done
