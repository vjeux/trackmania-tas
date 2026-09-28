// E2 scratch: one draw's record from a passcap draws log — the pixel/vertex stage's textures and the named cbuffer values (filtered by a prefix)
fn main() {
    let a: Vec<String> = std::env::args().collect(); // ROOT FRAME EID [PREFIX...]
    let root = std::path::PathBuf::from(&a[1]);
    let frame: u32 = a[2].parse().unwrap();
    let eid: u64 = a[3].parse().unwrap();
    let draws = lightmap::lmaccum::load_draws(&root, frame).expect("draws log");
    let d = draws.iter().find(|e| e.get("eid").and_then(|v| v.as_u64()) == Some(eid)).expect("eid");
    for stage in ["Vertex", "Pixel"] {
        let Some(s) = d.get(stage) else { continue };
        println!("== {stage}: shader {}", s.get("shader").and_then(|v| v.as_str()).unwrap_or("?"));
        if let Some(t) = s.get("textures").or_else(|| s.get("resources")) { println!("  textures: {}", serde_json::to_string(t).unwrap().chars().take(1500).collect::<String>()); }
        if let Some(cb) = s.get("cbuffers").and_then(|v| v.as_object()) {
            for (name, vals) in cb {
                let Some(obj) = vals.as_object() else { println!("  {name}: {}", vals); continue };
                for (k, v) in obj {
                    if a.len() > 4 && !a[4..].iter().any(|p| k.contains(p.as_str())) { continue; }
                    let s = serde_json::to_string(v).unwrap();
                    println!("  {name}.{k} = {}", if s.len() > 400 { format!("{}…", &s[..400]) } else { s });
                }
            }
        }
        for k in ["samplers", "srvs", "resource_views"] { if let Some(v) = s.get(k) { println!("  {k}: {}", serde_json::to_string(v).unwrap().chars().take(800).collect::<String>()); } }
    }
    let keys: Vec<&String> = d.as_object().unwrap().keys().collect();
    println!("top-level keys: {:?}", keys);
}
