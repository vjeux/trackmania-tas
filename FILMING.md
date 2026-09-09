# How a clip is shot

Not preferences. These are the rules the render scripts enforce, and each one is
here because it was broken once and the clip had to come down or be re-shot.

## 1. The camera is on our car. Always.

**Both runs go in one scene, the chase camera bolted to the TAS ghost, the human
record as the opponent.** The TAS ghost is imported first, which is what makes it
ghost 1 and what the stock External camera follows.

This is not a parameter and there is no flag for the other thing. `render2.sh`
used to take `CAMON=wr`, and it now refuses.

The escape hatch existed for a real problem: a ghost camera block lasts exactly
as long as its sample stream, our tapes are truncated at the finish, and a
downloaded human recording keeps sampling for about half a second past its own.
On 270053 that killed the shot at 4.383 s and left the last 0.6 s an empty view
of the map. But the fix chosen — bolt the camera to the longer-lived car — trades
the subject of the clip for the last few frames of it, and it kept being reached
for: 279197 shipped with the camera on ShcrTM's car to avoid **50 ms** of dead
tail on a 10.594 s run.

**When our camera block runs short, trim the clip.** That costs a frame or two
and this pipeline already does it elsewhere (274191 lost 0.43 s of dead camera
off its end). If the block cannot cover our own finish at all, that is a defect
in the recording — fix the recording.

## 2. One scene, not two panes

Two cars in one frame is the shot. `clip split` exists only for maps where a
chase camera provably cannot hold both — 276877 (the human 61.5 m away), 228607
(356.68 m). Distance is the test, and it is measured, not judged.

**A car leaving the frame is not a failure.** The point of this project is
showing where a machine's line diverges from the human record; a human who falls
behind and disappears has *shown* you the divergence. Do not switch to split
screen to keep both cars visible, and do not pick a slower pairing because it
frames better.

## 3. Prove they are two cars before filming two

A separation of zero and a separation you cannot see are the same picture. Decode
both to CSV and compare md5s; equal means one tape is carrying the other's run,
and the clip would be one lap wearing two liveries. On 285268 all eight of our
tapes decoded to a human's trajectory — **which is why this rule exists.**

*(Status of that example, checked 2026-08-22: the 285268 files have since been
repaired. `tmtraj corpus splice --root .` now reports all five of ours **CLEAN**
against `HUMAN_rank2_keyboard_49491` — 0 of 986–990 samples identical, worst
separation 3.55–15.60 m. The rule stands; its worked example is now a
before-and-after rather than a live defect.)*

**Two cheaper ways to run this check than by hand**, both over the whole corpus:
`tmtraj corpus splice --root .` for telemetry that is another driver's, and
`tmtraj corpus dup --root .` for two of *our* files carrying one recording.
Note `corpus dup` **silently passed everything until 2026-08-22** — it shelled
out to a command that does not exist and read the failure as "the tapes are
identical" — so any pre-dated clearance from it means nothing (`tools/README.md`).

## 4. The caption

```
**<Map>** — TAS **<time>** (<±ΔAT>) | AT <at> | WR <wr> by <holder>
```

No "vs". Two-car facts go in prose beneath it. Times are seconds with a decimal —
`20.237`, never `20237`.

Re-pull the leaderboard before writing a WR into a caption: one shipped 12 hours
stale and wrong. Confirm the human ghost you filmed against is actually rank 1 —
`279209/90_HUMAN_r001_6604` is rank 2, and the file name says otherwise.

## 5. Nothing publishes without the anonymous fetch

`clip ship` (in `tools/clip`) does the whole chain and refuses at each step. The step that
makes an asset public is **registering its URL in the release body**, not the
commit that references it — 19 clips were shipped before anyone learned that and
18 were 404 to everybody but us. The last step fetches the URL back under
`env -i`, with no cookie, no token, no netrc, and requires 200 and playable
bytes. A gate that runs with credentials is not a gate.

Takedowns run the same way in reverse: de-register from the release body first,
then delete the asset, then confirm the 404 twice about 30 s apart.

## 6. Watch what you made

A clip nobody has looked at is not a clip. Pull frames back out of the finished
file and look at them:

```
clip frames <clip.mp4> <outdir> --at 1.5,5.0,8.0        # or -n 8 to spread them
```

Grab the SAME instants out of the clip you are replacing and put the two side by
side. That is what showed the 2026-08-23 carrier re-shoot had worked — flat
black tyres at 5.000 before, lit hubs and sparks after — and it is also what
showed that untitled 02's published clip carried an input overlay its page never
mentioned, which the re-render had left off.

## 7. Every clip carries the controls overlay, and the renderer puts it there

vjeux, 2026-09-09: *"In the future can you add the controls overlay on the
videos you generate"* — and, when that was about to become a checklist item,
*"Please do not make it a rule, change the renderer to do it by default."* So it
is not a rule anyone remembers. It is what the tools do:

- **`clip cut` draws the overlay.** `clip cut <in.webm> <out.mp4> --ghost
  <run.Ghost.Gbx>` is the webm→mp4 step, and it draws the run's inputs (steer,
  gas, brake, respawn, and the strip of them over time) in the same pass that
  trims and re-encodes. A bare cut is `--no-overlay`, spelled out, and says in
  capitals what it is making.
- **`tinyctl video` cuts that way,** after every render and on an existing
  render (`--from-webm`), at a crf chosen for the lap's length so the file stays
  under GitHub's 100 MB. `--no-overlay` is the only way to get a bare mp4 out of
  it.
- **The file says so.** The overlay tool stamps the mp4's `comment` tag with
  `tas-overlay v1 ghost=<id> offset_ms=<N> window=-3000/+3000 how=<…>`, and
  **`clip ship` refuses a file without that stamp** — on any box, after any
  copy, with no memory of how the file was made. `clip ship --no-overlay` ships
  a bare clip and says so. A clip published before this rule is replaced by an
  overlaid one, not left alone.

### The timing, and how it is checked

An overlay is a claim: "at this frame the driver was doing this." Two things
settle where the tape sits on the video, and they are different things:

**The offset is a property of the render pipeline, verified by eye once.** The
`tinyctl` render starts the MediaTracker clip at the ghost's race 0 and cuts it
at the finish (`--footage` = the lap), so video time IS race time: offset 0.
That was checked on Tiny Summer 02 the only way that is lag-free: the **front
wheels**. The visual ghost draws the wheels at the tape's steering (interpolated
between the 50 ms samples, no camera spring), so a full-lock flick is visible to
the frame. In 02 the steer goes 0 → full left at race 1.950 (sample 1.900 = 0,
sample 1.950 = −1.0); frames pulled at 1.833 … 2.100 show the wheels straight
at 1.866–1.900, beginning to turn at 1.933, turned at 1.966 — offset 0 to
within a frame. Redo this check when the render path changes (another camera,
another lead-in, another game build), not per clip:

```
ghost tape csv <run.Ghost.Gbx> > tape.csv          # find a 0 → full-lock onset ≥ 100 ms
ffmpeg -i <render.webm> -vf "select='between(t,T-0.13,T+0.17)',crop=480:150:780:600,tile=3x3" -frames:v 1 -vsync 0 wheels.jpg
clip panel <run.Ghost.Gbx> panel.png --at T          # the strip at the same instant, to compare
```

Brake lights are NOT a cue on this car (the livery covers the rear, and the
TAS taps the brake for 10 ms at a time).

**The per-clip check is the picture against the tape, by the tool, every
time.** `clip sync <ghost> <video>` fits the world's sideways motion across the
frame (the camera turns with the car) to the telemetry's yaw rate, over every
lag in ±1.5 s. On a correct clip the fit lands 120–290 ms BEHIND the tape (the
chase camera swings behind the car on a spring: measured on all twenty ship15
renders) with a rank correlation of 0.4–0.85 and a peak a few frames wide.
`clip cut --ghost` runs it and REFUSES when the fit is weak (not the ghost that
was rendered; a static frame with no car), a plateau, or outside the window a
correct clip lands in (a render that started early or late, a clip cut in the
wrong place). The marker records the fit (`how=guard/lag-190ms/r0.73`). What it
cannot do is measure the offset to a frame — a delay and a smoothing look alike
to a correlation, so the spring cannot be separated from a shift smaller than
about 250 ms; that is what the wheel check above is for. `--offset-ms N` on
`clip cut` draws at N and skips the check (marker `how=given`) — for a clip
whose frames have been looked at.
