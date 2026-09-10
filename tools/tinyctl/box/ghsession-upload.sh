#!/bin/sh
# ghsession-upload.sh FILE [CONTENT-TYPE] — the `GHVID` shim: `clip ship` runs
# its uploader as `<uploader> <file>` and reads the asset URL off stdout. This
# hands that to `ghsession upload`, the uploader's OWN session (see
# UPLOADER-OWN-SESSION.md) — no browser cookie of vjeux's is read here.
# Exit 3 = the session is gone, the code `clip ship` already reads as "renew".
G=/home/vjeux/trackmania-tas/tools/target/release/ghsession
[ -x "$G" ] || { echo "ghsession-upload: no $G (tinyctl box-build --crates ghsession)" >&2; exit 1; }
if [ -n "$2" ]; then exec "$G" upload "$1" --content-type "$2"; else exec "$G" upload "$1"; fi
