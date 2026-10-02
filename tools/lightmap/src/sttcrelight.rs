//! `lmtool sttc-relight` — Hugo's "Straight to the Center" maps keep the SOURCE'S editor lightmap.
//!
//! The chart table binds every chart to an OBJECT INDEX of the game's load-time
//! list `[decoration P][authored blocks][generated: terrain tiles, then the
//! engine's free clips][items]` (`docs/formats/map-lightmap.md` §2, §3.6; on a
//! Nadeo source the file's baked list IS the generated list, 25/25). After
//! `mapgeom sttc` removed checkpoint items / blocks, swapped checkpoint blocks
//! for their plain twins and moved the finish, that list changed:
//!
//!   * removed objects → every later index shifts down;
//!   * the twins generate OTHER clips than the checkpoints did (a
//!     `PlatformTechBase` has no Racing-zone clips): the engine's clip list
//!     loses the checkpoints' clips and gains the twins', at the twins' places
//!     in the derivation order;
//!   * the moved finish and its clips keep their indices but their baked
//!     shadows belong to the old place.
//!
//! So: every chart of a KEPT, UNCHANGED object is renumbered to the object's
//! new index; the charts of changed objects (renamed twins, moved finish pieces
//! and their clips, the twins' new clips which never had one) are dropped —
//! those pieces get the game's per-piece load-time lighting while everything
//! else keeps Nadeo's bake; removed objects' charts go. The cache words are
//! untouched (no solid changed: `TimeWriteMostRecentSolid` stays valid).
//!
//! THE DERIVATION ORDER of the clips is the lightmapper's record order
//! (`itemrule::clip_owner_order`: the cell map's slot walk over the owner
//! blocks, then unit, face, position in the face's clip list) — `--check`
//! measures it against the file order of a source's own baked list, which is
//! how the model is validated (every Fall 2026 source: see the run log).

use std::collections::{HashMap, HashSet};
use std::path::Path;

/// One clip the engine derives, identified by its owner.
#[derive(Clone, Debug)]
pub struct Derived {
    pub cell: [u8; 3],
    pub face: usize,
    pub name: String,
    pub owner_index: usize,
    pub unit: usize,
}

/// The engine's drawn free clips of `m`, in the model's derivation order.
pub fn derived_clips(store: &mut mapgeom::store::DataStore, m: &tmmaps::map::MapFile, collection: &str) -> (Vec<Derived>, mapgeom::fillers::Faces) {
    let mut idx = mapgeom::blockmap::BlockInfoIndex::build(store, collection);
    let faces = mapgeom::fillers::faces(store, &mut idx, m);
    let dirs: HashMap<usize, u8> = m.blocks.iter().map(|b| (b.index, b.dir)).collect();
    let grounds = mapgeom::bake::record_grounds(&faces, m);
    let clips = mapgeom::bake::simulate(&faces, &dirs, &grounds);
    let cells_b: Vec<[i32; 3]> = m.blocks.iter().map(|b| { let (x, y, z) = b.coords(); [x, y, z] }).collect();
    let frees: Vec<bool> = m.blocks.iter().map(|b| b.flags & 0x1000_0000 != 0).collect();
    let owner_order = crate::itemrule::clip_owner_order(&cells_b, &frees);
    let owner_rank: HashMap<usize, usize> = owner_order.iter().enumerate().map(|(r, &bi)| (m.blocks[bi].index, r)).collect();
    let pos_in_list: Vec<usize> = clips.iter().map(|c| faces.occupants.get(&c.cell).and_then(|os| os.iter().find(|o| o.index == c.owner_index && o.unit == c.unit)).and_then(|o| o.faces[c.face].iter().position(|n| *n == c.name)).unwrap_or(0)).collect();
    let mut order: Vec<usize> = (0..clips.len()).filter(|&i| clips[i].drawn()).collect();
    order.sort_by_key(|&i| (owner_rank.get(&clips[i].owner_index).copied().unwrap_or(usize::MAX), clips[i].unit, clips[i].face, pos_in_list[i]));
    let out = order.iter().map(|&i| Derived { cell: clips[i].cell, face: clips[i].face, name: clips[i].name.clone(), owner_index: clips[i].owner_index, unit: clips[i].unit }).collect();
    (out, faces)
}

/// The drawn derived clip each GRID clip record of the file stands for (None: a
/// stale record, a terrain tile, a free record).
pub fn match_records(m: &tmmaps::map::MapFile, faces: &mapgeom::fillers::Faces, derived: &[Derived]) -> Vec<Option<usize>> {
    let mut index: HashMap<([u8; 3], usize, String), Vec<usize>> = HashMap::new();
    for (i, c) in derived.iter().enumerate() {
        index.entry((c.cell, c.face, c.name.clone())).or_default().push(i);
    }
    let mut taken: HashSet<usize> = HashSet::new();
    m.baked
        .iter()
        .map(|b| {
            if b.flags & mapgeom::blockmap::FLAG_FREE != 0 {
                return None;
            }
            let me = faces.stem_of(&b.name);
            let id = faces.clips.get(&me)?;
            let (cell, face) = mapgeom::bake::record_slot(b, id.ty)?;
            let v = index.get(&(cell, face, me))?;
            let i = *v.iter().find(|i| !taken.contains(i))?;
            taken.insert(i);
            Some(i)
        })
        .collect()
}

/// Which generated records are terrain tiles (not clip block infos, not free).
pub fn is_tile(faces: &mapgeom::fillers::Faces, b: &tmmaps::map::BlockRec) -> bool {
    b.flags & mapgeom::blockmap::FLAG_FREE == 0 && !faces.clips.contains_key(&faces.stem_of(&b.name))
}

fn collection_name(m: &tmmaps::map::MapFile) -> String {
    mapgeom::sttc::collection_name(m.body_collections().map(|c| c[0].1).unwrap_or(26)).to_string()
}

/// `--check`: how well the derivation-order model reproduces the file's own baked list.
pub fn check_order(store: &mut mapgeom::store::DataStore, path: &str) -> Result<String, String> {
    let m = tmmaps::map::MapFile::try_load(Path::new(path))?;
    let coll = collection_name(&m);
    let (derived, faces) = derived_clips(store, &m, &coll);
    let matched = match_records(&m, &faces, &derived);
    // the file's grid clip records in order → their derived indices; the model says they should be increasing
    let seq: Vec<usize> = matched.iter().flatten().copied().collect();
    let n_clip_recs = m.baked.iter().filter(|b| b.flags & mapgeom::blockmap::FLAG_FREE == 0 && faces.clips.contains_key(&faces.stem_of(&b.name))).count();
    let n_tiles = m.baked.iter().filter(|b| is_tile(&faces, b)).count();
    let n_free = m.baked.iter().filter(|b| b.flags & mapgeom::blockmap::FLAG_FREE != 0).count();
    let stale = n_clip_recs - seq.len();
    let missing = derived.len() - seq.len();
    let inversions = seq.windows(2).filter(|w| w[1] < w[0]).count();
    // exact positional agreement: file clip record r (r-th matched) == derived index r
    let exact = seq.iter().enumerate().filter(|(r, &d)| *r == d).count();
    // is the file list [tiles][clips] with the free records where?
    let kinds: Vec<char> = m.baked.iter().map(|b| if b.flags & mapgeom::blockmap::FLAG_FREE != 0 { 'F' } else if is_tile(&faces, b) { 'T' } else { 'C' }).collect();
    let mut runs = String::new();
    let mut prev = ' ';
    let mut n = 0;
    for k in kinds.iter().chain(std::iter::once(&' ')) {
        if *k != prev {
            if n > 0 {
                runs.push_str(&format!("{prev}{n} "));
            }
            prev = *k;
            n = 0;
        }
        n += 1;
    }
    Ok(format!(
        "{path}: {} baked = {n_tiles} tiles + {n_clip_recs} grid clip records + {n_free} free; derived drawn {}; matched {} ({stale} stale, {missing} missing); order: {inversions} inversions, {exact}/{} exact positions; layout {}",
        m.baked.len(), derived.len(), seq.len(), seq.len(), runs.trim_end()
    ))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Kept,
    Changed,
    Removed,
}

pub struct ObjMap {
    /// per class: source index → (dst index, state)
    pub block: Vec<(Option<usize>, State)>,
    pub baked: Vec<(Option<usize>, State)>,
    pub item: Vec<(Option<usize>, State)>,
}

pub fn read_objmap(path: &str) -> Result<ObjMap, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut om = ObjMap { block: Vec::new(), baked: Vec::new(), item: Vec::new() };
    for (ln, line) in text.lines().enumerate().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            continue;
        }
        let src: usize = f[1].parse().map_err(|_| format!("{path}:{}: bad src", ln + 1))?;
        let dst: Option<usize> = if f[2] == "-" { None } else { Some(f[2].parse().map_err(|_| format!("{path}:{}: bad dst", ln + 1))?) };
        let state = match f[3] {
            "kept" => State::Kept,
            "changed" => State::Changed,
            "removed" => State::Removed,
            s => return Err(format!("{path}:{}: state `{s}`", ln + 1)),
        };
        let v = match f[0] {
            "block" => &mut om.block,
            "baked" => &mut om.baked,
            "item" => &mut om.item,
            s => return Err(format!("{path}:{}: class `{s}`", ln + 1)),
        };
        if v.len() != src {
            return Err(format!("{path}:{}: {} rows must be dense from 0 (got src {src} at row {})", ln + 1, f[0], v.len()));
        }
        v.push((dst, state));
    }
    Ok(om)
}

pub struct Relit {
    pub charts_in: usize,
    pub kept: usize,
    pub dropped_changed: usize,
    pub dropped_removed: usize,
    pub dropped_unmatched: usize,
    pub p: u32,
    pub base_items_src: u32,
    pub base_items_dst: u32,
    pub n_clips_src: usize,
    pub n_clips_dst: usize,
    pub notes: Vec<String>,
}

/// Renumber the source's lightmap for the output map and write `out`.
///
/// THE MODEL IS POSITIONAL: on every Fall 2026 source the charted objects end
/// exactly at P + blocks + baked + items of the FILE (25/25, `lmtool binds`),
/// and the file's clip order is neither the lightmapper's cell-map walk nor any
/// walk of the clips' or owners' cells (`--check`, `--cellwalk`: hundreds of
/// inversions on every map) — so the game numbers the generated blocks the way
/// the FILE lists them, and `mapgeom sttc`'s reconcile pass makes the output's
/// file list hold exactly the clips the engine derives (as the source's does).
/// Every chart then follows its object's new index from the objmap; changed
/// objects (renamed twins, moved finish pieces) lose theirs; removed objects'
/// charts go.
pub fn relight(src_path: &str, dst_path: &str, objmap: &ObjMap, out: &str) -> Result<Relit, String> {
    let src = tmmaps::map::MapFile::try_load(Path::new(src_path))?;
    let dst = tmmaps::map::MapFile::try_load(Path::new(dst_path))?;
    if objmap.block.len() != src.blocks.len() || objmap.baked.len() != src.baked.len() || objmap.item.len() != src.items.len() {
        return Err(format!("objmap sizes {}/{}/{} vs the source's {}/{}/{}", objmap.block.len(), objmap.baked.len(), objmap.item.len(), src.blocks.len(), src.baked.len(), src.items.len()));
    }
    let n_dst = |v: &[(Option<usize>, State)]| v.iter().filter(|(d, _)| d.is_some()).count();
    // the output may carry MORE baked records than the objmap maps (the reconcile pass appends the twins' clips: chartless)
    if n_dst(&objmap.block) != dst.blocks.len() || n_dst(&objmap.baked) > dst.baked.len() || n_dst(&objmap.item) != dst.items.len() {
        return Err(format!("objmap kept counts {}/{}/{} vs the output's {}/{}/{}", n_dst(&objmap.block), n_dst(&objmap.baked), n_dst(&objmap.item), dst.blocks.len(), dst.baked.len(), dst.items.len()));
    }
    for (class, v) in [("block", &objmap.block), ("baked", &objmap.baked), ("item", &objmap.item)] {
        let mut dsts: Vec<usize> = v.iter().filter_map(|(d, _)| *d).collect();
        dsts.sort_unstable();
        if dsts.windows(2).any(|w| w[0] == w[1]) {
            return Err(format!("objmap {class}: a destination index is named twice"));
        }
    }
    let lm_src = crate::mapio::load(src_path)?;
    let d = lm_src.chunk.data.as_ref().ok_or_else(|| format!("{src_path}: no lightmap data (HasLightmaps 0)"))?;
    let mp0 = d.cache.mapping().ok_or("mapping")?;
    let max_obj = mp0.binds.iter().map(|b| b.obj_group_idx / 4).max().unwrap_or(0);
    let (nu, nb, ni) = (src.blocks.len() as u32, src.baked.len() as u32, src.items.len() as u32);
    let p = (max_obj + 1).checked_sub(nu + nb + ni).ok_or_else(|| format!("{src_path}: max charted object {max_obj} < blocks + baked + items {}", nu + nb + ni))?;
    let mut notes = Vec::new();
    if p != 0 && p != 16384 {
        return Err(format!("{src_path}: the charted objects end at {} = P {p} + {nu} blocks + {nb} baked + {ni} items; P must be 0 (terrain) or 16384 (Stadium decoration) for the positional model", max_obj + 1));
    }
    let (nu2, nb2) = (dst.blocks.len() as u32, dst.baked.len() as u32);
    let baked_base = p + nu2;
    let items_base = baked_base + nb2;
    let mut lm = crate::mapio::load(src_path)?;
    let dd = lm.chunk.data.as_mut().expect("has lightmaps");
    let mp = dd.cache.mapping_mut().expect("mapping");
    let charts_in = mp.binds.len();
    let mut keep = vec![true; charts_in];
    let (mut kept, mut dropped_changed, mut dropped_removed, mut dropped_unmatched) = (0usize, 0usize, 0usize, 0usize);
    let (mut kept_blocks, mut kept_baked, mut kept_items) = (0usize, 0usize, 0usize);
    for (ci, b) in mp.binds.iter_mut().enumerate() {
        let obj = b.obj_group_idx / 4;
        let sub = b.obj_group_idx % 4;
        let new: Option<u32> = if obj < p {
            Some(obj)
        } else if obj < p + nu {
            match objmap.block[(obj - p) as usize] {
                (Some(d), State::Kept) => { kept_blocks += 1; Some(p + d as u32) }
                (_, State::Changed) => { dropped_changed += 1; None }
                _ => { dropped_removed += 1; None }
            }
        } else if obj < p + nu + nb {
            match objmap.baked[(obj - p - nu) as usize] {
                (Some(d), State::Kept) => { kept_baked += 1; Some(baked_base + d as u32) }
                (_, State::Changed) => { dropped_changed += 1; None }
                _ => { dropped_removed += 1; None }
            }
        } else if obj < p + nu + nb + ni {
            match objmap.item[(obj - p - nu - nb) as usize] {
                (Some(d), State::Kept) => { kept_items += 1; Some(items_base + d as u32) }
                (_, State::Changed) => { dropped_changed += 1; None }
                _ => { dropped_removed += 1; None }
            }
        } else {
            dropped_unmatched += 1;
            None
        };
        match new {
            Some(n) => { b.obj_group_idx = n * 4 + sub; kept += 1; }
            None => keep[ci] = false,
        }
    }
    if keep.iter().any(|k| !k) {
        mp.retain_charts(&keep);
    }
    mp.sort_by_object();
    for z in mp.raw_z.iter_mut() {
        *z = None;
    }
    let payload = lm.chunk.write(true);
    let into = crate::mapio::load(dst_path).map_err(|e| format!("the output map must carry the source's lightmap chunk (keep): {e}"))?;
    crate::mapio::save_with_chunk(&into, &payload, out)?;
    notes.push(format!("charts kept by class: {kept_blocks} on blocks, {kept_baked} on generated records, {kept_items} on items; the output's objects: P {p} + {nu2} blocks + {nb2} generated + {} items", dst.items.len()));
    Ok(Relit { charts_in, kept, dropped_changed, dropped_removed, dropped_unmatched, p, base_items_src: p + nu + nb, base_items_dst: items_base, n_clips_src: nb as usize, n_clips_dst: nb2 as usize, notes })
}

pub fn cli(a: &[String]) -> Result<(), String> {
    // lmtool sttc-relight --pak F:KEY… (--check MAP… | --source S --map O --objmap M --out LIT)
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 0;
    let mut paks = 0;
    while i < a.len() {
        if a[i] == "--pak" {
            let spec = a.get(i + 1).ok_or("--pak F:KEY")?;
            let (path, key) = spec.rsplit_once(':').ok_or("--pak F:KEY")?;
            store.add_pak(path, key)?;
            paks += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    if paks == 0 {
        return Err("sttc-relight needs --pak FILE:KEY (the collection's block infos)".into());
    }
    if let Some(pos) = a.iter().position(|x| x == "--owners") {
        let limit: usize = f("--limit").and_then(|s| s.parse().ok()).unwrap_or(60);
        for p in a[pos + 1..].iter().take_while(|x| !x.starts_with("--")) {
            print!("{}", owners_report(&mut store, p, limit)?);
        }
        return Ok(());
    }
    if let Some(pos) = a.iter().position(|x| x == "--cellwalk") {
        for p in a[pos + 1..].iter().take_while(|x| !x.starts_with("--")) {
            print!("{}", cellwalk_report(&mut store, p)?);
        }
        return Ok(());
    }
    if let Some(pos) = a.iter().position(|x| x == "--check") {
        for p in a[pos + 1..].iter().take_while(|x| !x.starts_with("--")) {
            println!("{}", check_order(&mut store, p)?);
        }
        return Ok(());
    }
    let (src, dst, om, out) = (f("--source").ok_or("--source SRC.Map.Gbx")?, f("--map").ok_or("--map OUT.Map.Gbx")?, f("--objmap").ok_or("--objmap FILE.tsv")?, f("--out").ok_or("--out LIT.Map.Gbx")?);
    let objmap = read_objmap(&om)?;
    let r = relight(&src, &dst, &objmap, &out)?;
    println!(
        "wrote {out}: {} charts in, {} kept (renumbered), {} dropped (changed objects), {} dropped (removed objects), {} dropped (unmatched); P {}; item base {} -> {}; generated records {} -> {}",
        r.charts_in, r.kept, r.dropped_changed, r.dropped_removed, r.dropped_unmatched, r.p, r.base_items_src, r.base_items_dst, r.n_clips_src, r.n_clips_dst
    );
    for n in &r.notes {
        println!("  note: {n}");
    }
    Ok(())
}

/// `--owners MAP`: the file's grid clip records in order with their derived owner (block index, cell), for the order study.
pub fn owners_report(store: &mut mapgeom::store::DataStore, path: &str, limit: usize) -> Result<String, String> {
    let m = tmmaps::map::MapFile::try_load(Path::new(path))?;
    let coll = collection_name(&m);
    let (derived, faces) = derived_clips(store, &m, &coll);
    let matched = match_records(&m, &faces, &derived);
    let mut out = String::new();
    let mut runs = 0usize;
    let mut owners_seen: HashSet<usize> = HashSet::new();
    let mut prev_owner = usize::MAX;
    let mut n = 0usize;
    for (j, b) in m.baked.iter().enumerate() {
        let Some(g) = matched[j] else { continue };
        let c = &derived[g];
        let ob = &m.blocks[c.owner_index];
        if c.owner_index != prev_owner {
            runs += 1;
            prev_owner = c.owner_index;
        }
        owners_seen.insert(c.owner_index);
        if n < limit {
            out.push_str(&format!("b{j}\t{}\towner #{} {} cell {:?} dir {}\tunit {} face {}\n", b.name, c.owner_index, ob.name, ob.coords(), ob.dir, c.unit, c.face));
        }
        n += 1;
    }
    out.push_str(&format!("{path}: {n} matched clip records, {} distinct owners in {runs} runs (contiguous per owner = {})\n", owners_seen.len(), runs == owners_seen.len()));
    Ok(out)
}

/// Order study: the file's clip records vs a CellMap walk keyed by the CLIP's cell, clips inserted in block file order
/// (unit, face, list) — `--cellwalk MAP…`. Prints inversions and exact positions for a few key variants.
pub fn cellwalk_report(store: &mut mapgeom::store::DataStore, path: &str) -> Result<String, String> {
    let m = tmmaps::map::MapFile::try_load(Path::new(path))?;
    let coll = collection_name(&m);
    // derived clips in BLOCK FILE ORDER (not the owner-rank order): re-sort
    let (mut derived, faces) = derived_clips(store, &m, &coll);
    derived.sort_by_key(|c| (c.owner_index, c.unit, c.face));
    let matched = match_records(&m, &faces, &derived);
    let file_seq: Vec<usize> = matched.iter().flatten().copied().collect();
    let mut out = String::new();
    for (label, keyf) in [
        ("clip cell", Box::new(|c: &Derived| [c.cell[0] as i32, c.cell[1] as i32, c.cell[2] as i32]) as Box<dyn Fn(&Derived) -> [i32; 3]>),
        ("owner cell", Box::new(|c: &Derived| { let b = &m.blocks[c.owner_index]; let (x, y, z) = b.coords(); [x, y, z] })),
        ("clip cell x,z,y", Box::new(|c: &Derived| [c.cell[0] as i32, c.cell[2] as i32, c.cell[1] as i32])),
    ] {
        let mut cm: crate::itemrule::CellMap<usize> = crate::itemrule::CellMap::default();
        let mut per_cell: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
        for (g, c) in derived.iter().enumerate() {
            let k = keyf(c);
            per_cell.entry(k).or_default().push(g);
            cm.insert(k, g);
        }
        let mut pred: Vec<usize> = Vec::new();
        for (k, _) in cm.walk() {
            pred.extend(per_cell[&k].iter().copied());
        }
        let rank: HashMap<usize, usize> = pred.iter().enumerate().map(|(r, &g)| (g, r)).collect();
        let ranks: Vec<usize> = file_seq.iter().map(|g| rank[g]).collect();
        let inv = ranks.windows(2).filter(|w| w[1] < w[0]).count();
        let exact = file_seq.iter().enumerate().filter(|(r, &g)| pred.get(*r) == Some(&g)).count();
        out.push_str(&format!("{path}: key {label}: {inv} inversions, {exact}/{} exact\n", file_seq.len()));
    }
    Ok(out)
}
