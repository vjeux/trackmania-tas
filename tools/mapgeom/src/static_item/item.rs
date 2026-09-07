//! `CGameItemModel` (0x2E002000) body chunks, `CGameCommonItemEntityModel`
//! (0x2E027000), the `CPlugStaticObjectModel` archive (0x09159000) and
//! `CGameItemPlacementParam` (0x2E020000, all skippable chunks).

use super::{read_ref, write_ref, Id, RawChunk, Rd, Ref, Wr, R, FACADE};

/// `CPlugStaticObjectModel` is an archive class: no chunks, no FACADE.
#[derive(Clone, Debug, PartialEq)]
pub struct CPlugStaticObjectModel {
    pub version: u32,
    pub mesh: Ref,
    pub is_mesh_collidable: bool,
    /// Only when the mesh is not collidable.
    pub shape: Ref,
}

impl CPlugStaticObjectModel {
    pub fn parse(r: &mut Rd) -> R<CPlugStaticObjectModel> {
        let version = r.u32()?;
        let mesh = read_ref(r)?;
        let is_mesh_collidable = r.u8()? != 0;
        let shape = if is_mesh_collidable { super::null_ref() } else { read_ref(r)? };
        Ok(CPlugStaticObjectModel { version, mesh, is_mesh_collidable, shape })
    }
    pub fn write(&self, w: &mut Wr) {
        w.u32(self.version);
        write_ref(w, &self.mesh);
        w.u8(self.is_mesh_collidable as u8);
        if !self.is_mesh_collidable {
            write_ref(w, &self.shape);
        }
    }
    pub fn solid2(&self) -> Option<&super::solid2::CPlugSolid2Model> {
        match self.mesh.inline.as_deref()? {
            super::Node::Solid2(s) => Some(s),
            _ => None,
        }
    }
    pub fn surface(&self) -> Option<&super::surface::CPlugSurface> {
        match self.shape.inline.as_deref()? {
            super::Node::Surface(s) => Some(s),
            _ => None,
        }
    }
}

/// Chunk 0x2E027000 (TM2020 writes version 6).
#[derive(Clone, Debug, PartialEq)]
pub struct CGameCommonItemEntityModel {
    pub version: u32,
    /// v0: (PhyModel, VisModel).
    pub v0_models: Option<(Ref, Ref)>,
    /// v3: two strings.
    pub v3_strings: Option<(String, String)>,
    /// v4+
    pub static_object: Ref,
    /// v2+
    pub trigger_shape: Ref,
    pub iso: [f32; 12],
    pub particle_emitter: Ref,
    pub actions: Vec<Ref>,
    /// v2..5
    pub u_node: Ref,
    pub strings: [String; 5],
    pub iso2: [f32; 12],
    pub expr_validator: i32,
    /// v5+
    pub u_byte: u8,
}

impl CGameCommonItemEntityModel {
    pub fn parse(r: &mut Rd) -> R<CGameCommonItemEntityModel> {
        let cid = r.u32()?;
        if cid != 0x2E027000 {
            return Err(format!("CGameCommonItemEntityModel starts with chunk 0x{cid:08X}"));
        }
        let version = r.u32()?;
        let mut m = CGameCommonItemEntityModel {
            version,
            v0_models: None,
            v3_strings: None,
            static_object: super::null_ref(),
            trigger_shape: super::null_ref(),
            iso: [0.0; 12],
            particle_emitter: super::null_ref(),
            actions: Vec::new(),
            u_node: super::null_ref(),
            strings: Default::default(),
            iso2: [0.0; 12],
            expr_validator: 0,
            u_byte: 0,
        };
        if version == 0 {
            m.v0_models = Some((read_ref(r)?, read_ref(r)?));
        }
        if version == 3 {
            m.v3_strings = Some((r.string()?, r.string()?));
        }
        if version >= 4 {
            m.static_object = read_ref(r)?;
        }
        if version >= 2 {
            m.trigger_shape = read_ref(r)?;
            m.iso = r.floats::<12>()?;
            m.particle_emitter = read_ref(r)?;
            m.actions = r.array(read_ref)?;
            if version < 6 {
                m.u_node = read_ref(r)?;
            }
            for s in m.strings.iter_mut() {
                *s = r.string()?;
            }
            m.iso2 = r.floats::<12>()?;
            m.expr_validator = r.i32()?;
            if version >= 5 {
                m.u_byte = r.u8()?;
            }
        }
        let f = r.u32()?;
        if f != FACADE {
            return Err(format!("CGameCommonItemEntityModel: 0x{f:08X} after chunk 000 is not FACADE (at 0x{:x})", r.o - 4));
        }
        Ok(m)
    }

    pub fn write(&self, w: &mut Wr) {
        w.u32(0x2E027000);
        w.u32(self.version);
        if let Some((a, b)) = &self.v0_models {
            write_ref(w, a);
            write_ref(w, b);
        }
        if let Some((a, b)) = &self.v3_strings {
            w.string(a);
            w.string(b);
        }
        if self.version >= 4 {
            write_ref(w, &self.static_object);
        }
        if self.version >= 2 {
            write_ref(w, &self.trigger_shape);
            w.floats(&self.iso);
            write_ref(w, &self.particle_emitter);
            w.u32(self.actions.len() as u32);
            self.actions.iter().for_each(|a| write_ref(w, a));
            if self.version < 6 {
                write_ref(w, &self.u_node);
            }
            self.strings.iter().for_each(|s| w.string(s));
            w.floats(&self.iso2);
            w.i32(self.expr_validator);
            if self.version >= 5 {
                w.u8(self.u_byte);
            }
        }
        w.u32(FACADE);
    }

    pub fn static_object(&self) -> Option<&CPlugStaticObjectModel> {
        match self.static_object.inline.as_deref()? {
            super::Node::StaticObject(s) => Some(s),
            _ => None,
        }
    }
}

/// `CGameItemPlacementParam`: every chunk is skippable, kept raw.
#[derive(Clone, Debug, PartialEq)]
pub struct CGameItemPlacementParam {
    pub chunks: Vec<RawChunk>,
}

impl CGameItemPlacementParam {
    pub fn parse(r: &mut Rd) -> R<CGameItemPlacementParam> {
        let mut chunks = Vec::new();
        loop {
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            chunks.push(RawChunk { id: cid, payload: super::read_skippable_payload(r, cid)? });
        }
        Ok(CGameItemPlacementParam { chunks })
    }
    pub fn write(&self, w: &mut Wr) {
        for c in &self.chunks {
            super::write_skippable(w, c.id, &c.payload);
        }
        w.u32(FACADE);
    }
}

/// Chunk 0x2E002019: the entity model.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelChunk {
    pub version: u32,
    /// Read before the weapon name when the item type's own version is above
    /// the chunk version (`GetItemTypeVersion`).
    pub old_models: Option<(Ref, Ref)>,
    /// v3+
    pub default_weapon_name: Id,
    /// v4+ / v5+
    pub phy_model_custom: Ref,
    pub vis_model_custom: Ref,
    /// v6+
    pub actions: Vec<Ref>,
    /// v7+
    pub default_cam: i32,
    /// v8+; when null, `entity_model` follows.
    pub entity_model_edition: Ref,
    pub entity_model: Ref,
    /// v13+
    pub vfx: Ref,
    /// v15+
    pub material_modifier: Ref,
}

impl ModelChunk {
    pub fn entity_model(&self) -> Option<&CGameCommonItemEntityModel> {
        match self.entity_model.inline.as_deref()? {
            super::Node::EntityModel(e) => Some(e),
            _ => None,
        }
    }
}

fn item_type_version(item_type: i32) -> Option<u32> {
    match item_type {
        1 | 2 => Some(9),
        4 => Some(10),
        5 => Some(9),
        11 => None,
        _ => Some(12),
    }
}

/// One body chunk of `CGameItemModel`, in the order the file has them.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemChunk {
    Skippable(RawChunk),
    /// 0x2E001009: page name, icon (when the bool is set), u01 id.
    Collector1009 { page_name: String, icon: Option<Ref>, u01: Id },
    /// 0x2E00100B
    Ident { path: Id, collection: Id, author: Id },
    /// 0x2E00100C
    Name(String),
    /// 0x2E00100D
    Description(String),
    /// 0x2E00100E
    IconRender { auto: bool, quarter_rotation: i32 },
    /// 0x2E001010
    Skin { version: u32, default_skin: Ref, skin_directory: String, extra: Option<Ref> },
    /// 0x2E001011
    Catalog { version: u32, is_internal: bool, is_advanced: bool, position: i32, prod_state: Option<u8> },
    /// 0x2E001012
    Ints1012([i32; 4]),
    /// 0x2E002008
    NadeoSkinFids(Vec<Ref>),
    /// 0x2E002009 (deprec word, refs)
    Cameras(i32, Vec<Ref>),
    /// 0x2E00200C
    RaceInterface(Ref),
    /// 0x2E002012
    Ground { ground_point: [f32; 3], floats: [f32; 4] },
    /// 0x2E002015
    ItemType(i32),
    /// 0x2E002019
    Model(ModelChunk),
    /// 0x2E00201A
    Node201A(Ref),
    /// 0x2E00201B
    Node201B(Ref),
    /// 0x2E00201C (v5+): the placement param node.
    DefaultPlacement { version: u32, placement: Ref },
    /// 0x2E00201D
    Short201D(i16),
    /// 0x2E00201E
    Archetype { version: u32, archetype_ref: String, archetype_fid: Option<Ref>, skin_dir: Option<String>, u01: Option<i32> },
    /// 0x2E00201F (v8+ forms only)
    Waypoint { version: u32, waypoint_type: i32, disable_lightmap: bool, u_node: Option<Ref>, u_byte: Option<u8>, u_ints: Option<(i32, i32)> },
    /// 0x2E002020
    Icon { version: u32, icon_fid: String, u_byte: Option<u8> },
    /// 0x2E002023
    Bytes2023 { version: u32, u_byte: u8, u_int: i32 },
}

impl ItemChunk {
    pub fn id(&self) -> u32 {
        match self {
            ItemChunk::Skippable(c) => c.id,
            ItemChunk::Collector1009 { .. } => 0x2E001009,
            ItemChunk::Ident { .. } => 0x2E00100B,
            ItemChunk::Name(_) => 0x2E00100C,
            ItemChunk::Description(_) => 0x2E00100D,
            ItemChunk::IconRender { .. } => 0x2E00100E,
            ItemChunk::Skin { .. } => 0x2E001010,
            ItemChunk::Catalog { .. } => 0x2E001011,
            ItemChunk::Ints1012(_) => 0x2E001012,
            ItemChunk::NadeoSkinFids(_) => 0x2E002008,
            ItemChunk::Cameras(..) => 0x2E002009,
            ItemChunk::RaceInterface(_) => 0x2E00200C,
            ItemChunk::Ground { .. } => 0x2E002012,
            ItemChunk::ItemType(_) => 0x2E002015,
            ItemChunk::Model(_) => 0x2E002019,
            ItemChunk::Node201A(_) => 0x2E00201A,
            ItemChunk::Node201B(_) => 0x2E00201B,
            ItemChunk::DefaultPlacement { .. } => 0x2E00201C,
            ItemChunk::Short201D(_) => 0x2E00201D,
            ItemChunk::Archetype { .. } => 0x2E00201E,
            ItemChunk::Waypoint { .. } => 0x2E00201F,
            ItemChunk::Icon { .. } => 0x2E002020,
            ItemChunk::Bytes2023 { .. } => 0x2E002023,
        }
    }
}

/// The item body: chunks until FACADE.
#[derive(Clone, Debug, PartialEq)]
pub struct CGameItemModel {
    pub chunks: Vec<ItemChunk>,
}

impl CGameItemModel {
    pub fn model(&self) -> Option<&ModelChunk> {
        self.chunks.iter().find_map(|c| match c {
            ItemChunk::Model(m) => Some(m),
            _ => None,
        })
    }
    pub fn model_mut(&mut self) -> Option<&mut ModelChunk> {
        self.chunks.iter_mut().find_map(|c| match c {
            ItemChunk::Model(m) => Some(m),
            _ => None,
        })
    }
    /// The static object, through entity model -> static object.
    pub fn static_object(&self) -> Option<&CPlugStaticObjectModel> {
        self.model()?.entity_model()?.static_object()
    }
    /// The prefab entity model of a moving item: straight under the model
    /// chunk (the pack's layout) or inside a `CGameCommonItemEntityModel`.
    pub fn prefab(&self) -> Option<&super::prefab::CPlugPrefab> {
        let mc = self.model()?;
        match mc.entity_model.inline.as_deref()? {
            super::Node::Prefab(p) => Some(p),
            super::Node::EntityModel(e) => match e.static_object.inline.as_deref()? {
                super::Node::Prefab(p) => Some(p),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn item_type(&self) -> i32 {
        self.chunks
            .iter()
            .find_map(|c| match c {
                ItemChunk::ItemType(t) => Some(*t),
                _ => None,
            })
            .unwrap_or(0)
    }
}

impl CGameItemModel {
    pub fn parse(r: &mut Rd) -> R<CGameItemModel> {
        let mut chunks = Vec::new();
        let mut item_type = 0i32;
        loop {
            let at = r.o;
            let cid = r.u32()?;
            if cid == FACADE {
                break;
            }
            if super::is_skippable_here(r) {
                chunks.push(ItemChunk::Skippable(RawChunk { id: cid, payload: super::read_skippable_payload(r, cid)? }));
                continue;
            }
            let c = Self::parse_chunk(r, cid, item_type).map_err(|e| format!("item chunk 0x{cid:08X} at 0x{at:x}: {e}"))?;
            if let ItemChunk::ItemType(t) = &c {
                item_type = *t;
            }
            chunks.push(c);
        }
        Ok(CGameItemModel { chunks })
    }

    fn parse_chunk(r: &mut Rd, cid: u32, item_type: i32) -> R<ItemChunk> {
        Ok(match cid {
            0x2E001009 => {
                let page_name = r.string()?;
                let icon = if r.bool32()? { Some(read_ref(r)?) } else { None };
                ItemChunk::Collector1009 { page_name, icon, u01: r.id()? }
            }
            0x2E00100B => ItemChunk::Ident { path: r.id()?, collection: r.id()?, author: r.id()? },
            0x2E00100C => ItemChunk::Name(r.string()?),
            0x2E00100D => ItemChunk::Description(r.string()?),
            0x2E00100E => ItemChunk::IconRender { auto: r.bool32()?, quarter_rotation: r.i32()? },
            0x2E001010 => {
                let version = r.u32()?;
                let default_skin = read_ref(r)?;
                let skin_directory = r.string()?;
                let extra = if version >= 2 && skin_directory.is_empty() { Some(read_ref(r)?) } else { None };
                ItemChunk::Skin { version, default_skin, skin_directory, extra }
            }
            0x2E001011 => {
                let version = r.u32()?;
                let is_internal = r.bool32()?;
                let is_advanced = r.bool32()?;
                let position = r.i32()?;
                let prod_state = if version >= 1 { Some(r.u8()?) } else { None };
                ItemChunk::Catalog { version, is_internal, is_advanced, position, prod_state }
            }
            0x2E001012 => ItemChunk::Ints1012([r.i32()?, r.i32()?, r.i32()?, r.i32()?]),
            0x2E002008 => ItemChunk::NadeoSkinFids(r.array(read_ref)?),
            0x2E002009 => {
                let d = r.i32()?;
                ItemChunk::Cameras(d, r.array(read_ref)?)
            }
            0x2E00200C => ItemChunk::RaceInterface(read_ref(r)?),
            0x2E002012 => ItemChunk::Ground { ground_point: r.vec3()?, floats: r.floats::<4>()? },
            0x2E002015 => ItemChunk::ItemType(r.i32()?),
            0x2E002019 => ItemChunk::Model(Self::parse_model(r, item_type)?),
            0x2E00201A => ItemChunk::Node201A(read_ref(r)?),
            0x2E00201B => ItemChunk::Node201B(read_ref(r)?),
            0x2E00201C => {
                let version = r.u32()?;
                if version < 5 {
                    return Err(format!("chunk 0x2E00201C version {version} (inline placement) is not modelled"));
                }
                ItemChunk::DefaultPlacement { version, placement: read_ref(r)? }
            }
            0x2E00201D => ItemChunk::Short201D(r.i16()?),
            0x2E00201E => {
                let version = r.u32()?;
                let mut c = ItemChunk::Archetype { version, archetype_ref: String::new(), archetype_fid: None, skin_dir: None, u01: None };
                if let ItemChunk::Archetype { archetype_ref, archetype_fid, skin_dir, u01, .. } = &mut c {
                    if version >= 2 {
                        *archetype_ref = r.string()?;
                        if version >= 5 {
                            if archetype_ref.is_empty() {
                                *archetype_fid = Some(read_ref(r)?);
                            }
                            if version >= 6 {
                                *skin_dir = Some(r.string()?);
                                if version >= 7 {
                                    *u01 = Some(r.i32()?);
                                }
                            }
                        }
                    }
                }
                c
            }
            0x2E00201F => {
                let version = r.u32()?;
                if version < 8 {
                    return Err(format!("chunk 0x2E00201F version {version} is not modelled"));
                }
                let waypoint_type = r.i32()?;
                let disable_lightmap = r.bool32()?;
                let (mut u_node, mut u_byte, mut u_ints) = (None, None, None);
                if version >= 9 {
                    if version <= 12 {
                        u_node = Some(read_ref(r)?);
                    }
                    if version >= 11 {
                        u_byte = Some(r.u8()?);
                    }
                    if version >= 12 {
                        u_ints = Some((r.i32()?, r.i32()?));
                    }
                }
                ItemChunk::Waypoint { version, waypoint_type, disable_lightmap, u_node, u_byte, u_ints }
            }
            0x2E002020 => {
                let version = r.u32()?;
                if version < 2 {
                    return Err(format!("chunk 0x2E002020 version {version} is not modelled"));
                }
                let icon_fid = r.string()?;
                let u_byte = if version >= 3 { Some(r.u8()?) } else { None };
                ItemChunk::Icon { version, icon_fid, u_byte }
            }
            0x2E002023 => ItemChunk::Bytes2023 { version: r.u32()?, u_byte: r.u8()?, u_int: r.i32()? },
            c => return Err(format!("no reader for chunk 0x{c:08X}")),
        })
    }

    fn parse_model(r: &mut Rd, item_type: i32) -> R<ModelChunk> {
        let version = r.u32()?;
        let mut m = ModelChunk {
            version,
            old_models: None,
            default_weapon_name: Id::Null,
            phy_model_custom: super::null_ref(),
            vis_model_custom: super::null_ref(),
            actions: Vec::new(),
            default_cam: 0,
            entity_model_edition: super::null_ref(),
            entity_model: super::null_ref(),
            vfx: super::null_ref(),
            material_modifier: super::null_ref(),
        };
        if let Some(tv) = item_type_version(item_type) {
            if version < tv {
                m.old_models = Some((read_ref(r)?, read_ref(r)?));
            }
        }
        if version >= 3 {
            m.default_weapon_name = r.id()?;
        }
        if version >= 4 {
            m.phy_model_custom = read_ref(r)?;
        }
        if version >= 5 {
            m.vis_model_custom = read_ref(r)?;
        }
        if version >= 6 {
            m.actions = r.array(read_ref)?;
        }
        if version >= 7 {
            m.default_cam = r.i32()?;
        }
        if version >= 8 {
            m.entity_model_edition = read_ref(r)?;
            if m.entity_model_edition.index == -1 {
                m.entity_model = read_ref(r)?;
            }
        }
        if version >= 13 {
            m.vfx = read_ref(r)?;
        }
        if version >= 15 {
            m.material_modifier = read_ref(r)?;
        }
        Ok(m)
    }
}

impl CGameItemModel {
    pub fn write(&self, w: &mut Wr) {
        for c in &self.chunks {
            if let ItemChunk::Skippable(raw) = c {
                super::write_skippable(w, raw.id, &raw.payload);
                continue;
            }
            w.u32(c.id());
            match c {
                ItemChunk::Skippable(_) => unreachable!(),
                ItemChunk::Collector1009 { page_name, icon, u01 } => {
                    w.string(page_name);
                    w.bool32(icon.is_some());
                    if let Some(i) = icon {
                        write_ref(w, i);
                    }
                    w.id(u01);
                }
                ItemChunk::Ident { path, collection, author } => {
                    w.id(path);
                    w.id(collection);
                    w.id(author);
                }
                ItemChunk::Name(s) | ItemChunk::Description(s) => w.string(s),
                ItemChunk::IconRender { auto, quarter_rotation } => {
                    w.bool32(*auto);
                    w.i32(*quarter_rotation);
                }
                ItemChunk::Skin { version, default_skin, skin_directory, extra } => {
                    w.u32(*version);
                    write_ref(w, default_skin);
                    w.string(skin_directory);
                    if let Some(e) = extra {
                        write_ref(w, e);
                    }
                }
                ItemChunk::Catalog { version, is_internal, is_advanced, position, prod_state } => {
                    w.u32(*version);
                    w.bool32(*is_internal);
                    w.bool32(*is_advanced);
                    w.i32(*position);
                    if let Some(p) = prod_state {
                        w.u8(*p);
                    }
                }
                ItemChunk::Ints1012(v) => v.iter().for_each(|x| w.i32(*x)),
                ItemChunk::NadeoSkinFids(refs) => {
                    w.u32(refs.len() as u32);
                    refs.iter().for_each(|x| write_ref(w, x));
                }
                ItemChunk::Cameras(d, refs) => {
                    w.i32(*d);
                    w.u32(refs.len() as u32);
                    refs.iter().for_each(|x| write_ref(w, x));
                }
                ItemChunk::RaceInterface(n) | ItemChunk::Node201A(n) | ItemChunk::Node201B(n) => write_ref(w, n),
                ItemChunk::Ground { ground_point, floats } => {
                    w.floats(ground_point);
                    w.floats(floats);
                }
                ItemChunk::ItemType(t) => w.i32(*t),
                ItemChunk::Model(m) => Self::write_model(w, m),
                ItemChunk::DefaultPlacement { version, placement } => {
                    w.u32(*version);
                    write_ref(w, placement);
                }
                ItemChunk::Short201D(s) => w.i16(*s),
                ItemChunk::Archetype { version, archetype_ref, archetype_fid, skin_dir, u01 } => {
                    w.u32(*version);
                    if *version >= 2 {
                        w.string(archetype_ref);
                    }
                    if let Some(f) = archetype_fid {
                        write_ref(w, f);
                    }
                    if let Some(s) = skin_dir {
                        w.string(s);
                    }
                    if let Some(u) = u01 {
                        w.i32(*u);
                    }
                }
                ItemChunk::Waypoint { version, waypoint_type, disable_lightmap, u_node, u_byte, u_ints } => {
                    w.u32(*version);
                    w.i32(*waypoint_type);
                    w.bool32(*disable_lightmap);
                    if let Some(n) = u_node {
                        write_ref(w, n);
                    }
                    if let Some(b) = u_byte {
                        w.u8(*b);
                    }
                    if let Some((a, b)) = u_ints {
                        w.i32(*a);
                        w.i32(*b);
                    }
                }
                ItemChunk::Icon { version, icon_fid, u_byte } => {
                    w.u32(*version);
                    w.string(icon_fid);
                    if let Some(b) = u_byte {
                        w.u8(*b);
                    }
                }
                ItemChunk::Bytes2023 { version, u_byte, u_int } => {
                    w.u32(*version);
                    w.u8(*u_byte);
                    w.i32(*u_int);
                }
            }
        }
        w.u32(FACADE);
    }

    fn write_model(w: &mut Wr, m: &ModelChunk) {
        w.u32(m.version);
        if let Some((a, b)) = &m.old_models {
            write_ref(w, a);
            write_ref(w, b);
        }
        if m.version >= 3 {
            w.id(&m.default_weapon_name);
        }
        if m.version >= 4 {
            write_ref(w, &m.phy_model_custom);
        }
        if m.version >= 5 {
            write_ref(w, &m.vis_model_custom);
        }
        if m.version >= 6 {
            w.u32(m.actions.len() as u32);
            m.actions.iter().for_each(|a| write_ref(w, a));
        }
        if m.version >= 7 {
            w.i32(m.default_cam);
        }
        if m.version >= 8 {
            write_ref(w, &m.entity_model_edition);
            if m.entity_model_edition.index == -1 {
                write_ref(w, &m.entity_model);
            }
        }
        if m.version >= 13 {
            write_ref(w, &m.vfx);
        }
        if m.version >= 15 {
            write_ref(w, &m.material_modifier);
        }
    }
}
