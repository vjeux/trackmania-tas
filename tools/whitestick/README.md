# whitestick — devserver → WhiteStick box, without navi

`whitestick '<command>'` runs a command on the WhiteStick render box (Windows +
WSL, at home, on no network Meta can reach) from a Meta devserver or OD, and
streams stdin/stdout/stderr and the exit status back. It replaces the
navibot.dev bridge, which was shut down on 2026-09-09.

```
   devserver / OD                     Cloudflare                        home
  ┌──────────────┐  wss (via fwdproxy) ┌────────────────────┐  wss   ┌───────────────────┐
  │ whitestick   │────────────────────▶│ whitestick-relay   │◀───────│ whitestick agent  │
  │ '<cmd>'      │   /v1/ctl/WhiteStick│ Worker + Durable   │ /v1/   │ (WSL, as vjeux)   │
  └──────────────┘                     │ Object "box:..."   │ agent  └───────────────────┘
                                       └────────────────────┘
```

Both ends dial **out** over HTTPS; nothing accepts an inbound connection, no
port is forwarded at home, and nothing Meta-internal is in the path (the
devserver's only special step is fwdproxy, which the client handles itself).
The relay is ~200 lines of Rust on Cloudflare's free plan and never looks
inside the traffic; it just pairs sockets.

## Using it (devserver side)

```
whitestick 'echo hi; df -h /mnt/c'                # run; exit status is the remote one
whitestick 'cat > ~/in.bin; md5sum ~/in.bin' < f  # stdin is forwarded (new; navi dropped it)
whitestick 'cat ~/big.mp4' > big.mp4              # stdout streams; no 5 MB ceiling any more
echo 'ls' | whitestick                            # command on stdin (the wsx contract)
whitestick --json 'cmd'                           # {"success","stdout","stderr","exitCode"} like navi
whitestick --cwd /mnt/c/Users/vjeux/tm-video 'ls' # start somewhere else (default: the Windows profile)
whitestick --shell /bin/bash 'echo $BASH_VERSION' # default shell is /bin/sh (dash), as before
whitestick --timeout 600 'long thing'             # kill it and exit 124 after 10 min
whitestick --wait 300 'true'                      # wait up to 5 min for the box to come online
whitestick status                                 # is the agent connected? (exit 3 if not)
```

Ctrl-C once sends SIGINT to the remote command; twice kills it. A command's
process group is killed when the client goes away mid-run, so a daemon started
from a command must detach with `setsid ... </dev/null >log 2>&1 &` (a plain
`&` is killed with the group; a plain `&` that keeps stdout open also delays
the exit by 10 s while the agent waits for the pipe).

Config is `~/.whitestick/config.toml` (`relay`, `token`, `instance`, optional
`proxy`); environment `WHITESTICK_RELAY` / `WHITESTICK_TOKEN` /
`WHITESTICK_PROXY` (`none` = direct) override it. The proxy defaults to
`fwdproxy:8080` wherever that name resolves, so the same binary and config
work on any devserver or OD — no per-box setup like `.navi/credentials.json`.

Latency is one WebSocket handshake through fwdproxy plus the relay hop
(~0.3–0.6 s to start a command), then the network's speed. The old bridge's
random 15 s stalls and the 900 KB command / 5 MB reply ceilings are gone; the
one hard limit is the relay's 1 MiB per message, which caps the command
string itself at about 1 MB — stdin/stdout are chunked and unbounded.

## Setup, once

1. **Relay.** Free Cloudflare account, API token with the *Edit Cloudflare
   Workers* template, and the account ID (dashboard → Workers & Pages →
   right sidebar). Then on a devserver:
   ```
   sh tools/whitestick/install-devserver.sh          # builds, generates the shared secret
   CLOUDFLARE_API_TOKEN=… CLOUDFLARE_ACCOUNT_ID=… sh tools/whitestick/deploy-relay.sh
   ```
   `deploy-relay.sh` prints the relay URL, sets the Worker's `PSK` secret to
   the shared secret and points `~/.whitestick/config.toml` at the URL. It is
   also the redeploy command after editing `tools/whitestick-relay`.
2. **Box** (inside WSL, as the user the commands should run as):
   ```
   cd ~/trackmania-tas && git pull
   WHITESTICK_RELAY=https://<relay>.workers.dev WHITESTICK_TOKEN=<secret> sh tools/whitestick/install-box.sh
   ```
   Builds the agent, writes its config, installs `~/bin/whitestick-agent-loop.sh`
   and registers two Windows scheduled tasks for the current user (no admin):
   *WhiteStick Agent* at logon and *WhiteStick Agent Watchdog* every 5
   minutes. The loop holds a lock, so the watchdog is a no-op while the agent
   is up and a restart within 5 minutes after a `wsl --shutdown` or a crash.
   Log: `~/.whitestick/agent.log` (rotated at 10 MB).
3. **Other devservers/ODs** get `~/bin/whitestick` through the home sync;
   only `~/.whitestick/config.toml` has to exist (copy it — it is two lines
   that matter).

## Operating it

| symptom | meaning / fix |
|---|---|
| `WhiteStick is offline: no agent is connected` | the box is down, asleep, or WSL was shut down. The watchdog task restarts the agent within 5 min; `whitestick --wait 600 …` waits for it. On the box: `~/.whitestick/agent.log`. |
| `the relay rejected the token (401)` | `token` in the config differs from the Worker's `PSK` secret: rerun `deploy-relay.sh` (it re-sets the secret from the config). |
| `cannot reach the relay … via proxy fwdproxy:8080` | fwdproxy trouble on this devserver (`fixmyproxy`), or the Worker is gone — `curl -x fwdproxy:8080 https://<relay>/healthz` should say `whitestick-relay`. |
| `went offline mid-command` | the agent's socket dropped while a command ran; the command was killed with its group. Rerun. |
| command exits but `whitestick` returns 10 s later | something the command started kept stdout open (a background job without redirection). Redirect or `setsid` it. |

The relay's own view: `whitestick status` (agent connected since when, live
sessions). Cloudflare dashboard → the Worker → Logs shows edge errors.

## Security

One shared secret authenticates both roles at the edge (constant-time compare,
before anything reaches the Durable Object); everything else is TLS to
Cloudflare. Whoever holds the secret can run commands on the box as `vjeux`
— it lives in `~/.whitestick/config.toml` on the devserver (mode 600) and on
the box, and as the Worker secret. Rotate by editing both configs and
rerunning `deploy-relay.sh`. The relay code is ours and sees the plaintext of
commands and output; Cloudflare could too. This channel carries Trackmania
tooling, nothing else.

## Layout

- `tools/whitestick` — the one binary: client (default), `agent`, `status`.
  `proto.rs` is the wire format, `transport.rs` the proxy/TLS/WebSocket path,
  `agent.rs` and `client.rs` the two ends.
- `tools/whitestick-relay` — the Worker (workers-rs). Not a workspace member:
  it targets wasm32 and is built by `worker-build` via `wrangler deploy`.
- `wsx` still works unchanged (it talks to `~/bin/whitestick` over stdin); its
  parallel-chunk design was a workaround for navi's cost model and could now
  be a plain `whitestick 'cat > f' < f`.

## History

The previous client (`~/src/whitestick` on devvm42752, Aug 2026) POSTed to
`https://navibot.dev/api/v1/cli/dispatch` with the navi user token and got
`{stdout, stderr, exitCode}` back after the command finished. navibot.dev
stopped answering on 2026-09-09; `navi-node` on the box had already been
crash-looping (SIGABRT, 6.7 GB WSL dumps each) for weeks.
