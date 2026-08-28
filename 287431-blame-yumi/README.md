# blame yumi for blaming me

**A 21-second map where a perfect assembly of every human's best sector equals
the author time exactly. We beat the world record by 1.554 and stopped 1.093
short.**

**blame yumi for blaming me** — TAS **22.538** (+1.093) | AT 21.445 | WR 24.092 by ITZYNO1FAN

https://github.com/user-attachments/assets/bd1a2dd2-9fa4-4b0d-ba10-22ea02cae643

*Two cars, with this run's own inputs drawn on, against ITZYNO1FAN's world record.*

The author time is **not** beaten. The human world record is, by **1.554**.

## The map's own answer to "is the author time human?"

Take all fifteen records, split each into sectors, and take the best anyone has
ever driven in each:

| | time |
|---|---|
| perfect five-driver composite | **21.445** |
| author time | **21.445** |

**Exactly equal, to the millisecond.** No assembly of demonstrated human driving
beats this author time — it only ties it. That is unusual, and it is why this
map stayed unbeaten.

## What we did

**S1–S6 is the world record's own input tape, byte for byte.** It is also
chaotic: a single tick changed by one unit is fatal, 32/32 DNF with matched
same-sign controls. There is nothing to gain there and no safe way to look.

**Everything we won is in S7**, the last sector, decomposed by measurement
rather than by argument:

| phase | share | what it is |
|---|---|---|
| engine cap | 37 % | the car is at its speed ceiling |
| launcher physics | 52 % | full throttle, trajectory set at entry |
| pinned approach | 11 % | inputs that cannot change without DNF |

Each phase was perturbed independently. None yielded.

## The floor is a combination lock

The search saturated at **22.541** and would not move. Then a deliberately
damaged tape, re-fitted, returned **22.540** — below a floor a million clean
candidates never crossed. The mechanism turned out not to be the damage:

```
2225..2233 brake=0   alone ->  22.681
2266..2273 brake=1   alone ->  23.137
all five pieces together ->  22.540
```

**Every subset is worse than the incumbent. Only the assembly pays.** A search
that proposes one edit at a time can never reach it — not because the region is
unexplored, but because every path into it runs downhill through a regression.
That is an *arity* limit, not a basin.

Confirmed from the other side: a three-lobe joint fit reaches 22.540 from the
clean seed with no damage at all, and six distinct tapes now sit at 22.540
across four input channels, **all editing ticks 2210–2320**. The channel varies;
the place does not.

## What was measured and did not work

| | scale | result |
|---|---|---|
| single-window additive delta | 1,336,500 candidates, 844,899 finishers | nothing below 22.541 |
| wreck-and-re-drive | 29 arms, seeds paired | clean seed reaches 22.540 too — wrecks never load-bearing |
| wreck depth / width / onset | 168 characterised tapes | depth orders weakly; **width and onset inert at matched depth** |
| lobe counts 5 and 6 | | worse than 3, finish rate falls monotonically with count |
| route order | all 720 permutations | the human visiting order is optimal |

## Files

- `replays/TAS_22538.Ghost.Gbx` — the run in the video
- lineage: wreck `steer=-60` over ticks 2274–2304 (damaged to 22.678), re-fitted
