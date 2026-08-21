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
| a stale copy of the map file | md5 `16e7220f2128587c0d0018626feacb0f`; the repo ships no `.Map.Gbx` to compare against | unresolved, but the file is self-consistent |

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

- Opening the map through the game's own UI rather than the scripting API. If it
  opens by hand, the fault is in `EditMap` for this file specifically; if it does
  not, the file is bad in a way the editor rejects silently.
- Re-downloading the `.Map.Gbx` from TMX and comparing. There is no second copy
  to diff against, so "the staged file is corrupt in a way that keeps the header
  valid" remains open.
- Reading the game's own client log at load time for a rejected-path or parse
  error. Openplanet's log shows nothing, but that is the plugin's log, not the
  client's.
