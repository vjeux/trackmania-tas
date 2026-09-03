# Official Stadium physics epochs, TM2020 launch → January 2022

Audit of the interval the Historical Physics Lab taxonomy omits. The lab's
oldest Stadium profile is `STADIUM_PRE_2022_03_29` — "through 2022-03-25",
represented by the January 2022 client. This report shows that interval is
**not homogeneous**: it contains **nine distinct behavioral classes** between
the 2020-07-01 launch and 2022-01-21, two of them separated by very large
margins, and three of them coinciding with official Nadeo behavior statements.

Official vehicles only. Every probe is an official Stadium `CarSport` run; every
binary is an official Nadeo release. No community or restored cars.

---

## 1. Headline answer

**Launch 2020 differs behaviorally from January 2022, and the difference is not
one step.**

Two boundaries dominate the window:

| boundary | probes whose verdict flips | official statement |
|---|---|---|
| **2020-07-17 → 2020-07-23** | **87 of 125** | — (nearest: 2020-07-07 ice/wheel fix) |
| **2021-05-31 → 2021-06-08** | **97 of 125** | 2021-06-12 Royal: "new physics" |

At the 2021-06 boundary the Stadium physics pack, the shared pack and the title
pack are all **byte-identical** across the boundary and only the executable
changes — so that change is isolated to **code**, not data (§7).

---

## 2. Why the existing taxonomy could not see this

The lab's pre-2022 evidence (`evidence/server-inventory.tsv`) runs **one**
2022-era probe — Roevhaal's 63.546 on KEKL- SAUSAGE ICE — against all archived
servers. Every build from 2020-07-01 to 2022-03-25 returns the same verdict,
`WRONG_SIMU` or `INVALID_OTHER`.

**That instrument is saturated inside the window**: zero resolving power, so
declining to split was correct on that evidence, but it was never evidence of
homogeneity. The lab README says so itself — "The January profile is named after
its representative build rather than claiming every older build is homogeneous."

---

## 3. The instrument

1. **A replay embeds its own map.** `gbx::container::embedded_map_in`: when a
   replay carries chunk `0x03093002` the dedicated server simulates *that* copy,
   and any `UserData/Maps` entry is decoration. An era-correct replay is
   therefore a self-contained (map + input tape) pair — which removes the
   2020/2021 map-format barrier entirely, with no pack extraction and no
   title-pack decryption.
2. **A replay header names the recording client build, in plaintext**:
   `exebuild="YYYY-MM-DD_HH_MM"`, `validable="1"`, `best="<ms>"`. The corpus can
   be bucketed by recording era without decompressing a body.
3. **`/validatepath` reports the recording build back** in `GameBuild`, so every
   matrix cell carries its own provenance.

An epoch is an **equivalence class of builds under "same verdict on every
probe"** — never a hash change. Three verdicts are kept apart:

| verdict | meaning |
|---|---|
| `EXACT` | re-simulated to the declared time **and** checkpoint count, `IsValid` true |
| `WRONG` | enumerated and simulated, did not reach the declared result |
| `NOLOAD` | never enumerated — a **container-format** fact, not a physics fact |

Classes are computed only over probes that **every** compared build loaded, so
container-format churn cannot manufacture an epoch. Each boundary is reported
with the **number of probes that actually differ**, so a one-probe distinction
is never presented as equal to an 87-probe one.

Rust tools, no crates: `tmarc` (archive probe/fetch), `tmlab` (SHA-256, binary
identity), `tmx` + `tmxclass` (corpus harvest, classification by recording
build), `tmmatrix` (build × probe matrix, isolation mode), `tmepochs`
(equivalence classes, boundary strength), `tmnews` (official post extraction).

---

## 4. Archive inventory

### 4a. Official dedicated servers — the physics carriers

`http://files.v04.maniaplanet.com/server/TrackmaniaServer_YYYY-MM-DD.zip`

HEAD sweep of every date name 2020-01-01 → 2022-12-31 (1096 names, **0 errors**)
returns **56 builds**. **43 are in scope**: 41 in-range (2020-07-01 …
2021-12-02) plus 2022-01-01 and 2022-01-21 as the January-2022 boundary
reference.

- All 43 downloaded; **all 43 verified COMPLETE** against declared
  `Content-Length`; SHA-256 recorded for every archive and every executable.
- **39 distinct server executables.** `2020-10-02 / 10-12 / 11-04 / 11-16` share
  one (`cbceead7…`, `Svn=107183`); `2021-07-06 / 07-07` share one (`204a3fb5…`,
  `git=105481`).
- Versioning changes at **2020-12-04**: `Svn=NNNNNN` → `git=NNNNNN-<hash>`; the
  counters are unrelated (`Svn=107183` at 2020-10-02, then `git=103967`).

| endpoint | banner | exe SHA-256 |
|---|---|---|
| 2020-07-01 | `Date=2020-07-01_13_00 Svn=105768 GameVersion=3.3.0` | `dd5af3928a475d8b1abea85a7430acc57867c40c3020513e5b4a2ec1c9d80c2f` |
| 2022-01-21 | `date=2022-01-21_16_00 git=105899-1ab53b42729 GameVersion=3.3.0` | `0296a9db5a4eeb3ac312ebec5799a0af3b6cd87df924f1afda4ecd43d4be923f` |

The 2022-01-21 server build number `105899` equals the January 2022 **client**
build the lab already uses as its `STADIUM_PRE_2022_03_29` representative, so
the corpora join cleanly at that date.

### 4b. Full clients

Public item `archive.org/details/tm2020-archive`, 32 client archives.
**Exactly one is in-range:**

| archive | bytes | md5 | verified |
|---|---|---|---|
| `Trackmania 2021.7.8.1939.zip` | 1 966 239 131 | `b75e06df77c658798e0684959f50a2c9` | md5 ✓, size ✓, zip CRC ✓ ("No errors detected") |

Extracted `Trackmania.exe` — 43 183 896 bytes, SHA-256
`aabfb5229589112797e4420c1978dae4ecdb7f0abb9d0936b0b6657e6edff61e`, banner
`2021-07-07_08_00`, **`git=105481-88bf0d159f8`** — the *same build number* as
archived servers `2021-07-06 / 2021-07-07`. Client and dynamic server anchor
match exactly.

**There is no public full client from 2020 at all**; next-oldest is
`Trackmania 2022.1.21.1554.zip` (`9eb99fae6ccc4da584155624724ea557`). An
archive-coverage fact, not a claim that no such client existed — and the binding
constraint on implementation (§8).

---

## 5. Measured taxonomy — 43 builds × 259 probes

Probe corpus: 259 archived replays selected to span **60 distinct recording
client builds**, from 2020-07-07 to 2022-02. 125 probes are loaded by all 43
builds and form the physics-clean basis.

**Nine distinct behavioral classes:**

| # | builds | n | exact/125 | boundary into this class |
|---|---|---|---|---|
| 1 | 2020-07-01, 2020-07-02 | 2 | 0 | — (launch) |
| 2 | 2020-07-07, 2020-07-17 | 2 | 17 | 17 probes |
| 3 | 2020-07-10 | 1 | 19 | 2 probes **[thin]** |
| 4 | 2020-07-23 | 1 | 102 | **87 probes** |
| 5 | 2020-09-11 … 2020-11-16 | 6 | 117 | 15 probes |
| 6 | 2020-12-04 … 2020-12-18 | 3 | 120 | 3 probes |
| 7 | 2021-01-18 … 2021-04-08 | 11 | 121 | 1 probe **[thin]** |
| 8 | 2021-05-19 … 2021-05-31 | 2 | 120 | 1 probe **[thin]** |
| 9 | 2021-06-08 … 2022-01-21 | 15 | 23 | **97 probes** |

Class 3 sorts out of date order because classes are keyed by verdict vector, not
by date; 2020-07-10 differs from {07-07, 07-17} on 2 probes only.

### 5b. Fine structure inside class 9

All 15 post-2021-06-08 builds load all 259 probes, so restricted to that region
the common basis is the **full 259** and the region resolves further:

| builds | exact/259 | boundary into this class |
|---|---|---|
| 2021-06-08, 2021-06-09 | 109 | — |
| 2021-06-11 | 111 | 4 probes |
| 2021-06-12 | 112 | 1 probe **[thin]** |
| 2021-06-18 | 113 | 1 probe **[thin]** |
| 2021-07-01 | 114 | 3 probes |
| 2021-07-06, 2021-07-07 | 115 | 5 probes |
| 2021-09-29 | 120 | **11 probes** |
| 2021-10-12 … 2022-01-21 | 122 | 2 probes **[thin]** |

The 11-probe step at **2021-09-29** is the Fall 2021 update.

### 5c. A separate, non-physics axis: container format

`NOLOAD` transitions are file-format compatibility and are excluded from every
class computation: **2020-07-17**, **2020-12-04** (134 probes stop loading),
**2021-03-10** (all load again), **2021-06-08** (0 thereafter).

### 5d. Result-schema change (static marker)

Launch-era servers emit a **flat** validation schema — top-level `IsValid` /
`Time` / `NbCheckpoints`, no `ValidatedResult` / `DeclaredResult`, no `Desc`, no
`GameBuild`. The nested schema appears at **2020-07-17**. Called out because it
is an active trap (§9).

---

## 6. Official behavior notes (independent evidence)

From archived official Nadeo posts on `trackmania.com`.

| date | official statement | measured boundary |
|---|---|---|
| **2020-07-07** | "Fixed an issue where **frozen wheels would unfreeze instantly after leaving the ice**." Same post: "We've improved the way we validate records in order to detect any inappropriate time on every map…" | class 2 begins exactly 2020-07-07 (17 probes) |
| **2021-06-12** | "This new mode comes with **new physics** and innovations available throughout the game: **Water, Plastic Blocks, Animated & Dynamic items**" | class 9 begins 2021-06-08…06-12 (**97 probes**) |
| **2021-10-01** | "…many **new blocks and physics** in this Fall 2021 campaign. Plastic makes his first appearance in a seasonal campaign … **interactions with water are also added**" | 11-probe step at 2021-09-29 |

Reported the other way too: the **2020-09-11** official changelog
("[10/09/2020] New update and changelog!") covers netcode, UI, clubs, item/skin
editor and replay editor — **no driving-physics statement**. The 15-probe
boundary there is *measured but not officially announced*, and is not upgraded.

### In-binary marker

`using known-flawed game exe '%s'` is **absent from all six July 2020 server
executables and present in every build from 2020-09-11 onward**. The January
2022 server emits it against a 2020-12-22 client recording. This is Nadeo's own
machine-checked declaration that specific older client builds are behaviorally
untrustworthy. The predicate is computed, not a string table; localizing it is
follow-up work.

---

## 7. Code vs data: which axis moved

Per-build component hashes (`component_hashes.tsv`) across all 43 builds:

| component | distinct values |
|---|---|
| `TrackmaniaServer` (executable) | **39** |
| `Packs/dedicated_TMStadium.pak` (Stadium physics/collision data) | **14** |
| `Packs/dedicated.pak` | 13 |
| `Packs/resource.pak` | **1** (never changed) |
| `Packs/Trackmania.Title.Pack.Gbx` (campaign content) | 38 |

**At the largest boundary the cause is isolated to code:**

```
2021-05-31  exe 8fc19fa71472  stadium_pak 2c4464fe95be  ded_pak bac0b0d82978  title 6a59f546e735
2021-06-08  exe 405d86fc5d10  stadium_pak 2c4464fe95be  ded_pak bac0b0d82978  title 6a59f546e735
```

Identical Stadium physics pack, identical shared pack, identical title pack —
**only the executable differs, and 97 of 125 probes flip.** The June 2021
"new physics" change is in the **executable**, not in shipped data. (The lab
records the analogous Summer 2023 split as unresolved; this one is resolved.)

Stadium physics-pack transitions (the data axis) fall at 2020-07-23, 2020-09-11,
2020-10-02, 2020-12-04, 2020-12-10, 2021-01-18, 2021-06-11, 2021-06-12,
2021-06-18, 2021-07-01, 2021-07-06, 2021-09-29, 2021-10-12.

---

## 8. Implementation status

**A distinct 2020 release profile is proven behaviorally.** Under the standard
in force (complete closure, exhaustive field/call/RIP remap, no aliasing,
independent verifiers that trust no generator manifest, zero unclassified
carriers, deliberate-perturbation positive control, two fresh-process matches
with adjacent negatives, no selectable status until live trajectory control,
fail-closed when exact bytes are missing):

- **2020 epochs (classes 1–6): FAIL-CLOSED, catalog-only.** A build-128130
  native island must be generated from the exact preimage bytes of a
  representative **client**. **No 2020 full client exists in the public
  archive** (§4b). Generating an island from a *dedicated-server* binary would
  alias a different image's code into a client profile, which the standard
  forbids. These epochs are catalogued with their measured evidence and their
  exact blocking reason — the same treatment the lab already gives Spring 2022.
  Nothing generated, nothing selectable.
- **2021 post-Royal epoch (2021-06-18 … 2021-07-07): the one implementable
  target in the window.** Representative client `Trackmania 2021.7.8.1939`
  exists and is **fully verified** (md5, size, CRC); its `Trackmania.exe`
  (`aabfb522…`, `git=105481`) matches the dynamic server anchors
  `2021-07-06 / 07-07` exactly, and that pair sits in a measured class with a
  full 259-probe basis.
  **Closure enumeration has not begun.** Per the method requirement, no payload
  generation starts before 100 % of closure memory operands, LEAs, immediate
  field carriers, calls, RIP references and writes are enumerated and
  classified, provenance tracked, every helper identity and ABI verified, and an
  independent verifier passes with zero unclassified carriers.
  **Status: not started, fail-closed. Nothing is selectable.**

---

## 9. Controls

- **Determinism (identity control).** Two fresh processes, same build
  (2020-10-02), same 252 probes → **byte-identical verdicts**. The instrument is
  deterministic, so verdict differences are not run-to-run noise.
- **Positive control.** Runs first validate exactly on the build that recorded
  them: `17361`, `37901`, `8309` (recorded `2020-07-23_20_22`) validate exactly
  from **2020-07-23** onward and on **no earlier build**. `37364` (recorded
  `Svn=106088`) → `42.907` exact; `62644` (recorded `git=105849`) → `164.655`
  exact on 2022-01-21.
- **Negative control.** The same probes return `wrong simu` on other-era builds
  in the same batch — the instrument is not "everything passes". 9 probes
  validate on no archived server (their recording builds post-date the server
  corpus): the expected negative.
- **Hash-change negative control.** `2021-01-18 → 2021-01-20` and
  `2021-02-03 → 2021-02-08` each change the **executable** while Stadium pack
  and title pack stay identical — and both pairs land in the **same** behavioral
  class. An executable hash change is therefore demonstrably *not* sufficient
  for an epoch, which is exactly why 39 distinct executables are not 39 epochs.
- **Identical-binary control.** `2020-10-02 / 10-12 / 11-04 / 11-16` share one
  executable and byte-identical `dedicated_TMStadium.pak`, `dedicated.pak` and
  `resource.pak`; only `Trackmania.Title.Pack.Gbx` differs. Their small verdict
  differences are therefore **campaign-content effects, not car physics** — and
  they set the floor below which a boundary must not be called physics.
- **Instrument faults found and fixed, not explained away.** Each had already
  produced a plausible false epoch:
  1. *Flat result schema* — scored the launch builds 0-for-everything (a
     convincing fake epoch). Fixed; affected probes then validate on exactly
     those builds.
  2. *Port collision* — early servers bind the XML-RPC port even when only
     validating, so concurrent runs wrote empty logs. Fixed with per-build ports.
  3. *Daemonizing hang* — a detached grandchild holds the stdout pipe open, so
     the reader blocked forever after the child exited; one build hung the whole
     batch twice. Fixed with a bounded read plus a per-probe isolation mode, in
     which a hang is a recorded verdict for one probe instead of the loss of a
     build's entire vector.

### What is deliberately not claimed

- No byte-level cause is assigned to any boundary (only the code-vs-data axis at
  2021-06, §7).
- Boundaries marked **[thin]** rest on 1–2 probes and are reported as
  *distinguishable on this corpus*, not as settled epochs. Given the
  identical-binary control, a 1–2 probe boundary between builds whose executable
  and Stadium pack are unchanged should be treated as content, not physics.
- Classes 1–3 (July 2020) have era-matched probes but the corpus is thinnest
  there; their internal ordering is the least settled part of the taxonomy.
- The nine classes were derived **without reference to any hash**; the hash
  tables in §7 were used only afterwards, to attribute causes.
