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
        _ => {
            eprintln!("unknown command");
            std::process::exit(2);
        }
    }
}
