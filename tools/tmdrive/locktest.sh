#!/bin/sh
# Adversarial test of the render-box lock.
#
# Each case is a way the lock has actually failed, or could: an anonymous
# holder nobody can reach, two sessions at once, a holder whose game died, a
# holder that stopped renewing, a released lock whose key still worked, and a
# driver going around the lock entirely.
#
# NON-DESTRUCTIVE BY CONSTRUCTION: no case kills the game. An earlier version
# used `tmdrive kill` to prove a reclaim and destroyed the very instance the
# later plugin cases needed -- four "failures" that were all this bug.
#
# Run on the box. Prints PASS/FAIL per case and a count at the end.

TM=/home/vjeux/bin/tmdrive
LOCK=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/TmDriveLock
CURL=/mnt/c/Windows/System32/curl.exe
TASKLIST=/mnt/c/Windows/System32/tasklist.exe
A=11111111-aaaa-4aaa-8aaa-111111111111
B=22222222-bbbb-4bbb-8bbb-222222222222
pass=0; fail=0

ok()  { echo "  PASS  $1"; pass=$((pass+1)); }
bad() { echo "  FAIL  $1"; fail=$((fail+1)); }
free_lock() { rm -rf "$LOCK"; }
game_pid() {
  # tasklist prints "INFO: No tasks are running..." when there is no match, so
  # filter for the row before taking a field -- reading that message as a pid
  # made the harness skip its own precondition.
  $TASKLIST /FI "IMAGENAME eq Trackmania.exe" /NH /FO CSV 2>/dev/null \
    | grep -i 'Trackmania.exe' | head -1 | cut -d, -f2 | tr -d '"' | tr -d '\r'
}

echo "=== 0. precondition: the game is up (the plugin cases need it) ==="
GP=$(game_pid)
if [ -n "$GP" ]; then
  echo "  game pid $GP"
else
  echo "  launching..."
  free_lock
  TM_SESSION=$A TM_SESSION_TITLE="locktest" $TM launch --purpose "locktest precondition" --timeout 300 >/dev/null 2>&1
  i=0; while [ $i -lt 60 ]; do $CURL -s --max-time 3 http://127.0.0.1:29800/ping 2>/dev/null | grep -q pong && break; sleep 3; i=$((i+1)); done
  GP=$(game_pid)
  [ -n "$GP" ] && echo "  game pid $GP" || echo "  WARNING: no game; plugin cases will fail"
fi
free_lock

echo "=== 1. an anonymous driver is refused (an unreachable holder is the bug) ==="
out=$(env -u TM_SESSION -u AGENTCLOUD_SESSION_ID $TM plugin ctx --purpose anon 2>&1)
case "$out" in *"no session id"*) ok "refused without a session id";; *) bad "anonymous driver allowed: $out";; esac

echo "=== 2. a second live session is refused, and told who holds it ==="
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "holding for the test" -- sleep 6 &
runner=$!
sleep 2
out=$(TM_SESSION=$B TM_SESSION_TITLE="B" $TM plugin ctx --purpose "B barging in" 2>&1); rc=$?
case "$out" in *"BUSY"*) ok "second session refused";; *) bad "second session was NOT refused: $out";; esac
case "$out" in *"$A"*) ok "refusal names the holding session";; *) bad "refusal does not name the holder";; esac
[ "$rc" = "75" ] && ok "exit 75 (EX_TEMPFAIL) so callers can retry" || bad "exit was $rc, expected 75"
wait $runner 2>/dev/null

echo "=== 3. the lock is released when the holder exits ==="
out=$($TM status 2>&1)
case "$out" in FREE*) ok "released automatically on exit";; *) bad "still held after the holder exited: $out";; esac

echo "=== 4. a holder whose GAME died is reclaimable ==="
free_lock
mkdir -p "$LOCK"
printf '%s' "$A" > "$LOCK/session"; printf '%s' "dead-game" > "$LOCK/purpose"
printf '%s' "A"  > "$LOCK/title";   printf '%s' "999999"    > "$LOCK/game_pid"
now=$(date +%s); printf '%s' "$now" > "$LOCK/acquired_at"; printf '%s' "$now" > "$LOCK/renewed_at"
out=$(TM_SESSION=$B $TM status 2>&1)
case "$out" in *"DEAD(game gone)"*) ok "a dead game marks the lock reclaimable";; *) bad "not flagged: $out";; esac
out=$(TM_SESSION=$B TM_SESSION_TITLE=B $TM plugin ctx --purpose "taking a dead lock" 2>&1)
case "$out" in *"taking a DEAD lock"*) ok "reclaimed, loudly";; *) bad "not reclaimed: $out";; esac

echo "=== 5. a holder that stopped renewing is reclaimable ==="
free_lock
mkdir -p "$LOCK"
printf '%s' "$A" > "$LOCK/session"; printf '%s' "stale" > "$LOCK/purpose"
printf '%s' "A"  > "$LOCK/title";   printf '%s' "$GP"   > "$LOCK/game_pid"
old=$(( $(date +%s) - 3600 ))
printf '%s' "$old" > "$LOCK/acquired_at"; printf '%s' "$old" > "$LOCK/renewed_at"
out=$(TM_SESSION=$B $TM status 2>&1)
case "$out" in *"lease expired"*) ok "an unrenewed lease marks the lock reclaimable";; *) bad "not flagged: $out";; esac
out=$(TM_SESSION=$B TM_SESSION_TITLE=B $TM plugin ctx --purpose "taking an expired lock" 2>&1)
case "$out" in *"EXPIRED lock"*) ok "expired lock reclaimed, loudly";; *) bad "not reclaimed: $out";; esac

echo "=== 6. a RELEASED lock leaves no key behind ==="
free_lock
if [ -f "$LOCK/token" ]; then bad "a token survived release"; else ok "no token after release"; fi
out=$($CURL -s --max-time 8 "http://127.0.0.1:29800/playmap?mode=" 2>/dev/null)
case "$out" in *token-refused*) ok "the game refuses mutating commands with no lock held";; *) bad "game accepted a command with no lock: [$out]";; esac

echo "=== 7. going around the lock is refused BY THE GAME ==="
out=$($CURL -s --max-time 8 "http://127.0.0.1:29800/playmap?token=made-up-nonsense" 2>/dev/null)
case "$out" in *token-refused*) ok "a forged token is refused";; *) bad "a forged token was accepted: [$out]";; esac

echo "=== 8. reads stay open (or everyone holds the lock forever) ==="
out=$($CURL -s --max-time 8 "http://127.0.0.1:29800/ctx" 2>/dev/null)
case "$out" in *'"ctx"'*) ok "read-only commands need no lock";; *) bad "reads are gated: [$out]";; esac

echo "=== 9. the real holder CAN drive the game ==="
free_lock
out=$(TM_SESSION=$A TM_SESSION_TITLE="A" $TM plugin ctx --purpose "holder drives" 2>&1)
case "$out" in *'"ctx"'*) ok "the holder's command is accepted";; *) bad "the holder was refused: $out";; esac

echo "=== 10. the lease is renewed automatically during long work ==="
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "long job" -- sleep 50 &
runner=$!
sleep 46
r=$(cat "$LOCK/renewed_at" 2>/dev/null || echo 0)
age=$(( $(date +%s) - r ))
if [ "$age" -lt 45 ]; then ok "lease renewed in the background (last renewal ${age}s ago)"; else bad "lease went stale during work (${age}s)"; fi
wait $runner 2>/dev/null

echo "=== 11. the game survived the whole test ==="
[ -n "$(game_pid)" ] && ok "game still running" || bad "the test destroyed the game instance"
free_lock

echo
echo "================  $pass passed, $fail failed  ================"
[ "$fail" = "0" ] || exit 1
