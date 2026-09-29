#!/bin/bash
# author-line oracle over the ship7/8 centrelines: per map with a source ghost, <map>.author-line.json + a README table
source /tmp/tmp/env.sh; cd /tmp/tmp/repo/tools/route
M=/tmp/tmp/bank-mirror; A=$HOME/persistent/private-30d/tm-player/tiny/incoming/ship8-ca2c04fd/ANCHORS.tsv
B7=$M/tm-route/tiny-builds/ship7-3adf1f85; B8=$M/tm-route/tiny-builds/ship8-ca2c04fd
OUT=$B7/centreline/AUTHOR-LINE.md
{ echo "## Author-line oracle ($(date -u +%Y-%m-%dT%H:%MZ)): the ORIGINAL author's validation ghost mapped into the tiny frame (converter ANCHORS.tsv, scale 0.5) vs our polyline"; echo; echo "| map | samples | lateral median m | p90 m | within corridor | worst m @ s | longest off-line stretch |"; echo "|---|--:|--:|--:|--:|---|---|"; } > $OUT
for f in $B7/centreline/*.road-centreline.json; do
  b=$(basename $f .road-centreline.json); nn=${b:0:2}
  row=$(awk -F'\t' -v n=$nn '$1==n' $A); [ -n "$row" ] || continue
  src=/tmp/summer2026/$(basename "$(echo "$row" | cut -f2)")
  bytes=$(echo "$row" | cut -f10); if [ "$bytes" -le 12 ]; then fu=$(target/release/tmroute gates "$src" 2>/dev/null | head -1 | cut -f2); GH=$(ls /tmp/ghosts-$fu/ghosts/1-*.Ghost.Gbx 2>/dev/null | head -1); if [ -n "$GH" ]; then src="$GH"; else echo "| $b | — | — | — | — | no author ghost in the source, no bank ghost | — |" >> $OUT; continue; fi; fi
  anc=$(echo "$row" | awk -F'\t' '{print $3","$4","$5":"$6","$7","$8}')
  target/release/tmplan author-line "$src" --anchor $anc --centreline $f --out $B7/centreline/$b.author-line.json --row "$b" 2>&1 | grep "^| " >> $OUT
done
cp $B7/centreline/*.author-line.json $B7/centreline/AUTHOR-LINE.md $B8/centreline/ 2>/dev/null
cat $OUT
