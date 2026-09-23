//! `mapgeom item-set`: the block browser as a set of half-scale ITEMS a
//! mapper can build with — every block of the editor's block tree
//! (`catalog::block_browser`) baked through the tiny library's static-item
//! path (`tiny_library::bake_block`, the same bake the tiny campaign ships)
//! and written as loose `.Item.Gbx` files under `Items/<set>/<the block
//! browser's folders>/<Block>.Item.Gbx`, with the block's own icon, its name,
//! and editor settings that tile like blocks:
//!
//! * grid snap = half a scaled block unit horizontally (8 m at scale 0.5) and
//!   half a unit vertically (2 m), fly step the same — Nadeo's own tileable
//!   items (the `InflatableMat_H4V1` 8 m mats: grid 4 m, pivot at the mat's
//!   centre) are the precedent: any footprint made of whole units lands on
//!   the grid in every 90° rotation;
//! * one pivot at the footprint's floor centre, so the item rotates in place;
//! * flags 1 (the bit every Nadeo placement carries; no yaw-only, no
//!   auto-rotation, stackable on objects), `AlignToInterior` as the item
//!   editor writes it.
//!
//! A block's ground and air base variants are baked when they differ (the
//! air one gets the `_Air` suffix); identical recipes give one item. Blocks
//! whose picked variant has no geometry (a hidden pillar) or that the bake
//! refuses are listed in the report, never silently absent.
//!
//! Usage: mapgeom item-set --out DIR [--set TinyBlocks] [--scale 0.5]
//!        [--author ID] [--variants both|ground|air] [--only NAME[,NAME]]
//!        [--folder Roads/RoadTech] [--limit N] [--plan-only] [--report TSV]
//!        [--zip FILE] [--collection Stadium]

use crate::store::DataStore;
use crate::tiny_library::{bake_block, bake_env_reset, load_block_info};
use crate::static_item::placement::{PlacementParam, SClass};
use crate::static_item::{file::HeaderChunk, item::ItemChunk, Node};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Opts {
    pub out: PathBuf,
    pub set: String,
    pub author: String,
    pub scale: f32,
    pub collection_name: String,
    pub collection: u32,
    pub variants: Variants,
    pub only: Option<Vec<String>>,
    pub folder: Option<String>,
    pub limit: Option<usize>,
    pub plan_only: bool,
    pub report: Option<PathBuf>,
    pub zip: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Variants {
    Both,
    Ground,
    Air,
}

/// One item to bake.
#[derive(Clone, Debug)]
struct Job {
    /// The block browser folders (`Roads/RoadTech/Main/Main`).
    folders: Vec<String>,
    /// The block info name and pack path.
    name: String,
    path: String,
    ground: bool,
    /// The file stem (`RoadTechStraight`, `RoadTechStraight_Air`).
    stem: String,
    /// The ident (`TinyBlocks\Roads\...\RoadTechStraight.Item.Gbx`).
    ident: String,
    /// Footprint in block units (x, z).
    sx: u32,
    sz: u32,
    /// The variant's label, for the report.
    label: String,
}

struct Baked {
    bytes: Vec<u8>,
    pictures: Vec<(String, Vec<u8>)>,
    visuals: usize,
    triangles: usize,
    notes: Vec<String>,
    waypoint: Option<i32>,
}


/// The editor settings of a tiny block with an `sx` × `sz` unit footprint.
pub fn placement_for(sx: u32, sz: u32, scale: f32, sclass_index: i32) -> PlacementParam {
    let unit_h = 32.0 * scale;
    let unit_v = 8.0 * scale;
    PlacementParam {
        grid_h_step: unit_h / 2.0,
        grid_v_step: unit_v / 2.0,
        fly_v_step: unit_v / 2.0,
        pivot_positions: vec![[sx.max(1) as f32 * unit_h / 2.0, 0.0, sz.max(1) as f32 * unit_h / 2.0]],
        sclass: Some((sclass_index, SClass::default())),
        ..PlacementParam::default()
    }
}

/// Header chunk 0x2E001003 for a set item: ident / collection / author, the
/// `Items` page, item-editor flags (8), catalog position 1, the display name,
/// prod state 3 — `assemble::header_chunks` with a name.
fn desc_chunk(ident: &str, collection: u32, author: &str, name: &str) -> HeaderChunk {
    let mut d = Vec::new();
    let mut lb = crate::static_item::LookbackState::default();
    {
        let mut w = crate::static_item::Wr { w: &mut d, lb: &mut lb };
        w.id(&crate::static_item::Id::Str(ident.to_string()));
        w.id(&crate::static_item::Id::Raw(collection));
        w.id(&crate::static_item::Id::Str(author.to_string()));
        w.u32(8);
        w.string("Items");
        w.id(&crate::static_item::Id::Null);
        w.i32(8);
        w.i16(1);
        w.string(name);
        w.u8(3);
    }
    HeaderChunk { id: 0x2E001003, heavy: false, payload: d }
}

/// The baked item re-dressed for the browser: header desc with the name, the
/// block's icon, the body's name / description / ident author, and the tiny
/// grid placement in place of the library's free one.
fn dress(bytes: &[u8], job: &Job, opts: &Opts, icon: Option<&[u8]>, description: &str) -> Result<Vec<u8>, String> {
    let mut f = crate::static_item::parse_file(bytes)?;
    // header: desc, then the icon right after it (Nadeo's order)
    if let Some(k) = f.header_chunks.iter().position(|c| c.id == 0x2E001003) {
        f.header_chunks[k] = desc_chunk(&job.ident, opts.collection, &opts.author, &job.stem);
        f.header_chunks.retain(|c| c.id != 0x2E001004);
        if let Some(icon) = icon {
            f.header_chunks.insert(k + 1, HeaderChunk { id: 0x2E001004, heavy: false, payload: icon.to_vec() });
        }
    } else {
        return Err("baked item has no collector description header chunk".into());
    }
    let mut placed = false;
    for c in f.item.chunks.iter_mut() {
        match c {
            ItemChunk::Ident { path, author, .. } => {
                *path = crate::static_item::Id::Str(job.ident.clone());
                *author = crate::static_item::Id::Str(opts.author.clone());
            }
            ItemChunk::Name(n) => *n = job.stem.clone(),
            ItemChunk::Description(d) => *d = description.to_string(),
            ItemChunk::DefaultPlacement { placement, .. } => {
                let Some(node) = placement.inline.as_deref_mut() else { return Err("placement node is not inline".into()) };
                let Node::Placement(p) = node else { return Err("placement ref is not a CGameItemPlacementParam".into()) };
                let old = PlacementParam::from_node(p)?;
                let sclass_index = old.sclass.as_ref().map(|(i, _)| *i).unwrap_or(-1);
                let mut new = placement_for(job.sx, job.sz, opts.scale, sclass_index);
                new.extra = old.extra.clone();
                *p = new.to_node()?;
                placed = true;
            }
            _ => {}
        }
    }
    if !placed {
        return Err("baked item has no placement chunk (0x2E00201C)".into());
    }
    // loose files on disk are LZO-compressed like the game's own (an embedded
    // zip deflates them instead; 600 KB of vertex data become ~200)
    f.body_comp = b'C';
    Ok(crate::static_item::write_file(&f))
}

fn bake_job(store: &mut DataStore, job: &Job, opts: &Opts) -> Result<Baked, String> {
    bake_env_reset(opts.collection);
    let bi = crate::blockinfo::load(store, &job.path)?;
    let mut cache = BTreeMap::new();
    let (plan, mut notes) = match plan_standalone(store, &bi, job.ground, &mut cache) {
        Standalone::Bake(b, notes) => (b, notes),
        Standalone::Nothing(why) => return Err(format!("nothing to bake: {why}")),
        Standalone::Refused(e) => return Err(e),
    };
    let legacy = BTreeMap::new();
    let (bytes, m, _) = bake_block(store, &plan, &job.name, &job.path, &bi, &job.ident, opts.scale, opts.collection, &legacy, None, false)?;
    let triangles = m.visuals.iter().map(|v| v.visual.index_buffer.as_ref().map(|ib| ib.indices.len() / 3).unwrap_or(0)).sum();
    notes.extend(m.notes.iter().cloned());
    Ok(Baked { bytes, pictures: m.pictures.clone(), visuals: m.visuals.len(), triangles, notes, waypoint: m.waypoint_type })
}

fn parse_opts(rest: &[String]) -> Result<Opts, String> {
    let flag = |name: &str| rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1).cloned());
    let has = |name: &str| rest.iter().any(|a| a == name);
    let out = PathBuf::from(flag("--out").ok_or("item-set needs --out DIR")?);
    let variants = match flag("--variants").as_deref().unwrap_or("both") {
        "both" => Variants::Both,
        "ground" => Variants::Ground,
        "air" => Variants::Air,
        other => return Err(format!("--variants {other}: both | ground | air")),
    };
    let collection_name = flag("--collection").unwrap_or_else(|| "Stadium".to_string());
    let collection = match collection_name.as_str() {
        "Stadium" => 0x1a,
        "BlueBay" => 0x1c,
        "RedIsland" => 0x10,
        "WhiteShore" => 0x1d,
        "GreenCoast" => 0xf,
        other => return Err(format!("--collection {other}: unknown collection")),
    };
    Ok(Opts {
        out,
        set: flag("--set").unwrap_or_else(|| "TinyBlocks".to_string()),
        author: flag("--author").unwrap_or_else(|| crate::tiny_assets::AUTHOR.to_string()),
        scale: flag("--scale").unwrap_or_else(|| "0.5".to_string()).parse().map_err(|e| format!("--scale: {e}"))?,
        collection_name,
        collection,
        variants,
        only: flag("--only").map(|s| s.split(',').map(String::from).collect()),
        folder: flag("--folder"),
        limit: flag("--limit").map(|s| s.parse()).transpose().map_err(|e| format!("--limit: {e}"))?,
        plan_only: has("--plan-only"),
        report: flag("--report").map(PathBuf::from),
        zip: flag("--zip").map(PathBuf::from),
    })
}

fn tsv_escape(s: &str) -> String {
    s.replace(['\t', '\n'], " ")
}

pub fn run(store: &mut DataStore, rest: &[String]) -> Result<(), String> {
    let opts = parse_opts(rest)?;
    let t0 = std::time::Instant::now();
    let leaves = crate::catalog::block_browser(store, &opts.collection_name)?;
    let mut idx = crate::blockmap::BlockInfoIndex::build(store, &opts.collection_name);
    eprintln!("{} blocks in the {} block browser (DEV excluded); block-info index {} stems", leaves.len(), opts.collection_name, idx.stem_count());

    // --- plan: which variants exist, which differ, the footprints -------------
    let mut report = String::from("folder\tblock\tvariant\tstem\tstatus\tfootprint\twaypoint\tvisuals\ttriangles\tbytes\tdetail\n");
    let mut jobs: Vec<Job> = Vec::new();
    let mut icons: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut planned = 0usize;
    let mut clip_cache: BTreeMap<String, Result<crate::blockinfo::BlockInfo, String>> = BTreeMap::new();
    for leaf in &leaves {
        if let Some(only) = &opts.only {
            if !only.iter().any(|n| n == &leaf.name) {
                continue;
            }
        }
        if let Some(f) = &opts.folder {
            if !leaf.folders.join("/").starts_with(f.as_str()) {
                continue;
            }
        }
        if let Some(l) = opts.limit {
            if jobs.len() >= l {
                break;
            }
        }
        let folder = leaf.folders.join("/");
        let (path, bi) = match load_block_info(&mut idx, store, &leaf.name) {
            Ok(x) => x,
            Err(e) => {
                report.push_str(&format!("{folder}\t{}\t-\t-\tMISSING\t-\t-\t-\t-\t-\t{}\n", leaf.name, tsv_escape(&e)));
                continue;
            }
        };
        // the block's icon, for every item made of it
        if let Ok(bytes) = store.read(&path) {
            if let Ok((_, Some(icon))) = crate::catalog::CollectorDesc::from_file(&bytes) {
                icons.insert(leaf.name.clone(), icon.payload);
            }
        }
        // both base variants, then the names: one item when the two recipes
        // agree (or only one exists); else the AIR one takes the plain name —
        // it is the block as it looks anywhere but on the ground row — and the
        // ground one is `<Block>_Ground`
        let wanted: &[bool] = match opts.variants {
            Variants::Both => &[false, true],
            Variants::Ground => &[true],
            Variants::Air => &[false],
        };
        let mut baked_variants: Vec<(bool, String, u32, u32, String)> = Vec::new(); // (ground, recipe, sx, sz, label)
        for &ground in wanted {
            let variant = if ground { "ground" } else { "air" };
            match plan_standalone(store, &bi, ground, &mut clip_cache) {
                Standalone::Nothing(why) => {
                    report.push_str(&format!("{folder}\t{}\t{variant}\t-\tNOTHING\t-\t-\t-\t-\t-\t{}\n", leaf.name, tsv_escape(&why)));
                }
                Standalone::Refused(error) => {
                    report.push_str(&format!("{folder}\t{}\t{variant}\t-\tREFUSED\t-\t-\t-\t-\t-\t{}\n", leaf.name, tsv_escape(&error)));
                }
                Standalone::Bake(b, notes) => {
                    for n in notes {
                        report.push_str(&format!("{folder}\t{}\t{variant}\t-\tNOTE\t-\t-\t-\t-\t-\t{}\n", leaf.name, tsv_escape(&n)));
                    }
                    baked_variants.push((ground, b.recipe.clone(), b.footprint.sx, b.footprint.sz, b.pk.label.clone()));
                }
            }
        }
        let both_differ = baked_variants.len() == 2 && baked_variants[0].1 != baked_variants[1].1;
        for (k, (ground, recipe, sx, sz, label)) in baked_variants.iter().enumerate() {
            if k == 1 && !both_differ {
                report.push_str(&format!("{folder}\t{}\t{}\t{}\tSAME\t-\t-\t-\t-\t-\tidentical to the {} variant\n", leaf.name, if *ground { "ground" } else { "air" }, leaf.name, if *ground { "air" } else { "ground" }));
                continue;
            }
            let stem = if both_differ && *ground { format!("{}_Ground", leaf.name) } else { leaf.name.clone() };
            let _ = recipe;
            let mut ident_parts = vec![opts.set.clone()];
            ident_parts.extend(leaf.folders.iter().cloned());
            let ident = format!("{}\\{stem}.Item.Gbx", ident_parts.join("\\"));
            jobs.push(Job { folders: leaf.folders.clone(), name: leaf.name.clone(), path: path.clone(), ground: *ground, stem, ident, sx: *sx, sz: *sz, label: label.clone() });
            planned += 1;
        }
    }
    eprintln!("{planned} items planned ({} blocks with both variants differing) in {:.1} s", jobs.iter().filter(|j| j.stem.ends_with("_Ground")).count(), t0.elapsed().as_secs_f32());
    if opts.plan_only {
        for j in &jobs {
            println!("{}\t{}\t{}\t{}\t{}x{}\t{}", j.folders.join("/"), j.name, if j.ground { "ground" } else { "air" }, j.stem, j.sx, j.sz, j.label);
        }
        if let Some(r) = &opts.report {
            std::fs::write(r, &report).map_err(|e| format!("{}: {e}", r.display()))?;
        }
        return Ok(());
    }

    // --- bake, on every core -------------------------------------------------
    let t1 = std::time::Instant::now();
    let results: Vec<Result<Baked, String>> = crate::par::map(store, &jobs, |st, _, job| bake_job(st, job, &opts));
    eprintln!("{} bakes in {:.1} s", results.len(), t1.elapsed().as_secs_f32());

    // --- dress and write -----------------------------------------------------
    let root = opts.out.join("Items").join(&opts.set);
    let mut written = 0usize;
    let mut failed = 0usize;
    let mut total_bytes = 0usize;
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (job, res) in jobs.iter().zip(results) {
        let folder = job.folders.join("/");
        let variant = if job.ground { "ground" } else { "air" };
        match res {
            Err(e) => {
                failed += 1;
                report.push_str(&format!("{folder}\t{}\t{variant}\t{}\tFAILED\t{}x{}\t-\t-\t-\t-\t{}\n", job.name, job.stem, job.sx, job.sz, tsv_escape(&e)));
            }
            Ok(b) => {
                let description = format!(
                    "Tiny Blocks: the Nadeo block {} ({} variant) at scale {}, folder {}. Grid {} m, height step {} m, pivot at the footprint centre ({}x{} units).",
                    job.name,
                    variant,
                    opts.scale,
                    folder,
                    32.0 * opts.scale / 2.0,
                    8.0 * opts.scale / 2.0,
                    job.sx,
                    job.sz
                );
                match dress(&b.bytes, job, &opts, icons.get(&job.name).map(|v| v.as_slice()), &description) {
                    Err(e) => {
                        failed += 1;
                        report.push_str(&format!("{folder}\t{}\t{variant}\t{}\tDRESS-FAILED\t{}x{}\t-\t-\t-\t-\t{}\n", job.name, job.stem, job.sx, job.sz, tsv_escape(&e)));
                    }
                    Ok(bytes) => {
                        let dir: PathBuf = job.folders.iter().fold(root.clone(), |d, f| d.join(f));
                        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                        let file = dir.join(format!("{}.Item.Gbx", job.stem));
                        std::fs::write(&file, &bytes).map_err(|e| format!("{}: {e}", file.display()))?;
                        let rel = format!("Items/{}/{}/{}.Item.Gbx", opts.set, folder, job.stem);
                        total_bytes += bytes.len();
                        // the pictures the item's materials name, next to it
                        for (pic, dds) in &b.pictures {
                            let pf = dir.join(pic);
                            if !pf.is_file() {
                                std::fs::write(&pf, dds).map_err(|e| format!("{}: {e}", pf.display()))?;
                            }
                            files.entry(format!("Items/{}/{}/{}", opts.set, folder, pic)).or_insert_with(|| dds.clone());
                        }
                        let notes: Vec<&str> = b.notes.iter().filter(|n| n.contains("skipped") || n.contains("failed") || n.contains("unnamed") || n.contains("refused")).map(|s| s.as_str()).collect();
                        if std::env::var_os("ITEMSET_NOTES").is_some() {
                            for n in &b.notes {
                                eprintln!("  [{}] {n}", job.stem);
                            }
                        }
                        report.push_str(&format!(
                            "{folder}\t{}\t{variant}\t{}\tOK\t{}x{}\t{}\t{}\t{}\t{}\t{}\n",
                            job.name,
                            job.stem,
                            job.sx,
                            job.sz,
                            b.waypoint.map(|w| w.to_string()).unwrap_or_else(|| "-".into()),
                            b.visuals,
                            b.triangles,
                            bytes.len(),
                            tsv_escape(&notes.join(" | "))
                        ));
                        files.insert(rel, bytes);
                        written += 1;
                    }
                }
            }
        }
    }
    if let Some(r) = &opts.report {
        std::fs::write(r, &report).map_err(|e| format!("{}: {e}", r.display()))?;
    } else {
        std::fs::create_dir_all(&opts.out).ok();
        std::fs::write(opts.out.join("item-set-report.tsv"), &report).ok();
    }
    if let Some(z) = &opts.zip {
        let bytes = crate::tiny_assets::zip(&files);
        std::fs::write(z, &bytes).map_err(|e| format!("{}: {e}", z.display()))?;
        eprintln!("zip: {} entries, {:.1} MB -> {}", files.len(), bytes.len() as f64 / 1e6, z.display());
    }
    eprintln!("{written} items written ({:.1} MB), {failed} failed, under {} in {:.1} s", total_bytes as f64 / 1e6, root.display(), t0.elapsed().as_secs_f32());
    Ok(())
}

/// The `item-set` command entry: errors are printed and exit 1.
pub fn cmd(store: &mut DataStore, rest: &[String]) {
    if let Err(e) = run(store, rest) {
        eprintln!("item-set: {e}");
        std::process::exit(1);
    }
}

pub fn out_root(out: &Path, set: &str) -> PathBuf {
    out.join("Items").join(set)
}
/// What the engine draws for a block standing ALONE — its own prefabs plus
/// every free clip filler of every unit face (nothing next to it deletes
/// one; anti-clips, drawn only when deleted, stay away). The filler stands
/// in the cell across the face, turned to face it (`bake::record_slot`
/// backwards: side clip → cell + step(face), dir opposite(face); a bottom
/// clip → the cell below, a top clip → the cell above, dir = the clip's own
/// direction bits), placed like a 1×1 grid block (`place::grid_block`).
/// Returns (prefab path, transform in the owner's frame) per filler piece,
/// with notes.
pub fn standalone_fillers(store: &mut DataStore, variant: &crate::blockinfo::Variant, owner_ground: bool, cache: &mut BTreeMap<String, Result<crate::blockinfo::BlockInfo, String>>) -> (Vec<(String, crate::geom::Xform)>, Vec<String>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    for u in &variant.block_units {
        for face in 0..6 {
            for (ci, clip_path) in u.clips[face].iter().enumerate() {
                let bi_c = match cache.entry(clip_path.clone()).or_insert_with(|| crate::blockinfo::load(store, clip_path)) {
                    Ok(b) => b,
                    Err(e) => {
                        notes.push(format!("clip {clip_path}: {e}"));
                        continue;
                    }
                };
                let Some(c) = bi_c.clip.as_ref() else {
                    notes.push(format!("clip {clip_path}: not a clip block info"));
                    continue;
                };
                // the engine's IsFreeClip and IsAntiClip
                if !c.clip_type.map(|t| t != 0).unwrap_or(false) {
                    continue;
                }
                if c.extra_bytes.get(2).copied().unwrap_or(0) != 0 {
                    continue;
                }
                let (cell, dir) = match face {
                    0..=3 => {
                        let (dx, dy, dz) = crate::bake::step(face);
                        ([u.offset[0] + dx, u.offset[1] + dy, u.offset[2] + dz], crate::bake::opposite(face) as u8)
                    }
                    4 => ([u.offset[0], u.offset[1] + 1, u.offset[2]], ((u.u00c[1] >> (2 * ci)) & 3) as u8),
                    _ => ([u.offset[0], u.offset[1] - 1, u.offset[2]], ((u.u00c[0] >> (2 * ci)) & 3) as u8),
                };
                // the clip block's ground bit is its owner's, on the owner's ground row (side clips only)
                let ground_clip = face < 4 && owner_ground && u.offset[1] == 0;
                let Some(pk) = bi_c.pick_placement_add(ground_clip, 0, 0, 0).or_else(|| bi_c.pick_placement_add(!ground_clip, 0, 0, 0)) else {
                    notes.push(format!("clip {}: no variant with content", bi_c.name));
                    continue;
                };
                let (fx, fz) = pk.variant.block_units.iter().fold((1i32, 1i32), |(a, b), bu| (a.max(bu.offset[0] + 1), b.max(bu.offset[2] + 1)));
                let base = crate::place::grid_block((cell[0], cell[1], cell[2]), dir, (fx as f32 * 32.0, fz as f32 * 32.0), 0.0);
                for mb in &pk.mobils {
                    let Some(p) = mb.prefab.as_ref() else { continue };
                    let t = mb.translation.unwrap_or([0.0; 3]);
                    let local = match mb.rotation.filter(|r| r.iter().any(|v| v.abs() > 1e-6)) {
                        Some(r) => crate::geom::rotation_xyz_deg(r, t),
                        None => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, t[0], t[1], t[2]],
                    };
                    out.push((p.clone(), crate::geom::compose(&base, &local)));
                }
            }
        }
    }
    (out, notes)
}

/// The standalone plan of one block variant: the tiny library's `BlockBake`
/// with the engine's fillers added, or why there is no item.
pub(crate) enum Standalone<'b> {
    Bake(Box<crate::tiny_library::BlockBake<'b>>, Vec<String>),
    Nothing(String),
    Refused(String),
}

pub(crate) fn plan_standalone<'b>(store: &mut DataStore, bi: &'b crate::blockinfo::BlockInfo, ground: bool, cache: &mut BTreeMap<String, Result<crate::blockinfo::BlockInfo, String>>) -> Standalone<'b> {
    use crate::tiny_library::{BlockBake, Footprint};
    let Some(pk) = bi.pick_placement_add(ground, 0, 0, 0) else {
        return Standalone::Refused("block info has no variant with units or mobils".into());
    };
    let units: Vec<[i32; 3]> = pk.variant.block_units.iter().map(|u| u.offset).collect();
    let (sx, sz) = units.iter().fold((1u32, 1u32), |(sx, sz), u| (sx.max(u[0] as u32 + 1), sz.max(u[2] as u32 + 1)));
    let prefabs: Vec<(String, Option<[f32; 3]>, Option<[f32; 3]>)> = pk.mobils.iter().filter_map(|mb| mb.prefab.clone().map(|p| (p, mb.translation, mb.rotation))).collect();
    let solids: Vec<String> = pk.mobils.iter().filter_map(|mb| mb.solid.clone()).collect();
    let (fillers, notes) = standalone_fillers(store, pk.variant, ground, cache);
    if prefabs.is_empty() && fillers.is_empty() {
        if solids.is_empty() {
            return Standalone::Nothing(format!("no geometry in this variant [{}], no free-clip filler either", pk.label));
        }
        return Standalone::Refused(format!("legacy CPlugSolid model without a converted archive item ({solids:?})"));
    }
    let effective_mods: Vec<String> = bi.material_modifier.clone();
    let recipe = format!(
        "{}|wp{:?}|units{:?}|mod{:?}|fillers{:?}",
        prefabs.iter().map(|p| format!("{}@{:?}/{:?}", p.0, p.1, p.2)).collect::<Vec<_>>().join(","),
        bi.waypoint_type,
        units,
        effective_mods,
        fillers.iter().map(|(p, x)| format!("{p}@{:?}", x.iter().map(|v| (v * 100.0).round() as i32).collect::<Vec<_>>())).collect::<Vec<_>>()
    );
    let terrain = matches!(bi.kind, crate::blockinfo::Kind::Flat | crate::blockinfo::Kind::Frontier | crate::blockinfo::Kind::Transition);
    Standalone::Bake(Box::new(BlockBake { pk, footprint: Footprint { sx, sz, units, auto_terrain: None }, prefabs, legacy_item: None, recipe, effective_mods, terrain, fillers }), notes)
}

/// `mapgeom item-bounds FILE...`: the visual bounds (every visual's vertex
/// positions, and the union), the collision bounds and the placement
/// parameters of a static item — the offline check that a bake landed where
/// the block's cell is (a wall checkpoint's slab against its wall, a filler
/// under its owner) before a game load.
pub fn bounds_cmd(rest: &[String]) {
    let files: Vec<&String> = rest.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    let verbose = rest.iter().any(|a| a == "--visuals");
    for path in files {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                println!("{path}: {e}");
                continue;
            }
        };
        let f = match crate::static_item::parse_file(&bytes) {
            Ok(f) => f,
            Err(e) => {
                println!("{path}: {e}");
                continue;
            }
        };
        let mut all = Bounds::default();
        let mut lines = Vec::new();
        if let Some(so) = f.item.static_object() {
            if let Some(s2) = so.solid2() {
                for (k, v) in s2.visuals.iter().enumerate() {
                    let Some(Node::Visual(vis)) = v.inline.as_deref() else { continue };
                    let Some(st) = vis.stream() else { continue };
                    let Some(i) = st.decls.iter().position(|d| d.name() == crate::static_item::vstream::N_POSITION) else { continue };
                    let crate::static_item::vstream::Elem::Float3(pos) = &st.elems[i] else { continue };
                    let mut b = Bounds::default();
                    for p in pos {
                        b.add(*p);
                    }
                    let lod = s2.shaded_geoms.iter().find(|g| g.visual_index == k as i32).map(|g| g.lod_mask).unwrap_or(0);
                    if lod & 1 != 0 || lod == 0 {
                        all.merge(&b);
                    }
                    if verbose {
                        lines.push(format!("  visual {k:3} lod {lod:2} {} verts  {}", pos.len(), b));
                    }
                }
            }
            let coll = so.surface().map(|s| match &s.surf {
                crate::static_item::surface::Surf::Mesh { vertices, .. } => {
                    let mut b = Bounds::default();
                    for p in vertices {
                        b.add(*p);
                    }
                    format!("{} ({} vertices)", b, vertices.len())
                }
                other => format!("{:?}", std::mem::discriminant(other)),
            });
            println!("{path}\n  visual (LOD 0) {all}\n  collision {}", coll.unwrap_or_else(|| "none (mesh collidable or no shape)".into()));
        } else if let Some(Node::Prefab(p)) = f.item.model().and_then(|mc| mc.entity_model.inline.as_deref()) {
            // the prefab form: every entity's static object, through its pose
            let mut nso = 0usize;
            for (k, e) in p.ents.iter().enumerate() {
                let Some(Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
                nso += 1;
                let at = crate::geom::from_quat(e.rot, e.pos);
                if let Some(s2) = so.solid2() {
                    let mut eb = Bounds::default();
                    for (vk, v) in s2.visuals.iter().enumerate() {
                        let Some(Node::Visual(vis)) = v.inline.as_deref() else { continue };
                        let Some(st) = vis.stream() else { continue };
                        let Some(i) = st.decls.iter().position(|d| d.name() == crate::static_item::vstream::N_POSITION) else { continue };
                        let crate::static_item::vstream::Elem::Float3(pos) = &st.elems[i] else { continue };
                        let lod = s2.shaded_geoms.iter().find(|g| g.visual_index == vk as i32).map(|g| g.lod_mask).unwrap_or(0);
                        if lod & 1 == 0 && lod != 0 {
                            continue;
                        }
                        for q in pos {
                            eb.add(crate::geom::apply(&at, *q));
                        }
                    }
                    if verbose {
                        lines.push(format!("  entity {k:3} static object at {:?}: {eb}", e.pos));
                    }
                    all.merge(&eb);
                }
            }
            println!("{path}\n  prefab form: {} entities, {nso} static objects; visual (LOD 0) {all}", p.ents.len());
        } else {
            println!("{path}\n  (no static object: block item or unknown form)");
        }
        for c in &f.item.chunks {
            if let ItemChunk::DefaultPlacement { placement, .. } = c {
                if let Some(Node::Placement(p)) = placement.inline.as_deref() {
                    match PlacementParam::from_node(p) {
                        Ok(pp) => println!("  placement {}", pp.summary()),
                        Err(e) => println!("  placement: {e}"),
                    }
                }
            }
        }
        for l in lines {
            println!("{l}");
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub n: usize,
}

impl Default for Bounds {
    fn default() -> Bounds {
        Bounds { min: [f32::MAX; 3], max: [f32::MIN; 3], n: 0 }
    }
}

impl Bounds {
    pub fn add(&mut self, p: [f32; 3]) {
        for i in 0..3 {
            self.min[i] = self.min[i].min(p[i]);
            self.max[i] = self.max[i].max(p[i]);
        }
        self.n += 1;
    }
    pub fn merge(&mut self, o: &Bounds) {
        if o.n == 0 {
            return;
        }
        for i in 0..3 {
            self.min[i] = self.min[i].min(o.min[i]);
            self.max[i] = self.max[i].max(o.max[i]);
        }
        self.n += o.n;
    }
}

impl std::fmt::Display for Bounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.n == 0 {
            return write!(f, "(empty)");
        }
        write!(f, "x {:.2}..{:.2}  y {:.2}..{:.2}  z {:.2}..{:.2}", self.min[0], self.max[0], self.min[1], self.max[1], self.min[2], self.max[2])
    }
}
