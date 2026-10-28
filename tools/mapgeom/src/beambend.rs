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

use crate::static_item::build::{add_prefab, assemble, BuildOpts};
use crate::crystal_model::CPlugMaterialUserInst;
use crate::static_item::merged::Merged;
use crate::static_item::surface::Triangle;
use crate::static_item::vstream::{Elem, N_COLOR0, N_NORMAL, N_POSITION, N_TANGENT_U, N_TANGENT_V, N_TEXCOORD0, T_COLOR, T_DEC3N, T_FLOAT3};
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
    retain_triangles_by(m, &|t: &[[f32; 3]; 3]| t.iter().all(|c| keep(*c)))
}

/// Keep the triangles `keep` accepts (judged whole), compacting every stream.
fn retain_triangles_by(m: &mut Merged, keep: &dyn Fn(&[[f32; 3]; 3]) -> bool) {
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
            let tri = [positions[t[0] as usize], positions[t[1] as usize], positions[t[2] as usize]];
            if keep(&tri) {
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
        let tri = [verts[t.indices[0] as usize], verts[t.indices[1] as usize], verts[t.indices[2] as usize]];
        if keep(&tri) {
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

/// Split every triangle whose vertices span more than `max_span` radians of source angle
/// (longest-edge bisection, midpoints shared between the two triangles of an edge, every
/// vertex attribute interpolated: floats lerped, DEC3N normals/tangents lerped and
/// renormalised, colours lerped per byte, anything else copied from the edge's first end).
/// The vanilla beam's inner wall is 8 flat panels per quarter (11.25° each): bent onto a
/// 2-half ring those panels span 22.5° and poke through the curved lips — Everios96 17:05
/// "the inside of the ring is not flat like the original block". Chords this short hug the
/// curve. The collision mesh gets the same cut.
fn subdivide_by_angle(m: &mut Merged, max_span: f32) -> usize {
    use std::collections::HashMap;
    let span_of = |a: [f32; 3], b: [f32; 3]| -> f32 {
        let (ta, _, _) = polar(a);
        let (tb, _, _) = polar(b);
        let d = (ta - tb).abs();
        d.min(std::f32::consts::TAU - d)
    };
    let mut added = 0usize;
    let mut dbg: Vec<String> = Vec::new();
    for mv in &mut m.visuals {
        // the nearest detail level only: the far levels draw from 64 m out, where a chord
        // this size is under a pixel
        if mv.lod_mask != 0 && mv.lod_mask & 1 == 0 {
            continue;
        }
        dbg.push(format!("mask{}", mv.lod_mask));
        let v = &mut mv.visual;
        let Some(main) = v.main.as_mut() else { continue };
        let Some(Node::VertexStream(stream)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let pos_slot = stream.decls.iter().zip(stream.elems.iter()).position(|(d, e)| d.name() == N_POSITION && matches!(e, Elem::Float3(_)));
        let Some(pos_slot) = pos_slot else { continue };
        let Some(ib) = v.index_buffer.as_mut() else { continue };
        if ib.indices.len() % 3 != 0 {
            continue;
        }
        let n0 = match &stream.elems[pos_slot] {
            Elem::Float3(p) => p.len(),
            _ => continue,
        };
        // the kinds per slot, decided once
        let kinds: Vec<u8> = stream
            .decls
            .iter()
            .map(|d| {
                let t = d.stored_type(compress);
                if t == T_DEC3N {
                    1
                } else if t == T_COLOR || d.name() == N_COLOR0 {
                    2
                } else {
                    0
                }
            })
            .collect();
        let per_tangent: Vec<usize> = match v.tangents.as_ref() {
            Some((a, b)) => vec![if n0 > 0 && a.len() % n0 == 0 { a.len() / n0 } else { 0 }, if n0 > 0 && b.len() % n0 == 0 { b.len() / n0 } else { 0 }],
            None => Vec::new(),
        };
        let mut mids: HashMap<(u32, u32), u32> = HashMap::new();
        let mut work: Vec<[u32; 3]> = ib.indices.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
        let mut out: Vec<[u32; 3]> = Vec::with_capacity(work.len() * 2);
        let mut guard = 0usize;
        while let Some(t) = work.pop() {
            guard += 1;
            if guard > 4_000_000 {
                break;
            }
            let p = |i: u32, elems: &Vec<Elem>| -> [f32; 3] {
                match &elems[pos_slot] {
                    Elem::Float3(p) => p[i as usize],
                    _ => [0.0; 3],
                }
            };
            let (pa, pb, pc) = (p(t[0], &stream.elems), p(t[1], &stream.elems), p(t[2], &stream.elems));
            let spans = [span_of(pa, pb), span_of(pb, pc), span_of(pc, pa)];
            let (k, &worst) = spans.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).unwrap();
            if worst <= max_span {
                out.push(t);
                continue;
            }
            let (i, j) = (t[k], t[(k + 1) % 3]);
            let key = (i.min(j), i.max(j));
            let mid = if let Some(&mv) = mids.get(&key) {
                mv
            } else {
                // a new vertex: every element interpolated
                for (slot, e) in stream.elems.iter_mut().enumerate() {
                    let kind = kinds[slot];
                    match e {
                        Elem::Float2(x) => {
                            let (a, b) = (x[i as usize], x[j as usize]);
                            x.push([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
                        }
                        Elem::Float3(x) => {
                            let (a, b) = (x[i as usize], x[j as usize]);
                            let mut mval = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
                            if stream.decls[slot].name() == N_NORMAL || stream.decls[slot].name() == N_TANGENT_U || stream.decls[slot].name() == N_TANGENT_V {
                                let l = (mval[0] * mval[0] + mval[1] * mval[1] + mval[2] * mval[2]).sqrt();
                                if l > 1e-6 {
                                    mval = [mval[0] / l, mval[1] / l, mval[2] / l];
                                }
                            }
                            x.push(mval);
                        }
                        Elem::Float4(x) => {
                            let (a, b) = (x[i as usize], x[j as usize]);
                            x.push([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0, (a[3] + b[3]) / 2.0]);
                        }
                        Elem::Word(x) => {
                            let (a, b) = (x[i as usize], x[j as usize]);
                            let val = match kind {
                                1 => {
                                    let (na, nb) = (crate::static_item::merged::dec3n_unpack(a), crate::static_item::merged::dec3n_unpack(b));
                                    let mut mval = [(na[0] + nb[0]) / 2.0, (na[1] + nb[1]) / 2.0, (na[2] + nb[2]) / 2.0];
                                    let l = (mval[0] * mval[0] + mval[1] * mval[1] + mval[2] * mval[2]).sqrt();
                                    if l > 1e-6 {
                                        mval = [mval[0] / l, mval[1] / l, mval[2] / l];
                                    }
                                    crate::static_item::merged::dec3n_pack(mval)
                                }
                                2 => {
                                    let mut o = 0u32;
                                    for sh in [0, 8, 16, 24] {
                                        let ca = (a >> sh) & 0xFF;
                                        let cb = (b >> sh) & 0xFF;
                                        o |= ((ca + cb) / 2) << sh;
                                    }
                                    o
                                }
                                _ => a,
                            };
                            x.push(val);
                        }
                        Elem::Raw { size, bytes } => {
                            let sz = *size;
                            let rec = bytes[i as usize * sz..(i as usize + 1) * sz].to_vec();
                            bytes.extend_from_slice(&rec);
                        }
                    }
                }
                if let Some((a, b)) = v.tangents.as_mut() {
                    for (arr, per) in [(a, per_tangent[0]), (b, per_tangent[1])] {
                        if per > 0 {
                            let rec = arr[i as usize * per..(i as usize + 1) * per].to_vec();
                            arr.extend_from_slice(&rec);
                        }
                    }
                }
                let idx = match &stream.elems[pos_slot] {
                    Elem::Float3(p) => (p.len() - 1) as u32,
                    _ => unreachable!(),
                };
                mids.insert(key, idx);
                added += 1;
                idx
            };
            // the two halves, winding kept
            let third = t[(k + 2) % 3];
            work.push([i, mid, third]);
            work.push([mid, j, third]);
        }
        ib.indices = out.iter().flat_map(|t| t.iter().copied()).collect();
        let n = match &stream.elems[pos_slot] {
            Elem::Float3(p) => p.len(),
            _ => n0,
        };
        stream.count = n as i32;
        main.count = n as i32;
        if let Some(Elem::Float3(p)) = stream.elems.get(pos_slot) {
            main.bounding_box = crate::static_item::merged::bbox(p);
        }
    }
    if std::env::var("BEAM_DEBUG").is_ok() {
        eprintln!("subdivide: visuals {} / {} judged: {}", dbg.len(), m.visuals.len(), dbg.join(","));
    }
    // collision: positions only
    {
        let mut verts = m.surf_vertices.clone();
        let mut mids: HashMap<(u32, u32), u32> = HashMap::new();
        let mut work: Vec<Triangle> = m.surf_triangles.clone();
        let mut out: Vec<Triangle> = Vec::with_capacity(work.len() * 2);
        let mut guard = 0usize;
        while let Some(t) = work.pop() {
            guard += 1;
            if guard > 4_000_000 {
                break;
            }
            let (pa, pb, pc) = (verts[t.indices[0] as usize], verts[t.indices[1] as usize], verts[t.indices[2] as usize]);
            let spans = [span_of(pa, pb), span_of(pb, pc), span_of(pc, pa)];
            let (k, &worst) = spans.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).unwrap();
            if worst <= max_span {
                out.push(t);
                continue;
            }
            let (i, j) = (t.indices[k], t.indices[(k + 1) % 3]);
            let key = (i.min(j), i.max(j));
            let mid = *mids.entry(key).or_insert_with(|| {
                let (a, b) = (verts[i as usize], verts[j as usize]);
                verts.push([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0]);
                (verts.len() - 1) as u32
            });
            let third = t.indices[(k + 2) % 3];
            let mut t1 = t.clone();
            t1.indices = [i, mid, third];
            let mut t2 = t.clone();
            t2.indices = [mid, j, third];
            work.push(t1);
            work.push(t2);
        }
        m.surf_vertices = verts;
        m.surf_triangles = out;
    }
    added
}

/// The game draws the vanilla beam as a closed box: its concave face is FLAT and carries
/// the TRACKMANIA decal like the convex one (StructureSupportCurve1In, Blue, concave side at
/// 30 m: box shot h7 2026-10-27 17:45), while the prefab's own inner side is a chamfered
/// channel (lips at r 29.65, a rail at 30.4, a back wall at 30.5, no decal) — whatever the
/// engine does with the block, a one-to-one item shows the channel (Everios96 17:05 "the
/// inside of the ring is not flat like the original block"). So the OUTER SKIN and its
/// decal (every triangle whose vertices all sit at d ≥ `from_d`) are copied to the inner
/// side: positions reflected across the cylinder midway between d = 2 and `to_d` (the
/// skin lands at `to_d`, the decal 3 cm proud of it toward the centre), winding reversed
/// and radial normal components flipped so the copy faces the centre, u mirrored so the
/// decal reads the right way. The channel stays behind the new skin, 1 cm back, unseen.
fn mirror_outer_skin(m: &mut Merged, from_d: f32, to_d: f32) -> usize {
    let d_mid = (2.0 + to_d) / 2.0;
    let mut added = 0usize;
    for mv in &mut m.visuals {
        let v = &mut mv.visual;
        let Some(main) = v.main.as_mut() else { continue };
        let Some(Node::VertexStream(stream)) = main.vertex_streams.first_mut().and_then(|r| r.inline.as_deref_mut()) else { continue };
        let compress = stream.compress_local3d.unwrap_or(false);
        let Some(pos_slot) = stream.decls.iter().zip(stream.elems.iter()).position(|(d, e)| d.name() == N_POSITION && matches!(e, Elem::Float3(_))) else { continue };
        let positions: Vec<[f32; 3]> = match &stream.elems[pos_slot] {
            Elem::Float3(p) => p.clone(),
            _ => continue,
        };
        let Some(ib) = v.index_buffer.as_mut() else { continue };
        if ib.indices.len() % 3 != 0 {
            continue;
        }
        let picked: Vec<[u32; 3]> = ib
            .indices
            .chunks(3)
            .filter(|t| t.iter().all(|&i| polar(positions[i as usize]).1 >= from_d))
            .map(|t| [t[0], t[1], t[2]])
            .collect();
        if picked.is_empty() {
            continue;
        }
        // the u range of the picked vertices (per visual): the mirror line for u
        let mut used: Vec<u32> = picked.iter().flat_map(|t| t.iter().copied()).collect();
        used.sort_unstable();
        used.dedup();
        let uv_slot = stream.decls.iter().position(|d| d.name() == N_TEXCOORD0);
        let (umin, umax) = match uv_slot.map(|k| &stream.elems[k]) {
            Some(Elem::Float2(uv)) => used.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &i| (lo.min(uv[i as usize][0]), hi.max(uv[i as usize][0]))),
            _ => (0.0, 0.0),
        };
        let n0 = positions.len();
        let mut map: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        let per_tangent: Vec<usize> = match v.tangents.as_ref() {
            Some((a, b)) => vec![if a.len() % n0.max(1) == 0 { a.len() / n0.max(1) } else { 0 }, if b.len() % n0.max(1) == 0 { b.len() / n0.max(1) } else { 0 }],
            None => Vec::new(),
        };
        for &i in &used {
            let p = positions[i as usize];
            let (theta, d, y) = polar(p);
            let d2 = 2.0 * d_mid - d;
            let r2 = SRC_R + d2;
            let radial = [theta.cos(), 0.0, -theta.sin()];
            let reflect = |n: [f32; 3]| -> [f32; 3] {
                let k = 2.0 * (n[0] * radial[0] + n[2] * radial[2]);
                [n[0] - k * radial[0], n[1], n[2] - k * radial[2]]
            };
            for (slot, e) in stream.elems.iter_mut().enumerate() {
                let name = stream.decls[slot].name();
                let ty = stream.decls[slot].stored_type(compress);
                match e {
                    Elem::Float3(x) => {
                        let val = if name == N_POSITION {
                            [SRC_CENTRE[0] + r2 * theta.cos(), y, SRC_CENTRE[1] - r2 * theta.sin()]
                        } else if name == N_NORMAL || name == N_TANGENT_U || name == N_TANGENT_V {
                            reflect(x[i as usize])
                        } else {
                            x[i as usize]
                        };
                        x.push(val);
                    }
                    Elem::Float2(x) => {
                        let mut val = x[i as usize];
                        if name == N_TEXCOORD0 {
                            val[0] = umin + umax - val[0];
                        }
                        x.push(val);
                    }
                    Elem::Float4(x) => {
                        let val = x[i as usize];
                        x.push(val);
                    }
                    Elem::Word(x) => {
                        let mut val = x[i as usize];
                        if ty == T_DEC3N && (name == N_NORMAL || name == N_TANGENT_U || name == N_TANGENT_V) {
                            val = crate::static_item::merged::dec3n_pack(reflect(crate::static_item::merged::dec3n_unpack(val)));
                        }
                        x.push(val);
                    }
                    Elem::Raw { size, bytes } => {
                        let sz = *size;
                        let rec = bytes[i as usize * sz..(i as usize + 1) * sz].to_vec();
                        bytes.extend_from_slice(&rec);
                    }
                }
            }
            if let Some((a, b)) = v.tangents.as_mut() {
                for (arr, per) in [(a, per_tangent[0]), (b, per_tangent[1])] {
                    if per > 0 {
                        let rec = arr[i as usize * per..(i as usize + 1) * per].to_vec();
                        arr.extend_from_slice(&rec);
                    }
                }
            }
            let idx = match &stream.elems[pos_slot] {
                Elem::Float3(p) => (p.len() - 1) as u32,
                _ => unreachable!(),
            };
            map.insert(i, idx);
        }
        for t in &picked {
            // reversed winding: the reflection turned the face inside out
            ib.indices.extend_from_slice(&[map[&t[0]], map[&t[2]], map[&t[1]]]);
        }
        added += picked.len();
        let n = match &stream.elems[pos_slot] {
            Elem::Float3(p) => p.len(),
            _ => n0,
        };
        stream.count = n as i32;
        main.count = n as i32;
        if let Some(Elem::Float3(p)) = stream.elems.get(pos_slot) {
            main.bounding_box = crate::static_item::merged::bbox(p);
        }
    }
    added
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
/// What happens to the quarter's end caps.
enum Ends {
    /// every cap stays
    Keep,
    /// every cap goes (a closed ring)
    Drop,
    /// only the caps at these source angles stay (an open path keeps its two feet closed)
    KeepAt(Vec<f32>),
}

fn source_beam(store: &mut crate::store::DataStore, prefab: &str, ends: Ends, copies: &[f32]) -> R<Merged> {
    let mut m = Merged::default();
    // Two-sided (env `BEAM_TWO_SIDED`, default on; judged at merge time, so set before the
    // prefabs are added): the beam's top and inner skin are wound inward — the game's own
    // material draws them from both sides, a one-sided copy loses them (the "open trough"
    // of every build before this one, box A/B 2026-10-27 16:28).
    // Scope (`BEAM_TWO_SIDED=shell|all|0`, default shell): only the Colorize SHELL and its
    // decal are wound inward; the TechnicsTrims lattice, lips and channel wall render fine
    // one-sided, and their back copies showed inside the channel as odd geometry (Everios96
    // 17:05 "some geometry on the inside is looking odd").
    let scope = std::env::var("BEAM_TWO_SIDED").unwrap_or_else(|_| "shell".to_string());
    if scope != "0" {
        let links: &[&str] = if scope == "all" { &["TechnicsTrims", "TechnicsTrimsColorize", "DecalPaintLogo8x1Colorize"] } else { &["TechnicsTrimsColorize", "DecalPaintLogo8x1Colorize"] };
        for l in links {
            m.two_sided_links.push(l.to_string());
        }
        // the PROPER form (duplicated vertices, reversed normals + winding): the shared-vertex
        // shortcut leaves the back copy with a normal facing away (merged.rs); the beam is a
        // closed box, so the back copies sit inside it and never fight the front ones
        if std::env::var("TINY_TWO_SIDED_PROPER").is_err() {
            std::env::set_var("TINY_TWO_SIDED_PROPER", "all");
        }
    }
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
    retain_triangles(&mut m, &|c| {
        if keep_all {
            return true;
        }
        let (_, d, _) = polar(c);
        d >= -1.0
    });
    // The quarter's END CAPS (the plates and lattice ends in the planes θ = 0° and 90°, 9 cm
    // deep): a ring has no ends, and two caps meeting at a joint showed as a dark seam with
    // a step (Everios96 16:43 side-by-side review). Dropped unless `--keep-ends` (the arch
    // keeps its feet closed).
    if !matches!(ends, Ends::Keep) && !keep_all {
        let kept_angles: Vec<f32> = match &ends {
            Ends::KeepAt(a) => a.clone(),
            _ => Vec::new(),
        };
        // The lattice and the inner wall stop 9 cm short of each end plane (the cap plate's
        // thickness): with the caps gone that left an 18 cm slot at every joint (seam view,
        // 16:58). Every vertex within 0.35° of an end plane is snapped onto it first, so the
        // walls meet at the joint and the plates collapse to nothing.
        let ident = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        remap_frames(&mut m, &|p| {
            let (theta, d, y) = polar(p);
            let q = theta.rem_euclid(std::f32::consts::FRAC_PI_2);
            if q < 0.0061 || q > std::f32::consts::FRAC_PI_2 - 0.0061 {
                let t = (theta / std::f32::consts::FRAC_PI_2).round() * std::f32::consts::FRAC_PI_2;
                let r = SRC_R + d;
                ([SRC_CENTRE[0] + r * t.cos(), y, SRC_CENTRE[1] - r * t.sin()], ident)
            } else {
                (p, ident)
            }
        });
        let at_end = |c: [f32; 3]| -> bool {
            let (theta, _, _) = polar(c);
            let q = theta.rem_euclid(std::f32::consts::FRAC_PI_2);
            let near = q < 0.0061 || q > std::f32::consts::FRAC_PI_2 - 0.0061; // 0.35° ≈ 18 cm at r 30
            if !near {
                return false;
            }
            // a cap the caller keeps: within 0.35° of one of its angles (mod 2π)
            !kept_angles.iter().any(|a| {
                let dd = (theta - a).rem_euclid(std::f32::consts::TAU);
                dd < 0.0061 || dd > std::f32::consts::TAU - 0.0061
            })
        };
        let n0: usize = m.visuals.iter().filter_map(|v| v.visual.index_buffer.as_ref()).map(|ib| ib.indices.len() / 3).sum();
        retain_triangles_by(&mut m, &|t: &[[f32; 3]; 3]| !t.iter().all(|c| at_end(*c)));
        let n1: usize = m.visuals.iter().filter_map(|v| v.visual.index_buffer.as_ref()).map(|ib| ib.indices.len() / 3).sum();
        m.notes.push(format!("beam: {} end-cap triangle(s) removed ({} cap angle(s) kept)", n0 - n1, kept_angles.len()));
    }
    // the flat inner face (env `BEAM_INNER_SKIN=0` to skip, the bisect)
    if std::env::var("BEAM_INNER_SKIN").map(|v| v != "0").unwrap_or(true) {
        let n = mirror_outer_skin(&mut m, 1.9, -0.36);
        m.notes.push(format!("beam: {n} outer-skin/decal triangle(s) mirrored onto the inner face at d −0.36 (the vanilla block's flat concave side)"));
    }
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
    // A BLOCK's colour tints every `*Trims` material; an ITEM's colour tints only the
    // `*Colorize` ones. The beam is half TechnicsTrims (top strip, inner wall, lattice) and
    // half TechnicsTrimsColorize (outer skin): as an item the Trims half stayed grey — Everios96
    // 16:06 "I can still see invisible faces / it looks like the inner material" (the vanilla
    // block beside it: solid blue). So every TechnicsTrims link becomes TechnicsTrimsColorize
    // (same texture, colour-masked), and the item colours like the block.
    let mut relinked = 0;
    for mat in m.materials.iter_mut() {
        if let Some(main) = mat.main.as_mut() {
            if main.link.as_str().map(|l| l.ends_with("\\TechnicsTrims")).unwrap_or(false) {
                main.link = crate::crystal_model::Id::Str("Stadium\\Media\\Material\\TechnicsTrimsColorize".to_string());
                relinked += 1;
            }
        }
    }
    if relinked > 0 {
        m.notes.push(format!("beam: {relinked} TechnicsTrims material(s) → TechnicsTrimsColorize (the item colour tints the whole beam, like the block colour does)"));
        // the relink leaves two slots with one link: fold them (item-check: "duplicate slot")
        let mut remap: Vec<usize> = (0..m.materials.len()).collect();
        let mut keep: Vec<CPlugMaterialUserInst> = Vec::new();
        for (i, mat) in m.materials.iter().enumerate() {
            let key = (mat.link().map(|l| l.to_string()), mat.physics());
            if let Some(j) = keep.iter().position(|k| (k.link().map(|l| l.to_string()), k.physics()) == key) {
                remap[i] = j;
            } else {
                remap[i] = keep.len();
                keep.push(mat.clone());
            }
        }
        for v in m.visuals.iter_mut() {
            v.material = remap[v.material];
        }
        m.materials = keep;
    }
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
    let quarters: usize = flag(args, "--quarters").and_then(|v| v.parse().ok()).unwrap_or(2);
    let cx = flag(args, "--centre-x").and_then(|v| v.parse().ok()).unwrap_or(16.0f32);
    let cz = flag(args, "--centre-z").and_then(|v| v.parse().ok()).unwrap_or(16.0f32);
    let copies: Vec<f32> = (0..quarters).map(|q| q as f32 * std::f32::consts::FRAC_PI_2).collect();
    let mut m = source_beam(store, prefab, if keep_ends { Ends::Keep } else { Ends::Drop }, &copies)?;
    let ident3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    // `quarters` copies share the 360°: with 2, each 90° quarter opens to 180° (the arc
    // length 47.1 → 40.8 m at r 13: texture and TRACKMANIA decal nearly natural, 2 joints
    // instead of 4); with 4 each quarter is squeezed to 43 % (the first builds). Frames turn
    // with the angle (rotation about y by θ' − θ).
    let k = 4.0 / quarters.max(1) as f32;
    let _ = ident3;
    // BEAM_NO_BEND=1: geometry left where the source put it (the pipeline bisect)
    // chord tolerance (`--chord-tol` metres, default 3 cm) → the largest angle a chord may
    // span at the TARGET radius: 2·acos(1 − tol/R)
    let tol: f32 = flag(args, "--chord-tol").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.03);
    let max_span = 2.0 * (1.0 - tol / radius.max(1.0)).clamp(-1.0, 1.0).acos();
    let added = subdivide_by_angle(&mut m, max_span);
    m.notes.push(format!("ring: {added} vertices added so no chord spans more than {:.1}° ({tol} m at r {radius})", max_span.to_degrees()));
    if std::env::var("BEAM_NO_BEND").map(|v| v == "1").unwrap_or(false) {
        m.notes.push("ring: NO BEND (bisect)".into());
    } else {
        remap_frames(&mut m, &|p| {
            let (theta, d, y) = polar(p);
            let r = radius + d;
            let t2 = theta * k;
            let (sd, cd) = (t2 - theta).sin_cos();
            ([cx + r * t2.cos(), y, cz - r * t2.sin()], [[cd, 0.0, sd], [0.0, 1.0, 0.0], [-sd, 0.0, cd]])
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
    // handedness: the beam frame is left-handed (t × r = −u); the target frame must be too,
    // or the map is a reflection. Reverse the traversal when the contour runs the other way.
    let path: Vec<[f32; 2]> = {
        let mut sgn = 0.0f32;
        for w in path.windows(2) {
            let (tz, ty) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
            let l = (tz * tz + ty * ty).sqrt().max(1e-6);
            let (tz, ty) = (tz / l, ty / l);
            let (mut nz, mut ny) = (-ty, tz);
            let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            if nz * (centroid[0] - mid[0]) + ny * (centroid[1] - mid[1]) > 0.0 {
                nz = -nz;
                ny = -ny;
            }
            sgn += ty * nz - tz * ny;
        }
        if sgn < 0.0 {
            let mut r = path.clone();
            r.reverse();
            r
        } else {
            path
        }
    };
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
    let feet = vec![std::f32::consts::FRAC_PI_2, -((copies.len() as f32 - 1.0) * std::f32::consts::FRAC_PI_2)];
    let mut m = source_beam(store, prefab, if args.iter().any(|a| a == "--keep-ends") { Ends::Keep } else { Ends::KeepAt(feet) }, &copies)?;
    let src_len = n_q as f32 * quarter_len;
    // BEAM_NO_ROT=1: positions bent, normals/tangents left (the bisect); BEAM_NO_BEND=1: nothing moved
    let no_rot = std::env::var("BEAM_NO_ROT").map(|v| v == "1").unwrap_or(false);
    // chord tolerance (`--chord-tol` metres, default 3 cm) at the contour's own scale: the
    // source angle maps onto the contour at (path length / source length) m per radian
    let tol: f32 = flag(args, "--chord-tol").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.03);
    let eff_r = (total / src_len) * SRC_R;
    let max_span = 2.0 * (1.0 - tol / eff_r.max(1.0)).clamp(-1.0, 1.0).acos();
    let added = subdivide_by_angle(&mut m, max_span);
    m.notes.push(format!("arch: {added} vertices added so no chord spans more than {:.1}° ({tol} m at an effective radius of {eff_r:.1} m)", max_span.to_degrees()));
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
        // source frame at θ: s grows as θ FALLS (s = (π/2 − θ)·R), so the tangent for
        // increasing s is d(p)/d(−θ) = (sinθ, 0, cosθ) — the first build had it negated, a
        // reflection: the arch came out mirror-imaged with its skin inside out (Everios96
        // 16:09 "the entire outside texture is flipped"); radial (cosθ, 0, −sinθ), up (0,1,0).
        // target: tangent (0, t[1], t[0]), radial (0, nrm[1], nrm[0]), up (−1, 0, 0).
        // R = T_target · T_source^T; both frames are left-handed (t × r = −u), so R is a
        // proper rotation when the contour runs so that t[1]·nrm[0] − t[0]·nrm[1] = +1
        // (checked on the TrackWallArch contour: +1).
        let (sn, cs) = theta.sin_cos();
        let a = [[0.0, 0.0, -1.0], [t[1], nrm[1], 0.0], [t[0], nrm[0], 0.0]];
        let b = [[sn, 0.0, cs], [cs, 0.0, -sn], [0.0, 1.0, 0.0]];
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
