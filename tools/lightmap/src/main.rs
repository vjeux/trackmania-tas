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
