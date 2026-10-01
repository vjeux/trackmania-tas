//! `e8_walkdiff LAYOUT.tsv GAME.Map.Gbx [--show N]` — our layout table against the game's mapping in PLACEMENT ORDER (the
//! `walk` column: 0 = placed first): the first entries whose charts differ from the game's rects, with their sizes — a size
//! difference at the first divergence is the fit/carry rule, a position difference with the same size is the tree walk
//! (E8, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e8_walkdiff LAYOUT.tsv GAME.Map.Gbx [--show N]"); std::process::exit(2); }
    let flag = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let show: usize = flag("--show").map(|v| v.parse().unwrap()).unwrap_or(12);
    let txt = std::fs::read_to_string(&a[1]).expect("layout tsv");
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| hdr.iter().position(|h| *h == n).unwrap_or_else(|| panic!("no column {n}"));
    let (cobj, cx, cy, cw, ch, cex, cey, cname, cwalk, centry, cerect, cnb, cna, cord, carea) = (col("obj"), col("x"), col("y"), col("w"), col("h"), col("ext_x"), col("ext_y"), col("name"), col("walk"), col("entry"), col("entry_rect"), col("nb"), col("na"), col("ord"), col("area"));
    struct Row { obj: u32, x: i32, y: i32, w: i32, h: i32, ext: [f32; 2], name: String, walk: usize, entry: usize, erect: String, nb: u32, na: u32, ord: u32, area: String }
    let mut rows: Vec<Row> = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        rows.push(Row { obj: f[cobj].parse().unwrap(), x: f[cx].parse().unwrap(), y: f[cy].parse().unwrap(), w: f[cw].parse().unwrap(), h: f[ch].parse().unwrap(), ext: [f[cex].parse().unwrap(), f[cey].parse().unwrap()], name: f[cname].to_string(), walk: f[cwalk].parse().unwrap_or(usize::MAX), entry: f[centry].parse().unwrap_or(usize::MAX), erect: f[cerect].to_string(), nb: f[cnb].parse().unwrap_or(0), na: f[cna].parse().unwrap_or(0), ord: f[cord].parse().unwrap_or(0), area: f[carea].to_string() });
    }
    let lb = lightmap::mapio::load(&a[2]).unwrap_or_else(|e| panic!("{}: {e}", a[2]));
    let db = lb.chunk.data.as_ref().expect("no lightmap");
    let mb = db.cache.mapping().expect("no mapping");
    let mut by_obj: std::collections::HashMap<u32, usize> = Default::default();
    for c in 0..mb.count as usize { by_obj.insert(mb.binds[c].obj_group_idx / 4, c); }
    // per entry (walk order): all members identical? sizes equal? the game's bounding rect of the members vs ours
    let mut entries: std::collections::BTreeMap<usize, Vec<&Row>> = Default::default();
    for r in &rows { entries.entry(r.entry).or_default().push(r); }
    let mut by_walk: Vec<(usize, usize)> = entries.iter().map(|(e, m)| (m[0].walk, *e)).collect();
    by_walk.sort();
    let mut n_ok = 0usize; let mut n_shown = 0usize; let mut first_bad: Option<usize> = None;
    let mut n_size_bad = 0usize; let mut n_pos_bad = 0usize;
    for (walk, e) in &by_walk {
        let m = &entries[e];
        let mut all_same = true; let mut size_same = true;
        let (mut gx0, mut gy0, mut gx1, mut gy1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for r in m {
            let Some(&c) = by_obj.get(&r.obj) else { all_same = false; continue };
            let (p, s) = (mb.pos[c], mb.size[c]);
            if (r.w, r.h) != (s.0 as i32, s.1 as i32) { size_same = false; }
            if (r.x, r.y, r.w, r.h) != (p.0 as i32, p.1 as i32, s.0 as i32, s.1 as i32) { all_same = false; }
            gx0 = gx0.min(p.0 as i32 - 1); gy0 = gy0.min(p.1 as i32 - 1); gx1 = gx1.max(p.0 as i32 + s.0 as i32 + 1); gy1 = gy1.max(p.1 as i32 + s.1 as i32 + 1);
        }
        if all_same { n_ok += 1; continue; }
        if !size_same { n_size_bad += 1; } else { n_pos_bad += 1; }
        if first_bad.is_none() { first_bad = Some(*walk); }
        if n_shown < show {
            n_shown += 1;
            let r0 = m[0];
            let members: Vec<String> = m.iter().take(4).map(|r| { let c = by_obj[&r.obj]; format!("obj {} ord {} ours ({}, {}) {}×{} game ({}, {}) {}×{}", r.obj, r.ord, r.x, r.y, r.w, r.h, mb.pos[c].0, mb.pos[c].1, mb.size[c].0, mb.size[c].1) }).collect();
            println!("walk {walk}: entry {e} {} ×{} ({}×{} grid, area {}) ext {:.3}×{:.3} ours rect {} game members' bbox ({gx0}, {gy0}) {}×{} [{}]: {}", r0.name, m.len(), r0.nb, r0.na, r0.area, r0.ext[0], r0.ext[1], r0.erect, gx1 - gx0, gy1 - gy0, if size_same { "POSITION" } else { "SIZE" }, members.join("; "));
        }
    }
    println!("{} entries: {n_ok} identical in every member; {n_size_bad} with a size difference, {n_pos_bad} position-only; first divergence at walk {:?}", by_walk.len(), first_bad);
    // the walk's prefix: how many entries from walk 0 are identical before the first divergence, and the identical count overall
    let mut prefix = 0usize;
    for (walk, e) in &by_walk { let m = &entries[e]; let ok = m.iter().all(|r| by_obj.get(&r.obj).map(|&c| (r.x, r.y, r.w, r.h) == (mb.pos[c].0 as i32, mb.pos[c].1 as i32, mb.size[c].0 as i32, mb.size[c].1 as i32)).unwrap_or(false)); if !ok { println!("prefix of identical entries: {prefix} (walk 0..{walk})"); break; } prefix += 1; }
}
