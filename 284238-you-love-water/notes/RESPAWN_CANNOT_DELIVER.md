# 284238 — the respawn lever, measured: it cannot deliver a copy-0 entry

`state_ADDENDUM_v8_respawn_cannot_deliver.md`. Sidecar; supersedes nothing.
Times in seconds. Everything measured by re-simulating the record on the
**untouched map** and reading the engine's own state per tick (control in the
same run: median 0.0075 m against the record's telemetry, 91.1 % of ticks within
5 cm). Plain-oracle controls: record **440.238**, `best_97325` **97.325**.

**A `cps` number from a rung map is not a time. No milliseconds in the
`state_ADDENDUM_v6`/`v7` ladder files mean anything about the map's finish.**

---

## The question

`state_ADDENDUM_v7` measured that our **standing start flies Yhomas's launch to
within 2–7 m, point for point**, and reframed the map as: *deliver copy 0's
lane-entry state at the start of copies 1–3*. Copy 0's entry is a standing
acceleration across the deck. This map's respawn restores a checkpoint-crossing
state, and the record contains 31 of them, so the obvious mechanism is: respawn
into the next copy and drive it from there.

## What a respawn actually delivers — measured, all four copies

The record's respawn packets (`word0 == 34`) are at 11.040, 51.780, 82.680,
101.390, 105.620, … (31 in total). The state the engine holds immediately after
each of the four *first-in-copy* presses:

| respawn | restored position | speed | vz |
|---|---|---|---|
| 11.040 (CP1) | 1048.2, 1952.7, 958.1 | **52.82** | +45.2 |
| 51.780 (CP2) | 754.1, 1883.6, 513.8 | **45.79** | −39.5 |
| 82.680 (CP3) | 515.4, 1819.0, 990.9 | **41.43** | −8.7 |
| 188.340 (CP4) | 1044.7, 1766.7, 960.9 | **37.73** | +32.7 |

**A respawn restores the checkpoint-crossing state exactly — position,
velocity vector and attitude — and those are the four decaying crossing speeds
52.8 / 45.8 / 41.4 / 37.7 that define the map's problem.** It hands back the
state the run earned, at full speed, mid-air. It is a re-synchronisation, not a
reset.

The state we need is the other thing entirely:

| | speed at lane entry | vz | how it is reached |
|---|---|---|---|
| **copy 0 (works)** | **6–10** at 2.0–2.4, then accelerating | +2.4 | standing on the deck |
| copy 1 respawn (best available) | **45.79** | −39.5 | mid-air at the gate |

**So the respawn lever is dead, and the reason is structural rather than
tuning.** There is no input on this map that produces a low-speed state anywhere
except the spawn, which exists once. A respawn cannot manufacture a standing
start; it can only give you back the crossing you already had.

## And the cost, for the record

Every respawn freezes the car for **exactly 1.010 s** (measured at all four: the
position is bit-identical for 101 ticks, then motion resumes at the restored
velocity). Against the **11.67 s per cycle** the author time requires, a respawn
is 8.7 % of a cycle's budget before it has bought anything — and it buys the
same state the cycle already had.

## Where this leaves the map

The chain of measurements is now closed on every lever anyone has named:

1. the arc cannot bridge it — the **trilemma** (78 variants, `v4`);
2. two free channels cannot bridge it — 6500 evaluations, z_peak tops out at
   **922** inside the CP2-collecting basin (`v5`);
3. the copies cannot be entered differently — the tube **is** the connection
   between them, by construction of the screw (`v4` §4);
4. the target launch **is** reachable on this map — our standing start flies it
   to 2–7 m (`v7`);
5. and **no mechanism exists to hand a copy the state that launch starts from**
   (this file).

Stated plainly, and I believe this is now the honest description of 284238:

> **The author's route needs each copy entered slow and aligned, the way the
> start platform enters copy 0. Our record enters every copy fast and out of the
> tube, because that is the only connection the map's geometry provides. The
> sibling map 279008 — the same 167 blocks, byte-identical — replaces the water
> ramps with tech blocks in ALL FOUR copies, which is exactly the substitution
> that removes the tube-fed launcher, and a human beats that map's author time
> on it. On 284238 the entry we need exists once, at the spawn.**

That is a statement about the map's geometry, not about our search, and it is
falsifiable: it predicts that any tape which enters a copy 1–3 launch at
90+ m/s out of the tube will contact the wall high and lose the cycle, and that
nothing in the arc or the lane can prevent it. Every measurement tonight is
consistent with it, including the two that looked most like counterexamples (the
arc *can* reach his crossing height, and the yaw *is* available on our lane —
both true, and neither sufficient).

## What I would still try, in order, if the map stays open

1. **The previous cycle's EXIT.** Not tried. The trilemma is about the arc; the
   state handed to the arc is set by how the previous cycle leaves the gap, and
   that is a different window with different physics. `v7`'s ladder gives it a
   pass/fail objective (fire C0R2/R3/R4's copy-1 images) that does not require
   CP2.
2. **A slower, lower arrival into a copy.** Copy 0 works because it is slow at
   the deck. If a cycle can be made to arrive at a copy's entry at 30–40 m/s
   instead of 76–92 — deliberately, by braking in the tube — the launch may
   behave like copy 0's even though it cost time to get there. The trilemma says
   nothing about this because every variant in it was fast.
3. **Accept the geometric answer and say so**, with the sibling map as the
   evidence that the author's own remix removed the feature that blocks us.

## Enumeration

* 31 respawn packets located in the record; the four first-in-copy presses
  measured for restored state and freeze duration, from a per-tick re-simulation
  of the whole 440.8 s run.
* Freeze duration measured as the first tick whose position differs from the
  restored one: 1.010 s at all four, no variance.
