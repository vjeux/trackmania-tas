#!/bin/bash
# tiny-build.sh <incoming build dir> <build name>: the whole tiny pipeline for a converter build — copy the build to
# the mirror (one rsync), tiny-one.sh per map (collhash verified against its MANIFEST), then a build README + ONE
# tarball of the build's geom/plan/routes/centreline in the bank (the per-map tarballs already exist).
set -u
SRC=$1; BUILD=$2
source /tmp/tmp/env.sh
M=/tmp/tmp/bank-mirror; REAL=$HOME/persistent/private-30d/tm-route
T=$M/tm-player/tiny/$BUILD; mkdir -p $T
rsync -a "$SRC/" $T/
[ -f $T/MANIFEST.txt ] || { echo "no MANIFEST.txt in $SRC"; exit 2; }
# out2-style stems (the keys of gap-verdicts.tsv / road-exclusions.tsv) for builds that name maps "Tiny Summer 2026 - NN"
L=$T/by-stem; mkdir -p $L
declare -A COUNTRY=([21]=Argentina-2026 [22]=Saudi-Arabia-2026 [23]=Norway-2026 [24]=Poland-2026 [25]=Japan-2026)
for f in $T/*.Map.Gbx; do bn=$(basename "$f" .Map.Gbx); if [[ "$bn" =~ \ -\ ([0-9][0-9])$ ]]; then nn=${BASH_REMATCH[1]}; stem=${COUNTRY[$nn]:-$nn-Summer-2026---$nn}; [ -n "${COUNTRY[$nn]:-}" ] && stem="$nn-${COUNTRY[$nn]}"; ln -sf "$f" "$L/$stem.Map.Gbx"; else ln -sf "$f" "$L/$(echo "$bn" | tr ' ' '-').Map.Gbx"; fi; done
n=$(ls $L/*.Map.Gbx | wc -l); echo "$BUILD: $n maps"
OUT=$M/tm-route/tiny-builds/$BUILD; mkdir -p $OUT
: > $OUT/RUN.txt
for f in $L/*.Map.Gbx; do bash /tmp/tmp/repo/tools/route/scripts/tiny-one.sh "$f" --build $BUILD --manifest $T/MANIFEST.txt 2>&1 | tail -1 | tee -a $OUT/RUN.txt; done
# README: per-map line + every gap leg with the verdict carried over from the previous build (same group ids are
# NOT guaranteed across builds — verdicts are re-keyed by the converter when they re-verify)
{ echo "# Tiny build $BUILD — gates, deck gates (credit offsets), routes, road-following centrelines (+ speed_hint), $(date -u +%Y-%m-%dT%H:%MZ)"; echo; echo '```'; cat $OUT/RUN.txt | cut -c1-260; echo '```'; echo; echo "Every artefact stamps build/md5/collhash (collhash verified against the build's MANIFEST). Gap verdicts (tiny/gap-verdicts.tsv) and road exclusions (tiny/road-exclusions.tsv) were written for out2 and are applied by map stem — re-verify on this build."; } > $OUT/README.md
ts=$(date -u +%Y%m%dT%H%MZ)
tar czf /tmp/tiny-$BUILD-$ts.tgz -C $M/tm-route/tiny-builds $BUILD && cp /tmp/tiny-$BUILD-$ts.tgz $REAL/drops/ && echo -e "$ts\ttiny-$BUILD-$ts.tgz\ttiny build $BUILD: all maps — geom, deck gates, plan, routes, centrelines (+speed_hint), README\t$(stat -c %s /tmp/tiny-$BUILD-$ts.tgz)\t$(md5sum < /tmp/tiny-$BUILD-$ts.tgz | cut -c1-12)" >> $REAL/drops/MANIFEST.tsv
mkdir -p $REAL/tiny/$BUILD && tar xzf /tmp/tiny-$BUILD-$ts.tgz -C $REAL/tiny/ 2>/dev/null
echo "done $BUILD → drops/tiny-$BUILD-$ts.tgz (+ tm-route/tiny/$BUILD/)"
