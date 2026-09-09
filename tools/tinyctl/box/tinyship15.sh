#!/bin/sh
# tinyship15.sh NN TIME [CRF] — cut one ship15 lap clip to mp4 and publish it the README way
# (inline user-attachments player, registered in the videos-v1 release body, anonymous gate),
# from the render box with the browser cookie. Detached by the caller; writes
#   /mnt/c/Users/vjeux/tinyvid/ship15/NN.ship.log   (everything)
#   /mnt/c/Users/vjeux/tinyvid/ship15/NN.ship.done  (URL <url> | FAILED <why>)
export PATH=/home/vjeux/bin:/usr/local/bin:/usr/bin:/bin
NN=$1; T=$2; CRF=${3:-19}
V=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Tiny/videos
O=/mnt/c/Users/vjeux/tinyvid/ship15
CLIP=/home/vjeux/trackmania-tas/tools/target/release/clip
mkdir -p "$O"
LOG="$O/$NN.ship.log"; DONE="$O/$NN.ship.done"
rm -f "$DONE"
{
  echo "== $(date -u +%FT%TZ) tinyship15 $NN $T crf $CRF"
  WEBM="$V/$NN-ghost-$T-ship15.webm"
  MP4="$O/$NN-ghost-$T-ship15.mp4"
  [ -f "$WEBM" ] || { echo "FAILED no such clip $WEBM" > "$DONE"; exit 1; }
  if [ ! -f "$MP4" ] || [ "$WEBM" -nt "$MP4" ] || [ "$(cat "$MP4.crf" 2>/dev/null)" != "$CRF" ]; then
    rm -f "$MP4"
    $CLIP cut "$WEBM" "$MP4" --crf "$CRF" || { echo "FAILED cut rc=$?" > "$DONE"; exit 1; }
    echo "$CRF" > "$MP4.crf"
  fi
  SZ=$(stat -c %s "$MP4")
  echo "mp4 $MP4 $SZ bytes"
  if [ "$SZ" -gt 99000000 ]; then echo "FAILED mp4 too big ($SZ) — re-run with a higher crf" > "$DONE"; exit 1; fi
  GH_COOKIE="$(tr -d '\r\n' < /home/vjeux/.gh-upload/cookie)"; export GH_COOKIE
  # a cookie that answers 302 -> /login is dead: STOP, do not retry
  code=$(curl -s -o /dev/null -w '%{http_code}' -b "$GH_COOKIE" -H 'user-agent: Mozilla/5.0' https://github.com/vjeux/trackmania-tas/edit/main/README.md)
  echo "cookie probe: HTTP $code"
  case "$code" in 200) ;; *) echo "FAILED cookie probe HTTP $code (302 = logged out) — STOP" > "$DONE"; exit 1;; esac
  mkdir -p "/tmp/tinyship/tiny-summer-2026-$NN"
  $CLIP ship "$MP4" "/tmp/tinyship/tiny-summer-2026-$NN" --no-mirror > "$O/$NN.ship.out" 2>&1
  rc=$?
  cat "$O/$NN.ship.out"
  URL=$(grep -o 'https://github.com/user-attachments/assets/[0-9a-f-]*' "$O/$NN.ship.out" | head -1)
  if [ $rc -eq 0 ] && [ -n "$URL" ]; then echo "URL $URL" > "$DONE"; else echo "FAILED ship rc=$rc $URL" > "$DONE"; fi
  echo "== $(date -u +%FT%TZ) done: $(cat "$DONE")"
} >> "$LOG" 2>&1
