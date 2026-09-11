#!/bin/sh
# The lap -> the two ghosts, from a driven tape. Rust tools only; this is
# the order the steps go in (each refuses on its own control).
#
#   make-filmable.sh MAP.Map.Gbx TAPE.tsv TEMPLATE.grid.Ghost.Gbx OUT_DIR
#
# 1. the tape into the from-scratch container the driver used (validates)
# 2. the declared time + splits from the oracle
# 3. a HUMAN-shaped container (24 chunks) carrying the run's own inputs,
#    time, result and validation chunks -- a from-scratch container's
#    record has one descriptor and no notices and crashes the game client
#    at MediaTracker import (measured 2026-09-11)
# 4. the engine's telemetry regenerated into it (`ghost regen`) -> filmable
set -eu
MAP=$1; TAPE=$2; TPL=$3; OUT=$4
T=$HOME/trackmania-tas/tools
E=$T/search/target/release/tmexplore-real
G=$T/target/release/ghost
DONOR=$T/../134672-kekl-sausage-ice/replays/TAS_67319.Ghost.Gbx
export TM_SERVER=${TM_SERVER:-/tmp/tmoracle/server}
export FK_BIN=$T/fk/target/release/fk FK_SHIM=$T/search/target/release/libforkshim.so
mkdir -p "$OUT"
$E write --template "$TPL" --map "$MAP" --tape "$TAPE" --out "$OUT/raw.Ghost.Gbx" --confirm | tee "$OUT/write.log"
MS=$(grep -o 'Finish { ms: [0-9]*' "$OUT/write.log" | grep -o '[0-9]*$')
[ -n "$MS" ] || { echo "the tape did not finish"; exit 1; }
$G declare "$OUT/raw.Ghost.Gbx" "$OUT/Silverstone.lap.Ghost.Gbx" --from-oracle --map "$MAP" | tail -3
# the human container: rebind to this map, stretch to the tape, inject, declare
$G map rebind "$DONOR" "$OUT/h1.Ghost.Gbx" --map "$MAP" | head -1
TICKS=$($G inspect "$OUT/Silverstone.lap.Ghost.Gbx" | sed -n 's/.*archive 0: \([0-9]*\) ticks.*/\1/p' | head -1)
[ -n "$TICKS" ] || { echo "no tick count in the ghost"; exit 1; }
$G trim "$OUT/h1.Ghost.Gbx" "$OUT/h2.Ghost.Gbx" --to $(( (TICKS - 1) * 10 )) | grep -i "wrote\|ticks" | head -2
$G tape extract "$OUT/Silverstone.lap.Ghost.Gbx" --out "$OUT/lap.gtape" | tail -1
$G tape inject "$OUT/h2.Ghost.Gbx" "$OUT/h3.Ghost.Gbx" --tape "$OUT/lap.gtape" --allow-telemetry-mismatch | tail -1
SPLITS=$($G inspect "$OUT/Silverstone.lap.Ghost.Gbx" | grep '^splits' | sed 's/^splits *//; s/ *(.*//' | tr -s ' ' '\n' | sed '$d' | awk '{printf "%d,", $1*1000+0.5}' | sed 's/,$//')
$G declare "$OUT/h3.Ghost.Gbx" "$OUT/h4.Ghost.Gbx" --time "$MS" --splits "$SPLITS" | tail -1
$G swap-chunks "$OUT/h4.Ghost.Gbx" "$OUT/h5.Ghost.Gbx" --from "$OUT/Silverstone.lap.Ghost.Gbx" --ids 0309201D,03092005,0309202B,0309202D | tail -1
$G regen "$OUT/h5.Ghost.Gbx" "$OUT/Silverstone.lap.filmable.Ghost.Gbx" --map "$MAP" --carrier layout | tail -2
$G verify "$OUT/Silverstone.lap.filmable.Ghost.Gbx" --map "$MAP" | grep -E "^(PASS|FAIL|WARN) V(6|7|11)" | cut -c1-120
$T/search/target/release/tmsearch validate --map "$MAP" "$OUT/Silverstone.lap.Ghost.Gbx" "$OUT/Silverstone.lap.filmable.Ghost.Gbx" | tail -2
cp "$TAPE" "$OUT/Silverstone.lap.tape.tsv"
rm -f "$OUT"/h[1-5].Ghost.Gbx "$OUT/lap.gtape" "$OUT/raw.Ghost.Gbx"
echo "lap $MS ms -> $OUT"
