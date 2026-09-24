//! The decoration's `Scene3d` (`<Coll>\GameCtnDecoration\Scene3d\Base64x64.Scene3d.Gbx`,
//! class 0x0A003000 `CSceneLayout`): the island, sea and shadow-caster
//! geometry every map of a collection sits in, as one OBJ — the first surface
//! of the lightmap baker's sky dome (`lmtool bake --decoration`).
//!
//! The file is a scene layout (lights, then mobils) whose single chunk
//! 0x0A00301C has no typed reader here yet; what the baker needs are its
//! `CPlugSolid` mobils, and those the node walker reads whole (CPlugSolid →
//! CPlugTree → visuals, `classes.rs`). So: find every inline CPlugSolid start
//! in the body (`[i32 index][0x09005000][chunk 0x09005000]`), walk each from
//! there, and draw its tree. The three BlueBay solids (WarpSand island, Water,
//! InvisibleShadowCaster) sit at the identity in the layout (their mobil
//! records carry a zero translation and a unit quaternion) and their vertices
//! are in world metres, so no placement is applied; the extents are printed
//! so a placed solid would show. RedIsland/WhiteShore/GreenCoast keep the
//! geometry in external `Square64Water.Solid.Gbx` / `ShadowCaster64.Solid.Gbx`
//! files under `<Coll>\GameCtnDecoration\Scene3d\Media\Solid\`: an external
//! `.Solid.Gbx` reference is followed and drawn as well.
//!
//! The pak side of this (why the file did not decode until 2026-09-23) is in
//! `parents.rs` and `docs/formats/pak-nadeopak.md` §4.

use crate::node::{Graph, Node, Slot, C_SOLID};
use crate::store::Model;

pub struct Report {
    pub solids: Vec<(i32, String, usize)>,
    pub triangles: usize,
    pub groups: Vec<(String, usize, [f32; 6])>,
}

/// Every `[index][class][first chunk]` inline-node start of `class` in a body.
pub fn inline_starts(body: &[u8], num_nodes: u32, class: u32) -> Vec<(usize, i32)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 12 <= body.len() {
        let idx = i32::from_le_bytes(body[i..i + 4].try_into().unwrap());
        let c = u32::from_le_bytes(body[i + 4..i + 8].try_into().unwrap());
        let chunk = u32::from_le_bytes(body[i + 8..i + 12].try_into().unwrap());
        if c == class && idx >= 1 && (idx as u32) <= num_nodes && chunk & !0xFFF == class {
            out.push((i, idx));
            i += 12;
            continue;
        }
        i += 1;
    }
    out
}

/// The scene of a Scene3d file (or of any file holding CPlugSolids inline),
/// drawn into `collector`'s scene.
pub fn collect(collector: &mut crate::geom::Collector<'_>, path: &str) -> Result<Report, String> {
    let model: Model = collector.store.load_model(path)?;
    let mut graph = Graph::new(&model.body, model.num_nodes, &model.externals);
    let starts = inline_starts(&model.body, model.num_nodes, C_SOLID);
    let mut report = Report { solids: Vec::new(), triangles: 0, groups: Vec::new() };
    for (off, idx) in &starts {
        // already read as part of an earlier solid's tree? (a shared node)
        if !matches!(graph.slots[*idx as usize], Slot::Unset) {
            continue;
        }
        graph.node_at_offset(*off).map_err(|e| format!("{path}: CPlugSolid node {idx} at body 0x{off:x}: {e}"))?;
    }
    // external solids (the other collections' layouts reference theirs)
    let ext_solids: Vec<String> = model.externals.iter().filter(|(_, p)| p.to_ascii_uppercase().ends_with(".SOLID.GBX")).map(|(_, p)| p.clone()).collect();
    let slots = graph.slots.clone();
    drop(graph);
    let before = collector.scene.groups.values().map(|g| g.tris.len()).sum::<usize>();
    for (_, idx) in &starts {
        let name = match &slots[*idx as usize] {
            Slot::Node(Node::ItemModel(tree)) => tree_name(*tree, &slots),
            _ => String::new(),
        };
        let t0 = collector.scene.groups.values().map(|g| g.tris.len()).sum::<usize>();
        collector.slot(*idx, &slots, &crate::geom::IDENTITY, 0);
        let t1 = collector.scene.groups.values().map(|g| g.tris.len()).sum::<usize>();
        report.solids.push((*idx, name, t1 - t0));
    }
    for p in &ext_solids {
        if p.to_ascii_uppercase().contains("SKYDOME") {
            continue; // the mirror dome is the sky, not the ground
        }
        let t0 = collector.scene.groups.values().map(|g| g.tris.len()).sum::<usize>();
        match collector.store.load_model(p) {
            Ok(m) => collector.model(&m, &crate::geom::IDENTITY, 1),
            Err(e) => eprintln!("  external solid {p}: {e}"),
        }
        let t1 = collector.scene.groups.values().map(|g| g.tris.len()).sum::<usize>();
        report.solids.push((-1, p.clone(), t1 - t0));
    }
    let after = collector.scene.groups.values().map(|g| g.tris.len()).sum::<usize>();
    report.triangles = after - before;
    for (name, g) in &collector.scene.groups {
        let mut bb = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
        for v in &g.verts {
            for k in 0..3 {
                bb[k] = bb[k].min(v[k]);
                bb[3 + k] = bb[3 + k].max(v[k]);
            }
        }
        report.groups.push((name.clone(), g.tris.len(), bb));
    }
    Ok(report)
}

fn tree_name(tree: i32, slots: &[Slot]) -> String {
    match slots.get(tree.max(0) as usize) {
        Some(Slot::Node(Node::Tree(t))) if tree >= 0 => t.name.clone(),
        _ => String::new(),
    }
}
