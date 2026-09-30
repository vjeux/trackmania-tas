//! `e7_envprobe --draw DRAW.json | --draws DRAWS.json.gz --eid N  --depth ENV_DEPTH.dds[.gz] [--colour ENV_COLOUR.dds[.gz]] [--flip-y]
//!   x,y,z [x,y,z …]` — THE GAME'S ENV LAYER AT A RECEIVER'S PIXEL (E7, 2026-09-30): the captured direction's camera (the draw's
//! SceneV GbxV_WorldToCamera + GbxV_CameraProjection, row-vector convention: cam = p·M + t, clip = cam·P) projects each world point
//! to its render-target pixel and its own z01; the banked env layer's depth (R16, z01: 0 = the far plane = the dome, larger =
//! nearer the camera) and colour at that pixel say what the receiver's direction reads there — the DOME (0), or an env surface
//! NEARER the camera than the receiver (z_env > z_r: PS 17112 discards → the direction gives nothing, RE 16 22:35Z), or one
//! FARTHER (z_env ≤ z_r: the receiver takes its colour). Calibrate the raster's orientation with a point in the decoration's
//! footprint hole (the dome) and one far outside it (the skirt).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let draw: serde_json::Value = if let Some(p) = f("--draw") {
        serde_json::from_str(&std::fs::read_to_string(&p).expect("draw json")).expect("json")
    } else if let (Some(p), Some(eid)) = (f("--draws"), f("--eid")) {
        let eid: u64 = eid.parse().expect("--eid N");
        let bytes = std::fs::read(&p).expect("draws");
        let txt = if p.ends_with(".gz") { String::from_utf8(lightmap::passdiff::gunzip(&bytes).expect("gunzip")).expect("utf8") } else { String::from_utf8(bytes).expect("utf8") };
        let txt = lightmap::passdiff::nan_free_json(&txt);
        let all: serde_json::Value = serde_json::from_str(&txt).expect("draws json");
        all.as_array().expect("array").iter().find(|d| d["eid"].as_u64() == Some(eid)).cloned().unwrap_or_else(|| panic!("eid {eid} not in {p}"))
    } else { eprintln!("usage: e7_envprobe (--draw D.json | --draws D.json.gz --eid N) --depth ENV_DEPTH.dds[.gz] [--colour C.dds[.gz]] [--flip-y] x,y,z …"); std::process::exit(2) };
    let sv = draw.pointer("/Vertex/cbuffers/SceneV").or_else(|| draw.pointer("/Pixel/cbuffers/SceneV")).expect("SceneV");
    let m: Vec<Vec<f64>> = sv["GbxV_WorldToCamera"].as_array().expect("WorldToCamera").iter().map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()).collect();
    let p4: Vec<Vec<f64>> = sv["GbxV_CameraProjection"].as_array().expect("CameraProjection").iter().map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()).collect();
    let eye: Vec<f64> = sv["GbxV_EyeInWorld"].as_array().map(|v| v.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default();
    let d = [m[0][2], m[1][2], m[2][2]];
    eprintln!("camera: d (col 2) = ({:.4}, {:.4}, {:.4}); eye {:?}; projection diag ({:.3e}, {:.3e}, {:.3e}) + ({:.4}, {:.4}, {:.4})", d[0], d[1], d[2], eye, p4[0][0], p4[1][1], p4[2][2], p4[3][0], p4[3][1], p4[3][2]);
    let load = |p: &str| -> lightmap::passdiff::Buf {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let bytes = if p.ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
        lightmap::passdiff::load_dds_bytes(&bytes, "", 0, 0).unwrap_or_else(|e| panic!("{p}: {e}"))
    };
    let load_as = |p: &str, fmt: &str| -> lightmap::passdiff::Buf {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        let bytes = if p.ends_with(".gz") { lightmap::passdiff::gunzip(&bytes).expect("gunzip") } else { bytes };
        lightmap::passdiff::load_dds_bytes(&bytes, fmt, 0, 0).unwrap_or_else(|e| panic!("{p}: {e}"))
    };
    // the depth: RenderDoc exports the R16 depth buffer's bits under an R16_FLOAT header (a typeless view) — the bits are the
    // UNORM16 z01 (--depth-format to say otherwise)
    let depth = load_as(&f("--depth").expect("--depth"), &f("--depth-format").unwrap_or_else(|| "R16_UNORM".into()));
    let colour = f("--colour").map(|p| load(&p));
    let flip_y = a.iter().any(|x| x == "--flip-y");
    let (w, h) = (depth.w, depth.h);
    eprintln!("env depth {}×{} ({} ch); colour {}", w, h, depth.channels, colour.as_ref().map(|c| format!("{}×{} ({} ch)", c.w, c.h, c.channels)).unwrap_or_else(|| "-".into()));
    println!("point (x, y, z)\tpixel\tz01_receiver\tz01_env\tverdict\tenv colour");
    for arg in a.iter().skip(1).filter(|s| s.contains(',') && !s.starts_with("--")) {
        let v: Vec<f64> = arg.split(',').map(|x| x.trim().parse().expect("x,y,z")).collect();
        if v.len() != 3 { continue; }
        let cam = [
            v[0] * m[0][0] + v[1] * m[1][0] + v[2] * m[2][0] + m[3][0],
            v[0] * m[0][1] + v[1] * m[1][1] + v[2] * m[2][1] + m[3][1],
            v[0] * m[0][2] + v[1] * m[1][2] + v[2] * m[2][2] + m[3][2],
        ];
        let clip = [cam[0] * p4[0][0] + p4[3][0], cam[1] * p4[1][1] + p4[3][1], cam[2] * p4[2][2] + p4[3][2]];
        let px = ((1.0 + clip[0]) * 0.5 * w as f64).floor();
        let mut py = ((1.0 - clip[1]) * 0.5 * h as f64).floor();
        if flip_y { py = h as f64 - 1.0 - py; }
        let inside = px >= 0.0 && py >= 0.0 && px < w as f64 && py < h as f64;
        if !inside { println!("({:.0}, {:.0}, {:.0})\t({px:.0}, {py:.0}) OUTSIDE\t{:.4}\t-\t-\t-", v[0], v[1], v[2], clip[2]); continue; }
        let (xi, yi) = (px as u32, py as u32);
        let ze = depth.get(xi, yi, 0) as f64;
        let verdict = if ze == 0.0 { "DOME (far plane)" } else if ze > clip[2] { "env NEARER the camera than the receiver → discarded (nothing)" } else { "env FARTHER → the receiver takes its colour" };
        let col = colour.as_ref().map(|c| format!("({:.4}, {:.4}, {:.4})", c.get(xi, yi, 0), c.get(xi, yi, 1), c.get(xi, yi, 2))).unwrap_or_else(|| "-".into());
        println!("({:.0}, {:.0}, {:.0})\t({xi}, {yi})\t{:.4}\t{:.4}\t{verdict}\t{col}", v[0], v[1], v[2], clip[2], ze);
    }
    // the census: how much of the layer is dome / env at all
    let (mut n0, mut n) = (0usize, 0usize);
    for y in 0..h { for x in 0..w { n += 1; if depth.get(x, y, 0) == 0.0 { n0 += 1; } } }
    eprintln!("layer census: {n0} of {n} pixels at the far plane (dome) = {:.2} %", 100.0 * n0 as f64 / n as f64);
}
