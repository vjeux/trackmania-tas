#!/bin/bash
ts=$1; B=/tmp/tmp/bank-mirror/tm-route/tiny-builds/ship15-53383427; OUT=~/persistent/private-30d/tm-route/coord/WATER-PLANES-ship15.tsv
echo -e "# WATER PLANES — Tiny Summer 2026 ship15 (all 25 maps) — GEOM $ts. Method: downward ray grid at 6 m over the author-line bbox from several heights; a hit whose physics material is Water (13) is a water plane; footprints are axis-aligned boxes (hits ±3 m). item = the converted pool item (mapgeom who at the plane); physics_ids = the item's collision physics ids (item-check); water_visual = the item carries a Water material. Each pool item = a 16×16 m tile; a footprint spans several tiles. Campaign rule: zero water contact (a car at plane_y −1.5…+2 inside the footprint = contact)." > $OUT
echo -e "map\tplane_y\tx_min\tx_max\tz_min\tz_max\thits_6m\titems(anchor)\tphysics_ids\twater_visual\tauthor_line_samples_in_contact" >> $OUT
for nn in $(seq -w 1 25); do
  stem=$(ls $B/centreline/ | grep "^$nn-" | grep "\.author-line.json$" | sed 's/.author-line.json//'); [ -n "$stem" ] || continue
  bash /tmp/water-table.sh $nn $stem $ts > /dev/null 2>&1
  f=~/persistent/private-30d/tm-route/gen/sections/$nn/WATER-$ts.tsv
  tail -n +3 $f | while IFS=$'\t' read py xa xb za zb nh items cross; do
    model=$(echo "$items" | grep -o "[A-Z][A-Z][0-9]*\.Item\.Gbx" | head -1)
    phys="?"; vis="?"
    if [ -n "$model" ]; then mkdir -p /tmp/items$nn; /tmp/tmp/upstream-target/release/mapgeom --packs /tmp/tmp/server/Packs items "/tmp/ship15/Tiny Summer 2026 - $nn.Map.Gbx" --out /tmp/items$nn > /dev/null 2>&1; it=$(find /tmp/items$nn -name "$model" | head -1); [ -n "$it" ] && { phys=$(/tmp/tmp/upstream-target/release/mapgeom --packs /tmp/tmp/server/Packs item-check "$it" --facts 2>/dev/null | grep -o "physics \[[^]]*\]" | head -1); vis=$(/tmp/tmp/upstream-target/release/mapgeom --packs /tmp/tmp/server/Packs item-check "$it" --facts 2>/dev/null | grep -c "Material\\\\Water" ); [ "$vis" -gt 0 ] && vis=yes || vis=no; }; fi
    echo -e "$nn\t$py\t$xa\t$xb\t$za\t$zb\t$nh\t$items\t$phys\t$vis\t$cross" >> $OUT
  done
  echo "$nn done $(date -u +%H:%M:%S)"
done
echo "DONE $(date -u +%Y-%m-%dT%H:%M:%SZ) — complete, $(($(wc -l < $OUT)-2)) planes" >> $OUT
