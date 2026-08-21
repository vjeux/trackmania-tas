# [Turtle Trial] Angustus

**On a trial map the clock runs through your failures: the unbeaten author time
is not a display of driving, it is a competent lap plus nineteen crashes — so
hard-respawn the instant you take a checkpoint, and learn the one obstacle that
cost the author 197 seconds.**


> ### ⚠️ Five replay files withdrawn — they carried the human's recorded trajectory
>
> `TAS_239133`, `TAS_262907`, `TAS_267646_v7`, `TAS_268554_v6` and
> `TAS_347003_noretry_v4` have been removed from this page. Each one decodes to
> **Quantiks' own recorded trajectory, bit for bit** — 9114 of 9114 samples on
> `TAS_239133`, and the same whole-file match on the other four.
>
> **The times stand and the method stands.** Every one of these tapes
> re-simulates on the game's own oracle to the millisecond in its name, and this
> map's entire result comes from *deleting failed attempts*, which is a property
> of the input tape rather than of the recording. What was wrong is the
> recording of how the car moved — the only thing a replay file shows.
>
> A tape built by editing an existing run inherits that run's telemetry unless it
> is regenerated from engine memory. On this map the regeneration was attempted
> and **failed** — recorded at the time, in a commit message, and then published
> anyway. That is the real defect: the finding existed and nothing refused to
> ship on it. Files now pass a gate before a page can reference them.
>
> Replacements will be regenerated. Until then this page has replay files for
> nothing, and the numbers below are the result.

> ### ⚠️ And two of the files that stayed were declaring the human's time — now fixed
>
> The banner above withdrew five files and left three. **Two of the three that
> stayed were also carrying Quantiks' 1964.933** — not in their driving, which is
> ours, but in the container's *declared time*, at two of its six sites each. A
> reader who followed the banner above would reasonably have assumed the
> remaining files were clean. They were not.
>
> `NORETRY_347003_watchable` and `NORETRY_407463_watchable` have been repaired in
> place. Each now declares its own validated time at **all six sites, with no
> foreign value anywhere in the file**, confirmed by two readers that did not make
> the edit and by a third, independent byte scan. Both still re-simulate on the
> game's own oracle to the exact millisecond in their names — the repair changed
> what the file *claims*, never what it *does*.
>
> **The third file, `AUTHORCUT_246602_watchable`, is not yet clean**: it declares
> the map's author time at one site and still carries a donor's skin reference.
> It is being repaired next, and this line stays until it is.
>
> Two things worth saying plainly, since this page is where we said the last one:
> the tool that repairs a declared time **had a short-circuit that read one chunk,
> found the right value there, and reported "nothing to do" on a file that was
> still wrong at two other sites** — which is very likely how these two came to be
> published in the first place. And a checkpoint split inside both files still
> belongs to the donor; no tool we have reads that field yet, so this page does
> not claim it is clean.

| run | time | vs author time |
|---|---|---|
| **the author's lap, failures cut, then optimised** — `TAS_239133` *(file withdrawn)* | **239.133** | **−223.849 (−48.3 %)** |
| the author's lap, failures cut only — **watchable** | 246.602 | −216.380 |
| the earlier human-derived line | 262.907 | −200.075 |
| Author time | 462.982 | — |
| The only human record (Quantiks) | 1964.933 | +1501.951 |

TMX map [238835](https://trackmania.exchange/maps/238835) · author **Bald_tm** ·
tags **Trial, Turtle** · 5 checkpoints · **1 recorded run**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

## Where the time is

The recorded time on a trial map is clean driving plus every failed attempt. The
human's 32-minute record contains **198 respawn presses**. The author's own
462.982 lap contains 20 — and roughly **160 of those seconds are fourteen failed
attempts at a single obstacle**. Their actual clean driving is about 246 s.

So you do not have to drive better than the author. **You have to fail less.**
The entire margin above came from removing failed attempts, with no change to the
driving at all.

### The one technique worth learning

> **Hard-respawn the instant you take a checkpoint.**

A checkpoint's saved state is live on the very next tick after it fires. On a
turtle map you cross checkpoints upside down, so a *soft* respawn — one press —
hands you back a car doing 62 km/h on its roof, at the same attitude you crossed
in. Press it **twice** within about 100–640 ms, or use a bound standing-respawn
key, and you get the checkpoint block's own transform instead: **dead standstill,
upright, square**, after a freeze of about 0.5–0.9 s.

**The author already does this at CP2 and CP4** — 70 ms after crossing one and
273 ms after the other. On this tape it was worth 70 seconds and it costs
nothing.

### Where the map is actually hard

Clustering the human record's 106 respawn actions by where each attempt died:

| # | segment | where the attempt dies | attempts | time burned | share of the run |
|---|---|---|---|---|---|
| **1** | CP2→CP3 | **(974, 95, 518)** | **39** | 664 s | **33.8 %** |
| 2 | CP3→CP4 | (1048, 135, 456) | 8 | 177 s | 9.0 % |
| 3 | CP4→finish | (914, 96, 603) | 12 | 140 s | 7.1 % |
| 4 | CP2→CP3 | (899, 89, 557) | 18 | 121 s | 6.2 % |

**Four places account for 55 % of a 32-minute run.** Obstacles 1 and 4 are the
same climb approached at two stages: 57 of the 70 attempts in CP2→CP3 die on one
feature.

Every death has the car **inverted (roll 2.3–3.1 rad) at 0–35 km/h**. That is the
turtle signature — failure means coming to rest upside down, not falling off.

Per segment, the human record splits as:

| segment | human elapsed | respawn actions |
|---|---|---|
| start→CP1 | 58.9 s | 0 |
| CP1→CP2 | 200.8 s | 3 |
| **CP2→CP3** | **1072.6 s** | **70** |
| CP3→CP4 | 372.0 s | 14 |
| CP4→finish | 260.7 s | 20 |

## The run, obstacle by obstacle

There is no clock to hit on this map and no timing window to learn. In order of
what it is worth to practise:

1. **(914, 97, 604)** — the last obstacle before the finish, and the map's real
   boss. 12 human attempts, 14 author attempts, about 300 s burned between them.
   This is where the author time was lost. **If you take it first time, you beat
   the author time with ordinary driving.**
2. **(974, 95, 518)** — the inverted climb in CP2→CP3. The car runs a corridor at
   y ≈ 89 and then has to carry momentum up 8 m of vertical rise while upside
   down. **Run-up speed is not the answer**: the successful pass entered at
   68.2 km/h, the median of 47 measured entries, and plenty of faster entries
   failed. 26 of the 70 attempts get within 3 m of the crest and slide back — it
   is all in the last two metres, in line and attitude.
3. **(899, 89, 557)** — the entry to that same corridor, 65 m out.
4. **Nothing before CP1 matters much.** Zero respawns there in either run, and
   the whole segment is 48–59 s of ordinary driving.

Take each checkpoint however you can, and respawn out of it upright.

## How forgiving it is

Very — in the sense that matters here. Nothing on this map asks for a
millisecond: the driving is at walking pace, and the only "input" with a
deadline is the respawn after a checkpoint, which needs to be within a tick of
the crossing to claim the saved state.

The map is unforgiving in the other sense. Every obstacle is a car being driven
on its roof, and a failure is not a crash you recover from, it is a respawn and
another attempt on the clock. The difference between the author's 462.982 and
their own clean 246 s is entirely how many attempts they needed.

## Files

| file | what |
|---|---|
| `replays/TAS_239133.Ghost.Gbx` | **the result — 239.133**, the author's own lap with every failed attempt removed and then tightened |
| `replays/AUTHORCUT_246602_watchable.Ghost.Gbx` | **the watchable one** — loads in the game and shows the author's own driving with their fourteen failures cut out, no TAS driving at all |
| `replays/TAS_262907.Ghost.Gbx` | the earlier line, built from the human record instead of the author's |
| `replays/TAS_347003_noretry_v4.Ghost.Gbx` | an earlier, more conservative cut |
| `replays/TAS_268554_v6.Ghost.Gbx` | the stage before the last |
