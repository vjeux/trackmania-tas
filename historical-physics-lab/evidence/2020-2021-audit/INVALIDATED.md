# INVALIDATED — do not use the nine-class verdict taxonomy

The first 2020–2021 audit's `matrixW_all.tsv` and proposed nine-class taxonomy are retained only as forensic evidence. Its validator parser searched for bare `Time:` / `IsValid:` fields, but real server output is quoted JSON (`"Time" : ...`). When parsing yielded nothing, it fell through to substring matching the end-of-run summary, where `Wrong Simu : 0% ( 0)` also appears after successful runs. Genuine finishes could therefore be classified as `WRONG_SIMU`, invalidating the nine-class verdict interpretation.

The corrected behavior changelog uses only runs that completed on both adjacent builds and compares their returned lap times. Independently repeated results that survive are:

- S-0, bracketed 2020-07-23 → 2020-09-11: 26 of 194 common completions change time; largest −1.954 s.
- S-1, 2021-06-08: 92 of 139 common completions change time; median 0.016 s; largest +1.488 s. This boundary is isolated to executable code because Stadium/shared/title packs are byte-identical.

See `../../BEHAVIOR_CHANGELOG.md`, `../behavior-controls.md`, `../deltas_2020-09-11.tsv`, and `../deltas_2021-06-08.tsv`. Do not generate profiles from `stadium-epochs-2020-2021.profiles.json` without re-deriving them from the corrected parser.
