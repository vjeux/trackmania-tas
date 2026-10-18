//! Hugo's Grass alteration on the vista collections (2026-10-18): the terrain-integrated block
//! variants — `<Base>On<Dirt|Grass|Land|Water><Cliff|Hill><N>[SlopeBase…]` — are COMPOSED prefabs: the
//! collection pak's `<Variant>…Prefab.Gbx` places the Stadium base block's own mobil prefab
//! (`Stadium\Media\Prefab\RoadTech\Straight_Air.Prefab.Gbx` for RoadTechStraight…) as a sub-entity
//! beside the cliff / hill terrain pieces. So the deck geometry is the base block's by construction,
//! and the base block's placement inside the variant block = that sub-entity's transform. This module
//! reads it off the prefab (and cross-checks it on the collision triangles) for a list of variant
//! block names.
use std::collections::{BTreeMap, HashMap};

use crate::geom::{apply, compose, from_quat, Xform, IDENTITY};
use crate::stands::{prefab_triangles, MatTri};
use crate::store::DataStore;

/// One deck placement found in a variant block.
#[derive(Clone, Debug)]
pub struct Deck {
    pub variant: String,
    pub variant_path: String,
    pub base: String,
    pub base_prefab: String,
    /// The base prefab's transform inside the variant block's frame.
    pub iso: Xform,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub dyaw: f32,
    /// Variant footprint in cells (x, y, z), ground base variant.
    pub size: [i32; 3],
    /// Fraction of the base prefab's collision triangles found (same centroid, < 1 mm) in the variant's.
    pub deck_match: f32,
    pub base_tris: usize,
    pub notes: Vec<String>,
    /// Further Stadium-block pieces the variant embeds besides the primary deck (a diag "straight" = the X2 diag
    /// deck + a plain straight segment at the diagonal angle): (base block, dx, dy, dz, dyaw).
    pub extra: Vec<(String, f32, f32, f32, f32)>,
}

/// `(prefab path, uppercase) → base block names` for every Stadium block whose ground (or air)
/// base variant's first mobil draws that prefab.
pub fn base_prefab_index(store: &mut DataStore) -> Result<HashMap<String, Vec<String>>, String> {
    // the Stadium pak's block infos AND the vista collections' own copies of Stadium blocks
    // (`<Coll>\GameCtnBlockInfo\GameCtnBlockInfoClassic\Stadium\<Name>`: RedIsland redefines RoadIceSlopeBase with a
    // `SlopeBase_TrueNPB_Air` prefab) — the variant blocks are excluded by name
    let is_variant = |n: &str| ["OnDirt", "OnGrass", "OnLand", "OnWater"].iter().any(|k| n.contains(k)) && (n.contains("Cliff") || n.contains("Hill"));
    let paths: Vec<String> = store
        .entries()
        .map(|e| e.path())
        .filter(|p| {
            let u = p.to_uppercase();
            u.ends_with(".EDCLASSIC.GBX")
                && (u.starts_with("STADIUM\\GAMECTNBLOCKINFO\\GAMECTNBLOCKINFOCLASSIC\\") || u.contains("\\GAMECTNBLOCKINFO\\GAMECTNBLOCKINFOCLASSIC\\STADIUM\\"))
                && !is_variant(p.rsplit('\\').next().unwrap_or(p))
        })
        .collect();
    let mut idx: HashMap<String, Vec<String>> = HashMap::new();
    for p in &paths {
        let Ok(bi) = crate::blockinfo::load(store, p) else { continue };
        let name = p.rsplit('\\').next().unwrap_or(p).trim_end_matches(".EDClassic.Gbx").to_string();
        // base variants first, then the additional ones (NPB = no pillar below, InPillar…): a variant block
        // may embed either form of the deck
        let mut all: Vec<(&crate::blockinfo::Variant, String)> = Vec::new();
        for (v, tag) in [(&bi.variant_base_ground, "ground"), (&bi.variant_base_air, "air")] {
            if let Some(v) = v {
                all.push((v, tag.to_string()));
            }
        }
        for (i, v) in bi.additional_ground.iter().enumerate() {
            all.push((v, format!("ground/add{i} \"{}\"", v.name)));
        }
        for (i, v) in bi.additional_air.iter().enumerate() {
            all.push((v, format!("air/add{i} \"{}\"", v.name)));
        }
        for (v, tag) in all {
            for list in &v.mobils {
                for m in list {
                    if let Some(pf) = &m.prefab {
                        let e = idx.entry(pf.to_uppercase()).or_default();
                        let entry = format!("{name}@{tag}");
                        if !e.iter().any(|x| x.split('@').next() == Some(&name)) {
                            e.push(entry);
                        }
                    }
                }
            }
        }
    }
    Ok(idx)
}

/// The block info path of a block name across every loaded pak (the `GameCtnBlockInfoClassic\…` tree,
/// any sub-folder).
pub fn find_block_info(store: &DataStore, name: &str) -> Option<String> {
    let want = format!("\\{}.EDCLASSIC.GBX", name.to_uppercase());
    store.entries().map(|e| e.path()).find(|p| {
        let u = p.to_uppercase();
        u.contains("\\GAMECTNBLOCKINFOCLASSIC\\") && u.ends_with(&want)
    })
}

fn yaw_of(m: &Xform) -> f32 {
    // the image of local +x (rotation only): world (cos ψ, 0, −sin ψ) for a yaw ψ about +y
    let d = [m[0], m[1], m[2]];
    (-d[2]).atan2(d[0])
}

/// Walk a prefab's entities (sub-prefabs recursively) and return every (prefab path, iso) pair.
fn sub_prefabs(store: &mut DataStore, path: &str, at: &Xform, depth: usize, out: &mut Vec<(String, Xform)>) -> Result<(), String> {
    if depth > 4 {
        return Ok(());
    }
    let model = store.load_model(path)?;
    let pf = crate::static_item::prefab::CPlugPrefab::from_model(&model)?;
    let ext = model.externals.clone();
    for e in &pf.ents {
        if e.model.inline.is_some() {
            continue;
        }
        let Some((_, p)) = ext.iter().find(|(k, _)| *k as i32 == e.model.index) else { continue };
        if !p.to_ascii_lowercase().ends_with(".prefab.gbx") {
            continue;
        }
        let iso = compose(at, &from_quat(e.rot, e.pos));
        out.push((p.clone(), iso));
        sub_prefabs(store, p, &iso, depth + 1, out)?;
    }
    Ok(())
}

fn centroid_key(t: &[[f32; 3]; 3]) -> (i64, i64, i64) {
    let c = [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
        (t[0][2] + t[1][2] + t[2][2]) / 3.0,
    ];
    ((c[0] * 1000.0).round() as i64, (c[1] * 1000.0).round() as i64, (c[2] * 1000.0).round() as i64)
}

/// The deck placement of one variant block.
pub fn deck_of(store: &mut DataStore, index: &HashMap<String, Vec<String>>, variant: &str) -> Result<Deck, String> {
    let path = find_block_info(store, variant).ok_or_else(|| format!("{variant}: no block info in the loaded paks"))?;
    let bi = crate::blockinfo::load(store, &path)?;
    let v = bi
        .variant_base_ground
        .as_ref()
        .filter(|v| !v.block_units.is_empty())
        .or(bi.variant_base_air.as_ref())
        .ok_or_else(|| format!("{variant}: no base variant"))?;
    let mut size = [0i32; 3];
    for u in &v.block_units {
        for k in 0..3 {
            size[k] = size[k].max(u.offset[k] + 1);
        }
    }
    let mut notes = Vec::new();
    let mobil = v.mobils.first().and_then(|l| l.first()).ok_or_else(|| format!("{variant}: no mobil"))?;
    if v.mobils.len() > 1 || v.mobils[0].len() > 1 {
        notes.push(format!("{} mobil lists / {} mobils in list 0: measured on mobil[0][0]", v.mobils.len(), v.mobils[0].len()));
    }
    let vprefab = mobil.prefab.clone().ok_or_else(|| format!("{variant}: mobil[0][0] has no prefab"))?;
    // the variant's own mobil transform (tiny_library.rs: the `Road*OnLandHillSlopeBase2x1` block infos author
    // their prefab turned 180° about Y and moved by (32, 0, 64) so it lands back on the 1×2 footprint)
    let mut root = IDENTITY;
    if let Some(t) = mobil.translation {
        root[9] = t[0];
        root[10] = t[1];
        root[11] = t[2];
    }
    if let Some(r) = mobil.rotation.filter(|r| r.iter().any(|v| v.abs() > 1e-6)) {
        if r[0].abs() > 1e-6 || r[2].abs() > 1e-6 {
            return Err(format!("{variant}: mobil rotation {r:?} has an x/z component; only a yaw is implemented"));
        }
        root = crate::geom::yaw(r[1].to_radians(), [root[9], root[10], root[11]]);
    }
    if mobil.translation.is_some() || mobil.rotation.is_some() {
        notes.push(format!("variant mobil transform: translation {:?} rotation {:?} applied", mobil.translation, mobil.rotation));
    }
    let mut subs = Vec::new();
    sub_prefabs(store, &vprefab, &root, 0, &mut subs)?;
    // the sub-prefab that is some Stadium block's own mobil prefab = the deck
    let mut hits: Vec<(String, Xform, Vec<String>)> = subs
        .iter()
        .filter_map(|(p, iso)| index.get(&p.to_uppercase()).map(|bases| (p.clone(), *iso, bases.clone())))
        .collect();
    if hits.is_empty() {
        return Err(format!("{variant}: none of the {} sub-prefabs is a Stadium block prefab: {:?}", subs.len(), subs.iter().map(|(p, _)| p.rsplit('\\').next().unwrap_or(p).to_string()).collect::<Vec<_>>()));
    }
    let all_hits = hits.clone();
    // prefer the base whose name is the variant's name with the On… segment removed
    let stripped = strip_variant(variant);
    let eq = |b: &str| b.split('@').next().unwrap_or(b).eq_ignore_ascii_case(&stripped);
    // among the blocks sharing a prefab (PlatformDirtBase/…/PlatFormTechBase; OpenDirtRoadSlope2Base/…/OpenTechRoadSlope2Base)
    // the variant's own surface family = the longest case-insensitive common prefix with the variant name
    let common = |b: &str| -> usize {
        let b = b.split('@').next().unwrap_or(b).to_ascii_lowercase();
        let v = variant.to_ascii_lowercase();
        b.chars().zip(v.chars()).take_while(|(x, y)| x == y).count()
    };
    hits.sort_by_key(|(_, _, bases)| if bases.iter().any(|b| eq(b)) { 0 } else { 1 });
    let (bprefab, iso, bases) = hits.remove(0);
    let chosen = bases.iter().find(|b| eq(b)).cloned().unwrap_or_else(|| bases.iter().max_by_key(|b| common(b)).cloned().unwrap());
    let extra: Vec<(String, f32, f32, f32, f32)> = all_hits
        .iter()
        .filter(|(p, _, _)| !p.eq_ignore_ascii_case(&bprefab))
        .map(|(_, eiso, eb)| {
            let name = eb.iter().max_by_key(|b| common(b)).cloned().unwrap_or_default();
            let name = name.split('@').next().unwrap_or(&name).to_string();
            (name, eiso[9], eiso[10], eiso[11], yaw_of(eiso))
        })
        .collect();
    let (base, tag) = chosen.split_once('@').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((chosen.clone(), String::new()));
    if !base.eq_ignore_ascii_case(&stripped) {
        notes.push(format!("base '{base}' chosen by name family among the blocks sharing the prefab ({}); the literal strip would say '{stripped}'", bases.iter().map(|b| b.split('@').next().unwrap_or(b)).collect::<Vec<_>>().join("/")));
    } else if bases.len() > 1 {
        notes.push(format!("prefab shared by {}", bases.iter().map(|b| b.split('@').next().unwrap_or(b)).collect::<Vec<_>>().join("/")));
    }
    if tag != "ground" && tag != "air" {
        notes.push(format!("deck = the base's {} variant prefab {}", tag, bprefab.rsplit('\\').next().unwrap_or(&bprefab)));
    }
    // geometry cross-check: the base prefab's collision triangles under `iso` vs the variant's
    let mut vt: Vec<MatTri> = Vec::new();
    prefab_triangles(store, &vprefab, &root, 0, &mut vt)?;
    let vkeys: std::collections::HashSet<(i64, i64, i64)> = vt.iter().map(|t| centroid_key(&t.v)).collect();
    let mut bt: Vec<MatTri> = Vec::new();
    prefab_triangles(store, &bprefab, &iso, 0, &mut bt)?;
    let found = bt.iter().filter(|t| vkeys.contains(&centroid_key(&t.v))).count();
    let deck_match = if bt.is_empty() { 0.0 } else { found as f32 / bt.len() as f32 };
    // the stronger check: the BASE BLOCK's own ground mobil prefab (what Hugo's <Base>GrassSurface item is cut from)
    // under the same `iso` — which of ITS collision triangles the variant carries, per material
    if let Some(bpath) = find_block_info(store, &base) {
        if let Ok(bbi) = crate::blockinfo::load(store, &bpath) {
            let gv = bbi.variant_base_ground.as_ref().filter(|v| !v.block_units.is_empty()).or(bbi.variant_base_air.as_ref());
            if let Some(gp) = gv.and_then(|v| v.mobils.first()).and_then(|l| l.first()).and_then(|m| m.prefab.clone()) {
                let mut gt: Vec<MatTri> = Vec::new();
                if prefab_triangles(store, &gp, &iso, 0, &mut gt).is_ok() && !gt.is_empty() {
                    let mut per: BTreeMap<String, (usize, usize)> = BTreeMap::new();
                    for t in &gt {
                        let e = per.entry(t.material.trim_end_matches(".Material.Gbx").to_string()).or_insert((0, 0));
                        e.0 += 1;
                        if vkeys.contains(&centroid_key(&t.v)) {
                            e.1 += 1;
                        }
                    }
                    let total: usize = per.values().map(|e| e.0).sum();
                    let hit: usize = per.values().map(|e| e.1).sum();
                    let missing: Vec<String> = per.iter().filter(|(_, e)| e.1 < e.0).map(|(m, e)| format!("{m} {}/{}", e.1, e.0)).collect();
                    if gp.eq_ignore_ascii_case(&bprefab) {
                        notes.push(format!("base ground prefab = the embedded one ({hit}/{total} tris)"));
                    } else {
                        notes.push(format!("base GROUND prefab {} under the same pose: {hit}/{total} collision tris present in the variant{}", gp.rsplit('\\').next().unwrap_or(&gp), if missing.is_empty() { String::new() } else { format!("; missing: {}", missing.join(", ")) }));
                    }
                }
            }
        }
    }
    Ok(Deck {
        variant: variant.to_string(),
        variant_path: path,
        base,
        base_prefab: bprefab,
        iso,
        dx: iso[9],
        dy: iso[10],
        dz: iso[11],
        dyaw: yaw_of(&iso),
        size,
        deck_match,
        base_tris: bt.len(),
        notes,
        extra,
    })
}

/// `RoadTechStraightOnDirtCliff4` → `RoadTechStraight`; `RoadTechOnLandHillSlopeBase2x1` → `RoadTechSlopeBase2x1`;
/// `OpenTechRoadOnGrassHill1SlopeBase` → `OpenTechRoadSlopeBase`? — only a GUESS used to rank candidates; the
/// prefab identifies the base.
pub fn strip_variant(name: &str) -> String {
    let Some(i) = name.find("On") else { return name.to_string() };
    // the On… segment: On + (Dirt|Grass|Land|Water) + (Cliff|Hill) + digits
    let rest = &name[i + 2..];
    let mut j = 0;
    for terr in ["Dirt", "Grass", "Land", "Water"] {
        if rest.starts_with(terr) {
            j = terr.len();
            break;
        }
    }
    if j == 0 {
        return name.to_string();
    }
    let rest2 = &rest[j..];
    let mut k = 0;
    for shape in ["Cliff", "Hill"] {
        if rest2.starts_with(shape) {
            k = shape.len();
            break;
        }
    }
    if k == 0 {
        return name.to_string();
    }
    let rest3 = &rest2[k..];
    let digits = rest3.chars().take_while(|c| c.is_ascii_digit()).count();
    format!("{}{}", &name[..i], &rest3[digits..])
}

pub fn unused(_: &BTreeMap<String, String>) {}

/// A map ground block over terrain: nothing here yet.
pub fn describe(d: &Deck) -> String {
    format!(
        "{}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{}x{}x{}\t{:.1}% of {} tris\t{}",
        d.variant,
        d.base,
        d.dx,
        d.dy,
        d.dz,
        d.dyaw,
        d.size[0],
        d.size[1],
        d.size[2],
        d.deck_match * 100.0,
        d.base_tris,
        d.notes.join(" | ")
    )
}

pub fn _unused_apply(m: &Xform, v: [f32; 3]) -> [f32; 3] {
    apply(m, v)
}
