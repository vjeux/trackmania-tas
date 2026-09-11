//! Geodesy for the British National Grid.
//!
//! OSM speaks WGS84 latitude/longitude; the Environment Agency rasters are
//! on the Ordnance Survey grid (EPSG:27700, OSGB36 datum). The road from one
//! to the other is the textbook one: geodetic -> Cartesian on GRS80, a
//! seven-parameter Helmert shift onto Airy 1830, back to geodetic, then the
//! transverse Mercator projection with the OS constants. The Helmert shift is
//! good to a few metres nationally; `Frame::shift` lets a measured residual
//! against the LIDAR correct that locally.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bng {
    pub e: f64,
    pub n: f64,
}

struct Ellipsoid {
    a: f64,
    b: f64,
}

const GRS80: Ellipsoid = Ellipsoid { a: 6378137.0, b: 6356752.3141 };
const AIRY1830: Ellipsoid = Ellipsoid { a: 6377563.396, b: 6356256.909 };

fn to_cartesian(el: &Ellipsoid, lat: f64, lon: f64, h: f64) -> [f64; 3] {
    let e2 = (el.a * el.a - el.b * el.b) / (el.a * el.a);
    let (sl, cl) = lat.sin_cos();
    let nu = el.a / (1.0 - e2 * sl * sl).sqrt();
    [(nu + h) * cl * lon.cos(), (nu + h) * cl * lon.sin(), ((1.0 - e2) * nu + h) * sl]
}

fn to_geodetic(el: &Ellipsoid, p: [f64; 3]) -> (f64, f64) {
    let e2 = (el.a * el.a - el.b * el.b) / (el.a * el.a);
    let lon = p[1].atan2(p[0]);
    let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
    let mut lat = p[2].atan2(r * (1.0 - e2));
    for _ in 0..10 {
        let nu = el.a / (1.0 - e2 * lat.sin().powi(2)).sqrt();
        lat = (p[2] + e2 * nu * lat.sin()).atan2(r);
    }
    (lat, lon)
}

/// WGS84 -> OSGB36 (OS's published seven parameters; ~3-5 m accuracy).
fn helmert_wgs84_to_osgb36(p: [f64; 3]) -> [f64; 3] {
    let (tx, ty, tz) = (-446.448, 125.157, -542.060);
    let s = 20.4894e-6;
    let sec = std::f64::consts::PI / (180.0 * 3600.0);
    let (rx, ry, rz) = (-0.1502 * sec, -0.2470 * sec, -0.8421 * sec);
    [
        tx + (1.0 + s) * p[0] - rz * p[1] + ry * p[2],
        ty + rz * p[0] + (1.0 + s) * p[1] - rx * p[2],
        tz - ry * p[0] + rx * p[1] + (1.0 + s) * p[2],
    ]
}

/// OSGB36 geodetic -> British National Grid eastings/northings.
fn osgb36_to_grid(lat: f64, lon: f64) -> Bng {
    let (a, b) = (AIRY1830.a, AIRY1830.b);
    let f0 = 0.9996012717;
    let lat0 = 49.0_f64.to_radians();
    let lon0 = -2.0_f64.to_radians();
    let (e0, n0) = (400000.0, -100000.0);
    let e2 = (a * a - b * b) / (a * a);
    let n = (a - b) / (a + b);
    let (sl, cl) = lat.sin_cos();
    let tl = lat.tan();
    let nu = a * f0 / (1.0 - e2 * sl * sl).sqrt();
    let rho = a * f0 * (1.0 - e2) / (1.0 - e2 * sl * sl).powf(1.5);
    let eta2 = nu / rho - 1.0;
    let m = b * f0
        * ((1.0 + n + 1.25 * n * n + 1.25 * n * n * n) * (lat - lat0)
            - (3.0 * n + 3.0 * n * n + 21.0 / 8.0 * n * n * n) * (lat - lat0).sin() * (lat + lat0).cos()
            + (15.0 / 8.0 * n * n + 15.0 / 8.0 * n * n * n) * (2.0 * (lat - lat0)).sin() * (2.0 * (lat + lat0)).cos()
            - 35.0 / 24.0 * n * n * n * (3.0 * (lat - lat0)).sin() * (3.0 * (lat + lat0)).cos());
    let i = m + n0;
    let ii = nu / 2.0 * sl * cl;
    let iii = nu / 24.0 * sl * cl.powi(3) * (5.0 - tl * tl + 9.0 * eta2);
    let iiia = nu / 720.0 * sl * cl.powi(5) * (61.0 - 58.0 * tl * tl + tl.powi(4));
    let iv = nu * cl;
    let v = nu / 6.0 * cl.powi(3) * (nu / rho - tl * tl);
    let vi = nu / 120.0 * cl.powi(5) * (5.0 - 18.0 * tl * tl + tl.powi(4) + 14.0 * eta2 - 58.0 * tl * tl * eta2);
    let dl = lon - lon0;
    Bng {
        n: i + ii * dl * dl + iii * dl.powi(4) + iiia * dl.powi(6),
        e: e0 + iv * dl + v * dl.powi(3) + vi * dl.powi(5),
    }
}

/// WGS84 degrees -> BNG metres.
pub fn wgs84_to_bng(lat_deg: f64, lon_deg: f64) -> Bng {
    let p = to_cartesian(&GRS80, lat_deg.to_radians(), lon_deg.to_radians(), 0.0);
    let q = helmert_wgs84_to_osgb36(p);
    let (lat, lon) = to_geodetic(&AIRY1830, q);
    osgb36_to_grid(lat, lon)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OS's worked example (A guide to coordinate systems in Great Britain):
    /// 52°39'27.2531"N 1°43'4.5177"E on OSGB36 -> 651409.903, 313177.270.
    #[test]
    fn projection_matches_os_worked_example() {
        let lat = (52.0 + 39.0 / 60.0 + 27.2531 / 3600.0_f64).to_radians();
        let lon = (1.0 + 43.0 / 60.0 + 4.5177 / 3600.0_f64).to_radians();
        let g = osgb36_to_grid(lat, lon);
        assert!((g.e - 651409.903).abs() < 0.01, "E {}", g.e);
        assert!((g.n - 313177.270).abs() < 0.01, "N {}", g.n);
    }

    /// Cartesian round trip on one ellipsoid is the identity.
    #[test]
    fn cartesian_round_trip() {
        let (lat, lon) = (52.07_f64.to_radians(), -1.01_f64.to_radians());
        let (l2, o2) = to_geodetic(&GRS80, to_cartesian(&GRS80, lat, lon, 0.0));
        assert!((l2 - lat).abs() < 1e-12 && (o2 - lon).abs() < 1e-12);
    }

    /// Silverstone's Wikipedia coordinate lands in grid square SP 67 41.
    #[test]
    fn silverstone_lands_in_sp6741() {
        let g = wgs84_to_bng(52.0786, -1.0169);
        assert!((466000.0..469000.0).contains(&g.e), "E {}", g.e);
        assert!((240000.0..244000.0).contains(&g.n), "N {}", g.n);
    }
}
