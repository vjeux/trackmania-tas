#!/bin/bash
# sep-truncation-scan.sh — does any audit verdict rest on a comparison that
# silently stopped?
#
#   bash tools/sep-truncation-scan.sh
#
# Catches: *a CLEAN verdict produced by an instrument falling silent.* `sep`
# walks two files index by index and BAILS OUT when the recorded sample times
# differ, printing to stderr — which every pipeline here discards. Sample times
# are session times, so two recordings from different sessions can share no
# instants at all: all ten of 228607's files produced ZERO compared rows against
# `AUTHOR_LAP_20258`, and the pipeline read that silence as clean. This scans
# every our-file/reference pair in the corpus and reports any comparison whose
# row count falls short of min(samples) — the absent-signal bug, hunted in our
# own instrument rather than in the data.
#
# RUNS ON THE RENDER BOX: REPO is a checkout of this repo at /tmp/repo.
# Full scan: does ANY audit verdict rest on a truncated comparison?
# A CLEAN verdict from a comparison that stopped after 3 samples is not a
# verdict at all -- it is the tool falling silent and the caller hearing "fine".
cd /mnt/c/Users/vjeux/tj || exit 1
TJ=./target/release
REPO=/tmp/repo
is_ref(){ echo "$1" | grep -Eqi 'wr|human|rank|author'; }
printf '%-8s %-38s %-34s %7s %7s %s\n' MAP OURS REF ROWS MINSAMP STATUS
for d in "$REPO"/*/replays; do
  page=$(basename "$(dirname "$d")"); map=${page%%-*}
  for r in "$d"/*.Ghost.Gbx; do
    is_ref "$(basename "$r")" || continue
    for f in "$d"/*.Ghost.Gbx; do
      b=$(basename "$f"); is_ref "$b" && continue
      na=$($TJ/inputcount --meta "$f" 2>/dev/null | cut -f3)
      nb=$($TJ/inputcount --meta "$r" 2>/dev/null | cut -f3)
      [ -z "$na" ] || [ -z "$nb" ] && continue
      mn=$na; [ "$nb" -lt "$mn" ] && mn=$nb
      rows=$($TJ/sep "$f" "$r" 2>/dev/null | tail -n +2 | wc -l)
      if [ "$rows" -lt "$((mn - 1))" ]; then
        pct=$(awk -v a="$rows" -v b="$mn" 'BEGIN{printf "%.0f", 100*a/b}')
        printf '%-8s %-38s %-34s %7s %7s TRUNCATED at %s%%\n' "$map" "$b" "$(basename "$r")" "$rows" "$mn" "$pct"
      fi
    done
  done
done
echo "--- scan complete"
