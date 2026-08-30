// ringscan -- read a mapgeom .obj and report, for a structure that is a ring,
// where its OPENING is: rasterise the collidable triangles onto the plane the
// ring stands in and find the empty region.
//
// usage: ringscan FILE.obj [--plane xy|xz|yz] [--res M] [--off X,Y,Z]
//        --off translates local model coords into world coords.
use std::env;
use std::fs;

#[derive(Clone, Copy)]
struct V3 {
    x: f64,
    y: f64,
    z: f64,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];
    let mut plane = "xy".to_string();
    let mut res = 0.5f64;
    let mut off = V3 { x: 0.0, y: 0.0, z: 0.0 };
    let mut skip_mats: Vec<String> = vec!["NotCollidable".into()];
    let mut bbox: Option<(V3, V3)> = None;
    let mut only_mat: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--box" => {
                let (a, b) = args[i + 1].split_once(':').unwrap();
                let p: Vec<f64> = a.split(',').map(|s| s.parse().unwrap()).collect();
                let q: Vec<f64> = b.split(',').map(|s| s.parse().unwrap()).collect();
                bbox = Some((
                    V3 { x: p[0], y: p[1], z: p[2] },
                    V3 { x: q[0], y: q[1], z: q[2] },
                ));
                i += 2;
            }
            "--plane" => {
                plane = args[i + 1].clone();
                i += 2;
            }
            "--res" => {
                res = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--off" => {
                let p: Vec<f64> = args[i + 1].split(',').map(|s| s.parse().unwrap()).collect();
                off = V3 { x: p[0], y: p[1], z: p[2] };
                i += 2;
            }
            "--all-mats" => {
                skip_mats.clear();
                i += 1;
            }
            "--only-mat" => {
                only_mat = Some(args[i + 1].clone());
                skip_mats.clear();
                i += 2;
            }
            _ => panic!("unknown arg {}", args[i]),
        }
    }

    let text = fs::read_to_string(path).expect("read obj");
    let mut verts: Vec<V3> = Vec::new();
    let mut tris: Vec<([usize; 3], String)> = Vec::new();
    let mut cur_mat = String::from("?");
    for line in text.lines() {
        if let Some(r) = line.strip_prefix("v ") {
            let p: Vec<f64> = r.split_whitespace().map(|s| s.parse().unwrap()).collect();
            verts.push(V3 { x: p[0], y: p[1], z: p[2] });
        } else if let Some(r) = line.strip_prefix("o ") {
            cur_mat = r.trim().to_string();
        } else if let Some(r) = line.strip_prefix("usemtl ") {
            cur_mat = r.trim().to_string();
        } else if let Some(r) = line.strip_prefix("f ") {
            let idx: Vec<usize> = r
                .split_whitespace()
                .map(|s| s.split('/').next().unwrap().parse::<usize>().unwrap() - 1)
                .collect();
            for k in 1..idx.len() - 1 {
                tris.push(([idx[0], idx[k], idx[k + 1]], cur_mat.clone()));
            }
        }
    }
    eprintln!("{} verts, {} tris", verts.len(), tris.len());

    // pick the two in-plane axes and the through axis
    let (ai, bi, ci): (usize, usize, usize) = match plane.as_str() {
        "xy" => (0, 1, 2),
        "xz" => (0, 2, 1),
        "yz" => (1, 2, 0),
        _ => panic!("plane"),
    };
    let comp = |v: &V3, k: usize| match k {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    };
    let offc = |k: usize| match k {
        0 => off.x,
        1 => off.y,
        _ => off.z,
    };

    let sel: Vec<&([usize; 3], String)> = tris
        .iter()
        .filter(|(_, m)| match &only_mat { Some(o) => m.contains(o.as_str()), None => !skip_mats.iter().any(|s| m.contains(s.as_str())) })
        .filter(|(t, _)| match &bbox {
            None => true,
            Some((lo, hi)) => t.iter().all(|&v| {
                let p = &verts[v];
                p.x + off.x >= lo.x
                    && p.x + off.x <= hi.x
                    && p.y + off.y >= lo.y
                    && p.y + off.y <= hi.y
                    && p.z + off.z >= lo.z
                    && p.z + off.z <= hi.z
            }),
        })
        .collect();
    eprintln!("{} collidable tris (skipping {:?})", sel.len(), skip_mats);

    let mut amin = f64::MAX;
    let mut amax = f64::MIN;
    let mut bmin = f64::MAX;
    let mut bmax = f64::MIN;
    let mut cmin = f64::MAX;
    let mut cmax = f64::MIN;
    for (t, _) in sel.iter() {
        for &v in t.iter() {
            let p = &verts[v];
            amin = amin.min(comp(p, ai));
            amax = amax.max(comp(p, ai));
            bmin = bmin.min(comp(p, bi));
            bmax = bmax.max(comp(p, bi));
            cmin = cmin.min(comp(p, ci));
            cmax = cmax.max(comp(p, ci));
        }
    }
    println!(
        "collidable bounds (world): a[{}] {:.2}..{:.2}  b[{}] {:.2}..{:.2}  through[{}] {:.2}..{:.2}",
        axname(ai),
        amin + offc(ai),
        amax + offc(ai),
        axname(bi),
        bmin + offc(bi),
        bmax + offc(bi),
        axname(ci),
        cmin + offc(ci),
        cmax + offc(ci)
    );

    let na = ((amax - amin) / res).ceil() as usize + 1;
    let nb = ((bmax - bmin) / res).ceil() as usize + 1;
    let mut grid = vec![false; na * nb];
    for (t, _) in sel.iter() {
        let p: Vec<(f64, f64)> = t
            .iter()
            .map(|&v| (comp(&verts[v], ai), comp(&verts[v], bi)))
            .collect();
        let ta0 = p.iter().map(|q| q.0).fold(f64::MAX, f64::min);
        let ta1 = p.iter().map(|q| q.0).fold(f64::MIN, f64::max);
        let tb0 = p.iter().map(|q| q.1).fold(f64::MAX, f64::min);
        let tb1 = p.iter().map(|q| q.1).fold(f64::MIN, f64::max);
        let i0 = (((ta0 - amin) / res).floor() as isize).max(0) as usize;
        let i1 = ((((ta1 - amin) / res).ceil() as isize).min(na as isize - 1)).max(0) as usize;
        let j0 = (((tb0 - bmin) / res).floor() as isize).max(0) as usize;
        let j1 = ((((tb1 - bmin) / res).ceil() as isize).min(nb as isize - 1)).max(0) as usize;
        for gi in i0..=i1 {
            for gj in j0..=j1 {
                if grid[gi * nb + gj] {
                    continue;
                }
                let ca = amin + (gi as f64 + 0.5) * res;
                let cb = bmin + (gj as f64 + 0.5) * res;
                if point_in_tri((ca, cb), p[0], p[1], p[2]) {
                    grid[gi * nb + gj] = true;
                }
            }
        }
    }

    // ASCII map, downsampled to <= 110 cols
    let step = (na / 110).max(1);
    let stepb = (nb / 60).max(1);
    println!("\nplane {} occupancy (# solid, . empty), res {} m, downsample {}x{}", plane, res, step, stepb);
    print!("      ");
    for gi in (0..na).step_by(step) {
        let ca = amin + (gi as f64) * res + offc(ai);
        if gi % (step * 10) == 0 {
            print!("{:<10.0}", ca);
        }
    }
    println!();
    for gj in (0..nb).step_by(stepb).collect::<Vec<_>>().into_iter().rev() {
        let cb = bmin + (gj as f64) * res + offc(bi);
        print!("{:6.1}", cb);
        for gi in (0..na).step_by(step) {
            let mut solid = false;
            for dj in 0..stepb {
                for di in 0..step {
                    if gj + dj < nb && gi + di < na && grid[(gi + di) * nb + (gj + dj)] {
                        solid = true;
                    }
                }
            }
            print!("{}", if solid { '#' } else { '.' });
        }
        println!();
    }

    // largest empty axis-aligned box, by scanning rows (histogram method)
    let (best, ba, bb) = largest_empty(&grid, na, nb);
    let (i0, j0, i1, j1) = best;
    println!(
        "\nlargest empty box: {}[{:.2} .. {:.2}]  {}[{:.2} .. {:.2}]   ({:.1} x {:.1} m, area {:.0})",
        axname(ai),
        amin + i0 as f64 * res + offc(ai),
        amin + (i1 + 1) as f64 * res + offc(ai),
        axname(bi),
        bmin + j0 as f64 * res + offc(bi),
        bmin + (j1 + 1) as f64 * res + offc(bi),
        ba as f64 * res,
        bb as f64 * res,
        ba as f64 * bb as f64 * res * res
    );
    let ca = amin + ((i0 + i1 + 1) as f64 / 2.0) * res + offc(ai);
    let cb = bmin + ((j0 + j1 + 1) as f64 / 2.0) * res + offc(bi);
    println!("opening centre: {} {:.2}   {} {:.2}   through-axis {} {:.2}..{:.2}",
        axname(ai), ca, axname(bi), cb, axname(ci), cmin + offc(ci), cmax + offc(ci));
}

fn axname(i: usize) -> &'static str {
    ["x", "y", "z"][i]
}

fn point_in_tri(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let d = |u: (f64, f64), v: (f64, f64), w: (f64, f64)| {
        (v.0 - u.0) * (w.1 - u.1) - (v.1 - u.1) * (w.0 - u.0)
    };
    let d1 = d(p, a, b);
    let d2 = d(p, b, c);
    let d3 = d(p, c, a);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn largest_empty(grid: &[bool], na: usize, nb: usize) -> ((usize, usize, usize, usize), usize, usize) {
    // heights[j] = number of consecutive empty cells ending at column i, row j
    let mut heights = vec![0usize; nb];
    let mut best = ((0, 0, 0, 0), 0usize, 0usize);
    let mut best_area = 0usize;
    for i in 0..na {
        for j in 0..nb {
            heights[j] = if grid[i * nb + j] { 0 } else { heights[j] + 1 };
        }
        // largest rectangle in histogram
        let mut stack: Vec<(usize, usize)> = Vec::new(); // (start_j, height)
        for j in 0..=nb {
            let h = if j == nb { 0 } else { heights[j] };
            let mut start = j;
            while let Some(&(s, hh)) = stack.last() {
                if hh <= h {
                    break;
                }
                stack.pop();
                let area = hh * (j - s);
                if area > best_area {
                    best_area = area;
                    best = ((i + 1 - hh, s, i, j - 1), hh, j - s);
                }
                start = s;
            }
            if stack.last().map_or(true, |&(_, hh)| hh < h) {
                stack.push((start, h));
            }
        }
    }
    best
}
