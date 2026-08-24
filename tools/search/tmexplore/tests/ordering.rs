//! The ordering, swept rather than asserted in prose.
//!
//! The claim is "`Ord` puts every finisher above every non-finisher by
//! construction". A sentence like that is exactly what `FINISH_BASE` also
//! claimed, right up until an eleven-checkpoint map made it false.

use tmexplore::outcome::Reached;

#[test]
fn every_finisher_outranks_every_non_finisher() {
    // The corners that broke the old scheme: a very deep failure on a very
    // long map, against a very slow finish.
    let stopped: Vec<Reached> = [0u32, 1, 7, 11, 64, u32::MAX / 2]
        .iter()
        .flat_map(|&cps| {
            [0u32, 1, 143, 5000, 1_000_000, u32::MAX / 2]
                .iter()
                .flat_map(move |&station| {
                    [0u32, 1, 4500, u32::MAX / 2]
                        .iter()
                        .map(move |&ticks| Reached::Stopped { cps, station, ticks })
                })
        })
        .collect();
    let finished: Vec<Reached> = [0i64, 1, 20_555, 355_181, 10_000_000, i64::MAX / 4]
        .iter()
        .map(|&ms| Reached::Finished { ms })
        .collect();

    for s in &stopped {
        for f in &finished {
            assert!(f > s, "{:?} did not outrank {:?}", f, s);
            assert!(s < f);
        }
    }
    assert_eq!(stopped.len(), 6 * 6 * 4);
}

#[test]
fn a_faster_finish_wins() {
    assert!(Reached::Finished { ms: 20_555 } > Reached::Finished { ms: 20_556 });
    assert!(Reached::Finished { ms: 1 } > Reached::Finished { ms: 355_181 });
}

#[test]
fn among_failures_checkpoints_dominate_then_station_then_time() {
    // checkpoints first: our route can be wrong, the map's own gates cannot.
    assert!(
        Reached::Stopped { cps: 2, station: 1, ticks: 9999 }
            > Reached::Stopped { cps: 1, station: 500, ticks: 1 }
    );
    // then station
    assert!(
        Reached::Stopped { cps: 1, station: 44, ticks: 9999 }
            > Reached::Stopped { cps: 1, station: 43, ticks: 1 }
    );
    // then sooner
    assert!(
        Reached::Stopped { cps: 1, station: 44, ticks: 100 }
            > Reached::Stopped { cps: 1, station: 44, ticks: 101 }
    );
}

#[test]
fn the_ordering_is_total_and_consistent() {
    let mut v = vec![
        Reached::Finished { ms: 30_000 },
        Reached::Stopped { cps: 9, station: 900, ticks: 10 },
        Reached::Finished { ms: 20_000 },
        Reached::Stopped { cps: 0, station: 0, ticks: 0 },
        Reached::Stopped { cps: 9, station: 900, ticks: 9 },
    ];
    v.sort();
    assert_eq!(
        v,
        vec![
            Reached::Stopped { cps: 0, station: 0, ticks: 0 },
            Reached::Stopped { cps: 9, station: 900, ticks: 10 },
            Reached::Stopped { cps: 9, station: 900, ticks: 9 },
            Reached::Finished { ms: 30_000 },
            Reached::Finished { ms: 20_000 },
        ]
    );
}
