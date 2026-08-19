# Pain ft Mango & Teuflum — the author time falls by 7 ms, and one named player is one sector away

| | time | vs AT | vs human WR | device |
|---|---|---|---|---|
| **TAS** | **49.275** | **−0.007** | −0.171 | analog |
| TAS, earlier | 49.278 | −0.004 | −0.168 | analog |
| **keyboard** | 49.475 | +0.193 | −0.016 | **3 values** |
| Author time (never beaten by a human) | 49.282 | — | −0.164 | — |
| Human WR | 49.446 | +0.164 | — | — |
| **Ssnake01, rank 2 — pure keyboard** | 49.491 | +0.209 | +0.045 | keyboard, 58 presses |

TMX map [285268](https://trackmania.exchange/maps/285268) · **160 recorded
runs** · 10 sectors, 49 seconds.

**Not submitted to any Nadeo leaderboard, and it never will be.**

---

## The result a human can actually use

Our fastest tape is 7 ms under the author time and **is not a technique** — see
the honest caveat below. The useful finding is about somebody else's run:

> **Ssnake01's rank-2 lap is pure keyboard, owns four of the ten sectors, and is
> 20th of 20 in sector 4. Give that lap a merely *median* sector 4 and it
> finishes in 49.279 — under the author time, on a keyboard, with 58 key
> presses.**

Every other run in the top twenty needs three or four sectors to improve.
**Ssnake01 is the only one who is one sector away**, and the sector they need is
the one they are *last* in. That is a specific, nameable thing one identified
player can go and practise tomorrow.

## What the field is unanimously getting wrong

The map header carries `validated="1"`, so the author's own author-time lap
decodes straight out of the `.Map.Gbx` — 1046 samples, with the telemetry
`steer` column being the raw input. The author is on the field's route (maximum
6.0 m separation, no launcher, never airborne), so this is technique within a
known route.

And then:

> **All twenty of the top twenty — and the author — hold 100.0% full lock for
> the entire 4.7 seconds of sector 9.**

Our whole 171 ms over the world record is 52 ticks of easing **1–8 parts in 127**
off that lock. The single largest move is **400 ms at 97% lock instead of 100%,
worth 124 ms by itself.**

Verdict: **known-but-unanimously-wrong.** Nobody in the field is doing anything
exotic; everybody is pinning the wheel where the car wants a fraction less.

*(One statistical trap, recorded because it points the wrong way: the field's own
lock%-versus-time correlation is **negative** — −0.49 overall, −0.75 for keyboard
runs. That is a confound, not a finding. Slower drivers steer less because they
are correcting more.)*

## Where the author's 164 ms actually is

| sector | author vs field |
|---|---|
| 7 | **−203 ms** |
| 2 | −73 ms |
| 10 | −83 ms |
| 4 | **+96 ms** (author is slower) |
| 9 | +65 ms (author is slower) |

So the author time itself has about **100 ms of slack in it** — it is a good lap,
not a perfect one.

And sector 7 is not won in sector 7. The author is **4.4 km/h slower over the
sector-6 crest** at 28.2 s, and then carries **+6 to +8 km/h continuously from
29.6 s to 37.4 s**. The crest is where the sector is decided.

## Independently reproduced, from a different search window

The whole search was re-run from the same seed with the window opened a further
684 ticks earlier — 1.08 M candidates against the first run's 796 k. It accepted
**the identical four moves in the identical order** (49.322 → 49.290 → 49.278 →
49.277), then reached 49.275 by a **different fifth move**. Two byte-different
tapes, both validating at 49275, with the human controls exact in the batch.

So the basin is not an artefact of one window choice. And the negative from the
same run is worth stating: **opening sector 7 to the search bought nothing at
all.**

## One of the five changes is a stabiliser, not a time-gainer

All 31 non-empty subsets of the five changed blocks were run on the plain
oracle, and two results fall out that no heuristic would have found:

- **Three inputs carry 168 of the 171 ms.** The 400 ms at 97% lock is −124 on
  its own; three others are −25, −25 and −31.
- **One input costs +175 ms by itself — and without it, the big one is fatal.**
  Combine the 97%-lock ease with *any* of the other three and omit this one, and
  **all six such subsets DNF.** It is what keeps the car alive once the long ease
  is in.

A one-move-at-a-time search cannot see that: on its own the move looks like a
175 ms loss. It only survived here because it was accepted early, while the tail
was still soft.

## The honest caveat: our 49.275 is a chaos exploit, not a technique

The decisive input was swept over **3,276 cells** of placement × strength ×
duration on the plain oracle. **There is no tolerance basin.** The neighbours of
the −124 ms cell are +612 ms, +100 ms, +45 ms, and DNF.

For context on the map as a whole: **one tick of ±1/127, anywhere between race
6.5 s and 36.5 s, DNFs the run — 100% of the time.**

So the analog tape is published as a proof that the time exists, and nothing
more. **The drivable deliverables are the keyboard tape, the sector table, and
the Ssnake01 finding.**

## The low-input family, and what the alphabet costs

The keyboard tape was produced by a **constrained search**, never by converting
the analog tape (which is measured not to work on any map tried). Seeded from
Ssnake01's own lap, alphabet never leaving `{−127, 0, +127}`:

- **49.491 → 49.475 in two inputs** — a 20 ms release of the left key at 46.00 s
  worth 11 ms (the keyboard expression of the ease-off-lock finding), and a 30 ms
  brake tap at 48.39 s worth 5 ms.
- Then **53,173 further dense candidates found nothing.**

**So the alphabet costs 193 ms here, and a keyboard search does not close it.**
Duration matters far more than placement on that release: 20 ms is right, 80 ms
costs between +130 and +600 ms everywhere.

## Validation

- **Field reproduction: 163 human ghosts validated in one batch, 158 exact to
  their leaderboard millisecond.** The five failures (ranks 103, 105, 126, 153,
  161) are deep-field runs that used a respawn. **Every run in the top 100 is
  exact.**
- **Three cold validations**, each in a fresh directory with fresh servers,
  against the map re-read from the durable bank, with two human ghosts as
  known-answer controls in every batch: 49.275 / 49.278 / 49.475 stable across
  all three, controls returning 49.446 and 49.491 exactly.
- **Per-batch identity control:** every search batch carried the unlabelled
  incumbent and asserted its exact known time — ~1.6 M candidates over eleven
  stages, zero failures.

## A trap found here that would have produced a convincing wrong sector table

**`tmmaps splits` on a synthesised ghost returns the *template's* checkpoint
times.** The factory copies the declared-splits chunk and never recomputes it, so
a heavily modified tape cheerfully reports its seed's splits. Sector times for
our own tapes must come from telemetry plus an explicit plane crossing.

This is the same disease as the telemetry trap in this repo's
[`FINDINGS.md`](../FINDINGS.md) — a file we synthesised answering confidently
about a run it is not.

## Files

| file | what |
|---|---|
| `replays/HUMAN_rank2_keyboard_49491.Ghost.Gbx` | **Ssnake01's lap — the one that is one sector away** |
| `replays/KEYBOARD_49475.Ghost.Gbx` | that lap plus two key presses |
| `replays/TAS_49275.Ghost.Gbx` | the fastest run (a chaos exploit — see above) |
| `replays/TAS_49275_independent.Ghost.Gbx` | the same time from a different search window — byte-different tape |
| `replays/TAS_49278.Ghost.Gbx` | the first tape under the author time |
| `notes/RESULT.md` | the full write-up: author decode, sector tables, the 3,276-cell sweep |
