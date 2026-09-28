//! `e5_matparents MAP.Map.Gbx --pak FILE:KEY … [NAME_SUBSTR…]` — per embedded item model (all, or those whose name contains a
//! substring): every material link with its PARENT material and SHADER as the pack resolves them (paktables::shader_of) — RE 16's
//! 23:26Z listing: only a `Tech3 Warp*` PARENT (pass word 0x40030441, no PreLightGen binding) puts a geom into the environment
//! block with PS 16752's fogged analytic light; a Warp-named child of `Tech3_Block_TDSN_CubeOut` (WarpTechnic) is a plain item
//! material. E5, 2026-09-28.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let map = a.get(1).expect("MAP.Map.Gbx");
    let mut store = mapgeom::store::DataStore::empty();
    let mut subs: Vec<String> = Vec::new();
    let mut i = 2;
    while i < a.len() {
        if a[i] == "--pak" { if let Some(v) = a.get(i + 1) { let (pp, key) = v.rsplit_once(':').expect("--pak FILE:KEY"); store.add_pak(pp, key).expect("pak"); } i += 2; continue; }
        subs.push(a[i].clone());
        i += 1;
    }
    let scene = lightmap::geometry::Scene::from_map(map).unwrap_or_else(|e| panic!("{e}"));
    let mut seen: std::collections::BTreeMap<String, (String, String, usize)> = std::collections::BTreeMap::new();
    for (mi, m) in scene.models.iter().enumerate() {
        let name = scene.model_names.get(mi).cloned().unwrap_or_default();
        if !subs.is_empty() && !subs.iter().any(|s| name.contains(s.as_str())) { continue; }
        let n_inst = scene.instances.iter().filter(|it| it.model == mi).count();
        if n_inst == 0 { continue; }
        for l in &m.mat_links {
            let e = seen.entry(l.clone()).or_insert_with(|| {
                let file = if l.to_ascii_lowercase().ends_with(".gbx") { l.clone() } else { format!("{l}.Material.Gbx") };
                match lightmap::paktables::shader_of(&mut store, &file) { Ok((sh, pa)) => (pa, sh, 0), Err(e) => (format!("ERR {e}"), String::new(), 0) }
            });
            e.2 += n_inst;
        }
    }
    println!("{} distinct material links over the selected models' placements:", seen.len());
    for (l, (parent, shader, n)) in &seen {
        let warp_parent = parent.contains("warp");
        println!("{:>6} placements  {l}\n         parent {}\n         shader {}{}", n, if parent.is_empty() { "(none)" } else { parent }, if shader.is_empty() { "(none)" } else { shader }, if warp_parent { "   ← WARP PARENT (environment-block class)" } else { "" });
    }
}
