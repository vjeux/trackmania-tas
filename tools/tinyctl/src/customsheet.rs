//! `tinyctl custom-sheet --census census.tsv --src-root DIR --out PREFIX [--tile 160] [--cols 10] [--rows 8]`
//!
//! Every distinct custom item of the census as a tile on contact sheets: the item
//! baked from the first map that embeds it (mapgeom's static-item bake), drawn
//! top-down by mapgeom's raster (colour = physics material), labelled with the
//! verdict and the file name. vjeux 2026-09-14 14:47: "use mapgeom to render all
//! of them in a few images" — so the convert/scenery classification can be
//! checked by eye.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::png::{self, Image};

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let census = PathBuf::from(f("--census").ok_or("custom-sheet needs --census census.tsv")?);
    let src_root = PathBuf::from(f("--src-root").ok_or("custom-sheet needs --src-root DIR")?);
    let out = f("--out").ok_or("custom-sheet needs --out PREFIX")?;
    let tile: usize = f("--tile").and_then(|s| s.parse().ok()).unwrap_or(160);
    let cols: usize = f("--cols").and_then(|s| s.parse().ok()).unwrap_or(10);
    let rows: usize = f("--rows").and_then(|s| s.parse().ok()).unwrap_or(8);
    // --textured: the VISUAL mesh coloured by material identity (a representative
    // colour per game material, the tint of a modeler Custom* material), lit from
    // the upper left, viewed from above-front (vjeux 2026-09-14 15:00: "items
    // displayed with their textures")
    let textured = tmmaps::cli::has(args, "--textured");
    // the game pack for the bitmaps (`--pak PATH:KEY`, the same as mapgeom's)
    let pak = f("--pak").unwrap_or_else(|| "/tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6".to_string());
    let (pak_path, pak_key) = pak.rsplit_once(':').map(|(p, k)| (p.to_string(), k.to_string())).ok_or("--pak wants PATH:KEY")?;
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak(&pak_path, &pak_key).map_err(|e| format!("pak: {e}"))?;
    let mut texcache: TexCache = BTreeMap::new();

    // census rows: ident (lowercased model), verdict; sorted CONVERT first
    let text = std::fs::read_to_string(&census).map_err(|e| format!("{}: {e}", census.display()))?;
    let mut items: Vec<(String, String, String)> = Vec::new(); // ident, verdict, note
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() >= 11 {
            items.push((c[0].to_string(), c[10].to_string(), c.get(12).unwrap_or(&"").to_string()));
        }
    }
    items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    // every source map's embedded files, loaded once per map on demand
    let mut maps: Vec<PathBuf> = Vec::new();
    for p in std::fs::read_dir(&src_root).map_err(|e| e.to_string())?.flatten() {
        if p.path().is_dir() {
            for q in std::fs::read_dir(p.path()).map_err(|e| e.to_string())?.flatten() {
                if q.path().extension().map(|e| e == "Gbx").unwrap_or(false) {
                    maps.push(q.path());
                }
            }
        }
    }
    maps.sort();
    let mut cache: BTreeMap<String, Vec<u8>> = BTreeMap::new(); // ident -> item bytes
    for mp in &maps {
        let m = tmmaps::map::MapFile::load(mp);
        let files = match mapgeom::embedded::files(&m) {
            Ok(f) if !f.is_empty() => f,
            _ => continue,
        };
        for (ident, _, _) in &items {
            if cache.contains_key(ident) {
                continue;
            }
            let want = format!("items\\{ident}");
            if let Some((_, bytes)) = files.iter().find(|(k, _)| {
                let k = k.replace('/', "\\").to_ascii_lowercase();
                k == want || k == *ident || k.ends_with(&format!("\\{want}"))
            }) {
                cache.insert(ident.clone(), bytes.clone());
            }
        }
    }

    let per_sheet = cols * rows;
    let mut sheet_no = 0;
    let mut written = Vec::new();
    for chunk in items.chunks(per_sheet) {
        sheet_no += 1;
        let mut img = Image::new(cols * tile, rows * (tile + 22));
        img.fill(0, 0, img.w, img.h, [24, 24, 28]);
        for (i, (ident, verdict, note)) in chunk.iter().enumerate() {
            let (cx, cy) = ((i % cols) * tile, (i / cols) * (tile + 22));
            let name = ident.rsplit(['\\', '/']).next().unwrap_or(ident).trim_end_matches(".item.gbx").to_string();
            let mut drawn = false;
            if let Some(bytes) = cache.get(ident) {
                if let Ok((_, merged)) = mapgeom::static_item::build::static_item_from_item_report(bytes, "X.Item.Gbx", "X.Item.Gbx", 1.0, 26) {
                    if textured {
                        if let Some(sub) = render_textured(&mut store, &mut texcache, &merged, tile - 8) {
                            let ox = cx + (tile - sub.w.min(tile)) / 2;
                            let oy = cy + (tile - sub.h.min(tile)) / 2;
                            img.blit(&sub, ox, oy);
                            drawn = true;
                        }
                    } else {
                        let mut scene = mapgeom::scene::Scene::default();
                        for t in &merged.surf_triangles {
                            let key = mapgeom::scene::physics_name(t.material_id).to_string();
                            let g = scene.groups.entry(key).or_default();
                            let base = g.verts.len() as u32;
                            for k in 0..3 {
                                g.verts.push(merged.surf_vertices[t.indices[k] as usize]);
                            }
                            g.tris.push([base, base + 1, base + 2]);
                        }
                        if scene.tri_count() > 0 {
                            let r = mapgeom::render::top_down(&scene, 64.0, tile - 8, f32::INFINITY);
                            let sub = Image { w: r.w, h: r.h, px: r.rgb.clone() };
                            let ox = cx + (tile - r.w.min(tile)) / 2;
                            let oy = cy + (tile - r.h.min(tile)) / 2;
                            img.blit(&sub, ox, oy);
                            drawn = true;
                        }
                    }
                }
            }
            if !drawn {
                let msg = if note.is_empty() { "NO GEOMETRY" } else if note.starts_with("unreadable") { "UNREADABLE" } else { "NO COLLISION" };
                png::label(&mut img, cx + 8, cy + tile / 2 - 4, msg, 1, [200, 120, 120]);
            }
            let col = match verdict.as_str() {
                "CONVERT" => [110, 230, 110],
                "scenery" => [230, 160, 90],
                _ => [230, 230, 90],
            };
            png::label(&mut img, cx + 2, cy + tile + 2, &verdict.to_ascii_uppercase(), 1, col);
            let short: String = name.chars().take((tile - 4) / 6).collect();
            png::label(&mut img, cx + 2, cy + tile + 12, &short, 1, [220, 220, 220]);
        }
        let path = format!("{out}-{sheet_no:02}.png");
        std::fs::write(&path, png::encode(&img)).map_err(|e| format!("{path}: {e}"))?;
        println!("{path}: {} items", chunk.len());
        written.push(path);
    }
    println!("{} sheets, {} items ({} with geometry)", written.len(), items.len(), cache.len());
    let _ = Path::new(&out);
    Ok(())
}


/// A representative colour for a game material by its link stem (what the
/// texture mostly looks like), or the TargetColor tint of a modeler material.
fn material_colour(mat: &mapgeom::crystal_model::CPlugMaterialUserInst) -> [f32; 3] {
    let link = mat.link().unwrap_or("");
    let stem = link.rsplit('\\').next().unwrap_or(link);
    if let Some(main) = mat.main.as_ref() {
        if main.color.len() >= 3 && main.csts.iter().any(|c| c.u01.as_str() == Some("TargetColor")) {
            let f = |i: usize| f32::from_bits(main.color[i] as u32).clamp(0.0, 1.0);
            return [f(0), f(1), f(2)];
        }
    }
    let s = stem.to_ascii_lowercase();
    let pick = |a: &[(&str, [f32; 3])]| a.iter().find(|(k, _)| s.contains(k)).map(|(_, c)| *c);
    pick(&[
        ("roadice", [0.75, 0.88, 0.97]), ("ice", [0.75, 0.88, 0.97]),
        ("roaddirt", [0.55, 0.38, 0.22]), ("dirt", [0.55, 0.38, 0.22]),
        ("roadbump", [0.35, 0.36, 0.40]),
        ("themesnowroad", [0.62, 0.45, 0.25]), ("wood", [0.62, 0.45, 0.25]),
        ("roadtech", [0.28, 0.29, 0.31]), ("platformtech", [0.40, 0.41, 0.43]), ("opentechborders", [0.46, 0.47, 0.49]),
        ("trackwall", [0.66, 0.50, 0.30]), ("trackborders", [0.85, 0.85, 0.85]), ("technicstrims", [0.55, 0.57, 0.60]),
        ("technics", [0.50, 0.52, 0.55]), ("grass", [0.30, 0.55, 0.22]), ("decohill", [0.30, 0.55, 0.22]), ("deco", [0.36, 0.52, 0.26]),
        ("castle", [0.62, 0.60, 0.55]), ("rally", [0.62, 0.60, 0.55]), ("water", [0.20, 0.45, 0.70]),
        ("glass", [0.70, 0.85, 0.95]), ("canopy", [0.70, 0.85, 0.95]), ("plastic", [0.90, 0.30, 0.25]), ("magnetic", [0.15, 0.15, 0.45]),
        ("decal", [0.95, 0.85, 0.30]), ("sign", [0.90, 0.90, 0.90]), ("light", [1.0, 0.95, 0.70]), ("lamp", [1.0, 0.95, 0.70]),
        ("pylon", [0.45, 0.45, 0.45]), ("concrete", [0.60, 0.60, 0.58]), ("metal", [0.55, 0.57, 0.60]), ("barrier", [0.90, 0.45, 0.15]),
        ("inflatable", [0.85, 0.20, 0.20]), ("fence", [0.50, 0.50, 0.50]), ("stone", [0.55, 0.53, 0.50]), ("sand", [0.85, 0.75, 0.50]),
    ]).unwrap_or([0.62, 0.62, 0.64])
}

/// The item's visual mesh from above-front: an orthographic view tilted 35° from
/// vertical, each triangle in its material colour shaded by a directional light,
/// nearest surface wins (z-buffer). Fits `max_px` on the longer side.
/// The diffuse bitmap of a material, from the game pack (shrunk to 512 px for
/// sheet tiles), cached by link; `None` when the pack has no bitmap for it
/// (a modeler `Custom*` material is then drawn in its tint).
type TexCache = BTreeMap<String, Option<std::rc::Rc<mapgeom::dds::Bitmap>>>;

fn texture_for(store: &mut mapgeom::store::DataStore, cache: &mut TexCache, link: &str) -> Option<std::rc::Rc<mapgeom::dds::Bitmap>> {
    if let Some(v) = cache.get(link) {
        return v.clone();
    }
    let bm = mapgeom::dds::material_diffuse(store, link).map(|b| std::rc::Rc::new(b.shrink_to(512)));
    cache.insert(link.to_string(), bm.clone());
    bm
}

struct Tri {
    p: [[f32; 3]; 3],
    uv: [[f32; 2]; 3],
    colour: [f32; 3],
    tex: Option<std::rc::Rc<mapgeom::dds::Bitmap>>,
}

fn render_textured(store: &mut mapgeom::store::DataStore, cache: &mut TexCache, merged: &mapgeom::static_item::build::Merged, max_px: usize) -> Option<Image> {
    let mut tris: Vec<Tri> = Vec::new();
    for vis in &merged.visuals {
        let Some(main) = vis.visual.main.as_ref() else { continue };
        let Some(stream) = vis.visual.stream() else { continue };
        let find = |name: u32| stream.decls.iter().zip(stream.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let pos: Vec<[f32; 3]> = match find(mapgeom::static_item::vstream::N_POSITION) {
            Some(mapgeom::static_item::vstream::Elem::Float3(p)) => p.clone(),
            _ => continue,
        };
        let uvs: Vec<[f32; 2]> = match find(mapgeom::static_item::vstream::N_TEXCOORD0) {
            Some(mapgeom::static_item::vstream::Elem::Float2(t)) => t.clone(),
            _ => Vec::new(),
        };
        let idx: Vec<u32> = match vis.visual.index_buffer.as_ref() {
            Some(ib) => ib.indices.clone(),
            None => (0..main.count.max(0) as u32).collect(),
        };
        let mat = merged.materials.get(vis.material);
        let colour = mat.map(material_colour).unwrap_or([0.6, 0.6, 0.6]);
        let tex = mat.and_then(|m| m.link()).and_then(|l| if uvs.len() == pos.len() { texture_for(store, cache, l) } else { None });
        for t in idx.chunks(3) {
            if t.len() < 3 { continue; }
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a >= pos.len() || b >= pos.len() || c >= pos.len() { continue; }
            let uv = if uvs.len() == pos.len() { [uvs[a], uvs[b], uvs[c]] } else { [[0.0; 2]; 3] };
            tris.push(Tri { p: [pos[a], pos[b], pos[c]], uv, colour, tex: tex.clone() });
        }
    }
    if tris.is_empty() {
        // no visual: fall back to the collision in a neutral grey
        for t in &merged.surf_triangles {
            let p = |i: u32| merged.surf_vertices[i as usize];
            tris.push(Tri { p: [p(t.indices[0]), p(t.indices[1]), p(t.indices[2])], uv: [[0.0; 2]; 3], colour: [0.6, 0.6, 0.62], tex: None });
        }
    }
    if tris.is_empty() {
        return None;
    }
    // view: rotate about X by -35° (look down at the item from the front), then orthographic x / y'
    let tilt = 35f32.to_radians();
    let (ct, st) = (tilt.cos(), tilt.sin());
    let view = |p: [f32; 3]| -> [f32; 3] {
        // world: x right, y up, z depth (towards the viewer = +z). Tilt the camera down.
        let y = p[1] * ct + p[2] * st;
        let z = -p[1] * st + p[2] * ct;
        [p[0], y, z]
    };
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    let vt: Vec<([[f32; 3]; 3], [f32; 3], [f32; 3], usize)> = tris
        .iter()
        .enumerate()
        .map(|(ti, t)| {
            let p = &t.p;
            let q = [view(p[0]), view(p[1]), view(p[2])];
            for v in &q {
                for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); }
            }
            // world normal for lighting
            let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
            let w = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
            let mut n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
            n = [n[0] / l, n[1] / l, n[2] / l];
            (q, t.colour, n, ti)
        })
        .collect();
    let (w_m, h_m) = ((hi[0] - lo[0]).max(0.1), (hi[1] - lo[1]).max(0.1));
    let s = max_px as f32 / w_m.max(h_m);
    let (w, h) = (((w_m * s).ceil() as usize).max(1), ((h_m * s).ceil() as usize).max(1));
    let mut img = Image::new(w, h);
    img.fill(0, 0, w, h, [24, 24, 28]);
    let mut depth = vec![f32::NEG_INFINITY; w * h];
    let light = { let l = [-0.5f32, 0.8, 0.35]; let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt(); [l[0] / n, l[1] / n, l[2] / n] };
    for (q, c, n, ti) in &vt {
        let tri = &tris[*ti];
        let px: Vec<(f32, f32, f32)> = q.iter().map(|v| ((v[0] - lo[0]) * s, (hi[1] - v[1]) * s, v[2])).collect();
        let minx = px.iter().map(|p| p.0).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
        let maxx = (px.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(w - 1);
        let miny = px.iter().map(|p| p.1).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
        let maxy = (px.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(h - 1);
        if minx > maxx || miny > maxy { continue; }
        let lambert = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).abs();
        let shade = 0.45 + 0.55 * lambert;
        let col = [(c[0] * shade * 255.0).min(255.0) as u8, (c[1] * shade * 255.0).min(255.0) as u8, (c[2] * shade * 255.0).min(255.0) as u8];
        let (x0, y0, x1, y1, x2, y2) = (px[0].0, px[0].1, px[1].0, px[1].1, px[2].0, px[2].1);
        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if area.abs() < 1e-6 { continue; }
        for py in miny..=maxy {
            for pxx in minx..=maxx {
                let (fx, fy) = (pxx as f32 + 0.5, py as f32 + 0.5);
                let w0 = ((x1 - fx) * (y2 - fy) - (x2 - fx) * (y1 - fy)) / area;
                let w1 = ((x2 - fx) * (y0 - fy) - (x0 - fx) * (y2 - fy)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 { continue; }
                let z = w0 * px[0].2 + w1 * px[1].2 + w2 * px[2].2;
                let i = py * w + pxx;
                if z > depth[i] {
                    depth[i] = z;
                    let out = match &tri.tex {
                        Some(bm) => {
                            let u = w0 * tri.uv[0][0] + w1 * tri.uv[1][0] + w2 * tri.uv[2][0];
                            let v = w0 * tri.uv[0][1] + w1 * tri.uv[1][1] + w2 * tri.uv[2][1];
                            let t = bm.sample(u, v);
                            [(t[0] as f32 * shade) as u8, (t[1] as f32 * shade) as u8, (t[2] as f32 * shade) as u8]
                        }
                        None => col,
                    };
                    img.px[i * 3..i * 3 + 3].copy_from_slice(&out);
                }
            }
        }
    }
    Some(img)
}
