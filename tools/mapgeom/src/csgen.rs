//! `mapgeom csgen --out DIR [--paks-label TEXT]` — Hugo's generated C# (2026-10-27 02:34: "generate C# block classes
//! from the pak files so I don't pass around ids"): `Blocks.g.cs` (one `Block` record per block NAME across the 5
//! collection paks) and `Items.g.cs` (one `Item` per item of the paks' item libraries), namespace
//! `tman.src.Inventory.Generated`, a single header comment line.

use std::collections::{BTreeMap, BTreeSet};

const COLLECTIONS: [(&str, u32, u32); 5] = [("Stadium", 0x1a, 1), ("RedIsland", 0x10, 2), ("WhiteShore", 0x1d, 4), ("GreenCoast", 0xf, 8), ("BlueBay", 0x1c, 16)];

fn waypoint_cs(t: Option<i32>) -> &'static str {
    match t {
        Some(0) => "Waypoint.Start",
        Some(1) => "Waypoint.Finish",
        Some(2) => "Waypoint.Checkpoint",
        Some(4) => "Waypoint.StartFinish",
        _ => "Waypoint.None",
    }
}

fn collections_cs(mask: u32) -> String {
    if mask == 31 { return "Collections.All".into(); }
    let v: Vec<String> = COLLECTIONS.iter().filter(|(_, _, bit)| mask & bit != 0).map(|(n, _, _)| format!("Collections.{n}")).collect();
    if v.is_empty() { "0".into() } else { v.join(" | ") }
}

/// A C# identifier from a game id: letters / digits / `_` kept, anything else `_`, a leading digit prefixed `_`,
/// C# keywords suffixed `_`.
pub fn identifier(id: &str) -> (String, bool) {
    const KEYWORDS: [&str; 79] = ["abstract", "as", "base", "bool", "break", "byte", "case", "catch", "char", "checked", "class", "const", "continue", "decimal", "default", "delegate", "do", "double", "else", "enum", "event", "explicit", "extern", "false", "finally", "fixed", "float", "for", "foreach", "goto", "if", "implicit", "in", "int", "interface", "internal", "is", "lock", "long", "namespace", "new", "null", "object", "operator", "out", "override", "params", "private", "protected", "public", "readonly", "ref", "return", "sbyte", "sealed", "short", "sizeof", "stackalloc", "static", "string", "struct", "switch", "this", "throw", "true", "try", "typeof", "uint", "ulong", "unchecked", "unsafe", "ushort", "using", "virtual", "void", "volatile", "while", "record", "var"];
    let mut s: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    let mut escaped = s != id;
    if s.chars().next().map_or(true, |c| c.is_ascii_digit()) { s.insert(0, '_'); escaped = true; }
    if KEYWORDS.contains(&s.as_str()) { s.push('_'); escaped = true; }
    (s, escaped)
}

struct BlockRow {
    id: String,
    size: (i32, i32),
    cells: Option<Vec<(i32, i32)>>,
    collections: u32,
    waypoint: Option<i32>,
    ground: bool,
    air: bool,
    variants: usize,
    conflicts: Vec<String>,
}

struct ItemRow {
    id: String,
    collections: u32,
    waypoint: Option<i32>,
    variants: usize,
    size: [f32; 3],
    conflicts: Vec<String>,
}

fn variant_facts(v: &crate::blockinfo::Variant) -> Option<((i32, i32), Vec<(i32, i32)>)> {
    if v.block_units.is_empty() { return None; }
    let sx = v.block_units.iter().map(|u| u.offset[0] + 1).max().unwrap_or(1);
    let sz = v.block_units.iter().map(|u| u.offset[2] + 1).max().unwrap_or(1);
    let mut cells: Vec<(i32, i32)> = v.block_units.iter().map(|u| (u.offset[0], u.offset[2])).collect();
    cells.sort_unstable();
    cells.dedup();
    Some(((sx, sz), cells))
}

pub fn cmd(store: &mut crate::store::DataStore, rest: &[String], flag: &dyn Fn(&[String], &str) -> Option<String>) -> Result<(), String> {
    let out = flag(rest, "--out").ok_or("csgen needs --out DIR")?;
    let label = flag(rest, "--paks-label").unwrap_or_else(|| "the Trackmania 2020 Stadium / RedIsland / WhiteShore / GreenCoast / BlueBay paks".into());
    std::fs::create_dir_all(&out).map_err(|e| format!("{out}: {e}"))?;
    let entries: Vec<String> = store.entries().map(|e| e.path()).collect();
    // ---- blocks
    let mut blocks: BTreeMap<String, BlockRow> = BTreeMap::new();
    let mut failed = 0usize;
    for p in entries.iter().filter(|p| { let u = p.to_uppercase(); u.contains("\\GAMECTNBLOCKINFO\\") && u.ends_with(".GBX") }) {
        let coll_name = p.split('\\').next().unwrap_or("");
        let Some((_, _, bit)) = COLLECTIONS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(coll_name)) else { continue };
        let Ok(bi) = crate::blockinfo::load(store, p) else { failed += 1; continue };
        let name = if bi.name.is_empty() { p.rsplit('\\').next().unwrap_or(p).split('.').next().unwrap_or("").to_string() } else { bi.name.clone() };
        if name.is_empty() { continue; }
        let g = bi.variant_base_ground.as_ref().and_then(variant_facts);
        let a = bi.variant_base_air.as_ref().and_then(variant_facts);
        let ground = g.is_some();
        let air = a.is_some();
        let (size, cells) = match g.clone().or(a.clone()) { Some(x) => x, None => ((1, 1), vec![(0, 0)]) };
        let full = (cells.len() as i32) >= size.0 * size.1;
        // the variant count: every placement the editor can pick — the base ground + base air + the additional
        // ground/air variants that carry content (the same families `pick_placement_add` chooses from)
        let has = |v: &crate::blockinfo::Variant| !v.block_units.is_empty() || v.mobils.iter().any(|l| !l.is_empty());
        let variants = bi.variant_base_ground.iter().filter(|v| has(v)).count() + bi.variant_base_air.iter().filter(|v| has(v)).count() + bi.additional_ground.iter().filter(|v| has(v)).count() + bi.additional_air.iter().filter(|v| has(v)).count();
        let row = BlockRow { id: name.clone(), size, cells: if full { None } else { Some(cells) }, collections: *bit, waypoint: bi.waypoint_type, ground, air, variants, conflicts: Vec::new() };
        match blocks.get_mut(&name) {
            None => { blocks.insert(name, row); }
            Some(e) => {
                e.collections |= bit;
                if e.size != row.size { e.conflicts.push(format!("{coll_name}: size {}x{} vs {}x{}", row.size.0, row.size.1, e.size.0, e.size.1)); }
                if e.cells != row.cells { e.conflicts.push(format!("{coll_name}: cells differ")); }
                if e.variants != row.variants { e.conflicts.push(format!("{coll_name}: {} variants vs {}", row.variants, e.variants)); e.variants = e.variants.max(row.variants); }
                if e.waypoint != row.waypoint { e.conflicts.push(format!("{coll_name}: waypoint {:?} vs {:?}", row.waypoint, e.waypoint)); }
                e.ground |= row.ground;
                e.air |= row.air;
            }
        }
    }
    // ---- items
    let mut items: BTreeMap<String, ItemRow> = BTreeMap::new();
    let mut item_failed: Vec<String> = Vec::new();
    for p in entries.iter().filter(|p| { let u = p.to_uppercase(); u.contains("\\ITEMS\\") && u.ends_with(".ITEM.GBX") }) {
        let coll_name = p.split('\\').next().unwrap_or("");
        let Some((_, coll_id, bit)) = COLLECTIONS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(coll_name)) else { continue };
        let name = p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".Item.Gbx").trim_end_matches(".item.gbx").to_string();
        let variants = crate::static_item::build::pack_item_variants(store, p).map(|v| v.len()).unwrap_or(0);
        // the waypoint from the item's trigger entity (pack_item_merged reads it; a failure there only loses the waypoint)
        let waypoint = crate::static_item::build::pack_item_merged(store, p, 1.0, *coll_id, 0, None).ok().and_then(|m| m.waypoint_type);
        // the bbox of variant 0: the picked external baked whole through the Collector (prefab / static object with
        // every entity incl. the dyna parts of the obstacles); a tree species by its baked height and radius
        let picked = crate::static_item::build::pack_item_variants(store, p).ok().and_then(|v| v.into_iter().next());
        let size = match picked {
            Some(sp) if sp.to_ascii_lowercase().ends_with(".vegettreemodel.gbx") => {
                let mut mm = crate::static_item::build::Merged::default();
                match crate::static_item::build::add_veget_tree_model(store, &sp, 1.0, &mut mm) {
                    Ok(vb) => [2.0 * vb.radius, vb.height, 2.0 * vb.radius],
                    Err(e2) => { item_failed.push(format!("{p}: {e2}")); [0.0; 3] }
                }
            }
            Some(sp) => match store.load_model(&sp) {
                Ok(loaded) => {
                    let mut c = crate::geom::Collector::new(store);
                    c.model(&loaded, &crate::geom::IDENTITY, 0);
                    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                    for g in c.scene.groups.values() { for v in &g.verts { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } } }
                    if lo[0].is_finite() { [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]] } else { item_failed.push(format!("{p}: {sp}: no geometry")); [0.0; 3] }
                }
                Err(e) => { item_failed.push(format!("{p}: {e}")); [0.0; 3] }
            },
            None => { item_failed.push(format!("{p}: no prefab / static object / tree external")); [0.0; 3] }
        };
        let row = ItemRow { id: name.clone(), collections: *bit, waypoint, variants, size, conflicts: Vec::new() };
        match items.get_mut(&name) {
            None => { items.insert(name, row); }
            Some(e) => {
                e.collections |= bit;
                if (0..3).any(|k| (e.size[k] - row.size[k]).abs() > 0.01) { e.conflicts.push(format!("{coll_name}: size {:?} vs {:?}", row.size, e.size)); }
                if e.waypoint != row.waypoint { e.conflicts.push(format!("{coll_name}: waypoint {:?} vs {:?}", row.waypoint, e.waypoint)); }
                if e.variants != row.variants { e.conflicts.push(format!("{coll_name}: {} variants vs {}", row.variants, e.variants)); e.variants = e.variants.max(row.variants); }
            }
        }
    }
    // ---- identifiers (unique)
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut escaped: Vec<String> = Vec::new();
    let mut ident_of = |id: &str| -> String {
        let (mut s, esc) = identifier(id);
        if esc { escaped.push(format!("{id} → {s}")); }
        let base = s.clone();
        let mut n = 2;
        while !used.insert(s.clone()) { s = format!("{base}_{n}"); n += 1; }
        s
    };
    // (b) the checkpoint → plain twin mapping (hugo-checkpoints-cleaned/MAPPING.tsv: block, replacement, …)
    let plain: BTreeMap<String, String> = flag(rest, "--plain").map(|f| {
        std::fs::read_to_string(&f).unwrap_or_default().lines().skip(1).filter_map(|l| { let c: Vec<&str> = l.split('\t').collect(); if c.len() < 2 || c[1].is_empty() || c[1].starts_with('(') { return None; } Some((c[0].to_string(), c[1].to_string())) }).collect()
    }).unwrap_or_default();
    let plain_missing: Vec<String> = plain.iter().filter(|(k, v)| !blocks.contains_key(*k) || !blocks.contains_key(*v)).map(|(k, v)| format!("{k} → {v}")).collect();
    let header = format!("// generated by mapgeom csgen from {label}\n");
    // `#nullable enable`: the compiler treats *.g.cs as auto-generated and wants the context explicit (CS8669)
    let common = "#nullable enable\nusing System.Collections.Generic;\nusing GBX.NET;\n\nnamespace tman.src.Inventory;\n\n";
    let enums = "[System.Flags]\npublic enum Collections { Stadium = 1, RedIsland = 2, WhiteShore = 4, GreenCoast = 8, BlueBay = 16, All = 31 }\n\npublic enum Waypoint { None, Start, Finish, Checkpoint, StartFinish }\n\n";
    // Blocks.g.cs
    let mut cs = String::new();
    cs.push_str(&header);
    cs.push_str(common);
    cs.push_str(enums);
    cs.push_str("public readonly record struct BlockVariant(Block Model, int Variant);\n\n");
    cs.push_str("public sealed record Block(string Id, int SizeX, int SizeZ, int[][]? Cells, Collections Collections, Waypoint Waypoint, bool Ground, bool Air, int Variants)\n{\n    public Block? Plain => Blocks.PlainOf.TryGetValue(Id, out var plain) ? plain : null;\n\n    public BlockVariant Variant(int n) => new(this, n);\n}\n\n");
    cs.push_str("public static class Blocks\n{\n");
    let mut block_idents: Vec<(String, String)> = Vec::new();
    for (id, b) in &blocks {
        let ident = ident_of(id);
        let cells = match &b.cells {
            None => "null".to_string(),
            Some(c) => format!("new[] {{ {} }}", c.iter().map(|(x, z)| format!("new[] {{ {x}, {z} }}")).collect::<Vec<_>>().join(", ")),
        };
        cs.push_str(&format!("    public static readonly Block {ident} = new(\"{id}\", {}, {}, {cells}, {}, {}, {}, {}, {});\n", b.size.0, b.size.1, collections_cs(b.collections), waypoint_cs(b.waypoint), b.ground, b.air, b.variants));
        block_idents.push((ident, id.clone()));
    }
    cs.push_str("\n    public static readonly IReadOnlyDictionary<string, Block> ById = new Dictionary<string, Block>\n    {\n");
    for (ident, id) in &block_idents { cs.push_str(&format!("        [\"{id}\"] = {ident},\n")); }
    cs.push_str("    };\n\n    public static Block? FromId(string id) => ById.TryGetValue(id, out var block) ? block : null;\n\n");
    cs.push_str("    public static readonly IReadOnlyDictionary<string, Block> PlainOf = new Dictionary<string, Block>\n    {\n");
    let ident_lookup: BTreeMap<&str, &str> = block_idents.iter().map(|(i, id)| (id.as_str(), i.as_str())).collect();
    let mut plain_written = 0usize;
    for (cp, pl) in &plain {
        if let (Some(_), Some(pi)) = (ident_lookup.get(cp.as_str()), ident_lookup.get(pl.as_str())) {
            cs.push_str(&format!("        [\"{cp}\"] = {pi},\n"));
            plain_written += 1;
        }
    }
    cs.push_str("    };\n}\n");
    let blocks_path = format!("{out}/Blocks.g.cs");
    std::fs::write(&blocks_path, &cs).map_err(|e| format!("{blocks_path}: {e}"))?;
    // Items.g.cs
    used.clear();
    let mut ident_of2 = |id: &str| -> String {
        let (mut s, esc) = identifier(id);
        if esc { escaped.push(format!("item {id} → {s}")); }
        let base = s.clone();
        let mut n = 2;
        while !used.insert(s.clone()) { s = format!("{base}_{n}"); n += 1; }
        s
    };
    let mut ci = String::new();
    ci.push_str(&header);
    ci.push_str(common);
    ci.push_str("public readonly record struct ItemVariant(Item Model, int Variant);\n\n");
    ci.push_str("public sealed record Item(string Id, string Author, string? Path, Collections Collections, Waypoint Waypoint, int Variants, Vec3 Size)\n{\n    public ItemVariant Variant(int n) => new(this, n);\n}\n\n");
    ci.push_str("public static class Items\n{\n");
    let mut item_idents: Vec<(String, String)> = Vec::new();
    for (id, it) in &items {
        let ident = ident_of2(id);
        ci.push_str(&format!("    public static readonly Item {ident} = new(\"{id}\", \"Nadeo\", null, {}, {}, {}, new Vec3({:.3}f, {:.3}f, {:.3}f));\n", collections_cs(it.collections), waypoint_cs(it.waypoint), it.variants, it.size[0], it.size[1], it.size[2]));
        item_idents.push((ident, id.clone()));
    }
    ci.push_str("\n    public static readonly IReadOnlyDictionary<string, Item> ById = new Dictionary<string, Item>\n    {\n");
    for (ident, id) in &item_idents { ci.push_str(&format!("        [\"{id}\"] = {ident},\n")); }
    ci.push_str("    };\n\n    public static Item? FromId(string id) => ById.TryGetValue(id, out var item) ? item : null;\n}\n");
    let items_path = format!("{out}/Items.g.cs");
    std::fs::write(&items_path, &ci).map_err(|e| format!("{items_path}: {e}"))?;
    // (c) Custom.g.cs: `--project ROOT --custom SUBDIR` (repeatable SUBDIR): every *.Item.Gbx under ROOT/SUBDIR
    // (recursive) → one Item (Id = file name, Author = Id, Path = ROOT-relative with forward slashes, Waypoint from
    // chunk 0x2E00201F of a crystal item, Variants 1, Size = the crystal's position bbox)
    let project = flag(rest, "--project");
    let mut custom_dirs: Vec<String> = Vec::new();
    for (i, a) in rest.iter().enumerate() { if a == "--custom" { if let Some(d) = rest.get(i + 1) { custom_dirs.push(d.clone()); } } }
    let mut custom_n = 0usize;
    if !custom_dirs.is_empty() {
        let root = std::path::PathBuf::from(project.clone().unwrap_or_else(|| ".".into()));
        let mut cc = String::new();
        cc.push_str(&header);
        cc.push_str(common);
        cc.push_str("public static class Custom\n{\n");
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() { walk(&p, out); } else if p.file_name().map_or(false, |n| n.to_string_lossy().ends_with(".Item.Gbx")) { out.push(p); }
            }
        }
        for d in &custom_dirs { walk(&root.join(d), &mut files); }
        files.sort();
        let mut used_c: BTreeSet<String> = BTreeSet::new();
        let mut custom_idents: Vec<(String, String)> = Vec::new();
        let mut seen_names: BTreeSet<String> = BTreeSet::new();
        let mut dup_names: Vec<String> = Vec::new();
        for p in &files {
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            if !seen_names.insert(file.clone()) { dup_names.push(p.display().to_string()); continue; }
            let stem = file.trim_end_matches(".Item.Gbx");
            let (mut ident, esc) = identifier(stem);
            if esc { escaped.push(format!("custom {stem} → {ident}")); }
            let base = ident.clone();
            let mut n = 2;
            while !used_c.insert(ident.clone()) { ident = format!("{base}_{n}"); n += 1; }
            let rel = p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
            let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
            let (mut waypoint, mut size) = (None, [0.0f32; 3]);
            // the waypoint chunk 0x2E00201F (version u32, waypoint type u32): in the crystal item's decompressed body,
            // or straight in the file for our uncompressed static items (flags, stands, lamps)
            let find_wp = |hay: &[u8]| -> Option<i32> {
                let pos = hay.windows(4).position(|w| w == [0x1F, 0x20, 0x00, 0x2E])?;
                if pos + 12 > hay.len() { return None; }
                let ver = u32::from_le_bytes([hay[pos + 4], hay[pos + 5], hay[pos + 6], hay[pos + 7]]);
                let wp = u32::from_le_bytes([hay[pos + 8], hay[pos + 9], hay[pos + 10], hay[pos + 11]]);
                if ver <= 20 && wp <= 5 { Some(wp as i32) } else { None }
            };
            if let Ok(it) = crate::crystal::ItemCrystal::open(&bytes) {
                let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                for l in &it.model.layers {
                    if let crate::crystal_model::LayerKind::Geometry { crystal, .. } | crate::crystal_model::LayerKind::Trigger { crystal, .. } = &l.kind {
                        for q in &crystal.positions { for k in 0..3 { lo[k] = lo[k].min(q[k]); hi[k] = hi[k].max(q[k]); } }
                    }
                }
                if lo[0].is_finite() { size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]; }
                waypoint = find_wp(&it.body);
            } else if let Ok(f) = crate::static_item::file::parse_file(&bytes) {
                // a static item (our carriers): bbox over its visuals' bounding boxes + the trigger / collision shapes
                let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
                for b in crate::static_item::file::item_bboxes(&f.item) { for k in 0..3 { lo[k] = lo[k].min(b[k]); hi[k] = hi[k].max(b[k + 3]); } }
                if lo[0].is_finite() { size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]; }
                waypoint = find_wp(&bytes);
            } else {
                waypoint = find_wp(&bytes);
            }
            cc.push_str(&format!("    public static readonly Item {ident} = new(\"{file}\", \"{file}\", \"{rel}\", Collections.All, {}, 1, new Vec3({:.3}f, {:.3}f, {:.3}f));\n", waypoint_cs(waypoint), size[0], size[1], size[2]));
            custom_idents.push((ident, file));
            custom_n += 1;
        }
        cc.push_str("\n    public static readonly IReadOnlyDictionary<string, Item> ById = new Dictionary<string, Item>\n    {\n");
        for (ident, id) in &custom_idents { cc.push_str(&format!("        [\"{id}\"] = {ident},\n")); }
        cc.push_str("    };\n\n    public static Item? FromId(string id) => ById.TryGetValue(id, out var item) ? item : null;\n}\n");
        let custom_path = format!("{out}/Custom.g.cs");
        std::fs::write(&custom_path, &cc).map_err(|e| format!("{custom_path}: {e}"))?;
        println!("wrote {custom_path}: {custom_n} items ({} bytes) from {} under {}{}", cc.len(), custom_dirs.join(", "), root.display(), if dup_names.is_empty() { String::new() } else { format!("; duplicate file names skipped: {}", dup_names.join(", ")) });
    }
    println!("plain twins written: {plain_written} of {} mapping rows{}", plain.len(), if plain_missing.is_empty() { String::new() } else { format!("; unresolved: {}", plain_missing.join(", ")) });
    // report
    let block_conflicts: Vec<String> = blocks.values().filter(|b| !b.conflicts.is_empty()).map(|b| format!("{}: {}", b.id, b.conflicts.join("; "))).collect();
    let count_kind = |k: &str| blocks.values().filter(|b| b.conflicts.iter().any(|c| c.contains(k))).count();
    println!("block conflict kinds: size {} names, cells {} names, variants {} names (Stadium.pak carries extra 'additional' variants the vista paks lack — the max is written), waypoint {} names", count_kind(": size"), count_kind("cells differ"), count_kind("variants vs"), count_kind("waypoint"));
    let item_conflicts: Vec<String> = items.values().filter(|b| !b.conflicts.is_empty()).map(|b| format!("{}: {}", b.id, b.conflicts.join("; "))).collect();
    println!("wrote {blocks_path}: {} blocks ({} bytes; {} with cells, {} ground, {} air, waypoints {}), {} block infos unreadable", blocks.len(), cs.len(), blocks.values().filter(|b| b.cells.is_some()).count(), blocks.values().filter(|b| b.ground).count(), blocks.values().filter(|b| b.air).count(), blocks.values().filter(|b| b.waypoint.map_or(false, |w| w != 3)).count(), failed);
    println!("wrote {items_path}: {} items ({} bytes; waypoints {}), {} items unreadable", items.len(), ci.len(), items.values().filter(|b| b.waypoint.is_some()).count(), item_failed.len());
    println!("per collection: {}", COLLECTIONS.iter().map(|(n, _, bit)| format!("{n} {} blocks / {} items", blocks.values().filter(|b| b.collections & bit != 0).count(), items.values().filter(|b| b.collections & bit != 0).count())).collect::<Vec<_>>().join(", "));
    println!("escaped identifiers ({}): {}", escaped.len(), escaped.join(", "));
    println!("block conflicts across paks ({}): {}", block_conflicts.len(), block_conflicts.iter().take(20).cloned().collect::<Vec<_>>().join(" | "));
    println!("item conflicts across paks ({}): {}", item_conflicts.len(), item_conflicts.iter().take(20).cloned().collect::<Vec<_>>().join(" | "));
    for f in item_failed.iter().take(10) { println!("  item unreadable: {f}"); }
    Ok(())
}
