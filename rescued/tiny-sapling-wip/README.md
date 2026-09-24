# Rescued: the trackmania-tas-tiny Sapling WIP (2026-09-24)

This is uncommitted work rescued from `/home/vjeux/trackmania-tas-tiny` on
devvm42752 — a **Sapling** checkout, separate from this git repo, which was
left behind when the devserver's git clones were deleted.

It is parked on a branch rather than merged into master, deliberately. See
"Why this is not merged" below.

## What was there, and what happened to it

The checkout had **57 draft (unpushed) commits** dated 2026-09-04/05. Those
are NOT here, because they are already on master: the Sapling repo was an
earlier parallel history of the same work, later migrated into git. Spot
checks that all resolve to master commits —

| draft | on master |
|---|---|
| the embedded-item zip must be DEFLATED | yes |
| tmmaps tiny-catalog | yes |
| crystal writer: dedupe equal materials | yes |
| reusable half-scale campaign map generator | yes |
| complete CGameCtnBlockInfo reader | yes |

What was NOT on master is the **uncommitted** work on top, which is what this
directory holds.

## Contents

* **`bake-isoframe.patch`** — the substantive one. Adds an `isoframe`
  parameter to `mapgeom::static_item::bake::tangents_vprim` plus a census of
  bit-doubled UV pairs: a doubled pair shared by two or more triangles (fan
  mates across a degenerate diagonal) gets AGREEING tangent frames so the
  mates weld, while unique pairs keep index-order frames. Absent from master
  (`isoframe` and `paircount` appear nowhere in it).
* **`bake-tiny-road17.patch`** — a change to `tools/bake-tiny-road17.sh`,
  which master **deliberately deleted** in 7d186b4f ("stage-1/2 recipe
  scripts retired"). Kept only for the record; do not restore the script.
* **`examples/`** — 15 uncommitted mapgeom probe programs (detfit,
  edgemember, exactcorners, faceprobe, hisuv, indextest, layerdump, nbrnorms,
  partcomp, probe4093, qadtest, subsetfit, uprobedet, vcount, wedgetest).
  Note master removed twelve similar stage-1/2 probes in 706cee19 precisely
  because they no longer compiled and `cargo test` builds every example before
  running a single test — so adding these to `mapgeom/examples/` would break
  the workspace test run. They are kept here as source, outside the build.

## Why this is not merged into master

1. **It is unverified WIP.** It was left uncommitted for three weeks. Nothing
   says whether the isoframe approach worked or was a dead end.
2. **It changes geometry output.** `tangents_vprim` decides tangent frames,
   so a wrong merge is not a build failure — it is silently wrong shading in
   baked items, the hardest class of bug to notice.
3. **The target file has been reorganised under it.** `tangents_vprim` sits
   at line 274 on master and at line 620 in the Sapling tree; bake.rs has
   been substantially rewritten since. The patch will not apply cleanly, and
   forcing it means hand-porting a change whose correctness nobody can
   currently attest.

So it is preserved here, in the one canonical place, under version control,
where it can be picked up deliberately by whoever owns that work.

## To pick it up

```sh
git show rescued/tiny-sapling-wip:rescued/tiny-sapling-wip/bake-isoframe.patch > /tmp/iso.patch
# then hand-port onto the current bake.rs, and VERIFY the tangents against a
# 1:1 editor bake before trusting them
```
