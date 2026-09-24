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

SHOOTCTL=/home/vjeux/bin/shootctl
TM=/home/vjeux/bin/tmdrive
LOCK=/mnt/c/Users/vjeux/OpenplanetNext/PluginStorage/TmDriveLock
CURL=/mnt/c/Windows/System32/curl.exe
TASKLIST=/mnt/c/Windows/System32/tasklist.exe
A=11111111-aaaa-4aaa-8aaa-111111111111
B=22222222-bbbb-4bbb-8bbb-222222222222
pass=0; fail=0

ok()  { echo "  PASS  $1"; pass=$((pass+1)); }
bad() { echo "  FAIL  $1"; fail=$((fail+1)); }

# THE SUITE MUST NEVER DELETE SOMEONE ELSE'S LIVE LOCK.
#
# `free_lock` used to be a bare `rm -rf "$LOCK"`, run between cases with no
# check at all. On 2026-09-24 that deleted the u10s session's record three
# separate times while its publish was mid-flight -- once after my own
# `tmdrive wait` had TIMED OUT and the script pressed on regardless. Each
# time, their run kept executing with nothing behind it, their renewer saw
# the directory gone and stopped, and it surfaced to them as "the lease
# expired" and "no session holds the game lock" -- a phantom lock bug that
# cost a morning of debugging on both sides. The suite was the one thing on
# the box exempt from the lock, and it was the thing breaking it.
#
# So: the suite only ever removes records belonging to ITS OWN fake sessions
# (A and B) or to the session running it. A live record from anyone else
# aborts the whole suite, loudly, and `free_lock` is never a way past that.
OWN_SESSIONS="$A $B ${TM_SESSION:-}"
foreign_hold() {
  [ -d "$LOCK" ] || return 1
  o=$(cat "$LOCK/session" 2>/dev/null)
  for s in $OWN_SESSIONS; do [ "$o" = "$s" ] && return 1; done
  return 0
}
free_lock() {
  if foreign_hold; then
    echo "  ABORT: the box is held by $(cat "$LOCK/session" 2>/dev/null | cut -c1-8) ('$(cat "$LOCK/purpose" 2>/dev/null)') -- not ours to remove."
    echo "         The suite refuses to run over a live foreign hold. Wait for the box."
    exit 75
  fi
  rm -rf "$LOCK"
}

# Before anything else: the box must be free, or ours.
if foreign_hold; then
  echo "the box is held by another session: $($TM status 2>&1 | head -1)"
  echo "the suite will not run over a live hold. Waiting up to 30 min..."
  if ! $TM wait --timeout 1800 >/dev/null 2>&1; then
    echo "ABORT: still held after 30 min; not running."
    exit 75
  fi
fi
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

echo "=== 4. a dead game alone does NOT free the box ==="
# This asserts the MK64 fix, and is the INVERSE of what it used to assert.
# "Game gone => reclaimable" looked obviously right and was wrong: a holder
# restarting the game as part of its own work has no game process for a
# while, and the box got handed to someone else mid-run. A holder that is
# still renewing owns the box whatever the game is doing; a genuinely dead
# holder stops renewing and case 5 catches it.
free_lock
mkdir -p "$LOCK"
printf '%s' "$A" > "$LOCK/session"; printf '%s' "game restarting" > "$LOCK/purpose"
printf '%s' "A"  > "$LOCK/title";   printf '%s' "999999"          > "$LOCK/game_pid"
now=$(date +%s); printf '%s' "$now" > "$LOCK/acquired_at"; printf '%s' "$now" > "$LOCK/renewed_at"
out=$(TM_SESSION=$B $TM status 2>&1)
case "$out" in
  *HELD*) ok "a fresh lease keeps the box even with no game process";;
  *) bad "the box was declared free on a dead game alone: $out";;
esac
out=$(TM_SESSION=$B TM_SESSION_TITLE=B $TM plugin ctx --purpose "B barging in" 2>&1); rc=$?
[ "$rc" = "75" ] && ok "another session is refused while the holder restarts" \
                 || bad "another session took the box from a live holder (rc=$rc)"

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

echo "=== 12. a nested acquire does NOT end the outer hold ==="
# `tmdrive kill` inside `tmdrive run` used to join the outer lock and then
# delete it on exit, taking the token with it — kill+launch inside one hold
# was impossible (MK64 session, 2026-09-23).
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "outer hold" -- /bin/sh -c "
  TM_SESSION=$A TM_SESSION_TITLE=A $TM status >/dev/null 2>&1
  sleep 2
  [ -f '$LOCK/token' ] && echo NESTED_OK || echo NESTED_LOST
  sleep 3
" > /tmp/nested.out 2>&1
if grep -q NESTED_OK /tmp/nested.out; then ok "the outer hold survived a nested acquire"; else bad "a nested acquire destroyed the outer hold"; fi
out=$($TM status 2>&1)
case "$out" in FREE*) ok "the outer hold released normally at the end";; *) bad "outer hold leaked: $out";; esac

echo "=== 14. a nested acquire does NOT change the token ==="
# THE BUG (u10s session, 2026-09-24): the nested ("ours") path minted a FRESH
# token and overwrote the file. The outer `tmdrive run` had already exported
# the old one as TM_LOCK_TOKEN, so every later command in the run carried a
# stale key and the game refused them all.
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "outer hold" -- /bin/sh -c "
  t1=\$(cat '$LOCK/token' 2>/dev/null)
  TM_SESSION=$A TM_SESSION_TITLE=A $TM status >/dev/null 2>&1
  TM_SESSION=$A TM_SESSION_TITLE=A $TM plugin ctx --purpose 'nested' >/dev/null 2>&1
  t2=\$(cat '$LOCK/token' 2>/dev/null)
  [ \"\$t1\" = \"\$t2\" ] && echo TOKEN_STABLE || echo \"TOKEN_CHANGED \$t1 -> \$t2\"
  [ \"\$TM_LOCK_TOKEN\" = \"\$t2\" ] && echo ENV_MATCHES || echo \"ENV_STALE\"
" > /tmp/tok.out 2>&1
grep -q TOKEN_STABLE /tmp/tok.out && ok "the token survives a nested acquire" || bad "a nested acquire changed the token: $(grep TOKEN_ /tmp/tok.out)"
grep -q ENV_MATCHES  /tmp/tok.out && ok "the run's TM_LOCK_TOKEN still matches the file" || bad "the run's TM_LOCK_TOKEN went stale"

echo "=== 15. a nested RELEASE does not free the outer hold ==="
# The other half of the same report: `shootctl lock release` inside a live
# `tmdrive run` matched on session id -- and a nested call is the same
# session -- so it deleted the box out from under the running job.
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "outer hold" -- /bin/sh -c "
  TM_SESSION=$A TM_SESSION_TITLE=A $SHOOTCTL lock release >/dev/null 2>&1
  [ -f '$LOCK/token' ] && echo STILL_HELD || echo LOCK_DESTROYED
  sleep 2
" > /tmp/rel.out 2>&1
grep -q STILL_HELD /tmp/rel.out && ok "a nested release left the outer hold alone" || bad "a nested release destroyed the outer hold"
out=$($TM status 2>&1)
case "$out" in FREE*) ok "and the outer hold released normally at the end";; *) bad "the outer hold leaked: $out";; esac

echo "=== 16. the hold survives the game dying INSIDE the run ==="
# Their exact scenario: a run that starts with no game, launches one, and
# keeps driving it.
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "launch inside the run" -- /bin/sh -c "
  t1=\$(cat '$LOCK/token' 2>/dev/null)
  /mnt/c/Windows/System32/taskkill.exe /F /IM Trackmania.exe >/dev/null 2>&1
  sleep 6
  t2=\$(cat '$LOCK/token' 2>/dev/null)
  [ -n \"\$t2\" ] && [ \"\$t1\" = \"\$t2\" ] && echo HELD_THROUGH || echo \"LOST \$t1 -> \$t2\"
" > /tmp/restart.out 2>&1
grep -q HELD_THROUGH /tmp/restart.out && ok "the token and the hold survive the game dying mid-run" || bad "the hold was lost when the game died: $(cat /tmp/restart.out | tail -1)"
free_lock

echo "=== 17. a SECOND run from the same session is refused (not silently nested) ==="
# u10s, 2026-09-24: a probe run started while the publisher's run was live
# joined its hold and restarted the game under it — twice. Same session is
# not the same job.
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "publisher" -- /bin/sh -c "sleep 12" >/dev/null 2>&1 &
pub=$!
sleep 4
out=$(TM_SESSION=$A TM_SESSION_TITLE="A" env -u TM_LOCK_TOKEN $TM run --purpose "probe" -- /bin/sh -c "echo NESTED_RAN" 2>&1); rc=$?
[ "$rc" = "75" ] && ok "a second run from the same session is refused (rc=75)" || bad "a second run joined the first (rc=$rc): $out"
echo "$out" | grep -q "ALREADY holds" && ok "and it names the job it would have trampled" || bad "no explanation: $out"
out2=$(TM_SESSION=$A TM_SESSION_TITLE="A" env -u TM_LOCK_TOKEN $TM run --nested --purpose "probe" -- /bin/sh -c "echo NESTED_RAN" 2>&1)
echo "$out2" | grep -q NESTED_RAN && ok "--nested opts in deliberately" || bad "--nested did not nest: $out2"
wait $pub 2>/dev/null
free_lock

echo "=== 13. a holder keeps the box across its OWN game restart ==="
# A bisect restarts the game inside its run; the old "game gone => reclaimable"
# rule handed the box to someone else mid-run.
free_lock
TM_SESSION=$A TM_SESSION_TITLE="A" $TM run --purpose "restarting the game myself" -- /bin/sh -c "
  /mnt/c/Windows/System32/taskkill.exe /F /IM Trackmania.exe >/dev/null 2>&1
  sleep 8
" >/dev/null 2>&1 &
runner=$!
sleep 6
out=$(TM_SESSION=$B $TM status 2>&1)
case "$out" in
  *"HELD"*) ok "still HELD while its game is down";;
  *) bad "the box was released during the holder's own restart: $out";;
esac
out=$(TM_SESSION=$B TM_SESSION_TITLE=B $TM plugin ctx --purpose "B barging in mid-restart" 2>&1); rc=$?
[ "$rc" = "75" ] && ok "another session is still refused mid-restart" || bad "another session took the box mid-restart (rc=$rc)"
wait $runner 2>/dev/null
free_lock

echo
echo "================  $pass passed, $fail failed  ================"
[ "$fail" = "0" ] || exit 1
