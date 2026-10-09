//! `tinyctl flag-hitbox-draw --pak F:KEY --model Flag16m --out F.png` — Hugo's request (2026-10-09
//! 07:28 PT): ONE picture per flag model showing the visual flag against the finish hitbox of the
//! Flags campaign. Everything is measured, nothing drawn from memory: the pole from the item's
//! collision tube, the cloth from the pack dyna mesh's 86 vertex-animation frames placed by the
//! prefab's pose (the time-averaged silhouette = the resting flag, a few translucent frames = the
//! sway), the slab exactly as `mapgeom flags` builds it (`flags::cloth_of` + the same shear rule).
//! SIDE view (horizontal = the cloth direction, local +z; vertical = y) and TOP view (horizontal
//! = local +z, vertical = x, the sway axis), dimension labels in metres, a legend.
use crate::png::{label, Image};

type R<T> = Result<T, String>;

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

/// A tiny vector rasteriser over the RGB `Image`: lines, filled convex polygons (with alpha),
/// hatching, dashed lines, thick strokes.
struct Canvas {
    img: Image,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Canvas {
        let mut img = Image::new(w, h);
        img.fill(0, 0, w, h, [255, 255, 255]);
        Canvas { img }
    }
    fn blend(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.img.w as i64 || y >= self.img.h as i64 {
            return;
        }
        let o = self.img.get(x as usize, y as usize);
        let m = |p: u8, q: u8| ((p as f32) * (1.0 - a) + (q as f32) * a).round().clamp(0.0, 255.0) as u8;
        self.img.set(x as usize, y as usize, [m(o[0], c[0]), m(o[1], c[1]), m(o[2], c[2])]);
    }
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 3], t: f32, a: f32) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let n = (len * 2.0).ceil() as i32;
        let r = (t / 2.0).max(0.5);
        for i in 0..=n {
            let s = i as f32 / n as f32;
            let (cx, cy) = (x0 + dx * s, y0 + dy * s);
            let (ix0, ix1) = ((cx - r).floor() as i64, (cx + r).ceil() as i64);
            let (iy0, iy1) = ((cy - r).floor() as i64, (cy + r).ceil() as i64);
            for py in iy0..=iy1 {
                for px in ix0..=ix1 {
                    let d = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
                    if d <= r {
                        self.blend(px, py, c, a);
                    }
                }
            }
        }
    }
    fn dashed(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 3], t: f32, dash: f32) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let mut s = 0.0;
        while s < len {
            let e = (s + dash).min(len);
            self.line(x0 + dx * s / len, y0 + dy * s / len, x0 + dx * e / len, y0 + dy * e / len, c, t, 1.0);
            s += 2.0 * dash;
        }
    }
    /// A filled polygon (even-odd scanline), with alpha.
    fn poly(&mut self, pts: &[(f32, f32)], c: [u8; 3], a: f32) {
        if pts.len() < 3 {
            return;
        }
        let y0 = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min).floor() as i64;
        let y1 = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max).ceil() as i64;
        for py in y0.max(0)..=y1.min(self.img.h as i64 - 1) {
            let yc = py as f32 + 0.5;
            let mut xs: Vec<f32> = Vec::new();
            for i in 0..pts.len() {
                let (a0, b0) = pts[i];
                let (a1, b1) = pts[(i + 1) % pts.len()];
                if (b0 <= yc && b1 > yc) || (b1 <= yc && b0 > yc) {
                    xs.push(a0 + (yc - b0) / (b1 - b0) * (a1 - a0));
                }
            }
            xs.sort_by(|p, q| p.partial_cmp(q).unwrap());
            for pair in xs.chunks(2) {
                if pair.len() == 2 {
                    for px in (pair[0].round() as i64)..(pair[1].round() as i64) {
                        self.blend(px, py, c, a);
                    }
                }
            }
        }
    }
    fn outline(&mut self, pts: &[(f32, f32)], c: [u8; 3], t: f32) {
        for i in 0..pts.len() {
            let (a0, b0) = pts[i];
            let (a1, b1) = pts[(i + 1) % pts.len()];
            self.line(a0, b0, a1, b1, c, t, 1.0);
        }
    }
    /// Diagonal hatching clipped to a polygon (even-odd), spacing in px.
    fn hatch(&mut self, pts: &[(f32, f32)], c: [u8; 3], spacing: f32, t: f32) {
        let xmin = pts.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let xmax = pts.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let ymin = pts.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let ymax = pts.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        let inside = |x: f32, y: f32| -> bool {
            let mut c = false;
            for i in 0..pts.len() {
                let (a0, b0) = pts[i];
                let (a1, b1) = pts[(i + 1) % pts.len()];
                if ((b0 <= y && b1 > y) || (b1 <= y && b0 > y)) && x < a0 + (y - b0) / (b1 - b0) * (a1 - a0) {
                    c = !c;
                }
            }
            c
        };
        let mut k = xmin - (ymax - ymin);
        while k < xmax {
            // the line x = k + (y - ymin)
            let mut y = ymin;
            let step = 0.7;
            let mut run: Option<(f32, f32)> = None;
            while y <= ymax {
                let x = k + (y - ymin);
                if inside(x, y) {
                    run = Some(run.map(|r| r).unwrap_or((x, y)));
                } else if let Some((sx, sy)) = run.take() {
                    self.line(sx, sy, x, y, c, t, 1.0);
                }
                y += step;
            }
            if let Some((sx, sy)) = run {
                self.line(sx, sy, k + (ymax - ymin), ymax, c, t, 1.0);
            }
            k += spacing;
        }
    }
    fn text(&mut self, x: f32, y: f32, s: &str, scale: usize, c: [u8; 3]) {
        // the glyph painter fills a black box; paint white-on-white instead: draw the glyph box in bg then letters
        let n = s.chars().count();
        let (w, h) = ((n * 6 + 1) * scale, 9 * scale);
        let (xi, yi) = (x.round().max(0.0) as usize, y.round().max(0.0) as usize);
        self.img.fill(xi, yi, w, h, [255, 255, 255]);
        label(&mut self.img, xi, yi, s, scale, c);
        // `label` paints a black background box; repaint it white around the letters
        let snapshot = self.img.clone_region(xi, yi, w, h);
        for yy in 0..h {
            for xx in 0..w {
                let p = snapshot[(yy * w + xx) * 3..(yy * w + xx) * 3 + 3].to_vec();
                if p == [0, 0, 0] {
                    self.img.set(xi + xx, yi + yy, [255, 255, 255]);
                }
            }
        }
    }
    /// A dimension line with end ticks and a label at its middle.
    fn dim(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, text: &str, c: [u8; 3], offset: (f32, f32)) {
        self.line(x0, y0, x1, y1, c, 1.5, 1.0);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (nx, ny) = (-dy / len * 6.0, dx / len * 6.0);
        self.line(x0 - nx, y0 - ny, x0 + nx, y0 + ny, c, 1.5, 1.0);
        self.line(x1 - nx, y1 - ny, x1 + nx, y1 + ny, c, 1.5, 1.0);
        let n = text.chars().count() as f32;
        self.text((x0 + x1) / 2.0 - n * 6.0 * 2.0 / 2.0 + offset.0, (y0 + y1) / 2.0 - 9.0 + offset.1, text, 2, c);
    }
}

impl Image {
    fn clone_region(&self, x: usize, y: usize, w: usize, h: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 3);
        for yy in y..y + h {
            for xx in x..x + w {
                let p = if xx < self.w && yy < self.h { self.get(xx, yy) } else { [255, 255, 255] };
                v.extend_from_slice(&p);
            }
        }
        v
    }
}

/// The measured flag: pole, cloth frames (in the item frame), the slab corners.
struct Measured {
    pole_r: f32,
    pole_y0: f32,
    pole_y1: f32,
    /// per frame: the cloth vertices in the item frame
    frames: Vec<Vec<[f32; 3]>>,
    mean: Vec<[f32; 3]>,
    tris: Vec<[u32; 3]>,
    slab: [[f32; 3]; 8],
    cloth: mapgeom::flags::Cloth,
}

fn measure(store: &mut mapgeom::store::DataStore, model: &str, pad: f32, thick_pad: f32) -> R<Measured> {
    let pack_path = format!("Stadium\\Items\\{model}.Item.Gbx");
    let mut ctx = mapgeom::sttc::Ctx::for_collection(store, "Stadium");
    let (cx, cz, pole_r, pole_y0, pole_y1) = mapgeom::flags::pole_of(&mut ctx, &pack_path, 26)?;
    let _ = (cx, cz);
    let cloth = mapgeom::flags::cloth_of(&mut ctx, &pack_path)?;
    // the frames themselves (the same walk as cloth_of)
    let item = ctx.store.load_model(&pack_path)?;
    let prefab_path = item.externals.iter().map(|(_, p)| p.clone()).find(|p| p.to_lowercase().ends_with(".prefab.gbx")).ok_or("no prefab")?;
    let pm = ctx.store.load_model(&prefab_path)?;
    let prefab = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm)?;
    let mut dyna: Option<(String, [f32; 3], [f32; 4])> = None;
    for e in &prefab.ents {
        if let Some((_, path)) = pm.externals.iter().find(|(k, _)| *k as i32 == e.model.index) {
            if path.to_lowercase().ends_with(".dynaobject.gbx") {
                dyna = Some((path.clone(), e.pos, e.rot));
            }
        }
    }
    let (dpath, pos, q) = dyna.ok_or("no dyna entity")?;
    let mut scratch = mapgeom::static_item::build::Merged::default();
    let src = mapgeom::static_item::build::load_dyna_source(ctx.store, &dpath, &mut scratch, true)?;
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let rot = |v: [f32; 3]| -> [f32; 3] {
        let (vx, vy, vz) = (v[0], v[1], v[2]);
        let (ax, ay, az) = (y * vz - z * vy, z * vx - x * vz, x * vy - y * vx);
        let (bx, by, bz) = (y * az - z * ay, z * ax - x * az, x * ay - y * ax);
        [vx + 2.0 * (w * ax + bx) + pos[0], vy + 2.0 * (w * ay + by) + pos[1], vz + 2.0 * (w * az + bz) + pos[2]]
    };
    let mut best: Option<(usize, Vec<[f32; 3]>, usize, Vec<[u32; 3]>)> = None;
    for vr in &src.s2.visuals {
        let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() else { continue };
        let Some(main) = v.main.as_ref() else { continue };
        let Some(mapgeom::static_item::Node::VertexStream(st)) = main.vertex_streams.first().and_then(|r| r.inline.as_deref()) else { continue };
        let Some(mapgeom::static_item::vstream::Elem::Float3(pts)) = st.elems.first() else { continue };
        let nf = v.sub_visuals.len().max(1);
        let idx = v.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        let tris: Vec<[u32; 3]> = idx.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect();
        if best.as_ref().map(|b| pts.len() > b.1.len()).unwrap_or(true) {
            best = Some((nf, pts.clone(), pts.len() / nf, tris));
        }
    }
    let (nf, pts, per, tris) = best.ok_or("no cloth visual")?;
    let mut frames: Vec<Vec<[f32; 3]>> = Vec::with_capacity(nf);
    let mut mean = vec![[0.0f32; 3]; per];
    for fr in 0..nf {
        let v: Vec<[f32; 3]> = pts[fr * per..(fr + 1) * per].iter().map(|p| rot(*p)).collect();
        for (k, p) in v.iter().enumerate() {
            for c in 0..3 {
                mean[k][c] += p[c] / nf as f32;
            }
        }
        frames.push(v);
    }
    // the slab, exactly as flags.rs builds it
    let t = cloth.tilt_bottom_deg.to_radians().tan();
    let (z_near, z_far) = (cloth.z0 + pole_r, cloth.z0 + cloth.reach + pad);
    let drop = t * (z_far - z_near);
    let (xa, xb) = (cloth.x_lo - thick_pad, cloth.x_hi + thick_pad);
    let (ya, yb) = (cloth.y_lo - pad, cloth.y_hi + pad);
    let slab = [[xa, ya, z_near], [xb, ya, z_near], [xb, yb, z_near], [xa, yb, z_near], [xa, ya - drop, z_far], [xb, ya - drop, z_far], [xb, yb - drop, z_far], [xa, yb - drop, z_far]];
    Ok(Measured { pole_r, pole_y0, pole_y1, frames, mean, tris, slab, cloth })
}

const INK: [u8; 3] = [30, 30, 30];
const POLE: [u8; 3] = [90, 90, 90];
const CLOTH: [u8; 3] = [40, 120, 200];
const SWAY: [u8; 3] = [120, 170, 230];
const SLAB: [u8; 3] = [220, 60, 40];
const DIM: [u8; 3] = [0, 120, 60];

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let model = f(rest, "--model").unwrap_or_else(|| "Flag16m".into());
    let out = f(rest, "--out").unwrap_or_else(|| format!("{}-hitbox.png", model.to_lowercase()));
    let pad: f32 = f(rest, "--cloth-pad").and_then(|v| v.parse().ok()).unwrap_or(0.3);
    let thick_pad: f32 = f(rest, "--cloth-thick-pad").and_then(|v| v.parse().ok()).unwrap_or(0.35);
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
    let m = measure(&mut store, &model, pad, thick_pad)?;
    let (w, h) = (1600usize, 1200usize);
    let mut cv = Canvas::new(w, h);
    let s = &m.slab;
    // ---- SIDE view (left, zoomed on the cloth region): z → right, y → up
    let side_x0 = 150.0f32;
    let z_lo = -1.2f32;
    let z_hi = (m.cloth.z0 + m.cloth.reach + pad + 1.4).max(4.0);
    let y_lo = s.iter().map(|c| c[1]).fold(f32::INFINITY, f32::min).min(m.frames.iter().flatten().map(|p| p[1]).fold(f32::INFINITY, f32::min)) - 1.0;
    let y_hi = m.pole_y1 + 1.0;
    let scale = (820.0 / (z_hi - z_lo)).min(760.0 / (y_hi - y_lo));
    let sx = |z: f32| side_x0 + (z - z_lo) * scale;
    let sy = |y: f32| 900.0 - (y - y_lo) * scale;
    cv.text(60.0, 20.0, &format!("{}  -  FINISH HITBOX VS THE VISUAL FLAG", model.to_uppercase()), 4, INK);
    cv.text(side_x0, 80.0, "SIDE VIEW: ALONG THE CLOTH (+Z RIGHT), Y UP. ZOOMED ON THE CLOTH", 2, INK);
    // sway frames (translucent), every 6th frame
    for (k, fr) in m.frames.iter().enumerate() {
        if k % 6 != 0 {
            continue;
        }
        for t in &m.tris {
            let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = fr[i as usize]; (sx(v[2]), sy(v[1])) }).collect();
            cv.poly(&p, SWAY, 0.08);
        }
    }
    for t in &m.tris {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = m.mean[i as usize]; (sx(v[2]), sy(v[1])) }).collect();
        cv.poly(&p, CLOTH, 0.55);
    }
    // the pole (clipped to the view bottom with a break mark)
    let pole_bottom = sy(y_lo + 0.3);
    let pole = [(sx(-m.pole_r), pole_bottom), (sx(m.pole_r), pole_bottom), (sx(m.pole_r), sy(m.pole_y1)), (sx(-m.pole_r), sy(m.pole_y1))];
    cv.poly(&pole, POLE, 0.9);
    cv.line(sx(-m.pole_r) - 12.0, pole_bottom + 6.0, sx(m.pole_r) + 12.0, pole_bottom - 6.0, [255, 255, 255], 4.0, 1.0);
    cv.line(sx(-m.pole_r) - 12.0, pole_bottom + 14.0, sx(m.pole_r) + 12.0, pole_bottom + 2.0, POLE, 2.0, 1.0);
    cv.line(sx(-m.pole_r) - 12.0, pole_bottom - 2.0, sx(m.pole_r) + 12.0, pole_bottom - 14.0, POLE, 2.0, 1.0);
    // slab (side): corners 0,3,7,4
    let slab_side = [(sx(s[0][2]), sy(s[0][1])), (sx(s[3][2]), sy(s[3][1])), (sx(s[7][2]), sy(s[7][1])), (sx(s[4][2]), sy(s[4][1]))];
    cv.hatch(&slab_side, SLAB, 12.0, 1.2);
    cv.outline(&slab_side, SLAB, 3.0);
    // dimensions (side)
    let near_h = s[3][1] - s[0][1];
    cv.dim(sx(s[0][2]) - 40.0, sy(s[0][1]), sx(s[0][2]) - 40.0, sy(s[3][1]), &format!("{near_h:.2} M"), DIM, (-110.0, 0.0));
    cv.dim(sx(s[0][2]), sy(s[3][1]) - 50.0, sx(s[4][2]), sy(s[3][1]) - 50.0, &format!("{:.2} M ALONG THE CLOTH", s[4][2] - s[0][2]), DIM, (0.0, -24.0));
    let drop = s[3][1] - s[7][1];
    cv.dim(sx(s[4][2]) + 40.0, sy(s[3][1]), sx(s[4][2]) + 40.0, sy(s[7][1]), &format!("{drop:.2} M DOWN"), DIM, (24.0, 0.0));
    cv.text(sx(s[4][2]) + 64.0, (sy(s[3][1]) + sy(s[7][1])) / 2.0 + 14.0, &format!("= {:.1} DEG TILT", m.cloth.tilt_bottom_deg), 2, DIM);
    // pole annotations
    cv.text(sx(0.0) + 20.0, sy(m.pole_y1) - 30.0, &format!("POLE TOP Y {:.2}", m.pole_y1), 2, POLE);
    cv.text(side_x0 - 120.0, sy(y_lo + 0.3) + 20.0, &format!("POLE: R {:.3} M, Y {:.1}..{:.2} (NOT A TRIGGER)", m.pole_r, m.pole_y0, m.pole_y1), 2, POLE);
    // cloth attachment
    cv.text(side_x0 - 120.0, sy(y_lo + 0.3) + 50.0, &format!("CLOTH AT REST: {:.2} M TALL AT THE POLE (Y {:.2}..{:.2}), REACHING {:.2} M", m.cloth.y_hi - m.cloth.y_lo, m.cloth.y_lo, m.cloth.y_hi, m.cloth.reach), 2, CLOTH);
    // y axis ticks (metres)
    let mut yy = (y_lo + 1.0).ceil();
    while yy <= y_hi {
        cv.line(side_x0 - 8.0, sy(yy), side_x0, sy(yy), [170, 170, 170], 1.0, 1.0);
        cv.text(side_x0 - 60.0, sy(yy) - 8.0, &format!("Y {yy:.0}"), 2, [150, 150, 150]);
        yy += 1.0;
    }
    // ---- TOP view (right): z → right, x → down (the sway axis)
    let top_x0 = 1090.0f32;
    let x_all_lo = m.frames.iter().flatten().map(|p| p[0]).fold(f32::INFINITY, f32::min).min(s[0][0]) - 0.6;
    let x_all_hi = m.frames.iter().flatten().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max).max(s[1][0]) + 0.6;
    let tscale = (440.0 / (z_hi - z_lo)).min(440.0 / (x_all_hi - x_all_lo));
    let tx = |z: f32| top_x0 + (z - z_lo) * tscale;
    let ty = |x: f32| 150.0 + (x - x_all_lo) * tscale;
    cv.text(top_x0, 80.0, "TOP VIEW: +Z RIGHT, X (SWAY) DOWN", 2, INK);
    for (k, fr) in m.frames.iter().enumerate() {
        if k % 6 != 0 {
            continue;
        }
        for t in &m.tris {
            let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = fr[i as usize]; (tx(v[2]), ty(v[0])) }).collect();
            cv.poly(&p, SWAY, 0.08);
        }
    }
    for t in &m.tris {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = m.mean[i as usize]; (tx(v[2]), ty(v[0])) }).collect();
        cv.poly(&p, CLOTH, 0.55);
    }
    let pc: Vec<(f32, f32)> = (0..32).map(|k| { let a = k as f32 / 32.0 * std::f32::consts::TAU; (tx(m.pole_r * a.cos()), ty(m.pole_r * a.sin())) }).collect();
    cv.poly(&pc, POLE, 0.9);
    let slab_top = [(tx(s[0][2]), ty(s[0][0])), (tx(s[1][2]), ty(s[1][0])), (tx(s[5][2]), ty(s[5][0])), (tx(s[4][2]), ty(s[4][0]))];
    cv.hatch(&slab_top, SLAB, 12.0, 1.2);
    cv.outline(&slab_top, SLAB, 3.0);
    cv.dim(tx(s[4][2]) + 30.0, ty(s[0][0]), tx(s[4][2]) + 30.0, ty(s[1][0]), &format!("{:.2} M THICK", s[1][0] - s[0][0]), DIM, (24.0, 0.0));
    let xs_lo = m.frames.iter().flatten().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let xs_hi = m.frames.iter().flatten().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
    cv.dashed(tx(z_lo + 0.2), ty(xs_lo), tx(z_hi - 0.2), ty(xs_lo), SWAY, 1.5, 6.0);
    cv.dashed(tx(z_lo + 0.2), ty(xs_hi), tx(z_hi - 0.2), ty(xs_hi), SWAY, 1.5, 6.0);
    cv.text(tx(z_lo + 0.2), ty(xs_lo) - 24.0, &format!("FULL SWING X {:.2}..{:.2} M", xs_lo, xs_hi), 2, SWAY);
    cv.text(tx(z_lo + 0.2), ty(xs_hi) + 8.0, &format!("MEAN SWAY BAND X {:.2}..{:.2} M", m.cloth.x_lo, m.cloth.x_hi), 2, CLOTH);
    // ---- legend (bottom right, boxed)
    let (lx, ly) = (1090.0f32, 700.0f32);
    cv.outline(&[(lx - 16.0, ly - 16.0), (lx + 490.0, ly - 16.0), (lx + 490.0, ly + 300.0), (lx - 16.0, ly + 300.0)], [200, 200, 200], 1.5);
    cv.text(lx, ly, "LEGEND", 2, INK);
    cv.poly(&[(lx, ly + 30.0), (lx + 30.0, ly + 30.0), (lx + 30.0, ly + 46.0), (lx, ly + 46.0)], POLE, 0.9);
    cv.text(lx + 40.0, ly + 32.0, "POLE (COLLISION TUBE), NOT A TRIGGER", 2, INK);
    cv.poly(&[(lx, ly + 60.0), (lx + 30.0, ly + 60.0), (lx + 30.0, ly + 76.0), (lx, ly + 76.0)], CLOTH, 0.55);
    cv.text(lx + 40.0, ly + 62.0, &format!("CLOTH AT REST (MEAN OF {} ANIM FRAMES)", m.frames.len()), 2, INK);
    cv.poly(&[(lx, ly + 90.0), (lx + 30.0, ly + 90.0), (lx + 30.0, ly + 106.0), (lx, ly + 106.0)], SWAY, 0.3);
    cv.text(lx + 40.0, ly + 92.0, "CLOTH SWAY (EVERY 6TH FRAME)", 2, INK);
    cv.hatch(&[(lx, ly + 120.0), (lx + 30.0, ly + 120.0), (lx + 30.0, ly + 136.0), (lx, ly + 136.0)], SLAB, 8.0, 1.0);
    cv.outline(&[(lx, ly + 120.0), (lx + 30.0, ly + 120.0), (lx + 30.0, ly + 136.0), (lx, ly + 136.0)], SLAB, 2.0);
    cv.text(lx + 40.0, ly + 122.0, "FINISH TRIGGER SLAB (INVISIBLE ITEM)", 2, INK);
    cv.text(lx, ly + 160.0, &format!("EDGE PAD {pad} M, THICK = SWAY +/- {thick_pad} M"), 2, INK);
    cv.text(lx, ly + 185.0, &format!("Z {:.2}..{:.2} FROM THE POLE SURFACE", s[0][2], s[4][2]), 2, INK);
    cv.text(lx, ly + 205.0, &format!("Y {:.2}..{:.2} AT THE POLE", s[0][1], s[3][1]), 2, INK);
    cv.text(lx, ly + 225.0, &format!("Y {:.2}..{:.2} AT THE FAR EDGE", s[4][1], s[7][1]), 2, INK);
    cv.text(lx, ly + 245.0, &format!("X {:.2}..{:.2} ACROSS", s[0][0], s[1][0]), 2, INK);
    cv.text(lx, ly + 275.0, "THE SLAB TURNS WITH EACH FLAG (SAME POSE).", 2, INK);
    cv.text(60.0, 1150.0, "MEASURED ON THE PACK MODEL: POLE = ITEM COLLISION TUBE, CLOTH = THE FLAG DYNA MESH VERTEX ANIMATION AT THE PREFAB POSE. ALL METRES.", 2, [120, 120, 120]);
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {}x{}; slab {:?}", w, h, s);
    Ok(())
}
