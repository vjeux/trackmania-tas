# 203072 — §8 RESOLVED: the map is usable, and the cause is a bounded GAME-BUILD WINDOW

Write-once. Supersedes the "unfalsifiable map" verdict in
`RESULT-attitude-test-v1.md` §4 and the §8 text in `ACQUISITION.md` that cites
203072 as the canonical unusable map. **Fold into ACQUISITION.md as §8d.**

Measured 2026-08-18 on 34663.od.fbinfra.net. Times in seconds with a decimal.

---

## The verdict

**The map is USABLE.** The oracle is faithful on it; the failures are ghosts
recorded inside a bounded window of *game* builds, amplified by a map that is
chaotically sensitive. §8b's "build-correlated ⇒ the map is fine, exclude the
old ghosts and seed from a good one" branch applies — with the twist that the
bad set is not "old", it is an *interval*.

## 1. The whole field, not a sample: n = 270 of 272 records

| recording build | n | exact | different time | DNF | reproduces |
|---|---|---|---|---|---|
| 2024-09-16_20_00 | 21 | 21 | 0 | 0 | **100.0 %** |
| 2024-09-17_11_17 | 43 | 43 | 0 | 0 | **100.0 %** |
| 2024-12-12_15_15 | 7 | 7 | 0 | 0 | **100.0 %** |
| 2025-06-19_21_00 | 3 | 3 | 0 | 0 | **100.0 %** |
| 2025-07-04_14_15 | 2 | 2 | 0 | 0 | **100.0 %** |
| **2026-01-18_17_00** | **38** | 15 | 4 | 19 | **39.5 %** |
| **2026-02-02_17_51** | **151** | 77 | 17 | 57 | **51.0 %** |
| **2026-04-28_18_20** | **1** | 0 | 0 | 1 | **0.0 %** |
| 2026-07-22_18_27 | 4 | 4 | 0 | 0 | **100.0 %** |
| **total** | **270** | 172 | 21 | 77 | 63.7 % |

**Outside the window: 80 / 80 = 100.0 %. Inside it: 92 / 190 = 48.4 %.**

That is the finding. It is **not** "old ghosts fail" — the previous agent's
"build-correlated but backwards" reading was made on a 34-ghost sample in which
the post-window builds were represented by a single ghost. With the whole field
the shape is unmistakable: a **bounded interval of game builds,
2026-01-18 … 2026-04-28, with 100 % on both sides of it.**

## 2. The oracle is not at fault — three independent proofs

1. **Telemetry tracking.** `fk verify` re-simulates a ghost's own input tape and
   compares against that ghost's own recorded telemetry:

   | ghost | recording build | position RMS | max | n |
   |---|---|---|---|---|
   | p002 (12.242) | 2024-09-17 | **0.001747 m** | 0.005477 m | 241 |
   | p018 (14.191) | 2024-09-17 | **0.001719 m** | 0.005501 m | 279 |
   | p014 (13.762) | 2025-07-04 | **0.001718 m** | 0.005231 m | 271 |

   **1.7 mm RMS over a full 12–14 s run.** The 134672 precedent that exonerated
   that map measured 8 mm. (A fourth, p011, ABORTed at the state-locate gate —
   best candidate 0.058 m against a 0.05 m threshold. That is the instrument's
   negative control refusing, not a fidelity failure.)
2. **Map identity.** Nadeo's own `/maps/<guid>/file` is **sha256-identical** to
   the copy under test (`b821d5e7…b853d`). §8's candidate explanation (a), "the
   map was edited in place and TMX serves a different version", is dead. §8
   called this "an authenticated fetch this project has deliberately not
   attempted"; **it needs no authentication** — the same file's own 270051
   update says so.
3. **Codec identity.** The seed rebuilt through the search's own encoder
   re-validates to 12.242 exactly.

## 3. Why only *half* the window fails: the map is chaotic

`tmprobe hold` freezes every input from tick T onward and sweeps T over the
whole tape. On the current TAS tape, **every T from 400 to 1200 DNFs**; only
T = 1220, three ticks from the finish, survives. There is **no dead zone and no
decided tail** on this map — all 5.7 s of flight is live attitude control.

So any physics difference, however small, is amplified without limit. That
predicts the failures inside the window should look like a **coin flip
independent of driver skill**, and they do:

| finish-time group (within window) | n | reproduces |
|---|---|---|
| 12.083 – 14.828 | 19 | 63.2 % |
| 14.852 – 16.245 | 19 | 47.4 % |
| 16.373 – 18.429 | 19 | 42.1 % |
| 18.445 – 20.472 | 19 | 63.2 % |
| 20.573 – 23.016 | 19 | 36.8 % |
| 23.124 – 25.975 | 19 | 31.6 % |
| 26.024 – 27.403 | 19 | 57.9 % |
| 27.620 – 32.385 | 19 | 57.9 % |
| 32.674 – 38.414 | 19 | 36.8 % |
| 39.153 – 50.919 | 19 | 47.4 % |
| **outside the window, same measure** | **80** | **100.0 %** |

No trend with finish time. It is not that fast runs lean on an affected feature
— it is that ~half of *all* runs recorded under the anomalous physics have
diverged by the time they reach the gate, regardless of how they were driven.

## 4. What this means, stated carefully

* Our oracle (server build 2026-05-15 git128182) agrees **exactly** with every
  one of the 80 records set before 2026-01-18 or after 2026-04-28.
* Something in the game's physics differed for runs recorded between
  2026-01-18 and 2026-04-28 inclusive, on this map.
* **The human world record, 12.083, was set on 2026-02-02 — inside the window.**
  So the record we are chasing was set under physics our oracle does not
  reproduce, and it DNFs here. That is a real caveat and it is why the seed for
  any search on this map must be a run from outside the window.
* This is a statement about *this map*: the same oracle reproduces other maps'
  fields on the very same 2026-02-02 build (126859 validated a tape on it
  exactly). A chaotic map is simply a much more sensitive detector of a small
  physics difference than a road map is.

## 5. Consequences for the project

1. **203072 comes off the abandoned list.** §8's own text names it as the
   canonical unusable map; that should now read "usable, seed from outside the
   2026-01-18 … 2026-04-28 window".
2. **§8's sample-size guidance needs strengthening.** A 34-ghost sample gave a
   qualitatively *wrong* shape ("backwards") because the post-window builds had
   n = 1. When a §8 check fails, **read the build off every ghost** — it is free
   (`tmmaps scan GHOST` prints it out of the LZO'd body; `strings` on the file
   finds nothing) — and tabulate the whole field before interpreting.
3. **§8's remedy needs a third branch.** Not just "old ghosts fail ⇒ exclude
   them" but **"an interval of builds fails ⇒ exclude the interval"**, which you
   cannot see at all without both edges sampled.
4. **A §8 shortfall is not evidence about the oracle.** Run `fk verify` before
   drawing any conclusion: it answers "is our physics right on this map" 
   directly, in one command, and here it says yes to 1.7 mm while 31 % of a
   34-ghost sample was failing.

## Artefacts

`acq/val_full.txt` (all 270 validations), `acq/builds.tsv` (build per ghost),
`acq/joined.tsv` (the join behind every table above), `acq/fid_*.txt`
(`fk verify` outputs), `acq/nadeo.Map.Gbx` (sha256 above).
