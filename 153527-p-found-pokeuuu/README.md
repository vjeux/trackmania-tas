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
**one recorded run** (board 2026-08-24, unchanged).

No run has been driven on this map here, and no time is claimed. What follows is
what the single recorded run shows about where the time is.

> **"No clip" here is correct, and it is not a filming defect** (checked
> 2026-08-24 while the other no-clip pages were being repaired). The page claims
> no time of ours, so there is nothing of ours to film. The map itself is not
> the obstacle — it is a 1.8 MB Stadium map that declares nothing unusual and
> would open like any other. A clip here needs a **run**, not a render.
>
> The two spliced files below are AuwrahTM's recording, not ours, and neither is
> a lap; see what they are not.

## The two spliced files, and what they are not

| file | time | what it is |
|---|---|---|
| [`HUMAN_RECORD_retries_cut_1214545`](replays/HUMAN_RECORD_retries_cut_1214545.Ghost.Gbx) | **1214.545** | AuwrahTM's recording with every failed attempt deleted — one attempt per segment, the one that reached the checkpoint |
| [`HUMAN_RECORD_retries_and_loops_cut_941395`](replays/HUMAN_RECORD_retries_and_loops_cut_941395.Ghost.Gbx) | **941.395** | the same, with the seven closed loops of the junction census also deleted — **2.112 over the author time** |

**Neither is a lap and neither should be read as one.** They are AuwrahTM's own
driving with material removed, published as his recording, the way
`227654`'s `HUMAN_WR_retries_cut_64871` is. The car's state jumps at every
junction, so the plain oracle re-simulating either of them returns **DNF** —
and so does the *unmodified* record, which is the control: that recording has
never re-simulated (`fst_RESULT_v1` traces the divergence to a step at two
head-on wall contacts at 101.930 and 137.090). A splice cannot be validated by
the oracle on this map, and nothing here claims it was.

### The rule, so the files can be re-derived

`ghost splice --rule retries`, in `tools/ghost/src/splice.rs`:

> Let the recording declare crossings `s_1 < … < s_n` (its splits; `s_n` is the
> finish), and let segment *k* hold the ticks with `s_{k-1} < t ≤ s_k`
> (segment 1 also holds the countdown). Let `a_k` be the **last** tick of
> segment *k* carrying a respawn press — bit 31 of the input packet's state
> literal. For every segment that has one, delete that segment from its start
> through `a_k` inclusive. Delete nothing else; shorten, reorder and edit
> nothing that survives.

Nine of the twelve segments contain a respawn; 444 679 ticks come out, and the
new time is exactly `5661.335 − 4446.790`. The output carries **zero respawn
ticks**, which the command asserts before it writes.

```
ghost splice rank00001_5661335.Ghost.Gbx HUMAN_RECORD_retries_cut_1214545.Ghost.Gbx \
      --rule retries --driver-only
ghost splice rank00001_5661335.Ghost.Gbx HUMAN_RECORD_retries_and_loops_cut_941395.Ghost.Gbx \
      --rule retries --driver-only --drop \
      296650..308742,786297..798155,2907920..3010780,4725505..4762959,\
4763768..4778158,5093441..5169573,5181648..5200007
```

The seven `--drop` intervals are the seven junctions of the ≥ 5 s row of the
minimum-junction census, quoted from it unchanged. The de-looped figure the
census reports is **941.588**; deleted tick-exactly out of the file it is
**941.395**, because the census works on a line resampled to 0.10 m and charges
a junction fee (0.103 s of its total) and this deletes whole 10 ms ticks. Both
are the same seven loops. **The older 892.148 de-loop figure is withdrawn and is
not what these files are.** The retries-cut figure the census reports is
**1214.465** against this file's **1214.545**; the difference is 0.080 s and it
is which side of the respawn tick the boundary falls on, summed over nine
junctions.

### What the junctions cost, measured rather than asserted

At a retry junction the car jumps by the respawn's own landing error — the
checkpoint crossing on one side, the car as the game put it back on the other:

```
  junction        jump        junction        jump
    69.770      25.78 m         929.550     32.84 m
   224.490      17.20 m        3075.480     51.94 m
   322.980       2.41 m        4851.390     15.02 m
   504.230      19.30 m        5231.390     28.53 m
   605.850      21.34 m
```

The seven loop junctions are an order of magnitude tighter — **0.44, 0.66, 1.72,
1.77, 2.10, 3.23, 3.68 m** — which is the census's own claim (0.16–0.25 m on its
resampled line) confirmed on the 50 ms sample grid. A loop cut closes; a retry
cut does not, and cannot: the respawn moved the car.

### Which car is in the file

This recording holds **55 entities**, and the stock reader — take the
`CSceneVehicleVis` with the most samples — returns **another player on the
server**. The driver is **46 short entities** tiling the race at 10 ms, because
his car is destroyed and recreated at every respawn. `ghost record chain`
recovers him from the tiling and the path length (a spectator car parked at the
spawn tiles perfectly for the whole race and travels zero metres): **46 lives,
113 281 samples, 136 069 m**, which is the figure `route_RESULT_v1` reached
independently by a checkpoint-cell referee. Both files are written
`--driver-only`: the 46 lives merged into one entity, the other three cars
dropped.

That repair is visible in one number. On the unmodified record, tape/telemetry
agreement is **κ = −0.004** (the reader is describing the wrong car). On both
spliced files it is **κ = 0.995 / 0.994, lag 0, 99.7 % / 99.6 % of samples
exact** — the recording in the file is the tape in the file.

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

Two files are published (above): **AuwrahTM's own driving with the retries cut,
and with the retries and the seven loops cut.** Neither is a lap. No TAS replay
exists for this map.

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
