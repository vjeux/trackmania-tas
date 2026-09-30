#!/bin/sh
# Stage-2 batch: bake every validated reference/source pair with the current
# mechanisms and score it against Granady's item.
# Usage: stage2-batch.sh PAIRS.TSV OUTDIR   (run from tools/)
# PAIRS.TSV rows: REF_PATH<TAB>ITEM<TAB>SOURCE<TAB>SCALE (final-pairs.tsv format)
# Per pair: generic recipe (seed-largest normals, V-primary tangents, U
# smoothing, quantized U/V key, angle weights on Technics/TB/Trims, pos + uv1
# transplant from the reference), plus the fitted per-block script when one
# exists (bake-tiny-road17.sh / bake-tiny-road18.sh). Prints one line per
# pair: struct diff lines (chunkwalk, Ident excluded), per-visual vert deltas,
# atlas overlap, and N/U/V bit-exact rates over matched corners.
set -e
PAIRS=${1:?usage: $0 PAIRS.TSV OUTDIR}
OUT=${2:?usage: $0 PAIRS.TSV OUTDIR}
mkdir -p "$OUT"
B=target/release/examples
while IFS="$(printf '\t')" read -r ref kind src scale; do
  [ -n "$ref" ] || continue
  [ "$kind" = "ITEM" ] || { echo "$(basename "$ref"): skipped ($kind)"; continue; }
  [ -f "$src" ] || { echo "$(basename "$ref"): source missing $src"; continue; }
  name=$(basename "$ref" .Item.Gbx)
  out="$OUT/$name.Item.Gbx"
  ident=$(basename "$ref")
  case "$name" in
    Tiny_Road_17) ./bake-tiny-road17.sh "$src" "$ref" "$out" "$ident" >"$OUT/$name.log" 2>&1 ;;
    Tiny_Road_18) ./bake-tiny-road18.sh "$src" "$ref" "$out" "$ident" >"$OUT/$name.log" 2>&1 ;;
    *)
      env TINY_SEED_LARGEST=1 TINY_VPRIM=1 TINY_USMOOTH=1 TINY_UKEY=1 TINY_UKEYQ=1 \
          TINY_WEIGHT_MAP="TrackBorders:angle,Technics:angle,TechnicsTrims:angle" \
          TINY_CREASE_MAP="TrackBorders:65,TechnicsTrims:45,TechnicsSpecials:55,Technics:52,RoadTech:20,SpecialFXTurbo:55,LightSpot:44,DecalMarksRamp:44" \
          TINY_NMODE_MAP="TechnicsSpecials:corner,SpecialFXTurbo:corner" TINY_NDEDUP=face \
          TINY_UMODE_MAP="TechnicsSpecials:du,TechnicsTrims:du,Technics:du,TrackBorders:duoff,RoadTech:range,TrackWallClips:du" \
          TINY_TAN_MAP="Technics:12,TechnicsSpecials:60" TINY_UMAG_MAP="TechnicsSpecials" \
          TINY_POS_REF="$ref" TINY_UV1_REF="$ref" \
          target/release/mapgeom --packs /tmp static-item "$src" --out "$out" --ident "$ident" --author "$ident" --scale "$scale" --collection 26 >"$OUT/$name.log" 2>&1 ;;
  esac
  structdiff=$(diff <($B/chunkwalk "$ref" 2>&1) <($B/chunkwalk "$out" 2>&1) | grep -c '^[<>]' || true)
  # per-visual vert deltas (his order)
  # (joined by material name: a visual-order difference must not misattribute deltas)
  verts=$(join <($B/visflags "$ref" 2>/dev/null | sed -E 's/.*mat=([^ ]+) flags.*count=([0-9]+).*/\1 \2/' | sort) <($B/visflags "$out" 2>/dev/null | sed -E 's/.*mat=([^ ]+) flags.*count=([0-9]+).*/\1 \2/' | sort) | awk '{d=$3-$2; s=(d==0)?"":sprintf("%s%+d",$1,d); if(s!="") printf "%s ", s; n++; if(d==0) e++} END {printf "(%d/%d visuals exact)", e, n}')
  overlap=$($B/uv1overlap "$out" 512 2>/dev/null | grep -o 'conflicting texels=[0-9]*' | head -1)
  # N/U/V exactness over matched corners, summed over materials
  nuv=$($B/visflags "$ref" 2>/dev/null | sed -E 's/.*mat=([^ ]+) flags.*/\1/' | sort -u | while read -r stem; do $B/partdiff "$ref" "$out" "$stem" 0 2>/dev/null | grep 'corner values bit-exact'; done | awk '{gsub(/[()%]/," "); split($5,n,"/"); split($8,u,"/"); split($11,v,"/"); N+=n[1]; U+=u[1]; V+=v[1]; T+=n[2]} END {if(T>0) printf "N %.1f%% U %.1f%% V %.1f%% of %d corners", 100*N/T, 100*U/T, 100*V/T, T}')
  echo "$name: struct-diff-lines=$structdiff | verts: $verts | $overlap | $nuv"
done < "$PAIRS"
