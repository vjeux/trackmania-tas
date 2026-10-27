//! `tinyctl expandable-lineup --pak F:KEY --survey matsurvey.tsv --out F.png`
//! — Hugo's CheckpointExpandable v2: the original GateExpandableFinish ground segment, the new item (the Air
//! pane with `RaceTriggerFXFinish` swapped for `RaceTriggerFXCheckpoint`) and the pak GateCheckpointCenter32m
//! gate, side by side in FRONT elevation (x right, y up) and PLAN (x right, z up), same scale, every triangle
//! filled with its material's mean colour from `mapgeom material-survey` — so the finish red against the
//! checkpoint blue is visible at a glance. A colour-coded elevation, not a game frame.
use crate::flagdraw::{Canvas, DIM, INK};

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

struct Panel {
    title: String,
    groups: Vec<(String, Vec<[f32; 3]>, Vec<[u32; 3]>)>,
}

fn collect(store: &mut mapgeom::store::DataStore, path: &str) -> Result<Vec<(String, Vec<[f32; 3]>, Vec<[u32; 3]>)>, String> {
    let loaded = store.load_model(path)?;
    let mut c = mapgeom::geom::Collector::new(store);
    c.link_labels = true;
    c.model(&loaded, &mapgeom::geom::IDENTITY, 0);
    let mut out: Vec<(String, Vec<[f32; 3]>, Vec<[u32; 3]>)> = c.scene.groups.iter().map(|(k, g)| (k.clone(), g.verts.clone(), g.tris.clone())).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn stem(label: &str) -> String {
    // the Collector's link label is `<stem>|<slot>`
    let s = label.split('|').next().unwrap_or(label);
    let s = s.rsplit(['\\', '/']).next().unwrap_or(s);
    s.trim_end_matches(".Material.Gbx").trim_end_matches(".Material.gbx").trim_end_matches(".Material").to_string()
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let out = f(rest, "--out").unwrap_or_else(|| "expandable-lineup.png".into());
    let survey = f(rest, "--survey").ok_or("expandable-lineup needs --survey matsurvey.tsv (mapgeom material-survey --out)")?;
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--pak" {
            if let Some((p, k)) = rest.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                store.add_pak(p, k)?;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    // material name -> (rgb, translucent)
    let mut colours: std::collections::HashMap<String, ([u8; 3], bool)> = std::collections::HashMap::new();
    let text = std::fs::read_to_string(&survey).map_err(|e| format!("{survey}: {e}"))?;
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 14 {
            continue;
        }
        let rgb: Vec<u8> = c[8].split(',').filter_map(|v| v.trim_start_matches('a').parse::<u8>().ok()).take(3).collect();
        if rgb.len() == 3 {
            colours.entry(c[1].to_string()).or_insert(([rgb[0], rgb[1], rgb[2]], c[13] == "1"));
        }
    }
    let colour_of = |label: &str| -> ([u8; 3], bool) {
        let s = stem(label);
        match colours.get(&s) {
            Some(x) => *x,
            None => match s.as_str() {
                "Grass" | "GrassNoFence" | "GrassFence" => ([96, 140, 60], false),
                "Visual" | "Metal" | "Rubber" => ([150, 150, 150], false),
                _ => ([170, 170, 170], false),
            },
        }
    };
    let ground = collect(&mut store, "Stadium\\Media\\Prefab\\Gate\\ExpandableFinish_Ground.Prefab.Gbx")?;
    let air = collect(&mut store, "Stadium\\Media\\Prefab\\Gate\\ExpandableFinish_Air.Prefab.Gbx")?;
    let gate = collect(&mut store, "Stadium\\Items\\GateCheckpointCenter32m.Item.Gbx")?;
    // the new item = the Air pane verbatim with the one material swap (mapgeom expandable-item)
    let item: Vec<(String, Vec<[f32; 3]>, Vec<[u32; 3]>)> = air.iter().map(|(k, v, t)| (if stem(k) == "RaceTriggerFXFinish" { "RaceTriggerFXCheckpoint".to_string() } else { k.clone() }, v.clone(), t.clone())).collect();
    // the gate item is centred on its origin: shift it to the block frame (16, 0, 16) so the three line up
    let gate: Vec<(String, Vec<[f32; 3]>, Vec<[u32; 3]>)> = gate.into_iter().map(|(k, v, t)| (k, v.into_iter().map(|p| [p[0] + 16.0, p[1], p[2] + 16.0]).collect(), t)).collect();
    let panels = [
        Panel { title: "GATEEXPANDABLEFINISH GROUND SEGMENT (ORIGINAL, RED)".into(), groups: ground },
        Panel { title: "CHECKPOINTEXPANDABLE.ITEM.GBX (NEW: THE AIR PANE, BLUE)".into(), groups: item },
        Panel { title: "GATECHECKPOINTCENTER32M (PAK ITEM, FOR THE COLOUR)".into(), groups: gate },
    ];
    let (w, h) = (2400usize, 1500usize);
    let mut cv = Canvas::new(w, h);
    cv.text(40.0, 16.0, "EXPANDABLE GATE LINE-UP: FRONT ELEVATION (BLOCK X RIGHT, Y UP) OVER PLAN (X RIGHT, Z UP); COLOURS = MATERIAL MEAN RGB FROM THE PAK TEXTURES (MATERIAL-SURVEY); SAME SCALE", 2, INK);
    let pw = (w as f32 - 80.0) / 3.0;
    // common frame: block x -2..34, y -1..13, z -2..34
    let (x0, x1, y0, y1, z0, z1) = (-2.0f32, 34.0f32, -1.0f32, 13.0f32, -2.0f32, 34.0f32);
    let scale = ((pw - 40.0) / (x1 - x0)).min(520.0 / (y1 - y0)).min(620.0 / (z1 - z0));
    for (pi, p) in panels.iter().enumerate() {
        let ox = 40.0 + pi as f32 * pw;
        let front_bottom = 90.0 + (y1 - y0) * scale;
        let plan_top = front_bottom + 70.0;
        let fx = |x: f32| ox + 20.0 + (x - x0) * scale;
        let fy = |y: f32| front_bottom - (y - y0) * scale;
        let pz = |z: f32| plan_top + (z1 - z) * scale;
        cv.text(ox + 20.0, 50.0, &p.title, 2, DIM);
        // ground line and the cell
        cv.line(fx(x0), fy(0.0), fx(x1), fy(0.0), [120, 120, 120], 1.0, 1.0);
        cv.outline(&[(fx(0.0), pz(0.0)), (fx(32.0), pz(0.0)), (fx(32.0), pz(32.0)), (fx(0.0), pz(32.0))], [120, 120, 120], 1.0);
        cv.text(ox + 20.0, plan_top - 20.0, "PLAN (CELL 32 X 32, GATE PLANE AT Z 16)", 2, DIM);
        let mut legend: Vec<(String, [u8; 3])> = Vec::new();
        for (label, verts, tris) in &p.groups {
            let (rgb, translucent) = colour_of(label);
            let a = if translucent { 0.55 } else { 0.9 };
            // the grass fence blades of the ground segment are 14 k verts of noise: thin them
            let step = if tris.len() > 4000 { 4 } else { 1 };
            for (ti, t) in tris.iter().enumerate() {
                if ti % step != 0 {
                    continue;
                }
                let q: Vec<[f32; 3]> = t.iter().map(|k| verts[*k as usize]).collect();
                if q.iter().any(|v| v[0] < x0 - 40.0 || v[0] > x1 + 40.0) {
                    continue;
                }
                cv.poly(&q.iter().map(|v| (fx(v[0]), fy(v[1]))).collect::<Vec<_>>(), rgb, a);
                cv.poly(&q.iter().map(|v| (fx(v[0]), pz(v[2]))).collect::<Vec<_>>(), rgb, a);
            }
            if !legend.iter().any(|(l, _)| *l == stem(label)) {
                legend.push((stem(label), rgb));
            }
        }
        legend.sort();
        let mut ly = plan_top + (z1 - z0) * scale + 30.0;
        for (name, rgb) in legend.iter().take(16) {
            cv.poly(&[(ox + 20.0, ly), (ox + 44.0, ly), (ox + 44.0, ly + 14.0), (ox + 20.0, ly + 14.0)], *rgb, 0.9);
            cv.text(ox + 52.0, ly + 1.0, &name.to_uppercase(), 2, INK);
            ly += 20.0;
        }
    }
    cv.text(40.0, h as f32 - 30.0, "THE NEW ITEM IS THE AIR SEGMENT'S OWN 24-VERTEX PANE WITH ITS ONE MATERIAL LINK SWAPPED (RACETRIGGERFXFINISH -> RACETRIGGERFXCHECKPOINT, THE SAME TADDMODCV SHADER THE CHECKPOINT GATE'S HOLO PANE USES); TRIGGER = THE PAK'S EXPANDABLECHECKPOINT_TRIGGER SLAB.", 2, INK);
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}");
    for p in &panels {
        let names: Vec<String> = p.groups.iter().map(|(k, v, t)| format!("{} ({} v / {} t)", stem(k), v.len(), t.len())).collect();
        println!("{}: {}", p.title, names.join(", "));
    }
    Ok(())
}
