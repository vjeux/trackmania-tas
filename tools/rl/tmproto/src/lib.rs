//! tmproto: the rollout-worker wire (ROLLOUT-WORKER.md §1–2), shared by the worker (tmroll) and the master (tmrl).: `len u32 · kind u8 · payload`, little-endian.
//! Encoding and decoding are one table each; a round-trip test pins them.

use std::io::{Read, Write};

pub const PROTO_VERSION: u32 = 1;

pub const K_HELLO: u8 = 0x01;
pub const K_SET_POLICY: u8 = 0x02;
pub const K_RUN_EPISODES: u8 = 0x03;
pub const K_CANCEL: u8 = 0x04;
pub const K_LOAD_MAPS: u8 = 0x05;
pub const K_QUIT: u8 = 0x06;
pub const K_ARCHIVE_LIST: u8 = 0x07;
pub const K_READY: u8 = 0x81;
pub const K_POLICY_ACK: u8 = 0x82;
pub const K_EPISODE: u8 = 0x83;
pub const K_EPISODE_ERROR: u8 = 0x84;
pub const K_STATS: u8 = 0x85;
pub const K_MAP_LOADED: u8 = 0x86;
pub const K_ARCHIVE: u8 = 0x87;

pub const FLAG_ROWS: u8 = 1;
pub const FLAG_TAPE: u8 = 2;
pub const FLAG_ARGMAX: u8 = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeReq {
    pub ep_id: u64,
    pub map_uid: String,
    /// 0 root, 1 a state of the map's persisted StateArchive (`state_id`).
    pub start: u8,
    pub state_id: u64,
    pub seed: u64,
    pub temperature: f32,
    pub max_steps: u32,
    pub k_ticks: u16,
    pub flags: u8,
    /// Offer a policy snapshot to the map's archive every this many ticks (0 = none).
    pub snap_every: u16,
    /// The corridor margin, metres (CoreCfg::offroute_margin; LEARN's default 4).
    pub margin_m: f32,
}

/// `tmrl::ppo::Step`, field for field. The action is the sampled CHUNK: per
/// tick `[steer_bin, gas, brake]` as the bcnet categorical head chose it, k of
/// them -- the PPO ratio recomputes the chunk's log-prob from these.
#[derive(Clone, Debug, PartialEq)]
pub struct StepOut {
    pub obs: Vec<f32>,
    pub action: Vec<[u8; 3]>,
    pub logp: f32,
    pub value: f32,
    pub reward: f32,
    pub terminal: bool,
    pub truncated: bool,
    pub next_value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeOut {
    pub ep_id: u64,
    pub batch_id: u64,
    pub policy_id: u64,
    pub map_uid: String,
    pub seed: u64,
    pub start: u8,
    pub state_id: u64,
    pub obs_version: u32,
    pub obs_dim: u32,
    pub k_ticks: u16,
    /// 0 running(cap) 1 Finished 2 OffRoute 3 NoProgress 4 Crash 5 RunEnded 6 TickCap
    pub done: u8,
    pub truncated: bool,
    pub finish_ms: i32,
    pub gates: u32,
    pub reward_sum: f32,
    /// Furthest saturated progress along the geometry, metres, and the route's length.
    pub best_s: f32,
    pub length_m: f32,
    pub wall_ms: u32,
    pub worker_ticks: u32,
    pub steps: Vec<StepOut>,
    /// STATE_VERSION 3 CarState bytes, one per tick (empty unless asked).
    pub rows: Vec<u8>,
    /// The banked Ghost.Gbx (empty unless asked).
    pub tape: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    Hello { proto_version: u32, master_id: String },
    SetPolicy { policy_id: u64, obs_version: u32, tmw: Vec<u8> },
    RunEpisodes { batch_id: u64, episodes: Vec<EpisodeReq> },
    Cancel { batch_id: u64 },
    LoadMaps { uids: Vec<String>, snap_every: u16 },
    Quit,
    ArchiveList { map_uid: String },
    Ready { proto_version: u32, worker_id: String, n_workers: u32, obs_versions: u32, state_version: u32, git_head: String },
    PolicyAck { policy_id: u64, ok: bool, err: String },
    Episode(Box<EpisodeOut>),
    EpisodeError { ep_id: u64, err: String },
    Stats { batch_id: u64, done: u32, running: u32, queued: u32, env_steps_per_s: f32, load1: f32 },
    MapLoaded { map_uid: String, ok: bool, err: String, n_states: u32 },
    Archive { map_uid: String, entries: Vec<ArchiveEntry> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArchiveEntry {
    pub state_id: u64,
    pub progress_m: f32,
    pub race_ms: i32,
    pub score: f32,
    /// 0 human-line snapshot, 1 policy snapshot.
    pub origin: u8,
}

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) { self.0.push(v); }
    fn u16(&mut self, v: u16) { self.0.extend_from_slice(&v.to_le_bytes()); }
    fn u32(&mut self, v: u32) { self.0.extend_from_slice(&v.to_le_bytes()); }
    fn u64(&mut self, v: u64) { self.0.extend_from_slice(&v.to_le_bytes()); }
    fn i32(&mut self, v: i32) { self.0.extend_from_slice(&v.to_le_bytes()); }
    fn f32(&mut self, v: f32) { self.0.extend_from_slice(&v.to_le_bytes()); }
    fn str(&mut self, s: &str) {
        let b = s.as_bytes();
        self.u16(b.len().min(u16::MAX as usize) as u16);
        self.0.extend_from_slice(&b[..b.len().min(u16::MAX as usize)]);
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u32(b.len() as u32);
        self.0.extend_from_slice(b);
    }
}

struct R<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.b.len() {
            return Err(format!("frame truncated at byte {} (+{n} of {})", self.p, self.b.len()));
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, String> { Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    fn u32(&mut self) -> Result<u32, String> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn u64(&mut self) -> Result<u64, String> { Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap())) }
    fn i32(&mut self) -> Result<i32, String> { Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn f32(&mut self) -> Result<f32, String> { Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn str(&mut self) -> Result<String, String> {
        let n = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
}

impl Frame {
    /// The whole frame, length prefix included.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W(Vec::with_capacity(64));
        w.u32(0); // length, patched below
        match self {
            Frame::Hello { proto_version, master_id } => {
                w.u8(K_HELLO);
                w.u32(*proto_version);
                w.str(master_id);
            }
            Frame::SetPolicy { policy_id, obs_version, tmw } => {
                w.u8(K_SET_POLICY);
                w.u64(*policy_id);
                w.u32(*obs_version);
                w.bytes(tmw);
            }
            Frame::RunEpisodes { batch_id, episodes } => {
                w.u8(K_RUN_EPISODES);
                w.u64(*batch_id);
                w.u32(episodes.len() as u32);
                for e in episodes {
                    w.u64(e.ep_id);
                    w.str(&e.map_uid);
                    w.u8(e.start);
                    w.u64(e.state_id);
                    w.u64(e.seed);
                    w.f32(e.temperature);
                    w.u32(e.max_steps);
                    w.u16(e.k_ticks);
                    w.u8(e.flags);
                    w.u16(e.snap_every);
                    w.f32(e.margin_m);
                }
            }
            Frame::Cancel { batch_id } => {
                w.u8(K_CANCEL);
                w.u64(*batch_id);
            }
            Frame::LoadMaps { uids, snap_every } => {
                w.u8(K_LOAD_MAPS);
                w.u32(uids.len() as u32);
                for u in uids {
                    w.str(u);
                }
                w.u16(*snap_every);
            }
            Frame::Quit => w.u8(K_QUIT),
            Frame::ArchiveList { map_uid } => {
                w.u8(K_ARCHIVE_LIST);
                w.str(map_uid);
            }
            Frame::Ready { proto_version, worker_id, n_workers, obs_versions, state_version, git_head } => {
                w.u8(K_READY);
                w.u32(*proto_version);
                w.str(worker_id);
                w.u32(*n_workers);
                w.u32(*obs_versions);
                w.u32(*state_version);
                w.str(git_head);
            }
            Frame::PolicyAck { policy_id, ok, err } => {
                w.u8(K_POLICY_ACK);
                w.u64(*policy_id);
                w.u8(*ok as u8);
                w.str(err);
            }
            Frame::Episode(e) => {
                w.u8(K_EPISODE);
                w.u64(e.ep_id);
                w.u64(e.batch_id);
                w.u64(e.policy_id);
                w.str(&e.map_uid);
                w.u64(e.seed);
                w.u8(e.start);
                w.u64(e.state_id);
                w.u32(e.obs_version);
                w.u32(e.obs_dim);
                w.u32(e.steps.len() as u32);
                w.u16(e.k_ticks);
                w.u8(e.done);
                w.u8(e.truncated as u8);
                w.i32(e.finish_ms);
                w.u32(e.gates);
                w.f32(e.reward_sum);
                w.f32(e.best_s);
                w.f32(e.length_m);
                w.u32(e.wall_ms);
                w.u32(e.worker_ticks);
                for s in &e.steps {
                    for v in &s.obs {
                        w.f32(*v);
                    }
                    for t in 0..e.k_ticks as usize {
                        let a = s.action.get(t).copied().unwrap_or([0, 0, 0]);
                        w.u8(a[0]);
                        w.u8(a[1]);
                        w.u8(a[2]);
                    }
                    w.f32(s.logp);
                    w.f32(s.value);
                    w.f32(s.reward);
                    w.u8(s.terminal as u8);
                    w.u8(s.truncated as u8);
                    w.f32(s.next_value);
                }
                w.bytes(&e.rows);
                w.bytes(&e.tape);
            }
            Frame::EpisodeError { ep_id, err } => {
                w.u8(K_EPISODE_ERROR);
                w.u64(*ep_id);
                w.str(err);
            }
            Frame::Stats { batch_id, done, running, queued, env_steps_per_s, load1 } => {
                w.u8(K_STATS);
                w.u64(*batch_id);
                w.u32(*done);
                w.u32(*running);
                w.u32(*queued);
                w.f32(*env_steps_per_s);
                w.f32(*load1);
            }
            Frame::MapLoaded { map_uid, ok, err, n_states } => {
                w.u8(K_MAP_LOADED);
                w.str(map_uid);
                w.u8(*ok as u8);
                w.str(err);
                w.u32(*n_states);
            }
            Frame::Archive { map_uid, entries } => {
                w.u8(K_ARCHIVE);
                w.str(map_uid);
                w.u32(entries.len() as u32);
                for e in entries {
                    w.u64(e.state_id);
                    w.f32(e.progress_m);
                    w.i32(e.race_ms);
                    w.f32(e.score);
                    w.u8(e.origin);
                }
            }
        }
        let len = (w.0.len() - 4) as u32;
        w.0[0..4].copy_from_slice(&len.to_le_bytes());
        w.0
    }

    /// Decode one frame's body (`kind · payload`, the length prefix already consumed).
    pub fn decode(body: &[u8]) -> Result<Frame, String> {
        let mut r = R { b: body, p: 0 };
        let kind = r.u8()?;
        let f = match kind {
            K_HELLO => Frame::Hello { proto_version: r.u32()?, master_id: r.str()? },
            K_SET_POLICY => Frame::SetPolicy { policy_id: r.u64()?, obs_version: r.u32()?, tmw: r.bytes()? },
            K_RUN_EPISODES => {
                let batch_id = r.u64()?;
                let n = r.u32()? as usize;
                let mut episodes = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    episodes.push(EpisodeReq {
                        ep_id: r.u64()?,
                        map_uid: r.str()?,
                        start: r.u8()?,
                        state_id: r.u64()?,
                        seed: r.u64()?,
                        temperature: r.f32()?,
                        max_steps: r.u32()?,
                        k_ticks: r.u16()?,
                        flags: r.u8()?,
                        snap_every: r.u16()?,
                        margin_m: r.f32()?,
                    });
                }
                Frame::RunEpisodes { batch_id, episodes }
            }
            K_CANCEL => Frame::Cancel { batch_id: r.u64()? },
            K_LOAD_MAPS => {
                let n = r.u32()? as usize;
                let mut uids = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    uids.push(r.str()?);
                }
                let snap_every = r.u16()?;
                Frame::LoadMaps { uids, snap_every }
            }
            K_QUIT => Frame::Quit,
            K_ARCHIVE_LIST => Frame::ArchiveList { map_uid: r.str()? },
            K_READY => Frame::Ready {
                proto_version: r.u32()?,
                worker_id: r.str()?,
                n_workers: r.u32()?,
                obs_versions: r.u32()?,
                state_version: r.u32()?,
                git_head: r.str()?,
            },
            K_POLICY_ACK => Frame::PolicyAck { policy_id: r.u64()?, ok: r.u8()? != 0, err: r.str()? },
            K_EPISODE => {
                let ep_id = r.u64()?;
                let batch_id = r.u64()?;
                let policy_id = r.u64()?;
                let map_uid = r.str()?;
                let seed = r.u64()?;
                let start = r.u8()?;
                let state_id = r.u64()?;
                let obs_version = r.u32()?;
                let obs_dim = r.u32()?;
                let n_steps = r.u32()? as usize;
                let k_ticks = r.u16()?;
                let done = r.u8()?;
                let truncated = r.u8()? != 0;
                let finish_ms = r.i32()?;
                let gates = r.u32()?;
                let reward_sum = r.f32()?;
                let best_s = r.f32()?;
                let length_m = r.f32()?;
                let wall_ms = r.u32()?;
                let worker_ticks = r.u32()?;
                let mut steps = Vec::with_capacity(n_steps.min(1 << 20));
                for _ in 0..n_steps {
                    let mut obs = Vec::with_capacity(obs_dim as usize);
                    for _ in 0..obs_dim {
                        obs.push(r.f32()?);
                    }
                    let mut action = Vec::with_capacity(k_ticks as usize);
                    for _ in 0..k_ticks {
                        action.push([r.u8()?, r.u8()?, r.u8()?]);
                    }
                    steps.push(StepOut {
                        obs,
                        action,
                        logp: r.f32()?,
                        value: r.f32()?,
                        reward: r.f32()?,
                        terminal: r.u8()? != 0,
                        truncated: r.u8()? != 0,
                        next_value: r.f32()?,
                    });
                }
                let rows = r.bytes()?;
                let tape = r.bytes()?;
                Frame::Episode(Box::new(EpisodeOut {
                    ep_id,
                    batch_id,
                    policy_id,
                    map_uid,
                    seed,
                    start,
                    state_id,
                    obs_version,
                    obs_dim,
                    k_ticks,
                    done,
                    truncated,
                    finish_ms,
                    gates,
                    reward_sum,
                    best_s,
                    length_m,
                    wall_ms,
                    worker_ticks,
                    steps,
                    rows,
                    tape,
                }))
            }
            K_EPISODE_ERROR => Frame::EpisodeError { ep_id: r.u64()?, err: r.str()? },
            K_STATS => Frame::Stats {
                batch_id: r.u64()?,
                done: r.u32()?,
                running: r.u32()?,
                queued: r.u32()?,
                env_steps_per_s: r.f32()?,
                load1: r.f32()?,
            },
            K_MAP_LOADED => Frame::MapLoaded { map_uid: r.str()?, ok: r.u8()? != 0, err: r.str()?, n_states: r.u32()? },
            K_ARCHIVE => {
                let map_uid = r.str()?;
                let n = r.u32()? as usize;
                let mut entries = Vec::with_capacity(n.min(1 << 20));
                for _ in 0..n {
                    entries.push(ArchiveEntry { state_id: r.u64()?, progress_m: r.f32()?, race_ms: r.i32()?, score: r.f32()?, origin: r.u8()? });
                }
                Frame::Archive { map_uid, entries }
            }
            k => return Err(format!("unknown frame kind 0x{k:02x}")),
        };
        if r.p != body.len() {
            return Err(format!("frame kind 0x{kind:02x}: {} trailing bytes", body.len() - r.p));
        }
        Ok(f)
    }
}

/// Read one frame from a stream (blocking). `Ok(None)` at a clean EOF.
pub fn read_frame(s: &mut dyn Read) -> Result<Option<Frame>, String> {
    let mut len = [0u8; 4];
    match s.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.to_string()),
    }
    let n = u32::from_le_bytes(len) as usize;
    if n == 0 || n > (1 << 30) {
        return Err(format!("frame length {n} out of range"));
    }
    let mut body = vec![0u8; n];
    s.read_exact(&mut body).map_err(|e| e.to_string())?;
    Frame::decode(&body).map(Some)
}

pub fn write_frame(s: &mut dyn Write, f: &Frame) -> Result<(), String> {
    s.write_all(&f.encode()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(f: Frame) {
        let b = f.encode();
        let n = u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
        assert_eq!(n, b.len() - 4);
        let g = Frame::decode(&b[4..]).unwrap();
        assert_eq!(f, g);
    }

    #[test]
    fn every_frame_round_trips() {
        rt(Frame::Hello { proto_version: 1, master_id: "m".into() });
        rt(Frame::SetPolicy { policy_id: 7, obs_version: 2, tmw: vec![1, 2, 3] });
        rt(Frame::RunEpisodes {
            batch_id: 3,
            episodes: vec![EpisodeReq {
                ep_id: 1,
                map_uid: "buNzfsVlp2NF2oWtHM3729dEylg".into(),
                start: 1,
                state_id: 9,
                seed: 0xdead_beef,
                temperature: 1.0,
                max_steps: 220,
                k_ticks: 10,
                flags: FLAG_ROWS | FLAG_TAPE,
                snap_every: 50,
                margin_m: 4.0,
            }],
        });
        rt(Frame::ArchiveList { map_uid: "u".into() });
        rt(Frame::Archive { map_uid: "u".into(), entries: vec![ArchiveEntry { state_id: 3, progress_m: 12.5, race_ms: 1200, score: 0.4, origin: 0 }] });
        rt(Frame::Cancel { batch_id: 3 });
        rt(Frame::LoadMaps { uids: vec!["a".into(), "b".into()], snap_every: 50 });
        rt(Frame::Quit);
        rt(Frame::Ready { proto_version: 1, worker_id: "box".into(), n_workers: 96, obs_versions: 7, state_version: 3, git_head: "abc".into() });
        rt(Frame::PolicyAck { policy_id: 7, ok: false, err: "no".into() });
        rt(Frame::Episode(Box::new(EpisodeOut {
            ep_id: 1,
            batch_id: 3,
            policy_id: 7,
            map_uid: "u".into(),
            seed: 5,
            start: 0,
            state_id: 0,
            obs_version: 2,
            obs_dim: 2,
            k_ticks: 2,
            done: 1,
            truncated: false,
            finish_ms: 19811,
            gates: 3,
            reward_sum: 1.5,
            best_s: 100.0,
            length_m: 700.0,
            wall_ms: 12,
            worker_ticks: 1990,
            steps: vec![StepOut { obs: vec![0.5, -1.0], action: vec![[1, 1, 0], [2, 0, 1]], logp: -0.1, value: 0.2, reward: 0.3, terminal: true, truncated: false, next_value: 0.0 }],
            rows: vec![9; 120],
            tape: vec![1, 2],
        })));
        rt(Frame::EpisodeError { ep_id: 1, err: "x".into() });
        rt(Frame::Stats { batch_id: 3, done: 1, running: 2, queued: 3, env_steps_per_s: 100.0, load1: 1.5 });
        rt(Frame::MapLoaded { map_uid: "u".into(), ok: true, err: String::new(), n_states: 40 });
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let mut b = Frame::Quit.encode();
        b.push(0);
        assert!(Frame::decode(&b[4..]).is_err());
    }
}
