//! S2: read pack prefabs with the static-item model. For each prefab given
//! (logical pack path), parse it, re-emit it, compare bytes, and report the
//! vertex layouts of every inline CPlugStaticObjectModel visual.
use mapgeom::static_item::prefab::CPlugPrefab;
use mapgeom::static_item::{vstream, Node};
use mapgeom::store::DataStore;
use std::collections::BTreeMap;

fn layout(s: &vstream::CPlugVertexStream) -> String {
    let names = |n: u32| match n {
        0 => "Position".to_string(),
        5 => "Normal".into(),
        8 => "Color0".into(),
        10..=17 => format!("TexCoord{}", n - 10),
        18 => "TangentU".into(),
        20 => "TangentV".into(),
        n => format!("name{n}"),
    };
    let tys = |t: u32| match t {
        1 => "Float2".to_string(),
        2 => "Float3".into(),
        3 => "Float4".into(),
        4 => "Color".into(),
        5 => "Int32".into(),
        14 => "Dec3N".into(),
        t => format!("type{t}"),
    };
    let c = s.compress_local3d.unwrap_or(false);
    s.decls
        .iter()
        .map(|d| format!("{} {}{}@{:#x}", names(d.name()), tys(d.ty()), if d.stored_type(c) != d.ty() { "->Dec3N" } else { "" }, d.offset()))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut paks = Vec::new();
    let mut files = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--pak" {
            paks.push(it.next().expect("--pak PATH:KEY"));
        } else {
            files.push(a);
        }
    }
    let mut store = DataStore::empty();
    for p in &paks {
        let (path, key) = p.split_once(':').unwrap_or((p.as_str(), mapgeom::store::STADIUM_KEY));
        store.add_pak(path, key).unwrap();
    }
    if files.iter().any(|f| f == "--all-prefabs") {
        files.retain(|f| f != "--all-prefabs");
        let all: Vec<String> = store
            .entries()
            .filter(|e| e.class_id == 0x09145000)
            .map(|e| if e.folder.is_empty() { e.name.clone() } else { format!("{}\\{}", e.folder.trim_end_matches('\\'), e.name) })
            .collect();
        files.extend(all);
    }
    let quiet = files.len() > 20;
    let mut layouts: BTreeMap<String, usize> = BTreeMap::new();
    let mut n_ok = 0usize;
    let mut fail = 0;
    for f in &files {
        let m = match store.load_model(f) {
            Ok(m) => m,
            Err(e) => {
                println!("FAIL load {f}: {e}");
                fail += 1;
                continue;
            }
        };
        let p = match CPlugPrefab::from_model(&m) {
            Ok(p) => p,
            Err(e) => {
                println!("FAIL parse {f}: {e}");
                if let Some(p) = std::env::var_os("STATIC_FAIL_BODY") {
                    std::fs::write(p, &m.body).unwrap();
                }
                fail += 1;
                continue;
            }
        };
        let out = p.write();
        let same = out == m.body;
        if !same {
            fail += 1;
            let first = out.iter().zip(m.body.iter()).position(|(a, b)| a != b).unwrap_or(out.len().min(m.body.len()));
            println!("FAIL bytes {f}: first difference at 0x{first:x}");
        }
        let (mut n_static, mut n_ext, mut n_vis) = (0, 0, 0);
        let mut ext: BTreeMap<String, usize> = BTreeMap::new();
        for e in &p.ents {
            match e.model.inline.as_deref() {
                Some(Node::StaticObject(so)) => {
                    n_static += 1;
                    if !quiet {
                        if let Some(sf) = so.surface() {
                            let mut tails: BTreeMap<(u8, u8, i16), usize> = BTreeMap::new();
                            if let mapgeom::static_item::surface::Surf::Mesh { triangles, .. } = &sf.surf {
                                for t in triangles {
                                    *tails.entry((t.material_id, t.u03, t.surface_index)).or_default() += 1;
                                }
                            }
                            println!("  surface: material_ids {:?}, materials {}, dir {:?}, triangle (phys, u03, index) counts {:?}", sf.material_ids, sf.materials.len(), sf.gameplay_main_dir, tails);
                        }
                    }
                    if let Some(s2) = so.solid2() {
                        for v in &s2.visuals {
                            if let Some(Node::Visual(vis)) = v.inline.as_deref() {
                                n_vis += 1;
                                if let Some(vs) = vis.stream() {
                                    *layouts.entry(layout(vs)).or_default() += 1;
                                }
                            }
                        }
                    }
                }
                Some(other) => println!("  entity model class 0x{:08X} (inline, not a static object)", other.class_id()),
                None => {
                    n_ext += 1;
                    let name = m.externals.iter().find(|(i, _)| *i as i32 == e.model.index).map(|(_, p)| p.as_str()).unwrap_or("(null model)");
                    *ext.entry(name.to_string()).or_default() += 1;
                }
            }
        }
        if same {
            n_ok += 1;
        }
        if !quiet {
            println!("{} {f}: {} bytes, {} entities ({n_static} static objects, {n_ext} external), {n_vis} visuals", if same { "ok  " } else { "DIFF" }, m.body.len(), p.ents.len());
            for (n, k) in &ext {
                println!("       {k:4} x  {n}");
            }
        }
    }
    println!("{n_ok} identical of {} prefabs, {fail} failed", files.len());
    println!("vertex layouts:");
    for (l, n) in &layouts {
        println!("  {n:4} x  {l}");
    }
    if fail > 0 {
        std::process::exit(1);
    }
}
