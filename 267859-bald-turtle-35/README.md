# bald turtle #35

**The last three landings decide this map: arrive at each one presented the way
the fast runs present it, and stop chasing the approach — at the final contact
every radian of roll away from the fast presentation costs about 1.8 s.**

**bald turtle #35** — TAS **10.758** (−0.010) | AT 10.768 | WR 11.059 by Max_heyu

https://github.com/user-attachments/assets/f59172e4-9e8c-4f21-89a7-c10bf3bc9d32

**Both cars, and the whole race is the last half-second.** Ours is magenta,
Max_heyu's world record is the dark car. They run wheel to wheel through the
final landings — never more than **4.00 m apart, mean 1.30 m** — and ours edges
clear only at the line, crossing **0.301** ahead. The camera is bolted to *his*
car on purpose, so the shot stays alive after ours has finished and vanished:
you see us cross, and then you watch him arrive.

https://github.com/user-attachments/assets/71a7f89b-edfb-49e9-9d3c-ac8457e2935c

| run | time | vs author time | vs human WR |
|---|---|---|---|
| **TAS 10.758 — fastest, and the file this directory publishes** | **10.758** | **−0.010** | **−0.301** |
| TAS 10.759 — reproduced on three separately compiled binaries | 10.759 | −0.009 | −0.300 |
| TAS 10.768 | 10.768 | ±0 — ties the author time, which takes the medal | −0.291 |
| TAS 10.769 | 10.769 | +0.001 | −0.290 |
| Keyboard 10.788 | 10.788 | +0.020 | −0.271 |
| TAS 10.859 | 10.859 | +0.091 | −0.200 |
| Keyboard 10.897 | 10.897 | +0.129 | −0.162 |
| Author time | 10.768 | — | −0.291 |
| Human WR — Max_heyu | 11.059 | +0.291 | — |
| Human rank 2 | 11.189 | +0.421 | +0.130 |

TMX map [267859](https://trackmania.exchange/maps/267859) · author **Bald_tm /
BALDFROMSPB** · **21 recorded runs** (board 2026-08-24; the field measurements
on this page were taken over the 19 recorded then).

**Two numbers a millisecond apart, and they are cited for different reasons.**
10.759 has been rebuilt and re-run on three separately compiled binaries;
10.758 has only ever run on one toolchain. **This page cited the 10.759 for
that reason until 2026-08-24 and now cites the 10.758**, because that is the
one with a file: `replays/TAS_10758.Ghost.Gbx` states 10.758 in its own header,
and `tmtraj corpus shipped` reads the 10.759 claim as `FRONT-PAGE-BEHIND`
against it. A reader can download the 10.758 and check it; nobody can download
a 10.759. The reproducibility caveat is not withdrawn — it is the reason both
rows are still in the table — and **UNKNOWN: whether the 10.758 reproduces on a
second toolchain.** Rebuilding it on one of the other two binaries settles it,
and either answer is publishable.

## What the map is

The car is **upside down for the entire run**. It flops from one side to the
other, contact by contact, through fifteen successive inverted landings,
climbing from 10 to 99 km/h, with a fifth of the run spent airborne in 150–300 ms
hops. The last contact before the finish is the only one where the car comes
back near upright.

So there is no "keep it flat" here — flat is not available. What matters is
**presentation**: how squarely the car meets each surface, measured against what
the fastest runs do rather than against level.

## Where the time is

Sectioned against the human world record:

| section | our tape | human WR | we gain |
|---|---|---|---|
| start → about 2.9 s | 2.948 | 2.986 | 0.038 |
| the middle | 5.860 | 6.019 | 0.159 |
| the run-in | 0.451 | 0.484 | 0.033 |
| **the last 22 metres** | **1.500** | **1.680** | **0.180** |

**Forty-four per cent of the entire margin over the human field is in the last
twenty-two metres of an eleven-second run.** That is not where anyone would
look, and it is the whole coaching content of this map:

> **Do not chase the approach. Arrive at the last obstacle in the roll phase the
> flip wants.** Arriving five hundredths of a second earlier in the wrong phase
> costs you half a second.

Three measurements stand behind that sentence. Across the top ten runs, roll
error at the last contact predicts finish time at about **1.8 s per radian**;
40 ms of arrival-time variation contains a **400 ms** range of achievable
endgames, so the arrival window is fifty times finer than the outcome it
selects; and the field supplies its own counter-example — **the fastest human to
two gates finishes six tenths off the record**, because it arrives rolled +3.10
where the fast lines arrive at −2.55, and 0.6 rad times 1.8 s/rad closes the
arithmetic.

Counting back from the finish, the last three landings are worth roughly
**+1800, +310 and +140 ms per radian** of deviation. The last one is worth six
times the next.

## The run, as inputs

**The sector-by-sector guide for this map is not written yet.** What exists is
the record tape itself and the rule above; the tick-level walk-through of the
fifteen landings has not been done.

## How forgiving it is

Two different halves, and only the second is teachable.

**The opening is precision-bound — for everybody, including the humans.** There
are three windows in the first seconds where the tolerance is zero. Put the
human world record's own driven tape through the same test and its first seven
seconds survive 0–25 % of one-tick mistimings, against our tape's 70.4 %. On this
map the *human* is the tight one, and someone went out and hit it anyway.

**The finish is phase-bound**, which is friendlier: it does not ask for a
tighter input, it asks for the right *state* on arrival, and a state can be
aimed for and felt in a way a 10 ms window cannot.

Tolerance across whole tapes, as one-tick boundary shifts:

| tape | shifts tested | survive |
|---|---|---|
| **analog record 10.759** | 472 | **76.1 %** |
| keyboard 10.788 | 158 | 38.6 % |
| human record 11.169 — Schmaniol, the previous record | 74 | 24.3 % |

> **Study the 10.759 analog record.** It is the fastest thing on the map *and*
> three times more forgiving than the 11.169 a human actually drove.

That is worth saying plainly because the usual assumption runs the other way.
The sparse keyboard tape has a third of the boundaries and three steering values
and it loses on both axes at once: 0.029 slower and half as tolerant. Event count
predicts nothing about how forgiving a run is.

**What will take real practice** is the first seven seconds, where the map is
genuinely unforgiving for everyone, and then arriving at the last landing in the
right phase — which is a feel to be learned over many attempts, not a timing to
be memorised.

## Files

| file | what |
|---|---|
| `replays/TAS_10759.Ghost.Gbx` | **the record, and the most forgiving tape on the map** |
| `replays/TAS_10758.Ghost.Gbx` | fastest, not yet independently confirmed |
| `replays/TAS_10768.Ghost.Gbx` | equals the author time |
| `replays/TAS_10769.Ghost.Gbx` | one millisecond over it |
| `replays/KEYBOARD_10788.Ghost.Gbx` | keyboard only, three steering values |
| `replays/TAS_10859.Ghost.Gbx` | an earlier step in the chain |
| `replays/KEYBOARD_10897.Ghost.Gbx` | an earlier keyboard tape |

### What these recordings are

Each file's steer / gas / brake bytes come from its own input tape, so the two
channels of inputs a ghost carries agree exactly, and each one carried a
**stranger's ranked badge** — `Prestige=Yes&Level=7&Year=2026&…&Medal=Bronze`,
the carrier player's season standing, written twice per file and cleared by no
anonymiser because nobody had listed it. It is gone from all seven.

**Their positions were left alone, and this map is the one where that is not
good enough.** The regenerator does not sit on the game's own physics tick here:
regenerating a recording the game made itself moves it 0.136 m — one 10 ms tick
— reproducibly, on two different players' downloads, at every `--biastick`
between 60 and 400 and on five runs of each. All seven files sit at that same
offset, so all seven are **one physics tick out of step with the run they
claim**. Nothing here is a stranger's driving; the times are real and every file
re-simulates to the millisecond in its name. But the recording is a moment late,
and until the regenerator is fixed on this map that cannot be corrected — so do
not film a frame-synchronous side-by-side from these files.
