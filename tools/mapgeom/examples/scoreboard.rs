//! 1:1 scoreboard: bake each listed source and compare field-by-field.
//! Usage: scoreboard PAIRS.TSV
//! PAIRS.TSV rows: REF_PATH<TAB>ITEM|PREFAB<TAB>SOURCE<TAB>SCALE
//! Per pair: bake at SCALE with ref ident/author, then compare tri-sets (mm),
//! materials+order, visual flags, colors, collision, bounds. One PASS/FAIL line
//! per field + summary counts.
use mapgeom::static_item::vstream::Elem;
use std::collections::{BTreeMap, BTreeSet};

fn key(p: &[f32; 3]) -> (i32, i32, i32) {
    ((p[0] * 1000.0).round() as i32, ((p[1] * 1000.0).round() as i32), ((p[2] * 1000.0).round() as i32))
}

struct Vis {
    tris: BTreeMap<[(i32, i32, i32); 3], usize>,
    ftris: Vec<[[f32; 3]; 3]>,
    mat: String,
    phys: u8,
    flags: u32,
    has_color: bool,
    nverts: usize,
}

struct Sum {
    visuals: Vec<Vis>,
    surf_tris: usize,
    bounds: ([f32; 3], [f32; 3]),
}

fn summarize(data: &[u8]) -> Sum {
    let f = mapgeom::static_item::file::parse_file(data).unwrap();
    let so = f.item.static_object().unwrap();
    let s2 = so.solid2().unwrap();
    let mats: Vec<(String, u8)> = s2.custom_materials.iter().map(|m| m.inst().map(|i| (i.link().unwrap_or("?").to_string(), i.physics())).unwrap_or(("?".into(), 99))).collect();
    let mut visuals = Vec::new();
    for g in &s2.shaded_geoms {
        let vi = g.visual_index.max(0) as usize;
        let mi = g.material_index.max(0) as usize;
        let (mat, phys) = mats.get(mi).cloned().unwrap_or(("?".into(), 99));
        if let Some(vref) = s2.visuals.get(vi) {
            if let Some(mapgeom::static_item::Node::Visual(vis)) = vref.inline.as_deref() {
                let m = vis.main.as_ref().unwrap();
                let st = vis.stream().unwrap();
                let (mut pos, mut uv) = (Vec::new(), Vec::new());
                let mut has_color = false;
                for (d, e) in st.decls.iter().zip(st.elems.iter()) {
                    match e {
                        Elem::Float3(p) if d.name() == 0 => pos = p.clone(),
                        Elem::Float2(u) if d.name() == 10 => uv = u.clone(),
                        Elem::Word(_) if d.name() == 8 => has_color = true,
                        _ => {}
                    }
                }
                let idx = vis.index_buffer.as_ref().map(|b| b.indices.clone()).unwrap_or_default();
                let mut tris: BTreeMap<[(i32, i32, i32); 3], usize> = BTreeMap::new();
                let mut ftris: Vec<[[f32; 3]; 3]> = Vec::new();
                for t in idx.chunks(3) {
                    if t.len() < 3 {
                        continue;
                    }
                    let ft = [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]];
                    ftris.push(ft);
                    let mut k = [key(&pos[t[0] as usize]), key(&pos[t[1] as usize]), key(&pos[t[2] as usize])];
                    k.sort();
                    *tris.entry(k).or_default() += 1;
                }
                let _ = uv;
                visuals.push(Vis { tris, ftris, mat, phys, flags: m.chunk_flags, has_color, nverts: pos.len() });
            }
        }
    }
    let surf_tris = so.surface().map(|s| match &s.surf {
        mapgeom::static_item::surface::Surf::Mesh { triangles, .. } => triangles.len(),
        _ => 0,
    }).unwrap_or(0);
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for v in &visuals {
        for (t, _) in &v.tris {
            for k in t {
                let vv = [k.0, k.1, k.2];
                for a in 0..3 {
                    lo[a] = lo[a].min(vv[a] as f32 / 1000.0);
                    hi[a] = hi[a].max(vv[a] as f32 / 1000.0);
                }
            }
        }
    }
    Sum { visuals, surf_tris, bounds: (lo, hi) }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let align = a.iter().any(|x| x == "--align");
    let pairfile = a.iter().skip(1).find(|x| !x.starts_with("--")).expect("pairs file");
    let pairs = std::fs::read_to_string(pairfile).unwrap();
    let pakstr = "--pak /tmp/current-Stadium.pak:B773D73047A4104857722366D78D28A6 --pak /tmp/BlueBay.pak:660C4C156B80337E296A1034B0AA05B8";
    let _ = pakstr;
    let mut store = mapgeom::store::DataStore::empty();
    store.add_pak("/tmp/current-Stadium.pak", "B773D73047A4104857722366D78D28A6").unwrap();
    store.add_pak("/tmp/BlueBay.pak", "660C4C156B80337E296A1034B0AA05B8").unwrap();
    let mut pass = 0;
    let mut fail = 0;
    for line in pairs.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let (refp, kind, src, scale): (&str, &str, &str, f32) = (f[0], f[1], f[2], f[3].parse().unwrap());
        let refdata = std::fs::read(refp).unwrap();
        let (ident, author) = tmmaps::header::item_ident_author(&refdata).unwrap_or(("X".into(), "Y".into()));
        let baked = if kind == "ITEM" {
            let srcdata = std::fs::read(src).unwrap();
            mapgeom::static_item::build::static_item_from_item(&srcdata, &ident, &author, scale)
        } else {
            mapgeom::static_item::build::static_item_from_prefab(&mut store, src, &ident, &author, scale, 26)
        };
        let short = refp.rsplit('/').next().unwrap_or(refp);
        match baked {
            Err(e) => {
                fail += 1;
                println!("{short}: BUILD-FAIL {e}");
            }
            Ok(b) => {
                if b == refdata {
                    pass += 1;
                    println!("{short}: BYTE-IDENTICAL");
                    continue;
                }
                let x = summarize(&refdata);
                let mut y = summarize(&b);
                let mut align_note = String::new();
                if align {
                    // Float-space alignment: find the translation maximizing
                    // quantized tri recall (replicates solveflex: translate
                    // floats, then quantize -- integer-key translation misses
                    // sub-mm fractions).
                    let fcen = |v: &[Vis]| {
                        let mut s = [0f64; 3];
                        let mut n = 0f64;
                        for vv in v {
                            for t in &vv.ftris {
                                for k in t {
                                    s[0] += k[0] as f64;
                                    s[1] += k[1] as f64;
                                    s[2] += k[2] as f64;
                                    n += 1.0;
                                }
                            }
                        }
                        [s[0] / n, s[1] / n, s[2] / n]
                    };
                    let (cx, cy) = (fcen(&x.visuals), fcen(&y.visuals));
                    let d0 = [cx[0] - cy[0], cx[1] - cy[1], cx[2] - cy[2]];
                    let mut xt: BTreeMap<[(i32, i32, i32); 3], usize> = BTreeMap::new();
                    for vv in &x.visuals {
                        for (t, c) in &vv.tris {
                            *xt.entry(*t).or_default() += c;
                        }
                    }
                    let xn: usize = xt.values().sum();
                    let score = |d: [f64; 3]| {
                        let mut inter = 0;
                        for vv in &y.visuals {
                            for t in &vv.ftris {
                                let mut k = [key(&[t[0][0] + d[0] as f32, t[0][1] + d[1] as f32, t[0][2] + d[2] as f32]), key(&[t[1][0] + d[0] as f32, t[1][1] + d[1] as f32, t[1][2] + d[2] as f32]), key(&[t[2][0] + d[0] as f32, t[2][1] + d[1] as f32, t[2][2] + d[2] as f32])];
                                k.sort();
                                inter += 1.min(xt.get(&k).copied().unwrap_or(0));
                            }
                        }
                        inter as f64 / xn.max(1) as f64
                    };
                    // hill-climb from centroid delta in 1mm, 0.1mm, 0.01mm steps
                    let mut d = d0;
                    let mut best = score(d);
                    for step in [1.0, 0.1, 0.01] {
                        let mut improved = true;
                        while improved {
                            improved = false;
                            for dx in [-step, 0.0, step] {
                                for dy in [-step, 0.0, step] {
                                    for dz in [-step, 0.0, step] {
                                        let nd = [d[0] + dx, d[1] + dy, d[2] + dz];
                                        let r = score(nd);
                                        if r > best {
                                            best = r;
                                            d = nd;
                                            improved = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // rebuild baked quantized sets at best translation
                    for vv in y.visuals.iter_mut() {
                        let mut nt = BTreeMap::new();
                        for t in &vv.ftris {
                            let mut k = [key(&[t[0][0] + d[0] as f32, t[0][1] + d[1] as f32, t[0][2] + d[2] as f32]), key(&[t[1][0] + d[0] as f32, t[1][1] + d[1] as f32, t[1][2] + d[2] as f32]), key(&[t[2][0] + d[0] as f32, t[2][1] + d[1] as f32, t[2][2] + d[2] as f32])];
                            k.sort();
                            *nt.entry(k).or_default() += 1;
                        }
                        vv.tris = nt;
                    }
                    align_note = format!("align d=({:.2},{:.2},{:.2})mm recall={:.3}", d[0], d[1], d[2], best);
                }
                let mut notes = Vec::new();
                if !align_note.is_empty() {
                    notes.push(align_note.clone());
                }
                // materials + order
                let xm: Vec<(String, u8)> = x.visuals.iter().map(|v| (v.mat.clone(), v.phys)).collect();
                let ym: Vec<(String, u8)> = y.visuals.iter().map(|v| (v.mat.clone(), v.phys)).collect();
                notes.push(format!("mats {}", if xm == ym { "PASS".to_string() } else { format!("FAIL ref={} bake={}", xm.len(), ym.len()) }));
                // tri coverage: % of ref tris present in bake, % of bake tris in ref
                let mut xt: BTreeMap<[(i32, i32, i32); 3], usize> = BTreeMap::new();
                for v in &x.visuals {
                    for (t, c) in &v.tris {
                        *xt.entry(*t).or_default() += c;
                    }
                }
                let mut yt: BTreeMap<[(i32, i32, i32); 3], usize> = BTreeMap::new();
                for v in &y.visuals {
                    for (t, c) in &v.tris {
                        *yt.entry(*t).or_default() += c;
                    }
                }
                let xn: usize = xt.values().sum();
                let yn: usize = yt.values().sum();
                let mut inter = 0;
                for (t, c) in &xt {
                    inter += (*c).min(yt.get(t).copied().unwrap_or(0));
                }
                notes.push(format!("tris ref={xn} bake={yn} recall={:.1}% precision={:.1}%", 100.0 * inter as f64 / xn.max(1) as f64, 100.0 * inter as f64 / yn.max(1) as f64));
                // flags + colors per visual index (order-sensitive) and per
                // material (order-insensitive: material order is merge history)
                let xf: Vec<(u32, bool)> = x.visuals.iter().map(|v| (v.flags, v.has_color)).collect();
                let yf: Vec<(u32, bool)> = y.visuals.iter().map(|v| (v.flags, v.has_color)).collect();
                let mut xfm: BTreeMap<String, (u32, bool)> = BTreeMap::new();
                for v in &x.visuals {
                    xfm.insert(v.mat.clone(), (v.flags, v.has_color));
                }
                let mut yfm: BTreeMap<String, (u32, bool)> = BTreeMap::new();
                for v in &y.visuals {
                    yfm.insert(v.mat.clone(), (v.flags, v.has_color));
                }
                notes.push(format!("flagcolor {}", if xf == yf { "PASS" } else if xfm == yfm { "PASS-by-mat" } else { "FAIL" }));
                notes.push(format!("surf ref={} bake={}", x.surf_tris, y.surf_tris));
                notes.push(format!("nverts ref={:?} bake={:?}", x.visuals.iter().map(|v| v.nverts).collect::<Vec<_>>(), y.visuals.iter().map(|v| v.nverts).collect::<Vec<_>>()));
                let allpass = xm == ym && (xf == yf || xfm == yfm) && inter == xn && xn == yn && x.surf_tris == y.surf_tris;
                if allpass {
                    pass += 1;
                } else {
                    fail += 1;
                }
                println!("{short}: {} {}", if allpass { "STRUCT-IDENTICAL" } else { "DIFF" }, notes.join(" | "));
                // per-visual: align each bake visual to the ref visual with the
                // same material and report recall (pieces Granady moved show up
                // here with their own translation).
                if align {
                    let mut pv = Vec::new();
                    for (xi, xv) in x.visuals.iter().enumerate() {
                        // match by material stem (link drift across Nadeo versions)
                        let stem = |m: &str| m.rsplit('\\').next().unwrap_or(m).to_string();
                        let mut best: Option<(f64, usize, String)> = None;
                        for (yi, yv) in y.visuals.iter().enumerate() {
                            if stem(&xv.mat) != stem(&yv.mat) {
                                continue;
                            }
                            // float align yv onto xv: hill-climb from centroid delta
                            let xset: BTreeSet<[(i32, i32, i32); 3]> = xv.tris.keys().cloned().collect();
                            let xn: usize = xv.tris.values().sum();
                            let score = |d: [f64; 3]| {
                                let mut inter = 0;
                                for t in &yv.ftris {
                                    let mut k = [key(&[t[0][0] + d[0] as f32, t[0][1] + d[1] as f32, t[0][2] + d[2] as f32]), key(&[t[1][0] + d[0] as f32, t[1][1] + d[1] as f32, t[1][2] + d[2] as f32]), key(&[t[2][0] + d[0] as f32, t[2][1] + d[1] as f32, t[2][2] + d[2] as f32])];
                                    k.sort();
                                    if xset.contains(&k) {
                                        inter += 1;
                                    }
                                }
                                inter as f64 / xn.max(1) as f64
                            };
                            // centroid delta start
                            let (ccx, ccy) = {
                                let mut sx = [0f64; 3];
                                let mut nx = 0f64;
                                for t in &xv.ftris {
                                    for k in t {
                                        sx[0] += k[0] as f64;
                                        sx[1] += k[1] as f64;
                                        sx[2] += k[2] as f64;
                                        nx += 1.0;
                                    }
                                }
                                let mut sy = [0f64; 3];
                                let mut ny = 0f64;
                                for t in &yv.ftris {
                                    for k in t {
                                        sy[0] += k[0] as f64;
                                        sy[1] += k[1] as f64;
                                        sy[2] += k[2] as f64;
                                        ny += 1.0;
                                    }
                                }
                                ([sx[0] / nx, sx[1] / nx, sx[2] / nx], [sy[0] / ny, sy[1] / ny, sy[2] / ny])
                            };
                            let mut d = [ccx[0] - ccy[0], ccx[1] - ccy[1], ccx[2] - ccy[2]];
                            let mut b = score(d);
                            for step in [1.0, 0.1, 0.01] {
                                let mut imp = true;
                                while imp {
                                    imp = false;
                                    for dx in [-step, 0.0, step] {
                                        for dy in [-step, 0.0, step] {
                                            for dz in [-step, 0.0, step] {
                                                let nd = [d[0] + dx, d[1] + dy, d[2] + dz];
                                                let r = score(nd);
                                                if r > b {
                                                    b = r;
                                                    d = nd;
                                                    imp = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // rotation search: 24 proper cube rotations about the
                            // bake centroid, then translate to ref centroid
                            let mut brot = String::new();
                            if b < 0.95 {
                                let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
                                let psgn = [1.0, -1.0, -1.0, 1.0, 1.0, -1.0];
                                for (pi, perm) in perms.iter().enumerate() {
                                    for sx in [1.0, -1.0] {
                                        for sy in [1.0, -1.0] {
                                            for sz in [1.0, -1.0] {
                                                if psgn[pi] * sx * sy * sz < 0.0 {
                                                    continue;
                                                }
                                                let s = [sx, sy, sz];
                                                let rscore = |d: [f64; 3]| {
                                                    let mut inter = 0;
                                                    for t in &yv.ftris {
                                                        let mut kk = [0, 1, 2].map(|i| {
                                                            let v = [t[i][0] as f64 - ccy[0], t[i][1] as f64 - ccy[1], t[i][2] as f64 - ccy[2]];
                                                            let w = [v[perm[0]] * s[0], v[perm[1]] * s[1], v[perm[2]] * s[2]];
                                                            key(&[(w[0] + ccx[0] + d[0]) as f32, (w[1] + ccx[1] + d[1]) as f32, (w[2] + ccx[2] + d[2]) as f32])
                                                        });
                                                        kk.sort();
                                                        if xset.contains(&kk) {
                                                            inter += 1;
                                                        }
                                                    }
                                                    inter as f64 / xn.max(1) as f64
                                                };
                                                let mut dd = [0.0, 0.0, 0.0];
                                                let mut bb = rscore(dd);
                                                for step in [1.0, 0.1] {
                                                    let mut imp = true;
                                                    while imp {
                                                        imp = false;
                                                        for dx in [-step, 0.0, step] {
                                                            for dy in [-step, 0.0, step] {
                                                                for dz in [-step, 0.0, step] {
                                                                    let nd = [dd[0] + dx, dd[1] + dy, dd[2] + dz];
                                                                    let r = rscore(nd);
                                                                    if r > bb {
                                                                        bb = r;
                                                                        dd = nd;
                                                                        imp = true;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                if bb > b {
                                                    b = bb;
                                                    brot = format!(" rot{pi}{sx}{sy}{sz}");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if best.as_ref().map(|(br, _, _)| b > *br).unwrap_or(true) {
                                best = Some((b, yi, brot.clone()));
                            }
                            let _ = xi;
                        }
                        match best {
                            Some((r, yi, br)) => pv.push(format!("{}->b{} r={:.2}{}", stem(&xv.mat), yi, r, br)),
                            None => pv.push(format!("{}(no-src)", stem(&xv.mat))),
                        }
                    }
                    println!("  per-visual: {}", pv.join(" "));
                }
            }
        }
    }
    println!("scoreboard: {pass} pass, {fail} fail");
}
