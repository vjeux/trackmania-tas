//! `mapgeom beam-ring` / `mapgeom beam-arch` — Everios96's missing Structure beams
//! (2026-10-27): the StructureSupportCurve family stops at radius 32 m (Curve1: four
//! quarters make a 2×2-cell ring), and the 1×1 hole block (`PlatformTechBaseWithHole24m`,
//! 24 m hole) has no ring that fits around its hole; nor is there a beam that follows
//! the TrackWallArch1x1 opening. Both are the SAME beam BENT onto a new path: every
//! vertex of the pack prefab is read as (arc length along the beam, radial offset from
//! the beam's centreline, height) and re-emitted on the target curve, so the truss keeps
//! its cross-section (1 m × 2 m) and only its lattice pitch changes with the length ratio.
//!
//! ```text
//! mapgeom beam-ring --out F [--radius 13] [--prefab Stadium\Media\Prefab\Structure\SupportCurve1In_Airv2.Prefab.Gbx]
//!                   [--ident NAME.Item.Gbx] [--author LOGIN] [--keep-ends]
//! mapgeom beam-arch --out F --contour PTS.json [--face-x 1.5] [--offset 0.75] [--ident …] [--author …]
//! ```
//!
//! The source beam (SupportCurve1In_Airv2): a quarter circle about the cell corner
//! (0, 32): a 2.5 m × 2 m shell (TechnicsTrimsColorize, radius 29.5..32, y 0..2, the
//! TRACKMANIA decal on its outer face) around an inner TechnicsTrims lattice, angle 0..90°,
//! plus the junction cube (Structure/Pylon, r ≈ 22.6) and two braces (r 20..25) toward the
//! cell centre. Everios96 wants "just the simple beam, without the extra geometry": every
//! triangle with a vertex under r 29 goes. The shell takes the ITEM COLOUR (Blue = the
//! vanilla block's look in his maps; Default renders white).
//!
//! Ring: 4 quarters bent to `--radius` (default 13 m: inner edge 12.5 m, just outside the
//! 12 m hole) around the cell centre (16, 0, 16); pivot at the cell corner, 32 m grid, so
//! the item drops onto the hole block like a block. Arch: the contour (ordered (z, Y)
//! points on the block face, floor to floor over the top) offset outward by `--offset`,
//! the beam's 2 m height along the face normal (−x, out of the face), two quarters' worth
//! of beam length mapped onto the contour length.

use crate::geom::IDENTITY;
use crate::static_item::build::{add_prefab, assemble, BuildOpts};
use crate::static_item::merged::Merged;
use crate::static_item::surface::Triangle;
use crate::static_item::vstream::{Elem, N_NORMAL, N_POSITION, N_TANGENT_U, N_TANGENT_V, T_DEC3N, T_FLOAT3};
use crate::static_item::Node;

type R<T> = Result<T, String>;

const SRC_PREFAB: &str = "Stadium\\Media\\Prefab\\Structure\\SupportCurve1In_Airv2.Prefab.Gbx";
/// The source quarter's centre (x, z) in the prefab frame and its centreline radius.
const SRC_CENTRE: [f32; 2] = [0.0, 32.0];
const SRC_R: f32 = 30.0;

fn flag<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    tmmaps::cli::flag(args, k)
}

/// Source-frame coordinates of a prefab vertex: (angle along the quarter, radians
/// 0..π/2; radial offset from the centreline; height).
fn polar(p: [f32; 3]) -> (f32, f32, f32) {
    let dx = p[0] - SRC_CENTRE[0];
    let dz = p[2] - SRC_CENTRE[1];
    let r = (dx * dx + dz * dz).sqrt();
    // angle measured from the +x axis toward −z (the quarter runs from (0,1.5)… to (30.5,32)):
    // at the start x≈0, z≈1.5 → dz = −30.5 → θ = atan2(−dz, dx) = π/2; at the end x≈30.5, z≈32 → θ = 0
    let theta = (-dz).atan2(dx);
    (theta, r - SRC_R, p[1])
}

/// Drop every triangle with a vertex failing `keep` (visuals and the collision mesh),
/// compacting the vertex streams; visuals left empty go.
fn retain_triangles(m: &mut Merged, keep: &dyn Fn([f32; 3]) -> bool) {
    let mut dropped_meta = 0usize;
    let mut tangent_note: Vec<String> = Vec::new();
    for mv in &mut m.visuals {
        let v = &mut mv.visual;
        let Some(main) = v.main.as_mut() else { continue };
        let Some(Node::VertexStream(stream)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let positions: Vec<[f32; 3]> = stream
            .decls
            .iter()
            .zip(stream.elems.iter())
            .find_map(|(d, e)| match (d.name(), d.stored_type(compress), e) {
                (N_POSITION, T_FLOAT3, Elem::Float3(p)) => Some(p.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let Some(ib) = v.index_buffer.as_mut() else { continue };
        if positions.is_empty() || ib.indices.len() % 3 != 0 {
            continue;
        }
        let mut kept: Vec<u32> = Vec::with_capacity(ib.indices.len());
        for t in ib.indices.chunks(3) {
            if t.iter().all(|i| keep(positions[*i as usize])) {
                kept.extend_from_slice(t);
            }
        }
        // compact: remap used vertices to 0..n
        let mut map: Vec<i64> = vec![-1; positions.len()];
        let mut next = 0u32;
        for i in &kept {
            if map[*i as usize] < 0 {
                map[*i as usize] = next as i64;
                next += 1;
            }
        }
        let order: Vec<usize> = {
            let mut o = vec![0usize; next as usize];
            for (old, new) in map.iter().enumerate() {
                if *new >= 0 {
                    o[*new as usize] = old;
                }
            }
            o
        };
        ib.indices = kept.iter().map(|i| map[*i as usize] as u32).collect();
        for e in stream.elems.iter_mut() {
            match e {
                Elem::Float2(v) => *v = order.iter().map(|&i| v[i]).collect(),
                Elem::Float3(v) => *v = order.iter().map(|&i| v[i]).collect(),
                Elem::Float4(v) => *v = order.iter().map(|&i| v[i]).collect(),
                Elem::Word(v) => *v = order.iter().map(|&i| v[i]).collect(),
                Elem::Raw { size, bytes } => {
                    let s = *size;
                    *bytes = order.iter().flat_map(|&i| bytes[i * s..(i + 1) * s].to_vec()).collect();
                }
            }
        }
        stream.count = next as i32;
        main.count = next as i32;
        // per-vertex texcoord sets kept in the visual itself (the pre-stream form): compacted too
        for set in main.tex_coord_sets.iter_mut() {
            if set.coords.len() == positions.len() {
                set.coords = order.iter().map(|&i| set.coords[i].clone()).collect();
            }
            if set.u01.len() == positions.len() {
                set.u01 = order.iter().map(|&i| set.u01[i]).collect();
            }
        }
        // index-range metadata of the source visual (sub-visuals, splits) would point past
        // the compacted buffers: dropped (the builder's own visuals carry none and the game
        // draws them). The separate per-vertex tangent arrays (chunk 0x0902C004: `per`
        // bytes per vertex, 4 or 12 by the visual flags) are compacted in the same order.
        if !v.sub_visuals.is_empty() || !v.splits.is_empty() {
            dropped_meta += 1;
        }
        v.sub_visuals.clear();
        v.splits.clear();
        if let Some((a, b)) = v.tangents.as_mut() {
            // the two arrays are per-vertex; their stride is whatever the source wrote
            // (bytes / vertex count), never guessed from the flags
            let n = positions.len().max(1);
            let mut pick = |t: &Vec<u8>| -> Vec<u8> {
                if t.is_empty() || t.len() % n != 0 {
                    tangent_note.push(format!("tangent array of {} bytes does not divide by {n} vertices: left as is", t.len()));
                    return t.clone();
                }
                let per = t.len() / n;
                order.iter().flat_map(|&i| t[i * per..(i + 1) * per].to_vec()).collect()
            };
            let (na, nb) = (pick(a), pick(b));
            tangent_note.push(format!("tangents {}+{} B / {n} verts compacted to {}+{} B", a.len(), b.len(), na.len(), nb.len()));
            *a = na;
            *b = nb;
        }
        if let Some(Elem::Float3(p)) = stream.decls.iter().zip(stream.elems.iter()).find(|(d, _)| d.name() == N_POSITION).map(|(_, e)| e) {
            main.bounding_box = crate::static_item::merged::bbox(p);
        }
    }
    // collision
    let verts = m.surf_vertices.clone();
    let mut kept_t: Vec<Triangle> = Vec::new();
    for t in &m.surf_triangles {
        if t.indices.iter().all(|i| keep(verts[*i as usize])) {
            kept_t.push(t.clone());
        }
    }
    let mut map: Vec<i64> = vec![-1; verts.len()];
    let mut nv: Vec<[f32; 3]> = Vec::new();
    for t in &mut kept_t {
        for i in &mut t.indices {
            if map[*i as usize] < 0 {
                map[*i as usize] = nv.len() as i64;
                nv.push(verts[*i as usize]);
            }
            *i = map[*i as usize] as u32;
        }
    }
    m.surf_vertices = nv;
    m.surf_triangles = kept_t;
    // an emptied visual (a material group that was all junction) would fail the checker
    m.visuals.retain(|mv| mv.visual.index_buffer.as_ref().map(|ib| !ib.indices.is_empty()).unwrap_or(false));
    if dropped_meta > 0 {
        m.notes.push(format!("beam: {dropped_meta} visual(s) had sub-visual/split metadata, dropped"));
    }
    for t in tangent_note.into_iter().take(3) {
        m.notes.push(format!("beam: {t}"));
    }
}

/// Every position through `f`, every normal/tangent through the rotation `f` returns
/// for that position (column-major 3×3 applied to the vector).
fn remap_frames(m: &mut Merged, f: &dyn Fn([f32; 3]) -> ([f32; 3], [[f32; 3]; 3])) {
    let rot = |r: &[[f32; 3]; 3], v: [f32; 3]| -> [f32; 3] {
        let o = [
            r[0][0] * v[0] + r[0][1] * v[1] + r[0][2] * v[2],
            r[1][0] * v[0] + r[1][1] * v[1] + r[1][2] * v[2],
            r[2][0] * v[0] + r[2][1] * v[1] + r[2][2] * v[2],
        ];
        let l = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt().max(1e-6);
        [o[0] / l, o[1] / l, o[2] / l]
    };
    for mv in &mut m.visuals {
        let Some(main) = mv.visual.main.as_mut() else { continue };
        let Some(Node::VertexStream(stream)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let pos_idx = stream.decls.iter().position(|d| d.name() == N_POSITION);
        let Some(pi) = pos_idx else { continue };
        let old: Vec<[f32; 3]> = match &stream.elems[pi] {
            Elem::Float3(p) => p.clone(),
            _ => continue,
        };
        let frames: Vec<([f32; 3], [[f32; 3]; 3])> = old.iter().map(|p| f(*p)).collect();
        for (d, e) in stream.decls.iter().zip(stream.elems.iter_mut()) {
            match (d.name(), d.stored_type(compress), e) {
                (N_POSITION, T_FLOAT3, Elem::Float3(p)) => {
                    for (q, fr) in p.iter_mut().zip(frames.iter()) {
                        *q = fr.0;
                    }
                }
                (N_NORMAL | N_TANGENT_U | N_TANGENT_V, T_DEC3N, Elem::Word(w)) => {
                    for (x, fr) in w.iter_mut().zip(frames.iter()) {
                        *x = crate::static_item::merged::dec3n_pack(rot(&fr.1, crate::static_item::merged::dec3n_unpack(*x)));
                    }
                }
                (N_NORMAL | N_TANGENT_U | N_TANGENT_V, T_FLOAT3, Elem::Float3(p)) => {
                    for (q, fr) in p.iter_mut().zip(frames.iter()) {
                        *q = rot(&fr.1, *q);
                    }
                }
                _ => {}
            }
        }
        if let Elem::Float3(p) = &stream.elems[pi] {
            main.bounding_box = crate::static_item::merged::bbox(p);
        }
    }
    for v in &mut m.surf_vertices {
        *v = f(*v).0;
    }
}

/// The source beam as a Merged: the prefab loaded once per entry of `copies` (each a
/// rotation about the source centre, radians, so the copies lie end to end on the source
/// circle), junction/braces (and end plates unless `keep_ends`) removed.
fn source_beam(store: &mut crate::store::DataStore, prefab: &str, keep_ends: bool, copies: &[f32]) -> R<Merged> {
    let mut m = Merged::default();
    for a in copies {
        add_prefab(store, prefab, &rot_about_src(*a), 1.0, &mut m, 0)?;
    }
    let before: usize = m.visuals.iter().filter_map(|v| v.visual.index_buffer.as_ref()).map(|ib| ib.indices.len() / 3).sum();
    // BEAM_KEEP_ALL=1: no filtering at all (the pipeline bisect of 2026-10-27)
    let keep_all = std::env::var("BEAM_KEEP_ALL").map(|v| v == "1").unwrap_or(false);
    // The beam (measured 2026-10-27 on the box, Q1v2 A/B): a 2.5 m × 2 m shell of
    // TechnicsTrimsColorize (radius 29.5..32 — its OUTER face carries the TRACKMANIA decal
    // at r 32) around an inner TechnicsTrims lattice (r ≈ 30..30.5); the braces (r 20..25)
    // and the junction cube (Structure + Pylon at r ≈ 22.6) are the "extra geometry".
    // Everything from r 29 outward stays — the first cut (d < 1.2) threw the shell away and
    // shipped the bare dark lattice ("texture looks a bit off").
    let _ = keep_ends;
    retain_triangles(&mut m, &|c| {
        if keep_all {
            return true;
        }
        let (_, d, _) = polar(c);
        d >= -1.0
    });
    let after: usize = m.visuals.iter().filter_map(|v| v.visual.index_buffer.as_ref()).map(|ib| ib.indices.len() / 3).sum();
    m.notes.push(format!("beam: {} copies, {after} of {before} triangles kept (junction cube + braces removed)", copies.len()));
    Ok(m)
}

/// Rotation by `a` (radians, θ → θ + a in `polar`'s convention) about the source centre.
fn rot_about_src(a: f32) -> crate::geom::Xform {
    let (s, c) = a.sin_cos();
    // x' = c·dx + s·dz ; z' = −s·dx + c·dz   (dx, dz relative to the centre)
    let (cx, cz) = (SRC_CENTRE[0], SRC_CENTRE[1]);
    let tx = cx - (c * cx + s * cz);
    let tz = cz - (-s * cx + c * cz);
    [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c, tx, 0.0, tz]
}

fn rot_y(theta: f32) -> [[f32; 3]; 3] {
    let (s, c) = theta.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

fn mat_mul(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut o = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    o
}

fn write_item(m: &mut Merged, args: &[String], out: &str, default_ident: &str, grid: (f32, f32)) -> R<()> {
    let ident = flag(args, "--ident").unwrap_or(default_ident).to_string();
    let author = flag(args, "--author").unwrap_or("Everios96").to_string();
    let opts = BuildOpts { ident: ident.clone(), author, scale: 1.0, collection: 26, skin: None };
    // BEAM_NO_GRID=1: the item default placement (the pipeline bisect)
    if std::env::var("BEAM_NO_GRID").map(|v| v == "1").unwrap_or(false) {
        crate::static_item::assemble::GRID_SNAP.with(|g| g.set(None));
    } else {
        crate::static_item::assemble::GRID_SNAP.with(|g| g.set(Some(grid)));
    }
    let mut f = assemble(m, &opts)?;
    crate::static_item::assemble::GRID_SNAP.with(|g| g.set(None));
    // ⛔ an embedded item's body must be LZO-compressed ("BUCR"): an uncompressed body
    // ("BUUR") crashes the client at map load — edge-border learnt it 2026-10-24, and the
    // first beam items relearnt it today (five crashing variants before the header byte
    // was compared with a working library item)
    f.body_comp = b'C';
    let bytes = crate::static_item::write_file(&f);
    std::fs::write(out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    let tris: usize = m.visuals.iter().filter_map(|v| v.visual.index_buffer.as_ref()).map(|ib| ib.indices.len() / 3).sum();
    println!("wrote {out}: {} bytes, {} visuals, {tris} triangles, collision {} tris, grid {}×{} m", bytes.len(), m.visuals.len(), m.surf_triangles.len(), grid.0, grid.1);
    for n in &m.notes {
        if n.starts_with("beam") || n.starts_with("ring") || n.starts_with("arch") {
            println!("  {n}");
        }
    }
    Ok(())
}

/// `beam-ring`: the quarters laid end to end on the source circle, then every vertex
/// moved from the source radius to the ring radius about the cell centre.
pub fn ring_cmd(store: &mut crate::store::DataStore, args: &[String]) -> R<()> {
    let out = flag(args, "--out").ok_or("--out F")?;
    let radius: f32 = flag(args, "--radius").and_then(|v| v.parse().ok()).unwrap_or(13.0);
    let prefab = flag(args, "--prefab").unwrap_or(SRC_PREFAB);
    let keep_ends = args.iter().any(|a| a == "--keep-ends");
    let quarters: usize = flag(args, "--quarters").and_then(|v| v.parse().ok()).unwrap_or(4);
    let cx = flag(args, "--centre-x").and_then(|v| v.parse().ok()).unwrap_or(16.0f32);
    let cz = flag(args, "--centre-z").and_then(|v| v.parse().ok()).unwrap_or(16.0f32);
    let copies: Vec<f32> = (0..quarters).map(|q| q as f32 * std::f32::consts::FRAC_PI_2).collect();
    let mut m = source_beam(store, prefab, keep_ends, &copies)?;
    let ident3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    // BEAM_NO_BEND=1: geometry left where the source put it (the pipeline bisect)
    if std::env::var("BEAM_NO_BEND").map(|v| v == "1").unwrap_or(false) {
        m.notes.push("ring: NO BEND (bisect)".into());
    } else {
        remap_frames(&mut m, &|p| {
            let (theta, d, y) = polar(p);
            let r = radius + d;
            ([cx + r * theta.cos(), y, cz - r * theta.sin()], ident3)
        });
    }
    m.notes.push(format!("ring: {quarters} quarters of {prefab} bent from radius {SRC_R} to {radius} m about ({cx}, {cz})"));
    write_item(&mut m, args, out, "StructureSupportRing1x1.Item.Gbx", (32.0, 8.0))
}

/// `beam-arch`: the beam along a contour on a block face.
pub fn arch_cmd(store: &mut crate::store::DataStore, args: &[String]) -> R<()> {
    let out = flag(args, "--out").ok_or("--out F")?;
    let contour_path = flag(args, "--contour").ok_or("--contour PTS.json (ordered [[z, Y], …])")?;
    let face_x: f32 = flag(args, "--face-x").and_then(|v| v.parse().ok()).unwrap_or(1.5);
    let offset: f32 = flag(args, "--offset").and_then(|v| v.parse().ok()).unwrap_or(0.5);
    let prefab = flag(args, "--prefab").unwrap_or(SRC_PREFAB);
    let text = std::fs::read_to_string(contour_path).map_err(|e| format!("{contour_path}: {e}"))?;
    // minimal JSON: [[z, y], [z, y], …]
    let nums: Vec<f32> = text.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+')).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
    if nums.len() < 6 || nums.len() % 2 != 0 {
        return Err(format!("{contour_path}: want an even list of numbers, got {}", nums.len()));
    }
    let mut raw: Vec<[f32; 2]> = nums.chunks(2).map(|c| [c[0], c[1]]).collect();
    // the contour is a closed-ish loop around the opening ordered by angle; the beam runs
    // from one foot over the top to the other, so the list is rotated to start right
    // after the widest gap between consecutive points (the open floor)
    if raw.len() > 2 {
        let mut gap_at = 0;
        let mut gap = -1.0f32;
        for i in 0..raw.len() {
            let a = raw[i];
            let b = raw[(i + 1) % raw.len()];
            let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            if l > gap {
                gap = l;
                gap_at = i;
            }
        }
        let n = raw.len();
        raw.rotate_left((gap_at + 1) % n);
    }
    // the opening centroid (for the outward direction) and the offset contour
    let n = raw.len() as f32;
    let centroid = raw.iter().fold([0.0f32; 2], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    let path: Vec<[f32; 2]> = raw
        .iter()
        .map(|p| {
            let d = [p[0] - centroid[0], p[1] - centroid[1]];
            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
            [p[0] + d[0] / l * offset, p[1] + d[1] / l * offset]
        })
        .collect();
    // arc lengths
    let mut cum = vec![0.0f32];
    for w in path.windows(2) {
        let l = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        cum.push(cum.last().unwrap() + l);
    }
    let total = *cum.last().unwrap();
    // point + unit tangent + outward normal at arc length s
    let at = |s: f32| -> ([f32; 2], [f32; 2], [f32; 2]) {
        let s = s.clamp(0.0, total);
        let i = cum.iter().position(|c| *c >= s).unwrap_or(cum.len() - 1).max(1);
        let (a, b) = (path[i - 1], path[i]);
        let seg = (cum[i] - cum[i - 1]).max(1e-6);
        let u = (s - cum[i - 1]) / seg;
        let p = [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u];
        let t = [(b[0] - a[0]) / seg, (b[1] - a[1]) / seg];
        // outward = away from the centroid, perpendicular to t
        let mut nrm = [-t[1], t[0]];
        let to_c = [centroid[0] - p[0], centroid[1] - p[1]];
        if nrm[0] * to_c[0] + nrm[1] * to_c[1] > 0.0 {
            nrm = [-nrm[0], -nrm[1]];
        }
        (p, t, nrm)
    };
    // as many quarters of beam (end plates removed), end to end on the source circle, as
    // keep the lattice pitch closest to the original: source arc length n·(π/2)·R; θ runs
    // π/2 → π/2 − n·π/2 along it
    let quarter_len = std::f32::consts::FRAC_PI_2 * SRC_R;
    let n_q = (1..=3).min_by(|a, b| (total / (*a as f32 * quarter_len)).ln().abs().partial_cmp(&(total / (*b as f32 * quarter_len)).ln().abs()).unwrap()).unwrap_or(2);
    let copies: Vec<f32> = (0..n_q).map(|q| -(q as f32) * std::f32::consts::FRAC_PI_2).collect();
    let mut m = source_beam(store, prefab, false, &copies)?;
    let src_len = n_q as f32 * quarter_len;
    // BEAM_NO_ROT=1: positions bent, normals/tangents left (the bisect); BEAM_NO_BEND=1: nothing moved
    let no_rot = std::env::var("BEAM_NO_ROT").map(|v| v == "1").unwrap_or(false);
    if std::env::var("BEAM_NO_BEND").map(|v| v == "1").unwrap_or(false) {
        m.notes.push("arch: NO BEND (bisect)".into());
        let mut all = m;
        all.notes.push(format!("arch: {} contour points, length {total:.1} m, {n_q} quarter(s) unbent", raw.len()));
        return write_item(&mut all, args, out, "StructureSupportArch1x1.Item.Gbx", (32.0, 8.0));
    }
    remap_frames(&mut m, &|p| {
        let (theta, d, y) = polar(p);
        let s_src = (std::f32::consts::FRAC_PI_2 - theta) * SRC_R;
        let s = s_src / src_len * total;
        let (pt, t, nrm) = at(s);
        // the beam's height (y, 0..2) goes along −x, out of the face; radial offset along the outward normal
        let pos = [face_x - y, pt[1] + d * nrm[1], pt[0] + d * nrm[0]];
        // source frame at θ: tangent (−sinθ, 0, −cosθ) for increasing s, radial (cosθ, 0, −sinθ), up (0,1,0);
        // target: tangent (0, t[1], t[0]), radial (0, nrm[1], nrm[0]), up (−1, 0, 0). R = T_target · T_source^T
        let (sn, cs) = theta.sin_cos();
        let a = [[0.0, 0.0, -1.0], [t[1], nrm[1], 0.0], [t[0], nrm[0], 0.0]];
        let b = [[-sn, 0.0, -cs], [cs, 0.0, -sn], [0.0, 1.0, 0.0]];
        if no_rot {
            (pos, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
        } else {
            (pos, mat_mul(&a, &b))
        }
    });
    let mut all = m;
    all.notes.push(format!("arch: {} contour points, length {total:.1} m (+{offset} m outward), face x {face_x}, {n_q} quarter(s) of {prefab} ({src_len:.1} m) stretched ×{:.2}", raw.len(), total / src_len));
    write_item(&mut all, args, out, "StructureSupportArch1x1.Item.Gbx", (32.0, 8.0))
}
