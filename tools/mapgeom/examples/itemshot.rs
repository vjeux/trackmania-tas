//! Software render of static items side by side: a perspective camera,
//! z-buffered rasterizer, faces coloured by material stem (fixed palette)
//! and Lambert-shaded with the STORED vertex normals (so a normal difference
//! is visible), plus an optional wireframe of triangle edges. The items are
//! laid out along +x with a pitch; each is drawn at the scale it carries.
//! Also writes a per-pixel absolute difference image between item 1 and item
//! 2 when both are given (same camera, same slot), and prints the fraction of
//! differing pixels.
//! Usage: itemshot OUT.png [--wire] [--pitch M] [--yaw DEG] [--pitchdeg DEG]
//!        [--dist M] [--w W] [--h H] ITEM1 [ITEM2 [ITEM3 ...]]
use mapgeom::static_item::vstream::Elem;

struct Mesh {
    tris: Vec<[[f32; 3]; 3]>,
    nrms: Vec<[[f32; 3]; 3]>,
    mats: Vec<usize>,
    names: Vec<String>,
}

fn dec(v: u32) -> [f32; 3] {
    let x = ((v & 0x3FF) as i32) << 22 >> 22;
    let y = ((v >> 10 & 0x3FF) as i32) << 22 >> 22;
    let z = ((v >> 20 & 0x3FF) as i32) << 22 >> 22;
    [x as f32 / 511.0, y as f32 / 511.0, z as f32 / 511.0]
}

fn load(path: &str) -> Mesh {
    let data = std::fs::read(path).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mut m = Mesh { tris: vec![], nrms: vec![], mats: vec![], names: vec![] };
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        if std::env::var("ITEMSHOT_SKIP").map(|s| s.split(',').any(|x| x == stem)).unwrap_or(false) {
            continue;
        }
        let Some(vref) = s2.visuals.get(vi) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() else { continue };
        let st = vis.stream().unwrap();
        let (mut pos, mut nrm) = (Vec::new(), Vec::new());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                Elem::Word(w) if d.name() == 5 => nrm = w.iter().map(|v| dec(*v)).collect(),
                _ => {}
            }
        }
        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        let mid = match m.names.iter().position(|n| *n == stem) {
            Some(i) => i,
            None => {
                m.names.push(stem.clone());
                m.names.len() - 1
            }
        };
        for t in idx.chunks(3) {
            if t.len() < 3 {
                continue;
            }
            m.tris.push([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]);
            let n = |i: u32| nrm.get(i as usize).copied().unwrap_or([0.0, 1.0, 0.0]);
            m.nrms.push([n(t[0]), n(t[1]), n(t[2])]);
            m.mats.push(mid);
        }
    }
    m
}

fn palette(stem: &str) -> [f32; 3] {
    // stable colours per material family
    let s = stem.to_ascii_lowercase();
    if s.contains("roadtech") {
        [0.55, 0.55, 0.58]
    } else if s.contains("trackborder") {
        [0.90, 0.90, 0.92]
    } else if s.contains("technicsspecial") {
        [0.85, 0.35, 0.10]
    } else if s.contains("technicstrim") {
        [0.25, 0.25, 0.28]
    } else if s.contains("technics") {
        [0.35, 0.40, 0.45]
    } else if s.contains("clips") {
        [0.20, 0.60, 0.20]
    } else if s.contains("specialfx") {
        [1.00, 0.80, 0.10]
    } else if s.contains("signoff") {
        [0.30, 0.30, 0.30]
    } else if s.contains("sign") {
        [0.95, 0.60, 0.00]
    } else if s.contains("decalpaint") {
        [0.10, 0.30, 0.90]
    } else if s.contains("decal") {
        [0.60, 0.10, 0.60]
    } else {
        // hash
        let h = s.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        [0.3 + 0.6 * ((h & 0xFF) as f32 / 255.0), 0.3 + 0.6 * (((h >> 8) & 0xFF) as f32 / 255.0), 0.3 + 0.6 * (((h >> 16) & 0xFF) as f32 / 255.0)]
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args[0].clone();
    let wire = args.iter().any(|a| a == "--wire");
    let pitch: f32 = flag(&args, "--pitch").and_then(|v| v.parse().ok()).unwrap_or(24.0);
    let yaw: f32 = flag(&args, "--yaw").and_then(|v| v.parse().ok()).unwrap_or(-35.0f32).to_radians();
    let pitchdeg: f32 = flag(&args, "--pitchdeg").and_then(|v| v.parse().ok()).unwrap_or(30.0f32).to_radians();
    let dist: f32 = flag(&args, "--dist").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let w: usize = flag(&args, "--w").and_then(|v| v.parse().ok()).unwrap_or(1600);
    let h: usize = flag(&args, "--h").and_then(|v| v.parse().ok()).unwrap_or(700);
    let mut files: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--wire" => {}
            "--pitch" | "--yaw" | "--pitchdeg" | "--dist" | "--w" | "--h" | "--diff" | "--skip" => i += 1,
            f => files.push(f.to_string()),
        }
        i += 1;
    }
    let meshes: Vec<Mesh> = files.iter().map(|f| load(f)).collect();
    // scene bounds
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (k, m) in meshes.iter().enumerate() {
        for t in &m.tris {
            for p in t {
                let q = [p[0] + pitch * k as f32, p[1], p[2]];
                for d in 0..3 {
                    lo[d] = lo[d].min(q[d]);
                    hi[d] = hi[d].max(q[d]);
                }
            }
        }
    }
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let radius = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt() / 2.0;
    let dist = if dist > 0.0 { dist } else { radius * 2.2 };
    // camera: looking at center from direction (yaw, pitch)
    let dir = [yaw.sin() * pitchdeg.cos(), pitchdeg.sin(), yaw.cos() * pitchdeg.cos()];
    let eye = [center[0] + dir[0] * dist, center[1] + dir[1] * dist, center[2] + dir[2] * dist];
    let fwd = norm([center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]]);
    let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
    let up = cross(right, fwd);
    let fov = 2.0 * (radius / dist).atan() * 1.05;
    let focal = (h as f32 / 2.0) / (fov / 2.0).tan();
    let project = |p: [f32; 3]| -> [f32; 3] {
        let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let z = dot(d, fwd);
        let x = dot(d, right);
        let y = dot(d, up);
        [w as f32 / 2.0 + focal * x / z, h as f32 / 2.0 - focal * y / z, z]
    };
    let light = norm([0.4, 1.0, 0.3]);
    let mut rgb = vec![0u8; w * h * 3];
    let mut zbuf = vec![f32::MAX; w * h];
    // background: dark blue-grey gradient
    for y in 0..h {
        for x in 0..w {
            let t = y as f32 / h as f32;
            let c = [(40.0 + 30.0 * t) as u8, (44.0 + 30.0 * t) as u8, (56.0 + 30.0 * t) as u8];
            rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&c);
        }
    }
    // per-item images for diffing (items 1 and 2 drawn in the SAME slot)
    let mut slot_imgs: Vec<Vec<u8>> = Vec::new();
    for (k, m) in meshes.iter().enumerate() {
        let off = pitch * k as f32;
        let draw = |rgb: &mut Vec<u8>, zbuf: &mut Vec<f32>, off: f32, wire: bool| {
            for (ti, t) in m.tris.iter().enumerate() {
                let p: Vec<[f32; 3]> = t.iter().map(|q| project([q[0] + off, q[1], q[2]])).collect();
                if p.iter().any(|q| q[2] <= 0.1) {
                    continue;
                }
                let col = palette(&m.names[m.mats[ti]]);
                let (x0, x1) = (p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil().min(w as f32 - 1.0)) as usize);
                let (y0, y1) = (p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize, (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil().min(h as f32 - 1.0)) as usize);
                if x1 < x0 || y1 < y0 {
                    continue;
                }
                let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
                if area.abs() < 1e-6 {
                    continue;
                }
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let px = x as f32 + 0.5;
                        let py = y as f32 + 0.5;
                        let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[2][0] - px) * (p[1][1] - py)) / area;
                        let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[0][0] - px) * (p[2][1] - py)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let z = 1.0 / (w0 / p[0][2] + w1 / p[1][2] + w2 / p[2][2]);
                        let zi = y * w + x;
                        if z >= zbuf[zi] {
                            continue;
                        }
                        zbuf[zi] = z;
                        let n = m.nrms[ti];
                        let nn = norm([n[0][0] * w0 + n[1][0] * w1 + n[2][0] * w2, n[0][1] * w0 + n[1][1] * w1 + n[2][1] * w2, n[0][2] * w0 + n[1][2] * w1 + n[2][2] * w2]);
                        let lam = dot(nn, light).max(0.0);
                        let shade = 0.25 + 0.75 * lam;
                        let is_edge = wire && (w0 < 0.02 || w1 < 0.02 || w2 < 0.02);
                        let c = if is_edge { [0.0, 0.0, 0.0] } else { [col[0] * shade, col[1] * shade, col[2] * shade] };
                        rgb[zi * 3] = (c[0] * 255.0) as u8;
                        rgb[zi * 3 + 1] = (c[1] * 255.0) as u8;
                        rgb[zi * 3 + 2] = (c[2] * 255.0) as u8;
                    }
                }
            }
        };
        draw(&mut rgb, &mut zbuf, off, wire);
        // same-slot render for the diff (slot 0's offset, own buffers)
        let mut r2 = vec![0u8; w * h * 3];
        let mut z2 = vec![f32::MAX; w * h];
        draw(&mut r2, &mut z2, pitch * 0.0, false);
        slot_imgs.push(r2);
    }
    let img = mapgeom::render::Image { w, h, rgb };
    std::fs::write(&out, mapgeom::render::png(&img)).unwrap();
    println!("wrote {} ({} items, {}x{})", out, meshes.len(), w, h);
    let (di, dj): (usize, usize) = flag(&args, "--diff").and_then(|v| { let mut it = v.split(','); Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?)) }).unwrap_or((0, 1));
    if slot_imgs.len() > di.max(dj) {
        let (a, b) = (&slot_imgs[di], &slot_imgs[dj]);
        let mut diff = vec![0u8; w * h * 3];
        let (mut n_diff, mut n_cov) = (0usize, 0usize);
        let mut da_hist: std::collections::BTreeMap<i32, usize> = Default::default();
        for i in 0..w * h {
            let da = (a[i * 3] as i32 - b[i * 3] as i32).abs().max((a[i * 3 + 1] as i32 - b[i * 3 + 1] as i32).abs()).max((a[i * 3 + 2] as i32 - b[i * 3 + 2] as i32).abs());
            let cov = a[i * 3..i * 3 + 3] != [0, 0, 0] || b[i * 3..i * 3 + 3] != [0, 0, 0];
            if cov {
                n_cov += 1;
            }
            if da > 8 {
                n_diff += 1;
                *da_hist.entry((da / 16) as i32).or_insert(0usize) += 1;
                diff[i * 3] = 255;
                diff[i * 3 + 1] = (255 - da.min(255)) as u8;
                diff[i * 3 + 2] = 0;
            } else if cov {
                diff[i * 3] = 60;
                diff[i * 3 + 1] = 60;
                diff[i * 3 + 2] = 60;
            }
        }
        let dout = out.replace(".png", "-diff.png");
        std::fs::write(&dout, mapgeom::render::png(&mapgeom::render::Image { w, h, rgb: diff })).unwrap();
        println!("   diff magnitude histogram (bins of 16/255): {:?}", da_hist);
        println!("item{} vs item{} same-slot diff: {} of {} covered pixels differ (>8/255) = {:.2}% -> {}", di + 1, dj + 1, n_diff, n_cov, 100.0 * n_diff as f64 / n_cov.max(1) as f64, dout);
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt().max(1e-30);
    [v[0] / l, v[1] / l, v[2] / l]
}
