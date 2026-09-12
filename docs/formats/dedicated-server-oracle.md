# The dedicated server as oracle: `/validatepath`, its output, and what it checks

`TrackmaniaServer` validating a directory of ghosts/replays exactly the way it
validates a submitted record — the ONLY ground truth for "what time does this
file do". Driver: `tools/ghost/src/oracle.rs` (`validate_many`, the parser),
`ghost verify`, `ghost declare --from-oracle`, `tmmaps segments`; mechanics in
memory `tm2020-dedicated-server-validatepath.md` (2026-09-03) and
`OLDBUILD.md`. Fixture: `tools/testdata/oracle_transcript.json`. Confidence
**[VERIFIED-GAME]** throughout (every line is a measured server behaviour).

## 1. Invocation

```text
TrackmaniaServer.exe /nodaemon /validatepath=<DIR>       (under Wine on Linux)
```

* `<DIR>` is a directory name RELATIVE TO `<build>/UserData/Replays/`; an
  absolute path or the path of one file silently yields `Starting validation of
  0 ghosts`.
* It reads only files named `*.Ghost.Gbx` or `*.Replay.Gbx`; anything else is
  skipped with no record — a DNF look-alike (`ghost::oracle::readable_name`
  links every candidate in under a readable name).
* Maps go in `UserData/Maps` and are found by the ghost's uid (`0x03092010`);
  a REPLAY carries its own map and needs none (`ghost verify --empty-maps`
  validates at 7.241 with an empty Maps directory). Two maps with one uid in
  the directory: the server binds whichever it found first. The server
  refuses a map with an uncompressed body (`Can't load map`).
* One process validates a whole directory; startup + cache dominates, so
  batch (~20 per process). Records flush INCREMENTALLY. Concurrency is fine
  with a unique replay dir and `/xmlrpcport` per probe; redirect stdout to a
  FILE (never a pipe) and wrap in `timeout -s KILL`.
* Archives: `http://files.v04.maniaplanet.com/server/TrackmaniaServer_YYYY-MM-DD.zip`
  (`-x http://fwdproxy:8080` from a Meta box); the FULL archive (Packs/ +
  UserData/) is needed to run it. Physics come from the binary + its packs:
  the build stamp inside a ghost is decoration (`tools/strpatch`).

## 2. Output shape (a parser must model all of it)

```json
[
{
  "ValidatedResult" : { "NbCheckpoints" : 4, "NbRespawns" : 0, "Time" : 19538, "Score" : 0 },   ← what it SIMULATED (null on a non-finish)
  "Desc" : "validated time is actually better! (30000 > 19538)\n",
  "IsValid" : false,
  "DeclaredResult" : { "NbCheckpoints" : 5, "NbRespawns" : 0, "Time" : 30000, "Score" : 0 },    ← what the FILE SAYS
  "Inputs" : "5C46_121C34D53C17D99C16D69C7D10C38D5C99D6C10D108C12L186C83E5C55D2C46E12B55E5B176E4C43D5C277E160C9L231C",
  "GameBuild" : "Trackmania date=2026-02-02_17_51 git=128149-c7d05ad2551 GameVersion=3.3.0",
  "AccountId" : "xxxxxxxx-…", "Login" : "xxxxxxxxxxxxxxxxxxxxxx",
  "MapUid" : "buNzfsVlp2NF2oWtHM3729dEylg", "FileName" : "stale_decl.Ghost.Gbx"
}
]
```

* Records print INTERLEAVED with `Validating <file>...` progress lines inside
  the top-level `[ … ]` — the whole is NOT valid JSON: scan for objects
  starting in column 0 and parse each.
* **Two results per file, and the second is the file's own claim.** A parser
  that takes `"Time"` lines as they come reports the declaration as the
  world's answer (measured: a tape simulating 22.738 and declaring 22.730
  read as 22.730). Track the block.
* **A completion = `ValidatedResult` non-null with `Time > 0`.** `IsValid:
  false` does NOT mean no time: `race finished, time is worse. (a < b)` and
  `validated time is actually better!` are completions with a stale
  declaration. Non-finish `Desc`s: `wrong simu` (+ `, but reached some
  checkpoints (n out of m)`), `not finished, with N respawns` (DNF), `using
  unsupported respawns (N)` (a POLICY refusal, never simulated), `using
  known-flawed game exe '<build>'` (same). `Can't load replay: <file>` prints
  with no `Validating` line and no record. On a DNF `NbCheckpoints` never
  appears.
* End-of-run counters (`Wrong Simu :   0% (  0)`, `Can't load : …`) print after
  EVERY run; substring-matching them inverts verdicts — cut the summary off.
* `Inputs` is the server's own compact re-encoding of the tape it drove
  (`<ticks><letter>` runs; the letters seen `C D L E B _`); not decoded here
  — the `0x0309201D` chunk is read directly.
* `GameBuild` is the recording's build string as the server read it from
  `0x0309202D`; `MapUid`, `AccountId`, `Login` from the container. An era
  mismatch (a 2022 recording on the 2026 server) is `wrong simu`; a wrong
  server build for a file can also be a SIGSEGV (rc 139), not a message;
  the 2020-07-01 build prints a FLAT schema (no `ValidatedResult`) and
  segfaults on every replay recorded 2020-07-07+.

## 3. What the server checks, and what it does not

| checks | ignores |
|---|---|
| the input tape re-simulated against the map's physics; the container binding of `0x0309202D` (a tape moved alone → `wrong simu`, `ghost-cgamectnghost.md` §6); the respawn policy; the file extension; the map's body compression and shared lookback-table length | the declared time (echoed, then compared: a stale declaration is a `Desc`, never a rejection); the declared checkpoint COUNT (1, 2, 3, 5 on a 4-split map and 9 on a 3-split map all validated at the right time — `oracle.cps_does_not_gate`); the two hashes `0x0309200E`/`0x0309201C`; the build stamp; the map uid against the map's content; block model names (a renamed block still validates); the header; the `lightmap` version; the telemetry (never read — it PLAYS nothing); identity strings |

* The server starts the car at the waypoint INDEX named in `0x0309202D`
  (`map-validation-ghost.md` §3); the client starts on the Spawn item.
* The server and client agree on physics to the millimetre where measured
  (the 0.48–0.52 mm "floor" was two copies of the car struct in the server's
  memory, `tools/README.md`); a regenerated tape's tick alignment is
  nondeterministic in ~1 of 7 runs, the tape itself exact.
* A respawn on a checkpoint map is a SOFT respawn to the last checkpoint state
  (bit-identical after it).
* Gate specials fire on the server exactly as on the client once written in
  PREFAB form (turbo 01/04/05/08/16, reactor 05/07 — parity measured
  2026-09-08); physics 13 is transparent and 28 solid on BOTH engines.

## 4. Doing arithmetic with it

* `ghost verify` V-rules: V1 the oracle time equals the declared time (V2:
  in EVERY copy, header included), V3 identity clean, V6 tape/record kappa,
  V10 no identity-shaped strings outside the carried map, V11 (warning) the
  non-vehicle record present.
* Segment oracles (`tmmaps segments`): a reference ghost's splits (from
  `0x0309202B`) against the map's checkpoints, promoted one at a time to a
  finish — the finish trigger of `GateFinish32m` is geometrically identical
  to a checkpoint gate's, so a promoted checkpoint reports the declared split
  to the millisecond; a block finish fires ~100–150 ms before a checkpoint
  block in the same cell.
* Era studies (`OLDBUILD.md`): a 2021-05-31 → 2021-06-08 EXECUTABLE-only
  change (physics packs byte-identical) moved 74 % of common completions and
  turned 468 of 727 finishes into `wrong simu`; 2020-07-23 → 2020-09-11 changed
  both packs and moved NO time (2 of 241, max 3 ms) yet flipped 6 runs `wrong
  simu` → finish. Behaviour boundaries are not time boundaries.

## 5. Not known

* The `Inputs` string grammar (the letters' meaning; `_`).
* Which field of `0x0309202D` the binding check reads.
* `Score` (always 0 here).
