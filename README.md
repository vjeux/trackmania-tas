# trackmania-tas

Tool-assisted runs that beat **author times nobody has ever beaten**, on
Trackmania 2020 maps tracked by [unbeaten.at](https://unbeaten.at/).

Every run here was found by search against the game's own physics — the
headless `TrackmaniaServer` re-simulating input tapes — and then **re-validated
through a clean oracle** before being published. The point is not the replay.
The point is the **technique**, written up so a human can practise it.

## Results — author time beaten

| map | records | author time | best human | **this TAS** | vs AT | keyboard-only |
|---|---|---|---|---|---|---|
| [The Magnet Trial](186935-magnet-trial) | 7 | 2540.641 | 2575.154 | **793.893** | **−68.8%** | — |
| [[Turtle Trial] Angustus](238835-turtle-trial-angustus) | 1 | 462.982 | 1964.933 | **239.133** § | **−48.3%** | — |
| [[Turtle Trial] Leto](286279-turtle-trial-leto) | 5 | 355.181 | 441.002 | **218.812** | **−38.4%** | **218.812** (3 values) |
| [Welcome to wiggles](197047-welcome-to-wiggles) | 21 | 100.784 | 101.794 | **95.839** | **−4.945** | **96.412** (2 keys) |
| [Spring 2023 - 24 (2-UP)](199100-spring-2023-24-2up) | 6 | 51.602 | 52.202 | **49.778** | **−1.824** | **51.062** (3 values) |
| [Tap water 01](173636-tap-water-01) | 602 | 23.325 | 23.638 | **22.072** | **−1.253** | 23.125 (40 ms grain) |
| [YEET Fall 2024 - 04](203072-yeet-fall-2024-04) | 272 | 11.334 | 12.083 | **10.640** § | **−0.694** | **10.743** (14 presses) |
| [Kacky Reloaded #290](126859-kacky-reloaded-290) | 22 | 24.062 | 24.342 | **23.416** | **−0.646** | 24.164 |
| [idm ruinin ur day #460](165922-idm-ruinin-ur-day-460) | 1 | 15.643 | 8790.769 | **15.217** | **−0.426** | **15.217** (keyboard from 4.56 s) |
| [impossible at for ssano](249521-impossible-at-for-ssano) | 147 | 14.648 | 15.039 | **14.289** | **−0.359** | **14.349** (3 values) |
| [Torment (1-DOWN)](228811-torment-1-down) | 48 | 20.555 | 22.637 | **20.237** § | **−0.318** | — |
| [The Blev Special](227654-the-blev-special) | 2 | 57.853 | 147.031 ¶ | **57.573** | **−0.280** | — |
| [U10S_32 [Yeet] MAX-UP](274191-u10s-32-yeet-max-up) | 3 | 7.704 | 7.893 | **7.463** | **−0.241** | **7.476** (15 presses) |
| [Great wtf of what #165](227969-great-wtf-of-what-165) | 42 | 8.127 | 8.197 | **7.998** | **−0.129** | **8.075** (14 inputs) |
| [unluckE - get jiggy with it](145875-unlucke-get-jiggy-with-it) | 46 | 6.343 | 6.346 | **6.322** | **−0.021** | **6.323** (23 inputs) |
| [Fall 2025 - 13 Reverse CP1 End](279209-fall-2025-13-reverse-cp1-end) | 334 | 6.595 | 6.604 | **6.578** | **−0.017** | **6.595** (19 inputs) |
| [surely my least cooked at](252289-surely-my-least-cooked-at) | 706 | 3.851 | 3.867 | **3.836** | **−0.015** | **3.844** (WR+2 keys: 3.848) |
| [Get in the Hole ( Impossible )](203330-get-in-the-hole-impossible) | 5 | 13.995 | 14.018 | **13.984** | **−0.011** | **13.986** (12 inputs) |
| [Training - 10 Long](191465-training-10-long) | 856 | 13.080 | 13.081 | **13.071** | **−0.009** | **13.075** |
| [Pain ft Mango & Teuflum](285268-pain-ft-mango-teuflum) | 160 | 49.282 | 49.446 | **49.275** | **−0.007** | 49.475 |
| [Fall 2025 - 01 Reverse CP1 End](279197-fall-2025-01-reverse-cp1-end) | 561 | 10.598 | 10.602 | **10.594** | **−0.004** | 10.606 (16 detents) |
| [Fall 2025 - 22 Reverse CP1 End](279218-fall-2025-22-reverse-cp1-end) | 339 | 5.350 | 5.355 | **5.347** | **−0.003** | — |
| [Fall 2025 - 16 CP1 End](270051-fall-2025-16-cp1-end) | 903 | 4.831 | 4.834 | **4.830** | **−0.001** | 4.834 |
| [Fall 2025 - 18 CP1 End](270053-fall-2025-18-cp1-end) | 973 | 4.492 | 4.495 | **4.492** | **±0** | — |

## Not beaten — where each one actually stands

Every map we have opened is here, whether or not it fell. Several are still being
worked; the times below are the best validated on the untouched map at the time
of writing, and where they beat the human world record that is said too.

| map | records | author time | best human | **this TAS** | short of AT by | vs human WR | what stands in the way |
|---|---|---|---|---|---|---|---|
| [bald turtle #35](267859-bald-turtle-35) | 19 | 10.768 | 11.169 | **10.859** | **0.091** | **−0.310** | the closest live target here — one session of search so far, and all 19 records re-simulate exactly |
| [Spaghetti Nights 2](146612-spaghetti-nights-2) | 181 | 38.530 | 40.223 | **39.460** ‡‡ | 0.930 | **−0.763** | the 190 m gap jump that looked decisive is **dead** — it reaches CP5 1.128 s early and repays all of it within 82 m, and its exit is closed by a ballistic invariant. What works instead is scoring at the *next* checkpoint: arriving at CP5 earliest is not arriving best |
| [Fall 2024 - 25 (Pure Wet Icy Wood)](210218-fall-2024-25-pure-wet-icy-wood) | 30 | 94.477 | 96.281 | **96.068** | 1.591 | **−0.213** | the car model explains 1.6 % of yaw, so every steering prior we own is void. But **the field's own per-sector minima sum to 91.826 — 2.651 UNDER the author time** (93.847 after discarding every sector that could have inherited speed): every sector of a winning lap has been driven, nobody has assembled one |
| [Impossible Mini Trial 2](267460-impossible-mini-trial-2) | 1 | 16.888 | 23.068 | **21.918** | 5.030 | **−1.150** | 16.888 does not decompose into any launch + flight + endgame two independent agents can build; best construction ≈ 21.3 |
| [finish is on the roof to your right](285885-finish-is-on-the-roof) | 3 | 43.079 | 61.229 | **50.229** | 7.150 | **−11.000** | the finish trigger is **closed by measurement** — it tests a point on the car's *roof*, and every source of the ~26° tilt it needs costs 5.5× the time budget. But rank 1's flip is a validated human way to finish at 11.2 s, and the approach it needs has **never been searched**: ~6 s is unclaimed in one 14 s stretch |
| [KEKL- SAUSAGE ICE](134672-kekl-sausage-ice) | 15 | 58.687 | 68.442 † | **67.404** | 8.717 | **−1.038** | a 1/127 steer error e-folds in 0.7 s; the AT is 4.8 s below the field's best-sector splice, the 2022 WR, *and* our own per-sector optima |
| [Torment (1-UP)](228607-torment-1-up) | 23 | 20.258 | 24.902 | 24.854 ‡ | 4.596 | −0.048 | the map's decisive sector does not order the field; published for the pre-registered experiment that **failed** here |
| [YOU LOVE WATER](284238-you-love-water) | 1 | 50.459 | 440.238 | **97.325** | 46.866 | — | the map is one module ×4; a **sibling map where a human beats the author time on the same obstacle** is now the answer key |
| [P-Found - Pokeuuu](153527-p-found-pokeuuu) | 1 | 939.283 | 5661.335 | — | — | — | not a target: cutting every retry out of the only run still leaves 1214.585 against a 939.283 author time |

† 63.546 is the all-time human record on 134672, set on a 2022 build; it does not
re-simulate on a current one. 68.442 is the best human record set on a build the
game still runs.
‡ 228607's tape re-simulates on the untouched map, but no human ghost for that
map was banked, so the re-validation could not carry a known-answer control. No
replay is published for it — see that page.
§ **Not re-verified in the latest audit pass.** These three maps have their
headline control ghost archived, but not the wider *field* behind their
field-reproduction claims, so those claims are not reproducible from our archive
alone. The times themselves were validated with a control when they were found
and are carried forward unchanged.
‡‡ **146612 is moving fast.** Several arms are live and the figures change hour
by hour; every published tape re-validated on the untouched map with both human
records exact in the same batch. A **segment sum** of 39.229 also exists, and on
this map it is a weaker object than usual: **every sector boundary anyone has
tested is inseparable** — a tape 0.263 s faster to CP4 than any human returns
`DNF cps=4` — so it is not "these might not compose" but "each of these is
achievable and demonstrably breaks the next piece". The table states only what
has been driven end to end.
¶ **147.031 is not the gap on 227654.** That record contains eleven respawns; the
same human's own driving with the retries spliced out is **64.871**, and that is
the number the −0.280 should be read against. See that page.

**227654 is current best, not final** — the owning search is still improving and
that row is expected to move down tonight.

All times in **seconds**.  "Best human" is the online world record at the time
of the run. Every author time in both tables had **never** been beaten by a
human.

## Why an unbeaten author time is interesting

On these maps the author time is usually a **driven validation lap** — a real
person sat down and hit it, often the map's own author. It is not a formula and
not a theoretical bound. So when a map has hundreds of recorded runs and the AT
still stands, something specific is going on: a line nobody tried, or a technique
people know about and cannot hold.

That makes "a computer went faster" the boring half of the result. The
interesting half is *what the computer did differently*, and whether it can be
handed back to the people grinding the map.

(Two maps here carry a caveat worth flagging honestly: on
[210218](210218-fall-2024-25-pure-wet-icy-wood) and
[267460](267460-impossible-mini-trial-2) there is no author ghost in the map
file, unbeaten.at reports the time as plugin-set, and the map's own author sits
well behind it on the leaderboard. **That means we cannot read the author's line
off those maps — not that no such line exists**, and on 210218 a census of the
author's other maps closes the question the other way: nine of his nine other
author times are beaten by a human, two of them by the author himself. A
1.804 gap there is an ordinary margin for that series. We initially drew the
stronger conclusion on that page and have retracted it.)

Four findings from this repo that generalise:

- **On [Great wtf of what #165](227969-great-wtf-of-what-165)**, the whole field
  rolls the car onto its side through the final wall-ride and pays a third of
  its speed to the kicker. Arriving flat and square instead carries 69.2 m/s
  into the finish plane against the world record's 57.3. The keyboard version of
  this run uses **12 steering inputs against the world record holder's 11**, on
  the same three key values, and is **0.122 s faster**.
- **The part of a map that looks decisive usually is not.** On
  [279197](279197-fall-2025-01-reverse-cp1-end), intermediate gates across ranks
  1 to 502 show the closing sweeper costs *everyone* the same 1.100–1.110 s: a
  0.198 s field spread compressed into 0.010. On
  [270051](270051-fall-2025-16-cp1-end) the dramatic closing jump spreads 0.005
  across the field and correlates 0.07 with finishing order, while the quiet
  stretch at 2.4-3.7 s correlates 0.43. Both maps are won long before the part
  that looks hard.
- **Optimise for robustness and you get a teachable input.** On 270051 a
  speed-first search found the last millisecond as a one-tick 75%-lock stab, an
  unteachable lottery ticket. Scoring by the *worst* time over a placement
  window found the same physical effect as a three-tick, 7%-of-lock brush with a
  30 ms window — matching the author time with ±10 ms of slack on every input.
- **A map author who reuses modules has published an answer key.**
  [YOU LOVE WATER](284238-you-love-water)'s obstacle appears byte-identically on
  four of its author's other maps, and on one of them a human beats *that* map's
  author time driving the same geometry cleanly. That single ghost settled what
  the obstacle demands, and showed our field's launch fails on **sideways
  velocity, not speed**.

## Low-input runs

A tape of per-tick analog micro-corrections is worthless to a human. Where
possible each map also gets a **drivable** version: fewer input *change events*,
and a restricted value alphabet — pure keyboard (`left / nothing / right`) or a
small action-key ladder. The alphabet is read off the human world record's own
tape rather than assumed.

On three maps the keyboard-only run beats the author time outright:

- **idm ruinin ur day #460 — 15.217** on `{-127, 0, +127}` from race 4.56 s (AT 15.643)
- **Training - 10 Long — 13.075** on `{-127, 0, +127}` (AT 13.080)
- **Great wtf of what #165 — 8.075** on `{-127, 0, +127}`, 14 input changes (AT 8.127)

Those are the most useful artefacts here: same input device, comparable input
budget, faster than anyone has driven.

And on 165922 the keyboard tape is not a concession — it is **the fastest run on
the map**, ahead of the analog champion at 15.224. Restricting the alphabet found
time rather than costing it.

**Counting convention.** "Input change events" means *any tick where steer, gas or
brake differs from the previous tick*, counted over the whole tape including
pre-start ticks. Other conventions exist — counting only the axis that changed,
or emitting an explicit initial value per axis — and they differ by a handful of
events on the same file. Where two agents measured the same tape with different
rulers, the page says so.

## Findings

[`FINDINGS.md`](FINDINGS.md) collects the per-map results and the
transferable findings in one place.

## Layout

```
<mapid>-<slug>/
  README.md      what the run does and how a human would drive it
  replays/       .Ghost.Gbx replays — the validated time is in the filename
  inputs/        the same runs as input tapes (tick scripts / per-tick JSON)
  notes/         raw working notes and oracle validation transcripts
```

## Validation

Every published replay was re-simulated by the dedicated server against a
pristine copy of the map, in a fresh process, with a known-answer control (a
downloaded human ghost) in the same batch. A replay is only listed here if it
returns the millisecond in its filename.

This discipline is not decoration. Silent-corruption bugs keep being found —
searches that reported improvements which did not exist — and **every one of them
was caught by that control**. The corollary is on the
[Torment (1-UP)](228607-torment-1-up) page: where no human ghost exists to serve
as a control, no replay is published, however well the tape re-simulates.

## Maps

Map files are not redistributed here. Each map's README links to it; the
`.Map.Gbx` comes from Nadeo's own endpoint or from
[trackmania.exchange](https://trackmania.exchange/).

## Rules this project follows

1. **Nothing here has been or will be submitted to an official Nadeo
   leaderboard.** These are study replays.
2. Every claim is re-validated on the untouched map before it is published.
3. Public APIs are rate-limited and identify themselves honestly.
4. A retraction gets published next to the thing it retracts, not instead of it.

## Credit

The human runs referenced throughout are other people's work, and the route on
most of these maps is theirs — the TAS usually changes a handful of inputs on a
line the field established. Map authors and world record holders are named in
each map's README.
