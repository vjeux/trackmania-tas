# RESULTS — map 186935, v2: the low-input half, and a slightly better time

**Supersedes `RESULTS_186935_v1.md` in §1 (the best time) and §7 (the low-input
family) only. Everything else in v1 stands and v1 is not withdrawn** — its §4a
(packet cuts have no phase), §6 (the human deliverable) and §8/8a (why
cross-run splicing is blocked, and why that negative is void) are unchanged.

Times in seconds with a decimal.

---

## 1. Best validated time: 793.893

| | time | input events |
|---|---|---|
| author time | **2540.641** | — |
| best human (keby) | 2575.154 | — |
| v1 result, the pure retry cut | 795.034 | 20365 |
| **v2, after event minimisation** | **793.893** | **16397** |
| inside the author time by | **1746.748 (−68.8 %)** | |

`LOWINPUT_186935_ev16396_793893.Ghost.Gbx`, md5
`46d4c20efce76d573ca1255369421e4e`. Validated through the plain oracle on the
untouched map, in a batch with the untouched human record (2575154) and the v1
tape (795034) as known-answer controls.

Note the direction: minimising *input events* also made the run **1.141 s
faster**. Removing an input change is not a null edit on a magnet trial — it
lets the car hold a line it was being steered off.

## 2. MANDATORY FOLLOW-UP 2 — the low-input family, with the enumeration

### 2a. Event count — `--mode ddmin`, stopped short of 1-minimal

```
start   20365 events   795.034
end     16397 events   793.893      -19.5 % of the events, -1.141 s
```

**This is NOT a 1-minimal result and must not be quoted as one.** The run was
stopped after 60 rounds at block size 9 (the schedule still had 4, 2 and 1 to
go); the log `analysis/ddmin.txt` is the enumeration. It was stopped for two
reasons worth recording:

* the remaining rounds are ~4 000 / ~8 000 / ~16 000 evaluations of a
  79 656-tick tape and would have run for hours;
* rounds 28–30 overlapped a `/dev/shm` exhaustion (see 2c), so a few blocks in
  that window may have been recorded as non-removable when the write failed
  rather than the car. That biases the count **upwards**, never down — the
  delivered tape is real, because it was re-validated independently through the
  oracle afterwards.

So the honest claim is: **16397 events suffice**, and fewer probably do.

### 2b. Deletion probes — this tape is NOT rigid

| k | positions probed | stride | in budget | over budget | dead |
|---|---|---|---|---|---|
| 1 | 1019 | 20 | **235 (23 %)** | 0 | 784 |
| 3 | 679 | 30 | **121 (18 %)** | 0 | 558 |

Budget = the author time. Survivors overwhelmingly return **795034 exactly**
(a few 795037).

This **contradicts the generalisation in
`TRIAL-CUTTING-RULES-sidecar-erased-time-v1.md`** ("once the retries are cut,
expect no give" — 286279 measured 1 of 45). It is not a contradiction of that
*measurement*, it is a limit on its scope: 286279's cut tape is a fast, chaotic
turtle run, while this one keeps 32 respawn freezes (~0.85 s each, inputs inert)
and a great deal of walking-pace magnet climbing where a single 10 ms event is
not load-bearing. **Rigidity after a cut is a property of the tape's driving,
not of cut tapes.**

The zero column is still there, though, and it is the same finding as 286279's:
**0 candidates finished over budget at either k.** A deletion here either costs
nothing or kills the car. There is no "slower but alive" region.

### 2c. Value alphabet — conversion fails, and the identity control proves the tool

Snapping the whole cut tape's steer axis onto an N-level ladder (`mq`, in
`tools/`):

| ladder | ticks changed | result |
|---|---|---|
| 3 levels (−127, 0, 127) | 21405 | **DNF cps 1** |
| 5 levels | 21260 | **DNF cps 1** |
| 9 levels | 20916 | **DNF cps 1** |
| 17 levels | 20334 | **DNF cps 1** |
| **255 (identity control)** | **0** | **795034 — exact** |

Converting a finished analog tape does not work here either; that is now five
maps. The identity rung is what makes the four failures worth reporting: the
quantiser, the writer and the container are all proven able to say yes.

**Searching UNDER the constraint was attempted and did not run** — see 2c(i).
So the alphabet half of this map's low-input question is **open**, and I am not
reporting a negative on it.

#### 2c(i) HAZARD — `tmsimp --mode kbx` filled a shared box's `/dev/shm` with 123 GB

`kbx` on a 2 638-tick window of this 79 656-tick tape wrote **123 GB into
`/dev/shm`** in about 13 minutes and died on `StorageFull`, taking the tmpfs of
a shared 176-core box to 100 % (it does not appear to reap its per-candidate
files between rounds). My own root, cleaned immediately, and no other agent's
run was killed — but on a busier box this would have taken other people's work
down with it, and the failure mode of a full `/dev/shm` is a spurious DNF, which
is exactly the kind of silent instrument error this project keeps paying for.

> **Before running `kbx` (or any long `tmsimp` mode) on a long tape, watch
> `df -h /dev/shm`, and prefer a short window.** A 42-minute tape is ~2.7 MB per
> candidate; at a few thousand candidates a round that is tens of gigabytes.

The re-run was deliberately not attempted on this shared box.

## 3. Files added since v1

```
tapes/LOWINPUT_186935_ev16396_793893.Ghost.Gbx   46d4c20efce76d573ca1255369421e4e
tools/mq.rs                    ladder quantiser + its identity control
analysis/ddmin.txt             the 60-round minimisation log (the enumeration)
analysis/probe_k1.txt          1019 single-event deletions
analysis/probe_k3.txt          679 three-event deletions
analysis/kbx_s3.txt            the kbx run that filled /dev/shm
```
