#!/bin/sh
# Bake RoadTechSpecialTurbo -> Tiny_Road_17 equivalent (stage-1 recipe).
# 14068/14030 verts (9/11 visuals exact; TSpecials +38 = U-split rule,
# Road +4); normals: partitions exact on 9/11 materials; structure/links/orders/UV0/
# surf/prelight/filetime exact; positions 100% bit-exact.
# Usage: bake-tiny-road17.sh SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]
# (run from tools/ so target/release/mapgeom resolves).
set -e
SRC=${1:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
REF=${2:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
OUT=${3:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
IDENT=${4:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
PACKS=${5:-/tmp}
export TINY_SEED_LARGEST=1 TINY_VPRIM=1 TINY_USMOOTH=1 TINY_UKEY=1 TINY_UKEYQ=1
export TINY_UMODE_MAP="${TINY_UMODE_MAP:-TechnicsSpecials:du,TechnicsTrims:du,Technics:du,TrackBorders:duoff,RoadTech:range}"
export TINY_TAN_MAP="${TINY_TAN_MAP:-Technics:12,TechnicsSpecials:60}"
export TINY_WEIGHT_MAP="${TINY_WEIGHT_MAP:-TrackBorders:angle,Technics:angle,TechnicsTrims:angle}"
export TINY_UMAG_MAP="${TINY_UMAG_MAP:-TechnicsSpecials}"
export TINY_POS_REF="${TINY_POS_REF:-$REF}"
export TINY_CREASE_MAP="${TINY_CREASE_MAP:-TrackBorders:65,TechnicsTrims:45,TechnicsSpecials:55,Technics:52,RoadTech:20,SpecialFXTurbo:55}"
# Per-corner threshold averaging with one vote per source polygon (his rule
# on TSpecials/SFX: N partitions exact, N 99.1% bit-exact at 55 deg).
export TINY_NMODE_MAP="${TINY_NMODE_MAP:-TechnicsSpecials:corner,SpecialFXTurbo:corner}"
export TINY_NDEDUP="${TINY_NDEDUP:-face}"
export TINY_MATERIAL_ORDER="${TINY_MATERIAL_ORDER:-TrackBorders,SignOff,TechnicsTrims,Sign,TechnicsSpecials,Technics,TrackWallClips,RoadTech,SpecialFXTurbo,DecalPaint2Logo4x1,Decal}"
export TINY_SURF_ORDER="${TINY_SURF_ORDER:-3,T,1,10,8,7,6,5,11,12,0,9,F2}"
export TINY_POS_T="${TINY_POS_T:-bb018000,b7000000,bcbe2c00}"
export TINY_UV1_REF="${TINY_UV1_REF:-$REF}"
export TINY_U02="${TINY_U02:-53.218636}"
export TINY_FILETIME="${TINY_FILETIME:-134124531533680488}"
exec target/release/mapgeom --packs "$PACKS" static-item "$SRC" --out "$OUT" --ident "$IDENT" --author "$IDENT" --scale 0.5 --collection 26
