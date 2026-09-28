//! `re16_dxbccmp CAPTURED_SHADER.txt GPUCACHE_DUMP.txt [--blob K] [--show]` — does a RenderDoc-captured shader (the DXBC
//! disassembly `shaders-frame*/Pixel_N.txt`) match one of the DXBC blobs of a GpuCache dump (`shaders-all2/…hlsl.txt`, the
//! Jan-2026 cache's permutations)? Both texts are DXBC disassemblies; RenderDoc names resources/constants and the cache dump
//! uses raw registers, so the comparison is on the OPCODE SEQUENCE (every instruction's mnemonic, `_indexable(...)`/`_sat`
//! suffixes kept, `ret` included) and the declaration counts (samplers, textures, temps). A blob whose opcode sequence is
//! identical to the capture's is "the same program" to the precision this text allows; the RDEF of that blob then names the
//! ShaderP constants. RE 16 (2026-09-28): E3's question whether the cache's Block_PyPxz_X2H2_PeelDiff_Hue_p blob 0 is the Feb
//! build's pre-pass program — answered by matching the captured PS 9517/9544 (f4468) against the cache's PyPxz(T)_X2H2 dumps.
fn opcode(line: &str) -> Option<String> {
    let t = line.trim();
    if t.is_empty() { return None; }
    // RenderDoc: "  21:   movc r4.xyz, …" — strip the "N:" prefix
    let body = if let Some((n, rest)) = t.split_once(':') { if n.trim().chars().all(|c| c.is_ascii_digit()) && !n.trim().is_empty() { rest.trim() } else { t } } else { t };
    let op = body.split_whitespace().next()?;
    if op.starts_with("dcl_") || op.starts_with("ps_") || op.starts_with("vs_") || op.starts_with("cs_") || op == "//" || op.starts_with("--") || op.starts_with("==") || op.starts_with("Shader") || op.starts_with("DXBC") { return None; }
    // drop the resource-type annotation of sample_indexable(texture2d)(float,…) and the `_indexable` decoration (RenderDoc prints it, the cache dump does not); keep _sat/_l/_c_lz
    let op = op.split('(').next().unwrap_or(op);
    let op = op.strip_suffix("_indexable").unwrap_or(op).to_string(); let op = op.replace("_indexable", "");
    Some(op)
}
fn dcl_counts(lines: &[&str]) -> (usize, usize, usize) {
    let mut s = 0; let mut t = 0; let mut temps = 0;
    for l in lines { let l = l.trim(); if l.starts_with("dcl_sampler") { s += 1 } else if l.starts_with("dcl_resource") { t += 1 } else if let Some(r) = l.strip_prefix("dcl_temps ") { temps = r.trim().parse().unwrap_or(0) } }
    (s, t, temps)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 { eprintln!("usage: re16_dxbccmp CAPTURED.txt GPUCACHE_DUMP.txt [--blob K] [--show]"); std::process::exit(2); }
    let cap = std::fs::read_to_string(&a[1]).expect("captured shader text");
    let dump = std::fs::read_to_string(&a[2]).expect("gpucache dump text");
    let only: Option<usize> = a.iter().position(|x| x == "--blob").and_then(|i| a.get(i + 1)).and_then(|v| v.parse().ok());
    let show = a.iter().any(|x| x == "--show");
    let cap_lines: Vec<&str> = cap.lines().collect();
    let cap_ops: Vec<String> = cap_lines.iter().filter_map(|l| opcode(l)).collect();
    let cap_dcl = dcl_counts(&cap_lines);
    let hash = cap_lines.iter().find(|l| l.starts_with("Shader hash")).map(|s| s.to_string()).unwrap_or_default();
    println!("{}: {} ({} instructions; samplers {} textures {} temps {})", a[1], hash, cap_ops.len(), cap_dcl.0, cap_dcl.1, cap_dcl.2);
    // split the dump into blobs: "== blob K at …" … "-- SHEX" … next "== blob"
    let mut blobs: Vec<(usize, Vec<&str>, Vec<&str>)> = Vec::new(); // (k, rdef lines, shex lines)
    let mut cur: Option<(usize, Vec<&str>, Vec<&str>, bool)> = None;
    for l in dump.lines() {
        if let Some(rest) = l.strip_prefix("== blob ") {
            if let Some((k, r, s, _)) = cur.take() { blobs.push((k, r, s)); }
            let k: usize = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(blobs.len());
            cur = Some((k, Vec::new(), Vec::new(), false));
            continue;
        }
        if let Some(c) = cur.as_mut() {
            if l.starts_with("-- SHEX") { c.3 = true; continue; }
            if l.starts_with("-- RDEF") { c.3 = false; continue; }
            if c.3 { c.2.push(l) } else { c.1.push(l) }
        }
    }
    if let Some((k, r, s, _)) = cur.take() { blobs.push((k, r, s)); }
    let mut best: Option<(usize, usize)> = None;
    for (k, rdef, shex) in &blobs {
        if let Some(o) = only { if *k != o { continue; } }
        let ops: Vec<String> = shex.iter().filter_map(|l| opcode(l)).collect();
        let dcl = dcl_counts(shex);
        let n = cap_ops.len().max(ops.len());
        let same = cap_ops.iter().zip(ops.iter()).filter(|(x, y)| x == y).count();
        let mism = n - same;
        let first = cap_ops.iter().zip(ops.iter()).position(|(x, y)| x != y);
        let sp: Vec<String> = { let mut v = Vec::new(); let mut in_sp = false; for l in rdef { let t = l.trim(); if t.starts_with("cbuffer ShaderP") { in_sp = true; continue; } if in_sp { if t.starts_with("cbuffer ") { in_sp = false; } else if let Some(p) = t.find('+') { let rest = &t[p..]; let name = rest.split_whitespace().nth(1).unwrap_or(""); if !name.is_empty() { v.push(name.to_string()); } } } } v };
        println!("  blob {k}: {} instructions (samplers {} textures {} temps {}), opcode mismatches {mism}{}; ShaderP [{}]", ops.len(), dcl.0, dcl.1, dcl.2, first.map(|i| format!(" (first at #{i}: {} vs {})", cap_ops[i], ops[i])).unwrap_or_default(), sp.join(", "));
        if show && mism == 0 { for (i, (x, y)) in cap_lines.iter().filter(|l| opcode(l).is_some()).zip(shex.iter().filter(|l| opcode(l).is_some())).enumerate() { println!("    {i:3} | {} | {}", x.trim(), y.trim()); } }
        if best.map(|(_, m)| mism < m).unwrap_or(true) { best = Some((*k, mism)); }
    }
    match best { Some((k, 0)) => println!("MATCH: blob {k} has the captured shader's exact opcode sequence"), Some((k, m)) => println!("NO exact match; closest blob {k} with {m} opcode mismatches"), None => println!("no blobs") }
}
