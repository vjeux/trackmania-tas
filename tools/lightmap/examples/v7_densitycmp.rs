//! `v7_densitycmp OURS.Map.Gbx --against EDITOR.Map.Gbx --records R.tsv [--source SRC.Map.Gbx --collection C --quality Q --pak FILE:KEY…]
//!   [--lit-hdr 1e-3] [--editor-min S] [--min-texels 500] [--guard 2.9] [--by name|class] [--models SUBSTR,…] [--out TSV] [--pairs TSV]`
//!
//! THE TEXEL-DENSITY QUESTION (verification engineer V7, 2026-10-01, the coordinator's row 1): on RedIsland our layout is not the
//! game's (E7's e7_mapcmp: 0/10 948 rects identical; the tiles 18×18 where the game's are 4×4; items at ×0.8 … ×3 of the game's
//! linear size), so every per-class ratio read with `--own-rects` compares means taken at DIFFERENT texel densities. This tool pairs
//! every chart by its (obj, sub) bind word (no area guard unless `--guard`), and for each pair reads the same light four ways:
//!   own     — each side's mean over its OWN rect and its OWN lit texels (= `classcmp --own-rects`, V6-3's numbers);
//!   fine    — on the FINER side's grid: the coarser side's texel taken NEAREST (= `v6_hillbands --own-rects` when ours is the finer);
//!   coarse  — on the COARSER side's grid: the finer side box-filtered onto it (exact fractional footprints, lit-texel-normalised =
//!             a coverage-normalised rasterisation of the fine field at the coarse density);
//!   coarse/interior and coarse/edge — the coarse read split by whether the coarse texel's footprint is FULLY covered by geometry in
//!             our bake's LM raster (the frag list, as v6_hillbands) and fully lit on the fine side; an edge texel is where a coarse
//!             rasterisation (the game's) and a fine one differ in kind (partial coverage, dilation).
//! Means run over the EDITOR's lit texels of the grid in question (`--lit-hdr`; `--editor-min S` = the dry filter of V6-3: the editor's
//! Σrgb ≥ S). The density-INDEPENDENT yardstick per class = the coarse/interior ratio; `coarse − own` = the density artefact of the
//! V6-3 read; `all pairs − guarded pairs` = the population the ×2.9 area guard had dropped.

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let fl = |k: &str| -> Vec<String> { let mut out = Vec::new(); let mut i = 0; while i < a.len() { if a[i] == k { if let Some(v) = a.get(i + 1) { out.push(v.clone()); } } i += 1; } out };
    let ours_p = a.get(1).expect("OURS.Map.Gbx").clone();
    let theirs_p = f("--against").expect("--against EDITOR.Map.Gbx");
    let records = f("--records").expect("--records R.tsv");
    let source = f("--source");
    let collection = f("--collection").unwrap_or_else(|| "RedIsland".into());
    let quality: u32 = f("--quality").map(|v| v.parse().expect("--quality Q")).unwrap_or(4);
    let paks: Vec<(String, String)> = fl("--pak").iter().map(|p| { let (a, b) = p.rsplit_once(':').expect("--pak FILE:KEY"); (a.to_string(), b.to_string()) }).collect();
    let lit_hdr: f64 = f("--lit-hdr").map(|v| v.parse().expect("--lit-hdr F")).unwrap_or(1e-3);
    let editor_min: f64 = f("--editor-min").map(|v| v.parse().expect("--editor-min S")).unwrap_or(0.0);
    let min_texels: usize = f("--min-texels").map(|v| v.parse().expect("--min-texels N")).unwrap_or(500);
    let guard: Option<f64> = f("--guard").map(|v| v.parse().expect("--guard R"));
    let by_name = f("--by").map(|v| v != "class").unwrap_or(true);
    let models: Vec<String> = f("--models").map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let out = f("--out");
    let pairs_out = f("--pairs");
    // `--split-ratio`: the class key carries the pair's linear size ratio bin (ours/editor, ×10 rounded) — which density regime carries a term
    let split_ratio = a.iter().any(|x| x == "--split-ratio");

    // 1. the two lightmaps: frame 0 image 0 decoded to HDR per texel
    let ours = lightmap::mapio::load(&ours_p).unwrap_or_else(|e| panic!("{ours_p}: {e}"));
    let theirs = lightmap::mapio::load(&theirs_p).unwrap_or_else(|e| panic!("{theirs_p}: {e}"));
    let (d1, d2) = (ours.chunk.data.as_ref().expect("ours: lightmap"), theirs.chunk.data.as_ref().expect("theirs: lightmap"));
    let (m1, m2) = (d1.cache.mapping().expect("ours: mapping"), d2.cache.mapping().expect("theirs: mapping"));
    let bind = |m: &lightmap::format::Mapping, i: usize| (m.binds[i].obj_group_idx / 4, m.binds[i].obj_idx & 0x00ff_ffff);
    let theirs_of: std::collections::HashMap<(u32, u32), usize> = (0..m2.count as usize).map(|j| (bind(&m2, j), j)).collect();
    let i1 = lightmap::img::decode_webp(&d1.frames[0].images[0]).expect("ours image 0");
    let i2 = lightmap::img::decode_webp(&d2.frames[0].images[0]).expect("theirs image 0");
    let k1 = lightmap::classcmp::record_maxhdr(&m1, 0).expect("ours record");
    let k2 = lightmap::classcmp::record_maxhdr(&m2, 0).expect("theirs record");
    let (fb1, fb2) = (&m1.frame_bytes[0], &m2.frame_bytes[0]);
    let rows_v = lightmap::classcmp::read_records_tsv(&records).unwrap_or_else(|e| panic!("{e}"));
    let rows: std::collections::HashMap<(u32, u32), lightmap::classcmp::RecRow> = rows_v.iter().map(|r| ((r.obj, r.sub), r.clone())).collect();
    eprintln!("ours {} charts {}×{} MaxHDR {k1}; editor {} charts {}×{} MaxHDR {k2}; records {}", m1.count, i1.w, i1.h, m2.count, i2.w, i2.h, rows.len());

    // 2. OUR texels' geometry coverage from the bake's LM raster (optional): has[p] per stored texel, aligned to the stored mapping
    let mut has: Vec<bool> = Vec::new();
    if let Some(source) = &source {
        let scene = lightmap::geometry::Scene::from_map(source).unwrap_or_else(|e| panic!("scene: {e}"));
        let mf = tmmaps::map::MapFile::load(std::path::Path::new(source));
        let (pp, key) = paks.first().expect("--pak FILE:KEY");
        let base: u32 = 4096;
        let zone = lightmap::layout::ground_zone(&mf, &collection);
        let mut gl = lightmap::layout::for_map(source, &scene, base, quality.saturating_sub(1), lightmap::layout::TilePlg::BLUEBAY_SEA, Some((pp.as_str(), key.as_str())), &collection, &zone, None).unwrap_or_else(|e| panic!("layout: {e}"));
        let mine: std::collections::HashMap<(u32, u32), usize> = (0..m1.count as usize).map(|i| (bind(&m1, i), i)).collect();
        let (mut aligned, mut unmatched, mut moved) = (0usize, 0usize, 0usize);
        for k in 0..gl.records.len().min(gl.charts.len()) {
            match mine.get(&(gl.records[k].obj, gl.records[k].sub)) {
                Some(&i) => {
                    let (x, y, w, h) = (m1.pos[i].0 as i32, m1.pos[i].1 as i32, m1.size[i].0 as i32, m1.size[i].1 as i32);
                    if gl.charts[k].x != x || gl.charts[k].y != y || gl.charts[k].w != w || gl.charts[k].h != h { moved += 1; }
                    gl.charts[k].x = x; gl.charts[k].y = y; gl.charts[k].w = w; gl.charts[k].h = h; aligned += 1;
                }
                None => { gl.charts[k].w = 0; gl.charts[k].h = 0; unmatched += 1; }
            }
        }
        eprintln!("layout: {} charts / {} records; aligned to the mapping {aligned} (moved {moved}), unmatched {unmatched}", gl.charts.len(), gl.records.len());
        let mut store = mapgeom::store::DataStore::empty();
        for (p, k) in &paks { store.add_pak(p, k).unwrap_or_else(|e| panic!("pak {p}: {e}")); }
        let tile_world_y = lightmap::layout::tile_level(&mf, &collection) as f32 * 8.0 + lightmap::layout::CollectionProfile::of(&collection).yoff;
        let tile_mesh = lightmap::lmmesh::lm_mesh_of_zone(&mut store, &collection, &zone).unwrap_or_else(|e| panic!("zone mesh: {e}"));
        let files = mapgeom::embedded::files(&mf).unwrap_or_else(|e| panic!("embedded: {e}"));
        let by_name_f: std::collections::BTreeMap<String, Vec<u8>> = files.iter().map(|(k, v)| (k.rsplit(['/', '\\']).next().unwrap_or(k).to_string(), v.clone())).collect();
        let tile_plg = gl.records.iter().find(|r| r.class == "tile").map(|r| lightmap::layout::TilePlg { meter_by_uv: r.meter_by_uv, bounds: r.uv }).unwrap_or(lightmap::layout::TilePlg::BLUEBAY_SEA);
        let mut sc = lightmap::lmmesh::lm_scene_from_map_at(&scene, &gl, base, &|name| by_name_f.get(name).cloned(), tile_mesh, tile_plg, 2048.0, tile_world_y).unwrap_or_else(|e| panic!("lm scene: {e}"));
        let n_ent = lightmap::lmmesh::lm_scene_add_entities(&mut store, &gl, &mut sc, 2048.0).unwrap_or_else(|e| panic!("entities: {e}"));
        let t0 = std::time::Instant::now();
        let (fw, fh) = (i1.w * 2, i1.h * 2);
        let flist = lightmap::lmaccum::build_frag_list(&sc, 0, fw, fh);
        eprintln!("LM scene: {} meshes, {} instances ({n_ent} entities); frag list {:?} ({:.1} s)", sc.meshes.len(), sc.instances.len(), flist, t0.elapsed().as_secs_f32());
        has = vec![false; (i1.w * i1.h) as usize];
        for ty in 0..i1.h { for tx in 0..i1.w {
            let mut nf = 0usize;
            for sy in 0..2 { for sx in 0..2 { let q = ((ty * 2 + sy) * fw + (tx * 2 + sx)) as usize; nf += (flist.start[q + 1] - flist.start[q]) as usize; } }
            has[(ty * i1.w + tx) as usize] = nf > 0;
        } }
    }
    let covered = |p: usize| -> bool { has.is_empty() || has[p] };

    // 3. per pair
    #[derive(Default, Clone)]
    struct Sum { n: usize, o: [f64; 3], e: [f64; 3] }
    impl Sum { fn add(&mut self, o: [f64; 3], e: [f64; 3]) { self.n += 1; for c in 0..3 { self.o[c] += o[c]; self.e[c] += e[c]; } } }
    #[derive(Default, Clone)]
    struct Acc { charts: usize, tex_o: usize, tex_e: usize, lin_sum: f64, hist: std::collections::BTreeMap<i32, usize>, ours_coarse: usize, editor_coarse: usize,
        own_o: Sum, own_e: Sum, fine: Sum, coarse: Sum, interior: Sum, edge: Sum }
    let mut acc: std::collections::BTreeMap<String, Acc> = Default::default();
    let hdr_o = |x: u32, y: u32, fbi: u8| -> [f64; 3] { let a = i1.get(x, y); [0, 1, 2].map(|c| lightmap::classcmp::texel_hdr(0, a[c], fbi, k1)) };
    let hdr_e = |x: u32, y: u32, fbj: u8| -> [f64; 3] { let b = i2.get(x, y); [0, 1, 2].map(|c| lightmap::classcmp::texel_hdr(0, b[c], fbj, k2)) };
    let lit = |v: [f64; 3]| v[0].max(v[1]).max(v[2]) >= lit_hdr;
    let dry = |v: [f64; 3]| editor_min <= 0.0 || v[0] + v[1] + v[2] >= editor_min;
    // the fine side's box filter onto one coarse texel: (lit-normalised mean, lit weight share, every fine texel lit, every fine texel covered)
    let boxf = |cx: u32, cy: u32, cw: u32, ch: u32, fx0: u32, fy0: u32, fw: u32, fh: u32, val: &dyn Fn(u32, u32) -> [f64; 3], cov: &dyn Fn(u32, u32) -> bool| -> ([f64; 3], f64, bool, bool) {
        let (ux0, ux1) = (cx as f64 * fw as f64 / cw as f64, (cx + 1) as f64 * fw as f64 / cw as f64);
        let (uy0, uy1) = (cy as f64 * fh as f64 / ch as f64, (cy + 1) as f64 * fh as f64 / ch as f64);
        let (mut s, mut wl, mut wt, mut all_lit, mut all_cov) = ([0f64; 3], 0f64, 0f64, true, true);
        let mut fy = uy0.floor() as u32;
        while (fy as f64) < uy1 && fy < fh {
            let wy = (uy1.min(fy as f64 + 1.0) - uy0.max(fy as f64)).max(0.0);
            let mut fx = ux0.floor() as u32;
            while (fx as f64) < ux1 && fx < fw {
                let wx = (ux1.min(fx as f64 + 1.0) - ux0.max(fx as f64)).max(0.0);
                let w = wx * wy;
                if w > 1e-12 {
                    let v = val(fx0 + fx, fy0 + fy);
                    wt += w;
                    if lit(v) { wl += w; for c in 0..3 { s[c] += w * v[c]; } } else { all_lit = false; }
                    if !cov(fx0 + fx, fy0 + fy) { all_cov = false; }
                }
                fx += 1;
            }
            fy += 1;
        }
        if wl > 1e-12 { for c in 0..3 { s[c] /= wl; } }
        (s, if wt > 1e-12 { wl / wt } else { 0.0 }, all_lit, all_cov)
    };
    let mut pair_lines: Vec<String> = vec!["chart_ours\tchart_editor\tclass\tname\tow\toh\tew\teh\tlin_ratio\town_o_sum\town_e_sum\tcoarse_n\tcoarse_o_sum\tcoarse_e_sum".into()];
    let (mut paired, mut no_match, mut refused) = (0usize, 0usize, 0usize);
    for i in 0..m1.count as usize {
        let (obj, sub) = bind(&m1, i);
        let Some(r) = rows.get(&(obj, sub)) else { continue };
        let key = if by_name { format!("{}:{}", r.class, r.name) } else { r.class.clone() };
        if !models.is_empty() && !models.iter().any(|s| key.contains(s.as_str())) { continue; }
        let Some(&j) = theirs_of.get(&(obj, sub)) else { no_match += 1; continue };
        let (px, py, pw, ph) = lightmap::classcmp::chart_own_px(m1.pos[i], m1.size[i]);
        let (qx, qy, qw, qh) = lightmap::classcmp::chart_own_px(m2.pos[j], m2.size[j]);
        if pw == 0 || ph == 0 || qw == 0 || qh == 0 || px + pw > i1.w || py + ph > i1.h || qx + qw > i2.w || qy + qh > i2.h { continue; }
        let (a1, a2) = ((pw * ph) as f64, (qw * qh) as f64);
        if let Some(g) = guard { if a1 / a2 > g || a2 / a1 > g { refused += 1; continue; } }
        paired += 1;
        let (fbi, fbj) = (fb1.get(i).copied().unwrap_or(0), fb2.get(j).copied().unwrap_or(0));
        let lin = (a1 / a2).sqrt();
        let key = if split_ratio { format!("{key} @{:.1}", (lin * 10.0).round() / 10.0) } else { key };
        let e = acc.entry(key.clone()).or_default();
        e.charts += 1; e.tex_o += (pw * ph) as usize; e.tex_e += (qw * qh) as usize; e.lin_sum += lin;
        *e.hist.entry((lin * 10.0).round() as i32).or_default() += 1;
        // own: each side its own rect, its own lit texels
        let (mut so, mut se) = (Sum::default(), Sum::default());
        for y in py..py + ph { for x in px..px + pw { let v = hdr_o(x, y, fbi); if lit(v) { so.add(v, [0.0; 3]); } } }
        for y in qy..qy + qh { for x in qx..qx + qw { let v = hdr_e(x, y, fbj); if lit(v) && dry(v) { se.add([0.0; 3], v); } } }
        e.own_o.n += so.n; e.own_e.n += se.n; for c in 0..3 { e.own_o.o[c] += so.o[c]; e.own_e.e[c] += se.e[c]; }
        // which side is the finer grid
        let ours_fine = a1 > a2 || (a1 == a2 && (pw * ph) as f64 >= a2);
        let mut cs = Sum::default();
        if ours_fine {
            e.editor_coarse += 1;
            // fine: our texels, the editor's nearest
            for y in py..py + ph { for x in px..px + pw {
                let (u, v) = ((x - px) as f64 + 0.5, (y - py) as f64 + 0.5);
                let (ex, ey) = ((qx as f64 + (u / pw as f64) * qw as f64).floor() as u32, (qy as f64 + (v / ph as f64) * qh as f64).floor() as u32);
                let ve = hdr_e(ex.min(qx + qw - 1), ey.min(qy + qh - 1), fbj);
                if lit(ve) && dry(ve) { e.fine.add(hdr_o(x, y, fbi), ve); }
            } }
            // coarse: the editor's grid, ours box-filtered onto it
            for cy in 0..qh { for cx in 0..qw {
                let ve = hdr_e(qx + cx, qy + cy, fbj);
                if !(lit(ve) && dry(ve)) { continue; }
                let (vo, share, all_lit, all_cov) = boxf(cx, cy, qw, qh, px, py, pw, ph, &|x, y| hdr_o(x, y, fbi), &|x, y| covered((y * i1.w + x) as usize));
                if share <= 0.0 { continue; }
                e.coarse.add(vo, ve); cs.add(vo, ve);
                if all_lit && all_cov { e.interior.add(vo, ve); } else { e.edge.add(vo, ve); }
            } }
        } else {
            e.ours_coarse += 1;
            // fine: the editor's texels, ours nearest
            for y in qy..qy + qh { for x in qx..qx + qw {
                let ve = hdr_e(x, y, fbj);
                if !(lit(ve) && dry(ve)) { continue; }
                let (u, v) = ((x - qx) as f64 + 0.5, (y - qy) as f64 + 0.5);
                let (ox, oy) = ((px as f64 + (u / qw as f64) * pw as f64).floor() as u32, (py as f64 + (v / qh as f64) * ph as f64).floor() as u32);
                e.fine.add(hdr_o(ox.min(px + pw - 1), oy.min(py + ph - 1), fbi), ve);
            } }
            // coarse: our grid, the editor box-filtered onto it
            for cy in 0..ph { for cx in 0..pw {
                let (ve, share, all_lit, _) = boxf(cx, cy, pw, ph, qx, qy, qw, qh, &|x, y| hdr_e(x, y, fbj), &|_, _| true);
                if share <= 0.0 || !(lit(ve) && dry(ve)) { continue; }
                let vo = hdr_o(px + cx, py + cy, fbi);
                let cov = covered(((py + cy) * i1.w + px + cx) as usize);
                e.coarse.add(vo, ve); cs.add(vo, ve);
                if all_lit && cov && lit(vo) { e.interior.add(vo, ve); } else { e.edge.add(vo, ve); }
            } }
        }
        if pairs_out.is_some() {
            pair_lines.push(format!("{i}\t{j}\t{}\t{}\t{pw}\t{ph}\t{qw}\t{qh}\t{lin:.3}\t{:.5}/{:.5}/{:.5}\t{:.5}/{:.5}/{:.5}\t{}\t{:.5}/{:.5}/{:.5}\t{:.5}/{:.5}/{:.5}", r.class, r.name,
                so.o[0], so.o[1], so.o[2], se.e[0], se.e[1], se.e[2], cs.n, cs.o[0], cs.o[1], cs.o[2], cs.e[0], cs.e[1], cs.e[2]));
        }
    }
    eprintln!("{paired} pairs by bind word; {no_match} charts without an editor chart of the same bind word; {refused} refused by the guard {guard:?}");
    if let Some(p) = &pairs_out { std::fs::write(p, pair_lines.join("\n") + "\n").unwrap_or_else(|e| panic!("{p}: {e}")); eprintln!("wrote {p}"); }

    // 4. the table
    let rat = |s: &Sum| -> String { if s.n < 16 { return "—".into() } let n = s.n as f64; let r = |c: usize| if s.e[c] / n > 1e-9 { (s.o[c] / n) / (s.e[c] / n) } else { f64::NAN }; format!("{:.3}/{:.3}/{:.3}", r(0), r(1), r(2)) };
    let rat_own = |o: &Sum, e: &Sum| -> String { if o.n < 16 || e.n < 16 { return "—".into() } let r = |c: usize| if e.e[c] / e.n as f64 > 1e-9 { (o.o[c] / o.n as f64) / (e.e[c] / e.n as f64) } else { f64::NAN }; format!("{:.3}/{:.3}/{:.3}", r(0), r(1), r(2)) };
    let mean_e = |s: &Sum| -> String { if s.n == 0 { return "—".into() } let n = s.n as f64; format!("{:.4}/{:.4}/{:.4}", s.e[0] / n, s.e[1] / n, s.e[2] / n) };
    let hist_s = |h: &std::collections::BTreeMap<i32, usize>| -> String { h.iter().map(|(k, n)| format!("{:.1}×{n}", *k as f64 / 10.0)).collect::<Vec<_>>().join(" ") };
    let head = "class\tcharts\ttex_ours\ttex_editor\tlin_ours/editor\thist(lin×n)\tpairs_ours_finer\tpairs_ours_coarser\town_n_ours\town_n_editor\tOWN r/g/b\tfine_n\tFINE r/g/b\tcoarse_n\tCOARSE r/g/b\tinterior_n\tINTERIOR r/g/b\tedge_n\tEDGE r/g/b\teditor_mean_coarse r/g/b";
    let mut lines = vec![head.to_string()];
    println!("{head}");
    // a TOTAL row + the classes by texel count (editor's grid)
    let mut total = Acc::default();
    for e in acc.values() {
        total.charts += e.charts; total.tex_o += e.tex_o; total.tex_e += e.tex_e; total.lin_sum += e.lin_sum; total.ours_coarse += e.ours_coarse; total.editor_coarse += e.editor_coarse;
        for (k, n) in &e.hist { *total.hist.entry(*k).or_default() += n; }
        for (t, s) in [(&mut total.own_o, &e.own_o), (&mut total.own_e, &e.own_e), (&mut total.fine, &e.fine), (&mut total.coarse, &e.coarse), (&mut total.interior, &e.interior), (&mut total.edge, &e.edge)] { t.n += s.n; for c in 0..3 { t.o[c] += s.o[c]; t.e[c] += s.e[c]; } }
    }
    let mut order: Vec<(&String, &Acc)> = acc.iter().collect();
    order.sort_by(|a, b| b.1.coarse.n.cmp(&a.1.coarse.n));
    let mut rows_out: Vec<(String, &Acc)> = vec![("TOTAL".into(), &total)];
    for (k, e) in order { if e.tex_o.max(e.tex_e) >= min_texels { rows_out.push((k.clone(), e)); } }
    for (k, e) in rows_out {
        let line = format!("{k}\t{}\t{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", e.charts, e.tex_o, e.tex_e, e.lin_sum / e.charts.max(1) as f64, hist_s(&e.hist), e.editor_coarse, e.ours_coarse,
            e.own_o.n, e.own_e.n, rat_own(&e.own_o, &e.own_e), e.fine.n, rat(&e.fine), e.coarse.n, rat(&e.coarse), e.interior.n, rat(&e.interior), e.edge.n, rat(&e.edge), mean_e(&e.coarse));
        println!("{line}");
        lines.push(line);
    }
    if let Some(o) = out { std::fs::write(&o, lines.join("\n") + "\n").unwrap_or_else(|e| panic!("{o}: {e}")); eprintln!("wrote {o}"); }
}
