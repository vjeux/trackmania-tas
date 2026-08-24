# Operations log

Recurring failures and their fixes, so a future session does not rediscover
them. Newest at the top. Each entry: **what broke**, **how it presented**,
**the fix**. If it cost more than ten minutes, it belongs here.

---

## A restarted supervisor counted its resume point as fresh work

Twenty minutes after the first restart the budget read **582 evals for 308
actually done**. The supervisor's eval baseline started at zero, so the first
sample after every restart recorded the whole cumulative counter as a delta.
On a system whose *normal* mode of operation is "box dies, replacement
resumes", that inflates the one number the pre-committed switch condition
turns on — and it inflates it in the direction of switching too early, for a
reason nobody would find months later.

The baseline is now seeded from the repo before the first sample. The
miscount itself was corrected the only way an append-only log allows:
`tmhaul budget correct --evals 274 --why …`, which records the subtraction and
its reason as a new line rather than editing history.

## Starting a second supervisor deletes the STOP flag the first was reading

`tmhaul watch` clears the stop file at startup, so starting a new supervisor
while the old one is standing down — which takes up to a minute, because a
bank includes a push — removes the flag before the old one reads it. Both then
run. `watch` now refuses to start when another supervisor is alive on the box
(`--force` overrides), which is the right answer regardless: two supervisors
on one box is not a configuration anybody wants.

## The repo has other authors, and a push that fails is silent about the *cause*

Within an hour of the harness going live, an unrelated session pushed
`entorder can put the car in the MIDDLE of the entity list` to `main`. Every
subsequent bank recorded `PUSH FAILED … fetch first`. Nothing was lost — the
paste mirror uses a different credential and a different service and kept
working — but the repo, which is the state of record a human reads, went stale
while the journal filled with rejections.

A bank now fetches, rebases when the branch has diverged, and retries. The
state files are append-only and sharded per writer, so a rebase is
conflict-free by construction; it aborts and says so if that ever stops being
true.

**And the second-order trap, which cost the longer half of the time:** after a
rebase our commits have new shas, so the render box's scratch ref
`tmhaul-incoming` can no longer be fast-forwarded either. The fetch failed
with `exited 1` and *no stderr at all*, because it was running under `-q`.
Two lessons: force the scratch ref (`+main:refs/heads/tmhaul-incoming`) but
**never** force `main`, so a real divergence still fails loudly; and do not
run the remote half of a pipeline quietly, because the only thing you have when
it breaks is what it printed.

## No on-demand box holds a GitHub credential — and the fix is a 161-byte file

`git push` from an OD dies with `could not read Username for
'https://github.com'`. There is no `gh`, no `~/.git-credentials`, no deploy key.
It reads like "this box cannot push", which is a claim about the world and is
false: the render box has a repo-scoped deploy key, and the bridge to it needs
exactly one file the OD is missing — `~/.navi/credentials.json`, 161 bytes, on
devvm42752 (`RENDER-BOX.md` §2). Copy it and `~/bin/whitestick 'echo hello'`
answers immediately; devvm is not a hop, only the source of the file.

**The general shape, which is worth more than the instance: a credential that
lives on machine A is not a property of machine A. Ask what the thing actually
needs and whether that is a FILE.** `tmhaul bank` does the rest — bundle, `wsx`
push (md5 both ends), fetch and push on the far side, and it refuses to report
success unless the sha the box pushed equals our HEAD.

## `persistent/private-90d` is a 30-day store despite its name

The TTL is 30 days **from last modification**, on both aliases. Anything meant
to outlive a month either lives in the repo or gets
`persistent-storage mark --no-user-data`. Do not plan a months-long project
around either mount.

## Verify the bytes git HAS, not the bytes on the disk you are about to destroy

The first `tmhaul` stand-down reported `VERIFY FAILED` on a healthy run every
single time. Cause: the manifest is written, then the state is committed, and
then the journal gains the `bank` record recording that it happened — so the
working tree is *legitimately* one record ahead of the manifest by the time
anything checks. A verifier that fails on a healthy run gets ignored within a
day. `tmhaul verify` now hashes `git show HEAD:<path>`, which is also the thing
"banked" actually means, since a release destroys the working tree.

## The map corpus is the one thing the repo does not carry

`.Map.Gbx` files live in `~/persistent/private-30d/tm-unbeaten/<id>/` — a
30-day store. Everything else a fresh box needs is either in the repo or a
documented one-liner (`SETUP.md`). This is a real gap in the recovery story and
it is stated rather than papered over: a box provisioned after that store
expires can build and supervise, but the re-simulation sweep will report
`UNMEASURED: no .Map.Gbx for <id>` for every map rather than silently passing.

## `INFINITY` through a comparison makes every test false

A comparison against `INFINITY` is false in both directions, so a filter built
on one accepts nothing and a "best so far" seeded with one is never beaten —
and the code returns whatever came first, with no error anywhere. Seed with an
`Option` and let the type system make the empty case explicit.

## A constant steer channel makes the shim lock onto the wrong memory

The shim finds the input array by looking for the bytes it expects to be
changing. Drive with a steer value that never changes and it locks onto some
other region that happens to match, then reports confidently about it.
Vary the channel, or pin the array by identity rather than by content.

## The validator simulates until the *declared* time, not until the tape ends

A tape longer than its declared result is truncated silently; a tape shorter
than it runs past its own end. Either way the verdict is about a run nobody
asked for. Always set the declared time from the tape you are actually
submitting.

## cargo does not inherit the shell proxy

`https_proxy` in the environment gets the crates.io *index* nowhere: cargo
retries three times and dies on a random crate, which reads as a flaky network.
Write `~/.cargo/config.toml` with `[http] proxy` and `[net] git-fetch-with-cli`.
`SETUP.md` has the exact file.

## Clone into /tmp, never into ~/persistent

A clone into the persistent mount dies with `premature end of pack file` at a
different byte count every time. Three agents have tuned postBuffer, HTTP/2 and
`--filter` chasing it. Work in `/tmp`; bank to persistent and to the repo.
