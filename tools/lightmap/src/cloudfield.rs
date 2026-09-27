//! THE CLOUD SPRITE FIELD FROM THE PACKS (port engineer G2, 2026-09-27 — the g23 clouds cell, REPORT-5-G §4-G.15).
//!
//! WHAT THE PACKS HOLD [FILE]: the system is `Clouds\Func\FuncClouds\Tech3.FuncClouds.Gbx` (class 0x09180000, Maniaplanet.pak;
//! referenced by `Techno3\MotionManagerWeathers\DayTime.MotionManagerWeathers.Gbx`). Its chunk 0x09180007 = {ref CloudsMinColor.tga,
//! ref CloudsMaxColor.tga, ref CloudsTech3.Material.Gbx, an inline 0x09183000 node = {chunk 0x09183000: 16 refs to
//! `Clouds\Media\Solid\Cloudy\Cloudy01–17.Solid.Gbx` (no 05); chunk 0x09183001: (16000.0, 16000.0) = THE TILE PERIOD the capture
//! shows}, ref Tech3.FuncCloudsParam.Gbx}. Each Cloudy solid = CPlugTree "Desert" → trees named after the legacy environments
//! (Snow, Rally, Island, Bay, Coast, Stadium, U7), each a CPlugVisualSprite (0x09010000) whose 24-byte sprites are (pos xyz, size,
//! atlas cell u32 0–15, aspect f32) on the material `Clouds\Material\Clouds2`. Per collection × mood: `<Coll>\Media\Moods\<Mood>\
//! SkyCloudsParams.FuncCloudsParam.Gbx` (class 0x09182000: chunk 0x09182001 = {u32, f32 × 4}, chunk 0x09182002 = {u32, u32, f32 × 5,
//! u32, u32}), the 256² `Clouds.tga` (PS-side) and the 1024² DXT5 `SkyClouds.dds` sprite atlas (4 × 4 cells: the VS's v1 = ±0.25 +
//! the cell offset).
//!
//! WHAT THE CAPTURE SHOWS [CAPTURE pwc-day f127448, BlueBay Day]: 177 `GbxClouds3dInst0` instances per peel on an 8 × 8 grid of
//! 16 km tiles (x origins −64000 … 48000, z −61000 … 51000), 1–6 instances per tile, instance y 2 101.6 … 3 000.0, 52 distinct
//! vertex buffers — each a WHOLE tree of one solid (`match_captured`: 177 / 177 instances, 8 113 = 8 113 sprites, 0 culled; the
//! atlas cell of every sprite = its captured v1.zw offset). RE 14's read A (NOTES 17:00Z) is the CPU rule the data confirms: the
//! grid is centred on the world origin, tile (c, r) → solid c (even rows) / c + 8 (odd rows), one instance per tree, the altitude
//! by a distance table — and the tiles TRANSLATE WITH THE WIND every frame, wrapped over the 128 km period (pwc-day's grid sits
//! 3 000 m of z-drift off the origin), during a lightmap compute too. ⇒ the cloud PLACEMENT in the editor's bake is a function of
//! the time since map load, not of the map file: reproducible statistically, never per texel. The field for a bake therefore
//! comes from a CAPTURE (`clouds::world_sprites`) or from this module's generator with the drift as a parameter; this module
//! supplies the solids and the matcher that verifies a field against a captured one.

use mapgeom::node::{Node, Slot};
use mapgeom::store::DataStore;

/// One sprite of a Cloudy solid, in the solid's (visual) space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteDef {
    pub pos: [f32; 3],
    pub size: f32,
    /// The atlas cell 0–15 (the VS's v1.zw offset = (cell % 4, cell / 4) × 0.25 — checked by `match_captured`).
    pub cell: u32,
    pub aspect: f32,
}

#[derive(Clone, Debug, Default)]
pub struct CloudSolid {
    pub path: String,
    /// (tree name, sprites): "Snow", "Rally", "Island", …
    pub trees: Vec<(String, Vec<SpriteDef>)>,
}

#[derive(Clone, Debug, Default)]
pub struct FuncClouds {
    pub path: String,
    pub min_color: String,
    pub max_color: String,
    pub material: String,
    pub solids: Vec<String>,
    /// Chunk 0x09183001: the instance tile period (x, z) in metres.
    pub tile: [f32; 2],
    pub param: String,
}

/// The per-mood parameters, as stored (RE 14 read A: chunk 0x09182001 = the (d_key, y) altitude table with `a_word` = its count;
/// chunk 0x09182002 = {version, use-centre, centre x, centre z, y at d = 0, y past the last key, the wind speed, …}).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FuncCloudsParam {
    pub path: String,
    /// Chunk 0x09182001: u32 then four floats.
    pub a_word: u32,
    pub a: [f32; 4],
    /// Chunk 0x09182002: two u32, five floats, two u32.
    pub b_words: [u32; 2],
    pub b: [f32; 5],
    pub b_tail: [u32; 2],
}

pub const FUNC_CLOUDS_PATH: &str = "Clouds\\Func\\FuncClouds\\Tech3.FuncClouds.Gbx";

fn rd_u32(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| format!("body ends at {} (wanted 4 bytes at {o})", b.len()))
}
fn rd_f32(b: &[u8], o: usize) -> Result<f32, String> {
    rd_u32(b, o).map(f32::from_bits)
}

/// `Tech3.FuncClouds.Gbx`: the references by node index and the tile period, straight off the body (the graph reader has no
/// chunk 0x09180007).
pub fn load_func_clouds(store: &mut DataStore) -> Result<FuncClouds, String> {
    let m = store.load_model(FUNC_CLOUDS_PATH)?;
    let ext = |i: u32| -> Result<String, String> { m.externals.iter().find(|(n, _)| *n == i).map(|(_, p)| p.clone()).ok_or_else(|| format!("{FUNC_CLOUDS_PATH}: node {i} is not an external reference")) };
    let b = &m.body;
    let mut o = 0usize;
    let c = rd_u32(b, o)?;
    if c != 0x09180007 {
        return Err(format!("{FUNC_CLOUDS_PATH}: first chunk 0x{c:08X}, expected 0x09180007"));
    }
    o += 4;
    let min_color = ext(rd_u32(b, o)?)?;
    let max_color = ext(rd_u32(b, o + 4)?)?;
    let material = ext(rd_u32(b, o + 8)?)?;
    let set_node = rd_u32(b, o + 12)?;
    o += 16;
    // the inline set node: class id, then its chunks
    let cls = rd_u32(b, o)?;
    if cls != 0x09183000 {
        return Err(format!("{FUNC_CLOUDS_PATH}: node {set_node} class 0x{cls:08X}, expected 0x09183000"));
    }
    o += 4;
    let mut solids = Vec::new();
    let mut tile = [0f32; 2];
    loop {
        let cid = rd_u32(b, o)?;
        o += 4;
        match cid {
            0xFACADE01 => break,
            0x09183000 => {
                let n = rd_u32(b, o)? as usize;
                o += 4;
                for _ in 0..n {
                    solids.push(ext(rd_u32(b, o)?)?);
                    o += 4;
                }
            }
            0x09183001 => {
                tile = [rd_f32(b, o)?, rd_f32(b, o + 4)?];
                o += 8;
            }
            c => return Err(format!("{FUNC_CLOUDS_PATH}: unknown chunk 0x{c:08X} in the set node at {o}")),
        }
    }
    let param = ext(rd_u32(b, o)?)?;
    Ok(FuncClouds { path: FUNC_CLOUDS_PATH.into(), min_color, max_color, material, solids, tile, param })
}

/// `<Coll>\Media\Moods\<Mood>\SkyCloudsParams.FuncCloudsParam.Gbx` (or any FuncCloudsParam path), the two chunks as stored.
pub fn load_param_at(store: &mut DataStore, path: &str) -> Result<FuncCloudsParam, String> {
    let m = store.load_model(path)?;
    let b = &m.body;
    let mut p = FuncCloudsParam { path: path.to_string(), ..Default::default() };
    let mut o = 0usize;
    loop {
        let cid = rd_u32(b, o)?;
        o += 4;
        match cid {
            0xFACADE01 => break,
            0x09182001 => {
                p.a_word = rd_u32(b, o)?;
                for k in 0..4 { p.a[k] = rd_f32(b, o + 4 + 4 * k)?; }
                o += 20;
            }
            0x09182002 => {
                p.b_words = [rd_u32(b, o)?, rd_u32(b, o + 4)?];
                for k in 0..5 { p.b[k] = rd_f32(b, o + 8 + 4 * k)?; }
                p.b_tail = [rd_u32(b, o + 28)?, rd_u32(b, o + 32)?];
                o += 36;
            }
            c => return Err(format!("{path}: unknown chunk 0x{c:08X} at {o}")),
        }
    }
    Ok(p)
}

pub fn load_param(store: &mut DataStore, collection: &str, mood: &str) -> Result<FuncCloudsParam, String> {
    load_param_at(store, &format!("{collection}\\Media\\Moods\\{mood}\\SkyCloudsParams.FuncCloudsParam.Gbx"))
}

/// A Cloudy solid: its trees' sprite lists. The graph reader keeps a CPlugVisualSprite's 24-byte inline vertices as position +
/// three "normal" floats = (size, the cell's bits, aspect).
pub fn load_solid(store: &mut DataStore, path: &str) -> Result<CloudSolid, String> {
    let m = store.load_model(path)?;
    let g = m.graph()?;
    let root = match &g.root {
        Some(Node::ItemModel(t)) => *t,
        _ => return Err(format!("{path}: the root is not an item model → tree")),
    };
    let mut out = CloudSolid { path: path.to_string(), trees: Vec::new() };
    fn walk(slots: &[Slot], ti: i32, out: &mut CloudSolid, path: &str) -> Result<(), String> {
        let Some(Slot::Node(Node::Tree(t))) = slots.get(ti.max(0) as usize) else { return Ok(()) };
        if t.visual >= 0 {
            let Some(Slot::Node(Node::Visual(v))) = slots.get(t.visual as usize) else { return Err(format!("{path}: tree {} visual {} is not a visual node", t.name, t.visual)) };
            if v.inline_positions.len() != v.inline_normals.len() {
                return Err(format!("{path}: tree {}: {} positions vs {} attribute triples", t.name, v.inline_positions.len(), v.inline_normals.len()));
            }
            let sprites = v.inline_positions.iter().zip(&v.inline_normals).map(|(p, a)| SpriteDef { pos: *p, size: a[0], cell: a[1].to_bits(), aspect: a[2] }).collect();
            out.trees.push((t.name.clone(), sprites));
        }
        for c in &t.children {
            walk(slots, *c, out, path)?;
        }
        Ok(())
    }
    walk(&g.slots, root, &mut out, path)?;
    Ok(out)
}

/// The matcher's view of one captured instance's sprites (from `clouds::parse_vertices`: the four corners share v0 / v2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapturedSprite {
    pub pos: [f32; 3],
    pub size: f32,
    /// The atlas offset (v1.zw).
    pub atlas_off: [f32; 2],
    pub opacity: f32,
    /// v2.y as stored: aspect (≥ 0) or −1/aspect.
    pub aspect_word: f32,
}

pub fn captured_sprites(vb: &[u8]) -> Vec<CapturedSprite> {
    let mut out: Vec<CapturedSprite> = Vec::new();
    for v in crate::clouds::parse_vertices(vb) {
        if out.iter().any(|s| s.pos == [v.v0[0], v.v0[1], v.v0[2]] && s.size == v.v0[3]) { continue; }
        out.push(CapturedSprite { pos: [v.v0[0], v.v0[1], v.v0[2]], size: v.v0[3], atlas_off: [v.v1[2], v.v1[3]], opacity: v.v2[0], aspect_word: v.v2[1] });
    }
    out
}

/// Which (solid, tree) a captured instance's sprites come from: every captured sprite must sit at a sprite of the list (exact
/// position and size); returns the best (solid index, tree index, matched, list length) or None.
pub fn match_captured(solids: &[CloudSolid], sprites: &[CapturedSprite]) -> Option<(usize, usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize, usize)> = None;
    for (si, s) in solids.iter().enumerate() {
        for (ti, (_, list)) in s.trees.iter().enumerate() {
            let matched = sprites.iter().filter(|c| list.iter().any(|d| d.pos == c.pos && d.size == c.size)).count();
            if matched == sprites.len() && !sprites.is_empty() {
                let cand = (si, ti, matched, list.len());
                if best.map(|b| list.len() < b.3).unwrap_or(true) {
                    best = Some(cand);
                }
            }
        }
    }
    best
}

/// `lmtool clouds-field --pak FILE:KEY … [--collection C --mood M] [--match PASSCAP --frame F] [--verbose]`: the FuncClouds
/// system from the packs (the 16 solids' trees: sprite counts, bboxes, cells), the mood's params, and — with a capture — every
/// cloud instance of the frame's first peel camera matched to its (solid, tree) with the kept / total sprite count, the atlas-cell
/// check and the instance table (tile, y, tree, the tree's centroid / bbox centre and its distances from the origin).
pub fn cli(args: &[String]) -> Result<(), String> {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let verbose = args.iter().any(|x| x == "--verbose");
    let mut store = DataStore::empty();
    let paks: Vec<String> = args.iter().enumerate().filter(|(_, x)| *x == "--pak").filter_map(|(i, _)| args.get(i + 1).cloned()).collect();
    if paks.is_empty() {
        return Err("usage: lmtool clouds-field --pak FILE:KEY [--pak …] [--collection C --mood M] [--match PASSCAP --frame F] [--verbose]".into());
    }
    for p in &paks {
        let (pp, key) = p.rsplit_once(':').ok_or("--pak FILE:KEY")?;
        store.add_pak(pp, key)?;
    }
    let fc = load_func_clouds(&mut store)?;
    println!("{}: min {} max {} material {} tile {:?} param {}; {} solids", fc.path, fc.min_color, fc.max_color, fc.material, fc.tile, fc.param, fc.solids.len());
    let mut solids = Vec::new();
    for p in &fc.solids {
        let s = load_solid(&mut store, p)?;
        let desc: Vec<String> = s.trees.iter().map(|(n, l)| {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            let (mut smin, mut smax) = (f32::MAX, f32::MIN);
            let mut cells = [0usize; 16];
            for d in l { for k in 0..3 { lo[k] = lo[k].min(d.pos[k]); hi[k] = hi[k].max(d.pos[k]); } smin = smin.min(d.size); smax = smax.max(d.size); if (d.cell as usize) < 16 { cells[d.cell as usize] += 1; } }
            format!("{n}: {} sprites, x {:.0}…{:.0} y {:.0}…{:.0} z {:.0}…{:.0}, size {:.0}…{:.0}, cells {:?}", l.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], smin, smax, cells)
        }).collect();
        println!("  {}: {}", p.rsplit('\\').next().unwrap_or(p), desc.join(" | "));
        solids.push(s);
    }
    match load_param_at(&mut store, &fc.param) {
        Ok(p) => println!("  default params {}: a {} {:?} b {:?} {:?} {:?}", p.path, p.a_word, p.a, p.b_words, p.b, p.b_tail),
        Err(e) => println!("  default params: {e}"),
    }
    if let (Some(c), Some(m)) = (f("--collection"), f("--mood")) {
        let p = load_param(&mut store, &c, &m)?;
        println!("  {c} {m} params: a {} {:?} b {:?} {:?} {:?}", p.a_word, p.a, p.b_words, p.b, p.b_tail);
    }
    if let Some(root) = f("--match") {
        let root = std::path::PathBuf::from(root);
        let frame: u32 = f("--frame").map(|v| v.parse().map_err(|e| format!("--frame: {e}"))).transpose()?.unwrap_or(127448);
        let draws = crate::lmaccum::load_draws(&root, frame)?;
        let env = root.join(format!("env/frame{frame}"));
        let mesh: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(env.join("mesh.json")).map_err(|e| format!("mesh.json: {e}"))?).map_err(|e| format!("mesh.json: {e}"))?;
        // the first peel camera's draws only (the frame holds every peel's; the same instances repeat per camera)
        let mut first_eye: Option<serde_json::Value> = None;
        let (mut n, mut n_matched, mut n_sprites, mut n_kept) = (0usize, 0usize, 0usize, 0usize);
        let mut cell_ok = 0usize;
        let mut cell_bad = 0usize;
        let mut per_solid = vec![0usize; solids.len()];
        let mut rows: Vec<(i64, i64, f32, String)> = Vec::new();
        for d in draws.iter().filter(|d| d["Vertex"]["shader"].as_str() == Some("14514")) {
            let eye = d["Vertex"]["cbuffers"]["SceneV"]["GbxV_EyeInWorld"].clone();
            match &first_eye { None => first_eye = Some(eye.clone()), Some(e) if *e != eye => continue, _ => {} }
            let eid = d["eid"].as_u64().unwrap_or(0);
            let Some(rec) = mesh.as_array().and_then(|a| a.iter().find(|r| r["eid"].as_u64() == Some(eid))) else { continue };
            let Some(vb_file) = rec["vertex_buffers"].as_array().and_then(|a| a.first()).and_then(|v| v["file"].as_str()) else { continue };
            let vb = std::fs::read(env.join("mesh").join(vb_file)).map_err(|e| format!("{vb_file}: {e}"))?;
            let sprites = captured_sprites(&vb);
            let t = &d["Vertex"]["cbuffers"]["DrawV"]["GbxClouds3dInst0"]["VisualToWorld"][3];
            let tr = [t[0].as_f64().unwrap_or(0.0) as f32, t[1].as_f64().unwrap_or(0.0) as f32, t[2].as_f64().unwrap_or(0.0) as f32];
            n += 1;
            n_sprites += sprites.len();
            let mm = match_captured(&solids, &sprites);
            let label = match mm {
                Some((si, ti, matched, total)) => {
                    n_matched += 1;
                    n_kept += matched;
                    per_solid[si] += 1;
                    // the atlas cell: the solid's cell vs the captured offset (cell % 4, cell / 4) × 0.25
                    let list = &solids[si].trees[ti].1;
                    for c in &sprites {
                        if let Some(dd) = list.iter().find(|dd| dd.pos == c.pos && dd.size == c.size) {
                            let want = [(dd.cell % 4) as f32 * 0.25, (dd.cell / 4) as f32 * 0.25];
                            if (c.atlas_off[0] - want[0]).abs() < 1e-3 && (c.atlas_off[1] - want[1]).abs() < 1e-3 { cell_ok += 1; } else { cell_bad += 1; if verbose { println!("    eid {eid}: cell {} → offset {:?} vs captured {:?}", dd.cell, want, c.atlas_off); } }
                        }
                    }
                    // the tree's centroid and bbox centre in the solid's space: the per-instance point the CPU rule measures its distance from
                    let (mut c, mut lo, mut hi) = ([0f32; 3], [f32::MAX; 3], [f32::MIN; 3]);
                    for d in list { for k in 0..3 { c[k] += d.pos[k] / list.len() as f32; lo[k] = lo[k].min(d.pos[k]); hi[k] = hi[k].max(d.pos[k]); } }
                    let bc = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
                    let d_c = ((tr[0] + c[0]).powi(2) + (tr[2] + c[2]).powi(2)).sqrt();
                    let d_b = ((tr[0] + bc[0]).powi(2) + (tr[2] + bc[2]).powi(2)).sqrt();
                    let d_t = (tr[0].powi(2) + tr[2].powi(2)).sqrt();
                    format!("{}/{} {}/{}  centroid ({:.0}, {:.0}, {:.0}) bbox centre ({:.0}, {:.0}, {:.0})  d_tile {:.0} d_centroid {:.0} d_bbox {:.0}", solids[si].path.rsplit('\\').next().unwrap_or(""), solids[si].trees[ti].0, matched, total, c[0], c[1], c[2], bc[0], bc[1], bc[2], d_t, d_c, d_b)
                }
                None => "NO MATCH".to_string(),
            };
            if verbose { println!("  eid {eid}: T ({:.0}, {:.1}, {:.0}) {} sprites → {label}", tr[0], tr[1], tr[2], sprites.len()); }
            rows.push((tr[0] as i64, tr[2] as i64, tr[1], label));
        }
        println!("capture {}: frame {frame}, first camera eye {:?}: {n} instances, {n_matched} matched to a (solid, tree) with every sprite ({n_kept} of their lists' sprites kept, {n_sprites} captured); atlas cells {cell_ok} agree / {cell_bad} disagree; instances per solid {:?}", root.display(), first_eye, per_solid);
        rows.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.partial_cmp(&b.2).unwrap()));
        let mut last = (i64::MIN, i64::MIN);
        for (x, z, y, l) in &rows {
            if (*x, *z) != last { println!("  tile ({x}, {z}):"); last = (*x, *z); }
            println!("    y {y:.1}  {l}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The packs at /tmp/paks (the box recipe): the system file, its 16 solids, WhiteShore Day's params.
    #[test]
    fn the_func_clouds_system_reads_from_maniaplanet_pak() {
        if !std::path::Path::new("/tmp/paks/Maniaplanet.pak").exists() { eprintln!("no /tmp/paks — skipped"); return; }
        let mut st = DataStore::empty();
        st.add_pak("/tmp/paks/Maniaplanet.pak", "9A93723447347A8CE336CCFC49E65449").unwrap();
        let fc = load_func_clouds(&mut st).unwrap();
        assert_eq!(fc.tile, [16000.0, 16000.0]);
        assert_eq!(fc.solids.len(), 16);
        assert!(fc.solids[0].ends_with("Cloudy01.Solid.Gbx"), "{}", fc.solids[0]);
        assert!(fc.param.ends_with("Tech3.FuncCloudsParam.Gbx"), "{}", fc.param);
        let s = load_solid(&mut st, &fc.solids[0]).unwrap();
        assert_eq!(s.trees.len(), 2, "{:?}", s.trees.iter().map(|t| &t.0).collect::<Vec<_>>());
        assert!(s.trees.iter().all(|(_, l)| !l.is_empty() && l.iter().all(|d| d.cell < 16 && d.size > 0.0 && d.aspect > 0.0)));
        let p = load_param_at(&mut st, &fc.param).unwrap();
        assert_eq!((p.a_word, p.a), (2, [5000.0, 3000.0, 15000.0, 1000.0]));
        assert_eq!((p.b_words, p.b, p.b_tail), ([0, 1], [512.0, 512.0, 3000.0, 1500.0, 65.0], [0, 0]));
        if std::path::Path::new("/tmp/paks/WhiteShore.pak").exists() {
            st.add_pak("/tmp/paks/WhiteShore.pak", "660C4C156B80337E296A1034B0AA05B8").unwrap();
            let p = load_param(&mut st, "WhiteShore", "Day").unwrap();
            assert_eq!((p.a, p.b), ([1500.0, 5000.0, 15000.0, 2500.0], [1024.0, 1024.0, 5000.0, 2000.0, 70.0]));
        }
    }
}
