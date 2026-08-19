# 228607 — v2: the author's OWN lap, read out of the map file, and it corrects my v1 §3. The 1-UP time is won in the reactor flight, not in the carry.

Write-once, `key_` prefix (answer-key agent, session
`9f00f635-a11d-4d68-b07a-6ac2f1386397`, node 105213). **Supersedes §3 of
`key_RESULT_v1_sibling_228811_is_the_same_map.md`**; everything else in v1 (the
99.94 % identity, the exhaustive 7-record difference, the `cps=6` transfer
result) stands. Times in seconds.

---

## 0. What is new

228607 embeds the author's own recorded lap — 406 samples, 50 ms, 0 → 20.290,
declared **20.258**, the author time. It has been *counted* repeatedly (it is
the fleet's positive control for embedded-ghost scanners) and, as far as the
store shows, **never read**. It is readable:

```
ct recghost --map 228607.Map.Gbx --carrier <any 228607 ghost> \
            --out AUTHORREC_228607_20258.Ghost.Gbx --time 20258
tmtraj decode AUTHORREC_228607_20258.Ghost.Gbx --csv …
```

It carries no input archive, so it can never be re-simulated — but it is a
per-tick record of the run that set the time we are chasing. I did the same for
**228811**, whose embedded lap declares **20.550**, its own author time. So we
now have **both author laps of the same map, one ending at the high Goal and one
at the low Goal**, and the difference between them is the whole 1-UP problem.

## 1. What I got wrong in v1 §3

v1 said: *"1-UP is not a different route. It is the same flight carried ~60 m
higher through its last 150 m of x."* The carry number was right; the mechanism
was wrong, and it mattered. It is not carry. **It is the reactor flight, and the
difference is set in the first 0.5 s of it.**

## 2. The two author laps, side by side

Both fire the reactor at almost the same place, within 0.11 s of each other:

| | **228607 (1-UP), AT 20.258** | 228811 (1-DOWN), AT 20.550 |
|---|---|---|
| reactor fires | t = **18.540** at (77.7, 51.3, 708.9) | t = **18.650** at (75.6, 53.2, 708.3) |
| speed at ignition | 340 → **769 km/h**, vy **+92.0** | 323 → **751 km/h**, vy **+94.1** |
| 0.45 s later | **742 km/h, vy +79.5**, y = 91.0 | 548 km/h, **vy +22.5**, y = 81.9 |
| at x ≈ 200 | y ≈ 96, climbing | y ≈ 84, vy +22 |
| at x ≈ 360–405 | y = **160 → 173**, still climbing, **671–688 km/h** | y = **95**, flat, 521 km/h |
| ends | 20.290, (405.5, 172.9, 715.0) | 20.550, (366.1, 95.1, 694.0) |

**Ignition is nearly identical. What differs is what happens in the next second:
the 1-UP lap HOLDS the climb — vy ~80 m/s sustained, speed still 742 at +0.45 s —
while the 1-DOWN lap's vy collapses 94 → 22 and its speed 751 → 548 in the same
window.** By the time both reach the gate x-band the 1-UP car is **78 m higher
and 150 km/h faster**.

And our best 228811 tape's line is *less* climbing still: it apexes at y ≈ 155
around x ≈ 155 with vy 0 at 408 km/h, then descends through the gate band at
y ≈ 96–111. That is why it returns `cps=6` on 228607 and misses only the Goal:
it clears every checkpoint and then arrives at the gate ~60–70 m low.

## 3. The objective this gives the arm

Not a time and not a tape — a **state at a place**, which is the form 228811's
own launcher work already used:

> **Cross x ∈ [352, 416] (Goal cells 11–12), z ≈ 672–800, at y ≈ 155–175,
> ascending at vy ≈ +45…+80 m/s, at ≥ 670 km/h, by t ≈ 20.2.**

and the upstream condition that produces it:

> **Fire the reactor at x ≈ 78, y ≈ 51, z ≈ 709 at ≥ 340 km/h — then hold
> vy ≈ 80 m/s for the next ~1.5 s instead of levelling off.** The 1-DOWN author
> lets vy decay to 22 within 0.7 s; the 1-UP author does not.

This is an air-control problem in a reactor flight, which is the class 199100
found to be worth 1.824 on its map ("all of it is air control in the reactor
flight"). The 228811 arm's tapes already put the car at the ignition point with
the right speed — the whole of 228607's remaining 0.3 s sits in what the car
does after the reactor lights.

## 4. Why you can believe the read

* **Positive control:** the same reader returns 228607's node as v10, 406
  samples at 50 ms, 0 → 20.290 — exactly the figures
  `ACQUISITION_addendum_embedded_author_ghost.md` published for it.
* **The two laps were extracted with the same command from two different map
  files** and land on their own maps' Goal blocks (1-UP at (405.5, 172.9),
  1-DOWN at (366.1, 95.1) — the Goal cells are 11–12 in x, y 27 vs 19).
* Both declare their own map's author time (20.258 / 20.550) after `--time`, and
  their recorded ends (20.290 / 20.550) sit within the known +0.03 post-finish
  rolling window.
* **Caveat, stated:** a record-data node has no input archive, so neither lap can
  be re-simulated and neither is a seed. What is measured here is trajectory and
  state, nothing else. The carrier's splits leak into the rebuilt file's
  checkpoint list — ignore that field; it is the carrier's, not the author's.

## 5. Artefacts

* `228607/key_siblings/key_AUTHORREC_228607_20258.Ghost.Gbx` — the author's own
  1-UP lap, watchable
* `228607/key_siblings/key_AUTHORREC_228811_20550.Ghost.Gbx` — the 1-DOWN lap,
  for the comparison in §2
* `228607/key_siblings/key_authorlap_228607.csv`, `key_authorlap_228811.csv` —
  the decoded per-tick telemetry of both
* v1's artefacts unchanged (sibling map, identity tables, transfer carriers)
