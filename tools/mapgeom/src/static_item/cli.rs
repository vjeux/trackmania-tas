//! `mapgeom static-item <prefab-or-item> --out F --ident NAME.Item.Gbx
//! --author X --scale 0.5 [--collection 26]`: build a static item from a
//! pack prefab (logical path) or a local `.Item.Gbx` (static or crystal),
//! then re-parse the output and print its structure as a check.

use super::build;
use crate::store::DataStore;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// `rest` is the argument list starting at the subcommand; `open` yields the
/// pack store when a prefab is asked for.
pub fn run(rest: &[String], open: &mut dyn FnMut() -> DataStore) -> Result<(), String> {
    let src = rest.get(1).cloned().ok_or("static-item <prefab-or-item> --out F --ident NAME.Item.Gbx --author X --scale S [--collection 26]")?;
    let out = flag(rest, "--out").ok_or("--out FILE")?;
    let ident = flag(rest, "--ident").ok_or("--ident NAME.Item.Gbx")?;
    let author = flag(rest, "--author").ok_or("--author X")?;
    let scale: f32 = flag(rest, "--scale").unwrap_or_else(|| "1".into()).parse().map_err(|e| format!("--scale: {e}"))?;
    let collection: u32 = flag(rest, "--collection").unwrap_or_else(|| "26".into()).parse().map_err(|e| format!("--collection: {e}"))?;
    let is_file = std::path::Path::new(&src).is_file();
    let (bytes, merged) = if is_file {
        let data = std::fs::read(&src).map_err(|e| format!("{src}: {e}"))?;
        build::static_item_from_item_report(&data, &ident, &author, scale, collection)?
    } else if src.to_ascii_lowercase().ends_with(".item.gbx") {
        // a pack ITEM: baked through its external prefab / static-object files
        let mut store = open();
        build::static_item_from_pack_item_report(&mut store, &src, &ident, &author, scale, collection)?
    } else {
        let mut store = open();
        build::static_item_from_prefab_report(&mut store, &src, &ident, &author, scale, collection)?
    };
    std::fs::write(&out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    for n in &merged.notes {
        println!("  note: {n}");
    }
    // Verify with our own parser.
    let f = super::parse_file(&bytes).map_err(|e| format!("output does not parse back: {e}"))?;
    if super::write_file(&f) != bytes {
        return Err("output does not round-trip through the parser".into());
    }
    let so = f.item.static_object().ok_or("output has no static object")?;
    let s2 = so.solid2().ok_or("output has no solid")?;
    let ntri = so.surface().map(|s| match &s.surf {
        super::surface::Surf::Mesh { triangles, vertices, .. } => (vertices.len(), triangles.len()),
        _ => (0, 0),
    });
    // Structural consistency of every visual (what the game trips over).
    for (vi, vr) in s2.visuals.iter().enumerate() {
        let Some(super::Node::Visual(v)) = vr.inline.as_deref() else { continue };
        let Some(m) = v.main.as_ref() else { continue };
        let count = m.count.max(0) as usize;
        let per = (((!(m.flags() >> 17)) & 8) | 4) as usize;
        if let Some(s) = v.stream() {
            if s.count.max(0) as usize != count {
                return Err(format!("visual {vi}: stream count {} != visual count {count}", s.count));
            }
            for (d, e) in s.decls.iter().zip(s.elems.iter()) {
                if e.len() != count {
                    return Err(format!("visual {vi}: element name{} has {} entries for {count} vertices", d.name(), e.len()));
                }
            }
        }
        if let Some((a, b)) = v.tangents.as_ref() {
            for t in [a, b] {
                if !t.is_empty() && t.len() != count * per {
                    return Err(format!("visual {vi}: tangent array {} bytes != {count} x {per}", t.len()));
                }
            }
        }
        if let Some(ib) = v.index_buffer.as_ref() {
            if ib.indices.len() % 3 != 0 {
                return Err(format!("visual {vi}: {} indices is not a triangle list", ib.indices.len()));
            }
            if let Some(mx) = ib.indices.iter().max() {
                if *mx as usize >= count {
                    return Err(format!("visual {vi}: index {mx} past {count} vertices"));
                }
            }
        }
    }
    let nverts: usize = s2
        .visuals
        .iter()
        .filter_map(|v| match v.inline.as_deref() {
            Some(super::Node::Visual(x)) => x.main.as_ref().map(|m| m.count as usize),
            _ => None,
        })
        .sum();
    println!(
        "wrote {out}: {} bytes, {} nodes, {} visuals ({nverts} vertices), {} materials [{}], collision {:?} (verts, tris), physics ids {:?}",
        bytes.len(),
        f.num_nodes,
        s2.visuals.len(),
        s2.custom_materials.len(),
        s2.custom_materials.iter().filter_map(|m| m.inst()).map(|i| format!("{} ({})", i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?"), i.physics())).collect::<Vec<_>>().join(", "),
        ntri,
        so.surface().map(|s| s.material_ids.clone()).unwrap_or_default()
    );
    Ok(())
}
