#!/bin/sh
# tinyfile.sh FILE SLUG OUT [CONTENT-TYPE] — publish ONE non-video file (a zipped
# map) as a GitHub user-attachment, the way tinyship.sh publishes a clip: the same
# flock (one client on the session), the same probe, the same page view and the
# same 5-minute COOLDOWN afterwards. Verdict in OUT.done: `URL <href>` or `FAILED …`.
# Clips take priority: this waits on the same lock, so a clip's ship goes first
# whenever both are queued.
export PATH=/home/vjeux/bin:/usr/local/bin:/usr/bin:/bin
FILE=$1; SLUG=$2; OUT=$3; CT=${4:-application/zip}
LOCK=/home/vjeux/shoot/tinyship.lock
COOKIE=/home/vjeux/.gh-upload/cookie
EDIT=https://github.com/vjeux/trackmania-tas/edit/main/README.md
GHVID_COOKIE=/home/vjeux/trackmania-tas/tools/tinyctl/box/ghvid.sh
JAR=/home/vjeux/.gh-upload/session.json
GHSESSION=/home/vjeux/trackmania-tas/tools/target/release/ghsession
LOG="$OUT.log"; DONE="$OUT.done"
mkdir -p "$(dirname "$OUT")"
rm -f "$DONE"
{
  echo "== $(date -u +%FT%TZ) tinyfile $SLUG $FILE ($CT)"
  [ -f "$FILE" ] || { echo "FAILED no such file $FILE" > "$DONE"; exit 1; }
  SZ=$(stat -c %s "$FILE")
  echo "file $FILE $SZ bytes"
  if [ "$SZ" -gt 99000000 ]; then echo "FAILED file too big ($SZ > 99 MB, GitHub's attachment cap)" > "$DONE"; exit 1; fi
  echo "waiting for the ship lock $LOCK …"
  exec 9>"$LOCK"
  flock 9
  echo "holding the ship lock at $(date -u +%FT%TZ)"
  if [ -f "$JAR" ] && [ -x "$GHSESSION" ]; then
    OWN=1
    probe=$("$GHSESSION" status 2>&1); prc=$?
    echo "session probe: $probe"
    [ $prc -eq 0 ] || { echo "FAILED cookie probe (own session, rc=$prc): $probe — STOP" > "$DONE"; exit 1; }
    URL=$("$GHSESSION" upload "$FILE" --content-type "$CT" 2> "$OUT.err"); rc=$?
  else
    OWN=0
    GH_COOKIE="$(tr -d '\r\n' < "$COOKIE")"; export GH_COOKIE
    code=$(curl -s -o /dev/null -w '%{http_code}' -b "$GH_COOKIE" -H 'user-agent: Mozilla/5.0' "$EDIT")
    echo "cookie probe: HTTP $code"
    case "$code" in 200) ;; *) echo "FAILED cookie probe HTTP $code (302 = logged out) — STOP" > "$DONE"; exit 1;; esac
    URL=$(bash "$GHVID_COOKIE" "$FILE" "$CT" 2> "$OUT.err"); rc=$?
    curl -s -o /dev/null -b "$GH_COOKIE" -H 'user-agent: Mozilla/5.0' \
      -H 'accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8' \
      -H 'accept-language: en-US,en;q=0.9' -H 'sec-fetch-site: same-origin' \
      -H 'sec-fetch-mode: navigate' -H 'sec-fetch-dest: document' \
      https://github.com/vjeux/trackmania-tas
  fi
  echo "upload rc=$rc url=$URL"; cat "$OUT.err"
  sleep "${COOLDOWN:-0}"
  case "$URL" in
    https://github.com/user-attachments/*) [ $rc -eq 0 ] && echo "URL $URL" > "$DONE" || echo "FAILED rc=$rc $URL" > "$DONE" ;;
    *) echo "FAILED rc=$rc $(head -c 200 "$OUT.err" | tr '\n' ' ')" > "$DONE" ;;
  esac
  echo "== $(date -u +%FT%TZ) done: $(cat "$DONE")"
} >> "$LOG" 2>&1
