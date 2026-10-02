//! `e9_frame2 EDITOR.Map.Gbx --records R.tsv [--lamps lamps.tsv] [--out TSV]` (E9, 2026-10-02): THE STORAGE-2 FRAME of an editor bake —
//! frame 2 ("LightMap%u_LocalBig_Avg", RE 18) against frame 1 (the local-light frame), per chart: the lit texels of each, the overlap, the
//! value ratio f2/f1 where both are lit, the chroma of frame 2 (grey?), and — with a `lmtool map-lights` table — the nearest lamp's class
//! words (ball_flags / gx_flags / radius / cone) of every frame-2-lit chart: which lamp class the frame carries, how its amplitude relates to
//! frame 1. Values decoded with classcmp::texel_hdr (the record's MaxHDR, the per-chart frame byte).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let path = a.get(1).expect("EDITOR.Map.Gbx");
    let records = f("--records").expect("--records R.tsv");
    let lit_hdr: f64 = f("--lit-hdr").map(|v| v.parse().expect("--lit-hdr F")).unwrap_or(1e-3);
    let out = f("--out");
    let m = lightmap::mapio::load(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let d = m.chunk.data.as_ref().expect("lightmap");
    let mp = d.cache.mapping().expect("mapping");
    if d.frames.len() < 3 { eprintln!("{path}: {} frames — no Storage-2 frame", d.frames.len()); return; }
    let i1 = lightmap::img::decode_webp(&d.frames[1].images[0]).expect("frame 1 image 0");
    let i2 = lightmap::img::decode_webp(&d.frames[2].images[0]).expect("frame 2 image 0");
    let k1 = lightmap::classcmp::record_maxhdr(&mp, 1).expect("record 1");
    let k2 = lightmap::classcmp::record_maxhdr(&mp, 2).expect("record 2");
    let (fb1, fb2) = (&mp.frame_bytes[1], &mp.frame_bytes[2]);
    eprintln!("{path}: {} charts, frame 1 {}×{} MaxHDR {k1}, frame 2 {}×{} MaxHDR {k2}; frame bytes non-zero f1 {} f2 {}", mp.count, i1.w, i1.h, i2.w, i2.h, fb1.iter().filter(|&&b| b != 0).count(), fb2.iter().filter(|&&b| b != 0).count());
    let rows_v = lightmap::classcmp::read_records_tsv(&records).unwrap_or_else(|e| panic!("{e}"));
    let rows: std::collections::HashMap<(u32, u32), lightmap::classcmp::RecRow> = rows_v.iter().map(|r| ((r.obj, r.sub), r.clone())).collect();
    // lamps: x y z radius + the class words
    struct LampRow { p: [f32; 3], r: f32, ball: String, gx: String, cone: String, emit: String, owner: String }
    let lamps: Vec<LampRow> = f("--lamps").map(|p| {
        let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("--lamps {p}: {e}"));
        let mut lines = txt.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
        let col = |n: &str| head.iter().position(|h| *h == n).unwrap_or_else(|| panic!("--lamps: no {n} column"));
        let (cx, cy, cz, cr) = (col("x"), col("y"), col("z"), col("radius"));
        let (cb, cg, ci, co, cel, ceu, cow) = (col("ball_flags"), col("gx_flags"), col("cone_inner"), col("cone_outer"), col("emit_left"), col("emit_up"), col("owner"));
        lines.filter(|l| !l.trim().is_empty()).map(|l| { let c: Vec<&str> = l.split('\t').collect(); LampRow { p: [c[cx].parse().unwrap(), c[cy].parse().unwrap(), c[cz].parse().unwrap()], r: c[cr].parse().unwrap(), ball: c[cb].to_string(), gx: c[cg].to_string(), cone: format!("{}/{}", c[ci], c[co]), emit: format!("{}/{}", c[cel], c[ceu]), owner: c[cow].to_string() } }).collect()
    }).unwrap_or_default();
    eprintln!("--lamps: {} lamps", lamps.len());
    #[derive(Default)]
    struct Acc { charts: usize, tex: usize, lit1: usize, lit2: usize, both: usize, only2: usize, s1: [f64; 3], s2: [f64; 3], s1_both: [f64; 3], s2_both: [f64; 3], nongrey2: usize, maxcd: f64 }
    let mut by_class: std::collections::BTreeMap<String, Acc> = Default::default();
    let mut by_lamp: std::collections::BTreeMap<String, Acc> = Default::default();
    let mut total = Acc::default();
    let mut lines_out: Vec<String> = vec!["chart\tclass\tname\tw\th\tfb1\tfb2\tlit1\tlit2\tboth\tonly2\tmean1_both\tmean2_both\tratio\tnongrey2\tmaxcd2\tnearest_lamp\td\tR\tball\tgx\tcone\temit".into()];
    for i in 0..mp.count as usize {
        let obj = mp.binds[i].obj_group_idx / 4;
        let sub = mp.binds[i].obj_idx & 0x00ff_ffff;
        let (class, name) = rows.get(&(obj, sub)).map(|r| (r.class.clone(), r.name.clone())).unwrap_or(("?".into(), "?".into()));
        let (px, py, pw, ph) = lightmap::classcmp::chart_own_px(mp.pos[i], mp.size[i]);
        let mut e = Acc { charts: 1, ..Default::default() };
        for y in py..(py + ph).min(i1.h) { for x in px..(px + pw).min(i1.w) {
            let a1 = i1.get(x, y); let a2 = i2.get(x, y);
            let v1: Vec<f64> = (0..3).map(|c| lightmap::classcmp::texel_hdr(1, a1[c], fb1[i], k1)).collect();
            let v2: Vec<f64> = (0..3).map(|c| lightmap::classcmp::texel_hdr(2, a2[c], fb2[i], k2)).collect();
            let l1 = v1.iter().cloned().fold(0.0, f64::max) >= lit_hdr;
            let l2 = v2.iter().cloned().fold(0.0, f64::max) >= lit_hdr;
            e.tex += 1;
            if l1 { e.lit1 += 1; for c in 0..3 { e.s1[c] += v1[c]; } }
            if l2 { e.lit2 += 1; for c in 0..3 { e.s2[c] += v2[c]; } let cd = (a2[0] as f64 - a2[1] as f64).abs().max((a2[1] as f64 - a2[2] as f64).abs()); if cd > 2.0 { e.nongrey2 += 1; } e.maxcd = e.maxcd.max(cd); }
            if l1 && l2 { e.both += 1; for c in 0..3 { e.s1_both[c] += v1[c]; e.s2_both[c] += v2[c]; } }
            if l2 && !l1 { e.only2 += 1; }
        } }
        // the nearest lamp to the chart's record centre (the records TSV carries the centre)
        let nearest = rows.get(&(obj, sub)).and_then(|r| { let c = [r.centre_x?, r.centre_y, r.centre_z?]; lamps.iter().map(|l| { let dd: f32 = (0..3).map(|k| (l.p[k] - c[k]).powi(2)).sum(); (dd.sqrt(), l) }).min_by(|a, b| a.0.total_cmp(&b.0)) });
        let lamp_key = nearest.map(|(_, l)| format!("R {} ball {} gx {} cone {} emit {}", l.r, l.ball, l.gx, l.cone, l.emit)).unwrap_or("-".into());
        let ratio = if e.both > 0 && e.s1_both[1] > 0.0 { e.s2_both[1] / e.s1_both[1] } else { f64::NAN };
        if e.lit2 > 0 {
            lines_out.push(format!("{i}\t{class}\t{name}\t{pw}\t{ph}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.5}\t{:.5}\t{:.3}\t{}\t{}\t{}\t{:.1}\t{}", fb1[i], fb2[i], e.lit1, e.lit2, e.both, e.only2, e.s1_both[1] / e.both.max(1) as f64, e.s2_both[1] / e.both.max(1) as f64, ratio, e.nongrey2, e.maxcd, nearest.map(|(_, l)| l.owner.clone()).unwrap_or("-".into()), nearest.map(|(d, _)| d).unwrap_or(f32::NAN), lamp_key));
        }
        let add = |t: &mut Acc, e: &Acc| { t.charts += e.charts; t.tex += e.tex; t.lit1 += e.lit1; t.lit2 += e.lit2; t.both += e.both; t.only2 += e.only2; for c in 0..3 { t.s1[c] += e.s1[c]; t.s2[c] += e.s2[c]; t.s1_both[c] += e.s1_both[c]; t.s2_both[c] += e.s2_both[c]; } t.nongrey2 += e.nongrey2; t.maxcd = t.maxcd.max(e.maxcd); };
        add(&mut total, &e);
        add(by_class.entry(format!("{class}:{name}")).or_default(), &e);
        if e.lit2 > 0 || e.lit1 > 0 { add(by_lamp.entry(lamp_key).or_default(), &e); }
    }
    let show = |k: &str, e: &Acc| {
        let r = |c: usize| if e.both > 0 && e.s1_both[c] > 0.0 { e.s2_both[c] / e.s1_both[c] } else { f64::NAN };
        println!("{k}\tcharts {}\ttexels {}\tlit f1 {} f2 {} both {} f2-only {}\tmean f1 {:.4}/{:.4}/{:.4} f2 {:.4}/{:.4}/{:.4} (over lit)\tf2/f1 over both {:.3}/{:.3}/{:.3}\tnon-grey f2 {} (max cd {})", e.charts, e.tex, e.lit1, e.lit2, e.both, e.only2, e.s1[0] / e.lit1.max(1) as f64, e.s1[1] / e.lit1.max(1) as f64, e.s1[2] / e.lit1.max(1) as f64, e.s2[0] / e.lit2.max(1) as f64, e.s2[1] / e.lit2.max(1) as f64, e.s2[2] / e.lit2.max(1) as f64, r(0), r(1), r(2), e.nongrey2, e.maxcd);
    };
    show("TOTAL", &total);
    println!("\n== by the nearest lamp's class (charts with any frame-1 or frame-2 light)");
    let mut bl: Vec<(&String, &Acc)> = by_lamp.iter().collect();
    bl.sort_by(|a, b| b.1.lit2.cmp(&a.1.lit2));
    for (k, e) in bl { show(k, e); }
    println!("\n== by item class, the frame-2-lit ones");
    let mut bc: Vec<(&String, &Acc)> = by_class.iter().filter(|(_, e)| e.lit2 > 0).collect();
    bc.sort_by(|a, b| b.1.lit2.cmp(&a.1.lit2));
    for (k, e) in bc.iter().take(40) { show(k, e); }
    if let Some(o) = out { std::fs::write(&o, lines_out.join("\n") + "\n").unwrap_or_else(|e| panic!("{o}: {e}")); eprintln!("wrote {o} ({} frame-2-lit charts)", lines_out.len() - 1); }
}
