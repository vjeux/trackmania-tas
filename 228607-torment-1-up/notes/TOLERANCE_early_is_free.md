# 228607 — tolerance, and the one sentence a player can use: on this map EARLY IS FREE AND LATE IS FATAL

Agent `tor`, 2026-08-19, node 31830. Write-once. **Extends §B of
`228607/tor_RESULT_228607_AT_BEATEN_v1.md`** (md5 `5dfcf15f20e0eeb406523d3bea0fd51d`)
with a finer probe; it does not retract anything in it, but it **supersedes that
section's tolerance table as the number to quote**, because the whole-block
probe used there conflates three input channels and this one does not. Times in
seconds.

**New best while measuring this: 19.936** (author time 20.258, **−0.322**),
banked as `tor_bank/tapes/tor_BEST_19936.Ghost.Gbx`, validated from the store
with both controls exact in the same batch. It is the 19.947 tape with its
steering slipped **10 ms earlier** — the tolerance probe produced the incumbent.

---

## The probe

v1 slid the whole input block (steer, accelerate, brake together) from a fixed
tick. That is the fleet's comparable probe and it is right for comparing maps,
but it cannot say *which* input is load-bearing or *which direction* hurts. This
one slips **only the steering channel**, from a tick `T` onward, by ±1 tick
(10 ms), sweeping `T` across the flight:

```
torscript --template FAM.Ghost.Gbx --shift <±N> --shift-from <T> --shift-steer --out ...
```

34 variants of the 19.947 member, validated on the untouched map with a
known-answer control in the batch.

## The result

| steering slipped from tick T | **−1 tick (10 ms EARLY)** | **+1 tick (10 ms LATE)** |
|---|---|---|
| 1870 (race 18.70, just after the launch) | **19.936** ✔ *faster than the original* | **loses the Goal** |
| 1880 · 1890 · 1900 · 1910 · 1920 | **19.936** ✔ | **loses the Goal** |
| 1930 | loses the Goal | loses the Goal |
| 1940 · 1950 · 1960 · 1970 | (not sampled ✔) | 20.065 · 20.065 · 20.065 · 20.066 ✔ |
| 1980 · 1990 | 19.946 · 19.941 ✔ | 20.172 ✔ |
| 2010 · 2020 · 2030 | 19.947 ✔ (no change) | 19.947 ✔ (no change) |

> **Being early is free. Being late is fatal.**
>
> Through the decisive window — race **18.70 to 19.30**, which is the release
> and the counter-steer — a 10 ms *early* slip of everything that follows keeps
> the run and is in fact **11 ms faster**, while a 10 ms *late* slip loses the
> Goal outright. After ~19.40 both directions survive and lateness merely costs
> time (0.12–0.23). After ~20.10 the flight is committed and neither matters.

That is the usable coaching sentence for this map: **release the lock a touch
early rather than a touch late.** It is also consistent with the mechanism — the
flight is ballistic after ignition, so anything that stops the roll sooner leaves
more of the launch's vertical velocity intact, and anything that stops it later
has already spent the climb.

## The bounds on it, stated

* **One tick each way is the whole budget.** −2, −3, −4, −5, −7 and −10 ticks all
  lose the Goal from every T tested. "Early is free" means *one* tick early, not
  "the earlier the better".
* **The asymmetry belongs to the analog member.** The same probe on the
  low-input member (`FAM_lowinput_a8`, 16 steer values, 47 events) survives
  **neither** direction at any T tested.
* So **"fewer inputs is easier to drive" is measured FALSE on this map, twice** —
  by the whole-block probe in v1 (analog 1 survivor of 12, low-input 0) and by
  this finer one (analog tolerates a 10 ms early slip through a 600 ms window,
  low-input tolerates nothing). Each member needed its own number, exactly as the
  standard says.
* This probe slips **everything after T**, so it measures the tolerance of *the
  rest of the flight as a unit*, not of a single keypress. A true per-action
  number — how late the release alone may be, with the counter-steer left where
  it is — is still not measured, and it is the last thing I would have built.

## Controls

Every row above is a plain-oracle validation on the untouched
`m228607.Map.Gbx` with `CTRL_ident_24854` returning **24.854** in the same batch,
and the 19.936 claim re-validated afterwards reading the map, the tape and both
controls from the shared store only.
