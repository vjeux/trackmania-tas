//! `re7_lodfill EDITOR.Map.Gbx ITEM.Item.Gbx CHART` — does the game light texels of a chart that only the item's LOD-1/2
//! triangles reach? Rasterise every shaded geom's triangles (TexCoord1 → the chart rect through the PreLightGen uv bounds)
//! per lod mask, and compare with the editor image's lit texels (RE 7, 2026-09-26).
use mapgeom::static_item::vstream::{Elem, N_POSITION, N_TEXCOORD0};
use mapgeom::static_item::Node;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = lightmap::mapio::load(&a[1]).expect("map");
    let d = m.chunk.data.as_ref().expect("data");
    let mp = d.cache.mapping().expect("mapping");
    let chart: usize = a[3].parse().unwrap();
    let im = &d.frames[0].images[0];
    let sz = u32::from_le_bytes([im[4], im[5], im[6], im[7]]) as usize + 8;
    let img0 = lightmap::img::decode_webp(&im[..sz.min(im.len())]).expect("webp 0");
    let w = img0.w as usize;
    let (x2, y2) = mp.pos[chart];
    let (w2, h2) = mp.size[chart];
    let x0 = ((x2 as usize).saturating_sub(1)) / 2;
    let y0 = ((y2 as usize).saturating_sub(1)) / 2;
    let cw = (w2 as usize) / 2 + 1;
    let ch = (h2 as usize) / 2 + 1;
    let val: Vec<u32> = (0..cw * ch).map(|k| { let (x, y) = (x0 + k % cw, y0 + k / cw); let o = (y * w + x) * 3; img0.px[o] as u32 + img0.px[o + 1] as u32 + img0.px[o + 2] as u32 }).collect();
    let lit: Vec<bool> = val.iter().map(|&v| v > 0).collect();
    let b = std::fs::read(&a[2]).expect("item");
    let f = mapgeom::static_item::file::parse_file(&b).expect("parse");
    let so = f.item.static_object().expect("static object");
    let s2 = so.solid2().expect("solid2");
    let plg = s2.pre_light_gen.as_ref().expect("plg");
    let (umin, vmin, umax, vmax) = (plg.u04[0], plg.u04[1], plg.u04[2], plg.u04[3]);
    println!("chart {chart}: img rect ({x0},{y0}) {cw}×{ch}; editor lit {}/{}; PLG uv bounds ({umin}, {vmin})..({umax}, {vmax}) u02 {}", lit.iter().filter(|&&b| b).count(), cw * ch, plg.u02);
    // rasterise per lod mask
    let mut cover_by_mask: std::collections::BTreeMap<u32, Vec<bool>> = Default::default();
    for sg in &s2.shaded_geoms {
        let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
        let Some(Node::Visual(v)) = vr.inline.as_deref() else { continue };
        let Some(ib) = v.index_buffer.as_ref() else { continue };
        let Some(st) = v.stream() else { continue };
        let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
        let uv1: Option<Vec<[f32; 2]>> = match get(N_TEXCOORD0 + 1) { Some(Elem::Float2(u)) => Some(u.clone()), _ => v.main.as_ref().and_then(|m| m.tex_coord_sets.get(1)).map(|s| s.coords.iter().map(|c| c.0).collect()) };
        let Some(mut uv1) = uv1 else { println!("  geom v{} lod {}: no TexCoord1", sg.visual_index, sg.lod_mask); continue };
        let mode = std::env::var("RE7_UV").unwrap_or_default();
        let tc1_space = st.decls.iter().find(|d| d.name() == N_TEXCOORD0 + 1).map(|d| d.space()).unwrap_or(9);
        if mode == "0" || (mode == "space" && tc1_space != 2) {
            if let Some(Elem::Float2(u)) = get(N_TEXCOORD0) { uv1 = u.clone(); println!("  geom v{}: using TexCoord0 as the lightmap uv (TexCoord1 space {tc1_space})", sg.visual_index); }
        }
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for uv in &uv1 { for k in 0..2 { lo[k] = lo[k].min(uv[k]); hi[k] = hi[k].max(uv[k]); } }
        let uv0: Option<Vec<[f32; 2]>> = match get(N_TEXCOORD0) { Some(Elem::Float2(u)) => Some(u.clone()), _ => None };
        let (mut lo0, mut hi0) = ([f32::MAX; 2], [f32::MIN; 2]);
        if let Some(u0) = &uv0 { for uv in u0 { for k in 0..2 { lo0[k] = lo0[k].min(uv[k]); hi0[k] = hi0[k].max(uv[k]); } } }
        println!("  geom v{} uv1 range ({:.3},{:.3})..({:.3},{:.3}); uv0 range ({:.3},{:.3})..({:.3},{:.3}); {} verts; sets {}", sg.visual_index, lo[0], lo[1], hi[0], hi[1], lo0[0], lo0[1], hi0[0], hi0[1], uv1.len(), v.main.as_ref().map(|m| m.tex_coord_sets.len()).unwrap_or(0));
        let _ = get(N_POSITION);
        let cov = cover_by_mask.entry(sg.lod_mask as u32).or_insert_with(|| vec![false; cw * ch]);
        let mut n = 0usize;
        let mut area = 0.0f64;
        for t in ib.indices.chunks_exact(3) {
            let p: Vec<(f64, f64)> = t.iter().map(|&i| { let uv = uv1[i as usize]; ((((uv[0] - umin) / (umax - umin)) * cw as f32) as f64, (((uv[1] - vmin) / (vmax - vmin)) * ch as f32) as f64) }).collect();
            area += ((p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1)).abs() * 0.5;
        }
        if std::env::var_os("RE7_TRIS").is_some() { for t in ib.indices.chunks_exact(3).take(4) { println!("    tri {:?}", t.iter().map(|&i| uv1[i as usize]).collect::<Vec<_>>()); } }
        println!("  geom v{} uv-space triangle area {:.0} texels ({:.3} of the chart); index buffer {} indices, max index {}", sg.visual_index, area, area / (cw * ch) as f64, ib.indices.len(), ib.indices.iter().max().unwrap_or(&0));
        for t in ib.indices.chunks_exact(3) {
            let p: Vec<(f32, f32)> = t.iter().map(|&i| { let uv = uv1[i as usize]; (((uv[0] - umin) / (umax - umin)) * cw as f32, ((uv[1] - vmin) / (vmax - vmin)) * ch as f32) }).collect();
            // conservative raster: every texel whose centre is inside the triangle, plus the texels the edges touch
            let (minx, maxx) = (p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor().max(0.0) as usize, p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil().min(cw as f32) as usize);
            let (miny, maxy) = (p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize, p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil().min(ch as f32) as usize);
            for y in miny..maxy { for x in minx..maxx {
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let e = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0) * (cy - a.1) - (b.1 - a.1) * (cx - a.0);
                let (e0, e1, e2) = (e(p[0], p[1]), e(p[1], p[2]), e(p[2], p[0]));
                let tol = 0.75; // half a texel of slack: the GPU rasteriser + the 8-px dilation
                if (e0 >= -tol && e1 >= -tol && e2 >= -tol) || (e0 <= tol && e1 <= tol && e2 <= tol) { cov[y * cw + x] = true; n += 1; }
            } }
        }
        // per geom: its own coverage vs the editor's lit mask (uv1), and the same through uv0
        let mut own = vec![false; cw * ch];
        for t in ib.indices.chunks_exact(3) {
            let p: Vec<(f32, f32)> = t.iter().map(|&i| { let uv = uv1[i as usize]; (((uv[0] - umin) / (umax - umin)) * cw as f32, ((uv[1] - vmin) / (vmax - vmin)) * ch as f32) }).collect();
            let (minx, maxx) = (p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor().max(0.0) as usize, p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil().min(cw as f32) as usize);
            let (miny, maxy) = (p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor().max(0.0) as usize, p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil().min(ch as f32) as usize);
            for y in miny..maxy { for x in minx..maxx {
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let e = |a: (f32, f32), b: (f32, f32)| (b.0 - a.0) * (cy - a.1) - (b.1 - a.1) * (cx - a.0);
                let (e0, e1, e2) = (e(p[0], p[1]), e(p[1], p[2]), e(p[2], p[0]));
                if (e0 >= -0.75 && e1 >= -0.75 && e2 >= -0.75) || (e0 <= 0.75 && e1 <= 0.75 && e2 <= 0.75) { own[y * cw + x] = true; }
            } }
        }
        let oc = own.iter().filter(|&&b| b).count();
        let ol = own.iter().zip(lit.iter()).filter(|(a, b)| **a && **b).count();
        println!("  geom v{} material {} lod mask {}: {} tris, {} texel hits; own coverage {} texels, lit in the editor {} ({:.2})", sg.visual_index, sg.material_index, sg.lod_mask, ib.indices.len() / 3, n, oc, ol, ol as f64 / oc.max(1) as f64);
    }
    let mut all = vec![false; cw * ch];
    for (mask, cov) in &cover_by_mask {
        let c = cov.iter().filter(|&&b| b).count();
        let both = cov.iter().zip(lit.iter()).filter(|(a, b)| **a && **b).count();
        println!("lod mask {mask}: covers {c} texels ({:.3}); of them lit in the editor {both}", c as f64 / (cw * ch) as f64);
        for (k, &b) in cov.iter().enumerate() { if b { all[k] = true; } }
    }
    let c = all.iter().filter(|&&b| b).count();
    let lit_not_cov = lit.iter().zip(all.iter()).filter(|(l, c)| **l && !**c).count();
    let cov_not_lit = lit.iter().zip(all.iter()).filter(|(l, c)| !**l && **c).count();
    println!("all lods: covers {c} ({:.3}); editor lit but uncovered {lit_not_cov}; covered but unlit {cov_not_lit}", c as f64 / (cw * ch) as f64);
    // the magnitude of the uncovered-but-lit texels vs the covered ones (RGB sums, 0..765)
    let mut h_unc = [0usize; 8];
    let mut h_cov = [0usize; 8];
    for k in 0..cw * ch { let b = (val[k] as usize / 96).min(7); if lit[k] { if all[k] { h_cov[b] += 1; } else { h_unc[b] += 1; } } }
    println!("RGB-sum histogram (bins of 96): covered&lit {:?}; uncovered&lit {:?}", h_cov, h_unc);
    // the value map (RGB sum / 96 as a digit) at the same step, to see the structure of the fill
    if std::env::var_os("RE7_VALUES").is_some() {
        let step = ((cw + 119) / 120).max((ch + 79) / 80).max(1);
        for y in (0..ch).step_by(step) { let mut s = String::new(); for x in (0..cw).step_by(step) { let v = val[y * cw + x]; s.push(if v == 0 { '.' } else { char::from_digit((v / 96).min(7), 10).unwrap() }); } println!("{s}"); }
    }
    // an ASCII map: '#' lit&covered, 'L' lit only, 'c' covered only, '.' neither
    let step = ((cw + 119) / 120).max((ch + 79) / 80).max(1);
    for y in (0..ch).step_by(step) { let mut s = String::new(); for x in (0..cw).step_by(step) { let k = y * cw + x; s.push(match (lit[k], all[k]) { (true, true) => '#', (true, false) => 'L', (false, true) => 'c', _ => '.' }); } println!("{s}"); }
}
