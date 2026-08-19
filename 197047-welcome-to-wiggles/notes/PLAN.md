# PLAN — map 197047 "Welcome☺to wiggles" (AT 100784, human WR 101794)

Written after acquisition + the §4/§8 controls, arguing from what the map
actually turned out to be. Everything below is measured, not assumed.

## What the map is (this changes the whole attack)

- 3 blocks, 77 items, Stadium `48x48Day`. `isValidated: true` — the AT is a
  **driven validation lap**, not a plugin number.
- Only **2 checkpoints**: an item gate 6 m from the spawn (crossed at ~770 ms by
  everyone) and a block gate 620 m away. Splits for the WR: `[766, 100215,
  101794]`. So there is one 99.4 s sector and a 1.58 s tail. The "many
  checkpoints" the brief hoped for do not exist — I built my own with relocated
  finish gates instead (`tmmaps gate`, new subcommand).
- The car spawns at **y = 242 m** on a flat straight and stays at exactly that
  altitude for the whole run. x runs 1018 → 391; z stays inside ±3 m.
- From race ~1.95 s to ~100.4 s the WR holds **gas AND brake together** and
  flips the steering full-left / full-right at about 2.4 Hz. Speed sits at
  **~22 km/h (6.2 m/s)** the entire time.

**The map is 100 seconds of the "wiggle" — the TM2020 technique of moving a car
that cannot drive by rocking it left/right with gas+brake held.** Educational
tag, "Welcome to wiggles", 620 m of it. That is the whole map.

## Consequences for the attack

1. **There is no trick at a feature; there is a limit cycle.** The objective is
   the mean forward speed of a periodic steering waveform. 1010 ms out of
   100784 is a 1.0 % speed improvement. The search space is not 10 000
   dimensions, it is essentially *the rhythm*.
2. **Per-checkpoint shaping has to be manufactured.** `tmmaps gate --at x,y,z`
   writes a map whose only finish is a gate at a chosen x, with the far
   checkpoint neutralised. A ladder of them along the corridor gives exact
   sector times for any tape, human or synthetic, and lets a candidate be
   judged on 16 s of simulation instead of 100 s. One worker root per gate map
   — every gate map keeps the original mapUid (known defect).
3. **The tail is not driveable time.** After the far checkpoint the run is
   finished by a **respawn**, which teleports the car to the start line, and the
   finish is crossed ~1.5 s later. The respawn is not a steer/accel/brake field:
   it is bit 31 of the packet's 34-bit state literal (word0 bit 5), sitting at a
   fixed tick. Measured on the WR: moving that pair of packets k ticks earlier
   moves the finish 10k ms earlier, exactly. **finish ≈ (first respawn tick −
   154)·10 + 1504.** So the deliverable is: reach CP2 as early as possible, then
   respawn on the next tick.
4. **The run is chaotically unstable open-loop.** Replacing 50 ms of the WR's
   steering at t = 30 s loses the run entirely. Of 481 synthetic waveforms only
   12 survived the full 620 m. A human is closed-loop and corrects; our tape has
   to bake the corrections in. That forces a **sector-by-sector march** rather
   than a global perturbation search.

## The plan

1. Controls: §4 identity (22/22 exact) and §8 field reproduction (22/22 across
   the whole leaderboard, WR to last). Codec identity through the Factory:
   101794. **All passed.**
2. Build the gate ladder; time the whole field through it; get the per-sector
   table and the sum of per-sector minima. Decides whether the AT is reachable
   by assembling what the field already does.
3. Sweep the periodic waveform (hold length, ramp length, amplitude) against a
   mid-corridor gate to find the fastest rhythm, with survival factored out.
4. Take the best surviving waveform to the full map and bisect the respawn tick.
5. March: fix the tape up to gate g, search the half-cycle lengths in the next
   sector against gate g+1, bank, advance. This is the drift correction.
6. Human deliverables: a pure-keyboard family (the field already contains two
   pure `{-127,0,+127}` runs, ranks 9 and 14 — that is the ground truth for
   "keyboard"), per-input tolerance, and a driving guide off visual cues.

## What "human reproducibility" means on this map

Not a trick to discover — a **rhythm to hold**. The deliverable is a metronome
number plus how much slack each flip has. The AT was driven, so a human-sized
version exists by construction.
