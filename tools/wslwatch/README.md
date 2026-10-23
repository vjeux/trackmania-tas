# wslwatch

Restarts the WhiteStick box's WSL when it freezes while the whitestick agent
still looks online. It never restarts Windows.

**Why.** Oct 21 2026, 2:00 AM: WSL on the box stopped completing disk I/O. The
agent kept its relay connection (that needs no disk), so `whitestick status`
said *online* for 18 hours while every command hung. Nothing inside WSL can
recover from that, so this runs on the Windows side.

**What it does.** Every 60 s, `wslwatch.exe watch` runs
`wsl.exe -d Ubuntu -u vjeux --exec ~/bin/wslwatch probe`, which writes, fsyncs
and reads back `~/.whitestick/wslwatch-probe`. When probes have gone
unanswered for 5 minutes (each one killed at 60 s), it runs
`wsl.exe --shutdown`, then `--shutdown --force` if that hangs. The
Startup-folder `whitestick-agent.vbs` loop then brings the agent back.

Only silence counts. A probe that fails fast (missing or broken
`~/bin/wslwatch`, say) means WSL answered: it is logged once and never
restarts anything.

## Safety limits

- **Rate limit:** at most one WSL restart per hour and three per 24 h
  (`--min-gap 3600`, `--max-per-day 3`). Every restart wslwatch makes, test
  cycles included, is recorded in `.whitestick\wslwatch.restarts`, so the
  limits survive restarts of the watcher and of Windows. A hang inside the hour
  is logged and waited out. A hang that would need a fourth restart in 24 h
  turns the watcher OFF.
- **No loops:** after its own restart it polls until WSL answers AND the agent
  log shows `connected to relay` after the restart. If both are not back within
  10 minutes (`--recover-within 600`), it turns itself OFF.
- **Off switch:** while `%USERPROFILE%\.whitestick\wslwatch.off` exists, the
  watcher only logs. `wslwatch.exe off [--reason TEXT]` creates it, `wslwatch.exe on`
  removes it, or create/delete the file by hand at the console. When the watcher
  turns itself OFF it writes the reason there, logs it, and shows a message box
  on the box's screen (not topmost, so it never takes focus from a game).
- **Log-only until tested:** `install` always sets the off switch.
  `wslwatch.exe test-cycle --detach` runs the controlled cycle: a simulated hang
  (a probe that sleeps) must be detected, WSL is really restarted, and WSL plus
  the agent must come back. Only a pass removes the off switch.

Restarting WSL kills everything running in it. Before deliberate WSL
maintenance, `wslwatch.exe off --reason "..."`.

## Files on the box

| | |
|---|---|
| `C:\Users\vjeux\.whitestick\wslwatch.exe` | the watcher (GUI subsystem: no window); the previous build stays as `wslwatch.old.exe` |
| `C:\Users\vjeux\.whitestick\wslwatch.log` | its log, Windows side, so it survives a frozen WSL; rotates at 5 MB |
| `C:\Users\vjeux\.whitestick\wslwatch.off` | the off switch, with the reason |
| `C:\Users\vjeux\.whitestick\wslwatch.restarts` | every restart it made (unix seconds, UTC, why) |
| `HKCU\...\CurrentVersion\Run` → `WhiteStick wslwatch` | starts it at sign-in |
| `~/bin/wslwatch` (WSL) | the probe |

```
/mnt/c/Users/vjeux/.whitestick/wslwatch.exe status   # running? OFF? restarts in 24 h, log tail
                                                     # exit 3 = not running, 4 = OFF
```

## Build and install (on the box)

The probe is a Linux build. `.cargo/config.toml` here overrides the tools/
`znver4` target: the box's i7-12700F has no AVX-512, and a `znver4` build dies
with SIGILL there.

```
cargo test --release && install -m 755 target/release/wslwatch ~/bin/wslwatch
```

The watcher is a Windows build: copy `Cargo.toml`, `src`, `.cargo` to
`C:\Users\vjeux\.whitestick\wslwatch-src`, run `cargo.exe build --release` there,
then `target\release\wslwatch.exe install` (log-only), then
`C:\Users\vjeux\.whitestick\wslwatch.exe test-cycle --detach` and read the
result with `status` once the agent is back. Only one watcher runs at a time:
each holds an exclusive lock on `.whitestick\wslwatch.lock`.

Test without touching WSL: `--dry-run`, plus `--dir` for a scratch state
directory; anything after `--` replaces the probe, for example a fake freeze
(`-- C:\Windows\System32\wsl.exe -d Ubuntu -u vjeux --exec /bin/sleep 30`) with
`--interval 5 --probe-timeout 5 --hang-after 20 --max-restarts 1`.
