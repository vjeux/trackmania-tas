//! From a file name to triangles: the recursive walk that turns a prefab tree
//! into geometry, following external references across pack files.
//!
//! A block's shape is not one mesh. `RoadDirtTiltCurve3` is a prefab holding
//! two entities, each of which is another prefab file, one of which holds 51
//! entities of its own — road, borders, barrier supports, tree spots. Each
//! entity carries a position and a rotation, so the leaf triangles arrive in
//! the *block's* frame only if every transform on the way down is composed.
//!
//! What comes out is the **collision** surface (`CPlugSurface`), with the
//! game's physics material per triangle. In the dedicated server's Stadium
//! pack that is all there is: every `CPlugStaticObjectModel` on the road
//! reports `mesh = -1` — no `CPlugSolid2Model` — and a `shape`. The server
//! does not render, so it does not ship what it would render with. The visual
//! reader is here anyway (`Node::Solid2`), because a *map's own embedded
//! items* do carry visual meshes and a `Mesh.Gbx` from a game install would
//! too; it is simply never exercised by the stock blocks on this box.

use crate::node::{Node, Slot};
use crate::scene::Scene;
use crate::store::DataStore;
use std::collections::HashMap;

/// A rigid transform: three columns of a rotation, then a translation.
pub type Xform = [f32; 12];

pub const IDENTITY: Xform = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

pub fn apply(m: &Xform, v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2] + m[9],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2] + m[10],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2] + m[11],
    ]
}

pub fn compose(outer: &Xform, inner: &Xform) -> Xform {
    let mut out = [0f32; 12];
    for c in 0..3 {
        let col = [inner[c * 3], inner[c * 3 + 1], inner[c * 3 + 2]];
        out[c * 3] = outer[0] * col[0] + outer[3] * col[1] + outer[6] * col[2];
        out[c * 3 + 1] = outer[1] * col[0] + outer[4] * col[1] + outer[7] * col[2];
        out[c * 3 + 2] = outer[2] * col[0] + outer[5] * col[1] + outer[8] * col[2];
    }
    let t = apply(outer, [inner[9], inner[10], inner[11]]);
    out[9] = t[0];
    out[10] = t[1];
    out[11] = t[2];
    out
}

/// A GBX quaternion (x, y, z, w) and a translation, as one transform.
pub fn from_quat(q: [f32; 4], p: [f32; 3]) -> Xform {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    [
        2.0 * (w * w + x * x) - 1.0,
        2.0 * (x * y + w * z),
        2.0 * (x * z - w * y),
        2.0 * (x * y - w * z),
        2.0 * (w * w + y * y) - 1.0,
        2.0 * (y * z + w * x),
        2.0 * (x * z + w * y),
        2.0 * (y * z - w * x),
        2.0 * (w * w + z * z) - 1.0,
        p[0],
        p[1],
        p[2],
    ]
}

/// A rotation of `steps * 90` degrees clockwise about +Y (looking down), then
/// a translation. This is the map grid's `dir` byte.
pub fn yaw_quarter(steps: u8, t: [f32; 3]) -> Xform {
    let (c, s) = match steps & 3 {
        0 => (1.0, 0.0),
        1 => (0.0, 1.0),
        2 => (-1.0, 0.0),
        _ => (0.0, -1.0),
    };
    // x' = c*x + s*z ; z' = -s*x + c*z  (clockwise looking down, +x east,
    // +z north). The opposite convention scores measurably worse; see
    // MAPGEOM.md "which way a block faces".
    [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c, t[0], t[1], t[2]]
}

/// A yaw of an arbitrary angle about +Y, then a translation.
pub fn yaw(angle: f32, t: [f32; 3]) -> Xform {
    let (s, c) = angle.sin_cos();
    [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c, t[0], t[1], t[2]]
}

#[derive(Default)]
pub struct Stats {
    pub files: usize,
    pub surfaces: usize,
    pub triangles: usize,
    pub visual_meshes: usize,
    /// Nodes where an unknown layout forced a scan to the terminator.
    pub recovered: usize,
    /// Files a walk needed and could not open, with the reason. Never silent:
    /// a missing prefab is a hole in the model, and a hole that is not
    /// reported is indistinguishable from geometry the game does not have.
    pub missing: Vec<(String, String)>,
    /// Classes met that carry no geometry reader yet.
    pub unhandled: HashMap<u32, usize>,
}

pub struct Collector<'a> {
    pub store: &'a mut DataStore,
    pub scene: Scene,
    pub stats: Stats,
    /// Name visual groups by the game material they LINK (`Stadium\Media\
    /// Material\RoadTech|16`, link and physics id) instead of by physics
    /// name: what a crystal item needs to reproduce the surface.
    pub link_labels: bool,
    /// Keep only each Solid2Model's finest LOD level.
    pub finest_lod_only: bool,
    material_cache: HashMap<String, u8>,
    /// `LINK|PHYS` of every material a collision surface named, in the order
    /// met: the look material of terrain whose visual shader is a shared id
    /// material (`Techno3\...`).
    pub surface_links: Vec<String>,
    /// Inside a moving block: triangles collected now are named `(moving)`.
    moving: bool,
    /// Depth guard: prefab trees are shallow, and a cycle would otherwise
    /// spin forever.
    max_depth: usize,
}

impl<'a> Collector<'a> {
    pub fn new(store: &'a mut DataStore) -> Collector<'a> {
        Collector {
            store,
            scene: Scene::default(),
            stats: Stats::default(),
            link_labels: false,
            finest_lod_only: false,
            material_cache: HashMap::new(),
            surface_links: Vec::new(),
            moving: false,
            max_depth: 24,
        }
    }

    /// The scene group a triangle joins: its physics material, marked when it
    /// belongs to the hull of a block that moves.
    fn group_name(&self, phys: u8) -> String {
        let m = crate::scene::material_name(phys);
        if self.moving {
            format!("{} (moving)", m)
        } else {
            m
        }
    }

    /// Add everything a file contributes, placed by `at`.
    pub fn file(&mut self, logical: &str, at: &Xform, depth: usize) {        if depth > self.max_depth {
            self.stats.missing.push((logical.to_string(), "prefab nesting too deep".into()));
            return;
        }
        let model = match self.store.load_model(logical) {
            Ok(m) => m,
            Err(e) => {
                self.stats.missing.push((logical.to_string(), e));
                return;
            }
        };
        self.model(&model, at, depth);
    }

    /// Add everything an already-parsed file contributes. Separate from
    /// `file` because a map's EMBEDDED models have no path in any pack.
    pub fn model(&mut self, model: &crate::store::Model, at: &Xform, depth: usize) {
        let graph = match model.graph() {
            Ok(g) => g,
            Err(e) => {
                self.stats.missing.push((model.path.clone(), e));
                return;
            }
        };
        self.stats.files += 1;
        // The graph borrows the model, so pull out everything needed first.
        let root = graph.root.clone();
        let slots = graph.slots.clone();
        self.stats.recovered += graph.recovered.len();
        drop(graph);
        if let Some(n) = root {
            self.node(&n, &slots, at, depth);
        }
    }

    fn node(&mut self, n: &Node, slots: &[Slot], at: &Xform, depth: usize) {
        match n {
            Node::Prefab(p) => {
                for e in &p.ents {
                    let m = compose(at, &from_quat(e.rot, e.pos));
                    self.slot(e.model, slots, &m, depth);
                }
            }
            Node::StaticObject(s) => {
                if s.shape >= 0 {
                    self.slot(s.shape, slots, at, depth);
                }
                if s.mesh >= 0 {
                    self.slot(s.mesh, slots, at, depth);
                }
            }
            // A MOVING block. Its surface is somewhere different at every
            // instant, so there is no pose that is simply correct — and a
            // swept hull is worse than useless for a ride-height probe,
            // because a rotor sweeps a disc the car is inside for a few
            // hundredths of a second and outside for the rest.
            //
            // What is drawn here is the block **at its authored rest pose**,
            // and the moving hull's triangles are named `<material> (moving)`
            // so that a coverage number can be split rather than averaged. A
            // sample resting on `(moving)` geometry is a sample whose surface
            // this model happens to have caught at t = 0 and would not have
            // caught a tick later; a sample resting on the static half is a
            // real answer. Where a block gives the same node for both — the
            // tube does — it is drawn once, as static.
            Node::Dyna(d) => {
                if d.static_shape >= 0 {
                    self.slot(d.static_shape, slots, at, depth);
                }
                if d.mesh >= 0 {
                    self.slot(d.mesh, slots, at, depth);
                }
                if d.dyna_shape >= 0 && d.dyna_shape != d.static_shape {
                    let was = std::mem::replace(&mut self.moving, true);
                    self.slot(d.dyna_shape, slots, at, depth);
                    self.moving = was;
                }
            }
            Node::Surface(s) => {
                if self.link_labels {
                    for m in &s.materials {
                        if let Some(Slot::External(p)) = slots.get((*m).max(0) as usize) {
                            if p.to_ascii_lowercase().ends_with(".material.gbx") {
                                let phys = self.material_phys(p);
                                let l = format!("{}|{phys}", strip_material_ext(p));
                                if !self.surface_links.contains(&l) {
                                    self.surface_links.push(l);
                                }
                            }
                        }
                    }
                }
                self.stats.surfaces += 1;
                for m in &s.meshes {
                    let verts: Vec<[f32; 3]> = m.verts.iter().map(|v| apply(at, *v)).collect();
                    // Group by physics material so the scene renders as what
                    // the car feels, not as what the artist drew.
                    let mut by_mat: HashMap<u8, Vec<[i32; 3]>> = HashMap::new();
                    for (f, phys, _g) in &m.tris {
                        by_mat.entry(*phys).or_default().push(*f);
                    }
                    self.stats.triangles += m.tris.len();
                    for (phys, tris) in by_mat {
                        self.scene.add_tris(&self.group_name(phys), &verts, tris.into_iter());
                    }
                }
            }
            Node::Solid2(s) => {
                self.stats.visual_meshes += 1;
                // Each shaded geom carries a LOD level; the crystal writer wants
                // the finest one only (every level stacked = z-fighting).
                let min_lod = s.geoms.iter().map(|g| g.lod).min().unwrap_or(0);
                for g in &s.geoms {
                    if self.finest_lod_only && g.lod != min_lod {
                        continue;
                    }
                    let vi = match s.visuals.get(g.visual as usize) {
                        Some(v) => *v,
                        None => continue,
                    };
                    let mat = if self.link_labels {
                        self.material_link(s, g.material, slots)
                    } else {
                        material_label(s, g.material, slots)
                    };
                    if let Some(Node::Visual(v)) = slots.get(vi.max(0) as usize).and_then(as_node) {
                        let mut positions = v.inline_positions.clone();
                        for si in &v.vertex_streams {
                            if let Some(Node::VertexStream(vs)) =
                                slots.get((*si).max(0) as usize).and_then(as_node)
                            {
                                positions.extend_from_slice(&vs.positions);
                            }
                        }
                        let verts: Vec<[f32; 3]> =
                            positions.iter().map(|p| apply(at, *p)).collect();
                        let idx = decode_indices(&v.indices, v.index_is_absolute, verts.len());
                        if std::env::var_os("MAPGEOM_TRACE").is_some() {
                            eprintln!("solid2 geom: visual {} material idx {} -> {:?}: {} inline pos, {} streams, {} verts, {} indices -> {} tris; verts {:?} idx {:?}", vi, g.material, mat, v.inline_positions.len(), v.vertex_streams.len(), verts.len(), v.indices.len(), idx.len(), &verts[..verts.len().min(9)], &idx[..idx.len().min(8)]);
                        }
                        self.scene.add_tris(&mat, &verts, idx.into_iter());
                    }
                }
            }
            Node::ItemModel(i) => self.slot(*i, slots, at, depth),
            Node::Material(..) => {}
            Node::Crystal(c) => {
                self.stats.visual_meshes += 1;
                for m in &c.meshes {
                    let verts: Vec<[f32; 3]> = m.verts.iter().map(|v| apply(at, *v)).collect();
                    let mut by_mat: HashMap<usize, Vec<[i32; 3]>> = HashMap::new();
                    for (inds, mat) in &m.faces {
                        // An n-gon, fan-triangulated about its first corner.
                        for k in 1..inds.len().saturating_sub(1) {
                            by_mat
                                .entry(*mat)
                                .or_default()
                                .push([inds[0], inds[k], inds[k + 1]]);
                        }
                    }
                    for (mat, tris) in by_mat {
                        let name = match c.materials.get(mat) {
                            Some((n, _)) if !n.is_empty() => n.clone(),
                            Some((_, node)) => match slots.get((*node).max(0) as usize) {
                                Some(Slot::Node(Node::Material(n, phys))) => {
                                    // Colour by what the car FEELS, so a custom
                                    // ice ribbon reads as Ice beside a stock one.
                                    // A material that LINKS a Nadeo material
                                    // carries no physics id of its own -- the
                                    // id lives in the linked game material --
                                    // so fall back to the link's own name,
                                    // which is itself a physics name
                                    // (`Stadium\Media\Material\RoadIce`).
                                    let p = crate::scene::physics_name(*phys);
                                    if p != "Unknown" {
                                        p.to_string()
                                    } else {
                                        n.rsplit(['\\', '/']).next().unwrap_or(n).to_string()
                                    }
                                }
                                _ => "CustomMesh".to_string(),
                            },
                            None => "CustomMesh".to_string(),
                        };
                        self.stats.triangles += tris.len();
                        self.scene.add_tris(&name, &verts, tris.into_iter());
                    }
                }
            }
            Node::Visual(_) | Node::VertexStream(_) => {}
            Node::Other(c) => {
                *self.stats.unhandled.entry(*c).or_insert(0) += 1;
            }
        }
    }

    fn slot(&mut self, idx: i32, slots: &[Slot], at: &Xform, depth: usize) {
        if idx < 0 {
            return;
        }
        match slots.get(idx as usize) {
            Some(Slot::Node(n)) => {
                let n = n.clone();
                self.node(&n, slots, at, depth);
            }
            Some(Slot::External(path)) => {
                let path = path.clone();
                // Materials, textures and clips are named here too; only
                // things that can carry geometry are worth opening.
                let up = path.to_uppercase();
                if up.ends_with(".MATERIAL.GBX") || up.ends_with(".TEXTURE.GBX") || up.ends_with(".FXSYS.GBX") || up.ends_with(".LIGHT.GBX") || up.ends_with(".SOUND.GBX") {
                    return;
                }
                self.file(&path, at, depth + 1);
            }
            _ => {}
        }
    }
}

fn as_node(s: &Slot) -> Option<&Node> {
    match s {
        Slot::Node(n) => Some(n),
        _ => None,
    }
}

/// A GBX index buffer is either absolute indices or a delta chain from 0.
fn decode_indices(raw: &[u32], absolute: bool, n: usize) -> Vec<[i32; 3]> {
    let mut flat: Vec<i32> = Vec::with_capacity(raw.len());
    // The flag is not always trustworthy: an "absolute" stream whose values
    // exceed the vertex count is a delta stream (0xFFFF is -1).
    let absolute = absolute && (n == 0 || raw.iter().all(|v| (*v as usize) < n));
    if absolute || n == 0 {
        flat.extend(raw.iter().map(|v| *v as i32));
    } else {
        let mut cur = 0i64;
        for d in raw {
            cur = (cur + *d as i16 as i64).rem_euclid(n as i64);
            flat.push(cur as i32);
        }
    }
    flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

impl<'a> Collector<'a> {
    /// `LINK|PHYS` for a shaded geometry's material: the pack material file
    /// it references (extension dropped) and that material's surface id, read
    /// from the material file itself. Inline materials use their own name.
    fn material_link(&mut self, s: &crate::node::Solid2, idx: i32, slots: &[Slot]) -> String {
        let i = idx.max(0) as usize;
        if std::env::var_os("MAPGEOM_TRACE").is_some() {
            eprintln!(
                "material_link idx {idx}: names {:?} nodes {:?} -> slot {:?}",
                s.material_names,
                s.material_nodes,
                s.material_nodes.get(i).and_then(|n| slots.get((*n).max(0) as usize)).map(|sl| match sl {
                    Slot::External(p) => format!("External({p})"),
                    Slot::Node(Node::Material(n, p)) => format!("Material({n:?}, {p})"),
                    Slot::Node(n) => format!("Node({:?})", std::mem::discriminant(n)),
                    _ => "other".to_string(),
                })
            );
        }
        if let Some(n) = s.material_names.get(i) {
            if !n.is_empty() {
                return format!("{}|16", n.trim_end_matches(".Material.Gbx").trim_end_matches(".Material.gbx"));
            }
        }
        match s.material_nodes.get(i).and_then(|n| slots.get((*n).max(0) as usize)) {
            Some(Slot::External(path)) => {
                let link = strip_material_ext(path);
                let phys = self.material_phys(path);
                format!("{link}|{phys}")
            }
            Some(Slot::Node(Node::Material(name, phys))) if name.starts_with("@refs:") => {
                // An inline CPlugMaterial: the game material is the external
                // .Material.Gbx it references (first one wins).
                let link = name["@refs:".len()..]
                    .split(',')
                    .filter_map(|t| t.parse::<i32>().ok())
                    .filter(|i| *i >= 0)
                    .find_map(|i| match slots.get(i as usize) {
                        Some(Slot::External(p)) if p.to_ascii_lowercase().ends_with(".material.gbx") => Some(strip_material_ext(p)),
                        _ => None,
                    });
                match link {
                    Some(l) => format!("{l}|{phys}"),
                    None => format!("|{phys}"),
                }
            }
            Some(Slot::Node(Node::Material(name, phys))) if !name.is_empty() => format!("{}|{phys}", strip_material_ext(name)),
            Some(Slot::Node(Node::Material(_, phys))) => format!("|{phys}"),
            _ => "|16".to_string(),
        }
    }
}

impl<'a> Collector<'a> {
    /// The surface physics id a pack material file carries (cached).
    fn material_phys(&mut self, path: &str) -> u8 {
        if let Some(p) = self.material_cache.get(path) {
            return *p;
        }
        let p = match self.store.load_model(path) {
            Ok(m) => match m.graph() {
                Ok(g) => match g.slots.first().and_then(as_node) {
                    Some(Node::Material(_, phys)) => *phys,
                    _ => 16,
                },
                Err(_) => 16,
            },
            Err(_) => 16,
        };
        self.material_cache.insert(path.to_string(), p);
        p
    }
}

fn strip_material_ext(p: &str) -> String {
    let lower = p.to_ascii_lowercase();
    for ext in [".material.gbx"] {
        if lower.ends_with(ext) {
            return p[..p.len() - ext.len()].to_string();
        }
    }
    p.to_string()
}

/// What to call a shaded geometry's material: the physics id the car feels
/// where the model says so, the material's own name where it does not.
fn material_label(s: &crate::node::Solid2, idx: i32, slots: &[Slot]) -> String {
    if let Some(n) = s.material_names.get(idx.max(0) as usize) {
        if !n.is_empty() {
            return n.rsplit(['\\', '/']).next().unwrap_or(n).to_string();
        }
    }
    if let Some(node) = s.material_nodes.get(idx.max(0) as usize) {
        if let Some(Node::Material(n, phys)) = slots.get((*node).max(0) as usize).and_then(as_node)
        {
            let p = crate::scene::physics_name(*phys);
            if p != "Unknown" {
                return p.to_string();
            }
            return n.rsplit(['\\', '/']).next().unwrap_or(n).to_string();
        }
    }
    "Visual".to_string()
}
