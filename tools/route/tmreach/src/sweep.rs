//! ENTRY SWEEP (coordinator 2026-09-08 22:53Z): from a savestate a few seconds before a jump / climb /
//! wall ride, construct a LADDER of entries — brake ticks (speed) × a lateral lane-change pulse × a
//! heading pulse — fly each with the wheel straight and full gas, and read where it goes: the credits
//! it gains, its end position against the human line (arc length, lateral, height), the apex, and
//! whether it ends stopped. A table instead of waiting for random macros to find the window.
//!
//! The savestate is the seed chain (a previous best, cut back with `--cut`); the approach is `--pre`
//! ticks of gas with the pulses applied (a lane change is a steer pulse followed by its opposite; a
//! heading change is a single pulse at the end of the approach), then `--fly` ticks of gas, wheel
//! centred.

use crate::lap::Track;
use crate::rig::{pos, speed, Worker};
use forkoracle::forksrv::{rec_of, Rec};
use forkoracle::layout::Row;

pub struct SweepCfg {
    /// follow the human line during the approach (pure pursuit) instead of holding the wheel straight;
    /// the lateral pulse becomes a lateral OFFSET target (m, pulse/32) and the heading pulse a steer bias
    pub follow: bool,
    /// keep following the line during the fly ticks too (the pulses then act on the whole run)
    pub fly_follow: bool,
    pub steer_sign: f64,
    pub chain: Vec<Rec>,
    pub cut: usize,
    pub pre: usize,
    pub fly: usize,
    pub brakes: Vec<usize>,
    pub lat_pulses: Vec<i32>,
    pub head_pulses: Vec<i32>,
    pub lat_len: usize,
    pub head_len: usize,
}

pub struct Entry {
    pub brake: usize,
    pub lat: i32,
    pub head: i32,
    pub end: Row,
    pub apex_y: f64,
    pub credits: u8,
    pub s: f64,
    pub lat_m: f64,
    pub dy: f64,
    pub speed_at_pre: f64,
    pub stopped: bool,
    pub rows: usize,
}

fn q(v: i32) -> u8 {
    (v.clamp(-127, 127) as i8) as u8
}

/// One entry's input tape: brake for `b` ticks (gas held), gas after; the lane-change pulse at ticks
/// [b, b+len) and its opposite at [b+len, b+2len); the heading pulse over the last `hlen` ticks of
/// the approach; then straight gas for `fly` ticks.
pub fn entry_recs(cfg: &SweepCfg, b: usize, lat: i32, head: i32) -> Vec<Rec> {
    let mut v = Vec::with_capacity(cfg.pre + cfg.fly);
    for t in 0..cfg.pre {
        let mut steer = 0i32;
        if t >= b && t < b + cfg.lat_len {
            steer = lat;
        } else if t >= b + cfg.lat_len && t < b + 2 * cfg.lat_len {
            steer = -lat;
        }
        if t + cfg.head_len >= cfg.pre {
            steer = head;
        }
        let brake = if t < b { 1 } else { 0 };
        v.push(rec_of(q(steer), 1, brake));
    }
    for _ in 0..cfg.fly {
        v.push(rec_of(0, 1, 0));
    }
    v
}

pub fn run(w: &mut Worker, track: &Track, cfg: &SweepCfg) -> Result<Vec<Entry>, String> {
    let root = w.root_probe;
    let mut chain = cfg.chain.clone();
    let cut = cfg.cut.min(chain.len());
    chain.truncate(chain.len() - cut);
    let (rows, node) = w.rollout_keep(branch::ROOT, &chain, root, chain.len() as u64)?;
    let start = rows.last().cloned().ok_or("the seed chain produced no rows")?;
    let from = w.floor(node)?;
    let cps_of = |r: &Row| -> u8 { if r.cps == u32::MAX { 0 } else { r.cps as u8 } };
    let cps0 = cps_of(&start);
    let (s0, _, _, _) = track.project(pos(&start), track.pts.len() / 2, track.pts.len());
    eprintln!(
        "sweep from tick {} ({} chain ticks, cut {cut}): ({:.1}, {:.1}, {:.1}) v {:.1} cps {cps0} s {s0:.1}; {} entries",
        chain.len(),
        chain.len(),
        start.x,
        start.y,
        start.z,
        speed(&start),
        cfg.brakes.len() * cfg.lat_pulses.len() * cfg.head_pulses.len()
    );
    let mut out = Vec::new();
    for &b in &cfg.brakes {
        for &lat in &cfg.lat_pulses {
            for &head in &cfg.head_pulses {
                let r = if cfg.follow {
                    match follow_entry(w, track, cfg, node, from, b, lat, head) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("  entry b{b} lat{lat} head{head}: {e}");
                            continue;
                        }
                    }
                } else {
                    let recs = entry_recs(cfg, b, lat, head);
                    match w.rollout(node, &recs, from, recs.len() as u64) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("  entry b{b} lat{lat} head{head}: {e}");
                            continue;
                        }
                    }
                };
                let Some(end) = r.rows.last().cloned() else { continue };
                let apex_y = r.rows.iter().map(|x| x.y).fold(f64::MIN, f64::max);
                let speed_at_pre = r.rows.get(cfg.pre.min(r.rows.len().saturating_sub(1))).map(speed).unwrap_or(0.0);
                let (s, lat_m, seg, _) = track.project(pos(&end), track.pts.len() / 2, track.pts.len());
                let _ = seg;
                let dy = end.y - track.at(s)[1];
                out.push(Entry {
                    brake: b,
                    lat,
                    head,
                    credits: cps_of(&end).saturating_sub(cps0),
                    apex_y,
                    s,
                    lat_m,
                    dy,
                    speed_at_pre,
                    stopped: speed(&end) < 2.0,
                    rows: r.rows.len(),
                    end,
                });
            }
        }
    }
    w.release(node);
    Ok(out)
}

pub fn table(entries: &[Entry]) -> String {
    let mut s = String::from("brake_ticks\tlat_pulse\thead_pulse\tspeed_at_launch\tcredits\tend_s\tend_lat\tend_dy\tapex_y\tend_x\tend_y\tend_z\tend_speed\tstopped\trows\n");
    for e in entries {
        s.push_str(&format!(
            "{}\t{}\t{}\t{:.1}\t{}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}\n",
            e.brake, e.lat, e.head, e.speed_at_pre, e.credits, e.s, e.lat_m, e.dy, e.apex_y, e.end.x, e.end.y, e.end.z, speed(&e.end), e.stopped as u8, e.rows
        ));
    }
    s
}

/// A follow-the-line approach: closed-loop pure pursuit in 10-tick chunks for `pre` ticks — brake for
/// `b` ticks first, aim `lat/32` m beside the line (right positive), add `head/127` of steer bias —
/// then `fly` ticks of straight gas. Returns the rows like `Worker::rollout`.
fn follow_entry(w: &mut Worker, track: &Track, cfg: &SweepCfg, node: branch::Handle, from: usize, b: usize, lat: i32, head: i32) -> Result<crate::rig::Rolled, String> {
    let (mut rows, mut cur) = (Vec::new(), node);
    let mut last: Option<Row> = None;
    let mut seg_hint = track.pts.len() / 2;
    let mut window = track.pts.len();
    let mut done = 0usize;
    let mut exited = false;
    let total = if cfg.fly_follow { cfg.pre + cfg.fly } else { cfg.pre };
    while done < total {
        let k = 10.min(total - done);
        let r = match &last {
            Some(r) => r.clone(),
            None => {
                // one tick to read the state at the node
                let (rs, c) = w.rollout_keep(cur, &[rec_of(0, 1, 0)], from + done, 1)?;
                if cur != node {
                    w.release(cur);
                }
                cur = c;
                done += 1;
                rows.extend(rs.iter().cloned());
                let Some(x) = rs.last().cloned() else { break };
                x
            }
        };
        let (s, _l, seg, _d) = track.project(pos(&r), seg_hint, window);
        seg_hint = seg;
        window = 60;
        let v = speed(&r);
        let look = (1.0 * v).clamp(12.0, 45.0);
        let t = track.at(s + look);
        // lateral offset: shift the target sideways (right of the direction of travel)
        let ahead = track.at(s + look + 2.0);
        let dx = ahead[0] - t[0];
        let dz = ahead[2] - t[2];
        let nrm = (dx * dx + dz * dz).sqrt().max(1e-6);
        let off = lat as f64 / 32.0;
        let target = [t[0] + off * dz / nrm, t[1], t[2] - off * dx / nrm];
        let yaw = crate::lap::yaw_of(&r);
        let want = (target[0] - r.x).atan2(target[2] - r.z);
        let delta = crate::lap::wrap(want - yaw);
        let steer = ((cfg.steer_sign * delta / 25f64.to_radians()).clamp(-1.0, 1.0) + head as f64 / 127.0).clamp(-1.0, 1.0);
        let st = (steer * 127.0).round() as i8 as u8;
        let brake = if done < b { 1 } else { 0 };
        let recs: Vec<Rec> = (0..k).map(|_| rec_of(st, 1, brake)).collect();
        match w.forest.advance_or_end(cur, &recs, from + done, k as u64)? {
            branch::Advanced::Node(rs, c) => {
                if cur != node {
                    w.release(cur);
                }
                cur = c;
                last = rs.last().cloned();
                rows.extend(rs);
            }
            branch::Advanced::RunEnded(rs) => {
                rows.extend(rs);
                exited = true;
                break;
            }
        }
        done += k;
    }
    if !exited && !cfg.fly_follow {
        let recs: Vec<Rec> = (0..cfg.fly).map(|_| rec_of(0, 1, 0)).collect();
        let r = w.rollout(cur, &recs, from + done, cfg.fly as u64)?;
        rows.extend(r.rows);
        exited = r.exited;
    }
    if cur != node {
        w.release(cur);
    }
    Ok(crate::rig::Rolled { rows, exited })
}
