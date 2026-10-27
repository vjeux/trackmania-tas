//! `tinyctl expandable-draw --map F.Map.Gbx --pak F:KEY [--item GateCheckpointCenter32m] --out F.png`
//! — Hugo's There-and-Back fallback for `GateExpandableFinish` (the 1-cell finish segment that stacks):
//! every segment of the map (grid or free), its trigger slab (the pak's ExpandableCheckpoint_Trigger
//! shape) and visual pane in WORLD coordinates, with the proposed replacement item's trigger pane and
//! arch outline at the pose block-frame + (16, 0, 16), yaw 0 relative to the block — PLAN view (x
//! right, z up) and SIDE view along the gate line (gate-plane axis right, y up), so the item's trigger
//! visibly covers the ground opening of the bottom segment. Prints the poses and the coverage check.
use crate::flagdraw::{Canvas, DIM, INK, SLAB};
use mapgeom::geom::{apply, compose, Xform};

const ITEM: [u8; 3] = [40, 120, 200];
const SEG: [u8; 3] = [150, 150, 150];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

fn translate(t: [f32; 3]) -> Xform {
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, t[0], t[1], t[2]]
}

fn bbox(pts: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in pts {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

fn corners(lo: [f32; 3], hi: [f32; 3]) -> Vec<[f32; 3]> {
    let mut v = Vec::with_capacity(8);
    for &x in &[lo[0], hi[0]] {
        for &y in &[lo[1], hi[1]] {
            for &z in &[lo[2], hi[2]] {
                v.push([x, y, z]);
            }
        }
    }
    v
}

struct Seg {
    frame: Xform,
    label: String,
    trig: Vec<[f32; 3]>,
    pane: Vec<[f32; 3]>,
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let map = f(rest, "--map").ok_or("expandable-draw needs --map F.Map.Gbx")?;
    let item = f(rest, "--item").unwrap_or_else(|| "GateCheckpointCenter32m".into());
    let item_label = std::path::Path::new(&item).file_name().map(|s| s.to_string_lossy().trim_end_matches(".Item.Gbx").to_string()).unwrap_or_else(|| item.clone());
    let out = f(rest, "--out").unwrap_or_else(|| "expandable.png".into());
    let mut store = mapgeom::store::DataStore::empty();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--pak" {
            if let Some((p, k)) = rest.get(i + 1).and_then(|s| s.rsplit_once(':')) {
                store.add_pak(p, k)?;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    let m = tmmaps::map::MapFile::load(std::path::Path::new(&map));
    let collection = m.body_collections().map(|c| c[0].1).unwrap_or(26);
    let ground = tmmaps::map::ground_y(collection);
    // the segment's trigger slab and visual pane (the Air prefab = the bare pane), block frame
    let trig_local: Vec<[f32; 3]> = {
        let mm = store.load_model("Stadium\\Media\\Prefab\\Gate\\ExpandableCheckpoint_Trigger.Shape.Gbx")?;
        let g = mm.graph()?;
        match &g.root {
            Some(mapgeom::node::Node::Surface(s)) => s.meshes.iter().flat_map(|mm| mm.verts.iter().copied()).collect(),
            _ => return Err("trigger shape: no surface".into()),
        }
    };
    let pane_local: Vec<[f32; 3]> = {
        let loaded = store.load_model("Stadium\\Media\\Prefab\\Gate\\ExpandableFinish_Air.Prefab.Gbx")?;
        let mut c = mapgeom::geom::Collector::new(&mut store);
        c.model(&loaded, &mapgeom::geom::IDENTITY, 0);
        c.scene.groups.values().flat_map(|g| g.verts.iter().copied()).collect()
    };
    let (tl, th) = bbox(&trig_local);
    let (pl, ph) = bbox(&pane_local);
    println!("segment (block frame): trigger x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}; pane x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}", tl[0], th[0], tl[1], th[1], tl[2], th[2], pl[0], ph[0], pl[1], ph[1], pl[2], ph[2]);
    // the replacement item: visual vertices + trigger box, item frame — a pak gate item (centred on its origin: the
    // offset puts it on the pane) or a local crystal item file authored in the BLOCK frame (offset 0)
    let local_file = std::path::Path::new(&item).is_file();
    let (item_verts, item_trig): (Vec<[f32; 3]>, Vec<[f32; 3]>) = if local_file {
        let bytes = std::fs::read(&item).map_err(|e| format!("{item}: {e}"))?;
        let it = mapgeom::crystal::ItemCrystal::open(&bytes)?;
        let (mut g, mut t) = (Vec::new(), Vec::new());
        for l in &it.model.layers {
            match &l.kind {
                mapgeom::crystal_model::LayerKind::Geometry { crystal, .. } => g.extend(crystal.positions.iter().copied()),
                mapgeom::crystal_model::LayerKind::Trigger { crystal, .. } => t.extend(crystal.positions.iter().copied()),
                _ => {}
            }
        }
        if t.is_empty() {
            return Err(format!("{item}: no trigger layer"));
        }
        (g, t)
    } else {
        let item_path = format!("Stadium\\Items\\{item}.Item.Gbx");
        let loaded = store.load_model(&item_path)?;
        let mut c = mapgeom::geom::Collector::new(&mut store);
        c.model(&loaded, &mapgeom::geom::IDENTITY, 0);
        let verts: Vec<[f32; 3]> = c.scene.groups.values().flat_map(|g| g.verts.iter().copied()).collect();
        drop(c);
        let mg = mapgeom::static_item::build::pack_item_merged(&mut store, &item_path, 1.0, 26, 0, None)?;
        let trig = mg.trigger.as_ref().and_then(|sf| sf.surf.triangulate()).map(|(v, _)| v).ok_or("item has no trigger")?;
        (verts, trig)
    };
    let (il, ih) = bbox(&item_verts);
    let (itl, ith) = bbox(&item_trig);
    println!("{item_label} (item frame): visual x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}; trigger x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}", il[0], ih[0], il[1], ih[1], il[2], ih[2], itl[0], ith[0], itl[1], ith[1], itl[2], ith[2]);
    let offset = if local_file { [0.0, 0.0, 0.0] } else { [(tl[0] + th[0]) / 2.0, 0.0, (tl[2] + th[2]) / 2.0] };
    println!("proposed item pose in the block frame: offset ({:.3}, {:.3}, {:.3}), yaw +0 (same yaw as the block){}", offset[0], offset[1], offset[2], if local_file { " — a block-frame item" } else { "" });
    // every segment of the map
    let mut segs: Vec<Seg> = Vec::new();
    for b in m.blocks.iter().chain(m.baked.iter()) {
        if !b.name.eq_ignore_ascii_case("GateExpandableFinish") {
            continue;
        }
        let (frame, label) = match (b.free_pos, b.free_rot) {
            (Some(p), Some(r)) => (mapgeom::place::free(p, r), format!("free ({:.1}, {:.1}, {:.1}) yaw {:.2} pitch {:.2} roll {:.2}", p[0], p[1], p[2], r[0], r[1], r[2])),
            _ => {
                let c = b.coords();
                (mapgeom::place::grid_block(c, b.dir, (32.0, 32.0), ground), format!("cell ({}, {}, {}) dir {}", c.0, c.1, c.2, b.dir))
            }
        };
        segs.push(Seg { trig: trig_local.iter().map(|v| apply(&frame, *v)).collect(), pane: pane_local.iter().map(|v| apply(&frame, *v)).collect(), frame, label });
    }
    if segs.is_empty() {
        return Err("no GateExpandableFinish in this map".into());
    }
    // stacks: same frame translation in x/z and same rotation; bottom = lowest y
    let mut used = vec![false; segs.len()];
    let mut stacks: Vec<Vec<usize>> = Vec::new();
    for a in 0..segs.len() {
        if used[a] {
            continue;
        }
        let mut grp = vec![a];
        used[a] = true;
        for b in a + 1..segs.len() {
            if used[b] {
                continue;
            }
            let fa = &segs[a].frame;
            let fb = &segs[b].frame;
            let same_rot = (0..9).all(|k| (fa[k] - fb[k]).abs() < 1e-4);
            let same_xz = (fa[9] - fb[9]).abs() < 0.01 && (fa[11] - fb[11]).abs() < 0.01;
            if same_rot && same_xz {
                grp.push(b);
                used[b] = true;
            }
        }
        grp.sort_by(|x, y| segs[*x].frame[10].partial_cmp(&segs[*y].frame[10]).unwrap());
        stacks.push(grp);
    }
    println!("{} segments in {} stacks (same x/z and rotation)", segs.len(), stacks.len());
    // item poses: a pak gate item goes once per stack at its bottom segment; a block-frame item goes on EVERY segment
    let mut items: Vec<(Xform, Vec<[f32; 3]>, Vec<[f32; 3]>)> = Vec::new();
    let targets: Vec<(usize, Vec<usize>)> = if local_file { (0..segs.len()).map(|k| (k, vec![k])).collect() } else { stacks.iter().map(|st| (st[0], st.clone())).collect() };
    let mut ok = 0usize;
    for (si, (bi, st)) in targets.iter().enumerate() {
        let bottom = &segs[*bi];
        let pose = compose(&bottom.frame, &translate(offset));
        let tw: Vec<[f32; 3]> = item_trig.iter().map(|v| apply(&pose, *v)).collect();
        let vw: Vec<[f32; 3]> = item_verts.iter().map(|v| apply(&pose, *v)).collect();
        let (sl, sh) = bbox(&bottom.trig);
        let (ql, qh) = bbox(&tw);
        let covered = (0..3).all(|k| ql[k] <= sl[k] + 0.05 && qh[k] >= sh[k] - 0.05);
        let yaw = (pose[2]).atan2(pose[0]);
        if covered { ok += 1; }
        println!("{} {}: {} segments, bottom {}; item origin ({:.3}, {:.3}, {:.3}) yaw {:.4} rad; segment trigger {:?}..{:?}; item trigger {:?}..{:?}; ground opening covered: {}", if local_file { "segment" } else { "stack" }, si + 1, st.len(), bottom.label, pose[9], pose[10], pose[11], yaw, [sl[0], sl[1], sl[2]].map(|v| (v * 100.0).round() / 100.0), [sh[0], sh[1], sh[2]].map(|v| (v * 100.0).round() / 100.0), [ql[0], ql[1], ql[2]].map(|v| (v * 100.0).round() / 100.0), [qh[0], qh[1], qh[2]].map(|v| (v * 100.0).round() / 100.0), if covered { "YES" } else { "NO" });
        items.push((pose, tw, vw));
    }
    println!("coverage: {ok} of {} {} pass the box test", targets.len(), if local_file { "segments" } else { "stacks" });
    // ---- drawing: PLAN (top), SIDE along the first stack's gate line (bottom)
    let all: Vec<[f32; 3]> = segs.iter().flat_map(|s| s.pane.iter().copied()).chain(items.iter().flat_map(|(_, _, v)| v.iter().copied())).collect();
    let (wl, wh) = bbox(&all);
    let (w, h) = (1800usize, 1300usize);
    let mut cv = Canvas::new(w, h);
    cv.text(40.0, 16.0, &format!("{} -- GATEEXPANDABLEFINISH SEGMENTS (GREY) AND {} AT BLOCK + ({:.0}, {:.0}, {:.0}) (BLUE); TRIGGERS HATCHED", std::path::Path::new(&map).file_name().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default(), item_label.to_uppercase(), offset[0], offset[1], offset[2]), 2, INK);
    let margin = 6.0f32;
    let span_x = (wh[0] - wl[0]).max(40.0) + 2.0 * margin;
    let span_z = (wh[2] - wl[2]).max(40.0) + 2.0 * margin;
    let plan_h = 600.0f32;
    let scale_plan = ((w as f32 - 120.0) / span_x).min(plan_h / span_z);
    let px = |x: f32| 60.0 + (x - wl[0] + margin) * scale_plan;
    let pz = |z: f32| 60.0 + plan_h - (z - wl[2] + margin) * scale_plan;
    cv.text(60.0, 44.0, "PLAN VIEW (WORLD X RIGHT, WORLD Z UP)", 2, DIM);
    for s in &segs {
        let (l, hh) = bbox(&s.pane);
        cv.outline(&[(px(l[0]), pz(l[2])), (px(hh[0]), pz(l[2])), (px(hh[0]), pz(hh[2])), (px(l[0]), pz(hh[2]))], SEG, 2.0);
        let (tl2, th2) = bbox(&s.trig);
        cv.hatch(&[(px(tl2[0]), pz(tl2[2])), (px(th2[0]), pz(tl2[2])), (px(th2[0]), pz(th2[2])), (px(tl2[0]), pz(th2[2]))], SEG, 6.0, 1.0);
    }
    for (pose, tw, vw) in &items {
        let (l, hh) = bbox(vw);
        cv.outline(&[(px(l[0]), pz(l[2])), (px(hh[0]), pz(l[2])), (px(hh[0]), pz(hh[2])), (px(l[0]), pz(hh[2]))], ITEM, 2.0);
        let (tl2, th2) = bbox(tw);
        cv.hatch(&[(px(tl2[0]), pz(tl2[2])), (px(th2[0]), pz(tl2[2])), (px(th2[0]), pz(th2[2])), (px(tl2[0]), pz(th2[2]))], ITEM, 4.0, 1.5);
        cv.line(px(pose[9]) - 6.0, pz(pose[11]), px(pose[9]) + 6.0, pz(pose[11]), SLAB, 2.0, 1.0);
        cv.line(px(pose[9]), pz(pose[11]) - 6.0, px(pose[9]), pz(pose[11]) + 6.0, SLAB, 2.0, 1.0);
    }
    // SIDE view of the first stack: axis along the gate line = the bottom segment's local x
    let st = &stacks[0];
    let bottom = &segs[st[0]];
    let ax = [bottom.frame[0], bottom.frame[1], bottom.frame[2]];
    let proj = |p: [f32; 3]| -> (f32, f32) { ((p[0] - bottom.frame[9]) * ax[0] + (p[1] - bottom.frame[10]) * ax[1] + (p[2] - bottom.frame[11]) * ax[2], p[1]) };
    let mut side_pts: Vec<(f32, f32)> = Vec::new();
    for &k in st {
        side_pts.extend(segs[k].pane.iter().map(|p| proj(*p)));
    }
    let side_set: Vec<usize> = if local_file { st.clone() } else { vec![0] };
    for k in &side_set { side_pts.extend(items[*k].2.iter().map(|p| proj(*p))); }
    let (mut ul, mut uh, mut vl, mut vh) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    for (u, v) in &side_pts {
        ul = ul.min(*u);
        uh = uh.max(*u);
        vl = vl.min(*v);
        vh = vh.max(*v);
    }
    let side_top = 60.0 + plan_h + 70.0;
    let side_h = h as f32 - side_top - 60.0;
    let scale_side = ((w as f32 - 120.0) / (uh - ul + 2.0 * margin)).min(side_h / (vh - vl + 2.0 * margin));
    let su = |u: f32| 60.0 + (u - ul + margin) * scale_side;
    let sv = |v: f32| side_top + side_h - (v - vl + margin) * scale_side;
    cv.text(60.0, side_top - 26.0, &format!("SIDE VIEW OF STACK 1 ({} SEGMENTS, {}): ALONG THE GATE LINE RIGHT, Y UP", st.len(), bottom.label.to_uppercase()), 2, DIM);
    for &k in st {
        let pts: Vec<(f32, f32)> = segs[k].pane.iter().map(|p| proj(*p)).collect();
        let (mut a, mut b, mut c, mut d) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
        for (u, v) in &pts {
            a = a.min(*u);
            b = b.max(*u);
            c = c.min(*v);
            d = d.max(*v);
        }
        cv.outline(&[(su(a), sv(c)), (su(b), sv(c)), (su(b), sv(d)), (su(a), sv(d))], SEG, 2.0);
        let tp: Vec<(f32, f32)> = segs[k].trig.iter().map(|p| proj(*p)).collect();
        let (mut a, mut b, mut c, mut d) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
        for (u, v) in &tp {
            a = a.min(*u);
            b = b.max(*u);
            c = c.min(*v);
            d = d.max(*v);
        }
        cv.hatch(&[(su(a), sv(c)), (su(b), sv(c)), (su(b), sv(d)), (su(a), sv(d))], SEG, 8.0, 1.0);
    }
    for (ii, k) in side_set.iter().enumerate() {
        let (pose, tw, vw) = &items[*k];
        // the arch as its true silhouette: every visual triangle edge would be too much; the hull of its vertices per column
        let pts: Vec<(f32, f32)> = vw.iter().map(|p| proj(*p)).collect();
        let (mut a, mut b, mut c, mut d) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
        for (u, v) in &pts {
            a = a.min(*u);
            b = b.max(*u);
            c = c.min(*v);
            d = d.max(*v);
        }
        cv.outline(&[(su(a), sv(c)), (su(b), sv(c)), (su(b), sv(d)), (su(a), sv(d))], ITEM, 2.0);
        // posts and beam: columns of the arch mesh at 0.5 m
        let cols = ((b - a) / 0.5).ceil().max(1.0) as usize;
        let mut col_lo = vec![f32::INFINITY; cols];
        for (u, v) in &pts {
            let k = (((u - a) / 0.5).floor() as usize).min(cols - 1);
            col_lo[k] = col_lo[k].min(*v);
        }
        for k in 0..cols {
            if col_lo[k].is_finite() {
                let u0 = a + k as f32 * 0.5;
                cv.line(su(u0), sv(col_lo[k]), su(u0 + 0.5), sv(col_lo[k]), ITEM, 2.0, 0.9);
            }
        }
        let tp: Vec<(f32, f32)> = tw.iter().map(|p| proj(*p)).collect();
        let (mut a, mut b, mut c, mut d) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
        for (u, v) in &tp {
            a = a.min(*u);
            b = b.max(*u);
            c = c.min(*v);
            d = d.max(*v);
        }
        cv.hatch(&[(su(a), sv(c)), (su(b), sv(c)), (su(b), sv(d)), (su(a), sv(d))], ITEM, 5.0, 1.5);
        let (ou, ov) = proj([pose[9], pose[10], pose[11]]);
        cv.line(su(ou) - 8.0, sv(ov), su(ou) + 8.0, sv(ov), SLAB, 2.0, 1.0);
        cv.line(su(ou), sv(ov) - 8.0, su(ou), sv(ov) + 8.0, SLAB, 2.0, 1.0);
        if ii == 0 {
            cv.dim(su(a), sv(c) + 14.0, su(b), sv(c) + 14.0, &format!("ITEM TRIGGER {:.2} M WIDE, {:.2} M TALL{}", b - a, d - c, if side_set.len() > 1 { format!(", ONE PER SEGMENT ({})", side_set.len()) } else { String::new() }), DIM, (0.0, 14.0));
        }
    }
    cv.text(60.0, h as f32 - 40.0, "GREY OUTLINE = SEGMENT PANE (32 X 8 X 0.84 M, PLANE AT LOCAL Z 16); GREY HATCH = SEGMENT TRIGGER (Y 0..7.18); BLUE = THE ITEM (VISUAL BOX, ARCH UNDERSIDE, TRIGGER PANE Y 0..7.30); RED CROSS = ITEM ORIGIN", 2, INK);
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}");
    Ok(())
}
