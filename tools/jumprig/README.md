# Jump Button

An Openplanet plugin that gives the Trackmania car a jump, by hooking the
physics handler and adding vertical velocity to the live car state.

Press **Space** while driving. Measured: the car rises **1.9 m** with 0.43 s of
airtime at the default strength.

## What it actually does

Trackmania's scriptable API cannot reach the physics vehicle: Openplanet's
class dump has no `CSceneVehicleVis*`, so there is nothing to write a velocity
to from AngelScript. The car has to be found in memory, and that means a
native hook.

The plugin patches 12 bytes at the entry of the CarSport physics handler with
an absolute jump into a small allocated island. The island:

1. copies the car pointer out of `[r8+8]` into its own storage,
2. increments a tick counter (proof the handler is really being called),
3. runs the displaced prologue it replaced,
4. jumps back to the instruction after the patch.

The hook only *observes* — it never alters physics. The jump itself is an
ordinary write to the car's vertical velocity from AngelScript, using the
pointer the hook captured.

## Safety

This writes to another process's code, so every step is gated:

* **Exact-build gate.** The plugin refuses to patch anything unless both the
  41-byte handler signature and the build banner
  (`git=128130-6dda3728e91 GameVersion=3.3.0`) match. On any other build it
  loads, reports why it is idle, and writes nothing.
* **Preimage check, then readback.** The entry must contain exactly the bytes
  we expect before patching, and must contain exactly our jump afterwards.
  Either check failing restores the original bytes.
* **The island is never freed.** See the long comment on `RemoveHook`: the
  physics handler runs on a game thread, and freeing the trampoline while a
  thread is inside it is a use-after-free. It killed the game on every plugin
  hot-reload until this was understood. 0x80 bytes leak per load — a few
  hundred bytes a session — and in exchange a stale patch is always safe,
  because the code it points at stays mapped and still does the right thing.
* **Teardown restores the entry** on `OnDisabled` / `OnDestroyed`.

## Settings

| Setting | Default | Notes |
|---|---|---|
| Jump key | Space | |
| Jump strength | 10.0 | see the table below |
| Cooldown | 0.35 s | shorter than the 0.43 s airtime, so jumps cannot stack |
| Require ground | on | at least one wheel in contact; off lets you fly |
| Add to vertical | on | adds to current vertical velocity rather than replacing it |

Peak height against strength, measured on build 128130:

| strength | 4 | 6 | 8 | 10 | 12 | 15 |
|---|---|---|---|---|---|---|
| gain (m) | 0.35 | 0.73 | 1.25 | 1.89 | 2.66 | 4.02 |

10 is the default because 1.89 m clears a car and a low wall: visibly a jump,
without turning the car into an aircraft or making existing tracks trivial.

## Automation

With `S_Automation` on (default) the plugin publishes `state.json` at 20 Hz and
reads `cmd.txt`, both in its PluginStorage folder. That is what `jumprig`
drives, and it is how the numbers above were measured.

`state.json` carries the heartbeat, whether the build is supported and the hook
installed, the physics tick count, whether a playground and a valid car exist,
position, velocity, wheels in contact, jump count, and the last command result.

Both files are opened defensively: a reader and the plugin will collide, and on
Windows that throws rather than tearing. A failed open is skipped, not raised —
writing per frame and treating a collision as an error produced an exception
every frame, which is what destabilised the game before this was fixed.

### jumprig

`jumprig` is the harness. Its only synchronisation primitive is
`wait_for(event, timeout)`; the single poll tick inside it is the only sleep in
the system. Events: `process`, `exit`, `lock-free`, `alive`, `hooked`,
`in-map`, `car`, `ticking`, `grounded`, `airborne`, `apex`, `landed`.

A jump is verified as a sequence of real events — settle on the ground, jump,
apex (upward motion stops), landing (wheel contact regained) — not by sampling
for a fixed window.

Everything that drives the game goes through the `tmdrive` lock. One game, one
driver.

## Why runjump.sh retries

The box is shared. Another session can legitimately hold it, and the jump run
is refused with the holder's name and purpose — that is the `tmdrive` lock
working, not a fault.

It retries for a second reason too, learned the hard way on 2026-09-23: before
every driver was behind the lock, a concurrent session running a bisect was
killing and relaunching the game underneath this one. Clean exits, no crash
dump, no Windows error record — indistinguishable from a flaky game, and it
was diagnosed as exactly that until the lock named the culprit. With the lock
in place that cannot happen; the retry remains for the honest cases (a launch
that stalls in Openplanet's Nadeo login, which `tmdrive` also restarts, and a
`PlayMap` that reports success and loads nothing).
