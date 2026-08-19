# 146612 — what the near-identical sibling 151734 says, and it is about SECTOR 3

2026-08-19 07:56Z, from the answer-key agent's sibling corpus. Every number below
is measured on my node from the banked artefacts.

## The sibling is the same map where it matters

`key_151734.Map.Gbx` ("Spaghetti Nights 3", 98.1 % of my block records
byte-identical) has **the same four-finish-gate staircase, at the same world
cells** — G1 (720,34,976), G2 (688,34,944), G3 (656,34,912), G4 (656,34,880) —
and **the same checkpoint gates**: mernama crosses CP4 at (771.2, 18.0, 583.0)
against my map's (768, 19, 590), and CP5 at (1174.0, 42.0, 736.2) against mine
at (1170, 42, 736). Same structure: 5 checkpoints + finish.

So her 39.555 is a **human lap on my geometry**, and it is directly comparable
sector by sector. (Her tape DNFs on my map — the series does not replay — but
the *line* transfers as information.)

## Sector by sector, against my map's field

| sector | mernama | my rank 1 | my field's best | mern vs field-best |
|---|---|---|---|---|
| 0 | 7.272 | 7.311 | 7.295 | −23 |
| 1 | 8.439 | 8.407 | 8.401 | +38 |
| 2 | 4.214 | 4.262 | 3.784 | +430 |
| 3 | 8.158 | 7.854 | 7.854 | **+304** |
| 4 | **5.152** | 5.750 | 5.674 | **−522** |
| 5 | **6.320** | 6.639 | 6.396 | −76 |

**She is 522 ms faster through sector 4 than any of the 181 humans on my map —
driving the loop line, not the ramp** (closest approach to the ramp at
(944,10,592) is 77.5 m, and she runs the low loop to z ≈ 505).

## Why: it is entry speed, and it is bought in sector 3

| at CP4 | speed |
|---|---|
| **mernama** | **128.0 m/s** |
| my rank 1 | 113.2 |
| my rank 2 | 111.9 |

She arrives 15 m/s faster and **pays +304 ms in sector 3 to do it**. Net over
sectors 3+4: 13.310 against my field's best-possible 13.528, **−218 ms**, and it
keeps paying — she is still 13 m/s up at CP5 (97.5 vs my field's ~84) and
crosses G1 at **115.8 m/s** where my rank 1 crosses at 104.6 and the G1 group
averages 85.3.

This is the project's own standing finding — *entry speed into the decisive
feature is set several seconds earlier* — appearing as a measured, human-driven
trade on this exact geometry. **Sector 3 on my map has never been searched for
CP4 exit speed; it has only been searched for its own split.** That is a
different objective and it is the one that pays.

## What it does to the arithmetic

Best human CP5 on this geometry is mernama's **33.235** (349 ms better than my
map's rank 1). Combine with our own best sector 5 (6.147):

```
33.235 + 6.147 = 39.382     vs AT 38.530   ->  +852 ms
```

**So the best human reference on near-identical geometry, combined with the best
sector 5 this project has driven, is still 852 ms short of the author time.**
That is an independent confirmation of the conclusion from the other direction:
without something like the sector-4 jump this map's author time is not reachable
by recombining known driving — and the jump is dead by the heading law
(`EXIT-UNSOLVABLE-v1.md`).

It also sharpens where the remaining time is. Not sectors 0–2: mernama is +445
across them combined and my field is already at or near her level there. It is
**sector 3, re-optimised for CP4 exit speed rather than for its own split.**

## Note on tooling, for the record

The answer-key agent reports `tmmaps` panicking on 146612
(`unhandled inline node class 0x40000000`) and that its census returns nothing.
Confirmed as a panic — but the workaround `TMMAPS_NO_BAKED=1` has been in use
here since the first hour and gives the full census: **2880 blocks / 661 items**,
which matches their post-fix figure of 279 free + 2601 placed + 661 items
exactly. Their two-line parser fix is the better answer and should land; nothing
in this map's analysis was blocked by it. `blocks-v1.tsv` in this directory is
the full census and it is correct.

## Recommendation

Sector 3 belongs to the other arm and this is now the most valuable thing on the
map. Concretely: **score sector 3 by speed at CP4, not by time to CP4** — or
score it by time to a station 100–200 m *into* sector 4, which the ladder makes
free. A human has already demonstrated that the trade is worth 218 ms net, and
nothing in my map's 181-run field does it.
