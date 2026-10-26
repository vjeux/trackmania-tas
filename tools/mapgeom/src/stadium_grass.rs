//! `mapgeom stadium-grass --deco 48x48Screen155 --out FinishStadiumPenaltyGrass.Item.Gbx` — Hugo's penalty-grass item
//! (2026-10-26 11:46): the Stadium decoration's grass between the 48 × 48 grid edge (0..1536 m) and the stands, as a
//! SMOOTH exact polygon traced from the decoration's own stand / grass meshes (the cutouts' marching squares + snap
//! to the mesh boundary + 0.5 m structure inset), in MAP coordinates with the grass at local y 0 (the C# places the
//! item at (0, grassLevel, 0) = (0, 8, 0)), band 0..`above`: PureFloor4-recipe crystal with glass caps top and bottom
//! and the trigger prism of the same polygon(s). Verified here by a 0.5 m raster against the decoration's grass
//! collision and the stands' footprint, and compared with the 4 m strips of OuterTerrain.tsv.

type Tri = [[f32; 3]; 3];

pub struct GrassItem {
    pub bytes: Vec<u8>,
    pub outline: crate::cutout_outline::Outline,
    pub notes: Vec<String>,
    /// (samples in polygon, over no grass, under structure, grass samples missed by the polygon, grass samples total)
    pub check: (usize, usize, usize, usize, usize),
    pub margin: f32,
}

fn upward(t: &Tri) -> bool {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    len > 1e-9 && n[1] / len > 0.9
}

/// Clip a convex polygon (in xz) to the half-plane `a·x + b·z <= c`.
fn clip_half(poly: &[[f32; 3]], a: f32, b: f32, c: f32) -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let (dp, dq) = (a * p[0] + b * p[2] - c, a * q[0] + b * q[2] - c);
        if dp <= 0.0 { out.push(p); }
        if (dp < 0.0 && dq > 0.0) || (dp > 0.0 && dq < 0.0) {
            let t = dp / (dp - dq);
            out.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1]), p[2] + t * (q[2] - p[2])]);
        }
    }
    out
}

fn fan(poly: &[[f32; 3]]) -> Vec<Tri> {
    (1..poly.len().saturating_sub(1)).map(|i| [poly[0], poly[i], poly[i + 1]]).collect()
}

/// The parts of a triangle OUTSIDE the grid rectangle [0, gx] × [0, gz], as triangles (four non-overlapping regions:
/// x < 0; x > gx; 0 ≤ x ≤ gx with z < 0; 0 ≤ x ≤ gx with z > gz).
fn outside_grid(t: &Tri, gx: f32, gz: f32) -> Vec<Tri> {
    let mut out = Vec::new();
    let base: Vec<[f32; 3]> = t.to_vec();
    // x < 0
    let west = clip_half(&base, 1.0, 0.0, 0.0);
    out.extend(fan(&west));
    // x > gx
    let east = clip_half(&base, -1.0, 0.0, -gx);
    out.extend(fan(&east));
    // 0 <= x <= gx band
    let mid = clip_half(&clip_half(&base, -1.0, 0.0, 0.0), 1.0, 0.0, gx);
    if mid.len() >= 3 {
        let south = clip_half(&mid, 0.0, 1.0, 0.0);
        out.extend(fan(&south));
        let north = clip_half(&mid, 0.0, -1.0, -gz);
        out.extend(fan(&north));
    }
    out.into_iter().filter(|t| ((t[1][0] - t[0][0]) * (t[2][2] - t[0][2]) - (t[2][0] - t[0][0]) * (t[1][2] - t[0][2])).abs() > 1e-6).collect()
}

fn covers(p: [f32; 2], t: &Tri) -> bool {
    let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
    let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
    if s(a, b, c).abs() < 1e-6 { return false; }
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    !((d1 < -1e-5 || d2 < -1e-5 || d3 < -1e-5) && (d1 > 1e-5 || d2 > 1e-5 || d3 > 1e-5))
}

/// 2 m buckets over a triangle set for the raster checks.
struct Grid {
    cell: f32,
    origin: [f32; 2],
    nx: usize,
    nz: usize,
    buckets: Vec<Vec<usize>>,
    tris: Vec<Tri>,
}

impl Grid {
    fn new(tris: Vec<Tri>, origin: [f32; 2], size: [f32; 2], cell: f32) -> Grid {
        let (nx, nz) = ((size[0] / cell).ceil() as usize + 1, (size[1] / cell).ceil() as usize + 1);
        let mut buckets = vec![Vec::new(); nx * nz];
        for (i, t) in tris.iter().enumerate() {
            let (x0, x1) = (t.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max));
            let (z0, z1) = (t.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[2]).fold(f32::NEG_INFINITY, f32::max));
            let (i0, i1) = ((((x0 - origin[0]) / cell).floor() as i64).clamp(0, nx as i64 - 1), (((x1 - origin[0]) / cell).floor() as i64).clamp(0, nx as i64 - 1));
            let (j0, j1) = ((((z0 - origin[1]) / cell).floor() as i64).clamp(0, nz as i64 - 1), (((z1 - origin[1]) / cell).floor() as i64).clamp(0, nz as i64 - 1));
            for j in j0..=j1 { for ii in i0..=i1 { buckets[j as usize * nx + ii as usize].push(i); } }
        }
        Grid { cell, origin, nx, nz, buckets, tris }
    }
    fn hit(&self, p: [f32; 2]) -> bool {
        let i = ((p[0] - self.origin[0]) / self.cell).floor() as i64;
        let j = ((p[1] - self.origin[1]) / self.cell).floor() as i64;
        if i < 0 || j < 0 || i >= self.nx as i64 || j >= self.nz as i64 { return false; }
        self.buckets[j as usize * self.nx + i as usize].iter().any(|&k| covers(p, &self.tris[k]))
    }
}

pub fn build(store: &mut crate::store::DataStore, deco_id: &str, grid_cells: i32, margin_cells: i32, inset: f32, step: f32, above: f32, ident: &str, strips: &[(f32, f32, f32, f32)]) -> Result<GrassItem, String> {
    let (pieces, mut notes, deco_map) = crate::outer_ring::stadium_deco_pieces(store, deco_id)?;
    let grass_y = 8.0f32;
    let (gx, gz) = (32.0 * grid_cells as f32, 32.0 * grid_cells as f32);
    let margin = 32.0 * margin_cells as f32;
    // tags present
    let mut tags: std::collections::BTreeMap<String, (usize, f32, f32)> = Default::default();
    for (g, t) in &pieces {
        let e = tags.entry(g.clone()).or_insert((0, f32::INFINITY, f32::NEG_INFINITY));
        e.0 += 1;
        for v in t { e.1 = e.1.min(v[1]); e.2 = e.2.max(v[1]); }
    }
    notes.push(format!("{deco_map}: tags {}", tags.iter().map(|(k, (n, lo, hi))| format!("{k} ×{n} y {lo:.1}..{hi:.1}")).collect::<Vec<_>>().join(", ")));
    // fill = the grass collision (Green physics 76 / a Grass group), upward, at the grass level; structure = the
    // stands' walls over it (the 4 m strips' rule: top over grass + 0.3 and bottom under grass + 3.3)
    let is_grass = |g: &str| g.contains("phys76") || g.to_ascii_lowercase().contains("grass");
    let mut fill: Vec<Tri> = Vec::new();
    let mut structure: Vec<Tri> = Vec::new();
    for (g, t) in &pieces {
        let (y0, y1) = (t.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min), t.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max));
        if is_grass(g) && upward(t) && (y0 - grass_y).abs() <= 0.3 && (y1 - grass_y).abs() <= 0.3 {
            fill.extend(outside_grid(t, gx, gz));
        } else if y1 > grass_y + crate::outer_ring::WALL_ABOVE && y0 < grass_y + crate::outer_ring::WALL_UNDER + crate::outer_ring::WALL_ABOVE {
            structure.push(*t);
        }
    }
    notes.push(format!("fill: {} grass triangles outside the grid; structure: {} triangles within 0.3..3.3 m over the grass", fill.len(), structure.len()));
    if fill.is_empty() { return Err("no grass fill found".into()); }
    // trace in a box frame: map (x, z) → box (x + margin, z + margin), box size grid + 2·margin
    let shift = |t: &Tri| -> Tri { [[t[0][0] + margin, t[0][1], t[0][2] + margin], [t[1][0] + margin, t[1][1], t[1][2] + margin], [t[2][0] + margin, t[2][1], t[2][2] + margin]] };
    let fill_b: Vec<Tri> = fill.iter().map(shift).collect();
    let struct_b: Vec<Tri> = structure.iter().map(shift).collect();
    let size = (gx + 2.0 * margin, gz + 2.0 * margin);
    let mut o = crate::cutout_outline::trace(&fill_b, &struct_b, size, inset, step);
    // back to the map frame, slivers dropped
    for (outer, holes) in &mut o.polygons {
        for p in outer.iter_mut() { p[0] -= margin; p[1] -= margin; }
        for h in holes.iter_mut() { for p in h.iter_mut() { p[0] -= margin; p[1] -= margin; } }
    }
    let before = o.polygons.len();
    o.polygons.retain(|(outer, holes)| {
        let area = crate::floor_finish::polygon_area(outer) + holes.iter().map(|h| crate::floor_finish::polygon_area(h)).sum::<f32>();
        let perim: f32 = (0..outer.len()).map(|i| { let (a, b) = (outer[i], outer[(i + 1) % outer.len()]); ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }).sum();
        area >= 1.0 && perim > 0.0 && area / perim >= 0.3
    });
    o.area = o.polygons.iter().map(|(outer, holes)| crate::floor_finish::polygon_area(outer) + holes.iter().map(|h| crate::floor_finish::polygon_area(h)).sum::<f32>()).sum();
    o.vertices = o.polygons.iter().map(|(outer, holes)| outer.len() + holes.iter().map(|h| h.len()).sum::<usize>()).sum();
    notes.push(format!("outline: {} polygons ({} slivers dropped), {} vertices, {:.0} m², {} on the mesh boundary", o.polygons.len(), before - o.polygons.len(), o.vertices, o.area, o.on_mesh));
    // does the region touch the box edge (margin too small)?
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for (outer, _) in &o.polygons { for p in outer { lo[0] = lo[0].min(p[0]); lo[1] = lo[1].min(p[1]); hi[0] = hi[0].max(p[0]); hi[1] = hi[1].max(p[1]); } }
    if lo[0] < -margin + 1.0 || lo[1] < -margin + 1.0 || hi[0] > gx + margin - 1.0 || hi[1] > gz + margin - 1.0 {
        notes.push(format!("WARNING: the polygon reaches the trace box edge (extent x {:.1}..{:.1} z {:.1}..{:.1}, margin {margin:.0}) — enlarge --margin-cells", lo[0], hi[0], lo[1], hi[1]));
    } else {
        notes.push(format!("extent x {:.1}..{:.1} z {:.1}..{:.1} (grid 0..{gx:.0}; margin {margin:.0} m)", lo[0], hi[0], lo[1], hi[1]));
    }
    // the crystal: caps at 0 (down) and `above` (up) + the trigger prism 0..above
    let ((tv, tt), (iv, it)) = crate::cutout_outline::extrude(&o, 0.0, above, 0.0);
    let mut mesh = crate::crystal::CrystalMesh::default();
    crate::floor_tiles::polygon_faces(&mut mesh, &iv, &it, above, true, 0);
    crate::floor_tiles::polygon_faces(&mut mesh, &iv, &it, 0.0, false, 0);
    let mut trig = crate::crystal::CrystalMesh::default();
    crate::floor_tiles::tri_faces(&mut trig, &tv, &tt, 0);
    let bytes = crate::floor_tiles::build_pure_item(ident, 26, &mesh, Some(&trig))?;
    // verification raster (0.5 m): inside the polygon ⇒ grass under it and no structure; grass outside the polygon = missed
    let origin = [-margin, -margin];
    let gsize = [gx + 2.0 * margin, gz + 2.0 * margin];
    let grass_grid = Grid::new(fill.clone(), origin, gsize, 2.0);
    let struct_grid = Grid::new(structure.clone(), origin, gsize, 2.0);
    let top: Vec<Tri> = it.iter().map(|t| [iv[t[0] as usize], iv[t[1] as usize], iv[t[2] as usize]]).collect();
    let poly_grid = Grid::new(top, origin, gsize, 4.0);
    let (mut n_in, mut n_nograss, mut n_struct, mut n_missed, mut n_grass) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let (nx, nz) = ((gsize[0] / 0.5) as i32, (gsize[1] / 0.5) as i32);
    for j in 0..nz {
        for i in 0..nx {
            let p = [origin[0] + 0.5 * (i as f32 + 0.5), origin[1] + 0.5 * (j as f32 + 0.5)];
            if p[0] >= 0.0 && p[0] <= gx && p[1] >= 0.0 && p[1] <= gz { continue; }
            let g = grass_grid.hit(p);
            let s = struct_grid.hit(p);
            let inp = poly_grid.hit(p);
            if g && !s { n_grass += 1; if !inp { n_missed += 1; } }
            if inp {
                n_in += 1;
                if !g { n_nograss += 1; }
                if s { n_struct += 1; }
            }
        }
    }
    notes.push(format!("raster 0.5 m outside the grid: {n_in} samples in the polygon — {n_nograss} over no grass, {n_struct} under the stands' structure; {n_grass} clear grass samples, {n_missed} outside the polygon ({:.1} %, the {inset} m inset and the slivers)", 100.0 * n_missed as f32 / n_grass.max(1) as f32));
    // the 4 m strips (OuterTerrain's Stadium rows): area and how much of them the polygon covers
    if !strips.is_empty() {
        let strip_area: f32 = strips.iter().map(|(x0, x1, z0, z1)| (x1 - x0) * (z1 - z0)).sum();
        let (mut ns, mut ns_in) = (0usize, 0usize);
        for (x0, x1, z0, z1) in strips {
            let mut z = z0 + 0.5;
            while z < *z1 {
                let mut x = x0 + 0.5;
                while x < *x1 {
                    ns += 1;
                    if poly_grid.hit([x, z]) { ns_in += 1; }
                    x += 1.0;
                }
                z += 1.0;
            }
        }
        notes.push(format!("vs the {} 4 m strips ({strip_area:.0} m²): polygon {:.0} m² = {:.1} %; {:.1} % of the strips' 1 m samples lie inside the polygon", strips.len(), o.area, 100.0 * o.area / strip_area, 100.0 * ns_in as f32 / ns.max(1) as f32));
    }
    Ok(GrassItem { bytes, outline: o, notes, check: (n_in, n_nograss, n_struct, n_missed, n_grass), margin })
}
