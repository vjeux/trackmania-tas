#!/bin/bash
# gamebot-drive.sh — drive the game by hand, one action per call.
#
#   bash tools/gamebot-drive.sh click <x> <y>     # 1280x720 frame, scaled x3
#   bash tools/gamebot-drive.sh key <scancode>...
#   bash tools/gamebot-drive.sh shot              # base64 jpeg on stdout
#   bash tools/gamebot-drive.sh state [/route]    # the GhostShooter plugin
#
# Catches: *a click that was never delivered looking exactly like a click that
# was ignored.* powershell.exe is a WINDOWS binary, so every path handed to it —
# `-File` included — must be a Windows path; the /mnt/c form fails with "the
# argument ... does not exist", and with stderr discarded ffmpeg then quietly
# re-encoded the PREVIOUS screenshot. The screen appeared frozen and every click
# looked ignored. `shot` therefore prints the PNG's timestamp next to the wall
# clock on stderr: freshness is stated, never assumed.
#
# RUNS ON THE RENDER BOX. GameBot.exe, crop.ps1, keys.ps1 and the plugin's HTTP
# port are all WhiteStick-local; the coordinate scaling and the path discipline
# are the parts worth keeping.
# d.sh -- manual driver for the game, one action per call.
#
# Coordinates are in the 1280x720 SCREENSHOT frame; the screen is 3840x2160, so
# they are scaled by 3 on the way in.
#
# NOTE: powershell.exe is a WINDOWS binary. Every path handed to it -- including
# -File -- must be a Windows path. Passing the /mnt/c form makes it fail with
# "the argument ... does not exist", which d.sh was hiding behind 2>/dev/null:
# ffmpeg then quietly re-encoded the PREVIOUS screenshot, so the screen appeared
# frozen and every click looked like it had been ignored.
GB="/mnt/c/Users/vjeux/game-bot-cli/GameBot/bin/Release/net6.0-windows/GameBot.exe"
WTV='C:\Users\vjeux\tm-video'
TV=/mnt/c/Users/vjeux/tm-video
FFB=/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin

case "$1" in
  click)
    "$GB" click $(( $2 * 3 )) $(( $3 * 3 )) >/dev/null 2>&1
    echo "clicked $(( $2*3 )),$(( $3*3 ))" ;;
  key)
    shift
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$WTV\\keys.ps1" "$@" >/dev/null 2>&1
    echo "key $*" ;;
  shot)
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$WTV\\crop.ps1" \
      -x 0 -y 0 -w 3840 -h 2160 -out "$WTV\\now.png" -scale 1 >/dev/null 2>&1
    [ "$TV/now.png" -nt "$TV/.shotstamp" ] 2>/dev/null || true
    touch "$TV/.shotstamp"
    "$FFB/ffmpeg.exe" -y -loglevel error -i "C:/Users/vjeux/tm-video/now.png" \
      -vf scale=1280:-1 -q:v 5 "C:/Users/vjeux/tm-video/now.jpg"
    # freshness is printed, never assumed
    echo "PNG $(date -r "$TV/now.png" +%H:%M:%S) NOW $(date +%H:%M:%S)" >&2
    base64 -w0 "$TV/now.jpg" ;;
  state)
    curl.exe -sS -m 15 "http://127.0.0.1:29800${2:-/state}" 2>/dev/null | tr -d '\r' ;;
esac
