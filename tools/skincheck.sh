#!/bin/bash
# skincheck.sh <file.Ghost.Gbx> ...
#
# Refuse any ghost carrying a CUSTOM CAR SKIN. Every other identity field we
# chase is metadata; this one is the paint on the car in the video.
#
# 276874's two WATCH tapes read login TAS, carried no account id, and imported as
# Ghost:TAS -- clean on all three readers -- while carrying:
#
#   Skins\Models\CarSport\frckitbot (1)(1)_756eeda4-....zip
#   https://core.trackmania.nadeo.live/storageObjects/756eeda4-...
#
# They would have gone on screen in a stranger's livery, in a clip captioned as
# ours, and nothing in the gate would have said a word. The correct value is
# Skins\Models\CarSport\TAS.zip.
#
# Exit 0 clean, 1 refused. Strings are read from the raw bytes: the skin path is
# stored as plain text in the container, unlike the nickname.
set -u
rc=0
for f in "$@"; do
  [ -f "$f" ] || { echo "skin: no such file: $f"; rc=1; continue; }
  hits=$(strings -n 6 "$f" 2>/dev/null | grep -Ei 'Skins[\\/]Models|storageObjects|\.zip$' | sort -u)
  bad=$(printf '%s\n' "$hits" | grep -Eiv '(^$|Skins.Models.CarSport.TAS\.zip$)' || true)
  if [ -n "$bad" ]; then
    echo "REFUSED  $(basename "$f")"
    printf '%s\n' "$bad" | sed 's/^/    /'
    rc=1
  else
    echo "clean    $(basename "$f")  $(printf '%s' "$hits" | tr '\n' ' ')"
  fi
done
exit $rc
