#!/bin/bash
# launch_tm.sh -- start Trackmania AND prove Openplanet came up with it.
#
# THE WHOLE POINT IS THE SECOND HALF. Every script in this pipeline talks to the
# game through the GhostShooter plugin on port 29800; a Trackmania with no
# Openplanet is a process that satisfies `tasklist` and can do nothing we need.
#
# WHAT THIS SCRIPT USED TO DO, AND WHY IT WAS WRONG
# -------------------------------------------------
# It launched `steam.exe -applaunch 2225070`, waited for Trackmania.exe to
# appear in the task list, slept 15 s to see whether it stayed, and printed
# "TM UP". That is a check on the wrong noun. On 2026-08-21 the game was
# restarted to clear a cached ghost-picker listing and came back **without
# Openplanet**: the process was up, the splash screen wanted an Enter, the menu
# loaded, and `tasklist` was perfectly happy -- while port 29800 was dead, the
# Openplanet log had not been written since the previous session, and
# `Get-Process Trackmania | Modules` showed `C:\WINDOWS\SYSTEM32\DINPUT8.dll`
# loaded instead of the game directory's Openplanet proxy of the same name.
#
# The defect had been latent all day. Every render before that point worked
# because the game was ALREADY running with Openplanet from an earlier manual
# start, so the launcher's claim was never tested. It reported success for a
# condition it never checked -- the same shape as a gate that passes a file it
# never read.
#
# THREE LAUNCH PATHS ARE TRIED, then the plugin is REQUIRED
# ---------------------------------------------------------
# Openplanet is installed here as a `dinput8.dll` proxy beside Trackmania.exe;
# there is no Openplanet.exe to launch through. The proxy is picked up from the
# executable's own directory, so the launch must come from there. Steam's
# -applaunch did not produce an injection on 2026-08-21 and a direct start did
# not either, so this tries the direct path first (closest to what the proxy
# needs), then Steam, and treats /ping -- not the process -- as the verdict.
#
# If every path fails, the script says so and exits non-zero. Openplanet may
# then need reinstalling from ~/Downloads/OpenplanetNext_<version>.exe, which is
# a GUI installer and cannot be driven from here.
set -u
GAMEDIR="/mnt/c/Program Files (x86)/Steam/steamapps/common/Trackmania"
STEAM="/mnt/c/Program Files (x86)/Steam"
OPLOG=/mnt/c/Users/vjeux/OpenplanetNext/Openplanet.log
KEYS='C:\Users\vjeux\tm-video\keys.ps1'

say(){ echo "[$(date +%H:%M:%S)] $*"; }
ping_plugin(){ curl.exe -sS -m 5 "http://127.0.0.1:29800/ping" 2>/dev/null | tr -d '\r'; }
tm_running(){ tasklist.exe 2>/dev/null | grep -qi trackmania.exe; }

# The game opens on a "press ENTER to start" splash and will sit there for ever.
# Openplanet initialises about a second after process start, well before this,
# so the Enter is for the game rather than for the plugin -- but without it the
# menu never appears and nothing else can be driven either.
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

# Openplanet rewrites this log from its first line at every start. Comparing the
# file's first timestamp before and after is a second, independent witness to
# whether it initialised -- one that does not depend on the HTTP server having
# finished starting.
oplog_stamp(){ head -1 "$OPLOG" 2>/dev/null | grep -oE '[0-9]{2}:[0-9]{2}:[0-9]{2}' | head -1; }

BEFORE=$(oplog_stamp)
say "Openplanet log currently opens at ${BEFORE:-<none>}"

launch_direct(){
  say "launch: Trackmania.exe directly from its own directory"
  ( cd "$GAMEDIR" && cmd.exe /d /c start "" "Trackmania.exe" >/dev/null 2>&1 )
}
launch_steam(){
  say "launch: steam -applaunch 2225070"
  "$STEAM/steam.exe" -applaunch 2225070 >/dev/null 2>&1 &
}

for attempt in direct steam direct; do
  kill_all
  case "$attempt" in direct) launch_direct ;; steam) launch_steam ;; esac

  # wait for the process, then clear the splash, then REQUIRE the plugin
  up=0
  for i in $(seq 1 40); do sleep 3; tm_running && { up=1; break; }; done
  [ "$up" = 1 ] || { say "  no Trackmania.exe appeared"; continue; }
  say "  process up; clearing the splash"
  sleep 25; press_enter

  for i in $(seq 1 36); do
    if [ "$(ping_plugin)" = "pong" ]; then
      say "PLUGIN UP -- /ping answered after ~$((25 + i*5))s (launch path: $attempt)"
      say "Openplanet log now opens at $(oplog_stamp)"
      exit 0
    fi
    sleep 5
  done

  AFTER=$(oplog_stamp)
  say "  no /ping after ~3 min. Openplanet log opens at ${AFTER:-<none>} (was ${BEFORE:-<none>})"
  [ "$AFTER" = "$BEFORE" ] && say "  -- the log did not rotate, so Openplanet never initialised in this process"
done

say "FAILED: Trackmania is running but Openplanet is NOT loaded."
say "  Everything in this pipeline drives the game through the plugin on 29800,"
say "  so nothing can be filmed in this state. Do not report this as a launch."
say "  Check which dinput8 the process actually loaded:"
say "    powershell.exe -NoProfile -Command \\"
say "      \"Get-Process Trackmania | %{\\\$_.Modules} | ?{\\\$_.ModuleName -like '*dinput8*'} | select FileName\""
say "  The game directory's proxy must win over C:\\WINDOWS\\SYSTEM32\\DINPUT8.dll."
say "  If it does not, reinstall Openplanet from ~/Downloads/OpenplanetNext_<ver>.exe (GUI)."
exit 1
