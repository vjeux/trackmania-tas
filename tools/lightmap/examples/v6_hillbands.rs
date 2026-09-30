//! `v6_hillbands OURS.Map.Gbx --against EDITOR.Map.Gbx --source SRC.Map.Gbx --records R.tsv --collection C --quality Q
//!   --pak FILE:KEY… [--models SUBSTR,…] [--sun DX,DY,DZ] [--lit-hdr 1e-3] [--min-texels 500] [--own-rects] [--editor-min S] [--near PTS.tsv --radius R] [--out TSV]`
//! `--near PTS.tsv --radius R`: a TEXEL-level spatial split — the texel's world position (the fragment's) within R m (x, z) of a
//! listed point (classcmp::read_points: a census TSV) → the extra bands `N0 near ≤R` / `N1 far` (V6 row 4: the modded
//! TrackBorders placements; classcmp --near is chart-centre based and lumps a whole hill into one side).
//! `--editor-min S`: only the texels whose EDITOR value has Σrgb ≥ S count (RedIsland: the dry items at S 0.8 — the under-lake
//! placements of the same models are a separate defect, V5-5).
//! `--own-rects` (the RedIsland cells: the packing order differs): the editor's chart of the same (obj, sub) bind word, each of our
//! texels mapped PROPORTIONALLY into its rect (the same uv1 → rect affine on both sides); pairs whose rect areas differ > ×2.9 refused.
//!
//! THE HILLS' YARDSTICK BY FACE ORIENTATION (verification engineer V6, 2026-09-30, the coordinator's row 3): every texel of the named
//! item classes is placed on its receiver geometry through the bake's own LM raster (localdrive::setup_from_map → the LM scene →
//! lmaccum::build_frag_list at the atlas size: per pixel the world position and the WORLD NORMAL the accumulate shades with), then
//! binned by (a) the normal's ELEVATION (flat ≥ 75°, sloped 45–75°, steep 15–45°, vertical < 15°, down-facing) and (b) the cosine to
//! the sun (facing ≥ 0.5, grazing 0–0.5, back < 0). Per (class, band): texels, the lit fraction ours / editor, the C0 colour ratio
//! ours/editor per channel over the EDITOR's lit texels (the HDR decode of classcmp), both sides' chroma (G/R, B/R), and the H-basis
//! directional planes C1..C3 decoded as texeldelta does (mean |v| ratio + sign agreement). A face-orientation band IS an
//! incoming-direction band: a flat face integrates the zenith half of the sweep (k0-like directions), a vertical face the horizon
//! band (k38/k39) — so this table is the target for E7's per-direction env-raster compare.
//! The frag list's pixel grid = the stored atlas (2048²): the self-check column `rec✓` counts texels whose fragment belongs to the
//! chart's own record (pair → instance → rec_of).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let fl = |k: &str| -> Vec<String> { let mut out = Vec::new(); let mut i = 0; while i < a.len() { if a[i] == k { if let Some(v) = a.get(i + 1) { out.push(v.clone()); } } i += 1; } out };
    let ours_p = a.get(1).expect("OURS.Map.Gbx").clone();
    let theirs_p = f("--against").expect("--against EDITOR.Map.Gbx");
    let source = f("--source").expect("--source SRC.Map.Gbx");
    let records = f("--records").expect("--records R.tsv");
    let collection = f("--collection").expect("--collection C");
    let quality: u32 = f("--quality").map(|v| v.parse().expect("--quality Q")).unwrap_or(4);
    let paks: Vec<(String, String)> = fl("--pak").iter().map(|p| { let (a, b) = p.rsplit_once(':').expect("--pak FILE:KEY"); (a.to_string(), b.to_string()) }).collect();
    let models: Vec<String> = f("--models").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let sun: [f32; 3] = f("--sun").map(|v| { let p: Vec<f32> = v.split(',').map(|s| s.trim().parse().expect("--sun DX,DY,DZ")).collect(); [p[0], p[1], p[2]] }).unwrap_or([-0.22100, -0.48764, 0.84461]);
    let lit_hdr: f64 = f("--lit-hdr").map(|v| v.parse().expect("--lit-hdr F")).unwrap_or(1e-3);
    let min_texels: usize = f("--min-texels").map(|v| v.parse().expect("--min-texels N")).unwrap_or(500);
    let out = f("--out");
    let own_rects = a.iter().any(|x| x == "--own-rects");
    let editor_min: f64 = f("--editor-min").map(|v| v.parse().expect("--editor-min S")).unwrap_or(0.0);
    let near: Option<(Vec<(f32, f32)>, f32)> = f("--near").map(|p| { let pts = lightmap::classcmp::read_points(&p, f("--near-name").as_deref()).unwrap_or_else(|e| panic!("--near: {e}")); let r: f32 = f("--radius").map(|v| v.parse().expect("--radius R")).unwrap_or(30.0); eprintln!("--near: {} points, radius {r} m", pts.len()); (pts, r) });
    // the direction TO the sun (the bake's word is the light's travel)
    let to_sun = { let l = (sun[0] * sun[0] + sun[1] * sun[1] + sun[2] * sun[2]).sqrt(); [-sun[0] / l, -sun[1] / l, -sun[2] / l] };

    // 1. the two lightmaps: frame 0 image 0 (colour) + image 1's three planes
    let ours = lightmap::mapio::load(&ours_p).unwrap_or_else(|e| panic!("{ours_p}: {e}"));
    let theirs = lightmap::mapio::load(&theirs_p).unwrap_or_else(|e| panic!("{theirs_p}: {e}"));
    let (d1, d2) = (ours.chunk.data.as_ref().expect("ours: lightmap"), theirs.chunk.data.as_ref().expect("theirs: lightmap"));
    let (m1, m2) = (d1.cache.mapping().expect("ours: mapping"), d2.cache.mapping().expect("theirs: mapping"));
    if !own_rects { assert_eq!(m1.count, m2.count, "chart counts differ — the exact layout is required (or --own-rects)"); }
    let theirs_of: std::collections::HashMap<(u32, u32), usize> = (0..m2.count as usize).map(|j| ((m2.binds[j].obj_group_idx / 4, m2.binds[j].obj_idx & 0x00ff_ffff), j)).collect();
    let (f1, f2) = (&d1.frames[0], &d2.frames[0]);
    let i1 = lightmap::img::decode_webp(&f1.images[0]).expect("ours image 0");
    let i2 = lightmap::img::decode_webp(&f2.images[0]).expect("theirs image 0");
    if !own_rects { assert!(i1.w == i2.w && i1.h == i2.h, "image sizes differ"); }
    let planes1: Vec<lightmap::img::Rgb> = f1.images.get(1).map(|b| lightmap::texeldelta::riff_parts(b).iter().map(|p| lightmap::img::decode_webp(p).expect("ours plane")).collect()).unwrap_or_default();
    let planes2: Vec<lightmap::img::Rgb> = f2.images.get(1).map(|b| lightmap::texeldelta::riff_parts(b).iter().map(|p| lightmap::img::decode_webp(p).expect("theirs plane")).collect()).unwrap_or_default();
    let np = planes1.len().min(planes2.len()).min(3);
    let k1 = lightmap::classcmp::record_maxhdr(&m1, 0).expect("ours record");
    let k2 = lightmap::classcmp::record_maxhdr(&m2, 0).expect("theirs record");
    let hb = |m: &lightmap::format::Mapping| -> [f32; 3] { let r = 60; if r + 66 <= m.head.len() { [54usize, 58, 62].map(|o| f32::from_le_bytes(m.head[r + o..r + o + 4].try_into().unwrap())) } else { [1.0; 3] } };
    let (hb1, hb2) = (hb(&m1), hb(&m2));
    let cval = |b: u8, k: usize, hbw: [f32; 3]| -> f64 { let t = b as f64 / 255.0 - 0.5; t.signum() * (t / 0.5) * (t / 0.5) * (hbw[k] as f64 / 0.6909883) };
    let fb1 = &m1.frame_bytes[0];
    let fb2 = &m2.frame_bytes[0];
    let rows_v = lightmap::classcmp::read_records_tsv(&records).unwrap_or_else(|e| panic!("{e}"));
    let rows: std::collections::HashMap<(u32, u32), lightmap::classcmp::RecRow> = rows_v.iter().map(|r| ((r.obj, r.sub), r.clone())).collect();
    eprintln!("lightmaps: {} charts, image {}×{}, planes {np}, MaxHDR ours {k1} / editor {k2}, HBasis words ours {hb1:?} / editor {hb2:?}", m1.count, i1.w, i1.h);

    // 2. the bake's LM scene of the source map + the fragment list at the atlas size
    let mut log = |s: &str| eprintln!("setup: {s}");
    let setup = lightmap::localdrive::setup_from_map(&source, &paks, &collection, quality, &mut log).unwrap_or_else(|e| panic!("setup_from_map: {e}"));
    let sc = &setup.sc;
    let t0 = std::time::Instant::now();
    // the accumulate rasterises at TWICE the stored image (the layout units; the stored atlas is the half-resolution image,
    // classcmp::chart_own_px) — one stored texel = the 2×2 accumulate pixels under it
    let (fw, fh) = (i1.w * 2, i1.h * 2);
    let flist = lightmap::lmaccum::build_frag_list(sc, 0, fw, fh);
    eprintln!("frag list: {:?} ({:.1} s); layout {} charts / {} records; rec_of {}", flist, t0.elapsed().as_secs_f32(), setup.gl.charts.len(), setup.gl.records.len(), sc.rec_of.len());
    // per stored texel: the mean normal (normalised) over the 2×2 pixels' fragments and the record of the first fragment
    let n_px = (i1.w * i1.h) as usize;
    let mut nrm: Vec<[f32; 3]> = vec![[0.0; 3]; n_px];
    let mut rec_px: Vec<u32> = vec![u32::MAX; n_px];
    let mut has: Vec<bool> = vec![false; n_px];
    // the GEOMETRIC normal (the triangle's cross product, world) of the first fragment — the stored vertex normal's sanity check
    let mut geo: Vec<[f32; 3]> = vec![[0.0; 3]; n_px];
    let mut posv: Vec<[f32; 3]> = vec![[0.0; 3]; n_px];
    let rows_of: Vec<[[f32; 3]; 3]> = sc.instances.iter().map(|inst| lightmap::sunpass::rotation_rows(inst.q)).collect();
    for ty in 0..i1.h { for tx in 0..i1.w {
        let p = (ty * i1.w + tx) as usize;
        let mut acc = [0f32; 3];
        let mut pacc = [0f32; 3];
        let mut nf = 0usize;
        let mut first: Option<(u32, u32)> = None;
        for sy in 0..2 { for sx in 0..2 {
            let q = ((ty * 2 + sy) * fw + (tx * 2 + sx)) as usize;
            let (s, e) = (flist.start[q] as usize, flist.start[q + 1] as usize);
            for fr in &flist.frags[s..e] { for c in 0..3 { acc[c] += fr.nrm[c]; pacc[c] += fr.pos[c]; } nf += 1; if first.is_none() { first = Some((fr.pair, fr.tri)); } }
        } }
        let l = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt();
        if l > 1e-6 { nrm[p] = [acc[0] / l, acc[1] / l, acc[2] / l]; has[p] = true; }
        if nf > 0 { posv[p] = [pacc[0] / nf as f32, pacc[1] / nf as f32, pacc[2] / nf as f32]; }
        if let Some((pair, tri)) = first {
            let (m, ii) = flist.pairs[pair as usize];
            rec_px[p] = sc.rec_of.get(ii as usize).map(|&r| r as u32).unwrap_or(u32::MAX);
            let mesh = &sc.meshes[m as usize];
            let inst = &sc.instances[ii as usize];
            let t = tri as usize * 3;
            if t + 2 < mesh.indices.len() {
                let w = |k: usize| lightmap::lmaccum::world_pos(&mesh.verts[mesh.indices[t + k] as usize], inst, &rows_of[ii as usize]);
                let (p0, p1, p2) = (w(0), w(1), w(2));
                let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
                let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
                let c = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
                if l > 1e-9 { geo[p] = [c[0] / l, c[1] / l, c[2] / l]; }
            }
        }
    } }

    // 3. the bands
    let elev_band = |n: [f32; 3]| -> &'static str {
        let e = n[1].clamp(-1.0, 1.0).asin().to_degrees();
        if n[1] < 0.0 { "E4 down-facing" } else if e >= 75.0 { "E0 flat ≥75°" } else if e >= 45.0 { "E1 sloped 45–75°" } else if e >= 15.0 { "E2 steep 15–45°" } else { "E3 vertical <15°" }
    };
    let sun_band = |n: [f32; 3]| -> &'static str {
        let c = n[0] * to_sun[0] + n[1] * to_sun[1] + n[2] * to_sun[2];
        if c >= 0.5 { "S0 sun-facing c≥0.5" } else if c >= 0.0 { "S1 grazing 0–0.5" } else { "S2 back c<0" }
    };
    #[derive(Default, Clone)]
    struct Acc { texels: usize, no_frag: usize, rec_ok: usize, lit_o: usize, lit_e: usize, used: usize, so: [f64; 3], se: [f64; 3], pabs_o: [f64; 3], pabs_e: [f64; 3], pn: [usize; 3], sign_ok: [usize; 3], sign_n: [usize; 3], cos_sum: f64, geo_agree: usize, geo_n: usize, geo_ny: f64 }
    let mut acc: std::collections::BTreeMap<(String, String), Acc> = Default::default();
    let n = m1.count as usize;
    let mut charts_seen = 0usize;
    let mut refused = 0usize;
    for i in 0..n {
        let obj = m1.binds[i].obj_group_idx / 4;
        let sub = m1.binds[i].obj_idx & 0x00ff_ffff;
        let Some(r) = rows.get(&(obj, sub)) else { continue };
        let key = format!("{}:{}", r.class, r.name);
        if !models.is_empty() && !models.iter().any(|s| key.contains(s.as_str())) { continue; }
        let j = if own_rects {
            let Some(&j) = theirs_of.get(&(obj, sub)) else { refused += 1; continue };
            let (a1, a2) = (m1.size[i].0 as f64 * m1.size[i].1 as f64, m2.size[j].0 as f64 * m2.size[j].1 as f64);
            if a1 <= 0.0 || a2 <= 0.0 || a1 / a2 > 2.9 || a2 / a1 > 2.9 { refused += 1; continue }
            j
        } else { i };
        charts_seen += 1;
        let (fbi, fbj) = (fb1.get(i).copied().unwrap_or(0), fb2.get(j).copied().unwrap_or(0));
        let (px, py, pw, ph) = lightmap::classcmp::chart_own_px(m1.pos[i], m1.size[i]);
        let (qx, qy, qw, qh) = lightmap::classcmp::chart_own_px(m2.pos[j], m2.size[j]);
        for y in py..(py + ph).min(i1.h) { for x in px..(px + pw).min(i1.w) {
            let p = (y * i1.w + x) as usize;
            let (eb, sb) = if has[p] { (elev_band(nrm[p]), sun_band(nrm[p])) } else { ("E? no fragment", "S? no fragment") };
            let nb: Option<&'static str> = near.as_ref().map(|(pts, r)| {
                if !has[p] { return "N? no fragment" }
                let (x0, z0) = (posv[p][0], posv[p][2]);
                let r2 = r * r;
                if pts.iter().any(|(px, pz)| { let (dx, dz) = (px - x0, pz - z0); dx * dx + dz * dz <= r2 }) { "N0 near" } else { "N1 far" }
            });
            let a = i1.get(x, y);
            // the editor's texel: the same position, or the proportional one in its own rect
            let (ex, ey) = if own_rects {
                let u = (x - px) as f64 + 0.5; let v = (y - py) as f64 + 0.5;
                ((qx + ((u / pw.max(1) as f64) * qw as f64).floor() as u32).min(i2.w - 1), (qy + ((v / ph.max(1) as f64) * qh as f64).floor() as u32).min(i2.h - 1))
            } else { (x, y) };
            let b = i2.get(ex, ey);
            let lo = (0..3).map(|c| lightmap::classcmp::texel_hdr(0, a[c], fbi, k1)).fold(0.0, f64::max) >= lit_hdr;
            let le = (0..3).map(|c| lightmap::classcmp::texel_hdr(0, b[c], fbj, k2)).fold(0.0, f64::max) >= lit_hdr
                && (editor_min <= 0.0 || (0..3).map(|c| lightmap::classcmp::texel_hdr(0, b[c], fbj, k2)).sum::<f64>() >= editor_min);
            let mut bands = vec![format!("all"), eb.to_string(), sb.to_string(), format!("{eb} × {sb}")];
            if let Some(nb) = nb { bands.push(nb.to_string()); bands.push(format!("{nb} × {eb}")); }
            for band in bands {
                let e = acc.entry((key.clone(), band)).or_default();
                e.texels += 1;
                if !has[p] { e.no_frag += 1; } else if rec_px[p] as usize == i { e.rec_ok += 1; }
                if has[p] { e.cos_sum += (nrm[p][0] * to_sun[0] + nrm[p][1] * to_sun[1] + nrm[p][2] * to_sun[2]) as f64; }
                if has[p] && (geo[p][0] != 0.0 || geo[p][1] != 0.0 || geo[p][2] != 0.0) { e.geo_n += 1; e.geo_ny += geo[p][1] as f64; if nrm[p][0] * geo[p][0] + nrm[p][1] * geo[p][1] + nrm[p][2] * geo[p][2] > 0.0 { e.geo_agree += 1; } }
                if lo { e.lit_o += 1; }
                if le {
                    e.lit_e += 1; e.used += 1;
                    for c in 0..3 { e.so[c] += lightmap::classcmp::texel_hdr(0, a[c], fbi, k1); e.se[c] += lightmap::classcmp::texel_hdr(0, b[c], fbj, k2); }
                    for k in 0..np {
                        let (vo, ve) = (cval(planes1[k].get(x, y)[0], k, hb1), cval(planes2[k].get(ex, ey)[0], k, hb2));
                        e.pabs_o[k] += vo.abs(); e.pabs_e[k] += ve.abs(); e.pn[k] += 1;
                        if ve.abs() >= 0.05 * hb2[k] as f64 / 0.6909883 { e.sign_n[k] += 1; if (vo >= 0.0) == (ve >= 0.0) { e.sign_ok[k] += 1; } }
                    }
                }
            }
        } }
    }
    eprintln!("{charts_seen} charts matched the model filter {models:?}; {refused} pairs refused (own rects: no bind match or area guard)");
    // 4. print
    let mut lines: Vec<String> = Vec::new();
    let head = "class\tband\ttexels\tno_frag\trec_ok%\tmean_cos_sun\tlit_ours%\tlit_editor%\tused\tratio_r\tratio_g\tratio_b\tours_G/R\tours_B/R\ted_G/R\ted_B/R\tC1_abs_ratio\tC2_abs_ratio\tC3_abs_ratio\tC1_sign%\tC2_sign%\tC3_sign%\tmean_ed_r\tmean_ed_g\tmean_ed_b\tgeo_agree%\tgeo_ny_mean";
    println!("{head}");
    lines.push(head.to_string());
    // classes in the order of their texel count
    let mut classes: Vec<(String, usize)> = acc.iter().filter(|((_, b), _)| b == "all").map(|((c, _), e)| (c.clone(), e.texels)).collect();
    classes.sort_by(|a, b| b.1.cmp(&a.1));
    for (cls, tex) in &classes {
        if *tex < min_texels { continue; }
        for ((c, band), e) in acc.iter() {
            if c != cls || e.texels < min_texels.min(64) { continue; }
            let u = e.used.max(1) as f64;
            let mo = [e.so[0] / u, e.so[1] / u, e.so[2] / u];
            let me = [e.se[0] / u, e.se[1] / u, e.se[2] / u];
            let rat = |c: usize| if e.used >= 16 && me[c] > 1e-9 { mo[c] / me[c] } else { f64::NAN };
            let chroma = |m: [f64; 3], c: usize| if m[0] > 1e-9 { m[c] / m[0] } else { f64::NAN };
            let pr = |k: usize| if e.pn[k] > 0 && e.pabs_e[k] > 1e-9 { e.pabs_o[k] / e.pabs_e[k] } else { f64::NAN };
            let sg = |k: usize| if e.sign_n[k] > 0 { 100.0 * e.sign_ok[k] as f64 / e.sign_n[k] as f64 } else { f64::NAN };
            let nf = (e.texels - e.no_frag).max(1) as f64;
            let line = format!("{c}\t{band}\t{}\t{}\t{:.1}\t{:.3}\t{:.1}\t{:.1}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.1}\t{:.1}\t{:.1}\t{:.5}\t{:.5}\t{:.5}\t{:.1}\t{:.3}",
                e.texels, e.no_frag, 100.0 * e.rec_ok as f64 / nf, e.cos_sum / nf, 100.0 * e.lit_o as f64 / e.texels as f64, 100.0 * e.lit_e as f64 / e.texels as f64, e.used,
                rat(0), rat(1), rat(2), chroma(mo, 1), chroma(mo, 2), chroma(me, 1), chroma(me, 2), pr(0), pr(1), pr(2), sg(0), sg(1), sg(2), me[0], me[1], me[2], 100.0 * e.geo_agree as f64 / e.geo_n.max(1) as f64, e.geo_ny / e.geo_n.max(1) as f64);
            println!("{line}");
            lines.push(line);
        }
    }
    if let Some(o) = out { std::fs::write(&o, lines.join("\n") + "\n").unwrap_or_else(|e| panic!("{o}: {e}")); eprintln!("wrote {o}"); }
}
