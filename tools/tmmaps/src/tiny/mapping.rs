//! The `tmmaps tiny` mapping file (`mapgeom tiny-library --mapping-out`): one
//! row per block placement (`@index`, `b@index` for a generated filler),
//! item placement (`i@index`), prefab tree (`v@ALIAS`), sink (`y@index`) and
//! clearance verdict (`xv@` / `xvb@` / `xvi@` / `xi@`), read into `Mappings`.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug)]
pub struct Mapping {
    pub model: String,
    pub model_scale: f32,
    /// Footprint in cells (x, z) of the block's selected variant. The prefab
    /// geometry is authored from the block's local corner, so a rotated block
    /// must be shifted by its footprint to stay on its own cells.
    pub footprint: Option<(u32, u32)>,
    /// The unit cells of the block's selected variant, in the block's own
    /// frame (offsets from its cell; the mapping row's 6th field). A ground
    /// deck hides the terrain tile under EVERY one of them, not just under
    /// its origin cell (`replaced_cells`).
    pub units: Vec<[i32; 3]>,
    /// The variant's AUTO TERRAIN (the mapping row's 7th field): the terrain
    /// tiles the block brings along, as (offset in the block's frame, zone
    /// block name), plus the variant's place type. `None` when the row has no
    /// 7th field (an older mapping — the name rule of `stands_in_for_tile`
    /// stands in); `Some(empty)` when the variant declares none.
    pub auto_terrain: Option<(Vec<([i32; 3], String)>, i32)>,
    /// The row's 8th field is `S`: a generated SIDE clip (FreeClipSide). The
    /// engine hangs such a piece on its OWNER's face (the unit across the
    /// record's side), which is the record's cell turned half round; with
    /// TINY_SIDECLIP_OWNER=1 `tmmaps tiny` places it there (2026-09-09).
    pub side_clip: bool,
}


#[derive(Default)]
pub struct Mappings {
    /// `b@index`: a BAKED (generated) block -- the FC clip fillers -- that the
    /// tiny build re-emits as an item like an authored block.
    pub baked_by_index: BTreeMap<usize, Mapping>,
    pub by_name: BTreeMap<String, Mapping>,
    pub by_index: BTreeMap<usize, Mapping>,
    /// Original ITEM placements re-pointed at an embedded copy of their own
    /// model (`i@INDEX` rows). Items without a row keep their model.
    pub items_by_index: BTreeMap<usize, Mapping>,
    /// `v@ALIAS` rows: the vegetation a block's prefab carried, as stock
    /// items to place with every placement of that alias — (item, position
    /// in the item's scaled frame, yaw, pitch). A DecoLake shore carries hundreds
    /// of trees the static item cannot bake (VegetTreeModel: no mesh). The
    /// optional 7th field is a PITCH (radians): the hidden stock flag that
    /// drives an embedded tween cloth hangs upside down under it (pi).
    pub veget_by_alias: BTreeMap<String, Vec<(String, [f32; 3], f32, f32)>>,
    /// `y@INDEX` rows: metres an existing item placement is LOWERED by after the
    /// transform — a full-size stock tree standing in for a species the game
    /// cannot scale, sunk so its crown top sits where the original's would.
    pub sink_by_index: BTreeMap<usize, f32>,
    /// `yb@INDEX` rows: metres the item of BAKED record INDEX is LOWERED by
    /// after the transform. `mapgeom coplanar-sinks` (2026-09-09): a free
    /// clip whose top face lies exactly in an authored deck's top face — the
    /// caps of Norway 23's two sideways free pillars under checkpoint 8 —
    /// is drawn UNDER the deck by the game (the deck wins every frame; the cap
    /// shows only with the checkpoint block moved away) while two coplanar
    /// items z-fight, so the clip goes down a centimetre.
    pub sink_baked_by_index: BTreeMap<usize, f32>,
    /// The tree clearance verdicts (mapgeom tree_clear, 2026-09-08: a tree
    /// whose crown meets a driving deck is dropped). `xv@N<TAB>K`: the K-th
    /// `v@` tree of authored block placement N; `xvb@N<TAB>K`: of baked block
    /// N; `xvi@N<TAB>K`: of the vegetation-cluster item N; `xi@N`: the item
    /// placement N itself (a stock tree standing in for the map's own
    /// vegetation item) — parked like a dropped item.
    pub skip_block_trees: BTreeMap<usize, BTreeSet<usize>>,
    pub skip_baked_trees: BTreeMap<usize, BTreeSet<usize>>,
    pub skip_item_trees: BTreeMap<usize, BTreeSet<usize>>,
    pub drop_items: BTreeSet<usize>,
    /// `xf` rows: converted flag placements that get NO hidden stock
    /// driver (nothing below to hide it in — a deck over open air); their
    /// tween cloth stays at frame 0.
    pub no_driver: BTreeSet<usize>,
    /// `iv@INDEX<TAB>V` rows: the variant byte an item placement carries after
    /// its re-point — the byte indexes the SOURCE model's variant list, and a
    /// stock stand-in has its own: `Show` variant 28 (its fogger rig, 60
    /// placements over ten maps) becomes `ShowFogger8M` variant 0 (2026-09-08).
    /// Placements without a row keep their byte (embedded copies are cleared
    /// regardless: built for one variant).
    pub variant_by_index: BTreeMap<usize, u8>,
}

/// `BLOCK<TAB>ITEM[<TAB>MODEL_SCALE]`, or `@INDEX<TAB>...` for an exact block
/// placement, or `i@INDEX<TAB>...` for an existing item placement. Index rows
/// win over block-name rows. MODEL_SCALE is the scale already baked into the
/// item geometry. `v@ALIAS<TAB>ITEM<TAB>X<TAB>Y<TAB>Z<TAB>YAW` adds a stock
/// vegetation item to every placement of ALIAS.
pub fn read_mapping(path: &Path) -> Mappings {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = Mappings::default();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if let Some(index) = fields[0].strip_prefix("yb@") {
            assert!(fields.len() == 2, "{}:{}: expected yb@INDEX<TAB>DY", path.display(), line_no + 1);
            let idx: usize = index.parse().unwrap_or_else(|_| panic!("{}:{}: baked index expected", path.display(), line_no + 1));
            let dy: f32 = fields[1].parse().unwrap_or_else(|_| panic!("{}:{}: number expected", path.display(), line_no + 1));
            out.sink_baked_by_index.insert(idx, dy);
            continue;
        }
        if let Some(index) = fields[0].strip_prefix("y@") {
            assert!(fields.len() == 2, "{}:{}: expected y@INDEX<TAB>DY", path.display(), line_no + 1);
            let idx: usize = index.parse().unwrap_or_else(|_| panic!("{}:{}: item index expected", path.display(), line_no + 1));
            let dy: f32 = fields[1].parse().unwrap_or_else(|_| panic!("{}:{}: number expected", path.display(), line_no + 1));
            out.sink_by_index.insert(idx, dy);
            continue;
        }
        if let Some(rest) = fields[0].strip_prefix("xv@").map(|r| (r, 0)).or_else(|| fields[0].strip_prefix("xvb@").map(|r| (r, 1))).or_else(|| fields[0].strip_prefix("xvi@").map(|r| (r, 2))) {
            assert!(fields.len() == 2, "{}:{}: expected xv@INDEX<TAB>K", path.display(), line_no + 1);
            let idx: usize = rest.0.parse().unwrap_or_else(|_| panic!("{}:{}: placement index expected", path.display(), line_no + 1));
            let k: usize = fields[1].parse().unwrap_or_else(|_| panic!("{}:{}: tree index expected", path.display(), line_no + 1));
            let set = match rest.1 { 0 => &mut out.skip_block_trees, 1 => &mut out.skip_baked_trees, _ => &mut out.skip_item_trees };
            set.entry(idx).or_default().insert(k);
            continue;
        }
        if let Some(index) = fields[0].strip_prefix("iv@") {
            assert!(fields.len() == 2, "{}:{}: expected iv@INDEX<TAB>VARIANT", path.display(), line_no + 1);
            let idx: usize = index.parse().unwrap_or_else(|_| panic!("{}:{}: item index expected", path.display(), line_no + 1));
            let v: u8 = fields[1].parse().unwrap_or_else(|_| panic!("{}:{}: variant byte expected", path.display(), line_no + 1));
            out.variant_by_index.insert(idx, v);
            continue;
        }
        if let Some(index) = fields[0].strip_prefix("xi@") {
            assert!(fields.len() == 1, "{}:{}: expected xi@INDEX", path.display(), line_no + 1);
            let idx: usize = index.parse().unwrap_or_else(|_| panic!("{}:{}: item index expected", path.display(), line_no + 1));
            out.drop_items.insert(idx);
            continue;
        }
        if let Some(index) = fields[0].strip_prefix("xf@") {
            assert!(fields.len() == 1, "{}:{}: expected xf@INDEX", path.display(), line_no + 1);
            let idx: usize = index.parse().unwrap_or_else(|_| panic!("{}:{}: item index expected", path.display(), line_no + 1));
            out.no_driver.insert(idx);
            continue;
        }
        if let Some(alias) = fields[0].strip_prefix("v@") {
            assert!(fields.len() == 6 || fields.len() == 7, "{}:{}: expected v@ALIAS<TAB>ITEM<TAB>X<TAB>Y<TAB>Z<TAB>YAW[<TAB>PITCH]", path.display(), line_no + 1);
            let f = |i: usize| fields[i].parse::<f32>().unwrap_or_else(|_| panic!("{}:{}: number expected", path.display(), line_no + 1));
            let pitch = if fields.len() == 7 { f(6) } else { 0.0 };
            out.veget_by_alias.entry(alias.to_string()).or_default().push((fields[1].to_string(), [f(2), f(3), f(4)], f(5), pitch));
            continue;
        }
        assert!(
            (2..=8).contains(&fields.len()) && fields.len() != 4,
            "{}:{}: expected BLOCK<TAB>ITEM[<TAB>MODEL_SCALE[<TAB>SX<TAB>SZ[<TAB>UNITS[<TAB>AUTO_TERRAIN[<TAB>S]]]]]",
            path.display(),
            line_no + 1
        );
        let model_scale = fields
            .get(2)
            .map_or(1.0, |s| s.parse::<f32>().expect("MODEL_SCALE number"));
        assert!(model_scale.is_finite() && model_scale > 0.0);
        let footprint = if fields.len() >= 5 {
            let sx: u32 = fields[3].parse().expect("SX cells");
            let sz: u32 = fields[4].parse().expect("SZ cells");
            assert!(sx >= 1 && sz >= 1, "footprint must be at least 1x1");
            Some((sx, sz))
        } else {
            None
        };
        // 6th field: the variant's unit cells `x,y,z;x,y,z;…` (empty = the origin cell alone)
        let units: Vec<[i32; 3]> = match fields.get(5) {
            Some(s) if !s.trim().is_empty() => s
                .split(';')
                .map(|c| {
                    let v: Vec<i32> = c.split(',').map(|x| x.trim().parse::<i32>().unwrap_or_else(|_| panic!("{}:{}: unit cell x,y,z expected, got {c:?}", path.display(), line_no + 1))).collect();
                    assert!(v.len() == 3, "{}:{}: unit cell x,y,z expected, got {c:?}", path.display(), line_no + 1);
                    [v[0], v[1], v[2]]
                })
                .collect(),
            _ => Vec::new(),
        };
        // 7th field: `dx,dy,dz=Zone;…|placetype` (empty = the variant declares no auto terrain)
        let auto_terrain: Option<(Vec<([i32; 3], String)>, i32)> = fields.get(6).map(|s| {
            let s = s.trim();
            let (list, place) = s.split_once('|').unwrap_or((s, "0"));
            let entries = list
                .split(';')
                .filter(|e| !e.trim().is_empty())
                .map(|e| {
                    let (off, zone) = e.split_once('=').unwrap_or_else(|| panic!("{}:{}: auto terrain `dx,dy,dz=Zone` expected, got {e:?}", path.display(), line_no + 1));
                    let v: Vec<i32> = off.split(',').map(|x| x.trim().parse::<i32>().unwrap_or_else(|_| panic!("{}:{}: auto terrain offset x,y,z expected, got {off:?}", path.display(), line_no + 1))).collect();
                    assert!(v.len() == 3, "{}:{}: auto terrain offset x,y,z expected, got {off:?}", path.display(), line_no + 1);
                    ([v[0], v[1], v[2]], zone.to_string())
                })
                .collect();
            (entries, place.trim().parse::<i32>().unwrap_or(0))
        });
        let mapping = Mapping {
            model: fields[1].to_string(),
            model_scale,
            footprint,
            units,
            auto_terrain,
            side_clip: fields.get(7).map(|s| *s == "S").unwrap_or(false),
        };
        let prev = if let Some(index) = fields[0].strip_prefix("i@") {
            out.items_by_index
                .insert(index.parse().expect("i@INDEX number"), mapping)
        } else if let Some(index) = fields[0].strip_prefix("b@") {
            out.baked_by_index
                .insert(index.parse().expect("b@INDEX number"), mapping)
        } else if let Some(index) = fields[0].strip_prefix('@') {
            out.by_index
                .insert(index.parse().expect("@INDEX number"), mapping)
        } else {
            out.by_name.insert(fields[0].to_string(), mapping)
        };
        assert!(
            prev.is_none(),
            "{}:{}: duplicate mapping {}",
            path.display(),
            line_no + 1,
            fields[0]
        );
    }
    out
}

