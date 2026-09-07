//! `ghost tape census`: what the packet STATE WORD carries, over a corpus.
//!
//! The LEARN arm found (2026-09-06) that stripping the two 200 ms windows of
//! `flags=0x404` from the Summer 2026 - 01 WR turns a 19.538 validation into a
//! DNF: the state word is physics-relevant input, and `Action{steer,gas,brake}`
//! cannot express it. Before an experiment can ask WHAT each word does, the
//! corpus has to say which words exist, how often, where in the race, and
//! beside which steer values. That is this command. It reads only.
//!
//! A tick is PLAIN when its flags are 0 and its word0 is nothing but the packet
//! mode (the respawn bit, word0 bit 5, is a known input and is ignored here) --
//! the same rule `tmcrawl`'s sidecar `state_word_plain` uses, so the two censuses
//! agree on what "non-plain" means.

use gbx::tape::{StateEnc, Tape};
use std::collections::BTreeMap;

/// One ghost's identity as far as the census needs it: parsed from the file
/// name (`<rank>-<ms>.Ghost.Gbx` in the dataset, `p<rank>_<ms>.Ghost.Gbx` in
/// tm-pop) and its parent directory (the map uid in the dataset).
#[derive(Clone, Debug)]
pub struct GhostId {
    pub path: String,
    pub map: String,
    pub rank: u32,
    pub declared_ms: i64,
}

pub fn parse_id(path: &str) -> GhostId {
    let p = std::path::Path::new(path);
    let stem = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let stem = stem.trim_end_matches(".Ghost.Gbx").trim_end_matches(".Replay.Gbx").to_string();
    let mut dir = p.parent();
    if dir.and_then(|d| d.file_name()).map(|s| s == "ghosts").unwrap_or(false) {
        dir = dir.and_then(|d| d.parent());
    }
    let map = dir
        .and_then(|d| d.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let (rank, ms) = if let Some(rest) = stem.strip_prefix('p') {
        // p00001_19538
        let mut it = rest.splitn(2, '_');
        let r = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let m = it.next().and_then(|s| s.parse().ok()).unwrap_or(-1);
        (r, m)
    } else {
        let mut it = stem.splitn(2, '-');
        let r = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let m = it.next().and_then(|s| s.parse().ok()).unwrap_or(-1);
        (r, m)
    };
    GhostId { path: path.to_string(), map, rank, declared_ms: ms }
}

pub fn rank_bucket(r: u32) -> &'static str {
    match r {
        0 => "?",
        1..=10 => "1-10",
        11..=100 => "11-100",
        101..=1000 => "101-1000",
        _ => "1001+",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Word {
    /// word0 with the respawn bit cleared (the mode is in its low nibble).
    pub word0: u32,
    pub flags: u32,
}

impl Word {
    pub fn of(word0: u32, flags: u32) -> Word {
        Word { word0: word0 & !0x20, flags }
    }
    pub fn is_plain(&self) -> bool {
        self.flags == 0 && self.word0 == (self.word0 & 0xF)
    }
    pub fn label(&self) -> String {
        if self.flags != 0 && self.word0 == (self.word0 & 0xF) {
            format!("flags 0x{:x}", self.flags)
        } else if self.flags == 0 {
            format!("word0 0x{:x}", self.word0)
        } else {
            format!("word0 0x{:x} flags 0x{:x}", self.word0, self.flags)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Phase {
    Countdown,
    Race,
    PostFinish,
}

fn steer_class(s: i8) -> usize {
    match s {
        0 => 0,
        127 | -127 => 1,
        _ => 2,
    }
}
const STEER_CLASS: [&str; 3] = ["0", "±127", "partial"];

/// One maximal run of ticks carrying the same non-plain word.
#[derive(Clone, Debug)]
pub struct Run {
    pub ghost: usize,
    pub word: Word,
    pub tick_from: usize,
    pub tick_to: usize, // exclusive
    pub race_ms_from: i64,
    pub phase: Phase,
    /// Steer on the tick before the onset, at the onset, at the last tick of
    /// the run and on the tick after it.
    pub steer_before: i8,
    pub steer_on: i8,
    pub steer_last: i8,
    pub steer_after: i8,
    pub accel_on: u8,
    pub brake_on: u8,
    /// Ticks inside the run by steer class (0 / ±127 / partial).
    pub steer_hist: [u32; 3],
    /// Number of steer changes strictly inside the run.
    pub steer_changes: u32,
    /// Does the steer change within ±2 ticks of the onset / of the end?
    pub steer_edge_on: bool,
    pub steer_edge_off: bool,
    /// The state literal was coded explicitly on the first tick (as opposed
    /// to inherited through prev/prev2).
    pub onset_lit: bool,
    pub respawn_in_run: bool,
    /// Ticks from the last steer change before the onset to the onset (the
    /// onset tick itself counts as 0 when the steer changes there), and from
    /// the end of the run to the next steer change after it. usize::MAX = none.
    pub since_change: usize,
    pub until_change: usize,
    /// The APPLIED steer scale the telemetry records while the tape holds ±127,
    /// in the segment before this event's onset and in the segment from its
    /// onset to the next event: (mode of |steer| rounded to 0.05, samples used,
    /// distinct values). NaN / 0 when no full-lock sample exists there.
    pub scale_before: (f32, u32, u32),
    pub scale_after: (f32, u32, u32),
}

#[derive(Clone, Debug)]
pub struct GhostRow {
    pub id: GhostId,
    pub ticks: usize,
    pub race_ticks: usize,
    pub start_offset_ms: i32,
    pub format_version: u32,
    /// Share of in-race ticks whose steer is exactly 0 or ±127.
    pub digital_frac: f64,
    pub digital: bool,
    pub lits: usize,
    pub prev2: usize,
    pub nonplain_race_ticks: usize,
    pub runs: usize,
    pub words: Vec<Word>,
    pub respawns: u32,
    pub err: Option<String>,
}

pub struct Census {
    pub ghosts: Vec<GhostRow>,
    pub runs: Vec<Run>,
    /// Steer class histogram over ALL in-race ticks, per ghost class.
    pub steer_all: [[u64; 3]; 2],
    /// Steer-change rate baseline: changes per in-race tick.
    pub steer_change_rate: f64,
}

pub fn scan(files: &[String], digital_bar: f64) -> Census {
    let mut ghosts = Vec::new();
    let mut runs = Vec::new();
    let mut steer_all = [[0u64; 3]; 2];
    let mut chg = 0u64;
    let mut chg_n = 0u64;
    for f in files {
        let id = parse_id(f);
        let t = match Tape::from_file(f) {
            Ok(t) => t,
            Err(e) => {
                ghosts.push(GhostRow {
                    id,
                    ticks: 0,
                    race_ticks: 0,
                    start_offset_ms: 0,
                    format_version: 0,
                    digital_frac: f64::NAN,
                    digital: false,
                    lits: 0,
                    prev2: 0,
                    nonplain_race_ticks: 0,
                    runs: 0,
                    words: Vec::new(),
                    respawns: 0,
                    err: Some(e),
                });
                continue;
            }
        };
        let gi = ghosts.len();
        let ar = match t.archives.first() {
            Some(a) => a,
            None => continue,
        };
        let n = ar.packets.len();
        let steer: Vec<i8> = ar.packets.iter().map(|p| p.steer_i8()).collect();
        let phase_of = |tick: usize| -> Phase {
            let ms = ar.start_offset_ms as i64 + 10 * tick as i64;
            if ms < 0 {
                Phase::Countdown
            } else if id.declared_ms >= 0 && ms >= id.declared_ms {
                Phase::PostFinish
            } else {
                Phase::Race
            }
        };
        // digital classification over in-race ticks
        let mut race_ticks = 0usize;
        let mut dig = 0usize;
        let mut hist = [0u64; 3];
        for i in 0..n {
            if phase_of(i) == Phase::Race {
                race_ticks += 1;
                let c = steer_class(steer[i]);
                hist[c] += 1;
                if c != 2 {
                    dig += 1;
                }
                if i > 0 && steer[i] != steer[i - 1] {
                    chg += 1;
                }
                chg_n += 1;
            }
        }
        let digital_frac = if race_ticks > 0 { dig as f64 / race_ticks as f64 } else { f64::NAN };
        let digital = race_ticks > 0 && digital_frac >= digital_bar;
        for c in 0..3 {
            steer_all[digital as usize][c] += hist[c];
        }
        let lits = ar.packets.iter().filter(|p| matches!(p.state, StateEnc::Lit(_))).count();
        let prev2 = ar.packets.iter().filter(|p| matches!(p.state, StateEnc::Prev2(..))).count();
        let mut respawns = 0u32;
        let mut prev_rs = false;
        for p in &ar.packets {
            if p.respawn() && !prev_rs {
                respawns += 1;
            }
            prev_rs = p.respawn();
        }
        // runs
        let mut words: Vec<Word> = Vec::new();
        let mut nonplain_race = 0usize;
        let mut nruns = 0usize;
        let mut i = 0usize;
        while i < n {
            let w = Word::of(ar.packets[i].word0, ar.packets[i].flags);
            if w.is_plain() {
                i += 1;
                continue;
            }
            let mut j = i;
            while j < n && Word::of(ar.packets[j].word0, ar.packets[j].flags) == w {
                j += 1;
            }
            let mut sh = [0u32; 3];
            let mut sc = 0u32;
            let mut rs = false;
            for k in i..j {
                sh[steer_class(steer[k])] += 1;
                if k > i && steer[k] != steer[k - 1] {
                    sc += 1;
                }
                if ar.packets[k].respawn() {
                    rs = true;
                }
                if phase_of(k) == Phase::Race {
                    nonplain_race += 1;
                }
            }
            let edge = |c: usize| -> bool {
                let lo = c.saturating_sub(2);
                let hi = (c + 2).min(n - 1);
                (lo..hi).any(|k| steer[k] != steer[k + 1])
            };
            let r = Run {
                ghost: gi,
                word: w,
                tick_from: i,
                tick_to: j,
                race_ms_from: ar.start_offset_ms as i64 + 10 * i as i64,
                phase: phase_of(i),
                steer_before: if i > 0 { steer[i - 1] } else { steer[i] },
                steer_on: steer[i],
                steer_last: steer[j - 1],
                steer_after: if j < n { steer[j] } else { steer[j - 1] },
                accel_on: ar.packets[i].accel as u8,
                brake_on: ar.packets[i].brake as u8,
                steer_hist: sh,
                steer_changes: sc,
                steer_edge_on: edge(i),
                steer_edge_off: edge(j.min(n - 1)),
                onset_lit: matches!(ar.packets[i].state, StateEnc::Lit(_)),
                respawn_in_run: rs,
                since_change: {
                    let mut k = i;
                    let mut d = usize::MAX;
                    while k > 0 {
                        if steer[k] != steer[k - 1] {
                            d = i - k;
                            break;
                        }
                        k -= 1;
                    }
                    d
                },
                until_change: {
                    let mut k = j.saturating_sub(1);
                    let mut d = usize::MAX;
                    while k + 1 < n {
                        if steer[k + 1] != steer[k] {
                            d = k + 1 - j;
                            break;
                        }
                        k += 1;
                    }
                    d
                },
                scale_before: (f32::NAN, 0, 0),
                scale_after: (f32::NAN, 0, 0),
            };
            if !words.contains(&w) {
                words.push(w);
            }
            nruns += 1;
            runs.push(r);
            i = j;
        }
        let lo = runs.len() - nruns;
        fill_scales(f, ar, &steer, &mut runs[lo..]);
        ghosts.push(GhostRow {
            id,
            ticks: n,
            race_ticks,
            start_offset_ms: ar.start_offset_ms,
            format_version: ar.format_version,
            digital_frac,
            digital,
            lits,
            prev2,
            nonplain_race_ticks: nonplain_race,
            runs: nruns,
            words,
            respawns,
            err: None,
        });
    }
    Census {
        ghosts,
        runs,
        steer_all,
        steer_change_rate: if chg_n > 0 { chg as f64 / chg_n as f64 } else { 0.0 },
    }
}

fn pct(a: usize, b: usize) -> String {
    if b == 0 {
        "-".into()
    } else {
        format!("{:.1} %", 100.0 * a as f64 / b as f64)
    }
}

fn median(v: &mut Vec<usize>) -> usize {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[v.len() / 2]
}

pub fn render(c: &Census) -> String {
    let mut s = String::new();
    let ok: Vec<&GhostRow> = c.ghosts.iter().filter(|g| g.err.is_none()).collect();
    let bad = c.ghosts.len() - ok.len();
    let n_maps = {
        let mut m: Vec<&str> = ok.iter().map(|g| g.id.map.as_str()).collect();
        m.sort();
        m.dedup();
        m.len()
    };
    let race_ticks: usize = ok.iter().map(|g| g.race_ticks).sum();
    let nonplain_ticks: usize = ok.iter().map(|g| g.nonplain_race_ticks).sum();
    let nonplain_ghosts = ok.iter().filter(|g| g.nonplain_race_ticks > 0).count();
    let digital = ok.iter().filter(|g| g.digital).count();
    s.push_str(&format!(
        "# Packet state-word census\n\n{} ghosts read ({} unreadable), {} maps, {} in-race ticks.\n\n",
        ok.len(),
        bad,
        n_maps,
        race_ticks
    ));
    s.push_str(&format!(
        "- ghosts with a non-plain word inside the race: **{}** ({}); non-plain in-race ticks: **{}** ({}).\n",
        nonplain_ghosts,
        pct(nonplain_ghosts, ok.len()),
        nonplain_ticks,
        pct(nonplain_ticks, race_ticks)
    ));
    s.push_str(&format!(
        "- 'digital' ghosts (≥ bar of in-race steer ticks exactly 0/±127): **{}** of {} ({}).\n",
        digital,
        ok.len(),
        pct(digital, ok.len())
    ));
    s.push_str(&format!(
        "- steer classes over in-race ticks — analog ghosts: 0 {} / ±127 {} / partial {}; digital ghosts: 0 {} / ±127 {} / partial {}.\n",
        c.steer_all[0][0], c.steer_all[0][1], c.steer_all[0][2], c.steer_all[1][0], c.steer_all[1][1], c.steer_all[1][2]
    ));
    s.push_str(&format!(
        "- baseline steer-change rate: {:.4} per in-race tick (P(a change within ±2 ticks of a random tick) ≈ {:.3}).\n",
        c.steer_change_rate,
        1.0 - (1.0 - c.steer_change_rate).powi(4)
    ));
    let fv: BTreeMap<u32, usize> = ok.iter().fold(BTreeMap::new(), |mut m, g| {
        *m.entry(g.format_version).or_default() += 1;
        m
    });
    s.push_str(&format!("- archive format versions: {:?}\n", fv));
    let p2 = ok.iter().filter(|g| g.prev2 > 0).count();
    s.push_str(&format!(
        "- ghosts using the `prev2` (flag bits 0/1 override) state coding: {} ({} packets total).\n\n",
        p2,
        ok.iter().map(|g| g.prev2).sum::<usize>()
    ));

    // by rank bucket
    s.push_str("## Non-plain words by rank bucket\n\n| bucket | ghosts | with non-plain word in race | in-race ticks non-plain | digital ghosts |\n|---|---:|---:|---:|---:|\n");
    for b in ["1-10", "11-100", "101-1000", "1001+", "?"] {
        let gs: Vec<&&GhostRow> = ok.iter().filter(|g| rank_bucket(g.id.rank) == b).collect();
        if gs.is_empty() {
            continue;
        }
        let np = gs.iter().filter(|g| g.nonplain_race_ticks > 0).count();
        let rt: usize = gs.iter().map(|g| g.race_ticks).sum();
        let nt: usize = gs.iter().map(|g| g.nonplain_race_ticks).sum();
        let dg = gs.iter().filter(|g| g.digital).count();
        s.push_str(&format!(
            "| {} | {} | {} ({}) | {} ({}) | {} ({}) |\n",
            b,
            gs.len(),
            np,
            pct(np, gs.len()),
            nt,
            pct(nt, rt),
            dg,
            pct(dg, gs.len())
        ));
    }

    // word table
    struct Agg {
        ghosts: Vec<usize>,
        runs: usize,
        ticks: usize,
        phase: [usize; 3],
        lens: Vec<usize>,
        steer_on: [usize; 3],
        steer_hist: [u64; 3],
        edge_on: usize,
        edge_off: usize,
        changes_in: usize,
        digital_ghosts: Vec<usize>,
        buckets: BTreeMap<&'static str, usize>,
        onset_lit: usize,
        respawn: usize,
        gas_on: usize,
        brake_on: usize,
        race_ms: Vec<usize>,
        since: Vec<usize>,
        until: Vec<usize>,
    }
    let mut agg: BTreeMap<Word, Agg> = BTreeMap::new();
    for r in &c.runs {
        let a = agg.entry(r.word).or_insert_with(|| Agg {
            ghosts: Vec::new(),
            runs: 0,
            ticks: 0,
            phase: [0; 3],
            lens: Vec::new(),
            steer_on: [0; 3],
            steer_hist: [0; 3],
            edge_on: 0,
            edge_off: 0,
            changes_in: 0,
            digital_ghosts: Vec::new(),
            buckets: BTreeMap::new(),
            onset_lit: 0,
            respawn: 0,
            gas_on: 0,
            brake_on: 0,
            race_ms: Vec::new(),
            since: Vec::new(),
            until: Vec::new(),
        });
        if !a.ghosts.contains(&r.ghost) {
            a.ghosts.push(r.ghost);
            if c.ghosts[r.ghost].digital {
                a.digital_ghosts.push(r.ghost);
            }
            *a.buckets.entry(rank_bucket(c.ghosts[r.ghost].id.rank)).or_default() += 1;
        }
        a.runs += 1;
        let len = r.tick_to - r.tick_from;
        a.ticks += len;
        a.phase[r.phase as usize] += 1;
        a.lens.push(len);
        a.steer_on[steer_class(r.steer_on)] += 1;
        for k in 0..3 {
            a.steer_hist[k] += r.steer_hist[k] as u64;
        }
        a.edge_on += r.steer_edge_on as usize;
        a.edge_off += r.steer_edge_off as usize;
        a.changes_in += (r.steer_changes > 0) as usize;
        a.onset_lit += r.onset_lit as usize;
        a.respawn += r.respawn_in_run as usize;
        a.gas_on += r.accel_on as usize;
        a.brake_on += r.brake_on as usize;
        a.race_ms.push(r.race_ms_from.max(0) as usize);
        if r.since_change != usize::MAX {
            a.since.push(r.since_change);
        }
        if r.until_change != usize::MAX {
            a.until.push(r.until_change);
        }
    }
    let mut keys: Vec<&Word> = agg.keys().collect();
    keys.sort_by_key(|w| std::cmp::Reverse(agg[w].ghosts.len()));
    s.push_str("\n## Distinct non-plain words (sorted by ghosts carrying them)\n\n");
    s.push_str("| word | ghosts | of which digital | runs | ticks | runs countdown/race/post | run len min/med/max (ticks) | steer at onset 0/±127/partial | ticks in run 0/±127/partial | steer edge at onset | steer edge at end | steer changes inside | onset is explicit literal | gas on at onset | brake on at onset | ghosts by rank 1-10/11-100/101-1000/1001+ | onset race s min/med | runs in first 0.5 s | ticks since last steer change min/med/max | ticks to next steer change min/med/max |\n|---|---:|---:|---:|---:|---|---|---|---|---:|---:|---:|---:|---:|---:|---|---|---:|---|---|\n");
    for w in &keys {
        let a = &agg[w];
        let mut lens = a.lens.clone();
        let med = median(&mut lens);
        let mn = *lens.first().unwrap_or(&0);
        let mx = *lens.last().unwrap_or(&0);
        let b = |k: &str| a.buckets.get(k).copied().unwrap_or(0);
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {}/{}/{} | {}/{}/{} | {}/{}/{} | {}/{}/{} | {} | {} | {} | {} | {} | {} | {}/{}/{}/{} | {} | {} | {} | {} |\n",
            w.label(),
            a.ghosts.len(),
            a.digital_ghosts.len(),
            a.runs,
            a.ticks,
            a.phase[0],
            a.phase[1],
            a.phase[2],
            mn,
            med,
            mx,
            a.steer_on[0],
            a.steer_on[1],
            a.steer_on[2],
            a.steer_hist[0],
            a.steer_hist[1],
            a.steer_hist[2],
            pct(a.edge_on, a.runs),
            pct(a.edge_off, a.runs),
            pct(a.changes_in, a.runs),
            pct(a.onset_lit, a.runs),
            pct(a.gas_on, a.runs),
            pct(a.brake_on, a.runs),
            b("1-10"),
            b("11-100"),
            b("101-1000"),
            b("1001+"),
            {
                let mut v = a.race_ms.clone();
                let m = median(&mut v);
                format!("{:.3}/{:.3}", *v.first().unwrap_or(&0) as f64 / 1000.0, m as f64 / 1000.0)
            },
            a.race_ms.iter().filter(|m| **m < 500).count(),
            {
                let mut v = a.since.clone();
                let m = median(&mut v);
                format!("{}/{}/{}", v.first().copied().unwrap_or(0), m, v.last().copied().unwrap_or(0))
            },
            {
                let mut v = a.until.clone();
                let m = median(&mut v);
                format!("{}/{}/{}", v.first().copied().unwrap_or(0), m, v.last().copied().unwrap_or(0))
            }
        ));
    }

    // bit census over non-plain in-race ticks
    s.push_str("\n## Flag bits (over non-plain runs, weighted by runs)\n\n| bit | flags mask | literal bit | runs | ghosts |\n|---|---|---|---:|---:|\n");
    for bit in 0..22 {
        let m = 1u32 << bit;
        let runs = c.runs.iter().filter(|r| r.word.flags & m != 0).count();
        if runs == 0 {
            continue;
        }
        let mut g: Vec<usize> = c.runs.iter().filter(|r| r.word.flags & m != 0).map(|r| r.ghost).collect();
        g.sort_unstable();
        g.dedup();
        s.push_str(&format!("| flags bit {} | 0x{:x} | {} | {} | {} |\n", bit, m, bit + 5, runs, g.len()));
    }
    for bit in 4..12 {
        if bit == 5 {
            continue;
        }
        let m = 1u32 << bit;
        let runs = c.runs.iter().filter(|r| r.word.word0 & m != 0).count();
        if runs == 0 {
            continue;
        }
        let mut g: Vec<usize> = c.runs.iter().filter(|r| r.word.word0 & m != 0).map(|r| r.ghost).collect();
        g.sort_unstable();
        g.dedup();
        s.push_str(&format!("| word0 bit {} | word0 0x{:x} | - | {} | {} |\n", bit, m, runs, g.len()));
    }

    // applied steer scale transitions per word (telemetry, full-lock samples)
    s.push_str("\n## Applied steer scale (telemetry |steer| while the tape holds ±127) before → after each event\n\nOnly events with ≥ 3 full-lock samples on both sides. `x→y (n)`.\n\n| word | transitions (before→after: runs) |\n|---|---|\n");
    for w in &keys {
        let mut tr: BTreeMap<String, usize> = BTreeMap::new();
        for r in c.runs.iter().filter(|r| r.word == **w) {
            if r.scale_before.1 >= 3 && r.scale_after.1 >= 3 {
                *tr.entry(format!("{:.2}→{:.2}", r.scale_before.0, r.scale_after.0)).or_default() += 1;
            }
        }
        if tr.is_empty() {
            continue;
        }
        let mut v: Vec<(String, usize)> = tr.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let txt: Vec<String> = v.iter().map(|(k, n)| format!("{k} ({n})")).collect();
        s.push_str(&format!("| `{}` | {} |\n", w.label(), txt.join(", ")));
    }

    // co-occurrence: words appearing in the same ghost
    s.push_str("\n## Words per ghost\n\n| distinct non-plain words in ghost | ghosts |\n|---:|---:|\n");
    let mut per: BTreeMap<usize, usize> = BTreeMap::new();
    for g in &ok {
        *per.entry(g.words.len()).or_default() += 1;
    }
    for (k, v) in per {
        s.push_str(&format!("| {} | {} |\n", k, v));
    }
    s
}

pub fn runs_tsv(c: &Census) -> String {
    let mut s = String::from("file\tmap\trank\tdeclared_ms\tdigital\tword0\tflags\tlabel\ttick_from\ttick_to\tlen\trace_ms_from\tphase\tsteer_before\tsteer_on\tsteer_last\tsteer_after\taccel_on\tbrake_on\tsteer0\tsteer127\tsteerpartial\tsteer_changes\tedge_on\tedge_off\tonset_lit\trespawn\tsince_change\tuntil_change\tscale_before\tn_before\tscale_after\tn_after\tdistinct_after\n");
    for r in &c.runs {
        let g = &c.ghosts[r.ghost];
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t0x{:x}\t0x{:x}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\t{:.2}\t{}\t{}\n",
            g.id.path,
            g.id.map,
            g.id.rank,
            g.id.declared_ms,
            g.digital as u8,
            r.word.word0,
            r.word.flags,
            r.word.label(),
            r.tick_from,
            r.tick_to,
            r.tick_to - r.tick_from,
            r.race_ms_from,
            r.phase,
            r.steer_before,
            r.steer_on,
            r.steer_last,
            r.steer_after,
            r.accel_on,
            r.brake_on,
            r.steer_hist[0],
            r.steer_hist[1],
            r.steer_hist[2],
            r.steer_changes,
            r.steer_edge_on as u8,
            r.steer_edge_off as u8,
            r.onset_lit as u8,
            r.respawn_in_run as u8,
            r.since_change as i64,
            r.until_change as i64,
            r.scale_before.0,
            r.scale_before.1,
            r.scale_after.0,
            r.scale_after.1,
            r.scale_after.2
        ));
    }
    s
}

pub fn ghosts_tsv(c: &Census) -> String {
    let mut s = String::from("file\tmap\trank\tdeclared_ms\tticks\trace_ticks\tstart_offset_ms\tformat\tdigital_frac\tdigital\tlits\tprev2\tnonplain_race_ticks\truns\twords\trespawns\terr\n");
    for g in &c.ghosts {
        let words: Vec<String> = g.words.iter().map(|w| w.label()).collect();
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            g.id.path,
            g.id.map,
            g.id.rank,
            g.id.declared_ms,
            g.ticks,
            g.race_ticks,
            g.start_offset_ms,
            g.format_version,
            g.digital_frac,
            g.digital as u8,
            g.lits,
            g.prev2,
            g.nonplain_race_ticks,
            g.runs,
            words.join(";"),
            g.respawns,
            g.err.clone().unwrap_or_default()
        ));
    }
    s
}

/// `ghost tape census FILE|DIR... [--digital-bar F] [--runs OUT.tsv] [--ghosts OUT.tsv] [--md OUT.md]`
pub fn cmd(a: &[String]) {
    use crate::cli::{die, flag, has};
    let _ = has;
    let mut files = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if a[i].starts_with("--") {
            i += 2;
            continue;
        }
        collect(&a[i], &mut files);
        i += 1;
    }
    if files.is_empty() {
        die("ghost tape census FILE|DIR... [--digital-bar 0.98] [--runs R.tsv] [--ghosts G.tsv] [--md OUT.md]");
    }
    files.sort();
    let bar: f64 = flag(a, "--digital-bar").map(|v| v.parse().unwrap_or_else(|_| die("--digital-bar wants a number"))).unwrap_or(0.98);
    let c = scan(&files, bar);
    let md = render(&c);
    match flag(a, "--md") {
        Some(o) => std::fs::write(o, &md).unwrap_or_else(|e| die(format!("{o}: {e}"))),
        None => print!("{md}"),
    }
    if let Some(o) = flag(a, "--runs") {
        std::fs::write(o, runs_tsv(&c)).unwrap_or_else(|e| die(format!("{o}: {e}")));
    }
    if let Some(o) = flag(a, "--ghosts") {
        std::fs::write(o, ghosts_tsv(&c)).unwrap_or_else(|e| die(format!("{o}: {e}")));
    }
    eprintln!("{} files, {} runs", files.len(), c.runs.len());
}

fn collect(p: &str, out: &mut Vec<String>) {
    let path = std::path::Path::new(p);
    if path.is_dir() {
        if let Ok(rd) = std::fs::read_dir(path) {
            for e in rd.flatten() {
                collect(&e.path().to_string_lossy(), out);
            }
        }
    } else if p.ends_with(".Ghost.Gbx") || p.ends_with(".Replay.Gbx") {
        out.push(p.to_string());
    }
}

/// The telemetry's applied steer (byte 14 of each 50 ms sample, `Sample.steer`)
/// while the tape holds full lock, per segment between events. Aligned at shift
/// 0: sample t carries the input of the tick that starts at race t (DATA arm's
/// echo measurement), so the tape tick is `(t - start_offset_ms) / 10`.
fn fill_scales(path: &str, ar: &gbx::tape::Archive, steer: &[i8], runs: &mut [Run]) {
    if runs.is_empty() {
        return;
    }
    let dec = match gbx::record::decode_ghost(path) {
        Ok(d) => d,
        Err(_) => return,
    };
    if dec.sample_period_ms.map(|p| p != 50).unwrap_or(false) {
        return; // a multi-car record: the picked entity is not necessarily the driver
    }
    let n = steer.len();
    let mut onsets: Vec<usize> = runs.iter().map(|r| r.tick_from).collect();
    onsets.sort_unstable();
    onsets.dedup();
    // segment boundaries: [0, onsets..., n]
    let mut bounds = vec![0usize];
    bounds.extend(onsets.iter().copied());
    bounds.push(n);
    let seg_of = |tick: usize| -> usize { bounds.iter().rposition(|b| *b <= tick).unwrap_or(0) };
    let nseg = bounds.len() - 1;
    let mut vals: Vec<Vec<i32>> = vec![Vec::new(); nseg];
    for s in &dec.samples {
        let t = s.time_ms as i64 - ar.start_offset_ms as i64;
        if t < 0 || t % 10 != 0 {
            continue;
        }
        let tick = (t / 10) as usize;
        if tick >= n || (steer[tick] != 127 && steer[tick] != -127) {
            continue;
        }
        let v = (s.steer.abs() * 20.0).round() as i32; // units of 0.05
        let k = seg_of(tick).min(nseg - 1);
        vals[k].push(v);
    }
    let summarize = |v: &Vec<i32>| -> (f32, u32, u32) {
        if v.is_empty() {
            return (f32::NAN, 0, 0);
        }
        let mut c: BTreeMap<i32, u32> = BTreeMap::new();
        for x in v {
            *c.entry(*x).or_default() += 1;
        }
        let (mode, _) = c.iter().max_by_key(|(_, n)| **n).unwrap();
        (*mode as f32 / 20.0, v.len() as u32, c.len() as u32)
    };
    for r in runs.iter_mut() {
        let k = bounds.iter().position(|b| *b == r.tick_from).unwrap_or(0);
        // segment k starts at this onset; segment k-1 ends at it
        r.scale_after = summarize(&vals[k.min(nseg - 1)]);
        r.scale_before = if k >= 1 { summarize(&vals[k - 1]) } else { (f32::NAN, 0, 0) };
    }
}
