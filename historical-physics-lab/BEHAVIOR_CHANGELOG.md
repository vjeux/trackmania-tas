# Trackmania 2020 — official vehicle behavior changelog

**What this is.** A behavior-first record of every *official* Trackmania 2020
driving-behavior change that is supported by observed code/data or by measured
execution. Community, restored and hidden cars are out of scope.

**Evidence rule (hard).** A row appears in the behavior sections **only** if a
specific observed code or data delta, or a measured run, supports it. An
official release note alone is never enough. Where a code delta exists but
cannot be mapped to a player-visible outcome with rigour, the delta is recorded
in **Appendix B (unresolved)** and *no* behavior claim is made. No number here is
estimated or inferred from a hash change. Controls per row — independent
verification, deliberate perturbation, fresh-process repeats, adjacent
negatives — are in `CONTROLS.md`, including the ones **not** satisfied.

**A probe-count flip is not a behavior claim.** The dedicated server can refuse a
run for reasons that have nothing to do with driving: a blacklisted recording
client, a container-format mismatch, a respawn rule. Those are separated out
before anything is published (§7). What is published as a magnitude comes from
one channel only: runs that **complete on both sides** and return a **different
lap time**.

**Status vocabulary**

| Status | Meaning |
|---|---|
| **Measured** | Produced by executing the game/dedicated-server, or read out of shipped bytes with a decoder. Reproducible from `TEST_PLAN.md`. |
| **Derived** | Exact arithmetic on Measured values, or direct reading of instruction semantics. No modelling. |
| **Officially described** | Nadeo said it. Recorded only alongside an observed code/data delta; the *magnitude* stays Unresolved unless separately measured. |
| **Unresolved** | Not established by available evidence. Named explicitly, never silently dropped. |

**Units.** Speeds km/h, distances m, times in seconds with a decimal (`63.546`),
input axes normalised to full scale = 1.0.

---

## 1. Stadium (the default car)

### 1.1 Behavior changes with direct evidence

| # | Boundary | Player-visible behavior | Numbers | Status |
|---|---|---|---|---|
| **S-0** | **Between 2020-07-23 and 2020-09-11** | A small number of recorded runs, replayed from identical inputs, finish at a **different lap time**; the rest are unaffected. Most of the affected runs shift by hundredths of a second, two by more than a fifth of a second. | Of **194** runs completing on both builds, **168 unchanged**, **26 changed** (6 slower, 20 faster). Magnitudes: **19 ≤ 0.020 s**, 5 between 0.021 and 0.200 s, **2 > 0.200 s**; largest **−1.954 s** on a 12.981 s run (14.935 → 12.981), next **−0.403 s**. Spread over **15 distinct maps**. | **Measured** |
| **S-1** | **2021-06-08** (between server builds 105091 and the 2021-06-08 build) — the "new physics" release | **The biggest driving change in the game's history to date.** Replayed from identical inputs, **two thirds of all runs that still finish come back at a different lap time**, in both directions, across essentially the whole map corpus. This is not confined to one surface or feature: runs on maps recorded up to eleven months earlier, which contain none of the update's new elements, change too. | Of **139** runs completing on both builds, only **47 unchanged**; **54 slower**, **38 faster** — **92 of 139 (66%) change time**. Median change **0.016 s**, mean **0.109 s**, largest **+1.488 s** on a 25.470 s run (**+5.84%**), then **+0.962 s** on 15.036 s (**+6.40%**) and **+0.887 s** on 29.188 s (**+3.04%**). Changed runs span **40 distinct maps**; 10 maps carry both changed and unchanged runs. | **Measured** |
| **S-2** | **2022-03-29** (between server builds 112349 and 112449) | An ice lap completable from 2022-03-29 onward is **not completable** before it, from identical recorded inputs. | Finish **63.546 s** on all 8 builds 2022-03-29 → 2022-06-21; no valid finish on all 16 builds 2021-12-02 → 2022-03-25. | **Measured** |
| **S-3** | **2024-05-22** (global input path; all families) | Analog steering now lands **exactly** on the requested value once converged. Before, the smoothed value stopped inside a tolerance band and was never set equal to its target. | Snap fires when `\|stored − target\| ≤ max(1.0, max(\|stored\|,\|target\|)) × 1×10⁻⁵`. On an axis normalised to 1.0 the bound is **1×10⁻⁵ of full lock (0.001%)**: post-fix residual **0**, pre-fix **>0 and ≤1×10⁻⁵**. | **Measured** + **Derived** |

**S-1 is isolated to code, not data.** Across that boundary the Stadium physics
pack (`dedicated_TMStadium.pak`), the shared pack (`dedicated.pak`) and the
campaign title pack are **byte-identical**; only the executable differs. So the
June 2021 change lives in the executable. Which instructions changed is
**Unresolved** — no byte-level cause is assigned.

**Why S-1's numbers are trustworthy.** Two independent negative controls, both
computed over the same corpus and both returning exactly zero:

* **Campaign content alone changes no lap time.** Builds 2020-10-02 / 10-12 /
  11-04 / 11-16 share one executable and byte-identical physics packs, differing
  only in the campaign title pack. Across all three adjacent pairs: 203-204 runs
  complete, **0 change time**.
* **An executable hash change alone is not a behavior change.** 2021-01-18 →
  01-20 and 2021-02-03 → 02-08 each change the executable with packs identical.
  Both: 123 complete, **0 change time**. This is why 39 distinct server
  executables are not 39 epochs.

**S-1's official counterpart.** Nadeo's 2021-06-12 Royal post says the mode
"comes with new physics". The measured boundary sits at 2021-06-08, four days
earlier — the change ships before the post. The post is *consistent with* S-1 and
is not the evidence for it; the numbers above are.

**S-0 caveat.** No archived server exists between 2020-07-23 and 2020-09-11, so
the boundary is bracketed by those two dates, not pinned. With 26 of 194 runs
changing it clears both negative controls above, but it is an order of magnitude
weaker than S-1 and no official note describes it — the 2020-09-11 changelog
covers netcode, UI, clubs and editors, with no driving statement.

### 1.2 Current-build reference measurements (build 128130)

Absolute reference for the current Stadium car on a fixed ice-booster + jump
control map, fixed tape (full throttle, no steer, 22.000 s key-hold, 431 keydown
batches, 4543 trace records).

| Event | Value | Status |
|---|---|---|
| Speed entering the ice booster | **198.962 km/h** | Measured |
| Speed leaving the ice booster | **294.848 km/h** | Measured |
| Speed gained across the booster | **+95.886 km/h** | Derived |
| Speed at take-off | **337.568 km/h** | Measured |
| Take-off instant | **6.84517 s** | Measured |
| Airtime | **1.46540 s** | Measured |
| Landing instant | **8.31057 s** | Measured |
| Landing height (z) | **677.47345 m** | Measured |
| Finish-plane crossing | **12.79075 s** at **345.488 km/h** | Measured |

**Discrimination floor.** Three patched variants of the same build agree to five
decimals at every event except finish-plane speed, which spans **0.080 km/h**.
Cross-epoch speed claims on this map must exceed that; timings are good to 10⁻⁵ s.

**Open discrepancy.** The source bundle's prose says `12.88075 s` for the
finish-plane crossing where its own table says `12.79075 s`. The table value is
published; the 0.09 s conflict is **Unresolved** (`RR-01`). The parent has since
banked an exact Sep-30 trace/control — reconciling `RR-01` against it is the next
run, and is deliberately **not** pre-empted here.

### 1.3 Stadium windows with no behavior row

| Window | Established | Missing |
|---|---|---|
| 2020-07-01 → 2020-07-23 | Four adjacent boundaries measured; the corpus is thinnest here. 2020-07-10 → 07-17 changes 2 runs (both 0.009 s); 07-07 → 07-10 and 07-17 → 07-23 change **none**. Launch builds (2020-07-01/07-02) **core-dump** on later probes — reproduced here — so they yield no verdict at all. | Nadeo's 2020-07-07 note (frozen wheels unfreezing instantly after leaving ice) has **no measurable magnitude** on this corpus. Two runs at 0.009 s is at the level the negative controls call noise-adjacent, so no row. |
| 2020-09-11 → 2021-05-31 | Fourteen adjacent boundaries measured. Every one changes **0 or 1** run; three single-run changes (0.958 s, 0.009 s, and one at 2021-06-08's predecessor). | Single-run boundaries do not clear the controls. The audit marks these **[thin]**; they are *distinguishable on this corpus*, not settled epochs. |
| 2021-06-09 → 2022-01-21 | Seven adjacent boundaries measured after the big one: 06-09→06-11 changes 4 runs (max 0.200 s), 06-12→06-18 changes 4 (max 1.519 s), 06-18→07-01 changes 4 (max 0.190 s), 07-01→07-06 changes 2 (1.549 s), 07-07→09-29 changes 3 (max 0.104 s). 09-29 onward through 2022-01-21: **0 changes** across five boundaries. | These are 2-4 run boundaries. The **2021-09-29** step is the Fall 2021 update and is officially described ("new blocks and physics… interactions with water"), but 3 changed runs is below what the controls support as a physics epoch. **Unresolved magnitude.** |
| 2022-03-29 → 2022-09-20 (Spring 2022) | Post-boundary behavior measured on servers through 2022-06-21. | No full client in the public archive for the 252-day window; no server after 2022-06-21. Client-side epoch end **Unresolved**. |
| Staged 2022-09-21 / public 2022-10-01 (Fall 2022) | Sep. 30 and Oct. 6 share the canonical digest, so October 1 is not the executable boundary. A matched exact Sep. 30 / current stock / current+V5 control is now measured: exact vs stock exceeds 1 m at `3.840 s` with mean/max `6.109 / 10.016 m`; exact vs V5 exceeds 1 m at `3.700 s` with worse `7.135 / 11.200 m`; stock vs V5 mean/max is only `1.357 / 2.454 m`. Exact/stock/V5 peak speeds are `398.520 / 400.213 / 400.348 km/h`. | V5 is demonstrably closer to stock than exact Sep. 30 and is rejected. The historical endpoint difference is measured, but the responsible force-law component remains unresolved; Fall stays fail-closed. |
| Staged by 2023-06-23 (Summer 2023) | Handler and tracked packs both change between 2023-05-05 and 2023-06-23; 06-23 and 07-10 then match. | Causal split not localised; no dynamic oracle after 2022-06-21. Officially described ice change has **no measured magnitude**. |
| 2023-06-23 → current (128130) | The 25 Stadium runtime tunings match by name, order, scalar and key metadata between the Fall-2022 client and 128130; current only appends three non-Stadium Wood entries. | No later Stadium force-law change is evidenced. Absence is **not** claimed. |

---

## 2. Snow

Separate official vehicle family, released 2023-11-21 — not a Stadium epoch.

| # | Boundary | Player-visible behavior | Numbers | Status |
|---|---|---|---|---|
| SN-1 | **2024-02-27** | The Snow car's **body collision volume changes shape**: its three body primitives stop being spheres and become ellipsoids. The envelope loses height; the middle body volume gets longer. Lateral width unchanged to within half a millimetre. Wheel collision untouched. | Member 1: sphere **1.195428014 m** → ellipsoid **[1.195, 1.100, 1.195] m** (second axis **−0.095428014 m**). Member 2: **0.969449997 m** → **[0.969, 0.800, 1.200] m** (**−0.169449997 m**; third axis **+0.230550003 m**). Member 3: **1.119449973 m** → **[1.119, 1.000, 1.119] m** (**−0.119449973 m**). Four wheel spheres unchanged at **0.469999999 m**. | **Measured** + **Derived** |
| SN-2 | **2024-05-22** | Same global analog snap as S-3. | Residual bound **1×10⁻⁵ of full lock**; post-fix **0**. | Measured + Derived |

The collision file is `Vehicles\Cars\CarSnow\SnowCar.Shape.Gbx`, class
`0x0900C000` (`CPlugSurface`), 7-member compound: pre-Feb 1,123 B
(`82a0822220468e50…`), post-Feb 1,147 B (`ef0ebee29e98faec…`), current 1,151 B
(`7ea1385e37ecaa30…`). Parent item `CarSnow.Item.Gbx` is **byte-identical**
across the boundary (1,900 B, `1f7b1bc03a67d7cf…`) — the negative control.

Axis labelling ("height") assumes Y-up and is **Derived**; the nine numbers are
Measured and stand without it. What the geometry does to *driving* is not
claimed — `RR-02` measures it.

**No behavior row** for the 2023-11-21 release baseline, **2024-01-09**
(`SetPlayer_Delayed_` scripting fix — only the executable and updater manifest
change, all 18 packs byte-identical, wrappers structurally identical, cause
downstream and not localised), or the February **action-key re-ranging**
(input-routing delta observed; the announced semantics are not established from
those bytes). See Appendix B.

---

## 3. Rally

Released 2024-02-27.

| # | Boundary | Player-visible behavior | Numbers | Status |
|---|---|---|---|---|
| RA-1 | **2024-05-22** | Same global analog snap as S-3. | Residual bound **1×10⁻⁵ of full lock**. | Measured + Derived |

**2024-04-02 custom ice — no behavior row.** The only Rally-specific delta is
`CarRally.Item.Gbx` (3,056 → 3,057 B), semantically one model path
`Models\RallyCar\` → `Models\CarRally\`. The encrypted Rally physics-model entry
is **bit-identical** across the boundary (5,696 B, `a0a4c1fe0e771ca7…`) — a
negative control proving no tire/engine/force-law blob changed. A path string
cannot be converted into a quantified ice-behavior change. **Unresolved**
(`RR-07`).

---

## 4. Desert

Released 2024-05-22; closest archived payload is the 2024-04-30 staging build.
**No Desert behavior row exists**: no officially described post-release driving,
hitbox or control change through the 2026 changelogs (8135, 8256, 8442, 8717,
8952), and later hash churn is not behavioral evidence. Candidate static
boundaries at 2024-06-28 and 2024-12-12 are in Appendix A and are **not**
epochs. No Desert control baseline has ever been measured (`RR-08`).

---

## 5. What the probe-count flips actually contain

The 2020-2021 audit reports its boundaries as counts of probes whose verdict
flips — 87 at 2020-07-23, 97 at 2021-06-08. Decomposing those flips by the
server's own stated reason shows they are **not all physics**, which is why this
changelog republishes them as time magnitudes instead.

At **2021-05-31 → 2021-06-08**, over the whole matrix, the flips decompose as:

| Reason the verdict changed | Count | Is it a driving change? |
|---|---|---|
| Run still finishes, at a **different time** | ~92 | **Yes — this is the magnitude published as S-1** |
| Physics disagreement (`wrong simu`, no finish) | 47 lost / 22 gained | Yes, but yields no magnitude |
| **Refused: `using known-flawed game exe`** | **16 lost** | **No — validation policy** |
| Respawn-rule rejections | 2 | No |

The refusal count is not incidental: `using known-flawed game exe` appears on
**18** matrix rows at 2021-06-08 against **2** at 2021-05-31, and 1,211 rows
matrix-wide. It is Nadeo's server declaring specific *recording client builds*
untrustworthy and declining to validate them — the marker is absent from all six
July 2020 executables and present from 2020-09-11 onward. **A run refused this
way was never simulated**, so counting it as a physics flip would manufacture an
epoch out of a policy change. Container-format `NOLOAD` transitions
(2020-07-17, 2020-12-04, 2021-03-10, 2021-06-08) are excluded for the same
reason, as the audit itself does.

---

## 6. Omitted epochs and boundaries (explicit)

1. **2020-07-01 → 07-23:** four boundaries measured, none clears the controls.
   Launch builds core-dump on later probes.
2. **2020-09-11 → 2021-05-31:** fourteen boundaries, all 0-1 changed runs.
3. **2021-06-09 → 2022-01-21:** seven boundaries, 0-4 changed runs each,
   including the officially described **Fall 2021 (2021-09-29)** step —
   magnitude below control support.
4. **Spring 2022:** no client in the archive.
5. **After 2022-06-21:** no dedicated server exists; every later epoch depends on
   exact-client execution.
6. **Fall 2022 and Summer 2023 magnitudes:** unresolved.
7. **Snow 2024-01-09 and action-key re-ranging; Rally 2024-04-02:** observed
   deltas, no behavior mapping.
8. **Desert post-release:** no confirmed change.
9. **No era-matched replay control exists for any Snow, Rally or Desert epoch.**
10. **Nothing after client 2026-04-28** examined; current build is 128130.
11. **Behavior quantities never measured in any epoch:** steering response curve,
    grip/slip thresholds, wall repulsion impulse, reactor response, water bounce,
    bobsleigh steering, dirt/wet response, per-epoch acceleration curves. Lap
    time is an *aggregate* — S-0 and S-1 prove the car behaves differently, not
    which force law changed. `TEST_PLAN.md` specifies the micro-maps for each.

---

## 7. Controls, and what they caught

Full record in `CONTROLS.md`. Four things worth stating here:

* **S-1 was re-measured independently on this node**, with a harness sharing no
  code with the audit's. The three largest changes reproduced **to the
  millisecond**: 25.470 → 26.958, 15.036 → 15.998, 29.188 → 30.075. Two fresh
  processes, byte-identical output.
* **Two instrument defects were caught by controls before publication.** (1) A
  float scanner stepped 4 bytes and saw only one alignment phase, silently
  missing 3 of 7 collision values. (2) The validator-output parser never matched
  the server's real quoted-JSON schema at all, and fell through to substring
  matching on the end-of-run **summary counters** — so a genuine finish printing
  `Wrong Simu : 0% ( 0)` was scored as a failure. That defect inverts boundary
  verdicts wholesale. Both fixed, both now regression-tested against captured
  real output (12/12 tests pass).
* **The negative result is published as a result.** A current-era `.Map.Gbx`
  cannot be loaded by 2020-2022 servers. The fix was not to force it: era-correct
  replays **embed their own map**, which is what makes the whole 2020-2021 window
  measurable at all.
* **Policy refusals are separated from physics** (§5), which lowers the June 2021
  boundary from "97 probes flip" to a defensible "92 of 139 completing runs
  change time" — a smaller claim, and a real one.

---

## Appendix A — technical traceability

Current target: banner `2026-01-28_13_00`, build `128130`, GameVersion `3.3.0`,
exe SHA-256 `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda`,
CarSport handler RVA `0x851f00`.

**S-1 boundary builds.** 2021-05-31 = `date=2021-05-31_18_00 git=105091-b6879495a50`,
exe `8fc19fa7147254a0a92f29646d493884857b612f884c015e3d5728ec1b0d0ab4`.
2021-06-08 = exe `405d86fc5d10eac36c0917f23eebc8d1f2a1014b848c9ab78352d05e1a50ff81`.
Across the pair `dedicated_TMStadium.pak` (`2c4464fe95be`), `dedicated.pak`
(`bac0b0d82978`) and the title pack (`6a59f546e735`) are unchanged.
Reproduced probes: `2020-10-09_10_58__10640` (map `8TrzhmnV4NAY8x4LNqlVjbOa3og`),
`2020-09-25_18_53__19109` (`SsCdL6nGC__n8UrYnsX8xaqnjCh`),
`2021-03-30_19_17__33095` (`Yakz8xDlVWDfVCfXxW2_paCaHil`).

**S-2 boundary builds.** WRONG side: 2022-03-25, build 112349,
`a5536ac7dc242640…`. EXACT side: 2022-03-29, build 112449; last confirmed
2022-06-21, build 113135, `6623e71dad1ce1cf…`.

**S-3 / SN-2 / RA-1 analog snap.** `movss [rbp+rcx*4+0x74], xmm4`
(`F3 0F 11 64 8D 74`), RVA `0x2c360e`, **unique in the image**, file offset
`0x2c2a0e`. Guard block at file offset `0x2c29c7`, all operands classified:

```
movss  xmm5, [rbp+rcx*4+0x74]   ; stored (smoothed) value
movss  xmm4, [rbp+rcx*4+0x94]   ; target value
andps  xmm3, xmm7 / andps xmm1, xmm7   ; xmm7 = 0x7FFFFFFF  (abs mask)
maxss  xmm0, xmm1               ; max(|stored|, |target|)
subss  xmm2, xmm4 ; andps xmm2, xmm7   ; |stored - target|
maxss  xmm1, xmm0               ; max(xmm8, that)    xmm8 = 1.0f
mulss  xmm1, xmm9               ; xmm9 = 1e-5f
comiss xmm1, xmm2 ; setae al ; test eax,eax ; je ...
movss  [rbp+rcx*4+0x74], xmm4   ; <-- the snap: stored := target
```

`.rdata` constants (after the 0x1000 `.text`/`.rdata` section-delta correction):
`0x1d20000` = `ff ff ff 7f` (abs mask), `0x1d1d7c8` = `1.0`, `0x1d1d134` = `1e-5`.
Pre-May builds carry six `nop` bytes there (2024-03-19 RVA `0x273e00`,
2024-04-30 `0x274350`, per the source bundle — not re-read here).

**SN-1 collision files.** Sizes/hashes as in §2. Raw-read offsets, pre-Feb:
`0x31, 0x47, 0x5d` (body radii), `0x73, 0x89, 0x9f, 0xb5` (wheels); post-Feb:
`0x31/0x35/0x39`, `0x4f/0x53/0x57`, `0x6d/0x71/0x75` (ellipsoid axes),
`0x8b, 0xa1, 0xb7, 0xcd` (wheels).

**Corpus and grid.** 43 dated dedicated-server builds 2020-07-01 → 2022-01-21
(39 distinct executables; 56 endpoints probed, 0 errors); 259 archived official
Stadium replays spanning 60 recording client builds; 9,700 matrix cells. Client
archive: 32 archives, 31 distinct executables, 11 distinct normalised CarSport
handlers, 2021-07-08 → 2026-04-28; exactly one client in the 2020-2021 window
(`Trackmania 2021.7.8.1939`, exe `aabfb5229589…`, `git=105481`, matching servers
2021-07-06/07-07).

**Component axes across the 43 builds.** Executable 39 distinct;
`dedicated_TMStadium.pak` 14; `dedicated.pak` 13; `resource.pak` **1** (never
changed); title pack 38.

**Snow input routing (observed, unmapped).** `cmp eax,0x18; je` added before the
first action queue; current retains `83 F8 18 74 18` at RVA `0x2b8c49`.

**Snow delayed-player call sites (compatibility shim, not historical bytes).**
RVAs `0x1342927`, `0x1342ab7`, `0x1342c47`.

**Rally item payloads.** Release/2024-03-19 3,056 B `a1d5cdcd21ed4b15…`;
post-fix 3,057 B `7cf6976abe68c891…`; encrypted physics-model entry identical
across the boundary.

## Appendix B — observed deltas with no behavior mapping

| Boundary | Observed delta | Why no behavior row |
|---|---|---|
| 2020-07-07 | Official: frozen wheels unfreezing instantly after leaving ice | Only 2 runs change time nearby, both 0.009 s — below what the controls support. |
| 2021-09-29 (Fall 2021) | Official: "new blocks and physics… interactions with water"; Stadium pack changes | 3 of 115 completing runs change (0.003-0.104 s). Magnitude unresolved. |
| 2021-06-08 | 97-probe flip includes 16 policy refusals and 2 respawn rejections | Those are not driving changes; only the time deltas are published. |
| 2022-03-29 | Boundary follows the executable; both builds share the normalised CarSport handler | Responsible code outside the normalised root, not localised. Outcome measured (S-2), mechanism not. |
| Fall 2022 | `Trackmania.exe` + title pack change; 18 of 20 tracked files identical | Reconstructed adapter matches stock on every measured event. |
| Summer 2023 | Handler *and* packs change 2023-05-05 → 06-23 | Causal split not localised; no oracle after 2022-06-21. |
| Snow 2024-01-09 | Only exe + updater manifest change; wrappers structurally identical | Cause downstream, not localised. |
| Snow 2024-02-27 (action keys) | `cmp eax,0x18; je` added | Route change does not establish the announced re-ranging semantics. |
| Rally 2024-04-02 | Item path `Models\RallyCar\` → `Models\CarRally\`; physics blob bit-identical | A path string yields no quantified ice-behavior change. |
| Current build | Appended tunings `Wood0`, `Wood_20240101_MoreAccelForSlope(2)` | Non-Stadium; no measured behavior. |
| 2020-2021 | Dated tuning identifiers (`IceDrift2006xx`, `AntiWallHit201021`, `Water2101xx`/`210415`, `Reactors200605`, `ExperimentalBobsleighSteer`, `NoWiggle`, `NoWiggleAjusté`) | Names and dates measured; semantics and scalars not recovered. |
| `using known-flawed game exe` | Absent from all six July 2020 executables, present from 2020-09-11; 1,211 matrix rows | A validation-policy predicate. Its computation site is not localised, and it is never a physics verdict. |

## Appendix C — provenance

| Input | Identity |
|---|---|
| Current client executable | SHA-256 `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda` |
| Source evidence bundle | `tm-historical-physics-official-v5-failclosed.tar.zst`, `8cc088de3425697071dff85dd4527b0cb935b2a176f4cf28c2d8e53a172e6a42` |
| 2020-2021 audit | `/home/vjeux/tm2020-audit/`; matrix `matrixW_all.tsv` `9eb13b537d64d686…`; corpus `probe-corpus-259.tar.gz` `982b52a7adabf92c…`; report `64d0b68c23435d71…` |
| Dedicated-server archive | `files.v04.maniaplanet.com/server/TrackmaniaServer_<date>.zip`, 43 dated builds |
| Client archive | Internet Archive `tm2020-archive`, 32 clients |
| Snow collision shapes | 1,123 B and 1,147 B `CPlugSurface` files, decoded and independently re-read |
| Booster control run | map `89a303c02a18d9f3973012f3921dc5fdab07c44c0b307f97a6e29ce6c66039e7`, 4,543 records |

Companions: `measurements.json`, `CONTROLS.md`, `CONTROLS.json`, `TEST_PLAN.md`,
`RUN_REQUESTS.json`, `tmphys.rs` (std-only Rust, 12 passing tests).
