# Speed Cap — where Trackmania's 1000 km/h limit lives, and how the plugin lifts it

Client `Trackmania.exe` 45 467 720 B, md5 `4a28c00429c6f75c894cf7bc4378a8a2`,
banner `date=2026-01-28_13_00 git=128130-6dda3728e91 GameVersion=3.3.0` (the
build on the render box, the one the Jump Button targets). Addresses are
image addresses (base 0x140000000) as `objdump -d -M intel` prints them; read
with `tools/asmdig` over that listing. Claim tags as in `CLAIMS.md`:
**[DISASSEMBLY]** read off the image, **[RUNTIME]** read from the running game.

## 1. The limit is a model parameter, not a constant in the physics code

**[DISASSEMBLY]** There is no `277.78` (1000/3.6) literal in any data section
of the image (`asmdig consts Trackmania.exe 277.77777` → nothing; the loose
byte pattern `?? E3 8A 43` has exactly two hits, one of them the immediate
below, the other unaligned noise in a resource section). The value enters
the program in one place:

```
14060196b   mov DWORD PTR [r12+0x2f0], 0x438ae38e      ; 277.7778 m/s = 1000 km/h
140601977   mov DWORD PTR [r12+0x2f4], 0x42c80000      ; 100.0
140601983   mov DWORD PTR [r12+0x2f8], 0x3e99999a      ; 0.3
1406019a7   mov DWORD PTR [r12+0x2fc], 0x461c4000      ; 10000.0
```

inside the constructor at `0x140600d20` (vtable `0x141bd3b78`, allocation
size 0x3778) of a **vehicle physics tuning** — one of the dated physics eras
(`20fev2013`, `06/12/2019_TurboAirControl_Ice`, …,
`Wood_20240101_MoreAccelForSlope2`) that the physics model keeps in its
tunings container, the same 28-entry container `HPLTuneDump` walked. The
three neighbours never change and are the signature the plugin checks.

**[DISASSEMBLY]** Which tuning a car uses is decided once, at spawn
(`0x1407cbc9f`, the car-creation function `0x1407cbc70`):

```
1407cbc92   mov  rcx, [r8+0x18]          ; the physics model's tunings container
1407cbc9f   call 0x1405fc250             ; -> container.Data[container.ActiveIndex]
1407cbcab   mov  [rdi+0x88], rax         ; car->Tuning
```

with `0x1405fc250` being nothing but `return [[rcx+0x18] + [rcx+0x2c]*8]`
(data pointer at +0x18, active index at +0x2c — the container layout
HPLTuneDump recorded). **[RUNTIME]** On the render box the active entry is
`tunings[27]` of 28, `Wood_20240101_MoreAccelForSlope2`, its `+0x2f0` holds
`0x438AE38E`, its neighbours `42C80000 3E99999A 461C4000`; all 28 entries
carry the signature, and the physics model nod (class `0x090EA000`) also
caches the active entry's pointer at its `+0x300`.

**[DISASSEMBLY]** The one consumer of `+0x2f0` in the vehicle physics is the
tail of `NSceneVehiclePhy::ComputeForces` (function `0x1408427d0`, named by
its own profiler string at `0x141beea28`):

```
1408429bf   movss xmm7, [rsp+0x58]          ; v.x   (the velocity the step produced)
1408429c5   movss xmm8, [rsp+0x5c]          ; v.y
1408429cf   movss xmm9, [rsp+0x60]          ; v.z
1408429da   mov   r8, [rsp+0x70]            ; tuning  (= [car+0x88], saved at 140842860)
1408429ec   movss xmm6, [r8+0x2f0]          ; tuning->MaxSpeed
...         ; xmm2 = v.x² + v.y² + v.z²,  xmm0 = MaxSpeed²
140842a09   comiss xmm2, xmm0               ; |v|² > MaxSpeed² ?
140842a0c   jbe   0x140842a6b               ; no: keep v            <- bytes 76 5D
140842a0e   comiss xmm6, [0x141d1ed34]      ; MaxSpeed > 1e-5 ?
140842a15   jbe   0x140842a6b
140842a22   sqrtss xmm0, xmm2               ; |v|
140842a30   divss xmm6, xmm0                ; k = MaxSpeed / |v|
140842a3f   mulss xmm7, xmm6                ; v *= k   (x, y, z)
140842a43   mulss xmm8, xmm6
140842a48   mulss xmm9, xmm6
140842a4d   movss [rsp+0x58..0x60], v
140842a61   call  0x140845270               ; write the velocity back to the body
```

So every physics tick ends with `if |v| > MaxSpeed: v *= MaxSpeed/|v|` —
a hard clamp on the magnitude, direction preserved, which is exactly what
"pinned at 999" looks like from the driver's seat. (The speedometer shows
three digits; the physics value is 1000.0 km/h.)

Things that are **not** the cap, checked so nobody re-reads them:

| site | what it is |
|---|---|
| `0x140848ef0` | the replay/network sample quantiser: `min(speed_kmh, 1000)` → u16. Recorded ghost *speed* fields saturate at 1000 km/h; positions do not. |
| `0x140844dac` | gearbox: `1000.0 > [car+0x15d8]` is an RPM threshold |
| `0x1408521d6` | `dt * 1000` (seconds → ms) in the CarSport handler |
| `0x1408532ea` | normalising the velocity direction, no limit involved |

## 2. Reaching the tunings without a hook

The Jump Button obtains the car by hooking the physics handler. The Speed Cap
needs no car: the tunings hang off the CarSport physics model, which the
catalog hands out. The walk (proved on this build by `HPLTuneDump`):

```
GetApp().GlobalCatalog
  → chapter with IdName "Vehicles" (or "#10003")
  → article "CarSport" → Preload() if LoadedNod is null → LoadedNod (CGameItemModel)
  → Dev::GetOffsetNod(item,   0x288)   vehicle model
  → Dev::GetOffsetNod(model,  0x28)    physics model (class 0x090EA000)
  → +0x18 (or +0x20 on the Sep. 30 build) → tunings container, class 0x090EB000 at +0x28
      data +0x18, count +0x20, active index +0x2c
  → every entry Data[i]: +0x2f0 is MaxSpeed when +0x2f4..+0x2fc read
      0x42C80000 0x3E99999A 0x461C4000 bit for bit
```

Before writing, the plugin requires the container's class id and, per entry,
the three neighbours. A build that moves any of it fails those checks and
nothing is written. `carcheck` (a cmd.txt command) additionally compares
`[car+0x88]` for a live car with the active entry, which is how `jumprig
captest` proves the write is read by the physics and not by a copy.

The write itself: `Dev::Write(entry + 0x2f0, value)` for every signed entry
— not just the active one, so a mode that selects another era sees the same
limit. "Unlimited" is `1e6 m/s` (3.6 million km/h) — finite, so `MaxSpeed²`
stays a sane float for ComputeForces and anything else that reads the field.
The plugin re-checks the field at 4 Hz: the game rebuilds these objects
between maps and they come back with the constructor default, and a rebuilt
entry gets the value again within a quarter second. The original value
(persisted to `original.txt`, so a plugin reload that finds our value still
knows what stock was) is written back on disable and unload.

### The one-byte alternative, not used

`0x140842a0c: 76 5D` (`jbe`) → `EB 5D` (`jmp`) skips the clamp for every
vehicle regardless of the tuning. It is the right tool if a future build
stops exposing the parameter; on this build the data write is preferred
because it touches no code, needs no allocation, and makes the limit a
number rather than a switch.

## 3. What changes when the cap is gone

* Boost and downhill sections keep accelerating past 1000 km/h until drag
  wins; nothing else in the vehicle code reads the field.
* Recorded ghosts: the sample quantiser above still saturates the *speed*
  byte pair at 1000 km/h; positions, rotations and inputs are unaffected.
* Anything above the stock limit is client-local: the dedicated server
  re-simulates with its own model and will refuse the time (`wrong simu`).
  This is a local-play plugin, like the Jump Button.

## 4. Proof: `jumprig captest`

The probe is the Jump Button's `setspeed <m/s>`, which rescales the car's
velocity vector in place. ComputeForces clamps an over-limit velocity on the
very next tick, so the magnitude read back ten ticks later (100 ms) says
whether the limit is in force:

```
jumprig captest [--map PATH]
  1. a fresh game (the previous holder's game is stopped), both plugins alive
  2. into a map, car grounded; `probe` dumps car->tuning; the active tuning
     must be found with original = 277.7778 m/s
  3. carcheck: [car+0x88] must be the active tuning (SAME)
  4. stock:     setspeed 300 → must read ≤ 278.5 m/s after 10 ticks
  5. unlimited: setspeed 300 → must read ≥ 285 m/s after 10 ticks
  6. whatever the verdict, the plugin is put back to stock: the box is shared
```

Between phases the run is restarted through the playground API
(`RequestRestartMap`, JumpButton's `restartmap`); a synthesised Backspace
did not bring the car back.

### Result, 2026-09-24 18:58 PT (render box, build 128130, map `_shoot/S2025-01`)

```
probe:     car->tuning=0x0000022BC89BAF20 tunings[27] = the active entry
           name='Wood_20240101_MoreAccelForSlope2' +0x2f0=0x438AE38E (STOCK 1000 km/h)
           dwords +0x2e0..: 3F800000 00000000 3EE66666 00000000 438AE38E 42C80000 3E99999A 461C4000 ...
carcheck:  car->tuning == active  SAME   tuning.MaxSpeed=277.7778 m/s (1000.0 km/h)
stock:     setspeed 300 -> 277.5 m/s (999 km/h) after 10 ticks      <- clamped by ComputeForces
unlimited: setspeed 300 -> 299.5 m/s (1078 km/h) after 10 ticks     <- no clamp; the 0.5 m/s is drag
RESULT: speed cap removed - stock clamps 300 -> 277.5 m/s, unlimited keeps 299.5 m/s (1078 km/h)
```

The plugin wrote 28 of 28 tunings for "unlimited" and put all 28 back to
stock at the end of the run; the box was left with stock physics.
