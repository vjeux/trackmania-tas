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

## 173636 "tap water 01" — the regeneration re-simulates to the right time and the server still calls the run invalid

Found 2026-08-21. The regenerated tape `F1994618919`
(md5 `52f527fa171d2690a85133d7a171e41f`) was produced to unblock this map. It is
**not** clear to film, and the reason is narrow enough to be worth stating
precisely, because two of its three failures are innocent and one is not.

| check | regen | our published `TAS_22072` | downloaded human WR `rank00001_23638` |
|---|---|---|---|
| C6 ground contact | FAIL 85.4 % | FAIL 85.4 % | **FAIL 85.7 %** |
| oracle, re-simulated time | 22.072 — correct | 22.072 — correct | 23.638 — correct |
| oracle, `IsValid` | **false** | true | true |
| contamination | clean vs **31** human recordings, 0 bit-identical positions | — | — |

**C6 is refusing the map, not the file.** A downloaded human world record fails
it at 85.7 %, and on another map (227969) a human WR passes the same check at
100.0 %. The regen's 85.4 % is the number our own published file has carried
since it was made. C6 here is a named, signed override — not evidence.

**`IsValid false` is the block, and it belongs to this file alone.** Every other
file put through the same server on the same map came back valid, including our
published `TAS_22072` with the same `cps 1`. So: the server re-simulates the
regen to exactly its declared 22.072 s and still declines to call the run valid.
**The time is right and the run is not accepted are different statements**, and
this is the only file in the corpus where they disagree — which is exactly the
shape of defect that cost this project a night: a correct-looking number attached
to something that is not the thing claimed.

`cps 1` matches across all four runs, so it is not a missing checkpoint count.
The open candidates are the finish event and the respawn/state flags. A passing
file exists on the same map from the same pipeline family, so this is a diff, not
a mystery.

## The pattern behind three of the entries above: run the human's file through the gate first

C8 refused the snow car. C3 refused 197047. C6 refuses 173636. **All three were
genuine human recordings, and all three were caught only because somebody thought
to run a human's file through our own gate.** Three is not a coincidence: the
gate has a systematic bias against certain surfaces.

Standing step, not an ad-hoc control: before trusting any refusal on a map,
download that map's human world record and put it through the same gate. **A
check that refuses the human recording is measuring the map.** Where that is the
finding, record it as `GATE_OVERRIDE="<id>=<reason>"` — a signed claim in the log
— and never by widening the rule.

## The defect no positional check can see: our run inside somebody else's file

Found 2026-08-21, twice in ninety minutes, both times by accident. **A tape can be
completely ours in every position it contains and still be somebody else's file.**

A synthesised tape is built on a *carrier* — an existing ghost — and the search
overwrites the carrier's telemetry with our own. 173636's carrier turned out to be
**473 of 473 samples bit-identical to the human world record**: a complete copy of
his run, with the parts we remembered to overwrite replaced. Identity, walltime and
declared time are not three separate bugs. **They are three fields nobody got to.**

### The three known cases, and why no single check catches them all

| file | header declares | validates to | account id | caught by |
|---|---|---|---|---|
| 227969 film-ready | **8.197** — the WR holder's | 8.050 | Titoch_tm's | header + ident |
| 173636 regen | 23.638 — rank 1's | 22.072 | rank 1's | `IsValid`, **by accident** |
| 165922 × 9 | 15.217 — **correct** | 15.217 | wschseng's | **ident only** |

- Declared-vs-validated **misses the nine**: their declared time is right.
- `IsValid` **misses 227969**: the server is lenient when a header claims a
  *slower* time than the run achieves. 173636 was caught only because its header
  happened to claim a *faster* one — a coin flip.
- **Only the account id catches every one.** Read it on every file, no exceptions
  and no overrides. Ours carry none and report Login `TAS`.

### Two rules this cost us

**A check must read both of its operands out of the file or the world — never one
out of the command line.** 227969's manifest verifier compared the header's
declared time against a number passed to it as `--oracle 8050`. The comparison was
correct, the oracle was correct, and it never read the file.

**Declaring an inheritance is not checking it.** The same manifest said, truthfully,
that the header was inherited from the carrier. That line was read as provenance.
It was a finding.

### What this means before filming

A tape whose **container** is not ours is not filmable, however clean its driving.
A file with no manifest gets a manifest requested before it gets filmed — and a
manifest is a *claim*, so verify it against the bytes in the file you hold. Both
times this defect was caught, it was caught by someone re-reading a file that had
already passed a full green gate.

## Nine container fields, two mandatory readers, and one edit that vanishes silently

By the end of 2026-08-21 the count of things a searched tape inherits from its
**carrier** — the existing ghost it was built inside — stood at nine:

| field | the only reader that sees it |
|---|---|
| login → account id (the server derives one from the other) | the game's parser, `/validatepath` |
| **nickname** | **only a MediaTracker import** |
| car skin, and the URL it downloads from | plain text at offset 175 |
| trigram, club tag, zone | fixed in an earlier pass |
| declared time — stored **six times** | a u32 census, which needs no reference at all |
| session walltime | nothing yet; derived rather than copied, still unlocated |
| split vector, chunk `0x0309202B` | `tmmaps splits` |

**Two readers are mandatory and neither subsumes the other.** The proof is a
matched pair: 165922's nine displayed `TAS` while carrying the donor's account
id; a repaired 227969 carried no account id while displaying `Titoch_tm`. Each
reader is blind to exactly the case the other catches.

### The edit that succeeds at doing nothing

Patching the three plain-text strings shortened a file by 142 bytes, and the
server then **dropped it from the batch without a word** — `1 replay parsed`
where two were staged, `Can't load: 0%`, no error naming the file. **A file that
vanishes reads exactly like a file that was never there.** The fix is a u32 at
**offset 77**, the Gbx header-data size, decremented by the bytes removed.

There is a structural reason an earlier login patch needed none of this: **the
login lives in the body, where a length change is free; the nickname and skin
live in the header, where it invalidates the size.** Two identity fields, two
different rules, and nothing warns you which one you are editing.

> **After any container edit, assert that the parser produced a row FOR YOUR FILE
> by name.** Not that the batch succeeded. Not that the count looks plausible.

This is the same defect as a publish check that reads status codes: `curl`
succeeds perfectly at downloading a 404, and the parser succeeds perfectly at
parsing one file instead of two. Both tools did their job; neither did yours.

### And a filter that deleted the signal

The strings had been missed by an earlier scan because it filtered out anything
matching `skins|models|zip` as engine noise. The car skin **is** engine noise, and
it is also a stranger's custom livery with his account uuid in the filename. A
filter tuned to reduce noise will eventually be tuned past the thing you are
looking for.

## The regenerator is nondeterministic at tick alignment, and a wrong file passes a clean gate

Eleven runs of one identical `fk regen` invocation — same binary, same inputs,
same map, each in its own directory — produced **three** different published
files, one tick apart, plus three aborts:

| alignment | runs | relationship |
|---|---|---|
| **A** | predecessor, A, C, and two clean-carrier runs | the mode |
| B | 2 | **A − 1 tick** |
| ct5 | 1 | **A + 1 tick** |
| ABORT | 3 | the tool declining to guess, which is correct behaviour |

They are collinear in time: A↔B is 0.229 m, A↔ct5 0.228 m, B↔ct5 0.457 m, at
~23 m/s — one 10 ms physics tick each. **The spread is symmetric about A and A is
the mode**, which is what a sampling-phase race looks like rather than a bias. It
is evidence for A, not proof.

**The dangerous part: the off-by-one-tick file passes a completely clean gate.**
C2 446.8 m over 442 points, C1–C4 and C9 pass, `IsValid true`, the census clean.
Nothing we own can tell you which of three passing files is the true alignment.

The arm that found it predicted a bad run would show as an md5 mismatch **plus** a
C2 collapse — the decoy-anchor signature. That was half wrong: **md5 alone says
"different", not "wrong".** What caught it was running a third time, after a
green result had already been reported and had to be withdrawn.

> **Two runs agreeing proves nothing. Tick alignment is a property of the RUN,
> not the file** — so a pipeline that produced one good file has not been shown
> to produce good files, and every regenerated file needs its own check.

Three things that follow:

- **Filming does not re-run the regenerator.** A file already in the tree has its
  bytes decided; the lottery only exists for files we make. So this blocks new
  derivations, not publication of what we hold.
- **Any re-derivation must be compared byte-for-byte against the incumbent**,
  never assumed. Our one known fixed point returned byte-identical on only 2 of
  5 runs — the rest were two aborts and one neighbour.
- **Budget one re-attempt in three** for a corpus-wide re-run, and read an abort
  as the tool refusing rather than as a defect in the file.
