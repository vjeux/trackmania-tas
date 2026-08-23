//! The output side: a triangle scene in TM world coordinates, written as glTF
//! (`.glb`) or Wavefront OBJ.
//!
//! **The coordinate frame is the game's, unchanged.** X east, Y up, Z north,
//! metres — the same numbers a ghost's samples carry. Nothing here mirrors an
//! axis to please a viewer's handedness, because the whole point of the model
//! is to be measured against trajectories, and an axis flip applied for looks
//! is the kind of thing that later gets mistaken for a physics finding.
//!
//! Faces carry the game's physics material, so a scene renders as what the car
//! feels: tarmac, dirt, ice, grass, water.

use std::collections::BTreeMap;

/// `EPlugSurfaceMaterialId` — what the car feels under a wheel.
///
/// Transcribed from the game's own class reference
/// (`next.openplanet.dev/MetaNotPersistent/GmSurfaceIds`, build 2026-01-26),
/// not inferred. **The list this replaced was right to 25 and wrong from 26
/// on**, and wrong in a way that mattered rather than cosmetically: it called
/// 27 `RoadIce` when 27 is `Bumper_Deprecated` and RoadIce is 74, and — worse
/// — it called **28 `Bumper` when 28 is `NotCollidable`**, so triangles the
/// car cannot touch were being counted as surface underneath it.
pub fn physics_name(id: u8) -> &'static str {
    match id {
        0 => "Concrete",
        1 => "Pavement",
        2 => "Grass",
        3 => "Ice",
        4 => "Metal",
        5 => "Sand",
        6 => "Dirt",
        7 => "Turbo_Deprecated",
        8 => "DirtRoad",
        9 => "Rubber",
        10 => "SlidingRubber",
        11 => "Test",
        12 => "Rock",
        13 => "Water",
        14 => "Wood",
        15 => "Danger",
        16 => "Asphalt",
        17 => "WetDirtRoad",
        18 => "WetAsphalt",
        19 => "WetPavement",
        20 => "WetGrass",
        21 => "Snow",
        22 => "ResonantMetal",
        23 => "GolfBall",
        24 => "GolfWall",
        25 => "GolfGround",
        26 => "Turbo2_Deprecated",
        27 => "Bumper_Deprecated",
        28 => "NotCollidable",
        29 => "FreeWheeling_Deprecated",
        30 => "TurboRoulette_Deprecated",
        31 => "WallJump",
        32 => "MetalTrans",
        33 => "Stone",
        34 => "Player",
        35 => "Trunk",
        36 => "TechLaser",
        37 => "SlidingWood",
        38 => "PlayerOnly",
        39 => "Tech",
        40 => "TechArmor",
        41 => "TechSafe",
        42 => "OffZone",
        43 => "Bullet",
        44 => "TechHook",
        45 => "TechGround",
        46 => "TechWall",
        47 => "TechArrow",
        48 => "TechHook2",
        49 => "Forest",
        50 => "Wheat",
        51 => "TechTarget",
        52 => "PavementStair",
        53 => "TechTeleport",
        54 => "Energy",
        55 => "TechMagnetic",
        56 => "TurboTechMagnetic_Deprecated",
        57 => "Turbo2TechMagnetic_Deprecated",
        58 => "TurboWood_Deprecated",
        59 => "Turbo2Wood_Deprecated",
        60 => "FreeWheelingTechMagnetic_Deprecated",
        61 => "FreeWheelingWood_Deprecated",
        62 => "TechSuperMagnetic",
        63 => "TechNucleus",
        64 => "TechMagneticAccel",
        65 => "MetalFence",
        66 => "TechGravityChange",
        67 => "TechGravityReset",
        68 => "RubberBand",
        69 => "Gravel",
        70 => "Hack_NoGrip_Deprecated",
        71 => "Bumper2_Deprecated",
        72 => "NoSteering_Deprecated",
        73 => "NoBrakes_Deprecated",
        74 => "RoadIce",
        75 => "RoadSynthetic",
        76 => "Green",
        77 => "Plastic",
        78 => "DevDebug",
        79 => "Free3",
        80 => "XXX_Null",
        _ => "Unknown",
    }
}

/// A surface the car cannot rest on, however solid it looks in a render.
///
/// `NotCollidable` speaks for itself; `OffZone` is the volume that resets a
/// run. Both are real triangles in the pack and both would happily hold up a
/// plumb probe, so the coverage index leaves them out. On this corpus that is
/// currently a change of nothing — no map has a sample over either — which is
/// worth knowing and is not the same as not checking.
pub fn is_collidable(name: &str) -> bool {
    !matches!(name, "NotCollidable" | "OffZone")
}

/// The name a triangle's group carries: the physics material, and where the
/// enum runs past what is written down here, the raw id.
///
/// The table above is the whole enum as the reference gives it, so an
/// id past its end is a value the reference does not have. It used to read as
/// a bare `Unknown` — a class of ids, not an id, which tells you nothing about
/// which one to go and look up. The number is free and it is the diagnosis.
pub fn material_name(id: u8) -> String {
    match physics_name(id) {
        "Unknown" => format!("Unknown({})", id),
        n => n.to_string(),
    }
}

/// A rough colour per physics material, so a rendered scene reads at a glance.
fn physics_colour(id: u8) -> [f32; 4] {
    match physics_name(id) {
        "Concrete" | "Asphalt" | "WetAsphalt" | "Pavement" | "WetPavement" => [0.32, 0.33, 0.35, 1.0],
        "Grass" | "WetGrass" | "Green" => [0.24, 0.52, 0.22, 1.0],
        "Ice" | "RoadIce" => [0.55, 0.82, 0.94, 1.0],
        "Metal" | "ResonantMetal" | "MetalTrans" => [0.62, 0.64, 0.68, 1.0],
        "Sand" => [0.85, 0.76, 0.48, 1.0],
        "Dirt" | "DirtRoad" | "WetDirtRoad" => [0.55, 0.36, 0.20, 1.0],
        "Water" => [0.15, 0.35, 0.75, 0.65],
        "Wood" => [0.52, 0.36, 0.18, 1.0],
        "Turbo_Deprecated" | "Turbo2_Deprecated" | "TurboRoulette_Deprecated" => {
            [0.95, 0.72, 0.10, 1.0]
        }
        "Danger" => [0.85, 0.10, 0.10, 1.0],
        "Rubber" | "SlidingRubber" | "RubberBand" | "Bumper_Deprecated" | "Bumper2_Deprecated" => {
            [0.30, 0.16, 0.34, 1.0]
        }
        "Snow" => [0.93, 0.95, 0.98, 1.0],
        "Rock" => [0.42, 0.40, 0.38, 1.0],
        "Plastic" => [0.70, 0.55, 0.60, 1.0],
        "Stone" | "PavementStair" | "Gravel" => [0.50, 0.48, 0.45, 1.0],
        _ => [0.60, 0.60, 0.62, 1.0],
    }
}

#[derive(Default)]
pub struct Group {
    pub verts: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
}

/// A polyline in world coordinates — a driven trajectory, a centreline, a
/// probe. Written as a glTF `LINE_STRIP` primitive and as an OBJ `l` element.
pub struct Line {
    pub name: String,
    pub points: Vec<[f32; 3]>,
    pub colour: [f32; 4],
}

#[derive(Default)]
pub struct Scene {
    /// Triangles grouped by material name; the key orders the output.
    pub groups: BTreeMap<String, Group>,
    pub lines: Vec<Line>,
}

impl Scene {
    pub fn tri_count(&self) -> usize {
        self.groups.values().map(|g| g.tris.len()).sum()
    }
    pub fn vert_count(&self) -> usize {
        self.groups.values().map(|g| g.verts.len()).sum()
    }

    pub fn add_tris(
        &mut self,
        material: &str,
        verts: &[[f32; 3]],
        tris: impl Iterator<Item = [i32; 3]>,
    ) {
        let g = self.groups.entry(material.to_string()).or_default();
        let base = g.verts.len() as u32;
        g.verts.extend_from_slice(verts);
        for t in tris {
            if t.iter().any(|&i| i < 0 || i as usize >= verts.len()) {
                continue;
            }
            g.tris.push([base + t[0] as u32, base + t[1] as u32, base + t[2] as u32]);
        }
    }

    pub fn add_line(&mut self, name: &str, points: Vec<[f32; 3]>, colour: [f32; 4]) {
        self.lines.push(Line { name: name.to_string(), points, colour });
    }

    /// Append another scene's triangles, transformed. This is how a block
    /// model is INSTANCED: its geometry is read from the pack once and placed
    /// as many times as the map places the block.
    pub fn append(&mut self, other: &Scene, xf: &crate::geom::Xform) {
        for (name, g) in &other.groups {
            let dst = self.groups.entry(name.clone()).or_default();
            let base = dst.verts.len() as u32;
            dst.verts.extend(g.verts.iter().map(|v| crate::geom::apply(xf, *v)));
            dst.tris.extend(g.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
        }
    }

    /// The largest coordinate on each axis, or zero for an empty scene. Used
    /// to size a block model's footprint in whole cells.
    pub fn max_corner(&self) -> [f32; 3] {
        self.bounds().map(|(_, hi)| hi).unwrap_or([0.0; 3])
    }

    /// The scene's axis-aligned bounds, or `None` when it is empty.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        let mut any = false;
        for g in self.groups.values() {
            for v in &g.verts {
                any = true;
                for k in 0..3 {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                }
            }
        }
        if any {
            Some((lo, hi))
        } else {
            None
        }
    }

    // ------------------------------------------------------------------ OBJ

    /// Wavefront OBJ plus the matching MTL, as (obj, mtl).
    pub fn obj(&self, mtl_name: &str) -> (String, String) {
        let mut obj = String::new();
        let mut mtl = String::new();
        obj.push_str("# mapgeom -- TM2020 world coordinates (X east, Y up, Z north), metres\n");
        obj.push_str(&format!("mtllib {}\n", mtl_name));
        let mut base = 1usize;
        for (name, g) in &self.groups {
            let c = colour_for(name);
            mtl.push_str(&format!(
                "newmtl {}\nKd {:.3} {:.3} {:.3}\nd {:.3}\n\n",
                name, c[0], c[1], c[2], c[3]
            ));
            obj.push_str(&format!("o {}\nusemtl {}\n", name, name));
            for v in &g.verts {
                obj.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
            }
            for t in &g.tris {
                obj.push_str(&format!(
                    "f {} {} {}\n",
                    base + t[0] as usize,
                    base + t[1] as usize,
                    base + t[2] as usize
                ));
            }
            base += g.verts.len();
        }
        for l in &self.lines {
            mtl.push_str(&format!(
                "newmtl {}\nKd {:.3} {:.3} {:.3}\n\n",
                l.name, l.colour[0], l.colour[1], l.colour[2]
            ));
            obj.push_str(&format!("o {}\nusemtl {}\n", l.name, l.name));
            for p in &l.points {
                obj.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
            }
            if l.points.len() >= 2 {
                obj.push_str("l");
                for i in 0..l.points.len() {
                    obj.push_str(&format!(" {}", base + i));
                }
                obj.push('\n');
            }
            base += l.points.len();
        }
        (obj, mtl)
    }

    // ------------------------------------------------------------------ glB

    /// Binary glTF 2.0. One primitive per material group, plus one
    /// `LINE_STRIP` primitive per polyline.
    pub fn glb(&self) -> Vec<u8> {
        let mut bin: Vec<u8> = Vec::new();
        let mut views = String::new();
        let mut accessors = String::new();
        let mut meshes = String::new();
        let mut materials = String::new();
        let mut nodes = String::new();
        let mut n_views = 0usize;
        let mut n_acc = 0usize;
        let mut n_mesh = 0usize;
        let mut n_mat = 0usize;

        let push_view = |bin: &mut Vec<u8>, views: &mut String, bytes: &[u8], n: &mut usize| {
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let off = bin.len();
            bin.extend_from_slice(bytes);
            if *n > 0 {
                views.push(',');
            }
            views.push_str(&format!(
                "{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{}}}",
                off,
                bytes.len()
            ));
            *n += 1;
            *n - 1
        };

        let emit = |verts: &[[f32; 3]],
                        idx: &[u32],
                        mode: u32,
                        name: &str,
                        colour: [f32; 4],
                        bin: &mut Vec<u8>,
                        views: &mut String,
                        accessors: &mut String,
                        meshes: &mut String,
                        materials: &mut String,
                        nodes: &mut String,
                        n_views: &mut usize,
                        n_acc: &mut usize,
                        n_mesh: &mut usize,
                        n_mat: &mut usize| {
            if verts.is_empty() {
                return;
            }
            let mut pos_bytes = Vec::with_capacity(verts.len() * 12);
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for v in verts {
                for k in 0..3 {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                    pos_bytes.extend_from_slice(&v[k].to_le_bytes());
                }
            }
            let pv = push_view(bin, views, &pos_bytes, n_views);
            let mut idx_bytes = Vec::with_capacity(idx.len() * 4);
            for i in idx {
                idx_bytes.extend_from_slice(&i.to_le_bytes());
            }
            let iv = push_view(bin, views, &idx_bytes, n_views);
            if *n_acc > 0 {
                accessors.push(',');
            }
            accessors.push_str(&format!(
                "{{\"bufferView\":{},\"componentType\":5126,\"count\":{},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}}",
                pv, verts.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
            ));
            let pos_acc = *n_acc;
            *n_acc += 1;
            accessors.push(',');
            accessors.push_str(&format!(
                "{{\"bufferView\":{},\"componentType\":5125,\"count\":{},\"type\":\"SCALAR\"}}",
                iv,
                idx.len()
            ));
            let idx_acc = *n_acc;
            *n_acc += 1;

            if *n_mat > 0 {
                materials.push(',');
            }
            materials.push_str(&format!(
                "{{\"name\":{},\"doubleSided\":true,\"pbrMetallicRoughness\":{{\"baseColorFactor\":[{},{},{},{}],\"metallicFactor\":0.0,\"roughnessFactor\":0.9}}{}}}",
                json_str(name), colour[0], colour[1], colour[2], colour[3],
                if colour[3] < 1.0 { ",\"alphaMode\":\"BLEND\"" } else { "" }
            ));
            let mat = *n_mat;
            *n_mat += 1;

            if *n_mesh > 0 {
                meshes.push(',');
                nodes.push(',');
            }
            meshes.push_str(&format!(
                "{{\"name\":{},\"primitives\":[{{\"attributes\":{{\"POSITION\":{}}},\"indices\":{},\"material\":{},\"mode\":{}}}]}}",
                json_str(name), pos_acc, idx_acc, mat, mode
            ));
            nodes.push_str(&format!("{{\"mesh\":{},\"name\":{}}}", *n_mesh, json_str(name)));
            *n_mesh += 1;
        };

        for (name, g) in &self.groups {
            let flat: Vec<u32> = g.tris.iter().flat_map(|t| t.iter().copied()).collect();
            emit(
                &g.verts, &flat, 4, name, colour_for(name), &mut bin, &mut views, &mut accessors,
                &mut meshes, &mut materials, &mut nodes, &mut n_views, &mut n_acc, &mut n_mesh,
                &mut n_mat,
            );
        }
        for l in &self.lines {
            let idx: Vec<u32> = (0..l.points.len() as u32).collect();
            emit(
                &l.points, &idx, 3, &l.name, l.colour, &mut bin, &mut views, &mut accessors,
                &mut meshes, &mut materials, &mut nodes, &mut n_views, &mut n_acc, &mut n_mesh,
                &mut n_mat,
            );
        }

        let node_list: Vec<String> = (0..n_mesh).map(|i| i.to_string()).collect();
        let json = format!(
            "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"mapgeom\"}},\
             \"scene\":0,\"scenes\":[{{\"nodes\":[{}]}}],\
             \"nodes\":[{}],\"meshes\":[{}],\"materials\":[{}],\
             \"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]}}",
            node_list.join(","),
            nodes,
            meshes,
            materials,
            accessors,
            views,
            bin.len()
        );
        let mut json_bytes = json.into_bytes();
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0x42, 0x49, 0x4E, 0x00]);
        out.extend_from_slice(&bin);
        out
    }
}

pub fn colour_for(name: &str) -> [f32; 4] {
    for id in 0..=80u8 {
        if physics_name(id) == name {
            return physics_colour(id);
        }
    }
    [0.6, 0.6, 0.62, 1.0]
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A glb has to be a glb: the 12-byte header's length field must equal the
    /// file, and both chunk headers must land on 4-byte boundaries. A viewer
    /// that refuses the file tells you nothing about the geometry, so this is
    /// pinned separately from anything about meshes.
    #[test]
    fn glb_container_is_well_formed() {
        let mut s = Scene::default();
        s.add_tris(
            "Concrete",
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            [[0, 1, 2]].into_iter(),
        );
        s.add_line("run", vec![[0.0; 3], [1.0, 2.0, 3.0]], [1.0, 0.0, 0.0, 1.0]);
        let b = s.glb();
        assert_eq!(&b[0..4], b"glTF");
        let total = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        assert_eq!(total, b.len());
        let jlen = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
        assert_eq!(jlen % 4, 0);
        assert_eq!(&b[16..20], b"JSON");
        let blen = u32::from_le_bytes(b[20 + jlen..24 + jlen].try_into().unwrap()) as usize;
        assert_eq!(blen % 4, 0);
        assert_eq!(28 + jlen + blen, b.len());
    }

    /// Out-of-range indices are dropped, not written: a face that points past
    /// the vertex array crashes a viewer, and a crashed viewer looks like bad
    /// geometry rather than a bad index.
    #[test]
    fn out_of_range_faces_are_dropped() {
        let mut s = Scene::default();
        s.add_tris("Dirt", &[[0.0; 3], [1.0; 3]], [[0, 1, 7]].into_iter());
        assert_eq!(s.tri_count(), 0);
    }
}
