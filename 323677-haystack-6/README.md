# Haystack 6

Puzzle map by **m1el**: [Trackmania Exchange](https://trackmania.exchange/mapshow/323677)

Stock map MD5: `440c2bc538d9fcff2bde3f595c0d2f21`

## The 278-move run, on video

https://github.com/user-attachments/assets/3bc83e5f-33b5-430a-9d3c-8db53a1814a0

Nine 12-second samples spread across the run. The full render is 23:32.87 at
1280x720 — one clip as long as the run itself, because every move is a respawn
teleport and the car jump-cuts to a new cell every few seconds.

The filmed container is a **film container, not a publishable ghost**: our own
synthesised container crashes the retail client on import (it carries 6
skippable chunks where a game-recorded one carries 24), and it holds no
trajectory at all — it is an input tape, not a recording. So a game-recorded
container was rebuilt to the run span and this run's driven trace written into
it instant for instant. Verified: 28,235 shared instants, position median, p95
and max all **0.000 m** against the drive trace; the plain oracle independently
returns `cps Some(278)` on the untouched tape.

## Current status — 2026-08-26

- **Best fully driven route:** 278 moves, 278 fresh groups, every respawn placement exact; reproduced on a second fork.
- **All 322 groups are individually collectable** on the stock map, each with a direct driven proof.
- **A complete once-only route is not yet known.**
- The validator uses ordered, set-labelled events rather than a static one-color edge:

  ```text
  fresh = [group in ordered_events(move) if group not already held]
  move is legal iff fresh is non-empty
  held |= fresh
  destination = respawn state of the last fresh event
  ```

- The authored waypoint graph is not always the driven vehicle graph. In particular, the lower `CPBox_U` trap-box escapes exist in the map data but cannot be fired from the state where ordinary entries place the car.

## External graph analysis

[m1el’s graph extraction and solver notes](https://gist.github.com/m1el/fe744797fc68771de43362902bdca9c6) are a strong description of the authored waypoint structure. We compared them with our stock-map vehicle-oracle measurements:

- [Full comparison](M1EL-COMPARISON.md)

The short version: the census, funnel structure, special-LID inventory, eight double-takes, and four start/hub groups agree. The trap-box escape model, static destinations, event ordering, and exactly-one-edge-per-LID Eulerian reduction do not match the measured engine semantics.

## Important corrections discovered during this work

- One physical move can collect two or three groups.
- Events for held groups are suppressed.
- Suppression can change the respawn destination.
- The eight bonus-only respawn pockets are terminal rather than useful connectors.
- The eight formerly unexplained groups are collected through plain `CPBox` gates in the hand-built center, not through `CPBox_U`.
- The center is highly one-way, not an all-to-all teleport.

No leaderboard submission was made.
