# Operations log

Recurring failures and their fixes, so a future session does not rediscover
them. Newest at the top. Each entry: **what broke**, **how it presented**,
**the fix**. If it cost more than ten minutes, it belongs here.

---
## Narrowing a race is not closing it — pin the thing, do not re-read it

The bundle-integrity check has now cried wolf twice on perfectly good pushes.

* **First:** it compared the bundle against `HEAD` re-read *after* the push. A
  concurrent supervisor bank moved `HEAD` in between.
* **Then I "fixed" it** by reading the branch immediately after building the
  bundle. Smaller window, same race — and it fired again: the bundle carried
  the heartbeat's commit `ccb64ee`, the supervisor committed `07eaef7` eight
  seconds later, and the rev-parse returned the wrong one. A transfer that
  delivered exactly what it was handed was reported as a failed transfer.

**Narrowing the window was the wrong instinct both times.** Two reads of a
moving target are a race however close together they are. The sha is now
resolved once, written to `refs/tmhaul/send-<stamp>`, and *that ref* is what
gets bundled and fetched — the bundle cannot carry anything else, whatever the
branch does next. The ref is deleted immediately, so a crashed push leaves no
litter.

The general form: **when a check compares "what I sent" with "what arrived",
the sent side must be pinned, not looked up again.** And a check that fires on
healthy operation is worse than no check — it is the second time this one has
sent me hunting a transport bug that did not exist.

## One `/proc` scan, two opposite questions, both wrong in turn

`watch_pid()` skipped its own pid. That is right for `beat`, which asks
whether *somebody else* is supervising, and wrong for the supervisor, which
evaluates alarms about *itself* every pass: excluding itself, a perfectly
healthy supervisor journalled `supervisor_died` on its first pass on every new
box.

Fixing it by counting itself immediately broke the other caller. The
`watch --detach` guard is also a `tmhaul … watch …` process, so it found
itself and refused every start with *"a supervisor is already running"* —
naming its own pid.

Two live defects, twenty minutes apart, from one function answering a question
its callers meant differently. `watch_pid_excluding(skip)` makes each caller
say which it means: the alarm counts itself, the guard does not.

**When a predicate has two callers, check whether they mean the same thing by
it.** "Is a supervisor running here?" is not one question.

## `.unwrap_or_default()` on a discovery path is the same bug as `.ok()?`

The credential server's mirror discovery — the thing that lets it find a box
that cannot push its own registration yet — was wired as
`from_mirrors(...).unwrap_or_default()`. A failed lookup and "nobody announced
themselves" produced the identical empty list, so a server whose `meta` call
had started failing kept looking healthy while a fresh box sat DEGRADED.

Now it says so on stderr and falls back to the repo registry explicitly. The
project's own rule, in a new costume: **an absent answer is not a negative
answer**, and swallowing the difference is how an instrument fails toward
clean.

## Splitting the budget KEY was half a fix: the sweep still measured itself against somebody else's threshold

Yesterday the re-simulation sweep was found spending the archive search's
pre-committed budget, and each job got its own `budget_key`. That stopped the
sweep *consuming* the search's allowance. It did not stop the sweep being
*measured against the search's number* — and tonight it reached **98.3% of the
switch threshold**, hours from announcing "the pre-committed switch is due"
for a workload the switch was never about.

The 8M-eval / 10-productive-hour condition is a decision: reach it without CP1
and a learned ordering over archive bins gets added. That decision is
meaningless for a re-simulation sweep. A countdown to it is worse than
meaningless — it is a prompt to act, aimed at nobody.

`budget_has_switch` (default `yes`) lets a budget say it is a **meter, not a
countdown**. The numbers are still reported; the percentage and the "switch
reached" line are not, because they would be a fraction of a threshold this
job does not have.

**The general shape, and it is the third variation on it this week: scoping WHO
spends a budget is not the same as scoping WHAT the budget means.** Ask not
only "is this counter measuring the right work?" but "is the number it is
compared against a decision anyone could act on for this job?"

## The supervisor died and the alarm surface said nothing was wrong

A supervisor vanished on a healthy box — no `run_stop`, nothing in its own log,
no OOM, no reboot, the worker gone with it. `tmhaul beat` printed
`Supervisor on this box: NOT RUNNING`, because it reads `/proc`. On the same
data, `tmhaul alarms eval` printed **nothing firing**, and would have kept
printing it until `zero_throughput`'s ten-minute window closed.

Ten minutes of delay was not the defect. The defect is that **the harness knew
and the alarm surface did not say so** — the gap between a check existing and
the check being wired to the thing people read.

`supervisor_died` now covers it, and only on the box that owns the run: a
machine merely reading the repo reports `supervisor_here: None` and stays
silent, because "no supervisor on a box that was never running one" is not a
fault.

**And `pgrep -f "release/tmhaul watch"` matches the shell command containing
that string.** Every ad-hoc "is it alive?" check I ran that way could answer
yes with nothing running. The alarm parses `/proc/<pid>/cmdline` and requires
argv[0] to end in `tmhaul`, so it was right when my shell check was wrong —
which is exactly the direction you want that disagreement to run. For a
one-off check: `ls -la /proc/*/exe | grep release/tmhaul`.

## `grep` on a release binary is not a test of what the binary does

Chasing whether a rebuilt `tmhaul` contained a change, I used
`grep -ac <literal> target/release/tmhaul` with `zero_throughput` as a control:
the control found 4, the string in question found **0**. I concluded the binary
was stale and restarted the supervisor twice on that basis.

It was wrong. The behaviour test — change `bank_s` under the running supervisor
and watch for the `config_reloaded` journal record — shows the code is present
and running, while the same `grep` still reports 0.

**The control was not equivalent to the thing it was vouching for**, which is
the whole trap: a literal that appears four times near newlines is not evidence
about one that appears once. A binary's string table is not its behaviour.

**Test what the thing DOES.** For the supervisor that means: make the change,
wait one pass, read the journal. `run_start` already records `binary_built`,
and that timestamp — checked against when you built — is the cheap, honest
version of the question I was trying to answer.

## A supervisor that snapshots its config makes every committed change a lie

I tightened `bank_s` from 30m to 10m, banked it, and reported it done. The
running supervisor kept banking every 30 minutes: it had read `job.rec` once at
startup, and the edit landed forty seconds later.

Same shape as the stale binary the day before — **the thing running is not the
thing you edited** — and worse here, because on this project the thing editing
`job.rec` is a woken heartbeat agent with no reason to suspect otherwise.

The supervisor now re-reads the spec every pass. A spec that no longer parses
leaves the running config in place (refusing to *start* on a bad config is
right; abandoning a healthy run over one is not), and a changed `worker_cmd`
is explicitly NOT reloadable — that needs a stand-down, and it says so rather
than half-applying.


## On-demand boxes die several times more often than their leases predict

Two boxes were lost mid-lease inside ninety minutes on 2026-08-25:

| box | alive for | lease left when it went |
|---|---|---|
| `24576` | 7h 24m | 4h 33m |
| `36944` | 4h 10m | 7h 49m |

Both simply stopped answering — the orchestrator lost them, and `24576` also
refused ssh from devvm with `Permission denied (publickey)` rather than timing
out, so **unreachability does not always present as a hang**.

Nothing was lost either time: both boxes' last mirrors were already on
`origin/main`, and `recover` found every file identical across the two
transports. But the operating assumption changes. **The lease is not the
planning horizon; the BANK CADENCE is**, because that is what bounds the loss
when a box goes without warning. `bank_s` tightened 30m → 10m accordingly.

The rotation itself is now routine — provision, `recover`, retire the dead box,
restart the supervisor, and the credential server heals the new box on its
own — so this costs a few minutes of throughput rather than a day.


## A lock directory with a trap does not survive `kill -9`, and then wedges forever

The credential server's launcher took a lock with `mkdir` and released it in
an `EXIT` trap. The trap does not run on SIGKILL or a reboot, so the directory
outlived the process — and every cron re-run after that exited immediately,
believing a server was up. The self-healing restart was therefore a one-shot:
it worked until the first hard kill, which is exactly when it was needed.

The lock now records its PID and a stale one is reclaimed
(`kill -0` on the recorded pid). Proved by `kill -9`ing the server, confirming
the lock survived, and watching the launcher take it over and restart.

**Anything that heals itself needs its healing path exercised from the failure
it is meant to heal — not from a clean state.**

## A function that reads the machine cannot be tested

Adding the credential check, I had `reconstruct` — the function that promises a
picture built from committed state and NOTHING else — stat a file on the local
box. Two unit tests then passed or failed depending on whether the box running
them happened to hold a credential. They passed on the box where I wrote them
and failed on the next one.

That function is the foundation every alarm reads, so a result that depends on
WHERE IT RUNS is disqualifying. The credential is now passed in by the callers
that legitimately know about this machine (`status::render_here`,
`state::with_credential`), and the pure ones stay pure.

## A fresh box cannot announce itself through the channel it needs the credential FOR

The credential server reads the box registry out of the repo. A fresh box
writes its registration into that registry — and then cannot PUSH it, because
pushing is exactly what it has no credential for. So the server never learned
the box existed, never delivered, and the box sat DEGRADED with an alarm that
said only that it was degraded. Perfect chicken-and-egg, and the automated
bootstrap looked like it simply did not work.

The fix is to listen on the channel that still works: the **mirror pastes**.
Writing one needs an x509 cert, which every box has from its first minute, and
the title is `TMHAUL-STATE <node> <iso> sha=<sha>`. So a box announces itself
durably before it can push anything, and `credential serve` now unions the
repo registry with the nodes named by recent mirror titles.

**The general shape: a bootstrap must not depend on the capability it is
bootstrapping.** Worth checking that sentence against anything else that heals
itself here.

## `cargo test` does not rebuild `target/release/<bin>`

I fixed the budget attribution, edited `beat.rs` and `status.rs`, ran
`cargo test --release -p haul` — 134 green — and then watched `tmhaul beat`
keep printing the OLD number. The library the tests exercise and the binary on
disk are different artifacts: `cargo test` builds
`target/release/deps/tmhaul-<hash>`, not `target/release/tmhaul`.

**A green test suite is not evidence that the thing you are running contains
the fix.** Run `cargo build --release -p <pkg>` before believing any CLI
output after an edit. `run_start` now records the binary's build time so a
later reader can tell which binary produced a run, rather than wondering why
the journal disagrees with the source.

## A budget that counts the WRONG WORK is the same defect as one that counts a stall

The harness was built so that a stall could not spend the pre-committed switch
budget. It then spent five hours of that budget on the wrong workload, and
nobody noticed until it read **50.8%**.

The condition — 8M evals or 10 productive hours, after which a learned
ordering over archive bins gets added — was agreed for the **archive search**.
The worker actually running is the re-simulation sweep. At the sweep's rate
the eval arm would take a decade, so the TIME arm was going to fire first, in
about five more hours, and the harness would have announced "the pre-committed
switch is due" for a workload the condition was never about. The decision it
triggers would have been made for a reason nobody could reconstruct in three
months.

Every interval now carries the `budget_key` its job declares, and totals are
per budget. An interval written before the key existed reads as
`unattributed` — never folded into the search's, because absorbing legacy
rows into whichever budget asks first would re-create the bug silently at the
next upgrade. `tmhaul budget` names which budget it is reporting and prints
the cross-budget total underneath, so the difference is visible rather than
missing.

**The general shape: "counts work, not time" is not enough. It has to count
THE work the threshold was agreed about.**

## Compare the sha you SENT, not HEAD as it is now

A push failed with *"the box pushed 3842b5b9 but our HEAD is 69ee9f62 — the
bundle did not carry what we think it did"*. The bundle was perfect. The
supervisor had banked concurrently and moved HEAD between the bundle being
built and the check reading HEAD again. The claim worth checking is that the
far side pushed **what we sent**, and on a live box HEAD will not stand still
while you work — so the sha is captured at bundle-creation time now.

## Dropping a crate from the workspace members list deletes its CI

Another session's commit rewrote `tools/Cargo.toml` `members` to add its own
crate and, in doing so, removed `haul` and `resim`. Two things then happened
quietly:

* the documented bootstrap (`cargo build --release -p haul -p resim`, the
  first thing every heartbeat runs on a fresh box) **fails on a clean clone**;
* the same commit reverted this harness's additions to `tmmaps`
  (`MapHeader::authortime`, `MapFile::try_load`) — and because the crates that
  use them were no longer in the workspace, **nothing built them to notice**.

The members list is not a preference, it is the test surface. A crate outside
it is a crate nobody compiles, and a revert of its dependencies is invisible.
When editing that line, ADD to it; never retype it.

(The `tmmaps` additions were restored from the original commit with
`git show <sha> -- <paths> | git apply --3way`, which is the cheapest correct
move: it takes the exact bytes rather than reconstructing them from memory.)

## An on-demand box cannot ssh to a devserver; the reverse works

Measured while automating the credential bootstrap: `ssh devvm42752…` from an
OD times out at the connect, and `ssh <od>.od.fbinfra.net` from devvm succeeds
first try. So anything that moves a file between them is a **push from the
devserver**, never a pull by the OD — which is why `tmhaul credential serve`
runs on the devserver and is aimed by the box registry rather than being asked
for by the box that needs the file.

## A test that mutates `$HOME` breaks every test running beside it

`resolve_push` read `$HOME`, so its test set it to a nonexistent path — and
`set_var` is process-global, so unrelated tests in the same binary started
failing with a story about missing credentials. **A function that reads a
global is a function whose test can only be written by mutating one.** It takes
the home directory as an argument now (`resolve_push_in`).

## A retirement stamped before the start it follows is history, not a retirement

`lease::all` folds each box's log in TIMESTAMP order and `box_start` clears the
retired flag — correctly, since a box that starts again is active again. So a
caller whose clock disagrees with the records it writes can retire a box "in
the past" and watch the retirement be ignored. `retire_at` exists for callers
that must control the stamp, and the behaviour is pinned by a test rather than
left to be rediscovered.

## Free disk is a property of a MACHINE; the run spans machines

Minutes after the first rotation, `disk_filling` fired CRITICAL: *"380543 MB
free, falling 7740.4 MB/min — empty in 49m"*. Nothing was filling. The old box
had 1.23 TB free and its replacement has 380 GB, and the alarm had computed a
slope across the two — it measured the rotation.

Every other alarm is about the RUN, which legitimately spans boxes; disk is
not. Samples now carry their writing node and the disk trend only compares
within one. A second arm was needed too: a box's first minutes always fall
steeply (385 MB of server download, then a release build), so a trend now
needs a real window — projecting six hours off two minutes of bootstrap is
arithmetic, not evidence.

**A false critical on a routine event is how an alarm gets ignored**, so this
rated a fix rather than a tolerance bump. The control is in the same test
file: after both suppressions, a genuine slope on one box still fires.

## A box that VANISHES never retires itself, and nothing could retire it

Found by the first unplanned rotation: `117796`'s lease was reclaimed with
nine hours nominally left, so the supervisor never reached its stand-down and
the registry kept the box `ACTIVE` forever — firing `box_vanished` on every
heartbeat and counting against the fleet ceiling. There was no way to say "that
box is gone" short of editing a state file by hand.

`tmhaul lease retire --node N --why T`. It refuses a name the registry has
never seen, because a typo would otherwise write a retirement for a box that
does not exist and quietly leave the real one active.

## A fresh box needs the bridge credential every time

Each replacement box starts with no `~/.navi/credentials.json`, so `push =
auto` resolves to `none` until it is copied from devvm42752 (161 bytes,
`RENDER-BOX.md` §2). The mirror still works without it, so the failure is not
loud — `unbanked_drift` is what would eventually say so. **This is the one
manual step in an otherwise unattended rotation**, and it is a file, so it
could be automated by any box that can read devvm.

## A DETACHED supervisor has no proxy, and a silent fetch failure hid it

The worst bug in the harness so far, and it is this project's signature shape:
**a check that passes while doing nothing.**

`sync_with_remote` treated a failed `git fetch` as `Ok(None)` — "no network,
not fatal, the push will complain" — so on a box whose environment has no
proxy the rebase never ran at all, the push was rejected, and the retry loop
re-ran the same silent no-op three times before reporting *"the remote kept
moving"*. It had not moved once.

Two fixes, and the second is the general one:

* **A fetch that failed means the remote is UNKNOWN**, and pushing on unknown
  is exactly what this project forbids. It is now an error naming the proxy.
* **The harness supplies the proxy itself** for every network git call
  (`gitcmd::git_env`), rather than requiring whoever launches it to remember.
  `tmhaul watch --detach` inherits NOTHING from the shell that started it, so
  "export it first" is a rule that works interactively and fails every night
  at 3am. An already-set value still wins.

Proved by running a full bank under `env -u https_proxy -u http_proxy`: commit,
mirror and push all succeed.

## Rebase with `--autostash` when a supervisor is running

The same bank then failed with *"cannot rebase: You have unstaged changes"*.
Not a conflict: the worker appends to the journal continuously, so between the
commit a moment earlier and the rebase, the working tree had moved again.
`--autostash` is built for exactly this. Without it the harness reports a
conflict that is not one, which is worse than the failure.

## Rebasing before a push is not enough: the remote moves DURING the push

After the rebase fix, a push failed again the same way. It was not the same
bug: `sync_with_remote` had rebased onto the remote as it was a moment
earlier, and between that fetch and the push an unrelated session landed
`GW523 staging note`. A single-shot sync-then-push is a
time-of-check-to-time-of-use race, and on a repo with other active authors it
fires regularly rather than rarely — twice in this harness's first hour.

Both push routes now retry up to three times, re-syncing each round, and give
up rather than looping. Giving up costs freshness of the repo and never work,
because the paste mirror has already succeeded by then. **A retry loop is the
only correct answer to a shared ref; there is no amount of checking beforehand
that closes the window.**

## The bridge skips a file whose md5 already matches, so reuse a name at your peril

A push failed with `md5 mismatch after push: local 2237609f… remote bdc7fdf9…`,
and `bdc7fdf9` was the PREVIOUS push's bundle. `wsx` sends nothing when the
far side already holds a file of that md5, and the harness was writing every
bundle to one fixed remote name, so a stale file could satisfy the check while
holding the wrong commits. Each push now uses a unique name and deletes it
afterwards. The general shape: an idempotence optimisation keyed on content is
only safe if the NAME is unique per payload.

## A map directory holds several maps, and hashing "the map" picks one at random

`227654` holds the pristine map and a segment cut; `267460` holds four
variants. A registry keyed by directory recorded whichever the scanner picked
and the verifier compared whichever IT picked — two perfectly correct maps read
CHANGED. Rows are keyed by id AND file name. A derived variant cannot be
refetched from Nadeo; it is reproduced by re-running the surgery.

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

## The map corpus is refetched, not carried — SUPERSEDED, and here is what replaced it

This entry used to say the corpus living only in a 30-day store was an open
gap. Ruled 2026-08-24: Nadeo's map files do not go in a public repo, and the
answer is `autopilot/config/maps.rec` — uid, name, the author time out of the
map's own header, the documented GET route, an md5 and a byte count per FILE.
Recovery is refetch-then-`tmresim maps verify`. What is still untested is the
refetch itself: every map was already on the box when the registry was built,
so no row's URL has yet been shown to return the bytes its hash names.

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

## The push bridge is a machine somebody can switch off

2026-08-26T14:20Z: every push started failing with `[whitestick] error:
instance offline`. The render box hosting the bridge was simply not running.
Nothing on our side was wrong and nothing could fix it from here: a deploy key
on a box that is off is not reachable, and no on-demand box and no devserver
holds a GitHub credential of its own (checked: no `~/.git-credentials`, no
`gh` config, no ssh key).

What this costs and what it does not: **state is safe** — the paste mirror
needs only the box's x509 cert, so banking continues and `unbanked_drift`
stays armed as designed. What was *not* safe was **code**: a fix committed
during the outage lived only on the box that wrote it, and the rotation that
was two hours away is designed to throw that box away. Fixed by giving code
its own mirror through the transport that still works (`tmhaul code
mirror`/`code recover`, HARNESS.md §Durability).

Three defects fell out of the same outage, each one a thing the harness said
that was not true:

- **The status page contradicted itself.** The "GitHub banking" row read the
  credential; the alarms table was evaluated on a view that never carried it.
  The page said **DEGRADED** at the top and "None firing" below. Two renders of
  one page must come from one judgement, and now do.
- **A dead bridge reported as an untried one.** `Health::PresentUnproven` says
  "no bridge operation has been tried" — printed about a probe that had been
  sent and had failed. Two states, one variant. Split.
- **A transport's error body was being committed to a public repo** and pasted
  into a markdown table, where its newlines broke the table exactly when the
  page mattered. Receipts now carry one brief line; the full text stays on the
  box's stderr.

The general shape, and the reason all three sat unnoticed for days: **a
degraded path exercises code that the healthy path never runs.** Everything
here worked perfectly while the bridge was up.

## The tool that recovers code cannot be recovered by itself

First rotation after the code mirror was written (2026-08-26T15:30Z, box 56655
vanished mid-lease): the fresh box cloned GitHub, built `tmhaul`, ran
`tmhaul code recover` — and got the **usage text**. The binary came from
GitHub, GitHub was 17 commits behind, and the command it needed was in those
commits. Read carelessly, that output says "no such thing to recover".

Recovered by hand with `meta phabricator.paste read`, `base64 -d`, `git bundle
verify`, `git merge --ff-only` — nothing but git, meta and coreutils, which is
the property the fallback has to have. Written into HARNESS.md and, more
importantly, into the **heartbeat message**: the repo is stale in exactly the
situation where this matters, and the subscription text is the one channel
that is not.

The general shape: **a recovery mechanism that ships inside the thing being
recovered has a floor, and the floor is whatever the last successful publish
left behind.** Every layer of this harness needs a route that assumes only
what a bare box has.

Two more things the same rotation turned up:

- **Code was mirrored only when a push FAILED.** A box with no credential has
  push switched *off* — no failure, no error, no mirror — and that is the box
  most likely to be the only copy of something. The trigger is now "GitHub
  does not have these commits", which never consults the push settings.
- **HARNESS.md pointed at a `SETUP.md` that is not in this repo**, for both
  the Rust proxy config and the oracle download. A recovery instruction that
  names a file nobody can open is worse than none: it reads as authoritative.
  Both are inline now.
