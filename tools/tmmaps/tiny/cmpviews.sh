#!/bin/bash
# cmpviews.sh VIEWS.tsv  -> /tmp/tiny3/cmp-NAME.jpg per view (original | tiny)
# Many comparison views for the price of TWO map loads: the original is opened
# once and every view shot with shootcam.sh, then the tiny map the same way.
# VIEWS.tsv rows: NAME<TAB>ox,oy,oz<TAB>DIST<TAB>H<TAB>V  (source-map camera:
# target, orbital distance, angles in radians — see shootmap.sh); the tiny
# camera is the target mapped through the tiny transform (ANCHOR) at half the
# distance, like cmpshot.sh. Env: ANCHOR="sx,sy,sz:tx,ty,tz" ORIG= TINY=
# OUTDIR= (default /tmp/tiny3) TAG= (prefix for the names, e.g. s03).
set -u
VIEWS=$1
T=$(cd "$(dirname "$0")" && pwd)
OUTDIR=${OUTDIR:-/tmp/tiny3}
TAG=${TAG:-}
ORIG=${ORIG:-/tmp/Summer-2026-01.Map.Gbx}
TINY=${TINY:-/tmp/tiny3/Summer-01-Tiny.Map.Gbx}
IFS=,: read -r ax ay az bx by bz <<< "${ANCHOR:-1584,16,784:1584,11.5,784}"
map() { awk -v a="$1" -v b="$2" -v p="$3" 'BEGIN{printf "%.2f", b+(p-a)*0.5}'; }

shoot_all() { # MAP SIDE(o|t)
  local M=$1 SIDE=$2 first=1 name pt d h v ox oy oz cam
  while IFS=$'\t' read -r name pt d h v; do
    [ -z "$name" ] && continue; case "$name" in \#*) continue;; esac
    IFS=, read -r ox oy oz <<< "$pt"
    if [ "$SIDE" = t ]; then
      cam="$(map $ax $bx $ox),$(map $ay $by $oy),$(map $az $bz $oz),$(awk -v d=$d 'BEGIN{printf "%.2f", d*0.5}'),$h,$v"
    else
      cam="$ox,$oy,$oz,$d,$h,$v"
    fi
    local out="$OUTDIR/cmp-$TAG$name-$SIDE.png"
    if [ $first = 1 ]; then
      "$T/shootmap.sh" "$M" "$out" "$cam" "$([ $SIDE = t ] && echo Tiny || echo Orig)$TAG" 2>&1 | grep "DIALOG\|not responding\|^ctx\|dead" | head -3
      first=0
    else
      "$T/shootcam.sh" "$out" "$cam" 2>&1 | grep "not responding" | head -1
    fi
    echo "  $SIDE $name cam $cam -> $(stat -c %s "$out" 2>/dev/null) B"
  done < "$VIEWS"
}

shoot_all "$ORIG" o
shoot_all "$TINY" t
while IFS=$'\t' read -r name rest; do
  [ -z "$name" ] && continue; case "$name" in \#*) continue;; esac
  o="$OUTDIR/cmp-$TAG$name-o.png"; t="$OUTDIR/cmp-$TAG$name-t.png"
  [ -s "$o" ] && [ -s "$t" ] || { echo "cmp-$TAG$name: missing side"; continue; }
  ~/bin/ffmpeg -y -loglevel error -i "$o" -i "$t" -filter_complex "[0:v]scale=960:-1[a];[1:v]scale=960:-1[b];[a][b]hstack" -q:v 4 "$OUTDIR/cmp-$TAG$name.jpg"
  echo "$OUTDIR/cmp-$TAG$name.jpg"
done < "$VIEWS"
