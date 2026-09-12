#!/bin/sh
# tinyship.sh MP4 SLUG OUTBASE — publish one FINISHED mp4 the README way (inline
# user-attachments player, registered in the videos-v1 release body, anonymous
# gate), from the render box with the browser cookie. Started detached by
# `tinyctl video --ship` / `tinyctl shipwatch`; writes
#   OUTBASE.log   (everything)
#   OUTBASE.done  (URL <url> | PENDING <url> | FAILED <why>)
# PENDING = uploaded AND registered, but the anonymous gate had not turned 200
# within clip ship's ~100 s (a big asset can take an hour; 24 took 80 min on
# 2026-09-09) — `tinyctl shipwatch` keeps probing the URL; NEVER re-upload.
#
# ⛔ ONE CLIENT ON THE SESSION AT A TIME — THE LOCK COVERS THE PROBE TOO.
# 2026-09-09: five freshly-copied GitHub sessions died within minutes. The lock
# used to cover only `clip ship`, so when the watcher released a queue every
# waiting script probed github.com in the same second with the same header AND
# wrote the same cookie jar concurrently (12 at once, twice). GitHub answers a
# burst of parallel requests replaying one rotating session by LOGGING IT OUT —
# a Set-Cookie deleting user_session, caught in a jar run. Whatever touches the
# session — probe, uploader, gate — happens inside this lock, one at a time.
#
# The mp4 arrives finished: `tinyctl video` cut it WITH THE CONTROLS OVERLAY and
# stamped it; `clip ship` here REFUSES a file without that stamp, so nothing
# bare can pass through this script by accident.
export PATH=/home/vjeux/bin:/usr/local/bin:/usr/bin:/bin
MP4=$1; SLUG=$2; OUT=$3
CLIP=/home/vjeux/trackmania-tas/tools/target/release/clip
LOCK=/home/vjeux/shoot/tinyship.lock
COOKIE=/home/vjeux/.gh-upload/cookie
STATE=/home/vjeux/.gh-upload/session-state
EDIT=https://github.com/vjeux/trackmania-tas/edit/main/README.md
LOG="$OUT.log"; DONE="$OUT.done"
mkdir -p "$(dirname "$OUT")"
rm -f "$DONE"
{
  echo "== $(date -u +%FT%TZ) tinyship $SLUG $MP4"
  [ -f "$MP4" ] || { echo "FAILED no such file $MP4" > "$DONE"; exit 1; }
  SZ=$(stat -c %s "$MP4")
  echo "mp4 $MP4 $SZ bytes"
  if [ "$SZ" -gt 99000000 ]; then echo "FAILED mp4 too big ($SZ) — re-run with a higher crf" > "$DONE"; exit 1; fi

  # --- everything that touches the GitHub session, under the one lock --------
  echo "waiting for the ship lock $LOCK …"
  exec 9>"$LOCK"
  flock 9
  echo "holding the ship lock at $(date -u +%FT%TZ)"

  JAR=/home/vjeux/.gh-upload/session.json
  GHSESSION=/home/vjeux/trackmania-tas/tools/target/release/ghsession
  if [ -f "$JAR" ] && [ -x "$GHSESSION" ]; then
    # THE UPLOADER'S OWN SESSION (UPLOADER-OWN-SESSION.md, 2026-09-10): the jar
    # at $JAR is the one client of a login seeded once for this purpose; it
    # keeps every cookie GitHub sets and does its own page view after the
    # upload. vjeux's browser cookie is not read on this path.
    OWN=1
    probe=$("$GHSESSION" status 2>&1); prc=$?
    echo "session probe: $probe"
    [ $prc -eq 0 ] || { echo "FAILED cookie probe (own session, rc=$prc): $probe — STOP" > "$DONE"; exit 1; }
    GHVID=/home/vjeux/trackmania-tas/tools/tinyctl/box/ghsession-upload.sh; export GHVID
    unset GH_COOKIE
  else
    OWN=0
    GH_COOKIE="$(tr -d '\r\n' < "$COOKIE")"; export GH_COOKIE
    # The whole browser header, every time — no jar (see ghvid.sh: a jar dropped
    # __Host-user_session_same_site, and an earlier one destroyed the file). The
    # credential file is INPUT and is never written.
    code=$(curl -s -o /dev/null -w '%{http_code}' -b "$GH_COOKIE" -H 'user-agent: Mozilla/5.0' "$EDIT")
    echo "cookie probe: HTTP $code"
    # a cookie that answers 302 -> /login is dead: STOP, do not retry
    case "$code" in 200) ;; *) echo "FAILED cookie probe HTTP $code (302 = logged out) — STOP" > "$DONE"; exit 1;; esac
  fi

  mkdir -p "/tmp/tinyship/$SLUG"
  $CLIP ship "$MP4" "/tmp/tinyship/$SLUG" --no-mirror > "$OUT.out" 2>&1
  rc=$?
  # A NORMAL PAGE VIEW AFTER THE UPLOAD, and then a gap before the next one.
  # Measured 2026-09-09: yesterday one session published 20 clips with ~10 min
  # between them; today back-to-back uploads on the corrected uploader got 5,
  # and earlier sessions 2-3. Spacing is the last variable we have not held
  # steady, so the lock is held through COOLDOWN (default 300 s) and a plain
  # authenticated GET of the repo page follows each upload — what a person
  # doing this by hand would generate. COOLDOWN=0 turns the gap off.
  # (On the own-session path `ghsession upload` made that page view itself.)
  if [ $rc -eq 0 ] && [ "$OWN" = 0 ]; then
    curl -s -o /dev/null -b "$GH_COOKIE" -H 'user-agent: Mozilla/5.0' \
      -H 'accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8' \
      -H 'accept-language: en-US,en;q=0.9' -H 'sec-fetch-site: same-origin' \
      -H 'sec-fetch-mode: navigate' -H 'sec-fetch-dest: document' \
      https://github.com/vjeux/trackmania-tas
  fi
  # vjeux, 2026-09-12 02:22Z: "remove the 5 min and do it back to back" — no gap
  # by default; COOLDOWN=N restores one.
  sleep "${COOLDOWN:-0}"
  cat "$OUT.out"
  URL=$(grep -o 'https://github.com/user-attachments/assets/[0-9a-f-]*' "$OUT.out" | head -1)
  if [ $rc -eq 0 ] && [ -n "$URL" ]; then echo "URL $URL" > "$DONE"
  elif [ -n "$URL" ] && grep -q "registered in the videos-v1 body\|ANONYMOUS GATE FAILED" "$OUT.out"; then echo "PENDING $URL" > "$DONE"
  else echo "FAILED ship rc=$rc $(grep -m1 'clip:' "$OUT.out")" > "$DONE"; fi
  echo "== $(date -u +%FT%TZ) done: $(cat "$DONE")"
} >> "$LOG" 2>&1
