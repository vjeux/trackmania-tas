//! `tinyctl stand-hitbox-draw --pak F:KEY --kind StandStraight --out F.png [--pad-below 0.3 --thick 1.5]`
//! — Hugo's Manslaughter stands (2026-10-18): the stand's collision geometry as measured on the
//! pack prefab (the seating in blue, the walls / frame / roof in grey) and the finish slab the
//! carrier item carries (red hatched), side view (depth z → right, y up) and plan view (x right,
//! z up, the block cell outline), with the numbers and the placement convention.
use crate::flagdraw::{Canvas, DIM, INK, SLAB};

const SEAT: [u8; 3] = [40, 120, 200];
const SHELL: [u8; 3] = [150, 150, 150];
const CELL: [u8; 3] = [90, 90, 90];

fn f(rest: &[String], k: &str) -> Option<String> {
    rest.iter().position(|a| a == k).and_then(|i| rest.get(i + 1).cloned())
}

pub fn cmd(rest: &[String]) -> Result<(), String> {
    let kind = f(rest, "--kind").unwrap_or_else(|| "StandStraight".into());
    let out = f(rest, "--out").unwrap_or_else(|| format!("{}-hitbox.png", kind.to_lowercase()));
    let pad: f32 = f(rest, "--pad-below").and_then(|v| v.parse().ok()).unwrap_or(0.3);
    let thick: f32 = f(rest, "--thick").and_then(|v| v.parse().ok()).unwrap_or(1.5);
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
    let prefab = mapgeom::stands::prefab_of(&kind).ok_or_else(|| format!("{kind}: not a stand kind"))?;
    let mut all = Vec::new();
    mapgeom::stands::prefab_triangles(&mut store, prefab, &mapgeom::geom::IDENTITY, 0, &mut all)?;
    let seating = mapgeom::stands::seating_of(&mut store, prefab)?;
    let (sv, st) = mapgeom::stands::slab_of(&seating, pad, thick);
    let heights = mapgeom::stands::tread_heights(&seating);
    let (w, h) = (1600usize, 1200usize);
    let mut cv = Canvas::new(w, h);
    cv.text(60.0, 20.0, &format!("{}  -  FINISH HITBOX VS THE STAND (SEATING ONLY)", kind.to_uppercase()), 4, INK);
    // ---- SIDE view (left): z (the stand's depth, front → back) right, y up; the block is 32 × 24 × 32
    let (x0, y0) = (90.0f32, 880.0f32);
    let sc = 24.0f32; // px per metre
    let sx = |z: f32| x0 + (z + 1.0) * sc;
    let sy = |y: f32| y0 - (y + 1.0) * sc;
    cv.text(x0, 70.0, "SIDE VIEW: Z RIGHT (BACK WALL AT Z 0, TRACK SIDE AT Z 32), Y UP", 2, INK);
    cv.text(x0, 90.0, "DASHED = THE BLOCK CELL, 32 X 24 X 32 M", 2, INK);
    cv.dashed(sx(0.0), sy(0.0), sx(32.0), sy(0.0), CELL, 1.5, 8.0);
    cv.dashed(sx(0.0), sy(24.0), sx(32.0), sy(24.0), CELL, 1.5, 8.0);
    cv.dashed(sx(0.0), sy(0.0), sx(0.0), sy(24.0), CELL, 1.5, 8.0);
    cv.dashed(sx(32.0), sy(0.0), sx(32.0), sy(24.0), CELL, 1.5, 8.0);
    // the shell (non-seat materials) as faint grey triangles, then the seating in blue
    for t in &all {
        let p: Vec<(f32, f32)> = t.v.iter().map(|v| (sx(v[2]), sy(v[1]))).collect();
        if t.material.eq_ignore_ascii_case(mapgeom::stands::SEAT_MATERIAL) {
            cv.poly(&p, SEAT, 0.35);
        } else {
            cv.poly(&p, SHELL, 0.06);
        }
    }
    // the slab (red hatched): its silhouette = every slab triangle projected, hatched lightly
    for t in &st {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = sv[i as usize]; (sx(v[2]), sy(v[1])) }).collect();
        cv.poly(&p, SLAB, 0.10);
    }
    for t in &st {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = sv[i as usize]; (sx(v[2]), sy(v[1])) }).collect();
        cv.outline(&p, SLAB, 0.8);
    }
    // dimensions
    let (zmin, zmax) = sv.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v[2]), b.max(v[2])));
    let (ymin, ymax) = sv.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v[1]), b.max(v[1])));
    cv.dim(sx(zmin), sy(ymax) - 40.0, sx(zmax), sy(ymax) - 40.0, &format!("SLAB Z {zmin:.2}..{zmax:.2} ({:.2} M)", zmax - zmin), DIM, (0.0, -24.0));
    cv.dim(sx(zmax) + 40.0, sy(ymin), sx(zmax) + 40.0, sy(ymax), &format!("Y {ymin:.2}..{ymax:.2}"), DIM, (-150.0, 0.0));
    let rise = if heights.len() > 1 { heights[1] - heights[0] } else { 0.0 };
    cv.text(60.0, 1070.0, &format!("SEATING = {} TREADS (MATERIAL TECHNICSSTEP), Y {:.2}..{:.2}, RISE {:.2} M PER ROW, 1.60 M DEEP TREADS", heights.len(), seating.y_lo, seating.y_hi, rise), 2, INK);
    cv.text(60.0, 1090.0, &format!("THE SLAB = THAT STAIRCASE SURFACE {pad} M DOWN AND {:.2} M UP ({thick} M THICK), CLOSED ALONG ITS OUTLINE", thick - pad), 2, INK);
    let mut yy = 0.0f32;
    while yy <= 24.0 {
        cv.line(sx(0.0) - 8.0, sy(yy), sx(0.0), sy(yy), [170, 170, 170], 1.0, 1.0);
        cv.text(sx(0.0) - 60.0, sy(yy) - 8.0, &format!("Y {yy:.0}"), 2, [150, 150, 150]);
        yy += 4.0;
    }
    // ---- PLAN view (right): x right, z UP (north up); the cell square
    let (px0, pz0) = (1040.0f32, 660.0f32);
    let ps = 16.0f32;
    let tx = |x: f32| px0 + (x + 1.0) * ps;
    let tz = |z: f32| pz0 - (z + 1.0) * ps;
    cv.text(px0, 70.0, "PLAN: X RIGHT, Z UP (BLOCK FRAME, DIR 0)", 2, INK);
    cv.outline(&[(tx(0.0), tz(0.0)), (tx(32.0), tz(0.0)), (tx(32.0), tz(32.0)), (tx(0.0), tz(32.0))], CELL, 1.5);
    for t in &all {
        if !t.material.eq_ignore_ascii_case(mapgeom::stands::SEAT_MATERIAL) {
            let p: Vec<(f32, f32)> = t.v.iter().map(|v| (tx(v[0]), tz(v[2]))).collect();
            cv.poly(&p, SHELL, 0.05);
        }
    }
    for t in &seating.tris {
        let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = seating.verts[i as usize]; (tx(v[0]), tz(v[2])) }).collect();
        cv.poly(&p, SEAT, 0.5);
    }
    // the slab footprint: the top skin (second half of the slab triangles), hatched + outlined
    let n = seating.verts.len() as u32;
    for t in &st {
        if t.iter().all(|&i| i >= n) {
            let p: Vec<(f32, f32)> = t.iter().map(|&i| { let v = sv[i as usize]; (tx(v[0]), tz(v[2])) }).collect();
            cv.hatch(&p, SLAB, 10.0, 0.8);
            cv.outline(&p, SLAB, 0.6);
        }
    }
    // the item origin + axes
    cv.line(tx(0.0), tz(0.0), tx(6.0), tz(0.0), DIM, 2.0, 1.0);
    cv.line(tx(0.0), tz(0.0), tx(0.0), tz(6.0), DIM, 2.0, 1.0);
    cv.text(tx(6.5), tz(0.0) - 6.0, "+X", 2, DIM);
    cv.text(tx(0.0) - 8.0, tz(6.0) - 22.0, "+Z", 2, DIM);
    cv.poly(&[(tx(0.0) - 5.0, tz(0.0) - 5.0), (tx(0.0) + 5.0, tz(0.0) - 5.0), (tx(0.0) + 5.0, tz(0.0) + 5.0), (tx(0.0) - 5.0, tz(0.0) + 5.0)], DIM, 1.0);
    cv.text(tx(0.0) + 10.0, tz(0.0) + 8.0, "ITEM ORIGIN = THE CELL CORNER (X 0, Z 0)", 2, DIM);
    let (xmin, xmax) = sv.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v[0]), b.max(v[0])));
    cv.dim(tx(xmin), tz(0.0) + 44.0, tx(xmax), tz(0.0) + 44.0, &format!("X {xmin:.2}..{xmax:.2}"), DIM, (0.0, 8.0));
    // ---- legend
    let (lx, ly) = (1010.0f32, 770.0f32);
    cv.outline(&[(lx - 16.0, ly - 16.0), (lx + 560.0, ly - 16.0), (lx + 560.0, ly + 300.0), (lx - 16.0, ly + 300.0)], [200, 200, 200], 1.5);
    cv.text(lx, ly, "LEGEND", 2, INK);
    cv.poly(&[(lx, ly + 30.0), (lx + 30.0, ly + 30.0), (lx + 30.0, ly + 46.0), (lx, ly + 46.0)], SEAT, 0.5);
    cv.text(lx + 40.0, ly + 32.0, "SEATING: THE STEP ROWS, TREADS + RISERS", 2, INK);
    cv.poly(&[(lx, ly + 60.0), (lx + 30.0, ly + 60.0), (lx + 30.0, ly + 76.0), (lx, ly + 76.0)], SHELL, 0.3);
    cv.text(lx + 40.0, ly + 62.0, "WALLS, FRAME, ROOF, TOP DECK: NOT A TRIGGER", 2, INK);
    cv.hatch(&[(lx, ly + 90.0), (lx + 30.0, ly + 90.0), (lx + 30.0, ly + 106.0), (lx, ly + 106.0)], SLAB, 8.0, 1.0);
    cv.outline(&[(lx, ly + 90.0), (lx + 30.0, ly + 90.0), (lx + 30.0, ly + 106.0), (lx, ly + 106.0)], SLAB, 2.0);
    cv.text(lx + 40.0, ly + 92.0, &format!("FINISH TRIGGER SLAB ({} TRIS, INVISIBLE)", st.len()), 2, INK);
    cv.text(lx, ly + 130.0, &format!("BBOX X {xmin:.2}..{xmax:.2}  Y {ymin:.2}..{ymax:.2}"), 2, INK);
    cv.text(lx, ly + 150.0, &format!("     Z {zmin:.2}..{zmax:.2}  (METRES, BLOCK FRAME)"), 2, INK);
    cv.text(lx, ly + 180.0, "PLACE THE CARRIER IN THE BLOCK FRAME, PIVOT 0:", 2, INK);
    cv.text(lx, ly + 200.0, "POS = CELL ORIGIN + (0,0)/(32,0)/(32,32)/(0,32)", 2, INK);
    cv.text(lx, ly + 220.0, "IN X,Z FOR DIR 0/1/2/3;  YAW = -DIR X 90 DEG", 2, INK);
    cv.text(lx, ly + 250.0, "A CAR TOUCHING ANY STEP, OR LANDING ON THE ROWS,", 2, INK);
    cv.text(lx, ly + 270.0, "CROSSES THE SLAB AND FINISHES.", 2, INK);
    cv.text(60.0, 1150.0, "MEASURED ON THE PACK PREFAB (STADIUM.PAK): COLLISION TRIANGLES PER MATERIAL. THE STAND'S PEOPLE (ITS SPECTATOR BLOCK) SIT ON THE BLUE ROWS.", 2, [120, 120, 120]);
    std::fs::write(&out, crate::png::encode(&cv.img)).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {out}: {w}x{h}; {kind}: {} treads y {:.2}..{:.2}; slab {} tris bbox [{xmin:.2} {ymin:.2} {zmin:.2}]..[{xmax:.2} {ymax:.2} {zmax:.2}]", heights.len(), seating.y_lo, seating.y_hi, st.len());
    Ok(())
}
