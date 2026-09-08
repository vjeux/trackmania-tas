#!/bin/bash
# author-ground on all 25: ship8 (A) vs BUILD (B); table to tm-route/tiny/<build>/AUTHOR-GROUND.md
source /tmp/tmp/env.sh; cd /tmp/tmp/repo/tools/route
BUILD=${1:-ship10-5cd51cc9}; BDIR=${2:-/tmp/s10}; M=/tmp/tmp/bank-mirror; B=$M/tm-route/tiny-builds/$BUILD; T8=$M/tm-player/tiny/ship7-3adf1f85/by-stem
OUT=$B/centreline/AUTHOR-GROUND.md; mkdir -p /tmp/ag-$BUILD
ls $B/centreline/*.author-line.json | xargs -P 6 -I{} bash -c 'f={}; b=$(basename $f .author-line.json); nn=${b:0:2}; mp=$(ls "'$BDIR'"/*" - $nn.Map.Gbx" 2>/dev/null | head -1); [ -n "$mp" ] || mp=$(ls "'$BDIR'"/*$nn*.Map.Gbx | head -1); target/release/tmplan author-ground "'$T8'/$b.Map.Gbx" "$mp" --gates '$B'/geom/$b.deck.json --author-line $f 2>&1 | grep -E "MISSING|samples," > /tmp/ag-'$BUILD'/$b.txt'
{ echo "## Author ground check — ship8 (A) vs $BUILD (B), $(date -u +%Y-%m-%dT%H:%MZ): downward rays from every author-line sample (100 ms); a stretch where the author stands on a surface in A that B lacks within 3 m = a removed surface"; echo; echo "| map | samples | author airborne in A | A-supported, B-missing | stretches (samples, t, from → to, A surface, B result) |"; echo "|---|--:|--:|--:|---|"; for t in /tmp/ag-$BUILD/*.txt; do b=$(basename $t .txt); s=$(grep "samples," $t | sed -E 's/.*: ([0-9]+) samples, A airborne\/unsupported ([0-9]+) \(([0-9]+) %\), A-supported-but-B-missing ([0-9]+)/\1|\2 (\3 %)|\4/'); st=$(grep MISSING $t | sed 's/  MISSING in B: //' | tr '\n' ';' | sed 's/;$//'); echo "| $b | $(echo $s | cut -d'|' -f1) | $(echo $s | cut -d'|' -f2) | $(echo $s | cut -d'|' -f3) | ${st:-—} |"; done; } > $OUT
cp $OUT ~/persistent/private-30d/tm-route/tiny/$BUILD/centreline/ 2>/dev/null
cat $OUT
