#!/bin/bash
# record-stops-short-scan.sh — find records that end BEFORE the finish line.
#
#   bash tools/record-stops-short-scan.sh
#
# Catches: *filming a finish that is not in the file.* The tail past the line is
# the well-known shape; this is its opposite and nothing else looks for it.
# 126859's published files end 95 ms short of their declared race time, so the
# crossing is simply not in the record and no clip can show it. Scans every
# published file and prints race time, last sample, and the delta, flagging
# anything more than 60 ms either side — so the shape is visible across the
# corpus instead of one file at a time.
#
# RUNS ON THE RENDER BOX: the scan walks a checkout of this repo at /tmp/repo.
R="$HOME/trackmania-tas/tools/tmtraj"; [ -x "$R/target/release/tmtrajcheck" ] || { R=/mnt/c/Users/vjeux/tj; echo "[warn] using the STALE toolkit at $R -- build tools/tmtraj" >&2; }
cd "$R" || exit 1
# THE OPPOSITE OF A TAIL: a record that stops BEFORE the line. 126859's
# published files end 95 ms short of their declared race time, so the finish
# crossing is not in the record and a clip cannot show it. Scan every published
# file for it, so we see the shape across the corpus rather than one file at a
# time.
printf '%-8s %-40s %8s %10s %9s %s\n' MAP FILE RACE LAST_SAMPLE DELTA NOTE
for d in /tmp/repo/*/replays; do
  page=$(basename "$(dirname "$d")"); m=${page%%-*}
  for f in "$d"/*.Ghost.Gbx; do
    b=$(basename "$f")
    meta=$(./target/release/inputcount --meta "$f" 2>/dev/null) || continue
    [ -z "$meta" ] && continue
    race=$(echo "$meta" | cut -f2)
    last=$(./target/release/inputcount --csv "$f" 2>/dev/null | tail -1 | cut -d, -f1)
    case "$last" in ''|*[!0-9-]*) continue;; esac
    delta=$((last - race))
    if [ "$delta" -lt -60 ]; then note="RECORD STOPS SHORT -- the finish is not in it"
    elif [ "$delta" -gt 60 ]; then note="tail past the finish"
    else continue; fi
    printf '%-8s %-40s %8s %10s %+9s %s\n' "$m" "$b" "$race" "$last" "$delta" "$note"
  done
done
echo "--- scan complete"
