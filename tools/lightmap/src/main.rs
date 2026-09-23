use lightmap::{find_chunk, walk::walk_chunk, probe};

fn cache_chunks(cache: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let mut out = Vec::new();
    let mut c = lightmap::Cur::new(cache);
    while c.left() >= 4 {
        let id = c.u32().unwrap();
        if id == 0xFACA_DE01 {
            out.push((id, cache[c.o..].to_vec()));
            break;
        }
        let _magic = c.u32().unwrap();
        let size = c.u32().unwrap() as usize;
        out.push((id, c.take(size).unwrap().to_vec()));
    }
    out
}

/// Does the map's DayTime quarter carry a directional (sun/horizon) term in the editor's bakes?
/// Measured: Day no, Sunset yes (low, pink, ESE); Sunrise assumed yes (mirrored), Night no.
fn t_quarter(dt: Option<u32>, x: &lightmap::moods::MoodXml) -> bool {
    let t = match dt { Some(v) if v != 0xffff_ffff => v as f32 / 65536.0, _ => lightmap::moods::default_daytime(&x.collection, x.mood) as f32 / 65536.0 };
    (0.25..0.5).contains(&t) || t >= 0.75
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: lmtool walk MAP.Gbx... | lmtool dump MAP.Gbx OUTDIR | lmtool probe MAP.Gbx [OUTDIR]");
        std::process::exit(2);
    }
    match a[0].as_str() {
        "walk" => {
            for f in &a[1..] {
                let data = std::fs::read(f).expect("read");
                let g = gbx::Gbx::parse(&data);
                let Some((_, payload, size)) = find_chunk(&g.body) else {
                    println!("{f}: no lightmap chunk");
                    continue;
                };
                println!("== {f}: chunk {size} B");
                match walk_chunk(&g.body[payload..payload + size]) {
                    Ok(w) => print!("{}", w.log),
                    Err(e) => println!("ERR {e}"),
                }
            }
        }
        "dump" => {
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let dir = &a[2];
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(format!("{dir}/chunk.bin"), &g.body[payload..payload + size]).expect("write");
            let w = walk_chunk(&g.body[payload..payload + size]).expect("walk");
            std::fs::write(format!("{dir}/cache.bin"), &w.cache).unwrap();
            for (i, b) in w.blobs.iter().enumerate() {
                if !b.is_empty() {
                    std::fs::write(format!("{dir}/blob{i}.webp"), b).unwrap();
                }
            }
            for (id, payload) in cache_chunks(&w.cache) {
                std::fs::write(format!("{dir}/c{id:08x}.bin"), &payload).unwrap();
            }
            println!("wrote {dir}: chunk {} B, cache {} B, {} blobs", size, w.cache.len(), w.blobs.len());
        }
        "probe" => {
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let w = walk_chunk(&g.body[payload..payload + size]).expect("walk");
            let chunks = cache_chunks(&w.cache);
            let (_, m) = chunks.iter().find(|c| c.0 == 0x0602_201A).expect("0x1A");
            print!("{}", probe::probe_mapping(m));
            if let Some(dir) = a.get(2) {
                std::fs::create_dir_all(dir).unwrap();
                for (i, b) in probe::scan_zlib_blocks(m).iter().enumerate() {
                    std::fs::write(format!("{dir}/z{i}.bin"), &b.data).unwrap();
                }
            }
        }
        "words" => {
            let data = std::fs::read(&a[1]).expect("read");
            let off = usize::from_str_radix(a[2].trim_start_matches("0x"), 16).unwrap();
            let n: usize = a[3].parse().unwrap();
            print!("{}", probe::words(&data[off..], n));
        }
        "roundtrip" => {
            let mut bad = 0;
            for f in &a[1..] {
                let data = std::fs::read(f).expect("read");
                let g = gbx::Gbx::parse(&data);
                let Some((_, payload, size)) = find_chunk(&g.body) else {
                    println!("{f}: no lightmap chunk");
                    continue;
                };
                let p = &g.body[payload..payload + size];
                match lightmap::format::LightmapChunk::parse(p) {
                    Ok(lm) => {
                        let w = lm.write(false);
                        let ok = w == p;
                        // also: rebuild the cache from parsed parts and compare inflated bytes
                        let ok2 = lm.data.as_ref().map(|d| {
                            let raw = lightmap::zlib_inflate(&d.cache_compressed, d.cache_uncompressed_len as usize).unwrap();
                            d.cache.write() == raw
                        }).unwrap_or(true);
                        let m = lm.data.as_ref().and_then(|d| d.cache.mapping().map(|m| format!("N={} atlas {}x{} bbox {:?}..{:?} u01={} u02={} u03={} tail={:?}", m.count, m.atlas_w, m.atlas_h, m.bbox_min, m.bbox_max, m.m_u01, m.m_u02, m.m_u03, m.tail))).unwrap_or_default();
                        println!("{}  {} bytes  stored-stream={}  rebuilt-cache={}  {m}", std::path::Path::new(f).file_name().unwrap().to_string_lossy(), size, if ok {"OK"} else {"MISMATCH"}, if ok2 {"OK"} else {"MISMATCH"});
                        if !ok || !ok2 { bad += 1; }
                    }
                    Err(e) => { println!("{f}: PARSE ERROR {e}"); bad += 1; }
                }
            }
            if bad > 0 { std::process::exit(1); }
        }
        "binds" => {
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let from: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let n: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(40);
            println!("chart\tobj\tsub\tflags\tx\ty\tw\th\tf32\tfb0\tfb1\tfb2");
            for i in from..(from + n).min(m.count as usize) {
                let b = m.binds[i];
                println!("{i}\t{}\t{}\t{:#x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", b.obj_group_idx / 4, b.obj_idx & 0xffffff, b.obj_idx >> 24, m.pos[i].0, m.pos[i].1, m.size[i].0, m.size[i].1, m.chart_f32[i], m.frame_bytes[0][i], m.frame_bytes[1][i], m.frame_bytes[2][i]);
            }
        }
        "daytime" => {
            // lmtool daytime MAP… — the map's time of day (skippable chunk 0x03043056: version, u01,
            // DAYTIME 0..65535 = the fraction of the day, u02, dynamic-daylight, day duration ms)
            // against the time the game baked the lightmap at (the cache mapping head's word at
            // 0x48, `lmtool head`). The editor keeps its time of day across the maps opened in
            // one session (baker child, 2026-09-23): a bake made right after another map can
            // carry that map's time — the two words must agree before a file ships. Exit 1 on
            // any mismatch.
            let mut bad = 0usize;
            for f in a[1..].iter().filter(|x| !x.starts_with("--")) {
                let data = std::fs::read(f).expect("read");
                let g = gbx::Gbx::parse(&data);
                let own: Option<u32> = tmmaps::gbx::all_skip_chunks(&g.body).into_iter().find(|c| c.0 == 0x0304_3056 && c.3 >= 12).map(|(_, _, p, _)| u32::from_le_bytes(g.body[p + 8..p + 12].try_into().unwrap()));
                let lm: Option<u32> = lightmap::mapio::load(f).ok().and_then(|m| m.chunk.data.as_ref().and_then(|d| d.cache.mapping()).and_then(|mp| (mp.head.len() >= 72).then(|| u32::from_le_bytes(mp.head[68..72].try_into().unwrap()))));
                // 0xFFFFFFFF = no custom time: the mood default — measured on the giant Summer bakes
                // (2026-09-23; every -1 map got it whatever map preceded it in the game session):
                // Day 39769 (0.607) on BlueBay/RedIsland/GreenCoast/WhiteShore, Stadium Day 33041
                // (0.504), Sunrise 20043 (0.306) on RedIsland
                let hdr = tmmaps::header::read(f).ok();
                let default: Option<u32> = hdr.as_ref().and_then(|h| match (h.envir.as_str(), h.mood.as_str()) { ("Stadium", "Day") => Some(33041), (_, "Day") => Some(39769), ("RedIsland", "Sunrise") => Some(20043), _ => None });
                let own_eff = match own { Some(0xFFFF_FFFF) => default, o => o };
                let verdict = match (own_eff, lm) {
                    (Some(o), Some(l)) if o == l => if own == Some(0xFFFF_FFFF) { "OK (mood default)" } else { "OK" },
                    (None, Some(_)) if own == Some(0xFFFF_FFFF) => "CHECK: no custom time and no known default for this mood",
                    (Some(_), Some(_)) => { bad += 1; "MISMATCH" }
                    (None, _) => "no daytime chunk",
                    (_, None) => "no baked lightmap",
                };
                println!("{f}\tmap {}\tlightmap {}\t{verdict}", own.map(|v| format!("{v} ({:.3})", v as f32 / 65535.0)).unwrap_or_else(|| "-".into()), lm.map(|v| format!("{v} ({:.3})", v as f32 / 65535.0)).unwrap_or_else(|| "-".into()));
            }
            if bad > 0 { eprintln!("lmtool daytime: {bad} lightmaps baked at another time than the map's"); std::process::exit(1); }
        }
        "itembase" => {
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            // lmtool itembase LIT.Map.Gbx [--min-items 3]: the object-index base of the ITEMS in a
            // game-baked lightmap, measured — for every candidate base b (the items are M
            // consecutive objects somewhere in the object space) the items of one MODEL must
            // get the same chart signature (the editor sizes a chart from the model's uv1
            // bounds; only a ±2 px padding varies), so the base is the b with the fewest
            // distinct signatures per model. Prints the best five. 2026-09-22, the giant
            // Summer 05 x2 editor bake: base 28312 = 16384 + 604 authored tiles + 96² zone
            // slots + 2108 generated pieces, the items the LAST 2906 objects.
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let mm = d.cache.mapping().expect("mapping");
            let maxo = mm.binds.iter().map(|b| b.obj_group_idx / 4).max().unwrap_or(0) as usize;
            // per object: the sorted list of its chart sizes, as one string
            let mut sigs: Vec<Vec<(u16, u16)>> = vec![Vec::new(); maxo + 1];
            for i in 0..mm.count as usize {
                sigs[(mm.binds[i].obj_group_idx / 4) as usize].push((mm.size[i].0 as u16, mm.size[i].1 as u16));
            }
            let sig: Vec<String> = sigs.iter_mut().map(|s| { s.sort(); s.iter().map(|(w, h)| format!("{w}x{h}")).collect::<Vec<_>>().join(",") }).collect();
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let min_items: usize = f("--min-items").map(|s| s.parse().unwrap()).unwrap_or(3);
            // models with enough placements
            let mut by_model: std::collections::HashMap<&str, Vec<usize>> = Default::default();
            for (i, it) in m.items.iter().enumerate() { by_model.entry(it.model.as_str()).or_default().push(i); }
            let groups: Vec<&Vec<usize>> = by_model.values().filter(|v| v.len() >= min_items).collect();
            let n_items = m.items.len();
            if n_items == 0 || maxo + 1 < n_items { println!("{} items, object space {} — nothing to measure", n_items, maxo + 1); return; }
            let mut scored: Vec<(f64, usize, usize)> = Vec::new(); // (signatures per model, perfect models, base)
            for b in 0..=(maxo + 1 - n_items) {
                let mut total = 0usize;
                let mut perfect = 0usize;
                // every item object must carry a chart: an empty stretch of the object space is no base
                // (a few items have no lightmappable mesh — flags, lights — and no chart)
                let last = b + n_items == maxo + 1;
                if !last && (b..b + n_items).filter(|o| sig[*o].is_empty()).count() * 2 > n_items { continue; }
                // … and the items are DIVERSE: a stretch of identical zone-tile charts (every Sea tile
                // has the same chart) would score a perfect 1.00 for any model split — require at
                // least one distinct signature per three models
                {
                    let mut distinct: Vec<&str> = Vec::new();
                    for o in b..b + n_items { let s = sig[o].as_str(); if !distinct.contains(&s) { distinct.push(s); if distinct.len() * 3 >= groups.len() { break; } } }
                    if !last && distinct.len() * 3 < groups.len() { continue; }
                }
                for grp in &groups {
                    let mut seen: Vec<&str> = Vec::new();
                    for &i in grp.iter() {
                        let s = sig[b + i].as_str();
                        if s.is_empty() { continue; } // a chartless item says nothing
                        if !seen.contains(&s) { seen.push(s); }
                    }
                    total += seen.len().max(1);
                    if seen.len() <= 1 { perfect += 1; }
                }
                scored.push((total as f64 / groups.len().max(1) as f64, perfect, b));
            }
            scored.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap().then(y.1.cmp(&x.1)));
            // the items-last candidate wins whenever it is within 10 % of the best (every game bake
            // measured so far put the items last; a run of chartless vegetation weakens its score)
            if let Some(pos) = scored.iter().position(|s| s.2 + n_items == maxo + 1) {
                if pos > 0 && scored[pos].0 <= scored[0].0 * 1.10 { let it = scored.remove(pos); scored.insert(0, it); }
            }
            println!("{} items, {} models with >= {min_items} placements, object space {} (max object {maxo}); authored blocks {}, baked records {}, size words {:?}", n_items, groups.len(), maxo + 1, m.blocks.len(), m.baked.len(), m.size);
            for (score, perfect, b) in scored.iter().take(5) {
                println!("  base {b}: {score:.2} chart signatures per model, {perfect} models with one signature{}", if *b + n_items == maxo + 1 { "  (the items are the LAST objects)" } else { "" });
            }
            let best = scored[0].2;
            println!("itembase {best}  (object space {} = base + {n_items} items{})", maxo + 1, if best + n_items == maxo + 1 { "" } else { ", objects after the items too" });
        }
        "objstats" => {
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let maxo = m.binds.iter().map(|b| b.obj_group_idx / 4).max().unwrap_or(0) as usize;
            let mut per = vec![0u32; maxo + 1];
            let mut nonmult = 0;
            let mut sorted = true;
            for (i, b) in m.binds.iter().enumerate() {
                if b.obj_group_idx % 4 != 0 { nonmult += 1; }
                per[(b.obj_group_idx / 4) as usize] += 1;
                if i > 0 && b.obj_group_idx < m.binds[i - 1].obj_group_idx { sorted = false; }
            }
            let used = per.iter().filter(|&&c| c > 0).count();
            println!("charts {} objects-space {} used {} non-multiple-of-4 {} sorted-by-object {}", m.count, maxo + 1, used, nonmult, sorted);
            // gaps
            let mut gaps = Vec::new();
            let mut i = 0;
            while i <= maxo {
                if per[i] == 0 {
                    let s = i;
                    while i <= maxo && per[i] == 0 { i += 1; }
                    gaps.push((s, i - 1));
                } else { i += 1; }
            }
            println!("{} gaps; first 30: {:?}", gaps.len(), &gaps[..gaps.len().min(30)]);
            let mut hist = std::collections::BTreeMap::new();
            for &c in &per { if c > 0 { *hist.entry(c).or_insert(0) += 1; } }
            println!("charts-per-object histogram: {:?}", hist);
            let flags: std::collections::BTreeMap<u32, usize> = m.binds.iter().fold(Default::default(), |mut h, b| { *h.entry(b.obj_idx >> 24).or_insert(0) += 1; h });
            println!("flag byte histogram: {:?}", flags);
            let f32s: std::collections::BTreeMap<u32, usize> = m.chart_f32.iter().fold(Default::default(), |mut h, v| { *h.entry(v.to_bits()).or_insert(0) += 1; h });
            println!("chart f32 distinct: {}", f32s.len());
        }
        "runs" => {
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            // runs of equal (w,h) over consecutive charts, printed with object index range
            let mut i = 0;
            let n = m.count as usize;
            let maxruns: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(200);
            let mut printed = 0;
            while i < n && printed < maxruns {
                let s = i;
                while i < n && m.size[i] == m.size[s] { i += 1; }
                println!("charts {s}..{}  obj {}..{}  size {}x{}  ({} charts)", i - 1, m.binds[s].obj_group_idx / 4, m.binds[i - 1].obj_group_idx / 4, m.size[s].0, m.size[s].1, i - s);
                printed += 1;
            }
        }
        "align" => {
            // lmtool align MAP census.tsv : find the object-index offset K at which the
            // file's items line up with the charts (same model => same chart size)
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let census = std::fs::read_to_string(&a[2]).expect("census");
            let items: Vec<String> = census.lines().skip(1).filter(|l| l.starts_with("I\t")).map(|l| l.split('\t').nth(2).unwrap().to_string()).collect();
            let maxo = m.binds.iter().map(|b| b.obj_group_idx / 4).max().unwrap() as usize;
            // object -> first chart size
            let mut osize: Vec<Option<(u16, u16)>> = vec![None; maxo + 1];
            for (i, b) in m.binds.iter().enumerate() {
                let o = (b.obj_group_idx / 4) as usize;
                if osize[o].is_none() { osize[o] = Some(m.size[i]); }
            }
            println!("{} items, object space {}", items.len(), maxo + 1);
            let mut best = Vec::new();
            for k in 0..=(maxo + 1).saturating_sub(items.len()) {
                let mut per: std::collections::HashMap<&str, std::collections::HashMap<(u16, u16), usize>> = Default::default();
                let mut missing = 0;
                for (i, name) in items.iter().enumerate() {
                    match osize[k + i] {
                        Some(s) => *per.entry(name.as_str()).or_default().entry(s).or_insert(0) += 1,
                        None => missing += 1,
                    }
                }
                // consistency: fraction of items whose size is the majority size of their model
                let mut agree = 0usize;
                for (_, h) in &per { agree += h.values().max().copied().unwrap_or(0); }
                best.push((agree, k, missing));
            }
            best.sort_by(|a, b| b.0.cmp(&a.0));
            for (agree, k, missing) in best.iter().take(8) {
                println!("K={k}: {agree} of {} items agree with their model's majority size; {missing} items without a chart", items.len());
            }
        }
        "models" => {
            // lmtool models MAP census.tsv K : per-model chart-size distribution with items at object K+i
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let census = std::fs::read_to_string(&a[2]).expect("census");
            let k: usize = a[3].parse().unwrap();
            let items: Vec<String> = census.lines().skip(1).filter(|l| l.starts_with("I\t")).map(|l| l.split('\t').nth(2).unwrap().to_string()).collect();
            let maxo = m.binds.iter().map(|b| b.obj_group_idx / 4).max().unwrap() as usize;
            let mut ocharts: Vec<Vec<usize>> = vec![vec![]; maxo + 1];
            for (i, b) in m.binds.iter().enumerate() { ocharts[(b.obj_group_idx / 4) as usize].push(i); }
            let mut per: std::collections::BTreeMap<&str, std::collections::BTreeMap<String, Vec<usize>>> = Default::default();
            for (i, name) in items.iter().enumerate() {
                let o = k + i;
                let key = if o > maxo || ocharts[o].is_empty() { "none".to_string() } else {
                    ocharts[o].iter().map(|&c| format!("{}x{}", m.size[c].0, m.size[c].1)).collect::<Vec<_>>().join("+")
                };
                per.entry(name.as_str()).or_default().entry(key).or_default().push(i);
            }
            let mut rows: Vec<_> = per.iter().collect();
            rows.sort_by_key(|(_, h)| std::cmp::Reverse(h.values().map(|v| v.len()).sum::<usize>()));
            let limit: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(30);
            for (name, h) in rows.iter().take(limit) {
                let total: usize = h.values().map(|v| v.len()).sum();
                let mut parts: Vec<String> = h.iter().map(|(s, v)| format!("{s}:{}{}", v.len(), if v.len() <= 3 { format!("(i{})", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")) } else { String::new() })).collect();
                parts.sort();
                println!("{name}\t{total}\t{}", parts.join(" "));
            }
        }
        "charts" => {
            // lmtool charts MAP FROM N : per chart, mean colour in frame-0 image A and grey in image B
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("A");
            let ib = lightmap::img::decode_webp(&d.frames[0].images[1]).expect("B");
            let i1 = lightmap::img::decode_webp(&d.frames[1].images[0]).expect("F1");
            let from: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let n: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(40);
            println!("images A {}x{} B {}x{} F1 {}x{}", ia.w, ia.h, ib.w, ib.h, i1.w, i1.h);
            println!("chart\tobj\tx\ty\tw\th\tfb0\tfb1\tfb2\tA(rgb)\tB(rgb)\tF1(rgb)");
            for i in from..(from + n).min(m.count as usize) {
                let (x, y) = m.pos[i];
                let (w, h) = m.size[i];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
                let ma = ia.mean(px, py, pw, ph);
                let mb = ib.mean(px, py, pw, ph);
                let m1 = i1.mean(px, py, pw, ph);
                println!("{i}\t{}\t{x}\t{y}\t{w}\t{h}\t{}\t{}\t{}\t{:.0} {:.0} {:.0}\t{:.0} {:.0} {:.0}\t{:.0} {:.0} {:.0}", m.binds[i].obj_group_idx / 4, m.frame_bytes[0][i], m.frame_bytes[1][i], m.frame_bytes[2][i], ma[0], ma[1], ma[2], mb[0], mb[1], mb[2], m1[0], m1[1], m1[2]);
            }
        }
        "crop" => {
            // lmtool crop MAP CHART OUTBASE : write A/B/F1 crops of one chart (x8 nearest) as PPM
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let ci: usize = a[2].parse().unwrap();
            let (x, y) = m.pos[ci];
            let (w, h) = m.size[ci];
            let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
            println!("chart {ci}: obj {} pos {x},{y} size {w}x{h} -> pixels {px},{py} {pw}x{ph}; fb {} {} {}", m.binds[ci].obj_group_idx / 4, m.frame_bytes[0][ci], m.frame_bytes[1][ci], m.frame_bytes[2][ci]);
            let sc = 8u32;
            for (name, blob) in [("A", &d.frames[0].images[0]), ("B", &d.frames[0].images[1]), ("F1", &d.frames[1].images[0]), ("F2", &d.frames[2].images[0])] {
                let im = lightmap::img::decode_webp(blob).expect("decode");
                let mut out = lightmap::img::Rgb::new((pw + 2) * sc, (ph + 2) * sc);
                for oy in 0..out.h { for ox in 0..out.w {
                    let sx = (px as i64 - 1 + (ox / sc) as i64).clamp(0, im.w as i64 - 1) as u32;
                    let sy = (py as i64 - 1 + (oy / sc) as i64).clamp(0, im.h as i64 - 1) as u32;
                    out.set(ox, oy, im.get(sx, sy));
                }}
                lightmap::img::write_ppm(&out, &format!("{}_{name}.ppm", a[3])).unwrap();
            }
        }
        "biggest" => {
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let mut idx: Vec<usize> = (0..m.count as usize).collect();
            idx.sort_by_key(|&i| std::cmp::Reverse(m.size[i].0 as u32 * m.size[i].1 as u32));
            for &i in idx.iter().take(a.get(2).and_then(|s| s.parse().ok()).unwrap_or(10)) {
                println!("chart {i} obj {} sub {} size {}x{} at {},{} fb {} {} {}", m.binds[i].obj_group_idx / 4, m.binds[i].obj_idx & 0xffffff, m.size[i].0, m.size[i].1, m.pos[i].0, m.pos[i].1, m.frame_bytes[0][i], m.frame_bytes[1][i], m.frame_bytes[2][i]);
            }
        }
        "norm" => {
            // per-chart max of A and B channels: is each chart normalised to 255?
            let data = std::fs::read(&a[1]).expect("read");
            let g = gbx::Gbx::parse(&data);
            let (_, payload, size) = find_chunk(&g.body).expect("chunk");
            let lm = lightmap::format::LightmapChunk::parse(&g.body[payload..payload + size]).expect("parse");
            let d = lm.data.as_ref().expect("has lightmaps");
            let m = d.cache.mapping().expect("mapping");
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("A");
            let ib = lightmap::img::decode_webp(&d.frames[0].images[1]).expect("B");
            let mut ha = [0usize; 16]; let mut hb = [0usize; 16]; let mut hbmin = [0usize; 16];
            let mut fb_vs_max: std::collections::BTreeMap<u8, (f64, usize)> = Default::default();
            for i in 0..m.count as usize {
                let (x, y) = m.pos[i]; let (w, h) = m.size[i];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
                let mut ma = 0u8; let mut mb = 0u8; let mut mbmin = 255u8;
                for yy in py..(py + ph).min(ia.h) { for xx in px..(px + pw).min(ia.w) {
                    let c = ia.get(xx, yy); ma = ma.max(c[0]).max(c[1]).max(c[2]);
                    let b = ib.get(xx, yy)[0]; mb = mb.max(b); mbmin = mbmin.min(b);
                }}
                ha[(ma / 16) as usize] += 1; hb[(mb / 16) as usize] += 1; hbmin[(mbmin / 16) as usize] += 1;
                let e = fb_vs_max.entry(m.frame_bytes[0][i]).or_insert((0.0, 0)); e.0 += ma as f64; e.1 += 1;
            }
            println!("per-chart max(A) histogram /16: {:?}", ha);
            println!("per-chart max(B) histogram /16: {:?}", hb);
            println!("per-chart min(B) histogram /16: {:?}", hbmin);
            println!("fb0 -> mean of per-chart max(A):");
            for (k, (s, n)) in &fb_vs_max { if *n >= 20 { println!("  fb0 {k}: {:.1} (n={n})", s / *n as f64); } }
            // global B histogram
            let mut hg = [0usize; 32];
            for i in 0..(ib.w * ib.h) as usize { hg[(ib.px[i * 3] / 8) as usize] += 1; }
            println!("global B histogram /8: {:?}", hg);
        }
        "paint" => {
            // lmtool paint TEMPLATE.Map.Gbx OUT.Map.Gbx --base 4096 [--mode rgb3|grey|fbramp] [--b V] [--keep-b]
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let mut m = lightmap::mapio::load(&a[1]).expect("load");
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let mode = f("--mode").unwrap_or_else(|| "rgb3".into());
            let bval: Option<u8> = f("--b").map(|s| s.parse().unwrap());
            let d = m.chunk.data.as_mut().expect("has lightmaps");
            let mut ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("A");
            let mut ib = lightmap::img::decode_webp(&d.frames[0].images[1]).expect("B");
            let mp = d.cache.mapping_mut().expect("mapping");
            let mut painted = 0;
            for i in 0..mp.count as usize {
                let obj = mp.binds[i].obj_group_idx / 4;
                if obj < base { continue; }
                let item = obj - base;
                let (x, y) = mp.pos[i]; let (w, h) = mp.size[i];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
                let col: [u8; 3] = match mode.as_str() {
                    "rgb3" => [[255, 40, 40], [40, 255, 40], [40, 40, 255]][(item % 3) as usize],
                    "grey" => [200, 200, 200],
                    // brightness ramp by item index: 8 steps
                    "ramp" => { let v = (32 + (item % 8) * 31) as u8; [v, v, v] }
                    _ => panic!("mode"),
                };
                for yy in py..(py + ph).min(ia.h) { for xx in px..(px + pw).min(ia.w) {
                    ia.set(xx, yy, col);
                    if let Some(b) = bval { ib.set(xx, yy, [b, b, b]); }
                }}
                if mode == "fbramp" { mp.frame_bytes[0][i] = (32 + (item % 8) * 31) as u8; }
                painted += 1;
            }
            if mode == "fbramp" { mp.mark_edited(); }
            d.frames[0].images[0] = lightmap::img::encode_webp_lossless(&ia).expect("enc A");
            if bval.is_some() { d.frames[0].images[1] = lightmap::img::encode_webp_lossless(&ib).expect("enc B"); }
            let sizes = (d.frames[0].images[0].len(), d.frames[0].images[1].len());
            let recompress = mode == "fbramp";
            let payload = m.chunk.write(recompress);
            lightmap::mapio::save_with_chunk(&m, &payload, &a[2]).expect("save");
            println!("painted {painted} item charts; chunk {} B (A {} B, B {} B); wrote {}", payload.len(), sizes.0, sizes.1, a[2]);
        }
        "grad" => {
            // lmtool grad TEMPLATE.Map.Gbx census.tsv OUT.Map.Gbx [--base 4096] [--a V] [--b x|z|none] [--fb x|z|none]
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let mut m = lightmap::mapio::load(&a[1]).expect("load");
            let census = std::fs::read_to_string(&a[2]).expect("census");
            let items: Vec<(f32, f32)> = census.lines().skip(1).filter(|l| l.starts_with("I\t")).map(|l| { let c: Vec<&str> = l.split('\t').collect(); (c[8].parse().unwrap(), c[10].parse().unwrap()) }).collect();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let aval: u8 = f("--a").map(|s| s.parse().unwrap()).unwrap_or(200);
            let bmode = f("--b").unwrap_or_else(|| "x".into());
            let fbmode = f("--fb").unwrap_or_else(|| "z".into());
            let (xmin, xmax) = items.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.0), hi.max(p.0)));
            let (zmin, zmax) = items.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
            println!("items x {xmin}..{xmax} z {zmin}..{zmax}");
            let d = m.chunk.data.as_mut().expect("has lightmaps");
            let mut ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("A");
            let mut ib = lightmap::img::decode_webp(&d.frames[0].images[1]).expect("B");
            let mp = d.cache.mapping_mut().expect("mapping");
            let ramp = |mode: &str, p: (f32, f32)| -> Option<u8> { match mode { "x" => Some((255.0 * (p.0 - xmin) / (xmax - xmin)).round() as u8), "z" => Some((255.0 * (p.1 - zmin) / (zmax - zmin)).round() as u8), _ => None } };
            for i in 0..mp.count as usize {
                let obj = mp.binds[i].obj_group_idx / 4;
                if obj < base { continue; }
                let Some(&p) = items.get((obj - base) as usize) else { continue };
                let (x, y) = mp.pos[i]; let (w, h) = mp.size[i];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
                let bv = ramp(&bmode, p);
                for yy in py..(py + ph).min(ia.h) { for xx in px..(px + pw).min(ia.w) {
                    ia.set(xx, yy, [aval, aval, aval]);
                    if let Some(b) = bv { ib.set(xx, yy, [b, b, b]); }
                }}
                if let Some(v) = ramp(&fbmode, p) { mp.frame_bytes[0][i] = v; }
            }
            mp.mark_edited();
            d.frames[0].images[0] = lightmap::img::encode_webp_lossless(&ia).expect("enc A");
            if bmode != "none" { d.frames[0].images[1] = lightmap::img::encode_webp_lossless(&ib).expect("enc B"); }
            let payload = m.chunk.write(true);
            lightmap::mapio::save_with_chunk(&m, &payload, &a[3]).expect("save");
            println!("chunk {} B; wrote {}", payload.len(), a[3]);
        }
        "head" => {
            // lmtool head MAP... : the mapping head words side by side
            let mut heads = Vec::new();
            for f in &a[1..] {
                let m = lightmap::mapio::load(f).expect("load");
                let d = m.chunk.data.as_ref().unwrap();
                heads.push(d.cache.mapping().unwrap().head.clone());
            }
            let n = heads[0].len() / 4;
            for w in 0..n {
                let mut line = format!("{:04x}:", 4 + w * 4);
                for h in &heads {
                    let v = u32::from_le_bytes(h[w * 4..w * 4 + 4].try_into().unwrap());
                    let fl = f32::from_bits(v);
                    let s = if v < 0x10000 { format!("{v}") } else if fl.is_finite() && fl.abs() > 1e-3 && fl.abs() < 1e6 { format!("{fl:.4}") } else { format!("{v:#x}") };
                    line.push_str(&format!(" {s:>14}"));
                }
                println!("{line}");
            }
        }
        "classes" => {
            // lmtool classes TEMPLATE.Map.Gbx OUT.Map.Gbx [--base 4096]: 4 item classes by index%4
            //   0: B=0   fb0=64  A reddish   1: B=255 fb0=64  A greenish
            //   2: B=0   fb0=224 A bluish    3: B=255 fb0=224 A yellowish
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let mut m = lightmap::mapio::load(&a[1]).expect("load");
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let d = m.chunk.data.as_mut().expect("has lightmaps");
            let mut ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("A");
            let mut ib = lightmap::img::decode_webp(&d.frames[0].images[1]).expect("B");
            let mp = d.cache.mapping_mut().expect("mapping");
            let hues: [[u8; 3]; 4] = [[230, 170, 170], [170, 230, 170], [170, 170, 230], [230, 230, 170]];
            for i in 0..mp.count as usize {
                let obj = mp.binds[i].obj_group_idx / 4;
                if obj < base { continue; }
                let c = ((obj - base) % 4) as usize;
                let (x, y) = mp.pos[i]; let (w, h) = mp.size[i];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
                let b = if c % 2 == 1 { 255 } else { 0 };
                for yy in py..(py + ph).min(ia.h) { for xx in px..(px + pw).min(ia.w) {
                    ia.set(xx, yy, hues[c]);
                    ib.set(xx, yy, [b, b, b]);
                }}
                mp.frame_bytes[0][i] = if c >= 2 { 224 } else { 64 };
            }
            mp.mark_edited();
            d.frames[0].images[0] = lightmap::img::encode_webp_lossless(&ia).expect("enc A");
            d.frames[0].images[1] = lightmap::img::encode_webp_lossless(&ib).expect("enc B");
            let payload = m.chunk.write(true);
            lightmap::mapio::save_with_chunk(&m, &payload, &a[2]).expect("save");
            println!("chunk {} B; wrote {}", payload.len(), a[2]);
        }
        "greycheck" => {
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let d = m.chunk.data.as_ref().unwrap();
            for (fi, fr) in d.frames.iter().enumerate() { for (ii, im) in fr.images.iter().enumerate() {
                if im.is_empty() { continue; }
                let img = lightmap::img::decode_webp(im).expect("dec");
                let mut nongrey = 0usize; let mut maxdiff = 0i32;
                for p in img.px.chunks(3) { let dd = (p[0] as i32 - p[1] as i32).abs().max((p[1] as i32 - p[2] as i32).abs()); if dd > 2 { nongrey += 1; } maxdiff = maxdiff.max(dd); }
                println!("frame {fi} image {ii}: {}x{} non-grey pixels {} of {} (max channel diff {})", img.w, img.h, nongrey, img.w * img.h, maxdiff);
            }}
        }
        "shotstats" => {
            // lmtool shotstats FRAME.ppm : classify pixels by hue class (r,g,b,y tints) and report luminance stats per class
            let data = std::fs::read(&a[1]).expect("read ppm");
            // parse P6 header
            let mut idx = 0; let mut fields = Vec::new();
            while fields.len() < 4 { let s = idx; while data[idx] != b' ' && data[idx] != b'\n' { idx += 1; } fields.push(std::str::from_utf8(&data[s..idx]).unwrap().to_string()); idx += 1; }
            let w: usize = fields[1].parse().unwrap(); let h: usize = fields[2].parse().unwrap();
            let px = &data[idx..];
            let y0 = h / 8; // skip the HUD strip at the top
            let mut cls: [Vec<f32>; 4] = [vec![], vec![], vec![], vec![]];
            for y in y0..h { for x in 0..w {
                let p = &px[(y * w + x) * 3..][..3];
                let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
                let mx = r.max(g).max(b); let mn = r.min(g).min(b);
                if mx < 20.0 || mx - mn < 0.12 * mx { continue; } // dark or grey: unclassifiable
                let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                // tint pattern: which channels are "high"
                let hi = |v: f32| v > mn + 0.6 * (mx - mn);
                let (hr, hg, hb) = (hi(r), hi(g), hi(b));
                let c = match (hr, hg, hb) { (true, false, false) => 0, (false, true, false) => 1, (false, false, true) => 2, (true, true, false) => 3, _ => continue };
                cls[c].push(lum);
            }}
            let names = ["red   (B=0,fb=64) ", "green (B=255,fb=64)", "blue  (B=0,fb=224)", "yellow(B=255,fb=224)"];
            for c in 0..4 {
                let v = &mut cls[c]; v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if v.is_empty() { println!("{}: none", names[c]); continue; }
                let q = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
                println!("{}: n={:>8} lum p25 {:.0} median {:.0} p75 {:.0} p90 {:.0}", names[c], v.len(), q(0.25), q(0.5), q(0.75), q(0.9));
            }
        }
        "synth" => {
            // lmtool synth MAP.Map.Gbx --template T.Map.Gbx --out OUT.Map.Gbx [--base 4096] [--item-px 8] [--ground-px 2]
            //   [--a R,G,B] [--b V] [--fb0 V] [--spec spec.tsv (item<TAB>r,g,b<TAB>b<TAB>fb0)]
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let map_path = &a[1];
            let tpl = lightmap::mapio::load(&f("--template").expect("--template")).expect("template");
            let m = lightmap::mapio::load(map_path).expect("map");
            // item count and positions from the map itself
            let mf = tmmaps_items(map_path);
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let parse_rgb = |s: &str| -> [u8; 3] { let v: Vec<u8> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] };
            let mut item_default = lightmap::synth::ChartSpec::default();
            if let Some(s) = f("--a") { item_default.a = parse_rgb(&s); }
            if let Some(s) = f("--b") { item_default.b = s.parse().unwrap(); }
            if let Some(s) = f("--fb0") { item_default.fb[0] = s.parse().unwrap(); }
            let mut item_spec = vec![None; mf.len()];
            if let Some(sp) = f("--spec") {
                for l in std::fs::read_to_string(&sp).expect("spec").lines() {
                    if l.trim().is_empty() || l.starts_with('#') { continue; }
                    let c: Vec<&str> = l.split('\t').collect();
                    let i: usize = c[0].parse().unwrap();
                    let mut s = item_default;
                    s.a = parse_rgb(c[1]); s.b = c[2].parse().unwrap(); s.fb[0] = c[3].parse().unwrap();
                    item_spec[i] = Some(s);
                }
            }
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &mf { for k in 0..3 { lo[k] = lo[k].min(p[k] - 16.0); hi[k] = hi[k].max(p[k] + 16.0); } }
            let plan = lightmap::synth::Plan {
                base, items: mf.len() as u32,
                ground_px: f("--ground-px").map(|s| s.parse().unwrap()).unwrap_or(2),
                item_px: f("--item-px").map(|s| s.parse().unwrap()).unwrap_or(8),
                ground: lightmap::synth::ChartSpec::default(),
                item_default, item_spec, bbox: if a.iter().any(|x| x == "--bbox-template") { let tm = tpl.chunk.data.as_ref().unwrap().cache.mapping().unwrap(); (tm.bbox_min, tm.bbox_max) } else { (lo, hi) },
            };
            let s = lightmap::synth::synth(&plan, &tpl.chunk).expect("synth");
            let payload = s.chunk.write(false);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&m, &payload, &out).expect("save");
            println!("{} charts ({} ground + {} items); chunk {} B; wrote {out}", s.charts, base, mf.len(), payload.len());
        }
        "shotratio" => {
            // lmtool shotratio CLASSES.ppm WHITE.ppm : per hue class, the luminance ratio classes/white on
            // pixels that changed (items), albedo cancels; the tints' own luminance is divided out
            let load = |p: &str| -> (usize, usize, Vec<u8>) {
                let data = std::fs::read(p).expect("read ppm");
                let mut idx = 0; let mut fields = Vec::new();
                while fields.len() < 4 { let s = idx; while data[idx] != b' ' && data[idx] != b'\n' { idx += 1; } fields.push(std::str::from_utf8(&data[s..idx]).unwrap().to_string()); idx += 1; }
                (fields[1].parse().unwrap(), fields[2].parse().unwrap(), data[idx..].to_vec())
            };
            let (w, h, pc) = load(&a[1]);
            let (w2, h2, pw) = load(&a[2]);
            assert!(w == w2 && h == h2);
            let tints: [[f32; 3]; 4] = [[230.0, 170.0, 170.0], [170.0, 230.0, 170.0], [170.0, 170.0, 230.0], [230.0, 230.0, 170.0]];
            let lum = |r: f32, g: f32, b: f32| 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let mut ratios: [Vec<f32>; 4] = [vec![], vec![], vec![], vec![]];
            let mut chan_ratios: [Vec<[f32; 3]>; 4] = [vec![], vec![], vec![], vec![]];
            for y in h / 8..h { for x in 0..w {
                let i = (y * w + x) * 3;
                let (r, g, b) = (pc[i] as f32, pc[i + 1] as f32, pc[i + 2] as f32);
                let (r0, g0, b0) = (pw[i] as f32, pw[i + 1] as f32, pw[i + 2] as f32);
                // in the white run the item pixels are neutral-lit; require a real change and no saturation
                let lw = lum(r0, g0, b0);
                if lw < 40.0 || lw > 235.0 || r0.max(g0).max(b0) > 245.0 { continue; }
                let mx = r.max(g).max(b); let mn = r.min(g).min(b);
                if mx - mn < 0.10 * mx { continue; }
                // the tint changes channel RATIOS relative to the white run: classify by which channels dropped least
                let (qr, qg, qb) = (r / r0.max(1.0), g / g0.max(1.0), b / b0.max(1.0));
                let qmax = qr.max(qg).max(qb);
                let hi = |q: f32| q > 0.85 * qmax;
                let c = match (hi(qr), hi(qg), hi(qb)) { (true, false, false) => 0, (false, true, false) => 1, (false, false, true) => 2, (true, true, false) => 3, _ => continue };
                // remove the tint: expected channel factors tint/255
                let t = tints[c];
                let lt = lum(t[0], t[1], t[2]) / 255.0;
                ratios[c].push(lum(r, g, b) / lw / lt);
                chan_ratios[c].push([qr / (t[0] / 255.0), qg / (t[1] / 255.0), qb / (t[2] / 255.0)]);
            }}
            let names = ["red    B=0   fb0=64 ", "green  B=255 fb0=64 ", "blue   B=0   fb0=224", "yellow B=255 fb0=224"];
            for c in 0..4 {
                let v = &mut ratios[c]; v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if v.len() < 100 { println!("{}: only {} pixels", names[c], v.len()); continue; }
                let q = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
                let cr = &chan_ratios[c];
                let med = |k: usize| { let mut t: Vec<f32> = cr.iter().map(|x| x[k]).collect(); t.sort_by(|a, b| a.partial_cmp(b).unwrap()); t[t.len() / 2] };
                println!("{}: n={:>8} lum ratio vs white(fb0=148,B=128): p25 {:.3} median {:.3} p75 {:.3}   per-channel median r {:.3} g {:.3} b {:.3}", names[c], v.len(), q(0.25), q(0.5), q(0.75), med(0), med(1), med(2));
            }
        }
        "bake" | "sunfit" => {
            // lmtool bake MAP --template T --out OUT [--sun-az D --sun-el D] [--sky r,g,b] [--sun r,g,b] [--k K]
            //   [--tpm T] [--flip-v] [--bounce F] [--albedo A] [--sky-samples N] [--sun-samples N] [--ground-y Y] [--base 4096]
            // lmtool sunfit MAP [--items N] [--sky-samples N]: grid over sun directions vs the map's own bake
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let map_path = a[1].clone();
            let t0 = std::time::Instant::now();
            // --mood auto (the default without --template): the header's collection + mood select the
            // fitted lighting (moods.rs) and the template chunk `<Collection>-<Mood>.lmchunk` from the bank
            // (--templates DIR, $LMTOOL_TEMPLATES, or the store's lightmap-re/templates)
            let hdr = tmmaps::header::read(&map_path).ok();
            let mood_sel: Option<&lightmap::moods::MoodParams> = if a[0] == "bake" && (f("--template").is_none() || f("--mood").is_some()) {
                let h = hdr.as_ref().expect("map header (needed for --mood auto)");
                let mood_name = match f("--mood") { Some(m) if m != "auto" => m, _ => h.mood.clone() };
                match lightmap::moods::lookup(&h.envir, &mood_name) {
                    Some(p) => { if f("--model").as_deref() == Some("fitted") { eprintln!("mood: {} {} ({}) — sun az {} el {}, template {}", p.collection, p.mood, p.confidence, p.sun_az, p.sun_el, p.template); } Some(p) }
                    None => { if f("--model").as_deref() == Some("fitted") { panic!("no fitted mood parameters for {} / {} — known:\n{}", h.envir, mood_name, lightmap::moods::table()); } None }
                }
            } else { None };
            let scene = lightmap::geometry::Scene::from_map(&map_path).expect("scene");
            eprintln!("scene: {} models, {} instances, {} triangles ({:.1}s)", scene.models.len(), scene.instances.len(), scene.tri_count(), t0.elapsed().as_secs_f32());
            let tris = lightmap::bake::world_tris(&scene);
            let bvh = lightmap::bvh::Bvh::build(tris);
            eprintln!("bvh: {} nodes ({:.1}s)", bvh.node_count(), t0.elapsed().as_secs_f32());
            let parse_rgb = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] };
            let mut prm = lightmap::bake::BakeParams::default();
            // --model xml (default) | fitted: the game's model (RE child, 2026-09-23) — the effective mood by the
            // map's DayTime quarter, E = LAmbient·(0.8+0.2n.y) + LAmbient·SkyFactor·skyVis·S + BounceFactor·albedo·bounce,
            // NO direct sun (real-time; it only feeds the bounce), absolute HDR units; `fitted` = the 2026-09-22 rows
            let model = f("--model").unwrap_or_else(|| "xml".into());
            let mut xml_sel: Option<&lightmap::moods::MoodXml> = None;
            if a[0] == "bake" && model == "xml" {
                let h = hdr.as_ref().expect("map header");
                let mf0 = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let dt = lightmap::mapio::daytime(&mf0.gbx.body);
                let mood = match f("--mood") { Some(m) if m != "auto" => lightmap::moods::normalise_mood(&m), _ => lightmap::moods::effective_mood(&mf0.decoration_id, dt) };
                let x = lightmap::moods::mood_xml(&h.envir, mood).unwrap_or_else(|| panic!("no mood XML for {} {mood}", h.envir));
                xml_sel = Some(x);
                // the dome model: sky = 1.55·LAmbient·SkyFactor (an open floor on the BlueBay Sunset-quarter test
                // bakes reads (0.61, 0.58, 0.77) = 1.55 × LAmbient in LAmbient's hue), no separate ambient term
                let dome = !has("--hemi");
                let sky_s: f32 = f("--sky-scale").map(|s| s.parse().unwrap()).unwrap_or(if dome { 1.55 } else { 1.0 });
                prm.dome_deg = if dome { f("--cone-deg").map(|s| s.parse().unwrap()).unwrap_or(30.0) } else { 90.0 };
                prm.ambient_la = if dome { [0.0; 3] } else { x.l_ambient };
                prm.sky = [x.l_ambient[0] * x.sky_factor * sky_s, x.l_ambient[1] * x.sky_factor * sky_s, x.l_ambient[2] * x.sky_factor * sky_s];
                if let Some(g) = f("--ground-bounce") { prm.ground_bounce = g.parse().unwrap(); }
                prm.bounce_sphere = has("--bounce-sphere");
                // the game's peel model (RE child 2): the 256-point sphere set, first surface per direction —
                // the DEFAULT since 2026-09-23 09:15Z (it beats the cone on the tiny-16 q4 reference: per-texel
                // RMSE 64 % vs 92 %); --cone / --hemi select the older models
                let peel = !has("--cone") && !has("--hemi") && !has("--no-peel");
                if peel {
                    prm.peel = true;
                    let fit = lightmap::moods::sky_fit(x.collection, x.mood);
                    if f("--albedo").is_none() { prm.albedo = fit.1; }
                    // the rendered sky: the mood's SkyColor gradient (+ Atmo lobes from its XML) unless --flat-sky;
                    // --sky-grad-scale k scales the gradient (GlobalScale·ScaleGrad0 stand-in), --lobe-scale the lobes
                    if !has("--flat-sky") {
                        let coll = x.collection;
                        let mood = x.mood;
                        let path = f("--sky-grad").unwrap_or_else(|| lightmap::skygrad::mood_file(coll, mood, "SkyColor.dds"));
                        match lightmap::skygrad::SkyGradient::load(&path) {
                            Ok(mut g) => {
                                // the gradient's global scale: 1.6 fits the BlueBay Sunset open floor (0.607) — per-mood
                                // values pending (GlobalScale·ScaleGrad0 from the runtime sky constants)
                                g.scale = f("--sky-grad-scale").map(|s| s.parse().unwrap()).unwrap_or_else(|| lightmap::moods::sky_grad_scale(coll, mood)) * x.sky_factor;
                                g.sun_dir = prm.sun_dir;
                                g.sun_az = prm.sun_dir[0].atan2(prm.sun_dir[2]);
                                g.v_full = has("--v-full");
                                if has("--v-flip") || (fit.2 && !has("--no-v-flip")) { g.v_top_is_zenith = false; }
                                if let Some(o) = f("--u-sun") { g.u_sun = o.parse().unwrap(); }
                                if has("--u-flip") { g.u_sign = -1.0; }
                                let lobe_scale: f32 = f("--lobe-scale").map(|s| s.parse().unwrap()).unwrap_or(1.0);
                                if let Ok(xml) = std::fs::read_to_string(lightmap::skygrad::mood_file(coll, mood, "Mood.MoodSetting.xml")) {
                                    g.lobes = lightmap::skygrad::lobes_from_xml(&xml).into_iter().map(|(p, c, s)| (p, c, s * lobe_scale)).collect();
                                }
                                eprintln!("sky: {} ({}×{}), scale {}, lobes {:?}", path.rsplit('/').next().unwrap(), g.w, g.h, g.scale, g.lobes.iter().map(|l| (l.0, l.2)).collect::<Vec<_>>());
                                prm.sky_grad = Some(std::sync::Arc::new(g));
                            }
                            Err(e) => eprintln!("sky gradient: {e}; flat sky"),
                        }
                    }
                    // the sweeps' direction counts follow the quality (RE child 2: High = 1024, 512, 256, 128; the
                    // table set nearest the count, rotated by the lightmapper's fixed matrix); the first sweep's set
                    // is loaded here, the later ones per iteration below
                    let q: u32 = f("--quality").map(|s| s.parse().unwrap()).unwrap_or(3);
                    let counts = lightmap::dome::sweep_counts(q);
                    let n0 = counts.first().copied().unwrap_or(256);
                    let pp = f("--points").unwrap_or_else(lightmap::dome::default_path);
                    if let Ok(ps) = lightmap::dome::PointSets::load(&pp) { if let Some(set) = ps.nearest(n0) { prm.sphere_dirs = std::sync::Arc::new(lightmap::dome::rotate_set(set)); eprintln!("peel: quality {q}, sweeps {:?}, first set {} directions (rotated)", counts, set.len()); } }
                    if let Some(v) = f("--bounce-decode") { prm.bounce_decode = v.parse().unwrap(); }
                    if let Some(v) = f("--horizon-el") { prm.horizon_el = v.parse().unwrap(); }
                    if let Some(v) = f("--horizon-rgb") { prm.horizon_radiance = parse_rgb(&v); }
                }
                if has("--no-sun-bounce") { prm.sun = [0.0; 3]; }
                // the decoration's ground/sea plane (the terrain collections' water sits at y ≈ 8 in the tiny
                // maps' frame, the Stadium floor likewise) — a stand-in for the decoration meshes
                if f("--ground-y").is_none() { prm.ground_y = 8.0; }
                // the game's own directions: the sphere-table points inside the cone (the banked table, or --points)
                if dome && !has("--random-dome") {
                    let pp = f("--points").unwrap_or_else(lightmap::dome::default_path);
                    match lightmap::dome::PointSets::load(&pp) {
                        Ok(ps) => {
                            // the generator draws N/((1−cos A)/2) table points and keeps those in the cone (N = 256)
                            let draw = (256.0 / ((1.0 - prm.dome_deg.to_radians().cos()) / 2.0)) as usize;
                            let set = ps.set(draw.max(256)).cloned().unwrap_or_default();
                            let c = prm.dome_deg.to_radians().cos();
                            let dirs: Vec<[f32; 3]> = set.iter().copied().filter(|p| p[1] >= c).take(256).collect();
                            eprintln!("dome: {} of the table's {}-set inside {}° ({} drawn)", dirs.len(), set.len(), prm.dome_deg, draw.min(set.len()));
                            prm.dome_dirs = std::sync::Arc::new(dirs);
                        }
                        Err(e) => eprintln!("dome: {e}; using stratified random directions"),
                    }
                }
                prm.sun = x.l_dir_sun;
                prm.direct_sun = 0.0;
                prm.ambient = [0.0; 3]; prm.up = [0.0; 3];
                prm.bounce = f("--bounce").map(|s| s.parse().unwrap()).unwrap_or(x.bounce_factor);
                prm.albedo = f("--albedo").map(|s| s.parse().unwrap()).unwrap_or(if prm.peel { lightmap::moods::sky_fit(x.collection, x.mood).1 } else { 0.18 });
                prm.uv_bounds = true; prm.sky_model = 0; prm.texels_per_m = 1.0; prm.sky_samples = 64; prm.sun_samples = 4;
                prm.ambient_ao = !has("--no-ao");
                // local lights, absolute units: E = k·I·c·max(0,n·l)·(1−(d/R)²)²; k = 0.56 puts the peak under a lamp post
                // (2 omni lights, I 1, c 0.92, R 10 m, 4 m up) at the editor's 0.35–0.38 HDR (two BlueBay test bakes,
                // 9-px and 153-px pads); the tail beyond x ≈ 0.85 is fatter than (1−x²)² in the editor (unpinned)
                prm.light_k = 0.56;
                // the sun: IN the stored frame (a wall facing it reads 1.1 HDR on the Sunset-quarter test bake —
                // above any bounce source) with a jittered disc; its direction per quarter is MEASURED on the
                // test bakes until the DecorationMood formula is pinned: Sunset quarter (0.854) ≈ az 115° (ESE),
                // el 2° (floors get 3 % of it, east walls full). Other quarters: the noon-at-½ arc on the
                // mood's latitude until their test bakes land. --sun-az/--sun-el override.
                // NO direct sun on the receiving texels (RE child 2, RenderLightIndirectPeel: the sun lights only
                // the peeled bounce surfaces; the low pink ESE term of the Sunset-quarter bakes is the rendered
                // SKY's sun-side glow — Tech3/Sky_p, not LDirSun). --direct-sun forces the old term back on.
                let _ = t_quarter(dt, &x);
                prm.direct_sun = if has("--direct-sun") { 1.0 } else { 0.0 };
                prm.sun_radius = f("--sun-radius").map(|s: String| s.parse::<f32>().unwrap()).unwrap_or(2.0f32).to_radians();
                let t = match dt { Some(v) if v != 0xffff_ffff => v as f32 / 65536.0, _ => lightmap::moods::default_daytime(&x.collection, x.mood) as f32 / 65536.0 };
                // the sun DIRECTION (bounce input + the sky glow's azimuth): measured at the Sunset quarter
                // (0.854, BlueBay): az ≈ 115° (ESE), el ≈ 1–2°; other quarters: the noon-at-½ arc until the
                // DecorationMood formula is read (RE child 2)
                let (mut az_d, mut el_d) = if t >= 0.75 {
                    (115.0f32, 2.0f32)
                } else {
                    let hour_angle = (t - 0.5) * 2.0 * std::f32::consts::PI;
                    let lat = x.latitude.to_radians();
                    let el = (lat.cos() * hour_angle.cos()).asin();
                    let az = hour_angle.sin().atan2(-lat.sin() * hour_angle.cos());
                    (az.to_degrees(), el.to_degrees().max(2.0))
                };
                if let Some(v) = f("--sun-az") { az_d = v.parse().unwrap(); }
                if let Some(v) = f("--sun-el") { el_d = v.parse().unwrap(); }
                let (ar, er) = (az_d.to_radians(), el_d.to_radians());
                prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
                eprintln!("model xml: {} {} (decoration {}, daytime {}) — LAmbient {:?} SkyFactor {} Bounce {} MaxHDR {}; sun az {az_d:.1}° el {el_d:.1}° (direct {})", x.collection, x.mood, mf0.decoration_id, match dt { Some(v) if v != 0xffff_ffff => format!("{:.3}", v as f32 / 65536.0), _ => format!("default → {t:.3}") }, x.l_ambient, x.sky_factor, x.bounce_factor, x.max_hdr, prm.direct_sun > 0.0);
            } else if let Some(p) = mood_sel {
                prm.sky = p.sky; prm.sun = p.sun; prm.ambient = p.ambient; prm.up = p.up; prm.light_k = p.light_k;
                prm.uv_bounds = true; prm.sky_model = 1; prm.texels_per_m = 1.0; prm.sky_samples = 64; prm.sun_samples = 4;
                let (ar, er) = (p.sun_az.to_radians(), p.sun_el.to_radians());
                prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
            }
            if let Some(s) = f("--sky") { prm.sky = parse_rgb(&s); }
            if let Some(p) = f("--sky-cube") { prm.sky_cube = Some(std::sync::Arc::new(lightmap::skycube::CubeMap::load(&p).expect("sky cube"))); prm.sky = [0.0; 3]; }
            if let Some(s) = f("--sky-scale") { prm.sky_cube_scale = s.parse().unwrap(); }
            if let Some(s) = f("--sun") { prm.sun = parse_rgb(&s); }
            if let Some(s) = f("--ambient") { prm.ambient = parse_rgb(&s); }
            if let Some(s) = f("--up") { prm.up = parse_rgb(&s); }
            if let Some(s) = f("--tpm") { prm.texels_per_m = s.parse().unwrap(); }
            if let Some(s) = f("--bounce") { prm.bounce = s.parse().unwrap(); }
            if let Some(s) = f("--albedo") { prm.albedo = s.parse().unwrap(); }
            if let Some(s) = f("--sky-samples") { prm.sky_samples = s.parse().unwrap(); }
            if let Some(s) = f("--sun-samples") { prm.sun_samples = s.parse().unwrap(); }
            if let Some(s) = f("--ground-y") { prm.ground_y = s.parse().unwrap(); }
            if let Some(s) = f("--max-px") { prm.max_px = s.parse().unwrap(); }
            if let Some(s) = f("--sky-model") { prm.sky_model = s.parse().unwrap(); }
            prm.flip_v = has("--flip-v");
            if has("--uv-bounds") { prm.uv_bounds = true; }
            prm.pattern = has("--pattern");
            let sun_dir = |az: f32, el: f32| -> [f32; 3] { let (a, e) = (az.to_radians(), el.to_radians()); [e.cos() * a.sin(), e.sin(), e.cos() * a.cos()] };
            // --base N | auto (the default with a mood): decoration constant + blocks + empty ground columns (moods.rs)
            let base: u32 = match f("--base").as_deref() {
                Some("auto") | None if mood_sel.is_some() || f("--base").as_deref() == Some("auto") => {
                    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                    let h = hdr.as_ref().expect("header");
                    let cells = mf.blocks.iter().map(|b| { let c = b.coords(); (c.0, c.2, b.name.as_str()) });
                    let extra: u32 = f("--base-extra").map(|s| s.parse().unwrap()).unwrap_or(0);
                    let baked_total: Option<u32> = f("--baked-total").map(|s| s.parse().unwrap());
                    let r = lightmap::moods::base_rule(&h.envir, &mf.decoration_id, mf.size, cells, extra, baked_total);
                    match baked_total {
                        Some(t) => eprintln!("base auto: {} = {} (decoration) + {} authored blocks ({} custom) + {} baked (the game's list: S² + G)", r.base(), r.deco_const, r.authored, r.custom_blocks, t),
                        None => {
                            eprintln!("base auto: {} = {} (decoration) + {} authored blocks ({} custom) + {} ground tiles − {} replaced + {} generated pieces{}", r.base(), r.deco_const, r.authored, r.custom_blocks, r.ground_cols, r.replaced, r.extra, if !mf.baked.is_empty() { format!(" ({} baked records in the file do not count)", mf.baked.len()) } else { String::new() });
                            if r.stadium && r.authored > 0 && extra == 0 { eprintln!("  WARNING: {} authored blocks on a Stadium decoration — the game grows pillars/walls under elevated blocks (386 around the tiny 05's 46 water tiles, 7 under 25 ×2's platform, 2108 around 05 ×2's 604 pool tiles); pass --baked-total N (/mapblocks2?list=baked) or --base-extra G or --base N", r.authored); }
                        }
                    }
                    r.base()
                }
                Some(s) => s.parse().unwrap(),
                None => 4096,
            };
            // --base-candidates B1,B2,…: the object-index base MEASURED in one load — the
            // items' charts are emitted once per candidate base, each candidate in its own
            // hue (red, green, blue, yellow, magenta, cyan) over a neutral 4 m world
            // checkerboard (--pattern); the hue whose checkers render continuous on the
            // items in play is the game's base for this file class (2026-09-22, the giant
            // grids: is the base S², S²+N authored blocks, or the Stadium constant?)
            let candidates: Vec<u32> = f("--base-candidates").map(|s| s.split(',').filter(|t| !t.trim().is_empty()).map(|t| t.trim().parse().expect("--base-candidates wants numbers")).collect()).unwrap_or_default();
            if !candidates.is_empty() { prm.pattern = true; prm.pattern_flat = true; }
            // the map's own (Nadeo/editor) bake, for fitting and comparison
            let own = lightmap::mapio::load(&map_path).ok();
            let own_charts: Option<std::collections::HashMap<u32, (u8, [f32; 3])>> = own.as_ref().and_then(|m| {
                let d = m.chunk.data.as_ref()?;
                let mp = d.cache.mapping()?;
                let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).ok()?;
                let mut h = std::collections::HashMap::new();
                for i in 0..mp.count as usize {
                    let obj = mp.binds[i].obj_group_idx / 4;
                    if obj < base { continue; }
                    let (x, y) = mp.pos[i]; let (w, hh) = mp.size[i];
                    let mean = ia.mean((x as u32 + 1) / 2, (y as u32 + 1) / 2, w as u32 / 2, hh as u32 / 2);
                    h.insert(obj - base, (mp.frame_bytes[0][i], mean));
                }
                Some(h)
            });
            let compare = |charts: &[lightmap::bake::ChartBake], label: &str| -> f64 {
                if a[0] == "bake" { if let Some(k) = implied_k(charts, &own_charts) { IMPLIED_K.store(k.to_bits(), std::sync::atomic::Ordering::Relaxed); } }
                let Some(own) = &own_charts else { return 0.0 };
                // correlation of log(max) and of the colour ratio r/b with the bake
                let (mut xs, mut ys, mut rs, mut qs) = (vec![], vec![], vec![], vec![]);
                for c in charts {
                    let Some(&(fb0, mean)) = own.get(&(c.item as u32)) else { continue };
                    let mx = c.max_channel();
                    if mx <= 1e-4 || fb0 == 0 { continue; }
                    xs.push((mx as f64).ln()); ys.push((fb0 as f64).ln());
                    let mine = c.mean();
                    if mine[2] > 1e-4 && mean[2] > 1.0 { rs.push((mine[0] / mine[2]) as f64); qs.push((mean[0] / mean[2]) as f64); }
                }
                let corr = |x: &[f64], y: &[f64]| -> f64 {
                    let n = x.len() as f64; if n < 3.0 { return 0.0; }
                    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
                    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
                    for (a, b) in x.iter().zip(y) { sxy += (a - mx) * (b - my); sxx += (a - mx) * (a - mx); syy += (b - my) * (b - my); }
                    sxy / (sxx * syy).sqrt().max(1e-12)
                };
                let c1 = corr(&xs, &ys); let c2 = corr(&rs, &qs);
                // implied K: median of 255*max/fb0
                let mut ks: Vec<f64> = xs.iter().zip(&ys).map(|(x, y)| 255.0 * x.exp() / y.exp()).collect();
                ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let kmed = ks.get(ks.len() / 2).copied().unwrap_or(0.0);
                println!("{label}: n={} corr(log max, log fb0)={c1:.3} corr(r/b)={c2:.3} implied K median={kmed:.3}", xs.len());
                c1 + c2
            };
            if a[0] == "sunfit" {
                // a subset of instances, coarse sampling
                let nitems: usize = f("--items").map(|s| s.parse().unwrap()).unwrap_or(1200);
                prm.sky_samples = f("--sky-samples").map(|s| s.parse().unwrap()).unwrap_or(16);
                prm.sun_samples = 1;
                let step = (scene.instances.len() / nitems).max(1);
                let sub = lightmap::geometry::Scene { models: scene.models.clone(), model_names: scene.model_names.clone(), instances: scene.instances.iter().step_by(step).cloned().collect(), item_count: scene.item_count };
                // the subset's instances must keep their own inst id for self-hit filtering: rebuild the bvh over all, but
                // the shade() skip uses the instance index in `sub` — so we bake the subset against a bvh of the FULL scene
                // whose inst ids are full-scene indices; map them
                let full_ids: Vec<u32> = (0..scene.instances.len()).step_by(step).map(|x| x as u32).collect();
                let mut best = (f64::MIN, 0.0f32, 0.0f32);
                let azs: Vec<f32> = (0..24).map(|i| i as f32 * 15.0).collect();
                let els: Vec<f32> = f("--els").map(|s| s.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or_else(|| vec![10.0, 20.0, 30.0, 45.0, 60.0, 75.0]);
                for &el in &els { for &az in &azs {
                    prm.sun_dir = sun_dir(az, el);
                    let charts = lightmap::bake::bake_subset(&sub, &full_ids, &bvh, &prm);
                    let score = compare(&charts, &format!("az {az:>5.1} el {el:>4.1}"));
                    if score > best.0 { best = (score, az, el); }
                }}
                println!("best: az {} el {} (score {:.3})", best.1, best.2, best.0);
                return;
            }
            if xml_sel.is_none() && (f("--sun-az").is_some() || f("--sun-el").is_some() || mood_sel.is_none()) {
                let az: f32 = f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(0.0);
                let el: f32 = f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(45.0);
                prm.sun_dir = sun_dir(az, el);
            }
            let lights = if a.iter().any(|x| x == "--no-lights") { Vec::new() } else { scene.world_lights() };
            if let Some(s) = f("--light-k") { prm.light_k = s.parse().unwrap(); }
            eprintln!("{} point lights", lights.len());
            // atlas density auto-step: the item charts at this density plus the ground/decoration charts must
            // shelf-pack into 1024²; otherwise the largest density (binary search, 1 % steps) that packs is used
            // (24 ×4: 64516 ground tiles + 20k items fail at the tiny maps' 1.0)
            {
                let n_ground = if hdr.as_ref().map(|h| h.envir.eq_ignore_ascii_case("stadium")).unwrap_or(false) { base.saturating_sub(16384) } else { base };
                let fits = |tpm: f32| -> bool {
                    let mut p2 = prm.clone();
                    p2.texels_per_m = tpm;
                    let mut sizes: Vec<(u32, u32)> = scene.instances.iter().map(|inst| {
                        let m = &scene.models[inst.model];
                        let c0 = [inst.xf[0], inst.xf[1], inst.xf[2]];
                        lightmap::bake::chart_size(m, (c0[0] * c0[0] + c0[1] * c0[1] + c0[2] * c0[2]).sqrt(), &p2)
                    }).collect();
                    // ground tiles and the items without geometry (skipped models) are 2×2 charts
                    sizes.extend(std::iter::repeat((2u32, 2u32)).take(n_ground as usize + scene.item_count.saturating_sub(scene.instances.len())));
                    lightmap::synth::shelf_fits(&sizes, 1024)
                };
                // the editor's density is adaptive (a 33-item test map gets 153-px pads where the full map gets
                // 9-px ones): the largest density that packs, searched both ways unless --tpm pins it
                let pinned = f("--tpm").is_some();
                if !fits(prm.texels_per_m) {
                    let (mut lo, mut hi) = (0.05f32, prm.texels_per_m);
                    if fits(lo) {
                        for _ in 0..12 { let mid = 0.5 * (lo + hi); if fits(mid) { lo = mid; } else { hi = mid; } }
                        eprintln!("atlas density: {:.2} texels/m does not pack {} items ({} with geometry) + {} ground charts into 1024²; stepping to {:.3}", prm.texels_per_m, scene.item_count, scene.instances.len(), n_ground, lo);
                        prm.texels_per_m = lo;
                    } else {
                        eprintln!("atlas density: even {lo:.2} texels/m does not pack {} items + {} ground charts (2×2 minimum) into 1024² — the pack will shrink further", scene.instances.len(), n_ground);
                    }
                } else if !pinned {
                    let (mut lo, mut hi) = (prm.texels_per_m, prm.texels_per_m * 2.0);
                    while hi < 64.0 && fits(hi) { lo = hi; hi *= 2.0; }
                    for _ in 0..10 { let mid = 0.5 * (lo + hi); if fits(mid) { lo = mid; } else { hi = mid; } }
                    if lo > prm.texels_per_m * 1.05 {
                        eprintln!("atlas density: {:.2} texels/m leaves room — stepping UP to {:.3} ({} items + {} ground charts)", prm.texels_per_m, lo, scene.instances.len(), n_ground);
                        prm.texels_per_m = lo;
                    } else {
                        eprintln!("atlas density {:.2} texels/m packs ({} items + {} ground charts)", prm.texels_per_m, scene.instances.len(), n_ground);
                    }
                } else {
                    eprintln!("atlas density {:.2} texels/m (pinned) packs ({} items + {} ground charts)", prm.texels_per_m, scene.instances.len(), n_ground);
                }
            }
            // multi-bounce (the game: 2 iterations at Default, 4 High, 6 Ultra — each a sweep whose input
            // radiance is the lightmap so far + the direct sun): iteration 0 bakes with the one-bounce
            // estimate, every further pass reads the previous pass's charts at the hit points
            let q_sweeps = lightmap::dome::sweep_counts(f("--quality").map(|s| s.parse().unwrap()).unwrap_or(3));
            let iterations: usize = f("--bounces").map(|s| s.parse().unwrap()).unwrap_or(if prm.peel && !q_sweeps.is_empty() { q_sweeps.len() } else if xml_sel.is_some() { 2 } else { 1 });
            let mut charts = lightmap::bake::bake(&scene, &bvh, &prm, &lights);
            eprintln!("baked {} charts ({:.1}s)", charts.len(), t0.elapsed().as_secs_f32());
            for it in 1..iterations {
                let mut field = lightmap::bake::RadianceField { charts: vec![None; scene.instances.len()], flip_v: prm.flip_v, uv_bounds: prm.uv_bounds };
                let inst_of_item: std::collections::HashMap<usize, usize> = scene.instances.iter().enumerate().map(|(ii, inst)| (inst.item, ii)).collect();
                for c in &charts { if let Some(&ii) = inst_of_item.get(&c.item) { field.charts[ii] = Some((c.w, c.h, c.rgb.clone())); } }
                let mut p2 = prm.clone();
                p2.field = Some(std::sync::Arc::new(field));
                if prm.peel {
                    if let Some(&n) = q_sweeps.get(it) {
                        let pp = f("--points").unwrap_or_else(lightmap::dome::default_path);
                        if let Ok(ps) = lightmap::dome::PointSets::load(&pp) { if let Some(set) = ps.nearest(n) { p2.sphere_dirs = std::sync::Arc::new(lightmap::dome::rotate_set(set)); } }
                    }
                }
                charts = lightmap::bake::bake(&scene, &bvh, &p2, &lights);
                let mean: f32 = charts.iter().flat_map(|c| c.rgb.iter()).map(|c| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).sum::<f32>() / charts.iter().map(|c| c.rgb.len()).sum::<usize>().max(1) as f32;
                eprintln!("bounce iteration {it}: mean texel {mean:.4} ({:.1}s)", t0.elapsed().as_secs_f32());
            }
            compare(&charts, "bake vs own");
            let k: f32 = match f("--k") {
                Some(s) => s.parse().unwrap(),
                None if xml_sel.is_some() => {
                    // absolute units: the frame's MaxHDR = min(the brightest chart, the mood's MaxHDR); fb = 255·chartMax/K
                    let x = xml_sel.unwrap();
                    let max_e = charts.iter().flat_map(|c| c.rgb.iter()).flat_map(|c| c.iter().copied()).fold(0.0f32, f32::max);
                    let kk = max_e.min(x.max_hdr).max(0.05);
                    eprintln!("frame MaxHDR = min(max E {max_e:.3}, mood MaxHDR {}) = {kk:.4}", x.max_hdr);
                    kk
                }
                None if mood_sel.is_some() => 1.0,
                None => { let k = f32::from_bits(IMPLIED_K.load(std::sync::atomic::Ordering::Relaxed)); if k > 0.0 { eprintln!("K matched to the map's own bake: {k:.3}"); k } else { 3.0 } }
            };
            let tpl_path = match f("--template") {
                Some(p) => p,
                None => {
                    let dir = f("--templates").or_else(|| std::env::var("LMTOOL_TEMPLATES").ok()).unwrap_or_else(|| format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/templates", std::env::var("HOME").unwrap_or_default()));
                    match xml_sel {
                        // the template only supplies the constants the baker does not derive: the effective mood's
                        // chunk when the bank has one, else any chunk of the collection
                        Some(x) => {
                            let want = format!("{dir}/{}-{}.lmchunk", x.collection, x.mood);
                            if std::path::Path::new(&want).exists() { want } else {
                                let mut any: Vec<String> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path().to_string_lossy().to_string()).filter(|p| p.rsplit('/').next().unwrap_or("").starts_with(&format!("{}-", x.collection)) && p.ends_with(".lmchunk")).collect()).unwrap_or_default();
                                any.sort();
                                any.first().cloned().unwrap_or_else(|| panic!("no template chunk for {} under {dir}", x.collection))
                            }
                        }
                        None => { let p = mood_sel.expect("--template or --mood auto"); format!("{dir}/{}.lmchunk", p.template) }
                    }
                }
            };
            eprintln!("template {tpl_path}");
            let tpl = lightmap::mapio::load_template(&tpl_path).expect("template");
            let m = lightmap::mapio::load(&map_path).expect("map");
            // a ground tile = an open horizontal surface: LA·1.0 + sky (+ the direct sun only when baked)
            let ground_e: [f32; 3] = { let l = prm.sun_dir[1].max(0.0) * prm.direct_sun; [prm.ambient_la[0] + prm.ambient[0] + prm.up[0] + prm.sky[0] + prm.sun[0] * l, prm.ambient_la[1] + prm.ambient[1] + prm.up[1] + prm.sky[1] + prm.sun[1] * l, prm.ambient_la[2] + prm.ambient[2] + prm.up[2] + prm.sky[2] + prm.sun[2] * l] };
            let mut out_charts = Vec::new();
            // the Stadium decorations reserve objects 0..16383 (0..3 = the decoration's own charts); everything
            // between that constant and the item base is a block or a ground column → a flat ground chart
            let stadium = hdr.as_ref().map(|h| h.envir.eq_ignore_ascii_case("stadium")).unwrap_or(base > 8192 && f("--base").is_none());
            let deco_const: u32 = if stadium { 16384 } else { 0 };
            if stadium {
                // the 4 decoration objects (0..3, several charts each — copied from the template's atlas as flat grey
                // charts of the same size) and the ground slots 16384..base
                let td = tpl.chunk.data.as_ref().unwrap();
                let tmm = td.cache.mapping().unwrap();
                let tia = lightmap::img::decode_webp(&td.frames[0].images[0]).expect("template atlas");
                let mut deco = 0;
                for i in 0..tmm.count as usize {
                    let obj = tmm.binds[i].obj_group_idx / 4;
                    if obj >= 4 { continue; }
                    let (x, y) = tmm.pos[i]; let (w, h) = tmm.size[i];
                    let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                    let mut a8 = Vec::with_capacity((pw * ph) as usize);
                    for yy in 0..ph { for xx in 0..pw { a8.push(tia.get((px + xx).min(tia.w - 1), (py + yy).min(tia.h - 1))); } }
                    out_charts.push(lightmap::synth::Chart { obj, w: pw, h: ph, a: a8, a1: Vec::new(), b: 128, fb: [tmm.frame_bytes[0][i], 0, 0], sub: tmm.binds[i].obj_idx });
                    deco += 1;
                }
                eprintln!("  {deco} decoration charts copied from the template");
            }
            for obj in deco_const..base { out_charts.push(lightmap::synth::Chart::from_hdr(obj, 2, 2, &[ground_e; 4], k, 128)); }
            let mut have = vec![false; scene.item_count];
            if candidates.is_empty() {
                for c in &charts { have[c.item] = true; out_charts.push(lightmap::synth::Chart::from_hdr2(base + c.item as u32, c.w, c.h, &c.rgb, &c.rgb1, k, 128)); }
            } else {
                // the candidate test: every item's chart once per candidate base, in that
                // candidate's hue; overlapping candidate ranges would double-book objects
                let n_items = scene.item_count as u32;
                for (i, a) in candidates.iter().enumerate() {
                    for b in candidates.iter().skip(i + 1) {
                        if a.max(b) < &(a.min(b) + n_items) { panic!("--base-candidates {a} and {b} overlap over {n_items} items"); }
                    }
                }
                let hues: [[f32; 3]; 6] = [[1.0, 0.12, 0.12], [0.12, 1.0, 0.12], [0.2, 0.2, 1.0], [1.0, 1.0, 0.12], [1.0, 0.12, 1.0], [0.12, 1.0, 1.0]];
                let names = ["red", "green", "blue", "yellow", "magenta", "cyan"];
                for (j, cb) in candidates.iter().enumerate() {
                    let hue = hues[j % hues.len()];
                    eprintln!("  base candidate {cb}: items in {}", names[j % names.len()]);
                    for c in &charts {
                        have[c.item] = true;
                        let rgb: Vec<[f32; 3]> = c.rgb.iter().map(|p| [p[0] * hue[0], p[1] * hue[1], p[2] * hue[2]]).collect();
                        let rgb1: Vec<[f32; 3]> = c.rgb1.iter().map(|p| [p[0] * hue[0], p[1] * hue[1], p[2] * hue[2]]).collect();
                        out_charts.push(lightmap::synth::Chart::from_hdr2(cb + c.item as u32, c.w, c.h, &rgb, &rgb1, k, 128));
                    }
                }
                // the ground slots of the primary base stay; slots claimed by a candidate range are dropped
                out_charts.retain(|ch| !(ch.obj < base && candidates.iter().any(|cb| ch.obj >= *cb && ch.obj < cb + n_items) && ch.w == 2 && ch.h == 2));
            }
            for (i, h) in have.iter().enumerate() { if !h { out_charts.push(lightmap::synth::Chart::from_hdr(base + i as u32, 2, 2, &[ground_e; 4], k, 128)); } }
            let tm = tpl.chunk.data.as_ref().unwrap().cache.mapping().unwrap();
            // the probe volume: ours unless --template-probes
            let vp8_q: Option<u8> = f("--vp8").map(|s| s.parse().unwrap());
            let probes = if has("--template-probes") { None } else {
                let tv = lightmap::volume::Volume::parse(&tpl.chunk.data.as_ref().unwrap().cache.trailer).expect("template trailer");
                let mut pp = prm.clone();
                pp.sky_samples = f("--probe-samples").map(|s| s.parse().unwrap()).unwrap_or(48);
                // the slot grid: origin per decoration, counts from the map grid (size words) and the lit
                // geometry; --slots NX,NY,NZ / --slot-origin X,Y,Z override, --slots template copies the template's
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let h = hdr.as_ref().expect("header");
                let grid_m = [mf.size[0] as f32 * 32.0, mf.size[1] as f32 * 8.0, mf.size[2] as f32 * 32.0];
                let (mut glo, mut ghi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for t in &bvh.tris { for p in [t.p0, lightmap::geometry::add(t.p0, t.e1), lightmap::geometry::add(t.p0, t.e2)] { for k in 0..3 { glo[k] = glo[k].min(p[k]); ghi[k] = ghi[k].max(p[k]); } } }
                // every item's position counts for the extent, stock (non-embedded) items included: the giant
                // builds park unused vegetation at (8, −900, 8) and the game's grid follows them
                for it in &mf.items { for k in 0..3 { glo[k] = glo[k].min(it.pos[k] - 16.0); ghi[k] = ghi[k].max(it.pos[k]); } }
                let mut grid = lightmap::probes::SlotGrid::for_map(&h.envir, &mf.decoration_id, grid_m, glo, ghi);
                if let Some(o) = f("--slot-origin") { grid.origin = parse_rgb(&o); }
                match f("--slots").as_deref() {
                    Some("template") => { grid.n = tv.slot_grid; grid.origin = tv.world_origin(); grid.cell = tv.cell_size(); }
                    Some(s) => { let v: Vec<u32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); grid.n = [v[0], v[1], v[2]]; }
                    None => {}
                }
                if let Some(c) = f("--probe-cell") { grid.cell = c.parse().unwrap(); }
                eprintln!("slot grid {:?} origin {:?} cell {} m (map {:?} = {:.0}×{:.0}×{:.0} m, decoration {}, geometry ({:.0}, {:.0}, {:.0})..({:.0}, {:.0}, {:.0}); template {:?} origin {:?})", grid.n, grid.origin, grid.cell, mf.size, grid_m[0], grid_m[1], grid_m[2], mf.decoration_id, glo[0], glo[1], glo[2], ghi[0], ghi[1], ghi[2], tv.slot_grid, tv.world_origin());
                // the probe images are Monte-Carlo noisy: a coarser quantizer than the atlases (Nadeo's 874² probe
                // blob is 394 KB; ours at q 8 was 1.5 MB — over the 25 MiB file cap on the ×4 maps)
                let probe_q: u8 = f("--probe-vp8").map(|s| s.parse().unwrap()).unwrap_or(28);
                let po = lightmap::probes::build(&scene, &bvh, &pp, &lights, prm.light_k, &tv, probe_q, &grid).expect("probes");
                eprintln!("probe volume: {} blocks, {} slices, atlas {}x{}, blob {} B ({:.1}s)", po.blocks, po.slices, po.atlas_w, po.atlas_h, po.blob.len(), t0.elapsed().as_secs_f32());
                Some(lightmap::synth::ProbeBlob { blob: po.blob, trailer: po.volume.write() })
            };
            // the frame records' time-of-day word: the MAP's (chunk 0x03043056), or the mood's default word
            // when the map has none (what Nadeo's editor baked the default-word sources with) — `--daytime N`
            // overrides, `--daytime template` keeps the template's (giant child 2026-09-23 + baker-3)
            let frame_params = xml_sel.map(|x| {
                let mf0 = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let own = lightmap::mapio::daytime(&mf0.gbx.body).unwrap_or(0xffff_ffff);
                let daytime = match f("--daytime").as_deref() {
                    Some("template") => 0xffff_ffff,
                    Some(n) if n != "auto" => n.parse().expect("--daytime auto|template|N"),
                    _ if own == 0xffff_ffff => lightmap::moods::default_daytime(x.collection, x.mood),
                    _ => own,
                };
                // Σ chart area (m²): the items' PreLightGen extents × scale + √2² per zone tile (BlueBay measured;
                // the editor's chunk 0x0602200B) — the density the editor's own layout would use
                let items_area: f32 = scene.instances.iter().map(|inst| { let mdl = &scene.models[inst.model]; let sc = (inst.xf[0] * inst.xf[0] + inst.xf[1] * inst.xf[1] + inst.xf[2] * inst.xf[2]).sqrt(); match mdl.plg_bounds { Some(b) => (b[2] - b[0]) * mdl.plg_u02 * sc * (b[3] - b[1]) * mdl.plg_u02 * sc, None => 0.0 } }).sum();
                let n_tiles = base.saturating_sub(deco_const) as f32;
                let quality: u32 = f("--quality").map(|s| s.parse::<u32>().unwrap()).unwrap_or(3).saturating_sub(1);
                lightmap::synth::FrameParams { daytime, max_hdr_mood: x.max_hdr, max_hdr: k, bounce: x.bounce_factor, sky: x.sky_factor, sum_area: Some(items_area + 2.0 * n_tiles), quality: Some(quality), decoration: Some(mf0.decoration_id.clone()) }
            });
            let s = lightmap::synth::build_full2(out_charts, (tm.bbox_min, tm.bbox_max), &tpl.chunk, probes, vp8_q, frame_params).expect("build");
            let payload = s.chunk.write(false);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&m, &payload, &out).expect("save");
            println!("{} charts, atlas fill {:.1}%, chunk {} B; wrote {out} ({:.1}s)", s.charts, s.fill * 100.0, payload.len(), t0.elapsed().as_secs_f32());
        }
        "geomstats" => {
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let mut hist: std::collections::BTreeMap<u32, usize> = Default::default();
            let mut big: Vec<(usize, &str, f32, [f32; 2], [f32; 2])> = Vec::new();
            for (mi, m) in scene.models.iter().enumerate() {
                *hist.entry(m.metres_per_uv.round() as u32).or_insert(0) += 1;
                big.push((m.tris.len(), &scene.model_names[mi], m.metres_per_uv, m.uv_min, m.uv_max));
            }
            big.sort_by_key(|b| std::cmp::Reverse(b.0));
            println!("metres_per_uv histogram (rounded): {:?}", hist);
            for b in big.iter().take(12) { println!("  {:>8} tris  {}  m/uv {:.2}  uv [{:.2},{:.2}]..[{:.2},{:.2}]", b.0, b.1, b.2, b.3[0], b.3[1], b.4[0], b.4[1]); }
            let no_uv = scene.models.iter().filter(|m| m.tris.is_empty()).count();
            println!("{} models without lightmap triangles", no_uv);
        }
        "rbhist" => {
            // per-chart mean colour ratio r/b and its relation to fb0, in the map's own bake (items only)
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let d = m.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let base: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(4096);
            let mut rows: Vec<(f32, f32, u8)> = Vec::new();
            for i in 0..mp.count as usize {
                let obj = mp.binds[i].obj_group_idx / 4;
                if obj < base { continue; }
                let (x, y) = mp.pos[i]; let (w, h) = mp.size[i];
                let mean = ia.mean((x as u32 + 1) / 2, (y as u32 + 1) / 2, w as u32 / 2, h as u32 / 2);
                if mean[2] < 1.0 { continue; }
                rows.push((mean[0] / mean[2], mean[1] / mean[2], mp.frame_bytes[0][i]));
            }
            let mut hist: std::collections::BTreeMap<i32, (usize, f32)> = Default::default();
            for (rb, _, fb) in &rows { let e = hist.entry((rb * 10.0).floor() as i32).or_insert((0, 0.0)); e.0 += 1; e.1 += *fb as f32; }
            println!("r/b bucket: count, mean fb0");
            for (k, (n, s)) in &hist { println!("  {:.1}-{:.1}: {n:>6}  fb0 {:.0}", *k as f32 / 10.0, (*k + 1) as f32 / 10.0, s / *n as f32); }
            let mut gb: Vec<f32> = rows.iter().map(|r| r.1).collect(); gb.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("g/b median {:.3}", gb[gb.len() / 2]);
        }
        "flatten" => {
            // lmtool flatten TEMPLATE OUT --what a|b : flatten only image A (to white per chart) or only image B (to 128)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let mut m = lightmap::mapio::load(&a[1]).expect("load");
            let what = f("--what").unwrap_or_else(|| "b".into());
            let d = m.chunk.data.as_mut().expect("has lightmaps");
            if what == "b" {
                let mut ib = lightmap::img::decode_webp(&d.frames[0].images[1]).expect("B");
                for p in ib.px.iter_mut() { *p = 128; }
                d.frames[0].images[1] = lightmap::img::encode_webp_lossless(&ib).expect("enc");
            } else {
                let mut ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("A");
                for p in ia.px.iter_mut() { *p = 255; }
                d.frames[0].images[0] = lightmap::img::encode_webp_lossless(&ia).expect("enc");
            }
            let payload = m.chunk.write(false);
            lightmap::mapio::save_with_chunk(&m, &payload, &a[2]).expect("save");
            println!("wrote {}", a[2]);
        }
        "facefit" => {
            // lmtool facefit MAP [--base 4096]: per-face-direction mean HDR (A*fb0/255) of the map's own bake,
            // sampled through OUR uv rasterisation of each item, for both V orientations
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own bake");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let dirs = ["+x", "-x", "+y", "-y", "+z", "-z"];
            for (flip, ub) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut sum = [[0f64; 3]; 6]; let mut cnt = [0f64; 6]; let mut var = 0f64; let mut nvar = 0f64;
                for (ii, inst) in scene.instances.iter().enumerate() {
                    let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                    let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                    let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
                    if pw == 0 || ph == 0 { continue; }
                    let fb0 = mp.frame_bytes[0][ci] as f64 / 255.0;
                    let (samples, _cov) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, flip, ub);
                    let mut per_face: Vec<Vec<f64>> = vec![vec![]; 6];
                    for s in &samples {
                        let n = s.n;
                        let bucket = if n[0] > 0.9 { 0 } else if n[0] < -0.9 { 1 } else if n[1] > 0.9 { 2 } else if n[1] < -0.9 { 3 } else if n[2] > 0.9 { 4 } else if n[2] < -0.9 { 5 } else { continue };
                        let c = ia.get((px + s.px).min(1023), (py + s.py).min(1023));
                        let lum = (0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64) * fb0;
                        per_face[bucket].push(lum);
                        for k in 0..3 { sum[bucket][k] += c[k] as f64 * fb0; }
                        cnt[bucket] += 1.0;
                    }
                    for v in &per_face { if v.len() >= 4 { let m = v.iter().sum::<f64>() / v.len() as f64; var += v.iter().map(|x| (x - m) * (x - m)).sum::<f64>(); nvar += v.len() as f64; } }
                }
                println!("flip_v={flip} uv_bounds={ub}: within-face luminance stddev {:.2}", (var / nvar.max(1.0)).sqrt());
                for b in 0..6 { if cnt[b] > 0.0 { println!("  {}: n={:>8} mean HDR (K=1 units) r {:.3} g {:.3} b {:.3}", dirs[b], cnt[b], sum[b][0] / cnt[b], sum[b][1] / cnt[b], sum[b][2] / cnt[b]); } }
            }
        }
        "chartcmp" => {
            // lmtool chartcmp MAP ITEM OUTBASE [--sun-az D --sun-el D] [--flip-v] [--k K]: Nadeo's chart vs ours for one item, x8 PPMs
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let item: usize = a[2].parse().unwrap();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let ci = (0..mp.count as usize).find(|&i| mp.binds[i].obj_group_idx / 4 == base + item as u32).expect("chart");
            let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
            let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
            let fb0 = mp.frame_bytes[0][ci];
            let ii = scene.instances.iter().position(|i| i.item == item).expect("instance");
            println!("item {item}: model {} chart {pw}x{ph} px fb0 {fb0} m/uv {:.1}", scene.instances[ii].model_name, scene.models[scene.instances[ii].model].metres_per_uv);
            let mut prm = lightmap::bake::BakeParams::default();
            let az: f32 = f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(90.0);
            let el: f32 = f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(15.0);
            let (ar, er) = (az.to_radians(), el.to_radians());
            prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
            prm.flip_v = has("--flip-v");
            prm.uv_bounds = has("--uv-bounds");
            if let Some(s) = f("--bounce") { prm.bounce = s.parse().unwrap(); }
            prm.sky_samples = 64;
            let sub = lightmap::geometry::Scene { models: scene.models.clone(), model_names: scene.model_names.clone(), instances: vec![scene.instances[ii].clone()], item_count: scene.item_count };
            // bake at Nadeo's resolution
            prm.min_px = pw.max(ph); prm.max_px = pw.max(ph);
            let mine = lightmap::bake::bake_subset_px(&sub, &[ii as u32], &bvh, &prm, pw, ph);
            let c = &mine[0];
            let mymax = c.max_channel();
            let sc = 8u32;
            let mut out_n = lightmap::img::Rgb::new(pw * sc, ph * sc);
            let mut out_m = lightmap::img::Rgb::new(pw * sc, ph * sc);
            for oy in 0..ph * sc { for ox in 0..pw * sc {
                let (sx, sy) = (ox / sc, oy / sc);
                out_n.set(ox, oy, ia.get(px + sx, py + sy));
                let v = c.rgb[(sy * pw + sx) as usize];
                let g = |t: f32| (t / mymax * 255.0).clamp(0.0, 255.0) as u8;
                out_m.set(ox, oy, [g(v[0]), g(v[1]), g(v[2])]);
            }}
            lightmap::img::write_ppm(&out_n, &format!("{}_nadeo.ppm", a[3])).unwrap();
            lightmap::img::write_ppm(&out_m, &format!("{}_mine.ppm", a[3])).unwrap();
            println!("my chart max {mymax:.3} (K=1 units: Nadeo fb0/255 = {:.3}); wrote {}_nadeo.ppm / _mine.ppm", fb0 as f32 / 255.0, a[3]);
        }
        "uvinfo" => {
            // lmtool uvinfo ITEM.Item.Gbx : per-visual uv1 bounds, triangle counts, world area
            let bytes = std::fs::read(&a[1]).expect("read");
            let f = mapgeom::static_item::file::parse_file(&bytes).expect("parse");
            let s2 = f.item.static_object().and_then(|so| so.solid2()).expect("solid2");
            println!("{} shaded geoms, {} visuals, lod_max_dist {:?}", s2.shaded_geoms.len(), s2.visuals.len(), s2.lod_max_dist);
            if let Some(plg) = &s2.pre_light_gen { println!("PreLightGen v{} u01 {} u02 {} u03 {} u04 {:?} sprites {:?} boxes {} uv_groups {}", plg.version, plg.u01, plg.u02, plg.u03, plg.u04, plg.sprite_count, plg.boxes.len(), plg.uv_groups.len()); }
            use mapgeom::static_item::vstream::{Elem, N_POSITION, N_TEXCOORD0};
            for (gi, sg) in s2.shaded_geoms.iter().enumerate() {
                let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
                let Some(mapgeom::static_item::Node::Visual(v)) = vr.inline.as_deref() else { println!("geom {gi}: visual {} not inline", sg.visual_index); continue };
                let Some(st) = v.stream() else { continue };
                let get = |name: u32| st.decls.iter().zip(st.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
                let ntri = v.index_buffer.as_ref().map(|ib| ib.indices.len() / 3).unwrap_or(0);
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                if let Some(Elem::Float2(uv)) = get(N_TEXCOORD0 + 1) { for p in uv { lo[0] = lo[0].min(p[0]); lo[1] = lo[1].min(p[1]); hi[0] = hi[0].max(p[0]); hi[1] = hi[1].max(p[1]); } }
                let (mut plo, mut phi) = ([f32::MAX; 3], [f32::MIN; 3]);
                if let Some(Elem::Float3(p)) = get(N_POSITION) { for q in p { for k in 0..3 { plo[k] = plo[k].min(q[k]); phi[k] = phi[k].max(q[k]); } } }
                let main = v.main.as_ref();
                println!("geom {gi}: visual {} mat {} lod {} tris {ntri} uv1 [{:.3},{:.3}]..[{:.3},{:.3}] pos [{:.1},{:.1},{:.1}]..[{:.1},{:.1},{:.1}] texsets {} uv_groups {:?} bitmap_elems {:?}", sg.visual_index, sg.material_index, sg.lod_mask, lo[0], lo[1], hi[0], hi[1], plo[0], plo[1], plo[2], phi[0], phi[1], phi[2], main.map(|m| m.tex_coord_sets.len()).unwrap_or(0), main.map(|m| m.uv_groups.clone()).unwrap_or_default(), main.map(|m| m.bitmap_elems.clone()).unwrap_or_default());
            }
        }
        "texcorr" => {
            // lmtool texcorr MAP [--sun-az --sun-el] [--items N]: per-texel correlation of our bake with the map's own
            // atlas, for the 4 uv-mapping conventions (flip × bounds)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let nitems: usize = f("--items").map(|s| s.parse().unwrap()).unwrap_or(600);
            let az: f32 = f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(90.0);
            let el: f32 = f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(15.0);
            let (ar, er) = (az.to_radians(), el.to_radians());
            // the biggest charts carry the signal: take the N largest
            let mut cand: Vec<(usize, u32)> = scene.instances.iter().enumerate().filter_map(|(ii, inst)| chart_of.get(&(inst.item as u32)).map(|&ci| (ii, mp.size[ci].0 as u32 * mp.size[ci].1 as u32))).collect();
            cand.sort_by_key(|c| std::cmp::Reverse(c.1));
            let sel: Vec<usize> = cand.iter().take(nitems).map(|c| c.0).collect();
            for (flip, ub) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut prm = lightmap::bake::BakeParams::default();
                prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
                prm.flip_v = flip; prm.uv_bounds = ub; prm.sky_samples = 16; prm.sun_samples = 1;
                let (mut csum, mut cn) = (0f64, 0usize);
                for &ii in &sel {
                    let inst = &scene.instances[ii];
                    let ci = chart_of[&(inst.item as u32)];
                    let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                    let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32) / 2, (h as u32) / 2);
                    if pw < 4 || ph < 4 { continue; }
                    let sub = lightmap::geometry::Scene { models: scene.models.clone(), model_names: scene.model_names.clone(), instances: vec![inst.clone()], item_count: scene.item_count };
                    let mine = lightmap::bake::bake_subset_px(&sub, &[ii as u32], &bvh, &prm, pw, ph);
                    let c = &mine[0];
                    let (mut xs, mut ys) = (vec![], vec![]);
                    for yy in 0..ph { for xx in 0..pw {
                        let i = (yy * pw + xx) as usize;
                        if !c.covered[i] { continue; }
                        let v = c.rgb[i]; let lum_m = 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
                        let n = ia.get((px + xx).min(1023), (py + yy).min(1023)); let lum_n = 0.2126 * n[0] as f32 + 0.7152 * n[1] as f32 + 0.0722 * n[2] as f32;
                        xs.push(lum_m as f64); ys.push(lum_n as f64);
                    }}
                    if xs.len() < 12 { continue; }
                    let n = xs.len() as f64;
                    let (mx, my) = (xs.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
                    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
                    for (p, q) in xs.iter().zip(&ys) { sxy += (p - mx) * (q - my); sxx += (p - mx) * (p - mx); syy += (q - my) * (q - my); }
                    if sxx > 1e-9 && syy > 1e-9 { csum += sxy / (sxx * syy).sqrt(); cn += 1; }
                }
                println!("flip_v={flip} uv_bounds={ub}: mean per-chart texel correlation {:.3} over {cn} charts", csum / cn.max(1) as f64);
            }
        }
        "sunfit2" => {
            // lmtool sunfit2 MAP [--items N] [--sky-samples N] [--bounce F] [--azs a,b,..] [--els a,b,..]
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let nitems: usize = f("--items").map(|s| s.parse().unwrap()).unwrap_or(300);
            let mut cand: Vec<(usize, u32)> = scene.instances.iter().enumerate().filter_map(|(ii, inst)| chart_of.get(&(inst.item as u32)).map(|&ci| (ii, mp.size[ci].0 as u32 * mp.size[ci].1 as u32))).collect();
            cand.sort_by_key(|c| std::cmp::Reverse(c.1));
            let sel: Vec<(usize, u32, u32, u32, u32)> = cand.iter().take(nitems).filter_map(|&(ii, _)| {
                let ci = chart_of[&(scene.instances[ii].item as u32)];
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (pw, ph) = (w as u32 / 2, h as u32 / 2);
                if pw < 4 || ph < 4 { return None; }
                Some((ii, (x as u32 + 1) / 2, (y as u32 + 1) / 2, pw, ph))
            }).collect();
            let mut prm = lightmap::bake::BakeParams::default();
            prm.uv_bounds = true; prm.flip_v = false;
            prm.sky_samples = f("--sky-samples").map(|s| s.parse().unwrap()).unwrap_or(16); prm.sun_samples = 1;
            if let Some(s) = f("--bounce") { prm.bounce = s.parse().unwrap(); }
            if let Some(s) = f("--sky-model") { prm.sky_model = s.parse().unwrap(); }
            if let Some(s) = f("--inset") { prm.inset_px = s.parse().unwrap(); }
            if let Some(s) = f("--sun") { let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect(); prm.sun = [v[0], v[1], v[2]]; }
            if let Some(s) = f("--sky") { let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect(); prm.sky = [v[0], v[1], v[2]]; }
            let azs: Vec<f32> = f("--azs").map(|s| s.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or_else(|| (0..12).map(|i| i as f32 * 30.0).collect());
            let els: Vec<f32> = f("--els").map(|s| s.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or_else(|| vec![10.0, 25.0, 45.0]);
            let mut best = (f64::MIN, 0.0f32, 0.0f32);
            for &el in &els { for &az in &azs {
                let (ar, er) = (az.to_radians(), el.to_radians());
                prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
                let t = std::time::Instant::now();
                let (c, n) = lightmap::bake::texel_correlation(&scene, &bvh, &sel, &ia, &prm);
                println!("az {az:>5.1} el {el:>4.1}: texel corr {c:.4} ({n} charts, {:.1}s)", t.elapsed().as_secs_f32());
                if c > best.0 { best = (c, az, el); }
            }}
            println!("best: az {} el {} corr {:.4}", best.1, best.2, best.0);
        }
        "poolfit" => {
            // lmtool poolfit MAP --sun-az A --sun-el E [--items N] [--sun r,g,b] [--sky r,g,b] [--bounce F] [--sky-model M]
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let nitems: usize = f("--items").map(|s| s.parse().unwrap()).unwrap_or(1500);
            let step = (scene.instances.len() / nitems).max(1);
            let sel: Vec<(usize, u32, u32, u32, u32, u8)> = scene.instances.iter().enumerate().step_by(step).filter_map(|(ii, inst)| {
                let &ci = chart_of.get(&(inst.item as u32))?;
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (pw, ph) = (w as u32 / 2, h as u32 / 2);
                if pw < 2 || ph < 2 { return None; }
                Some((ii, (x as u32 + 1) / 2, (y as u32 + 1) / 2, pw, ph, mp.frame_bytes[0][ci]))
            }).collect();
            let mut prm = lightmap::bake::BakeParams::default();
            prm.uv_bounds = true; prm.flip_v = false; prm.sky_samples = 32; prm.sun_samples = 1;
            if let Some(s) = f("--bounce") { prm.bounce = s.parse().unwrap(); }
            if let Some(s) = f("--sky-model") { prm.sky_model = s.parse().unwrap(); }
            if let Some(s) = f("--regressor") { prm.fit_regressor = s.parse().unwrap(); }
            if let Some(s) = f("--sun") { let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect(); prm.sun = [v[0], v[1], v[2]]; }
            if let Some(s) = f("--sky") { let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect(); prm.sky = [v[0], v[1], v[2]]; }
            let az: f32 = f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(75.0);
            let el: f32 = f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(45.0);
            let (ar, er) = (az.to_radians(), el.to_radians());
            prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
            let (c, slope, n) = lightmap::bake::pooled_fit(&scene, &bvh, &sel, &ia, &prm);
            println!("pooled: pearson {c:.4} slope(nadeo/mine) {slope:.4} over {n} texels  [sun {:?} sky {:?} bounce {} skymodel {}]", prm.sun, prm.sky, prm.bounce, prm.sky_model);
            let (ca, cb, cc, r2) = lightmap::bake::component_fit(&scene, &bvh, &sel, &ia, &prm);
            let (rgb, _) = lightmap::bake::component_fit_rgb(&scene, &bvh, &sel, &ia, &prm);
            println!("per channel (K=1 units): sky r,g,b = {:.4},{:.4},{:.4}  sun = {:.4},{:.4},{:.4}  bounce(lum-weight) = {:.4},{:.4},{:.4}  const = {:.4},{:.4},{:.4}", rgb[0][0], rgb[1][0], rgb[2][0], rgb[0][1], rgb[1][1], rgb[2][1], rgb[0][2], rgb[1][2], rgb[2][2], rgb[0][3], rgb[1][3], rgb[2][3]);
            println!("component fit: ref_lum(K=1 units) = {ca:.4}·skyVis + {cb:.4}·(N·L·sunVis) + {cc:.4}   r² {r2:.3}   sun/sky ratio {:.3}", cb / ca.max(1e-9));
        }
        "trailer" => {
            for f in &a[1..] {
                let m = lightmap::mapio::load(f).expect("load");
                let d = m.chunk.data.as_ref().unwrap();
                println!("== {f}: trailer {} bytes", d.cache.trailer.len());
                match lightmap::volume::Volume::parse(&d.cache.trailer) {
                    Ok(v) => {
                        let rt = v.write() == d.cache.trailer; print!("{}", v.describe()); println!("round-trip {}", if rt { "OK" } else { "MISMATCH" });
                        // cell4 hypothesis: one u16 per 4x4 atlas cell, bit = pixel covered by a stored tile
                        if let Ok(im) = lightmap::img::decode_webp(&d.frames[0].images[2]) {
                            let (cw, chh) = ((im.w + 3) / 4, (im.h + 3) / 4);
                            println!("atlas {}x{} -> {}x{} cells = {} (table {})", im.w, im.h, cw, chh, cw * chh, v.cell4.len());
                            let mut cov = vec![false; (cw * 4 * chh * 4) as usize];
                            for b in &v.blocks { let (tw, th) = (b.max[0] - b.min[0], b.max[2] - b.min[2]); for s in b.slices.iter().flatten() { for y in 0..th { for x in 0..tw { let (px, py) = (s.0 + x, s.1 + y); if px < cw * 4 && py < chh * 4 { cov[(py * cw * 4 + px) as usize] = true; } } } } }
                            // variant: bit = NOT a dark probe (uncovered pixels count as set)
                            let mut lit = vec![true; (cw * 4 * chh * 4) as usize];
                            for y in 0..im.h { for x in 0..im.w { let c = im.get(x, y); let l = 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32; if l < 40.0 && cov[(y * cw * 4 + x) as usize] { lit[(y * cw * 4 + x) as usize] = false; } } }
                            let mut agree_lit = 0; let mut shown2 = 0;
                            for cy in 0..chh { for cx in 0..cw { let mut mask = 0u16; for y in 0..4 { for x in 0..4 { if lit[((cy * 4 + y) * cw * 4 + cx * 4 + x) as usize] { mask |= 1 << (y * 4 + x); } } } let stored = v.cell4.get((cy * cw + cx) as usize).copied().unwrap_or(0); if stored == mask { agree_lit += 1; } else if shown2 < 6 { println!("  LIT cell ({cx},{cy}): stored {stored:016b} lit {mask:016b}"); shown2 += 1; } } }
                            println!("cell4 LIT variant: {} of {} agree", agree_lit, cw * chh);
                            let mut agree = 0; let mut agree_inv = 0; let mut shown = 0;
                            for cy in 0..chh { for cx in 0..cw {
                                let mut mask = 0u16;
                                for y in 0..4 { for x in 0..4 { if cov[((cy * 4 + y) * cw * 4 + cx * 4 + x) as usize] { mask |= 1 << (y * 4 + x); } } }
                                let stored = v.cell4.get((cy * cw + cx) as usize).copied().unwrap_or(0);
                                if stored == mask { agree += 1; } if stored == !mask { agree_inv += 1; }
                                if stored != mask && stored != !mask && shown < 6 { println!("  cell ({cx},{cy}): stored {stored:016b} covered {mask:016b}"); shown += 1; }
                            }}
                            println!("cell4: {} of {} cells equal the coverage mask, {} equal its inverse", agree, cw * chh, agree_inv);
                        }
                    }
                    Err(e) => println!("ERR {e}"),
                }
            }
        }
        "voltiles" => {
            // lmtool voltiles MAP OUTDIR: every stored probe-volume slice as a x8 PPM + dark-fraction stats
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let d = m.chunk.data.as_ref().unwrap();
            let v = lightmap::volume::Volume::parse(&d.cache.trailer).expect("trailer");
            let im = lightmap::img::decode_webp(&d.frames[0].images[2]).expect("atlas 2");
            std::fs::create_dir_all(&a[2]).unwrap();
            println!("atlas {}x{}", im.w, im.h);
            for (bi, b) in v.blocks.iter().enumerate() {
                let (tw, th) = (b.max[0] - b.min[0], b.max[2] - b.min[2]);
                for (si, s) in b.slices.iter().enumerate() {
                    let Some((tx, ty)) = s else { continue };
                    let mut dark = 0usize;
                    let mut sum = [0f64; 3];
                    let mut rows_dark = vec![0usize; th as usize];
                    let sc = 8u32;
                    let mut out = lightmap::img::Rgb::new(tw * sc, th * sc);
                    for y in 0..th {
                        for x in 0..tw {
                            let c = im.get((tx + x).min(im.w - 1), (ty + y).min(im.h - 1));
                            let l = 0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64;
                            if l < 40.0 {
                                dark += 1;
                                rows_dark[y as usize] += 1;
                            }
                            for k in 0..3 {
                                sum[k] += c[k] as f64;
                            }
                            for oy in 0..sc {
                                for ox in 0..sc {
                                    out.set(x * sc + ox, y * sc + oy, c);
                                }
                            }
                        }
                    }
                    let n = (tw * th) as f64;
                    let rd: Vec<String> = rows_dark.iter().map(|r| format!("{}", (10 * *r as u32 / tw.max(1)).min(9))).collect();
                    // darkest texel (for point-like probes)
                    let mut best = (f32::MAX, 0u32, 0u32);
                    for y in 0..th { for x in 0..tw { let c = im.get((tx + x).min(im.w - 1), (ty + y).min(im.h - 1)); let l = 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32; if l < best.0 { best = (l, x, y); } } }
                    print!("darkest ({},{}) lum {:.0}  ", best.1, best.2, best.0);
                    println!("block {bi:>2} slice {si:>2} (axis1 cell {}) tile {}x{} at ({},{}): dark {:.1}% mean ({:.0},{:.0},{:.0}) rows-dark/10 {}", b.min[1] + si as u32, tw, th, tx, ty, 100.0 * dark as f64 / n, sum[0] / n, sum[1] / n, sum[2] / n, rd.join(""));
                    lightmap::img::write_ppm(&out, &format!("{}/b{bi:02}_s{si:02}.ppm", a[2])).unwrap();
                }
            }
        }
        "volfit" => {
            // lmtool volfit MAP [--dy D,...]: with probe world = pos + 16·(cell + ½) (x, z) and
            // y = pos.y + 16·(c1 + ½) + dy, how well do the dark texels match "inside geometry"
            // (the first hit of a ray straight up is a back face)?
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let d = m.chunk.data.as_ref().unwrap();
            let v = lightmap::volume::Volume::parse(&d.cache.trailer).expect("trailer");
            let im = lightmap::img::decode_webp(&d.frames[0].images[2]).expect("atlas 2");
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let mut samples: Vec<([f32; 3], bool)> = Vec::new();
            for b in &v.blocks {
                let (tw, th) = (b.max[0] - b.min[0], b.max[2] - b.min[2]);
                for (si, s) in b.slices.iter().enumerate() {
                    let Some((tx, ty)) = s else { continue };
                    let c1 = b.min[1] + si as u32;
                    for y in 0..th { for x in 0..tw {
                        let c = im.get((tx + x).min(im.w - 1), (ty + y).min(im.h - 1));
                        let l = 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32;
                        let (cx, cz) = (b.min[0] + x, b.min[2] + y);
                        let p = [b.pos[0] + 16.0 * (cx as f32 + 0.5), b.pos[1] + 16.0 * (c1 as f32 + 0.5), b.pos[2] + 16.0 * (cz as f32 + 0.5)];
                        samples.push((p, l < 40.0));
                    }}
                }
            }
            println!("{} texels, {} dark", samples.len(), samples.iter().filter(|s| s.1).count());
            let dys: Vec<f32> = f("--dy").map(|s| s.split(',').map(|x| x.parse().unwrap()).collect()).unwrap_or_else(|| vec![-24.0, -16.0, -8.0, 0.0, 8.0, 16.0]);
            // per-level profile: dark fraction vs box-overlap fraction at dy = 0 (levels relative to pos.y)
            {
                let mut per: std::collections::BTreeMap<i32, (usize, usize, usize)> = Default::default();
                for (p, dark) in &samples {
                    let l = ((p[1] + 46.0 - 8.0) / 16.0).round() as i32; // c1 relative (pos.y = -46 - 256·row)
                    let e = per.entry(l).or_insert((0, 0, 0));
                    e.0 += 1; if *dark { e.1 += 1; }
                    if bvh.any_in_box([p[0] - 8.0, p[1] - 8.0, p[2] - 8.0], [p[0] + 8.0, p[1] + 8.0, p[2] + 8.0]) { e.2 += 1; }
                }
                for (l, (n, d, o)) in &per { println!("level {l:>2} (probe y {:>4}): {n:>6} texels dark {:>5.1}% box-overlap {:>5.1}%", -46.0 + 16.0 * (*l as f32 + 0.5), 100.0 * *d as f64 / *n as f64, 100.0 * *o as f64 / *n as f64); }
            }
            for &dy in &dys { for &sign in &[1.0f32, -1.0, 0.0, 4.0, 8.0] {
                let (mut tp, mut fp, mut fnn, mut tn) = (0usize, 0usize, 0usize, 0usize);
                for (p, dark) in &samples {
                    let o = [p[0], p[1] + dy, p[2]];
                    // sign 0/4/8: "any triangle within r m of the probe" instead (r = 0 → the 16-m cell box)
                    let inside = if sign == 0.0 || sign > 1.5 {
                        let r = if sign == 0.0 { 8.0 } else { sign };
                        bvh.any_in_box([o[0] - r, o[1] - r, o[2] - r], [o[0] + r, o[1] + r, o[2] + r])
                    } else { match bvh.closest(o, [0.0, 1.0, 0.0], 1000.0) {
                        Some(h) => { let t = &bvh.tris[h.tri as usize]; let n = lightmap::geometry::cross(t.e1, t.e2); sign * n[1] > 0.0 }
                        None => false,
                    } };
                    match (inside, *dark) { (true, true) => tp += 1, (true, false) => fp += 1, (false, true) => fnn += 1, (false, false) => tn += 1 }
                }
                let (tpf, fpf, fnf, tnf) = (tp as f64, fp as f64, fnn as f64, tn as f64);
                let den = ((tpf + fpf) * (tpf + fnf) * (tnf + fpf) * (tnf + fnf)).sqrt().max(1e-9);
                println!("dy {dy:>5} sign {sign:>3}: mcc {:.3}  tp {tp} fp {fp} fn {fnn} tn {tn}", (tpf * tnf - fpf * fnf) / den);
            }}
        }
        "volmosaic" => {
            // lmtool volmosaic MAP LEVEL OUT.ppm: the probe-volume slices of height level LEVEL (0..16 within
            // each block) assembled on one canvas: x = record axis 0 (cells), rows = strip (origin[1]/16) × 32 + axis 2
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let d = m.chunk.data.as_ref().unwrap();
            let v = lightmap::volume::Volume::parse(&d.cache.trailer).expect("trailer");
            let im = lightmap::img::decode_webp(&d.frames[0].images[2]).expect("atlas 2");
            let level: u32 = a[2].parse().unwrap();
            let strips = v.grid[1] / 16;
            let (cw, ch) = (v.grid[0], strips * 32);
            let sc = 4u32;
            let mut out = lightmap::img::Rgb::new(cw * sc, ch * sc);
            for p in out.px.iter_mut() { *p = 90; }
            for b in &v.blocks {
                let strip = b.origin[1] / 16;
                let c1 = b.origin[1] + level;
                if c1 < b.min[1] || c1 >= b.max[1] { continue; }
                let Some((tx, ty)) = b.slices[(c1 - b.min[1]) as usize] else {
                    // absent slice: mark the block's footprint dark grey
                    for z in b.min[2]..b.max[2] { for x in b.min[0]..b.max[0] {
                        for oy in 0..sc { for ox in 0..sc { out.set(x * sc + ox, (strip * 32 + z) * sc + oy, [40, 40, 40]); } }
                    }}
                    continue;
                };
                let (tw, th) = (b.max[0] - b.min[0], b.max[2] - b.min[2]);
                for y in 0..th { for x in 0..tw {
                    let c = im.get((tx + x).min(im.w - 1), (ty + y).min(im.h - 1));
                    let (cx, cy) = (b.min[0] + x, strip * 32 + b.min[2] + y);
                    for oy in 0..sc { for ox in 0..sc { out.set(cx * sc + ox, cy * sc + oy, c); } }
                }}
            }
            lightmap::img::write_ppm(&out, &a[3]).unwrap();
            println!("wrote {} ({}x{} cells, {} strips)", a[3], cw, ch, strips);
        }
        "geombox" => {
            // lmtool geombox MAP: world AABB of the item meshes grouped by pivot position (probe-map analysis)
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let mut groups: std::collections::BTreeMap<String, ([f32; 3], [f32; 3], usize)> = Default::default();
            for inst in &scene.instances {
                let md = &scene.models[inst.model];
                let piv = [inst.xf[9], inst.xf[10], inst.xf[11]];
                let key = format!("{:.0},{:.0},{:.0}", piv[0], piv[1], piv[2]);
                let e = groups.entry(key).or_insert(([f32::MAX; 3], [f32::MIN; 3], 0));
                e.2 += 1;
                for t in &md.tris { for p in &t.p { let w = lightmap::geometry::xf_point(&inst.xf, *p); for k in 0..3 { e.0[k] = e.0[k].min(w[k]); e.1[k] = e.1[k].max(w[k]); } } }
            }
            for (k, (lo, hi, n)) in &groups { println!("pivot {k}: {n} instances, mesh bbox [{:.1},{:.1},{:.1}]..[{:.1},{:.1},{:.1}]", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]); }
        }
        "volpaint" => {
            // lmtool volpaint MAP --out OUT [--img K --color r,g,b]... [--mask-all] [--vp8 Q]
            //   repaint probe image K (0 colour, 1 occlusion, 2 pale colour, 3 lights) with one colour;
            //   the blob is rebuilt as the concatenation of the 4 WEBPs with the trailer offsets updated
            let mut m = lightmap::mapio::load(&a[1]).expect("load");
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let q: u8 = f("--vp8").map(|s| s.parse().unwrap()).unwrap_or(8);
            let d = m.chunk.data.as_mut().expect("has lightmaps");
            let mut v = lightmap::volume::Volume::parse(&d.cache.trailer).expect("trailer");
            let mut parts = lightmap::volume::split_probe_blob(&d.frames[0].images[2], &v.frame_info);
            println!("probe blob: {} images: {:?}", parts.len(), parts.iter().map(|p| p.len()).collect::<Vec<_>>());
            let mut i = 0;
            while i < a.len() {
                if a[i] == "--img" {
                    let k: usize = a[i + 1].parse().unwrap();
                    let col: Vec<u8> = a[i + 3].split(',').map(|x| x.trim().parse().unwrap()).collect();
                    let mut im = lightmap::img::decode_webp(&parts[k]).expect("decode part");
                    for p in im.px.chunks_mut(3) { p.copy_from_slice(&col[..3]); }
                    parts[k] = lightmap::vp8enc::encode(&im.px, im.w, im.h, q);
                    println!("image {k} -> {:?} ({} bytes)", col, parts[k].len());
                    i += 4;
                } else if a[i] == "--reencode" {
                    for (k, p) in parts.iter_mut().enumerate() { let im = lightmap::img::decode_webp(p).expect("decode"); *p = lightmap::vp8enc::encode(&im.px, im.w, im.h, q); println!("image {k} re-encoded ({} bytes)", p.len()); }
                    i += 1;
                } else { i += 1; }
            }
            if a.iter().any(|x| x == "--mask-all") { for c in v.cell4.iter_mut() { *c = 0xffff; } }
            let (blob, ends) = lightmap::volume::join_probe_blob(&parts);
            for (k, e) in ends.iter().enumerate() { if k < v.frame_info.len() { v.frame_info[k].1 = *e; } }
            d.frames[0].images[2] = blob;
            d.cache.trailer = v.write();
            let payload = m.chunk.write(true);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&m, &payload, &out).expect("save");
            println!("wrote {out}");
        }
        "lights" => {
            // lmtool lights MAP: every embedded model's CPlugLights (socket transform, colour, intensity, radius, spot angles)
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let files = mapgeom::embedded::files(&m).expect("embedded");
            let mut per_model: std::collections::BTreeMap<String, usize> = Default::default();
            for it in &m.items { *per_model.entry(it.model.clone()).or_insert(0) += 1; }
            let mut total = 0usize;
            for (k, bytes) in &files {
                let base = k.rsplit(['/', '\\']).next().unwrap_or(k).to_string();
                let Ok(f) = mapgeom::static_item::file::parse_file(bytes) else { continue };
                let Some(s2) = f.item.static_object().and_then(|so| so.solid2()) else { continue };
                if s2.lights.is_empty() && s2.light_insts.is_empty() { continue; }
                let n = per_model.get(&base).copied().unwrap_or(0);
                println!("{base} ({n} placements): {} lights, {} user models, {} insts", s2.lights.len(), s2.light_user_models.len(), s2.light_insts.len());
                for l in &s2.lights {
                    let t = &l.u05;
                    let mut desc = String::new();
                    if let Some(mapgeom::static_item::Node::Light(pl)) = l.node.inline.as_deref() {
                        if let Some(g) = pl.gx_light() {
                            let (c, i, r) = g.summary();
                            desc = format!("class {:#x} colour ({:.2},{:.2},{:.2}) intensity {i:.2} radius {r:.1}", g.class_id, c[0], c[1], c[2]);
                            for ch in &g.chunks {
                                if let mapgeom::static_item::light::GxChunk::Spot { angle_inner, angle_outer, falloff_exponent, .. } = ch { desc.push_str(&format!(" spot inner {angle_inner:.2} outer {angle_outer:.2} falloff {falloff_exponent:.2}")); }
                                if let mapgeom::static_item::light::GxChunk::Spot01 { angle_inner, angle_outer, falloff_exponent, .. } = ch { desc.push_str(&format!(" spot01 inner {angle_inner:.2} outer {angle_outer:.2} falloff {falloff_exponent:.2}")); }
                                if let mapgeom::static_item::light::GxChunk::Ball08 { radius, emitting_radius, .. } = ch { desc.push_str(&format!(" ball08 r {radius:.1} emit {emitting_radius:.1}")); }
                                if let mapgeom::static_item::light::GxChunk::Ball06 { radius, emitting_radius, attenuation, .. } = ch { desc.push_str(&format!(" ball06 r {radius:.1} emit {emitting_radius:.1} att {attenuation:?}")); }
                                if let mapgeom::static_item::light::GxChunk::Light0A { diffuse_intensity, .. } = ch { desc.push_str(&format!(" diffuse {diffuse_intensity:.2}")); }
                                if let mapgeom::static_item::light::GxChunk::Light09 { diffuse_intensity, .. } = ch { desc.push_str(&format!(" diffuse {diffuse_intensity:.2}")); }
                            }
                            if pl.is_animated() { desc.push_str(" ANIMATED"); }
                        }
                    } else { desc = format!("external/string {:?} node idx {}", l.u04, l.node.index); }
                    println!("   {}: pos ({:.2},{:.2},{:.2}) fwd ({:.2},{:.2},{:.2}) up ({:.2},{:.2},{:.2}) ints {:?} {desc}", l.u01.as_str().unwrap_or("?"), t[9], t[10], t[11], t[6], t[7], t[8], t[3], t[4], t[5], l.ints);
                    total += 1;
                }
            }
            println!("{total} light sockets in models with lights");
        }
        "lightfit" => {
            // lmtool lightfit MAP [--items N] [--step S]: the editor's frame-1 (point light) atlas against our light
            // list — per texel (through our uv rasterisation of the lit charts) the sum over lights of
            // I·c·n·l·vis·spot·att(d/R) for several falloff laws, least-squares scale k and r² per law,
            // plus a binned profile of ref / (I·c·ndl·spot) against d/R for single-light texels.
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let lights = scene.world_lights();
            eprintln!("{} lights in the scene", lights.len());
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let i1 = lightmap::img::decode_webp(&d.frames[1].images[0]).unwrap();
            let f1_max = d.cache.frame_max_hdr_n(1).unwrap_or(1.0);
            eprintln!("frame 1 MaxHDR {f1_max:.4} (frame 0 {:.4})", d.cache.frame_max_hdr().unwrap_or(0.0));
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let laws: Vec<(&str, Box<dyn Fn(f64) -> f64 + Sync>)> = vec![
                ("(1-x²)²", Box::new(|x: f64| (1.0 - x * x).max(0.0).powi(2))),
                ("(1-x)²", Box::new(|x: f64| (1.0 - x).max(0.0).powi(2))),
                ("1-x", Box::new(|x: f64| (1.0 - x).max(0.0))),
                ("(1-x²)", Box::new(|x: f64| (1.0 - x * x).max(0.0))),
                ("(1-x²)²/(1+16x²)", Box::new(|x: f64| (1.0 - x * x).max(0.0).powi(2) / (1.0 + 16.0 * x * x))),
                ("(1-x²)²/x²", Box::new(|x: f64| (1.0 - x * x).max(0.0).powi(2) / (x * x).max(0.0025))),
                ("(1-x⁴)/(1+4x²)", Box::new(|x: f64| (1.0 - x.powi(4)).max(0.0) / (1.0 + 4.0 * x * x))),
                ("(1-x)⁴", Box::new(|x: f64| (1.0 - x).max(0.0).powi(4))),
            ];
            let cone_mul: f32 = if a.iter().any(|x| x == "--cone-half") { 1.0 } else { 0.5 };
            let spot = |l: &lightmap::geometry::LightDef, to_tex: [f32; 3]| -> f64 {
                let ang = lightmap::geometry::dot(l.dir, to_tex).clamp(-1.0, 1.0).acos().to_degrees();
                let (hi, ho) = (l.cone.0 * cone_mul, l.cone.1 * cone_mul);
                if ang <= hi { 1.0 } else if ang >= ho { 0.0 } else { let t = ((ho - ang) / (ho - hi).max(1e-3)) as f64; t * t * (3.0 - 2.0 * t) }
            };
            let nitems: usize = f("--items").map(|s| s.parse().unwrap()).unwrap_or(1500);
            let step: usize = f("--step").map(|s| s.parse().unwrap()).unwrap_or(3);
            let sel: Vec<(usize, usize)> = scene.instances.iter().enumerate().filter_map(|(ii, inst)| { let &ci = chart_of.get(&(inst.item as u32))?; if mp.frame_bytes[1][ci] == 0 { return None; } let (w, h) = mp.size[ci]; if w < 6 || h < 6 { return None; } Some((ii, ci)) }).collect();
            let stride = (sel.len() / nitems).max(1);
            let sel: Vec<(usize, usize)> = sel.into_iter().step_by(stride).collect();
            eprintln!("{} lit charts sampled", sel.len());
            // rows: (ref, single-light (d/R, ndl·spot) or None, per-law sums)
            let rows = std::sync::Mutex::new(Vec::<(f64, Option<(f64, f64)>, Vec<f64>)>::new());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
            std::thread::scope(|sc| { for _ in 0..threads { sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() { break; }
                let (ii, ci) = sel[k];
                let fb1 = mp.frame_bytes[1][ci];
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, w as u32 / 2, h as u32 / 2);
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                let mut local = Vec::new();
                for s in samples.iter().step_by(step) {
                    let o = lightmap::geometry::add(s.p, lightmap::geometry::mul(s.n, 0.03));
                    let mut sums = vec![0f64; laws.len()];
                    let mut single: Vec<(f64, f64)> = Vec::new();
                    for (li, l) in &lights {
                        let to = lightmap::geometry::sub(l.pos, o);
                        let dist = lightmap::geometry::dot(to, to).sqrt();
                        if dist >= l.radius || dist < 0.05 { continue; }
                        let ldir = lightmap::geometry::mul(to, 1.0 / dist);
                        let ndl = lightmap::geometry::dot(s.n, ldir);
                        if ndl <= 0.0 { continue; }
                        let sp = spot(l, lightmap::geometry::mul(ldir, -1.0));
                        if sp <= 0.0 { continue; }
                        // shadow ray from the light towards the texel, ignoring the lamp's own item near the light
                        let back = lightmap::geometry::mul(ldir, -1.0);
                        if !a.iter().any(|x| x == "--no-shadow") && bvh.occluded(l.pos, back, dist - 0.08, *li as u32, 1.5) { continue; }
                        let clum = 0.2126 * l.color[0] + 0.7152 * l.color[1] + 0.0722 * l.color[2];
                        let base_term = (l.intensity * clum * ndl) as f64 * sp;
                        let xr = (dist / l.radius) as f64;
                        for (j, (_, law)) in laws.iter().enumerate() { sums[j] += base_term * law(xr); }
                        single.push((xr, base_term));
                    }
                    let c = i1.get((px + s.px).min(i1.w - 1), (py + s.py).min(i1.h - 1));
                    // sqrt-encoded like frame 0; absolute with frame 1's own MaxHDR
                    let linear = std::env::var("LMTOOL_LINEAR").is_ok();
                    let dv = |p: u8| if linear { p as f64 / 255.0 * fb1 as f64 / 255.0 } else { lightmap::synth::decode_value(p, fb1) as f64 * f1_max as f64 };
                    let lum = 0.2126 * dv(c[0]) + 0.7152 * dv(c[1]) + 0.0722 * dv(c[2]);
                    let sg = if single.len() == 1 { Some(single[0]) } else { None };
                    local.push((lum, sg, sums));
                }
                rows.lock().unwrap().extend(local);
            }); } });
            let rows = rows.into_inner().unwrap();
            println!("{} texels ({} with exactly one light in range)", rows.len(), rows.iter().filter(|r| r.1.is_some()).count());
            let mut bins = vec![(0f64, 0usize); 20];
            for (v, sg, _) in &rows { if let Some((xr, bt)) = sg { if *bt > 1e-4 { let b = ((xr * 20.0) as usize).min(19); bins[b].0 += v / bt; bins[b].1 += 1; } } }
            for (b, (s, n)) in bins.iter().enumerate() { if *n > 0 { println!("d/R {:.2}-{:.2}: n={n:>6} mean ref/(I·c·ndl·spot) = {:.4}", b as f32 / 20.0, (b + 1) as f32 / 20.0, s / *n as f64); } }
            let my = rows.iter().map(|r| r.0).sum::<f64>() / rows.len().max(1) as f64;
            {
                // robust view: the ratio ref/model for the (1-x²)² law, its quartiles; texels the model misses and vice versa
                let mut ratios: Vec<f64> = rows.iter().filter(|r| r.2[0] > 1e-4 && r.0 > 1e-4).map(|r| r.0 / r.2[0]).collect();
                ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let q = |f: f64| ratios.get(((ratios.len() as f64 - 1.0) * f) as usize).copied().unwrap_or(0.0);
                let ref_only = rows.iter().filter(|r| r.2[0] <= 1e-4 && r.0 > 0.02).count();
                let model_only = rows.iter().filter(|r| r.2[0] > 1e-4 && r.0 <= 1e-4).count();
                let max_model = rows.iter().map(|r| r.2[0]).fold(0.0, f64::max);
                let max_ref = rows.iter().map(|r| r.0).fold(0.0, f64::max);
                println!("ratio ref/model (law (1-x²)²) over {} texels: p10 {:.4} p25 {:.4} median {:.4} p75 {:.4} p90 {:.4}; ref>0.02 with no model light: {ref_only}; model>0 with ref 0: {model_only}; max model {max_model:.2} max ref {max_ref:.3}", ratios.len(), q(0.1), q(0.25), q(0.5), q(0.75), q(0.9));
            }
            for (j, (name, _)) in laws.iter().enumerate() {
                let (mut sxy, mut sxx, mut ssr, mut sst) = (0.0, 0.0, 0.0, 0.0);
                for (v, _, sums) in &rows { sxy += sums[j] * v; sxx += sums[j] * sums[j]; }
                let k = sxy / sxx.max(1e-12);
                for (v, _, sums) in &rows { let p = k * sums[j]; ssr += (v - p) * (v - p); sst += (v - my) * (v - my); }
                println!("law {name:>20}: k = {k:.5}  r² = {:.3}", 1.0 - ssr / sst.max(1e-12));
            }
        }
        "lightpools" => {
            // lmtool lightpools MAP CHART: the bright frame-1 texels of one chart (world positions) beside the lights within 40 m
            let base: u32 = 4096;
            let ci: usize = a[2].parse().unwrap();
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let i1 = lightmap::img::decode_webp(&d.frames[1].images[0]).unwrap();
            let item = (mp.binds[ci].obj_group_idx / 4 - base) as usize;
            let ii = scene.instances.iter().position(|i| i.item == item).expect("instance");
            let inst = &scene.instances[ii];
            println!("chart {ci} = item {item} model {} at {:?}", inst.model_name, [inst.xf[9], inst.xf[10], inst.xf[11]]);
            let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
            let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, w as u32 / 2, h as u32 / 2);
            let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
            let mut bright: Vec<(u8, [f32; 3], [f32; 3], u32, u32)> = Vec::new();
            for s in &samples {
                let c = i1.get((px + s.px).min(i1.w - 1), (py + s.py).min(i1.h - 1));
                bright.push((c[1], s.p, s.n, s.px, s.py));
            }
            bright.sort_by_key(|b| std::cmp::Reverse(b.0));
            println!("brightest frame-1 texels (value, world pos, normal, px, py):");
            for b in bright.iter().take(12) { println!("  {:>3} ({:.1},{:.1},{:.1}) n ({:.2},{:.2},{:.2}) px ({},{})", b.0, b.1[0], b.1[1], b.1[2], b.2[0], b.2[1], b.2[2], b.3, b.4); }
            let cen = { let mut s = [0f32; 3]; for b in &bright { for k in 0..3 { s[k] += b.1[k]; } } [s[0] / bright.len() as f32, s[1] / bright.len() as f32, s[2] / bright.len() as f32] };
            println!("chart centroid ({:.1},{:.1},{:.1}); lights within 40 m:", cen[0], cen[1], cen[2]);
            for (li, l) in scene.world_lights() {
                let dd = lightmap::geometry::sub(l.pos, cen);
                let dist = lightmap::geometry::dot(dd, dd).sqrt();
                if dist < 40.0 { println!("  inst {li} ({}) pos ({:.1},{:.1},{:.1}) dir ({:.2},{:.2},{:.2}) R {:.1} I {:.2} cone {:?} dist {dist:.1}", scene.instances[li].model_name, l.pos[0], l.pos[1], l.pos[2], l.dir[0], l.dir[1], l.dir[2], l.radius, l.intensity, l.cone); }
            }
        }
        "ambfit" => {
            // lmtool ambfit MAP [--items N] [--sun-az A --sun-el E]: the RE model — E = LA·(0.8+0.2n.y) + S·skyVis + b·bounce
            // with LA = the effective mood's LAmbient (XML), no direct sun (it only feeds the bounce); fits S and b per
            // channel plus a constant against the map's own (sqrt-decoded, MaxHDR-scaled) editor bake
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let hdr = tmmaps::header::read(&a[1]).expect("header");
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let dt = lightmap::mapio::daytime(&own.gbx.body);
            let mood = lightmap::moods::effective_mood(&mf.decoration_id, dt);
            let xml = lightmap::moods::mood_xml(&hdr.envir, mood).expect("mood xml");
            let frame_max = d.cache.frame_max_hdr().unwrap_or(1.0);
            println!("{}: decoration {} daytime {:?} → mood {} ; XML LAmbient {:?} MaxHDR {} Bounce {} Sky {} ; frame MaxHDR {:.4}", a[1].rsplit('/').next().unwrap(), mf.decoration_id, dt.filter(|t| *t != 0xffff_ffff).map(|t| t as f64 / 65536.0), mood, xml.l_ambient, xml.max_hdr, xml.bounce_factor, xml.sky_factor, frame_max);
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let step = (scene.instances.len() / f("--items").map(|s| s.parse().unwrap()).unwrap_or(1500)).max(1);
            let fsel: Vec<(usize, u32, u32, u32, u32, u8)> = scene.instances.iter().enumerate().step_by(step).filter_map(|(ii, inst)| {
                let &ci = chart_of.get(&(inst.item as u32))?;
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (pw, ph) = (w as u32 / 2, h as u32 / 2);
                if pw < 2 || ph < 2 { return None; }
                Some((ii, (x as u32 + 1) / 2, (y as u32 + 1) / 2, pw, ph, mp.frame_bytes[0][ci]))
            }).collect();
            let mut prm = lightmap::bake::BakeParams::default();
            prm.uv_bounds = true; prm.sky_samples = 32; prm.sun_samples = 1; prm.sky_model = 0; prm.direct_sun = 0.0;
            prm.ambient_la = xml.l_ambient; prm.sun = xml.l_dir_sun; prm.sky = [1.0; 3]; prm.want_bounce = true;
            let (az, el) = (f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(77.5f32), f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(45.0f32));
            let (ar, er) = (az.to_radians(), el.to_radians());
            prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
            // the fit: regressors [skyVis, (unused sun) 0, bounce, 1]; target = decoded value × frame MaxHDR − LA·(0.8+0.2n.y)
            let (rgb, r2) = lightmap::bake::component_fit_rgb2(&scene, &bvh, &fsel, &ia, &prm, frame_max);
            println!("fit r² {r2:.3}: sky S = ({:.3}, {:.3}, {:.3})  ambient A·(0.8+0.2n.y) with A = ({:.3}, {:.3}, {:.3}) [XML LA ({:.3}, {:.3}, {:.3})]  bounce b = ({:.3}, {:.3}, {:.3})  const = ({:.3}, {:.3}, {:.3})", rgb[0][0], rgb[1][0], rgb[2][0], rgb[0][1], rgb[1][1], rgb[2][1], xml.l_ambient[0], xml.l_ambient[1], xml.l_ambient[2], rgb[0][2], rgb[1][2], rgb[2][2], rgb[0][3], rgb[1][3], rgb[2][3]);
        }
        "domefit" => {
            // lmtool domefit MAP [--base N] [--items N] [--cone-deg A] [--sun-az A --sun-el E]: fit the dome model's
            // three gains against the map's own editor bake — per channel, E = S·skyFrac + K·(n·L)·sunVis + B·bounce₁
            // (+ c), with bounce₁ the one-bounce estimate at unit albedo·BounceFactor; prints S/LAmbient, K/LDirSun, B, r²
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let fm = d.cache.frame_max_hdr().unwrap_or(1.0);
            let hdr = tmmaps::header::read(&a[1]).expect("header");
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let dt = lightmap::mapio::daytime(&own.gbx.body);
            let mood = lightmap::moods::effective_mood(&mf.decoration_id, dt);
            let xml = lightmap::moods::mood_xml(&hdr.envir, mood).expect("mood xml");
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let mut prm = lightmap::bake::BakeParams::default();
            prm.uv_bounds = true; prm.sky_samples = 64; prm.sun_samples = 4; prm.direct_sun = 1.0;
            prm.dome_deg = f("--cone-deg").map(|s| s.parse().unwrap()).unwrap_or(25.0);
            prm.sky = [1.0; 3]; prm.sun = [1.0; 3]; prm.bounce = 1.0; prm.albedo = 1.0; prm.ground_bounce = 0.0; prm.ground_y = 8.0;
            prm.bounce_sphere = !a.iter().any(|x| x == "--bounce-cone");
            prm.sun_radius = 2.0f32.to_radians();
            let (az, el) = (f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(115.0f32), f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(2.0f32));
            let (ar, er) = (az.to_radians(), el.to_radians());
            prm.sun_dir = [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()];
            let step = (scene.instances.len() / f("--items").map(|s| s.parse().unwrap()).unwrap_or(1500)).max(1);
            let sel: Vec<(usize, usize)> = scene.instances.iter().enumerate().step_by(step).filter_map(|(ii, inst)| { let &ci = chart_of.get(&(inst.item as u32))?; let (w, h) = mp.size[ci]; if w < 4 || h < 4 || mp.frame_bytes[0][ci] == 0 { return None; } Some((ii, ci)) }).collect();
            // rows: [skyFrac, ndl·sunVis, bounce lum, target r, g, b, bounce r, g, b]
            let rows = std::sync::Mutex::new(Vec::<[f64; 9]>::new());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(8).min(160);
            std::thread::scope(|sc| { for _ in 0..threads { sc.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if k >= sel.len() { break; }
                let (ii, ci) = sel[k];
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, w as u32 / 2, h as u32 / 2);
                let fb = mp.frame_bytes[0][ci];
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                let mut local = Vec::new();
                for s in samples.iter().step_by(2) {
                    let sh = lightmap::bake::shade_point_inst(&scene, &bvh, &prm, s.p, s.n, ii as u32);
                    let ndl = lightmap::geometry::dot(s.n, prm.sun_dir).max(0.0);
                    let c = ia.get((px + s.px).min(ia.w - 1), (py + s.py).min(ia.h - 1));
                    let t = |k: usize| lightmap::synth::decode_value(c[k], fb) as f64 * fm as f64;
                    local.push([sh.sky_rgb[0] as f64, (ndl * sh.sun_vis) as f64, 0.0, t(0), t(1), t(2), sh.bounce[0] as f64, sh.bounce[1] as f64, sh.bounce[2] as f64]);
                }
                rows.lock().unwrap().extend(local);
            }); } });
            let v = rows.into_inner().unwrap();
            println!("{}: mood {mood}, frame MaxHDR {fm:.4}, {} texels; sun az {az} el {el}, cone {}°, bounce {}", a[1].rsplit('/').next().unwrap(), v.len(), prm.dome_deg, if prm.bounce_sphere { "sphere" } else { "cone" });
            let names = ["r", "g", "b"];
            for ch in 0..3 {
                let mut mtx = [[0f64; 4]; 4]; let mut r = [0f64; 4];
                for p in &v { let x = [p[0], p[1], p[6 + ch], 1.0]; for i in 0..4 { for j in 0..4 { mtx[i][j] += x[i] * x[j]; } r[i] += x[i] * p[3 + ch]; } }
                let Some(coef) = lightmap::bake::solve4_pub(mtx, r) else { continue };
                let mean = v.iter().map(|p| p[3 + ch]).sum::<f64>() / v.len() as f64;
                let (mut ssr, mut sst) = (0.0, 0.0);
                for p in &v { let pred = coef[0] * p[0] + coef[1] * p[1] + coef[2] * p[6 + ch] + coef[3]; ssr += (p[3 + ch] - pred).powi(2); sst += (p[3 + ch] - mean).powi(2); }
                println!("  {}: S {:.3} (= {:.2} × LAmbient {:.3})  K_sun {:.3} (= {:.2} × LDirSun {:.3})  B {:.3}  c {:.3}   r² {:.3}", names[ch], coef[0], coef[0] / xml.l_ambient[ch].max(1e-6) as f64, xml.l_ambient[ch], coef[1], coef[1] / xml.l_dir_sun[ch].max(1e-6) as f64, xml.l_dir_sun[ch], coef[2], coef[3], 1.0 - ssr / sst.max(1e-12));
            }
        }
        "moodfit" => {
            // lmtool moodfit MAP [--items N] [--fine]: the sun direction (per-texel correlation over the largest
            // charts, coarse grid then a fine grid around the best) and the per-channel component fit
            // (ambient, upness, sky, sun) of the map's own editor bake — one line of bake flags out.
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let t0 = std::time::Instant::now();
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let nitems: usize = f("--items").map(|s| s.parse().unwrap()).unwrap_or(400);
            // --flat: only charts whose triangles are mostly horizontal (roads, platforms): the shadow
            // edges on those carry the direction; the terrain charts mostly add noise
            let flat = a.iter().any(|x| x == "--flat");
            let mut cand: Vec<(usize, u32)> = scene.instances.iter().enumerate().filter_map(|(ii, inst)| {
                let &ci = chart_of.get(&(inst.item as u32))?;
                if flat {
                    let m = &scene.models[inst.model];
                    let up = m.tris.iter().filter(|t| t.n[0][1].abs() > 0.9).count();
                    if up * 5 < m.tris.len() * 4 { return None; }
                }
                Some((ii, mp.size[ci].0 as u32 * mp.size[ci].1 as u32))
            }).collect();
            cand.sort_by_key(|c| std::cmp::Reverse(c.1));
            let sel: Vec<(usize, u32, u32, u32, u32)> = cand.iter().take(nitems).filter_map(|&(ii, _)| {
                let ci = chart_of[&(scene.instances[ii].item as u32)];
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (pw, ph) = (w as u32 / 2, h as u32 / 2);
                if pw < 4 || ph < 4 { return None; }
                Some((ii, (x as u32 + 1) / 2, (y as u32 + 1) / 2, pw, ph))
            }).collect();
            let mut prm = lightmap::bake::BakeParams::default();
            prm.uv_bounds = true; prm.sky_samples = 16; prm.sun_samples = 1; prm.sky_model = 1;
            if let Some(p) = f("--sky-cube") { prm.sky_cube = Some(std::sync::Arc::new(lightmap::skycube::CubeMap::load(&p).expect("sky cube"))); }
            let no_sun = a.iter().any(|x| x == "--no-sun");
            let dir = |az: f32, el: f32| -> [f32; 3] { let (ar, er) = (az.to_radians(), el.to_radians()); [er.cos() * ar.sin(), er.sin(), er.cos() * ar.cos()] };
            let mut best = (f64::MIN, 0.0f32, 0.0f32);
            let mut grid: Vec<(f32, f32)> = Vec::new();
            for el in [10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0] { for i in 0..24 { grid.push((i as f32 * 15.0, el)); } }
            // objective: --shadow = lit/shadowed agreement on bimodal charts (robust to blur and scale); default = texel correlation
            let shadow = a.iter().any(|x| x == "--shadow");
            let score = |prm: &lightmap::bake::BakeParams| -> f64 { if shadow { lightmap::bake::shadow_agreement(&scene, &bvh, &sel, &ia, prm).0 } else { lightmap::bake::texel_correlation(&scene, &bvh, &sel, &ia, prm).0 } };
            if no_sun { grid.clear(); best = (0.0, f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(0.0), f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(45.0)); }
            for &(az, el) in &grid {
                prm.sun_dir = dir(az, el);
                let c = score(&prm);
                if c > best.0 { best = (c, az, el); }
            }
            eprintln!("coarse: az {} el {} score {:.4} ({:.0}s)", best.1, best.2, best.0, t0.elapsed().as_secs_f32());
            let (caz, cel) = (best.1, best.2);
            for del in if no_sun { vec![] } else { vec![-7.5f32, -5.0, -2.5, 0.0, 2.5, 5.0, 7.5] } { for daz in [-10.0f32, -7.5, -5.0, -2.5, 0.0, 2.5, 5.0, 7.5, 10.0] {
                let (az, el) = (caz + daz, (cel + del).clamp(2.0, 88.0));
                prm.sun_dir = dir(az, el);
                let c = score(&prm);
                if c > best.0 { best = (c, az, el); }
            }}
            if shadow { let (_, n) = lightmap::bake::shadow_agreement(&scene, &bvh, &sel, &ia, &prm); println!("sun: az {:.1} el {:.1} (shadow agreement {:.4} over {} texels)", best.1, best.2, best.0, n); } else { println!("sun: az {:.1} el {:.1} (texel corr {:.4})", best.1, best.2, best.0); }
            // component fit on a broad sample
            let step = (scene.instances.len() / 1500).max(1);
            let fsel: Vec<(usize, u32, u32, u32, u32, u8)> = scene.instances.iter().enumerate().step_by(step).filter_map(|(ii, inst)| {
                let &ci = chart_of.get(&(inst.item as u32))?;
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (pw, ph) = (w as u32 / 2, h as u32 / 2);
                if pw < 2 || ph < 2 { return None; }
                Some((ii, (x as u32 + 1) / 2, (y as u32 + 1) / 2, pw, ph, mp.frame_bytes[0][ci]))
            }).collect();
            prm.sun_dir = dir(best.1, best.2); prm.sky_samples = 32; prm.fit_regressor = 1;
            let (rgb, r2) = lightmap::bake::component_fit_rgb(&scene, &bvh, &fsel, &ia, &prm);
            // rgb[ch] = [sky, sun, upness, const]
            let cl = |k: usize| format!("{:.3},{:.3},{:.3}", rgb[0][k].max(0.0), rgb[1][k].max(0.0), rgb[2][k].max(0.0));
            if prm.sky_cube.is_some() { println!("fit r² {r2:.3} WITH THE SKY CUBE: sky-scale per channel {} (1.0 = the cube's E/π as is), sun {}, up {}, const {}", cl(0), cl(1), cl(2), cl(3)); }
            println!("fit r² {r2:.3}: --sun-az {:.1} --sun-el {:.1} --ambient {} --up {} --sky {} --sun {}", best.1, best.2, cl(3), cl(2), cl(0), cl(1));
            println!("raw per channel: sky {:?} sun {:?} up {:?} const {:?}", [rgb[0][0], rgb[1][0], rgb[2][0]], [rgb[0][1], rgb[1][1], rgb[2][1]], [rgb[0][2], rgb[1][2], rgb[2][2]], [rgb[0][3], rgb[1][3], rgb[2][3]]);
            eprintln!("moodfit done ({:.0}s)", t0.elapsed().as_secs_f32());
        }
        "framecmp" => {
            // lmtool framecmp REF.ppm OURS.ppm OUTBASE [--frames N]: per stacked frame (equal heights), the
            // luminance RMSE and mean abs diff (HUD strip skipped), the max-diff spot, and a sheet
            // OUTBASE-cmp.ppm: [ref | ours | |diff|×4] per frame plus a 160×120 crop around the max diff ×4
            let load = |p: &str| -> (usize, usize, Vec<u8>) {
                let data = std::fs::read(p).expect("read ppm");
                let mut idx = 0; let mut fields = Vec::new();
                while fields.len() < 4 { let s = idx; while data[idx] != b' ' && data[idx] != b'\n' { idx += 1; } fields.push(std::str::from_utf8(&data[s..idx]).unwrap().to_string()); idx += 1; }
                (fields[1].parse().unwrap(), fields[2].parse().unwrap(), data[idx..].to_vec())
            };
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let (w, h, pa) = load(&a[1]);
            let (w2, h2, pb) = load(&a[2]);
            assert!(w == w2 && h == h2, "sheets differ in size");
            let nframes: usize = f("--frames").map(|s| s.parse().unwrap()).unwrap_or(3);
            let fh = h / nframes;
            let lum = |p: &[u8], i: usize| 0.2126 * p[i] as f64 + 0.7152 * p[i + 1] as f64 + 0.0722 * p[i + 2] as f64;
            let cw = 160usize; let ch = 120usize; let sc = 4usize;
            let out_w = w * 3 + cw * sc * 2 + 8;
            let out_h = fh * nframes;
            let mut out = lightmap::img::Rgb::new(out_w as u32, out_h as u32);
            // --register: the intro camera drifts a few pixels between two loads; search the integer shift
            // (±8 px) of OURS that minimises the RMSE and report the registered numbers beside the raw ones
            let register = a.iter().any(|x| x == "--register");
            println!("frame\tRMSE(lum)\tmeanAbs\tmaxDiff\tat(x,y)\tmeanLumRef\tmeanLumOurs\tshift\tRMSE(registered)");
            for fr in 0..nframes {
                let y0 = fr * fh + fh / 8; // skip the HUD strip
                let (mut se, mut sa, mut n, mut mr, mut mo) = (0f64, 0f64, 0usize, 0f64, 0f64);
                let mut best = (0f64, 0usize, 0usize);
                for y in y0..(fr + 1) * fh { for x in 0..w {
                    let i = (y * w + x) * 3;
                    let (la, lb) = (lum(&pa, i), lum(&pb, i));
                    let d = lb - la;
                    se += d * d; sa += d.abs(); n += 1; mr += la; mo += lb;
                    if d.abs() > best.0 { best = (d.abs(), x, y); }
                }}
                let rmse = (se / n as f64).sqrt();
                let mut reg = (rmse, 0i32, 0i32);
                if register {
                    for dy in -8i32..=8 { for dx in -8i32..=8 {
                        let (mut se2, mut n2) = (0f64, 0usize);
                        let mut yy = y0 + 8; while yy < (fr + 1) * fh - 8 { let mut xx = 8usize; while xx < w - 8 {
                            let i = (yy * w + xx) * 3;
                            let j = (((yy as i32 + dy) as usize) * w + (xx as i32 + dx) as usize) * 3;
                            let d = lum(&pb, j) - lum(&pa, i);
                            se2 += d * d; n2 += 1; xx += 2; } yy += 2; }
                        let r = (se2 / n2 as f64).sqrt();
                        if r < reg.0 { reg = (r, dx, dy); }
                    }}
                }
                println!("{fr}\t{rmse:.2}\t{:.2}\t{:.0}\t({},{})\t{:.1}\t{:.1}\t({},{})\t{:.2}", sa / n as f64, best.0, best.1, best.2, mr / n as f64, mo / n as f64, reg.1, reg.2, reg.0);
                for y in fr * fh..(fr + 1) * fh { for x in 0..w {
                    let i = (y * w + x) * 3;
                    out.set(x as u32, y as u32, [pa[i], pa[i + 1], pa[i + 2]]);
                    out.set((w + x) as u32, y as u32, [pb[i], pb[i + 1], pb[i + 2]]);
                    let d = ((lum(&pb, i) - lum(&pa, i)) * 4.0).clamp(-255.0, 255.0);
                    let c = if d >= 0.0 { [d as u8, d as u8 / 3, 0] } else { [0, (-d) as u8 / 3, (-d) as u8] };
                    out.set((2 * w + x) as u32, y as u32, c);
                }}
                // crops around the max-diff spot
                let (cx, cy) = (best.1.clamp(cw / 2, w - cw / 2), best.2.clamp(fr * fh + ch / 2, (fr + 1) * fh - ch / 2));
                for yy in 0..ch * sc { for xx in 0..cw * sc {
                    let sx = cx - cw / 2 + xx / sc; let sy = cy - ch / 2 + yy / sc;
                    let i = (sy * w + sx) * 3;
                    let oy = fr * fh + yy; if oy >= (fr + 1) * fh { continue; }
                    out.set((3 * w + 4 + xx) as u32, oy as u32, [pa[i], pa[i + 1], pa[i + 2]]);
                    out.set((3 * w + 8 + cw * sc + xx) as u32, oy as u32, [pb[i], pb[i + 1], pb[i + 2]]);
                }}
            }
            lightmap::img::write_ppm(&out, &format!("{}-cmp.ppm", a[3])).unwrap();
            println!("wrote {}-cmp.ppm ({}x{})", a[3], out_w, out_h);
        }
        "uvcover" => {
            // lmtool uvcover MAP [--top N]: per model (the N with most triangles), the fraction of the uv1 square
            // (or of the PreLightGen bounds) its triangles cover at 64×64 — the chart texels that carry geometry
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let top: usize = f("--top").map(|s| s.parse().unwrap()).unwrap_or(12);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let mut order: Vec<usize> = (0..scene.models.len()).collect();
            order.sort_by_key(|&m| std::cmp::Reverse(scene.models[m].tris.len()));
            let (mut tot_cov, mut tot_n) = (0f64, 0usize);
            for &mi in order.iter().take(top) {
                let Some(ii) = scene.instances.iter().position(|i| i.model == mi) else { continue };
                let (_, cov) = lightmap::bake::rasterise_pub(&scene, ii, 64, 64, false, true);
                let c = cov.iter().filter(|&&b| b).count() as f64 / cov.len() as f64;
                tot_cov += c; tot_n += 1;
                println!("{:<24} {:>7} tris  uv1-bounds coverage {:>5.1}%  (m/uv {:.1})", scene.model_names[mi], scene.models[mi].tris.len(), 100.0 * c, scene.models[mi].metres_per_uv);
            }
            // all models, weighted by placements
            let (mut wsum, mut wcov) = (0f64, 0f64);
            for (mi, m) in scene.models.iter().enumerate() {
                if m.tris.is_empty() { continue; }
                let Some(ii) = scene.instances.iter().position(|i| i.model == mi) else { continue };
                let n = scene.instances.iter().filter(|i| i.model == mi).count() as f64;
                let (_, cov) = lightmap::bake::rasterise_pub(&scene, ii, 32, 32, false, true);
                let c = cov.iter().filter(|&&b| b).count() as f64 / cov.len() as f64;
                wsum += n; wcov += n * c;
            }
            println!("top {tot_n} models: mean coverage {:.1}%; all placements (32×32): mean coverage {:.1}%", 100.0 * tot_cov / tot_n.max(1) as f64, 100.0 * wcov / wsum.max(1.0));
        }
        "mapinfo" => {
            // lmtool mapinfo MAP...: size words, decoration, block/item counts, item extent — what the probe
            // grid and the object base derive from
            for p in &a[1..] {
                let m = tmmaps::map::MapFile::load(std::path::Path::new(p));
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for it in &m.items { for k in 0..3 { lo[k] = lo[k].min(it.pos[k]); hi[k] = hi[k].max(it.pos[k]); } }
                let nb = m.blocks.len();
                let lm = lightmap::mapio::load(p).ok();
                let (ncharts, minobj, bbox) = match &lm {
                    Some(l) => { let d = l.chunk.data.as_ref(); match d.and_then(|d| d.cache.mapping()) { Some(mp) => { let objs: Vec<u32> = mp.binds.iter().map(|b| b.obj_group_idx / 4).collect(); (mp.count, objs.iter().copied().min().unwrap_or(0), Some((mp.bbox_min, mp.bbox_max))) } None => (0, 0, None) } }
                    None => (0, 0, None),
                };
                println!("{p}\n  size {:?} ({} m × {} m × {} m)  decoration {}  blocks {nb}  items {}  item pos x [{:.0}, {:.0}] y [{:.0}, {:.0}] z [{:.0}, {:.0}]\n  lightmap: {ncharts} charts, min object {minobj}, bbox {bbox:?}", m.size, m.size[0] * 32, m.size[1] * 8, m.size[2] * 32, m.decoration_id, m.items.len(), lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
                if let Some(l) = &lm { if let Some(d) = l.chunk.data.as_ref() { if let Ok(v) = lightmap::volume::Volume::parse(&d.cache.trailer) { println!("  probe volume: slot grid {:?} label grid {:?} blocks {} unk_f {:?} → origin ({:.0}, {:.0}, {:.0}) m", v.slot_grid, v.grid, v.blocks.len(), v.unk_f, -v.unk_f[0] * 480.0, -v.unk_f[1] * 224.0, -v.unk_f[2] * 480.0); } } }
            }
        }
        "basecheck" => {
            // lmtool basecheck MAP...: the base rule against each map's own bake (objects − items)
            println!("map\tsize\tdecoration\tunbaked\tbaked\tcustom\treplaced\tpieces\titems\tmeasured\trule(P+auth+S²−repl+G)\tok\tnadeo(P+unbaked+baked)\tok");
            for p in &a[1..] {
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(p));
                let h = tmmaps::header::read(p).expect("header");
                let Ok(lm) = lightmap::mapio::load(p) else { continue };
                let Some(mp) = lm.chunk.data.as_ref().and_then(|d| d.cache.mapping()) else { continue };
                let max_obj = mp.binds.iter().map(|b| b.obj_group_idx / 4).max().unwrap_or(0) + 1;
                let measured = max_obj as i64 - mf.items.len() as i64;
                let cells = mf.blocks.iter().map(|b| { let c = b.coords(); (c.0, c.2, b.name.as_str()) });
                let r = lightmap::moods::base_rule(&h.envir, &mf.decoration_id, mf.size, cells, 0, None);
                let nadeo = r.deco_const + mf.blocks.len() as u32 + mf.baked.len() as u32;
                let verdict = |v: u32| if measured == v as i64 { "OK" } else if measured < v as i64 && v as i64 - measured <= 16 { "OK (last items chart-less)" } else { "DIFF" };
                println!("{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", p.rsplit('/').next().unwrap_or(p), mf.size, mf.decoration_id, mf.blocks.len(), mf.baked.len(), r.custom_blocks, r.replaced, r.extra, mf.items.len(), measured, r.base(), verdict(r.base()), nadeo, verdict(nadeo));
            }
        }
        "volcmp" => {
            // lmtool volcmp REF.Map.Gbx OURS.Map.Gbx: the two probe volumes slot by slot — grid, origin, and per
            // occupied slot the world cell ranges (x/y/z) of the reference block against ours
            let load = |p: &str| -> lightmap::volume::Volume { let m = lightmap::mapio::load(p).expect("load"); lightmap::volume::Volume::parse(&m.chunk.data.as_ref().unwrap().cache.trailer).expect("trailer") };
            let (va, vb) = (load(&a[1]), load(&a[2]));
            println!("slot grid: ref {:?} ours {:?}; origin: ref {:?} ours {:?}; blocks: ref {} ours {}; label grid: ref {:?} ours {:?}", va.slot_grid, vb.slot_grid, va.world_origin(), vb.world_origin(), va.blocks.len(), vb.blocks.len(), va.grid, vb.grid);
            let by_slot = |v: &lightmap::volume::Volume| -> std::collections::BTreeMap<(i32, i32, i32), ([f32; 3], [f32; 3], usize, usize)> {
                v.blocks.iter().map(|b| { let (lo, hi) = v.block_world_range(b); (v.block_slot(b), (lo, hi, b.slices.iter().filter(|s| s.is_some()).count(), b.slices.len())) }).collect()
            };
            let (ma, mb) = (by_slot(&va), by_slot(&vb));
            let keys: std::collections::BTreeSet<_> = ma.keys().chain(mb.keys()).copied().collect();
            let (mut same, mut diff) = (0, 0);
            for k in keys {
                match (ma.get(&k), mb.get(&k)) {
                    (Some(a1), Some(b1)) => {
                        let eq = a1.0 == b1.0 && a1.1 == b1.1;
                        if eq { same += 1 } else { diff += 1 }
                        println!("slot {k:?}: ref x[{:.0},{:.0}) y[{:.0},{:.0}) z[{:.0},{:.0}) {}/{} | ours x[{:.0},{:.0}) y[{:.0},{:.0}) z[{:.0},{:.0}) {}/{} {}", a1.0[0], a1.1[0], a1.0[1], a1.1[1], a1.0[2], a1.1[2], a1.2, a1.3, b1.0[0], b1.1[0], b1.0[1], b1.1[1], b1.0[2], b1.1[2], b1.2, b1.3, if eq { "=" } else { "≠" });
                    }
                    (Some(a1), None) => { diff += 1; println!("slot {k:?}: ref x[{:.0},{:.0}) y[{:.0},{:.0}) z[{:.0},{:.0}) | ours —", a1.0[0], a1.1[0], a1.0[1], a1.1[1], a1.0[2], a1.1[2]); }
                    (None, Some(b1)) => { diff += 1; println!("slot {k:?}: ref — | ours x[{:.0},{:.0}) y[{:.0},{:.0}) z[{:.0},{:.0})", b1.0[0], b1.1[0], b1.0[1], b1.1[1], b1.0[2], b1.1[2]); }
                    _ => {}
                }
            }
            println!("{same} slots with identical ranges, {diff} differ");
        }
        "contrast" => {
            // lmtool contrast MAP [--base N] [--min-px N]: per-chart luminance contrast of the frame-0 atlas — for every
            // item chart with ≥ min-px covered texels (own rasterisation), the p10/p90 of the HDR value (fb0-scaled),
            // then the distribution of p10/p90 over charts and the pooled lit/shadow split (a mood's shadow depth)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let min_px: usize = f("--min-px").map(|s| s.parse().unwrap()).unwrap_or(40);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let mut ratios: Vec<f64> = Vec::new();
            let mut all: Vec<f64> = Vec::new();
            let mut horiz: Vec<(f64, f64)> = Vec::new(); // (value, n.y) for horizontal-ish texels
            let mut horiz_rgb: Vec<([f64; 3], f64)> = Vec::new();
            for (ii, inst) in scene.instances.iter().enumerate() {
                let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, w as u32 / 2, h as u32 / 2);
                if pw < 4 || ph < 4 { continue; }
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                if samples.len() < min_px { continue; }
                let k = mp.frame_bytes[0][ci] as f64 / 255.0;
                let fbv = mp.frame_bytes[0][ci];
                let dv = |c: [u8; 3]| -> [f64; 3] { [lightmap::synth::decode_value(c[0], fbv) as f64, lightmap::synth::decode_value(c[1], fbv) as f64, lightmap::synth::decode_value(c[2], fbv) as f64] };
                let mut v: Vec<f64> = samples.iter().map(|s| { let c = dv(ia.get((px + s.px).min(ia.w - 1), (py + s.py).min(ia.h - 1))); 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2] }).collect();
                for (s, val) in samples.iter().zip(v.iter()) { if s.n[1] > 0.9 { horiz.push((*val, s.n[1] as f64)); let c = dv(ia.get((px + s.px).min(ia.w - 1), (py + s.py).min(ia.h - 1))); horiz_rgb.push((c, *val)); } }
                all.extend(v.iter().copied());
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let (p10, p90) = (v[v.len() / 10], v[v.len() * 9 / 10]);
                if p90 > 1e-4 { ratios.push(p10 / p90); }
            }
            ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
            all.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let q = |v: &Vec<f64>, p: f64| v[((v.len() as f64 - 1.0) * p) as usize];
            println!("{} charts, {} texels; per-chart p10/p90 ratio: median {:.3}, p25 {:.3}, p75 {:.3}", ratios.len(), all.len(), q(&ratios, 0.5), q(&ratios, 0.25), q(&ratios, 0.75));
            println!("pooled HDR value (K=1 units): p5 {:.3} p25 {:.3} p50 {:.3} p75 {:.3} p95 {:.3} max {:.3}", q(&all, 0.05), q(&all, 0.25), q(&all, 0.5), q(&all, 0.75), q(&all, 0.95), all[all.len() - 1]);
            // horizontal texels (n.y > 0.9): two-mode split by the midpoint between p10 and p90
            let mut hv: Vec<f64> = horiz.iter().map(|h| h.0).collect();
            hv.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if hv.len() > 100 {
                let (lo, hi) = (q(&hv, 0.1), q(&hv, 0.9));
                let thr = 0.5 * (lo + hi);
                let (mut sl, mut nl, mut sh, mut nh) = (0.0, 0usize, 0.0, 0usize);
                for v in &hv { if *v < thr { sl += v; nl += 1; } else { sh += v; nh += 1; } }
                println!("horizontal texels: {} ; p10 {:.3} p90 {:.3}; below/above the midpoint: {} at mean {:.3} | {} at mean {:.3} → shadow/lit ratio {:.3}", hv.len(), lo, hi, nl, sl / nl.max(1) as f64, nh, sh / nh.max(1) as f64, (sl / nl.max(1) as f64) / (sh / nh.max(1) as f64).max(1e-6));
                let (mut rl, mut rh, mut nl2, mut nh2) = ([0f64; 3], [0f64; 3], 0usize, 0usize);
                for (rgb, v) in &horiz_rgb { if *v < thr { for c in 0..3 { rl[c] += rgb[c]; } nl2 += 1; } else { for c in 0..3 { rh[c] += rgb[c]; } nh2 += 1; } }
                let n1 = nl2.max(1) as f64; let n2 = nh2.max(1) as f64;
                println!("  shadow mean rgb ({:.3}, {:.3}, {:.3})  lit mean rgb ({:.3}, {:.3}, {:.3})  lit − shadow ({:.3}, {:.3}, {:.3})", rl[0] / n1, rl[1] / n1, rl[2] / n1, rh[0] / n2, rh[1] / n2, rh[2] / n2, rh[0] / n2 - rl[0] / n1, rh[1] / n2 - rl[1] / n1, rh[2] / n2 - rl[2] / n1);
            }
        }
        "modelinfo" => {
            // lmtool modelinfo MAP [--min-tris N]: per embedded model — placements, triangles, local bbox size, m/uv
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let min_tris: usize = f("--min-tris").map(|s| s.parse().unwrap()).unwrap_or(1);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let mut counts = vec![0usize; scene.models.len()];
            let mut first_item = vec![usize::MAX; scene.models.len()];
            for inst in &scene.instances { counts[inst.model] += 1; first_item[inst.model] = first_item[inst.model].min(inst.item); }
            let mut rows: Vec<(usize, String)> = Vec::new();
            for (mi, m) in scene.models.iter().enumerate() {
                if m.tris.len() < min_tris { continue; }
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for t in &m.tris { for p in t.p { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } } }
                let up = m.tris.iter().filter(|t| t.n[0][1].abs() > 0.9).count();
                rows.push((counts[mi], format!("{:<26} {:>5} placed  first item {:>5}  {:>7} tris ({:>3}% horizontal)  size {:>6.1} × {:>5.1} × {:>6.1} m  lo ({:.1},{:.1},{:.1})  m/uv {:.1}  lights {}", scene.model_names[mi], counts[mi], first_item[mi], m.tris.len(), 100 * up / m.tris.len().max(1), hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], lo[0], lo[1], lo[2], m.metres_per_uv, m.lights.len())));
            }
            rows.sort_by_key(|r| std::cmp::Reverse(r.0));
            for r in rows { println!("{}", r.1); }
        }
        "daytime" => {
            // lmtool daytime MAP [--out OUT --set N|default]: read (and set) the map's DayTime word in chunk
            // 0x03043056 { u32 version, u32, u32 daytime (0xffffffff = the mood's own), bool dynamic, u32 duration_ms }
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let body = &m.gbx.body;
            let cs = tmmaps::gbx::all_skip_chunks(body);
            let Some(c) = cs.iter().find(|c| c.0 == 0x03043056) else { println!("no chunk 0x03043056"); return };
            let p = c.2;
            let rd = |o: usize| u32::from_le_bytes([body[p + o], body[p + o + 1], body[p + o + 2], body[p + o + 3]]);
            println!("0x03043056: version {} u {} daytime {:#x} ({}) dynamic {} duration {} ms", rd(0), rd(4), rd(8), if rd(8) == 0xffff_ffff { "default = the mood's DayTime01".to_string() } else { format!("{:.4} of the day", rd(8) as f64 / 65536.0) }, rd(12), rd(16));
            if let (Some(out), Some(v)) = (f("--out"), f("--set")) {
                let val: u32 = if v == "default" { 0xffff_ffff } else if let Some(h) = v.strip_prefix("0x") { u32::from_str_radix(h, 16).unwrap() } else { v.parse().unwrap() };
                let mut nb = body.clone();
                nb[p + 8..p + 12].copy_from_slice(&val.to_le_bytes());
                // the lightmap chunk's frame records carry the time too — the editor takes ITS time from them
                // when it opens the map (a bake copy with a stale record re-bakes at the stale time)
                let mut rec_note = String::new();
                if let Some(d) = &m.chunk.data {
                    let rec_val = if val == 0xffff_ffff { d.cache.frame_mood_max_hdr().map(|(_, t)| t).unwrap_or(val) } else { val };
                    let mut chunk = m.chunk.clone();
                    if let Some(dd) = chunk.data.as_mut() {
                        if let Some(mp) = dd.cache.mapping_mut() {
                            for i in 0..3 { let r = 60 + 66 * i + 8; if r + 4 <= mp.head.len() { mp.head[r..r + 4].copy_from_slice(&rec_val.to_le_bytes()); } }
                        }
                        let payload = chunk.write(true);
                        // splice the rewritten lightmap chunk into the body (its skippable size fields follow)
                        if let Some((off, pl, size)) = lightmap::find_chunk(&nb) {
                            // chunk id, skip marker, size, then the payload
                            let mut out_body = Vec::with_capacity(nb.len());
                            out_body.extend_from_slice(&nb[..off + 8]);
                            out_body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                            out_body.extend_from_slice(&payload);
                            out_body.extend_from_slice(&nb[pl + size..]);
                            nb = out_body;
                            rec_note = format!(" and the lightmap frame records ({rec_val:#x})");
                        }
                    }
                }
                let uncompressed = { let mut f = m.gbx.header_bytes_u(); f.extend_from_slice(body); f };
                let t = tmmaps::gbx::Gbx::parse(&uncompressed);
                std::fs::write(&out, t.write_body_recompressed(&nb)).expect("write");
                println!("wrote {out} with daytime {val:#x}{rec_note}");
            }
        }
        "testmap" => {
            // lmtool testmap MAP [--base N] [--dump T.tsv]: the BlueBay-style lighting test map (3 pad groups at y 40,
            // a 16 m pole on the pad centre (324, 324), a tower east of group B, a roof over the group-C centre; --y0 = the pad tops' height, 160) read
            // from the map's own bake: per region the HDR luminance stats (K = 1 units), the pole shadow's direction
            // and length → sun azimuth/elevation, the tower faces' values, the roofed pad's value.
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let y0: f32 = f("--y0").map(|s| s.parse().unwrap()).unwrap_or(160.0); // the pad tops
            let dx: f32 = f("--dx").map(|s| s.parse().unwrap()).unwrap_or(0.0); // layout offset (compact layout: 540, 790)
            let dz: f32 = f("--dz").map(|s| s.parse().unwrap()).unwrap_or(0.0);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            // every texel of every instance inside the test area (x < 600)
            struct Tx { p: [f32; 3], n: [f32; 3], rgb: [f32; 3], lum: f32 }
            let mut tx: Vec<Tx> = Vec::new();
            for (ii, inst) in scene.instances.iter().enumerate() {
                if inst.xf[9] - dx > 600.0 || inst.xf[11] - dz > 600.0 || inst.xf[9] - dx < 250.0 || inst.xf[11] - dz < 250.0 { continue; }
                let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                let fbv = mp.frame_bytes[0][ci];
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                for s in &samples {
                    let c = ia.get((px + s.px).min(ia.w - 1), (py + s.py).min(ia.h - 1));
                    let rgb = [lightmap::synth::decode_value(c[0], fbv), lightmap::synth::decode_value(c[1], fbv), lightmap::synth::decode_value(c[2], fbv)];
                    tx.push(Tx { p: s.p, n: s.n, rgb, lum: 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2] });
                }
            }
            println!("{} texels in the test area", tx.len());
            if let Some(out) = f("--dump") {
                let mut s = String::from("x\ty\tz\tnx\tny\tnz\tr\tg\tb\tlum\n");
                for t in &tx { s.push_str(&format!("{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\n", t.p[0], t.p[1], t.p[2], t.n[0], t.n[1], t.n[2], t.rgb[0], t.rgb[1], t.rgb[2], t.lum)); }
                std::fs::write(&out, s).unwrap();
            }
            let stats = |name: &str, sel: &dyn Fn(&Tx) -> bool| -> (f32, usize) {
                let v: Vec<&Tx> = tx.iter().filter(|t| sel(t)).collect();
                if v.is_empty() { println!("{name:<44} (no texels)"); return (0.0, 0); }
                let mut l: Vec<f32> = v.iter().map(|t| t.lum).collect();
                l.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mean_rgb = { let mut s = [0f32; 3]; for t in &v { for c in 0..3 { s[c] += t.rgb[c]; } } [s[0] / v.len() as f32, s[1] / v.len() as f32, s[2] / v.len() as f32] };
                let med = l[l.len() / 2];
                println!("{name:<44} n {:>6}  lum median {:.4}  p10 {:.4}  p90 {:.4}  mean rgb ({:.3}, {:.3}, {:.3})", v.len(), med, l[l.len() / 10], l[l.len() * 9 / 10], mean_rgb[0], mean_rgb[1], mean_rgb[2]);
                (med, v.len())
            };
            let up = |t: &Tx| t.n[1] > 0.9 && t.p[1] > y0 - 1.0 && t.p[1] < y0 + 1.5;
            // group A: pads x 300..348, z 300..348; the pole base at (323.2..324.7, 323.2..324.7)
            let in_a = |t: &Tx| up(t) && (t.p[0] - dx) >= 300.0 && (t.p[0] - dx) < 348.0 && (t.p[2] - dz) >= 300.0 && (t.p[2] - dz) < 348.0;
            let (med_a, _) = stats("A open pads (all)", &in_a);
            // shadow texels of A: lum < 0.75 × median, away from the pole itself
            let shadow: Vec<&Tx> = tx.iter().filter(|t| in_a(t) && t.lum < 0.75 * med_a && (((t.p[0] - dx) - 324.0).abs() > 1.5 || ((t.p[2] - dz) - 324.0).abs() > 1.5)).collect();
            println!("A shadow texels (< 75 % of median): {}", shadow.len());
            if shadow.len() >= 3 {
                // direction: mean vector from the pole base; length: the farthest shadow texel along that direction
                let (mut sx, mut sz) = (0f32, 0f32);
                for t in &shadow { sx += (t.p[0] - dx) - 324.0; sz += (t.p[2] - dz) - 324.0; }
                let len = (sx * sx + sz * sz).sqrt(); let (dx, dz) = (sx / len, sz / len);
                let far = shadow.iter().map(|t| ((t.p[0] - dx) - 324.0) * dx + ((t.p[2] - dz) - 324.0) * dz).fold(0f32, f32::max);
                // the sun is opposite the shadow; az measured like the baker: dir = (cos el sin az, sin el, cos el cos az)
                let sun_az = (-dx).atan2(-dz).to_degrees().rem_euclid(360.0);
                let el = (16.0f32 / far.max(0.1)).atan().to_degrees();
                println!("pole shadow: direction ({dx:.2}, {dz:.2}) length {far:.1} m (pole 16 m) → SUN az {sun_az:.1}° el {el:.1}° (el from the shadow tip; texel size limits it to ±{:.1}°)", (16.0f32 / (far - 2.0).max(0.1)).atan().to_degrees() - el);
                stats("A lit pads (≥ 75 % of median)", &|t: &Tx| in_a(t) && t.lum >= 0.75 * med_a);
                stats("A shadow", &|t: &Tx| in_a(t) && t.lum < 0.75 * med_a && (((t.p[0] - dx) - 324.0).abs() > 1.5 || ((t.p[2] - dz) - 324.0).abs() > 1.5));
            }
            // group B pads and the tower shadow
            let in_b = |t: &Tx| up(t) && (t.p[0] - dx) >= 400.0 && (t.p[0] - dx) < 448.0 && (t.p[2] - dz) >= 300.0 && (t.p[2] - dz) < 348.0;
            let (med_b, _) = stats("B pads (tower east of them)", &in_b);
            stats("B lit", &|t: &Tx| in_b(t) && t.lum >= 0.75 * med_b.max(med_a));
            stats("B shadow", &|t: &Tx| in_b(t) && t.lum < 0.75 * med_b.max(med_a));
            // tower faces (item at (448, 40, 316), yaw 0; mesh lo (−0.7, 0, −1.1), 17.4 × 52 × 19.6)
            let tower = |t: &Tx| (t.p[0] - dx) > 446.0 && (t.p[0] - dx) < 466.5 && (t.p[2] - dz) > 313.0 && (t.p[2] - dz) < 337.0 && t.p[1] > y0 + 2.0 && t.p[1] < y0 + 52.0;
            stats("tower face −x (west)", &|t: &Tx| tower(t) && t.n[0] < -0.9);
            stats("tower face +x (east)", &|t: &Tx| tower(t) && t.n[0] > 0.9);
            stats("tower face −z (south)", &|t: &Tx| tower(t) && t.n[2] < -0.9);
            stats("tower face +z (north)", &|t: &Tx| tower(t) && t.n[2] > 0.9);
            stats("tower top", &|t: &Tx| (t.p[0] - dx) > 446.0 && (t.p[0] - dx) < 466.5 && (t.p[2] - dz) > 313.0 && (t.p[2] - dz) < 337.0 && t.p[1] > y0 + 48.0 && t.n[1] > 0.9);
            // group C: pads x 300..348, z 400..448; the roof (316, 52, 416): 16 × 16 above the centre pad, 12 m up
            let in_c = |t: &Tx| up(t) && (t.p[0] - dx) >= 300.0 && (t.p[0] - dx) < 348.0 && (t.p[2] - dz) >= 400.0 && (t.p[2] - dz) < 448.0;
            stats("C open pads (not under the roof)", &|t: &Tx| in_c(t) && !((t.p[0] - dx) >= 316.0 && (t.p[0] - dx) < 332.0 && (t.p[2] - dz) >= 416.0 && (t.p[2] - dz) < 432.0));
            stats("C pad under the roof", &|t: &Tx| in_c(t) && (t.p[0] - dx) >= 316.0 && (t.p[0] - dx) < 332.0 && (t.p[2] - dz) >= 416.0 && (t.p[2] - dz) < 432.0);
            stats("C pad under the roof, inner 8×8", &|t: &Tx| in_c(t) && (t.p[0] - dx) >= 320.0 && (t.p[0] - dx) < 328.0 && (t.p[2] - dz) >= 420.0 && (t.p[2] - dz) < 428.0);
            // radial profile under/around the roof: pad texels of group C binned by distance from the roof centre
            {
                let (cx, cz) = (324.0f32 + dx, 424.0f32 + dz);
                let mut bins = vec![(0f64, 0usize); 16];
                for t in tx.iter().filter(|t| in_c(t)) {
                    let r = ((t.p[0] - cx).powi(2) + (t.p[2] - cz).powi(2)).sqrt();
                    let b = (r / 1.5) as usize;
                    if b < bins.len() { bins[b].0 += t.lum as f64; bins[b].1 += 1; }
                }
                let open = bins.iter().rev().find(|b| b.1 > 0).map(|b| b.0 / b.1 as f64).unwrap_or(1.0);
                println!("C pads by distance from the roof centre (roof 16×16 at +12 m; ratio to the outermost bin):");
                for (b, (s, n)) in bins.iter().enumerate() { if *n > 0 { println!("  r {:>4.1}–{:<4.1} m  n {:>6}  lum {:.4}  ratio {:.3}", b as f32 * 1.5, (b + 1) as f32 * 1.5, n, s / *n as f64, s / *n as f64 / open); } }
            }
            stats("roof top", &|t: &Tx| t.n[1] > 0.9 && t.p[1] > y0 + 15.0 && t.p[1] < y0 + 18.0 && (t.p[0] - dx) >= 316.0 && (t.p[0] - dx) < 332.0 && (t.p[2] - dz) >= 416.0 && (t.p[2] - dz) < 432.0);
            stats("roof underside", &|t: &Tx| t.n[1] < -0.9 && t.p[1] > y0 + 11.0 && t.p[1] < y0 + 14.0 && (t.p[0] - dx) >= 316.0 && (t.p[0] - dx) < 332.0 && (t.p[2] - dz) >= 416.0 && (t.p[2] - dz) < 432.0);
            stats("pad undersides (n.y < −0.9)", &|t: &Tx| t.n[1] < -0.9 && t.p[1] > y0 - 1.0 && t.p[1] < y0 + 1.0);
            // vertical texels by the azimuth of their normal (12 bins of 30°): the brightest bin faces the sun
            {
                let mut bins = vec![(0f64, [0f64; 3], 0usize); 12];
                for t in tx.iter().filter(|t| t.n[1].abs() < 0.3) {
                    let az = t.n[0].atan2(t.n[2]).to_degrees().rem_euclid(360.0); // 0 = +z (north), 90 = +x (east)
                    let b = ((az / 30.0) as usize).min(11);
                    bins[b].0 += t.lum as f64; for k in 0..3 { bins[b].1[k] += t.rgb[k] as f64; } bins[b].2 += 1;
                }
                println!("vertical texels by normal azimuth (0 = +z north, 90 = +x east): mean lum / rgb");
                for (b, (s, rgb, n)) in bins.iter().enumerate() { if *n > 0 { let nn = *n as f64; println!("  az {:>3}–{:<3} n {:>6}  lum {:.4}  rgb ({:.3}, {:.3}, {:.3})", b * 30, (b + 1) * 30, n, s / nn, rgb[0] / nn, rgb[1] / nn, rgb[2] / nn); } }
            }
            // the free-standing wall AC16902402 (1×8×16) at compact (448, 160, 410): west face looks at the B pads, east face at the sea
            let wall = |t: &Tx| (t.p[0] - dx) > 440.0 && (t.p[0] - dx) < 470.0 && (t.p[2] - dz) > 405.0 && (t.p[2] - dz) < 430.0 && t.p[1] > y0 + 0.5 && t.p[1] < y0 + 9.0;
            stats("wall west face (−x, towards the B pads)", &|t: &Tx| wall(t) && t.n[0] < -0.9);
            stats("wall east face (+x, towards the sea)", &|t: &Tx| wall(t) && t.n[0] > 0.9);
            stats("wall west face, lower half (y0..y0+4)", &|t: &Tx| wall(t) && t.n[0] < -0.9 && t.p[1] < y0 + 4.0);
            stats("wall west face, upper half", &|t: &Tx| wall(t) && t.n[0] < -0.9 && t.p[1] >= y0 + 4.0);
            stats("wall top", &|t: &Tx| wall(t) && t.n[1] > 0.9 && t.p[1] > y0 + 6.0);
        }
        "sky" => {
            // lmtool sky FILE.dds [--face-out DIR]: decode a BC6H DDS (a cubemap or a 2:1 panorama) and print, per face,
            // the mean radiance, and the cosine-weighted irradiance E(n) = ∫ L(ω) max(0, n·ω) dω for the six axis
            // normals — what a sky-lit surface receives from that image (K = XML HDR units)
            let data = std::fs::read(&a[1]).expect("read dds");
            let dds = lightmap::bc6h::parse_dds(&data).expect("dds");
            println!("{}: {}x{} format {} mips {} cubemap {}", a[1], dds.w, dds.h, dds.format, dds.mips, dds.cubemap);
            let faces = if dds.cubemap { 6 } else { 1 };
            let mut imgs: Vec<Vec<[f32; 3]>> = Vec::new();
            if dds.cubemap {
                let cube = lightmap::skycube::CubeMap::load(&a[1]).expect("cube");
                imgs = cube.faces;
            } else {
                assert!(dds.format == 95 || dds.format == 96, "not BC6H");
                imgs.push(lightmap::bc6h::decode_image(dds.data, dds.w, dds.h, dds.format == 96));
            }
            let names = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"];
            let mut total = [0f64; 3];
            for (fi, im) in imgs.iter().enumerate() {
                let mut s = [0f64; 3];
                let mut mx = 0f32;
                for p in im { for c in 0..3 { s[c] += p[c] as f64; total[c] += p[c] as f64; } mx = mx.max(p[0].max(p[1]).max(p[2])); }
                let n = im.len() as f64;
                println!("  face {} ({}): mean ({:.4}, {:.4}, {:.4}) max {:.3}", fi, if dds.cubemap { names[fi] } else { "pano" }, s[0] / n, s[1] / n, s[2] / n, mx);
            }
            // direction of texel (face, u, v) in the D3D cubemap convention
            let dir_of = |face: usize, u: f32, v: f32| -> [f32; 3] {
                // u, v in [-1, 1], v down
                match face { 0 => [1.0, -v, -u], 1 => [-1.0, -v, u], 2 => [u, 1.0, v], 3 => [u, -1.0, -v], 4 => [u, -v, 1.0], _ => [-u, -v, -1.0] }
            };
            let normals = [("up +Y", [0.0f32, 1.0, 0.0]), ("down -Y", [0.0, -1.0, 0.0]), ("+X", [1.0, 0.0, 0.0]), ("-X", [-1.0, 0.0, 0.0]), ("+Z", [0.0, 0.0, 1.0]), ("-Z", [0.0, 0.0, -1.0])];
            let mut irr = vec![[0f64; 3]; normals.len()];
            let mut solid = 0f64;
            if dds.cubemap {
                let n = dds.w;
                for face in 0..6 { for y in 0..n { for x in 0..n {
                    let (u, v) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                    let d = dir_of(face, u, v);
                    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                    let dn = [d[0] / len2.sqrt(), d[1] / len2.sqrt(), d[2] / len2.sqrt()];
                    // solid angle of a cube texel: (2/n)² / len³
                    let dw = (4.0 / (n * n) as f64) / (len2 as f64).powf(1.5);
                    solid += dw;
                    let p = imgs[face][y * n + x];
                    for (k, (_, nn)) in normals.iter().enumerate() {
                        let c = dn[0] * nn[0] + dn[1] * nn[1] + dn[2] * nn[2];
                        if c > 0.0 { for ch in 0..3 { irr[k][ch] += p[ch] as f64 * c as f64 * dw; } }
                    }
                }}}
            } else {
                // equirectangular panorama: x → azimuth, y → polar angle from the top
                let (w, h) = (dds.w, dds.h);
                for y in 0..h { for x in 0..w {
                    let theta = (y as f32 + 0.5) / h as f32 * std::f32::consts::PI;
                    let phi = (x as f32 + 0.5) / w as f32 * 2.0 * std::f32::consts::PI;
                    let dn = [theta.sin() * phi.sin(), theta.cos(), theta.sin() * phi.cos()];
                    let dw = (theta.sin() as f64) * (std::f64::consts::PI / h as f64) * (2.0 * std::f64::consts::PI / w as f64);
                    solid += dw;
                    let p = imgs[0][y * w + x];
                    for (k, (_, nn)) in normals.iter().enumerate() {
                        let c = dn[0] * nn[0] + dn[1] * nn[1] + dn[2] * nn[2];
                        if c > 0.0 { for ch in 0..3 { irr[k][ch] += p[ch] as f64 * c as f64 * dw; } }
                    }
                }}
            }
            println!("  solid angle check {:.4} (4π = {:.4})", solid, 4.0 * std::f64::consts::PI);
            for (k, (name, _)) in normals.iter().enumerate() {
                let e = irr[k];
                println!("  irradiance E(n) for n = {name:<8}: ({:.4}, {:.4}, {:.4})  lum {:.4}   E/π = ({:.4}, {:.4}, {:.4})", e[0], e[1], e[2], 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2], e[0] / std::f64::consts::PI, e[1] / std::f64::consts::PI, e[2] / std::f64::consts::PI);
            }
            if let Some(dir) = a.iter().position(|x| x == "--face-out").and_then(|i| a.get(i + 1)) {
                std::fs::create_dir_all(dir).unwrap();
                for (fi, im) in imgs.iter().enumerate() {
                    let mut rgb = lightmap::img::Rgb::new(dds.w as u32, dds.h as u32);
                    for (i, p) in im.iter().enumerate() { let q = |v: f32| ((v / (1.0 + v)).powf(1.0 / 2.2) * 255.0).clamp(0.0, 255.0) as u8; rgb.px[i * 3] = q(p[0]); rgb.px[i * 3 + 1] = q(p[1]); rgb.px[i * 3 + 2] = q(p[2]); }
                    lightmap::img::write_ppm(&rgb, &format!("{dir}/face{fi}.ppm")).unwrap();
                }
            }
        }
        "transplant" => {
            // lmtool transplant --from REDUCED_BAKED.Map.Gbx --into FULL.Map.Gbx --kept i1,i2,… --out OUT [--base N]:
            // the editor's lightmap of a REDUCED item set (the groves / light-carrying items dropped so the editor
            // survives) put into the FULL map — every item chart's object id is renumbered from the reduced index
            // to the full index (kept[r]); the dropped items get no chart (the game lights them from the probes).
            // Tiles (object < base) keep their ids.
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let kept: Vec<u32> = f("--kept").expect("--kept").split(',').filter(|s| !s.is_empty()).map(|s| s.trim().parse().unwrap()).collect();
            let from = lightmap::mapio::load(&f("--from").expect("--from")).expect("load --from");
            let into = lightmap::mapio::load(&f("--into").expect("--into")).expect("load --into");
            let mut chunk = from.chunk.clone();
            let d = chunk.data.as_mut().expect("the reduced bake has no lightmap");
            let mp = d.cache.mapping_mut().expect("mapping");
            let (mut items, mut tiles, mut oob) = (0usize, 0usize, 0usize);
            for b in mp.binds.iter_mut() {
                let obj = b.obj_group_idx / 4;
                let sub = b.obj_group_idx % 4;
                if obj >= base {
                    let r = (obj - base) as usize;
                    match kept.get(r) { Some(&full) => { b.obj_group_idx = (base + full) * 4 + sub; items += 1; } None => { oob += 1; } }
                } else { tiles += 1; }
            }
            // the mapping's own count of items may live in the head/tail — the bind ids are what the loader uses
            let payload = chunk.write(true);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&into, &payload, &out).expect("save");
            println!("wrote {out}: {items} item charts renumbered (reduced → full), {tiles} tile charts kept, {oob} charts beyond the kept list; the full map has {} items, the reduced bake {}", tmmaps::map::MapFile::load(std::path::Path::new(&f("--into").unwrap())).items.len(), kept.len());
        }
        "graft" => {
            // lmtool graft MAP --from OTHER.Map.Gbx --out OUT: MAP with OTHER's lightmap chunk verbatim (a deliberately
            // foreign chunk makes the editor recompute the lightmap at load — the slow open the save path needs)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let src = lightmap::mapio::load(&f("--from").expect("--from")).expect("load --from");
            let payload = src.chunk.write(false);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&m, &payload, &out).expect("save");
            println!("wrote {out} with the lightmap chunk of {} ({} B)", f("--from").unwrap(), payload.len());
        }
        "strip" => {
            // lmtool strip MAP --out OUT: the map with an EMPTY lightmap chunk (has_lightmaps = 0) — the editor then
            // bakes fresh instead of recomputing a coarse lightmap at load for a stored one that no longer fits
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let mut m = lightmap::mapio::load(&a[1]).expect("load");
            let v = m.chunk.version;
            m.chunk = lightmap::format::LightmapChunk { version: v, u01: 0, u02: 0, data: None };
            let payload = m.chunk.write(false);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&m, &payload, &out).expect("save");
            println!("wrote {out} with an empty lightmap chunk ({} B)", payload.len());
        }
        "check" => {
            // lmtool check MAP [--base N|auto] [--baked-total N]: validate a lit map against the rules — chunk
            // structure (frames, kinds, the three-image blob1, four-image probe blob), the object base, the probe
            // grid (slot table, origin, cell, tiles inside the atlas), atlas fill, texel encoding sanity
            // (fb0 vs chart max), file size; exit 1 on any FAIL
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let mut fails = 0;
            let mut report = |ok: bool, what: &str| { println!("  [{}] {what}", if ok { " ok " } else { "FAIL" }); if !ok { fails += 1; } };
            let p = &a[1];
            let m = lightmap::mapio::load(p).expect("load");
            let d = m.chunk.data.as_ref().expect("has lightmaps");
            let mp = d.cache.mapping().expect("mapping");
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(p));
            let hdr = tmmaps::header::read(p).expect("header");
            println!("{p}: {} {}, size {:?}, {} items, {} unbaked + {} baked blocks, file {} B", hdr.envir, mf.decoration_id, mf.size, mf.items.len(), mf.blocks.len(), mf.baked.len(), std::fs::metadata(p).map(|x| x.len()).unwrap_or(0));
            let fsz = std::fs::metadata(p).map(|x| x.len()).unwrap_or(0);
            println!("  [{}] file {} B vs the 25 MiB upload cap (an embedded-items map may legitimately exceed it locally)", if fsz < 25 * 1024 * 1024 { " ok " } else { "warn" }, fsz);
            // frames
            report((d.frames.len() == 3 || d.frames.len() == 2) && d.frames[0].images.len() == 3, &format!("3 frames (2 without local lights) × 3 image slots ({} frames)", d.frames.len()));
            let riffs = |b: &[u8]| -> usize { let mut n = 0; let mut o = 0; while o + 12 <= b.len() && &b[o..o + 4] == b"RIFF" { let sz = u32::from_le_bytes([b[o + 4], b[o + 5], b[o + 6], b[o + 7]]) as usize + 8; n += 1; o += sz + (sz & 1); } if o == b.len() { n } else { 0 } };
            report(riffs(&d.frames[0].images[0]) == 1, "frame 0 image 0 is one WebP (H-basis colour)");
            report(riffs(&d.frames[0].images[1]) == 3, &format!("frame 0 image 1 is THREE concatenated WebPs (directional coefficients; found {})", riffs(&d.frames[0].images[1])));
            // the frame → texture upload reads every image of a frame at chart rects taken from image 0's
            // size (RE child 2: the 09-22 editor crashes at Trackmania.exe+0x280bf4): all the frame's
            // full-size images must share image 0's dimensions
            {
                let dims = |b: &[u8]| -> Option<(u32, u32)> { let mut o = 0; let mut first = None; while o + 12 <= b.len() && &b[o..o + 4] == b"RIFF" { let sz = u32::from_le_bytes([b[o + 4], b[o + 5], b[o + 6], b[o + 7]]) as usize + 8; if let Ok(im) = lightmap::img::decode_webp(&b[o..(o + sz).min(b.len())]) { if first.is_none() { first = Some((im.w, im.h)); } else if first != Some((im.w, im.h)) { return None; } } o += sz + (sz & 1); } first };
                let mut all_ok = true;
                let mut notes = Vec::new();
                for (fi, fr) in d.frames.iter().enumerate() {
                    let d0 = fr.images.first().filter(|b| !b.is_empty()).and_then(|b| dims(b));
                    for (ii, im) in fr.images.iter().enumerate().skip(1) {
                        if im.is_empty() || (fi == 0 && ii == 2) { continue; } // frame 0 image 2 = the probe atlas (its own size)
                        let di = dims(im);
                        if d0.is_some() && di != d0 { all_ok = false; notes.push(format!("frame {fi} image {ii} {:?} vs image 0 {:?}", di, d0)); }
                    }
                }
                report(all_ok, &format!("every frame's images share image 0's size (the upload reads them at image 0's chart rects){}", if notes.is_empty() { String::new() } else { format!(": {}", notes.join(", ")) }));
            }
            let v = lightmap::volume::Volume::parse(&d.cache.trailer);
            report(v.is_ok(), "probe trailer parses");
            if let Ok(v) = &v {
                report(v.write() == d.cache.trailer, "probe trailer round-trips byte for byte");
                let parts = lightmap::volume::split_probe_blob(&d.frames[0].images[2], &v.frame_info);
                report(parts.len() == 4 && parts.iter().all(|x| x.len() > 12 && &x[..4] == b"RIFF"), &format!("probe blob = four WebPs at the trailer offsets (found {})", parts.len()));
                let dims: Vec<(u32, u32)> = parts.iter().filter_map(|x| lightmap::img::decode_webp(x).ok().map(|i| (i.w, i.h))).collect();
                report(dims.len() == 4 && dims.iter().all(|x| *x == dims[0]), &format!("probe images decode with one size {:?}", dims.first()));
                if let Some(&(aw, ah)) = dims.first() {
                    report(v.cell4.len() as u32 == ((aw + 3) / 4) * ((ah + 3) / 4), "mask table matches the probe atlas");
                    let inside = v.blocks.iter().all(|b| { let (tw, th) = (b.max[0] - b.min[0], b.max[2] - b.min[2]); b.slices.iter().flatten().all(|s| s.0 + tw <= aw && s.1 + th <= ah) });
                    report(inside, "every stored probe tile lies inside the atlas");
                }
                let table_ok = v.blocks.iter().enumerate().all(|(i, b)| { let (si, sj, sk) = v.block_slot(b); let idx = si + v.slot_grid[0] as i32 * sj + (v.slot_grid[0] * v.slot_grid[1]) as i32 * sk; idx >= 0 && v.slots.get(idx as usize).copied() == Some(i as i32) });
                report(table_ok, &format!("slot table consistent with every block's pos ({} blocks, grid {:?}, origin {:?}, cell {} m)", v.blocks.len(), v.slot_grid, v.world_origin(), v.cell_size()));
                let grid_m = [mf.size[0] as f32 * 32.0, mf.size[1] as f32 * 8.0, mf.size[2] as f32 * 32.0];
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for it in &mf.items { for k in 0..3 { lo[k] = lo[k].min(it.pos[k] - 16.0); hi[k] = hi[k].max(it.pos[k]); } }
                let want = lightmap::probes::SlotGrid::for_map(&hdr.envir, &mf.decoration_id, grid_m, lo, hi);
                report(want.n == v.slot_grid, &format!("slot grid {:?} = the rule's {:?} for size {:?} (from item positions alone)", v.slot_grid, want.n, mf.size));
            }
            // base
            let max_obj = mp.binds.iter().map(|b| b.obj_group_idx / 4).max().unwrap_or(0) + 1;
            let measured = max_obj as i64 - mf.items.len() as i64;
            let cells = mf.blocks.iter().map(|b| { let c = b.coords(); (c.0, c.2, b.name.as_str()) });
            let r = lightmap::moods::base_rule(&hdr.envir, &mf.decoration_id, mf.size, cells, 0, f("--baked-total").map(|s| s.parse().unwrap()));
            let expect: i64 = match f("--base") { Some(s) if s != "auto" => s.parse().unwrap(), _ => r.base() as i64 };
            report(measured == expect || (measured < expect && expect - measured <= 16), &format!("object base {measured} (max charted object − items) = rule {expect} (P {} + authored {} + tiles {} − replaced {} + G {})", r.deco_const, r.authored, r.ground_cols, r.replaced, r.extra));
            // charts: every item has a chart; fill
            let mut have = std::collections::HashSet::new();
            for b in &mp.binds { let o = b.obj_group_idx / 4; if o as i64 >= expect { have.insert(o as i64 - expect); } }
            let missing = (0..mf.items.len() as i64).filter(|i| !have.contains(i)).count();
            report(missing <= mf.items.len() / 100, &format!("{missing} of {} items without a chart", mf.items.len()));
            let area: u64 = mp.size.iter().map(|(w, h)| (*w as u64 / 2) * (*h as u64 / 2)).sum();
            println!("  [info] atlas fill {:.1} % ({} charts)", 100.0 * area as f64 / (1024.0 * 1024.0), mp.count);
            // encoding sanity: the brightest texel of a sample of charts should sit near 255 (p = sqrt(E/max) → 255 at the max)
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("atlas");
            let mut near = 0; let mut n = 0;
            for i in (0..mp.count as usize).step_by((mp.count as usize / 400).max(1)) {
                let (x, y) = mp.pos[i]; let (w, h) = mp.size[i];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                let mut mx = 0u8;
                for yy in 0..ph { for xx in 0..pw { let c = ia.get((px + xx).min(ia.w - 1), (py + yy).min(ia.h - 1)); mx = mx.max(c[0]).max(c[1]).max(c[2]); } }
                if mp.frame_bytes[0][i] > 0 { n += 1; if mx >= 200 { near += 1; } }
            }
            report(n > 0 && near * 10 >= n * 7, &format!("chart maxima reach the top of the sqrt scale ({near} of {n} sampled charts ≥ 200)"));
            // the time the bake was made at (the frame record) must be the map's (chunk 0x03043056; a default
            // word = the decoration mood's own): the editor keeps its time across the maps of one session
            if let Some((_, rec_t)) = d.cache.frame_mood_max_hdr() {
                let map_t = lightmap::mapio::daytime(&m.gbx.body).unwrap_or(0xffff_ffff);
                let mood_name = lightmap::moods::effective_mood(&mf.decoration_id, Some(map_t));
                let expect_t = if map_t == 0xffff_ffff { lightmap::moods::default_daytime(&hdr.envir, mood_name) } else { map_t };
                report(rec_t == expect_t, &format!("baked at the map's time: record {rec_t:#x} ({:.3}) vs map word {} → {expect_t:#x} ({:.3}); mood {mood_name}", rec_t as f64 / 65536.0, if map_t == 0xffff_ffff { "default".to_string() } else { format!("{map_t:#x}") }, expect_t as f64 / 65536.0));
            }
            if let Some(fm) = d.cache.frame_max_hdr() { println!("  [info] frame MaxHDR {fm:.4}"); }
            println!("{}", if fails == 0 { "CHECK PASSED" } else { "CHECK FAILED" });
            if fails > 0 { std::process::exit(1); }
        }
        "itemcharts" => {
            // lmtool itemcharts MAP [--base N] [--model NAME]: per model, the chart sizes the map's own bake gives its
            // placements (min/median/max px), the frame bytes, and whether our loader has geometry for it — finds the
            // items the editor charts that we skip (no TexCoord1) or size differently
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let mut have_geom = vec![false; mf.items.len()];
            for inst in &scene.instances { if inst.item < have_geom.len() { have_geom[inst.item] = true; } }
            let want = f("--model");
            let mut per: std::collections::BTreeMap<String, (usize, Vec<u32>, Vec<u8>, usize, usize)> = Default::default(); // placements, areas px, fb0, with geom, charted
            for (i, it) in mf.items.iter().enumerate() {
                if let Some(w) = &want { if &it.model != w { continue; } }
                let e = per.entry(it.model.clone()).or_default();
                e.0 += 1;
                if have_geom[i] { e.3 += 1; }
                if let Some(&ci) = chart_of.get(&(i as u32)) {
                    e.4 += 1;
                    let (w, h) = mp.size[ci];
                    e.1.push((w as u32 / 2) * (h as u32 / 2));
                    e.2.push(mp.frame_bytes[0][ci]);
                }
            }
            let mut rows: Vec<_> = per.into_iter().collect();
            rows.sort_by_key(|(_, e)| std::cmp::Reverse(e.0));
            println!("{:<26} {:>6} {:>6} {:>7}  chart px² min/med/max   fb0 med", "model", "placed", "geom", "charted");
            for (m, (n, mut areas, mut fbs, g, c)) in rows {
                areas.sort(); fbs.sort();
                let med = |v: &Vec<u32>| v.get(v.len() / 2).copied().unwrap_or(0);
                println!("{:<26} {:>6} {:>6} {:>7}  {:>4}/{:>4}/{:>4}   {}", m, n, g, c, areas.first().copied().unwrap_or(0), med(&areas), areas.last().copied().unwrap_or(0), fbs.get(fbs.len() / 2).copied().unwrap_or(0));
            }
        }
        "lightprofile" => {
            // lmtool lightprofile MAP --lamp x,z [--base N] [--y0 Y]: the frame-1 (local lights) value of the horizontal
            // texels around a lamp, binned by horizontal distance (2 m bins), absolute HDR (sqrt-decoded × frame-1
            // MaxHDR) — the falloff law and reach of the lamp's lights read straight off an editor bake; also lists
            // the scene's lights within 30 m of the lamp (our reading of its CPlugLights)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let y0: f32 = f("--y0").map(|s| s.parse().unwrap()).unwrap_or(160.0);
            let lamp: Vec<f32> = f("--lamp").expect("--lamp x,z").split(',').map(|s| s.trim().parse().unwrap()).collect();
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let i1 = lightmap::img::decode_webp(&d.frames[1].images[0]).unwrap();
            let i0 = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let f1_max = d.cache.frame_max_hdr_n(1).unwrap_or(1.0);
            let f0_max = d.cache.frame_max_hdr().unwrap_or(1.0);
            println!("frame 1 MaxHDR {f1_max:.4}, frame 0 MaxHDR {f0_max:.4}");
            for (li, l) in scene.world_lights() {
                let dx = l.pos[0] - lamp[0]; let dz = l.pos[2] - lamp[1];
                if (dx * dx + dz * dz).sqrt() < 30.0 {
                    println!("  light of item {li}: pos ({:.2}, {:.2}, {:.2}) dir ({:.2}, {:.2}, {:.2}) colour ({:.2}, {:.2}, {:.2}) intensity {:.3} radius {:.2} cone ({:.1}, {:.1})", l.pos[0], l.pos[1], l.pos[2], l.dir[0], l.dir[1], l.dir[2], l.color[0], l.color[1], l.color[2], l.intensity, l.radius, l.cone.0, l.cone.1);
                }
            }
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let mut bins: Vec<(f64, [f64; 3], f64, usize)> = vec![(0.0, [0.0; 3], 0.0, 0); 40]; // lum1 sum, rgb1 sum, lum0 sum, n
            for (ii, inst) in scene.instances.iter().enumerate() {
                let dx = inst.xf[9] - lamp[0]; let dz = inst.xf[11] - lamp[1];
                if (dx * dx + dz * dz).sqrt() > 120.0 { continue; }
                let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                let (fb0, fb1) = (mp.frame_bytes[0][ci], mp.frame_bytes[1][ci]);
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                for s in &samples {
                    if s.n[1] < 0.9 || (s.p[1] - y0).abs() > 1.5 { continue; }
                    let r = ((s.p[0] - lamp[0]).powi(2) + (s.p[2] - lamp[1]).powi(2)).sqrt();
                    let b = (r / 2.0) as usize;
                    if b >= bins.len() { continue; }
                    let c1 = i1.get((px + s.px).min(i1.w - 1), (py + s.py).min(i1.h - 1));
                    let c0 = i0.get((px + s.px).min(i0.w - 1), (py + s.py).min(i0.h - 1));
                    let v1 = [lightmap::synth::decode_value(c1[0], fb1) as f64 * f1_max as f64, lightmap::synth::decode_value(c1[1], fb1) as f64 * f1_max as f64, lightmap::synth::decode_value(c1[2], fb1) as f64 * f1_max as f64];
                    let l0 = (0.2126 * lightmap::synth::decode_value(c0[0], fb0) as f64 + 0.7152 * lightmap::synth::decode_value(c0[1], fb0) as f64 + 0.0722 * lightmap::synth::decode_value(c0[2], fb0) as f64) * f0_max as f64;
                    let e = &mut bins[b];
                    e.0 += 0.2126 * v1[0] + 0.7152 * v1[1] + 0.0722 * v1[2];
                    for k in 0..3 { e.1[k] += v1[k]; }
                    e.2 += l0;
                    e.3 += 1;
                }
            }
            // the falloff read against the nearest light: g(x) = E / (I·c·ndl) per 0.05 of x = d/R, texel by texel
            if let Some(h) = f("--light-h") {
                let h: f32 = h.parse().unwrap(); // the light's height above the pad
                let (r0, c0i) = (f("--light-r").map(|s| s.parse::<f32>().unwrap()).unwrap_or(10.0), f("--light-ic").map(|s| s.parse::<f32>().unwrap()).unwrap_or(0.92));
                let mut xb: Vec<(f64, usize)> = vec![(0.0, 0); 21];
                for (ii, inst) in scene.instances.iter().enumerate() {
                    let dx = inst.xf[9] - lamp[0]; let dz = inst.xf[11] - lamp[1];
                    if (dx * dx + dz * dz).sqrt() > 60.0 { continue; }
                    let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                    let (x, y) = mp.pos[ci]; let (w, hh) = mp.size[ci];
                    let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (hh as u32 / 2).max(1));
                    let fb1 = mp.frame_bytes[1][ci];
                    let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                    for s in &samples {
                        if s.n[1] < 0.9 || (s.p[1] - y0).abs() > 1.5 { continue; }
                        let r = ((s.p[0] - lamp[0]).powi(2) + (s.p[2] - lamp[1]).powi(2)).sqrt();
                        let d = (r * r + h * h).sqrt();
                        let xx = d / r0;
                        if xx >= 1.05 { continue; }
                        let ndl = h / d;
                        let c1 = i1.get((px + s.px).min(i1.w - 1), (py + s.py).min(i1.h - 1));
                        let l1 = (0.2126 * lightmap::synth::decode_value(c1[0], fb1) as f64 + 0.7152 * lightmap::synth::decode_value(c1[1], fb1) as f64 + 0.0722 * lightmap::synth::decode_value(c1[2], fb1) as f64) * f1_max as f64;
                        let g = l1 / (c0i as f64 * ndl as f64);
                        let b = ((xx / 0.05) as usize).min(20);
                        xb[b].0 += g; xb[b].1 += 1;
                    }
                }
                // least squares over the texels for E = k·I·c·ndl·(1−x²)^p and a few other windows
                let mut pts: Vec<(f64, f64, f64)> = Vec::new(); // (x, ndl, E)
                for (ii, inst) in scene.instances.iter().enumerate() {
                    let dx = inst.xf[9] - lamp[0]; let dz = inst.xf[11] - lamp[1];
                    if (dx * dx + dz * dz).sqrt() > 60.0 { continue; }
                    let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                    let (x, y) = mp.pos[ci]; let (w, hh) = mp.size[ci];
                    let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (hh as u32 / 2).max(1));
                    let fb1 = mp.frame_bytes[1][ci];
                    let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                    for s in &samples {
                        if s.n[1] < 0.9 || (s.p[1] - y0).abs() > 1.5 { continue; }
                        let r = ((s.p[0] - lamp[0]).powi(2) + (s.p[2] - lamp[1]).powi(2)).sqrt();
                        let d = (r * r + h * h).sqrt();
                        let xx = d / r0;
                        if xx >= 1.2 { continue; }
                        let c1 = i1.get((px + s.px).min(i1.w - 1), (py + s.py).min(i1.h - 1));
                        let l1 = (0.2126 * lightmap::synth::decode_value(c1[0], fb1) as f64 + 0.7152 * lightmap::synth::decode_value(c1[1], fb1) as f64 + 0.0722 * lightmap::synth::decode_value(c1[2], fb1) as f64) * f1_max as f64;
                        pts.push((xx as f64, (h / d) as f64, l1));
                    }
                }
                let laws: Vec<(String, Box<dyn Fn(f64) -> f64>)> = {
                    let mut v: Vec<(String, Box<dyn Fn(f64) -> f64>)> = Vec::new();
                    for p in [1.0f64, 1.25, 1.5, 1.75, 2.0, 2.5] { v.push((format!("(1-x²)^{p}"), Box::new(move |x: f64| (1.0 - x * x).max(0.0).powf(p)))); }
                    v.push(("(1-x)²".into(), Box::new(|x: f64| (1.0 - x).max(0.0).powi(2))));
                    v.push(("(1-x)".into(), Box::new(|x: f64| (1.0 - x).max(0.0))));
                    v.push(("(1-x³)²".into(), Box::new(|x: f64| (1.0 - x * x * x).max(0.0).powi(2))));
                    v.push(("(1-x⁴)²".into(), Box::new(|x: f64| (1.0 - x.powi(4)).max(0.0).powi(2))));
                    v.push(("smoothstep(1,0,x)".into(), Box::new(|x: f64| { let t = x.clamp(0.0, 1.0); 1.0 - t * t * (3.0 - 2.0 * t) })));
                    v.push(("(1-x²)²/(1+x²)".into(), Box::new(|x: f64| (1.0 - x * x).max(0.0).powi(2) / (1.0 + x * x))));
                    v.push(("(1-x²)/(1+3x²)".into(), Box::new(|x: f64| (1.0 - x * x).max(0.0) / (1.0 + 3.0 * x * x))));
                    v
                };
                let my = pts.iter().map(|p| p.2).sum::<f64>() / pts.len().max(1) as f64;
                for (name, law) in &laws {
                    let (mut sxy, mut sxx) = (0.0, 0.0);
                    for (x, ndl, e) in &pts { let m = c0i as f64 * ndl * law(*x); sxy += m * e; sxx += m * m; }
                    let k = sxy / sxx.max(1e-12);
                    let (mut ssr, mut sst) = (0.0, 0.0);
                    for (x, ndl, e) in &pts { let p = k * c0i as f64 * ndl * law(*x); ssr += (e - p) * (e - p); sst += (e - my) * (e - my); }
                    println!("  law {name:>18}: k = {k:.3}  r² = {:.4}  ({} texels)", 1.0 - ssr / sst.max(1e-12), pts.len());
                }
                println!("{:>11} {:>5} {:>10}  {:>9} {:>9} {:>9} {:>9}", "x = d/R", "n", "g = E/(Ic·ndl)", "(1-x²)²", "1-x²", "(1-x)²", "1/x²-1");
                for (b, (s, n)) in xb.iter().enumerate() {
                    if *n == 0 { continue; }
                    let x = (b as f64 + 0.5) * 0.05;
                    println!("{:>4.2}–{:<4.2} {:>5} {:>10.4}  {:>9.4} {:>9.4} {:>9.4} {:>9.4}", b as f64 * 0.05, (b + 1) as f64 * 0.05, n, s / *n as f64, (1.0 - x * x).max(0.0).powi(2), (1.0 - x * x).max(0.0), (1.0 - x).max(0.0).powi(2), (1.0 / (x * x) - 1.0).max(0.0));
                }
            }
            // raw bytes of the charts within 12 m of the lamp: fb0, fb1 and the brightest frame-1 pixel
            for (ii, inst) in scene.instances.iter().enumerate() {
                let dx = inst.xf[9] - lamp[0]; let dz = inst.xf[11] - lamp[1];
                if (dx * dx + dz * dz).sqrt() > 12.0 { continue; }
                let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                let mut mx1 = 0u8; let mut mx0 = 0u8;
                for yy in 0..ph { for xx in 0..pw { let c = i1.get((px + xx).min(i1.w - 1), (py + yy).min(i1.h - 1)); mx1 = mx1.max(c[0]).max(c[1]).max(c[2]); let c0 = i0.get((px + xx).min(i0.w - 1), (py + yy).min(i0.h - 1)); mx0 = mx0.max(c0[0]).max(c0[1]).max(c0[2]); } }
                let mut sum0 = 0u64; let mut cnt = 0u64;
                for yy in 0..ph { for xx in 0..pw { let c0 = i0.get((px + xx).min(i0.w - 1), (py + yy).min(i0.h - 1)); sum0 += c0[1] as u64; cnt += 1; } }
                println!("  chart of item {} ({}) at ({:.0}, {:.0}): chart {ci} at atlas ({px}, {py}) {}×{} px, fb0 {} fb1 {} fb2 {}, brightest pixel frame0 {} frame1 {}, mean G frame0 {:.1}", inst.item, inst.model_name, inst.xf[9], inst.xf[11], pw, ph, mp.frame_bytes[0][ci], mp.frame_bytes[1][ci], mp.frame_bytes[2][ci], mx0, mx1, sum0 as f64 / cnt.max(1) as f64);
            }
            println!("{:>10} {:>6} {:>12} {:>28} {:>12}", "r (m)", "n", "frame1 lum", "frame1 rgb", "frame0 lum");
            for (b, (s1, rgb, s0, n)) in bins.iter().enumerate() {
                if *n == 0 { continue; }
                let nn = *n as f64;
                println!("{:>4.0}–{:<4.0} {:>6} {:>12.4} ({:.4}, {:.4}, {:.4}) {:>12.4}", b as f64 * 2.0, (b + 1) as f64 * 2.0, n, s1 / nn, rgb[0] / nn, rgb[1] / nn, rgb[2] / nn, s0 / nn);
            }
        }
        "shadeat" => {
            // lmtool shadeat MAP x,y,z nx,ny,nz [--cone-deg D] [--samples N]: the dome model's components at one point
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let v3 = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] };
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let bvh = lightmap::bvh::Bvh::build(lightmap::bake::world_tris(&scene));
            let mut prm = lightmap::bake::BakeParams::default();
            prm.dome_deg = f("--cone-deg").map(|s| s.parse().unwrap()).unwrap_or(40.0);
            prm.sky_samples = f("--samples").map(|s| s.parse().unwrap()).unwrap_or(4096);
            prm.sky = [1.0; 3]; prm.sun = [0.0; 3]; prm.direct_sun = 0.0; prm.bounce = 2.0; prm.albedo = 0.18; prm.ground_y = 8.0;
            if a.iter().any(|x| x == "--peel") {
                prm.peel = true;
                if let Ok(ps) = lightmap::dome::PointSets::load(&lightmap::dome::default_path()) { if let Some(set) = ps.set(256) { prm.sphere_dirs = std::sync::Arc::new(set.clone()); } }
            }
            let sh = lightmap::bake::shade_point(&scene, &bvh, &prm, v3(&a[2]), v3(&a[3]));
            println!("at {} n {}: sky share {:.4} (cone vis {:.4}), bounce {:?}, E {:?}", a[2], a[3], sh.sky_rgb[0], sh.sky_vis, sh.bounce, sh.e);
            // the geometry above: the first hit straight up and at 30° tilts
            let o = lightmap::geometry::add(v3(&a[2]), lightmap::geometry::mul(v3(&a[3]), 0.03));
            for tilt in [0.0f32, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0] {
                let mut line = format!("  tilt {tilt:>4.0}°:");
                for az in [0.0f32, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0] {
                    let (t, p) = (tilt.to_radians(), az.to_radians());
                    let d = [t.sin() * p.sin(), t.cos(), t.sin() * p.cos()];
                    match bvh.closest(o, d, 1.0e4) { Some(h) => line.push_str(&format!(" {:>6.1}", h.t)), None => line.push_str("   open") }
                }
                println!("{line}");
            }
        }
        "rastcheck" => {
            // lmtool rastcheck MAP ITEM_INDEX [W H]: how many rasterised samples land on each chart pixel of an
            // instance, and the normals of the duplicates (shared-uv top/bottom faces shade the same texel)
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let item: usize = a[2].parse().unwrap();
            let ii = scene.instances.iter().position(|i| i.item == item).expect("instance");
            let (w, h): (u32, u32) = (a.get(3).map(|s| s.parse().unwrap()).unwrap_or(64), a.get(4).map(|s| s.parse().unwrap()).unwrap_or(64));
            let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, w, h, false, true);
            let mut per: std::collections::HashMap<(u32, u32), Vec<[f32; 3]>> = Default::default();
            for s in &samples { per.entry((s.px, s.py)).or_default().push(s.n); }
            let dup = per.values().filter(|v| v.len() > 1).count();
            let mut up = 0; let mut down = 0; let mut side = 0;
            for s in &samples { if s.n[1] > 0.9 { up += 1 } else if s.n[1] < -0.9 { down += 1 } else { side += 1 } }
            println!("item {item} ({}): {} samples on {} pixels ({} pixels with several samples); normals up {up} down {down} side {side}", scene.instances[ii].model_name, samples.len(), per.len(), dup);
            let mut shown = 0;
            for (k, v) in &per { if v.len() > 1 && shown < 5 { shown += 1; println!("  pixel {:?}: normals {:?}", k, v.iter().map(|n| [(n[0] * 100.0).round() / 100.0, (n[1] * 100.0).round() / 100.0, (n[2] * 100.0).round() / 100.0]).collect::<Vec<_>>()); } }
            let ys: Vec<f32> = samples.iter().filter(|s| s.n[1] > 0.9).map(|s| s.p[1]).collect();
            if !ys.is_empty() { let (mn, mx) = ys.iter().fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y))); println!("  up-facing sample heights {mn:.3}..{mx:.3}"); }
        }
        "diff" => {
            // lmtool diff EDITOR.Map.Gbx OURS.Map.Gbx: the gate's lossless parts, field by field — chunk head, frame
            // records, the mapping (count, atlas size, bbox, per-chart (x, y, w, h), binds, f32s, per-frame bytes),
            // the other cache chunks, the probe trailer, and the blob sizes. Prints the differing entries and a
            // total of differing bytes in the lossless parts (0 = bit-identical). Exit 1 on any difference.
            let load = |p: &str| { let o = lightmap::mapio::load(p).expect("load"); let d = o.chunk.data.clone().expect("lightmap"); (o.chunk.version, o.chunk.u01, o.chunk.u02, d) };
            let (va, ua1, ua2, da) = load(&a[1]);
            let (vb, ub1, ub2, db) = load(&a[2]);
            let mut bytes_diff = 0usize;
            let mut note = |what: &str, same: bool, n: usize| { if !same { bytes_diff += n.max(1); println!("  DIFF {what}"); } };
            note(&format!("chunk head version/u01/u02 {va}/{ua1}/{ua2} vs {vb}/{ub1}/{ub2}"), (va, ua1, ua2) == (vb, ub1, ub2), 12);
            note(&format!("lightmap version {} vs {}", da.lightmap_version, db.lightmap_version), da.lightmap_version == db.lightmap_version, 4);
            note(&format!("frame count {} vs {}", da.frames.len(), db.frames.len()), da.frames.len() == db.frames.len(), 4);
            for (fi, (fa, fb)) in da.frames.iter().zip(db.frames.iter()).enumerate() {
                for (ii, (ia, ib)) in fa.images.iter().zip(fb.images.iter()).enumerate() {
                    if ia.len() != ib.len() { println!("  info: frame {fi} image {ii}: {} vs {} bytes (lossy — compared by texels elsewhere)", ia.len(), ib.len()); }
                }
            }
            let (ma, mb) = (da.cache.mapping().expect("mapping A"), db.cache.mapping().expect("mapping B"));
            // frame records: the 66-byte records in the head
            for i in 0..3 {
                let r = 60 + 66 * i;
                if r + 66 <= ma.head.len() && r + 66 <= mb.head.len() {
                    let (ra, rb) = (&ma.head[r..r + 66], &mb.head[r..r + 66]);
                    if ra != rb {
                        let fields = ["Bump", "u0", "DayTime", "ReplayTime", "MaxHDR_Mood", "MaxHDR", "Bounce", "Sky", "SkyUseClouds", "f16×3", "f16/StoreLAmbient", "LocalLight_Storage", "LocalLight_Switch", "LAmbient.r", "LAmbient.g", "LAmbient.b"];
                        let mut d = Vec::new();
                        for (k, name) in fields.iter().enumerate() { let o = k * 4; if o + 4 <= 66 && ra[o..o + 4] != rb[o..o + 4] { d.push(format!("{name}: {:02x?} vs {:02x?}", &ra[o..o + 4], &rb[o..o + 4])); } }
                        note(&format!("frame record {i}: {}", d.join("; ")), false, ra.iter().zip(rb).filter(|(x, y)| x != y).count());
                    }
                }
            }
            note(&format!("head bytes 0..60 (constants) differ"), ma.head[..60.min(ma.head.len())] == mb.head[..60.min(mb.head.len())], 60);
            note(&format!("head tail (after the records) differs"), ma.head.get(258..) == mb.head.get(258..), 12);
            note(&format!("mapping u01/atlas/u02/u03 {}/{}x{}/{}/{} vs {}/{}x{}/{}/{}", ma.m_u01, ma.atlas_w, ma.atlas_h, ma.m_u02, ma.m_u03, mb.m_u01, mb.atlas_w, mb.atlas_h, mb.m_u02, mb.m_u03), (ma.m_u01, ma.atlas_w, ma.atlas_h, ma.m_u02, ma.m_u03) == (mb.m_u01, mb.atlas_w, mb.atlas_h, mb.m_u02, mb.m_u03), 20);
            note(&format!("bbox {:?}..{:?} vs {:?}..{:?}", ma.bbox_min, ma.bbox_max, mb.bbox_min, mb.bbox_max), ma.bbox_min == mb.bbox_min && ma.bbox_max == mb.bbox_max, 24);
            note(&format!("chart count {} vs {}", ma.count, mb.count), ma.count == mb.count, 4);
            // per chart, matched by object id
            let index = |m: &lightmap::format::Mapping| -> std::collections::HashMap<(u32, u32), usize> { (0..m.count as usize).map(|i| ((m.binds[i].obj_group_idx, m.binds[i].obj_idx), i)).collect() };
            let (ia, ib) = (index(ma), index(mb));
            let (mut pos_d, mut size_d, mut f32_d, mut fb_d, mut missing) = (0usize, 0usize, 0usize, 0usize, 0usize);
            let mut shown = 0;
            for (key, &i) in &ia {
                let Some(&j) = ib.get(key) else { missing += 1; continue };
                if ma.size[i] != mb.size[j] { size_d += 1; }
                if ma.pos[i] != mb.pos[j] { pos_d += 1; }
                if ma.chart_f32[i].to_bits() != mb.chart_f32[j].to_bits() { f32_d += 1; }
                for fr in 0..ma.frame_bytes.len().min(mb.frame_bytes.len()) { if ma.frame_bytes[fr][i] != mb.frame_bytes[fr][j] { fb_d += 1; } }
                if (ma.size[i] != mb.size[j] || ma.pos[i] != mb.pos[j]) && shown < 8 { shown += 1; println!("  chart obj {}: A {}×{} at {:?}, B {}×{} at {:?}", key.0 / 4, ma.size[i].0, ma.size[i].1, ma.pos[i], mb.size[j].0, mb.size[j].1, mb.pos[j]); }
            }
            let order_same = ma.binds == mb.binds;
            note(&format!("bind order/ids identical"), order_same, if order_same { 0 } else { 8 * ma.count as usize });
            note(&format!("{missing} charts of A missing in B"), missing == 0, missing * 16);
            note(&format!("{size_d} chart sizes differ"), size_d == 0, size_d * 4);
            note(&format!("{pos_d} chart positions differ"), pos_d == 0, pos_d * 4);
            note(&format!("{f32_d} chart f32s differ"), f32_d == 0, f32_d * 4);
            note(&format!("{fb_d} per-frame chart bytes differ (the sqrt-domain chart max; lossy-adjacent)"), fb_d == 0, fb_d);
            note(&format!("mapping tail {} vs {} bytes", ma.tail.len(), mb.tail.len()), ma.tail == mb.tail, ma.tail.len().max(mb.tail.len()));
            // the other cache chunks
            for (ca, cb) in da.cache.chunks.iter().zip(db.cache.chunks.iter()) {
                if ca.id != cb.id { note(&format!("cache chunk id {:#x} vs {:#x}", ca.id, cb.id), false, 4); continue; }
                if let (lightmap::format::ChunkBody::Raw(x), lightmap::format::ChunkBody::Raw(y)) = (&ca.body, &cb.body) {
                    let hex = |b: &[u8]| b.iter().map(|v| format!("{v:02x}")).collect::<Vec<_>>().join(" ");
                    let f32s = |b: &[u8]| b.chunks(4).filter(|c| c.len() == 4).map(|c| { let u = u32::from_le_bytes([c[0], c[1], c[2], c[3]]); let f = f32::from_bits(u); if f.is_finite() && f.abs() > 1e-6 && f.abs() < 1e6 { format!("{f:.4}") } else { format!("{u}") } }).collect::<Vec<_>>().join(", ");
                    note(&format!("cache chunk {:#x} raw body {} vs {} bytes\n       A [{}] = ({})\n       B [{}] = ({})", ca.id, x.len(), y.len(), hex(x), f32s(x), hex(y), f32s(y)), x == y, x.len().max(y.len()));
                }
            }
            note(&format!("cache chunk count {} vs {}", da.cache.chunks.len(), db.cache.chunks.len()), da.cache.chunks.len() == db.cache.chunks.len(), 4);
            note(&format!("probe trailer {} vs {} bytes", da.cache.trailer.len(), db.cache.trailer.len()), da.cache.trailer == db.cache.trailer, da.cache.trailer.iter().zip(db.cache.trailer.iter()).filter(|(x, y)| x != y).count().max((da.cache.trailer.len() as i64 - db.cache.trailer.len() as i64).unsigned_abs() as usize));
            println!("lossless parts: {bytes_diff} differing bytes{}", if bytes_diff == 0 { " — BIT-IDENTICAL" } else { "" });
            if bytes_diff > 0 { std::process::exit(1); }
        }
        "skygrad" => {
            // lmtool skygrad SkyColor.dds: decode the mood's sky gradient and print its row profile
            let g = lightmap::skygrad::SkyGradient::load(&a[1]).expect("SkyColor");
            println!("{}: {}×{}", a[1].rsplit('/').next().unwrap(), g.w, g.h);
            println!("{}", g.profile());
            // the column profile of the row through the brightest texel (the sun glow's u)
        }
        "skyprofile" => {
            // lmtool skyprofile CUBE.dds: radiance by elevation band (mean over azimuth) and the share of the
            // up-facing irradiance that comes from above 60° / 30° elevation — is the sky zenith-concentrated?
            let cube = lightmap::skycube::CubeMap::load(&a[1]).expect("cube");
            let mut bands = vec![([0f64; 3], 0usize); 9]; // −90..90 in 20° bands
            let (mut e_up, mut e_up60, mut e_up30) = ([0f64; 3], [0f64; 3], [0f64; 3]);
            let n = 400usize;
            for i in 0..n { for j in 0..(2 * n) {
                let el = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * (i as f64 + 0.5) / n as f64;
                let az = 2.0 * std::f64::consts::PI * (j as f64 + 0.5) / (2 * n) as f64;
                let d = [(el.cos() * az.sin()) as f32, el.sin() as f32, (el.cos() * az.cos()) as f32];
                let l = cube.sample(d);
                let b = (((el.to_degrees() + 90.0) / 20.0) as usize).min(8);
                for k in 0..3 { bands[b].0[k] += l[k] as f64; } bands[b].1 += 1;
                if el > 0.0 {
                    let w = el.sin() * el.cos() * (std::f64::consts::PI / n as f64) * (std::f64::consts::PI / n as f64); // cosθ dω
                    for k in 0..3 { e_up[k] += l[k] as f64 * w; if el.to_degrees() > 60.0 { e_up60[k] += l[k] as f64 * w; } if el.to_degrees() > 30.0 { e_up30[k] += l[k] as f64 * w; } }
                }
            } }
            println!("{}", a[1].rsplit('/').next().unwrap());
            for (b, (s, c)) in bands.iter().enumerate() { if *c > 0 { let cc = *c as f64; println!("  el {:>4}..{:<4} mean L ({:.3}, {:.3}, {:.3})", b as i32 * 20 - 90, b as i32 * 20 - 70, s[0] / cc, s[1] / cc, s[2] / cc); } }
            let lum = |e: [f64; 3]| 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
            println!("  E(up) = ({:.3}, {:.3}, {:.3}) lum {:.3}; share from el > 30°: {:.2}, el > 60°: {:.2}", e_up[0], e_up[1], e_up[2], lum(e_up), lum(e_up30) / lum(e_up).max(1e-9), lum(e_up60) / lum(e_up).max(1e-9));
            for (name, nrm) in [("north wall (+z)", [0f32, 0.0, 1.0]), ("east wall (+x)", [1.0, 0.0, 0.0]), ("south wall", [0.0, 0.0, -1.0]), ("west wall", [-1.0, 0.0, 0.0])] {
                let e = cube.irradiance(nrm);
                println!("  E({name}) = ({:.3}, {:.3}, {:.3}) lum {:.3} = {:.2} of up", e[0], e[1], e[2], lum(e), lum(e) / lum(e_up).max(1e-9));
            }
        }
        "sunaz" => {
            // lmtool sunaz MAP [--base N] [--items N]: over the whole bake, vertical texels binned by the azimuth of
            // their normal (mean absolute HDR and colour) — the brightest bin faces the baked sun; plus the
            // horizontal texels' mean, the p95 of both, and the colour of the brightest 5 % of vertical texels
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let fm = d.cache.frame_max_hdr().unwrap_or(1.0);
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let step = (scene.instances.len() / f("--items").map(|s| s.parse().unwrap()).unwrap_or(3000)).max(1);
            let mut bins = vec![(0f64, [0f64; 3], 0usize); 12];
            let mut vert: Vec<(f32, [f32; 3])> = Vec::new();
            let mut horiz: Vec<f32> = Vec::new();
            for (ii, inst) in scene.instances.iter().enumerate().step_by(step) {
                let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                if pw < 2 || ph < 2 { continue; }
                let fb = mp.frame_bytes[0][ci];
                if fb == 0 { continue; }
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                for s in samples.iter().step_by(3) {
                    let c = ia.get((px + s.px).min(ia.w - 1), (py + s.py).min(ia.h - 1));
                    let rgb = [lightmap::synth::decode_value(c[0], fb) * fm, lightmap::synth::decode_value(c[1], fb) * fm, lightmap::synth::decode_value(c[2], fb) * fm];
                    let lum = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
                    if s.n[1].abs() < 0.3 {
                        let az = s.n[0].atan2(s.n[2]).to_degrees().rem_euclid(360.0);
                        let b = ((az / 30.0) as usize).min(11);
                        bins[b].0 += lum as f64; for k in 0..3 { bins[b].1[k] += rgb[k] as f64; } bins[b].2 += 1;
                        vert.push((lum, rgb));
                    } else if s.n[1] > 0.9 {
                        horiz.push(lum);
                    }
                }
            }
            let hdr = tmmaps::header::read(&a[1]).expect("header");
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let dt = lightmap::mapio::daytime(&own.gbx.body);
            println!("{}: {} {} daytime {:?} mood {} frame MaxHDR {fm:.3}; {} vertical / {} horizontal texels sampled", a[1].rsplit('/').next().unwrap(), hdr.envir, mf.decoration_id, dt.map(|t| format!("{:.3}", t as f32 / 65536.0)), lightmap::moods::effective_mood(&mf.decoration_id, dt), vert.len(), horiz.len());
            horiz.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if !horiz.is_empty() { println!("  horizontal: mean {:.3} median {:.3} p95 {:.3}", horiz.iter().sum::<f32>() / horiz.len() as f32, horiz[horiz.len() / 2], horiz[horiz.len() * 95 / 100]); }
            vert.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            if !vert.is_empty() {
                let top = &vert[vert.len() * 95 / 100..];
                let mut c = [0f32; 3]; for (_, rgb) in top { for k in 0..3 { c[k] += rgb[k]; } }
                let n = top.len() as f32;
                println!("  vertical: median {:.3} p95 {:.3}; brightest 5 % mean rgb ({:.3}, {:.3}, {:.3}) = hue ({:.2}, {:.2}, {:.2})", vert[vert.len() / 2].0, vert[vert.len() * 95 / 100].0, c[0] / n, c[1] / n, c[2] / n, 1.0, c[1] / c[0].max(1e-6), c[2] / c[0].max(1e-6));
            }
            for (b, (s, rgb, n)) in bins.iter().enumerate() { if *n > 0 { let nn = *n as f64; println!("  az {:>3}–{:<3} n {:>7}  lum {:.4}  rgb ({:.3}, {:.3}, {:.3})", b * 30, (b + 1) * 30, n, s / nn, rgb[0] / nn, rgb[1] / nn, rgb[2] / nn); } }
        }
        "webpcmp" => {
            // lmtool webpcmp MAP [--q 91] [--image 0]: re-encode the map's frame-0 image with our libwebp at --q and
            // compare the VP8 frame header (segment quantizers, filter) and the size with the editor's bytes
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let q: f32 = f("--q").map(|s| s.parse().unwrap()).unwrap_or(91.0);
            let idx: usize = f("--image").map(|s| s.parse().unwrap()).unwrap_or(0);
            let own = lightmap::mapio::load(&a[1]).expect("load");
            let d = own.chunk.data.as_ref().unwrap();
            let src = &d.frames[0].images[idx];
            // the first RIFF of a possibly concatenated blob
            let src0: Vec<u8> = if src.len() > 12 && &src[..4] == b"RIFF" { let sz = u32::from_le_bytes([src[4], src[5], src[6], src[7]]) as usize + 8; src[..sz.min(src.len())].to_vec() } else { src.clone() };
            let im = lightmap::img::decode_webp(&src0).expect("decode");
            println!("libwebp {} linked: {}", lightmap::webpenc::version(), lightmap::webpenc::available());
            let ours = lightmap::webpenc::encode_rgb(&im.px, im.w, im.h, q).expect("libwebp");
            let hdr = |b: &[u8]| -> String { let p = b.iter().position(|&x| x == b'V').map(|i| i).unwrap_or(0); let start = p + 8; b[start..(start + 24).min(b.len())].iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ") };
            println!("editor image {idx}: {} bytes, VP8 frame head: {}", src0.len(), hdr(&src0));
            println!("ours q{q}:      {} bytes, VP8 frame head: {}", ours.len(), hdr(&ours));
            let same = src0.len() == ours.len() && src0 == ours;
            let common = src0.iter().zip(ours.iter()).take_while(|(a, b)| a == b).count();
            println!("byte-identical: {same}; common prefix {common} bytes");
            let back = lightmap::img::decode_webp(&ours).expect("decode ours");
            let mut diff = 0u64; let mut maxd = 0u8;
            for (a, b) in im.px.iter().zip(back.px.iter()) { let dd = a.abs_diff(*b); diff += dd as u64; maxd = maxd.max(dd); }
            println!("decoded difference vs the editor's decoded image: mean {:.3} levels, max {maxd}", diff as f64 / im.px.len() as f64);
        }
        "cacheuid" => {
            // lmtool cacheuid MAP [--out OUT [--set HEX|fresh]]: the header's lightmapCacheUID (chunk 0x03043003:
            // … string mapStyle, u64 lightmapCacheUID, u8 lightmapVersion, string titleId). The game keys its
            // C:\ProgramData\Trackmania\Cache\*.Bump.LightMap.zip entries by it — a stale entry for the uid
            // makes the editor re-open the cached bake (and ITS time of day) instead of the map's word.
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let hb = m.gbx.header_bytes_u();
            // locate: the titleId Id = u32 0x40000000 + u32 len + "TMStadium", preceded by u8 lightmapVersion
            // and the u64 lightmapCacheUID
            let title = b"TMStadium";
            let mut pos = None;
            for i in 0..hb.len().saturating_sub(title.len() + 4) {
                if &hb[i + 4..i + 4 + title.len()] == title && u32::from_le_bytes([hb[i], hb[i + 1], hb[i + 2], hb[i + 3]]) as usize == title.len() { pos = Some(i); break; }
            }
            let Some(p) = pos else { println!("no titleId in the header"); return };
            let p = p - 4; // the Id flag word
            if hb[p..p + 4] != [0, 0, 0, 0x40] { println!("unexpected Id word before titleId at {p:#x}"); return }
            let ver = hb[p - 1];
            let uid = u64::from_le_bytes(hb[p - 9..p - 1].try_into().unwrap());
            println!("lightmapCacheUID {uid:#018x} lightmapVersion {ver}");
            if a.iter().any(|x| x == "--hex") {
                let lo = p.saturating_sub(48);
                for row in (lo..p + 16).step_by(16) {
                    let bytes: Vec<String> = hb[row..(row + 16).min(hb.len())].iter().map(|b| format!("{b:02x}")).collect();
                    let ascii: String = hb[row..(row + 16).min(hb.len())].iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                    println!("  {row:06x}: {}  {ascii}", bytes.join(" "));
                }
            }
            if let (Some(out), Some(v)) = (f("--out"), f("--set")) {
                let nv: u64 = if v == "fresh" {
                    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
                    (nanos as u64) ^ 0x9E37_79B9_7F4A_7C15
                } else { u64::from_str_radix(v.trim_start_matches("0x"), 16).unwrap() };
                let mut nh = hb.clone();
                nh[p - 9..p - 1].copy_from_slice(&nv.to_le_bytes());
                // rebuild the file: patched header + the original body
                let mut file = nh.clone();
                file.extend_from_slice(&m.gbx.body);
                let t = tmmaps::gbx::Gbx::parse(&file);
                std::fs::write(&out, t.write_body_recompressed(&m.gbx.body)).expect("write");
                println!("wrote {out} with lightmapCacheUID {nv:#018x}");
            }
        }
        "packtest" => {
            // lmtool packtest MAP [--base N] [--w 1024] [--g 1] [--iter 8] [--tile-ext M]: run the editor's chart
            // allocation walk (§3.1) on the map's items (ext = uv bounds × MeterByUv) + the zone tiles, and compare
            // the sizes/positions with the map's own (editor) chart table
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            // the layout side per quality: 2048 for VFast..High, 4096 for Ultra (RE child 2); g/pad/m derive from it
            let w_atlas: u16 = f("--w").map(|s| s.parse().unwrap()).unwrap_or(2048);
            let (g0, pad0, m0) = lightmap::pack::layout_params(w_atlas, w_atlas);
            let g: u16 = f("--g").map(|s| s.parse().unwrap()).unwrap_or(g0);
            let pad: u32 = f("--pad").map(|s| s.parse().unwrap()).unwrap_or(pad0 as u32);
            let mmin: u16 = f("--m").map(|s| s.parse().unwrap()).unwrap_or(m0);
            println!("layout {w_atlas}: g {g} pad {pad} min {mmin}");
            let max_iter: u32 = f("--iter").map(|s| s.parse().unwrap()).unwrap_or(8);
            let tile_ext: f32 = f("--tile-ext").map(|s| s.parse().unwrap()).unwrap_or(0.0);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            // the editor's table: object id → (x, y, w, h) in 2048 units
            let mut ed: std::collections::HashMap<u32, (u16, u16, u16, u16)> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; ed.insert(obj, (mp.pos[i].0, mp.pos[i].1, mp.size[i].0, mp.size[i].1)); }
            let n_tiles = ed.keys().filter(|&&o| o < base).count();
            if a.iter().any(|x| x == "--tile-order") {
                // the editor's tile charts by atlas row then column: the object-id pattern reveals the placement order
                let mut tiles: Vec<(u32, (u16, u16, u16, u16))> = ed.iter().filter(|(&o, _)| o < base).map(|(&o, &r)| (o, r)).collect();
                tiles.sort_by_key(|(_, r)| (r.1, r.0));
                for (o, r) in tiles.iter().take(40) { println!("  tile obj {o:>5} (cell x {:>2} z {:>2}) at ({:>4}, {:>4}) {}×{}", o % 64, o / 64, r.0, r.1, r.2, r.3); }
                return;
            }
            {
                let mut hist: std::collections::BTreeMap<(u16, u16), usize> = Default::default();
                for (&o, &(_, _, w, h)) in &ed { if o < base { *hist.entry((w, h)).or_default() += 1; } }
                println!("editor tile chart sizes: {:?}", hist);
                println!("editor atlas {}×{}, m_u01 {}, m_u02 {}, m_u03 {}", mp.atlas_w, mp.atlas_h, mp.m_u01, mp.m_u02, mp.m_u03);
            }
            // our chart list in IdForLightMap order: tiles (ids 0..base) then items (base + item)
            let mut charts: Vec<lightmap::pack::ChartExt> = Vec::new();
            let mut ids: Vec<u32> = Vec::new();
            for o in 0..base { if ed.contains_key(&o) { charts.push(lightmap::pack::ChartExt { ext: [tile_ext, tile_ext], mins: [1, 1] }); ids.push(o); } }
            for inst in &scene.instances {
                let m = &scene.models[inst.model];
                let sc = ((inst.xf[0] * inst.xf[0] + inst.xf[1] * inst.xf[1] + inst.xf[2] * inst.xf[2]) as f32).sqrt();
                let ext = match m.plg_bounds { Some(b) => [(b[2] - b[0]) * m.plg_u02 * sc, (b[3] - b[1]) * m.plg_u02 * sc], None => [0.0, 0.0] };
                charts.push(lightmap::pack::ChartExt { ext, mins: [1, 1] });
                ids.push(base + inst.item as u32);
            }
            let sum_area: f32 = charts.iter().map(|c| c.ext[0] * c.ext[1]).sum();
            println!("{} charts ({n_tiles} tiles in the editor's table, {} items), Σarea {sum_area:.1} m², W {w_atlas} g {g} maxIter {max_iter}", charts.len(), scene.instances.len());
            {
                // the biggest extents (uv-tiled models blow the area up)
                let mut big: Vec<(f32, usize)> = charts.iter().enumerate().map(|(k, c)| (c.ext[0] * c.ext[1], k)).collect();
                big.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                for &(area, k) in big.iter().take(6) {
                    if ids[k] >= base { let inst = scene.instances.iter().find(|i| i.item as u32 == ids[k] - base).unwrap(); let mdl = &scene.models[inst.model]; println!("  ext ({:.1}, {:.1}) m = {area:.0} m²: item {} model {} (MeterByUv {:.4}, uv bounds {:?}, metres_per_uv {:?}); editor chart {:?}", charts[k].ext[0], charts[k].ext[1], ids[k] - base, inst.model_name, mdl.plg_u02, mdl.plg_bounds, mdl.metres_per_uv, ed.get(&ids[k])); }
                }
            }
            // --scan: the size-match count against the editor's table over a grid of s (which s did the editor use?)
            if a.iter().any(|x| x == "--scan") {
                let order = lightmap::pack::area_order(&charts);
                let mut best = (0usize, 0f32);
                let mut s = 8.0f32;
                while s < 12.0 {
                    if let Some(p) = lightmap::pack::try_pack(&charts, &order, s, 8192, 8192, g, mmin) {
                        let sz = |q: &lightmap::pack::Placed| if pad > 0 { ((q.w as u32).saturating_sub(2 * pad), (q.h as u32).saturating_sub(2 * pad)) } else { (2 * (q.w as u32).saturating_sub(1), 2 * (q.h as u32).saturating_sub(1)) };
                        let ok = p.iter().enumerate().filter(|(k, q)| ed.get(&ids[*k]).map(|&(_, _, ew, eh)| sz(q) == (ew as u32, eh as u32)).unwrap_or(false)).count();
                        if ok > best.0 { best = (ok, s); }
                        if (s * 1000.0).round() as i32 % 100 == 0 { println!("  s {s:.3}: {ok} sizes equal"); }
                    }
                    s += 0.004;
                }
                println!("best: s {:.3} with {} of {} sizes equal", best.1, best.0, charts.len());
                return;
            }
            // --tie KEY: the order among equal areas (the editor sorts by block position-like keys before the
            // area): index (default), x, z, y, -x, -z, -y, or combos like "z,x" (last key = most significant)
            let tie = f("--tie").unwrap_or_else(|| "index".into());
            let placed_order: Vec<usize> = {
                let mut idx: Vec<usize> = (0..charts.len()).collect();
                // the keys are the block's world bbox CENTRE (RE child 2): items from their transformed
                // triangles, tiles from their cell (x-major or z-major: --tiles-zmajor) at the sea height
                let tile_zmajor = a.iter().any(|x| x == "--tiles-zmajor");
                let tile_y: f32 = f("--tile-y").map(|s| s.parse().unwrap()).unwrap_or(8.0);
                let centres: std::collections::HashMap<u32, [f32; 3]> = scene.instances.iter().map(|inst| {
                    let mdl = &scene.models[inst.model];
                    let mut lo = [f32::MAX; 3]; let mut hi = [f32::MIN; 3];
                    for t in &mdl.tris { for p in &t.p { let w = lightmap::geometry::xf_point(&inst.xf, *p); for k in 0..3 { lo[k] = lo[k].min(w[k]); hi[k] = hi[k].max(w[k]); } } }
                    (base + inst.item as u32, [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0])
                }).collect();
                let pos_of = |k: usize| -> [f32; 3] { if ids[k] >= base { centres.get(&ids[k]).copied().unwrap_or([0.0; 3]) } else { let o = ids[k]; let (cx, cz) = if tile_zmajor { (o / 64, o % 64) } else { (o % 64, o / 64) }; [cx as f32 * 16.0 + 8.0, tile_y, cz as f32 * 16.0 + 8.0] } };
                for key in tie.split(',') {
                    let (neg, axis) = match key.trim() { "x" => (false, 0), "y" => (false, 1), "z" => (false, 2), "-x" => (true, 0), "-y" => (true, 1), "-z" => (true, 2), _ => (false, 9) };
                    if axis < 3 { idx.sort_by(|&a, &b| { let (pa, pb) = (pos_of(a)[axis], pos_of(b)[axis]); let o = pa.partial_cmp(&pb).unwrap(); if neg { o.reverse() } else { o } }); }
                }
                // the area sort last (stable)
                idx.sort_by_key(|&i| (charts[i].ext[0] * charts[i].ext[1]).to_bits());
                idx
            };
            let Some((s, placed)) = lightmap::pack::allocate_ordered(&charts, &placed_order, w_atlas, w_atlas, g, mmin, max_iter) else { println!("allocation failed"); return };
            println!("s_final {s:.4} texels/m (tie {tie})");
            let (mut n_cmp, mut size_ok, mut pos_ok) = (0, 0, 0);
            let mut shown = 0;
            for (k, p) in placed.iter().enumerate() {
                let Some(&(ex, ey, ew, eh)) = ed.get(&ids[k]) else { continue };
                n_cmp += 1;
                let (ox, oy, ow, oh) = if pad > 0 { (p.x as u32 + pad, p.y as u32 + pad, (p.w as u32).saturating_sub(2 * pad), (p.h as u32).saturating_sub(2 * pad)) } else { (2 * p.x as u32 + 1, 2 * p.y as u32 + 1, 2 * (p.w as u32).saturating_sub(1), 2 * (p.h as u32).saturating_sub(1)) };
                let s_ok = ow == ew as u32 && oh == eh as u32;
                let p_ok = s_ok && ox == ex as u32 && oy == ey as u32;
                size_ok += s_ok as u32; pos_ok += p_ok as u32;
                if !s_ok && shown < 12 && ids[k] >= base { shown += 1; println!("  item {} ext ({:.2}, {:.2}) m: ours {}×{} at ({}, {}) vs editor {}×{} at ({}, {})", ids[k] - base, charts[k].ext[0], charts[k].ext[1], ow, oh, ox, oy, ew, eh, ex, ey); }
            }
            let items_ok = placed.iter().enumerate().filter(|(k, p)| ids[*k] >= base && ed.get(&ids[*k]).map(|&(ex, ey, ew, eh)| { let (ox, oy, ow, oh) = if pad > 0 { (p.x as u32 + pad, p.y as u32 + pad, (p.w as u32).saturating_sub(2 * pad), (p.h as u32).saturating_sub(2 * pad)) } else { (2 * p.x as u32 + 1, 2 * p.y as u32 + 1, 2 * (p.w as u32).saturating_sub(1), 2 * (p.h as u32).saturating_sub(1)) }; (ox, oy, ow, oh) == (ex as u32, ey as u32, ew as u32, eh as u32) }).unwrap_or(false)).count();
            println!("compared {n_cmp}: sizes equal {size_ok}, positions equal {pos_ok} (items fully equal: {items_ok} of {})", scene.instances.len());
        }
        "points" => {
            // lmtool points [FILE]: the game's sphere point sets — set sizes, and the zenith cone counts at 30°
            let p = a.get(1).cloned().unwrap_or_else(lightmap::dome::default_path);
            let ps = lightmap::dome::PointSets::load(&p).expect("point sets");
            println!("{} sets: {:?}", ps.sets.len(), ps.sets.iter().map(|s| s.len()).collect::<Vec<_>>());
            for n in [256usize, 512, 1032, 2040, 4112, 8192] {
                let c = ps.cone(n, 30.0);
                let mean_y = c.iter().map(|p| p[1] as f64).sum::<f64>() / c.len().max(1) as f64;
                let norms: Vec<f32> = ps.set(n).map(|s| s.iter().map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()).collect()).unwrap_or_default();
                let (nmin, nmax) = norms.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
                println!("  set {n}: {} points within 30° of +y (mean cosθ {mean_y:.4}; expected uniform (1+cos30)/2 = {:.4}); |p| {nmin:.4}..{nmax:.4}", c.len(), (1.0 + 30f64.to_radians().cos()) / 2.0);
            }
        }
        "conefit" => {
            // lmtool conefit R1:V1,R2:V2,…: the editor's response under a 16×16 roof 12 m up (pad value at distance R
            // from the roof centre over the open value) against dome models — a cosine-weighted cone of half-angle
            // θc around the zenith, and a cos^k-weighted hemisphere — prints the profile per model and its RMS error
            let obs: Vec<(f64, f64)> = a[1].split(',').map(|t| { let (r, v) = t.split_once(':').unwrap(); (r.parse().unwrap(), v.parse().unwrap()) }).collect();
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let roof_h: f64 = f("--roof-h").map(|s| s.parse().unwrap()).unwrap_or(12.0); let half: f64 = f("--half").map(|s| s.parse().unwrap()).unwrap_or(8.0);
            let vis = |r: f64, weight: &dyn Fn(f64) -> f64| -> f64 {
                // cosine-weighted directions over the hemisphere on a fine grid; occluded if the ray hits the roof square
                let (n_th, n_ph) = (180usize, 360usize);
                let (mut num, mut den) = (0.0, 0.0);
                for i in 0..n_th {
                    let th = (i as f64 + 0.5) / n_th as f64 * std::f64::consts::FRAC_PI_2;
                    let w = weight(th) * th.cos() * th.sin();
                    for j in 0..n_ph {
                        let ph = (j as f64 + 0.5) / n_ph as f64 * 2.0 * std::f64::consts::PI;
                        let (dx, dz) = (th.sin() * ph.cos(), th.sin() * ph.sin());
                        let t = roof_h / th.cos().max(1e-6);
                        let (hx, hz) = (r + dx * t, dz * t);
                        let blocked = hx.abs() <= half && hz.abs() <= half;
                        den += w;
                        if !blocked { num += w; }
                    }
                }
                num / den
            };
            let mut models: Vec<(String, Box<dyn Fn(f64) -> f64>)> = Vec::new();
            for tc in [25.0f64, 27.5, 30.0, 32.5, 35.0, 40.0, 45.0, 60.0, 90.0] { let c = tc.to_radians(); models.push((format!("cone {tc}°"), Box::new(move |th: f64| if th <= c { 1.0 } else { 0.0 }))); }
            for (tc, soft) in [(30.0f64, 10.0f64), (35.0, 10.0), (35.0, 20.0), (40.0, 20.0)] { let (c, s) = (tc.to_radians(), soft.to_radians()); models.push((format!("cone {tc}°±{soft}"), Box::new(move |th: f64| ((c + s / 2.0 - th) / s).clamp(0.0, 1.0)))); }
            for k in [1.0f64, 2.0, 4.0, 6.0, 8.0, 12.0] { models.push((format!("cos^{k}"), Box::new(move |th: f64| th.cos().powf(k)))); }
            println!("{:<10} {}", "model", obs.iter().map(|(r, _)| format!("{r:>6.1}")).collect::<Vec<_>>().join(" "));
            println!("{:<10} {}", "observed", obs.iter().map(|(_, v)| format!("{v:>6.3}")).collect::<Vec<_>>().join(" "));
            for (name, w) in &models {
                let pred: Vec<f64> = obs.iter().map(|(r, _)| vis(*r, w)).collect();
                let rms = (obs.iter().zip(&pred).map(|((_, v), p)| (v - p) * (v - p)).sum::<f64>() / obs.len() as f64).sqrt();
                println!("{:<10} {}  rms {rms:.3}", name, pred.iter().map(|p| format!("{p:>6.3}")).collect::<Vec<_>>().join(" "));
            }
        }
        "testmap2" => {
            // lmtool testmap2 MAP [--x0 1000 --z0 1000 --y0 160] [--base N]: the per-collection layout — 3×3 tiles (16 m)
            // at (x0.., z0..) with a tile 16 m above the centre, a second 3×3 at (x0+48.., z0..) with a tile 8 m above
            // its centre, a block east of it at (x0+104, z0+16); prints the regions' values in frame-MaxHDR units and
            // the roof profiles (ratio to the open tiles)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let x0: f32 = f("--x0").map(|s| s.parse().unwrap()).unwrap_or(1000.0);
            let z0: f32 = f("--z0").map(|s| s.parse().unwrap()).unwrap_or(1000.0);
            let y0: f32 = f("--y0").map(|s| s.parse().unwrap()).unwrap_or(160.0);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).unwrap();
            let fm = d.cache.frame_max_hdr().unwrap_or(1.0);
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            struct Tx { p: [f32; 3], n: [f32; 3], rgb: [f32; 3], lum: f32 }
            let mut tx: Vec<Tx> = Vec::new();
            for (ii, inst) in scene.instances.iter().enumerate() {
                let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                let (x, y) = mp.pos[ci]; let (w, h) = mp.size[ci];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                let fbv = mp.frame_bytes[0][ci];
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pw, ph, false, true);
                for s in &samples {
                    let c = ia.get((px + s.px).min(ia.w - 1), (py + s.py).min(ia.h - 1));
                    let rgb = [lightmap::synth::decode_value(c[0], fbv) * fm, lightmap::synth::decode_value(c[1], fbv) * fm, lightmap::synth::decode_value(c[2], fbv) * fm];
                    tx.push(Tx { p: s.p, n: s.n, rgb, lum: 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2] });
                }
            }
            println!("{}: frame MaxHDR {fm:.4}, {} texels (absolute HDR below)", a[1].rsplit('/').next().unwrap(), tx.len());
            let stats = |name: &str, sel: &dyn Fn(&Tx) -> bool| -> f32 {
                let v: Vec<&Tx> = tx.iter().filter(|t| sel(t)).collect();
                if v.is_empty() { println!("{name:<40} (no texels)"); return 0.0; }
                let mut l: Vec<f32> = v.iter().map(|t| t.lum).collect();
                l.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mut s = [0f32; 3]; for t in &v { for c in 0..3 { s[c] += t.rgb[c]; } }
                let nn = v.len() as f32;
                println!("{name:<40} n {:>7}  lum median {:.4}  p10 {:.4}  p90 {:.4}  mean rgb ({:.3}, {:.3}, {:.3})", v.len(), l[l.len() / 2], l[l.len() / 10], l[l.len() * 9 / 10], s[0] / nn, s[1] / nn, s[2] / nn);
                l[l.len() / 2]
            };
            let top = |t: &Tx, y: f32| t.n[1] > 0.9 && (t.p[1] - y).abs() < 1.6; // the tiles' tops sit 1 m above the item position
            let in_a = |t: &Tx| t.p[0] >= x0 && t.p[0] < x0 + 48.0 && t.p[2] >= z0 && t.p[2] < z0 + 48.0;
            let in_b = |t: &Tx| t.p[0] >= x0 + 48.0 && t.p[0] < x0 + 96.0 && t.p[2] >= z0 && t.p[2] < z0 + 48.0;
            let under_a = |t: &Tx| t.p[0] >= x0 + 16.0 && t.p[0] < x0 + 32.0 && t.p[2] >= z0 + 16.0 && t.p[2] < z0 + 32.0;
            let under_b = |t: &Tx| t.p[0] >= x0 + 64.0 && t.p[0] < x0 + 80.0 && t.p[2] >= z0 + 16.0 && t.p[2] < z0 + 32.0;
            let open = stats("A open tiles (not under the 16 m roof)", &|t: &Tx| top(t, y0) && in_a(t) && !under_a(t));
            stats("A centre tile under the 16 m roof, all", &|t: &Tx| top(t, y0) && in_a(t) && under_a(t));
            stats("A centre under the 16 m roof, inner 8×8", &|t: &Tx| top(t, y0) && under_a(t) && (t.p[0] - x0 - 24.0).abs() < 4.0 && (t.p[2] - z0 - 24.0).abs() < 4.0);
            stats("B open tiles (not under the 8 m roof)", &|t: &Tx| top(t, y0) && in_b(t) && !under_b(t));
            stats("B centre under the 8 m roof, inner 8×8", &|t: &Tx| top(t, y0) && under_b(t) && (t.p[0] - x0 - 72.0).abs() < 4.0 && (t.p[2] - z0 - 24.0).abs() < 4.0);
            stats("16 m roof top", &|t: &Tx| top(t, y0 + 16.0) && under_a(t));
            stats("8 m roof top", &|t: &Tx| top(t, y0 + 8.0) && under_b(t));
            stats("roof undersides (n down)", &|t: &Tx| t.n[1] < -0.9 && t.p[1] > y0 + 6.0 && t.p[1] < y0 + 18.0);
            stats("tile undersides (n down, y ≈ y0)", &|t: &Tx| t.n[1] < -0.9 && (t.p[1] - y0).abs() < 1.6);
            let blk = |t: &Tx| t.p[0] >= x0 + 100.0 && t.p[0] < x0 + 124.0 && t.p[2] >= z0 + 12.0 && t.p[2] < z0 + 36.0 && t.p[1] > y0 - 1.0 && t.p[1] < y0 + 20.0;
            stats("block face −x (west)", &|t: &Tx| blk(t) && t.n[0] < -0.9);
            stats("block face +x (east)", &|t: &Tx| blk(t) && t.n[0] > 0.9);
            stats("block face −z (south)", &|t: &Tx| blk(t) && t.n[2] < -0.9);
            stats("block face +z (north)", &|t: &Tx| blk(t) && t.n[2] > 0.9);
            stats("block top", &|t: &Tx| blk(t) && t.n[1] > 0.9 && t.p[1] > y0 + 2.0);
            for (name, cx, cz, roof_h) in [("16 m roof", x0 + 24.0, z0 + 24.0, 16.0f32), ("8 m roof", x0 + 72.0, z0 + 24.0, 8.0)] {
                let mut bins = vec![(0f64, 0usize); 16];
                let grp: &dyn Fn(&Tx) -> bool = if roof_h > 10.0 { &in_a } else { &in_b };
                for t in tx.iter().filter(|t| top(t, y0) && grp(t)) {
                    let r = ((t.p[0] - cx).powi(2) + (t.p[2] - cz).powi(2)).sqrt();
                    let b = (r / 1.5) as usize;
                    if b < bins.len() { bins[b].0 += t.lum as f64; bins[b].1 += 1; }
                }
                let line: Vec<String> = bins.iter().enumerate().filter(|(_, b)| b.1 > 0).map(|(i, (s, n))| format!("{:.0}m:{:.3}", i as f32 * 1.5 + 0.75, s / *n as f64 / open.max(1e-6) as f64)).collect();
                println!("{name} profile (ratio to open, by distance from the roof centre): {}", line.join(" "));
            }
        }
        "atlascmp" => {
            // lmtool atlascmp REF.Map.Gbx OURS.Map.Gbx [--base N] [--items N]: the gate, camera-free — for every texel
            // of the reference bake (sqrt-decoded, absolute HDR) the other bake's value at the same texel of the same
            // chart (both files are the same map, so charts map 1:1 by item; when chart sizes differ the other atlas
            // is sampled at the same uv). Reports the mean luminance of both, their ratio, the per-texel RMSE
            // (absolute and relative to the reference mean), the lit/occluded split (reference texels above/below
            // its median) and the mean ratio in each half, and the same numbers for horizontal texels only.
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let load = |p: &str| {
                let own = lightmap::mapio::load(p).expect("load");
                let d = own.chunk.data.clone().expect("lightmap");
                let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("atlas");
                let fm = d.cache.frame_max_hdr().unwrap_or(1.0);
                (d, ia, fm)
            };
            let (da, ia, fma) = load(&a[1]);
            let (db, ib, fmb) = load(&a[2]);
            let (mpa, mpb) = (da.cache.mapping().unwrap(), db.cache.mapping().unwrap());
            let chart_of = |mp: &lightmap::format::Mapping| { let mut h: std::collections::HashMap<u32, usize> = Default::default(); for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { h.insert(obj - base, i); } } h };
            let (ca, cb) = (chart_of(mpa), chart_of(mpb));
            let (flip_u, flip_v, swap_uv) = (a.iter().any(|x| x == "--flip-u"), a.iter().any(|x| x == "--flip-v"), a.iter().any(|x| x == "--swap-uv"));
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let step = (scene.instances.len() / f("--items").map(|s| s.parse().unwrap()).unwrap_or(4000)).max(1);
            // rows: (ref lum, ours lum, horizontal?)
            let mut rows: Vec<(f64, f64, bool)> = Vec::new();
            for (ii, inst) in scene.instances.iter().enumerate().step_by(step) {
                let (Some(&i), Some(&j)) = (ca.get(&(inst.item as u32)), cb.get(&(inst.item as u32))) else { continue };
                let (xa, ya) = mpa.pos[i]; let (wa, ha) = mpa.size[i];
                let (xb, yb) = mpb.pos[j]; let (wb, hb) = mpb.size[j];
                let (pxa, pya, pwa, pha) = ((xa as u32 + 1) / 2, (ya as u32 + 1) / 2, (wa as u32 / 2).max(1), (ha as u32 / 2).max(1));
                let (pxb, pyb, pwb, phb) = ((xb as u32 + 1) / 2, (yb as u32 + 1) / 2, (wb as u32 / 2).max(1), (hb as u32 / 2).max(1));
                if pwa < 2 || pha < 2 { continue; }
                let (fba, fbb) = (mpa.frame_bytes[0][i], mpb.frame_bytes[0][j]);
                if fba == 0 || fbb == 0 { continue; }
                let (samples, _) = lightmap::bake::rasterise_pub(&scene, ii, pwa, pha, false, true);
                for s in &samples {
                    let c = ia.get((pxa + s.px).min(ia.w - 1), (pya + s.py).min(ia.h - 1));
                    let la = (0.2126 * lightmap::synth::decode_value(c[0], fba) + 0.7152 * lightmap::synth::decode_value(c[1], fba) + 0.0722 * lightmap::synth::decode_value(c[2], fba)) * fma;
                    // the same uv in the other chart
                    let (mut u, mut v) = ((s.px as f32 + 0.5) / pwa as f32, (s.py as f32 + 0.5) / pha as f32);
                    if flip_u { u = 1.0 - u; }
                    if flip_v { v = 1.0 - v; }
                    if swap_uv { std::mem::swap(&mut u, &mut v); }
                    let (qx, qy) = (((u * pwb as f32) as u32).min(pwb - 1), ((v * phb as f32) as u32).min(phb - 1));
                    let c2 = ib.get((pxb + qx).min(ib.w - 1), (pyb + qy).min(ib.h - 1));
                    let lb = (0.2126 * lightmap::synth::decode_value(c2[0], fbb) + 0.7152 * lightmap::synth::decode_value(c2[1], fbb) + 0.0722 * lightmap::synth::decode_value(c2[2], fbb)) * fmb;
                    rows.push((la as f64, lb as f64, s.n[1] > 0.9));
                }
            }
            let report = |name: &str, rows: &[(f64, f64, bool)]| {
                if rows.is_empty() { println!("{name}: no texels"); return; }
                let n = rows.len() as f64;
                let (ma, mb) = (rows.iter().map(|r| r.0).sum::<f64>() / n, rows.iter().map(|r| r.1).sum::<f64>() / n);
                let rmse = (rows.iter().map(|r| (r.0 - r.1) * (r.0 - r.1)).sum::<f64>() / n).sqrt();
                let mut sorted: Vec<f64> = rows.iter().map(|r| r.0).collect();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let med = sorted[sorted.len() / 2];
                let (lo, hi): (Vec<&(f64, f64, bool)>, Vec<&(f64, f64, bool)>) = rows.iter().partition(|r| r.0 < med);
                let mean = |v: &Vec<&(f64, f64, bool)>, k: usize| v.iter().map(|r| if k == 0 { r.0 } else { r.1 }).sum::<f64>() / v.len().max(1) as f64;
                let (lo_a, lo_b, hi_a, hi_b) = (mean(&lo, 0), mean(&lo, 1), mean(&hi, 0), mean(&hi, 1));
                println!("{name}: {} texels — mean lum ref {ma:.4} ours {mb:.4} ratio {:.4} ({:+.1} %); RMSE {rmse:.4} ({:.1} % of the ref mean); occluded half (below the ref median {med:.3}) ref {lo_a:.4} ours {lo_b:.4} ratio {:.3}; lit half ref {hi_a:.4} ours {hi_b:.4} ratio {:.3}; contrast (lit/occluded) ref {:.3} ours {:.3}",
                    rows.len(), mb / ma.max(1e-9), 100.0 * (mb / ma.max(1e-9) - 1.0), 100.0 * rmse / ma.max(1e-9), lo_b / lo_a.max(1e-9), hi_b / hi_a.max(1e-9), hi_a / lo_a.max(1e-9), hi_b / lo_b.max(1e-9));
            };
            println!("{} (frame MaxHDR {fma:.4}) vs {} (frame MaxHDR {fmb:.4})", a[1].rsplit('/').next().unwrap(), a[2].rsplit('/').next().unwrap());
            // the joint relation: ref value bins → mean and spread of ours
            {
                let mut bins: Vec<(f64, f64, usize)> = vec![(0.0, 0.0, 0); 12];
                for (ra, rb, _) in &rows { let b = ((ra / 0.1) as usize).min(11); bins[b].0 += rb; bins[b].1 += rb * rb; bins[b].2 += 1; }
                println!("ref bin → ours mean ± sd (n): {}", bins.iter().enumerate().filter(|(_, b)| b.2 > 0).map(|(i, (s, s2, n))| { let m = s / *n as f64; let sd = (s2 / *n as f64 - m * m).max(0.0).sqrt(); format!("[{:.1}–{:.1}) {m:.3}±{sd:.3} ({n})", i as f64 * 0.1, (i + 1) as f64 * 0.1) }).collect::<Vec<_>>().join("  "));
            }
            report("all texels", &rows);
            let horiz: Vec<(f64, f64, bool)> = rows.iter().copied().filter(|r| r.2).collect();
            report("horizontal texels", &horiz);
            let vert: Vec<(f64, f64, bool)> = rows.iter().copied().filter(|r| !r.2).collect();
            report("non-horizontal texels", &vert);
        }
        _ => {
            eprintln!("unknown command");
            std::process::exit(2);
        }
    }
}

/// Item positions in file order, through `tmmaps census` (the item list
/// reader lives in tmmaps; this keeps lightmap free of a second parser).
fn tmmaps_items(map: &str) -> Vec<[f32; 3]> {
    let exe = std::env::current_exe().unwrap().with_file_name("tmmaps");
    let out = std::process::Command::new(&exe).arg("census").arg(map).output().expect("tmmaps census");
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().skip(1).filter(|l| l.starts_with("I\t")).map(|l| {
        let c: Vec<&str> = l.split('\t').collect();
        [c[8].parse().unwrap(), c[9].parse().unwrap(), c[10].parse().unwrap()]
    }).collect()
}

static IMPLIED_K: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// K such that our charts' `255·max/K` matches the reference bake's fb0 in the median.
fn implied_k(charts: &[lightmap::bake::ChartBake], own: &Option<std::collections::HashMap<u32, (u8, [f32; 3])>>) -> Option<f32> {
    let own = own.as_ref()?;
    let mut ks: Vec<f32> = charts.iter().filter_map(|c| {
        let &(fb0, _) = own.get(&(c.item as u32))?;
        let mx = c.max_channel();
        if mx <= 1e-4 || fb0 == 0 { return None; }
        Some(255.0 * mx / fb0 as f32)
    }).collect();
    if ks.len() < 10 { return None; }
    ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(ks[ks.len() / 2])
}
