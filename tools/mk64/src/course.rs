//! A Mario Kart 64 course as the decomp spells it: the packed vertex array,
//! the display lists, the `TrackSections` collision table, the centre path,
//! the actor spawns — and an interpreter that walks the display lists like
//! the RSP would, collecting textured triangles per display-list piece.

use crate::cdata::{self, Consts, Node};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The 20 courses of the ROM in `gCourseTable` order (the game's course ids);
/// `dir` is the decomp's `courses/<dir>` folder. The 16 race courses carry
/// their official lap length in metres (Nintendo's course guide numbers, used
/// to calibrate the world scale); the 4 battle courses have none.
pub const COURSES: &[(&str, &str, Option<f32>)] = &[
    ("mario_raceway", "Mario Raceway", Some(567.0)),
    ("choco_mountain", "Choco Mountain", Some(687.0)),
    ("bowsers_castle", "Bowser's Castle", Some(777.0)),
    ("banshee_boardwalk", "Banshee Boardwalk", Some(747.0)),
    ("yoshi_valley", "Yoshi Valley", Some(772.0)),
    ("frappe_snowland", "Frappe Snowland", Some(734.0)),
    ("koopa_troopa_beach", "Koopa Troopa Beach", Some(691.0)),
    ("royal_raceway", "Royal Raceway", Some(1025.0)),
    ("luigi_raceway", "Luigi Raceway", Some(717.0)),
    ("moo_moo_farm", "Moo Moo Farm", Some(527.0)),
    ("toads_turnpike", "Toad's Turnpike", Some(1036.0)),
    ("kalimari_desert", "Kalimari Desert", Some(753.0)),
    ("sherbet_land", "Sherbet Land", Some(756.0)),
    ("rainbow_road", "Rainbow Road", Some(2000.0)),
    ("wario_stadium", "Wario Stadium", Some(1591.0)),
    ("block_fort", "Block Fort", None),
    ("skyscraper", "Skyscraper", None),
    ("double_deck", "Double Deck", None),
    ("dks_jungle_parkway", "D.K.'s Jungle Parkway", Some(893.0)),
    ("big_donut", "Big Donut", None),
];

/// The game's `SURFACE_TYPE` enum.
pub const SURFACES: &[(&str, u8)] = &[
    ("AIRBORNE", 0),
    ("ASPHALT", 1),
    ("DIRT", 2),
    ("SAND", 3),
    ("STONE", 4),
    ("SNOW", 5),
    ("BRIDGE", 6),
    ("SAND_OFFROAD", 7),
    ("GRASS", 8),
    ("ICE", 9),
    ("WET_SAND", 10),
    ("SNOW_OFFROAD", 11),
    ("CLIFF", 12),
    ("DIRT_OFFROAD", 13),
    ("TRAIN_TRACK", 14),
    ("CAVE", 15),
    ("ROPE_BRIDGE", 16),
    ("WOOD_BRIDGE", 17),
    ("BOOST_RAMP_WOOD", 0xFC),
    ("OUT_OF_BOUNDS", 0xFD),
    ("BOOST_RAMP_ASPHALT", 0xFE),
    ("RAMP", 0xFF),
];

pub fn surface_name(id: u8) -> &'static str {
    SURFACES.iter().find(|(_, v)| *v == id).map(|(n, _)| *n).unwrap_or("?")
}

/// One packed course vertex (`CourseVtx`): position and texture coordinates
/// in the course's own units, the vertex colour with its two flag bits
/// already masked off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub pos: [i16; 3],
    pub tc: [i16; 2],
    pub rgb: [u8; 3],
    pub flag: u8,
}

/// The display-list commands the courses use.
#[derive(Clone, Debug, PartialEq)]
pub enum Gfx {
    /// `gsSPVertex(0x04000000 + 16·index, n, v0)`.
    Vertex { index: usize, n: usize, v0: usize },
    /// A vertex load from another segment (an object model) — not course geometry.
    ForeignVertex { addr: u32, n: usize, v0: usize },
    Tri([usize; 3]),
    Call(String),
    End,
    TexImage { fmt: u8, siz: u8, width: u32, sym: String },
    SetTile { fmt: u8, siz: u8, tile: u8, cmt: u8, maskt: u8, cms: u8, masks: u8 },
    TileSize { tile: u8, uls: u32, ult: u32, lrs: u32, lrt: u32 },
    Texture { s: u32, t: u32, on: bool },
    GeomSet(u32),
    GeomClear(u32),
    /// Anything else, with its macro name (sync, load block, combine, render mode...).
    Other(String),
}

/// A `TrackSections` row: the collision piece and its surface.
#[derive(Clone, Debug)]
pub struct Section {
    pub dl: String,
    pub surface: u8,
    pub section_id: u8,
    pub flags: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct PathPoint {
    pub pos: [i16; 3],
    pub section_id: u16,
}

#[derive(Clone, Debug)]
pub struct Spawn {
    pub pos: [i16; 3],
    pub id: i16,
}

/// The texture state a triangle was drawn with (the render tile).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexState {
    /// Index into `Course::tex_syms`.
    pub sym: u16,
    pub fmt: u8,
    pub siz: u8,
    /// Tile size from `gsDPSetTileSize` (texels) — 0 when never set.
    pub w: u16,
    pub h: u16,
    pub uls: u16,
    pub ult: u16,
    /// Wrap modes and masks of the render tile (bit 0 mirror, bit 1 clamp).
    pub cms: u8,
    pub cmt: u8,
    pub masks: u8,
    pub maskt: u8,
    /// `gsSPTexture` scale (1/65536 units).
    pub scale_s: u16,
    pub scale_t: u16,
}

/// A triangle as drawn: vertex indices into `Course::vertices`, the texture
/// state (None = untextured), the geometry mode at the draw.
#[derive(Clone, Copy, Debug)]
pub struct DrawnTri {
    pub v: [u32; 3],
    pub tex: Option<u16>,
    pub geom: u32,
}

/// Triangles emitted by one display list (the innermost list holding the
/// triangle commands), in draw order.
#[derive(Clone, Debug, Default)]
pub struct Piece {
    pub dl: String,
    pub tris: Vec<DrawnTri>,
}

#[derive(Debug, Default)]
pub struct Course {
    pub dir: String,
    pub vertices: Vec<Vertex>,
    pub dls: HashMap<String, Vec<Gfx>>,
    /// The render lists (`d_course_<x>_dl_*`): what the game draws from the
    /// sections of the track, each a list of packed pieces.
    pub render_lists: Vec<String>,
    pub sections: Vec<Section>,
    pub path: Vec<PathPoint>,
    pub item_boxes: Vec<Spawn>,
    pub spawns: Vec<(String, Vec<Spawn>)>,
    pub tex_syms: Vec<String>,
    pub tex_states: Vec<TexState>,
    tex_state_index: HashMap<TexState, u16>,
    pub notes: Vec<String>,
}

pub fn geom_mode_mask() -> Consts {
    cdata::gbi_consts()
}

impl Course {
    /// Load `courses/<dir>` of a decomp checkout.
    pub fn load(decomp: &Path, dir: &str) -> Result<Course, String> {
        let root = decomp.join("courses").join(dir);
        if !root.is_dir() {
            return Err(format!("no course folder {}", root.display()));
        }
        let consts = cdata::gbi_consts();
        let mut c = Course { dir: dir.to_string(), ..Default::default() };
        let read = |name: &str| -> Result<String, String> {
            let p: PathBuf = root.join(name);
            std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
        };
        // vertices
        let vsrc = cdata::strip(&read("course_vertices.inc.c")?);
        for arr in cdata::arrays(&vsrc) {
            if arr.ty != "CourseVtx" {
                continue;
            }
            for it in &arr.items {
                if let Some(v) = parse_vertex(it, &consts) {
                    c.vertices.push(v);
                } else {
                    c.notes.push(format!("unparsed vertex {it:?}"));
                }
            }
        }
        // display lists: the packed pieces and the course_data lists
        for file in ["course_displaylists.inc.c", "course_data.c"] {
            let src = cdata::strip(&read(file)?);
            let arrays = cdata::arrays(&src);
            for arr in &arrays {
                match arr.ty.as_str() {
                    "Gfx" => {
                        let cmds = parse_gfx(&arr.items, &consts, &mut c.notes);
                        if file == "course_data.c" && cmds.iter().any(|g| matches!(g, Gfx::Call(n) if n.contains("_packed_dl_"))) {
                            c.render_lists.push(arr.name.clone());
                        }
                        c.dls.insert(arr.name.clone(), cmds);
                    }
                    "TrackSections" => {
                        for it in &arr.items {
                            if let Some(row) = it.list() {
                                let dl = match row.first().and_then(|n| n.ident()) {
                                    Some(d) => d.to_string(),
                                    None => continue, // the terminating { 0, ... } row
                                };
                                let surface = row.get(1).and_then(|n| surface_id(n)).unwrap_or(0);
                                let section_id = row.get(2).and_then(|n| n.int(&consts)).unwrap_or(0) as u8;
                                let flags = row.get(3).and_then(|n| n.int(&consts)).unwrap_or(0) as u16;
                                c.sections.push(Section { dl, surface, section_id, flags });
                            }
                        }
                    }
                    "TrackPathPoint" if arr.name.ends_with("_track_path") => {
                        for it in &arr.items {
                            if let Some(row) = it.list() {
                                let n: Vec<i64> = row.iter().filter_map(|x| x.int(&consts)).collect();
                                if n.len() == 4 && n[0] != -32768 {
                                    c.path.push(PathPoint { pos: [n[0] as i16, n[1] as i16, n[2] as i16], section_id: n[3] as u16 });
                                }
                            }
                        }
                    }
                    "struct ActorSpawnData" => {
                        let mut v = Vec::new();
                        for it in &arr.items {
                            if let Some(row) = it.list() {
                                // rows are `{ { x, y, z }, { id } }`: flatten one level
                                let n: Vec<i64> = row
                                    .iter()
                                    .flat_map(|x| match x.list() {
                                        Some(inner) => inner.iter().filter_map(|y| y.int(&consts)).collect::<Vec<_>>(),
                                        None => x.int(&consts).into_iter().collect(),
                                    })
                                    .collect();
                                if n.len() >= 4 && n[0] != -32768 {
                                    v.push(Spawn { pos: [n[0] as i16, n[1] as i16, n[2] as i16], id: n[3] as i16 });
                                }
                            }
                        }
                        if arr.name.ends_with("item_box_spawns") {
                            c.item_boxes = v.clone();
                        }
                        c.spawns.push((arr.name.clone(), v));
                    }
                    _ => {}
                }
            }
        }
        if c.vertices.is_empty() {
            return Err("no vertices parsed".into());
        }
        if c.render_lists.is_empty() {
            return Err("no render lists found in course_data.c".into());
        }
        Ok(c)
    }

    fn tex_sym(&mut self, sym: &str) -> u16 {
        if let Some(i) = self.tex_syms.iter().position(|s| s == sym) {
            return i as u16;
        }
        self.tex_syms.push(sym.to_string());
        (self.tex_syms.len() - 1) as u16
    }

    fn intern_state(&mut self, st: TexState) -> u16 {
        if let Some(&i) = self.tex_state_index.get(&st) {
            return i;
        }
        let i = self.tex_states.len() as u16;
        self.tex_states.push(st);
        self.tex_state_index.insert(st, i);
        i
    }

    /// Walk every render list in order with one persistent RDP state (the
    /// lists are drawn in sequence by the game; texture state carries over
    /// between pieces) and return the pieces in first-draw order, each piece
    /// once — the union of what the course ever draws.
    pub fn visual_pieces(&mut self) -> Vec<Piece> {
        let lists = self.render_lists.clone();
        let mut seen: HashSet<String> = HashSet::new();
        let mut pieces: Vec<Piece> = Vec::new();
        let mut st = RspState::default();
        for name in lists {
            self.walk(&name, &mut st, &mut seen, &mut pieces, 0);
        }
        pieces
    }

    /// The collision pieces: every `TrackSections` row walked on its own,
    /// with the row's surface. Pieces repeat if the table lists them twice.
    pub fn collision_pieces(&mut self) -> Vec<(Section, Piece)> {
        let sections = self.sections.clone();
        let mut out = Vec::new();
        for s in sections {
            let mut st = RspState::default();
            let mut seen = HashSet::new();
            let mut pieces = Vec::new();
            self.walk(&s.dl, &mut st, &mut seen, &mut pieces, 0);
            let mut merged = Piece { dl: s.dl.clone(), tris: Vec::new() };
            for p in pieces {
                merged.tris.extend(p.tris);
            }
            out.push((s, merged));
        }
        out
    }

    fn walk(&mut self, name: &str, st: &mut RspState, seen: &mut HashSet<String>, out: &mut Vec<Piece>, depth: usize) {
        if depth > 16 {
            self.notes.push(format!("display list nesting too deep at {name}"));
            return;
        }
        let cmds = match self.dls.get(name) {
            Some(c) => c.clone(),
            None => {
                self.notes.push(format!("unknown display list {name}"));
                return;
            }
        };
        let first_time = seen.insert(name.to_string());
        let mut piece = Piece { dl: name.to_string(), tris: Vec::new() };
        for cmd in &cmds {
            match cmd {
                Gfx::Vertex { index, n, v0 } => {
                    for k in 0..*n {
                        let slot = v0 + k;
                        if slot < st.slots.len() {
                            st.slots[slot] = Some((index + k) as u32);
                        }
                    }
                }
                Gfx::ForeignVertex { addr, n, v0 } => {
                    self.notes.push(format!("{name}: vertex load from segment {:#x} ({n} at slot {v0}) skipped", addr >> 24));
                    for k in 0..*n {
                        if v0 + k < st.slots.len() {
                            st.slots[v0 + k] = None;
                        }
                    }
                }
                Gfx::Tri(s) => {
                    let v = [st.slots[s[0]], st.slots[s[1]], st.slots[s[2]]];
                    if let [Some(a), Some(b), Some(c)] = v {
                        let tex = if st.texture_on && st.tex_sym.is_some() { Some(self.intern_state(st.render_state())) } else { None };
                        piece.tris.push(DrawnTri { v: [a, b, c], tex, geom: st.geom });
                    }
                }
                Gfx::Call(callee) => {
                    if !piece.tris.is_empty() {
                        if first_time {
                            out.push(std::mem::take(&mut piece));
                        }
                        piece = Piece { dl: name.to_string(), tris: Vec::new() };
                    }
                    self.walk(callee, st, seen, out, depth + 1);
                }
                Gfx::End => break,
                Gfx::TexImage { fmt, siz, width, sym } => {
                    st.tex_sym = Some(self.tex_sym(sym));
                    st.tex_fmt = *fmt;
                    st.tex_siz = *siz;
                    st.tex_width = *width;
                }
                Gfx::SetTile { fmt, siz, tile, cmt, maskt, cms, masks } => {
                    let t = &mut st.tiles[(*tile & 7) as usize];
                    t.fmt = *fmt;
                    t.siz = *siz;
                    t.cmt = *cmt;
                    t.maskt = *maskt;
                    t.cms = *cms;
                    t.masks = *masks;
                }
                Gfx::TileSize { tile, uls, ult, lrs, lrt } => {
                    let t = &mut st.tiles[(*tile & 7) as usize];
                    t.uls = *uls;
                    t.ult = *ult;
                    t.lrs = *lrs;
                    t.lrt = *lrt;
                }
                Gfx::Texture { s, t, on } => {
                    st.scale_s = *s;
                    st.scale_t = *t;
                    st.texture_on = *on;
                }
                Gfx::GeomSet(m) => st.geom |= m,
                Gfx::GeomClear(m) => st.geom &= !m,
                Gfx::Other(_) => {}
            }
        }
        if !piece.tris.is_empty() && first_time {
            out.push(piece);
        }
    }

    /// Length of the centre path in course units (closed loop).
    pub fn path_length(&self) -> f64 {
        let n = self.path.len();
        if n < 2 {
            return 0.0;
        }
        let mut len = 0.0;
        for i in 0..n {
            let a = self.path[i].pos;
            let b = self.path[(i + 1) % n].pos;
            let d = [(b[0] - a[0]) as f64, (b[1] - a[1]) as f64, (b[2] - a[2]) as f64];
            len += (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        }
        len
    }

    pub fn bbox(&self) -> ([i16; 3], [i16; 3]) {
        let mut lo = [i16::MAX; 3];
        let mut hi = [i16::MIN; 3];
        for v in &self.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v.pos[k]);
                hi[k] = hi[k].max(v.pos[k]);
            }
        }
        (lo, hi)
    }

    /// Texture symbols used by drawn triangles, with triangle counts.
    pub fn texture_usage(&self, pieces: &[Piece]) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for p in pieces {
            for t in &p.tris {
                let key = match t.tex {
                    Some(i) => self.tex_syms[self.tex_states[i as usize].sym as usize].clone(),
                    None => "(untextured)".to_string(),
                };
                *m.entry(key).or_insert(0) += 1;
            }
        }
        m
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Tile {
    fmt: u8,
    siz: u8,
    cmt: u8,
    maskt: u8,
    cms: u8,
    masks: u8,
    uls: u32,
    ult: u32,
    lrs: u32,
    lrt: u32,
}

#[derive(Clone, Debug)]
struct RspState {
    slots: [Option<u32>; 64],
    tex_sym: Option<u16>,
    tex_fmt: u8,
    tex_siz: u8,
    tex_width: u32,
    tiles: [Tile; 8],
    texture_on: bool,
    scale_s: u32,
    scale_t: u32,
    geom: u32,
}

impl Default for RspState {
    fn default() -> Self {
        RspState {
            slots: [None; 64],
            tex_sym: None,
            tex_fmt: 0,
            tex_siz: 0,
            tex_width: 0,
            tiles: [Tile::default(); 8],
            texture_on: false,
            scale_s: 0xFFFF,
            scale_t: 0xFFFF,
            // the game draws a course with back-face culling on (render_courses.c
            // clears G_CULL_BACK around the few two-sided pieces and sets it back)
            geom: crate::mesh::G_CULL_BACK | 0x4 | 0x200 | 0x1,
        }
    }
}

impl RspState {
    fn render_state(&self) -> TexState {
        let t = self.tiles[0];
        let w = if t.lrs >= t.uls { ((t.lrs - t.uls) >> 2) + 1 } else { 0 };
        let h = if t.lrt >= t.ult { ((t.lrt - t.ult) >> 2) + 1 } else { 0 };
        TexState {
            sym: self.tex_sym.unwrap_or(0),
            fmt: self.tex_fmt,
            siz: self.tex_siz,
            w: w as u16,
            h: h as u16,
            uls: (t.uls >> 2) as u16,
            ult: (t.ult >> 2) as u16,
            cms: t.cms,
            cmt: t.cmt,
            masks: t.masks,
            maskt: t.maskt,
            scale_s: self.scale_s as u16,
            scale_t: self.scale_t as u16,
        }
    }
}

fn surface_id(n: &Node) -> Option<u8> {
    let s = n.ident()?;
    SURFACES.iter().find(|(name, _)| *name == s).map(|(_, v)| *v)
}

fn parse_vertex(n: &Node, consts: &Consts) -> Option<Vertex> {
    let row = n.list()?;
    let pos = row.first()?.list()?;
    let tc = row.get(1)?.list()?;
    let col = row.get(2)?.list()?;
    let p: Vec<i64> = pos.iter().filter_map(|x| x.int(consts)).collect();
    let t: Vec<i64> = tc.iter().filter_map(|x| x.int(consts)).collect();
    if p.len() != 3 || t.len() != 2 {
        return None;
    }
    let (rgb, flag) = match col.first()? {
        Node::Call(name, args) if name == "MACRO_COLOR_FLAG" && args.len() == 4 => {
            let r = args[0].int(consts)? as u8;
            let g = args[1].int(consts)? as u8;
            let b = args[2].int(consts)? as u8;
            let f = args[3].int(consts)? as u8;
            ([r & 0xFC, g & 0xFC, b], f)
        }
        other => {
            // a plain colour triple, no flags
            let c: Vec<i64> = std::iter::once(other).chain(col.iter().skip(1)).filter_map(|x| x.int(consts)).collect();
            if c.len() < 3 {
                return None;
            }
            ([c[0] as u8, c[1] as u8, c[2] as u8], 0)
        }
    };
    Some(Vertex { pos: [p[0] as i16, p[1] as i16, p[2] as i16], tc: [t[0] as i16, t[1] as i16], rgb, flag })
}

fn parse_gfx(items: &[Node], consts: &Consts, notes: &mut Vec<String>) -> Vec<Gfx> {
    let mut out = Vec::new();
    for it in items {
        let (name, args) = match it.call() {
            Some(c) => c,
            None => continue,
        };
        let int = |i: usize| args.get(i).and_then(|a| a.int(consts));
        match name {
            "gsSPVertex" => {
                let addr = int(0).unwrap_or(0) as u32;
                let n = int(1).unwrap_or(0) as usize;
                let v0 = int(2).unwrap_or(0) as usize;
                if addr >> 24 == 4 {
                    out.push(Gfx::Vertex { index: ((addr & 0x00FF_FFFF) / 16) as usize, n, v0 });
                } else {
                    out.push(Gfx::ForeignVertex { addr, n, v0 });
                }
            }
            "gsSP1Triangle" => {
                out.push(Gfx::Tri([int(0).unwrap_or(0) as usize, int(1).unwrap_or(0) as usize, int(2).unwrap_or(0) as usize]));
            }
            "gsSP2Triangles" => {
                out.push(Gfx::Tri([int(0).unwrap_or(0) as usize, int(1).unwrap_or(0) as usize, int(2).unwrap_or(0) as usize]));
                out.push(Gfx::Tri([int(4).unwrap_or(0) as usize, int(5).unwrap_or(0) as usize, int(6).unwrap_or(0) as usize]));
            }
            "gsSPDisplayList" => {
                if let Some(s) = args.first().and_then(|a| a.ident()) {
                    out.push(Gfx::Call(s.to_string()));
                }
            }
            "gsSPEndDisplayList" => out.push(Gfx::End),
            "gsDPSetTextureImage" => {
                let sym = args.get(3).and_then(|a| a.ident()).unwrap_or("?").to_string();
                out.push(Gfx::TexImage { fmt: int(0).unwrap_or(0) as u8, siz: int(1).unwrap_or(0) as u8, width: int(2).unwrap_or(0) as u32, sym });
            }
            "gsDPSetTile" => {
                out.push(Gfx::SetTile {
                    fmt: int(0).unwrap_or(0) as u8,
                    siz: int(1).unwrap_or(0) as u8,
                    tile: int(4).unwrap_or(0) as u8,
                    cmt: int(6).unwrap_or(0) as u8,
                    maskt: int(7).unwrap_or(0) as u8,
                    cms: int(9).unwrap_or(0) as u8,
                    masks: int(10).unwrap_or(0) as u8,
                });
            }
            "gsDPSetTileSize" => {
                out.push(Gfx::TileSize {
                    tile: int(0).unwrap_or(0) as u8,
                    uls: int(1).unwrap_or(0) as u32,
                    ult: int(2).unwrap_or(0) as u32,
                    lrs: int(3).unwrap_or(0) as u32,
                    lrt: int(4).unwrap_or(0) as u32,
                });
            }
            "gsSPTexture" => {
                out.push(Gfx::Texture { s: int(0).unwrap_or(0xFFFF) as u32, t: int(1).unwrap_or(0xFFFF) as u32, on: int(4).unwrap_or(1) != 0 });
            }
            "gsSPSetGeometryMode" => match int(0) {
                Some(m) => out.push(Gfx::GeomSet(m as u32)),
                None => notes.push(format!("geometry mode {:?} not evaluated", args.first())),
            },
            "gsSPClearGeometryMode" => match int(0) {
                Some(m) => out.push(Gfx::GeomClear(m as u32)),
                None => notes.push(format!("geometry mode {:?} not evaluated", args.first())),
            },
            other => out.push(Gfx::Other(other.to_string())),
        }
    }
    out
}

/// `courses/<dir>` folders present in a decomp checkout, in `COURSES` order.
pub fn available(decomp: &Path) -> Vec<&'static str> {
    COURSES.iter().filter(|(d, _, _)| decomp.join("courses").join(d).is_dir()).map(|(d, _, _)| *d).collect()
}

pub fn course_title(dir: &str) -> &'static str {
    COURSES.iter().find(|(d, _, _)| *d == dir).map(|(_, t, _)| *t).unwrap_or("?")
}

pub fn official_length_m(dir: &str) -> Option<f32> {
    COURSES.iter().find(|(d, _, _)| *d == dir).and_then(|(_, _, l)| *l)
}
