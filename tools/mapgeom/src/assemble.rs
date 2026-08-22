//! A whole map, assembled: every block and item read by `tmmaps`, placed by
//! `place`, with each model's geometry read from the pack once and instanced.

use crate::geom::{Collector, Stats, Xform, IDENTITY};
use crate::place;
use crate::scene::Scene;
use crate::store::DataStore;
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
    pub stats: Stats,
    /// Model name -> how many placements used it, and whether it had geometry.
    pub used: BTreeMap<String, (usize, bool)>,
}

impl<'a> Assembler<'a> {
    pub fn new(store: &'a mut DataStore) -> Assembler<'a> {
        Assembler { store, cache: HashMap::new(), stats: Stats::default(), used: BTreeMap::new() }
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
