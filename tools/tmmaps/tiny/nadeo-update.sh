#!/bin/sh
# nadeo-update.sh MAP.Map.Gbx "Map name"
# Runs ON THE RENDER BOX (WSL). Re-uploads a map that is already on Nadeo
# Services under the same uid: looks the record up by uid, POSTs the new
# file and parameters to /maps/{mapId} (the update route — a second POST to
# /maps/ with the same uid keeps the old collectionName), then reads the
# record back and prints what changed (file url, timestamps, collection).
# Auth = the game's own session through the GhostShooter /nadeotoken route
# (see nadeo-publish.sh).
set -eu
MAP=$1; NAME=$2
ST=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter
S="$HOME/trackmania-tas/tools/target/release/shootctl"
"$S" get "/nadeotoken?aud=NadeoServices" >/dev/null
CORE=$(cat $ST/token-NadeoServices.txt)
xml() { strings -n 6 "$MAP" | grep -o "<$1 [^>]*>" | head -1; }
UID_=$(xml ident | grep -o 'uid="[^"]*"' | cut -d'"' -f2)
T=$(xml times); AT=$(echo "$T" | grep -o 'authortime="[0-9]*"' | grep -o '[0-9]*'); G=$(echo "$T" | grep -o 'gold="[0-9]*"' | grep -o '[0-9]*'); SV=$(echo "$T" | grep -o 'silver="[0-9]*"' | grep -o '[0-9]*'); BR=$(echo "$T" | grep -o 'bronze="[0-9]*"' | grep -o '[0-9]*')
ENV=$(xml desc | grep -o 'envir="[^"]*"' | cut -d'"' -f2)
REC=$(curl -s -H "Authorization: $CORE" "https://prod.trackmania.core.nadeo.online/maps/?mapUidList=$UID_")
echo "before: $REC" | cut -c1-600
MAPID=$(echo "$REC" | grep -o '"mapId":"[^"]*"' | head -1 | cut -d'"' -f4)
ME=$(echo "$REC" | grep -o '"author":"[^"]*"' | head -1 | cut -d'"' -f4)
[ -n "$MAPID" ] || { echo "no record for uid $UID_"; exit 2; }
P="{\"isPlayable\":true,\"author\":\"$ME\",\"authorScore\":$AT,\"bronzeScore\":$BR,\"silverScore\":$SV,\"goldScore\":$G,\"collectionName\":\"$ENV\",\"mapStyle\":\"\",\"mapType\":\"TrackMania\\\\TM_Race\",\"name\":\"$NAME\",\"mapUid\":\"$UID_\"}"
echo "update $MAPID ($UID_) as $ME ($ENV, AT $AT) md5 $(md5sum "$MAP" | cut -c1-32)"
curl -s -w "\n%{http_code}\n" -H "Authorization: $CORE" -F "nadeoservices-core-parameters=$P;type=application/json" -F "data=@$MAP;type=application/octet-stream;filename=$NAME.Map.Gbx" "https://prod.trackmania.core.nadeo.online/maps/$MAPID" | tail -c 600
echo
AFTER=$(curl -s -H "Authorization: $CORE" "https://prod.trackmania.core.nadeo.online/maps/?mapUidList=$UID_")
echo "after: $AFTER" | cut -c1-600
URL=$(echo "$AFTER" | grep -o '"fileUrl":"[^"]*"' | head -1 | cut -d'"' -f4)
[ -n "$URL" ] && { curl -s -L -H "Authorization: $CORE" -o /tmp/nadeo-readback.Map.Gbx "$URL"; echo "stored md5 $(md5sum /tmp/nadeo-readback.Map.Gbx | cut -c1-32) ($(stat -c %s /tmp/nadeo-readback.Map.Gbx) bytes)"; }
