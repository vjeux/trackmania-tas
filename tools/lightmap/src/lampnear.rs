//! The lamp ↔ receiver table for RE 13's lamp-class read (coordinator, 2026-09-26 22:12Z).
//!
//! `lmtool bake … --lights-tsv FILE` writes every lamp the bake would drive (id, owner, model, position, radius, r_eff,
//! intensity, colour, NightOnly, the GxLight ball flags / flags, the emitter-area sample count).
//!
//! `lmtool lampnear EDITOR.Map.Gbx --records REC.tsv --lights LIGHTS.tsv [--frame 1] [--lit 8] [--min-lit 0.05] [--out TSV]`:
//! for every chart of the EDITOR's file, the lit fraction of its texels in frame `frame`, its class / model / centre (from the
//! bake's records table — the 9-column form with x/z), the NEAREST lamp (3-D distance from the chart centre to the lamp
//! position), how many lamps reach the centre (distance < r_eff), and that lamp's model + flags; then two histograms — the
//! nearest-lamp models of the LIT receivers and of the UNLIT receivers within a lamp's reach. Same model lights one receiver
//! and not another → a per-instance rule; different models → the flags.

use crate::classcmp::RecRow;

#[derive(Clone, Debug)]
pub struct LampRow {
    pub id: u16,
    pub owner: String,
    pub model: String,
    pub pos: [f32; 3],
    pub radius: f32,
    pub r_eff: f32,
    pub intensity: f32,
    pub rgb: [f32; 3],
    pub night_only: bool,
    pub ball_flags: u32,
    pub gx_flags: u32,
    pub samples: usize,
    /// the spot axis and the (inner, outer) cone angles in degrees when the table carries them ((180, 180) = a ball)
    pub dir: Option<[f32; 3]>,
    pub cone: (f32, f32),
}

/// The bake's `--lights-tsv FILE` writer; `map_path` resolves "item N" owners to the map's item model.
/// The lamp table; `instances` = the lamp pass scene's instances (a lamp's owner "item N" is the scene INSTANCE index — items without a
/// model are not instances and a reduced oracle's kept set drops more, so N is NOT the map item index; E 2026-09-27 20:10Z: the model
/// column named AC16497083 for the arch lamps of AC16497075). Without the scene the map's item list is used as before (wrong when
/// indices shift).
pub fn write_lights_tsv(path: &str, lamps: &[crate::localdrive::Lamp], map_path: &str, instances: Option<&[crate::geometry::Instance]>) -> Result<(), String> {
    use std::io::Write;
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(map_path));
    let mut fh = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    writeln!(fh, "id\towner\tmodel\tx\ty\tz\tradius\tr_eff\tintensity\tr\tg\tb\tnight_only\tball_flags\tgx_flags\tsamples\tdx\tdy\tdz\tcone_inner\tcone_outer\tleft_x\tleft_y\tleft_z\tup_x\tup_y\tup_z").map_err(|e| e.to_string())?;
    for l in lamps {
        let idx = l.owner.strip_prefix("item ").and_then(|n| n.trim().parse::<usize>().ok());
        let item_idx = match (idx, instances) { (Some(i), Some(inst)) => inst.get(i).map(|x| x.item), (Some(i), None) => Some(i), _ => None };
        let model = match item_idx.and_then(|i| mf.items.get(i)) { Some(it) => it.model.rsplit('\\').next().unwrap_or(&it.model).to_string(), None => l.owner.clone() };
        let p = l.light.pos;
        let dd = l.light.dir;
        writeln!(fh, "{}\t{}\t{model}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}\t{:#x}\t{:#x}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:.1}\t{:.1}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}", l.id, l.owner, p[0], p[1], p[2], l.light.radius, l.r_eff, l.light.intensity, l.light.color[0], l.light.color[1], l.light.color[2], l.light.night_only, l.light.ball_flags, l.light.gx_flags, 1usize /* the emitter-sample count: E's emitter-area commit is reverted in this base, so every lamp is one sample */, dd[0], dd[1], dd[2], l.light.cone.0, l.light.cone.1, l.light.left[0], l.light.left[1], l.light.left[2], l.light.up[0], l.light.up[1], l.light.up[2]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn read_lights_tsv(path: &str) -> Result<Vec<LampRow>, String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for (ln, line) in txt.lines().enumerate() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 16 { continue; }
        let Ok(id) = f[0].parse::<u16>() else { if ln == 0 { continue } else { return Err(format!("{path}:{}: id {:?}", ln + 1, f[0])) } };
        let num = |i: usize| f[i].parse::<f32>().map_err(|e| format!("{path}:{}: col {i}: {e}", ln + 1));
        let hex = |i: usize| u32::from_str_radix(f[i].trim_start_matches("0x"), 16).map_err(|e| format!("{path}:{}: col {i}: {e}", ln + 1));
        out.push(LampRow { id, owner: f[1].to_string(), model: f[2].to_string(), pos: [num(3)?, num(4)?, num(5)?], radius: num(6)?, r_eff: num(7)?, intensity: num(8)?, rgb: [num(9)?, num(10)?, num(11)?], night_only: f[12] == "true", ball_flags: hex(13)?, gx_flags: hex(14)?, samples: f[15].parse().unwrap_or(1), dir: if f.len() >= 21 { Some([num(16)?, num(17)?, num(18)?]) } else { None }, cone: if f.len() >= 21 { (num(19)?, num(20)?) } else { (180.0, 180.0) } });
    }
    Ok(out)
}

pub struct NearRow {
    pub chart: usize,
    pub class: String,
    pub name: String,
    pub centre: [f32; 3],
    pub texels: usize,
    pub lit: usize,
    pub nearest: Option<(u16, f32)>,
    pub reaching: usize,
    /// lamps within r_eff whose spot cone (half the outer angle about the axis) also contains the chart centre
    pub in_cone: usize,
}

/// The table over the editor's charts (matched to the records by (obj, sub) bind word).
pub fn table(editor: &crate::mapio::MapLightmap, records: &[RecRow], lamps: &[LampRow], frame: usize, lit_thr: u8) -> Result<Vec<NearRow>, String> {
    let d = editor.chunk.data.as_ref().ok_or("a map without a lightmap")?;
    let m = d.cache.mapping().ok_or("a map without a mapping chunk")?;
    let img = crate::img::decode_webp(d.frames.get(frame).and_then(|f| f.images.first()).ok_or("no such frame")?)?;
    let rows: std::collections::HashMap<(u32, u32), &RecRow> = records.iter().map(|r| ((r.obj, r.sub), r)).collect();
    let mut out = Vec::with_capacity(m.count as usize);
    for i in 0..m.count as usize {
        let key = (m.binds[i].obj_group_idx / 4, m.binds[i].obj_idx & 0x00ff_ffff);
        let Some(r) = rows.get(&key) else { continue };
        let (Some(cx), Some(cz)) = (r.centre_x, r.centre_z) else { return Err("the records table has no centre_x / centre_z columns — re-bake with the 9-column --records-tsv".into()) };
        let centre = [cx, r.centre_y, cz];
        let (px, py, pw, ph) = crate::classcmp::chart_own_px(m.pos[i], m.size[i]);
        let (mut texels, mut lit) = (0usize, 0usize);
        for y in py..(py + ph).min(img.h) { for x in px..(px + pw).min(img.w) { texels += 1; let a = img.get(x, y); if a[0].max(a[1]).max(a[2]) >= lit_thr { lit += 1; } } }
        let mut nearest: Option<(u16, f32)> = None;
        let mut reaching = 0usize;
        let mut in_cone = 0usize;
        for l in lamps {
            let d2 = (l.pos[0] - centre[0]).powi(2) + (l.pos[1] - centre[1]).powi(2) + (l.pos[2] - centre[2]).powi(2);
            let dist = d2.sqrt();
            if dist < l.r_eff {
                reaching += 1;
                let inside = match l.dir { Some(dd) if l.cone.1 < 179.0 && dist > 1e-3 => { let v = [(centre[0] - l.pos[0]) / dist, (centre[1] - l.pos[1]) / dist, (centre[2] - l.pos[2]) / dist]; let n = (dd[0] * dd[0] + dd[1] * dd[1] + dd[2] * dd[2]).sqrt().max(1e-6); let cosang = (v[0] * dd[0] + v[1] * dd[1] + v[2] * dd[2]) / n; cosang >= (l.cone.1.to_radians() * 0.5).cos() } _ => true };
                if inside { in_cone += 1; }
            }
            if nearest.map(|(_, nd)| dist < nd).unwrap_or(true) { nearest = Some((l.id, dist)); }
        }
        out.push(NearRow { chart: i, class: r.class.clone(), name: r.name.clone(), centre, texels, lit, nearest, reaching, in_cone });
    }
    Ok(out)
}

pub fn print(rows: &[NearRow], lamps: &[LampRow], min_lit: f64, out: Option<&str>) -> Result<(), String> {
    let lamp_of = |id: u16| lamps.iter().find(|l| l.id == id);
    let mut sorted: Vec<&NearRow> = rows.iter().collect();
    sorted.sort_by(|a, b| (b.lit as f64 / b.texels.max(1) as f64).partial_cmp(&(a.lit as f64 / a.texels.max(1) as f64)).unwrap());
    let mut tsv = String::from("chart\tclass\tname\tcx\tcy\tcz\ttexels\tlit_pct\tnearest_lamp\tnearest_model\tdist\tr_eff\tgx_flags\tball_flags\tnight_only\tsamples\treaching\tin_cone\n");
    let (mut lit_hist, mut unlit_hist): (std::collections::BTreeMap<String, usize>, std::collections::BTreeMap<String, usize>) = Default::default();
    let (mut n_lit, mut n_unlit_reach) = (0usize, 0usize);
    for r in &sorted {
        let frac = r.lit as f64 / r.texels.max(1) as f64;
        let (lid, lmodel, dist, reff, gx, ball, no, ns) = match r.nearest.and_then(|(id, d)| lamp_of(id).map(|l| (id, d, l))) { Some((id, d, l)) => (id.to_string(), l.model.clone(), format!("{d:.2}"), format!("{:.2}", l.r_eff), format!("{:#x}", l.gx_flags), format!("{:#x}", l.ball_flags), l.night_only.to_string(), l.samples.to_string()), None => ("—".into(), "—".into(), "—".into(), "—".into(), "—".into(), "—".into(), "—".into(), "—".into()) };
        tsv.push_str(&format!("{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}\t{:.1}\t{lid}\t{lmodel}\t{dist}\t{reff}\t{gx}\t{ball}\t{no}\t{ns}\t{}\t{}\n", r.chart, r.class, r.name, r.centre[0], r.centre[1], r.centre[2], r.texels, 100.0 * frac, r.reaching, r.in_cone));
        if frac >= min_lit { n_lit += 1; *lit_hist.entry(format!("{lmodel} {gx}/{ball} night_only {no}")).or_default() += 1; }
        else if r.in_cone > 0 { n_unlit_reach += 1; *unlit_hist.entry(format!("{lmodel} {gx}/{ball} night_only {no}")).or_default() += 1; }
    }
    println!("{} charts; {} LIT (≥ {:.0} % of texels) — their nearest lamp: model gx_flags/ball_flags night_only → count", rows.len(), n_lit, 100.0 * min_lit);
    for (k, v) in &lit_hist { println!("  lit\t{v}\t{k}"); }
    println!("{} UNLIT charts within a lamp's r_eff AND its spot cone — their nearest lamp:", n_unlit_reach);
    for (k, v) in &unlit_hist { println!("  unlit\t{v}\t{k}"); }
    println!("lamp models in the bake: {}", { let mut h: std::collections::BTreeMap<(String, u32, u32, bool), usize> = Default::default(); for l in lamps { *h.entry((l.model.clone(), l.gx_flags, l.ball_flags, l.night_only)).or_default() += 1; } h.iter().map(|((m, g, b, n), c)| format!("{m} {g:#x}/{b:#x} night_only {n} ×{c}")).collect::<Vec<_>>().join("; ") });
    println!("top lit charts: chart, class:name, centre, lit %, nearest lamp (model, dist, r_eff), reaching");
    for r in sorted.iter().take(25) { let frac = r.lit as f64 / r.texels.max(1) as f64; if frac < min_lit { break; } let near = r.nearest.and_then(|(id, d)| lamp_of(id).map(|l| format!("{} #{id} at {d:.1} m (r_eff {:.1})", l.model, l.r_eff))).unwrap_or_else(|| "—".into()); println!("  {}\t{}:{}\t({:.1}, {:.1}, {:.1})\t{:.1} %\t{near}\t{} reaching", r.chart, r.class, r.name, r.centre[0], r.centre[1], r.centre[2], 100.0 * frac, r.reaching); }
    if let Some(p) = out { std::fs::write(p, tsv).map_err(|e| format!("{p}: {e}"))?; }
    Ok(())
}
