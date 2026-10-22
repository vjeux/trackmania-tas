# wslwatch

Restarts the WhiteStick box's WSL when it freezes while the whitestick agent
still looks online.

**Why.** Oct 21 2026, 2:00 AM: WSL on the box stopped completing disk I/O. The
agent kept its relay connection (that needs no disk), so `whitestick status`
said *online* for 18 hours while every command hung. Nothing inside WSL can
recover from that, so this runs on the Windows side.

**What it does.** Every 60 s, `wslwatch.exe watch` runs
`wsl.exe -d Ubuntu -u vjeux --exec ~/bin/wslwatch probe`, which writes, fsyncs
and reads back `~/.whitestick/wslwatch-probe`. When probes have gone
unanswered for 5 minutes (each one killed at 60 s), it runs
`wsl.exe --shutdown`, then `--shutdown --force` if that hangs. The
Startup-folder `whitestick-agent.vbs` loop then brings the agent back. After a
restart it waits 10 minutes before it would act again.

Only silence counts. A probe that fails fast (missing or broken
`~/bin/wslwatch`, say) means WSL answered: it is logged once and never
restarts anything, so a bad probe binary cannot kill the box's jobs every 10
minutes.

Restarting WSL kills everything running in it. Before a deliberate long WSL
shutdown (compacting the disk), stop the watcher: `wslwatch.exe uninstall`.

## Files on the box

| | |
|---|---|
| `C:\Users\vjeux\.whitestick\wslwatch.exe` | the watcher (GUI subsystem: no window) |
| `C:\Users\vjeux\.whitestick\wslwatch.log` | its log, Windows side, so it survives a frozen WSL; rotates at 5 MB |
| `HKCU\...\CurrentVersion\Run` → `WhiteStick wslwatch` | starts it at sign-in |
| `~/bin/wslwatch` (WSL) | the probe |

```
/mnt/c/Users/vjeux/.whitestick/wslwatch.exe status     # running? + log tail (exit 3 = not running)
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
then `target\release\wslwatch.exe install`. Install copies itself to
`.whitestick\wslwatch.exe`, sets the Run entry, stops the older copy and starts
the new one. Only one watcher runs at a time: each holds an exclusive lock on
`.whitestick\wslwatch.lock`.

Test without touching WSL: `--dry-run` logs *would run wsl.exe --shutdown*
instead, and anything after `--` replaces the probe, for example a fake freeze
(`-- C:\Windows\System32\wsl.exe -d Ubuntu -u vjeux --exec /bin/sleep 30`) with
`--interval 5 --probe-timeout 5 --hang-after 20`. Give a test instance its own
`--log` and `--lock`, and `--max-restarts 1` so it exits after acting: a Windows
process outlives the WSL it shuts down, and the next command that could kill
it only runs once the agent is back.
