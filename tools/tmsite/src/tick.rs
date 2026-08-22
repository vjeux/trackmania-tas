//! TICK ("Trackmania Input Control Kit") input-script export, and the
//! round-trip check that the script really reproduces the ghost.
//!
//! Grammar accepted by TICK 1.0.13 (established in `docs/TICK_TEARDOWN.md` by
//! probing its parser):
//!
//! ```text
//! <ms> accel  0|1|up|down
//! <ms> brake  0|1|up|down
//! <ms> steer  -127..127 | left | right
//! <ms> respawn | srespawn
//! <ms> seed <uint32>
//! <ms> flags <uint32>
//! ```
//!
//! `<ms>` must be a multiple of 10; only *changes* are emitted; `#` starts a
//! comment; duplicate/out-of-order lines are accepted with last-wins per
//! (action, tick).
//!
//! The ghost side of this file is the shared `gbx` crate's `tape` module: one
//! decoder for the whole workspace.

use gbx::container::Container;
use gbx::tape::{self, Packet, Tape};

/// The standing-respawn bit of `word0`. `gbx::tape::Packet::respawn()` covers
/// bit 5 (0x20, literal bit 31); this one has no accessor there because only
/// the exporter cares about the distinction.
pub const STANDING_RESPAWN_BIT: u32 = 0x1000;

pub fn is_standing_respawn(p: &Packet) -> bool {
    p.word0 & STANDING_RESPAWN_BIT != 0
}

/// The ghost stores steer as a raw byte; TICK wants it signed.
pub fn sgn(v: u32) -> i32 {
    let v = (v & 0xFF) as i32;
    if v > 127 {
        v - 256
    } else {
        v
    }
}

/// A ghost as the exporter needs it: the input tape plus the declared race
/// time, which lives outside the tape in chunk `0x03092005`.
pub struct Loaded {
    pub tape: Tape,
    pub race_time_ms: Option<u32>,
}

impl Loaded {
    pub fn load(path: &str) -> Result<Loaded, String> {
        let c = Container::load(path)?;
        let inputs = tape::find_inputs_chunk(c.body())
            .ok_or_else(|| format!("no 0x0309201D input chunk in {}", path))?;
        let tape = Tape::from_body(c.body())?;
        // The declared time is the LAST 0x03092005 before the input chunk: a
        // replay carries one per ghost it holds, and a file built on a borrowed
        // carrier has been caught with the carrier's value in one of them.
        let race_time_ms = c
            .declared_times()
            .into_iter()
            .filter(|(off, _)| *off < inputs.0)
            .last()
            .map(|(_, v)| v);
        Ok(Loaded { tape, race_time_ms })
    }
}

pub struct Opts {
    pub path: String,
    pub archive: usize,
    /// Emit `-128` verbatim (what the Python did) instead of clamping it to the
    /// -127 TICK accepts.
    pub raw: bool,
    /// Optional `0 seed <n>` line. The Python read `template.validation_seed`,
    /// which never existed, so it never emitted one.
    pub seed: Option<u32>,
}

pub struct Export {
    pub text: String,
    pub ticks: usize,
    pub start_offset_ms: i32,
    pub race_time_ms: Option<u32>,
    /// Ticks whose steer byte was 0x80 (-128), outside TICK's -127..127.
    pub out_of_range: Vec<usize>,
    /// Ticks carrying the respawn input (state-literal bit 31).
    pub respawns: Vec<usize>,
    /// Ticks carrying the standing-respawn input (`word0 & 0x1000`).
    pub standing_respawns: Vec<usize>,
}

/// Milliseconds as seconds with three decimals. Times are reported this way
/// everywhere in this project; only tick indices stay integers.
pub fn secs(ms: i64) -> String {
    let neg = ms < 0;
    let v = ms.unsigned_abs();
    format!("{}{}.{:03}", if neg { "-" } else { "" }, v / 1000, v % 1000)
}

pub fn export(o: &Opts) -> Result<Export, String> {
    let g = Loaded::load(&o.path)?;
    let a = g
        .tape
        .archives
        .get(o.archive)
        .ok_or_else(|| format!("no archive {} in {}", o.archive, o.path))?;
    let base = std::path::Path::new(&o.path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| o.path.clone());

    let mut out: Vec<String> = Vec::new();
    out.push(format!("# {}", base));
    // Seconds, not milliseconds -- these are times. The tick COUNT is a count.
    out.push(format!(
        "# {} ticks, ghost start offset {} s, declared {} s",
        a.packets.len(),
        secs(a.start_offset_ms as i64),
        g.race_time_ms
            .map(|v| secs(v as i64))
            .unwrap_or_else(|| "None".into())
    ));
    if let Some(s) = o.seed {
        out.push(format!("0 seed {}", s));
    }

    let mut pa: Option<u32> = None;
    let mut pb: Option<u32> = None;
    let mut ps: Option<i32> = None;
    let mut out_of_range = Vec::new();
    let mut respawns = Vec::new();
    let mut standing_respawns = Vec::new();
    for (i, p) in a.packets.iter().enumerate() {
        let ms = 10 * i;
        // Respawn first within the tick: the car is put back on the ground and
        // THEN this tick's steer/accel/brake apply to it.
        if p.respawn() {
            out.push(format!("{} respawn", ms));
            respawns.push(i);
        }
        if is_standing_respawn(p) {
            out.push(format!("{} srespawn", ms));
            standing_respawns.push(i);
        }
        if Some(p.accel) != pa {
            out.push(format!("{} accel {}", ms, p.accel));
            pa = Some(p.accel);
        }
        if Some(p.brake) != pb {
            out.push(format!("{} brake {}", ms, p.brake));
            pb = Some(p.brake);
        }
        let mut s = sgn(p.steer);
        if s < -127 {
            out_of_range.push(i);
            if !o.raw {
                s = -127;
            }
        }
        if Some(s) != ps {
            let word = if s == -127 {
                "left".to_string()
            } else if s == 127 {
                "right".to_string()
            } else {
                s.to_string()
            };
            out.push(format!("{} steer {}", ms, word));
            ps = Some(s);
        }
    }
    Ok(Export {
        text: out.join("\n"),
        ticks: a.packets.len(),
        start_offset_ms: a.start_offset_ms,
        race_time_ms: g.race_time_ms,
        out_of_range,
        respawns,
        standing_respawns,
    })
}

/// Per-tick state replayed out of a TICK script, the way the runtime would:
/// each held action keeps its value until the next change; `respawn` and
/// `srespawn` are EVENTS and fire only on the tick that names them.
pub struct Replayed {
    pub steer: Vec<i32>,
    pub accel: Vec<u32>,
    pub brake: Vec<u32>,
    pub respawn: Vec<bool>,
    pub srespawn: Vec<bool>,
}

const A_ACCEL: u8 = 0;
const A_BRAKE: u8 = 1;
const A_STEER: u8 = 2;
const A_RESPAWN: u8 = 3;
const A_SRESPAWN: u8 = 4;

pub fn replay(script: &str, ticks: usize) -> Result<Replayed, String> {
    let mut r = Replayed {
        steer: vec![0i32; ticks],
        accel: vec![0u32; ticks],
        brake: vec![0u32; ticks],
        respawn: vec![false; ticks],
        srespawn: vec![false; ticks],
    };
    // (tick, action, value), later lines win within a tick
    let mut ev: Vec<(usize, u8, i64)> = Vec::new();
    for (ln, line) in script.lines().enumerate() {
        let line = match line.find('#') {
            Some(k) => &line[..k],
            None => line,
        };
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        if t.len() < 2 {
            return Err(format!("line {}: incomplete action {:?}", ln + 1, line));
        }
        let ms: i64 = t[0]
            .parse()
            .map_err(|_| format!("line {}: bad time {:?}", ln + 1, t[0]))?;
        if ms % 10 != 0 {
            return Err(format!(
                "line {}: time {} does not align to a 10 ms tick",
                ln + 1,
                ms
            ));
        }
        let tick = ms / 10;
        let act = t[1];
        let arg = t.get(2).copied().unwrap_or("");
        let (code, val): (u8, i64) = match act {
            "accel" | "brake" => {
                let v = match arg {
                    "1" | "down" => 1,
                    "0" | "up" => 0,
                    _ => return Err(format!("line {}: bad {} value {:?}", ln + 1, act, arg)),
                };
                (if act == "accel" { A_ACCEL } else { A_BRAKE }, v)
            }
            "steer" => {
                let v: i64 = match arg {
                    "left" => -127,
                    "right" => 127,
                    _ => arg
                        .parse()
                        .map_err(|_| format!("line {}: bad steer {:?}", ln + 1, arg))?,
                };
                if !(-127..=127).contains(&v) {
                    return Err(format!(
                        "line {}: steer {} outside TICK's -127..127",
                        ln + 1,
                        v
                    ));
                }
                (A_STEER, v)
            }
            // Argument-less events. TICK takes no value here, and a value would
            // be a different action -- reject it rather than ignore it.
            "respawn" | "srespawn" => {
                if !arg.is_empty() {
                    return Err(format!(
                        "line {}: {} takes no argument (got {:?})",
                        ln + 1,
                        act,
                        arg
                    ));
                }
                (if act == "respawn" { A_RESPAWN } else { A_SRESPAWN }, 1)
            }
            "seed" | "flags" => continue,
            _ => return Err(format!("line {}: unsupported action {:?}", ln + 1, act)),
        };
        if tick < 0 {
            return Err(format!("line {}: negative tick not replayable here", ln + 1));
        }
        ev.push((tick as usize, code, val));
    }
    let (mut ca, mut cb, mut cs) = (0i64, 0i64, 0i64);
    let mut k = 0usize;
    ev.sort_by_key(|e| e.0); // stable: later lines still win within a tick
    for i in 0..ticks {
        r.respawn[i] = false;
        r.srespawn[i] = false;
        while k < ev.len() && ev[k].0 == i {
            match ev[k].1 {
                A_ACCEL => ca = ev[k].2,
                A_BRAKE => cb = ev[k].2,
                A_STEER => cs = ev[k].2,
                A_RESPAWN => r.respawn[i] = true,
                _ => r.srespawn[i] = true,
            }
            k += 1;
        }
        r.accel[i] = ca as u32;
        r.brake[i] = cb as u32;
        r.steer[i] = cs as i32;
    }
    // A line past the end of the ghost is a real defect: the script says
    // something about a tick the run does not have.
    if let Some(e) = ev.iter().find(|e| e.0 >= ticks) {
        return Err(format!(
            "script has an action at tick {} but the ghost is {} ticks long",
            e.0, ticks
        ));
    }
    Ok(r)
}

pub struct Diff {
    pub ticks: usize,
    pub steer_bad: Vec<usize>,
    pub accel_bad: Vec<usize>,
    pub brake_bad: Vec<usize>,
    pub respawn_bad: Vec<usize>,
    pub srespawn_bad: Vec<usize>,
}

impl Diff {
    pub fn is_exact(&self) -> bool {
        self.steer_bad.is_empty()
            && self.accel_bad.is_empty()
            && self.brake_bad.is_empty()
            && self.respawn_bad.is_empty()
            && self.srespawn_bad.is_empty()
    }
}

/// Re-read the emitted script and compare it, tick by tick, with the ghost's
/// own decoded inputs.
pub fn verify(o: &Opts, script: &str) -> Result<Diff, String> {
    let g = Loaded::load(&o.path)?;
    let a = g
        .tape
        .archives
        .get(o.archive)
        .ok_or_else(|| format!("no archive {} in {}", o.archive, o.path))?;
    let r = replay(script, a.packets.len())?;
    let mut d = Diff {
        ticks: a.packets.len(),
        steer_bad: vec![],
        accel_bad: vec![],
        brake_bad: vec![],
        respawn_bad: vec![],
        srespawn_bad: vec![],
    };
    for (i, p) in a.packets.iter().enumerate() {
        let mut want = sgn(p.steer);
        if !o.raw && want < -127 {
            want = -127;
        }
        if r.steer[i] != want {
            d.steer_bad.push(i);
        }
        if r.accel[i] != p.accel {
            d.accel_bad.push(i);
        }
        if r.brake[i] != p.brake {
            d.brake_bad.push(i);
        }
        if r.respawn[i] != p.respawn() {
            d.respawn_bad.push(i);
        }
        if r.srespawn[i] != is_standing_respawn(p) {
            d.srespawn_bad.push(i);
        }
    }
    Ok(d)
}
