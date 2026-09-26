//! THE PEEL FRUSTUM VS THE CASTERS (study; port engineer G, 2026-09-26). RE 13's read of the game's fitted-peel camera
//! (render 0x140a4fab0 l.985–1000): after the cell box's fit (CHmsVolumeShadow::UpdateFrustum) the frustum is REFIT per
//! visible caster — the light-space AABB of each caster (FUN_140183fd0 / FUN_140185f70: c = R·(centre − eye), h = |R|·half),
//! near = min(near, c_z − h_z), far = max(far, c_z + h_z) — so the depth follows the casters the cell sees, not the box;
//! his 18 captured (camera, direction) rows (client-re/re13/stsun-peel-frusta.tsv) show the depth TIGHTER and the lateral
//! extents WIDER than the box's AABB, always symmetric about the cell centre, near = −h·(1 + 1e-4), far = (h + 5)·(1 + 1e-4).
//!
//! `LMTOOL_FRUSTUM_TSV=FILE` on a bake runs this after the peel plan: for every row the plan box the eye belongs to, the
//! box's fit, and the casters' union under several caster sets — the record scene's instances by class (items / blocks /
//! clips / tiles), with or without the zone tiles, all of them or only those whose lateral light-space AABB meets the
//! box's — each as (h_z, h_x, h_y) about the eye, symmetric (max |extent|) and asymmetric (min / max), against the game's
//! (−MinZ, px, py). The rule that reproduces the 18 rows is the one to transcribe into lightcam.

use crate::lightcam::{basis_game_opts, Aabb, GameRenorm};
use crate::tiledpeel::PeelPlan;

struct Row {
    eye: [f32; 3],
    min_z: f32,
    max_z: f32,
    px: f32,
    py: f32,
    r: [[f32; 3]; 3],
}

fn parse_v3(s: &str) -> Option<[f32; 3]> {
    let v: Vec<f32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
    if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None }
}

fn read_rows(path: &str) -> Vec<Row> {
    let txt = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut out = Vec::new();
    for line in txt.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 || f[0].starts_with('#') || f[0] == "frame" {
            continue;
        }
        let (Some(eye), Some(r0), Some(r1), Some(r2)) = (parse_v3(f[1]), parse_v3(f[6]), parse_v3(f[7]), parse_v3(f[8])) else { continue };
        let (Ok(min_z), Ok(max_z), Ok(px), Ok(py)) = (f[2].trim().parse(), f[3].trim().parse(), f[4].trim().parse(), f[5].trim().parse()) else { continue };
        out.push(Row { eye, min_z, max_z, px, py, r: [r0, r1, r2] });
    }
    out
}

/// A caster: its world AABB and a class tag.
struct Caster {
    min: [f32; 3],
    max: [f32; 3],
    kind: u8, // 0 item, 1 block/clip entity, 2 zone tile, 3 decoration
    name: String,
}

fn casters_of(scene: &crate::geometry::Scene) -> Vec<Caster> {
    let mut out = Vec::with_capacity(scene.instances.len());
    for inst in &scene.instances {
        let m = &scene.models[inst.model];
        let mut b = Aabb::empty();
        for t in &m.tris {
            for p in &t.p {
                b.add_point(mapgeom::geom::apply(&inst.xf, *p));
            }
        }
        if b.is_empty() {
            continue;
        }
        let kind = if inst.item < scene.item_count { 0 } else if inst.model_name.contains("#tile") || inst.model_name.contains("Zone\\") { 2 } else { 1 };
        out.push(Caster { min: b.min, max: b.max, kind, name: inst.model_name.clone() });
    }
    for d in &scene.decor {
        let mut b = Aabb::empty();
        for p in &d.p {
            b.add_point(*p);
        }
        out.push(Caster { min: b.min, max: b.max, kind: 3, name: "decor".into() });
    }
    out
}

/// The light-space AABB of a world box about `eye` in the basis (r, u, f): centre c = R·(centre − eye), half h = |R|·half.
fn light_aabb(min: [f32; 3], max: [f32; 3], eye: [f32; 3], basis: &[[f32; 3]; 3]) -> ([f32; 3], [f32; 3]) {
    let centre = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5];
    let half = [(max[0] - min[0]) * 0.5, (max[1] - min[1]) * 0.5, (max[2] - min[2]) * 0.5];
    let d = [centre[0] - eye[0], centre[1] - eye[1], centre[2] - eye[2]];
    let mut c = [0f32; 3];
    let mut h = [0f32; 3];
    for k in 0..3 {
        let a = basis[k];
        c[k] = a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
        h[k] = a[0].abs() * half[0] + a[1].abs() * half[1] + a[2].abs() * half[2];
    }
    (c, h)
}

pub fn run(tsv: &str, plan: &PeelPlan, scene: &crate::geometry::Scene, _dirs: &[[f32; 3]]) {
    let rows = read_rows(tsv);
    let casters = casters_of(scene);
    let n_kind = |k: u8| casters.iter().filter(|c| c.kind == k).count();
    eprintln!("frustum study: {} rows; casters {} (items {}, entities {}, tiles {}, decor {})", rows.len(), casters.len(), n_kind(0), n_kind(1), n_kind(2), n_kind(3));
    let boxes: Vec<(&str, Aabb)> = std::iter::once(("world", plan.world)).chain(plan.tiles.iter().enumerate().map(|(i, t)| (if i == 0 { "tile 0" } else if i == 1 { "tile 1" } else { "tile n" }, *t))).collect();
    for (ri, row) in rows.iter().enumerate() {
        // the plan box whose centre is the eye
        let (bname, bx) = boxes.iter().min_by(|a, b| { let da = dist2(a.1.centre_from_half(), row.eye); let db = dist2(b.1.centre_from_half(), row.eye); da.partial_cmp(&db).unwrap() }).unwrap();
        // the TSV's R0..R2 are GbxV_WorldToCamera's ROWS (column k = the k-th axis): axis k = (R0[k], R1[k], R2[k])
        let axis = |k: usize| [row.r[0][k], row.r[1][k], row.r[2][k]];
        let f = axis(2);
        let (r_ours, u_ours, f_ours) = basis_game_opts(f, GameRenorm::None);
        let basis_game = [axis(0), axis(1), axis(2)];
        let basis_dev = ((r_ours[0] - basis_game[0][0]).abs() + (r_ours[2] - basis_game[0][2]).abs() + (u_ours[1] - basis_game[1][1]).abs() + (f_ours[0] - basis_game[2][0]).abs()).max(0.0);
        let game_h = [-row.min_z, row.px, row.py];
        // (a) the box's own light-space AABB about the eye
        let (bc, bh) = light_aabb(bx.min, bx.max, row.eye, &basis_game);
        eprintln!("row {ri}: {bname} eye ({:.2}, {:.2}, {:.2}) f ({:.4}, {:.4}, {:.4}) [our basis dev {basis_dev:.1e}]  GAME h_z {:.2} (MinZ {:.3} MaxZ {:.3}) px {:.2} py {:.2}", row.eye[0], row.eye[1], row.eye[2], f[0], f[1], f[2], game_h[0], row.min_z, row.max_z, game_h[1], game_h[2]);
        eprintln!("   box fit: c ({:.2}, {:.2}, {:.2}) h ({:.2}, {:.2}, {:.2}) → hz {:.2} hx {:.2} hy {:.2}", bc[0], bc[1], bc[2], bh[0], bh[1], bh[2], bh[2], bh[0], bh[1]);
        // (b) the casters' union under several sets
        let sets: [(&str, Box<dyn Fn(&Caster, &([f32; 3], [f32; 3])) -> bool>); 6] = [
            ("all casters", Box::new(|_, _| true)),
            ("no zone tiles", Box::new(|c, _| c.kind != 2)),
            ("items + entities only", Box::new(|c, _| c.kind == 0 || c.kind == 1)),
            ("lateral-visible (meets the box's lateral AABB)", Box::new(move |_, l| (l.0[0] - l.1[0]) <= bc[0] + bh[0] && (l.0[0] + l.1[0]) >= bc[0] - bh[0] && (l.0[1] - l.1[1]) <= bc[1] + bh[1] && (l.0[1] + l.1[1]) >= bc[1] - bh[1])),
            ("lateral-visible, no zone tiles", Box::new(move |c, l| c.kind != 2 && (l.0[0] - l.1[0]) <= bc[0] + bh[0] && (l.0[0] + l.1[0]) >= bc[0] - bh[0] && (l.0[1] - l.1[1]) <= bc[1] + bh[1] && (l.0[1] + l.1[1]) >= bc[1] - bh[1])),
            ("world-XZ inside the box (any y)", Box::new(move |c, _| c.max[0] >= bx.min[0] && c.min[0] <= bx.max[0] && c.max[2] >= bx.min[2] && c.min[2] <= bx.max[2])),
        ];
        for (sname, keep) in sets.iter() {
            let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
            let mut n = 0usize;
            for c in &casters {
                let l = light_aabb(c.min, c.max, row.eye, &basis_game);
                if !keep(c, &l) {
                    continue;
                }
                n += 1;
                for k in 0..3 {
                    mn[k] = mn[k].min(l.0[k] - l.1[k]);
                    mx[k] = mx[k].max(l.0[k] + l.1[k]);
                }
            }
            if n == 0 {
                eprintln!("   {sname}: no casters");
                continue;
            }
            let sym = [mn[0].abs().max(mx[0].abs()), mn[1].abs().max(mx[1].abs()), mn[2].abs().max(mx[2].abs())];
            let mark = |ours: f32, game: f32| -> String { let d = ours / game - 1.0; if d.abs() < 2e-3 { format!("{ours:.2} ✓") } else { format!("{ours:.2} ({:+.1} %)", d * 100.0) } };
            eprintln!("   {sname} ({n}): z [{:.2}, {:.2}] x [{:.2}, {:.2}] y [{:.2}, {:.2}] → symmetric hz {} hx {} hy {}", mn[2], mx[2], mn[0], mx[0], mn[1], mx[1], mark(sym[2], game_h[0]), mark(sym[0], game_h[1]), mark(sym[1], game_h[2]));
        }
        // the largest single casters along each axis, for the eye
        let mut far: Vec<(f32, &str, u8)> = casters.iter().map(|c| { let l = light_aabb(c.min, c.max, row.eye, &basis_game); ((l.0[2] - l.1[2]).abs().max((l.0[2] + l.1[2]).abs()), c.name.as_str(), c.kind) }).collect();
        far.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        eprintln!("   deepest casters along z: {}", far.iter().take(4).map(|(d, n, k)| format!("{d:.1} {n} (kind {k})")).collect::<Vec<_>>().join("; "));
    }
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}
