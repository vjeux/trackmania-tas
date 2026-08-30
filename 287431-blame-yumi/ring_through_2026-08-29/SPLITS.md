# Inherited checkpoint splits — what I fixed, and what is still blocked

**2026-08-29, box 129490.** vjeux caught it: our tapes declared ITZYNO1FAN's six
intermediate splits beside our own finish time. A published clip therefore said
"CP6 at 16.945" when that is HIS crossing.

## What is fixed

**`ghost declare` now blanks stale intermediate splits, unconditionally.**

When the declared time CHANGES, the intermediate splits timed the old run by
construction, so they are zeroed and the change is announced:

```
the plain oracle re-simulated ring/seeds/ring20756.Ghost.Gbx to 20.756
  the declared time changed (24.092 -> 20.756), so the 6 intermediate split(s)
  are STALE -- they timed the old run. They were ["5.154", "7.375", "9.920",
  "11.680", "14.477", "16.945"] and are now 0.000.
  0.000 means "this container does not know its intermediate splits".
  To fill them in, measure each checkpoint crossing and pass --splits.
```

`regen` calls `declare`, so every regenerated ghost from now on is honest by
default. `--splits` still writes measured times and suppresses the blanking.

**The test is "the time changed", not "this is a synthesised tape"**, because
that is the part the code can actually know. One blind spot, stated so nobody
has to rediscover it: a searched tape whose time lands EXACTLY on the donor's
keeps the donor's splits. The alternative — blanking on every declare — would
destroy the good splits of a genuine recording being re-declared to its own
time, which is the commoner case.

**`ghost verify` gained V12**, the split-table check: the last entry must be the
race time, the list must not go backwards, no intermediate may sit at or past
the finish, and the table is PRINTED. A blank table passes and says so; a
populated one WARNS that self-consistency is not provenance.

## ⚠ What V12 CANNOT do, and why I did not pretend otherwise

**V12 would not have caught this bug.** An inherited table is monotonic, ends at
the declared time, and lies inside the run — it passes all three checks. I have
written that in the code comment rather than leaving a green tick to be
misread.

**And the obvious stronger gate is decoration on this map. I built it, measured
it, and threw it away.** "Was the car near checkpoint k at split k?" — our line
tracks his to about 2 m through CP4, so at HIS split times OUR car is 1.2, 2.9,
2.4 and 2.6 m from where his was, against the 20–46 m that a checkpoint block's
ORIGIN sits from its own gate. Any threshold loose enough not to fire on honest
files passes all six of ours. A test any outcome satisfies is decoration.

The defect is caught in `declare`, which is the only place that sees the run
before AND after.

## ⚠ What is still blocked: we CANNOT measure our own splits on this map

The exact route is a segment map per checkpoint — the finish moved to
checkpoint k, so the oracle returns that crossing as a race time. **`tmmaps
segments` cannot build one for 287431, for three independent reasons, all
verified:**

1. **CP5 is a LINKED group** — five `PlatformTechCheckpoint` blocks at 32 m
   spacing (b105–b109, x 569…697, y 43, z 688) that the game counts as ONE
   checkpoint number. `segments` filters on `tag == "Checkpoint"` and refuses
   before building anything. *(This one I fixed — see `segat` below.)*
2. **There is no relocatable gate item.** The exact promotion method moves a
   spare waypoint gate ITEM onto the cut. This map's only item waypoints are
   **six decorative `LargeLetters_Finish_*` letters and the spawn**. Relocated,
   the letter never fires: the CP6 segment returns **24.092**, the full race.
3. **The rename fallback cannot apply.** It renames `<X>Checkpoint` →
   `<X>Finish`; five of the six checkpoints are
   `Blog.Gbx.Block.Gbx_CustomBlock`, whose name contains no "Checkpoint", so
   the rename is a **no-op**. On the linked group it is worse than a no-op:
   `PlatformTechCheckpoint` is a **platform the car drives on**, and renaming it
   changes the structure under the wheels — every such segment returned DNF.

Ruler calibration, run against the WR whose splits ARE his own
(expect 5.154 / 7.375 / 9.920 / 11.680 / 14.477 / 16.945):

```
cp1 DNF   cp2 DNF   cp3 DNF   cp4 DNF   cp5_105..109 DNF   cp6 24.092
```

**Not one ruler reproduced its own reference split, so not one is a ruler.** No
number was taken from them.

## New tool: `tmmaps segat`

```
tmmaps segat MAP --out F --promote W [--neutralise W,W,...] [--force-rename]
```

ONE segment map with the policy taken out: the caller says which waypoint
becomes the finish and which stop being checkpoints, nothing is inferred,
nothing is verified (the caller owns the comparison). It is the only way to cut
at a **linked** checkpoint, and it addresses `LinkedCheckpoint` waypoints, which
`segments` cannot. `resolve_waypoint` was split out of `resolve_order` so a
single name gets the same parsing and error prose without `--order`'s arity
check.

Rules the caller must keep (in the command's help): neutralise every checkpoint
at or after the cut AND every other member of the cut's own linked group; leave
an EARLIER linked group completely alone.

On 287431 `segat` builds all ten maps and they are ten distinct bodies — the
blocker is (2) and (3), not the enumeration.

## What would unblock it

* **Import one real gate item into the map** (a `GateFinish` item this project
  controls) so the exact relocation method has something that fires. That makes
  every plain checkpoint measurable and is the general fix for custom-block maps.
* Or **read the crossing out of the engine** the way `fk` reads the car — the
  crossing times exist in memory during the simulation. Note `fk trace` cannot
  locate the car on this map at all (542 s to fail its own self-check, mean
  speed 3.1 m/s), so that needs work first.

Tape truncation + `cps` counting was considered and rejected: it resolves to a
10 ms tick, and the game's own splits are sub-tick (5154, 7375, 14477, 16945 are
not multiples of 10).

## Files

```
tapes/ring20756_honest.Ghost.Gbx    the 20.756, splits blanked, V12 PASS
tools/splits_honesty.patch          declare + verify V12 + tmmaps segat
```

The published 20.756 should be **re-emitted from this file** or annotated: its
final time was always right; its six intermediates were his.
