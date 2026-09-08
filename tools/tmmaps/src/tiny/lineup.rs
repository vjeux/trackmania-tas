//! Test maps for the render box: `tmmaps tiny-catalog` (every library item
//! once, in a grid) and `tmmaps lineup` (a row of stock and embedded items on
//! a host map, for A/B shots).

use super::*;

/// Block-by-block verification map. For every distinct authored block model of
/// the source map, one ORIGINAL block is kept and moved onto a grid cell; its
/// generated item is placed beside it at scale 1 (+64 m in x) and again at the
/// requested scale (+112 m in x). Every other block is parked, every original
/// item is moved out of sight. A TSV of the grid (name, alias, cell, positions)
/// is written next to the map for camera planning and for reading the shots.
///
///   tmmaps tiny-catalog SRC.Map.Gbx --mapping placements.tsv --library ITEMS.zip
///       --out CATALOG.Map.Gbx [--scale 0.5] [--cols 7] [--host HOST.Map.Gbx]
pub fn catalog_cmd(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(cli::flag(args, "--out").expect("tiny-catalog needs --out MAP"));
    let mapping = read_mapping(&PathBuf::from(cli::flag(args, "--mapping").expect("--mapping FILE.tsv")));
    let library = PathBuf::from(cli::flag(args, "--library").expect("--library ITEMS.zip"));
    let scale: f32 = cli::flag(args, "--scale").unwrap_or("0.5").parse().expect("--scale");
    let cols: i32 = cli::flag(args, "--cols").unwrap_or("7").parse().expect("--cols");
    let host: Option<PathBuf> = cli::flag(args, "--host").map(PathBuf::from);
    let only: Option<String> = cli::flag(args, "--only").map(String::from);
    let source = MapFile::load(&src);
    set_ground(source.items.first().map(|it| it.collection_raw).unwrap_or(26));

    // one representative per block name, grid-placed blocks only
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut reps: Vec<usize> = Vec::new();
    for b in &source.blocks {
        if b.flags & FREE_BLOCK_FLAG != 0 {
            continue;
        }
        if !mapping.by_index.contains_key(&b.index) && !mapping.by_name.contains_key(&b.name) {
            continue;
        }
        if let Some(o) = &only {
            if !o.split(',').any(|n| n == b.name) {
                continue;
            }
        }
        if seen.insert(b.name.clone()) {
            reps.push(b.index);
        }
    }
    println!("  {} block models to verify", reps.len());

    // grid: 6 cells per column (192 m), 3 cells per row (96 m)
    let col_cells = 6;
    let row_cells = 3;
    let mut grid: Vec<(usize, (i32, i32, i32))> = Vec::new();
    for (k, &bi) in reps.iter().enumerate() {
        let col = k as i32 % cols;
        let row = k as i32 / cols;
        let (_, cy, _) = source.blocks[bi].coords();
        // start well inside the grid: the map edge is the decoration's scenery
        let origin: i32 = cli::flag(args, "--origin-cell").unwrap_or("20").parse().expect("--origin-cell");
        let raise: i32 = cli::flag(args, "--raise").unwrap_or("0").parse().expect("--raise cells");
        grid.push((bi, (origin + col * col_cells, cy + raise, origin + row * row_cells)));
    }

    let mut specs: Vec<Spec> = Vec::new();
    let mut ref_authors: BTreeMap<String, String> = BTreeMap::new();
    let mut tsv = String::from("name\talias\tcell_x\tcell_y\tcell_z\tblock_x\tblock_y\tblock_z\titem1_x\titem1_z\titem_scaled_x\titem_scaled_z\n");
    for &(bi, cell) in &grid {
        let b = &source.blocks[bi];
        let map = mapping.by_index.get(&bi).or_else(|| mapping.by_name.get(&b.name)).unwrap();
        // the block's origin once moved: recompute from the new cell
        let mut moved = b.clone();
        moved.file_cell = [(cell.0 + 1) as u8, cell.1 as u8, (cell.2 + 1) as u8];
        let origin = match map.footprint {
            Some(fp) => block_origin(&moved, fp),
            None => block_pos(&moved),
        };
        let rot = [block_yaw(b), 0.0, 0.0];
        // --geometry-scaled: slot C uses the AS-prefixed library twin whose
        // mesh already carries the scale (the game ignores placement scale).
        let geom_scaled = args.iter().any(|a| a == "--geometry-scaled");
        // --ref-item F[,G...]: foreign item files, under their OWN idents
        // and authors, placed at +160 m, +208 m, ... Falls through to the
        // normal slots below (side-by-side needs both).
        if let Some(refitems) = cli::flag(args, "--ref-item") {
            for (ri, refitem) in refitems.split(',').enumerate() {
                let bytes = std::fs::read(refitem).unwrap_or_else(|e| panic!("--ref-item {refitem}: {e}"));
                let (ident, author) = crate::header::item_ident_author(&bytes).expect("item header ident");
                let pos = [origin[0] + 160.0 + 48.0 * ri as f32, origin[1], origin[2]];
                specs.push(Spec { model: ident.clone(), pos, yaw: rot[0], frame: Some((rot, [0.0, 0.0, 0.0])), scale: 1.0, tag: None, order: 0, color: 1 });
                ref_authors.insert(ident, author);
            }
        }
        // --lineup F[,G...]: the block alone, then each listed item file
        // under its OWN ident and author at +64 m, +112 m, ... (48 m pitch),
        // no library slots: "original block, our tiny, his tiny" in one frame.
        if let Some(files) = cli::flag(args, "--lineup") {
            for (ri, file) in files.split(',').enumerate() {
                let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("--lineup {file}: {e}"));
                let (ident, author) = crate::header::item_ident_author(&bytes).expect("item header ident");
                let pos = [origin[0] + 64.0 + 48.0 * ri as f32, origin[1], origin[2]];
                specs.push(Spec { model: ident.clone(), pos, yaw: rot[0], frame: Some((rot, [0.0, 0.0, 0.0])), scale: 1.0, tag: None, order: 0, color: 1 });
                ref_authors.insert(ident, author);
            }
            let bp = block_pos(&moved);
            tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\tlineup {} at x+64/+112/...\n", b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2], files));
            continue;
        }
        if args.iter().any(|a| a == "--overlay") {
            // the scale-1 item exactly on the block: mismatches peek out.
            // --yaw-offset DEG turns the item relative to the block's yaw.
            let off: f32 = cli::flag(args, "--yaw-offset").unwrap_or("0").parse::<f32>().expect("--yaw-offset deg").to_radians();
            let rot = [rot[0] + off, rot[1], rot[2]];
            // --overlay-lift M raises the item a little so a coplanar match
            // reads as covered instead of z-fighting with the block.
            let lift: f32 = cli::flag(args, "--overlay-lift").unwrap_or("0").parse().expect("--overlay-lift m");
            let pos = [origin[0], origin[1] + lift, origin[2]];
            specs.push(Spec { model: map.model.clone(), pos, yaw: rot[0], frame: Some((rot, [0.0, 0.0, 0.0])), scale: 1.0 / map.model_scale, tag: None, order: 0, color: 1 });
            let bp = block_pos(&moved);
            tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\toverlay\n", b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2]));
            continue;
        }
        if args.iter().any(|a| a == "--yaw-sweep") {
            // four copies of the scale-1 item at yaw 0, 90, 180, 270 degrees
            for k in 0..4 {
                let pos = [origin[0] + 64.0 + 48.0 * k as f32, origin[1], origin[2]];
                let yaw = k as f32 * std::f32::consts::FRAC_PI_2;
                specs.push(Spec { model: map.model.clone(), pos, yaw, frame: Some(([yaw, 0.0, 0.0], [0.0, 0.0, 0.0])), scale: 1.0 / map.model_scale, tag: None, order: 0, color: 1 });
            }
            let bp = block_pos(&moved);
            tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\tsweep yaw 0/90/180/270 at x+64/+112/+160/+208\n", b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2]));
            continue;
        }
        for (dx, s) in [(64.0f32, 1.0f32), (112.0, scale)] {
            let pos = [origin[0] + dx, origin[1], origin[2]];
            let (model, s) = if geom_scaled && s != 1.0 {
                (map.model.replacen("AC", "AS", 1), 1.0)
            } else {
                (map.model.clone(), s / map.model_scale)
            };
            specs.push(Spec {
                model,
                pos,
                yaw: rot[0],
                frame: Some((rot, [0.0, 0.0, 0.0])),
                scale: s,
                tag: None,
                order: 0,
                color: 1,
            });
        }
        let bp = block_pos(&moved);
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\n",
            b.name, map.model, cell.0, cell.1, cell.2, bp[0], bp[1], bp[2], origin[0] + 64.0, origin[2], origin[0] + 112.0, origin[2]
        ));
    }

    let base = host.clone().unwrap_or_else(|| src.clone());
    // --stock A,B,C: stock (pack) items by name — vegetation species — in a
    // row 40 m in front of the first block, 16 m apart, standing on the
    // block's deck level, under author Nadeo: a size-and-colour survey of a
    // collection's trees in one frame (GreenCoast has 45 species).
    if let Some(list) = cli::flag(args, "--stock") {
        let (bx, by, bz) = grid.first().map(|g| g.1).unwrap_or((20, 5, 20));
        let base_pos = [bx as f32 * crate::map::CELL_XZ, by as f32 * crate::map::CELL_Y + ground() + 2.0, bz as f32 * crate::map::CELL_XZ - 40.0];
        for (k, name) in list.split(',').filter(|s| !s.is_empty()).enumerate() {
            let pos = [base_pos[0] + 16.0 * k as f32, base_pos[1], base_pos[2]];
            specs.push(Spec { model: name.to_string(), pos, yaw: 0.0, frame: Some(([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])), scale: 1.0, tag: None, order: 0, color: 0 });
            ref_authors.insert(name.to_string(), "Nadeo".to_string());
            tsv.push_str(&format!("stock\t{name}\t\t\t\t{:.0}\t{:.0}\t{:.0}\n", pos[0], pos[1], pos[2]));
        }
    }
    let tmp0 = out.with_extension("cat0.Map.Gbx");
    let tmp1 = out.with_extension("cat1.Map.Gbx");
    let tmp2 = out.with_extension("cat2.Map.Gbx");
    let mut m = MapFile::load(&base);
    let n_existing = m.items.len();
    m.append_item_clones(n_existing + specs.len());
    m.write_to(&tmp0).expect("write slots");

    let mut m = MapFile::load(&tmp0);
    if host.is_none() {
        // Every block but the representatives and the zone (terrain) tiles is
        // DELETED — not parked in cell (0,0,0): a pile of blocks in one cell is
        // what cost Summer 05 five and a half minutes in the game's lightmapper
        // (2026-09-07), and 2 936 Lake tiles stacked there left the GreenCoast
        // editor view solid white (Summer 04, 2026-09-06). The island stays as
        // scenery for the survey. The deletion is variable-length and wants a
        // fresh load, so it comes first and the map is reloaded before the
        // uid, the moves and the item edits.
        let keep: BTreeSet<usize> = grid.iter().map(|g| g.0).collect();
        let zones: BTreeSet<String> = source.genealogy_zones().into_iter().collect();
        let kept_idx: Vec<usize> = m.blocks.iter().filter(|b| keep.contains(&b.index) || zones.contains(&b.name)).map(|b| b.index).collect();
        let r = m.remove_blocks(|b| !keep.contains(&b.index) && !zones.contains(&b.name), |_| false);
        println!("  {} blocks deleted ({} kept: {} representatives + zone tiles)", r.blocks, kept_idx.len(), keep.len());
        let tmpd = out.with_extension("catd.Map.Gbx");
        m.write_to(&tmpd).expect("write block-deletion stage");
        m = MapFile::load(&tmpd);
        assert_eq!(m.blocks.len(), kept_idx.len(), "block count after the deletion");
        for (new_i, &old_i) in kept_idx.iter().enumerate() {
            if let Some(&(_, cell)) = grid.iter().find(|g| g.0 == old_i) {
                m.move_block_cell(new_i, cell);
            }
        }
    }
    // Fresh UID per build: the game caches embedded items and lightmaps by
    // map UID, so reusing the host's UID shows stale items (empty grass
    // where new items should be -- graft tests 2026-09-05).
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    m.set_map_uid(&format!("Cat1{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000));
    // existing items out of sight
    for i in 0..n_existing {
        m.move_item(i, [8.0, -900.0, 8.0], 0.0, (0, 0, 0));
    }
    m.write_to(&tmp1).expect("write block stage");

    let mut m = MapFile::load(&tmp1);
    for (k, s) in specs.iter().enumerate() {
        let i = n_existing + k;
        m.set_item_model(i, &s.model);
        m.set_item_author(i, ref_authors.get(&s.model).map(|a| a.as_str()).unwrap_or(&s.model));
        m.move_item(i, s.pos, s.yaw, cell_for(s.pos));
        if let Some((rot, pivot)) = s.frame {
            m.set_item_frame(i, rot, pivot);
        }
        m.set_item_scale(i, s.scale);
        m.clear_item_variant(i);
        m.set_item_color(i, s.color);
    }
    m.write_to(&tmp2).expect("write models");

    let mut m = MapFile::load(&tmp2);
    m.remove_password();
    let mut zip = std::fs::read(&library).unwrap_or_else(|e| panic!("{}: {e}", library.display()));
    // Foreign items claim the MAP's collection inside (header + body idents):
    // a BlueBay map drops a Stadium-collection item silently (Lineup7,
    // 2026-09-06: both tiny items absent, no dialog, probe listed none).
    let map_collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
    for flag in ["--ref-item", "--lineup"] {
        if let Some(refitems) = cli::flag(args, flag) {
            for refitem in refitems.split(',') {
                let bytes = std::fs::read(refitem).unwrap();
                let (ident, _) = crate::header::item_ident_author(&bytes).unwrap();
                let bytes = crate::header::set_ident_collection(&bytes, map_collection);
                zip = crate::header::zip_add(&zip, &format!("Items/{ident}"), &bytes); // zip_add re-emits deflated
            }
        }
    }
    let mut names: Vec<String> = specs.iter().map(|s| s.model.clone()).filter(|n| n.ends_with(".Item.Gbx")).collect();
    names.sort();
    names.dedup();
    let manifest: Vec<(&str, &str)> = names
        .iter()
        .map(|n| (n.as_str(), ref_authors.get(n).map(|a| a.as_str()).unwrap_or(n.as_str())))
        .collect();
    m.replace_embedded_objects(&manifest, &zip);
    m.write_to(&out).expect("write output");
    for p in [&tmp0, &tmp1, &tmp2] {
        let _ = std::fs::remove_file(p);
    }
    if host.is_none() {
        // no regenerated island under the grid
        let _ = MapFile::clear_genealogy_file(&out);
    }
    let tsv_path = out.with_extension("grid.tsv");
    std::fs::write(&tsv_path, tsv).unwrap();
    println!("wrote {} ({} blocks x [original, item x1, item x{}]); grid {}", out.display(), grid.len(), scale, tsv_path.display());
}

/// `tmmaps lineup MAP --out F --stock A,B,C --at X,Y,Z [--pitch M]
/// [--items F.Item.Gbx,G.Item.Gbx]`: the map unchanged plus a row of STOCK
/// (pack) items by name — vegetation species — starting at X,Y,Z, `pitch`
/// metres apart along +x, under author Nadeo. A species survey in one frame
/// (which SpringTree is green, which is pink), on the real map so the editor
/// renders it (a parked-block catalog map came out blank in GreenCoast).
/// `--items` continues the row with EMBEDDED item files under their own ident
/// and author (re-stamped to the map's collection): a stock `Lamp` next to
/// our baked lamp on a night map is the oracle for the lights work.
pub fn lineup_cmd(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(cli::flag(args, "--out").expect("lineup needs --out MAP"));
    let list = cli::flag(args, "--stock").unwrap_or("");
    let at = vec3(&cli::flag(args, "--at").expect("lineup needs --at X,Y,Z"), "--at");
    let pitch: f32 = cli::flag(args, "--pitch").unwrap_or("16").parse().expect("--pitch metres");
    let step: [f32; 3] = cli::flag(args, "--step").map(|s| vec3(s, "--step")).unwrap_or([pitch, 0.0, 0.0]);
    // --yaw R turns every item of the row (radians): a pusher's piston runs
    // along its local z, so pi/2 makes it run along the row, visible from the north
    let yaw: f32 = cli::flag(args, "--yaw").unwrap_or("0").parse().expect("--yaw radians");
    // a stock name may carry a variant: `ShowLights@23` = the Light4Spots entry
    let mut variants: Vec<u8> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for s in list.split(',').filter(|s| !s.is_empty()) {
        let (name, v) = match s.split_once('@') {
            Some((n, v)) => (n.to_string(), v.parse::<u8>().unwrap_or_else(|_| panic!("--stock {s}: variant is not a byte"))),
            None => (s.to_string(), 0),
        };
        names.push(name);
        variants.push(v);
    }
    // --place "x,y,z,yaw[,pitch,roll];…": one pose per item instead of the row
    // (the play-mode collision test: pushers on four sides of the spawn; a
    // fogger hanging from a show rig is yaw −π/2, pitch −π — 2026-09-08)
    let places: Vec<[f32; 6]> = cli::flag(args, "--place")
        .map(|s| {
            s.split(';')
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let v: Vec<f32> = p.split(',').map(|x| x.trim().parse().expect("--place x,y,z,yaw[,pitch,roll]")).collect();
                    assert!(v.len() == 4 || v.len() == 6, "--place wants x,y,z,yaw or x,y,z,yaw,pitch,roll per item");
                    [v[0], v[1], v[2], v[3], v.get(4).copied().unwrap_or(0.0), v.get(5).copied().unwrap_or(0.0)]
                })
                .collect()
        })
        .unwrap_or_default();
    // --colors 2,3,5,…: the placement colour byte of each item in row order
    // (0 Default 1 White 2 Green 3 Blue 4 Red 5 Black); an item past the list
    // keeps the default (stock 0, embedded 1). A stock flag at Green next to
    // ours at Green is the hue-mask oracle.
    // --scales 1,1.001,…: one placement scale per item of the row (1 past the
    // list) — does a scale of its own keep an item out of the game's
    // instanced draw of identical placements? (2026-09-07)
    let scales: Vec<f32> = cli::flag(args, "--scales")
        .map(|s| s.split(',').filter(|c| !c.is_empty()).map(|c| c.trim().parse::<f32>().expect("--scales wants floats")).collect())
        .unwrap_or_default();
    let colors: Vec<u8> = cli::flag(args, "--colors")
        .map(|s| s.split(',').filter(|c| !c.is_empty()).map(|c| c.trim().parse::<u8>().expect("--colors wants bytes 0..5")).collect())
        .unwrap_or_default();
    let n_stock = names.len();
    // embedded item files: (ident, author, bytes)
    let mut embedded: Vec<(String, String, Vec<u8>)> = Vec::new();
    if let Some(files) = cli::flag(args, "--items") {
        for file in files.split(',').filter(|s| !s.is_empty()) {
            // `F.Item.Gbx*8`: the same embedded item eight times in the row
            // (one pose each with --place) — a distance ladder of one model
            let (file, copies) = match file.rsplit_once('*') {
                Some((f, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => (f, n.parse::<usize>().unwrap().max(1)),
                _ => (file, 1),
            };
            let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("--items {file}: {e}"));
            let (ident, author) = crate::header::item_ident_author(&bytes).unwrap_or_else(|| panic!("--items {file}: no item header ident"));
            for _ in 0..copies {
                names.push(ident.clone());
            }
            embedded.push((ident, author, bytes));
        }
    }
    if names.is_empty() {
        panic!("lineup needs --stock A,B,C and/or --items F.Item.Gbx");
    }
    let source = MapFile::load(&src);
    set_ground(source.items.first().map(|it| it.collection_raw).unwrap_or(26));
    let map_collection = source.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let n = source.items.len();
    let tmp0 = out.with_extension("lineup0.Map.Gbx");
    let mut m = MapFile::load(&src);
    m.append_item_clones(n + names.len());
    m.write_to(&tmp0).expect("write slots");
    let mut m = MapFile::load(&tmp0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    m.set_map_uid(&format!("Lin1{:08X}{:07}{:08X}", nanos % 100_000_000, std::process::id() % 10_000_000, (nanos / 7) % 100_000_000));
    for (k, name) in names.iter().enumerate() {
        let i = n + k;
        // --step X,Y,Z: the offset between two items (default pitch along +x)
        let (pos, yaw, pitch, roll) = match places.get(k) {
            Some(p) => ([p[0], p[1], p[2]], p[3], p[4], p[5]),
            None => ([at[0] + step[0] * k as f32, at[1] + step[1] * k as f32, at[2] + step[2] * k as f32], yaw, 0.0, 0.0),
        };
        m.set_item_model(i, name);
        let author = embedded.iter().find(|(id, _, _)| id == name).map(|(_, a, _)| a.as_str()).unwrap_or("Nadeo");
        m.set_item_author(i, author);
        m.move_item(i, pos, yaw, cell_for(pos));
        // the appended record is a byte copy of a donor item: its pivot (and
        // pitch/roll) come along, and the game puts the PIVOT at `pos` —
        // a donor pivot of a few metres turned every yawed pusher of the
        // play-mode tests 3-4 m sideways of the car (2026-09-07)
        m.set_item_frame(i, [yaw, pitch, roll], [0.0; 3]);
        m.set_item_scale(i, scales.get(k).copied().unwrap_or(1.0));
        m.set_item_variant(i, variants.get(k).copied().unwrap_or(0));
        let color = colors.get(k).copied().unwrap_or(if k < n_stock { 0 } else { 1 });
        m.set_item_color(i, color);
        println!("  {name} ({author}) at {:.0},{:.0},{:.0} colour {color}", pos[0], pos[1], pos[2]);
    }
    let tmp1 = out.with_extension("lineup1.Map.Gbx");
    m.write_to(&tmp1).expect("write models");
    // variable-length splices (the password chunk) only after a write+reload
    let mut m = MapFile::load(&tmp1);
    m.remove_password();
    // --skin K=PATH,…: the K-th item of the row (0-based) gets a placement skin
    // (`Skins\Any\Advertisement6x1\X.png` — a file put into the archive with
    // --extra, or one of the game's). The 2026-09-07 probe: does an explicit
    // skin reach an embedded item's screen where the default advertisement
    // did not?
    if let Some(list) = cli::flag(args, "--skin") {
        for entry in list.split(',').filter(|s| !s.is_empty()) {
            let (k, path) = entry.split_once('=').unwrap_or_else(|| panic!("--skin wants K=Skins\\…, got {entry:?}"));
            let k: usize = k.parse().unwrap_or_else(|_| panic!("--skin: {k:?} is not a row index"));
            assert!(k < names.len(), "--skin {k}: the row has {} items", names.len());
            let mut checksum = [0u8; 32];
            checksum[0] = 2; // what the game writes for its own skins (Summer 15: every LightColors ref)
            let f = crate::header::FileRef { version: 3, checksum, path: path.to_string(), url: String::new() };
            m.set_item_skin(n + k, Some(&f));
            println!("  skin on {} ({}): {path}", names[k], n + k);
        }
    }
    // --phases P0,P1,…: the anchored object's animation phase word (chunk
    // 0x03101005, 4 on every Summer placement) per item of the row — the
    // 2026-09-08 probe of whether it de-synchronises two kinematic pushers
    if let Some(list) = cli::flag(args, "--phases") {
        for (k, p) in list.split(',').filter(|s| !s.is_empty()).enumerate() {
            let p: u32 = p.trim().parse().unwrap_or_else(|_| panic!("--phases wants integers, got {p:?}"));
            let ok = m.set_item_anim_phase(n + k, p);
            println!("  anim phase {p} on row item {k} ({}){}", n + k, if ok { "" } else { " — record has no 0x03101005 chunk" });
        }
    }
    if !embedded.is_empty() {
        // the same file placed several times is ONE archive entry / manifest row
        let mut seen: Vec<&str> = Vec::new();
        let unique: Vec<&(String, String, Vec<u8>)> = embedded.iter().filter(|(id, _, _)| if seen.contains(&id.as_str()) { false } else { seen.push(id.as_str()); true }).collect();
        // a map that already embeds items (a TINY map) keeps them: its archive is
        // the base the new items are added to, its entries stay in the manifest
        // (their author is their ident, as tiny-library writes them)
        let (mut zip, existing): (Vec<u8>, Vec<String>) = crate::header::embedded_zip_bytes(&m.gbx.body).unwrap_or_default();
        for (ident, _, bytes) in &unique {
            let bytes = crate::header::set_ident_collection(bytes, map_collection);
            zip = crate::header::zip_add(&zip, &format!("Items/{ident}"), &bytes);
        }
        // --extra ARCHIVE/PATH=LOCAL,…: more files into the map's archive next
        // to the items (a texture an item names by path, a skin zip a
        // placement points at) — the 2026-09-07 probe of what an embedded
        // item can reach inside its own archive.
        if let Some(list) = cli::flag(args, "--extra") {
            for entry in list.split(',').filter(|s| !s.is_empty()) {
                let (name, local) = entry.split_once('=').unwrap_or_else(|| panic!("--extra wants ARCHIVE/PATH=LOCALFILE, got {entry:?}"));
                let bytes = std::fs::read(local).unwrap_or_else(|e| panic!("--extra {local}: {e}"));
                zip = crate::header::zip_add(&zip, name, &bytes);
                println!("  archive file {name} ({} bytes)", bytes.len());
            }
        }
        let kept: Vec<String> = existing.iter().filter(|n| n.to_ascii_lowercase().ends_with(".item.gbx")).map(|n| n.rsplit(['/', '\\']).next().unwrap_or(n).to_string()).collect();
        let mut manifest: Vec<(&str, &str)> = kept.iter().map(|n| (n.as_str(), n.as_str())).collect();
        manifest.extend(unique.iter().map(|(id, a, _)| (id.as_str(), a.as_str())));
        if !kept.is_empty() {
            println!("  kept {} embedded items of the source map", kept.len());
        }
        m.replace_embedded_objects(&manifest, &zip);
    }
    m.write_to(&out).expect("write output");
    let _ = std::fs::remove_file(&tmp0);
    let _ = std::fs::remove_file(&tmp1);
    println!("wrote {} ({} stock + {} embedded items in a row)", out.display(), n_stock, embedded.len());
}
