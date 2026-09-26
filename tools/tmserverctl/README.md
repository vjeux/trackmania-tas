# Yannex — low-g: a Trackmania 2020 dedicated server on the VPS `fooo`

A Nadeo dedicated server (Linux build 2026-05-16) on `195.154.114.196` (Scaleway, Debian 13,
2 cores / 4 GB, user `vjeux`, **no root**), running the custom **LowG** mode: Nadeo's online
Time Attack plus a gravity coefficient per player, car-to-car collisions and "balloon" bots
to push around. Everything lives under `~/tmserver/` and is driven by `tmserverctl`, a Rust
supervisor from `trackmania-tas/tools/tmserverctl` (no root, no systemd, no shell scripts).

Status (2026-09-26 04:31Z): INSTALLED on the VPS under ~/tmserver (archive md5
b2de4fca0e42e9800f82e6df55df32bf, 386 MB), smoke-tested there in `/lan` mode (mode compiles, YannexDoor
loads, bots take gravity 0.05, 2350 TCP+UDP listening, XML-RPC on 127.0.0.1:5000, RSS ~160 MB), then
stopped with `lan = false`. Waiting for the server account (Invalid credentials otherwise) and the
security-group rules; activation = `tmserverctl account <login> <pw> && tmserverctl start` + the cron lines.

## What is where

```
~/tmserver/
  bin/tmserverctl                 the supervisor / CLI (x86-64, glibc >= 2.34)
  tmserverctl.conf                where the server is, how it is started (key = value)
  server/                         the unpacked Nadeo archive
    TrackmaniaServer              the server binary
    Packs/                        game data (dedicated_TMStadium.pak, ...)
    UserData/Config/dedicated_cfg.txt         server name, account, ports, XML-RPC passwords
    UserData/Maps/MatchSettings/lowg.txt      playlist + mode settings
    UserData/Maps/Yannex/*.Map.Gbx            the maps
    UserData/Scripts/Modes/TrackMania/LowG.Script.txt   the mode (extends TM_TimeAttack_Online)
    UserData/Scripts/Libs/LowG/LowG.Script.txt          its state and logic
  run/tmserverctl.pid, run/server.pid
  logs/server.log (+ .1 .. .5)    server output + supervisor + chat-bridge notes
```

Ports: game **2350 TCP+UDP** (public), P2P 3450 TCP+UDP (legacy, harmless to open),
XML-RPC 5000 **localhost only**. The WhiteStick relay on 8443 is untouched.

## Install on the VPS (as `vjeux`, no root)

From the devserver that holds the build (`~/trackmania-tas/tools/target/release/tmserverctl`)
and the deploy files (`~/trackmania-tas/tools/tmserverctl/deploy/`). From a Meta box the VPS
is reachable only through fwdproxy:

```
# ~/.ssh/config on the devserver
Host fooo
    HostName 195.154.114.196
    User vjeux
    IdentityFile ~/.ssh/id_ed25519_fooo
    IdentitiesOnly yes
    ProxyCommand ncat --proxy fwdproxy:8080 --proxy-type http %h %p
```
then `ssh fooo ...` and `scp ... fooo:...` below (`vps` = `ssh fooo`, `vpscp` = `scp`).

1. Layout and the Nadeo archive (253 MB, downloaded by the VPS itself):
   ```
   vps 'mkdir -p ~/tmserver/bin ~/tmserver/server && cd ~/tmserver/server && curl -sSfo server.zip http://files.v04.maniaplanet.com/server/TrackmaniaServer_Latest.zip && unzip -oq server.zip && rm server.zip && chmod +x TrackmaniaServer && ./TrackmaniaServer /nodaemon /help | head -1'
   ```
   The last command prints `Starting Trackmania date=2026-05-15_18_00 ...` and exits (the
   `/help` flag has no help text; without `/nodaemon` the binary would fork into the
   background — never run it without `/nodaemon`). If `unzip` is missing on the box, use
   `busybox unzip server.zip` or `python3 -m zipfile -e server.zip .`.
2. Our files:
   ```
   cd ~/trackmania-tas/tools/tmserverctl
   vpscp target/../../target/release/tmserverctl vjeux@195.154.114.196:tmserver/bin/tmserverctl
   vpscp deploy/tmserverctl.conf vjeux@195.154.114.196:tmserver/
   vpscp -r deploy/UserData/Config deploy/UserData/Scripts deploy/UserData/Maps vjeux@195.154.114.196:tmserver/server/UserData/
   ```
   (`scp -r` merges into the existing `UserData/`; the maps under `deploy/UserData/Maps/Yannex/`
   are copied from the devserver, they are not in the public repo.)
3. Secrets, then check:
   ```
   vps '~/tmserver/bin/tmserverctl passwords && ~/tmserver/bin/tmserverctl account <SERVER_LOGIN> <SERVER_PASSWORD> && ~/tmserver/bin/tmserverctl check'
   ```
   `passwords` writes random XML-RPC passwords; `account` writes the dedicated-server account
   (see below). `check` prints the exact command line and verifies every map exists.
4. Start, watch, verify:
   ```
   vps '~/tmserver/bin/tmserverctl start'
   vps '~/tmserver/bin/tmserverctl logs -n 40'      # "...Load succeeds", "Script 'Mode:LowG': LowG ... loaded"
   vps '~/tmserver/bin/tmserverctl status'          # status "Running - Play", public ip 195.154.114.196:2350
   ```
   A wrong account shows in the log as `Connecting to master server... ...ERROR: Invalid
   credentials. (code NadeoServices/0x00000191)` and the server exits (the supervisor retries
   with a 5..60 s backoff); fix with `account` and `restart`.
5. Survive reboots (cron, like the WhiteStick relay does):
   ```
   vps '(crontab -l 2>/dev/null; ~/tmserver/bin/tmserverctl cron) | crontab -'
   ```
   `tmserverctl start` is a no-op when the supervisor already runs, so the 2-minute watchdog
   line is safe.

## The server account (needs vjeux, once)

A TM2020 dedicated server only goes online with a **dedicated-server account** bound to a
Ubisoft account. Finding (2026-09-26, from openplanet-nl/nadeoapi-docs and the trackmania.com
page itself): it is created **only** on <https://www.trackmania.com/player/dedicated-servers>
after logging in with the Ubisoft account (Ubisoft Connect web login, 2FA if enabled) —
the page is server-rendered by trackmania.com and its form posts to trackmania.com, which
talks to Nadeo server-side; no public Nadeo API route creates one, and the Nadeo core/live
tokens we can mint for the player do not give access to that page. What the tokens CAN do:
`GET https://live-services.trackmania.nadeo.live/api/token/server/player-server/account`
(audience NadeoLiveServices, Ubisoft-user token) lists the accounts once created (login,
accountId, alreadyUsed, clubRoomId), and
`POST /api/token/club/{clubId}/room/create-from-server` attaches a server login to a club
room. So: no programmatic creation; listing/verification yes.

Steps for vjeux: trackmania.com → log in → Player → Dedicated servers → *Create a server
account* → pick a login (e.g. `vjeux_lowg`) → **copy the password immediately** (shown once,
cannot be retrieved) → send login + password to the engineer session (or run
`tmserverctl account <login> <password>` on the VPS and `tmserverctl restart`).

## Scaleway security group (vjeux's console)

Instance → Security groups → the group of `fooo` → Inbound rules, add (default policy for
inbound may be Drop):

| Action | Protocol | Port | Source    | Why                      |
|--------|----------|------|-----------|--------------------------|
| Accept | TCP      | 2350 | 0.0.0.0/0 | game connections         |
| Accept | UDP      | 2350 | 0.0.0.0/0 | game traffic             |
| Accept | TCP      | 3450 | 0.0.0.0/0 | P2P (legacy, optional)   |
| Accept | UDP      | 3450 | 0.0.0.0/0 | P2P (legacy, optional)   |

Keep 22 (SSH) and 8443 (WhiteStick relay). Do NOT open 5000 (XML-RPC, localhost only).
Outbound stays open (the server talks HTTPS to the Nadeo master servers). Debian 13 on
Scaleway has no local firewall by default; `ss -ltnup | grep 2350` on the box shows the
listener once the server runs.

## Operating it

```
tmserverctl status              pids, uptime, GetStatus, map, mode, players
tmserverctl logs -n 100 [-f]    the log (server output, supervisor, chat bridge)
tmserverctl restart | stop | start
tmserverctl check               layout, ports, account login, maps
tmserverctl rpc <Method> args   any XML-RPC call as SuperAdmin, e.g.
    rpc GetPlayerList 50 0
    rpc NextMap
    rpc RestartMap
    rpc LoadMatchSettings MatchSettings/lowg.txt
    rpc SetServerPassword s:secret          (s: forces a string)
    rpc ChatSendServerMessage 'hello all'
    rpc GetModeScriptSettings
tmserverctl lowg gravity all 0.3 | lowg bots 4 | lowg collisions off | lowg status
```

The supervisor restarts the server when it exits (5 s, doubling to 60 s on a crash loop,
reset after 10 min of uptime) and rotates `logs/server.log` at 20 MB (5 kept). To upgrade the
binary while it runs: `cp tmserverctl tmserver/bin/tmserverctl.new && mv -f tmserver/bin/tmserverctl.new tmserver/bin/tmserverctl`
(a plain `cp` over a running binary fails with "Text file busy"), then `restart`.

Config keys (`~/tmserver/tmserverctl.conf`): `server_dir`, `dedicated_cfg`, `game_settings`,
`title`, `lan`, `restart_delay_s`, `log_max_mb`, `log_keep`, `bridge`.

## Chat commands (the LowG mode)

Typed in the in-game chat; the bridge inside tmserverctl reads them over XML-RPC and forwards
them to the mode (a mode script has no chat access in TM2020).

| Command                   | Effect                                                   |
|---------------------------|----------------------------------------------------------|
| `/gravity 0.3` (or `/g`)  | your gravity, 0 = weightless … 1 = normal, applied live and on every respawn |
| `/gravity all 0.3`        | everyone's gravity and the default for newcomers         |
| `/gravity reset`          | back to the server default                               |
| `/gravity`                | show yours                                               |
| `/bots 4` (`/bots 4 fake`)| 4 balloons on the start line (engine bots; `fake` = named "Ballon N" fake users in the player list) |
| `/bots 0`                 | remove them                                              |
| `/collisions on|off`      | car-to-car collisions                                    |
| `/status`                 | current settings                                         |

Mode settings (matchsettings `<mode_script_settings>` or `rpc SetModeScriptSettings`):
`S_LowG_DefaultGravity` (0.5), `S_LowG_Collisions` (1), `S_LowG_Bots` (0, balloons at each map
start), `S_LowG_BotGravity` (0.05), `S_LowG_MaxBots` (16), `S_LowG_BotsAreFakeUsers` (0), plus
everything Time Attack has (`S_TimeLimit` 0 = no limit, `S_WarmUpNb`, `S_ChatTime`...).
Low-gravity times are kept off the normal leaderboards
(`Scores_AutoUploadPersonalBests = False`, mode name "LowG").

## Adding Yannex's maps

Copy `*.Map.Gbx` into `server/UserData/Maps/Yannex/`, add `<map><file>Yannex/Name.Map.Gbx</file></map>`
to `UserData/Maps/MatchSettings/lowg.txt`, then `tmserverctl rpc LoadMatchSettings MatchSettings/lowg.txt`
(or `restart`). `tmserverctl check` lists missing files. Any TM_Race map works; Time Attack
rules apply (no time limit by default, `rpc NextMap` or a vote to move on).

## Troubleshooting

- Mode script errors appear in the log as `ERROR [Modes/TrackMania/LowG.Script.txt : line, col] ...`
  followed by "Could not load the match settings" / "no Maps available": fix the script and
  `restart`. The same server binary compiles the script on a devserver in `/lan` mode (below),
  so test there first.
- "Server not started: no ServerName specified": `<name>` empty in dedicated_cfg.txt.
- The server keeps restarting: `tmserverctl logs -n 200`; the supervisor backs off to 60 s.
- Not listed in the server browser: account wrong/missing (log), or 2350 TCP/UDP closed in the
  security group, or `force_ip_address` wrong (`tmserverctl status` shows what is published).
- Memory: the server idles around a few hundred MB; 4 GB is plenty for 16 players.

## Local test harness (devserver, no account)

```
mkdir -p ~/tmserver-test && cd ~/tmserver-test && curl -sSfo s.zip http://files.v04.maniaplanet.com/server/TrackmaniaServer_Latest.zip && mkdir server && (cd server && unzip -q ../s.zip)
cp -r ~/trackmania-tas/tools/tmserverctl/deploy/UserData/* server/UserData/
printf 'lan = true\n' > tmserverctl.conf
~/trackmania-tas/tools/target/release/tmserverctl --home ~/tmserver-test passwords
~/trackmania-tas/tools/target/release/tmserverctl --home ~/tmserver-test start
~/trackmania-tas/tools/target/release/tmserverctl --home ~/tmserver-test rpc ConnectFakePlayer   # a fake car on the start line
~/trackmania-tas/tools/target/release/tmserverctl --home ~/tmserver-test lowg status              # gravity per car
```

`/lan` needs no master-server account; a real client cannot join across the internet, but the
mode compiles, maps load, fake players spawn and take the gravity, and every XML-RPC path is
the real one.

## How players find the server

TM2020 has no public browser for standalone dedicated servers (wiki.trackmania.io, "Dedicated
server / Setup"): a server is joined (a) by **join link** — the server log prints
`URL: trackmania://#join=<login>@Trackmania` once online; open that link from a browser (the game
registers the trackmania:// protocol) or paste `#join=<login>@Trackmania` into the game's
Main menu → Settings → System → Join link; (b) through a **club room**: the owner of the server
account, in his club, Administration → Club Activities (+) → Room → Use Dedicated Server →
pick the server login → Create; the room then shows under Play → Live → Clubs → that club (and
in the Arcade list when public). Only the Ubisoft account that owns the server account can bind
it to a club; the API route is `POST live-services .../api/token/club/{clubId}/room/create-from-server`.
(c) Play → Local → Local network only sees servers on the same LAN. Dedicated servers are PC only.

## Binding the server to vjeux's club (room) — the exact calls

Needs a **NadeoLiveServices** access token of vjeux's Ubisoft account (the WhiteStick box mints
one: `tinyctl nadeo-here` / the GhostShooter `/nadeotoken` route; valid ~1 h). From an agent-first
devserver the Nadeo hosts are reachable directly (`live-services.trackmania.nadeo.live` answers).
`$T` below is the raw token; every call carries `Authorization: nadeo_v1 t=$T` and a User-Agent.

```
UA='User-Agent: vjeux tmserverctl (Yannex low-g server)'
L=https://live-services.trackmania.nadeo.live
# 1. the club id (expected: 43788 "Vjeux")
curl -sS -H "Authorization: nadeo_v1 t=$T" -H "$UA" "$L/api/token/club/mine?length=20&offset=0" | jq '.clubList[] | {id, name, role}'
# 2. the server accounts of the Ubisoft account (login, accountId, alreadyUsed, clubRoomId)
curl -sS -H "Authorization: nadeo_v1 t=$T" -H "$UA" "$L/api/token/server/player-server/account" | jq .
# 3. the room, bound to the server login (name <= 20 chars; folderId 0 = club root)
curl -sS -X POST -H "Authorization: nadeo_v1 t=$T" -H "$UA" -H 'Content-Type: application/json' \
  -d '{"name":"Yannex low-g","login":"<SERVER_LOGIN>","folderId":0}' "$L/api/token/club/43788/room/create-from-server" | jq '{id, activityId, name, playerServerLogin, public, active}'
# 4. the club-room join link (starts the room if inactive; retry every 2 s while "starting")
curl -sS -X POST -H "Authorization: nadeo_v1 t=$T" -H "$UA" "$L/api/token/club/43788/room/<ACTIVITY_ID>/join" | jq .
# 5. check
curl -sS -H "Authorization: nadeo_v1 t=$T" -H "$UA" "$L/api/token/club/43788/room/<ACTIVITY_ID>" | jq '{name, public, active, room: .room.serverInfo, playerServerLogin}'
```

Errors to expect: `clubMemberRole:error-notContentCreator` (the token's account cannot create rooms in
that club), `serverLogin:error-inArray` (login is not one of the account's server logins). The
same login may be bound more than once without an error. Public/active flags are edited with
`POST /api/token/club/{clubId}/activity/{activityId}/edit` (`{"public":true,"active":true}`).

Players then find it under Play → Live → Clubs → Vjeux → « Yannex low-g », or through the
server's own link `trackmania://#join=<SERVER_LOGIN>@Trackmania` (`tmserverctl status` prints it).

## Pour Yannex — comment rejoindre (FR)

> Salut Yannex ! Le serveur **« Yannex — low-g »** tourne sur Trackmania 2020 (PC uniquement,
> les serveurs dédiés ne sont pas accessibles depuis les consoles).
>
> **Rejoindre** — il n'y a pas de liste publique des serveurs dédiés dans TM2020, on passe par un lien :
> 1. Clique sur `trackmania://#join=SERVEUR_LOGIN@Trackmania` — ou colle
>    `#join=SERVEUR_LOGIN@Trackmania` dans **Menu principal → Paramètres → Système → Lien de
>    connexion (Join link)** puis Entrée. (SERVEUR_LOGIN = le login du serveur, envoyé avec ce message.)
> 2. Quand la room de club sera créée : **Jouer → Live → Clubs → le club de vjeux → la room « Yannex low-g »**.
>
> **Une fois dedans**, tout se règle dans le **chat** :
> - `/gravity 0.3` → ta gravité (1 = normale, 0 = apesanteur). Appliquée tout de suite et à chaque
>   respawn. `/gravity all 0.3` pour tout le monde, `/gravity reset` pour revenir au réglage du
>   serveur (0.5 par défaut).
> - `/bots 4` → 4 voitures « ballons » sans pilote, quasi sans gravité, posées sur la ligne de
>   départ : fonce dedans pour les envoyer en l'air (`/bots 0` pour les retirer).
> - `/collisions off` / `/collisions on` → collisions entre voitures.
> - `/status` → les réglages du moment.
>
> Le mode est un Time Attack normal (chrono, respawn, classement), sans limite de temps : la map
> reste tant qu'on ne vote pas *Passer la map*. Tes maps s'ajoutent en les envoyant à vjeux ; la
> première en ligne est YannexDoor. Les temps faits en gravité réduite ne sont pas envoyés sur
> les classements officiels.
