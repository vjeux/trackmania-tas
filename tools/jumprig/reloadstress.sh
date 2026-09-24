#!/bin/sh
# Reload-stress the native hook.
#
# THE BUG THIS EXISTS FOR: RemoveHook used to free the trampoline island while
# a physics thread could still be executing inside it. Restoring the entry
# bytes stops new calls entering; it says nothing about a thread already in
# there. Every Openplanet hot-reload (an edit to Main.as is enough) was a
# coin-flip on a use-after-free in the game's physics thread — the process
# vanished with no dump and no log line, ~10 s after a map loaded.
#
# So: get physics running, then reload the plugin repeatedly, and require the
# game to survive and still jump.
set -u
export TM_SESSION=979f4ff1-2c12-4e17-9d0b-77778502312f
export TM_SESSION_TITLE="jump button"
J=/home/vjeux/bin/jumprig
P=/mnt/c/Users/vjeux/OpenplanetNext/Plugins/JumpButton/Main.as
TASKLIST=/mnt/c/Windows/System32/tasklist.exe
MAP='C:/Users/vjeux/Documents/Trackmania/Maps/Probe/old630.Map.Gbx'
alive() { $TASKLIST /FI "IMAGENAME eq Trackmania.exe" /NH 2>/dev/null | grep -qi trackmania.exe; }

echo "=== get to a driving car ==="
$J launch 300 >/dev/null || { echo "FAIL: launch"; exit 1; }
$J cmd playmap "$MAP" >/dev/null 2>&1
$J waitfor in-map 180 >/dev/null || { echo "FAIL: no playground"; exit 1; }
$J waitfor car 60 >/dev/null    || { echo "FAIL: no car"; exit 1; }
$J waitfor ticking 30 >/dev/null || { echo "FAIL: hook not firing"; exit 1; }
echo "  driving, physics hook firing"

echo "=== reload the plugin 6x while physics runs ==="
i=1
while [ $i -le 6 ]; do
  touch "$P"                 # Openplanet developer mode reloads on mtime change
  sleep 4
  if ! alive; then echo "  FAIL: the game died on reload #$i"; exit 1; fi
  # The hook must come back, not just the plugin.
  $J waitfor hooked 30 >/dev/null 2>&1 || { echo "  FAIL: hook did not reinstall after reload #$i"; exit 1; }
  echo "  reload #$i: game alive, hook reinstalled"
  i=$((i+1))
done

echo "=== the jump still works after all those reloads ==="
$J waitfor ticking 60 >/dev/null 2>&1
$J waitfor grounded 60 >/dev/null 2>&1
$J jumptest 2>&1 | grep -E "settled|apex|landed|RESULT|FAIL"
