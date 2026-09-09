#!/bin/sh
# tinyship.sh MP4 SLUG OUTBASE — publish one FINISHED mp4 the README way (inline
# user-attachments player, registered in the videos-v1 release body, anonymous
# gate), from the render box with the browser cookie. Started detached by
# `tinyctl video --ship`; writes
#   OUTBASE.log   (everything)
#   OUTBASE.done  (URL <url> | FAILED <why>)
# The mp4 arrives finished: `tinyctl video` cut it WITH THE CONTROLS OVERLAY and
# stamped it; `clip ship` here REFUSES a file without that stamp, so nothing
# bare can pass through this script by accident (2026-09-09).
export PATH=/home/vjeux/bin:/usr/local/bin:/usr/bin:/bin
MP4=$1; SLUG=$2; OUT=$3
CLIP=/home/vjeux/trackmania-tas/tools/target/release/clip
LOG="$OUT.log"; DONE="$OUT.done"
mkdir -p "$(dirname "$OUT")"
rm -f "$DONE"
{
  echo "== $(date -u +%FT%TZ) tinyship $SLUG $MP4"
  [ -f "$MP4" ] || { echo "FAILED no such file $MP4" > "$DONE"; exit 1; }
  SZ=$(stat -c %s "$MP4")
  echo "mp4 $MP4 $SZ bytes"
  if [ "$SZ" -gt 99000000 ]; then echo "FAILED mp4 too big ($SZ) — re-run with a higher crf" > "$DONE"; exit 1; fi
  GH_COOKIE="$(tr -d '\r\n' < /home/vjeux/.gh-upload/cookie)"; export GH_COOKIE
  # a cookie that answers 302 -> /login is dead: STOP, do not retry
  code=$(curl -s -o /dev/null -w '%{http_code}' -b "$GH_COOKIE" -H 'user-agent: Mozilla/5.0' https://github.com/vjeux/trackmania-tas/edit/main/README.md)
  echo "cookie probe: HTTP $code"
  case "$code" in 200) ;; *) echo "FAILED cookie probe HTTP $code (302 = logged out) — STOP" > "$DONE"; exit 1;; esac
  mkdir -p "/tmp/tinyship/$SLUG"
  $CLIP ship "$MP4" "/tmp/tinyship/$SLUG" --no-mirror > "$OUT.out" 2>&1
  rc=$?
  cat "$OUT.out"
  URL=$(grep -o 'https://github.com/user-attachments/assets/[0-9a-f-]*' "$OUT.out" | head -1)
  if [ $rc -eq 0 ] && [ -n "$URL" ]; then echo "URL $URL" > "$DONE"; else echo "FAILED ship rc=$rc $(grep -m1 'clip:' "$OUT.out")" > "$DONE"; fi
  echo "== $(date -u +%FT%TZ) done: $(cat "$DONE")"
} >> "$LOG" 2>&1
