//! `e8_objrects LAYOUT.tsv GAME.Map.Gbx --class C` — every record of class C: obj, name, entry/ordinal, our rect, the game's rect,
//! sorted by the game's position (the shape of the game's entries for a class we place differently) (E8, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e8_objrects LAYOUT.tsv GAME.Map.Gbx --class C"); std::process::exit(2); }
    let flag = |n: &str| a.iter().position(|x| x == n).and_then(|i| a.get(i + 1).cloned());
    let class = flag("--class").unwrap_or_else(|| "item0".into());
    let txt = std::fs::read_to_string(&a[1]).expect("layout tsv");
    let mut lines = txt.lines();
    let hdr: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| hdr.iter().position(|h| *h == n).unwrap_or_else(|| panic!("no column {n}"));
    let (cc, cobj, cx, cy, cw, ch, cname, centry, cord, cnb, cna, cerect, cwalk) = (col("class"), col("obj"), col("x"), col("y"), col("w"), col("h"), col("name"), col("entry"), col("ord"), col("nb"), col("na"), col("entry_rect"), col("walk"));
    let lb = lightmap::mapio::load(&a[2]).unwrap_or_else(|e| panic!("{}: {e}", a[2]));
    let db = lb.chunk.data.as_ref().expect("no lightmap");
    let mb = db.cache.mapping().expect("no mapping");
    let mut by_obj: std::collections::HashMap<u32, usize> = Default::default();
    for c in 0..mb.count as usize { by_obj.insert(mb.binds[c].obj_group_idx / 4, c); }
    let mut rows: Vec<(u16, u16, String)> = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f[cc] != class { continue; }
        let obj: u32 = f[cobj].parse().unwrap();
        let Some(&c) = by_obj.get(&obj) else { continue };
        let (p, s) = (mb.pos[c], mb.size[c]);
        rows.push((p.0, p.1, format!("game ({}, {}) {}×{} bind sub {:#x} | ours ({}, {}) {}×{} entry {} ord {} grid {}×{} rect {} walk {} | obj {obj} {}", p.0, p.1, s.0, s.1, mb.binds[c].obj_idx, f[cx], f[cy], f[cw], f[ch], f[centry], f[cord], f[cnb], f[cna], f[cerect], f[cwalk], f[cname])));
    }
    rows.sort();
    for r in &rows { println!("{}", r.2); }
    println!("{} records of class {class}", rows.len());
}
