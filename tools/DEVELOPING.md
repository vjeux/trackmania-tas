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
Pushing does not work from there — see the next section, which is the part
that decides the workflow.

## An agent-first devserver can CLONE but cannot PUSH

This is the important limit, and it is not a misconfiguration to fix.
Measured on 2026-09-24 from an OSS Builder box, all with the same valid
`repo`-scoped token:

| operation | result |
|---|---|
| `git clone` / `git fetch` (public repo, no auth) | works, 3.6 s |
| `GET .../info/refs?service=git-upload-pack` (read) | 200 |
| `GET .../info/refs?service=git-receive-pack` (push) | **401 "No anonymous write access"** |
| `gh api /user` (authenticated read) | 200 |
| `gh api -X PATCH /repos/...` (authenticated **write**) | **works** |
| `git push` with the token in the credential helper | 401 |
| `git push` with the token inline in the URL, no helper | **401** |

The last row is what makes it conclusive: with the credential helper taken
out of the picture entirely, the push still comes back anonymous. The
credential is fine — `gh` authenticates as `vjeux` and the API reports
`push: true, admin: true` on the repo. The egress proxy is stripping
authentication on the git push path specifically, while leaving reads and the
REST API alone.

That is a deliberate control rather than a gap: `git push` is the bulk
data-out channel, which is exactly the exfiltration vector the agent-egress
framework exists to close. Read it as the same policy that keeps github.com
off the ordinary allowlist, applied one layer in.

So **do not bother setting up `gh auth` for pushing** — it cannot buy that. It
is still worth having if you want `gh` for the REST API (issues, PRs, release
metadata, higher rate limits), and nothing about this repo's build or test
loop needs it.

**Push from the WhiteStick box**, which reaches GitHub on its own network with
the repo deploy key, and is the one machine that has always worked. To move
commits there from an agent-first box:

```sh
git bundle create /tmp/work.bundle <base>..HEAD      # on the agent-first box
# hand it to the box (see below), then:
wsx sh 'cd /home/vjeux/trackmania-tas && git fetch /tmp/work.bundle HEAD:refs/incoming/work'
```

`wsx` runs on the devserver, not on the agent-first box, so the bundle goes
devserver-ward first.

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
| **Push** | WhiteStick box — the proxy blocks push everywhere else |
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
