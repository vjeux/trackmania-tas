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
            let scene = lightmap::geometry::Scene::from_map(&map_path).expect("scene");
            eprintln!("scene: {} models, {} instances, {} triangles ({:.1}s)", scene.models.len(), scene.instances.len(), scene.tri_count(), t0.elapsed().as_secs_f32());
            let tris = lightmap::bake::world_tris(&scene);
            let bvh = lightmap::bvh::Bvh::build(tris);
            eprintln!("bvh: {} nodes ({:.1}s)", bvh.node_count(), t0.elapsed().as_secs_f32());
            let parse_rgb = |s: &str| -> [f32; 3] { let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect(); [v[0], v[1], v[2]] };
            let mut prm = lightmap::bake::BakeParams::default();
            if let Some(s) = f("--sky") { prm.sky = parse_rgb(&s); }
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
            prm.uv_bounds = has("--uv-bounds");
            prm.pattern = has("--pattern");
            let sun_dir = |az: f32, el: f32| -> [f32; 3] { let (a, e) = (az.to_radians(), el.to_radians()); [e.cos() * a.sin(), e.sin(), e.cos() * a.cos()] };
            let base: u32 = f("--base").map(|s| s.parse().unwrap()).unwrap_or(4096);
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
            let az: f32 = f("--sun-az").map(|s| s.parse().unwrap()).unwrap_or(0.0);
            let el: f32 = f("--sun-el").map(|s| s.parse().unwrap()).unwrap_or(45.0);
            prm.sun_dir = sun_dir(az, el);
            let lights = if a.iter().any(|x| x == "--no-lights") { Vec::new() } else { scene.world_lights() };
            if let Some(s) = f("--light-k") { prm.light_k = s.parse().unwrap(); }
            eprintln!("{} point lights", lights.len());
            let charts = lightmap::bake::bake(&scene, &bvh, &prm, &lights);
            eprintln!("baked {} charts ({:.1}s)", charts.len(), t0.elapsed().as_secs_f32());
            compare(&charts, "bake vs own");
            let k: f32 = match f("--k") { Some(s) => s.parse().unwrap(), None => { let k = f32::from_bits(IMPLIED_K.load(std::sync::atomic::Ordering::Relaxed)); if k > 0.0 { eprintln!("K matched to the map's own bake: {k:.3}"); k } else { 3.0 } } };
            let Some(tpl_path) = f("--template") else { return };
            let tpl = lightmap::mapio::load(&tpl_path).expect("template");
            let m = lightmap::mapio::load(&map_path).expect("map");
            let ground_e: [f32; 3] = { let l = prm.sun_dir[1].max(0.0); [prm.ambient[0] + prm.up[0] + prm.sky[0] + prm.sun[0] * l, prm.ambient[1] + prm.up[1] + prm.sky[1] + prm.sun[1] * l, prm.ambient[2] + prm.up[2] + prm.sky[2] + prm.sun[2] * l] };
            let mut out_charts = Vec::new();
            for obj in 0..base { out_charts.push(lightmap::synth::Chart::from_hdr(obj, 2, 2, &[ground_e; 4], k, 128)); }
            let mut have = vec![false; scene.item_count];
            for c in &charts { have[c.item] = true; out_charts.push(lightmap::synth::Chart::from_hdr2(base + c.item as u32, c.w, c.h, &c.rgb, &c.rgb1, k, 128)); }
            for (i, h) in have.iter().enumerate() { if !h { out_charts.push(lightmap::synth::Chart::from_hdr(base + i as u32, 2, 2, &[prm.sky; 4], k, 128)); } }
            let tm = tpl.chunk.data.as_ref().unwrap().cache.mapping().unwrap();
            // the probe volume: ours unless --template-probes
            let vp8_q: Option<u8> = f("--vp8").map(|s| s.parse().unwrap());
            let probes = if has("--template-probes") { None } else {
                let tv = lightmap::volume::Volume::parse(&tpl.chunk.data.as_ref().unwrap().cache.trailer).expect("template trailer");
                let mut pp = prm.clone();
                pp.sky_samples = f("--probe-samples").map(|s| s.parse().unwrap()).unwrap_or(48);
                let po = lightmap::probes::build(&scene, &bvh, &pp, &lights, prm.light_k, &tv, vp8_q.unwrap_or(8)).expect("probes");
                eprintln!("probe volume: {} blocks, {} slices, atlas {}x{}, blob {} B ({:.1}s)", po.blocks, po.slices, po.atlas_w, po.atlas_h, po.blob.len(), t0.elapsed().as_secs_f32());
                Some(lightmap::synth::ProbeBlob { blob: po.blob, trailer: po.volume.write() })
            };
            let s = lightmap::synth::build_full(out_charts, (tm.bbox_min, tm.bbox_max), &tpl.chunk, probes, vp8_q).expect("build");
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
                    let lum = (0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64) / 255.0 * fb1 as f64 / 255.0;
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
