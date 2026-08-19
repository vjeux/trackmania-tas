# The Magnet Trial — the author time falls by 1746.748 s (−68.8%)

| | time | vs AT |
|---|---|---|
| **best** | **793.893** | **−1746.748 (−68.8%)** |
| the 16 sector cuts, before event minimisation | 795.034 | −1745.607 |
| **the human record with ONE attempt deleted** | **2501.894** | **−38.747** |
| Author time (never beaten by a human) | 2540.641 | — |
| Human WR — keby | 2575.154 | +34.513 |

unbeaten.at MapId 186935 · **7 recorded runs** (the source data said 3) · 16
checkpoints + finish, named `magnet-trial-cp-01…16` in the skin dependencies.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## The one-line version

**keby's gap to the author time is 34.513 s. Eight of their 116 failed attempts
individually last longer than that.**

Delete exactly one — a **73.260 s fall** — and their run finishes in 2501.894,
**beating the author time by 38.747 s** with every other input untouched.

**The author time on this map measures patience, not driving.**

## What the record actually is

**221 respawn presses → 115 events, 106 of them standing respawns.** 68.5% of
the recorded time is failed attempts, spread across 25 obstacle regions.

| | |
|---|---|
| one magnet climb at (1024, 315, 716) | **639.194 s over 35 attempts — a quarter of the entire race** |
| the top five obstacles together | **51.0%** of the run |
| clean-equivalent of the human record | **792.431** |

That last number was known **before a single candidate was evaluated** — the
first hour was diagnosis, and it decided the map.

## How the 793.893 was built

Sixteen sector cuts, with `finish = base − deleted` exact to the millisecond:
2575.154 − 1780.120 = **795.034**. All sixteen were tested as a *cumulative
ladder* rather than assumed to compose. Event minimisation then gave **793.893**
at 16,397 events.

**No driving search was run at all.** The driving is keby's, unchanged.

## A correction to this project's own cutting rules

The fleet had a rule that cutting to a standing respawn only works at **one
exact, non-periodic phase**. That is **a property of one tool**, not of the game:
`cutsweep` splices through a respawn-blind factory. Packet-level `tmcut del`
carries the respawn bit with the packet, and a 1-tick sweep of sector 14's cut
point gave **27 consecutive survivors, all arithmetic-exact**.

**Cut with `tmcut` first.**

Also established here: the `0x0309202D` + `0x0309202B` chunk pair must be **the
tape's own** — installing a donor's pair breaks the container's own tape — so a
mixed-donor tape has no valid pair. And `0x0309202B` is where a map's checkpoint
order is readable.

## Two negatives, stated honestly

**Best-of-field splicing is worth 129.294 s here, and our negative is VOID.**
168 join offsets on one sector and 36 on another all DNF'd — but the control run
afterwards showed **each donor's own segments do not reassemble either**, while
keby's do. So the instrument was broken and the result says nothing. Reported as
void, with the fix named, rather than as a finding.

**The alphabet half of the low-input question is OPEN, not negative.** The
minimiser's alphabet mode wrote **123 GB into `/dev/shm` in 13 minutes** and took
a shared machine to 100% before completing a single round. It was cleaned
immediately and not re-run, so nothing is known about the alphabet here.

## Validation

Three cold passes, three known-answer controls each; identity control 7/7 exact
against the live leaderboard. **And the whole directory has been re-validated
with the map and tapes read only from the durable store** — nothing from scratch
— after the coordinator found the map file had never been banked. 20/20 manifest
hashes verified.

## Files

| file | what |
|---|---|
| `replays/BEST_793893.Ghost.Gbx` | the best run |
| `replays/CUT_795034.Ghost.Gbx` | the sixteen sector cuts, before event minimisation |
| `replays/ONE_ATTEMPT_DELETED_2501894.Ghost.Gbx` | **keby's own run, one 73.260 s fall removed — under the author time** |
| `notes/RESULT.md` | the full write-up: obstacle table, cut ladder, the void negative |
