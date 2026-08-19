# 228607 — the author's 14 m is TWO knobs the field already has, held at once. No second mechanism; the map is a search problem.

Agent `tor`, 2026-08-19, node 31830. Write-once. Companion to
`228607/tor_RESULT_228607_AT_BEATEN_v1.md`. Times in seconds; `vy` in m/s,
accelerations in m/s². Read-only against trackmania.io; nothing submitted.

---

## The question

The author crosses the Goal x-band at **y = 160.5, still climbing**. The best of
41 profiled official humans is 14 m short and the field's top is 17–33 m short.
Is the author's line **on the same curve** as the field — a corner nobody
occupied — or **off it**, i.e. a second mechanism nobody has?

## The decomposition

Altitude at the band is produced by exactly two things: how much vertical speed
the launcher hands you, and how much of it you keep. So per run:

* **`vy` at ignition** — the launch output, fixed at the contact;
* **`ay_mean`** — mean vertical acceleration from ignition to x = 352. Gravity
  alone is **−24.7**; anything more negative is climb spent on drag or on thrust
  pointed the wrong way.

| run | `vy` at ignition | `ay_mean` | y @ x=352 |
|---|---|---|---|
| **author** | **92.0** | **−27.1** | **160.5** |
| p08 · 20.337 | **101.6** ← field's best launch | **−45.5** ← field's worst coast | 134.3 |
| p13 · 20.703 | 93.3 | −44.7 | 127.8 |
| p09 · 20.387 | 88.6 | −40.1 | 127.6 |
| p07 · 20.307 | 84.7 | −36.4 | 136.9 |
| p15 · 20.938 | 82.3 | −36.3 | 133.1 |
| p06 · 20.272 | 80.8 | −35.5 | 130.5 |
| p10 · 20.426 | 84.2 | −34.8 | **140.4** ← best human |
| p01 · 20.034 (WR) | 85.1 | −34.3 | 135.0 |
| p12 · 20.480 | 77.4 | −33.5 | 128.1 |
| p11 · 20.430 | 76.8 | −33.0 | 128.0 |
| p14 · 20.738 | 76.3 | −32.7 | 131.7 |
| p03 · 20.217 | 74.6 ← near the worst launch | **−26.3** ← field's best coast | 136.8 |

## The answer: ON the curve

**Both of the author's numbers sit inside the field's own demonstrated range** —
92.0 against a field maximum of 101.6, and −27.1 against a field best of −26.3.
He does nothing on either axis that a human has not already done.

**He is the only one who does both at once**, and in this field the two are
plainly **anti-correlated**: the best launch has the worst coast (p08), the best
coast has nearly the worst launch (p03), and everyone else trades along the
middle. The 14 m is an unoccupied **corner of the achievable box**, not a point
outside it.

> **So there is no second mechanism, and 228607 is a search problem.** Which is
> what the search independently demonstrated: our tapes beat the author time
> before any of this was measured.

**The ceiling the field itself implies**: p08's launch (`vy` 101.6) with p03's
coast (−26.3) puts the car at **y ≈ 173** at the band — 12 m *above* the author.

## What the two knobs physically are

**The launch knob is the nose angle at the contact.** The launcher fires along
the car's nose and splits its energy by that angle: p06 takes 997 km/h with `vy`
only 80.8 (flat), p08 takes 807 km/h with `vy` 101.6 (steep). Speed at the
contact barely varies across the field (692–857 except p06's 997); the *split*
is what varies.

**The coast knob is whether the roll stops before the car goes inverted.**

| run | peak roll after the launch | `ay_mean` | speed, launch → +1.0 s |
|---|---|---|---|
| author | −1.61, unwinds to −0.18 | −27.1 | 769 → 720 |
| p03 | −1.72, holds, unwinds | −26.3 | 788 → 748 |
| p01 (WR) | −1.26, unwinds | −34.3 | 830 → 670 |
| p10 | −2.31, **wraps to +2.13**, tumbles | −34.8 | 820 → 662 |
| p08 | −2.49, **wraps to +2.25** | −45.5 | 807 → 677 |

Past about ±1.8 rad the car goes through inverted, presents its flank to 800 km/h
of airflow, and pays for it: p08 loses 92 km/h in the first 100 ms alone. The
runs that keep their climb are the runs whose roll never gets there.

**And the official world record already performs the author's release.**
Emelius. returns the stick to centre at **18.750** and counter-steers to full
left by 19.05 — the same script as the author's 18.740, 10 ms later. He does it
with a flatter launch (85.1 against 92.0), which is the whole of why he is 25 m
lower on a map where being lower is *rewarded*.

## The coaching sentence, final form

> **Hit the launcher nose-up for the steepest launch you can get, then stop the
> roll before the car goes inverted — release at about 200 ms and counter-steer.
> Early is free, late is fatal, and one tick is the whole budget.**

Every clause is measured: the nose angle from the 13-run split above; the roll
threshold from the tumblers; the release from three independent laps (both
authors and the official WR); the tolerance from
`tor_TOLERANCE_early_is_free_late_is_fatal_v1.md`.

## Bounds on this

* 13 runs with usable ignition-to-band telemetry, out of 41 profiled and 55
  downloaded. Ranks 1–18 plus a sample to ~80.
* `ay_mean` is a mean over ~1.5 s, not an instantaneous fit; it is a summary of
  the coast, not a force measurement. The claim it supports is a **ranking**, and
  the ranking is what the question needed.
* The author's lap is record data with no input archive, so his row is trajectory
  and attitude only, never replay.
