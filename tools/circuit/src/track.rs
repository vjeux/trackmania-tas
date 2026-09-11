//! From the OSM lap polyline to a smooth, sampled 3D centreline with the
//! ground read off the LIDAR: one station per metre with heading,
//! curvature, height, cross-slope and the corner it belongs to.

use crate::geo::Bng;
use crate::osm::Loop;
use crate::tiff::Mosaic;

#[derive(Clone, Debug)]
pub struct Station {
    /// Arc length from the first station (m).
    pub s: f64,
    pub e: f64,
    pub n: f64,
    /// Ground height at the centreline (m above datum), smoothed along s.
    pub z: f64,
    /// Direction of travel, radians, atan2(dn, de).
    pub heading: f64,
    /// Signed curvature (1/m): positive = turning left.
    pub curvature: f64,
    /// Cross-slope: height rise per metre towards the LEFT edge.
    pub cross_slope: f64,
    pub label: String,
}

pub struct Track {
    pub stations: Vec<Station>,
    pub ds: f64,
    /// A lap (the last station joins the first) or an open road (a pit lane).
    pub closed: bool,
}

/// Centripetal Catmull-Rom through a polyline, resampled to `ds`. Closed:
/// the last point joins the first. Open: the ends are clamped (the curve
/// starts at the first point and ends at the last).
fn resample(pts: &[Bng], labels: &[String], ds: f64, closed: bool) -> (Vec<Bng>, Vec<String>) {
    let n = pts.len();
    let get = |i: i64| -> Bng {
        if closed {
            pts[((i % n as i64) + n as i64) as usize % n]
        } else {
            pts[i.clamp(0, n as i64 - 1) as usize]
        }
    };
    let mut out = Vec::new();
    let mut out_labels = Vec::new();
    let mut carry = 0.0; // distance already covered into the current segment
    if !closed {
        out.push(pts[0]);
        out_labels.push(labels[0].clone());
    }
    let segments = if closed { n as i64 } else { n as i64 - 1 };
    for i in 0..segments {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        let d = |a: Bng, b: Bng| ((a.e - b.e).powi(2) + (a.n - b.n).powi(2)).sqrt().max(1e-6);
        // centripetal parameterisation
        let (t0, t1, t2, t3) = (0.0, d(p0, p1).sqrt(), d(p0, p1).sqrt() + d(p1, p2).sqrt(), d(p0, p1).sqrt() + d(p1, p2).sqrt() + d(p2, p3).sqrt());
        let eval = |t: f64| -> Bng {
            let a1 = lerp(p0, p1, (t - t0) / (t1 - t0));
            let a2 = lerp(p1, p2, (t - t1) / (t2 - t1));
            let a3 = lerp(p2, p3, (t - t2) / (t3 - t2));
            let b1 = lerp(a1, a2, (t - t0) / (t2 - t0));
            let b2 = lerp(a2, a3, (t - t1) / (t3 - t1));
            lerp(b1, b2, (t - t1) / (t2 - t1))
        };
        // walk the segment p1..p2 in fine steps, emitting every ds of arc
        let steps = ((d(p1, p2) / 0.1).ceil() as usize).max(2);
        let mut prev = eval(t1);
        for k in 1..=steps {
            let t = t1 + (t2 - t1) * k as f64 / steps as f64;
            let cur = eval(t);
            let seg = d(prev, cur);
            let mut along = 0.0;
            while carry + (seg - along) >= ds {
                let need = ds - carry;
                along += need;
                out.push(lerp(prev, cur, along / seg));
                out_labels.push(labels[i as usize].clone());
                carry = 0.0;
            }
            carry += seg - along;
            prev = cur;
        }
    }
    (out, out_labels)
}

fn lerp(a: Bng, b: Bng, t: f64) -> Bng {
    Bng { e: a.e + (b.e - a.e) * t, n: a.n + (b.n - a.n) * t }
}

/// Gaussian smoothing of a series: around a loop, or with the ends held
/// (an open road).
pub fn smooth(v: &[f64], sigma: f64, closed: bool) -> Vec<f64> {
    if closed {
        return smooth_periodic(v, sigma);
    }
    if sigma <= 0.0 {
        return v.to_vec();
    }
    let n = v.len() as i64;
    let r = (3.0 * sigma).ceil() as i64;
    let w: Vec<f64> = (-r..=r).map(|k| (-0.5 * (k as f64 / sigma).powi(2)).exp()).collect();
    let ws: f64 = w.iter().sum();
    (0..n)
        .map(|i| {
            let mut acc = 0.0;
            for (j, k) in (-r..=r).enumerate() {
                acc += w[j] * v[(i + k).clamp(0, n - 1) as usize];
            }
            acc / ws
        })
        .collect()
}

/// Gaussian smoothing of a periodic series.
pub fn smooth_periodic(v: &[f64], sigma: f64) -> Vec<f64> {
    if sigma <= 0.0 {
        return v.to_vec();
    }
    let n = v.len() as i64;
    let r = (3.0 * sigma).ceil() as i64;
    let w: Vec<f64> = (-r..=r).map(|k| (-0.5 * (k as f64 / sigma).powi(2)).exp()).collect();
    let ws: f64 = w.iter().sum();
    (0..n)
        .map(|i| {
            let mut acc = 0.0;
            for (j, k) in (-r..=r).enumerate() {
                acc += w[j] * v[(((i + k) % n) + n) as usize % n as usize];
            }
            acc / ws
        })
        .collect()
}

impl Track {
    /// `sigma_xy`: plan smoothing (m) applied to the resampled centreline;
    /// `sigma_z`: height smoothing along the lap (m); `half_width`: the
    /// offset at which the cross-slope is measured.
    pub fn build(lp: &Loop, dtm: &Mosaic, ds: f64, sigma_xy: f64, sigma_z: f64, half_width: f64) -> Track {
        Self::build_points(&lp.points, &lp.labels, true, dtm, ds, sigma_xy, sigma_z, half_width)
    }

    /// An open road (a pit lane): the polyline's ends are its ends.
    pub fn build_open(points: &[Bng], label: &str, dtm: &Mosaic, ds: f64, sigma_xy: f64, sigma_z: f64, half_width: f64) -> Track {
        let labels = vec![label.to_string(); points.len()];
        Self::build_points(points, &labels, false, dtm, ds, sigma_xy, sigma_z, half_width)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_points_pub(points: &[Bng], labels: &[String], closed: bool, dtm: &Mosaic, ds: f64, sigma_xy: f64, sigma_z: f64, half_width: f64) -> Track {
        Self::build_points(points, labels, closed, dtm, ds, sigma_xy, sigma_z, half_width)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_points(points: &[Bng], labels: &[String], closed: bool, dtm: &Mosaic, ds: f64, sigma_xy: f64, sigma_z: f64, half_width: f64) -> Track {
        let (pts, labels) = resample(points, labels, ds, closed);
        let es: Vec<f64> = pts.iter().map(|p| p.e).collect();
        let ns: Vec<f64> = pts.iter().map(|p| p.n).collect();
        let es = smooth(&es, sigma_xy / ds, closed);
        let ns = smooth(&ns, sigma_xy / ds, closed);
        // re-resample to restore uniform spacing after smoothing
        let sm: Vec<Bng> = es.iter().zip(&ns).map(|(&e, &n)| Bng { e, n }).collect();
        let (pts, labels) = resample(&sm, &labels, ds, closed);
        let n = pts.len();
        let nb = |i: usize, d: i64| -> usize {
            if closed {
                ((i as i64 + d).rem_euclid(n as i64)) as usize
            } else {
                (i as i64 + d).clamp(0, n as i64 - 1) as usize
            }
        };
        let heading: Vec<f64> = (0..n)
            .map(|i| {
                let a = pts[nb(i, -1)];
                let b = pts[nb(i, 1)];
                (b.n - a.n).atan2(b.e - a.e)
            })
            .collect();
        let mut curvature: Vec<f64> = (0..n)
            .map(|i| {
                let mut dh = heading[nb(i, 1)] - heading[nb(i, -1)];
                while dh > std::f64::consts::PI {
                    dh -= 2.0 * std::f64::consts::PI;
                }
                while dh < -std::f64::consts::PI {
                    dh += 2.0 * std::f64::consts::PI;
                }
                dh / (2.0 * ds)
            })
            .collect();
        curvature = smooth(&curvature, 4.0 / ds, closed);
        // ground: centre height and cross-slope from a transverse fit
        let mut z = Vec::with_capacity(n);
        let mut slope = Vec::with_capacity(n);
        let mut missing = 0usize;
        for i in 0..n {
            let (sh, ch) = heading[i].sin_cos();
            // left normal = (-sin, cos) in (e, n)
            let mut xs = Vec::new();
            let mut zs = Vec::new();
            for k in -4..=4 {
                let off = half_width * k as f64 / 4.0;
                let e = pts[i].e - sh * off;
                let nn = pts[i].n + ch * off;
                if let Some(h) = dtm.sample(e, nn) {
                    xs.push(off);
                    zs.push(h);
                }
            }
            if xs.len() < 3 {
                missing += 1;
                z.push(f64::NAN);
                slope.push(0.0);
                continue;
            }
            // least squares z = a + b x
            let m = xs.len() as f64;
            let sx: f64 = xs.iter().sum();
            let sz: f64 = zs.iter().sum();
            let sxx: f64 = xs.iter().map(|x| x * x).sum();
            let sxz: f64 = xs.iter().zip(&zs).map(|(x, z)| x * z).sum();
            let den = m * sxx - sx * sx;
            let b = if den.abs() < 1e-9 { 0.0 } else { (m * sxz - sx * sz) / den };
            let a = (sz - b * sx) / m;
            z.push(a);
            slope.push(b);
        }
        assert!(missing == 0, "{missing} stations have no LIDAR ground under them");
        let z = smooth(&z, sigma_z / ds, closed);
        let slope = smooth(&slope, (sigma_z * 1.5) / ds, closed);
        let stations = (0..n)
            .map(|i| Station { s: i as f64 * ds, e: pts[i].e, n: pts[i].n, z: z[i], heading: heading[i], curvature: curvature[i], cross_slope: slope[i], label: labels[i].clone() })
            .collect();
        Track { stations, ds, closed }
    }

    /// The station after `i` (wrapping on a lap; `i` itself at an open end).
    pub fn next(&self, i: usize) -> usize {
        if self.closed {
            (i + 1) % self.stations.len()
        } else {
            (i + 1).min(self.stations.len() - 1)
        }
    }

    pub fn len(&self) -> usize {
        self.stations.len()
    }

    /// Point at signed lateral offset `off` (positive = left) from station i,
    /// with the ground height there from the fitted cross-slope.
    pub fn offset(&self, i: usize, off: f64) -> [f64; 3] {
        let st = &self.stations[i];
        let (sh, ch) = st.heading.sin_cos();
        [st.e - sh * off, st.n + ch * off, st.z + st.cross_slope * off]
    }

    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for s in &self.stations {
            b.0 = b.0.min(s.e);
            b.1 = b.1.min(s.n);
            b.2 = b.2.max(s.e);
            b.3 = b.3.max(s.n);
        }
        b
    }
}
