# The long-haul harness

*How this project keeps running for months on boxes that live at most eighteen
hours, with nobody tending it.*

The state of record is **this repository**. A box is a consumable: it is
registered, it works, it banks, and it is thrown away. Nothing important lives
only on a box, and no part of the system depends on an agent choosing to stay
awake.

---

## The one-minute version

```bash
# on any fresh internal box
export https_proxy=http://fwdproxy:8080 http_proxy=http://fwdproxy:8080
git clone https://github.com/vjeux/trackmania-tas.git /tmp/tmtas
cd /tmp/tmtas/tools && cargo build --release -p haul -p resim

cd /tmp/tmtas
./tools/target/release/tmhaul beat        # what is going on, and what to do
./tools/target/release/tmhaul recover     # take the run over from the repo
./tools/target/release/tmhaul watch --detach --lease-expires 2026-08-25T06:18:00Z
```

`tmhaul beat` reconstructs everything from committed files and prints a **DO
NOW** list. If you read nothing else, read its output.

---

## What each piece is, and which failure it exists for

Every one of these is a failure that actually happened on 2026-08-24, in the
project's first two hours of running as a swarm. None is hypothetical.

| the failure | what the harness does about it |
|---|---|
| A supervising **agent settled while its child ran**; an hour disappeared with nobody watching. | Liveness is `tmhaul watch`, a detached OS process. It survives the session that started it. A cron heartbeat wakes an agent every 30 minutes to check *from the repo* that the supervisor is alive — so even the watcher has a watcher, and neither is an agent's attention. |
| A search **ran three times logging 0 evals/s and its own stall detector never fired**, because it watched "furthest station not improving across a 2M-eval window" — which cannot fire when there are no evals. | `zero_throughput` and `no_progress` are **separate alarms with separate predicates**. Zero throughput treats *no samples at all* as zero, not as "no data yet". Every alarm has a test that fires it and a control that keeps it silent. |
| **Node leases cap at 18 hours.** No box survives a month. | Boxes are registered with their expiry. The supervisor banks and stands down `lease_bank_lead_s` **before** it, verifies every file by md5 against the commit, and retires the box. A queue claim held by a box that vanished expires and returns to `pending` by itself. |
| **`~/persistent/private-30d/` is 30-day retention** — not a months-long store. | The repo is the state of record, and every bank also mirrors the state tree off the box as a `HAULPACK` in a Phabricator paste. A fresh box plus a public clone plus an x509 cert is a complete recovery. |
| **A budget clock ran while the instrument was broken** — 8M evals / 10 hours burning during runs that did nothing. | The budget counts *work*. A sampling interval in which the eval counter did not move adds to `stalled_s`, never to `productive_s`. The switch threshold means what it says. |

---

## The layout

```
autopilot/
  STATUS.md            the page a human opens after a week away (generated)
  HARNESS.md           this file
  OPS-LOG.md           recurring failures and their fixes
  config/job.rec       what to run, how often, and every threshold
  state/
    journal/           what happened            (append-only, one file per box)
    ledger/            what was tried, and WHY  (append-only, one file per box)
    alarms/            what fired               (append-only, one file per box)
    budget/            work counters            (grow-only, one file per box)
    boxes/             the box registry         (append-only, one file per box)
    queue/{pending,claimed,done}/   one file per work item
    frontier/          best-known artifacts and results
    MANIFEST.md5       every state file's md5, rewritten on every bank
```

**Every log is append-only and sharded by writer.** Two boxes running at once
touch disjoint files, so a git merge is a directory union and never a conflict,
and the logical log is every shard in timestamp order. This is also what makes
recovery safe: the union of two logs is a log, so merging a mirror into a
checkout cannot lose a record or double one (`tmhaul recover`, idempotent).

---

## Durability, in three independent layers

A bank runs all three and returns a receipt naming each one's outcome. A layer
that silently did nothing is the failure shape this project keeps paying for.

1. **Commit** to the checkout. Cheap, always available. Survives a crashed
   process, and nothing else.
2. **Mirror** — the state tree as a `HAULPACK` in a Phabricator paste, titled
   `TMHAUL-STATE <node> <iso> sha=<sha>`. Needs only the x509 cert every
   internal box already has, so it works on an on-demand box with no GitHub
   credential at all. Discovered by title (`tmhaul mirror latest`), so a fresh
   box needs no id from anybody. Every file in the pack carries its own md5 and
   the manifest carries a digest: a truncated or corrupted pack is **refused**,
   never half-restored.
3. **Push** to GitHub — the state of record a human reads.

### The push route, because it is not obvious

**No on-demand box holds a GitHub credential.** `git push` from one dies with
`could not read Username for 'https://github.com'`. The working route is the
render box, which has a repo-scoped deploy key that has never left it:

```
OD ──git bundle──► wsx (md5-checked both ends) ──► WhiteStick ──deploy key──► github
```

`tmhaul` does this itself (`push = auto` resolves to `whitestick`), into its own
clone at `~/haul-push` on the render box rather than the shared
`~/trackmania-tas` checkout, which is usually in the middle of somebody's
render. It refuses to report success unless the sha the box pushed equals our
own HEAD.

The bridge needs `~/.navi/credentials.json` (161 bytes) on the OD; a fresh OD
has none. Copy it from devvm42752 — `RENDER-BOX.md` §2. Without it, `push = auto`
resolves to `none`, the mirror still runs, and `unbanked_drift` will tell you.

> **Never commit a credential.** This repo is public. `tmhaul bank` scans the
> state tree for private keys, tokens and `credentials.json` before every
> commit and refuses the whole bank if it finds one.

---

## Alarms

```bash
tmhaul alarms eval        # what is firing now, from the repo
tmhaul alarms selftest    # fire every alarm from its fixture, here, now
tmhaul alarms live-test   # fire alarms against real processes on this box
```

| alarm | fires when |
|---|---|
| `zero_throughput` | a run is active and the eval counter has not moved — **including when nothing has been reported at all** |
| `throughput_collapse` | still moving, but below a quarter of its own trailing baseline. Suppressed while `zero_throughput` fires: one event, one alarm |
| `no_progress` | healthy throughput, objective flat for 2M evals |
| `worker_died` | the run is marked active and there is no worker process |
| `box_vanished` | a registered, unretired box has been silent for 30 minutes |
| `queue_stalled` | pending work and nothing completing, or claims expired |
| `disk_filling` | below the floor, **or** on trend to reach zero within six hours |
| `unbanked_drift` | nothing banked off the box for 90 minutes |

`selftest` is the standing answer to *"has anyone ever seen this fire?"* — it
walks a table pairing each alarm with a state that must fire it, plus a healthy
control that must fire nothing, and exits non-zero if any of that is untrue.
`live-test` proves the plumbing between a live process, the progress file, the
journal and the evaluator, which is a different claim from the predicates being
right.

---

## The worker contract

A worker is any process. `job.rec` says how to start it. It must:

* append `<iso8601>\tprogress\tevals=<cumulative>\tbest=<objective>` to
  `$TMHAUL_PROGRESS`, as often as it likes;
* resume from `$TMHAUL_RESUME_EVALS` and `$TMHAUL_RESUME_BEST` when they are
  set. The supervisor computes those **from the repo**, not from the box, which
  is what makes a dead box cost one banking window instead of the whole run.

Today's worker is `tmresim`, the standing re-simulation sweep: every banked
result in the repo back through the plain oracle, forever, with a gate that
**refuses** a human's recording rather than skipping it silently. When agent C's
rung-1 explorer lands, changing `worker_cmd` in `job.rec` is the whole
integration.

---

## Rotating a box

The supervisor does the first half by itself. An agent — woken by the
heartbeat — does the half that needs the platform.

```bash
tmhaul stop            # bank, verify md5 against the commit, retire the box
tmhaul verify          # must be clean. Never release a box before it is
# provision the replacement, then on it:
tmhaul recover         # pull, merge the newest mirror, reap dead claims
tmhaul watch --detach --lease-expires <new expiry>
# only now release the old box
```

`tmhaul verify` re-hashes what **git has**, not what is on a disk that is about
to be destroyed. The working tree legitimately runs ahead of the manifest —
the journal gains a record the instant banking finishes — so verifying the
working tree would fail on a healthy run and teach everyone to ignore it.

---

## What a fresh box needs that the repo does not carry

Honest list, because a recovery that quietly needs something nobody wrote down
is not a recovery:

| | where it comes from |
|---|---|
| Rust | `rustup`, plus `~/.cargo/config.toml` with the proxy — `SETUP.md` §1. **cargo does not inherit the shell proxy** |
| the dedicated server (385 MB) | `SETUP.md` §3, one `curl` |
| the map corpus (`.Map.Gbx`) | `~/persistent/private-30d/tm-unbeaten/<id>/`. **This is a 30-day store and it is the one real gap in the recovery story** — see OPS-LOG |
| the bridge credential | `~/.navi/credentials.json` from devvm42752, for pushing |

---

## Commands

```
tmhaul init | status [--write] | beat | recover | bank [--why T] | verify
tmhaul watch [--detach] [--lease-expires ISO] [--note T] | stop
tmhaul journal add|tail   ledger add|list   queue push|claim|complete|list|reap
tmhaul budget show   lease   mirror latest|restore   config get KEY
tmhaul alarms eval|selftest|live-test
tmhaul selftest-worker --mode normal|stall|slow|flat|silent|crash
```

Tests: `cd tools && cargo test --release -p haul -p resim`.
