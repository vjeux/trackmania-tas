#!/bin/sh
# Install the whitestick AGENT on the box (run this INSIDE WSL, as the user
# whose home the commands should land in).
#
#   cd ~/trackmania-tas && git pull && \
#   WHITESTICK_RELAY=https://<relay>.workers.dev WHITESTICK_TOKEN=<secret> \
#   sh tools/whitestick/install-box.sh
#
# What it does:
#   1. builds tools/whitestick (needs the rust toolchain in ~/.cargo/bin)
#   2. installs ~/bin/whitestick and ~/bin/whitestick-agent-loop.sh
#   3. writes ~/.whitestick/config.toml (relay, token, box name, start cwd)
#   4. registers two Windows scheduled tasks for the current Windows user (no
#      admin needed): "WhiteStick Agent" at logon and "WhiteStick Agent
#      Watchdog" every 5 minutes. Both run a hidden launcher that starts the
#      loop inside WSL; the loop takes a lock, so extra starts exit at once.
#   5. starts it and asks the relay whether the box is online.
#
# Re-running is safe; it is also how you upgrade (git pull, then run again).
set -eu

REPO=$(cd "$(dirname "$0")/../.." && pwd)
export PATH="$HOME/.cargo/bin:$HOME/bin:$PATH"

: "${WHITESTICK_RELAY:?set WHITESTICK_RELAY=https://<relay>.workers.dev}"
: "${WHITESTICK_TOKEN:?set WHITESTICK_TOKEN=<the shared secret>}"
NAME=${WHITESTICK_NAME:-WhiteStick}

if ! command -v cargo >/dev/null 2>&1; then
    echo "install-box: cargo not found (expected ~/.cargo/bin/cargo)" >&2
    exit 1
fi
if [ -z "${WSL_DISTRO_NAME:-}" ]; then
    echo "install-box: this does not look like WSL (WSL_DISTRO_NAME unset); the agent" >&2
    echo "  will be installed but no Windows scheduled task can be registered." >&2
fi

echo "== building whitestick"
(cd "$REPO/tools" && cargo build --release -p whitestick)
mkdir -p "$HOME/bin" "$HOME/.whitestick"
install -m 755 "$REPO/tools/target/release/whitestick" "$HOME/bin/whitestick"

# Commands start where navi started them: the Windows user profile.
WINPROFILE=$(cmd.exe /c 'echo %USERPROFILE%' 2>/dev/null | tr -d '\r' || true)
if [ -n "$WINPROFILE" ]; then
    CWD=$(wslpath "$WINPROFILE" 2>/dev/null || echo "$HOME")
else
    CWD=$HOME
fi

echo "== writing ~/.whitestick/config.toml"
umask 077
cat > "$HOME/.whitestick/config.toml" <<EOF
relay = "$WHITESTICK_RELAY"
token = "$WHITESTICK_TOKEN"
instance = "$NAME"
proxy = "none"

[agent]
name = "$NAME"
cwd = "$CWD"
shell = "/bin/sh"
EOF
umask 022

cat > "$HOME/bin/whitestick-agent-loop.sh" <<'EOF'
#!/bin/sh
# Keeps exactly one `whitestick agent` alive. Safe to start as often as you
# like: a second copy finds the lock taken and exits.
LOG="$HOME/.whitestick/agent.log"
mkdir -p "$HOME/.whitestick"
exec 9>"$HOME/.whitestick/agent-loop.lock"
flock -n 9 || exit 0
export PATH="$HOME/bin:$PATH"
while :; do
    if [ "$(stat -c %s "$LOG" 2>/dev/null || echo 0)" -gt 10000000 ]; then
        mv -f "$LOG" "$LOG.1"
    fi
    "$HOME/bin/whitestick" agent >>"$LOG" 2>&1
    echo "[$(date -u +%FT%TZ)] loop: agent exited rc=$?; restarting in 5 s" >>"$LOG"
    sleep 5
done
EOF
chmod 755 "$HOME/bin/whitestick-agent-loop.sh"

if [ -n "${WSL_DISTRO_NAME:-}" ] && [ -n "$WINPROFILE" ]; then
    echo "== registering Windows scheduled tasks"
    WUSER=$(id -un)
    WINDIR_W="$WINPROFILE\\whitestick"
    WINDIR=$(wslpath "$WINDIR_W")
    mkdir -p "$WINDIR"
    # A .vbs launcher so no console window pops up at logon.
    printf 'Set sh = CreateObject("WScript.Shell")\r\nsh.Run "wsl.exe -d %s -u %s -- /bin/sh -lc ~/bin/whitestick-agent-loop.sh", 0, False\r\n' \
        "$WSL_DISTRO_NAME" "$WUSER" > "$WINDIR/start-agent.vbs"
    case "$WINDIR_W" in
        *" "*) echo "warning: the Windows profile path has spaces; check the tasks in Task Scheduler" ;;
    esac
    TR="wscript.exe $WINDIR_W\\start-agent.vbs"
    if ! schtasks.exe /Create /F /TN "WhiteStick Agent" /SC ONLOGON /TR "$TR" >/dev/null 2>&1; then
        echo "   (could not create the logon task without admin rights; the 5-minute watchdog covers it)"
    fi
    schtasks.exe /Create /F /TN "WhiteStick Agent Watchdog" /SC MINUTE /MO 5 /TR "$TR" >/dev/null
    schtasks.exe /Run /TN "WhiteStick Agent Watchdog" >/dev/null 2>&1 || true
else
    echo "== starting the agent loop directly (no Task Scheduler)"
    nohup "$HOME/bin/whitestick-agent-loop.sh" >/dev/null 2>&1 &
fi

echo "== waiting for the agent to report in"
i=0
while [ $i -lt 15 ]; do
    if "$HOME/bin/whitestick" status >/dev/null 2>&1; then break; fi
    sleep 1; i=$((i + 1))
done
"$HOME/bin/whitestick" status || {
    echo "agent not online yet; last log lines:" >&2
    tail -n 20 "$HOME/.whitestick/agent.log" >&2 || true
    exit 1
}
echo "== done. Log: ~/.whitestick/agent.log"
if schtasks.exe /Query /FO LIST 2>/dev/null | grep -qi navi; then
    echo "note: a navi scheduled task still exists; navibot.dev is gone, so it can be deleted."
fi
