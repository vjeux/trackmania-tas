#!/bin/bash
# author-attitude-all.sh TS — the author's surface-relative attitude per gate section, all 25 tiny ship15 maps
# -> tm-route/coord/AUTHOR-ATTITUDE-<TS>.tsv (write-once) + per-sample rows in gen/sections/<NN>/author-attitude-rows-<TS>.tsv
TS=$1; cd /tmp/tmp/repo/tools/route && source /tmp/tmp/env.sh
B=/tmp/tmp/bank-mirror/tm-route/tiny-builds/ship15-53383427; OUT=~/persistent/private-30d/tm-route/coord/AUTHOR-ATTITUDE-$TS.tsv
{
echo "# AUTHOR ATTITUDE — Tiny Summer 2026 ship15, all 25 maps — GEOM $TS. Source: the author-line ghosts' 50 ms samples (position + rotation quaternion) mapped into the tiny frame (tmplan author-attitude). body up = R(q)·ŷ; supporting surface = the closest of five 2.5 m rays (down, ±x, ±z) from the sample; in_contact = that surface ≤ 1.2 m; tilt = angle(body up, surface normal) = the SURFACE-RELATIVE attitude of the ratified rule. Sections = the author's own gate crossings (arrival-bands v3). airborne_rotation_events = stretches with no surface within 2.5 m and body up ≥ 70° from vertical (flips/rolls in the air). wall_ride_s = seconds on a surface ≥ 70° from horizontal with tilt < 30° (four-wheel wall rides — legal by construction) with their windows. Caveat: the supporting surface is ray-inferred from the collision import (the 20 pad→floor slope is missing from it; a few surfaces may read as air)."
echo -e "map\tsection\tt0\tt1\tsamples\tcontact_s\ttilt_ge45_s\ttilt_ge70_s\tmax_tilt_deg\tairborne_rotation_events\twall_ride_s\twall_ride_windows"
for nn in $(seq -w 1 25); do
  stem=$(ls $B/centreline/ | grep "^$nn-" | grep "\.author-line.json$" | sed 's/.author-line.json//'); [ -n "$stem" ] || continue
  bands=$B/arrival-bands/$stem.arrival-bands.v3.json; cuts=$(jq -r '[.gates[] | "wp\(.map_waypoint)=\(.author.t_s)"] | join(",")' $bands 2>/dev/null)
  mkdir -p ~/persistent/private-30d/tm-route/gen/sections/$nn
  target/release/tmplan author-attitude "/tmp/ship15/Tiny Summer 2026 - $nn.Map.Gbx" --gates $B/geom/$stem.deck.json --author-line $B/centreline/$stem.author-line.json --cuts "$cuts" --map-id $nn --rows ~/persistent/private-30d/tm-route/gen/sections/$nn/author-attitude-rows-$TS.tsv 2>/dev/null | grep -v "^built\|^map"
done
echo "DONE $(date -u +%Y-%m-%dT%H:%M:%SZ) — complete, 25 maps"
} > $OUT
wc -l $OUT; awk -F'\t' 'NF>=9 && $8+0 > 0 {print $1, $2, "ge70 " $8 " s, max " $9 "°, air-rot " $10}' $OUT | head -20
