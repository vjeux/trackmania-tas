# Publishing a clip — the traps, in the order they will bite you

## 0. THE HARD RULE: the publish gate is an ANONYMOUS fetch

**A clip is published when a logged-out visitor can download and play it. Nothing
else counts.** Every check must be made with no cookie, no `gh auth`, no
credential of any kind:

```bash
curl -s -o /tmp/c.mp4 -w '%{http_code}\n' -L "$URL"   # must be 200
ffprobe -v error -show_entries format=duration -of csv=p=0 /tmp/c.mp4
```

Status code alone is not enough — fetch the bytes and probe them. A `302` only
says GitHub was willing to sign a redirect.

**This rule exists because we broke it and shipped 19 clips, 18 of which were
invisible to the entire world.** On 2026-08-21 an anonymous fetch of all 19
published embeds returned:

```
anon=404  auth=302     18 clips
anon=302  auth=302      1 clip  (203072)
```

The assets existed, the repo was public, the READMEs were committed and pushed,
the embeds were correctly formed, and every clip played perfectly **for us**,
because every verification we had ever run carried vjeux's session cookie. The
pipeline was built end to end behind a credential, so the one failure mode it
could not detect was the only one that mattered. A gate that runs with
credentials is not a gate.

`tools/ship-clip.sh` now performs this fetch itself and refuses to report
success on anything but 200 + playable bytes.

## 1. A pushed commit does NOT make an attachment public

This is the mechanism behind trap 0, and it is the opposite of what we believed.
The old note here read: *"a fresh asset 404s publicly until a commit references
it — it becomes public when a README referencing it lands."* **That is false.**
All 19 READMEs landed on the default branch of a public repo (`origin/main ==
HEAD`, 0 ahead / 0 behind) and 18 of the 19 assets stayed 404 to the public.

What actually authorises an asset is a reference in content **GitHub itself
renders when the content is saved** — a release body, an issue or PR comment, a
web-editor save. A plain `git push` of markdown never triggers it: the bytes
arrive over git, and nothing in GitHub's attachment service is told the asset is
now referenced by public content.

Proven directly, on an asset that had been 404 for hours:

```
before:  anon=404
         (append the asset URL to the videos-v1 release body)
after:   anon=302, then 200 with playable bytes — within seconds
```

Applied to all 18, then re-verified one at a time: **19 of 19 now download
anonymously and probe as valid MP4s.**

So the `videos-v1` release body carries a listing of every inline asset URL,
inside a `<details>` block. **That listing is load-bearing. Deleting it may take
every inline player in the repo dark again.**

One thing remains unexplained: why 203072 was public without such a reference.
No issue, PR, or web-flow commit mentions it — every commit in the repo has a
plain committer, none came through GitHub's web editor. Time alone is not the
answer: the asset I tested had been sitting for hours at 404 and flipped only
when it was referenced. Treat 203072 as an unexplained exception, not as
evidence that pushing is sometimes enough.

## 2. Never hash a file while it is still downloading

`curl` returning is not the same as the bytes being on disk and settled. Hashing
a partially-written file gives a short size and a wrong md5, which then reads as
a *transfer mismatch* — a real-looking failure caused entirely by measuring too
early. This happened twice in one session and nearly caused a good clip to be
withdrawn.

```bash
curl -s -L --retry 3 "$URL" -o "$F"
a=0; b=1; while [ "$a" != "$b" ]; do a=$(stat -f%z "$F"); sleep 1; b=$(stat -f%z "$F"); done
md5 -q "$F"      # only now
```

## 3. The attachment size limit is not a constant — do not predict it

One upload was rejected with S3 `EntityTooLarge`:

```
ProposedSize 22670023   MaxSizeAllowed 22667264
```

It is tempting to treat 22,667,264 as the cap and pre-emptively re-encode
anything larger. **That is wrong**: a **26,082,003-byte** file uploaded
successfully a few minutes later. Whatever the limit depends on, it is not a
fixed byte count.

So: **upload first, and re-encode only if the upload actually fails.** A
prediction here costs quality on files that would have gone through untouched.
The full-quality original should always go to the release asset regardless, so
the re-encode only ever affects the inline player.

## 4. The CSRF token is not a form input

This is what made every early attempt return 422. The token the drag-and-drop
upload needs lives in the **edit page's embedded JSON**, not in any `<input>`:

```
csrf_tokens['/upload/policies/assets'].post
```

Fetch any file's edit page (`https://github.com/<owner>/<repo>/edit/main/<path>`)
with the session cookie and read it from there. The three-request dance is then:

1. `POST /upload/policies/assets` → an S3 policy, an asset id, **and a second
   token** (the one step 3 needs — it is not the same token as step 1's)
2. `POST` the bytes to the S3 URL, using that policy's own form fields verbatim
3. `PUT /upload/assets/<id>` with the second token, to finalise

Finalising is what makes the asset *exist*. It is not what makes it *public* —
see trap 1.

## 5. Before filming two cars, prove they are two cars

**A separation of zero and a separation you cannot see look identical on the
gate.** Decode both ghosts to their sample CSVs and compare the md5s. If they
match you are about to film one lap twice, wearing two liveries.

```bash
for f in <ours> <opponent>; do inputcount --csv "$f" | md5sum; done   # must differ
seplag <ours> <opponent>                                             # want INDEPENDENT / incidental
```

This is not hypothetical. On 285268 **all eight of our tapes decode to a human's
trajectory** — seven of them to burntbagels' 49.446 with one identical CSV md5,
including the tape named as our best. Every one of them re-simulates to its own
claimed time, so the *results* are real; what is unreliable is the recording of
how they got there, which is exactly the material a video is made of. A two-car
clip from that map would have shown burntbagels racing himself, and its
separation profile (868 of 986 samples in the "two visible cars" band) looked
like the *best* pairing in the set.

Run it before every two-car shoot, on the file you are about to film.

## 6. Getting the opponent: the render box can fetch its own

No bridging needed, and reading a leaderboard is not submitting to one:

```bash
# TMX map id -> Nadeo map uid
curl -s -A "$UA" https://trackmania.exchange/api/maps/get_map_info/id/<tmxid> | jq -r .TrackUID
# uid -> the live board, with times, players, timestamps and ghost URLs
curl -s -A "$UA" https://trackmania.io/api/leaderboard/map/<uid> \
  | jq -r '.tops[] | "\(.position) \(.time) \(.player.name) \(.url)"'
# tops[].url -> the recording itself
curl -s -A "$UA" -L https://trackmania.io/api/download/ghost/<guid> -o rank001_<time>_<player>.Ghost.Gbx
```

Keep the player's real login in the filename and stage with a `90_`/`91_` prefix
so it sorts last — the ghost picker selects by **row index**, so inserting a file
anywhere else silently shifts every row after it. Never relabel another player's
recording as ours.

Two provenance controls, both cheap: a genuine downloaded ghost **re-simulates to
its leaderboard time exactly**, and the game itself reads the login off the file
(`track[1] name=Ghost:AffiTM` in `/mtclip`), so the opponent names itself on
import.

## 7. A ghost's camera dies with the ghost

A MediaTracker camera bolted to an entity lives exactly as long as that entity's
sample stream — and **our tapes stop sampling before the finish while downloaded
human recordings keep going past it.** On 270053 our stream ends at 4.45 s, our
own crossing is at 4.492, and the human crosses at 4.495: with the camera on our
car the shot cut to a stray static view of the map at 4.383 and the last 0.6 s
contained **no car and neither finish**. `duration`, `blackspans=0` and the gate
all passed it.

So for any pairing where the finish matters, bolt the camera to the
**longer-lived** entity (`CAMON=wr` in `render2.sh`). Ours is still in frame; it
is simply not what the camera follows.

## 8. When one camera cannot hold both cars, use two cameras

`tools/splitscreen.sh` renders each run alone and stacks them, clocks aligned at
the start. Use it whenever the opponent spends the run outside the shot — 276877
is 61.5 m and 6.061 s away, 228607 is 356 m and 4.605 s away. A two-car label on
a clip whose second car is behind the camera is **a caption writing a cheque the
picture cannot cash**.

The shorter run holds on its final frame rather than cutting to black, so the gap
reads as *time*: our car parked at the flag while the other pane is still driving
**is** the 4.605 s.

**It only runs on the render box.** The Mac's Homebrew ffmpeg 8.0.1 has no
libfreetype, so `drawtext` does not exist there (`No such filter: 'drawtext'`);
WhiteStick's 9.0.1 essentials build has it, and Windows ships the fonts.

## 9. Overriding a gate check without switching the gate off

`GATE_OVERRIDE="C3,C8=<reason>"` films a ghost whose **only** failures are the
named ones. It exists because a check can be superseded while still wired in —
197047's C3 fires on a 620 m jump that is a **respawn we actually drove** (proved
by our own inputs reproducing it at the same instant, which a splice cannot
survive), and its C8 is the known wheelspin false positive on a map that slides
for a hundred seconds.

The override must name **every** failure exactly; one unexpected id and it
refuses. That keeps the distinction between *this check is wrong about this file*
and *no check ran*, and the reason is echoed into the render log beside the
verdict. Whoever sets it owns it — and the page must tell the viewer what they
are seeing.

## The general shape

Traps 2 and 3 are the same mistake: **a check that was wrong about data that was
right.** Trap 0 is its mirror image and the more dangerous one: **a check that
could not fail**, because the instrument was standing inside the thing it was
supposed to be testing. Traps 5 and 7 are the third face of it: **a check that
was in no position to see the thing that was wrong** — a gate cannot tell a
separation of zero from a separation you cannot see, and no file-level check can
tell you the camera went blind before the flag.

Before believing a green result, ask what a red one would have looked like — and
if you cannot answer, you do not have a check. Then watch the video.

## 10. Verifying a TAKEDOWN: the same instrument in reverse, run twice

Deregistering an asset from the release body **does** take it down. Measured on
five withdrawn clips, `env -i` with no credential, checking bytes rather than
status codes:

```
before deregistration:   all five  200, full playable bytes
immediately after:       four      404, 9 bytes ("Not Found")
                         one       200, 13,481,014 bytes   <-- still serving
30 s later:              that one  404, 9 bytes
```

**A withdrawn asset returns HTTP 404 with a 9-byte "Not Found" body** — byte for
byte what an asset that was never published returns. So the withdrawal is real:
GitHub unmarks an asset when the content authorising it goes away.

**But propagation takes up to a minute, and it fails in the dangerous
direction.** For that minute the asset keeps serving the whole file to
logged-out visitors after the reference is gone. An operator who checks once,
immediately, sees `200` and can reasonably conclude the deregistration did not
work — and then goes looking for a second mechanism that does not exist.

> **Verify a takedown twice, at least thirty seconds apart, and treat the second
> reading as the truth.**

This is the publish gate run backwards, and the same principle governs both: the
question is always *what bytes does an anonymous visitor get*, never what any
status code or dashboard says. Note the pleasing symmetry with §0 — a dead asset
and a withdrawn asset are indistinguishable, which is exactly right, because to
a reader they are the same thing.

## 11. Four independent readers of a ghost's container, and none subsumes another

A tape can be **completely ours in every position it contains and still be
somebody else's file**. Four readers now exist, each reading a *different* field,
and each has caught something the other three missed:

| reader | field it reads | what only it catches |
|---|---|---|
| MediaTracker import name | **nickname** | a container still wearing its owner's name — `Ghost:OrmeEssence44` where ours all import as `Ghost:TAS` |
| the game's parser (`/validatepath`) | **login → account id** | a half-cleaned container: 165922's nine display `TAS` and still carry the donor's account |
| byte scan for a per-player GUID | a second identifier | the 276874/276877 pair, on maps whose parser reads were unavailable |
| **u32 census of the declared time** | **six time slots** | **needs no reference file and no server at all** |

**The census is the cheapest and the only reference-free one.** The declared time
is stored exactly **six times** as a little-endian u32, at a structural spacing
(`+52 / +148 / +4 / … / +20`) confirmed across two different maps. Six copies of
its own validated time and none of any other is a clean container; zero of its
own and six of another is a foreign one. There is even a tell for the borrowed
case — one gap widens from +148 to **+167**, because the carrier's skin path
string is 19 bytes longer.

**Every one of these is a positive detector, not a clearance.** The census reads
the time as a u32 only, so a value stored as a float or split across a struct
boundary is invisible to it. The nickname reader passes files that still carry a
foreign account underneath. Use them together, and never read a pass from one as
a verdict from all.

### The two identity readers are exact mirrors, proven on one file

227969's **repaired** file (`0fe68885…`) is the case that settles how these fit
together. It passed everything: header 8.050 = validated 8.050, **no account id**,
census clean (`8050 ×6, 8197 ×0`), telemetry bit-identical to our own tape at 162
of 162 positions and independent of the world record at every lag. It imported
into the MediaTracker as:

```
track[0] name=Ghost:Titoch_tm   end=8.05     <- ours
track[1] name=Ghost:Titoch_tm   end=8.17     <- the actual world record
```

**Our run, still wearing the record holder's nickname.** The two block ends prove
the import was right, so this is the field itself, not a mix-up. Set against
165922's nine — donor's **account id** present, nickname reading `TAS` — the two
failures are mirror images:

| | nickname | account id |
|---|---|---|
| 165922 × 9 | `TAS` ✓ | donor's ✗ |
| 227969 repaired | `Titoch_tm` ✗ | none ✓ |

**Each reader is blind to exactly the case the other catches.** A container is
ours only when both agree — and the nickname is the field a byte scan cannot
read, so the game is the only instrument for it. Both renderers now run that
check automatically before rendering a frame: if our ghost does not import as
`Ghost:TAS`, the shoot dies with the name the game gave it.

## 12. "Wrong ghost" is not always the picker

The ghost picker selects by row index, so it is the first suspect when an import
opens the wrong run — but it is not always guilty, and the fix differs.

199100 failed three times with `wrong ghost: clip end=52.2s, expected span
49.90s`, twice after a full game restart and once with **exactly two files in the
folder**. The picker was innocent every time. The file we asked for was the file
that loaded — and it *declared* 52.202, because it was built in the previous
record holder's container. The block end the check reads comes from the
container's declared time, not from the sample stream.

So when a shoot reports the wrong ghost, read the track name before touching the
staging:

- `name=Ghost:<a human>` — the file is wearing someone else's container. Nothing
  about the folder will fix it.
- `name=Ghost:TAS` with an unexpected duration — now suspect the row index, and
  stage exactly one ghost in the folder, which cannot be mis-indexed.

The check earned its keep either way: keyed to the *outcome* — is this the ghost
I asked for — rather than to any mechanism, it survived the mechanism being
wrong, twice, and stopped a video of `Ghost:OrmeEssence44` going out captioned as
our arc.

## 13. Reported coverage is not measured coverage

Every check that failed tonight was **reporting on itself**. Five different
tools, one shape:

| tool | what it reported | what was true |
|---|---|---|
| `M-time` in a manifest verifier | `PASS` | it compared two numbers **from its own command line** and never read the file |
| `curl` | success | it had successfully downloaded a **404 page** |
| an anonymiser | *"2 strings replaced"* | it left the donor's zone string in place — the file said `World\|Europe\|Sweden`, the tool looked for `World\|World\|Sweden` |
| the ghost parser | *"1 replay parsed"* | **two** were staged; the edited one had vanished from the batch with no error naming it |
| a locator | *"0 candidates"* | it was pointed at the wrong offset |

The fix is always the same shape: **measure the outcome, not the operation.**
After an upload, fetch the bytes. After an anonymisation pass, **grep the output
for the donor's strings** — the tool tells you what it did, only a scan tells you
what remains. After a container edit, assert the parser produced a row **for your
file, by name**, rather than that the batch succeeded.

**A file that vanishes reads exactly like a file that was never there.**

## 14. The file's own layout decides the repair method

Two container repairs failed tonight by doing what had worked on the previous
file:

- **Header path vs body path.** A ghost with header user data keeps its nickname
  and skin strings in the header, and editing them requires decrementing the
  size u32 at **offset 77**. A ghost whose header user data size is **0** keeps
  everything in the body, where a length change is free. Running the header path
  on a body-only file misreads the body as a chunk table, grows it 5263 → 10436
  bytes and **overwrites the map UID** — a repair that destroys the map binding
  while still producing a plausible file.
- **Block transplant vs in-place edit.** Copying a clean twin's identity block
  worked on 227969 and silently broke 145875, whose container carries an extra
  37-byte skin blob and a second skin path the twin does not have. The
  transplant removed fields the file needed and the server dropped it from the
  batch without naming it.

So: **diff against a clean twin to find *which* fields differ, but repair in
place, preserving every intervening field and the field count.** The diff tells
you what to change; the file tells you its own layout. **Never pattern-match the
last file that worked.**

## 15. Agreement between readers of the same field is one reader

On 145875 three tools agreed the tape declared 6.360 — `inputcount --meta`,
`ghostqc`, and a u32 census showing `6360 ×6` with no `6342` anywhere. All three
were correct and all three were **reading the same stored field**, which is
`DeclaredResult`. The server's `ValidatedResult` was 6.342, and its own `Desc`
said *"validated time is actually better! (6360 > 6342)"* — a sentence it only
emits when a run beats its header.

Two rules come out of it:

- **Never quote `DeclaredResult` or `ValidatedResult` without their labels.** A
  mislabelled operand is worse than an invented one: an invented number gets
  checked, a mislabelled one gets *corroborated*.
- **Before treating three sources as three, ask what each one read.** Three
  readers of three different fields is strong evidence. Three readers of one
  field is one reader wearing three hats.

And the counterpart, for synthesised tapes: **never read a tape's time off its
header, filename or manifest — re-simulate the file you are holding.**

## 16. The majority is not the answer

A regeneration on 208024 was run **24 times** and ranked against ground truth
(the tape's own route dump):

- **2** attempts landed on the true clock, byte-identical to each other at 0.0026 m
- **5** clustered together on a *wrong* answer, 7.81 m out
- **11** were 900–1400 m out

**The largest agreeing cluster was wrong.** Any procedure of the form
"regenerate a few times and take the agreement" ships the 7.81 m file. The error
also tracked the clock offset monotonically — −210 ms → 7.81 m, −300 → 17.29,
−690 → 43.29 — which is how you tell *right car, wrong tick* from *wrong car*.

Elsewhere the same trap: four of five regenerations agreed with each other on one
map and **all four were wrong**. Rank against ground truth, or do not rank.

## 17. Absence of a result is not a result

The nickname reader records what the MediaTracker calls an imported ghost. When
an import produces no track it writes `NONE` — and **`NONE` looks exactly like a
finding.** It lied twice in one night, in both available directions:

**A dead game answers every import with no track.** Trackmania crashed during a
long sweep and the reader wrote `NONE` for eleven files that had read
`Ghost:TAS` an hour earlier. Guarded now: it refuses to start unless `/ping`
answers, and aborts mid-sweep rather than record a row against a dead
instrument.

**An import sometimes just misses.** Five files were reported to the fleet as
*"the game refuses to load them while our own parser reads them fine."* Four of
them import perfectly on retry, with correct durations — 347.0 s for the tape
named 347003, 239.1 s for the 239133 cut. **They were an instrument artefact
published as a finding.**

The existing data said so and was not read properly: the failures came in
**consecutive pairs** — rows 3 and 4 of one map with rows 5–9 fine afterwards,
the last two rows of another. *A per-file property does not recover after two
rows, and a dedupe does not un-dedupe.* Non-monotonic failure is the signature of
a transient, and it was visible in the table before anyone ran an experiment.

The experiment that settled it carried both controls, which is what a
negative-result test needs:

- a **positive control from a different map's folder**, imported first — it
  still produces a track, so it proves the session is healthy without spending
  the experiment on a sibling
- a **mirror control**: the subject first, then a sibling. If order were the
  cause, the failure would move to the sibling. It did not move; both imported.

`nickcheck.sh` now retries a nameless import three times before recording
anything. A real refusal still reports `NONE-after-3`; a missed attempt no
longer reports at all.

**The rule:** distinguish *the answer is no* from *I did not get an answer*, or
the table fills up with the second wearing the first's clothes. Every tool in
this pipeline that has produced a false finding produced it this way — the 404
that `curl` downloaded successfully, the parser that said `1 replay parsed`, the
locator that printed `0 candidates`, and now the reader built to catch them.

## 18. What decides a clip's length: the last sample's timestamp

Not the declared time, not the container's record span, not the last checkpoint.
**The MediaTracker block end is the timestamp of the ghost's last sample**, and
that is what the render runs to.

Measured on this box, controls at open and close of each session:

| file | own | span | last checkpoint | **last sample** | block end |
|---|---|---|---|---|---|
| `126859/KEYBOARD_24164` | 24.164 | 24.400 | 24.342 | **24.150** | **24.15** |
| `126859/TAS_23416` | 23.416 | 27.800 | 27.609 | **23.400** | **23.40** |
| `279218/KEYBOARD_5350` | 5.350 | 566.080 | 5.350 | **5.350** | **5.35** |
| `238835/TAS_239133` | 239.133 | 1964.930 | 462.982 | **462.950** | **462.95** |
| `238835/TAS_347003_noretry_v4` | 347.003 | 1964.930 | 347.003 | **347.000** | **347.00** |
| `238835/NORETRY_347003_watchable` | 347.003 | 346.970 | **1964.933** | **346.970** | **346.97** |

Six for six on the last sample; the other four columns each fail at least once.
The last row is the decisive one — its last checkpoint is a donor's 1964.933 and
it still imports at 346.97.

**A near-miss worth recording**, because it fitted eight of ten cases and was
wrong: *block end = (samples − 1) × 50 ms*. That is only the last sample's
timestamp when the samples sit on an unbroken 50 ms grid. `TAS_239133` has 9114
samples and a last sample at 462.950 — the grid has gaps — so the formula reads
455.65 and the file renders to 462.95. **Count the timestamp, not the samples.**

### The two ways a short run becomes a long render

They are different fields and they barely overlap, so a blocklist needs both:

1. **Extra car-entity groups → a second track.** `279218/KEYBOARD_5350` imports
   as `Ghost:TAS@5.35` *plus* `Ghost:SceneryEvents@566.08`, and a clip runs to
   its longest block. Measured: **16,983 frames, 566.066 s of video, ~19 minutes
   of rendering** against the control's 162 frames, 5.400 s and ~40 s.
2. **A late last sample → a long ghost track.** `186935/CUT_795034`'s last
   sample is at **2575.150 s** — 43 minutes of video, and at the measured ~28×
   that is roughly **20 hours of rendering** for one clip.

A file with one entity group and a foreign span is harmless: 238835's five all
import with one track at their own block end.

## 19. The wedged game: `/editmap` returns `ok` and nothing opens

After a crash the game can come back answering `/ping` with `pong` and reporting
the menu from `/ctx`, while every `/editmap` returns `ok` and no map ever loads.
Two different maps failed identically, including one that had opened forty
minutes earlier — which is how you tell the state from a bad map file.

**`ok` from the plugin means the call was accepted, not that the game did it.**
A liveness check that reads a return code passes cleanly against this state, the
same way a dead game answers every import with `NONE`. The only reliable exit is
a full restart with re-injection.

## 20. An exact-equality contamination test is blind to a re-encoded copy

`seplag` reports `DONOR-GRAFT` when two ghosts share **exactly zero** separation
for a run of samples. On 199100 it reported `INDEPENDENT: no identical position
at any lag` for a pair that is the same run:

```
sample-CSV md5   our 47.483        08bedfc0efdbcb7d2a048054910eae45
                 uelen.'s 47.838   08bedfc0efdbcb7d2a048054910eae45   identical

separation t < 40.000 s   800 samples   mean 0.000476 m   max 0.000906 m
separation t >= 40.100 s  157 samples   mean 18.71 m      max 52.86 m
```

**Half a millimetre for 800 consecutive samples is not independence — it is a
copy that has been through a float re-encode.** The exact test cannot see it,
because a re-encode never reproduces the bits.

This is the `sep {:.2}` bug from the other side. That one rounded real
differences away and called distinct files identical; this one demands exact
equality and calls one run two. **Both failed toward "clean", which is the
direction that publishes.**

A contamination test needs a **near-identity band** — positions within ~1 mm for
N consecutive samples — not an equality. And the cheapest independent check is
the one that settled it here: **compare the input tapes.** Two runs that were
driven separately do not share a sample-CSV md5.

## 21. Two ghosts in one frame beats two panes

**A car leaving the frame is content, not a defect.** The project exists to show
where our run diverges from the human record, and the moment the human stops
keeping up is the thing a viewer came to see. A split screen hides exactly that:
each car gets its own pane and its own scale, so neither is ever *behind*.

Use a split screen only when one camera genuinely cannot show the run — 228607's
356 m, 276877's 61 m with the human six seconds adrift, 208024's 335 m. **Not
because the cars separated.**

It also decides the camera target. Following **our** car makes the divergence
read as the human falling behind, which is what happened. Following the
longer-lived ghost keeps a car on screen past both finishes (trap 7) — that is
worth it when the finish is the story, and not otherwise.
