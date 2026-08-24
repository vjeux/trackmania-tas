# Miru's Hell 2

**This is not a driving map. It is a wall of 1,173 spinning rotors, and the whole
run is one question: can you arrive in a state the upper row will accept? The
author time is now beaten — 18.160 against 18.806.**

**Miru's Hell 2** — TAS **18.160** (−0.646) | AT 18.806 | WR **9.075** by AshenBeast1985

> ### The 18.160 is published here now — `replays/TAS_18160.Ghost.Gbx`
>
> **This directory had no `replays/` at all until 2026-08-24, so the number at
> the top of this page named a run nobody could download.** That was a
> publishing gap, not a missing run: the tape has been in the store since
> 2026-08-21 and the plain dedicated server has always re-simulated it to
> 18.160.
>
> What was in the store was a TAPE IN SOMEBODY ELSE'S CONTAINER. It declared
> **25.681** — the carrier's time, not this run's — and carried a stranger's
> storage locator and account id (`Herrlille`), and the recording beside the
> tape was the carrier's rather than this run's (kappa 0.976 against a 25.681
> record). None of that is publishable, and none of it was the run's fault.
>
> The file here is that tape regenerated on the live engine with
> `--inputs --carrier layout`, and it verifies end to end:
>
> | | |
> |---|---|
> | declared time | **18.160**, one copy, and the ghost-result chunk agrees |
> | kappa (tape vs its own recording) | **1.000**, 364 of 364 samples |
> | plain oracle on the WRITTEN file | **18.160** |
> | identity | login `TAS`, our own livery, no account id, no locator |
> | span | telemetry 0.000 .. 18.150 inside a record ending 18.160 |
>
> *Read before trusting the store directory:* it also holds
> `hl_ATREC_208024_18806.Ghost.Gbx`, which is a **different run** at 18.806, and
> four `*_WITHDRAWN_doubled` files that a de-skinning bug doubled in size. The
> 18.160's tape and its two watchable variants are not among the withdrawn
> ones — they were made after that bug was found, and they are the newest
> entries in the store's own manifest. The `RETRACTION` note in the same
> directory is about a claim on the map's reachable set, not about this run.

> ### ⚠️ SUPERSEDED 2026-08-24 — a human is now 9.085 ahead of this run
>
> **What this page said, and what is left of it below.** Two claims here have
> fallen and they are not deleted, because rule 4: *"2.945 inside the human
> record"* (deeperjungle's 21.105) and *"18.160 is the fastest time anyone has
> recorded on this map"*. Both were true when written, against a three-run
> board topped by 21.105.
>
> **MEASURED — board fetched 2026-08-24 03:32–03:37 UTC**, one 76-GET pass,
> all HTTP 200. **AshenBeast1985 finished this map in 9.075 on 2026-08-21
> 20:44:52 UTC**, and the board now holds **4** records. *Control:* the same
> pass re-read every board the repo tracked — 37 at the time — and **28 came back with exactly
> the record the page here already printed** — so the pass reads boards
> rather than manufacturing them. The nine that moved are named in
> [`../LEADERBOARDS.md`](../LEADERBOARDS.md).
>
> **What survives, and it is the point of the page: the author time is still
> beaten.** 18.160 against 18.806 is **−0.646**, and that margin is against
> the map, not against the field. What does not survive is every comparison
> on this page to *the human record*: we are **9.085 behind** the run now at
> the top of this board, not 2.945 clear of it.
>
> **And there is still no file.** This directory has **no `replays/`
> directory at all** — `tmtraj corpus shipped` calls it `NO-REPLAYS-DIR` —
> so the 18.160 is a validated time with nothing here a reader can download.
>
> **UNKNOWN: what the 9.075 actually does.** Nobody here has downloaded or
> re-simulated it, so which of this map's fifteen finish gates it crosses,
> and whether its launch is reachable from the author's own mechanism, is
> not known. `ghost inspect` on the downloaded replay plus a plain-oracle
> re-simulation would settle it. That is the open task on this map, and it
> is a more interesting one than the page's old ending suggested.

https://github.com/user-attachments/assets/6df66b41-5469-4008-ac02-e8a1b5566c91

**The author time is beaten, by 0.646.** 18.160 is **0.646 under the 18.806**.
This is the first author-time beat on this map, and **the 0.646 against the
author is the result.**

> *Retracted, kept in place:* this paragraph used to continue *"and 2.945
> inside the human record — deeperjungle's 21.105, set on 2026-08-20 and still
> standing when this board was re-pulled on 2026-08-21… the human board has
> three runs on it and has not converged, so being 2.945 clear of it says
> little."* The hedge was right and the number is dead: on 2026-08-21 the
> board gained a 9.075. See the banner above.

**Re-shot 2026-08-24 from the newly published `replays/TAS_18160.Ghost.Gbx`.**
Same treatment — one camera, both cars, chase on our car, input overlay — but
**the opponent is not the same driver.** The clip this replaces was shot against
deeperjungle's 21.105, the record at the time; **that ghost is not in the store
and cannot be re-downloaded**, so the second car here is the rank-1 human file
this project does hold, `ghosts/hl_rank00001_23689_lqpzz`, at 23.689. It is
neither the old opponent nor the current record holder (AshenBeast1985, 9.075),
and saying so is better than letting the substitution pass as if nothing
changed. The paragraph below describes the deeperjungle pairing and its
separation figures are that pairing's.

One thing measured on the substitute while checking it: it re-simulates to
**23.665** on the plain oracle while declaring 23.689, a 24 ms disagreement.
That is a property of the downloaded file, not of our run, and it is why it is
in the frame rather than in a claim.

**One camera, both cars.** Ours is the magenta car the camera follows;
deeperjungle's 21.105 — the record when this was shot — is in the same frame,
not in a second pane. They are
within **25 m of each other for the first 15.5 s** — 232 of 364 paired samples
sit in the band where two cars read as two cars — and then ours goes: **42 m
apart at 16 s, 92 m at 17 s, 183 m at 18 s.** deeperjungle leaves the shot in the
last two seconds and does not come back, which is the whole point of the clip.
The earlier 19.427 below is a split screen because *that* pairing separates from
the start; this one does not need one.

**The clip is 18.4 s and the runs are 18.160 and 21.105, which needs saying.**
A MediaTracker camera lives exactly as long as the ghost it is bolted to, and
ours stops sampling at 18.150. The full render runs to the longer block at
21.13 s — and measured frame to frame, **everything after 18.20 s is a frozen
still**, clock stuck at 18.160, deeperjungle not drawn. That is 2.9 s of dead
picture that `blackspans=0` and a duration check both pass. It is cut. Nothing
was lost with it: the last live frame is our car crossing the ring.

**What it cost to get a file worth filming.** Across **52 regeneration attempts
on this tape, 4 landed on the true clock — under 8 %.** The rest cluster at
7.78 m, 24–25 m, 403 m and 1368–1372 m from the run's own route. The two winners
in the final batch are byte-identical to each other, and they were picked by
ranking every attempt against ground truth rather than by taking the answer the
attempts agreed on: **the largest agreeing cluster was one of the wrong ones.**
The filmed file is the second reconstruction, `mh2_WATCHABLE_18160_v2`
(md5 `0f63623a…`), which sits **2.5 mm** from the tape's own route dump where the
first sat 0.865 m.

**Miru's Hell 2** — TAS **19.427** (+0.621) | AT 18.806 | WR **9.075** by AshenBeast1985 *(the opponent in this clip is deeperjungle's 21.105, the record when it was shot)*

https://github.com/user-attachments/assets/d7f87580-9b89-4ae3-9eed-ea3f0b232053

**The clip above is the 19.427, an earlier tape, against the record.** Ours on
the left, deeperjungle's 21.105 on the right, both clocks from the same start.
They take visibly different lines through the red structure from about 6 s, and
by 12.6 s ours is a whole section ahead. It is two panes rather than one camera
because *those* two runs finish **335 m apart at their widest** — a chase camera
would lose the second car within seconds.

| run | time | vs author time | vs deeperjungle's 21.105 |
|---|---|---|---|
| **AshenBeast1985 — the board's record since 2026-08-21** | **9.075** | **−9.731** | −12.030 |
| **TAS, the filmed one** | **18.160** | **−0.646** | **−2.945** |
| TAS, previous best | 18.942 | +0.136 | −2.163 |
| TAS, filmed earlier | 19.427 | +0.621 | −1.678 |
| TAS, watchable earlier | 20.296 | +1.490 | −0.809 |
| TAS, earlier still | 20.942 | +2.136 | −0.163 |
| Author time | 18.806 | — | −2.299 |
| deeperjungle — the record until 2026-08-21 | 21.105 | +2.299 | — |
| lqpzz | 23.689 | +4.883 | +2.584 |
| Herrlille | 25.681 | +6.875 | +4.576 |

The third column is kept against deeperjungle's 21.105 rather than re-based on
the new record, because every one of those margins was measured against that
run and re-basing them would be arithmetic dressed as measurement. Against the
board's record today, subtract 9.075.

The 18.942 held this page's headline until 18.160 validated and reconstructed;
its clip has been withdrawn so the page shows one best.

> *Retracted, kept in place:* this paragraph used to say **"18.160 is the
> fastest time anyone has recorded on this map"**. It is not, and has not been
> since 2026-08-21 20:44:52 UTC — AshenBeast1985's 9.075 is. What is still true
> is the clause after it: 18.160 is the first run here that reaches the
> author's own launch mechanism rather than merely getting near it, and it is
> the first to take the author medal.

### What was checked before it was filmed

| check | reading |
|---|---|
| gate (`tmtrajcheck --race 18160`) | PUBLISHABLE — 0 failures, 1 warning (C10 geometry) |
| custom car skin | clean — `Skins\Models\CarSport\TAS.zip`, nothing else |
| spawn vs deeperjungle's, as a rotation | **0.001 m**, \|dot\| **1.0000** |
| MediaTracker import name | `Ghost:TAS`, one track, one entity block, end 18.15 |
| donor strings (nickname, GUID, zone, storage URL) | zero occurrences |
| contamination vs the human recording | INDEPENDENT — longest near-identical run 53 samples, under the 100 bar |
| input tapes ours vs theirs | different md5s — two runs, not one lap twice |

## The rotor wall is the launcher — and the map is a gate CHOICE

The map holds **1,173 spinning `ObstacleRotor24mWing90X2Level2`** in two rows of
574 — one at y 197 / z 704, the other at y 207 / z 687, spanning x 919–1207 — plus
38 pushers. An earlier reading of this map called eight "launcher bays" the
mechanism; they are scenery. **Removing the rotor rows removes the launch**, which
is how the real answer was found.

**Every finisher needs the LOW row.** Delete it — 282 movers, with the origin
control passing — and every tape on this map DNFs, the weak-launch ones and the
author-launcher ones alike. It is not an obstacle to be got past; it is the first
link in the chain. What the fast tapes changed is not bypassing it but arriving at
the *upper* row in a state that row accepts, after the low row has done its part.

**And the map has fifteen finish gates, so the run is a choice of which one to
reach.** That is where the time is, and two of our tapes prove it:

| tape | launch speed | gate crossed | time |
|---|---|---|---|
| 19.427 | **779 km/h** | #1031 at (1008, 402, 1360) | 19.427 |
| **18.942** | **696 km/h** | **#1033 at (1104, 394, 1232)** | **18.942** |
| the author | 884 km/h | #1026 at (1008, 474, 1136) | 18.806 |
| 18.160 | not measured | not measured | 18.160 |

**The faster of the two launches at 83 km/h LESS.** It reaches a nearer gate and
wins by 0.485. So peak launch speed is the wrong objective — **where the launch
puts you is the objective**, and an earlier note on this page saying the residue is
"launch quality" was at best incomplete. Which gate the 18.160 takes has not been
read off the tape, so it is not in the argument above; it is filmed and
validated, not characterised.

**The interesting part is a gate nobody has reached.** Geometry says **#1027 at
(1168, 458, 1072)** is the cheapest of the fifteen — 63 m closer to the launcher
row than the author own #1026 — with a predicted run of about **18.45**. Nothing
has ever crossed it. #1024 and #1032 are unvisited too. If that prediction holds,
the remaining margin is sitting in a gate no tape has found, which is a far more
tractable target than out-launching the author.

Three things that are measured and closed:

- **The start cannot be shifted by one tick.** A real shift operator was applied at
  k = 0 (control, passes) and k = 1…260 — every one dies. The author's 1.90 s idle
  is not a copyable knob.
- **Lateral aim is not the blocker.** In-flight steering is worth 0.55 m over 4.3 s,
  and a car placed 0.1 m from a bay centre still crashes.
- **The finish set is measure-zero.** 20,815 exhaustive one-move edits from a
  working tape produced **one** finisher.

## Two corrections this map forced on us

**A bound we published here was wrong, and the reason generalises.** An earlier
analysis concluded, from 253 aimed arrivals, that "the approach's reachable set at
the wall is a one-dimensional curve and the author's state is not on it". The
19.427 is a three-operation candidate — gas cut, brake pulse, late steer — that
the analysis's own parameterisation could not express. The measurement was fine;
the inference was scoped to the wrong object.

> **An exhaustive search measures its parameterisation. Exhaustiveness is a
> property of the grid, never of the map.**

With numbers, from this map: every finisher here bar one came from the random
hunt, while the focused enumeration around this solution returned 48 finishers and
**not one under 19.427**. They explore different objects. Run both.

**And a 1.000 s rotor period published mid-analysis is withdrawn** — binning the
data refutes it.

## Why this page took an extra day: the file, not the run

The 19.427 was a validated *time* for hours before it was a renderable *ghost*,
and both problems are worth recording.

**Its regeneration would not certify** — the telemetry sat 7.81 m from the tape's
own route. The cause was a **clock offset**, not a bad locate: the position error
tracked the regeneration window monotonically, so the right car was being found
every time and attributed to the wrong tick. Fixing it took **24 independent
attempts ranked against ground truth**, and the distribution is the lesson —
**two** landed on the true clock (byte-identical to each other, 2.6 mm), **five
agreed with each other on a wrong answer** 7.81 m out, and eleven were 900–1400 m
away. Any procedure of the form "regenerate a few times and take what agrees"
would have shipped a file a kilometre off its own route with a majority behind it.

**And the file was in somebody else's container.** It was built by bit-patching
Herrlille's recording, and it announced itself to the game as `Ghost:Herrlille`
until it was repaired — while passing a byte census of its declared time
cleanly, because the patcher had fixed the time and nothing else.

There is a pleasing detail in that. **Herrlille was rank 3 on this board when
this was written.** Without checking the live leaderboard before writing this
page, we would have compared our run to a record that had already fallen, using
a container borrowed from the man who lost it.

**And the same thing has now happened to this page.** The board gained a 9.075
on 2026-08-21 and nothing here noticed for three days — see the banner at the
top. The lesson does not stop applying to the page that wrote it down.
