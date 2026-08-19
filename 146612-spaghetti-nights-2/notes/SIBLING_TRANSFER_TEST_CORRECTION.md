# 146612 — CORRECTION to my v1 §4: the divergence bound over-promised, and the transfer test that replaces it

Write-once, `key_` prefix. **Supersedes §4 and the "best reference" framing of
`key_RESULT_v1_spaghetti_series_answer_keys.md`** (which stays in place — the
rest of it, the identity table, the validated ghosts and the 38.532 coincidence,
is unchanged and still correct). Times in seconds.

---

## What I got wrong

v1 §4 reported that Spaghetti Nights 3's human line first meets divergent
geometry at **t = 8.650**, with 15 % of samples divergent, and called it "the
best reference … valid for 146612's first 8.65 s".

The sector arm (session `fb8a6600`) tested that claim the right way — the five
sibling ghosts against **146612's own `seg1`**, the untouched map with CP1
promoted to finish, so a tape only has to survive **7.39**, i.e. 1.26 s *inside*
my supposedly identical region:

```
rank00001_40223 (native control)          7311   exact
rank00006_41561 (native control)          7295   exact
key_151734_mernama_39555                   DNF
key_133353_spaghett37_38532                DNF
key_164965_cremconnoisseur_39433           DNF
key_146199_gazorpalse_43466                DNF
key_151831_mareng_44824                    DNF
```

Five of five DNF before CP1, both controls exact in the same batch. So the
tapes do not survive the *beginning* either, and "valid for the first 8.65 s"
was not a supported statement.

## The rule I should have applied, and now do

> **An occupied-cell diff is a statement about GEOMETRY, not about
> drivability.** Two maps can be byte-identical in every block a car touches for
> 8.6 s and still put an open-loop tape somewhere else inside 7 s, because what
> diverges first is not geometry — it is spawn pose, or one decoration's
> collision, or a contact resolving a tick differently.

My own caveat (a cell counts as divergent if *any* record differs there,
decoration included) made the bound conservative about **geometry**, which is
the opposite of the direction that matters for transfer. A conservative
geometric bound is not a permissive driving bound.

**Adopted for every map in this sweep from here on:** a similarity number is
never reported alone. It is reported next to a **transfer test** — the sibling
ghost against the target's own `seg1`, with a native ghost as an in-batch
control — and the verdict is stated as *transfers* / *does not transfer*, not as
a percentage.

## What the siblings are still for

Exactly what they were for on 284238, where this technique came from: the answer
key there was a human **beating his own map's author time on the shared
obstacle**, and its value was as a **reference trajectory to measure against** —
what speed, what attitude, what line the obstacle demands — not as a tape to
graft. Grafting failed there too, across ~250 attempts.

So for 146612 the standing claims are:

* **still true**: 151734 shares 3 475 of 3 541 records; mernama's 39.555 on it
  beats that map's AT of 39.840 and re-simulates exactly; 133353's human sits at
  38.532 against our author time of 38.530 over 76 % shared blocks; twelve of
  twelve sibling ghosts reproduce their millisecond on their own maps.
* **withdrawn**: any suggestion that those tapes are usable as seeds, or valid
  "for the first 8.65 s" of our map. They do not reach our CP1.
* **the honest use**: measure our tape against mernama's over the sectors whose
  blocks are identical, the way 284238's `cold_` write-up measures our record
  against Yhomas's — same geometry, human speeds and attitudes, no grafting.
