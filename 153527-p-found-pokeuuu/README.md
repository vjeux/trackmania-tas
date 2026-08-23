# P-Found - Pokeuuu

**The author time is roughly the one recorded run's own route with the loops
taken out, driven at that human's own pace. It is reachable — the barrier is
being able to drive this map cleanly at all, not finding a shorter way round
it.**

| | time |
|---|---|
| Author time | **939.283** |
| The only human record — AuwrahTM | 5661.335 |

TMX map [153527](https://trackmania.exchange/maps/153527) · author
**PokeuuuTM** · RPG / Pathfinding · 44 388 blocks, 11 checkpoints and a goal ·
**one recorded run**.

No run has been driven on this map here, and no time is claimed. What follows is
what the single recorded run shows about where the time is.

## What the one recorded run is

It reads 1 h 34 m, but it is not a lap at that pace. It is 110 respawns spread
over the race, all of them back to checkpoints, with the clock running through
every failed attempt. The driver **clears CP8 at 929.549 — inside the author
time** — and then spends 65 minutes on the next two segments.

Deleting every failed attempt and keeping only the surviving one in each segment
leaves **1214.465 s of real driving**, still 275 s outside the author time. So
retries alone do not explain the gap, and neither does pace: that surviving line
averages 72.8 km/h across 24 546 m, and it is genuinely driving rather than
milling about.

## Where the time is: the route

Take that surviving line and cut out every place where the car comes back to a
point it has already occupied — going the same way, at the same speed, in the
same attitude — with a minimum junction size, so that only real loops are cut
and the analysis cannot buy time by splicing. Seven loops of 5 s or more come
out, and what is left is **941.588 s of the driver's own elapsed time**, against
an author time of 939.283.

**So the author time is approximately this human's own route, de-looped, driven
at this human's pace** — 2.305 s over it. The chain time is 457.203 s; the rest
is the cost of the junctions themselves.

That route is reachable, and it is still slow going: it spends **51.950 s under
5 km/h and 165.868 s under 20 km/h** on a line averaging 78 km/h. The difficulty
is in driving the map, not in routing it.

The loops that come out are not measurement subtleties. The three biggest are
closed: 102.860 s and 1 066.7 m ending 16 cm from where it started; 76.132 s and
1 123.8 m ending 17 cm away; 37.454 s and 905.5 m ending 21 cm away. Each one is
a lap of a platform the driver had already cleared.

**And it is concentrated.** Segments 9, 10 and 11 carry 269.710 s of what comes
out. Five of the twelve segments — 1, 2, 5, 6 and 12 — come back with nothing to
cut at all: those are already clean lines, and there is no point studying them.

For context, this author's author times are routinely beaten: on 11 of their
other maps with a live board, a human is faster than the author time, by 2 % to
67 % — including a 1193.844 s marathon of the same kind uploaded four days
earlier, beaten by 25.8 %. **The author time here is unbeaten because one person
has played this map once, not because it is extreme.**

## Driving it

The sector-by-sector guide for this map is not written yet, and with one recorded
run there is not much to compare against. The one thing worth knowing before you
start: the checkpoints sit on Tech, **Dirt** and **Ice** platform blocks, and the
segments that decide the map are the three after CP8.

No replay is published for this map.

## The hill after CP2, and how the driver gets up it

Between the second checkpoint and the third the route climbs a 32 m-wide ice
ramp from y = 106 to the y = 138 deck. It is where every attempt to re-drive
this map has stopped, and for a long time it was read as an energy wall: a car
coasting up the fall line with its engine cut by a gate at the top, stopping
0.11 m short of the next marker.

That reading was arithmetic on the wrong constant. Gravity in this game is
**24.3 m/s²**, measured here from 335 free-fall stretches of the driver's own
recording, not 9.81 — so the "2.4× gravity" deceleration on the climb is
**0.97×**, an ordinary coast, and differencing the car's energy with the right
constant shows the engine **making** energy all the way up, including past the
gate. The run ends in a 22–34 m/s² deceleration over 1.51 m, which is a wall,
not a stall.

**The driver climbs this ramp in switchbacks**, and resolved sample by sample
each traverse is a **full-lock turn of about 1.2 seconds followed by 2.5 seconds
of straight running with the steering back at zero** — holding full lock across
a traverse scrubs the speed the traverse exists to carry. He crosses the ramp
five times between z ≈ 450 and z ≈ 478, at 25–49 km/h, taking 45 s and two
complete failures to gain 32 m of height, each traverse at 8–12° where the fall
line is 20–27°.

Driving that manoeuvre in the simulator gets a car up the hill and onto the
deck. **It collects the map's third checkpoint** — `cps=3` on the unmodified
map through the plain oracle, where the driver's own tape and every previous
attempt return `cps=2`, deterministic over three runs and reproduced on a fresh
copy of the map. The car reaches the deck at 164 s and runs on to x = 1 067,
160 m past the point where every earlier attempt slid back and wedged.

Three checkpoints of eleven. The map is still not driven and the author time is
not beaten. Details, controls and what is still open: `tools/pkz2`, and the
arm's write-up in the store.

## Past the third checkpoint: he does not drive there, he is thrown

The section from CP3 to CP4 is not more of the same driving. It is a **chain of
two turbo gates**: a `GateSpecial8mTurbo` at (904, 153, 438) that takes him from
135 to 188 km/h in a quarter of a second, and a `GateSpecial32mTurbo` at
(880, 138, 481) that holds him at **215–238 km/h** down a 250 m corridor and
throws him into the checkpoint's cell. Nothing on this map has driven at those
speeds.

So the objective for that section is the first gate, not the checkpoint — and
the first gate sits **15 m above the deck the car is driving on**. Scoring on it
takes our best approach from 57.5 m to 21.5 m immediately and then stops
improving, which says the way up to it is a move we have not written yet.
