//! `CPlugSolid2Model` (0x090BB000): chunk 0x090BB000 (every version GBX.NET
//! reads, TM2020 writes 34) and the skippable 0x090BB002 kept raw.

use super::{read_ref, write_ref, Id, Rd, Ref, Wr, R, FACADE};

#[derive(Clone, Debug, PartialEq)]
pub struct ShadedGeom {
    pub visual_index: i32,
    pub material_index: i32,
    pub u01: i32,
    /// v1+
    pub lod_mask: i32,
    /// v32+
    pub u02: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreLightGen {
    pub version: u32,
    pub u01: i32,
    pub u02: f32,
    pub u03: bool,
    pub u04: [f32; 8],
    pub sprite_count: [i32; 2],
    pub boxes: Vec<[f32; 6]>,
    /// v1+
    pub uv_groups: Vec<[i32; 4]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    pub u01: Id,
    pub u02: bool,
    /// When `u02`: an external CPlugLight; else a string.
    pub node: Ref,
    pub u04: String,
    pub u05: [f32; 12],
    /// Six ints. (The chunkl marks three more as "v26+", but GBX.NET reads
    /// the Light archive with version 0 -- `ArrayReadableWritable<Light>`
    /// passes none -- and the pack bytes agree: `TreeGen\RoadBorderSpot`
    /// has 6 ints, then the bool.)
    pub ints: [i32; 6],
    pub u15: bool,
    pub u16: [f32; 3],
}

/// A custom material slot: a name, or (empty name) an inline material node.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub node: Option<Ref>,
}

impl Material {
    pub fn inst(&self) -> Option<&crate::crystal_model::CPlugMaterialUserInst> {
        match self.node.as_ref()?.inline.as_deref()? {
            super::Node::Material(m) => Some(m),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CPlugSolid2Model {
    /// Chunk ids in file order (0x090BB000, then any skippable ones).
    pub chunks: Vec<u32>,
    pub version: u32,
    pub u01: Id,
    pub shaded_geoms: Vec<ShadedGeom>,
    /// v6+: (deprec word, visuals).
    pub visuals_deprec: i32,
    pub visuals: Vec<Ref>,
    pub material_ids: Vec<Id>,
    /// Written when no custom materials: (deprec word, external refs).
    pub materials_deprec: i32,
    pub materials: Vec<Ref>,
    pub skel: Ref,
    pub lod_max_dist: Vec<f32>,
    pub vis_cst_type: i32,
    pub pre_light_gen: Option<PreLightGen>,
    pub file_write_time: u64,
    pub u03: String,
    pub materials_folder: String,
    pub u04: String,
    pub lights: Vec<Light>,
    /// v<16
    pub material_insts: Vec<Ref>,
    pub light_user_models: Vec<Ref>,
    pub light_insts: Vec<(i32, i32)>,
    pub damage_zone: i32,
    pub flags: u32,
    pub u05: i32,
    pub u06: String,
    /// v30+
    pub u07: i32,
    pub custom_materials: Vec<Material>,
    /// v17..21
    pub boxes: Vec<[f32; 6]>,
    pub joints: Vec<Id>,
    pub u10: Vec<i32>,
    pub u11: i32,
    pub u12: Vec<i32>,
    pub u13: i32,
    pub u14: Ref,
    pub u15: f32,
    pub u16: f32,
    pub u17: Id,
    pub u18: i32,
    pub u19: Vec<[i32; 5]>,
    /// Skippable chunks (0x090BB002) kept raw, by id.
    pub raw: Vec<super::RawChunk>,
}

impl CPlugSolid2Model {
    /// Empty model at TM2020's version 34, with the constants every reference
    /// item carries.
    pub fn new_v34() -> CPlugSolid2Model {
        CPlugSolid2Model {
            chunks: vec![0x090BB000, 0x090BB002],
            version: 34,
            u01: Id::Null,
            shaded_geoms: Vec::new(),
            visuals_deprec: 10,
            visuals: Vec::new(),
            material_ids: Vec::new(),
            materials_deprec: 10,
            materials: Vec::new(),
            skel: super::null_ref(),
            lod_max_dist: Vec::new(),
            vis_cst_type: 1,
            pre_light_gen: None,
            file_write_time: 0,
            u03: String::new(),
            materials_folder: String::new(),
            u04: String::new(),
            lights: Vec::new(),
            material_insts: Vec::new(),
            light_user_models: Vec::new(),
            light_insts: Vec::new(),
            damage_zone: 0,
            flags: 0,
            u05: 1,
            u06: String::new(),
            u07: 1,
            custom_materials: Vec::new(),
            boxes: Vec::new(),
            joints: Vec::new(),
            u10: Vec::new(),
            u11: 0,
            u12: Vec::new(),
            u13: 0,
            u14: super::null_ref(),
            u15: 1.0,
            u16: 1.0,
            u17: Id::Null,
            u18: 0,
            u19: Vec::new(),
            raw: vec![super::RawChunk { id: 0x090BB002, payload: vec![0; 8] }],
        }
    }
}

fn read_light(r: &mut Rd) -> R<Light> {
    let u01 = r.id()?;
    let u02 = r.bool32()?;
    let mut l = Light { u01, u02, node: super::null_ref(), u04: String::new(), u05: [0.0; 12], ints: [0; 6], u15: false, u16: [0.0; 3] };
    if u02 {
        l.node = read_ref(r)?;
    } else {
        l.u04 = r.string()?;
    }
    l.u05 = r.floats::<12>()?;
    for x in l.ints.iter_mut() {
        *x = r.i32()?;
    }
    l.u15 = r.bool32()?;
    if l.u15 {
        l.u16 = r.vec3()?;
    }
    Ok(l)
}

fn write_light(w: &mut Wr, l: &Light) {
    w.id(&l.u01);
    w.bool32(l.u02);
    if l.u02 {
        write_ref(w, &l.node);
    } else {
        w.string(&l.u04);
    }
    w.floats(&l.u05);
    l.ints.iter().for_each(|x| w.i32(*x));
    w.bool32(l.u15);
    if l.u15 {
        w.floats(&l.u16);
    }
}

fn read_prelight(r: &mut Rd) -> R<PreLightGen> {
    let version = r.u32()?;
    let u01 = r.i32()?;
    let u02 = r.f32()?;
    let u03 = r.bool32()?;
    let u04 = r.floats::<8>()?;
    let sprite_count = [r.i32()?, r.i32()?];
    let boxes = r.array(|r| r.floats::<6>())?;
    let uv_groups = if version >= 1 { r.array(|r| Ok([r.i32()?, r.i32()?, r.i32()?, r.i32()?]))? } else { Vec::new() };
    Ok(PreLightGen { version, u01, u02, u03, u04, sprite_count, boxes, uv_groups })
}

fn write_prelight(w: &mut Wr, p: &PreLightGen) {
    w.u32(p.version);
    w.i32(p.u01);
    w.f32(p.u02);
    w.bool32(p.u03);
    w.floats(&p.u04);
    w.i32(p.sprite_count[0]);
    w.i32(p.sprite_count[1]);
    w.u32(p.boxes.len() as u32);
    p.boxes.iter().for_each(|b| w.floats(b));
    if p.version >= 1 {
        w.u32(p.uv_groups.len() as u32);
        for g in &p.uv_groups {
            g.iter().for_each(|x| w.i32(*x));
        }
    }
}

fn read_ids(r: &mut Rd) -> R<Vec<Id>> {
    r.array(|r| r.id())
}

fn write_ids(w: &mut Wr, ids: &[Id]) {
    w.u32(ids.len() as u32);
    ids.iter().for_each(|i| w.id(i));
}

fn read_refs_deprec(r: &mut Rd) -> R<(i32, Vec<Ref>)> {
    let d = r.i32()?;
    Ok((d, r.array(read_ref)?))
}

fn write_refs_deprec(w: &mut Wr, d: i32, refs: &[Ref]) {
    w.i32(d);
    w.u32(refs.len() as u32);
    refs.iter().for_each(|x| write_ref(w, x));
}

impl CPlugSolid2Model {
    pub fn parse(r: &mut Rd) -> R<CPlugSolid2Model> {
        let mut m = CPlugSolid2Model::new_v34();
        m.chunks.clear();
        m.raw.clear();
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            m.chunks.push(cid);
            match cid {
                0x090BB000 => m.parse_main(r).map_err(|e| format!("CPlugSolid2Model chunk 000 at 0x{at:x}: {e}"))?,
                c if super::is_skippable_here(r) => {
                    let payload = super::read_skippable_payload(r, c)?;
                    m.raw.push(super::RawChunk { id: c, payload });
                }
                c => return Err(format!("CPlugSolid2Model chunk 0x{c:08X} at 0x{at:x} has no reader")),
            }
        }
        Ok(m)
    }

    fn parse_main(&mut self, r: &mut Rd) -> R<()> {
        let m = self;
        let v = r.u32()?;
        m.version = v;
        m.u01 = r.id()?;
        m.shaded_geoms = r.array(|r| {
            Ok(ShadedGeom {
                visual_index: r.i32()?,
                material_index: r.i32()?,
                u01: r.i32()?,
                lod_mask: if v >= 1 { r.i32()? } else { 0 },
                u02: if v >= 32 { r.i32()? } else { 0 },
            })
        })?;
        if v >= 6 {
            (m.visuals_deprec, m.visuals) = read_refs_deprec(r)?;
        }
        m.material_ids = read_ids(r)?;
        let mut material_count = if v >= 29 { r.count()? } else { 0 };
        if material_count == 0 {
            (m.materials_deprec, m.materials) = read_refs_deprec(r)?;
        }
        m.skel = read_ref(r)?;
        if v < 1 {
            return Ok(());
        }
        m.lod_max_dist = r.array(|r| r.f32())?;
        if v < 2 {
            return Ok(());
        }
        m.vis_cst_type = r.i32()?;
        if v < 3 {
            return Ok(());
        }
        if r.bool32()? {
            m.pre_light_gen = Some(read_prelight(r)?);
        }
        if v < 4 {
            return Ok(());
        }
        m.file_write_time = r.u64()?;
        if v < 5 {
            return Ok(());
        }
        m.u03 = r.string()?;
        if v < 7 {
            return Ok(());
        }
        m.materials_folder = r.string()?;
        if v >= 19 {
            m.u04 = r.string()?;
        }
        if v < 8 {
            return Ok(());
        }
        m.lights = r.array(read_light)?;
        if v < 16 {
            m.material_insts = r.array(read_ref)?;
        }
        if v < 10 {
            return Ok(());
        }
        m.light_user_models = r.array(read_ref)?;
        m.light_insts = r.array(|r| Ok((r.i32()?, r.i32()?)))?;
        if v < 11 {
            return Ok(());
        }
        m.damage_zone = r.i32()?;
        if v < 12 {
            return Ok(());
        }
        m.flags = r.u32()?;
        if v < 13 {
            return Ok(());
        }
        m.u05 = r.i32()?;
        if v < 14 {
            return Ok(());
        }
        m.u06 = r.string()?;
        if v < 15 {
            return Ok(());
        }
        if v < 29 {
            material_count = r.count()?;
        }
        if v >= 30 {
            m.u07 = r.i32()?;
        }
        for _ in 0..material_count {
            let name = r.string()?;
            let node = if name.is_empty() { Some(read_ref(r)?) } else { None };
            m.custom_materials.push(Material { name, node });
        }
        if v < 17 {
            return Ok(());
        }
        if v < 21 {
            m.boxes = r.array(|r| r.floats::<6>())?;
        }
        if v < 20 {
            return Ok(());
        }
        m.joints = read_ids(r)?;
        if v < 22 {
            return Ok(());
        }
        m.u10 = r.array(|r| r.i32())?;
        if v < 23 {
            return Ok(());
        }
        m.u11 = r.i32()?;
        if m.u11 > 0 {
            return Err(format!("CPlugSolid2Model U11 = {} (GBX.NET: throw)", m.u11));
        }
        m.u12 = r.array(|r| r.i32())?;
        if v < 24 {
            return Ok(());
        }
        m.u13 = r.i32()?;
        if v < 25 {
            return Ok(());
        }
        m.u14 = read_ref(r)?;
        m.u15 = r.f32()?;
        m.u16 = r.f32()?;
        if v < 27 {
            return Ok(());
        }
        m.u17 = r.id()?;
        if v < 31 {
            return Ok(());
        }
        m.u18 = r.i32()?;
        if m.u18 > 0 {
            return Err(format!("CPlugSolid2Model U18 = {} (GBX.NET: throw)", m.u18));
        }
        if v < 33 {
            return Ok(());
        }
        if v < 34 {
            r.i32()?;
        }
        m.u19 = r.array(|r| Ok([r.i32()?, r.i32()?, r.i32()?, r.i32()?, r.i32()?]))?;
        Ok(())
    }
}

impl CPlugSolid2Model {
    pub fn write(&self, w: &mut Wr) {
        for cid in &self.chunks {
            match *cid {
                0x090BB000 => {
                    w.u32(*cid);
                    self.write_main(w);
                }
                c => {
                    let raw = self.raw.iter().find(|x| x.id == c).expect("raw chunk listed but absent");
                    super::write_skippable(w, c, &raw.payload);
                }
            }
        }
        w.u32(FACADE);
    }

    fn write_main(&self, w: &mut Wr) {
        let m = self;
        let v = m.version;
        w.u32(v);
        w.id(&m.u01);
        w.u32(m.shaded_geoms.len() as u32);
        for g in &m.shaded_geoms {
            w.i32(g.visual_index);
            w.i32(g.material_index);
            w.i32(g.u01);
            if v >= 1 {
                w.i32(g.lod_mask);
            }
            if v >= 32 {
                w.i32(g.u02);
            }
        }
        if v >= 6 {
            write_refs_deprec(w, m.visuals_deprec, &m.visuals);
        }
        write_ids(w, &m.material_ids);
        let material_count = m.custom_materials.len();
        if v >= 29 {
            w.u32(material_count as u32);
        }
        if material_count == 0 {
            write_refs_deprec(w, m.materials_deprec, &m.materials);
        }
        write_ref(w, &m.skel);
        if v < 1 {
            return;
        }
        w.u32(m.lod_max_dist.len() as u32);
        w.floats(&m.lod_max_dist);
        if v < 2 {
            return;
        }
        w.i32(m.vis_cst_type);
        if v < 3 {
            return;
        }
        w.bool32(m.pre_light_gen.is_some());
        if let Some(p) = &m.pre_light_gen {
            write_prelight(w, p);
        }
        if v < 4 {
            return;
        }
        w.u64(m.file_write_time);
        if v < 5 {
            return;
        }
        w.string(&m.u03);
        if v < 7 {
            return;
        }
        w.string(&m.materials_folder);
        if v >= 19 {
            w.string(&m.u04);
        }
        if v < 8 {
            return;
        }
        w.u32(m.lights.len() as u32);
        m.lights.iter().for_each(|l| write_light(w, l));
        if v < 16 {
            w.u32(m.material_insts.len() as u32);
            m.material_insts.iter().for_each(|x| write_ref(w, x));
        }
        if v < 10 {
            return;
        }
        w.u32(m.light_user_models.len() as u32);
        m.light_user_models.iter().for_each(|x| write_ref(w, x));
        w.u32(m.light_insts.len() as u32);
        for (a, b) in &m.light_insts {
            w.i32(*a);
            w.i32(*b);
        }
        if v < 11 {
            return;
        }
        w.i32(m.damage_zone);
        if v < 12 {
            return;
        }
        w.u32(m.flags);
        if v < 13 {
            return;
        }
        w.i32(m.u05);
        if v < 14 {
            return;
        }
        w.string(&m.u06);
        if v < 15 {
            return;
        }
        if v < 29 {
            w.u32(material_count as u32);
        }
        if v >= 30 {
            w.i32(m.u07);
        }
        for c in &m.custom_materials {
            w.string(&c.name);
            if c.name.is_empty() {
                write_ref(w, c.node.as_ref().expect("a nameless material needs a node"));
            }
        }
        if v < 17 {
            return;
        }
        if v < 21 {
            w.u32(m.boxes.len() as u32);
            m.boxes.iter().for_each(|b| w.floats(b));
        }
        if v < 20 {
            return;
        }
        write_ids(w, &m.joints);
        if v < 22 {
            return;
        }
        w.u32(m.u10.len() as u32);
        m.u10.iter().for_each(|x| w.i32(*x));
        if v < 23 {
            return;
        }
        w.i32(m.u11);
        w.u32(m.u12.len() as u32);
        m.u12.iter().for_each(|x| w.i32(*x));
        if v < 24 {
            return;
        }
        w.i32(m.u13);
        if v < 25 {
            return;
        }
        write_ref(w, &m.u14);
        w.f32(m.u15);
        w.f32(m.u16);
        if v < 27 {
            return;
        }
        w.id(&m.u17);
        if v < 31 {
            return;
        }
        w.i32(m.u18);
        if v < 33 {
            return;
        }
        if v < 34 {
            w.i32(0);
        }
        w.u32(m.u19.len() as u32);
        for x in &m.u19 {
            x.iter().for_each(|y| w.i32(*y));
        }
    }
}
