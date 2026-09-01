# decoder-goldens — what is here, and why the counts differ

```
ghosts/   45 .Ghost.Gbx   the source recordings
csv/      45 .csv         one per ghost      -- regenerable
paths/    51 .json        45 + 6 orphans     -- 45 regenerable, 6 not
reports/  analysis.txt, analysis2.txt        -- Python-era, never reproducible
```

## The 45: decoder goldens, re-blessable

Every `csv/<name>.csv` and 45 of the `paths/<name>.json` have a
`ghosts/<name>.Ghost.Gbx` behind them, so they can be regenerated and
`golden_decode` compares them **byte for byte**:

```
cargo run --release -p tmtraj -- export \
    --dir tools/testdata/decoder-goldens/ghosts \
    --out-csv tools/testdata/decoder-goldens/csv \
    --out-json tools/testdata/decoder-goldens/paths
```

Do that only with a reason, and put the reason in the commit message.

## The 6 orphans: analysis inputs, NOT goldens

`05_19556`, `06_19560`, `07_19560`, `08_19560`, `09_19563`, `10_19563` have
**no source ghost** — the recordings were lost. They keep a `paths/*.json` and
no `csv/*.csv`, which is deliberate:

- As decoder goldens they were worthless. Nothing can re-decode them, so
  nothing could ever check them, and `golden_decode` skipped them anyway
  (it only compares runs whose ghost is present). Their CSVs were deleted for
  exactly that reason.
- As **corpus data they are load-bearing**. `golden_stats.rs` re-derives every
  figure quoted in `reports/analysis.txt` and `analysis2.txt` from all 51
  paths — and as that test's header records, *the ad-hoc code that produced
  those reports was not preserved*. Drop the six and the pair count goes
  1275 → 990, every statistic shifts, and the published analysis becomes
  permanently unverifiable.

**So do not "fix" the 45/51 asymmetry by deleting the six JSONs.** The
asymmetry is the point: a file can be useless as a golden and still be the only
surviving input to a published claim.

If the six ghosts are ever recovered, add them to `ghosts/`, regenerate, and
this note can go.
