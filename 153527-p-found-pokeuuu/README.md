# P-Found - Pokeuuu — not a target, and the reason is not the one we first published

**Author time 939.283 · the only human record 5661.335 · no TAS time claimed.**

TMX map [153527](https://trackmania.exchange/maps/153527) · uid
`4ympwQ3XZfX8balg2UcVJBL_pnf` · author **PokeuuuTM** · RPG / Pathfinding ·
44 388 blocks, 12 waypoints (11 checkpoints + the goal) · **one recorded run**.

**No search was run on this map and no time is claimed.** It is published because
three of the things that happened while ruling it out are worth more than the map
is, and because "we looked and here is why we stopped" is part of an honest
index.

---

## Why it is not a target

The one human record is 1 h 34 m against a 15 m 39 s author time — a 6.03×
ratio. On an RPG/pathfinding map that normally means retries, and retries can be
deleted. Here they cannot get you far enough: **the retry-deletion floor is
1 214.585 s against an author time of 939.283.** Cutting every failed attempt out
of the only run in existence still leaves you 275 s slow.

There is also nothing to seed from. The single record does not re-simulate — the
plain oracle returns a bare `wrong simu` — so there is no finishing tape on this
map, human or machine, to build a search on.

For context, this author's other maps do not behave like this at all: on all
eight siblings with a live board, humans beat the author time, usually by a lot
(0.33× to 0.98×). 153527 is the outlier in their own catalogue.

## Finding 1 — the published telemetry was another player's car

This is the one that costs other people time, and it invalidated an entire
earlier analysis of this map before anyone noticed.

`tmtraj decode` picks **the entity with the most samples**. On a ghost where the
driver respawns repeatedly, their own car is split across many
`CSceneVehicleVis` entities — one per life — while some *other* recorded entity
can hold more samples than any single fragment of theirs. So the decoder
confidently returns a complete, plausible, smooth trajectory belonging to
somebody else.

Decoding per-life and merging the driver's own fragments is what fixed it. With
the driver's real line in hand, de-looping it gives **892.148 s against an author
time of 939.283** — i.e. **the author time is that same line with the loops cut
out**. The route was never mysterious; the decoder was.

> **On any ghost with respawns, merge per-life tracks before you read a
> trajectory. A confident, smooth, complete decode is not evidence that it is the
> right car.**

## Finding 2 — the map is healthy, and the experiment that "proved" otherwise was a false negative

The standing worry was that something about this map or this ghost defeats the
simulator. It does not. **Six independent relocated-finish placements returned a
real `ValidatedResult` block from this map** — 8.729, 531.122, 1049.307,
1064.969, 1242.529, 2716.208 — against a declared time of 5661.335 in every one
of them. A validated block carrying a number the tape does not contain cannot be
an echo.

The briefed experiment — relocate the finish onto the car's early path and see if
it fires — **would have produced a confident, published, wrong answer**, because
it fails on a *healthy* map too:

```
152940 (sibling, validates perfectly)   untouched              886.277, IsValid true
152940   Goal moved to a cell its own car crosses at 1.850 s   null, "wrong simu"
```

The mechanism, once seen: **a finish only counts once every checkpoint has been
collected.** The recipe was invented on
[165922](../165922-idm-ruinin-ur-day-460), which has a spawn, a goal, and *no*
checkpoints. 153527 declares eleven. Move only the goal and the car crosses it
having collected nothing.

Stacking all eleven checkpoints into a single early cell — legal in the file
format, the map loads, and all eleven triggers fire — is what made the map
debuggable for the first time.

## Finding 3 — this probe's "yes" is worth everything and its "no" is worth nothing

Two adjacent cells, 1.5 s apart on the recorded line, the car within 2 m of each
centre and sitting on the surface:

```
96.180 s   cell (26,20,12)   mindist 1.2 m  ->  11 of 12 checkpoints   FIRES
97.680 s   cell (26,20,13)   mindist 1.7 m  ->  bare "wrong simu"      DOES NOT FIRE
```

Roughly a third of well-chosen placements simply do not trigger. (The reason was
found later, on [146612](../146612-spaghetti-nights-2): a relocated gate is a
**plane**, and a byte in the block record picks which axis. A silent rung is
usually a gate the car ran parallel to.) There is also no "hang it underneath
where it cannot obstruct" trick — a gate 8 m below the driving surface does not
fire.

Two more confounds, both found the hard way:

* **A relocated checkpoint leaves a hole in the track.** Its block is load-bearing
  road. Moving all eleven produced a beautiful, entirely fictitious "faithful to
  11.23 s, nothing after" cliff, because CP1's block is a surface the car drives
  on at 11.230 s. Leave in place every checkpoint whose real crossing precedes
  the probe time.
* **The probe is one-shot.** A relocated checkpoint ends the useful part of the
  run at that cell, so each rung measures faithfulness up to itself and tells you
  nothing past it.

Used correctly, the probe says: **the one human record re-simulates faithfully
for at least 96.180 s** of its 5 661.335 — through two real checkpoints and one
respawn. Whatever breaks it is a localised event later in the tape, not a
property of the file. That is an ordinary debugging problem now, which is a much
better place to leave a map than "unsimulable, therefore unknowable".

## Notes

* [`RESULT.md`](notes/RESULT.md) — the verdict and the three reasons
* [`VALIDATION_STUDY.md`](notes/VALIDATION_STUDY.md) — the first `ValidatedResult` ever obtained
  from this map, with the false-negative control that made it possible
* [`ROUTE.md`](notes/ROUTE.md) — the de-looped route and the wrong-entity finding
* [`SEED.md`](notes/SEED.md) — the sibling-map seed hunt (closed here; it worked on
  [284238](../284238-you-love-water))
* [`GHOST_ENTITY_SELECTION.md`](notes/GHOST_ENTITY_SELECTION.md) — the decoder defect, written up for the fleet
