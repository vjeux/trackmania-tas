#!/usr/bin/env bash
# ghvid.sh — upload a file to GitHub's user-attachments store, the way the web
# editor does when you drag a video into a README, and print the embeddable URL.
#
#   ./ghvid.sh <file.mp4> [content-type]
#
# Three requests, mirroring the browser exactly:
#   1. POST /upload/policies/assets      -> S3 policy + asset id + a second token
#   2. POST <s3 url>                     -> the bytes
#   3. PUT  /upload/assets/<id>          -> finalise
#
# Needs GH_COOKIE in the environment (the browser's Cookie header) and
# GH_REPO_ID. Reads a fresh CSRF token off the repo's edit page each run.
set -euo pipefail

FILE="${1:?usage: ghvid.sh <file> [content-type]}"
CT="${2:-video/mp4}"
NAME="$(basename "$FILE")"
SIZE="$(wc -c < "$FILE" | tr -d ' ')"

: "${GH_COOKIE:?set GH_COOKIE to the browser Cookie header}"
: "${GH_REPO_ID:=1338960733}"
: "${GH_EDIT_URL:=https://github.com/vjeux/trackmania-tas/edit/main/README.md}"

UA='Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36'
J="$(mktemp -d)"; trap 'rm -rf "$J"' EXIT

# --- the cookie JAR, and why this is not just `-b "$GH_COOKIE"` --------------
# GitHub ROTATES `_gh_sess` on its responses. A static Cookie header keeps
# replaying the value the browser had when it was copied, and after a couple of
# uploads the server stops accepting it: every request then 302s to /login and
# the session looks "expired" although the browser is still signed in. That is
# what killed publishing twice on 2026-09-09 (three uploads, then dead; renewed,
# two uploads, then dead again).
#
# So the session is kept the way a browser keeps it: in a JAR that curl updates
# from every Set-Cookie, seeded from the header file once and written BACK to it
# at the end, so the next run starts from the current values. Values are never
# printed. The jar also means the S3 upload cannot receive github.com cookies:
# curl matches by domain, a blanket header does not.
COOKIE_FILE="${GH_COOKIE_FILE:-$HOME/.gh-upload/cookie}"
JAR="${GH_COOKIE_JAR:-$HOME/.gh-upload/jar}"
umask 077
if [ ! -s "$JAR" ] || [ "$COOKIE_FILE" -nt "$JAR" ]; then
  : > "$JAR"; chmod 600 "$JAR"
  printf '# Netscape HTTP Cookie File\n' >> "$JAR"
  EXP=$(( $(date +%s) + 31536000 ))
  printf '%s\n' "$GH_COOKIE" | tr ';' '\n' | while IFS= read -r kv; do
    kv="${kv# }"; [ -n "$kv" ] || continue
    k="${kv%%=*}"; v="${kv#*=}"
    case "$k" in
      __Host-*|__Secure-*) printf 'github.com\tFALSE\t/\tTRUE\t%s\t%s\t%s\n' "$EXP" "$k" "$v" >> "$JAR" ;;
      *) printf '.github.com\tTRUE\t/\tTRUE\t%s\t%s\t%s\n' "$EXP" "$k" "$v" >> "$JAR" ;;
    esac
  done
fi
COOKIE_ARGS=(-b "$JAR" -c "$JAR")

# The jar, back into the header file the rest of the pipeline reads.
save_cookie() {
  awk -F'\t' '/^[^#]/ && NF>=7 { printf "%s%s=%s", sep, $6, $7; sep="; " } END { print "" }' "$JAR" > "$COOKIE_FILE.new" 2>/dev/null \
    && [ -s "$COOKIE_FILE.new" ] && chmod 600 "$COOKIE_FILE.new" && mv -f "$COOKIE_FILE.new" "$COOKIE_FILE"
}
trap 'save_cookie; rm -rf "$J"' EXIT

# --- 0. a fresh CSRF token, from the same form the browser posts -------------
curl -sS --url "$GH_EDIT_URL" "${COOKIE_ARGS[@]}" -H "user-agent: $UA" -o "$J/edit.html"
# The edit page carries its CSRF tokens in an embedded JSON blob, keyed by path:
#   "csrf_tokens":{ ... "/upload/policies/assets":{"post":"<token>"} ... }
TOKEN="$(perl -0ne 'print $1 if m{"/upload/policies/assets":\{"post":"([^"]+)"\}}' "$J/edit.html")"
[ -n "$TOKEN" ] || TOKEN="$(perl -0ne 'print $1 if m{action="/upload/policies/assets".*?name="authenticity_token"[^>]*value="([^"]+)"}s' "$J/edit.html")"
[ -n "$TOKEN" ] || { echo "ghvid: no upload CSRF token on $GH_EDIT_URL — is the cookie still valid?" >&2; exit 3; }

# --- 1. ask for an upload policy --------------------------------------------
curl -sS --url 'https://github.com/upload/policies/assets' \
  -H 'accept: application/json' -H 'origin: https://github.com' \
  -H "referer: $GH_EDIT_URL" -H "user-agent: $UA" \
  -H 'x-requested-with: XMLHttpRequest' \
  "${COOKIE_ARGS[@]}" \
  -F "name=$NAME" -F "size=$SIZE" -F "content_type=$CT" \
  -F "authenticity_token=$TOKEN" \
  -F "repository_id=$GH_REPO_ID" \
  -F 'upload_container_type=blob' \
  -F "upload_container_id=$GH_REPO_ID" \
  -o "$J/policy.json" -w '%{http_code}' > "$J/code1"
[ "$(cat "$J/code1")" = "201" ] || [ "$(cat "$J/code1")" = "200" ] || {
  echo "ghvid: step 1 returned $(cat "$J/code1")" >&2; head -c 400 "$J/policy.json" >&2; echo >&2; exit 4; }

UPLOAD_URL="$(jq -r '.upload_url' "$J/policy.json")"
ASSET_ID="$(jq -r '.asset.id' "$J/policy.json")"
ASSET_HREF="$(jq -r '.asset.href' "$J/policy.json")"
ASSET_TOKEN="$(jq -r '.asset_upload_authenticity_token' "$J/policy.json")"
ASSET_PUT="$(jq -r '.asset_upload_url' "$J/policy.json")"
[ "$ASSET_PUT" = "null" ] && ASSET_PUT="/upload/assets/$ASSET_ID"

# --- 2. push the bytes to S3, with the policy's own form fields --------------
S3ARGS=(); while IFS=$'\t' read -r k v; do S3ARGS+=(-F "$k=$v"); done < <(jq -r '.form | to_entries[] | [.key,.value] | @tsv' "$J/policy.json")
curl -sS --url "$UPLOAD_URL" -H 'origin: https://github.com' -H "referer: $GH_EDIT_URL" -H "user-agent: $UA" \
  "${S3ARGS[@]}" -F "file=@$FILE;type=$CT" -o "$J/s3.out" -w '%{http_code}' > "$J/code2"
case "$(cat "$J/code2")" in 200|201|204) ;; *) echo "ghvid: S3 upload returned $(cat "$J/code2")" >&2; head -c 400 "$J/s3.out" >&2; exit 5;; esac

# --- 3. finalise -------------------------------------------------------------
curl -sS -X PUT --url "https://github.com${ASSET_PUT}" \
  -H 'accept: application/json' -H 'origin: https://github.com' \
  -H "referer: $GH_EDIT_URL" -H "user-agent: $UA" \
  -H 'x-requested-with: XMLHttpRequest' \
  "${COOKIE_ARGS[@]}" \
  -F "authenticity_token=$ASSET_TOKEN" \
  -o "$J/done.json" -w '%{http_code}' > "$J/code3"
case "$(cat "$J/code3")" in 200|201) ;; *) echo "ghvid: finalise returned $(cat "$J/code3")" >&2; head -c 400 "$J/done.json" >&2; exit 6;; esac

echo "$ASSET_HREF"
