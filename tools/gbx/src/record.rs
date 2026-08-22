//! `entrec` -- decoder for `CPlugEntRecordData` (class 0x0911F000) inside
//! Trackmania 2020 `*.Ghost.Gbx` / `*.Replay.Gbx` files.
//!
//! A line-by-line port of `tmtas/trajectories/entrec.py`. Extracts the recorded
//! car trajectory: per-sample time, position, orientation (quaternion +
//! yaw/pitch/roll), velocity, speed, gear, rpm, inputs, wheel state.
//!
//! The format was reverse-engineered from GBX.NET
//! (`Src/GBX.NET/Engines/Plug/CPlugEntRecordData.cs`,
//!  `Src/GBX.NET/Engines/Scene/CSceneVehicleVis.EntRecordDelta.cs`,
//!  `Src/GBX.NET/Serialization/GbxReader.cs :: ReadTransform`)
//! and then validated against independent ground truth (see `tests/selftest.rs`,
//! which is the port of `entrec.py --selftest`).
//!
//! CONFIDENCE LEVELS -- see [`FIELD_CONFIDENCE`], preserved from the Python:
//!   * VERIFIED : cross-checked numerically against ground truth in this repo.
//!   * DERIVED  : straight from GBX.NET's reference implementation,
//!                self-consistent here, but not independently cross-checked.
//!   * GUESS    : semantics taken from GBX.NET naming only; treat as opaque
//!                bytes.

use crate::container::Gbx;
use miniz_oxide::inflate::decompress_to_vec_zlib;

pub const CLASS_CPLUGENTRECORDDATA: u32 = 0x0911_F000;
pub const CLASS_CSCENEVEHICLEVIS: u32 = 0x0A01_8000;

const TAU: f64 = std::f64::consts::PI * 2.0;

pub type Res<T> = Result<T, String>;

// ---------------------------------------------------------------------------
// GBX container access
// ---------------------------------------------------------------------------

/// The decompressed GBX body bytes (`gbx.load(path).body` in the Python).
pub fn load_body(path: &str) -> Res<Vec<u8>> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {}", path, e))?;
    if data.len() < 16 || &data[0..3] != b"GBX" {
        return Err(format!("{}: not a GBX file", path));
    }
    Ok(Gbx::parse(&data).body)
}

// ---------------------------------------------------------------------------
// Little binary reader (GbxReader subset)
// ---------------------------------------------------------------------------

struct R<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn new(b: &'a [u8]) -> Self {
        R { b, p: 0 }
    }
    fn u32(&mut self) -> Res<u32> {
        if self.p + 4 > self.b.len() {
            return Err("read past end of record blob".into());
        }
        let v = u32::from_le_bytes(self.b[self.p..self.p + 4].try_into().unwrap());
        self.p += 4;
        Ok(v)
    }
    fn i32(&mut self) -> Res<i32> {
        Ok(self.u32()? as i32)
    }
    fn u8(&mut self) -> Res<u8> {
        if self.p >= self.b.len() {
            return Err("read past end of record blob".into());
        }
        let v = self.b[self.p];
        self.p += 1;
        Ok(v)
    }
    /// `MwBuffer`: i32 length + raw bytes.
    fn data(&mut self) -> Res<Vec<u8>> {
        let n = self.i32()?;
        if n < 0 || self.p + n as usize > self.b.len() {
            return Err(format!("bad MwBuffer length {}", n));
        }
        let d = self.b[self.p..self.p + n as usize].to_vec();
        self.p += n as usize;
        Ok(d)
    }
}

// ---------------------------------------------------------------------------
// Locating + decompressing the node
// ---------------------------------------------------------------------------

/// Locate chunk 0x0911F000 in a GBX body and return `(version, decompressed_blob)`.
///
/// Layout at the chunk site (observed, TM2020 ghosts, chunk version 11):
/// ```text
///     u32 chunkId          = 0x0911F000
///     u32 version          (11 for TM2020 2023+ ghosts)
///     u32 uncompressedSize
///     u32 compressedSize
///     <zlib stream>        (starts 78 9C)
/// ```
/// Versions < 5 store the record data uncompressed inline; not seen in TM2020
/// and not supported here (we return an error).
pub fn find_entrecord_blob(body: &[u8]) -> Res<(u32, Vec<u8>)> {
    let needle = CLASS_CPLUGENTRECORDDATA.to_le_bytes();
    let mut off: usize = 0;
    loop {
        let Some(rel) = find(&body[off..], &needle) else {
            return Err("CPlugEntRecordData (0x0911F000) chunk not found".into());
        };
        let hit = off + rel;
        off = hit + 1;
        // the class id often appears twice in a row (node class id, then chunk
        // id); walk forward over repeats
        let mut q = hit;
        while q + 8 <= body.len() && body[q + 4..q + 8] == needle {
            q += 4;
        }
        let p = q + 4;
        if p + 12 > body.len() {
            continue;
        }
        let version = u32::from_le_bytes(body[p..p + 4].try_into().unwrap());
        let usize_ = u32::from_le_bytes(body[p + 4..p + 8].try_into().unwrap()) as usize;
        let csize = u32::from_le_bytes(body[p + 8..p + 12].try_into().unwrap()) as usize;
        if !(1..=20).contains(&version) {
            continue;
        }
        if csize == 0 || usize_ == 0 || p + 12 + csize > body.len() {
            continue;
        }
        if body[p + 12..p + 14] != [0x78, 0x9c] {
            continue;
        }
        if version < 5 {
            return Err(format!(
                "CPlugEntRecordData version {} stores record data uncompressed inline; \
                 not implemented",
                version
            ));
        }
        let blob = decompress_to_vec_zlib(&body[p + 12..p + 12 + csize])
            .map_err(|e| format!("zlib: {:?}", e))?;
        if blob.len() != usize_ {
            return Err(format!(
                "size mismatch: header says {}, got {}",
                usize_,
                blob.len()
            ));
        }
        return Ok((version, blob));
    }
}


fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// The record-data grammar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Desc {
    pub class_id: u32,
    pub u01: i32,
    pub u02: i32,
    pub u03: i32,
    pub u04: Vec<u8>,
    pub u05: i32,
}

#[derive(Debug, Clone)]
pub struct Ent {
    pub type_: i32,
    pub u01: i32,
    pub u02: i32,
    pub u03: i32,
    pub u04: i32,
    pub times: Vec<i32>,
    pub raw: Vec<u8>,
    pub sample_size: usize,
    pub deltas2: Vec<(i32, i32, Vec<u8>)>,
}

#[derive(Debug, Clone)]
pub struct CustomModuleList {
    pub deltas: Vec<(i32, Vec<u8>, Vec<u8>)>,
    pub period: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct RecordData {
    pub version: u32,
    pub start_ms: i32,
    pub end_ms: i32,
    pub descs: Vec<Desc>,
    pub notices: Vec<(i32, i32, Option<u32>)>,
    pub ents: Vec<Ent>,
    pub bulk_notices: Vec<(i32, i32, Vec<u8>)>,
    pub custom_modules: Vec<CustomModuleList>,
    pub bytes_consumed: usize,
    pub bytes_total: usize,
}

/// Parse the decompressed `CPlugEntRecordData` payload.
///
/// Grammar (from GBX.NET `CPlugEntRecordData.ReadWrite`, version >= 11 path):
/// ```text
///     v>=1: i32 start; i32 end                      # ms
///     i32 nDesc; nDesc x EntRecordDesc
///         EntRecordDesc := u32 classId; i32 u01; i32 u02; i32 u03;
///                          data u04; i32 u05
///     v>=2: i32 nNotice; nNotice x NoticeRecordDesc
///         NoticeRecordDesc := i32 u01; i32 u02; v>=4: u32 classId
///     EntList := u8 hasNext; while hasNext:
///         i32 type            # index into the EntRecordDesc array
///         i32 u01; i32 u02    # u02 ~ start ms
///         i32 u03             # ~ end ms of this entity's recording
///         v>=6: i32 u04
///         v>=11: EncodedDeltas   (v<11: a plain list of (time, MwBuffer))
///         u8 hasNext
///         v>=2: Deltas2 := while u8: (i32 type; i32 time; data)
///     v>=3: BulkNoticeList := while u8: (i32; i32; data)
///           CustomModulesDeltaLists:
///               v>=8: i32 nLists (else 1)
///               each := (while u8: i32 u01; data; v>=9: data) ; v>=10: i32 period
///
///     EncodedDeltas := i32 numSamples;
///                      if numSamples: i32 sampleSize;
///                      numSamples x i32 deltaTime   (cumulative -> abs ms)
///                      then COLUMNAR delta coding: for each byte index i in
///                      [0,sampleSize): read numSamples bytes; running u8
///                      accumulator across samples (acc += byte) gives
///                      sample[b].data[i].  Accumulator resets per column.
/// ```
///
/// Verified end-to-end: on every test ghost the parse consumes the blob to the
/// exact last byte, which is a strong structural check on every field width
/// above.
pub fn parse_record_data(blob: &[u8], version: u32) -> Res<RecordData> {
    let mut r = R::new(blob);
    let start_ms = r.i32()?;
    let end_ms = r.i32()?;

    let n = r.i32()?;
    let mut descs = Vec::with_capacity(n.max(0) as usize);
    for _ in 0..n {
        descs.push(Desc {
            class_id: r.u32()?,
            u01: r.i32()?,
            u02: r.i32()?,
            u03: r.i32()?,
            u04: r.data()?,
            u05: r.i32()?,
        });
    }

    let mut notices = Vec::new();
    if version >= 2 {
        let n = r.i32()?;
        for _ in 0..n {
            let u01 = r.i32()?;
            let u02 = r.i32()?;
            let cid = if version >= 4 { Some(r.u32()?) } else { None };
            notices.push((u01, u02, cid));
        }
    }

    let mut ents = Vec::new();
    let mut has_next = r.u8()?;
    while has_next != 0 {
        let type_ = r.i32()?;
        let u01 = r.i32()?;
        let u02 = r.i32()?;
        let u03 = r.i32()?;
        let u04 = if version >= 6 { r.i32()? } else { u01 };
        let (times, raw, sample_size) = if version >= 11 {
            read_encoded_deltas(&mut r)?
        } else {
            let mut times = Vec::new();
            let mut bufs: Vec<Vec<u8>> = Vec::new();
            while r.u8()? != 0 {
                times.push(r.i32()?);
                bufs.push(r.data()?);
            }
            let ss = bufs.first().map_or(0, |b| b.len());
            (times, bufs.concat(), ss)
        };
        has_next = r.u8()?;
        let mut deltas2 = Vec::new();
        if version >= 2 {
            while r.u8()? != 0 {
                deltas2.push((r.i32()?, r.i32()?, r.data()?));
            }
        }
        ents.push(Ent {
            type_,
            u01,
            u02,
            u03,
            u04,
            times,
            raw,
            sample_size,
            deltas2,
        });
    }

    let mut bulk_notices = Vec::new();
    let mut custom_modules = Vec::new();
    if version >= 3 {
        while r.u8()? != 0 {
            bulk_notices.push((r.i32()?, r.i32()?, r.data()?));
        }
        let n_lists = if version >= 8 { r.i32()? } else { 1 };
        for _ in 0..n_lists {
            let mut deltas = Vec::new();
            while r.u8()? != 0 {
                let u01 = r.i32()?;
                let d = r.data()?;
                let u02 = if version >= 9 { r.data()? } else { Vec::new() };
                deltas.push((u01, d, u02));
            }
            let period = if version >= 10 { Some(r.i32()?) } else { None };
            custom_modules.push(CustomModuleList { deltas, period });
        }
    }

    Ok(RecordData {
        version,
        start_ms,
        end_ms,
        descs,
        notices,
        ents,
        bulk_notices,
        custom_modules,
        bytes_consumed: r.p,
        bytes_total: blob.len(),
    })
}

fn read_encoded_deltas(r: &mut R) -> Res<(Vec<i32>, Vec<u8>, usize)> {
    let n = r.i32()?;
    if n == 0 {
        return Ok((Vec::new(), Vec::new(), 0));
    }
    let ss = r.i32()?;
    if n < 0 || ss < 0 {
        return Err(format!("bad dimensions {}x{}", n, ss));
    }
    let (n, ss) = (n as usize, ss as usize);
    let mut times = Vec::with_capacity(n);
    let mut t: i32 = 0;
    for _ in 0..n {
        t = t.wrapping_add(r.i32()?);
        times.push(t);
    }
    if r.p + ss * n > r.b.len() {
        return Err("encoded deltas run past end of blob".into());
    }
    let mut buf = vec![0u8; n * ss];
    let src = r.b;
    let base = r.p;
    for i in 0..ss {
        let mut acc: u8 = 0;
        let row = base + i * n;
        let mut o = i;
        for b in 0..n {
            acc = acc.wrapping_add(src[row + b]);
            buf[o] = acc;
            o += ss;
        }
    }
    r.p = base + ss * n;
    Ok((times, buf, ss))
}

// ---------------------------------------------------------------------------
// CSceneVehicleVis sample layout (sampleSize >= 103; TM2020 uses 116)
// ---------------------------------------------------------------------------

/// `GbxReader.ReadTransform`, 22 bytes at offset 47:
/// ```text
///     f32 x, f32 y, f32 z          # world position, metres (Y is up)
///     u16 angle                    #  * pi / 65535
///     i16 axisHeading              #  * pi / 32767
///     i16 axisPitch                #  * pi / 32767 / 2
///     i16 speedLog                 # speed_m_s = exp(v/1000)
///     i8  velHeading               #  * pi / 127
///     i8  velPitch                 #  * pi / 127 / 2
/// ```
/// Quaternion = `(sin(angle)cos(axisPitch)cos(axisHeading),
/// sin(angle)cos(axisPitch)sin(axisHeading), sin(angle)sin(axisPitch),
/// cos(angle))` -> `(x, y, z, w)`, unit norm.
///
/// Velocity = `speed * (cos(vp)cos(vh), cos(vp)sin(vh), sin(vp))` and this
/// tuple IS the world (x, y, z) velocity (verified against a finite difference
/// of the position track).
pub fn read_transform_pub(d: &[u8], o: usize) -> ([f64; 3], [f64; 4], f64, [f64; 3]) {
    read_transform(d, o)
}

fn read_transform(d: &[u8], o: usize) -> ([f64; 3], [f64; 4], f64, [f64; 3]) {
    let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap()) as f64;
    let x = f(o);
    let y = f(o + 4);
    let z = f(o + 8);
    let ang = u16::from_le_bytes(d[o + 12..o + 14].try_into().unwrap()) as f64;
    let ah = i16::from_le_bytes(d[o + 14..o + 16].try_into().unwrap()) as f64;
    let ap = i16::from_le_bytes(d[o + 16..o + 18].try_into().unwrap()) as f64;
    let sp = i16::from_le_bytes(d[o + 18..o + 20].try_into().unwrap()) as f64;
    let vh = d[o + 20] as i8 as f64;
    let vp = d[o + 21] as i8 as f64;
    let pi = std::f64::consts::PI;
    let ang = ang * pi / 65535.0;
    let ah = ah * pi / 32767.0;
    let ap = ap / 32767.0 * pi / 2.0;
    let speed = (sp / 1000.0).exp();
    let vh = vh / 127.0 * pi;
    let vp = vp / 127.0 * pi / 2.0;
    let sa = ang.sin();
    let q = [
        sa * ap.cos() * ah.cos(),
        sa * ap.cos() * ah.sin(),
        sa * ap.sin(),
        ang.cos(),
    ];
    let vel = [
        speed * vp.cos() * vh.cos(),
        speed * vp.cos() * vh.sin(),
        speed * vp.sin(),
    ];
    ([x, y, z], q, speed, vel)
}

pub fn quat_rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let [x, y, z, w] = q;
    let (ux, uy, uz) = (x, y, z);
    // t = 2 * u x v
    let tx = 2.0 * (uy * v[2] - uz * v[1]);
    let ty = 2.0 * (uz * v[0] - ux * v[2]);
    let tz = 2.0 * (ux * v[1] - uy * v[0]);
    [
        v[0] + w * tx + (uy * tz - uz * ty),
        v[1] + w * ty + (uz * tx - ux * tz),
        v[2] + w * tz + (ux * ty - uy * tx),
    ]
}

/// Yaw (around +Y, 0 = facing +Z), pitch, roll -- from the car's local forward
/// (+Z) and up (+Y) axes. Radians.
///
/// (The Python had a dead `roll = atan2(rx*0 + ry*0 + rx*0, 1.0)` placeholder
/// and an unused right-vector `rx, ry, rz`; both are dropped here. The live
/// formula below is identical.)
pub fn quat_to_ypr(q: [f64; 4]) -> (f64, f64, f64) {
    let [fx, fy, fz] = quat_rotate(q, [0.0, 0.0, 1.0]);
    let [ux, uy, uz] = quat_rotate(q, [0.0, 1.0, 0.0]);
    let yaw = fx.atan2(fz);
    let pitch = fy.clamp(-1.0, 1.0).asin();
    // world-up projected: horizontal right vector of the un-rolled frame
    let (hx, hz) = (yaw.cos(), -yaw.sin()); // right = (cos yaw, 0, -sin yaw)
    let roll = (ux * hx + uz * hz).atan2(uy);
    (yaw, pitch, roll)
}

/// One decoded `CSceneVehicleVis` sample.
///
/// Field groups mirror `FIELD_CONFIDENCE`; the doc comment on each group says
/// how much to trust it.
#[derive(Debug, Clone, Default)]
pub struct Sample {
    /// VERIFIED: sample time in ms from the record start (50 ms grid in TM2020)
    pub time_ms: i32,
    // --- VERIFIED ---
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub speed_ms: f64,
    pub speed_kmh: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub qx: f64,
    pub qy: f64,
    pub qz: f64,
    pub qw: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    pub gear: f64,
    pub rpm_raw: u8,
    // --- DERIVED (GBX.NET reference, self-consistent, not cross-checked) ---
    pub side_speed: f64,
    pub steer: f64,
    pub brake: f64,
    pub gas: f64,
    pub is_turbo: bool,
    pub turbo_time: f64,
    pub is_ground_contact: bool,
    pub is_top_contact: bool,
    pub wetness: f64,
    pub sim_time_coef: f64,
    pub fl_wheel_rot: f64,
    pub fr_wheel_rot: f64,
    pub rr_wheel_rot: f64,
    pub rl_wheel_rot: f64,
    pub fl_dampen: f64,
    pub fr_dampen: f64,
    pub rr_dampen: f64,
    pub rl_dampen: f64,
    // --- GUESS (opaque byte semantics from names only) ---
    pub fl_ice: f64,
    pub fr_ice: f64,
    pub rr_ice: f64,
    pub rl_ice: f64,
    pub fl_dirt: f64,
    pub fr_dirt: f64,
    pub rr_dirt: f64,
    pub rl_dirt: f64,
    pub ground_mode_raw: u8,
    pub booster_air_control_raw: u8,
    pub vehicle_state_raw: u8,
    pub gear_raw: u8,
}

/// Decode one `CSceneVehicleVis` EntRecordDelta buffer.
/// Byte offsets are from GBX.NET `CSceneVehicleVis.EntRecordDelta.Read()`.
pub fn decode_vehicle_sample(d: &[u8]) -> Sample {
    let (pos, quat, speed, vel) = read_transform(d, 47);
    let (yaw, pitch, roll) = quat_to_ypr(quat);
    let gear_raw = d[91];
    let b = |i: usize| d[i] as f64;
    Sample {
        time_ms: 0,
        // --- VERIFIED ---
        x: pos[0],
        y: pos[1],
        z: pos[2],
        speed_ms: speed,
        speed_kmh: speed * 3.6,
        vx: vel[0],
        vy: vel[1],
        vz: vel[2],
        qx: quat[0],
        qy: quat[1],
        qz: quat[2],
        qw: quat[3],
        yaw,
        pitch,
        roll,
        gear: (gear_raw as f64 - 1.0) / 4.0,
        rpm_raw: d[5],
        // --- DERIVED ---
        side_speed: ((u16::from_le_bytes(d[2..4].try_into().unwrap()) as f64 / 65536.0) - 0.5)
            * 2000.0,
        steer: ((b(14) / 255.0) - 0.5) * 2.0,
        brake: b(18) / 255.0,
        gas: (b(15) / 255.0) + (b(18) / 255.0),
        is_turbo: (d[31] & 0x82) != 0,
        turbo_time: b(21) / 255.0,
        is_ground_contact: (d[89] & 0x1) != 0,
        is_top_contact: (d[76] & 0x20) != 0,
        wetness: b(101) / 255.0,
        sim_time_coef: b(102) / 255.0,
        fl_wheel_rot: (b(6) / 255.0 * TAU) + b(7) * TAU,
        fr_wheel_rot: (b(8) / 255.0 * TAU) + b(9) * TAU,
        rr_wheel_rot: (b(10) / 255.0 * TAU) + b(11) * TAU,
        rl_wheel_rot: (b(12) / 255.0 * TAU) + b(13) * TAU,
        fl_dampen: ((b(23) / 255.0) - 0.5) * 4.0,
        fr_dampen: ((b(25) / 255.0) - 0.5) * 4.0,
        rr_dampen: ((b(27) / 255.0) - 0.5) * 4.0,
        rl_dampen: ((b(29) / 255.0) - 0.5) * 4.0,
        // --- GUESS ---
        fl_ice: b(81) / 255.0,
        fr_ice: b(82) / 255.0,
        rr_ice: b(83) / 255.0,
        rl_ice: b(84) / 255.0,
        fl_dirt: b(93) / 255.0,
        fr_dirt: b(95) / 255.0,
        rr_dirt: b(97) / 255.0,
        rl_dirt: b(99) / 255.0,
        ground_mode_raw: d[89],
        booster_air_control_raw: d[90],
        vehicle_state_raw: d[76],
        gear_raw,
    }
}

// ---------------------------------------------------------------------------
// Generic field access (used by the CSV writer and the golden-data tests)

/// Every field a decoded sample carries, with its confidence level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conf {
    /// Cross-checked numerically against independent ground truth.
    Verified,
    /// GBX.NET reference implementation, internally consistent, no independent
    /// ground truth here.
    Derived,
    /// Byte position and name from GBX.NET only; values look plausible but
    /// nothing pins the semantics.
    Guess,
}

impl Conf {
    pub fn label(self) -> &'static str {
        match self {
            Conf::Verified => "VERIFIED",
            Conf::Derived => "DERIVED",
            Conf::Guess => "GUESS",
        }
    }
}

pub struct FieldDoc {
    pub name: &'static str,
    pub conf: Conf,
    pub note: &'static str,
}

/// The confidence taxonomy carried over verbatim from `entrec.py`'s
/// `FIELD_CONFIDENCE` string, but machine-readable: `tmtraj fields` prints it
/// and a test asserts it covers every field of [`Sample`].
pub const FIELD_CONFIDENCE: &[FieldDoc] = &[
    FieldDoc { name: "time_ms", conf: Conf::Verified, note: "sample time in ms from the record start (50 ms grid in TM2020)" },
    FieldDoc { name: "x", conf: Conf::Verified, note: "world position X, metres -- start pos == start block centre to <0.01 m" },
    FieldDoc { name: "y", conf: Conf::Verified, note: "world position Y (up), metres" },
    FieldDoc { name: "z", conf: Conf::Verified, note: "world position Z, metres -- CP1/CP3 crossings within 2.3-2.6 m of the nominal block centres at the declared split times" },
    FieldDoc { name: "speed_ms", conf: Conf::Verified, note: "scalar speed exp(i16/1000) -- equals |d(pos)/dt| to a median of ~0.02 m/s" },
    FieldDoc { name: "speed_kmh", conf: Conf::Verified, note: "speed_ms * 3.6" },
    FieldDoc { name: "vx", conf: Conf::Verified, note: "velocity vector X -- direction equals the path tangent, median cos > 0.9999" },
    FieldDoc { name: "vy", conf: Conf::Verified, note: "velocity vector Y" },
    FieldDoc { name: "vz", conf: Conf::Verified, note: "velocity vector Z" },
    FieldDoc { name: "qx", conf: Conf::Verified, note: "orientation quaternion X -- unit norm to 1e-7" },
    FieldDoc { name: "qy", conf: Conf::Verified, note: "orientation quaternion Y" },
    FieldDoc { name: "qz", conf: Conf::Verified, note: "orientation quaternion Z" },
    FieldDoc { name: "qw", conf: Conf::Verified, note: "orientation quaternion W -- local +Z is the car's forward axis (cos to velocity ~1.000 on the ground, correctly diverging in the air)" },
    FieldDoc { name: "yaw", conf: Conf::Verified, note: "derived from the quaternion; yaw 0 = facing +Z, right-handed about +Y" },
    FieldDoc { name: "pitch", conf: Conf::Verified, note: "derived from the quaternion" },
    FieldDoc { name: "roll", conf: Conf::Verified, note: "derived from the quaternion" },
    FieldDoc { name: "gear", conf: Conf::Verified, note: "(byte91 - 1) / 4 -- byte only ever takes values 1+4k (5,9,13,17,21 => 1..5)" },
    FieldDoc { name: "rpm_raw", conf: Conf::Verified, note: "byte 5, 0..255, monotone with engine load; the absolute RPM scale factor is NOT known" },
    FieldDoc { name: "steer", conf: Conf::Derived, note: "byte 14: ((v/255)-0.5)*2" },
    FieldDoc { name: "gas", conf: Conf::Derived, note: "byte 15/255 + brake" },
    FieldDoc { name: "brake", conf: Conf::Derived, note: "byte 18 / 255" },
    FieldDoc { name: "side_speed", conf: Conf::Derived, note: "u16 at bytes 2,3: ((v/65536)-0.5)*2000" },
    FieldDoc { name: "turbo_time", conf: Conf::Derived, note: "byte 21 / 255" },
    FieldDoc { name: "is_turbo", conf: Conf::Derived, note: "byte 31 & 0x82" },
    FieldDoc { name: "is_ground_contact", conf: Conf::Derived, note: "byte 89 & 0x01" },
    FieldDoc { name: "is_top_contact", conf: Conf::Derived, note: "byte 76 & 0x20" },
    FieldDoc { name: "wetness", conf: Conf::Derived, note: "byte 101 / 255" },
    FieldDoc { name: "sim_time_coef", conf: Conf::Derived, note: "byte 102 / 255" },
    FieldDoc { name: "fl_wheel_rot", conf: Conf::Derived, note: "bytes 6,7: rotation + rotation count, radians" },
    FieldDoc { name: "fr_wheel_rot", conf: Conf::Derived, note: "bytes 8,9" },
    FieldDoc { name: "rr_wheel_rot", conf: Conf::Derived, note: "bytes 10,11" },
    FieldDoc { name: "rl_wheel_rot", conf: Conf::Derived, note: "bytes 12,13" },
    FieldDoc { name: "fl_dampen", conf: Conf::Derived, note: "byte 23: ((v/255)-0.5)*4" },
    FieldDoc { name: "fr_dampen", conf: Conf::Derived, note: "byte 25" },
    FieldDoc { name: "rr_dampen", conf: Conf::Derived, note: "byte 27" },
    FieldDoc { name: "rl_dampen", conf: Conf::Derived, note: "byte 29" },
    FieldDoc { name: "fl_ice", conf: Conf::Guess, note: "byte 81 / 255" },
    FieldDoc { name: "fr_ice", conf: Conf::Guess, note: "byte 82 / 255" },
    FieldDoc { name: "rr_ice", conf: Conf::Guess, note: "byte 83 / 255" },
    FieldDoc { name: "rl_ice", conf: Conf::Guess, note: "byte 84 / 255" },
    FieldDoc { name: "fl_dirt", conf: Conf::Guess, note: "byte 93 / 255" },
    FieldDoc { name: "fr_dirt", conf: Conf::Guess, note: "byte 95 / 255" },
    FieldDoc { name: "rr_dirt", conf: Conf::Guess, note: "byte 97 / 255" },
    FieldDoc { name: "rl_dirt", conf: Conf::Guess, note: "byte 99 / 255" },
    FieldDoc { name: "ground_mode_raw", conf: Conf::Guess, note: "byte 89, raw" },
    FieldDoc { name: "booster_air_control_raw", conf: Conf::Guess, note: "byte 90, raw" },
    FieldDoc { name: "vehicle_state_raw", conf: Conf::Guess, note: "byte 76, raw" },
    FieldDoc { name: "gear_raw", conf: Conf::Guess, note: "byte 91, raw (1+4*gear)" },
];

pub const NOT_DECODED: &str = "\
NOT DECODED: the non-vehicle entities in the record (classes 0x0A019000,
0x2F0CB000, 0x032E3000, 0x032AC000, 0x2D001000, 0x032CB000), the notice
records, and Deltas2 -- their per-byte layouts are not in GBX.NET. Ground
contact material ids (bytes 24/26/28/30) are GUESS and are not surfaced.";

pub fn print_field_confidence() {
    for conf in [Conf::Verified, Conf::Derived, Conf::Guess] {
        println!("\n{}:", conf.label());
        for f in FIELD_CONFIDENCE.iter().filter(|f| f.conf == conf) {
            println!("  {:<24} {}", f.name, f.note);
        }
    }
    println!("\n{}", NOT_DECODED);
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EntInfo {
    pub type_: i32,
    pub class_id: Option<u32>,
    pub n_samples: usize,
    pub sample_size: usize,
    pub t_first: Option<i32>,
    pub t_last: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct Decoded {
    pub path: String,
    pub name: String,
    pub version: u32,
    pub start_ms: i32,
    pub end_ms: i32,
    pub sample_period_ms: Option<i32>,
    pub sample_size: usize,
    pub samples: Vec<Sample>,
    /// The vehicle entity's raw sample bytes, flat: sample `i` is
    /// `raw[i * sample_size ..][.. sample_size]`, and `raw_sample(i)` gets it.
    ///
    /// Here so that no caller has to re-open the file and re-pick the vehicle
    /// entity to read a byte. Six modules used to do that by hand, and two of
    /// them picked by sample size alone without checking the class id — which
    /// takes the WRONG entity on a container whose donor carries a bigger
    /// foreign one (165922's donor holds 175815 samples of the undecoded
    /// 0x2D001000 entity, spanning its whole 2.4-hour session).
    pub raw: Vec<u8>,
    pub checkpoints_ms: Vec<i32>,
    pub race_time_ms: Option<i32>,
    pub ents: Vec<EntInfo>,
    pub bytes_consumed: usize,
    pub bytes_total: usize,
}

/// Decode a `.Ghost.Gbx` (or `.Replay.Gbx`).
pub fn decode_ghost(path: &str) -> Res<Decoded> {
    let body = load_body(path)?;
    decode_body(&body, path)
}

pub fn decode_body(body: &[u8], path: &str) -> Res<Decoded> {
    let (version, blob) = find_entrecord_blob(body)?;
    let rec = parse_record_data(&blob, version)?;

    let mut veh: Option<&Ent> = None;
    let mut others = Vec::new();
    for ent in &rec.ents {
        let cid = rec
            .descs
            .get(ent.type_.max(0) as usize)
            .filter(|_| ent.type_ >= 0)
            .map(|d| d.class_id);
        others.push(EntInfo {
            type_: ent.type_,
            class_id: cid,
            n_samples: ent.times.len(),
            sample_size: ent.sample_size,
            t_first: ent.times.first().copied(),
            t_last: ent.times.last().copied(),
        });
        // Some ghosts carry TWO CSceneVehicleVis entities: a heavily decimated
        // one (6-7 samples, ~3 s apart) plus the real full-rate track. Always
        // take the one with the most samples.
        if cid == Some(CLASS_CSCENEVEHICLEVIS)
            && veh.map_or(true, |v| ent.times.len() > v.times.len())
        {
            veh = Some(ent);
        }
    }
    let veh = veh.ok_or("no CSceneVehicleVis (0x0A018000) entity in record")?;

    let res = read_ghost_result(body);
    let ss = veh.sample_size;
    if ss < 103 {
        return Err(format!("vehicle sample size {} < 103, layout unknown", ss));
    }
    let mut samples = Vec::with_capacity(veh.times.len());
    for (i, &t) in veh.times.iter().enumerate() {
        let d = &veh.raw[i * ss..(i + 1) * ss];
        let mut s = decode_vehicle_sample(d);
        s.time_ms = t;
        samples.push(s);
    }
    // modal inter-sample gap (Python: max(set(diffs), key=diffs.count); ties
    // there fall to set iteration order, here to the smallest value)
    let per = if veh.times.len() > 2 {
        let diffs: Vec<i32> = veh.times.windows(2).map(|w| w[1] - w[0]).collect();
        let mut uniq: Vec<i32> = diffs.clone();
        uniq.sort_unstable();
        uniq.dedup();
        uniq.into_iter()
            .max_by_key(|d| (diffs.iter().filter(|x| *x == d).count(), -(*d as i64)))
    } else {
        None
    };

    Ok(Decoded {
        path: path.to_string(),
        name: name_for(path),
        version,
        start_ms: rec.start_ms,
        end_ms: rec.end_ms,
        sample_period_ms: per,
        sample_size: ss,
        raw: veh.raw.clone(),
        samples,
        checkpoints_ms: res.1,
        race_time_ms: res.0,
        ents: others,
        bytes_consumed: rec.bytes_consumed,
        bytes_total: rec.bytes_total,
    })
}

/// `decode_all.py :: name_for`
pub fn name_for(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.replace(".Ghost.Gbx", "")
}

/// `CGameCtnGhost` chunk 0x0309202B, stored as a skippable chunk:
/// ```text
///     u32 chunkId = 0x0309202B
///     'PIKS'                       # 0x534B4950 "SKIP" as a LE u32
///     u32 chunkSize                # 60 for a 4-CP run
///     --- payload ---
///     u32 version = 1
///     i32 raceTime_ms              # the official run time
///     i32 u01, i32 u02
///     i32 nbRespawns               # 3 in all our ghosts
///     i32 nCheckpoints
///     nCheckpoints x (i32 time_ms, i32 stuntsScore-or-flag)
///     i32 -1
/// ```
/// Verified on all reference ghosts: the extracted times equal the splits
/// quoted independently, and the final split always equals raceTime.
pub fn read_ghost_result(body: &[u8]) -> (Option<i32>, Vec<i32>) {
    let mut needle = Vec::with_capacity(8);
    needle.extend_from_slice(&0x0309_202Bu32.to_le_bytes());
    needle.extend_from_slice(b"PIKS");
    let Some(off) = find(body, &needle) else {
        return (None, Vec::new());
    };
    if off + 12 > body.len() {
        return (None, Vec::new());
    }
    let size = u32::from_le_bytes(body[off + 8..off + 12].try_into().unwrap()) as usize;
    let p = off + 12;
    let n_ints = size / 4;
    if n_ints < 6 || p + n_ints * 4 > body.len() {
        return (None, Vec::new());
    }
    let iat = |i: usize| i32::from_le_bytes(body[p + i * 4..p + i * 4 + 4].try_into().unwrap());
    let race = iat(1);
    let n = iat(5);
    let cps = if n > 0 && n < 200 && 6 + 2 * (n as usize - 1) < n_ints {
        (0..n as usize).map(|i| iat(6 + 2 * i)).collect()
    } else {
        Vec::new()
    };
    (Some(race), cps)
}

pub fn read_checkpoints(body: &[u8]) -> Vec<i32> {
    read_ghost_result(body).1
}

impl Decoded {
    /// The raw bytes of sample `i`, or `None` past the end.
    pub fn raw_sample(&self, i: usize) -> Option<&[u8]> {
        let ss = self.sample_size;
        self.raw.get(i * ss..(i + 1) * ss)
    }

    /// Every raw sample, in order.
    pub fn raw_samples(&self) -> impl Iterator<Item = &[u8]> {
        self.raw.chunks_exact(self.sample_size.max(1))
    }
}

// ---------------------------------------------------------------------------
// The recorded INPUT channel
// ---------------------------------------------------------------------------
//
// A ghost carries its driver's inputs TWICE: in the 10 ms input chunk
// (`gbx::tape`, what the driver pressed) and in three bytes of every 50 ms
// telemetry sample (what the car was being given). That redundancy is the
// cheapest contamination check this project has — a search tape whose telemetry
// came from a template disagrees with its own tape — and it only works if the
// encoding is exact.

/// The recorded steer byte for a tape steer value.
///
/// MEASURED against the whole corpus, not assumed: `floor((s + 127) * 255 /
/// 254)`. Note the 254, and note the FLOOR. A `round` misses `s = 0` and
/// `s = 60`, which is how a "close enough" encoder ends up one grid step out on
/// half a file — that exact error took one verification statistic from a
/// Cohen's kappa of 0.467 to 1.000 (455 of 455 samples exact) when it was
/// found and fixed.
pub fn steer_byte(s: i8) -> u8 {
    (((s as i32 + 127) as i64 * 255) / 254) as u8
}

/// The EXACT inverse: the tape steer value a recorded byte 14 came from.
///
/// `None` when no steer value produces that byte. The map is injective and
/// skips values (255/254 > 1), so an unreachable byte is a real signal — it
/// means the byte was not written by this encoding, i.e. not by the game
/// recording a car being steered.
///
/// This is what makes a run's inputs readable out of the telemetry ALONE, with
/// no input chunk and no engine. Before it, `tmtraj` decoded byte 14 as the
/// float `((v/255) - 0.5) * 2` and every comparison against a tape had to allow
/// a +-1 slop that hid exactly the off-by-one above.
pub fn steer_i8_from_byte(b: u8) -> Option<i8> {
    // 255 candidates; a direct search is clearer than an inverted formula and
    // cannot disagree with `steer_byte`, which is the point.
    (-127i8..=127).find(|s| steer_byte(*s) == b)
}

/// Byte 15 is 255 with the gas down and 0 otherwise; byte 18 is the brake.
/// Digital, not analogue: any other value is not this channel.
pub fn pedal_from_byte(b: u8) -> Option<bool> {
    match b {
        0 => Some(false),
        255 => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod input_channel_tests {
    use super::*;

    #[test]
    fn the_steer_byte_round_trips_exactly_over_the_whole_range() {
        for s in -127i8..=127 {
            assert_eq!(steer_i8_from_byte(steer_byte(s)), Some(s), "steer {}", s);
        }
    }

    #[test]
    fn a_round_would_be_wrong_and_here_is_where() {
        // The two values a `round` gets wrong, named, so the fix cannot be
        // undone by somebody "simplifying" the formula.
        for s in [0i8, 60] {
            let floor_b = steer_byte(s);
            let round_b = (((s as f64 + 127.0) * 255.0 / 254.0).round()) as u8;
            assert_ne!(floor_b, round_b, "steer {} is where floor and round differ", s);
        }
    }

    #[test]
    fn an_unreachable_byte_is_reported_as_unreachable() {
        let reachable: std::collections::BTreeSet<u8> =
            (-127i8..=127).map(steer_byte).collect();
        let missing: Vec<u8> = (0u8..=255).filter(|b| !reachable.contains(b)).collect();
        assert!(!missing.is_empty(), "the map skips values; that is the point");
        for b in missing {
            assert_eq!(steer_i8_from_byte(b), None);
        }
    }
}
