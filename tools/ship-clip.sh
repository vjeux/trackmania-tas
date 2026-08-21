#!/usr/bin/env bash
# ship-clip.sh — publish one rendered clip so that a LOGGED-OUT VISITOR can watch it.
#
#   tools/ship-clip.sh <file.mp4> <map-dir> [release-asset-name]
#
# Steps, in order, each one refusing rather than warning:
#   1. settle + probe the local file
#   2. upload the full-quality original to the videos-v1 release (download mirror)
#   3. upload to GitHub's user-attachments store (inline player)  -> asset URL
#   4. REGISTER the asset URL in the release body listing         <- what makes it public
#   5. ANONYMOUS GATE: fetch the URL with no credential at all; require 200 and
#      playable bytes. This is the only step that decides whether the clip is published.
#
# Why step 4 exists: a pushed commit does NOT authorise an attachment for public
# serving. Only a reference in content GitHub renders at save time does. We shipped
# 19 clips before learning this and 18 of them were 404 to everyone but us.
# See VIDEO-UPLOAD-NOTES.md traps 0 and 1.
#
# Why step 5 uses a scrubbed environment: every check we ran for a whole night
# carried a session cookie, so the failure was invisible. A gate that runs with
# credentials is not a gate.
set -euo pipefail

REPO="${REPO:-vjeux/trackmania-tas}"
RELEASE="${RELEASE:-videos-v1}"
GHVID="${GHVID:-$HOME/tas-test/.ghvid/ghvid.sh}"
ROOT="${ROOT:-$HOME/tas-test}"

FILE="${1:?usage: ship-clip.sh <file.mp4> <map-dir> [release-asset-name]}"
MAPDIR="${2:?usage: ship-clip.sh <file.mp4> <map-dir> [release-asset-name]}"
ASSET_NAME="${3:-$(basename "$FILE")}"

die() { echo "ship: $*" >&2; exit 1; }

# --- 1. the local file, settled and playable --------------------------------
[ -f "$FILE" ] || die "no such file: $FILE"
a=0; b=1; while [ "$a" != "$b" ]; do a=$(stat -f%z "$FILE"); sleep 1; b=$(stat -f%z "$FILE"); done
LOCAL_DUR="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$FILE" 2>/dev/null || true)"
[ -n "$LOCAL_DUR" ] || die "local file does not probe as playable: $FILE"
echo "ship: local $FILE  ${b} bytes  ${LOCAL_DUR}s"

# --- 2. the stable download mirror ------------------------------------------
cp "$FILE" "/tmp/$ASSET_NAME" 2>/dev/null || true
gh release upload "$RELEASE" "/tmp/$ASSET_NAME" -R "$REPO" --clobber >/dev/null \
  || die "release upload failed"
echo "ship: release asset $ASSET_NAME uploaded"

# --- 3. the inline player ----------------------------------------------------
URL="$("$GHVID" "$FILE")" || die "attachment upload failed"
case "$URL" in https://github.com/user-attachments/assets/*) ;; *) die "unexpected asset url: $URL";; esac
echo "ship: asset $URL"

# --- 4. authorise it for the public -----------------------------------------
BODY="$(gh release view "$RELEASE" -R "$REPO" --json body -q .body)"
if ! printf '%s' "$BODY" | grep -qF "$URL"; then
  TMP="$(mktemp)"
  if printf '%s' "$BODY" | grep -q '</details>'; then
    # insert before the closing tag of the listing block
    printf '%s' "$BODY" | awk -v u="$URL" -v n="$(basename "$MAPDIR")" \
      '{ if ($0 ~ /<\/details>/ && !done) { print n ": " u; print ""; done=1 } print }' > "$TMP"
  else
    { printf '%s\n\n' "$BODY"; printf '%s: %s\n' "$(basename "$MAPDIR")" "$URL"; } > "$TMP"
  fi
  gh release edit "$RELEASE" -R "$REPO" --notes-file "$TMP" >/dev/null || die "release body edit failed"
  rm -f "$TMP"
  echo "ship: registered in the $RELEASE body (this is what makes it public)"
fi

# --- 5. THE GATE: what a logged-out visitor gets -----------------------------
# env -i so no cookie jar, no GH_TOKEN, no netrc can leak into the check.
OUT="/tmp/anon-$$.mp4"
CODE="$(env -i /usr/bin/curl -s -L --retry 3 --max-time 300 -o "$OUT" -w '%{http_code}' "$URL" || true)"
[ "$CODE" = "200" ] || die "ANONYMOUS GATE FAILED: http $CODE for $URL — NOT published"
ANON_BYTES="$(stat -f%z "$OUT")"
ANON_DUR="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$OUT" 2>/dev/null || true)"
[ -n "$ANON_DUR" ] || die "ANONYMOUS GATE FAILED: fetched $ANON_BYTES bytes that do not probe — NOT published"
rm -f "$OUT"
echo "ship: ANONYMOUS GATE PASSED  http 200  ${ANON_BYTES} bytes  ${ANON_DUR}s"

echo
echo "PUBLISHED  $URL"
echo "Embed it on its own line in $MAPDIR/README.md, under the caption line, then"
echo "commit only that README. Re-run the gate after the push:"
echo "  env -i /usr/bin/curl -s -o /dev/null -w '%{http_code}\\n' -L $URL"
