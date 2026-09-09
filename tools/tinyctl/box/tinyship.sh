#!/bin/sh
# tinyship.sh MP4 SLUG OUTBASE — publish one FINISHED mp4 the README way (inline
# user-attachments player, registered in the videos-v1 release body, anonymous
# gate), from the render box with the browser cookie. Started detached by
# `tinyctl video --ship`; writes
#   OUTBASE.log   (everything)
#   OUTBASE.done  (URL <url> | PENDING <url> | FAILED <why>)
# PENDING = uploaded AND registered, but the anonymous gate had not turned 200
# within clip ship's ~100 s (a big asset can take an hour; 24 took 80 min on
# 2026-09-09) — `tinyctl shipwatch` keeps probing the URL; NEVER re-upload.
# The ships are SERIALISED (flock): two at once race on the release body's
# read-modify-write and one registration is lost.
# The mp4 arrives finished: `tinyctl video` cut it WITH THE CONTROLS OVERLAY and
# stamped it; `clip ship` here REFUSES a file without that stamp, so nothing
# bare can pass through this script by accident (2026-09-09).
export PATH=/home/vjeux/bin:/usr/local/bin:/usr/bin:/bin
MP4=$1; SLUG=$2; OUT=$3
CLIP=/home/vjeux/trackmania-tas/tools/target/release/clip
LOCK=/home/vjeux/shoot/tinyship.lock
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
  # a cookie that answers 302 -> /login is dead: STOP, do not retry.
  # Through the SAME session state ghvid.sh keeps (GitHub rotates _gh_sess and
  # logs out a client that replays a stale one), never through the credential
  # file, which every tool here only ever READS — see the warning in ghvid.sh.
  STATE=/home/vjeux/.gh-upload/session-state
  [ -s "$STATE" ] && [ ! /home/vjeux/.gh-upload/cookie -nt "$STATE" ] \
    && COOKIEARG="-b $STATE -c $STATE" || COOKIEARG="-b $GH_COOKIE"
  code=$(curl -s -o /dev/null -w '%{http_code}' $COOKIEARG -H 'user-agent: Mozilla/5.0' https://github.com/vjeux/trackmania-tas/edit/main/README.md)
  echo "cookie probe: HTTP $code"
  case "$code" in 200) ;; *) echo "FAILED cookie probe HTTP $code (302 = logged out) — STOP" > "$DONE"; exit 1;; esac
  mkdir -p "/tmp/tinyship/$SLUG"
  # SERIALISED, back to back. The lock is what keeps two ships off one session;
  # there is NO cool-down (COOLDOWN=0 by default, vjeux 16:58Z: "can you just
  # upload them back to back, why a specific delay?"). The first 2026-09-09
  # logout was a stale rotating _gh_sess and a re-login, not a rate limit; set
  # COOLDOWN=N if that is ever disproved.
  echo "waiting for the ship lock $LOCK …"
  flock "$LOCK" sh -c "$CLIP ship \"$MP4\" \"/tmp/tinyship/$SLUG\" --no-mirror; rc=\$?; sleep ${COOLDOWN:-0}; exit \$rc" > "$OUT.out" 2>&1
  rc=$?
  cat "$OUT.out"
  URL=$(grep -o 'https://github.com/user-attachments/assets/[0-9a-f-]*' "$OUT.out" | head -1)
  if [ $rc -eq 0 ] && [ -n "$URL" ]; then echo "URL $URL" > "$DONE"
  elif [ -n "$URL" ] && grep -q "registered in the videos-v1 body\|ANONYMOUS GATE FAILED" "$OUT.out"; then echo "PENDING $URL" > "$DONE"
  else echo "FAILED ship rc=$rc $(grep -m1 'clip:' "$OUT.out")" > "$DONE"; fi
  echo "== $(date -u +%FT%TZ) done: $(cat "$DONE")"
} >> "$LOG" 2>&1
