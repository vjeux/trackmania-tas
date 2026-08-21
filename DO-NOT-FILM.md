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
