//! `tinyctl unproject --view "ox,oy,oz,dist,h,v" --px X,Y [--size 1920,1080] [--fov 85]
//!                    [--ground Y] [--anchor sx,sy,sz:tx,ty,tz [--scale 0.5]] [--side o|t]`
//!
//! Where in the world is a pixel of a comparison shot? Reviewing Summer 24's
//! top view showed a slab in the water that no census row explained; finding
//! its coordinates by hand (frame fraction → metres, twice) took forty
//! minutes. This does the arithmetic.
//!
//! The camera is the editor's orbital camera as `shootctl shootset` drives it
//! (the views file's columns): target `o`, distance `dist`, angles `h` and `v`
//! in radians. h = 0 puts the camera NORTH of the target (−z) looking +z, so
//! +x appears on the LEFT of the frame and the far side is the top; v > 0
//! looks down (1.3 ≈ top-down). The pixel's ray is intersected with the
//! horizontal plane y = `--ground` (default: the target's y — the deck the
//! camera was aimed at) and printed as a world point.
//!
//! `--side t` treats the view as the TINY side sees it: the target through
//! the anchor (`tmmaps tiny` prints it; `tinyctl views` puts it in the header)
//! and the distance × scale, exactly what `shootset --anchor` does — the
//! answer is then in tiny-map coordinates, and the source point it came from
//! is printed too. `--side o` (default) is the original side, source
//! coordinates. `--px` takes pixels of the full frame (`--size`, default
//! 1920×1080 — a `cmp-*.jpg` sheet halves each side: double what you measure
//! there) or fractions of the frame when both values are ≤ 1.
//!
//! The one unknown is the horizontal field of view: 85° reproduces Summer
//! 24's island extent in its top view to within a few percent; refine it with
//! `--fov` once a shot of a surveyed object says otherwise. The sign of `h`
//! for side views is taken from the h = 0 convention rotated about +y; it has
//! only been checked at h = 0 and h = ±π/2 by eye.

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn nums(s: &str, n: usize, what: &str) -> Result<Vec<f64>, String> {
    let v: Vec<f64> = s.split(|c| c == ',' || c == ':').map(|t| t.trim().parse::<f64>().map_err(|_| format!("{what}: `{t}` is not a number in `{s}`"))).collect::<Result<_, _>>()?;
    if v.len() != n {
        return Err(format!("{what}: expected {n} numbers, got {} in `{s}`", v.len()));
    }
    Ok(v)
}

/// The orbital camera's frame: (position, forward, right, up).
pub fn camera(target: [f64; 3], dist: f64, h: f64, v: f64) -> ([f64; 3], [f64; 3], [f64; 3], [f64; 3]) {
    // horizontal unit vector from the target towards the camera: north (−z) at h = 0
    let toward = [-h.sin(), 0.0, -h.cos()];
    let cam = [target[0] + dist * v.cos() * toward[0], target[1] + dist * v.sin(), target[2] + dist * v.cos() * toward[2]];
    let f = norm([target[0] - cam[0], target[1] - cam[1], target[2] - cam[2]]);
    let r = norm(cross(f, [0.0, 1.0, 0.0]));
    let u = cross(r, f);
    (cam, f, r, u)
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

/// The world point where the ray through pixel (px, py) of a w×h frame meets
/// the plane y = ground; None when the ray does not descend to it.
pub fn unproject(target: [f64; 3], dist: f64, h: f64, v: f64, hfov_deg: f64, size: (f64, f64), px: (f64, f64), ground: f64) -> Option<[f64; 3]> {
    let (cam, f, r, u) = camera(target, dist, h, v);
    let th = (hfov_deg.to_radians() / 2.0).tan();
    let tv = th * size.1 / size.0;
    let xn = 2.0 * px.0 / size.0 - 1.0;
    let yn = 1.0 - 2.0 * px.1 / size.1;
    let d = [f[0] + xn * th * r[0] + yn * tv * u[0], f[1] + xn * th * r[1] + yn * tv * u[1], f[2] + xn * th * r[2] + yn * tv * u[2]];
    let dy = ground - cam[1];
    if (dy < 0.0) != (d[1] < 0.0) || d[1].abs() < 1e-9 {
        return None;
    }
    let t = dy / d[1];
    Some([cam[0] + t * d[0], ground, cam[2] + t * d[2]])
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let view = flag(args, "--view").ok_or("unproject needs --view \"ox,oy,oz,dist,h,v\" (a views-file row without its name)")?;
    let vv = nums(&view, 6, "--view")?;
    let (mut target, mut dist, h, v) = ([vv[0], vv[1], vv[2]], vv[3], vv[4], vv[5]);
    let size = match flag(args, "--size") {
        Some(s) => {
            let n = nums(&s, 2, "--size")?;
            (n[0], n[1])
        }
        None => (1920.0, 1080.0),
    };
    let px = nums(&flag(args, "--px").ok_or("unproject needs --px X,Y (pixels of the full frame, or fractions ≤ 1)")?, 2, "--px")?;
    let px = if px[0] <= 1.0 && px[1] <= 1.0 { (px[0] * size.0, px[1] * size.1) } else { (px[0], px[1]) };
    let fov: f64 = flag(args, "--fov").map(|s| s.parse::<f64>().map_err(|_| format!("--fov `{s}` is not a number"))).transpose()?.unwrap_or(85.0);
    let scale: f64 = flag(args, "--scale").map(|s| s.parse::<f64>().map_err(|_| format!("--scale `{s}` is not a number"))).transpose()?.unwrap_or(0.5);
    let side = flag(args, "--side").unwrap_or_else(|| "o".into());
    let anchor = flag(args, "--anchor").map(|a| nums(&a, 6, "--anchor")).transpose()?;
    // the tiny side: the camera through the anchor, as shootset --anchor drives it
    let to_tiny = |p: [f64; 3], a: &[f64]| [a[3] + (p[0] - a[0]) * scale, a[4] + (p[1] - a[1]) * scale, a[5] + (p[2] - a[2]) * scale];
    let to_source = |p: [f64; 3], a: &[f64]| [a[0] + (p[0] - a[3]) / scale, a[1] + (p[1] - a[4]) / scale, a[2] + (p[2] - a[5]) / scale];
    if side == "t" {
        let a = anchor.as_ref().ok_or("--side t needs --anchor sx,sy,sz:tx,ty,tz")?;
        target = to_tiny(target, a);
        dist *= scale;
    } else if side != "o" {
        return Err(format!("--side is o or t, not `{side}`"));
    }
    let ground: f64 = match flag(args, "--ground") {
        Some(g) => g.parse::<f64>().map_err(|_| format!("--ground `{g}` is not a number"))?,
        None => target[1],
    };
    let (cam, f, _, _) = camera(target, dist, h, v);
    println!("camera {:.1},{:.1},{:.1} looking {:.3},{:.3},{:.3} at target {:.1},{:.1},{:.1} (side {side}, dist {dist:.1}, hfov {fov}°, frame {}x{})", cam[0], cam[1], cam[2], f[0], f[1], f[2], target[0], target[1], target[2], size.0, size.1);
    match unproject(target, dist, h, v, fov, size, px, ground) {
        None => println!("pixel {:.0},{:.0}: the ray never reaches y = {ground:.1} (above the horizon)", px.0, px.1),
        Some(p) => {
            println!("pixel {:.0},{:.0} -> {:.1},{:.1},{:.1} on the plane y = {ground:.1} ({} coordinates)", px.0, px.1, p[0], p[1], p[2], if side == "t" { "tiny-map" } else { "source" });
            if let Some(a) = anchor.as_ref() {
                let other = if side == "t" { to_source(p, a) } else { to_tiny(p, a) };
                println!("  = {:.1},{:.1},{:.1} in the {} map (through the anchor, scale {scale})", other[0], other[1], other[2], if side == "t" { "source" } else { "tiny" });
            }
            println!("  cell {},{} (32 m grid of that map)", (p[0] / 32.0).floor() as i64, (p[2] / 32.0).floor() as i64);
        }
    }
    Ok(())
}
