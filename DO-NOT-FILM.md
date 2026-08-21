# Maps that must not be filmed, and why

Every entry here was established by measurement, not by impression. A map on this
list will otherwise cost somebody a box re-deriving the same answer — that has
already happened twice.

The audit tooling is on WhiteStick at `~/tj/src/bin/`: `seplag` (compares two
recordings at every integer time offset, since sample times are *session* times
and index alignment across recordings is meaningless), `ghostqc`, `spdcheck`,
`whlvar`, `c3speed`.

## 227654 "The BLEV Special" — the published telemetry is the human's

All eight of our published tapes carry the human world-record holder's recorded
trajectory. Not similar to it — **identical to it, bit for bit**:

| file | vs `HUMAN_WR_retries_cut_64871` |
|---|---|
| `TAS_57493`, `_57498`, `_57573`, `TAS_59912_watchable` | **365 of 365 samples exactly equal** |
| `TAS_57503` | 364 of 365, after diverging 2004 m — a splice |

The times may well be honest: the oracle validates the *input tape*, and the tape
is not what is spliced. What is wrong is the *record*, which is what a video
shows. Filming any of these produces a video of the human's driving captioned as
ours.

This is the page claiming the author time beaten by 0.371, so it is the most
damaging one to get wrong. Note also that the record is 365 samples = 18.25 s for
runs of 57–64 s: it only ever covered the first third of the run.

**Repairable in principle** by regenerating from the input tapes.

## 238835 "Turtle Trial Angustus" — no reference exists to grade a repair

Its seven tapes are **independent of each other** (verified at every offset — this
corrected an earlier finding that said otherwise, which had been measured with a
session-time-keyed tool that could not compare across recordings). The problem is
different: the map's **only human file is itself the donor**, so there is nothing
uncontaminated to check a regeneration against.

Also `AUTHORCUT_246602_watchable` has **every sample at exactly (0,0,0)** — no
position telemetry at all.

**Not repairable** until a downloaded human recording of this map exists.

## 165922 "IDM Ruinin ur day 460" — the files contain no car

All nine have **no `CSceneVehicleVis` entity**. There is no vehicle record to
decode, so they cannot be filmed *and cannot be checked* by any of the tooling.

This is also the explanation for a long-standing mystery: `01_KEYBOARD_16276_tolerant`
kills the game on import. It was blacklisted as a crash for weeks. The cause is
simply that there is no car in it.

**Needs telemetry synthesised, not regenerated** — there is nothing to regenerate
from.

## 186935 "Magnet Trial" — `BEST_793893` has no positions

Every sample exactly (0,0,0). An all-null file is precisely the shape that passes
a gate vacuously, which is why the "does the car travel any distance" check
exists. `CUT_795034` is separately a splice at 1021× the plausible speed ratio.

## 286279 "Turtle Trial LETO" — six files carry a donor's trajectory

| file | vs | evidence |
|---|---|---|
| `TAS_235625`, `TAS_analog26_235814`, `KEYBOARD_235939` | `HUMANCUT_236972_watchable` | **2366 consecutive identical positions** |
| `BEST_218812`, `KEYBOARD_218877` | `AUTHORMIN_831ev_354781` | 10 samples re-converge after 652 m |
| `MINIMAL_832ev_219581` | `AUTHORCUT_220391` | **3046** samples re-converge after 64 m |

## 279218 `TAS_5345_starttrick` — the file, not the map

112 of 112 samples identical to the human `r001` 5.355. The map's other five
tapes are clean and one is published. Do not film **this file**.

## 146612 "Spaghetti Nights 2" — the game will not open the map

Different in kind from the above: the ghosts are fine, the *map* cannot be
loaded. See `146612-spaghetti-nights-2/CANNOT-OPEN.md` for what was ruled out.

---

## How contamination was established, so the method can be re-run

A shared bit-identical **prefix proves nothing** — the simulation is
deterministic, so two runs with the same opening inputs produce identical f32
positions. Our own sibling tapes routinely share 67 % of their samples.

Two things are proof:

- **Re-convergence.** Once two runs are metres apart they are different physical
  states, and no input sequence returns them to *exactly* 0.000000 m. identical →
  diverge → exactly identical again can only be a splice.
- **Wholesale identity** with a human recording — the file simply *is* that
  recording, possibly truncated. This catches a splice whose second seam was cut
  off, which re-convergence alone cannot see.

Compare only against **human recordings**; same-pipeline siblings trip the
re-convergence test legitimately. And restrict the window to the race: a shared
post-finish carrier tail makes two independent runs agree exactly after the line,
which is a tail problem and not a provenance one.

## 285268 "Pain ft Mango & Teuflum" — eight tapes, one human's trajectory

Found 2026-08-21, while this map was queued as the *best-looking* two-car pairing
in the set. All eight of our tapes decode to a human's trajectory, seven of them
to the same one:

| file | sample-CSV md5 | whose run it is |
|---|---|---|
| our 49.275 BEST, 49.278, 49.285, 49.288, 49.311, 49.355 | `12333c2541b62c9b5a854d1950b35050` | burntbagels' 49.446 |
| downloaded `rank001_49446` (burntbagels) | `12333c2541b62c9b5a854d1950b35050` | — |
| our 49.475 keyboard | `f4639ab2917031c2e6d69efa0e107ee4` | Ssnake01's 49.491 |
| downloaded `rank002_49491` (Ssnake01) | `f4639ab2917031c2e6d69efa0e107ee4` | — |

Every one re-simulates to its own claimed time, so the **results** stand; what is
unreliable is the recording of how they were driven, which is the only thing a
video shows. A two-car clip here would have been burntbagels racing himself.

The trap worth remembering: its separation profile was the healthiest on the
board — **868 of 986 samples** in the "two visible cars" band — because two
copies of one lap, offset slightly, look exactly like a close race.

## 279218 `TAS_5345_starttrick` — the file is Matik_K's run

One file on an otherwise filmable map. The published 279218 clip is not this
tape; do not swap it in.

## 134672 "kekl sausage ice" — held twice over

Its gate refuses `ksi_67319_watchable_v2` at C5/C7 for carrier-owned contact and
surface bytes. Independently, its two files disagree with **each other**:
`ksi_67319.Ghost.Gbx` decodes with `race_time=68442`, 1.123 s away from the
67319 in its own filename, while `ksi_67319_watchable_v2` decodes to 67319 with a
different split vector. One of those names is wrong, and until it is known which,
neither is safe to caption.

## Not a map, but the same disease: check the pairing, not just the file

Before any two-car shoot, decode **both** ghosts and compare sample-CSV md5s. The
gate cannot help here: a separation of zero and a separation you cannot see
produce the same verdict. See `VIDEO-UPLOAD-NOTES.md` trap 5.
