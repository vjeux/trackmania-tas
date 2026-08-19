# PLAN — map 191465 `Training - 10 Long` (uid kpOLuGFTMICPkW7gp383PEQ_0A2)

Owner: this agent session, started 2026-08-18 16:34 PT, node 77493.od.fbinfra.net.
Everything durable lives in this directory; /tmp is disposable.

## Target
AT 13080 ms (author `in-.-`, their own validation lap), human WR 13081 ms
(same player), 856 records, AT never beaten. Beat 13080 and hand a human a
technique they can practise.

## State (live)

| tape | validated | notes |
|---|---|---|
| `tapes/TAS_13074_analog.Ghost.Gbx` | **13074 ms** | unconstrained, 284 input events, 128 steer values |
| `tapes/WIP_pad5.Ghost.Gbx` | **13074 ms** | steer in {-127,-64,0,64,127} |
| `tapes/WIP_keyboard.Ghost.Gbx` | **13075 ms** | steer in {-127,0,127} — pure keyboard |
| `tapes/TAS_13080_firstpass.Ghost.Gbx` | 13080 | plain-oracle search, kept as the "before" |
| `human_WR_13081.Ghost.Gbx` | 13081 | seed + known-answer control in every batch |

Three cold validation passes in `VALIDATION.md`, each a fresh oracle process,
each carrying the human WR as the control (13081 every time). sha256 there too.

## Method that worked (details in RESULT.md §2)
1. Plain integer-ms search stalls at 13080 in 9 s — the objective is 1000x too
   coarse (1 ms = 24 cm at the finish speed of 858 km/h).
2. Added a **sub-tick timing plane** to the fork server's in-child state reader:
   crossing of x = 28.9 m interpolated inside the tick, reported in the summary,
   used as a microsecond-scale score (`--plane` in tmsearch, `Eval::plane_x` in
   `shared/pred_core.rs`).
3. Per-worker calibration of the child's tick labelling (it moves by one tick
   between fork servers — 4 of 56 workers disagreed in one run), plus a
   per-worker identity control that aborts the worker if the incumbent does not
   reproduce its oracle time.
4. Every reported number re-validated cold through the plain oracle.

## Remaining work
- [x] acquisition recipe -> ../ACQUISITION.md
- [x] beat the AT (13074, −6 ms)
- [x] why-nobody investigation (drafted, needs the final keyboard tape's numbers)
- [ ] low-input family: simplify the keyboard tape's event count, measure cost
- [ ] tolerance table for the drivable tape (slack per input)
- [ ] RESULT.md + driving guide, RESULTS.md append

## Rebuild-on-a-fresh-node
`bash ~/persistent/private-30d/tm-setup/setup_node.sh`, then the analysis
binaries `u10an` / `u10cand` live in the workspace tarball
`tools/u10-tools.tgz` in this directory (crate `u10an`, drop into
/tmp/tmtas-rs2 and add to the workspace members).
