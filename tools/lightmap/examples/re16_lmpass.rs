//! `re16_lmpass MAP.Map.Gbx --pak FILE:KEY [--pak …] [--tris]` — every game-material link of a map's item models against the
//! game's LIGHTMAP-PASS TEST as the binary applies it (RE 11 09:20Z, FUN_140216b30: a geom gets a TcLM vertex stream — is a
//! receiver and, since every peel draw is the ILightInput shader (f1617: PS 12447/12450 only), an occluder/emitter — iff its
//! material's SHADER has pass bit 0x1000 (`.Shader.Gbx` chunk 0x09002020 u16) AND a `PreLightGen*` bitmap binding (whose
//! CPlugBitmapAddress::TexCoordIndex names the LM uv set)). Prints per link: the chain's shader, its pass word, the PLG tc,
//! the verdict (LIT | NOT-LIT | UNRESOLVED) and the triangle / model counts, then the totals. The port's lmmesh.rs applies the
//! same two facts (`prefetch_lm_uv_index`); an UNRESOLVED link falls back to "has a second uv set" there. RE 16 (2026-09-28),
//! read 2 (tiny03's record: which embedded materials the game lets into the passes).
use std::collections::BTreeMap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 { eprintln!("usage: re16_lmpass MAP.Map.Gbx --pak FILE:KEY [--pak …] [--tris]"); std::process::exit(2); }
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 2;
    while i < a.len() { if a[i] == "--pak" { if let Some((p, k)) = a[i + 1].rsplit_once(':') { store.add_pak(p, k).unwrap_or_else(|e| eprintln!("{p}: {e}")); } i += 2; } else { i += 1; } }
    let scene = lightmap::geometry::Scene::from_map(&a[1]).expect("scene");
    // per link: (triangles over all placements, models using it)
    let mut per: BTreeMap<String, (usize, std::collections::BTreeSet<usize>)> = BTreeMap::new();
    for inst in &scene.instances {
        let m = &scene.models[inst.model];
        for t in &m.tris { if let Some(l) = m.mat_links.get(t.mat as usize) { let e = per.entry(l.clone()).or_default(); e.0 += 1; e.1.insert(inst.model); } }
    }
    let (mut lit, mut not, mut unres) = ((0usize, 0usize), (0usize, 0usize), (0usize, 0usize));
    println!("{}: {} instances, {} models, {} material links", a[1], scene.instances.len(), scene.models.len(), per.len());
    println!("{:<48} {:<44} {:>6} {:>4}  {:<10} {:>9} {:>6}", "link", "shader", "pass", "plg", "verdict", "tris", "models");
    for (link, (tris, models)) in &per {
        let mat = if link.to_ascii_lowercase().ends_with(".material.gbx") { link.clone() } else { format!("{link}.Material.Gbx") };
        let chain = mapgeom::envblock::material_chain(&mut store, &mat);
        let (shader, pass, plg, verdict) = if chain.shader.is_empty() {
            (String::from("-"), None, None, "UNRESOLVED")
        } else {
            let pass = chain.flags.map(|f| f.pass_bits);
            let plg = store.read(&chain.shader).ok().and_then(|b| lightmap::lmmesh::shader_prelightgen_tc(&b));
            let ok = pass.map(|p| p & 0x1000 != 0).unwrap_or(false) && plg.is_some();
            (chain.shader.rsplit('\\').next().unwrap_or(&chain.shader).to_string(), pass, plg, if ok { "LIT" } else { "NOT-LIT" })
        };
        match verdict { "LIT" => { lit.0 += tris; lit.1 += 1 }, "NOT-LIT" => { not.0 += tris; not.1 += 1 }, _ => { unres.0 += tris; unres.1 += 1 } }
        let short = link.rsplit('\\').next().unwrap_or(link);
        println!("{:<48} {:<44} {:>6} {:>4}  {:<10} {:>9} {:>6}", short, shader, pass.map(|p| format!("0x{p:04x}")).unwrap_or_else(|| "-".into()), plg.map(|t| t.to_string()).unwrap_or_else(|| "-".into()), verdict, tris, models.len());
    }
    println!("TOTAL: LIT {} links / {} tris; NOT-LIT {} links / {} tris; UNRESOLVED {} links / {} tris", lit.1, lit.0, not.1, not.0, unres.1, unres.0);
}
