# Impossible Mini Trial 2 (TMX 267460)

**Author time 16.888 · World record 23.068 (Wirtual, the only record ever set on this map) · Our best 21.022**

This is **one of only two maps in this repo where we have not beaten the author
time.** We are 2.046 under the human world record and still **4.134 over the
author time**. This page is the best account anyone has of why — including a
technique we found tonight that saves five and a half seconds and that we still
cannot convert into a lap.

**Impossible Mini Trial 2** — TAS **21.022** (+4.134) | AT 16.888 | WR 23.068 by Wirtual

> ### ⚠️ Video withdrawn — the tape imports into the game under Wirtual's name
>
> The clip that was here has been taken down. Nothing about the run is wrong: the
> 21.022 is validated, the car path is regenerated from engine memory and
> accurate to half a millimetre, and the driving is ours.
>
> What is wrong is whose file it is. Loaded into the game, the tape announces
> itself as **`Ghost:WirtualTM`** — the same name as the human world record on
> this map, whose recording the tape was built inside. Our own files announce
> themselves as `Ghost:TAS`.
>
> The shape of it is worth stating, because it is not bad luck. This map has four
> tapes; the three nobody filmed all read `Ghost:TAS`, and **the one made
> specifically for filming is the one carrying his name** — because the watchable
> version is the *regenerated* one, and a regeneration rewrites the car's
> telemetry while inheriting every other field of the container it was built in.
> The file most likely to be published is therefore the file most likely to be
> wearing somebody else's identity.
>
> A replacement will be rebuilt on a clean carrier and re-filmed. The other tape
> on this map that is already clean, `TAS_21918_analog`, is a **different run** —
> 0.896 slower — and is not a substitute for this one.


---

## The map

A chain of floating platforms threaded through two enormous stadium screens.
One checkpoint (the finish), no respawns — "Trial" here is a building style, not
a checkpoint mechanic, so a respawn sends you back to the start with the clock
running.

The lap has three parts:

1. **The ice run (0 – 4 s).** You spawn on a turbo pad and are fired west along
   a strip of ice at 143 km/h. You do not get a choice about this: we tried
   twelve maximally different opening programs — every combination of full left,
   full right and straight, with gas and brake on and off — and *all twelve*
   cross the same point 9 m away between **0.422 and 0.455**. The human is at
   0.422. The first half-second of this map is the same whatever you press.
2. **The pit (4 – 13 s).** You fall off the west end onto tilted dirt. Wirtual
   descends, U-turns at the bottom, climbs back north, U-turns again at the top,
   and only then charges east — **nine seconds, of which about seven net zero
   displacement.** He is not being slow: the second descent is where his run-up
   speed comes from, 58 → 155 km/h in 1.6 s.
3. **The launch and the flight (13 – 23 s).** Through a big turbo gate at
   155 km/h, off a ramp, a long dive to 268 km/h, land on the grass, U-turn,
   come back west past a no-engine gate, and coast into the flag. Wirtual
   crosses the line at 8.5 km/h.

Our 21.022 drives Wirtual's route and wins in the last six seconds — mostly by
**not braking into the finish**. He is on the brake from 20.0 s and takes 0.829
for the final ten metres; carrying speed instead takes 0.258.

## The dirt ice flick — the pit's nine seconds are not forced

Everything above is the *route*, and the route really is forced (see "Two things
that are genuinely closed"). But **being forced to drive somewhere is not the
same as being forced to take nine seconds doing it**, and that is where this
map has been hiding.

The car can be **flicked** across the first dirt turn instead of looping around
it. Measured against three checkpoints along the eastward deck, required in
order so nothing can score by falling past them:

| | first deck marker | second | third (x = 767) |
|---|---|---|---|
| Wirtual | 12.954 | 13.297 | 13.560 |
| our previous best line | 12.254 | 12.652 | 12.933 |
| **the flick** | **6.119** | **6.848** | **7.283** |

**5.650 seconds earlier to the same point**, and it is not a fluke: 415 of
24,300 candidate runs arrive there in order.

### Why it is a flick and not a bug slide

A Trackmania player who saw our first write-up corrected us: you cannot bug
slide here, because the car is on **icy wheels**. We checked that against the
game rather than taking it on trust — the map's own recording carries per-wheel
surface state, and this is it:

| race time | ice, four wheels | dirt | where the car is |
|---|---|---|---|
| 2.0 s | **1.00 — saturated** | 0.00 | on the ice run |
| 4.6 s | 0.46 / 0.56 / 0.44 / 0.56 | 0.000 | airborne off the west end |
| **4.7 s — the contact** | **0.44 / 0.54 / 0.43 / 0.55** | **0.008** | **the flick** |
| 6.5 s | 0.01 – 0.09 | 0.11 – 0.16 | pit floor |
| 14.0 s | 0.00 | **1.00** | the deck |

**Ice on the wheels, dirt underneath.** He was right, and now it is measured
rather than asserted.

One thing that falls out of the table and that we have not seen written down
anywhere: **ice is a decaying clock, not a state.** It saturates by 2.0 s and
bleeds away to nothing by 6.5 s, roughly 0.2 per second. So how icy your wheels
are when you hit the dirt depends on *when* you hit it.

### The gain is route, not grip

It would be a nicer story to say the flick keeps your speed through a corner
that should scrub it. **It does not, and here is the measurement that says so.**
Speed carried through the contact, read from engine memory across our 200 best
candidates:

| | before contact | after (5.3 s) | lost |
|---|---|---|---|
| **Wirtual** | 143.6 | **77.9** | 65.7 |
| **best of our 200** | 147.6 | **79.7** | 67.8 |
| our median | ~146 | ~55 | ~91 |

The best flick we have keeps **1.8 km/h more** than Wirtual keeps through his
own contact. He scrubs just as hard as we do. The five and a half seconds come
entirely from **not driving the two loops** — it is a shortcut, not a grip
trick.

## Where it stops

Skipping the loops also skips the drop that pays for them, so we arrive at the
launch **50 km/h short**. Wirtual converts 78 → 175 km/h in 6.7 s using two
loops and a 15 m descent; the flick converts 78 → 119 in 2.3 s with 10 m to work
with. Two ways out, both measured shut:

**The lower platforms.** There are two dirt platforms 16 m below the deck that
**no run in this project had ever touched**, and the fast flick line runs
straight over them at **157.8 km/h** — 5 km/h off Wirtual's speed, 5.5 s
earlier. The energy is there. What is missing is a way back up: those platforms
end at x ≈ 784 and the next drivable surface east is
`OpenTechRoadSlope2FCLeft` at x ≈ 816, so the return is a **32 m gap that has to
be crossed while climbing 7 m — departing from a surface rolled 30° downward.**
At 157.8 km/h the crossing takes 0.73 s and gravity alone costs 6.4 m; you
arrive about 13 m low. **Zero of roughly 135,000 evaluations reach the far
side.** (A marker 16 m further east, built the same way in the same batch, fires
normally — so the test can succeed; this route just never gets there.)

**Under the flag instead of over it.** The fast line crosses the flag's exact
height at z = 653 against the flag's z = 656 — dead on in two coordinates — but
at **x = 826 against the flag's x = 990**. Closing 164 m at that height needs
about **416 km/h**. The fastest anything has ever gone on this map, in any run
we or anyone else has produced, is 294.7.

## Two things that *are* genuinely closed

**You cannot leave the start going east.** There is a large hole in the first
screen east of the spawn, and an ice tile pointing at it, and it would be worth
about ten seconds. It is unreachable: the spawn turbo fires you west and the
first 0.45 s is input-independent, so 3,840 turn-around programs and a detector
wall two metres from the spawn produce nothing.

**You cannot finish out of the air.** We re-measured the second screen by
sliding it across a run that crosses it exactly once (1,685 real validations)
and mapped where it is solid. The only opening a launch can reach puts the car
*past* the flag — which is why every run on this map, ours and Wirtual's, lands
on the grass, U-turns, and comes back.

## A retraction

An earlier version of this page said the author time was **below this map's
physical floor** — that even granting a wall that exists and a prefix that
cannot continue, the best conceivable lap was 17.102 against 16.888.

**That was wrong, and we broke it ourselves within the hour.** The physics was
fine. The *premise* was not: "the best ramp exit ever measured is 15.014" was a
report on five sessions of searching, not a property of the map, and the flick
beat it by 5.650 s.

There were two errors, and both generalise:

* **A bound whose premise is a search record is a report on the search, not on
  the map.** If a floor rests on "the best anyone has measured", it has to say
  so in the same sentence, or it should not be stated at all.
* **"The route is forced" and "the route's *timing* is forced" are different
  claims.** We proved the first and quietly acted on the second. The pit is *on*
  the forced route — which is exactly why nobody had ever priced its nine
  seconds.

## What is left

We can estimate what a perfect run-up would be worth, **and this is an estimate,
not a bound** — its inputs are speeds and times we have measured, which is
precisely the kind of premise that just failed above. A 2.5 s run-up returning
Wirtual's conversion rate puts the deck arrival near 9.8 s at 175 km/h, and our
existing ending from there is 8.09 s: **about 17.9.** Three seconds better than
our published run, and still over the author time.

So the open question is a single number:

> **A deck arrival at ≈ 8.8 s carrying ≈ 175 km/h.**

Get that and the author time is in range. Every other part of this lap has now
been measured.

## What is wrong with this video

The run in the clip is real — validated by the game's own simulator, three times
cold. But a Trackmania ghost stores its inputs and its *telemetry* separately,
and a searched run is built inside a donor file, so the raw file plays back as
somebody else's run entirely. We regenerate the telemetry from engine memory to
fix that, and the car path in this video is accurate to **half a millimetre**.

One byte still is not ours: the surface-contact flag. Our own publish check
refuses this ghost because of it (contact reads "on" for 15 of 39 samples where
the car is provably airborne), while the map's downloaded human recording passes
every check. So the *driving* you see is exactly what the simulator validated,
and some of the *effects* — dirt spray, sparks, wheel behaviour at the edges —
are not. Three independent attempts to decode that byte have failed; we would
rather ship the video with this note than quietly fit a value that looks right
on one map and is wrong everywhere else.

## Files

| file | what |
|---|---|
| `replays/TAS_21918_analog.Ghost.Gbx` | the fastest tape |
| `replays/TAS_22290_thinned.Ghost.Gbx` | the same line at 84 input changes |
| `replays/TAS_22698_lowinput.Ghost.Gbx` | **ten steering values — the one worth studying** |
| `inputs/m267460_TAS_lowinput_76inputs.script.txt` | the low-input run as a readable script |
| `inputs/m267460_TAS_thinned_82inputs.script.txt` | the thinned run |

TMX map [267460](https://trackmania.exchange/maps/267460).

**Not submitted to any Nadeo leaderboard, and it never will be.**
