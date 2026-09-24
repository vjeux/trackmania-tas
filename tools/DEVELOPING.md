# Where to develop

**Use an AGENT-FIRST devserver for anything that needs GitHub. Use the
WhiteStick box for anything that needs the GAME.**

Both statements matter, and they are about different things: the agent-first
devserver solves network access, the WhiteStick box is where Trackmania and
Openplanet actually run.

## GitHub: agent-first devservers can, ordinary ones cannot

An ordinary devserver or OnDemand box **cannot reach GitHub**. fwdproxy denies
it for the agent identity, over SSH and HTTPS alike:

```
github.com has not been allowlisted in filter {"agent_id":"agent:agentcloud"}
```

Tested 2026-09-24 on two freshly-leased ODs: 12 attempts, 12 denials. The
filter keys on the agent identity, not the host, so a fresh OD is not an
escape. fwdproxy itself works there — pypi.org returns 200 — but
github, gitlab, bitbucket, crates.io and docs.rs are all denied. (On a plain
OD, also note that nothing has internet at all until you set
`http_proxy`/`https_proxy=http://fwdproxy:8080` and
`no_proxy=.facebook.com,...`; there is no direct egress and public DNS does
not resolve.)

An **agent-first devserver** (the "OSS Builder" role) is a different
environment with full internet by design. Provision one with:

```
node__provision  envspec="agentic-ai-short-term-lease"
```

To see the other pools: `devfeature install devenv_admin` then
`devenv-admin envspec list` (there are also product-builder and data-analyst
variants; the data-analyst one is deliberately air-gapped from the internet).

Verified on 2026-09-24, on a box provisioned exactly that way:

```
git clone https://github.com/vjeux/trackmania-tas.git   # 3.6 s, no credentials, no proxy
identity: AGENT:dev.oss  agent.role=open_source_developer  env.type=agent_devserver
gh CLI preinstalled (2.86.0)
```

Two things worth knowing about it:

* **Egress is a transparent MITM proxy, not raw internet.** Public hosts
  resolve into `2001:db8:face:b00c::/64` — the documentation range — because
  traffic is intercepted and forwarded. It works; it is just not a direct
  route, so don't debug it as one.
* **The trade is production access.** No prod data, no lateral SSH, and
  internal Thrift services are allowlisted per asset. A denial looks like
  `ABAC_AGENT_ROLE_DENIED`, and sometimes like an empty result rather than an
  error; the self-service unblock tool is at
  `agent-first-unblock-me.internalmeta.com`.

This repo is **public**, so cloning and fetching need no credentials at all.
Pushing works too, with one trap that looks like an auth failure — next
section.

## Pushing from an agent-first devserver: works, with one trap

**Pushes work from an agent-first devserver** once the sandbox is disabled
(vjeux did that on 2026-09-24; verified by pushing and deleting a scratch
branch). Before that change, every authenticated write came back 401 — an
earlier version of this note called that deliberate policy. It was not; see
the cache trap below, which is the part that will bite you.

### The trap: fwdproxy caches responses per URL, ignoring `Authorization`

The A1D's egress is fwdproxy2 (the `via:` header names it, on `:8082`), and
it caches HTTP responses **keyed on the URL alone**. Git's push protocol
starts with an ANONYMOUS probe of
`.../info/refs?service=git-receive-pack`, which GitHub answers with 401
"No anonymous write access". The proxy caches that 401. Every later request
for the same URL — including the authenticated retry git makes a moment
later — gets the cached 401 back. Same `x-github-request-id`, for 15+
minutes and counting.

That produces exactly the symptoms that look like "auth is stripped":

* `git push` fails with "Authentication failed" while the token is valid
* `gh api` writes succeed (different URL, never poisoned)
* `curl -u` to the same URL ALSO fails once the cache is poisoned
* a cache-buster query string gets a fresh (404) response; `Cache-Control:
  no-cache` does not help

### The fix: authenticate on git's FIRST request

Send the credential preemptively, so the first request for the URL is
authenticated and what gets cached is the 200:

```sh
AUTH=$(printf 'vjeux:%s' "$(gh auth token)" | base64 -w0)
git -c http.extraHeader="Authorization: Basic $AUTH" push origin HEAD:refs/heads/<branch>
```

Inline URL credentials (`https://user:token@…`) do NOT work — git still
probes anonymously first. The credential helper does not either, for the same
reason. Only `http.extraHeader` puts auth on the first request.

If the canonical URL is already poisoned, use a differently-cased repo path as
a fresh cache key — GitHub treats repo names case-insensitively and answers
with a "repository moved" hint, harmlessly:

```sh
git -c http.extraHeader="Authorization: Basic $AUTH" \
    push https://github.com/vjeux/TRACKMANIA-tas.git HEAD:refs/heads/<branch>
```

### Always push with an EXPLICIT refspec

`git push <url> <branchname>` with a bare branch name resolved against
`main`'s upstream and landed a scratch commit on `main` (2026-09-24,
reverted within 20 s). Write `HEAD:refs/heads/<branch>` every time.

### Getting a token there

`gh auth login --hostname github.com --web` runs the device flow: it prints a
one-time code, vjeux approves it in a browser, and GitHub mints the token
straight onto the box — nothing passes through the agent. The token does NOT
persist to the next box: agent-first devservers sync dotfiles against their
own `agent_first_devserver` dotsync universe, deliberately isolated from
regular devservers, and `.config/gh` is not in that set. Adding it there is
possible and would make every future A1D log in automatically; it would also
park a `repo`-scoped token in the snapshot. That is vjeux's call, not made.

Clone and fetch need no token at all — the repo is public.

## The game: only the WhiteStick box

Trackmania, Openplanet, the plugins, the lock and the maps live on the
WhiteStick box and nowhere else. No devserver can load a map, hook physics or
verify a jump, so any task that touches the game runs there, through `wsx`,
under the `tmdrive` lock (one game, one driver).

The box also reaches GitHub on its own network, with a repo deploy key
(`~/.ssh/config.tmtas` defines the `github-tmtas` host, key
`~/.ssh/id_ed25519_tmtas`). A note for whoever debugs that key next:

```
$ ssh -T github-tmtas
Hi vjeux/trackmania-tas! You've successfully authenticated,
but GitHub does not provide shell access.        # rc=1
```

That is **success** for a deploy key, not a failure. A deploy key is
repo-scoped and has no shell; `git ls-remote origin` returning a SHA is the
check that means something.

## The split, in practice

| work | where |
|---|---|
| Clone, fetch, edit, build, test | agent-first devserver (no credentials needed — the repo is public) |
| **Push** | agent-first devserver with preemptive auth (see the trap above), or the WhiteStick box |
| Anything that runs the game — plugins, maps, renders, the jump | WhiteStick box, via `wsx`, under `tmdrive` |
| Move build artifacts or commits to the box | `wsx push`, or `git bundle` |

Do **not** keep a long-lived clone on an ordinary devserver. It can commit,
and nothing it commits is ever visible to anyone — that is not a hypothetical:
on 2026-09-23 two clones were both committing to `main`, diverged at
`71301f22`, and a `shootctl` built from the invisible one was installed over
the box's copy and broke another session's tooling. Neither clone had pushed;
GitHub was 20 commits ahead of both. Merging them back needed a hand-carried
bundle, because that devserver could not fetch. Those clones were deleted on
2026-09-24.

## Do not ask for a fwdproxy exception for github.com

It will be refused, and it should be. `github.com` is the canonical example
the agent-egress framework uses for what must never be allowlisted — its own
config says so, in `agent_destinations.cinc`:

> "no repos, uploads, pastes or comments, so it is not the github.com class of
> exfiltration surface"

The bar is the **dead-drop test**: can an untrusted third party write data to
the domain and read it back? Gists and issues are exactly that. The
agent-first devserver is the sanctioned answer to the same need — it gets full
internet by giving up production access, rather than by widening an allowlist.
See `https://www.internalfb.com/intern/staticdocs/ai-vault/docs/wut/agent-internet-access`.
