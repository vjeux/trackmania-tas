#!/bin/bash
# jump-recheck-speedometer.sh — re-grade every C3/C4 jump refusal against the
# car's own speedometer.
#
#   bash tools/jump-recheck-speedometer.sh
#
# Catches: *a distance threshold condemning a published original.* C3 refused a
# file when consecutive positions were far apart in METRES, which cannot tell a
# teleport from fast driving — it produced a work queue of 24 files across 8
# maps, most of which were never broken. This is the re-run that emptied that
# queue: for every C3/C4 refusal it prints the worst step's implied speed, the
# speed `CSceneVehicleVis` recorded at that sample, and their ratio, and also the
# last sample's offset from the declared race time — because a record can stop
# SHORT of the line, the opposite defect, which no check was looking for.
#
# RUNS ON THE RENDER BOX: REPO is a checkout of this repo at /tmp/repo and the
# binaries are read out of the crate's target/release. The logic is the point.
# recheck.sh -- re-run every C3/C4 refusal using the SPEEDOMETER rule.
#
# WHY THIS RE-RUN EXISTS
#
# C3 refused a file when consecutive positions were far apart in METRES. That
# measure cannot tell a teleport from fast driving, and on these maps the cars
# are very fast. It produced a work queue of 24 files across 8 maps, most of
# which were never broken. The bug was compounded by measuring a STALE staging
# generation rather than the published tree.
#
# The discriminator that works needs no threshold on distance at all, because
# the recording carries an independent witness: the car's own scalar speed.
#
#   real driving : implied / recorded ~= 1.00
#   respawn      : recorded speed EXACTLY 0.0  (normal on Trial maps)
#   splice       : ratio in the thousands (227654: 50090 implied vs 19.2)
#
# Also reported: last sample vs declared race time, because a record can stop
# SHORT of the line -- the opposite defect, and one no check was looking for.
set -u
cd /mnt/c/Users/vjeux/tj || exit 1
TJ=./target/release
REPO=/tmp/repo

printf '%-8s %-38s %-9s %-8s %-9s %-7s %s\n' MAP FILE GATE-FAILS WORST-V RATIO DELTA VERDICT
for d in "$REPO"/*/replays; do
  page=$(basename "$(dirname "$d")"); m=${page%%-*}
  for f in "$d"/*.Ghost.Gbx; do
    b=$(basename "$f")
    ms=$(echo "$b" | grep -oE '[0-9]{4,8}' | tail -1); [ -z "$ms" ] && continue
    out=$("$TJ/tmtrajcheck" "$f" --race "$ms" 2>&1); rc=$?
    if [ "$rc" -ge 100 ]; then
      printf '%-8s %-38s %-9s %-8s %-9s %-7s %s\n' "$m" "$b" CRASH - - - "gate panicked -- cannot judge"
      continue
    fi
    ids=$(echo "$out" | grep '^  FAIL' | awk '{print $2}' | paste -sd, -)
    [ -z "$ids" ] && continue
    case "$ids" in *C3*|*C4*) : ;; *) continue ;; esac

    # the speedometer witness for the worst step
    read -r V REC RATIO <<<"$("$TJ/spdcheck" "$f" 2>/dev/null | awk 'NR==2{print $1, $2, $3}')"
    # last sample vs declared race
    last=$("$TJ/inputcount" --csv "$f" 2>/dev/null | tail -1 | cut -d, -f1)
    delta="-"
    case "$last" in ''|*[!0-9-]*) : ;; *) delta=$((last - ms)) ;; esac

    verdict=$(awk -v r="${RATIO:-999}" -v rec="${REC:-0}" '
      BEGIN{
        if (r+0 < 1.5)            print "DRIVING -- C3 was a false refusal";
        else if (rec+0 == 0)      print "RESPAWN -- legitimate, not a defect";
        else                      print "SPLICE -- genuinely refused";
      }')
    printf '%-8s %-38s %-9s %-8s %-9s %+7s %s\n' "$m" "$b" "$ids" "${V:-?}" "${RATIO:-?}" "$delta" "$verdict"
  done
done
echo "--- done"
