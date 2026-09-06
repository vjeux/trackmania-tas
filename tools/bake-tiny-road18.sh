#!/bin/sh
# Bake RoadTechRampLow2 -> Tiny_Road_18 equivalent (transfer validation).
# Same mechanisms as bake-tiny-road17.sh; per-block params (t, orders,
# surf, thetas) fitted on Road_18. Residuals: Trims -7, Road +6,
# Technics +2, TB +4 (same knife-edge families as Road_17).
# Usage: bake-tiny-road18.sh SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]
# (run from tools/ so target/release/mapgeom resolves).
set -e
SRC=${1:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
REF=${2:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
OUT=${3:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
IDENT=${4:?usage: $0 SRC.ITEM.GBX REF.ITEM.GBX OUT.ITEM.GBX IDENT [PACKS_DIR]}
PACKS=${5:-/tmp}
export TINY_SEED_LARGEST=1 TINY_VPRIM=1 TINY_USMOOTH=1 TINY_UKEY=1 TINY_UKEYQ=1
export TINY_UMODE_MAP="TechnicsTrims:du,Technics:du,TrackBorders:duoff,RoadTech:range"
export TINY_TAN_MAP="Technics:16.4"
export TINY_WEIGHT_MAP="TrackBorders:angle,Technics:angle,TechnicsTrims:angle"
export TINY_POS_REF="$REF"
export TINY_CREASE_MAP="TrackBorders:58,TechnicsTrims:45,Technics:52,RoadTech:20,LightSpot:44,DecalMarksRamp:44"
export TINY_MATERIAL_ORDER="DecalMarksRamp,DecalPaint2Logo4x1,TechnicsTrims,RoadTech,LightSpot,Technics,TrackBorders,TrackWallClips"
export TINY_SURF_ORDER="4,3,1,0,7,6,2,8,5"
export TINY_POS_T="b8ae0000,b7000000,bcbe2c00"
export TINY_UV1_REF="$REF"
export TINY_U02="49.840961"
export TINY_FILETIME="134124532993945037"
exec target/release/mapgeom --packs "$PACKS" static-item "$SRC" --out "$OUT" --ident "$IDENT" --author "$IDENT" --scale 0.5 --collection 26
