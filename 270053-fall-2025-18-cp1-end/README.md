# Fall 2025 - 18 CP1 End

**You are not turning hard enough in the first half-second: an extra stab of
lock anywhere between 0.240 and 0.770 is worth a millisecond or two, and it is
what makes the faster corner exit pay.**

**Fall 2025 - 18 (CP1 end)** — TAS **4.492** (±0) | AT 4.492 | WR 4.495 (six players tied)

https://github.com/user-attachments/assets/fba3cbee-2914-428e-8519-e8e99e6d00aa

> **The two-car clip that was here was pulled, and has now been replaced.** It was filmed from a tape
> one physics tick late -- 10 ms, which puts the car 0.336 m behind itself along
> its own line. On a solo clip that is invisible and harmless; in a side-by-side
> on THIS map it is not, because the whole subject here is 0.689 ms and about
> 3.4 cm at the line. The clip would have been showing a display artefact
> fifteen times larger than the result it was illustrating. A re-shoot from the
> repaired tape is coming; the times and the tapes are unchanged.
>
> What it showed is worth keeping, because it will be true of the re-shoot too:
> our tape and AffiTM's world record never separate by more than **0.48 m**
> (mean **0.08 m**), so the two cars render inside one another and there is no
> moment where a viewer sees two vehicles. Six players are tied at 4.495 and the
> whole top 15 fits inside 0.003. The 0.003 we lead by is **14 cm at the line**,
> about a tenth of one video frame — **a chase camera cannot show it**, and the
> clip never pretended to. What it showed was the sameness: one racing line,
> driven to the half-metre, by a machine hunting the author medal and by the
> best human on the board.

> **Replaced 2026-08-21.** Both clips on this page are now filmed from the
> repaired tape `bb84db2ced762e657fa45359f29e3a81`, which the game imports as
> `Ghost:TAS` and which sits at the tick the game itself uses. The times and the
> tapes are unchanged.

**Fall 2025 - 18 (CP1 end)** — TAS **4.492** (±0) | AT 4.492 | WR 4.495 by AffiTM

https://github.com/user-attachments/assets/1279d078-eded-49ea-8b75-8c5085f1cde2

| run | time | vs author time | vs human WR |
|---|---|---|---|
| **TAS** | **4.492** | **±0** | **−0.003** |
| TAS, single-tick variant | 4.493 | +0.001 | −0.002 |
| Author time (never beaten by a human) | 4.492 | — | −0.003 |
| Human WR (six players tied) | 4.495 | +0.003 | — |

TMX map [270053](https://trackmania.exchange/maps/270053) · author **in-.-** ·
1,052 recorded runs.

The tape matches the author time exactly, which takes the author medal — the
game awards it at or under the time. Nobody in 1,052 recorded attempts has managed
either: this is the most-hunted map in the collection and the best human is
0.003 short.

## The map in one paragraph

Four and a half seconds. **Full throttle from the lights to the line: never
lift, never brake.** The car stays on the ground the whole way and it never
slides — lateral speed stays under 0.7 km/h from start to finish. You are not
managing grip here, you are managing steering angle. The shape is a left kink
out of the start, a short straight, a small right correction, then one huge
downhill left sweeper held at full lock for about 1.3 s, crossing the line at
216 km/h.

Scale, so you know what you are fighting for: **1 millisecond is 6 cm of travel
at the line.** The whole top 15 of the leaderboard is spread over 0.003 — 18 cm.

## Where the time is

Take the human record's tape and change exactly one thing:

| what changes on the human record | time |
|---|---|
| nothing (the human record) | 4.495 |
| **one tick of extra steering lock at 0.420** | **4.493** |
| the whole corner-exit release, and nothing else | 4.495 — nothing |
| everything the fast tape does *except* that early lock | 4.495 — nothing |
| the early lock **and** the exit release together | **4.492** |

So the spectacular-looking part — releasing the exit two tenths early — is worth
**zero on its own**. The margin starts at the turn-in. The human record holds
about 52% lock (−66 of 127) through it, and the car wants more.

### The exit, worth about a millisecond once the entry is right

The human record holds full left lock right up to 4.350 and then snaps the
counter-steer in. The fast tape starts unwinding at **4.160** — nearly two
tenths earlier — and rolls it off progressively:

| race | human record | the 4.492 |
|---|---|---|
| 4.000–4.150 | −127 (full left) | −127 |
| 4.160 | −127 | **−124, starts releasing** |
| 4.200 | −127 | −102 |
| 4.250 | −127 | −74 |
| 4.300 | −127 | **−52 (about 40% lock)** |
| 4.350 | −68 (human starts) | −6 |
| 4.380 | +96 | **+127 (full right)** |

What the finish clock measures is the part of your speed pointing *through* the
line. While you hold lock the car is still rotating, and every degree not yet
cancelled is speed thrown across the line instead of through it. Unwinding
earlier stops the rotation sooner and lets the car accelerate a touch harder
over the last three tenths.

## The run, as inputs

Throttle is held from the countdown to the line and never released; the brake is
never touched. Everything else is steering.

```
race 0.240–0.770  extra LOCK   | more left than feels natural through the
                               |   turn-in; best around 0.420, and anything
                               |   from −83 to full lock −127 pays
race 4.160        start UNWIND | begin rolling the lock off, progressively
race 4.300        ~40% lock    | still turning, but already straightening
race 4.380        full RIGHT   | counter-steer in, hold across the line
```

## How forgiving it is

**Very.** This is the practical part.

- **Timing.** An extra stab of lock anywhere between **0.240 and 0.770** gains a
  millisecond; six different moments in that window gain two. There is no frame
  you have to hit.
- **Amount.** At the best moment, *any* value from about **−83 to full lock
  −127** pays. That is a 45-unit window on a 127-unit axis — you cannot miss it
  by being a bit greedy.
- **Shape.** You do not need a stab at all. An extra 3–8 units held for 5–10
  ticks, or a smooth swell of −5 over a tenth of a second, also pays. Sharp,
  smooth, brief, sustained: all of them work.
- **The exit is forgiving too.** There is a wide family of release shapes —
  release moment, release rate, how far you unwind, when and how fast you
  counter-steer — that all produce 4.492. Roll the lock off about two tenths
  earlier than you do now and commit to the counter-steer sooner.

In driver language: **turn in a little harder in the first half-second than
feels natural, and let it breathe back out.** It costs almost nothing at
20–40 km/h and it sets up the whole rest of the lap.

**What will take real practice** is nothing on the entry — it is keeping your
nerve on the exit. Releasing early feels like giving up the corner, and the
temptation is to hold lock and let the car run wide, which is the one thing that
does not work here.

### The warning

The finish trigger is narrow and has a hard edge on the outside, and the fast
line passes about half a metre from it. **Half a metre wider costs 0.010; two
metres wider and the run does not finish at all** — no time, no explanation. It
is equally unforgiving underneath: 25 cm lower and the car misses the trigger
entirely. So read "release earlier" as *stop turning sooner*, never as *run
wider*. The early release makes the car straighter, not the line wider, and that
distinction is the difference between a record and a DNF you will not
understand.

**Do not go looking for a different line.** The top 14 runs on the leaderboard
are within 30 cm of each other for the entire lap, and nothing faster than that
line exists — only better inputs on it.

## Can a human really do 4.492?

Yes. The author validated the map by driving it, so a human-sized 4.492 exists,
and this tape reaches the same millisecond by inputs no leaderboard run uses.
The margin sits in a coarse decision — more lock early — inside a half-second
timing window with a 45-unit value window. That is a driving change, not a
frame-perfect trick.

## Files

| file | what |
|---|---|
| `replays/tas_4492_v1.Ghost.Gbx` | the run |
| `replays/tas_4493_singletick_v1.Ghost.Gbx` | the 4.493 variant, one steering tick different |
| `replays/ablation_early_only_4493.Ghost.Gbx` | the human record with only the early lock added — 4.493 |
| `replays/ablation_exit_only_4495.Ghost.Gbx` | the human record with only the exit release — still 4.495 |
| `inputs/tas_4492_v1.inputs.csv` | per-tick inputs |
| `inputs/human_wr_4495.inputs.csv` | the human world record's inputs, for comparison |
