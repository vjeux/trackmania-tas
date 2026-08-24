# Validator-owned car state (server build 128182)

This is the production identity path for the headless `/validatepath` simulator.
It replaces the old writable-memory sweep, which could select an unrelated
coherent state record. Geometry, speed, quaternion quality, and agreement with a
recording are guards only; none chooses the object.

Binary: `TrackmaniaServer`, SHA-256
`3b6107824feea402bd32c888e994dcd6df4a10afc8676a2647ac9560ccf0d779`,
`date=2026-05-15_18_00 git=128182-0de74ece09e`.

## Pointer provenance

| hop | static evidence | meaning | Summer 2026 - 01 example |
|---|---|---|---|
| validator entry | `0x113ade4: call [validator.vtable+0x238]`; target `0x1182b40` | `/validatepath` invokes the validation job for one loaded ghost | job `0x5555584e2180`, item count 1 |
| validation job → simulation | callback `0x118c170`, arguments `rdi=controller`, `rcx=sim`; first instruction sequence stores `rcx` at `[rdi+0x1a70]` | binds the simulation object owned by this validation | sim `0x55555c5b95f0` |
| simulation → playground | `0x1218e3d: mov rax,[r14+0x18]`, with `r14` the simulation callback argument | playground updated for each validator tick | stable field `+0x18` |
| playground → participants | `0x1218e41/0x1218e4e` load `[playground+0x660]` and `[playground+0x668]` | player array and count | count exactly 1 |
| participant → vehicle | `0x11a9b16..0x11a9b21` stores the class id and pointer after the `CGameVehiclePhy` class test | primary controlled vehicle | class `0x032e2000`, pointer at `+0x1118` |
| vehicle → state | state update/read sites around `0x11f38fe..0x11f3919` and `0x9cdb14` | quaternion, world position, velocity | q `+0x12e0`, pos `+0x12f0`, vel `+0x12fc` |

The runtime path is therefore:

```text
captured validator controller
  +0x1a70 -> validation simulation
  +0x18   -> CGameCtnPlayground
  +0x660  -> sole-player pointer array   (+0x668 count, required == 1)
  [0]     -> validation participant
  +0x1110 = 0x032e2000                   (CGameVehiclePhy class id, required)
  +0x1118 -> primary CGameVehiclePhy
  +0x12e0 -> quaternion (w,x,y,z)
  +0x12f0 -> world position (x,y,z)
  +0x12fc -> velocity (x,y,z)
```

The shim installs a one-shot, byte-checked breakpoint on callback `0x118c170`
before `main`. It captures `rdi` and `rcx`, restores the original `push rbp`,
and re-executes it. The handshake exports both pointers. The Rust resolver
requires the captured simulation pointer to equal `[controller+0x1a70]`, then
follows the remaining fields. Every null, unreadable, class/count mismatch, or
structural state failure is fatal. There is no scan or fallback.

## Independent controls run so far

- **Fresh processes / ASLR:** the resolver succeeded in five fresh servers. Heap
  addresses differed; the fixed field path did not. See `full-chain.gdb.txt` and
  the `VALIDATOR CAR` lines in the one-worker integration run.
- **Input mirror:** identical synthetic containers with 3,000 ticks of hard-left
  and hard-right produced opposite signed X responses through this exact path:
  `1349.411` vs `1370.566`, with opposite yaw quaternion signs. See
  `hard-left.gdb.txt` and `hard-right.gdb.txt`.
- **Broken hop:** unit tests perturb `participant_vehicle` by 8 bytes and require
  an unreadable-hop error. Separate tests reject a callback/object disagreement
  and a non-`CGameVehiclePhy` class id.
- **Decoded input provenance:** the existing page-fault boundary probe now logs
  the faulting RIP/registers/frame chain. The fault is at module `+0x119f169`,
  the exact 32-byte input-record load, called from validator simulation
  `+0x1219f89`. See `input-fault-hook.log`.

## Container initialization finding

The typed path exposed a second defect rather than hiding it. On the current
synthetic container, the validator-owned car is near waypoint index 0 / CP3,
not the semantic `RoadTechStart`:

- hard-left: `(1349.411, 10.024, 1098.210)`
- hard-right: `(1370.566, 10.025, 1098.214)`
- moving only `RoadTechStart` 64 m west leaves the left state unchanged at
  `(1349.411, 10.024, 1098.212)`

The opposite signed response proves the object consumes our input, while the
moved-start control proves the synthesized container supplies its initial state
independently of the map start. This is not a locator ambiguity. Search remains
fail-closed at the start-position control until a game-recorded container is
used as opaque structure and its entire input archive is replaced.

## Evidence

- `evidence/validator-car-128182/static-disassembly.txt`
- `evidence/validator-car-128182/full-chain.gdb.txt`
- `evidence/validator-car-128182/hard-left.gdb.txt`
- `evidence/validator-car-128182/hard-right.gdb.txt`
- `evidence/validator-car-128182/hard-left-start-west-64m.gdb.txt`
- `evidence/validator-car-128182/input-fault-hook.log`
- `evidence/validator-car-128182/validator-string-xrefs.tsv`
