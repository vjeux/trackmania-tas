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

## A threshold on a quantised quantity is a threshold on the quantiser

Three checks in one kit changed meaning between decoded-CSV input (~1 cm) and
full-precision f32 input, silently, in both directions:

| check | at ~1 cm | at f32 |
|---|---|---|
| agreement **count** vs a human | hundreds of samples agree with the seed | a clean regenerated file agrees with **nothing** — 0 samples |
| pre-divergence **floor** | reads a floor | reads the true sub-millimetre separation |
| consecutive-run **graft** detector | an honest tape shows **69 consecutive** agreeing samples on the shared part of a lap | the run collapses |

The third one refused a certified known-good file as a graft. **The fix is not a
bigger threshold** — it is that the check now **refuses to run** on quantised
input and prints `not run (needs full-precision .json input)` in its own row.

> **State the precision a check needs, and refuse to run rather than run blind.**

Corollary already paid for elsewhere in these notes: on full-precision input read
the **separation**, not the agreement count. A clean file agrees with nothing at
f32, so a count of zero is meaningless and only the distance still carries the
signal.

### And the shape of the graft test itself

A splice is a **consecutive run** of identical positions — a segment — not a
scattering of coincidences. Two runs on a similar line coincide at isolated
instants long after they have genuinely parted, so an agreement *count* over the
whole file finds grafts in honest work. Threshold: 10 consecutive samples, 0.5 s.

Combined with the prefix rule from the entry above:

- **identical from the start, then diverging** — innocent, that is shared
  deterministic physics with no steering yet
- **identical in the interior** — a splice
- **re-convergent after real separation** — a splice
- **identical to the end** — a whole-file copy

### A trap that documentation does not fix

The author of these notes hit their own documented `$(basename $f)` trap **two
hours after writing it down** — command substitution inside an `echo` sets `$?`
before it is read, so a verification loop reported `rc=0` for three known-bad
files. Documenting a trap does not stop anyone falling into it. **Capturing
`rc=$?` on its own line does.** Prefer the fix that removes the possibility over
the note that warns about it — which is this file's own thesis applied to itself.

## Partially rewritten containers: a file that carries two answers at once

A census over all 173 published ghosts found a class nobody had named. The
declared time lives at **five or six sites**, and eighteen files have only *some*
of them rewritten — so the container holds **both** its own time and a human's,
and **which one the game reads depends on which site it reads.**

```
126859  ALPHABET153_23545 / TAS_23416 / TAS_23462_v1 / THIN_318ev_23508
                                    2x own + 4x 27.609  (TheWoreL, rank 13)
126859  KEYBOARD_24164              2x own + 4x 24.342  (zetos.)
286279  x9 files                    4x own + 1x 441.002 (Bald_tm)
238835  NORETRY_347003 / _407463    4x own + 2x 1964.933 (Quantiks)
228607  AUTHOR_LAP_20258            5x own + 2x 24.902  (Falco_TM_)
227654  TAS_59912_watchable         4x own + 1x 147.031 (ailiei.)
```

**A census that asks only "does it contain a foreign time" calls these foreign; one
that asks only "does it contain its own" calls them clean. Both single questions
are wrong** — ask both and compare the counts.

126859 is the clearest case and it matches that map's telemetry audit exactly: the
donor is **TheWoreL's rank-13 ghost**, chosen originally because "the fastest
approach to the final tower belongs to rank 13". Same donor, two different fields,
one of them half-cleaned.

### Three outright foreign, one of them new

`146612/SEGMENT_cp5_32702_DO_NOT_PUBLISH_declares_40226` carries **5× 40.226 — Mr.Compiler's, rank 2, not the
world record.** Anyone checking against the WR alone would have missed it.
`279218/TAS_5345_starttrick` (already withdrawn) and
`286279/AUTHOR_AT_ghost_from_map_DNF_cps1_DO_NOT_PUBLISH_declares_441002` are the others.

### The two axes are independent, and this is the proof

`227654/TAS_57573` is a **whole-file telemetry copy** of ailiei. — and its
**container is clean**. `126859/TAS_23416` has clean-ish telemetry and a
**half-rewritten container**. So *whose motion is recorded* and *whose file it is*
are orthogonal. **A file can fail either one independently, and neither reader
implies the other.**

### Caveats that keep this table honest

- The "own time" is taken **from the filename**, which is an assumption. Right
  often enough to be useful, wrong sometimes. Confirm against the oracle before
  withdrawing anything.
- It reads the declared time **as a u32 only** — it cannot see the nickname or the
  login, so a clean census is not a clearance.
- The "neither" bucket contains parser artefacts: `._TAS_12759` and two siblings
  are macOS resource forks, not ghosts. The real content there is **165922's
  nine**, which contain no copy of any time at all — consistent with their having
  had no decodable record.

## A seventh time field, invisible to the census: the container's own timeline

199100's cleaned 49.778 arc was repaired on the nickname and on the declared time,
passed the census, and imported as `Ghost:TAS`. **The MediaTracker still reported a
52.2 s block** — which is OrmeEssence44's 52.202, the donor's time, in a **third**
place.

The file declares 49778. Its 998 samples span 49.900. And the container's own
timeline still runs to 52.2.

**The census cannot see this**, because it counts u32 copies of the *declared*
time and this is a different field entirely. Only the game reports it, and only
when the ghost is imported — which means it shares a reader with the nickname and
was found the same way: by watching what the game said about a file that every
other instrument had cleared.

**So the field count is at least ten**, and the seventh time-bearing one arrived
after four hours of looking specifically for time-bearing fields. That is the
argument against every enumeration: the list is what we have tripped over.

### The check that caught it, and why it was not simply widened

The shoot refuses when the imported block length disagrees with the run's own
span. It fired three times here before an explicit `SPAN_OK=1` was added, which
accepts a container timeline longer than the run **and logs the discrepancy with
its reason**. Default behaviour is unchanged, because that same check caught three
wrong-ghost imports earlier the same night.

**An override that records why it was used is a decision. Relaxing the default
would have been a hole.**

## The majority was wrong: 2 of 24 regenerations were right

208024's regeneration would not certify — its telemetry sat 7.81 m from the tape's
own route dump. **The cause was not a bad locate. It was a clock offset.** The
regen's window varied per attempt, and the position error tracked it monotonically:

```
window  −210 ms  ->   7.81 m        window  −690 ms  ->  43.29 m
        −300 ms  ->  17.29 m                +290 ms  ->  33.96 m
```

**The locate was finding the right car every time and attributing it to the wrong
tick.**

The fix was 24 independent attempts ranked by ground truth — each output compared
against the tape's own `fk btraj2` route dump — and the distribution is the
finding:

| outcome | attempts |
|---|---|
| **true clock, 0.0026 m, byte-identical to each other** | **2** |
| clustered at 7.81 m | 5 |
| 900–1400 m out | 11 |

**Two runs in twenty-four were right.** Five agreed with each other on a wrong
answer. Any procedure of the form "regenerate a few times and take what agrees"
would have shipped a file that is metres — or a kilometre — off its own route,
with a majority behind it.

> **Agreement is a diagnostic on the chooser, never an acceptance test.** Only a
> check that can *identify* the answer settles it: here, the tape's own route
> dump; elsewhere, spawn exactness, path length, bit-identity against a known
> file, or the game's own reader.

Third instance of this shape tonight, and by far the worst ratio — the others
were four-of-five agreeing wrongly, and three deltas agreeing at the same wrong
distance. On that map a regeneration needs **~24 attempts and a ranker**, not six
and a vote.

## Repairing a container: two paths, and picking the wrong one destroys the file

**There are two repair paths and the choice is not cosmetic.**

- **header user data present** — the nickname, skin path and storage URL live in
  the header block. Edit each string in place, then adjust the header-size u32 at
  **offset 77** by the total delta.
- **header user data size 0** — everything is in the body (one file had them at
  `body@150 / 242 / 340 / 10374`). Use the body path; no size adjustment.

Running the **header** path on a file whose header user data is empty misreads the
body as a chunk table, grows it 5263 → 10436 bytes and **overwrites the map UID**.
Silent, and fatal.

And in either case, edit strings **in place** — never transplant a clean twin's
whole block. Two containers on different maps had different field layouts (an
extra skin blob, a second skin path), so a wholesale copy removed fields the
target needed and the server **dropped the file from the batch without a word**.

## A repair tool reports its actions, not its coverage

An anonymisation pass was given the zone as `World|World|Sweden` when the file
said `World|Europe|Sweden`. It reported **"2 strings replaced"**, looked entirely
successful, and left the real zone string sitting in the file. Nothing in its
output suggested anything had been missed — **it only replaces what you name.**

> **After any anonymisation pass, grep the output for the donor's strings.** The
> tool tells you what it did; only a scan tells you what remains.

Same shape as a manifest check comparing two numbers from the command line, and as
`curl` succeeding perfectly at downloading a 404: **a tool doing exactly the
subset of the job it was told about, and reporting success.**

## The four combinations, all with real instances

| declared-time census | container identity | example |
|---|---|---|
| foreign | foreign | 199100's two arcs — time *and* nickname both the donor's |
| **clean** | **foreign** | 208024 — a bit-patch fixed the time sites and nothing else |
| partially rewritten | clean | 126859's five — half the time sites, `Ghost:TAS` throughout |
| clean | clean | the 137 published files that passed both |

**Neither reader implies the other, in either direction.** That is now demonstrated
rather than argued, and it is why a clip needs both plus the residual grep.

## A repair that made the file worse, and the control that caught it

Five files were repaired for a one-tick timing error. **Three were improved. Two
were pulled onto a human's trajectory.**

| file | published | after the repair |
|---|---|---|
| `285268/TAS_49278` vs rank001 | 1 of 986 | **410 of 986** |
| `285268/HUMAN_rank2_keyboard_49491` vs rank002 | 1 of 990 | **514 of 990** |
| 270053 ×3 vs all five humans | 0–1 of 90 | **0 of 90** ✓ |

**The control is what makes this a finding rather than an alarm.** Every
unrepaired file on that map sits at 0 or 1 identical positions against either
human — and **two genuine, unrelated human recordings score 2 of 989 against each
other.** So 0–2 is the map's entire natural range, and the repairs land two orders
of magnitude outside every control, including the human-versus-human one.

**It is not the tail-overlap artefact** (unchanged with `--race`), and **it is not
a splice**: the 410 identical samples arrive in **226 runs averaging under two
samples**, scattered from 0.000 s to the finish and interleaved with
non-identical samples throughout. A splice is long contiguous blocks. At 41 % of
instants our car is exactly where the human was, and in between it is up to
15.7 m away — the two records are being *mixed*, not copied.

Best available reading: the tick shift changed which entity or memory copy the
regeneration read, so it picks up an adjacent ghost part of the time. Unproven,
and nobody guessed further. It happens on **both** files toward **different**
humans, which argues it pulls in whatever ghost is nearby rather than one donor.

### The one that would have been waved through

`HUMAN_rank2_keyboard_49491` is published **as** that human's run. So 514 of 990
identical positions reads as *more faithful* — and it is on the allowlist for
exactly that reason. **The easiest of the five to approve was the one whose number
should worry us most**, because the same mechanism produced both.

> **Lag-versus-control is necessary and not sufficient.** A repair can be right
> about the lag and wrong about the file, so the contamination check runs on every
> repaired file before it lands.

## The baseline: zero unexplained foreign containers

After three renames and four repairs, a census over the whole published tree:

```
foreign containers — UNEXPLAINED:    0
foreign containers — KNOWN & NAMED:  2   146612 SEGMENT_cp5_…_declares_40226
                                         279218 TAS_5345_…_TELEMETRY_IS_Matik_K_…
carries its own time, no human's:  137
mixed (partially rewritten):        18   identity axis clean wherever read
neither:                            15   165922's nine record-less files, three
                                         macOS resource forks, three name-parse misses
```

**A census with no unexplained exceptions is a fact. One with exceptions is a
snapshot.** Re-run it whenever the tree changes — it costs about a second per file
and it is the only check that needs no reference and no server.

## The last identity field, and the only one the audience can see

Two files passed every reader we had — no account id, login `TAS`, `Ghost:TAS` on
the game's own import — and carried the donor's **custom car skin**:

```
Skins\Models\CarSport\frckitbot (1)(1)_756eeda4-….zip
https://core.trackmania.nadeo.live/storageObjects/756eeda4-…
```

Every other inherited field is metadata. **A custom skin is the paint on the car
in the video.** Those files would have gone on screen wearing a stranger's livery
in a clip captioned as ours, and no reader was looking for it — it was found by a
tool that scans skin paths on its way to account ids.

> **No clip ships from a file carrying a custom skin path.** The correct value is
> `Skins\Models\CarSport\TAS.zip` with the checksum zeroed.

Four readers, four fields, **none subsumes another**: the game's import for the
nickname, the server's parser for login and account id, the u32 census for
declared-time sites, a skin-path read for the livery. Each has caught something
the other three passed.

## A search that could not have found the thing it reported absent

Every "0 wheel-block candidates" in this project's record was produced by a finder
that searches **stride 44 only**. There are **two** blocks: family A at stride 44,
and family B at **stride 184**, and they are **3,844 bytes apart** — so a window
sized for one excludes the other.

So none of those zeros is evidence of absence. Not because they are wrong — A and
B coexist on all three maps measured — but because **the search could not have
found B**, on top of a false-negative rate already demonstrated on A. The table of
four maps built on those zeros is **retired rather than corrected**.

### How B was settled, and why the control is the whole argument

At its own stride, B's four wheels score 86.09 / 88.34 / 83.64 / 85.28 % with
phases identical to A's, same map, same gather — **the same physical quantity**.
Then the discriminating test:

| comparison | identical slot-instants | worst numeric difference |
|---|---|---|
| **A vs B** | 5.76 % | **0.000610 rad** |
| **control: A wheel *i* vs A wheel *i+1*** | 8.37 % | **1527.8** |

**"Not bit-identical" alone would have been weak.** With the control sitting six
orders of magnitude away, the instrument is *shown to resolve what it is being
asked to resolve* — so 0.000610 rad means "the same quantity, separately stored",
not "close enough to be a copy". Damper at B−4 lands *below* each column's own
majority baseline, so B's interior is not A's.

Most likely a render- or interpolation-side copy, which fits both the tiny
disagreement and the wider stride.

### Two things it does not license

**B is not preferable to A for reading fields** — A's interior is confirmed, B's
is unknown, and B's wheel value is marginally *less* faithful.

**And the shared 0.3636 m radius told us nothing.** The open note said in advance
that a matching radius was expected under *both* hypotheses, and it duly failed to
discriminate. **The bit-comparison did.** Writing down what a measurement cannot
settle, before running it, is what stopped that number being read as evidence.

> **"We have not tested it" and "we tested it and found nothing" are different
> claims.** Half of tonight's false negatives came from a tool reporting the
> second while only entitled to the first.

## An instrument that was gone, reporting

A corpus sweep returned `NONE` for 15 of 31 files — including all eleven on a map
whose files had been read as `Ghost:TAS` forty-five minutes earlier and filmed
from. The game had crashed. **A dead game answers every import with no track, and
`NONE` looks exactly like a finding.**

Fixed rather than noted. The reader now refuses to start unless the game answers
`/ping`, and if a file comes back nameless it **re-checks liveness and aborts
instead of writing the row**:

```
nickcheck: the game is not answering /ping -- refusing to score files against a dead instrument
nickcheck: the game died during <file> -- aborting rather than recording NONE
```

**The general form, and it is the last of the night's dozen:** every failed check
tonight reported on its own state while appearing to report on the world. This one
reported on its own *existence*. A check that cannot tell "the answer is no" from
"I am not running" will fill a table with confident absences.

> **An instrument must be able to say it is not there.** Liveness is a
> precondition of a verdict, not a footnote to one.

## `238835/NORETRY_347003_watchable` imports as `Ghost:vjeux`

Not a stranger's name — it is the project owner's own account. So it is not the
container defect this file's neighbours had, and nothing about the run is in
question.

But it is not `TAS` either, and a clip from it would put a personal identity on
screen where every other clip puts the project's. **Left as-is and recorded here
rather than repaired**, because unlike a donor's login this one may be entirely
intended, and a repair would be us deciding that for him.

## An acceptance test one sector deep is not a composability proof

210218, sector 11. A perturbation was accepted because the tape still reached the
*next* milestone with the following sector's inputs frozen — the h = 1 contract.
It recovered **990 ms** at CP12 and was banked as progress.

Measured properly, against the same tail inputs and the same repair tool with one
free variable:

| tape | CP13+300 rung |
|---|---|
| incumbent 95.575 | 78.113 |
| chain **without** the sector-11 edit | 78.008 |
| chain **with** it, plus a 45-minute dense repair | 80.768 |

**The 990 ms bought at CP12 costs 2.760 s by CP13 — net −1.770 s.** Forty-five
minutes of repair moved it 56 ms in the last 21; the curve is flat and will not
close the gap.

Nothing is wrong with h = 1 as a *search objective* — it still dominates h = 0 and
h = ∞. The error is reading its acceptance as a property of the route:

> **Surviving one rung says nothing about the rung after it.** An h = 1 acceptance
> is a scoring rule, not a promise that the gain composes. On this map the two
> differ by 2.8 s at one sector's remove.

The corollary is what to search next: if a 990 ms perturbation bills 2.8 s a sector
later, the tapes worth converting are the ones whose perturbation is *small*.

## A relocated gate the goal is standing in for

Same arm, same night — the repo's own rule (*a relocated gate is a fine ruler and
an unsafe objective*) walked into from a new direction.

The dense-milestone spec relocated CP14–16 into a scoring sector **and CP13 along
with them** — the checkpoint the sector's goal was standing in for. The map's route
then no longer required the real CP13 gate. The winning candidate fires the dense
goal at 80.768 while being **DNF cps 12 on the real map**: it passes through the
goal's trigger volume near CP13 without satisfying CP13's own directional gate.

The number survives only in its weaker form — an upper bound against a *relaxed*
objective — and the true statement is the blunt one: after 636 000 evaluations
with a dense ladder, the tape still cannot complete sector 13 on the real map at
all.

> **A goal may relocate the gates it measures you past, never the gate it is
> standing in for.** If the goal replaces a checkpoint, satisfying the goal stops
> implying the route.

Fixed in the tool rather than in a note: `pwdense` now refuses a spec that
relocates the checkpoint its goal substitutes for, so the next arm cannot make
this mistake by hand.

## Absence of a result is not a result — the retraction of "five files the game refuses"

Claimed to the fleet: **five files across three maps that the game refuses to
import while our own parser reads them fine.** It was one file. The other four
were the reading instrument.

The experiment that settled it, one fresh session per subject, with a positive
control ahead of the subject and a *sibling mirror control* behind it — the
mirror being the part that could have proved the alternative:

```
227654  CONTROL 270053/50_tas_4492_v1_v2  Ghost:TAS  4.45   session healthy
        SUBJECT 03_TAS_57498              Ghost:TAS  147.03  <- had reported NONE
        SIBLING 02_TAS_57493              Ghost:TAS  147.03

238835  CONTROL 270053/50_tas_4492_v1_v2  Ghost:TAS  4.45
        SUBJECT 08_TAS_347003_noretry_v4  Ghost:TAS  347.0   <- had reported NONE
        SIBLING 50_TAS_239133_cut         Ghost:TAS  239.1   <- had reported NONE
```

Four for four, correct durations. The standing hypothesis — that the game
deduplicates near-identical siblings, so the *second* copy imports as a no-op —
died on the same rows: the sibling imported cleanly straight after the subject,
and on 238835 both alleged-bad files imported back to back.

**The evidence had been sitting in the original table.** The failures came in
consecutive pairs — 03 and 04 on 227654, then 05–09 all fine; the last two on
238835. A per-file property does not recover after two rows, and a dedupe does
not un-dedupe. That shape is a transient stall, and the reader recorded a single
missed attempt as a verdict.

This is the same disease as the dead game two sections up, one layer in: that
tool learned to ask whether it existed, and still could not tell a *no* from a
*not yet*.

> **Absence of a result is not a result.** Distinguish *the answer is no* from
> *I did not get an answer*, or the table fills up with the second wearing the
> first's clothes.

Fixed in the tool: `nickcheck.sh` retries a nameless import three times, logging
each miss, liveness checked between attempts. A genuine refusal now reads
`NONE-after-3`; a transient no longer reaches the table.

**What survives: one file.** 228607's `01_AUTHOR_LAP_20258_watchable`, refused
for the malformed skin path documented above — an independent explanation from an
independent reader, and a sweep of all 174 published files found no second
specimen.

## Two repairs, no file gets both — the record span and the declared time are separate operations

A **sixth** container field, found from the 227654 rows: the **record span**, the
container's statement of how long the recording runs (`tmtraj decode` prints it
as `start 0 end N`). It is independent of the declared time and of the six
declared-time sites.

On 227654 all eight of our tapes **declare their own time correctly at every
site and still carry ailiei.'s 147.030 span.** Both readings are true; they are
different fields.

**Population: 45 of 165 published files, across 12 maps.** Worst ratios:

| map | file | declared | span | ratio |
|---|---|---|---|---|
| 279218 | `best_pC_5348_32098` | 5.348 | **566.080** | **106×** |
| 238835 | `TAS_262907` | 262.907 | 1964.930 | 7.5× |
| 284238 | `TAS_97325` | 97.325 | 440.230 | 4.5× |
| 227654 | ×8 | ~57.5 | 147.030 | 2.6× |

### The shape of the defect, from one run under two treatments

238835 holds 347.003 twice, and the two copies are wrong in **opposite** ways:

| | `TAS_347003_noretry_v4` | `NORETRY_347003_watchable` |
|---|---|---|
| span end | **1964930** (Quantiks') | **346970** ✅ own |
| declared-time sites | **5× own, 0 foreign** ✅ | 4× own + **2× 1964933** ❌ |
| oracle | 347003 | 347003 |

And the before/after pair that was supposed to demonstrate the fix instead
demonstrated its absence: `50_TAS_239133_cut` halves the samples of its uncut
sibling (9114 → 4783) and **leaves the span at Quantiks' 1964.930**.

> **Cutting samples and fixing the span are two different operations, and no
> file in the corpus has had both.** A repair pass must do both and then re-read
> both fields — fixing one and reporting success is how the corpus got into this
> state.

### What was NOT claimed, and why the restraint mattered

The tempting inference — *the span is the clip's length, so a 566-second
container films 9½ minutes for a 5-second run* — was **withdrawn before it was
written down**. What is actually measured is span ↔ which treatment was applied.
Span ↔ rendered duration is untested.

Measured instead, on the artefacts the audience actually has: **all 30 live clips
fetched logged-out, all playable, none padded** — longest 101.800 (197047) and
96.300 (210218), both genuinely long maps; 279218's clip is 5.400 s. That bounds
the damage to zero *today* without answering the question, because that clip came
from `KEYBOARD_5352_11events`, whose span is correct.

> **A bound on the damage is not a test of the mechanism.** Thirty clean
> measurements said nothing about the defect, because none of the thirty carried
> it.

The pair that would settle it is on 279218, which holds both classes in one
folder: `KEYBOARD_5350_equals_AT` (span 566.080) against
`KEYBOARD_5352_11events` (span correct, **and the file behind the live clip, so
the control's expected value is already measured on the published artefact**).
Measure duration, **frame count**, and **wall-clock render time** — a renderer
that truncates output to the declared time while walking the whole container
shows the defect in minutes spent rather than in the file it writes.

### A third and fourth field, still unreconciled

- **Block-end is not a proxy for span**: the game's imported block end agrees
  with the span on 227654 and disagrees on 238835. Unresolved; do not substitute
  one for the other.
- `TAS_239133`'s **checkpoint list ends at 462982** — 238835's *author* time,
  neither its own nor the donor's. Three fields, three answers, in a file that
  validates perfectly on the oracle. No reader we own reports the checkpoint
  list.

### The span reaches the render — and it arrives as a second track

The matched pair on 279218, run control / subject / control so a drifting
renderer could not be mistaken for a file property:

```
CONTROL-1  KEYBOARD_5352_11events   5.400 s  162 frames  94 s wall  ok
SUBJECT    KEYBOARD_5350_equals_AT  refused at the gate   47 s wall
CONTROL-2  KEYBOARD_5352_11events   5.400 s  162 frames  92 s wall  ok
```

The two controls agree exactly, so the set is valid.

**The donor's span does not stretch the car's block. It materialises as a whole
extra MediaTracker track:**

```
clip=Trigger 1  tracks=2
  track[0]  Ghost:TAS            block end =   5.35
  track[1]  Ghost:SceneryEvents  block end = 566.08
```

The ghost is correct at 5.35 s. The game builds a `SceneryEvents` entity
spanning the full 566.080 beside it, and **a MediaTracker clip runs to its
longest block** — so the render is 566 seconds of which 5.35 contains a car.

> **The 45-file list is a render blocklist, not container hygiene.** Anything on
> it either produces a 566-second video or costs a 566-second render.

**Why no reader we own saw it coming:** `inputcount --meta` reports 5400 for both
files, because it reads the *sample* span. The 566.080 does not exist until the
game builds the clip. The cheap detector is therefore not a byte read at all —
**import the ghost and count the tracks.** One track clean, two tracks foreign.

Two detectors, and they must stay two columns: the `tmtraj decode` span read sees
the declared field; the track count sees what the renderer will do. A file whose
span is foreign but which imports with one track is safe to film and dirty in the
container. **That disagreement is the blocklist's boundary, and no byte reader can
draw it.**

The publish gate refused the subject in 47 s with no file written —
`wrong ghost: clip end=566.08s, expected span 5.40s`. That check was written for
mis-imports and turns out to be a span detector as well: luck, not design, and
worth saying so rather than claiming coverage we did not plan.

**Still unmeasured, and not claimed:** whether a forced render produces 566 s of
video or 5.4 s after a very long render. The gate refused before filming. Either
answer leaves the blocklist standing; only its cost changes.

**And it retires a hedge honestly.** Thirty published clips measured clean bounded
the damage to zero — but the boundary held by luck, not process: every published
clip came from a file whose span happens to be correct, 279218's from the sibling
rather than the subject. **A bound that holds by accident is not a control.**

## A backlog that cannot be counted, and why refusing the number was right

Asked for the size of the declared-time backlog after `setdecl`'s short-circuit
was fixed. The honest answer came back as a refusal, and the reasoning is the
finding.

**The short-circuit itself:** `setdecl` read the current time from chunk
`0x03092005` alone, and if that one site already held the target it copied the
file unchanged and printed

```
already declares 347003 ms; nothing to do
```

— on a file still wrong at two sites in chunks it never opened. Its own header
comment predicted exactly this failure and the code reintroduced it. **That is
very likely how the two repaired 238835 files came to be published.** Fixed: the
tool now surveys and refuses rather than reporting a no-op, and `--from <donor>`
reproduces the banked repairs byte for byte.

**Three attempts to make it decide automatically, three ways to be wrong:**

1. Rewrite every plausible-time u32 at the known sites — *"23 sites rewritten"*.
   `0x0309202B` is the **checkpoint-splits** chunk; this would have flattened the
   whole split list onto the finish time. Caught only by a regression on a file
   whose correct answer was already known.
2. Exclude values that appear in the file's own split list. Cleaner — and it
   excluded 238835's **real** defect, because the donor's 1964.933 appears in the
   split list too. The splits were inherited as well.

> **Presence in the file's own split list does not prove a value is legitimate.**
> A donor's number can be consistent with itself throughout the file it came from.

The offsets are not universal either: `0x0309202B+56` is the finish on a
5-checkpoint map and a genuine **intermediate split** on a longer one — 228607's
`TAS_19907` has splits `[4687, 8196, 9542, 11781, 15008, 18434, 20034]` and +56
holds 15008.

**Three guards produced three corpus counts — 67, then 39, then 12 — each with
demonstrable errors in a different direction.** So no number was given:

> **A count nobody can defend is worse than no count**, because it becomes a work
> queue and then a completion claim. What can be defended is a survey: **51 files
> across 11 maps carry a non-target value at a declared-time offset** — an upper
> bound requiring per-file judgement against a healthy sibling on the same map.

All three counts are banked beside the survey so the instability is visible
rather than hidden.

### 238835 is now clean in every field we can read

All three published files repaired and verified by readers that did not make the
edits, plus an independent raw u32 scan:

| file | before | after |
|---|---|---|
| `NORETRY_347003_watchable` | 4 own + 2× 1964933 | 6 own, 0 foreign, oracle 347003 |
| `NORETRY_407463_watchable` | 4 own + 2× 1964933 | 6 own, 0 foreign, oracle 407463 |
| `AUTHORCUT_246602_watchable` | 4 own + 1× 462982, **two** donor skins, GUID + storage URL | 5 own, 0 foreign, both paths `CarSport\TAS.zip`, no GUID, no URL, oracle 246602 |

The skin strip was checked with the **malformed-path detector built for 228607** —
a strip is a rewrite, and the acceptance test for the repair is the tool that
catches the failure mode the repair could introduce.

**Field #7 remains unread**: a donor checkpoint split survives inside the two
NORETRY files. The page says so rather than implying they are clean in every
field.

## Field #7: the checkpoint split list — a reader with a stated limit, and no count

The seventh inheritable field now has a reader (`splits.rs`, read-only), and the
first thing it established is that the field is **not shaped the way we assumed**:
the entries sit **8 bytes apart** — 15153, 15161, 15169 — a time plus one more
field, not a count-prefixed u32 array. The first version, keyed to an offset,
found nothing at all on a file whose splits were already known.

Verified against three files whose answers came from an independent decoder:

```
228607 TAS_19907            7 splits  4.687 8.196 9.542 11.781 15.008 18.434 20.034  ✓
285268 TAS_49275           10 splits  6.522 … 49.275                                 ✓
238835 NORETRY_347003       4 splits  58.906 148.023 347.003 1704.277                ✓
```

**The third control is the point of the exercise.** That file was certified clean
on all six declared-time sites two hours earlier — and its split list ends at
**1704.277**, a donor split no tool we own touches.

> **A file certified in one field is not certified.** Each reader is clean only
> about what it reads, and the certificate says so or it lies by omission.

It also reproduces the known defect: `TAS_239133`'s list ends at **462.982**, the
map's *author* time, in a file the oracle validates at 239.133.

### Why no count was given — again

The sweep flags 27 of 174, and several flags are visibly the locator rather than
the file: 126859's ~23-second runs reported with a "finish split" of **3146.112**;
another cluster at 7.168; 199100's at 2138.325. Those are a run-finder locking
onto a coincidental increasing sequence elsewhere in the body. It had already been
tightened twice — a plausibility cap after it found a 50929.676 "split", and
last-run-not-longest after a junk run outscored the real list — and both times the
controls caught it. **A third tightening would be a third guess.**

Credible flags are the ones whose finish split is in scale with the map: 238835's
five, 228607's `AUTHOR_LAP` (24.902 — Falco_TM_'s time, in the file already broken
three other ways), and the two repaired NORETRY files. The out-of-scale rows are
the instrument.

The real fix is to anchor on the chunk id (`0x0309202B`) by walking the chunk
table, rather than heuristically locating the run — a bigger job needing the game
to test against.

> **Hand over a reader with a stated limit rather than a number that becomes a
> work queue.** Same conclusion as the declared-time backlog, reached
> independently, one hour apart, by the same reasoning.

### Two facts that survive whatever the locator does

1. **The split list is inherited as a unit** — 238835's donor split survives in
   files whose declared time was fully repaired. The two fields are written by
   different tools and neither knows about the other.
2. **A donor's split list agrees with the donor's declared time.** That is why the
   "exclude values present in the split list" guard failed: *agreement across
   fields is not evidence of legitimacy when both fields came from the same file.*

### The detector is the ENTITY count, not the node count — and the corpus splits three ways

The obvious byte-level reading of the span defect was a node count: one
`CPlugEntRecordData` node clean, two foreign, no game required. **On the matched
pair that produced the finding, it fails.**

```
KEYBOARD_5350_equals_AT (foreign span)  1 node, 0..566080 ms, 10 entities, 4 car entities
KEYBOARD_5352_11events  (clean)         1 node, 0..  5350 ms,  3 entities, 1 car entity
```

**Both are one node.** The dirty file carries **four copies of the car-entity
group** where the clean one has one: the carrier's other entities rode along. So
the byte-level detector is a count of class `0x0a018000` — and it still needs no
game.

> **A detector proposed from a mechanism is a hypothesis about that mechanism.**
> The node count was derived from "a second track appears", which was true, and
> the second track does not come from a second node.

Across all 174 published files:

| | count | meaning |
|---|---|---|
| **foreign span AND >1 car entity** | **19** | the carrier's entities rode along — the render-visible class |
| foreign span, ONE car entity | **94** | span field inherited, no extra entities |
| span ok, >1 car entity | 2 | entities rode along, the span was fixed |
| both clean | 59 |  |

The 19 cluster hard: 227654 ×9 (twenty-seven car entities each), 279218 ×5,
126859 ×5.

**The 94 are the honest open question.** The `SceneryEvents` track was measured on
a file from the 19. Whether a foreign span with a *single* entity group also
produces a second track is untested, and the two columns disagree on all 94.
**Only an import can say which side of the line they fall** — which is exactly why
the columns stay two.

A fourth class, 165922's nine files: span 8790760 (a 2.44-hour donor) and **zero
car entities**, consistent with their decoding to nothing at all. There is no run
in them to have a span.

### The repair is not `tail fix --cut` alone

208024's four files are span-correct by construction — 18900 against the
carrier's 25650, each ending within 50 ms of its own finish — and the obvious
inference was that `tmtraj tail fix --cut MS` is the missing step the 45 never
got.

**The one before/after pair in the corpus says otherwise.** `50_TAS_239133_cut`
has half the samples of its uncut sibling (4783 vs 9114) and **the same span,
1964930**. Cutting samples off an inherited record leaves the inherited span;
208024's files went through `tail fix` *and* a regeneration that rebuilt the
record from our own run.

> **Test a repair recipe on one file before committing it to forty-five.** The
> mechanism that produced a clean result is not necessarily the step you can name
> in it.

### The forced render: both answers were true at once

The question left open was whether a foreign-span file produces 566 seconds of
video or a short file after a very long render. **Both.**

```
control (clean span)   5.4 s of video     5.2 MB    94 s wall
subject (566 s span)   real frames        156 MB    1139 s wall and counting
```

Thirty times the control's size, growing at ~0.96 MB per second of video — it
writes real frames for the entire container. At ~6.5× real time, that projects to
**about an hour of rendering for a 5.35-second run**.

> **The 19 are a hard blocklist.** A file from that group is a 566-second video
> *and* an hour of render time. One of them in an unattended batch eats a night.

**Still open, and it is the whole size of the problem:** the 94 files with a
foreign span but a *single* car-entity group. The measurement above was made on a
file from the 19. Whether the 94 also produce a second track is one import away,
and it is the difference between a 19-file exception list and a 113-file
corpus-wide defect.

### Track count and node count are different quantities

Withdrawn by its own author: the phrasing "the span materialises as a second
`SceneryEvents` track". What was observed is a clip with **two tracks** —
`Ghost:TAS` at 5.35 and `Ghost:SceneryEvents` at 566.08 — and the byte reading
shows that comes from **one node carrying four car-entity groups**.

> The MediaTracker's **track** count predicts the render. The **node** count does
> not. A detector derived from a mechanism inherits that mechanism's imprecision.

### The line all seven fields sit on

Reached independently from the repair side, and it is the general form of the
whole night:

> **Our readers measure the samples; the game reads the container; the two
> disagree by design.**

The span is a statement *in the container*, not a property of the sample stream —
which is why cutting samples leaves it untouched, and why no sample-level
operation repairs the 45.

Every field found tonight is on the container side of that line: the nickname,
the login and account id, the six declared-time sites, the skin path, its length
prefix, the record span, the entity groups, the checkpoint split list. **All of
them are things the file says about itself rather than things the driving did.**
The oracle validates the driving and is silent on every one.

That is why a tape that validates three times is a **time**, and not yet a
**file**.

### The 94 import with one track — the blocklist is 19

The question that decided the size of the problem: does a foreign span with a
*single* car-entity group also produce a second MediaTracker track? **No.**

238835 was the specimen — five files in the 94 with span excesses of 1617–1726
seconds, and three span-correct siblings in the same folder as in-map controls.
Whole folder, control at open and close:

```
01_AUTHORCUT_246602_watchable   1 track   Ghost:Bald_tm@246.57   CONTROL-open
02_NORETRY_347003_watchable     1 track   Ghost:TAS@346.97       span ok
03_NORETRY_407463_watchable     1 track   Ghost:TAS@407.43       span ok
04_TAS_239133                   1 track   Ghost:TAS@462.95       ← the 94
05_TAS_262907                   1 track   Ghost:TAS@262.90       ← the 94
06_TAS_267646_v7                1 track   Ghost:TAS@267.60       ← the 94
07_TAS_268554_v6                1 track   Ghost:TAS@268.55       ← the 94
08_TAS_347003_noretry_v4        1 track   Ghost:TAS@347.00       ← the 94
50_TAS_239133_cut               1 track   Ghost:TAS@239.10       ← the 94
01_AUTHORCUT_246602_watchable   1 track   Ghost:Bald_tm@246.57   CONTROL-close
```

Controls agree at open and close, so the session is valid. **Every block end
matches the file's own declared time**, not the donor's 1964.930 span.

This agrees with the *mechanism*, not merely with the numbers: 279218's second
track came from the carrier's extra entity groups, and a file with one group has
nothing to make a second track out of. **The entity count is not just a proxy for
the track count — it looks like the cause.**

Scope stated by its own author: **five of the 94, one map, one span value.**
286279 and 186935 carry different spans and were not imported. The mechanism
argues they behave the same; that is an argument, not a measurement.

> **Render blocklist: the 19.** The 94 are container hygiene — real, worth
> repairing, invisible to the viewer and harmless to the pipeline.

### A projection offered twenty minutes before the measurement

The interim figure was "~60 minutes of render for a 5.35-second run", derived
from the output file's **byte growth rate**. Measured:

```
control (clean span)   162 frames    5.400 s    ~40 s of rendering
subject (566 s span)   16,983 frames 566.066 s  ~1,140 s of rendering
```

Real video — 16,983 frames counted, not a header read. But the cost is **~28×,
not 90×**: about 19 minutes against 40 seconds.

**Bytes are not frames.** A long clip of mostly-static scenery compresses far
better than five seconds of moving car, so the byte rate understated progress
badly.

> **Do not offer a projection when the measurement is twenty minutes away** —
> and if you must, name the quantity you extrapolated from, because that is where
> the error lives.

### One anomaly for the census

`238835/04_TAS_239133` declares 239133 and imports with a block end of **462.95**
— not its declared time, not the 1964.93 span, roughly double the declaration.
One track, so it is not a render problem. **A third number from a file that
should have two.**

### The block end follows the LAST CHECKPOINT — the 94 was cleared on the wrong axis

`238835/04_TAS_239133` was logged above as an anomaly: declares 239133, imports at
462.95, span 1964.930. It is not an anomaly. It is the discriminating case for the
rule, because its three numbers all differ:

| file | own time | last checkpoint | span | measured block end |
|---|---|---|---|---|
| `TAS_239133` | 239.133 | **462.982** | 1964.930 | **462.95** |
| `TAS_347003_noretry_v4` | 347.003 | 347.003 | 1964.930 | **347.00** |
| `KEYBOARD_5350` (Ghost track) | 5.350 | 5.350 | 566.080 | **5.35** |

**Every block end measured all night equals the last checkpoint.** So field #7 is
not a curiosity beside the span — **it is the field that drives the render.** The
span mattered on the 19 only because extra entity groups produce a *second* track.

> **A class cleared on one axis is not cleared.** The 94 were imported, measured,
> and found harmless — on the span axis. Two of them were the corpus's worst files
> on the axis that turned out to matter.

### Two published files that would have rendered for 32 minutes, fixed by accident

```
NORETRY_347003_watchable   own 347.003   last CP 1964.933   ← Quantiks' time
NORETRY_407463_watchable   own 407.463   last CP 1964.933   ← Quantiks' time
```

Both sit in the 94 — one car entity, span correct — so both were cleared as safe
to film. On the checkpoint axis they were the two worst files in the corpus.

They are already clean, because the `setdecl --from 1964933` repair rewrote
`0x0309202B+56`, which **is** the last split:

```
BEFORE [58906, 148023, 347003, 1704277, 1964933]
AFTER  [58906, 148023, 347003, 1704277,  347003]
```

The repair was aimed at the declared time and happened to fix the render. **Luck,
not process** — the exposure was removed by a commit written for another reason,
before anyone knew it existed.

### The falsifiable prediction

If the rule holds, `146612/KEYBOARD_39706` imports with a block end of **39.555** —
**below its own declared time of 39.706**. No span-based or declared-time-based
theory produces a block end lower than the declaration, so one import decides it.

Others predicted, all one-car-entity so no second-track confound: 126859's
`KEYBOARD_24164` → 24.342 (a human's WR); 228607's `TAS_19907` and seven siblings
→ 20.034; 279209's `ms_r002_6608_best_6585` → 6.585; 286279's
`AUTHORMIN_831ev_354781` → 355.181 (the author time).

**The render-cost class is therefore "last checkpoint ≫ own time", not the 19** —
which is the second-track class, a different set. On the current tree the
render-cost class is `TAS_239133` alone.

### What the render's block end actually is: the last sample's TIMESTAMP

Two rules were proposed tonight and both were wrong, each refuted by a case the
third explains:

```
file                    samples  last sample t   (n-1)*50   lastCP    observed
TAS_239133                9114       462950        455650   462982     462.95
TAS_347003_noretry_v4     6891       347000        344500   347003     347.00
KEYBOARD_24164             484        24150         24150    24342      24.15
KEYBOARD_5352_11events     108         5350          5350     5352       5.35
TAS_19907                  401        20000         20000    20034      20.00
```

**The last sample's timestamp fits all five exactly.**

- The **sample count** rule fails on the edited files: 9114 samples, last one at
  462950, because **a cut removes samples from the middle and leaves the
  timeline**. `(n−1)×50` equals the last timestamp only when there are no gaps.
- The **last checkpoint** rule fails on `KEYBOARD_24164` — 24.342 predicted,
  24.15 observed.

And the two failures are one phenomenon from opposite ends: on an unedited file
the last sample *is* `(n−1)×50` and sits just before the finish checkpoint, so all
three rules agree. **Only edited files separate them, and both go to the
timestamp.**

> **A rule that fits every case you have may be fitted to cases that could not
> have refuted it.** Every clean file confirmed all three hypotheses equally.

This also dissolves a hypothesis that sounded like a bigger finding — that our
parser and the game disagree about sample count. **They do not.** Both read 9114;
the game is not counting at all.

The checkpoint correlation was real and was an artefact: edited tapes were cut
*at* checkpoints, so the last surviving sample lands on one.

What survives from the original observation, and it was the load-bearing part:
**the block end is neither the span nor the declared time** — and now it has a
positive answer instead of a candidate. The render-cost predictor is
`last_sample_ms`, computable for all 174 files with no import.

### The sign error

208024's 18.942 was relayed twice as "0.136 from the author time" and then written
as **(−0.136)** in a draft caption. The author time is **18.806**, confirmed
against TMX: the run is **0.136 SLOWER**, and the map is not beaten.

The board is three runs, all within two days, **the record a day old**:
deeperjungle 21.105 (2026-08-20), lqpzz 23.689, Herrlille 25.681. So the run sits
2.163 inside the human record while still short of the author time — an unusual
shape worth stating rather than smoothing over.

> A margin and a deficit are the same number with a different sign, and the
> caption format puts that sign next to a person's name. **Publish the sign from
> the live board, never from a recollection.**

### Two independent render-cost mechanisms, and the worst file in the corpus

Computing `last_sample_ms` for all 174 files produces an ordering **nothing like**
the span-based list. Ten files carry more than 2 s of excess over their own time:

| map | file | block end | excess |
|---|---|---|---|
| **186935** | **`CUT_795034`** | **2575.150** | **1780 s** |
| 286279 | `AUTHOR_AT_ghost_from_map…DO_NOT_PUBLISH` | 441.000 | 441 s |
| 238835 | `TAS_239133` | 462.950 | 224 s |
| 286279 | `AUTHORCUT_220391` | 441.000 | 221 s |
| 286279 | `AUTHORMIN_831ev_354781` | 441.000 | 86 s |
| 191465 | `WIP_pad5`, `WIP_keyboard` | 13.050 | 13 s |
| 146612 | `SEGMENT_cp5_32702…DO_NOT_PUBLISH` | 40.200 | 8 s |
| 279209 | `kb_gasfull`, `kb20` | 6.600 | 7 s |

**`186935/CUT_795034` is the worst file we have** — a 43-minute block end, roughly
**20 hours of rendering** at the measured ~28×. It has **one car entity**, it is
**not in the 19**, and it is **referenced from a published page's file table**. It
would have passed every check that existed before this rule.

**And the 19 turns out not to be the render-cost class at all.** 279218's
`KEYBOARD_5350` — the file that actually rendered for 19 minutes — has a
**5.350 s** block end on its own `Ghost:TAS` track. Its cost came from the
*second* track, the `SceneryEvents` one at 566.08, produced by the extra entity
groups.

> **Two independent mechanisms, barely overlapping:** extra entity groups give a
> second long track (the 19, measured); a late last sample gives a long ghost
> track (these 10, predicted). **A blocklist needs both columns.**

165922's nine report a block end of 39.700 with **zero car entities** — a
24-second excess on files with no run in them at all. Listed separately; they are
still the pre-repair copies.

### "Settled" withdrawn by its own author

The last-sample rule was reported as *"settled … five files, no import needed"*.
Withdrawn the same hour:

> **Five agreements is a good fit, not a settlement** — and one of the five was
> taken from another arm's sweep rather than measured directly.

Honest status: the rule fits every observation anyone has, and it is the only one
of the three candidates that fits the two edited files. The cheapest falsifier is
`186935/CUT_795034` — if the rule holds, its import shows a 2575-second block end,
which is also the single most valuable thing to know before that map is ever
filmed.

### Confirmed on six files, and one of the night's better stories was false

The block end is **the timestamp of the last sample**. Measured against every
rival column:

| file | own | span | last cp | last sample | measured |
|---|---|---|---|---|---|
| `126859/KEYBOARD_24164` | 24.164 | 24.400 | 24.342 | **24.150** | **24.15** |
| `126859/TAS_23416` | 23.416 | 27.800 | 27.609 | **23.400** | **23.40** |
| `279218/KEYBOARD_5350` | 5.350 | 566.080 | 5.350 | **5.350** | **5.35** |
| `238835/TAS_239133` | 239.133 | 1964.930 | 462.982 | **462.950** | **462.95** |
| `238835/TAS_347003_noretry_v4` | 347.003 | 1964.930 | 347.003 | **347.000** | **347.00** |
| `238835/NORETRY_347003_watchable` | 347.003 | 346.970 | **1964.933** | **346.970** | **346.97** |

**Six for six. Every other column fails at least once.**

**The last row retracts a story this file told two sections ago.**
`NORETRY_347003_watchable` was described as one of the two worst render-cost files
in the corpus — last checkpoint 1964.933, Quantiks' time — saved by accident when
`setdecl --from` happened to rewrite that split. The **pre-repair** copy imports
at **346.97**.

> **It was never an exposure. The checkpoint never drove the block end, so the
> accidental fix fixed nothing** — there was nothing there to fix. A satisfying
> narrative about luck, built on a rule that was already wrong.

### The near-miss that fitted eight of ten

`block end = (samples − 1) × 50 ms` — proposed, and correct only when the sample
grid is unbroken. `TAS_239133` has **9114 samples** and a last sample at
**462.950**, because a cut leaves gaps: the formula says 455.65 and the file
renders to 462.95.

**Count the timestamp, not the samples.** And the hypothesis that hung off it —
that the game sees more samples than our parser — is dead: both readers agree on
9114, and the game never counts.

### The falsifiers that would have tested nothing, a second time

Three cheap discriminators were proposed from the table: `191465/WIP_pad5` and
`WIP_keyboard` (last sample 13.050), `279209/kb_gasfull` and `kb20` (6.600). **All
four coincide with the checkpoint prediction inside the display grid.**

The same trap as the first round, one round later: **rows proposed as tests were
rows where the hypotheses agree.** Of everything staged, `126859/KEYBOARD_24164`
was the only clean discriminator, and it decided the question alone.

### The blocklist, computable with no further measurement

19 files whose block end runs more than 2 s past their own time, 16 of them past
10 s: `186935/CUT_795034` at **+1780 s** (~20 hours of render), 286279's three at
+86 to +441 s, `238835/TAS_239133` at +224 s, and 165922's nine at +24 s each —
though those carry zero car entities and would render nothing at all.

### Field #7 gets an address, and two readers now disagree on 12 files

The split list lives inside chunk `0x0309202B` and **always starts at +24**, with
an 8-byte stride. Verified against the decoder on every control:

```
228607 TAS_19907    7 splits, +24..+72, ends 20.034
285268 TAS_49275   10 splits, +24..+96, ends 49.275
238835 TAS_239133   5 splits, +24..+56, ends 462.982
```

**That retires the `+56` mystery as arithmetic rather than a quirk:** `+56` is the
*fifth entry* — the finish on a five-split map, an intermediate split on a
seven-split one.

The heuristic version it replaces had a failure the address exposes. 126859's
`TAS_23416` holds `w[1] +4 = 23416` (its own time) and `w[6] +24 = 27609`
(TheWoreL's) — **two single-entry runs of equal length**, and the search picked
the first. Corpus agreement went 149 → 153 on that one change.

> **A reader that searches for a structure will lose a tie; a reader that knows
> its address cannot.**

**Where the two readers still disagree, the pattern is informative:**

```
186935  BEST_793893, CUT_795034     16 splits, ends 2138.325   decoder: 793.893 / 795.034
238835  TAS_262907 + 3 siblings      4 splits, ends 1704.277   decoder: their own times
197047  4 files                      2 splits, ends  100.215   decoder: their own times
284238  TAS_97325                    4 splits, ends  184.638   decoder: 97.325
228607  AUTHOR_LAP_20258_watchable   1 split,  ends   20.258   decoder: 24.902
```

In **eleven of twelve** the chunk reader sees the donor's number and the decoder
sees ours — the reverse of the 126859 case. So the two are reading *different
fields*, and on an edited file those fields disagree.

**`AUTHOR_LAP_20258_watchable` is the one that flips**: the chunk reader sees our
20.258, the decoder sees Falco_TM_'s 24.902. That is also the file the game
refuses to open and the one with the malformed skin string — **three readers,
three answers, on the most broken file in the corpus.**

Neither reader is being called right. They agree on 153 of 174; where they differ
a third source is needed, and **the measured block ends are no help, because the
block end follows the last sample's timestamp rather than either of these.** The
table carries both columns per file so the disagreement stays visible instead of
being resolved by fiat.

165922's nine are excluded — the decoder returns nothing for them, so there is
nothing to compare.

### The nickname lives in two places and the strip reaches neither

208024's v2 failed `skincheck` on a donor livery. After `skin set` cleared the
skin path, the file **still read `nickname "Herrlille"`** — the third-place human
whose container the tape was built in.

The nickname has **two** sites:

- a **header** copy, chunk `0x00000000`
- a **body** copy, chunk `0x03092000`

`tmtraj hdr setlogin` and `tmtraj body setlogin` are both required. And the trap
is in the reporting:

> **`skin info` shows only the body copy.** Fix the header and stop, and the tool
> reports a clean file that still announces a human's name on import.

The same shape as the malformed-path specimen, one field over: a reader that sees
one of two sites, reporting a pass.

The full donor set on that file — skin path, storage URL, GUID, sha256, nickname
×2, trigram `HER`, zone `World|Europe|Sweden`, clubtag, login — is what a strip
has to clear. Verified after: reparse OK, **map UID present exactly once** (the
hazard, since a string rewrite can overwrite it), zero occurrences of the donor's
name or GUID, oracle exact on three cold runs, telemetry unchanged at 2.5 mm.

It is now `mh2_deskin.sh` rather than a hand step, **run last — after the declare,
because a string rewrite moves offsets.**

### A margin quoted for twelve hours after it expired

208024's run was reported all night as **6.739 s inside the human record**. True
against Herrlille's 25.681; stale from the moment **deeperjungle's 21.105** landed
the day before. The real margin is **2.163**.

The author-time sign was right throughout in the arm's own reports — the run is
**+0.136 OVER** the AT — and was inverted only in this coordinator's relay.

> **A leaderboard figure has an expiry date and does not announce it.** Re-pull
> the board before publishing a margin, especially on a three-run board that has
> moved within the day.

### 210218: the horizon is a real dial, and it collapses between one sector and two

Sector 11's 990 ms was accepted under an h = 1 contract. Measured across the
horizon table, same seed, same window, same operators, same 35 minutes, same
binary — **only the contract rung moved**:

| horizon | contract | sector 11 yields | evals |
|---|---|---|---|
| h = 0 | its own rung | −926 ms | |
| **h = 1** | seg12, sector 12 frozen | **−990 ms** | 482 k |
| **h = 2** | seg13, sectors 12–13 frozen | **0 ms** | 305 k |
| h = ∞ | the finish | 0 ms | 197 k |

`DONE best=78008` — the seed's own time to the millisecond, after 304,560
evaluations.

**The null is trustworthy because of what sits beside it.** The positive control
is the row above, in the same configuration one sector nearer: 990 ms. **An
instrument that finds nearly a second at h = 1 and nothing at h = 2 is measuring
the contract, not its own limits.** And 18 % of candidates fired the rung, so the
objective was bright and dense throughout — not a dark-objective null. The
scoring goal relocates only the checkpoints *after* CP13 and leaves CP13 at home,
so there is no repeat of the relocated-gate error.

With the other measurement from the same night — the h = 1 winner costing 2.760 s
by CP13 — the two agree from opposite directions:

> **Sector 11's 990 ms is an artefact of where the clock stops, not a property of
> the car.** It exists under a one-sector contract, costs 2.8 s under a two-sector
> one, and does not exist at all if the car must still be on the line two sectors
> later.

The strategy's central claim survives: h = 1 does dominate h = 0 and h = ∞ *as a
search objective*. **What does not survive is reading an h = 1 gain as time you
can spend.**

### And the 95.538 may not belong to the chain

Interim, reported before it could be claimed: at 36 of 70 minutes the
**plain-incumbent** tail search is at 95.570, where the chain-seeded run was at
95.607 at the same point. **The control is currently ahead.**

If it finishes ahead, the 68 ms credited to the chain conversion belongs to the
tail search instead, and the "first upstream gain converted into a lap" reading
goes with it.

> **A result attributed to a mechanism needs the same run without the mechanism.**
> Not as a formality — the attribution control here is on course to take the
> finding back.

### A winning run that was nearly lost to a filename

208024's 18.160 — the run that finally beat that author time — was found, logged,
and then **its spec file was gone.** `mh_hunt3.sh` writes
`/tmp/hunt3_<chunk>.spec` and restarts the chunk counter at 1 on every
invocation, so relaunching the hunt overwrote the file holding the winning line.

The failure did not look like a lost file. `mh emit` produced nothing and the
oracle returned **`NORECORD` five times** — which reads exactly like a broken
tape, not a missing input.

It was recovered only because the candidate generator is deterministic:
`srand(seed)` with seed 70002, candidate index 7161, so re-running the awk with
those two numbers reproduced the line exactly and the emitted tape validated at
18160.

> **That was luck of design, not of process.** With `$RANDOM` or a time seed the
> run would have been unrecoverable — a number in a log and nothing else.

Two fixes, both cheap: per-seed filenames, and **bank the spec line of every
finisher at the moment its chunk completes**, not when someone gets round to
looking at the log.

### The doubling bug: the repair step was never needed

The four 208024 files that came out exactly twice their input size are withdrawn
and re-issued from the pre-deskin originals, each **186 bytes smaller** than its
input with the size assertion armed.

Root cause: **`hdr setlogin` doubles a body-only file, and it was never needed.**
`skin set` + `body setlogin` clears the nickname on its own. The "header copy"
that seemed to require a second edit was an artefact of a mis-parse that reported
`user data 0 B` beside a 50 MB chunk.

> A repair added to cover a field that another step already covered, justified by
> a reader that was misreporting the file. **Three wrongs that cancelled into a
> plausible-looking pipeline.**

### 210218 closed: 95.507, and the attribution control says the chain earned 51 of the 68 ms

| tape | lap | vs 95.575 |
|---|---|---|
| `pwch_best_95507_SEARCHTAPE…DO_NOT_PUBLISH_declares_96281` | **95.507** | −68 ms |
| `pwch_ctl_95558_attribution_control` | 95.558 | −17 ms |

All validated three times with the human record `r01` returning 96281 exact in the
same sweep. **0.774 s under the human world record, 1.030 s over the author time
— not beaten.**

The control is the same search from the **plain incumbent**: identical window,
flags, caps, workers and 70-minute budget, differing only in the seed. **So 51 ms
belongs to the chain's sector-14 handover and 17 ms is tail search any seed would
have found.**

And the chain earned *more at the finish* than at its contract rung — 51 ms
against the 20 ms the rung showed. The S14 edit changes the state at CP15, and the
tail re-driven from that state beats the tail re-driven from the incumbent's.
**First positive evidence on this map that an h = 1 handover is worth something
downstream: small, attributed, controlled.**

### The claim that was published mid-run and withdrawn

> **"The small end of the band fails."** It was the headline for an hour and it
> was an artefact of elapsed time: **a repair 32 ms behind at half budget was the
> winner at full budget.**

Compare converged runs, or say the budget out loud. The surviving statement is
narrower and holds: the convertible band is **small and late** — −20 ms at CP15
converted, −990 ms at CP12 did not at any budget. Start at the tail and convert
one sector before moving back.

Three instrument findings from the same arm, each worth more than the millisecond
it cost: **1e8 score bands invert on a 16-checkpoint map**; **a silent rung still
pays the depth bonus** (needs `--segstrict`, and strict alone is a cliff);
**concurrent searches share `/dev/shm/tmsearch` by default.**

### And the tape is a time, not a file

All three laps are raw search output **on r01's container, declaring 96281** — the
human world record's time — carrying r01's telemetry. Nothing regenerated, no
container touched, no page written. The arm said so itself rather than being
asked:

> Treat 95.507 as a search result only until the container gate has looked at it.

`pwiw_TAPE_95575_watchable_v1` remains the last renderable artefact on that map.

## Field #8: the car is facing the wrong way, and no positional check can see it

Reported by vjeux watching the 197047 clip — **"the car orientation in the TAS is
wrong"** — on a video that had passed every gate we own.

The page claimed the tape **"regenerates 1917 of 1917 samples"** with a sample CSV
*identical* to the filmed file, an equality rather than a tolerance. Both true,
and both silent about the defect: in the record layout

```
pos +208    quat +192    vel +220    (reclen 452)
```

**position and orientation are different fields.** A comparison over positions
passes at 1917 of 1917 while the facing is wrong for the whole run.

> **"Every sample identical" is a claim about the samples you compared.**

### The mechanism, read from source rather than from a report

`fk regen` **does** write the quaternion — `write_transform` encodes
`ang = acos(q[3])`, heading `atan2(q[1], q[0])`, pitch `asin(q[2]/sin ang)`. So
the field was never simply unwritten; the fault is in *how*.

Every run on a map spawns identically, which gives a free reference. All 26 human
recordings on 197047 read `(3.39e-05, −0.7071, 0, 0.7071)`:

```
TAS_96852_v1              (-3.39e-05, -0.7071, 0, 0.7071)   matches
KEYBOARD_96759_metronome  (-3.39e-05, -0.7071, 0, 0.7071)   matches
TAS_95839_analog          ( 1.0000,    0,      0, -2.4e-05) IDENTITY — no rotation
KEYBOARD_96412_twokey     ( 1.0000,    0,      0, -2.4e-05) IDENTITY
```

`TAS_95839_analog` is the file in the withdrawn clip. **Two of the four tapes on
that page are affected and two are fine — which is why comparing our own files
against each other never showed it.**

### The corpus, and why three classes had to be separated

173 files, 24 flagged, **13 real**:

| class | files | |
|---|---|---|
| **identity quaternion — no rotation at all** | **6** | 197047 ×2 (incl. the filmed one), 186935 ×2, 227654 `TAS_57503`, 238835 `AUTHORCUT_246602_watchable` |
| **kind flip `(w,x,y,z)` vs `(x,y,z,w)`** | **4** | 145875 ×3, 274191 `KEYBOARD_7476` — permuted dot **1.000** |
| no car entity at all | 9 | 165922's nine |
| **q vs −q — the SAME rotation, not a defect** | 5 | 199100 ×5 |

**149 of 173 match the human spawn exactly.**

The five 199100 rows are why this must be computed as a rotation rather than a
byte comparison: ours read `(−0.7071, 0, 0.7071, 0)` against the humans'
`(0.7071, 0, −0.7071, 0)` — **dot product 1.000**, the same rotation with the
opposite sign, exactly as `write_transform`'s own comment says. A naive equality
check calls those five broken and they are perfect.

> **q and −q are the same rotation.** A checker that does not know that will
> condemn correct files and teach everyone to ignore it.

Positive control: `NORETRY_347003_watchable` reads `(0, 0.7071, 0, 0.7071)` —
exactly 238835's human spawn — so the reader distinguishes at the fourth decimal
on a file whose history is known.

### The part that stings

The project's own notes **predicted this failure and named it**, before it
happened:

> *"the probe reports the same quaternion offset every run but its KIND flips …
> positions are unaffected, so NO GATE CHECK SEES IT; the car simply faces the
> wrong way for the whole render."*

`fk regen --quat-kind N` exists to pin it. **The tooling anticipated the bug, the
gate never checked for it, and six files went to film.**

The check is one line — *first-sample orientation against a human recording of
the same spawn, compared as a rotation* — and it would have caught all ten.
