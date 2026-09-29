//! `e6_domeband COLLECTION MOOD [--bands 5] [--sun-az DEG --sun-el DEG] [--eye X,Y,Z]` — OUR DOME'S RADIANCE PER ELEVATION BAND
//! (azimuth-averaged over 360 directions per band) for one collection + mood, built exactly as `lmtool bake --lm-from-map` builds it
//! (main.rs: the mood folder's SkyColor.dds, ScaleGrad0 1, the XML's Atmo lobes, the fog lerp toward the XML's Fog Color by
//! SkyClouds GlobalIntens, GlobalScale = the mood's SkyFactor (--sky-global-scale mood), v = sin(elevation), u mirror), read
//! through `SkyGradient::dome_radiance` (the transcribed ellipsoid dome, VS 16773 / PS 16774). Two collections side by side =
//! the horizon-band question of E6 day 5 (coordinator 17:00Z): is WhiteShore's / RedIsland's dome at −15…+15° greener/redder
//! than BlueBay's at the same elevations, per channel? Also prints every constant it used (the path per collection).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let coll = a.get(1).expect("COLLECTION").clone();
    let mood = a.get(2).expect("MOOD").clone();
    let band: f32 = f("--bands").and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let x = lightmap::moods::mood_xml(&coll, &mood).unwrap_or_else(|| panic!("no mood XML table entry for {coll} {mood}"));
    let path = lightmap::skygrad::mood_file(&coll, &mood, "SkyColor.dds");
    let mut g = lightmap::skygrad::SkyGradient::load(&path).unwrap_or_else(|e| panic!("{e}"));
    // the sun: the mood's default word through the sun arc, or --sun-az/--sun-el (degrees; az = atan2(x, z))
    let (az_d, el_d): (f32, f32) = match (f("--sun-az"), f("--sun-el")) {
        (Some(p), Some(q)) => (p.parse().unwrap(), q.parse().unwrap()),
        _ => (45.0, 55.0),
    };
    let (az, el) = (az_d.to_radians(), el_d.to_radians());
    let sun_dir = [el.cos() * az.sin(), el.sin(), el.cos() * az.cos()];
    g.scale = 1.0;
    g.sun_dir = sun_dir;
    g.sun_az = sun_dir[0].atan2(sun_dir[2]);
    g.v_full = false;
    g.v_sin = true;
    let xml = std::fs::read_to_string(lightmap::skygrad::mood_file(&coll, &mood, "Mood.MoodSetting.xml")).unwrap_or_default();
    g.lobes = lightmap::skygrad::lobes_from_xml(&xml);
    g.fog = lightmap::skygrad::fog_from_xml(&xml, 5300.0);
    g.global_scale = x.sky_factor;
    g.dome_u_mode = 1;
    let eye: [f32; 3] = f("--eye").map(|v| { let p: Vec<f32> = v.split(',').map(|s| s.parse().unwrap()).collect(); [p[0], p[1], p[2]] }).unwrap_or([1024.0, 50.0, 1024.0]);
    println!("{coll} {mood}: SkyColor {} ({}×{}), SkyFactor (GlobalScale) {}, fog {:?}, lobes {:?}, LAmbient {:?}, LDirSun {:?}, sun az {az_d}° el {el_d}°, eye {:?}", path.rsplit('/').next().unwrap(), g.w, g.h, x.sky_factor, g.fog, g.lobes, x.l_ambient, x.l_dir_sun, eye);
    println!("elev band      mean dome L (r g b)            G/R    B/R   |  min-max R over azimuth");
    let mut el0 = -90.0f32;
    while el0 < 90.0 - 1e-3 {
        let el1 = el0 + band;
        let (mut s, mut n) = ([0f64; 3], 0usize);
        let (mut rmin, mut rmax) = (f32::MAX, f32::MIN);
        for i in 0..360 {
            let azd = (i as f32).to_radians();
            let e = ((el0 + el1) * 0.5).to_radians();
            let d = [e.cos() * azd.sin(), e.sin(), e.cos() * azd.cos()];
            let l = g.dome_radiance(eye, d, eye);
            for k in 0..3 { s[k] += l[k] as f64; }
            rmin = rmin.min(l[0]); rmax = rmax.max(l[0]);
            n += 1;
        }
        let m = [s[0] / n as f64, s[1] / n as f64, s[2] / n as f64];
        println!("{:>5.0}..{:>4.0}°   ({:.4} {:.4} {:.4})   {:.3}  {:.3}   |  {:.4}–{:.4}", el0, el1, m[0], m[1], m[2], m[1] / m[0].max(1e-9), m[2] / m[0].max(1e-9), rmin, rmax);
        el0 = el1;
    }
}
