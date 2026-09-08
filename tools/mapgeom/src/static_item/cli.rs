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
    // --variant N: which entry of a pack item's variant list to bake (the placement's variant byte)
    let variant: usize = flag(rest, "--variant").unwrap_or_else(|| "0".into()).parse().map_err(|e| format!("--variant: {e}"))?;
    // --light-skin NAME: bake the placement's light colour skin (Coral, Off, …) into the lights and the glass
    let light_skin = match flag(rest, "--light-skin") {
        Some(name) => Some(crate::light_skin::lookup(&name).ok_or_else(|| format!("--light-skin {name}: not one of the game's LightColors swatches"))?),
        None => None,
    };
    // --phase01 F: an explicit animation phase (0..1 of the period) written into
    // the SInstanceParams of every constrained moving part (pack pushers say
    // -1 = unset there); the 2026-09-08 probe of whether an embedded kinematic
    // dyna honours its own Phase01 (the map's per-placement AnimPhaseOffset
    // byte, chunk 0x03043063, does nothing for an embedded item)
    if let Some(p) = flag(rest, "--phase01") {
        let p: f32 = p.parse().map_err(|e| format!("--phase01: {e}"))?;
        build::DYNA_PHASE01.with(|o| o.set(Some(p)));
    }
    let is_file = std::path::Path::new(&src).is_file();
    let (bytes, merged) = if is_file {
        let data = std::fs::read(&src).map_err(|e| format!("{src}: {e}"))?;
        build::static_item_from_item_report(&data, &ident, &author, scale, collection)?
    } else if src.to_ascii_lowercase().ends_with(".item.gbx") {
        // a pack ITEM: baked through its external prefab / static-object files
        let mut store = open();
        build::static_item_from_pack_item_report_skin(&mut store, &src, &ident, &author, scale, collection, variant, light_skin)?
    } else {
        let mut store = open();
        build::static_item_from_prefab_report(&mut store, &src, &ident, &author, scale, collection)?
    };
    std::fs::write(&out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    // the side files the item names (its `.Light.Gbx` copies, sign logos):
    // next to the output, as they ride next to the item in a library archive
    for (name, data) in &merged.pictures {
        let p = std::path::Path::new(&out).with_file_name(name.replace('\\', "/"));
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        std::fs::write(&p, data).map_err(|e| format!("{}: {e}", p.display()))?;
        println!("  side file {} ({} bytes)", p.display(), data.len());
    }
    // the sidecar node files of the `TINY_FLAG_REF=file` form (the dyna
    // object and its mesh as files the item names by bare name)
    let sidecars = super::assemble::SIDECARS.with(|s| std::mem::take(&mut *s.borrow_mut()));
    for (name, data) in &sidecars {
        let p = std::path::Path::new(&out).with_file_name(name);
        std::fs::write(&p, data).map_err(|e| format!("{}: {e}", p.display()))?;
        println!("  sidecar {} ({} bytes)", p.display(), data.len());
    }
    for n in &merged.notes {
        println!("  note: {n}");
    }
    // Verify with our own parser.
    let f = super::parse_file(&bytes).map_err(|e| format!("output does not parse back: {e}"))?;
    if super::write_file(&f) != bytes {
        return Err("output does not round-trip through the parser".into());
    }
    if let Some(p) = f.item.prefab() {
        println!("wrote {out}: {} bytes, {} nodes, prefab entity model with {} entities:", bytes.len(), f.num_nodes, p.ents.len());
        for (i, e) in p.ents.iter().enumerate() {
            let what = match e.model.inline.as_deref() {
                Some(super::Node::Dyna(d)) => {
                    let s2 = match d.mesh.inline.as_deref() {
                        Some(super::Node::Solid2(s)) => format!("{} visuals, {} materials", s.visuals.len(), s.custom_materials.len()),
                        _ => "no inline mesh".into(),
                    };
                    let hull = |r: &super::Ref| match r.inline.as_deref() {
                        Some(super::Node::Surface(s)) => {
                            let (v, t) = s.surf.counts();
                            format!("type {} {v}v/{t}f ids {:?}", s.surf.type_id(), s.material_ids)
                        }
                        _ => "none".into(),
                    };
                    format!("CPlugDynaObjectModel: {s2}, move shape {}, hit shape {}", hull(&d.dyna_shape), hull(&d.static_shape))
                }
                Some(super::Node::StaticObject(so)) => format!(
                    "CPlugStaticObjectModel: {} visuals, collision {:?}",
                    so.solid2().map(|s| s.visuals.len()).unwrap_or(0),
                    so.surface().map(|s| match &s.surf {
                        super::surface::Surf::Mesh { triangles, vertices, .. } => (vertices.len(), triangles.len()),
                        _ => (0, 0),
                    })
                ),
                Some(super::Node::Kinematic(k)) => format!("NPlugDyna_SKinematicConstraint: {} (params {:?})", k.summary(), super::dyna::ConstraintParams::parse(&e.params).map(|c| (c.ent1, c.ent2))),
                Some(other) => format!("class 0x{:08X}", other.class_id()),
                None => format!("external node {}", e.model.index),
            };
            println!("  entity {i} at {:?} rot {:?}: {what}", e.pos, e.rot);
        }
        return Ok(());
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

/// `mapgeom fx-dump [--check] FILE…`: a `.FxSys.Gbx` or `.ParticleModel.Gbx`
/// (pulled out of a pack with `extract`) parsed by the typed particle
/// reader and described; `--check` re-serialises it and demands the body
/// come back byte-identical (the proof the reader can move these nodes into
/// an item). Exits non-zero on the first file that fails.
pub fn fx_dump(rest: &[String]) -> Result<(), String> {
    let check = rest.iter().any(|a| a == "--check");
    let files: Vec<&String> = rest.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        return Err("fx-dump [--check] FILE…".into());
    }
    let mut failed = 0;
    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let g = tmmaps::gbx::Gbx::parse(&data);
        let externals = super::file::ref_table_nodes(&g.ref_table);
        let mut lb = super::LookbackState::default();
        lb.defined_nodes.extend(externals.iter().copied());
        let mut r = super::Rd::new(&g.body, 0, lb);
        let parsed = super::read_node(&mut r, g.class_id);
        let node = match parsed {
            Ok(n) => n,
            Err(e) => {
                println!("{path}: class 0x{:08X}: FAILED at 0x{:x} of {} body bytes: {e}", g.class_id, r.o, g.body.len());
                failed += 1;
                continue;
            }
        };
        let trailing = g.body.len() - r.o;
        println!("{path}: class 0x{:08X}, {} nodes, {} body bytes{}", g.class_id, g.num_nodes, g.body.len(), if trailing > 0 { format!(", {trailing} TRAILING BYTES") } else { String::new() });
        match &node {
            super::Node::FxSystem(fx) => print!("{}", fx.describe()),
            super::Node::Particle(p) => {
                let mut s = String::new();
                p.describe(1, &mut s);
                print!("{s}");
            }
            other => println!("  (a {:?} node, not a particle class)", other.class_id()),
        }
        if check {
            let mut out = Vec::new();
            let mut lb2 = super::LookbackState::default();
            lb2.defined_nodes.extend(externals.iter().copied());
            let mut w = super::Wr { w: &mut out, lb: &mut lb2 };
            super::write_node(&mut w, &node);
            if out == g.body {
                println!("  round trip: IDENTICAL ({} bytes)", out.len());
            } else {
                let first = out.iter().zip(g.body.iter()).position(|(a, b)| a != b).unwrap_or(out.len().min(g.body.len()));
                println!("  round trip: DIFFERS at 0x{first:x} ({} written vs {} read)", out.len(), g.body.len());
                failed += 1;
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} file(s) failed"));
    }
    Ok(())
}
