#!/bin/bash
# splitscreen.sh — two runs side by side, for maps where a chase camera cannot
# hold both cars in one shot. RUNS ON THE RENDER BOX (WhiteStick): the Mac's
# ffmpeg is built without libfreetype, so drawtext is unavailable there.
#
#   bash splitscreen.sh <left.mp4> <right.mp4> <left-label> <right-label> <out.mp4>
#
# WHY THIS EXISTS
# A two-car MediaTracker clip only works while both cars stay near the camera.
# On 276877 the human record is 6.061 s slower and 61.5 m away, and on 228607 it
# is 4.605 s slower and 356.68 m away: the opponent is behind the camera for the
# entire run, so the "comparison" is one car and a caption that lies. Rendering
# each run on its own and putting them side by side shows what the other driver
# actually does — which is the thing that was asked for.
#
# Both inputs start at the race start (t=0), so they align with no offset. The
# shorter run is held on its final frame until the longer one finishes: the fast
# car parks at the flag while the slow car is still out on the track, so the gap
# reads as TIME. A black half-screen would read as a broken video instead.
set -euo pipefail

FF=${FF:-/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe}
FP=${FP:-/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffprobe.exe}
FONT=${FONT:-'C\:/Windows/Fonts/arialbd.ttf'}

L="${1:?usage: splitscreen.sh <left.mp4> <right.mp4> <left-label> <right-label> <out.mp4>}"
R="${2:?need right.mp4}"; LL="${3:?need left label}"; RL="${4:?need right label}"; OUT="${5:?need out.mp4}"

dur(){ "$FP" -v error -show_entries format=duration -of csv=p=0 "$1" | tr -d '\r'; }
DL="$(dur "$L")"; DR="$(dur "$R")"
LONG="$(awk -v a="$DL" -v b="$DR" 'BEGIN{print (a>b)?a:b}')"
echo "split: left ${DL}s [${LL}] | right ${DR}s [${RL}] | output ${LONG}s"

"$FF" -v error -y -i "$L" -i "$R" -filter_complex "\
[0:v]scale=960:-2,tpad=stop_mode=clone:stop_duration=60,trim=duration=${LONG},setpts=PTS-STARTPTS,\
drawtext=fontfile='${FONT}':text='${LL}':x=18:y=14:fontsize=28:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=9[l];\
[1:v]scale=960:-2,tpad=stop_mode=clone:stop_duration=60,trim=duration=${LONG},setpts=PTS-STARTPTS,\
drawtext=fontfile='${FONT}':text='${RL}':x=18:y=14:fontsize=28:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=9[r];\
[l][r]hstack=inputs=2[v]" -map "[v]" -c:v libx264 -crf 19 -preset medium -pix_fmt yuv420p -an "$OUT"

echo "split: $(dur "$OUT")s $(stat -c%s "$OUT") bytes -> $OUT"
