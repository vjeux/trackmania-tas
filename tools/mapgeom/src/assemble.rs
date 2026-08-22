//! A whole map, assembled: every block and item read by `tmmaps`, placed by
//! `place`, with each model's geometry read from the pack once and instanced.

use crate::geom::{Collector, Stats, Xform, IDENTITY};
use crate::place;
use crate::scene::Scene;
use crate::store::{DataStore, Model};
use std::collections::{BTreeMap, HashMap};
use tmmaps::map::{MapFile, FREE_BLOCK_FLAG};

/// One model's geometry in its own local frame, plus the footprint it implies.
pub struct LocalModel {
    pub scene: Scene,
    pub size: (f32, f32),
}

/// Where a block model lives in the pack, across the environments that carry
/// terrain-adapted copies of the Stadium set. Where a model exists in both,
/// the geometry is identical.
fn block_candidates(name: &str) -> Vec<String> {
    let mut v = vec![format!(
        "Stadium\\GameCtnBlockInfo\\GameCtnBlockInfoClassic\\{}.EDClassic.Gbx",
        name
    )];
    // `Stadium256` carries the stadium shell itself (`Stade4096`,
    // `Stade1536`), which is what a DECORATION map is made of.
    v.push(format!(
        "Stadium256\\GameCtnBlockInfo\\GameCtnBlockInfoClassic\\{}.EDClassic.Gbx",
        name
    ));
    for env in ["BlueBay", "GreenCoast", "RedIsland", "WhiteShore"] {
        v.push(format!(
            "{}\\GameCtnBlockInfo\\GameCtnBlockInfoClassic\\Stadium\\{}.EDClassic.Gbx",
            env, name
        ));
    }
    v
}

fn item_candidates(name: &str) -> Vec<String> {
    // A map spells an item either bare or with its extension already on.
    let base = name.trim_end_matches(".Item.Gbx");
    vec![
        format!("Stadium\\Items\\{}.Item.Gbx", base),
        format!("{}.Item.Gbx", base),
        base.to_string(),
    ]
}

pub struct Assembler<'a> {
    pub store: &'a mut DataStore,
    cache: HashMap<String, Option<LocalModel>>,
    /// The map's own embedded `.Item.Gbx` / `.Block.Gbx` files, by zip path.
    embedded: BTreeMap<String, Vec<u8>>,
    pub stats: Stats,
    /// Model name -> how many placements used it, and whether it had geometry.
    pub used: BTreeMap<String, (usize, bool)>,
}

/// The zip path a map's model name refers to, if it is a custom model.
///
/// A map spells an embedded block `FlinkIceBlocks\3-1-1-1-Ice-Light.Block.Gbx_CustomBlock`
/// and the zip holds it at `Items/FlinkIceBlocks/3-1-1-1-Ice-Light.Block.Gbx`.
fn embedded_key(name: &str) -> String {
    let n = name.trim_end_matches("_CustomBlock").replace('\\', "/");
    format!("items/{}", n.to_lowercase())
}

impl<'a> Assembler<'a> {
    pub fn new(store: &'a mut DataStore) -> Assembler<'a> {
        Assembler {
            store,
            cache: HashMap::new(),
            embedded: BTreeMap::new(),
            stats: Stats::default(),
            used: BTreeMap::new(),
        }
    }

    /// Take the map's embedded models. Call before `map`; a map with none is
    /// unaffected.
    pub fn with_embedded(&mut self, m: &MapFile) -> Result<usize, String> {
        self.embedded = crate::embedded::files(m)?
            .into_iter()
            .map(|(k, v)| (k.replace('\\', "/").to_lowercase(), v))
            .collect();
        Ok(self.embedded.len())
    }

    /// A model the map carries itself, if this name is one.
    fn embedded_model(&mut self, name: &str) -> Option<LocalModel> {
        let bytes = self.embedded.get(&embedded_key(name))?.clone();
        let model = match Model::parse(&bytes, name) {
            Ok(m) => m,
            Err(e) => {
                self.stats.missing.push((name.to_string(), e));
                return None;
            }
        };
        let mut c = Collector::new(self.store);
        c.model(&model, &IDENTITY, 0);
        let scene = std::mem::take(&mut c.scene);
        merge_stats(&mut self.stats, &c.stats);
        if scene.tri_count() == 0 {
            return None;
        }
        let hi = scene.max_corner();
        let size = place::footprint(hi[0], hi[2]);
        Some(LocalModel { scene, size })
    }

    /// A block model's geometry, in block-local metres.
    ///
    /// A `.EDClassic.Gbx` reaches its shape through variants and mobils; this
    /// takes the union of **every** prefab the model's reference table names.
    /// That is deliberately more than any single placement shows — the ground
    /// and air variants both come in — and it is safe for the question the
    /// model is built to answer, because the two variants differ in what is
    /// UNDER the road, not in where the road is. Variant selection is the next
    /// thing to build; see MAPGEOM.md.
    pub fn block_model(&mut self, name: &str) -> Option<&LocalModel> {
        let key = format!("block:{}", name);
        if !self.cache.contains_key(&key) {
            let built = self.build_block(name);
            self.cache.insert(key.clone(), built);
        }
        self.cache.get(&key).and_then(|o| o.as_ref())
    }

    fn build_block(&mut self, name: &str) -> Option<LocalModel> {
        if let Some(lm) = self.embedded_model(name) {
            return Some(lm);
        }
        let path = block_candidates(name).into_iter().find(|p| self.store.resolve(p).is_some())?;
        let model = self.store.load_model(&path).ok()?;
        let prefabs = model.refs_ending(".Prefab.Gbx");
        if prefabs.is_empty() {
            return None;
        }
        let mut c = Collector::new(self.store);
        for p in &prefabs {
            c.file(p, &IDENTITY, 0);
        }
        let scene = std::mem::take(&mut c.scene);
        merge_stats(&mut self.stats, &c.stats);
        let hi = scene.max_corner();
        let size = place::footprint(hi[0], hi[2]);
        Some(LocalModel { scene, size })
    }

    pub fn item_model(&mut self, name: &str) -> Option<&LocalModel> {
        let key = format!("item:{}", name);
        if !self.cache.contains_key(&key) {
            let built = self.build_item(name);
            self.cache.insert(key.clone(), built);
        }
        self.cache.get(&key).and_then(|o| o.as_ref())
    }

    fn build_item(&mut self, name: &str) -> Option<LocalModel> {
        if let Some(lm) = self.embedded_model(name) {
            return Some(lm);
        }
        let path = item_candidates(name).into_iter().find(|p| self.store.resolve(p).is_some())?;
        let mut c = Collector::new(self.store);
        c.file(&path, &IDENTITY, 0);
        let scene = std::mem::take(&mut c.scene);
        merge_stats(&mut self.stats, &c.stats);
        if scene.tri_count() == 0 {
            return None;
        }
        let hi = scene.max_corner();
        let size = place::footprint(hi[0], hi[2]);
        Some(LocalModel { scene, size })
    }

    /// Assemble a map into one scene.
    pub fn map(&mut self, m: &MapFile, yoff: f32, with_items: bool) -> Scene {
        let mut out = Scene::default();
        for b in &m.blocks {
            let free = b.flags & FREE_BLOCK_FLAG != 0;
            let xf: Xform = {
                let size = match self.block_model(&b.name) {
                    Some(lm) => lm.size,
                    None => {
                        self.note(&b.name, false);
                        continue;
                    }
                };
                if free {
                    match (b.free_pos, b.free_rot) {
                        (Some(p), Some(r)) => place::free(p, r),
                        (Some(p), None) => place::free(p, [0.0; 3]),
                        _ => continue,
                    }
                } else {
                    place::grid_block(b.coords(), b.dir, size, yoff)
                }
            };
            self.note(&b.name, true);
            if let Some(lm) = self.block_model(&b.name) {
                let s = &lm.scene;
                out.append(s, &xf);
            }
        }
        if with_items {
            for it in &m.items {
                let xf = place::free(it.pos, [it.yaw, 0.0, 0.0]);
                match self.item_model(&it.model) {
                    Some(lm) => {
                        let s = &lm.scene;
                        out.append(s, &xf);
                        self.note(&it.model, true);
                    }
                    None => self.note(&it.model, false),
                }
            }
        }
        out
    }

    fn note(&mut self, name: &str, ok: bool) {
        let e = self.used.entry(name.to_string()).or_insert((0, ok));
        e.0 += 1;
        e.1 |= ok;
    }

    /// The stadium the map sits inside.
    ///
    /// A map's blocks are only half of what a car can touch: the decoration —
    /// the stands, the canopy, the outer walls — is a whole second `.Map.Gbx`,
    /// stored inside the pack and named by the map's decoration id. It matters
    /// for real questions: 173691's car comes to rest on a canopy deck, and a
    /// model without the decoration has nothing there at all.
    ///
    /// The decoration's own map is ENCRYPTED in the pack (unlike almost every
    /// other asset, which carries ForceNoCrypt), and it decrypts with the same
    /// pack key.
    pub fn decoration(&mut self, m: &MapFile, yoff: f32) -> Option<(String, Scene)> {
        let name = decoration_id(m)?;
        let deco = ["Base", ""]
            .iter()
            .map(|p| format!("Stadium\\GameCtnDecoration\\{}{}.Decoration.Gbx", p, name))
            .find(|p| self.store.resolve(p).is_some())?;
        let model = self.store.load_model(&deco).ok()?;
        let deco_map = model.refs_ending(".Map.Gbx").into_iter().next()?;
        let bytes = self.store.read(&deco_map).ok()?;
        let inner = MapFile::from_gbx(tmmaps::gbx::Gbx::parse(&bytes));
        Some((deco_map, self.map(&inner, yoff, true)))
    }
}

/// The decoration a map names, e.g. `48x48Day` or `NoStadium48x48Night`.
///
/// Read out of the header's own strings rather than by transcribing
/// `CGameCtnChallenge`'s header chunks: the id has a shape nothing else in the
/// header shares, and a wrong match fails loudly (no such decoration file)
/// rather than quietly placing a stadium that is not this map's.
fn decoration_id(m: &MapFile) -> Option<String> {
    let head = String::from_utf8_lossy(&m.gbx.user_data);
    let bytes = head.as_bytes();
    let mut best: Option<String> = None;
    for start in 0..bytes.len() {
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'x')
            && end - start < 40
        {
            end += 1;
        }
        if end - start < 6 {
            continue;
        }
        let s = &head[start..end];
        if looks_like_decoration(s) && best.is_none() {
            best = Some(s.to_string());
        }
    }
    best
}

fn looks_like_decoration(s: &str) -> bool {
    let moods = ["Day", "Night", "Sunrise", "Sunset"];
    if !moods.iter().any(|m| s.ends_with(m)) {
        return false;
    }
    // ...NxM... somewhere in the name.
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == 'x' && i > 0 {
            let before = s[..i].chars().rev().take_while(|c| c.is_ascii_digit()).count();
            let after = s[i + 1..].chars().take_while(|c| c.is_ascii_digit()).count();
            if before >= 1 && after >= 1 {
                return true;
            }
        }
    }
    false
}

fn merge_stats(into: &mut Stats, from: &Stats) {
    into.files += from.files;
    into.surfaces += from.surfaces;
    into.triangles += from.triangles;
    into.visual_meshes += from.visual_meshes;
    into.missing.extend(from.missing.iter().cloned());
    for (k, v) in &from.unhandled {
        *into.unhandled.entry(*k).or_insert(0) += v;
    }
}
