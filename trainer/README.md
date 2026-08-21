# Drive it yourself — the input trainer

**[`index.html`](index.html) — open it in a browser. No server, no build, no
network.** Save it and double-click it.

It is a falling-note rhythm game (DDR / Guitar Hero) built from the **real input
tape** of the keyboard 6.323 on
[unluckE - get jiggy with it](../145875-unlucke-get-jiggy-with-it): four lanes
for the four controls, notes that are *held durations* rather than points, and
judgement in real milliseconds against the tape.

The point of the page: **6.323 is 23 steer events on three values and one brake
tap. A person could learn it.** This is the tool for finding out.

| | |
|---|---|
| tape | `kb6323.csv`, 793 rows at 10 ms, −1.560 → 6.360 |
| ghost | `KEYBOARD_6323.Ghost.Gbx`, md5 `25e04568c299eccdb867c107f40ed650` |
| times shown | our **6.323** · author **6.343** · human WR **6.346** (xeap-.-) |

The CSV is embedded verbatim in the HTML and parsed at load. **Nothing about the
run is hand-transcribed** — every note, duration and coaching number on the page
is measured off those rows when you open it.

## Playing it

`←` `↑` `↓` `→` (or `A W S D`) are left / gas / brake / right.
`Enter` play · `R` retry · `P` pause · `[` `]` speed.

You are judged on **edges, not notes**: every held note is a press *and* a
release, 28 judged edges across the race. That is what makes the 10 ms flick at
2.360 scoreable at all — as a note it is invisible, as two edges 10 ms apart it
is the hardest thing on the page.

- **Speed 0.15× – 1.00×.** Windows are measured in *tape* milliseconds, so
  slowing down genuinely widens them: a ±20 ms perfect is 80 real ms at 0.25×.
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
  50 edges if you want them.

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
| `node test.js` | parses the tape, diffs every race transition against the published table, checks the judging rules |
| `node headless.js` | boots the page's own script against a stub DOM and plays whole runs through it |
| `node analyze.js` | raw dump: transitions, notes, burst segments |
