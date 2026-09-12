# RESUME — tiny campaign ghost videos, session 4 — STOPPED 2026-09-12 03:18Z (closed by the parent)

## Delivered
- tiny/README.md: 25 rows, every row = its Nadeo author ghost's video rendered on ship18f
  (film-grade telemetry for the 10 re-driven laps), build label, ↻ notes, a `map:` link to the
  ship18f zip (10 maps > 25 MiB link the `-lite` Nadeo-variant zip), no ⚠ lid lines, no pending lines.
- FINAL-video4.md (this dir): the delivery table — map | title | lap | build | asset URL | approval | notes.
- Zips: mapzips-ship18f/ (25 full + 10 lite, MD5.tsv + MD5-lite.tsv); each = map + certified ghost + README.txt.

## State files (this dir, *-video4.*)
ships.tsv (every clip ever rendered: URL / pending / staged / held / superseded), approvals.tsv
(receipts; tokens water_ok=B, attitude_ok=parent), holds.tsv (only 21 `none` left), rowbuilds.tsv
(row build + zip link + `clip=` note), cams.tsv (16 → camera 6), priority.tsv (upload order),
builds.tsv (`* latest latest` = the newest installed incoming/ship* set with STARTCHECK 25/25),
lidrows.tsv (empty), shipwatch/run-watch logs, the four loop scripts.

## How to resume (a fresh box)
1. `persistent-storage mount --auto`; rsync ~/tt-video4 ~/tt-page ~/tinyvid4 ~/.cargo ~/.rustup
   ~/.whitestick ~/.ssh/{id_ed25519_tmtas,config.tmtas} from the last box (or rebuild: git clone
   vjeux/trackmania-tas → tt-video4; page clone → tt-page; state from this dir → ~/tinyvid4/out).
2. `cargo build --release -p tinyctl` (https_proxy=http://fwdproxy:8080).
3. Loops: ~/tinyvid4/loop-up.sh (render loop, --rebuild-all, --idle-quit-min 0 — drop that when
   not bursting), shipwatch-up.sh (currently --collect-only: the PARENT session uploads; remove the
   flag to let it launch tinyship.sh itself), pagestatus-loop.sh, bank.sh.
4. Box (WhiteStick): uploader /home/vjeux/shoot/tinyship.sh (flock /home/vjeux/shoot/tinyship.lock,
   cookie /home/vjeux/.gh-upload/cookie, COOLDOWN default 0), zips /home/vjeux/shoot/tinyfile.sh,
   zips staged at /home/vjeux/shoot/_mapzips/, verdicts at /mnt/c/Users/vjeux/tinyvid/ship/*.done.

## Rules learned today (details in memory tm2020-tiny-campaign.md)
- A GitHub cookie copied from a LIVE browser dies after 1–2 uploads; one captured right after a
  fresh login (#8, #12) carries dozens. The ghsession seed would end the churn.
- Receipts follow the TAPE (`tinyctl tape-id`), not the file md5; inherited-flag laps pass on the
  parent's literal receipt; `attitude_ok=parent` overrides the gate per (map, time) with the ruling quoted.
- Sliver rule only vs a PUBLIC lap; a lap certified on the render build outranks an old-build clip.
- Camera per map in cams.tsv (Ext2 keeps the car in frame on lips/bowls); cam-6 review renders settle
  roll-vs-pitch disputes.
- Never edit ships.tsv while shipwatch runs; ships.tsv merge is keyed (map, time, clip).
