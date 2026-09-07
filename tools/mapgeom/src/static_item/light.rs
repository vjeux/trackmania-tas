//! `CPlugLight` (0x0901D000) — the node a Solid2 `lights` socket points at
//! (`Stadium\Media\Light\ItemLampSpot.Light.Gbx`) — and the `GxLight*` node
//! it carries inline (0x0400B000 spot, 0x04002000 ball, 0x0400A000 frustum,
//! 0x04007000 directional, 0x04005000 ambient), typed for a byte-exact round
//! trip and for the static-item builder, which embeds a pack light INLINE in
//! its own Solid2 with the radii scaled.
//!
//! Layouts: GBX.NET `CPlugLight.chunkl` / `GxLight*.chunkl`, checked against
//! all 73 `.Light.Gbx` of the Stadium pack (2026-09-07; they only decrypted
//! past byte 0x100 once the pak "dummy write" was emulated, see `parents.rs`).
//! Chunk 0x0901D004 is not in GBX.NET's chunkl beyond "GxLight + 5 ints": the
//! files say version, GxLight, FuncLight, BitmapFlare, BitmapProjector (the
//! lamp's `ItemLamp_I` texture), flags, ColorTargetTable.

use super::{read_ref, write_ref, Id, Rd, Ref, Wr, R, FACADE};

pub const C_PLUG_LIGHT: u32 = 0x0901D000;

pub fn is_gx_light_class(c: u32) -> bool {
    matches!(c, 0x04001000 | 0x04002000 | 0x04003000 | 0x04005000 | 0x04006000 | 0x04007000 | 0x0400A000 | 0x0400B000)
}

#[derive(Clone, Debug, PartialEq)]
pub enum LightChunk {
    /// 0x0901D000 (flags None) / 0x0901D002 (flags Some): GxLight, FuncLight,
    /// BitmapFlare, BitmapProjector.
    Base { id: u32, refs: [Ref; 4], flags: Option<u32> },
    /// 0x0901D003: ImageAnim, AnimPeriodMin/Max, v1+ AnimTimerName.
    Anim { version: u32, image_anim: Ref, period: [f32; 2], timer_name: Option<Id> },
    /// 0x0901D004.
    Model { version: u32, gx: Ref, func_light: Ref, bitmap_flare: Ref, bitmap_projector: Ref, flags: u32, color_table: Ref },
    Raw(super::RawChunk),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugLight {
    pub chunks: Vec<LightChunk>,
}

impl CPlugLight {
    pub fn parse(r: &mut Rd) -> R<CPlugLight> {
        let mut chunks = Vec::new();
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            let c = match cid {
                0x0901D000 | 0x0901D002 => {
                    let refs = [read_ref(r)?, read_ref(r)?, read_ref(r)?, read_ref(r)?];
                    let flags = if cid == 0x0901D002 { Some(r.u32()?) } else { None };
                    LightChunk::Base { id: cid, refs, flags }
                }
                0x0901D003 => {
                    let version = r.u32()?;
                    let image_anim = read_ref(r)?;
                    let period = [r.f32()?, r.f32()?];
                    let timer_name = if version >= 1 { Some(r.id()?) } else { None };
                    LightChunk::Anim { version, image_anim, period, timer_name }
                }
                0x0901D004 => {
                    let version = r.u32()?;
                    let gx = read_ref(r)?;
                    let func_light = read_ref(r)?;
                    let bitmap_flare = read_ref(r)?;
                    let bitmap_projector = read_ref(r)?;
                    let flags = r.u32()?;
                    let color_table = read_ref(r)?;
                    LightChunk::Model { version, gx, func_light, bitmap_flare, bitmap_projector, flags, color_table }
                }
                c if super::is_skippable_here(r) => LightChunk::Raw(super::RawChunk { id: c, payload: super::read_skippable_payload(r, c)? }),
                c => return Err(format!("CPlugLight chunk 0x{c:08X} at 0x{at:x} has no reader")),
            };
            chunks.push(c);
        }
        Ok(CPlugLight { chunks })
    }

    pub fn write(&self, w: &mut Wr) {
        for c in &self.chunks {
            match c {
                LightChunk::Base { id, refs, flags } => {
                    w.u32(*id);
                    refs.iter().for_each(|x| write_ref(w, x));
                    if let Some(f) = flags {
                        w.u32(*f);
                    }
                }
                LightChunk::Anim { version, image_anim, period, timer_name } => {
                    w.u32(0x0901D003);
                    w.u32(*version);
                    write_ref(w, image_anim);
                    w.floats(period);
                    if let Some(id) = timer_name {
                        w.id(id);
                    }
                }
                LightChunk::Model { version, gx, func_light, bitmap_flare, bitmap_projector, flags, color_table } => {
                    w.u32(0x0901D004);
                    w.u32(*version);
                    write_ref(w, gx);
                    write_ref(w, func_light);
                    write_ref(w, bitmap_flare);
                    write_ref(w, bitmap_projector);
                    w.u32(*flags);
                    write_ref(w, color_table);
                }
                LightChunk::Raw(rc) => super::write_skippable(w, rc.id, &rc.payload),
            }
        }
        w.u32(FACADE);
    }

    /// The GxLight reference (chunk 004, else the base chunk).
    pub fn gx(&self) -> Option<&Ref> {
        self.chunks.iter().find_map(|c| match c {
            LightChunk::Model { gx, .. } => Some(gx),
            _ => None,
        }).or_else(|| {
            self.chunks.iter().find_map(|c| match c {
                LightChunk::Base { refs, .. } => Some(&refs[0]),
                _ => None,
            })
        })
    }

    pub fn gx_mut(&mut self) -> Option<&mut Ref> {
        let idx = self.chunks.iter().position(|c| matches!(c, LightChunk::Model { .. })).or_else(|| self.chunks.iter().position(|c| matches!(c, LightChunk::Base { .. })))?;
        match &mut self.chunks[idx] {
            LightChunk::Model { gx, .. } => Some(gx),
            LightChunk::Base { refs, .. } => Some(&mut refs[0]),
            _ => None,
        }
    }

    /// The light's own inline GxLight, if it carries one.
    pub fn gx_light(&self) -> Option<&GxLight> {
        match self.gx()?.inline.as_deref()? {
            super::Node::GxLight(g) => Some(g),
            _ => None,
        }
    }

    /// Driven by an animation image (chunk 003 `ImageAnim`) or a `CFuncLight`
    /// (chunk 000/002/004): the light's output is not constant.
    pub fn is_animated(&self) -> bool {
        self.chunks.iter().any(|c| match c {
            LightChunk::Anim { image_anim, .. } => image_anim.index >= 0 || image_anim.inline.is_some(),
            LightChunk::Base { refs, .. } => refs[1].index >= 0 || refs[1].inline.is_some(),
            LightChunk::Model { func_light, .. } => func_light.index >= 0 || func_light.inline.is_some(),
            LightChunk::Raw(_) => false,
        })
    }

    /// Every node reference except the GxLight (FuncLight, the flare and
    /// projector bitmaps, the animation image, the colour table) set to null:
    /// they name files of the pack, which an item embedded in a map cannot
    /// reach through its own reference table.
    pub fn drop_external_refs(&mut self) {
        for c in self.chunks.iter_mut() {
            match c {
                LightChunk::Base { refs, .. } => {
                    for r in refs.iter_mut().skip(1) {
                        if r.inline.is_none() {
                            *r = super::null_ref();
                        }
                    }
                }
                LightChunk::Anim { image_anim, .. } => {
                    if image_anim.inline.is_none() {
                        *image_anim = super::null_ref();
                    }
                }
                LightChunk::Model { func_light, bitmap_flare, bitmap_projector, color_table, .. } => {
                    for r in [func_light, bitmap_flare, bitmap_projector, color_table] {
                        if r.inline.is_none() {
                            *r = super::null_ref();
                        }
                    }
                }
                LightChunk::Raw(_) => {}
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GxChunk {
    /// 0x04001008
    Light08 { color: [f32; 3], intensity: f32, flags: u32, shadow_intensity: f32, flare_intensity: f32, shadow_rgb: [f32; 3] },
    /// 0x04001009
    Light09 { color: [f32; 3], flags: u32, intensity: f32, diffuse_intensity: f32, specular_intens: f32, specular_power: f32, shadow_intensity: f32, flare_intensity: f32, shadow_rgb: [f32; 3] },
    /// 0x0400100A
    Light0A { version: u32, color: [f32; 3], flags: u32, intensity: f32, diffuse_intensity: f32, shadow_intensity: f32, flare_intensity: f32, shadow_rgb: [f32; 3] },
    /// 0x04005000 GxLightAmbient
    Ambient00 { shade_min_y: f32, shade_max_y: f32 },
    /// 0x04003003 / 0x04003004 GxLightPoint
    Point { id: u32, flare_size: f32, flare_bias_z: Option<f32> },
    /// 0x04002002 GxLightBall
    Ball02 { radius: f32, attenuation: [f32; 2], emitting_radius: f32, ambient_rgb: [f32; 3] },
    /// 0x04002006
    Ball06 { flags: u32, radius: f32, radius_specular: f32, radius_shadow: f32, radius_flare: f32, emitting_radius: f32, attenuation: [f32; 2], ambient_rgb: [f32; 3] },
    /// 0x04002008
    Ball08 {
        flags: u32,
        radius: f32,
        radius_specular: f32,
        radius_shadow: f32,
        radius_flare: f32,
        emitting_radius: f32,
        emitting_cylinder_len_z: f32,
        att_htnlr: [f32; 2],
        ambient_rgb: [f32; 3],
        att_hyper2: [f32; 2],
    },
    /// 0x04002009 (the radius again in every pack light) / 0x0400200A (1/64).
    BallFloat { id: u32, value: f32 },
    /// 0x0400A004 GxLightFrustum
    Frustum04 { ints: [i32; 3], floats: [f32; 4], int: i32 },
    /// 0x0400A006
    Frustum06 { flag: u32, aligned_box: [f32; 6], u: u32 },
    /// 0x0400B001 GxLightSpot
    Spot01 { angle_inner: f32, angle_outer: f32, angle_flare: f32, falloff_exponent: f32 },
    /// 0x0400B002 / 0x0400B003 (version Some): the tail is two bytes from
    /// v1, one int at v0, nothing on 002.
    Spot { version: Option<u32>, flags: u32, angle_inner: f32, angle_outer: f32, angle_flare: f32, angle_inner_shadow: f32, angle_outer_shadow: f32, falloff_exponent: f32, tail: SpotTail },
    /// 0x04007001..5 GxLightDirectional, kept as words.
    Directional { id: u32, words: Vec<u32> },
    Raw(super::RawChunk),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpotTail {
    None,
    Bytes([u8; 2]),
    Int(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GxLight {
    pub class_id: u32,
    pub chunks: Vec<GxChunk>,
}

impl GxLight {
    pub fn parse(r: &mut Rd, class_id: u32) -> R<GxLight> {
        let mut chunks = Vec::new();
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            let c = match cid {
                0x04001008 => GxChunk::Light08 { color: r.vec3()?, intensity: r.f32()?, flags: r.u32()?, shadow_intensity: r.f32()?, flare_intensity: r.f32()?, shadow_rgb: r.vec3()? },
                0x04001009 => GxChunk::Light09 {
                    color: r.vec3()?,
                    flags: r.u32()?,
                    intensity: r.f32()?,
                    diffuse_intensity: r.f32()?,
                    specular_intens: r.f32()?,
                    specular_power: r.f32()?,
                    shadow_intensity: r.f32()?,
                    flare_intensity: r.f32()?,
                    shadow_rgb: r.vec3()?,
                },
                0x0400100A => GxChunk::Light0A {
                    version: r.u32()?,
                    color: r.vec3()?,
                    flags: r.u32()?,
                    intensity: r.f32()?,
                    diffuse_intensity: r.f32()?,
                    shadow_intensity: r.f32()?,
                    flare_intensity: r.f32()?,
                    shadow_rgb: r.vec3()?,
                },
                0x04005000 => GxChunk::Ambient00 { shade_min_y: r.f32()?, shade_max_y: r.f32()? },
                0x04003003 => GxChunk::Point { id: cid, flare_size: r.f32()?, flare_bias_z: None },
                0x04003004 => GxChunk::Point { id: cid, flare_size: r.f32()?, flare_bias_z: Some(r.f32()?) },
                0x04002002 => GxChunk::Ball02 { radius: r.f32()?, attenuation: [r.f32()?, r.f32()?], emitting_radius: r.f32()?, ambient_rgb: r.vec3()? },
                0x04002006 => GxChunk::Ball06 {
                    flags: r.u32()?,
                    radius: r.f32()?,
                    radius_specular: r.f32()?,
                    radius_shadow: r.f32()?,
                    radius_flare: r.f32()?,
                    emitting_radius: r.f32()?,
                    attenuation: [r.f32()?, r.f32()?],
                    ambient_rgb: r.vec3()?,
                },
                0x04002008 => GxChunk::Ball08 {
                    flags: r.u32()?,
                    radius: r.f32()?,
                    radius_specular: r.f32()?,
                    radius_shadow: r.f32()?,
                    radius_flare: r.f32()?,
                    emitting_radius: r.f32()?,
                    emitting_cylinder_len_z: r.f32()?,
                    att_htnlr: [r.f32()?, r.f32()?],
                    ambient_rgb: r.vec3()?,
                    att_hyper2: [r.f32()?, r.f32()?],
                },
                0x04002009 | 0x0400200A => GxChunk::BallFloat { id: cid, value: r.f32()? },
                0x0400A004 => GxChunk::Frustum04 { ints: [r.i32()?, r.i32()?, r.i32()?], floats: r.floats::<4>()?, int: r.i32()? },
                0x0400A006 => GxChunk::Frustum06 { flag: r.u32()?, aligned_box: r.floats::<6>()?, u: r.u32()? },
                0x0400B001 => GxChunk::Spot01 { angle_inner: r.f32()?, angle_outer: r.f32()?, angle_flare: r.f32()?, falloff_exponent: r.f32()? },
                0x0400B002 | 0x0400B003 => {
                    let version = if cid == 0x0400B003 { Some(r.u32()?) } else { None };
                    let flags = r.u32()?;
                    let angle_inner = r.f32()?;
                    let angle_outer = r.f32()?;
                    let angle_flare = r.f32()?;
                    let angle_inner_shadow = r.f32()?;
                    let angle_outer_shadow = r.f32()?;
                    let falloff_exponent = r.f32()?;
                    let tail = match version {
                        None => SpotTail::None,
                        Some(v) if v >= 1 => SpotTail::Bytes([r.u8()?, r.u8()?]),
                        Some(_) => SpotTail::Int(r.i32()?),
                    };
                    GxChunk::Spot { version, flags, angle_inner, angle_outer, angle_flare, angle_inner_shadow, angle_outer_shadow, falloff_exponent, tail }
                }
                0x04007001 | 0x04007002 | 0x04007003 | 0x04007004 | 0x04007005 => {
                    let n = match cid {
                        0x04007001 => 4,
                        0x04007002 => 6,
                        0x04007003 => 3,
                        0x04007004 => 4,
                        _ => 2,
                    };
                    let mut words = Vec::with_capacity(n);
                    for _ in 0..n {
                        words.push(r.u32()?);
                    }
                    GxChunk::Directional { id: cid, words }
                }
                c if super::is_skippable_here(r) => GxChunk::Raw(super::RawChunk { id: c, payload: super::read_skippable_payload(r, c)? }),
                c => return Err(format!("GxLight 0x{class_id:08X} chunk 0x{c:08X} at 0x{at:x} has no reader")),
            };
            chunks.push(c);
        }
        Ok(GxLight { class_id, chunks })
    }

    pub fn write(&self, w: &mut Wr) {
        for c in &self.chunks {
            match c {
                GxChunk::Light08 { color, intensity, flags, shadow_intensity, flare_intensity, shadow_rgb } => {
                    w.u32(0x04001008);
                    w.floats(color);
                    w.f32(*intensity);
                    w.u32(*flags);
                    w.f32(*shadow_intensity);
                    w.f32(*flare_intensity);
                    w.floats(shadow_rgb);
                }
                GxChunk::Light09 { color, flags, intensity, diffuse_intensity, specular_intens, specular_power, shadow_intensity, flare_intensity, shadow_rgb } => {
                    w.u32(0x04001009);
                    w.floats(color);
                    w.u32(*flags);
                    w.floats(&[*intensity, *diffuse_intensity, *specular_intens, *specular_power, *shadow_intensity, *flare_intensity]);
                    w.floats(shadow_rgb);
                }
                GxChunk::Light0A { version, color, flags, intensity, diffuse_intensity, shadow_intensity, flare_intensity, shadow_rgb } => {
                    w.u32(0x0400100A);
                    w.u32(*version);
                    w.floats(color);
                    w.u32(*flags);
                    w.floats(&[*intensity, *diffuse_intensity, *shadow_intensity, *flare_intensity]);
                    w.floats(shadow_rgb);
                }
                GxChunk::Ambient00 { shade_min_y, shade_max_y } => {
                    w.u32(0x04005000);
                    w.f32(*shade_min_y);
                    w.f32(*shade_max_y);
                }
                GxChunk::Point { id, flare_size, flare_bias_z } => {
                    w.u32(*id);
                    w.f32(*flare_size);
                    if let Some(b) = flare_bias_z {
                        w.f32(*b);
                    }
                }
                GxChunk::Ball02 { radius, attenuation, emitting_radius, ambient_rgb } => {
                    w.u32(0x04002002);
                    w.f32(*radius);
                    w.floats(attenuation);
                    w.f32(*emitting_radius);
                    w.floats(ambient_rgb);
                }
                GxChunk::Ball06 { flags, radius, radius_specular, radius_shadow, radius_flare, emitting_radius, attenuation, ambient_rgb } => {
                    w.u32(0x04002006);
                    w.u32(*flags);
                    w.floats(&[*radius, *radius_specular, *radius_shadow, *radius_flare, *emitting_radius]);
                    w.floats(attenuation);
                    w.floats(ambient_rgb);
                }
                GxChunk::Ball08 { flags, radius, radius_specular, radius_shadow, radius_flare, emitting_radius, emitting_cylinder_len_z, att_htnlr, ambient_rgb, att_hyper2 } => {
                    w.u32(0x04002008);
                    w.u32(*flags);
                    w.floats(&[*radius, *radius_specular, *radius_shadow, *radius_flare, *emitting_radius, *emitting_cylinder_len_z]);
                    w.floats(att_htnlr);
                    w.floats(ambient_rgb);
                    w.floats(att_hyper2);
                }
                GxChunk::BallFloat { id, value } => {
                    w.u32(*id);
                    w.f32(*value);
                }
                GxChunk::Frustum04 { ints, floats, int } => {
                    w.u32(0x0400A004);
                    ints.iter().for_each(|x| w.i32(*x));
                    w.floats(floats);
                    w.i32(*int);
                }
                GxChunk::Frustum06 { flag, aligned_box, u } => {
                    w.u32(0x0400A006);
                    w.u32(*flag);
                    w.floats(aligned_box);
                    w.u32(*u);
                }
                GxChunk::Spot01 { angle_inner, angle_outer, angle_flare, falloff_exponent } => {
                    w.u32(0x0400B001);
                    w.floats(&[*angle_inner, *angle_outer, *angle_flare, *falloff_exponent]);
                }
                GxChunk::Spot { version, flags, angle_inner, angle_outer, angle_flare, angle_inner_shadow, angle_outer_shadow, falloff_exponent, tail } => {
                    w.u32(if version.is_some() { 0x0400B003 } else { 0x0400B002 });
                    if let Some(v) = version {
                        w.u32(*v);
                    }
                    w.u32(*flags);
                    w.floats(&[*angle_inner, *angle_outer, *angle_flare, *angle_inner_shadow, *angle_outer_shadow, *falloff_exponent]);
                    match tail {
                        SpotTail::None => {}
                        SpotTail::Bytes(b) => w.bytes(b),
                        SpotTail::Int(i) => w.i32(*i),
                    }
                }
                GxChunk::Directional { id, words } => {
                    w.u32(*id);
                    words.iter().for_each(|x| w.u32(*x));
                }
                GxChunk::Raw(rc) => super::write_skippable(w, rc.id, &rc.payload),
            }
        }
        w.u32(FACADE);
    }

    /// Every distance the light carries times `s`: the ball radii (range,
    /// specular, shadow, flare), the emitting radius and cylinder, chunk
    /// 0x04002009 (the range again in every pack light), the frustum box.
    /// Angles, colours, intensities and attenuation shapes are unitless.
    pub fn scale(&mut self, s: f32) {
        for c in self.chunks.iter_mut() {
            match c {
                GxChunk::Ball02 { radius, emitting_radius, .. } => {
                    *radius *= s;
                    *emitting_radius *= s;
                }
                GxChunk::Ball06 { radius, radius_specular, radius_shadow, radius_flare, emitting_radius, .. } => {
                    for x in [radius, radius_specular, radius_shadow, radius_flare, emitting_radius] {
                        *x *= s;
                    }
                }
                GxChunk::Ball08 { radius, radius_specular, radius_shadow, radius_flare, emitting_radius, emitting_cylinder_len_z, .. } => {
                    for x in [radius, radius_specular, radius_shadow, radius_flare, emitting_radius, emitting_cylinder_len_z] {
                        *x *= s;
                    }
                }
                GxChunk::BallFloat { id: 0x04002009, value } => *value *= s,
                // the light sprite (GxLight flag 32): its size and its offset
                // along the beam are metres too — unscaled, the half Lamp's glow
                // filled a frame the stock one lit a quarter of
                GxChunk::Point { flare_size, flare_bias_z, .. } => {
                    *flare_size *= s;
                    if let Some(b) = flare_bias_z {
                        *b *= s;
                    }
                }
                GxChunk::Frustum06 { aligned_box, .. } => aligned_box.iter_mut().for_each(|x| *x *= s),
                _ => {}
            }
        }
    }

    /// Multiply the light's colour (every GxLight chunk form) by `rgb`: a
    /// placement's light colour skin (light_skin.rs).
    pub fn tint(&mut self, rgb: [f32; 3]) {
        for c in self.chunks.iter_mut() {
            match c {
                GxChunk::Light08 { color, .. } | GxChunk::Light09 { color, .. } | GxChunk::Light0A { color, .. } => {
                    for k in 0..3 {
                        color[k] *= rgb[k];
                    }
                }
                _ => {}
            }
        }
    }

    /// (colour, intensity, range) for reports.
    pub fn summary(&self) -> ([f32; 3], f32, f32) {
        let mut color = [0.0; 3];
        let mut intensity = 0.0;
        let mut range = 0.0;
        for c in &self.chunks {
            match c {
                GxChunk::Light08 { color: col, intensity: i, .. } | GxChunk::Light09 { color: col, intensity: i, .. } | GxChunk::Light0A { color: col, intensity: i, .. } => {
                    color = *col;
                    intensity = *i;
                }
                GxChunk::Ball02 { radius, .. } | GxChunk::Ball06 { radius, .. } | GxChunk::Ball08 { radius, .. } => range = *radius,
                _ => {}
            }
        }
        (color, intensity, range)
    }
}

/// `CPlugLightUserModel` (0x090F9000): the light the ITEM EDITOR / NadeoImporter
/// writes for a custom item (`MeshParams.xml` `<Light Type="Spot|Point" …/>`),
/// listed in a Solid2's `light_user_models` and placed on a `lights` socket
/// through `light_insts` (model index, socket index). Layout: GBX.NET
/// `CPlugLightUserModel.chunkl` (version, int, Color, Intensity, Distance,
/// PointEmissionRadius, PointEmissionLength, SpotInnerAngle, SpotOuterAngle,
/// SpotEmissionSizeX, SpotEmissionSizeY, v1+ NightOnly); the exe's reflection
/// lists the same members. The unnamed int is read as the light KIND
/// (0 point, 1 spot — a guess to be verified in-game, 2026-09-07).
#[derive(Clone, Debug, PartialEq)]
pub struct CPlugLightUserModel {
    pub version: u32,
    pub kind: i32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub distance: f32,
    pub point_emission_radius: f32,
    pub point_emission_length: f32,
    pub spot_inner_angle: f32,
    pub spot_outer_angle: f32,
    pub spot_emission_size_x: f32,
    pub spot_emission_size_y: f32,
    pub night_only: Option<bool>,
    pub raw: Vec<super::RawChunk>,
}

pub const C_LIGHT_USER_MODEL: u32 = 0x090F9000;

impl CPlugLightUserModel {
    pub fn parse(r: &mut Rd) -> R<CPlugLightUserModel> {
        let mut m = CPlugLightUserModel { version: 1, kind: 0, color: [1.0; 3], intensity: 1.0, distance: 10.0, point_emission_radius: 0.0, point_emission_length: 0.0, spot_inner_angle: 0.0, spot_outer_angle: 0.0, spot_emission_size_x: 0.0, spot_emission_size_y: 0.0, night_only: None, raw: Vec::new() };
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            match cid {
                0x090F9000 => {
                    m.version = r.u32()?;
                    m.kind = r.i32()?;
                    m.color = r.vec3()?;
                    m.intensity = r.f32()?;
                    m.distance = r.f32()?;
                    m.point_emission_radius = r.f32()?;
                    m.point_emission_length = r.f32()?;
                    m.spot_inner_angle = r.f32()?;
                    m.spot_outer_angle = r.f32()?;
                    m.spot_emission_size_x = r.f32()?;
                    m.spot_emission_size_y = r.f32()?;
                    m.night_only = if m.version >= 1 { Some(r.bool32()?) } else { None };
                }
                c if super::is_skippable_here(r) => m.raw.push(super::RawChunk { id: c, payload: super::read_skippable_payload(r, c)? }),
                c => return Err(format!("CPlugLightUserModel chunk 0x{c:08X} at 0x{at:x} has no reader")),
            }
        }
        Ok(m)
    }

    pub fn write(&self, w: &mut Wr) {
        w.u32(0x090F9000);
        w.u32(self.version);
        w.i32(self.kind);
        w.floats(&self.color);
        w.floats(&[self.intensity, self.distance, self.point_emission_radius, self.point_emission_length, self.spot_inner_angle, self.spot_outer_angle, self.spot_emission_size_x, self.spot_emission_size_y]);
        if self.version >= 1 {
            w.bool32(self.night_only.unwrap_or(false));
        }
        for rc in &self.raw {
            super::write_skippable(w, rc.id, &rc.payload);
        }
        w.u32(FACADE);
    }

    /// The item-editor form of a pack light: colour and intensity as they
    /// are, the ball radius as the distance, the emitting radius/cylinder as
    /// the point emission, the spot angles when the GxLight is a spot.
    pub fn from_gx(g: &GxLight) -> CPlugLightUserModel {
        let (color, intensity, range) = g.summary();
        let mut m = CPlugLightUserModel { version: 1, kind: 0, color, intensity, distance: range, point_emission_radius: 0.0, point_emission_length: 0.0, spot_inner_angle: 0.0, spot_outer_angle: 0.0, spot_emission_size_x: 0.0, spot_emission_size_y: 0.0, night_only: Some(false), raw: Vec::new() };
        for c in &g.chunks {
            match c {
                GxChunk::Ball02 { emitting_radius, .. } | GxChunk::Ball06 { emitting_radius, .. } => m.point_emission_radius = *emitting_radius,
                GxChunk::Ball08 { emitting_radius, emitting_cylinder_len_z, .. } => {
                    m.point_emission_radius = *emitting_radius;
                    m.point_emission_length = *emitting_cylinder_len_z;
                }
                GxChunk::Spot01 { angle_inner, angle_outer, .. } | GxChunk::Spot { angle_inner, angle_outer, .. } => {
                    m.spot_inner_angle = *angle_inner;
                    m.spot_outer_angle = *angle_outer;
                }
                _ => {}
            }
        }
        if g.class_id == 0x0400B000 {
            m.kind = std::env::var("TINY_LIGHT_SPOT_KIND").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
            m.spot_emission_size_x = m.point_emission_radius;
            m.spot_emission_size_y = m.point_emission_radius;
        }
        m
    }
}

impl CPlugLight {
    /// The external files this light's references name, as (path, slot) with
    /// slot 0 = flare bitmap, 1 = projector bitmap, 2 = colour table, 3 =
    /// animation image — looked up in the file's reference table `externals`
    /// (node index -> path).
    pub fn external_slots(&self, externals: &[(u32, String)]) -> Vec<(String, u8)> {
        let name = |r: &Ref| -> Option<String> {
            if r.inline.is_some() || r.index < 0 {
                return None;
            }
            externals.iter().find(|(i, _)| *i as i32 == r.index).map(|(_, p)| p.clone())
        };
        let mut out = Vec::new();
        for c in &self.chunks {
            match c {
                LightChunk::Base { refs, .. } => {
                    if let Some(p) = name(&refs[2]) {
                        out.push((p, 0));
                    }
                    if let Some(p) = name(&refs[3]) {
                        out.push((p, 1));
                    }
                }
                LightChunk::Model { bitmap_flare, bitmap_projector, color_table, .. } => {
                    if let Some(p) = name(bitmap_flare) {
                        out.push((p, 0));
                    }
                    if let Some(p) = name(bitmap_projector) {
                        out.push((p, 1));
                    }
                    if let Some(p) = name(color_table) {
                        out.push((p, 2));
                    }
                }
                LightChunk::Anim { image_anim, .. } => {
                    if let Some(p) = name(image_anim) {
                        out.push((p, 3));
                    }
                }
                LightChunk::Raw(_) => {}
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Point the slot's reference (see `external_slots`) at `r`.
    pub fn set_bitmap(&mut self, slot: u8, r: Ref) {
        for c in self.chunks.iter_mut() {
            match c {
                LightChunk::Base { refs, .. } => match slot {
                    0 => refs[2] = r.clone(),
                    1 => refs[3] = r.clone(),
                    _ => {}
                },
                LightChunk::Model { bitmap_flare, bitmap_projector, color_table, .. } => match slot {
                    0 => *bitmap_flare = r.clone(),
                    1 => *bitmap_projector = r.clone(),
                    2 => *color_table = r.clone(),
                    _ => {}
                },
                LightChunk::Anim { image_anim, .. } => {
                    if slot == 3 {
                        *image_anim = r.clone();
                    }
                }
                LightChunk::Raw(_) => {}
            }
        }
    }
}

/// A standalone `.Light.Gbx` (class 0x0901D000, GBX version 6, uncompressed)
/// holding `light` with its GxLight inline as node 1 and `externals` as the
/// reference table (node index, path) — the file the production bake puts
/// NEXT TO THE ITEM (`Items/<stem>_L<k>.Light.Gbx`) and points the socket at:
/// an embedded item resolves a level-0 reference-table path both against its
/// own archive folder and against the game's packs (probe of 2026-09-07: a
/// socket naming `Stadium\Media\Light\ItemLampSpot.Light.Gbx` lit the grass
/// exactly like the stock Lamp; an INLINE CPlugLight rendered no sprite and
/// could not reach its projector texture).
pub fn light_file(light: &CPlugLight, externals: &[(u32, String)]) -> Vec<u8> {
    let mut light = light.clone();
    if let Some(gx) = light.gx_mut() {
        if gx.inline.is_some() {
            gx.index = 1;
        }
    }
    let mut body = Vec::new();
    let mut lb = super::LookbackState::default();
    lb.defined_nodes.extend(externals.iter().map(|(i, _)| *i));
    {
        let mut w = Wr { w: &mut body, lb: &mut lb };
        light.write(&mut w);
    }
    let num_nodes = externals.iter().map(|(i, _)| *i + 1).max().unwrap_or(2).max(2);
    let mut out = Vec::with_capacity(body.len() + 256);
    out.extend_from_slice(b"GBX");
    out.extend_from_slice(&6u16.to_le_bytes());
    out.push(b'B');
    out.push(b'U');
    out.push(b'U');
    out.push(b'R');
    out.extend_from_slice(&C_PLUG_LIGHT.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // no header chunks
    out.extend_from_slice(&num_nodes.to_le_bytes());
    out.extend_from_slice(&super::file::ref_table(0, externals));
    out.extend_from_slice(&body);
    out
}
