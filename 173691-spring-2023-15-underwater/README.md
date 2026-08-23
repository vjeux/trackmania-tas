# Spring 2023 - 15 (Underwater) — the jump lands

**GothMommyTM's underwater jump was measured at 4.1 m short of the stadium's
lower canopy. It isn't. A search over his own flight inputs closes the gap: the
car crosses the deck plane at z = 448.2 and comes to rest on the deck at
y = 114.0, stationary, 0.6 km/h.**

TMX map [173691](https://trackmania.exchange/maps/173691) by **Reddnox** ·
3 checkpoints · author time **2672.290** · **0 online records**.
**The map is not beaten and this does not beat it** — see "not a win" and
"which map this is" below.

**Spring 2023 - 15 (Underwater)** — TAS **36.049** (not a completion — a landing) | AT 2672.290 | WR — by nobody (0 online records)

https://github.com/user-attachments/assets/301f3c33-ae38-4ffd-9c31-8f709bf257d0

Two cars, one scene, camera on ours: the TAS car and GothMommyTM's own
demonstration run, which leaves the same lip at the same speed and sinks past
the deck into the water. They are the same car until race 25.750, because every
input up to that tick is his.

## What the jump is

The route is GothMommyTM's and the credit for it is his. The map is driven
underwater; at race **25.80** the car leaves the end of a banked curve at
(1311.8, 138.2, 386.4) doing 101 km/h and glides. Water drag takes the speed
within about four seconds and after that the car only sinks, at a terminal
2.7 m/s, to the stadium floor at y = 79.

Ahead of that lip, the first solid thing is the stadium's **lower canopy**: a
flat deck at **y = 114.16**, whose solid cells begin at **z = 448, x ≥ 1312**
(`CanopyCenterFlatBase`). The ring of `CanopyCenterFlatHFC` one cell nearer is
**not solid** — his flight passes straight through it.

So the whole jump is one number: **the z the car has reached at the moment it
falls through deck height.** His demonstration reaches **444.41** and the
landing threshold is **448.5 ± 0.4**.

| | z at deck height | outcome |
|---|---|---|
| GothMommyTM's demonstration | 444.41 | sinks to the floor |
| 126 one-move perturbations of it (earlier pass) | 442.9 best | all worse |
| **this run** | **448.2** | **lands, and stops** |

What closes the 4.1 m is not a cleverer move but **rewriting the whole input
stream from the lip onward**: sixteen hill-climbers, ~40k evaluations, scored
first on how far the crossing point misses the deck footprint and then on
**contact time** — how long the car goes without descending. Contact time is
what makes the last two metres a hill to climb: the first candidates to touch
the deck landed on its very lip and slid back off after 0.6 s, which a yes/no
landing test scores exactly like a clean miss.

Touchdown, from the re-simulated file:

```
 t=36.10   y 114.39   vy -2.67     still falling
 t=36.20   y 114.13   vy -2.60
 t=36.30   y 113.95   vy -0.28     <- contact
 t=36.40   y 114.01   vy +0.25
 t=36.60   y 114.06   vy -0.13     at rest, 0.6 km/h
```

## WHICH MAP THIS IS — read this before quoting the result

**The run is on GothMommyTM's own copy of the map, not on Reddnox's 173691.**
A `.Replay.Gbx` carries a whole map inside it and the engine simulates *that*
copy; the `--map` argument and the uid in the header are decoration. Measured
directly, with `ghost map extract`:

| | Reddnox's 173691 | the map inside the replay |
|---|---|---|
| uid | `D0KdisOjKSxSIAXawtwlBqLz9Kb` | `7FoTPm93enV5nhdsbD9u7D5Zqcm` |
| author | `UNgaAHUXR-GoQnjEpzf2RA` (Reddnox) | `3Awx2_MzSdaCJZjZOht51A` (GothMommyTM) |
| blocks | 68 166 | 68 155 |
| spawn | (1136, 18, 736) | **(752, 98, 400)** — his own start block |
| Goal blocks | 15 `GateFinish` | those **plus 4 `GateExpandableFinish`** |

He added a start a few hundred metres before the launch so he could practise
the jump, and a finish gate so the game would let him save a replay. **That
gate is what stops the clock at 36.049 — 0.15 s before the car reaches the
deck** — so the touchdown is the last 0.75 s of the recording, *after* the
timer stops.

What survives that, and what does not:

* **The landing surface is real.** `CanopyCenterFlatBase` and
  `StructurePillar`, all 1106 rows of them, are byte-identical between the two
  maps. The car rests at **y = 113.98–114.06**; the deck's drivable surface was
  measured at **114.16** on the untouched map by dropping a car down 35
  separate columns, and the only other surfaces anywhere in that region are the
  stadium floor at 9.16 and the upper canopy at 170.16. His gate's own pieces
  sit at y = 105, 113 and 121.
* **"The jump lands on the unbeaten map" is NOT supported.** His inputs on
  Reddnox's map put the car on the stadium floor within six seconds, because
  the start is 400 m away and 80 m lower. Reaching this lip from 173691's own
  spawn is an open question, and there is a lead: a partial TAS of the
  untouched map passes **1.73 m from his start block at race 223.920, doing
  97.0 km/h**, drives the same last road, and falls off at x ≈ 1269 instead of
  following the curve round to z = 386.

## Not a win — the finish is 15.7 m above this deck

**Corrected 2026-08-22.** An earlier version of this page said the finish was on
the upper deck at y ≈ 163–169 and that the lower canopy was sealed, on the
strength of 0 finishes in 2 400 fuzzed tapes. That framing is retired: the
finish is far lower than anyone had recorded, and the null was a search result,
not a structure.

What is measured now (see [`ENDGAME-MEASURED.md`](ENDGAME-MEASURED.md)): of the
15 `GateFinish` blocks, five are the START wall and **ten are finish gates
spanning y 130…194, and both rows fire**. Thirteen firings put the trigger on a
plane at **z = 494.5 ± 0.6**, the lowest at **y = 133.97**. There is a drivable
ledge at **130.2 → 134** inside the gate slot, and driving it finishes the map.

So the deck this jump reaches at **114.16** is not the wrong storey by 50 m. It
is **15.7 m** below a finish that fires. The map is still unfinished because
neither of two independent routes closes that 15.7 m:

* **From the deck.** A complete 456-block census of the volume between the
  decks finds water, *vertical* pillars, the stands' slope (bottom edge 162)
  and the gates — no ramp. 10 100 blind oracle runs, zero finishes; 204 scored
  trajectories never exceed **y = 114.267**. The null is demonstrated rather
  than assumed: twelve non-firing runs sat inside the trigger window at deck
  height for 28–1131 consecutive samples.
* **From the air.** The last road's exit is (1345.4, 154.9, 387.1) and the
  plane is **107 m downrange**. Underwater drag is **linear** (fitted on four
  independent glides), so reach has an **asymptote at 47–59 m** — 62 m
  observed. Short by 46–57 m, and bounded by an asymptote rather than by search
  effort. The nearest turbo is 559 m in a straight line, about 1.1 km by road,
  against a 340 m half-life.

Reaching this deck is still the end of this route, not the start of a lap.

## The file

`replays/TAS_36049_landing.Ghost.Gbx`, with its provenance manifest beside it.
It carries the map it runs on, so it needs nothing else.

**It passes the publish gate, exit 0** (`tmtraj gate … --require-manifest
--route … --source …`), and the two things it does *not* prove are printed as
loudly as the things it does:

| | |
|---|---|
| oracle | the dedicated server re-simulates **the written bytes** to 36.049, the time the file declares, `IsValid` true |
| tape ↔ record | Cohen's kappa **1.000** over all 737 samples |
| engine ↔ record | the engine's own run of this tape matches the recording to **0.0005 m** mean, 0.0206 m worst — the answer key’s own floor (the cause of that floor is UNKNOWN; see `CLAIMS.md`) |
| an independent instrument | `fk trace`, which never reads the record, agrees to **0.0021 m** over 562 instants |
| orientation | the stored quaternion is **0.072°** from the engine's own; a permuted reading of the same bytes gives 166.6° |
| second generation | a second, independent regeneration agrees to **0.000497 m** — the same floor, not a stale buffer. **Note this is ours-vs-ours**, and 270051 reads 0.000000 m on the same comparison, which is why the “client-vs-server” name for this number is withdrawn |
| contamination | **0 of 737** samples bit-identical to GothMommyTM's recording; they part by up to 4.3 m |
| identity | login `TAS`, no account id, no locator URL — in the body **and in the replay header** |
| **not proved** | **the ground-contact byte is still the carrier's.** `ghost regen` writes 22 of each sample's 116 bytes from engine memory and three from the tape; byte 89 is not among them, so the gate's C6 and C10 report UNMEASURED rather than a verdict. Reading it out of engine memory is an open task. |
| **not proved** | the near-copy test (C12) could not run: the only human recording of this route is the carrier itself |

### One tick, and its control

`fk trace` reads the record as one tick early. So does **GothMommyTM's own
downloaded recording**, at 0.0028 m, put through exactly the same comparison —
so −1 is this instrument's zero on this map, not a defect in this file. A
negative result needs a positive control, and that is it.

### The clip and the file

The clip was rendered from a sibling build of this file, made before a fix to
the anonymiser landed. Rather than assert that it does not matter: a render
reads the **record**, and the two files' records are **identical at shift 0,
mean 0.000000 m and worst 0.000000 m over all 737 samples** (`ghost trajdiff`,
which also reports 0.93 m at ±1 sample, so that zero is identity and not a
lag). The clip is a film of the record in the published file.

## Method notes worth keeping

**Distance travelled is not progress.** The first objective maximised how far
the car got from the launch point, and the fleet promptly learned to fly it 166 m
sideways, still sinking, nowhere near the platform. An objective has to name the
place you want, not the amount of movement.

**The block census names the deck but not what is solid.** Two block families
sit at y = 114 one cell apart; one holds a car and one does not, and nothing in
their names says which. The engine settles it in one run.

**A fork-server score is not a result.** Every number above is read off a
written `.Ghost.Gbx` re-simulated by the plain oracle.

**A control can fail after you have already used the instrument.** To ask
whether the car lands on the canopy or on the added gate, three maps were built
— gate removed, deck removed, and a road block the car provably drives on
removed as the control — and re-simulated. All three gave the same trajectory to
the centimetre, including the one that should have dropped the car through the
road. The replay carries its own map, so the surgery never reached the engine
and the two informative runs said nothing at all. The height is what identifies
the surface.

**A container can be ours in every position it contains and still be somebody
else's file — and the header is a second container.** The first version of this
page was filmed from a file that still carried GothMommyTM's account id, his
locator uuid, and his declared time 49.958 — inside the replay's GBX **header**,
where no check in this toolchain looked. `ghost identity set --anonymise` and
`ghost declare --from-oracle` both reported success on it; `ghost verify` V2
said *"1 copies, all 36.049"* while two more copies of 49.958 sat in the header.
A count of a set you cannot see all of is worse than no count.
