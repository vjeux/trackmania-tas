# 146612 "Spaghetti Nights 2" cannot be opened in the in-game editor

**Status: the map file itself is the problem. Not the ghost, not accumulated
state in the game, not a slow load, not a wrong path.** Nothing can be filmed on
this map until someone works out why, and a regeneration for it is wasted effort
until then.

## The symptom

`ManiaTitleControlScriptAPI::EditMap()` accepts the path and returns `ok`, and
the editor never opens. The plugin's `/ctx` stays at `0` (menu) indefinitely
while `/ping` keeps answering and the game process stays alive and responsive.
Every other map in the corpus opens in 6–25 seconds through exactly the same
call.

## What was ruled out, and how

| hypothesis | test | result |
|---|---|---|
| slow load, timeout too short | waited 90 s, then 120 s | never opens |
| accumulated state in the game (`EditMap` stops working after ~40 loads) | relaunched the game, made 146612 the **very first** map loaded, zero prior loads | never opens |
| wrong or mangled path | dumped the exact bytes handed to `EditMap` with `od -c` | correct |
| path does not resolve on the Windows side | `powershell Test-Path '<abs path>'` | `True` |
| file missing or truncated | `stat` = 3,824,673 bytes; GBX header intact (`GBX\6\0BUCR...`) | fine |
| the map is too large | 285268 is 4.65 MB and films fine; 210218 is 10.0 MB | not size |
| a stale copy of the map file | md5 `16e7220f2128587c0d0018626feacb0f`; **the shared store has a byte-identical second copy, and the engine loads and simulates the map** | **CLOSED 2026-08-22 — see below** |

The accumulated-state hypothesis was the strongest candidate, because the editor
genuinely does stop accepting maps after roughly forty loads — `/ping` answers,
`/ctx` stays 0, and only a relaunch fixes it. **146612 fails identically on a
completely fresh process**, which is what separates the two.

## A trap worth knowing, because it invalidated three earlier attempts

Do **not** build a Windows path with `printf`:

```bash
printf 'C:\\Users\\vjeux\\...\\tas\\146612.Map.Gbx'   # WRONG
```

`printf` interprets `\v` as a vertical tab and **`\146` as an octal escape for
`f`**, so `\tas\146612` was silently handed over as `\tasf612` — a path to a file
that does not exist. Three "diagnoses" of this map were made against that
nonexistent path before the mangling was spotted in an echoed log line. Use a
quoted heredoc, which interprets nothing:

```bash
cat > "$PS/editmap.txt" <<'EOF'
C:\Users\vjeux\OneDrive\Documents\Trackmania\Maps\Downloaded\tas\146612.Map.Gbx
EOF
```

The render scripts were never affected — they use doubled backslashes — but every
hand-probe was.

## What has not been tried

**Two of the three below have now been answered — 2026-08-22.**

> ### The map file is not corrupt, and the second copy exists
>
> The table above lists *"a stale copy of the map file … the repo ships no
> `.Map.Gbx` to compare against"* as **unresolved**. There is a second copy: the
> shared store carries `tm-unbeaten/146612/map.Map.Gbx`, and it is **byte-identical
> to the staged file** — md5 `16e7220f2128587c0d0018626feacb0f`, 3 824 673 bytes,
> the same figures this page already records.
>
> And the stronger test, which needs no second copy at all: **the game's own
> engine loads this map and simulates on it.** The dedicated server was pointed
> at it and re-simulated every publishable ghost in the directory to exactly the
> time in its name — eight of eight, with the `SEGMENT_…_DO_NOT_PUBLISH` file
> returning DNF at cps 5 as its name says, which is the negative control:
>
> ```
> TAS_39183.Ghost.Gbx   PASS V7   oracle re-simulated the written file: 39.183
> TAS_39430.Ghost.Gbx   PASS V7   oracle re-simulated the written file: 39.430
> ...
> SEGMENT_cp5_32702_…   FAIL V7   oracle: DNF (cps Some(5))
> ```
>
> A file that parses, loads, spawns a car and runs 40 seconds of physics
> **is not corrupt**. So *"the staged file is corrupt in a way that keeps the
> header valid"* is **closed**, and with it the "re-download and diff" item.
>
> **What this does not settle** is the actual symptom. `EditMap()` still returns
> `ok` and never opens the editor, and that remains unexplained — but it is now
> known to be a fault in the **editor path**, not in the map. The remaining
> untried item below is the one that would localise it.

- ~~Re-downloading the `.Map.Gbx` from TMX and comparing.~~ **Done differently
  and closed**: the store copy is byte-identical, and the engine runs the map.
- Opening the map through the game's own UI rather than the scripting API. If it
  opens by hand, the fault is in `EditMap` for this file specifically; if it does
  not, the file is bad in a way the editor rejects silently — **though the
  oracle result above makes "the file is bad" much harder to sustain.**
- Reading the game's own client log at load time for a rejected-path or parse
  error. Openplanet's log shows nothing, but that is the plugin's log, not the
  client's.
