#!/bin/sh
# render-box: restart the WhiteStick bridge (navi-node) when its SSE stream is dead while the process lives.
# The 2026-09-08 outage (3 h): pid alive, heartbeats 200, "SSE stream ended {... "timedOut":true}" every
# 30 s, not one "Command received" — the */10 pid keepalive cannot see it. Detector: >= N timed-out
# endings in the last W minutes of the log AND no command received in that window. Never restarts while
# a render driver holds the lock (a restart mid-render costs a lap): logs and waits. WD_DRY=1 only logs.
# cron: */2 * * * * $HOME/bin/navi-watchdog.sh        (installed 2026-09-09; source: tools/tinyctl/box/)
LOG=${NAVI_LOG:-$HOME/.navi/logs/navi-node.log}; OUT=${WD_OUT:-$HOME/navi-watchdog.log}
N=${WD_MIN_TIMEOUTS:-3}; W=${WD_WINDOW_MIN:-3}; SHOOTCTL=$HOME/trackmania-tas/tools/target/release/shootctl
PIDF=$HOME/.navi/navi-node.pid
since=$(date -u -d "-$W min" +%Y-%m-%dT%H:%M:%S)
win=$(tail -c 4000000 "$LOG" 2>/dev/null | awk -v s="$since" '{t=substr($2,2,19); if (t>=s) print}')
to=$(printf '%s\n' "$win" | grep -c 'SSE stream ended.*"timedOut":true')
cmd=$(printf '%s\n' "$win" | grep -c 'Command received')
[ "$to" -ge "$N" ] && [ "$cmd" -eq 0 ] || exit 0
now=$(date -Is)
pid=$(cat "$PIDF" 2>/dev/null)
if [ -z "$pid" ] || ! kill -0 "$pid" 2>/dev/null; then echo "$now SSE dead ($to timeouts/$W min) and no live pid -- the keepalive's job" >> "$OUT"; exit 0; fi
lock=$("$SHOOTCTL" lock status 2>/dev/null | head -1)
case "$lock" in *alive*) echo "$now SSE dead ($to timeouts, 0 commands in $W min) but a driver is live: $lock -- waiting" >> "$OUT"; exit 0;; esac
last=$(grep 'RESTARTED' "$OUT" 2>/dev/null | tail -1 | cut -d' ' -f1)
if [ -n "$last" ] && [ "$(( $(date +%s) - $(date -d "$last" +%s) ))" -lt 600 ]; then echo "$now SSE still dead; restarted <10 min ago -- waiting" >> "$OUT"; exit 0; fi
args=$(ps -o args= -p "$pid" 2>/dev/null)
if [ "${WD_DRY:-0}" = 1 ]; then echo "$now DRY RUN: would restart pid $pid ($to timeouts, 0 commands in $W min): $args" | sed 's/--token [^ ]*/--token .../' >> "$OUT"; exit 0; fi
echo "$now RESTARTED pid $pid ($to timeouts, 0 commands in $W min)" >> "$OUT"
kill "$pid" 2>/dev/null; sleep 3; kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null; sleep 1
setsid nohup sh -c "$args" >> "$HOME/navi-node-restart.log" 2>&1 < /dev/null &
