//! `e7_pixtrace DBG.json SHADER.txt` — a RenderDoc pixel-debug export (baker-7's rdexport `pixeldbg` mode: inputs, cbuffers,
//! `steps[]` with the changed registers as u32 bit patterns) printed step by step beside the DXBC instruction it executed, the
//! registers as f32 (NaN/±INF spelled out, the hex beside) — the hardware's own arithmetic on one fragment, the authority for
//! a transcription's NaN/saturate semantics (E7, 2026-09-30: PS 13424 on the g23 skirt at pixel (2503, 214) of frame 557).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: e7_pixtrace DBG.json SHADER.txt"); std::process::exit(2); }
    let dbg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&a[1]).expect("dbg")).expect("json");
    let sh = std::fs::read_to_string(&a[2]).expect("shader");
    // the instruction listing: "  N: text"
    let mut instr: Vec<(usize, String)> = Vec::new();
    for l in sh.lines() {
        let t = l.trim_start();
        if let Some((n, rest)) = t.split_once(": ") { if let Ok(k) = n.trim().parse::<usize>() { if !t.starts_with("dcl") { instr.push((k, rest.trim().to_string())); } } }
    }
    let text_of = |k: usize| instr.iter().find(|(n, _)| *n == k).map(|(_, t)| t.as_str()).unwrap_or("?");
    let f = |v: &serde_json::Value| -> String {
        match v {
            serde_json::Value::Number(n) => {
                if let Some(u) = n.as_u64() { let x = f32::from_bits(u as u32); if x.is_nan() { format!("NaN[{u:#010x}]") } else if x.is_infinite() { format!("{}INF", if x > 0.0 { "+" } else { "-" }) } else if u == 0 { "0".into() } else { format!("{x:.6}") } }
                else if let Some(x) = n.as_f64() { format!("{x:.6}") } else { n.to_string() }
            }
            other => other.to_string(),
        }
    };
    println!("pixel ({}, {}) of eid {}", dbg["x"], dbg["y"], dbg["eid"]);
    if let Some(inputs) = dbg["inputs"].as_array() {
        for i in inputs { let vals = match &i["value"] { serde_json::Value::Array(v) => v.iter().map(|x| format!("{}", x)).collect::<Vec<_>>().join(", "), x => x.to_string() }; println!("  in {} = ({vals})", i["name"]); }
    }
    let Some(steps) = dbg["steps"].as_array() else { println!("no steps"); return };
    for s in steps {
        let next = s["nextInstruction"].as_u64().unwrap_or(0) as usize;
        // step k executed instruction next−1 (the first step is the register init)
        let exec = if next == 0 { None } else { Some(next - 1) };
        let ch: Vec<String> = s["changes"].as_array().map(|c| c.iter().map(|c| { let vals = c["after"].as_array().map(|v| v.iter().map(|x| f(x)).collect::<Vec<_>>().join(", ")).unwrap_or_default(); format!("{} = ({vals})", c["name"].as_str().unwrap_or("?")) }).collect()).unwrap_or_default();
        match exec { Some(k) => println!("{:>3}: {:<80} → {}", k, text_of(k), ch.join("; ")), None => println!("init: {}", ch.join("; ")) }
    }
}
