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
    let mut a: Vec<String> = std::env::args().skip(1).collect();
    // `bake … --lm-from-map` IMPLIES the transcribed chain: the game's peel and accumulation (--game-peel), one raster
    // sub-sample (--ss 1), the sky at the game's scale (--sky-global-scale 1), the H-basis without the port's kappa
    // (--hbasis-kappa 1), the first sweep's sun through the ILightInput chain (--sweep0-sun) and the game's layout
    // (--layout-game) — a run without them wrote an all-zero lightmap. --legacy-port keeps the flags as given.
    if a.first().map(|s| s.as_str()) == Some("bake") && a.iter().any(|x| x == "--lm-from-map") && !a.iter().any(|x| x == "--legacy-port") {
        let implied: &[(&str, Option<&str>)] = &[("--game-peel", None), ("--ss", Some("1")), ("--sky-global-scale", Some("1")), ("--hbasis-kappa", Some("1")), ("--sweep0-sun", None), ("--layout-game", None)];
        let mut added: Vec<String> = Vec::new();
        // the peel COLOURS: the setup chain from the map (A's setupmap.rs, `--ilightinput-from map`) needs the frozen
        // collection tables (`--env-from ROOT`); without an atlas the peels colour from the port's albedo × sun model,
        // which came out ~2× too bright on pwc-day (2026-09-25 validation) — so imply `map` when --env-from is given
        // and say so when it is not
        if !a.iter().any(|x| x == "--ilightinput-from") {
            if a.iter().any(|x| x == "--env-from") {
                a.push("--ilightinput-from".to_string()); a.push("map".to_string());
                added.push("--ilightinput-from map".to_string());
            } else {
                eprintln!("lm-from-map: no --env-from ROOT (the frozen collection tables) — the peels colour from the port's own model, NOT the transcribed ILightInput chain (measured ~2x too bright on pwc-day); pass --env-from PASSCAP_ROOT for the game's chain");
            }
        }
        for (flag, val) in implied {
            if !a.iter().any(|x| x == flag) {
                a.push(flag.to_string());
                if let Some(v) = val { a.push(v.to_string()); }
                added.push(match val { Some(v) => format!("{flag} {v}"), None => flag.to_string() });
            }
        }
        if !added.is_empty() { eprintln!("lm-from-map implies: {}", added.join(", ")); }
    }
    if a.is_empty() {
        eprintln!("usage: lmtool walk MAP.Gbx... | lmtool dump MAP.Gbx OUTDIR | lmtool probe MAP.Gbx [OUTDIR]");
        std::process::exit(2);
    }
    run(a);
}

/// A POSIX shell single-quoting of one argument (for the `ssh host lmtool …` range bakes).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn run(a: Vec<String>) {
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
                println!("{i}\t{}\t{}\t{:#x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", b.obj_group_idx / 4, b.obj_idx & 0xffffff, b.obj_idx >> 24, m.pos[i].0, m.pos[i].1, m.size[i].0, m.size[i].1, m.chart_f32[i], m.frame_bytes[0][i], m.frame_bytes.get(1).map(|v| v[i]).unwrap_or(0), m.frame_bytes.get(2).map(|v| v[i]).unwrap_or(0));
            }
        }
        "daytime" if a.iter().any(|x| x == "--set") => {
            // lmtool daytime MAP --out F --set N|default: the baker's setter (chunk word + lightmap frame records)
            let mut b = a.clone();
            b[0] = "daytime-set".into();
            return run(b);
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
                let fb = |k: usize| m.frame_bytes.get(k).and_then(|v| v.get(i)).copied().unwrap_or(0);
                println!("{i}\t{}\t{x}\t{y}\t{w}\t{h}\t{}\t{}\t{}\t{:.0} {:.0} {:.0}\t{:.0} {:.0} {:.0}\t{:.0} {:.0} {:.0}", m.binds[i].obj_group_idx / 4, fb(0), fb(1), fb(2), ma[0], ma[1], ma[2], mb[0], mb[1], mb[2], m1[0], m1[1], m1[2]);
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
            let mut scene = lightmap::geometry::Scene::from_map(&map_path).expect("scene");
            // the decoration's surroundings: --decoration FILE.obj[,FILE…] [--decoration-scale S --decoration-offset x,y,z],
            // or by default lightmap-re/scene3d/<Collection>.obj when it exists (RE child 3's Scene3d export),
            // --no-decoration to leave it out
            {
                let coll = hdr.as_ref().map(|h| h.envir.clone()).unwrap_or_default();
                let default_obj = format!("{}/persistent/private-30d/tm-player/tiny/lightmap-re/scene3d/{coll}.obj", std::env::var("HOME").unwrap_or_default());
                let paths: Vec<String> = match f("--decoration") {
                    Some(p) => p.split(',').map(|s| s.to_string()).collect(),
                    None if !has("--no-decoration") && std::path::Path::new(&default_obj).exists() => vec![default_obj.clone()],
                    None => Vec::new(),
                };
                let dscale: f32 = f("--decoration-scale").map(|s| s.parse().unwrap()).unwrap_or(1.0);
                let doff: [f32; 3] = f("--decoration-offset").map(|s| { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] }).unwrap_or([0.0; 3]);
                for p in paths {
                    match lightmap::geometry::load_obj_decor(&p, dscale, doff) {
                        Ok(t) => { eprintln!("decoration: {} triangles from {p}", t.len()); scene.decor.extend(t); }
                        Err(e) => eprintln!("decoration: {e} (ignored)"),
                    }
                }
            }
            // --env-from PASSCAP_DIR: the game's environment block read off the capture (the sea box and the
            // four terrain patches as black occluders; the sky dome is `dome_radiance`) in place of the
            // Scene3d decoration
            if let Some(dir) = f("--env-from") {
                let te = std::time::Instant::now();
                match lightmap::envcap::load_env(std::path::Path::new(&dir)) {
                    Ok(meshes) => {
                        let n_before = scene.decor.len();
                        scene.decor.clear();
                        scene.decor.extend(lightmap::envcap::env_decor(&meshes));
                        eprintln!("env-from {dir}: {} environment triangles replace the {n_before} decoration triangles ({:.2}s)", scene.decor.len(), te.elapsed().as_secs_f32());
                    }
                    Err(e) => eprintln!("env-from {dir}: {e} (the decoration stays)"),
                }
            }
            // THE ZONE TILES the game regenerates from the genealogy (chunk 0x03043043: one CurrentZoneId per
            // cell; the tiny maps' BlueBay genealogy is Sea ×4096) — the Scene3d meshes leave the whole map
            // area open (the Water and WarpSand have a hole over 0..2048 × 0..2048), the tiles fill it. Each
            // Sea/Water/Lake cell = a water quad at the collection's sea level (BlueBay 7.0, RedIsland −0.3,
            // WhiteShore −1.0, GreenCoast −0.8 — the Scene3d water heights) over a sand floor 3 m under it;
            // a Land-type cell = a land plane at sea level + 3 (BlueBay's Land tile top is at +2 over the
            // cell above the sea's). With an EMPTY genealogy the map's own Sea BLOCKS are the water cells.
            // --no-zone-tiles drops them.
            if !has("--no-zone-tiles") {
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let envir = hdr.as_ref().map(|h| h.envir.to_ascii_lowercase()).unwrap_or_default();
                let sea_y: f32 = f("--sea-y").map(|s| s.parse().unwrap()).unwrap_or(match envir.as_str() { "bluebay" => 7.0, "redisland" => -0.3, "whiteshore" => -1.0, "greencoast" => -0.8, _ => f32::NAN });
                if sea_y.is_finite() {
                    let zones = mf.genealogy_zones();
                    let mut water_cells: Vec<(u32, u32)> = Vec::new();
                    let mut land_cells: Vec<(u32, u32)> = Vec::new();
                    if zones.len() == 4096 {
                        for (i, z) in zones.iter().enumerate() {
                            let (cx, cz) = ((i % 64) as u32, (i / 64) as u32);
                            let zl = z.to_ascii_lowercase();
                            if zl.contains("sea") || zl.contains("water") || zl.contains("lake") { water_cells.push((cx, cz)); } else if !zl.is_empty() { land_cells.push((cx, cz)); }
                        }
                    } else {
                        for b in mf.blocks.iter().chain(mf.baked.iter()) {
                            let nl = b.name.to_ascii_lowercase();
                            if nl == "sea" || nl == "water" || nl == "lake" { let c = b.coords(); water_cells.push((c.0 as u32, c.2 as u32)); }
                        }
                        // THE CAPTURE (2026-09-24, pwc-day: 2412 Sea blocks): the game's peel renders the zone tile as
                        // 4096 instances — every cell of the 64×64 grid carries a tile whether or not a Sea block is
                        // authored there (the empty genealogy's default zone); --zone-fill blocks keeps the authored cells only
                        if f("--zone-fill").as_deref() != Some("blocks") && mf.size[0] == 64 && mf.size[2] == 64 {
                            let have: std::collections::HashSet<(u32, u32)> = water_cells.iter().copied().collect();
                            for cz in 0..64u32 { for cx in 0..64u32 { if !have.contains(&(cx, cz)) { water_cells.push((cx, cz)); } } }
                        }
                    }
                    let water_alb = lightmap::albedo::for_link("Water").unwrap_or([0.3; 3]);
                    let sand_alb = lightmap::albedo::for_link("Sand").unwrap_or([0.5; 3]);
                    let land_alb = lightmap::albedo::for_link("Land").unwrap_or([0.2; 3]);
                    let mut quad = |cx: u32, cz: u32, y: f32, alb: [f32; 3], water: bool| {
                        let (x0, z0) = (cx as f32 * 32.0, cz as f32 * 32.0);
                        let q = [[x0, y, z0], [x0 + 32.0, y, z0], [x0 + 32.0, y, z0 + 32.0], [x0, y, z0 + 32.0]];
                        scene.decor.push(lightmap::geometry::DecorTri { p: [q[0], q[2], q[1]], albedo: alb, water, env: false, env_far_only: false });
                        scene.decor.push(lightmap::geometry::DecorTri { p: [q[0], q[3], q[2]], albedo: alb, water, env: false, env_far_only: false });
                    };
                    // THE CAPTURE (2026-09-24, passcap-info on the world peel's layer 0): the game's zone tile is ONE flat
                    // surface at y = 3.76 + bias ≈ 4.0 = the SEABED, 3 m under the collection's sea level — the water
                    // plane is not in the peel at all (the second world layer holds the items alone: 651 px against
                    // 10.5 M with a floor under a water quad). The tile is the sand floor; --tile-water restores the
                    // water quad at the sea level (with the old sand floor under it: --sand-floor)
                    let tile_water = has("--tile-water");
                    let sand = has("--sand-floor");
                    for &(cx, cz) in &water_cells {
                        if tile_water { quad(cx, cz, sea_y, water_alb, true); if sand { quad(cx, cz, sea_y - 3.0, sand_alb, false); } } else { quad(cx, cz, sea_y - 3.0, sand_alb, false); }
                    }
                    for &(cx, cz) in &land_cells { quad(cx, cz, sea_y + 3.0, land_alb, false); }
                    eprintln!("zone tiles: {} water cells (sea level {sea_y}), {} land cells ({})", water_cells.len(), land_cells.len(), if zones.len() == 4096 { "from the genealogy" } else { "from the map's Sea blocks — the genealogy is empty" });
                }
            }
            // the raster peel has no analytic ground: without a decoration mesh, a ground/sea quad at
            // --ground-y (8 m: the sea of the terrain collections, the Stadium floor) with the ground's bounce
            // albedo stands in (a downward direction must hit SOMETHING dark, not the sky gradient's bottom rows)
            if has("--raster") && scene.decor.is_empty() && !has("--no-ground") {
                let gy: f32 = f("--ground-y").map(|s| s.parse().unwrap()).unwrap_or(8.0);
                let ga: f32 = f("--ground-bounce").map(|s| s.parse().unwrap()).unwrap_or(0.37);
                let (lo, hi) = (-4096.0f32, 8192.0f32);
                let q = [[lo, gy, lo], [hi, gy, lo], [hi, gy, hi], [lo, gy, hi]];
                scene.decor.push(lightmap::geometry::DecorTri { p: [q[0], q[1], q[2]], albedo: [ga; 3], water: false, env: false, env_far_only: false });
                scene.decor.push(lightmap::geometry::DecorTri { p: [q[0], q[2], q[3]], albedo: [ga; 3], water: false, env: false, env_far_only: false });
                eprintln!("decoration: none — a ground quad at y = {gy} (albedo {ga}) stands in");
            }
            eprintln!("scene: {} models, {} instances, {} triangles (+ {} decoration) ({:.1}s)", scene.models.len(), scene.instances.len(), scene.tri_count(), scene.decor.len(), t0.elapsed().as_secs_f32());
            let tb = std::time::Instant::now();
            let (tris, alpha_masks) = lightmap::bake::world_tris_masks(&scene);
            let t_masks = tb.elapsed().as_secs_f32();
            let bvh = lightmap::bvh::Bvh::build(tris);
            eprintln!("bvh: {} nodes ({:.1}s; the masks {t_masks:.2}s, the build {:.2}s)", bvh.node_count(), t0.elapsed().as_secs_f32(), tb.elapsed().as_secs_f32() - t_masks);
            let parse_rgb = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] };
            let mut prm = lightmap::bake::BakeParams::default();
            prm.alpha_masks = std::sync::Arc::new(alpha_masks);
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
                // the mood BLEND: the map's DayTime word is a blend key between the moods' DayTime01 keys and the
                // game lerps every mood field between the two bracketing moods (--no-mood-blend: the quarter's mood
                // alone, the pre-2026-09-23 form; --mood M forces a pure mood)
                let key: f32 = match dt { Some(t) if t != 0xffff_ffff => t as f32 / 65536.0, _ => lightmap::moods::default_daytime(&h.envir, mood) as f32 / 65536.0 };
                // OPT-IN (--mood-blend) until the blender's semantics are pinned: at key 0.75 (pure "Sunset" by the
                // DayTime01 keys) the editor's open pad is as blue and bright as at Day (0.85 of it), which the
                // Sunset mood's own sky does not give — the per-mood fits are made at the moods' DEFAULT words
                let blend = if has("--mood-blend") && f("--mood").map(|m| m == "auto").unwrap_or(true) { lightmap::moods::blend(&h.envir, key) } else { None };
                let x_pure = lightmap::moods::mood_xml(&h.envir, mood).unwrap_or_else(|| panic!("no mood XML for {} {mood}", h.envir));
                let x_blended: lightmap::moods::MoodXml = match blend { Some(_) => lightmap::moods::blended_xml(&h.envir, key).unwrap(), None => *x_pure };
                let x: &lightmap::moods::MoodXml = &x_blended;
                if let Some((a, b, t)) = blend { eprintln!("mood blend: key {key:.4} = {} {:.0} % + {} {:.0} % (record mood {mood})", a.mood, (1.0 - t) * 100.0, b.mood, t * 100.0); }
                xml_sel = Some(x_pure);
                // the dome model: sky = 1.55·LAmbient·SkyFactor (an open floor on the BlueBay Sunset-quarter test
                // bakes reads (0.61, 0.58, 0.77) = 1.55 × LAmbient in LAmbient's hue), no separate ambient term
                let dome = !has("--hemi");
                let sky_s: f32 = f("--sky-scale").map(|s| s.parse().unwrap()).unwrap_or(if dome { 1.55 } else { 1.0 });
                prm.dome_deg = if dome { f("--cone-deg").map(|s| s.parse().unwrap()).unwrap_or(30.0) } else { 90.0 };
                prm.ambient_la = if dome { [0.0; 3] } else { x.l_ambient };
                prm.l_ambient = x.l_ambient;
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
                        let (mood_a, mood_b, bt) = match blend { Some((a, b, t)) => (a.mood, b.mood, t), None => (x.mood, x.mood, 0.0) };
                        let mood = mood_a;
                        let path = f("--sky-grad").unwrap_or_else(|| lightmap::skygrad::mood_file(coll, mood_a, "SkyColor.dds"));
                        match lightmap::skygrad::SkyGradient::load(&path) {
                            Ok(mut g) => {
                                if bt > 0.0 && f("--sky-grad").is_none() {
                                    match lightmap::skygrad::SkyGradient::load(&lightmap::skygrad::mood_file(coll, mood_b, "SkyColor.dds")) {
                                        Ok(g2) => g.blend_with(&g2, bt),
                                        Err(e) => eprintln!("sky gradient of {mood_b}: {e}; no texture blend"),
                                    }
                                }
                                // the gradient's global scale: 1.6 fits the BlueBay Sunset open floor (0.607) — per-mood
                                // values pending (GlobalScale·ScaleGrad0 from the runtime sky constants)
                                // the fitted per-mood scale, lerped between the two moods like every other field
                                let fitted = lightmap::moods::sky_grad_scale(coll, mood_a) * (1.0 - bt) + lightmap::moods::sky_grad_scale(coll, mood_b) * bt;
                                // ScaleGrad0 = 1 (RE child 4); the mood's SkyFactor scales the WHOLE dome result (it rides
                                // in the sky pass constant 4·D.y·SkyFactor/N), so it goes into global_scale below, not here
                                g.scale = f("--sky-grad-scale").map(|s| s.parse().unwrap()).unwrap_or(fitted);
                                g.sun_dir = prm.sun_dir;
                                g.sun_az = prm.sun_dir[0].atan2(prm.sun_dir[2]);
                                g.v_full = has("--v-full");
                                g.v_sin = !has("--v-linear");
                                if has("--v-flip") || (fit.2 && !has("--no-v-flip")) { g.v_top_is_zenith = false; }
                                if let Some(o) = f("--u-sun") { g.u_sun = o.parse().unwrap(); }
                                if has("--u-flip") { g.u_sign = -1.0; }
                                let lobe_scale: f32 = f("--lobe-scale").map(|s| s.parse().unwrap()).unwrap_or(1.0);
                                // 5300 m: the BlueBay Day open pad's colour (fog intensity 0.32; the pad-only test map vs the editor, 2026-09-23)
                                let dome_m: f32 = f("--fog-dome-m").map(|s| s.parse().unwrap()).unwrap_or(5300.0);
                                let xml_a = std::fs::read_to_string(lightmap::skygrad::mood_file(coll, mood_a, "Mood.MoodSetting.xml")).ok();
                                let xml_b = if bt > 0.0 { std::fs::read_to_string(lightmap::skygrad::mood_file(coll, mood_b, "Mood.MoodSetting.xml")).ok() } else { None };
                                if let Some(xa) = &xml_a {
                                    let la = lightmap::skygrad::lobes_from_xml(xa);
                                    let lobes = match &xml_b { Some(xb) => lightmap::skygrad::lerp_lobes(&la, &lightmap::skygrad::lobes_from_xml(xb), bt), None => la };
                                    g.lobes = lobes.into_iter().map(|(p, c, s)| (p, c, s * lobe_scale)).collect();
                                    // Tech3/Sky_p: the dome is at the fog's far depth → lerp(sky, Fog.Color, Fog.Intens(dome)); --no-fog / --fog-intens F
                                    if !has("--no-fog") {
                                        let fa = lightmap::skygrad::fog_from_xml(xa, dome_m);
                                        let fog = match &xml_b { Some(xb) => lightmap::skygrad::lerp_fog(fa, lightmap::skygrad::fog_from_xml(xb, dome_m), bt), None => fa };
                                        let fitted_fi = match blend { Some((a, b, t)) => match (lightmap::moods::fog_intens(coll, a.mood), lightmap::moods::fog_intens(coll, b.mood)) { (Some(p), Some(q)) => Some(p + (q - p) * t), (p, q) => p.or(q) }, None => lightmap::moods::fog_intens(coll, mood_a) };
                                        g.fog = fog.map(|(c, i)| (c, f("--fog-intens").map(|s| s.parse().unwrap()).or(fitted_fi).unwrap_or(i)));
                                    }
                                }
                                // with the fog blend the fitted per-mood number is GlobalScale (the gradient's own ScaleGrad0 = 1 for the HDR BC6H texture)
                                if f("--sky-grad-scale").is_none() { g.global_scale = g.scale * x.sky_factor; g.scale = 1.0; }
                                // --sky-global-scale G: Sky_p's GlobalScale (after the fog blend) — the per-mood fitted number
                                if let Some(v) = f("--sky-global-scale") { g.global_scale = v.parse().unwrap(); }
                                // --dome-u-mode wrap|mirror|clamp: the gradient sampler's u addressing in the dome transcription
                                g.dome_u_mode = match f("--dome-u-mode").as_deref() { Some("wrap") => 0, Some("clamp") => 2, Some("mirror") | None => 1, Some(o) => panic!("--dome-u-mode wrap|mirror|clamp, not {o}") };
                                eprintln!("sky: {} ({}×{}), grad scale {}, global scale {}, fog {:?}, lobes {:?}", path.rsplit('/').next().unwrap(), g.w, g.h, g.scale, g.global_scale, g.fog, g.lobes.iter().map(|l| (l.0, l.2)).collect::<Vec<_>>());
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
                    // the directions come IN THE GAME'S ISSUE ORDER (RE child 5: SPlugGroupOfPointInSphere, ss²
                    // interleaved groups — `dome::sweep_directions`; direction k carries raster sub-sample k mod 9),
                    // so the accumulation after direction k is the game's after k; --table-order keeps the table's
                    if let Ok(ps) = lightmap::dome::PointSets::load(&pp) {
                        if has("--table-order") { if let Some(set) = ps.nearest(n0) { prm.sphere_dirs = std::sync::Arc::new(lightmap::dome::rotate_set(set)); } }
                        else if let Some(d) = lightmap::dome::sweep_directions(&ps, q, 0, false) { prm.sphere_dirs = std::sync::Arc::new(d); }
                        eprintln!("peel: quality {q}, sweeps {:?}, first set {} directions (rotated{})", counts, prm.sphere_dirs.len(), if has("--table-order") { ", table order" } else { ", the game's issue order" });
                    }
                    // the lightmap-so-far is read back divided by BounceFactor (RE child 2 (d)); the Day-quarter
                    // test bake confirms a weak bounce (a pad under an 8 m plate: 51 % of open, walls 43 % of floors)
                    // the read-back divisor: 1 (DIFFERENTIAL, 2026-09-23 21:35Z — the pad-only test map's post
                    // faces at Day 0.93 → 0.98 and at Sunset 0.67 → 0.76 of the editor with the floors unchanged;
                    // RE 3 read the ÷BounceFactor on the decode scales but not whether the accumulate constant
                    // compensates it — the measurement says it does). --bounce-decode overrides.
                    prm.bounce_decode = f("--bounce-decode").map(|v| v.parse().unwrap()).unwrap_or(1.0);
                    if let Some(v) = f("--horizon-el") { prm.horizon_el = v.parse().unwrap(); }
                    if let Some(v) = f("--horizon-rgb") { prm.horizon_radiance = parse_rgb(&v); }
                }
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
                prm.sun = if has("--no-sun-bounce") { [0.0; 3] } else { x.l_dir_sun };
                if let Some(v) = f("--decor-ambient") { prm.decor_ambient = v.parse().unwrap(); }
                if let Some(v) = f("--water-reflect") { prm.water_reflect = v.parse().unwrap(); }
                if let Some(v) = f("--water-sun") { prm.water_sun = v.parse().unwrap(); }
                if let Some(v) = f("--water-sun-pow") { prm.water_sun_pow = v.parse().unwrap(); }
                prm.direct_sun = 0.0;
                prm.ambient = [0.0; 3]; prm.up = [0.0; 3];
                prm.bounce = f("--bounce").map(|s| s.parse().unwrap()).unwrap_or(x.bounce_factor);
                prm.albedo = f("--albedo").map(|s| s.parse().unwrap()).unwrap_or(if prm.peel { lightmap::moods::sky_fit(x.collection, x.mood).1 } else { 0.18 });
                prm.flat_albedo = f("--albedo").is_some() || has("--flat-albedo");
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
                // the sun DIRECTION — the game's own functions [DISASSEMBLY, RE child 3 2026-09-23]: the map's
                // DayTime word/65536 is a BLEND KEY u; the decoration's CPlugMoodBlender curve turns it into a
                // time t (SunRise t_r 06:00, SunFall t_s 18:00, w = 30 min — the defaults; BlueBay's blender entry
                // is not decoded yet), t into the arc parameter b (0 at sunrise, 1 at sunset; night → 0 or 1), and
                // the light direction is D = (cos πb, −cos(lat)·sin πb, −sin(lat)·sin πb) with the mood XML's
                // Latitude: the sun rises towards −X, culminates at 90° − lat leaning +Z, sets towards +X.
                // Checked against the BlueBay DayTime series: u 0.312 → the glow due −X at 1.8°, u 0.854 → +X
                // at 4.1°, u 0.607 → 67° high. --sunrise/--sunfall/--sun-w override the curve, --sun-lat the latitude.
                let t_r: f32 = f("--sunrise").map(|s| s.parse().unwrap()).unwrap_or(0.25);
                // the decoration blenders (RE child 4, the five CPlugMoodBlender XMLs): SunRise 06:00, SunFall 21:00
                let t_s: f32 = f("--sunfall").map(|s| s.parse().unwrap()).unwrap_or(0.875);
                let w_b: f32 = f("--sun-w").map(|s| s.parse().unwrap()).unwrap_or(1.0 / 48.0);
                let lat_d: f32 = f("--sun-lat").map(|s| s.parse().unwrap()).unwrap_or(x.latitude);
                let (mut az_d, mut el_d) = {
                    let u = t; // the blend key
                    // THE CAPTURE DECIDES (pwc-day, DayTime 0x9b59 = 0.606827, Latitude 20): the bake's cbuffer DirInWorld =
                    // (−0.22097, −0.91646, 0.33357) = D at b = (u − t_r)/(t_s − t_r) = 0.5709 with t_r 0.25 / t_s 0.875 — the
                    // DayTime word IS the arc's time (cos πb = −0.2211, −cos(lat)·sin πb = −0.9165, sin(lat)·sin πb = 0.3336);
                    // the piecewise blender curve the port carried (fitted to two dome-glow observations) put the sun at
                    // b = 0.433, mirrored in x (engineer 2's finding: every shadow on the wrong side of the wall).
                    // --sun-time-curve blender restores the old mapping.
                    let time = if f("--sun-time-curve").as_deref() == Some("blender") {
                        if u <= 0.25 { (t_s + 4.0 * u * (t_r + 1.0 - t_s)).rem_euclid(1.0) }
                        else if u <= 0.5 { t_r + 4.0 * (u - 0.25) * w_b }
                        else if u <= 0.75 { t_r + w_b + 4.0 * (u - 0.5) * (t_s - t_r - 2.0 * w_b) }
                        else { t_s - w_b + 4.0 * (u - 0.75) * w_b }
                    } else { u };
                    let b = if time >= t_r && time <= t_s { ((time - t_r) / (t_s - t_r)).clamp(0.0, 1.0) } else if time > (t_r + t_s) * 0.5 { 1.0 } else { 0.0 };
                    let (pb, lat) = (std::f32::consts::PI * b, lat_d.to_radians());
                    // the light direction (sun → ground); the sun's position is −D. The Z sign: the rotation is
                    // about X by −lat (newZ = sin(−lat)·Y + cos(−lat)·Z with Y = −sin πb), so D_z = +sin(lat)·sin(πb)
                    // and the sun culminates leaning towards −Z — the editor's wall on the pad-only test map is
                    // brightest on its −z face (the sky glow side), 2026-09-23.
                    let dl = [pb.cos(), -lat.cos() * pb.sin(), lat.sin() * pb.sin()];
                    let sp = [-dl[0], -dl[1], -dl[2]];
                    eprintln!("sun: blend key {u:.4} → time {time:.4} → arc b {b:.4}, latitude {lat_d}°");
                    (sp[0].atan2(sp[2]).to_degrees().rem_euclid(360.0), sp[1].clamp(-1.0, 1.0).asin().to_degrees())
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
                let sub = lightmap::geometry::Scene { models: scene.models.clone(), model_names: scene.model_names.clone(), instances: scene.instances.iter().step_by(step).cloned().collect(), item_count: scene.item_count, decor: scene.decor.clone(), alpha_masks: scene.alpha_masks.clone(), card_albedo: scene.card_albedo.clone(), tex_albedo: scene.tex_albedo.clone() };
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
            // --raster: the dome peel as a software raster (crate::peel) — the game's own pipeline; --ss N
            // sub-samples per axis (the quality table: 1/2/3/3/3/3), --peel-res PX, --peel-bias M
            prm.raster_peel = has("--raster");
            if let Some(v) = f("--ss") { prm.ss = v.parse().unwrap(); } else { prm.ss = match f("--quality").map(|s| s.parse::<u32>().unwrap()).unwrap_or(3) { 0 => 1, 1 => 2, _ => 3 }; }
            if let Some(v) = f("--peel-res") { prm.peel_res = v.parse().unwrap(); }
            if let Some(v) = f("--peel-bias") { prm.peel_bias = v.parse().unwrap(); }
            // THE DIFFERENTIAL HARNESS (port engineer 2, 2026-09-24): --dump-passes DIR writes every intermediate
            // at the game's points (passdump.rs); --game-peel gathers with the game's layer semantics (default with
            // a dump), --per-subsample at every ss² sub-sample (default with a dump; --centroid = the port's
            // texel-centroid gather), --dump-dirs N|i,j,k|all selects the directions whose per-direction buffers
            // are written (default the first 8 of each sweep), --frustum-from GAME/MANIFEST.json rasterises the
            // peel and the sun shadow map in the captured frustums, --quant-peel/--quant-ilightdir/--quant-accum
            // none|r11g11b10|f16 (the targets' formats; defaults r11g11b10 / r11g11b10 / f16 = RE child 3's
            // pins), --rounding rtne|rtz, --depth-bias C,S (D3D, default 1,1.0), --no-inset, --no-dome-layer
            let dump_dir = f("--dump-passes");
            prm.game_peel = if has("--no-game-peel") { false } else { has("--game-peel") || dump_dir.is_some() };
            prm.per_subsample = if has("--centroid") { false } else { has("--per-subsample") || dump_dir.is_some() };
            prm.quant_peel = lightmap::gpufmt::Quant::parse(&f("--quant-peel").unwrap_or_else(|| "r11g11b10".into())).expect("--quant-peel none|r11g11b10|f16");
            prm.quant_ilightdir = lightmap::gpufmt::Quant::parse(&f("--quant-ilightdir").unwrap_or_else(|| "r11g11b10".into())).expect("--quant-ilightdir none|r11g11b10|f16");
            prm.quant_accum = lightmap::gpufmt::Quant::parse(&f("--quant-accum").unwrap_or_else(|| "f16".into())).expect("--quant-accum none|r11g11b10|f16");
            if !prm.game_peel && f("--quant-peel").is_none() && f("--quant-ilightdir").is_none() && f("--quant-accum").is_none() {
                // the product path stays f32 until the capture says otherwise
                prm.quant_peel = lightmap::gpufmt::Quant::None; prm.quant_ilightdir = lightmap::gpufmt::Quant::None; prm.quant_accum = lightmap::gpufmt::Quant::None;
            }
            prm.rounding = match f("--rounding").as_deref() { Some("rtne") | Some("nearest") => lightmap::gpufmt::Rounding::NearestEven, _ => lightmap::gpufmt::Rounding::Truncate }; // the capture: the R11G11B10 target conversion truncates (the dome colours land exactly with it, one quantum high with RTNE)
            if let Some(v) = f("--depth-bias") { let p: Vec<&str> = v.split(',').collect(); prm.depth_bias = (p[0].trim().parse().unwrap(), p.get(1).map(|s| s.trim().parse().unwrap()).unwrap_or(1.0)); }
            prm.dome_layer = !has("--no-dome-layer");
            // the capture (2026-09-24): DepthClip ON, D16 depth target, viewport inset by 1 px (the Bias rows'
            // (w−2)/w inset registers to it, so the lookup takes no extra shift)
            prm.depth_clip = !has("--no-depth-clip");
            prm.depth_bits = f("--depth-bits").map(|v| v.parse().unwrap()).unwrap_or(16);
            prm.peel_inset = has("--inset");
            // --accum hbasis|rnm: the game's H-basis constant-term projection (the capture's PS 17536) or the
            // RNM-style clamped cosine; default hbasis under a dump / game-peel, rnm on the product path until the
            // level is settled (--hbasis-kappa K: 1/√(2π) default, 1 = the raw C0)
            prm.accum_hbasis = match f("--accum").as_deref() { Some("hbasis") => true, Some("rnm") => false, Some(o) => panic!("--accum hbasis|rnm, not {o}"), None => prm.game_peel };
            if let Some(k) = f("--hbasis-kappa") { prm.hbasis_kappa = k.parse().expect("--hbasis-kappa"); }
            prm.sweep0_sun = has("--sweep0-sun");
            prm.dome_exact = !has("--dome-per-direction");
            // THE DOME MESH (domemesh.rs): with --env-from PASSCAP the game's own dome triangles (mesh e001051) are
            // rasterised per peel and PS 16774 runs on the interpolated (u, v); --dome-analytic keeps the ellipsoid model
            if !has("--dome-analytic") {
                if let Some(dir) = f("--env-from") {
                    match lightmap::domemesh::DomeMesh::load(std::path::Path::new(&dir)) {
                        Ok(m) => { eprintln!("env-from {dir}: the sky dome mesh ({} vertices, {} triangles) rasterised per peel", m.pos.len(), m.indices.len() / 3); prm.dome_mesh = Some(std::sync::Arc::new(m)); }
                        Err(e) => eprintln!("env-from {dir}: no dome mesh ({e}) — the analytic dome stands in"),
                    }
                }
            }
            // --raster-jitter / --no-raster-jitter: the game's per-direction LM raster offsets (default on under
            // --game-peel with --ss 1); --jitter-sign +1|-1 (the sampling side of the offset, under test)
            prm.raster_jitter = if has("--no-raster-jitter") { false } else if has("--raster-jitter") { true } else { prm.game_peel };
            if let Some(v) = f("--jitter-sign") { prm.jitter_sign = v.parse().expect("--jitter-sign"); }
            if let Some(v) = f("--max-dirs") { prm.max_dirs = v.parse().expect("--max-dirs"); }
            prm.profile = has("--profile");
            // the peel layer-count rule (peelcap::PeelStop): --peel-stop-threshold F (0.001 of the viewport), --peel-stop-lag L
            // (the query readback lag in layers, 2), --peel-layers N (a fixed item-layer count), --layers-by-rule (ignore the
            // captured counts a --frustum-from manifest carries)
            if let Some(v) = f("--peel-stop-threshold") { prm.peel_stop.threshold = v.parse().expect("--peel-stop-threshold"); }
            if let Some(v) = f("--peel-stop-lag") { prm.peel_stop.lag = v.parse().expect("--peel-stop-lag"); }
            if let Some(v) = f("--peel-layers") { prm.peel_layers_fixed = Some(v.parse().expect("--peel-layers")); }
            prm.layers_from_capture = !has("--layers-by-rule");
            // --layers-estimate: the stop rule on a census estimate (every 8th pixel) instead of the exact
            // dense depth-only pass (the default: the game's statistic over the whole viewport)
            prm.layers_estimate = has("--layers-estimate");
            // THE DIRECTION-RANGE SPLIT (contrib.rs): --dir-range a..b|k/N --contrib-out DIR writes the range's
            // per-direction contributions; --merge-contrib DIR[,DIR…] replays every direction's in issue order;
            // --sweep-only S runs sweep S alone (S ≥ 1 needs --field-from F = the previous sweep's merged field,
            // --field-out F writes a sweep's field for the next one)
            prm.contrib_out = f("--contrib-out").map(std::path::PathBuf::from);
            prm.merge_contrib = f("--merge-contrib").map(|v| v.split(',').map(|s| std::path::PathBuf::from(s.trim())).collect());
            let dir_range_arg = f("--dir-range");
            let sweep_only: Option<usize> = f("--sweep-only").map(|v| v.parse().expect("--sweep-only"));
            let field_from = f("--field-from");
            let field_out = f("--field-out");
            if prm.contrib_out.is_some() || prm.merge_contrib.is_some() { assert!(prm.lm_scene.is_none(), "the split does not carry the --lm-from harness accumulate"); }
            prm.obj_base = base;
            let game_manifest: Option<lightmap::passdump::Manifest> = f("--frustum-from").map(|p| {
                let txt = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("--frustum-from {p}: {e}"));
                lightmap::passdiff::read_manifest(&txt).unwrap_or_else(|e| panic!("--frustum-from {p}: {e}"))
            });
            // --lm-from PASSCAP_ROOT [--lm-env-frame 127448] [--fitted-world-box x0,z0,x1,z1]: the transcribed rows 7–9 in the
            // harness (lmaccum.rs) — the capture's own LM meshes / instance stream drive LmILightDir_Set over OUR peel
            // layers and the H-basis MRTs; the fitted blocks' world box from the frame's draws log (VS 17115's cbuffer)
            // unless given; every direction's ilightdir / MRTs compared with the capture's banked buffers (--frustum-from's
            // manifest, or --lm-game-manifest FILE) by the true issue index
            if let Some(lm_root) = f("--lm-from") {
                let root = std::path::PathBuf::from(&lm_root);
                let env_frame: u32 = f("--lm-env-frame").map(|v| v.parse().expect("--lm-env-frame")).unwrap_or(127448);
                let sc = lightmap::lmaccum::load_lm_scene(&root, env_frame).unwrap_or_else(|e| panic!("--lm-from {lm_root}: {e}"));
                eprintln!("lm-from: {} LM meshes, {} instances from env/frame{env_frame}", sc.meshes.len(), sc.instances.len());
                prm.fitted_world_box = match f("--fitted-world-box") {
                    Some(v) => { let c: Vec<f32> = v.split(',').map(|x| x.trim().parse().expect("--fitted-world-box x0,z0,x1,z1")).collect(); Some([[c[0], c[1]], [c[2], c[3]]]) }
                    None => {
                        // the first fitted block of the frame's log names the box
                        let draws = lightmap::lmaccum::load_draws(&root, env_frame).unwrap_or_default();
                        let blocks = lightmap::lmaccum::set_blocks(&draws, &sc).unwrap_or_default();
                        blocks.iter().flat_map(|b| b.draws.iter()).find_map(|d| d.world_box)
                    }
                };
                eprintln!("lm-from: the fitted blocks' world box {:?}", prm.fitted_world_box);
                let mp = f("--lm-game-manifest").map(std::path::PathBuf::from).or_else(|| f("--frustum-from").map(std::path::PathBuf::from)).unwrap_or_else(|| root.join("MANIFEST.json"));
                let entries = lightmap::lmaccum::load_capture_entries(&mp).unwrap_or_else(|e| panic!("{}: {e}", mp.display()));
                eprintln!("lm-from: {} capture entries from {} ({} ilightdir_final, {} banked hbasis0)", entries.len(), mp.display(), entries.iter().filter(|e| e.pass == "ilightdir_final").count(), entries.iter().filter(|e| e.pass == "hbasis0" && e.banked).count());
                prm.hbasis_game = Some((root, std::sync::Arc::new(entries)));
                prm.lm_scene = Some(std::sync::Arc::new(sc));
                // the sweep's H-basis MRTs are taken for the finalisation + the transcribed writer
                if prm.hb_out.is_none() { prm.hb_out = Some(std::sync::Arc::new(lightmap::ilatlas::HbSlot(std::sync::Mutex::new(None)))); }
            }
            // --ilightinput-from e2e|FILE (with --lm-from PASSCAP_ROOT): the peel colours from the game's ILightInput ATLAS — the
            // transcribed setup chain's dilated 17095 (e2e.rs: pre-pass → MDiffuse → shadow map → direct sun → PS 1038 / 17043 /
            // 1109 / 1335 × 8, computed here from the capture's frozen inputs when `e2e`, or a raw 2048² R11G11B10 / RGBA16F
            // image) sampled at every peel fragment's lightmap coordinate through the LM instance stream's ST (ilatlas.rs) —
            // in place of the port's per-fragment albedo × sun
            let mut e2e_out: Option<lightmap::e2e::ChainOut> = None;
            let mut from_map_setup: Option<lightmap::setupmap::FromMap> = None;
            if let Some(src) = f("--ilightinput-from").filter(|s| s != "map") {
                let lm_root = std::path::PathBuf::from(f("--lm-from").expect("--ilightinput-from needs --lm-from PASSCAP_ROOT (the LM instance stream)"));
                let env_frame: u32 = f("--lm-env-frame").map(|v| v.parse().expect("--lm-env-frame")).unwrap_or(127448);
                let pre_frame: u32 = f("--pre-frame").map(|v| v.parse().expect("--pre-frame")).unwrap_or(127447);
                let ti = std::time::Instant::now();
                let atlas = if src == "e2e" {
                    let out = lightmap::e2e::chain(&lm_root, pre_frame, env_frame, has("--skip-prepass"), true);
                    for (i, s) in out.stages.iter().enumerate() { eprintln!("  e2e stage {}: {} — {} exact / {} within 1 quantum / {} beyond of {}", i + 1, s.name, s.report.exact, s.report.ulp1, s.report.beyond, s.report.values); }
                    let il = out.ilightinput.clone();
                    e2e_out = Some(out);
                    il
                } else {
                    lightmap::ilatlas::load_atlas(std::path::Path::new(&src)).unwrap_or_else(|e| panic!("--ilightinput-from {src}: {e}"))
                };
                let mesh_dir = lm_root.join(format!("env/frame{env_frame}/mesh"));
                let insts = lightmap::prepass::read_maybe_gz(&mesh_dir.join("vb_17033.bin")).unwrap_or_else(|e| panic!("{e}"));
                let tile_vb = lightmap::prepass::read_maybe_gz(&mesh_dir.join("vb_5350.bin")).unwrap_or_else(|e| panic!("{e}"));
                let n_items = scene.item_count.max(1);
                let il = lightmap::ilatlas::IlAtlas::new(atlas, &insts, &tile_vb, n_items);
                let item_map = il.map_items(&scene);
                eprintln!("ilightinput-from {src}: atlas {}×{}, {} LM instances ({} items, {} tiles by footprint), port items mapped {:?} ({:.1}s)", il.buf.w, il.buf.h, il.insts.len(), n_items, il.tile_of.len(), item_map, ti.elapsed().as_secs_f32());
                prm.ilatlas = Some(std::sync::Arc::new(lightmap::ilatlas::IlSource { atlas: il, item_map }));
                prm.hb_out = Some(std::sync::Arc::new(lightmap::ilatlas::HbSlot(std::sync::Mutex::new(None))));
                prm.ambient_out = Some(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())));
            }
            if let Some(gm) = &game_manifest {
                // the sun shadow map's frustum (only with --shadow-frustum-from-capture: the sun pass's
                // conventions are the baker's transcription; the port's own frame otherwise) and the
                // per-direction peel frustums of sweep 0 (later sweeps below)
                if has("--shadow-frustum-from-capture") { if let Some(e) = gm.passes.iter().find(|e| e.pass == "sun_shadow") { if let Some(fr) = &e.frustum { prm.shadow_frustum = Some(fr.clone()); eprintln!("frustum-from: the sun shadow map's frustum adopted (centre {:?}, half {:?})", fr.center, fr.half); } } }
                let fs = lightmap::passdiff::peel_frustums_for(gm, 0, &prm.sphere_dirs);
                if !fs.is_empty() { eprintln!("frustum-from: sweep 0: {} directions' peels adopted ({} peels per direction) ({:.1}s since start)", fs.len(), fs.iter().map(|v| v.len()).max().unwrap_or(0), t0.elapsed().as_secs_f32()); prm.frustums = Some(std::sync::Arc::new(fs)); }
                // which of OUR direction indices carry a captured peel (the ones worth --dump-dirs)
                {
                    let mut seen: Vec<([f32; 3], String, u32)> = Vec::new();
                    for e in gm.passes.iter().filter(|e| (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep.unwrap_or(0) == 0) {
                        if let Some(d) = e.dir { if !seen.iter().any(|(v, _, _)| (v[0] - d[0]).abs() < 1e-4 && (v[1] - d[1]).abs() < 1e-4 && (v[2] - d[2]).abs() < 1e-4) { seen.push((d, e.capture.clone().unwrap_or_default(), e.frame.unwrap_or(0))); } }
                    }
                    let hits: Vec<String> = seen.iter().filter_map(|(d, cap, fr)| {
                        let (i, c) = prm.sphere_dirs.iter().enumerate().map(|(i, o)| (i, o[0] * d[0] + o[1] * d[1] + o[2] * d[2])).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
                        (c >= 0.999_99).then(|| format!("{cap} frame {fr} ({:.3}, {:.3}, {:.3}) = our direction {i}", d[0], d[1], d[2]))
                    }).collect();
                    if !hits.is_empty() && f("--dir-order").is_none() { eprintln!("frustum-from: captured peel directions: {}", hits.join("; ")); }
                }
                // the captured ITEM-LAYER COUNTS per direction and peel (the game's count is timing-dependent —
                // the pixel-count query is polled without waiting — so the harness renders as many layers as
                // the capture shows; --layers-by-rule uses the stop rule instead)
                let lc = lightmap::peelcap::captured_layer_counts(gm, 0, &prm.sphere_dirs);
                let n_known = lc.iter().filter(|v| v.iter().any(|c| c.is_some())).count();
                if n_known > 0 { eprintln!("frustum-from: {} directions' captured item-layer counts adopted: {}", n_known, lc.iter().enumerate().filter(|(_, v)| v.iter().any(|c| c.is_some())).map(|(i, v)| format!("dir {i} {:?}", v)).collect::<Vec<_>>().join(", ")); prm.peel_layer_counts = Some(std::sync::Arc::new(lc)); }
                if let Some(e) = gm.passes.iter().find(|e| e.pass == "peel_depth") { if e.width > 0 && e.width != prm.peel_res { eprintln!("frustum-from: peel resolution {} → {}", prm.peel_res, e.width); prm.peel_res = e.width; } }
            }
            if let Some(dir) = &dump_dir {
                let q: u32 = f("--quality").map(|s| s.parse().unwrap()).unwrap_or(3);
                let mood_name = xml_sel.map(|x| format!("{}/{}", x.collection, x.mood)).unwrap_or_default();
                let mut dmp = lightmap::passdump::PassDump::new(dir, &map_path, q, &mood_name, prm.ss).expect("--dump-passes dir");
                dmp.dirs = match f("--dump-dirs").as_deref() {
                    Some("all") => None,
                    // the directions the capture holds (by nearest vector; --frustum-from names the capture)
                    Some("game") => { let gm = game_manifest.as_ref().expect("--dump-dirs game needs --frustum-from GAME/MANIFEST.json"); let v = lightmap::passdiff::game_dir_indices(gm, 0, &prm.sphere_dirs); eprintln!("dump-dirs game: sweep 0 → our directions {:?}", v); Some(v) }
                    Some(s) if s.contains(',') => Some(s.split(',').map(|t| t.trim().parse().expect("--dump-dirs")).collect()),
                    Some(s) => Some((0..s.parse::<u32>().expect("--dump-dirs N|i,j,k|all")).collect()),
                    None => Some((0..8).collect()),
                };
                dmp.manifest.sun_dir = prm.sun_dir;
                dmp.manifest.sun_rgb = prm.sun;
                dmp.manifest.daytime_word = { let mf0 = tmmaps::map::MapFile::load(std::path::Path::new(&map_path)); lightmap::mapio::daytime(&mf0.gbx.body) };
                dmp.convention("depth", serde_json::json!("reversed_z01: 0.5 + (center·forward − p·forward)/(2·half.z); forward = the peel direction D (the camera looks from the receivers towards the sky)"));
                dmp.convention("layer0", serde_json::json!("the farthest real surface; the sky (the mood's Sky_p radiance along D) fills the facing texels before layer 0 as the game's first accumulate does (internally a synthetic dome layer)"));
                dmp.convention("layer_order", serde_json::json!("far-to-near (k = 0 farthest from the camera = nearest the sky)"));
                dmp.convention("depth_bias", serde_json::json!({"const": prm.depth_bias.0, "slope": prm.depth_bias.1, "target": format!("D{}", prm.depth_bits), "applied_to": "peel_depth (stored), the layer selection"}));
                dmp.convention("depth_clip", serde_json::json!(if prm.depth_clip { "fragments beyond the far plane dropped (DepthClipEnable)" } else { "pancaked: fragments beyond the far plane land on it at z01 = 0" }));
                dmp.convention("lookup_inset", serde_json::json!(if prm.peel_inset { "the Bias rows' (w−2)/w applied to the lookup alone (misregistered)" } else { "none: the layer viewport is inset by 1 px and the Bias rows' (w−2)/w registers to it (the capture)" }));
                dmp.convention("gather", serde_json::json!(if prm.per_subsample { "per sub-sample (ss² per layout texel)" } else { "per layout texel at the centroid of its covered sub-samples" }));
                dmp.convention("quantisers", serde_json::json!({"peel_color": prm.quant_peel.dxgi_rgb(), "ilightdir": prm.quant_ilightdir.dxgi_rgb(), "lightsum": prm.quant_accum.dxgi_rgb(), "rounding": format!("{:?}", prm.rounding)}));
                dmp.convention("chart_ss", serde_json::json!(format!("per-chart buffers at (2·w·{ss})×(2·h·{ss}) = the layout footprint × ss (the game's atlas × ss, cut by chart); `chart` = stored texels", ss = if prm.per_subsample { prm.ss } else { 1 })));
                dmp.convention("obj_base", serde_json::json!(base));
                dmp.convention("game_peel", serde_json::json!(prm.game_peel));
                dmp.convention("sweep0_sun", serde_json::json!(prm.sweep0_sun));
                dmp.convention("raster_jitter", serde_json::json!({"on": prm.raster_jitter, "cycle_texels_x9": prm.jitter_cycle, "sign": prm.jitter_sign, "rule": "direction k (issue order) at cycle[k mod 9]/9 layout texels; the sun pass walks all nine × 1/9"}));
                dmp.convention("dome", serde_json::json!(if prm.dome_exact { "transcribed dome per peel pixel (VS 16773 / PS 16774, mesh e001051 at the world origin)" } else { "one sky colour per direction" }));
                dmp.convention("accumulate", serde_json::json!(if prm.accum_hbasis { format!("H-basis C0: E += (4π/N)·P(n·D)·L × κ, P = 0.093506(3s²−1) + 0.398928 s + 0.199472, κ = {}", prm.hbasis_kappa) } else { "RNM: E += 4/N·max(0, n·D)·L".to_string() }));
                dmp.convention("frustum_source", serde_json::json!(if game_manifest.is_some() { "the captured MANIFEST (--frustum-from)" } else { "the receivers' bbox + 1 m, square, (res − 1) px over the larger extent; the far plane pushed out to every occluder (ground, sea, decoration)" }));
                prm.dump = Some(std::sync::Arc::new(std::sync::Mutex::new(dmp)));
            }
            // --layout-from REF.Map.Gbx: every item chart takes the reference bake's chart SIZE (its object id
            // = base + item), so the two bakes share texel grids — the gate then measures the lighting alone, not
            // the packer (a measurement aid; the product sizes charts by the game's allocation walk)
            let mut ref_rects: std::collections::HashMap<usize, [i32; 4]> = std::collections::HashMap::new();
            let ref_sizes: Option<std::collections::HashMap<usize, (u32, u32)>> = f("--layout-from").map(|rp| {
                let r = lightmap::mapio::load(&rp).expect("--layout-from");
                let d = r.chunk.data.expect("reference lightmap");
                let mp = d.cache.mapping().unwrap();
                let mut out = std::collections::HashMap::new();
                for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { out.insert((obj - base) as usize, ((mp.size[i].0 as u32 / 2).max(1), (mp.size[i].1 as u32 / 2).max(1))); ref_rects.insert((obj - base) as usize, [mp.pos[i].0 as i32, mp.pos[i].1 as i32, mp.size[i].0 as i32, mp.size[i].1 as i32]); } }
                eprintln!("layout from {rp}: {} item chart sizes", out.len());
                out
            });
            // --layout-game [--pak FILE:KEY] [--layout-quality Q]: THE GAME'S OWN CHART LAYOUT (layout::for_map — REPORT-5 §4-E.1/2:
            // the zone tiles' quality rings, the items' PreLightGen extents, the radix keys, the scale search, TryPack): the
            // items' chart SIZES and POSITIONS and the tiles' chart ST come from it; the mapping written is the game's table
            // (`lmtool packtest OURS.Map.Gbx --against EDITOR.Map.Gbx`). Supersedes --layout-from.
            let mut game_layout: Option<lightmap::layout::GameLayout> = None;
            if has("--layout-game") {
                let pak_arg = f("--pak");
                let pak: Option<(&str, &str)> = pak_arg.as_deref().and_then(|p| p.rsplit_once(':'));
                let q: u32 = f("--layout-quality").map(|v| v.parse().unwrap()).unwrap_or_else(|| f("--quality").map(|v| v.parse::<u32>().unwrap()).unwrap_or(3).saturating_sub(1));
                let t0 = std::time::Instant::now();
                // --kept FILE: the items that are records (RE 7's reduction list of the reference bakes) — the rest get no chart
                let layout_kept: Option<std::collections::HashSet<usize>> = f("--kept").map(|p| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("--kept {p}: {e}")).split(|c: char| c == ',' || c.is_whitespace()).filter_map(|t| t.trim().parse::<usize>().ok()).collect());
                let gl = lightmap::layout::for_map(&map_path, &scene, base, q, lightmap::layout::TilePlg::BLUEBAY_SEA, pak, &f("--collection").unwrap_or_else(|| "BlueBay".into()), &f("--zone").unwrap_or_else(|| "Sea".into()), layout_kept.as_ref()).unwrap_or_else(|e| panic!("--layout-game: {e}"));
                let bound = gl.charts.iter().filter(|c| c.charted == lightmap::layout::Charted::Bound).count();
                eprintln!("layout-game: {} charts ({bound} bound), s {} layout units/m, Σarea {} m², quality index {q} ({} iterations), keys from {} ({:.1}s)", gl.charts.len(), gl.s, gl.sum_area, gl.max_iter, if pak.is_some() { "the block records (pak)" } else { "the cell / triangle centres" }, t0.elapsed().as_secs_f32());
                game_layout = Some(gl);
            }
            let game_sizes: Option<std::collections::HashMap<usize, (u32, u32)>> = game_layout.as_ref().map(|gl| {
                let mut out = std::collections::HashMap::new();
                for c in &gl.charts { if c.obj >= base && c.charted == lightmap::layout::Charted::Bound { out.insert((c.obj - base) as usize, ((c.w as u32 / 2).max(1), (c.h as u32 / 2).max(1))); ref_rects.insert((c.obj - base) as usize, [c.x, c.y, c.w, c.h]); } }
                out
            });
            let ref_sizes = game_sizes.or(ref_sizes);
            // the tiles' chart ST from the layout (D's rule, peelcolor::chart_st, the Sea tile's PreLightGen bounds), per cell
            if let Some(gl) = &game_layout {
                let mut table: Vec<Option<[f32; 4]>> = vec![None; 64 * 64];
                for c in gl.charts.iter().filter(|c| c.obj < base) {
                    let (cx, cz) = gl.cell_of[c.obj as usize];
                    if (0..64).contains(&cx) && (0..64).contains(&cz) { table[(cz * 64 + cx) as usize] = Some(lightmap::peelcolor::chart_st([c.x, c.y, c.w, c.h], lightmap::layout::TilePlg::BLUEBAY_SEA.bounds, 2048.0)); }
                }
                prm.tile_st = Some(std::sync::Arc::new(table));
            }
            // --lm-from-map (with --layout-game [--pak FILE:KEY]): THE LM SCENE FROM THE MAP — the transcribed rows 7–9 over OUR peel
            // layers with the game's LM vertex stream rebuilt from the models (lmmesh.rs: bit-identical to pwc-day's captured
            // streams) and every instance's chart ST from the game's layout; no capture needed. The fitted blocks' world box =
            // the items' block-record boxes (lmtiles) unless --fitted-world-box. --lm-game-manifest FILE (+ --lm-cap-root DIR) still
            // compares every direction with the capture's banked buffers.
            if has("--lm-from-map") && prm.lm_scene.is_none() {
                let Some(gl) = game_layout.as_ref() else { panic!("--lm-from-map needs --layout-game") };
                let pak_arg = f("--pak");
                let pak: Option<(&str, &str)> = pak_arg.as_deref().and_then(|p| p.rsplit_once(':'));
                let tile_mesh = match pak { Some((pp, key)) => { let mut store = mapgeom::store::DataStore::empty(); store.add_pak(pp, key).expect("pak"); lightmap::lmmesh::lm_mesh_of_zone(&mut store, &f("--collection").unwrap_or_else(|| "BlueBay".into()), &f("--zone").unwrap_or_else(|| "Sea".into())).expect("zone tile mesh") } None => { eprintln!("lm-from-map: no --pak — the zone tiles have no LM mesh (items only)"); None } };
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let files = mapgeom::embedded::files(&mf).expect("embedded items");
                let by_name: std::collections::BTreeMap<String, Vec<u8>> = files.iter().map(|(k, v)| (k.rsplit(['/', '\\']).next().unwrap_or(k).to_string(), v.clone())).collect();
                let sc = lightmap::lmmesh::lm_scene_from_map(&scene, gl, base, &|name| by_name.get(name).cloned(), tile_mesh, lightmap::layout::TilePlg::BLUEBAY_SEA, 2048.0).unwrap_or_else(|e| panic!("--lm-from-map: {e}"));
                eprintln!("lm-from-map: {} LM meshes, {} instances from the map's models + the layout", sc.meshes.len(), sc.instances.len());
                prm.fitted_world_box = match f("--fitted-world-box") {
                    Some(v) => { let c: Vec<f32> = v.split(',').map(|x| x.trim().parse().expect("--fitted-world-box x0,z0,x1,z1")).collect(); Some([[c[0], c[1]], [c[2], c[3]]]) }
                    None => {
                        // the items' block records' union box (RE 6: the fitted peel tile = the records' cell of the tiling; pwc-day: one cell)
                        let recs: Vec<lightmap::lmtiles::BlockRecord> = lightmap::lmtiles::item_records(&scene, 1.0, false).iter().filter_map(|it| it.record).collect();
                        if recs.is_empty() { None } else { let sbox = lightmap::lmtiles::scene_box(&recs); Some([[sbox.min()[0], sbox.min()[2]], [sbox.max()[0], sbox.max()[2]]]) }
                    }
                };
                eprintln!("lm-from-map: the fitted blocks' world box {:?}", prm.fitted_world_box);
                if let Some(mp) = f("--lm-game-manifest").map(std::path::PathBuf::from) {
                    let root = f("--lm-cap-root").map(std::path::PathBuf::from).unwrap_or_else(|| mp.parent().map(|p| p.to_path_buf()).unwrap_or_default());
                    let entries = lightmap::lmaccum::load_capture_entries(&mp).unwrap_or_else(|e| panic!("{}: {e}", mp.display()));
                    eprintln!("lm-from-map: {} capture entries from {} for the comparison", entries.len(), mp.display());
                    prm.hbasis_game = Some((root, std::sync::Arc::new(entries)));
                }
                prm.lm_scene = Some(std::sync::Arc::new(sc));
                // the sweep's H-basis MRTs are taken for the finalisation + the transcribed writer
                if prm.hb_out.is_none() { prm.hb_out = Some(std::sync::Arc::new(lightmap::ilatlas::HbSlot(std::sync::Mutex::new(None)))); }
            }
                // --ilightinput-from map: THE SETUP CHAIN FROM THE MAP (setupmap.rs) — the sun camera fit, the shadow map, the direct sun,
                // the nine pre-pass runs and the ILightInput chain on E's from-map LM scene; only the collection / zone tables named in
                // the log come from the captured environment (--env-from ROOT: prepass_check::frozen_tables). With --lm-from ROOT every
                // stage is also compared with the capture's buffers.
                if f("--ilightinput-from").as_deref() == Some("map") {
                    let ti = std::time::Instant::now();
                    let lm = prm.lm_scene.clone().expect("--ilightinput-from map needs --lm-from-map");
                    let env_root = std::path::PathBuf::from(f("--env-from").expect("--ilightinput-from map needs --env-from PASSCAP_ROOT for the frozen collection tables"));
                    let mut frozen = lightmap::prepass_check::frozen_tables(&env_root, 127447, 127448).unwrap_or_else(|e| panic!("frozen tables: {e}"));
                    // the collection tables from the pak(s) (RE 8's chain): every --pak FILE:KEY on the line opens a store; the zone tiles'
                    // Pxz texture name from --tile-pxz (SeaFloor for BlueBay's Sea zone), the mood from --mood-name (Day)
                    let mut pak_notes = Vec::new();
                    {
                        let paks: Vec<String> = a.iter().enumerate().filter(|(_, x)| *x == "--pak").filter_map(|(i, _)| a.get(i + 1).cloned()).collect();
                        if !paks.is_empty() {
                            let mut store = mapgeom::store::DataStore::empty();
                            for p in &paks { if let Some((pp, key)) = p.rsplit_once(':') { store.add_pak(pp, key).unwrap_or_else(|e| panic!("--pak {p}: {e}")); } }
                            let collection = f("--collection").unwrap_or_else(|| "BlueBay".into());
                            // RE 8's paktables (the material chain, the water descriptor, the LUT image + generator) — the zone tiles' material link
                            // from --tile-material (default <Coll>\Media\Material\SeaFloor: BlueBay's Sea zone); the interim corner-mean path is the fallback
                            let tile_link = f("--tile-material").unwrap_or_else(|| format!("{collection}\\Media\\Material\\SeaFloor"));
                            if let Err(e) = lightmap::setupmap::tables_from_paktables(&mut frozen, &mut store, &collection, &tile_link, &scene, &mut pak_notes) {
                                pak_notes.push(format!("paktables: {e} — the interim pak path (corner means + WaterColor.tga + the descriptor table) is used"));
                                let mut read = |path: &str| -> Option<Vec<u8>> { store.read(path).ok().map(|b| b.to_vec()) };
                                lightmap::setupmap::tables_from_pak(&mut frozen, &mut read, &collection, &f("--mood-name").unwrap_or_else(|| "Day".into()), &f("--tile-pxz").unwrap_or_else(|| "SeaFloor".into()), &mut pak_notes);
                            }
                        }
                    }
                    for n in &pak_notes { eprintln!("setup-from-map: {n}"); }
                    if pak_notes.is_empty() {
                        eprintln!("setup-from-map: FROZEN from the capture — the terrain constants (tile slices {:?} → {:?}, Land → {:?}), the TrackWall constant {:?}, the water id map / plane tables / LUTs 15075 + 15078 ({:.1}s)", frozen.tile_slices, frozen.tile_rgb, frozen.wall_rgb, frozen.pad_rgb, ti.elapsed().as_secs_f32());
                    } else {
                        eprintln!("setup-from-map: the collection tables from the pack ({:.1}s)", ti.elapsed().as_secs_f32());
                    }
                    // the scene box S = the union of the LM scene's placed vertices (the block records' union on pwc-day: the tiles at
                    // y 3.9999785 over 0..2048, the items)
                    let mut sbox = lightmap::lightcam::Aabb { min: [f32::MAX; 3], max: [f32::MIN; 3] };
                    for (k, mesh) in lm.meshes.iter().enumerate() {
                        for inst in lm.instances.iter().skip(lm.inst_first[k]).take(lm.inst_count[k]) {
                            let r = lightmap::sunpass::rotation_rows(inst.q);
                            for v in &mesh.verts {
                                let p = v.pos;
                                let w = [r[0][0] * p[0] + r[0][1] * p[1] + r[0][2] * p[2] + inst.t[0], r[1][0] * p[0] + r[1][1] * p[1] + r[1][2] * p[2] + inst.t[1], r[2][0] * p[0] + r[2][1] * p[1] + r[2][2] * p[2] + inst.t[2]];
                                for c in 0..3 { sbox.min[c] = sbox.min[c].min(w[c]); sbox.max[c] = sbox.max[c].max(w[c]); }
                            }
                        }
                    }
                    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                    let files = mapgeom::embedded::files(&mf).expect("embedded items");
                    let by_name: std::collections::BTreeMap<String, Vec<u8>> = files.iter().map(|(k, v)| (k.rsplit(['/', '\\']).next().unwrap_or(k).to_string(), v.clone())).collect();
                    let item_bytes = |name: &str| -> Option<Vec<u8>> { by_name.get(name).cloned().or_else(|| by_name.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())) };
                    let dir_in_world = [-prm.sun_dir[0], -prm.sun_dir[1], -prm.sun_dir[2]];
                    let fm = lightmap::setupmap::build(&scene, &lm, &sbox, dir_in_world, prm.sun, &frozen, &item_bytes, false);
                    for n in &fm.notes { eprintln!("setup-from-map: {n}"); }
                    // against the capture: the stages the e2e chain compares (the same entries)
                    if let Some(root) = f("--lm-from").map(std::path::PathBuf::from) {
                        if let Ok(txt) = std::fs::read_to_string(root.join("MANIFEST.json")) {
                            if let Ok(m) = lightmap::passdiff::read_manifest(&txt) {
                                let cmp = |name: &str, ours: &lightmap::passdiff::Buf, pass: &str, frame: u32, suffix: &str, ch: u32, fmt: lightmap::gpucmp::Fmt| {
                                    let cands = m.passes.iter().filter(|e| e.pass == pass && e.frame == Some(frame) && e.file.contains(suffix));
                                    // the sun shadow map = the pass's first snapshot (the later ones are the peel-phase depth targets); the others = the last
                                    let e = if pass == "sun_shadow" { cands.min_by_key(|e| e.eid_last.unwrap_or(0)) } else { cands.max_by_key(|e| e.eid_last.unwrap_or(0)) };
                                    match e.and_then(|e| lightmap::passdiff::load_entry(&root, e).ok()) {
                                        Some(cap) => { let r = lightmap::gpucmp::compare_where(ours, &cap, ch, fmt, &|_x, _y| true); eprintln!("setup-from-map: {name} vs the captured {pass}: {} exact / {} within 1 quantum / {} beyond of {} (max |Δ| {:.6} at {:?})", r.exact, r.ulp1, r.beyond, r.values, r.max_abs, r.worst); }
                                        None => eprintln!("setup-from-map: {name}: no captured {pass} entry"),
                                    }
                                };
                                cmp("shadow map", &fm.shadow, "sun_shadow", 127448, "", 1, lightmap::gpucmp::Fmt::Exact);
                                cmp("direct sun", &fm.sun, "sun_direct", 127448, "", 4, lightmap::gpucmp::Fmt::F16);
                                cmp("pre-pass 16963", &fm.attr, "atlas_attr_2", 127447, "", 4, lightmap::gpucmp::Fmt::F16);
                                cmp("MDiffuse 16969", &fm.mdiffuse8, "setup_ps17043", 127448, "_16969", 4, lightmap::gpucmp::Fmt::Unorm8);
                                if let Some(cap) = m.passes.iter().filter(|e| e.pass == "setup_ps17043" && e.frame == Some(127448) && e.file.contains("_16969")).max_by_key(|e| e.eid_last.unwrap_or(0)).and_then(|e| lightmap::passdiff::load_entry(&root, e).ok()) {
                                    let (mut per_ch, mut shown) = ([0usize; 4], 0);
                                    for y in 0..2048u32 { for x in 0..2048u32 { let mut diff = false; for c in 0..4 { if (fm.mdiffuse8.get(x, y, c) - cap.get(x, y, c)).abs() > 0.5 / 255.0 { per_ch[c as usize] += 1; diff = true; } } if diff && shown < 6 && (x + y) % 7 == 0 { eprintln!("setup-from-map:   MDiffuse ({x}, {y}): ours [{:.4}, {:.4}, {:.4}, {:.4}] captured [{:.4}, {:.4}, {:.4}, {:.4}]", fm.mdiffuse8.get(x, y, 0), fm.mdiffuse8.get(x, y, 1), fm.mdiffuse8.get(x, y, 2), fm.mdiffuse8.get(x, y, 3), cap.get(x, y, 0), cap.get(x, y, 1), cap.get(x, y, 2), cap.get(x, y, 3)); shown += 1; } } }
                                    eprintln!("setup-from-map:   MDiffuse values off per channel {:?}", per_ch);
                                }
                                cmp("ILightInput 17095 dilated", &fm.ilightinput, "setup_ps1335", 127448, "_rt0_", 3, lightmap::gpucmp::Fmt::R11G11B10);
                            }
                        }
                    }
                    if let Some(dir) = f("--chain-final-dir") {
                        let dump = |name: &str, b: &lightmap::passdiff::Buf| { let mut out = Vec::with_capacity(b.data.len() * 4); for x in &b.data { out.extend_from_slice(&x.to_le_bytes()); } std::fs::write(format!("{dir}/{name}"), out).expect("write"); };
                        dump("frommap-shadow.f32", &fm.shadow); dump("frommap-sun.f32", &fm.sun); dump("frommap-attr.f32", &fm.attr); dump("frommap-mdiffuse8.f32", &fm.mdiffuse8); dump("frommap-ilightinput.f32", &fm.ilightinput);
                    }
                    let il = lightmap::ilatlas::IlAtlas::from_lm_scene(fm.ilightinput.clone(), &lm);
                    let item_map = il.map_items(&scene);
                    eprintln!("setup-from-map: the peels colour from OUR from-map ILightInput atlas — {} LM instances ({} items, {} tiles by footprint), port items mapped {:?} ({:.1}s total)", il.insts.len(), il.n_items, il.tile_of.len(), item_map, ti.elapsed().as_secs_f32());
                    prm.ilatlas = Some(std::sync::Arc::new(lightmap::ilatlas::IlSource { atlas: il, item_map }));
                    if prm.hb_out.is_none() { prm.hb_out = Some(std::sync::Arc::new(lightmap::ilatlas::HbSlot(std::sync::Mutex::new(None)))); }
                    if prm.ambient_out.is_none() { prm.ambient_out = Some(std::sync::Arc::new(std::sync::Mutex::new(Vec::new()))); }
                    from_map_setup = Some(fm);
                }
            // THE ZONE TILES' CHART ST from the capture's instance buffer (`lmaccum::load_lm_scene`: the 4096-instance
            // object's g_InstanceDatas — t = the cell's corner, st = its chart ST; the mapping's obj index = the instance
            // index, a traversal RE-6's block records describe): the harness's tile colour path
            if let (Some(dir), true) = (f("--env-from"), f("--frustum-from").is_some()) {
                match lightmap::lmaccum::load_lm_scene(std::path::Path::new(&dir), 127448) {
                    Ok(sc) => {
                        let mut table: Vec<Option<[f32; 4]>> = vec![None; 64 * 64];
                        let mut n = 0usize;
                        for (k, &cnt) in sc.inst_count.iter().enumerate() {
                            if cnt < 4096 { continue; }
                            for inst in sc.instances.iter().skip(sc.inst_first[k]).take(cnt) {
                                let (cx, cz) = ((inst.t[0] / 32.0).round() as i64, (inst.t[2] / 32.0).round() as i64);
                                if (0..64).contains(&cx) && (0..64).contains(&cz) { table[(cz * 64 + cx) as usize] = Some(inst.st); n += 1; }
                            }
                        }
                        if n > 0 { eprintln!("env-from {dir}: the zone tiles' chart ST of {n} cells from the captured instance buffer"); prm.tile_st = Some(std::sync::Arc::new(table)); }
                    }
                    Err(e) => eprintln!("env-from {dir}: no tile ST table ({e})"),
                }
            }
            // the items' layout rects per instance (the peel colour's chart ST: `peelcolor::chart_st`)
            if !ref_rects.is_empty() {
                prm.chart_rects = Some(std::sync::Arc::new(scene.instances.iter().map(|inst| ref_rects.get(&inst.item).copied()).collect()));
            }
            // --ilightinput-from FILE.dds[.gz]: the captured ILightInput atlas (PS 17131's SRV1) as the peel
            // colour's texture — the harness's transcription of the colour path on the game's own input
            // (integration: the value `e2e` belongs to the chain's handler above — ilatlas from the transcribed setup
            // chain — and is not a file; D's peelcolor path takes a file only)
            if let Some(path) = f("--ilightinput-from").filter(|p| p != "e2e" && p != "map") {
                let pb = std::path::PathBuf::from(&path);
                let (root, file) = (pb.parent().map(|p| p.to_path_buf()).unwrap_or_default(), pb.file_name().unwrap().to_string_lossy().to_string());
                let mut e = lightmap::passdump::entry("ilightinput", file, "atlas");
                e.format = "R11G11B10_FLOAT".into();
                let b = lightmap::passdiff::load_entry(&root, &e).unwrap_or_else(|er| panic!("--ilightinput-from {path}: {er}"));
                let nz = (0..b.h).flat_map(|y| (0..b.w).map(move |x| (x, y))).filter(|&(x, y)| b.get(x, y, 0) != 0.0 || b.get(x, y, 1) != 0.0 || b.get(x, y, 2) != 0.0).count();
                eprintln!("ilightinput-from {path}: {}×{} atlas, {nz} non-zero texels — the peel colour samples it at the LM uv", b.w, b.h);
                prm.ilight_atlas = Some(std::sync::Arc::new(lightmap::peelcolor::AtlasTex::from_buf(&b)));
            }
            let chart_sizes = |p: &lightmap::bake::BakeParams| -> Vec<(u32, u32)> {
                scene.instances.iter().map(|inst| {
                    if let Some(rs) = &ref_sizes { if let Some(&s) = rs.get(&inst.item) { return s; } }
                    let m = &scene.models[inst.model]; let sc = (inst.xf[0] * inst.xf[0] + inst.xf[1] * inst.xf[1] + inst.xf[2] * inst.xf[2]).sqrt(); lightmap::bake::chart_size(m, sc, p)
                }).collect()
            };
            // --sky-probe GAME/MANIFEST.json: the FIRST DIVERGENT PASS's own tool — the game's dome colour along
            // every captured direction against our sky model for the same vector (per-channel ratios, the
            // elevation), then exit; --sky-probe-fit tries the model's switches and reports the best
            // --dir-order GAME/MANIFEST.json: bake the first sweep's directions in the GAME's issue order (from
            // its accumulation snapshots), so the accumulation after direction k is the game's after k; the
            // directions the capture has not reached yet follow in our order. --dump-lightsum-after LIST|game:
            // dump the accumulation target after those directions (game = the banked snapshots' indices)
            if let Some(gm) = f("--dir-order") {
                let game = lightmap::passdiff::read_manifest(&std::fs::read_to_string(&gm).expect("--dir-order manifest")).expect("--dir-order manifest");
                let ord = lightmap::passdiff::game_issue_order(&game, 0, &prm.sphere_dirs);
                let mut used = vec![false; prm.sphere_dirs.len()];
                let mut new_dirs: Vec<[f32; 3]> = Vec::new();
                // the source index of every new position (the per-direction tables below follow the permutation)
                let mut src: Vec<usize> = Vec::new();
                let mut worst = 0.0f32;
                for (k, oi) in &ord {
                    if new_dirs.len() != *k as usize { eprintln!("dir-order: the capture's issue order has a gap before position {k} (have {}); the order is followed as far as it goes", new_dirs.len()); break; }
                    if used[*oi as usize] { eprintln!("dir-order: our direction {oi} matched twice; stopping at position {k}"); break; }
                    used[*oi as usize] = true;
                    let gd = game.passes.iter().find(|e| e.sweep_direction_index == Some(*k) && e.dir.is_some()).and_then(|e| e.dir).unwrap();
                    let od = prm.sphere_dirs[*oi as usize];
                    worst = worst.max((od[0] * gd[0] + od[1] * gd[1] + od[2] * gd[2]).clamp(-1.0, 1.0).acos().to_degrees());
                    new_dirs.push(od);
                    src.push(*oi as usize);
                }
                let matched = new_dirs.len();
                for (i, d) in prm.sphere_dirs.iter().enumerate() { if !used[i] { new_dirs.push(*d); src.push(i); } }
                eprintln!("dir-order: {matched} of {} directions in the game's issue order (worst match {worst:.3}°), the rest in ours ({:.1}s since start)", new_dirs.len(), t0.elapsed().as_secs_f32());
                prm.sphere_dirs = std::sync::Arc::new(new_dirs);
                // THE PER-DIRECTION TABLES FOLLOW THE REORDER: --frustum-from filled the peel frusta and the captured
                // item-layer counts per direction of the ORIGINAL list; direction k of the reordered list is the
                // original src[k] (without this, every direction but the first peeled through another one's frustum)
                if let Some(fs) = &prm.frustums {
                    let re: Vec<Vec<lightmap::passdump::Frustum>> = src.iter().map(|&i| fs.get(i).cloned().unwrap_or_default()).collect();
                    prm.frustums = Some(std::sync::Arc::new(re));
                }
                if let Some(lc) = &prm.peel_layer_counts {
                    let re: Vec<Vec<Option<usize>>> = src.iter().map(|&i| lc.get(i).cloned().unwrap_or_default()).collect();
                    prm.peel_layer_counts = Some(std::sync::Arc::new(re));
                }
                // which of the reordered directions carry a captured peel (the ones worth --dump-dirs)
                if let Some(gm) = &game_manifest {
                    let mut seen: Vec<([f32; 3], String, u32)> = Vec::new();
                    for e in gm.passes.iter().filter(|e| (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep.unwrap_or(0) == 0) {
                        if let Some(d) = e.dir { if !seen.iter().any(|(v, _, _)| (v[0] - d[0]).abs() < 1e-4 && (v[1] - d[1]).abs() < 1e-4 && (v[2] - d[2]).abs() < 1e-4) { seen.push((d, e.capture.clone().unwrap_or_default(), e.frame.unwrap_or(0))); } }
                    }
                    let hits: Vec<String> = seen.iter().filter_map(|(d, cap, fr)| {
                        let (i, c) = prm.sphere_dirs.iter().enumerate().map(|(i, o)| (i, o[0] * d[0] + o[1] * d[1] + o[2] * d[2])).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
                        (c >= 0.999_99).then(|| format!("{cap} frame {fr} ({:.3}, {:.3}, {:.3}) = direction {i}", d[0], d[1], d[2]))
                    }).collect();
                    if !hits.is_empty() { eprintln!("dir-order: captured peel directions after the reorder: {}", hits.join("; ")); }
                }
                if let Some(v) = f("--dump-lightsum-after") {
                    prm.lightsum_after = if v == "game" {
                        game.passes.iter().filter(|e| e.pass == "hbasis0" && e.banked.unwrap_or(true) && e.sweep.unwrap_or(0) == 0).filter_map(|e| e.sweep_direction_index).filter(|k| (*k as usize) < matched).collect()
                    } else { v.split(',').map(|x| x.trim().parse().expect("--dump-lightsum-after")).collect() };
                    eprintln!("dump-lightsum-after: {:?}", prm.lightsum_after);
                }
            } else if let Some(v) = f("--dump-lightsum-after") {
                prm.lightsum_after = v.split(',').map(|x| x.trim().parse().expect("--dump-lightsum-after")).collect();
            }
            // --sun-dir-in-world x,y,z: the bake's light direction as the capture's cbuffer holds it
            // (GbxP_LightDirDirInWorld0 / DirInWorld, the direction the light travels; the sun is at −it)
            if let Some(v) = f("--sun-dir-in-world") {
                let c: Vec<f32> = v.split(',').map(|x| x.trim().parse().expect("--sun-dir-in-world x,y,z")).collect();
                let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
                prm.sun_dir = [-c[0] / l, -c[1] / l, -c[2] / l];
                eprintln!("sun from the capture: toward the sun ({:.5}, {:.5}, {:.5}) = az {:.2}° el {:.2}°", prm.sun_dir[0], prm.sun_dir[1], prm.sun_dir[2], prm.sun_dir[0].atan2(prm.sun_dir[2]).to_degrees().rem_euclid(360.0), prm.sun_dir[1].asin().to_degrees());
            }
            // the sky's lobes are around the SUN — the gradient was built before the mood fixed the sun
            // direction, so it carried the parameter default (0.508, 0.609, 0.609) until now
            if let Some(g) = &prm.sky_grad {
                if g.sun_dir != prm.sun_dir {
                    let mut g2 = (**g).clone();
                    g2.sun_dir = prm.sun_dir;
                    prm.sky_grad = Some(std::sync::Arc::new(g2));
                }
            }
            if std::env::var_os("LMTOOL_BENCH_DOME").is_some() {
                if let Some(sg) = &prm.sky_grad {
                    let t = std::time::Instant::now();
                    let mut acc = 0.0f32;
                    for i in 0..1_000_000u32 {
                        let q = [871.0 + (i % 1000) as f32 * 0.01, 50.0, 353.0 + (i / 1000) as f32 * 0.01];
                        let v = sg.dome_radiance(q, [0.345, 0.117, 0.931], [871.0, 50.0, 353.0]);
                        acc += v[0];
                    }
                    eprintln!("bench: 1 M dome_radiance in {:.3}s ({:.0} ns each), checksum {acc}", t.elapsed().as_secs_f32(), t.elapsed().as_nanos() as f64 / 1e6);
                    // pure-compute scaling (no shared memory at all), short and long workloads
                    for (nt, total) in [(4usize, 40_000_000u32), (16, 40_000_000), (64, 40_000_000), (160, 40_000_000), (16, 800_000_000), (64, 800_000_000), (160, 800_000_000)] {
                        let t = std::time::Instant::now();
                        std::thread::scope(|sc| {
                            for k in 0..nt {
                                sc.spawn(move || {
                                    let mut acc = 0.0f64;
                                    for i in 0..(total / nt as u32) {
                                        acc += ((i + k as u32) as f64 * 1e-3).sin();
                                    }
                                    std::hint::black_box(acc);
                                });
                            }
                        });
                        eprintln!("bench: {} M sin on {nt} threads in {:.3}s", total / 1_000_000, t.elapsed().as_secs_f32());
                    }
                    for nt in [4usize, 16, 64, 160] {
                        let t = std::time::Instant::now();
                        std::thread::scope(|sc| {
                            for k in 0..nt {
                                let sg = sg.clone();
                                sc.spawn(move || {
                                    let mut acc = 0.0f32;
                                    for i in 0..(4_000_000u32 / nt as u32) {
                                        let q = [871.0 + ((i + k as u32 * 7) % 1000) as f32 * 0.01, 50.0, 353.0 + (i / 1000) as f32 * 0.01];
                                        let v = sg.dome_radiance(q, [0.345, 0.117, 0.931], [871.0, 50.0, 353.0]);
                                        acc += v[0];
                                    }
                                    std::hint::black_box(acc);
                                });
                            }
                        });
                        eprintln!("bench: 4 M dome_radiance on {nt} threads in {:.3}s ({:.0} ns each per thread)", t.elapsed().as_secs_f32(), t.elapsed().as_nanos() as f64 / 4e6 * nt as f64);
                    }
                }
                return;
            }
            if let Some(gm) = f("--sky-probe") {
                // (--game-dir DIR: where the capture's files live when the manifest is a frozen copy elsewhere)
                let root = f("--game-dir").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::Path::new(&gm).parent().unwrap().to_path_buf());
                let game = lightmap::passdiff::read_manifest(&std::fs::read_to_string(&gm).expect("--sky-probe manifest")).expect("--sky-probe manifest");
                let mut domes = lightmap::passdiff::dome_colours(&game, &root);
                domes.sort_by(|a, b| a.0[1].partial_cmp(&b.0[1]).unwrap());
                // ours: the transcribed dome (`SkyGradient::dome_radiance`, per pixel: the frame-centre ray of the
                // peel, the eye at the frustum centre) next to the port's old per-direction model
                println!("| game dir / peel | D (x, y, z) | elevation | frame centre | game dome (R11G11B10) | dome transcription | ratio | old model | ratio |");
                println!("|---|---|---|---|---|---|---|---|---|");
                let (mut lr, mut lr_old, mut n) = ([0f64; 3], [0f64; 3], 0usize);
                for (d, g, di, pi, fr) in &domes {
                    let o_old = lightmap::bake::sky_radiance(&prm, *d);
                    let centre = fr.as_ref().map(|f| f.center).unwrap_or([1024.0, 71.0, 1024.0]);
                    let o = match &prm.sky_grad { Some(sg) => { let v = sg.dome_radiance(centre, *d, centre); lightmap::gpufmt::quantise_r11g11b10(v, prm.rounding) } None => o_old };
                    let r = [o[0] / g[0].max(1e-6), o[1] / g[1].max(1e-6), o[2] / g[2].max(1e-6)];
                    let r_old = [o_old[0] / g[0].max(1e-6), o_old[1] / g[1].max(1e-6), o_old[2] / g[2].max(1e-6)];
                    println!("| {di} / {pi} | ({:.3}, {:.3}, {:.3}) | {:+.1}° | ({:.0}, {:.0}, {:.0}) | ({:.4}, {:.4}, {:.4}) | ({:.4}, {:.4}, {:.4}) | ({:.3}, {:.3}, {:.3}) | ({:.4}, {:.4}, {:.4}) | ({:.3}, {:.3}, {:.3}) |", d[0], d[1], d[2], d[1].asin().to_degrees(), centre[0], centre[1], centre[2], g[0], g[1], g[2], o[0], o[1], o[2], r[0], r[1], r[2], o_old[0], o_old[1], o_old[2], r_old[0], r_old[1], r_old[2]);
                    if g[1] > 1e-3 { for c in 0..3 { lr[c] += (r[c] as f64).ln(); lr_old[c] += (r_old[c] as f64).ln(); } n += 1; }
                }
                if n > 0 { println!("\ngeometric mean ratio ours/game over {n} dome layers: transcription ({:.4}, {:.4}, {:.4}), old model ({:.4}, {:.4}, {:.4})", (lr[0] / n as f64).exp(), (lr[1] / n as f64).exp(), (lr[2] / n as f64).exp(), (lr_old[0] / n as f64).exp(), (lr_old[1] / n as f64).exp(), (lr_old[2] / n as f64).exp()); }
                // --sky-probe-fit: which VS u shift (in the mesh's az/π units) each dome layer fits best, and the
                // best common shift — the LightDirAngle_m11Zx convention under test
                if has("--sky-probe-fit") {
                    if let Some(sg) = &prm.sky_grad {
                        let err = |shift: f32, only: Option<usize>| -> f64 {
                            let mut e = 0.0f64;
                            for (i, (d, g, _, _, fr)) in domes.iter().enumerate() {
                                if let Some(o) = only { if o != i { continue; } }
                                if g[1] <= 1e-3 { continue; }
                                let centre = fr.as_ref().map(|f| f.center).unwrap_or([1024.0, 71.0, 1024.0]);
                                let o = sg.dome_radiance_shift(centre, *d, centre, Some(shift));
                                for c in 0..3 { e += ((o[c] / g[c].max(1e-6)) as f64).ln().powi(2); }
                            }
                            e
                        };
                        let mut best = (f64::MAX, 0.0f32);
                        for i in 0..400 { let s = -1.0 + i as f32 / 200.0; let e = err(s, None); if e < best.0 { best = (e, s); } }
                        println!("\nbest common u shift: {:.3} (rms log-ratio {:.4}); per layer:", best.1, (best.0 / (3.0 * n as f64)).sqrt());
                        for (i, (d, _, di, pi, _)) in domes.iter().enumerate() {
                            let mut b = (f64::MAX, 0.0f32);
                            for k in 0..400 { let s = -1.0 + k as f32 / 200.0; let e = err(s, Some(i)); if e < b.0 { b = (e, s); } }
                            println!("  dir {di} peel {pi} D ({:.3}, {:.3}, {:.3}): best shift {:.3} (rms {:.4}); u_mesh {:.4}", d[0], d[1], d[2], b.1, (b.0 / 3.0).sqrt(), d[0].atan2(d[2]) / std::f32::consts::PI);
                        }
                        println!("  the capture's LightDirAngle_m11Zx = sun az/π = {:.4}", prm.sun_dir[0].atan2(prm.sun_dir[2]) / std::f32::consts::PI);
                    }
                }
                return;
            }
            // THE PEEL CAMERAS WITHOUT A CAPTURE (tiledpeel.rs): the world peel + the tiling rule's fitted tiles per
            // direction, fit by the transcribed light camera — unless --frustum-from gave the captured ones or
            // --no-tiles asks for the port's single frame; --tile-scale S = the allocation scale in layout units
            // per metre (default: the port's atlas density × 2), --tile-quality Q (the EHmsLightMapQuality, 3),
            // --tile-vram-mb N (8192), --tile-max N (4)
            let peel_plan: Option<lightmap::tiledpeel::PeelPlan> = if prm.game_peel && prm.raster_peel && prm.frustums.is_none() && !has("--no-tiles") {
                let gq: f32 = f("--global-quality").map(|v| v.parse().unwrap()).unwrap_or(1.0);
                let recs: Vec<lightmap::lmtiles::BlockRecord> = lightmap::lmtiles::item_records(&scene, gq, has("--lod0")).iter().filter_map(|it| it.record).collect();
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let size = [mf.size[0].max(0) as u32, mf.size[1].max(0) as u32, mf.size[2].max(0) as u32];
                // the zone tiles' box: the seabed quads over the map footprint (see the zone tiles above)
                let envir_l = hdr.as_ref().map(|h| h.envir.to_ascii_lowercase()).unwrap_or_default();
                let sea_y: f32 = f("--sea-y").map(|s| s.parse().unwrap()).unwrap_or(match envir_l.as_str() { "bluebay" => 7.0, "redisland" => -0.3, "whiteshore" => -1.0, "greencoast" => -0.8, _ => f32::NAN });
                let tiles_box = if sea_y.is_finite() && !has("--no-zone-tiles") && size[0] > 0 && size[2] > 0 {
                    let (w, d) = (size[0] as f32 * 32.0, size[2] as f32 * 32.0);
                    Some(lightmap::lmtiles::CBox::from_min_max([0.0, sea_y - 3.0, 0.0], [w, sea_y - 3.0, d]))
                } else { None };
                let scene_ch = { let mut s = lightmap::lmtiles::scene_box(&recs); if let Some(t) = &tiles_box { if s.is_valid() { s.union_into(t); } else { s = *t; } } s };
                let (_, _, _, chunks_aabb) = lightmap::probechunk::for_records(size, [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, &recs, &scene_ch, 2048);
                let alloc_scale: f32 = f("--tile-scale").map(|v| v.parse().unwrap()).unwrap_or(prm.texels_per_m * 2.0);
                let tq: u32 = f("--tile-quality").map(|v| v.parse().unwrap()).unwrap_or(3);
                let vram: i64 = f("--tile-vram-mb").map(|v| v.parse::<i64>().unwrap() << 20).unwrap_or(8 << 30);
                let max_tiles: u32 = f("--tile-max").map(|v| v.parse().unwrap()).unwrap_or(4);
                let plan = lightmap::tiledpeel::plan(&recs, tiles_box, chunks_aabb.as_ref(), alloc_scale, tq, vram, max_tiles);
                eprintln!("peel cameras: {} item records, scene box [{:.1}, {:.1}]×[{:.1}, {:.1}]×[{:.1}, {:.1}], world peel box [{:.1}, {:.1}]×[{:.1}, {:.1}]×[{:.1}, {:.1}]; tiling at scale {alloc_scale:.3} layout units/m: ext {:.1} → target {}², n = {}, {} fitted tile(s){}", recs.len(), plan.scene.min()[0], plan.scene.max()[0], plan.scene.min()[1], plan.scene.max()[1], plan.scene.min()[2], plan.scene.max()[2], plan.world.min[0], plan.world.max[0], plan.world.min[1], plan.world.max[1], plan.world.min[2], plan.world.max[2], plan.ext, plan.size, plan.n, plan.tiles.len(), if plan.tiles.is_empty() { " (the world pass only)" } else { "" });
                for (i, t) in plan.tiles.iter().enumerate() { eprintln!("  tile {i}: [{:.1}, {:.1}]×[{:.1}, {:.1}]×[{:.1}, {:.1}]", t.min[0], t.max[0], t.min[1], t.max[1], t.min[2], t.max[2]); }
                if plan.size != prm.peel_res { eprintln!("peel cameras: peel resolution {} → {}", prm.peel_res, plan.size); prm.peel_res = plan.size; }
                prm.frustums = Some(std::sync::Arc::new(plan.table(&prm.sphere_dirs)));
                // the tiles' world-XZ clip for the accumulate (RE 7): the world peel unclipped, each tile its cell
                let mut clips: Vec<Option<[f32; 4]>> = vec![None];
                for t in &plan.tiles { clips.push(Some([t.min[0], t.min[2], t.max[0], t.max[2]])); }
                prm.peel_tile_clip = Some(std::sync::Arc::new(clips));
                Some(plan)
            } else { None };
            // (the sweep's direction list goes into the manifest before the sweep bakes, so a manifest written
            // at the end of the sweep already carries it)
            if let Some(d) = &prm.dump { let mut dm = d.lock().unwrap(); let n = prm.sphere_dirs.len() as u32; dm.manifest.sweeps.push(lightmap::passdump::Sweep { sweep: 0, n_dirs: n, scale: 4.0 / n.max(1) as f32, dirs: prm.sphere_dirs.iter().copied().collect() }); }
            // the template chunk (its constants), loaded before the bake: the transcribed probe passes need its trailer
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
            // the probe SLOT GRID of the map (shared by the port's Monte-Carlo probes and the transcribed probe passes)
            let build_slot_grid = |prm: &lightmap::bake::BakeParams| -> (lightmap::volume::Volume, lightmap::probes::SlotGrid) {
                let tv = lightmap::volume::Volume::parse(&tpl.chunk.data.as_ref().unwrap().cache.trailer).expect("template trailer");
                let _ = prm;
                // the slot grid: origin per decoration, counts from the map grid (size words) and the lit
                // geometry; --slots NX,NY,NZ / --slot-origin X,Y,Z override, --slots template copies the template's
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                let h = hdr.as_ref().expect("header");
                let grid_m = [mf.size[0] as f32 * 32.0, mf.size[1] as f32 * 8.0, mf.size[2] as f32 * 32.0];
                let (mut glo, mut ghi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for t in bvh.tris.iter().filter(|t| t.inst != lightmap::geometry::DECOR_INST) { for p in [t.p0, lightmap::geometry::add(t.p0, t.e1), lightmap::geometry::add(t.p0, t.e2)] { for k in 0..3 { glo[k] = glo[k].min(p[k]); ghi[k] = ghi[k].max(p[k]); } } }
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
                (tv, grid)
            };
            // THE PROBES IN THE BAKE (probebake.rs): on in the chain mode (--lm-from + --ilightinput-from e2e) or with
            // --probes transcribed; off with --probes port. The block/tile layout is the port's (probes::layout); the safety
            // offsets come from the capture when --probe-offsets-from FILE names a DDS volume, else none.
            // --probes transcribed|port; --probe-layout-from MAP takes the probe BLOCKS (cell ranges, pos, tile table, atlas size)
            // from a saved map's trailer (the editor's own bake of this map: the same-run comparison the chart --layout-from
            // gives), else the port's block layout (probes::layout); --probe-offsets-from DDS = the capture's safety offsets
            let probes_transcribed = match f("--probes").as_deref() { Some("transcribed") => true, Some("port") => false, Some(o) => panic!("--probes {o}: transcribed|port"), None => prm.lm_scene.is_some() && prm.game_peel };
            let mut probe_layout: Option<lightmap::probebake::ProbeLayoutSrc> = None;
            if probes_transcribed {
                // the layout: a saved map's trailer (--probe-layout-from MAP), else RE-6's transcribed chunking of the map's item
                // records (probechunk::for_records — the game's block/slot/pos structure; its cell ranges differ from the saves by a
                // margin cell: pwc-day min.x 23 vs 22, hill4 (21,3,19)–(28,9,27) vs (20,2,18)–(30,8,28) — eng 2 / RE-6's open item),
                // else the port's SlotGrid layout (--probe-layout port)
                let src: Result<lightmap::probebake::ProbeLayoutSrc, String> = match (f("--probe-layout-from"), f("--probe-layout").as_deref()) {
                    (Some(mp), _) => lightmap::mapio::load(&mp).and_then(|m| { let d = m.chunk.data.as_ref().ok_or("--probe-layout-from: no lightmap data")?; lightmap::volume::Volume::parse(&d.cache.trailer) }).map(|v| lightmap::probebake::ProbeLayoutSrc::from_volume(v)),
                    (None, Some("port")) => { let (tv, grid) = build_slot_grid(&prm); lightmap::probes::layout(&bvh, &grid).map(|lay| lightmap::probebake::ProbeLayoutSrc::from_layout(lay, tv, grid)) },
                    (None, _) => {
                        let tv = lightmap::volume::Volume::parse(&tpl.chunk.data.as_ref().unwrap().cache.trailer).expect("template trailer");
                        let gq: f32 = f("--global-quality").map(|v| v.parse().unwrap()).unwrap_or(1.0);
                        let recs: Vec<lightmap::lmtiles::BlockRecord> = lightmap::lmtiles::item_records(&scene, gq, has("--lod0")).iter().filter_map(|it| it.record).collect();
                        let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map_path));
                        let size = [mf.size[0].max(0) as u32, mf.size[1].max(0) as u32, mf.size[2].max(0) as u32];
                        let scene_ch = lightmap::lmtiles::scene_box(&recs);
                        let (_, _, chunking, _) = lightmap::probechunk::for_records(size, [32.0, 8.0, 32.0], [0.0, -38.0, 0.0], 0.0, &recs, &scene_ch, 2048);
                        match lightmap::probebake::layout_from_chunking(&chunking, &tv, [0.0, -38.0, 0.0]) {
                            Some(src) => Ok(src),
                            None => { let (tv, grid) = build_slot_grid(&prm); lightmap::probes::layout(&bvh, &grid).map(|lay| lightmap::probebake::ProbeLayoutSrc::from_layout(lay, tv, grid)) }
                        }
                    }
                };
                match src {
                    Ok(src) => {
                        let dims = src.dims;
                        let offsets = f("--probe-offsets-from").map(|p| { let b = lightmap::prepass::read_maybe_gz(std::path::Path::new(&p)).unwrap_or_else(|e| panic!("{e}")); lightmap::probepass::load_dds_volume(&b, None, dims[2]).expect("--probe-offsets-from") });
                        eprintln!("probes: TRANSCRIBED passes in the bake — volume {:?}, {} blocks ({}), atlas {}×{}, offsets {}, layout {}", dims, src.blocks.len(), src.blocks.iter().map(|b| format!("cells {:?}..{:?} pos {:?}", b.min, b.max, b.pos)).collect::<Vec<_>>().join("; "), src.atlas.0, src.atlas.1, if offsets.is_some() { "the capture's" } else { "none" }, if f("--probe-layout-from").is_some() { "the saved map's trailer" } else if f("--probe-layout").as_deref() == Some("port") { "the port's" } else { "RE-6's chunking" });
                        prm.probe_bake = Some(std::sync::Arc::new(std::sync::Mutex::new(lightmap::probebake::ProbeBake::new(dims, src.blocks.clone(), offsets))));
                        probe_layout = Some(src);
                    }
                    Err(e) => eprintln!("probes: transcribed passes skipped ({e})"),
                }
            }
            let write_field = |path: &str, charts: &[lightmap::bake::ChartBake]| {
                // the sweep's field as the next sweep reads it: per instance (w, h, the plain irradiance)
                let inst_of_item: std::collections::HashMap<usize, usize> = scene.instances.iter().enumerate().map(|(ii, inst)| (inst.item, ii)).collect();
                let mut slots: Vec<Option<(u32, u32, Vec<[f32; 3]>)>> = vec![None; scene.instances.len()];
                for c in charts { if let Some(&ii) = inst_of_item.get(&c.item) { slots[ii] = Some((c.w, c.h, if c.rgb_irr.is_empty() { c.rgb.clone() } else { c.rgb_irr.clone() })); } }
                let mut v: Vec<u8> = Vec::new();
                v.extend_from_slice(b"LMFIELD1");
                v.extend_from_slice(&(slots.len() as u32).to_le_bytes());
                for sl in &slots {
                    match sl {
                        None => v.extend_from_slice(&0u32.to_le_bytes()),
                        Some((w, h, rgb)) => { v.extend_from_slice(&1u32.to_le_bytes()); v.extend_from_slice(&w.to_le_bytes()); v.extend_from_slice(&h.to_le_bytes()); for t in rgb { for k in 0..3 { v.extend_from_slice(&t[k].to_bits().to_le_bytes()); } } }
                    }
                }
                // the probe accumulators after this sweep (they fold across sweeps: the next sweep's merge continues them)
                match &prm.probe_bake {
                    Some(pb) => {
                        let pb = pb.lock().unwrap();
                        v.extend_from_slice(&1u32.to_le_bytes());
                        for vol in [&pb.colour, &pb.updown, &pb.skyvis] {
                            for x in [vol.w, vol.h, vol.d, vol.channels] { v.extend_from_slice(&x.to_le_bytes()); }
                            for f in &vol.data { v.extend_from_slice(&f.to_bits().to_le_bytes()); }
                        }
                        v.extend_from_slice(&(pb.n_sky_adds as u64).to_le_bytes());
                    }
                    None => v.extend_from_slice(&0u32.to_le_bytes()),
                }
                std::fs::write(path, &v).expect("--field-out");
                eprintln!("field-out: {} ({} instances, {} B)", path, slots.len(), v.len());
            };
            let read_field = |path: &str| -> lightmap::bake::RadianceField {
                let b = std::fs::read(path).expect("--field-from");
                assert_eq!(&b[0..8], b"LMFIELD1", "--field-from: not a field file");
                let mut o = 8usize;
                let rd = |o: &mut usize| -> u32 { let v = u32::from_le_bytes(b[*o..*o + 4].try_into().unwrap()); *o += 4; v };
                let n = rd(&mut o) as usize;
                assert_eq!(n, scene.instances.len(), "--field-from: {n} instances, the scene has {}", scene.instances.len());
                let mut charts: Vec<Option<(u32, u32, Vec<[f32; 3]>)>> = Vec::with_capacity(n);
                for _ in 0..n {
                    if rd(&mut o) == 0 { charts.push(None); continue; }
                    let (w, h) = (rd(&mut o), rd(&mut o));
                    let mut rgb = Vec::with_capacity((w * h) as usize);
                    for _ in 0..w * h { rgb.push([f32::from_bits(rd(&mut o)), f32::from_bits(rd(&mut o)), f32::from_bits(rd(&mut o))]); }
                    charts.push(Some((w, h, rgb)));
                }
                // the probe accumulators of the previous sweep
                if o < b.len() && rd(&mut o) == 1 {
                    let mut vols: Vec<lightmap::probepass::Volume3> = Vec::new();
                    for _ in 0..3 {
                        let (w, h, d, c) = (rd(&mut o), rd(&mut o), rd(&mut o), rd(&mut o));
                        let n = (w * h * d * c) as usize;
                        let mut data = Vec::with_capacity(n);
                        for _ in 0..n { data.push(f32::from_bits(rd(&mut o))); }
                        vols.push(lightmap::probepass::Volume3 { w, h, d, channels: c, data });
                    }
                    let sky_adds = u64::from_le_bytes(b[o..o + 8].try_into().unwrap()) as usize;
                    if let Some(pb) = &prm.probe_bake {
                        let mut pb = pb.lock().unwrap();
                        assert_eq!((pb.colour.w, pb.colour.h, pb.colour.d), (vols[0].w, vols[0].h, vols[0].d), "--field-from: the probe grid differs");
                        pb.skyvis = vols.pop().unwrap();
                        pb.updown = vols.pop().unwrap();
                        pb.colour = vols.pop().unwrap();
                        pb.n_sky_adds = sky_adds;
                        eprintln!("field-from: the probe accumulators restored");
                    }
                }
                eprintln!("field-from: {path} ({n} instances)");
                lightmap::bake::RadianceField { charts, flip_v: prm.flip_v, uv_bounds: prm.uv_bounds }
            };
            let dir_range_for = |n: usize| -> Option<(usize, usize)> { dir_range_arg.as_ref().map(|a| lightmap::contrib::parse_range(a, n).unwrap_or_else(|e| panic!("{e}"))) };
            let run_sweep0 = sweep_only.map(|s| s == 0).unwrap_or(true);
            let mut charts = if run_sweep0 {
                prm.dir_range = dir_range_for(prm.sphere_dirs.len());
                if let Some(r) = prm.dir_range { eprintln!("dir-range: sweep 0: directions {}..{} of {}", r.0, r.1, prm.sphere_dirs.len()); }
                if prm.raster_peel { lightmap::peel::bake_peel_raster(&scene, &bvh, &prm, &chart_sizes(&prm)) } else { lightmap::bake::bake(&scene, &bvh, &prm, &lights) }
            } else { Vec::new() };
            if run_sweep0 { eprintln!("baked {} charts ({:.1}s)", charts.len(), t0.elapsed().as_secs_f32()); }
            if let (Some(fo), true) = (&field_out, sweep_only == Some(0)) { write_field(fo, &charts); }
            // --- THE CHAIN through the sweeps (--ilightinput-from): the sweep's transcribed H-basis MRTs (E's lm-from targets)
            //     are the next sweep's ILightInput through C's sweep-transition chain (sweep1::ilightinput_from_c0: PS 25113 resolve,
            //     PS 1038 × κ = 1/√(2π), the alpha mask, PS 1109 × MDiffuse, PS 1335 × 8) and, after the last sweep, the finalisation
            //     (finalprep.rs: PS 25113, PS 1109 × 2 onto the previous sweep's images, PS 1034, PS 1332 × 8, the encode)
            let mut hb_sweeps: Vec<lightmap::lmaccum::HbTargets> = Vec::new();
            let take_hb = |p: &lightmap::bake::BakeParams| -> Option<lightmap::lmaccum::HbTargets> { p.hb_out.as_ref().and_then(|s| s.0.lock().unwrap().take()) };
            if let Some(hb) = take_hb(&prm) { eprintln!("chain: sweep 0's H-basis MRTs taken ({}×{})", hb.w, hb.h); hb_sweeps.push(hb); }
            let mut chain_ambient_xyz: Option<[f32; 3]> = None;
            // the AddAmbient accumulator of sweep 0 (E's CS 17125 on OUR environment renders' centre pixels) against every banked
            // pwc2 snapshot: a snapshot at (frame, eid) holds the directions 0..=k, k = the direction whose H-basis draws come next
            // (E's ambient-check rule); ours after direction k is compared value for value
            if let Some(acc) = &prm.ambient_out {
                let ours = acc.lock().unwrap().clone();
                let last = ours.last().copied().unwrap_or([0.0; 4]);
                chain_ambient_xyz = Some([last[0], last[1], last[2]]);
                eprintln!("chain: AddAmbient accumulator after sweep 0 (ours, {} directions): [{:.6}, {:.6}, {:.6}, w {:.6}]", ours.len(), last[0], last[1], last[2], last[3]);
                if let Some(mp) = f("--frustum-from") {
                    if let Ok(entries) = lightmap::lmaccum::load_capture_entries(std::path::Path::new(&mp)) {
                        let root = std::path::PathBuf::from(f("--lm-from").unwrap());
                        let snaps: Vec<&lightmap::lmaccum::CapEntry> = entries.iter().filter(|e| e.pass == "ambient_accum" && e.capture == "pwc2").collect();
                        let mut dirs: Vec<(u32, u32, u64)> = entries.iter().filter(|e| e.pass == "hbasis0" && e.capture == "pwc2" && e.sweep_direction_index.is_some()).map(|e| (e.sweep_direction_index.unwrap(), e.frame, e.eid_last)).collect();
                        dirs.sort_by_key(|d| (d.1, d.2));
                        let (mut n, mut exact, mut ulp1, mut worst) = (0usize, 0usize, 0usize, 0f32);
                        let mut shown = 0;
                        for e in &snaps {
                            let Ok(b) = lightmap::passdiff::read_entry_bytes(&root, &e.file) else { continue };
                            if b.len() < 16 { continue; }
                            let g = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
                            let cap = [g(0), g(4), g(8), g(12)];
                            let Some(k) = dirs.iter().find(|d| (d.1, d.2) > (e.frame, e.eid_last)).map(|d| d.0) else { continue };
                            let Some(o) = ours.get(k as usize) else { continue };
                            for c in 0..4 {
                                n += 1;
                                let d = (o[c] - cap[c]).abs();
                                if d == 0.0 { exact += 1; } else if d <= 2.0 * f32::EPSILON * cap[c].abs().max(1e-6) { ulp1 += 1; }
                                let rel = d / cap[c].abs().max(1e-6);
                                if rel > worst { worst = rel; }
                            }
                            if shown < 4 { eprintln!("chain:   through direction {k}: ours [{:.6}, {:.6}, {:.6}, w {:.6}] captured [{:.6}, {:.6}, {:.6}, w {:.6}]", o[0], o[1], o[2], o[3], cap[0], cap[1], cap[2], cap[3]); shown += 1; }
                        }
                        eprintln!("chain: AddAmbient vs {} banked pwc2 snapshots: {exact} of {n} values bit-identical, {ulp1} within 2 f32 ulps, worst relative |Δ| {:.3e}", snaps.len(), worst);
                    }
                }
            }
            let mrt_buf = |hb: &lightmap::lmaccum::HbTargets, m: usize| -> lightmap::passdiff::Buf { let mut b = lightmap::passdiff::Buf::new(hb.w, hb.h, 4); for i in 0..(hb.w * hb.h) as usize { for c in 0..4 { b.data[i * 4 + c] = hb.mrt[m][i][c]; } } b };
            for it in 1..iterations {
                if let Some(s) = sweep_only { if it != s { continue; } }
                let field = match (&field_from, sweep_only) {
                    (Some(ff), Some(_)) => read_field(ff),
                    _ => {
                        let mut field = lightmap::bake::RadianceField { charts: vec![None; scene.instances.len()], flip_v: prm.flip_v, uv_bounds: prm.uv_bounds };
                        let inst_of_item: std::collections::HashMap<usize, usize> = scene.instances.iter().enumerate().map(|(ii, inst)| (inst.item, ii)).collect();
                        for c in &charts { if let Some(&ii) = inst_of_item.get(&c.item) { field.charts[ii] = Some((c.w, c.h, if c.rgb_irr.is_empty() { c.rgb.clone() } else { c.rgb_irr.clone() })); } }
                        field
                    }
                };
                let mut p2 = prm.clone();
                p2.field = Some(std::sync::Arc::new(field));
                p2.sweep = it as u32;
                if prm.peel {
                    if let Some(&n) = q_sweeps.get(it) {
                        let pp = f("--points").unwrap_or_else(lightmap::dome::default_path);
                        if let Ok(ps) = lightmap::dome::PointSets::load(&pp) {
                            if has("--table-order") { if let Some(set) = ps.nearest(n) { p2.sphere_dirs = std::sync::Arc::new(lightmap::dome::rotate_set(set)); } }
                            else if let Some(d) = lightmap::dome::sweep_directions(&ps, f("--quality").map(|s| s.parse().unwrap()).unwrap_or(3), it, false) { p2.sphere_dirs = std::sync::Arc::new(d); }
                        }
                    }
                }
                // THE GAME'S SWEEP ≥ 1 PEEL HAS NO ENVIRONMENT BLOCK (pwc6 frame 7534, sdi 8's range 4445–13359: per layer a
                // clear, the item draws, the accumulates — no sea box / terrain / dome / clouds): the layers are item layers
                // from the first, the environment neither drawn nor occluding; its colour is the sweep's ILightInput atlas
                // (--ilightinput-from-s1 FILE: the captured 8490 for the harness)
                if p2.game_peel {
                    p2.dome_layer = false;
                    p2.env_in_peel = false;
                    p2.ilight_atlas = None;
                    if let Some(path) = f("--ilightinput-from-s1") {
                        let pb = std::path::PathBuf::from(&path);
                        let (root, file) = (pb.parent().map(|p| p.to_path_buf()).unwrap_or_default(), pb.file_name().unwrap().to_string_lossy().to_string());
                        let mut e = lightmap::passdump::entry("ilightinput", file, "atlas");
                        e.format = "R11G11B10_FLOAT".into();
                        let b = lightmap::passdiff::load_entry(&root, &e).unwrap_or_else(|er| panic!("--ilightinput-from-s1 {path}: {er}"));
                        eprintln!("ilightinput-from-s1 {path}: {}×{} atlas — sweep {it}'s peel colour samples it at the LM uv", b.w, b.h);
                        p2.ilight_atlas = Some(std::sync::Arc::new(lightmap::peelcolor::AtlasTex::from_buf(&b)));
                    }
                }
                if let Some(plan) = &peel_plan {
                    p2.frustums = Some(std::sync::Arc::new(plan.table(&p2.sphere_dirs)));
                }
                if let Some(gm) = &game_manifest {
                    let fs = lightmap::passdiff::peel_frustums_for(gm, it as u32, &p2.sphere_dirs);
                    if !fs.is_empty() { eprintln!("frustum-from: sweep {it}: {} directions' peels ({} peels per direction; the capture's where it has them, else the transcribed light-camera fit on the map's boxes)", fs.len(), fs.iter().map(|v| v.len()).max().unwrap_or(0)); }
                    p2.frustums = if fs.is_empty() { None } else { Some(std::sync::Arc::new(fs)) };
                    let lc = lightmap::peelcap::captured_layer_counts(gm, it as u32, &p2.sphere_dirs);
                    let n_known = lc.iter().filter(|v| v.iter().any(|c| c.is_some())).count();
                    p2.peel_layer_counts = if n_known > 0 { Some(std::sync::Arc::new(lc.clone())) } else { None };
                    // which of our sweep-it directions carry a captured peel
                    let mut seen: Vec<([f32; 3], String, u32)> = Vec::new();
                    for e in gm.passes.iter().filter(|e| (e.pass == "peel_depth" || e.pass == "peel_color") && e.sweep == Some(it as u32)) {
                        if let Some(d) = e.dir { if !seen.iter().any(|(v, _, _)| (v[0] - d[0]).abs() < 1e-4 && (v[1] - d[1]).abs() < 1e-4 && (v[2] - d[2]).abs() < 1e-4) { seen.push((d, e.capture.clone().unwrap_or_default(), e.frame.unwrap_or(0))); } }
                    }
                    let hits: Vec<String> = seen.iter().filter_map(|(d, cap, fr)| {
                        let (i, c) = p2.sphere_dirs.iter().enumerate().map(|(i, o)| (i, o[0] * d[0] + o[1] * d[1] + o[2] * d[2])).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
                        Some(format!("{cap} frame {fr} ({:.3}, {:.3}, {:.3}) = sweep-{it} direction {i} ({:.3}°){}", d[0], d[1], d[2], c.clamp(-1.0, 1.0).acos().to_degrees(), if n_known > 0 { format!(", captured layer counts {:?}", lc.get(i)) } else { String::new() }))
                    }).collect();
                    if !hits.is_empty() { eprintln!("sweep {it}: captured peel directions: {}", hits.join("; ")); }
                    if f("--dump-dirs").as_deref() == Some("game") { if let Some(d) = &prm.dump { let v = lightmap::passdiff::game_dir_indices(gm, it as u32, &p2.sphere_dirs); eprintln!("dump-dirs game: sweep {it} → our directions {:?}", v); d.lock().unwrap().dirs = Some(v); } }
                }
                // the sweep-transition chain: OUR sweep-(it−1) C0 → the sweep-it ILightInput atlas (C's transcription), compared with the
                // capture's sweep-1 ILightInput when banked (pwc6's, another run of the same bake)
                // (the MDiffuse: the capture-driven chain's (e2e) or the from-map setup's — setupmap.rs)
                let mdiffuse8_src: Option<&lightmap::passdiff::Buf> = e2e_out.as_ref().map(|e| &e.mdiffuse8).or(from_map_setup.as_ref().map(|fm| &fm.mdiffuse8));
                if let (Some(hb), Some(md8), Some(il0)) = (hb_sweeps.last(), mdiffuse8_src, prm.ilatlas.as_ref()) {
                    let c0 = mrt_buf(hb, 0);
                    let mdl = lightmap::sweep1::mdiffuse_linear(md8, None);
                    let ts = std::time::Instant::now();
                    let atlas = lightmap::sweep1::ilightinput_from_c0(&c0, &mdl, None, 0.3989423);
                    if let Some(gm) = &game_manifest {
                        if let Some(e) = gm.passes.iter().filter(|e| e.pass == "ilightinput" && (e.sweep == Some(it as u32) || (it == 1 && e.frame == Some(7534)))).min_by_key(|e| e.eid.unwrap_or(u64::MAX)) {
                            match lightmap::passdiff::load_entry(std::path::Path::new(&f("--lm-from").unwrap()), e) {
                                Ok(cap) => { let r = lightmap::gpucmp::compare(&atlas, &cap, 3, lightmap::gpucmp::Fmt::R11G11B10); eprintln!("chain: the sweep-{it} ILightInput from OUR sweep-{} C0 vs the captured {} ({:?}): {}", it - 1, e.file, e.capture, r.line()); }
                                Err(err) => eprintln!("chain: the captured sweep-{it} ILightInput {}: {err}", e.file),
                            }
                        } else { eprintln!("chain: no captured sweep-{it} ILightInput entry in the manifest"); }
                    }
                    if let Some(dir) = f("--chain-final-dir") {
                        std::fs::create_dir_all(&dir).expect("--chain-final-dir");
                        let mut r11 = Vec::with_capacity(2048 * 2048 * 4);
                        for i in 0..(2048 * 2048) as usize { r11.extend_from_slice(&lightmap::gpufmt::pack_r11g11b10([atlas.data[i * 3], atlas.data[i * 3 + 1], atlas.data[i * 3 + 2]], lightmap::gpufmt::Rounding::Truncate).to_le_bytes()); }
                        std::fs::write(format!("{dir}/chain-sweep{it}-ilightinput.r11g11b10"), r11).expect("write");
                    }
                    // the LM instance stream: the from-map scene's when the setup came from the map, else the capture's
                    let il = match (from_map_setup.as_ref(), prm.lm_scene.as_ref()) {
                        (Some(_), Some(lm)) => lightmap::ilatlas::IlAtlas::from_lm_scene(atlas, lm),
                        _ => {
                            let n_items = scene.item_count.max(1);
                            let lm_root = std::path::PathBuf::from(f("--lm-from").unwrap());
                            let env_frame: u32 = f("--lm-env-frame").map(|v| v.parse().expect("--lm-env-frame")).unwrap_or(127448);
                            let mesh_dir = lm_root.join(format!("env/frame{env_frame}/mesh"));
                            let insts = lightmap::prepass::read_maybe_gz(&mesh_dir.join("vb_17033.bin")).unwrap_or_else(|e| panic!("{e}"));
                            let tile_vb = lightmap::prepass::read_maybe_gz(&mesh_dir.join("vb_5350.bin")).unwrap_or_else(|e| panic!("{e}"));
                            lightmap::ilatlas::IlAtlas::new(atlas, &insts, &tile_vb, n_items)
                        }
                    };
                    let item_map = il0.item_map.clone();
                    p2.ilatlas = Some(std::sync::Arc::new(lightmap::ilatlas::IlSource { atlas: il, item_map }));
                    p2.hb_out = Some(std::sync::Arc::new(lightmap::ilatlas::HbSlot(std::sync::Mutex::new(None))));
                    eprintln!("chain: sweep {it} peels sample OUR sweep-{} ILightInput ({:.1}s)", it - 1, ts.elapsed().as_secs_f32());
                }
                if let Some(d) = &prm.dump { let mut dm = d.lock().unwrap(); let n = p2.sphere_dirs.len() as u32; dm.manifest.sweeps.push(lightmap::passdump::Sweep { sweep: it as u32, n_dirs: n, scale: 4.0 / n.max(1) as f32, dirs: p2.sphere_dirs.iter().copied().collect() }); }
                p2.dir_range = dir_range_for(p2.sphere_dirs.len());
                if let Some(r) = p2.dir_range { eprintln!("dir-range: sweep {it}: directions {}..{} of {}", r.0, r.1, p2.sphere_dirs.len()); }
                charts = if prm.raster_peel { lightmap::peel::bake_peel_raster(&scene, &bvh, &p2, &chart_sizes(&p2)) } else { lightmap::bake::bake(&scene, &bvh, &p2, &lights) };
                if let (Some(fo), Some(s)) = (&field_out, sweep_only) { if s == it { write_field(fo, &charts); } }
                let mean: f32 = charts.iter().flat_map(|c| c.rgb.iter()).map(|c| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).sum::<f32>() / charts.iter().map(|c| c.rgb.len()).sum::<usize>().max(1) as f32;
                eprintln!("bounce iteration {it}: mean texel {mean:.4} ({:.1}s)", t0.elapsed().as_secs_f32());
                if let Some(hb) = take_hb(&p2) { eprintln!("chain: sweep {it}'s H-basis MRTs taken"); hb_sweeps.push(hb); }
            }
            // --- the finalisation of the chain (ROW 12 + the baker's dilation and encode) on OUR sweeps' MRTs, against the captured
            //     finalisation chain (pwc4 frame 74490, another run of the same bake) — `--chain-final-dir DIR` writes the images
            // the finalised (×2-added) coefficient images of the transcribed chain, kept for the transcribed FILE writer below
            let mut chain_finals: Option<Vec<lightmap::passdiff::Buf>> = None;
            // (the finalisation runs whenever a sweep's H-basis MRTs exist — from the captured chain (--lm-from + --ilightinput-from e2e)
            // or from the map alone (--lm-from-map); the comparison with the captured finals needs the capture root)
            if hb_sweeps.len() >= 1 && (e2e_out.is_some() || has("--lm-from-map")) {
                let tf = std::time::Instant::now();
                let n_sw = hb_sweeps.len();
                // per coefficient image: Σ_sweeps 2 · resolve(MRT) — the game adds each sweep's resolved image × 2 into the previous
                // sweep's finalised targets (f16: source truncated, sum RTNE)
                // (`finalprep::finalise_sweeps` — the library form of this step)
                let finals: Vec<lightmap::passdiff::Buf> = lightmap::finalprep::finalise_sweeps(&hb_sweeps).into_iter().collect();
                if let (Some(gm), Some(lm_root)) = (&game_manifest, f("--lm-from").or_else(|| f("--lm-cap-root"))) {
                    let root = std::path::PathBuf::from(lm_root);
                    let mut ents: Vec<&lightmap::passdump::Entry> = gm.passes.iter().filter(|e| e.pass == "final_02_scaled_x2_ps1109").collect();
                    ents.sort_by_key(|e| e.eid_last.unwrap_or(0));
                    for (m, e) in ents.iter().enumerate().take(4) {
                        if let Ok(cap) = lightmap::passdiff::load_entry(&root, e) {
                            let r = lightmap::gpucmp::compare(&finals[m], &cap, 4, lightmap::gpucmp::Fmt::F16);
                            let (mut n, mut within) = (0usize, 0usize);
                            for i in 0..(2048 * 2048) as usize { for c in 0..3 { let g = cap.data[i * 4 + c]; let o = finals[m].data[i * 4 + c]; if g != 0.0 || o != 0.0 { n += 1; if (o - g).abs() <= 0.02 * g.abs().max(1e-6) { within += 1; } } } }
                            eprintln!("chain: finalised image {m} ({n_sw} sweeps × 2 · PS 25113) vs the captured {} ({:?}): {} — rgb within 2 %: {within}/{n} ({:.2} %)", e.file, e.capture, r.line(), 100.0 * within as f64 / n.max(1) as f64);
                        }
                    }
                }
                if let Some(dir) = f("--chain-final-dir") {
                    std::fs::create_dir_all(&dir).expect("--chain-final-dir");
                    for (m, b) in finals.iter().enumerate() {
                        let mut bytes = Vec::new();
                        for i in 0..(b.w * b.h) as usize { for c in 0..4 { bytes.extend_from_slice(&lightmap::gpufmt::encode_f16(b.data[i * 4 + c], lightmap::gpufmt::Rounding::NearestEven).to_le_bytes()); } }
                        std::fs::write(format!("{dir}/chain-final-{m}.rgba16f"), bytes).expect("write");
                    }
                    eprintln!("chain: wrote the finalised images under {dir}");
                }
                eprintln!("chain: finalisation of {n_sw} sweep(s) in {:.1}s", tf.elapsed().as_secs_f32());
                chain_finals = Some(finals);
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
            // the zone tiles: flat charts (the tiles' own lighting is not baked here) — sized by the game's layout when --layout-game
            let tile_size_of: std::collections::HashMap<u32, (u32, u32)> = game_layout.as_ref().map(|gl| gl.charts.iter().filter(|c| c.obj < base).map(|c| (c.obj, ((c.w as u32 / 2).max(1), (c.h as u32 / 2).max(1)))).collect()).unwrap_or_default();
            for obj in deco_const..base { let (tw, th) = tile_size_of.get(&obj).copied().unwrap_or((2, 2)); out_charts.push(lightmap::synth::Chart::from_hdr(obj, tw, th, &vec![ground_e; (tw * th) as usize], k, 128)); }
            let unbound: std::collections::HashSet<usize> = game_layout.as_ref().map(|gl| gl.charts.iter().filter(|c| c.obj >= base && c.charted != lightmap::layout::Charted::Bound).map(|c| (c.obj - base) as usize).collect()).unwrap_or_default();
            let mut have = vec![false; scene.item_count];
            if candidates.is_empty() {
                for c in &charts { have[c.item] = true; if unbound.contains(&c.item) { continue; } out_charts.push(lightmap::synth::Chart::from_hdr2(base + c.item as u32, c.w, c.h, &c.rgb, &c.rgb1, k, 128)); }
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
            for (i, h) in have.iter().enumerate() { if !h && !unbound.contains(&i) { out_charts.push(lightmap::synth::Chart::from_hdr(base + i as u32, 2, 2, &[ground_e; 4], k, 128)); } }
            let tm = tpl.chunk.data.as_ref().unwrap().cache.mapping().unwrap();
            // the probe volume: ours unless --template-probes
            let vp8_q: Option<u8> = f("--vp8").map(|s| s.parse().unwrap());
            // THE PROBES from the bake's ProbeBake (probebake.rs): downloaded → the four atlases → the WEBPs (the blob) + the trailer
            // with the download's scales / validity — for BOTH writers (the port's chunk writer takes the blob as well): the no-box
            // path's probes come from the transcribed passes over the port's own peels, the port's Monte-Carlo probes are the fallback
            // when the passes did not run. (The record's LAmbient = the AddAmbient accumulator is engineer A's BakeParams::ambient_out →
            // transcribed_images' ambient_xyz; None here until it lands.)
            let ambient_xyz: Option<[f32; 3]> = None;
            let transcribed_probes: Option<lightmap::synth::ProbeBlob> = match (&prm.probe_bake, &probe_layout) {
                (Some(pb), Some(src)) => {
                    let tp = std::time::Instant::now();
                    let pbl = pb.lock().unwrap();
                    match pbl.finish(&src.tiles, src.atlas) {
                        Some(r) => {
                            let aw = src.atlas.0;
                            let vol = src.volume(r.scales, r.ends, &|x, y| !r.valid[(y * aw + x) as usize]);
                            if let Some(dir) = f("--chain-final-dir") { let _ = pbl.dump(std::path::Path::new(&dir)); }
                            eprintln!("probes: TRANSCRIBED — {} directions, {} world layers, {} probe writes, {} sky-visibility adds; {} of {} probes valid; scales max0 {} max2 {}; blob {} B (parts {:?}) ({:.1}s)", pbl.n_dirs, pbl.n_layers, pbl.n_written, pbl.n_sky_adds, r.n_valid, r.n_probes, r.scales[0], r.scales[1], r.blob.len(), r.ends, tp.elapsed().as_secs_f32());
                            if let Some(dir) = f("--chain-final-dir") { for (k, im) in r.images.iter().enumerate() { let _ = std::fs::write(format!("{dir}/probe-image{k}.rgb"), im); } }
                            Some(lightmap::synth::ProbeBlob { blob: r.blob, trailer: vol.write() })
                        }
                        None => { eprintln!("probes: the transcribed probe WEBPs need libwebp; the port's probes are used"); None }
                    }
                }
                _ => None,
            };
            let probes = if let Some(tp) = transcribed_probes { Some(tp) } else if has("--template-probes") { None } else {
                let (tv, grid) = build_slot_grid(&prm);
                let mut pp = prm.clone();
                pp.sky_samples = f("--probe-samples").map(|s| s.parse().unwrap()).unwrap_or(48);
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
            // --bake-time now|TICKS: the file's FILETIME word; default = the template's (a deterministic writer)
            let bake_filetime: Option<u64> = match f("--bake-time").as_deref() { Some("now") => Some(lightmap::synth::filetime_now()), Some(t) => Some(t.parse().expect("--bake-time now|TICKS")), None => None };
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
                lightmap::synth::FrameParams { daytime, max_hdr_mood: x.max_hdr, max_hdr: k, bounce: x.bounce_factor, sky: x.sky_factor, sum_area: Some(items_area + 2.0 * n_tiles), quality: Some(quality), filetime: bake_filetime, decoration: Some(mf0.decoration_id.clone()) }
            });
            // the game's positions when --layout-game: stored texel (px, py) = ((X + 1)/2, (Y + 1)/2) of the layout rect, for every chart
            // (the tiles included — their objects are the 4096 first)
            let fixed_pos: Option<std::collections::HashMap<(u32, u32), (u32, u32)>> = game_layout.as_ref().map(|gl| gl.charts.iter().filter(|c| c.charted == lightmap::layout::Charted::Bound).map(|c| ((c.obj, 0u32), (((c.x + 1) / 2) as u32, ((c.y + 1) / 2) as u32))).collect());
            // THE TRANSCRIBED FILE WRITER (--writer transcribed|port; default: transcribed whenever the transcribed chain produced
            // its finalised images, i.e. with --lm-from): the four finalised coefficient images → PS 1034 → PS 1332 × 8 → the max
            // reduce → CS 23025 → the client's CPU steps (filecheck::frame0_blobs: YCbCr_to_RGB_Down2x2, per-chart fb0, the greys,
            // libwebp at the game's settings) → the record's scale fields → synth::build_transcribed. The port's own encoder writes
            // only the fallback. The probe blob is still the port's (or the template's) until the transcribed probe passes run
            // over our peels in the bake; the LAmbient triple stays the template's — both flagged in the log.
            let writer_transcribed = match f("--writer").as_deref() { Some("port") => false, Some("transcribed") => true, Some(o) => panic!("--writer {o}: port|transcribed"), None => chain_finals.is_some() };
            let mood_max_hdr_for_encode: f32 = frame_params.as_ref().map(|fp| fp.max_hdr_mood).unwrap_or(7.519885063171387);
            // LMTOOL_PROBE_DUMP_DIR=DIR: the transcribed probe accumulators as raw volumes (probebake::dump) — the
            // sparse/dense and split/single checks compare them byte for byte (engineer 2)
            if let (Some(pb), Ok(dir)) = (&prm.probe_bake, std::env::var("LMTOOL_PROBE_DUMP_DIR")) {
                let d = std::path::PathBuf::from(&dir);
                std::fs::create_dir_all(&d).expect("probe dump dir");
                pb.lock().unwrap().dump(&d).expect("probe dump");
                eprintln!("probes: accumulators dumped to {dir}");
            }
            let probes_for_transcribed = if writer_transcribed { probes.clone() } else { None };
            let frame_params_for_transcribed = if writer_transcribed { frame_params.clone() } else { None };
            let s = lightmap::synth::build_full2_placed(out_charts, (tm.bbox_min, tm.bbox_max), &tpl.chunk, probes, vp8_q, frame_params, fixed_pos.as_ref()).expect("build");
            let s = match (writer_transcribed, &chain_finals) {
                (true, Some(finals)) => {
                    let tw = std::time::Instant::now();
                    let (_imgs, maxhdr, enc) = lightmap::e2e::finalise_tail(finals, mood_max_hdr_for_encode);
                    let mut s = s;
                    let atlas8 = s.atlas8.take();
                    // the layout rects (2048 layout units) in the mapping's order = the placed charts (obj, sub) ascending
                    let rects: Vec<(u32, u32, u32, u32)> = s.placed.iter().map(|&(_o, _s, px, py, w, h)| ((2 * px).saturating_sub(1), (2 * py).saturating_sub(1), 2 * w, 2 * h)).collect();
                    match lightmap::e2e::transcribed_images(&enc, maxhdr, mood_max_hdr_for_encode, &rects, chain_ambient_xyz) {
                        Some(img) => match lightmap::synth::build_transcribed(&s.placed, (tm.bbox_min, tm.bbox_max), &tpl.chunk, &img, probes_for_transcribed, frame_params_for_transcribed) {
                            Ok(st) => {
                                eprintln!("writer: TRANSCRIBED — MaxHdr {maxhdr:?} (Mood {mood_max_hdr_for_encode}), record MaxHDR {} / √3κ·max {:?}; blob0 {} B, blob1 {} B, {} charts; probes {} ; LAmbient {} ({:.1}s)", img.max_hdr, img.hbasis234, img.blob0.len(), img.blob1.len(), st.charts, if prm.probe_bake.is_some() && probe_layout.is_some() { "the transcribed passes' blob" } else if st.chunk.data.as_ref().map(|d| !d.frames[0].images[2].is_empty()).unwrap_or(false) { "the port's/template's blob" } else { "none" }, match img.lambient_f16 { Some(l) => format!("= f16(AddAmbient) {l:?}"), None => "= the template's".into() }, tw.elapsed().as_secs_f32());
                                lightmap::synth::Synth { atlas8, ..st }
                            }
                            Err(e) => { eprintln!("writer: transcribed chunk failed ({e}); the port's writer is used"); s }
                        },
                        None => { eprintln!("writer: the transcribed frame-0 blobs need libwebp (webpenc); the port's writer is used"); s }
                    }
                }
                (true, None) => { eprintln!("writer: --writer transcribed needs the transcribed chain's images (--lm-from PASSCAP with --ilightinput-from e2e); the port's writer is used"); s }
                _ => s,
            };
            if let Some(d) = &prm.dump {
                // the final atlas before encode: the 8-bit colour image, the HDR C0 composed on the same layout, and the layout itself
                let mut dm = d.lock().unwrap();
                let item_of_obj: std::collections::HashMap<u32, usize> = charts.iter().map(|c| (base + c.item as u32, c.item)).collect();
                for &(obj, sub, x, y, w, h) in &s.placed {
                    let item = item_of_obj.get(&obj).copied().unwrap_or(usize::MAX);
                    dm.manifest.layout.push(lightmap::passdump::ChartRect { obj, item: item as u32, sub, x: 2 * x as i32 - 1, y: 2 * y as i32 - 1, w: 2 * w as i32, h: 2 * h as i32, chart_w: w, chart_h: h });
                }
                if let Some(ia) = &s.atlas8 {
                    let mut e = lightmap::passdump::entry("final_atlas", "final_atlas/color8.bin".into(), "atlas");
                    e.format = "R8G8B8_UNORM".into();
                    e.notes = Some("the sqrt-encoded, per-chart-normalised 8-bit colour image handed to the WEBP encoder (frame 0 image 0); the chart bytes are in `layout`".into());
                    dm.write_u8(e, ia.w, ia.h, 3, &ia.px).expect("dump final_atlas");
                    let mut hdr = vec![[0.0f32; 3]; (ia.w * ia.h) as usize];
                    for &(obj, _sub, x, y, w, h) in &s.placed {
                        if let Some(c) = item_of_obj.get(&obj).and_then(|it| charts.iter().find(|c| c.item == *it)) {
                            if c.w == w && c.h == h {
                                for yy in 0..h { for xx in 0..w { let (ax, ay) = (x + xx, y + yy); if ax < ia.w && ay < ia.h { hdr[(ay * ia.w + ax) as usize] = c.rgb[(yy * w + xx) as usize]; } } }
                            }
                        }
                    }
                    let mut e = lightmap::passdump::entry("final_hdr", "final_hdr/c0.bin".into(), "atlas");
                    e.notes = Some(format!("the HDR irradiance (C0) per stored texel on the final layout, before the frame normalisation (MaxHDR {k:.4}) and the sqrt encode"));
                    dm.write_rgb(e, ia.w, ia.h, &hdr, lightmap::gpufmt::Quant::None, prm.rounding).expect("dump final_hdr");
                }
                dm.finish().expect("write MANIFEST.json");
                eprintln!("dump-passes: {} entries, {:.1} MB under {}", dm.manifest.passes.len(), dm.bytes_written as f64 / 1e6, dm.root.display());
            }
            let payload = s.chunk.write(false);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&m, &payload, &out).expect("save");
            println!("{} charts, atlas fill {:.1}%, chunk {} B; wrote {out} ({:.1}s)", s.charts, s.fill * 100.0, payload.len(), t0.elapsed().as_secs_f32());
            // the process's peak resident set (Linux: VmHWM of /proc/self/status) — `lmtool bench` reads it
            if let Ok(st) = std::fs::read_to_string("/proc/self/status") {
                if let Some(l) = st.lines().find(|l| l.starts_with("VmHWM:")) {
                    let kb: u64 = l.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                    eprintln!("peak RSS {:.2} GB", kb as f64 / 1_048_576.0);
                }
            }
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
            let sub = lightmap::geometry::Scene { models: scene.models.clone(), model_names: scene.model_names.clone(), instances: vec![scene.instances[ii].clone()], item_count: scene.item_count, decor: scene.decor.clone(), alpha_masks: scene.alpha_masks.clone(), card_albedo: scene.card_albedo.clone(), tex_albedo: scene.tex_albedo.clone() };
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
                    let sub = lightmap::geometry::Scene { models: scene.models.clone(), model_names: scene.model_names.clone(), instances: vec![inst.clone()], item_count: scene.item_count, decor: scene.decor.clone(), alpha_masks: scene.alpha_masks.clone(), card_albedo: scene.card_albedo.clone(), tex_albedo: scene.tex_albedo.clone() };
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
        "daytime-set" => {
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
            let offs = lightmap::mapio::daytime_word_offsets(body);
            if let Some(w) = lightmap::mapio::daytime(body) { println!("0x0304306B (the word the editor uses): {w:#x}; {} DayTime words in the body", offs.len()); }
            if let (Some(out), Some(v)) = (f("--out"), f("--set")) {
                let val: u32 = if v == "default" { 0xffff_ffff } else if let Some(h) = v.strip_prefix("0x") { u32::from_str_radix(h, 16).unwrap() } else { v.parse().unwrap() };
                let mut nb = body.clone();
                for o in &offs { nb[*o..*o + 4].copy_from_slice(&val.to_le_bytes()); }
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
                        // the cache chunk 0x06022015 (5, joint id, 3, 0x1c, Id decoration, 1, 0, DayTime) carries the
                        // time too — and THAT is the word the editor adopts when it opens the map (every DayTime
                        // variant of 2026-09-23 baked at the chunk's old time until this was patched)
                        for c in dd.cache.chunks.iter_mut() {
                            if c.id == 0x0602_2015 {
                                if let lightmap::format::ChunkBody::Raw(b) = &mut c.body {
                                    if b.len() >= 40 && b[20..24] == [0, 0, 0, 0x40] {
                                        let name_len = u32::from_le_bytes([b[24], b[25], b[26], b[27]]) as usize;
                                        let o = 28 + name_len + 8;
                                        if o + 4 <= b.len() { b[o..o + 4].copy_from_slice(&rec_val.to_le_bytes()); }
                                    }
                                }
                            }
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
        // lmtool peel-layers PASSCAP_ROOT [--game-manifest FILE]: the captured peel layers' pixel counts and the
        // game's layer-count rule against them (peelcap.rs)
        "peel-layers" => lightmap::peelcap::run(&a),
        "quanta-diff" => lightmap::peelcap::quanta_diff(&a),
        "layer-gap" => lightmap::peelcap::layer_gap(&a),
        "lm-st" => lightmap::peelcap::lm_st(&a),
        "sweep1-annotate" => { if let Err(e) = lightmap::peelcap::sweep1_annotate(&a) { eprintln!("sweep1-annotate: {e}"); std::process::exit(1); } }
        "clouds-check" => { if let Err(e) = lightmap::clouds::check(&a) { eprintln!("clouds-check: {e}"); std::process::exit(1); } }
        "passcap-info" => {
            // lmtool passcap-info DIR [--pass P] [--max N]: per entry of a MANIFEST.json the buffer's statistics
            // (min / max / mean per channel, the fraction of clear pixels) and the derived frustum — a look at a
            // capture (or a dump) before comparing it
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let m = lightmap::passdiff::read_manifest(&std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json")).unwrap_or_else(|e| panic!("{e}"));
            let max: usize = f("--max").map(|s| s.parse().unwrap()).unwrap_or(400);
            println!("{}: map {} baked {:?} quality {} mood {} sweeps {:?} layout {} passes {}", a[1], m.map, m.baked_map, m.quality, m.mood, m.sweeps.iter().map(|s| (s.sweep, s.dirs.len())).collect::<Vec<_>>(), m.layout.len(), m.passes.len());
            for (k, v) in &m.conventions { println!("  convention {k}: {v}"); }
            let mut n = 0;
            for e in &m.passes {
                if let Some(p) = f("--pass") { if e.pass != p { continue; } }
                if n >= max { break; }
                n += 1;
                let head = format!("{} s{:?} d{:?} p{:?} l{:?} eid{:?} {} {}×{} {}", e.pass, e.sweep, e.direction, e.peel, e.layer, e.eid_last, e.format, e.width, e.height, e.file);
                match lightmap::passdiff::load_entry(&root, e) {
                    Ok(b) => {
                        let ch = b.channels as usize;
                        let mut lo = vec![f32::MAX; ch]; let mut hi = vec![f32::MIN; ch]; let mut sum = vec![0f64; ch];
                        let mut zero = 0usize;
                        let npx = (b.w * b.h) as usize;
                        for i in 0..npx {
                            let mut allz = true;
                            for c in 0..ch { let v = b.data[i * ch + c]; if v.is_finite() { lo[c] = lo[c].min(v); hi[c] = hi[c].max(v); sum[c] += v as f64; } if v != 0.0 { allz = false; } }
                            if allz { zero += 1; }
                        }
                        let mean: Vec<String> = (0..ch).map(|c| format!("{:.4}", sum[c] / npx as f64)).collect();
                        let los: Vec<String> = lo.iter().map(|v| format!("{v:.4}")).collect();
                        let his: Vec<String> = hi.iter().map(|v| format!("{v:.4}")).collect();
                        println!("{head}: min [{}] max [{}] mean [{}] clear {:.1} %", los.join(","), his.join(","), mean.join(","), 100.0 * zero as f64 / npx as f64);
                        // a few samples: the centre and the corners
                        let sample = |x: u32, y: u32| -> String { (0..ch).map(|c| format!("{:.4}", b.get(x.min(b.w - 1), y.min(b.h - 1), c as u32))).collect::<Vec<_>>().join(",") };
                        println!("    centre ({}) corners ({}) ({}) ({}) ({})", sample(b.w / 2, b.h / 2), sample(0, 0), sample(b.w - 1, 0), sample(0, b.h - 1), sample(b.w - 1, b.h - 1));
                        // the distribution of a depth buffer: the histogram over 8 bins, and — with a frustum — the
                        // world height of the surface at a few pixels (a peel of the ground gives the tile's y)
                        if ch == 1 {
                            let mut bins = [0usize; 8];
                            for i in 0..npx { let v = b.data[i]; if v > 0.0 && v.is_finite() { bins[((v.clamp(0.0, 0.99999) * 8.0) as usize).min(7)] += 1; } }
                            println!("    depth bins (z01 0..1 in 8): {:?}", bins);
                            if let Some(fr) = &e.frustum {
                                let mut ys: Vec<f32> = Vec::new();
                                for (x, y) in [(b.w / 2, b.h / 2), (b.w / 4, b.h / 4), (3 * b.w / 4, b.h / 4), (b.w / 4, 3 * b.h / 4), (3 * b.w / 4, 3 * b.h / 4), (b.w / 2, b.h / 4), (b.w / 2, 3 * b.h / 4)] {
                                    let z = b.get(x, y, 0);
                                    if z > 0.0 && z < 1.0 { let p = fr.unproject(x as f32 + 0.5, y as f32 + 0.5, z, b.w, b.h); ys.push(p[1]); }
                                }
                                // the most common height among the whole buffer's surfaces (1 cm bins, every 64th pixel)
                                let mut hist: std::collections::HashMap<i32, usize> = Default::default();
                                for i in (0..npx).step_by(64) { let z = b.data[i]; if z > 0.0 && z < 1.0 { let (x, y) = ((i % b.w as usize) as f32 + 0.5, (i / b.w as usize) as f32 + 0.5); let p = fr.unproject(x, y, z, b.w, b.h); *hist.entry((p[1] * 100.0).round() as i32).or_insert(0) += 1; } }
                                let mut top: Vec<(i32, usize)> = hist.into_iter().collect();
                                top.sort_by(|a, b| b.1.cmp(&a.1));
                                println!("    surface heights y at centre/quarter pixels: {:?}; most common y (1 cm bins): {:?}", ys.iter().map(|v| format!("{v:.2}")).collect::<Vec<_>>(), top.iter().take(4).map(|(k, n)| format!("{:.2} m ×{n}", *k as f32 / 100.0)).collect::<Vec<_>>());
                            }
                        }
                    }
                    Err(err) => println!("{head}: {err}"),
                }
                if let Some(fr) = &e.frustum {
                    println!("    frustum: centre ({:.2},{:.2},{:.2}) half ({:.2},{:.2},{:.2}) right ({:.3},{:.3},{:.3}) up ({:.3},{:.3},{:.3}) forward ({:.3},{:.3},{:.3}){}", fr.center[0], fr.center[1], fr.center[2], fr.half[0], fr.half[1], fr.half[2], fr.right[0], fr.right[1], fr.right[2], fr.up[0], fr.up[1], fr.up[2], fr.forward[0], fr.forward[1], fr.forward[2], e.viewport.as_ref().map(|v| format!(" viewport {:?}", v)).unwrap_or_default());
                }
                if let (Some(d), Some(fr)) = (e.dir, &e.frustum) {
                    let c = d[0] * fr.forward[0] + d[1] * fr.forward[1] + d[2] * fr.forward[2];
                    if (c - 1.0).abs() > 1e-3 { println!("    NOTE: dir vs frustum forward: cos {c:.5} ({:.2}°)", c.clamp(-1.0, 1.0).acos().to_degrees()); }
                }
            }
        }
        "sun-check" => {
            // lmtool sun-check PASSCAP_ROOT [--frame N] [--blend round-sum|round-src] [--dump OUT.dds] [--shadow OURS.dds]
            //   the transcribed DIRECT SUN pass (sunpass.rs: VS 15183 + PS 15187, D3D11 rasterisation, 2×2 PCF GreaterEqual)
            //   run from the capture's own inputs (the LM meshes + instance stream + chart table in env/frame<N>/, the 36
            //   draws' cbuffers in logs/draws-frame<N>.json.gz, the captured D16 shadow map) and compared f16 for f16
            //   with the captured sun_direct target
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127448);
            // default = what the capture shows: the shader output truncated to f16, the blend sum rounded to nearest
            let blend = match f("--blend").as_deref() { Some("round-src") => lightmap::sunpass::BlendModel::RoundSrcAndSum, Some("trunc") => lightmap::sunpass::BlendModel::TruncSum, Some("trunc-src") => lightmap::sunpass::BlendModel::TruncSrcAndSum, Some("round") => lightmap::sunpass::BlendModel::RoundSum, _ => lightmap::sunpass::BlendModel::TruncSrcRoundSum };
            let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
            let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
            let env = root.join(format!("env/frame{frame}"));
            let mesh_json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(env.join("mesh.json")).expect("mesh.json")).expect("mesh.json");
            let draws_bytes = lightmap::passdiff::read_entry_bytes(&root, &format!("logs/draws-frame{frame}.json")).expect("draws log");
            let t0 = std::time::Instant::now();
            let draws: serde_json::Value = serde_json::from_slice(&draws_bytes).expect("draws json");
            println!("draws log parsed in {:.1} s", t0.elapsed().as_secs_f32());
            let sun: Vec<&serde_json::Value> = draws.as_array().unwrap().iter().filter(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some("15187")).collect();
            println!("{} sun draws (PS 15187)", sun.len());
            // the four objects of the first block, in issue order, give the meshes; later blocks repeat the order
            let first_block: Vec<u64> = sun.iter().take(4).map(|e| e["eid"].as_u64().unwrap()).collect();
            let mut meshes = Vec::new();
            let mut inst_first = Vec::new();
            let mut instance_bytes: Option<Vec<u8>> = None;
            for eid in &first_block {
                let rec = mesh_json.as_array().unwrap().iter().find(|r| r["eid"].as_u64() == Some(*eid)).unwrap_or_else(|| panic!("mesh.json has no eid {eid}"));
                let vbs = rec["vertex_buffers"].as_array().unwrap();
                let vb0 = std::fs::read(env.join("mesh").join(vbs[0]["file"].as_str().unwrap())).expect("vb0");
                let idx_file = rec["vsout"]["index_file"].as_str().expect("index file");
                let ib = std::fs::read(env.join("mesh").join(idx_file)).expect("indices");
                let indices: Vec<u16> = ib.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                let verts = lightmap::sunpass::parse_lm_vertices(&vb0);
                println!("  eid {eid}: {} vertices, {} indices, instance stream offset {}", verts.len(), indices.len(), vbs[1]["offset"]);
                meshes.push(lightmap::sunpass::LmMesh { verts, indices });
                inst_first.push((vbs[1]["offset"].as_u64().unwrap_or(0) / 48) as usize);
                if instance_bytes.is_none() { instance_bytes = Some(std::fs::read(env.join("mesh").join(vbs[1]["file"].as_str().unwrap())).expect("instance vb")); }
            }
            let instances = lightmap::sunpass::parse_instances(instance_bytes.as_ref().unwrap());
            println!("  {} instances", instances.len());
            let table_bytes = std::fs::read(env.join("bufs").join(format!("e{:06}_Vertex_srv0_16959.bin", first_block[0]))).unwrap_or_default();
            let table: Vec<[f32; 4]> = table_bytes.chunks_exact(16).map(|c| [f32::from_le_bytes(c[0..4].try_into().unwrap()), f32::from_le_bytes(c[4..8].try_into().unwrap()), f32::from_le_bytes(c[8..12].try_into().unwrap()), f32::from_le_bytes(c[12..16].try_into().unwrap())]).collect();
            let m4 = |v: &serde_json::Value| -> [[f32; 4]; 4] { let mut o = [[0f32; 4]; 4]; for i in 0..4 { for j in 0..4 { o[i][j] = v[i][j].as_f64().unwrap() as f32; } } o };
            let v3 = |v: &serde_json::Value| -> [f32; 3] { [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32, v[2].as_f64().unwrap() as f32] };
            let v2 = |v: &serde_json::Value| -> [f32; 2] { [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32] };
            let mut sd = Vec::new();
            for (k, e) in sun.iter().enumerate() {
                let ps = &e["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"];
                let vs = &e["Vertex"]["cbuffers"]["ShaderV"]["g_CBuffer"];
                let inst = e["inst"].as_u64().unwrap_or(0).max(1) as usize;
                sd.push(lightmap::sunpass::SunDraw { eid: e["eid"].as_u64().unwrap(), mesh: k % 4, instance_first: inst_first[k % 4], instance_count: inst, scale_ss: v2(&vs["LM01_Scale_RasterSS"]), trans_ss: v2(&vs["LM01_Trans_RasterSS"]), world_pw01_shadow: m4(&ps["WorldPw01Shadow"]), dir_in_world: v3(&ps["DirInWorld"]), light_rgb: v3(&ps["LightRgb"]), out_scale: ps["OutScale"].as_f64().unwrap() as f32 });
            }
            let ent = |pass: &str| m.passes.iter().find(|e| e.pass == pass && e.frame == Some(frame)).unwrap_or_else(|| panic!("no {pass} entry for frame {frame}"));
            // --shadow FILE.dds: OUR shadow map (lmtool shadow-check --dump) in place of the captured one — the row 2 → row 3 chain
            let shadow = match f("--shadow") { Some(p) => { let b = std::fs::read(&p).expect("--shadow file"); let sm = lightmap::passdiff::load_dds_bytes(&b, "R16_UNORM", 0, 0).expect("--shadow dds"); println!("shadow map from {p}"); sm } None => lightmap::passdiff::load_entry(&root, ent("sun_shadow")).expect("shadow map") };
            let target = lightmap::passdiff::load_entry(&root, ent("sun_direct")).expect("sun_direct");
            println!("shadow map {}×{} (D16 as UNORM16), target {}×{}×{}", shadow.w, shadow.h, target.w, target.h, target.channels);
            let sm = lightmap::sunpass::ShadowMap { depth: &shadow };
            let t1 = std::time::Instant::now();
            let ours = lightmap::sunpass::run_sun_pass(&meshes, &instances, &table, &sd, &sm, target.w, target.h, blend);
            println!("rasterised {} draws in {:.1} s", sd.len(), t1.elapsed().as_secs_f32());
            let (covered, exact, ulp1, worse, maxd) = lightmap::sunpass::compare_f16(&ours, &target);
            println!("{blend:?}: texels touched (either side) {covered}; channel values: {exact} exact, {ulp1} within 1 f16 ulp, {worse} worse (max |Δ| {maxd:.5})");
            // coverage agreement: alpha = the summed OutScale (1 where all 9 jitters hit)
            let mut cov_ours = 0usize; let mut cov_theirs = 0usize; let mut cov_both = 0usize;
            for i in 0..ours.px.len() { let (x, y) = ((i as u32) % ours.w, (i as u32) / ours.w); let o = ours.px[i][3] > 0.0; let t = target.get(x, y, 3) > 0.0; if o { cov_ours += 1; } if t { cov_theirs += 1; } if o && t { cov_both += 1; } }
            println!("coverage (alpha > 0): ours {cov_ours}, game {cov_theirs}, both {cov_both}");
            // value statistics over the covered texels + a few samples
            let mut so = [0f64; 4]; let mut st = [0f64; 4]; let mut n = 0usize; let mut shown = 0;
            for i in 0..ours.px.len() { let (x, y) = ((i as u32) % ours.w, (i as u32) / ours.w); if target.get(x, y, 3) > 0.0 { n += 1; for k in 0..4 { so[k] += ours.px[i][k] as f64; st[k] += target.get(x, y, k as u32) as f64; } if shown < 8 && (ours.px[i][0] - target.get(x, y, 0)).abs() > 0.01 && x % 37 == 0 { println!("  sample ({x},{y}): ours {:?} game {:?}", ours.px[i], [target.get(x, y, 0), target.get(x, y, 1), target.get(x, y, 2), target.get(x, y, 3)]); shown += 1; } } }
            println!("mean over the game's covered texels: ours {:?} game {:?}", so.iter().map(|v| (v / n as f64) as f32).collect::<Vec<_>>(), st.iter().map(|v| (v / n as f64) as f32).collect::<Vec<_>>());
            // where are the mismatches? by the game's alpha (1 = one chart, 2 = two charts overlap, else partial coverage)
            let mut by = std::collections::BTreeMap::<String, (usize, usize, f32)>::new();
            for i in 0..ours.px.len() { let (x, y) = ((i as u32) % ours.w, (i as u32) / ours.w); let ta = target.get(x, y, 3); if ta == 0.0 && ours.px[i][3] == 0.0 { continue; } let key = if (ta - 1.0).abs() < 1e-3 { "alpha=1".to_string() } else if (ta - 2.0).abs() < 1e-3 { "alpha=2".to_string() } else { "partial".to_string() }; let e = by.entry(key).or_insert((0, 0, 0.0)); e.0 += 1; let d = (0..3).map(|k| (ours.px[i][k] - target.get(x, y, k as u32)).abs()).fold(0f32, f32::max); if d > 0.0 { e.1 += 1; if d > e.2 { e.2 = d; } } }
            for (k, (n, bad, mx)) in by { println!("  {k}: {n} texels, {bad} with an rgb difference (max |Δ| {mx:.4})"); }
            // partial texels: split the differences into "alpha differs" (coverage: a jitter edge decided differently) and
            // "alpha equal, rgb differs" (the shading of the covered fraction)
            let (mut a_diff, mut a_same_rgb_diff, mut shown) = (0usize, 0usize, 0usize);
            for i in 0..ours.px.len() { let (x, y) = ((i as u32) % ours.w, (i as u32) / ours.w); let ta = target.get(x, y, 3); let oa = ours.px[i][3]; if (ta - 1.0).abs() < 1e-3 || (ta == 0.0 && oa == 0.0) { continue; } let rgbd = (0..3).map(|k| (ours.px[i][k] - target.get(x, y, k as u32)).abs()).fold(0f32, f32::max); if oa != ta { a_diff += 1; if shown < 6 { println!("  alpha differs at ({x},{y}): ours {:?} game {:?}", ours.px[i], [target.get(x, y, 0), target.get(x, y, 1), target.get(x, y, 2), ta]); shown += 1; } } else if rgbd > 0.0 { a_same_rgb_diff += 1; if shown < 12 { println!("  same alpha, rgb differs at ({x},{y}): ours {:?} game {:?}", ours.px[i], [target.get(x, y, 0), target.get(x, y, 1), target.get(x, y, 2), ta]); shown += 1; } } }
            println!("  partial texels: alpha differs {a_diff}, alpha equal but rgb differs {a_same_rgb_diff}");
            if let Some(out) = f("--dump") {
                let mut bytes = Vec::with_capacity(ours.px.len() * 8);
                for p in &ours.px { for k in 0..4 { bytes.extend_from_slice(&lightmap::gpufmt::encode_f16(p[k], lightmap::gpufmt::Rounding::NearestEven).to_le_bytes()); } }
                std::fs::write(&out, &bytes).expect("dump");
                println!("wrote {out} (raw RGBA16F {}×{})", ours.w, ours.h);
            }
        }
        "probe-check" => {
            // lmtool probe-check PASSCAP_ROOT [--frame N] [--fma] [--no-ref-clamp] [--rtne] [--pcf-bits B] [-v]
            //   ROW 10: the transcribed PROBE passes (probepass.rs: PS 17151 ProbeGrid_SetILightDir per world-peel layer,
            //   PS 17154 AddSkyVisibility, PS 1112 folds) run from the capture's own inputs (the layers' colour + depth,
            //   the TMapProbeSafetyOffset volume, the draws' cbuffers + GS slice range + scissor) and compared f16 for f16
            //   with the captured 32×16×32 volumes after every draw
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127448);
            let mut opts = lightmap::probepass::ProbeOpts::default();
            opts.fma = has("--fma");
            opts.clamp_ref = !has("--no-ref-clamp");
            if has("--rtne") { opts.store = lightmap::gpufmt::Rounding::NearestEven; }
            if let Some(b) = f("--pcf-bits") { opts.pcf_frac_bits = b.parse().expect("--pcf-bits"); }
            let co = lightmap::probecheck::CheckOpts { frame, opts, verbose: has("-v") };
            if let Err(e) = lightmap::probecheck::run(&root, &co) { eprintln!("probe-check: {e}"); std::process::exit(1); }
        }
        "lmscene" => {
            // lmtool lmscene PASSCAP_ROOT [--env-frame 127448]: the capture's LM meshes as the accumulate draws see them —
            // per object the vertex/index counts, the PSIZE tangent-frame modes, the instance quaternion, the vertex
            // normals against the geometric ones (are the cards' normals the faces'?), duplicated (two-sided) triangles
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let env_frame: u32 = f("--env-frame").map(|v| v.parse().expect("--env-frame")).unwrap_or(127448);
            let sc = lightmap::lmaccum::load_lm_scene(&root, env_frame).expect("LM scene");
            println!("{} instances, {} chart ST entries", sc.instances.len(), sc.table.len());
            for (mi, m) in sc.meshes.iter().enumerate() {
                let inst = &sc.instances[sc.inst_first[mi]];
                let rows = lightmap::sunpass::rotation_rows(inst.q);
                let mut modes = std::collections::BTreeMap::<i32, usize>::new();
                for v in &m.verts { *modes.entry(v.psize.round() as i32).or_insert(0) += 1; }
                let mut tris: std::collections::HashMap<[u16; 3], usize> = std::collections::HashMap::new();
                let (mut rev, mut n_off, mut worst) = (0usize, 0usize, 0f32);
                for t in m.indices.chunks_exact(3) {
                    let mut k = [t[0], t[1], t[2]];
                    k.sort();
                    *tris.entry(k).or_insert(0) += 1;
                    // the geometric normal vs the mean vertex normal
                    let (p0, p1, p2) = (m.verts[t[0] as usize].pos, m.verts[t[1] as usize].pos, m.verts[t[2] as usize].pos);
                    let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
                    let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
                    let g = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                    let gl = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt().max(1e-12);
                    let mut nm = [0f32; 3];
                    for &i in t { for k in 0..3 { nm[k] += m.verts[i as usize].normal[k] / 3.0; } }
                    let nl = (nm[0] * nm[0] + nm[1] * nm[1] + nm[2] * nm[2]).sqrt().max(1e-12);
                    let c = (g[0] * nm[0] + g[1] * nm[1] + g[2] * nm[2]) / (gl * nl);
                    if c < 0.0 { rev += 1; }
                    let ang = c.abs().min(1.0).acos().to_degrees();
                    if ang > 5.0 { n_off += 1; }
                    if ang > worst { worst = ang; }
                }
                let dup = tris.values().filter(|&&c| c > 1).count();
                let (u0, u1) = m.verts.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), v| ([lo[0].min(v.uv[0]), lo[1].min(v.uv[1])], [hi[0].max(v.uv[0]), hi[1].max(v.uv[1])]));
                let ci: std::collections::BTreeSet<u32> = m.verts.iter().map(|v| v.chart_idx).collect();
                println!("mesh {mi} (eid {}): {} vertices, {} triangles, {} instance(s) from {}; PSIZE modes {:?}; quaternion {:?} (rows {:?}); chart_idx {:?} st.x bits {:#x} st {:?}; uv [{:.4},{:.4}]..[{:.4},{:.4}]", sc.eids[mi], m.verts.len(), m.indices.len() / 3, sc.inst_count[mi], sc.inst_first[mi], modes, inst.q, rows, ci.iter().take(6).collect::<Vec<_>>(), inst.st_x_bits, inst.st, u0[0], u0[1], u1[0], u1[1]);
                println!("    vertex normal vs face normal: {rev} triangles reversed (n·g < 0), {n_off} more than 5° off (worst {worst:.1}°); {dup} triangle sets drawn more than once (same three vertices)");
                if mi == 2 || m.indices.len() / 3 > 100 {
                    // a few sample vertices of the big mesh
                    for v in m.verts.iter().step_by((m.verts.len() / 6).max(1)).take(6) {
                        println!("    v pos {:?} n {:?} t {:?} psize {} uv {:?} chart {}", v.pos, v.normal, v.tangent, v.psize, v.uv, v.chart_idx);
                    }
                }
            }
            // --tiles-map BAKED.Map.Gbx: every tile instance of the capture's instance stream → its chart rect in the editor's
            // table (by the instance ST: the rect's x = T·2048 − 1/8, D's `lmtool lm-st` rule) → the object id; then the
            // instance's world translation → the map cell: does obj = cell_z·64 + cell_x hold, and what order are the
            // instances in?
            if let Some(bp) = f("--tiles-map") {
                let own = lightmap::mapio::load(&bp).expect("--tiles-map");
                let d = own.chunk.data.as_ref().unwrap();
                let mp = d.cache.mapping().unwrap();
                let mut by_pos: std::collections::HashMap<(i32, i32), u32> = Default::default();
                for i in 0..mp.count as usize { by_pos.insert((mp.pos[i].0 as i32, mp.pos[i].1 as i32), mp.binds[i].obj_group_idx / 4); }
                let (mut n, mut ok_zx, mut ok_xz) = (0usize, 0usize, 0usize);
                let mut rows: Vec<(usize, u32, i32, i32, [f32; 3])> = Vec::new();
                for mi in 0..sc.meshes.len() {
                    if sc.inst_count[mi] < 2 { continue; }
                    for ii in sc.inst_first[mi]..sc.inst_first[mi] + sc.inst_count[mi] {
                        let inst = &sc.instances[ii];
                        // (the ST here is the stored instance float4 (scale.xy, trans.zw); the rect's x ≈ trans·2048 − 1/8)
                        let px = ((inst.st[2] * 2048.0) - 0.125).round() as i32;
                        let py = ((inst.st[3] * 2048.0) - 0.125).round() as i32;
                        let mut found = None;
                        'o: for dx in -2..=2 { for dy in -2..=2 { if let Some(o) = by_pos.get(&(px + dx, py + dy)) { found = Some(*o); break 'o; } } }
                        let Some(o) = found else { if n < 5 { println!("  instance {ii}: ST {:?} → ({px},{py}) no rect", inst.st); } n += 1; continue };
                        let (cx, cz) = ((inst.t[0] / 32.0).floor() as i32, (inst.t[2] / 32.0).floor() as i32);
                        n += 1;
                        if o as i32 == cz * 64 + cx { ok_zx += 1; }
                        if o as i32 == cx * 64 + cz { ok_xz += 1; }
                        rows.push((ii, o, cx, cz, inst.t));
                    }
                }
                println!("{n} tile instances: obj = cz·64 + cx for {ok_zx}, obj = cx·64 + cz for {ok_xz}");
                for r in rows.iter().take(12) { println!("  instance {:4}: obj {:4} cell x{:>2} z{:>2} t {:?}", r.0, r.1, r.2, r.3, r.4); }
                let from: usize = f("--from").map(|s| s.parse().unwrap()).unwrap_or(usize::MAX);
                for r in rows.iter().filter(|r| r.1 as usize >= from).take(40) { println!("  obj {:4} cell x{:>2} z{:>2} y {}", r.1, r.2, r.3, r.4[1]); }
                // the instance-stream order vs the object id: is the stream in object order?
                let mono = rows.windows(2).filter(|w| w[1].1 > w[0].1).count();
                println!("  instance stream: {} of {} consecutive pairs ascend in obj id", mono, rows.len().saturating_sub(1));
                return;
            }
            // the tiles: the ST of the first few tile instances
            for i in 3..sc.instances.len().min(6) { let t = &sc.instances[i]; println!("tile instance {i}: q {:?} t {:?} scale {} st {:?}", t.q, t.t, t.scale, t.st); }
            // the items' world-space AABB (the fitted blocks' WorldBoxMinXZ / MaxXZ candidates)
            {
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for mi in 0..sc.meshes.len() {
                    if sc.inst_count[mi] > 1 { continue; }
                    let inst = &sc.instances[sc.inst_first[mi]];
                    let rows = lightmap::sunpass::rotation_rows(inst.q);
                    for v in &sc.meshes[mi].verts { let p = lightmap::lmaccum::world_pos(v, inst, &rows); for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
                }
                println!("items' world AABB: x {:.4}..{:.4}  y {:.4}..{:.4}  z {:.4}..{:.4} (the captured fitted-block box: x 861.01..880.0, z 336.98..369.0)", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
                for mi in 0..sc.meshes.len() { if sc.inst_count[mi] > 1 { continue; } let inst = &sc.instances[sc.inst_first[mi]]; let rows = lightmap::sunpass::rotation_rows(inst.q); let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]); for v in &sc.meshes[mi].verts { let p = lightmap::lmaccum::world_pos(v, inst, &rows); for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } } println!("  mesh {mi}: translation {:?} scale {}  AABB x {:.4}..{:.4} y {:.4}..{:.4} z {:.4}..{:.4}", inst.t, inst.scale, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]); }
            }
            // --dump-mesh M: every vertex (model space) and triangle of LM mesh M — the oracle for a mesh built from the map
            if let Some(mi) = f("--dump-mesh").map(|v| v.parse::<usize>().unwrap()) {
                let m = &sc.meshes[mi];
                println!("mesh {mi}: {} vertices, {} triangles", m.verts.len(), m.indices.len() / 3);
                for (i, v) in m.verts.iter().enumerate() { println!("  v{i:<4} pos [{:.6}, {:.6}, {:.6}] n [{:.6}, {:.6}, {:.6}] t [{:.6}, {:.6}, {:.6}, {}] psize {} uv [{:.7}, {:.7}] chart {}", v.pos[0], v.pos[1], v.pos[2], v.normal[0], v.normal[1], v.normal[2], v.tangent[0], v.tangent[1], v.tangent[2], v.tangent[3], v.psize, v.uv[0], v.uv[1], v.chart_idx); }
                for (t, tri) in m.indices.chunks_exact(3).enumerate() { println!("  tri{t:<3} [{}, {}, {}]", tri[0], tri[1], tri[2]); }
            }
            // --mesh M --tri T [--dir x,y,z]: one triangle's vertices and their VS 17118 outputs
            if let (Some(mi), Some(ti)) = (f("--mesh"), f("--tri")) {
                let (mi, ti): (usize, usize) = (mi.parse().unwrap(), ti.parse().unwrap());
                let m = &sc.meshes[mi];
                let inst = &sc.instances[sc.inst_first[mi]];
                let d: [f32; 3] = f("--dir").map(|s| { let v: Vec<f32> = s.split(',').map(|t| t.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] }).unwrap_or([0.3454768657684326, 0.11707823723554611, 0.9310952425003052]);
                let cb = lightmap::lmaccum::LmRasterCb::for_offset(0, 2048, 2048);
                for &vi in &m.indices[ti * 3..ti * 3 + 3] {
                    let v = &m.verts[vi as usize];
                    let o = lightmap::lmaccum::vs_17118(v, inst, &sc.table, &cb, d);
                    let nl = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
                    let tl = (v.tangent[0].powi(2) + v.tangent[1].powi(2) + v.tangent[2].powi(2)).sqrt();
                    let ndt = v.normal[0] * v.tangent[0] + v.normal[1] * v.tangent[1] + v.normal[2] * v.tangent[2];
                    println!("  vertex {vi}: pos {:?} n {:?} (|n| {nl:.5}) t {:?} (|t| {tl:.5}, n·t {ndt:.5}) psize {} uv {:?} → clip {:?} o2 {:?} (|o2| {:.5}) o3 {:?}", v.pos, v.normal, v.tangent, v.psize, v.uv, o.clip, o.o2, (o.o2[0].powi(2) + o.o2[1].powi(2) + o.o2[2].powi(2)).sqrt(), o.o3);
                }
            }
        }
        "ilightdir-check" => {
            // lmtool ilightdir-check PASSCAP_ROOT [--frame 127448] [--env-frame 127448] [--cmp unorm|float] [--only-chained]
            //   the transcribed LmILightDir_Set block (lmaccum.rs: VS 17111 + PS 17112) run over the CAPTURED peel colour +
            //   depth of every layer snapshot of the frame, compared with the CAPTURED TMapILightDir after that block —
            //   chained (our target carried from block to block, as the game's) and per block (each block started from
            //   the game's own previous snapshot, isolating the block's error)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            if let Some(t) = f("--snap-tie") { lightmap::sunpass::RASTER_SNAP_TIE.store(t.parse().unwrap(), std::sync::atomic::Ordering::Relaxed); }
            let has = |k: &str| a.iter().any(|x| x == k);
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127448);
            let env_frame: u32 = f("--env-frame").map(|v| v.parse().expect("--env-frame")).unwrap_or(127448);
            let cmp = match f("--cmp").as_deref() { Some("float") => lightmap::lmaccum::DepthCompare::Float, _ => lightmap::lmaccum::DepthCompare::Unorm16Round };
            let manifest = f("--manifest").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"));
            let t0 = std::time::Instant::now();
            let sc = lightmap::lmaccum::load_lm_scene(&root, env_frame).expect("LM scene");
            let entries = lightmap::lmaccum::load_capture_entries(&manifest).expect("manifest");
            let draws = lightmap::lmaccum::load_draws(&root, frame).expect("draws log");
            let blocks = lightmap::lmaccum::set_blocks(&draws, &sc).expect("accumulate blocks");
            println!("frame {frame}: {} accumulate blocks in the log ({} draws), scene loaded in {:.1} s", blocks.len(), blocks.iter().map(|b| b.draws.len()).sum::<usize>(), t0.elapsed().as_secs_f32());
            let ild: Vec<&lightmap::lmaccum::CapEntry> = entries.iter().filter(|e| e.pass == "ilightdir" && e.frame == frame).collect();
            let layers: Vec<&lightmap::lmaccum::CapEntry> = entries.iter().filter(|e| (e.pass == "peel_color" || e.pass == "peel_depth") && e.frame == frame).collect();
            println!("  manifest: {} ilightdir snapshots, {} layer buffers for this frame", ild.len(), layers.len());
            let mut chained = lightmap::lmaccum::DirTarget::cleared(2048, 2048);
            let mut prev_game: Option<lightmap::passdiff::Buf> = None;
            let mut totals = (0usize, 0usize, 0usize, 0usize);
            for (bi, b) in blocks.iter().enumerate() {
                let Some(snap) = ild.iter().find(|e| e.eid_first <= b.eid_first && b.eid_last <= e.eid_last || e.eid_last == b.eid_last).copied() else {
                    println!("block {bi:2} eids {}-{}: no captured ilightdir snapshot", b.eid_first, b.eid_last);
                    continue;
                };
                let color = layers.iter().filter(|e| e.pass == "peel_color" && e.eid_last < b.eid_first).max_by_key(|e| e.eid_last).copied();
                let depth = layers.iter().filter(|e| e.pass == "peel_depth" && e.eid_last < b.eid_first).max_by_key(|e| e.eid_last).copied();
                let (Some(ce), Some(de)) = (color, depth) else {
                    println!("block {bi:2} eids {}-{}: no captured layer colour/depth before it", b.eid_first, b.eid_last);
                    continue;
                };
                let (cb, db) = (ce.load(&root).expect("layer colour"), de.load(&root).expect("layer depth"));
                let game = snap.load(&root).expect("ilightdir snapshot");
                let layer = lightmap::lmaccum::LayerTargets { color: &cb, depth: &db };
                let probe: Option<(u32, u32)> = f("--probe").map(|s| { let v: Vec<u32> = s.split(',').map(|t| t.trim().parse().expect("--probe x,y")).collect(); (v[0], v[1]) });
                let only_block: Option<usize> = f("--block").map(|s| s.parse().expect("--block N"));
                if let Some(ob) = only_block { if ob != bi { prev_game = Some(game); continue; } }
                if probe.is_some() || has("--borders") {
                    // the layer targets' border (the 1-px frame the viewport (1,1,4094,4094) never draws) and centre
                    let (w, h) = (db.w, db.h);
                    let px = |x: u32, y: u32| format!("d {:.5} (q {}) c ({:.4},{:.4},{:.4})", db.get(x, y, 0), (db.get(x, y, 0) * 65535.0).round(), cb.get(x, y, 0), cb.get(x, y, 1), cb.get(x, y, 2));
                    println!("    layer targets {w}×{h}: (0,0) {} | (1,1) {} | (w-1,0) {} | (w-2,1) {} | (0,h-1) {} | (w-1,h-1) {} | centre {}", px(0, 0), px(1, 1), px(w - 1, 0), px(w - 2, 1), px(0, h - 1), px(w - 1, h - 1), px(w / 2, h / 2));
                    if let Some((x, y)) = probe { println!("    game ilightdir at ({x},{y}) after this block: ({:.4},{:.4},{:.4}); before: {:?}", game.get(x, y, 0), game.get(x, y, 1), game.get(x, y, 2), prev_game.as_ref().map(|p| [p.get(x, y, 0), p.get(x, y, 1), p.get(x, y, 2)])); }
                }
                let d0 = b.draws[0].cb.peel_dir;
                // the peel this block belongs to: the projection's scale tells the world (≈5e-4) from the fitted (≈1e-2) frustum
                let sx = b.draws[0].cb.world_pw01_shadow[0][0].abs().max(b.draws[0].cb.world_pw01_shadow[2][0].abs());
                let t1 = std::time::Instant::now();
                // per block: from the game's previous snapshot
                let mut fresh = lightmap::lmaccum::DirTarget::cleared(2048, 2048);
                if let Some(pg) = &prev_game { for y in 0..2048u32 { for x in 0..2048u32 { fresh.px[(y * 2048 + x) as usize] = lightmap::gpufmt::pack_r11g11b10([pg.get(x, y, 0), pg.get(x, y, 1), pg.get(x, y, 2)], lightmap::gpufmt::Rounding::Truncate); } } }
                let mut trace: lightmap::lmaccum::SetTrace = if has("--classify") { vec![None; 2048 * 2048] } else { Vec::new() };
                lightmap::lmaccum::run_set_block_trace(&sc.meshes, &sc.instances, &sc.table, &b.draws, &layer, cmp, &mut fresh, probe, if has("--classify") { Some(&mut trace) } else { None });
                if has("--classify") {
                    // the mismatching pixels by the fragment's z range and uv range: which rule the game applies
                    let mut cls = std::collections::BTreeMap::<String, (usize, usize)>::new();
                    for y in 0..2048u32 { for x in 0..2048u32 {
                        let i = (y * 2048 + x) as usize;
                        let Some((u, v, z, stored, ndd)) = trace[i] else { continue };
                        let o = fresh.rgb(x, y); let g = [game.get(x, y, 0), game.get(x, y, 1), game.get(x, y, 2)];
                        let same = lightmap::gpufmt::pack_r11g11b10(g, lightmap::gpufmt::Rounding::Truncate) == fresh.px[i];
                        let zr = if z < 0.0 { "z<0" } else if z > 1.0 { "z>1" } else { "0≤z≤1" };
                        let ur = if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 { "uv-out" } else { "uv-in" };
                        let sr = if stored == 0.0 { "st=0" } else if stored >= 1.0 { "st=1" } else { "0<st<1" };
                        let cr = if z >= stored { "ge" } else { "lt" };
                        let nr = if ndd < 0.0 { "back" } else { "front" };
                        let gw = if prev_game.as_ref().map(|p| [p.get(x, y, 0), p.get(x, y, 1), p.get(x, y, 2)] != g).unwrap_or(g != [0.0; 3]) { "game-wrote" } else { "game-kept" };
                        let ow = if o != [0.0; 3] || fresh.px[i] != prev_game.as_ref().map(|p| lightmap::gpufmt::pack_r11g11b10([p.get(x, y, 0), p.get(x, y, 1), p.get(x, y, 2)], lightmap::gpufmt::Rounding::Truncate)).unwrap_or(0) { "ours-wrote" } else { "ours-kept" };
                        let e = cls.entry(format!("{zr:7} {ur:6} {sr:7} {cr} {nr:5} {gw} {ow}")).or_insert((0, 0));
                        e.0 += 1; if !same { e.1 += 1; }
                        if !same && e.1 <= 2 { println!("      example {zr} {ur} {sr} {cr}: ({x},{y}) uv ({u:.4},{v:.4}) z {z:.5} stored {stored:.5} ours ({:.4},{:.4},{:.4}) game ({:.4},{:.4},{:.4}) before {:?}", o[0], o[1], o[2], g[0], g[1], g[2], prev_game.as_ref().map(|p| [p.get(x, y, 0), p.get(x, y, 1), p.get(x, y, 2)])); }
                    } }
                    for (k, (n, bad)) in &cls { println!("      {k}: {n:>8} pixels, {bad:>8} mismatching"); }
                }
                let cf = lightmap::lmaccum::compare_dir(&fresh, &game);
                if !has("--only-chained") {
                    println!("block {bi:2} eids {:5}-{:5} {} layer {:?} ({}) D ({:.3},{:.3},{:.3}) proj-scale {sx:.1e}  per-block : {}", b.eid_first, b.eid_last, ce.phase.clone().unwrap_or_default(), ce.layer, if sx < 2e-3 { "world" } else { "fitted" }, d0[0], d0[1], d0[2], lightmap::lmaccum::fmt_dircmp(&cf));
                }
                lightmap::lmaccum::run_set_block(&sc.meshes, &sc.instances, &sc.table, &b.draws, &layer, cmp, &mut chained);
                let cc = lightmap::lmaccum::compare_dir(&chained, &game);
                println!("block {bi:2} eids {:5}-{:5} {:56} chained   : {}  ({:.1} s)", b.eid_first, b.eid_last, "", lightmap::lmaccum::fmt_dircmp(&cc), t1.elapsed().as_secs_f32());
                totals.0 += cf.touched; totals.1 += cf.exact; totals.2 += cf.quantum; totals.3 += cf.worse;
                // the worst per-block texels, a few, with what each side holds
                if cf.worse > 0 && !has("--quiet") {
                    let mut shown = 0;
                    'outer: for y in (0..2048u32).step_by(3) { for x in (0..2048u32).step_by(3) {
                        let o = fresh.rgb(x, y); let g = [game.get(x, y, 0), game.get(x, y, 1), game.get(x, y, 2)];
                        let d = (0..3).map(|k| (o[k] - g[k]).abs()).fold(0f32, f32::max);
                        if d > 0.01 { println!("      ({x},{y}) ours ({:.4},{:.4},{:.4}) game ({:.4},{:.4},{:.4})", o[0], o[1], o[2], g[0], g[1], g[2]); shown += 1; if shown >= 5 { break 'outer; } }
                    } }
                }
                prev_game = Some(game);
            }
            let pct = |n: usize| if totals.0 > 0 { 100.0 * n as f64 / totals.0 as f64 } else { 0.0 };
            println!("ALL BLOCKS per-block: touched {} exact {} ({:.3} %) ±1 quantum {} ({:.3} %) worse {} ({:.3} %)  [{:?}, {:.1} s]", totals.0, totals.1, pct(totals.1), totals.2, pct(totals.2), totals.3, pct(totals.3), cmp, t0.elapsed().as_secs_f32());
        }
        "hbasis-check" => {
            // lmtool hbasis-check PASSCAP_ROOT [--frame 127448] [--index K] [--env-frame 127448] [--blend trunc-src-round|round|trunc|round-src|trunc-src] [--dump-prefix P]
            //   the transcribed H-basis accumulate (lmaccum.rs: VS 17118 + PS 17122) run over the CAPTURED TMapILightDir after the
            //   direction's last accumulate, added onto the CAPTURED MRTs after the previous direction, compared f16 for f16 with the
            //   CAPTURED four MRTs after the direction (hbasis0..3)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let root = std::path::PathBuf::from(&a[1]);
            let env_frame: u32 = f("--env-frame").map(|v| v.parse().expect("--env-frame")).unwrap_or(127448);
            let manifest = f("--manifest").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"));
            let blend = match f("--blend").as_deref() { Some("round-src") => lightmap::sunpass::BlendModel::RoundSrcAndSum, Some("trunc") => lightmap::sunpass::BlendModel::TruncSum, Some("trunc-src") => lightmap::sunpass::BlendModel::TruncSrcAndSum, Some("round") => lightmap::sunpass::BlendModel::RoundSum, _ => lightmap::sunpass::BlendModel::TruncSrcRoundSum };
            if has("--coalesced") { lightmap::lmaccum::HB_FRAG_MODEL.store(1, std::sync::atomic::Ordering::Relaxed); }
            if has("--no-fma") { lightmap::lmaccum::HB_FMA.store(false, std::sync::atomic::Ordering::Relaxed); }
            if has("--snap-floor") { lightmap::sunpass::RASTER_SNAP_FLOOR.store(true, std::sync::atomic::Ordering::Relaxed); }
            if let Some(t) = f("--snap-tie") { lightmap::sunpass::RASTER_SNAP_TIE.store(t.parse().unwrap(), std::sync::atomic::Ordering::Relaxed); }
            // --tie-census: how many vegetation vertices sit exactly on a half 1/256 step (the tie rule matters only for those)
            if has("--interp-unsnapped") { lightmap::sunpass::RASTER_INTERP_UNSNAPPED.store(true, std::sync::atomic::Ordering::Relaxed); }
            let t0 = std::time::Instant::now();
            let sc = lightmap::lmaccum::load_lm_scene(&root, env_frame).expect("LM scene");
            let entries = lightmap::lmaccum::load_capture_entries(&manifest).expect("manifest");
            // the direction: by sweep index (--index K) or the first banked hbasis of --frame
            let capture = f("--capture").unwrap_or_else(|| "pwc2".into());
            let hb0: Vec<&lightmap::lmaccum::CapEntry> = entries.iter().filter(|e| e.pass == "hbasis0" && e.capture == capture && e.banked).collect();
            let target = match (f("--index"), f("--frame")) {
                (Some(k), _) => { let k: u32 = k.parse().expect("--index"); hb0.iter().find(|e| e.sweep_direction_index == Some(k) && e.cbuffers["ShaderP"].is_object()).copied().unwrap_or_else(|| panic!("no banked hbasis0 with sweep_direction_index {k}")) }
                (None, fr) => { let fr: u32 = fr.map(|v| v.parse().expect("--frame")).unwrap_or(127448); hb0.iter().find(|e| e.frame == fr).copied().unwrap_or_else(|| panic!("no banked hbasis0 in frame {fr}")) }
            };
            let k = target.sweep_direction_index.unwrap_or(0);
            let (cb, raster) = target.hb_constants().expect("hbasis entry constants");
            println!("direction index {k} (frame {}, eids {}-{}): D ({:.5},{:.5},{:.5}) InvDirCount {} raster offset {:?} → Trans {:?}", target.frame, target.eid_first, target.eid_last, cb.peel_dir[0], cb.peel_dir[1], cb.peel_dir[2], cb.inv_dir_count, lightmap::lmaccum::RASTER_OFFSETS[k as usize % 9], raster.trans_ss);
            // the four MRTs after this direction, and after the previous one (zero for the sweep's first)
            let mrt_after: Vec<&lightmap::lmaccum::CapEntry> = (0..4).map(|m| entries.iter().find(|e| e.pass == format!("hbasis{m}") && e.capture == capture && e.frame == target.frame && e.eid_last == target.eid_last && e.banked).unwrap_or_else(|| panic!("hbasis{m} after direction {k} not banked"))).collect();
            let prev: Option<Vec<&lightmap::lmaccum::CapEntry>> = if k == 0 { None } else {
                let p0 = hb0.iter().filter(|e| e.sweep_direction_index == Some(k - 1)).max_by_key(|e| (e.frame, e.eid_last)).copied().unwrap_or_else(|| panic!("hbasis0 after direction {} not banked — the MRTs before direction {k} are unknown", k - 1));
                Some((0..4).map(|m| entries.iter().find(|e| e.pass == format!("hbasis{m}") && e.capture == capture && e.frame == p0.frame && e.eid_last == p0.eid_last && e.banked).unwrap_or_else(|| panic!("hbasis{m} after direction {} not banked", k - 1))).collect())
            };
            // the ilightdir after the direction's last accumulate: the snapshot with the greatest eid_last before the H-basis draws
            // the ilightdir the H-basis draw reads: the `ilightdir_final` bank (17098 at the H-basis draw) or the last per-layer snapshot before it
            let ild = entries.iter().filter(|e| (e.pass == "ilightdir_final" || e.pass == "ilightdir") && e.capture == capture && e.frame == target.frame && e.eid_last <= target.eid_first).max_by_key(|e| (e.eid_last, e.pass == "ilightdir_final")).unwrap_or_else(|| panic!("no ilightdir snapshot before the H-basis draws of direction {k} (frame {} eid {})", target.frame, target.eid_first));
            println!("  inputs: ilightdir {} (eids {}-{}); MRTs before: {}", ild.file, ild.eid_first, ild.eid_last, prev.as_ref().map(|p| p[0].file.clone()).unwrap_or_else(|| "cleared (0)".into()));
            let ildb = ild.load(&root).expect("ilightdir");
            let mut ilt = lightmap::lmaccum::DirTarget::cleared(ildb.w, ildb.h);
            for y in 0..ildb.h { for x in 0..ildb.w { ilt.px[(y * ildb.w + x) as usize] = lightmap::gpufmt::pack_r11g11b10([ildb.get(x, y, 0), ildb.get(x, y, 1), ildb.get(x, y, 2)], lightmap::gpufmt::Rounding::Truncate); } }
            let mut tgt = match &prev {
                Some(p) => { let bufs: Vec<lightmap::passdiff::Buf> = p.iter().map(|e| e.load(&root).expect("previous MRT")).collect(); lightmap::lmaccum::HbTargets::from_bufs([&bufs[0], &bufs[1], &bufs[2], &bufs[3]]) }
                None => lightmap::lmaccum::HbTargets::cleared(2048, 2048),
            };
            // the four draws in the block's order (frame 127448: the pad, the wall, the vegetation item, the tiles)
            let order = [0usize, 1, 2, 3];
            let draws: Vec<lightmap::lmaccum::HbDraw> = order.iter().map(|&m| lightmap::lmaccum::HbDraw { eid: target.eid_first, mesh: m, instance_first: sc.inst_first[m], instance_count: sc.inst_count[m], raster, cb }).collect();
            let t1 = std::time::Instant::now();
            let probe: Option<(u32, u32)> = f("--probe").map(|s| { let v: Vec<u32> = s.split(',').map(|t| t.trim().parse().expect("--probe x,y")).collect(); (v[0], v[1]) });
            let mut owner = vec![0u8; 2048 * 2048];
            lightmap::lmaccum::run_hbasis_probe(&sc.meshes, &sc.instances, &sc.table, &draws, &ilt, &mut tgt, blend, Some(&mut owner), probe);
            println!("  rasterised in {:.1} s ({blend:?})", t1.elapsed().as_secs_f32());
            let game: Vec<lightmap::passdiff::Buf> = mrt_after.iter().map(|e| e.load(&root).expect("MRT after")).collect();
            if let Some((x, y)) = probe { println!("  game at ({x},{y}): {:?}; ours {:?}", (0..4).map(|m| [game[m].get(x, y, 0), game[m].get(x, y, 1), game[m].get(x, y, 2), game[m].get(x, y, 3)]).collect::<Vec<_>>(), (0..4).map(|m| tgt.mrt[m][(y * 2048 + x) as usize]).collect::<Vec<_>>()); }
            // per object (the mesh that wrote the pixel): exact / 1 ulp / worse over rgb of every MRT
            for (mi, name) in ["pad (24 idx)", "wall (12 idx)", "vegetation (5751 idx)", "tiles (24 × 4096)"].iter().enumerate() {
                let (mut n, mut exact, mut ulp1, mut worse) = (0usize, 0usize, 0usize, 0usize);
                for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] != mi as u8 + 1 { continue; } for m in 0..4 { for ch in 0..3 { let g = game[m].get(x, y, ch as u32); let o = tgt.mrt[m][i][ch]; if g == 0.0 && o == 0.0 { continue; } n += 1; let d = (o - g).abs(); if d == 0.0 { exact += 1; } else { let ulp = (lightmap::gpufmt::decode_f16(lightmap::gpufmt::encode_f16(g, lightmap::gpufmt::Rounding::NearestEven).wrapping_add(1)) - g).abs(); if d <= ulp * 1.001 { ulp1 += 1 } else { worse += 1 } } } } } }
                let pct = |v: usize| if n > 0 { 100.0 * v as f64 / n as f64 } else { 0.0 };
                println!("  {name:22} rgb of C0..C3: {n:>9} values  exact {exact:>9} ({:6.2} %)  1 ulp {ulp1:>7} ({:5.2} %)  worse {worse:>5} ({:5.3} %)", pct(exact), pct(ulp1), pct(worse));
            }
            // the vegetation alone by fragment count (its single-fragment pixels test the interpolation of a VARYING attribute
            // without the two-fragment blend in the way; the flat pad / wall / tiles cannot)
            {
                let n_dirs = (1.0 / cb.inv_dir_count).round();
                let mut by = std::collections::BTreeMap::<u32, (usize, usize, usize, usize)>::new();
                for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] != 3 { continue; } let frags = (tgt.mrt[0][i][3] * n_dirs).round() as u32; let e = by.entry(frags).or_insert((0, 0, 0, 0)); for m in 0..4 { for ch in 0..3 { let g = game[m].get(x, y, ch as u32); let o = tgt.mrt[m][i][ch]; e.0 += 1; if g == o { e.1 += 1; } else { let ulps = (lightmap::gpufmt::encode_f16(g, lightmap::gpufmt::Rounding::NearestEven) as i32 - lightmap::gpufmt::encode_f16(o, lightmap::gpufmt::Rounding::NearestEven) as i32).abs(); if ulps <= 1 { e.2 += 1; } else { e.3 += 1; } } } } } }
                for (k, (n, ex, u1, w)) in &by { println!("  vegetation pixels with {k} fragment(s): {n:>9} values  exact {ex:>9} ({:6.2} %)  1 ulp {u1:>7} ({:5.2} %)  worse {w:>5}", 100.0 * *ex as f64 / (*n).max(1) as f64, 100.0 * *u1 as f64 / (*n).max(1) as f64); }
            }
            // by the number of fragments the pixel received this direction (alpha increment × N): the multi-fragment
            // (two-sided / overlapping card) pixels are where the blend order and rounding show
            {
                let n_dirs = (1.0 / cb.inv_dir_count).round();
                let mut by = std::collections::BTreeMap::<u32, (usize, usize, usize, usize)>::new();
                let before_alpha = |x: u32, y: u32| -> f32 { prev.as_ref().map(|_| 0.0).unwrap_or(0.0) + 0.0 * (x + y) as f32 };
                for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] == 0 { continue; } let frags = ((tgt.mrt[0][i][3] - before_alpha(x, y)) * n_dirs).round() as u32; let e = by.entry(frags).or_insert((0, 0, 0, 0)); for m in 0..4 { for ch in 0..3 { let g = game[m].get(x, y, ch as u32); let o = tgt.mrt[m][i][ch]; if g == 0.0 && o == 0.0 { continue; } e.0 += 1; let d = (o - g).abs(); if d == 0.0 { e.1 += 1; } else { let ulp = (lightmap::gpufmt::decode_f16(lightmap::gpufmt::encode_f16(g, lightmap::gpufmt::Rounding::NearestEven).wrapping_add(1)) - g).abs(); if d <= ulp * 1.001 { e.2 += 1 } else { e.3 += 1 } } } } } }
                if prev.is_none() { for (k, (n, ex, u1, w)) in &by { println!("  pixels with {k} fragment(s): {n:>9} values  exact {ex:>9} ({:6.2} %)  1 ulp {u1:>7} ({:5.2} %)  worse {w:>5}", 100.0 * *ex as f64 / (*n).max(1) as f64, 100.0 * *u1 as f64 / (*n).max(1) as f64); } }
            }
            for m in 0..4 {
                for ch in 0..4 {
                    let (n, exact, ulp1, worse, maxd, worst) = lightmap::lmaccum::compare_mrt(&tgt.mrt[m], &game[m], ch);
                    let pct = |v: usize| if n > 0 { 100.0 * v as f64 / n as f64 } else { 0.0 };
                    println!("  C{m}.{}: {n:>8} values  exact {exact:>8} ({:6.2} %)  1 ulp {ulp1:>7} ({:5.2} %)  worse {worse:>6} ({:5.3} %)  max |Δ| {maxd:.6} at ({},{}) game {:.6} ours {:.6}", ["r", "g", "b", "a"][ch], pct(exact), pct(ulp1), pct(worse), worst.0, worst.1, worst.2, worst.3);
                }
            }
            // --list-worse N: the first N mismatching vegetation values (pixel, MRT, channel, game, ours, fragments) — the probe targets
            if let Some(n) = f("--list-worse").map(|v| v.parse::<usize>().unwrap()) {
                let n_dirs = (1.0 / cb.inv_dir_count).round();
                let mut shown = 0usize;
                'outer: for y in 0..2048u32 { for x in 0..2048u32 { let i = (y * 2048 + x) as usize; if owner[i] != 3 { continue; } let frags = (tgt.mrt[0][i][3] * n_dirs).round() as u32; if f("--list-frags").map(|v| v.parse::<u32>().unwrap()) .map_or(false, |k| k != frags) { continue; } for m in 0..4 { for ch in 0..3 { let g = game[m].get(x, y, ch as u32); let o = tgt.mrt[m][i][ch]; if g != o { let ulps = (lightmap::gpufmt::encode_f16(g, lightmap::gpufmt::Rounding::NearestEven) as i32 - lightmap::gpufmt::encode_f16(o, lightmap::gpufmt::Rounding::NearestEven) as i32).abs(); println!("  worse: ({x},{y}) C{m}.{} game {g:.7} ours {o:.7} ({ulps} f16 ulp) fragments {frags}", ["r", "g", "b"][ch]); shown += 1; if shown >= n { break 'outer; } } } } } }
            }
            // alpha = the coverage count: where do the two rasters disagree?
            let (mut ours_only, mut game_only) = (0usize, 0usize);
            for y in 0..2048u32 { for x in 0..2048u32 { let o = tgt.mrt[0][(y * 2048 + x) as usize][3]; let g = game[0].get(x, y, 3); if o > 0.0 && g == 0.0 { ours_only += 1; } if g > 0.0 && o == 0.0 { game_only += 1; } } }
            println!("  coverage (alpha > 0): ours-only {ours_only}, game-only {game_only}");
            if let Some(p) = f("--dump-prefix") {
                for m in 0..4 { let mut bytes = Vec::with_capacity(2048 * 2048 * 8); for px in &tgt.mrt[m] { for k in 0..4 { bytes.extend_from_slice(&lightmap::gpufmt::encode_f16(px[k], lightmap::gpufmt::Rounding::NearestEven).to_le_bytes()); } } std::fs::write(format!("{p}_hbasis{m}.bin"), &bytes).expect("dump"); }
                println!("  wrote {p}_hbasis0..3.bin (raw RGBA16F 2048²)");
            }
            println!("done in {:.1} s", t0.elapsed().as_secs_f32());
        }
        "ambient-check" => {
            // lmtool ambient-check PASSCAP_ROOT [--capture pwc2]: the AddAmbient UAV snapshots (ambient_accum/) against the
            //   transcribed CS 17125 — the .w channel = Σ Scale/2 over the directions issued so far (Scale = D.y/32 from the
            //   H-basis entries' directions), the .xyz increments ÷ Scale = the peel colour target's centre pixel after the
            //   direction's environment render, compared with the banked env-layer colour where one exists
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let manifest = f("--manifest").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"));
            let capture = f("--capture").unwrap_or_else(|| "pwc2".into());
            let entries = lightmap::lmaccum::load_capture_entries(&manifest).expect("manifest");
            let mut acc: Vec<&lightmap::lmaccum::CapEntry> = entries.iter().filter(|e| e.pass == "ambient_accum" && e.capture == capture).collect();
            acc.sort_by_key(|e| (e.frame, e.eid_last));
            // every direction of the capture in issue order with its D (from any hbasis0 entry, banked or not)
            let mut dirs: Vec<(u32, u32, u64, [f32; 3])> = entries.iter().filter(|e| e.pass == "hbasis0" && e.capture == capture && e.dir.is_some() && e.sweep_direction_index.is_some()).map(|e| (e.sweep_direction_index.unwrap(), e.frame, e.eid_last, e.dir.unwrap())).collect();
            dirs.sort_by_key(|d| (d.0, d.1, d.2));
            dirs.dedup_by_key(|d| d.0);
            println!("{} ambient_accum snapshots, {} directions with a vector (indices {}..{})", acc.len(), dirs.len(), dirs.first().map(|d| d.0).unwrap_or(0), dirs.last().map(|d| d.0).unwrap_or(0));
            let load4 = |e: &lightmap::lmaccum::CapEntry| -> [f32; 4] { let b = lightmap::passdiff::read_entry_bytes(&root, &e.file).expect("uav bytes"); let g = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap()); [g(0), g(4), g(8), g(12)] };
            // a snapshot taken at (frame, eid) covers the directions whose H-basis draws come after it... the dispatch of
            // direction k precedes k's blocks: the snapshot includes k when k's hbasis eid_last > eid in the same frame or later
            let mut prev: Option<([f32; 4], u32)> = None;
            let (mut n_w_ok, mut n_w) = (0usize, 0usize);
            let (mut n_cs_ok, mut n_cs) = (0usize, 0usize);
            for e in &acc {
                let v = load4(e);
                // the direction this dispatch belongs to: the one whose H-basis draws come next after it — the dispatch
                // of direction k opens k's iteration (right after k's environment render, before its first accumulate)
                let next = dirs.iter().find(|d| (d.1, d.2) > (e.frame, e.eid_last)).map(|d| d.0);
                let Some(k) = next else { println!("frame {} eid {}: {:?} (after the last known direction)", e.frame, e.eid_last, v); continue; };
                // the snapshot holds the directions 0..=k: only the UPWARD ones dispatch (FUN_140234df0: the dispatch runs
                // when 4·w·D.y·Sky > 0) with Scale = D.y/32 — checkable when every index ≤ k has a captured vector
                let count = k + 1;
                let complete = (0..count).all(|j| dirs.iter().any(|d| d.0 == j));
                let w_expect: f32 = dirs.iter().filter(|d| d.0 < count && d.3[1] > 0.0).fold(0f32, |s, d| d.3[1] / 32.0 * 0.5 + s);
                let w_ok = complete && (v[3] - w_expect).abs() <= 2e-7 * v[3].abs().max(1.0);
                if complete { n_w += 1; if w_ok { n_w_ok += 1; } }
                let mut line = format!("frame {:6} eid {:5}: through direction {:3}: w {:.7} (Σ max(D.y,0)/64 over 0..={k}: {:.7} {})  xyz ({:.6},{:.6},{:.6})", e.frame, e.eid_last, k, v[3], w_expect, if !complete { "gap: uncaptured directions" } else if w_ok { "OK" } else { "MISMATCH" }, v[0], v[1], v[2]);
                // the previous snapshot (or the cleared buffer before direction 0)
                let (pv, pcount) = prev.unwrap_or(([0.0f32; 4], 0));
                if pcount + 1 == count {
                    // one direction between the two snapshots: its Scale = 2·Δw (the .w channel adds Scale/2), its centre
                    // colour = Δxyz / Scale
                    let d = dirs.iter().find(|d| d.0 == k).unwrap();
                    let scale_meas = (v[3] - pv[3]) * 2.0;
                    let c = [(v[0] - pv[0]) / scale_meas, (v[1] - pv[1]) / scale_meas, (v[2] - pv[2]) / scale_meas];
                    line += &format!("  Scale = 2Δw = {:.7} (×32 = {:.5}; D ({:.4},{:.4},{:.4}), D.y/32 = {:.7} {})  Δ/Scale = centre colour ({:.4},{:.4},{:.4})", scale_meas, scale_meas * 32.0, d.3[0], d.3[1], d.3[2], d.3[1] / 32.0, if scale_meas == d.3[1] / 32.0 { "EXACT" } else { "differs" }, c[0], c[1], c[2]);
                } else {
                    line += &format!("  ({} directions since the previous snapshot)", count - pcount);
                }
                // the transcribed CS over the banked environment renders: for every upward direction j in (pcount, k],
                // its env render = the first peel_color banked between direction j−1's H-basis and direction j's
                // (the env block opens the iteration, before the dispatch); when every one is banked the chain from the
                // previous snapshot must land on this one exactly
                let mut acc2 = pv;
                let mut all_env = true;
                let mut used: Vec<String> = Vec::new();
                for j in pcount..count {
                    let Some(d) = dirs.iter().find(|d| d.0 == j) else { all_env = false; break };
                    if d.3[1] <= 0.0 { continue; }
                    let lo = dirs.iter().filter(|p| p.0 < j).map(|p| (p.1, p.2)).max().unwrap_or((0, 0));
                    let hi = (d.1, d.2);
                    let env = entries.iter().filter(|x| x.pass == "peel_color" && x.capture == capture && (x.frame, x.eid_last) > lo && (x.frame, x.eid_last) < hi).min_by_key(|x| (x.frame, x.eid_first));
                    let Some(env) = env else { all_env = false; break };
                    let b = env.load(&root).expect("env colour");
                    let (cx, cy) = lightmap::lmaccum::ambient_pixel(b.w, b.h);
                    let g = [b.get(cx, cy, 0), b.get(cx, cy, 1), b.get(cx, cy, 2)];
                    lightmap::lmaccum::cs_17125(&mut acc2, g, d.3[1] / 32.0);
                    used.push(format!("{} centre ({:.4},{:.4},{:.4})", env.file.rsplit('/').next().unwrap_or(""), g[0], g[1], g[2]));
                }
                if all_env && !used.is_empty() {
                    let exact = acc2 == v;
                    n_cs += 1; if exact { n_cs_ok += 1; }
                    line += &format!("; CS 17125 over the banked env renders [{}] → ({:.7},{:.7},{:.7},{:.7}) {}", used.join(", "), acc2[0], acc2[1], acc2[2], acc2[3], if exact { "EXACT" } else { "differs" });
                }
                println!("{line}");
                prev = Some((v, count));
            }
            println!("w channel: {n_w_ok} of {n_w} gap-free snapshots equal Σ max(D.y, 0)/64 over the directions issued (the upward directions dispatch with Scale = 8·D.y/N, N = 256; the downward ones do not); CS 17125 over banked env renders: {n_cs_ok} of {n_cs} snapshots reproduced bit for bit");
        }
        "dxbc-literals" => {
            // lmtool dxbc-literals FILE.dxbc: every IMMEDIATE32 literal of the shader's token stream, in instruction order,
            //   with its exact bits (the disassembly prints six decimals)
            let b = std::fs::read(&a[1]).expect("dxbc file");
            let (ptype, major, minor, t) = lightmap::dxbc::tokens(&b).expect("no SHEX/SHDR chunk");
            println!("{}: program type {ptype}, sm {major}.{minor}, {} tokens", a[1], t.len());
            for (name, idx, reg, mask) in lightmap::dxbc::signature(&b, b"ISGN") { println!("  input  v{reg} = {name}{idx} (mask {mask:#x})"); }
            for (name, idx, reg, mask) in lightmap::dxbc::signature(&b, b"OSGN") { println!("  output o{reg} = {name}{idx} (mask {mask:#x})"); }
            for (idx, op, v, bits) in lightmap::dxbc::literal_f32s(&b) {
                println!("  inst {idx:3} opcode {op:#04x}: {v:>14.9} ({bits:#010x}){}", if bits & 0x7f80_0000 == 0 && bits != 0 { " denormal" } else if v.is_nan() { " NaN" } else { "" });
            }
        }
        "cappixel" => {
            // lmtool cappixel PASSCAP_ROOT --pass hbasis0 [--index K | --frame N --eid E] --at x,y [--manifest M] [--capture pwc2]
            //   one pixel of a captured buffer (every channel), for quick looks at the capture
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let manifest = f("--manifest").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"));
            let entries = lightmap::lmaccum::load_capture_entries(&manifest).expect("manifest");
            let pass = f("--pass").unwrap_or_else(|| "hbasis0".into());
            let capture = f("--capture").unwrap_or_else(|| "pwc2".into());
            let at: Vec<u32> = f("--at").expect("--at x,y").split(',').map(|t| t.trim().parse().expect("--at x,y")).collect();
            let sel: Vec<&lightmap::lmaccum::CapEntry> = entries.iter().filter(|e| e.pass == pass && e.capture == capture && e.banked).filter(|e| {
                match (f("--index"), f("--frame"), f("--eid")) {
                    (Some(k), _, _) => e.sweep_direction_index == Some(k.parse().expect("--index")),
                    (None, Some(fr), Some(eid)) => e.frame == fr.parse::<u32>().expect("--frame") && e.eid_last == eid.parse::<u64>().expect("--eid"),
                    (None, Some(fr), None) => e.frame == fr.parse::<u32>().expect("--frame"),
                    _ => true,
                }
            }).collect();
            for e in sel {
                let b = e.load(&root).expect("load");
                if a.iter().any(|x| x == "--stats") {
                    let (mut nz, mut sum) = (0usize, [0f64; 4]);
                    for y in 0..b.h { for x in 0..b.w { let mut any = false; for c in 0..b.channels.min(4) { let v = b.get(x, y, c); sum[c as usize] += v as f64; if v != 0.0 { any = true; } } if any { nz += 1; } } }
                    println!("{} frame {} eids {}-{} sdi {:?}: {}×{}×{}, {nz} non-zero pixels ({:.2} %), channel means {:?}", e.file, e.frame, e.eid_first, e.eid_last, e.sweep_direction_index, b.w, b.h, b.channels, 100.0 * nz as f64 / (b.w * b.h) as f64, sum.iter().map(|s| s / (b.w * b.h) as f64).collect::<Vec<_>>());
                }
                let v: Vec<f32> = (0..b.channels).map(|c| b.get(at[0], at[1], c)).collect();
                println!("{} frame {} eids {}-{} sdi {:?} dir {:?}: ({},{}) = {:?}", e.file, e.frame, e.eid_first, e.eid_last, e.sweep_direction_index, e.dir.map(|d| [(d[0] * 1000.0).round() / 1000.0, (d[1] * 1000.0).round() / 1000.0, (d[2] * 1000.0).round() / 1000.0]), at[0], at[1], v);
            }
        }
        "sweep1-check" => {
            // lmtool sweep1-check PASSCAP_ROOT [--kappa K] [--fit-srgb]: ROW 11 — the sweep-1 ILightInput chain (sweep1.rs:
            //   PS 25113 resolve of the sweep-0 C0 → × 1/√(2π) → × MDiffuse (PS 1109) → PS 1335 × 8) from the captured inputs
            //   vs the captured texture the sweep-1 peels sample (frame 7534 eid 32)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let root = std::path::PathBuf::from(&a[1]);
            let kappa: f32 = f("--kappa").map(|v| v.parse().expect("--kappa")).unwrap_or(lightmap::sweep1::KAPPA_BOUNCE);
            if let Some(k) = f("--direction") {
                // --direction K [--frame 7534] [--capture pwc6] [--env-frame 127448]: one sweep-1 direction end to end
                let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(7534);
                let env_frame: u32 = f("--env-frame").map(|v| v.parse().expect("--env-frame")).unwrap_or(127448);
                let capture = f("--capture").unwrap_or_else(|| "pwc6".into());
                let ids = if capture == "pwc2" { &lightmap::sweep1::PWC2_IDS } else { &lightmap::sweep1::PWC6_IDS };
                if let Err(e) = lightmap::sweep1::check_direction(&root, frame, k.parse().expect("--direction"), env_frame, ids, &capture) { eprintln!("sweep1-check: {e}"); std::process::exit(1); }
            } else if let Err(e) = lightmap::sweep1::check_ilightinput(&root, kappa, has("--fit-srgb"), has("--mask-sun")) { eprintln!("sweep1-check: {e}"); std::process::exit(1); }
        }
        "sweep-dirs" => {
            // lmtool sweep-dirs GAME/MANIFEST.json [--points FILE]: per sweep the point set the captured directions come
            // from (the rotated N-set of Std.PointsInSphere), the issue order as set indices, the missing ones
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let txt = std::fs::read_to_string(&a[1]).expect("manifest");
            let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
            let ps = lightmap::dome::PointSets::load(&f("--points").unwrap_or_else(lightmap::dome::default_path)).expect("point sets");
            print!("{}", lightmap::sweep1::report(&m, &ps));
        }
        "probe-download-check" => {
            // lmtool probe-download-check PASSCAP_ROOT MAP.Gbx [--frame 74490]: the end-state probe volumes through the CPU download
            //   model (probepass::download_probes) vs the baked map's trailer scales and its stored WEBP probe images
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(74490);
            if let Err(e) = lightmap::probecheck::download_check(&std::path::PathBuf::from(&a[1]), &a[2], frame) { eprintln!("probe-download-check: {e}"); std::process::exit(1); }
        }
        "probe-images" => {
            // lmtool probe-images MAP.Gbx [OUTDIR]: the baked map's four probe images level by level (+ ×8 PNGs)
            if let Err(e) = lightmap::probecheck::probe_images(&a[1], a.get(2).map(|s| s.as_str())) { eprintln!("probe-images: {e}"); std::process::exit(1); }
        }
        "dilate-check" => {
            // lmtool dilate-check PASSCAP_ROOT [--frame N]: the transcribed PS 1332 gutter dilation (gpuenc::dilate_ps1332), 8 passes on
            // each captured final_03 image, compared f16 for f16 with the captured final_04 images
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(74490);
            let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
            let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
            for id in ["24858", "24752", "24749", "24852"] {
                let before = m.passes.iter().find(|e| e.pass == "final_03_after_colormat_ps1034" && e.frame == Some(frame) && e.file.contains(&format!("_{id}.dds"))).expect("final_03 entry");
                let after = m.passes.iter().find(|e| e.pass == "final_04_after_dilate8_ps1332" && e.frame == Some(frame) && e.file.contains(&format!("_{id}.dds"))).expect("final_04 entry");
                let mut img = lightmap::passdiff::load_entry(&root, before).expect("load before");
                let target = lightmap::passdiff::load_entry(&root, after).expect("load after");
                let t0 = std::time::Instant::now();
                let fma = a.iter().any(|x| x == "--fma");
                // the capture shows the un-blended f16 store TRUNCATES (all 4 × 16 777 216 values identical with it, 3 % off by one ulp with RTNE)
                let store = if a.iter().any(|x| x == "--rtne") { lightmap::gpufmt::Rounding::NearestEven } else { lightmap::gpufmt::Rounding::Truncate };
                for _ in 0..8 { img = lightmap::gpuenc::dilate_ps1332_full(&img, fma, store); }
                let (n, exact, ulp1, worse, maxd) = lightmap::gpuenc::compare_buf(&img, &target, 4);
                let covered = (0..target.h).flat_map(|y| (0..target.w).map(move |x| (x, y))).filter(|&(x, y)| target.get(x, y, 3) > 0.0).count();
                println!("image {id}: 8 × PS 1332 in {:.1} s → {exact} of {n} values bit-identical, {ulp1} within 1 f16 ulp, {worse} worse (max |Δ| {maxd:.5}); covered texels after: {covered}", t0.elapsed().as_secs_f32());
            }
        }
        "finalprep-check" => {
            // lmtool finalprep-check PASSCAP_ROOT [--frame 74490] [--rtne] [--show N]
            //   ROW 12: the transcribed finalisation prep (finalprep.rs — PS 25113 resolve, PS 1109 ×2, PS 1034 copy) run on the
            //   captured final_00 images and compared f16 for f16 with the captured final_01 / final_02 / final_03, step by step
            //   (each kernel on the captured input of its step) and chained (final_00 → our three kernels → final_03)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(74490);
            let show: usize = f("--show").map(|v| v.parse().expect("--show")).unwrap_or(0);
            let store = if a.iter().any(|x| x == "--rtne") { lightmap::gpufmt::Rounding::NearestEven } else { lightmap::gpufmt::Rounding::Truncate };
            let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
            let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
            let entry = |pass: &str, id: &str| m.passes.iter().find(|e| e.pass == pass && e.frame == Some(frame) && e.file.contains(&format!("_{id}.dds"))).unwrap_or_else(|| panic!("no {pass} entry for texture {id} in frame {frame}"));
            let load = |pass: &str, id: &str| lightmap::passdiff::load_entry(&root, entry(pass, id)).unwrap_or_else(|e| panic!("{e}"));
            use lightmap::gpucmp::{compare, Fmt};
            // the chain's texture ids (draws-frame74490.json eids 34675–34917): PS 25113 src → dst, PS 1109 src → dst, PS 1034 src → dst
            const ROT: [(&str, &str); 4] = [("24752", "24858"), ("24749", "24752"), ("24852", "24749"), ("24855", "24852")];
            const SCL: [(&str, &str); 4] = [("24858", "24911"), ("24752", "24914"), ("24749", "24917"), ("24852", "24920")];
            const CPY: [(&str, &str); 4] = [("24911", "24858"), ("24914", "24752"), ("24917", "24749"), ("24920", "24852")];
            let mut all_closed = true;
            println!("PS 25113 (final_00 → final_01), RGBA16F store {:?}:", store);
            let mut chained: Vec<lightmap::passdiff::Buf> = Vec::new();
            for (src, dst) in ROT {
                let s = load("final_00_hbasis_sweep_end", src);
                let t = load("final_01_after_rotate_ps25113", dst);
                let t0 = std::time::Instant::now();
                let ours = lightmap::finalprep::resolve_ps25113(&s, false, store);
                let r = compare(&ours, &t, 4, Fmt::F16);
                let covered = (0..t.h).flat_map(|y| (0..t.w).map(move |x| (x, y))).filter(|&(x, y)| t.get(x, y, 3) > 0.99).count();
                println!("  {src} → {dst} ({:.1} s): {} — {covered} texels with alpha > 0.99 after", t0.elapsed().as_secs_f32(), r.line());
                if show > 0 { lightmap::gpucmp::print_diffs(&ours, &t, 4, show); }
                all_closed &= r.closed();
                chained.push(ours);
            }
            // PS 1109 blends One/One onto whatever the targets 24911/24914/24917/24920 hold: the capture shows they are NOT empty
            // (final_02 ≠ 2 × final_01) — the hypothesis tested here: they hold the PREVIOUS SWEEP's finalised images (the same
            // chain run at the end of sweep 0: PS 25113 on the sweep-0 H-basis MRTs, × 2), so the ×2 step is where the sweeps add up.
            // --prior FRAME:EID names the banked hbasis0..3 snapshot at the end of the previous sweep (default 7533:13903 = pwc6's
            // sweep-0 end); --prior none tests the cleared-target reading.
            let prior = f("--prior").unwrap_or_else(|| "7533:13903".to_string());
            let prior_imgs: Option<Vec<lightmap::passdiff::Buf>> = if prior == "none" { None } else {
                let (pf, pe) = prior.split_once(':').expect("--prior FRAME:EID");
                let (pf, pe): (u32, u64) = (pf.parse().unwrap(), pe.parse().unwrap());
                let imgs: Vec<lightmap::passdiff::Buf> = (0..4).map(|k| {
                    let e = m.passes.iter().find(|e| e.pass == format!("hbasis{k}") && e.frame == Some(pf) && e.eid_last == Some(pe)).unwrap_or_else(|| panic!("no hbasis{k} entry at frame {pf} eid {pe}"));
                    let b = lightmap::passdiff::load_entry(&root, e).unwrap_or_else(|e| panic!("{e}"));
                    // the previous sweep's finalisation: PS 25113 then × 2 (its own 1109 onto a cleared target)
                    lightmap::finalprep::scale_ps1109(&lightmap::finalprep::resolve_ps25113(&b, false, store), [2.0, 2.0, 2.0, 0.0], lightmap::gpufmt::Rounding::Truncate, lightmap::gpufmt::Rounding::NearestEven)
                }).collect();
                println!("PS 1109 × ScaleSrc (2, 2, 2, 0), blend One/One onto the previous sweep's finalised images (hbasis0..3 at frame {pf} eid {pe} → our 25113 → × 2) (final_01 → final_02):");
                Some(imgs)
            };
            if prior_imgs.is_none() { println!("PS 1109 × ScaleSrc (2, 2, 2, 0), blend One/One onto a cleared target (final_01 → final_02):"); }
            for (i, (src, dst)) in SCL.iter().enumerate() {
                let s = load("final_01_after_rotate_ps25113", src);
                let t = load("final_02_scaled_x2_ps1109", dst);
                let blend = |src_img: &lightmap::passdiff::Buf| -> lightmap::passdiff::Buf {
                    let scaled = lightmap::finalprep::scale_ps1109(src_img, [2.0, 2.0, 2.0, 0.0], lightmap::gpufmt::Rounding::Truncate, lightmap::gpufmt::Rounding::Truncate);
                    match &prior_imgs {
                        None => scaled,
                        Some(p) => {
                            // the blend: dst = f16_rtne(prior + f16_rtz(src)) per channel (the baker's blended-f16 rule)
                            let mut out = scaled.clone();
                            for y in 0..out.h { for x in 0..out.w { for k in 0..4 {
                                let v = p[i].get(x, y, k) + scaled.get(x, y, k);
                                out.set(x, y, k, lightmap::gpufmt::quantise_f16(v, lightmap::gpufmt::Rounding::NearestEven));
                            } } }
                            out
                        }
                    }
                };
                let ours = blend(&s);
                let r = compare(&ours, &t, 4, Fmt::F16);
                println!("  {src} → {dst}: {}", r.line());
                if show > 0 { lightmap::gpucmp::print_diffs(&ours, &t, 4, show); }
                all_closed &= r.closed();
                chained[i] = blend(&chained[i]);
            }
            println!("PS 1034 copy, write mask RGB, Raster_ST_Input (1, 1, 0, 0) (final_02 → final_03, alpha kept from final_01):");
            for (i, (src, dst)) in CPY.iter().enumerate() {
                let s = load("final_02_scaled_x2_ps1109", src);
                let base = load("final_01_after_rotate_ps25113", dst);
                let t = load("final_03_after_colormat_ps1034", dst);
                let ours = lightmap::finalprep::write_masked(&base, &lightmap::finalprep::copy_ps1034(&s, [1.0, 1.0, 0.0, 0.0], s.w, s.h), 7, store);
                let r = compare(&ours, &t, 4, Fmt::F16);
                println!("  {src} → {dst}: {}", r.line());
                if show > 0 { lightmap::gpucmp::print_diffs(&ours, &t, 4, show); }
                all_closed &= r.closed();
                // the chain: our resolve → our ×2 → our copy, against the captured final_03
                let rot_out = &chained[i];
                let ours_chain = lightmap::finalprep::write_masked(&lightmap::finalprep::resolve_ps25113(&load("final_00_hbasis_sweep_end", ROT[i].0), false, store), &lightmap::finalprep::copy_ps1034(rot_out, [1.0, 1.0, 0.0, 0.0], s.w, s.h), 7, store);
                let rc = compare(&ours_chain, &t, 4, Fmt::F16);
                println!("    chained final_00 {} → our 25113 → our 1109 → our 1034 vs captured final_03 {dst}: {}", ROT[i].0, rc.line());
                all_closed &= rc.closed();
            }
            println!("ROW 12 {}", if all_closed { "CLOSED: every value bit-identical or within one f16 quantum" } else { "NOT closed (values beyond one quantum above)" });
        }
        "ilightin-check" => {
            // lmtool ilightin-check PASSCAP_ROOT [--frame 127448] [--pre-frame 127447] [--fma] [--show N]
            //   ROW 4: the transcribed ILightInput chain (ilightin.rs) of the first compute frame, step by step against the banked
            //   setup_ps* snapshots: PS 17043 (the MDiffuse resolve into B8G8R8A8, eid 27), the cleared-target 1332/1034 steps,
            //   PS 1038 (the coverage mask from the direct sun's alpha, eid 791), PS 17043 + PS 1109 DstCol (eids 812/836),
            //   PS 1335 × 8 (eids 857–927, colour + coverage MRT)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127448);
            let pre_frame: u32 = f("--pre-frame").map(|v| v.parse().expect("--pre-frame")).unwrap_or(127447);
            let show: usize = f("--show").map(|v| v.parse().expect("--show")).unwrap_or(0);
            let fma = !a.iter().any(|x| x == "--no-fma");
            let dm = if a.iter().any(|x| x == "--div-ieee") { lightmap::gpucmp::DivModel::Ieee } else { lightmap::gpucmp::DivModel::MulRcp };
            let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
            let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
            let find = |pass: &str, fr: u32, sub: &str| m.passes.iter().find(|e| e.pass == pass && e.frame == Some(fr) && e.file.contains(sub)).unwrap_or_else(|| panic!("no {pass} entry matching {sub} in frame {fr}"));
            let load = |pass: &str, fr: u32, sub: &str| lightmap::passdiff::load_entry(&root, find(pass, fr, sub)).unwrap_or_else(|e| panic!("{e}"));
            use lightmap::gpucmp::{compare, Fmt};
            use lightmap::gpuenc::UnormRounding;
            use lightmap::gpufmt::Rounding;
            let unorm_modes = [UnormRounding::NearestEven, UnormRounding::HalfUp, UnormRounding::Truncate, UnormRounding::Trunc12];
            let mut all_closed = true;
            // A. eid 27: the 9-run MDiffuse accumulation (16963 at the end of the pre-pass frame) resolved into 16969 B8G8R8A8 — the
            //    capture shows the store goes through an sRGB view (0.435 linear → the byte 176 = 0.690 encoded): rgb are encoded
            //    by the store, alpha is not
            {
                let pre = m.passes.iter().filter(|e| e.pass == "setup_ps1109" && e.frame == Some(pre_frame)).max_by_key(|e| e.eid_last.unwrap_or(0)).expect("the pre-pass frame's last setup_ps1109 snapshot (16963)");
                let src = lightmap::passdiff::load_entry(&root, pre).unwrap_or_else(|e| panic!("{e}"));
                let t = load("setup_ps17043", frame, "_16969");
                let ours = lightmap::ilightin::resolve_ps17043(&src, false);
                println!("A. PS 17043 (eid 27): {} (16963 after the last pre-pass run) → 16969 B8G8R8A8 through an _SRGB view, per UNORM8 store rounding:", pre.file);
                let mut best: Option<(UnormRounding, bool, lightmap::gpucmp::Report, lightmap::passdiff::Buf)> = None;
                for srgb in [true, false] {
                    for mode in unorm_modes {
                        let mut q = lightmap::passdiff::Buf::new(ours.w, ours.h, 4);
                        for y in 0..ours.h { for x in 0..ours.w { for k in 0..4 {
                            let v = ours.get(x, y, k);
                            let v = if srgb && k < 3 { lightmap::gpufmt::linear_to_srgb(v) } else { v };
                            q.set(x, y, k, lightmap::ilightin::unorm8_rt(v, mode));
                        } } }
                        let r = compare(&q, &t, 4, Fmt::Unorm8);
                        println!("  srgb {srgb} {mode:?}: {}", r.line());
                        if best.as_ref().map_or(true, |(_, _, b, _)| r.exact > b.exact) { best = Some((mode, srgb, r, q)); }
                    }
                }
                let (mode, srgb, r, q) = best.unwrap();
                println!("  → best srgb {srgb} {mode:?}: {} exact of {}", r.exact, r.values);
                if show > 0 { lightmap::gpucmp::print_diffs(&q, &t, 4, show); }
                all_closed &= r.closed();
            }
            // B/C. the cleared 16963 through PS 1332 × 8 and the two PS 1034 copies: every snapshot is zero
            {
                let mut nz = 0usize;
                let mut n = 0usize;
                for e in m.passes.iter().filter(|e| (e.pass == "setup_ps1332" || e.pass == "setup_ps1034") && e.frame == Some(frame)) {
                    let b = lightmap::passdiff::load_entry(&root, e).unwrap_or_else(|e| panic!("{e}"));
                    let k = b.data.iter().filter(|v| **v != 0.0).count();
                    nz += k;
                    n += 1;
                    if k > 0 { println!("  {} has {k} non-zero values", e.file); }
                }
                println!("B/C. PS 1332 × 8 + PS 1034 × 2 on the cleared 16963: {n} snapshots, {nz} non-zero values (a dilation and two copies of zeros)");
                all_closed &= nz == 0;
            }
            // D. eid 791: PS 1038 — the direct-sun target's alpha into the R8 coverage 17104
            let sun = load("sun_direct", frame, "");
            let mask_rows = [[0.0f32; 4], [0.0; 4], [0.0; 4], [1.0; 4]];
            let mask_f = lightmap::ilightin::mask_ps1038(&sun, [1.0, 1.0, 0.0, 0.0], mask_rows, sun.w, sun.h);
            let mask_t = load("setup_ps1038", frame, "_17104");
            let mut mask_q = lightmap::passdiff::Buf::new(sun.w, sun.h, 1);
            {
                println!("D. PS 1038 (eid 791): sun_direct alpha → 17104 R8_UNORM, per UNORM8 store rounding:");
                let mut best: Option<(UnormRounding, lightmap::gpucmp::Report, lightmap::passdiff::Buf)> = None;
                for mode in unorm_modes {
                    let q = lightmap::ilightin::quantise_unorm8(&mask_f, mode);
                    let r = compare(&q, &mask_t, 1, Fmt::Unorm8);
                    println!("  {mode:?}: {}", r.line());
                    if best.as_ref().map_or(true, |(_, b, _)| r.exact > b.exact) { best = Some((mode, r, q)); }
                }
                let (mode, r, q) = best.unwrap();
                println!("  → best {mode:?}: {} exact of {}", r.exact, r.values);
                if show > 0 { lightmap::gpucmp::print_diffs(&q, &mask_t, 1, show); }
                all_closed &= r.closed();
                mask_q = q;
            }
            // E. eids 812 + 836: PS 17043 blended One/One onto the zeroed 17095, then PS 1109 DstCol/Zero with the MDiffuse 16969
            let mdiff_raw = load("setup_ps17043", frame, "_16969");
            // the SRV of 16969 is an _SRGB view: `ld` returns the decoded (linear) value of each byte
            let mut mdiff = mdiff_raw.clone();
            for y in 0..mdiff.h { for x in 0..mdiff.w { for k in 0..3 { mdiff.set(x, y, k, lightmap::gpufmt::srgb_to_linear(mdiff_raw.get(x, y, k))); } } }
            let il_t = load("setup_ps1109", frame, "_17095");
            let mut il_q = lightmap::passdiff::Buf::new(sun.w, sun.h, 3);
            {
                println!("E. PS 17043 (eid 812, One/One onto zeros) then PS 1109 (eid 836, DstCol/Zero × 16969) → 17095 R11G11B10, per store rounding (resolve store, product store):");
                if a.iter().any(|x| x == "--fit-srgb") {
                    // the sRGB decode table this GPU applies to the 16969 bytes, bounded from the data: every texel with a non-zero
                    // R11-truncated resolve value d and a captured product q gives q ≤ d·L(byte) < q + quantum(q), i.e.
                    // L(byte) ∈ [q/d, (q + quantum)/d); the intersection over the texels sharing a byte value is printed against the
                    // formula's value (an empty intersection would refute the product-then-truncate model)
                    let res = lightmap::ilightin::resolve_ps17043(&sun, false);
                    let stage = lightmap::ilightin::quantise_r11(&res, Rounding::Truncate);
                    let mut lo = vec![[0.0f64; 3]; 256];
                    let mut hi = vec![[f64::INFINITY; 3]; 256];
                    let mut cnt = vec![[0usize; 3]; 256];
                    for y in 0..sun.h { for x in 0..sun.w { for k in 0..3u32 {
                        let d = stage.get(x, y, k) as f64;
                        if d <= 0.0 { continue; }
                        let q = il_t.get(x, y, k);
                        let mb = if k == 2 { 5 } else { 6 };
                        let e = lightmap::gpufmt::encode_unsigned(q, mb, Rounding::Truncate);
                        let qn = lightmap::gpufmt::decode_unsigned(e + 1, mb) as f64;
                        let b = (mdiff_raw.get(x, y, k) * 255.0).round() as usize;
                        let (l, h) = (q as f64 / d, qn / d);
                        if l > lo[b][k as usize] { lo[b][k as usize] = l; }
                        if h < hi[b][k as usize] { hi[b][k as usize] = h; }
                        cnt[b][k as usize] += 1;
                    } } }
                    println!("  sRGB decode table bounds from the capture (byte: formula | [lo, hi) per channel R G B | texels):");
                    let mut inside = 0; let mut outside = 0; let mut empty = 0;
                    for b in 0..256 {
                        let formula = lightmap::gpufmt::srgb_to_linear(b as f32 / 255.0) as f64;
                        let mut line = format!("    {b:3}: {formula:.9}");
                        let mut any = false;
                        for k in 0..3 {
                            if cnt[b][k] == 0 { line.push_str(" | –"); continue; }
                            any = true;
                            let (l, h) = (lo[b][k], hi[b][k]);
                            let tag = if l >= h { empty += 1; "EMPTY" } else if formula >= l && formula < h { inside += 1; "ok" } else { outside += 1; if formula < l { "formula LOW" } else { "formula HIGH" } };
                            line.push_str(&format!(" | [{l:.9}, {h:.9}) {tag} ×{}", cnt[b][k]));
                        }
                        if any { println!("{line}"); }
                    }
                    println!("  → formula inside the interval: {inside}, outside: {outside}, empty intersections: {empty}");
                    // UNORM-n tables (k / (2^n − 1)): the formula rounded to the nearest step, and — rounding aside — whether ANY step
                    // of the table falls inside every interval (a table of that precision can reproduce the capture)
                    for n in 8..=16u32 {
                        let s = ((1u64 << n) - 1) as f64;
                        let (mut ins, mut outs, mut any_ok, mut any_no) = (0, 0, 0, 0);
                        for b in 0..256 { for k in 0..3 {
                            if cnt[b][k] == 0 || lo[b][k] >= hi[b][k] { continue; }
                            let v = (lightmap::gpufmt::srgb_to_linear(b as f32 / 255.0) as f64 * s).round() / s;
                            if v >= lo[b][k] && v < hi[b][k] { ins += 1 } else { outs += 1 }
                            // any k/s in [lo, hi)?
                            let kmin = (lo[b][k] * s).ceil();
                            if kmin / s < hi[b][k] { any_ok += 1 } else { any_no += 1 }
                        } }
                        println!("    candidate UNORM{n} (k/{s}): formula rounded inside {ins}, outside {outs}; some step inside {any_ok}, none {any_no}");
                    }
                    // candidate precisions of the decode table: the formula's value rounded to m mantissa bits (RTNE / RTZ) or to
                    // 2^−k fixed point — how many observed (byte, channel) intervals each candidate lands in
                    let round_mant = |v: f64, m: u32, rtz: bool| -> f64 { if v <= 0.0 { return 0.0; } let e = v.log2().floor(); let q = (2f64).powf(e - m as f64); let n = v / q; let n = if rtz { n.floor() } else { n.round() }; n * q };
                    let mut cands: Vec<(String, Box<dyn Fn(f64) -> f64>)> = Vec::new();
                    for m in 6..=16u32 { cands.push((format!("{m}-bit mantissa RTNE"), Box::new(move |v| round_mant(v, m, false)))); cands.push((format!("{m}-bit mantissa RTZ"), Box::new(move |v| round_mant(v, m, true)))); }
                    for k in 8..=16u32 { let s = (1u64 << k) as f64; cands.push((format!("fixed 2^-{k} RTNE"), Box::new(move |v| (v * s).round() / s))); cands.push((format!("fixed 2^-{k} RTZ"), Box::new(move |v| (v * s).floor() / s))); }
                    for (name, fun) in &cands {
                        let (mut ins, mut outs) = (0, 0);
                        for b in 0..256 { for k in 0..3 { if cnt[b][k] == 0 || lo[b][k] >= hi[b][k] { continue; } let v = fun(lightmap::gpufmt::srgb_to_linear(b as f32 / 255.0) as f64); if v >= lo[b][k] && v < hi[b][k] { ins += 1 } else { outs += 1 } } }
                        println!("    candidate {name}: inside {ins}, outside {outs}");
                    }
                }
                if let Some(t) = f("--debug-e") {
                    let (dx, dy) = t.split_once(',').expect("--debug-e X,Y");
                    let (tx, ty): (u32, u32) = (dx.parse().unwrap(), dy.parse().unwrap());
                    let s = [sun.get(tx, ty, 0), sun.get(tx, ty, 1), sun.get(tx, ty, 2), sun.get(tx, ty, 3)];
                    let res = lightmap::ilightin::resolve_ps17043_texel(&sun, tx, ty, false);
                    let q = lightmap::gpufmt::quantise_r11g11b10([res[0], res[1], res[2]], Rounding::Truncate);
                    let bytes = [(mdiff_raw.get(tx, ty, 0) * 255.0).round(), (mdiff_raw.get(tx, ty, 1) * 255.0).round(), (mdiff_raw.get(tx, ty, 2) * 255.0).round()];
                    let lin = [mdiff.get(tx, ty, 0), mdiff.get(tx, ty, 1), mdiff.get(tx, ty, 2)];
                    let prod = [q[0] * lin[0], q[1] * lin[1], q[2] * lin[2]];
                    let pq = lightmap::gpufmt::quantise_r11g11b10(prod, Rounding::Truncate);
                    println!("  texel ({tx}, {ty}): sun_direct {s:?}; resolve rgb/a {res:?}; R11 trunc {q:?}; albedo bytes {bytes:?} → linear {lin:?}; product {prod:?} → R11 trunc {pq:?}; captured 17095 [{}, {}, {}]", il_t.get(tx, ty, 0), il_t.get(tx, ty, 1), il_t.get(tx, ty, 2));
                    // the R11 quantum boundaries around the product
                    for k in 0..3 {
                        let mb = if k == 2 { 5 } else { 6 };
                        let e = lightmap::gpufmt::encode_unsigned(prod[k], mb, Rounding::Truncate);
                        println!("    ch {k}: product {:.9} sits between quanta {:.9} and {:.9}; captured/product − 1 = {:+.3e}", prod[k], lightmap::gpufmt::decode_unsigned(e, mb), lightmap::gpufmt::decode_unsigned(e + 1, mb), il_t.get(tx, ty, k as u32) / prod[k] - 1.0);
                    }
                }
                let res = lightmap::ilightin::resolve_ps17043(&sun, false);
                let mut best: Option<((Rounding, Rounding), lightmap::gpucmp::Report, lightmap::passdiff::Buf)> = None;
                for r1 in [Rounding::Truncate, Rounding::NearestEven] {
                    let stage = lightmap::ilightin::quantise_r11(&res, r1);
                    let prod = lightmap::finalprep::multiply_ps1109(&stage, &mdiff, [1.0, 1.0, 1.0, 1.0]);
                    for r2 in [Rounding::Truncate, Rounding::NearestEven] {
                        let q = lightmap::ilightin::quantise_r11(&prod, r2);
                        let r = compare(&q, &il_t, 3, Fmt::R11G11B10);
                        println!("  ({r1:?}, {r2:?}): {}", r.line());
                        if best.as_ref().map_or(true, |(_, b, _)| r.exact > b.exact) { best = Some(((r1, r2), r, q)); }
                    }
                }
                let (modes, r, q) = best.unwrap();
                println!("  → best {modes:?}: {} exact of {}", r.exact, r.values);
                if show > 0 { lightmap::gpucmp::print_diffs(&q, &il_t, 3, show); }
                all_closed &= r.closed();
                il_q = q;
            }
            // F. eids 857–927: PS 1335 × 8 from the captured (17095, 17104): each pass against its banked MRTs, chained (our previous
            //    output feeds the next pass) and stepwise (the captured previous pass feeds ours)
            {
                println!("F. PS 1335 × 8 (colour R11G11B10 store RTZ, coverage R8):");
                if let Some(t) = f("--debug-texel") {
                    // the 3×3 neighbourhood of one texel at pass 1: captured colour + coverage inputs, our sum / weight / quotient, the captured output
                    let (dx, dy) = t.split_once(',').expect("--debug-texel X,Y");
                    let (tx, ty): (i64, i64) = (dx.parse().unwrap(), dy.parse().unwrap());
                    let first = m.passes.iter().filter(|e| e.pass == "setup_ps1335" && e.frame == Some(frame) && e.file.contains("_rt0_")).min_by_key(|e| e.eid_last.unwrap_or(u64::MAX)).unwrap();
                    let out1 = lightmap::passdiff::load_entry(&root, first).unwrap();
                    println!("  texel ({tx}, {ty}) before pass 1 (captured 17095 / 17104) and its neighbours:");
                    for ddy in -1..=1i64 { for ddx in -1..=1i64 {
                        let (x, y) = (tx + ddx, ty + ddy);
                        if x < 0 || y < 0 || x >= il_t.w as i64 || y >= il_t.h as i64 { continue; }
                        let (x, y) = (x as u32, y as u32);
                        println!("    ({ddx:+},{ddy:+}) colour [{:.6}, {:.6}, {:.6}] coverage {:.6} (= {}/255)", il_t.get(x, y, 0), il_t.get(x, y, 1), il_t.get(x, y, 2), mask_t.get(x, y, 0), (mask_t.get(x, y, 0) * 255.0).round());
                    } }
                    let (c, w) = lightmap::ilightin::dilate_ps1335_texel(&il_t, &mask_t, tx as u32, ty as u32, false);
                    let (cf, _) = lightmap::ilightin::dilate_ps1335_texel(&il_t, &mask_t, tx as u32, ty as u32, true);
                    println!("    ours f32 [{:.7}, {:.7}, {:.7}] (fma [{:.7}, {:.7}, {:.7}]) coverage {w}; captured after pass 1 [{:.6}, {:.6}, {:.6}]", c[0], c[1], c[2], cf[0], cf[1], cf[2], out1.get(tx as u32, ty as u32, 0), out1.get(tx as u32, ty as u32, 1), out1.get(tx as u32, ty as u32, 2));
                }
                let snaps: Vec<&lightmap::passdump::Entry> = m.passes.iter().filter(|e| e.pass == "setup_ps1335" && e.frame == Some(frame)).collect();
                let mut cur_c = il_t.clone();
                let mut cur_w = mask_t.clone();
                let (mut step_c, mut step_w) = (il_t.clone(), mask_t.clone());
                let mut eids: Vec<u64> = snaps.iter().filter_map(|e| e.eid_last).collect();
                eids.sort();
                eids.dedup();
                // pass 1 is where the weights are fractional (the R8 coverage in ninths): the arithmetic model (fused mad, the
                // division as a × rcp(b)) decides the truncation of the many quotients that land on an R11G11B10 quantum
                if a.iter().any(|x| x == "--variants") {
                    let first = *eids.first().unwrap();
                    let tc = lightmap::passdiff::load_entry(&root, snaps.iter().find(|e| e.eid_last == Some(first) && e.file.contains("_rt0_")).unwrap()).unwrap();
                    for (fm, dm) in [(false, lightmap::gpucmp::DivModel::Ieee), (true, lightmap::gpucmp::DivModel::Ieee), (false, lightmap::gpucmp::DivModel::MulRcp), (true, lightmap::gpucmp::DivModel::MulRcp)] {
                        let (oc, _) = lightmap::ilightin::dilate_ps1335_div(&il_t, &mask_t, fm, dm);
                        for st in [Rounding::Truncate, Rounding::NearestEven] {
                            let q = lightmap::ilightin::quantise_r11(&oc, st);
                            let r = compare(&q, &tc, 3, Fmt::R11G11B10);
                            println!("  pass 1 arithmetic model fma {fm} div {dm:?} store {st:?}: {} exact of {}, {} within 1 quantum, {} beyond", r.exact, r.values, r.ulp1, r.beyond);
                        }
                    }
                }
                let t0 = std::time::Instant::now();
                for (k, eid) in eids.iter().enumerate() {
                    let rt0 = snaps.iter().find(|e| e.eid_last == Some(*eid) && e.file.contains("_rt0_")).expect("rt0");
                    let rt1 = snaps.iter().find(|e| e.eid_last == Some(*eid) && e.file.contains("_rt1_")).expect("rt1");
                    let tc = lightmap::passdiff::load_entry(&root, rt0).unwrap_or_else(|e| panic!("{e}"));
                    let tw = lightmap::passdiff::load_entry(&root, rt1).unwrap_or_else(|e| panic!("{e}"));
                    // chained
                    let (oc, ow) = lightmap::ilightin::dilate_ps1335_div(&cur_c, &cur_w, fma, dm);
                    let oc = lightmap::ilightin::quantise_r11(&oc, Rounding::Truncate);
                    let ow = lightmap::ilightin::quantise_unorm8(&ow, UnormRounding::NearestEven);
                    let rc = compare(&oc, &tc, 3, Fmt::R11G11B10);
                    let rw = compare(&ow, &tw, 1, Fmt::Unorm8);
                    // stepwise: from the captured previous
                    let (sc, sw) = lightmap::ilightin::dilate_ps1335_div(&step_c, &step_w, fma, dm);
                    let sc = lightmap::ilightin::quantise_r11(&sc, Rounding::Truncate);
                    let sw = lightmap::ilightin::quantise_unorm8(&sw, UnormRounding::NearestEven);
                    let rsc = compare(&sc, &tc, 3, Fmt::R11G11B10);
                    let rsw = compare(&sw, &tw, 1, Fmt::Unorm8);
                    let covered = (0..tw.h).flat_map(|y| (0..tw.w).map(move |x| (x, y))).filter(|&(x, y)| tw.get(x, y, 0) > 0.0).count();
                    println!("  pass {} (eid {eid}) colour chained: {}", k + 1, rc.line());
                    println!("           colour stepwise: {}", rsc.line());
                    println!("           coverage chained: {} | stepwise: {} exact of {} ({covered} texels covered after)", rw.line(), rsw.exact, rsw.values);
                    if show > 0 && !rc.closed() { lightmap::gpucmp::print_diffs(&oc, &tc, 3, show); }
                    all_closed &= rc.closed() && rw.closed();
                    cur_c = oc;
                    cur_w = ow;
                    step_c = tc;
                    step_w = tw;
                }
                println!("  8 passes in {:.1} s", t0.elapsed().as_secs_f32());
                // the whole chain from our own D/E outputs
                let (mut cc, mut cw) = (il_q.clone(), mask_q.clone());
                for _ in 0..eids.len() {
                    let (oc, ow) = lightmap::ilightin::dilate_ps1335_div(&cc, &cw, fma, dm);
                    cc = lightmap::ilightin::quantise_r11(&oc, Rounding::Truncate);
                    cw = lightmap::ilightin::quantise_unorm8(&ow, UnormRounding::NearestEven);
                }
                let last = *eids.last().unwrap();
                let tc = lightmap::passdiff::load_entry(&root, snaps.iter().find(|e| e.eid_last == Some(last) && e.file.contains("_rt0_")).unwrap()).unwrap();
                let tw = lightmap::passdiff::load_entry(&root, snaps.iter().find(|e| e.eid_last == Some(last) && e.file.contains("_rt1_")).unwrap()).unwrap();
                let rc = compare(&cc, &tc, 3, Fmt::R11G11B10);
                let rw = compare(&cw, &tw, 1, Fmt::Unorm8);
                println!("  whole chain from sun_direct + 16969 through our D, E and 8 × F vs the captured last pass: colour {} | coverage {} exact of {}", rc.line(), rw.exact, rw.values);
                all_closed &= rc.closed() && rw.closed();
            }
            println!("ROW 4 {}", if all_closed { "CLOSED: every value bit-identical or within one quantum of its target" } else { "NOT closed (values beyond one quantum above)" });
        }
        "shadow-check" => {
            // lmtool shadow-check PASSCAP_ROOT [--frame N] [--arith fma|separate|both] [--unorm nearest|truncate|both]
            //   [--no-alpha-test] [--dump OUT.dds] [--worst N]
            //   the transcribed SUN SHADOW MAP (shadowmap.rs: VS 5394 / 1142 / 14613 + PS 1147, the D3D11 rasteriser with
            //   the D16 depth bias) run from the capture's own inputs (the casters' vertex/index buffers, the instance
            //   remap + static-mesh table banked per draw, the alpha textures, the seven draws' cbuffers and state) and
            //   compared 16-bit value for 16-bit value with the captured depth target; the post-VS positions are checked
            //   bit for bit against the captured vsout of every draw first
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127448);
            let env = root.join(format!("env/frame{frame}"));
            // --manifest FILE: a frozen copy (the capture's MANIFEST.json changes while the baker banks)
            let mtxt = std::fs::read_to_string(f("--manifest").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"))).expect("MANIFEST.json");
            let m = lightmap::passdiff::read_manifest(&mtxt).expect("manifest");
            let mval: serde_json::Value = serde_json::from_str(&lightmap::passdiff::repair_truncated_json(&mtxt)).expect("manifest json");
            let shadow_ent = mval["passes"].as_array().unwrap().iter().find(|e| e["pass"].as_str() == Some("sun_shadow") && e["frame"].as_u64() == Some(frame as u64)).expect("a sun_shadow entry for the frame");
            let (eid_first, eid_last) = (shadow_ent["eid_first"].as_u64().unwrap_or(0), shadow_ent["eid_last"].as_u64().unwrap_or(u64::MAX));
            let mesh_json: serde_json::Value = serde_json::from_str(&lightmap::passdiff::repair_truncated_json(&std::fs::read_to_string(env.join("mesh.json")).expect("mesh.json"))).expect("mesh.json");
            let samplers: serde_json::Value = std::fs::read_to_string(env.join("samplers.json")).ok().and_then(|t| serde_json::from_str(&lightmap::passdiff::repair_truncated_json(&t)).ok()).unwrap_or(serde_json::Value::Null);
            let draws_bytes = lightmap::passdiff::read_entry_bytes(&root, &format!("logs/draws-frame{frame}.json")).expect("draws log");
            let draws: serde_json::Value = serde_json::from_slice(&draws_bytes).expect("draws json");
            let pass_draws: Vec<&serde_json::Value> = draws.as_array().unwrap().iter().filter(|e| { let eid = e["eid"].as_u64().unwrap_or(0); eid >= eid_first && eid <= eid_last && e["flags"].as_str().map(|s| s.contains("Drawcall")).unwrap_or(false) }).collect();
            println!("sun shadow pass: eids {eid_first}–{eid_last}, {} draws", pass_draws.len());
            let m4 = |v: &serde_json::Value| -> [[f32; 4]; 4] { let mut o = [[0f32; 4]; 4]; for i in 0..4 { for j in 0..4 { o[i][j] = v[i][j].as_f64().unwrap() as f32; } } o };
            let mut cam: Option<lightmap::shadowmap::LightCamera> = None;
            let mut state: Option<lightmap::shadowmap::RasterState> = None;
            let mut casters: Vec<lightmap::shadowmap::CasterDraw> = Vec::new();
            for e in &pass_draws {
                let eid = e["eid"].as_u64().unwrap();
                let vs = e["Vertex"]["shader"].as_str().unwrap_or("?");
                let ps = e["Pixel"]["shader"].as_str().unwrap_or("?");
                let scene_v = &e["Vertex"]["cbuffers"]["SceneV"];
                let c = lightmap::shadowmap::LightCamera { world_pr_camera: m4(&scene_v["GbxV_WorldPrCamera"]) };
                if let Some(c0) = &cam { if c0.world_pr_camera != c.world_pr_camera { println!("  NOTE: eid {eid} has a different GbxV_WorldPrCamera"); } }
                cam.get_or_insert(c);
                let r = &e["raster"];
                let vp = e["viewport"].as_array().map(|v| { let mut o = [0f32; 6]; for (i, x) in v.iter().take(6).enumerate() { o[i] = x.as_f64().unwrap_or(0.0) as f32; } o }).unwrap_or([0.0, 0.0, 4096.0, 4096.0, 0.0, 1.0]);
                let st = lightmap::shadowmap::RasterState { viewport: vp, depth_bias: r["depthBias"].as_i64().unwrap_or(0) as i32, slope_scaled_depth_bias: r["slopeScaledDepthBias"].as_f64().unwrap_or(0.0) as f32, depth_bias_clamp: r["depthBiasClamp"].as_f64().unwrap_or(0.0) as f32, cull_back: r["cull"].as_str() == Some("CullMode.Back"), front_ccw: r["frontCCW"].as_bool().unwrap_or(true), depth_clip: r["depthClip"].as_bool().unwrap_or(true), plane: match f("--plane").as_deref() { Some("unsnapped") => lightmap::shadowmap::PlaneEval::F64Unsnapped, Some("f32a") => lightmap::shadowmap::PlaneEval::F32VertexA, Some("f32o") => lightmap::shadowmap::PlaneEval::F32Origin, Some("f32b") => lightmap::shadowmap::PlaneEval::F32Bbox, Some("f32co") => lightmap::shadowmap::PlaneEval::F32CoefOrigin, Some("f32ca") => lightmap::shadowmap::PlaneEval::F32CoefVertexA, Some("f32cb") => lightmap::shadowmap::PlaneEval::F32CoefBbox, Some("fixfloor") => lightmap::shadowmap::PlaneEval::FixedCoefFloor, Some("fixtrunc") => lightmap::shadowmap::PlaneEval::FixedCoefTrunc, Some("fixround") => lightmap::shadowmap::PlaneEval::FixedCoefRound, Some("fixall") => lightmap::shadowmap::PlaneEval::FixedAllRound, Some("bary") => lightmap::shadowmap::PlaneEval::BaryFixed, _ => lightmap::shadowmap::PlaneEval::F64Snapped }, coef_bits: f("--coef-bits").map(|v| v.parse().unwrap()).unwrap_or(36), vertex_z_bits: f("--vertex-z-bits").map(|v| v.parse().unwrap()).unwrap_or(0) };
                if let Some(s0) = &state { if format!("{s0:?}") != format!("{st:?}") { println!("  NOTE: eid {eid} has a different raster state: {st:?}"); } }
                state.get_or_insert(st);
                if e["depthstate"]["func"].as_str() != Some("CompareFunction.Greater") { println!("  NOTE: eid {eid} depth func {:?}", e["depthstate"]["func"]); }
                let drawv = &e["Vertex"]["cbuffers"]["DrawV"]["g_CBufferV_Draw"];
                let instance_start = drawv["InstanceStart"].as_u64().unwrap_or(0) as u32;
                let visual_to_world = drawv.get("VisualToWorld").and_then(|v| v.as_array()).map(|rows| { let mut o = [[0f32; 3]; 4]; for i in 0..4 { for j in 0..3 { o[i][j] = rows[i][j].as_f64().unwrap_or(0.0) as f32; } } o });
                let rec = mesh_json.as_array().unwrap().iter().find(|r| r["eid"].as_u64() == Some(eid)).unwrap_or_else(|| panic!("mesh.json has no eid {eid} (ask the baker for the mesh export)"));
                let vbs = rec["vertex_buffers"].as_array().unwrap();
                let vb = std::fs::read(env.join("mesh").join(vbs[0]["file"].as_str().unwrap())).unwrap_or_else(|err| panic!("vb of eid {eid}: {err}"));
                let stride = vbs[0]["stride"].as_u64().unwrap() as usize;
                let il = rec["input_layout"].as_array().unwrap();
                let find = |sem: &str, idx: u64| il.iter().find(|x| x["semantic"].as_str() == Some(sem) && x["index"].as_u64() == Some(idx)).map(|x| x["offset"].as_u64().unwrap() as usize);
                let pos_off = find("POSITION", 0).expect("POSITION0");
                let uv_off = if vs == "14613" { find("TEXCOORD", 0) } else { None };
                let ib = std::fs::read(env.join("mesh").join(rec["vsout"]["index_file"].as_str().expect("index file"))).expect("indices");
                let mesh = lightmap::shadowmap::CasterMesh::parse(&vb, stride, pos_off, uv_off, &ib);
                // the post-VS positions: the output vertex is SV_Position (+ TEXCOORD0 for VS 14613) → the stride from the record
                let vs_stride = rec["vsout"]["vertexByteStride"].as_u64().unwrap_or(16).max(16) as usize;
                let vsout = std::fs::read(env.join("mesh").join(rec["vsout"]["file"].as_str().unwrap_or(""))).ok().map(|b| b.chunks_exact(vs_stride).map(|c| [f32::from_le_bytes(c[0..4].try_into().unwrap()), f32::from_le_bytes(c[4..8].try_into().unwrap()), f32::from_le_bytes(c[8..12].try_into().unwrap()), f32::from_le_bytes(c[12..16].try_into().unwrap())]).collect::<Vec<_>>());
                let dyna = std::fs::read(env.join("bufs").join(format!("e{eid:06}_Vertex_srv0_2185.bin"))).unwrap_or_default();
                let sm = std::fs::read(env.join("bufs").join(format!("e{eid:06}_Vertex_srv1_17163.bin"))).unwrap_or_default();
                if dyna.is_empty() && instance_start != 0xffff_ffff { println!("  WARNING: eid {eid}: no g_Buf_DynaU32s snapshot banked (bufs/e{eid:06}_Vertex_srv0_2185.bin)"); }
                let tables = lightmap::shadowmap::InstanceTables::parse(&dyna, &sm);
                let mut alpha = None;
                if ps == "1147" {
                    let thr = e["Pixel"]["cbuffers"]["ShaderP"]["GbxShadowAlphaThreshold"].as_f64().expect("GbxShadowAlphaThreshold") as f32;
                    let tex_id = e["Pixel"]["srvs"][0]["tex"]["id"].as_str().expect("PS srv0");
                    // the texture file: textures.json maps the id to its file (exported under the first eid that bound it)
                    let textures: serde_json::Value = std::fs::read_to_string(env.join("textures.json")).ok().and_then(|t| serde_json::from_str(&lightmap::passdiff::repair_truncated_json(&t)).ok()).unwrap_or(serde_json::Value::Null);
                    let tex_file = textures.as_array().and_then(|arr| arr.iter().find(|t| t["id"].as_u64().map(|i| i.to_string()) == Some(tex_id.to_string()) || t["id"].as_str() == Some(tex_id))).and_then(|t| t["file"].as_str().map(|s| s.to_string())).unwrap_or_else(|| format!("e{eid:06}_{tex_id}.dds"));
                    let tex_bytes = lightmap::passdiff::read_entry_bytes(&root, &format!("env/frame{frame}/textures/{tex_file}")).unwrap_or_else(|err| panic!("alpha texture of eid {eid}: {err}"));
                    let texture = lightmap::shadowmap::AlphaTexture::from_dds(&tex_bytes).unwrap_or_else(|err| panic!("alpha texture of eid {eid}: {err}"));
                    let aniso = samplers.as_array().and_then(|arr| arr.iter().find(|s| s["eid"].as_u64() == Some(eid))).and_then(|s| s["stages"]["Pixel"][0]["maxAnisotropy"].as_f64()).unwrap_or(16.0) as f32;
                    println!("  eid {eid}: alpha test threshold {thr} on texture {tex_id} ({}×{}, {} mips), anisotropy {aniso}", texture.w, texture.h, texture.mips.len());
                    alpha = Some(lightmap::shadowmap::AlphaTest { threshold: thr, texture, max_anisotropy: aniso });
                }
                let inst = e["inst"].as_u64().unwrap_or(0).max(1) as u32;
                let idx0 = tables.index(instance_start, 0);
                // where the instance's origin lands on the target (a single-instance draw)
                let origin_px = if inst == 1 && visual_to_world.is_none() {
                    tables.rows(instance_start, 0, lightmap::shadowmap::Arith::Fma).map(|rows| {
                        let cl = lightmap::shadowmap::vs_static_mesh([0.0, 0.0, 0.0], &rows, &c, lightmap::shadowmap::Arith::Fma);
                        format!(", origin ({:.1}, {:.1}, {:.1}) → pixel ({:.1}, {:.1}) z01 {:.4}", rows[0][3], rows[1][3], rows[2][3], (cl[0] * 0.5 + 0.5) * vp[2] + vp[0], (0.5 - cl[1] * 0.5) * vp[3] + vp[1], cl[2])
                    }).unwrap_or_default()
                } else { String::new() };
                println!("  eid {eid}: VS {vs} PS {ps}, {} vertices, {} indices, {} instance(s), InstanceStart {}{}{}{origin_px}", mesh.pos.len(), mesh.indices.len(), inst, instance_start as i32, idx0.map(|i| format!(" → static mesh {i}")).unwrap_or_default(), visual_to_world.map(|_| " (VisualToWorld)").unwrap_or(""));
                casters.push(lightmap::shadowmap::CasterDraw { eid, mesh, instance_start, instance_count: inst, visual_to_world, tables, alpha, vsout });
            }
            let cam = cam.expect("a camera");
            let st = state.expect("a raster state");
            println!("camera GbxV_WorldPrCamera rows {:?}", cam.world_pr_camera);
            println!("raster state {st:?}");
            // 1. the vertex shader against the captured post-VS positions, per arithmetic model
            let ariths: Vec<lightmap::shadowmap::Arith> = match f("--arith").as_deref() { Some("fma") => vec![lightmap::shadowmap::Arith::Fma], Some("separate") => vec![lightmap::shadowmap::Arith::Separate], _ => vec![lightmap::shadowmap::Arith::Fma, lightmap::shadowmap::Arith::Separate] };
            let mut best = (ariths[0], 0usize);
            for ar in &ariths {
                let mut tot = (0, 0, 0, 0, 0f32);
                for d in &casters {
                    if let Some((n, same, ulp1, worse, maxd)) = lightmap::shadowmap::check_vsout(d, &cam, *ar) {
                        println!("  VS {ar:?} eid {}: {same} of {n} clip components bit-identical, {ulp1} within 1 ulp, {worse} worse (max |Δ| {maxd:.3e})", d.eid);
                        tot.0 += n; tot.1 += same; tot.2 += ulp1; tot.3 += worse; tot.4 = tot.4.max(maxd);
                    }
                }
                println!("VS {ar:?}: {} of {} bit-identical, {} within 1 ulp, {} worse (max |Δ| {:.3e})", tot.1, tot.0, tot.2, tot.3, tot.4);
                if tot.1 > best.1 { best = (*ar, tot.1); }
            }
            let arith = best.0;
            // 2. the pass, per UNORM rounding model
            let unorms: Vec<lightmap::shadowmap::UnormRounding> = match f("--unorm").as_deref() { Some("truncate") => vec![lightmap::shadowmap::UnormRounding::Truncate], Some("nearest") => vec![lightmap::shadowmap::UnormRounding::Nearest], _ => vec![lightmap::shadowmap::UnormRounding::Nearest, lightmap::shadowmap::UnormRounding::Truncate] };
            let game = lightmap::passdiff::load_entry(&root, m.passes.iter().find(|e| e.pass == "sun_shadow" && e.frame == Some(frame)).expect("sun_shadow entry")).expect("captured shadow map");
            println!("captured D16 {}×{}", game.w, game.h);
            let worst_n: usize = f("--worst").map(|v| v.parse().unwrap()).unwrap_or(0);
            let mut best_tgt: Option<(lightmap::shadowmap::ShadowTarget, usize)> = None;
            for un in &unorms {
                let o = lightmap::shadowmap::RunOpts { arith, unorm: *un, alpha_test: !has("--no-alpha-test"), depth_fixed_bits: f("--depth-fixed-bits").map(|v| v.parse().unwrap()).unwrap_or(0), fixed_before_bias: has("--fixed-before-bias"), step_fixed_k: f("--fixed-k").map(|v| v.parse().unwrap()).unwrap_or(20), bias_round: f("--bias-round").map(|v| v.parse().unwrap()).unwrap_or(3), scale_ulps: f("--scale-ulps").map(|v| v.parse().unwrap()).unwrap_or(0.0) };
                let mut tgt = lightmap::shadowmap::ShadowTarget::new(game.w, game.h);
                let t0 = std::time::Instant::now();
                for (k, d) in casters.iter().enumerate() {
                    let s = lightmap::shadowmap::draw_caster(d, &cam, &st, &mut tgt, (k + 1) as u8, &o);
                    println!("  {un:?} eid {}: {} triangles ({} culled), {} fragments, {} depth-clipped, {} alpha-discarded, {} depth writes", d.eid, s.triangles, s.culled, s.fragments, s.depth_clipped, s.alpha_discarded, s.depth_passed);
                }
                println!("{un:?}: rasterised in {:.1} s", t0.elapsed().as_secs_f32());
                let cs = lightmap::shadowmap::compare_d16(&tgt, &game, casters.len());
                let written = cs.texels - cs.both_clear;
                println!("{arith:?}/{un:?} vs the captured D16: {} texels written on either side; {} bit-identical ({:.3} %), {} off by one 16-bit step ({:.3} %; ours = game + 1 for {} of them), {} further ({:.3} %); coverage: {} only ours, {} only game{}", written, cs.identical, 100.0 * cs.identical as f64 / written.max(1) as f64, cs.off_by_one, 100.0 * cs.off_by_one as f64 / written.max(1) as f64, cs.off_plus, cs.worse, 100.0 * cs.worse as f64 / written.max(1) as f64, cs.only_ours, cs.only_game, cs.worst.map(|(x, y, g, o)| format!("; worst texel ({x},{y}): game {g} ours {o} (Δ {} steps = {:.3} m of the 1224.7 m depth range)", o as i32 - g as i32, (o as i32 - g as i32).abs() as f32 / 65535.0 * 1224.67)).unwrap_or_default());
                for (k, d) in casters.iter().enumerate() {
                    let (n, same, one, worse, only_ours) = cs.per_tag[k + 1];
                    println!("    eid {}: {n} texels (ours last writer): {same} identical, {one} off by one, {worse} further, {only_ours} where the game has 0", d.eid);
                }
                let (n, same, one, worse, _) = cs.per_tag[0];
                println!("    game-only texels (ours 0): {n} → {same} identical, {one} off by one, {worse} further");
                if has("--frac-hist") {
                    // the mismatch fraction by the fragment's offset from its triangle's reference vertex (bins of 4 px in
                    // dx and dy, signed), for the largest draw
                    let big = casters.iter().enumerate().max_by_key(|(_, d)| d.instance_count as usize * d.mesh.indices.len()).map(|(k, _)| (k + 1) as u8).unwrap_or(1);
                    let mut by_dy = vec![(0usize, 0usize); 24];
                    let mut by_dx = vec![(0usize, 0usize); 24];
                    for y in 0..tgt.h { for x in 0..tgt.w { let i = (y * tgt.w + x) as usize; if tgt.source[i] != big { continue; } let g = (game.get(x, y, 0) * 65535.0 + 0.5) as i32; let o = tgt.depth[i] as i32; let bx = (((tgt.dxy[i][0] + 48.0) / 4.0) as i64).clamp(0, 23) as usize; let by = (((tgt.dxy[i][1] + 48.0) / 4.0) as i64).clamp(0, 23) as usize; by_dx[bx].0 += 1; by_dy[by].0 += 1; if o != g { by_dx[bx].1 += 1; by_dy[by].1 += 1; } } }
                    let pct = |v: &[(usize, usize)]| v.iter().map(|(n, m)| if *n > 0 { format!("{:.4}", *m as f64 / *n as f64) } else { "-".into() }).collect::<Vec<_>>().join(" ");
                    println!("    largest draw: mismatch fraction by dx from vertex a (−48..48 in 4 px): {}", pct(&by_dx));
                    println!("    largest draw: mismatch fraction by dy from vertex a (−48..48 in 4 px): {}", pct(&by_dy));
                    // the mismatch fraction over the screen (8 × 8 bands) for the two big draws — a growth with the
                    // distance from some origin would betray an incremental evaluation
                    for (k, d) in casters.iter().enumerate() {
                        if d.instance_count as usize * d.mesh.indices.len() < 500 { continue; }
                        let tag = (k + 1) as u8;
                        let mut grid = vec![(0usize, 0usize); 64];
                        for y in 0..tgt.h { for x in 0..tgt.w { let i = (y * tgt.w + x) as usize; if tgt.source[i] != tag { continue; } let g = (game.get(x, y, 0) * 65535.0 + 0.5) as i32; let o = tgt.depth[i] as i32; let b = ((y * 8 / tgt.h) * 8 + x * 8 / tgt.w) as usize; grid[b].0 += 1; if o != g { grid[b].1 += 1; } } }
                        println!("    eid {}: mismatch fraction over the screen (rows top→bottom, 8 columns each):", d.eid);
                        for r in 0..8 { println!("      {}", (0..8).map(|c| { let (n, m) = grid[r * 8 + c]; if n > 0 { format!("{:.4}", m as f64 / n as f64) } else { "  -   ".into() } }).collect::<Vec<_>>().join(" ")); }
                    }
                    // the transition band in fine bins (0.002) around 0.5 for the tiles (tag of the largest draw): sharp = a
                    // constant δ, gradual = a varying one
                    let big = casters.iter().enumerate().max_by_key(|(_, d)| d.instance_count as usize * d.mesh.indices.len()).map(|(k, _)| (k + 1) as u8).unwrap_or(1);
                    let mut same = vec![0usize; 60];
                    let mut plus = vec![0usize; 60];
                    for y in 0..tgt.h { for x in 0..tgt.w { let i = (y * tgt.w + x) as usize; if tgt.source[i] != big { continue; } let g = (game.get(x, y, 0) * 65535.0 + 0.5) as i32; let o = tgt.depth[i] as i32; let v = tgt.raw[i] as f64 * 65535.0; let fr = v - v.floor(); if fr < 0.47 || fr >= 0.59 { continue; } let b = ((fr - 0.47) / 0.002) as usize; if o == g { same[b.min(59)] += 1; } else if o == g + 1 { plus[b.min(59)] += 1; } } }
                    println!("    transition band (fraction 0.470..0.590 in 0.002 bins), P(ours = game + 1): {}", same.iter().zip(plus.iter()).map(|(s, p)| if s + p > 0 { format!("{:.2}", *p as f64 / (*s + *p) as f64) } else { "-".into() }).collect::<Vec<_>>().join(" "));
                    // δ per class: the mismatch fraction of a class with a constant GPU-vs-ours offset δ (in steps) IS δ (uniform
                    // fractions) — by draw and by depth decile, and by the slope term of the fragment (bins of 2 steps)
                    for (k, d) in casters.iter().enumerate() {
                        let tag = (k + 1) as u8;
                        let mut by_z = vec![(0usize, 0usize); 10];
                        let mut by_slope = vec![(0usize, 0usize); 40];
                        for y in 0..tgt.h { for x in 0..tgt.w { let i = (y * tgt.w + x) as usize; if tgt.source[i] != tag { continue; } let g = (game.get(x, y, 0) * 65535.0 + 0.5) as i32; let o = tgt.depth[i] as i32; let zb = ((o as f64 / 65535.0) * 10.0) as usize; let sb = ((tgt.slope[i] * 65535.0 / 2.0) as usize).min(39); by_z[zb.min(9)].0 += 1; by_slope[sb].0 += 1; if o != g { by_z[zb.min(9)].1 += 1; by_slope[sb].1 += 1; } } }
                        let pct = |v: &[(usize, usize)]| v.iter().map(|(n, m)| if *n > 0 { format!("{:.4}", *m as f64 / *n as f64) } else { "-".into() }).collect::<Vec<_>>().join(" ");
                        println!("    eid {}: mismatch fraction by depth decile: {}", d.eid, pct(&by_z));
                        println!("    eid {}: by slope term (bins of 2 steps/px): {}", d.eid, pct(&by_slope));
                    }
                    // where in the quantisation interval do the disagreements sit? the fraction of raw·65535 in 20 bins,
                    // for the identical texels and for the off-by-one ones (ours = game + 1 / − 1)
                    let mut same = [0usize; 20];
                    let mut plus = [0usize; 20];
                    let mut minus = [0usize; 20];
                    for y in 0..tgt.h { for x in 0..tgt.w { let i = (y * tgt.w + x) as usize; if tgt.depth[i] == 0 { continue; } let g = (game.get(x, y, 0) * 65535.0 + 0.5) as i32; let o = tgt.depth[i] as i32; let v = tgt.raw[i] as f64 * 65535.0; let fr = v - v.floor(); let b = ((fr * 20.0) as usize).min(19); if o == g { same[b] += 1; } else if o == g + 1 { plus[b] += 1; } else if o == g - 1 { minus[b] += 1; } } }
                    println!("    fraction of raw·65535 (bins of 0.05): identical {:?}", same);
                    println!("    ours = game + 1: {:?}", plus);
                    println!("    ours = game − 1: {:?}", minus);
                }
                if worst_n > 0 {
                    // the largest differences, with what each side has
                    let mut diffs: Vec<(i32, u32, u32)> = Vec::new();
                    for y in 0..tgt.h { for x in 0..tgt.w { let o = tgt.depth[(y * tgt.w + x) as usize] as i32; let g = (game.get(x, y, 0) * 65535.0 + 0.5) as i32; if o != g { diffs.push(((o - g).abs(), x, y)); } } }
                    diffs.sort_by(|a, b| b.0.cmp(&a.0));
                    for (d, x, y) in diffs.iter().take(worst_n) { let i = (y * tgt.w + x) as usize; println!("      ({x},{y}): game {} ours {} (Δ {d}), ours written by eid {}", (game.get(*x, *y, 0) * 65535.0 + 0.5) as i32, tgt.depth[i], (tgt.source[i] as usize).checked_sub(1).and_then(|k| casters.get(k)).map(|c| c.eid.to_string()).unwrap_or_else(|| "none".into())); }
                }
                if best_tgt.as_ref().map(|b| cs.identical > b.1).unwrap_or(true) { best_tgt = Some((tgt, cs.identical)); }
            }
            if let Some(out) = f("--dump") {
                let (tgt, _) = best_tgt.as_ref().unwrap();
                std::fs::write(&out, tgt.to_dds()).expect("dump");
                println!("wrote {out} (DDS R16_UNORM {}×{})", tgt.w, tgt.h);
            }
        }
        "tiles" => {
            // lmtool tiles MAP.Gbx --s S [--scene xmin,ymin,zmin,xmax,ymax,zmax] [--quality Q] [--vram-mb N] [--max-tiles 4]
            //   [--global-quality G] [--lod0] [--no-half] [--size-override N]
            //   the lightmapper's CPU inputs of the peel cameras WITHOUT a capture (lmtiles.rs): per item the stored visual
            //   boxes, the model box (CPlugTree), the mobil Iso4 (RE 4's pose chain), the block record and its quality
            //   scale; the scene box (the record fold — or --scene, e.g. the captured casters' when the zone tiles are not
            //   items of the map) and the TILING RULE's target size, grid and fitted tiles at the alloc scale --s
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("map scene");
            let gq: f32 = f("--global-quality").map(|v| v.parse().unwrap()).unwrap_or(1.0);
            let items = lightmap::lmtiles::item_records(&scene, gq, has("--lod0"));
            let mut recs: Vec<lightmap::lmtiles::BlockRecord> = Vec::new();
            for it in &items {
                let inst = scene.instances.iter().find(|i| i.item == it.item).unwrap();
                println!("item {} ({}): pose yaw {} pitch {} roll {} pos {:?} pivot {:?} scale {} lm-quality byte {}", it.item, it.model_name, inst.pose.yaw, inst.pose.pitch, inst.pose.roll, inst.pose.pos, inst.pose.pivot, inst.pose.scale, inst.lm_quality);
                for (lod, c, h) in &scene.models[inst.model].stored_boxes_all { println!("    visual box (lod mask {lod}): c {:?} h {:?}", c, h); }
                println!("    Iso4 rows {:?} {:?} {:?} t {:?}", &it.iso4[0..3], &it.iso4[3..6], &it.iso4[6..9], &it.iso4[9..12]);
                match (&it.model_box, &it.record) {
                    (Some(mb), Some(r)) => { println!("    model box c {:?} h {:?} → RECORD c {:?} h {:?} (x [{}, {}] y [{}, {}] z [{}, {}]) quality {:.4}{}", mb.c, mb.h, r.world.c, r.world.h, r.world.min()[0], r.world.max()[0], r.world.min()[1], r.world.max()[1], r.world.min()[2], r.world.max()[2], r.quality, if lightmap::lmtiles::in_fitted_tiles(r) { "" } else { " (below the 0.51 gate)" }); recs.push(*r); }
                    _ => println!("    no stored bounding box"),
                }
            }
            let scene_ch = match f("--scene") {
                Some(s) => { let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect(); lightmap::lmtiles::CBox::from_min_max([v[0], v[1], v[2]], [v[3], v[4], v[5]]) }
                None => lightmap::lmtiles::scene_box(&recs),
            };
            println!("SCENE BOX c {:?} h {:?} = [{}, {}] × [{}, {}] × [{}, {}]{}", scene_ch.c, scene_ch.h, scene_ch.min()[0], scene_ch.max()[0], scene_ch.min()[1], scene_ch.max()[1], scene_ch.min()[2], scene_ch.max()[2], if has("--scene") { " (given)" } else { " (the records' fold — the zone tiles are missing when they are not items)" });
            let p = lightmap::lmtiles::TileParams {
                alloc_scale: f("--s").map(|v| v.parse().unwrap()).unwrap_or(31.75),
                quality: f("--quality").map(|v| v.parse().unwrap()).unwrap_or(3),
                half_at_low_quality: !has("--no-half"),
                size_override: f("--size-override").map(|v| v.parse().unwrap()).unwrap_or(0),
                vram_bytes: f("--vram-mb").map(|v| v.parse::<i64>().unwrap() << 20).unwrap_or(8 << 30),
                max_tiles: f("--max-tiles").map(|v| v.parse().unwrap()).unwrap_or(4),
            };
            let t = lightmap::lmtiles::peel_tiling(&scene_ch, &recs, &p);
            println!("TILING {p:?}: ext {:.2} layout units → peel target {}², n = {} cells per axis, {} fitted tile(s){}", t.ext, t.size, t.n, t.tiles.len(), if t.tiles.is_empty() { " (world pass only)" } else { "" });
            for tile in &t.tiles { println!("  tile c {:?} h {:?} = x [{}, {}] y [{}, {}] z [{}, {}]", tile.c, tile.h, tile.min()[0], tile.max()[0], tile.min()[1], tile.max()[1], tile.min()[2], tile.max()[2]); }
        }
        "probe-chunks" => {
            // lmtool probe-chunks MAP.Gbx [--scene xmin,ymin,zmin,xmax,ymax,zmax] [--block-size 32,8,32] [--offset 0,-38,0]
            //   [--level-h 0] [--global-quality 1] [--max-dim 2048]
            //   the probe grid of a bake (probechunk.rs): the grid def from the map's size words + the collection's block
            //   size + the decoration's base height (FUN_140c53ab0), the probe boxes (quality² > 0.9 records), the grid
            //   expansion, the 30×14×30 chunking and its atlas, every chunk record (slot, atlas range, world origin) and the
            //   chunks' AABB (the world peel's extra box)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let v3 = |s: String| -> [f32; 3] { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] };
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let size = [mf.size[0].max(0) as u32, mf.size[1].max(0) as u32, mf.size[2].max(0) as u32];
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("map scene");
            let gq: f32 = f("--global-quality").map(|v| v.parse().unwrap()).unwrap_or(1.0);
            let recs: Vec<lightmap::lmtiles::BlockRecord> = lightmap::lmtiles::item_records(&scene, gq, false).iter().filter_map(|it| it.record).collect();
            let scene_ch = match f("--scene") { Some(s) => { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); lightmap::lmtiles::CBox::from_min_max([v[0], v[1], v[2]], [v[3], v[4], v[5]]) } None => lightmap::lmtiles::scene_box(&recs) };
            let bs = f("--block-size").map(v3).unwrap_or([32.0, 8.0, 32.0]);
            let off = f("--offset").map(v3).unwrap_or([0.0, -40.0, 0.0]);
            let h: f32 = f("--level-h").map(|v| v.parse().unwrap()).unwrap_or(0.0);
            let max_dim: u32 = f("--max-dim").map(|v| v.parse().unwrap()).unwrap_or(2048);
            let (g, boxes, c, aabb) = lightmap::probechunk::for_records(size, bs, off, h, &recs, &scene_ch, max_dim);
            let g0 = lightmap::probechunk::grid_def(size, bs, off, h, true);
            println!("map size {:?} blocks × block size {:?} m, decoration offset {:?}, level h {h} → grid {} × {} × {} probes, cell {:?}, first probe at {:?}", size, bs, off, g0.n[0], g0.n[1], g0.n[2], g0.cell, g0.origin);
            println!("{} probe boxes (records with quality² > 0.9{}):", boxes.len(), if recs.is_empty() { "; none → the scene box's x/z with y −32..128" } else { "" });
            for b in &boxes { println!("  [{}, {}] × [{}, {}] × [{}, {}]", b.min()[0], b.max()[0], b.min()[1], b.max()[1], b.min()[2], b.max()[2]); }
            if g != g0 { println!("EXPANDED grid → {} × {} × {} probes, first probe at {:?}", g.n[0], g.n[1], g.n[2], g.origin); }
            println!("CHUNKING: {} × {} × {} chunks of 30 × 14 × 30 probes (cell {:?}); {} non-empty → atlas {} × {} × {} ({} columns × {} rows of 32 × 16 × 32 tiles)", c.chunk_counts[0], c.chunk_counts[1], c.chunk_counts[2], c.grid.cell, c.records.len(), c.atlas[0], c.atlas[1], c.atlas[2], c.columns, c.rows);
            for r in &c.records { println!("  chunk {:?}: probes {:?}..={:?} → slot {:?}, atlas {:?}..{:?}, world origin of atlas index 0 {:?} (ProbeToWorld = diag(cell) + this), cells [{}, {}] × [{}, {}] × [{}, {}]", r.chunk, r.imin, r.imax, r.slot, r.amin, r.amax, r.origin, (r.amin[0] as f32 - 0.5) * r.cell[0] + r.origin[0], (r.amax[0] as f32 - 0.5) * r.cell[0] + r.origin[0], (r.amin[1] as f32 - 0.5) * r.cell[1] + r.origin[1], (r.amax[1] as f32 - 0.5) * r.cell[1] + r.origin[1], (r.amin[2] as f32 - 0.5) * r.cell[2] + r.origin[2], (r.amax[2] as f32 - 0.5) * r.cell[2] + r.origin[2]); }
            if let Some(b) = aabb { println!("CHUNKS AABB (FUN_140233150, ∪ into the world peel box): c {:?} h {:?} = [{}, {}] × [{}, {}] × [{}, {}]", b.c, b.h, b.min()[0], b.max()[0], b.min()[1], b.max()[1], b.min()[2], b.max()[2]); let w = lightmap::lmtiles::world_peel_box(&scene_ch, Some(&b)); println!("WORLD PEEL BOX = scene ∪ chunks: c {:?} h {:?} = [{}, {}] × [{}, {}] × [{}, {}]", w.c, w.h, w.min()[0], w.max()[0], w.min()[1], w.max()[1], w.min()[2], w.max()[2]); }
        }
        "lmquality" => {
            // lmtool lmquality MAP: the MapElemLightmapQuality chunk 0x03043068 — its header words and the byte histogram over the
            // blocks / baked blocks / items ranges (one byte per element, in that order per the spec)
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let chunks = tmmaps::gbx::all_skip_chunks(&mf.gbx.body);
            let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == 0x0304_3068) else { println!("no 0x03043068 chunk"); return };
            let b = &mf.gbx.body[payload..payload + size];
            println!("chunk 0x03043068: {} bytes; header words {:?}; blocks {} baked {} items {} (sum {})", b.len(), (0..4.min(b.len() / 4)).map(|i| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap())).collect::<Vec<_>>(), mf.blocks.len(), mf.baked.len(), mf.items.len(), mf.blocks.len() + mf.baked.len() + mf.items.len());
            let body = &b[4.min(b.len())..];
            let mut hist: std::collections::BTreeMap<u8, usize> = Default::default();
            for &x in body { *hist.entry(x).or_default() += 1; }
            println!("byte histogram (whole body): {:?}", hist);
            let ranges = [("blocks", 0usize, mf.blocks.len()), ("baked", mf.blocks.len(), mf.blocks.len() + mf.baked.len()), ("items", mf.blocks.len() + mf.baked.len(), mf.blocks.len() + mf.baked.len() + mf.items.len())];
            for (name, s, e) in ranges { let mut h: std::collections::BTreeMap<u8, usize> = Default::default(); for &x in body.get(s..e.min(body.len())).unwrap_or(&[]) { *h.entry(x).or_default() += 1; } println!("  {name} [{s}..{e}): {:?}", h); }
            if a.iter().any(|x| x == "--list") { let s = mf.blocks.len() + mf.baked.len(); for (i, &x) in body.get(s..).unwrap_or(&[]).iter().enumerate() { if x != 0 { println!("  item {i} ({}) quality byte {x}", mf.items.get(i).map(|it| it.model.as_str()).unwrap_or("?")); } } }
        }
        "filecheck" => {
            // lmtool filecheck OURS.Map.Gbx --against EDITOR.Map.Gbx: two WRITTEN maps side by side — the mapping (count, order,
            // rects), frame 0's blobs (bytes, decoded values), the per-chart frame bytes, the three frame records' scale words,
            // the probe blob and the trailer — the gate of the transcribed file writer over a bake's own output
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let other = f("--against").expect("--against EDITOR.Map.Gbx");
            let ours = lightmap::mapio::load(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
            let theirs = lightmap::mapio::load(&other).unwrap_or_else(|e| panic!("{other}: {e}"));
            let (Some(d1), Some(d2)) = (ours.chunk.data.as_ref(), theirs.chunk.data.as_ref()) else { println!("a map without a lightmap"); return };
            let (Some(m1), Some(m2)) = (d1.cache.mapping(), d2.cache.mapping()) else { println!("a map without a mapping chunk"); return };
            println!("mapping: ours {} charts, theirs {} charts; atlas {}×{} vs {}×{}; bbox {:?}..{:?} vs {:?}..{:?}", m1.count, m2.count, m1.atlas_w, m1.atlas_h, m2.atlas_w, m2.atlas_h, m1.bbox_min, m1.bbox_max, m2.bbox_min, m2.bbox_max);
            let n = (m1.count.min(m2.count)) as usize;
            let (mut same_bind, mut same_rect) = (0usize, 0usize);
            for i in 0..n { if m1.binds[i].obj_group_idx == m2.binds[i].obj_group_idx && m1.binds[i].obj_idx == m2.binds[i].obj_idx { same_bind += 1; } if m1.pos[i] == m2.pos[i] && m1.size[i] == m2.size[i] { same_rect += 1; } }
            println!("  entry by entry: {same_bind} of {n} same bind words, {same_rect} of {n} same rects");
            for (k, (fb1, fb2)) in m1.frame_bytes.iter().zip(m2.frame_bytes.iter()).enumerate() {
                let nn = fb1.len().min(fb2.len());
                let eq = (0..nn).filter(|&i| fb1[i] == fb2[i]).count();
                let w1 = (0..nn).filter(|&i| fb1[i].abs_diff(fb2[i]) <= 1).count();
                let mx = (0..nn).map(|i| fb1[i].abs_diff(fb2[i])).max().unwrap_or(0);
                println!("  frame {k} bytes: {eq} of {nn} identical, {w1} within 1, max |Δ| {mx}");
            }
            // the frame records' scale words (head: 3 × 66 bytes from offset 60: +16 MaxHdrMood, +20 MaxHDR, +24 bounce, +28 sky)
            for i in 0..3 {
                let r = 60 + 66 * i;
                if r + 32 > m1.head.len() || r + 32 > m2.head.len() { break; }
                let w = |h: &[u8], o: usize| f32::from_le_bytes(h[r + o..r + o + 4].try_into().unwrap());
                println!("  frame record {i}: MaxHdrMood {} vs {}, MaxHDR {} vs {}, bounce {} vs {}, sky {} vs {}{}", w(&m1.head, 16), w(&m2.head, 16), w(&m1.head, 20), w(&m2.head, 20), w(&m1.head, 24), w(&m2.head, 24), w(&m1.head, 28), w(&m2.head, 28), if m1.head[r..r + 66] == m2.head[r..r + 66] { "  (record bytes identical)" } else { "" });
            }
            for (fi, (f1, f2)) in d1.frames.iter().zip(d2.frames.iter()).enumerate() {
                for (k, (b1, b2)) in f1.images.iter().zip(f2.images.iter()).enumerate() {
                    if b1.is_empty() && b2.is_empty() { continue; }
                    println!("  frame {fi} image {k}: {}", lightmap::filecheck::cmp_bytes(b2, b1));
                    if let Some((nv, ex, w1, w2, mx)) = lightmap::filecheck::cmp_decoded(b2, b1) { println!("      decoded: {ex} of {nv} values identical ({:.3} %), {w1} within 1, {w2} within 2, max |Δ| {mx}", 100.0 * ex as f64 / nv.max(1) as f64); } else { println!("      (decode: no libwebp in this build, or different sizes)"); }
                }
            }
            let (t1, t2) = (&d1.cache.trailer, &d2.cache.trailer);
            println!("  cache trailer: {}", lightmap::filecheck::cmp_bytes(t2, t1));
            let ids = |d: &lightmap::format::LightmapData| d.cache.chunks.iter().map(|c| format!("{:08x}", c.id)).collect::<Vec<_>>().join(" ");
            if ids(d1) != ids(d2) { println!("  cache chunk ids differ: ours [{}] theirs [{}]", ids(d1), ids(d2)); } else { println!("  cache chunk ids identical ({} chunks)", d1.cache.chunks.len()); }
        }
        "lmmesh-check" => {
            // lmtool lmmesh-check MAP.Gbx PASSCAP_ROOT [--env-frame 127448] [--items-dir DIR]: the LM meshes BUILT FROM THE MAP's
            // models (lmmesh::lm_mesh_of_item) against the captured LM vertex streams, mesh by mesh (the pad, the wall, the
            // vegetation, the tile) — vertex order, positions, snorm16 normals / uvs / tangents, psize, the triangle list
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[2]);
            let env_frame: u32 = f("--env-frame").map(|v| v.parse().unwrap()).unwrap_or(127448);
            let sc = lightmap::lmaccum::load_lm_scene(&root, env_frame).expect("captured LM scene");
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let items_dir = f("--items-dir");
            // every item's LM mesh from its model, paired with the captured one-instance mesh of the same vertex + triangle count
            // (the capture's draw order is the bind order, not the file order)
            for (item_i, it) in mf.items.iter().enumerate() {
                let inst = scene.instances.iter().find(|i| i.item == item_i);
                let bytes = match &items_dir { Some(d) => std::fs::read(format!("{d}/Items/{}", it.model)).ok(), None => None };
                let Some(bytes) = bytes else { println!("item {item_i} {} — no item file (--items-dir DIR from `mapgeom items MAP --out DIR`)", it.model); continue };
                if a.iter().any(|x| x == "--geoms") { if let Ok(fl) = mapgeom::static_item::file::parse_file(&bytes) { if let Some(s2) = fl.item.static_object().and_then(|so| so.solid2()) { for l in lightmap::lmmesh::geom_summary(s2) { println!("    {l}"); } } } }
                let ours_opt = lightmap::lmmesh::lm_mesh_of_item(&bytes);
                let mi = match &ours_opt { Ok(Some(o)) => (0..sc.meshes.len()).find(|&k| sc.inst_count[k] == 1 && sc.meshes[k].verts.len() == o.verts.len() && sc.meshes[k].indices.len() == o.indices.len()).or_else(|| (0..sc.meshes.len()).find(|&k| sc.inst_count[k] == 1 && sc.meshes[k].indices.len() == o.indices.len())), _ => None };
                let Some(mi) = mi else { println!("item {item_i} {}: {}", it.model, match &ours_opt { Ok(Some(o)) => format!("ours {} verts / {} tris — no captured one-instance mesh of that size", o.verts.len(), o.indices.len() / 3), Ok(None) => "no lightmapped visual in the model".into(), Err(e) => e.clone() }); continue };
                let m = &sc.meshes[mi];
                match ours_opt {
                    Ok(Some(ours)) => {
                        let d = lightmap::lmmesh::diff_meshes(&ours, m);
                        println!("mesh {mi} (eid {}): item {item_i} {}: ours {} verts / {} tris, captured {} / {}; positions exact {} within 1e-4 {} (worst {:.2e}); normals exact {}, uvs exact {}, tangents exact {}, psize exact {}; triangle list {}", sc.eids[mi], it.model, d.n_ours, ours.indices.len() / 3, d.n_theirs, m.indices.len() / 3, d.pos_exact, d.pos_within_1e4, d.pos_worst, d.nrm_exact, d.uv_exact, d.tan_exact, d.psize_exact, if d.tris_equal { "IDENTICAL" } else { "differs" });
                        if a.iter().any(|x| x == "--uv-study") { let raw = lightmap::lmmesh::raw_lm_uvs(&bytes).unwrap_or_default(); for i in 0..raw.len().min(m.verts.len()).min(12) { let r = raw[i]; let t = m.verts[i].uv; println!("    v{i}: raw uv [{:.9}, {:.9}] ×32767 = [{:.3}, {:.3}]; captured [{:.9}, {:.9}] = [{}, {}]/32767; 1−v: (1−raw.v)·32767 = {:.3}, captured 1−v → {:.3}", r[0], r[1], r[0] * 32767.0, r[1] * 32767.0, t[0], t[1], (t[0] * 32767.0).round(), (t[1] * 32767.0).round(), (1.0 - r[1]) * 32767.0, ((1.0 - t[1]) * 32767.0)); } }
                        // --perm: the captured vertex order against ours by position (the builder's order study): the runs of our indices
                        if a.iter().any(|x| x == "--perm") {
                            // match on the whole vertex when possible (duplicate positions on the two-sided cards), else on the position
                            let mut map: Vec<Option<usize>> = Vec::with_capacity(m.verts.len());
                            for t in &m.verts { map.push(ours.verts.iter().position(|o| o.pos == t.pos && o.uv == t.uv && o.normal == t.normal).or_else(|| ours.verts.iter().position(|o| o.pos == t.pos))); }
                            // the first-use re-indexing hypothesis: our vertices renumbered in order of first appearance in our index list
                            { let mut first: Vec<usize> = Vec::new(); let mut seen = vec![false; ours.verts.len()]; for &i in &ours.indices { if !seen[i as usize] { seen[i as usize] = true; first.push(i as usize); } } let ok = first.iter().zip(m.verts.iter()).filter(|(&o, t)| ours.verts[o].pos == t.pos).count(); println!("    first-use re-indexing: {} of {} captured vertices at the position our first-use order predicts", ok, m.verts.len().min(first.len())); }
                            let unmatched = map.iter().filter(|x| x.is_none()).count();
                            let mut runs: Vec<(usize, usize, usize)> = Vec::new(); // (captured start, ours start, len)
                            let mut i = 0; while i < map.len() { let Some(o0) = map[i] else { i += 1; continue }; let mut len = 1; while i + len < map.len() && map[i + len] == Some(o0 + len) { len += 1; } runs.push((i, o0, len)); i += len; }
                            println!("    permutation: {unmatched} captured vertices without a position match; {} runs of consecutive ours-indices; first 12 runs (captured start → ours start, len): {:?}", runs.len(), runs.iter().take(12).collect::<Vec<_>>());
                            // per captured vertex in a matched pair: are the normal / tangent / uv / psize equal after the permutation?
                            let (mut n_eq, mut t_eq, mut uv_eq, mut ps_eq, mut n_tr) = (0, 0, 0, 0, 0);
                            for (i, mo) in map.iter().enumerate() { if let Some(o) = mo { let (a, b) = (&ours.verts[*o], &m.verts[i]); if a.normal == b.normal { n_eq += 1; } if a.tangent == b.tangent { t_eq += 1; } if a.uv == b.uv { uv_eq += 1; } if a.psize == b.psize { ps_eq += 1; } n_tr += 1; } }
                            println!("    after the permutation ({n_tr} matched): normals equal {n_eq}, tangents equal {t_eq}, uvs equal {uv_eq}, psize equal {ps_eq}");
                            let words = lightmap::lmmesh::raw_normal_words(&bytes).unwrap_or_default();
                            for (i, mo) in map.iter().enumerate().take(4) { if let Some(o) = mo { let (a, b) = (&ours.verts[*o], &m.verts[i]); println!("      captured v{i} = ours v{o}: n ours {:?} capt {:?}; t ours {:?} capt {:?}; ps {} vs {}", a.normal, b.normal, a.tangent, b.tangent, a.psize, b.psize); if let Some((nw, tu, tv, nf)) = words.get(*o) { let cap_units: Vec<f32> = b.normal.iter().map(|v| v * 32767.0).collect(); println!("        raw words: normal {:?} tanU {:?} tanV {:?} normal f32 {:?}; dec3n/511 {:?} dec3n/512 {:?} dec3n/1023·2 {:?}; captured n × 32767 = {:?}; tanU/511 {:?}", nw.map(|w| format!("{w:#010x}")), tu.map(|w| format!("{w:#010x}")), tv.map(|w| format!("{w:#010x}")), nf, nw.map(|w| lightmap::lmmesh::dec3n_raw(w, 511.0)), nw.map(|w| lightmap::lmmesh::dec3n_raw(w, 512.0)), nw.map(|w| lightmap::lmmesh::dec3n_raw(w, 511.5)), cap_units, tu.map(|w| lightmap::lmmesh::dec3n_raw(w, 511.0))); } } }
                        }
                        if a.iter().any(|x| x == "--verbose") { for i in 0..ours.verts.len().min(m.verts.len()).min(12) { let (o, t) = (&ours.verts[i], &m.verts[i]); println!("    v{i}: ours pos {:?} n {:?} uv {:?} t {:?} ps {} | captured pos {:?} n {:?} uv {:?} t {:?} ps {}", o.pos, o.normal, o.uv, o.tangent, o.psize, t.pos, t.normal, t.uv, t.tangent, t.psize); } if !d.tris_equal { println!("    tris ours {:?}", &ours.indices[..ours.indices.len().min(24)]); println!("    tris capt {:?}", &m.indices[..m.indices.len().min(24)]); } }
                        if let Some(inst) = inst { let li = lightmap::lmmesh::lm_instance(&inst.pose, [0.0; 4]); let ci = &sc.instances[sc.inst_first[mi]]; println!("    instance: ours q {:?} t {:?} scale {} | captured q {:?} t {:?} scale {} st {:?}", li.q, li.t, li.scale, ci.q, ci.t, ci.scale, ci.st); }
                    }
                    Ok(None) => println!("mesh {mi}: item {item_i} {} — no lightmapped visual in the model", it.model),
                    Err(e) => println!("mesh {mi}: item {item_i} {} — {e}", it.model),
                }
            }
            // --scene: the whole LM scene from the map + the game's layout (lmmesh::lm_scene_from_map) against the capture's instances
            if a.iter().any(|x| x == "--scene") {
                let pak_arg = f("--pak"); let pak: Option<(&str, &str)> = pak_arg.as_deref().and_then(|p| p.rsplit_once(':'));
                let base = 4096u32;
                let gl = lightmap::layout::for_map(&a[1], &scene, base, f("--layout-quality").map(|v| v.parse().unwrap()).unwrap_or(2), lightmap::layout::TilePlg::BLUEBAY_SEA, pak, "BlueBay", "Sea", None).expect("layout");
                let tile_mesh = match pak { Some((pp, key)) => { let mut store = mapgeom::store::DataStore::empty(); store.add_pak(pp, key).expect("pak"); lightmap::lmmesh::lm_mesh_of_zone(&mut store, "BlueBay", "Sea").expect("zone") } None => None };
                let dir = items_dir.clone();
                let ours = lightmap::lmmesh::lm_scene_from_map(&scene, &gl, base, &|name| dir.as_ref().and_then(|d| std::fs::read(format!("{d}/Items/{name}")).ok()), tile_mesh, lightmap::layout::TilePlg::BLUEBAY_SEA, 2048.0).expect("lm scene");
                println!("LM scene from the map: {} meshes, {} instances (captured: {} meshes, {} instances)", ours.meshes.len(), ours.instances.len(), sc.meshes.len(), sc.instances.len());
                // pair instances by translation: q (up to sign), scale, st bits
                let (mut n, mut q_eq, mut st_eq, mut st_1ulp, mut sc_eq) = (0, 0, 0, 0, 0);
                let mut shown = 0;
                for o in &ours.instances { if let Some(c) = sc.instances.iter().find(|c| c.t == o.t) { n += 1; if o.q == c.q || o.q.iter().zip(c.q.iter()).all(|(a, b)| *a == -*b) { q_eq += 1; } if o.scale == c.scale { sc_eq += 1; } if o.st == c.st { st_eq += 1; } else { if (0..4).all(|k| (o.st[k].to_bits() as i64 - c.st[k].to_bits() as i64).abs() <= 1) { st_1ulp += 1; } if shown < 6 { shown += 1; println!("  st differs at t {:?}: ours {:?} captured {:?}", o.t, o.st, c.st); } } } }
                println!("  {n} instances paired by translation: q equal (up to sign) {q_eq}, scale equal {sc_eq}, st bit-identical {st_eq}, st within 1 ulp {st_1ulp}");
                // the ST formula study: which operation order reproduces every captured S / T bit for bit? (D's rule: S = (w − 1/4)/2048/(b_hi − b_lo),
                // T = (x + 1/8)/2048 − b_lo·S) — the variants differ in the last bit
                {
                    let b = lightmap::layout::TilePlg::BLUEBAY_SEA.bounds;
                    // every chart with its captured instance and its uv bounds: the tiles (the Sea bounds) and the items (their PreLightGen)
                    let mut tiles: Vec<(&lightmap::layout::LayoutChart, &lightmap::sunpass::LmInstance, [f32; 4])> = gl.charts.iter().filter(|c| c.obj < base).filter_map(|c| { let (cx, cz) = gl.cell_of[c.obj as usize]; sc.instances.iter().find(|i| i.t == [cx as f32 * 32.0, 0.0, cz as f32 * 32.0]).map(|i| (c, i, b)) }).collect();
                    // --st-mesh-bounds: the items' bounds from the MESH's TexCoord1 range (the lightmap-uv stats) instead of the PLG u04
                    let mesh_b = a.iter().any(|x| x == "--st-mesh-bounds");
                    for c in gl.charts.iter().filter(|c| c.obj >= base) { if let Some(inst) = scene.instances.iter().find(|i| base + i.item as u32 == c.obj) { if let Some(ci) = sc.instances.iter().find(|i| i.t == inst.pose.pos) { let m = &scene.models[inst.model]; let bb = if mesh_b { [m.uv_min[0], m.uv_min[1], m.uv_max[0], m.uv_max[1]] } else { m.plg_bounds.unwrap_or([0.0, 0.0, 1.0, 1.0]) }; if mesh_b { println!("  item {} bounds: PLG {:?} mesh {:?}", inst.item, m.plg_bounds, [m.uv_min[0], m.uv_min[1], m.uv_max[0], m.uv_max[1]]); } tiles.push((c, ci, bb)); } } }
                    let variants: Vec<(&str, Box<dyn Fn(f32, f32, f32, f32) -> (f32, f32)>)> = vec![
                        ("(w-1/4)/2048/dv ; (x+1/8)/2048 - lo*S", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) / 2048.0 / (hi - lo); (s, (x + 0.125) / 2048.0 - lo * s) })),
                        ("(w-1/4)/(2048*dv)", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) / (2048.0 * (hi - lo)); (s, (x + 0.125) / 2048.0 - lo * s) })),
                        ("((w-1/4)/dv)/2048", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = ((w - 0.25) / (hi - lo)) / 2048.0; (s, (x + 0.125) / 2048.0 - lo * s) })),
                        ("(w-1/4)*(1/2048)*rcp(dv)", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, (x + 0.125) / 2048.0 - lo * s) })),
                        ("(w*(1/2048) - 1/8192)/dv", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w * (1.0 / 2048.0) - 0.25 / 2048.0) / (hi - lo); (s, (x + 0.125) / 2048.0 - lo * s) })),
                        ("fma: (x+1/8)/2048 − lo·S fused", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) / 2048.0 / (hi - lo); (s, (-lo).mul_add(s, (x + 0.125) / 2048.0)) })),
                        ("S=(w-0.25)/dv/2048; T=(x+0.125 - lo*(w-0.25)/dv)/2048", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let sd = (w - 0.25) / (hi - lo); (sd / 2048.0, (x + 0.125 - lo * sd) / 2048.0) })),
                        ("S=(w-0.25)/dv/2048; T=(x+0.125 - lo*sd)*(1/2048)", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let sd = (w - 0.25) / (hi - lo); (sd * (1.0 / 2048.0), (x + 0.125 - lo * sd) * (1.0 / 2048.0)) })),
                        ("S rcp; T = (x+1/8)*(1/2048) - lo*S", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, (x + 0.125) * (1.0 / 2048.0) - lo * s) })),
                        ("S rcp; T = (x+1/8 - lo*(w-1/4)*rcp(dv))*(1/2048)", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let r = 1.0 / (hi - lo); let s = (w - 0.25) * (1.0 / 2048.0) * r; (s, (x + 0.125 - lo * ((w - 0.25) * r)) * (1.0 / 2048.0)) })),
                        ("S rcp; T = (x+1/8)/2048 - (lo*(w-1/4))*(1/2048)*rcp", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let r = 1.0 / (hi - lo); let s = (w - 0.25) * (1.0 / 2048.0) * r; (s, (x + 0.125) / 2048.0 - (lo * (w - 0.25)) * (1.0 / 2048.0) * r) })),
                        ("S rcp; T = fma(-lo, S, (x+1/8)/2048)", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, (-lo).mul_add(s, (x + 0.125) / 2048.0)) })),
                        ("S rcp; T = (x+1/8)/2048 + (-lo*S) via fma(x+1/8, 1/2048, -lo*S)", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, (x + 0.125).mul_add(1.0 / 2048.0, -(lo * s))) })),
                        ("S rcp; T = -(lo*S - (x+1/8)/2048)", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, -(lo * s - (x + 0.125) / 2048.0)) })),
                        ("S rcp; T = (x*(1/2048) + 1/16384) - lo*S", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, (x * (1.0 / 2048.0) + 0.125 / 2048.0) - lo * s) })),
                        ("S rcp; T = fma(x, 1/2048, 1/16384) - lo*S", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, x.mul_add(1.0 / 2048.0, 0.125 / 2048.0) - lo * s) })),
                        ("S rcp; T = fma(-lo, S, fma(x, 1/2048, 1/16384))", Box::new(|w: f32, x: f32, lo: f32, hi: f32| { let s = (w - 0.25) * (1.0 / 2048.0) * (1.0 / (hi - lo)); (s, (-lo).mul_add(s, x.mul_add(1.0 / 2048.0, 0.125 / 2048.0))) })),
                    ];
                    // the cross product: the stored S from one form, T's S from another (the game may keep an unrounded intermediate)
                    {
                        let sforms: Vec<(&str, Box<dyn Fn(f32, f32, f32) -> f32>)> = vec![
                            ("A=((w-¼)·(1/2048))·rcp", Box::new(|w: f32, lo: f32, hi: f32| ((w - 0.25) * (1.0 / 2048.0)) * (1.0 / (hi - lo)))),
                            ("B=((w-¼)·rcp)·(1/2048)", Box::new(|w: f32, lo: f32, hi: f32| ((w - 0.25) * (1.0 / (hi - lo))) * (1.0 / 2048.0))),
                            ("C=(w-¼)/2048/dv", Box::new(|w: f32, lo: f32, hi: f32| (w - 0.25) / 2048.0 / (hi - lo))),
                            ("D=(w-¼)/dv/2048", Box::new(|w: f32, lo: f32, hi: f32| (w - 0.25) / (hi - lo) / 2048.0)),
                            ("E=(w-¼)·rcp(dv·2048)", Box::new(|w: f32, lo: f32, hi: f32| (w - 0.25) * (1.0 / ((hi - lo) * 2048.0)))),
                            ("F=(w-¼)·rcp(dv)·(1/2048) as f64→f32", Box::new(|w: f32, lo: f32, hi: f32| (((w - 0.25) as f64) / ((hi - lo) as f64) / 2048.0) as f32)),
                        ];
                        for (sn, sf) in &sforms { for (tn, tf) in &sforms {
                            let (mut s_ok, mut t_ok, mut t_ok_f) = (0, 0, 0);
                            for (c, i, bb) in &tiles {
                                let (sx, sy) = (sf(c.w as f32, bb[0], bb[2]), sf(c.h as f32, bb[1], bb[3]));
                                let (tsx, tsy) = (tf(c.w as f32, bb[0], bb[2]), tf(c.h as f32, bb[1], bb[3]));
                                if sx == i.st[0] && sy == i.st[1] { s_ok += 1; }
                                let (tx, ty) = ((c.x as f32 + 0.125) / 2048.0 - bb[0] * tsx, (c.y as f32 + 0.125) / 2048.0 - bb[1] * tsy);
                                if tx == i.st[2] && ty == i.st[3] { t_ok += 1; }
                                let (txf, tyf) = ((-bb[0]).mul_add(tsx, (c.x as f32 + 0.125) / 2048.0), (-bb[1]).mul_add(tsy, (c.y as f32 + 0.125) / 2048.0));
                                if txf == i.st[2] && tyf == i.st[3] { t_ok_f += 1; }
                            }
                            if s_ok == tiles.len() || t_ok == tiles.len() || t_ok_f == tiles.len() { println!("  cross: stored S {sn:<36} T's S {tn:<36}: S both {s_ok}, T both {t_ok} (fused {t_ok_f}) of {}", tiles.len()); }
                        } }
                    }
                    for (name, fv) in &variants {
                        let (mut sx, mut sy, mut tx, mut ty) = (0, 0, 0, 0);
                        for (c, i, bb) in &tiles { let (s0, t0) = fv(c.w as f32, c.x as f32, bb[0], bb[2]); let (s1, t1) = fv(c.h as f32, c.y as f32, bb[1], bb[3]); if s0 == i.st[0] { sx += 1; } if s1 == i.st[1] { sy += 1; } if t0 == i.st[2] { tx += 1; } if t1 == i.st[3] { ty += 1; } }
                        println!("  ST variant {name:<52}: S.x {sx} S.y {sy} T.x {tx} T.y {ty} of {}", tiles.len());
                    }
                }
            }
            for (mi, m) in sc.meshes.iter().enumerate() { if sc.inst_count[mi] > 1 { println!("mesh {mi} (eid {}): {} vertices, {} triangles × {} instances — the zone tiles (the Sea prefab's SeaFloor plane)", sc.eids[mi], m.verts.len(), m.indices.len() / 3, sc.inst_count[mi]);
                // --pak FILE:KEY: the tile mesh from the zone prefab
                if let Some(pak) = f("--pak") { let (pak_path, key) = pak.rsplit_once(':').expect("--pak FILE:KEY"); let mut store = mapgeom::store::DataStore::empty(); store.add_pak(pak_path, key).expect("pak"); match lightmap::lmmesh::lm_mesh_of_zone(&mut store, &f("--collection").unwrap_or_else(|| "BlueBay".into()), &f("--zone").unwrap_or_else(|| "Sea".into())) { Ok(Some(ours)) => { let d = lightmap::lmmesh::diff_meshes(&ours, m); println!("    from the pak: ours {} verts / {} tris; positions exact {} (worst {:.2e}); normals exact {}, uvs exact {}, tangents exact {}, psize exact {}; triangle list {}", d.n_ours, ours.indices.len() / 3, d.pos_exact, d.pos_worst, d.nrm_exact, d.uv_exact, d.tan_exact, d.psize_exact, if d.tris_equal { "IDENTICAL" } else { "differs" }); if a.iter().any(|x| x == "--verbose") { for i in 0..ours.verts.len().min(m.verts.len()) { let (o, t) = (&ours.verts[i], &m.verts[i]); println!("      v{i}: ours pos {:?} uv {:?} | captured pos {:?} uv {:?}", o.pos, o.uv, t.pos, t.uv); } } } Ok(None) => println!("    from the pak: no lightmapped visual"), Err(e) => println!("    from the pak: {e}") } }
            } }
        }
        "item-parse" => {
            // lmtool item-parse ITEM.Item.Gbx: the typed static-item parse (geometry::load_model) with MAPGEOM_NODE_TRACE — which nodes define
            // --store LOGICAL --pak FILE:KEY: the stock-item route (geometry::load_model_from_store); --prefab LOGICAL: a prefab's
            // entities' PLGs (records::prefab_entity_records at the identity)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            // --zone-plg COLL:ZONE: the zone prefab's PLG (records::zone_tiles)
            if let Some(cz) = f("--zone-plg") {
                let (coll, zone) = cz.split_once(':').expect("--zone-plg COLL:ZONE");
                let pak_arg = f("--pak").expect("--pak FILE:KEY");
                let (pp, key) = pak_arg.rsplit_once(':').expect("--pak FILE:KEY");
                let mut store = mapgeom::store::DataStore::empty();
                store.add_pak(pp, key).expect("pak");
                match lightmap::records::zone_tiles(&mut store, coll, zone, 1, 0.0, 0.0, &|_, _| 1.0) {
                    Ok(v) => for r in &v { println!("{coll}/{zone}: MBU {} uv {:?} box centre {:?} half {:?}", r.meter_by_uv, r.uv, r.centre, r.half); },
                    Err(e) => println!("{coll}/{zone}: {e}"),
                }
                return;
            }
            if let Some(logical) = f("--prefab") {
                let pak_arg = f("--pak").expect("--pak FILE:KEY");
                let (pp, key) = pak_arg.rsplit_once(':').expect("--pak FILE:KEY");
                let mut store = mapgeom::store::DataStore::empty();
                store.add_pak(pp, key).expect("pak");
                let mut out = Vec::new(); let mut sub = 0u32;
                match lightmap::records::prefab_entity_records(&mut store, &logical, &mapgeom::geom::IDENTITY, "prefab", 0, &mut sub, 1.0, &mut out) {
                    Ok(()) => { for r in &out { println!("{logical}: sub {} MBU {} uv {:?} centre {:?} half {:?}", r.sub, r.meter_by_uv, r.uv, r.centre, r.half); } if out.is_empty() { println!("{logical}: no PLG entities"); } }
                    Err(e) => println!("{logical}: {e}"),
                }
                return;
            }
            if let Some(logical) = f("--store") {
                let pak_arg = f("--pak").expect("--pak FILE:KEY");
                let (pp, key) = pak_arg.rsplit_once(':').expect("--pak FILE:KEY");
                let mut store = mapgeom::store::DataStore::empty();
                store.add_pak(pp, key).expect("pak");
                match lightmap::geometry::load_model_from_store(&mut store, &logical) {
                    Ok(g) => println!("{logical}: {} tris, PLG u02 {} bounds {:?}, uv range {:?}..{:?}", g.tris.len(), g.plg_u02, g.plg_bounds, g.uv_min, g.uv_max),
                    Err(e) => println!("{logical}: {e}"),
                }
                return;
            }
            let bytes = std::fs::read(&a[1]).expect("item");
            match mapgeom::static_item::file::parse_file(&bytes) {
                Ok(f) => {
                    println!("typed parse ok: prefab {:?} (entities, truncated), static object {}", f.item.prefab().map(|p| (p.ents.len(), p.truncated)), f.item.static_object().is_some());
                    if let Some(p) = f.item.prefab() { for (i, e) in p.ents.iter().enumerate() { println!("  entity {i}: model index {} inline {:?}", e.model.index, e.model.inline.as_deref().map(|n| std::mem::discriminant(n))); } }
                }
                Err(e) => println!("typed parse FAILED: {e}"),
            }
            match lightmap::geometry::load_model(&bytes) {
                Ok(g) => println!("{}: {} tris, PLG u02 {} bounds {:?}, uv range {:?}..{:?}", a[1], g.tris.len(), g.plg_u02, g.plg_bounds, g.uv_min, g.uv_max),
                Err(e) => println!("{}: {e}", a[1]),
            }
        }
        "records-check" => {
            // lmtool records-check MAP --pak FILE:KEY --dump TSV [--collection Stadium] [--zone Grass] [--grid 96] [--cell-y 1] [--yoff 0]
            //   [--tile-q-far]: our record list vs the baker's /lmrecords dump (records.rs)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let pak = f("--pak").expect("--pak FILE:KEY");
            let (pp, key) = pak.rsplit_once(':').expect("--pak FILE:KEY");
            let mut store = mapgeom::store::DataStore::empty();
            store.add_pak(pp, key).expect("pak");
            let dump = lightmap::records::read_dump(&f("--dump").expect("--dump TSV")).expect("dump");
            let coll = f("--collection").unwrap_or_else(|| "Stadium".into());
            let zone = f("--zone").unwrap_or_else(|| "Grass".into());
            let grid: usize = f("--grid").map(|v| v.parse().unwrap()).unwrap_or(96);
            let cell_y: f32 = f("--cell-y").map(|v| v.parse().unwrap()).unwrap_or(1.0);
            let yoff: f32 = f("--yoff").map(|v| v.parse().unwrap()).unwrap_or(0.0);
            // the items' cells for the ring rule: the map's items at their file cells (x, y, z) — the tile level = the ground row
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            // the marked cells: the items' file cells AND the blocks' cells (the WaterBase blocks one level above the ground mark
            // their tiles at ring 1)
            let tile_y0: i32 = f("--tile-level").map(|v| v.parse().unwrap()).unwrap_or(9);
            let mut item_cells: std::collections::HashSet<(i32, i32, i32)> = mf.items.iter().map(|it| (it.file_cell[0] as i32, it.file_cell[1] as i32, it.file_cell[2] as i32)).filter(|c| a.iter().any(|x| x == "--items-3d") || c.1 == tile_y0).collect();
            // (a GHOST block — flags bit 28 — marks nothing: stpad's 12 flagged WaterBase blocks leave their tiles at ring ≥ 2)
            if !a.iter().any(|x| x == "--no-block-cells") { for b in &mf.blocks { if b.flags & 0x1000_0000 != 0 && !a.iter().any(|x| x == "--ghost-marks") { continue; } let (x, y, z) = b.coords(); item_cells.insert((x, y, z)); } }
            let tile_y: i32 = f("--tile-level").map(|v| v.parse().unwrap()).unwrap_or(9);
            println!("marked cells: {} (items {}, blocks {}); tile level {tile_y}; item cell y values {:?}", item_cells.len(), mf.items.len(), mf.blocks.len(), { let mut v: Vec<i32> = mf.items.iter().map(|it| it.file_cell[1] as i32).collect(); v.sort(); v.dedup(); v });
            let cells: Vec<(i32, i32)> = (0..grid as i32).flat_map(|cx| (0..grid as i32).map(move |cz| (cx, cz))).collect();
            let tq = lightmap::layout::tile_quality(&cells, tile_y, &item_cells);
            let q_of = |cx: usize, cz: usize| -> f32 { tq[cx * grid + cz] };
            let tiles = lightmap::records::zone_tiles(&mut store, &coll, &zone, grid, cell_y, yoff, &q_of).expect("zone tiles");
            println!("{} zone tiles; first {:?}", tiles.len(), tiles.first().map(|r| (r.centre, r.half, r.meter_by_uv, r.uv, r.quality)));
            let qh: std::collections::BTreeMap<u32, usize> = tiles.iter().fold(Default::default(), |mut m, r| { *m.entry(r.quality.to_bits()).or_default() += 1; m });
            println!("  tile quality histogram: {:?}", qh.iter().map(|(b, n)| (f32::from_bits(*b), *n)).collect::<Vec<_>>());
            // THE BLOCKS (authored, in block order — one obj each) and THE ENGINE'S CLIPS (mapgeom::bake::simulate: the free clips the
            // client instantiates at load, 1 028 drawn on stpad = the dump's 1 028 clip objects), then THE ITEMS
            let mut recs: Vec<lightmap::records::Rec> = Vec::new();
            let yoff_blocks: f32 = f("--block-yoff").map(|v| v.parse().unwrap()).unwrap_or(-64.0);
            let mut idx = mapgeom::blockmap::BlockInfoIndex::build(&store, &coll);
            let mut n_block_recs = 0usize;
            for (bi_, b) in mf.blocks.iter().enumerate() {
                let Some(path) = idx.path_for(&b.name) else { eprintln!("block {}: no block info for {}", bi_, b.name); continue };
                let bi = match idx.load(&mut store, &path) { Ok(bi) => bi.clone(), Err(e) => { eprintln!("block {}: {e}", bi_); continue } };
                let (x, y, z) = b.coords();
                let ground = b.flags & mapgeom::blockmap::FLAG_GROUND != 0;
                let variant = (b.flags & mapgeom::blockmap::FLAG_VARIANT_MASK) as usize;
                let subvariant = ((b.flags >> mapgeom::blockmap::FLAG_SUBVARIANT_SHIFT) & 63) as usize;
                let additional = ((b.flags >> mapgeom::blockmap::FLAG_ADDITIONAL_SHIFT) & 127) as usize;
                let n = lightmap::records::block_records(&mut store, &bi, [x, y, z], b.dir, ground, variant, subvariant, additional, yoff_blocks, "block", 16384 + bi_ as u32, 1.0, &mut recs).unwrap_or_else(|e| { eprintln!("block {bi_}: {e}"); 0 });
                n_block_recs += n;
            }
            println!("{} authored blocks → {n_block_recs} records; first {:?}", mf.blocks.len(), recs.first().map(|r| (r.centre, r.half, r.meter_by_uv, r.uv)));
            recs.extend(tiles.iter().cloned());
            // the clips
            let faces = mapgeom::fillers::faces(&mut store, &mut idx, &mf);
            let dirs: std::collections::HashMap<usize, u8> = mf.blocks.iter().map(|b| (b.index, b.dir)).collect();
            let grounds = mapgeom::bake::record_grounds(&faces, &mf);
            let mut clips = mapgeom::bake::simulate(&faces, &dirs, &grounds);
            // the game's creation order: the authored blocks in block order, per face, per clip of the face's list (mapgeom's simulate
            // walks its occupant cells sorted) — --clip-order sim keeps mapgeom's order
            if f("--clip-order").as_deref() != Some("sim") {
                let pos_in_list: Vec<usize> = clips.iter().map(|c| faces.occupants.get(&c.cell).and_then(|os| os.iter().find(|o| o.index == c.owner_index && o.unit == c.unit)).and_then(|o| o.faces[c.face].iter().position(|n| *n == c.name)).unwrap_or(0)).collect();
                let mut idx: Vec<usize> = (0..clips.len()).collect();
                idx.sort_by_key(|&i| (clips[i].owner_index, clips[i].unit, clips[i].face, pos_in_list[i]));
                clips = idx.iter().map(|&i| clips[i].clone()).collect();
            }
            let mut n_clip_objs = 0u32;
            let mut n_clip_recs = 0usize;
            let clip_obj0: u32 = 16384 + mf.blocks.len() as u32 + (grid * grid) as u32;
            for c in clips.iter().filter(|c| c.drawn()) {
                let Some(path) = idx.path_for(&c.name) else { eprintln!("clip {}: no block info", c.name); continue };
                let bi = match idx.load(&mut store, &path) { Ok(bi) => bi.clone(), Err(e) => { eprintln!("clip {}: {e}", c.name); continue } };
                // --clip-neighbour-cell: the piece's block cell = the owner's cell + step(face) (fillers.rs: a piece stands on side d
                // of ITS cell, its owner across that side)
                // the clip's cell = the OWNER block's own coords (mapgeom's occupant cells carry the rotated unit footprint — one off in
                // x and z for stpad's dir-3 blocks; the WaterBase blocks are single units)
                let owner_b = mf.blocks.iter().find(|b| b.index == c.owner_index);
                let owner_cell = match owner_b { Some(b) => { let (x, y, z) = b.coords(); [x, y, z] } None => [c.cell[0] as i32, c.cell[1] as i32, c.cell[2] as i32] };
                // THE CLIP BLOCK (stpad's table, 2026-09-25): a SIDE clip's block sits in the cell ACROSS the owner's face (owner cell +
                // step(face)) with dir = opposite(face) — the piece stands on that side of its cell, its owner across it (fillers.rs) —
                // and its mesh (local x ≈ −0.58: just outside its own cell) lands 0.58 m inside the owner's; a top/bottom clip keeps the
                // owner's cell above/below and the owner's dir (+ the clip's own)
                let mut cell = owner_cell;
                let d: u8 = if c.face < 4 {
                    let st = mapgeom::bake::step(c.face);
                    cell = [owner_cell[0] + st.0, owner_cell[1] + st.1, owner_cell[2] + st.2];
                    let fd: Option<Vec<u8>> = f("--clip-face-dirs").map(|v| v.split(',').map(|t| t.parse().unwrap()).collect());
                    match &fd { Some(v) => v[c.face], None => mapgeom::bake::opposite(c.face) as u8 }
                } else {
                    let st = mapgeom::bake::step(c.face);
                    if !a.iter().any(|x| x == "--tb-owner-cell") { cell = [owner_cell[0] + st.0, owner_cell[1] + st.1, owner_cell[2] + st.2]; }
                    c.dir_word() as u8
                };
                let class: &'static str = match c.face { 0 => "clipN", 1 => "clipE", 2 => "clipS", 3 => "clipW", 4 => "clipT", _ => "clipB" };
                // THE HORIZONTAL CLIP'S SHAPE (stpad's table, 2026-09-25 — 120 In / 16 Out / 288 Str reproduced): the mobil list of an
                // HFC Left (Right) piece is read off the owner's neighbours at the piece's end of the face — the cell beside the owner
                // on that side (ghost blocks count as present): empty → InEnd (list 4); present and the diagonal beyond the face also
                // present → OutEnd (list 8); present, diagonal empty → StrEnd (list 12). (EndEnd, list 0, never occurs here.)
                let mut variant = 0usize;
                if c.face < 4 && (c.name == "waterhfcleft" || c.name == "waterhfcright") {
                    let st = mapgeom::bake::step(c.face);
                    let (lx, lz) = if c.name == "waterhfcleft" { (-st.2, st.0) } else { (st.2, -st.0) };
                    let occ = |dx: i32, dz: i32| mf.blocks.iter().any(|b| { let (x, y, z) = b.coords(); (x, y, z) == (owner_cell[0] + dx, owner_cell[1], owner_cell[2] + dz) });
                    // the 16 lists = 4 left-end shapes × 4 right-end shapes (index = left·4 + right; End / In / Out / Str): the Left clip
                    // sets the left factor (0, 4, 8, 12), the Right clip the right one (0, 1, 2, 3)
                    let shape = if !occ(lx, lz) { 1 } else if occ(st.0 + lx, st.2 + lz) { 2 } else { 3 };
                    variant = if c.name == "waterhfcleft" { shape * 4 } else { shape };
                }
                let n = lightmap::records::block_records(&mut store, &bi, cell, d, c.ground, variant, 0, 0, yoff_blocks, class, clip_obj0 + n_clip_objs, 1.0, &mut recs).unwrap_or_else(|e| { eprintln!("clip {}: {e}", c.name); 0 });
                n_clip_objs += 1;
                n_clip_recs += n;
            }
            println!("{} drawn clips → {n_clip_recs} records ({n_clip_objs} objects from {clip_obj0})", clips.iter().filter(|c| c.drawn()).count());
            // THE ITEMS: one record per placed item with a PreLightGen (geometry::Scene: the embedded / stock models), the record box
            // from lmtiles::item_records (the mobil Iso4 chain), obj after the clips
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let irecs = lightmap::lmtiles::item_records(&scene, 1.0, false);
            let item_obj0 = clip_obj0 + n_clip_objs;
            let mut n_item_recs = 0usize;
            for (k, inst) in scene.instances.iter().enumerate() {
                let m = &scene.models[inst.model];
                let Some(b) = m.plg_bounds else { continue };
                let fx_only = !m.mat_links.is_empty() && m.mat_links.iter().all(|l| l.contains("RaceTriggerFX"));
                if fx_only { continue; }
                let Some(ir) = irecs.iter().find(|r| r.item == inst.item) else { continue };
                let Some(rec) = &ir.record else { continue };
                let q = lightmap::layout::item_quality(inst.lm_quality);
                recs.push(lightmap::records::Rec { class: "item", obj: item_obj0 + k as u32, sub: 0, meter_by_uv: m.plg_u02, uv: b, quality: q, centre: rec.world.c, half: rec.world.h, group: ((inst.model as u64) << 32) | q.to_bits() as u64 });
                n_item_recs += 1;
            }
            println!("{} items → {n_item_recs} records (objects from {item_obj0})", scene.instances.len());
            // --hfc-study: the horizontal free clips' SHAPE (the mobil list the engine picks: EndEnd / InEnd / OutEnd / StrEnd) read off
            // the dump against the owner's neighbourhood — the corner cells beside the face
            if a.iter().any(|x| x == "--hfc-study") {
                let occupied: std::collections::HashSet<(i32, i32, i32)> = mf.blocks.iter().filter(|b| b.flags & 0x1000_0000 == 0 || a.iter().any(|x| x == "--hfc-ghosts")).map(|b| { let (x, y, z) = b.coords(); (x, y, z) }).collect();
                let shape_of = |mbu: f32| -> &'static str { if (mbu - 32.76258).abs() < 1e-3 { "In" } else if (mbu - 32.740406).abs() < 1e-3 { "Out" } else if (mbu - 32.749474).abs() < 1e-3 { "Str" } else if (mbu - 33.512375).abs() < 1e-3 { "EndEnd" } else { "?" } };
                let mut table: std::collections::BTreeMap<String, usize> = Default::default();
                for c in clips.iter().filter(|c| c.drawn() && (c.name == "waterhfcleft" || c.name == "waterhfcright")) {
                    let Some(b) = mf.blocks.iter().find(|b| b.index == c.owner_index) else { continue };
                    let (ox, oy, oz) = b.coords();
                    // our piece (list 0) position → the dump's nearest HFC record
                    let hit = recs.iter().find(|r| r.class.starts_with("clip") && r.obj == clip_obj0 + clips.iter().filter(|d| d.drawn()).position(|d| std::ptr::eq(d, c)).unwrap() as u32 && r.sub == 0);
                    let Some(r) = hit else { continue };
                    let near = dump.iter().filter(|d| shape_of(d.meter_by_uv) != "?" && shape_of(d.meter_by_uv) != "EndEnd").min_by(|p, q| { let dp = (0..3).map(|k| (p.centre[k] - r.centre[k]).powi(2)).sum::<f32>(); let dq = (0..3).map(|k| (q.centre[k] - r.centre[k]).powi(2)).sum::<f32>(); dp.partial_cmp(&dq).unwrap() });
                    let Some(d) = near else { continue };
                    let dist = (0..3).map(|k| (d.centre[k] - r.centre[k]).powi(2)).sum::<f32>().sqrt();
                    let shape = if dist < 1.5 { shape_of(d.meter_by_uv) } else { "none" };
                    // the neighbourhood in the face's frame: fwd = step(face); left/right = the perpendicular
                    let st = mapgeom::bake::step(c.face);
                    let (lx, lz) = (-st.2, st.0); // a left-hand perpendicular of (dx, dz)
                    let occ = |dx: i32, dz: i32| occupied.contains(&(ox + dx, oy, oz + dz));
                    let key = format!("{} {}: fwd {} | left {} right {} | fwd-left {} fwd-right {}", c.name, shape, occ(st.0, st.2) as u8, occ(lx, lz) as u8, occ(-lx, -lz) as u8, occ(st.0 + lx, st.2 + lz) as u8, occ(st.0 - lx, st.2 - lz) as u8);
                    *table.entry(key).or_default() += 1;
                }
                println!("hfc study (shape read off the dump at our piece's position; neighbours of the owner in the face frame):");
                let mut v: Vec<_> = table.into_iter().collect(); v.sort();
                for (k, n) in v { println!("  {n:4} × {k}"); }
            }
            lightmap::records::compare(&recs, &dump);
            // --layout: the grouped allocation over our records against the map's own chart table (the dump as the bridge: our record →
            // its dump record → the key obj·4 | sub → the editor's rect)
            if a.iter().any(|x| x == "--layout") {
                let own = lightmap::mapio::load(&a[1]).expect("own bake");
                let q = lightmap::layout::quality_index_of(&own).unwrap_or(2);
                let d = own.chunk.data.as_ref().unwrap();
                let mp = d.cache.mapping().unwrap();
                let mut ed: std::collections::HashMap<u32, (u16, u16, u16, u16)> = Default::default();
                for i in 0..mp.count as usize { ed.insert(mp.binds[i].obj_group_idx, (mp.pos[i].0, mp.pos[i].1, mp.size[i].0, mp.size[i].1)); }
                let link = lightmap::records::match_dump(&recs, &dump);
                let gl = lightmap::records::layout_of(&recs, q).expect("layout");
                let (mut n, mut ok, mut same_size) = (0usize, 0usize, 0usize);
                let mut misses: Vec<String> = Vec::new();
                for c in &gl.charts {
                    let k = c.obj as usize;
                    let Some(Some(j)) = link.get(k) else { continue };
                    let key = (dump[*j].key >> 32) as u32;
                    let Some(&(ex, ey, ew, eh)) = ed.get(&key) else { continue };
                    n += 1;
                    if c.x == ex as i32 && c.y == ey as i32 && c.w == ew as i32 && c.h == eh as i32 { ok += 1; } else { if c.w == ew as i32 && c.h == eh as i32 { same_size += 1; } if misses.len() < 8 { misses.push(format!("{} #{k}: ours ({}, {}) {}×{} editor ({ex}, {ey}) {ew}×{eh}", recs[k].class, c.x, c.y, c.w, c.h)); } }
                }
                println!("layout: {} charts, s {} Σ {} (editor TotalLmSurfaceMeter in the chunk: see packtest), {ok} of {n} rects equal to the editor's table ({same_size} same size elsewhere)", gl.charts.len(), gl.s, gl.sum_area);
                for m in &misses { println!("  {m}"); }
            }
            // --ring-map: our tile ring index vs the dump's over the marked region (one char per cell: ours/dump differences marked)
            if a.iter().any(|x| x == "--ring-map") {
                let ring_of = |q: f32| -> char { let r = (-(q.log2()) * 2.0).round() as i32; if r >= 9 { '.' } else { char::from_digit(r as u32, 10).unwrap_or('?') } };
                let mut dq: std::collections::HashMap<(i32, i32), f32> = Default::default();
                for d in &dump { if (d.centre[1] - 8.125).abs() < 1e-3 && (d.meter_by_uv - 32.0641289).abs() < 1e-4 { dq.insert((((d.centre[0] - 6.6274185) / 32.0).round() as i32, ((d.centre[2] - 11.3137197) / 32.0).round() as i32), d.quality); } }
                let (x0, x1, z0, z1): (i32, i32, i32, i32) = (10, 78, 20, 70);
                println!("ring map (ours; a lowercase letter where the dump differs: the dump's ring as a..i = 0..8, j = far):");
                for cz in z0..z1 {
                    let mut line = String::new();
                    for cx in x0..x1 {
                        let ours = tq[(cx as usize) * grid + cz as usize];
                        let oc = ring_of(ours);
                        match dq.get(&(cx, cz)) { Some(&d) if (d - ours).abs() > 1e-6 => { let r = (-(d.log2()) * 2.0).round() as i32; line.push((b'a' + r.min(9) as u8) as char); } _ => line.push(oc) }
                    }
                    println!("{cz:3} {line}");
                }
            }
        }
        "dome-check" => {
            // lmtool dome-check PASSCAP [--frame 127448] [--direction 0] [--filter f32|f16|f16sum] [--weights floor|round|f32] [--rsq-approx]
            //   [--half H] [--top N]: PS 16774 transcribed on the frame's own inputs against the captured environment layer (domecheck.rs)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let root = std::path::PathBuf::from(&a[1]);
            let frame_no: u32 = f("--frame").map(|v| v.parse().unwrap()).unwrap_or(127448);
            let direction: u32 = f("--direction").map(|v| v.parse().unwrap()).unwrap_or(0);
            let st = lightmap::domecheck::Study {
                filter: match f("--filter").as_deref() { Some("f16") => lightmap::domecheck::FilterArith::F16Each, Some("f16sum") => lightmap::domecheck::FilterArith::F16Sum, Some("f16prod") => lightmap::domecheck::FilterArith::F16Prod, Some("f16lerp") => lightmap::domecheck::FilterArith::F16Lerp, Some("f16fma") => lightmap::domecheck::FilterArith::F16Fma, Some("f16sumrtz") => lightmap::domecheck::FilterArith::F16SumRtz, _ => lightmap::domecheck::FilterArith::F32 },
                weights: match f("--weights").as_deref() { Some("round") => lightmap::domecheck::WeightPrec::Round8, Some("f32") => lightmap::domecheck::WeightPrec::F32, Some("floor9") => lightmap::domecheck::WeightPrec::Floor9, Some("even8") => lightmap::domecheck::WeightPrec::Even8, Some("coord8") => lightmap::domecheck::WeightPrec::Coord8, Some("coord8floor") => lightmap::domecheck::WeightPrec::Coord8Floor, _ => lightmap::domecheck::WeightPrec::Floor8 },
                rsq_approx: a.iter().any(|x| x == "--rsq-approx"),
                half: f("--half").map(|v| v.parse().unwrap()).unwrap_or(0.5),
                frac_bits: f("--frac-bits").map(|v| v.parse().unwrap()).unwrap_or(8),
            };
            // the draw's constants from the frame's draws log
            let draws = lightmap::lmaccum::load_draws(&root, frame_no).expect("draws log");
            let dome = draws.iter().find(|e| e.pointer("/Pixel/shader").and_then(|v| v.as_str()) == Some("16774")).expect("the dome draw (PS 16774)");
            let g = |p: &str| dome.pointer(p).cloned().unwrap_or(serde_json::Value::Null);
            let f3 = |v: &serde_json::Value| -> [f32; 3] { let a = v.as_array().expect("float3"); [a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32, a[2].as_f64().unwrap() as f32] };
            let f4 = |v: &serde_json::Value| -> [f32; 4] { let a = v.as_array().expect("float4"); [a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32, a[2].as_f64().unwrap() as f32, a[3].as_f64().unwrap() as f32] };
            let cb = "/Pixel/cbuffers/ShaderP/g_CBuffer";
            let sp = "/Pixel/cbuffers/SceneP";
            let c = lightmap::domecheck::DomeConsts {
                scale_grad0: g(&format!("{cb}/ScaleGrad0")).as_f64().unwrap() as f32,
                scale_grad1: g(&format!("{cb}/ScaleGrad1")).as_f64().unwrap() as f32,
                sun_power: g(&format!("{cb}/SunPower")).as_f64().unwrap() as f32,
                sun_is_visible: g(&format!("{cb}/SunIsVisible")).as_f64().unwrap_or(0.0) != 0.0,
                pow_scale: f4(&g(&format!("{cb}/SunAtmo_PowScale1_PowScale2"))),
                rgb1: f3(&g(&format!("{cb}/SunAtmo_RgbLinear1"))),
                rgb2: f3(&g(&format!("{cb}/SunAtmo_RgbLinear2"))),
                fog_intens: g(&format!("{cb}/FogIntens")).as_f64().unwrap() as f32,
                global_scale: g(&format!("{cb}/GlobalScale")).as_f64().unwrap() as f32,
                light_dir: f3(&g(&format!("{sp}/GbxP_LightDirDirInWorld0"))),
                light_rgb: f3(&g(&format!("{sp}/GbxP_LightDirRgbLinear0"))),
                fog_rgb: f3(&g(&format!("{sp}/GbxP_Fog_LinearRGB"))),
                eye: f3(&g(&format!("{sp}/GbxP_EyeInWorld"))),
                light_dir_angle: g("/Vertex/cbuffers/DrawV/GbxSkyV0/LightDirAngle_m11Zx").as_f64().unwrap() as f32,
                force_x: g("/Vertex/cbuffers/DrawV/GbxSkyV0/GradientV_ForceX").as_f64().unwrap_or(-1.0) as f32,
                invert_y: g("/Vertex/cbuffers/DrawV/GbxSkyV0/GradientV_InvertY").as_f64().unwrap_or(1.0) != 0.0,
            };
            println!("dome draw eid {}: {:?}", dome["eid"], c);
            let grad0 = lightmap::domecheck::load_grad(&root, frame_no, 16801).expect("TMapGradientV");
            let grad1 = lightmap::domecheck::load_grad(&root, frame_no, 16803).expect("TMapGradientV1");
            println!("TMapGradientV {} | TMapGradientV1 {}", lightmap::domecheck::grad_summary(&grad0), lightmap::domecheck::grad_summary(&grad1));
            // the world peel's frustum + the captured environment layer (colour + depth) of the direction
            let txt = std::fs::read_to_string(root.join("MANIFEST.json")).or_else(|_| std::fs::read_to_string(f("--manifest").unwrap_or_default())).expect("MANIFEST.json (or --manifest)");
            let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
            // the direction's FIRST environment layer (the world peel: the lowest event id) — colour + depth
            // (a frame holds one direction's environment render; the manifest's direction indices are re-numbered on read, so the
            // frame is the key — --direction only labels the run)
            let mut ents: Vec<&lightmap::passdump::Entry> = m.passes.iter().filter(|e| e.frame == Some(frame_no) && e.layer == Some(0) && (e.pass == "peel_color" || e.pass == "peel_depth")).collect();
            let _ = direction;
            ents.sort_by_key(|e| e.eid_last.unwrap_or(0));
            let col = ents.iter().find(|e| e.pass == "peel_color").expect("the captured environment colour layer");
            let dep = ents.iter().find(|e| e.pass == "peel_depth" && e.eid_last == col.eid_last).or_else(|| ents.iter().find(|e| e.pass == "peel_depth"));
            // the world peel's frustum: the captured one when the entry carries it, else the TRANSCRIBED fit (lightcam, bit-identical to
            // the captured pwc-day cameras) for the draw's forward axis (GbxP_WorldToCamera's third row)
            let fwd_row = dome.pointer("/Pixel/cbuffers/SceneP/GbxP_WorldToCamera").and_then(|v| v.as_array()).map(|rows| f3(&rows[2])).unwrap_or([0.3454769, 0.1170782, 0.9310952]);
            let fr: lightmap::passdump::Frustum = match &col.frustum { Some(fr) => fr.clone(), None => { let fit = lightmap::lightcam::peel_frusta(fwd_row, &lightmap::lightcam::PeelBoxes::pwc_day(), 4096, &lightmap::lightcam::FitRules::default()); fit.into_iter().next().expect("the transcribed world peel frustum") } };
            let frame = lightmap::peel::PeelFrame::from_frustum(&fr, if col.width > 0 { col.width } else { 4096 }, if col.height > 0 { col.height } else { 4096 });
            println!("frustum ({}): centre {:?} half {:?} forward {:?}; colour {} ({} eid {:?}); depth {}", if col.frustum.is_some() { "captured" } else { "transcribed fit" }, fr.center, fr.half, fr.forward, col.file, col.capture.as_deref().unwrap_or("?"), col.eid_last, dep.map(|e| e.file.as_str()).unwrap_or("none"));
            let game = lightmap::passdiff::load_entry(&root, col).expect("captured colour");
            let gdepth = dep.map(|e| lightmap::passdiff::load_entry(&root, e).expect("captured depth"));
            let mesh = lightmap::domemesh::DomeMesh::load(&root).expect("dome mesh");
            let eye = if a.iter().any(|x| x == "--eye-centre") { frame.frustum().center } else { c.eye };
            let dr = mesh.rasterise(&frame, eye, c.light_dir_angle, c.force_x, c.invert_y);
            println!("dome mesh: {} kept triangles; eye {:?}", dr.triangles(), eye);
            let mut stats = lightmap::domecheck::QuantaStats::default();
            let (mut n_dome_game, mut n_ours_only, mut n_game_only) = (0usize, 0usize, 0usize);
            let top: usize = f("--top").map(|v| v.parse().unwrap()).unwrap_or(8);
            let mut worst: Vec<(i64, u32, u32, [f32; 3], [f32; 3], [f32; 2])> = Vec::new();
            let inset = 1u32;
            for y in inset..game.h - inset {
                for x in inset..game.w - inset {
                    // a dome pixel of the game: the env depth at the clear (0 = the dome) when a depth is banked, else any non-black
                    let is_dome_game = match &gdepth { Some(d) => d.get(x, y, 0) == 0.0, None => { let p = lightmap::domecheck::buf_rgb(&game, x, y); p[0] > 0.0 || p[1] > 0.0 || p[2] > 0.0 } };
                    let ours = dr.at(x, y);
                    match (is_dome_game, ours) {
                        (true, Some((uv, view))) => {
                            n_dome_game += 1;
                            let o = lightmap::domecheck::ps_16774(uv, view, &c, &grad0, &grad1, &st);
                            let oq = lightmap::gpufmt::quantise_r11g11b10(o, lightmap::gpufmt::Rounding::Truncate);
                            let gq = lightmap::domecheck::buf_rgb(&game, x, y);
                            lightmap::domecheck::compare_quanta(oq, gq, &mut stats);
                            let dsum: i64 = (0..3).map(|k| (lightmap::domecheck::r11_steps(oq[k], k == 2) - lightmap::domecheck::r11_steps(gq[k], k == 2)).abs()).sum();
                            if dsum > 0 && worst.len() < 4000 { worst.push((dsum, x, y, o, gq, uv)); }
                        }
                        (true, None) => { n_dome_game += 1; n_game_only += 1; }
                        (false, Some(_)) => { n_ours_only += 1; }
                        _ => {}
                    }
                }
            }
            let pct = |v: usize| 100.0 * v as f64 / stats.n.max(1) as f64;
            println!("dome pixels (game): {n_dome_game}; compared {}; game-only (no dome triangle of ours) {n_game_only}; ours-only (game not dome) {n_ours_only}", stats.n);
            for k in 0..3 { println!("  {}: exact {} ({:.3} %), within ±1 step {} ({:.3} %), worse {}; +1 {} / −1 {}", ["R", "G", "B"][k], stats.exact[k], pct(stats.exact[k]), stats.within1[k], pct(stats.within1[k]), stats.worse[k], stats.plus1[k], stats.minus1[k]); }
            worst.sort_by_key(|w| std::cmp::Reverse(w.0));
            for w in worst.iter().take(top) { println!("  ({}, {}): ours f32 {:?} → game {:?}; uv ({:.6}, {:.6})", w.1, w.2, w.3, w.4, w.5[0], w.5[1]); }
            // the residual's size: for each miss, the relative distance of our f32 value to the R11 boundary it should have crossed
            // (ours below the boundary when the game is +1: the needed shift is positive) — the histogram in decades
            {
                let mut hist: std::collections::BTreeMap<i32, usize> = Default::default();
                let mut signed: [i64; 2] = [0, 0];
                for w in &worst {
                    for k in 0..3 {
                        let (a, b) = (lightmap::domecheck::r11_steps(lightmap::gpufmt::quantise_r11g11b10(w.3, lightmap::gpufmt::Rounding::Truncate)[k], k == 2), lightmap::domecheck::r11_steps(w.4[k], k == 2));
                        if a == b { continue; }
                        // the boundary = the game's quantised value when the game is above ours, else the next quantum above the game's
                        let boundary = if b > a { w.4[k] } else { let up = lightmap::gpufmt::quantise_r11g11b10([if k == 0 { w.4[k] } else { 0.0 } + if k == 0 { 0.0 } else { 0.0 }; 3], lightmap::gpufmt::Rounding::Truncate); let _ = up; f32::NAN };
                        if boundary.is_nan() { signed[1] += 1; continue; }
                        signed[0] += 1;
                        let rel = ((boundary - w.3[k]) / boundary.abs().max(1e-9)).abs();
                        let dec = rel.log10().floor() as i32;
                        *hist.entry(dec).or_default() += 1;
                    }
                }
                println!("  needed relative shift (game above ours) by decade: {:?}; game below ours: {} channel misses", hist, signed[1]);
            }
            // the mismatch map by uv: where in the texture do the misses sit?
            let mut vhist: std::collections::BTreeMap<u32, usize> = Default::default();
            for w in &worst { *vhist.entry((w.5[1] * 64.0) as u32).or_default() += 1; }
            println!("  misses by v band (1/64): {:?}", vhist);
        }
        "genealogy" => {
            // lmtool genealogy MAP: the zone genealogy records (chunk 0x03043043) — per cell the CurrentZoneId and its Dir,
            // as a histogram and the first few records; plus the baked block list (the generated tiles when the file has them)
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let chunks = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
            let Some(&(_, _, payload, size)) = chunks.iter().find(|(c, ..)| *c == 0x0304_3043) else { println!("no genealogy chunk"); return };
            let recs = tmmaps::map::genealogy_full(&m.gbx.body[payload..payload + size]).expect("genealogy");
            println!("{} genealogy records (size {:?})", recs.len(), m.size);
            let mut hist: std::collections::BTreeMap<(String, u32), usize> = Default::default();
            for r in &recs { *hist.entry((r.current.clone(), r.dir)).or_insert(0) += 1; }
            for ((z, d), n) in &hist { println!("  zone {z:?} dir {d}: {n}"); }
            for (i, r) in recs.iter().enumerate().take(4) { println!("  rec {i}: chain {:?} current_index {} dir {} current {:?}", r.ids, r.current_index, r.dir, r.current); }
            println!("blocks {} baked {}", m.blocks.len(), m.baked.len());
            for b in m.baked.iter().take(4) { println!("  baked {:?} coords {:?} dir {} flags {:#x}", b.name, b.coords(), b.dir, b.flags); }
        }
        "tile-boxes" => {
            // lmtool tile-boxes MAP.Gbx --pak FILE:KEY [--collection BlueBay] [--yoff Y] [--zone Sea] [--out TSV]
            //   the zone tiles' BLOCK RECORDS the way the game builds them: per cell (the genealogy's CurrentZoneId + Dir, or
            //   --zone for every cell of a size-64 map without a genealogy) the zone block info's ground variant → its mobil's
            //   prefab → entity 0's static-object solid → the stored visual bounding boxes (all geoms, geom order) →
            //   lmtiles::model_box; the block Iso4 (cell·(32, 8, 32) + the decoration offset, the Dir rotation about the unit's
            //   centre) → CBox::transformed; then lmtiles::scene_box (the FUN_140184fa0 fold in cell order) — the sun camera's
            //   focus box S from the map alone (frustum-check --scene takes the printed min/max)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let m = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let coll = f("--collection").unwrap_or_else(|| "BlueBay".into());
            let pak = f("--pak").expect("--pak FILE:KEY");
            let (pak_path, key) = pak.rsplit_once(':').expect("--pak FILE:KEY");
            let mut store = mapgeom::store::DataStore::empty();
            store.add_pak(pak_path, key).expect("pak");
            // the per-cell zone + dir: the genealogy, else --zone everywhere (the capture's 4096 Sea tiles on a cleared genealogy)
            let chunks = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
            let gen: Vec<(String, u32)> = chunks.iter().find(|(c, ..)| *c == 0x0304_3043).and_then(|&(_, _, payload, size)| tmmaps::map::genealogy_full(&m.gbx.body[payload..payload + size]).ok()).map(|recs| recs.into_iter().map(|r| (r.current, r.dir)).collect()).unwrap_or_default();
            let (sx, sz) = (m.size[0].max(0) as usize, m.size[2].max(0) as usize);
            let cells: Vec<(usize, usize, String, u32)> = if gen.len() == sx * sz {
                // records are indexed x·64 + z (map-genealogy.md §1)
                (0..gen.len()).map(|i| (i / sz, i % sz, gen[i].0.clone(), gen[i].1)).collect()
            } else {
                let z = f("--zone").unwrap_or_else(|| "Sea".into());
                let mut v = Vec::new();
                for cz in 0..sz { for cx in 0..sx { v.push((cx, cz, z.clone(), 0u32)); } }
                v
            };
            let yoff: f32 = f("--yoff").map(|v| v.parse().unwrap()).unwrap_or(-38.0);
            // per zone: the prefab's visual boxes + the ground variant's cell height (the block y) — cached
            let mut zone_boxes: std::collections::BTreeMap<String, (String, Vec<lightmap::lmtiles::CBox>)> = Default::default();
            let mut records: Vec<lightmap::lmtiles::BlockRecord> = Vec::with_capacity(cells.len());
            let mut out_rows: Vec<String> = Vec::new();
            for (cx, cz, zone, dir) in &cells {
                if !zone_boxes.contains_key(zone) {
                    let mut found: Option<(String, Vec<lightmap::lmtiles::CBox>)> = None;
                    for fam in ["GameCtnBlockInfoFlat", "GameCtnBlockInfoFrontier", "GameCtnBlockInfoTransition", "GameCtnBlockInfoClassic"] {
                        let ext = match fam { "GameCtnBlockInfoFlat" => "EDFlat", "GameCtnBlockInfoFrontier" => "EDFrontier", "GameCtnBlockInfoTransition" => "EDTransition", _ => "EDClassic" };
                        let path = format!("{coll}\\GameCtnBlockInfo\\{fam}\\{zone}.{ext}.Gbx");
                        let Ok(bi) = mapgeom::blockinfo::load(&mut store, &path) else { continue };
                        let Some(v) = bi.variant_base_ground.as_ref() else { continue };
                        let Some(pp) = v.mobils.iter().flatten().find_map(|mb| mb.prefab.clone()) else { continue };
                        let pm = store.load_model(&pp).expect("prefab");
                        let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm).expect("prefab parse");
                        let mut boxes = Vec::new();
                        for e in &pf.ents {
                            let Some(mapgeom::static_item::Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
                            let Some(s2) = so.solid2() else { continue };
                            for sg in &s2.shaded_geoms {
                                let Some(vr) = s2.visuals.get(sg.visual_index as usize) else { continue };
                                let Some(mapgeom::static_item::Node::Visual(vis)) = vr.inline.as_deref() else { continue };
                                if let Some(mm) = vis.main.as_ref() { let b = mm.bounding_box; boxes.push(lightmap::lmtiles::CBox::new([b[0], b[1], b[2]], [b[3], b[4], b[5]])); }
                            }
                            break; // entity 0 = the tile mesh
                        }
                        println!("zone {zone}: {path} → {pp}: {} visual boxes: {:?}", boxes.len(), boxes);
                        found = Some((pp, boxes));
                        break;
                    }
                    zone_boxes.insert(zone.clone(), found.unwrap_or_else(|| { eprintln!("zone {zone}: no block info found"); (String::new(), Vec::new()) }));
                }
                let (pp, boxes) = &zone_boxes[zone];
                let Some(mb) = lightmap::lmtiles::model_box(boxes) else { continue };
                // the block Iso4: the Dir rotation about the unit's centre (16, ·, 16), translation = cell·(32, 8, 32) + yoff
                let cy = f("--cell-y").map(|v| v.parse::<f32>().unwrap()).unwrap_or(5.0);
                let (s, c) = match dir { 0 => (0.0f32, 1.0f32), 1 => (1.0, 0.0), 2 => (0.0, -1.0), _ => (-1.0, 0.0) };
                // R = Ry(dir·90°) in the engine's row-major layout: world = R·local + t; rotation about (16, 0, 16) inside the cell
                let (px, pz) = (16.0f32, 16.0f32);
                let r: [f32; 9] = [c, 0.0, s, 0.0, 1.0, 0.0, -s, 0.0, c];
                let tx = *cx as f32 * 32.0 + (px - (c * px + s * pz));
                let tz = *cz as f32 * 32.0 + (pz - (-s * px + c * pz));
                let iso: lightmap::lmtiles::Iso4 = [r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], tx, cy * 8.0 + yoff, tz];
                let rec = lightmap::lmtiles::block_record(&mb, &iso, lightmap::lmtiles::quality_byte(0, 0.5));
                if has("--verbose") || out_rows.len() < 3 { println!("  cell ({cx}, {cz}) {zone} dir {dir}: model box c {:?} h {:?} → record c {:?} h {:?} q {:.4}", mb.c, mb.h, rec.world.c, rec.world.h, rec.quality); }
                out_rows.push(format!("{cx}\t{cz}\t{zone}\t{dir}\t{pp}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", rec.world.c[0], rec.world.c[1], rec.world.c[2], rec.world.h[0], rec.world.h[1], rec.world.h[2], rec.quality));
                records.push(rec);
            }
            let s = lightmap::lmtiles::scene_box(&records);
            println!("{} tile records; SCENE BOX (tiles only, cell order) c {:?} h {:?} = [{}, {}] × [{}, {}] × [{}, {}]", records.len(), s.c, s.h, s.min()[0], s.max()[0], s.min()[1], s.max()[1], s.min()[2], s.max()[2]);
            // + the items (the map's own records) in the game's bind order: --items-first puts them before the tiles
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("map scene");
            let item_recs: Vec<lightmap::lmtiles::BlockRecord> = lightmap::lmtiles::item_records(&scene, 1.0, false).iter().filter_map(|it| it.record).collect();
            let all: Vec<lightmap::lmtiles::BlockRecord> = if has("--items-first") { item_recs.iter().chain(records.iter()).copied().collect() } else { records.iter().chain(item_recs.iter()).copied().collect() };
            let s_all = lightmap::lmtiles::scene_box(&all);
            println!("SCENE BOX S (tiles + {} items, {}): c {:?} h {:?} = [{}, {}] × [{}, {}] × [{}, {}]  → --scene {},{},{},{},{},{}", item_recs.len(), if has("--items-first") { "items first" } else { "tiles first" }, s_all.c, s_all.h, s_all.min()[0], s_all.max()[0], s_all.min()[1], s_all.max()[1], s_all.min()[2], s_all.max()[2], s_all.min()[0], s_all.min()[1], s_all.min()[2], s_all.max()[0], s_all.max()[1], s_all.max()[2]);
            println!("  bits: c {:08x} {:08x} {:08x} h {:08x} {:08x} {:08x}", s_all.c[0].to_bits(), s_all.c[1].to_bits(), s_all.c[2].to_bits(), s_all.h[0].to_bits(), s_all.h[1].to_bits(), s_all.h[2].to_bits());
            if let Some(out) = f("--out") { let mut t = String::from("cx\tcz\tzone\tdir\tprefab\tcx_w\tcy_w\tcz_w\thx\thy\thz\tquality\n"); for r in &out_rows { t.push_str(r); t.push('\n'); } std::fs::write(&out, t).expect("write"); println!("wrote {out}"); }
        }
        "pak-tables" => {
            // lmtool pak-tables --pak FILE:KEY [--pak FILE:KEY …] --collection C [--material LINK …] [--out-fog F] [--out-transmittance F]
            //   the attribute pre-pass's collection inputs FROM THE PACK (paktables.rs, RE 8): the constant of every
            //   world-projected material named (PS 8401 terrain slices / PS 17025 projected textures at the zero
            //   world matrix), and the water pass's tables: g_WaterTop_ByPlanes, g_WaterDepth_FogMaxDepthInv_ByIds,
            //   the fog LUT (15075) and the transmittance LUT (15078), written as raw RGBA8 when asked.
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let mut store = mapgeom::store::DataStore::empty();
            let mut it = a.iter();
            while let Some(x) = it.next() {
                if x == "--pak" {
                    let spec = it.next().expect("--pak FILE:KEY");
                    let (pp, key) = spec.rsplit_once(':').expect("--pak FILE:KEY");
                    store.add_pak(pp, key).unwrap_or_else(|e| panic!("{pp}: {e}"));
                }
            }
            let coll = f("--collection").unwrap_or_else(|| "BlueBay".into());
            let mut links: Vec<String> = Vec::new();
            let mut it = a.iter();
            while let Some(x) = it.next() {
                if x == "--material" {
                    links.push(it.next().expect("--material LINK").clone());
                }
            }
            for link in &links {
                match lightmap::paktables::material_constant(&mut store, link) {
                    Ok(c) => println!("{link}: {:?} constant ({:.8}, {:.8}, {:.8}) = bits {:08x} {:08x} {:08x}  image {} uv {:?} ids {:?}", c.family, c.rgb[0], c.rgb[1], c.rgb[2], c.rgb[0].to_bits(), c.rgb[1].to_bits(), c.rgb[2].to_bits(), c.image, c.uv, c.ids),
                    Err(e) => println!("{link}: {e}"),
                }
            }
            match lightmap::paktables::water_tables(&mut store, &coll) {
                Ok(w) => {
                    println!("{coll} water: type {:?} top {} floor {} FogMaxDepth {} → g_WaterTop_ByPlanes [{}], g_WaterDepth_FogMaxDepthInv_ByIds [({}, {} = {:08x})]", w.desc.name, w.desc.top, w.desc.floor, w.desc.fog_max_depth, w.top, w.depth_inv[0], w.depth_inv[1], w.depth_inv[1].to_bits());
                    println!("  fog LUT {} ({} texels; [0] {:?} [255] {:?}); transmittance {} ({} texels; [1024] {:?} [2047] {:?})", w.desc.fog_image, w.fog.len(), w.fog[0], w.fog[w.fog.len() - 1], w.desc.transmittance, w.transmittance.len(), w.transmittance[w.transmittance.len() / 2], w.transmittance[w.transmittance.len() - 1]);
                    if let Some(o) = f("--out-fog") {
                        std::fs::write(&o, w.fog.iter().flat_map(|p| p.iter().copied()).collect::<Vec<u8>>()).expect("--out-fog");
                        println!("  wrote {o} (RGBA8, {} texels)", w.fog.len());
                    }
                    if let Some(o) = f("--out-transmittance") {
                        std::fs::write(&o, w.transmittance.iter().flat_map(|p| p.iter().copied()).collect::<Vec<u8>>()).expect("--out-transmittance");
                        println!("  wrote {o} (RGBA8, {} texels)", w.transmittance.len());
                    }
                }
                Err(e) => println!("{coll} water: {e}"),
            }
        }
        "frustum-check" => {
            // lmtool frustum-check PASSCAP_ROOT [--frame N] [--manifest FROZEN.json] [--eps E] [--far-pad P] [--norm div|rsqrt] [--fma] [--expand-scale]
            //   the CPU light-camera fit (lightcam.rs) against the capture's SceneV cbuffers: every distinct camera of the
            //   draws log is back-solved to its world box (eye = centre, the half extents from the projected extents through
            //   the rules), the sun camera is recomputed from the capture's own caster geometry (the scene box = the union of
            //   the tiles + items) and every cbuffer value printed against the captured one in f32 ulps
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(127448);
            let env = root.join(format!("env/frame{frame}"));
            let rules = lightmap::lightcam::FitRules { expand_eps: f("--eps").map(|v| v.parse().unwrap()).unwrap_or(1e-4), far_pad: f("--far-pad").map(|v| v.parse().unwrap()).unwrap_or(5.0), normalise: match f("--norm").as_deref() { Some("rsqrt") => lightmap::lightcam::Normalise::MulRsqrt, Some("rsqrt64") => lightmap::lightcam::Normalise::MulRsqrtF64, Some("newton") => lightmap::lightcam::Normalise::MulRsqrtNewton, Some("xz") => lightmap::lightcam::Normalise::MulRsqrtXZ, Some("magic1") => lightmap::lightcam::Normalise::Magic1, Some("magic2") => lightmap::lightcam::Normalise::Magic2, Some("div") => lightmap::lightcam::Normalise::DivSqrt, _ => lightmap::lightcam::Normalise::MulRsqrt }, dot: if has("--fma") { lightmap::lightcam::DotOrder::Fma } else { lightmap::lightcam::DotOrder::Separate }, expand_by_scale: has("--expand-scale"), centre_from_half: !has("--centre-mean"), norm_forward: has("--norm-fwd"), norm_up: has("--norm-up"), game_basis: !has("--no-game-basis"), game_renorm: match f("--renorm").as_deref() { Some("none") => lightmap::lightcam::GameRenorm::None, Some("forward") => lightmap::lightcam::GameRenorm::Forward, Some("all") => lightmap::lightcam::GameRenorm::All, Some("cross") => lightmap::lightcam::GameRenorm::CrossOnly, _ => lightmap::lightcam::GameRenorm::None } };
            println!("rules: {rules:?}");
            let draws_bytes = lightmap::passdiff::read_entry_bytes(&root, &format!("logs/draws-frame{frame}.json")).expect("draws log");
            let draws: serde_json::Value = serde_json::from_slice(&draws_bytes).expect("draws json");
            let m4 = |v: &serde_json::Value| -> [[f32; 4]; 4] { let mut o = [[0f32; 4]; 4]; for i in 0..4 { for j in 0..4 { o[i][j] = v[i][j].as_f64().unwrap() as f32; } } o };
            let m43 = |v: &serde_json::Value| -> [[f32; 3]; 4] { let mut o = [[0f32; 3]; 4]; for i in 0..4 { for j in 0..3 { o[i][j] = v[i][j].as_f64().unwrap() as f32; } } o };
            let v4 = |v: &serde_json::Value| -> [f32; 4] { [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32, v[2].as_f64().unwrap() as f32, v[3].as_f64().unwrap_or(0.0) as f32] };
            // the distinct cameras (by eye + projection), in eid order
            struct Cap { eid: u64, eye: [f32; 4], w2c: [[f32; 3]; 4], proj: [[f32; 4]; 4], mm: [f32; 4], wpc: [[f32; 4]; 4] }
            let mut caps: Vec<Cap> = Vec::new();
            for e in draws.as_array().unwrap() {
                let Some(sv) = e["Vertex"]["cbuffers"].get("SceneV") else { continue };
                if sv.get("GbxV_EyeInWorld").is_none() || sv["GbxV_CameraIsOrtho"].as_i64() != Some(1) { continue; }
                let c = Cap { eid: e["eid"].as_u64().unwrap(), eye: v4(&sv["GbxV_EyeInWorld"]), w2c: m43(&sv["GbxV_WorldToCamera"]), proj: m4(&sv["GbxV_CameraProjection"]), mm: v4(&sv["GbxV_Camera_MinZ_MaxZ_InvRange_HasDeferredZ"]), wpc: m4(&sv["GbxV_WorldPrCamera"]) };
                if caps.iter().any(|k| k.eye == c.eye && k.proj == c.proj) { continue; }
                caps.push(c);
            }
            println!("{} distinct orthographic cameras in frame {frame}", caps.len());
            let report = |name: &str, cam: &lightmap::lightcam::OrthoCamera, cap: &Cap| {
                let u = lightmap::lightcam::ulps;
                let w2c = cam.world_to_camera();
                let p = cam.projection();
                let mm = cam.min_max_inv();
                let wpc = cam.world_pr_camera();
                let mut worst = 0i64;
                let mut lines = Vec::new();
                let mut push = |what: String, ours: f32, game: f32| { let d = u(ours, game); worst = worst.max(d.abs()); if d != 0 { lines.push(format!("      {what}: ours {ours:.9} game {game:.9} ({d:+} ulp)")); } };
                for k in 0..3 { push(format!("EyeInWorld[{k}]"), cam.eye[k], cap.eye[k]); }
                for i in 0..4 { for j in 0..3 { push(format!("WorldToCamera[{i}][{j}]"), w2c[i][j], cap.w2c[i][j]); } }
                for i in 0..4 { for j in 0..4 { push(format!("CameraProjection[{i}][{j}]"), p[i][j], cap.proj[i][j]); } }
                for k in 0..3 { push(format!("MinZ_MaxZ_InvRange[{k}]"), mm[k], cap.mm[k]); }
                for i in 0..4 { for j in 0..4 { push(format!("WorldPrCamera[{i}][{j}]"), wpc[i][j], cap.wpc[i][j]); } }
                println!("    {name}: worst {worst} ulp over 55 values{}", if lines.is_empty() { " — ALL BIT-IDENTICAL".to_string() } else { format!(", {} differ:", lines.len()) });
                for l in lines.iter().take(24) { println!("{l}"); }
            };
            // back-solve every camera's world box from its cbuffers: eye = centre; the light-space extents
            // (halfW, halfH, hd = −near) ÷ scale = |R|·h → three equations in (hx, hy, hz)
            for cap in &caps {
                let d = [cap.w2c[0][2], cap.w2c[1][2], cap.w2c[2][2]];
                let right = [cap.w2c[0][0], cap.w2c[1][0], cap.w2c[2][0]];
                let up = [cap.w2c[0][1], cap.w2c[1][1], cap.w2c[2][1]];
                let half_w = -1.0 / cap.proj[0][0] as f64;
                let half_h = 1.0 / cap.proj[1][1] as f64;
                let (near, far) = (cap.mm[0] as f64, cap.mm[1] as f64);
                let s = 1.0 + rules.expand_eps as f64;
                let ext = [half_w / s, half_h / s, -near / s];
                // solve |A|·h = ext, A rows = |right|, |up|, |d|
                let am = [[right[0].abs() as f64, right[1].abs() as f64, right[2].abs() as f64], [up[0].abs() as f64, up[1].abs() as f64, up[2].abs() as f64], [d[0].abs() as f64, d[1].abs() as f64, d[2].abs() as f64]];
                let det = |m: &[[f64; 3]; 3]| m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
                let dm = det(&am);
                let mut h = [0f64; 3];
                for k in 0..3 { let mut mk = am; for r in 0..3 { mk[r][k] = ext[r]; } h[k] = det(&mk) / dm; }
                println!("  eid {}: D ({:.5}, {:.5}, {:.5}) eye ({:.4}, {:.7}, {:.4}) halfW {half_w:.4} halfH {half_h:.4} near {near:.4} far {far:.4} (far + near = {:.5}) → box half ({:.4}, {:.4}, {:.4}) = y range [{:.4}, {:.4}]", cap.eid, d[0], d[1], d[2], cap.eye[0], cap.eye[1], cap.eye[2], far + near, h[0], h[1], h[2], cap.eye[1] as f64 - h[1], cap.eye[1] as f64 + h[1]);
                // the fit on the back-solved box (the arithmetic check: the eye and the rules; the box itself is the camera's)
                let mut b = lightmap::lightcam::Aabb::empty();
                b.add_point([cap.eye[0] - h[0] as f32, cap.eye[1] - h[1] as f32, cap.eye[2] - h[2] as f32]);
                b.add_point([cap.eye[0] + h[0] as f32, cap.eye[1] + h[1] as f32, cap.eye[2] + h[2] as f32]);
                let cam = lightmap::lightcam::fit_camera(&b, d, &rules);
                report("fit on the back-solved box", &cam, cap);
            }
            // the sun camera from the capture's own geometry: the scene box = the union of the tiles + the items (the
            // shadow-map casters of VS 5394 — the same buffers as shadow-check)
            // --scene xmin,ymin,zmin,xmax,ymax,zmax: the scene box given (a frame without the sun shadow pass / env: the
            // peel cameras of the other frames against frame 127448's casters)
            let mut scene_override: Option<lightmap::lightcam::Aabb> = f("--scene").map(|s| { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); lightmap::lightcam::Aabb { min: [v[0], v[1], v[2]], max: [v[3], v[4], v[5]] } });
            // --tiles-from-pak FILE:KEY [--collection BlueBay] [--zone Sea] [--cell-y 5] [--yoff -40]: S from the map + pak alone
            // (lmtiles::tile_records + the items, the FUN_140184fa0 fold) — the capture-less route; its exact {c, h} feeds the fits
            let mut scene_ch_override: Option<lightmap::lmtiles::CBox> = None;
            if let (Some(pak), Some(map)) = (f("--tiles-from-pak"), f("--map")) {
                let (pak_path, key) = pak.rsplit_once(':').expect("--tiles-from-pak FILE:KEY");
                let mut store = mapgeom::store::DataStore::empty();
                store.add_pak(pak_path, key).expect("pak");
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map));
                let chunks = tmmaps::gbx::all_skip_chunks(&mf.gbx.body);
                let gen: Vec<(String, u32)> = chunks.iter().find(|(c, ..)| *c == 0x0304_3043).and_then(|&(_, _, payload, size)| tmmaps::map::genealogy_full(&mf.gbx.body[payload..payload + size]).ok()).map(|recs| recs.into_iter().map(|r| (r.current, r.dir)).collect()).unwrap_or_default();
                let size = [mf.size[0].max(0) as usize, mf.size[1].max(0) as usize, mf.size[2].max(0) as usize];
                let tiles = lightmap::lmtiles::tile_records(&mut store, &f("--collection").unwrap_or_else(|| "BlueBay".into()), size, &gen, &f("--zone").unwrap_or_else(|| "Sea".into()), f("--cell-y").map(|v| v.parse().unwrap()).unwrap_or(5.0), f("--yoff").map(|v| v.parse().unwrap()).unwrap_or(-40.0), 0.5).expect("tile records");
                let scene = lightmap::geometry::Scene::from_map(&map).expect("map scene");
                let mut recs: Vec<lightmap::lmtiles::BlockRecord> = tiles.iter().map(|t| t.4).collect();
                recs.extend(lightmap::lmtiles::item_records(&scene, 1.0, false).iter().filter_map(|it| it.record));
                let s = lightmap::lmtiles::scene_box(&recs);
                println!("SCENE BOX S from the pak tiles ({}) + the items ({}): c {:?} h {:?} = [{}, {}] × [{}, {}] × [{}, {}]", tiles.len(), recs.len() - tiles.len(), s.c, s.h, s.min()[0], s.max()[0], s.min()[1], s.max()[1], s.min()[2], s.max()[2]);
                scene_ch_override = Some(s);
                scene_override = Some(s.aabb());
            }
            let mtxt = std::fs::read_to_string(f("--manifest").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("MANIFEST.json"))).expect("MANIFEST.json");
            let mval: serde_json::Value = serde_json::from_str(&lightmap::passdiff::repair_truncated_json(&mtxt)).expect("manifest json");
            let shadow_ent = mval["passes"].as_array().unwrap().iter().find(|e| e["pass"].as_str() == Some("sun_shadow") && e["frame"].as_u64() == Some(frame as u64));
            let (eid_first, eid_last) = shadow_ent.map(|s| (s["eid_first"].as_u64().unwrap_or(0), s["eid_last"].as_u64().unwrap_or(u64::MAX))).unwrap_or((u64::MAX, 0));
            let mesh_json: serde_json::Value = if scene_override.is_some() { serde_json::Value::Array(vec![]) } else { serde_json::from_str(&lightmap::passdiff::repair_truncated_json(&std::fs::read_to_string(env.join("mesh.json")).expect("mesh.json"))).expect("mesh.json") };
            let mut scene_box = scene_override.unwrap_or_else(lightmap::lightcam::Aabb::empty);
            let mut items_box = lightmap::lightcam::Aabb::empty();
            let mut sun_cap: Option<&Cap> = None;
            for e in draws.as_array().unwrap() {
                if scene_override.is_some() { break; }
                let eid = e["eid"].as_u64().unwrap_or(0);
                if eid < eid_first || eid > eid_last || !e["flags"].as_str().map(|s| s.contains("Drawcall")).unwrap_or(false) { continue; }
                let vs_id = e["Vertex"]["shader"].as_str().unwrap_or("");
                if vs_id != "5394" && vs_id != "14613" { continue; } // the block records: the tiles + the items (the vegetation item = trunk + its two leaf meshes)
                if sun_cap.is_none() { sun_cap = caps.iter().find(|c| c.eid <= eid && e["Vertex"]["cbuffers"]["SceneV"]["GbxV_EyeInWorld"].as_array().map(|v| v[0].as_f64().unwrap() as f32 == c.eye[0] && v[1].as_f64().unwrap() as f32 == c.eye[1]).unwrap_or(false)); }
                let drawv = &e["Vertex"]["cbuffers"]["DrawV"]["g_CBufferV_Draw"];
                let instance_start = drawv["InstanceStart"].as_u64().unwrap_or(0) as u32;
                let rec = mesh_json.as_array().unwrap().iter().find(|r| r["eid"].as_u64() == Some(eid)).unwrap_or_else(|| panic!("mesh.json has no eid {eid}"));
                let vbs = rec["vertex_buffers"].as_array().unwrap();
                let vb = std::fs::read(env.join("mesh").join(vbs[0]["file"].as_str().unwrap())).expect("vb");
                let stride = vbs[0]["stride"].as_u64().unwrap() as usize;
                let il = rec["input_layout"].as_array().unwrap();
                let pos_off = il.iter().find(|x| x["semantic"].as_str() == Some("POSITION")).map(|x| x["offset"].as_u64().unwrap() as usize).unwrap();
                let ib = std::fs::read(env.join("mesh").join(rec["vsout"]["index_file"].as_str().unwrap())).expect("indices");
                let mesh = lightmap::shadowmap::CasterMesh::parse(&vb, stride, pos_off, None, &ib);
                let dyna = std::fs::read(env.join("bufs").join(format!("e{eid:06}_Vertex_srv0_2185.bin"))).unwrap_or_default();
                let sm = std::fs::read(env.join("bufs").join(format!("e{eid:06}_Vertex_srv1_17163.bin"))).unwrap_or_default();
                let tables = lightmap::shadowmap::InstanceTables::parse(&dyna, &sm);
                let inst = e["inst"].as_u64().unwrap_or(0).max(1) as u32;
                // the instance's world box = the transformed vertex box (|R|·h + T on the mesh's own box: the record form)
                let mut mesh_box = lightmap::lightcam::Aabb::empty();
                for p in &mesh.pos { mesh_box.add_point(*p); }
                let (mc, mh) = (mesh_box.centre(), mesh_box.half());
                for iid in 0..inst {
                    let Some(rows) = tables.rows(instance_start, iid, lightmap::shadowmap::Arith::Fma) else { continue };
                    let c = [rows[0][0] * mc[0] + rows[0][1] * mc[1] + rows[0][2] * mc[2] + rows[0][3], rows[1][0] * mc[0] + rows[1][1] * mc[1] + rows[1][2] * mc[2] + rows[1][3], rows[2][0] * mc[0] + rows[2][1] * mc[1] + rows[2][2] * mc[2] + rows[2][3]];
                    let h = [rows[0][0].abs() * mh[0] + rows[0][1].abs() * mh[1] + rows[0][2].abs() * mh[2], rows[1][0].abs() * mh[0] + rows[1][1].abs() * mh[1] + rows[1][2].abs() * mh[2], rows[2][0].abs() * mh[0] + rows[2][1].abs() * mh[1] + rows[2][2].abs() * mh[2]];
                    let b = lightmap::lightcam::Aabb { min: [c[0] - h[0], c[1] - h[1], c[2] - h[2]], max: [c[0] + h[0], c[1] + h[1], c[2] + h[2]] };
                    scene_box.add_box(&b);
                    if inst == 1 { items_box.add_box(&b); }
                }
                println!("  scene box after eid {eid} ({inst} instance(s)): min {:?} max {:?}", scene_box.min, scene_box.max);
            }
            // with --scene the caster walk is skipped: the sun camera is the first camera inside the manifest's sun_shadow eid range
            if scene_override.is_some() && sun_cap.is_none() { sun_cap = caps.iter().find(|c| c.eid >= eid_first && c.eid <= eid_last); }
            println!("SCENE BOX from the casters: centre {:?} half {:?}; items' box: centre {:?} half {:?}", scene_box.centre(), scene_box.half(), items_box.centre(), items_box.half());
            // the PROBE GRID box (the world peel's focus box = the scene box ∪ this one, RE 5): the probe draw's
            // ProbeToShadow (PS 17151, grid coords → the sun shadow map's uvz) times the inverse of the sun's
            // WorldPw01Shadow (PS 15187's cbuffer) = GridToWorld: the cell size on the diagonal, the origin in the last
            // row; the grid is the 3D target's size (32 × 16 × 32)
            let probe = draws.as_array().unwrap().iter().find(|e| e["Pixel"]["shader"].as_str() == Some("17151"));
            let sun_direct = draws.as_array().unwrap().iter().find(|e| e["Pixel"]["shader"].as_str() == Some("15187"));
            let mut grid_box: Option<lightmap::lightcam::Aabb> = None;
            if let (Some(pr), Some(sd)) = (probe, sun_direct) {
                let pts = m43(&pr["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"]["ProbeToShadow"]);
                // TMapShadow at the probe draw is the CURRENT direction's world-peel depth (texture 17089 is reused): its WorldPw01Shadow
                // the peel's WorldPw01Shadow = the first accumulate draw (PS 17112) AFTER the probe draw (same direction block)
                let probe_eid = pr["eid"].as_u64().unwrap();
                let peel_pw = draws.as_array().unwrap().iter().filter(|e| e["Pixel"]["shader"].as_str() == Some("17112") && e["eid"].as_u64().unwrap_or(0) > probe_eid).next().map(|e| m4(&e["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"]["WorldPw01Shadow"]));
                let pw = match (has("--probe-sun"), peel_pw) { (false, Some(p)) => { println!("  (through the WorldPw01Shadow of the first accumulate draw after the probe draw {probe_eid})"); p } _ => m4(&sd["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"]["WorldPw01Shadow"]) };
                let dims = [pr["outputs"][0]["w"].as_u64().unwrap_or(32) as f64, pr["outputs"][0]["h"].as_u64().unwrap_or(16) as f64, pr["outputs"][0]["d"].as_u64().unwrap_or(32) as f64];
                // invert the 4×4 (row-vector affine: rotation/scale 3×3 + translation row) in f64
                let mut a = [[0f64; 3]; 3];
                for i in 0..3 { for j in 0..3 { a[i][j] = pw[i][j] as f64; } }
                let t = [pw[3][0] as f64, pw[3][1] as f64, pw[3][2] as f64];
                let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0]) + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
                let mut inv = [[0f64; 3]; 3];
                inv[0][0] = (a[1][1] * a[2][2] - a[1][2] * a[2][1]) / det; inv[0][1] = (a[0][2] * a[2][1] - a[0][1] * a[2][2]) / det; inv[0][2] = (a[0][1] * a[1][2] - a[0][2] * a[1][1]) / det;
                inv[1][0] = (a[1][2] * a[2][0] - a[1][0] * a[2][2]) / det; inv[1][1] = (a[0][0] * a[2][2] - a[0][2] * a[2][0]) / det; inv[1][2] = (a[0][2] * a[1][0] - a[0][0] * a[1][2]) / det;
                inv[2][0] = (a[1][0] * a[2][1] - a[1][1] * a[2][0]) / det; inv[2][1] = (a[0][1] * a[2][0] - a[0][0] * a[2][1]) / det; inv[2][2] = (a[0][0] * a[1][1] - a[0][1] * a[1][0]) / det;
                // world = (uvz − t) · inv ; grid → uvz = g · P + p_t ; so GridToWorld rows = P_rows · inv, origin = (p_t − t) · inv
                let mul = |v: [f64; 3]| [v[0] * inv[0][0] + v[1] * inv[1][0] + v[2] * inv[2][0], v[0] * inv[0][1] + v[1] * inv[1][1] + v[2] * inv[2][1], v[0] * inv[0][2] + v[1] * inv[1][2] + v[2] * inv[2][2]];
                let rows: Vec<[f64; 3]> = (0..3).map(|i| mul([pts[i][0] as f64, pts[i][1] as f64, pts[i][2] as f64])).collect();
                let origin = mul([pts[3][0] as f64 - t[0], pts[3][1] as f64 - t[1], pts[3][2] as f64 - t[2]]);
                println!("PROBE GRID (eid {} ProbeToShadow through the sun's WorldPw01Shadow⁻¹): GridToWorld rows {:?} {:?} {:?}, origin ({:.4}, {:.4}, {:.4}), dims {:?}", pr["eid"], rows[0].map(|v| (v * 1e4).round() / 1e4), rows[1].map(|v| (v * 1e4).round() / 1e4), rows[2].map(|v| (v * 1e4).round() / 1e4), origin[0], origin[1], origin[2], dims);
                let cell = [rows[0][0], rows[1][1], rows[2][2]];
                let lo = [origin[0] - 0.5 * cell[0], origin[1] - 0.5 * cell[1], origin[2] - 0.5 * cell[2]];
                let hi = [origin[0] + (dims[0] - 0.5) * cell[0], origin[1] + (dims[1] - 0.5) * cell[1], origin[2] + (dims[2] - 0.5) * cell[2]];
                println!("  cells (i − 0.5)·cell + origin, i = 0..dims: box [{:.4}, {:.4}] × [{:.4}, {:.4}] × [{:.4}, {:.4}]", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
                grid_box = Some(lightmap::lightcam::Aabb { min: [lo[0] as f32, lo[1] as f32, lo[2] as f32], max: [hi[0] as f32, hi[1] as f32, hi[2] as f32] });
            }
            if let Some(cap) = sun_cap {
                let d = [cap.w2c[0][2], cap.w2c[1][2], cap.w2c[2][2]];
                let cam = match scene_ch_override { Some(s) => lightmap::lightcam::fit_camera_ch(s.c, s.h, d, &rules), None => lightmap::lightcam::fit_camera(&scene_box, d, &rules) };
                println!("SUN CAMERA (eid {}) from the scene box and the rules:", cap.eid);
                report("sun camera", &cam, cap);
            }
            // the PEEL cameras: the world peel's box W = the scene box with its y max raised to --world-ymax (138.0 read off
            // the capture: the source record is RE 5's), the fitted peel's box F = the items' x/z with the scene's y range
            // (the tiling rule's one cell on this map: the union of the item records' x/z clipped to the cell, y = the box's)
            let world_ymax: f32 = f("--world-ymax").map(|v| v.parse().unwrap()).unwrap_or(138.0);
            let mut w_box = scene_box;
            w_box.max[1] = world_ymax;
            let mut f_box = lightmap::lightcam::Aabb { min: [items_box.min[0], scene_box.min[1], items_box.min[2]], max: [items_box.max[0], scene_box.max[1], items_box.max[2]] };
            // --f-box xmin,zmin,xmax,zmax: the item RECORDS' x/z box (the models' own bounding boxes, not the meshes' vertices)
            if let Some(s) = f("--f-box") { let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect(); f_box.min[0] = v[0]; f_box.min[2] = v[1]; f_box.max[0] = v[2]; f_box.max[2] = v[3]; }
            // --map MAP.Gbx: the item RECORDS from the map's own items THE GAME'S WAY (lmtiles: the CPlugTree box = the
            // stored visual boxes copied/unioned in geom order, the mobil Iso4 by RE 4's pose chain, FUN_140185f70) and the
            // fitted peel's box from the TILING RULE (FUN_140230080, --s = the chart allocation scale in layout units/m,
            // default the editor's 31.75 for pwc-day) — the tile's own {centre, half} feeds the fit (no Aabb round trip).
            // --b-records keeps engineer B's route (mapgeom Xform, Aabb re-centring); --lod0 restricts the model box to
            // the LOD-0 geoms; --global-quality G (1.0 = the map's own objects)
            let mut f_tile: Option<lightmap::lmtiles::CBox> = None;
            let mut w_ch: Option<lightmap::lmtiles::CBox> = None;
            if let Some(map) = f("--map") {
                let scene = lightmap::geometry::Scene::from_map(&map).expect("map scene");
                if has("--b-records") {
                    let mut recs: Vec<([f32; 3], [f32; 3])> = Vec::new();
                    for inst in &scene.instances {
                        let mdl = &scene.models[inst.model];
                        if mdl.stored_boxes.is_empty() { println!("  item {} ({}): no stored bounding box", inst.item, inst.model_name); continue; }
                        let boxes: Vec<([f32; 3], [f32; 3])> = if has("--record-per-visual") { mdl.stored_boxes.clone() } else {
                            let u = lightmap::lightcam::union_of_records(&mdl.stored_boxes);
                            vec![(u.centre(), u.half())]
                        };
                        for (c, h) in boxes {
                            let (wc, wh) = lightmap::lightcam::record_box(c, h, &inst.xf);
                            println!("  item {} ({}): model box centre {:?} half {:?} → record centre {:?} half {:?}", inst.item, inst.model_name, c, h, wc, wh);
                            recs.push((wc, wh));
                        }
                    }
                    let rec_union = lightmap::lightcam::union_of_records(&recs);
                    println!("  item records' union (B's route): min {:?} max {:?}", rec_union.min, rec_union.max);
                    if !rec_union.is_empty() {
                        f_box.min[0] = rec_union.min[0]; f_box.min[2] = rec_union.min[2]; f_box.max[0] = rec_union.max[0]; f_box.max[2] = rec_union.max[2];
                    }
                } else {
                    let gq: f32 = f("--global-quality").map(|v| v.parse().unwrap()).unwrap_or(1.0);
                    let items = lightmap::lmtiles::item_records(&scene, gq, has("--lod0"));
                    let mut recs: Vec<lightmap::lmtiles::BlockRecord> = Vec::new();
                    for it in &items {
                        match (&it.model_box, &it.record) {
                            (Some(mb), Some(r)) => {
                                println!("  item {} ({}): visual boxes {:?}; model box c {:?} h {:?}; Iso4 rows {:?} {:?} {:?} t {:?} → record c {:?} h {:?} q {:.4}", it.item, it.model_name, scene.models[scene.instances.iter().find(|i| i.item == it.item).unwrap().model].stored_boxes_all, mb.c, mb.h, &it.iso4[0..3], &it.iso4[3..6], &it.iso4[6..9], &it.iso4[9..12], r.world.c, r.world.h, r.quality);
                                recs.push(*r);
                            }
                            _ => println!("  item {} ({}): no stored bounding box", it.item, it.model_name),
                        }
                    }
                    let s_alloc: f32 = f("--s").map(|v| v.parse().unwrap()).unwrap_or(31.75);
                    // the scene box for the tiling: the captured casters' (the tiles are not in the map's item list)
                    // the captured casters give S as min/max; the game holds {c, h} — --scene-centre mean|half picks the round
                    // trip (half = min + (max − min)·0.5, the sun camera's bit-exact form)
                    let scene_ch = if let Some(s) = scene_ch_override { s } else if f("--scene-centre").as_deref() == Some("mean") { lightmap::lmtiles::CBox::from_min_max(scene_box.min, scene_box.max) } else { lightmap::lmtiles::CBox::new(scene_box.centre_from_half(), scene_box.half()) };
                    let params = lightmap::lmtiles::TileParams::pwc_day(s_alloc);
                    let t = lightmap::lmtiles::peel_tiling(&scene_ch, &recs, &params);
                    println!("  TILING (s {s_alloc}): ext {:.1} layout units → target {}², n {} → {} fitted tile(s)", t.ext, t.size, t.n, t.tiles.len());
                    for tile in &t.tiles { println!("    tile c {:?} h {:?} = x [{}, {}] z [{}, {}]", tile.c, tile.h, tile.min()[0], tile.max()[0], tile.min()[2], tile.max()[2]); }
                    if let Some(tile) = t.tiles.first() {
                        f_tile = Some(*tile);
                        f_box = tile.aabb();
                    }
                    // the WORLD peel box from the map too: S ∪ the probe chunks' AABB (probechunk.rs — the map's size words, the
                    // collection's block size --block-size, the decoration's base height --offset, the level --level-h)
                    if !has("--no-probe-from-map") {
                        let v3 = |s: String| -> [f32; 3] { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] };
                        let mf = tmmaps::map::MapFile::load(std::path::Path::new(&map));
                        let size = [mf.size[0].max(0) as u32, mf.size[1].max(0) as u32, mf.size[2].max(0) as u32];
                        let bs = f("--block-size").map(v3).unwrap_or([32.0, 8.0, 32.0]);
                        let off = f("--offset").map(v3).unwrap_or([0.0, -40.0, 0.0]);
                        let h: f32 = f("--level-h").map(|v| v.parse().unwrap()).unwrap_or(0.0);
                        let (g, _boxes, c, aabb) = lightmap::probechunk::for_records(size, bs, off, h, &recs, &scene_ch, 2048);
                        println!("  PROBE GRID from the map: {} × {} × {} probes, cell {:?}, first probe {:?}; {} chunk(s), atlas {:?}", g.n[0], g.n[1], g.n[2], g.cell, g.origin, c.records.len(), c.atlas);
                        for r in &c.records { println!("    chunk {:?}: world origin of atlas index 0 {:?}, atlas range {:?}..{:?}", r.chunk, r.origin, r.amin, r.amax); }
                        if let Some(b) = aabb {
                            let w = lightmap::lmtiles::world_peel_box(&scene_ch, Some(&b));
                            println!("  WORLD PEEL BOX = S ∪ chunks AABB: c {:?} h {:?} = y [{}, {}]", w.c, w.h, w.min()[1], w.max()[1]);
                            w_ch = Some(w);
                            w_box = w.aabb();
                        }
                    }
                }
            }
            println!("PEEL boxes: W min {:?} max {:?}; F min {:?} max {:?}", w_box.min, w_box.max, f_box.min, f_box.max);
            // the ORIGINAL direction of each peel camera: the game re-normalises D inside the basis (FUN_140186d40), so the
            // cbuffer's forward is d̂ = D · fl(1/√|D|²), not the table's D — the fit must start from the table's
            let table_dirs: Vec<[f32; 3]> = lightmap::dome::PointSets::load(&f("--points").unwrap_or_else(lightmap::dome::default_path)).ok().and_then(|ps| {
                let q: u32 = f("--quality").map(|v| v.parse().unwrap()).unwrap_or(3);
                let mut all = Vec::new();
                for sw in 0..3 { if let Some(d) = lightmap::dome::sweep_directions(&ps, q, sw, false) { all.extend(d); } }
                Some(all)
            }).unwrap_or_default();
            let original_dir = |dhat: [f32; 3]| -> Option<[f32; 3]> {
                table_dirs.iter().copied().filter(|t| { let c = t[0] * dhat[0] + t[1] * dhat[1] + t[2] * dhat[2]; c > 0.999_999 }).min_by(|a, b| { let da = (a[0] - dhat[0]).abs() + (a[1] - dhat[1]).abs() + (a[2] - dhat[2]).abs(); let db = (b[0] - dhat[0]).abs() + (b[1] - dhat[1]).abs() + (b[2] - dhat[2]).abs(); da.partial_cmp(&db).unwrap() })
            };
            println!("{} table directions loaded for the original-D lookup", table_dirs.len());
            for cap in &caps {
                if cap.eye[0] == 0.0 && cap.eye[2] == 0.0 { continue; }
                if sun_cap.map(|s| s.eid == cap.eid).unwrap_or(false) { continue; }
                let dhat = [cap.w2c[0][2], cap.w2c[1][2], cap.w2c[2][2]];
                let d = match (has("--dhat"), original_dir(dhat)) { (false, Some(o)) => { println!("  original D from the table: ({:.9}, {:.9}, {:.9}) vs the cbuffer's forward ({:.9}, {:.9}, {:.9}): {:+}/{:+}/{:+} ulp", o[0], o[1], o[2], dhat[0], dhat[1], dhat[2], lightmap::lightcam::ulps(o[0], dhat[0]), lightmap::lightcam::ulps(o[1], dhat[1]), lightmap::lightcam::ulps(o[2], dhat[2])); o } _ => dhat };
                // which box: the eye tells (W's centre y 71 vs F's 49.75)
                let (name, b) = if (cap.eye[0] - w_box.centre()[0]).abs() < 1.0 && (cap.eye[2] - w_box.centre()[2]).abs() < 1.0 { ("world peel", w_box) } else { ("fitted peel", f_box) };
                // the fitted peel from the tile record's own {centre, half} when the tiling rule produced it
                let cam = match (name, f_tile, w_ch) { ("fitted peel", Some(tile), _) => lightmap::lightcam::fit_camera_ch(tile.c, tile.h, d, &rules), ("world peel", _, Some(w)) if !has("--world-aabb") => lightmap::lightcam::fit_camera_ch(w.c, w.h, d, &rules), _ => lightmap::lightcam::fit_camera(&b, d, &rules) };
                println!("PEEL CAMERA eid {} ({name}, D ({:.4}, {:.4}, {:.4})):", cap.eid, d[0], d[1], d[2]);
                report(name, &cam, cap);
                // the lookup matrix of this camera's accumulate draws (PS 17112 with the same PeelDirInW, the first one
                // after this camera's eid whose WorldPw01Shadow's z column matches the camera's forward)
                let pw01 = cam.world_pw01_shadow(4096, 4096);
                let acc = draws.as_array().unwrap().iter().filter(|e| e["Pixel"]["shader"].as_str() == Some("17112") && e["eid"].as_u64().unwrap_or(0) > cap.eid).find(|e| { let m = m4(&e["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"]["WorldPw01Shadow"]); (m[3][2] - pw01[3][2]).abs() < 1e-3 && (m[0][0] - pw01[0][0]).abs() < 1e-6 });
                match acc {
                    Some(e) => {
                        let m = m4(&e["Pixel"]["cbuffers"]["ShaderP"]["g_CBufferP"]["WorldPw01Shadow"]);
                        let mut worst = 0i64;
                        let mut lines = Vec::new();
                        for i in 0..4 { for j in 0..4 { let dlt = lightmap::lightcam::ulps(pw01[i][j], m[i][j]); worst = worst.max(dlt.abs()); if dlt != 0 { lines.push(format!("      WorldPw01Shadow[{i}][{j}]: ours {:.9} game {:.9} ({dlt:+} ulp)", pw01[i][j], m[i][j])); } } }
                        println!("    WorldPw01Shadow of the accumulate eid {}: worst {worst} ulp over 16 values{}", e["eid"], if lines.is_empty() { " — ALL BIT-IDENTICAL".to_string() } else { format!(", {} differ:", lines.len()) });
                        for l in lines { println!("{l}"); }
                    }
                    None => println!("    (no accumulate draw with this camera's WorldPw01Shadow in the frame)"),
                }
            }
        }
        "prepass-check" => {
            // lmtool prepass-check PASSCAP_ROOT [--frame 127447] [--run K] [--all-runs] [--coverage] [--bc1 ideal|expand8-trunc|expand8-round]
            //   [--aniso N] [--weight-bits B|none] [--our-ids] [--show N] [--alpha-report] — ROW 1: the attribute pre-pass emulated draw by
            //   draw (prepass.rs) against every banked snapshot of the frame (prepass_check.rs)
            lightmap::prepass_check::run(a.clone());
        }
        "e2e-check" => {
            // lmtool e2e-check PASSCAP_ROOT [--pre-frame 127447] [--frame 127448] [--skip-prepass] [--dump-dir DIR] [--tol F]: the transcribed
            //   passes CHAINED on our own outputs (pre-pass → MDiffuse → shadow map → direct sun → ILightInput chain), each stage against
            //   its captured intermediate, the first divergent stage named (e2e.rs)
            lightmap::e2e::run(a.clone());
        }
        "chain-final" => {
            // lmtool chain-final DIR PASSCAP_ROOT [--map SAVE.Map.Gbx] [--frame 74490]: the chain's tail on OUR finalised coefficient images
            //   (the bake's --chain-final-dir): PS 1034, PS 1332 × 8, the max reduce, CS 23025, the file writer — against the captured
            //   finalisation and the save's blobs (e2e.rs)
            lightmap::e2e::chain_final(a.clone());
        }
        "texstat" => {
            // lmtool texstat FILE.dds[.gz]: per mip the min / mean / max of each channel (a look at a texture the pass samples)
            let t = lightmap::texsample::load_dds(std::path::Path::new(&a[1]), lightmap::texsample::Bc1Decode::Ideal).unwrap_or_else(|e| panic!("{e}"));
            println!("{}: {:?} {}×{} × {} slices, {} mips in file, complete {}", a[1], t.fmt, t.w, t.h, t.slices, t.levels[0].len(), t.complete);
            for (si, sl) in t.levels.iter().enumerate() {
                for (mi, lv) in sl.iter().enumerate() {
                    let mut lo = [f32::MAX; 4]; let mut hi = [f32::MIN; 4]; let mut sum = [0f64; 4];
                    let mut a_hist = [0usize; 11];
                    for p in &lv.texels() { for k in 0..4 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); sum[k] += p[k] as f64; } a_hist[((p[3] * 10.0) as usize).min(10)] += 1; }
                    let n = lv.len() as f64;
                    println!("  slice {si} mip {mi} {}×{}: min [{:.3}, {:.3}, {:.3}, {:.3}] mean [{:.3}, {:.3}, {:.3}, {:.3}] max [{:.3}, {:.3}, {:.3}, {:.3}] alpha deciles {:?}", lv.w, lv.h, lo[0], lo[1], lo[2], lo[3], sum[0] / n, sum[1] / n, sum[2] / n, sum[3] / n, hi[0], hi[1], hi[2], hi[3], a_hist);
                    if mi >= 4 && si > 0 { break; }
                }
            }
        }
        "encode-check" => {
            // lmtool encode-check PASSCAP_ROOT [--frame N] [--fma] [--half-up] [--all]
            //   the transcribed finalisation (gpuenc.rs: the |rgb| max reduction + CS 23025 LmCompress_HBasis_YCbCr4)
            //   run on the capture's post-dilation coefficient images (final_04_after_dilate8_ps1332/frame<N>/) and
            //   compared BYTE FOR BYTE with the captured Y4 / Cb4 / Cr4 textures (final_06_encoded_rgba8_cs23025/) and
            //   with the captured MaxHdr buffer (final_05_maxreduce/…_uav0_*.bin); --all tries every arithmetic option
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let root = std::path::PathBuf::from(&a[1]);
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(74490);
            let txt = std::fs::read_to_string(root.join("MANIFEST.json")).expect("MANIFEST.json");
            let m = lightmap::passdiff::read_manifest(&txt).expect("manifest");
            let ent = |pass: &str| -> Vec<lightmap::passdump::Entry> { m.passes.iter().filter(|e| e.pass == pass && e.frame == Some(frame)).cloned().collect() };
            // the four coefficient images after the dilation, in the CS's SRV order t0..t3 = the manifest order of
            // the final_04 entries (24858, 24752, 24749, 24852: the rotated MRT0..3)
            // --chain: start from final_03 (before the dilation) and run OUR transcribed PS 1332 ×8 first — two closed
            // passes chained, our output feeding our next pass, compared with the capture at the end
            let chain = has("--chain");
            let dil = ent(if chain { "final_03_after_colormat_ps1034" } else { "final_04_after_dilate8_ps1332" });
            assert_eq!(dil.len(), 4, "final_0x entries for frame {frame}: {}", dil.len());
            let order = ["24858", "24752", "24749", "24852"];
            let mut imgs: Vec<lightmap::passdiff::Buf> = Vec::new();
            for id in order {
                let e = dil.iter().find(|e| e.file.contains(&format!("_{id}.dds"))).unwrap_or_else(|| panic!("no final image {id}"));
                let mut img = lightmap::passdiff::load_entry(&root, e).expect("load");
                if chain { for _ in 0..8 { img = lightmap::gpuenc::dilate_ps1332(&img); } }
                println!("image {id}: {}×{} ×{}{}", img.w, img.h, img.channels, if chain { " (final_03 → our 8 × PS 1332)" } else { "" });
                imgs.push(img);
            }
            let maxhdr = [lightmap::gpuenc::maxhdr_hbasis(&imgs[0]), lightmap::gpuenc::maxhdr_hbasis(&imgs[1]), lightmap::gpuenc::maxhdr_hbasis(&imgs[2]), lightmap::gpuenc::maxhdr_hbasis(&imgs[3])];
            println!("MaxHdr (ours, f16 max |rgb| per image): {maxhdr:?}");
            // the captured reduction buffer
            let red = m.passes.iter().find(|e| e.pass == "final_05_maxreduce_buffer" && e.frame == Some(frame));
            let mut mood = 7.519885063171387f32;
            if let Some(fe) = m.final_encode.as_ref() { if let Some(v) = fe.get("cbuffers").and_then(|c| c.get("Shader")).and_then(|c| c.get("g_CBufferC")).and_then(|c| c.get("Mood_MaxHdr")).and_then(|v| v.as_f64()) { mood = v as f32; } }
            println!("Mood_MaxHdr (cbuffer): {mood}");
            if let Some(r) = red {
                let b = lightmap::passdiff::read_entry_bytes(&root, &r.file).expect("maxhdr buffer");
                let cap: Vec<f32> = b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
                println!("MaxHdr (captured buffer 25127): {cap:?}");
                for k in 0..4 { if (cap[k] - maxhdr[k]).abs() > 0.0 { println!("  MISMATCH image {k}: ours {} captured {}", maxhdr[k], cap[k]); } }
            }
            let enc = ent("final_06_encoded_rgba8_cs23025");
            let load_id = |id: &str| -> lightmap::passdiff::Buf { let e = enc.iter().find(|e| e.file.contains(&format!("_{id}.dds"))).unwrap_or_else(|| panic!("no final_06 {id}")); lightmap::passdiff::load_entry(&root, e).expect("load") };
            let (y4c, cb4c, cr4c) = (load_id("25137"), load_id("25140"), load_id("25143"));
            let opts: Vec<lightmap::gpuenc::EncodeOpts> = if has("--all") {
                let mut v = Vec::new();
                for f16c in [false] { for f16 in [false] { for fma in [false, true] { for u in [lightmap::gpuenc::UnormRounding::Trunc12, lightmap::gpuenc::UnormRounding::NearestEven] { v.push(lightmap::gpuenc::EncodeOpts { fma, unorm: u, f16_store: f16, f16_c: f16c }); } } } }
                v
            } else {
                vec![lightmap::gpuenc::EncodeOpts { fma: has("--fma"), unorm: if has("--half-up") { lightmap::gpuenc::UnormRounding::HalfUp } else if has("--truncate") { lightmap::gpuenc::UnormRounding::Truncate } else if has("--nearest") { lightmap::gpuenc::UnormRounding::NearestEven } else { lightmap::gpuenc::UnormRounding::Trunc12 }, f16_store: has("--f16-store"), f16_c: has("--f16-c") }]
            };
            if has("--probe-mismatch") {
                // where do the ±1 bytes come from? print the first mismatching Y4 texels with the float value
                // before the UNORM store and its distance to the rounding boundary
                let o = lightmap::gpuenc::EncodeOpts::default();
                let inv = 1.0f32 / (maxhdr[0] / mood).max(1.0);
                let mut shown = 0;
                let mut hist = [0usize; 8]; // |frac - 0.5| buckets: <1e-5, <1e-4, <1e-3, <1e-2, <0.05, <0.1, <0.3, rest
                let mut total = 0usize;
                for y in 0..imgs[0].h { for x in 0..imgs[0].w { for k in 0..4 {
                    let m = inv * maxhdr[k];
                    let p = [imgs[k].get(x, y, 0), imgs[k].get(x, y, 1), imgs[k].get(x, y, 2)];
                    let mut c = [0f32; 3];
                    for i in 0..3 {
                        let v = p[i] / m;
                        if k == 0 { c[i] = v.max(0.0).sqrt().min(1.0); } else { let sgn = ((0.0 < v) as i32 - (v < 0.0) as i32) as f32; c[i] = (v.abs().sqrt() * sgn * 0.5 + 0.5).max(-1.0).min(1.0); }
                        if o.f16_c { c[i] = lightmap::gpufmt::quantise_f16(c[i], lightmap::gpufmt::Rounding::NearestEven); }
                    }
                    let yv = ((0.256788f32 * c[0] + 0.504129 * c[1]) + 0.097906 * c[2]) + 0.062745;
                    let ours = lightmap::gpuenc::unorm8(lightmap::gpuenc::store_value(yv.max(0.0), o), o.unorm);
                    let theirs = (y4c.get(x, y, k as u32) * 255.0).round() as u8;
                    if ours != theirs {
                        total += 1;
                        let scaled = yv.max(0.0).clamp(0.0, 1.0) * 255.0;
                        let frac = scaled - scaled.floor();
                        let d = (frac - 0.5).abs();
                        let b = if d < 1e-5 { 0 } else if d < 1e-4 { 1 } else if d < 1e-3 { 2 } else if d < 1e-2 { 3 } else if d < 0.05 { 4 } else if d < 0.1 { 5 } else if d < 0.3 { 6 } else { 7 };
                        hist[b] += 1;
                        if shown < 12 { println!("  ({x},{y}) k={k}: rgb {:?} /m {:?} c {:?} Y {yv:.7} ×255 = {scaled:.5} → ours {ours} theirs {theirs}", p, [p[0] / m, p[1] / m, p[2] / m], c); shown += 1; }
                    }
                }}}
                println!("Y4 mismatches {total}; |frac−0.5| histogram (<1e-5, <1e-4, <1e-3, <1e-2, <0.05, <0.1, <0.3, rest): {hist:?}");
                // the empirical rounding threshold per output byte: over ALL texels (k = 0 only), the largest of our
                // scaled values that the GPU still stored as b, and the smallest it stored as b + 1
                let mut lo_of_next = vec![f32::INFINITY; 256]; // min s with theirs = b+1  (indexed by b+1)
                let mut hi_of_this = vec![f32::NEG_INFINITY; 256]; // max s with theirs = b
                for y in 0..imgs[0].h { for x in 0..imgs[0].w {
                    let m = inv * maxhdr[0];
                    let p = [imgs[0].get(x, y, 0), imgs[0].get(x, y, 1), imgs[0].get(x, y, 2)];
                    let mut c = [(p[0] / m).max(0.0).sqrt().min(1.0), (p[1] / m).max(0.0).sqrt().min(1.0), (p[2] / m).max(0.0).sqrt().min(1.0)];
                    if o.f16_c { for i in 0..3 { c[i] = lightmap::gpufmt::quantise_f16(c[i], lightmap::gpufmt::Rounding::NearestEven); } }
                    let yv = ((0.256788f32 * c[0] + 0.504129 * c[1]) + 0.097906 * c[2]) + 0.062745;
                    let sc = lightmap::gpuenc::store_value(yv.max(0.0), o).clamp(0.0, 1.0) * 255.0;
                    let t = (y4c.get(x, y, 0) * 255.0).round() as usize;
                    if sc > hi_of_this[t] { hi_of_this[t] = sc; }
                    if sc < lo_of_next[t] { lo_of_next[t] = sc; }
                }}
                println!("byte b: max s stored as b | min s stored as b (threshold between b-1 and b lies in [max s(b-1), min s(b)])");
                for b in [16usize, 40, 66, 67, 100, 128, 153, 154, 180, 200, 230, 250] {
                    if hi_of_this[b - 1].is_finite() && lo_of_next[b].is_finite() { println!("  b={b}: s(b-1) max {:.4}  s(b) min {:.4}  → threshold − (b − 0.5) ∈ [{:.4}, {:.4}]", hi_of_this[b - 1], lo_of_next[b], hi_of_this[b - 1] - (b as f32 - 0.5), lo_of_next[b] - (b as f32 - 0.5)); }
                }
            }
            for o in opts {
                let e = lightmap::gpuenc::encode_ycbcr4([&imgs[0], &imgs[1], &imgs[2], &imgs[3]], maxhdr, mood, o);
                let (n, d, mx, ch) = lightmap::gpuenc::compare_u8(&e.y4, &y4c);
                println!("{o:?}: Y4  {}×{}: {d} of {n} bytes differ (max |Δ| {mx}) per channel {ch:?}", e.w, e.h);
                let (n, d, mx, ch) = lightmap::gpuenc::compare_u8(&e.cb4, &cb4c);
                println!("{o:?}: Cb4 {}×{}: {d} of {n} bytes differ (max |Δ| {mx}) per channel {ch:?}", e.w / 2, e.h / 2);
                let (n, d, mx, ch) = lightmap::gpuenc::compare_u8(&e.cr4, &cr4c);
                println!("{o:?}: Cr4 {}×{}: {d} of {n} bytes differ (max |Δ| {mx}) per channel {ch:?}", e.w / 2, e.h / 2);
            }
        }
        "bench" => {
            // lmtool bench MAP... [--quality Q] [--jobs J] [--out TABLE.md] [--dir OUTDIR] [-- EXTRA BAKE ARGS]:
            // bake every map with `lmtool bake --raster --game-peel --profile` (J at a time, each a child
            // process of this binary), read the profile lines, and write a Markdown timing table (map,
            // triangles, instances, layout texels, per-sweep seconds, total, peak RSS) — the fleet-sizing table
            let mut maps: Vec<String> = Vec::new();
            let mut extra: Vec<String> = Vec::new();
            let mut quality = "4".to_string();
            let mut jobs = 1usize;
            let mut table = "bench.md".to_string();
            let mut outdir = std::env::temp_dir().join("lmtool-bench");
            let mut i = 1;
            let mut in_extra = false;
            while i < a.len() {
                let x = &a[i];
                if in_extra { extra.push(x.clone()); i += 1; continue; }
                match x.as_str() {
                    "--" => in_extra = true,
                    "--quality" => { quality = a[i + 1].clone(); i += 1; }
                    "--jobs" => { jobs = a[i + 1].parse().expect("--jobs"); i += 1; }
                    "--out" => { table = a[i + 1].clone(); i += 1; }
                    "--dir" => { outdir = std::path::PathBuf::from(&a[i + 1]); i += 1; }
                    _ => maps.push(x.clone()),
                }
                i += 1;
            }
            std::fs::create_dir_all(&outdir).expect("bench dir");
            let exe = std::env::current_exe().expect("exe");
            #[derive(Default, Clone)]
            struct Row { map: String, tris: String, insts: String, texels: String, sweeps: Vec<f32>, total: f32, rss: String, ok: bool, note: String }
            let rows: std::sync::Mutex<Vec<Row>> = std::sync::Mutex::new(Vec::new());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let t_all = std::time::Instant::now();
            std::thread::scope(|sc| {
                for _ in 0..jobs.max(1) {
                    sc.spawn(|| loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if k >= maps.len() { break; }
                        let m = &maps[k];
                        let name = std::path::Path::new(m).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(m.clone());
                        let out = outdir.join(format!("{name}.baked.Map.Gbx"));
                        let t = std::time::Instant::now();
                        let mut cmd = std::process::Command::new(&exe);
                        cmd.arg("bake").arg(m).arg("--raster").arg("--quality").arg(&quality).arg("--game-peel").arg("--profile").arg("--out").arg(&out);
                        for e in &extra { cmd.arg(e); }
                        let output = cmd.output();
                        let wall = t.elapsed().as_secs_f32();
                        let mut row = Row { map: name.clone(), total: wall, ..Default::default() };
                        match output {
                            Ok(o) => {
                                row.ok = o.status.success();
                                let err = String::from_utf8_lossy(&o.stderr).to_string();
                                let log = outdir.join(format!("{name}.log"));
                                let _ = std::fs::write(&log, format!("{}
{}", String::from_utf8_lossy(&o.stdout), err));
                                for l in err.lines() {
                                    if let Some(r) = l.strip_prefix("scene: ") {
                                        // "N models, N instances, N triangles (+ N decoration) (…s)"
                                        let parts: Vec<&str> = r.split(", ").collect();
                                        if parts.len() >= 3 { row.insts = parts[1].split(' ').next().unwrap_or("").to_string(); row.tris = parts[2].split(' ').next().unwrap_or("").to_string(); }
                                    }
                                    if l.starts_with("peel: ") && l.contains(" layout texels over ") && row.texels.is_empty() { row.texels = l["peel: ".len()..].split(' ').next().unwrap_or("").to_string(); }
                                    if l.starts_with("profile [sweep ") { if let Some(p) = l.rfind("sweep total ") { row.sweeps.push(l[p + "sweep total ".len()..].trim_end_matches('s').parse().unwrap_or(0.0)); } }
                                    if let Some(r) = l.strip_prefix("peak RSS ") { row.rss = r.to_string(); }
                                }
                                if !row.ok { row.note = err.lines().rev().take(2).collect::<Vec<_>>().join(" | "); }
                            }
                            Err(e) => { row.note = e.to_string(); }
                        }
                        eprintln!("bench: {name}: {:.1}s ({}) sweeps {:?} RSS {}", wall, if row.ok { "ok" } else { "FAILED" }, row.sweeps, row.rss);
                        rows.lock().unwrap().push(row);
                    });
                }
            });
            let mut rows = rows.into_inner().unwrap();
            rows.sort_by(|a, b| a.map.cmp(&b.map));
            let mut md = String::new();
            md.push_str(&format!("# lmtool bench — quality {quality}, {} maps, {} at a time, extra args {:?}, {:.0} s wall in all ({})

", rows.len(), jobs, extra, t_all.elapsed().as_secs_f32(), std::env::var("HOSTNAME").unwrap_or_default()));
            md.push_str("| map | triangles | items | layout texels | sweeps (s) | total (s) | peak RSS | |
|---|---|---|---|---|---|---|---|
");
            for r in &rows {
                md.push_str(&format!("| {} | {} | {} | {} | {} | {:.1} | {} | {} |
", r.map, r.tris, r.insts, r.texels, r.sweeps.iter().map(|s| format!("{s:.1}")).collect::<Vec<_>>().join(" / "), r.total, r.rss, if r.ok { "ok".to_string() } else { format!("FAILED: {}", r.note) }));
            }
            std::fs::write(&table, &md).expect("bench table");
            print!("{md}");
        }
        "relight-batch" => {
            // lmtool relight-batch (MAP... | --maps LIST.txt) --out-dir DIR [--quality Q] [--jobs J] [--manifest relight.json]
            //   [--writer port|transcribed] [-- EXTRA BAKE ARGS]: THE RE-LIGHT DRIVER — every map baked by a child
            //   `lmtool bake --raster --game-peel --quality Q --profile` into DIR/<name>, then `lmtool check` on the
            //   written file (the structural self-check: chunk structure, blobs, probe grid, atlas fill, encoding
            //   sanity), J maps at a time; the manifest (JSON) carries per map the timings per sweep, the peak RSS,
            //   the peel cameras line (the tiling), the check's verdict and the bake's last lines on failure, plus a
            //   Markdown table beside it. --writer transcribed asks the bake for the transcribed file chain
            //   (finalisation → filecheck::file_images → the writer) once it is wired (--file-transcribed); the
            //   default is the port's own chunk encoder. Nothing is uploaded.
            // --boxes N [--hosts h1,h2,…] [--work DIR] [--keep-work]: THE DIRECTION-RANGE SPLIT (contrib.rs) — every
            //   sweep of a map runs as N range bakes (box k takes the directions k/N; local child processes, or
            //   `ssh host lmtool …` per host of --hosts, which must see this binary, the map and --work at the same
            //   paths), then one merge that replays the contributions in issue order and writes the sweep's field
            //   for the next sweep's boxes (the last one writes the map) — bit-identical to the single-box bake
            let mut maps: Vec<String> = Vec::new();
            let mut extra: Vec<String> = Vec::new();
            let mut quality = "4".to_string();
            let mut jobs = 1usize;
            let mut out_dir: Option<String> = None;
            let mut manifest: Option<String> = None;
            let mut writer = "port".to_string();
            let mut boxes = 1usize;
            let mut hosts: Vec<String> = Vec::new();
            let mut work: Option<String> = None;
            let mut keep_work = false;
            let mut i = 1;
            let mut in_extra = false;
            while i < a.len() {
                let x = &a[i];
                if in_extra { extra.push(x.clone()); i += 1; continue; }
                match x.as_str() {
                    "--" => in_extra = true,
                    "--quality" => { quality = a[i + 1].clone(); i += 1; }
                    "--jobs" => { jobs = a[i + 1].parse().expect("--jobs"); i += 1; }
                    "--out-dir" => { out_dir = Some(a[i + 1].clone()); i += 1; }
                    "--manifest" => { manifest = Some(a[i + 1].clone()); i += 1; }
                    "--writer" => { writer = a[i + 1].clone(); i += 1; }
                    "--boxes" => { boxes = a[i + 1].parse().expect("--boxes"); i += 1; }
                    "--hosts" => { hosts = a[i + 1].split(',').map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect(); i += 1; }
                    "--work" => { work = Some(a[i + 1].clone()); i += 1; }
                    "--keep-work" => keep_work = true,
                    "--maps" => { let txt = std::fs::read_to_string(&a[i + 1]).expect("--maps list"); for l in txt.lines() { let l = l.trim(); if !l.is_empty() && !l.starts_with('#') { maps.push(l.to_string()); } } i += 1; }
                    _ => maps.push(x.clone()),
                }
                i += 1;
            }
            let out_dir = std::path::PathBuf::from(out_dir.expect("--out-dir DIR"));
            std::fs::create_dir_all(&out_dir).expect("out dir");
            let manifest = manifest.map(std::path::PathBuf::from).unwrap_or_else(|| out_dir.join("relight-manifest.json"));
            if writer == "transcribed" { extra.push("--file-transcribed".to_string()); }
            let exe = std::env::current_exe().expect("exe");
            #[derive(Default, Clone)]
            struct Row { map: String, out: String, tris: String, insts: String, texels: String, cameras: String, peels: String, sweeps: Vec<f32>, bake_s: f32, check_s: f32, rss: String, bake_ok: bool, check_ok: bool, check_fails: usize, note: String, started: u64, host: String }
            let rows: std::sync::Mutex<Vec<Row>> = std::sync::Mutex::new(Vec::new());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let t_all = std::time::Instant::now();
            let host = std::env::var("HOSTNAME").unwrap_or_default();
            std::thread::scope(|sc| {
                for _ in 0..jobs.max(1) {
                    sc.spawn(|| loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if k >= maps.len() { break; }
                        let m = &maps[k];
                        let name = std::path::Path::new(m).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(m.clone());
                        let out = out_dir.join(&name);
                        let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                        let t = std::time::Instant::now();
                        let output: std::io::Result<std::process::Output> = if boxes <= 1 {
                            let mut cmd = std::process::Command::new(&exe);
                            cmd.arg("bake").arg(m).arg("--raster").arg("--quality").arg(&quality).arg("--game-peel").arg("--profile").arg("--out").arg(&out);
                            for e in &extra { cmd.arg(e); }
                            cmd.output()
                        } else {
                            // THE SPLIT: per sweep the N range bakes (local, or one per host), then the merge
                            let n_sweeps = lightmap::dome::sweep_counts(quality.parse().unwrap_or(3)).len().max(1);
                            let wroot = std::path::PathBuf::from(work.clone().unwrap_or_else(|| out_dir.join("work").to_string_lossy().to_string())).join(&name);
                            let _ = std::fs::create_dir_all(&wroot);
                            let mut all_err = String::new();
                            let mut all_out = String::new();
                            let mut ok = true;
                            for sw in 0..n_sweeps {
                                let field_prev = wroot.join(format!("field{}.bin", sw.wrapping_sub(1)));
                                let mut children: Vec<(usize, std::process::Child)> = Vec::new();
                                let t_sw = std::time::Instant::now();
                                for k in 0..boxes {
                                    let cdir = wroot.join(format!("s{sw}-box{k}"));
                                    let mut args: Vec<String> = vec!["bake".into(), m.clone(), "--raster".into(), "--quality".into(), quality.clone(), "--game-peel".into(), "--profile".into()];
                                    args.extend(extra.iter().cloned());
                                    args.extend(["--sweep-only".into(), sw.to_string(), "--dir-range".into(), format!("{k}/{boxes}"), "--contrib-out".into(), cdir.to_string_lossy().to_string(), "--out".into(), wroot.join(format!("s{sw}-box{k}.Map.Gbx")).to_string_lossy().to_string()]);
                                    if sw > 0 { args.extend(["--field-from".into(), field_prev.to_string_lossy().to_string()]); }
                                    let mut cmd = if hosts.is_empty() {
                                        let mut c = std::process::Command::new(&exe);
                                        c.args(&args);
                                        c
                                    } else {
                                        let host = &hosts[k % hosts.len()];
                                        let mut c = std::process::Command::new("ssh");
                                        c.arg("-o").arg("BatchMode=yes").arg(host).arg(exe.to_string_lossy().to_string());
                                        for x in &args { c.arg(shell_quote(x)); }
                                        c
                                    };
                                    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
                                    match cmd.spawn() {
                                        Ok(ch) => children.push((k, ch)),
                                        Err(e) => { all_err += &format!("sweep {sw} box {k}: spawn: {e}\n"); ok = false; }
                                    }
                                }
                                for (k, ch) in children {
                                    match ch.wait_with_output() {
                                        Ok(o) => {
                                            let err = String::from_utf8_lossy(&o.stderr).to_string();
                                            let _ = std::fs::write(wroot.join(format!("s{sw}-box{k}.log")), &err);
                                            if !o.status.success() { ok = false; all_err += &format!("sweep {sw} box {k} FAILED: {}\n", err.lines().rev().take(3).collect::<Vec<_>>().join(" | ")); }
                                        }
                                        Err(e) => { ok = false; all_err += &format!("sweep {sw} box {k}: {e}\n"); }
                                    }
                                }
                                let range_s = t_sw.elapsed().as_secs_f32();
                                if !ok { break; }
                                // THE MERGE of the sweep (the last one writes the map)
                                let t_m = std::time::Instant::now();
                                let mut cmd = std::process::Command::new(&exe);
                                cmd.arg("bake").arg(m).arg("--raster").arg("--quality").arg(&quality).arg("--game-peel").arg("--profile");
                                for e in &extra { cmd.arg(e); }
                                let dirs: Vec<String> = (0..boxes).map(|k| wroot.join(format!("s{sw}-box{k}")).to_string_lossy().to_string()).collect();
                                cmd.arg("--sweep-only").arg(sw.to_string()).arg("--merge-contrib").arg(dirs.join(",")).arg("--field-out").arg(wroot.join(format!("field{sw}.bin")));
                                if sw > 0 { cmd.arg("--field-from").arg(&field_prev); }
                                cmd.arg("--out").arg(if sw + 1 == n_sweeps { out.clone() } else { wroot.join(format!("merge{sw}.Map.Gbx")) });
                                match cmd.output() {
                                    Ok(o) => {
                                        let err = String::from_utf8_lossy(&o.stderr).to_string();
                                        all_out += &String::from_utf8_lossy(&o.stdout);
                                        // the merge's profile lines carry the sweep totals of the merge alone: the row's
                                        // sweep timings are the range bakes' wall + the merge's
                                        all_err += &err.lines().filter(|l| !l.starts_with("profile [sweep ")).collect::<Vec<_>>().join("\n");
                                        all_err += &format!("\nprofile [sweep {sw}]: split over {boxes} box(es): range bakes {range_s:.1}s, merge {:.1}s; sweep total {:.1}s\n", t_m.elapsed().as_secs_f32(), range_s + t_m.elapsed().as_secs_f32());
                                        if !o.status.success() { ok = false; all_err += &format!("sweep {sw} merge FAILED: {}\n", err.lines().rev().take(3).collect::<Vec<_>>().join(" | ")); }
                                        if !keep_work { for d in &dirs { let _ = std::fs::remove_dir_all(d); } }
                                    }
                                    Err(e) => { ok = false; all_err += &format!("sweep {sw} merge: {e}\n"); }
                                }
                                if !ok { break; }
                            }
                            if !keep_work && ok { let _ = std::fs::remove_dir_all(&wroot); }
                            Ok(std::process::Output { status: std::process::Command::new(if ok { "true" } else { "false" }).status().expect("status"), stdout: all_out.into_bytes(), stderr: all_err.into_bytes() })
                        };
                        let bake_s = t.elapsed().as_secs_f32();
                        let mut row = Row { map: m.clone(), out: out.to_string_lossy().to_string(), bake_s, started, host: host.clone(), ..Default::default() };
                        match output {
                            Ok(o) => {
                                row.bake_ok = o.status.success();
                                let err = String::from_utf8_lossy(&o.stderr).to_string();
                                let _ = std::fs::write(out_dir.join(format!("{name}.bake.log")), format!("{}\n{}", String::from_utf8_lossy(&o.stdout), err));
                                for l in err.lines() {
                                    if let Some(r) = l.strip_prefix("scene: ") {
                                        let parts: Vec<&str> = r.split(", ").collect();
                                        if parts.len() >= 3 { row.insts = parts[1].split(' ').next().unwrap_or("").to_string(); row.tris = parts[2].split(' ').next().unwrap_or("").to_string(); }
                                    }
                                    if l.starts_with("peel: ") && l.contains(" layout texels over ") && row.texels.is_empty() { row.texels = l["peel: ".len()..].split(' ').next().unwrap_or("").to_string(); }
                                    if let Some(r) = l.strip_prefix("peel cameras: ") { if r.contains("tiling at scale") { row.cameras = r.to_string(); } }
                                    if l.starts_with("peel: direction 1/") && row.peels.is_empty() { if let Some(p) = l.find('(') { row.peels = l[p + 1..].split(' ').next().unwrap_or("").to_string(); } }
                                    if l.starts_with("profile [sweep ") { if let Some(p) = l.rfind("sweep total ") { row.sweeps.push(l[p + "sweep total ".len()..].trim_end_matches('s').parse().unwrap_or(0.0)); } }
                                    if let Some(r) = l.strip_prefix("peak RSS ") { row.rss = r.to_string(); }
                                }
                                if !row.bake_ok { row.note = err.lines().rev().take(3).collect::<Vec<_>>().join(" | "); }
                            }
                            Err(e) => { row.note = e.to_string(); }
                        }
                        if row.bake_ok {
                            let t2 = std::time::Instant::now();
                            let chk = std::process::Command::new(&exe).arg("check").arg(&out).output();
                            row.check_s = t2.elapsed().as_secs_f32();
                            match chk {
                                Ok(o) => {
                                    let txt = format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                                    let _ = std::fs::write(out_dir.join(format!("{name}.check.log")), &txt);
                                    row.check_fails = txt.lines().filter(|l| l.contains("[FAIL]")).count();
                                    row.check_ok = o.status.success() && row.check_fails == 0;
                                    if !row.check_ok { row.note = txt.lines().filter(|l| l.contains("[FAIL]")).take(3).collect::<Vec<_>>().join(" | "); }
                                }
                                Err(e) => { row.note = format!("check: {e}"); }
                            }
                        }
                        eprintln!("relight: {name}: bake {:.1}s ({}), check {:.1}s ({}), sweeps {:?}, RSS {}, peels {}", row.bake_s, if row.bake_ok { "ok" } else { "FAILED" }, row.check_s, if row.check_ok { "ok" } else if row.bake_ok { "FAIL" } else { "-" }, row.sweeps, row.rss, row.peels);
                        rows.lock().unwrap().push(row);
                    });
                }
            });
            let mut rows = rows.into_inner().unwrap();
            rows.sort_by(|a, b| a.map.cmp(&b.map));
            let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
            let mut js = String::new();
            js.push_str(&format!("{{\n  \"tool\": \"lmtool relight-batch\",\n  \"quality\": {quality},\n  \"jobs\": {jobs},\n  \"writer\": \"{}\",\n  \"extra_args\": [{}],\n  \"host\": \"{}\",\n  \"wall_s\": {:.1},\n  \"maps\": [\n", esc(&writer), extra.iter().map(|e| format!("\"{}\"", esc(e))).collect::<Vec<_>>().join(", "), esc(&host), t_all.elapsed().as_secs_f32()));
            for (i, r) in rows.iter().enumerate() {
                js.push_str(&format!("    {{\"map\": \"{}\", \"out\": \"{}\", \"started_unix\": {}, \"bake_ok\": {}, \"bake_s\": {:.1}, \"sweeps_s\": [{}], \"peak_rss\": \"{}\", \"triangles\": \"{}\", \"items\": \"{}\", \"layout_texels\": \"{}\", \"peels_per_direction\": \"{}\", \"peel_cameras\": \"{}\", \"check_ok\": {}, \"check_fails\": {}, \"check_s\": {:.1}, \"note\": \"{}\"}}{}\n", esc(&r.map), esc(&r.out), r.started, r.bake_ok, r.bake_s, r.sweeps.iter().map(|s| format!("{s:.1}")).collect::<Vec<_>>().join(", "), esc(&r.rss), r.tris, r.insts, r.texels, r.peels, esc(&r.cameras), r.check_ok, r.check_fails, r.check_s, esc(&r.note), if i + 1 < rows.len() { "," } else { "" }));
            }
            js.push_str("  ]\n}\n");
            std::fs::write(&manifest, &js).expect("manifest");
            let mut md = String::new();
            md.push_str(&format!("# lmtool relight-batch — quality {quality}, {} maps, {} at a time, {boxes} box(es) per map{}, writer {writer}, extra {:?}, {:.0} s wall ({})\n\n| map | triangles | items | texels | peels/dir | sweeps (s) | bake (s) | peak RSS | check | |\n|---|---|---|---|---|---|---|---|---|---|\n", rows.len(), jobs, if hosts.is_empty() { String::new() } else { format!(" on {}", hosts.join(",")) }, extra, t_all.elapsed().as_secs_f32(), host));
            for r in &rows {
                md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {:.1} | {} | {} | {} |\n", std::path::Path::new(&r.map).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(), r.tris, r.insts, r.texels, r.peels, r.sweeps.iter().map(|s| format!("{s:.1}")).collect::<Vec<_>>().join(" / "), r.bake_s, r.rss, if r.check_ok { "ok".to_string() } else if r.bake_ok { format!("{} FAIL", r.check_fails) } else { "bake FAILED".to_string() }, r.note));
            }
            let md_path = manifest.with_extension("md");
            std::fs::write(&md_path, &md).expect("table");
            print!("{md}");
            eprintln!("relight-batch: manifest {} table {}", manifest.display(), md_path.display());
        }
        "passdiff" => {
            // lmtool passdiff GAME_DIR OURS_DIR [--pass P] [--tol T] [--floor F] [--stride S] [--threshold PCT]
            //   [--game-map MAP.Gbx] [--heat DIR] [--all-heat] [--report FILE.md]
            // the per-pass differential of the game's captured render targets against our --dump-passes tree
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let has = |k: &str| a.iter().any(|x| x == k);
            let game = std::path::PathBuf::from(&a[1]);
            let ours = std::path::PathBuf::from(&a[2]);
            let mut opts = lightmap::passdiff::Opts::default();
            if let Some(v) = f("--hbasis-scale") { opts.hbasis_scale = v.parse().expect("--hbasis-scale"); }
            opts.pass = f("--pass");
            if let Some(v) = f("--tol") { opts.tol = v.parse().expect("--tol"); }
            if let Some(v) = f("--floor") { opts.floor = v.parse().expect("--floor"); }
            if let Some(v) = f("--stride") { opts.stride = v.parse().expect("--stride"); }
            if let Some(v) = f("--threshold") { opts.pass_threshold = v.parse().expect("--threshold"); }
            opts.game_map = f("--game-map");
            opts.game_manifest = f("--game-manifest");
            opts.delta_from = f("--delta-from").map(|v| v.parse().expect("--delta-from"));
            let heat = f("--heat");
            opts.keep_pairs = heat.is_some();
            let t0 = std::time::Instant::now();
            let (rows, findings) = lightmap::passdiff::run(&game, &ours, &opts).unwrap_or_else(|e| panic!("passdiff: {e}"));
            let rep = lightmap::passdiff::report(&rows, &findings, opts.pass_threshold, opts.tol);
            println!("{rep}");
            // the per-buffer rows (every buffer of the first divergent pass, the worst 3 of the others)
            let sums = lightmap::passdiff::summarise(&rows);
            let first = sums.iter().find(|s| s.n > 0 && s.pct_within() < opts.pass_threshold).map(|s| s.pass.clone());
            println!("per buffer (the first divergent pass in full, the worst 3 of every other pass):");
            for s in &sums {
                let mut rs: Vec<&lightmap::passdiff::Row> = rows.iter().filter(|r| r.pass == s.pass).collect();
                rs.sort_by(|x, y| y.stats.rmse.partial_cmp(&x.stats.rmse).unwrap_or(std::cmp::Ordering::Equal));
                let take = if Some(&s.pass) == first.as_ref() || has("--all-rows") { rs.len() } else { 3.min(rs.len()) };
                for r in rs.iter().take(take) {
                    println!("  {:<40} n {:>9} max {:>9.4} mean {:>9.5} rmse {:>9.5} Δ {:>+9.5} within {:>6.2} %  {}{}", r.key(), r.stats.n, r.stats.max_abs, r.stats.mean_abs, r.stats.rmse, r.stats.mean_signed, r.stats.pct_within(), r.transforms.join(", "), if r.note.is_empty() { String::new() } else { format!("  [{}]", r.note) });
                }
            }
            if let Some(dir) = heat {
                std::fs::create_dir_all(&dir).expect("--heat dir");
                let mut n = 0;
                for r in &rows {
                    let dump_it = has("--all-heat") || Some(&r.pass) == first.as_ref();
                    if !dump_it { continue; }
                    if let Some((g, o, ch)) = &r.pair {
                        let name = format!("{dir}/{}.png", r.key().replace(' ', "_"));
                        lightmap::passdiff::heat_png(&name, g, o, *ch, opts.tol).expect("heat png");
                        n += 1;
                    }
                }
                println!("{n} heat maps (game | ours | relative Δ: blue ≤ tol, green 2×, yellow 4×, red ≥ 8×) under {dir}");
            }
            if let Some(p) = f("--report") {
                let mut full = format!("# passdiff {} vs {}\n\n{}\n", a[1], a[2], rep);
                full += "\n## Per buffer\n\n| buffer | texels | max abs | mean abs | RMSE | mean Δ | within | transforms | note |\n|---|---|---|---|---|---|---|---|---|\n";
                for r in &rows { full += &format!("| {} | {} | {:.4} | {:.5} | {:.5} | {:+.5} | {:.2} % | {} | {} |\n", r.key(), r.stats.n, r.stats.max_abs, r.stats.mean_abs, r.stats.rmse, r.stats.mean_signed, r.stats.pct_within(), r.transforms.join(", "), r.note); }
                std::fs::write(&p, full).expect("--report");
                eprintln!("report written to {p}");
            }
            eprintln!("passdiff: {} buffers compared ({:.1}s)", rows.len(), t0.elapsed().as_secs_f32());
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
            // the writer keeps a table's STORED compressed bytes when it has them — drop them so the
            // renumbered binds are what gets written (every `--reduced` reference before 2026-09-23 21:00Z
            // carried the reduced indices: chart r landed on full item r — a mis-association)
            for z in mp.raw_z.iter_mut() { *z = None; }
            let payload = chunk.write(true);
            let out = f("--out").expect("--out");
            lightmap::mapio::save_with_chunk(&into, &payload, &out).expect("save");
            println!("wrote {out}: {items} item charts renumbered (reduced → full), {tiles} tile charts kept, {oob} charts beyond the kept list; the full map has {} items, the reduced bake {}", tmmaps::map::MapFile::load(std::path::Path::new(&f("--into").unwrap())).items.len(), kept.len());
        }
        "materials" => {
            // lmtool materials MAP: the game-material links of the map's item models, triangles per link (over all
            // placements) and the bounce albedo the table gives each — what the per-material bounce works from
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let mut per: std::collections::BTreeMap<String, (usize, usize, [f32; 3])> = std::collections::BTreeMap::new();
            let mut unknown = 0usize;
            for inst in &scene.instances {
                let m = &scene.models[inst.model];
                for t in &m.tris {
                    if (t.mat as usize) < m.mat_links.len() {
                        let e = per.entry(m.mat_links[t.mat as usize].clone()).or_insert((0, 0, m.mat_albedo[t.mat as usize]));
                        let _ = lightmap::albedo::is_measured(&m.mat_links[t.mat as usize]);
                        e.0 += 1;
                    } else {
                        unknown += 1;
                    }
                }
                for l in &m.mat_links { if let Some(e) = per.get_mut(l) { e.1 += 1; } }
            }
            let mut rows: Vec<_> = per.into_iter().collect();
            rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
            let (mut ylo, mut yhi) = (f32::MAX, f32::MIN);
            for inst in &scene.instances { ylo = ylo.min(inst.xf[10]); yhi = yhi.max(inst.xf[10]); }
            // per model: the lightmap uv1 range (spill past 0..1 lands in neighbouring charts in the editor)
            {
                let mut seen: std::collections::BTreeSet<usize> = Default::default();
                for inst in &scene.instances {
                    if !seen.insert(inst.model) { continue; }
                    let g = &scene.models[inst.model];
                    let mut per_mat: std::collections::BTreeMap<u16, ([f32; 2], [f32; 2], usize)> = Default::default();
                    for t in &g.tris { let e = per_mat.entry(t.mat).or_insert(([f32::MAX; 2], [f32::MIN; 2], 0)); for uv in t.uv { for k in 0..2 { e.0[k] = e.0[k].min(uv[k]); e.1[k] = e.1[k].max(uv[k]); } } e.2 += 1; }
                    let outside = per_mat.values().any(|(lo, hi, _)| lo[0] < -0.01 || lo[1] < -0.01 || hi[0] > 1.01 || hi[1] > 1.01);
                    if a.iter().any(|x| x == "--normals") {
                        // vertex normal vs winding normal agreement per material
                        let mut agree: std::collections::BTreeMap<u16, (usize, usize)> = Default::default();
                        for t in &g.tris {
                            let fn_ = lightmap::geometry::norm(lightmap::geometry::cross(lightmap::geometry::sub(t.p[1], t.p[0]), lightmap::geometry::sub(t.p[2], t.p[0])));
                            let vn = lightmap::geometry::norm([t.n[0][0] + t.n[1][0] + t.n[2][0], t.n[0][1] + t.n[1][1] + t.n[2][1], t.n[0][2] + t.n[1][2] + t.n[2][2]]);
                            let e = agree.entry(t.mat).or_insert((0, 0));
                            if lightmap::geometry::dot(fn_, vn) >= 0.0 { e.0 += 1 } else { e.1 += 1 }
                        }
                        let (mut cut, mut opaque_nomat) = (0usize, 0usize);
                        for t in &g.tris { if t.alpha != u16::MAX { cut += 1 } else if t.mat == u16::MAX { opaque_nomat += 1 } }
                        println!("  model {}: {} alpha-tested tris, {} OPAQUE tris without a material link; alpha textures {:?}", inst.model, cut, opaque_nomat, g.alpha_tex);
                        // uv1 orientation: the signed area of each triangle in lightmap-uv space (the game's chart
                        // raster may cull one winding)
                        let mut uvo: std::collections::BTreeMap<u16, (usize, usize)> = Default::default();
                        for t in &g.tris {
                            let a = (t.uv[1][0] - t.uv[0][0]) * (t.uv[2][1] - t.uv[0][1]) - (t.uv[2][0] - t.uv[0][0]) * (t.uv[1][1] - t.uv[0][1]);
                            let e = uvo.entry(t.mat).or_insert((0, 0));
                            if a >= 0.0 { e.0 += 1 } else { e.1 += 1 }
                        }
                        println!("  model {}: uv1 winding per material (ccw / cw): {}", inst.model, uvo.iter().map(|(mat, (p, n))| format!("mat {mat}: {p} / {n}")).collect::<Vec<_>>().join("; "));
                        println!("  model {} ({}): vertex-normal vs winding: {}", inst.model, scene.model_names.get(inst.model).cloned().unwrap_or_default(), agree.iter().map(|(mat, (a, d))| format!("mat {} ({}): {a} agree / {d} disagree", mat, g.mat_links.get(*mat as usize).map(|s| s.rsplit('\\').next().unwrap_or(s).to_string()).unwrap_or_else(|| "-".into()))).collect::<Vec<_>>().join("; "));
                    }
                    if outside || a.iter().any(|x| x == "--uv-all") {
                        println!("  model {} ({}): uv1 {:?}..{:?} plg_bounds {:?}", inst.model, scene.model_names.get(inst.model).cloned().unwrap_or_default(), g.uv_min, g.uv_max, g.plg_bounds);
                        for (mat, (lo, hi, n)) in &per_mat { println!("      mat {mat} ({}): {n} tris uv ({:.3},{:.3})..({:.3},{:.3})", g.mat_links.get(*mat as usize).map(|s| s.rsplit('\\').next().unwrap_or(s).to_string()).unwrap_or_else(|| "-".into()), lo[0], lo[1], hi[0], hi[1]); }
                    }
                }
            }
            println!("{} material links over {} instances (item y {ylo:.1}..{yhi:.1}); {unknown} triangles without a material", rows.len(), scene.instances.len());
            for (link, (tris, insts, alb)) in rows {
                let tag = if lightmap::albedo::is_measured(&link) { "measured" } else if alb[0].is_finite() { "keyword " } else { "DEFAULT " };
                println!("{tris:>9} tris {insts:>6} inst  {tag} albedo ({:.2},{:.2},{:.2}) lum {:.2}  {link}", alb[0], alb[1], alb[2], lightmap::albedo::lum(alb));
            }
        }
        "chunkhex" => {
            // lmtool chunkhex MAP ID…: the payload bytes of small skippable body chunks (hex + u32/f32 readings)
            let m = lightmap::mapio::load(&a[1]).expect("load");
            let cs = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
            for id_s in &a[2..] {
                let id = u32::from_str_radix(id_s.trim_start_matches("0x"), 16).unwrap();
                for c in cs.iter().filter(|c| c.0 == id) {
                    let b = &m.gbx.body[c.2..c.2 + c.3.min(128)];
                    let words: Vec<String> = b.chunks(4).filter(|w| w.len() == 4).map(|w| { let u = u32::from_le_bytes([w[0], w[1], w[2], w[3]]); let f = f32::from_bits(u); if f.is_finite() && f.abs() > 1e-5 && f.abs() < 1e6 && u > 0x1000 { format!("{u}/{f:.4}") } else { format!("{u}") } }).collect();
                    println!("{id_s} ({} B): {}  = [{}]", c.3, b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "), words.join(", "));
                }
            }
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
            // --row V: the texel values along the row at texture v = V (u = 0..1 in 1/20 steps), bilinear
            if let Some(v) = a.iter().position(|x| x == "--row").and_then(|i| a.get(i + 1)) {
                let v: f32 = v.parse().unwrap();
                for i in 0..=20 {
                    let u = i as f32 / 20.0;
                    let t = g.sample_linear(u, v, 2);
                    println!("  u {u:.2} v {v:.3}: ({:.4}, {:.4}, {:.4})", t[0], t[1], t[2]);
                }
            }
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
        "probe-chain-check" => {
            // lmtool probe-chain-check DIR PASSCAP_ROOT MAP [--frame 7537]: the bake's dumped probe accumulators (--chain-final-dir) vs the
            //   capture's end volumes, and the probe WEBPs rebuilt from them vs the saved map's (probebake::chain_check)
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(7537);
            if let Err(e) = lightmap::probebake::chain_check(std::path::Path::new(&a[1]), std::path::Path::new(&a[2]), &a[3], frame) { eprintln!("probe-chain-check: {e}"); std::process::exit(1); }
        }
        "final-check" => {
            // lmtool final-check PASSCAP_ROOT MAP [--frame 7537] [--q 30,40,50,75,80,91]: the pwc6 END buffers through the CPU steps
            //   to the same run's save (filecheck.rs) — first the greys (frame 0 image 1) from Y4's channels 1..3 via libwebp
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let frame: u32 = f("--frame").map(|v| v.parse().expect("--frame")).unwrap_or(7537);
            let qs: Vec<f32> = f("--q").map(|s| s.split(',').map(|t| t.parse().expect("--q")).collect()).unwrap_or_else(|| vec![30.0, 40.0, 50.0, 75.0, 80.0, 91.0]);
            let has = |k: &str| a.iter().any(|x| x == k);
            let r = if has("--frame0") { lightmap::filecheck::check_frame0(&std::path::PathBuf::from(&a[1]), &a[2], frame) } else if has("--records") { lightmap::filecheck::check_records(&std::path::PathBuf::from(&a[1]), &a[2], frame) } else if has("--probes") { lightmap::filecheck::check_probes(&std::path::PathBuf::from(&a[1]), &a[2], frame) } else if has("--rects") { lightmap::filecheck::check_rects(&std::path::PathBuf::from(&a[1]), &a[2], frame) } else if has("--colour") { lightmap::filecheck::check_colour(&std::path::PathBuf::from(&a[1]), &a[2], frame) }
                else if has("--greys2") { lightmap::filecheck::check_greys2(&std::path::PathBuf::from(&a[1]), &a[2], frame, qs[0]) }
                else { lightmap::filecheck::check_greys(&std::path::PathBuf::from(&a[1]), &a[2], frame, &qs) };
            if let Err(e) = r { eprintln!("final-check: {e}"); std::process::exit(1); }
        }
        "webp-dump" => {
            // lmtool webp-dump FILE OUT.rgb: decode one WEBP (or the first RIFF of a concatenation; --all splits every RIFF into
            //   OUT.<k>.rgb) to raw RGB bytes and print the dimensions
            let b = std::fs::read(&a[1]).expect("read");
            let mut parts: Vec<&[u8]> = Vec::new();
            let mut off = 0usize;
            while off + 12 <= b.len() && &b[off..off + 4] == b"RIFF" {
                let sz = u32::from_le_bytes([b[off + 4], b[off + 5], b[off + 6], b[off + 7]]) as usize + 8;
                parts.push(&b[off..(off + sz).min(b.len())]);
                off += sz;
            }
            if parts.is_empty() { parts.push(&b[..]); }
            for (k, p) in parts.iter().enumerate() {
                let im = lightmap::img::decode_webp(p).expect("decode");
                let out = if parts.len() == 1 { a[2].clone() } else { format!("{}.{k}.rgb", a[2]) };
                std::fs::write(&out, &im.px).expect("write");
                println!("{out}: part {k} of {}: {} bytes of WEBP → {}×{} RGB", parts.len(), p.len(), im.w, im.h);
            }
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
        "genealogy" => {
            // lmtool genealogy MAP [--cells]: the zone genealogy chunk 0x03043043 per cell — the histogram of (CurrentZoneId, Dir)
            // and, with --cells, one line per record: record index → cell (x = i % 64, z = i / 64), zone, dir, chain
            let gb = tmmaps::gbx::Gbx::load(std::path::Path::new(&a[1])).expect("map");
            let Some(&(_, _, payload, size)) = tmmaps::map::skip_chunks(&gb.body).iter().find(|(cid, ..)| *cid == 0x0304_3043) else { println!("no genealogy chunk"); return };
            let recs = tmmaps::map::genealogy_full(&gb.body[payload..payload + size]).expect("genealogy");
            // (record order = x·64 + z per tmmaps genealogy-cells)
            let mut hist: std::collections::BTreeMap<(String, u32), usize> = Default::default();
            for r in &recs { *hist.entry((r.current.clone(), r.dir)).or_default() += 1; }
            println!("{} records; (zone, dir) histogram: {:?}", recs.len(), hist);
            if a.iter().any(|x| x == "--cells") { for (i, r) in recs.iter().enumerate() { println!("{i:4} cell x{:>2} z{:>2} {} d{} [{}]", i / 64, i % 64, r.current, r.dir, r.ids.join(">")); } }
        }
        "packtest" => {
            // the zone tile's chart extent from its PreLightGen (uv bounds b, MeterByUv) and the block scale k: the game computes
            // f = MeterByUv × blockScale, ext = (uvExt × f) (spec §3.1 per chart; korder 1) — korder 0 = (uvExt × MeterByUv) × k
            let korder: u32 = a.iter().position(|x| x == "--k-order").and_then(|i| a.get(i + 1)).map(|s| s.parse().unwrap()).unwrap_or(1);
            let tile_ext_of = |b: &[f32; 4], mbu: f32, k: f32, korder: u32| -> [f32; 2] { if korder == 0 { [((b[2] - b[0]) * mbu) * k, ((b[3] - b[1]) * mbu) * k] } else { let f = mbu * k; [(b[2] - b[0]) * f, (b[3] - b[1]) * f] } };
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
            // the scale search's iteration count from the bake's quality (cache chunk 0x0602200F = (q, 0); the game's table by quality index
            // {0: 1, 1: 3, 2: 6, 3: 8, 4: 10, 5: 10} — BlockSplit l.299–316); --iter overrides
            let max_iter_default: u32 = { let own0 = lightmap::mapio::load(&a[1]).expect("map"); let d0 = own0.chunk.data.as_ref().unwrap(); let mut q = 2u32; for c in &d0.cache.chunks { if c.id == 0x0602_200F { if let lightmap::format::ChunkBody::Raw(b) = &c.body { if b.len() >= 4 { q = u32::from_le_bytes([b[0], b[1], b[2], b[3]]); } } } } let it = match q { 0 => 1, 1 => 3, 2 => 6, 3 => 8, _ => 10 }; println!("bake quality index {q} → scale search maxIter {it}"); it };
            let max_iter: u32 = f("--iter").map(|s| s.parse().unwrap()).unwrap_or(max_iter_default);
            let tile_ext: f32 = f("--tile-ext").map(|s| s.parse().unwrap()).unwrap_or(0.0);
            // THE ZONE TILE'S CHART EXTENT (the layout rule, closed on pwc-day 2026-09-25 — 4096 of 4096 tile rects + the 3 items
            // reproduce the editor's chart table in position and size): f = MeterByUv × blockScale, ext = (uvExt × f) from the
            // tile solid's PreLightGen (spec §3.1 per chart); blockScale = √2/32 (f32 0x3d3504f3) for the BlueBay ground tile
            // (the BlockInfo float × the quality byte 255/255); the defaults below are the BlueBay `Zone\Sea\Base.Prefab.Gbx`
            // entity-0 values (`mapgeom zone-tile-plg`); --tile-plg MBU,U0,V0,U1,V1 [--tile-k K] for another collection's tile,
            // --tile-ext-xy X,Y an explicit extent, --tile-ext E a square one
            let tile_plg: (f32, [f32; 4]) = f("--tile-plg").map(|s| { let v: Vec<f32> = s.split(',').map(|t| t.parse().unwrap()).collect(); (v[0], [v[1], v[2], v[3], v[4]]) }).unwrap_or((f32::from_bits(0x420dc57e), [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)]));
            let tile_k: f32 = f("--tile-k").map(|s| s.parse().unwrap()).unwrap_or(f32::from_bits(0x3d3504f3));
            let tile_ext_xy: [f32; 2] = match (f("--tile-ext-xy"), tile_ext > 0.0) {
                (Some(s), _) => { let v: Vec<f32> = s.split(',').map(|t| t.parse().unwrap()).collect(); [v[0], v[1]] }
                (None, true) => [tile_ext, tile_ext],
                (None, false) => tile_ext_of(&tile_plg.1, tile_plg.0, tile_k, korder),
            };
            println!("tile chart extent ({}, {}) m [{:#010x} {:#010x}] (area {}) from MeterByUv {} × k {} × uv extents ({}, {})", tile_ext_xy[0], tile_ext_xy[1], tile_ext_xy[0].to_bits(), tile_ext_xy[1].to_bits(), tile_ext_xy[0] * tile_ext_xy[1], tile_plg.0, tile_k, tile_plg.1[2] - tile_plg.1[0], tile_plg.1[3] - tile_plg.1[1]);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("own");
            let d = own.chunk.data.as_ref().unwrap();
            let mp = d.cache.mapping().unwrap();
            // the editor's table: object id → (x, y, w, h) in 2048 units
            let mut ed: std::collections::HashMap<u32, (u16, u16, u16, u16)> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; ed.insert(obj, (mp.pos[i].0, mp.pos[i].1, mp.size[i].0, mp.size[i].1)); }
            let n_tiles = ed.keys().filter(|&&o| o < base).count();
            if a.iter().any(|x| x == "--bind-order") {
                // the mapping's bind order (the order the chart records were appended = the block-array order?) — the first and
                // last tile objects, and the rank of the tiles the editor placed first (obj 128, 3377, 2169, …)
                let objs: Vec<u32> = (0..mp.count as usize).map(|i| mp.binds[i].obj_group_idx / 4).collect();
                println!("bind order: {} entries; first 24: {:?}", objs.len(), &objs[..objs.len().min(24)]);
                println!("  last 24: {:?}", &objs[objs.len().saturating_sub(24)..]);
                for want in [128u32, 3377, 2169, 2164, 2153, 2148, 2133, 2117, 1828, 116, 104, 2174, 2159] { if let Some(r) = objs.iter().position(|&o| o == want) { println!("  obj {want} at bind rank {r} (cell x {} z {})", want % 64, want / 64); } }
                return;
            }
            // --against EDITOR.Map.Gbx: compare THIS map's mapping table with another bake's (the bake's --layout-game output
            // against the editor's), object by object
            if let Some(other) = f("--against") {
                let o2 = lightmap::mapio::load(&other).expect("--against");
                let d2 = o2.chunk.data.as_ref().unwrap();
                let mp2 = d2.cache.mapping().unwrap();
                let mut ed2: std::collections::HashMap<u32, (u16, u16, u16, u16)> = Default::default();
                for i in 0..mp2.count as usize { ed2.insert(mp2.binds[i].obj_group_idx / 4, (mp2.pos[i].0, mp2.pos[i].1, mp2.size[i].0, mp2.size[i].1)); }
                let (mut n, mut eq, mut size_eq, mut only_here, mut only_there) = (0usize, 0usize, 0usize, 0usize, 0usize);
                let mut shown = 0;
                for (o, r) in ed.iter() { match ed2.get(o) { Some(r2) => { n += 1; if r == r2 { eq += 1; } if r.2 == r2.2 && r.3 == r2.3 { size_eq += 1; } else if shown < 8 { shown += 1; println!("  obj {o}: ours {}×{} at ({}, {}) vs {}×{} at ({}, {})", r.2, r.3, r.0, r.1, r2.2, r2.3, r2.0, r2.1); } } None => only_here += 1 } }
                for o in ed2.keys() { if !ed.contains_key(o) { only_there += 1; } }
                println!("against {other}: {n} shared objects, {eq} identical rects, {size_eq} equal sizes; {only_here} only here, {only_there} only there");
                // the mapping ORDER (frame0_blobs' fb0 indexing follows it): is each table sorted by object id, and do the two agree?
                let order = |m: &lightmap::format::Mapping| -> Vec<u32> { (0..m.count as usize).map(|i| m.binds[i].obj_group_idx / 4).collect() };
                let (o1, o2) = (order(mp), order(mp2));
                let sorted = |v: &Vec<u32>| v.windows(2).all(|w| w[0] <= w[1]);
                let same = o1 == o2;
                println!("  mapping order: ours sorted by object {} ({} entries), theirs sorted {} ({} entries), identical sequences {same}; first 6 ours {:?} theirs {:?}", sorted(&o1), o1.len(), sorted(&o2), o2.len(), &o1[..6.min(o1.len())], &o2[..6.min(o2.len())]);
                return;
            }
            if a.iter().any(|x| x == "--tile-order") {
                // the editor's tile charts by atlas row then column: the object-id pattern reveals the placement order
                let mut tiles: Vec<(u32, (u16, u16, u16, u16))> = ed.iter().filter(|(&o, _)| o < base).map(|(&o, &r)| (o, r)).collect();
                tiles.sort_by_key(|(_, r)| (r.1, r.0));
                let take: usize = f("--take").map(|s| s.parse().unwrap()).unwrap_or(40);
                if let Some(col) = f("--col") { let cx: u16 = col.parse().unwrap(); tiles.retain(|(_, r)| r.0 == cx); tiles.sort_by_key(|(_, r)| r.1); }
                if let Some(row) = f("--row") { let cy: u16 = row.parse().unwrap(); tiles.retain(|(_, r)| r.1 == cy); tiles.sort_by_key(|(_, r)| r.0); }
                for (o, r) in tiles.iter().take(take) { println!("  tile obj {o:>5} (cell x {:>2} z {:>2}) at ({:>4}, {:>4}) {}×{}", o % 64, o / 64, r.0, r.1, r.2, r.3); }
                return;
            }
            {
                let mut hist: std::collections::BTreeMap<(u16, u16), usize> = Default::default();
                for (&o, &(_, _, w, h)) in &ed { if o < base { *hist.entry((w, h)).or_default() += 1; } }
                println!("editor tile chart sizes: {:?}", hist);
                let (mut mx, mut my) = (0u32, 0u32); let mut ymax_row: std::collections::BTreeMap<u32, usize> = Default::default(); for (&_o, &(x, y, w, h)) in &ed { mx = mx.max(x as u32 + w as u32); my = my.max(y as u32 + h as u32); *ymax_row.entry((y as u32 + h as u32) / 128).or_default() += 1; }
                println!("editor layout extent: x up to {mx}, y up to {my} (of 2048); charts per 128-row band of their bottom edge: {:?}", ymax_row);
                println!("editor atlas {}×{}, m_u01 {}, m_u02 {}, m_u03 {}", mp.atlas_w, mp.atlas_h, mp.m_u01, mp.m_u02, mp.m_u03);
                // the editor's Σ chart area (TotalLmSurfaceMeter, cache chunk 0x0602200B = (1, Σarea))
                for c in &d.cache.chunks { if c.id == 0x0602_200B { if let lightmap::format::ChunkBody::Raw(b) = &c.body { if b.len() >= 8 { let area = f32::from_le_bytes([b[4], b[5], b[6], b[7]]); println!("editor TotalLmSurfaceMeter (0x0602200B): {area} m²  → D = W·H/Σarea = {:.4} (layout units/m)²", (w_atlas as f64 * w_atlas as f64) / area as f64); } } } }
            }
            // THE TILE OBJECT IDS (measured on pwc-day against the capture's instance stream, `lmscene --tiles-map`): tile
            // object k < |baked records| is the k-th BAKED block record of the map file (the tiny build's fill records, in
            // file order); the cells no record covers get the following ids in x-major order (x ascending, z ascending
            // within x) — the game's own generation order. `cell_of[obj]` = (cx, cz).
            let cell_of: Vec<(i32, i32)> = {
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
                let (sx, sz) = (64i32, 64i32);
                let mut covered = vec![false; (sx * sz) as usize];
                let mut cells: Vec<(i32, i32)> = Vec::new();
                for b in &mf.baked {
                    let (cx, _cy, cz) = b.coords();
                    if cx >= 0 && cx < sx && cz >= 0 && cz < sz { covered[(cx * sz + cz) as usize] = true; }
                    cells.push((cx, cz));
                }
                for cx in 0..sx { for cz in 0..sz { if !covered[(cx * sz + cz) as usize] { cells.push((cx, cz)); } } }
                println!("tile cells: {} baked records + {} generated = {}", mf.baked.len(), cells.len() - mf.baked.len(), cells.len());
                cells
            };
            // our chart list in IdForLightMap order: tiles (ids 0..base) then items (base + item)
            let mut charts: Vec<lightmap::pack::ChartExt> = Vec::new();
            let mut ids: Vec<u32> = Vec::new();
            // THE GENERATED TILE'S QUALITY (CGameCtnApp::HmsLightMapUpdateBlocksAndItemsQuality 0x140dcc290 → FUN_140dcc8e0, the
            // ring search): the items mark their cells (file cell x, y, z) in a 3-D grid; a ground tile whose own cell is marked
            // keeps the enum quality (Normal → 1.0); otherwise rings r = 1…8 around the tile AT ITS OWN LEVEL are searched for
            // a marked cell — found at ring r → f = powf(0.5, r/2) = (√2)^−r; none within 8 → (√2)^−9 (the 0.044194 of the
            // far tiles); the tile's chart scale = f × G (1.0 for the map's own objects) × the BlockInfo float (1.0 for Sea).
            // (The blocks branch of the search — a block in the column whose height above the tile is ≤ r — is not needed for
            // the item-only tiny maps and is not transcribed here.) Oracle: refs/hill*-q3-editor's size ladder, one size class per ring.
            let tile_quality: Vec<f32> = {
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
                let tile_y: i32 = lightmap::layout::tile_level(&mf, &f("--collection").unwrap_or_else(|| "BlueBay".into()));
                // --tile-mark K: an item marks its cell at the tile level when |item level − tile level| ≤ K (default 0 = its own level only; 99 = any level)
                let mark_k: i32 = f("--tile-mark").map(|v| v.parse().unwrap()).unwrap_or(0);
                let marked: std::collections::HashSet<(i32, i32, i32)> = mf.items.iter().flat_map(|it| { let (x, y, z) = (it.file_cell[0] as i32, it.file_cell[1] as i32, it.file_cell[2] as i32); (-mark_k..=mark_k).map(move |d| (x, y + d, z)) }).collect();
                // --quant-byte [F]: RE 6's reading — the record carries byte = clamp(int(255·q), 1, 255) and the chart scale is byte/255 (× F)
                let quant: Option<f32> = a.iter().position(|x| x == "--quant-byte").map(|i| a.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1.0));
                let f_of = |r: u32| -> f32 { let q = (0.5f32).powf((r as f32 + 1.0) * 0.5); match quant { Some(ff) => { let b = ((q * 255.0) as i32).clamp(1, 255); b as f32 / 255.0 * ff } None => q } };
                let mut hist: std::collections::BTreeMap<u32, usize> = Default::default();
                let q: Vec<f32> = cell_of.iter().map(|&(cx, cz)| {
                    if marked.contains(&(cx, tile_y, cz)) { *hist.entry(0).or_default() += 1; return match quant { Some(ff) => ff, None => 1.0 }; }
                    let mut found = 9u32;
                    'r: for r in 1..=8i32 {
                        for dx in -r..=r { for dz in -r..=r {
                            if dx.abs() != r && dz.abs() != r { continue; }
                            if marked.contains(&(cx + dx, tile_y, cz + dz)) { found = r as u32; break 'r; }
                        } }
                    }
                    *hist.entry(found).or_default() += 1;
                    if found == 9 { f_of(8) } else { f_of(found - 1) }
                }).collect();
                if !a.iter().any(|x| x == "--uniform-tiles") { println!("tile quality by ring (0 = the item's own cell, 9 = none within 8): {:?}", hist); }
                q
            };
            for o in 0..base { if ed.contains_key(&o) { // the tile's chart scale = its quality q ITSELF (1.0 on an item's cell = the same ext as a tile-quad item → an exact area tie the z key resolves; the far tiles' 0.5^4.5 = 0x3d3504f3 = tile_k)
                let q = if a.iter().any(|x| x == "--uniform-tiles") { tile_k } else { tile_quality.get(o as usize).copied().unwrap_or(tile_k) }; let e = if f("--tile-ext-xy").is_some() || tile_ext > 0.0 { tile_ext_xy } else { tile_ext_of(&tile_plg.1, tile_plg.0, q, korder) }; charts.push(lightmap::pack::ChartExt { ext: e, mins: [1, 1] }); ids.push(o); } }
            // --kept FILE (RE 7): the item indices the REDUCED map kept (the editor baked the reduced map; `transplant --kept`
            // renumbered) — only those items exist in the editor's table; --tile-mark MODE: the item-cell marking variant for
            // the ring rule (same = the item's own level only (default), any = every level, near = |Δlevel| ≤ 1)
            let kept: Option<std::collections::HashSet<usize>> = f("--kept").map(|p| std::fs::read_to_string(&p).expect("--kept").split(|c: char| c == ',' || c.is_whitespace()).filter_map(|t| t.trim().parse().ok()).collect());
            // WHICH ITEMS GET A CHART (tiny 16's editor table as the oracle, `--item-audit`): an item whose Solid2 carries
            // LIGHTS is not charted (49 of the 50 uncharted models there have lights, all 446 charted have none); the one
            // light-less uncharted model is the finish trigger FX (material RaceTriggerFXFinish) — the material-class flag
            // 0x4 of spec §3.1 (DAT_141e7c768), read here by the material name until RE 7 pins the class. --chart-all-items
            // keeps them. Their ids stay (IdForLightMap runs over every item).
            let chart_all = a.iter().any(|x| x == "--chart-all-items");
            let mut skipped_items = 0usize;
            for inst in &scene.instances {
                let m = &scene.models[inst.model];
                let fx_only = !m.mat_links.is_empty() && m.mat_links.iter().all(|l| l.contains("RaceTriggerFX"));
                if let Some(k) = &kept { if !k.contains(&inst.item) { skipped_items += 1; continue; } }
                let one_uv_set = m.plg_bounds.map_or(true, |b| !(b[2] > b[0] && b[3] > b[1]));
                if kept.is_some() { if one_uv_set || fx_only { skipped_items += 1; continue; } } else if !chart_all && (!m.lights.is_empty() || fx_only) { skipped_items += 1; continue; }
                let sc = ((inst.xf[0] * inst.xf[0] + inst.xf[1] * inst.xf[1] + inst.xf[2] * inst.xf[2]) as f32).sqrt();
                // the item's chart scale = its quality q (MapElemLightmapQuality e → (√2)^e · G, FUN_140dcc1c0) × the placement scale
                let e: i32 = match inst.lm_quality { 0 => 0, 1 => 1, 2 => 2, 3 => 3, 4 => -1, 5 => -2, 6 => -3, _ => 0 };
                let q = (0.5f32).powf(-(e as f32) * 0.5);
                let ext = match m.plg_bounds { Some(b) => { let fq = m.plg_u02 * (q * sc); [(b[2] - b[0]) * fq, (b[3] - b[1]) * fq] } None => [0.0, 0.0] };
                charts.push(lightmap::pack::ChartExt { ext, mins: [1, 1] });
                ids.push(base + inst.item as u32);
            }
            if skipped_items > 0 { println!("items without a chart (lights / FX material): {skipped_items} of {}", scene.instances.len()); }
            let sum_area: f32 = charts.iter().map(|c| c.ext[0] * c.ext[1]).sum();
            println!("{} charts ({n_tiles} tiles in the editor's table, {} items), Σarea {sum_area:.1} m², W {w_atlas} g {g} maxIter {max_iter}", charts.len(), scene.instances.len());
            { let items: f64 = charts.iter().zip(&ids).filter(|(_, &o)| o >= base).map(|(c, _)| c.ext[0] as f64 * c.ext[1] as f64).sum(); for c in &d.cache.chunks { if c.id == 0x0602_200B { if let lightmap::format::ChunkBody::Raw(b) = &c.body { if b.len() >= 8 { let total = f32::from_le_bytes([b[4], b[5], b[6], b[7]]) as f64; println!("items Σarea {items:.6} m² (f32 {}); editor total {total}; residual per tile ({total} − items)/{n_tiles} = {:.9} m²", items as f32, (total - items) / n_tiles as f64); } } } } }
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
            if a.iter().any(|x| x == "--editor-holes") {
                // the atlas cells no chart of the editor's mapping covers (packer units, incl. the 1-unit gutter): the charts the mapping
                // does not list (the light-carrying items?) would show as rectangular holes
                let mut cov = vec![false; 2048 * 2048];
                for (_, &(x, y, w, h)) in ed.iter() { for yy in (y as usize).saturating_sub(1)..(y as usize + h as usize + 1).min(2048) { for xx in (x as usize).saturating_sub(1)..(x as usize + w as usize + 1).min(2048) { cov[yy * 2048 + xx] = true; } } }
                let free = cov.iter().filter(|c| !**c).count();
                println!("editor layout: {} of {} atlas cells uncovered ({:.2} %)", free, 2048 * 2048, 100.0 * free as f64 / (2048.0 * 2048.0));
                // greedy maximal free rectangles (row scan): report the 20 largest
                let mut rects: Vec<(usize, usize, usize, usize)> = Vec::new();
                let mut used = cov.clone();
                for y in 0..2048usize { for x in 0..2048usize {
                    if used[y * 2048 + x] { continue; }
                    let mut w = 0; while x + w < 2048 && !used[y * 2048 + x + w] { w += 1; }
                    let mut h = 1; 'h: while y + h < 2048 { for xx in x..x + w { if used[(y + h) * 2048 + xx] { break 'h; } } h += 1; }
                    for yy in y..y + h { for xx in x..x + w { used[yy * 2048 + xx] = true; } }
                    rects.push((x, y, w, h));
                } }
                rects.sort_by_key(|r| std::cmp::Reverse(r.2 * r.3));
                println!("{} free rectangles; the 24 largest (x, y, w, h):", rects.len());
                for r in rects.iter().take(24) { println!("  ({}, {}) {}×{}", r.0, r.1, r.2, r.3); }
                let big: usize = rects.iter().filter(|r| r.2 >= 6 && r.3 >= 6).map(|r| r.2 * r.3).sum();
                println!("free area in rectangles ≥ 6×6: {big}");
                return;
            }
            if a.iter().any(|x| x == "--editor-big") {
                // the editor's largest charts and what sits at the atlas origin
                let mut all: Vec<(u32, (u16, u16, u16, u16))> = ed.iter().map(|(&o, &r)| (o, r)).collect();
                all.sort_by_key(|(_, r)| std::cmp::Reverse(r.2 as u32 * r.3 as u32));
                for (o, r) in all.iter().take(12) { let who = if *o >= base { scene.instances.iter().find(|i| base + i.item as u32 == *o).map(|i| format!("item {} {}", i.item, i.model_name)).unwrap_or_else(|| format!("item {} (not in our scene)", o - base)) } else { format!("tile {o}") }; println!("  editor {}×{} at ({}, {}): {who}", r.2, r.3, r.0, r.1); }
                for (o, r) in ed.iter() { if r.0 <= 1 && r.1 <= 1 { println!("  at the origin: obj {o} {}×{}", r.2, r.3); } }
                for it in [1555u32, 2398, 2399, 2400] { match ed.get(&(base + it)) { Some(r) => println!("  stock-screen item {it}: editor chart {}×{} at ({}, {})", r.2, r.3, r.0, r.1), None => println!("  stock-screen item {it}: NOT in the editor's mapping") } }
                return;
            }
            // --kept-diag: which kept items are not records (no / degenerate uv-set-0 bounds)
            if a.iter().any(|x| x == "--kept-diag") {
                let Some(k) = &kept else { panic!("--kept") };
                let (mut n, mut no_plg, mut degen) = (0, 0, 0);
                for inst in &scene.instances {
                    if !k.contains(&inst.item) { continue; }
                    n += 1;
                    match scene.models[inst.model].plg_bounds { None => { no_plg += 1; println!("  item {} {}: no PLG bounds; pos {:?} plg_u02 {} uv range {:?}..{:?}", inst.item, inst.model_name, inst.pose.pos, scene.models[inst.model].plg_u02, scene.models[inst.model].uv_min, scene.models[inst.model].uv_max); } Some(b) => if !(b[2] > b[0] && b[3] > b[1]) { degen += 1; println!("  item {} {}: degenerate bounds {:?}", inst.item, inst.model_name, b); } }
                }
                println!("kept-diag: {n} kept items in the scene ({} in the list), {no_plg} without PLG bounds, {degen} degenerate", k.len());
                return;
            }
            // --uv-stats-sum: Σ over the charted items of (the MESH's TexCoord1 range × MeterByUv)² against the PLG-bounds form — the
            // TotalLmSurfaceMeter study (tiny 16: the records' PLG form sums to 14.2 M, the editor's total is 15.78 M)
            if a.iter().any(|x| x == "--uv-stats-sum") {
                let (mut s_plg, mut s_mesh, mut n, mut n_diff) = (0f64, 0f64, 0usize, 0usize);
                let mut ex: Vec<(f64, f64, String)> = Vec::new();
                for inst in &scene.instances {
                    if let Some(k) = &kept { if !k.contains(&inst.item) { continue; } }
                    let m = &scene.models[inst.model];
                    let Some(b) = m.plg_bounds else { continue };
                    let a_plg = ((b[2] - b[0]) * m.plg_u02) as f64 * ((b[3] - b[1]) * m.plg_u02) as f64;
                    let a_mesh = ((m.uv_max[0] - m.uv_min[0]) * m.plg_u02) as f64 * ((m.uv_max[1] - m.uv_min[1]) * m.plg_u02) as f64;
                    s_plg += a_plg; s_mesh += a_mesh; n += 1;
                    if (a_plg - a_mesh).abs() > 1e-3 * a_plg.max(1.0) { n_diff += 1; if ex.len() < 8 { ex.push((a_plg, a_mesh, format!("{} plg [{:.4} {:.4} {:.4} {:.4}] mesh uv [{:.4} {:.4}]..[{:.4} {:.4}]", inst.model_name, b[0], b[1], b[2], b[3], m.uv_min[0], m.uv_min[1], m.uv_max[0], m.uv_max[1]))); } }
                }
                println!("uv-stats: {n} charted items; Σ area from the PLG bounds {s_plg:.1}, from the mesh TexCoord1 range {s_mesh:.1}; {n_diff} items differ");
                for (p, mm, d) in &ex { println!("  plg {p:.1} vs mesh {mm:.1}: {d}"); }
                return;
            }
            // --ext-ratio: per charted item the editor's chart width against our ext.x — (w + 2)/ext.x should be one constant (the
            // editor's s) if our extents are the game's; a spread = per-item extent differences (the item-side rule)
            if a.iter().any(|x| x == "--ext-ratio") {
                let mut rows: Vec<(f32, String, f32, f32, u16, u16)> = Vec::new();
                for (k, c) in charts.iter().enumerate() {
                    if ids[k] < base || c.ext[0] <= 0.0 { continue; }
                    let Some(&(_, _, ew, eh)) = ed.get(&ids[k]) else { continue };
                    let inst = scene.instances.iter().find(|i| base + i.item as u32 == ids[k]).unwrap();
                    let sc = ((inst.xf[0] * inst.xf[0] + inst.xf[1] * inst.xf[1] + inst.xf[2] * inst.xf[2]) as f32).sqrt();
                    rows.push(((ew as f32 + 2.0) / c.ext[0], inst.model_name.clone(), c.ext[0], sc, ew, eh));
                }
                rows.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
                let n = rows.len();
                println!("(w + 2)/ext.x over {n} charted items: min {:.4} p10 {:.4} median {:.4} p90 {:.4} max {:.4}", rows[0].0, rows[n / 10].0, rows[n / 2].0, rows[9 * n / 10].0, rows[n - 1].0);
                let mut by_model: std::collections::BTreeMap<String, Vec<f32>> = Default::default();
                for r in &rows { by_model.entry(r.1.clone()).or_default().push(r.0); }
                let mut v: Vec<(f32, String, usize, f32)> = by_model.iter().map(|(m, rs)| { let mean = rs.iter().sum::<f32>() / rs.len() as f32; let sc = rows.iter().find(|r| &r.1 == m).map(|r| r.3).unwrap_or(1.0); (mean, m.clone(), rs.len(), sc) }).collect();
                v.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
                { let s_e = rows[n / 2].0; let mut big: Vec<&(f32, String, f32, f32, u16, u16)> = rows.iter().filter(|r| r.4 >= 60).collect(); big.sort_by(|p, q| (p.0 / s_e).partial_cmp(&(q.0 / s_e)).unwrap()); println!("large charts (editor w ≥ 60): {} items; (w+2)/(ext.x·s_e) with s_e = median {s_e:.4}: min {:.4} median {:.4} max {:.4}", big.len(), big.first().map(|r| r.0 / s_e).unwrap_or(0.0), big.get(big.len() / 2).map(|r| r.0 / s_e).unwrap_or(0.0), big.last().map(|r| r.0 / s_e).unwrap_or(0.0)); for r in big.iter().take(10) { println!("    {:.4} {:<22} ext.x {:.2} editor {}×{}", r.0 / s_e, r.1, r.2, r.4, r.5); } for r in big.iter().rev().take(5) { println!("    {:.4} {:<22} ext.x {:.2} editor {}×{}", r.0 / s_e, r.1, r.2, r.4, r.5); } }
                println!("per model (mean ratio, instances, placement scale) — the lowest 8 and the highest 8:");
                for r in v.iter().take(8).chain(v.iter().rev().take(8)) { let ex = rows.iter().find(|x| x.1 == r.1).unwrap(); println!("  {:.4} {:<22} ×{} scale {} (e.g. ext.x {:.3} editor {}×{})", r.0, r.1, r.2, r.3, ex.2, ex.4, ex.5); }
                return;
            }
            // --item-audit: which items the editor charted and which not, against what our model reader knows about them (the
            // PreLightGen bounds/MeterByUv, the sub-visual count, the uv1 presence, the material links) — the item-chart rule's oracle
            if a.iter().any(|x| x == "--item-audit") {
                let mut rows: Vec<(bool, String)> = Vec::new();
                let mut by_model: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
                for inst in &scene.instances {
                    let m = &scene.models[inst.model];
                    let o = base + inst.item as u32;
                    let charted = ed.contains_key(&o);
                    let e = by_model.entry(inst.model_name.clone()).or_default();
                    if charted { e.0 += 1 } else { e.1 += 1 }
                    rows.push((charted, format!("item {:>5} {:<22} charted {:<5} plg_u02 {:>10.4} bounds {:?} boxes_all {} tris {} mats {}", inst.item, inst.model_name, charted, m.plg_u02, m.plg_bounds.map(|b| format!("[{:.4} {:.4} {:.4} {:.4}]", b[0], b[1], b[2], b[3])).unwrap_or("none".into()), m.stored_boxes_all.len(), m.tris.len(), m.mat_links.len())));
                }
                let (nc, nu) = (rows.iter().filter(|r| r.0).count(), rows.iter().filter(|r| !r.0).count());
                { let objs: Vec<u32> = ed.keys().copied().filter(|&o| o >= base).collect(); let mx = objs.iter().max().copied().unwrap_or(0); let beyond = objs.iter().filter(|&&o| o >= base + scene.instances.len() as u32).count(); println!("mapping: {} item entries, max obj {} (base {base} + {} items = {}), {beyond} beyond the item range; binds with (obj_group_idx & 3) != 0: {}", objs.len(), mx, scene.instances.len(), base + scene.instances.len() as u32, (0..mp.count as usize).filter(|&i| mp.binds[i].obj_group_idx & 3 != 0).count()); let mut miss: Vec<u32> = (0..scene.instances.len() as u32).filter(|i| !ed.contains_key(&(base + i))).collect(); miss.truncate(30); println!("  first uncharted item indices: {:?}", miss); }
                println!("items: {nc} charted by the editor, {nu} not; per model (charted, not):");
                // --items-dir DIR (mapgeom items MAP --out DIR): the raw Solid2 of every model — which property separates the
                // charted from the uncharted?
                if let Some(dir) = f("--items-dir") {
                    let mut per: Vec<(bool, String, String)> = Vec::new();
                    for (name, (c, u)) in &by_model {
                        let p = format!("{dir}/Items/{name}");
                        let Ok(bytes) = std::fs::read(&p) else { continue };
                        let Ok(fl) = mapgeom::static_item::file::parse_file(&bytes) else { per.push((*u > 0, name.clone(), "unparsed".into())); continue };
                        let Some(so) = fl.item.static_object() else { per.push((*u > 0, name.clone(), "no static object".into())); continue };
                        let Some(s2) = so.solid2() else { per.push((*u > 0, name.clone(), "no solid2".into())); continue };
                        let plg = s2.pre_light_gen.as_ref();
                        let lod0 = s2.shaded_geoms.iter().filter(|g| g.lod_mask == 1 || g.lod_mask == 0).count();
                        let desc = format!("v{} geoms {} (lod0 {}) visuals {} lods {:?} vis_cst {} dmg {} flags {:#x} u05 {} u07 {} lights {} light_insts {} boxes {} joints {} | plg {} u01 {} u03 {} sprite {:?} boxes {} uv_groups {} | u04[4..8] {:?}", s2.version, s2.shaded_geoms.len(), lod0, s2.visuals.len(), s2.lod_max_dist, s2.vis_cst_type, s2.damage_zone, s2.flags, s2.u05, s2.u07, s2.lights.len(), s2.light_insts.len(), s2.boxes.len(), s2.joints.len(), plg.map(|g| g.version.to_string()).unwrap_or("none".into()), plg.map(|g| g.u01).unwrap_or(-9), plg.map(|g| g.u03 as i32).unwrap_or(-9), plg.map(|g| g.sprite_count).unwrap_or([-9, -9]), plg.map(|g| g.boxes.len()).unwrap_or(0), plg.map(|g| g.uv_groups.len()).unwrap_or(0), plg.map(|g| g.u04[4..8].to_vec()).unwrap_or_default());
                        per.push((*u > 0 && *c == 0, name.clone(), desc));
                    }
                    per.sort();
                    for (unch, name, d) in &per { println!("  {} {name:<22} {d}", if *unch { "UNCHARTED" } else { "charted  " }); }
                }
                for (name, (c, u)) in &by_model { if *u > 0 || a.iter().any(|x| x == "--all-models") { let ex = &scene.models[scene.instances.iter().find(|i| i.model_name == *name).unwrap().model]; let lq: std::collections::BTreeSet<u8> = scene.instances.iter().filter(|i| i.model_name == *name).map(|i| i.lm_quality).collect(); println!("  {name:<22} charted {c:>4} not {u:>4}  lm_quality {lq:?} plg_u02 {:>10.4} bounds {} boxes_all {} tris {} mats {:?}", ex.plg_u02, ex.plg_bounds.map(|b| format!("[{:.4} {:.4} {:.4} {:.4}]", b[0], b[1], b[2], b[3])).unwrap_or("none".into()), ex.stored_boxes_all.len(), ex.tris.len(), ex.mat_links.iter().take(4).collect::<Vec<_>>()); } }
                return;
            }
            // --ring-hist: the editor's tile chart size against the tile's Chebyshev distance to the nearest ITEM anchor cell
            // AT THE TILES' OWN CELL LEVEL (FUN_140dcc8e0: the ring search over the grid the items mark; a cell is found only
            // when its marked level == the tile's) — the quality-byte rule's oracle
            if a.iter().any(|x| x == "--ring-hist") {
                let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
                let tile_y: i32 = mf.baked.first().map(|b| b.coords().1).unwrap_or(5);
                // the items' cells as the file stores them (the census convention; the grid FUN_140dcc290 marks)
                let anchors: Vec<(i32, i32, i32)> = mf.items.iter().map(|it| (it.file_cell[0] as i32, it.file_cell[1] as i32, it.file_cell[2] as i32)).collect();
                println!("tiles at cell level y {tile_y}; item anchor cells (x, y, z): {:?}", anchors);
                let mut hist: std::collections::BTreeMap<(i32, u16, u16), usize> = Default::default();
                for (o, &(_, _, w, h)) in &ed {
                    if *o >= base { continue; }
                    let Some(&(cx, cz)) = cell_of.get(*o as usize) else { continue };
                    let mut r = i32::MAX;
                    for &(ax, ay, az) in &anchors { if ay != tile_y { continue; } r = r.min((ax - cx).abs().max((az - cz).abs())); }
                    let r = if r == i32::MAX { 99 } else { r };
                    *hist.entry((r, w, h)).or_default() += 1;
                }
                let mut cur = -1; let mut line = String::new();
                for ((r, w, h), n) in &hist { if *r != cur { if !line.is_empty() { println!("{line}"); } cur = *r; line = format!("  r {r:>2}:"); } line += &format!("  {w}×{h} ×{n}"); }
                println!("{line}");
                return;
            }
            // --via-layout: the library's `layout::for_map` (what `bake --layout-game` uses) against the editor's table — must agree
            // with this command's own walk
            if a.iter().any(|x| x == "--via-layout") {
                let pak_arg = f("--pak");
                let pak: Option<(&str, &str)> = pak_arg.as_deref().and_then(|p| p.rsplit_once(':'));
                let q = lightmap::layout::quality_index_of(&own).unwrap_or(2);
                let gl = lightmap::layout::for_map(&a[1], &scene, base, q, lightmap::layout::TilePlg::BLUEBAY_SEA, pak, &f("--collection").unwrap_or_else(|| "BlueBay".into()), &f("--zone").unwrap_or_else(|| "Sea".into()), kept.as_ref()).expect("layout");
                let (mut n, mut ok, mut bound) = (0usize, 0usize, 0usize);
                // the mismatch classes: same size elsewhere (a placement / cell-order difference) vs a different size
                let (mut same_size, mut diff_size, mut shown) = (0usize, 0usize, 0usize);
                let mut size_hist: std::collections::BTreeMap<(i32, i32, i32, i32), usize> = Default::default();
                for c in &gl.charts { if c.charted == lightmap::layout::Charted::Bound { bound += 1; } if let Some(&(ex, ey, ew, eh)) = ed.get(&c.obj) { n += 1; if c.x == ex as i32 && c.y == ey as i32 && c.w == ew as i32 && c.h == eh as i32 { ok += 1; } else { if c.w == ew as i32 && c.h == eh as i32 { same_size += 1; } else { diff_size += 1; *size_hist.entry((c.w, c.h, ew as i32, eh as i32)).or_default() += 1; } if shown < 10 && a.iter().any(|x| x == "--show-misses") { shown += 1; println!("  miss obj {}: ours ({}, {}) {}×{} editor ({ex}, {ey}) {ew}×{eh}", c.obj, c.x, c.y, c.w, c.h); } } } }
                println!("via layout::for_map: s {} Σarea {} maxIter {}; {} charts ({bound} bound), {ok} of {n} equal to the editor's table (of {} editor entries); misses: {same_size} same size elsewhere, {diff_size} other size", gl.s, gl.sum_area, gl.max_iter, gl.charts.len(), ed.len());
                if diff_size > 0 { let mut v: Vec<_> = size_hist.into_iter().collect(); v.sort_by_key(|(_, n)| std::cmp::Reverse(*n)); println!("  size differences (ours w×h → editor w×h: count): {:?}", v.iter().take(12).map(|((a, b, c, d), n)| format!("{a}×{b}→{c}×{d}:{n}")).collect::<Vec<_>>()); }

                // --obj-diff: the objects only one side has (ours without an editor entry / the editor's without a chart of ours)
                if a.iter().any(|x| x == "--obj-diff") {
                    let ours: std::collections::BTreeSet<u32> = gl.charts.iter().map(|c| c.obj).collect();
                    let theirs: std::collections::BTreeSet<u32> = ed.keys().copied().collect();
                    let only_ours: Vec<u32> = ours.difference(&theirs).copied().collect();
                    let only_theirs: Vec<u32> = theirs.difference(&ours).copied().collect();
                    println!("  objects only ours: {} {:?}", only_ours.len(), &only_ours[..only_ours.len().min(40)]);
                    // our record items by (PLG bounds, material count) class
                    let mut cls: std::collections::BTreeMap<String, usize> = Default::default();
                    for c in &gl.charts { if c.obj >= base { let it = (c.obj - base) as usize; if let Some(inst) = scene.instances.iter().find(|i| i.item == it) { let m = &scene.models[inst.model]; *cls.entry(format!("PLG {:?} mats {} tris {}", m.plg_bounds.map(|b| [(b[0] * 1000.0).round() / 1000.0, (b[1] * 1000.0).round() / 1000.0, (b[2] * 1000.0).round() / 1000.0, (b[3] * 1000.0).round() / 1000.0]), m.mat_links.len().min(1), if m.tris.is_empty() { 0 } else { 1 })).or_default() += 1; } } }
                    let mut v: Vec<_> = cls.into_iter().collect(); v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
                    println!("  our item record classes: {:?}", &v[..v.len().min(10)]);
                    println!("  scene: {} instances, {} models; file items {}", scene.instances.len(), scene.models.len(), scene.instances.iter().map(|i| i.item).max().unwrap_or(0) + 1);
                    // --obj-align: the editor numbers its item objects by RANK among the record items; align our record items (item
                    // order, our chart size) with the editor's item entries (obj order, size) greedily — an entry of ours whose size is
                    // not within 25 % of the editor's next entry is one the editor did not chart
                    if a.iter().any(|x| x == "--obj-align") {
                        let mut ours_items: Vec<(u32, i32, i32)> = gl.charts.iter().filter(|c| c.obj >= base).map(|c| (c.obj, c.w, c.h)).collect();
                        ours_items.sort();
                        let mut ed_items: Vec<(u32, i32, i32)> = ed.iter().filter(|(o, _)| **o >= base).map(|(o, r)| (*o, r.2 as i32, r.3 as i32)).collect();
                        ed_items.sort();
                        let (mut i, mut j) = (0usize, 0usize);
                        let mut excluded: Vec<u32> = Vec::new();
                        let mut extra_ed = 0usize;
                        while i < ours_items.len() && j < ed_items.len() {
                            let (o, w, h) = ours_items[i]; let (_, ew, eh) = ed_items[j];
                            let close = |a: i32, b: i32| (a - b).abs() as f32 <= 0.25 * (a.max(b) as f32) + 2.0;
                            if close(w, ew) && close(h, eh) { i += 1; j += 1; }
                            else if i + 1 < ours_items.len() && close(ours_items[i + 1].1, ew) && close(ours_items[i + 1].2, eh) { excluded.push(o); i += 1; }
                            else { extra_ed += 1; j += 1; }
                        }
                        while i < ours_items.len() { excluded.push(ours_items[i].0); i += 1; }
                        println!("  obj-align: {} of ours excluded by the editor, {} editor entries unmatched; excluded items: {:?}", excluded.len(), extra_ed + (ed_items.len() - j), excluded.iter().take(30).map(|o| o - base).collect::<Vec<_>>());
                        let mut cls: std::collections::BTreeMap<String, usize> = Default::default();
                        for o in &excluded { let it = (o - base) as usize; if let Some(inst) = scene.instances.iter().find(|i| i.item == it) { let m = &scene.models[inst.model]; *cls.entry(format!("model {} PLG {:?} mats {} lm_uv_geoms {} tris {}", inst.model, m.plg_bounds.map(|b| [(b[0] * 1000.0).round() / 1000.0, (b[1] * 1000.0).round() / 1000.0, (b[2] * 1000.0).round() / 1000.0, (b[3] * 1000.0).round() / 1000.0]), m.mat_links.len(), m.lm_uv_geoms, m.tris.len())).or_default() += 1; } }
                        for (k, n) in &cls { println!("    {n:4} × {k}"); }
                    }
                    println!("  objects only the editor's: {} {:?}", only_theirs.len(), &only_theirs[..only_theirs.len().min(40)]);
                    for &o in only_ours.iter().take(6) { if o >= base { let it = (o - base) as usize; if let Some(inst) = scene.instances.iter().find(|i| i.item == it) { let m = &scene.models[inst.model]; println!("    ours obj {o} = item {it} model {} PLG {:?} mats {:?}", inst.model, m.plg_bounds, m.mat_links.iter().take(3).collect::<Vec<_>>()); } } }
                    for &o in only_theirs.iter().take(6) { if o >= base { let it = (o - base) as usize; if let Some(inst) = scene.instances.iter().find(|i| i.item == it) { let m = &scene.models[inst.model]; println!("    editor obj {o} = item {it} model {} PLG {:?} mats {:?} rect {:?}", inst.model, m.plg_bounds, m.mat_links.iter().take(3).collect::<Vec<_>>(), ed.get(&o)); } else { println!("    editor obj {o} = item {it}: not in the scene"); } } }
                }                // --cell-study: per grid entry (nb·na > 1) whose members all have editor rects: the editor's cell edge sequence per axis
                // (from the members' rects) against ours — the error-diffusion rule study
                if a.iter().any(|x| x == "--cell-study") {
                    let (g, pad, _m) = gl.params;
                    let mut shown = 0;
                    let mut agree = 0usize; let mut total = 0usize;
                    let mut miss_kinds: std::collections::BTreeMap<String, usize> = Default::default();
                    for (rect, (nb, na), mem) in &gl.entries {
                        if nb * na <= 1 { continue; }
                        let rects: Vec<Option<(u32, u32, u32, u32)>> = mem.iter().map(|(k, _)| ed.get(&gl.charts[*k].obj).map(|r| (r.0 as u32, r.1 as u32, r.2 as u32, r.3 as u32))).collect();
                        if rects.iter().any(|r| r.is_none()) { continue; }
                        total += 1;
                        // the editor's distinct cell x starts / widths along the row (cells share x per column)
                        let mut xs: Vec<(u32, u32)> = rects.iter().map(|r| { let r = r.unwrap(); (r.0, r.2) }).collect(); xs.sort(); xs.dedup();
                        let mut ys: Vec<(u32, u32)> = rects.iter().map(|r| { let r = r.unwrap(); (r.1, r.3) }).collect(); ys.sort(); ys.dedup();
                        let ex = lightmap::itemrule::cell_edges(rect.2 as u32, *nb, g as u32);
                        let ey = lightmap::itemrule::cell_edges(rect.3 as u32, *na, g as u32);
                        let ours_x: Vec<(u32, u32)> = (0..*nb as usize).map(|i| (rect.0 as u32 + ex[i] + pad as u32, ex[i + 1] - ex[i] - 2 * pad as u32)).collect();
                        let ours_y: Vec<(u32, u32)> = (0..*na as usize).map(|i| (rect.1 as u32 + ey[i] + pad as u32, ey[i + 1] - ey[i] - 2 * pad as u32)).collect();
                        let ok = xs == ours_x && ys == ours_y;
                        if ok { agree += 1; } else {
                            let ed_wx: Vec<u32> = xs.iter().map(|(_, w)| *w).collect(); let our_wx: Vec<u32> = ours_x.iter().map(|(_, w)| *w).collect();
                            let ed_wy: Vec<u32> = ys.iter().map(|(_, w)| *w).collect(); let our_wy: Vec<u32> = ours_y.iter().map(|(_, w)| *w).collect();
                            *miss_kinds.entry(format!("w {} nb {nb}: editor {ed_wx:?} ours {our_wx:?}", rect.2)).or_default() += 1;
                            if ed_wy != our_wy { *miss_kinds.entry(format!("h {} na {na}: editor {ed_wy:?} ours {our_wy:?}", rect.3)).or_default() += 1; }
                            if shown < 6 { shown += 1; println!("  entry {:?} {nb}×{na}: editor x-cells {:?} ours {:?}; y-cells {:?} ours {:?}", rect, xs, ours_x, ys, ours_y); }
                        }
                    }
                    println!("cell study: {agree} of {total} grid entries with every member in the editor's table have our cell edges");
                    // the ENTRY rects: the editor's union of the members' rects (outer: minus the pad) vs our placed entry
                    let (mut e_same, mut e_moved, mut e_size) = (0usize, 0usize, 0usize);
                    let mut moved_examples: Vec<String> = Vec::new();
                    let mut size_kinds: std::collections::BTreeMap<(i32, i32, i32, i32), usize> = Default::default();
                    let mut moved_rows: Vec<String> = Vec::new();
                    for (ei, (rect, (nb, na), mem)) in gl.entries.iter().enumerate() {
                        let rects: Vec<Option<(u32, u32, u32, u32)>> = mem.iter().map(|(k, _)| ed.get(&gl.charts[*k].obj).map(|r| (r.0 as u32, r.1 as u32, r.2 as u32, r.3 as u32))).collect();
                        if rects.iter().any(|r| r.is_none()) { continue; }
                        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
                        for r in rects.iter().flatten() { x0 = x0.min(r.0 - pad as u32); y0 = y0.min(r.1 - pad as u32); x1 = x1.max(r.0 + r.2 + pad as u32); y1 = y1.max(r.1 + r.3 + pad as u32); }
                        // a partially filled grid (empty z-last cells) makes the union smaller than the entry: compare sizes only when full
                        let full = (nb * na) as usize == mem.len();
                        let ours = (rect.0 as u32, rect.1 as u32, rect.2 as u32, rect.3 as u32);
                        if (x0, y0) == (ours.0, ours.1) && (!full || (x1 - x0, y1 - y0) == (ours.2, ours.3)) { e_same += 1; }
                        else if !full || (x1 - x0, y1 - y0) == (ours.2, ours.3) { e_moved += 1; let k = gl.entry_keys[ei]; if moved_examples.len() < 5 { moved_examples.push(format!("{nb}×{na} {}×{} ours ({}, {}) editor ({x0}, {y0})", ours.2, ours.3, ours.0, ours.1)); } moved_rows.push(format!("entry {ei} group {} chunk {} area {:#010x} n {} {nb}×{na}: ours ({}, {}) editor ({x0}, {y0})", k.0, k.1, k.2.to_bits(), mem.len(), ours.0, ours.1)); }
                        else { e_size += 1; *size_kinds.entry((ours.2 as i32, ours.3 as i32, (x1 - x0) as i32, (y1 - y0) as i32)).or_default() += 1; }
                    }
                    println!("entry study: {e_same} entries at the editor's place and size, {e_moved} same size elsewhere, {e_size} another size; moved e.g. {moved_examples:?}");
                    if a.iter().any(|x| x == "--show-moved") { for r in moved_rows.iter().take(40) { println!("    {r}"); } }
                    // the first moved entry in WALK order (the packer's processing order): where the placements diverge
                    let wp = lightmap::layout::WALK_POS.with(|w| w.borrow().clone());
                    let mut first: Vec<(usize, String)> = Vec::new();
                    for (ei, (rect, (nb, na), mem)) in gl.entries.iter().enumerate() {
                        let rects: Vec<Option<(u32, u32, u32, u32)>> = mem.iter().map(|(k, _)| ed.get(&gl.charts[*k].obj).map(|r| (r.0 as u32, r.1 as u32, r.2 as u32, r.3 as u32))).collect();
                        if rects.iter().any(|r| r.is_none()) { continue; }
                        let (mut x0, mut y0) = (u32::MAX, u32::MAX);
                        for r in rects.iter().flatten() { x0 = x0.min(r.0 - pad as u32); y0 = y0.min(r.1 - pad as u32); }
                        if (x0, y0) != (rect.0 as u32, rect.1 as u32) { let k = gl.entry_keys[ei]; first.push((wp.get(ei).copied().unwrap_or(0), format!("walk {} entry {ei} group {} chunk {} area {:#010x} {nb}×{na} {}×{}: ours ({}, {}) editor ({x0}, {y0})", wp.get(ei).copied().unwrap_or(0), k.0, k.1, k.2.to_bits(), rect.2, rect.3, rect.0, rect.1))); }
                    }
                    first.sort();
                    // the placements around the first divergence, in walk order (ours; the editor's union rect when known)
                    if let Some((pos0, _)) = first.first() {
                        let lo = pos0.saturating_sub(10);
                        let mut by_walk: Vec<(usize, usize)> = wp.iter().enumerate().map(|(ei, p)| (*p, ei)).collect(); by_walk.sort();
                        println!("  placements at walk {lo}..={}:", pos0 + 3);
                        for (p, ei) in by_walk.iter().filter(|(p, _)| *p >= lo && *p <= pos0 + 3) {
                            let (rect, (nb, na), mem) = &gl.entries[*ei];
                            let k = gl.entry_keys[*ei];
                            let rects: Vec<Option<(u32, u32, u32, u32)>> = mem.iter().map(|(kk, _)| ed.get(&gl.charts[*kk].obj).map(|r| (r.0 as u32, r.1 as u32, r.2 as u32, r.3 as u32))).collect();
                            let edp = if rects.iter().all(|r| r.is_some()) { let (mut x0, mut y0) = (u32::MAX, u32::MAX); for r in rects.iter().flatten() { x0 = x0.min(r.0 - pad as u32); y0 = y0.min(r.1 - pad as u32); } format!("({x0}, {y0})") } else { "?".into() };
                            println!("    walk {p}: entry {ei} g{} c{} area {:#010x} {nb}×{na} {}×{} ours ({}, {}) editor {edp}", k.0, k.1, k.2.to_bits(), rect.2, rect.3, rect.0, rect.1);
                        }
                    }
                    // --region x0,y0,x1,y1: the editor's charts in that region with their entries' walk positions (ours)
                    if let Some(rg) = f("--region") {
                        let v: Vec<u32> = rg.split(',').map(|t| t.parse().unwrap()).collect();
                        let entry_of_chart: std::collections::HashMap<usize, usize> = gl.entries.iter().enumerate().flat_map(|(ei, (_, _, mem))| mem.iter().map(move |(k, _)| (*k, ei))).collect();
                        let chart_of_obj: std::collections::HashMap<u32, usize> = gl.charts.iter().enumerate().map(|(k, c)| (c.obj, k)).collect();
                        let mut rows: Vec<(u32, u32, String)> = Vec::new();
                        let mut seen: std::collections::HashSet<usize> = Default::default();
                        for (obj, &(ex, ey, ew, eh)) in ed.iter() {
                            if (ex as u32) >= v[0] && (ey as u32) >= v[1] && (ex as u32 + ew as u32) <= v[2] && (ey as u32 + eh as u32) <= v[3] {
                                if let Some(&ei) = chart_of_obj.get(obj).and_then(|k| entry_of_chart.get(k)) {
                                    if !seen.insert(ei) { continue; }
                                    let k = gl.entry_keys[ei]; let r = gl.entries[ei].0;
                                    rows.push((ey as u32, ex as u32, format!("entry {ei} g{} c{} area {:#010x} walk {} {}×{}: ours ({}, {}); editor cell at ({ex}, {ey})", k.0, k.1, k.2.to_bits(), wp.get(ei).copied().unwrap_or(0), r.2, r.3, r.0, r.1)));
                                }
                            }
                        }
                        rows.sort();
                        println!("  editor entries in region {rg}:");
                        for (_, _, r) in rows.iter().take(30) { println!("    {r}"); }
                    }
                    println!("first moved entries in walk order ({} moved of {} entries):", first.len(), gl.entries.len());
                    for (_, r) in first.iter().take(12) { println!("    {r}"); }
                    // what the editor placed inside OUR rect of the first moved entry: the objects there → their entry (group, chunk, walk)
                    if let Some((pos0, _)) = first.first() {
                        let ei0 = wp.iter().position(|p| p == pos0).unwrap_or(0);
                        let r0 = gl.entries[ei0].0;
                        let mut inside: Vec<(u32, u32, u32, String)> = Vec::new();
                        let entry_of_chart: std::collections::HashMap<usize, usize> = gl.entries.iter().enumerate().flat_map(|(ei, (_, _, mem))| mem.iter().map(move |(k, _)| (*k, ei))).collect();
                        let chart_of_obj: std::collections::HashMap<u32, usize> = gl.charts.iter().enumerate().map(|(k, c)| (c.obj, k)).collect();
                        for (obj, &(ex, ey, ew, eh)) in ed.iter() {
                            if (ex as i32) >= r0.0 && (ey as i32) >= r0.1 && (ex as i32 + ew as i32) <= r0.0 + r0.2 && (ey as i32 + eh as i32) <= r0.1 + r0.3 {
                                let desc = chart_of_obj.get(obj).and_then(|k| entry_of_chart.get(k)).map(|&ei| { let k = gl.entry_keys[ei]; format!("entry {ei} group {} chunk {} area {:#010x} walk {} ours at ({}, {})", k.0, k.1, k.2.to_bits(), wp.get(ei).copied().unwrap_or(0), gl.entries[ei].0.0, gl.entries[ei].0.1) }).unwrap_or_else(|| "not in our layout".into());
                                inside.push((ey as u32, ex as u32, *obj, desc));
                            }
                        }
                        inside.sort();
                        println!("  the editor's charts inside our first moved entry's rect {:?}:", r0);
                        for (ey, ex, obj, d) in inside.iter().take(12) { println!("    obj {obj} at ({ex}, {ey}): {d}"); }
                        // the members of the entry the editor put there, and of ours: record index, ordinal, the record's own area bits
                        for ei in [ei0].into_iter().chain(inside.first().and_then(|(_, _, obj, _)| chart_of_obj.get(obj).and_then(|k| entry_of_chart.get(k)).copied())) {
                            let (_, (nb, na), mem) = &gl.entries[ei];
                            let k = gl.entry_keys[ei];
                            println!("  entry {ei} (group {} chunk {} {nb}×{na}, entry area {:#010x}): members (record, ordinal, own grid-area bits):", k.0, k.1, k.2.to_bits());
                            let mut rows: Vec<String> = Vec::new();
                            for (kk, o) in mem { let e = gl.charts[*kk].ext; let area = ((nb * na) as f32 * e[1]) * e[0]; rows.push(format!("({kk}, {o}, {:#010x})", area.to_bits())); }
                            println!("    {}", rows.join(" "));
                        }
                    }
                    let mut v: Vec<_> = size_kinds.into_iter().collect(); v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
                    println!("  entry size differences (ours → editor): {:?}", v.iter().take(10).map(|((a, b, c, d), n)| format!("{a}×{b}→{c}×{d}:{n}")).collect::<Vec<_>>());
                    let mut v: Vec<_> = miss_kinds.into_iter().collect(); v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
                    for (k, n) in v.iter().take(14) { println!("    {n:4} × {k}"); }
                }
                return;
            }
            // --tie KEY: the order among equal areas (the editor sorts by block position-like keys before the
            // area): index (default), x, z, y, -x, -z, -y, or combos like "z,x" (last key = most significant)
            // the radix keys (RE 6 / spec §3.1): ascending (area, centre z, y, x, |h|², record index), walked from the end — "x,z"
            let tie = f("--tie").unwrap_or_else(|| "x,z".into());
            let placed_order: Vec<usize> = {
                let mut idx: Vec<usize> = (0..charts.len()).collect();
                // --pak FILE:KEY [--collection BlueBay] [--zone Sea] [--cell-y 5] [--yoff -40]: THE GAME'S KEYS — every chart's
                // block record (RE 6, lmtiles): the tiles from the zone prefab's stored visual boxes through the cell Iso4
                // (`tile_records`), the items from their model's stored boxes through the item Iso4 (`item_records`); the radix
                // order = ascending (area, centre z, centre y, centre x, |h|², record index), floats compared sign-aware
                // (−0.0 below +0.0), walked from the end. Without --pak the cell-centre approximation below stands.
                let game_keys: Option<std::collections::HashMap<u32, ([f32; 3], f32)>> = f("--pak").map(|pak| {
                    let (pak_path, key) = pak.rsplit_once(':').expect("--pak FILE:KEY");
                    let mut store = mapgeom::store::DataStore::empty();
                    store.add_pak(pak_path, key).expect("pak");
                    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
                    let chunks = tmmaps::gbx::all_skip_chunks(&mf.gbx.body);
                    let gen: Vec<(String, u32)> = chunks.iter().find(|(c, ..)| *c == 0x0304_3043).and_then(|&(_, _, payload, size)| tmmaps::map::genealogy_full(&mf.gbx.body[payload..payload + size]).ok()).map(|recs| recs.into_iter().map(|r| (r.current, r.dir)).collect()).unwrap_or_default();
                    let size = [mf.size[0].max(0) as usize, mf.size[1].max(0) as usize, mf.size[2].max(0) as usize];
                    let cell_y: f32 = f("--cell-y").map(|v| v.parse().unwrap()).unwrap_or(mf.baked.first().map(|b| b.coords().1 as f32).unwrap_or(5.0));
                    let tiles = lightmap::lmtiles::tile_records(&mut store, &f("--collection").unwrap_or_else(|| "BlueBay".into()), size, &gen, &f("--zone").unwrap_or_else(|| "Sea".into()), cell_y, f("--yoff").map(|v| v.parse().unwrap()).unwrap_or(-40.0), 1.0).expect("tile records");
                    let mut by_cell: std::collections::HashMap<(i32, i32), ([f32; 3], f32)> = Default::default();
                    for (cx, cz, _zone, _dir, rec) in &tiles { let h = rec.world.h; by_cell.insert((*cx as i32, *cz as i32), (rec.world.c, (h[0] * h[0] + h[1] * h[1]) + h[2] * h[2])); }
                    let mut keys: std::collections::HashMap<u32, ([f32; 3], f32)> = Default::default();
                    for (o, &(cx, cz)) in cell_of.iter().enumerate() { if let Some(k) = by_cell.get(&(cx, cz)) { keys.insert(o as u32, *k); } }
                    for it in lightmap::lmtiles::item_records(&scene, 1.0, false) { if let Some(r) = it.record { let h = r.world.h; keys.insert(base + it.item as u32, (r.world.c, (h[0] * h[0] + h[1] * h[1]) + h[2] * h[2])); } }
                    println!("game keys: {} tile records (cell y {cell_y}), {} item records; e.g. tile 0 {:?}, items {:?}", tiles.len(), scene.instances.len(), keys.get(&0), scene.instances.iter().map(|i| keys.get(&(base + i.item as u32)).map(|k| k.0)).collect::<Vec<_>>());
                    keys
                });
                if let Some(gk) = &game_keys {
                    // sign-aware float order (the radix sorter compares the bit patterns: −0.0 sorts below +0.0)
                    let fcmp = |x: f32, y: f32| -> std::cmp::Ordering { let kx = if x.is_sign_negative() { !(x.to_bits()) } else { x.to_bits() | 0x8000_0000 }; let ky = if y.is_sign_negative() { !(y.to_bits()) } else { y.to_bits() | 0x8000_0000 }; kx.cmp(&ky) };
                    let mut idx: Vec<usize> = (0..charts.len()).collect();
                    idx.sort_by(|&p, &q| {
                        let (ap, aq) = (charts[p].ext[0] * charts[p].ext[1], charts[q].ext[0] * charts[q].ext[1]);
                        let (kp, kq) = (gk.get(&ids[p]).copied().unwrap_or(([0.0; 3], 0.0)), gk.get(&ids[q]).copied().unwrap_or(([0.0; 3], 0.0)));
                        // --key-order zyx (default: z most significant, then y, x) | yzx | zxy … — the study switch
                        let ko = f("--key-order").unwrap_or_else(|| "zyx".into());
                        let ax = |c: char| -> usize { match c { 'x' => 0, 'y' => 1, _ => 2 } };
                        let kc: Vec<usize> = ko.chars().map(ax).collect();
                        fcmp(ap, aq).then(fcmp(kp.0[kc[0]], kq.0[kc[0]])).then(fcmp(kp.0[kc[1]], kq.0[kc[1]])).then(fcmp(kp.0[kc[2]], kq.0[kc[2]])).then(fcmp(kp.1, kq.1)).then(ids[p].cmp(&ids[q]))
                    });
                    idx
                } else {
                // the keys are the block's world bbox CENTRE (RE child 2): items from their transformed
                // triangles, tiles from their cell (x-major or z-major: --tiles-zmajor) at the sea height
                let tile_zmajor = a.iter().any(|x| x == "--tiles-zmajor");
                let tile_y: f32 = f("--tile-y").map(|s| s.parse().unwrap()).unwrap_or(8.0);
                let centres: std::collections::HashMap<u32, [f32; 3]> = scene.instances.iter().map(|inst| {
                    let mdl = &scene.models[inst.model];
                    let mut lo = [f32::MAX; 3]; let mut hi = [f32::MIN; 3];
                    for t in &mdl.tris { for p in &t.p { let w = lightmap::geometry::xf_point(&inst.xf, *p); for k in 0..3 { lo[k] = lo[k].min(w[k]); hi[k] = hi[k].max(w[k]); } } }
                    if std::env::var_os("LMTOOL_PACK_TRACE").is_some() { eprintln!("  item {} triangle bbox lo {:?} hi {:?} (xf {:?})", inst.item, lo, hi, inst.xf); }
                    (base + inst.item as u32, [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0])
                }).collect();
                for (o, c) in &centres { let inst = scene.instances.iter().find(|i| base + i.item as u32 == *o).unwrap(); println!("  item {} centre (triangle bbox) {:?}; the tile cells' centres are (cx·32 + 16, {tile_y}, cz·32 + 16)", o - base, c); let _ = inst; }
                // the tile's centre: its cell (the true obj → cell map above; --tiles-idgrid = the old (o % 64, o / 64) guess) at the sea height
                let pos_of = |k: usize| -> [f32; 3] { if ids[k] >= base { centres.get(&ids[k]).copied().unwrap_or([0.0; 3]) } else { let o = ids[k]; let (cx, cz) = if a.iter().any(|x| x == "--tiles-idgrid") { if tile_zmajor { ((o / 64) as i32, (o % 64) as i32) } else { ((o % 64) as i32, (o / 64) as i32) } } else { cell_of.get(o as usize).copied().unwrap_or((0, 0)) }; [cx as f32 * 32.0 + 16.0, tile_y, cz as f32 * 32.0 + 16.0] } };
                for key in tie.split(',') {
                    let (neg, axis) = match key.trim() { "x" => (false, 0), "y" => (false, 1), "z" => (false, 2), "-x" => (true, 0), "-y" => (true, 1), "-z" => (true, 2), _ => (false, 9) };
                    if axis < 3 { idx.sort_by(|&a, &b| { let (pa, pb) = (pos_of(a)[axis], pos_of(b)[axis]); let o = pa.partial_cmp(&pb).unwrap(); if neg { o.reverse() } else { o } }); }
                }
                // the area sort last (stable)
                idx.sort_by_key(|&i| (charts[i].ext[0] * charts[i].ext[1]).to_bits());
                idx
                }
            };
            // --s X: one TryPack at a given s (layout units per metre) instead of the scale search — to test the
            // sizes the editor's own s produces; --try-s A,B,N: the success/failure of TryPack over a range of s
            if let Some(r) = f("--try-s") {
                let v: Vec<f32> = r.split(',').map(|t| t.parse().unwrap()).collect();
                let n = v[2] as usize;
                for i in 0..=n {
                    let s = v[0] + (v[1] - v[0]) * i as f32 / n as f32;
                    let ok = lightmap::pack::try_pack(&charts, &placed_order, s, w_atlas, w_atlas, g, mmin).is_some();
                    println!("  s {s:.4}: TryPack {}", if ok { "succeeds" } else { "FAILS" });
                }
                return;
            }
            if let Some(r) = f("--fit-round") { lightmap::pack::FIT_ROUND.store(r.parse().unwrap(), std::sync::atomic::Ordering::Relaxed); }
            if let Some(r) = f("--carry-mode") { lightmap::pack::CARRY_MODE.store(r.parse().unwrap(), std::sync::atomic::Ordering::Relaxed); }
            if let Some(r) = f("--a-order") { lightmap::pack::A_ORDER.store(r.parse().unwrap(), std::sync::atomic::Ordering::Relaxed); }
            if a.iter().any(|x| x == "--no-node-cap") { lightmap::pack::PACK_NODE_CAP.store(false, std::sync::atomic::Ordering::Relaxed); }
            if a.iter().any(|x| x == "--editor-sum") { for c in &d.cache.chunks { if c.id == 0x0602_200B { if let lightmap::format::ChunkBody::Raw(b) = &c.body { if b.len() >= 8 { lightmap::pack::SUM_AREA_OVERRIDE.store(u32::from_le_bytes([b[4], b[5], b[6], b[7]]), std::sync::atomic::Ordering::Relaxed); } } } } }
            let forced_s: Option<f32> = f("--s").map(|v| v.parse().unwrap());
            let fail_iters: Vec<u32> = f("--fail-iters").map(|v| v.split(',').map(|t| t.parse().unwrap()).collect()).unwrap_or_default();
            // --carry-audit: replay the walk with the EDITOR's tile sizes (so the carry sequence is the game's up to a constant
            // offset) and derive, per tile, the one-sided constraint the editor's bump decisions put on that offset: off ≥ c* − c
            // where the editor bumped, off < c* − c where it did not (c* = A·(24/X)² − a, the carry at which the fit reaches 24).
            // A consistent [lo, hi) window = the model is exact up to the items' carry; an empty one = a per-tile drift.
            if a.iter().any(|x| x == "--carry-audit") {
                let k = f32::from_bits(0x3d3504f3);
                let (mbu, b) = (f32::from_bits(0x420dc57e), [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)]);
                let ext_t = tile_ext_of(&b, mbu, k, korder);
                let mut ch = charts.clone();
                for (c, &o) in ch.iter_mut().zip(&ids) { if o < base { c.ext = ext_t; } }
                let Some((s, placed)) = lightmap::pack::allocate_ordered_forced(&ch, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) else { println!("allocation failed"); return };
                let s = f32::from_bits((s.to_bits() as i64 + f("--s-ulps").map(|v| v.parse::<i64>().unwrap()).unwrap_or(0)) as u32);
                let placed = lightmap::pack::try_pack(&ch, &placed_order, s, w_atlas, w_atlas, g, mmin).unwrap_or(placed);
                let a_t = ((ext_t[1] * s) * ext_t[0]) * s;
                let area_t = ext_t[0] * ext_t[1];
                let (xx, yy) = (ext_t[0] as f64 * s as f64, ext_t[1] as f64 * s as f64);
                let c_star = |v: f64, target: f64| -> f64 { area_t as f64 * (target / v).powi(2) * (s as f64).powi(2) / (s as f64).powi(2) - a_t as f64 + (area_t as f64 * ((target / v).powi(2) - 1.0)) * 0.0 };
                // c* such that v·sqrt((a + c*)/A) = target  ⇔  c* = A·(target/v)² − a
                let cs_w = a_t as f64 * ((24.0 / xx).powi(2) - 1.0);
                let cs_h = a_t as f64 * ((24.0 / yy).powi(2) - 1.0);
                let _ = c_star;
                println!("carry-audit: s {s}, tile a {a_t}, X {xx:.6} Y {yy:.6}, bump thresholds c*_w {cs_w:.6} c*_h {cs_h:.6}");
                let mut carry = 0f32;
                let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
                let (mut lo_k, mut hi_k) = (0usize, 0usize);
                let mut kk = 0usize;
                let mut n_bump_w = 0usize; let mut n_bump_h = 0usize;
                for &i in placed_order.iter().rev() {
                    if ids[i] >= base {
                        let p = &placed[i]; let c = &ch[i];
                        let av = ((c.ext[1] * s) * c.ext[0]) * s;
                        carry = carry + (av - (p.w as i32 * p.h as i32) as f32);
                        continue;
                    }
                    kk += 1;
                    let Some(&(_, _, ew, eh)) = ed.get(&ids[i]) else { continue };
                    let c = carry.max(0.0) as f64;
                    let bw = ew == 22; let bh = eh == 22;
                    if bw { n_bump_w += 1; if cs_w - c > lo { lo = cs_w - c; lo_k = kk; } } else if cs_w - c < hi { hi = cs_w - c; hi_k = kk; }
                    if bh { n_bump_h += 1; if cs_h - c > lo { lo = cs_h - c; lo_k = kk; } } else if cs_h - c < hi { hi = cs_h - c; hi_k = kk; }
                    let (pw, ph) = (ew as i32 + 2 * pad as i32, eh as i32 + 2 * pad as i32);
                    carry = carry + (a_t - (pw * ph) as f32);
                    if kk % 512 == 0 || kk == 4096 { println!("  through tile #{kk}: offset window [{lo:.6}, {hi:.6}) (lo from #{lo_k}, hi from #{hi_k}); carry {carry}"); }
                }
                println!("editor bumps: w {n_bump_w}, h {n_bump_h} of {kk}; the offset window over all tiles: [{lo:.6}, {hi:.6}) {}", if lo < hi { "CONSISTENT" } else { "EMPTY (drift)" });
                return;
            }
            // --items-ext-variants: the items' ext under the candidate f32 formulas (uv bounds × MeterByUv) × the a op orders:
            // which combination lands Σ(a − w·h) in the window the carry audit demands ([+0.198, +0.213) over ours at s − 1 ulp)
            if a.iter().any(|x| x == "--items-ext-variants") {
                let k = f32::from_bits(0x3d3504f3);
                let (mbu, b) = (f32::from_bits(0x420dc57e), [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)]);
                let ext_t = tile_ext_of(&b, mbu, k, korder);
                let mut ch = charts.clone();
                for (c, &o) in ch.iter_mut().zip(&ids) { if o < base { c.ext = ext_t; } }
                let Some((s, placed)) = lightmap::pack::allocate_ordered_forced(&ch, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) else { println!("allocation failed"); return };
                let s = f32::from_bits((s.to_bits() as i64 + f("--s-ulps").map(|v| v.parse::<i64>().unwrap()).unwrap_or(0)) as u32);
                let placed = lightmap::pack::try_pack(&ch, &placed_order, s, w_atlas, w_atlas, g, mmin).unwrap_or(placed);
                let ext_f: [(&str, fn(&[f32; 4], f32) -> [f32; 2]); 4] = [
                    ("(b2−b0)·u", |b, u| [(b[2] - b[0]) * u, (b[3] - b[1]) * u]),
                    ("b2·u − b0·u", |b, u| [b[2] * u - b[0] * u, b[3] * u - b[1] * u]),
                    ("u·(b2−b0)", |b, u| [u * (b[2] - b[0]), u * (b[3] - b[1])]),
                    ("(b2−b0)·u·1.0 (byte 255/255)", |b, u| [((b[2] - b[0]) * u) * (255.0f32 / 255.0), ((b[3] - b[1]) * u) * (255.0f32 / 255.0)]),
                ];
                let a_f: [(&str, fn([f32; 2], f32) -> f32); 6] = [
                    ("((y·s)·x)·s", |e, s| ((e[1] * s) * e[0]) * s),
                    ("((x·s)·y)·s", |e, s| ((e[0] * s) * e[1]) * s),
                    ("(x·s)·(y·s)", |e, s| (e[0] * s) * (e[1] * s)),
                    ("(x·y)·(s·s)", |e, s| (e[0] * e[1]) * (s * s)),
                    ("((x·y)·s)·s", |e, s| ((e[0] * e[1]) * s) * s),
                    ("((y·x)·s)·s", |e, s| ((e[1] * e[0]) * s) * s),
                ];
                let ours: f32 = { let mut t = 0f32; for (kk, c) in ch.iter().enumerate() { if ids[kk] >= base { let p = &placed[kk]; t += ((c.ext[1] * s) * c.ext[0]) * s - (p.w as i32 * p.h as i32) as f32; } } t };
                println!("items at s {s} ({:#010x}): ours Σ(a − wh) = {ours}", s.to_bits());
                for (en, ef) in ext_f.iter() { for (an, af) in a_f.iter() {
                    let mut tot = 0f32; let mut detail = String::new();
                    for (kk, _c) in ch.iter().enumerate() {
                        if ids[kk] < base { continue; }
                        let inst = scene.instances.iter().find(|i| i.item as u32 == ids[kk] - base).unwrap();
                        let m = &scene.models[inst.model];
                        let Some(bb) = m.plg_bounds else { continue };
                        let e = ef(&bb, m.plg_u02);
                        let av = af(e, s);
                        let p = &placed[kk];
                        tot += av - (p.w as i32 * p.h as i32) as f32;
                        detail += &format!(" [item {} ext ({}, {}) a {av}]", ids[kk] - base, e[0], e[1]);
                    }
                    let d = tot - ours;
                    println!("  ext {en:<30} a {an:<12}: Σ {tot} (ours {d:+.6}){}{detail}", if (0.198..0.213).contains(&d) { "  ← IN THE WINDOW" } else { "" });
                } }
                return;
            }
            // --items-a: the items' a = ext·ext·s² under the candidate f32 op orders and their w·h — the initial carry the
            // tiles inherit (the carry-offset scan says the editor's differs from ours by −0.0243 ± 0.0001)
            if a.iter().any(|x| x == "--items-a") {
                let k = f32::from_bits(0x3d3504f3);
                let (mbu, b) = (f32::from_bits(0x420dc57e), [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)]);
                let ext_t = tile_ext_of(&b, mbu, k, korder);
                let mut ch = charts.clone();
                for (c, &o) in ch.iter_mut().zip(&ids) { if o < base { c.ext = ext_t; } }
                let Some((s, placed)) = lightmap::pack::allocate_ordered_forced(&ch, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) else { println!("allocation failed"); return };
                let s = f32::from_bits((s.to_bits() as i64 + f("--s-ulps").map(|v| v.parse::<i64>().unwrap()).unwrap_or(0)) as u32);
                let placed = lightmap::pack::try_pack(&ch, &placed_order, s, w_atlas, w_atlas, g, mmin).unwrap_or(placed);
                println!("s {s} ({:#010x})", s.to_bits());
                let orders: [(&str, fn([f32; 2], f32) -> f32); 6] = [
                    ("((y·s)·x)·s", |e, s| ((e[1] * s) * e[0]) * s),
                    ("((x·s)·y)·s", |e, s| ((e[0] * s) * e[1]) * s),
                    ("(x·s)·(y·s)", |e, s| (e[0] * s) * (e[1] * s)),
                    ("(x·y)·(s·s)", |e, s| (e[0] * e[1]) * (s * s)),
                    ("((x·y)·s)·s", |e, s| ((e[0] * e[1]) * s) * s),
                    ("f64 exact", |e, s| ((e[0] as f64) * (e[1] as f64) * (s as f64) * (s as f64)) as f32),
                ];
                let mut totals = vec![0f32; orders.len()];
                for (k, c) in ch.iter().enumerate() {
                    if ids[k] < base { continue; }
                    let p = &placed[k];
                    let wh = (p.w as i32 * p.h as i32) as f32;
                    let mut line = format!("  item {} ext ({}, {}) [{:#010x} {:#010x}] packer {}×{} (w·h {wh}):", ids[k] - base, c.ext[0], c.ext[1], c.ext[0].to_bits(), c.ext[1].to_bits(), p.w, p.h);
                    for (oi, (name, f)) in orders.iter().enumerate() { let av = f(c.ext, s); totals[oi] += av - wh; line += &format!("  {name} a {av} (a−wh {})", av - wh); }
                    println!("{line}");
                }
                for (oi, (name, _)) in orders.iter().enumerate() { println!("  Σ(a − wh) over the items with {name}: {} (vs order 0: {:+})", totals[oi], totals[oi] - totals[0]); }
                // the tile's a under the same orders
                for (name, f) in orders.iter() { println!("  tile a with {name}: {} ({:#010x})", f(ext_t, s), f(ext_t, s).to_bits()); }
                return;
            }
            // --scan-carry LO,HI,N: the initial carry offset (the items' a rounding) that reproduces the most tile rects
            if let Some(r) = f("--scan-carry") {
                let v: Vec<f32> = r.split(',').map(|t| t.parse().unwrap()).collect();
                let n = v[2] as usize;
                let k = f32::from_bits(f("--k-bits").map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()).unwrap_or(0x3d3504f3));
                let (mbu, b) = (f32::from_bits(0x420dc57e), [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)]);
                let ext = tile_ext_of(&b, mbu, k, korder);
                let mut ch = charts.clone();
                for (c, &o) in ch.iter_mut().zip(&ids) { if o < base { c.ext = ext; } }
                let Some((s0, _)) = lightmap::pack::allocate_ordered_forced(&ch, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) else { println!("allocation failed"); return };
                let n_tiles_c = ch.iter().zip(&ids).filter(|(_, &o)| o < base).count();
                let mut best = (0usize, 0usize, 0f32);
                for i in 0..=n {
                    let off = v[0] + (v[1] - v[0]) * i as f32 / n as f32;
                    lightmap::pack::CARRY_OFFSET.store(off.to_bits(), std::sync::atomic::Ordering::Relaxed);
                    let Some(placed) = lightmap::pack::try_pack(&ch, &placed_order, s0, w_atlas, w_atlas, g, mmin) else { continue };
                    let mut first_bad = usize::MAX; let mut kk = 0usize; let mut m = 0usize;
                    for &i in placed_order.iter().rev() {
                        if ids[i] >= base { continue; }
                        let p = &placed[i]; kk += 1;
                        let ok = ed.get(&ids[i]).map(|&(ex, ey, ew, eh)| p.x as u32 + pad == ex as u32 && p.y as u32 + pad == ey as u32 && (p.w as u32).saturating_sub(2 * pad) == ew as u32 && (p.h as u32).saturating_sub(2 * pad) == eh as u32).unwrap_or(false);
                        if ok { m += 1; } else if first_bad == usize::MAX { first_bad = kk; }
                    }
                    let fb = first_bad.min(n_tiles_c + 1);
                    let detail = { let mut kk = 0usize; let mut s = String::new(); for &i in placed_order.iter().rev() { if ids[i] >= base { continue; } kk += 1; if kk == fb { let p = &placed[i]; if let Some(&(ex, ey, ew, eh)) = ed.get(&ids[i]) { s = format!(" (ours ({}, {}) {}×{} vs editor ({ex}, {ey}) {ew}×{eh})", p.x as u32 + pad, p.y as u32 + pad, (p.w as u32).saturating_sub(2 * pad), (p.h as u32).saturating_sub(2 * pad)); } } } s };
                    if (fb, m) > (best.1, best.0) || a.iter().any(|x| x == "--scan-all") { if (fb, m) > (best.1, best.0) { best = (m, fb, off); } println!("  carry offset {off:+.5}: first mismatch at tile #{fb}{detail}, {m} of {n_tiles_c} equal"); }
                }
                lightmap::pack::CARRY_OFFSET.store(0, std::sync::atomic::Ordering::Relaxed);
                println!("best carry offset {:+.5}: first mismatch at #{}, {} equal", best.2, best.1, best.0);
                return;
            }
            // --scan-s-ulps N [--k-bits HEX]: with the tile extent from k (default √2/32 = 0x3d3504f3), TryPack at the replayed
            // s and at s ± 1..N f32 ulps — is the search's last bit the residual?
            if let Some(nu) = f("--scan-s-ulps") {
                let nu: i32 = nu.parse().unwrap();
                let k = f32::from_bits(f("--k-bits").map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()).unwrap_or(0x3d3504f3));
                let (mbu, b) = (f32::from_bits(0x420dc57e), [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)]);
                let ext = tile_ext_of(&b, mbu, k, korder);
                let mut ch = charts.clone();
                for (c, &o) in ch.iter_mut().zip(&ids) { if o < base { c.ext = ext; } }
                let Some((s0, _)) = lightmap::pack::allocate_ordered_forced(&ch, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) else { println!("allocation failed"); return };
                let n_tiles_c = ch.iter().zip(&ids).filter(|(_, &o)| o < base).count();
                for du in -nu..=nu {
                    let s = f32::from_bits((s0.to_bits() as i64 + du as i64) as u32);
                    let Some(placed) = lightmap::pack::try_pack(&ch, &placed_order, s, w_atlas, w_atlas, g, mmin) else { println!("  s {s} ({du:+} ulp): TryPack fails"); continue };
                    let mut first_bad = usize::MAX; let mut kk = 0usize; let mut m = 0usize; let mut detail = String::new();
                    for &i in placed_order.iter().rev() {
                        if ids[i] >= base { continue; }
                        let p = &placed[i]; kk += 1;
                        let ok = ed.get(&ids[i]).map(|&(ex, ey, ew, eh)| p.x as u32 + pad == ex as u32 && p.y as u32 + pad == ey as u32 && (p.w as u32).saturating_sub(2 * pad) == ew as u32 && (p.h as u32).saturating_sub(2 * pad) == eh as u32).unwrap_or(false);
                        if ok { m += 1; } else if first_bad == usize::MAX { first_bad = kk; if let Some(&(ex, ey, ew, eh)) = ed.get(&ids[i]) { detail = format!(" (ours ({}, {}) {}×{} vs editor ({ex}, {ey}) {ew}×{eh})", p.x as u32 + pad, p.y as u32 + pad, (p.w as u32).saturating_sub(2 * pad), (p.h as u32).saturating_sub(2 * pad)); } }
                    }
                    println!("  s {s} ({:#010x}, {du:+} ulp): first mismatch at tile #{}{detail}, {m} of {n_tiles_c} equal", s.to_bits(), first_bad.min(n_tiles_c + 1));
                }
                return;
            }
            // --scan-k LO,HI,N [--k-order 0|1]: the tile extent from the ZONE TILE'S OWN PreLightGen (the Sea prefab's entity 0:
            // MeterByUv 35.442863 = 0x420dc57e, uv bounds 0x3d448f40 0x3d58373f 0x3f7272d8 0x3f759e83 — `mapgeom zone-tile-plg`)
            // times a block scale k: ext = (uvExt × MeterByUv) × k (order 0) or uvExt × (MeterByUv × k) (order 1); the s of the
            // replayed search is fixed (D from --editor-sum), so one TryPack per k; scored by the first mismatching tile rect
            if let Some(r) = f("--scan-k") {
                let v: Vec<f64> = r.split(',').map(|t| t.parse().unwrap()).collect();
                let n = v[2] as usize;
                // --scan-k-bits LO,HI: every f32 between the two hex bit patterns instead
                let bit_range: Option<(u32, u32)> = f("--scan-k-bits").map(|s| { let (a, b) = s.split_once(',').unwrap(); (u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(b.trim_start_matches("0x"), 16).unwrap()) });
                let n = bit_range.map(|(a, b)| (b - a) as usize).unwrap_or(n);
                let order: u32 = korder;
                let (mbu, b) = (f32::from_bits(0x420dc57e), [f32::from_bits(0x3d448f40), f32::from_bits(0x3d58373f), f32::from_bits(0x3f7272d8), f32::from_bits(0x3f759e83)]);
                let uv_ext = [b[2] - b[0], b[3] - b[1]];
                let Some((s_fixed, _)) = lightmap::pack::allocate_ordered_forced(&charts, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) else { println!("allocation failed"); return };
                println!("scan-k: s {s_fixed} ({:#010x}), uvExt ({}, {}), MeterByUv {mbu}", s_fixed.to_bits(), uv_ext[0], uv_ext[1]);
                let n_tiles_c = charts.iter().zip(&ids).filter(|(_, &o)| o < base).count();
                let mut best = (0usize, 0usize, 0f32, [0f32; 2]);
                let mut last_k = f32::NAN;
                for i in 0..=n {
                    let k = match bit_range { Some((a, _)) => f32::from_bits(a + i as u32), None => (v[0] + (v[1] - v[0]) * i as f64 / n as f64) as f32 };
                    if k == last_k { continue; }
                    last_k = k;
                    let ext = tile_ext_of(&b, mbu, k, order);
                    let mut ch = charts.clone();
                    for (c, &o) in ch.iter_mut().zip(&ids) { if o < base { c.ext = ext; } }
                    let Some(placed) = lightmap::pack::try_pack(&ch, &placed_order, s_fixed, w_atlas, w_atlas, g, mmin) else { continue };
                    let mut first_bad = usize::MAX; let mut kk = 0usize; let mut m = 0usize;
                    for &i in placed_order.iter().rev() {
                        if ids[i] >= base { continue; }
                        let p = &placed[i]; kk += 1;
                        let ok = ed.get(&ids[i]).map(|&(ex, ey, ew, eh)| p.x as u32 + pad == ex as u32 && p.y as u32 + pad == ey as u32 && (p.w as u32).saturating_sub(2 * pad) == ew as u32 && (p.h as u32).saturating_sub(2 * pad) == eh as u32).unwrap_or(false);
                        if ok { m += 1; } else if first_bad == usize::MAX { first_bad = kk; }
                    }
                    let fb = first_bad.min(n_tiles_c + 1);
                    if (fb, m) > (best.1, best.0) { best = (m, fb, k, ext); println!("  k {k} ({:#010x}) ext ({}, {}) area {}: first mismatch at tile #{fb}, {m} of {n_tiles_c} rects equal", k.to_bits(), ext[0], ext[1], ext[0] * ext[1]); }
                }
                println!("best k {} ({:#010x}) ext ({}, {}): first mismatch at #{}, {} equal", best.2, best.2.to_bits(), best.3[0], best.3[1], best.1, best.0);
                return;
            }
            // --fit-ext: search the tile extent (ext.x, ext.y) around the given one for the pair that reproduces the most of
            // the editor's tile rects (position + size) in the walk order — the game's exact ext (uv bounds × MeterByUv ×
            // block scale) is not read yet; the carry rounding is sensitive to its last digits
            if a.iter().any(|x| x == "--fit-ext") {
                let n_tiles_c = charts.iter().zip(&ids).filter(|(_, &o)| o < base).count();
                let score = |ext: [f32; 2]| -> (usize, usize) {
                    let mut ch = charts.clone();
                    for (c, &o) in ch.iter_mut().zip(&ids) { if o < base { c.ext = ext; } }
                    let Some((s, placed)) = lightmap::pack::allocate_ordered_forced(&ch, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) else { return (0, 0) };
                    let _ = s;
                    let mut first_bad = usize::MAX; let mut k = 0usize; let mut m = 0usize;
                    for &i in placed_order.iter().rev() {
                        if ids[i] >= base { continue; }
                        let p = &placed[i]; k += 1;
                        let ok = ed.get(&ids[i]).map(|&(ex, ey, ew, eh)| p.x as u32 + pad == ex as u32 && p.y as u32 + pad == ey as u32 && (p.w as u32).saturating_sub(2 * pad) == ew as u32 && (p.h as u32).saturating_sub(2 * pad) == eh as u32).unwrap_or(false);
                        if ok { m += 1; } else if first_bad == usize::MAX { first_bad = k; }
                    }
                    (m, first_bad.min(n_tiles_c + 1))
                };
                let base_ext = tile_ext_xy;
                let (sx, sy, nx, ny): (f32, f32, i32, i32) = f("--fit-ext-grid").map(|v| { let t: Vec<f32> = v.split(',').map(|x| x.parse().unwrap()).collect(); (t[0], t[1], t[2] as i32, t[3] as i32) }).unwrap_or((2e-5, 2e-5, 25, 25));
                let mut best = (0usize, 0usize, base_ext);
                let t0 = std::time::Instant::now();
                for ix in -nx..=nx { for iy in -ny..=ny {
                    let ext = [base_ext[0] + ix as f32 * sx, base_ext[1] + iy as f32 * sy];
                    let (m, fb) = score(ext);
                    if (fb, m) > (best.1, best.0) { best = (m, fb, ext); println!("  ext ({:.7}, {:.7}) area {:.8}: first mismatch at tile #{fb}, {m} of {n_tiles_c} rects equal", ext[0], ext[1], ext[0] * ext[1]); }
                } }
                println!("best ext ({:.7}, {:.7}) area {:.8}: first mismatch at #{}, {} equal ({:.1}s)", best.2[0], best.2[1], best.2[0] * best.2[1], best.1, best.0, t0.elapsed().as_secs_f32());
                return;
            }
            let Some((s, placed)) = (match forced_s { Some(s) => lightmap::pack::try_pack(&charts, &placed_order, s, w_atlas, w_atlas, g, mmin).map(|p| (s, p)), None => lightmap::pack::allocate_ordered_forced(&charts, &placed_order, w_atlas, w_atlas, g, mmin, max_iter, &fail_iters) }) else { println!("allocation failed"); return };
            {
                // our tile chart sizes (layout units) against the editor's histogram
                let mut hist: std::collections::BTreeMap<(u32, u32), usize> = Default::default();
                for (k, p) in placed.iter().enumerate() { if ids[k] < base { let sz = if pad > 0 { ((p.w as u32).saturating_sub(2 * pad), (p.h as u32).saturating_sub(2 * pad)) } else { (2 * (p.w as u32).saturating_sub(1), 2 * (p.h as u32).saturating_sub(1)) }; *hist.entry(sz).or_default() += 1; } }
                println!("our tile chart sizes at s {s:.4}: {:?}", hist);
            }
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
            if a.iter().any(|x| x == "--show-order") {
                // our first placed tiles in walk order (largest area first = the end of placed_order) with their atlas positions
                println!("our walk (from the end of the order): the first 24 tiles");
                let mut shown = 0;
                let take: usize = f("--take").map(|s| s.parse().unwrap()).unwrap_or(24);
                for &k in placed_order.iter().rev() { if ids[k] >= base { continue; } let p = &placed[k]; let o = ids[k]; let (cx, cz) = cell_of.get(o as usize).copied().unwrap_or((-1, -1)); let ed_r = ed.get(&o).map(|&(x, y, w, h)| format!("editor ({x:>4}, {y:>4}) {w}×{h}")).unwrap_or_default(); println!("  tile obj {o:>5} (cell x{cx:>2} z{cz:>2}) ours ({:>4}, {:>4}) {}×{}  {ed_r}", p.x as u32 + pad, p.y as u32 + pad, (p.w as u32).saturating_sub(2 * pad), (p.h as u32).saturating_sub(2 * pad)); shown += 1; if shown >= take { break; } }
            }
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
        "dome-order" => {
            // lmtool dome-order [--quality Q] [--sweep S] [--points FILE] [--fold]: the game's ISSUE ORDER of a dome
            // sweep (RE child 5: SPlugGroupOfPointInSphere) — one line per issue index: set index, raster sub-sample
            // (ix, iy) and its rotated-grid offset in ninths of a texel, the direction
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let q: u32 = f("--quality").map(|s| s.parse().unwrap()).unwrap_or(3);
            let sweep: usize = f("--sweep").map(|s| s.parse().unwrap()).unwrap_or(0);
            let ps = lightmap::dome::PointSets::load(&f("--points").unwrap_or_else(lightmap::dome::default_path)).expect("point sets");
            let n = *lightmap::dome::sweep_counts(q).get(sweep).expect("the quality has no such sweep");
            let mut list = lightmap::dome::rotate_set(ps.nearest(n).expect("set"));
            if a.iter().any(|x| x == "--fold") { lightmap::dome::fold_down(&mut list); }
            let ss = lightmap::dome::supersample(q);
            let order = lightmap::dome::issue_order(&list, ss);
            println!("quality {q} sweep {sweep}: {} directions, {ss}² = {} interleaved groups; issue → set index, sub-sample (ix, iy), offset/9 texel, direction", list.len(), ss * ss);
            for (i, &o) in order.iter().enumerate() {
                let (ix, iy) = lightmap::dome::raster_subsample(i, ss);
                let off = lightmap::dome::raster_offset_rotated(ix, iy, ss, ss);
                let d = list[o as usize];
                println!("{i:4} {o:4}  ({ix}, {iy}) ({:+.0}, {:+.0})  {:+.6} {:+.6} {:+.6}", off[0] * 9.0, off[1] * 9.0, d[0], d[1], d[2]);
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
        "chartrule" => {
            // lmtool chartrule EDITOR.Map.Gbx [--base N]: per model, the editor's chart size next to the model's
            // PreLightGen u02, uv1 extent, world bbox and metres-per-uv — to read the game's chart-size rule
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let own = lightmap::mapio::load(&a[1]).expect("load");
            let d = own.chunk.data.clone().expect("lightmap");
            let mp = d.cache.mapping().unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            // the per-item lightmap quality byte (chunk 0x03043068: Normal 0, High 1, VeryHigh 2, Highest 3,
            // Lowest 4, VeryLow 5, Low 6), after the blocks' and baked blocks' bytes
            let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
            let qual: Vec<u8> = tmmaps::gbx::all_skip_chunks(&mf.gbx.body).iter().find(|(c, ..)| *c == 0x0304_3068).map(|&(_, _, payload, size)| {
                let start = payload + 4 + mf.blocks.len() + mf.baked.len();
                mf.gbx.body[start..(payload + size).min(start + mf.items.len())].to_vec()
            }).unwrap_or_default();
            let per_item = a.iter().any(|x| x == "--per-item");
            let mut seen: std::collections::BTreeSet<usize> = Default::default();
            println!("model\tu02\tew\teh\tm_per_uv\tbbox_x\tbbox_y\tbbox_z\ted_w\ted_h\tu02rule_w\tu02rule_h\tquality\titem");
            for inst in &scene.instances {
                if !per_item && !seen.insert(inst.model) { continue; }
                let Some(&ci) = chart_of.get(&(inst.item as u32)) else { continue };
                let (w, h) = mp.size[ci];
                let g = &scene.models[inst.model];
                let (ew, eh) = match g.plg_bounds { Some(b) => (b[2] - b[0], b[3] - b[1]), None => (1.0, 1.0) };
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for t in &g.tris { for p in t.p { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } } }
                let rw = g.plg_u02 * 0.5625 * ew / 2.0;
                let rh = g.plg_u02 * 0.5625 * eh / 2.0;
                println!("{}\t{:.2}\t{:.3}\t{:.3}\t{:.2}\t{:.1}\t{:.1}\t{:.1}\t{}\t{}\t{:.1}\t{:.1}\t{}\t{}", inst.model_name, g.plg_u02, ew, eh, g.metres_per_uv, hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], w / 2, h / 2, rw, rh, qual.get(inst.item).copied().unwrap_or(255), inst.item);
            }
        }
        "chartimg" => {
            // lmtool chartimg MAP.Map.Gbx ITEM... --out X.png [--base N] [--scale S]: the frame-0 colour texels of the
            // items' charts (sqrt-decoded, × frame MaxHDR, × S, clipped) side by side as an 8-bit PNG, each chart
            // magnified 4× — to LOOK at the structure the numbers hide
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let scale: f32 = f("--scale").map(|s| s.parse().unwrap()).unwrap_or(1.0);
            let out = f("--out").expect("--out");
            let items: Vec<u32> = a[2..].iter().take_while(|x| !x.starts_with("--")).map(|s| s.parse().unwrap()).collect();
            let own = lightmap::mapio::load(&a[1]).expect("load");
            let d = own.chunk.data.clone().expect("lightmap");
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("atlas");
            let fm = d.cache.frame_max_hdr().unwrap_or(1.0);
            let mp = d.cache.mapping().unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let mag = 4usize;
            let scene_opt = if a.iter().any(|x| x == "--faces") { lightmap::geometry::Scene::from_map(&a[1]).ok() } else { None };
            let mut tiles: Vec<(usize, usize, Vec<u8>)> = Vec::new();
            for it in &items {
                let Some(&i) = chart_of.get(it) else { eprintln!("item {it}: no chart"); continue };
                let (x, y) = mp.pos[i]; let (w, h) = mp.size[i];
                let (px, py, pw, ph) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2, (w as u32 / 2).max(1), (h as u32 / 2).max(1));
                let fb = mp.frame_bytes[0][i];
                let mut rgb = vec![0u8; pw as usize * ph as usize * 3];
                for ty in 0..ph { for tx in 0..pw {
                    let c = ia.get((px + tx).min(ia.w - 1), (py + ty).min(ia.h - 1));
                    for k in 0..3 { let v = lightmap::synth::decode_value(c[k], fb) * fm * scale; rgb[((ty * pw + tx) * 3 + k as u32) as usize] = (v.clamp(0.0, 1.0) * 255.0) as u8; }
                } }
                eprintln!("item {it}: chart {i} rect ({px},{py}) {pw}×{ph} fb {fb} MaxHDR {fm:.3}");
                // per-face means (texels grouped by the surface normal the raster gives them)
                if let Some(ii) = scene_opt.as_ref().and_then(|s| s.instances.iter().position(|q| q.item as u32 == *it)) {
                    let scene = scene_opt.as_ref().unwrap();
                    let (samples, _) = lightmap::bake::rasterise_pub(scene, ii, pw, ph, false, true);
                    let mut acc: std::collections::BTreeMap<&str, (f64, usize, [f64; 3])> = Default::default();
                    for s in &samples {
                        let cls = if s.n[1] > 0.7 { "up" } else if s.n[1] < -0.7 { "down" } else if s.n[0] > 0.7 { "+x" } else if s.n[0] < -0.7 { "-x" } else if s.n[2] > 0.7 { "+z" } else if s.n[2] < -0.7 { "-z" } else { "slanted" };
                        let c = ia.get((px + s.px).min(ia.w - 1), (py + s.py).min(ia.h - 1));
                        let rgb = [lightmap::synth::decode_value(c[0], fb) * fm, lightmap::synth::decode_value(c[1], fb) * fm, lightmap::synth::decode_value(c[2], fb) * fm];
                        let l = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
                        let e = acc.entry(cls).or_insert((0.0, 0, [0.0; 3])); e.0 += l as f64; e.1 += 1; for k in 0..3 { e.2[k] += rgb[k] as f64; }
                    }
                    eprintln!("   faces: {}", acc.iter().map(|(k, (s, n, c))| format!("{k} {:.3} ({:.2},{:.2},{:.2}) ({n})", s / *n as f64, c[0] / *n as f64, c[1] / *n as f64, c[2] / *n as f64)).collect::<Vec<_>>().join("  "));
                }
                tiles.push((pw as usize, ph as usize, rgb));
            }
            let tw: usize = tiles.iter().map(|t| t.0 * mag + 4).sum();
            let th: usize = tiles.iter().map(|t| t.1 * mag).max().unwrap_or(1);
            let mut img = mapgeom::render::Image { w: tw.max(1), h: th.max(1), rgb: vec![40u8; tw.max(1) * th.max(1) * 3] };
            let mut ox = 0usize;
            for (pw, ph, rgb) in &tiles {
                for y in 0..ph * mag { for x in 0..pw * mag { for k in 0..3 { img.rgb[((y * img.w) + ox + x) * 3 + k] = rgb[((y / mag) * pw + x / mag) * 3 + k]; } } }
                ox += pw * mag + 4;
            }
            std::fs::write(&out, mapgeom::render::png(&img)).expect("write png");
            println!("wrote {out} ({}×{})", img.w, img.h);
        }
        "coverage" => {
            // lmtool coverage EDITOR.Map.Gbx [--base N] [--items N] [--ss 3]: the chart raster's coverage against the
            // editor's — per item chart, raster the object's TexCoord1 geometry into the chart's layout rect at ss
            // sub-samples per axis; a colour texel (the atlas at layout/2) counts as OURS when any of its 2×2 layout
            // texels has a sub-sample, as the EDITOR's when its stored value is non-black. Reports: our texels the
            // editor also wrote (must → 100 %), the editor's texels we do not cover (its gutter dilation + our misses).
            let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
            let ss: u32 = f("--ss").map(|s| s.parse().unwrap()).unwrap_or(3);
            let own = lightmap::mapio::load(&a[1]).expect("load");
            let d = own.chunk.data.clone().expect("lightmap");
            let ia = lightmap::img::decode_webp(&d.frames[0].images[0]).expect("atlas");
            let mp = d.cache.mapping().unwrap();
            let mut chart_of: std::collections::HashMap<u32, usize> = Default::default();
            for i in 0..mp.count as usize { let obj = mp.binds[i].obj_group_idx / 4; if obj >= base { chart_of.insert(obj - base, i); } }
            let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
            let step = (scene.instances.len() / f("--items").map(|s| s.parse().unwrap()).unwrap_or(2000)).max(1);
            let (mut ours, mut ours_in_ed, mut ed, mut ed_not_ours, mut charts) = (0usize, 0usize, 0usize, 0usize, 0usize);
            let mut ring: [usize; 2] = [0, 0]; // editor-only texels adjacent to ours (gutter) vs isolated
            for (ii, inst) in scene.instances.iter().enumerate().step_by(step) {
                let Some(&i) = chart_of.get(&(inst.item as u32)) else { continue };
                let (x, y) = mp.pos[i]; let (w, h) = mp.size[i];
                let (w, h) = (w as u32, h as u32);
                if w < 4 || h < 4 || mp.frame_bytes[0][i] == 0 { continue; }
                let r = lightmap::chartraster::raster_chart(&scene, ii, w, h, ss, false, true);
                let (cw, ch) = ((w / 2).max(1), (h / 2).max(1));
                let (px0, py0) = ((x as u32 + 1) / 2, (y as u32 + 1) / 2);
                let mut ours_mask = vec![false; (cw * ch) as usize];
                for ty in 0..h { for tx in 0..w { if r.count[(ty * w + tx) as usize] > 0 { let (cx, cy) = ((tx / 2).min(cw - 1), (ty / 2).min(ch - 1)); ours_mask[(cy * cw + cx) as usize] = true; } } }
                let ed_mask: Vec<bool> = (0..ch).flat_map(|cy| (0..cw).map(move |cx| (cx, cy))).map(|(cx, cy)| { let c = ia.get((px0 + cx).min(ia.w - 1), (py0 + cy).min(ia.h - 1)); c[0] > 2 || c[1] > 2 || c[2] > 2 }).collect();
                for cy in 0..ch { for cx in 0..cw {
                    let k = (cy * cw + cx) as usize;
                    if ours_mask[k] { ours += 1; if ed_mask[k] { ours_in_ed += 1; } }
                    if ed_mask[k] { ed += 1; if !ours_mask[k] { ed_not_ours += 1;
                        let near = (-1i32..=1).any(|dy| (-1i32..=1).any(|dx| { let (nx, ny) = (cx as i32 + dx, cy as i32 + dy); nx >= 0 && ny >= 0 && (nx as u32) < cw && (ny as u32) < ch && ours_mask[(ny as u32 * cw + nx as u32) as usize] }));
                        if near { ring[0] += 1; } else { ring[1] += 1; } } }
                } }
                charts += 1;
            }
            println!("{charts} charts at ss {ss}: ours {ours} colour texels, {:.2} % of them written by the editor; editor {ed} texels, {ed_not_ours} not covered by us ({:.2} %): {} adjacent to ours (gutter), {} isolated (our misses)", 100.0 * ours_in_ed as f64 / ours.max(1) as f64, 100.0 * ed_not_ours as f64 / ed.max(1) as f64, ring[0], ring[1]);
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
            let per_item = a.iter().any(|x| x == "--per-item");
            let mut per_rows: Vec<(usize, String, usize, f64, f64, f64)> = Vec::new(); // item, model, n, ref mean, ours mean, rmse
            let mut per_rgb: Vec<(usize, [f64; 3], [f64; 3])> = Vec::new();
            let mut flat_skipped = 0usize;
            let mut classes: Vec<u8> = Vec::new();
            let mut rows_rgb: Vec<([f32; 3], [f32; 3])> = Vec::new();
            let sun_az_ref: Option<f32> = f("--sun-az-ref").map(|s| s.parse().unwrap());
            let mut class_mats: std::collections::BTreeMap<(u8, String), (usize, f64, f64)> = Default::default();
            for (ii, inst) in scene.instances.iter().enumerate().step_by(step) {
                let row0 = rows.len();
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
                    rows_rgb.push(([lightmap::synth::decode_value(c[0], fba) * fma, lightmap::synth::decode_value(c[1], fba) * fma, lightmap::synth::decode_value(c[2], fba) * fma], [lightmap::synth::decode_value(c2[0], fbb) * fmb, lightmap::synth::decode_value(c2[1], fbb) * fmb, lightmap::synth::decode_value(c2[2], fbb) * fmb]));
                    // the gate class: vegetation (AV items and alpha-tested cards), else small chart (< 64 texels),
                    // else by the normal: floor (up), underside (down), wall (vertical), slanted
                    // slanted and vertical faces split by whether they face the sun's azimuth (--sun-az-ref D, my
                    // frame: atan2(x, z)) — classes 6/7 = slanted/walls turned AWAY from it
                    let facing_sun = match sun_az_ref { Some(az) => { let (sx, sz) = (az.to_radians().sin(), az.to_radians().cos()); s.n[0] * sx + s.n[2] * sz > 0.0 } None => true };
                    let cls: u8 = if inst.model_name.starts_with("AV") || s.cut { 4 } else if (pwa * pha) < 64 { 5 } else if s.n[1] > 0.7 { 0 } else if s.n[1] < -0.7 { 1 } else if s.n[1].abs() < 0.3 { if facing_sun { 2 } else { 7 } } else if facing_sun { 3 } else { 6 };
                    classes.push(cls);
                    if cls == 3 || cls == 6 || cls == 2 || cls == 7 {
                        let mname = scene.models[inst.model].mat_links.get(s.mat as usize).map(|l| l.rsplit('\\').next().unwrap_or(l).to_string()).unwrap_or_else(|| "-".into());
                        let e = class_mats.entry((cls, format!("{} {}", inst.model_name, mname))).or_insert((0usize, 0.0f64, 0.0f64));
                        e.0 += 1; e.1 += la as f64; e.2 += lb as f64;
                    }
                    if per_item { let e = if per_rgb.last().map(|r| r.0) == Some(inst.item) { per_rgb.last_mut().unwrap() } else { per_rgb.push((inst.item, [0.0; 3], [0.0; 3])); per_rgb.last_mut().unwrap() }; for k in 0..3 { e.1[k] += (lightmap::synth::decode_value(c[k], fba) * fma) as f64; e.2[k] += (lightmap::synth::decode_value(c2[k], fbb) * fmb) as f64; } }
                }
                // an UNWRITTEN reference chart (every texel the same value: the atlas background, e.g. the
                // charts of items a `--reduced` bake dropped) is no reference — dropped from every statistic
                if rows.len() > row0 && !a.iter().any(|x| x == "--keep-flat") {
                    let r = &rows[row0..];
                    let (mn, mx) = r.iter().fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x.0), hi.max(x.0)));
                    if r.len() >= 4 && mx - mn < 1e-6 {
                        rows.truncate(row0);
                        classes.truncate(row0);
                        rows_rgb.truncate(row0);
                        if per_item { if per_rgb.last().map(|q| q.0) == Some(inst.item) { per_rgb.pop(); } }
                        flat_skipped += 1;
                        continue;
                    }
                }
                if per_item && rows.len() > row0 {
                    let r = &rows[row0..];
                    let n = r.len() as f64;
                    let ma = r.iter().map(|x| x.0).sum::<f64>() / n;
                    let mb = r.iter().map(|x| x.1).sum::<f64>() / n;
                    let rm = (r.iter().map(|x| (x.0 - x.1) * (x.0 - x.1)).sum::<f64>() / n).sqrt();
                    per_rows.push((inst.item, inst.model_name.clone(), r.len(), ma, mb, rm));
                }
            }
            if flat_skipped > 0 { println!("{flat_skipped} items skipped: their reference chart is unwritten (one flat value)"); }
            if a.iter().any(|x| x == "--class-mats") {
                for cls in [3u8, 6, 2, 7] {
                    let mut rows: Vec<(&(u8, String), &(usize, f64, f64))> = class_mats.iter().filter(|(k, _)| k.0 == cls).collect();
                    rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
                    println!("class {cls} top model+material by texels:");
                    for (k, (n, sa, sb)) in rows.iter().take(6) { println!("   {:<60} {n:>7} ref {:.3} ours {:.3}", k.1, sa / *n as f64, sb / *n as f64); }
                }
            }
            // the per-class gate table
            {
                let names = ["floors (up)", "undersides (down)", "walls (vertical, sun side)", "slanted (sun side)", "vegetation (AV items + cards)", "small charts (< 64 texels)", "slanted (away from the sun)", "walls (away from the sun)"];
                println!("per class: texels ref-mean ours-mean ratio rmse(% of ref mean)");
                for c in 0..names.len() {
                    let sel: Vec<&(f64, f64, bool)> = rows.iter().zip(classes.iter()).filter(|(_, k)| **k as usize == c).map(|(r, _)| r).collect();
                    if sel.is_empty() { continue; }
                    let n = sel.len() as f64;
                    let (ma, mb) = (sel.iter().map(|r| r.0).sum::<f64>() / n, sel.iter().map(|r| r.1).sum::<f64>() / n);
                    let rmse = (sel.iter().map(|r| (r.0 - r.1) * (r.0 - r.1)).sum::<f64>() / n).sqrt();
                    let mut ra = [0f64; 3]; let mut rb = [0f64; 3];
                    for ((_, k), (pa, pb)) in rows.iter().zip(classes.iter()).zip(rows_rgb.iter()) { if *k as usize == c { for j in 0..3 { ra[j] += pa[j] as f64; rb[j] += pb[j] as f64; } } }
                    println!("  {:<32} {:>8} {ma:.3} {mb:.3} {:.3} {:.1} %   ref rgb ({:.2},{:.2},{:.2}) ours ({:.2},{:.2},{:.2})", names[c], sel.len(), mb / ma.max(1e-9), 100.0 * rmse / ma.max(1e-9), ra[0] / n, ra[1] / n, ra[2] / n, rb[0] / n, rb[1] / n, rb[2] / n);
                }
            }
            if per_item {
                per_rows.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap());
                println!("per item (worst RMSE first): item model texels ref-mean ours-mean ratio rmse");
                let take = if a.iter().any(|x| x == "--per-item-all") { usize::MAX } else { 40 };
                for (item, model, n, ma, mb, rm) in per_rows.iter().take(take) {
                    let rgb = per_rgb.iter().find(|r| r.0 == *item).map(|r| format!("  ref rgb ({:.2},{:.2},{:.2}) ours ({:.2},{:.2},{:.2})", r.1[0] / *n as f64, r.1[1] / *n as f64, r.1[2] / *n as f64, r.2[0] / *n as f64, r.2[1] / *n as f64, r.2[2] / *n as f64)).unwrap_or_default();
                    println!("  {item:>5} {model:<28} {n:>7} {ma:.3} {mb:.3} {:.3} {rm:.3}{rgb}", mb / ma.max(1e-6));
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
