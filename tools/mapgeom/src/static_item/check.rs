//! `mapgeom item-check [--packs …] FILE.Item.Gbx…`: offline structural
//! validation of static items — everything the game is known to trip over,
//! checked in milliseconds instead of a two-minute load:
//!   * every index < vertex count, every stream element sized to the count,
//!     tangent arrays sized to the count, no empty visual / geom
//!   * shaded geoms sorted by material, every material index in range
//!   * every material used by some geom, no duplicate (link, physics) slot
//!   * every material link resolvable as `<link>.Material.Gbx` in the packs
//!   * collision: every triangle index < vertex count, surface index in range
//! With `--facts` it also prints one line per visual / material (the
//! structural profile used to diff a crashing item against a loading one).

use crate::store::DataStore;

pub fn run(rest: &[String], open: &mut dyn FnMut() -> DataStore) -> Result<(), String> {
    let facts = rest.iter().any(|a| a == "--facts");
    let files: Vec<&String> = rest.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        return Err("item-check [--facts] FILE.Item.Gbx…".into());
    }
    let mut store: Option<DataStore> = None;
    let mut bad = 0usize;
    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let f = match super::parse_file(&data) {
            Ok(f) => f,
            Err(e) => {
                println!("{path}: FAIL does not parse: {e}");
                bad += 1;
                continue;
            }
        };
        let mut problems: Vec<String> = Vec::new();
        // The item's waypoint type and trigger shape: what a checkpoint /
        // finish fires on, or a gameplay gate's effect volume (the id table
        // carries physics | gameplay << 8; a special gate's gameplay is the
        // effect — 18 ReactorBoost_Oriented, 4 FreeWheeling, 8 Reset, 1 Turbo).
        if facts {
            let wt = f.item.chunks.iter().find_map(|c| match c {
                super::item::ItemChunk::Waypoint { waypoint_type, .. } => Some(*waypoint_type),
                _ => None,
            });
            let trig = f.item.model().and_then(|mc| mc.entity_model()).and_then(|e| match e.trigger_shape.inline.as_deref() {
                Some(super::Node::Surface(s)) => Some(s),
                _ => None,
            });
            // a gameplay gate in prefab form: its NPlugTrigger_SGateSpecial entity's shape
            if let Some(p) = f.item.prefab() {
                for (ei, e) in p.ents.iter().enumerate() {
                    if let Some(super::Node::GateSpecial(g)) = e.model.inline.as_deref() {
                        match g.shape.inline.as_deref() {
                            Some(super::Node::Surface(sf)) => {
                                let ids: Vec<String> = sf.material_ids.iter().map(|x| format!("{x} (phys {} gp {})", x & 0xff, x >> 8)).collect();
                                let (nv, nt) = sf.surf.counts();
                                println!("{path}: prefab entity {ei} special trigger v{} {nv} vertices {nt} triangles ids [{}] main dir {:?}", g.version, ids.join(", "), sf.gameplay_main_dir);
                            }
                            _ => println!("{path}: prefab entity {ei} special trigger v{} with shape node {} (not inline)", g.version, g.shape.index),
                        }
                    }
                }
            }
            match trig {
                Some(sf) => {
                    let ids: Vec<String> = sf.material_ids.iter().map(|x| format!("{x} (phys {} gp {})", x & 0xff, x >> 8)).collect();
                    match &sf.surf {
                        super::surface::Surf::Mesh { vertices, triangles, .. } => {
                            let mut lo = [f32::MAX; 3];
                            let mut hi = [f32::MIN; 3];
                            for v in vertices {
                                for k in 0..3 {
                                    lo[k] = lo[k].min(v[k]);
                                    hi[k] = hi[k].max(v[k]);
                                }
                            }
                            let mut bytes: std::collections::BTreeMap<(u8, u8), usize> = Default::default();
                            for t in triangles {
                                *bytes.entry((t.material_id, t.u03)).or_default() += 1;
                            }
                            println!("{path}: waypoint type {:?} trigger {} vertices {} triangles bounds [{:.2}, {:.2}, {:.2}]..[{:.2}, {:.2}, {:.2}] ids [{}] tri (phys, gp) {:?} main dir {:?} materials {}", wt, vertices.len(), triangles.len(), lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], ids.join(", "), bytes, sf.gameplay_main_dir, sf.materials.len());
                        }
                        other => println!("{path}: waypoint type {:?} trigger surf type {} ids [{}]", wt, other.type_id(), ids.join(", ")),
                    }
                }
                None => println!("{path}: waypoint type {:?} no trigger shape", wt),
            }
        }
        // The solids to check: a static item's one, or every entity of a
        // moving item's prefab (each dyna part's mesh, the static part's).
        let mut parts: Vec<(String, &super::solid2::CPlugSolid2Model, Option<&super::surface::CPlugSurface>)> = Vec::new();
        // indices into `parts` whose missing hull is fine (tween parts)
        let mut hull_optional: Vec<usize> = Vec::new();
        if let Some(so) = f.item.static_object() {
            match so.solid2() {
                Some(s2) => {
                    // a mesh-collidable static object (the modeler's items:
                    // `is_mesh_collidable`, no shape) collides on its visual
                    // mesh — no separate hull to check
                    if so.is_mesh_collidable && so.surface().is_none() {
                        hull_optional.push(parts.len());
                        if facts {
                            println!("{path}: collision is the visual mesh (is_mesh_collidable, no shape)");
                        }
                    }
                    parts.push((String::new(), s2, so.surface()))
                }
                None => {
                    println!("{path}: FAIL no solid2");
                    bad += 1;
                    continue;
                }
            }
        } else if let Some(p) = f.item.prefab() {
            let mut constraints = 0usize;
            let mut tween_parts = 0usize;
            for (i, e) in p.ents.iter().enumerate() {
                match e.model.inline.as_deref() {
                    Some(super::Node::Dyna(d)) => match d.mesh.inline.as_deref() {
                        Some(super::Node::Solid2(s2)) => {
                            fn hull(r: &super::Ref) -> Option<&super::surface::CPlugSurface> {
                                match r.inline.as_deref() {
                                    Some(super::Node::Surface(s)) => Some(s),
                                    _ => None,
                                }
                            }
                            // A self-animating part (the flag cloth: every visual carries a
                            // frame table, 0x09006005, for its vertex-tween material) rides
                            // without a constraint and without a hull, as in the pack
                            // (Flag.DynaObject.Gbx: both shape refs null; measured drawing and
                            // waving in a Summer 15 lineup, 2026-09-07).
                            let is_tween = !s2.visuals.is_empty() && s2.visuals.iter().all(|vr| matches!(vr.inline.as_deref(), Some(super::Node::Visual(v)) if !v.sub_visuals.is_empty()));
                            if is_tween {
                                tween_parts += 1;
                                let hull = hull(&d.static_shape).or_else(|| hull(&d.dyna_shape));
                                if hull.is_none() {
                                    hull_optional.push(parts.len());
                                }
                                parts.push((format!("entity {i} (tween): "), s2, hull));
                            } else {
                                parts.push((format!("entity {i} (moving): "), s2, hull(&d.static_shape).or_else(|| hull(&d.dyna_shape))));
                            }
                        }
                        _ => problems.push(format!("entity {i}: moving part without an inline mesh")),
                    },
                    Some(super::Node::StaticObject(so)) => match so.solid2() {
                        Some(s2) => parts.push((format!("entity {i}: "), s2, so.surface())),
                        None => problems.push(format!("entity {i}: static object without an inline solid")),
                    },
                    Some(super::Node::Kinematic(k)) => {
                        constraints += 1;
                        match super::dyna::ConstraintParams::parse(&e.params) {
                            Some(c) => {
                                let target = if c.ent2 >= 0 { c.ent2 } else { c.ent1 };
                                if !matches!(p.ents.get(target as usize).and_then(|t| t.model.inline.as_deref()), Some(super::Node::Dyna(_))) {
                                    problems.push(format!("entity {i}: constraint binds entity {target}, which is not a moving part"));
                                }
                                if facts {
                                    println!("{path}: entity {i} constraint on entity {target}: {}", k.summary());
                                }
                            }
                            None => problems.push(format!("entity {i}: constraint with {}-byte params", e.params.len())),
                        }
                    }
                    // a gameplay gate's effect volume (NPlugTrigger_SGateSpecial,
                    // 3f5da2a): its shape is reported above under --facts; the
                    // entity itself is the pack's own layout, nothing to check
                    Some(super::Node::GateSpecial(g)) => {
                        if !matches!(g.shape.inline.as_deref(), Some(super::Node::Surface(_))) {
                            problems.push(format!("entity {i}: gate special trigger without an inline shape (node {})", g.shape.index));
                        }
                    }
                    // an effect system (the Show items' smoke / sparks): an entity
                    // of its own, no mesh; its emitters must name inline models
                    Some(super::Node::FxSystem(fx)) => {
                        let emitters = fx.root.emitters();
                        for em in &emitters {
                            if em.model.index >= 0 && em.model.inline.is_none() && !emitters.iter().any(|o| o.model.index == em.model.index && o.model.inline.is_some()) {
                                problems.push(format!("entity {i}: emitter {:?} names model node {} which is not inline", em.name.as_str().unwrap_or(""), em.model.index));
                            }
                        }
                        if facts {
                            println!("{path}: entity {i} effect system at {:?}: {} emitter(s)", e.pos, emitters.len());
                            print!("{}", fx.describe());
                        }
                    }
                    Some(other) => problems.push(format!("entity {i}: class 0x{:08X} in the prefab", other.class_id())),
                    None => problems.push(format!("entity {i}: external model node {}", e.model.index)),
                }
            }
            let moving = p.ents.iter().filter(|e| matches!(e.model.inline.as_deref(), Some(super::Node::Dyna(_)))).count() - tween_parts;
            if constraints != moving {
                problems.push(format!("{moving} moving parts but {constraints} constraints"));
            }
            if parts.is_empty() {
                println!("{path}: FAIL prefab with no solid");
                bad += 1;
                continue;
            }
        } else {
            println!("{path}: FAIL no static object (nor a prefab entity model)");
            bad += 1;
            continue;
        }
        let nparts = parts.len();
        let mut total_visuals = 0usize;
        let mut total_mats = 0usize;
        for (part_index, (label, s2, surface)) in parts.into_iter().enumerate() {
        let p0 = problems.len();
        total_visuals += s2.visuals.len();
        total_mats += s2.custom_materials.len();
        let nmat = s2.custom_materials.len();
        let mut used = vec![0usize; nmat];
        if s2.shaded_geoms.is_empty() {
            problems.push("no shaded geoms".into());
        }
        // The detail ladder: `lod_max_dist` has one switch distance per level
        // but the last, so `levels` levels; every geom's mask must name only
        // those, every level must be drawn by something (else the item
        // vanishes at that distance), the distances must climb.
        let levels = s2.lod_max_dist.len() as u32 + 1;
        let all_levels: u32 = if levels >= 32 { u32::MAX } else { (1u32 << levels) - 1 };
        let mut level_geoms = vec![0usize; levels as usize];
        let mut level_verts = vec![0usize; levels as usize];
        for (k, d) in s2.lod_max_dist.iter().enumerate() {
            if !(*d > 0.0) || (k > 0 && *d < s2.lod_max_dist[k - 1]) {
                problems.push(format!("lod_max_dist {:?}: level {k} distance {d} not positive and non-decreasing", s2.lod_max_dist));
            }
        }
        let level_of = |mask: i32| -> u32 { if mask <= 0 { 0 } else { (mask as u32).trailing_zeros() } };
        // Order: level-major with the materials sorted inside a level (the
        // pack prefabs' layout, ours since 2026-09-07), or material-major with
        // the levels sorted inside a material (`TINY_LOD_ORDER=material`);
        // a one-level item is material-sorted either way (SH rule: the first
        // unsorted split items crashed the client reading a garbage index).
        let keys: Vec<(u32, i32)> = s2.shaded_geoms.iter().map(|g| (level_of(g.lod_mask), g.material_index)).collect();
        let level_major = keys.windows(2).all(|w| w[0] <= w[1]);
        let material_major = keys.windows(2).all(|w| (w[0].1, w[0].0) <= (w[1].1, w[1].0));
        if !level_major && !material_major {
            let first_bad = keys.windows(2).position(|w| w[0] > w[1]).unwrap_or(0) + 1;
            problems.push(format!("geom {first_bad}: (level, material) {:?} after {:?} — geoms sorted neither level-major nor material-major", keys[first_bad], keys[first_bad - 1]));
        }
        for (gi, g) in s2.shaded_geoms.iter().enumerate() {
            if g.material_index < 0 || g.material_index as usize >= nmat {
                problems.push(format!("geom {gi}: material index {} of {nmat}", g.material_index));
            } else {
                used[g.material_index as usize] += 1;
            }
            if g.visual_index < 0 || g.visual_index as usize >= s2.visuals.len() {
                problems.push(format!("geom {gi}: visual index {} of {}", g.visual_index, s2.visuals.len()));
            }
            if g.lod_mask <= 0 || (g.lod_mask as u32) & !all_levels != 0 {
                problems.push(format!("geom {gi}: lod mask {:#x} outside the {levels}-level ladder {:?}", g.lod_mask, s2.lod_max_dist));
            }
            let nverts = s2.visuals.get(g.visual_index.max(0) as usize).and_then(|r| r.inline.as_deref()).and_then(|n| match n { super::Node::Visual(v) => v.main.as_ref().map(|m| m.count.max(0) as usize), _ => None }).unwrap_or(0);
            for k in 0..levels {
                if g.lod_mask > 0 && (g.lod_mask as u32) & (1 << k) != 0 {
                    level_geoms[k as usize] += 1;
                    level_verts[k as usize] += nverts;
                }
            }
        }
        // An empty LAST level is Nadeo's cull idiom (Sparkler8m: distances
        // [16, 128, 256] but masks 1/2/4 only — the sparkler is not drawn
        // past 256 m; 2 of the 93 laddered pack models surveyed 2026-09-07 do
        // this); an empty level BEFORE a drawn one would make the item blink
        // out and back in with distance.
        let last_drawn = level_geoms.iter().rposition(|n| *n > 0);
        for (k, n) in level_geoms.iter().enumerate() {
            if *n == 0 && last_drawn.map(|l| k < l).unwrap_or(false) {
                problems.push(format!("detail level {k} of {levels} is drawn by no geom while level {} is (the item blinks out between {} and {} m)", last_drawn.unwrap_or(0), s2.lod_max_dist.get(k.saturating_sub(1)).copied().unwrap_or(0.0), s2.lod_max_dist.get(k).copied().unwrap_or(f32::INFINITY)));
            }
        }
        if facts {
            let cull = match last_drawn {
                Some(l) if l + 1 < levels as usize => format!(", culled beyond {} m", s2.lod_max_dist.get(l).copied().unwrap_or(0.0)),
                _ => String::new(),
            };
            println!("{path}: {levels} detail level(s), switch distances {:?}, geoms per level {:?}, vertices per level {:?}{cull}", s2.lod_max_dist, level_geoms, level_verts);
        }
        for (mi, n) in used.iter().enumerate() {
            if *n == 0 {
                problems.push(format!("material {mi} used by no geom"));
            }
        }
        // Two slots are duplicates when they draw the same (`same_look`: link,
        // physics and every constant, names aside). A mesh-modeler item
        // (Summer 21's TME nation items) legitimately carries one game
        // material many times with a different `TargetColor` per part.
        let mut seen: Vec<&crate::crystal_model::CPlugMaterialUserInst> = Vec::new();
        for (mi, m) in s2.custom_materials.iter().enumerate() {
            let Some(inst) = m.inst() else {
                problems.push(format!("material {mi}: not a CPlugMaterialUserInst"));
                continue;
            };
            let link = inst.link().unwrap_or("").to_string();
            // A custom-texture material (2026-09-07, the baked screen pictures):
            // no game link, a shading model and textures named by file — the
            // files must ride in the same archive folder as the item, which
            // the check cannot see; it requires the form to be complete.
            let custom = inst.main.as_ref().filter(|m| !m.is_using_game_material && link.is_empty()).map(|m| (m.model.clone(), m.user_textures.clone()));
            if let Some(prev) = seen.iter().find(|p| super::build::same_look(p, inst)) {
                let what = prev.link().filter(|l| !l.is_empty()).map(|l| l.to_string()).unwrap_or_else(|| "custom-texture material".into());
                problems.push(format!("material {mi}: duplicate slot {what} ({})", inst.physics()));
            }
            seen.push(inst);
            if let Some((model, textures)) = &custom {
                let model_name = match model {
                    crate::crystal_model::Id::Str(s) => s.clone(),
                    _ => String::new(),
                };
                if model_name.is_empty() {
                    problems.push(format!("material {mi}: custom-texture material without a shading model"));
                }
                if textures.is_empty() {
                    problems.push(format!("material {mi}: custom-texture material without textures"));
                }
                for t in textures {
                    if !t.texture.to_ascii_lowercase().ends_with(".dds") || t.texture.contains('\\') || t.texture.contains('/') {
                        problems.push(format!("material {mi}: texture {:?} is not a bare .dds file name (the game resolves only that, next to the item)", t.texture));
                    }
                }
                if facts {
                    let anims = if textures.is_empty() { String::new() } else { inst.main.as_ref().map(|m| if m.uv_anims.is_empty() { String::new() } else { format!(" uvanims [{}]", m.uv_anims.iter().map(|a| format!("{:?}/{:?}/{}/{:#x}/{:?}", a.u01, a.u02, a.u03, a.u04, a.u05)).collect::<Vec<_>>().join(", ")) }).unwrap_or_default() };
                    println!("{path}: material {mi} custom {model_name} textures [{}] phys {} used by {} geoms{anims}", textures.iter().map(|t| format!("{}={}", t.u01, t.texture)).collect::<Vec<_>>().join(" "), inst.physics(), used.get(mi).copied().unwrap_or(0));
                }
                continue;
            }
            if link.is_empty() {
                problems.push(format!("material {mi}: empty link"));
            } else {
                let st = store.get_or_insert_with(|| open());
                // A modeler material (`is_using_game_material` false) names its
                // file relative to the Solid2's `materials_folder`
                // (`TechnicsTrims` under `Stadium\Media\Material\`).
                let modeler = inst.main.as_ref().map(|m| !m.is_using_game_material).unwrap_or(false) && !link.contains('\\');
                let file = if modeler {
                    let folder = if s2.materials_folder.is_empty() { "Stadium\\Media\\Material\\" } else { s2.materials_folder.as_str() };
                    format!("{}{link}.Material.Gbx", folder.strip_suffix('\\').map(|f| format!("{f}\\")).unwrap_or_else(|| folder.to_string()))
                } else {
                    format!("{link}.Material.Gbx")
                };
                if st.resolve(&file).is_none() {
                    problems.push(format!("material {mi}: link {link} resolves to no .Material.Gbx in the packs{}", if modeler { format!(" (modeler material, tried {file})") } else { String::new() }));
                }
            }
            if facts {
                let extra = inst
                    .main
                    .as_ref()
                    .map(|m| {
                        let mut s = String::new();
                        if !m.is_using_game_material {
                            s.push_str(" modeler");
                        }
                        if !m.uv_anims.is_empty() {
                            s.push_str(&format!(" uvanims [{}]", m.uv_anims.iter().map(|a| format!("{:?}/{:?}/{}/{:#x}/{:?}", a.u01, a.u02, a.u03, a.u04, a.u05)).collect::<Vec<_>>().join(", ")));
                        }
                        // constants: `TargetColor Real 3` + three f32 bits in `color`
                        for c in &m.csts {
                            s.push_str(&format!(" cst {}:{}x{}", c.u01.as_str().unwrap_or("?"), c.u02.as_str().unwrap_or("?"), c.u03));
                        }
                        if !m.color.is_empty() {
                            s.push_str(&format!(" color [{}]", m.color.iter().map(|v| format!("{:.3}", f32::from_bits(*v as u32))).collect::<Vec<_>>().join(", ")));
                        }
                        if !m.user_textures.is_empty() {
                            s.push_str(&format!(" textures [{}]", m.user_textures.iter().map(|t| format!("{}={}", t.u01, t.texture)).collect::<Vec<_>>().join(" ")));
                        }
                        s
                    })
                    .unwrap_or_default();
                println!("{path}: material {mi} {link} phys {} used by {} geoms{extra}", inst.physics(), used.get(mi).copied().unwrap_or(0));
            }
        }
        for (vi, vr) in s2.visuals.iter().enumerate() {
            let Some(super::Node::Visual(v)) = vr.inline.as_deref() else {
                problems.push(format!("visual {vi}: not an inline visual"));
                continue;
            };
            let Some(m) = v.main.as_ref() else {
                problems.push(format!("visual {vi}: no chunk 0x0900600F"));
                continue;
            };
            let count = m.count.max(0) as usize;
            if count == 0 {
                problems.push(format!("visual {vi}: 0 vertices"));
            }
            let per = (((!(m.flags() >> 17)) & 8) | 4) as usize;
            let mut decl_desc = String::new();
            match v.stream() {
                Some(s) => {
                    if s.count.max(0) as usize != count {
                        problems.push(format!("visual {vi}: stream count {} != visual count {count}", s.count));
                    }
                    let compress = s.compress_local3d.unwrap_or(false);
                    for (d, e) in s.decls.iter().zip(s.elems.iter()) {
                        if e.len() != count {
                            problems.push(format!("visual {vi}: element name{} has {} entries for {count} vertices", d.name(), e.len()));
                        }
                        decl_desc.push_str(&format!(" n{}t{}", d.name(), d.stored_type(compress)));
                        if facts && rest.iter().any(|a| a == "--decls") {
                            decl_desc.push_str(&format!("[{:x}/{:x}/{:?}]", d.flags1, d.flags2, d.extra));
                        }
                        if facts && rest.iter().any(|a| a == "--values") {
                            decl_desc.push_str(&elem_summary(e));
                            if let (5 | 18 | 20, super::vstream::Elem::Word(w)) = (d.name(), e) {
                                let mut lens: Vec<f32> = w.iter().map(|x| { let n = super::build::dec3n_unpack(*x); (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() }).collect();
                                lens.sort_by(|a, b| a.partial_cmp(b).unwrap());
                                let first: Vec<String> = w.iter().take(3).map(|x| { let n = super::build::dec3n_unpack(*x); format!("[{:.2},{:.2},{:.2}]", n[0], n[1], n[2]) }).collect();
                                decl_desc.push_str(&format!("<len {:.2}..{:.2} first {}>", lens.first().copied().unwrap_or(0.0), lens.last().copied().unwrap_or(0.0), first.join("")));
                            }
                        }
                    }
                    if s.decls.len() != s.elems.len() {
                        problems.push(format!("visual {vi}: {} decls but {} element arrays", s.decls.len(), s.elems.len()));
                    }
                }
                None => problems.push(format!("visual {vi}: no inline vertex stream")),
            }
            if let Some((a, b)) = v.tangents.as_ref() {
                for t in [a, b] {
                    if !t.is_empty() && t.len() != count * per {
                        problems.push(format!("visual {vi}: tangent array {} bytes != {count} x {per}", t.len()));
                    }
                }
            }
            let ntri = match v.index_buffer.as_ref() {
                Some(ib) => {
                    if ib.indices.is_empty() {
                        problems.push(format!("visual {vi}: empty index buffer"));
                    }
                    if ib.indices.len() % 3 != 0 {
                        problems.push(format!("visual {vi}: {} indices is not a triangle list", ib.indices.len()));
                    }
                    if let Some(mx) = ib.indices.iter().max() {
                        if *mx as usize >= count {
                            problems.push(format!("visual {vi}: index {mx} past {count} vertices"));
                        }
                    }
                    let mut seen = vec![false; count];
                    for i in &ib.indices {
                        if let Some(s) = seen.get_mut(*i as usize) {
                            *s = true;
                        }
                    }
                    let unused = seen.iter().filter(|s| !**s).count();
                    if unused > 0 && facts {
                        println!("{path}: visual {vi}: {unused} of {count} vertices referenced by no triangle");
                    }
                    ib.indices.len() / 3
                }
                None => {
                    problems.push(format!("visual {vi}: no index buffer"));
                    0
                }
            };
            if facts && rest.iter().any(|a| a == "--uvhist") {
                if let Some(s) = v.stream() {
                    let pos = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 0).map(|(_, e)| e) {
                        Some(super::vstream::Elem::Float3(p)) => p.clone(),
                        _ => Vec::new(),
                    };
                    let uv = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 10).map(|(_, e)| e) {
                        Some(super::vstream::Elem::Float2(p)) => p.clone(),
                        _ => Vec::new(),
                    };
                    if let (Some(ib), false, false) = (v.index_buffer.as_ref(), pos.is_empty(), uv.is_empty()) {
                        let nrm: Vec<[f32; 3]> = match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == 5).map(|(_, e)| e) {
                            Some(super::vstream::Elem::Word(w)) => w.iter().map(|x| super::build::dec3n_unpack(*x)).collect(),
                            _ => Vec::new(),
                        };
                        let (mut agree, mut disagree) = (0usize, 0usize);
                        if nrm.len() == pos.len() {
                            for t in ib.indices.chunks(3) {
                                if t.len() < 3 { continue; }
                                let [a, b, c] = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                                let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                                let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                                let vn = [0, 1, 2].map(|k| nrm[t[0] as usize][k] + nrm[t[1] as usize][k] + nrm[t[2] as usize][k]);
                                if n[0] * vn[0] + n[1] * vn[1] + n[2] * vn[2] >= 0.0 { agree += 1 } else { disagree += 1 }
                            }
                        }
                        println!("{path}: visual {vi} winding vs vertex normals: {agree} agree, {disagree} disagree");
                        let tan = |name: u32| -> Vec<[f32; 3]> { match s.decls.iter().zip(s.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e) { Some(super::vstream::Elem::Word(w)) => w.iter().map(|x| super::build::dec3n_unpack(*x)).collect(), _ => Vec::new() } };
                        let (tu, tv) = (tan(18), tan(20));
                        if tu.len() == nrm.len() && tv.len() == nrm.len() {
                            let (mut pos_h, mut neg_h, mut perp) = (0usize, 0usize, 0usize);
                            for i in 0..nrm.len() {
                                let c = [tu[i][1] * tv[i][2] - tu[i][2] * tv[i][1], tu[i][2] * tv[i][0] - tu[i][0] * tv[i][2], tu[i][0] * tv[i][1] - tu[i][1] * tv[i][0]];
                                let d = c[0] * nrm[i][0] + c[1] * nrm[i][1] + c[2] * nrm[i][2];
                                let un = tu[i][0] * nrm[i][0] + tu[i][1] * nrm[i][1] + tu[i][2] * nrm[i][2];
                                if un.abs() > 0.3 { perp += 1 }
                                if d > 0.0 { pos_h += 1 } else { neg_h += 1 }
                            }
                            println!("{path}: visual {vi} tangent frame: cross(tu,tv).n > 0 for {pos_h}, < 0 for {neg_h}; tu not perpendicular to n: {perp}");
                        }
                        // (uv cell 1/20, dominant normal axis) -> area
                        let mut cells: std::collections::BTreeMap<(i32, i32, &'static str), f32> = Default::default();
                        for t in ib.indices.chunks(3) {
                            if t.len() < 3 {
                                continue;
                            }
                            let [a, b, c] = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                            let area = 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                            let ax = if n[0].abs() >= n[1].abs() && n[0].abs() >= n[2].abs() {
                                if n[0] > 0.0 { "+x" } else { "-x" }
                            } else if n[1].abs() >= n[2].abs() {
                                if n[1] > 0.0 { "+y" } else { "-y" }
                            } else if n[2] > 0.0 { "+z" } else { "-z" };
                            let cu = (uv[t[0] as usize][0] + uv[t[1] as usize][0] + uv[t[2] as usize][0]) / 3.0;
                            let cv = (uv[t[0] as usize][1] + uv[t[1] as usize][1] + uv[t[2] as usize][1]) / 3.0;
                            *cells.entry(((cu * 20.0).floor() as i32, (cv * 20.0).floor() as i32, ax)).or_default() += area;
                        }
                        let mut per_axis: std::collections::BTreeMap<&str, f32> = Default::default();
                        for ((_, _, ax), a) in cells.iter() {
                            *per_axis.entry(ax).or_default() += a;
                        }
                        println!("{path}: visual {vi} area by normal axis {per_axis:?}");
                        let mut rows: Vec<_> = cells.into_iter().collect();
                        rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                        for ((cu, cv, ax), area) in rows.iter().take(12) {
                            println!("{path}: visual {vi} uv0 cell u{:.2} v{:.2} {ax} area {area:.3}", *cu as f32 / 20.0, *cv as f32 / 20.0);
                        }
                    }
                }
            }
            if facts {
                let mat = s2.shaded_geoms.iter().find(|g| g.visual_index as usize == vi).map(|g| g.material_index).unwrap_or(-1);
                println!("{path}: visual {vi} mat {mat} verts {count} tris {ntri} decls{decl_desc} cflags {:x} sflags {:x} u03 {} tangents {:?} bbox {:?}", m.chunk_flags, v.stream().map(|s| s.flags).unwrap_or(0), m.u03, v.tangents.as_ref().map(|(a, b)| (a.len(), b.len())), m.bounding_box);
            }
        }
        if let Some(sf) = surface {
            if let super::surface::Surf::Mesh { vertices, triangles, .. } = &sf.surf {
                for (ti, t) in triangles.iter().enumerate() {
                    if t.indices.iter().any(|i| *i as usize >= vertices.len()) {
                        problems.push(format!("collision triangle {ti}: index past {} vertices", vertices.len()));
                        break;
                    }
                    if t.surface_index < 0 || t.surface_index as usize >= sf.material_ids.len().max(1) {
                        problems.push(format!("collision triangle {ti}: surface index {} of {}", t.surface_index, sf.material_ids.len()));
                        break;
                    }
                }
                if facts {
                    println!("{path}: {label}collision {} vertices {} triangles physics {:?}", vertices.len(), triangles.len(), sf.material_ids);
                    // Per-triangle physics, two ways: the triangle's own u8 and
                    // the surface's material table indexed by the triangle's
                    // surface index. They disagree on some Nadeo hulls (road
                    // tops read 9/Rubber as the byte and 16/Asphalt through the
                    // table — reported by the route project, 2026-09-07), and
                    // the game believes the TABLE.
                    let mut pairs: std::collections::BTreeMap<(u8, i32), usize> = std::collections::BTreeMap::new();
                    for t in triangles {
                        let via_table = sf.material_ids.get(t.surface_index.max(0) as usize).map(|id| (id & 0xFF) as i32).unwrap_or(-1);
                        *pairs.entry((t.material_id, via_table)).or_default() += 1;
                    }
                    let mut rows: Vec<_> = pairs.into_iter().collect();
                    rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
                    for ((byte, table), n) in rows.into_iter().take(8) {
                        let flag = if table >= 0 && i32::from(byte) != table { "  <-- DISAGREE" } else { "" };
                        println!("{path}: {label}  {n} triangles: byte {byte}, table[idx] {table}{flag}");
                    }
                }
            } else if facts {
                let (v, t) = sf.surf.counts();
                println!("{path}: {label}collision surf type {} {v} vertices {t} faces physics {:?}", sf.surf.type_id(), sf.material_ids);
            }
        } else if !hull_optional.contains(&part_index) {
            problems.push("no collision surface".into());
        }
        for p in problems[p0..].iter_mut() {
            p.insert_str(0, &label);
        }
        }
        if problems.is_empty() {
            println!("{path}: ok ({total_visuals} visuals, {total_mats} materials{})", if nparts > 1 { format!(", {nparts} solids") } else { String::new() });
        } else {
            bad += 1;
            for p in &problems {
                println!("{path}: FAIL {p}");
            }
        }
    }
    if bad > 0 {
        return Err(format!("{bad} item(s) failed"));
    }
    Ok(())
}

/// One-line value summary of a vertex element: per-component min..max for
/// float elements, distinct count and first values for one-word ones.
fn elem_summary(e: &super::vstream::Elem) -> String {
    use super::vstream::Elem;
    fn range<const N: usize>(v: &[[f32; N]]) -> String {
        let mut lo = [f32::INFINITY; N];
        let mut hi = [f32::NEG_INFINITY; N];
        for x in v {
            for i in 0..N {
                lo[i] = lo[i].min(x[i]);
                hi[i] = hi[i].max(x[i]);
            }
        }
        (0..N).map(|i| format!("{:.3}..{:.3}", lo[i], hi[i])).collect::<Vec<_>>().join(",")
    }
    match e {
        Elem::Float2(v) => format!("{{{}}}", range(v)),
        Elem::Float3(v) => format!("{{{}}}", range(v)),
        Elem::Float4(v) => format!("{{{}}}", range(v)),
        Elem::Word(v) => {
            let mut d: Vec<u32> = v.clone();
            d.sort_unstable();
            d.dedup();
            let first: Vec<String> = d.iter().take(6).map(|x| format!("{x:08x}")).collect();
            format!("{{{} distinct: {}}}", d.len(), first.join(" "))
        }
        Elem::Raw { size, bytes } => format!("{{raw {size}x{}}}", bytes.len() / size.max(&1)),
    }
}
