# idm ruinin ur day #460 — the author time falls, and a respawn was the crowbar

**Author time 15.643 · the only human record 8790.769 · best validated 15.230.**

| tape | validated | vs AT | what it is |
|---|---|---|---|
| [`TAS_15230_clean`](replays/TAS_15230_clean.Ghost.Gbx) | **15.230** | **−0.413** | the best run on this map, clean start, **zero respawns** |
| [`TAS_15240_clean`](replays/TAS_15240_clean.Ghost.Gbx) | **15.240** | **−0.403** | the tape the gate ladder brought home first |
| [`TAS_15290_lowinput`](replays/TAS_15290_lowinput.Ghost.Gbx) | **15.290** | **−0.353** | keyboard steering on the ramp, wheel centred for the whole glide — **86 input changes** |
| [`TAS_15549_provenance`](replays/TAS_15549_provenance.Ghost.Gbx) | 15.549 | −0.094 | the first tape ever to beat this author time, kept as the provenance record |
| author time | 15.643 | — | — |
| human record, wschseng *(control)* | 8790.769 | — | 2 h 26 m — see below |

TMX map [165922](https://trackmania.exchange/maps/165922) · uid
`mP8HzG68YxUY6yJcrQFx2inUjtk` · **one recorded run**.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## The leaderboard is one run, and it is not a lap

The single human record on this map reads 2 hours 26 minutes. It is not a slow
lap. It is **one session of 915 attempts and 941 respawn presses** with the clock
running through all of them, and the driver never converted one: their best
attempt stopped at x = 2294.8, **five metres short of the first gate row at
x = 2300**, and everything after that is a retry.

So the classification for this map is **known-but-unheld**. Nobody needs to
discover the technique — one person performed it 915 times in a single sitting.
What nobody has done is finish one.

## What the run is, for a driver

The map is a start block on a chute, a fall, two booster gates, and then a very
long glide.

* **13.5 of the 15.2 seconds need no input at all.** Force steer = 0 from tick
  954 to the end of the tape and the time changes by **1 millisecond**
  (15.292 → 15.293). The whole 1.9 km glide is ballistic. The driver lets go.
* The load-bearing part is **ticks 154–954 — eight seconds**: creep off the start
  block, fall, take the two booster gates. Zeroing any 100-tick window before
  tick 754 is a DNF; every window after 954 is free.
* Of those eight seconds, **the first ~3.5 s is gravity**: 168 m from about
  7 m/s against 23.29 m/s² solves to 3.51 s, which is exactly what the human
  achieves. It is not compressible.

Two physical facts bound the whole map, both measured here:

* the opening 3.5 s is free fall;
* the glide is unpowered and capped at 277.55 m/s, and it is 1 880 m across and
  1 889 m down — a 2 665 m path, so **≥ 9.6 s at the cap**. Our glide takes
  9.70 s.

## Precision, stated honestly

`tol` shifts every input-change boundary in the ramp by ±1…8 ticks: **343 of 343
shifts DNF**, except three at ticks 491–502 which survive at +0.114 to +0.133.
That is a fact about *our* tape, not about the map — ours is a 10 ms-grain TAS
line. A human drove down this chute 915 times, so a forgiving line exists; the
obvious next piece of work is to re-search the ramp with per-boundary tolerance
as the objective rather than the finish time, the way
[270051](../270051-fall-2025-16-cp1-end) was done.

## How it was cracked: a respawn is a legal input

Two earlier sessions established the deliverable had to be one clean
no-respawn attempt from tick 0, and that the human's one winning attempt could
not be transplanted there. Both are true, and both left the map stuck.

The step that was missing is that **a respawn is an input, and the state it
restores is canonical**. It rides in bit 31 of the input packet's 34-bit state
literal (literal `0x80000002`) — a place `ghost::Factory` cannot see, which is
why 941 of them were invisible against 914 telemetry discontinuities. That turns
the impossible transplant into two lines:

```
[ any prefix reaching race t = 1.670 s ] ++ [ respawn packet ] ++ [ the winning attempt ]
```

and it finishes at exactly `(K + L)·10 − 1540` ms. Swept over 4 700 ticks of
prefix, the arithmetic is perfectly linear — K=321 → 20.519, K=400 → 21.309,
K=1400 → 31.309, K=5000 → 67.309 — including from mid-flight at the speed cap.
Mutate the prefix 3 000 times and **140 finish, every one of them at exactly the
same millisecond**. The 1 885 ticks after the respawn replay identically
regardless of what the car was doing before it.

That produced the field's **first finishing clean-start tape on this map**, at
20.519, and 16.785 with a better tail.

**It was never the deliverable.** The respawn cannot be armed before race
t = 1.670 s (K = 321 finishes, K = 320 does not, and that floor is not a property
of the human's driving), so the respawn route bottoms out near 16.1 — always
outside the author time. It was the *instrument*: having any finishing tape is
what made a dense score, a calibrated gate ladder and a real search possible,
and the ladder is what carried a genuinely respawn-free tape home.

## Where the 1.2 seconds actually is

Station-by-station, the winning clean tape against the same tail on the respawn
route:

| station | x | clean tape | respawn route | Δ |
|---|---|---|---|---|
| p1 | 423 | 1.958 | 2.968 | −1.010 |
| s1 | 505 | 4.118 | 5.206 | −1.088 |
| launch | 713 | **5.550** | 6.656 | **−1.106** |
| finish | 2300 | **15.246** | 16.461 | −1.215 |

**−1.106 of it is the start**, and −0.109 is a slightly better glide. The clean
run reaches the state the respawn manufactures in 0.56 s; the respawn costs
1.670 s. **That difference is the author time.** The author's route was always a
clean start; there was never another way.

## The ladder, and the trap it sprang on schedule

Return-to-origin control first: rewriting the 132 gates back onto their own
lattice reproduces the human record at 8790.769 and the incumbent at 16.461, so
the surgery is faithful and no model is being swapped. Calibrated against the
human's own winning attempt, every station matched to ≤ 8 ms.

Then the decoy fired exactly as documented. Scoring on the **mid-course** rung at
x = 1216 drove the crossing from 9.640 to **7.860** — 1.838 s ahead of the human
— and those tapes reached neither the next station nor the finish. Optimising
"time to a rung in the middle" buys a dive. Moving the objective to the **far**
rung at x = 1822 fixed it in one round, and the first harvest from that arm
finished the real map at 15.549.

## Low-input

Converting a finished tape does not work here — quantising the whole steer array
to `{−127, 0, +127}` DNFs, and the search tool refuses to start on it. So the
constraint was applied *under* search, and only to the part of the tape that
tolerates it. The glide is already input-free, so the march ran backwards from
tick 954.

| | analog 15.240 | low-input 15.290 |
|---|---|---|
| input change events | 611 | **86** |
| distinct steer values | 226 | **39** |
| ticks with no steering at all | 1155 (11.55 s) | 1155 (11.55 s) |

The frontier is tick 604 (race t ≈ 4.5 s) and it is sharp: extend the constrained
window ten ticks earlier and the seed DNFs, at three levels, at five, and at
nine. Ticks 594–604 are in the booster phase and want finer than 32/254 of lock
*on this line*. The free-fall window does tolerate keyboard (it finishes, 3.5 s
slower), so the basin is reachable — just not from here for free.

*Counting convention: an input change event is any tick where steer, gas or
brake differs from the previous tick; the alphabet is counted over the whole
tape including the pre-start ticks.*

## Validation

Every tape above re-validated on the untouched map (md5
`1cc927bbb1d640c665ff69068352d4e6`) through the plain oracle, in a batch
containing the human record as a known-answer control:

```
AT_BEATER_15240.Ghost.Gbx     15240
AT_BEATER_15549.Ghost.Gbx     15549
lowinput_15290.Ghost.Gbx      15290
vj4_clean_15230.Ghost.Gbx     15230
rank00001_8790769.Ghost.Gbx  8790769   <- known-answer control, exact
```

The 15.549 tape was additionally re-measured on a **third machine** by a
different agent with its own build fork and staging root, which is where the
respawn audit below comes from.

Every published tape here carries **zero packets with bit 31 set** — no respawns,
one attempt, from tick 0:

```
archive 0: fmt 12  start_offset -1540  packets 2109  (1 state literal)  0 with bit31
```

`start_offset -1540` is the ordinary 1.54 s countdown a normal human tape
carries, so the race begins at tape tick 154 and ends 15.23 s later.

## Notes

* [`RESULT.md`](notes/RESULT.md) — the full session write-up
* [`VERIFICATION.md`](notes/VERIFICATION.md) — the independent third-node re-measurement
* [`ORACLE_THROUGHPUT.md`](notes/ORACLE_THROUGHPUT.md) — three oracle defects found here that
  are worth ~1000× on any tape cut from a long recording. They are not specific
  to this map and they are written up in [`FINDINGS.md`](../FINDINGS.md).
