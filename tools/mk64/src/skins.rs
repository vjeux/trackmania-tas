//! MK64 character car skins for TM2020 — "sprite karts".
//!
//! The N64 game draws every kart as a pre-rendered sprite; the most faithful
//! TM2020 skin is therefore the sprite itself, standing up: a 3D skin
//! (`MainBody.Mesh.gbx`, the writer in `mapgeom::static_item::skin`) whose
//! geometry is the sprite's OPAQUE PIXELS extruded into quads — the silhouette
//! is exact without alpha (vehicle materials cannot alpha-test), and every
//! pixel keeps its palette colour through a nearest-upscaled atlas. Four
//! sheets make the kart readable from every side: the back-view frame facing
//! the chase camera, the front-view frame facing forward, the right-profile
//! frame on the right and the same frame seen from the other side on the left
//! (the N64 mirrors sprites for that half too). A chassis slab in the kart's
//! colour sits between the game's own wheels, which stay.
//!
//! Frames (measured on the contact sheet, `mk64 sprites`): the 21 frames
//! 0..=20 sweep back → right side → front in 9° steps; 0 = straight back,
//! 10 = right profile, 20 = straight front.
//!
//! Skin frame (measured 2026-09-12): +z = car front, +x = the car's LEFT, y up,
//! origin = the car position; the template's body spans the same frame.
use crate::sprites::{contact_sheet, KartSprites, CHARACTERS};
use crate::texture::{AssetIndex, Image, Rom};
use mapgeom::static_item::skin::{self, Binding, Options, Part};
use mapgeom::static_item::Node;
use std::collections::BTreeMap;
use std::path::Path;

pub const FRAME_BACK: usize = 0;
pub const FRAME_RIGHT: usize = 10;
pub const FRAME_FRONT: usize = 20;

/// Metres per sprite pixel: the sprite's kart body (~48 of 64 px) as wide as
/// the TM car (2.05 m, the same calibration the courses use).
pub const M_PER_PX: f32 = 2.05 / 48.0;
/// The atlas upscale (nearest): DXT blocks then straddle no pixel edge.
pub const ATLAS_UPSCALE: u32 = 4;
/// Half-thickness of the standee cross: the back and front sheets sit this far
/// behind/ahead of the centre, the side sheets this far right/left.
pub const SHEET_OFFSET: f32 = 0.06;

pub struct Sheets {
    pub back: Image,
    pub front: Image,
    pub right: Image,
}

pub fn sheets(ks: &KartSprites, rom: &Rom) -> Result<Sheets, String> {
    Ok(Sheets { back: ks.frame(rom, FRAME_BACK)?, front: ks.frame(rom, FRAME_FRONT)?, right: ks.frame(rom, FRAME_RIGHT)? })
}

/// The lowest opaque row of an image (the ground contact), and its opaque
/// column span.
fn opaque_extent(img: &Image) -> (u32, u32, u32, u32) {
    let (mut x0, mut y0, mut x1, mut y1) = (img.w, img.h, 0, 0);
    for y in 0..img.h {
        for x in 0..img.w {
            if img.rgba[((y * img.w + x) * 4 + 3) as usize] > 0 {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    (x0, y0, x1, y1)
}

/// The kart body colour: the most frequent saturated colour of the sprite's
/// lower half (the kart; the driver sits above).
pub fn kart_colour(img: &Image) -> [u8; 3] {
    let mut hist: std::collections::HashMap<[u8; 3], u32> = std::collections::HashMap::new();
    let (_, y0, _, y1) = opaque_extent(img);
    // the bottom third: the chassis and its frame, below the driver's clothes
    let mid = y0 + (y1 - y0) * 2 / 3;
    for y in mid..=y1 {
        for x in 0..img.w {
            let i = ((y * img.w + x) * 4) as usize;
            let (r, g, b, a) = (img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]);
            if a == 0 {
                continue;
            }
            let mx = r.max(g).max(b) as i32;
            let mn = r.min(g).min(b) as i32;
            if mx < 60 || mx - mn < 50 {
                continue; // black tyres, greys, white
            }
            // quantise so shading variants merge
            *hist.entry([r & 0xE0, g & 0xE0, b & 0xE0]).or_insert(0) += 1;
        }
    }
    hist.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| [c[0] | 0x10, c[1] | 0x10, c[2] | 0x10]).unwrap_or([200, 40, 40])
}

/// Where each frame lands in the 128×128 atlas (in sprite pixels).
const ATLAS_SLOTS: [(u32, u32); 4] = [(0, 0), (64, 0), (0, 64), (64, 64)]; // back, front, right, slab-colour

pub struct Atlas {
    pub img: Image, // 128×128 at sprite resolution (the DDS is this ×ATLAS_UPSCALE)
}

pub fn atlas(sh: &Sheets, slab: [u8; 3], portrait: Option<&Image>) -> Atlas {
    let mut img = Image::solid(128, 128, [0, 0, 0, 255]);
    let blit = |img: &mut Image, src: &Image, ox: u32, oy: u32| {
        for y in 0..src.h {
            for x in 0..src.w {
                let si = ((y * src.w + x) * 4) as usize;
                let di = (((oy + y) * img.w + ox + x) * 4) as usize;
                if src.rgba[si + 3] > 0 {
                    img.rgba[di..di + 3].copy_from_slice(&src.rgba[si..si + 3]);
                } else {
                    // transparent texels take the kart colour so DXT bleed at pixel
                    // edges reads as body colour, not black
                    img.rgba[di..di + 3].copy_from_slice(&slab);
                }
                img.rgba[di + 3] = 255;
            }
        }
    };
    blit(&mut img, &sh.back, ATLAS_SLOTS[0].0, ATLAS_SLOTS[0].1);
    blit(&mut img, &sh.front, ATLAS_SLOTS[1].0, ATLAS_SLOTS[1].1);
    blit(&mut img, &sh.right, ATLAS_SLOTS[2].0, ATLAS_SLOTS[2].1);
    for y in 64..128 {
        for x in 64..128 {
            let di = ((y * 128 + x) * 4) as usize;
            img.rgba[di..di + 3].copy_from_slice(&slab);
        }
    }
    // the result-screen portrait (32×32) at (96, 96): the slab's rear face wears
    // it like a licence plate
    if let Some(p) = portrait {
        if p.w == 32 && p.h == 32 {
            blit(&mut img, p, 96, 96);
        }
    }
    Atlas { img }
}

/// UV of an atlas texel centre; the skin atlas is sampled with v = 0 at the
/// BOTTOM (measured 2026-09-13), hence the flip.
fn uv(px: u32, py: u32) -> [f32; 2] {
    [(px as f32 + 0.5) / 128.0, 1.0 - (py as f32 + 0.5) / 128.0]
}

#[derive(Clone, Copy, Debug)]
pub enum View {
    Back,
    Front,
    Right,
    Left,
}

/// One standee sheet: a quad per opaque pixel of `frame`, in the plane of `view`.
/// `ground_y` is the skin-frame height of the sprite's lowest opaque row.
pub fn standee(frame: &Image, slot: usize, view: View, ground_y: f32, texset: &str) -> Part {
    let s = M_PER_PX;
    let (_, _, _, y_bottom) = opaque_extent(frame);
    let (ax, ay) = ATLAS_SLOTS[slot];
    let mut part = Part { texset: texset.to_string(), source: format!("{view:?}"), ..Default::default() };
    // pixel (px, py) → screen-space centre (sx, sy) in metres: sx to the viewer's right, sy up
    let half_w = frame.w as f32 * 0.5;
    for py in 0..frame.h {
        for px in 0..frame.w {
            if frame.rgba[((py * frame.w + px) * 4 + 3) as usize] == 0 {
                continue;
            }
            let sx0 = (px as f32 - half_w) * s;
            let sx1 = sx0 + s;
            let sy0 = (y_bottom as f32 - py as f32) * s + ground_y;
            let sy1 = sy0 + s;
            // screen → skin frame per view (right-handed, y up; the viewer looks
            // along `fwd`, screen-right = fwd × up)
            let to_world = |sx: f32, sy: f32| -> [f32; 3] {
                match view {
                    View::Back => [-sx, sy, -SHEET_OFFSET],  // viewer behind, looking +z: right = −x
                    View::Front => [sx, sy, SHEET_OFFSET],   // viewer ahead, looking −z: right = +x
                    View::Right => [-SHEET_OFFSET, sy, sx],  // viewer at the car's right (−x), looking +x: right = +z
                    View::Left => [SHEET_OFFSET, sy, sx],    // viewer at the car's left (+x), looking −x: right = −z … the same
                                                             // frame seen from behind the sheet = the mirrored sprite
                }
            };
            let n = match view {
                View::Back => [0.0, 0.0, -1.0],
                View::Front => [0.0, 0.0, 1.0],
                View::Right => [-1.0, 0.0, 0.0],
                View::Left => [1.0, 0.0, 0.0],
            };
            let base = part.pos.len() as u32;
            let corners = [(sx0, sy0), (sx1, sy0), (sx1, sy1), (sx0, sy1)];
            for (sx, sy) in corners {
                part.pos.push(to_world(sx, sy));
                part.nrm.push(n);
                part.uv.push(uv(ax + px, ay + py));
            }
            // winding: counter-clockwise seen from the viewer. `to_world` maps
            // screen-right onto the viewer's right for every view but Left, where
            // the image is deliberately mirrored (nose kept at +z) — flip it there.
            let ccw = match view {
                View::Left => [0, 2, 1, 0, 3, 2],
                _ => [0, 1, 2, 0, 2, 3],
            };
            for k in ccw {
                part.idx.push(base + k);
            }
        }
    }
    part
}

/// The chassis slab between the wheels, in the kart colour.
pub fn slab(ground_y: f32, texset: &str) -> Part {
    let (hx, y0, y1, hz) = (0.95f32, ground_y + 0.12, ground_y + 0.32, 1.25f32);
    let mut p = Part { texset: texset.to_string(), source: "slab".into(), ..Default::default() };
    let c = uv(ATLAS_SLOTS[3].0 + 16, ATLAS_SLOTS[3].1 + 16);
    let mut quad = |a: [f32; 3], b: [f32; 3], cc: [f32; 3], d: [f32; 3], n: [f32; 3], uvs: Option<[[f32; 2]; 4]>| {
        let base = p.pos.len() as u32;
        for (k, v) in [a, b, cc, d].into_iter().enumerate() {
            p.pos.push(v);
            p.nrm.push(n);
            p.uv.push(uvs.map(|u| u[k]).unwrap_or(c));
        }
        p.idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    // the portrait square of the atlas (96..128 × 96..128), v flipped like `uv`
    let (pu0, pu1, pv_top, pv_bot) = (96.0 / 128.0, 128.0 / 128.0, 1.0 - 96.0 / 128.0, 1.0 - 128.0 / 128.0);
    // top (+y), bottom (−y), front (+z), back (−z), left (+x), right (−x)
    quad([-hx, y1, -hz], [-hx, y1, hz], [hx, y1, hz], [hx, y1, -hz], [0.0, 1.0, 0.0], None);
    quad([-hx, y0, hz], [-hx, y0, -hz], [hx, y0, -hz], [hx, y0, hz], [0.0, -1.0, 0.0], None);
    quad([-hx, y0, hz], [hx, y0, hz], [hx, y1, hz], [-hx, y1, hz], [0.0, 0.0, 1.0], None);
    // back face, seen from behind (screen-right = −x): the portrait, square,
    // centred, as tall as the slab
    let ph = (y1 - y0) * 0.5;
    quad([ph, y0, -hz], [-ph, y0, -hz], [-ph, y1, -hz], [ph, y1, -hz], [0.0, 0.0, -1.0], Some([[pu0, pv_bot], [pu1, pv_bot], [pu1, pv_top], [pu0, pv_top]]));
    quad([hx, y0, -hz], [ph, y0, -hz], [ph, y1, -hz], [hx, y1, -hz], [0.0, 0.0, -1.0], None);
    quad([-ph, y0, -hz], [-hx, y0, -hz], [-hx, y1, -hz], [-ph, y1, -hz], [0.0, 0.0, -1.0], None);
    quad([hx, y0, hz], [hx, y0, -hz], [hx, y1, -hz], [hx, y1, hz], [1.0, 0.0, 0.0], None);
    quad([-hx, y0, -hz], [-hx, y0, hz], [-hx, y1, hz], [-hx, y1, -hz], [-1.0, 0.0, 0.0], None);
    p
}

/// The template's body bounds from its vertex positions (the first Float3
/// element of every inline vertex stream — the position), skin frame.
pub fn template_bounds(t: &mapgeom::static_item::solid2::CPlugSolid2Model) -> Option<([f32; 3], [f32; 3])> {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    let mut any = false;
    for v in &t.visuals {
        if let Some(Node::Visual(vis)) = v.inline.as_deref() {
            if let Some(m) = &vis.main {
                for sref in &m.vertex_streams {
                    if let Some(Node::VertexStream(vs)) = sref.inline.as_deref() {
                        if let Some(mapgeom::static_item::vstream::Elem::Float3(ps)) = vs.elems.iter().find(|e| matches!(e, mapgeom::static_item::vstream::Elem::Float3(_))) {
                            for p in ps {
                                for k in 0..3 {
                                    lo[k] = lo[k].min(p[k]);
                                    hi[k] = hi[k].max(p[k]);
                                }
                                any = true;
                            }
                        }
                    }
                }
            }
        }
    }
    any.then_some((lo, hi))
}

pub struct SkinOut {
    pub name: String,
    pub zip: Vec<u8>,
    pub atlas_png: Vec<u8>,
    pub report: String,
}

/// One character's skin zip.
pub fn build_character(
    template: &mapgeom::static_item::solid2::CPlugSolid2Model,
    ks: &KartSprites,
    rom: &mut Rom,
    assets: &AssetIndex,
    display: &str,
    ground_y: f32,
    with_slab: bool,
) -> Result<SkinOut, String> {
    let sh = sheets(ks, rom)?;
    let colour = kart_colour(&sh.back);
    let portrait = ks.portrait(rom, assets).ok();
    let at = atlas(&sh, colour, portrait.as_ref());
    let mut parts = vec![
        standee(&sh.back, 0, View::Back, ground_y, "Details"),
        standee(&sh.front, 1, View::Front, ground_y, "Details"),
        standee(&sh.right, 2, View::Right, ground_y, "Details"),
        standee(&sh.right, 2, View::Left, ground_y, "Details"),
    ];
    if with_slab {
        parts.push(slab(ground_y, "Details"));
    }
    let mut o = Options::default();
    o.binding = Binding::Template;
    o.prune_materials = true;
    let built = skin::build(template, &parts, &o)?;
    // the atlas ×4 nearest → Details_B with mips; flats for the rest; the three
    // other sets the vehicle insists on
    let up = ATLAS_UPSCALE;
    let (w, h) = (128 * up, 128 * up);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let si = (((y / up) * 128 + x / up) * 4) as usize;
            let di = ((y * w + x) * 4) as usize;
            rgba[di..di + 4].copy_from_slice(&at.img.rgba[si..si + 4]);
        }
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    files.insert("MainBody.Mesh.gbx".into(), built.file.clone());
    for (n, b) in skin::texture_set("Details", w, h, &rgba, 230) {
        files.insert(n, b);
    }
    // wheels: MK64 tyres are plain black rubber with a grey hub; the game's wheel
    // mesh keeps its own UVs, so a uniform dark base is the faithful choice
    for (n, b) in skin::texture_set("Wheels", 4, 4, &skin::flat_rgba(4, [28, 28, 30, 255]), 235) {
        files.insert(n, b);
    }
    for (n, b) in skin::texture_set("Skin", 4, 4, &skin::flat_rgba(4, [colour[0], colour[1], colour[2], 255]), 220) {
        files.insert(n, b);
    }
    for (n, b) in skin::texture_set("Glass", 4, 4, &skin::flat_rgba(4, [20, 20, 20, 0]), 40) {
        files.insert(n, b);
    }
    let zip = tmmaps::header::deflated_zip(&files);
    let report = format!(
        "{display}: {} visuals, {} vertices, {} triangles, kart colour #{:02X}{:02X}{:02X}, bounds x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}, zip {} bytes",
        built.visuals, built.vertices, built.triangles, colour[0], colour[1], colour[2], built.bounds.0[0], built.bounds.1[0], built.bounds.0[1], built.bounds.1[1], built.bounds.0[2], built.bounds.1[2], zip.len()
    );
    Ok(SkinOut { name: format!("MK64 {display}"), zip, atlas_png: contact_sheet(&[at.img.clone()], 1, 2).png(), report })
}

/// `mk64 skins --template MainBody.Mesh.gbx --out DIR [--chars a,b] [--ground Y] [--no-slab]`
pub fn cmd(args: &[String], decomp: &Path, rom_path: &Path) {
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let tpl = flag("--template").expect("--template <community MainBody.Mesh.gbx>");
    let out = std::path::PathBuf::from(flag("--out").expect("--out DIR"));
    std::fs::create_dir_all(&out).expect("create --out");
    let mut rom = Rom::load(rom_path).expect("rom");
    let assets = AssetIndex::load(decomp).expect("asset index");
    let bytes = std::fs::read(&tpl).expect("template");
    let model = mapgeom::store::Model::parse(&bytes, &tpl).expect("template model");
    let template = skin::template_from_body(&model.body).expect("template solid2");
    let tb = template_bounds(&template);
    if let Some((lo, hi)) = tb {
        println!("template body: x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
    }
    // the sprite's ground row: the template body's lowest point by default
    let ground_y: f32 = flag("--ground").and_then(|s| s.parse().ok()).unwrap_or_else(|| tb.map(|(lo, _)| lo[1]).unwrap_or(0.0));
    let with_slab = !args.iter().any(|a| a == "--no-slab");
    let want: Vec<String> = flag("--chars").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
    for (stem, face_stem, display) in CHARACTERS {
        if !want.is_empty() && !want.iter().any(|w| w == stem) {
            continue;
        }
        let ks = match KartSprites::load(decomp, stem, face_stem) {
            Ok(k) => k,
            Err(e) => {
                println!("{stem}: {e}");
                continue;
            }
        };
        match build_character(&template, &ks, &mut rom, &assets, display, ground_y, with_slab) {
            Ok(s) => {
                let file = out.join(format!("{}.zip", s.name));
                std::fs::write(&file, &s.zip).expect("write zip");
                std::fs::write(out.join(format!("{}_atlas.png", s.name)), &s.atlas_png).expect("write atlas");
                println!("{}  → {}", s.report, file.display());
            }
            Err(e) => println!("{display}: {e}"),
        }
    }
}
