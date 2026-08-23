//! `tmmaps rotate` — tilt a FREE block in place, and refuse to tilt a quarter
//! of a surface.
//!
//! WHY (arm `wtr_`, 284238, 2026-08-22)
//! ------------------------------------
//! Every mover in this tool is position-only, on purpose: a model swap changes
//! the trigger volume and the origin control cannot see it. But the question
//! 284238 came down to is a question about a SURFACE'S SHAPE — its water
//! launcher is a shallow trough (−0.78 ° of roll per metre across the lane)
//! where the sibling map's tech deck is flat, and the car's roll and its lateral
//! position are therefore the same variable. Position-only surgery cannot ask
//! that question: sliding the trough sideways moves the car with it (measured:
//! 1.70 m buys 0.58 ° and nothing else), because a car rides the surface it is
//! on.
//!
//! What separates "the car's roll" from "the shape a tilted surface presents to
//! the kicker" is the opposite experiment: **tilt a flat deck under a run that
//! works.** That needs a rotation, and a rotation of a free block is not a
//! model swap — it is the three f32 immediately after the position in chunk
//! `0x0304305F`, the same field the origin control already replays for every
//! free block on every map. So the write path is covered; only the verb was
//! missing.
//!
//! THE REFUSAL THIS COMMAND EXISTS TO MAKE
//! ---------------------------------------
//! Two arms on 284238 ran "raise the kicker by 1.00 m" as their decisive
//! experiment and both got a null. The ice kicker is **four blocks** —
//! `PlatformIceLoopStartCurve0Out` + three `PlatformLoopStartCurve0*` — sharing
//! one anchor to the millimetre, of which exactly one is free-placed. Raising
//! "the kicker" raised a quarter of it and built a 1 m step: entry speed
//! 99.81 → 50.84 m/s. The experiment could not have worked, and nothing said so.
//!
//! So this command looks at what it is about to leave behind. Every free block
//! within `--group-radius` of a block being rotated, and not itself in the
//! list, is a REFUSAL that names it. `--allow-partial` overrides and prints the
//! blocks you are choosing to leave flat, because sometimes a partial tilt is
//! what you want — but never by accident.

//! WHAT A PER-BLOCK ROTATION IS NOT
//! --------------------------------
//! A block's stored rotation turns it about **its own anchor**. A surface made
//! of 32 m tiles is therefore NOT tilted by giving every tile the same roll: at
//! 3.4 ° each tile's far edge lifts 1.9 m above its neighbour's near edge, and
//! the deck becomes a staircase. Measured on 279008's launcher: the same tilt
//! applied per-block deflected the human's car from vz −25.29 to −8.28 and cost
//! it 3.4 m/s before it reached the kicker at all. It is the four-block trap
//! again in another dress — *the object you are rotating is not the object you
//! think you are rotating.*
//!
//! `--about X,Y,Z --dir DEG --angle RAD` is the honest version: one axis, and
//! every block in the group rotated about it — **position and rotation
//! together**. The position is exact (a rotation of the anchor about the axis
//! line); the orientation is the small-angle decomposition of a world tilt into
//! the block's own pitch and roll fields,
//!
//! ```text
//! droll = angle * cos(yaw_block - dir)      dpitch = angle * sin(yaw_block - dir)
//! ```
//!
//! which is exact at first order and is why the command prints the resulting
//! surface's own measured slope rather than asking you to trust it. Blocks in
//! one group may have different yaws — 279008's four deck tiles carry +30 °,
//! +30 °, −60 ° and +30 ° — and a decomposition is the only thing that tilts
//! them all the same way in the world.

use crate::gbx;
use crate::map::MapFile;
use std::path::{Path, PathBuf};

/// `BLK:a,b,c` — a rotation for one block, absolute or as a delta.
pub struct Spec {
    pub block: usize,
    pub v: [f32; 3],
}

pub fn parse_spec(s: &str) -> Spec {
    let (b, r) = s.split_once(':').unwrap_or_else(|| panic!("--rot/--drot want BLK:yaw,pitch,roll, got {:?}", s));
    let v: Vec<f32> = r.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    assert!(v.len() == 3, "--rot/--drot want three angles (yaw,pitch,roll), got {:?}", s);
    Spec { block: b.trim().parse().unwrap_or_else(|_| panic!("block index in {:?}", s)), v: [v[0], v[1], v[2]] }
}

/// Free blocks near `p` that are not in `chosen`. The four-block trap, as a list.
pub fn left_behind(m: &MapFile, chosen: &[usize], radius: f64) -> Vec<(usize, String, f64)> {
    let mut out = Vec::new();
    for &c in chosen {
        let pc = match m.blocks[c].free_pos {
            Some(p) => p,
            None => continue,
        };
        for b in &m.blocks {
            if chosen.contains(&b.index) {
                continue;
            }
            if let Some(p) = b.free_pos {
                let d = (((p[0] - pc[0]) as f64).powi(2)
                    + ((p[1] - pc[1]) as f64).powi(2)
                    + ((p[2] - pc[2]) as f64).powi(2))
                .sqrt();
                if d <= radius && !out.iter().any(|(i, _, _)| *i == b.index) {
                    out.push((b.index, b.name.clone(), d));
                }
            }
        }
    }
    out.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    out
}

/// Tilt a group of blocks about ONE world axis: the horizontal line through
/// `c` pointing at heading `dir` (radians, the same convention as a block's
/// yaw). Returns, per block, the new position and the new rotation triple.
///
/// Position is exact. Orientation is the first-order decomposition of the world
/// tilt into the block's own pitch and roll, which is why the caller measures
/// the result instead of trusting it.
pub fn tilt_about(
    blocks: &[(usize, [f32; 3], [f32; 3])],
    c: [f64; 3],
    dir: f64,
    angle: f64,
) -> Vec<(usize, [f32; 3], [f32; 3])> {
    // Unit vector along the axis, horizontal.
    let a = [dir.cos(), 0.0, dir.sin()];
    let (s, co) = (angle.sin(), angle.cos());
    let rot = |v: [f64; 3]| -> [f64; 3] {
        // Rodrigues about the unit axis `a`.
        let dot = a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
        let cross = [a[1] * v[2] - a[2] * v[1], a[2] * v[0] - a[0] * v[2], a[0] * v[1] - a[1] * v[0]];
        [
            v[0] * co + cross[0] * s + a[0] * dot * (1.0 - co),
            v[1] * co + cross[1] * s + a[1] * dot * (1.0 - co),
            v[2] * co + cross[2] * s + a[2] * dot * (1.0 - co),
        ]
    };
    blocks
        .iter()
        .map(|(i, p, r)| {
            let d = [p[0] as f64 - c[0], p[1] as f64 - c[1], p[2] as f64 - c[2]];
            let q = rot(d);
            let yaw = r[0] as f64;
            let np = [(c[0] + q[0]) as f32, (c[1] + q[1]) as f32, (c[2] + q[2]) as f32];
            let nr = [
                r[0],
                (r[1] as f64 + angle * (yaw - dir).sin()) as f32,
                (r[2] as f64 + angle * (yaw - dir).cos()) as f32,
            ];
            (*i, np, nr)
        })
        .collect()
}

pub fn cmd(args: &[String]) {
    let src = PathBuf::from(&args[2]);
    let out = PathBuf::from(crate::cli::flag(args, "--out").expect("--out F"));
    let abs: Vec<Spec> = crate::cli::flag_multi(args, "--rot").iter().map(|s| parse_spec(s)).collect();
    let del: Vec<Spec> = crate::cli::flag_multi(args, "--drot").iter().map(|s| parse_spec(s)).collect();
    // The common-axis form: `--tilt N,N,N --about X,Y,Z --dir DEG --angle RAD`.
    let tilt: Vec<usize> = crate::cli::flag_multi(args, "--tilt")
        .iter()
        .flat_map(|s| s.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or_else(|_| panic!("--tilt wants block indices, got {:?}", s)))
        .collect();
    assert!(
        !abs.is_empty() || !del.is_empty() || !tilt.is_empty(),
        "--rot BLK:yaw,pitch,roll | --drot BLK:dyaw,dpitch,droll | --tilt N,N,N --about X,Y,Z --dir DEG --angle RAD"
    );
    let radius: f64 = crate::cli::flag(args, "--group-radius")
        .and_then(|v| v.parse().ok())
        .unwrap_or(4.0);
    let allow_partial = crate::cli::has(args, "--allow-partial");

    let m0 = MapFile::load(&src);
    let chosen: Vec<usize> = abs.iter().chain(del.iter()).map(|s| s.block).chain(tilt.iter().copied()).collect();

    for &b in &chosen {
        assert!(b < m0.blocks.len(), "block#{} does not exist (map has {})", b, m0.blocks.len());
        assert!(
            m0.blocks[b].free_off.is_some(),
            "block#{} {} is a GRID block (flags {:08X}): it has no stored rotation, only a dir byte — \
             use `move --move {}:cx,cy,cz:dir`",
            b,
            m0.blocks[b].name,
            m0.blocks[b].flags,
            b
        );
    }

    // THE REFUSAL. A rotation applied to part of an assembly builds a step.
    let orphans = left_behind(&m0, &chosen, radius);
    if !orphans.is_empty() {
        println!("rotate: {} free block(s) within {} m of the rotation are NOT in it:", orphans.len(), radius);
        for (i, n, d) in &orphans {
            println!("  block#{:<5} {:<44} {:.3} m away", i, n, d);
        }
        if !allow_partial {
            println!(
                "REFUSED. Rotating part of an assembly builds a step, and it looks exactly like a \
                 null result: on 284238 the ice kicker is FOUR blocks sharing one anchor and two \
                 arms rotated one of them. Add them to the rotation, shrink --group-radius, or \
                 pass --allow-partial if a partial tilt is genuinely what you want."
            );
            std::process::exit(2);
        }
        println!("--allow-partial: leaving the blocks above where they are.");
    }

    // THE ORIGIN CONTROL, run here rather than trusted. Replay every chosen
    // block's OWN rotation and require the rebuilt body to be byte-identical.
    // On the decompressed body, never the file's sha256: LZO recompression is
    // not bit-reproducible.
    {
        let mut m = MapFile::load(&src);
        for &b in &chosen {
            m.set_block_free_rot(b, m0.blocks[b].free_rot.unwrap());
            m.move_block_free(b, m0.blocks[b].free_pos.unwrap());
        }
        let same = gbx::Gbx::parse(&m.build()).body == m0.gbx.body;
        println!("origin control (rewrite each block's own position AND rotation): body identical = {}", same);
        assert!(same, "the rotation write path does not reproduce the untouched body — refusing to write");
    }

    let mut m = MapFile::load(&src);
    if !tilt.is_empty() {
        let c: Vec<f64> = crate::cli::flag(args, "--about")
            .expect("--tilt needs --about X,Y,Z")
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        assert!(c.len() == 3, "--about wants X,Y,Z");
        let dir: f64 = crate::cli::flag(args, "--dir")
            .expect("--tilt needs --dir DEG (the heading of the tilt axis)")
            .parse::<f64>()
            .expect("--dir DEG")
            .to_radians();
        let angle: f64 = crate::cli::flag(args, "--angle")
            .expect("--tilt needs --angle RAD")
            .parse()
            .expect("--angle RAD");
        let src_blocks: Vec<(usize, [f32; 3], [f32; 3])> = tilt
            .iter()
            .map(|&i| (i, m0.blocks[i].free_pos.unwrap(), m0.blocks[i].free_rot.unwrap()))
            .collect();
        for (i, np, nr) in tilt_about(&src_blocks, [c[0], c[1], c[2]], dir, angle) {
            let (op, or) = (m0.blocks[i].free_pos.unwrap(), m0.blocks[i].free_rot.unwrap());
            m.move_block_free(i, np);
            m.set_block_free_rot(i, nr);
            println!(
                "  block#{} {}  pos {:?} -> {:?}   rot {:?} -> {:?}",
                i, m0.blocks[i].name, op, np, or, nr
            );
        }
        println!(
            "  tilted {} block(s) by {:.4} rad ({:.2} deg) about the axis through {:?} at heading {:.1} deg",
            tilt.len(),
            angle,
            angle.to_degrees(),
            c,
            dir.to_degrees()
        );
        println!(
            "  NOTE: the orientation is a FIRST-ORDER decomposition. Measure the resulting surface \
             (a car's roll across it) before quoting the tilt as achieved."
        );
    }
    for s in &abs {
        let home = m0.blocks[s.block].free_rot.unwrap();
        m.set_block_free_rot(s.block, s.v);
        println!(
            "  block#{} {} rot {:?} -> {:?}",
            s.block, m0.blocks[s.block].name, home, s.v
        );
    }
    for s in &del {
        let home = m0.blocks[s.block].free_rot.unwrap();
        let v = [home[0] + s.v[0], home[1] + s.v[1], home[2] + s.v[2]];
        m.set_block_free_rot(s.block, v);
        println!(
            "  block#{} {} rot {:?} + {:?} -> {:?}",
            s.block, m0.blocks[s.block].name, home, s.v, v
        );
    }
    m.write_to(&out).expect("write rotated map");
    println!("wrote {}", out.display());

    // Read it back and say what the file now claims, so a rotation that did not
    // land cannot be reported as one that did.
    let back = MapFile::load(&out);
    for &b in &chosen {
        println!(
            "  read back: block#{} {} pos {:?} rot {:?}",
            b,
            back.blocks[b].name,
            back.blocks[b].free_pos.unwrap(),
            back.blocks[b].free_rot.unwrap()
        );
    }
}

/// Zero delta, applied to any free block, must reproduce the body. This is the
/// same claim `controls::origin` makes for every free block on every map, and
/// it is asserted at the top of `cmd` on the blocks the user named — but the
/// suite runs it too, because a control that only runs when someone uses the
/// command is a control that is not run.
pub fn selftest_identity(src: &Path) -> bool {
    let m0 = MapFile::load(src);
    let mut m = MapFile::load(src);
    let mut n = 0;
    for b in &m0.blocks {
        if let Some(r) = b.free_rot {
            m.set_block_free_rot(b.index, r);
            n += 1;
        }
    }
    if n == 0 {
        return true;
    }
    gbx::Gbx::parse(&m.build()).body == m0.gbx.body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_parses_and_a_bad_one_is_refused() {
        let s = parse_spec("146:0.5236,0,0.07");
        assert_eq!(s.block, 146);
        assert!((s.v[0] - 0.5236).abs() < 1e-6 && (s.v[2] - 0.07).abs() < 1e-6);
        assert!(std::panic::catch_unwind(|| parse_spec("146:0.5,0")).is_err(), "two angles must be refused");
        assert!(std::panic::catch_unwind(|| parse_spec("146")).is_err(), "no colon must be refused");
    }

    // The property that makes a common-axis tilt a tilt rather than a shear:
    // a point ON the axis does not move, and two tiles 32 m apart ALONG the
    // axis rise by the same amount, so the join between them stays a join.
    // A per-block rotation fails exactly this test, which is why it turned a
    // deck into a staircase.
    #[test]
    fn a_common_axis_tilt_keeps_the_tiles_joined() {
        let c = [800.0, 1816.0, 690.0];
        let dir = 30f64.to_radians();
        let ang = 0.06;
        // two tiles 32 m apart along the axis, plus one on the axis itself
        let on = [800.0f32, 1816.0, 690.0];
        let a = [800.0 + 32.0 * (dir.cos() as f32), 1816.0, 690.0 + 32.0 * (dir.sin() as f32)];
        let b = [800.0 + 64.0 * (dir.cos() as f32), 1816.0, 690.0 + 64.0 * (dir.sin() as f32)];
        let r = [dir as f32, 0.0, 0.0];
        let out = tilt_about(&[(0, on, r), (1, a, r), (2, b, r)], c, dir, ang);
        for (i, p, _) in &out {
            let dy = p[1] - 1816.0;
            assert!(dy.abs() < 1e-3, "block {} lifted {} m: a point along the axis must not rise", i, dy);
        }
        // and the ROLL each tile is given is the same, because they share a yaw
        let rolls: Vec<f32> = out.iter().map(|(_, _, r)| r[2]).collect();
        assert!((rolls[0] - rolls[1]).abs() < 1e-6 && (rolls[1] - rolls[2]).abs() < 1e-6);
        assert!((rolls[0] as f64 - ang).abs() < 1e-6, "a tile whose yaw is the axis heading gets the whole tilt as ROLL");
    }

    // A tile at 90 deg to the axis must take the tilt as PITCH, not roll —
    // this is the case that a "just add the angle to the roll field" rotator
    // gets silently wrong, and 279008's deck has one such tile in four.
    #[test]
    fn a_crosswise_tile_takes_the_tilt_as_pitch() {
        let c = [0.0, 0.0, 0.0];
        let dir = 0.0;
        let ang = 0.05;
        let r = [std::f32::consts::FRAC_PI_2, 0.0, 0.0]; // yaw 90 deg
        let out = tilt_about(&[(0, [0.0, 0.0, 10.0], r)], c, dir, ang);
        let (_, p, nr) = out[0];
        assert!((nr[1] as f64 - ang).abs() < 1e-6, "pitch should take the tilt, got {:?}", nr);
        assert!(nr[2].abs() < 1e-6, "roll should be untouched, got {:?}", nr);
        // and 10 m off the axis it rises by 10*sin(ang)
        assert!((p[1] as f64 + 10.0 * ang.sin() - 0.0).abs() < 0.02 || (p[1] as f64 - 10.0 * ang.sin()).abs() < 0.02);
    }
}
