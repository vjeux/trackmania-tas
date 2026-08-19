# A greedy per-station crawl locks in its own accidents

Measured on map 146612, sector 5, 2026-08-19. Written as a fleet note because
the failure is a property of the *method*, not of that map.

## The method

A ladder (`tmmaps ladder`, origin-controlled) turns "distance along a sector"
into a millisecond. That makes an unsearchable plateau searchable: on 146612 the
final sector produced **0 finishers in 207 000 evaluations** with only the
finish as an objective, and **13 of 22 stations climbed** once each station
became its own objective. The obvious way to use it is a crawl — optimise
arrival at station *k*, take the winner, use it as the seed for station *k+1*.

## The failure

Delta to the human world record, per station, for the crawl seeded from the
sector-4 jump tape:

| st02 | st03 | **st04** | st05 | st06 | st07 | st08 | st09 | st10 | st11 | st12 | st13 | st14 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| −0.501 | −0.231 | **+1.232** | +1.202 | +1.416 | +1.589 | +1.728 | +1.779 | +1.891 | +2.026 | +2.161 | +2.304 | +2.601 |

then stalled at st15. **The entire run is decided at one station.** st03→st04 is
1.813 s for 28 m of track — a wall contact that dropped the car from 74.5 m/s to
22.6 m/s. Every station after it inherits a dead run and the crawl spends the
rest of its budget nursing one. The tape that came out at st14 is 2.601 s behind
a run that was 0.501 s *ahead* twelve stations earlier.

Nothing in the crawl notices. Each station reported an improvement over its own
seed, every result validated, no phantom, no error. The greedy accept is doing
exactly what it was told.

## Three fixes, cheapest first

1. **Watch the delta, not the absolute.** Track arrival-minus-reference at each
   station. A jump in the delta is the signal; here it was a 1.5 s
   discontinuity in an otherwise smooth sequence. Re-run any station whose
   delta jumps before continuing past it.
2. **Keep the best *k* per station, not the best one.** The crawl has no way
   back once it has committed. A beam of 3–4 costs 3–4× the search but is the
   difference between finding a line and polishing a crash.
3. **Use a LOOKAHEAD objective: at station *k*, score arrival at station
   *k+2..3*.** This is the one that addresses the cause rather than the
   symptom. "Fastest to station *k*" is satisfiable by a tape that arrives fast
   and pointing at the outside wall; "fastest to station *k+3*" is not, because
   a bad exit cannot reach it. Same ladder, same cost per evaluation.

The general form of (3), which is worth stating on its own: **optimise arrival
PAST a checkpoint, never at it.** On 146612 the sector-4 jump reaches CP5
1.128 s ahead of the world record and is still 0.639 s ahead 26 m later, and
every millisecond of that is gone 55 m after that — because "fastest to CP5"
bought a state that could not use its own speed. A ladder makes the better
objective cost exactly the same to evaluate as the worse one.
