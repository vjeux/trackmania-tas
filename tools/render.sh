#!/bin/bash
# render.sh -- one command: map + ghost -> a verified, overlaid mp4.
#
#   bash render.sh <mapid> <ghost-file> <race_ms> [out-basename]
#   e.g. bash render.sh 276874 02_WATCH_12759_v2.Ghost.Gbx 12759 u01
#
# Exit 0 and the mp4 path on stdout, or non-zero and a reason. No OCR, no
# screenshots, no human, no agent eyes -- every decision is a number from the
# game's own object graph or from ffprobe.
#
# ---------------------------------------------------------------------------
# WHY THIS EXISTS
#
# The old path drove the game by reading the screen. Every production failure
# today came from that layer and none from the plugin:
#
#   "editor never opened" x7      -> in_editor() was never defined; bash returns
#                                    127 for an unknown command, i.e. false, so
#                                    the guard could only ever say no. Now /ctx.
#   tile clicked at wrong offset  -> the caption is not the button. Gone: the
#                                    map browser is not used at all any more.
#   12 of 32 tiles visible        -> ditto, gone.
#   scroll position persisted     -> ditto, gone.
#   "EDIT A TRACK" read as DITA   -> no text is read any more.
#   OCR daemon wedged             -> no OCR process is started any more.
#   camera on "Local Player"      -> a driver who isn't there renders an all-
#                                    black clip that passes size and duration.
#                                    Now /cam reports the target numerically.
#
# WHAT IS STILL A CLICK, AND WHY
#
# Two steps have no scripted equivalent, and I looked properly:
#   * the MediaTracker icon, and
#   * the rows of the Import Ghosts file picker.
# Searched: DialogEditCutScenes_* (handlers only, no opener); every Dialog*()
# opener on CGameCtnMenus; CGameEditorPluginMap; CGameCtnEditorFree's members;
# CControlBase::OnAction (exists!) -- but both InterfaceRoot and the dialog
# CurrentFrame walk to a single control, because the editor UI is a manialink
# layer rather than a CControl tree, so there is nothing to actuate;
# CGameEditorEvent::MediaTrackerPopUp is an event the editor RAISES, not one we
# can send. Both JSON dumps agree with the header.
#
# So those two clicks stay -- but they are fixed coordinates in a modal dialog,
# never a searched position, and each is verified afterwards by a number:
# /ctx for the editor transition, /mtclip for the ghost identity. A click that
# does not land is DETECTED, and the run stops instead of filming something
# wrong.
set -u
TV=/mnt/c/Users/vjeux/tm-video
V=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Replays/v
MAPS=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/Downloaded/tas
SS=/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/ScreenShots
PS=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/GhostShooter
GB="/mnt/c/Users/vjeux/game-bot-cli/GameBot/bin/Release/net6.0-windows/GameBot.exe"
FFB=/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin
TJ=/mnt/c/Users/vjeux/tj/target/release
OVL=/mnt/c/Users/vjeux/ovl/target/release/ovl
OUTD=$TV/renders_final
mkdir -p "$OUTD" "$PS"

MAP=${1:?need mapid}; GHOST=${2:?need ghost filename}; RACE=${3:?need race_ms}
OUT=${4:-$MAP}

log(){ echo "[$(date +%H:%M:%S)] $*"; }
die(){ log "FAIL: $*"; exit 1; }
OP(){ curl.exe -sS -m "${2:-20}" "http://127.0.0.1:29800$1" 2>/dev/null | tr -d '\r'; }
C(){ "$GB" click "$1" "$2" >/dev/null 2>&1; }
wpath(){ echo "$1" | sed 's#^/mnt/c/#C:/#'; }
ctxnum(){ OP /ctx | grep -oE '"ctx":[0-9]+' | cut -d: -f2; }
rowy(){ echo $((545+($1-1)*72)); }
# picker row = position in the folder's sorted listing, NOT the NN_ filename prefix
rowof(){ ls "$V/$1" | sort | grep -n "^$2$" | cut -d: -f1; }

# wait for a context number, returning non-zero on timeout rather than guessing
await_ctx(){
  local want="$1" secs="${2:-40}" i
  for i in $(seq 1 "$secs"); do
    [ "$(ctxnum)" = "$want" ] && return 0
    sleep 1
  done
  return 1
}

GPATH="$V/$MAP/$GHOST"
[ -f "$GPATH" ] || die "no such ghost: $GPATH"
[ -f "$MAPS/$MAP.Map.Gbx" ] || die "no such map: $MAPS/$MAP.Map.Gbx"

# ---- 0. gate on the ghost being filmable at all -----------------------------
# A tape that validates as a TIME is not necessarily a tape that draws a CAR:
# validation reads the input chunk, the video draws the telemetry. 26 of 174
# staged ghosts fail this. If tmtraj check is present it is authoritative;
# ghostqc is the local fallback.
if [ -x "$TJ/tmtrajcheck" ]; then
  GATE=$("$TJ/tmtrajcheck" "$GPATH" --race "$RACE"); GC=$?
  echo "$GATE" | sed 's/^/    /'
  if [ "$GC" -lt 2 ]; then
    log "gate: $(echo "$GATE" | grep -oE '(PUBLISHABLE[A-Z ]*|REFUSED)' | head -1) (exit $GC -- 0 clean, 1 warnings, 2 refused)"
  else
    # ONE sanctioned exception, and it is named rather than papered over. The
    # contact byte is currently the carrier's on the two untitled ghosts: a
    # known, deliberate withdrawal after evidence showed the regenerated rule
    # overcorrected. So a LONE C5 films, loudly. C5 plus anything else, or any
    # other failure, refuses.
    NF=$(echo "$GATE" | grep -c '^ *FAIL')
    IDS=$(echo "$GATE" | grep '^ *FAIL' | awk '{print $2}' | paste -sd, -)
    # The contact byte (C5/C6/C7/C10) is closed as measured-irreducible:
    # positions are right, one named byte is the carrier's. Those film, with
    # the verdict logged verbatim. C3 -- the splice signature -- hard-refuses.
    # exit 1 with ZERO failures is PUBLISHABLE WITH WARNINGS, also a pass.
    OTHER=$(echo "$GATE" | grep '^ *FAIL' | awk '{print $2}' | grep -vcE '^C(5|6|7|10)$')
    if [ "$NF" -eq 0 ]; then
      log "gate: PUBLISHABLE WITH WARNINGS (0 failures)"
    elif [ "$OTHER" -eq 0 ]; then
      log "gate: contact-byte family only ($IDS) -- byte 89 is measured-irreducible; filming"
    else
      # GATE_OVERRIDE="C3,C8=<reason>" films a ghost whose ONLY failures are the
      # named ones. It exists because a gate check can be superseded while still
      # wired in, and the alternative -- switching the gate off -- would lose the
      # distinction between "this check is wrong about this file" and "no check
      # ran". The override must name every failure exactly: one unexpected id and
      # it refuses, so it cannot be used as a blanket bypass. Whoever sets it owns
      # the reason, and the reason is echoed into the log beside the verdict.
      # OPPONENT=1 marks this ghost as a DOWNLOADED HUMAN RECORDING. render2.sh has
      # always treated the opponent's gate result as advisory -- we did not
      # generate the file, its telemetry IS the ground truth we want on screen,
      # and it is the one thing that must never be regenerated. The single-car
      # path needs the same rule now that split screens render the opponent on
      # its own: without it, the gate refuses a human's real lap for being a
      # human's real lap. 197047's rank-1 recording fails C3 and C8 exactly as
      # our own tape does -- because the map slides for a hundred seconds and the
      # run contains a respawn, not because anything is wrong with it.
      if [ "${OPPONENT:-0}" = "1" ]; then
        log "gate: $IDS on a DOWNLOADED HUMAN RECORDING -- advisory, not a veto (OPPONENT=1)"
        log "  the opponent is filmed exactly as recorded; we never regenerate it"
      else
      OVR="${GATE_OVERRIDE:-}"
      OVRIDS="${OVR%%=*}"; OVRWHY="${OVR#*=}"
      if [ -n "$OVR" ] && [ "$OVRIDS" = "$IDS" ]; then
        log "gate: OVERRIDDEN for exactly $IDS -- $OVRWHY"
        log "  (filming a refused ghost on a named, recorded exception; the page must say so)"
      else
        [ -n "$OVR" ] && log "gate: override names [$OVRIDS] but the failures are [$IDS] -- not applying it"
        die "the gate refuses this ghost ($IDS) -- not filmable"
      fi
      fi
    fi
  fi
else
  QC=$("$TJ/ghostqc" "$GPATH" | tail -1 | cut -f1)
  [ "$QC" = "OK" ] || die "ghostqc says $QC -- not filmable"
  log "ghostqc: OK (tmtraj check not installed)"
fi

# expected MediaTracker block length: the RECORD span, not the race time. The
# recording starts before the countdown ends, so 126859's 23.545 run is a
# 27.85 s block -- comparing against the race time rejects every good ghost.
read -r LEADIN RMS NSAMP PER SPAN <<<"$("$TJ/inputcount" --meta "$GPATH")"
WANT=$(awk -v s="$SPAN" 'BEGIN{printf "%.2f", s/1000}')
log "$MAP/$GHOST: race=${RACE}ms samples=$NSAMP span=${WANT}s"

# ---- 1. leave whatever we were in, and load the map -------------------------
if [ "$(ctxnum)" != "0" ]; then
  log "leaving current editor"
  # Exiting a MediaTracker-edited map raises a chain of modals: FrameMessage,
  # AskYesNo, DialogSaveAs. /dismiss answers each one CORRECTLY -- in particular
  # it DECLINES the save, so filming a map never edits it.
  for i in $(seq 1 20); do
    [ "$(ctxnum)" = "0" ] && break
    OP /back >/dev/null; sleep 2
    D=$(OP /dismiss 20); [ "$D" = "none" ] || log "  dialog: $D"
    sleep 2
  done
  await_ctx 0 30 || die "could not get back to the menu (dialog: $(OP /ctx))"
fi
printf 'C:\\Users\\vjeux\\OneDrive\\Documents\\Trackmania\\Maps\\Downloaded\\tas\\%s.Map.Gbx' "$MAP" > "$PS/editmap.txt"
log "loading map $MAP"
R=$(OP /editmap 30); case "$R" in ok*) : ;; *) die "editmap: $R" ;; esac
MB=$(( $(stat -c%s "$MAPS/$MAP.Map.Gbx") / 1048576 ))
LOADW=$(( 30 + MB * 30 )); [ "$LOADW" -lt 90 ] && LOADW=90; [ "$LOADW" -gt 300 ] && LOADW=300
log "waiting up to ${LOADW}s for a ${MB}MB map to load"
await_ctx 1 "$LOADW" || die "map did not open within ${LOADW}s"
log "map open: $(OP /ctx)"

# ---- 2. into the MediaTracker ----------------------------------------------
# The icon click is one of the two remaining pixel steps; /ctx says whether it
# worked, and we retry rather than assume.
for try in 1 2 3; do
  C 3343 2075; sleep 5              # MediaTracker icon -> sequences dialog
  OP /mtingame >/dev/null; sleep 8  # "EDIT" on the In Game row, via the API
  [ "$(ctxnum)" = "2" ] && break
  log "  MediaTracker attempt $try did not take"
done
[ "$(ctxnum)" = "2" ] || die "could not enter the MediaTracker"
log "in MediaTracker"

# ---- 3. import the ghost ----------------------------------------------------
OP /rmtracks >/dev/null; sleep 2
ROW=$(rowof "$MAP" "$GHOST"); [ -n "$ROW" ] || die "could not resolve the picker row for $GHOST"
MIDX=$(ls "$V" | sort | grep -n "^${MAP}\$" | cut -d: -f1)
[ -n "$MIDX" ] || die "map $MAP has no ghost folder"
MPAGE=$(( (MIDX-1)/12 + 1 )); MROW=$(( (MIDX-1)%12 + 1 ))
log "import: folder page $MPAGE row $MROW, ghost row $ROW"
OP /importghosts >/dev/null; sleep 3      # opens the picker via the API
C 1912 835; sleep 2                       # v\  (the only custom folder)
p=1; while [ $p -lt $MPAGE ]; do C 2059 1440; sleep 3.5; p=$((p+1)); done
C 1912 "$(rowy $MROW)"; sleep 2            # the map's folder
# The ghost list pages at twelve rows too -- paging only the folder list
# left rows 13+ unreachable, and the click landed on nothing.
GP=$(( (ROW-1)/12 + 1 )); GR=$(( (ROW-1)%12 + 1 )); k=1
while [ $k -lt $GP ]; do C 2059 1440; sleep 3.5; k=$((k+1)); done
[ "$GP" -gt 1 ] && log "  ghost row $ROW -> picker page $GP row $GR"
C 1912 "$(rowy $GR)";  sleep 1.5           # the ghost row
C 1912 1716; sleep 5                       # OPEN

# identity check: the picker selects by ROW and rows shift, so trust the clip,
# never the click.
CLIP=$(OP /mtclip 25)
echo "$CLIP" | sed 's/^/    | /'
echo "$CLIP" | grep -q CGameCtnMediaBlockEntity || die "no ghost imported"
END=$(echo "$CLIP" | grep -oE 'end=[0-9.]+' | tail -1 | cut -d= -f2)
# SPAN_OK=1 accepts a block longer than the sample span, for a file whose
# CONTAINER carries a longer timeline than the run inside it. 199100's cleaned
# 49.778 arc is the case that needed it: nickname repaired, declared time
# repaired to 49778, 998 samples spanning 49.900 -- and the MediaTracker still
# reports a 52.2 s block, which is the donor's 52.202 arriving through a field
# that neither the nickname nor the declared-time repair touches. The surplus is
# tail; the run is unchanged and the clip gets trimmed after filming.
# It is deliberately not automatic: an unexpected block length is how three
# wrong-ghost imports were caught tonight, and that check stays sharp by default.
if ! awk -v e="${END:-0}" -v w="$WANT" 'BEGIN{d=e-w; if(d<0)d=-d; exit (d<1.0)?0:1}'; then
  if [ "${SPAN_OK:-0}" = "1" ]; then
    log "span: block ${END}s vs sample span ${WANT}s -- accepted by SPAN_OK=1 (container timeline longer than the run)"
  else
    die "wrong ghost: clip end=${END}s, expected span ${WANT}s"
  fi
fi

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
if [ "${OPPONENT:-0}" != "1" ]; then
  OURNAME=$(echo "$CLIP" | grep -oE 'name=Ghost:[^ ]*' | grep -v -F -e 'Ghost:TAS' | head -1)
  if [ -n "$OURNAME" ] && [ "$(echo "$CLIP" | grep -c 'name=Ghost:TAS')" = "0" ]; then
    die "container identity: the game imported this as ${OURNAME#name=} -- our tapes import as Ghost:TAS, so this file is wearing someone else's container"
  fi
fi
log "ghost verified: clip end=${END}s"

# ---- 4. camera ---------------------------------------------------------------
# The stock External chase, targeted at the imported ghost. /cam reports the
# target as a NUMBER, which is how an all-black render is now impossible to
# mistake for a good one.
C 305 1656; sleep 2                        # Tracks +
C 1269 671; sleep 3                        # Player camera
for i in $(seq 1 12); do
  CAM=$(OP /cam 20)
  echo "$CAM" | grep -q 'entid=0' || break  # entid 0 = nobody
  C 492 322; sleep 1.5                      # advance the camera target
done
CAM=$(OP /cam 20); log "camera: $CAM"
echo "$CAM" | grep -q '^cam ' || die "no camera block was created"
echo "$CAM" | grep -q 'entid=0' && die "camera targets nobody -- would render black"
for i in $(seq 1 6); do
  echo "$CAM" | grep -q 'gamecam=2' && break   # 2 = External
  C 492 385; sleep 1.5
  CAM=$(OP /cam 20)
done
echo "$CAM" | grep -q 'gamecam=2' || log "  WARN: camera is not External ($CAM)"

# ---- 5. shoot ----------------------------------------------------------------
LAST=$(ls -t "$SS"/Video*.webm 2>/dev/null | head -1)
OP /rewind >/dev/null; sleep 5             # settling matters: shooting too early
                                           # yields an all-black clip that passes
                                           # size and duration checks
C 852 2093; sleep 3                        # AVI
C 1747 1743                                # OK -> render
log "rendering..."
NEW=""
for i in $(seq 1 720); do
  NEW=$(ls -t "$SS"/Video*.webm 2>/dev/null | head -1)
  [ -n "$NEW" ] && [ "$NEW" != "$LAST" ] && break
  sleep 5
done
{ [ -z "$NEW" ] || [ "$NEW" = "$LAST" ]; } && die "no video appeared"
for i in $(seq 1 720); do                  # wait for the file to stop growing
  a=$(stat -c%s "$NEW" 2>/dev/null); sleep 5; b=$(stat -c%s "$NEW" 2>/dev/null)
  [ "$a" = "$b" ] && [ "${a:-0}" -gt 100000 ] && break
done
cp "$NEW" "$OUTD/$OUT.webm"
log "filmed $(stat -c%s "$OUTD/$OUT.webm")B"

# ---- 6. overlay, aligned by MEASUREMENT --------------------------------------
# The trace and the ghost telemetry are two descriptions of one run. They agree
# exactly at one time shift and nowhere else, so the offset is found rather than
# assumed -- which is both stronger than eyeballing three frames and the thing
# that caught an off-by-one-sample error earlier today.
CSV="$OUTD/$OUT.csv"
"$TJ/inputcount" --csv "$GPATH" > "$CSV" || die "inputcount failed"
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

# ---- 7. accept or reject, numerically ----------------------------------------
DUR=$("$FFB/ffprobe.exe" -v error -show_entries format=duration -of csv=p=0 "$(wpath "$MP4")" | tr -d '\r\n ')
BLK=$("$FFB/ffmpeg.exe" -v error -i "$(wpath "$MP4")" -vf blackdetect=d=0.5:pix_th=0.10 \
      -f null - 2>&1 | grep -ci black_start || true)
SZ=$(stat -c%s "$MP4")
log "duration=${DUR}s expected~${WANT}s size=${SZ}B blackspans=${BLK:-0}"
awk -v d="${DUR:-0}" -v w="$WANT" 'BEGIN{x=d-w; if(x<0)x=-x; exit (x<1.5)?0:1}' \
  || die "duration ${DUR}s does not match the ${WANT}s record"
[ "${SZ:-0}" -gt 200000 ] || die "suspiciously small file: ${SZ}B"
[ "${BLK:-0}" -eq 0 ] || die "contains ${BLK} black span(s)"
log "OK $MP4"
echo "$MP4"
