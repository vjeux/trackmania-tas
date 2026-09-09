#!/bin/sh
# tiny-publish.sh — publish ONE finished, overlaid clip to the tiny page, using
# nothing but a POSIX shell, curl, gh and ffprobe.
#
#   ./tiny-publish.sh <clip.mp4> [path/to/tiny/README.md]
#
# This is `clip ship` + the page swap, in shell, for a machine that cannot run
# our binaries — vjeux's Mac, where the antivirus kills freshly built ones and
# where the browser session that GitHub accepts actually lives. Same five steps,
# same refusals, same order:
#
#   1. the clip must CARRY THE CONTROLS OVERLAY (its `comment` tag), or stop
#   2. upload it to GitHub's user-attachments store  -> the inline player URL
#   3. REGISTER that URL in the videos-v1 release body  <- what makes it public
#   4. ANONYMOUS GATE: fetch it back with no credential; require 200 + bytes
#   5. swap the map's line on the page, drop its "video pending" line, push
#
# Needs, on that machine: ghvid.sh + a live `Cookie:` header (step 2), `gh`
# logged in (step 3), ffprobe (step 1), and a checkout of the repo (step 5).
#
#   GH_COOKIE_FILE   default ~/.gh-upload/cookie   (READ ONLY. Never written.)
#   GHVID            default ~/bin/ghvid.sh
#   REPO             default vjeux/trackmania-tas
#   RELEASE          default videos-v1
#   NO_PUSH=1        do everything but the git push
#   DRY=1            print what would happen; touch nothing
set -eu

MP4=${1:?usage: tiny-publish.sh <clip.mp4> [tiny/README.md]}
PAGE=${2:-tiny/README.md}
GH_COOKIE_FILE=${GH_COOKIE_FILE:-$HOME/.gh-upload/cookie}
GHVID=${GHVID:-$HOME/bin/ghvid.sh}
REPO=${REPO:-vjeux/trackmania-tas}
RELEASE=${RELEASE:-videos-v1}
FFPROBE=${FFPROBE:-ffprobe}

base=$(basename "$MP4")
nn=$(echo "$base" | cut -c1-2)
time=$(echo "$base" | sed -n 's/^[0-9][0-9]-ghost-\([0-9][0-9]*\.[0-9][0-9]*\)-.*/\1/p')
[ -n "$time" ] || { echo "tiny-publish: $base is not NN-ghost-<time>-<suffix>.mp4" >&2; exit 2; }

# The page title of this map: 21-25 go by their countries (vjeux, 2026-09-09).
case "$nn" in
  21) title="Tiny Argentina 2026" ;;
  22) title="Tiny Saudi Arabia 2026" ;;
  23) title="Tiny Norway 2026" ;;
  24) title="Tiny Poland 2026" ;;
  25) title="Tiny Japan 2026" ;;
  *)  title="Tiny Summer 2026 - $nn" ;;
esac
slug=$(echo "$title" | tr 'A-Z' 'a-z' | sed 's/ - /-/; s/ /-/g')

echo "== $base  ->  $title  ($time)"

# --- 1. THE OVERLAY, or nothing ---------------------------------------------
mark=$("$FFPROBE" -v error -show_entries format_tags=comment -of default=nw=1:nk=1 "$MP4" 2>/dev/null || true)
case "$mark" in
  "tas-overlay v1 "*) echo "   overlay: $mark" ;;
  *) echo "tiny-publish: NO CONTROLS OVERLAY on $base (comment tag: ${mark:-none}). Every published clip carries it; make it with 'clip cut <webm> <mp4> --ghost <run.Ghost.Gbx>' on a machine that can run the tools." >&2; exit 3 ;;
esac
bytes=$(wc -c < "$MP4" | tr -d ' ')
[ "$bytes" -le 99000000 ] || { echo "tiny-publish: $base is $bytes bytes; the inline player refuses over 100 MB" >&2; exit 4; }
echo "   $bytes bytes"
[ -z "${DRY:-}" ] || { echo "   DRY: would upload, register, gate and swap the page line"; exit 0; }

# --- 2. the inline player ----------------------------------------------------
GH_COOKIE="$(tr -d '\r\n' < "$GH_COOKIE_FILE")"; export GH_COOKIE
url=$("$GHVID" "$MP4")
case "$url" in
  https://github.com/user-attachments/assets/?*) echo "   asset $url" ;;
  *) echo "tiny-publish: the uploader did not return an asset url: $url" >&2; exit 5 ;;
esac

# --- 3. authorise it for the public -----------------------------------------
# A pushed commit does NOT make an attachment public; a reference in content
# GitHub re-renders at save time does. 19 clips were shipped before that was
# learned and 18 were 404 to everybody but their author.
body=$(gh release view "$RELEASE" -R "$REPO" --json body -q .body)
case "$body" in
  *"$url"*) echo "   already registered" ;;
  *)
    tmp=$(mktemp)
    printf '%s\n' "$body" | awk -v line="$slug: $url" '
      !done && /<\/details>/ { print line; print ""; done = 1 }
      { print }
      END { if (!done) { print ""; print line } }
    ' > "$tmp"
    gh release edit "$RELEASE" -R "$REPO" --notes-file "$tmp"
    rm -f "$tmp"
    echo "   registered in the $RELEASE body (this is what makes it public)"
    ;;
esac

# --- 4. THE ANONYMOUS GATE ---------------------------------------------------
# env -i: no cookie jar, no token, no netrc. A gate that runs with credentials
# is not a gate. Registration can take ~45 s to propagate, so it retries.
i=1
while [ "$i" -le 10 ]; do
  out=$(mktemp)
  code=$(env -i /usr/bin/curl -s -L --retry 3 --max-time 300 -o "$out" -w '%{http_code}' "$url" || true)
  got=$(wc -c < "$out" | tr -d ' ')
  rm -f "$out"
  if [ "$code" = "200" ] && [ "$got" -gt 100000 ]; then
    echo "   ANONYMOUS GATE PASSED  http 200  $got bytes"
    break
  fi
  echo "   gate attempt $i: http $code, $got bytes — not public yet, retrying"
  i=$((i + 1))
  [ "$i" -le 10 ] && sleep 15
done
if [ "$i" -gt 10 ]; then
  echo "tiny-publish: the asset is uploaded and registered but the gate never turned 200 — do NOT re-upload; re-run the gate later:" >&2
  echo "  env -i /usr/bin/curl -s -o /dev/null -w '%{http_code}\\n' -L $url" >&2
  exit 6
fi

# --- 5. the page -------------------------------------------------------------
[ -f "$PAGE" ] || { echo "tiny-publish: no page at $PAGE (pass it as the second argument)" >&2; exit 7; }
tmp=$(mktemp)
awk -v title="$title" -v nn="$nn" -v time="$time" -v url="$url" '
  # The map’s block, rewritten: caption, blank, the new video, blank.
  # Everything the old block had between the caption and the next row — its
  # "video pending" line, its previous asset url, the blank lines around them —
  # is swallowed, so the result has the same shape whether the row had a video
  # before or not.
  BEGIN { state = "look" }
  state == "look" && (index($0, "**" title "**") == 1 || index($0, "**Tiny Summer 2026 - " nn "**") == 1) {
    p = index($0, "· ")
    line = (p > 0) ? substr($0, 1, p + 1) " tiny ghost **" time "** (build ship15, controls overlay)" : $0
    sub(/· +/, "· ", line)
    print line; print ""; print url
    state = "swallow"
    next
  }
  state == "swallow" {
    if ($0 == "") next
    if ($0 ~ /^\*latest lap /) next
    if ($0 ~ /^https:\/\/github\.com\/user-attachments\/assets\//) next
    print ""
    state = "done"
  }
  { print }
' "$PAGE" > "$tmp"
if cmp -s "$PAGE" "$tmp"; then
  echo "   page already says this"
  rm -f "$tmp"
else
  mv "$tmp" "$PAGE"
  echo "   page: $title = $time with the controls overlay"
  if [ -z "${NO_PUSH:-}" ]; then
    git -C "$(dirname "$PAGE")/.." add "$(basename "$(dirname "$PAGE")")/$(basename "$PAGE")"
    git -C "$(dirname "$PAGE")/.." commit -q -m "tiny page: $title = $time (build ship15) with the controls overlay ($base)"
    git -C "$(dirname "$PAGE")/.." pull -q --rebase
    git -C "$(dirname "$PAGE")/.." push -q
    echo "   pushed"
  fi
fi
echo "== done: $url"
