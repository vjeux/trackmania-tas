//! `tinyctl gate-align-draw --pak F:KEY [--checkpoint GateCheckpointLeft8m --special GateSpecial8mTurbo] --out F.png`
//! — Hugo's CPEffect (2026-10-18): the checkpoint gate and the effect gate that centre-aligns on it, in
//! PLAN view (local x right, local z up — the car passes along +z) and FRONT view (x right, y up), both
//! drawn from the pack items' visual vertices in the item frame, origins marked, the trigger extents
//! outlined — so the sideways offset (0: both arches are symmetric about their origin; the Left/Right
//! checkpoint's wall post sits INSIDE the arch) is visible.
use crate::flagdraw::{Canvas, DIM, INK, SLAB};

const CP: [u8; 3] = [40, 120, 200];
const SP: [u8; 3] = [230, 140, 30];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

struct Geo {
    verts: Vec<[f32; 3]>,
    tris: Vec<[usize; 3]>,
    trig: Option<([f32; 3], [f32; 3])>,
}

fn geo(store: &mut mapgeom::store::DataStore, stem: &str) -> Result<Geo, String> {
    let p = format!("Stadium\\Items\\{stem}.Item.Gbx");
    let loaded = store.load_model(&p)?;
    let mut c = mapgeom::geom::Collector::new(store);
    c.model(&loaded, &mapgeom::geom::IDENTITY, 0);
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    for g in c.scene.groups.values() {
        let base = verts.len();
        verts.extend(g.verts.iter().copied());
        for t in &g.tris {
            tris.push([base + t[0] as usize, base + t[1] as usize, base + t[2] as usize]);
        }
    }
    drop(c);
    let mg = mapgeom::static_item::build::pack_item_merged(store, &p, 1.0, 26, 0, None)?;
    let trig = mg.trigger.as_ref().or(mg.special.as_ref()).and_then(|sf| sf.surf.triangulate()).map(|(v, _)| {
        let (mut l, mut h) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for p in &v {
            for k in 0..3 {
                l[k] = l[k].min(p[k]);
                h[k] = h[k].max(p[k]);
            }
        }
        (l, h)
    });
    Ok(Geo { verts, tris, trig })
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let cp = f(rest, "--checkpoint").unwrap_or_else(|| "GateCheckpointLeft8m".into());
    let sp = f(rest, "--special").unwrap_or_else(|| "GateSpecial8mTurbo".into());
    let dx: f32 = f(rest, "--dx").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let out = f(rest, "--out").unwrap_or_else(|| format!("{}-align.png", cp.to_lowercase()));
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
    let gc = geo(&mut store, &cp)?;
    let gs = geo(&mut store, &sp)?;
    let (w, h) = (1600usize, 1200usize);
    let mut cv = Canvas::new(w, h);
    cv.text(60.0, 20.0, &format!("{}  +  {}  (OFFSET DX {dx:.2} M)", cp.to_uppercase(), sp.to_uppercase()), 4, INK);
    let half = gc.verts.iter().map(|v| v[0].abs()).fold(0.0f32, f32::max).max(gs.verts.iter().map(|v| v[0].abs() + dx.abs()).fold(0.0f32, f32::max)) + 1.5;
    // ---- FRONT view (top): x right, y up
    let (fx0, fy0) = (800.0f32, 660.0f32);
    let fs = (700.0 / half).min(50.0);
    let fx = |x: f32| fx0 + x * fs;
    let fy = |y: f32| fy0 - y * fs;
    cv.text(60.0, 70.0, "FRONT VIEW (LOOKING ALONG +Z): X RIGHT, Y UP", 2, INK);
    for t in &gs.tris {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = gs.verts[i]; (fx(v[0] + dx), fy(v[1])) }).collect();
        cv.poly(&p, SP, 0.12);
    }
    for t in &gc.tris {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = gc.verts[i]; (fx(v[0]), fy(v[1])) }).collect();
        cv.poly(&p, CP, 0.12);
    }
    if let Some((l, hh)) = gs.trig {
        cv.hatch(&[(fx(l[0] + dx), fy(l[1])), (fx(hh[0] + dx), fy(l[1])), (fx(hh[0] + dx), fy(hh[1])), (fx(l[0] + dx), fy(hh[1]))], SLAB, 18.0, 0.6);
        cv.outline(&[(fx(l[0] + dx), fy(l[1])), (fx(hh[0] + dx), fy(l[1])), (fx(hh[0] + dx), fy(hh[1])), (fx(l[0] + dx), fy(hh[1]))], SLAB, 2.0);
        cv.text(fx(l[0] + dx), fy(0.0) + 34.0, &format!("EFFECT TRIGGER X {:.2}..{:.2} Y {:.2}..{:.2}", l[0] + dx, hh[0] + dx, l[1], hh[1]), 2, SLAB);
    }
    if let Some((l, hh)) = gc.trig {
        cv.outline(&[(fx(l[0]), fy(l[1])), (fx(hh[0]), fy(l[1])), (fx(hh[0]), fy(hh[1])), (fx(l[0]), fy(hh[1]))], CP, 3.0);
        cv.text(fx(l[0]), fy(hh[1]) - 22.0, &format!("CP TRIGGER X {:.2}..{:.2} Y {:.2}..{:.2}", l[0], hh[0], l[1], hh[1]), 2, CP);
        // the wall post: the arch span beyond the trigger
        let span = gc.verts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v[0]), b.max(v[0])));
        if hh[0] < span.1 - 0.5 {
            cv.text(fx(hh[0]), fy(3.5), &format!("WALL POST X {:.2}..{:.2}", hh[0], span.1), 2, CP);
        }
        if l[0] > span.0 + 0.5 {
            cv.text(fx(span.0), fy(3.5), &format!("WALL POST X {:.2}..{:.2}", span.0, l[0]), 2, CP);
        }
    }
    // origins
    cv.line(fx(0.0), fy(-1.0), fx(0.0), fy(11.5), DIM, 1.5, 1.0);
    cv.text(fx(0.0) + 8.0, fy(11.5), "X 0 = BOTH ORIGINS", 2, DIM);
    if dx != 0.0 {
        cv.line(fx(dx), fy(-1.5), fx(dx), fy(10.0), SP, 1.5, 1.0);
    }
    let mut xx = -half.floor();
    while xx <= half {
        cv.line(fx(xx), fy(0.0) + 4.0, fx(xx), fy(0.0) - 4.0, [150, 150, 150], 1.0, 1.0);
        if (xx as i32) % 2 == 0 {
            cv.text(fx(xx) - 10.0, fy(0.0) + 10.0, &format!("{xx:.0}"), 2, [120, 120, 120]);
        }
        xx += 1.0;
    }
    // ---- PLAN view (bottom): x right, z up
    let (px0, pz0) = (800.0f32, 880.0f32);
    let ps = fs;
    let px = |x: f32| px0 + x * ps;
    let pz = |z: f32| pz0 - z * ps;
    cv.text(60.0, 760.0, "PLAN VIEW: X RIGHT, Z UP (THE CAR PASSES ALONG +Z)", 2, INK);
    for t in &gc.tris {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = gc.verts[i]; (px(v[0]), pz(v[2])) }).collect();
        cv.poly(&p, CP, 0.25);
    }
    for t in &gs.tris {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = gs.verts[i]; (px(v[0] + dx), pz(v[2])) }).collect();
        cv.poly(&p, SP, 0.25);
    }
    if let Some((l, hh)) = gc.trig {
        cv.outline(&[(px(l[0]), pz(l[2])), (px(hh[0]), pz(l[2])), (px(hh[0]), pz(hh[2])), (px(l[0]), pz(hh[2]))], CP, 2.5);
    }
    if let Some((l, hh)) = gs.trig {
        cv.hatch(&[(px(l[0] + dx), pz(l[2])), (px(hh[0] + dx), pz(l[2])), (px(hh[0] + dx), pz(hh[2])), (px(l[0] + dx), pz(hh[2]))], SLAB, 10.0, 0.8);
        cv.outline(&[(px(l[0] + dx), pz(l[2])), (px(hh[0] + dx), pz(l[2])), (px(hh[0] + dx), pz(hh[2])), (px(l[0] + dx), pz(hh[2]))], SLAB, 2.0);
        cv.text(px(hh[0] + dx) + 12.0, pz(hh[2]) - 6.0, &format!("EFFECT TRIGGER Z {:.2}..{:.2}", l[2], hh[2]), 2, SLAB);
    }
    if let Some((l, hh)) = gc.trig {
        cv.text(px(hh[0]) + 12.0, pz(l[2]) + 10.0, &format!("CP TRIGGER Z {:.2}..{:.2}", l[2], hh[2]), 2, CP);
    }
    cv.poly(&[(px(0.0) - 5.0, pz(0.0) - 5.0), (px(0.0) + 5.0, pz(0.0) - 5.0), (px(0.0) + 5.0, pz(0.0) + 5.0), (px(0.0) - 5.0, pz(0.0) + 5.0)], DIM, 1.0);
    cv.text(px(0.0) + 10.0, pz(0.0) + 30.0, "ORIGIN (0, 0) OF BOTH ITEMS", 2, DIM);
    cv.line(px(0.0), pz(0.0), px(0.0), pz(3.5), DIM, 2.0, 1.0);
    cv.text(px(0.0) - 8.0, pz(3.5) - 24.0, "+Z", 2, DIM);
    cv.line(px(0.0), pz(0.0), px(3.5), pz(0.0), DIM, 2.0, 1.0);
    cv.text(px(3.5) + 6.0, pz(0.0) - 8.0, "+X", 2, DIM);
    // legend
    let (lx, ly) = (60.0f32, 980.0f32);
    cv.poly(&[(lx, ly), (lx + 30.0, ly), (lx + 30.0, ly + 16.0), (lx, ly + 16.0)], CP, 0.4);
    cv.text(lx + 40.0, ly + 2.0, &format!("{} (CHECKPOINT): VISUAL; THICK BLUE = ITS TRIGGER", cp.to_uppercase()), 2, INK);
    cv.poly(&[(lx, ly + 30.0), (lx + 30.0, ly + 30.0), (lx + 30.0, ly + 46.0), (lx, ly + 46.0)], SP, 0.4);
    cv.text(lx + 40.0, ly + 32.0, &format!("{} (EFFECT GATE) AT DX {dx:.2}: VISUAL; RED HATCH = ITS TRIGGER", sp.to_uppercase()), 2, INK);
    cv.text(lx, ly + 70.0, "BOTH ARCHES ARE SYMMETRIC ABOUT THEIR ORIGIN (X +-W/2). THE WALL POST OF A LEFT/RIGHT CHECKPOINT", 2, INK);
    cv.text(lx, ly + 90.0, "STANDS INSIDE THE ARCH (LEFT: X +W/2-1.42..+W/2, RIGHT: -W/2..-W/2+1.42) AND ITS TRIGGER STOPS AT THE POST.", 2, INK);
    cv.text(lx, ly + 110.0, "SO THE EFFECT GATE OF THE SAME WIDTH GOES AT OFFSET (0, 0, 0), SAME YAW. ALL NUMBERS IN METRES, ITEM FRAME.", 2, INK);
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}");
    Ok(())
}
