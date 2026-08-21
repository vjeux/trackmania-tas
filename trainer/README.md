# Drive it yourself — the input trainer

**[`index.html`](index.html) — open it in a browser. No server, no build, no
network.** Save it and double-click it.

It is a falling-note rhythm game (DDR / Guitar Hero) built from the **real input
tape** of the keyboard 6.323 on
[unluckE - get jiggy with it](../145875-unlucke-get-jiggy-with-it): three lanes,
notes that are *held durations* rather than points, and judgement in real
milliseconds against the tape.

The point of the page: **6.323 is 23 steer events on three values and one brake
tap. A person could learn it.** This is the tool for finding out.

| | |
|---|---|
| tape | `kb6323.csv`, 793 rows at 10 ms, −1.560 → 6.360 |
| ghost | `BEST_KEYBOARD_6323.Ghost.Gbx`, md5 `40472ddf8733aeaa9ec9a9a5322be21a` (793 of 793 rows verified against its own input archive) |
| times shown | our **6.323** · author **6.343** · human WR **6.346** (xeap-.-) |

The CSV is embedded verbatim in the HTML and parsed at load. **Nothing about the
run is hand-transcribed** — every note, duration and coaching number on the page
is measured off those rows when you open it.

## Playing it

`←` `↓` `→` (or `A S D`) are the three lanes: left, brake, right.
`↑` (or `W`) is gas — hold it the whole time.
`Enter` play · `R` retry · `P` pause · `[` `]` speed.

**Gas is not a lane.** The tape puts it down at −0.270 and never lifts it, so a
gas lane would be one solid bar carrying no information and a quarter of the
screen saying "hold this". It is a permanently-lit bar under the keys instead,
and it is *checked continuously* rather than timed: hold it and nothing happens,
come off it during the race — or never press it at all — and the bar goes red
and the results say where. **It is never scored on timing.** Its only press is
at −0.270, during the countdown, and countdown timing is exactly the thing this
page refuses to teach.

The brake keeps a lane, narrow, even though it has one note in the whole run:
it is a real key under another finger *while left stays down*, the only moment
in the run your hands do two things at once. The width it gives up goes to the
two steer lanes, which is where the run is genuinely hard to read.

### The timing windows

Late is the roomy side, because you react to a note arriving and never before
it, and a release gets about double the room a press does — letting go long is
a milder mistake, and judging a 10 ms flick's release as strictly as its press
is simply unplayable.

| | perfect | great | good | late | still a hit |
|---|---|---|---|---|---|
| **press** | −25 / +25 | −45 / +70 | −80 / +140 | −130 / +240 | −200 / +420 |
| **release** | −45 / +45 | −85 / +120 | −140 / +220 | −200 / +360 | −260 / +560 |

Anything inside *still a hit* is a late **hit** — never a miss, and never an
extra. **Extra** means one thing only: a key pressed where the tape has no note
at all. One input is never counted as two errors.

Matching is by **sequence, not by nearest note**. Nearest cannot work here: the
presses at 2.360 and 2.380 are 20 ms apart, so a 12 ms late press on the first
would be stolen by the second and the cluster would unravel. Each input is
credited to the earliest edge in its lane it could legitimately be a hit on, so
playing slightly behind the beat stays a sequence.

Three more things are deliberately not your fault:

- **A drill hands you the state it starts in.** "The burst" opens inside the
  0.520 s right and "Long left→out" inside a left, so those lanes start held,
  the lead-in names them (*ALREADY HOLDING → RIGHT*), and pressing them
  yourself during the lead-in is absorbed rather than counted. Judging the
  release of a note whose press scrolled past before the drill began is the
  definition of unfair.
- **A key pressed before the drill's first note is warm-up**, and letting go
  after the run is over is you stopping — the tape ends still holding gas and
  the final right; it does not let go.
- **Losing window focus drops the held keys and pauses**, because the OS never
  delivers that keyup and the lane would otherwise stay stuck down for the rest
  of the run.

Notes outside the drill are drawn as hollow dashed ghosts: visibly not yours to
play, so the drill cannot bait you into an extra.

You are judged on **edges, not notes**: every held note is a press *and* a
release, 27 judged edges across the race. That is what makes the 10 ms flick at
2.360 scoreable at all — as a note it is invisible, as two edges 10 ms apart it
is the hardest thing on the page.

- **Speed 0.15× – 1.00×.** Windows are measured in *tape* milliseconds, so
  slowing down genuinely widens them: a ±25 ms perfect is 100 real ms at 0.25×.
- **The ladder** (on by default) walks the speed for you: clear a run at 90% and
  it steps up 0.05×, drop below 62% and it backs off, and the results always say
  what it did and why. That is how the burst stops being a blur — doing it by
  hand means thinking about the slider instead of about the run. Turn it off in
  Transport to pin the speed.
- **Note spacing** is a separate slider, because slow motion does not make a
  10 ms note *bigger*.
- **Sections**: Full run · Launch · The burst · Long left→out. The burst is
  where attempts die; drill it alone.
- **Input offset** calibrates your keyboard. If the results keep saying you run
  +18 ms late, dial +18 and the rest of the practice is about the run.
- **Demo mode** drives it for you, so you can watch before you try.
- **Countdown fidget is hidden by default.** The tape carries 23 input changes
  *before* the line while the car is locked; only the state carried across
  0.000 matters, and showing the rest would teach a lie. The toggle exposes all
  43 edges if you want them.

## What the tape says

Measured over the race window 0.000 → 6.323:

- **left 3.820 s · right 1.910 s · centre 0.600 s · brake 0.130 s.**
  Gas goes down at −0.270 and is **never released**.
- 24 state changes after the line; **23 steer segments** — that is the "23
  events". The brake tap is not one of them.
- **The brake tap is a brake-turn.** 0.750 → 0.880, 0.130 s, taken at full left
  with the gas still down. It is not a slowdown: it rotates the car, and you
  never lift.
- **The burst, 2.090 → 3.590, is not 18 inputs.** It is *left held with the
  finger twitching off it*. From 2.250 the centre gaps run 50 → 30 → 10 ms with
  left presses of 30 → 10 ms between them: a converging flutter, like a ball
  settling, not a rhythm. The only two rights in it — 0.080 at 2.710 and 0.210
  at 3.130 — bracket a 0.320 left, and the second cluster stutters again at
  20 / 10 / 20 / 20 ms.
  **The shape to hold in your head is four things, not eighteen: flutter, blip
  right, long left, blip right, flutter, commit.**
- **Three single-tick events exist**: left 10 ms at 2.360, centre 10 ms at
  2.370, centre 10 ms at 3.340. At 1.00× nobody hits them. At 0.15× they are
  67 real ms, which is the only honest way anyone learns them.
- **The commit** is 1.520 s of left from 3.590 — the only place in the run you
  can breathe — then right from 5.230 to the line.

## Building it

There is no build system and no Python. The page is assembled by concatenation:

```sh
cat head.html kb6323.csv tail.html > index.html
```

Checks (Node, no dependencies):

| | |
|---|---|
| `node test.js` | parses the tape, diffs every race transition against the published table, checks the judging rules and the drill arithmetic |
| `node headless.js` | boots the page's own script against a stub DOM and plays whole runs and drills through it |
| `sh playtest.sh` | plays the page **in a real headless Chrome** — real canvas, real `KeyboardEvent`s, real DOM, only the frame clock is ours so the run reproduces |
| `node analyze.js` | raw dump: transitions, notes, burst segments |

The two harnesses answer different questions. `headless.js` is fast and covers
the judging logic; `playtest.sh` is the one that catches things a stub cannot
fake — it found a missing `setLineDash` and confirmed a real browser scores an
on-tape run **S+ 100%, 27/27 perfect, 0 miss, 0 extra**, and a 60 ms-late
player **C 72.2% with still no misses and no extras**.
