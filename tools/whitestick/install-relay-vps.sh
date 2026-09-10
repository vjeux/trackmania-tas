#!/bin/sh
# Install the whitestick RELAY on a plain box with a public IP (a VPS), as an
# alternative to the Cloudflare Worker. Run it FROM a devserver:
#
#   sh tools/whitestick/install-relay-vps.sh --host <ip-or-name> [--user vjeux]
#       [--port 8443] [--ssh-key ~/.whitestick-vps/id_ed25519] [--bin <path>]
#
# It needs to reach the box over ssh with a key (password logins are not
# scripted here on purpose). What it does there, all as an unprivileged user —
# no root, no sudo, no systemd unit:
#   1. copies the whitestick binary to ~/bin/whitestick
#   2. generates a self-signed P-256 certificate (10 years) if there is none
#   3. writes ~/.whitestick/config.toml with the shared secret
#   4. installs ~/bin/whitestick-relay-loop.sh (flock + restart) and registers
#      two crontab entries: @reboot and a two-minute watchdog
#   5. starts it, then checks /healthz through fwdproxy from this devserver
# and prints the two config lines the clients need (relay + pin).
#
# Re-running is the upgrade path (it replaces the binary and restarts).
set -eu

HOST=""
USER_AT=vjeux
PORT=8443
KEY="$HOME/.whitestick-vps/id_ed25519"
BIN=""
while [ $# -gt 0 ]; do
    case "$1" in
        --host) HOST=$2; shift 2 ;;
        --user) USER_AT=$2; shift 2 ;;
        --port) PORT=$2; shift 2 ;;
        --ssh-key) KEY=$2; shift 2 ;;
        --bin) BIN=$2; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
: "${HOST:?--host is required}"

REPO=$(cd "$(dirname "$0")/../.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
if [ -z "$BIN" ]; then
    if [ -x "$REPO/tools/target/release/whitestick" ]; then
        BIN="$REPO/tools/target/release/whitestick"
    else
        echo "== building whitestick"
        getent hosts fwdproxy >/dev/null 2>&1 && export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080
        (cd "$REPO/tools" && cargo build --release -p whitestick)
        BIN="$REPO/tools/target/release/whitestick"
    fi
fi

TOKEN=${WHITESTICK_TOKEN:-}
if [ -z "$TOKEN" ] && [ -f "$HOME/.whitestick/config.toml" ]; then
    TOKEN=$(sed -n 's/^token *= *"\(.*\)"/\1/p' "$HOME/.whitestick/config.toml" | head -n1)
fi
: "${TOKEN:?no shared secret: run install-devserver.sh first, or set WHITESTICK_TOKEN}"

# ssh out through fwdproxy when we are on corp, directly otherwise. The
# ProxyCommand carries a space, so it goes through a wrapper function rather
# than a variable that word-splitting would tear in half.
if getent hosts fwdproxy >/dev/null 2>&1; then
    ssh_to() { ssh -o ProxyCommand="fwdproxy_ssh_proxy %h %p" -o BatchMode=yes \
        -o StrictHostKeyChecking=accept-new -o ConnectTimeout=20 -i "$KEY" "$USER_AT@$HOST" "$@"; }
else
    ssh_to() { ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new \
        -o ConnectTimeout=20 -i "$KEY" "$USER_AT@$HOST" "$@"; }
fi

echo "== copying the binary"
ssh_to 'mkdir -p ~/bin ~/.whitestick ~/whitestick'
ssh_to 'cat > ~/bin/whitestick.new && chmod 755 ~/bin/whitestick.new' < "$BIN"

echo "== certificate, config, loop, crontab"
ssh_to "PORT='$PORT' TOKEN='$TOKEN' HOSTADDR='$HOST' sh -s" <<'REMOTE'
set -eu
umask 077
cd "$HOME"
if [ ! -f whitestick/cert.pem ]; then
    # A real end-entity certificate: openssl's -x509 default would set
    # CA:TRUE, which webpki rejects as a server certificate (CaUsedAsEndEntity)
    # — invisible while every client pins, and a trap for any that does not.
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
        -keyout whitestick/key.pem -out whitestick/cert.pem -days 3650 \
        -subj "/CN=whitestick-relay" \
        -addext "subjectAltName=IP:$HOSTADDR,DNS:whitestick-relay" \
        -addext "basicConstraints=critical,CA:FALSE" \
        -addext "keyUsage=critical,digitalSignature,keyEncipherment" \
        -addext "extendedKeyUsage=serverAuth" 2>/dev/null
fi
cat > .whitestick/config.toml <<EOF
# This box IS the relay; the client here talks to it over the loopback.
relay = "https://127.0.0.1:$PORT"
token = "$TOKEN"
instance = "WhiteStick"
proxy = "none"
EOF
cat > bin/whitestick-relay-loop.sh <<EOF
#!/bin/sh
# Keeps exactly one relay alive. Started by cron (@reboot and every 2 min);
# extra copies find the lock taken and exit.
LOG="\$HOME/.whitestick/relay.log"
exec 9>"\$HOME/.whitestick/relay.lock"
flock -n 9 || exit 0
while :; do
    if [ "\$(stat -c %s "\$LOG" 2>/dev/null || echo 0)" -gt 20000000 ]; then
        mv -f "\$LOG" "\$LOG.1"
    fi
    "\$HOME/bin/whitestick" relay --listen 0.0.0.0:$PORT \\
        --cert "\$HOME/whitestick/cert.pem" --key "\$HOME/whitestick/key.pem" >>"\$LOG" 2>&1
    echo "[\$(date -u +%FT%TZ)] loop: relay exited rc=\$?; restarting in 5 s" >>"\$LOG"
    sleep 5
done
EOF
chmod 755 bin/whitestick-relay-loop.sh
umask 022

# Replace the binary, then let the loop restart onto it.
mv -f bin/whitestick.new bin/whitestick

LINE_BOOT='@reboot $HOME/bin/whitestick-relay-loop.sh'
LINE_WATCH='*/2 * * * * $HOME/bin/whitestick-relay-loop.sh'
( crontab -l 2>/dev/null | grep -v whitestick-relay-loop ; echo "$LINE_BOOT" ; echo "$LINE_WATCH" ) | crontab -

# Stop the running relay; whichever loop holds the lock restarts it onto the
# new binary (that takes its 5 s backoff, so give the port time to come back).
pkill -f 'whitestick relay --listen' 2>/dev/null || true
setsid nohup "$HOME/bin/whitestick-relay-loop.sh" >/dev/null 2>&1 < /dev/null &
echo "pin: $("$HOME/bin/whitestick" relay --print-pin --cert "$HOME/whitestick/cert.pem")"
i=0
while [ $i -lt 20 ]; do
    if ss -ltn 2>/dev/null | grep -q ":$PORT"; then break; fi
    sleep 1; i=$((i + 1))
done
echo "listening:"; ss -ltn 2>/dev/null | grep ":$PORT" || { echo "  NOT LISTENING"; tail -5 "$HOME/.whitestick/relay.log"; }
REMOTE

PIN=$(ssh_to "~/bin/whitestick relay --print-pin --cert ~/whitestick/cert.pem")
URL="https://$HOST:$PORT"

echo "== checking $URL/healthz from this devserver"
if getent hosts fwdproxy >/dev/null 2>&1; then
    OUT=$(curl -sk -m 20 -x http://fwdproxy:8080 "$URL/healthz" || true)
    [ -n "$OUT" ] || OUT=$(curl -sk -m 20 --proxy-cert /var/facebook/x509_identities/server.pem \
        --proxy-key /var/facebook/x509_identities/server.pem -x https://fwdproxy:8082 "$URL/healthz" || true)
else
    OUT=$(curl -sk -m 20 "$URL/healthz" || true)
fi
case "$OUT" in
    *whitestick-relay*) echo "   reachable" ;;
    *) echo "   NOT reachable through the proxy (got: ${OUT:-nothing})" ; exit 1 ;;
esac

if [ -f "$HOME/.whitestick/config.toml" ]; then
    sed -i "s#^relay *=.*#relay = \"$URL\"#" "$HOME/.whitestick/config.toml"
    grep -q '^pin *=' "$HOME/.whitestick/config.toml" \
        && sed -i "s#^pin *=.*#pin = \"$PIN\"#" "$HOME/.whitestick/config.toml" \
        || printf 'pin = "%s"\n' "$PIN" >> "$HOME/.whitestick/config.toml"
    echo "== this devserver's config now points at it"
fi

cat <<EOF

Relay is up at $URL
  relay = "$URL"
  pin   = "$PIN"

Box install (inside WSL, on the render box):
  cd ~/trackmania-tas && git pull && \\
  WHITESTICK_RELAY=$URL WHITESTICK_PIN=$PIN WHITESTICK_TOKEN=<the shared secret> \\
  sh tools/whitestick/install-box.sh
EOF
