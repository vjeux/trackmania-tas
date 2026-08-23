# CARRIER.md — reading the sample bytes out of engine memory

> **2026-08-23: five rows below are contradicted by the game's own writer, which
> has since been disassembled — see `SAMPLE-LAYOUT.md`.** Nothing in this
> document or in `carrier-bytes.tsv` has been edited: the contradictions are
> reported there with the measurement that settles each one. They are `b22`
> (the coefficient is 255/2π, not the wheel constant), `u16@4` rpm (65535/30000
> exactly, no offset), `b31` (a 3-bit enum plus one flag bit, not a byte copy),
> the four ground-material bytes (13 is substituted when the wheel's flag bit 1
> is set), and the refutation of the dirt channel (the hypothesis was right; the
> slot is dead in the dedicated server). §5's "byte 89 is closed" is also
> overturned: it is bit 0 of a 32-bit field spanning bytes 89-91 that carries
> all five reactor members and the gear.

A `CSceneVehicleVis` telemetry sample is **116 bytes**. `fk regen` writes 22 of
them (the transform) and the tape echo writes 3 more; the other **91 were still
the donor container's**, which is why `ghost regen` named them every time it
wrote a file. A regenerated ghost's rpm, gear, wheel rotation, suspension travel
and surface state were somebody else's run.

**Thirty of them are now ours**, measured on ten recordings across seven maps, and **`fk regen --carrier` writes them in the same invocation that writes
the transform** — one command, no second pass, no ordering rule.

```
fk regen --carrier TABLE   write them as part of a regeneration   [THE DEFAULT]
         --transform-from-fields   also take the transform from the same copy (see §6)

fk carrier scan     sweep engine memory against one recording     [PROPOSES]
fk carrier merge    intersect several scans into one table
fk carrier confirm  score a frozen table on another recording     [DECIDES]
fk carrier write    write them into an existing ghost, on their own
```

The frozen table is checked in at `tools/fk/carrier-bytes.tsv`.

**The authoritative names for these fields are published**:
[`next.openplanet.dev/Scene/CSceneVehicleVisState`](https://next.openplanet.dev/Scene/CSceneVehicleVisState)
(class `0x0A00C000`) lists every member of the struct this decodes, and
[`openplanet.dev/docs/reference/vehiclestate`](https://openplanet.dev/docs/reference/vehiclestate)
lists the accessors. It gives names and types, not offsets — the offsets and
coefficients here stay the evidence — but it turns "what is byte 22" into "which
of these named floats is byte 22". **Read it before the next sweep.** The
per-wheel members it names (`…DamperLen`, `…WheelRot`, `…WheelRotSpeed`,
`…SteerAngle`, `…GroundContactMaterial`, `…SlipCoef`, `…Icing01`, `…TireWear01`,
`…BreakNormedCoef`) are the eleven slots of the 44-byte record below, and three
of them are now placed.

---

## 1. What was named

Every row scored on **all eight keys** with the coefficients below and **no
refit**: `fk carrier confirm` reads the frozen table and scores it, and there is
no flag that would let it fit anything. The worst per-key exact-agreement rate is
the column that matters.

| sample bytes | field | offset from the car | encoding | worst of 8 keys |
|---|---|---|---|---|
| 0, 1 | *(unnamed 16-bit quantity)* | car+36 | `floor(v · 65535/11000 + 65535000/11000)` | **99.25 %** |
| 2, 3 | `side_speed` | car+40 | `floor(v · 65535/2000 + 65535/2)` | **99.62 %** |
| 4, 5 | `rpm` | car+328 | `floor(v · 2.1844886 + 0.1)` | **96.91 %** |
| 6, 7 | `fl_wheel_rot` | car+92 | `floor(v · 40.743044) mod 65536` | **99.25 %** |
| 8, 9 | `fr_wheel_rot` | car+136 | same | **99.25 %** |
| 10, 11 | `rr_wheel_rot` | car+180 | same | **99.25 %** |
| 12, 13 | `rl_wheel_rot` | car+224 | same | **99.25 %** |
| 21 | `turbo_time` | car+348 | `floor(v · 255)` | **100.00 %** ‡ |
| 22 | *(an angle — `SteerAngle`?)* | car+100 | `floor(v · 40.743044 + 127.5)` | **92.31 %** |
| 23 | `fl_dampen` | car+88 | `floor(v · 63.75 + 127.5)` | **100.00 %** |
| 24 | fl ground material | car+104 | the engine byte, verbatim | **97.44 %** † |
| 25 | `fr_dampen` | car+132 | `floor(v · 63.75 + 127.5)` | **100.00 %** |
| 26 | fr ground material | car+148 | the engine byte, verbatim | **97.44 %** † |
| 27 | `rr_dampen` | car+176 | `floor(v · 63.75 + 127.5)` | **100.00 %** |
| 28 | rr ground material | car+192 | the engine byte, verbatim | **98.09 %** † |
| 29 | `rl_dampen` | car+220 | `floor(v · 63.75 + 127.5)` | **100.00 %** |
| 30 | rl ground material | car+236 | the engine byte, verbatim | **98.47 %** † |
| 31 | `is_turbo` | car+332 | the engine byte, verbatim | **91.60 %** |
| 81 | `FLIcing01` | car+116 | `floor(v · 255)` | **100.00 %** § |
| 82 | `FRIcing01` | car+160 | same | **100.00 %** § |
| 83 | `RRIcing01` | car+204 | same | **100.00 %** § |
| 84 | `RLIcing01` | car+248 | same | **100.00 %** § |
| 91 | `gear_raw` | car+340 | `4 · u8 + 1` | **100.00 %** |

### WHAT `car` MEANS, because an offset without its anchor is not a measurement

> **`car` is the address of the f32 position triple of the copy of the vehicle
> state WHOSE FOUR WHEEL-ROTATION SLOTS HOLD LIVE FLOATS** — where "the four
> wheel-rotation slots" means `car+92 / +136 / +180 / +224`, i.e. the definition
> is closed on this table's own offsets and is not a law about the struct.

That qualifier is the whole difference and it is not pedantry. The engine keeps
several copies of the car; they all hold the same position, and they all pass
every structural test there is -- unit quaternion, velocity equal to the
position's derivative -- and only one of them has the fields around it.
`forkoracle`'s `Layout::pos`, the address the locator returns and the natural
thing to anchor on, is reliably one of the others on this fixture.

**THIS DEFINITION IS A RULE FOR THIS TABLE, NOT A TEST TO EXPORT.** Another arm
applied it to its own anchor by adding the 408 the `gear` offset implies, got
four dead slots, and would have discarded an anchor from which *wetness*
reproduces the recording at 95.4–96.0 % and *gear* at 99.43 %. From that anchor
the wheel block is near `car+1288`, a delta of 1196 where gear says 408 — so the
two anchors are **not** related by one constant, and the wheel block may not sit
at a fixed offset from the position at all (a pointer, or a separate
allocation). Everything below is measured **from this anchor**, and reproduces
on ten recordings across seven maps; that is what it is evidence for.

The test that cannot be wrong is the other arm's, and it is worth adopting:
**does a named channel reproduce the recording?** That needs an answer key and
is therefore not always available — which is why the liveness rule exists at all
— but where a key exists, `fk probe` beats anything here.

**Anchoring on `Layout::pos` wrote ZEROED wheel rotations and gear into a file
that passed the entire `ghost verify` gate** -- codec identity, tape agreement at kappa
1.000, the plain oracle re-simulating to the declared time -- because none of
these bytes affects the simulation. A provenance check does not see it either:
the bytes are not the donor's, they are zeros.

And it cost 408 bytes of apparent disagreement with another arm on the same day.
It located `gear` at its `car+748` with correlation 0.9953 and scored 8.60 % at
`car+340`, where this table says 100.00 %. `748 - 408 = 340`: the same slot, two
anchors, and neither of us had published which one. **State the anchor with the
offset, always.**


**Every row holds on every key that can test it, and nothing fails anywhere.**
The final acceptance, ten recordings across seven maps, the frozen table, no
refit:

```
m134672_68442  23 held, 0 failed,  0 could not be tested
m267460_23068  23 held, 0 failed,  0
m285885_61229  19 held, 0 failed,  4
m191465_13081  18 held, 0 failed,  5
m252289_3867   18 held, 0 failed,  5
m279209_6604   18 held, 0 failed,  5
map2_22730     18 held, 0 failed,  5
m270051_4834   14 held, 0 failed,  9
m134672_63546  the locate refused (no verdict, not a failure)
m134672_69522  the locate refused
```

"Could not be tested" is the third verdict: on that key the channel never moves,
so the prediction scores exactly the constant and is neither confirmed nor
contradicted. Every channel has power on at least three keys — the wheel
rotations, `side_speed`, rpm, the suspension travel, `is_turbo` and `gear` on all
eight; the ground materials on six; `turbo_time` and the four ice channels on
three.

§ The four ice channels are named by the class reference (`FLIcing01` …) and
are the **fourth** placed slot of the wheel record, at +28. They are exact on
**two independent recordings on two different maps** — 462 of 462 samples of
267460 against a 71.9 % constant, and 1370 of 1370 of 134672 rank00002 against a
79.0 % constant — and untestable on the rest, where the car never leaves one
surface. Two keys is below this document's own three-key bar and they are in the
table anyway, for a reason that is stated rather than assumed: the channel's
shape rises, saturates and **dips twice in different places on each run**, which
is not a shape a coincidence produces, and 267460 *cannot* supply a second key
(every other recording of that map is a search tape on the same carrier, with
md5-identical telemetry). Two more 134672 keys exist and are the first thing to
run.

‡ `turbo_time` is exact on all eight keys, but it only has POWER on three of
them (191465, 267460, 285885, where the constant scores 97.71–99.84 %); on the
other five the channel never moves. Three keys with power and eight exact is
above the bar, and the bar is stated rather than the eight quoted alone.

† The four ground-material bytes have power on six of the eight keys: on the
other two the car never leaves one surface, so the byte is
98.97 % and 99.25 % constant and our prediction scores **exactly** the constant —
neither confirmed nor contradicted. That is a third verdict and the tool prints
it as one, because calling it a pass and calling it a failure are equally
dishonest. What separates it from a real failure is the shape: no power reads AT
the baseline, a wrong offset reads far below it (0.75 % when these same four
bytes were anchored one copy-stride away — see below).

### The structure this fell into, which nobody put there

**From this anchor**, and reproducibly on ten recordings across seven maps, the
four wheels are a **44-byte record each**, and four of the fields are in it.
Whether that offset from the position is a property of the game or of this
anchor is **not established** — see the note above.

```
    wheel k at car + 88 + 44k          k = 0..3, front-left first
        +0    suspension travel        -> sample byte 23 / 25 / 27 / 29
        +4    rotation angle           -> sample bytes 6,7 / 8,9 / 10,11 / 12,13
        +16   ground material id       -> sample byte 24 / 26 / 28 / 30
        +28   Icing01                  -> sample byte 81 / 82 / 83 / 84
```

**The ground-material bytes were nearly recorded at the wrong address, and the
way they were wrong is worth more than the bytes.** The sweep first found them at
car−760, −716, −672, −628 — the same stride 44, 100 % exact on five of eight
keys and *0.75 %* on the other three. −760 is +104 minus **864**, and 864 is the
stride of the array of copies of the vehicle state: the sweep had found the same
field in the PREVIOUS copy, which holds the same values whenever that copy is
live and garbage when it is not. Predicting +104 from the arithmetic and testing
it gave 97.4–100 % on every key. A block that scores 100 % five times out of
eight is not a weaker version of a result; it is a different address.

Stride 44 is the same stride a deleted 2026-08-20 probe reported for "four f32
each accumulating distance / one shared radius, |corr| > 0.9999". That probe's
offsets could not be reproduced and its code is gone; this is the same block,
found from scratch, anchored to something that transfers, and confirmed on eight
recordings. A test asserts the stride and the dampen/rotation pairing, so the
structure cannot be quietly broken by a future edit to the table.

### Two coefficients that are exact fractions, and one that is not

* **The wheel constant is 40.743044** on all eight keys, agreeing to about a part
  in a million — which is the fitter's own resolution (a test measures that).
  `256/(2π) · 65535/65536 = 40.7430437`. So a wheel rotation is an **angle in
  radians** put onto 256 units per turn, with the same off-by-one-in-the-last-
  place scaling the project already found in the steer echo (`255/254`) and in
  `side_speed` (`65535/2000`, not `65536/2000`). Three independent fields with
  the same quirk is a house style, not three coincidences.
* **`side_speed` is `65535/2000` with an offset of `65535/2`** — a signed value
  in ±1000 laid symmetrically over the `u16`, exactly as `gbx::record`'s field
  table describes it, now with the engine slot that produces it.
* **rpm's 2.1844886 is a measurement, not a fraction.** It agrees to 8e-6 across
  the keys; `65535/30000 = 2.184500` is 5e-6 away and is a guess, so the table
  carries the measured number.

---

## 2. Three things the census said that turned out to be wrong

`tmtraj corpus bytes` is the enumeration this work started from and it was right
to enumerate. Three of its readings do not survive a direct fit, and all three
failed the same way — a plausible mechanism standing in for a measurement.

1. **"Bytes 4 and 5 are NOT one u16."** The census tested how often byte 4
   wrapping is accompanied by byte 5 changing: 10.5 % of 62 211 wraps, printed as
   its own refuted-hypothesis control. Read as one `u16` against the engine slot
   at car+328, bytes 4 and 5 agree **exactly on 96.9–100 %** of instants on every
   one of eight keys. The carry test assumes the quantity moves by less than 256
   between samples, and rpm does not: at a 50 ms grid the `u16` jumps by
   hundreds, so byte 4 "wraps" without a carry being observable. The census's own
   warning — *a plausible mechanism is not a measurement* — applies to the
   control it printed the warning next to.
2. **"Bytes 0,1 carry like one u16 … go and find it in engine memory."** They do,
   it was, and it is at car+36. Read as one `u16` it scores **99.25–100 %**; byte
   0 alone tops out at 90.1 %, so the pair is not merely tidier, it is the
   quantity.
3. **`rpm_raw` is not "byte 5, absolute scale unknown"** — the scale is known now
   because the slot that produces it is known.

Byte 89 (`is_ground_contact`) is **not** in the table and this work did not
attack it. Three arms have failed on it from three directions and the standing
verdict is that it is closed; nothing here disturbs that.

---

## 3. Why an offset is measured from the car, and why the car is not the anchor

This is the whole methodological content of the exercise, and every line of it
was paid for.

**The heap layout is bimodal run to run**, so nothing can be an absolute
address. **The anchor a locate proposes does not transfer either**: an anchor
that passed every structural check in one server process pointed at an object
**1662.8 m** from the car in the next, and on the following attempt the same
locate returned a single candidate that was a frozen slot. So the gather is
centred on the anchor and the car is then *found inside the window*, against the
recording's own recorded positions — which for an answer key are not a reference,
they are the run.

**The engine keeps several copies of the car and they are not interchangeable.**

* **Shadows.** A copy at car−2648 and another at car−2144 hold a valid state of
  the same car half a millimetre away. Anchoring on one shifts every offset in
  the table by 2648, which is how six keys fitted the same wheel coefficient to
  seven figures at six apparently unrelated offsets. The discriminator is not a
  judgement call: the car agrees with the recording to **0.000000–0.000008 m**
  and the nearest shadow is **0.000486 m**, a hundred times further. `find_car`
  refuses anything above 5e-5 m and widens the window instead.
* **Twins.** Two bit-identical copies sit 262 400 bytes apart. Harmless: the
  field slots are twinned too, so the same relative offset appears for both, and
  the intersection across keys picks it out.
* **Bare position copies, which are the dangerous one.** A copy with the right
  position and *nothing around it*. It is exactly as close to the recording's
  path as the real car — often closer — and every offset measured from it reads
  dead memory. Caught here doing the worst version of that: a two-pass
  regeneration wrote **zeroed wheel rotations and zeroed gear** into a file that
  then passed the entire `ghost verify` gate, codec identity, kappa 1.000 and
  the plain oracle re-simulating to its declared 22.730 — because none of these
  bytes affects the simulation, so nothing downstream can see them being wrong.

  The tie-break is the wheel block: **are the four slots at car+92, +136, +180
  and +224 holding anything that moves.** Four against zero, with nothing in
  between. Two cleverer versions were tried and both are wrong for the same
  reason — the slot is an angle that wraps twice between samples at racing
  speed, so "each wheel tracks the distance travelled" reads |corr| 0.016 on the
  real car and "the four wheels move together" reads 0.4967. `fk carrier write`
  additionally refuses outright if any channel it is about to write came out
  CONSTANT while the container's own values move.

**A "tie" is every copy of this car, not every copy within a ratio of the
best.** The candidate set is everything within a millimetre and the winner must
be within 0.0002 m; a relative window collapses exactly when it matters, because
on a file whose transform has already been regenerated the closest copy matches
at 0.000000 m and `best × 1.5` is then zero, so the real vehicle struct at
0.000083 m is excluded from its own tie. That refused every anchor in a 5 MB
window with the car in it three times over.

**And the write within the tick is named relative to the car, not absolutely.**
The engine writes the vehicle state more than once per 10 ms tick and the
recorder captured one of those instants — the first on five keys here and the
last on three, on the same binary. A table that said `first` scored 100 % on five
keys and 1 % on three: every offset right, every instant wrong. The table says
`car` — the same write the position that identified the car came from.

---

## 4. What makes a row in the table, arithmetically

A scan proposes and cannot test itself. Three bars, in order:

1. **The permutation floor.** Every channel is scored twice in the same sweep,
   once against the recording and once against a **row-permuted copy of the same
   column**. At ~460 instants the best of tens of thousands of candidates sits
   about four standard deviations high for nothing, and this measures exactly
   that, for this channel, this many candidates and this many instants. A
   channel that does not clear its own permutation best is printed as noise on
   the same line.
2. **A constant.** The rate of the channel's single commonest value. A candidate
   below it has learnt nothing — which is why the four surface bytes in §5 are
   *not* in the table despite scoring 100 % on five keys.
3. **Three keys, and in the event eight.** `fk carrier merge` keeps only rows
   where the same offset, kind and write won on at least `--min-keys`
   independent recordings. This is the bar that a previous attempt at this table
   failed: with one recording as the key it reported six bytes at 94–99 % exact,
   and with five keys only two survived — the four per-wheel entries had fitted
   one recording by coincidence.

Each key contributes its **whole candidate set**, not its best offset. `b24`
agrees exactly at hundreds of offsets on one key, so publishing one of them
publishes an arbitrary member of a tie and makes two keys that are both right
look like two keys that disagree. The intersection is the discriminator, so the
set is what a scan publishes.

---

## 5. What is left, enumerated

Of the **91** bytes a regenerated ghost inherited, **30 are now written** and 61
are not.

### Refuted, so nobody re-runs them

* **The dirt channel (bytes 93, 95, 97, 99) is not a `×255` float in the wheel's
  own 44-byte record.** Pre-registered from the class reference — four wheels ×
  the eight slots the three placed fields leave — and scored on the three keys
  where dirt actually moves. The best worst-key lift over a constant is
  **−7.35 points**: below a constant. They stay **absent**, and a page carrying
  a regenerated ghost should say absent rather than let a zero read as a
  measurement. This is the channel behind the wrong-tyre clip, so it is the most
  valuable thing left.
* **Byte 89 (`IsGroundContact`) is refused a fourth time.** The sweep offers it
  at car+58 with an affine map at 91–100 % on five keys, which is the
  small-integer-lookup trap the `k` bound exists for; scored as a raw byte on
  eight keys with no refit it is **0.00 % everywhere**. Three earlier arms failed
  on it from three other directions. The class reference names `GroundDist`
  beside it — a float distance is a better handle than a bool, and that is the
  next thing to try rather than the bool again.

### Live, unnamed, and not attacked — 16 bytes

```
19  20  32  33  34  39  40  41  42  43
69  70  71  72  73  103
```

`b32` reached 55 % on six keys — above its floor, well below anything worth
writing. `b33` reached 58 % on four. The rest never cleared their permutation
floor in a 1.25 MB window around the car.

The class reference names what is probably in them: `SlipCoef`, `TireWear01`,
`BreakNormedCoef`, `WheelRotSpeed`, `GroundDist`, `WorldCarUp`, `IsWheelsBurning`
and the five reactor members. **Fit to a named quantity rather than sweeping**:
a channel with a name has an expected shape, and a permutation floor is a much
sharper test when the hypothesis is one slot rather than three hundred thousand.

### Named by the decoder and still not sourced

`wetness` (101), `sim_time_coef` (102), `is_top_contact` (76),
`booster_air_control_raw` (90) and bytes 103–115, which the decoder never touches
at all.

### The bound on the search, stated

Every negative above is a negative **within 1 048 576 bytes before and 262 144
bytes after the car**, on both writes of the tick, under three encodings (a byte
copy, an affine function of an f32 slot, an affine function of an integer slot),
on eight recordings of six maps. The positive control that the search would have
found a byte if one were there is the same sweep finding 19 channels at 91–100 %
in the same window on the same runs.

## 6. The 0.5 mm "client-vs-server floor" is our own chooser — on the position

This came out of the copy work and it is the most consequential thing here.

Three maps regenerate to **0.489, 0.511 and 0.501 mm** of their own recordings,
and that agreement across three unrelated maps and three run lengths has been
quoted in this project as a *client-vs-server physics floor*. It is not. A
physics difference would scale with something — run length, speed, collisions —
and 22 microns of spread across 67 s to 219 s scales with nothing. It is the
**distance between two copies of the car in the server's own memory**, and the
regeneration was reading the wrong one. Three readings of one quantity, wearing
three hats.

`fk regen --carrier` prints that distance on every run (`the copy holding the
fields is 0.000491 m from the copy the transform was read from`), so it is now a
number on the screen rather than an inference.

**Measured**, map 2, `human_22730`, against the game's own recording of that run,
two files one flag apart:

| | transform from the located copy | from the live-wheeled copy |
|---|---|---|
| worst separation | 0.001 m | **0.000 m** |
| samples reproducing the recorded bytes exactly | **0 of 455** | **227 of 455** |
| position byte 47 | 57 of 455 identical | **335** |
| position byte 51 | 0 of 455 | **396** |
| position byte 55 | 27 of 455 | **350** |
| orientation bytes 59–64 | 237 / 222 / 453 / 209 / 452 | **2 / 8 / 1 / 3 / 5** |

**The position half is confirmed and is stronger than the prediction**: bit
identity goes from nothing to nearly half the run.

**The orientation half is located, exact, and still writes the wrong bytes** —
which is a much sharper statement than the "unexplained regression" this started
as, and it names what is left.

* The anchor's offset does **not** transfer to this copy. That was the first
  guess and it is wrong: taking it makes the orientation bytes worse (2 of 455
  identical against 237). These two are not copies in the sense that matters —
  one has a live wheel block and the other does not — so an offset measured on
  one has no reason to hold on the other.
* Searching the 8 KB around the car for four floats forming a **varying unit
  quaternion**, ranked by the spread of the angle between the body's forward
  axis and the direction of travel, puts the answer first: **car+2632, read as
  (w,x,y,z)**, spread 0.1102 rad against 0.1310 for the runner-up 1056 bytes
  away. Scored against the orientation the game itself recorded, that candidate
  is **exact on 75.0 % of instants with a p90 of 0.00042 rad** — a fortieth of a
  degree — and the runner-up is exact on 3.9 % with a p90 of 0.442. The
  reference-free rule picks the right one and the answer key agrees decisively;
  the rule's own margin is only 16 %, which is one calibration point and not
  enough to set a threshold on, so the ranking decides and the answer key may
  only **veto**.
* The component-order flip that cost 165922 three files was tested and is not
  the cause (0.6919 rad against 0.6957 — no discrimination). The **sign** was
  tested too, because `recwrite` deliberately does not normalise it and q and −q
  are the same rotation in different bytes: the container holds +q, so that is
  not it either.
* **And a quarter of the instants are still wrong.** Bytes 59–64 come back
  2 / 0 / 4 / 1 / 4 of 455 identical, against 75 % of the sampled instants being
  an exact rotation match. Those two are consistent, not contradictory: three
  quarters right and a quarter wrong on a channel that is compared bit-for-bit
  is a file that is wrong wherever it is wrong.

* **And the encoder is cleared.** Reading a real recording's own transform out
  and writing it straight back reproduces the bytes on **453 of 455** samples;
  the two that do not are the degenerate identity rotation, where the writer's
  `sin(ang)` guard zeroes the heading and pitch words. So the encode step is the
  inverse of the read step and is not the fault. A test pins that measurement
  (`the_transform_encoder_round_trips_a_real_recording`).

**What is left is the INSTANT, not the offset and not the encoding.** Another
arm isolated a pairing error the same day — each record instant pairing with an
engine instant one tick ahead, which is nearly invisible in position (a car
moves little along its own track in 10 ms) and loud in attitude. That is the
shape of this exactly. The reason it was not caught here sooner is a mistake
worth recording: **the check that said the quaternion was "exact" reported a
MEDIAN**, and about half the instants match and half do not, so the median sat
in one mode of a bimodal population and read 0.00000. This project has that
written down — *a bimodal population masquerades as a refuted law; split before
you quote a spread* — and it cost an hour here anyway. The candidate report now
prints the fraction exact and the p90 instead.

So the next step is not another offset hunt. It is `fk regen --pair-shift-ms`
against this measurement, and the two fixes land together with both controls,
because a live-wheel copy paired a tick late is worse than what ships today.

So it is **`--transform-from-fields`, default OFF**. `--carrier` alone writes the
fields from the right copy and leaves the transform exactly as it is today. Half
a confirmation is not a confirmation, and this is the publish path.

A 2026-08-20 note suspected exactly this and could not act on it — *"~0.0005 m is
the signature of the shadow, not a measure of accuracy. A gather that found the
car is bit-identical or ~0.000001 m."* Confirmed for the position, two days
later. Recorded as a suspicion confirmed rather than quietly folded in.

**The next person's job**: find where the quaternion lives on the live-wheeled
copy. Then `ghost roundtrip` on 134672 / 227654 / 286279 is the verdict, and the
statistic to watch is its count of samples reproducing the original 22 transform
bytes exactly, which is currently zero on all three.

---

## 7. Running it

```bash
# one key at a time; each is an engine run of a minute or two
fk carrier scan --template KEY.Ghost.Gbx --map M.Map.Gbx --out cand_KEY.tsv --tag KEY

# intersect them
fk carrier merge --tables cand_A.tsv,cand_B.tsv,... --min-keys 3 --out frozen.tsv

# score the frozen table on a key that chose none of its offsets
fk carrier confirm --template OTHER.Ghost.Gbx --map M.Map.Gbx --table frozen.tsv

# write the bytes into a ghost
fk carrier write --template IN.Ghost.Gbx --map M.Map.Gbx \
                 --table ../fk/carrier-bytes.tsv --out OUT.Ghost.Gbx
```

`write` prints, per channel, how far what it wrote agrees with what the
container already held — which on an answer key is the verdict and on a
transplanted ghost is the measure of how much of a stranger's run was in there.

### The end-to-end control

On `human_22730` with the checked-in table: 455 of 455 samples rewritten, 0 left
to the carrier, agreement with the game's own recording **98.68–100 %** per
channel (13 of the 18 channels exact on every sample), and the written file
passes the whole `ghost verify` gate — codec identity, tape/record agreement at
kappa 1.000, and **the plain oracle re-simulating the written file to 22.730,
its declared time**.

That control is leave-one-out in the part that matters: the offsets and every
coefficient were frozen before the file was written, and the only thing the
recording contributed was which copy of the car to read, which is a position
match to a micron rather than a fit.

### The old two-pass pipeline, superseded

`fk regen --neutralise` writes the transform, writes the tape echo and **zeros
every other per-run byte** so no donor byte survives. `fk carrier write` then
fills 21 of those zeros from engine memory. Run in that order the result is 46
of 116 bytes ours, 28 honestly zeroed and the rest the decoder's constants:

```bash
fk regen        --template IN.Ghost.Gbx --map M --out mid.Ghost.Gbx --inputs --neutralise
fk carrier write --template mid.Ghost.Gbx --map M --out OUT.Ghost.Gbx \
                 --table tools/fk/carrier-bytes.tsv
```

Measured, on `human_22730`: both passes succeed on the first attempt and the
result passes the whole `ghost verify` gate — codec identity, kappa 1.000, and
**the plain oracle re-simulating the written file to its declared 22.730**.

**`ghost regen` does not call the second pass yet, and that is deliberate.** It
is the publish path, it runs 46 checks and a plain-oracle gate around the file
it writes, and adding a second engine pass inside that gate is a change to the
one path in this project where being quietly wrong gets published. The change is
small; the control it needs is a corpus re-run, which is a job of its own.

Note the second pass takes `mid.Ghost.Gbx` as its template, so the positions it
identifies the car by are the ones pass one wrote. **That is why the copy rule in
§3 cannot be a distance bar** — the copy pass one read matches at 0.000000 m and
the vehicle struct is 0.000488 m away. On a file whose transform has *not* been
regenerated the positions are the DONOR's, the identification finds the donor's
car, and `fk carrier write` would faithfully write the wrong run's bytes.
**Regenerate the transform first.**

### One flake worth knowing about

`could not measure the clock bias at any checkpoint` happens: 285885 failed twice
and then worked on the third identical invocation. It is a server-start failure,
not a measurement failure — the run produces nothing rather than something wrong
— but a fleet driving this in a loop should retry rather than record an absence.

### The costs

A scan is one engine run plus a sweep: **1–3 minutes** per key on 20 threads
(a 1.25 MB window is 328 000–1 300 000 offsets against 95 channels on both
writes, twice, for the permutation floor). A confirm is the same engine run and
a scoring pass. Eight keys in parallel on this box: about five minutes.
