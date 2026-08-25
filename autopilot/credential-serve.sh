#!/bin/sh
# The credential server's launcher — the ONE piece of this harness that is not
# Rust, because cron needs an entry point and this is ten lines of it. Every
# decision it makes is in `tmhaul credential serve`.
#
# It lives here in the repo so a rebuilt devserver can restore it; the copy
# that runs is `~/tmhaul/serve.sh` on devvm42752, started by cron:
#
#   @reboot sleep 60 && /home/vjeux/tmhaul/serve.sh >> /tmp/tmhaul-serve.log 2>&1
#   */15 * * * *      /home/vjeux/tmhaul/serve.sh >> /tmp/tmhaul-serve.log 2>&1
#
# It runs on the DEVSERVER because that is the machine holding the bridge
# credential and the only one that can reach a fresh on-demand box: an OD
# cannot ssh to a devserver, and the reverse works.
set -e
export PATH="$HOME/.cargo/bin:$PATH"
export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080

# ONE INSTANCE, AND A CRASH MUST NOT WEDGE IT FOREVER.
#
# A lock directory with a trap looks right and is not: the trap does not run on
# SIGKILL or a reboot, so the directory outlives the process and every cron
# re-run then exits immediately, believing a server is up. Observed here. So
# the lock records its PID and a stale one is reclaimed.
LOCK=/tmp/tmhaul-serve.lock
if ! mkdir "$LOCK" 2>/dev/null; then
  if [ -f "$LOCK/pid" ] && kill -0 "$(cat "$LOCK/pid")" 2>/dev/null; then
    exit 0            # a live server holds it
  fi
  rm -rf "$LOCK"      # stale: whoever held it is gone
  mkdir "$LOCK" || exit 0
fi
echo $$ > "$LOCK/pid"
trap 'rm -rf "$LOCK"' EXIT

[ -d /tmp/tmtas ] || git clone -q https://github.com/vjeux/trackmania-tas.git /tmp/tmtas
git -C /tmp/tmtas pull -q --ff-only origin main || true
cd /tmp/tmtas/tools && cargo build --release -p haul >/dev/null 2>&1
exec /tmp/tmtas/tools/target/release/tmhaul credential serve --every-s 300
