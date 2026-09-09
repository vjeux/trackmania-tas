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
#
# ⛔ DO NOT MAKE THIS WRITE THE COOKIE FILE. On 2026-09-09 I replaced the static
# Cookie header with a curl JAR (-b/-c) that was written back to
# ~/.gh-upload/cookie at exit, on the theory that GitHub rotates `_gh_sess` and
# a copied header goes stale. The theory may even be right; the mechanism is
# not. `curl -c` writes only cookies with an expiry, so a jar round trip drops
# every SESSION cookie the server sets — and the write-back then replaced a
# 1749-byte header holding user_session, __Host-user_session_same_site,
# logged_in, dotcom_user and _gh_sess with 33 bytes holding `_octo`. That
# destroyed a session a human had just copied out of his browser for the third
# time that hour, and cost another renewal. The header file is INPUT: read it,
# never write it. If the rotation theory is ever worth testing again, do it in a
# scratch copy of the file and prove the round trip keeps every name first.
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

# --- 0. a fresh CSRF token, from the same form the browser posts -------------
curl -sS --url "$GH_EDIT_URL" -b "$GH_COOKIE" -H "user-agent: $UA" -o "$J/edit.html"
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
  -b "$GH_COOKIE" \
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
  -b "$GH_COOKIE" \
  -F "authenticity_token=$ASSET_TOKEN" \
  -o "$J/done.json" -w '%{http_code}' > "$J/code3"
case "$(cat "$J/code3")" in 200|201) ;; *) echo "ghvid: finalise returned $(cat "$J/code3")" >&2; head -c 400 "$J/done.json" >&2; exit 6;; esac

echo "$ASSET_HREF"
