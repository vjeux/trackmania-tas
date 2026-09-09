//! `CGameCtnBlockInfo*` — the game's description of a BLOCK MODEL: which
//! prefabs it draws per variant, which cells it occupies, and which CLIP block
//! infos it hangs on each free side of each cell.
//!
//! Layouts follow GBX.NET's `.chunkl` transcriptions (see REPORT.md for the
//! files) and were checked byte by byte against BlueBay's
//! `RoadTechStraight.EDClassic.Gbx`; where GBX.NET throws for a version the
//! comment on the chunk says what the TM2020 files actually contain.
//!
//! Two layers: the *Raw* structs are what the chunk walk accumulates (node
//! references are still node INDICES); `BlockInfo::from_graph` turns them into
//! the typed, path-resolved form everything else uses.

use crate::node::{Graph, Node, Slot};
use crate::reader::R;

pub const C_BLOCK_INFO: u32 = 0x0304E000;
pub const C_BI_CLASSIC: u32 = 0x03051000;
pub const C_BI_FLAT: u32 = 0x0304F000;
pub const C_BI_FRONTIER: u32 = 0x03050000;
pub const C_BI_TRANSITION: u32 = 0x0314C000;
pub const C_BI_CLIP: u32 = 0x03053000;
pub const C_BI_CLIP_HORIZONTAL: u32 = 0x0335B000;
pub const C_BI_CLIP_VERTICAL: u32 = 0x03340000;
pub const C_BI_PYLON: u32 = 0x03055000;
pub const C_BI_ROAD: u32 = 0x03052000;
pub const C_BI_SLOPE: u32 = 0x03054000;
pub const C_BI_RECT_ASYM: u32 = 0x03056000;
pub const C_VARIANT: u32 = 0x0315B000;
pub const C_VARIANT_GROUND: u32 = 0x0315C000;
pub const C_VARIANT_AIR: u32 = 0x0315D000;
pub const C_MOBIL: u32 = 0x03122000;
pub const C_BLOCK_UNIT: u32 = 0x03036000;
pub const C_AUTO_TERRAIN: u32 = 0x03120000;
pub const C_ZONE_GENEALOGY: u32 = 0x0311D000;
pub const C_SOLID_DECALS: u32 = 0x03121000;
pub const C_ROAD_CHUNK: u32 = 0x09128000;
pub const C_PLACEMENT_PATCH: u32 = 0x09160000;

/// Is this class a `CGameCtnBlockInfo` (any of its concrete subclasses)?
pub fn is_block_info_class(c: u32) -> bool {
    matches!(
        c,
        C_BLOCK_INFO
            | C_BI_CLASSIC
            | C_BI_FLAT
            | C_BI_FRONTIER
            | C_BI_TRANSITION
            | C_BI_CLIP
            | C_BI_CLIP_HORIZONTAL
            | C_BI_CLIP_VERTICAL
            | C_BI_PYLON
            | C_BI_ROAD
            | C_BI_SLOPE
            | C_BI_RECT_ASYM
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Classic,
    Flat,
    Frontier,
    Pillar,
    Transition,
    Clip,
    ClipHorizontal,
    ClipVertical,
    Pylon,
    Road,
    Slope,
    RectAsym,
    Other(u32),
}

impl Kind {
    pub fn of(class_id: u32, path: &str) -> Kind {
        match class_id {
            // Pillars ship as CGameCtnBlockInfoClassic under the Pillar folder.
            C_BI_CLASSIC if path.to_uppercase().contains("GAMECTNBLOCKINFOPILLAR") => Kind::Pillar,
            C_BI_CLASSIC => Kind::Classic,
            C_BI_FLAT => Kind::Flat,
            C_BI_FRONTIER => Kind::Frontier,
            C_BI_TRANSITION => Kind::Transition,
            C_BI_CLIP => Kind::Clip,
            C_BI_CLIP_HORIZONTAL => Kind::ClipHorizontal,
            C_BI_CLIP_VERTICAL => Kind::ClipVertical,
            C_BI_PYLON => Kind::Pylon,
            C_BI_ROAD => Kind::Road,
            C_BI_SLOPE => Kind::Slope,
            C_BI_RECT_ASYM => Kind::RectAsym,
            c => Kind::Other(c),
        }
    }
}

pub fn waypoint_name(t: i32) -> &'static str {
    match t {
        0 => "Start",
        1 => "Finish",
        2 => "Checkpoint",
        3 => "None",
        4 => "StartFinish",
        5 => "Dispenser",
        _ => "?",
    }
}

pub fn cardinal_name(d: i32) -> &'static str {
    match d {
        0 => "North",
        1 => "East",
        2 => "South",
        3 => "West",
        _ => "?",
    }
}

pub fn clip_type_name(t: i32) -> &'static str {
    match t {
        0 => "ClassicClip",
        1 => "FreeClipSide",
        2 => "FreeClipTop",
        3 => "FreeClipBottom",
        _ => "?",
    }
}

pub fn multi_dir_name(d: i32) -> &'static str {
    match d {
        0 => "SameDir",
        1 => "SymmetricalDirs",
        2 => "AllDir",
        3 => "OpposedDirOnly",
        4 => "PerpendicularDirsOnly",
        5 => "NextDirOnly",
        6 => "PreviousDirOnly",
        _ => "?",
    }
}

// ------------------------------------------------------------------ raw

/// `CGameCtnBlockInfo` as accumulated by the chunk walk (node indices).
#[derive(Clone, Debug, Default)]
pub struct BlockInfoRaw {
    pub no_respawn: bool,
    pub icon_auto_use_ground: bool,
    pub u017: bool,
    pub waypoint_type: Option<i32>,
    pub char_phy_special_property: i32,
    pub podium_info: i32,
    pub intro_info: i32,
    pub mat_modifier: Option<(String, String)>,
    pub variant_base_ground: i32,
    pub variant_base_air: i32,
    pub additional_ground: Vec<i32>,
    pub additional_air: Vec<i32>,
    pub symmetrical_block_info_id: String,
    pub dir: i32,
    pub fog_volume_box: i32,
    pub sounds: [i32; 2],
    pub base_type: Option<i32>,
    pub prod_state: Option<i32>,
    pub is_pillar: Option<bool>,
    pub pillar_shape_multi_dir: Option<u8>,
    pub material_modifier: [i32; 3],
    // ---- CGameCtnBlockInfoClip
    pub asym_clip_id: Option<String>,
    pub is_full_free_clip: Option<bool>,
    pub is_exclusive_free_clip: Option<bool>,
    pub clip_type: Option<i32>,
    pub can_be_deleted_by_full_free_clip: Option<bool>,
    pub top_bottom_multi_dir: Option<i32>,
    pub clip_006_bytes: Vec<u8>,
    pub passing_point: Option<([f32; 2], f32, f32)>,
    pub clip_group_id: Option<String>,
    pub symmetrical_clip_group_id: Option<String>,
    pub clip_group_ids_v1: Option<(String, String)>,
    pub horizontal_clip_group_id: Option<String>,
    pub vertical_clip_group_id: Option<String>,
    // ---- CGameCtnBlockInfoFrontier
    pub frontier_flag: Option<bool>,
    /// (chunk id, version) of every chunk walked on this node, in order.
    pub chunks: Vec<u32>,
}

/// One water volume of a block variant (chunk 0x0315B00B): the cell boxes it
/// fills (`[x0, y0, z0, x1, y1, z1]` in block units), seven words the engine
/// reads (level, flags — kept raw), and its id (the water kind).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterVolume {
    pub boxes: Vec<[i32; 6]>,
    pub words: [u32; 7],
    pub id: String,
}

#[derive(Clone, Debug, Default)]
pub struct VariantRaw {
    pub multi_dir: i32,
    pub symmetrical_variant_index: i32,
    pub cardinal_dir: i32,
    pub variant_base_type: i32,
    pub no_pillar_below_index: i32,
    pub u004: i16,
    pub mobils: Vec<Vec<i32>>,
    pub helper_solid: i32,
    pub facultative_helper_solid: i32,
    pub u005_03: i32,
    pub screen_interaction_trigger_solid: i32,
    pub waypoint_trigger_solid: i32,
    pub trigger_shapes: [i32; 2],
    pub gate: i32,
    pub teleporter: i32,
    pub capture_zone: i32,
    pub turbine: i32,
    pub flock_model: i32,
    pub spawn_model: i32,
    pub entity_spawners: Vec<i32>,
    pub probe: i32,
    pub block_units: Vec<i32>,
    pub u008_int: i32,
    pub manual_symmetry: [bool; 4],
    /// v2+: `boxaligned` — six floats (spawn translation, yaw, pitch, roll...).
    pub spawn_loc: [f32; 6],
    pub name: String,
    pub placed_pillars: Vec<(i32, [i32; 4])>,
    pub replaced_pillars: Vec<(i32, [i32; 4], u8)>,
    pub compound_model: i32,
    pub water_volumes: usize,
    pub water_volume_list: Vec<WaterVolume>,
    pub u00c: i32,
    pub u00d: [i32; 2],
    // ---- VariantGround
    pub auto_terrains: Vec<i32>,
    pub auto_terrain_height_offset: i32,
    pub auto_terrain_place_type: i32,
    pub chunks: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct BlockUnitRaw {
    pub place_pylons: i32,
    pub u000_b1: bool,
    pub u000_b2: bool,
    pub offset: [i32; 3],
    /// Chunk 0x000's flat clip list (the pre-`0x00C` form).
    pub clips_000: Vec<i32>,
    pub surface: String,
    pub frontier: i32,
    pub dir: i32,
    pub underground: bool,
    pub accept_pylons: i32,
    pub terrain_modifier_id: String,
    pub u006: [i32; 9],
    pub pylons: [i32; 4],
    pub bottom_clip: i32,
    pub top_clip: i32,
    pub bottom_clip_dir: i32,
    pub top_clip_dir: i32,
    /// North, East, South, West, Top, Bottom (chunk 0x00C).
    pub clips: [Vec<i32>; 6],
    pub u00c: [i32; 2],
    pub u00d_data: Vec<u8>,
    pub chunks: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct MobilRaw {
    pub solid_decals: Vec<i32>,
    pub u002: i32,
    pub version: u32,
    pub solid_frequency: i32,
    pub geom_translation: Option<[f32; 3]>,
    pub geom_rotation: Option<[f32; 3]>,
    pub solid_fid: i32,
    pub old_mobil: i32,
    pub prefab_fid: i32,
    pub old_solid_aggreg: i32,
    pub rail_path: i32,
    pub u04: i32,
    pub road_chunks: Vec<i32>,
    pub u08: Vec<i32>,
    pub vfxs: i32,
    pub u09: u8,
    pub u10_11: Option<f32>,
    pub u13: [f32; 3],
    pub u14: [f32; 3],
    pub u15: f32,
    pub u16: Vec<i32>,
    pub u17: i32,
    pub dyna_links: Vec<(String, i32)>,
    pub chunks: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct AutoTerrainRaw {
    pub offset: [i32; 3],
    pub genealogy: i32,
}

#[derive(Clone, Debug, Default)]
pub struct GenealogyRaw {
    pub zone_ids: Vec<String>,
    pub current_index: i32,
    pub dir: i32,
    pub current_zone_id: String,
}

#[derive(Clone, Debug, Default)]
pub struct RoadChunkRaw {
    pub version: u32,
    pub u01: i32,
    pub u02: i32,
    pub u03: Vec<[f32; 3]>,
    pub u04: Vec<[f32; 3]>,
    pub u05: Vec<[f32; 3]>,
    pub u07: Vec<[f32; 3]>,
    pub u14: String,
    pub u17: String,
    pub quat: [f32; 4],
}

/// The block-info side of `Acc`: one slot per node kind, created on the first
/// chunk of that family.
#[derive(Default)]
pub struct BiAcc {
    pub block: Option<Box<BlockInfoRaw>>,
    pub variant: Option<Box<VariantRaw>>,
    pub unit: Option<Box<BlockUnitRaw>>,
    pub mobil: Option<Box<MobilRaw>>,
    pub auto_terrain: Option<AutoTerrainRaw>,
    pub genealogy: Option<GenealogyRaw>,
    pub road_chunk: Option<Box<RoadChunkRaw>>,
}

impl BiAcc {
    pub fn finish(self, class_id: u32) -> Option<Node> {
        if let Some(b) = self.block {
            return Some(Node::BlockInfo(b));
        }
        if let Some(v) = self.variant {
            return Some(Node::Variant(v));
        }
        if let Some(u) = self.unit {
            return Some(Node::BlockUnit(u));
        }
        if let Some(m) = self.mobil {
            return Some(Node::Mobil(m));
        }
        if let Some(a) = self.auto_terrain {
            return Some(Node::AutoTerrain(a));
        }
        if let Some(g) = self.genealogy {
            return Some(Node::Genealogy(g));
        }
        if let Some(r) = self.road_chunk {
            return Some(Node::RoadChunk(r));
        }
        // A block info file whose chunks were all base-class collector
        // chunks still IS a block info.
        if is_block_info_class(class_id) {
            return Some(Node::BlockInfo(Box::default()));
        }
        None
    }
}

/// Chunk ids this module reads. `known()` in classes.rs consults it so the
/// skippable ones are walked rather than stepped over.
pub fn known(cid: u32) -> bool {
    match cid & 0xFFFF_F000 {
        C_BLOCK_INFO => matches!(
            cid & 0xFFF,
            0x009 | 0x00F | 0x013 | 0x015 | 0x017 | 0x020 | 0x023 | 0x026 | 0x027 | 0x028 | 0x029
                | 0x02A | 0x02B | 0x02C | 0x02E | 0x02F | 0x031
        ),
        C_BI_CLIP => matches!(cid & 0xFFF, 0x002 | 0x004 | 0x005 | 0x006 | 0x007 | 0x008),
        C_BI_CLIP_HORIZONTAL | C_BI_CLIP_VERTICAL => cid & 0xFFF == 0,
        C_BI_FRONTIER => cid & 0xFFF == 0,
        C_VARIANT => matches!(
            cid & 0xFFF,
            0x002 | 0x003 | 0x004 | 0x005 | 0x006 | 0x007 | 0x008 | 0x009 | 0x00A | 0x00B | 0x00C
                | 0x00D
        ),
        C_VARIANT_GROUND => cid & 0xFFF == 1,
        C_MOBIL => matches!(cid & 0xFFF, 0x002 | 0x003 | 0x004),
        C_BLOCK_UNIT => matches!(
            cid & 0xFFF,
            0x000 | 0x001 | 0x002 | 0x004 | 0x005 | 0x006 | 0x007 | 0x008 | 0x00B | 0x00C | 0x00D
        ),
        C_AUTO_TERRAIN => cid & 0xFFF == 1,
        C_ZONE_GENEALOGY => matches!(cid & 0xFFF, 0x001 | 0x002),
        C_SOLID_DECALS => matches!(cid & 0xFFF, 0x001 | 0x002 | 0x003 | 0x004),
        C_ROAD_CHUNK => cid & 0xFFF == 0,
        _ => false,
    }
}

impl<'a> Graph<'a> {
    /// `count` node references in a row.
    fn noderefs(&mut self, n: usize) -> R<Vec<i32>> {
        if n > 1_000_000 {
            return Err(format!("absurd node-ref array count {}", n));
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(self.noderef()?);
        }
        Ok(v)
    }
    /// A u32 count followed by that many node references.
    fn noderef_array(&mut self) -> R<Vec<i32>> {
        let n = self.r.u32()? as usize;
        self.noderefs(n)
    }
    /// GBX.NET's `[]_deprec`: a list-version word (always 10), then a count,
    /// then the references.
    fn noderef_array_deprec(&mut self) -> R<Vec<i32>> {
        let lv = self.r.u32()?;
        if lv != 10 {
            return Err(format!("deprecated-array list version {} (expected 10)", lv));
        }
        self.noderef_array()
    }

    /// A node written DIRECTLY — no index word, no class id, the chunks of
    /// the given class straight away, up to their FACADE. The node gets a
    /// fresh slot past the declared table so the rest of the graph can point
    /// at it like any other.
    fn direct_node(&mut self, class_id: u32) -> R<i32> {
        let n = self.node_body(class_id)?;
        self.slots.push(Slot::Node(n));
        Ok((self.slots.len() - 1) as i32)
    }

    /// One chunk of any of the block-info classes. Returns `None` when the
    /// chunk is not one this module reads.
    pub fn bi_chunk(&mut self, class_id: u32, cid: u32) -> Option<R<()>> {
        if !known(cid) {
            return None;
        }
        Some(self.bi_chunk_inner(class_id, cid))
    }

    fn bi_chunk_inner(&mut self, _class_id: u32, cid: u32) -> R<()> {
        match cid & 0xFFFF_F000 {
            C_BLOCK_INFO | C_BI_CLIP | C_BI_CLIP_HORIZONTAL | C_BI_CLIP_VERTICAL | C_BI_FRONTIER => {
                self.bi_block_chunk(cid)
            }
            C_VARIANT | C_VARIANT_GROUND => self.bi_variant_chunk(cid),
            C_MOBIL => self.bi_mobil_chunk(cid),
            C_BLOCK_UNIT => self.bi_unit_chunk(cid),
            C_AUTO_TERRAIN => {
                let mut a = AutoTerrainRaw::default();
                a.offset = [self.r.i32()?, self.r.i32()?, self.r.i32()?];
                a.genealogy = self.noderef()?;
                self.bi().auto_terrain = Some(a);
                Ok(())
            }
            C_ZONE_GENEALOGY => {
                let mut g = self.bi().genealogy.take().unwrap_or_default();
                if cid & 0xFFF == 1 {
                    g.current_index = self.r.i32()?;
                    g.dir = self.r.i32()?;
                } else {
                    g.zone_ids = self.r.array(|r| r.lookback())?;
                    g.current_index = self.r.i32()?;
                    g.dir = self.r.i32()?;
                    g.current_zone_id = self.r.lookback()?;
                }
                self.bi().genealogy = Some(g);
                Ok(())
            }
            C_SOLID_DECALS => {
                match cid & 0xFFF {
                    1 => {
                        self.r.i32()?;
                        self.r.bytes_pfx()?;
                    }
                    2 => {
                        self.r.string()?;
                    }
                    3 => {
                        self.r.lookback()?;
                        self.r.i32()?;
                    }
                    _ => {
                        self.r.i32()?;
                    }
                }
                Ok(())
            }
            C_ROAD_CHUNK => self.bi_road_chunk(),
            c => Err(format!("bi_chunk: family 0x{:08X} not handled", c)),
        }
    }

    /// The block-info accumulator of the node currently being read: the top
    /// of the stack `node_body` keeps, one entry per nested node.
    fn bi(&mut self) -> &mut BiAcc {
        self.bi_stack.last_mut().expect("bi_chunk outside a node body")
    }

    fn bi_block_chunk(&mut self, cid: u32) -> R<()> {
        let mut b = self.bi().block.take().unwrap_or_default();
        b.chunks.push(cid);
        let res = self.bi_block_chunk_into(cid, &mut b);
        self.bi().block = Some(b);
        res
    }

    fn bi_block_chunk_into(&mut self, cid: u32, b: &mut BlockInfoRaw) -> R<()> {
        match cid {
            0x0304E009 => {
                b.is_pillar = Some(self.r.bool32()?);
            }
            0x0304E00F => b.no_respawn = self.r.bool32()?,
            0x0304E013 => b.icon_auto_use_ground = self.r.bool32()?,
            0x0304E015 => {
                self.noderef()?;
                self.r.iso4()?;
            }
            0x0304E017 => b.u017 = self.r.bool32()?,
            0x0304E020 => {
                let v = self.r.u32()?;
                b.char_phy_special_property = self.noderef()?;
                if v < 6 {
                    self.noderef()?;
                }
                if v >= 2 {
                    b.podium_info = self.noderef()?;
                }
                if v >= 3 {
                    b.intro_info = self.noderef()?;
                }
                if v >= 4 {
                    self.r.bool32()?;
                }
                if v == 5 {
                    self.r.bool32()?;
                }
                if v >= 8 && self.r.bool32()? {
                    b.mat_modifier = Some((self.r.string()?, self.r.string()?));
                }
            }
            // The two base variants, written DIRECTLY (no index, no class id):
            // ground first, then air. Checked on RoadTechStraight: the payload
            // opens with chunk 0x0315B002, not a node index.
            0x0304E023 => {
                b.variant_base_ground = self.direct_node(C_VARIANT_GROUND)?;
                b.variant_base_air = self.direct_node(C_VARIANT_AIR)?;
            }
            0x0304E026 => b.waypoint_type = Some(self.r.i32()?),
            0x0304E027 => b.additional_ground = self.noderef_array_deprec()?,
            0x0304E028 => {
                b.symmetrical_block_info_id = self.r.lookback()?;
                b.dir = self.r.i32()?;
            }
            0x0304E029 => b.fog_volume_box = self.noderef()?,
            0x0304E02A => {
                let v = self.r.u32()?;
                b.sounds = [self.noderef()?, self.noderef()?];
                if v <= 2 {
                    self.r.iso4()?;
                    self.r.iso4()?;
                } else {
                    if b.sounds[0] != -1 {
                        self.r.iso4()?;
                    }
                    if b.sounds[1] != -1 {
                        self.r.iso4()?;
                    }
                }
            }
            0x0304E02B => {
                let _v = self.r.u32()?;
                // GBX.NET throws on v0; the TM2020 files carry one int after
                // the version at every version met, see REPORT.md.
                b.base_type = Some(self.r.i32()?);
            }
            0x0304E02C => b.additional_air = self.noderef_array_deprec()?,
            0x0304E02E => {
                let _v = self.r.u32()?;
                b.prod_state = Some(self.r.i32()?);
            }
            0x0304E02F => {
                let v = self.r.u32()?;
                b.is_pillar = Some(self.r.u8()? != 0);
                b.pillar_shape_multi_dir = Some(self.r.u8()?);
                if v >= 1 {
                    self.r.u8()?;
                }
            }
            0x0304E031 => {
                let v = self.r.u32()?;
                b.material_modifier[0] = self.noderef()?;
                b.material_modifier[1] = self.noderef()?;
                if v >= 1 {
                    b.material_modifier[2] = self.noderef()?;
                }
            }
            // ---- CGameCtnBlockInfoClip
            0x03053002 => b.asym_clip_id = Some(self.r.lookback()?),
            0x03053004 => {
                b.is_full_free_clip = Some(self.r.bool32()?);
                b.is_exclusive_free_clip = Some(self.r.bool32()?);
            }
            0x03053005 => b.clip_type = Some(self.r.i32()?),
            0x03053006 => {
                let v = self.r.u32()?;
                b.can_be_deleted_by_full_free_clip = Some(self.r.bool32()?);
                if v >= 1 {
                    b.top_bottom_multi_dir = Some(self.r.i32()?);
                }
                for k in 2..=4 {
                    if v >= k {
                        let x = self.r.u8()?;
                        b.clip_006_bytes.push(x);
                    }
                }
            }
            0x03053007 => {
                let _v = self.r.u32()?;
                if self.r.bool32()? {
                    b.passing_point = Some((self.r.vec2()?, self.r.f32()?, self.r.f32()?));
                }
            }
            0x03053008 => {
                let v = self.r.u32()?;
                b.clip_group_id = Some(self.r.lookback()?);
                b.symmetrical_clip_group_id = Some(self.r.lookback()?);
                if v >= 1 {
                    b.clip_group_ids_v1 = Some((self.r.lookback()?, self.r.lookback()?));
                }
            }
            0x0335B000 => {
                let _v = self.r.u32()?;
                b.horizontal_clip_group_id = Some(self.r.lookback()?);
            }
            0x03340000 => {
                let _v = self.r.u32()?;
                b.vertical_clip_group_id = Some(self.r.lookback()?);
            }
            // ---- CGameCtnBlockInfoFrontier
            0x03050000 => b.frontier_flag = Some(self.r.bool32()?),
            c => return Err(format!("block info chunk 0x{:08X} has no reader", c)),
        }
        Ok(())
    }

    fn bi_variant_chunk(&mut self, cid: u32) -> R<()> {
        let mut v = self.bi().variant.take().unwrap_or_default();
        v.chunks.push(cid);
        let res = self.bi_variant_chunk_into(cid, &mut v);
        self.bi().variant = Some(v);
        res
    }

    fn bi_variant_chunk_into(&mut self, cid: u32, v: &mut VariantRaw) -> R<()> {
        match cid {
            0x0315B002 => v.multi_dir = self.r.i32()?,
            0x0315B003 => {
                let ver = self.r.u32()?;
                v.symmetrical_variant_index = self.r.i32()?;
                if ver == 0 {
                    v.cardinal_dir = self.r.i32()?;
                } else {
                    v.cardinal_dir = self.r.u8()? as i32;
                    v.variant_base_type = self.r.u8()? as i32;
                    if ver >= 2 {
                        v.no_pillar_below_index = self.r.u8()? as i32;
                    }
                }
            }
            0x0315B004 => v.u004 = self.r.u16()? as i16,
            0x0315B005 => {
                let ver = self.r.u32()?;
                let n = self.r.u32()? as usize;
                if n > 10_000 {
                    return Err(format!("variant claims {} mobil lists", n));
                }
                for _ in 0..n {
                    let list = self.noderef_array()?;
                    v.mobils.push(list);
                }
                if ver < 2 {
                    self.noderef()?;
                    self.noderef()?;
                } else {
                    v.helper_solid = self.noderef()?;
                    v.facultative_helper_solid = self.noderef()?;
                    if ver >= 3 {
                        v.u005_03 = self.r.i32()?;
                    }
                }
            }
            0x0315B006 => {
                let ver = self.r.u32()?;
                if ver <= 8 {
                    self.noderef()?;
                }
                v.screen_interaction_trigger_solid = self.noderef()?;
                v.waypoint_trigger_solid = self.noderef()?;
                if ver >= 11 {
                    v.trigger_shapes = [self.noderef()?, self.noderef()?];
                }
                if ver <= 8 {
                    self.r.i32()?;
                }
                if ver >= 2 {
                    v.gate = self.noderef()?;
                }
                if ver >= 3 {
                    v.teleporter = self.noderef()?;
                }
                if ver >= 5 {
                    v.capture_zone = self.noderef()?;
                }
                if ver >= 6 {
                    v.turbine = self.noderef()?;
                }
                if ver >= 7 {
                    v.flock_model = self.noderef()?;
                    if v.flock_model != -1 {
                        // FlockEmitterState
                        let fv = self.r.i32()?;
                        self.r.take(4 * 3 + 4 * 2)?;
                        if fv >= 1 {
                            self.r.take(9 * 4)?;
                        }
                        self.r.vec3()?;
                    }
                }
                if ver >= 8 {
                    v.spawn_model = self.noderef()?;
                }
                if ver >= 10 {
                    v.entity_spawners = self.noderef_array()?;
                }
            }
            0x0315B007 => {
                let _ver = self.r.u32()?;
                v.probe = self.noderef()?;
            }
            0x0315B008 => {
                let ver = self.r.u32()?;
                v.block_units = self.noderef_array()?;
                v.u008_int = self.r.i32()?;
                for k in 0..4 {
                    v.manual_symmetry[k] = self.r.bool32()?;
                }
                if ver <= 1 {
                    let t = self.r.vec3()?;
                    v.spawn_loc = [t[0], t[1], t[2], self.r.f32()?, self.r.f32()?, 0.0];
                } else {
                    v.spawn_loc = self.r.boxf()?;
                }
                v.name = self.r.string()?;
            }
            0x0315B009 => {
                let ver = self.r.u32()?;
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    let node = self.noderef()?;
                    let p = [self.r.i32()?, self.r.i32()?, self.r.i32()?, self.r.i32()?];
                    v.placed_pillars.push((node, p));
                }
                if ver >= 1 {
                    let n = self.r.u32()? as usize;
                    for _ in 0..n {
                        let node = self.noderef()?;
                        let p = [self.r.i32()?, self.r.i32()?, self.r.i32()?, self.r.i32()?];
                        let u = self.r.u8()?;
                        v.replaced_pillars.push((node, p, u));
                    }
                }
            }
            0x0315B00A => {
                let ver = self.r.u32()?;
                if ver <= 1 {
                    self.noderef()?;
                    self.noderef()?;
                    if ver == 1 {
                        self.r.iso4()?;
                    }
                } else {
                    v.compound_model = self.noderef()?;
                }
            }
            // Water volumes: the ENGINE data behind a pool block's water — per
            // volume a list of cell boxes (int3 min, int3 max), seven words and
            // (v1+) a lookback id. The engine renders the water SURFACE, the
            // underwater look and the OVERFLOW of an open face (the falling
            // sheet with its three white foam lips on Summer 15's raised pools)
            // from these, never from the prefab (`Base_Air` carries one plain
            // Water quad). An ITEM has no water volume: a pool baked as an item
            // keeps the quad and loses the overflow (2026-09-08).
            0x0315B00B => {
                let ver = self.r.u32()?;
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    let boxes = self.r.array(|r| {
                        let b = r.take(24)?;
                        let w = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
                        Ok([w(0), w(4), w(8), w(12), w(16), w(20)])
                    })?;
                    let raw = self.r.take(7 * 4)?;
                    let mut words = [0u32; 7];
                    for (i, w) in words.iter_mut().enumerate() {
                        *w = u32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap());
                    }
                    let id = if ver >= 1 { self.r.lookback()? } else { String::new() };
                    v.water_volume_list.push(WaterVolume { boxes, words, id });
                }
                v.water_volumes = n;
            }
            0x0315B00C => {
                let _ver = self.r.u32()?;
                v.u00c = self.r.i32()?;
                if v.u00c > 0 {
                    return Err(format!("variant chunk 0x0315B00C U01 = {} (GBX.NET throws; layout unknown)", v.u00c));
                }
            }
            0x0315B00D => v.u00d = [self.r.i32()?, self.r.i32()?],
            0x0315C001 => {
                let _ver = self.r.u32()?;
                v.auto_terrains = self.noderef_array_deprec()?;
                v.auto_terrain_height_offset = self.r.i32()?;
                v.auto_terrain_place_type = self.r.i32()?;
            }
            c => return Err(format!("variant chunk 0x{:08X} has no reader", c)),
        }
        Ok(())
    }

    fn bi_mobil_chunk(&mut self, cid: u32) -> R<()> {
        let mut m = self.bi().mobil.take().unwrap_or_default();
        m.chunks.push(cid);
        let res = self.bi_mobil_chunk_into(cid, &mut m);
        self.bi().mobil = Some(m);
        res
    }

    fn bi_mobil_chunk_into(&mut self, cid: u32, m: &mut MobilRaw) -> R<()> {
        match cid {
            0x03122002 => {
                m.solid_decals = self.noderef_array_deprec()?;
                m.u002 = self.r.i32()?;
            }
            // Read off RoadTechStraight at version 23 and matches GBX.NET's
            // transcription exactly, including the byte-sized U09.
            0x03122003 => {
                let v = self.r.u32()?;
                m.version = v;
                if v <= 1 {
                    self.r.bool32()?;
                    if v == 0 {
                        m.old_mobil = self.noderef()?;
                    }
                }
                m.solid_frequency = self.r.i32()?;
                if v >= 1 {
                    if self.r.u8()? != 0 {
                        m.geom_translation = Some(self.r.vec3()?);
                        m.geom_rotation = Some(self.r.vec3()?);
                    }
                }
                if v >= 2 {
                    m.solid_fid = self.noderef()?;
                    if m.solid_fid == -1 {
                        m.old_mobil = self.noderef()?;
                    }
                    if v >= 14 {
                        m.prefab_fid = self.noderef()?;
                    }
                }
                if v >= 3 {
                    m.old_solid_aggreg = self.noderef()?;
                }
                if v >= 5 {
                    if v == 5 {
                        self.noderef_array()?; // CPlugPolyLine3[]
                    }
                    if v <= 10 {
                        self.r.bool32()?;
                    }
                }
                if v >= 6 {
                    m.rail_path = self.noderef()?;
                }
                if v >= 7 {
                    if v <= 22 {
                        self.noderef()?; // TrafficPath
                    }
                    if v >= 15 {
                        m.u04 = self.noderef()?;
                    }
                }
                if v >= 8 {
                    if v <= 12 {
                        self.r.i32()?;
                    }
                    if v >= 9 {
                        m.road_chunks = if v <= 22 {
                            self.noderef_array_deprec()?
                        } else {
                            self.noderef_array()?
                        };
                    }
                    if v >= 10 {
                        if v <= 12 {
                            self.r.i32()?;
                            if v == 12 {
                                self.r.i32()?;
                            }
                        }
                        if v >= 16 {
                            if v <= 22 {
                                self.noderef()?; // CitizenNetworkPath
                            }
                            if v >= 17 {
                                m.u08 = self.noderef_array()?;
                            }
                            if v >= 18 {
                                m.vfxs = self.noderef()?;
                                m.u09 = self.r.u8()?;
                                if m.u09 == 0 || m.u09 == 1 {
                                    m.u10_11 = Some(self.r.f32()?);
                                }
                                if v == 18 {
                                    self.r.iso4()?;
                                }
                                if v >= 19 {
                                    m.u13 = self.r.vec3()?;
                                    m.u14 = self.r.vec3()?;
                                }
                                m.u15 = self.r.f32()?;
                            }
                            if v >= 20 {
                                m.u16 = self.noderef_array()?;
                            }
                            if v >= 21 {
                                m.u17 = self.noderef()?;
                            }
                        }
                    }
                }
            }
            0x03122004 => {
                let _v = self.r.u32()?;
                let lv = self.r.u32()?;
                if lv != 10 {
                    return Err(format!("DynaLinks list version {} (expected 10)", lv));
                }
                let n = self.r.u32()? as usize;
                for _ in 0..n {
                    let lver = self.r.i32()?;
                    let socket = self.r.lookback()?;
                    let model = self.noderef()?;
                    if lver == 0 {
                        self.noderef()?;
                    }
                    m.dyna_links.push((socket, model));
                }
            }
            c => return Err(format!("mobil chunk 0x{:08X} has no reader", c)),
        }
        Ok(())
    }

    fn bi_unit_chunk(&mut self, cid: u32) -> R<()> {
        let mut u = self.bi().unit.take().unwrap_or_default();
        u.chunks.push(cid);
        let res = self.bi_unit_chunk_into(cid, &mut u);
        self.bi().unit = Some(u);
        res
    }

    fn bi_unit_chunk_into(&mut self, cid: u32, u: &mut BlockUnitRaw) -> R<()> {
        match cid {
            0x03036000 => {
                u.place_pylons = self.r.i32()?;
                u.u000_b1 = self.r.bool32()?;
                u.u000_b2 = self.r.bool32()?;
                u.offset = [self.r.i32()?, self.r.i32()?, self.r.i32()?];
                u.clips_000 = self.noderef_array()?;
            }
            0x03036001 => {
                u.surface = self.r.lookback()?;
                u.frontier = self.r.i32()?;
                u.dir = self.r.i32()?;
            }
            0x03036002 => u.underground = self.r.bool32()?,
            0x03036004 => u.accept_pylons = self.r.i32()?,
            0x03036005 => u.terrain_modifier_id = self.r.lookback()?,
            0x03036006 => {
                for k in 0..9 {
                    u.u006[k] = self.r.i32()?;
                }
            }
            0x03036007 => {
                for k in 0..4 {
                    u.pylons[k] = self.noderef()?;
                }
            }
            0x03036008 => {
                u.bottom_clip = self.noderef()?;
                u.top_clip = self.noderef()?;
            }
            0x0303600B => {
                let _v = self.r.u32()?;
                u.bottom_clip = self.noderef()?;
                u.top_clip = self.noderef()?;
                u.bottom_clip_dir = self.r.i32()?;
                u.top_clip_dir = self.r.i32()?;
            }
            0x0303600C => {
                let v = self.r.u32()?;
                if v == 0 {
                    return Err("block unit chunk 0x0303600C version 0 (GBX.NET throws; layout unknown)".into());
                }
                let bits = self.r.u32()?;
                for side in 0..6 {
                    let n = ((bits >> (3 * side)) & 7) as usize;
                    u.clips[side] = self.noderefs(n)?;
                }
                if v >= 2 {
                    u.u00c = [self.r.u16()? as i32, self.r.u16()? as i32];
                } else {
                    u.u00c = [self.r.i32()?, self.r.i32()?];
                }
            }
            // GBX.NET: "version, data" — a version word then a length-prefixed
            // byte blob (11 bytes on DecoCliffDirtToDecoPlatformSlopeBaseCornerIn,
            // empty on most files).
            0x0303600D => {
                let _v = self.r.u32()?;
                u.u00d_data = self.r.bytes_pfx()?.to_vec();
            }
            c => return Err(format!("block unit chunk 0x{:08X} has no reader", c)),
        }
        Ok(())
    }

    /// `CPlugRoadChunk` 0x000 (also the whole body of a
    /// `CPlugPlacementPatch`). Version 12 on RoadTechStraight.
    fn bi_road_chunk(&mut self) -> R<()> {
        let mut c = RoadChunkRaw::default();
        let v = self.r.u32()?;
        c.version = v;
        c.u01 = self.r.i32()?;
        c.u02 = self.r.i32()?;
        c.u03 = self.r.array(|r| r.vec3())?;
        c.u04 = self.r.array(|r| r.vec3())?;
        c.u05 = self.r.array(|r| r.vec3())?;
        if v >= 2 {
            self.r.i32()?;
            c.u07 = self.r.array(|r| r.vec3())?;
            if v >= 3 {
                self.r.i32()?;
                if v >= 5 {
                    self.r.take(2)?;
                    if v >= 6 {
                        self.r.take(8)?;
                        if v >= 7 {
                            self.r.u8()?;
                            if v >= 8 {
                                c.u14 = self.r.lookback()?;
                                if v >= 9 {
                                    let n = self.r.u32()? as usize;
                                    for _ in 0..n {
                                        self.r.array(|r| r.vec3())?;
                                    }
                                    if v >= 10 {
                                        self.r.u8()?;
                                        c.u17 = self.r.lookback()?;
                                        if v == 11 {
                                            self.r.vec3()?;
                                        }
                                        if v >= 12 {
                                            c.quat = self.r.quat()?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        self.bi().road_chunk = Some(Box::new(c));
        Ok(())
    }
}

// ---------------------------------------------------------------- typed

#[derive(Clone, Debug, Default)]
pub struct Mobil {
    pub prefab: Option<String>,
    pub solid: Option<String>,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 3]>,
    pub solid_frequency: i32,
    pub version: u32,
    /// Placement patches / road chunks hanging off the mobil, as their
    /// (left edge, right edge) point counts — enough to say the road runs.
    pub road_chunks: Vec<(usize, usize)>,
}

#[derive(Clone, Debug, Default)]
pub struct BlockUnit {
    pub offset: [i32; 3],
    /// North, East, South, West, Top, Bottom: logical paths of the clip
    /// block infos hung on that side.
    pub clips: [Vec<String>; 6],
    pub terrain_modifier_id: String,
    pub surface: String,
    pub frontier: i32,
    pub dir: i32,
    pub underground: bool,
    pub accept_pylons: i32,
    pub place_pylons: i32,
    pub bottom_clip: Option<String>,
    pub top_clip: Option<String>,
    /// chunk 0x0303600B: the direction words of the single bottom / top clip
    pub bottom_clip_dir: i32,
    pub top_clip_dir: i32,
    /// chunk 0x0303600C's two trailing words (v>=2: u16 each) — read as the
    /// per-clip directions of the Top and Bottom lists
    pub u00c: [i32; 2],
}

pub const SIDE_NAMES: [&str; 6] = ["North", "East", "South", "West", "Top", "Bottom"];

#[derive(Clone, Debug, Default)]
pub struct Variant {
    pub name: String,
    pub cardinal_dir: i32,
    pub symmetrical_variant_index: i32,
    pub variant_base_type: i32,
    pub no_pillar_below_index: i32,
    pub multi_dir: i32,
    pub block_units: Vec<BlockUnit>,
    pub mobils: Vec<Vec<Mobil>>,
    pub spawn_loc: [f32; 6],
    pub manual_symmetry: [bool; 4],
    pub helper_solid: Option<String>,
    pub waypoint_trigger_solid: Option<String>,
    /// The variant's waypoint trigger SHAPES (chunk 0x0315B006 v11+, two
    /// refs: `Checkpoint_Trigger.Shape.Gbx` and a second slot, usually
    /// empty): the volume the game tests the car against — for the road
    /// checkpoints a 0.1 m plane across the middle of the block, deck to
    /// ~8 m up, NOT the block's unit volume.
    pub trigger_shapes: Vec<String>,
    pub gate: Option<String>,
    pub water_volumes: usize,
    pub water_volume_list: Vec<WaterVolume>,
    pub placed_pillars: Vec<(Option<String>, [i32; 4])>,
    pub replaced_pillars: Vec<(Option<String>, [i32; 4], u8)>,
    pub auto_terrains: Vec<([i32; 3], Vec<String>, String)>,
    pub auto_terrain_height_offset: i32,
    pub auto_terrain_place_type: i32,
    pub chunks: Vec<u32>,
}

impl Variant {
    /// The block's footprint: the cells its units occupy, block-local.
    pub fn cells(&self) -> Vec<[i32; 3]> {
        self.block_units.iter().map(|u| u.offset).collect()
    }
    /// Every prefab any mobil draws, in order, without duplicates.
    pub fn prefabs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for list in &self.mobils {
            for m in list {
                if let Some(p) = &m.prefab {
                    if !out.contains(p) {
                        out.push(p.clone());
                    }
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct ClipInfo {
    pub asym_clip_id: Option<String>,
    pub is_full_free_clip: Option<bool>,
    pub is_exclusive_free_clip: Option<bool>,
    pub clip_type: Option<i32>,
    pub can_be_deleted_by_full_free_clip: Option<bool>,
    pub top_bottom_multi_dir: Option<i32>,
    pub extra_bytes: Vec<u8>,
    pub passing_point: Option<([f32; 2], f32, f32)>,
    pub clip_group_id: Option<String>,
    pub symmetrical_clip_group_id: Option<String>,
    pub clip_group_ids_v1: Option<(String, String)>,
    pub horizontal_clip_group_id: Option<String>,
    pub vertical_clip_group_id: Option<String>,
}

/// What one map placement draws, see `BlockInfo::pick_placement`.
pub struct Picked<'a> {
    pub variant: &'a Variant,
    pub label: String,
    /// the mobil list index used
    pub list: usize,
    /// the mobil(s) drawn: one, or the whole list when the subvariant was out
    /// of range
    pub mobils: Vec<&'a Mobil>,
    pub notes: Vec<String>,
}

impl<'a> Picked<'a> {
    pub fn prefabs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for m in &self.mobils {
            if let Some(p) = &m.prefab {
                if !out.contains(p) {
                    out.push(p.clone());
                }
            } else if let Some(s) = &m.solid {
                out.push(format!("(solid) {}", s));
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct BlockInfo {
    pub path: String,
    pub class_id: u32,
    pub kind: Kind,
    /// The collector's name (chunk 0x2E00100C), falling back to the file stem.
    pub name: String,
    pub waypoint_type: Option<i32>,
    pub no_respawn: bool,
    pub is_pillar: Option<bool>,
    pub pillar_shape_multi_dir: Option<u8>,
    pub symmetrical_block_info_id: String,
    pub dir: i32,
    pub base_type: Option<i32>,
    pub prod_state: Option<i32>,
    pub mat_modifier: Option<(String, String)>,
    pub material_modifier: Vec<String>,
    /// The same three refs of chunk 0x0304E031 by SLOT (None = null or inline):
    /// which slot a modifier sits in tells the mechanisms apart
    /// (`TrackWallToDecoCliff.Gbx` vs `X.TerrainModifier.Gbx`, 2026-09-08).
    pub material_modifier_slots: [Option<String>; 3],
    pub variant_base_ground: Option<Variant>,
    pub variant_base_air: Option<Variant>,
    pub additional_ground: Vec<Variant>,
    pub additional_air: Vec<Variant>,
    pub clip: Option<ClipInfo>,
    pub frontier_flag: Option<bool>,
    pub chunks: Vec<u32>,
    /// Bytes of body consumed by the walk vs the body's length; equal when
    /// the file parsed to its end.
    pub consumed: (usize, usize),
    pub recovered: Vec<String>,
    pub skipped_chunks: Vec<(u32, u32)>,
}

impl BlockInfo {
    /// The variant a placement uses: `ground` picks the ground family, and
    /// `index` 0 is the base variant, `n >= 1` the (n-1)th additional one.
    /// Falls back to the other family's base when the asked-for one is
    /// absent, and says so through the returned label.
    pub fn pick(&self, ground: bool, index: usize) -> Option<(&Variant, String)> {
        let (base, add, label) = if ground {
            (&self.variant_base_ground, &self.additional_ground, "ground")
        } else {
            (&self.variant_base_air, &self.additional_air, "air")
        };
        if index == 0 {
            if let Some(v) = base {
                return Some((v, format!("{}/base", label)));
            }
        } else if let Some(v) = add.get(index - 1) {
            return Some((v, format!("{}/add{}", label, index - 1)));
        } else if let Some(v) = base {
            return Some((v, format!("{}/base(no add{})", label, index - 1)));
        }
        let (obase, olabel) = if ground {
            (&self.variant_base_air, "air")
        } else {
            (&self.variant_base_ground, "ground")
        };
        obase.as_ref().map(|v| (v, format!("{}/base(fallback)", olabel)))
    }

    /// What a MAP PLACEMENT draws. Measured on Summer 2026 - 01 (see
    /// REPORT.md): the block flags' low six bits index the base variant's
    /// **mobil lists** (`Variant::mobils[variant]`), and the next six bits
    /// index within that list (`[variant][subvariant]`). Beach carries no
    /// additional variants but 14 mobil lists, and the map stores variant
    /// indices up to 9 with subvariant 1 — `mobils[5][1]` is `DeadendB`. The
    /// ADDITIONAL variants ("NPB", "InPillar", the 22 StructurePillar
    /// shapes) are not addressed by these bits; which one the game uses is
    /// decided at load time and is reported as an ambiguity, not guessed.
    ///
    /// Ground picks the ground base variant when it has units (some
    /// support blocks have an empty ground variant), else air, and the label
    /// says which. An index past the lists falls back to list 0 with a note.
    pub fn pick_placement(&self, ground: bool, variant: usize, subvariant: usize) -> Option<Picked<'_>> {
        self.pick_placement_add(ground, variant, subvariant, 0)
    }

    /// `additional` = the block flags' bits 21..27: 0 is the base variant,
    /// k picks additional variant k-1 of the ground (or air) list. Read off
    /// RedIsland's `DecoTerrainHD` (61 variants: base "0-WaterHill Base1" +
    /// add0..add58, the HD detail meshes matching every cliff/hill shape),
    /// whose 62 placements in Summer 2026 - 02 carry 4, 5, 6, 14..17, 34,
    /// 35, 45, 47, 59 there with the low variant bits all zero; every one of
    /// them came out as the base WaterHill piece (a black lake-bottom square
    /// in the middle of the dirt) before this was decoded. Within the chosen
    /// variant the low bits pick the mobil list and bits 6..11 the mobil.
    pub fn pick_placement_add(&self, ground: bool, variant: usize, subvariant: usize, additional: usize) -> Option<Picked<'_>> {
        // "Has content": units, or at least one mobil somewhere. StructurePillarFCBGround's
        // ground variant has no units and one EMPTY mobil list; its geometry is the air variant's.
        let has_units = |v: &Option<Variant>| v.as_ref().is_some_and(|v| !v.block_units.is_empty() || v.mobils.iter().any(|l| !l.is_empty()));
        let adds = if ground { &self.additional_ground } else { &self.additional_air };
        let has_content = |v: &Variant| !v.block_units.is_empty() || v.mobils.iter().any(|l| !l.is_empty());
        // A CLIP whose asked-for family is PRESENT but EMPTY (no units, an empty
        // mobil list) draws nothing — the engine does not fall back to the other
        // family. Measured 2026-09-09 on Summer 13: `WaterShore1_Rocky_FCLeft`
        // is recorded 52 times as an AIR clip (the row above the shore tiles)
        // and its air variant is exactly that; the original and the original
        // minus those 52 records render identically from two cameras (tinyctl
        // compare 0/144 cells), while the ground-variant fallback drew a rocky
        // skirt floating at snow level over 08/13/23. Non-clip infos keep the
        // fallback (support blocks with an empty ground variant, unmeasured).
        let is_clip = matches!(self.kind, Kind::Clip | Kind::ClipHorizontal | Kind::ClipVertical);
        let asked = if ground { &self.variant_base_ground } else { &self.variant_base_air };
        let (v, label) = if additional > 0 && additional - 1 < adds.len() && has_content(&adds[additional - 1]) {
            (&adds[additional - 1], if ground { "ground/add" } else { "air/add" })
        } else if is_clip && asked.as_ref().is_some_and(|v| !has_content(v)) {
            (asked.as_ref()?, if ground { "ground/base(empty)" } else { "air/base(empty)" })
        } else if ground && has_units(&self.variant_base_ground) {
            (self.variant_base_ground.as_ref()?, "ground/base")
        } else if !ground && has_units(&self.variant_base_air) {
            (self.variant_base_air.as_ref()?, "air/base")
        } else if has_units(&self.variant_base_ground) {
            (self.variant_base_ground.as_ref()?, "ground/base(fallback)")
        } else if has_units(&self.variant_base_air) {
            (self.variant_base_air.as_ref()?, "air/base(fallback)")
        } else {
            return None;
        };
        let mut notes = Vec::new();
        let list_idx = if variant < v.mobils.len() {
            variant
        } else {
            if !v.mobils.is_empty() {
                notes.push(format!("variant {} past {} mobil lists, list 0 used", variant, v.mobils.len()));
            }
            0
        };
        let mobils: Vec<&Mobil> = match v.mobils.get(list_idx) {
            Some(list) if subvariant < list.len() => vec![&list[subvariant]],
            Some(list) if !list.is_empty() => {
                notes.push(format!("subvariant {} past {} mobils in list {}, all listed", subvariant, list.len(), list_idx));
                list.iter().collect()
            }
            _ => Vec::new(),
        };
        let n_add = if ground { self.additional_ground.len() } else { self.additional_air.len() };
        if n_add > 0 && additional == 0 {
            notes.push(format!("{n_add} additional {} variant(s) exist; base used (flags bits 21..27 are 0)", if ground { "ground" } else { "air" }));
        }
        let label = if label.ends_with("/add") { format!("{label}{}", additional - 1) } else { label.to_string() };
        Some(Picked { variant: v, label, list: list_idx, mobils, notes })
    }

    pub fn all_variants(&self) -> Vec<(String, &Variant)> {
        let mut out = Vec::new();
        if let Some(v) = &self.variant_base_ground {
            out.push(("ground/base".to_string(), v));
        }
        for (i, v) in self.additional_ground.iter().enumerate() {
            out.push((format!("ground/add{}", i), v));
        }
        if let Some(v) = &self.variant_base_air {
            out.push(("air/base".to_string(), v));
        }
        for (i, v) in self.additional_air.iter().enumerate() {
            out.push((format!("air/add{}", i), v));
        }
        out
    }

    /// Every clip block-info path any unit of any variant names.
    pub fn clip_paths(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, v) in self.all_variants() {
            for u in &v.block_units {
                for side in &u.clips {
                    for c in side {
                        if !out.contains(c) {
                            out.push(c.clone());
                        }
                    }
                }
                for c in [&u.bottom_clip, &u.top_clip].into_iter().flatten() {
                    if !out.contains(c) {
                        out.push(c.clone());
                    }
                }
            }
        }
        out
    }

    /// Build the typed form from a parsed graph.
    pub fn from_graph(g: &Graph, path: &str, class_id: u32, body_len: usize) -> Result<BlockInfo, String> {
        let root = match &g.root {
            Some(Node::BlockInfo(b)) => b,
            Some(n) => return Err(format!("{}: root is {} not a block info", path, crate::node::node_kind_name(n))),
            None => return Err(format!("{}: no root node", path)),
        };
        let ext = |i: i32| -> Option<String> { g.external(i).map(|s| s.to_string()) };
        let variant = |i: i32| -> Option<Variant> {
            let Some(Node::Variant(v)) = g.node(i) else { return None };
            let mut out = Variant {
                name: v.name.clone(),
                cardinal_dir: v.cardinal_dir,
                symmetrical_variant_index: v.symmetrical_variant_index,
                variant_base_type: v.variant_base_type,
                no_pillar_below_index: v.no_pillar_below_index,
                multi_dir: v.multi_dir,
                spawn_loc: v.spawn_loc,
                manual_symmetry: v.manual_symmetry,
                helper_solid: ext(v.helper_solid),
                waypoint_trigger_solid: ext(v.waypoint_trigger_solid),
                trigger_shapes: v.trigger_shapes.iter().filter_map(|i| ext(*i)).collect(),
                gate: ext(v.gate),
                water_volumes: v.water_volumes,
                water_volume_list: v.water_volume_list.clone(),
                placed_pillars: v.placed_pillars.iter().map(|(n, p)| (ext(*n), *p)).collect(),
                replaced_pillars: v.replaced_pillars.iter().map(|(n, p, u)| (ext(*n), *p, *u)).collect(),
                auto_terrain_height_offset: v.auto_terrain_height_offset,
                auto_terrain_place_type: v.auto_terrain_place_type,
                chunks: v.chunks.clone(),
                ..Default::default()
            };
            for ui in &v.block_units {
                let Some(Node::BlockUnit(u)) = g.node(*ui) else { continue };
                let mut bu = BlockUnit {
                    offset: u.offset,
                    terrain_modifier_id: u.terrain_modifier_id.clone(),
                    surface: u.surface.clone(),
                    frontier: u.frontier,
                    dir: u.dir,
                    underground: u.underground,
                    accept_pylons: u.accept_pylons,
                    place_pylons: u.place_pylons,
                    bottom_clip: ext(u.bottom_clip),
                    top_clip: ext(u.top_clip),
                    bottom_clip_dir: u.bottom_clip_dir,
                    top_clip_dir: u.top_clip_dir,
                    u00c: u.u00c,
                    ..Default::default()
                };
                for side in 0..6 {
                    bu.clips[side] = u.clips[side].iter().filter_map(|c| ext(*c)).collect();
                }
                // Older files carry the flat list in chunk 0x000 only; keep
                // it visible rather than lose it, under "North" with a tag.
                if u.clips.iter().all(|c| c.is_empty()) && !u.clips_000.is_empty() {
                    bu.clips[0] = u.clips_000.iter().filter_map(|c| ext(*c)).map(|p| format!("{} (from chunk 000, side unknown)", p)).collect();
                }
                out.block_units.push(bu);
            }
            for list in &v.mobils {
                let mut ms = Vec::new();
                for mi in list {
                    let Some(Node::Mobil(m)) = g.node(*mi) else { continue };
                    let mut mob = Mobil {
                        prefab: ext(m.prefab_fid),
                        solid: ext(m.solid_fid),
                        translation: m.geom_translation,
                        rotation: m.geom_rotation,
                        solid_frequency: m.solid_frequency,
                        version: m.version,
                        road_chunks: Vec::new(),
                    };
                    for rc in m.road_chunks.iter().chain(m.u16.iter()) {
                        if let Some(Node::RoadChunk(r)) = g.node(*rc) {
                            mob.road_chunks.push((r.u04.len(), r.u05.len()));
                        }
                    }
                    ms.push(mob);
                }
                out.mobils.push(ms);
            }
            for ai in &v.auto_terrains {
                let Some(Node::AutoTerrain(a)) = g.node(*ai) else { continue };
                let (ids, cur) = match g.node(a.genealogy) {
                    Some(Node::Genealogy(z)) => (z.zone_ids.clone(), z.current_zone_id.clone()),
                    _ => (Vec::new(), String::new()),
                };
                out.auto_terrains.push((a.offset, ids, cur));
            }
            Some(out)
        };
        let clip = if matches!(Kind::of(class_id, path), Kind::Clip | Kind::ClipHorizontal | Kind::ClipVertical) {
            Some(ClipInfo {
                asym_clip_id: root.asym_clip_id.clone(),
                is_full_free_clip: root.is_full_free_clip,
                is_exclusive_free_clip: root.is_exclusive_free_clip,
                clip_type: root.clip_type,
                can_be_deleted_by_full_free_clip: root.can_be_deleted_by_full_free_clip,
                top_bottom_multi_dir: root.top_bottom_multi_dir,
                extra_bytes: root.clip_006_bytes.clone(),
                passing_point: root.passing_point,
                clip_group_id: root.clip_group_id.clone(),
                symmetrical_clip_group_id: root.symmetrical_clip_group_id.clone(),
                clip_group_ids_v1: root.clip_group_ids_v1.clone(),
                horizontal_clip_group_id: root.horizontal_clip_group_id.clone(),
                vertical_clip_group_id: root.vertical_clip_group_id.clone(),
            })
        } else {
            None
        };
        let stem = path.rsplit('\\').next().unwrap_or(path);
        let stem = stem.split('.').next().unwrap_or(stem).to_string();
        Ok(BlockInfo {
            path: path.to_string(),
            class_id,
            kind: Kind::of(class_id, path),
            name: if g.collector_name.is_empty() { stem } else { g.collector_name.clone() },
            waypoint_type: root.waypoint_type,
            no_respawn: root.no_respawn,
            is_pillar: root.is_pillar,
            pillar_shape_multi_dir: root.pillar_shape_multi_dir,
            symmetrical_block_info_id: root.symmetrical_block_info_id.clone(),
            dir: root.dir,
            base_type: root.base_type,
            prod_state: root.prod_state,
            mat_modifier: root.mat_modifier.clone(),
            material_modifier: root.material_modifier.iter().filter_map(|i| ext(*i)).collect(),
            material_modifier_slots: [ext(root.material_modifier[0]), ext(root.material_modifier[1]), ext(root.material_modifier[2])],
            variant_base_ground: variant(root.variant_base_ground),
            variant_base_air: variant(root.variant_base_air),
            additional_ground: root.additional_ground.iter().filter_map(|i| variant(*i)).collect(),
            additional_air: root.additional_air.iter().filter_map(|i| variant(*i)).collect(),
            clip,
            frontier_flag: root.frontier_flag,
            chunks: root.chunks.clone(),
            consumed: (g.r.o, body_len),
            recovered: g.recovered.clone(),
            skipped_chunks: g.skipped.clone(),
        })
    }

    pub fn parsed_to_end(&self) -> bool {
        self.consumed.0 == self.consumed.1 && self.recovered.is_empty()
    }

    /// Everything, as text.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let p = |s: &mut String, line: String| {
            s.push_str(&line);
            s.push('\n');
        };
        p(&mut s, format!("{}", self.path));
        p(&mut s, format!("  class 0x{:08X}  kind {:?}  name {}", self.class_id, self.kind, self.name));
        p(&mut s, format!(
            "  waypoint {}  no_respawn {}  is_pillar {:?}  pillar_multidir {:?}  sym_id {:?} dir {}  base_type {:?}  prod_state {:?}",
            self.waypoint_type.map(waypoint_name).unwrap_or("-"),
            self.no_respawn, self.is_pillar, self.pillar_shape_multi_dir, self.symmetrical_block_info_id, self.dir, self.base_type, self.prod_state
        ));
        if let Some(m) = &self.mat_modifier {
            p(&mut s, format!("  mat modifier {:?}", m));
        }
        if !self.material_modifier.is_empty() {
            p(&mut s, format!("  material modifier refs {:?}", self.material_modifier));
            p(&mut s, format!("  material modifier slots {:?}", self.material_modifier_slots));
        }
        if let Some(f) = self.frontier_flag {
            p(&mut s, format!("  frontier flag {}", f));
        }
        if let Some(c) = &self.clip {
            p(&mut s, format!(
                "  CLIP type {} ({:?})  asym_id {:?}  full_free {:?} exclusive_free {:?}  deletable_by_full_free {:?}  top_bottom_multidir {:?} extra {:?}",
                c.clip_type.map(clip_type_name).unwrap_or("-"), c.clip_type, c.asym_clip_id, c.is_full_free_clip, c.is_exclusive_free_clip,
                c.can_be_deleted_by_full_free_clip, c.top_bottom_multi_dir.map(multi_dir_name), c.extra_bytes
            ));
            p(&mut s, format!(
                "       group {:?} sym_group {:?} v1 {:?} horizontal {:?} vertical {:?} passing_point {:?}",
                c.clip_group_id, c.symmetrical_clip_group_id, c.clip_group_ids_v1, c.horizontal_clip_group_id, c.vertical_clip_group_id, c.passing_point
            ));
        }
        p(&mut s, format!(
            "  chunks {}",
            self.chunks.iter().map(|c| format!("{:08X}", c)).collect::<Vec<_>>().join(" ")
        ));
        for (label, v) in self.all_variants() {
            p(&mut s, format!(
                "  VARIANT {}  name {:?}  cardinal {} sym_index {} base_type {} no_pillar_below {} multidir {}  units {}  mobil lists {}",
                label, v.name, cardinal_name(v.cardinal_dir), v.symmetrical_variant_index, v.variant_base_type,
                v.no_pillar_below_index, multi_dir_name(v.multi_dir), v.block_units.len(), v.mobils.len()
            ));
            p(&mut s, format!("    spawn {:?}  manual symmetry {:?}", v.spawn_loc, v.manual_symmetry));
            if v.helper_solid.is_some() || v.waypoint_trigger_solid.is_some() || v.gate.is_some() || !v.trigger_shapes.is_empty() {
                p(&mut s, format!("    helper solid {:?}  waypoint trigger {:?}  trigger shapes {:?}  gate {:?}", v.helper_solid, v.waypoint_trigger_solid, v.trigger_shapes, v.gate));
            }
            if v.water_volumes > 0 {
                p(&mut s, format!("    water volumes {}", v.water_volumes));
                for wv in &v.water_volume_list {
                    let f = |w: u32| f32::from_bits(w);
                    p(&mut s, format!("      volume id {:?} boxes {:?} words {:?} (as f32 {:?})", wv.id, wv.boxes, wv.words, wv.words.iter().map(|w| f(*w)).collect::<Vec<_>>()));
                }
            }
            for (n, prm) in &v.placed_pillars {
                p(&mut s, format!("    placed pillar {:?} {:?}", n, prm));
            }
            for (n, prm, u) in &v.replaced_pillars {
                p(&mut s, format!("    replaced pillar {:?} {:?} {}", n, prm, u));
            }
            for (i, list) in v.mobils.iter().enumerate() {
                for (j, m) in list.iter().enumerate() {
                    p(&mut s, format!(
                        "    mobil[{}][{}] v{}  prefab {}  solid {}  freq {}  translation {:?} rotation {:?}  road chunks {:?}",
                        i, j, m.version,
                        m.prefab.as_deref().unwrap_or("-"), m.solid.as_deref().unwrap_or("-"), m.solid_frequency,
                        m.translation, m.rotation, m.road_chunks
                    ));
                }
            }
            for (i, u) in v.block_units.iter().enumerate() {
                p(&mut s, format!(
                    "    unit[{}] offset {:?}  terrain_modifier {:?} surface {:?} frontier {} dir {} underground {} pylons place {} accept {}  clipdirs bottom {} top {} 00c {:?}",
                    i, u.offset, u.terrain_modifier_id, u.surface, u.frontier, u.dir, u.underground, u.place_pylons, u.accept_pylons, u.bottom_clip_dir, u.top_clip_dir, u.u00c
                ));
                for side in 0..6 {
                    if !u.clips[side].is_empty() {
                        p(&mut s, format!("      {:<6} {}", SIDE_NAMES[side], u.clips[side].join(" | ")));
                    }
                }
                if let Some(c) = &u.bottom_clip {
                    p(&mut s, format!("      bottom clip (00B) {}", c));
                }
                if let Some(c) = &u.top_clip {
                    p(&mut s, format!("      top clip (00B) {}", c));
                }
            }
            for (off, ids, cur) in &v.auto_terrains {
                p(&mut s, format!("    auto terrain {:?} zones {:?} current {:?}", off, ids, cur));
            }
            if !v.auto_terrains.is_empty() {
                p(&mut s, format!("    auto terrain height offset {} place type {}", v.auto_terrain_height_offset, v.auto_terrain_place_type));
            }
        }
        p(&mut s, format!(
            "  parsed {} of {} body bytes{}{}",
            self.consumed.0,
            self.consumed.1,
            if self.parsed_to_end() { " (to the end)" } else { "  *** NOT TO THE END ***" },
            if self.recovered.is_empty() { String::new() } else { format!("  recovered: {:?}", self.recovered) }
        ));
        if !self.skipped_chunks.is_empty() {
            p(&mut s, format!(
                "  skipped (unknown skippable) chunks: {}",
                self.skipped_chunks.iter().map(|(c, n)| format!("{:08X}({} B)", c, n)).collect::<Vec<_>>().join(" ")
            ));
        }
        s
    }
}

/// Load and fully type one block info file from the store.
pub fn load(store: &mut crate::store::DataStore, logical: &str) -> Result<BlockInfo, String> {
    let m = store.load_model(logical)?;
    let g = m.graph()?;
    BlockInfo::from_graph(&g, &m.path, m.class_id, m.body.len())
}

#[cfg(test)]
mod pick_tests {
    use super::*;

    fn variant(units: usize, mobils: Vec<Vec<Option<&str>>>) -> Variant {
        Variant {
            name: String::new(),
            cardinal_dir: 0,
            symmetrical_variant_index: -1,
            variant_base_type: 0,
            no_pillar_below_index: 255,
            multi_dir: 0,
            block_units: (0..units).map(|_| BlockUnit::default()).collect(),
            mobils: mobils.into_iter().map(|l| l.into_iter().map(|p| Mobil { prefab: p.map(String::from), ..Mobil::default() }).collect()).collect(),
            spawn_loc: [0.0; 6],
            manual_symmetry: [false; 4],
            helper_solid: None,
            waypoint_trigger_solid: None,
            trigger_shapes: Vec::new(),
            gate: None,
            water_volumes: 0,
            water_volume_list: Vec::new(),
            placed_pillars: Vec::new(),
            replaced_pillars: Vec::new(),
            auto_terrains: Vec::new(),
            auto_terrain_height_offset: 0,
            auto_terrain_place_type: 0,
            chunks: Vec::new(),
        }
    }

    fn info(kind: Kind, ground: Option<Variant>, air: Option<Variant>) -> BlockInfo {
        BlockInfo {
            path: String::new(),
            class_id: 0,
            kind,
            name: String::new(),
            waypoint_type: None,
            no_respawn: false,
            is_pillar: None,
            pillar_shape_multi_dir: None,
            symmetrical_block_info_id: String::new(),
            dir: 0,
            base_type: None,
            prod_state: None,
            mat_modifier: None,
            material_modifier: Vec::new(),
            material_modifier_slots: [None, None, None],
            variant_base_ground: ground,
            variant_base_air: air,
            additional_ground: Vec::new(),
            additional_air: Vec::new(),
            clip: None,
            frontier_flag: None,
            chunks: Vec::new(),
            consumed: (0, 0),
            recovered: Vec::new(),
            skipped_chunks: Vec::new(),
        }
    }

    /// WaterShore1_Rocky_FCLeft: ground variant = one FCLeft prefab, air
    /// variant present with no units and one empty mobil list. An AIR record
    /// draws nothing (Summer 13, measured); a GROUND record draws FCLeft.
    #[test]
    fn a_clip_with_an_empty_asked_for_variant_draws_nothing() {
        let bi = info(Kind::Clip, Some(variant(1, vec![vec![Some("FCLeft.Prefab.Gbx")]])), Some(variant(0, vec![vec![]])));
        let air = bi.pick_placement_add(false, 0, 0, 0).expect("a pick");
        assert_eq!(air.label, "air/base(empty)");
        assert!(air.mobils.is_empty() && air.prefabs().is_empty());
        let ground = bi.pick_placement_add(true, 0, 0, 0).expect("a pick");
        assert_eq!(ground.label, "ground/base");
        assert_eq!(ground.prefabs(), vec!["FCLeft.Prefab.Gbx".to_string()]);
    }

    /// A clip whose asked-for family is ABSENT still takes the other one
    /// (TrackWallWaterStraightFCBInsideV2: a ground variant with no units, an
    /// air variant with the floor — a ground record is not the empty case
    /// when the ground variant is missing altogether).
    #[test]
    fn an_absent_family_falls_back() {
        let bi = info(Kind::Clip, None, Some(variant(1, vec![vec![Some("Straight_FCBInside.Prefab.Gbx")]])));
        let p = bi.pick_placement_add(true, 0, 0, 0).expect("a pick");
        assert_eq!(p.label, "air/base(fallback)");
        assert_eq!(p.prefabs(), vec!["Straight_FCBInside.Prefab.Gbx".to_string()]);
    }

    /// A non-clip keeps the old behaviour: an empty ground variant is skipped
    /// for the air one (the support blocks of 2026-09-06, unmeasured).
    #[test]
    fn a_classic_with_an_empty_ground_variant_still_falls_back() {
        let bi = info(Kind::Classic, Some(variant(0, vec![vec![]])), Some(variant(1, vec![vec![Some("Base_Air.Prefab.Gbx")]])));
        let p = bi.pick_placement_add(true, 0, 0, 0).expect("a pick");
        assert_eq!(p.label, "air/base(fallback)");
    }
}
