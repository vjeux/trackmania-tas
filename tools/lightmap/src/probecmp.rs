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
}

pub const IMAGE_NAMES: [&str; 4] = ["colour", "occlusion", "pale-colour", "point-lights"];

/// The one-line summary of a cell's probe blob for the corpus column: layout differences, common probes, per image the ALL-split
/// accumulator, and the (ours, editor) scale words.
pub struct Summary {
    pub layout_diffs: usize,
    pub common: usize,
    pub only_ours: usize,
    pub only_theirs: usize,
    pub images: [ImgAcc; 4],
    pub scales: [(Option<f32>, Option<f32>); 4],
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
    let mut common = 0usize;
    let mut only_ours = 0usize;
    for (key, (ra, _, _)) in &a.probes {
        let Some((rb, _, _)) = b.probes.get(key) else { only_ours += 1; continue };
        common += 1;
        for k in 0..4 { images[k].add(ra[k], rb[k]); }
    }
    let scales = [0, 1, 2, 3].map(|k| (va.frame_info.get(k).map(|f| f.0), vb.frame_info.get(k).map(|f| f.0)));
    Ok(Summary { layout_diffs, common, only_ours, only_theirs: b.probes.len().saturating_sub(common), images, scales })
}

/// The corpus column's text for one cell: `layout ✓ · 392 probes · colour 48.5 % id / 83.9 % ±2 / max 7 / 0.999/1.000/0.999 (scale 1.0264 vs 1.0225) · occl 96.4 % · pale 53.3 % · lights 100 %`.
pub fn summary_line(s: &Summary) -> String {
    let c = &s.images[0];
    let r3 = |v: [f64; 3]| -> String { let one = |x: f64| if x.is_finite() { format!("{x:.3}") } else { "—".to_string() }; format!("{}/{}/{}", one(v[0]), one(v[1]), one(v[2])) };
    let sc = |k: usize| -> String { match s.scales[k] { (Some(a), Some(b)) => if a.to_bits() == b.to_bits() { format!("scale {a} =") } else { format!("scale {a} vs {b} ({:+.2} %)", 100.0 * (a as f64 / b.max(1e-12) as f64 - 1.0)) }, _ => "scale —".to_string() } };
    // the value ratio ≈ the byte ratio × the scale ratio (the stored byte is the value over the image's scale word; the encode's curve
    // is the same on both sides, so the product reads the value to first order)
    let val = |k: usize| -> String { match s.scales[k] { (Some(a), Some(b)) if b > 1e-9 && a.to_bits() != b.to_bits() => { let r = s.images[k].ratio(); let f = a as f64 / b as f64; format!(" ≈ value {}", r3([r[0] * f, r[1] * f, r[2] * f])) } _ => String::new() } };
    format!("layout {} · {} probes{} · colour {:.1} % id / {:.1} % ±2 / max {} / bytes {} ({}){} · occlusion {:.1} % id ({}) · pale {:.1} % · lights {:.1} %",
        if s.layout_diffs == 0 { "✓" } else { &"DIFFERS" }, s.common, if s.only_ours + s.only_theirs > 0 { format!(" (+{} only ours, +{} only editor)", s.only_ours, s.only_theirs) } else { String::new() },
        c.pct(c.exact), c.pct(c.within2), c.max_delta, r3(c.ratio()), sc(0), val(0), s.images[1].pct(s.images[1].exact), sc(1), s.images[2].pct(s.images[2].exact), s.images[3].pct(s.images[3].exact))
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
    let mut worst: Vec<(u32, (i32, i32, i32), [u8; 3], [u8; 3], bool)> = Vec::new();
    for (key, (ra, valid_a, level)) in &a.probes {
        let Some((rb, valid_b, _)) = b.probes.get(key) else { only_ours += 1; continue };
        common += 1;
        let both_valid = *valid_a && *valid_b;
        for k in 0..4 {
            acc[k][if both_valid { 0 } else { 1 }].add(ra[k], rb[k]);
            acc[k][2].add(ra[k], rb[k]);
        }
        if o.levels { let e = by_level.entry(*level).or_default(); for k in 0..4 { e[k].add(ra[k], rb[k]); } }
        if o.worst > 0 {
            let md = (0..3).map(|c| (ra[0][c] as i32 - rb[0][c] as i32).unsigned_abs()).max().unwrap_or(0);
            if worst.len() < o.worst || md > worst.last().map(|w| w.0).unwrap_or(0) {
                worst.push((md, *key, ra[0], rb[0], both_valid));
                worst.sort_by(|p, q| q.0.cmp(&p.0));
                worst.truncate(o.worst);
            }
        }
    }
    only_theirs = b.probes.len().saturating_sub(common);
    println!("  probes: {common} common (by world position, {} m cells); {only_ours} only in ours, {only_theirs} only in the editor's", va.cell_size());
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
            let line = format!("level y {:>6} m\t{} probes\tcolour {:.1} % {} bias {}\tocclusion {:.1} % {}\tpale {:.1} %\tlights {:.1} %", y, e[0].n, e[0].pct(e[0].exact), r3(e[0].ratio(), 3), r3(e[0].bias(), 2), e[1].pct(e[1].exact), r3(e[1].ratio(), 3), e[2].pct(e[2].exact), e[3].pct(e[3].exact));
            println!("{line}");
            tsv.push_str(&format!("level\ty={y}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\t{}\t{}\t{}\n", e[0].n, e[0].pct(e[0].exact), e[0].pct(e[0].within1), e[0].pct(e[0].within2), e[0].max_delta, r3(e[0].mean_ours(), 1), r3(e[0].mean_theirs(), 1), r3(e[0].ratio(), 3), r3(e[0].bias(), 2)));
        }
    }
    if o.worst > 0 && !worst.is_empty() {
        println!("\nthe {} probes with the largest colour |Δ| (world x, y, z; ours rgb vs editor rgb; valid = both outside geometry):", worst.len());
        for (md, key, ra, rb, valid) in &worst { println!("  ({:>6}, {:>5}, {:>6})  |Δ| {:>3}  ours {:?} editor {:?}{}", key.0, key.1, key.2, md, ra, rb, if *valid { "" } else { "  (inside geometry on a side)" }); }
    }
    tsv.push_str(&format!("#probes\t{common}\t{only_ours}\t{only_theirs}\t{}\n", if layout.is_empty() { "layout-identical" } else { "layout-differs" }));
    if let Some(p) = &o.tsv { std::fs::write(p, tsv).map_err(|e| format!("{p}: {e}"))?; }
    Ok(())
}
