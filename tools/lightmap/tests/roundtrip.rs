//! Byte-identical round trip of chunk 0x0304305B over the store.
//!
//! The store is the tiny-campaign material on private-30d; the test reads
//! `LIGHTMAP_TEST_MAPS` (a `:`-separated list of directories, default the
//! Summer 2026 sources + the editor-baked tiny maps) and is a no-op when
//! none of them is mounted, so `cargo test` stays green on a bare box.

use lightmap::format::LightmapChunk;

fn default_dirs() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let base = format!("{home}/persistent/private-30d/tm-player/tiny");
    vec![
        format!("{base}/sources"),
        format!("{base}/incoming/fix16-lightmap-20260921"),
        format!("{base}/incoming/lightmap-wip-20260912/full"),
        format!("{base}/incoming/lightmap-wip-20260912/var"),
    ]
}

#[test]
fn store_round_trips_byte_identically() {
    let dirs: Vec<String> = match std::env::var("LIGHTMAP_TEST_MAPS") {
        Ok(v) => v.split(':').map(String::from).collect(),
        Err(_) => default_dirs(),
    };
    let mut maps: Vec<std::path::PathBuf> = Vec::new();
    for d in &dirs {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.to_string_lossy().ends_with(".Map.Gbx") {
                maps.push(p);
            }
        }
    }
    if maps.is_empty() {
        eprintln!("no maps found under {dirs:?}: nothing to test");
        return;
    }
    maps.sort();
    let mut tested = 0;
    for p in &maps {
        let data = std::fs::read(p).unwrap();
        let g = gbx::Gbx::parse(&data);
        let Some((_, payload, size)) = lightmap::find_chunk(&g.body) else { continue };
        let chunk = &g.body[payload..payload + size];
        let lm = LightmapChunk::parse(chunk).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(lm.write(false), chunk, "{}: stored-stream round trip", p.display());
        if let Some(d) = &lm.data {
            let raw = lightmap::zlib_inflate(&d.cache_compressed, d.cache_uncompressed_len as usize).unwrap();
            assert_eq!(d.cache.write(), raw, "{}: cache rebuilt from parsed parts", p.display());
            // a recompressed write must parse back to the same structure
            let re = LightmapChunk::parse(&lm.write(true)).unwrap_or_else(|e| panic!("{}: reparse {e}", p.display()));
            assert_eq!(re.data.as_ref().unwrap().cache, d.cache, "{}: recompressed cache differs", p.display());
            let m = d.cache.mapping().expect("mapping chunk");
            assert_eq!(m.binds.len(), m.count as usize);
            assert!(m.frame_bytes.iter().all(|f| f.len() == m.count as usize));
            // the probe volume: parsed and written back byte for byte, its four WEBPs split at the
            // trailer's offsets, every stored tile inside the atlas
            let v = lightmap::volume::Volume::parse(&d.cache.trailer).unwrap_or_else(|e| panic!("{}: trailer {e}", p.display()));
            assert_eq!(v.write(), d.cache.trailer, "{}: trailer round trip", p.display());
            let parts = lightmap::volume::split_probe_blob(&d.frames[0].images[2], &v.frame_info);
            assert_eq!(parts.len(), 4, "{}: probe blob images", p.display());
            let mut dims = None;
            for part in &parts {
                assert_eq!(&part[..4], b"RIFF", "{}: probe image is a WEBP", p.display());
                let im = lightmap::img::decode_webp(part).unwrap_or_else(|e| panic!("{}: probe image {e}", p.display()));
                assert!(dims.map_or(true, |d| d == (im.w, im.h)), "{}: probe images share a size", p.display());
                dims = Some((im.w, im.h));
            }
            let (aw, ah) = dims.unwrap();
            assert_eq!(v.cell4.len() as u32, ((aw + 3) / 4) * ((ah + 3) / 4), "{}: mask table size", p.display());
            for b in &v.blocks {
                let (tw, th) = (b.max[0] - b.min[0], b.max[2] - b.min[2]);
                for s in b.slices.iter().flatten() {
                    assert!(s.0 + tw <= aw && s.1 + th <= ah, "{}: tile inside the atlas", p.display());
                }
            }
        }
        tested += 1;
    }
    eprintln!("{tested} maps round-tripped");
    assert!(tested >= 1);
}
