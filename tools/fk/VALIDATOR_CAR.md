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
- **Cross-map:** the same callback and field path resolved on
  `tools/testdata/map2.Map.Gbx` with `human_23013.Ghost.Gbx`: 1,207 rows,
  0.172 m/s median velocity residual, `1.24e-7` quaternion error, and no clock
  gaps. See `cross-map-map2.txt`.
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

## Container initialization finding and fix

The typed path exposed a second defect rather than hiding it. On the synthetic
container, the validator-owned car was near waypoint index 0 / CP3 and ignored a
64 m `RoadTechStart` relocation. Packet modes were not the cause: both the
recorded and synthetic Summer 01 input archives contain only mode 2 packets.
The structural difference is that the recorded file has 24 skippable chunks,
including an 8,975-byte `0x03092000` entity-record chunk, while the synthetic
file has five chunks and no entity record.

The explorer now accepts the recorded file only as an opaque
`ContainerTemplate`. Before either the fork workers or policy receive it,
`ContainerTemplate::prepare` replaces every input-bearing packet with an
independently generated `GeneratedTape`, clears mouse/trigger/respawn inputs,
writes the sanitized container, and requires an exact decode-back match. The
original inputs and trajectory are not exposed by this API.

On a fresh one-worker run with the recorded container extended to 120.010 s:

- first authoritative sample `(1584.0, 18.0, 801.6)` versus semantic spawn
  `(1584.0, 16.0, 784.0)`: 17.7 m, inside the 40 m startup tolerance;
- generated full-throttle control reached 185.2 m from the real start over 340
  states;
- structural check: 0.1936 m/s median velocity residual and `1.19e-7` maximum
  quaternion norm error;
- neutral and full-throttle files produced different server input echoes;
- a fresh plain oracle re-simulated the sanitized file as DNF, while an
  independent `fk trace` of those same generated inputs produced 1,943 rows,
  619.6 m of path, and moved from `(1583.99, 10.37, 827.80)` to
  `(1124.69, 10.01, 1018.22)`.

See `recorded-container-start-control.txt` for the startup measurements and
packet comparison; `plain-oracle-generated-container.txt` and
`generated-container-trace.*` are the independent replay evidence.

## Summer 2026 - 01 acceptance result

A six-minute, 16-worker run from the semantic start evaluated 20,624 candidates.
The plain oracle confirmed a **71.196 s finish**; a finish necessarily traverses
all three checkpoints, so CP1 and the full course were reached. A second plain
oracle invocation reproduced 71.196 twice from the self-contained tape at frame
307. An independent `fk trace` then read 6,943 validator-owned states over
2.380..71.800 s and measured a 1,991.0 m path. The regenerated final ghost
passes every `ghost verify` gate, including exact tape/telemetry agreement
(kappa 1.000) and a fresh 71.196 oracle replay.

Banked result:

- `summer01-finish-71.196.tape.tsv` — generated inputs plus `# frame 307`
- `summer01-finish-71.196-regenerated.Ghost.Gbx` — SHA-256
  `b06fbe7dd54764e19b1ef9213bc6950e0b755ad639f8f5b831cf5e48aa3a9a5a`

## Evidence

- `evidence/validator-car-128182/static-disassembly.txt`
- `evidence/validator-car-128182/full-chain.gdb.txt`
- `evidence/validator-car-128182/hard-left.gdb.txt`
- `evidence/validator-car-128182/hard-right.gdb.txt`
- `evidence/validator-car-128182/hard-left-start-west-64m.gdb.txt`
- `evidence/validator-car-128182/input-fault-hook.log`
- `evidence/validator-car-128182/validator-string-xrefs.tsv`
- `evidence/validator-car-128182/cross-map-map2.txt`
- `evidence/validator-car-128182/recorded-container-start-control.txt`
- `evidence/validator-car-128182/opaque-container-live-run.txt`
- `evidence/validator-car-128182/plain-oracle-generated-container.txt`
- `evidence/validator-car-128182/generated-container-trace.log`
- `evidence/validator-car-128182/generated-container-trace.csv`
- `evidence/validator-car-128182/generated-tape-confirm.log`
- `evidence/validator-car-128182/finish-search.log`
- `evidence/validator-car-128182/finish-trace.log`
- `evidence/validator-car-128182/finish-trace.csv`
- `evidence/validator-car-128182/finish-verify.log`
