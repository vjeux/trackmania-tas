# Where to develop: the WhiteStick box, not the devserver

**Do TM development in the WhiteStick box's checkout
(`/home/vjeux/trackmania-tas` on the render box, reached with `wsx`).
Not on devvm42752.**

## Why

The devserver **cannot reach GitHub at all**. fwdproxy blocks it for the agent
identity, over SSH and over HTTPS alike:

```
github.com has not been allowlisted in filter {"agent_id":"agent:agentcloud"}
```

So a clone there can commit, and nothing it commits is ever visible to anyone
else. That is not a theoretical hazard — on 2026-09-23 two clones were both
committing to `main`, diverged at `71301f22`, and it cost real time:

* The `tmdrive` game lock was built in the devserver clone; `playshots`,
  `render --fast` and the editor-load diagnosis were built in the box clone.
* `shootctl` was then built from the devserver tree and installed over the
  box's binary. The result had the lock but **no `playshots` and no
  `shootset`**, which broke another session's drive tests and A/B shots.
* Neither clone had pushed. GitHub was 20 commits ahead of both, with work
  from other checkouts that neither had seen.

The merge back together took about an hour and needed a hand-carried bundle,
because the devserver could not fetch.

## Do not ask for a fwdproxy exception for github.com

It will be refused, and it should be. `github.com` is the canonical example
the agent-egress framework uses for what must never be allowlisted — its own
config says so, in `agent_destinations.cinc`:

> "no repos, uploads, pastes or comments, so it is not the github.com class of
> exfiltration surface"

The bar is the **dead-drop test**: can an untrusted third party write data to
the domain and read it back? Gists and issues are exactly that. See
`https://www.internalfb.com/intern/staticdocs/ai-vault/docs/wut/agent-internet-access`.

## What to do instead

* **Work in the box checkout.** It reaches GitHub on its own network — fetch,
  push and all — and it is where every other session already works.
* **Need to move something from a devserver to the box?** `wsx push`, or a
  `git bundle` for commits:
  ```sh
  git bundle create /tmp/work.bundle <base>..HEAD
  wsx push /tmp/work.bundle /tmp/work.bundle
  wsx sh 'cd /home/vjeux/trackmania-tas && git fetch /tmp/work.bundle HEAD:refs/incoming/work'
  ```
* **`haul push-ref`** already pushes a ref to GitHub across the bridge, with
  rebase-on-race retry. Note its *fetch* still goes direct, so it works from
  the box, not from a devserver.

## Build and run on the box too

Related and separate: binaries are built and run on the box, not the laptop
(an antivirus there kills freshly built binaries). The box is the one machine
with the game, the checkout, and network access to GitHub — which is why
development lives there.

## History

The devserver clones were removed on 2026-09-24 for the reasons above:
`trackmania-tas`, `trackmania-tas-9`, `tt-colour`, `tt-v2`, and the two
orphaned worktrees `trackmania-tas-lights` and `trackmania-tas-pub4`.
Everything unsaved was banked to the box first — see
`/home/vjeux/rescued/` and `refs/rescued/ttv2/HEAD` there.

`/home/vjeux/trackmania-tas-tiny` on the devserver was NOT removed: it is a
**Sapling** checkout with unpushed draft commits and live uncommitted work.
It has the same invisibility problem; land or bank it before relying on it.
