# Pain ft Mango & Teuflum

**Every one of the top twenty runs — and the author's own lap — holds 100 % full
lock for all 4.7 seconds of sector 9. That is the one place nobody has looked:
easing fractionally off that lock is worth 0.171 to a machine, and on a keyboard
a single 20 ms blip off the left key at 46.0 s is worth 0.011.**

**Pain ft Mango & Teuflum** — TAS **49.275** (−0.007) | AT 49.282 | WR 49.446 by burntbagels

https://github.com/user-attachments/assets/cb6deb78-4e12-4de0-b9f3-4996eb2d41ef

Both cars are on screen: our keyboard run against Ssnake01's rank-2 lap, sixteen milliseconds apart over forty-nine seconds.

**Pain ft Mango & Teuflum** — TAS **49.275** (−0.007) | AT 49.282 | WR 49.446 by burntbagels

https://github.com/user-attachments/assets/eb3a201a-49e9-4511-8daf-04640f5e64c0

| run | time | vs author time | vs human WR | device |
|---|---|---|---|---|
| **TAS** | **49.275** | **−0.007** | −0.171 | analog |
| TAS, independent tape | 49.275 | −0.007 | −0.171 | analog |
| TAS, earlier | 49.278 | −0.004 | −0.168 | analog |
| **keyboard** | **49.475** | +0.193 | +0.029 | 3 steer values, 59 changes |
| Author time (never beaten by a human) | 49.282 | — | −0.164 | — |
| Human WR — burntbagels | 49.446 | +0.164 | — | analog, 294 steer changes |
| Ssnake01, rank 2 | 49.491 | +0.209 | +0.045 | **pure keyboard**, 57 steer changes |

TMX map [285268](https://trackmania.exchange/maps/285268) · author **Slidelock**
· Stadium Ice · 10 sectors, 49 seconds · **163 recorded runs**.

The keyboard tape is faster than all 72 keyboard runs on the leaderboard and
would sit 3rd overall.

## One named player is one sector away

Nobody has ever put a clean lap together here. **The field's own best sectors add
up to 48.826** — about half a second under the author time — and every sector
carries 0.100–0.400 of spread among the top twenty. The author time stands
because a 49-second ice map punishes consistency, not because the route is
squeezed dry.

Sector ranks within the top twenty (1 = fastest):

| run | final | S1 | S2 | S3 | S4 | S5 | S6 | S7 | S8 | S9 | S10 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 burntbagels | 49.446 | 10 | 10 | 4 | 5 | 4 | 2 | 11 | 5 | 3 | **19** |
| **2 Ssnake01 (kbd)** | 49.491 | 5 | 8 | **1** | **20** | 2 | **1** | 14 | **1** | 7 | **1** |
| 3 Tigu. | 49.535 | 12 | **1** | 10 | 10 | 10 | 4 | 5 | 8 | **1** | 3 |
| 4 thgiN_ (kbd) | 49.541 | 6 | 5 | 12 | 16 | 3 | 5 | 2 | 6 | 11 | 5 |
| 5 Jastastic000 | 49.634 | 8 | 3 | 6 | 3 | 7 | 6 | 4 | 18 | 12 | 9 |
| 6 ChooDawn | 49.640 | **1** | 9 | 2 | 15 | 5 | 3 | 6 | 15 | 8 | 8 |
| 7 Shikurima | 49.720 | 3 | 7 | 15 | 13 | **1** | 13 | 9 | 4 | 13 | 2 |
| 13 Slidelock (author) | 50.140 | 14 | 17 | 11 | **1** | 17 | 16 | 13 | 17 | 6 | 10 |

> **Ssnake01's rank-2 lap is pure keyboard, owns four of the ten sectors, and is
> 20th of 20 in sector 4 — 4.745 against a top-20 median of 4.533. Give that
> same lap a merely median sector 4 and it finishes in 49.279: under the author
> time, on a keyboard, with 58 key presses.**

Everybody else in the top twenty needs three or four sectors to improve.
Ssnake01 needs one, and it is the one they are last in. (Sectors are not fully
independent — a sector time is bought partly with entry speed from the one
before — so this points at the slack, it does not predict a lap.)

## What the field is unanimously getting wrong

On ice this field pins the wheel: 74 % of all race ticks at full lock across 163
runs, 85 % across the top twenty. The machine's whole 0.171 over the world record
comes from 52 ticks of easing **1–8 parts in 127** off that lock, all of it in
the last 6.4 seconds:

| race window | duration | the WR | ours | ease |
|---|---|---|---|---|
| 42.88 – 42.90 | 30 ms | −127 | −103 | 19 % |
| **43.41 – 43.80** | **400 ms** | −127 | **−123** | **3 %** |
| 45.04 | 10 ms | −127 | −124 | 2 % |
| 46.32 – 46.37 | 60 ms | −127 | −126 | 1 % |
| 47.66 – 47.67 | 20 ms | −127 | −123 | 3 % |

The single biggest move on the map is that **400 ms at 97 % lock instead of
100 %, worth 0.124 on its own.** Nothing exotic is happening — everybody is
holding the wheel where the car wants a fraction less.

The rule that survives, stated carefully: **on ice, back off lock where you are
trying to keep the car pointed and accelerating; keep it pinned where you are
trying to rotate.** Do not read the leaderboard's own lock-versus-time
correlation as evidence of anything — it is negative, and all it measures is that
drivers who steer less are the ones correcting mistakes.

## Where the author's 0.164 is

The author's lap is stored inside the map file, so it can be read sector by
sector. It is on the field's route — never more than 6.0 m from the world
record's line, no launcher, never airborne — so this is technique, not a
shortcut.

| sector | author | WR | Δ |
|---|---|---|---|
| S2 | 5.754 | 5.827 | **−0.073** |
| S4 | 4.543 | 4.447 | **+0.096** |
| S7 | **6.636** | 6.839 | **−0.203** |
| S9 | 4.729 | 4.664 | +0.065 |
| S10 | 1.727 | 1.810 | **−0.083** |

The author's S7 is 0.091 faster than the whole field's best S7 — and it is not
won in S7. Against the world record the author is **4.4 km/h slower over the
sector-6 crest** at 28.2 s, then carries **+6 to +8 km/h continuously from 29.6 s
to 37.4 s**. Slow in, fast out, over a seven-second sweep. The author also gives
0.096 back in S4 and 0.065 in S9: the author time is one good lap with two bad
sectors, which is why the field's own sector bests are the honest target.

## The run, as inputs

The drivable tape is the keyboard one. It is Ssnake01's own lap with **two**
things added:

| # | race | input | worth |
|---|---|---|---|
| ① | **46.00** | **release the left key for 20 ms** in the middle of the long full-left hold | −0.011 |
| ② | **48.39** | **tap the brake for 30 ms** | −0.005 |

That is the keyboard expression of the finding above: you cannot hold 97 % lock
on a keyboard, but you can blip off it.

### Sector by sector, off what you can see

Targets are the field's own best sector times.

| sector | what you see | target | what to do |
|---|---|---|---|
| **S1** 0 → 6.5 | standing start to the first gate, climbing to ~215 km/h | **6.501** | the one sector where the field really steers — only 53 % full lock. Do not over-drive it; it barely predicts the result. |
| **S2** 6.5 → 12.3 | 98 % full lock, the most braking of any sector | **5.738** | brake earlier, hold the lock. The author is 0.073 up on the record here. |
| **S3** 12.3 → 18.5 | the slowest section, down to 153 km/h at the exit | **6.052** | Ssnake01 owns it on a keyboard. |
| **S4** 18.5 → 22.9 | 98 % lock, exit at 153 km/h | **4.410** | **the sector that costs Ssnake01 the author time**, and the author loses 0.096 here too. The record lifts the throttle 9 % of the sector and is faster for it. **Lift here.** |
| **S5** 22.9 → 27.0 | acceleration to 220 km/h | **3.968** | 0.331 of spread and it tracks the finishing order closely. Worth real practice. |
| **S6** 27.0 → 31.8 | the crest at 28.2 s — the track climbs 42 → 64 m and turns hard | **4.790** | the least-lock sector on the map (author 27 %, record 32 %, Ssnake01 70 %). **Be 4 km/h slower over the crest.** That sacrifice is what pays sector 7. |
| **S7** 31.8 → 38.6 | the long fast sweep, 250–285 km/h for 6.8 s | **6.727** | the biggest prize on the map and the sector that tracks the final time hardest. You do not gain it here — you carry 6–8 km/h through it because of the crest. |
| **S8** 38.6 → 43.0 | fast, 270–280 km/h at the gate | **4.270** | Ssnake01 owns it. |
| **S9** 43.0 → 47.6 | the long left; **everyone holds full lock for all 4.7 s** | **4.641** | **the blind spot.** Braking in here is good. On a keyboard, blip off the left key for 20 ms around 46.0 s. |
| **S10** 47.6 → finish | the run to the line, 240 km/h | **1.729** | the record is 19th of 20 here, 0.081 off the best. The author lifts the gas through 8 % of it and is 0.083 up. **Lift, and do not brake.** |

## How forgiving it is

The keyboard blip at 46.00 s was swept across placement and duration:

- **Duration matters far more than placement.** 20 ms is right; 80 ms costs
  between +0.130 and +0.600 *everywhere*, and 10 ms is mostly worth nothing.
- At 20 ms, the whole placement band from −50 ms to +30 ms costs at most +0.048,
  and four offsets inside it are worth −0.004 to −0.011. Mistiming it by 100 ms
  costs under 0.050.

That last property is the one that matters for something you will try a hundred
times: it is a **cheap** input to attempt.

The analog 49.275 is a different animal. Its decisive input has no tolerance
basin at all — the neighbouring cells are +0.612, +0.100, +0.045 and DNF — so it
is published as proof that 49.282 is not a floor, not as a technique. Take the
sector table and the keyboard blip from this map, not that tape.

**Honest about difficulty.** S6 → S7 is the real skill: giving up 4 km/h at a
crest to carry 7 km/h for the next seven seconds feels wrong and pays. S4 is the
cheapest fix in the top twenty for anyone whose profile looks like Ssnake01's.
S9 and S10 are the shortest, calmest sectors and the ones where the field's
unanimity leaves unexplored ground. And the first 36 seconds of this map are
brutally sensitive — in simulation, 0.8 % of lock for a single frame anywhere
between 6.5 s and 36.5 s ends the run. A driver is closed-loop and recovers, but
that is why the map is called Pain and why consistency, not peak pace, sorts this
leaderboard.

## Files

| file | what |
|---|---|
| `replays/HUMAN_rank2_keyboard_49491.Ghost.Gbx` | **Ssnake01's lap — the one that is one sector away** |
| `replays/KEYBOARD_49475.Ghost.Gbx` | **the drivable one** — that lap plus two key presses |
| `replays/TAS_49275.Ghost.Gbx` | the fastest run |
| `replays/TAS_49275_independent.Ghost.Gbx` | the same time reached independently — a byte-different tape |
| `replays/TAS_49278.Ghost.Gbx` | the first tape under the author time |
