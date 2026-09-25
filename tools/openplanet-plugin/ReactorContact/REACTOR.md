# Reactor contact — where Trackmania keeps it, and how a plugin reads it

For AR's [tm_Reactor-Duration](https://github.com/st-AR-gazer/tm_Reactor-Duration):
a direct, read-only "the car is touching a reactor-granting surface right now",
instead of guessing it from block positions. Everything below was read off the
shipped client and then checked against a running game; nothing is inferred
from a Cheat Engine pointer chain.

Client: **Trackmania 2026-01-28, git 128130, GameVersion 3.3.0** (Steam,
`Trackmania.exe` md5 `4a28c00429c6f75c894cf7bc4378a8a2`), Openplanet 1.29.x.
Image addresses are for base `0x140000000`; `Trackmania.exe+X` is the RVA.

## 1. The state, in one picture

The reactor state lives on the **car physics object** — the per-vehicle object
`NSceneVehiclePhy` steps every 10 ms (the one whose `+0x88` holds the active
physics tuning, the same object the SpeedCap work used). Byte offsets on this
build:

```
car+0x13A8  u32  ReactorBoostType   1 = up, 2 = down, 0 = no boost active
car+0x13AC  u32  ReactorBoostLvl    1 = ReactorBoost (yellow), 2 = ReactorBoost2 (red), 0 = none
car+0x13B0  u32  last-contact time  GameTime (ms) of the LAST physics tick the car touched a
                                    reactor surface. Rewritten EVERY tick while touching.  <-- the field
car+0x13B4  u32  activation time    GameTime of the tick that started the current/last boost
car+0x13B8  u32  force-ramp start   GameTime the wheel force started applying; 0 while it is not
car+0x13BC  u32  duration           6000 (tuning.ReactorDuration / coef); the boost ends at +0x13B0 + +0x13BC
car+0x1AFC  f32  coef               1.0 normally (a duration divider the mode can lower)
car+0x1C60  u64  touched shape      handle of the shape the car touched, 0 once the boost is gone
```

This is exactly the 2023 layout AR and XertroV found (`contact` at
`duration − 0xC`, `activation` at `−0x8`, a "start" that keeps going back to 0
at `−0x4`, `70 17 00 00` for the duration), 0x13A8 bytes into the object.

The fields are NOT a countdown: leaving the surface freezes `+0x13B0`; expiry
zeroes only `+0x13A8`, `+0x13AC` and `+0x13B8`; `+0x13B0`/`+0x13B4`/`+0x13BC`
keep their last values (so 6000 stays readable after the boost is over). A
fresh car object (map start / restart) has all six at 0.

`GameTime` here is the playground clock a plugin reads as
`GetApp().Network.PlaygroundClientScriptAPI.GameTime`: measured live, a render
frame sees `GameTime − car[+0x13B0]` in 0..9 ms while touching (the frame runs
0..1 tick after the stamp).

## 2. The code that writes it

`PhysicsStep_TM` (profiler string; `Trackmania.exe+0x1101800`) runs the
per-car gameplay step `+0x43df50` once per tick with `r9d = time_ms`; that step
calls the **reactor update `+0x43d5b0`** (guarded by `tuning+0x3278 != 0`, the
tuning's reactor-enabled flag):

```
// Trackmania.exe+0x43d5b0 (rdx = car, r8d = now, xmm3 = dt)
touching = query(car, material 12) || query(car, 18)   // ReactorBoost, ReactorBoost_Oriented  -> lvl 1
        || query(car, material 14) || query(car, 19);  // ReactorBoost2, ReactorBoost2_Oriented -> lvl 2
if (!inhibited(car, now) && touching) {
    car+0x13A8 = (surface normal z <= 0) ? 2 : 1;      // type (0x14083d768..77a)
    car+0x13B0 = now;                                  // 0x14083d787  <-- every tick in contact
    car+0x13BC = trunc(tuning+0x3290 / coef);          // 0x14083d79b  (6000 / 1.0)
    if (old type == 0) car+0x13B4 = now;               // 0x14083d7a5  activation
    car+0x1C60 = touched shape;                        // 0x14083d7af
    car+0x13AC = (12 || 18 hit) ? 1 : 2;               // 0x14083d7c9  level
}
end = car+0x13B0 + car+0x13BC;
if (now + 400 >= end && now - dt_ms + 400 < end) *finalWindowFlag = 1;   // the "final timer" starts
if (now > end && car+0x13A8 != 0) reset(car);          // +0x43d580: zero 0x13A8/0x13AC/0x13B8 + 4 wheel words
```

The contact query is `+0x43b410(car, …, materialId, out dir, out shape, out
flag)`; the ids are the game's gameplay-material ids (12 ReactorBoost, 14
ReactorBoost2, 18 ReactorBoost_Oriented, 19 ReactorBoost2_Oriented — pads,
rings and gates all resolve to these four), so "touching" means the car's
contact solver reports a shape carrying one of them this tick. Rings and gates
use the same query as pads; the only difference is the material id.

`+0x13B8` is written by the per-wheel force routine `+0x43b820`: set to `now`
when the wheel force starts, zeroed whenever the force condition drops — the
value XertroV saw "become 0" on re-contact.

The vis state Openplanet exposes is *derived* from these fields once per tick
by `+0x3d1380`: `CSceneVehicleVisState+0x174 ReactorBoostLvl`,
`+0x178 ReactorBoostType`, `+0x17C` = the float VehicleState calls
`ReactorFinalTimer` = `1 − (end − now) / tuning+0x3638` in the last second.
None of the three says whether the car is touching a surface now — only
`car+0x13B0` does.

The 6000 default is written by the tuning constructor (`+0x200510`,
`tuning+0x3290 = 0x1770`, inside the reactor sub-struct at `tuning+0x3278`);
the live active tuning carries it.

## 3. Reaching the car from Openplanet

Openplanet has no handle to `NSceneVehiclePhy` objects, but the **CSmPlayer**
does, and its reflected `Score` member is a usable anchor (the highest-offset
named member of the class — `0x1068` on this build; the 2023 hint
`GetOffset(player, "Score") − (0xE60 − 0xC80)` used the same idea with the
offsets of that year):

```
CSmPlayer + Score.Offset + 0xB0        (0x1118)   4 vehicle slots, one qword each, 0x10 apart:
                                                   slot k -> vehicle object k (contiguous, stride 0x1E08 = the object size)
CSmPlayer + Score.Offset - 0x228       (0xE40)    pointer INTO the active car: car + 0x12C8
car + 0x88                                        -> physics tuning:  +0x2F4 = 100.0, +0x2F8 = 0.3, +0x2FC = 10000.0
                                                     (the MaxSpeed neighbours) and +0x3290 = 6000
car + 0x12F0   vec3                               the car position, == CSmScriptPlayer.Position / vis Position
car + 0x1A0    ptr                                -> the car's CSceneVehicleVis (entity id at its +0)
```

Both anchors were measured with the physics handler's own `car` argument (a
native hook) as ground truth, on two different car objects, and agree.

Fail-closed resolution, as implemented in `ReactorContact/Main.as`:

1. `score = Reflection::TypeOf(player).GetMember("Score").Offset` (null → stop).
2. `sp = GetOffsetUint64(player, score − 0x228)`; `car = sp − 0x12C8`.
3. `car` must equal one of the four slots at `score + 0xB0 + 8k`.
4. `[car+0x88]` must carry the tuning signature and `6000` at `+0x3290`.
5. `[car+0x12F0]` must be within 2 m of `player.ScriptAPI.Position` (when spawned).
6. Re-check every second; on any failure every export returns false/0.

Then `touching = contact != 0 && 0 <= GameTime − contact <= 20 ms`
(`contact = ReadUInt32(car + 0x13B0)`), or, without a clock, "`contact`
changed since the previous frame".

The offsets `0xB0` / `−0x228` / `0x13B0` are build-specific like every raw
offset; the checks in 3–5 are what make a wrong build fail loudly instead of
reading somebody else's dword. When Nadeo ships a new build, re-derive with
`jumprig reactortest` (below) — the hook hands the truth over in minutes.

## 4. Live evidence (2026-09-25, `jumprig reactortest`; box runs /home/vjeux/reactor/run-1790369667, -1790369966, -1790370383 and the final one named in the commit)

Purpose-built straight (`tmmaps straight … --specials`): two
`RoadTechSpecialBoost` pads at z 192..256, a `RoadTechSpecialBoost2` pad at
z 320..352, ring gates further on. The Reactor Probe plugin logged the six
words every frame next to `GameTime`, the vis state and the position.

* Over the pads: ONE episode of 108 frames in which `car+0x13B0` moved on every
  frame, `GameTime − contact` ≤ 9 ms throughout, first move at z 190, last at
  z 257 (the pad ends at 256; the position is the body centre) — then frozen
  for the rest of the run. Zero frames with a moving word anywhere else.
* `car+0x13BC` = 6000 from the first touch on; `+0x13A8/+0x13AC` = 1/1 during
  the boost, 0 exactly at `contact + 6000`; the vis `ReactorBoostLvl` followed;
  the final-timer float rose 0→1 over the last second.
* Boost2 pad: the first tick set level 2 (`+0x13AC` 1→2, `+0x13B0` refreshed),
  then the level-2 reactor lifted the car ~0.3 m and it hovered along the pad —
  no surface contact, no refresh. The field tells the truth; the car was not
  touching.
* Ring gate (`GateSpecialBoost`, material 18 ReactorBoost_Oriented): the hoop
  registered exactly like a pad — `+0x13AC` 2→1 (the ring's own level wins),
  `+0x13B0` refreshed on the 4–5 ticks the car took to cross the trigger disc
  at ~90 m/s (game 24032..24072 in drive A, 55594..55621 in drive C), frozen
  again the tick after, `GameTime − contact` ≤ 8 ms. Same code path, same
  words, only the material id differs. (A stock hoop stands on the cell floor
  with its hole ~2.4 m up — driven into on a road or platform it is a wall; the
  probe map sinks it 2 m as a free block so a car passes through.)
* Map restart (`RequestRestartMap`): the car object comes back at all-zero
  (sometimes the same allocation, sometimes a new one), same behaviour, same
  CSmPlayer offsets; the hook-free resolver picked the pointer the hook
  reported, both times.
* The Reactor Contact plugin's own `IsTouching` ran beside the probe in the
  same drives: touching exactly over the pads and the ring, not touching
  everywhere else, resolved to the hook's car on every sample.

## 5. Files

* `tools/openplanet-plugin/ReactorContact/` — the read-only plugin: window,
  `Export.as` (`IsTouching`, `LastContactTime`, `ActivationTime`, `Duration`,
  `Level`, `Type`, `IsResolved`, `Status`). Port the resolver + one
  `ReadUInt32` into tm_Reactor-Duration, or depend on the plugin.
* `tools/openplanet-plugin/ReactorProbe/` — the probe (per-frame log,
  CSmPlayer/vis/car cross-scan, signature scan, member dumps).
* `tools/jumprig/src/reactortest.rs` — the harness: fresh game, probe map,
  hook pointer vs the resolver, drives, restart, verdict.
* `tools/tmmaps/src/cmd/straight.rs --specials` — the probe map.
* Static analysis scratch on the box: `/home/vjeux/speedcap/tm.asm` (full
  objdump), `fn_14083d780.txt` (the reactor update), `fn_14083d580.txt`
  (reset), `fn_14083bb83.txt` (wheel force), `fn_1407d1380.txt` (vis builder).

## 6. Gotchas met on the way

* `CSceneVehicleVis` is not a `CMwNod`: its first qword is the entity id, not a
  vtable. `Reflection::TypeOf(vis)` dereferences it and **crashes the game**.
  Only ever `Dev::GetOffset*` on it.
* The car objects are 8-byte aligned, not 16; a 16-byte alignment filter in a
  pointer scan skips the real one.
* `Dev::SafeRead*` throw on unmapped memory (catch them); plain `Dev::Read*`
  crash. A rebuilt playground leaves the old car's bytes readable, so
  "still carries the signature" is not "still alive" — re-resolve from the
  player instead.
* A plugin that fails to compile at game start is never hot-reloaded by a file
  change; `Meta::ReloadPlugin(Meta::GetPluginFromID(id))` from another plugin
  recompiles it in place.
