# tinyctl — the tiny-campaign operations, one binary, no shell

Per map, in order (the devserver is where these run; the render box is
reached through `~/bin/wsx`):

```
tinyctl probe   /tmp/summer2026/NN-*.Map.Gbx [--paks "$PAKS"]
tinyctl views   /tmp/summer2026/NN-*.Map.Gbx --out /tmp/tinyNN/views.tsv
mapgeom $PAKS tiny-library …   ;  tmmaps tiny …                 (unchanged)
tinyctl shoot   --orig SRC --tiny /tmp/tinyNN/Summer-NN-Tiny.Map.Gbx \
                --views /tmp/tinyNN/views.tsv --tag sNN --anchor <from views.tsv>
tinyctl publish-map NN --map /tmp/tinyNN/Summer-NN-Tiny.Map.Gbx --items-dir /tmp/tinyNN/libx/Items --playcheck
```

* `probe` — what the environment onboarding measured by hand: collection,
  ground row (table vs the items' own measurement), fixed plane, the anchor,
  genealogy zones + the policy `tmmaps tiny` applies, zone-block census,
  waypoints, block/item model histograms; with `--paks` a dry library build
  that lists the models the packs lack.
* `views` — the cameras: start / every checkpoint / finish looked at along the
  gate from behind, the map from above and its four quadrants. Edit the TSV
  freely; the anchor line is in its header comment.
* `shoot` — pushes both maps and the views, runs `shootctl shootset` on the
  box (ONE editor load per side, one screenshot per view — `cmpshot.sh`
  loaded two maps per view), runs `tinyctl compare` there, pulls back
  `cmpdiff-sNN-crops.png` (the worst cells, original | tiny, labelled),
  `cmpdiff-sNN-overview.png` (every view, flagged cells outlined), the TSV and
  the plain side-by-side JPGs. Look at the crops sheet first: one image view
  instead of one per view. Full frames stay on the box in
  `/mnt/c/Users/vjeux/tinyshots/sNN/` (`--pull-full` to fetch them).
* `compare` — the diff alone, on shots already taken (`cmp-<tag><view>-o.png`
  / `-t.png`, the names `cmpviews.sh` used). Per-cell mean colour and edge
  energy; `--color`/`--edge` thresholds; the top and bottom cell rows (editor
  UI) are ignored unless `--keep-hud`. Live screens, water and vegetation
  flag by nature — the labels make that a glance.
* `publish-map` — the item-check gate (in-process `mapgeom item-check` over
  `--items-dir`), a header sanity check (`Tin…` uid), the push, then
  `tinyctl publish-here` on the box: game tokens through the GhostShooter
  `/nadeotoken` route, create-or-UPDATE by uid (a second create keeps the old
  collectionName), the campaign playlist (`--position`, default NN-1),
  stored-bytes md5 read back against the local file, `--playcheck` plays the
  stored copy and screenshots it.
* `box-build` — `git pull` + `cargo build --release` ON the box (WSL has
  cargo) for shootctl, tinyctl, mapgeom, tmmaps; polls to completion. No more
  pushing 4 MB binaries at 1.4 MB/s.

Long jobs on the box (`shootset`, `publish-here`, `selfbuild`) detach
themselves and write a done file; the devserver side polls it, because the
bridge cuts a command at ~90 s and forwards no stdin.

## video — the lap's clip, with the controls overlay by default

`tinyctl video --map NN` renders the lap on the box (renamed render copy, the
guard on sample 0 and the finish), pulls the webm, and **cuts the publishable
mp4 with the controls overlay** (`clip cut --ghost`: crf by lap length under
100 MB, the timing checked against the picture, the file stamped), pushes the
mp4 beside the webm on the box, copies webm + mp4 + sheet to `--store`, and with
`--ship` starts the box-side publish (`box/tinyship.sh`, whose `clip ship`
refuses an unstamped file). `--from-webm F` runs the same cut + overlay + ship
on an existing render; `--no-overlay` is the bare mp4, in capitals. `--all
--watch 60 --ghosts-sync host:dir --build ship15 --ship` is the day loop: every
ship15 ghost whose trajectory is new gets rendered and shipped; `tinyctl
shipwatch --out DIR --readme tiny/README.md --commit` collects the URLs and
swaps the page rows (21–25 by their country names). Per-lap REPORT rows land
in `<out>/REPORT.md` with the overlay column (offset, how it was checked).
