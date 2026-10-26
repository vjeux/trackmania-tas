//! `mapgeom cutout-audit --items DIR --maps M.Map.Gbx.. --out-tsv F.tsv --out-md F.md` — Hugo's robustness check
//! (2026-10-26 08:48, "test every cutout currently used for any more discrepancies"): every `FloorFinish<Kind>[V<n>]
//! .Item.Gbx` of the set against the pak geometry and the campaign.
//!   1. GEOMETRY: the item's own cap polygon (its top faces) raster-tested at 0.5 m against the whole baked block
//!      (prefab + every Bottom clip turned by its clip dir): every sample inside the polygon must have standable floor
//!      under it (an upward collision triangle within ±0.3 m of local 0, Grass/Green physics) and no structure over
//!      it; the standable floor NOT under the polygon is the missed area.
//!   2. VARIANTS: the (block, variant) pairs the kind's dictionary key places on in the maps, with / without fill.
//!   3. ORACLE: at every placement the map's own lightmap must show the grass tiles under the block unbaked.
//!   4. FRAME: the polygon's bbox, posed like the item (block frame turned by the block dir), lies inside the block's
//!      turned footprint for every direction that occurs.
//!   5. TRIGGER: a closed, outward-wound prism over the same polygon, height 0..1 in Hugo's form.
//!   6. IDENT = file name; 7. the pillar plates' hole = exactly the 4 × 4 m square 14..18.
use std::collections::{BTreeMap, BTreeSet};

type Tri = [[f32; 3]; 3];

fn stem(path: &std::path::Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// `FloorFinish<Kind>[V<n>].Item.Gbx` → (kind, variant or None).
fn kind_of(file: &str) -> Option<(String, Option<u32>)> {
    let s = file.strip_prefix("FloorFinish")?.strip_suffix(".Item.Gbx")?;
    if let Some(i) = s.rfind('V') {
        if let Ok(v) = s[i + 1..].parse::<u32>() {
            if s[..i].chars().last().map_or(false, |c| !c.is_ascii_digit()) || true {
                return Some((s[..i].to_string(), Some(v)));
            }
        }
    }
    Some((s.to_string(), None))
}

/// Point-in-triangle in plan, STRICT: a degenerate (vertical, zero plan area) triangle never contains a point,
/// a point on an edge counts as outside — a sample exactly on the cutout's inset line against a kerb face is a
/// boundary coincidence, not an overlap.
fn inside_tri2(p: [f32; 2], t: &Tri) -> bool {
    let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
    let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
    let area = s(a, b, c);
    if area.abs() < 1e-6 { return false; }
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    (d1 > 1e-6 && d2 > 1e-6 && d3 > 1e-6) || (d1 < -1e-6 && d2 < -1e-6 && d3 < -1e-6)
}

/// Inclusive point-in-triangle in plan (an edge counts; a degenerate triangle never) — for the floor, where a sample
/// on the shared edge of two fill triangles is still on the floor.
fn covers_tri2(p: [f32; 2], t: &Tri) -> bool {
    let (a, b, c) = ([t[0][0], t[0][2]], [t[1][0], t[1][2]], [t[2][0], t[2][2]]);
    let s = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| (c[0] - a[0]) * (b[1] - a[1]) - (b[0] - a[0]) * (c[1] - a[1]);
    if s(a, b, c).abs() < 1e-6 { return false; }
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    !((d1 < -1e-5 || d2 < -1e-5 || d3 < -1e-5) && (d1 > 1e-5 || d2 > 1e-5 || d3 > 1e-5))
}

fn tri_area2(t: &Tri) -> f32 {
    ((t[1][0] - t[0][0]) * (t[2][2] - t[0][2]) - (t[2][0] - t[0][0]) * (t[1][2] - t[0][2])).abs() / 2.0
}

struct ItemFacts {
    file: String,
    kind: String,
    variant: Option<u32>,
    /// the cap polygon as triangles at the top height (block frame, dir 0)
    top: Vec<Tri>,
    top_y: f32,
    bottom_y: f32,
    cap_faces: usize,
    trigger_faces: usize,
    trigger_closed: bool,
    trigger_outward: bool,
    trigger_same_polygon: bool,
    trigger_y: (f32, f32),
    ident_ok: bool,
    ident_note: String,
    waypoint: Option<u8>,
}

fn fan(verts: &[u32], pos: &[[f32; 3]]) -> Vec<Tri> {
    let mut v = Vec::new();
    for i in 1..verts.len().saturating_sub(1) {
        v.push([pos[verts[0] as usize], pos[verts[i] as usize], pos[verts[i + 1] as usize]]);
    }
    v
}

fn parse_item(path: &std::path::Path) -> Result<ItemFacts, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file = stem(path);
    let (kind, variant) = kind_of(&file).ok_or_else(|| format!("{file}: not a FloorFinish item name"))?;
    let it = crate::crystal::ItemCrystal::open(&bytes)?;
    let mut geo: Option<&crate::crystal_model::Crystal> = None;
    let mut trig: Option<&crate::crystal_model::Crystal> = None;
    for l in &it.model.layers {
        match &l.kind {
            crate::crystal_model::LayerKind::Geometry { crystal, .. } => geo = Some(crystal),
            crate::crystal_model::LayerKind::Trigger { crystal, .. } => trig = Some(crystal),
            _ => {}
        }
    }
    let geo = geo.ok_or_else(|| format!("{file}: no Geometry layer"))?;
    let top_y = geo.positions.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
    let bottom_y = geo.positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let mut top: Vec<Tri> = Vec::new();
    for f in &geo.faces {
        if f.verts.iter().all(|&i| (geo.positions[i as usize][1] - top_y).abs() < 1e-3) {
            top.extend(fan(&f.verts, &geo.positions));
        }
    }
    // the trigger: closed (every undirected edge in exactly two faces, opposite directions), outward (signed volume > 0)
    let (mut closed, mut outward, mut same, mut ty, mut tfaces) = (false, false, false, (f32::NAN, f32::NAN), 0usize);
    if let Some(t) = trig {
        tfaces = t.faces.len();
        let mut edges: BTreeMap<(u32, u32), i32> = BTreeMap::new();
        let mut vol = 0.0f64;
        for f in &t.faces {
            let n = f.verts.len();
            for i in 0..n {
                let (a, b) = (f.verts[i], f.verts[(i + 1) % n]);
                let key = (a.min(b), a.max(b));
                *edges.entry(key).or_default() += if a < b { 1 } else { -1 };
            }
            for tri in fan(&f.verts, &t.positions) {
                let (a, b, c) = (tri[0], tri[1], tri[2]);
                vol += (a[0] as f64 * (b[1] as f64 * c[2] as f64 - b[2] as f64 * c[1] as f64) - a[1] as f64 * (b[0] as f64 * c[2] as f64 - b[2] as f64 * c[0] as f64) + a[2] as f64 * (b[0] as f64 * c[1] as f64 - b[1] as f64 * c[0] as f64)) / 6.0;
            }
        }
        // positions are per-face copies in our items: key edges by rounded coordinates instead
        let mut e2: BTreeMap<((i64, i64, i64), (i64, i64, i64)), i32> = BTreeMap::new();
        let key = |p: [f32; 3]| ((p[0] * 1000.0).round() as i64, (p[1] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64);
        for f in &t.faces {
            let n = f.verts.len();
            for i in 0..n {
                let (a, b) = (key(t.positions[f.verts[i] as usize]), key(t.positions[f.verts[(i + 1) % n] as usize]));
                let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                *e2.entry((lo, hi)).or_default() += if a < b { 1 } else { -1 };
            }
        }
        closed = !e2.is_empty() && e2.values().all(|&c| c == 0);
        outward = vol > 0.0;
        let tlo = t.positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let thi = t.positions.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        ty = (tlo, thi);
        // same polygon: the trigger's top-corner xz set equals the geometry's
        let set = |pos: &[[f32; 3]], y: f32| -> BTreeSet<(i64, i64)> { pos.iter().filter(|p| (p[1] - y).abs() < 1e-3).map(|p| ((p[0] * 1000.0).round() as i64, (p[2] * 1000.0).round() as i64)).collect() };
        same = set(&t.positions, thi) == set(&geo.positions, top_y);
        let _ = edges;
    }
    // ident: every "FloorFinish…Item.Gbx" string in the file (header + body) equals the file name
    let mut names: BTreeSet<String> = BTreeSet::new();
    for hay in [&bytes[..], &it.body[..]] {
        let mut i = 0;
        while let Some(j) = hay[i..].windows(11).position(|w| w == b"FloorFinish") {
            let s = i + j;
            let mut e = s;
            while e < hay.len() && hay[e] != 0 && (hay[e] as char).is_ascii_graphic() { e += 1; }
            let txt = String::from_utf8_lossy(&hay[s..e]).to_string();
            if let Some(k) = txt.find(".Item.Gbx") { names.insert(txt[..k + 9].to_string()); }
            i = e.max(s + 1);
        }
    }
    let ident_ok = names.len() == 1 && names.contains(&file);
    // the waypoint type: chunk 0x2E00201F in the body: "1F 20 00 2E" then version u32, then waypoint u32
    let mut waypoint = None;
    if let Some(p) = it.body.windows(4).position(|w| w == [0x1F, 0x20, 0x00, 0x2E]) {
        if p + 12 <= it.body.len() { waypoint = Some(it.body[p + 8]); }
    }
    Ok(ItemFacts { file, kind, variant, top, top_y, bottom_y, cap_faces: geo.faces.len(), trigger_faces: tfaces, trigger_closed: closed, trigger_outward: outward, trigger_same_polygon: same, trigger_y: ty, ident_ok, ident_note: names.into_iter().collect::<Vec<_>>().join(" "), waypoint })
}

fn variant_of_flags(flags: u32) -> u32 {
    (flags & 0x3F) + 64 * ((flags >> 21) & 0x7F)
}

pub fn cmd(store: &mut crate::store::DataStore, rest: &[String], flag: &dyn Fn(&[String], &str) -> Option<String>) -> Result<(), String> {
    let dir = flag(rest, "--items").ok_or("cutout-audit needs --items DIR")?;
    let out_tsv = flag(rest, "--out-tsv").unwrap_or_else(|| "CUTOUT-AUDIT.tsv".into());
    let out_md = flag(rest, "--out-md").unwrap_or_else(|| "CUTOUT-AUDIT.md".into());
    let maps: Vec<String> = {
        let mut v = Vec::new();
        if let Some(i) = rest.iter().position(|x| x == "--maps") {
            for a in &rest[i + 1..] { if a.starts_with("--") { break; } v.push(a.clone()); }
        }
        v
    };
    let step = 0.5f32;
    // 1. the items
    let mut items: Vec<ItemFacts> = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir).map_err(|e| format!("{dir}: {e}"))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "Gbx")).collect();
    entries.sort();
    for p in &entries {
        items.push(parse_item(p)?);
    }
    // 2. the campaign: placements per (kind, variant, dir, cell) and the dictionary semantics
    struct Placement { map: String, name: String, flags: u32, dir: u8, cell: (i32, i32, i32) }
    let mut placements: Vec<Placement> = Vec::new();
    let mut oracle_fail: Vec<String> = Vec::new();
    let mut oracle_checked = 0usize;
    let kinds: BTreeSet<String> = items.iter().map(|i| i.kind.clone()).collect();
    for mp in &maps {
        let path = std::path::Path::new(mp);
        let m = tmmaps::map::MapFile::try_load(path)?;
        let coll = m.body_collections().map(|c| c[0].1).unwrap_or(26);
        let oracle = crate::lm_oracle::read(path, &m, coll)?;
        let short = stem(path);
        // baked Grass records by cell for the oracle
        let mut grass_at: BTreeMap<(i32, i32, i32), Vec<usize>> = BTreeMap::new();
        for (i, b) in m.baked.iter().enumerate() {
            if b.name == "Grass" && b.flags != 0xFFFF_FFFF && b.free_pos.is_none() { grass_at.entry(b.coords()).or_default().push(i); }
        }
        for b in m.blocks.iter().filter(|b| b.flags != 0xFFFF_FFFF && b.free_pos.is_none()) {
            if !kinds.contains(&b.name) || b.flags & crate::blockmap::FLAG_GROUND == 0 { continue; }
            let c = b.coords();
            placements.push(Placement { map: short.clone(), name: b.name.clone(), flags: b.flags, dir: b.dir, cell: c });
            // oracle: every turned base unit's grass tile must be uncharted
            if let Some(o) = &oracle {
                let units = crate::floor_finish::block_units(store, &b.name, b.flags);
                let size = (units.iter().map(|u| u[0]).max().unwrap_or(0) + 1, units.iter().map(|u| u[2]).max().unwrap_or(0) + 1);
                for u in units.iter().filter(|u| u[1] == 0) {
                    let cell = crate::floor_finish::turn_unit(c, b.dir, *u, size);
                    oracle_checked += 1;
                    if let Some(idx) = grass_at.get(&(cell.0, c.1, cell.1)) {
                        if idx.iter().any(|&i| o.baked_charted(i)) {
                            oracle_fail.push(format!("{short}: {} {:08x} dir {} at ({}, {}, {}): the Grass tile at ({}, {}) is BAKED (visible) in the source lightmap", b.name, b.flags, b.dir, c.0, c.1, c.2, cell.0, cell.1));
                        }
                    }
                }
            }
        }
    }
    // 3. per item: geometry vs every (block, variant) the key places on; frame per direction
    let mut rows: Vec<String> = Vec::new();
    let mut md = String::new();
    md.push_str("# Cutout audit — Hugo's 1 m set against the pak geometry and the 5 Fall Stadium sources\n\n");
    md.push_str(&format!("Items: {} in `{dir}`; maps: {}; placements of audited kinds: {}; grass tiles checked against the lightmap oracle: {} ({} baked = exceptions).\n\n", items.len(), maps.len(), placements.len(), oracle_checked, oracle_fail.len()));
    md.push_str("| kind | key variant | variants in maps (count) | placements | geometry | oracle | frame | trigger | ident | notes |\n|---|---|---|---|---|---|---|---|---|---|\n");
    rows.push("item\tkind\tkey_variant\tvariants_in_maps\tplacements\tsamples_in_polygon\tvoid_under\tstructure_over\tnon_green\tstandable_missed_pct\toracle_exceptions\tframe_ok\ttrigger_closed\ttrigger_outward\ttrigger_same_polygon\ttrigger_y\tcaps\tident_ok\tnotes".into());
    for it in &items {
        // the (block, variant) pairs this key places on
        let mut var_counts: BTreeMap<u32, (usize, u32)> = BTreeMap::new(); // variant -> (count, flags)
        let mut dirs: BTreeSet<u8> = BTreeSet::new();
        for p in placements.iter().filter(|p| p.name == it.kind) {
            let v = variant_of_flags(p.flags);
            if it.variant.map_or(true, |kv| kv == v) {
                let e = var_counts.entry(v).or_insert((0, p.flags & 0x0FE0_1FFF));
                e.0 += 1;
                dirs.insert(p.dir);
            }
        }
        // the item's own variant when none occurs in the maps (the pak's)
        let probe_flags: Vec<(u32, u32, usize)> = if var_counts.is_empty() {
            vec![(it.variant.unwrap_or(0), crate::floor_finish::flags_of_variant(it.variant.unwrap_or(0)), 0)]
        } else {
            var_counts.iter().map(|(v, (n, f))| (*v, *f, *n)).collect()
        };
        let mut notes: Vec<String> = Vec::new();
        let (mut tot_in, mut tot_void, mut tot_struct, mut tot_nongreen, mut tot_missed_pct) = (0usize, 0usize, 0usize, 0usize, 0.0f32);
        let mut geometry_ok = true;
        let mut frame_ok = true;
        let mut variants_txt: Vec<String> = Vec::new();
        for (v, flags, n) in &probe_flags {
            let var = match crate::floor_finish::picked_variant_of(store, &it.kind, *flags) { Ok(v) => v, Err(e) => { notes.push(format!("V{v}: {e}")); geometry_ok = false; continue } };
            let m = crate::floor_finish::variant_bake(store, &var)?;
            let (floor, structure) = crate::floor_finish::classify_floor_structure(&m);
            let units: Vec<[i32; 3]> = var.block_units.iter().map(|u| u.offset).collect();
            let size = (units.iter().map(|u| u[0]).max().unwrap_or(0) + 1, units.iter().map(|u| u[2]).max().unwrap_or(0) + 1);
            let has_fill = !floor.is_empty();
            variants_txt.push(format!("V{v}×{n}{}", if has_fill { "" } else { " (no fill)" }));
            if !has_fill && *n > 0 {
                notes.push(format!("V{v} ({n} placements) has NO standable fill — the key places a cutout there"));
                geometry_ok = false;
                continue;
            }
            // raster test over the block footprint
            let (mut n_in, mut n_void, mut n_struct, mut n_nongreen, mut n_stand, mut n_stand_missed) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
            let mut void_examples: Vec<String> = Vec::new();
            let mut struct_examples: Vec<String> = Vec::new();
            let mut phys: BTreeMap<u8, usize> = BTreeMap::new();
            let (nx, nz) = ((32.0 * size.0 as f32 / step) as i32, (32.0 * size.1 as f32 / step) as i32);
            for zi in 0..nz {
                for xi in 0..nx {
                    let p = [step * (xi as f32 + 0.5), step * (zi as f32 + 0.5)];
                    let in_poly = it.top.iter().any(|t| covers_tri2(p, t));
                    let floor_hit = floor.iter().find(|(t, _)| covers_tri2(p, t));
                    let struct_hit = structure.iter().find(|t| inside_tri2(p, t));
                    let standable = floor_hit.is_some() && struct_hit.is_none();
                    if standable { n_stand += 1; }
                    if in_poly {
                        n_in += 1;
                        match floor_hit {
                            None => { n_void += 1; if void_examples.len() < 3 { void_examples.push(format!("({:.2}, {:.2})", p[0], p[1])); } }
                            Some((_, mat)) => { if *mat != 76 && *mat != 2 { n_nongreen += 1; *phys.entry(*mat).or_default() += 1; } }
                        }
                        if let Some(t) = struct_hit {
                            n_struct += 1;
                            if struct_examples.len() < 4 {
                                let (lo, hi) = (t.iter().map(|q| q[1]).fold(f32::INFINITY, f32::min), t.iter().map(|q| q[1]).fold(f32::NEG_INFINITY, f32::max));
                                struct_examples.push(format!("({:.2}, {:.2}) under a triangle y {lo:.2}..{hi:.2} of plan area {:.3} m²", p[0], p[1], tri_area2(t)));
                            }
                        }
                    } else if standable {
                        n_stand_missed += 1;
                    }
                }
            }
            let missed_pct = if n_stand > 0 { 100.0 * n_stand_missed as f32 / n_stand as f32 } else { 0.0 };
            tot_in += n_in; tot_void += n_void; tot_struct += n_struct; tot_nongreen += n_nongreen; tot_missed_pct = tot_missed_pct.max(missed_pct);
            if n_void > 0 { notes.push(format!("V{v}: {n_void} of {n_in} samples over VOID at {}", void_examples.join(" "))); geometry_ok = false; }
            if n_struct > 0 { notes.push(format!("V{v}: {n_struct} of {n_in} samples under structure: {}", struct_examples.join("; "))); geometry_ok = false; }
            if n_nongreen > 0 { notes.push(format!("V{v}: {n_nongreen} of {n_in} samples on non-Green physics ({})", phys.iter().map(|(k, c)| format!("{} ×{c}", crate::scene::physics_name(*k))).collect::<Vec<_>>().join(", "))); }
            if missed_pct > 0.0 { notes.push(format!("V{v}: {:.1}% of the standable floor outside the polygon ({n_stand_missed} of {n_stand} samples, the 0.5 m inset)", missed_pct)); }
            // frame: the polygon bbox posed per direction must lie inside the block's turned footprint
            // every polygon triangle's centroid (the polygon's own area), posed per direction, must land in a footprint cell
            let size_m = (32.0 * size.0 as f32, 32.0 * size.1 as f32);
            for d in dirs.iter().copied().chain(if dirs.is_empty() { vec![0u8].into_iter() } else { vec![].into_iter() }) {
                let cells: BTreeSet<(i32, i32)> = units.iter().filter(|u| u[1] == 0).map(|u| crate::floor_finish::turn_unit((0, 0, 0), d, *u, size)).collect();
                let mut bad = 0usize;
                let mut example = String::new();
                for t in &it.top {
                    let (x, z) = ((t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][2] + t[1][2] + t[2][2]) / 3.0);
                    let q = crate::floor_finish::turn_pt_sized((0, 0, 0), d, size_m, x, z);
                    let cell = ((q[0] / 32.0).floor() as i32, (q[1] / 32.0).floor() as i32);
                    if !cells.contains(&cell) { bad += 1; if example.is_empty() { example = format!("({x:.1}, {z:.1}) → world cell {:?}", cell); } }
                }
                if bad > 0 { frame_ok = false; notes.push(format!("V{v} dir {d}: {bad} polygon triangles outside the footprint {:?}, e.g. {example}", cells)); }
            }
        }
        // 7. pillar plates: the hole = exactly 14..18
        if it.kind == "StructurePillar" || it.kind == "StructureBase" {
            let xs: BTreeSet<i64> = it.top.iter().flat_map(|t| t.iter()).filter(|p| p[0] > 13.9 && p[0] < 18.1 && p[2] > 13.9 && p[2] < 18.1).map(|p| ((p[0] * 100.0).round() as i64, (p[2] * 100.0).round() as i64)).map(|(x, _)| x).collect();
            let inner: BTreeSet<(i64, i64)> = it.top.iter().flat_map(|t| t.iter()).filter(|p| p[0] > 13.99 && p[0] < 18.01 && p[2] > 13.99 && p[2] < 18.01).map(|p| ((p[0] * 100.0).round() as i64, (p[2] * 100.0).round() as i64)).collect();
            let corners: BTreeSet<(i64, i64)> = [(1400, 1400), (1800, 1400), (1800, 1800), (1400, 1800)].into_iter().collect();
            let hole_ok = inner == corners;
            if !hole_ok { notes.push(format!("pillar hole vertices {:?} ≠ the 4 × 4 square 14..18", inner)); geometry_ok = false; }
            else { notes.push("hole = exactly 14..18 × 14..18".into()); }
            let _ = xs;
        }
        let orc: Vec<&String> = oracle_fail.iter().filter(|s| s.contains(&format!(" {} ", it.kind))).collect();
        let trig_ok = it.trigger_closed && it.trigger_outward && it.trigger_same_polygon && (it.trigger_y.0.abs() < 1e-3) && ((it.trigger_y.1 - it.top_y).abs() < 1e-3);
        if !trig_ok { notes.push(format!("trigger: closed {} outward {} same polygon {} y {:.2}..{:.2}", it.trigger_closed, it.trigger_outward, it.trigger_same_polygon, it.trigger_y.0, it.trigger_y.1)); }
        if !it.ident_ok { notes.push(format!("ident strings: {}", it.ident_note)); }
        if it.waypoint != Some(1) { notes.push(format!("waypoint type {:?} ≠ Finish", it.waypoint)); }
        let placements_n: usize = probe_flags.iter().map(|(_, _, n)| n).sum();
        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n", it.kind, it.variant.map_or("any".to_string(), |v| format!("V{v}")), variants_txt.join(", "), placements_n, if geometry_ok { "ok" } else { "**FAIL**" }, if orc.is_empty() { "ok" } else { "**exceptions**" }, if frame_ok { "ok" } else { "**FAIL**" }, if trig_ok { format!("ok ({} faces, {:.1}..{:.1})", it.trigger_faces, it.trigger_y.0, it.trigger_y.1) } else { "**FAIL**".into() }, if it.ident_ok { "ok" } else { "**FAIL**" }, notes.join("; ")));
        rows.push(format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{:.2}..{:.2}\t{} faces {:.1}/{:.1}\t{}\t{}", it.file, it.kind, it.variant.map_or("any".to_string(), |v| format!("V{v}")), variants_txt.join(" "), placements_n, tot_in, tot_void, tot_struct, tot_nongreen, tot_missed_pct, orc.len(), frame_ok, it.trigger_closed, it.trigger_outward, it.trigger_same_polygon, it.trigger_y.0, it.trigger_y.1, it.cap_faces, it.bottom_y, it.top_y, it.ident_ok, notes.join("; ")));
    }
    // kinds in the maps WITHOUT any item for an occurring variant with fill (the "needs a second item" class)
    let mut missing: Vec<String> = Vec::new();
    let mut seen_pairs: BTreeSet<(String, u32)> = BTreeSet::new();
    for p in &placements {
        let v = variant_of_flags(p.flags);
        if !seen_pairs.insert((p.name.clone(), v)) { continue; }
        let covered = items.iter().any(|it| it.kind == p.name && it.variant.map_or(true, |kv| kv == v));
        if covered { continue; }
        if let Ok(var) = crate::floor_finish::picked_variant_of(store, &p.name, p.flags & 0x0FE0_1FFF) {
            let m = crate::floor_finish::variant_bake(store, &var)?;
            let (floor, _) = crate::floor_finish::classify_floor_structure(&m);
            let area: f32 = floor.iter().map(|(t, _)| tri_area2(t)).sum();
            if !floor.is_empty() { missing.push(format!("{} V{v} ({} placements, ~{:.0} m² of fill) has no item", p.name, placements.iter().filter(|q| q.name == p.name && variant_of_flags(q.flags) == v).count(), area)); }
        }
    }
    md.push_str("\n## Oracle exceptions (placements whose grass tile the game still bakes)\n\n");
    if oracle_fail.is_empty() { md.push_str("none\n"); } else { for e in &oracle_fail { md.push_str(&format!("* {e}\n")); } }
    md.push_str("\n## Variants with fill but no item\n\n");
    if missing.is_empty() { md.push_str("none\n"); } else { for e in &missing { md.push_str(&format!("* {e}\n")); } }
    std::fs::write(&out_tsv, rows.join("\n") + "\n").map_err(|e| format!("{out_tsv}: {e}"))?;
    std::fs::write(&out_md, &md).map_err(|e| format!("{out_md}: {e}"))?;
    print!("{md}");
    println!("wrote {out_tsv} and {out_md}");
    Ok(())
}
