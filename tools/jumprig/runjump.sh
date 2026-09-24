#!/bin/sh
# Drive the jump button end to end, under the game lock, and RETRY the whole
# sequence if the box drops the game under us.
#
# WHY RETRY: this box's game instance is intermittently short-lived. It exits
# cleanly after a few minutes — no crash dump, no Windows error record, and
# Ubisoft Connect only observes it afterwards — and a PlayMap sometimes
# reports success and loads nothing. Both reproduce with this plugin REMOVED
# (verified 2026-09-23), so they are the box, not the jump. shootctl hit the
# same wall in August and answered it the same way: relaunch and try again.
#
# Every wait is jumprig's `waitfor`, which polls a real condition and names
# the one it gave up on. Nothing sleeps for a guessed duration.
set -u
export TM_SESSION=979f4ff1-2c12-4e17-9d0b-77778502312f
export TM_SESSION_TITLE="jump button"
J=/home/vjeux/bin/jumprig
TASKLIST=/mnt/c/Windows/System32/tasklist.exe
MAP='C:/Users/vjeux/Documents/Trackmania/Maps/Probe/old630.Map.Gbx'

attempt=1
while [ $attempt -le 3 ]; do
  echo "######## attempt $attempt ########"

  echo "=== 1. game up, Openplanet started, hook installed ==="
  if ! $J launch 300; then
    echo "  launch failed"; attempt=$((attempt+1)); continue
  fi

  echo "=== 2. load a map ==="
  $J cmd playmap "$MAP" || { attempt=$((attempt+1)); continue; }

  echo "=== 3. playground, car, physics ==="
  if ! $J waitfor in-map 120 >/dev/null; then
    echo "  no playground (PlayMap reported success and loaded nothing)"
    /mnt/c/Windows/System32/taskkill.exe /F /IM Trackmania.exe >/dev/null 2>&1
    attempt=$((attempt+1)); continue
  fi
  echo "  in a map"
  $J waitfor car 60 >/dev/null     || { echo "  no car pointer";  attempt=$((attempt+1)); continue; }
  echo "  car pointer captured"
  $J waitfor ticking 30 >/dev/null || { echo "  hook not firing"; attempt=$((attempt+1)); continue; }
  echo "  physics hook is firing"

  echo "=== 4. the jump ==="
  if $J jumptest; then
    echo "######## SUCCESS on attempt $attempt ########"
    exit 0
  fi
  attempt=$((attempt+1))
done

echo "######## all 3 attempts failed ########"
exit 1
