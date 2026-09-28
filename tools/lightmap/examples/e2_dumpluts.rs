// E2: the two ACosSmooth LUTs of the pwc-day capture (R16_UNORM 1024×1, eid 1028 t0 = 5457, t1 = 16813) as a Rust source file
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]).join("env/frame127448/textures");
    let mut out = String::from("//! The `TMapACosSmooth` / `TMapACosSmoothPy` LUTs of PS 16752 (shader-global 1024×1 R16_UNORM textures), read off the pwc-day\n//! capture's eid-1028 bindings (ids 5457 / 16813) by `e2_dumpluts` — E2 2026-09-28. Generated; do not edit.\n\n");
    for (name, id) in [("ACOS_SMOOTH", 5457u32), ("ACOS_SMOOTH_PY", 16813)] {
        let b = std::fs::read(dir.join(format!("e001028_{id}.dds"))).expect("dds");
        let t = lightmap::texsample::parse_dds(&b, lightmap::texsample::Bc1Decode::Ideal).expect("parse");
        let lv = &t.levels[0][0];
        out += &format!("pub const {name}: [f32; {}] = [\n", lv.w);
        for x in 0..lv.w { out += &format!("{:.7}, ", lv.get(x, 0)[0]); if x % 8 == 7 { out += "\n"; } }
        out += "];\n\n";
        eprintln!("{name}: {} values, first {:.5} mid {:.5} last {:.5}", lv.w, lv.get(0, 0)[0], lv.get(lv.w / 2, 0)[0], lv.get(lv.w - 1, 0)[0]);
    }
    std::fs::write(&a[2], out).unwrap();
}
