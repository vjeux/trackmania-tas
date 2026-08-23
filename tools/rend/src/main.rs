// rend -- a small painter's-algorithm renderer for a mapgeom .obj plus ghost
// polylines, so a map can be LOOKED AT from an arbitrary camera.
//
// Deliberately dependency-free: reads the .obj mapgeom writes, projects with a
// pinhole camera, sorts triangles back-to-front, fills them with a flat shade
// from the face normal, then draws the paths on top with depth testing against
// the filled depth buffer. Writes a binary PPM, which anything can convert.
//
// Not a beauty renderer. It exists so the height story -- which a top-down PNG
// cannot show -- is visible.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};

#[derive(Clone, Copy)]
struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

impl V3 {
    fn sub(self, o: V3) -> V3 {
        V3 { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z }
    }
    fn cross(self, o: V3) -> V3 {
        V3 {
            x: self.y * o.z - self.z * o.y,
            y: self.z * o.x - self.x * o.z,
            z: self.x * o.y - self.y * o.x,
        }
    }
    fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn norm(self) -> V3 {
        let l = self.dot(self).sqrt().max(1e-9);
        V3 { x: self.x / l, y: self.y / l, z: self.z / l }
    }
}

struct Tri {
    a: V3,
    b: V3,
    c: V3,
    mat: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut obj = String::new();
    let mut out = String::from("/tmp/out.ppm");
    let mut paths: Vec<(String, [u8; 3])> = Vec::new();
    let mut eye = V3 { x: 0.0, y: 0.0, z: 0.0 };
    let mut at = V3 { x: 0.0, y: 0.0, z: 0.0 };
    let (mut w, mut h) = (1600usize, 1000usize);
    let mut fov = 45.0f32;
    let mut auto = true;
    let mut yaw = 45.0f32;
    let mut pitch = 28.0f32;
    let mut dist_mul = 1.0f32;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--obj" => { obj = args[i + 1].clone(); i += 2; }
            "--out" => { out = args[i + 1].clone(); i += 2; }
            "--path" => {
                let spec = args[i + 1].clone();
                // file:R,G,B
                let (f, c) = match spec.split_once(':') {
                    Some((f, c)) => {
                        let p: Vec<u8> = c.split(',').map(|v| v.parse().unwrap_or(255)).collect();
                        (f.to_string(), [p[0], p[1], p[2]])
                    }
                    None => (spec, [255, 40, 40]),
                };
                paths.push((f, c));
                i += 2;
            }
            "--size" => {
                let p: Vec<usize> = args[i + 1].split('x').map(|v| v.parse().unwrap()).collect();
                w = p[0]; h = p[1]; i += 2;
            }
            "--yaw" => { yaw = args[i + 1].parse().unwrap(); i += 2; }
            "--pitch" => { pitch = args[i + 1].parse().unwrap(); i += 2; }
            "--dist" => { dist_mul = args[i + 1].parse().unwrap(); i += 2; }
            "--fov" => { fov = args[i + 1].parse().unwrap(); i += 2; }
            "--eye" => {
                let p: Vec<f32> = args[i + 1].split(',').map(|v| v.parse().unwrap()).collect();
                eye = V3 { x: p[0], y: p[1], z: p[2] }; auto = false; i += 2;
            }
            "--at" => {
                let p: Vec<f32> = args[i + 1].split(',').map(|v| v.parse().unwrap()).collect();
                at = V3 { x: p[0], y: p[1], z: p[2] }; i += 2;
            }
            x => panic!("unknown flag {x}"),
        }
    }

    // ---- read the obj -------------------------------------------------
    let mut verts: Vec<V3> = Vec::new();
    let mut tris: Vec<Tri> = Vec::new();
    let mut mats: Vec<String> = Vec::new();
    let mut cur = 0usize;
    let f = File::open(&obj).expect("open obj");
    for line in BufReader::new(f).lines() {
        let line = line.unwrap();
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let x: f32 = it.next().unwrap().parse().unwrap();
                let y: f32 = it.next().unwrap().parse().unwrap();
                let z: f32 = it.next().unwrap().parse().unwrap();
                verts.push(V3 { x, y, z });
            }
            Some("usemtl") | Some("o") => {
                let name = it.next().unwrap_or("?").to_string();
                cur = match mats.iter().position(|m| *m == name) {
                    Some(k) => k,
                    None => { mats.push(name); mats.len() - 1 }
                };
            }
            Some("f") => {
                let idx: Vec<usize> = it
                    .map(|t| {
                        let s = t.split('/').next().unwrap();
                        let n: i64 = s.parse().unwrap();
                        if n < 0 { (verts.len() as i64 + n) as usize } else { (n - 1) as usize }
                    })
                    .collect();
                for k in 1..idx.len().saturating_sub(1) {
                    tris.push(Tri { a: verts[idx[0]], b: verts[idx[k]], c: verts[idx[k + 1]], mat: cur });
                }
            }
            _ => {}
        }
    }
    eprintln!("{} verts, {} tris, {} materials", verts.len(), tris.len(), mats.len());

    // ---- read the paths -----------------------------------------------
    let mut polys: Vec<(Vec<V3>, [u8; 3])> = Vec::new();
    for (p, c) in &paths {
        let mut pts = Vec::new();
        let f = File::open(p).expect("open path csv");
        for (n, line) in BufReader::new(f).lines().enumerate() {
            let line = line.unwrap();
            if n == 0 && line.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false) {
                continue;
            }
            let col: Vec<&str> = line.split(',').collect();
            if col.len() < 4 { continue; }
            // mapgeom/tmtraj csv: t,x,y,z,...
            let x: f32 = match col[1].trim().parse() { Ok(v) => v, Err(_) => continue };
            let y: f32 = match col[2].trim().parse() { Ok(v) => v, Err(_) => continue };
            let z: f32 = match col[3].trim().parse() { Ok(v) => v, Err(_) => continue };
            pts.push(V3 { x, y, z });
        }
        eprintln!("path {p}: {} points", pts.len());
        polys.push((pts, *c));
    }

    // ---- camera --------------------------------------------------------
    let mut lo = V3 { x: f32::MAX, y: f32::MAX, z: f32::MAX };
    let mut hi = V3 { x: f32::MIN, y: f32::MIN, z: f32::MIN };
    let acc = |p: V3, lo: &mut V3, hi: &mut V3| {
        lo.x = lo.x.min(p.x); lo.y = lo.y.min(p.y); lo.z = lo.z.min(p.z);
        hi.x = hi.x.max(p.x); hi.y = hi.y.max(p.y); hi.z = hi.z.max(p.z);
    };
    for t in &tris { acc(t.a, &mut lo, &mut hi); acc(t.b, &mut lo, &mut hi); acc(t.c, &mut lo, &mut hi); }
    for (p, _) in &polys { for q in p { acc(*q, &mut lo, &mut hi); } }

    if at.x == 0.0 && at.y == 0.0 && at.z == 0.0 {
        at = V3 { x: (lo.x + hi.x) * 0.5, y: (lo.y + hi.y) * 0.5, z: (lo.z + hi.z) * 0.5 };
    }
    let span = ((hi.x - lo.x).powi(2) + (hi.y - lo.y).powi(2) + (hi.z - lo.z).powi(2)).sqrt();
    if auto {
        let d = span * 0.75 * dist_mul;
        let (yr, pr) = (yaw.to_radians(), pitch.to_radians());
        eye = V3 {
            x: at.x + d * pr.cos() * yr.sin(),
            y: at.y + d * pr.sin(),
            z: at.z + d * pr.cos() * yr.cos(),
        };
    }
    eprintln!("bounds x {:.0}..{:.0}  y {:.0}..{:.0}  z {:.0}..{:.0}", lo.x, hi.x, lo.y, hi.y, lo.z, hi.z);
    eprintln!("eye {:.0},{:.0},{:.0} -> at {:.0},{:.0},{:.0}", eye.x, eye.y, eye.z, at.x, at.y, at.z);

    let fwd = at.sub(eye).norm();
    let up0 = V3 { x: 0.0, y: 1.0, z: 0.0 };
    let right = fwd.cross(up0).norm();
    let up = right.cross(fwd).norm();
    let fscale = (h as f32 * 0.5) / (fov.to_radians() * 0.5).tan();

    let project = |p: V3| -> (f32, f32, f32) {
        let d = p.sub(eye);
        let cx = d.dot(right);
        let cy = d.dot(up);
        let cz = d.dot(fwd);
        if cz <= 0.1 { return (f32::NAN, f32::NAN, cz); }
        (w as f32 * 0.5 + cx * fscale / cz, h as f32 * 0.5 - cy * fscale / cz, cz)
    };

    // ---- raster --------------------------------------------------------
    let mut img = vec![18u8; w * h * 3];
    // faint vertical gradient so the model reads against the background
    for y in 0..h {
        let t = y as f32 / h as f32;
        let v = (16.0 + 26.0 * (1.0 - t)) as u8;
        for x in 0..w {
            let o = (y * w + x) * 3;
            img[o] = v; img[o + 1] = v; img[o + 2] = (v as f32 * 1.15) as u8;
        }
    }
    let mut zbuf = vec![f32::MAX; w * h];

    let base = |m: usize, name: &str| -> [f32; 3] {
        let n = name.to_ascii_lowercase();
        if n.contains("ice") { [0.55, 0.75, 0.95] }
        else if n.contains("dirt") { [0.72, 0.52, 0.30] }
        else if n.contains("grass") { [0.30, 0.50, 0.28] }
        else if n.contains("water") { [0.25, 0.45, 0.70] }
        else if n.contains("asphalt") || n.contains("road") { [0.55, 0.55, 0.58] }
        else if n.contains("metal") { [0.62, 0.62, 0.68] }
        else if n.contains("wood") { [0.60, 0.45, 0.30] }
        else { let k = 0.45 + 0.1 * ((m % 5) as f32 / 5.0); [k, k, k * 1.05] }
    };

    let sun = V3 { x: 0.4, y: 0.85, z: 0.3 }.norm();
    let mut order: Vec<usize> = (0..tris.len()).collect();
    let key = |t: &Tri| -> f32 {
        let c = V3 { x: (t.a.x + t.b.x + t.c.x) / 3.0, y: (t.a.y + t.b.y + t.c.y) / 3.0, z: (t.a.z + t.b.z + t.c.z) / 3.0 };
        c.sub(eye).dot(fwd)
    };
    order.sort_by(|&i, &j| key(&tris[j]).partial_cmp(&key(&tris[i])).unwrap());

    for &ti in &order {
        let t = &tris[ti];
        let (ax, ay, az) = project(t.a);
        let (bx, by, bz) = project(t.b);
        let (cx, cy, cz) = project(t.c);
        if ax.is_nan() || bx.is_nan() || cx.is_nan() { continue; }
        let n = t.b.sub(t.a).cross(t.c.sub(t.a)).norm();
        let lam = (n.dot(sun).abs() * 0.72 + 0.28).min(1.0);
        let col = base(t.mat, &mats[t.mat]);
        let minx = ax.min(bx).min(cx).floor().max(0.0) as usize;
        let maxx = ax.max(bx).max(cx).ceil().min(w as f32 - 1.0) as usize;
        let miny = ay.min(by).min(cy).floor().max(0.0) as usize;
        let maxy = ay.max(by).max(cy).ceil().min(h as f32 - 1.0) as usize;
        if minx >= maxx || miny >= maxy { continue; }
        let area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
        if area.abs() < 1e-6 { continue; }
        for py in miny..=maxy {
            for px in minx..=maxx {
                let fx = px as f32 + 0.5;
                let fy = py as f32 + 0.5;
                let w0 = ((bx - ax) * (fy - ay) - (by - ay) * (fx - ax)) / area;
                let w1 = ((fx - ax) * (cy - ay) - (fy - ay) * (cx - ax)) / area;
                if w0 < 0.0 || w1 < 0.0 || w0 + w1 > 1.0 { continue; }
                let zz = az * (1.0 - w0 - w1) + bz * w1 + cz * w0;
                let o = py * w + px;
                if zz >= zbuf[o] { continue; }
                zbuf[o] = zz;
                let p = o * 3;
                img[p] = (col[0] * lam * 255.0) as u8;
                img[p + 1] = (col[1] * lam * 255.0) as u8;
                img[p + 2] = (col[2] * lam * 255.0) as u8;
            }
        }
    }

    // ---- paths, drawn as fat depth-tested segments ---------------------
    for (pts, c) in &polys {
        for k in 1..pts.len() {
            let (x0, y0, z0) = project(pts[k - 1]);
            let (x1, y1, z1) = project(pts[k]);
            if x0.is_nan() || x1.is_nan() { continue; }
            let steps = (((x1 - x0).abs()).max((y1 - y0).abs()).ceil() as usize).max(1);
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let x = x0 + (x1 - x0) * t;
                let y = y0 + (y1 - y0) * t;
                let z = z0 + (z1 - z0) * t;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let px = x as i32 + dx;
                        let py = y as i32 + dy;
                        if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 { continue; }
                        let o = py as usize * w + px as usize;
                        // a hair in front, so the line is not eaten by the surface it sits on
                        if z > zbuf[o] + 0.8 { continue; }
                        let p = o * 3;
                        img[p] = c[0]; img[p + 1] = c[1]; img[p + 2] = c[2];
                    }
                }
            }
        }
    }

    let mut f = File::create(&out).expect("create out");
    write!(f, "P6\n{} {}\n255\n", w, h).unwrap();
    f.write_all(&img).unwrap();
    eprintln!("wrote {out}");
}
