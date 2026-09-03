# Verification controls for every measured row

Required standard: no guessed behavior; complete operand classification where
native payloads are involved; independent verification; a deliberate-perturbation
positive control for the harness; two fresh-process matches; adjacent negative
controls. This file records what was actually run — including what is **not**
satisfied.

Executed 2026-09-03 on devvm42752. Commands in `TEST_PLAN.md`; tool `tmphys.rs`
(std-only Rust, **12/12 tests pass**).

---

## S-1 — the 2021-06-08 Stadium change (largest row in the document)

| Control | Requirement | Result |
|---|---|---|
| **Independent verification** | Reproduce the audit's numbers with a harness sharing no code | **PASS.** My own stager/parser re-ran the three largest cases on both builds and reproduced them **to the millisecond**: 25.470 → 26.958, 15.036 → 15.998, 29.188 → 30.075. The server's own `Desc` strings were captured verbatim (`race finished, time is worse. (25470 < 26958)`). |
| **Two fresh-process matches** | Same answer in separate processes | **PASS.** Every verification was run twice into separate working trees; outputs byte-identical both times (verdicts *and* times). |
| **Adjacent negative control (probe level)** | Runs that must NOT change, do not | **PASS.** Probes `2020-07-07_23_07__1065` (7.442 s) and `__1066` (7.051 s) return identical times on both sides of the boundary in the same batch. |
| **Adjacent negative control (cause level) — content** | Campaign content alone must not move lap times | **PASS.** Builds 2020-10-02/10-12/11-04/11-16 share one executable and byte-identical physics packs, differing only in the title pack. Across all three adjacent pairs: 203-204 runs complete, **0 change time**. |
| **Adjacent negative control (cause level) — hashes** | An executable hash change alone must not imply behavior | **PASS.** 2021-01-18→01-20 and 2021-02-03→02-08 change the executable with packs identical: 123 complete, **0 change** on both. |
| **Policy/physics separation** | A refusal must never be counted as a physics flip | **PASS.** `using known-flawed game exe` is isolated as its own verdict (`POLICY_KNOWN_FLAWED_EXE`), tested, and excluded from all magnitudes. 16 of the boundary's flips are refusals; they are reported in §5 of the changelog and contribute nothing to S-1's numbers. |
| **Code-vs-data isolation** | — | **PASS.** Stadium pack, shared pack and title pack byte-identical across the boundary; only the executable differs. |
| **Operand classification** | — | **NOT POSSIBLE.** No byte-level cause is localised inside the executable, so there are no operands to classify. The changelog states the outcome as Measured and the mechanism as Unresolved; nothing is generated or patched. |

## S-0 — between 2020-07-23 and 2020-09-11

| Control | Result |
|---|---|
| Independent verification | **PARTIAL.** Computed by my own analysis over the audit's matrix; the individual runs were not re-executed here. The two cause-level negative controls (above) apply and are passed. |
| Adjacent negatives | **PASS** — 30 of the 42 adjacent boundaries in the same sweep change **zero** runs, so the instrument is not "everything changes". |
| Boundary pinning | **FAIL — bracketed only.** No archived server exists between those dates. Stated as a bracket, not a date. |
| Magnitude honesty | 26 of 194 runs, 19 of them ≤ 0.020 s. Published with the full band distribution rather than the headline maximum. |

## S-2 — 2022-03-29

| Control | Result |
|---|---|
| Adjacent negative | **PASS.** 2022-03-25 (112349) `WRONG_SIMU` / −1; 2022-03-29 (112449) exactly 63.546. |
| Repeat breadth | **PASS.** 16 builds consistent on the failing side, 8 on the succeeding side. |
| DNF guard (positive control on the parser) | **PASS.** 2022 servers report a DNF as a *present* `ValidatedResult` with `Time: -1`; a unit test asserts this parses as `DNF_TIME_NEGATIVE`, never a finish. |
| Two fresh processes | **NOT SATISFIED** — single run per build (`RR-04`). |
| Operand classification | **NOT POSSIBLE** — cause outside the normalised handler, not localised. |

## S-3 / SN-2 / RA-1 — analog snap, 2024-05-22

| Control | Result |
|---|---|
| Complete operand classification | **PASS.** Destination = stored smoothed slot (`+0x74`); source `xmm4` = target slot (`+0x94`); `xmm7` = `0x7FFFFFFF` abs mask; `xmm8` = `1.0f` floor; `xmm9` = `1e-5f` tolerance; `comiss`/`setae` comparison. No unclassified operand in the decisive block. |
| Independent verification | **PASS.** Constants located by disassembly and read independently by direct byte scan; the two agree. The `.text`/`.rdata` section-delta correction is itself controlled — the abs mask reads exactly `ff ff ff 7f` at the corrected offset and not at the uncorrected one. |
| Deliberate perturbation | **PASS.** Poking `3.0` into `0x1d1d134` of a copy makes the reader return `3`; the original returns `1e-5`. |
| Two fresh processes | **PASS.** |
| Uniqueness | **PASS.** The six-byte store occurs exactly once in the image. |
| Adjacent negative | **PARTIAL.** The pre-May six-nop state is carried from the source bundle; those builds were not re-read here (`RR-03`). |
| Behavioral confirmation | **NOT DONE** (`RR-03`). |

## SN-1 — Snow collision geometry, 2024-02-27

| Control | Result |
|---|---|
| Independent verification | **PASS.** A raw f32 sweep (stride 1, no structural parsing) returns all 7 pre-Feb values and all 13 post-Feb values at the documented offsets, matching the structural decoder exactly. |
| Deliberate perturbation | **PASS.** Poking `2.0` into offset `0x31` makes the decoder report `radius=2.000000000` for member 0 and leaves members 1-3 unchanged. |
| Two fresh processes | **PASS.** |
| Adjacent negative | **PASS.** Parent item byte-identical across the boundary; four wheel spheres unchanged. |
| Operand classification | **N/A** — pure data, no native payload. |
| Behavioral confirmation | **NOT DONE** (`RR-02`). |

## REF-01 — current-build reference run

| Control | Result |
|---|---|
| Discrimination floor | **PASS.** 0.080 km/h on finish-plane speed; everything earlier identical to 5 decimals. |
| Internal consistency | **FAIL — one open discrepancy.** Bundle prose `12.88075 s` vs its own table `12.79075 s`. Published as unresolved; `RR-01`, to be reconciled against the newly banked exact Sep-30 trace/control. |
| Two fresh processes | **NOT SATISFIED** (`RR-01`). |

---

## Harness controls, and the two defects they caught

| Control | Result |
|---|---|
| Unit tests | **12/12 pass**, including three written against *captured real server output* rather than invented strings. |
| **Defect 1 — float scanner alignment** | The first independent read of the Snow collision file silently missed 3 of 7 values: the scanner stepped 4 bytes and therefore only ever saw one alignment phase. Fixed (stride 1 default), control re-run clean. Caught before publication. |
| **Defect 2 — validator parser never read the real schema** | The parser matched bare `Time:` / `IsValid:`, which the server never emits — its report is quoted JSON (`"Time" : 26448`). Parsing therefore produced nothing, and the verdict fell through to substring-matching the end-of-run **summary counters**, where `Wrong Simu : 0% ( 0)` appears on *successful* runs. A genuine finish was scored `WRONG_SIMU`. **This defect inverts boundary verdicts wholesale** and would have made every epoch conclusion worthless. Found because my independent re-run disagreed with the audit — the disagreement was the control working. Fixed by parsing the real schema; two regression tests pin it (`summary_counters_do_not_override_a_valid_run`, `real_valid_report_is_a_finish`). |
| Determinism | **PASS.** Every validation batch run twice into separate trees; byte-identical. |
| Negative result published | A current-era `.Map.Gbx` cannot be loaded by 2020-2022 servers — reported as a container-format blocker, not as absence of change. The remedy (era-correct replays embed their own map) is what made the 2020-2021 window measurable. |
| Crash observed and recorded | Launch builds (2020-07-01/07-02) **core-dump** on later probes — reproduced here. A crash is not a verdict and yields no epoch. |

## Not yet satisfied

1. S-0: individual runs not re-executed on this node; boundary bracketed, not pinned.
2. S-2 and REF-01: two fresh-process matches (`RR-04`, `RR-01`).
3. S-3: pre-May adjacent negative re-read on this node (`RR-03`).
4. SN-1, S-3: behavioral confirmation (`RR-02`, `RR-03`).
5. Byte-level cause for S-0, S-1 and S-2: not localised — and deliberately not guessed.

No primary-changelog row depends on an unsatisfied control for its **stated**
claim: where a control is missing, the claim is narrowed to what the satisfied
controls support and the remainder is named Unresolved.
