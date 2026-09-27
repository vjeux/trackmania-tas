//! `lmtool layoutcmp LAYOUT.tsv EDITOR.Map.Gbx` — OUR LAYOUT TABLE AGAINST THE EDITOR'S MAPPING, entry by entry (port
//! engineer F, 2026-09-27; the giant-scale packer row).
//!
//! `LAYOUT.tsv` is `lmtool bake … --layout-game --layout-tsv FILE` (chart, class, obj, x, y, w, h, ext_x, ext_y, entry,
//! entry_rect, nb, na, ord); the editor's mapping is read from the map's lightmap cache. Chart k of the table is
//! mapping entry k (the bind words are checked first — a table whose bind order differs from the editor's is refused).
//!
//! Sections: (1) per class — charts, same rect, same size, our size vs the editor's size histogram; (2) per OUR entry —
//! the editor's rects of its members clustered into GRID COMPONENTS (members whose rects sit one gutter (2·pad) apart
//! along one axis and overlap on the other), i.e. the editor's own entries: a histogram of (our entry count → editor
//! component counts) reveals the editor's chunk size, and each component's bounding box its grid dims; (3) the editor's
//! scale estimate from the solo charts (w / ext).

use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug)]
pub struct Row {
    pub chart: usize,
    pub class: String,
    pub obj: u32,
    pub rect: (i32, i32, i32, i32),
    pub ext: [f32; 2],
    pub entry: usize,
    pub entry_rect: (i32, i32, i32, i32),
    pub nb: u32,
    pub na: u32,
    pub ord: u32,
    /// The record's sub index and model name (columns 15–16, written since F's layoutcmp; 0 / empty on older tables).
    pub sub: u32,
    pub name: String,
    /// The editor's mapping index of this record after the join (= `chart` when the orders agree).
    pub ed: usize,
}

pub fn read_layout_tsv(path: &str) -> Result<Vec<Row>, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = Vec::new();
    for (ln, line) in s.lines().enumerate() {
        if ln == 0 || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 14 {
            return Err(format!("{path}:{}: {} fields", ln + 1, f.len()));
        }
        let p = |i: usize| -> Result<i32, String> { f[i].trim().parse::<i32>().map_err(|e| format!("{path}:{}: field {i} {:?}: {e}", ln + 1, f[i])) };
        let pf = |i: usize| -> Result<f32, String> { f[i].trim().parse::<f32>().map_err(|e| format!("{path}:{}: field {i} {:?}: {e}", ln + 1, f[i])) };
        let er = {
            let t = f[10].trim().trim_start_matches('(').trim_end_matches(')');
            let v: Vec<i32> = t.split(',').filter_map(|x| x.trim().parse::<i32>().ok()).collect();
            if v.len() == 4 { (v[0], v[1], v[2], v[3]) } else { (-1, -1, 0, 0) }
        };
        out.push(Row {
            chart: p(0)? as usize,
            class: f[1].to_string(),
            obj: p(2)? as u32,
            rect: (p(3)?, p(4)?, p(5)?, p(6)?),
            ext: [pf(7)?, pf(8)?],
            entry: p(9)? as usize,
            entry_rect: er,
            nb: p(11)? as u32,
            na: p(12)? as u32,
            ord: p(13)? as u32,
            sub: if f.len() > 14 { f[14].trim().parse().unwrap_or(0) } else { 0 },
            name: if f.len() > 15 { f[15].to_string() } else { String::new() },
            ed: p(0)? as usize,
        });
    }
    Ok(out)
}

/// The class of a row for the per-class tables: `tile`, `block:NAME`, `clip:NAME`, `item:NAME` as the records table
/// writes them.
fn short_class(c: &str) -> String {
    c.to_string()
}

/// Gutter adjacency of two chart rects (both inset by `pad` inside their cells): one gutter apart on one axis and
/// overlapping on the other.
fn adjacent(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32), gutter: i32) -> bool {
    let (ax0, ay0, aw, ah) = a;
    let (bx0, by0, bw, bh) = b;
    let (ax1, ay1, bx1, by1) = (ax0 + aw, ay0 + ah, bx0 + bw, by0 + bh);
    let x_touch = ax1 + gutter == bx0 || bx1 + gutter == ax0;
    let y_touch = ay1 + gutter == by0 || by1 + gutter == ay0;
    let x_overlap = ax0 < bx1 && bx0 < ax1;
    let y_overlap = ay0 < by1 && by0 < ay1;
    (x_touch && y_overlap) || (y_touch && x_overlap)
}

struct Dsu(Vec<usize>);
impl Dsu {
    fn new(n: usize) -> Self { Dsu((0..n).collect()) }
    fn find(&mut self, i: usize) -> usize { let mut r = i; while self.0[r] != r { r = self.0[r]; } let mut c = i; while self.0[c] != r { let n = self.0[c]; self.0[c] = r; c = n; } r }
    fn union(&mut self, a: usize, b: usize) { let (ra, rb) = (self.find(a), self.find(b)); if ra != rb { self.0[ra] = rb; } }
}

/// The editor's components (its entries) among a set of chart rects: connected components of gutter adjacency.
pub fn components(rects: &[(i32, i32, i32, i32)], gutter: i32) -> Vec<Vec<usize>> {
    let n = rects.len();
    let mut d = Dsu::new(n);
    for i in 0..n {
        for j in i + 1..n {
            if adjacent(rects[i], rects[j], gutter) {
                d.union(i, j);
            }
        }
    }
    let mut by_root: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        by_root.entry(d.find(i)).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = by_root.into_values().collect();
    out.sort_by_key(|c| (rects[c[0]].1, rects[c[0]].0));
    out
}

/// The grid dims of a component: the distinct cell columns / rows (rects sharing an x0 form a column).
pub fn component_dims(rects: &[(i32, i32, i32, i32)], comp: &[usize]) -> (usize, usize, (i32, i32, i32, i32)) {
    let mut xs: Vec<i32> = comp.iter().map(|&i| rects[i].0).collect();
    let mut ys: Vec<i32> = comp.iter().map(|&i| rects[i].1).collect();
    xs.sort();
    xs.dedup();
    ys.sort();
    ys.dedup();
    let x0 = comp.iter().map(|&i| rects[i].0).min().unwrap();
    let y0 = comp.iter().map(|&i| rects[i].1).min().unwrap();
    let x1 = comp.iter().map(|&i| rects[i].0 + rects[i].2).max().unwrap();
    let y1 = comp.iter().map(|&i| rects[i].1 + rects[i].3).max().unwrap();
    (xs.len(), ys.len(), (x0, y0, x1 - x0, y1 - y0))
}

pub fn run(a: &[String]) -> Result<(), String> {
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let has = |k: &str| a.iter().any(|x| x == k);
    let lay_path = a.get(1).ok_or("usage: lmtool layoutcmp LAYOUT.tsv EDITOR.Map.Gbx [--pad 1] [--entries] [--class NAME] [--tsv OUT]")?;
    let ed_path = a.get(2).ok_or("EDITOR.Map.Gbx")?;
    let pad: i32 = f("--pad").map(|v| v.parse().unwrap()).unwrap_or(1);
    let gutter = 2 * pad;
    let rows = read_layout_tsv(lay_path)?;
    let ed = crate::mapio::load(ed_path)?;
    let d = ed.chunk.data.as_ref().ok_or("the editor map has no lightmap data")?;
    let m = d.cache.mapping().ok_or("no mapping chunk")?;
    // THE JOIN: chart k ↔ mapping entry k when the bind words agree there; otherwise (a table whose record order or count
    // differs from the editor's — g23's 74 267 vs 74 263) by the bind word obj·4 | sub (the table's `sub` column; tables
    // without it join by obj alone, ambiguous for multi-record objects). `row.ed` = the editor's
    // mapping index of the matched chart; unmatched rows are dropped (counted).
    let mut by_index_ok = 0usize;
    for r in &rows { if r.chart < m.count as usize && m.binds[r.chart].obj_group_idx / 4 == r.obj { by_index_ok += 1; } }
    let n_rows = rows.len();
    let rows: Vec<Row> = if by_index_ok == n_rows && m.count as usize == n_rows { rows } else {
        let has_sub = true; // the key is always obj·4 | sub (a table without the column has sub 0: multi-record objects then collide → ambiguous)
        let mut ed_of: HashMap<u32, Vec<usize>> = HashMap::new();
        for i in 0..m.count as usize { ed_of.entry(if has_sub { m.binds[i].obj_group_idx } else { m.binds[i].obj_group_idx / 4 }).or_default().push(i); }
        let mut out: Vec<Row> = Vec::with_capacity(n_rows);
        let (mut unmatched, mut ambiguous) = (0usize, 0usize);
        let mut used: std::collections::HashSet<usize> = Default::default();
        for r in rows {
            let key = if has_sub { r.obj * 4 | r.sub } else { r.obj };
            match ed_of.get(&key) { Some(v) if v.len() == 1 => { let mut r2 = r; r2.ed = v[0]; used.insert(v[0]); out.push(r2); } Some(_) => { ambiguous += 1; } None => { unmatched += 1; if unmatched <= 20 { println!("  unmatched row: chart {} {} obj {} sub {} {} ext ({}, {}) entry {} ours ({}, {}, {}, {})", r.chart, r.class, r.obj, r.sub, r.name, r.ext[0], r.ext[1], r.entry, r.rect.0, r.rect.1, r.rect.2, r.rect.3); } } }
        }
        println!("layoutcmp: JOIN BY BIND WORD — {} of {n_rows} rows matched an editor chart ({unmatched} unmatched, {ambiguous} ambiguous keys); {} editor charts without a row", out.len(), m.count as usize - used.len());
        out
    };
    println!("layoutcmp: {} charts compared; the editor has {}; {by_index_ok} of {n_rows} rows named the editor's object at the same chart index", rows.len(), m.count);
    let ed_rect = |k: usize| -> (i32, i32, i32, i32) { (m.pos[k].0 as i32, m.pos[k].1 as i32, m.size[k].0 as i32, m.size[k].1 as i32) };
    if let Some(w) = f("--entry") { let which: Vec<usize> = w.split(',').filter_map(|t| t.trim().parse().ok()).collect(); let mf = f("--item-base").map(|b| (tmmaps::map::MapFile::load(std::path::Path::new(ed_path.as_str())), b.parse::<u32>().unwrap())); print_entries(&rows, m, &which, mf.as_ref().map(|(m, b)| (m, *b))); return Ok(()); }
    if has("--walk") { print_walk(&rows, m, gutter, f("--from").map(|v| v.parse().unwrap()).unwrap_or(0), f("--count").map(|v| v.parse().unwrap()).unwrap_or(40), lay_path); return Ok(()); }
    if let Some(e) = f("--morton-study") { return morton_study(&rows, m, e.parse().map_err(|_| "--morton-study ENTRY")?, &f("--records").ok_or("--records RECORDS.tsv")?, gutter); }
    if let Some(g) = f("--group-order") { print_group_order(&rows, m, &g, gutter); return Ok(()); }
    if let Some(g) = f("--group") { let mf = f("--item-base").map(|b| (tmmaps::map::MapFile::load(std::path::Path::new(ed_path.as_str())), b.parse::<u32>().unwrap())); print_group(&rows, m, &g, gutter, mf.as_ref().map(|(m, b)| (m, *b))); return Ok(()); }
    if let Some(w) = f("--areas") { let which: Vec<usize> = w.split(',').filter_map(|t| t.trim().parse().ok()).collect(); print_areas(&rows, &which); return Ok(()); }
    if has("--diff") { print_diff(&rows, m, f("--show").map(|v| v.parse().unwrap()).unwrap_or(80)); return Ok(()); }
    if has("--global") { global_components(&rows, m, gutter, has("--verbose")); return Ok(()); }
    // (1) per class
    struct ClassStat { n: usize, same_rect: usize, same_size: usize, sizes: BTreeMap<((i32, i32), (i32, i32)), usize> }
    let mut classes: BTreeMap<String, ClassStat> = BTreeMap::new();
    let (mut same_rect, mut same_size) = (0usize, 0usize);
    for r in &rows {
        let e = ed_rect(r.ed);
        let sr = e == r.rect;
        let ss = (e.2, e.3) == (r.rect.2, r.rect.3);
        same_rect += sr as usize;
        same_size += ss as usize;
        let c = classes.entry(short_class(&r.class)).or_insert(ClassStat { n: 0, same_rect: 0, same_size: 0, sizes: BTreeMap::new() });
        c.n += 1;
        c.same_rect += sr as usize;
        c.same_size += ss as usize;
        *c.sizes.entry(((r.rect.2, r.rect.3), (e.2, e.3))).or_default() += 1;
    }
    println!("  {same_rect} same rects, {same_size} same sizes of {}", rows.len());
    let only: Option<String> = f("--class");
    println!("class\tcharts\tsame_rect\tsame_size\tours→editor sizes (count)");
    for (name, c) in &classes {
        if let Some(o) = &only { if !name.contains(o.as_str()) { continue; } }
        let mut sz: Vec<(&((i32, i32), (i32, i32)), &usize)> = c.sizes.iter().collect();
        sz.sort_by_key(|(_, &n)| std::cmp::Reverse(n));
        let s: Vec<String> = sz.iter().take(8).map(|(k, n)| format!("{}×{}→{}×{} ({n})", k.0 .0, k.0 .1, k.1 .0, k.1 .1)).collect();
        println!("{name}\t{}\t{}\t{}\t{}", c.n, c.same_rect, c.same_size, s.join(" "));
    }
    // (2) per our entry: the editor's components
    let mut by_entry: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        by_entry.entry(r.entry).or_default().push(i);
    }
    // histogram: (our count) → (editor component count) → entries
    let mut hist: BTreeMap<usize, BTreeMap<Vec<usize>, usize>> = BTreeMap::new();
    let mut n_split = 0usize;
    let mut dims_hist: BTreeMap<((u32, u32), (usize, usize)), usize> = BTreeMap::new();
    let mut entry_lines: Vec<String> = Vec::new();
    let mut tsv = String::from("entry\tclass\tcount\tour_nb\tour_na\tour_rect\ted_components\ted_dims\ted_rects\n");
    for (&ei, members) in &by_entry {
        let rects: Vec<(i32, i32, i32, i32)> = members.iter().map(|&i| ed_rect(rows[i].ed)).collect();
        let comps = components(&rects, gutter);
        let mut sizes: Vec<usize> = comps.iter().map(|c| c.len()).collect();
        sizes.sort_by(|a, b| b.cmp(a));
        *hist.entry(members.len()).or_default().entry(sizes.clone()).or_default() += 1;
        if comps.len() > 1 { n_split += 1; }
        let r0 = &rows[members[0]];
        let dims: Vec<(usize, usize, (i32, i32, i32, i32))> = comps.iter().map(|c| component_dims(&rects, c)).collect();
        if comps.len() == 1 {
            *dims_hist.entry(((r0.nb, r0.na), (dims[0].0, dims[0].1))).or_default() += 1;
        }
        let ed_rects: Vec<String> = dims.iter().map(|(_, _, r)| format!("({}, {}, {}, {})", r.0 - pad, r.1 - pad, r.2 + gutter, r.3 + gutter)).collect();
        let ed_dims: Vec<String> = dims.iter().map(|(nb, na, _)| format!("{nb}×{na}")).collect();
        tsv.push_str(&format!("{ei}\t{}\t{}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\n", r0.class, members.len(), r0.nb, r0.na, r0.entry_rect, sizes, ed_dims.join(" "), ed_rects.join(" ")));
        if has("--entries") && (members.len() > 1 || comps.len() > 1) {
            entry_lines.push(format!("  entry {ei} {} ×{} ours {}×{} at {:?} → editor {} component(s) {:?} dims [{}] rects [{}]", r0.class, members.len(), r0.nb, r0.na, r0.entry_rect, comps.len(), sizes, ed_dims.join(" "), ed_rects.join(" ")));
        }
    }
    println!("entries: {} ours; {} of them span several editor components", by_entry.len(), n_split);
    println!("our entry count → the editor's component sizes (entries):");
    for (cnt, h) in &hist {
        let mut v: Vec<(&Vec<usize>, &usize)> = h.iter().collect();
        v.sort_by_key(|(_, &n)| std::cmp::Reverse(n));
        let s: Vec<String> = v.iter().take(6).map(|(k, n)| format!("{k:?} ×{n}")).collect();
        println!("  {cnt}: {}", s.join("  "));
    }
    println!("grid dims ours (nb×na) → editor's (cols×rows) for the single-component entries:");
    let mut dv: Vec<(&((u32, u32), (usize, usize)), &usize)> = dims_hist.iter().collect();
    dv.sort_by_key(|(_, &n)| std::cmp::Reverse(n));
    for (k, n) in dv.iter().take(40) {
        println!("  {}×{} → {}×{}: {n}{}", k.0 .0, k.0 .1, k.1 .0, k.1 .1, if (k.0 .0 as usize, k.0 .1 as usize) != k.1 { "  ≠" } else { "" });
    }
    for l in entry_lines.iter().take(f("--show").map(|v| v.parse().unwrap()).unwrap_or(60)) {
        println!("{l}");
    }
    // (3) the editor's scale from the solo charts: w / ext.x and h / ext.y over the entries of one member
    let mut ratios: Vec<f32> = Vec::new();
    for (_, members) in &by_entry {
        if members.len() != 1 { continue; }
        let r = &rows[members[0]];
        let e = ed_rect(r.ed);
        if r.ext[0] > 8.0 { ratios.push((e.2 + gutter) as f32 / r.ext[0]); }
        if r.ext[1] > 8.0 { ratios.push((e.3 + gutter) as f32 / r.ext[1]); }
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !ratios.is_empty() {
        let med = ratios[ratios.len() / 2];
        println!("editor scale from {} solo sides (cell size / ext): median {med:.5}, p10 {:.5}, p90 {:.5}; ours from the table: {:.5}", ratios.len(), ratios[ratios.len() / 10], ratios[ratios.len() * 9 / 10], { let r = &rows[0]; let _ = r; f32::NAN });
    }
    if let Some(p) = f("--tsv") {
        std::fs::write(&p, tsv).map_err(|e| format!("{p}: {e}"))?;
        println!("entry table → {p}");
    }
    Ok(())
}

/// `--entry N[,N…]`: the members of our entries N with the editor's rect of each (a study print).
pub fn print_entries(rows: &[Row], m: &crate::format::Mapping, which: &[usize], map: Option<(&tmmaps::map::MapFile, u32)>) {
    for &ei in which {
        println!("entry {ei}:");
        let mut mem: Vec<&Row> = rows.iter().filter(|r| r.entry == ei).collect();
        mem.sort_by_key(|r| r.ord);
        for r in mem {
            let k = r.ed;
            let extra = match map { Some((mf, base)) if r.obj >= base && ((r.obj - base) as usize) < mf.items.len() => { let it = &mf.items[(r.obj - base) as usize]; format!("  item {} {} coll {:#x} author {:?} flags {:#06x} variant {} scale {} pos ({:.3}, {:.3}, {:.3}) pivot ({:.3}, {:.3}, {:.3}) cell {:?} yaw {:.4} pitch {:.4} roll {:.4} skin {}", it.index, it.model, it.collection_raw, it.author, it.flags, it.variant(), it.scale, it.pos[0], it.pos[1], it.pos[2], it.pivot[0], it.pivot[1], it.pivot[2], it.file_cell, it.yaw, it.pitch, it.roll, it.skin_region.is_some()) } _ => String::new() };
            println!("  chart {k} {} obj {} ord {} ours ({}, {}, {}, {}) ext ({}, {}) → editor ({}, {}, {}, {}){extra}", r.class, r.obj, r.ord, r.rect.0, r.rect.1, r.rect.2, r.rect.3, r.ext[0], r.ext[1], m.pos[k].0, m.pos[k].1, m.size[k].0, m.size[k].1);
        }
    }
}

/// `--global`: the editor's entries inferred over ALL charts of one (class, ext) key — gutter-adjacent rects of the same
/// key form a component; compared with our entries of that key (count histogram both sides).
pub fn global_components(rows: &[Row], m: &crate::format::Mapping, gutter: i32, verbose: bool) {
    let ed_rect = |k: usize| -> (i32, i32, i32, i32) { (m.pos[k].0 as i32, m.pos[k].1 as i32, m.size[k].0 as i32, m.size[k].1 as i32) };
    let mut by_key: BTreeMap<(String, u32, u32), Vec<usize>> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        by_key.entry((r.class.clone(), r.ext[0].to_bits(), r.ext[1].to_bits())).or_default().push(i);
    }
    let mut n_keys_differ = 0usize;
    let mut lines: Vec<String> = Vec::new();
    for (key, idx) in &by_key {
        let rects: Vec<(i32, i32, i32, i32)> = idx.iter().map(|&i| ed_rect(rows[i].ed)).collect();
        let comps = components(&rects, gutter);
        let mut ed_sizes: Vec<usize> = comps.iter().map(|c| c.len()).collect();
        ed_sizes.sort_by(|a, b| b.cmp(a));
        let mut our: BTreeMap<usize, usize> = BTreeMap::new();
        for &i in idx { *our.entry(rows[i].entry).or_default() += 1; }
        let mut our_sizes: Vec<usize> = our.values().cloned().collect();
        our_sizes.sort_by(|a, b| b.cmp(a));
        if ed_sizes != our_sizes {
            n_keys_differ += 1;
            // which of our entries are cut, which editor components merge several of ours
            let mut detail: Vec<String> = Vec::new();
            for c in &comps {
                let mut ents: BTreeMap<usize, usize> = BTreeMap::new();
                for &j in c { *ents.entry(rows[idx[j]].entry).or_default() += 1; }
                if ents.len() > 1 || ents.values().next().map(|&n| n != our[ents.keys().next().unwrap()]).unwrap_or(false) {
                    let (_, _, bb) = component_dims(&rects, c);
                    detail.push(format!("editor comp ×{} at ({}, {}, {}, {}) = ours {:?}", c.len(), bb.0, bb.1, bb.2, bb.3, ents));
                }
            }
            lines.push(format!("  {} ext ({}, {}) ×{}: editor {:?} vs ours {:?}{}", key.0, f32::from_bits(key.1), f32::from_bits(key.2), idx.len(), ed_sizes, our_sizes, if verbose { format!("\n      {}", detail.join("\n      ")) } else { String::new() }));
        }
    }
    println!("global: {} (class, ext) keys; {} keys whose editor component sizes differ from our entry sizes", by_key.len(), n_keys_differ);
    for l in &lines { println!("{l}"); }
}

/// `--diff`: every chart whose rect differs from the editor's, with its entry (a study print).
pub fn print_diff(rows: &[Row], m: &crate::format::Mapping, limit: usize) {
    let mut n = 0usize;
    for r in rows {
        let k = r.ed;
        let e = (m.pos[k].0 as i32, m.pos[k].1 as i32, m.size[k].0 as i32, m.size[k].1 as i32);
        if e != r.rect {
            n += 1;
            if n <= limit {
                println!("  chart {k} {} obj {} entry {} ({}×{} ord {}) at {:?}: ours ({}, {}, {}, {}) vs editor ({}, {}, {}, {}){}", r.class, r.obj, r.entry, r.nb, r.na, r.ord, r.entry_rect, r.rect.0, r.rect.1, r.rect.2, r.rect.3, e.0, e.1, e.2, e.3, if (e.2, e.3) != (r.rect.2, r.rect.3) { "  SIZE" } else { "" });
            }
        }
    }
    println!("{n} charts differ");
}

/// `--areas e,e,…`: the entry areas as the walk keys them — ((nb·na) as f32 · ey) · ex from the first member's ext —
/// with the f32 bits and the alternative products (a study of near-ties).
pub fn print_areas(rows: &[Row], which: &[usize]) {
    for &ei in which {
        let mut mem: Vec<&Row> = rows.iter().filter(|r| r.entry == ei).collect();
        if mem.is_empty() { println!("entry {ei}: no members"); continue; }
        mem.sort_by_key(|r| r.chart);
        let r0 = mem[0];
        let n = (r0.nb * r0.na) as f32;
        let a1 = (n * r0.ext[1]) * r0.ext[0];
        let a2 = (n * r0.ext[0]) * r0.ext[1];
        let a3 = n * (r0.ext[0] * r0.ext[1]);
        let a4 = ((r0.ext[1] * r0.ext[0]) as f64 * n as f64) as f32;
        println!("entry {ei} {} ×{} grid {}×{} ext ({}, {}) [{:#010x} {:#010x}]: area' ((n·ey)·ex) {} [{:#010x}]; ((n·ex)·ey) {} [{:#010x}]; n·(ex·ey) {} [{:#010x}]; f64 {} [{:#010x}]", r0.class, mem.len(), r0.nb, r0.na, r0.ext[0], r0.ext[1], r0.ext[0].to_bits(), r0.ext[1].to_bits(), a1, a1.to_bits(), a2, a2.to_bits(), a3, a3.to_bits(), a4, a4.to_bits());
    }
}

/// `--group NAME`: one model group's records — the editor's components (its chunks) with each member's editor cell and
/// z-order ordinal against OUR entry / ordinal — the Morton-order study.
pub fn print_group(rows: &[Row], m: &crate::format::Mapping, name: &str, gutter: i32, map: Option<(&tmmaps::map::MapFile, u32)>) {
    let idx: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].name == name).collect();
    if idx.is_empty() { println!("no rows named {name}"); return; }
    let ed_rect = |k: usize| -> (i32, i32, i32, i32) { (m.pos[k].0 as i32, m.pos[k].1 as i32, m.size[k].0 as i32, m.size[k].1 as i32) };
    let rects: Vec<(i32, i32, i32, i32)> = idx.iter().map(|&i| ed_rect(rows[i].ed)).collect();
    let comps = components(&rects, gutter);
    println!("group {name}: {} records; ours: {} entries; editor: {} components", idx.len(), { let mut s: std::collections::BTreeSet<usize> = Default::default(); for &i in &idx { s.insert(rows[i].entry); } s.len() }, comps.len());
    for (ci, c) in comps.iter().enumerate() {
        let (nb, na, bb) = component_dims(&rects, c);
        let mut xs: Vec<i32> = c.iter().map(|&j| rects[j].0).collect(); xs.sort(); xs.dedup();
        let mut ys: Vec<i32> = c.iter().map(|&j| rects[j].1).collect(); ys.sort(); ys.dedup();
        let cells = crate::itemrule::zorder_cells(nb as u32, na as u32);
        let mut mem: Vec<(u32, usize)> = c.iter().map(|&j| { let r = rects[j]; let cx = xs.iter().position(|&x| x == r.0).unwrap() as u32; let cy = ys.iter().position(|&y| y == r.1).unwrap() as u32; let o = cells.iter().position(|&cc| cc == (cx, cy)).map(|o| o as u32).unwrap_or(u32::MAX); (o, idx[j]) }).collect();
        mem.sort();
        let our_entries: std::collections::BTreeMap<usize, usize> = { let mut mm = std::collections::BTreeMap::new(); for &(_, i) in &mem { *mm.entry(rows[i].entry).or_insert(0) += 1; } mm };
        println!("  editor component {ci}: {} records, grid {nb}×{na} at ({}, {}, {}, {}); our entries {:?}", c.len(), bb.0, bb.1, bb.2, bb.3, our_entries);
        let line: Vec<String> = mem.iter().map(|&(o, i)| { let r = &rows[i]; let pos = map.and_then(|(mf, base)| if r.obj >= base && ((r.obj - base) as usize) < mf.items.len() { let it = &mf.items[(r.obj - base) as usize]; Some(format!(" @({:.0},{:.0},{:.0})", it.pos[0], it.pos[1], it.pos[2])) } else { None }).unwrap_or_default(); format!("{o}←c{}:e{}/o{}{pos}", r.chart, r.entry, r.ord) }).collect();
        println!("    editor ordinal ← chart:our entry/our ordinal: {}", line.join("  "));
    }
}

/// `--group-order NAME`: the group's records in RECORD ORDER with the editor's component index and cell column/row —
/// does a second hash entry partition the records by record order?
pub fn print_group_order(rows: &[Row], m: &crate::format::Mapping, name: &str, gutter: i32) {
    let idx: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].name == name).collect();
    let ed_rect = |k: usize| -> (i32, i32, i32, i32) { (m.pos[k].0 as i32, m.pos[k].1 as i32, m.size[k].0 as i32, m.size[k].1 as i32) };
    let rects: Vec<(i32, i32, i32, i32)> = idx.iter().map(|&i| ed_rect(rows[i].ed)).collect();
    let comps = components(&rects, gutter);
    let mut comp_of: Vec<usize> = vec![0; idx.len()];
    let mut colrow: Vec<(usize, usize)> = vec![(0, 0); idx.len()];
    for (ci, c) in comps.iter().enumerate() {
        let mut xs: Vec<i32> = c.iter().map(|&j| rects[j].0).collect(); xs.sort(); xs.dedup();
        let mut ys: Vec<i32> = c.iter().map(|&j| rects[j].1).collect(); ys.sort(); ys.dedup();
        for &j in c { comp_of[j] = ci; colrow[j] = (xs.iter().position(|&x| x == rects[j].0).unwrap(), ys.iter().position(|&y| y == rects[j].1).unwrap()); }
    }
    let mut order: Vec<usize> = (0..idx.len()).collect();
    order.sort_by_key(|&j| rows[idx[j]].chart);
    let mut line: Vec<String> = Vec::new();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &j in &order {
        let r = &rows[idx[j]];
        line.push(format!("c{}:E{}[{},{}]:e{}/o{}", r.chart, comp_of[j], colrow[j].0, colrow[j].1, r.entry, r.ord));
        if let Some(last) = runs.last_mut() { if last.0 == comp_of[j] { last.1 += 1; continue; } }
        runs.push((comp_of[j], 1));
    }
    println!("group {name}: {} records in record order (editor component E, cell [col,row], our entry/ordinal):", idx.len());
    println!("  {}", line.join(" "));
    println!("  runs of one editor component along the record order: {:?}", runs);
}

/// `--morton-study ENTRY --records RECORDS.tsv`: the members of our entry with the editor's cell ordinal (from its grid) against
/// our Morton ordinal under key variants — full-precision lroundf centres (the transcription), 12-bit and 10-bit wrapped
/// coordinates (a 36- / 30-bit key) — which variant reproduces the editor's order.
pub fn morton_study(rows: &[Row], m: &crate::format::Mapping, entry: usize, records: &str, gutter: i32) -> Result<(), String> {
    let recs = crate::classcmp::read_records_tsv(records)?;
    let mem: Vec<&Row> = rows.iter().filter(|r| r.entry == entry).collect();
    if mem.is_empty() { return Err(format!("entry {entry}: no members")); }
    let ed_rect = |k: usize| -> (i32, i32, i32, i32) { (m.pos[k].0 as i32, m.pos[k].1 as i32, m.size[k].0 as i32, m.size[k].1 as i32) };
    let rects: Vec<(i32, i32, i32, i32)> = mem.iter().map(|r| ed_rect(r.ed)).collect();
    let comps = components(&rects, gutter);
    if comps.len() != 1 { println!("entry {entry}: the editor has {} components — not one grid", comps.len()); }
    let c = &comps[0];
    let (nb, na, _) = component_dims(&rects, c);
    let mut xs: Vec<i32> = c.iter().map(|&j| rects[j].0).collect(); xs.sort(); xs.dedup();
    let mut ys: Vec<i32> = c.iter().map(|&j| rects[j].1).collect(); ys.sort(); ys.dedup();
    let cells = crate::itemrule::zorder_cells(nb as u32, na as u32);
    // the editor's ordinal per member (record order = our chart order)
    let mut order: Vec<usize> = (0..mem.len()).collect();
    order.sort_by_key(|&j| mem[j].chart);
    let ed_ord: Vec<u32> = order.iter().map(|&j| { let r = rects[j]; let cx = xs.iter().position(|&x| x == r.0).unwrap() as u32; let cy = ys.iter().position(|&y| y == r.1).unwrap() as u32; cells.iter().position(|&cc| cc == (cx, cy)).map(|o| o as u32).unwrap_or(u32::MAX) }).collect();
    let centres: Vec<[f32; 3]> = order.iter().map(|&j| { let r = recs.get(mem[j].chart).unwrap_or_else(|| panic!("records table has no chart {}", mem[j].chart)); [r.centre_x.unwrap_or(0.0), r.centre_y, r.centre_z.unwrap_or(0.0)] }).collect();
    let variant = |name: &str, key: &dyn Fn([f32; 3]) -> u128| {
        let mut perm: Vec<usize> = (0..centres.len()).collect();
        perm.sort_by_key(|&i| (key(centres[i]), i));
        let ours: Vec<u32> = perm.iter().map(|&p| p as u32).collect();
        let ok = ours.iter().zip(ed_ord.iter()).filter(|(a, b)| a == b).count();
        println!("  {name}: {ok} of {} ordinals match the editor's{}", ours.len(), if ok == ours.len() { "  ✓" } else { "" });
        if ok != ours.len() { println!("    ours   {:?}\n    editor {:?}", ours, ed_ord); }
    };
    let interleave = |x: u32, y: u32, z: u32, bits: u32| -> u128 { let mut k = 0u128; for b in 0..bits { k |= (((x >> b) & 1) as u128) << (3 * b); k |= (((y >> b) & 1) as u128) << (3 * b + 1); k |= (((z >> b) & 1) as u128) << (3 * b + 2); } k };
    println!("entry {entry}: {} members, editor grid {nb}×{na}; centres from {records}", mem.len());
    variant("lroundf, 30 bits", &|c: [f32; 3]| crate::itemrule::morton3(c));
    variant("round(), 30 bits", &|c: [f32; 3]| { let r = |v: f32| v.max(0.0).round() as u32; interleave(r(c[0]), r(c[1]), r(c[2]), 30) });
    variant("12-bit wrap", &|c: [f32; 3]| { let r = |v: f32| (v.max(0.0).round() as u32) & 0xfff; interleave(r(c[0]), r(c[1]), r(c[2]), 12) });
    variant("10-bit wrap", &|c: [f32; 3]| { let r = |v: f32| (v.max(0.0).round() as u32) & 0x3ff; interleave(r(c[0]), r(c[1]), r(c[2]), 10) });
    variant("12-bit saturate", &|c: [f32; 3]| { let r = |v: f32| (v.max(0.0).round() as u32).min(0xfff); interleave(r(c[0]), r(c[1]), r(c[2]), 12) });
    variant("y then x then z levels (z,x,y)", &|c: [f32; 3]| { let r = |v: f32| v.max(0.0).round() as u32; interleave(r(c[2]), r(c[0]), r(c[1]), 30) });
    for (i, &j) in order.iter().enumerate() { println!("    chart {} obj {} centre ({:.3}, {:.3}, {:.3}) editor ord {} our ord {}", mem[j].chart, mem[j].obj, centres[i][0], centres[i][1], centres[i][2], ed_ord[i], mem[j].ord); }
    Ok(())
}

/// `--walk`: the entries in OUR walk order (the `walk` column) with the editor's placed rect of each — the first position where
/// the two packers diverge.
pub fn print_walk(rows: &[Row], m: &crate::format::Mapping, gutter: i32, from: usize, count: usize, tsv_path: &str) {
    // the walk column is the 17th field; re-read it (the Row struct predates it)
    let s = std::fs::read_to_string(tsv_path).unwrap_or_default();
    let mut walk_of_entry: HashMap<usize, (usize, String)> = HashMap::new();
    for (ln, line) in s.lines().enumerate() {
        if ln == 0 { continue; }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() > 17 { if let (Ok(e), Ok(w)) = (f[9].parse::<usize>(), f[16].parse::<usize>()) { walk_of_entry.insert(e, (w, f[17].to_string())); } }
    }
    let ed_rect = |k: usize| -> (i32, i32, i32, i32) { (m.pos[k].0 as i32, m.pos[k].1 as i32, m.size[k].0 as i32, m.size[k].1 as i32) };
    let mut by_entry: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() { by_entry.entry(r.entry).or_default().push(i); }
    let mut lines: Vec<(usize, String)> = Vec::new();
    for (&ei, members) in &by_entry {
        let Some((w, area)) = walk_of_entry.get(&ei) else { continue };
        if *w < from || *w >= from + count { continue; }
        let rects: Vec<(i32, i32, i32, i32)> = members.iter().map(|&i| ed_rect(rows[i].ed)).collect();
        let comps = components(&rects, gutter);
        let (_, _, bb) = component_dims(&rects, &comps[0]);
        let r0 = &rows[members[0]];
        let ours = r0.entry_rect;
        let ed = (bb.0 - 1, bb.1 - 1, bb.2 + gutter, bb.3 + gutter);
        lines.push((*w, format!("  walk {w} entry {ei} {} ×{} {}×{} area {area} ext ({}, {}): ours {:?} editor ({}, {}, {}, {}){}", r0.name, members.len(), r0.nb, r0.na, r0.ext[0], r0.ext[1], ours, ed.0, ed.1, ed.2, ed.3, if (ours.0, ours.1, ours.2, ours.3) == ed { "" } else if (ours.2, ours.3) == (ed.2, ed.3) { "  MOVED" } else { "  SIZE" })));
    }
    lines.sort();
    for (_, l) in lines { println!("{l}"); }
}
