# ROLLOUT-WORKER.md — multi-box rollouts: one worker process per box, a master on any

Proposed by the ENV arm, 2026-09-07 07:30, for LEARN's OK before any code (coordinator's order). INTERFACES.md style:
what crosses the wire is spelled out to the byte; everything else is the implementer's.

## 0. Shape

```
                 ┌──────────── box 1 ────────────┐
 tmrl ppo  ──TCP──▶ tmenv rollout-worker :7001  ── 96 ForkEnv workers on 96 servers
 (master, ──TCP──▶ box 2  :7001                ── …
  any box) ──TCP──▶ box N  :7001
```

* **One `tmenv rollout-worker` per box** (`--listen 0.0.0.0:7001 --maps-dir <data/v0/maps> --workers 96
  --work /dev/shm/rw`). It owns the servers, the envs, the maps (geom.json, template, StateArchive) and the RNGs. It
  never touches the bank: the master hands it everything (the policy bytes, the episode requests) and takes everything
  back (steps, rows, the banked tape bytes).
* **The master is `tmrl ppo --workers host1:7001,host2:7001,…`** (LEARN's). It sends one `SetPolicy` per iteration,
  then `RunEpisodes` batches, and receives `Episode` frames as they finish. The master is the only writer of the
  bank and of the trainer's state.
* **TCP, length-prefixed frames, little-endian, no TLS** (the devvms are one network). The bank is a slow FUSE; a job
  dir would put every step on it. The policy forward runs IN the worker (obs → action must be local to the env step),
  so the worker embeds the policy net: see §4.

## 1. Frames

Every message is `len: u32` (bytes after this field) · `kind: u8` · payload. Strings are `u16 len + UTF-8`. Arrays of
f32 are raw LE bytes. A frame the receiver does not know is a protocol error: close the connection.

### 1.1 master → worker

| kind | name | payload |
|---|---|---|
| 0x01 | `Hello` | `proto_version u32 = 1`, `master_id str` |
| 0x02 | `SetPolicy` | `policy_id u64`, `obs_version u32`, `tmw_len u32`, `tmw bytes` (the `policy.tmw` file, INTERFACES.md §Trained-policy artefact; ~100 KB) |
| 0x03 | `RunEpisodes` | `batch_id u64`, `n u32`, then `n ×` [`ep_id u64`, `map_uid str`, `start: u8` (0 root, 1 archive state), `state_id u64` (when start = 1: the id in the map's persisted StateArchive), `seed u64`, `temperature f32`, `max_steps u32`, `k_ticks u16`, `flags u8` (bit0 want CarState rows, bit1 want the banked tape bytes, bit2 deterministic argmax)] |
| 0x04 | `Cancel` | `batch_id u64` (episodes not yet started are dropped; running ones finish) |
| 0x05 | `LoadMaps` | `n u32`, `n × map_uid str` — warm these maps' servers/templates/archives now (optional; a `RunEpisodes` for an unloaded map loads it on demand, ~3 s per map per worker) |
| 0x06 | `Quit` | — |

### 1.2 worker → master

| kind | name | payload |
|---|---|---|
| 0x81 | `Ready` | `proto_version u32`, `worker_id str` (host), `n_workers u32`, `obs_versions_supported u32 = 3` (bit i = version i+1), `state_version u32 = 3`, `git_head str` |
| 0x82 | `PolicyAck` | `policy_id u64`, `ok u8`, `err str` (a policy whose OBS_VERSION/OBS_DIM the worker cannot observe with is refused here, never at episode time) |
| 0x83 | `Episode` | §2 |
| 0x84 | `EpisodeError` | `ep_id u64`, `err str` (the env refused: no such map, no such state, template failure; the master decides whether to retry elsewhere) |
| 0x85 | `Stats` | `batch_id u64`, `done u32`, `running u32`, `queued u32`, `env_steps_per_s f32`, `load1 f32` — every 2 s while a batch runs |
| 0x86 | `MapLoaded` | `map_uid str`, `ok u8`, `err str`, `n_states u32` (archive size) |

## 2. The `Episode` frame (0x83)

```
ep_id u64 · batch_id u64 · policy_id u64 · map_uid str · seed u64 · start u8 · state_id u64
obs_version u32 · obs_dim u32 · n_steps u32 · k_ticks u16
done u8            0 running(cap) 1 Finished 2 OffRoute 3 NoProgress 4 Crash 5 RunEnded 6 TickCap   (tmenv::core::Done + running)
truncated u8       1 when cut by max_steps (the master bootstraps), else 0
finish_ms i32      the engine's finish word (−1 none)
gates u32 · reward_sum f32 · wall_ms u32 · worker_ticks u32
steps: n_steps × [ obs f32×obs_dim · action u32 · logp f32 · value f32 · reward f32 · terminal u8 · truncated u8 · next_value f32 ]
rows_len u32 · rows: rows_len × CarState (120 B, STATE_VERSION 3, one per TICK, tmstate LABEL CONVENTION)   (0 unless flag bit0)
tape_len u32 · tape bytes (the banked Ghost.Gbx, tmenv banked_tape, 0 unless flag bit1)
```

`Step` is `tmrl::ppo::Step` byte for byte in field order: the master appends it to the batch as it does today.

## 3. Determinism (the control)

An episode is a pure function of `(policy bytes, map_uid, start, state_id, seed, temperature, max_steps, k_ticks)`:
the worker seeds its RNG with `seed` alone (not the worker index), samples with `tmrl::net::sample(logits, rng.f32())`
and steps the env; the env is deterministic (G4). **Control: `tmenv rollout-worker-control` runs 50 episodes locally
through the same code path and 50 through a worker on the loopback and compares the `Episode` frames byte for byte
(steps, rows, tape); then 10 through a worker on ANOTHER box** — the same 50/50 shape as G4. A frame that differs is a
bug, not noise.

## 4. Who owns what — the one interface question for LEARN

The worker must run the policy forward. Today the forward (`bcnet::Weights::forward`, `net::sample`, `policy::read`)
lives in `tmrl` (a binary crate). Proposal: **`tmrl` gains a `lib.rs` exporting `policy`, `bcnet`, `net`, `ppo::Step`**
(no behaviour change), and the worker is a new binary `tools/rl/tmroll` (ENV's) depending on `tmenv` + `tmrl`
(lib). LEARN keeps the master (`tmrl ppo --workers …`) and the artefact format; ENV keeps the worker, the protocol
and the controls. The alternative — copying the forward into tmenv — is a second implementation of the policy, which
the 50/50 control would catch drifting but nobody wants to maintain.

What the master must do differently from today: instead of stepping local envs, it sends `SetPolicy` once per
iteration and `RunEpisodes` with the iteration's episode list (map uids from the dataset gate, `start` from its
curriculum, seeds from its RNG), and assembles `Step`s from the `Episode` frames as they arrive from any worker (order
is by completion, not by request — `ep_id` says which is which).

## 5. Throughput expectations

A worker box does what `tmrl ppo` does on one box today: 96 envs on tmpfs, 12.9k env-steps/s at k=1, 14.6k at k=10
(G5 reference table). Per episode frame at k=10, 200 steps, obs v2: 200 × (400 + 20) B = 84 KB + rows 2000 × 120 B =
240 KB (if asked) + tape ~55 KB (if asked): ~380 KB per episode, ~70 episodes/s per box → ~27 MB/s per box with
everything on, 6 MB/s with steps only. Ten boxes: 146k env-steps/s to one master over TCP; the master's PPO update
(candle, one box) is the bottleneck then, not the wire.

## 6. Failure rules

* A worker that loses the master exits its envs after 60 s idle and waits for a new `Hello`.
* An env error inside an episode → `EpisodeError`; the worker rebuilds that env (fresh server) and keeps serving.
* `Ready.git_head` ≠ the master's expectation → the master decides (a mixed fleet is a control violation; the master
  refuses by default).
* No retries inside the protocol: the master re-requests an `ep_id` on another worker if it wants to.

## 7. Not in v1

Streaming steps mid-episode (the master needs whole episodes for GAE anyway); worker-side value bootstrapping
policy updates; compression (the frames are floats; LZ4 later if the wire ever matters); TLS/auth.

## 8. Status 2026-09-07 07:50 — built, LEARN's changes in, controls pass (player-env 64befedb+)

* LEARN's seven changes (07:37) are in: chunk actions `k × [steer_bin, gas, brake]` with the chunk log-prob;
  `bcnet::sample_chunk` through `tmrl` as a library (c7289890; `TmwPolicy`); `best_s`/`length_m` in `Episode`;
  `ArchiveList`/`Archive` with `origin` (0 human line, 1 policy); `LoadMaps` seeds the human line (donor tape, snapshot
  every `snap_every` ticks) into a box-wide per-map STORE (prefix + metadata under the file's `state_id`; any env
  thread imports a state it has not seen); `RunEpisodes` gains `snap_every` and `margin_m`; `Ready.git_head` for the
  master to refuse a mixed fleet. Frames: §1 updated in place; `[::]` listening (the devvms are IPv6-only).
* **Byte-identity control** (`tmroll control`): 50 episodes through a loopback worker (8 envs) vs the same 50 locally
  → 50/50 identical frames (steps, per-tick rows, banked tapes; only `wall_ms` masked); **cross-box**: box B worker
  (16 envs) vs box A local → 50/50 identical. `controls/…/rollout-worker-crossbox-64befedb.log`.
* **Throughput** (`tmroll bench`, Summer 2026 - 01, k = 10, 60 steps, 96 envs per box, steps only): 1 box 10.3–13.1k
  env-steps/s (the 12 s window includes the ramp), **2 boxes 25.2k env-steps/s = 252k game-ticks/s, 592 episodes/s,
  11.4 MB/s to the master, 0 errors** — linear. `controls/…/rollout-worker-bench-64befedb.log`. 4 boxes when the
  replacement boxes arrive.
* Not yet: a `.tmw` end-to-end run with LEARN's master (the worker side accepts a `.tmw` and refuses a shape it cannot
  observe with; the master's `--workers` is LEARN's).

## 9. Fleet notes 2026-09-07 08:35

* **Throughput, 4 boxes** (R1 devvm62172 96 envs, box B devvm64651 96, R2 devvm62311 64, box A devvm64415 64; k = 10,
  60 steps, 3,000 episodes per box, steps only): **47,960 env-steps/s = 480k game-ticks/s, 1,126 episodes/s,
  21.6 MB/s to the master, 0 errors** — 1 box 12.6k, 2 boxes 19–25k, 4 boxes 48k: linear.
  `controls/…/rollout-worker-bench-4box-*.log`.
* **Regions.** A direct TCP connection from a cln0 box to a worker in lla0 is accepted and then RESET by the network
  (both sides see `Connection reset by peer` right after the handshake); same-region works. Put the master and its
  workers in one region, or tunnel: `ssh -N -L 17002:localhost:7001 <worker>` and give the master `localhost:17002`
  (R2 ran the full bench through such a tunnel at 13.6k env-steps/s).
* The master flag in LEARN's build is `tmrl ppo --workers-remote host:7001,…`. LEARN's first end-to-end (2 boxes, 12
  maps, 3 PPO iterations): 46k steps per iteration in 15 s wall, 0 protocol errors, one env error in 1,440 episodes
  (a residual `PROBE-HUNG`, reported as `EpisodeError`, under investigation).
* `bootstrap-worker.sh` runs ONE restart loop; a second launch on a box that already has one flaps on
  `Address already in use` — kill the old `/tmp/tmroll-run.sh` first (the script does `pkill -x tmroll` but not the loop;
  fixed to kill the loop too).
* **Map affinity.** An env thread serves jobs on the map it already holds before taking the queue's head (a map
  switch is a server restart, ~3 s). A 3,000-episode batch round-robined over 6 maps on a 48-env worker: 7.4–8.0k
  env-steps/s against 9–12k single-map — the master may mix maps freely in a batch; grouping still helps.
* `tmroll status host:7001,…` — Hello → Ready per worker (a health check for launchers; exit 1 if any is down). A
  worker whose envs a master holds still answers (Ready marked BUSY; work from a second master is refused with
  errors), so the check works mid-run.
