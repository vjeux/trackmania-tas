//! `re17_matswitch --pak FILE:KEY … [--prefix 'Stadium\Media\Material\'] [--switch PreLightGen] [--all] [LINK …]` — every
//! material's BOOL SWITCH list (CPlugMaterialCustom chunk 0x0903A00C: `(name, value)` pairs — `PreLightGen`,
//! `BaseColorHueMask`, `OpacityIsDiffuseAlpha`, `IsPoleEmblem`, `UseTexBlend`, `PyAxeU`, …) with its parent material,
//! for the given links or for every 0x09079000 entry under a prefix. Default: print only the materials whose named
//! switch (default `PreLightGen`) is SET; `--all` prints every material's list.
//!
//! WHY (RE 17, 2026-09-29, the FLAG SPILL — box 1c): the switch `PreLightGen` = 1 is the material-side selector of
//! the shader permutation `DTwk_SkipMap_PreLightGen` (105 of the 500 Tech3 GpuCache programs carry that define): a
//! material with it set takes NO lightmap set — no chart, no pre-pass draw, no accumulate draw, and the item
//! pipeline's PreLightGen uv box excludes its geoms (AI06220000's box covers the pole, not the flag). Stadium has
//! exactly two such materials: ItemFlagNoAnim (Tech3_Block_TDSN_CubeOut) and the Speedometer (TDSNI_CubeOut).
//! RE 16's 18:05Z gate (pass bit 0x1000 + a PreLightGen* binding on the PARENT shader) admits them because it reads
//! the parent shader's binding list, not the permutation the material's switch selects.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = mapgeom::store::DataStore::empty();
    let mut links: Vec<String> = Vec::new();
    let mut prefix = String::new();
    let mut switch = "PreLightGen".to_string();
    let mut all = false;
    let mut i = 1;
    while i < a.len() {
        match a[i].as_str() {
            "--pak" => { if let Some(v) = a.get(i + 1) { let (pp, key) = v.rsplit_once(':').expect("--pak FILE:KEY"); store.add_pak(pp, key).expect("pak"); } i += 2; }
            "--prefix" => { prefix = a.get(i + 1).cloned().unwrap_or_default(); i += 2; }
            "--switch" => { switch = a.get(i + 1).cloned().unwrap_or_default(); i += 2; }
            "--all" => { all = true; i += 1; }
            s => { links.push(s.to_string()); i += 1; }
        }
    }
    if links.is_empty() {
        let pfx = prefix.to_ascii_lowercase();
        let mut v: Vec<String> = store.entries().filter(|e| e.class_id == 0x0907_9000 && (pfx.is_empty() || e.path().to_ascii_lowercase().starts_with(&pfx))).map(|e| e.path()).collect();
        v.sort();
        links = v;
    }
    let mut n_set = 0usize;
    let mut n_read = 0usize;
    for l in &links {
        let file = if l.to_ascii_lowercase().ends_with(".gbx") || store.resolve(l).is_some() { l.clone() } else { format!("{l}.Material.Gbx") };
        let m = match store.load_model(&file) { Ok(m) => m, Err(e) => { eprintln!("{l}: {e}"); continue; } };
        let g = match m.graph() { Ok(g) => g, Err(e) => { eprintln!("{l}: graph: {e}"); continue; } };
        let parent = m.externals.iter().map(|(_, p)| p.clone()).find(|p| p.to_ascii_lowercase().ends_with(".material.gbx") && !p.eq_ignore_ascii_case(&file)).unwrap_or_default();
        let textures: Vec<String> = m.externals.iter().map(|(_, p)| p.clone()).filter(|p| p.to_ascii_lowercase().contains(".texture.")).map(|p| p.rsplit('\\').next().unwrap_or(&p).to_string()).collect();
        for s in &g.slots {
            if let mapgeom::node::Slot::Node(mapgeom::node::Node::MaterialCustom(c)) = s {
                n_read += 1;
                let set = c.switches.iter().any(|(n, v)| n.eq_ignore_ascii_case(&switch) && *v);
                if set { n_set += 1; }
                if all || set {
                    let sw: Vec<String> = c.switches.iter().map(|(n, v)| format!("{n}={}", *v as u8)).collect();
                    println!("{l}\n    parent {}\n    switches [{}]\n    textures {:?}", if parent.is_empty() { "(none)" } else { &parent }, sw.join(", "), textures);
                }
            }
        }
    }
    println!("{n_read} custom materials read under {prefix:?}; {n_set} with `{switch}` set");
}
