# inputidle — "is a person using this machine?" for the WhiteStick box

vjeux, 2026-10-04: *"implement a check for me using the machine, needs to have
keyboard/mouse/controller in the past minute. Nobody has been using the machine most of
the day."*

The box is **free** when keyboard, mouse and controller have all been quiet for
`--window-s` (60) seconds — and **busy** otherwise, including when the check itself
cannot run.

**A fresh clock is not a person** (vjeux 2026-10-05, after a Minecraft launcher sat in
the foreground for nine hours): a game keeps the OS input timer at 0 by itself. So when
the clock looks fresh, `status` watches for `--confirm-s` (5) seconds — a person moves
the pointer, switches window, or produces DISCRETE events (the idle counter climbs
between them); a flat 0 with nothing moving is the foreground app pumping it, reported as
`clock=app-pumped` and judged FREE. `probe --seconds N` is the same reading over a longer
window, for a human to look at (`person=yes|app|no`). Before this, a launch was gated on the process list, which twice reported a
free box while Hades II / Minecraft were in the foreground (the screenshots came back as
vjeux's screen), and the alternative — waiting for his word — left the machine idle for
most of a day.

## What it reads
- **keyboard + mouse**: `GetLastInputInfo`, the OS's own last-input tick for the
  interactive session (a remote-play session counts: its input arrives as input).
- **controllers**: XInput has no "last input" clock, so `watch` samples the four slots'
  packet numbers every 250 ms and records the time of the last change in a small state
  file; `status` reads it AND runs a ~1.5 s live sample of its own, so a pad in use right
  now is seen even with no watcher running. A pad that speaks only DirectInput is not
  seen (Steam's virtual Xbox pad is).
- the foreground window's exe and title ride along in the line, for the log.

## Use
```text
inputidle.exe status [--window-s 60] [--state FILE] [--pad-sample-ms 1500]
  verdict=free|busy kbm_idle_s=N pad_idle_s=N|none|unknown pads=K fg=<exe> "<title>"
  exit 0 free · 1 busy · 2 error
inputidle.exe watch   --state FILE [--poll-ms 250]   the controller sampler (detached)
inputidle.exe install --state FILE                    starts the watcher now
```
On the box: `C:/Users/vjeux/inputidle.exe`, state `C:/Users/vjeux/inputidle-pad.txt`.
From the devserver: `tinyctl box-idle [--window-s 60] [--wait-s N [--poll-s 20]]` (same
exit codes), and every game-driving tinyctl command (`play`, `shoot`, `lightmap`,
`loadloop`) refuses to start on a busy box — `--ignore-idle` / `TINY_IGNORE_IDLE=1` is
the deliberate override.

## Build (no Windows SDK on the box — the credread recipe)
`raw-dylib` imports (user32/kernel32/shell32) + our own `mainCRTStartup` (`entry.rs`) and
CRT shim (`shim.rs`), linked with rust-lld against the generated import libs of
`tools/credread/target/xbuild/winlib`:

```
SR=$(rustc --print sysroot); LLD=$SR/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld
WL=/home/vjeux/trackmania-tas/tools/credread/target/xbuild/winlib
RUSTC_BOOTSTRAP=1 rustc --edition=2021 -O --target x86_64-pc-windows-msvc \
  -C linker=$LLD -C linker-flavor=lld-link -C panic=abort \
  -C link-arg=/LIBPATH:$WL -C link-arg=kernel32.lib -C link-arg=ntdll.lib \
  -C link-arg=userenv.lib -C link-arg=dbghelp.lib -C link-arg=advapi32.lib \
  -C link-arg=ole32.lib -C link-arg=oleaut32.lib -C link-arg=bcrypt.lib \
  -C link-arg=crypt32.lib -C link-arg=shell32.lib \
  -C link-arg=/errorlimit:0 -C link-arg=/NODEFAULTLIB:msvcrt -C link-arg=/SUBSYSTEM:CONSOLE \
  -C "link-arg=/ALTERNATENAME:??_7type_info@@6B@=__RUSTC_TYPE_INFO_VFTABLE" \
  -o out/inputidle.exe src/main.rs
```
(`cargo build --target x86_64-pc-windows-msvc` does NOT work on the box: cargo wants
`link.exe`.)
