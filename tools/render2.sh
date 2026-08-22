#!/bin/bash
# render2.sh -- TAS and world record in ONE scene, camera on the TAS car.
#
#   bash render2.sh <mapid> <tas-ghost> <wr-ghost> <race_ms> [out-basename]
#   bash render2.sh 227969 03_best_8010.Ghost.Gbx 90_WR_Titoch_8197.Ghost.Gbx 8010 cmp227969
#
# Prints the mp4 path and exits 0, or a reason and non-zero. Like render.sh:
# no OCR, no screenshots, nothing judged by eye.
#
# TWO GHOSTS IN ONE CLIP: SETTLED, WITH THE EVIDENCE
# --------------------------------------------------
# This was recorded as "unresolved -- /mtclip shows both entity blocks active
# but only the camera-targeted car appears", and OpponentVisibility was tried as
# a fix. Both conclusions were wrong. The test had been run on untitled 02,
# where sep(1) shows the two tapes are 0.00 m apart for the first 5 s of a 9 s
# run and 9.5 m apart only at the very end -- by which point the second car is
# behind the camera. "Not drawn" and "in the same place" were the same picture.
#
# Retested on 227969, where the runs stay ~2 m apart from t=3 s to t=7 s: BOTH
# CARS ARE DRAWN throughout, and the skins tell them apart (magenta TAS vs the
# WR holder's own). OpponentVisibility was never needed.
#
# So before filming, this script MEASURES the separation and refuses a pairing
# that would silently look like one car.
#
# IMPORT ORDER MATTERS: the TAS goes in FIRST, so it is ghost 1 and the camera
# follows it. The WR is the opponent.
set -u
TV=/mnt/c/Users/vjeux/tm-video
V=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/v
MAPS=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Downloaded/tas
SS=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/ScreenShots
PS=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter
GB="/mnt/c/Users/vjeux/game-bot-cli/GameBot/bin/Release/net6.0-windows/GameBot.exe"
FFB=/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin
# The toolkit. Prefer the repo's own build -- tools/tmtraj is the maintained
# source and the only copy that gets fixes. /mnt/c/Users/vjeux/tj is an older
# hand-built tree that happened to be what these scripts pointed at, and on
# 2026-08-21 the two had drifted far enough to change a verdict: the stale
# tree's C3 measured a position step in METRES and refused 197047's regenerated
# tape for a 620 m respawn jump, while the maintained C3 measures the implied
# SPEED and passes the same file with no failures at all. A clip was filmed on a
# named override that the current gate does not need. Its nearident was three
# versions behind as well, and would still print a verdict having compared
# nothing.
#
# So: build with `cd tools/tmtraj && cargo build --release`, and let the old
# tree be the fallback rather than the default.
TJ_REPO="$HOME/trackmania-tas/tools/tmtraj/target/release"
TJ_OLD=/mnt/c/Users/vjeux/tj/target/release
if [ -x "$TJ_REPO/tmtrajcheck" ]; then TJ="$TJ_REPO"; else
  TJ="$TJ_OLD"
  echo "[warn] using the STALE toolkit at $TJ_OLD -- build tools/tmtraj to get the maintained one" >&2
fi
OVL=/mnt/c/Users/vjeux/ovl/target/release/ovl
OUTD=$TV/renders_final
mkdir -p "$OUTD" "$PS"

MAP=${1:?need mapid}; TAS=${2:?need tas ghost}; WR=${3:?need wr ghost}
RACE=${4:?need race_ms}; OUT=${5:-cmp$MAP}

log(){ echo "[$(date +%H:%M:%S)] $*"; }
die(){ log "FAIL: $*"; exit 1; }
OP(){ curl.exe -sS -m "${2:-20}" "http://127.0.0.1:29800$1" 2>/dev/null | tr -d '\r'; }
C(){ "$GB" click "$1" "$2" >/dev/null 2>&1; }
wpath(){ echo "$1" | sed 's#^/mnt/c/#C:/#'; }
ctxnum(){ OP /ctx | grep -oE '"ctx":[0-9]+' | cut -d: -f2; }
await_ctx(){ local w="$1" s="${2:-40}" i; for i in $(seq 1 "$s"); do [ "$(ctxnum)" = "$w" ] && return 0; sleep 1; done; return 1; }
rowy(){ echo $((545+($1-1)*72)); }
# picker row = position in the folder's sorted listing, NOT the NN_ filename prefix
rowof(){ ls "$V/$1" | sort | grep -n "^$2$" | cut -d: -f1; }

TP="$V/$MAP/$TAS"; WP="$V/$MAP/$WR"
[ -f "$TP" ] || die "no such TAS ghost: $TP"
[ -f "$WP" ] || die "no such WR ghost: $WP"
[ -f "$MAPS/$MAP.Map.Gbx" ] || die "no map: $MAPS/$MAP.Map.Gbx"

# ---- 0. gate ----------------------------------------------------------------
# OUR ghost must pass (lone C5 tolerated -- the named contact-byte withdrawal).
# THEIR ghost is a downloaded human recording, so a failure is reported and is
# NOT disqualifying: we did not generate it, its telemetry is the ground truth
# we want on screen, and it is exactly what must not be regenerated.
GATE=$("$TJ/tmtrajcheck" "$TP" --race "$RACE"); GC=$?
echo "$GATE" | sed 's/^/    /'
if [ "$GC" -ge 2 ]; then
    NF=$(echo "$GATE" | grep -c '^ *FAIL')
    C5=$(echo "$GATE" | grep -c '^ *FAIL C5')
    CX=$(echo "$GATE" | grep -c '^ *FAIL C10')
    # exit 1 with ZERO failures is PUBLISHABLE WITH WARNINGS -- a pass.
    # Demanding exactly one NAMED failure refused the cleanest file of the
    # batch for being clean.
    IDS=$(echo "$GATE" | grep '^ *FAIL' | awk '{print $2}' | paste -sd, -)
    OTHER=$(echo "$GATE" | grep '^ *FAIL' | awk '{print $2}' | grep -vcE '^C(5|6|7|10)$')
    if [ "$NF" -eq 0 ]; then
      log "gate: PUBLISHABLE WITH WARNINGS (0 failures)"
    elif [ "$OTHER" -eq 0 ]; then
      log "gate: contact-byte family only ($IDS) -- byte 89 is measured-irreducible; filming, verdict logged"
    else
      # Same named-override convention as render.sh: GATE_OVERRIDE="C8=<reason>"
      # films a ghost whose ONLY failures are the ones named, and refuses if any
      # other id appears. The case it was written for is a SnowCar map: C8 reads
      # "these wheel bytes are another run's" because the check knows one wheel
      # radius -- the Stadium car's 0.36 m -- and the SnowCar's wheels measure
      # 0.4697 m. The check is right about the number and wrong about what it
      # means.
      OVR="${GATE_OVERRIDE:-}"; OVRIDS="${OVR%%=*}"; OVRWHY="${OVR#*=}"
      if [ -n "$OVR" ] && [ "$OVRIDS" = "$IDS" ]; then
        log "gate: OVERRIDDEN for exactly $IDS -- $OVRWHY"
        log "  (a refused ghost filmed on a named, recorded exception; the page must say so)"
      else
        [ -n "$OVR" ] && log "gate: override names [$OVRIDS] but the failures are [$IDS] -- not applying it"
        die "the gate refuses this ghost ($NF failures: $IDS) -- not filmable"
      fi
    fi
else
  log "gate: PUBLISHABLE"
fi
log "--- the WR ghost is a human recording; its gate result is FYI, not a veto:"
"$TJ/tmtrajcheck" "$WP" 2>&1 | sed 's/^/    /' || true

# ---- 0b. would the two cars be distinguishable? -----------------------------
# A pairing that never separates renders as one car and teaches the viewer
# nothing. Measure it rather than hoping.
SEPST=$("$TJ/sep" "$TP" "$WP" 2>/dev/null | awk -F'\t' '
  NR>1 { n++; s+=$2; if ($2>mx) mx=$2; if ($2>=1.5 && $2<=40) vis++ }
  END { printf "%d %.2f %.2f %d", n, mx, s/n, vis }')
read -r SN SMAX SMEAN SVIS <<<"$SEPST"
log "separation: $SN paired samples, max ${SMAX}m, mean ${SMEAN}m, ${SVIS} samples in the 1.5-40m 'two visible cars' band"
# vjeux, 2026-08-21: TWO CARS IS THE DEFAULT AND A TINY GAP IS NOT A REASON TO
# DROP THE COMPARISON. "I want to see the difference, otherwise we don't know
# what the human record did differently than the tas." On a map like 270053 --
# 973 recorded runs, six players tied at 4.495, six centimetres at the line --
# the overlap IS the content: the viewer watches the gap fail to appear for four
# and a half seconds and then appear at the flag. So a tight pairing is filmable
# ON PURPOSE, with TIGHT=1, and the prose under the clip must say the cars
# overlap. Without TIGHT=1 the old refusal stands, so an unattended batch still
# cannot ship a two-car clip that silently looks like one car.
if awk -v v="$SVIS" 'BEGIN{exit (v>=10)?0:1}'; then
  log "separation: ${SVIS} samples in the visible band -- the two cars read as two cars"
elif [ "${TIGHT:-0}" = "1" ]; then
  log "separation: TIGHT=1 -- only ${SVIS} samples in the visible band, max ${SMAX}m."
  log "  Filming anyway: the overlap is the story. The page MUST say so under the clip."
else
  die "the two runs never separate enough to look like two cars (${SVIS} usable samples) -- pass TIGHT=1 if the overlap is deliberately the point"
fi

read -r _ _ _ _ TSPAN <<<"$("$TJ/inputcount" --meta "$TP")"
WANT=$(awk -v s="$TSPAN" 'BEGIN{printf "%.2f", s/1000}')
log "$MAP: TAS=$TAS vs WR=$WR, record span ${WANT}s"

# ---- 1. load the map ---------------------------------------------------------
if [ "$(ctxnum)" != "0" ]; then
  log "leaving current editor"
  for i in $(seq 1 20); do
    [ "$(ctxnum)" = "0" ] && break
    OP /back >/dev/null; sleep 2
    D=$(OP /dismiss 20); [ "$D" = "none" ] || log "  dialog: $D"   # DECLINES the save
    sleep 2
  done
  await_ctx 0 30 || die "could not get back to the menu"
fi
printf 'C:\\Users\\vjeux\\OneDrive\\Documents\\Trackmania\\Maps\\Downloaded\\tas\\%s.Map.Gbx' "$MAP" > "$PS/editmap.txt"
log "loading map $MAP"
R=$(OP /editmap 30); case "$R" in ok*) : ;; *) die "editmap: $R" ;; esac
MB=$(( $(stat -c%s "$MAPS/$MAP.Map.Gbx") / 1048576 ))
LOADW=$(( 30 + MB * 30 )); [ "$LOADW" -lt 90 ] && LOADW=90; [ "$LOADW" -gt 300 ] && LOADW=300
# MAP_LOADW overrides the size-derived wait. File size is a poor proxy for how
# long the editor takes to build a map: 173691 "Underwater" is 1.9 MB and holds
# 77,688 free water blocks, so it builds far more slowly than a 1.9 MB track of
# ordinary road. A timeout here aborts the shoot, which is the safe direction --
# but re-running with a longer wait is the fix, not working around it.
[ -n "${MAP_LOADW:-}" ] && LOADW="$MAP_LOADW"
log "waiting up to ${LOADW}s for a ${MB}MB map to load"
await_ctx 1 "$LOADW" || die "map did not open within ${LOADW}s"

# ---- 2. MediaTracker ---------------------------------------------------------
for try in 1 2 3; do
  C 3343 2075; sleep 5
  OP /mtingame >/dev/null; sleep 8
  [ "$(ctxnum)" = "2" ] && break
  log "  MediaTracker attempt $try did not take"
done
[ "$(ctxnum)" = "2" ] || die "could not enter the MediaTracker"
log "in MediaTracker"

# ---- 3. import BOTH ghosts, TAS first ----------------------------------------
MIDX=$(ls "$V" | sort | grep -n "^${MAP}\$" | cut -d: -f1)
[ -n "$MIDX" ] || die "no ghost folder for $MAP"
MPAGE=$(( (MIDX-1)/12 + 1 )); MROW=$(( (MIDX-1)%12 + 1 ))
import_row(){
  local row="$1" p gp gr k
  OP /importghosts >/dev/null; sleep 3
  C 1912 835; sleep 2                       # v\
  p=1; while [ $p -lt $MPAGE ]; do C 2059 1440; sleep 3.5; p=$((p+1)); done
  C 1912 "$(rowy $MROW)"; sleep 2           # the map folder
  # THE GHOST LIST PAGES AT TWELVE ROWS TOO. I paged the folder list and not
  # this one, so on 228607 -- 17 files once the tail cuts were staged -- rows
  # 13-17 were off the first page. rowy(13) is y1409, below the last row, so
  # the click landed on nothing and the import silently did nothing.
  gp=$(( (row-1)/12 + 1 )); gr=$(( (row-1)%12 + 1 )); k=1
  while [ $k -lt $gp ]; do C 2059 1440; sleep 3.5; k=$((k+1)); done
  [ "$gp" -gt 1 ] && log "    ghost row $row -> picker page $gp row $gr"
  C 1912 "$(rowy $gr)";  sleep 1.5          # the ghost row
  C 1912 1716; sleep 5                      # OPEN
}
OP /rmtracks >/dev/null; sleep 2
TROW=$(rowof "$MAP" "$TAS"); WROW=$(rowof "$MAP" "$WR")
[ -n "$TROW" ] && [ -n "$WROW" ] || die "could not resolve picker rows (TAS=$TROW WR=$WROW)"
# THE CAMERA IS ON OUR CAR. THIS IS NOT A PARAMETER.
#
# There used to be a CAMON=wr escape hatch here that imported the human first so
# the chase camera followed THEIR car. It was added for a real problem — a ghost
# camera block lasts exactly as long as its sample stream, our tapes are
# truncated at the finish, and a downloaded human recording keeps sampling for
# half a second past it, so on 270053 the shot died at 4.383 and the last 0.6 s
# was an empty view of the map.
#
# The hatch was the wrong fix, and it kept getting used: 279197 was shipped with
# the camera bolted to ShcrTM's car to avoid 50 ms of dead tail on a 10.594 s
# clip. The clip belongs to OUR run. A human still driving 8 ms behind at the
# flag is the whole point of the shot, not an inconvenience.
#
# The right fix for a short tail is to TRIM THE CLIP, which costs a frame or two
# and is already done elsewhere in this pipeline. So: the TAS ghost is always
# imported first, the camera always follows it, and this script now refuses to
# do anything else. If our camera block genuinely cannot cover our own finish,
# that is a defect in the recording — fix the recording, do not move the camera.
log "importing TAS (row $TROW) then WR (row $WROW) -- camera on OUR car, always"
if [ -n "${CAMON:-}" ] && [ "${CAMON}" != "tas" ]; then
  die "CAMON=${CAMON} is no longer supported: the camera is always on the TAS car (see the note above this line)"
fi
import_row "$TROW"
import_row "$WROW"

CLIP=$(OP /mtclip 25)
echo "$CLIP" | sed 's/^/    | /'
NB=$(echo "$CLIP" | grep -c CGameCtnMediaBlockEntity)
[ "$NB" -ge 2 ] || die "only $NB ghost(s) imported -- a one-car clip is not a comparison"
END=$(echo "$CLIP" | grep -oE 'end=[0-9.]+' | head -1 | cut -d= -f2)
EXPECT="$WANT"
awk -v e="${END:-0}" -v w="$EXPECT" 'BEGIN{d=e-w; if(d<0)d=-d; exit (d<1.5)?0:1}' \
  || die "first block end=${END}s, expected the camera-target span ${EXPECT}s -- wrong import order?"
CLIPEND=$(echo "$CLIP" | grep -oE 'end=[0-9.]+' | cut -d= -f2 | sort -g | tail -1)

# ---- container identity, free, from the game itself --------------------------
# The MediaTracker names each imported track after the NICKNAME STORED IN THE
# GHOST FILE. Our own tapes come back as "Ghost:TAS"; a tape built on someone
# else's recording comes back wearing THEIR name. 199100's regenerated arc
# imported as "Ghost:OrmeEssence44" -- the previous world-record holder -- which
# is how we learned that file was our telemetry inside his container.
#
# THIS IS A DETECTOR, NOT A CLEARANCE. 165922's nine tapes display "TAS" and
# still carry a real player's account id underneath, because the nickname was
# scrubbed and the account was not. Passing this check means "the game does not
# say it is somebody else's"; it does not mean the container is ours. The
# account-id read is the backstop and it lives outside this pipeline.
# ALLOW_FOREIGN_CONTAINER=1 films a tape that is DELIBERATELY somebody else's
# container -- the case this was written for is a GRAFT: the clip's inputs
# written into the donor's own input array, which is the only way to compare
# "his run" with "his run plus these inputs" on his own map. Both cars then
# import under HIS nickname and none under Ghost:TAS, so the check below fires
# correctly and must be waived explicitly rather than dodged by scrubbing the
# nickname (165922 scrubbed the name and still carried the player's account id).
if [ "${ALLOW_FOREIGN_CONTAINER:-0}" != "1" ]; then
  OURNAME=$(echo "$CLIP" | grep -oE 'name=Ghost:[^ ]*' | grep -v -F -e 'Ghost:TAS' | head -1)
  if [ -n "$OURNAME" ] && [ "$(echo "$CLIP" | grep -c 'name=Ghost:TAS')" = "0" ]; then
    die "container identity: the game imported this as ${OURNAME#name=} -- our tapes import as Ghost:TAS, so this file is wearing someone else's container"
  fi
fi
log "both ghosts in; camera target ends ${END}s, longest block ${CLIPEND}s -- the clip must run to the LONGEST"

# ---- 4. camera: the stock External chase, on ghost 1 -------------------------
C 305 1656; sleep 2                         # Tracks +
C 1269 671; sleep 3                         # Player camera
for i in $(seq 1 12); do
  CAM=$(OP /cam 20); echo "$CAM" | grep -q 'entid=0' || break
  C 492 322; sleep 1.5
done
CAM=$(OP /cam 20); log "camera: $CAM"
echo "$CAM" | grep -q '^cam ' || die "no camera block"
echo "$CAM" | grep -q 'entid=0' && die "camera targets nobody -- would render black"
for i in $(seq 1 6); do
  echo "$CAM" | grep -q 'gamecam=2' && break
  C 492 385; sleep 1.5; CAM=$(OP /cam 20)
done

# ---- 5. shoot -----------------------------------------------------------------
LAST=$(ls -t "$SS"/Video*.webm 2>/dev/null | head -1)
OP /rewind >/dev/null; sleep 5
C 852 2093; sleep 3                         # AVI
C 1747 1743                                 # OK
log "rendering..."
NEW=""
for i in $(seq 1 720); do
  NEW=$(ls -t "$SS"/Video*.webm 2>/dev/null | head -1)
  [ -n "$NEW" ] && [ "$NEW" != "$LAST" ] && break
  sleep 5
done
{ [ -z "$NEW" ] || [ "$NEW" = "$LAST" ]; } && die "no video appeared"
for i in $(seq 1 720); do
  a=$(stat -c%s "$NEW" 2>/dev/null); sleep 5; b=$(stat -c%s "$NEW" 2>/dev/null)
  [ "$a" = "$b" ] && [ "${a:-0}" -gt 100000 ] && break
done
cp "$NEW" "$OUTD/$OUT.webm"
log "filmed $(stat -c%s "$OUTD/$OUT.webm")B"

# ---- 6. overlay (OUR inputs -- the point of the comparison) -------------------
CSV="$OUTD/$OUT.csv"
"$TJ/inputcount" --csv "$TP" > "$CSV" || die "inputcount failed"
NFR=$("$FFB/ffprobe.exe" -v error -count_frames -select_streams v \
      -show_entries stream=nb_read_frames -of csv=p=0 "$(wpath "$OUTD/$OUT.webm")" | tr -d '\r\n ')
case "$NFR" in ''|*[!0-9]*) die "unreadable frame count [$NFR]" ;; esac
"$OVL" "$CSV" 30 "$NFR" "$OUTD/ovl.rgba" "$RACE" 2>/dev/null || die "overlay failed"
MP4="$OUTD/$OUT.mp4"
"$FFB/ffmpeg.exe" -y -loglevel error -i "$(wpath "$OUTD/$OUT.webm")" \
  -f rawvideo -pixel_format rgba -video_size 900x200 -framerate 30 -i "$(wpath "$OUTD/ovl.rgba")" \
  -filter_complex "[1:v]scale=900:200[o];[0:v][o]overlay=x=(W-900)/2:y=H-215:format=auto" \
  -c:v libx264 -preset veryfast -crf 22 -pix_fmt yuv420p -movflags +faststart "$(wpath "$MP4")"
rm -f "$OUTD/ovl.rgba"
[ -s "$MP4" ] || die "compositing produced nothing"

# ---- 7. accept or reject, numerically -----------------------------------------
DUR=$("$FFB/ffprobe.exe" -v error -show_entries format=duration -of csv=p=0 "$(wpath "$MP4")" | tr -d '\r\n ')
BLK=$("$FFB/ffmpeg.exe" -v error -i "$(wpath "$MP4")" -vf blackdetect=d=0.5:pix_th=0.10 -f null - 2>&1 | grep -ci black_start || true)
SZ=$(stat -c%s "$MP4")
log "duration=${DUR}s expected~${CLIPEND}s (TWO-CAR: longest block, not our race time) size=${SZ}B blackspans=${BLK:-0}"
awk -v d="${DUR:-0}" -v w="$CLIPEND" 'BEGIN{x=d-w; if(x<0)x=-x; exit (x<1.5)?0:1}' || die "duration ${DUR}s does not match the longest block ${CLIPEND}s"
[ "${SZ:-0}" -gt 200000 ] || die "suspiciously small: ${SZ}B"
[ "${BLK:-0}" -eq 0 ] || die "contains ${BLK} black span(s)"
log "OK $MP4"
echo "$MP4"
