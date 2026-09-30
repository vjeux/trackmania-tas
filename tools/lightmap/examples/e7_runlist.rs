//! `e7_runlist tex-frameN.json [--ps N] [--rt ID]` — a baker `tex` export's run index as one line per run: run, eid range, draw
//! count, the VS/PS, the render targets (id, size, format) and which files were saved (E7, 2026-09-30: naming the accumulate /
//! SET / H-basis runs of a captured direction for the per-direction compare).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 { eprintln!("usage: e7_runlist tex-frameN.json [--ps N] [--rt ID]"); std::process::exit(2); }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let txt = std::fs::read_to_string(&a[1]).expect("json");
    let v: serde_json::Value = serde_json::from_str(&lightmap::passdiff::nan_free_json(&txt)).expect("json");
    let want_ps = f("--ps");
    let want_rt = f("--rt");
    println!("runs_total {}", v["runs_total"]);
    for r in v["saved"].as_array().expect("saved") {
        let ps = r["shaders"]["Pixel"].as_str().unwrap_or("-");
        let vs = r["shaders"]["Vertex"].as_str().unwrap_or("-");
        let files: Vec<String> = r["files"].as_array().map(|fs| fs.iter().map(|x| format!("{}={} {}×{} {}{}", x["slot"].as_str().unwrap_or("?"), x["tex"]["id"].as_str().unwrap_or("?"), x["tex"]["w"], x["tex"]["h"], x["tex"]["format"].as_str().unwrap_or("?").replace("_FLOAT", "F").replace("_TYPELESS", "T").replace("_UNORM", "U"), if x["ok"].as_bool() == Some(true) { "" } else { " (not saved)" })).collect()).unwrap_or_default();
        if let Some(p) = &want_ps { if ps != p { continue; } }
        if let Some(rt) = &want_rt { if !files.iter().any(|s| s.contains(&format!("={rt} "))) { continue; } }
        println!("run {:>3}: eid {:>6}..{:<6} n {:>5}  VS {:>5} PS {:>5}  {}  {}", r["run"], r["first"], r["last"], r["n"], vs, ps, files.join(" | "), r["flags"].as_str().unwrap_or("").replace("ActionFlags.", ""));
    }
}
