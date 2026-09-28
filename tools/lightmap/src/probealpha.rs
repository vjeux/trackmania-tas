//! `lmtool probe-alpha DUMPDIR OURS.Map.Gbx [--editor EDITOR.Map.Gbx] [--level L] [--tsv OUT]` — the bake's dumped colour fold
//! (`LMTOOL_PROBE_DUMP_DIR/probe-colour.f32`, `probebake::ProbeBake::dump`) read probe by probe: the world position (the block's
//! `pos` + cell · index, the shader's ProbeToWorld), α = the fold's fourth channel (Σ over the sweep-0 directions of 1/N where the
//! direction's first surface was not black — the game's validity accumulator, valid iff α ≥ 0.5), the colour; with `--editor`
//! the editor save's validity bit and colour bytes at the same world position (`probecmp::load_side`'s key). Per height level:
//! the α histogram of our probes split by the editor's verdict — the read of the buried rows (E4, 2026-09-28).

use std::collections::BTreeMap;

pub fn run(dump: &std::path::Path, ours: &str, editor: Option<&str>, only_level: Option<i32>, tsv: Option<&str>) -> Result<(), String> {
    let colour = crate::probebake::load_dump(&dump.join("probe-colour.f32")).map_err(|e| format!("probe-colour.f32: {e}"))?;
    let updown = crate::probebake::load_dump(&dump.join("probe-updown.f32")).ok();
    let m = crate::mapio::load(ours)?;
    let d = m.chunk.data.as_ref().ok_or("the map has no lightmap data")?;
    let v = crate::volume::Volume::parse(&d.cache.trailer)?;
    if (colour.w, colour.h, colour.d) != (v.grid[0], v.grid[1], v.grid[2]) {
        return Err(format!("the dump's volume {}×{}×{} is not the map's probe grid {:?}", colour.w, colour.h, colour.d, v.grid));
    }
    let ed = match editor {
        Some(p) => { let em = crate::mapio::load(p)?; Some(crate::probecmp::load_side(&em, "editor")?) }
        None => None,
    };
    let c = v.cell_size();
    // per level (probecmp's label y = pos.y + c·(L − ½)): [editor valid, editor invalid, editor absent] × α bins
    // bins: 0 = α == 0, 1 = 0 < α < 0.5, 2 = α == 0.5, 3 = α > 0.5
    let mut per_level: BTreeMap<i32, [[usize; 4]; 3]> = BTreeMap::new();
    let mut sum_alpha: BTreeMap<i32, [(f64, usize); 3]> = BTreeMap::new();
    let mut out = String::from("x\ty\tz\tlabel_y\talpha\tr\tg\tb\tud_r\tud_g\tud_b\teditor_valid\ted_r\ted_g\ted_b\n");
    let mut seen = std::collections::HashSet::new();
    for b in &v.blocks {
        for y in b.min[1]..b.max[1] {
            let label_y = (b.pos[1] + c * (y as f32 - 0.5)).round() as i32;
            if let Some(l) = only_level { if l != label_y { continue; } }
            for z in b.min[2]..b.max[2] { for x in b.min[0]..b.max[0] {
                if x >= colour.w || y >= colour.h || z >= colour.d { continue; }
                // the world position as the shader places the probe (index · cell + pos) and probecmp's key (the doc's half-cell form)
                let (wx, wy, wz) = (b.pos[0] + c * x as f32, b.pos[1] + c * y as f32, b.pos[2] + c * z as f32);
                let key = ((b.pos[0] + c * (x as f32 + 0.5)).round() as i32, label_y, (b.pos[2] + c * (z as f32 + 0.5)).round() as i32);
                if !seen.insert(key) { continue; }
                let alpha = colour.get(x, y, z, 3);
                let rgb = [colour.get(x, y, z, 0), colour.get(x, y, z, 1), colour.get(x, y, z, 2)];
                let ud = updown.as_ref().map(|u| [u.get(x, y, z, 0), u.get(x, y, z, 1), u.get(x, y, z, 2)]).unwrap_or([0.0; 3]);
                let e = ed.as_ref().and_then(|s| s.probes.get(&key));
                let (side, ev, eb) = match e { Some((bytes, valid, _)) => (if *valid { 0 } else { 1 }, if *valid { "1" } else { "0" }, bytes[0]), None => (2, "-", [0u8; 3]) };
                let bin = if alpha == 0.0 { 0 } else if alpha < 0.5 { 1 } else if alpha == 0.5 { 2 } else { 3 };
                per_level.entry(label_y).or_default()[side][bin] += 1;
                let s = &mut sum_alpha.entry(label_y).or_default()[side]; s.0 += alpha as f64; s.1 += 1;
                if tsv.is_some() { out.push_str(&format!("{wx}\t{wy}\t{wz}\t{label_y}\t{alpha}\t{}\t{}\t{}\t{}\t{}\t{}\t{ev}\t{}\t{}\t{}\n", rgb[0], rgb[1], rgb[2], ud[0], ud[1], ud[2], eb[0], eb[1], eb[2])); }
            } }
        }
    }
    println!("probe α per level (label y = pos.y + cell·(L − ½); the probe itself sits 8 m higher): our α bins [α = 0 | 0 < α < 0.5 | α = 0.5 | α > 0.5] and mean α, split by the editor's validity at the same probe");
    println!("label_y\teditor VALID: bins / mean α\teditor INVALID: bins / mean α\tno editor probe: bins / mean α");
    for (y, sides) in &per_level {
        let sa = &sum_alpha[y];
        let fmt = |k: usize| -> String { let s = sides[k]; let n: usize = s.iter().sum(); if n == 0 { "—".into() } else { format!("{:?} / {:.4}", s, sa[k].0 / sa[k].1 as f64) } };
        println!("{y:>6}\t{}\t{}\t{}", fmt(0), fmt(1), fmt(2));
    }
    if let Some(p) = tsv { std::fs::write(p, out).map_err(|e| format!("{p}: {e}"))?; }
    Ok(())
}
