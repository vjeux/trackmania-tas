# The recovery demonstration, 2026-08-24

*The deliverable that proves the rest. A run was killed mid-flight with
`kill -9`, the box's copy of the project was deleted, and the run was picked
up again from the repository alone.*

This is a transcript of what actually happened, with the real timestamps. It
is reproducible: the commands are in `HARNESS.md` and nothing here was staged.

---

## Before

A supervisor and the re-simulation worker had been running for four minutes.

```
wall clock:   2026-08-24T18:46:09Z
HEAD:         9e5dfa8c125bd886c3509e78756a21f264ef64b3
origin/main:  9e5dfa8c125bd886c3509e78756a21f264ef64b3      ← identical
worker, last progress:  evals=114 best=100   (18:45:21Z)
repo, last banked sample: evals=80           (18:44:20Z)
processes: tmhaul watch (278248), tmresim (278251)
```

## The kill

`kill -9` on both, so there is **no clean shutdown, no `run_stop` record, no
final bank** — the state a box leaves behind when its lease is reclaimed
underneath it. Then the box's copy of the project was destroyed:

```
rm -rf /tmp/tmtas /tmp/tmhaul
  ls: cannot access '/tmp/tmtas': No such file or directory
  ls: cannot access '/tmp/tmhaul': No such file or directory
killed and wiped at 2026-08-24T18:46:24Z
```

Kept: `/tmp/tmoracle`, the 385 MB dedicated server — a documented one-`curl`
bootstrap step (`SETUP.md` §3), not project state.

## The control, before the alarm

A clean clone was built from the public repo and asked what was going on, **one
and a half minutes after the kill**:

```
cloned HEAD: 9e5dfa8c125bd886c3509e78756a21f264ef64b3
git clone: 3.8 s   ·   cargo build --release -p haul -p resim: 4.9 s

now: 2026-08-24T18:46:42Z
$ tmhaul alarms eval
nothing firing
exit=0
```

**This is the half that makes the next half mean something.** A stall detector
that fires the instant a run goes quiet is not a detector, and the interesting
question about any alarm is whether it can be silent.

## The detection, from the repo alone

Ten minutes later, on that same fresh checkout — which had never seen the run,
on no context, with no process to ask:

```
now: 2026-08-24T18:56:20Z
$ tmhaul alarms eval
Critical  zero_throughput  run active, no progress sample for 12m (last at 2026-08-24T18:44:20Z)
exit=3
```

and the briefing a woken session reads:

```
RUN
  Active. Last sample 12m ago: 80 evals, objective 66.
  Recent throughput: not measurable
  Supervisor on this box (117796): NOT RUNNING
  Last bank: 13m ago — commit 28c92363431b · mirror P2474171818 · push whitestick→github 28c92363431b

ALARMS
  [CRITICAL] zero_throughput — run active, no progress sample for 12m

DO NOW
  $ tmhaul watch --detach   # nothing is supervising this box; start it
  $ tail -50 .../worker.log # the worker is producing nothing; read what it is saying
  $ tmhaul status --write && tmhaul bank --why heartbeat
```

Note `Recent throughput: not measurable` rather than `0.00 evals/s`. A zero
that means "I could not measure this" is the exact shape of the bug this
project keeps paying for, and it is not printed anywhere.

## The recovery

```
$ tmhaul recover
recovering into /tmp/tmtas
  git: Already up to date.
  mirror: P2474174556 (TMHAUL-STATE 117796 2026-08-24T18:44:58Z sha=9e5dfa8c125b)
  17 files seen · 0 written · 0 merged (+0 records) · 17 identical
```

**"17 identical" is a cross-check worth naming.** The clone came down from
GitHub through a deploy key on a Windows machine; the mirror came from a
Phabricator paste through an x509 cert. Two transports with nothing in common
produced byte-identical state, each file verified against the md5 carried
inside the archive.

## The continuation

```
$ tmhaul watch --detach --lease-expires 2026-08-25T06:18:00Z
supervisor detached, pid 317652

18:56:50Z  worker_start  pid=317655  resume_evals=80  resume_best=66
18:56:50Z  run_start     node=117796  map=Summer 2026 - 01
18:57:48Z  bank          commit 43465f1c9ddf · mirror P2474188709 · push whitestick→github 43465f1c9ddf
18:58:48Z  sample        evals=160  best=66
18:59:48Z  sample        evals=189  best=95

$ tmhaul alarms eval
nothing firing
```

`resume_evals=80` is the whole point: the replacement worker was told where to
start **by the repository**, not by the box — the box's progress file had been
deleted ten minutes earlier.

---

## What the crash actually cost, stated honestly

| | |
|---|---|
| work the worker had done | 114 evals |
| work the repo knew about | 80 evals |
| **lost** | **34 evals — about 90 seconds** |

The bound is the banking cadence, and it is worth being precise about what
"lost" means here. Two different things are at risk and they are protected
differently:

* **the record of progress** — journalled every `sample_s` (60 s) locally,
  committed and pushed every `bank_s` (30 min). A crash costs at most one
  banking window of *record*;
* **artifacts** — a tape, an archive, a frontier. These are lost unless the
  worker writes them into `autopilot/state/frontier/`, where the next bank
  picks them up. **A worker that keeps its best tape in `/tmp` will lose it,
  and the harness cannot save it.** For the re-simulation sweep this does not
  matter, because a re-simulation is cheap to redo. For the rung-1 explorer it
  will matter enormously, and it is the first thing to check when that worker
  goes under the harness.
