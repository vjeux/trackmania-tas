//! Search face-traversal rules reproducing Granady's material order.
//! Usage: orderule SRC.Item.Gbx REF.Item.Gbx
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let mats: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone())).collect();
    // his order (stems)
    let rdata = std::fs::read(&a[2]).unwrap();
    let rf = mapgeom::static_item::file::parse_file(&rdata).unwrap();
    let rso = rf.item.static_object().unwrap();
    let rs2 = rso.solid2().unwrap();
    let mut his: Vec<String> = Vec::new();
    for geom in &rs2.shaded_geoms {
        let mi = geom.material_index.max(0) as usize;
        let link = rs2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into());
        his.push(link);
    }
    let stem = |l: &str| l.replace("SpecialSignTurbo", "Sign").replace("SpecialSignOff", "SignOff").replace("SpecialFXTurbo", "SpecialFX").replace("DecalSpecialTurbo", "Decal").replace("Modifier\\Turbo\\", "");
    let his_s: Vec<String> = his.iter().map(|s| stem(s)).collect();
    println!("his order: {:?}", his_s);
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    // candidate traversals: sequence of (layer_idx, face_idx) in order
    let nl = layers.len();
    let mut seqs: Vec<(&str, Vec<(usize, usize)>)> = Vec::new();
    let fwd: Vec<(usize, usize)> = (0..nl).flat_map(|li| (0..layers[li].0.faces.len()).map(move |fi| (li, fi))).collect();
    seqs.push(("fwd", fwd.clone()));
    seqs.push(("rev-layers", (0..nl).rev().flat_map(|li| (0..layers[li].0.faces.len()).map(move |fi| (li, fi))).collect()));
    seqs.push(("rev-faces", (0..nl).flat_map(|li| (0..layers[li].0.faces.len()).rev().map(move |fi| (li, fi))).collect()));
    seqs.push(("fully-rev", (0..nl).rev().flat_map(|li| (0..layers[li].0.faces.len()).rev().map(move |fi| (li, fi))).collect()));
    for (name, seq) in &seqs {
        // first-appearance and last-appearance orders of material stems (visible only)
        let mut first: Vec<usize> = Vec::new();
        let mut last: std::collections::BTreeMap<usize, usize> = Default::default();
        for (pos, (li, fi)) in seq.iter().enumerate() {
            let (cr, visible, _) = &layers[*li];
            if !visible { continue; }
            let mi = cr.faces[*fi].material.max(0) as usize;
            if !first.contains(&mi) { first.push(mi); }
            last.insert(mi, pos);
        }
        let mut by_last: Vec<usize> = last.keys().cloned().collect();
        by_last.sort_by_key(|m| std::cmp::Reverse(last[m]));
        let show = |v: &[usize]| v.iter().map(|m| stem(mats.get(*m).cloned().unwrap_or("?".into()).as_str())).collect::<Vec<_>>();
        let fa = show(&first);
        let la = show(&by_last);
        println!("{name}: first-app match={} last-desc match={}", fa == his_s, la == his_s);
        if fa != his_s && la != his_s {
            println!("  first: {fa:?}");
            println!("  lastd: {la:?}");
        }
    }
}
