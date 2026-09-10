#!/bin/sh
# Install the whitestick AGENT on the box (run this INSIDE WSL, as the user
# whose home the commands should land in).
#
#   cd ~/trackmania-tas && git pull && \
#   WHITESTICK_RELAY=https://<relay> WHITESTICK_TOKEN=<secret> [WHITESTICK_PIN=<pin>] \
#   sh tools/whitestick/install-box.sh
#
# WHITESTICK_PIN is the self-hosted relay's certificate pin (printed by
# install-relay-vps.sh); leave it unset for a Cloudflare Worker relay.
#
# What it does:
#   1. builds tools/whitestick (needs the rust toolchain in ~/.cargo/bin)
#   2. installs ~/bin/whitestick and ~/bin/whitestick-agent-loop.sh
#   3. writes ~/.whitestick/config.toml (relay, token, box name, start cwd)
#   4. starts the agent, and installs a hidden .vbs in the Windows Startup
#      folder that keeps it running across logons and `wsl --shutdown`
#      (no admin, no Task Scheduler -- schtasks refuses these tasks here)
#   5. asks the relay whether the box is online.
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
# Separate from the heredoc on purpose: inside one, the quotes in
# ${VAR:+pin = "$VAR"} are removed by the shell and TOML rejects a bare hex
# value ("expected newline"). Cost an evening once.
if [ -n "${WHITESTICK_PIN:-}" ]; then
    printf 'pin = "%s"\n' "$WHITESTICK_PIN" >> "$HOME/.whitestick/config.toml"
fi
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

echo "== starting the agent"
setsid --fork "$HOME/bin/whitestick-agent-loop.sh" >/dev/null 2>&1 </dev/null ||
    nohup "$HOME/bin/whitestick-agent-loop.sh" >/dev/null 2>&1 </dev/null &

# Autostart WITHOUT Task Scheduler. schtasks refuses these tasks on the render
# box ("Invalid argument", and the ONLOGON one needs admin anyway), while a
# .vbs in the Startup folder needs no privileges at all -- and because it
# loops on the WINDOWS side it also restarts the agent after `wsl --shutdown`,
# which is what the 5-minute watchdog task was there for.
#
# Nothing below may block: this script once hung here because it launched the
# never-returning .vbs through cmd.exe and interop waited on it. So: no
# Windows program is started, and every interop call has a timeout.
if [ -n "${WSL_DISTRO_NAME:-}" ]; then
    echo "== autostart at logon"
    STARTUP=""
    for d in "/mnt/c/Users/$(id -un)/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup" \
             "$CWD/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup"; do
        [ -d "$d" ] && { STARTUP=$d; break; }
    done
    if [ -z "$STARTUP" ] && [ -n "$WINPROFILE" ]; then
        A=$(timeout 15 cmd.exe /c 'echo %APPDATA%' 2>/dev/null | tr -d '\r')
        [ -n "$A" ] && STARTUP=$(timeout 10 wslpath "$A\\Microsoft\\Windows\\Start Menu\\Programs\\Startup" 2>/dev/null)
    fi
    if [ -n "$STARTUP" ] && [ -d "$STARTUP" ]; then
        {
            printf 'Set sh = CreateObject("WScript.Shell")\r\n'
            printf 'Do\r\n'
            printf '  sh.Run "wsl.exe -d %s -u %s -- /bin/sh -lc ~/bin/whitestick-agent-loop.sh", 0, True\r\n' \
                "$WSL_DISTRO_NAME" "$(id -un)"
            printf '  WScript.Sleep 15000\r\n'
            printf 'Loop\r\n'
        } > "$STARTUP/whitestick-agent.vbs" 2>/dev/null &&
            echo "   $STARTUP/whitestick-agent.vbs" ||
            echo "   could not write the Startup folder; the agent runs now but not after a logout" >&2
    else
        echo "   Startup folder not found; the agent runs now but not after a logout" >&2
    fi
    # Tasks from older installs: they never worked here.
    timeout 15 schtasks.exe /Delete /F /TN "WhiteStick Agent" >/dev/null 2>&1
    timeout 15 schtasks.exe /Delete /F /TN "WhiteStick Agent Watchdog" >/dev/null 2>&1
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
if crontab -l 2>/dev/null | grep -qi navi; then
    echo "note: navi cron entries are still installed; navibot.dev is gone, so they can be removed."
fi
