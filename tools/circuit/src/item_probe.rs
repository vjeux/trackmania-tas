//! What a mesh-modeler item (`CGameItemModel` + `CPlugCrystal`) carries, so a
//! template can be chosen with open eyes: every layer with its kind and
//! size, every material slot, and the waypoint chunk if the body has one.

use mapgeom::crystal::ItemCrystal;
use mapgeom::crystal_model::LayerKind;
use std::path::Path;

pub fn probe(path: &Path) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    println!("== {} ({} bytes)", path.display(), bytes.len());
    let it = match ItemCrystal::open(&bytes) {
        Ok(it) => it,
        Err(e) => {
            println!("   not a crystal item: {e}");
            return;
        }
    };
    println!("   body {} bytes, crystal node {} at 0x{:x}..0x{:x}, {} nodes", it.body.len(), it.loc.node_index, it.loc.at, it.end, it.gbx.num_nodes);
    for (i, m) in it.model.materials.iter().enumerate() {
        match m.inst() {
            Some(inst) => println!("   material {i}: link {:?} physics {}", inst.link().unwrap_or(""), inst.physics()),
            None => println!("   material {i}: name {:?} (no inst)", m.name),
        }
    }
    for (i, l) in it.model.layers.iter().enumerate() {
        let base = &l.base;
        match &l.kind {
            LayerKind::Geometry { crystal, is_visible, collidable, .. } => {
                let (lo, hi) = bounds(&crystal.positions);
                println!(
                    "   layer {i} Geometry {:?} v{} visible {} collidable {}: {} positions, {} faces, {} groups; bounds {:?}..{:?}",
                    base.layer_name, crystal.version, is_visible, collidable, crystal.positions.len(), crystal.faces.len(), crystal.groups.len(), lo, hi
                );
            }
            LayerKind::Trigger { crystal, .. } => {
                let (lo, hi) = bounds(&crystal.positions);
                println!("   layer {i} Trigger {:?}: {} positions, {} faces; bounds {:?}..{:?}", base.layer_name, crystal.positions.len(), crystal.faces.len(), lo, hi);
            }
            LayerKind::SpawnPosition { position, horizontal_angle, vertical_angle, roll_angle, .. } => {
                println!("   layer {i} SpawnPosition {:?}: at {:?} h {} v {} roll {}", base.layer_name, position, horizontal_angle, vertical_angle, roll_angle);
            }
            other => println!("   layer {i} {} {:?}", other.name(), base.layer_name),
        }
    }
    // The waypoint chunk (0x2E00201F) lives outside the crystal, in the item
    // body; find it by id and print its type word.
    let body = &it.body;
    let mut i = 0;
    while i + 8 <= body.len() {
        if u32::from_le_bytes(body[i..i + 4].try_into().unwrap()) == 0x2E00201F {
            let v = u32::from_le_bytes(body[i + 4..i + 8].try_into().unwrap());
            let t = i32::from_le_bytes(body[i + 8..i + 12].try_into().unwrap());
            println!("   waypoint chunk 0x2E00201F at 0x{:x}: version {v} type {t} (0 start, 1 finish, 2 checkpoint, 3 none, 4 start+finish)", i);
        }
        i += 4;
    }
}

fn bounds(p: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for v in p {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    (lo, hi)
}
