# whitestick — devserver → WhiteStick box, without navi

`whitestick '<command>'` runs a command on the WhiteStick render box (Windows +
WSL, at home, on no network Meta can reach) from a Meta devserver or OD, and
streams stdin/stdout/stderr and the exit status back. It replaces the
navibot.dev bridge, which was shut down on 2026-09-09.

```
   devserver / OD                    a box with a public IP              home
  ┌──────────────┐  wss (via fwdproxy) ┌────────────────────┐  wss   ┌───────────────────┐
  │ whitestick   │────────────────────▶│ whitestick relay   │◀───────│ whitestick agent  │
  │ '<cmd>'      │   /v1/ctl/WhiteStick│  (or the Worker)   │ /v1/   │ (WSL, as vjeux)   │
  └──────────────┘                     └────────────────────┘ agent  └───────────────────┘
```

The rendezvous point comes in two interchangeable shapes, same wire protocol:
**`whitestick relay`** on any VPS (what runs today, on `195.154.114.196:8443`)
or the Cloudflare Worker in `tools/whitestick-relay`. The box and the
devservers cannot see each other — only this middle.

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

1. **Relay, on a VPS** (the current arrangement). From a devserver, with ssh
   key access to the VPS:
   ```
   sh tools/whitestick/install-devserver.sh                       # builds, generates the shared secret
   sh tools/whitestick/install-relay-vps.sh --host <ip> --user <user>
   ```
   That copies the binary, makes a self-signed certificate, runs the relay
   under a flock'd loop with `@reboot` + a 2-minute cron watchdog — all as an
   unprivileged user, no root, no systemd — and prints the `relay` and `pin`
   lines. It is also the upgrade command. Re-running after a certificate
   change prints a new pin, which every client config needs.

   *Or the Cloudflare Worker instead:* API token with the *Edit Cloudflare
   Workers* template plus the account ID, then
   `CLOUDFLARE_API_TOKEN=… CLOUDFLARE_ACCOUNT_ID=… sh tools/whitestick/deploy-relay.sh`.
   A Worker has a CA-signed certificate, so its clients need no `pin`.
2. **Box** (inside WSL, as the user the commands should run as). The box
   cannot reach anything at Meta, so the secret has to come from the relay
   box or by hand:
   ```
   scp <user>@<relay-ip>:whitestick-box-setup.sh ~/ && sh ~/whitestick-box-setup.sh
   ```
   or directly:
   ```
   cd ~/trackmania-tas && git pull
   WHITESTICK_RELAY=https://<ip>:8443 WHITESTICK_PIN=<pin> WHITESTICK_TOKEN=<secret> \
     sh tools/whitestick/install-box.sh
   ```
   Builds the agent, writes its config, starts it, and installs a hidden
   `whitestick-agent.vbs` in the Windows **Startup folder** — no admin, no
   Task Scheduler (`schtasks` refuses these tasks on this box: *Invalid
   argument*). The .vbs loops on the Windows side, so it restarts the agent
   after a logout, a crash, or a `wsl --shutdown` — measured at ~15 s. Inside
   WSL a flock'd loop restarts it 5 s after any exit. Log:
   `~/.whitestick/agent.log` (rotated at 10 MB).
3. **Other devservers/ODs** get `~/bin/whitestick` through the home sync;
   only `~/.whitestick/config.toml` has to exist (copy it — it is two lines
   that matter).

## Operating it

| symptom | meaning / fix |
|---|---|
| `WhiteStick is offline: no agent is connected` | the box is down, asleep, or WSL was shut down. The watchdog task restarts the agent within 5 min; `whitestick --wait 600 …` waits for it. On the box: `~/.whitestick/agent.log`. |
| `the relay rejected the token (401)` | `token` in the config differs from the Worker's `PSK` secret: rerun `deploy-relay.sh` (it re-sets the secret from the config). |
| `cannot reach the relay … via proxy fwdproxy:8082` | fwdproxy trouble on this devserver (`fixmyproxy`), or the relay is down — `curl -k -x fwdproxy:8080 https://<relay>/healthz` should say `whitestick-relay`. On the VPS: `tail ~/.whitestick/relay.log`, and the cron watchdog restarts it within 2 minutes. |
| `relay certificate does not match the pin in the config` | the relay's certificate was regenerated: copy the new `pin` (printed by `install-relay-vps.sh`, or `whitestick relay --print-pin --cert ~/whitestick/cert.pem` on the VPS) into every `~/.whitestick/config.toml`. |
| `CaUsedAsEndEntity` | a certificate made with openssl's `-x509` default (CA:TRUE). Regenerate with the extensions `install-relay-vps.sh` passes. |
| `went offline mid-command` | the agent's socket dropped while a command ran; the command was killed with its group. Rerun. |
| command exits but `whitestick` returns 10 s later | something the command started kept stdout open (a background job without redirection). Redirect or `setsid` it. |

The relay's own view: `whitestick status` (agent connected since when, live
sessions). Cloudflare dashboard → the Worker → Logs shows edge errors.

## Security

One shared secret authenticates both roles, checked in constant time before a
socket is paired with anything; everything else is TLS. A self-hosted relay's
certificate is self-signed and pinned by SHA-256 in each client's config
(`pin = …`), which is a tighter promise than CA trust: only that exact
certificate is accepted, so a mis-issued public certificate for the IP buys
nothing. Whoever holds the secret can run commands on the box as `vjeux`
— it lives in `~/.whitestick/config.toml` on the devserver (mode 600) and on
the box, and as the Worker secret. Rotate by editing both configs and
rerunning `deploy-relay.sh`. The relay code is ours and sees the plaintext of
commands and output; Cloudflare could too. This channel carries Trackmania
tooling, nothing else.

## Layout

- `tools/whitestick` — the one binary: client (default), `agent`, `status`.
  `proto.rs` is the wire format, `transport.rs` the proxy/TLS/WebSocket path,
  `agent.rs` and `client.rs` the two ends.
- `tools/whitestick/src/relay.rs` — `whitestick relay`, the self-hosted
  rendezvous point (TLS, one process, no dependencies on the box it runs on).
- `tools/whitestick-relay` — the same thing as a Cloudflare Worker
  (workers-rs). Not a workspace member: it targets wasm32 and is built by
  `worker-build` via `wrangler deploy`.
- `wsx` still works unchanged (it talks to `~/bin/whitestick` over stdin); its
  parallel-chunk design was a workaround for navi's cost model and could now
  be a plain `whitestick 'cat > f' < f`.

## What it looks like when it works

```
$ whitestick 'echo "host: $(hostname)"; whoami; df -h /mnt/c | tail -1'
host: WhiteStick
vjeux
C:\             937G  924G   13G  99% /mnt/c
```

~0.65 s per command from a devserver (fwdproxy → Paris → home). A 3 MB push
takes ~2.7 s, the same pull ~2.0 s.

## History

The previous client (`~/src/whitestick` on devvm42752, Aug 2026) POSTed to
`https://navibot.dev/api/v1/cli/dispatch` with the navi user token and got
`{stdout, stderr, exitCode}` back after the command finished. navibot.dev
stopped answering on 2026-09-09; `navi-node` on the box had already been
crash-looping (SIGABRT, 6.7 GB WSL dumps each) for weeks.
