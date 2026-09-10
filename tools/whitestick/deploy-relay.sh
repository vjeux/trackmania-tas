#!/bin/sh
# Deploy (or redeploy) the whitestick-relay Worker to Cloudflare.
#
#   CLOUDFLARE_API_TOKEN=... CLOUDFLARE_ACCOUNT_ID=... sh tools/whitestick/deploy-relay.sh
#
# or put those two lines (KEY=value) in ~/.cloudflare/whitestick-relay.env.
# The API token needs the "Edit Cloudflare Workers" template permissions.
#
# Needs: node >= 20 on PATH (for wrangler), rust with the wasm32 target
# (installed here if missing), worker-build (installed here if missing).
# On a devserver everything goes out through fwdproxy automatically.
#
# The Worker's PSK secret is set to the `token` in ~/.whitestick/config.toml
# (or $WHITESTICK_TOKEN), so the client on this machine works right after.
set -eu

cd "$(dirname "$0")/../whitestick-relay"
export PATH="$HOME/.cargo/bin:$HOME/.local/node/bin:$PATH"
[ -f "$HOME/.cloudflare/whitestick-relay.env" ] && . "$HOME/.cloudflare/whitestick-relay.env"
: "${CLOUDFLARE_API_TOKEN:?set CLOUDFLARE_API_TOKEN}"
: "${CLOUDFLARE_ACCOUNT_ID:?set CLOUDFLARE_ACCOUNT_ID}"
export CLOUDFLARE_API_TOKEN CLOUDFLARE_ACCOUNT_ID
export WRANGLER_SEND_METRICS=false CI=1

if getent hosts fwdproxy >/dev/null 2>&1; then
    export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080
    export HTTPS_PROXY=http://fwdproxy:8080 HTTP_PROXY=http://fwdproxy:8080
    export no_proxy=localhost,127.0.0.1 NO_PROXY=localhost,127.0.0.1
fi

TOKEN=${WHITESTICK_TOKEN:-}
if [ -z "$TOKEN" ] && [ -f "$HOME/.whitestick/config.toml" ]; then
    TOKEN=$(sed -n 's/^token *= *"\(.*\)"/\1/p' "$HOME/.whitestick/config.toml" | head -n1)
fi
: "${TOKEN:?no token: set WHITESTICK_TOKEN or run install-devserver.sh first}"

echo "== toolchain"
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
command -v worker-build >/dev/null 2>&1 || cargo install -q worker-build
if ! [ -x node_modules/.bin/wrangler ]; then
    npm config set proxy "${http_proxy:-}" >/dev/null 2>&1 || true
    npm config set https-proxy "${https_proxy:-}" >/dev/null 2>&1 || true
    npm install --no-save --no-audit --no-fund wrangler@4 >/dev/null
fi

# A workers.dev subdomain must exist once per account; registering is idempotent.
SUB=${WORKERS_DEV_SUBDOMAIN:-}
if [ -n "$SUB" ]; then
    curl -sS -X PUT "https://api.cloudflare.com/client/v4/accounts/$CLOUDFLARE_ACCOUNT_ID/workers/subdomain" \
        -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" -H 'Content-Type: application/json' \
        --data "{\"subdomain\":\"$SUB\"}" >/dev/null || true
fi

echo "== deploying"
node_modules/.bin/wrangler deploy 2>&1 | tee /tmp/wrangler-deploy.log
URL=$(grep -o 'https://[a-zA-Z0-9.-]*workers\.dev' /tmp/wrangler-deploy.log | head -n1 || true)

echo "== setting the PSK secret"
printf '%s' "$TOKEN" | node_modules/.bin/wrangler secret put PSK >/dev/null

if [ -n "$URL" ]; then
    echo "== relay is at $URL"
    if [ -f "$HOME/.whitestick/config.toml" ]; then
        sed -i "s#^relay *=.*#relay = \"$URL\"#" "$HOME/.whitestick/config.toml"
        echo "== ~/.whitestick/config.toml now points at it"
    fi
    echo "== box install line:"
    echo "   WHITESTICK_RELAY=$URL WHITESTICK_TOKEN=<token from ~/.whitestick/config.toml> sh tools/whitestick/install-box.sh"
else
    echo "deployed, but could not read the URL from wrangler's output; see /tmp/wrangler-deploy.log"
fi
