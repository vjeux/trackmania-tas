//! Group-aware traversal search. Usage: ordergrp SRC.Item.Gbx REF.Item.Gbx
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let g = tmmaps::gbx::Gbx::parse(&data);
    let loc = mapgeom::crystal_model::locate(&g.body).unwrap();
    let (c, _, _) = mapgeom::crystal_model::CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone()).unwrap();
    let mats: Vec<String> = c.materials.iter().map(|m| m.inst().map(|x| x.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or_else(|| m.name.clone())).collect();
    let rdata = std::fs::read(&a[2]).unwrap();
    let rf = mapgeom::static_item::file::parse_file(&rdata).unwrap();
    let rso = rf.item.static_object().unwrap();
    let rs2 = rso.solid2().unwrap();
    let mut his: Vec<String> = Vec::new();
    for geom in &rs2.shaded_geoms {
        let mi = geom.material_index.max(0) as usize;
        his.push(rs2.custom_materials.get(mi).and_then(|m| m.inst()).map(|i| i.link().unwrap_or("?").rsplit('\\').next().unwrap_or("?").to_string()).unwrap_or("?".into()));
    }
    let stem = |l: &str| l.replace("SpecialSignTurbo", "Sign").replace("SpecialSignOff", "SignOff").replace("SpecialFXTurbo", "SpecialFX").replace("DecalSpecialTurbo", "Decal").replace("Modifier\\Turbo\\", "");
    let his_s: Vec<String> = his.iter().map(|s| stem(s)).collect();
    let layers = mapgeom::static_item::bake::geometry_layers(&c);
    // flatten visible faces: (group, mat)
    let mut faces: Vec<(u32, usize)> = Vec::new();
    for (cr, visible, _) in &layers {
        if !visible { continue; }
        for f in &cr.faces {
            faces.push((f.group, f.material.max(0) as usize));
        }
    }
    // group order of first appearance + contiguity check
    let mut seen: Vec<u32> = Vec::new();
    for (grp, _) in &faces {
        if !seen.contains(grp) { seen.push(*grp); }
    }
    println!("group first-appearance order: {seen:?}");
    let mut groups: std::collections::BTreeMap<u32, (usize, usize)> = Default::default();
    for (i, (grp, _)) in faces.iter().enumerate() {
        groups.entry(*grp).or_insert((i, 0));
        groups.get_mut(grp).unwrap().1 = i;
    }
    let mut gr: Vec<(u32, usize, usize)> = groups.into_iter().map(|(g, (s, e))| (g, s, e)).collect();
    gr.sort_by_key(|(_, s, _)| *s);
    println!("group spans (id,start,end): {gr:?}");
    // candidate traversals over face indices
    let n = faces.len();
    let all: Vec<usize> = (0..n).collect();
    let mut cands: Vec<(&str, Vec<usize>)> = vec![("file", all.clone())];
    // groups ascending/descending, faces within group forward/reverse
    let mut gasc: Vec<u32> = seen.clone(); gasc.sort();
    let mut gdesc: Vec<u32> = gasc.clone(); gdesc.reverse();
    for (gn, gl) in [("gasc", gasc), ("gdesc", gdesc)] {
        for (fn_, rev) in [("ffwd", false), ("frev", true)] {
            let mut seq = Vec::new();
            for grp in &gl {
                let mut idx: Vec<usize> = faces.iter().enumerate().filter(|(_, (g, _))| g == grp).map(|(i, _)| i).collect();
                if rev { idx.reverse(); }
                seq.extend(idx);
            }
            cands.push((match (gn, fn_) { ("gasc", "ffwd") => "gasc-ffwd", ("gasc", "frev") => "gasc-frev", ("gdesc", "ffwd") => "gdesc-ffwd", _ => "gdesc-frev" }, seq));
        }
    }
    for (name, seq) in &cands {
        let mut first: Vec<usize> = Vec::new();
        for i in seq {
            let mi = faces[*i].1;
            if !first.contains(&mi) { first.push(mi); }
        }
        let show: Vec<String> = first.iter().map(|m| stem(mats.get(*m).cloned().unwrap_or("?".into()).as_str())).collect();
        println!("{name}: match={} {show:?}", show == his_s);
    }
}
