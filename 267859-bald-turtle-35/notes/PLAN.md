# 267859 `bald turtle #35` — PLAN for the TAS attempt (v1)

Written 2026-08-18 21:05 PT, **before** any search result was inspected beyond
the first 45 s smoke test. Sidecar: the existing `PLAN.md` in this directory
belongs to the earlier *attitude-experiment* agent, who explicitly ran **no
search**. This file is the search plan; nothing of theirs is overwritten.

AT **10.768** · human WR **11.169** (Schmaniol) · gap **0.401** · **19 records**
· uid `auaaMFbt2cKnZPYjP11sySqEb_6` · author **Bald_tm / BALDFROMSPB** · tag
**Turtle** (and *only* Turtle — no Trial tag).

---

## 0. §0 prior work — DONE FIRST

`memory__search 267859` + directory listing. This map **has been worked before**:
`PLAN.md` + `RESULT-attitude-test-v1.md` (the roll/attitude pre-registered test,
confirmed) and `csv/` with all 19 decoded trajectories. That agent's closing
sentence: *"No search was run here; this map was taken as an experiment, not as
a target. The AT is unbeaten and the gap is 401 ms over 19 records, so it remains
a good TAS target for whoever wants it."* So the search is open work, and their
§8 result is a free prior.

## 1. WHICH FAMILY IS THIS MAP IN — settled in the first 20 minutes

The brief's headline hypothesis (a Turtle/Trial retry-grind map, where the win is
deleting the record-holder's failed attempts) is **REJECTED for this map**, on
three independent facts:

| evidence | value |
|---|---|
| `tmmaps list` | **12 blocks, 1 item.** Waypoints: `GateStartCenter8m` (Spawn) and `PlatformTechFinish` (Goal). Nothing else. |
| `tmtas splits` on the human WR | `race_time=11169 splits=[11169]` — **NbCheckpoints = 1**, the finish itself |
| unbeaten.at tags | `Turtle` only. `[Turtle Trial] Angustus` and `[Turtle Trial] Leto` both carry **Trial** |

ACQUISITION §10 is explicit about what that means: with no intermediate
checkpoint a respawn returns the car **to the start with the clock running**, so
it is never a recovery and the retry-splice method has nothing to bite on. The
401 ms is **line and timing**, not deleted attempts. 11.169 s over 1117 ticks is
also simply too short to hide a retry.

**"Turtle" here is a driving style, not a checkpoint mechanic**: the car is
upside down for the whole run (the prior agent measured roll alternating
±2.4…2.9 rad across 15 successive inverted landings, 19–22 % airborne).

## 2. Controls run before searching (all PASS)

* **§4 identity / §8 field reproduction:** all **19/19** ghosts re-simulate to
  their exact recorded millisecond through the plain oracle. Independently
  re-run by me, not inherited. The oracle can be trusted on this map.
* **codec identity:** 5 seeds rebuilt through `tmsearch --verify` and
  re-validated: 11169 / 11189 / 11407 / 11467 / 11659, all exact.
* **§9 embedded author ghost: ABSENT.** Header says `validated="1"` but
  `tmtraj decode map.Map.Gbx` reports no `CPlugEntRecordData` — the §9a outcome
  (like 267460). So the AT's route is *not* readable and cannot seed us. We
  search from the human field.

## 3. What the field looks like

The **top four humans are keyboard runs**: steer alphabet exactly
{−127, 0, +127}, 42 / 40 / 52 / 60 input change events. Rank 5 is the first pad
run (119 values, 217 events) and it is 410 ms slower than rank 1. So the map's
native idiom is digital, which means the low-input deliverable (§B) should cost
close to nothing here — the opposite of a pad map.

## 4. The plan

1. **Four independent search arms**, one per fast keyboard seed
   (rank 1 / 2 / 3 / 6), `--ops mix2 --temp 25`, 40 workers each, **distinct
   `--root` per process** (the phantom rule), guard on by default.
2. Promote the best arm, re-seed a second round from it, localise the window
   where the delta against the human WR accumulates.
3. **Every banked best re-validated through the plain oracle** with absolute
   paths before it is reported. A guard failure is a STOP and an incident.
4. On reaching the AT: the two mandatory follow-ups —
   §A the tick-by-tick "how does a human do this" against the WR (this map is
   *hunted* only 19 deep, so a technique statement matters more than another
   20 ms), and §B the low-input family, which on a keyboard map should be
   cheap.

## 5. What would make me stop and report a failure

A guard quarantine that re-validates DNF under distinct roots; or the search
plateauing above the AT with the residual traced to a feature the field cannot
express. Not expected: 401 ms over 1117 ticks with a 3-value alphabet is a wide
target.
