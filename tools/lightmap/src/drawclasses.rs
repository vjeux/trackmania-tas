//! `lmtool draw-classes ROOT --frame N [--ps 17131,17134] [--eids]`: the captured frame's draw calls grouped
//! by RASTERISER CLASS — vertex / pixel shader, cull mode, front-face winding, depth bias, depth test, blend, the
//! render target's format and size, the viewport — with the draw count, the instance and index totals per class.
//! The culling question of the peel raster (perf engineer 5): which draw classes the GPU back-face culls, which
//! are two-sided, so the port culls exactly where the game does and nowhere else.

use std::collections::BTreeMap;

#[derive(Default, Clone)]
struct Class {
    draws: usize,
    instances: u64,
    indices: u64,
    eids: Vec<u64>,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let f = |k: &str| args.iter().position(|x| x == k).and_then(|i| args.get(i + 1)).cloned();
    let root = std::path::PathBuf::from(args.get(1).ok_or("usage: lmtool draw-classes ROOT --frame N [--ps A,B] [--eids]")?);
    let frame: u32 = f("--frame").ok_or("--frame N")?.parse().map_err(|e| format!("--frame: {e}"))?;
    let ps_filter: Option<Vec<String>> = f("--ps").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let show_eids = args.iter().any(|x| x == "--eids");
    let t0 = std::time::Instant::now();
    let bytes = crate::passdiff::read_entry_bytes(&root, &format!("logs/draws-frame{frame}.json"))?;
    let draws: serde_json::Value = serde_json::from_slice(&bytes).or_else(|_| serde_json::from_str(&crate::passdiff::repair_truncated_json(&String::from_utf8_lossy(&bytes)))).map_err(|e| format!("draws json: {e}"))?;
    let all = draws.as_array().ok_or("draws: not an array")?;
    eprintln!("{} entries parsed in {:.1} s", all.len(), t0.elapsed().as_secs_f32());
    let mut classes: BTreeMap<String, Class> = BTreeMap::new();
    let mut n_draws = 0usize;
    for e in all {
        if !e["flags"].as_str().map_or(false, |s| s.contains("Drawcall")) { continue; }
        let ps = e["Pixel"]["shader"].as_str().unwrap_or("-").to_string();
        if let Some(fl) = &ps_filter { if !fl.contains(&ps) { continue; } }
        n_draws += 1;
        let vs = e["Vertex"]["shader"].as_str().unwrap_or("-");
        let r = &e["raster"];
        let cull = r["cull"].as_str().unwrap_or("?").trim_start_matches("CullMode.");
        let ccw = r["frontCCW"].as_bool().map(|b| if b { "CCW" } else { "CW" }).unwrap_or("?");
        let bias = format!("bias {} slope {} clamp {}", r["depthBias"].as_i64().unwrap_or(0), r["slopeScaledDepthBias"].as_f64().unwrap_or(0.0), r["depthBiasClamp"].as_f64().unwrap_or(0.0));
        let clip = if r["depthClip"].as_bool().unwrap_or(true) { "clip" } else { "noclip" };
        let ds = &e["depthstate"];
        let depth = if ds["enable"].as_bool().unwrap_or(false) { format!("depth {} {}", ds["func"].as_str().unwrap_or("?").trim_start_matches("CompareFunction."), if ds["writes"].as_bool().unwrap_or(false) { "w" } else { "ro" }) } else { "nodepth".to_string() };
        let blend = e["blend"].as_array().and_then(|b| b.first()).map(|b| if b["enabled"].as_bool().unwrap_or(false) { format!("blend {}/{} {}", b["src"].as_str().unwrap_or("?").trim_start_matches("BlendMultiplier."), b["dst"].as_str().unwrap_or("?").trim_start_matches("BlendMultiplier."), b["op"].as_str().unwrap_or("?").trim_start_matches("BlendOperation.")) } else { "noblend".to_string() }).unwrap_or_default();
        let outs: Vec<String> = e["outputs"].as_array().map(|o| o.iter().map(|t| format!("{}x{} {}", t["w"].as_u64().unwrap_or(0), t["h"].as_u64().unwrap_or(0), t["format"].as_str().unwrap_or("?"))).collect()).unwrap_or_default();
        let dep = if e["depth"].is_null() { "nodsv".to_string() } else { format!("dsv {}x{} {}", e["depth"]["w"].as_u64().unwrap_or(0), e["depth"]["h"].as_u64().unwrap_or(0), e["depth"]["format"].as_str().unwrap_or("?")) };
        let vp = e["viewport"].as_array().map(|v| v.iter().take(4).map(|x| format!("{}", x.as_f64().unwrap_or(0.0))).collect::<Vec<_>>().join(",")).unwrap_or_default();
        let key = format!("VS {vs} PS {ps} | {cull} {ccw} {clip} | {bias} | {depth} | {blend} | rt [{}] {dep} | vp {vp}", outs.join(";"));
        let c = classes.entry(key).or_default();
        c.draws += 1;
        c.instances += e["inst"].as_u64().unwrap_or(0).max(1);
        c.indices += e["idx"].as_u64().unwrap_or(0);
        if show_eids && c.eids.len() < 12 { c.eids.push(e["eid"].as_u64().unwrap_or(0)); }
    }
    println!("frame {frame}: {n_draws} draw calls in {} classes", classes.len());
    let mut rows: Vec<(&String, &Class)> = classes.iter().collect();
    rows.sort_by(|a, b| b.1.draws.cmp(&a.1.draws));
    for (k, c) in rows {
        println!("{:>6} draws {:>8} inst {:>12} idx  {k}", c.draws, c.instances, c.indices);
        if show_eids { println!("        eids {:?}", c.eids); }
    }
    Ok(())
}
