//! True lightmap overlap test: rasterize every uv1 triangle of every visual
//! with a uv1 stream into an NxN grid (texel centers, strict interior);
//! count texels covered by 2+ triangles that are not neighbours (share no
//! position) -- shared-edge neighbours legitimately touch. Also reports
//! coverage and fold-over (negative-area) triangles.
//! Usage: uv1overlap FILE [N=1024]
use mapgeom::static_item::vstream::Elem;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1024);
    let data = std::fs::read(&a[1]).unwrap();
    let f = mapgeom::static_item::file::parse_file(&data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    // grid cell -> list of (visual, tri) owners (small)
    let mut owner: Vec<Vec<(u16, u32)>> = vec![Vec::new(); n * n];
    let mut tri_pos: Vec<Vec<[[u32; 3]; 3]>> = Vec::new();
    let mut tri_uv: Vec<Vec<[[f32; 2]; 3]>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut neg = 0usize;
    let mut total_tris = 0usize;
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let mat = s2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("").to_string()).unwrap_or("".into());
        let stem = mat.rsplit('\\').next().unwrap_or(&mat).to_string();
        let Some(vref) = s2.visuals.get(vi) else { continue };
        let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() else { continue };
        let st = vis.stream().unwrap();
        let (mut pos, mut uv1) = (Vec::new(), Vec::new());
        for (d, e) in st.decls.iter().zip(st.elems.iter()) {
            match e {
                Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                Elem::Float2(u) if d.name() == 11 => uv1 = u.clone(),
                _ => {}
            }
        }
        if uv1.is_empty() { continue; }
        let vid = names.len() as u16;
        names.push(stem);
        tri_pos.push(Vec::new());
        tri_uv.push(Vec::new());
        let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
        for (ti, t) in idx.chunks(3).enumerate() {
            if t.len() < 3 { continue; }
            total_tris += 1;
            let q = [uv1[t[0] as usize], uv1[t[1] as usize], uv1[t[2] as usize]];
            let pk = |p: [f32; 3]| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
            tri_pos[vid as usize].push([pk(pos[t[0] as usize]), pk(pos[t[1] as usize]), pk(pos[t[2] as usize])]);
            tri_uv[vid as usize].push(q);
            let area = (q[1][0] - q[0][0]) * (q[2][1] - q[0][1]) - (q[2][0] - q[0][0]) * (q[1][1] - q[0][1]);
            if area < 0.0 { neg += 1; }
            if area.abs() < 1e-12 { continue; }
            // raster
            let (x0, x1) = (q.iter().map(|v| v[0]).fold(f32::MAX, f32::min), q.iter().map(|v| v[0]).fold(f32::MIN, f32::max));
            let (y0, y1) = (q.iter().map(|v| v[1]).fold(f32::MAX, f32::min), q.iter().map(|v| v[1]).fold(f32::MIN, f32::max));
            let cx0 = ((x0 * n as f32).floor().max(0.0)) as usize;
            let cx1 = ((x1 * n as f32).ceil().min(n as f32)) as usize;
            let cy0 = ((y0 * n as f32).floor().max(0.0)) as usize;
            let cy1 = ((y1 * n as f32).ceil().min(n as f32)) as usize;
            for cy in cy0..cy1 {
                for cx in cx0..cx1 {
                    let px = (cx as f32 + 0.5) / n as f32;
                    let py = (cy as f32 + 0.5) / n as f32;
                    // barycentric strict interior
                    let e = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0]);
                    let (w0, w1, w2) = (e(q[0], q[1]), e(q[1], q[2]), e(q[2], q[0]));
                    let s = area.signum();
                    if w0 * s > 1e-9 && w1 * s > 1e-9 && w2 * s > 1e-9 {
                        owner[cy * n + cx].push((vid, ti as u32));
                    }
                }
            }
        }
    }
    let mut covered = 0usize;
    let mut conflict = 0usize;
    let mut shown_pairs = 0usize;
    let mut conflict_pairs: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for cell in &owner {
        if cell.is_empty() { continue; }
        covered += 1;
        if cell.len() < 2 { continue; }
        // any two owners sharing no position are a real overlap
        let mut bad = false;
        'outer: for i in 0..cell.len() {
            for j in i + 1..cell.len() {
                let (va, ta) = cell[i];
                let (vb, tb) = cell[j];
                let pa = tri_pos[va as usize][ta as usize];
                let pb = tri_pos[vb as usize][tb as usize];
                let shares = pa.iter().any(|p| pb.contains(p));
                if !shares {
                    if shown_pairs < 6 {
                        shown_pairs += 1;
                        println!("   conflict: {} tri{} uv={:?}  vs  {} tri{} uv={:?}", names[va as usize], ta, tri_uv[va as usize][ta as usize], names[vb as usize], tb, tri_uv[vb as usize][tb as usize]);
                    }
                    bad = true;
                    *conflict_pairs.entry((names[va as usize].clone(), names[vb as usize].clone())).or_insert(0) += 1;
                    break 'outer;
                }
            }
        }
        if bad { conflict += 1; }
    }
    println!("{}: grid {}x{} tris={} covered texels={} ({:.1}%) conflicting texels={} ({:.3}% of covered) fold-over tris={}", a[1].rsplit('/').next().unwrap(), n, n, total_tris, covered, 100.0 * covered as f64 / (n * n) as f64, conflict, 100.0 * conflict as f64 / covered.max(1) as f64, neg);
    let mut v: Vec<_> = conflict_pairs.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, c) in v.iter().take(8) { println!("   {} x {}: {} texels", k.0, k.1, c); }
}
