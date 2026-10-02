//! `lmtool partcmp A.Map.Gbx B.Map.Gbx` / `lmtool partcmp --banks W/OLD W/NEW [--tsv OUT]` — the lightmap chunk compared PART BY PART
//! (V8, 2026-10-02; the v12 sweep): which bytes of two lit files differ. A new base that only fixes the record/cache WORDS must leave
//! every image byte-identical — this prints, per part, the md5 and EQUAL/DIFF: every frame's images (the colour atlas, the directional
//! planes, the probe blob), the mapping TABLE (count, pos, size, binds, frame bytes), the mapping HEAD (the 60 constants, each 66-byte
//! frame record with its kind word, the tail), the probe trailer, and every other cache chunk by id (0x06022013 FILETIME, 0x17/0x18 …).
//! `--banks` walks two bank dirs and prints one line per cell present in both: `cell  images  table  head  records  cache  verdict`.

use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct PartReport {
    pub lines: Vec<(String, String, String, bool)>, // part, md5 a, md5 b, equal
    pub images_equal: bool,
    pub table_equal: bool,
    pub head_equal: bool,
    pub records_a: Vec<u32>,
    pub records_b: Vec<u32>,
    pub cache_equal: bool,
    pub frames_a: usize,
    pub frames_b: usize,
}

fn m(b: &[u8]) -> String { crate::corpusgate::md5_hex(b)[..12].to_string() }

pub fn compare(a: &crate::mapio::MapLightmap, b: &crate::mapio::MapLightmap) -> Result<PartReport, String> {
    let (da, db) = (a.chunk.data.as_ref().ok_or("A: no lightmap chunk")?, b.chunk.data.as_ref().ok_or("B: no lightmap chunk")?);
    let mut r = PartReport { images_equal: true, table_equal: true, head_equal: true, cache_equal: true, frames_a: da.frames.len(), frames_b: db.frames.len(), ..Default::default() };
    let mut push = |r: &mut PartReport, part: String, x: &[u8], y: &[u8]| -> bool { let eq = x == y; r.lines.push((part, m(x), m(y), eq)); eq };
    for i in 0..da.frames.len().max(db.frames.len()) {
        let empty: Vec<Vec<u8>> = Vec::new();
        let (fa, fb) = (da.frames.get(i).map(|f| &f.images).unwrap_or(&empty), db.frames.get(i).map(|f| &f.images).unwrap_or(&empty));
        for j in 0..fa.len().max(fb.len()) {
            let (x, y) = (fa.get(j).map(|v| v.as_slice()).unwrap_or(&[]), fb.get(j).map(|v| v.as_slice()).unwrap_or(&[]));
            let name = match (i, j) { (0, 0) => "frame 0 image 0 (colour atlas)".to_string(), (0, 1) => "frame 0 image 1 (directional planes)".to_string(), (0, 2) => "frame 0 image 2 (probe blob)".to_string(), (i, j) => format!("frame {i} image {j}") };
            if !push(&mut r, name, x, y) { r.images_equal = false; }
        }
    }
    let (ma, mb) = (da.cache.mapping().ok_or("A: no mapping chunk")?, db.cache.mapping().ok_or("B: no mapping chunk")?);
    // the table: count + the four per-chart tables + the frame bytes
    let table = |mp: &crate::format::Mapping| -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&mp.count.to_le_bytes());
        v.extend_from_slice(&mp.atlas_w.to_le_bytes()); v.extend_from_slice(&mp.atlas_h.to_le_bytes());
        for f in &mp.chart_f32 { v.extend_from_slice(&f.to_le_bytes()); }
        for b in &mp.binds { v.extend_from_slice(&b.obj_idx.to_le_bytes()); v.extend_from_slice(&b.obj_group_idx.to_le_bytes()); }
        for p in &mp.pos { v.extend_from_slice(&p.0.to_le_bytes()); v.extend_from_slice(&p.1.to_le_bytes()); }
        for s in &mp.size { v.extend_from_slice(&s.0.to_le_bytes()); v.extend_from_slice(&s.1.to_le_bytes()); }
        v
    };
    if !push(&mut r, format!("mapping rects: binds + pos + size ({} / {} charts)", ma.count, mb.count), &table(ma), &table(mb)) { r.table_equal = false; }
    // the per-chart frame bytes are LIGHT data (each chart's max): a light change moves them with the images, a word fix must not
    let fbytes = |mp: &crate::format::Mapping| -> Vec<u8> { mp.frame_bytes.iter().flat_map(|f| f.iter().copied()).collect() };
    if !push(&mut r, format!("mapping frame bytes ({} frames)", ma.frame_bytes.len()), &fbytes(ma), &fbytes(mb)) { r.images_equal = false; }
    // the head: constants, each record, the tail
    let hc = |h: &[u8]| h.get(..60).unwrap_or(h).to_vec();
    if !push(&mut r, "mapping head constants (60 B)".to_string(), &hc(&ma.head), &hc(&mb.head)) { r.head_equal = false; }
    r.records_a = crate::loadsinplay::record_kinds(&ma.head);
    r.records_b = crate::loadsinplay::record_kinds(&mb.head);
    for i in 0..r.records_a.len().max(r.records_b.len()) {
        let rec = |h: &[u8], i: usize| -> Vec<u8> { h.get(60 + 66 * i..60 + 66 * (i + 1)).map(|s| s.to_vec()).unwrap_or_default() };
        let (x, y) = (rec(&ma.head, i), rec(&mb.head, i));
        let kinds = format!(" kind {} / {}", r.records_a.get(i).map(|k| k.to_string()).unwrap_or("—".into()), r.records_b.get(i).map(|k| k.to_string()).unwrap_or("—".into()));
        // the record minus its kind word: the light words (MaxHDR, bounce, sky, LAmbient, HBasis) must not move when only the kind does
        let body = |v: &[u8]| v.get(4..).map(|s| s.to_vec()).unwrap_or_default();
        let eq_body = body(&x) == body(&y);
        if !push(&mut r, format!("frame record {i}{kinds}{}", if eq_body && x != y { " — only the kind word differs" } else { "" }), &x, &y) { r.head_equal = false; }
    }
    let tail = |h: &[u8]| { let n = h.len().saturating_sub(72) / 66; h.get(60 + 66 * n..).map(|s| s.to_vec()).unwrap_or_default() };
    if !push(&mut r, "mapping head tail (12 B)".to_string(), &tail(&ma.head), &tail(&mb.head)) { r.head_equal = false; }
    if !push(&mut r, "probe trailer".to_string(), &da.cache.trailer, &db.cache.trailer) { r.cache_equal = false; }
    let mut ids: Vec<u32> = da.cache.chunks.iter().chain(db.cache.chunks.iter()).map(|c| c.id).collect();
    ids.sort(); ids.dedup();
    for id in ids {
        let raw = |d: &crate::format::LightmapData| -> Option<Vec<u8>> { d.cache.chunk(id).and_then(|c| match &c.body { crate::format::ChunkBody::Raw(b) => Some(b.clone()), _ => None }) };
        let (x, y) = (raw(da), raw(db));
        if x.is_none() && y.is_none() { continue; } // the mapping chunk, compared above
        let (x, y) = (x.unwrap_or_default(), y.unwrap_or_default());
        if !push(&mut r, format!("cache chunk 0x{id:08X} ({} / {} B)", x.len(), y.len()), &x, &y) { r.cache_equal = false; }
    }
    Ok(r)
}

pub fn verdict(r: &PartReport) -> String {
    let kinds_ok = |k: &[u32], n: usize| crate::loadsinplay::kinds_rule(n, k).is_ok();
    match (r.images_equal, r.table_equal, r.head_equal, r.cache_equal) {
        (true, true, true, true) => "IDENTICAL".to_string(),
        (true, true, _, _) => format!("IMAGES + TABLE IDENTICAL; words moved (records {:?} → {:?}{}; cache {})", r.records_a, r.records_b, if kinds_ok(&r.records_b, r.frames_b) { " = the game's" } else { " ≠ the game's" }, if r.cache_equal { "equal" } else { "moved" }),
        (true, false, _, _) => "IMAGES IDENTICAL, RECTS MOVED (a layout change)".to_string(),
        (false, true, _, _) => "IMAGES MOVED on the same rects (a light change)".to_string(),
        (false, false, _, _) => "IMAGES + LAYOUT MOVED".to_string(),
    }
}

pub fn print(r: &PartReport) {
    for (part, x, y, eq) in &r.lines { println!("  [{}] {part:<48} {x}  {y}", if *eq { "  =  " } else { " DIFF" }); }
    println!("  → {}", verdict(r));
}

/// Two bank dirs: one line per cell present in both (`<cell>/ours.Map.Gbx`).
pub fn banks(old: &Path, new: &Path, tsv: Option<&Path>) -> Result<(), String> {
    let mut cells: Vec<String> = std::fs::read_dir(new).map_err(|e| format!("{}: {e}", new.display()))?.filter_map(|e| e.ok()).filter(|e| e.path().join("ours.Map.Gbx").exists()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    cells.sort();
    let mut out = String::from("cell\timages\ttable\thead\tcache\trecords_old\trecords_new\tverdict\n");
    let mut n_ident = 0; let mut n_words = 0; let mut n_moved = 0; let mut n_only = 0;
    for c in &cells {
        let (pa, pb) = (old.join(c).join("ours.Map.Gbx"), new.join(c).join("ours.Map.Gbx"));
        if !pa.exists() { println!("{c:<48} only in the new bank"); n_only += 1; out.push_str(&format!("{c}\t-\t-\t-\t-\t-\t-\tonly in the new bank\n")); continue; }
        let a = crate::mapio::load(&pa.to_string_lossy())?;
        let b = crate::mapio::load(&pb.to_string_lossy())?;
        let r = compare(&a, &b)?;
        let v = verdict(&r);
        if r.images_equal && r.table_equal && r.head_equal && r.cache_equal { n_ident += 1; } else if r.images_equal && r.table_equal { n_words += 1; } else { n_moved += 1; }
        println!("{c:<48} images {} table {} head {} cache {} records {:?} → {:?}: {v}", if r.images_equal { "=" } else { "DIFF" }, if r.table_equal { "=" } else { "DIFF" }, if r.head_equal { "=" } else { "DIFF" }, if r.cache_equal { "=" } else { "DIFF" }, r.records_a, r.records_b);
        out.push_str(&format!("{c}\t{}\t{}\t{}\t{}\t{:?}\t{:?}\t{v}\n", r.images_equal, r.table_equal, r.head_equal, r.cache_equal, r.records_a, r.records_b));
    }
    println!("== partcmp --banks: {} cells — {n_ident} identical, {n_words} images+table identical with words moved, {n_moved} images or layout moved, {n_only} only in the new bank", cells.len());
    if let Some(p) = tsv { std::fs::write(p, out).map_err(|e| format!("{}: {e}", p.display()))?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verdict_names_the_moved_part() {
        let r = PartReport { images_equal: true, table_equal: true, head_equal: false, cache_equal: true, records_a: vec![3, 3], records_b: vec![2, 3], frames_a: 2, frames_b: 2, ..Default::default() };
        assert!(verdict(&r).contains("IMAGES + TABLE IDENTICAL") && verdict(&r).contains("= the game's"), "{}", verdict(&r));
        let r2 = PartReport { images_equal: false, table_equal: true, head_equal: true, cache_equal: true, ..Default::default() };
        assert!(verdict(&r2).starts_with("IMAGES MOVED"));
        let r3 = PartReport { images_equal: true, table_equal: true, head_equal: true, cache_equal: true, ..Default::default() };
        assert_eq!(verdict(&r3), "IDENTICAL");
    }
}
