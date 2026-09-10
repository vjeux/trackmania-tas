#!/bin/sh
# Install the whitestick CLIENT on a devserver / OD.
#
#   sh tools/whitestick/install-devserver.sh --relay https://<relay>.workers.dev [--token <secret>]
#
# - builds tools/whitestick if cargo is available (or uses $WHITESTICK_BIN)
# - installs ~/bin/whitestick; the old navi client is kept as ~/bin/whitestick.navi
# - writes ~/.whitestick/config.toml; without --token a fresh secret is generated
#   (that is the one to hand to the box install and to the relay's PSK secret)
set -eu

REPO=$(cd "$(dirname "$0")/../.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
RELAY=""
TOKEN=""
INSTANCE=${WHITESTICK_NAME:-WhiteStick}
while [ $# -gt 0 ]; do
    case "$1" in
        --relay) RELAY=$2; shift 2 ;;
        --token) TOKEN=$2; shift 2 ;;
        --instance) INSTANCE=$2; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

CFG="$HOME/.whitestick/config.toml"
if [ -z "$TOKEN" ] && [ -f "$CFG" ]; then
    TOKEN=$(sed -n 's/^token *= *"\(.*\)"/\1/p' "$CFG" | head -n1)
fi
if [ -z "$TOKEN" ]; then
    TOKEN=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')
    echo "generated a new shared secret (also needed by the box and the relay):"
    echo "  $TOKEN"
fi
if [ -z "$RELAY" ] && [ -f "$CFG" ]; then
    RELAY=$(sed -n 's/^relay *= *"\(.*\)"/\1/p' "$CFG" | head -n1)
fi

if [ -n "${WHITESTICK_BIN:-}" ]; then
    BIN=$WHITESTICK_BIN
elif command -v cargo >/dev/null 2>&1; then
    echo "== building whitestick"
    if getent hosts fwdproxy >/dev/null 2>&1; then
        export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080
    fi
    (cd "$REPO/tools" && cargo build --release -p whitestick)
    BIN="$REPO/tools/target/release/whitestick"
else
    echo "no cargo and no WHITESTICK_BIN: cannot build" >&2
    exit 1
fi

mkdir -p "$HOME/bin" "$HOME/.whitestick"
if [ -f "$HOME/bin/whitestick" ] && ! [ -f "$HOME/bin/whitestick.navi" ]; then
    if strings "$HOME/bin/whitestick" 2>/dev/null | grep -q navibot.dev; then
        mv "$HOME/bin/whitestick" "$HOME/bin/whitestick.navi"
        echo "kept the old navi client as ~/bin/whitestick.navi"
    fi
fi
install -m 755 "$BIN" "$HOME/bin/whitestick"

umask 077
cat > "$CFG" <<EOF
relay = "$RELAY"
token = "$TOKEN"
instance = "$INSTANCE"
# proxy: unset = fwdproxy when that name resolves (8082 with the host cert, else 8080), direct otherwise
EOF
umask 022
echo "== wrote $CFG (relay: ${RELAY:-<none yet>})"
"$HOME/bin/whitestick" --version
if [ -n "$RELAY" ]; then
    "$HOME/bin/whitestick" status || true
fi
