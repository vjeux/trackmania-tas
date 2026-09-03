# Matched live-map test plan — January 2022 Stadium profile

For execution on WhiteStick by the parent session. Nothing here licenses
marking January selectable or certified; see "What no outcome licenses".

## Question

Does the January island reproduce pre-2022-03-29 Stadium physics, or is it a
**behavior alias** of stock current — the exact failure that closed Fall, whose
control table shows stock, graph-only and V5 agreeing to six significant figures
on every longitudinal channel?

## Rules carried in from the Fall result and from earlier method failures

1. **Matched pairs only.** Same map, same input tape, same start state; the only
   variable is which profile is installed.
2. **Score the divergence ladder, not point speeds.** Use the HPLTRC3 form
   already in use (`>5mm@t; >0.1m@t; >1m@t; >10m@t`). Two speed traces compared
   at the nearest instant lie on a ramp and manufacture differences.
3. **Measure the floor before believing any difference.** Arm A exists for this.
4. **A negative needs a positive control.** Arm B exists for this; the Fall
   table has no equivalent, which is why "matches stock" there could not
   distinguish "island is inert" from "instrument is blind".
5. **Every run records: profile id, island SHA-256, exe SHA-256, map SHA-256,
   trace SHA-256, record count.** A run without its five hashes is not evidence.

## Payloads under test

| Id | Island SHA-256 | Note |
|---|---|---|
| `stock_current` | — | build 128130, `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda` |
| `january_shipped` | `3af4b5c266abf3f05301f61aec1d3c81aa4fb1d5707ec467f2edbc9fa60ddf68` | comparator only — carries 26 wrong-field accesses |
| `january_corrected` | `f8a14a062fb2ff0ac2b1cf13b67dd3e3b71f2f3acf29de4cb49ebfc729b2677a` | 16 repaired, 10 known-wrong remain |
| `positive_control` | built in Arm B | corrected island, one constant perturbed |

## Arm A — determinism floor (run first, blocking)

Three identical `stock_current` runs on the booster map, same keyhold.
Record the maximum pairwise divergence across the three traces.

* Expected: bit-identical traces (the Fall table shows 4,543 records reproducing
  exactly). If so the floor is zero and any nonzero divergence later is real.
* If nonzero: that value is the floor. Every criterion below reads "beyond the
  floor", never "nonzero".

## Arm B — positive control (blocking; abort the plan if it fails)

Build `positive_control` from `january_corrected` by scaling exactly one
longitudinal force constant in the copied constant pool by 1.02 — a single
4-byte float in the island's pool, no other change — and record which offset was
touched.

Install it and run the Arm C protocol once.

* **Pass**: the divergence ladder moves beyond the floor.
* **Fail**: the pipeline cannot see an island-side physics change at all. Then
  every "matches stock" result in Arms C–E is uninformative and the plan stops
  until the instrument is fixed. This is the check the Fall campaign never ran.

## Arm C — press-forward booster, matched quad

Map: `HPL_Fall_Ice_Booster_Jump.Map.Gbx`, SHA-256
`3746b9dc3801eba6e3bc27e326c6e254f6d8c64921237521f0b36a0b87fde215`
(ice + booster + jump; already built by `tmmaps_synth_fall_booster.rs`).
Input: the existing 22.000 s full-throttle, no-steer keyhold, 431 keydown
batches — identical for every run. Trace: HPLTRC3.

Run all four payloads. Record the Fall control columns unchanged, so the two
campaigns are directly comparable:

`pre_booster_kmh, post_booster_kmh, takeoff_kmh, takeoff_s, airtime_s,
landing_s, landing_z_m, finish_plane_s, finish_plane_kmh`

plus the divergence ladder of each run against `stock_current`.

* **Alias verdict**: if `january_corrected` stays within the floor on every
  channel while `positive_control` moved, the island is inert — it runs but does
  not carry the January force law. January stays disabled; the static repairs
  did not make it a physics profile.
* `january_shipped` vs `january_corrected` also quantifies what the 16 repairs
  changed. A large difference here is itself a finding: it means the shipped
  payload's wrong accesses were reaching the force path.

## Arm D — January-specific ice and water control

This is the arm Fall could not run, and the reason January needs its own plan.
January 2022 precedes **both** later Stadium changes: the Fall-2022-staged
wiggle and water-bounce behavior, and the Summer-2023 "modern ice: slowdown and
wiggle changes, slightly faster ice". Ice and water are therefore exactly where
a faithful January island must separate from current, and a booster-only test
cannot see it.

**D1 — ice (no new map needed).** Use the ice straight of the same map. Zero
steering throughout. Measure over the ice segment:
* speed-decay slope (km/h per metre travelled), fitted over the whole segment,
  not sampled at an instant;
* lateral drift with zero steering input — the wiggle amplitude — as peak
  absolute lateral offset from the entry heading line;
* exit speed at a fixed distance gate.

**D2 — water.** Extend the synthesiser with a shallow-water strip after the
booster (`tmmaps_synth_fall_booster.rs`; keep the ice+booster+jump body and add
the strip, then record the new map's SHA-256). Zero steering. Measure:
* vertical bounce peak and penetration depth;
* Δspeed across the strip;
* time to re-establish ground contact.

* **Discrimination criterion**: at least one of D1, D2 must separate
  `january_corrected` from `stock_current` beyond the floor. If ice **and**
  water are both within the floor while `positive_control` moved, the island is
  not carrying the January force law and no amount of static repair will change
  that conclusion.

## Arm E — authoritative historical anchor

The only true historical target this project owns for January, and it is on ice:
**KEKL- SAUSAGE ICE**, Roevhaal's 63.546.

Already established server-side, and not to be re-litigated:
* 2022-03-25, build 112349 → `WRONG_SIMU`, validated time −1;
* 2022-03-29, build 112449 → valid, exact 63.546.

New work: replay that same input tape in the current client under
`stock_current` and under `january_corrected`.

* A faithful January island must land on the **pre-March-29** side of the
  boundary — a divergence, not a reproduction of 63.546.
* If `january_corrected` reproduces 63.546 exactly, it is behaving like a
  post-March-29 build and the profile is mislabelled.

Two guards, both learned the hard way:
* the 2022 servers report a DNF as a **present** `ValidatedResult` with
  `Time: -1`; any parser must treat −1 as failure, never as a best time;
* they never print `reached some checkpoints (N of M)`, so there is no DNF
  depth — use a segment ladder if depth is needed.

State plainly in the result: this arm compares a client-side island against a
server-side boundary. It is corroboration, not proof of client semantic
equivalence.

## Arm F — state-corruption watch

Motivated by the three stray writes and three wrong pointers found statically.
Run against `january_shipped` **and** `january_corrected`; the shipped result is
the direct evidence for disabling immediately.

* Dump the vehicle object's first `0x2000` bytes at a fixed tick under stock and
  under each January payload; diff. Any field differing outside the profile's
  intended footprint is corruption, and the wrong-offset writes name where to
  look first: current `0x1758`, `0x1504`, and the sub-objects addressed by
  `lea` at `0x1378` and `0x18d0`.
* Soak: the full 22.000 s run, then install → switch → unload → reinstall, three
  cycles, verifying the 41-byte entry preimage restores exactly each time.
* Any crash, any failed restore, or any drift outside the footprint is an
  immediate stop.

## Order and aborts

A → B → C → D → E → F. Abort the whole plan on: Arm B failing, any crash, or any
failed unload restore.

## What no outcome licenses

Even a clean sweep of A–F does **not** make January selectable or certified. Ten
known-wrong structure accesses remain in the corrected payload, unresolvable
without `Trackmania-2022-01-21.exe`
(`e2255c415f0f7fc2d0a66512fa7609256c42cf639a5380b7a5bcdbb4486ab75b`), its
disassembly, and the two field-map TSVs. A passing behavior test with ten wrong
accesses still in the island would mean the test is insensitive to them, not
that they are harmless.
