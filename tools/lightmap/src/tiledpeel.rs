//! THE PEEL CAMERAS OF A BAKE WITHOUT A CAPTURE: per direction the WORLD peel (the scene box ∪ the probe
//! chunks' box, `lmtiles::world_peel_box`) and the FITTED peels — the tiling rule's n×n cells over the
//! lightmapped items' records (`lmtiles::peel_tiling`, RE 6), each fit by the transcribed light-camera fit
//! (`lightcam::fit_camera`, engineer B) with its own depth range — as the port's `Frustum`s in the order
//! the game issues them (the world peel first, then the tiles), so the peel loop's "later peel wins" rule
//! composes them as the game's LmILightDir_Set draws do.
//!
//! The scene box S here = the items' block records ∪ the zone tiles' box (the seabed quads the port lays
//! at sea level − 3 over the map footprint — the capture's S had the 4096 seabed tiles at y ≈ 4.0 with the
//! items; the tiles are not items of the map, so `lmtiles::scene_box` alone would miss them). The chart
//! allocation's final scale s (layout units per metre) that sizes the extent is the port's own atlas
//! density × 2 until RE 2's allocation rule lands (`--tile-scale S` overrides).

use crate::lightcam::{fit_camera, Aabb, FitRules};
use crate::lmtiles::{peel_tiling, world_peel_box, BlockRecord, CBox, TileParams, Tiling};
use crate::passdump::Frustum;

/// The boxes of a bake's peels: the world peel's, and the fitted tiles' (empty = the world pass only).
#[derive(Clone, Debug)]
pub struct PeelPlan {
    pub scene: CBox,
    pub world: Aabb,
    pub tiles: Vec<Aabb>,
    /// The tiling rule's target size (4096 or 2048) and grid.
    pub size: u32,
    pub n: u32,
    pub ext: f32,
    pub rules: FitRules,
    /// The fitted tiles' frame size — `size` (the game's) unless the non-exact `--tile-res` lowers it (the
    /// tiles exist for the items' fine shadows; a 2048² tile has 4× fewer pixels per direction).
    pub tile_size: u32,
}

fn cbox_to_aabb(b: &CBox) -> Aabb {
    Aabb { min: b.min(), max: b.max() }
}

/// Plan the peels: `records` = the items' block records (`lmtiles::item_records(..).record`), `tiles_box` =
/// the zone tiles' box (None when the map has none), `chunks_aabb` = the probe chunks' box
/// (`probechunk::for_records`), `alloc_scale` = the chart allocation's scale in layout units per metre.
pub fn plan(records: &[BlockRecord], tiles_box: Option<CBox>, chunks_aabb: Option<&CBox>, alloc_scale: f32, quality: u32, vram_bytes: i64, max_tiles: u32) -> PeelPlan {
    let mut scene = crate::lmtiles::scene_box(records);
    if let Some(t) = &tiles_box {
        if scene.is_valid() { scene.union_into(t); } else { scene = *t; }
    }
    let world = world_peel_box(&scene, chunks_aabb);
    let p = TileParams { alloc_scale, quality, half_at_low_quality: true, size_override: 0, vram_bytes, max_tiles };
    let t: Tiling = peel_tiling(&scene, records, &p);
    PeelPlan { scene, world: cbox_to_aabb(&world), tiles: t.tiles.iter().map(cbox_to_aabb).collect(), size: t.size, n: t.n, ext: t.ext, rules: FitRules::default(), tile_size: t.size }
}

impl PeelPlan {
    /// THE NON-EXACT `--tile-res N` (perf engineer 7): the fitted tiles rendered at N² instead of the rule's
    /// size — the same boxes and cameras, a coarser pixel; the world peel keeps its size.
    pub fn with_tile_size(mut self, n: u32) -> PeelPlan {
        if n >= 64 { self.tile_size = n; }
        self
    }
    /// THE NON-EXACT `--tiles-from-world`: no fitted tiles at all — every texel reads the world peel alone
    /// (the composition rule "a later peel wins where it has a layer" then has one peel to compose).
    pub fn world_only(mut self) -> PeelPlan {
        self.tiles.clear();
        self
    }
    /// The frame size of peel `pi` (0 = the world peel, then the tiles).
    pub fn frame_size(&self, pi: usize) -> u32 {
        if pi == 0 { self.size } else { self.tile_size }
    }
    /// The ordered frusta of one direction: the world peel, then every tile (each fit on its own box, its own
    /// depth range), read back through `Frustum::from_pw01` as the captured frusta are. (The shadow matrix's
    /// (w − 2)/w inset scale is the frame's own: a tile at `tile_size` is fit for that size.)
    pub fn frusta(&self, d: [f32; 3]) -> Vec<Frustum> {
        let mut out = Vec::with_capacity(1 + self.tiles.len());
        for (pi, b) in std::iter::once(&self.world).chain(self.tiles.iter()).enumerate() {
            let cam = fit_camera(b, d, &self.rules);
            let s = self.frame_size(pi);
            if let Some(fr) = Frustum::from_pw01(&cam.world_pw01_shadow(s, s)) {
                out.push(fr);
            }
        }
        out
    }
    /// The per-direction table for a sweep's direction list.
    pub fn table(&self, dirs: &[[f32; 3]]) -> Vec<Vec<Frustum>> {
        dirs.iter().map(|d| self.frusta(*d)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_scene_gets_the_world_peel_and_one_fitted_tile_and_a_wide_one_tiles() {
        // three records on a 2048 m footprint at a scale that keeps ext ≤ 4096: no tiling → world + 1 fitted
        let rec = |c: [f32; 3], h: [f32; 3]| BlockRecord { world: CBox::new(c, h), quality: 1.0 };
        let recs = vec![rec([870.0, 50.0, 353.0], [10.0, 45.0, 16.0])];
        let tiles = Some(CBox::from_min_max([0.0, 4.0, 0.0], [2048.0, 4.0, 2048.0]));
        let p = plan(&recs, tiles, None, 1.0, 3, 8 << 30, 4);
        assert_eq!(p.n, 1, "{p:?}");
        // ext = 2048·1.0 = 2048 ≤ 3072 → size 2048; the fitted peel exists (one cell, n < 2 with records → none!)
        // (the rule: count ≠ 0 && n < 2 → the world pass only)
        assert!(p.tiles.is_empty(), "{:?}", p.tiles);
        let f = p.frusta([0.3, 0.2, 0.93]);
        assert_eq!(f.len(), 1);
        // a wide footprint at a big scale: n = ceil(8128·1.25 / 4096) = 3 → up to 9 tiles
        let recs2: Vec<BlockRecord> = (0..9).map(|i| rec([300.0 + 2700.0 * (i % 3) as f32, 50.0, 300.0 + 2700.0 * (i / 3) as f32], [100.0, 40.0, 100.0])).collect();
        let tiles2 = Some(CBox::from_min_max([0.0, 4.0, 0.0], [8128.0, 4.0, 8128.0]));
        let p2 = plan(&recs2, tiles2, None, 1.25, 3, 8 << 30, 4);
        assert_eq!(p2.n, 3, "{p2:?}");
        assert!(!p2.tiles.is_empty());
        let f2 = p2.frusta([0.3, 0.2, 0.93]);
        assert_eq!(f2.len(), 1 + p2.tiles.len());
        // every tile frustum has its own depth range
        assert!(f2.iter().skip(1).all(|fr| fr.half[2] > 0.0));
    }
}
