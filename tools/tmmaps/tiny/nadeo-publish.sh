#!/bin/sh
# nadeo-publish.sh MAP.Map.Gbx "Map name" CLUB_ID CAMPAIGN_ID|new "Campaign name"
# Runs ON THE RENDER BOX (WSL): uploads a map to Nadeo Services and puts it in
# a club campaign, authenticated with the GAME's own session. Tokens come from
# the GhostShooter plugin route /nadeotoken?aud=… (writes
# PluginStorage/GhostShooter/token-<aud>.txt = the Authorization header).
# Ubisoft's password login (v3/profiles/sessions) answers 403 errorCode 4 for
# this account/app id, so the game session is the only route (2026-09-06).
# Verified: map e22c57a6 / uid Tin2buNzfsVlp2NF2oWtHM3729d in club 43788
# campaign 155555 "Tiny Campaign"; the stored file is byte-identical and
# opens in a playground through shootctl probe --how play.
set -eu
MAP=$1; NAME=$2; CLUB=$3; CAMP=$4; CNAME=${5:-Tiny Campaign}
ST=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter
S="$HOME/trackmania-tas/tools/target/release/shootctl"
"$S" get "/nadeotoken?aud=NadeoServices" >/dev/null
"$S" get "/nadeotoken?aud=NadeoLiveServices" >/dev/null
CORE=$(cat $ST/token-NadeoServices.txt); LIVE=$(cat $ST/token-NadeoLiveServices.txt)
ME=$(curl -s -H "Authorization: $LIVE" "https://live-services.trackmania.nadeo.live/api/token/club/mine?length=1&offset=0" | grep -o '"authorAccountId":"[^"]*"' | head -1 | cut -d'"' -f4)
xml() { strings -n 6 "$MAP" | grep -o "<$1 [^>]*>" | head -1; }
UID_=$(xml ident | grep -o 'uid="[^"]*"' | cut -d'"' -f2)
T=$(xml times); AT=$(echo "$T" | grep -o 'authortime="[0-9]*"' | grep -o '[0-9]*'); G=$(echo "$T" | grep -o 'gold="[0-9]*"' | grep -o '[0-9]*'); SV=$(echo "$T" | grep -o 'silver="[0-9]*"' | grep -o '[0-9]*'); BR=$(echo "$T" | grep -o 'bronze="[0-9]*"' | grep -o '[0-9]*')
ENV=$(xml desc | grep -o 'envir="[^"]*"' | cut -d'"' -f2)
P="{\"isPlayable\":true,\"author\":\"$ME\",\"authorScore\":$AT,\"bronzeScore\":$BR,\"silverScore\":$SV,\"goldScore\":$G,\"collectionName\":\"$ENV\",\"mapStyle\":\"\",\"mapType\":\"TrackMania\\\\TM_Race\",\"name\":\"$NAME\",\"mapUid\":\"$UID_\"}"
echo "upload $UID_ as $ME ($ENV, AT $AT)"
curl -s -w "\n%{http_code}\n" -H "Authorization: $CORE" -F "nadeoservices-core-parameters=$P;type=application/json" -F "data=@$MAP;type=application/octet-stream;filename=$NAME.Map.Gbx" "https://prod.trackmania.core.nadeo.online/maps/" | tail -c 400
if [ "$CAMP" = new ]; then
  CAMP=$(curl -s -X POST -H "Authorization: $LIVE" -H "Content-Type: application/json" -d "{\"name\":\"$CNAME\",\"description\":\"\",\"color\":\"\",\"useCase\":2,\"publicationTimestamp\":$(date +%s),\"mediaUrl\":\"\",\"video\":false}" "https://live-services.trackmania.nadeo.live/api/token/club/$CLUB/campaign/create" | grep -o '"campaignId":[0-9]*' | head -1 | grep -o '[0-9]*')
  echo "created campaign $CAMP"
fi
# the edit body's playlist entries are {"mapUid":…,"position":…}; {"id":…} is silently ignored
OLD=$(curl -s -H "Authorization: $LIVE" "https://live-services.trackmania.nadeo.live/api/token/club/$CLUB/campaign/$CAMP" | grep -o '"mapUid":"[^"]*"' | cut -d'"' -f4 | grep -v "^$UID_$")
PL=""; i=0; for u in $OLD $UID_; do PL="$PL${PL:+,}{\"mapUid\":\"$u\",\"position\":$i}"; i=$((i+1)); done
curl -s -w "\n%{http_code}\n" -X POST -H "Authorization: $LIVE" -H "Content-Type: application/json" -d "{\"name\":\"$CNAME\",\"playlist\":[$PL]}" "https://live-services.trackmania.nadeo.live/api/token/club/$CLUB/campaign/$CAMP/edit" | grep -o '"playlist":\[[^]]*\]\|^[0-9]*$'
