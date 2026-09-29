//! `lmtool probecmp OURS.Map.Gbx --against EDITOR.Map.Gbx [--tsv OUT.tsv] [--levels] [--worst N]` — THE PROBE-BLOB DIFF
//! (verification engineer V4, 2026-09-28): V3's gap. The corpus gate compared the lightmap frames only; the PROBE VOLUME
//! (the trailer after `FACADE01` + frame 0 image 2 = four concatenated WEBPs: 0 probe colour, 1 occlusion (up/down),
//! 2 pale colour, 3 point lights — `volume::Volume`) had no editor-side compare, so a landing that "moves the probes only"
//! (04cb38a69e: E2's Warp terrain lit in the peel — 15 BlueBay cells "bytes differ, every metric equal") was invisible to
//! the matrix. This reads both files' volumes, keys every stored probe by its WORLD position (block pos + cell·(index + ½);
//! the two-cell slot margins duplicate their neighbours and are kept once), and over the common probes reports, per image:
//! probes compared, bytes identical %, within ±1 / ±2, max |Δ|, the mean per channel on both sides and the ratio, split by
//! the trailer's validity mask (a probe INSIDE geometry is dark and never interpolated — compared separately) and, with
//! `--levels`, per height level (the probes read the world peel: a terrain/water/far-layer term has a vertical profile).
//! The lossless part first: the two trailers' layouts (block count, per-block slot/min/max/pos, slot table, scales) and the
//! per-image scale words. `--worst N` lists the N probes with the largest colour |Δ| (world position, level, both sides).
//! `--tsv` writes the per-image rows (+ per-level rows) for the matrix.

use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct ImgAcc {
    pub n: usize,
    pub exact: usize,
    pub within1: usize,
    pub within2: usize,
    pub max_delta: u32,
    pub sum_ours: [f64; 3],
    pub sum_theirs: [f64; 3],
    /// signed byte Δ ours − theirs summed per channel (the bias)
    pub sum_delta: [f64; 3],
}

impl ImgAcc {
    fn add(&mut self, a: [u8; 3], b: [u8; 3]) {
        self.n += 1;
        let mut md = 0u32;
        for c in 0..3 {
            let d = (a[c] as i32 - b[c] as i32).unsigned_abs();
            md = md.max(d);
            self.sum_ours[c] += a[c] as f64;
            self.sum_theirs[c] += b[c] as f64;
            self.sum_delta[c] += a[c] as f64 - b[c] as f64;
        }
        if md == 0 { self.exact += 1; }
        if md <= 1 { self.within1 += 1; }
        if md <= 2 { self.within2 += 1; }
        self.max_delta = self.max_delta.max(md);
    }
    pub fn pct(&self, k: usize) -> f64 { if self.n == 0 { f64::NAN } else { 100.0 * k as f64 / self.n as f64 } }
    pub fn mean_ours(&self) -> [f64; 3] { let n = self.n.max(1) as f64; [self.sum_ours[0] / n, self.sum_ours[1] / n, self.sum_ours[2] / n] }
    pub fn mean_theirs(&self) -> [f64; 3] { let n = self.n.max(1) as f64; [self.sum_theirs[0] / n, self.sum_theirs[1] / n, self.sum_theirs[2] / n] }
    pub fn ratio(&self) -> [f64; 3] { let (a, b) = (self.mean_ours(), self.mean_theirs()); let r = |x: f64, y: f64| if y > 1e-9 && self.n >= 8 { x / y } else { f64::NAN }; [r(a[0], b[0]), r(a[1], b[1]), r(a[2], b[2])] }
    pub fn bias(&self) -> [f64; 3] { let n = self.n.max(1) as f64; [self.sum_delta[0] / n, self.sum_delta[1] / n, self.sum_delta[2] / n] }
}

/// One side's probes: world-keyed (x, y, z in whole metres) → (per-image RGB ×4, valid, level y).
pub struct Side {
    pub volume: crate::volume::Volume,
    pub images: Vec<crate::img::Rgb>,
    pub probes: HashMap<(i32, i32, i32), ([[u8; 3]; 4], bool, i32)>,
    pub stored_levels: usize,
    pub duplicates: usize,
}

pub fn load_side(map: &crate::mapio::MapLightmap, label: &str) -> Result<Side, String> {
    let d = map.chunk.data.as_ref().ok_or(format!("{label}: no lightmap data"))?;
    let volume = crate::volume::Volume::parse(&d.cache.trailer).map_err(|e| format!("{label}: trailer: {e}"))?;
    let blob = d.frames.first().and_then(|f| f.images.get(2)).ok_or(format!("{label}: no frame 0 image 2 (the probe blob)"))?;
    let parts = crate::volume::split_probe_blob(blob, &volume.frame_info);
    if parts.len() < 4 { return Err(format!("{label}: the probe blob splits into {} images, expected 4", parts.len())); }
    let mut images = Vec::new();
    for (k, p) in parts.iter().enumerate().take(4) { images.push(crate::img::decode_webp(p).map_err(|e| format!("{label}: probe image {k}: {e}"))?); }
    let (w, h) = (images[0].w, images[0].h);
    if images.iter().any(|i| i.w != w || i.h != h) { return Err(format!("{label}: the four probe images differ in size: {:?}", images.iter().map(|i| (i.w, i.h)).collect::<Vec<_>>())); }
    let cw = (w + 3) / 4;
    let valid = |x: u32, y: u32| -> bool {
        let cell = volume.cell4.get(((y / 4) * cw + x / 4) as usize).copied().unwrap_or(0xffff);
        cell & (1u16 << ((y % 4) * 4 + x % 4)) != 0
    };
    let c = volume.cell_size();
    let mut probes: HashMap<(i32, i32, i32), ([[u8; 3]; 4], bool, i32)> = HashMap::new();
    let (mut stored_levels, mut duplicates) = (0usize, 0usize);
    for b in &volume.blocks {
        let (tw, th) = (b.max[0] - b.min[0], b.max[2] - b.min[2]);
        for (si, s) in b.slices.iter().enumerate() {
            let Some((tx, ty)) = s else { continue };
            stored_levels += 1;
            let level = b.min[1] + si as u32;
            let wy = b.pos[1] + c * (level as f32 - 0.5);
            for zz in 0..th { for xx in 0..tw {
                let (px, py) = (tx + xx, ty + zz);
                if px >= w || py >= h { continue; }
                let wx = b.pos[0] + c * ((b.min[0] + xx) as f32 + 0.5);
                let wz = b.pos[2] + c * ((b.min[2] + zz) as f32 + 0.5);
                let key = (wx.round() as i32, wy.round() as i32, wz.round() as i32);
                let rgb = [images[0].get(px, py), images[1].get(px, py), images[2].get(px, py), images[3].get(px, py)];
                if probes.contains_key(&key) { duplicates += 1; continue; }
                probes.insert(key, (rgb, valid(px, py), wy.round() as i32));
            } }
        }
    }
    Ok(Side { volume, images, probes, stored_levels, duplicates })
}

pub struct Options {
    pub levels: bool,
    pub worst: usize,
    pub tsv: Option<String>,
    /// `--dump FILE`: every common probe as one row — world x y z, the validity of both sides, the four images' bytes of
    /// both sides (E4's per-probe read of the buried rows).
    pub dump: Option<String>,
    /// `--far-from FILE --radius R` (V5, 2026-09-29): compare only the probes FARTHER than R m (in x, z) from every listed point
    /// (`tmmaps census` of the map's items) — RE 17's open-sea-floor row: probes above the bare floor far from any item read the
    /// floor's emitted colour (their downward hemisphere), so ours/editor there ≈ MDiffuse_ours/MDiffuse_game of the tiles.
    pub far_from: Option<(Vec<(f32, f32)>, f32)>,
    /// `--y-range LO,HI` (V5): compare only the probe rows with LO ≤ world y ≤ HI (the first rows above the floor).
    pub y_range: Option<(i32, i32)>,
    /// `--boxes RECORDS.tsv --box-names A,B,… [--box-above M]` (E6 2026-09-29, E4's hill-probe oracle as a flag): only the probes
    /// inside the XZ × Y box (centre ± half, from the 12-column records table) of a chart whose name contains one of the
    /// substrings; `--box-above M` extends each box upward by M metres (the air just over the surface).
    pub boxes: Option<Vec<([f32; 3], [f32; 3])>>,
}

/// The boxes of `--boxes`: (min, max) per selected record row (the 12-column records table: chart, class, obj, sub, name,
/// quality, centre_y, centre_x, centre_z, half_x, half_y, half_z).
pub fn read_boxes(path: &str, names: &[String], above: f32) -> Result<Vec<([f32; 3], [f32; 3])>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for line in txt.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 12 { continue; }
        if !names.iter().any(|n| c[4].contains(n.as_str())) { continue; }
        let f = |i: usize| c[i].trim().parse::<f32>().unwrap_or(f32::NAN);
        let (cy, cx, cz, hx, hy, hz) = (f(6), f(7), f(8), f(9), f(10), f(11));
        if [cy, cx, cz, hx, hy, hz].iter().any(|v| !v.is_finite()) { continue; }
        out.push(([cx - hx, cy - hy, cz - hz], [cx + hx, cy + hy + above, cz + hz]));
    }
    Ok(out)
}

/// The `--far-from` / `--y-range` / `--boxes` filter of one probe at world (x, y, z).
pub fn probe_selected(key: &(i32, i32, i32), o: &Options) -> bool {
    if let Some((lo, hi)) = o.y_range { if key.1 < lo || key.1 > hi { return false; } }
    if let Some(bx) = &o.boxes {
        let (x, y, z) = (key.0 as f32, key.1 as f32, key.2 as f32);
        if !bx.iter().any(|(lo, hi)| x >= lo[0] && x <= hi[0] && y >= lo[1] && y <= hi[1] && z >= lo[2] && z <= hi[2]) { return false; }
    }
    if let Some((pts, r)) = &o.far_from {
        let (x, z) = (key.0 as f32, key.2 as f32);
        let r2 = r * r;
        if pts.iter().any(|(px, pz)| { let (dx, dz) = (px - x, pz - z); dx * dx + dz * dz <= r2 }) { return false; }
    }
    true
}

pub const IMAGE_NAMES: [&str; 4] = ["colour", "occlusion", "pale-colour", "point-lights"];

/// The one-line summary of a cell's probe blob for the corpus column: layout differences, common probes, per image the ALL-split
/// accumulator, and the (ours, editor) scale words.
pub struct Summary {
    pub layout_diffs: usize,
    pub common: usize,
    pub only_ours: usize,
    pub only_theirs: usize,
    /// per image over ALL common probes
    pub images: [ImgAcc; 4],
    /// per image over the probes BOTH sides mark valid (outside geometry) — the value read
    pub valid: [ImgAcc; 4],
    /// the colour image over the probes EITHER side marks inside geometry (the editor blackens them)
    pub buried: ImgAcc,
    pub scales: [(Option<f32>, Option<f32>); 4],
}

/// The P-frame state of a cell (the coordinator's matrix frame "P", 2026-09-28): the same vocabulary as the LM cells.
/// P-CLOSED (texel) = the layout identical, every scale word within 3 %, the colour VALUE (bytes × scale, valid probes) within 3 %,
/// the lamp images present when the editor's are, AND the colour identity ≥ 90 % of the probe CEILING (the editor against itself:
/// pwc-day editor vs repeat 62.5 % colour-identical, stpad Night editor vs nocache 72.2 % — `probecmp` on the ceiling cells);
/// P-CLOSED (class) = the same without the identity; P-RESIDUE = otherwise, the worst term named (a layout difference counts:
/// a probe the game does not store, or a grid that is not the game's, is a lossless-part defect); P-OPEN = no probe volume.
pub fn p_state(s: &Summary, lamps: bool, p_ceiling: f64) -> (String, String) {
    let mut bad: Vec<String> = Vec::new();
    if s.layout_diffs > 0 { bad.push(format!("layout {} diff(s){}", s.layout_diffs, if s.only_ours + s.only_theirs > 0 { format!(" (+{} probes only ours, +{} only editor)", s.only_ours, s.only_theirs) } else { String::new() })); }
    let c = &s.valid[0];
    let scale_ratio = |k: usize| -> Option<f64> { match s.scales[k] { (Some(a), Some(b)) if b > 1e-9 => Some(a as f64 / b as f64), _ => None } };
    for k in 0..4 {
        if k >= 2 && !lamps { continue; }
        if let Some(r) = scale_ratio(k) { if (r - 1.0).abs() > 0.03 { bad.push(format!("{} scale {:+.1} %", IMAGE_NAMES[k], 100.0 * (r - 1.0))); } }
    }
    let f = scale_ratio(0).unwrap_or(1.0);
    let v = c.ratio().map(|x| x * f);
    let dev = v.iter().filter(|x| x.is_finite()).map(|x| (x - 1.0).abs()).fold(0.0, f64::max);
    if c.n >= 8 && dev > 0.03 { bad.push(format!("colour value {:.3}/{:.3}/{:.3}", v[0], v[1], v[2])); }
    if lamps {
        let (mo, mt) = (s.images[3].mean_ours(), s.images[3].mean_theirs());
        if mt.iter().sum::<f64>() > 1.5 && mo.iter().sum::<f64>() < 0.1 * mt.iter().sum::<f64>() { bad.push("lamp images EMPTY".into()); }
    }
    if s.buried.n >= 50 { let r = s.buried.ratio(); if r.iter().any(|x| x.is_finite() && *x > 1.5) { bad.push(format!("{} buried probes lit ({:.1}× the editor's)", s.buried.n, r[0].max(r[1]).max(r[2]))); } }
    let ident_ok = c.n > 0 && c.pct(c.exact) >= 0.9 * p_ceiling;
    let state = if s.common == 0 { "P-OPEN" } else if !bad.is_empty() { "P-RESIDUE" } else if ident_ok { "P-CLOSED (texel)" } else { "P-CLOSED (class)" };
    (state.to_string(), if bad.is_empty() { format!("colour {:.1} % identical vs the probe ceiling {p_ceiling:.0}", c.pct(c.exact)) } else { bad.join("; ") })
}

pub fn summary(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap) -> Result<Summary, String> {
    let a = load_side(ours, "ours")?;
    let b = load_side(theirs, "editor")?;
    let (va, vb) = (&a.volume, &b.volume);
    let mut layout_diffs = 0usize;
    if va.grid != vb.grid { layout_diffs += 1; }
    if va.slot_grid != vb.slot_grid { layout_diffs += 1; }
    if va.slots != vb.slots { layout_diffs += 1; }
    if va.blocks.len() != vb.blocks.len() { layout_diffs += 1; }
    for (x, y) in va.blocks.iter().zip(vb.blocks.iter()) {
        if x.origin != y.origin || x.min != y.min || x.max != y.max { layout_diffs += 1; }
        if x.pos.map(f32::to_bits) != y.pos.map(f32::to_bits) { layout_diffs += 1; }
        if x.slices != y.slices { layout_diffs += 1; }
    }
    if va.cell4 != vb.cell4 { layout_diffs += 1; }
    let mut images: [ImgAcc; 4] = Default::default();
    let mut valid: [ImgAcc; 4] = Default::default();
    let mut buried = ImgAcc::default();
    let mut common = 0usize;
    let mut only_ours = 0usize;
    for (key, (ra, va_ok, _)) in &a.probes {
        let Some((rb, vb_ok, _)) = b.probes.get(key) else { only_ours += 1; continue };
        common += 1;
        for k in 0..4 { images[k].add(ra[k], rb[k]); }
        if *va_ok && *vb_ok { for k in 0..4 { valid[k].add(ra[k], rb[k]); } } else { buried.add(ra[0], rb[0]); }
    }
    let scales = [0, 1, 2, 3].map(|k| (va.frame_info.get(k).map(|f| f.0), vb.frame_info.get(k).map(|f| f.0)));
    Ok(Summary { layout_diffs, common, only_ours, only_theirs: b.probes.len().saturating_sub(common), images, valid, buried, scales })
}

/// The corpus column's text for one cell: `layout ✓ · 392 probes · colour 48.5 % id / 83.9 % ±2 / max 7 / 0.999/1.000/0.999 (scale 1.0264 vs 1.0225) · occl 96.4 % · pale 53.3 % · lights 100 %`.
pub fn summary_line(s: &Summary) -> String {
    // the colour statistics over the probes BOTH sides mark valid; the buried probes (either side inside geometry) reported apart
    let c = &s.valid[0];
    let r3 = |v: [f64; 3]| -> String { let one = |x: f64| if x.is_finite() { format!("{x:.3}") } else { "—".to_string() }; format!("{}/{}/{}", one(v[0]), one(v[1]), one(v[2])) };
    let sc = |k: usize| -> String { match s.scales[k] { (Some(a), Some(b)) => if a.to_bits() == b.to_bits() { format!("scale {a} =") } else { format!("scale {a} vs {b} ({:+.2} %)", 100.0 * (a as f64 / b.max(1e-12) as f64 - 1.0)) }, _ => "scale —".to_string() } };
    // the value ratio ≈ the byte ratio × the scale ratio (the stored byte is the value over the image's scale word; the encode's curve
    // is the same on both sides, so the product reads the value to first order)
    let val = |k: usize| -> String { match s.scales[k] { (Some(a), Some(b)) if b > 1e-9 && a.to_bits() != b.to_bits() => { let r = s.valid[k].ratio(); let f = a as f64 / b as f64; format!(" ≈ value {}", r3([r[0] * f, r[1] * f, r[2] * f])) } _ => String::new() } };
    let buried = if s.buried.n > 0 { let r = s.buried.ratio(); format!(" · {} buried probes (either side inside geometry) bytes {}", s.buried.n, r3(r)) } else { String::new() };
    format!("layout {} · {} probes{} · colour (valid {}) {:.1} % id / {:.1} % ±2 / max {} / bytes {} ({}){} · occlusion {:.1} % id ({}) · pale {:.1} % · lights {:.1} %{}",
        if s.layout_diffs == 0 { "✓" } else { &"DIFFERS" }, s.common, if s.only_ours + s.only_theirs > 0 { format!(" (+{} only ours, +{} only editor)", s.only_ours, s.only_theirs) } else { String::new() },
        c.n, c.pct(c.exact), c.pct(c.within2), c.max_delta, r3(c.ratio()), sc(0), val(0), s.images[1].pct(s.images[1].exact), sc(1), s.images[2].pct(s.images[2].exact), s.images[3].pct(s.images[3].exact), buried)
}

pub fn run(ours: &crate::mapio::MapLightmap, theirs: &crate::mapio::MapLightmap, o: &Options) -> Result<(), String> {
    let a = load_side(ours, "ours")?;
    let b = load_side(theirs, "editor")?;
    let (va, vb) = (&a.volume, &b.volume);
    println!("probe volume: ours {} blocks / {} stored levels / {} probes ({} margin duplicates), atlas {}×{}, cell {} m, slot grid {:?}; editor {} / {} / {} ({}), atlas {}×{}, cell {} m, slot grid {:?}",
        va.blocks.len(), a.stored_levels, a.probes.len(), a.duplicates, a.images[0].w, a.images[0].h, va.cell_size(), va.slot_grid,
        vb.blocks.len(), b.stored_levels, b.probes.len(), b.duplicates, b.images[0].w, b.images[0].h, vb.cell_size(), vb.slot_grid);
    // the lossless part: the layouts and the scale words
    let mut layout: Vec<String> = Vec::new();
    if va.grid != vb.grid { layout.push(format!("atlas grid ours {:?} editor {:?}", va.grid, vb.grid)); }
    if va.slot_grid != vb.slot_grid { layout.push(format!("slot grid ours {:?} editor {:?}", va.slot_grid, vb.slot_grid)); }
    if va.slots != vb.slots { layout.push("slot table differs".into()); }
    if va.blocks.len() != vb.blocks.len() { layout.push(format!("block count ours {} editor {}", va.blocks.len(), vb.blocks.len())); }
    for (i, (x, y)) in va.blocks.iter().zip(vb.blocks.iter()).enumerate() {
        if x.origin != y.origin || x.min != y.min || x.max != y.max { layout.push(format!("block {i}: origin/min/max ours {:?}/{:?}/{:?} editor {:?}/{:?}/{:?}", x.origin, x.min, x.max, y.origin, y.min, y.max)); }
        if x.pos.map(f32::to_bits) != y.pos.map(f32::to_bits) { layout.push(format!("block {i}: pos ours {:?} editor {:?}", x.pos, y.pos)); }
        if x.slices != y.slices { layout.push(format!("block {i}: slice tiles differ ({} vs {} stored)", x.slices.iter().filter(|s| s.is_some()).count(), y.slices.iter().filter(|s| s.is_some()).count())); }
    }
    if va.cell4 != vb.cell4 { let diff = va.cell4.iter().zip(vb.cell4.iter()).filter(|(p, q)| p != q).count(); layout.push(format!("validity mask: {} of {} cells differ (lengths {} / {})", diff, va.cell4.len().max(vb.cell4.len()), va.cell4.len(), vb.cell4.len())); }
    println!("  layout: {}", if layout.is_empty() { "IDENTICAL (blocks, ranges, positions, slice tiles, slot table, validity mask)".to_string() } else { format!("{} difference(s): {}", layout.len(), layout.join("; ")) });
    for k in 0..4 {
        let (sa, sb) = (va.frame_info.get(k).map(|f| f.0), vb.frame_info.get(k).map(|f| f.0));
        println!("  image {k} ({}): scale ours {:?} editor {:?}{}", IMAGE_NAMES[k], sa, sb, if sa.map(f32::to_bits) == sb.map(f32::to_bits) { "" } else { "  ← DIFFERS (the values below compare stored bytes; the scale is part of the value)" });
    }
    // the common probes
    let mut common = 0usize;
    let (mut only_ours, mut only_theirs) = (0usize, 0usize);
    let mut acc: [[ImgAcc; 3]; 4] = Default::default(); // [image][0 both valid, 1 either inside geometry, 2 all]
    let mut by_level: std::collections::BTreeMap<i32, [ImgAcc; 4]> = Default::default();
    // per level: the probes EITHER side marks inside geometry (the buried set) — count and the colour bytes on both sides
    let mut buried_level: std::collections::BTreeMap<i32, ImgAcc> = Default::default();
    // the histogram of the EDITOR's (and our) colour max-channel byte over the buried probes: 0 / 1 / 2 / 3 / 4–15 / 16+ (RE 16: fill-0 vs the (1,1,1) valid-black rule)
    let (mut hist_ed, mut hist_ours) = ([0usize; 6], [0usize; 6]);
    let bin = |v: u8| -> usize { match v { 0 => 0, 1 => 1, 2 => 2, 3 => 3, 4..=15 => 4, _ => 5 } };
    let mut worst: Vec<(u32, (i32, i32, i32), [u8; 3], [u8; 3], bool)> = Vec::new();
    let mut dump = o.dump.as_ref().map(|_| String::from("x\ty\tz\tvalid_ours\tvalid_editor\tours_c0\tours_c1\tours_c2\tours_c3\teditor_c0\teditor_c1\teditor_c2\teditor_c3\n"));
    let mut filtered_out = 0usize;
    for (key, (ra, valid_a, level)) in &a.probes {
        let Some((rb, valid_b, _)) = b.probes.get(key) else { only_ours += 1; continue };
        if !probe_selected(key, o) { filtered_out += 1; continue; }
        common += 1;
        let both_valid = *valid_a && *valid_b;
        if let Some(d) = dump.as_mut() {
            let px = |v: [u8; 3]| format!("{},{},{}", v[0], v[1], v[2]);
            d.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", key.0, key.1, key.2, *valid_a as u8, *valid_b as u8, px(ra[0]), px(ra[1]), px(ra[2]), px(ra[3]), px(rb[0]), px(rb[1]), px(rb[2]), px(rb[3])));
        }
        for k in 0..4 {
            acc[k][if both_valid { 0 } else { 1 }].add(ra[k], rb[k]);
            acc[k][2].add(ra[k], rb[k]);
        }
        if o.levels { let e = by_level.entry(*level).or_default(); for k in 0..4 { e[k].add(ra[k], rb[k]); } if !both_valid { buried_level.entry(*level).or_default().add(ra[0], rb[0]); hist_ed[bin(rb[0][0].max(rb[0][1]).max(rb[0][2]))] += 1; hist_ours[bin(ra[0][0].max(ra[0][1]).max(ra[0][2]))] += 1; } }
        if o.worst > 0 {
            let md = (0..3).map(|c| (ra[0][c] as i32 - rb[0][c] as i32).unsigned_abs()).max().unwrap_or(0);
            if worst.len() < o.worst || md > worst.last().map(|w| w.0).unwrap_or(0) {
                worst.push((md, *key, ra[0], rb[0], both_valid));
                worst.sort_by(|p, q| q.0.cmp(&p.0));
                worst.truncate(o.worst);
            }
        }
    }
    only_theirs = b.probes.len().saturating_sub(common + filtered_out);
    println!("  probes: {common} common (by world position, {} m cells); {only_ours} only in ours, {only_theirs} only in the editor's{}", va.cell_size(), if filtered_out > 0 { format!("; {filtered_out} common probes OUTSIDE the --far-from / --y-range selection (not compared)") } else { String::new() });
    let r3 = |v: [f64; 3], p: usize| -> String { let one = |x: f64| if x.is_finite() { format!("{:.*}", p, x) } else { "—".to_string() }; format!("{} / {} / {}", one(v[0]), one(v[1]), one(v[2])) };
    println!("image\tsplit\tprobes\tidentical %\twithin 1 %\twithin 2 %\tmax|Δ|\tmean bytes ours (r/g/b)\tmean bytes editor (r/g/b)\tratio ours/editor\tbias bytes ours−editor (r/g/b)");
    let mut tsv = String::from("image\tsplit\tprobes\tidentical_pct\twithin1_pct\twithin2_pct\tmax_delta\tmean_ours_rgb\tmean_editor_rgb\tratio_rgb\tbias_rgb\n");
    for k in 0..4 {
        for (si, name) in ["both valid", "inside geometry", "ALL"].iter().enumerate() {
            let c = &acc[k][si];
            if c.n == 0 { continue; }
            let line = format!("{} ({})\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\t{}\t{}\t{}", k, IMAGE_NAMES[k], name, c.n, c.pct(c.exact), c.pct(c.within1), c.pct(c.within2), c.max_delta, r3(c.mean_ours(), 1), r3(c.mean_theirs(), 1), r3(c.ratio(), 3), r3(c.bias(), 2));
            println!("{line}");
            tsv.push_str(&line); tsv.push('\n');
        }
    }
    if o.levels {
        println!("\nper height level (world y of the probe row; all common probes): image 0 colour identical % / ratio / bias, image 1 occlusion identical % / ratio, image 2 pale identical %, image 3 lights identical %");
        for (y, e) in &by_level {
            let bur = buried_level.get(y).map(|b| format!("\tburied {} ({:.0}/{:.0}/{:.0} vs {:.0}/{:.0}/{:.0} bytes)", b.n, b.mean_ours()[0], b.mean_ours()[1], b.mean_ours()[2], b.mean_theirs()[0], b.mean_theirs()[1], b.mean_theirs()[2])).unwrap_or_default();
            let line = format!("level y {:>6} m\t{} probes\tcolour {:.1} % {} bias {}\tocclusion {:.1} % {}\tpale {:.1} %\tlights {:.1} %{bur}", y, e[0].n, e[0].pct(e[0].exact), r3(e[0].ratio(), 3), r3(e[0].bias(), 2), e[1].pct(e[1].exact), r3(e[1].ratio(), 3), e[2].pct(e[2].exact), e[3].pct(e[3].exact));
            println!("{line}");
            tsv.push_str(&format!("level\ty={y}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\t{}\t{}\t{}\n", e[0].n, e[0].pct(e[0].exact), e[0].pct(e[0].within1), e[0].pct(e[0].within2), e[0].max_delta, r3(e[0].mean_ours(), 1), r3(e[0].mean_theirs(), 1), r3(e[0].ratio(), 3), r3(e[0].bias(), 2)));
        }
    }
    if o.levels && hist_ed.iter().sum::<usize>() > 0 {
        println!("\nburied probes (either side inside geometry), colour max-channel byte histogram [0 / 1 / 2 / 3 / 4–15 / 16+]: editor {:?}, ours {:?}", hist_ed, hist_ours);
    }
    if o.worst > 0 && !worst.is_empty() {
        println!("\nthe {} probes with the largest colour |Δ| (world x, y, z; ours rgb vs editor rgb; valid = both outside geometry):", worst.len());
        for (md, key, ra, rb, valid) in &worst { println!("  ({:>6}, {:>5}, {:>6})  |Δ| {:>3}  ours {:?} editor {:?}{}", key.0, key.1, key.2, md, ra, rb, if *valid { "" } else { "  (inside geometry on a side)" }); }
    }
    tsv.push_str(&format!("#probes\t{common}\t{only_ours}\t{only_theirs}\t{}\n", if layout.is_empty() { "layout-identical" } else { "layout-differs" }));
    if let Some(p) = &o.tsv { std::fs::write(p, tsv).map_err(|e| format!("{p}: {e}"))?; }
    if let (Some(p), Some(d)) = (&o.dump, dump) { std::fs::write(p, d).map_err(|e| format!("{p}: {e}"))?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn far_from_and_y_range_select_the_open_floor_probes() {
        let o = Options { levels: false, worst: 0, tsv: None, dump: None, far_from: Some((vec![(100.0, 100.0), (500.0, 40.0)], 100.0)), y_range: Some((-6, 10)), boxes: None };
        assert!(probe_selected(&(300, 2, 300), &o));          // far from both points, in the band
        assert!(!probe_selected(&(150, 2, 100), &o));         // 50 m from the first point
        assert!(!probe_selected(&(300, 34, 300), &o));        // above the band
        assert!(!probe_selected(&(300, -7, 300), &o));        // below the band
        assert!(!probe_selected(&(600, 10, 40), &o));         // exactly 100 m away counts as near (≤ r)
        assert!(probe_selected(&(601, 10, 40), &o));          // 101 m away is far
        let none = Options { levels: false, worst: 0, tsv: None, dump: None, far_from: None, y_range: None, boxes: None };
        assert!(probe_selected(&(0, 0, 0), &none));
    }
}
