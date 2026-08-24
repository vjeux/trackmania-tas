//! The archive: the quantized whole car state at a station, best entry per
//! bin, and the policy that decides where to spend the next fork.
//!
//! # The thing this replaces
//!
//! `SEARCH.md` §5's missing feature, item 1: *a state objective — score the
//! car's WHOLE state at a place, in non-overlapping bands.* Three maps needed
//! it and each one hand-rolled it in a private fork of the search, which is
//! what a missing feature looks like.
//!
//! The reason it has to be the whole state is one sentence: **two cars at the
//! same arc length with different speeds and attitudes are not comparable by
//! arc length alone, and a search that pretends otherwise throws away the fast
//! one.** An archive keyed on arc length keeps whichever car got there first,
//! which on any map with a corner is the one that arrived too hot to turn.
//!
//! # Non-overlapping, by construction
//!
//! Every field of the key is `floor(x / band)` on a scalar. That is a
//! partition of the real line: each value lands in exactly one bin, and two
//! values in one bin differ by less than the band. There is no window, no
//! tolerance and no nearest-neighbour lookup, so there is nothing to get
//! subtly wrong — and `tests/bins.rs` checks both halves, because a binner
//! that puts everything in one bin satisfies "same bin ⇒ close" and one that
//! puts everything in its own bin satisfies "different value ⇒ different bin".

use crate::branch::{CarState, Handle, Progress, Route};
use crate::rng::Rng;
use crate::trunk::NodeId;
use std::collections::HashMap;

/// The band widths. Every one of them is a choice, so every one of them is a
/// flag, and the defaults are stated here rather than scattered through the
/// code.
#[derive(Clone, Copy, Debug)]
pub struct Bands {
    /// Metres of arc length per station. B's route supplies this; the archive
    /// takes it so it can be reported alongside the bins.
    pub station_m: f32,
    /// Metres of lateral offset per bin.
    pub lateral_m: f32,
    /// Metres of height per bin. Height matters: the same station at road
    /// level and ten metres up are different situations.
    pub height_m: f32,
    /// Metres per second of speed per bin.
    pub speed_ms: f32,
    /// Degrees of heading per bin.
    pub yaw_deg: f32,
    /// **The ablation switch, and it is here rather than in a test harness so
    /// that the negative control is the same code as the positive one.**
    ///
    /// When set, the key is the station and nothing else: the archive keyed on
    /// arc length alone, which is what a search that believes two cars at the
    /// same place are comparable actually does. It is expected to be worse and
    /// the point is to measure how much.
    pub state_blind: bool,
}

impl Default for Bands {
    fn default() -> Self {
        Bands {
            station_m: 20.0,
            lateral_m: 3.0,
            height_m: 4.0,
            speed_ms: 5.0,
            yaw_deg: 20.0,
            state_blind: false,
        }
    }
}

/// Airtime, bucketed rather than binned linearly, because the interesting
/// distinction is not "how long" but "which regime".
///
/// A car on the ground, a car that has just left it, and a car three seconds
/// into a 259 m flight are three different situations, and the third one has
/// no arc-length progress at all — which is exactly the case a naive progress
/// metric scores as "stopped" and throws away.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AirBucket {
    Grounded,
    /// 1–10 ticks off the ground: a bump, a kerb, a crest.
    Hop,
    /// 11–40 ticks: a real jump.
    Flight,
    /// Over 40 ticks: a long ballistic arc.
    LongFlight,
}

impl AirBucket {
    pub fn of(airtime: u16, wheels: u8) -> AirBucket {
        if wheels != 0 {
            AirBucket::Grounded
        } else if airtime <= 10 {
            AirBucket::Hop
        } else if airtime <= 40 {
            AirBucket::Flight
        } else {
            AirBucket::LongFlight
        }
    }
}

/// The quantized whole car state at a station.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BinKey {
    pub station: u32,
    pub lateral: i16,
    pub height: i16,
    pub speed: i16,
    pub yaw: i16,
    /// The wheel-contact PATTERN, not a count.
    pub wheels: u8,
    pub air: AirBucket,
    /// Checkpoints collected. In the key because a car that has taken a
    /// shortcut past a checkpoint is not in the same situation as one that has
    /// not, however identical its metres and metres per second.
    pub cps: u32,
}

impl BinKey {
    pub fn of(st: &CarState, pr: &Progress, route: &dyn Route, b: &Bands) -> BinKey {
        if b.state_blind {
            return BinKey {
                station: route.station_of(pr.s),
                lateral: 0,
                height: 0,
                speed: 0,
                yaw: 0,
                wheels: 0,
                air: AirBucket::Grounded,
                cps: 0,
            };
        }
        BinKey {
            station: route.station_of(pr.s),
            lateral: qfloor(pr.lateral, b.lateral_m),
            height: qfloor(st.pos[1], b.height_m),
            speed: qfloor(st.speed(), b.speed_ms),
            yaw: qfloor(st.yaw().to_degrees(), b.yaw_deg),
            wheels: st.wheels,
            air: AirBucket::of(st.airtime, st.wheels),
            cps: st.cps,
        }
    }
}

/// `floor(x / band)`, saturating into an `i16`.
///
/// Saturating and not wrapping: a wrap would fold two far-apart states into
/// one bin, which is the one error a bin key must not make. The saturation
/// range is ±32767 bands — 98 km of lateral offset at the default — so it can
/// only be reached by a NaN or an escaped car, and both of those belong in one
/// bin anyway.
pub fn qfloor(x: f32, band: f32) -> i16 {
    if !x.is_finite() {
        return i16::MAX;
    }
    let q = (x / band).floor();
    if q > i16::MAX as f32 {
        i16::MAX
    } else if q < i16::MIN as f32 {
        i16::MIN
    } else {
        q as i16
    }
}

/// One archived state: the best way we have found to be in this bin.
#[derive(Clone, Debug)]
pub struct Entry {
    pub node: NodeId,
    /// Ticks of input in the prefix that produces this state. This is both the
    /// arrival time and the truncation point, and they are the same number by
    /// construction — so a state can never be scored on one tape and replayed
    /// as another.
    pub ticks: u32,
    pub state: CarState,
    pub progress: Progress,
    /// A live handle parked exactly at `ticks`, if the backend keeps one.
    /// Purely an optimisation; losing it costs time, never correctness.
    pub live: Option<Handle>,
    /// How many times this bin has been chosen for expansion.
    pub visits: u32,
    /// How many distinct states have landed in this bin.
    pub seen: u32,
}

pub struct Archive {
    bins: HashMap<BinKey, Entry>,
    /// Insertion order, so selection can sample without rebuilding a Vec of
    /// keys on every step.
    keys: Vec<BinKey>,
    pub bands: Bands,
    pub max_station: u32,
    /// Best (highest) speed ever observed at each station, over every run we
    /// have made. **This is a self-referential diagnostic and it is the
    /// replacement for "the human does 82 m/s here"**, which this project may
    /// not say. It gets better as our own corpus grows.
    pub best_speed_at: Vec<f32>,
    /// How many states we have ever binned at each station.
    pub visits_at: Vec<u32>,
    /// The earliest tick at which we have ever reached each station, over all
    /// our own runs. Used by selection (a bin that is reachable sooner is a
    /// better place to spend a fork) and reported as a self-referential
    /// diagnostic.
    pub best_ticks_at: Vec<u32>,
}

/// How the frontier is favoured.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// Bins within this many stations of the furthest are "frontier".
    pub frontier_depth: u32,
    /// Probability of drawing from the frontier rather than from everywhere.
    ///
    /// Not 1.0, deliberately. A search that only ever expands the furthest
    /// bins is depth-first and dies in the first cul-de-sac it finds; the
    /// whole value of an archive is that a state abandoned two hundred
    /// stations ago is still there when the frontier turns out to be a trap.
    pub p_frontier: f64,
    /// Weight exponent on visits: `1 / (1 + visits)^decay`. Higher favours
    /// least-visited bins more strongly.
    pub visit_decay: f64,
    /// Ticks of lateness that halve a bin's selection weight, relative to the
    /// earliest we have ever reached that station.
    pub time_halflife: f64,
}

impl Default for Policy {
    fn default() -> Self {
        Policy { frontier_depth: 3, p_frontier: 0.85, visit_decay: 0.5, time_halflife: 200.0 }
    }
}

impl Archive {
    pub fn new(bands: Bands, n_stations: u32) -> Archive {
        Archive {
            bins: HashMap::new(),
            keys: Vec::new(),
            bands,
            max_station: 0,
            best_speed_at: vec![f32::NEG_INFINITY; n_stations as usize + 1],
            visits_at: vec![0; n_stations as usize + 1],
            best_ticks_at: vec![u32::MAX; n_stations as usize + 1],
        }
    }

    pub fn len(&self) -> usize {
        self.bins.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bins.is_empty()
    }
    pub fn get(&self, k: &BinKey) -> Option<&Entry> {
        self.bins.get(k)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&BinKey, &Entry)> {
        self.bins.iter()
    }

    /// Offer a state to the archive.
    ///
    /// Returns `true` if it was kept — either a new bin, or a strictly earlier
    /// arrival in an existing one.
    ///
    /// **Strictly earlier, and nothing else.** Within a bin the states are
    /// already alike in station, lateral offset, height, speed, heading,
    /// contact pattern, airtime regime and checkpoints; what is left to prefer
    /// is arriving sooner. Adding any further tie-break here would be a second
    /// objective smuggled into the archive, and objectives belong on the
    /// command line.
    pub fn offer(&mut self, key: BinKey, e: Entry) -> bool {
        let st = key.station as usize;
        if st < self.best_speed_at.len() {
            let sp = e.state.speed();
            if sp > self.best_speed_at[st] {
                self.best_speed_at[st] = sp;
            }
            self.visits_at[st] += 1;
            if e.ticks < self.best_ticks_at[st] {
                self.best_ticks_at[st] = e.ticks;
            }
        }
        if key.station > self.max_station {
            self.max_station = key.station;
        }
        match self.bins.get_mut(&key) {
            None => {
                self.keys.push(key);
                self.bins.insert(key, e);
                true
            }
            Some(old) => {
                old.seen += 1;
                if e.ticks < old.ticks {
                    let (visits, seen) = (old.visits, old.seen);
                    *old = e;
                    old.visits = visits;
                    old.seen = seen;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Attach or drop a live handle on a bin, without disturbing anything else.
    pub fn set_live(&mut self, key: &BinKey, h: Option<Handle>) {
        if let Some(e) = self.bins.get_mut(key) {
            e.live = h;
        }
    }

    /// Choose a bin to expand.
    ///
    /// Furthest station, then best time, then least-visited — as a *weighting*
    /// and not as a sort, so the policy favours the frontier without ever
    /// being unable to leave it.
    pub fn pick(&mut self, pol: &Policy, rng: &mut Rng) -> Option<BinKey> {
        if self.keys.is_empty() {
            return None;
        }
        let lo = self.max_station.saturating_sub(pol.frontier_depth);
        let use_frontier = rng.f64() < pol.p_frontier;

        let mut total = 0.0f64;
        let mut acc: Vec<(f64, BinKey)> = Vec::with_capacity(self.keys.len());
        for k in &self.keys {
            let e = &self.bins[k];
            if use_frontier && k.station < lo {
                continue;
            }
            // station: exponential preference for the frontier, so a bin two
            // stations back is worth a quarter of one at the edge.
            let back = (self.max_station - k.station) as f64;
            let w_station = 0.5f64.powf(back.min(60.0));
            // visits: least-visited first.
            let w_visit = 1.0 / (1.0 + e.visits as f64).powf(pol.visit_decay);
            // time: among bins at the same station, prefer the one we can
            // reach soonest. Without this the search happily builds on a
            // dawdling prefix and then runs out of tick budget before the
            // finish -- measured on the toy, which stalled at station 23 of 56
            // with the frontier sitting at 37.590 of a 40.000 limit.
            let st = (k.station as usize).min(self.best_ticks_at.len() - 1);
            let floor_t = self.best_ticks_at[st];
            let w_time = if floor_t == u32::MAX {
                1.0
            } else {
                0.5f64.powf(((e.ticks.saturating_sub(floor_t)) as f64 / pol.time_halflife).min(20.0))
            };
            let w = w_station * w_visit * w_time;
            total += w;
            acc.push((total, *k));
        }
        if acc.is_empty() || total <= 0.0 {
            // The frontier draw found nothing (can only happen if the frontier
            // is empty, which it is not) — fall back to uniform rather than
            // returning None, because None would end the search.
            let i = rng.below(self.keys.len() as u64) as usize;
            let k = self.keys[i];
            if let Some(e) = self.bins.get_mut(&k) {
                e.visits += 1;
            }
            return Some(k);
        }
        let t = rng.f64() * total;
        let i = acc.partition_point(|(c, _)| *c < t).min(acc.len() - 1);
        let k = acc[i].1;
        if let Some(e) = self.bins.get_mut(&k) {
            e.visits += 1;
        }
        Some(k)
    }

    /// The furthest-station histogram, coarsened into `n` buckets.
    pub fn station_histogram(&self, n: usize) -> Vec<(u32, u32)> {
        let stations = self.visits_at.len().max(1);
        let per = (stations + n - 1) / n;
        let mut out = Vec::new();
        for b in 0..n {
            let lo = b * per;
            let hi = ((b + 1) * per).min(stations);
            if lo >= hi {
                break;
            }
            let c: u32 = self.visits_at[lo..hi].iter().sum();
            out.push((lo as u32, c));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qfloor_is_a_partition() {
        // Two-sided. A binner that returns a constant passes the first half; a
        // binner that returns the raw value passes the second. Only a real
        // partition passes both.
        let band = 5.0f32;
        for i in -2000..2000 {
            let x = i as f32 * 0.1;
            let y = x + band * 0.999;
            let z = x + band * 1.001;
            // same bin implies closer than a band
            if qfloor(x, band) == qfloor(y, band) {
                assert!((x - y).abs() < band);
            }
            // a full band apart is never the same bin
            assert_ne!(qfloor(x, band), qfloor(z + band, band), "x={}", x);
        }
    }

    #[test]
    fn qfloor_saturates_rather_than_wrapping() {
        assert_eq!(qfloor(f32::INFINITY, 1.0), i16::MAX);
        assert_eq!(qfloor(f32::NAN, 1.0), i16::MAX);
        assert_eq!(qfloor(1e30, 1.0), i16::MAX);
        assert_eq!(qfloor(-1e30, 1.0), i16::MIN);
    }

    #[test]
    fn airtime_buckets_are_ordered_and_grounded_wins() {
        assert_eq!(AirBucket::of(0, 0b1111), AirBucket::Grounded);
        // a car with 500 ticks of stored airtime that is back on the ground is
        // GROUNDED. The bucket is about the car now, not about its history.
        assert_eq!(AirBucket::of(500, 0b0001), AirBucket::Grounded);
        assert_eq!(AirBucket::of(5, 0), AirBucket::Hop);
        assert_eq!(AirBucket::of(20, 0), AirBucket::Flight);
        assert_eq!(AirBucket::of(200, 0), AirBucket::LongFlight);
        assert!(AirBucket::Grounded < AirBucket::LongFlight);
    }
}
