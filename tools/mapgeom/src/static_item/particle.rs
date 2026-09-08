//! `CPlugFxSystem` (0x0915C000) — the effect script a Show item's prefab
//! carries as an entity (`Fogger16M.Prefab.Gbx` entity 1 = external
//! `Fogger16M.FxSys.Gbx`) — and the particle-model chain it drives:
//! `CPlugParticleEmitterModel` (0x090B3000, `*.ParticleModel.Gbx`) ->
//! `CPlugParticleEmitterSubModel` (0x090B2000) -> its render node
//! (0x090B5000), `CPlugParticleGpuSpawn` (0x090C5000) and
//! `CPlugParticleGpuModel` (0x090C6000). Typed where the static-item builder
//! must relocate something (node references, lookback ids, the expression
//! strings), byte-exact everywhere else; a parse -> write round trip of
//! every FxSys and ParticleModel file of the Stadium pack is identical
//! (`mapgeom fx-dump --check`).
//!
//! The FxSystem grammar (2026-09-08, read off the six Stadium `.FxSys.Gbx`
//! files and the engine's reflection tables — `mapgeom exe-class`):
//!
//! ```text
//! chunk 0x0915C000: version 1, u32 10, root node,
//!                   ContextClassId, ExtraContextClassId, var count, u32 55, vars
//! node:  u32 type, Id Name, then per type
//!   0 Parallel         : u32 count, children
//!   1 Condition        : string ConditionExpr, child node
//!   3 UpdateVar        : Id VarName, u32 ResetToDefaultIfInactive, string UpdateVarExpr
//!   4 ParticleEmitter  : ref Model (CPlugParticleEmitterModel), Id JointName,
//!                        10 expression strings, u32 DOVAndUpAreLocalSpace,
//!                        2 expression strings
//! var:   Id name, u8 type, type 2 (Real): f32, u8, f32, f32; type 6: u8, u8
//! ```
//!
//! The engine's `CPlugFxSystemNode_ParticleEmitter` members, in registration
//! order: Model, JointName, DOVAndUpAreLocalSpace, LocalOffsetExpr,
//! WorldOffsetExpr, DOVExpr, LinearVelInWExpr, SpawnFreqModifierExpr,
//! ScaleExpr, OpacityExpr, LAmbientExpr, WaterTopExpr, HueLightness — plus
//! two unregistered expression slots (object layout +0x58 and +0xd8). The
//! file order of the twelve strings is NOT the registration order; what is
//! known from the files: string 1 is LocalOffsetExpr (`float3(0,0,5)` on
//! SparklerEnd8m = the end emitter 5 m along the axis), string 4 is the
//! SpawnFreqModifierExpr (`cos(Time/800-1)` on the pulsing sparklers),
//! string 11 is the hue (`-1.0`, or the `Hue` variable). The exe's own
//! serialiser (exe+0x62098c) settles the rest: 1 LocalOffset, 2 WorldOffset,
//! 3 LinearVelInW, 4 SpawnFreqModifier, 5 Scale, 6 LAmbient, 7 Up, 8 DOV,
//! 9 Opacity, 10 WaterTop, [bool DOVAndUpAreLocalSpace], 11 LinearHue01,
//! 12 HueLightness (`TINY_FX_EXPR_K=expr` overrides string K).

use super::{read_ref, write_ref, Id, Rd, Ref, Wr, R, FACADE};

pub const C_FX_SYSTEM: u32 = 0x0915C000;
pub const C_PARTICLE_EMITTER_MODEL: u32 = 0x090B3000;
pub const C_PARTICLE_EMITTER_SUB_MODEL: u32 = 0x090B2000;
/// The sub-model's render/shape node (0x090B5000; one chunk of 158 bytes).
pub const C_PARTICLE_SUB_NODE: u32 = 0x090B5000;
pub const C_PARTICLE_GPU_SPAWN: u32 = 0x090C5000;
pub const C_PARTICLE_GPU_MODEL: u32 = 0x090C6000;
/// `CPlugBitmap` (`*.Texture.gbx`): the sub-model's texture, read here so it
/// can ride INLINE in an item with its image as an archive file
/// (`TINY_FX_TEXTURE=archive`). Chunk 0x09011030 = {version 5, ref image,
/// 28 bytes}; 0x09011034 = {version 4, ref, u32, count, refs, ref, u32,
/// u32} (the frame list of the multi-image screen textures); 0x0901102A/2C
/// single refs; the rest fixed-size (measured on 40 Stadium textures).
pub const C_BITMAP: u32 = 0x09011000;

pub fn is_particle_class(c: u32) -> bool {
    matches!(c, C_FX_SYSTEM | C_PARTICLE_EMITTER_MODEL | C_PARTICLE_EMITTER_SUB_MODEL | C_PARTICLE_SUB_NODE | C_PARTICLE_GPU_SPAWN | C_PARTICLE_GPU_MODEL | C_BITMAP)
}

// ------------------------------------------------------------- CPlugFxSystem

#[derive(Clone, Debug, PartialEq)]
pub enum FxNode {
    Parallel { name: Id, children: Vec<FxNode> },
    Condition { name: Id, condition: String, child: Box<FxNode> },
    UpdateVar { name: Id, var: Id, reset_if_inactive: u32, expr: String },
    ParticleEmitter(FxEmitter),
}

/// A `CPlugFxSystemNode_ParticleEmitter`.
#[derive(Clone, Debug, PartialEq)]
pub struct FxEmitter {
    pub name: Id,
    pub model: Ref,
    pub joint_name: Id,
    /// The ten expression strings before the bool (file order).
    pub exprs: [String; 10],
    pub dov_and_up_local: u32,
    /// The two expression strings after the bool: hue (`-1.0` / `Hue`), and
    /// the last one (`0.5` on every Stadium file).
    pub tail: [String; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxVar {
    pub name: Id,
    pub kind: u8,
    /// The value bytes after the type: 13 for type 2 (Real), 2 for type 6.
    pub raw: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugFxSystem {
    pub version: u32,
    pub u01: u32,
    pub root: FxNode,
    pub context_class_id: i32,
    pub extra_context_class_id: i32,
    pub vars_tag: u32,
    pub vars: Vec<FxVar>,
}

impl FxNode {
    fn parse(r: &mut Rd) -> R<FxNode> {
        let at = r.o;
        let ty = r.u32()?;
        let name = r.id()?;
        Ok(match ty {
            0 => {
                let n = r.count()?;
                if n > 256 {
                    return Err(format!("FxSystem parallel node at 0x{at:x} claims {n} children"));
                }
                let mut children = Vec::with_capacity(n);
                for _ in 0..n {
                    children.push(FxNode::parse(r)?);
                }
                FxNode::Parallel { name, children }
            }
            1 => {
                let condition = r.string()?;
                let child = Box::new(FxNode::parse(r)?);
                FxNode::Condition { name, condition, child }
            }
            3 => {
                let var = r.id()?;
                let reset_if_inactive = r.u32()?;
                let expr = r.string()?;
                FxNode::UpdateVar { name, var, reset_if_inactive, expr }
            }
            4 => {
                let model = read_ref(r)?;
                let joint_name = r.id()?;
                let mut exprs: [String; 10] = Default::default();
                for e in exprs.iter_mut() {
                    *e = r.string()?;
                }
                let dov_and_up_local = r.u32()?;
                let tail = [r.string()?, r.string()?];
                FxNode::ParticleEmitter(FxEmitter { name, model, joint_name, exprs, dov_and_up_local, tail })
            }
            other => return Err(format!("FxSystem node type {other} at 0x{at:x} has no reader (0 Parallel, 1 Condition, 3 UpdateVar, 4 ParticleEmitter are known)")),
        })
    }

    fn write(&self, w: &mut Wr) {
        match self {
            FxNode::Parallel { name, children } => {
                w.u32(0);
                w.id(name);
                w.u32(children.len() as u32);
                for c in children {
                    c.write(w);
                }
            }
            FxNode::Condition { name, condition, child } => {
                w.u32(1);
                w.id(name);
                w.string(condition);
                child.write(w);
            }
            FxNode::UpdateVar { name, var, reset_if_inactive, expr } => {
                w.u32(3);
                w.id(name);
                w.id(var);
                w.u32(*reset_if_inactive);
                w.string(expr);
            }
            FxNode::ParticleEmitter(e) => {
                w.u32(4);
                w.id(&e.name);
                write_ref(w, &e.model);
                w.id(&e.joint_name);
                for s in &e.exprs {
                    w.string(s);
                }
                w.u32(e.dov_and_up_local);
                for s in &e.tail {
                    w.string(s);
                }
            }
        }
    }

    /// Every particle emitter in the tree, depth first.
    pub fn emitters(&self) -> Vec<&FxEmitter> {
        match self {
            FxNode::Parallel { children, .. } => children.iter().flat_map(|c| c.emitters()).collect(),
            FxNode::Condition { child, .. } => child.emitters(),
            FxNode::UpdateVar { .. } => Vec::new(),
            FxNode::ParticleEmitter(e) => vec![e],
        }
    }

    pub fn emitters_mut(&mut self) -> Vec<&mut FxEmitter> {
        match self {
            FxNode::Parallel { children, .. } => children.iter_mut().flat_map(|c| c.emitters_mut()).collect(),
            FxNode::Condition { child, .. } => child.emitters_mut(),
            FxNode::UpdateVar { .. } => Vec::new(),
            FxNode::ParticleEmitter(e) => vec![e],
        }
    }

    pub fn describe(&self, depth: usize, out: &mut String) {
        use std::fmt::Write;
        let pad = "  ".repeat(depth);
        match self {
            FxNode::Parallel { name, children } => {
                let _ = writeln!(out, "{pad}Parallel {:?} ({} children)", name.as_str().unwrap_or(""), children.len());
                for c in children {
                    c.describe(depth + 1, out);
                }
            }
            FxNode::Condition { name, condition, child } => {
                let _ = writeln!(out, "{pad}Condition {:?} if `{condition}`", name.as_str().unwrap_or(""));
                child.describe(depth + 1, out);
            }
            FxNode::UpdateVar { name, var, reset_if_inactive, expr } => {
                let _ = writeln!(out, "{pad}UpdateVar {:?}: {} = `{expr}` (reset if inactive {reset_if_inactive})", name.as_str().unwrap_or(""), var.as_str().unwrap_or("?"));
            }
            FxNode::ParticleEmitter(e) => {
                let _ = writeln!(out, "{pad}ParticleEmitter {:?} model node {} joint {:?} dov/up local {}", e.name.as_str().unwrap_or(""), e.model.index, e.joint_name.as_str().unwrap_or(""), e.dov_and_up_local);
                for (k, s) in e.exprs.iter().enumerate() {
                    let _ = writeln!(out, "{pad}  expr {}: `{s}`", k + 1);
                }
                let _ = writeln!(out, "{pad}  expr 11 (hue): `{}`   expr 12: `{}`", e.tail[0], e.tail[1]);
            }
        }
    }
}

impl CPlugFxSystem {
    /// The node body (after the class id): one chunk, then FACADE.
    pub fn parse(r: &mut Rd) -> R<CPlugFxSystem> {
        let cid = r.u32()?;
        if cid != C_FX_SYSTEM {
            return Err(format!("CPlugFxSystem starts with chunk 0x{cid:08X}"));
        }
        let version = r.u32()?;
        if version != 1 {
            return Err(format!("CPlugFxSystem version {version} (only 1 is read)"));
        }
        let u01 = r.u32()?;
        let root = FxNode::parse(r)?;
        let context_class_id = r.i32()?;
        let extra_context_class_id = r.i32()?;
        let n = r.count()?;
        let vars_tag = r.u32()?;
        let mut vars = Vec::with_capacity(n);
        for _ in 0..n {
            let name = r.id()?;
            let kind = r.u8()?;
            let len = match kind {
                2 => 13,
                6 => 2,
                other => return Err(format!("FxSystem var {:?} of type {other} at 0x{:x}: unknown value size", name, r.o)),
            };
            let raw = r.take(len)?.to_vec();
            vars.push(FxVar { name, kind, raw });
        }
        let f = r.u32()?;
        if f != FACADE {
            return Err(format!("CPlugFxSystem: 0x{f:08X} after its chunk is not FACADE (at 0x{:x})", r.o - 4));
        }
        Ok(CPlugFxSystem { version, u01, root, context_class_id, extra_context_class_id, vars_tag, vars })
    }

    pub fn write(&self, w: &mut Wr) {
        w.u32(C_FX_SYSTEM);
        w.u32(self.version);
        w.u32(self.u01);
        self.root.write(w);
        w.i32(self.context_class_id);
        w.i32(self.extra_context_class_id);
        w.u32(self.vars.len() as u32);
        w.u32(self.vars_tag);
        for v in &self.vars {
            w.id(&v.name);
            w.u8(v.kind);
            w.bytes(&v.raw);
        }
        w.u32(FACADE);
    }

    pub fn describe(&self) -> String {
        let mut out = String::new();
        self.root.describe(0, &mut out);
        out.push_str(&format!("context class 0x{:08X} extra 0x{:08X}, {} vars\n", self.context_class_id, self.extra_context_class_id, self.vars.len()));
        for v in &self.vars {
            out.push_str(&format!("  var {:?} type {} {:02x?}\n", v.name.as_str().unwrap_or("?"), v.kind, v.raw));
        }
        out
    }
}

// ---------------------------------------------------- the particle model chain

/// One chunk of a particle-chain node: typed where it carries a reference or
/// an id, raw bytes (fixed size per chunk id and version) elsewhere.
#[derive(Clone, Debug, PartialEq)]
pub enum PChunk {
    /// 0x090B3000 v10: the sub-model references.
    SubModels { version: u32, models: Vec<Ref> },
    /// 0x090B3001: the model's name.
    ModelName(Id),
    /// 0x090B202D v4: eight words, the sub-model's name, one word.
    SubModelHead { version: u32, words: [u32; 8], name: Id, u01: u32 },
    /// 0x090B202E v0: the render node (0x090B5000).
    RenderNode { version: u32, node: Ref },
    /// 0x090B2036 v0: two words and the texture (`FoggerSmoke.Texture.gbx`).
    Texture { version: u32, u01: u32, u02: u32, texture: Ref },
    /// 0x090B203A v1: the GPU spawn and GPU model nodes.
    Gpu { version: u32, spawn: Ref, model: Ref },
    /// 0x090C5000 v1: fifteen words, then `count` keys of four words each.
    GpuSpawn { version: u32, words: [u32; 15], keys: Vec<[u32; 4]> },
    /// 0x09011030 v5 (CPlugBitmap): the image (`Image\X.dds`), 28 bytes.
    BitmapImage { version: u32, image: Ref, tail: Vec<u8> },
    /// 0x09011034 v4 (CPlugBitmap): the frame list.
    BitmapFrames { version: u32, r1: Ref, u01: u32, frames: Vec<Ref>, r2: Ref, u02: u32, u03: u32 },
    /// 0x0901102A / 0x0901102C (CPlugBitmap): one reference.
    SingleRef { id: u32, node: Ref },
    /// 0x09011036 v1 (CPlugBitmap): a reference, an Id (a LOOKBACK string —
    /// in the pack file it is preceded by the body's lookback version word,
    /// which an inline copy inside an item must NOT carry: copied raw it
    /// misaligned the engine's read and crashed the client, 2026-09-08
    /// 06:33Z), a reference.
    BitmapNamed { version: u32, r1: Ref, name: Id, r2: Ref },
    /// Any other chunk: id and payload (its size is a function of the id).
    Raw { id: u32, payload: Vec<u8> },
}

/// Payload size of the fixed-size chunks (after the 4-byte chunk id), as the
/// Stadium pack writes them. A chunk absent here needs a typed arm.
pub fn raw_payload_len(id: u32) -> Option<usize> {
    Some(match id {
        0x090B3002 => 12,
        0x090B3003 => 8,
        0x090B3004 => 52,
        0x090B5000 => 158,
        0x090B202F => 74,
        0x090B2030 => 16,
        0x090B2031 => 158,
        0x090B2032 => 36,
        0x090B2033 => 285,
        0x090B2034 => 95,
        0x090B2035 => 4,
        0x090B2037 => 94,
        0x090B2038 => 12,
        0x090B2039 => 48,
        0x090B203B => 48,
        0x090C6000 => 40,
        0x090C6001 => 28,
        0x090C6002 => 188,
        0x090C6003 => 16,
        // CPlugBitmap
        0x09011019 => 4,
        0x09011020 => 4,
        0x09011023 => 4,
        0x09011025 => 24,
        0x09011028 => 8,
        0x0901102D => 8,
        0x09011032 => 8,
        0x09011033 => 4,
        0x09011035 => 6,
        0x09011037 => 36,
        0x09011038 => 12,
        _ => return None,
    })
}

/// A node of the particle chain: its class and chunks, in file order.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleNode {
    pub class_id: u32,
    pub chunks: Vec<PChunk>,
}

impl ParticleNode {
    pub fn parse(r: &mut Rd, class_id: u32) -> R<ParticleNode> {
        let mut chunks = Vec::new();
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            let c = match cid {
                0x090B3000 => {
                    let version = r.u32()?;
                    if version != 10 {
                        return Err(format!("CPlugParticleEmitterModel chunk 000 version {version} (only 10 is read)"));
                    }
                    let n = r.count()?;
                    if n > 64 {
                        return Err(format!("particle model claims {n} sub-models"));
                    }
                    let mut models = Vec::with_capacity(n);
                    for _ in 0..n {
                        models.push(read_ref(r)?);
                    }
                    PChunk::SubModels { version, models }
                }
                0x090B3001 => PChunk::ModelName(r.id()?),
                0x090B202D => {
                    let version = r.u32()?;
                    if version != 4 {
                        return Err(format!("CPlugParticleEmitterSubModel chunk 02D version {version} (only 4 is read)"));
                    }
                    let mut words = [0u32; 8];
                    for x in words.iter_mut() {
                        *x = r.u32()?;
                    }
                    let name = r.id()?;
                    let u01 = r.u32()?;
                    PChunk::SubModelHead { version, words, name, u01 }
                }
                0x090B202E => {
                    let version = r.u32()?;
                    if version != 0 {
                        return Err(format!("CPlugParticleEmitterSubModel chunk 02E version {version} (only 0 is read)"));
                    }
                    let node = read_ref(r)?;
                    PChunk::RenderNode { version, node }
                }
                0x090B2036 => {
                    let version = r.u32()?;
                    if version != 0 {
                        return Err(format!("CPlugParticleEmitterSubModel chunk 036 version {version} (only 0 is read)"));
                    }
                    let u01 = r.u32()?;
                    let u02 = r.u32()?;
                    let texture = read_ref(r)?;
                    PChunk::Texture { version, u01, u02, texture }
                }
                0x090B203A => {
                    let version = r.u32()?;
                    if version != 1 {
                        return Err(format!("CPlugParticleEmitterSubModel chunk 03A version {version} (only 1 is read)"));
                    }
                    let spawn = read_ref(r)?;
                    let model = read_ref(r)?;
                    PChunk::Gpu { version, spawn, model }
                }
                0x090C5000 => {
                    let version = r.u32()?;
                    if version != 1 {
                        return Err(format!("CPlugParticleGpuSpawn chunk 000 version {version} (only 1 is read)"));
                    }
                    let mut words = [0u32; 15];
                    for x in words.iter_mut() {
                        *x = r.u32()?;
                    }
                    let n = r.count()?;
                    if n > 64 {
                        return Err(format!("GPU spawn claims {n} keys"));
                    }
                    let mut keys = Vec::with_capacity(n);
                    for _ in 0..n {
                        let mut k = [0u32; 4];
                        for x in k.iter_mut() {
                            *x = r.u32()?;
                        }
                        keys.push(k);
                    }
                    PChunk::GpuSpawn { version, words, keys }
                }
                0x09011030 => {
                    let version = r.u32()?;
                    if version != 5 {
                        return Err(format!("CPlugBitmap chunk 030 version {version} (only 5 is read)"));
                    }
                    let image = read_ref(r)?;
                    let tail = r.take(28)?.to_vec();
                    PChunk::BitmapImage { version, image, tail }
                }
                0x09011034 => {
                    let version = r.u32()?;
                    if version != 4 {
                        return Err(format!("CPlugBitmap chunk 034 version {version} (only 4 is read)"));
                    }
                    let r1 = read_ref(r)?;
                    let u01 = r.u32()?;
                    let n = r.count()?;
                    if n > 64 {
                        return Err(format!("bitmap claims {n} frames"));
                    }
                    let mut frames = Vec::with_capacity(n);
                    for _ in 0..n {
                        frames.push(read_ref(r)?);
                    }
                    let r2 = read_ref(r)?;
                    let u02 = r.u32()?;
                    let u03 = r.u32()?;
                    PChunk::BitmapFrames { version, r1, u01, frames, r2, u02, u03 }
                }
                0x0901102A | 0x0901102C => PChunk::SingleRef { id: cid, node: read_ref(r)? },
                0x09011036 => {
                    let version = r.u32()?;
                    if version != 1 {
                        return Err(format!("CPlugBitmap chunk 036 version {version} (only 1 is read)"));
                    }
                    let r1 = read_ref(r)?;
                    let name = r.id()?;
                    let r2 = read_ref(r)?;
                    PChunk::BitmapNamed { version, r1, name, r2 }
                }
                c => match raw_payload_len(c) {
                    Some(n) => PChunk::Raw { id: c, payload: r.take(n)?.to_vec() },
                    None if super::is_skippable_here(r) => {
                        let payload = super::read_skippable_payload(r, c)?;
                        // kept as a Raw with the PIKS framing re-emitted by write()
                        PChunk::Raw { id: c | 0x8000_0000, payload }
                    }
                    None => return Err(format!("particle class 0x{class_id:08X}: chunk 0x{c:08X} at 0x{at:x} has no reader")),
                },
            };
            chunks.push(c);
        }
        Ok(ParticleNode { class_id, chunks })
    }

    pub fn write(&self, w: &mut Wr) {
        for c in &self.chunks {
            match c {
                PChunk::SubModels { version, models } => {
                    w.u32(0x090B3000);
                    w.u32(*version);
                    w.u32(models.len() as u32);
                    for m in models {
                        write_ref(w, m);
                    }
                }
                PChunk::ModelName(id) => {
                    w.u32(0x090B3001);
                    w.id(id);
                }
                PChunk::SubModelHead { version, words, name, u01 } => {
                    w.u32(0x090B202D);
                    w.u32(*version);
                    for x in words {
                        w.u32(*x);
                    }
                    w.id(name);
                    w.u32(*u01);
                }
                PChunk::RenderNode { version, node } => {
                    w.u32(0x090B202E);
                    w.u32(*version);
                    write_ref(w, node);
                }
                PChunk::Texture { version, u01, u02, texture } => {
                    w.u32(0x090B2036);
                    w.u32(*version);
                    w.u32(*u01);
                    w.u32(*u02);
                    write_ref(w, texture);
                }
                PChunk::Gpu { version, spawn, model } => {
                    w.u32(0x090B203A);
                    w.u32(*version);
                    write_ref(w, spawn);
                    write_ref(w, model);
                }
                PChunk::GpuSpawn { version, words, keys } => {
                    w.u32(0x090C5000);
                    w.u32(*version);
                    for x in words {
                        w.u32(*x);
                    }
                    w.u32(keys.len() as u32);
                    for k in keys {
                        for x in k {
                            w.u32(*x);
                        }
                    }
                }
                PChunk::BitmapImage { version, image, tail } => {
                    w.u32(0x09011030);
                    w.u32(*version);
                    write_ref(w, image);
                    w.bytes(tail);
                }
                PChunk::BitmapFrames { version, r1, u01, frames, r2, u02, u03 } => {
                    w.u32(0x09011034);
                    w.u32(*version);
                    write_ref(w, r1);
                    w.u32(*u01);
                    w.u32(frames.len() as u32);
                    for f in frames {
                        write_ref(w, f);
                    }
                    write_ref(w, r2);
                    w.u32(*u02);
                    w.u32(*u03);
                }
                PChunk::SingleRef { id, node } => {
                    w.u32(*id);
                    write_ref(w, node);
                }
                PChunk::BitmapNamed { version, r1, name, r2 } => {
                    w.u32(0x09011036);
                    w.u32(*version);
                    write_ref(w, r1);
                    w.id(name);
                    write_ref(w, r2);
                }
                PChunk::Raw { id, payload } if id & 0x8000_0000 != 0 => super::write_skippable(w, id & 0x7FFF_FFFF, payload),
                PChunk::Raw { id, payload } => {
                    w.u32(*id);
                    w.bytes(payload);
                }
            }
        }
        w.u32(FACADE);
    }

    /// The sub-model references of a `CPlugParticleEmitterModel`.
    pub fn sub_models(&self) -> Vec<&Ref> {
        self.chunks
            .iter()
            .filter_map(|c| match c {
                PChunk::SubModels { models, .. } => Some(models.iter().collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// Every node reference this node's chunks hold, mutable (for relocation).
    pub fn refs_mut(&mut self) -> Vec<&mut Ref> {
        let mut out: Vec<&mut Ref> = Vec::new();
        for c in self.chunks.iter_mut() {
            match c {
                PChunk::SubModels { models, .. } => out.extend(models.iter_mut()),
                PChunk::RenderNode { node, .. } => out.push(node),
                PChunk::Texture { texture, .. } => out.push(texture),
                PChunk::Gpu { spawn, model, .. } => {
                    out.push(spawn);
                    out.push(model);
                }
                PChunk::BitmapImage { image, .. } => out.push(image),
                PChunk::BitmapFrames { r1, frames, r2, .. } => {
                    out.push(r1);
                    out.extend(frames.iter_mut());
                    out.push(r2);
                }
                PChunk::SingleRef { node, .. } => out.push(node),
                PChunk::BitmapNamed { r1, r2, .. } => {
                    out.push(r1);
                    out.push(r2);
                }
                _ => {}
            }
        }
        out
    }

    pub fn describe(&self, depth: usize, out: &mut String) {
        use std::fmt::Write;
        let pad = "  ".repeat(depth);
        let _ = writeln!(out, "{pad}node class 0x{:08X}: {} chunks", self.class_id, self.chunks.len());
        for c in &self.chunks {
            match c {
                PChunk::SubModels { models, .. } => {
                    let _ = writeln!(out, "{pad}  000: {} sub-models -> nodes {:?}", models.len(), models.iter().map(|m| m.index).collect::<Vec<_>>());
                    for m in models {
                        if let Some(super::Node::Particle(n)) = m.inline.as_deref() {
                            n.describe(depth + 2, out);
                        }
                    }
                }
                PChunk::ModelName(id) => {
                    let _ = writeln!(out, "{pad}  001: name {:?}", id.as_str().unwrap_or("?"));
                }
                PChunk::SubModelHead { words, name, u01, .. } => {
                    let _ = writeln!(out, "{pad}  02D: words {:?} name {:?} u01 {u01}", words.iter().map(|w| *w as i32).collect::<Vec<_>>(), name.as_str().unwrap_or("?"));
                }
                PChunk::RenderNode { node, .. } => {
                    let _ = writeln!(out, "{pad}  02E: render node {}", node.index);
                    if let Some(super::Node::Particle(n)) = node.inline.as_deref() {
                        n.describe(depth + 2, out);
                    }
                }
                PChunk::Texture { u01, u02, texture, .. } => {
                    let _ = writeln!(out, "{pad}  036: {u01} {u02} texture node {}", texture.index);
                }
                PChunk::Gpu { spawn, model, .. } => {
                    let _ = writeln!(out, "{pad}  03A: gpu spawn node {} gpu model node {}", spawn.index, model.index);
                    for n in [spawn, model] {
                        if let Some(super::Node::Particle(n)) = n.inline.as_deref() {
                            n.describe(depth + 2, out);
                        }
                    }
                }
                PChunk::GpuSpawn { words, keys, .. } => {
                    let f: Vec<String> = words.iter().map(|w| fmt_word(*w)).collect();
                    let _ = writeln!(out, "{pad}  0C5000: [{}] keys {:?}", f.join(" "), keys.iter().map(|k| k.iter().map(|w| fmt_word(*w)).collect::<Vec<_>>().join(",")).collect::<Vec<_>>());
                }
                PChunk::BitmapImage { image, .. } => {
                    let _ = writeln!(out, "{pad}  030: image node {}", image.index);
                }
                PChunk::BitmapFrames { frames, .. } => {
                    let _ = writeln!(out, "{pad}  034: {} frame refs {:?}", frames.len(), frames.iter().map(|f| f.index).collect::<Vec<_>>());
                }
                PChunk::SingleRef { id, node } => {
                    let _ = writeln!(out, "{pad}  {:03X}: ref node {}", id & 0xFFF, node.index);
                }
                PChunk::BitmapNamed { r1, name, r2, .. } => {
                    let _ = writeln!(out, "{pad}  036: ref {} name {:?} ref {}", r1.index, name.as_str().unwrap_or("(null)"), r2.index);
                }
                PChunk::Raw { id, payload } => {
                    let words: Vec<String> = payload.chunks(4).map(|c| if c.len() == 4 { fmt_word(u32::from_le_bytes([c[0], c[1], c[2], c[3]])) } else { format!("{c:02x?}") }).collect();
                    let _ = writeln!(out, "{pad}  {:03X}: {} bytes [{}]", id & 0xFFF, payload.len(), words.join(" "));
                }
            }
        }
    }
}

/// A word as a float when it looks like one, else as an integer.
fn fmt_word(w: u32) -> String {
    let f = f32::from_bits(w);
    if w == 0 {
        "0".into()
    } else if f.is_finite() && f.abs() >= 1e-4 && f.abs() < 1e6 && (w >> 23) & 0xFF >= 0x60 {
        format!("{f}")
    } else if (w as i32) == -1 {
        "-1".into()
    } else {
        format!("{}", w as i32)
    }
}

/// Every chunk id of the effect-system and particle classes this module
/// reads (the generic walker's `known`).
pub fn is_particle_chunk(cid: u32) -> bool {
    matches!(cid, C_FX_SYSTEM | 0x090B3000 | 0x090B3001 | 0x090B202D | 0x090B202E | 0x090B2036 | 0x090B203A | 0x090C5000 | 0x09011030 | 0x09011034 | 0x09011036 | 0x0901102A | 0x0901102C) || raw_payload_len(cid).is_some()
}

/// A CPlugBitmap chunk this exe's reader does NOT know from a user file
/// (the pack textures' 0x19, 0x20, 0x23, 0x25, 0x28, 0x2A): the switch at
/// exe+0x3f78eb dispatches 0x2B-0x2E, 0x30, 0x32-0x3A; the legacy ids reach
/// the fallback, which knows only the CPlug chunks, and the stream is
/// misread from there (2026-09-08).
pub fn is_legacy_bitmap_chunk(c: &PChunk) -> bool {
    let id = match c {
        PChunk::Raw { id, .. } => *id & 0x7FFF_FFFF,
        PChunk::SingleRef { id, .. } => *id,
        PChunk::BitmapImage { .. } => 0x09011030,
        PChunk::BitmapFrames { .. } => 0x09011034,
        PChunk::BitmapNamed { .. } => 0x09011036,
        _ => return false,
    };
    id >> 12 == 0x09011 && !matches!(id & 0xFFF, 0x02B..=0x02E | 0x030 | 0x032..=0x03A)
}
