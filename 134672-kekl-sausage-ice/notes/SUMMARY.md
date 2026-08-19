# 134672 `KEKL- SAUSAGE ICE` — AT 58687 NOT BEATEN (best validated 67404)

(Block for `RESULTS.md`. Written to a versioned file instead of appended:
a concurrent writer clobbered the append, and `private-30d` is not
read-your-writes. Merge this in when RESULTS.md is quiet.)

AT **58687** · human WR **63546** (2022, does not re-simulate) · best human on a
current build **68442** · 15 records. Full write-up: `134672/RESULT.md`.

| tape | validated | vs AT | vs best today-legal human | alphabet | steer events |
|---|---|---|---|---|---|
| analog floor | **67404** | +8717 | −1038 | 74 values | 193 |
| **keyboard** | **67625** | +8938 | −817 | **3** | 114 |

**Why it did not fall — three independent estimates of what this route is worth
land within 283 ms of each other, and the AT is 4.8 s below all three:** the
field's best-sector recombination 63263, the 2022 human WR 63546, and our own
per-sector TAS optima summed 63492. The author's own online record is 69522
(rank 3), 10.8 s slower than their own validation lap, on a map they say they
"built in 15 mins". The map file is byte-identical to Nadeo's copy and no route
cut was found, so the remaining untested hypothesis is that the validation lap
predates the embedded custom ice blocks.

## Findings that generalise

* **Measure the map's Lyapunov time before choosing a method.** Here a **1/127
  steer change on ONE tick e-folds every 0.6–0.8 s**: invisible for a second,
  +173 ms at CP1 four seconds later, DNF by six. Everything else follows — a
  40-second spread over 15 records, 0 of 319 input events deletable by `tmsimp`
  at a 40 ms budget (83 319 evaluations), and the §8 result below. Cost: five
  perturbed candidates and one gate ladder.
* **A build-correlated §8 failure is not the same as 203072.** 10 of 15 ghosts
  fail here and **all ten are from one 2022 build**, while **5 of 5** from three
  different 2025–2026 builds reproduce exactly, including a 101 259 ms run, and
  the state locator tracks a ghost's own telemetry to rms 0.008 m over 68 s. On a
  chaotic map any build difference is fatal to a replay; that is a property of
  the map, not evidence against the oracle. Check the ghosts' `git=` build before
  condemning a map.
* **NEW TOOL — `tmmaps gateladder`**: park every checkpoint off the track
  (rename to a finish so it is not required, move to a corner cell) and relocate
  the real Goal block to ANY 32 m cell, optionally keeping the first N
  checkpoints real. Needs `MapFile::set_block_cell`/`set_block_dir` (blocks now
  carry `coord_off`). Turns a DNF into "reached cell (x,z) at t" for one
  validation. **Verified with yes-controls** (a gate at CP2's cell returns each
  ghost's own declared CP2 split to the ms). `dir` 1/3 fires for crossings along
  x, 0/2 along z — build both. This is how a DNF was localised to the tick.
* **A gate with no yes-control proves nothing.** My mid-field "is there a cut"
  probes are weak negatives for exactly that reason; recorded as such.
* **`tmmaps build` can derive the checkpoint order wrong** (it did here:
  243,165,170,261 instead of 165,170,243,261), which silently makes `seg2/3/4`
  all a CP4 gate. Check a segment map against a ghost's declared split before
  using it.
* **Per-sector optima do not compose on a chaotic map.** Measured independently,
  the five sectors offer −4950 ms; a 42-minute, 528 000-evaluation search with
  the true full-run objective and the window opened to ticks 2500–7000 found
  **nothing** — the 1778 ms available in sector 4 is incompatible with finishing.
  All of our gain is in the last 7.5 s, where the tail is short enough to
  re-derive. **Search backwards from the finish, and do not believe a sector
  ceiling until it survives the full objective.**
* **Steering saturation on ice, a NEGATIVE for the 285268/279209 rule.** Here
  more full lock correlates with a FASTER time — corr −0.40 overall, −0.77 among
  the 8 pure-keyboard runs, −0.47 among the 7 pad runs — and the top three
  records are keyboard. Mean |lateral velocity| is 13.8–23.2 m/s over the whole
  lap and monotone in pace. Proposed reconciliation: back off lock where you are
  keeping the car pointed and accelerating, keep it pinned where you are
  rotating.
* **`validated="1"` does not imply the author ghost is embedded.** Not present
  here in any form (no `0x0911F000`, no `0x0309201D`, no `CGameCtnGhost`).
* Repairing a DNF human tape whose divergence you have localised to 1.8 s is
  still hopeless if the map is chaotic: **11 869 exhaustive single-move
  candidates over the break, 0 finished.**
