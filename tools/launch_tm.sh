#!/bin/bash
# launch_tm.sh -- start Trackmania so that Openplanet ACTUALLY INJECTS, and
# prove it did.
#
# THE ONE THING THIS SCRIPT EXISTS FOR
# ------------------------------------
# Every script in this pipeline drives the game through the GhostShooter plugin
# on port 29800. A Trackmania with no Openplanet is a process that satisfies
# `tasklist` and can do nothing we need. So the verdict here is /ping, never the
# process.
#
# WHY THE LAUNCH GOES THROUGH EXPLORER  (2026-08-21, measured)
# -------------------------------------------------------------
# Openplanet is a `dinput8.dll` proxy beside Trackmania.exe: the game statically
# imports DINPUT8.dll, Windows resolves that from the application directory
# first, and the proxy loads Openplanet. That is the whole mechanism.
#
# It breaks under one process-creation mitigation:
#
#     PROCESS_CREATION_MITIGATION_POLICY_IMAGE_LOAD_PREFER_SYSTEM32
#
# With PreferSystem32 ON, the loader prefers C:\WINDOWS\SYSTEM32 over the
# application directory for every DLL that exists in both. The game then loads
# System32's real dinput8.dll, the proxy is never touched, Openplanet never
# initialises, and 29800 is dead -- while the game itself runs perfectly.
#
# AND THE MITIGATION IS INHERITED FROM WHOEVER LAUNCHES THE GAME. Measured on
# this box on 2026-08-21, with the game up and every file in place:
#
#     explorer.exe          PreferSystem32=OFF
#     powershell / cmd      PreferSystem32=ON     <- our bridge runs here
#     steam.exe             PreferSystem32=ON
#     upc.exe               PreferSystem32=ON
#     UbisoftGameLauncher   PreferSystem32=ON
#     Trackmania.exe        PreferSystem32=ON     -> System32\DINPUT8.dll
#
# So a game started BY HAND from the desktop got the proxy and worked all
# morning, and every launch from this script inherited ON from the bridge's own
# shell and silently did not. Nothing was broken: not the proxy (present, x64,
# unblocked, and it loads fine via LoadLibraryW), not Openplanet (installed),
# not Defender (no detections), not the game (Feb 22 build, older than the
# Openplanet build). Only the launcher's parentage.
#
# `explorer.exe <path>` hands the request to the already-running Explorer, which
# creates the process as ITS child and therefore with ITS policy -- OFF. That is
# the fix, it needs no admin and no reboot, and it is one line.
#
# The proof that closes it, from the same box minutes apart:
#     launched from cmd:      Trackmania loads C:\WINDOWS\SYSTEM32\DINPUT8.dll
#     launched via explorer:  Trackmania loads <gamedir>\DINPUT8.dll  + /ping -> pong
#
# THE CONTROL THAT MADE IT FINDABLE, worth keeping in mind for the next one of
# these: a byte-identical copy of System32's own dinput8.dll placed in the game
# directory was ALSO ignored, and so was a copy of version.dll. That ruled out
# "the proxy file is bad" and pointed at the loader, not the payload -- ten
# minutes after an hour of inspecting the DLL itself.
set -u
GAMEDIR="/mnt/c/Program Files (x86)/Steam/steamapps/common/Trackmania"
GAMEEXE_WIN='C:\Program Files (x86)\Steam\steamapps\common\Trackmania\Trackmania.exe'
STEAM="/mnt/c/Program Files (x86)/Steam"
OPLOG=/mnt/c/Users/vjeux/OpenplanetNext/Openplanet.log
KEYS='C:\Users\vjeux\tm-video\keys.ps1'

say(){ echo "[$(date +%H:%M:%S)] $*"; }
ping_plugin(){ curl.exe -sS -m 5 "http://127.0.0.1:29800/ping" 2>/dev/null | tr -d '\r'; }
tm_running(){ tasklist.exe 2>/dev/null | grep -qi trackmania.exe; }

press_enter(){
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$KEYS" activate Trackmania >/dev/null 2>&1
  sleep 2
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$KEYS" tap 1C >/dev/null 2>&1
}

kill_all(){
  taskkill.exe /F /IM Trackmania.exe >/dev/null 2>&1
  taskkill.exe /F /IM UbisoftGameLauncher.exe >/dev/null 2>&1
  sleep 6
}

oplog_stamp(){ head -1 "$OPLOG" 2>/dev/null | grep -oE '[0-9]{2}:[0-9]{2}:[0-9]{2}' | head -1; }

# What the running game's loader actually did. This is the diagnosis, printed
# every time, so a failure never needs an hour of archaeology again.
report_policy(){
  powershell.exe -NoProfile -Command "
    \$p = Get-Process Trackmania -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not \$p) { 'no Trackmania process'; exit }
    \$par = (Get-CimInstance Win32_Process -Filter (\"ProcessId=\" + \$p.Id)).ParentProcessId
    \$pn  = (Get-Process -Id \$par -ErrorAction SilentlyContinue).ProcessName
    \$ps  = (Get-ProcessMitigation -Id \$p.Id -ErrorAction SilentlyContinue).ImageLoad.PreferSystem32
    'parent=' + \$pn + '  PreferSystem32=' + \$ps
    \$p.Modules | Where-Object { \$_.ModuleName -like '*dinput*' } | ForEach-Object { '  loaded: ' + \$_.FileName }
  " 2>/dev/null | tr -d '\r' | sed 's/^/    /'
}

# THE LAUNCH. Explorer is first because it is the one that works; the others
# remain as fallbacks and are expected to fail while PreferSystem32 is on.
launch_explorer(){
  say "launch: via explorer.exe -- inherits Explorer's PreferSystem32=OFF, which is what lets the dinput8 proxy load"
  explorer.exe "$GAMEEXE_WIN" >/dev/null 2>&1
}
launch_direct(){
  say "launch: Trackmania.exe directly (inherits THIS shell's mitigation policy -- expected to miss the proxy)"
  ( cd "$GAMEDIR" && cmd.exe /d /c start "" "Trackmania.exe" >/dev/null 2>&1 )
}
launch_steam(){
  say "launch: steam -applaunch 2225070"
  "$STEAM/steam.exe" -applaunch 2225070 >/dev/null 2>&1 &
}

BEFORE=$(oplog_stamp)
say "Openplanet log currently opens at ${BEFORE:-<none>}"

for attempt in explorer explorer direct steam; do
  kill_all
  case "$attempt" in
    explorer) launch_explorer ;;
    direct)   launch_direct ;;
    steam)    launch_steam ;;
  esac

  up=0
  for i in $(seq 1 40); do sleep 3; tm_running && { up=1; break; }; done
  [ "$up" = 1 ] || { say "  no Trackmania.exe appeared"; continue; }
  say "  process up; clearing the splash"
  sleep 25; press_enter

  for i in $(seq 1 24); do
    if [ "$(ping_plugin)" = "pong" ]; then
      say "PLUGIN UP -- /ping answered after ~$((25 + i*5))s (launch path: $attempt)"
      say "Openplanet log now opens at $(oplog_stamp)"
      report_policy
      exit 0
    fi
    sleep 5
  done

  say "  no /ping after ~2 min on the '$attempt' path. What the loader did:"
  report_policy
  AFTER=$(oplog_stamp)
  [ "$AFTER" = "$BEFORE" ] && say "  -- the Openplanet log did not rotate, so it never initialised in this process"
done

say "FAILED: Trackmania runs but Openplanet is NOT loaded. Nothing can be filmed."
say "  Read the policy line above FIRST:"
say "    * PreferSystem32=ON  -> the launch inherited the mitigation. Explorer"
say "      should have avoided it; if Explorer itself now reports ON, something"
say "      has turned the mitigation on machine-wide and the per-image override"
say "      is the fix (needs an ELEVATED shell, no reboot):"
say "        Set-ProcessMitigation -Name Trackmania.exe -Disable PreferSystem32"
say "    * PreferSystem32=OFF and still no proxy -> now it IS the payload."
say "      Reinstall Openplanet from ~/Downloads/OpenplanetNext_<ver>.exe (GUI,"
say "      needs UAC), then run this again."
exit 1
