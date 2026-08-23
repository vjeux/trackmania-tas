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
    /// Which point of `scene` a placement position names. Zero unless the
    /// model carries a `CGameItemPlacementParam`.
    pub pivot: [f32; 3],
    pub scene: Scene,
    pub size: (f32, f32),
}

/// Where a block model lives in the pack.
///
/// A block name resolves through **five** block-info families, not one, and
/// the file extension changes with the family:
///
/// | family | extension | what it holds |
/// |---|---|---|
/// | `GameCtnBlockInfoClassic` | `.EDClassic.Gbx` | roads, walls, platforms |
/// | `GameCtnBlockInfoPillar` | `.EDClassic.Gbx` | the supports under everything |
/// | `GameCtnBlockInfoFlat` | `.EDFlat.Gbx` | the terrain sheet — `Grass` |
/// | `GameCtnBlockInfoFrontier` | `.EDFrontier.Gbx` | cliffs and hills |
/// | `GameCtnBlockInfoTransition` | `.EDTransition.Gbx` | the joins between them |
///
/// Classic and Pillar each additionally carry a `Theme\` subfolder, which is
/// where the seasonal sets live (`SnowRoadStraight`, `RallyCastleRoadStraight`)
/// — 122 more block models that are otherwise invisible.
///
/// Looking in Classic alone left 125 537 placements across the corpus with no
/// geometry, 92 619 of them `DecoWallBasePillar`.
///
/// The environments beyond Stadium carry terrain-adapted copies of the Stadium
/// set; where a model exists in both the geometry is identical.
fn block_candidates(name: &str) -> Vec<String> {
    let mut v = Vec::new();
    for env in ["Stadium", "Stadium256"] {
        // `Stadium256` carries the stadium shell itself (`Stade4096`,
        // `Stade1536`), which is what a DECORATION map is made of.
        for (family, ext) in [
            ("GameCtnBlockInfoClassic", "EDClassic"),
            ("GameCtnBlockInfoPillar", "EDClassic"),
            ("GameCtnBlockInfoFlat", "EDFlat"),
            ("GameCtnBlockInfoFrontier", "EDFrontier"),
            ("GameCtnBlockInfoTransition", "EDTransition"),
        ] {
            v.push(format!("{}\\GameCtnBlockInfo\\{}\\{}.{}.Gbx", env, family, name, ext));
            v.push(format!("{}\\GameCtnBlockInfo\\{}\\Theme\\{}.{}.Gbx", env, family, name, ext));
        }
    }
    for env in ["BlueBay", "GreenCoast", "RedIsland", "WhiteShore"] {
        for (family, ext) in [
            ("GameCtnBlockInfoClassic", "EDClassic"),
            ("GameCtnBlockInfoPillar", "EDClassic"),
            ("GameCtnBlockInfoFlat", "EDFlat"),
            ("GameCtnBlockInfoFrontier", "EDFrontier"),
            ("GameCtnBlockInfoTransition", "EDTransition"),
        ] {
            v.push(format!("{}\\GameCtnBlockInfo\\{}\\Stadium\\{}.{}.Gbx", env, family, name, ext));
            v.push(format!("{}\\GameCtnBlockInfo\\{}\\{}.{}.Gbx", env, family, name, ext));
        }
    }
    v
}

fn item_candidates(name: &str) -> Vec<String> {
    // A map spells an item either bare or with its extension already on, and
    // the seasonal items live one folder deeper.
    let base = name.trim_end_matches(".Item.Gbx");
    let mut v = vec![
        format!("Stadium\\Items\\{}.Item.Gbx", base),
        format!("Stadium\\Items\\Theme\\{}.Item.Gbx", base),
        format!("{}.Item.Gbx", base),
        base.to_string(),
    ];
    for env in ["GreenCoast", "RedIsland", "BlueBay", "WhiteShore"] {
        v.push(format!("{}\\Items\\Stadium\\{}.Item.Gbx", env, base));
    }
    v
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
/// and the zip holds it under a container folder — `Items/…` on 134672 and
/// **`Blocks/…`** on 197047, whose whole run is on 75 placements of one
/// embedded platform. Keying on `Items/` alone left that map at 2.6 % of its
/// samples over a surface with the model reading `76 placements of 3 models
/// had no geometry`. So the name is matched as a SUFFIX of the zip path
/// rather than under an assumed folder.
fn embedded_key(name: &str) -> String {
    name.trim_end_matches("_CustomBlock").replace('\\', "/").to_lowercase()
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
    ///
    /// The map's spelling is matched as a suffix of the zip path, longest
    /// suffix first: 197047 places the same platform under two names,
    /// `StupsKiesel\MiniPlatform\…` and `StupsKiesel\StupsKiesel\MiniPlatform\…`,
    /// and carries two files that differ only by that folder.
    fn embedded_model(&mut self, name: &str) -> Option<LocalModel> {
        let key = embedded_key(name);
        let bytes = self
            .embedded
            .get(&key)
            .or_else(|| {
                // Shortest match: 197047 carries the same platform twice, at
                // `…/StupsKiesel/MiniPlatform/…` and
                // `…/StupsKiesel/StupsKiesel/MiniPlatform/…`, and the shorter
                // name is a suffix of BOTH paths. The longer file is the other
                // block's.
                self.embedded
                    .iter()
                    .filter(|(k, _)| k.ends_with(&format!("/{}", key)))
                    .min_by_key(|(k, _)| k.len())
                    .map(|(_, v)| v)
            })?
            .clone();
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
        Some(LocalModel { pivot: c.pivot, scene, size })
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
        let Some(path) = block_candidates(name).into_iter().find(|p| self.store.resolve(p).is_some())
        else {
            // A map's block list is not only blocks: the gates are ITEMS
            // placed on the grid (`GateCheckpointRight32m`, `GateSpecial32m*`),
            // and so are the rotors and the seasonal props. A name that is not
            // a block model is worth one more lookup before it becomes a hole.
            return self.build_item(name);
        };
        let model = self.store.load_model(&path).ok()?;
        let mut prefabs = model.refs_ending(".Prefab.Gbx");
        if prefabs.is_empty() {
            // Some block families name no prefab of their own and carry their
            // whole shape in their CLIPS — the pieces that fill the seam
            // between neighbouring blocks. `DecoWall*` and `Platform*` are
            // like this, and on 146612 that is 458 placements including the
            // loop wall the run is driven on, which is why that map's car read
            // as 2.048 m above the model until this was followed.
            //
            // Only when there are none: a road block's clips are extra
            // geometry at the same height, and drawing them everywhere would
            // be more than any single placement shows, for no gain.
            for kind in
                [".EDClip.Gbx", ".EDVerticalClip.Gbx", ".EDHorizontalClip.Gbx", ".EDClassic.Gbx"]
            {
                for clip in model.refs_ending(kind) {
                    if let Ok(m) = self.store.load_model(&clip) {
                        prefabs.extend(m.refs_ending(".Prefab.Gbx"));
                    }
                }
            }
            prefabs.sort();
            prefabs.dedup();
        }
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
        Some(LocalModel { pivot: c.pivot, scene, size })
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
        Some(LocalModel { pivot: c.pivot, scene, size })
    }

    /// Assemble a map into one scene.
    ///
    /// A FREE placement — an item, or a block the author dragged off the grid
    /// — names the model's PIVOT, so the mesh is shifted by minus the pivot
    /// before it is turned. A GRID placement names a cell and is not shifted:
    /// the cell IS the anchor.
    pub fn map(&mut self, m: &MapFile, yoff: f32, with_items: bool) -> Scene {
        let mut out = Scene::default();
        for b in &m.blocks {
            let free = b.flags & FREE_BLOCK_FLAG != 0;
            let xf: Xform = {
                let (size, pivot) = match self.block_model(&b.name) {
                    Some(lm) => (lm.size, lm.pivot),
                    None => {
                        self.note(&b.name, false);
                        continue;
                    }
                };
                if free {
                    match (b.free_pos, b.free_rot) {
                        (Some(p), Some(r)) => place::free(p, r, pivot),
                        (Some(p), None) => place::free(p, [0.0; 3], pivot),
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
                let pivot = match self.item_model(&it.model) {
                    Some(lm) => lm.pivot,
                    None => {
                        self.note(&it.model, false);
                        continue;
                    }
                };
                let xf = place::free(it.pos, [it.yaw, 0.0, 0.0], pivot);
                if let Some(lm) = self.item_model(&it.model) {
                    let s = &lm.scene;
                    out.append(s, &xf);
                    self.note(&it.model, true);
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
    into.recovered += from.recovered;
    into.missing.extend(from.missing.iter().cloned());
    for (k, v) in &from.unhandled {
        *into.unhandled.entry(*k).or_insert(0) += v;
    }
}
