//! `tinyctl moth-sheet --pak F:KEY --mapping MOTH-MAPPING.tsv --uses LIGHTS.tsv --out sheet.png [--set core|optional] [--cols 4]`
//! — Hugo's "which id is which light" (2026-10-22): one tile per Moth carrier kind: the source item's own
//! header icon (the pak's WEBP; a variant kind shows the parent item's icon + "v23"), a SIDE view of the source
//! mesh at that variant (grey; the light-emitting parts — materials with an `_I` illumination map or a light-ish
//! name, and the light nodes as yellow dots — in yellow; the finish trigger box hatched red), and a label (carrier
//! file, source model + variant byte, a one-line description, the Fall 2026 placement count). Blocks (the
//! StageTechnicsLight* strips) are baked through the tiny bake at scale 1 in the block frame.
use crate::flagdraw::Canvas;
use crate::png::Image;
use mapgeom::static_item::build::Merged;

const INK: [u8; 3] = [20, 20, 20];
const GREY: [u8; 3] = [150, 150, 150];
const LIGHT: [u8; 3] = [240, 190, 20];
const TRIG: [u8; 3] = [220, 40, 40];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

struct Kind {
    set: String,
    model: String,
    variant: Option<u8>,
    is_block: bool,
    carrier: String,
    lo: [f32; 3],
    hi: [f32; 3],
}

/// The header icon of a pak item (chunk 0x2E001004 carries a WEBP), decoded to RGB.
fn icon(store: &mut mapgeom::store::DataStore, model: &str) -> Option<Image> {
    let bytes = store.read(&format!("Stadium\\Items\\{model}.Item.Gbx")).ok()?;
    let b: &[u8] = &bytes;
    if b.len() < 21 || &b[..3] != b"GBX" {
        return None;
    }
    let u32at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
    let n = u32at(17);
    let mut o = 21 + 8 * n;
    for i in 0..n {
        let cid = u32at(21 + 8 * i);
        let sz = u32at(21 + 8 * i + 4) & 0x7fff_ffff;
        if cid == 0x2E00_1004 {
            let d = &b[o..o + sz];
            let p = d.windows(4).position(|w| w == b"RIFF")?;
            return decode_webp(&d[p..]);
        }
        o += sz;
    }
    None
}

fn decode_webp(d: &[u8]) -> Option<Image> {
    let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(d)).ok()?;
    let (w, h) = dec.dimensions();
    let bpp = if dec.has_alpha() { 4 } else { 3 };
    let mut buf = vec![0u8; (w * h) as usize * bpp];
    dec.read_image(&mut buf).ok()?;
    let mut img = Image::new(w as usize, h as usize);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = (y * w as usize + x) * bpp;
            let (r, g, bl) = (buf[i], buf[i + 1], buf[i + 2]);
            // the icons are RGBA over transparent: composite on a dark grey so white lamp heads stay visible
            let a = if bpp == 4 { buf[i + 3] as f32 / 255.0 } else { 1.0 };
            let mix = |c: u8| ((c as f32) * a + 60.0 * (1.0 - a)) as u8;
            img.set(x, y, [mix(r), mix(g), mix(bl)]);
        }
    }
    Some(img)
}

/// Nearest-neighbour resize into a `w`×`h` box (aspect kept, centred on `bg`).
fn fit(src: &Image, w: usize, h: usize, bg: [u8; 3]) -> Image {
    let mut out = Image::new(w, h);
    out.fill(0, 0, w, h, bg);
    let s = (w as f32 / src.w as f32).min(h as f32 / src.h as f32);
    let (ow, oh) = ((src.w as f32 * s) as usize, (src.h as f32 * s) as usize);
    let (x0, y0) = ((w - ow) / 2, (h - oh) / 2);
    for y in 0..oh {
        for x in 0..ow {
            let sx = ((x as f32 / s) as usize).min(src.w - 1);
            let sy = ((y as f32 / s) as usize).min(src.h - 1);
            out.set(x0 + x, y0 + y, src.get(sx, sy));
        }
    }
    out
}

/// The mesh of a kind: triangles tagged light/not, plus the light-node positions.
struct Mesh {
    tris: Vec<([f32; 3], [f32; 3], [f32; 3], bool)>,
    nodes: Vec<[f32; 3]>,
}

fn is_light_material(store: &mut mapgeom::store::DataStore, cache: &mut mapgeom::lights::MatCache, link: &str) -> bool {
    let tex = cache.emissive(store, link);
    let has_i = tex.map(|(i, _)| !i.is_empty()).unwrap_or(false);
    let s = link.rsplit('\\').next().unwrap_or(link).to_ascii_lowercase();
    has_i || s.contains("lightspot") || s.contains("itemlamplight") || s.contains("lightshape") || s.contains("lighttube")
}

fn mesh_of(m: &Merged, store: &mut mapgeom::store::DataStore, cache: &mut mapgeom::lights::MatCache) -> Mesh {
    let mut tris = Vec::new();
    for mv in &m.visuals {
        let link = m.materials.get(mv.material).and_then(|mat| mat.link()).unwrap_or("").to_string();
        let light = is_light_material(store, cache, &link);
        let Some(main) = mv.visual.main.as_ref() else { continue };
        let Some(mapgeom::static_item::Node::VertexStream(stream)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let mut pos: &[[f32; 3]] = &[];
        for (d, e) in stream.decls.iter().zip(stream.elems.iter()) {
            if let (mapgeom::static_item::vstream::N_POSITION, mapgeom::static_item::vstream::T_FLOAT3, mapgeom::static_item::vstream::Elem::Float3(p)) = (d.name(), d.stored_type(compress), e) {
                pos = p;
            }
        }
        let Some(ib) = mv.visual.index_buffer.as_ref() else { continue };
        for t in ib.indices.chunks(3) {
            if t.len() < 3 { continue; }
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a < pos.len() && b < pos.len() && c < pos.len() {
                tris.push((pos[a], pos[b], pos[c], light));
            }
        }
    }
    let nodes = m.lights_out.iter().map(|l| [l.socket.u05[9], l.socket.u05[10], l.socket.u05[11]]).collect();
    Mesh { tris, nodes }
}

fn describe(model: &str, variant: Option<u8>, is_block: bool) -> &'static str {
    match (model, variant) {
        ("Lamp", _) => "STREET LAMP (TALL): THE HEAD ONLY, 5.5-7.8 M UP",
        ("LampB", _) | ("LampC", _) => "STREET LAMP VARIANT (OTHER HEAD), 5.5-7.7 M UP",
        ("LampSmall", _) => "STREET LAMP (SHORT): THE HEAD, 2.5-4.7 M UP",
        ("LampSmallB", _) | ("LampSmallC", _) => "SHORT LAMP VARIANT (OTHER HEAD), 2.8-4.4 M UP",
        ("ShowTorch", _) => "SHOW TORCH (FLAME LIGHT), 1.9 M UP",
        ("ShowTorchSmall", _) => "SMALL SHOW TORCH, 1 M UP",
        ("ShowLight4Spots", _) => "4-SPOT RIG HEAD (STANDALONE ITEM)",
        ("ShowLightRamp4m", _) => "4 M LIGHT BAR (STANDALONE ITEM)",
        ("ShowLightRamp8m", _) => "8 M LIGHT BAR (STANDALONE ITEM)",
        ("ShowLights", Some(23)) | ("Show", Some(23)) => "RIG ITEM, VARIANT = THE 4-SPOT HEAD (LIGHT4SPOTS)",
        ("ShowLights", Some(24)) | ("Show", Some(24)) => "RIG ITEM, VARIANT = THE 4 M LIGHT BAR (LIGHTRAMP4M)",
        ("ShowLights", Some(25)) | ("Show", Some(25)) => "RIG ITEM, VARIANT = THE 8 M LIGHT BAR (LIGHTRAMP8M)",
        ("ShowLights", Some(32)) | ("Show", Some(35)) => "RIG ITEM, VARIANT = LIGHTS FRONT: 8 SPOTS ON A 28 M TRUSS",
        ("ShowLights", Some(33)) | ("Show", Some(36)) => "RIG ITEM, VARIANT = LIGHTS UP: 8 SPOTS AIMED UP, 28 M TRUSS",
        ("StageTechnicsLight", _) => "STAGE LIGHT BAR (BLOCK): 32 M TRUSS, 2 SPOTS + LED STRIP, HUNG AT 6.2-7.4 M",
        ("StageTechnicsLightDeadend", _) => "STAGE LIGHT BAR, DEAD-END PLACEMENT (NEAR EDGE), HUNG AT 6.2-7.4 M",
        ("StageTechnicsLightDown", _) => "THE SAME LIGHT BAR MOUNTED LOW: STRIP AT 0.2-1.4 M (FLOOR LEVEL)",
        ("StageTechnicsLightStraight", _) => "STAGE LIGHT BAR, STRAIGHT: 14 M STRIP ALONG THE CELL, 4 SPOTS, 6.2-7.4 M",
        ("StageTechnicsLightToStructure", _) => "STAGE LIGHT BAR WITH A STRUCTURE CONNECTOR, HUNG AT 6.2-7.4 M",
        _ if model.starts_with("LightTube") && model.contains("Curve") => "LIGHT TUBE, CURVED (THE WHOLE TUBE IS THE LIGHT)",
        _ if model.starts_with("LightTube") && model.contains("Diag") => "LIGHT TUBE, DIAGONAL (THE WHOLE TUBE IS THE LIGHT)",
        _ if model.starts_with("LightTube") => "LIGHT TUBE, STRAIGHT (THE WHOLE TUBE IS THE LIGHT)",
        _ if model.starts_with("LightCubeSlope") => "LIGHT SHAPE: SLOPED CUBE",
        _ if model.starts_with("LightCubeArch") => "LIGHT SHAPE: ARCH",
        _ if model.starts_with("LightCube") => "LIGHT SHAPE: CUBE",
        _ if model.starts_with("LightSphere") => "LIGHT SHAPE: SPHERE (OR A PART OF ONE)",
        _ if model.starts_with("LightCylinder") => "LIGHT SHAPE: CYLINDER (OR A QUARTER)",
        _ if model.starts_with("LightCone") => "LIGHT SHAPE: CONE",
        _ => if is_block { "BLOCK" } else { "ITEM" },
    }
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let mapping = f(rest, "--mapping").ok_or("moth-sheet needs --mapping MOTH-MAPPING.tsv")?;
    let out = f(rest, "--out").unwrap_or_else(|| "moth-sheet.png".into());
    let set = f(rest, "--set").unwrap_or_else(|| "core".into());
    let cols: usize = f(rest, "--cols").and_then(|v| v.parse().ok()).unwrap_or(4);
    let uses_tsv = f(rest, "--uses");
    let title = f(rest, "--title").unwrap_or_else(|| format!("MOTH CARRIERS - {} SET", set.to_uppercase()));
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--pak" {
            if let Some((p, k)) = rest.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                store.add_pak(p, k)?;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    // ---- the kinds
    let text = std::fs::read_to_string(&mapping).map_err(|e| format!("{mapping}: {e}"))?;
    let mut kinds: Vec<Kind> = Vec::new();
    for (n, l) in text.lines().enumerate() {
        if n == 0 { continue; }
        let p: Vec<&str> = l.split('\t').collect();
        if p.len() < 13 { continue; }
        // columns: set folder source_model variant source_kind carrier_file carrier_ident lo_x lo_y lo_z hi_x hi_y hi_z pose_rule
        let (s, model, variant, kind, carrier) = (p[0], p[2], p[3], p[4], p[5]);
        let want = if set == "core" { s == "core" } else if set == "all" { true } else { s.starts_with("optional") && s.contains(&set) };
        if !want { continue; }
        let g = |k: usize| -> f32 { p[k].parse().unwrap_or(0.0) };
        kinds.push(Kind { set: s.to_string(), model: model.to_string(), variant: variant.parse::<u8>().ok(), is_block: kind == "block", carrier: carrier.to_string(), lo: [g(7), g(8), g(9)], hi: [g(10), g(11), g(12)] });
    }
    if kinds.is_empty() {
        return Err(format!("no kinds of set {set:?} in {mapping}"));
    }
    // ---- Fall 2026 uses from LIGHTS.tsv (column 16 = fall2026_uses, keyed by name / name@vN)
    let mut uses: std::collections::HashMap<String, u32> = Default::default();
    if let Some(u) = &uses_tsv {
        let t = std::fs::read_to_string(u).map_err(|e| format!("{u}: {e}"))?;
        for l in t.lines().skip(1) {
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() > 16 {
                let n: u32 = p[15].parse().unwrap_or(0);
                uses.entry(p[2].to_string()).and_modify(|v| *v = (*v).max(n)).or_insert(n);
            }
        }
    }
    let mut cache = mapgeom::lights::MatCache::new();
    // ---- layout
    let tile_w = 1600 / cols;
    let icon_px = 150usize;
    let view_h = 300usize;
    let label_h = 120usize;
    let tile_h = icon_px.max(view_h) + label_h + 20;
    let rows = (kinds.len() + cols - 1) / cols;
    let header = 70usize;
    let (w, h) = (1600usize, header + rows * tile_h + 20);
    let mut cv = Canvas::new(w, h);
    cv.text(30.0, 18.0, &title, 4, INK);
    cv.text(30.0, 50.0, "PER TILE: ITEM ICON | SIDE VIEW (GREY = MESH, YELLOW = LIGHT-EMITTING PARTS AND LIGHT NODES, RED HATCH = FINISH TRIGGER BOX) | CARRIER / SOURCE / FALL 2026 PLACEMENTS", 1, GREY);
    let mut legend = String::from("tile\tcarrier\tsource_model\tvariant\tkind\tdescription\tfall2026_uses\ttrigger_lo\ttrigger_hi\tmesh_tris\tlight_tris\tlight_nodes\n");
    for (k, kind) in kinds.iter().enumerate() {
        let (col, row) = (k % cols, k / cols);
        let (tx, ty) = (col * tile_w + 10, header + row * tile_h);
        cv.img.rect(tx, ty, tile_w - 20, tile_h - 10, 1, [210, 210, 210]);
        // (1) icon
        let ic = icon(&mut store, &kind.model).map(|im| fit(&im, icon_px, icon_px, [60, 60, 60])).unwrap_or_else(|| { let mut im = Image::new(icon_px, icon_px); im.fill(0, 0, icon_px, icon_px, [90, 90, 90]); im });
        cv.img.blit(&ic, tx + 8, ty + 8);
        if let Some(v) = kind.variant {
            cv.img.fill(tx + 8, ty + 8 + icon_px - 26, 60, 22, [0, 0, 0]);
            cv.text((tx + 12) as f32, (ty + 8 + icon_px - 23) as f32, &format!("V{v}"), 2, [255, 230, 80]);
        }
        if kind.is_block {
            cv.img.fill(tx + 8, ty + 8, 70, 20, [0, 0, 0]);
            cv.text((tx + 12) as f32, (ty + 11) as f32, "BLOCK", 2, [120, 200, 255]);
        }
        // (2) the mesh at the variant
        let merged: Result<Merged, String> = if kind.is_block {
            mapgeom::tiny_library::bake_named_block(&mut store, 26, &kind.model, mapgeom::blockmap::FLAG_GROUND, &format!("{}.Item.Gbx", kind.model), "Nadeo", 1.0).map(|(_, m, _)| m)
        } else {
            mapgeom::static_item::build::pack_item_merged(&mut store, &format!("Stadium\\Items\\{}.Item.Gbx", kind.model), 1.0, 26, kind.variant.unwrap_or(0) as usize, None)
        };
        let (vx, vy, vw, vh) = (tx + icon_px + 20, ty + 8, tile_w - icon_px - 48, view_h);
        cv.img.fill(vx, vy, vw, vh, [245, 245, 245]);
        let mut stats = (0usize, 0usize, 0usize);
        match merged {
            Ok(m) => {
                let mesh = mesh_of(&m, &mut store, &mut cache);
                stats = (mesh.tris.len(), mesh.tris.iter().filter(|t| t.3).count(), mesh.nodes.len());
                // side view: the longer horizontal extent → the horizontal axis (x or z), y up; frame = mesh ∪ trigger
                let (mut lo, mut hi) = (kind.lo, kind.hi);
                for t in &mesh.tris {
                    for p in [t.0, t.1, t.2] {
                        for a in 0..3 { lo[a] = lo[a].min(p[a]); hi[a] = hi[a].max(p[a]); }
                    }
                }
                let use_x = (hi[0] - lo[0]) >= (hi[2] - lo[2]);
                let haxis = if use_x { 0 } else { 2 };
                let (h_lo, h_hi) = (lo[haxis], hi[haxis]);
                let (y_lo, y_hi) = (lo[1], hi[1]);
                let margin = 0.6f32;
                let scale = ((vw as f32 - 20.0) / (h_hi - h_lo + 2.0 * margin)).min((vh as f32 - 20.0) / (y_hi - y_lo + 2.0 * margin));
                let cx = vx as f32 + vw as f32 / 2.0;
                let cy = vy as f32 + vh as f32 / 2.0;
                let mh = (h_lo + h_hi) / 2.0;
                let my = (y_lo + y_hi) / 2.0;
                let px = |p: [f32; 3]| -> (f32, f32) { (cx + (p[haxis] - mh) * scale, cy - (p[1] - my) * scale) };
                // grey first, then the light parts on top
                for pass in 0..2 {
                    for t in &mesh.tris {
                        if (pass == 1) != t.3 { continue; }
                        let pts = [px(t.0), px(t.1), px(t.2)];
                        cv.poly(&pts, if t.3 { LIGHT } else { GREY }, if t.3 { 0.9 } else { 0.35 });
                        if t.3 {
                            cv.outline(&pts, [200, 140, 0], 1.5);
                        }
                    }
                }
                for n in &mesh.nodes {
                    let (x, y) = px(*n);
                    let dot: Vec<(f32, f32)> = (0..12).map(|k| { let a = k as f32 / 12.0 * std::f32::consts::TAU; (x + 5.0 * a.cos(), y + 5.0 * a.sin()) }).collect();
                    cv.poly(&dot, LIGHT, 1.0);
                    cv.outline(&dot, [120, 90, 0], 1.0);
                }
                // the trigger box
                let bl = px([kind.lo[0], kind.lo[1], kind.lo[2]]);
                let tr = px([kind.hi[0], kind.hi[1], kind.hi[2]]);
                let rect = [(bl.0, bl.1), (tr.0, bl.1), (tr.0, tr.1), (bl.0, tr.1)];
                cv.hatch(&rect, TRIG, 9.0, 1.0);
                cv.outline(&rect, TRIG, 2.0);
                // ground line + scale bar
                let g = px([0.0, 0.0, 0.0]);
                cv.dashed(vx as f32 + 4.0, g.1, (vx + vw) as f32 - 4.0, g.1, [120, 120, 120], 1.0, 5.0);
                let bar = (1.0f32).max(((h_hi - h_lo) / 4.0).round());
                let bx0 = vx as f32 + 10.0;
                let by = (vy + vh) as f32 - 12.0;
                cv.line(bx0, by, bx0 + bar * scale, by, INK, 2.0, 1.0);
                cv.text(bx0, by - 16.0, &format!("{bar:.0} M"), 1, INK);
                cv.text((vx + vw) as f32 - 70.0, by - 16.0, if use_x { "X RIGHT, Y UP" } else { "Z RIGHT, Y UP" }, 1, GREY);
            }
            Err(e) => {
                cv.text(vx as f32 + 10.0, vy as f32 + 10.0, &format!("NO MESH: {}", e.chars().take(40).collect::<String>().to_uppercase()), 1, TRIG);
            }
        }
        // (3) label
        let ly = ty + icon_px.max(view_h) + 14;
        // the glyph cell is 6 px × scale wide: the name at scale 2 when it fits the tile, else scale 1
        let name = kind.carrier.trim_end_matches(".Item.Gbx").to_uppercase();
        let name_scale = if name.len() * 12 + 16 <= tile_w - 20 { 2 } else { 1 };
        cv.text((tx + 8) as f32, ly as f32, &name, name_scale, INK);
        let src = match kind.variant { Some(v) => format!("{} VARIANT {v} (FLAGS HIGH BYTE)", kind.model.to_uppercase()), None => format!("{}{}", kind.model.to_uppercase(), if kind.is_block { " (BLOCK)" } else { "" }) };
        cv.text((tx + 8) as f32, (ly + 22) as f32, &src, 1, INK);
        let desc = describe(&kind.model, kind.variant, kind.is_block);
        // wrap the description at the tile width (6 px per glyph at scale 1)
        let max_chars = (tile_w - 36) / 6;
        let (d1, d2) = if desc.len() <= max_chars { (desc.to_string(), String::new()) } else {
            let cut = desc[..max_chars].rfind(' ').unwrap_or(max_chars);
            (desc[..cut].to_string(), desc[cut..].trim_start().to_string())
        };
        cv.text((tx + 8) as f32, (ly + 38) as f32, &d1, 1, [60, 60, 60]);
        cv.text((tx + 8) as f32, (ly + 52) as f32, &d2, 1, [60, 60, 60]);
        let key = match kind.variant { Some(v) => format!("{}@v{v}", kind.model), None => kind.model.clone() };
        let n_uses = uses.get(&key).copied().or_else(|| uses.get(&format!("{}@v0", kind.model)).copied()).unwrap_or(0);
        cv.text((tx + 8) as f32, (ly + 70) as f32, &format!("FALL 2026: {n_uses} PLACEMENTS", ), 1, [60, 60, 60]);
        cv.text((tx + 8) as f32, (ly + 86) as f32, &format!("TRIGGER X {:.1}..{:.1}  Y {:.1}..{:.1}  Z {:.1}..{:.1} M", kind.lo[0], kind.hi[0], kind.lo[1], kind.hi[1], kind.lo[2], kind.hi[2]), 1, TRIG);
        legend.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2} {:.2} {:.2}\t{:.2} {:.2} {:.2}\t{}\t{}\t{}\n", k + 1, kind.carrier, kind.model, kind.variant.map(|v| v.to_string()).unwrap_or_else(|| if kind.is_block { "block".into() } else { "any".into() }), kind.set, desc, n_uses, kind.lo[0], kind.lo[1], kind.lo[2], kind.hi[0], kind.hi[1], kind.hi[2], stats.0, stats.1, stats.2));
    }
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    if let Some(l) = f(rest, "--legend") {
        std::fs::write(&l, legend).map_err(|e| format!("{l}: {e}"))?;
    }
    println!("wrote {out}: {w}x{h}, {} tiles", kinds.len());
    Ok(())
}
