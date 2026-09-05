//! Byte-exact round trip of the CPlugCrystal model over a corpus of items:
//! every `*.Item.Gbx` under the given directories/files is parsed, its crystal
//! re-serialised, and the crystal's byte span compared. Prints the pass count
//! and, for every failure, the first differing offset with the field there.
//!
//!     crystal_roundtrip_all /tmp/tiny-full/nadeo /tmp/Sheep.Item.Gbx
use mapgeom::crystal_model::{locate, CPlugCrystal, LayerKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tmmaps::gbx::Gbx;

fn collect(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        let mut ents: Vec<_> = std::fs::read_dir(p).unwrap().map(|e| e.unwrap().path()).collect();
        ents.sort();
        for e in ents {
            collect(&e, out);
        }
    } else if p.to_string_lossy().ends_with(".Item.Gbx") {
        out.push(p.to_path_buf());
    }
}

/// Which field of the crystal a body offset falls in: the span's chunks are
/// re-parsed with a probe that records the offsets of its major fields.
fn field_at(body: &[u8], at: usize, off: usize) -> String {
    // Cheap, good enough for the report: name the enclosing chunk and, for
    // the layers chunk, the enclosing layer.
    let rd = |o: usize| u32::from_le_bytes(body[o..o + 4].try_into().unwrap());
    let mut o = at;
    let mut last = String::from("?");
    while o + 4 <= body.len() && o <= off {
        let cid = rd(o);
        last = format!("chunk 0x{cid:08X} (+0x{:x})", off.saturating_sub(o));
        if cid == 0xFACADE01 {
            break;
        }
        let next = match cid {
            0x09003004 => o + 12 + rd(o + 8) as usize,
            _ => {
                // walk forward to the next chunk id we know
                let mut p = o + 4;
                loop {
                    if p + 4 > body.len() {
                        break body.len();
                    }
                    let w = rd(p);
                    if (w & 0xFFFFF000) == 0x09003000 || w == 0xFACADE01 || w == 0x09051000 {
                        break p;
                    }
                    p += 1;
                }
            }
        };
        if next > off {
            break;
        }
        o = next;
    }
    last
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut files = Vec::new();
    for a in &args {
        collect(Path::new(a), &mut files);
    }
    let mut pass = 0usize;
    let mut fails: Vec<String> = Vec::new();
    let mut layer_types: BTreeMap<String, usize> = BTreeMap::new();
    let mut versions: BTreeMap<(u32, u32, u32), usize> = BTreeMap::new();
    let mut mat_versions: BTreeMap<(u32, u32, u32), usize> = BTreeMap::new();
    let mut chunk_orders: BTreeMap<String, usize> = BTreeMap::new();
    let mut notes: BTreeMap<String, usize> = BTreeMap::new();
    for f in &files {
        let bytes = match std::fs::read(f) {
            Ok(b) => b,
            Err(e) => {
                fails.push(format!("{}: read: {e}", f.display()));
                continue;
            }
        };
        let g = Gbx::parse(&bytes);
        let loc = match locate(&g.body) {
            Ok(l) => l,
            Err(e) => {
                fails.push(format!("{}: locate: {e}", f.display()));
                continue;
            }
        };
        let (c, end, lb) = match CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()) {
            Ok(x) => x,
            Err(e) => {
                fails.push(format!("{}: parse: {e}", f.display()));
                continue;
            }
        };
        let mut out = Vec::new();
        let mut lb2 = loc.lookback.clone();
        c.write(&mut out, &mut lb2);
        let orig = &g.body[loc.at..end];
        *chunk_orders.entry(format!("{:08X?}", c.chunks)).or_default() += 1;
        for l in &c.layers {
            *layer_types.entry(format!("{} v{} (layer v{})", l.kind.name(), match &l.kind {
                LayerKind::Geometry { version, .. } | LayerKind::Trigger { version, .. } => *version,
                LayerKind::SubdivideSmooth { version, .. } | LayerKind::Translation { version, .. } | LayerKind::Rotation { version, .. }
                | LayerKind::Scale { version, .. } | LayerKind::Mirror { version, .. } | LayerKind::MoveToGround { version, .. }
                | LayerKind::Extrude { version, .. } | LayerKind::Subdivide { version, .. } | LayerKind::Chaos { version, .. }
                | LayerKind::Smooth { version, .. } | LayerKind::BorderTransition { version, .. } | LayerKind::Deformation { version, .. }
                | LayerKind::SpawnPosition { version, .. } | LayerKind::Light { version, .. } => *version,
            }, l.base.version)).or_default() += 1;
            if let Some(cr) = l.kind.crystal() {
                *versions.entry((cr.version, c.layers_version, c.lightmap.as_ref().map(|l| l.version()).unwrap_or(99))).or_default() += 1;
                if !cr.edges.is_empty() { *notes.entry("crystal with non-empty edge array".into()).or_default() += 1; }
                if cr.positions.len() == 255 || cr.positions.len() == 65535 { *notes.entry(format!("positions == {}", cr.positions.len())).or_default() += 1; }
                if cr.tex_coords.len() == 255 || cr.tex_coords.len() == 65535 { *notes.entry(format!("tex_coords == {}", cr.tex_coords.len())).or_default() += 1; }
                if cr.groups.len() >= 255 { *notes.entry("groups >= 255".into()).or_default() += 1; }
                let nidx: usize = cr.faces.iter().map(|f| f.uv_index.len()).sum();
                if (nidx >= 256) != (cr.tex_coords.len() >= 256) || (nidx >= 65536) != (cr.tex_coords.len() >= 65536) { *notes.entry("tex index array: length band != coord-count band (a coord-count rule would differ from the length rule)".into()).or_default() += 1; }
            }
        }
        if c.materials.len() >= 255 { *notes.entry("materials >= 255".into()).or_default() += 1; }
        if let Some(mapgeom::crystal_model::Lightmap::V2 { coords, indices }) = &c.lightmap {
            if (indices.len() >= 256) != (coords.len() >= 256) || (indices.len() >= 65536) != (coords.len() >= 65536) { *notes.entry("lightmap index array: length band != coord-count band (a coord-count rule would differ from the length rule)".into()).or_default() += 1; }
        }
        for m in &c.materials {
            if let Some(i) = m.inst() {
                let mv = i.main.as_ref().map(|m| m.version).unwrap_or(99);
                let tv = i.tiling.as_ref().map(|t| t.version).unwrap_or(99);
                let cv = i.chunk2.map(|c| c.0).unwrap_or(99);
                *mat_versions.entry((mv, tv, cv)).or_default() += 1;
                if let Some(m) = &i.main { if !m.is_using_game_material { *notes.entry("material not using game material".into()).or_default() += 1; } }
            } else if m.node.is_some() { *notes.entry("material node back-reference".into()).or_default() += 1; } else { *notes.entry("named material".into()).or_default() += 1; }
        }
        let lit = c.lit_face_count();
        let all_geom: usize = c.layers.iter().filter_map(|l| match &l.kind { LayerKind::Geometry { crystal, .. } => Some(crystal.faces.len()), _ => None }).sum();
        let lm_n = match &c.lightmap { Some(mapgeom::crystal_model::Lightmap::V2 { indices, .. }) => indices.len(), _ => 0 };
        let lit_corners: usize = c.layers.iter().filter_map(|l| match &l.kind { LayerKind::Geometry { crystal, is_visible, .. } if l.base.is_enabled && *is_visible => Some(crystal.faces.iter().map(|f| f.verts.len()).sum::<usize>()), _ => None }).sum();
        *notes.entry(format!("smoothing ints == lit faces: {}, == all geometry faces: {}", c.per_face_ints.len() == lit, c.per_face_ints.len() == all_geom)).or_default() += 1;
        *notes.entry(format!("lightmap indices == lit corners: {}", lm_n == lit_corners)).or_default() += 1;
        for l in &c.layers { if let LayerKind::Geometry { is_visible, collidable, .. } = &l.kind { *notes.entry(format!("geometry layer {:?}: visible {} collidable {}", l.base.layer_name, is_visible, collidable)).or_default() += 1; } }
        match mapgeom::store::Model::parse(&bytes, "item").and_then(|m| { let g = m.graph()?; Ok(g.noderef_sites.len()) }) { Ok(_) => *notes.entry("Graph::parse ok".into()).or_default() += 1, Err(e) => *notes.entry(format!("Graph::parse FAILED: {}", e.chars().take(300).collect::<String>())).or_default() += 1 }
        if lb.table.len() != lb2.table.len() { *notes.entry("lookback table size differs after write".into()).or_default() += 1; }
        if out == orig {
            pass += 1;
        } else {
            let first = out.iter().zip(orig.iter()).position(|(a, b)| a != b).unwrap_or(out.len().min(orig.len()));
            fails.push(format!(
                "{}: MISMATCH at body 0x{:x} (span +0x{:x}, orig {} bytes, ours {} bytes) in {}; orig {:02x?} ours {:02x?}",
                f.display(), loc.at + first, first, orig.len(), out.len(), field_at(&g.body, loc.at, loc.at + first),
                &orig[first.min(orig.len())..(first + 8).min(orig.len())], &out[first.min(out.len())..(first + 8).min(out.len())]
            ));
        }
    }
    println!("{} items: {} pass, {} fail", files.len(), pass, fails.len());
    for f in &fails {
        println!("FAIL {f}");
    }
    println!("\nlayer types seen:");
    for (k, v) in &layer_types { println!("  {v:5}  {k}"); }
    println!("\n(crystal version, layers chunk version, lightmap version) seen:");
    for (k, v) in &versions { println!("  {v:5}  {k:?}"); }
    println!("\nmaterial chunk versions (000, 001, 002) seen:");
    for (k, v) in &mat_versions { println!("  {v:5}  {k:?}"); }
    println!("\nchunk orders seen:");
    for (k, v) in &chunk_orders { println!("  {v:5}  {k}"); }
    println!("\nnotes:");
    for (k, v) in &notes { println!("  {v:5}  {k}"); }
}
