# `Fall 2025 - 18 CP1 End` — how a human takes the record

**Map** uid `6r7HjKPCuImnLMBfqiKwWpGK1U1`, TMX 270053, by `in-.-`.
**Author time 4.492 s. Best human ever 4.495 s** — AffiTM and four others, tied,
out of **973 recorded runs**. Nobody has ever beaten the author time.

A TAS run validating at **4.492 s** is in this directory
(`tas_4492_v1.Ghost.Gbx`). It equals the author time and beats the human record
by 3 ms. Below is exactly where those 3 ms come from, and — this is the useful
part — **most of them are one coarse, forgiving change that you can practise.**

---

## 1. The map in one paragraph

4.5 seconds. **Full throttle from the lights to the line: never lift, never
brake.** The car stays on the ground the whole way and — the fact that decides
everything — **it never slides**. Lateral speed stays under 0.7 km/h from start
to finish. You are not managing grip on this map; you are managing steering
angle. The shape is a left kink out of the start, a short straight, a small
right correction, then one huge downhill left sweeper taken at **full lock for
about 1.3 seconds**, crossing the line at 216 km/h.

Scale, so you know what you are fighting for: **1 millisecond is 6 cm of
travel at the line.** The whole top 15 of the leaderboard is spread over 3 ms —
18 cm.

## 2. Where the 3 ms are — measured by ablation, not by opinion

Take the human record's tape, change exactly one thing, and re-simulate:

| what you change on the human record | time |
|---|---|
| nothing (the human record itself) | 4.495 |
| **one tick of extra steering lock at 0.42 s** | **4.493** |
| the whole corner-exit release, and nothing else | 4.495 (nothing) |
| everything the TAS does *except* that early lock | 4.495 (nothing) |
| the early lock **and** the exit release together | **4.492** |

So the headline is not the spectacular-looking finish. It is this:

> **You are not turning hard enough in the first half-second.**

The human record holds about 52% steering lock (−66 of 127) through the initial
turn-in. The car wants more. A brief extra stab of lock somewhere in the first
0.8 s is worth 1–2 ms on its own, and it is what makes the better exit possible
— the exit change is worth nothing without it.

## 3. How forgiving is it? (Very.)

This is the part that makes it practical. We enumerated **every** single-tick
steering value at **every** tick of the turn-in phase:

- **Timing.** A single extra stab of lock anywhere between **0.24 s and 0.77 s**
  gains a millisecond. Six different moments in that window gain two. There is
  no single frame you have to hit.
- **Amount.** At the best moment (0.42 s), *any* value from about **−83 all the
  way to full lock −127** produces 4.494; −87 exactly produces 4.493. A ~45-unit
  window on a 127-unit axis. You cannot miss it by being a bit greedy.
- **Shape.** You do not need a stab at all. A gentle extra 3–8 units of lock
  held for 5–10 ticks, or a smooth 10-tick swell of −5, anywhere in that window,
  also gives 4.494. Sustained, smooth, brief, sharp — they all pay.

In driver language: **turn in a little harder in the first half-second than
feels natural, and let it breathe back out.** It costs you almost nothing in
speed at 20–40 km/h, and it sets the whole rest of the lap on a better line.

## 4. The second half: release the exit earlier

Worth ~1 ms, and only once you have the first half right.

The human record holds **full left lock right up to 4.35 s** and then snaps the
counter-steer in. The TAS starts unwinding at **4.16 s** — nearly two tenths
earlier — and rolls it off progressively:

| race time | human record | the 4.492 |
|---|---|---|
| 4.00–4.15 s | −127 (full left) | −127 |
| 4.16 s | −127 | **−124, starts releasing** |
| 4.20 s | −127 | −102 |
| 4.25 s | −127 | −74 |
| 4.30 s | −127 | **−52 (about 40% lock)** |
| 4.35 s | −68 (human starts) | −6 |
| 4.38 s | +96 | **+127 (full right)** |

Why it is faster: what the finish clock measures is the part of your speed
pointing *through* the line, not your speed. While you hold lock the car is
still rotating, and every degree not yet cancelled is speed thrown across the
line instead of through it. Unwinding earlier stops the rotation sooner,
straightens the car onto the line sooner, and lets it accelerate a touch harder
over the last three tenths.

This one is forgiving too, in timing: of **169,793** different exit shapes we
enumerated — every combination of release moment, release rate, how far you
unwind, when you counter-steer and how fast — **54,777 also produce a 4.492**.
There is a wide family of correct exits. Roll off the lock about two tenths
earlier than you do now, and commit to the counter-steer sooner.

## 5. What is TAS-only, and one thing that will bite you

**Not reproducible, and not worth chasing:**
- The exact per-tick numbers (−124, −117, −106, −97 …). That is a computer
  interpolating a smooth ramp at 100 Hz. A human rolling the stick off smoothly
  over the same two tenths gets the same shape.
- The last ~0.1 ms of polish. Our search spent ten million simulations grinding
  it out in 10-microsecond steps. It is not a technique, it is arithmetic.

**Do not go looking for a different line.** The top 14 runs on the leaderboard
are within 30 cm of each other for the entire lap, and an exhaustive search
agrees with them: it never found a better line, only better *inputs* on the same
one.

**The warning.** The finish trigger on this map is narrow and has a hard edge on
the outside. We measured it by sliding the gate: the fast line crosses about
half a metre from that edge. **Half a metre wider costs 10 ms. Two metres wider
and the run does not finish at all** — no time, no explanation, nothing on
screen. It is equally unforgiving underneath: 25 cm lower and the car misses the
trigger entirely. So take "release earlier" as *stop turning sooner*, never as
*run wider*. The early release makes the car straighter, not the line wider, and
that distinction is the difference between a record and a DNF you will not
understand.

## 6. Can a human really do 4.492?

Yes. The author validated the map by driving it, so a human-sized 4.492 exists.
We reached the same millisecond independently, by inputs no leaderboard run
uses, which is corroboration rather than luck.

And the margin is concentrated in a **coarse** decision — more lock early —
inside a **half-second-wide** timing window with a **45-unit-wide** value
window. That is not a frame-perfect trick. That is a driving change.

Our tape's true crossing is **4.49286 s** (measured to 10 microseconds by
walking the finish plane). The author time needs 4.492x; the human record is
**4.49597 s**. The 3 ms are there, and they are in the first half-second.
