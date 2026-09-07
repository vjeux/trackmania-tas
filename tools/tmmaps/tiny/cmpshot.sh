#!/bin/bash
# cmpshot.sh "ox,oy,oz" DIST H V NAME  -> /tmp/tiny3/cmp-NAME.jpg (original | tiny)
# Same view of the original Summer 01 and of the tiny map: the tiny camera
# target is the original's mapped through the tiny transform (anchor
# 1584,16,784 -> 1584,11.5,784, scale 0.5) and the distance is halved.
set -u
IFS=, read -r ox oy oz <<< "$1"
D=$2; H=$3; V=$4; NAME=$5
# ANCHOR="sx,sy,sz:tx,ty,tz" = the source spawn -> tiny anchor `tmmaps tiny` printed
# (default: Summer 01's 1584,16,784 -> 1584,11.5,784)
IFS=,: read -r ax ay az bx by bz <<< "${ANCHOR:-1584,16,784:1584,11.5,784}"
tx=$(awk -v a=$ax -v b=$bx -v p="$ox" 'BEGIN{printf "%.2f", b+(p-a)*0.5}')
ty=$(awk -v a=$ay -v b=$by -v p="$oy" 'BEGIN{printf "%.2f", b+(p-a)*0.5}')
tz=$(awk -v a=$az -v b=$bz -v p="$oz" 'BEGIN{printf "%.2f", b+(p-a)*0.5}')
td=$(awk -v d="$D" 'BEGIN{printf "%.2f", d*0.5}')
T=$(cd "$(dirname "$0")" && pwd)
ORIG=${ORIG:-/tmp/Summer-2026-01.Map.Gbx}
TINY=${TINY:-/tmp/tiny3/Summer-01-Tiny.Map.Gbx}
${SHOOT:-$T/shootmap.sh} "$ORIG" /tmp/tiny3/cmp-$NAME-o.png "$ox,$oy,$oz,$D,$H,$V" Orig01 2>&1 | grep "DIALOG\|not responding" | head -2
${SHOOT:-$T/shootmap.sh} "$TINY" /tmp/tiny3/cmp-$NAME-t.png "$tx,$ty,$tz,$td,$H,$V" Tiny01 2>&1 | grep "DIALOG\|not responding" | head -2
~/bin/ffmpeg -y -loglevel error -i /tmp/tiny3/cmp-$NAME-o.png -i /tmp/tiny3/cmp-$NAME-t.png -filter_complex "[0:v]scale=960:-1[a];[1:v]scale=960:-1[b];[a][b]hstack" -q:v 4 /tmp/tiny3/cmp-$NAME.jpg
echo "/tmp/tiny3/cmp-$NAME.jpg  tiny cam $tx,$ty,$tz dist $td"
