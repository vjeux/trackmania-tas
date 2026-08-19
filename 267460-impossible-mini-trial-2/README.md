# Impossible Mini Trial 2 — the human record falls by 1.150 s; the author time does not

**Author time 16.888 · the only human record 23.068 · best validated 21.918.**

| tape | validated | vs human WR | vs AT | steer values | input events |
|---|---|---|---|---|---|
| [`TAS_21918_analog`](replays/TAS_21918_analog.Ghost.Gbx) | **21.918** | **−1.150** | +5.030 | 214 | 515 |
| [`TAS_22290_thinned`](replays/TAS_22290_thinned.Ghost.Gbx) | 22.290 | −0.778 | +5.402 | 31 | 84 |
| [`TAS_22698_lowinput`](replays/TAS_22698_lowinput.Ghost.Gbx) | 22.698 | −0.370 | +5.810 | **10** | **78** |
| author time | 16.888 | — | — | — | — |
| human WR, Wirtual *(control)* | 23.068 | — | +6.180 | 3 | 87 |

TMX map [267460](https://trackmania.exchange/maps/267460) · uid
`KLiIUnR3oNZTnfJwL3GImI1VtOl` · **exactly one recorded run** · 1 checkpoint.

**Not submitted to any Nadeo leaderboard, and it never will be.**

*Counting convention: an input change event is any tick where steer, gas or brake
differs from the previous tick, counted over the whole tape including the
pre-start ticks. Two agents measured this tape and got 30/82 and 31/84 with
different rules — the numbers above are one ruler, applied consistently.*

---

## "Trial" is a building style here, not a respawn mechanic

The obvious read of a 23-second run on a map called *Mini Trial* is that most of
it is retries, the way it was on
[`[Turtle Trial] Leto`](../286279-turtle-trial-leto) and
[`YOU LOVE WATER`](../284238-you-love-water). It is not.

```
human WR:  NbRespawns 0     NbCheckpoints 1
```

**One checkpoint** means the only waypoints on the map are the Spawn block and
the Goal gate. There is nowhere to respawn *to* except the start, with the clock
running. There are no retries to delete, and the entire retry-cutting family of
techniques is inapplicable.

> **Check `NbCheckpoints` before assuming a trial map's time is mostly retries.**

## Where the 1.150 s is: the last ten metres

Nine of the map's 23 seconds are a crawl through tilted dirt platforms and four
more are the endgame. **All of the time we found is in the endgame.** The human
brakes from 20.0 s onward through a no-engine gate and crosses the line at
8.5 km/h, taking **0.829 s for the last 10 m**. Ours takes **0.258 s**.

The engine is dead after (1056, 49, 672), so any braking before that point is
unrecoverable — you cannot accelerate back. The whole trick is to arrive at the
kill-line carrying speed you are willing to keep.

That is also the honest answer to "how would a human drive this": the
low-input tape gets within **0.370** of the world record on **ten steer values**,
against the record holder's three. This part is teachable.

## Three routes to the author time that do not exist, all measured

The author time needs another 5 s, and 5 s on this map means a different route,
not a better line. Three were enumerated and closed:

1. **Fly through the flag mid-dive.** The corridor stalls against the z = 686
   wall after 414 000 evaluations. 49 tapes do get through the low doorway and
   then hit nothing — because the doorway is at x > 976 and the flag is at
   x = 990.
2. **Drop off the start platform into the turbo gate.** 2 600 enumerated tapes,
   0 hits; a second agent reproduced independently with 882 more.
3. **A faster line through the pit.** Closed in `RESULT.md` §5.

Plus, from the independent verification pass: **upward launch is closed too.**
The y = 136 panel row at z = 686 covers only x ∈ [912, 1008], so a crossing above
y = 120 west of x = 912 is geometrically open — 1 900 programs across six dive
depths and five steer values reach **none** of the detectors. The flat ramp does
not produce upward velocity.

## The finding worth more than the time: a negative needs a positive control

The slope-route negative — *"0 of 5 940 launch-sweep tapes reach any gate on the
finish platform"* — was measured with gates at (1005, 50, 665), (1012, 50, 660)
and (1000, 52, 668). The second agent checked the cheapest possible thing first:
does the **finishing 21.918 tape** fire those gates?

```
21.918 tape vs (1005,50,665) -> DNF     (1012,50,660) -> DNF
            vs (1000,52,668) -> DNF     ( 996,56,690) -> DNF
```

It does not. A y-sweep at the same x explains why, and confirms the trigger
model rather than overturning it:

| gate at x = 1005 | y window | 21.918 tape |
|---|---|---|
| y = 46 | [40, 46] | DNF |
| y = 50 | [44, 50] | DNF |
| **y = 54** | **[48, 54]** | **21.546** |
| y = 58 | [52, 58] | DNF |

The car crosses x = 1005 at y ∈ (50, 52), and only a gate at y = 54 brackets it.
The original gates at y = 50 and y = 52 sat **on either side of a 6 m window
without containing it** — about four metres out, which on this trigger is the
whole window.

> **5 940 tapes reported "nothing reaches the platform", from an instrument that
> answers DNF to the tape that demonstrably drives across it.**

Re-run with detectors proven able to say yes (y = 54, z = 656; the 21.918 tape
fires all four), the negative **survives**: 2 × 720 programs per detector,
5 672 hits, and **0 arrivals earlier than the incumbent's own**. But it is now a
negative about *perturbations of this line*, not the sweeping claim that nothing
can reach the platform early. That distinction is the whole point.

No blame attaches to anyone here — the first agent disclosed the flaw in their
own gate tool unprompted and invited the re-run, which is how this was caught
within minutes rather than being published.

## Two more traps this map produced

**A relocated gate is only a valid search *objective* if reaching it implies the
route.** Otherwise it is a *probe*, valid only on tapes independently known to be
on the route. This map produced three separate false positives from that one
mistake — every one a car falling through a gate's 14 m half-width on the wrong
side of a screen. Cross-check each new hit against a second gate the same tape
must also fire.

**The item y cell is `floor(y/8) + 8`, not `floor(y/8)`** — the map's vertical
origin sits 64 m below y = 0. A wrong y cell still loads and still usually fires,
which makes a relocated gate built with it a silently inconsistent instrument.

## What the map is made of

22 of its 31 blocks are `CanopyCenterFlatBase` — stadium screens rotated vertical
into two solid walls at z = 740 and z = 686. Every route question on this map is
"which hole in which wall". Free-block world positions live in chunk
`0x0304305F` (pos + rot, 24 bytes each, the first N records in block order); the
block record itself carries cell (−1, 0, −1) and tells you nothing.

The map header says `validated="1"` but there is **no embedded ghost of any
kind** — checked properly, by decompressing the LZO body and scanning for
`0x0911F000` / `0x0309201D` / `0x0303F005`, not by trusting the header. With one
human record and no author ghost, the container cannot settle where 16.888 came
from, and there is no field to cross-check it against.

## Where this leaves it

**16.888 does not decompose into any launch + flight + endgame either of two
independent agents can build.** Best construction ≈ 21.3; best actual 21.918.
Two live possibilities remain: a route neither of us found, or an author time
that was not driven — unbeaten.at reports `inPlugin: true` for this map, which is
a reason to spend an hour on provenance before spending another day on search.

Priority for whoever picks it up:

1. **Re-run the remaining route negatives with yes-controlled detectors.** The
   broken-detector failure was live in this map's evidence base for a whole
   session. The hole-A doorway measurement and the aim ceiling have never been
   re-measured with an instrument proven able to say yes.
2. Settle the z = 686 screen's real extent by driving a slow tape into it and
   bisecting, rather than inferring it from nulls.
3. Treat the author time's provenance as an open question.

## Notes

* [`RESULT.md`](notes/RESULT.md) — the full session, all three closed routes, and the
  new tools (`tmgen`, `tmsearch --ladder`, `mutate::redrive`)
* [`GEOMETRY.md`](notes/GEOMETRY.md) — the block/item layout and the wall holes
* [`VERIFICATION_v1.md`](notes/VERIFICATION_v1.md), [`VERIFICATION_v2.md`](notes/VERIFICATION_v2.md) —
  the independent re-measurement and the broken-detector finding
