# `tmauto` — the interface C and D call

Agent A owns three things: **`Verdict`** (what the oracle says and how answers
order), **`Tape` → container** (the only writer of `.Ghost.Gbx`), and the
**no-ghost gate**. This file is the contract; the doc comments in the source are
the detail.

Crate: `tools/tmauto` (in the `tools` cargo workspace). Add
`tmauto = { path = "../tmauto" }`.

---

## 1. The one call you probably want

```rust
use tmauto::oracle;
use tmauto::tape::Input;

// tapes: one Vec<Input> per candidate, tick 0 first.
let evals: Vec<Option<tmauto::Eval>> = oracle::evaluate(
    map_path,      // &Path to the .Map.Gbx
    &tapes,        // &[Vec<Input>]
    4500,          // min_ticks: pad every tape to at least this
    80,            // jobs: parallel server launches
    &workdir,      // scratch dir for the candidate files
)?;
```

It synthesizes a container per tape, pools the dedicated server across the box,
and returns answers **in the same order as `tapes`**.

**`None` is not a DNF.** `None` means the server declined to run that file —
a container fault. `Some(Eval { verdict: Dnf { cps: 0 }, .. })` means the engine
ran your tape and the car collected nothing. They look identical if you collapse
them, and they mean opposite things: a search that scores a refusal as a deep
DNF will optimise toward broken containers.

---

## 2. `Verdict`, and the one rule about it

```rust
pub enum Verdict {
    Finish { ms: u32 },
    Dnf { cps: u32 },
}
```

**Every finisher outranks every non-finisher, by construction.** `Verdict: Ord`
delegates to a `Score` enum whose `Ord` is *derived*, and Rust derives enum
ordering in variant-declaration order with `Dnf` declared first. Within a
finish the time is wrapped in `Reverse`, so **faster compares greater**. Within
a DNF, more checkpoints compares greater.

So `verdicts.iter().max()` is the best one, and `sort()` puts the best last.

There is no `FINISH_BASE` sentinel and no arithmetic on a score. Do not
introduce one: that constant silently corrupted five maps' objectives in this
project's history, and nothing about a number saturating looks like a bug.

```rust
assert!(Verdict::finish(999_999) > Verdict::Dnf { cps: 250 });
assert!(Verdict::finish(43_079)  > Verdict::finish(43_080));
```

`Eval` is a `Verdict` plus provenance:

```rust
pub struct Eval { pub verdict: Verdict, pub source: OracleSource }

pub enum OracleSource {
    Plain,
    Fork { boundary: u32, reference_hash: TapeHash, distance: ForkDistance },
}
```

`source.is_bankable()` is true only for `Plain`. **A result is a written file
the plain oracle re-simulates** — a fork answer is a search signal, never a
result. `ForkDistance::is_forward_only(boundary)` is the check that a fork
answer is inside its regime.

---

## 3. Writing one container

```rust
let bytes = tmauto::synth::write_for(map_path, &inputs, 4500, out_path)?;
```

or, for full control,
`tmauto::synth::synthesize(&inputs, &meta, &ChunkSet::ALL)` with
`meta = tmauto::synth::meta_for_map(map_path)?`.

CLI: `tmauto synth write --map MAP.Map.Gbx --out F.Ghost.Gbx --ticks N`.

### Pad your tapes, and know why

**The validator only simulates while the input archive lasts.** A tape shorter
than the run it is trying to produce stops early and the server reports a DNF —
indistinguishable in the verdict from a car that drove off the track. So
`min_ticks` is not a detail: set it past the longest run you could plausibly
produce on that map. `synth::pad_to` repeats the **last** input rather than a
neutral one, deliberately: a neutral pad lifts the throttle before the line on
any tape whose length was underestimated.

---

## 4. Throughput, measured on A's box (80 cpus)

| what | value |
|---|---|
| server launch | **2.68 s** |
| marginal eval, 2000-tick tape | **~4.3 ms** |
| best measured | **2519 evals/s** (jobs 80, 1200 per launch, n = 96 000) |

Cost is startup-dominated, so **the single thing that matters is how many
candidates you hand over per call.** Batch as hard as you can:

| candidates in one `evaluate` call | evals/s |
|---|---|
| 600 | 173 |
| 4 800 | 845 |
| 24 000 | 1 724 |
| 48 000 | 2 192 |
| 96 000 | 2 519 |

Tape length scales the marginal part only — at 24 000 candidates: 5 s tapes
2208/s, 20 s tapes 1819/s, 45 s tapes 1500/s.

**The inherited constant was wrong.** The earlier rig recorded that throughput
"peaks at roughly 30 candidates per invocation and collapses well beyond that".
It does not collapse here; 30 per launch is the slowest setting measured.
`DEFAULT_PER_LAUNCH` is now 600. Re-measure with `tmauto bench` on any new box
rather than trusting either number.

Caveat, stated because it bounds the claim: every candidate in these runs was a
**DNF**, so the engine simulated each tape to its end. A finishing run stops at
the line, so these figures should be an upper bound on cost, not a best case —
but that is an inference, not a measurement, and it will be re-measured the
first time C produces finishes.

---

## 5. The gate

The oracle driver refuses to load any input file that is not chain-rooted at a
container this system synthesized, **fails closed, and logs the refusal**.
`oracle::validate_gated` is the gated path; `oracle::validate_raw` is not gated
and exists for A's own rung-0 probes.

Every tape you write carries a `PROV` record — producing component, parent tape
hash, seed, timestamp, map uid — and a tape is *chain-rooted* iff following
those parents terminates at a `Producer::Synthesizer` record with no parent.
Register what you write:

```rust
ledger.record_tape(tape.hash(), &tape.prov)?;
ledger.record_file(&sha256_hex(&bytes), tape.hash(), &path)?;
```

A record that claims to be a root while naming a parent is refused rather than
interpreted, an unreadable ledger refuses everything, and the gate hashes the
file it was handed rather than believing anything its caller says about it.

---

## 6. Things that will bite you

* **The server only reads files ending `.Ghost.Gbx` or `.Replay.Gbx`.** A
  candidate under any other name is not read, and the result is
  indistinguishable from a run that did not finish.
* **`wrong simu` with nothing appended is a DNF with zero checkpoints**, not a
  refusal — it is written at the branch right beside `wrong simu, but reached
  some checkpoints (N out of M)`. A refusal appends its reason; `Answer::simulated()`
  separates the two by an explicit list of the seven reasons observed.
* **The server prints two results per file and the second is the file's own
  claim.** `ValidatedResult` is what it simulated; `DeclaredResult` is what the
  file says. Never quote one without its label.
* A time of `4294967295` is the "never crossed the line" sentinel, not a finish.
