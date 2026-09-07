#!/bin/bash
# bisect-map.sh SRC.Map.Gbx LIB.zip MAPPING.tsv ALIASES.txt LO HI [ALWAYS.txt]
# Builds the tiny map with only the aliases numbered LO..HI (0-based lines of
# ALIASES.txt) kept in the mapping — every other alias becomes "-" (dropped,
# not refused) — loads it in the editor and prints dead / editor / the raw
# context. Item rows (i@) and baked rows (b@) follow the same filter.
set -u
SRC=$1; LIB=$2; MAP=$3; AL=$4; LO=$5; HI=$6; ALWAYS=${7:-/dev/null}
T=/home/vjeux/trackmania-tas-tiny/tools
keep=$( { sed -n "$((LO+1)),$((HI+1))p" "$AL"; cat "$ALWAYS"; } )
awk -v keep="$keep" -F'\t' 'BEGIN{OFS="\t"; n=split(keep,k,"\n"); for(i=1;i<=n;i++) K[k[i]".Item.Gbx"]=1}
  /^#/ {print; next}
  { if ($2 ~ /\.Item\.Gbx$/ && !($2 in K)) $2="-"; print }' "$MAP" > /tmp/bisect-mapping.tsv
$T/target/release/tmmaps tiny "$SRC" --mapping /tmp/bisect-mapping.tsv --library "$LIB" --out /tmp/Bisect.Map.Gbx >/dev/null 2>&1 || { echo "tiny failed"; exit 2; }
echo "items $LO..$HI ($(echo "$keep" | wc -l) aliases): $($T/tmmaps/tiny/loadmap.sh /tmp/Bisect.Map.Gbx)"
