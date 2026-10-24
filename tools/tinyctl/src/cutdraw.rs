//! `tinyctl cutouts-draw --outlines F.tsv --out PNG [--px 12] [--only BLOCK --px 30]` — the contact sheet of the
//! exact-outline cutouts: per block its footprint box, the structure footprint (grey triangles), the cutout
//! polygons (red fill, dark outline, every vertex a dot), name + area; `--only` draws one block large.
use std::collections::BTreeMap;

use crate::flagdraw::{Canvas, INK};

const STRUCT: [u8; 3] = [150, 150, 150];
const CUT: [u8; 3] = [230, 110, 110];
const EDGE: [u8; 3] = [140, 20, 20];
const GRID: [u8; 3] = [225, 225, 225];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

struct Shape {
    size: (i32, i32),
    rings: Vec<(String, Vec<[f32; 2]>)>, // ring label → points (polygons: "p<i> outer" / "p<i> hole n"; structure tris)
    structure: Vec<[[f32; 2]; 3]>,
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let tsv = f(rest, "--outlines").ok_or("--outlines F.tsv")?;
    let out = f(rest, "--out").unwrap_or_else(|| "cutouts-v2-sheet.png".into());
    let only = f(rest, "--only");
    let px: f32 = f(rest, "--px").and_then(|v| v.parse().ok()).unwrap_or(if only.is_some() { 30.0 } else { 12.0 });
    let text = std::fs::read_to_string(&tsv).map_err(|e| format!("{tsv}: {e}"))?;
    let mut shapes: BTreeMap<String, Shape> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut tri_acc: BTreeMap<(String, usize), Vec<[f32; 2]>> = BTreeMap::new();
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 9 {
            continue;
        }
        let name = c[0].to_string();
        if let Some(o) = &only {
            if &name != o {
                continue;
            }
        }
        let size = (c[2].parse().unwrap_or(1), c[3].parse().unwrap_or(1));
        let p = [c[7].parse::<f32>().unwrap_or(0.0), c[8].parse::<f32>().unwrap_or(0.0)];
        if !shapes.contains_key(&name) {
            order.push(name.clone());
        }
        let sh = shapes.entry(name.clone()).or_insert(Shape { size, rings: Vec::new(), structure: Vec::new() });
        if c[5] == "structure" {
            let ti: usize = c[4].parse().unwrap_or(0);
            let e = tri_acc.entry((name.clone(), ti)).or_default();
            e.push(p);
            if e.len() == 3 {
                sh.structure.push([e[0], e[1], e[2]]);
            }
        } else {
            let label = format!("p{} {}", c[4], c[5]);
            match sh.rings.last_mut() {
                Some((l, pts)) if *l == label => pts.push(p),
                _ => sh.rings.push((label, vec![p])),
            }
        }
    }
    if shapes.is_empty() {
        return Err("no shapes".into());
    }
    // layout: tiles of (size.x*32*px + 20) wide, rows wrap at 2400 px
    let max_w = if only.is_some() { 4000.0 } else { 2400.0 };
    let mut tiles: Vec<(String, f32, f32)> = Vec::new(); // name, x, y
    let (mut cx, mut cy, mut row_h) = (10.0f32, 10.0f32, 0.0f32);
    for name in &order {
        let sh = &shapes[name];
        let (tw, th) = (sh.size.0 as f32 * 32.0 * px + 20.0, sh.size.1 as f32 * 32.0 * px + 44.0);
        if cx + tw > max_w && cx > 10.0 {
            cx = 10.0;
            cy += row_h + 10.0;
            row_h = 0.0;
        }
        tiles.push((name.clone(), cx, cy));
        cx += tw + 10.0;
        row_h = row_h.max(th);
    }
    let total_h = cy + row_h + 20.0;
    let total_w = if only.is_some() { cx + 10.0 } else { max_w };
    let mut cv = Canvas::new(total_w as usize, total_h as usize);
    for (name, tx, ty) in &tiles {
        let sh = &shapes[name];
        let (w, h) = (sh.size.0 as f32 * 32.0, sh.size.1 as f32 * 32.0);
        // block frame: x right, z UP (flip z so north is up)
        let sx = |x: f32| tx + 10.0 + x * px;
        let sz = |z: f32| ty + 34.0 + (h - z) * px;
        cv.poly(&[(sx(0.0), sz(0.0)), (sx(w), sz(0.0)), (sx(w), sz(h)), (sx(0.0), sz(h))], [246, 248, 240], 1.0);
        for gx in 0..=sh.size.0 {
            cv.line(sx(32.0 * gx as f32), sz(0.0), sx(32.0 * gx as f32), sz(h), GRID, 1.0, 1.0);
        }
        for gz in 0..=sh.size.1 {
            cv.line(sx(0.0), sz(32.0 * gz as f32), sx(w), sz(32.0 * gz as f32), GRID, 1.0, 1.0);
        }
        for t in &sh.structure {
            cv.poly(&[(sx(t[0][0]), sz(t[0][1])), (sx(t[1][0]), sz(t[1][1])), (sx(t[2][0]), sz(t[2][1]))], STRUCT, 0.5);
        }
        let mut area = 0.0f32;
        let mut nverts = 0usize;
        for (label, pts) in &sh.rings {
            let poly: Vec<(f32, f32)> = pts.iter().map(|p| (sx(p[0]), sz(p[1]))).collect();
            let a = mapgeom::floor_finish::polygon_area(pts);
            area += a;
            nverts += pts.len();
            if label.contains("outer") {
                cv.poly(&poly, CUT, 0.75);
            } else {
                cv.poly(&poly, [246, 248, 240], 1.0);
            }
            cv.outline(&poly, EDGE, 1.5);
            if px >= 20.0 {
                for (x, y) in &poly {
                    cv.poly(&[(x - 2.0, y - 2.0), (x + 2.0, y - 2.0), (x + 2.0, y + 2.0), (x - 2.0, y + 2.0)], INK, 1.0);
                }
            }
        }
        cv.outline(&[(sx(0.0), sz(0.0)), (sx(w), sz(0.0)), (sx(w), sz(h)), (sx(0.0), sz(h))], [120, 120, 120], 1.0);
        cv.text(tx + 10.0, ty + 4.0, name, 1, INK);
        cv.text(tx + 10.0, ty + 16.0, &format!("{:.0} m2, {} verts, {} rings", area, nverts, sh.rings.len()), 1, [90, 90, 90]);
    }
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {}x{} px, {} blocks", total_w as usize, total_h as usize, tiles.len());
    Ok(())
}
