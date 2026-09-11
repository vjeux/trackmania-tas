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
}

/// Centripetal Catmull-Rom through a closed polyline, resampled to `ds`.
fn resample_closed(pts: &[Bng], labels: &[String], ds: f64) -> (Vec<Bng>, Vec<String>) {
    let n = pts.len();
    let get = |i: i64| pts[((i % n as i64) + n as i64) as usize % n];
    let mut out = Vec::new();
    let mut out_labels = Vec::new();
    let mut carry = 0.0; // distance already covered into the current segment
    for i in 0..n as i64 {
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
        let (pts, labels) = resample_closed(&lp.points, &lp.labels, ds);
        let es: Vec<f64> = pts.iter().map(|p| p.e).collect();
        let ns: Vec<f64> = pts.iter().map(|p| p.n).collect();
        let es = smooth_periodic(&es, sigma_xy / ds);
        let ns = smooth_periodic(&ns, sigma_xy / ds);
        // re-resample to restore uniform spacing after smoothing
        let sm: Vec<Bng> = es.iter().zip(&ns).map(|(&e, &n)| Bng { e, n }).collect();
        let (pts, labels) = resample_closed(&sm, &labels, ds);
        let n = pts.len();
        let heading: Vec<f64> = (0..n)
            .map(|i| {
                let a = pts[(i + n - 1) % n];
                let b = pts[(i + 1) % n];
                (b.n - a.n).atan2(b.e - a.e)
            })
            .collect();
        let mut curvature: Vec<f64> = (0..n)
            .map(|i| {
                let mut dh = heading[(i + 1) % n] - heading[(i + n - 1) % n];
                while dh > std::f64::consts::PI {
                    dh -= 2.0 * std::f64::consts::PI;
                }
                while dh < -std::f64::consts::PI {
                    dh += 2.0 * std::f64::consts::PI;
                }
                dh / (2.0 * ds)
            })
            .collect();
        curvature = smooth_periodic(&curvature, 4.0 / ds);
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
        let z = smooth_periodic(&z, sigma_z / ds);
        let slope = smooth_periodic(&slope, (sigma_z * 1.5) / ds);
        let stations = (0..n)
            .map(|i| Station { s: i as f64 * ds, e: pts[i].e, n: pts[i].n, z: z[i], heading: heading[i], curvature: curvature[i], cross_slope: slope[i], label: labels[i].clone() })
            .collect();
        Track { stations, ds }
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
