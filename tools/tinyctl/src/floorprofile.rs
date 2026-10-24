//! `tinyctl floorfinish-profile --pak F:KEY.. --out F.png` — Hugo's slope diagram (2026-10-24 02:05): for every
//! vista collection a side section through the shore tile's bank (left = land, right = open water), drawn from the
//! REAL pak geometry: the bed profile sampled every metre along the bank's centreline (the straight dead-end
//! variant, in the block frame, then the open-water tile's flat floor), the water surface, the ≥ 2 m depth line,
//! the seabed + 0.75 m band, and the finish slabs exactly as the generator places them (`deep_squares_near`); a
//! fifth row is the BlueBay canal — two Beach dead-ends back to back (Fall 14's channel). True scale, 1 m = 24 px
//! both ways (no vertical exaggeration; the picture says so).
use crate::flagdraw::{Canvas, INK};

const WATER: [u8; 3] = [70, 130, 220];
const DEPTH2: [u8; 3] = [30, 80, 170];
const BED: [u8; 3] = [120, 90, 50];
const LAND: [u8; 3] = [222, 206, 160];
const BAND: [u8; 3] = [240, 180, 60];
const FINISH: [u8; 3] = [220, 40, 40];
const SLOPE: [u8; 3] = [150, 150, 150];
const GRID: [u8; 3] = [215, 215, 215];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

/// A shore kind's section: the bed height every metre along the bank's water direction at the bank's centre
/// line, and the finish squares (their 4 m index along that direction + min bed) — all in the block frame, with
/// the water direction turned so that it runs +s (land at s = 0, water at s = 32).
struct Section {
    /// bed height at s = 0.5, 1.5, … 31.5 (None where the land is above the water quad)
    bed: Vec<Option<f32>>,
    /// land / sand height every metre (the collision top, water quad excluded) — the dry profile too
    top: Vec<Option<f32>>,
    /// finish squares along s: (s0, s1, min bed)
    slabs: Vec<(f32, f32, f32)>,
    surface: f32,
    /// the water quad's extent along s
    water_from: f32,
    label: String,
}

fn section(store: &mut mapgeom::store::DataStore, collection: u32, tile: &str, flags: u32, open_bed: f32) -> Result<Section, String> {
    let w = mapgeom::floor_finish::water_rect(store, collection, tile, flags)?.ok_or_else(|| format!("{tile}: no water quad"))?;
    // the water lies along +z (z0 .. 32) or −z (0 .. z1) or along x: pick the axis and sense so that s runs land → water
    let (axis_z, flip) = if (w.z1 - w.z0) < (w.x1 - w.x0) - 0.5 || ((w.z0 > 1.0) != (w.z1 < 31.0)) {
        // the water strip is a band across z → the bank runs along z
        (true, w.z0 <= 1.0 && w.z1 < 31.0)
    } else {
        (false, w.x0 <= 1.0 && w.x1 < 31.0)
    };
    // sampling line: the centre of the OTHER axis
    let mid = 16.0f32;
    let to_local = |s: f32| -> [f32; 2] {
        let t = if flip { 32.0 - s } else { s };
        if axis_z { [mid, t] } else { [t, mid] }
    };
    let mut bed = Vec::new();
    let mut top = Vec::new();
    for i in 0..32 {
        let p = to_local(i as f32 + 0.5);
        bed.push(mapgeom::floor_finish::bed_height_at_pub(&w, p));
        top.push(mapgeom::floor_finish::top_height_at(&w, p));
    }
    let deep = mapgeom::floor_finish::deep_squares_near(&w, open_bed);
    let mut slabs = Vec::new();
    for zi in 0..8 {
        for xi in 0..8 {
            let Some(y) = deep[zi * 8 + xi] else { continue };
            // the square's extent along s
            let (a0, a1) = if axis_z { (4.0 * zi as f32, 4.0 * zi as f32 + 4.0) } else { (4.0 * xi as f32, 4.0 * xi as f32 + 4.0) };
            let other_mid = if axis_z { 4.0 * xi as f32 + 2.0 } else { 4.0 * zi as f32 + 2.0 };
            if (other_mid - mid).abs() > 2.1 {
                continue; // only the squares on the centre line
            }
            let (s0, s1) = if flip { (32.0 - a1, 32.0 - a0) } else { (a0, a1) };
            slabs.push((s0, s1, (y * 4.0).floor() / 4.0));
        }
    }
    let water_from = if axis_z { if flip { 32.0 - w.z1 } else { w.z0 } } else if flip { 32.0 - w.x1 } else { w.x0 };
    Ok(Section { bed, top, slabs, surface: w.surface, water_from, label: format!("{tile} ({})", w.prefab.trim_end_matches(".Prefab.Gbx")) })
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let out = f(rest, "--out").unwrap_or_else(|| "floorfinish-profiles.png".into());
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--pak" {
            if let Some((p, k)) = rest.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                store.add_pak(p, k)?;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    // (collection id, name, shore tile, flags of the straight dead-end placement, open tile)
    let rows: [(u32, &str, &str, u32, &str); 4] = [
        (0x1c, "BlueBay", "Beach", 0x1005, "Sea"),
        (0x1d, "WhiteShore", "WaterShore1", 0x1005, "Water"),
        (0x10, "RedIsland", "WaterHill", 0x1005, "Water"),
        (0xf, "GreenCoast", "LakeShore", 0x1005, "Lake"),
    ];
    let (w, h) = (1800usize, 2000usize);
    let mut cv = Canvas::new(w, h);
    let px_m = 24.0f32; // 24 px per metre, both axes (64 m -> 1536 px; 11 m -> 264 px)
    let x0 = 140.0f32;
    let top_margin = 125.0f32;
    let row_h = 365.0f32;
    let sec_h = 11.0 * px_m;
    cv.text(30.0, 16.0, "Floor Finish v4.1 - where the sea floor is finishable: side sections, left = land, right = open water", 2, INK);
    cv.text(30.0, 42.0, "bed profile from the pak collision along the bank centre line, 1 m samples. TRUE SCALE, 1 m = 24 px both ways (no vertical exaggeration)", 1, [90, 90, 90]);
    // legend, two rows of three
    let items: [(&str, [u8; 3], u8); 6] = [
        ("water surface", WATER, 1),
        ("at least 2 m of water (surface - 2), dashed", DEPTH2, 2),
        ("seabed .. seabed + 0.75 m (near-floor band)", BAND, 0),
        ("finish slab (the 4 m squares that pass, bed -0.1 .. +0.4)", FINISH, 0),
        ("wet slope under the water, no finish", SLOPE, 0),
        ("land / sand", LAND, 0),
    ];
    for (k, (label, col, kind)) in items.iter().enumerate() {
        let (lx, ly) = (30.0 + 560.0 * (k % 3) as f32, 66.0 + 20.0 * (k / 3) as f32);
        match kind {
            1 => cv.line(lx, ly + 5.0, lx + 30.0, ly + 5.0, *col, 2.0, 1.0),
            2 => cv.dashed(lx, ly + 5.0, lx + 30.0, ly + 5.0, *col, 2.0, 4.0),
            _ => cv.poly(&[(lx, ly), (lx + 30.0, ly), (lx + 30.0, ly + 10.0), (lx, ly + 10.0)], *col, if *col == BAND { 0.7 } else { 1.0 }),
        }
        cv.text(lx + 38.0, ly - 1.0, label, 1, INK);
    }
    // a dimension: a line with end ticks and a left-aligned label under it
    let dim = |cv: &mut Canvas, xa: f32, xb: f32, y: f32, label: &str| {
        let (a, b) = (xa.min(xb), xa.max(xb));
        cv.line(a, y, b, y, [0, 120, 60], 1.5, 1.0);
        cv.line(a, y - 6.0, a, y + 6.0, [0, 120, 60], 1.5, 1.0);
        cv.line(b, y - 6.0, b, y + 6.0, [0, 120, 60], 1.5, 1.0);
        cv.text(a, y + 9.0, label, 1, [0, 120, 60]);
    };
    let grid = |cv: &mut Canvas, base_y: f32| {
        for m in 0..=11 {
            let y = base_y - m as f32 * px_m;
            cv.line(x0, y, x0 + 64.0 * px_m, y, GRID, 1.0, 0.7);
            cv.text(x0 - 40.0, y - 5.0, &format!("{m:>2} m"), 1, [120, 120, 120]);
        }
        for s in (0..=64).step_by(4) {
            cv.line(x0 + s as f32 * px_m, base_y, x0 + s as f32 * px_m, base_y - sec_h, GRID, 1.0, 0.7);
        }
        for s in (0..=64).step_by(8) {
            cv.text(x0 + s as f32 * px_m - 6.0, base_y + 4.0, &format!("{s}"), 1, [120, 120, 120]);
        }
    };
    let mut report = Vec::new();
    for (r, (coll, name, tile, flags, open_tile)) in rows.iter().enumerate() {
        let (_, open_bed) = mapgeom::floor_finish::water_tiles(*coll);
        let surface = mapgeom::floor_finish::water_surface(*coll);
        let sec = section(&mut store, *coll, tile, *flags, open_bed)?;
        let sec_top = top_margin + row_h * r as f32 + 30.0;
        let base_y = sec_top + sec_h; // screen y of local 0
        let sy = |y: f32| base_y - y * px_m;
        let sx = |s: f32| x0 + s * px_m;
        grid(&mut cv, base_y);
        cv.text(x0, sec_top - 26.0, &format!("{name}  -  shore tile {} to the open {open_tile} tile. Sea floor at local {open_bed:.1}, surface {surface:.2}: {:.1} m deep", sec.label, surface - open_bed), 2, INK);
        // the dry top (land / sand) then the wet bank under the water quad
        let mut prev: Option<(f32, f32)> = None;
        for (i, t) in sec.top.iter().enumerate() {
            let s = i as f32 + 0.5;
            if let Some(y) = *t {
                let p = (sx(s), sy(y));
                if let Some(q) = prev {
                    cv.line(q.0, q.1, p.0, p.1, BED, 2.0, 1.0);
                    cv.poly(&[(q.0, q.1), (p.0, p.1), (p.0, sy(0.0)), (q.0, sy(0.0))], if s < sec.water_from { LAND } else { SLOPE }, 0.45);
                }
                prev = Some(p);
            }
        }
        prev = None;
        for (i, b) in sec.bed.iter().enumerate() {
            let s = i as f32 + 0.5;
            if let Some(y) = *b {
                let p = (sx(s), sy(y));
                if let Some(q) = prev {
                    cv.line(q.0, q.1, p.0, p.1, BED, 2.0, 1.0);
                    cv.poly(&[(q.0, q.1), (p.0, p.1), (p.0, sy(0.0)), (q.0, sy(0.0))], SLOPE, 0.45);
                }
                prev = Some(p);
            }
        }
        // the open-water floor 32..64
        cv.poly(&[(sx(32.0), sy(open_bed)), (sx(64.0), sy(open_bed)), (sx(64.0), sy(0.0)), (sx(32.0), sy(0.0))], SLOPE, 0.45);
        cv.line(sx(32.0), sy(open_bed), sx(64.0), sy(open_bed), BED, 2.0, 1.0);
        // water: fill, surface, the 2 m line, the near-floor band
        cv.poly(&[(sx(sec.water_from), sy(surface)), (sx(64.0), sy(surface)), (sx(64.0), sy(open_bed)), (sx(sec.water_from), sy(open_bed))], WATER, 0.18);
        cv.line(sx(sec.water_from), sy(surface), sx(64.0), sy(surface), WATER, 2.0, 1.0);
        cv.dashed(sx(sec.water_from), sy(surface - 2.0), sx(64.0), sy(surface - 2.0), DEPTH2, 2.0, 5.0);
        cv.poly(&[(sx(sec.water_from), sy(open_bed + 0.75)), (sx(64.0), sy(open_bed + 0.75)), (sx(64.0), sy(open_bed)), (sx(sec.water_from), sy(open_bed))], BAND, 0.5);
        // the finish slabs: the shore squares that pass, then the open tile
        let mut first_finish: Option<f32> = None;
        for (s0, s1, ybed) in &sec.slabs {
            cv.poly(&[(sx(*s0), sy(ybed + 0.4)), (sx(*s1), sy(ybed + 0.4)), (sx(*s1), sy(ybed - 0.1)), (sx(*s0), sy(ybed - 0.1))], FINISH, 0.9);
            first_finish = Some(first_finish.map_or(*s0, |v: f32| v.min(*s0)));
        }
        cv.poly(&[(sx(32.0), sy(open_bed + 0.4)), (sx(64.0), sy(open_bed + 0.4)), (sx(64.0), sy(open_bed - 0.1)), (sx(32.0), sy(open_bed - 0.1))], FINISH, 0.9);
        // marks
        cv.line(sx(sec.water_from), sy(surface) - 26.0, sx(sec.water_from), sy(surface), INK, 1.0, 1.0);
        cv.text(sx(sec.water_from) - 28.0, sy(surface) - 38.0, "waterline", 1, INK);
        cv.line(sx(32.0), sy(0.0), sx(32.0), sy(11.0), [90, 90, 90], 1.0, 1.0);
        cv.text(sx(32.0) + 4.0, sy(11.0) + 3.0, &format!("| open {open_tile} tile, flat floor at {open_bed:.1}"), 1, [90, 90, 90]);
        cv.text(sx(0.0) + 4.0, sy(11.0) + 3.0, &format!("{tile} tile, 32 m"), 1, [90, 90, 90]);
        let ff = first_finish.unwrap_or(32.0);
        dim(&mut cv, sx(sec.water_from), sx(ff), base_y + 22.0, &format!("{:.1} m from the waterline to the first finish square (bed {:.2} there)", ff - sec.water_from, sec.slabs.iter().map(|s| s.2).fold(f32::INFINITY, f32::min).min(open_bed + 0.75)));
        cv.text(sx(46.0), sy(surface) - 13.0, &format!("{:.1} m deep", surface - open_bed), 1, DEPTH2);
        report.push(format!("{name}: {} — waterline at s {:.1}, bed {:.2} → {:.2} over the bank, first finish square at s {:.0} ({:.1} m from the waterline), {} shore squares finish on the centre line", sec.label, sec.water_from, sec.bed.iter().flatten().cloned().fold(f32::INFINITY, f32::min).max(open_bed), sec.bed.iter().flatten().cloned().fold(f32::NEG_INFINITY, f32::max), ff, ff - sec.water_from, sec.slabs.len()));
    }
    // row 5: the BlueBay canal — two Beach dead-ends back to back (s 0..32 bank A toward +s, s 32..64 its mirror)
    {
        let coll = 0x1cu32;
        let (_, open_bed) = mapgeom::floor_finish::water_tiles(coll);
        let surface = mapgeom::floor_finish::water_surface(coll);
        let sec = section(&mut store, coll, "Beach", 0x1005, open_bed)?;
        let sec_top = top_margin + row_h * 4.0 + 30.0;
        let base_y = sec_top + sec_h;
        let sy = |y: f32| base_y - y * px_m;
        let sx = |s: f32| x0 + s * px_m;
        grid(&mut cv, base_y);
        cv.text(x0, sec_top - 26.0, "BlueBay CANAL  -  two Beach dead-ends back to back (the Fall 14 channel): a V trough, the finish lane along its bottom", 2, INK);
        let (wf0, wf1) = (sec.water_from, 64.0 - sec.water_from);
        cv.poly(&[(sx(wf0), sy(surface)), (sx(wf1), sy(surface)), (sx(wf1), sy(open_bed)), (sx(wf0), sy(open_bed))], WATER, 0.18);
        for mirror in [false, true] {
            let m = |s: f32| if mirror { 64.0 - s } else { s };
            let mut prev: Option<(f32, f32)> = None;
            for (i, t) in sec.top.iter().enumerate() {
                let s = i as f32 + 0.5;
                if let Some(y) = t.or(sec.bed[i]) {
                    let p = (sx(m(s)), sy(y));
                    if let Some(q) = prev {
                        cv.line(q.0, q.1, p.0, p.1, BED, 2.0, 1.0);
                        cv.poly(&[(q.0, q.1), (p.0, p.1), (p.0, sy(0.0)), (q.0, sy(0.0))], if s < sec.water_from { LAND } else { SLOPE }, 0.45);
                    }
                    prev = Some(p);
                }
            }
            prev = None;
            for (i, b) in sec.bed.iter().enumerate() {
                let s = i as f32 + 0.5;
                if let Some(y) = *b {
                    let p = (sx(m(s)), sy(y));
                    if let Some(q) = prev {
                        cv.line(q.0, q.1, p.0, p.1, BED, 2.0, 1.0);
                        cv.poly(&[(q.0, q.1), (p.0, p.1), (p.0, sy(0.0)), (q.0, sy(0.0))], SLOPE, 0.45);
                    }
                    prev = Some(p);
                }
            }
            for (s0, s1, ybed) in &sec.slabs {
                let (a, b) = (m(*s0), m(*s1));
                cv.poly(&[(sx(a.min(b)), sy(ybed + 0.4)), (sx(a.max(b)), sy(ybed + 0.4)), (sx(a.max(b)), sy(ybed - 0.1)), (sx(a.min(b)), sy(ybed - 0.1))], FINISH, 0.9);
            }
        }
        cv.line(sx(wf0), sy(surface), sx(wf1), sy(surface), WATER, 2.0, 1.0);
        cv.dashed(sx(wf0), sy(surface - 2.0), sx(wf1), sy(surface - 2.0), DEPTH2, 2.0, 5.0);
        cv.poly(&[(sx(wf0), sy(open_bed + 0.75)), (sx(wf1), sy(open_bed + 0.75)), (sx(wf1), sy(open_bed)), (sx(wf0), sy(open_bed))], BAND, 0.5);
        cv.line(sx(32.0), sy(0.0), sx(32.0), sy(11.0), [90, 90, 90], 1.0, 1.0);
        for wf in [wf0, wf1] {
            cv.line(sx(wf), sy(surface) - 26.0, sx(wf), sy(surface), INK, 1.0, 1.0);
            cv.text(sx(wf) - 28.0, sy(surface) - 38.0, "waterline", 1, INK);
        }
        let lane0 = sec.slabs.iter().map(|s| s.0).fold(f32::INFINITY, f32::min);
        let lane_w = 2.0 * (32.0 - lane0);
        dim(&mut cv, sx(lane0), sx(64.0 - lane0), base_y + 22.0, &format!("{lane_w:.0} m finish lane along the trough bottom (bed {:.2} .. {:.2}), the banks above: no finish", open_bed, open_bed + 0.75));
        cv.text(sx(0.0) + 4.0, sy(11.0) + 3.0, "Beach dead-end facing East", 1, [90, 90, 90]);
        cv.text(sx(46.0), sy(11.0) + 3.0, "Beach dead-end facing West", 1, [90, 90, 90]);
        let shallow = surface - sec.bed.iter().flatten().cloned().fold(f32::NEG_INFINITY, f32::max);
        cv.text(sx(24.0), sy(surface) - 13.0, &format!("water {shallow:.1} .. {:.1} m deep across the trough", surface - open_bed), 1, DEPTH2);
        report.push(format!("canal: {lane_w:.0} m lane (bed {open_bed:.2} .. {:.2}) between two {} banks", open_bed + 0.75, sec.label));
    }
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {w}x{h}");
    for l in report {
        println!("  {l}");
    }
    Ok(())
}
