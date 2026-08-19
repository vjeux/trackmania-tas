# 153527 — `P-Found - Pokeuuu` (RPG / Pathfinding) — **NOT A TARGET**

Worked 2026-08-18 on node 10003. Verdict: **not a target.** No time claimed, no
search run.

> **This is v2. `evidence/RESULT_v1_RETRACTED.md` is the first write and it is
> WRONG on its second reason** — it claimed the map itself was unvalidable. It
> is not; see §2 and the retraction note in
> `../ACQUISITION_addendum_153527_simulation_hazards.md`. The verdict did not
> change, but the mechanism did, and the wrong version was the more alarming
> one, so it is kept visible rather than deleted.

| | |
|---|---|
| MapId (unbeaten.at) | 153527 |
| uid | `4ympwQ3XZfX8balg2UcVJBL_pnf` |
| Nadeo map guid | `4dc3b1f1-f43b-4ab3-8498-ce2eacacffd6` |
| Author time | **939.283 s** (15m 39s) |
| Only human record | **5661.335 s** (1h 34m), AuwrahTM, set 2024-02-16 |
| Records on the board | 1 · TMX replays: **0** |
| Map saved on build | `2024-01-10_12_53` · header `validated="1"` · mood `Day (no stadium)` |
| Size | 44 388 blocks, 1 536 items, 12 waypoints (11 CPs + Goal) |

**Three reasons, in increasing order of how decisive they are. The third stands
entirely on its own and is the one that closes the map.**

---

## 1. The decode: `validated="1"`, and **no author ghost in the file**

```
tmtraj decode map.Map.Gbx
  FAIL map.Map.Gbx: CPlugEntRecordData (0x0911F000) chunk not found
```

Verified the §9a way — the body is LZO-compressed, so a raw byte scan of the
file proves nothing. `tools/` holds a small Rust `mapscan` that decompresses the
body and counts class ids. **Decompressed body 2 702 856 bytes:**

| marker | 153527 | 228607 (positive control) |
|---|---|---|
| `0x0911F000` CPlugEntRecordData | **0** | 2 (offsets 607759, 607763) |
| `0x0309201D` ghost inputs | 0 | 0 |
| `0x0303F005` | 0 | 0 |
| substring `Ghost` | **0** | 1 (offset 607715) |
| `0x03092000` | 1 @ 869148 — inside baked-blocks chunk `0x03043048` (851231…1038503), coincidental bytes, the 267460 pattern | 1, likewise |

**The positive control passes**: the same binary decodes 228607's embedded
author lap (406 samples, 50 ms, 0→20290 ms). The negative is real.

§9f outcome #1 (`validated="1"`, no ghost) on a **2024** saving build — newer
than 134672's 2022 one, so "the saving build predicts presence" takes another
hit and should probably be retired as a rule.

Consequence: **the author's route is unknowable**, which on a *Pathfinding* map
is the entire question.

## 2. The one human record does not re-simulate — because it is a **dead-build ghost**

```
tmtas validate --map map.Map.Gbx ghosts/rank00001_5661335.Ghost.Gbx
  rank00001_5661335.Ghost.Gbx     DNF     1
```

Not a truncated download (§8a): a refetch is byte-identical (sha256
`b8309cd6…3bbe3`, 10 703 882 bytes), the archive parses complete (566 327
packets = 5663.27 s of tape against a 5661.335 s race time), and the 12 splits
read out cleanly. Raw validator:

```
"ValidatedResult" : null,
"Desc" : "wrong simu\nhad simulation hazards '0-1-0'\n",
"DeclaredResult" : { "NbCheckpoints": 12, "NbRespawns": 4294967295, "Time": 5661335 }
"GameBuild" : "Trackmania date=2024-01-10_12_53 git=126731-1573de4d161 GameVersion=3.3.0"
```

### What the hazard clause is NOT

**v1 of this file claimed `had simulation hazards '0-1-0'` is a map-level
refusal. It is not.** On 126859 a TAS tape prints the identical clause and
returns a real time (`ValidatedResult.Time = 23466`). The clause coexists with
success and is not a failure signal.

### What actually killed it

`GameBuild = 2024-01-10_12_53 git=126731` with `NbRespawns = 4294967295`
(= `(u32)-1`, never written). 126859's field gives the controlled comparison —
**21 of 22 exact, and its single failure is the byte-identical build string with
the byte-identical corrupt respawn count**:

| map | ghost | GameBuild | NbRespawns | result |
|---|---|---|---|---|
| 126859 | r01 (WR) | 2025-07-04 | 0 | exact, 24342 |
| 126859 | tas | 2026-02-02 | 0 | exact, 23466 (+ hazard clause) |
| 126859 | **r22** | **2024-01-10 `git=126731`** | **4294967295** | `wrong simu` + hazard |
| **153527** | **the only record** | **2024-01-10 `git=126731`** | **4294967295** | `wrong simu` + hazard |
| 238835 | human record | 2026-02-02 | 107 | exact, 1964933 |

So this is **§8b build-correlated**, which normally means *the map is fine —
exclude the old ghost and seed from a recent one.* Here that remedy has nothing
to work with: **one record on the board, zero replays on TMX.** The map is
probably healthy and simultaneously **unseedable** — no finishing tape that
re-simulates exists anywhere, and §1 says the map carries no author ghost
either. With a field of one, a mismatch is disqualifying per the brief.

*(My own experiment design here was faulty and is worth not repeating: I varied
the tape — five prefixes down to a stationary 1.48 s countdown cut — saw the
clause every time, and concluded "not the driving, therefore the map". Every cut
came from the same ghost, so the test could not separate ghost from map. Raw
outputs kept in `evidence/rawval_*.txt`.)*

## 3. It **is** a retry map — and deleting every retry still misses the AT by 275.302 s

**This reason is independent of §1 and §2 and does not depend on the oracle at
all** (telemetry decodes regardless of whether a tape re-simulates — §9e).

`NbCheckpoints: 12` (11 intermediate + Goal), so there is plenty to respawn to —
not the 267460 spawn-and-goal trap.

`tmpk changes`: **111 respawn presses, `word0 = 34` (0x22)**, plus one
`word0 = 66` at t = −1.570 (a start-line packet, not a respawn). **Every gap
between presses is ≥ 3.770 s** — not one 100–640 ms double-tap, so all 111 are
**SOFT** respawns to the driver's own crossing state.

The human clears **CP8 at 929.549 s**, inside the 939.283 s author time, then
spends 65 minutes on two segments:

```
CP1  end=    11.613 dur=   11.613 presses=0   survives=   11.613
CP2  end=    69.769 dur=   58.156 presses=0   survives=   58.156
CP3  end=   224.486 dur=  154.717 presses=2   survives=  111.856
CP4  end=   322.974 dur=   98.488 presses=1   survives=   57.154
CP5  end=   504.220 dur=  181.246 presses=1   survives=  106.740
CP6  end=   605.844 dur=  101.624 presses=2   survives=   54.124
CP7  end=   815.602 dur=  209.758 presses=5   survives=   95.312
CP8  end=   929.549 dur=  113.947 presses=0   survives=  113.947
CP9  end=  3075.474 dur= 2145.925 presses=39  survives=  235.274
CP10 end=  4851.380 dur= 1775.906 presses=39  survives=  132.270
CP11 end=  5231.384 dur=  380.004 presses=8   survives=  154.024
CP12 end=  5661.335 dur=  429.951 presses=14  survives=   84.115
```

`survives` = the final attempt in each segment: what is left once every failed
attempt is deleted, i.e. the ceiling of the 238835 method here.

> **Retry-deletion floor = 1214.585 s.  Author time = 939.283 s.
> Short by 275.302 s — the surviving line must still get 22.7 % faster.**
> On 238835 the same method landed 42 % *under* the AT with no driving search.
> Here it lands 29 % *over*.

### And the residue is not milling, so it is route, not retries

Over the 1214.585 s that survive (`evidence/residue_table.txt`, from the
record's own 60 ms telemetry, 85 811 samples):

- **26 023 m** of path, i.e. **77.1 km/h average**. Matching the AT over that
  same path needs **99.7 km/h average**, on a map whose checkpoints sit on Tech,
  **Dirt** and **Ice** platform blocks.
- only **96.3 s (7.9 %)** below 5 km/h and **267.1 s (22.0 %)** below 20 km/h;
  median speed is 45–105 km/h in nine of twelve segments. Nothing like 284238's
  ~40 s of milling in bowls — this line is genuinely driving.
- the two bad segments are CP10 (132.270 s, median **13.2 km/h**, 31.4 s under
  5 km/h) and CP11 (154.024 s, median 27.6 km/h, 23.2 s under 5 km/h): together
  ~55 s of the 96 s near standstill. Deleting *all* of it still leaves ~180 s.
- path/displacement is 17× in CP3 (4461 m driven, 267 m net) and 20× in CP12 —
  on a *Pathfinding* map that means the human's route is long, plausibly far
  longer than the author's. **That is exactly the missing information**, and it
  is missing because there is no author ghost (§1) and one human record (§2).

284238's verdict with different numbers: after every removable failure is
deleted, the residue is **route**, not retries — and here the route cannot be
seen, seeded, or scored.

---

## Files

```
map.Map.Gbx  mapinfo.json  lb0.json  tmxrep.json
ghosts/rank00001_5661335.Ghost.Gbx      the only human record
evidence/RESULT_v1_RETRACTED.md         first write; §2 mechanism is WRONG
evidence/rawval_full_tape.txt           raw validator: build + corrupt NbRespawns
evidence/rawval_prefixes.txt            5 prefixes (flawed experiment, kept)
evidence/rawval_countdown_only.txt      countdown cut  (flawed experiment, kept)
evidence/control_238835_full.txt        control: exact 1964933 on a working map
evidence/control_238835_countdown.txt   control: countdown cut, wrong simu, no hazard
evidence/changes.txt  resp34.txt        packet word0 change events / the 111 presses
evidence/segment_table.txt              the table above
evidence/residue_table.txt              speed/path breakdown of the surviving line
evidence/listall.txt                    44 388 blocks + 1 536 items
evidence/wr_telemetry.csv.gz            85 811 samples, 60 ms
tools/                                  Rust `mapscan` (body class-id scan) + `residue`
```

## If anyone reopens this map

The one thing that would change the picture is **a second human record from a
2025+ build** — it would give a seedable tape and let §3's floor be attacked
directly. As of 2026-08-18 the board has one record, set 2024-02-16, and TMX has
none. Nothing else here is worth re-running.
