#!/bin/bash
# ghost-splice-audit.sh — the corpus-wide published-ghost telemetry audit.
#
#   bash tools/ghost-splice-audit.sh > audit.tsv
#
# Catches: *a published ghost whose telemetry is another driver's.* Walks every
# map's replays/ directory and grades each of our files against the human
# recordings held for that map, emitting one TSV row per file (map, file, md5,
# reference, identical samples, max divergence, re-convergences, verdict).
#
# RUNS ON THE RENDER BOX. The paths are WhiteStick's — REPO is a checkout of this
# repo at /tmp/repo, and EXTRA points at downloaded human ghosts that the repo
# does not ship. The logic is the point; repoint the three variables below.
#
# ONE TOLERANCE CAVEAT, added on publication. Tests 1 and 3 below ask for
# distance EXACTLY 0, which is structurally blind to a float re-encode: a copy
# that has been through one is not bit-identical and reads as independent. See
# `nearident` in the crate, which asks the same question at 1 mm and is the
# instrument to reach for now. Test 2 — re-convergence, the one that actually
# proves a splice — is unaffected. This script is published as the audit that
# was run, so its numbers can be reproduced.
# audit_all.sh -- splice/contamination audit of every published ghost.
#
# ===========================================================================
# METHOD, so the reader can judge it
# ===========================================================================
#
# A ghost is an input tape PLUS a recorded trajectory. They are separate
# payloads: the oracle validates the tape, a video draws the record. This audit
# asks only about the RECORD -- whether the telemetry a viewer would see is our
# car's, or somebody else's.
#
# TEST 1 -- shared bit-identical prefix. WEAK ON ITS OWN.
#   The sim is deterministic, so two runs with the same opening inputs produce
#   identical f32 positions. Our own variant tapes do it routinely: on 203072,
#   KEYBOARD vs our own TAS is 67 % bit-identical. A shared prefix alone
#   therefore proves nothing and must never be reported as contamination.
#
# TEST 2 -- RE-CONVERGENCE. This is the proof.
#   Once two runs are more than DIVERGE_M apart they are different physical
#   states, and no sequence of inputs returns them to EXACTLY 0.000000 m.
#   identical -> diverge -> exactly identical again can only be a splice.
#
# TEST 3 -- WHOLESALE IDENTITY with a human recording.
#   Distinct from a prefix: if ~all samples are exactly 0 against a human's
#   ghost, the file simply IS that human's record (possibly truncated). This
#   catches a splice whose second seam was cut off, which Test 2 cannot see.
#
# WHO IS A TRUSTWORTHY REFERENCE
#   Only a HUMAN recording. Same-pipeline siblings can trip Test 2 as well
#   (203072 KEYBOARD vs our own TAS re-converges after 9 m), so sibling
#   comparisons are excluded rather than reported as evidence.
#
# HONEST EXCEPTION
#   Files whose own names declare them derived from a human run -- AUTHORCUT,
#   HUMANCUT, AUTHORMIN, AUTHOR_LAP -- are SUPPOSED to match a human's record.
#   High identity there is the label being truthful, and is reported as
#   DERIVED-AS-LABELLED, not as contamination.
#
# UNTESTABLE
#   A map with no human recording cannot be tested this way at all.
#   NO-HUMAN-REFERENCE means UNTESTED. It does not mean clean.
set -u
TJ="$HOME/trackmania-tas/tools/tmtraj/target/release"
[ -x "$TJ/tmtrajcheck" ] || { TJ=/mnt/c/Users/vjeux/tj/target/release; echo "[warn] using the STALE toolkit at $TJ -- build tools/tmtraj" >&2; }
REPO=/tmp/repo
V=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/v
DIVERGE_M=5.0
IDENT_PCT=90        # "wholesale identity" bar for Test 3

# human references we hold that the repo does not ship
declare -A EXTRA
EXTRA[203072]="$V/203072/90_WR_12083.Ghost.Gbx $V/203072/91_RANK2_12242.Ghost.Gbx"
EXTRA[227969]="$V/227969/90_WR_Titoch_8197.Ghost.Gbx"
EXTRA[252289]="$V/252289/90_WR_KevinMagPizza_3867.Ghost.Gbx"
EXTRA[285885]="$V/285885/90_WR_61229.Ghost.Gbx"

is_ref(){ echo "$1" | grep -Eqi 'wr|human|rank|author'; }          # case-INsensitive
is_derived(){ echo "$1" | grep -Eqi 'authorcut|humancut|authormin|author_lap|author_at'; }

pair(){
  "$TJ/sep" "$1" "$2" 2>/dev/null | awk -F'\t' -v D="$DIVERGE_M" '
    BEGIN{mx=-1}
    NR>1 { n++; d=$2+0
           if (d==0) { ident++; if (diverged) recon++ }
           else if (d>D) { diverged=1; if (d>mx) mx=d } }
    END { printf "%d %d %.2f %d", ident+0, n+0, (mx<0?0:mx), recon+0 }'
}

printf 'map\tfile\tmd5\treference\tident\ttotal\tident_pct\tmax_diverge_m\treconverged\tverdict\n'
for d in "$REPO"/*/replays; do
  page=$(basename "$(dirname "$d")"); map=${page%%-*}
  refs=""
  for r in "$d"/*.Ghost.Gbx; do is_ref "$(basename "$r")" && refs="$refs $r"; done
  refs=$(echo $refs ${EXTRA[$map]:-})
  for f in "$d"/*.Ghost.Gbx; do
    b=$(basename "$f"); md5=$(md5sum "$f" | cut -c1-32)
    if is_ref "$b"; then
      k=REFERENCE; is_derived "$b" && k=DERIVED-AS-LABELLED
      printf '%s\t%s\t%s\t(itself a human-derived file)\t-\t-\t-\t-\t-\t%s\n' "$map" "$b" "$md5" "$k"; continue
    fi
    if [ -z "$refs" ]; then
      printf '%s\t%s\t%s\t-\t-\t-\t-\t-\t-\tNO-HUMAN-REFERENCE\n' "$map" "$b" "$md5"; continue
    fi
    # keep the most incriminating reference: re-convergence first, then identity
    best=""; bref=""; brec=-1; bpct=-1
    for r in $refs; do
      set -- $(pair "$f" "$r")
      pct=$(awk -v i="$1" -v n="$2" 'BEGIN{printf "%.0f", (n>0)?100*i/n:0}')
      if [ "$4" -gt "$brec" ] || { [ "$4" -eq "$brec" ] && [ "$pct" -gt "$bpct" ]; }; then
        brec=$4; bpct=$pct; best="$1 $2 $pct $3 $4"; bref=$(basename "$r")
      fi
    done
    set -- $best
    if [ "$5" -gt 0 ]; then v=CONTAMINATED-SPLICE
    elif [ "$3" -ge "$IDENT_PCT" ]; then v=CONTAMINATED-IS-THE-HUMAN-RECORD
    else v=CLEAN; fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s%%\t%s\t%s\t%s\n' "$map" "$b" "$md5" "$bref" "$1" "$2" "$3" "$4" "$5" "$v"
  done
done
