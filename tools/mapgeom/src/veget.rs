//! The procedural vegetation models (`.VegetTreeModel.Gbx`, class
//! 0x2F086000): a table-less struct the node walker cannot read, but its
//! meshes are ordinary `CPlugVisualIndexedTriangles` nodes written inline,
//! each with the visual's bounding box in chunk 0x0900600F. The union of
//! those boxes is the species' size — what the tiny transform needs to sink
//! a full-size tree so its crown ends where a half-size tree's would.

use crate::static_item::visual::CPlugVisualIndexedTriangles;
use crate::static_item::{LookbackState, Rd};
use crate::store::DataStore;

const CLASS_VISUAL_INDEXED_TRIANGLES: u32 = 0x0901E000;

#[derive(Clone, Debug)]
pub struct VisualStats {
    pub vertices: usize,
    /// Centre xyz, half extents xyz (the chunk's own words).
    pub bbox: [f32; 6],
}

#[derive(Clone, Debug)]
pub struct TreeStats {
    pub visuals: Vec<VisualStats>,
    /// Lowest / highest y over every visual's box (metres, model space).
    pub bottom: f32,
    pub top: f32,
    /// The widest horizontal half extent.
    pub radius: f32,
}

/// The VegetTreeModel behind a path: the path itself, or the model an
/// `.Item.Gbx` references (`Stadium\Items\PalmTreeSmall.Item.Gbx`).
pub fn tree_model_path(store: &mut DataStore, path: &str) -> Result<String, String> {
    if path.to_ascii_lowercase().ends_with(".vegettreemodel.gbx") {
        return Ok(path.to_string());
    }
    let m = store.load_model(path)?;
    m.externals
        .iter()
        .map(|(_, p)| p.clone())
        .find(|n| n.to_ascii_lowercase().ends_with(".vegettreemodel.gbx"))
        .ok_or_else(|| format!("{path}: no VegetTreeModel reference"))
}

/// Every inline `CPlugVisualIndexedTriangles` of a tree model file and the
/// union of their boxes.
pub fn tree_model_stats(store: &mut DataStore, path: &str) -> Result<TreeStats, String> {
    let model_path = tree_model_path(store, path)?;
    // a model whose fold hunt stops short still decodes up to the failing chunk;
    // the visuals before it are read (the LOD boxes of a species agree to
    // centimetres — the Big palms of BlueBay decode 17 of 20 chunks)
    let m = match store.load_model(&model_path) {
        Ok(m) => m,
        Err(e) => {
            std::env::set_var("MAPGEOM_LENIENT_LZ4", "1");
            let r = store.load_model(&model_path);
            std::env::remove_var("MAPGEOM_LENIENT_LZ4");
            r.map_err(|e2| format!("{e}; partial read: {e2}"))?
        }
    };
    let body = &m.body;
    let externals: Vec<u32> = m.externals.iter().map(|(i, _)| *i).collect();
    let mut visuals = Vec::new();
    let mut i = 0usize;
    while i + 4 <= body.len() {
        let cid = u32::from_le_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]);
        if cid == CLASS_VISUAL_INDEXED_TRIANGLES {
            // the node index sits just before the class id
            let mut lb = LookbackState::default();
            // the body's lookback version word went by with the struct's own
            // strings (material names) before the first visual
            lb.version_seen = true;
            lb.defined_nodes.extend(externals.iter().copied());
            let mut r = Rd::new(body, i + 4, lb);
            let parsed = CPlugVisualIndexedTriangles::parse(&mut r);
            if std::env::var_os("MAPGEOM_VEGET_DEBUG").is_some() {
                if let Err(e) = &parsed {
                    eprintln!("  visual at body offset {i:#x}: {e}");
                }
            }
            if let Ok(v) = parsed {
                if let Some(m) = &v.main {
                    visuals.push(VisualStats { vertices: m.count.max(0) as usize, bbox: m.bounding_box });
                    i = r.o;
                    continue;
                }
            }
        }
        i += 1;
    }
    if visuals.is_empty() {
        return Err(format!("{model_path}: no visual parsed ({} body bytes)", body.len()));
    }
    let mut bottom = f32::MAX;
    let mut top = f32::MIN;
    let mut radius = 0.0f32;
    for v in &visuals {
        let [cx, cy, cz, hx, hy, hz] = v.bbox;
        bottom = bottom.min(cy - hy);
        top = top.max(cy + hy);
        radius = radius.max((cx.abs() + hx).max(cz.abs() + hz));
    }
    Ok(TreeStats { visuals, bottom, top, radius })
}
