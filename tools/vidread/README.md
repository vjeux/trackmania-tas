# vidread — reading a Trackmania run off a screen recording

`vidread` turns a video of a car driving into numbers: the race clock, the
speed readout, and the state of an input overlay, per frame. It was written to
attack one specific problem — reconstruct Wirtual's tool-assisted run on
*Cobalt Cove* from the published video and nothing else — and everything in it
is shaped by what that video actually contains.

It has no dependencies. Frames arrive as raw `rgb24` on stdin and ffmpeg is the
decoder:

```
ffmpeg -v error -ss 523 -t 76 -i v.webm -f rawvideo -pix_fmt rgb24 - \
  | vidread read --field 2403,1326,31.3,34,62,3 --templates speed.txt \
                 --fps 60 --t0 0.0047
```

Every frame-reading subcommand prints a TSV whose first column is that frame's
time. What that time *means* is the caller's business: pass `--t0` in video
seconds and it prints video time; pass the run's own zero and it prints race
time.

## The commands

| | what it does |
|---|---|
| `lamps` | the five-lamp key overlay (BRAKE and the four arrows), per frame |
| `sections` | contiguous stretches of a `lamps` table where the overlay is up |
| `ink` | the ink profile of a rectangle, summed over every frame — how digit cells are placed |
| `patches` | a PGM contact sheet of digit cells, for labelling glyphs by eye |
| `train` | build templates, from eye labels or by bootstrapping a previous pass |
| `read` | read a digit field on every frame |
| `trace` | a `read` table as a numeric series, with impossible readings removed |
| `keytape` | the clock and the lamps joined into an input record indexed by RACE time |
| `align` | where in the run a clip sits, by matching its speed to a reference trace |
| `xcheck` | does a clip show the same run as the reference, at the same race time |

## What is hard about this, and what the code does about it

**Lamps are easy; everything else is not.** The key overlay is composited by
the video editor at a fixed screen position, in two flat greys. It reads
cleanly. The *game's* readouts — clock, speed — are inside the footage, so they
move whenever the editor reframes a clip, and they are white text with no box
behind them.

* **The grey levels move between clips.** Unlit read 71.0 in one clip and 90.0
  in another, because the clips are graded differently. `lamps` therefore reads
  each lamp against its own light frame — a ratio, not a level — and calls the
  overlay present only when all five lamps sit clearly in one of the two
  states with the gap between them empty. Five lamps agreeing is what makes it
  safe; one bright rectangle in a scene is not.
* **White text on a white background is not recoverable.** On the pale surfaces
  of this map the speed readout disappears completely: not faint, absent. About
  a quarter of the final replay's frames are unreadable for this reason and the
  tool reports them as gaps rather than guessing.
* **Cells are fixed once, from the ink summed over every frame** (`ink`), never
  from a per-frame bounding box: a box tracks the ink *threshold*, so the same
  digit comes out a pixel wider on the next frame.
* **Scoring is contrast-normalised correlation** over a softened patch, and the
  whole field is shifted together over a small window — the digits of a readout
  are one sprite and do not move independently.
* **A right-aligned field pads with BLANK, not with a zero.** A blank cell has
  no glyph to match, so it is called blank when its own score collapses while
  the cells to its right stay legible.
* **Nothing is fitted to the decoded digits.** `trace` drops a reading the
  matcher was *confident* about but that the car cannot have done — the failure
  mode that matters, because a confident wrong digit is invisible in the score
  column.

## The controls

A reader is worth nothing until something known has been through it.

* **`align` against the reference's own table** returns rate 1.000, the right
  offset, 99.6 % agreement, and a runner-up peak at 43.7 % — so the instrument
  finds a true placement and the peak is distinct.
* **Two instruments, different pixels, same answer.** On the clip at video
  405–419 the race clock (OCR of the game's bottom-centre timer) and the speed
  aligner (matching the HUD speed against the finished replay) place the clip
  at race 50.06 s / rate 0.698 and race 50.26 s / rate 0.680 respectively.
  Neither can borrow from the other.
* **Negative controls that fire.** On clips of an *earlier* build of the run,
  `align` finds no peak (fit 30–32 % against a 28–30 % runner-up) and `xcheck`
  reports a median speed disagreement of tens of km/h. The instrument declines
  rather than inventing a placement.
* **`lamps` positive and negative controls** are frames read by eye: at video
  519.03 the overlay reads up+left with brake, down and right dark, which is
  what the frame shows; at 540.03 the overlay is absent and `present` is 0.
