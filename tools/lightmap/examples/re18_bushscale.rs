//! `re18_bushscale MAP.Gbx MODEL_SUBSTR [scale_var01 rotxz randY]` — per placement (item index ii): the variation seed, the scale pick k,
//! the rotation draws, the yaw/pitch/roll, the cell — to find the kind-0 records' grouping key (RE 18, 2026-10-01).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mf = tmmaps::map::MapFile::load(std::path::Path::new(&a[1]));
    let want = &a[2];
    let params = mapgeom::veget_instance::TreeParams { scale_var01: a.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.1), angle_max_rot_xz_deg: a.get(4).and_then(|v| v.parse().ok()).unwrap_or(1.0), enable_random_rotation_y: a.get(5).map(|v| v == "1").unwrap_or(true) };
    println!("ii\tseed\tk\tscale\tyaw_draw\ttx\ttz\tyaw\tpitch\troll\tx\ty\tz\tcell32\tcell8y");
    for (ii, it) in mf.items.iter().enumerate() {
        if !it.model.contains(want.as_str()) { continue; }
        let (q1, t, seed) = mapgeom::veget_instance::item_pose(it.yaw, it.pitch, it.roll, it.pos, it.pivot);
        let inst = mapgeom::veget_instance::variation(q1, t, seed, params, true);
        let k = ((1.0 - inst.scale) / params.scale_var01 * 7.0).round() as u32;
        let (yd, tx, tz) = inst.rotation.unwrap_or((0.0, 0.0, 0.0));
        println!("{ii}\t{seed:#010x}\t{k}\t{:.5}\t{yd:.4}\t{tx:.5}\t{tz:.5}\t{:.4}\t{:.4}\t{:.4}\t{:.2}\t{:.2}\t{:.2}\t{},{}\t{}", inst.scale, it.yaw, it.pitch, it.roll, it.pos[0], it.pos[1], it.pos[2], (it.pos[0] / 32.0).floor(), (it.pos[2] / 32.0).floor(), ((it.pos[1] + 120.0) / 8.0).floor());
    }
}
