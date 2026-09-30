//! The scaled reference set: the original map's cartographer pack + route
//! mapped through the tiny transform `p' = A' + k·(p − A)`, and the control
//! that says the transform is right — the scaled gate centres must land on the
//! tiny map's own waypoint items.
//!
//! Priors from the full-scale map are HYPOTHESES on the tiny map (decision 2):
//! this module only moves geometry; nothing in it says the scaled line is
//! drivable, and every arm that uses it races it against an alternative.

use tmtraj::json::{parse, J};

#[derive(Clone, Copy, Debug)]
pub struct Xform {
    pub anchor: [f64; 3],
    pub anchor_to: [f64; 3],
    pub k: f64,
}

impl Xform {
    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        [
            self.anchor_to[0] + self.k * (p[0] - self.anchor[0]),
            self.anchor_to[1] + self.k * (p[1] - self.anchor[1]),
            self.anchor_to[2] + self.k * (p[2] - self.anchor[2]),
        ]
    }
    /// Lengths (arc length, corridor width) scale by `k` alone.
    pub fn len(&self, s: f64) -> f64 {
        self.k * s
    }
}

pub fn parse_xyz(s: &str) -> Result<[f64; 3], String> {
    let v: Vec<f64> = s
        .split(',')
        .map(|t| t.trim().parse::<f64>().map_err(|_| format!("bad number in {s:?}")))
        .collect::<Result<_, _>>()?;
    if v.len() != 3 {
        return Err(format!("{s:?}: want x,y,z"));
    }
    Ok([v[0], v[1], v[2]])
}

fn num3(j: &J) -> [f64; 3] {
    let a = j.arr();
    [a[0].num(), a[1].num(), a[2].num()]
}

fn fmt3(p: [f64; 3]) -> String {
    format!("[{:.3}, {:.3}, {:.3}]", p[0], p[1], p[2])
}

fn fmt_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Emit `j` as JSON, transforming every array named `pos`/`p`/`spawn` by the
/// point transform and every `s`/`w`/`length_m`/`gate_s`/`stations` by the
/// length scale. Directions (`start_dir`, `gate_dir_tour_order`, `yaw`) are
/// invariant under a uniform scale about a point. `uid`/`name` at the top
/// level are replaced by the tiny map's.
pub fn scale_json(j: &J, x: &Xform, key: Option<&str>, top: bool, uid: &str, name: &str, out: &mut String) {
    match j {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Num(v) => {
            let v = match key {
                Some("s") | Some("w") | Some("length_m") => x.len(*v),
                _ => *v,
            };
            out.push_str(&tmtraj::json::fmt_g(v, 9));
        }
        J::Str(s) => {
            let s = match (top, key) {
                (true, Some("uid")) => uid,
                (true, Some("name")) => name,
                _ => s.as_str(),
            };
            out.push_str(&fmt_str(s));
        }
        J::Arr(a) => match key {
            Some("pos") | Some("p") | Some("spawn") if a.len() == 3 && a.iter().all(|v| matches!(v, J::Num(_))) => {
                out.push_str(&fmt3(x.apply(num3(j))));
            }
            Some("gate_s") | Some("stations") => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&format!("{:.3}", x.len(v.num())));
                }
                out.push(']');
            }
            _ => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    // Elements of an array inherit no key: a `checkpoints`
                    // element is an object whose own `pos` is transformed.
                    scale_json(v, x, None, false, uid, name, out);
                }
                out.push(']');
            }
        },
        J::Obj(kv) => {
            out.push('{');
            for (i, (k, v)) in kv.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&fmt_str(k));
                out.push(':');
                scale_json(v, x, Some(k.as_str()), top, uid, name, out);
            }
            out.push('}');
        }
    }
}

pub struct GateControlRow {
    pub tag: String,
    pub scaled_centre: [f64; 3],
    pub item: Option<(usize, String, [f64; 3], f64)>,
    /// Distance from the scaled centre to the item's CENTRE (origin + rotated
    /// half-footprint), metres.
    pub residual_m: Option<f64>,
}

/// Where a tiny waypoint item's footprint centre is, from its placement.
///
/// `tmmaps tiny` places a converted block's item at the block's rotated corner
/// with the block's yaw, and the half-scale footprint is `half` metres square
/// (16 m for a 32 m block at k = 0.5). Centre = origin + R(yaw)·(half, 0, half).
/// `yaw_sign` is the rotation convention (+1 or −1); the control that calls this
/// tries both and reports which one lands, so the convention is measured, not
/// assumed.
pub fn item_centre(origin: [f32; 3], yaw: f32, half: f64, yaw_sign: f64) -> [f64; 3] {
    let t = yaw as f64 * yaw_sign;
    let (s, c) = t.sin_cos();
    let (lx, lz) = (half, half);
    [
        origin[0] as f64 + lx * c + lz * s,
        origin[1] as f64,
        origin[2] as f64 - lx * s + lz * c,
    ]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The transform control: every gate centre of the ORIGINAL pack, scaled,
/// against the tiny map's own waypoint items of the same tag. Horizontal
/// residuals are what the control decides on (the item's `pos.y` is its base;
/// the deck sits above it by the item's own height and is not read here).
fn nearest_item(
    wps: &[tmmaps::map::Waypoint],
    tag: &str,
    centre: [f64; 3],
    footprint: Option<(f64, f64)>,
) -> Option<(usize, String, [f64; 3], f64)> {
    let mut best: Option<(usize, String, [f64; 3], f64)> = None;
    for w in wps.iter().filter(|w| w.kind == tmmaps::map::Kind::Item && w.tag == tag) {
        let o = w.pos.unwrap_or([0.0; 3]);
        let c = match footprint {
            Some((half, sign)) => item_centre(o, w.yaw.unwrap_or(0.0), half, sign),
            None => [o[0] as f64, o[1] as f64, o[2] as f64],
        };
        let d = ((c[0] - centre[0]).powi(2) + (c[2] - centre[2]).powi(2)).sqrt();
        if best.as_ref().map(|b| d < b.3).unwrap_or(true) {
            best = Some((w.index, w.name.clone(), c, d));
        }
    }
    best
}

pub fn gate_control(
    pack: &J,
    x: &Xform,
    tiny: &tmmaps::map::MapFile,
    half: f64,
    yaw_sign: f64,
) -> Vec<GateControlRow> {
    let wps = tiny.waypoints();
    let mut rows = Vec::new();
    let mut row = |tag: String, centre: [f64; 3], footprint: Option<(f64, f64)>| GateControlRow {
        item: nearest_item(&wps, tag.split(' ').next().unwrap_or(""), centre, footprint),
        residual_m: nearest_item(&wps, tag.split(' ').next().unwrap_or(""), centre, footprint).map(|b| b.3),
        scaled_centre: centre,
        tag,
    };
    if let Some(sp) = pack.get("spawn") {
        // The pack's spawn is the CELL BASE of the start block; its footprint
        // centre horizontally is the same point.
        rows.push(row("Spawn".into(), x.apply(num3(sp)), Some((half, yaw_sign))));
    }
    for (tag, key) in [("Checkpoint", "checkpoints"), ("Goal", "finish")] {
        if let Some(list) = pack.get(key) {
            for cp in list.arr() {
                for g in cp.get("gates").map(|g| g.arr()).unwrap_or(&[]) {
                    let p = num3(g.get("pos").expect("gate pos"));
                    let from_item = g.get("from_item").map(|v| matches!(v, J::Bool(true))).unwrap_or(false);
                    let centre = x.apply(p);
                    if from_item {
                        // An original ITEM gate carries an absolute position;
                        // the tiny map keeps it as an item at the scaled point
                        // (no footprint offset).
                        rows.push(row(format!("{tag} (item gate)"), centre, None));
                    } else {
                        rows.push(row(tag.to_string(), centre, Some((half, yaw_sign))));
                    }
                }
            }
        }
    }
    let _ = dist;
    rows
}

pub fn load_json(p: &std::path::Path) -> Result<J, String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {}", p.display(), e))?;
    parse(t.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn x() -> Xform {
        Xform {
            anchor: [1584.0, 16.0, 784.0],
            anchor_to: [1584.0, 11.5, 784.0],
            k: 0.5,
        }
    }

    #[test]
    fn the_anchor_is_fixed_and_lengths_halve() {
        let x = x();
        assert_eq!(x.apply([1584.0, 16.0, 784.0]), [1584.0, 11.5, 784.0]);
        // Summer 2026 - 01's CP0 cell centre (1360, 10 deck, 1104) -> (1472, 8.5, 944)
        let p = x.apply([1360.0, 10.0, 1104.0]);
        assert!((p[0] - 1472.0).abs() < 1e-9 && (p[1] - 8.5).abs() < 1e-9 && (p[2] - 944.0).abs() < 1e-9);
        assert_eq!(x.len(1900.293), 950.1465);
    }

    #[test]
    fn scale_json_moves_points_and_lengths_but_not_directions() {
        let j = parse(
            r#"{"uid":"orig","name":"Summer 2026 - 01","spawn":[1584.0,16.0,784.0],"start_dir":[0.6,0.0,0.8],
                "checkpoints":[{"pos":[1360.0,8.0,1104.0],"gates":[{"pos":[1360.0,8.0,1104.0],"yaw":1.571,"from_item":false}]}],
                "verts":[{"p":[1584.0,18.0,800.0],"s":16.0,"w":8.0,"m":"Asphalt","g":0}],"gate_s":[511.56,1900.293],"length_m":1900.293}"#,
        )
        .unwrap();
        let mut out = String::new();
        scale_json(&j, &x(), None, true, "Tiny", "Tiny Summer 2026 - 01", &mut out);
        let back = parse(&out).unwrap();
        assert_eq!(back.get("uid").unwrap().str(), "Tiny");
        assert_eq!(num3(back.get("spawn").unwrap()), [1584.0, 11.5, 784.0]);
        assert_eq!(num3(back.get("start_dir").unwrap()), [0.6, 0.0, 0.8]);
        let cp = &back.get("checkpoints").unwrap().arr()[0];
        assert_eq!(num3(cp.get("pos").unwrap()), [1472.0, 7.5, 944.0]);
        let gate = &cp.get("gates").unwrap().arr()[0];
        assert_eq!(gate.get("yaw").unwrap().num(), 1.571);
        let v = &back.get("verts").unwrap().arr()[0];
        assert_eq!(num3(v.get("p").unwrap()), [1584.0, 12.5, 792.0]);
        assert_eq!(v.get("s").unwrap().num(), 8.0);
        assert_eq!(v.get("w").unwrap().num(), 4.0);
        assert_eq!(back.get("length_m").unwrap().num(), 950.1465);
        assert_eq!(back.get("gate_s").unwrap().arr()[0].num(), 255.78);
    }

    #[test]
    fn item_centre_is_origin_plus_rotated_half_footprint() {
        // yaw 0: origin (1576, 11.5, 776) -> centre (1584, 11.5, 784)
        let c = item_centre([1576.0, 11.5, 776.0], 0.0, 8.0, 1.0);
        assert!((c[0] - 1584.0).abs() < 1e-6 && (c[2] - 784.0).abs() < 1e-6);
        // yaw pi: origin (1480, 7.5, 952) -> centre (1472, 7.5, 944)
        let c = item_centre([1480.0, 7.5, 952.0], std::f32::consts::PI, 8.0, 1.0);
        assert!((c[0] - 1472.0).abs() < 1e-3 && (c[2] - 944.0).abs() < 1e-3);
    }
}
