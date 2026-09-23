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
    /// `--no-skin-header`: leave out the source model's CPlugGameSkin header
    /// chunk (0x090F4000, the advertisement / colour skin slot). item.exchange's
    /// parser (an old ManiaPlanetSharp) refuses items that carry the version-8
    /// chunk the game writes today (2026-09-22: every set with a start,
    /// checkpoint or finish failed to upload); without it the slot shows the
    /// material's own default picture.
    pub no_skin_header: bool,
    /// `--form merged`: one merged mesh per item (the campaign's static form)
    /// instead of the default INSTANCED prefab (`Merged::share`: one node per
    /// distinct sub-object, one entity per placement — 12 % smaller over the
    /// set, 40 % on the road straights, and every sub-object keeps the pack's
    /// own lightmap layout).
    pub merged: bool,
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
    if opts.no_skin_header {
        f.header_chunks.retain(|c| c.id != 0x090F4000);
    }
    // header: desc, then the icon right after it (Nadeo's order)
    if let Some(k) = f.header_chunks.iter().position(|c| c.id == 0x2E001003) {
        f.header_chunks[k] = desc_chunk(&job.ident, opts.collection, &opts.author, &job.stem);
        f.header_chunks.retain(|c| c.id != 0x2E001004);
        if let Some(icon) = icon {
            // the icon is a HEAVY header chunk (bit 31 of its size word) in every
            // game-written file — block infos and item-editor items alike
            f.header_chunks.insert(k + 1, HeaderChunk { id: 0x2E001004, heavy: true, payload: icon.to_vec() });
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
    // THE PLACEMENT RULE (measured 2026-09-23, TinySet4..15 on the render box):
    // a loose item with pivot V placed at P with rotation R has its geometry,
    // its collision AND its spawn entity at P + R(p + V) — the same rule for
    // all three. (A morning was lost to the assumption "geometry at P + R(p -
    // V)": the spawn then looked displaced by 2V, a compensation put the car
    // off the deck, and every drop test was read against the wrong deck.) The
    // pack's ISO layout — spawn in the SSpawn node's Iso4, entity at 0 —
    // differs: that one lands at P + R(s), without the pivot; so the spawn is
    // the ENTITY position (Nadeo's start gates' layout), nothing compensated.
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
    // The form: the instanced prefab unless `--form merged`; `TINY_WP_MERGED=1`
    // bakes the waypoint blocks (start, checkpoint, finish, multilap) in the
    // entity-model form (a test knob — the prefab form's SSpawn layout is the
    // one verified in-game, see `dress`).
    let waypoint = bi.waypoint_type.map(|t| t != 3).unwrap_or(false);
    let share = !opts.merged && !(waypoint && std::env::var("TINY_WP_MERGED").as_deref() == Ok("1"));
    crate::static_item::merged::SHARE_OVERRIDE.with(|o| o.set(Some(share)));
    let r = bake_block(store, &plan, &job.name, &job.path, &bi, &job.ident, opts.scale, opts.collection, &legacy, None, false);
    crate::static_item::merged::SHARE_OVERRIDE.with(|o| o.set(None));
    let (bytes, m, _) = r?;
    let triangles = m.triangle_count();
    notes.extend(m.notes.iter().cloned());
    Ok(Baked { bytes, pictures: m.pictures.clone(), visuals: m.visual_count(), triangles, notes, waypoint: m.waypoint_type })
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
        no_skin_header: has("--no-skin-header"),
        merged: flag("--form").map(|f| f == "merged").unwrap_or(false),
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
                match icon.to_raw_payload() {
                    Ok(raw) => {
                        icons.insert(leaf.name.clone(), raw);
                    }
                    Err(e) => report.push_str(&format!("{folder}\t{}\t-\t-\tNOTE\t-\t-\t-\t-\t-\ticon: {}\n", leaf.name, tsv_escape(&e))),
                }
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
        let mut baked_bakes: Vec<Box<crate::tiny_library::BlockBake<'_>>> = Vec::new();
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
                    baked_bakes.push(b);
                }
            }
        }
        // a ground variant that is the air one MINUS some fillers (the
        // platforms: air = side skirts + underside slab, ground = the skirts)
        // adds nothing a mapper can see once the piece sits on something: one
        // item, the air one. A ground variant that ADDS a filler (the roads'
        // grass skirt) or changes the prefabs is a second item.
        let ground_is_subset = baked_bakes.len() == 2 && {
            let (a, g) = if baked_variants[0].0 { (&baked_bakes[1], &baked_bakes[0]) } else { (&baked_bakes[0], &baked_bakes[1]) };
            let key = |(p, x): &(String, crate::geom::Xform)| format!("{p}@{:?}", x.iter().map(|v| (v * 100.0).round() as i32).collect::<Vec<_>>());
            let mut pool: Vec<String> = a.fillers.iter().map(key).collect();
            let core_same = a.prefabs == g.prefabs && a.footprint.units == g.footprint.units && a.effective_mods == g.effective_mods;
            core_same
                && g.fillers.iter().all(|f| {
                    let k = key(f);
                    match pool.iter().position(|q| *q == k) {
                        Some(i) => {
                            pool.swap_remove(i);
                            true
                        }
                        None => false,
                    }
                })
        };
        let recipes_differ = baked_variants.len() == 2 && baked_variants[0].1 != baked_variants[1].1;
        let both_differ = recipes_differ && !ground_is_subset;
        for (ground, recipe, sx, sz, label) in baked_variants.iter() {
            if baked_variants.len() == 2 && !both_differ && *ground {
                let why = if recipes_differ { "the air variant minus some fillers (its underside): the air item stands for both" } else { "identical to the air variant" };
                report.push_str(&format!("{folder}\t{}\tground\t{}\tSAME\t-\t-\t-\t-\t-\t{why}\n", leaf.name, leaf.name));
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
    crate::static_item::merged::SHARE.store(!opts.merged, std::sync::atomic::Ordering::Relaxed);
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
            // the prefab form: every entity's static object, through its pose —
            // an INSTANCE entity (a bare reference to a node an earlier entity
            // defined) through the defining entity's model
            let mut nso = 0usize;
            let mut coll_all = Bounds::default();
            let mut coll_n = 0usize;
            let defined: std::collections::HashMap<i32, &crate::static_item::item::CPlugStaticObjectModel> = p.ents.iter().filter_map(|e| match e.model.inline.as_deref() { Some(Node::StaticObject(so)) => Some((e.model.index, so)), _ => None }).collect();
            for (k, e) in p.ents.iter().enumerate() {
                let so = match e.model.inline.as_deref() {
                    Some(Node::StaticObject(so)) => so,
                    None => match defined.get(&e.model.index) { Some(so) => *so, None => continue },
                    _ => continue,
                };
                nso += 1;
                let at = crate::geom::from_quat(e.rot, e.pos);
                if let Some(Node::Surface(sf)) = so.shape.inline.as_deref() {
                    if let crate::static_item::surface::Surf::Mesh { vertices, .. } = &sf.surf {
                        let mut cb = Bounds::default();
                        for q in vertices {
                            cb.add(crate::geom::apply(&at, *q));
                        }
                        coll_n += vertices.len();
                        coll_all.merge(&cb);
                        if verbose {
                            lines.push(format!("  entity {k:3} collision at {:?}: {cb} ({} vertices)", e.pos, vertices.len()));
                        }
                    }
                }
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
            println!("{path}\n  prefab form: {} entities, {nso} static objects (instances included); visual (LOD 0) {all}\n  collision {coll_all} ({coll_n} vertices)", p.ents.len());
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

// ---------------------------------------------------------------------------
// The item browser: Nadeo's own items at half scale (`mapgeom item-set-items`)
// ---------------------------------------------------------------------------

/// The pack item's own placement parameters (its external `.PlaceParam.Gbx`,
/// or the inline node of its body), or the item-editor defaults.
fn pack_item_placement(store: &mut DataStore, item_path: &str) -> Result<PlacementParam, String> {
    let m = store.load_model(item_path)?;
    if let Some(pp) = m.externals.iter().map(|(_, e)| e.clone()).find(|e| e.ends_with(".PlaceParam.Gbx")) {
        let bytes = store.read(&pp)?;
        return crate::static_item::placement::parse_place_param_file(&bytes);
    }
    // inline: through the typed item parser (the body is uncompressed in the packs)
    let bytes = store.read(item_path)?;
    if let Ok(f) = crate::static_item::parse_file(&bytes) {
        for c in &f.item.chunks {
            if let ItemChunk::DefaultPlacement { placement, .. } = c {
                if let Some(Node::Placement(p)) = placement.inline.as_deref() {
                    return PlacementParam::from_node(p);
                }
            }
        }
    }
    Ok(PlacementParam::default())
}

/// The placement of a scaled copy: every length scaled, the flags kept, the
/// placement class reduced to the item-editor default (a size group is an Id
/// in the body's lookback table, which the raw chunk cannot name).
pub fn scaled_placement(p: &PlacementParam, scale: f32, sclass_index: i32) -> PlacementParam {
    let s = |v: f32| if v > 0.0 { v * scale } else { v };
    PlacementParam {
        version: p.version,
        flags: p.flags,
        cube_center: [p.cube_center[0] * scale, p.cube_center[1] * scale, p.cube_center[2] * scale],
        cube_size: p.cube_size * scale,
        grid_h_step: s(p.grid_h_step),
        grid_v_step: s(p.grid_v_step),
        grid_h_offset: p.grid_h_offset * scale,
        grid_v_offset: p.grid_v_offset * scale,
        fly_v_step: s(p.fly_v_step),
        fly_v_offset: p.fly_v_offset * scale,
        pivot_snap_distance: s(p.pivot_snap_distance),
        pivot_positions: p.pivot_positions.iter().map(|v| [v[0] * scale, v[1] * scale, v[2] * scale]).collect(),
        pivot_rotations: p.pivot_rotations.clone(),
        magnet_locs: p.magnet_locs.iter().map(|(pos, rot)| ([pos[0] * scale, pos[1] * scale, pos[2] * scale], *rot)).collect(),
        magnet_version: p.magnet_version,
        sclass: Some((sclass_index, SClass::default())),
        extra: Vec::new(),
    }
}

/// Re-dress a baked item copy of a pack item: ident/author/name/description,
/// the pack item's icon, its placement scaled.
fn dress_item(bytes: &[u8], ident: &str, stem: &str, author: &str, collection: u32, icon: Option<&[u8]>, description: &str, placement: &PlacementParam, scale: f32, no_skin_header: bool) -> Result<Vec<u8>, String> {
    let mut f = crate::static_item::parse_file(bytes)?;
    if no_skin_header {
        f.header_chunks.retain(|c| c.id != 0x090F4000);
    }
    if let Some(k) = f.header_chunks.iter().position(|c| c.id == 0x2E001003) {
        f.header_chunks[k] = desc_chunk(ident, collection, author, stem);
        f.header_chunks.retain(|c| c.id != 0x2E001004);
        if let Some(icon) = icon {
            f.header_chunks.insert(k + 1, HeaderChunk { id: 0x2E001004, heavy: true, payload: icon.to_vec() });
        }
    } else {
        return Err("baked item has no collector description header chunk".into());
    }
    let mut placed = false;
    for c in f.item.chunks.iter_mut() {
        match c {
            ItemChunk::Ident { path, author: a, .. } => {
                *path = crate::static_item::Id::Str(ident.to_string());
                *a = crate::static_item::Id::Str(author.to_string());
            }
            ItemChunk::Name(n) => *n = stem.to_string(),
            ItemChunk::Description(d) => *d = description.to_string(),
            ItemChunk::DefaultPlacement { placement: pref, .. } => {
                let Some(node) = pref.inline.as_deref_mut() else { return Err("placement node is not inline".into()) };
                let Node::Placement(p) = node else { return Err("placement ref is not a CGameItemPlacementParam".into()) };
                let old = PlacementParam::from_node(p)?;
                let sclass_index = old.sclass.as_ref().map(|(i, _)| *i).unwrap_or(-1);
                *p = scaled_placement(placement, scale, sclass_index).to_node()?;
                placed = true;
            }
            _ => {}
        }
    }
    if !placed {
        return Err("baked item has no placement chunk (0x2E00201C)".into());
    }
    f.body_comp = b'C';
    Ok(crate::static_item::write_file(&f))
}

#[derive(Clone, Debug)]
struct ItemJob {
    folders: Vec<String>,
    name: String,
    path: String,
    stem: String,
    ident: String,
}

fn bake_item_job(store: &mut DataStore, job: &ItemJob, opts: &Opts) -> Result<(Baked, PlacementParam, Option<Vec<u8>>), String> {
    bake_env_reset(opts.collection);
    let mut pnotes: Vec<String> = Vec::new();
    let placement = match pack_item_placement(store, &job.path) {
        Ok(p) => {
            if crate::static_item::placement::was_truncated(&p) {
                pnotes.push("placement file: garbage tail after the grid chunk (pak reader); pivots/magnets/class from what parsed".into());
            }
            crate::static_item::placement::without_marker(&p)
        }
        Err(e) => {
            pnotes.push(format!("placement params unreadable ({e}); item-editor defaults"));
            PlacementParam::default()
        }
    };
    let icon = store.read(&job.path).ok().and_then(|b| crate::catalog::CollectorDesc::from_file(&b).ok()).and_then(|(_, i)| i).and_then(|i| i.to_raw_payload().ok());
    // vegetation (a VegetTreeModel behind the item) takes the tree bake; everything else the pack-item path
    let is_tree = crate::veget::tree_model_path(store, &job.path).is_ok();
    let (bytes, m) = if is_tree {
        let (b, m, _) = crate::static_item::build::static_item_from_veget_report(store, &job.path, &job.ident, &opts.author, opts.scale, opts.collection)?;
        (b, m)
    } else {
        crate::static_item::build::static_item_from_pack_item_report_skin(store, &job.path, &job.ident, &opts.author, opts.scale, opts.collection, 0, None)?
    };
    let triangles = m.triangle_count();
    let mut notes = m.notes.clone();
    notes.extend(pnotes);
    Ok((Baked { bytes, pictures: m.pictures.clone(), visuals: m.visual_count(), triangles, notes, waypoint: m.waypoint_type }, placement, icon))
}

/// `mapgeom item-set-items --out DIR [--set TinyItems] [--scale 0.5] [--author ID]
/// [--only N,N] [--folder Deco/Flags] [--limit N] [--plan-only] [--report TSV]`:
/// every item of the editor's item browser (the pack's CGameItemModelTreeRoot,
/// `Dev` excluded) as a half-scale copy under `Items/<set>/<browser folders>/`,
/// with the item's own icon and its own placement parameters scaled.
pub fn run_items(store: &mut DataStore, rest: &[String]) -> Result<(), String> {
    let mut opts = parse_opts(rest)?;
    if !rest.iter().any(|a| a == "--set") {
        opts.set = "TinyItems".to_string();
    }
    let t0 = std::time::Instant::now();
    let trees = crate::catalog::browser_trees(store, &opts.collection_name);
    let (_, _, leaves) = trees.into_iter().filter(|(_, c, _)| c == "CGameItemModelTreeRoot").max_by_key(|(_, _, l)| l.len()).ok_or("no CGameItemModelTreeRoot tree file in the packs")?;
    let leaves: Vec<crate::catalog::TreeLeaf> = leaves.into_iter().filter(|l| l.folders.first().map(|f| f != "Dev").unwrap_or(true)).collect();
    eprintln!("{} items in the {} item browser (Dev excluded)", leaves.len(), opts.collection_name);
    let mut report = String::from("folder\titem\tstem\tstatus\tpack\tvisuals\ttriangles\tbytes\tplacement\tdetail\n");
    let mut jobs: Vec<ItemJob> = Vec::new();
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
        // a leaf may be a bare name or a pack path
        let path = if leaf.name.contains('\\') { Some(leaf.name.clone()) } else { crate::tiny_library::find_item_file(store, &leaf.name) };
        let Some(path) = path else {
            report.push_str(&format!("{folder}\t{}\t-\tMISSING\t-\t-\t-\t-\t-\tno .Item.Gbx of this name in the packs\n", leaf.name));
            continue;
        };
        let stem = leaf.name.rsplit('\\').next().unwrap_or(&leaf.name).trim_end_matches(".Item.Gbx").to_string();
        let mut ident_parts = vec![opts.set.clone()];
        ident_parts.extend(leaf.folders.iter().cloned());
        let ident = format!("{}\\{stem}.Item.Gbx", ident_parts.join("\\"));
        jobs.push(ItemJob { folders: leaf.folders.clone(), name: leaf.name.clone(), path, stem, ident });
    }
    eprintln!("{} items planned in {:.1} s", jobs.len(), t0.elapsed().as_secs_f32());
    if opts.plan_only {
        for j in &jobs {
            println!("{}\t{}\t{}", j.folders.join("/"), j.stem, j.path);
        }
        if let Some(r) = &opts.report {
            std::fs::write(r, &report).map_err(|e| format!("{}: {e}", r.display()))?;
        }
        return Ok(());
    }
    crate::static_item::merged::SHARE.store(!opts.merged, std::sync::atomic::Ordering::Relaxed);
    let t1 = std::time::Instant::now();
    let results = crate::par::map(store, &jobs, |st, _, job| bake_item_job(st, job, &opts));
    eprintln!("{} bakes in {:.1} s", results.len(), t1.elapsed().as_secs_f32());
    let root = opts.out.join("Items").join(&opts.set);
    let (mut written, mut failed, mut total_bytes) = (0usize, 0usize, 0usize);
    for (job, res) in jobs.iter().zip(results) {
        let folder = job.folders.join("/");
        match res {
            Err(e) => {
                failed += 1;
                report.push_str(&format!("{folder}\t{}\t{}\tFAILED\t{}\t-\t-\t-\t-\t{}\n", job.name, job.stem, job.path, tsv_escape(&e)));
            }
            Ok((b, placement, icon)) => {
                if b.visuals == 0 && !b.notes.iter().any(|n| n.contains("moving") || n.contains("dyna") || n.contains("tree")) {
                    failed += 1;
                    report.push_str(&format!("{folder}\t{}\t{}\tEMPTY\t{}\t0\t0\t-\t-\t{}\n", job.name, job.stem, job.path, tsv_escape(&b.notes.join(" | "))));
                    continue;
                }
                let description = format!("Tiny Items: the Nadeo item {} at scale {}, folder {}. Its own placement settings, scaled.", job.stem, opts.scale, folder);
                match dress_item(&b.bytes, &job.ident, &job.stem, &opts.author, opts.collection, icon.as_deref(), &description, &placement, opts.scale, opts.no_skin_header) {
                    Err(e) => {
                        failed += 1;
                        report.push_str(&format!("{folder}\t{}\t{}\tDRESS-FAILED\t{}\t-\t-\t-\t-\t{}\n", job.name, job.stem, job.path, tsv_escape(&e)));
                    }
                    Ok(bytes) => {
                        let dir: PathBuf = job.folders.iter().fold(root.clone(), |d, f| d.join(f));
                        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                        let file = dir.join(format!("{}.Item.Gbx", job.stem));
                        std::fs::write(&file, &bytes).map_err(|e| format!("{}: {e}", file.display()))?;
                        for (pic, dds) in &b.pictures {
                            let pf = dir.join(pic);
                            if !pf.is_file() {
                                std::fs::write(&pf, dds).map_err(|e| format!("{}: {e}", pf.display()))?;
                            }
                        }
                        total_bytes += bytes.len();
                        let notes: Vec<&str> = b.notes.iter().filter(|n| n.contains("skipped") || n.contains("failed") || n.contains("unnamed") || n.contains("refused") || n.contains("placement")).map(|s| s.as_str()).collect();
                        if std::env::var_os("ITEMSET_NOTES").is_some() {
                            for n in &b.notes {
                                eprintln!("  [{}] {n}", job.stem);
                            }
                        }
                        report.push_str(&format!("{folder}\t{}\t{}\tOK\t{}\t{}\t{}\t{}\t{}\t{}\n", job.name, job.stem, job.path, b.visuals, b.triangles, bytes.len(), tsv_escape(&scaled_placement(&placement, opts.scale, 0).summary()), tsv_escape(&notes.join(" | "))));
                        written += 1;
                    }
                }
            }
        }
    }
    match &opts.report {
        Some(r) => std::fs::write(r, &report).map_err(|e| format!("{}: {e}", r.display()))?,
        None => {
            std::fs::create_dir_all(&opts.out).ok();
            std::fs::write(opts.out.join("item-set-items-report.tsv"), &report).ok();
        }
    }
    eprintln!("{written} items written ({:.1} MB), {failed} failed, under {} in {:.1} s", total_bytes as f64 / 1e6, root.display(), t0.elapsed().as_secs_f32());
    Ok(())
}

pub fn items_cmd(store: &mut DataStore, rest: &[String]) {
    if let Err(e) = run_items(store, rest) {
        eprintln!("item-set-items: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// item.exchange packing: one set per browser folder, ≤ N MB zipped
// ---------------------------------------------------------------------------

/// `mapgeom item-set-zips --root Items/TinyBlocks --out DIR [--max-mb 28]
/// [--skip LIST] [--prefix "Tiny Blocks - "]`: the items under `root` grouped
/// by FAMILY (the browser's second level: Roads/RoadTech, Terrain/Grass) and
/// packed in sub-folder order into zips of at most `max-mb` (paths relative
/// to the Items folder, deflate) — a family over the cap becomes `(k/n: the
/// sub-folders it holds)` parts; item.exchange refuses any request over
/// 30,000,000 bytes (2026-09-22). `--skip`
/// names a file of item paths (relative to root, one per line) already
/// published. Writes `manifest.tsv` in `out`: set name, folder, part, items,
/// zip bytes, zip file, the items' paths (`|`-joined).
pub fn zips_cmd(rest: &[String]) {
    if let Err(e) = run_zips(rest) {
        eprintln!("item-set-zips: {e}");
        std::process::exit(1);
    }
}

fn run_zips(rest: &[String]) -> Result<(), String> {
    let flag = |name: &str| rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1)).cloned();
    let root = PathBuf::from(flag("--root").ok_or("--root DIR (the Items/<set> folder) is required")?);
    let out = PathBuf::from(flag("--out").ok_or("--out DIR is required")?);
    let max_bytes: usize = (flag("--max-mb").unwrap_or_else(|| "28".into()).parse::<f64>().map_err(|e| format!("--max-mb: {e}"))? * 1_000_000.0) as usize;
    let prefix = flag("--prefix").unwrap_or_else(|| "Tiny Blocks - ".into());
    // `--group-depth 1`: sets per TOP folder (the small TinyItems tree: 9 sets
    // instead of 48 crumbs)
    let group_depth: usize = flag("--group-depth").unwrap_or_else(|| "2".into()).parse().map_err(|e| format!("--group-depth: {e}"))?;
    let skip: std::collections::HashSet<String> = match flag("--skip") {
        Some(f) => std::fs::read_to_string(&f).map_err(|e| format!("{f}: {e}"))?.lines().map(|l| l.trim().replace('\\', "/")).filter(|l| !l.is_empty()).collect(),
        None => Default::default(),
    };
    let set_name = root.file_name().and_then(|s| s.to_str()).ok_or("--root has no final component")?.to_string();
    let readme_bytes = std::fs::read(root.join("README.txt")).ok();
    fn walk(dir: &Path, root: &Path, acc: &mut Vec<(String, u64)>) -> Result<(), String> {
        for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let e = e.map_err(|e| e.to_string())?;
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, acc)?;
            } else if p.to_string_lossy().ends_with(".Item.Gbx") {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                acc.push((rel, e.metadata().map_err(|e| e.to_string())?.len()));
            }
        }
        Ok(())
    }
    let mut all = Vec::new();
    walk(&root, &root, &mut all)?;
    // by FAMILY (the browser's second level: Roads/RoadTech, Terrain/Grass, …),
    // the items in sub-folder order so a part holds whole sub-folders
    let mut by_family: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
    let mut skipped = 0usize;
    for (rel, size) in all {
        if skip.contains(&rel) {
            skipped += 1;
            continue;
        }
        let parts: Vec<&str> = rel.split('/').collect();
        let family = parts[..parts.len().saturating_sub(1).min(group_depth)].join("/");
        by_family.entry(family).or_default().push((rel, size));
    }
    for v in by_family.values_mut() {
        v.sort();
    }
    eprintln!("{} families, {} items ({skipped} skipped)", by_family.len(), by_family.values().map(|v| v.len()).sum::<usize>());
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    // the zip of a run of items (+ the pictures of their folders + the README)
    let zip_of = |items: &[(String, u64)]| -> Result<Vec<u8>, String> {
        let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut dirs_done: std::collections::HashSet<PathBuf> = Default::default();
        for (rel, _) in items {
            let p = root.join(rel);
            files.insert(format!("{set_name}/{rel}"), std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?);
            let dir = p.parent().unwrap().to_path_buf();
            if dirs_done.insert(dir.clone()) {
                for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                    let q = e.map_err(|e| e.to_string())?.path();
                    let name = q.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    if q.is_file() && !name.ends_with(".Item.Gbx") && name != "README.txt" {
                        let qrel = q.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                        files.insert(format!("{set_name}/{qrel}"), std::fs::read(&q).map_err(|e| e.to_string())?);
                    }
                }
            }
        }
        if let Some(r) = &readme_bytes {
            files.insert(format!("{set_name}/README.txt"), r.clone());
        }
        Ok(tmmaps::header::deflated_zip(&files))
    };
    // a display name for the family: the second-level folder when it names
    // its kind itself (RoadTech, PlatformDirt, TrackWall, OpenIce), else with
    // the top folder in front (Terrain Grass, Themes RallyCastle)
    let family_label = |family: &str| -> String {
        let mut it = family.split('/');
        let top = it.next().unwrap_or("");
        match it.next() {
            None => top.to_string(),
            Some(second) if second.starts_with("Road") || second.starts_with("Platform") || second.starts_with("Open") || second.ends_with("Wall") => second.to_string(),
            Some(second) => format!("{top} {second}"),
        }
    };
    let sub_of = |rel: &str| -> String {
        let parts: Vec<&str> = rel.split('/').collect();
        if parts.len() >= group_depth + 2 { parts[group_depth].to_string() } else { String::new() }
    };
    let mut manifest = String::from("set\tfolder\tpart\tparts\titems\tzip_bytes\tzip\tsubfolders\tpaths\n");
    let mut total_sets = 0usize;
    for (family, items) in &by_family {
        // greedy bins in sub-folder order: estimated at 0.62 of the LZO bytes,
        // each zip verified and split in two when it still passes the cap
        let mut bins: Vec<Vec<(String, u64)>> = Vec::new();
        let mut cur: Vec<(String, u64)> = Vec::new();
        let mut cur_est = 0.0f64;
        for it in items {
            let est = it.1 as f64 * 0.62;
            if !cur.is_empty() && cur_est + est > max_bytes as f64 * 0.97 {
                bins.push(std::mem::take(&mut cur));
                cur_est = 0.0;
            }
            cur.push(it.clone());
            cur_est += est;
        }
        if !cur.is_empty() {
            bins.push(cur);
        }
        // verify; a bin over the cap is halved until it fits
        let mut zips: Vec<(Vec<u8>, Vec<(String, u64)>)> = Vec::new();
        let mut queue: std::collections::VecDeque<Vec<(String, u64)>> = bins.into_iter().collect();
        while let Some(bin) = queue.pop_front() {
            let z = zip_of(&bin)?;
            if z.len() > max_bytes {
                if bin.len() == 1 {
                    return Err(format!("{family}: {} alone is {} bytes zipped, over the cap", bin[0].0, z.len()));
                }
                let half = bin.len() / 2;
                let (a, b) = bin.split_at(half);
                // keep the order: the second half goes right after the first
                queue.push_front(b.to_vec());
                queue.push_front(a.to_vec());
                continue;
            }
            zips.push((z, bin));
        }
        let n = zips.len();
        let label = family_label(family);
        for (k, (z, bin)) in zips.iter().enumerate() {
            let mut subs: Vec<String> = Vec::new();
            for (rel, _) in bin {
                let s = sub_of(rel);
                if !subs.contains(&s) {
                    subs.push(s);
                }
            }
            let subs_text = subs.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(", ");
            let name = if n == 1 {
                format!("{prefix}{label}")
            } else {
                let mut t = subs_text.clone();
                if t.len() > 60 {
                    t.truncate(57);
                    t.push_str("...");
                }
                if t.is_empty() { format!("{prefix}{label} ({}/{n})", k + 1) } else { format!("{prefix}{label} ({}/{n}: {t})", k + 1) }
            };
            let file = format!("{}{}.zip", family.replace('/', "-"), if n > 1 { format!("-{}of{n}", k + 1) } else { String::new() });
            std::fs::write(out.join(&file), z).map_err(|e| format!("{file}: {e}"))?;
            manifest.push_str(&format!("{name}\t{family}\t{}\t{n}\t{}\t{}\t{file}\t{}\t{}\n", k + 1, bin.len(), z.len(), tsv_escape(&subs_text), bin.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>().join("|")));
            eprintln!("{name}: {} items, {:.1} MB -> {file}", bin.len(), z.len() as f64 / 1e6);
            total_sets += 1;
        }
    }
    std::fs::write(out.join("manifest.tsv"), &manifest).map_err(|e| e.to_string())?;
    eprintln!("{total_sets} sets -> {}", out.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Set screenshots: one map per set, its items in a grid (`mapgeom item-set-shoot`)
// ---------------------------------------------------------------------------

/// `mapgeom item-set-shoot --manifest DIR/manifest.tsv --report REPORT.tsv --host HOST.Map.Gbx
/// --out DIR [--author ID] [--origin 400,8,400] [--max-items 30] [--set-name TinyBlocks]`: for every set of the
/// manifest a map with the set's items laid out in a grid on the block-free
/// host (every host item record parked far away, one re-pointed per item —
/// `item_set_map`'s method), written as `DIR/<zip stem>.Map.Gbx`, and
/// `DIR/cams.tsv`: set name, map file, the editor camera (target, distance,
/// h, v — the probe plugin's cam.txt spec) framing the whole grid, the grid
/// size. The footprint of every item comes from the bake report (column
/// `footprint`, `SxSz` in tiny units): the grid pitch is the set's largest
/// footprint plus a gap, so pieces never overlap.
pub fn shoot_cmd(rest: &[String]) {
    if let Err(e) = run_shoot(rest) {
        eprintln!("item-set-shoot: {e}");
        std::process::exit(1);
    }
}

fn run_shoot(rest: &[String]) -> Result<(), String> {
    let flag = |name: &str| rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1)).cloned();
    let manifest = PathBuf::from(flag("--manifest").ok_or("--manifest FILE is required")?);
    let report = PathBuf::from(flag("--report").ok_or("--report FILE (the bake report) is required")?);
    let host = PathBuf::from(flag("--host").ok_or("--host MAP is required")?);
    let out = PathBuf::from(flag("--out").ok_or("--out DIR is required")?);
    let author = flag("--author").unwrap_or_else(|| crate::tiny_assets::AUTHOR.to_string());
    // the set folder under Items/ the idents start with (`TinyBlocks\Roads\…`)
    let set_name = flag("--set-name").unwrap_or_else(|| "TinyBlocks".into());
    let max_items: usize = flag("--max-items").unwrap_or_else(|| "30".into()).parse().map_err(|e| format!("--max-items: {e}"))?;
    let origin: Vec<f32> = flag("--origin").unwrap_or_else(|| "400,8,400".into()).split(',').filter_map(|v| v.parse().ok()).collect();
    if origin.len() != 3 {
        return Err("--origin needs x,y,z".into());
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    // footprints by item path (relative to the set root), from the report
    let mut footprint: std::collections::HashMap<String, (u32, u32)> = Default::default();
    let text = std::fs::read_to_string(&report).map_err(|e| format!("{}: {e}", report.display()))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    // the item report (TinyItems) has no footprint column: every piece
    // counts as 2x2 there (Nadeo's items are up to ~30 m)
    let (c_folder, c_stem, c_status) = (col("folder").ok_or("report: no folder column")?, col("stem").ok_or("report: no stem column")?, col("status").ok_or("report: no status column")?);
    let c_fp = col("footprint");
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() <= c_status.max(c_stem) || f[c_status] != "OK" {
            continue;
        }
        let (sx, sz) = match c_fp {
            Some(c) if f.len() > c => f[c].split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).unwrap_or((1, 1)),
            _ => (2, 2),
        };
        footprint.insert(format!("{}/{}.Item.Gbx", f[c_folder], f[c_stem]), (sx, sz));
    }
    let mtext = std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let mut mlines = mtext.lines();
    let mhead: Vec<&str> = mlines.next().unwrap_or("").split('\t').collect();
    let mcol = |name: &str| mhead.iter().position(|h| *h == name);
    let (c_set, c_zip, c_paths) = (mcol("set").ok_or("manifest: no set column")?, mcol("zip").ok_or("manifest: no zip column")?, mcol("paths").ok_or("manifest: no paths column")?);
    let set_root = manifest.parent().and_then(|p| p.parent()).map(|_| ()).unwrap_or(());
    let _ = set_root;
    let mut cams = String::from("set\tmap\tcamera\tgrid\titems\n");
    let base = tmmaps::map::MapFile::load(&host);
    let host_items = base.items.len();
    drop(base);
    for l in mlines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() <= c_paths {
            continue;
        }
        let set = f[c_set];
        let zip = f[c_zip];
        let all_paths: Vec<&str> = f[c_paths].split('|').filter(|p| !p.is_empty()).collect();
        if all_paths.is_empty() {
            continue;
        }
        // a readable picture shows at most `max_items` pieces, sampled evenly
        // through the set (every sub-folder gets its share)
        let paths: Vec<&str> = if all_paths.len() <= max_items {
            all_paths.clone()
        } else {
            (0..max_items).map(|k| all_paths[k * all_paths.len() / max_items]).collect()
        };
        // the set folder name is the first component of the ident
        let set_dir = paths[0].split('/').next().unwrap_or("TinyBlocks");
        let _ = set_dir;
        let n = paths.len();
        // a wide grid for a 16:9 frame
        let cols = ((n as f64) * 1.6).sqrt().ceil().max(1.0) as usize;
        let rows = (n + cols - 1) / cols;
        // a variable grid: the pieces sorted big to small, every column as
        // wide as its widest piece and every row as deep as its deepest (16 m
        // per tiny unit) plus an 8 m gap — a 4x4 loop and a 1x1 straight in one
        // set no longer put 72 m between every pair of straights
        let mut paths: Vec<&str> = paths;
        paths.sort_by_key(|p| { let (sx, sz) = footprint.get(*p).copied().unwrap_or((1, 1)); std::cmp::Reverse(sx.max(sz) * 8 + sx + sz) });
        let gap = 8.0f32;
        let mut col_w = vec![0f32; cols];
        let mut row_d = vec![0f32; rows];
        for (i, p) in paths.iter().enumerate() {
            let (sx, sz) = footprint.get(*p).copied().unwrap_or((1, 1));
            let (r, c) = (i / cols, i % cols);
            col_w[c] = col_w[c].max(sx as f32 * 16.0 + gap);
            row_d[r] = row_d[r].max(sz as f32 * 16.0 + gap);
        }
        let col_x: Vec<f32> = col_w.iter().scan(0.0f32, |acc, w| { let x = *acc; *acc += w; Some(x) }).collect();
        let row_z: Vec<f32> = row_d.iter().scan(0.0f32, |acc, d| { let z = *acc; *acc += d; Some(z) }).collect();
        let total_w: f32 = col_w.iter().sum();
        let total_d: f32 = row_d.iter().sum();
        // CLONED records for every piece (uniform, from one donor): the host's own
        // records carry per-placement state (2026-09-23: re-pointed U10S records
        // collided only for the first one). The item array grows only in the
        // written file: grow, write, reload.
        let stem = zip.trim_end_matches(".zip");
        let grown = out.join(format!("{stem}.grown.Map.Gbx"));
        let mut m = tmmaps::map::MapFile::load(&host);
        let first = m.items.len();
        m.append_item_clones(first + n);
        m.write_to(&grown).map_err(|e| format!("{}: {e}", grown.display()))?;
        m = tmmaps::map::MapFile::load(&grown);
        if m.items.len() < first + n {
            return Err(format!("{set}: the host grew to {} item records, {} needed", m.items.len(), first + n));
        }
        m.set_map_uid(&format!("Sho{:024}", (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u128 + zip.len() as u128) % 10u128.pow(24)));
        for i in 0..m.items.len() {
            m.move_item_pos(i, [16.0, -1000.0, 16.0]);
        }
        let cell = |p: [f32; 3]| ((p[0] / 32.0) as i32, ((p[1] + 64.0) / 8.0) as i32, (p[2] / 32.0) as i32);
        for (k, p) in paths.iter().enumerate() {
            let i = first + k;
            let (sx, sz) = footprint.get(*p).copied().unwrap_or((1, 1));
            let (r, c) = (k / cols, k % cols);
            // the placement rule (see `dress`): geometry at P + R(p + V), so at
            // yaw 0 the piece's corner sits at P + V — the cell's corner less the
            // pivot, the piece centred in its cell
            let pivot = [sx as f32 * 8.0, 0.0, sz as f32 * 8.0];
            let corner = [origin[0] + col_x[c] + (col_w[c] - gap - sx as f32 * 16.0) / 2.0, origin[1], origin[2] + row_z[r] + (row_d[r] - gap - sz as f32 * 16.0) / 2.0];
            let pos = [corner[0] - pivot[0], corner[1] - pivot[1], corner[2] - pivot[2]];
            let ident = format!("{set_name}\\{}", p.replace('/', "\\"));
            m.move_item(i, pos, 0.0, cell(pos));
            m.set_item_frame(i, [0.0, 0.0, 0.0], pivot);
            m.set_item_scale(i, 1.0);
            m.set_item_model(i, &ident);
            m.set_item_author(i, &author);
            m.clear_item_variant(i);
        }
        let file = out.join(format!("{stem}.Map.Gbx"));
        let stage = out.join(format!("{stem}.stage.Map.Gbx"));
        m.write_to(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
        let mut m2 = tmmaps::map::MapFile::load(&stage);
        m2.remove_password();
        m2.write_to(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let _ = std::fs::remove_file(&stage);
        let _ = std::fs::remove_file(&grown);
        // the camera: the grid centre, from the south-east and above, far
        // enough that the whole grid fits a 16:9 frame
        let w = total_w;
        let d = total_d;
        let centre = [origin[0] + w / 2.0, origin[1], origin[2] + d / 2.0];
        // the grid fills ~70 % of the frame width (the picture is cropped to
        // its centre afterwards: the editor UI sits at the top and bottom)
        let extent = w.max(d * 1.4);
        let dist = (extent * 0.62 + 10.0).max(30.0);
        cams.push_str(&format!("{set}\t{}\t{:.1},{:.1},{:.1},{:.1},{:.3},{:.3}\t{cols}x{rows}\t{n}\n", file.file_name().unwrap().to_string_lossy(), centre[0], centre[1], centre[2], dist, 2.356, 0.62));
        eprintln!("{set}: {n} items in {cols}x{rows}, {total_w:.0} x {total_d:.0} m -> {}", file.display());
    }
    let _ = host_items;
    std::fs::write(out.join("cams.tsv"), &cams).map_err(|e| e.to_string())?;
    Ok(())
}
