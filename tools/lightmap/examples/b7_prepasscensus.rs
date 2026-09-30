//! `b7_prepasscensus DRAWS.json[.gz] [--ps N] [--idx N] [--eid N] [--top K]` — the pre-pass frame's draws by PIXEL SHADER
//! class with what RE 17's item 1 reads (2026-09-30, the g23pc frame 537): per PS class the draw/instance/index totals,
//! the DISTINCT SRV bindings of the pixel stage (slot, texture id, size, format, VIEW format, array size) with their
//! counts, the cbuffer names + the first draw's cbuffer values of `ShaderP` (RgbTargetColor …) and `DrawP`
//! (i4_PyPxzX2H2s …); `--idx 24` restricts to DrawIndexed 24 draws (the zone tiles), `--eid N` prints ONE draw whole
//! (its SRVs and every cbuffer), `--ps N` one class. Python's bare NaN tokens are rewritten to null before parsing.
use std::collections::BTreeMap;

fn nan_free(txt: &str) -> String { lightmap::passdiff::nan_free_json(txt) }

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let path = a.iter().find(|x| !x.starts_with("--") && x.contains("json")).expect("DRAWS.json[.gz]");
    let only_ps: Option<u64> = f("--ps").map(|v| v.parse().unwrap());
    let only_idx: Option<u64> = f("--idx").map(|v| v.parse().unwrap());
    let only_eid: Option<u64> = f("--eid").map(|v| v.parse().unwrap());
    let top: usize = f("--top").map(|v| v.parse().unwrap()).unwrap_or(40);
    // read_entry_bytes transparently gunzips (the same reader every capture tool uses)
    let p = std::path::Path::new(path);
    let raw = lightmap::passdiff::read_entry_bytes(p.parent().unwrap_or(std::path::Path::new(".")), p.file_name().unwrap().to_str().unwrap().trim_end_matches(".gz")).expect("read");
    let txt = String::from_utf8_lossy(&raw).to_string();
    let v: serde_json::Value = serde_json::from_str(&txt).or_else(|_| serde_json::from_str(&nan_free(&txt))).expect("json");
    let all = v.as_array().expect("array");
    struct Class { draws: usize, inst: u64, idx: u64, eids: Vec<u64>, srvs: BTreeMap<String, usize>, cbs: BTreeMap<String, usize>, first_shaderp: Option<serde_json::Value>, first_drawp: Option<serde_json::Value>, vs: String, rt: String }
    let mut classes: BTreeMap<u64, Class> = BTreeMap::new();
    for e in all {
        if !e["flags"].as_str().map_or(false, |s| s.contains("Drawcall")) { continue; }
        let eid = e["eid"].as_u64().unwrap_or(0);
        if let Some(x) = only_eid { if eid != x { continue; } println!("{}", serde_json::to_string_pretty(e).unwrap()); return; }
        let ps: u64 = e["Pixel"]["shader"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
        if let Some(x) = only_ps { if ps != x { continue; } }
        let idx = e["idx"].as_u64().unwrap_or(0);
        if let Some(x) = only_idx { if idx != x { continue; } }
        let c = classes.entry(ps).or_insert_with(|| Class { draws: 0, inst: 0, idx: 0, eids: Vec::new(), srvs: BTreeMap::new(), cbs: BTreeMap::new(), first_shaderp: None, first_drawp: None, vs: e["Vertex"]["shader"].as_str().unwrap_or("-").to_string(), rt: e["outputs"].as_array().map(|o| o.iter().map(|t| format!("{}x{} {}", t["w"], t["h"], t["format"].as_str().unwrap_or("?"))).collect::<Vec<_>>().join(";")).unwrap_or_default() });
        c.draws += 1;
        c.inst += e["inst"].as_u64().unwrap_or(0).max(1);
        c.idx += idx;
        if c.eids.len() < 6 { c.eids.push(eid); }
        if let Some(srvs) = e["Pixel"]["srvs"].as_array().or_else(|| e["srvs"]["Pixel"].as_array()) {
            for s in srvs {
                let t = &s["tex"];
                let key = format!("t{} id {} {}x{}x{} {} mips {} view {}", s["slot"], t["id"].as_str().or_else(|| s["resource"].as_str()).unwrap_or("?"), t["w"], t["h"], t["arr"], t["format"].as_str().unwrap_or("?"), t["mips"], s["viewFormat"].as_str().unwrap_or("(log mode: resource format only)"));
                *c.srvs.entry(key).or_insert(0) += 1;
            }
        }
        if let Some(cbs) = e["Pixel"]["cbuffers"].as_object() {
            for (name, val) in cbs {
                *c.cbs.entry(name.clone()).or_insert(0) += 1;
                if name.contains("ShaderP") && c.first_shaderp.is_none() { c.first_shaderp = Some(val.clone()); }
                if name.contains("DrawP") && c.first_drawp.is_none() { c.first_drawp = Some(val.clone()); }
            }
        }
        if let Some(cbs) = e["Vertex"]["cbuffers"].as_object() {
            for (name, val) in cbs {
                *c.cbs.entry(format!("VS:{name}")).or_insert(0) += 1;
                if name.contains("DrawP") && c.first_drawp.is_none() { c.first_drawp = Some(val.clone()); }
            }
        }
    }
    let mut order: Vec<(&u64, &Class)> = classes.iter().collect();
    order.sort_by(|a, b| b.1.draws.cmp(&a.1.draws));
    println!("{} draws in {} PS classes{}", order.iter().map(|(_, c)| c.draws).sum::<usize>(), order.len(), only_idx.map(|i| format!(" (idx {i} only)")).unwrap_or_default());
    for (ps, c) in order.iter().take(top) {
        println!("\n== PS {ps} (VS {}): {} draws, {} inst, {} idx; rt {}; eids {:?}", c.vs, c.draws, c.inst, c.idx, c.rt, c.eids);
        for (k, n) in &c.srvs { println!("   SRV {k}  ×{n}"); }
        println!("   cbuffers: {}", c.cbs.iter().map(|(k, n)| format!("{k}×{n}")).collect::<Vec<_>>().join(", "));
        if let Some(sp) = &c.first_shaderp { println!("   first ShaderP: {}", serde_json::to_string(sp).unwrap().chars().take(1200).collect::<String>()); }
        if let Some(dp) = &c.first_drawp { println!("   first DrawP: {}", serde_json::to_string(dp).unwrap().chars().take(1200).collect::<String>()); }
    }
}
