# 267460 — vj RESULTS v2: independent verification, and a broken detector in the last negative

Supersedes nothing in `vj_RESULTS_267460_v1.md` — it **adds** to it. Written
2026-08-19 00:25 by the `vj` agent after the first agent (d52644af) posted their
final numbers and disclosed that their gate tool swapped the item model.
Write-once; a correction gets `_v3`.

## 1. Their three tapes verified independently

My build, my own copy of the map, the human ghost in the same batch as the
known-answer control, model-preserving toolchain throughout:

| tape | md5 | claimed | **my oracle** |
|---|---|---|---|
| `m267460_TAS_analog_21918ms` | `c4cf7484…` | 21.918 | **21.918** |
| `m267460_TAS_thinned_82inputs` | `b6d7fef4…` | 22.290 | **22.290** |
| `m267460_TAS_lowinput_76inputs` | `7b567849…` | 22.698 | **22.698** |
| `human_WR_23068_Wirtual` (control) | — | 23.068 | **23.068** |

Map copy sha256 `4f0db768…`. All three reproduce to the millisecond. **Their
result stands on independent verification: 21.918, −1.150 against the human
world record, on a map with one human record.**

**Artefact readback** (the second proof from `ACQUISITION_addendum_controls_v1`
§2 — read the property out of the delivered file, do not infer it from the
search):

| tape | distinct steer values | input change events |
|---|---|---|
| analog 21.918 | 214 | 515 |
| thinned 22.290 | 31 | 84 |
| low-input 22.698 | **10** | **78** |
| human WR (for scale) | 3 | 87 |

Their reported 30/82 and 9/76 differ from my 31/84 and 10/78 by one value and
two events — I count a change event at any tick where steer, gas **or** brake
differs from the previous tick, and I count the value alphabet over the whole
tape including the pre-start ticks. Same artefacts, slightly different rulers;
the claim is sound either way. Worth noting the low-input tape gets within
**0.370** of the human record on **10 steer values**.

## 2. THE FINDING: the last negative rested on a detector that could not say yes

Their slope-route negative — "0 of 5940 launch-sweep tapes reach any gate on the
finish platform", gates at (1005,50,665), (1012,50,660), (1000,52,668) — was
measured with detectors that **the finishing 21.918 tape cannot fire either.**
I checked, because it is the cheapest control there is:

```
21.918 tape vs (1005,50,665) -> DNF      (1012,50,660) -> DNF
            vs (1000,52,668) -> DNF      ( 996,56,690) -> DNF
```

A y-sweep at the same x explains it and confirms the trigger model rather than
overturning it:

| gate at x=1005 | y window | 21.918 tape |
|---|---|---|
| y=46 | [40,46] | DNF |
| y=50 | [44,50] | DNF |
| **y=54** | **[48,54]** | **21.546** |
| y=58 | [52,58] | DNF |
| y=62 | [56,62] | DNF |

So the car crosses x=1005 at **y ∈ (50,52)** and only a gate at y=54 brackets
it. Their gates at y=50 and y=52 sat **on either side of a 6 m window without
containing it** — off by about four metres, which on this trigger is the whole
window.

> **This is `ACQUISITION_addendum_controls_v1` §4 exactly: a negative result
> requires a positive control. 5 940 tapes reported "0 reach the platform" from
> an instrument that answers DNF to the tape that demonstrably drives across
> it.** No blame attaches — they disclosed the model swap unprompted and invited
> the re-run, which is how this was caught within minutes.

## 3. The slope route, re-tested with detectors proven able to say yes

Corrected detectors at y=54, z=656 (window y[48,54], z[642,670]) across the
platform. Yes-control first — the 21.918 tape fires every one:

| detector | 21.918 tape |
|---|---|
| (1000,54,656) | 21.660 |
| (1008,54,656) | 21.479 |
| (1016,54,656) | 21.306 |
| (1024,54,656) | 21.132 |

Then 2 × 720 programs per detector, mutating the incumbent (arm a: t ∈
[1400,1640] step 2, s ∈ {8…80}, gas forced — their launch-turn family; arm b:
t ∈ [1640,1900] step 2, s ∈ {−127…+64} — the flight and slope):

| detector | hits | **arrivals earlier than the incumbent's own** | best |
|---|---|---|---|
| x=1000 | 1 313 | **0** | 21.660 |
| x=1008 | 1 351 | **0** | 21.479 |
| x=1016 | 1 432 | **0** | 21.306 |
| x=1024 | 1 576 | **0** | 21.131 |

Every hit is the incumbent's own path to the millisecond. **The negative
survives re-measurement on a working instrument** — but it is now a negative
about *this family* (perturbations of the incumbent in two windows), not the
sweeping claim that nothing can reach the platform early. That distinction is
the whole point of §2.

## 4. The other lever I left open in v1 §7.3: upward launch

The y=136 panel row at z=686 covers only x ∈ [912,1008], so a crossing above
y=120 west of x=912 is geometrically open. Detectors at x=900 for y ∈ [120,138],
z ≤ 700/714; 1 900 programs (t ∈ [1400,1780] step 2, d ∈ {8,15,25,40,60,90},
s ∈ {−127,−64,0,+64,+127}): **0 reach any of them.** The flat ramp does not
produce upward velocity. That closes the last untested route I had named.

## 5. Where this leaves the map

Unchanged from v1 §4 and now agreed by both agents: **16.888 does not decompose
into any launch + flight + endgame either of us can build.** Best construction
≈21.3; best actual **21.918**; gap to the AT **5.030**.

The two live possibilities remain, and the second one has hardened slightly:
either a route neither of us found, or an AT that was not driven. The map is
`validated="1"` and carries **no embedded ghost of any kind**, so the container
cannot settle it — and on this map, unusually, there is exactly **one** human
record, so there is no field to cross-check the AT against either.

**What I would hand the next agent, in priority order:**

1. **Re-run the remaining route negatives with yes-controlled detectors.** §2
   shows the failure was live in this map's evidence base for the whole session.
   The spawn-dive negative I already re-ran independently with model-preserving
   gates (882 programs, 10 detector cells, 0 reach); the **hole-A doorway
   measurement and the aim ceiling** are the two that have never been re-measured
   with a detector proven able to say yes on a known-good tape.
2. **Settle the z=686 screen's real extent** by driving a slow tape into it and
   bisecting, rather than inferring it from nulls (v1 §7.1, §9).
3. Treat the AT's provenance as an open question worth one hour: `inPlugin: true`
   on unbeaten.at, `validated="1"` in the header, no embedded ghost, one record.
