#!/bin/sh
# shoot-junction.sh — move the staging folder OUT of the OneDrive tree and leave a
# directory JUNCTION at Maps\_shoot pointing to C:\tm\_shoot, so a staged map is
# a plain NTFS write (OneDrive does not follow reparse points) while the game
# still sees it under the Trackmania user tree, the only place it loads from
# (RENDER-BOX.md, 2026-08-26). Measured before: a 6 MB stage took ~10 min and a
# 35 MB one ~14 min on a 100 %-full C: with OneDrive syncing every write.
#
# Runs detached on the box under the render lock (other threads' shoots finish
# first and none can start a load underneath the switch); writes
# ~/shoot/shoot-junction.{log,done}. Idempotent: an existing junction is kept.
LOG=/home/vjeux/shoot/shoot-junction.log; DONE=/home/vjeux/shoot/shoot-junction.done
SC=/home/vjeux/trackmania-tas/tools/target/release/shootctl
OD="/mnt/c/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot"
ODW='C:\Users\vjeux\OneDrive\Documents\Trackmania\Maps\_shoot'
NEW=/mnt/c/tm/_shoot
NEWW='C:\tm\_shoot'
rm -f "$DONE"
fail() { echo "FAILED $1" | tee "$DONE"; $SC lock release --owner shoot-junction >/dev/null 2>&1; exit 1; }
{
  echo "== $(date -u +%FT%TZ) waiting for the render lock (other threads' shoots finish first) …"
  $SC lock acquire --owner shoot-junction --wait 5400 --max-age 3600 || fail "lock not acquired in 90 min"
  echo "== $(date -u +%FT%TZ) holding the render lock"
  echo "--- free before: $(df -m /mnt/c | awk 'NR==2{print $4}') MB"

  if timeout 20 cmd.exe /c "fsutil reparsepoint query \"$ODW\"" >/dev/null 2>&1; then
    echo "Maps\\_shoot is already a reparse point:"; timeout 20 cmd.exe /c "dir /AL \"C:\\Users\\vjeux\\OneDrive\\Documents\\Trackmania\\Maps\"" | tr -d '\r' | grep -i _shoot
  else
    mkdir -p "$NEW" || fail "mkdir $NEW"
    n=$(ls -A "$OD" | wc -l)
    echo "--- moving $n entries ($(du -sm "$OD" | cut -f1) MB) to $NEW"
    cp -a "$OD"/. "$NEW"/ || fail "copy"
    # every file byte-identical on the other side before anything is removed
    (cd "$OD" && find . -type f -exec md5sum {} + | sort -k2) > /tmp/sj-src.md5
    (cd "$NEW" && find . -type f -exec md5sum {} + | sort -k2) > /tmp/sj-dst.md5
    if ! cmp -s /tmp/sj-src.md5 /tmp/sj-dst.md5; then diff /tmp/sj-src.md5 /tmp/sj-dst.md5 | head; fail "copy differs"; fi
    echo "--- $(wc -l < /tmp/sj-src.md5) files verified by md5"
    rm -rf "$OD" || fail "rm source"
    [ -e "$OD" ] && fail "source still there"
    timeout 30 cmd.exe /c "mklink /J \"$ODW\" \"$NEWW\"" | tr -d '\r' || fail "mklink"
  fi
  echo "--- the alias:"; ls -la "$OD" | head -4
  timeout 20 cmd.exe /c "fsutil reparsepoint query \"$ODW\"" | tr -d '\r' | head -3

  # write speed through the alias (the thing this is for)
  t0=$(date +%s.%N); head -c 35000000 /dev/urandom > "$OD/.iotest.tmp" && sync; t1=$(date +%s.%N)
  echo "--- 35 MB written through the junction in $(echo "$t1 - $t0" | bc) s"
  [ "$(stat -c %s "$NEW/.iotest.tmp")" = 35000000 ] && echo "    (and it landed in $NEW)" || fail "write did not land in $NEW"
  rm -f "$OD/.iotest.tmp"

  # THE GAME LOADS THROUGH THE JUNCTION: one editor load of a map staged there
  m=$(ls "$NEW"/*Orig.Map.Gbx 2>/dev/null | head -1)
  [ -n "$m" ] || m=$(ls "$NEW"/*.Map.Gbx | head -1)
  [ -n "$m" ] || m=/home/vjeux/shoot/_stage/Tiny01.Map.Gbx
  cp -f "$m" "$NEW/junction-check.Map.Gbx"
  $SC launch 240 >/dev/null 2>&1 || echo "(launch: the game may already be up)"
  echo "--- probe: EditMap C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/junction-check.Map.Gbx ($(basename "$m"), $(stat -c %s "$m") B)"
  $SC probe --map "C:/Users/vjeux/OneDrive/Documents/Trackmania/Maps/_shoot/junction-check.Map.Gbx" --how edit --timeout 240 > /tmp/sj-probe.txt 2>&1; rc=$?
  tail -12 /tmp/sj-probe.txt
  grep -q "OPENED — and it is the map we asked for" /tmp/sj-probe.txt && verdict=OPENED || verdict=NOT-OPENED
  rm -f "$NEW/junction-check.Map.Gbx"
  $SC quit >/dev/null 2>&1
  $SC lock release --owner shoot-junction
  echo "--- free after: $(df -m /mnt/c | awk 'NR==2{print $4}') MB"
  echo "== $(date -u +%FT%TZ) done (probe rc=$rc)"
  echo "OK probe=$verdict rc=$rc" > "$DONE"
} >> "$LOG" 2>&1
