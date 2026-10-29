//! `mapgeom csgen --out DIR [--paks-label TEXT]` — Hugo's generated C# (2026-10-27 02:34: "generate C# block classes
//! from the pak files so I don't pass around ids"): `Blocks.g.cs` (one `Block` record per block NAME across the 5
//! collection paks) and `Items.g.cs` (one `Item` per item of the paks' item libraries), namespace
//! `tman.src.Inventory.Generated`, a single header comment line.

use std::collections::{BTreeMap, BTreeSet};

const COLLECTIONS: [(&str, u32, u32); 5] = [("Stadium", 0x1a, 1), ("RedIsland", 0x10, 2), ("WhiteShore", 0x1d, 4), ("GreenCoast", 0xf, 8), ("BlueBay", 0x1c, 16)];

/// `v` to three decimals as the C# generator does it: thousandths rounded half away from zero on the f64 value.
pub fn f3(v: f32) -> String {
    let t = (v as f64 * 1000.0).round() as i64;
    format!("{}{}.{:03}", if t < 0 { "-" } else { "" }, t.abs() / 1000, t.abs() % 1000)
}

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
    /// cells tall: the base variant's units' max y offset + 1 (StructurePillar 1, DecoWallSlope2Straight 2, …)
    size_y: i32,
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

/// The Size rule shared with tman.Generator (C#): the bbox of every vertex of the variant-0 model — prefab entities
/// posed by `from_quat` (the formula copied verbatim on the C# side), static / dyna objects (static shape, mesh, and
/// the moving shape when it is another node), surfaces (their meshes, compounds flattened by their Iso4s), every
/// visual's positions (inline + streams), solid trees composed — nothing else (triggers, spawns, helpers, lights,
/// materials carry no size). Externals are followed by name; a vegetation model is a tree (the (2r, h, 2r) rule).
struct SizeWalk<'a> {
    store: &'a mut crate::store::DataStore,
    lo: [f32; 3],
    hi: [f32; 3],
    any: bool,
    depth: usize,
}

impl<'a> SizeWalk<'a> {
    fn point(&mut self, p: [f32; 3]) {
        for k in 0..3 { self.lo[k] = self.lo[k].min(p[k]); self.hi[k] = self.hi[k].max(p[k]); }
        self.any = true;
    }
    fn file(&mut self, path: &str, at: &crate::geom::Xform) {
        if self.depth > 16 { return; }
        let Ok(model) = self.store.load_model(path) else { return };
        let Ok(graph) = model.graph() else { return };
        let root = graph.root.clone();
        let slots = graph.slots.clone();
        drop(graph);
        if let Some(n) = root {
            self.depth += 1;
            self.node(&n, &slots, at);
            self.depth -= 1;
        }
    }
    fn slot(&mut self, idx: i32, slots: &[crate::node::Slot], at: &crate::geom::Xform) {
        if idx < 0 { return; }
        match slots.get(idx as usize) {
            Some(crate::node::Slot::Node(n)) => { let n = n.clone(); self.node(&n, slots, at); }
            Some(crate::node::Slot::External(path)) => {
                let up = path.to_uppercase();
                if up.ends_with(".PREFAB.GBX") || up.ends_with(".STATICOBJECT.GBX") || up.ends_with(".DYNAOBJECT.GBX") || up.ends_with(".MESH.GBX") || up.ends_with(".SOLID2MODEL.GBX") || up.ends_with(".SHAPE.GBX") || up.ends_with(".HITSHAPE.GBX") || up.ends_with(".SOLID.GBX") {
                    let path = path.clone();
                    self.file(&path, at);
                }
            }
            _ => {}
        }
    }
    fn node(&mut self, n: &crate::node::Node, slots: &[crate::node::Slot], at: &crate::geom::Xform) {
        use crate::node::Node;
        match n {
            Node::Prefab(p) => {
                for e in &p.ents {
                    let m = crate::geom::compose(at, &crate::geom::from_quat(e.rot, e.pos));
                    self.slot(e.model, slots, &m);
                }
            }
            Node::StaticObject(s) => {
                self.slot(s.shape, slots, at);
                self.slot(s.mesh, slots, at);
            }
            Node::Dyna(d) => {
                self.slot(d.static_shape, slots, at);
                self.slot(d.mesh, slots, at);
                if d.dyna_shape != d.static_shape { self.slot(d.dyna_shape, slots, at); }
            }
            Node::Surface(s) => {
                for m in &s.meshes {
                    for v in &m.verts { let q = crate::geom::apply(at, *v); self.point(q); }
                }
            }
            Node::Solid2(s) => {
                for vi in &s.visuals {
                    if let Some(crate::node::Slot::Node(Node::Visual(v))) = slots.get((*vi).max(0) as usize) {
                        for p in &v.inline_positions { let q = crate::geom::apply(at, *p); self.point(q); }
                        for si in &v.vertex_streams {
                            if let Some(crate::node::Slot::Node(Node::VertexStream(vs))) = slots.get((*si).max(0) as usize) {
                                for p in &vs.positions { let q = crate::geom::apply(at, *p); self.point(q); }
                            }
                        }
                    }
                }
            }
            Node::Tree(t) => {
                let m = match &t.transform { Some(x) => crate::geom::compose(at, x), None => *at };
                if t.visual >= 0 {
                    if let Some(crate::node::Slot::Node(Node::Visual(v))) = slots.get(t.visual as usize) {
                        for p in &v.inline_positions { let q = crate::geom::apply(&m, *p); self.point(q); }
                        for si in &v.vertex_streams {
                            if let Some(crate::node::Slot::Node(Node::VertexStream(vs))) = slots.get((*si).max(0) as usize) {
                                for p in &vs.positions { let q = crate::geom::apply(&m, *p); self.point(q); }
                            }
                        }
                    }
                }
                for c in &t.children { self.slot(*c, slots, &m); }
            }
            Node::ItemModel(i) => self.slot(*i, slots, at),
            _ => {}
        }
    }
}

/// Size of a pack item's variant-0 model by the shared rule; None when nothing has geometry.
pub fn model_size(store: &mut crate::store::DataStore, path: &str) -> Option<[f32; 3]> {
    let mut w = SizeWalk { store, lo: [f32::INFINITY; 3], hi: [f32::NEG_INFINITY; 3], any: false, depth: 0 };
    w.file(path, &crate::geom::IDENTITY);
    if w.any { Some([w.hi[0] - w.lo[0], w.hi[1] - w.lo[1], w.hi[2] - w.lo[2]]) } else { None }
}

/// The item's own waypoint type: chunk 0x2E00201F (version, type) in its body, None when absent.
pub fn item_waypoint(body: &[u8]) -> Option<i32> {
    let pos = body.windows(4).position(|w| w == [0x1F, 0x20, 0x00, 0x2E])?;
    if pos + 12 > body.len() { return None; }
    let ver = u32::from_le_bytes([body[pos + 4], body[pos + 5], body[pos + 6], body[pos + 7]]);
    let wp = u32::from_le_bytes([body[pos + 8], body[pos + 9], body[pos + 10], body[pos + 11]]);
    if ver <= 20 && wp <= 5 { Some(wp as i32) } else { None }
}

/// Waypoint families, as tman.Generator computes them: a waypoint block/item's twins of the other kinds = its name
/// with the kind token (Start / Finish / Checkpoint / Multilap) swapped, kept when that name exists with the expected
/// waypoint; `gates`: GateFinish{W}m ↔ the Right-side gate family (there is no GateFinishRight). (name, kind, twin).
fn families(waypoints: &BTreeMap<String, Option<i32>>, gates: bool) -> Vec<(String, i32, String, bool)> {
    const KINDS: [(i32, &str); 4] = [(0, "Start"), (1, "Finish"), (2, "Checkpoint"), (4, "Multilap")];
    let is = |name: &str, kind: i32| waypoints.get(name).map_or(false, |w| *w == Some(kind));
    let mut out = Vec::new();
    for (name, wp) in waypoints {
        let Some(w) = *wp else { continue };
        let Some((_, own)) = KINDS.iter().find(|(k, _)| *k == w) else { continue };
        let at = name.find(own);
        // (side, width) of a Gate{Start|Checkpoint|Multilap}{Left|Right}{W}m item; its Finish twin is the one-pole
        // GateFinish{W}m — turned by a half-turn for the Left side (the pole swaps sides), `HalfTurned` lists those
        let side_gate: Option<(&str, &str)> = if gates {
            name.strip_prefix("Gate").and_then(|r| ["Start", "Checkpoint", "Multilap"].iter().find_map(|t| r.strip_prefix(t))).and_then(|r| ["Left", "Right"].iter().find_map(|side| r.strip_prefix(side).map(|w| (*side, w)))).and_then(|(side, r)| r.strip_suffix('m').map(|w| (side, w))).filter(|(_, w)| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit()))
        } else { None };
        let finish_gate = if gates { name.strip_prefix("GateFinish").and_then(|r| r.strip_suffix('m')).filter(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit())) } else { None };
        for (kind, token) in KINDS {
            if kind == w { continue; }
            let mut twin = at.map(|i| format!("{}{}{}", &name[..i], token, &name[i + own.len()..]));
            let mut half_turned = false;
            if twin.as_deref().map_or(true, |t| !is(t, kind)) {
                twin = match (finish_gate, side_gate) {
                    (Some(width), _) => Some(format!("Gate{token}Right{width}m")),
                    (None, Some((side, width))) if kind == 1 => { half_turned = side == "Left"; Some(format!("GateFinish{width}m")) }
                    _ => None,
                };
            }
            if let Some(t) = twin { if is(&t, kind) { out.push((name.clone(), kind, t, half_turned)); } }
        }
    }
    out
}

fn families_cs(ty: &str, fams: &[(String, i32, String, bool)], ident_of: &BTreeMap<&str, &str>) -> String {
    let mut s = format!("\n    public static readonly IReadOnlyDictionary<(string, Waypoint), {ty}> Families = new Dictionary<(string, Waypoint), {ty}>\n    {{\n");
    for (name, kind, twin, _) in fams { s.push_str(&format!("        [(\"{name}\", {})] = {},\n", waypoint_cs(Some(*kind)), ident_of.get(twin.as_str()).copied().unwrap_or(""))); }
    s.push_str("    };\n");
    if ty == "Item" {
        s.push_str("\n    public static readonly IReadOnlySet<(string, Waypoint)> HalfTurned = new HashSet<(string, Waypoint)>\n    {\n");
        for (name, kind, _, half) in fams.iter().filter(|f| f.3) { s.push_str(&format!("        (\"{name}\", {}),\n", waypoint_cs(Some(*kind)))); let _ = half; }
        s.push_str("    };\n");
    }
    s
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
    let label = flag(rest, "--paks-label").unwrap_or_else(|| "Trackmania 2020 Stadium.pak / RedIsland.pak / WhiteShore.pak / GreenCoast.pak / BlueBay.pak (client 2026-10, tm-paks store)".into());
    std::fs::create_dir_all(&out).map_err(|e| format!("{out}: {e}"))?;
    let entries: Vec<String> = store.entries().map(|e| e.path()).collect();
    let name_vs_ident = rest.iter().any(|x| x == "--name-vs-ident");
    // --catalogs-only: no enum / record declarations in the files (Hugo's reorg keeps those hand-written)
    let catalogs_only = rest.iter().any(|x| x == "--catalogs-only");
    // --size-y: `Block(string Id, int SizeX, int SizeZ, int SizeY, int[][]? Cells, …)` — the block's height in cells
    let size_y_field = rest.iter().any(|x| x == "--size-y");
    // ---- blocks
    let mut blocks: BTreeMap<String, BlockRow> = BTreeMap::new();
    let mut failed = 0usize;
    for p in entries.iter().filter(|p| { let u = p.to_uppercase(); u.contains("\\GAMECTNBLOCKINFO\\") && u.ends_with(".GBX") }) {
        let coll_name = p.split('\\').next().unwrap_or("");
        let Some((_, _, bit)) = COLLECTIONS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(coll_name)) else { continue };
        let Ok(bi) = crate::blockinfo::load(store, p) else { failed += 1; continue };
        // the IDENT (what the map records carry), the display name only when the file has no ident chunk
        let name = if !bi.ident.is_empty() { bi.ident.clone() } else if !bi.name.is_empty() { bi.name.clone() } else { p.rsplit('\\').next().unwrap_or(p).split('.').next().unwrap_or("").to_string() };
        if name.is_empty() { continue; }
        // --name-vs-ident: the block infos whose display name differs from their ident
        if name_vs_ident && !bi.ident.is_empty() && !bi.name.is_empty() && !bi.ident.eq_ignore_ascii_case(&bi.name) { println!("name/ident differ: {coll_name}: name {} ident {} ({p})", bi.name, bi.ident); }
        let g = bi.variant_base_ground.as_ref().and_then(variant_facts);
        let a = bi.variant_base_air.as_ref().and_then(variant_facts);
        let ground = g.is_some();
        let air = a.is_some();
        let (size, cells) = match g.clone().or(a.clone()) { Some(x) => x, None => ((1, 1), vec![(0, 0)]) };
        // the height in cells from the variant that gives the footprint (the first base variant WITH units), as the C#
        let size_y = [bi.variant_base_ground.as_ref(), bi.variant_base_air.as_ref()].into_iter().flatten().find(|v| !v.block_units.is_empty()).map(|v| v.block_units.iter().map(|u| u.offset[1] + 1).max().unwrap_or(1).max(1)).unwrap_or(1);
        let full = (cells.len() as i32) >= size.0 * size.1;
        // the variant count: every placement the editor can pick — the base ground + base air + the additional
        // ground/air variants that carry content (the same families `pick_placement_add` chooses from)
        let has = |v: &crate::blockinfo::Variant| !v.block_units.is_empty() || v.mobils.iter().any(|l| !l.is_empty());
        let variants = bi.variant_base_ground.iter().filter(|v| has(v)).count() + bi.variant_base_air.iter().filter(|v| has(v)).count() + bi.additional_ground.iter().filter(|v| has(v)).count() + bi.additional_air.iter().filter(|v| has(v)).count();
        let row = BlockRow { id: name.clone(), size, size_y, cells: if full { None } else { Some(cells) }, collections: *bit, waypoint: bi.waypoint_type, ground, air, variants, conflicts: Vec::new() };
        match blocks.get_mut(&name) {
            None => { blocks.insert(name, row); }
            Some(e) => {
                e.collections |= bit;
                if e.size != row.size { e.conflicts.push(format!("{coll_name}: size {}x{} vs {}x{}", row.size.0, row.size.1, e.size.0, e.size.1)); }
                if e.size_y != row.size_y { e.conflicts.push(format!("{coll_name}: height {} vs {}", row.size_y, e.size_y)); e.size_y = e.size_y.max(row.size_y); }
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
    // --trees-out F.tsv: the vegetation items (variant 0 = a VegetTreeModel) with their sizes — the data table the C#
    // generator carries, since GBX.NET.PAK cannot read those pak entries
    let mut trees: Vec<(String, [f32; 3])> = Vec::new();
    for p in entries.iter().filter(|p| { let u = p.to_uppercase(); u.contains("\\ITEMS\\") && u.ends_with(".ITEM.GBX") }) {
        let coll_name = p.split('\\').next().unwrap_or("");
        let Some((_, coll_id, bit)) = COLLECTIONS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(coll_name)) else { continue };
        let name = p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".Item.Gbx").trim_end_matches(".item.gbx").to_string();
        let variants = crate::static_item::build::pack_item_variants(store, p).map(|v| v.len()).unwrap_or(0);
        let waypoint = store.load_model(p).ok().and_then(|m| item_waypoint(&m.body));
        let picked = crate::static_item::build::pack_item_variants(store, p).ok().and_then(|v| v.into_iter().next());
        let size = match picked {
            Some(sp) if sp.to_ascii_lowercase().ends_with(".vegettreemodel.gbx") => {
                let mut mm = crate::static_item::build::Merged::default();
                let sz = match crate::static_item::build::add_veget_tree_model(store, &sp, 1.0, &mut mm) {
                    Ok(vb) => [2.0 * vb.radius, vb.height, 2.0 * vb.radius],
                    Err(e2) => { item_failed.push(format!("{p}: {e2}")); [0.0; 3] }
                };
                if !items.contains_key(&name) { trees.push((name.clone(), sz)); }
                sz
            }
            Some(sp) => match model_size(store, &sp) { Some(sz) => sz, None => { item_failed.push(format!("{p}: {sp}: no geometry")); [0.0; 3] } },
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
    // --respawn-census F.tsv: every waypoint block's NoRespawn (chunk 0x0304E00F) and every waypoint item's
    // NPlugTrigger_SWaypoint NoRespawn field (prefab form; an entity-model item has no such field), per pak
    if let Some(cp) = flag(rest, "--respawn-census") {
        let mut t = String::from("kind\tpak\tname\twaypoint\tno_respawn\tsource\n");
        for p in entries.iter().filter(|p| { let u = p.to_uppercase(); u.contains("\\GAMECTNBLOCKINFO\\") && u.ends_with(".GBX") }) {
            let coll_name = p.split('\\').next().unwrap_or("").to_string();
            let Ok(bi) = crate::blockinfo::load(store, p) else { continue };
            if bi.waypoint_type.map_or(true, |w| w == 3) { continue; }
            let name = if !bi.ident.is_empty() { bi.ident.clone() } else { bi.name.clone() };
            t.push_str(&format!("block\t{coll_name}\t{name}\t{}\t{}\tchunk 0x0304E00F\n", waypoint_cs(bi.waypoint_type), bi.no_respawn));
        }
        for p in entries.iter().filter(|p| { let u = p.to_uppercase(); u.contains("\\ITEMS\\") && u.ends_with(".ITEM.GBX") }) {
            let coll_name = p.split('\\').next().unwrap_or("").to_string();
            let name = p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".Item.Gbx").to_string();
            let wp = store.load_model(p).ok().and_then(|m| item_waypoint(&m.body));
            if wp.map_or(true, |w| w == 3) { continue; }
            match crate::static_item::build::pack_item_merged(store, p, 1.0, 26, 0, None) {
                Ok(mg) => t.push_str(&format!("item\t{coll_name}\t{name}\t{}\t{}\t{}\n", waypoint_cs(wp), mg.no_respawn, if mg.trigger.is_some() { "NPlugTrigger_SWaypoint.NoRespawn" } else { "no trigger entity" })),
                Err(e) => t.push_str(&format!("item\t{coll_name}\t{name}\t{}\t?\tunreadable: {e}\n", waypoint_cs(wp))),
            }
        }
        std::fs::write(&cp, t).map_err(|e| format!("{cp}: {e}"))?;
        println!("wrote {cp}");
    }
    if let Some(tp) = flag(rest, "--trees-out") {
        let mut t = String::from("item\tsize_x\tsize_y\tsize_z\n");
        trees.sort_by(|a, b| a.0.cmp(&b.0));
        for (n, sz) in &trees { t.push_str(&format!("{n}\t{}\t{}\t{}\n", f3(sz[0]), f3(sz[1]), f3(sz[2]))); }
        std::fs::write(&tp, t).map_err(|e| format!("{tp}: {e}"))?;
        println!("wrote {tp}: {} vegetation items", trees.len());
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
    let header = format!("// generated by tman.Generator from {label}\n");
    let namespace = flag(rest, "--namespace").unwrap_or_else(|| "tman.Core".into());
    // `#nullable enable`: the compiler treats *.g.cs as auto-generated and wants the context explicit (CS8669)
    let common_s = format!("#nullable enable\nusing System.Collections.Generic;\nusing GBX.NET;\n\nnamespace {namespace};\n\n");
    let common = common_s.as_str();
    let enums = "[System.Flags]\npublic enum Collections { Stadium = 1, RedIsland = 2, WhiteShore = 4, GreenCoast = 8, BlueBay = 16, All = 31 }\n\npublic enum Waypoint { None, Start, Finish, Checkpoint, StartFinish }\n\n";
    // Blocks.g.cs
    let mut cs = String::new();
    cs.push_str(&header);
    cs.push_str(common);
    if !catalogs_only {
    cs.push_str(enums);
    cs.push_str("public readonly record struct BlockVariant(Block Model, int Variant);\n\n");
    cs.push_str(&format!("public sealed record Block(string Id, int SizeX, int SizeZ, {}int[][]? Cells, Collections Collections, Waypoint Waypoint, bool Ground, bool Air, int Variants)\n{{\n", if size_y_field { "int SizeY, " } else { "" }));
    cs.push_str("    public Block? Plain => Blocks.PlainOf.TryGetValue(Id, out var plain) ? plain : null;\n\n    public Block? As(Waypoint kind) => kind == Waypoint ? this : Blocks.Families.TryGetValue((Id, kind), out var twin) ? twin : null;\n\n    public BlockVariant Variant(int n) => new(this, n);\n}\n\n");
    }
    cs.push_str("public static class Blocks\n{\n");
    let mut block_idents: Vec<(String, String)> = Vec::new();
    for (id, b) in &blocks {
        let ident = ident_of(id);
        let cells = match &b.cells {
            None => "null".to_string(),
            Some(c) => format!("new[] {{ {} }}", c.iter().map(|(x, z)| format!("new[] {{ {x}, {z} }}")).collect::<Vec<_>>().join(", ")),
        };
        let sy = if size_y_field { format!("{}, ", b.size_y) } else { String::new() };
        cs.push_str(&format!("    public static readonly Block {ident} = new(\"{id}\", {}, {}, {sy}{cells}, {}, {}, {}, {}, {});\n", b.size.0, b.size.1, collections_cs(b.collections), waypoint_cs(b.waypoint), b.ground, b.air, b.variants));
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
    cs.push_str("    };\n");
    let block_wps: BTreeMap<String, Option<i32>> = blocks.iter().map(|(k, b)| (k.clone(), b.waypoint)).collect();
    cs.push_str(&families_cs("Block", &families(&block_wps, false), &ident_lookup));
    cs.push_str("}\n");
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
    if !catalogs_only {
    ci.push_str("public readonly record struct ItemVariant(Item Model, int Variant);\n\n");
    ci.push_str("public sealed record Item(string Id, string Author, string? Path, Collections Collections, Waypoint Waypoint, int Variants, Vec3 Size)\n{\n    public Item? As(Waypoint kind) => kind == Waypoint ? this : Items.Families.TryGetValue((Id, kind), out var twin) ? twin : null;\n\n    public ItemVariant Variant(int n) => new(this, n);\n}\n\n");
    }
    ci.push_str("public static class Items\n{\n");
    let mut item_idents: Vec<(String, String)> = Vec::new();
    for (id, it) in &items {
        let ident = ident_of2(id);
        ci.push_str(&format!("    public static readonly Item {ident} = new(\"{id}\", \"Nadeo\", null, {}, {}, {}, new Vec3({}f, {}f, {}f));\n", collections_cs(it.collections), waypoint_cs(it.waypoint), it.variants, f3(it.size[0]), f3(it.size[1]), f3(it.size[2])));
        item_idents.push((ident, id.clone()));
    }
    ci.push_str("\n    public static readonly IReadOnlyDictionary<string, Item> ById = new Dictionary<string, Item>\n    {\n");
    for (ident, id) in &item_idents { ci.push_str(&format!("        [\"{id}\"] = {ident},\n")); }
    ci.push_str("    };\n\n    public static Item? FromId(string id) => ById.TryGetValue(id, out var item) ? item : null;\n");
    let item_wps: BTreeMap<String, Option<i32>> = items.iter().map(|(k, b)| (k.clone(), b.waypoint)).collect();
    let item_lookup: BTreeMap<&str, &str> = item_idents.iter().map(|(i, id)| (id.as_str(), i.as_str())).collect();
    ci.push_str(&families_cs("Item", &families(&item_wps, true), &item_lookup));
    ci.push_str("}\n");
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
            cc.push_str(&format!("    public static readonly Item {ident} = new(\"{file}\", \"{file}\", \"{rel}\", Collections.All, {}, 1, new Vec3({}f, {}f, {}f));\n", waypoint_cs(waypoint), f3(size[0]), f3(size[1]), f3(size[2])));
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
    {
        let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
        for b in blocks.values() { *hist.entry(b.size_y).or_default() += 1; }
        println!("block heights (cells): {}", hist.iter().map(|(h, n)| format!("{h} → {n}")).collect::<Vec<_>>().join(", "));
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
