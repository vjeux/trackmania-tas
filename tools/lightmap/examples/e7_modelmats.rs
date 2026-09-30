//! `e7_modelmats MAP.Map.Gbx NAME_SUBSTR …` — per matching model: its material links (triangles per link, the TargetColor override), its
//! link-less diffuse textures and cut-out masks — what the pre-pass shades each class of its triangles with (E7, 2026-09-30: the g23
//! emitters whose sweep-0 ILightInput differs from the game's).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e7_modelmats MAP.Map.Gbx NAME_SUBSTR …"); std::process::exit(2); }
    let scene = lightmap::geometry::Scene::from_map(&a[1]).unwrap_or_else(|e| panic!("{}: {e}", a[1]));
    let mut seen = std::collections::HashSet::new();
    for inst in &scene.instances {
        if !a[2..].iter().any(|n| inst.model_name.contains(n.as_str())) || !seen.insert(inst.model) { continue; }
        let m = &scene.models[inst.model];
        let mut per_link: std::collections::BTreeMap<String, usize> = Default::default();
        let (mut n_diff, mut n_alpha, mut n_none) = (std::collections::BTreeMap::<String, usize>::new(), std::collections::BTreeMap::<String, usize>::new(), 0usize);
        for t in &m.tris {
            if (t.mat as usize) < m.mat_links.len() { *per_link.entry(format!("{} {}", m.mat_links[t.mat as usize], m.mat_params.get(t.mat as usize).and_then(|p| p.map(|c| format!("TargetColor ({:.3}, {:.3}, {:.3})", c[0], c[1], c[2]))).unwrap_or_default())).or_default() += 1; }
            else if (t.diff as usize) < m.diff_tex.len() { *n_diff.entry(m.diff_tex[t.diff as usize].clone()).or_default() += 1; }
            else if (t.alpha as usize) < m.alpha_tex.len() { *n_alpha.entry(m.alpha_tex[t.alpha as usize].clone()).or_default() += 1; }
            else { n_none += 1; }
        }
        let mut colours: std::collections::BTreeMap<u8, usize> = Default::default();
        for i in scene.instances.iter().filter(|i| i.model == inst.model) { *colours.entry(i.colour).or_default() += 1; }
        println!("{} ({} tris, {} instances of this model; placement colours (MapElemColor byte → count) {:?}):", inst.model_name, m.tris.len(), scene.instances.iter().filter(|i| i.model == inst.model).count(), colours);
        for (l, n) in &per_link { println!("   link  {n:>7} tris  {l}  albedo {:?}", m.mat_links.iter().position(|x| l.starts_with(x.as_str())).and_then(|i| m.mat_albedo.get(i)).map(|c| format!("({:.3}, {:.3}, {:.3})", c[0], c[1], c[2]))); }
        for (f, n) in &n_diff { println!("   diffuse texture (link-less) {n:>7} tris  {f}"); }
        for (f, n) in &n_alpha { println!("   cut-out (link-less)         {n:>7} tris  {f}"); }
        if n_none > 0 { println!("   no material {n_none} tris"); }
    }
}
