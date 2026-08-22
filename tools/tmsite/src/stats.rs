//! Read a built page back and report what is actually in it: the numbers the
//! verification claims rest on, measured from the artefact rather than from the
//! builder's own bookkeeping. Works on either variant, and on the pages the
//! Python produced.

use crate::json;
use crate::pyfmt::b64_decode;

pub struct Stats {
    pub kind: &'static str,
    pub bytes: usize,
    pub runs: usize,
    pub samples: usize,
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
    pub zmin: f64,
    pub zmax: f64,
    pub vmin: f64,
    pub vmax_data: f64,
    pub vmax_legend: i64,
    pub tmin: i64,
    pub tmax: i64,
    pub names: Vec<String>,
    pub speed_stops: String,
    pub run_hue: String,
    pub cps: String,
}

fn between<'a>(h: &'a str, a: &str, b: &str) -> Result<&'a str, String> {
    let i = h.find(a).ok_or_else(|| format!("marker {:?} not found", a))?;
    let rest = &h[i + a.len()..];
    let j = rest.find(b).ok_or_else(|| format!("terminator {:?} not found", b))?;
    Ok(&rest[..j])
}

fn grab(h: &str, a: &str, b: &str) -> String {
    between(h, a, b).map(|s| s.trim().to_string()).unwrap_or_else(|_| "?".into())
}

pub fn analyse(html: &str) -> Result<Stats, String> {
    if html.contains("const RUNS = ") {
        analyse_full(html)
    } else if html.contains("const META=") {
        analyse_compact(html)
    } else {
        Err("not a tmsite page (no RUNS or META payload)".into())
    }
}

fn fold(s: &mut Stats, x: f64, y: f64, z: f64, v: f64) {
    s.xmin = s.xmin.min(x);
    s.xmax = s.xmax.max(x);
    s.ymin = s.ymin.min(y);
    s.ymax = s.ymax.max(y);
    s.zmin = s.zmin.min(z);
    s.zmax = s.zmax.max(z);
    s.vmin = s.vmin.min(v);
    s.vmax_data = s.vmax_data.max(v);
}

fn blank(kind: &'static str, bytes: usize) -> Stats {
    Stats {
        kind,
        bytes,
        runs: 0,
        samples: 0,
        xmin: f64::INFINITY,
        xmax: f64::NEG_INFINITY,
        ymin: f64::INFINITY,
        ymax: f64::NEG_INFINITY,
        zmin: f64::INFINITY,
        zmax: f64::NEG_INFINITY,
        vmin: f64::INFINITY,
        vmax_data: f64::NEG_INFINITY,
        vmax_legend: 0,
        tmin: i64::MAX,
        tmax: i64::MIN,
        names: vec![],
        speed_stops: String::new(),
        run_hue: String::new(),
        cps: String::new(),
    }
}

fn analyse_full(html: &str) -> Result<Stats, String> {
    let payload = between(html, "const RUNS = ", ";\nconst CPS")?;
    // strict parse: this is exactly what JSON.parse in the browser will do
    let v = json::parse(payload).map_err(|e| format!("RUNS payload is not valid JSON: {}", e))?;
    let arr = v.as_arr().ok_or("RUNS is not an array")?;
    let mut s = blank("full", html.len());
    s.cps = grab(html, "const CPS  = ", ";\nconst VMAX");
    s.vmax_legend = grab(html, "const VMAX = ", ";").parse().unwrap_or(0);
    s.speed_stops = grab(html, "const stops=", ";");
    s.run_hue = grab(html, "function runColour(i){return ", "}");
    for r in arr {
        s.runs += 1;
        s.names.push(r.get("name").and_then(|x| x.as_str()).unwrap_or("?").into());
        let t = r.get("time").and_then(|x| x.as_i64()).unwrap_or(0);
        s.tmin = s.tmin.min(t);
        s.tmax = s.tmax.max(t);
        for p in r.get("p").and_then(|x| x.as_arr()).ok_or("run has no p array")? {
            let q = p.as_arr().ok_or("sample is not an array")?;
            s.samples += 1;
            fold(
                &mut s,
                q[0].as_f64().unwrap(),
                q[1].as_f64().unwrap(),
                q[2].as_f64().unwrap(),
                q[3].as_f64().unwrap(),
            );
        }
    }
    Ok(s)
}

fn analyse_compact(html: &str) -> Result<Stats, String> {
    let meta_s = between(html, "const META=", ", X0=")?;
    let v = json::parse(meta_s).map_err(|e| format!("META payload is not valid JSON: {}", e))?;
    let arr = v.as_arr().ok_or("META is not an array")?;
    let x0: f64 = grab(html, ", X0=", ",").parse().map_err(|_| "bad X0")?;
    let y0: f64 = grab(html, ", Y0=", ",").parse().map_err(|_| "bad Y0")?;
    let z0: f64 = grab(html, ", Z0=", ",").parse().map_err(|_| "bad Z0")?;
    let bin = b64_decode(&grab(html, "atob(\"", "\""));
    let mut s = blank("compact", html.len());
    s.vmax_legend = grab(html, "VMAX=", ",").parse().unwrap_or(0);
    s.cps = grab(html, "CPS=", ";\n");
    s.speed_stops = grab(html, "const ST=", ";");
    s.run_hue = grab(html, "function rc(i){return ", "}");
    let mut o = 0usize;
    for m in arr {
        let m = m.as_arr().ok_or("META row is not an array")?;
        s.runs += 1;
        s.names.push(m[0].as_str().unwrap_or("?").into());
        let t = m[1].as_i64().unwrap_or(0);
        s.tmin = s.tmin.min(t);
        s.tmax = s.tmax.max(t);
        let n = m[2].as_i64().unwrap_or(0) as usize;
        for i in 0..n {
            let b = o + i * 6;
            if b + 6 > bin.len() {
                return Err("binary blob shorter than META claims".into());
            }
            let x = x0 + u16::from_le_bytes([bin[b], bin[b + 1]]) as f64 / 10.0;
            let z = z0 + u16::from_le_bytes([bin[b + 2], bin[b + 3]]) as f64 / 10.0;
            let y = y0 + bin[b + 4] as f64 / 10.0;
            let v = bin[b + 5] as f64 * 2.0;
            s.samples += 1;
            fold(&mut s, x, y, z, v);
        }
        o += n * 6;
    }
    if o != bin.len() {
        return Err(format!(
            "binary blob has {} trailing bytes",
            bin.len() as i64 - o as i64
        ));
    }
    Ok(s)
}

impl Stats {
    pub fn report(&self, label: &str) -> String {
        let mut o = String::new();
        o.push_str(&format!("{}  [{} variant]\n", label, self.kind));
        o.push_str(&format!("  bytes            {}\n", self.bytes));
        o.push_str(&format!("  paths            {}\n", self.runs));
        o.push_str(&format!("  samples          {}\n", self.samples));
        o.push_str(&format!("  x range          {:.1} .. {:.1}\n", self.xmin, self.xmax));
        o.push_str(&format!("  y range          {:.1} .. {:.1}\n", self.ymin, self.ymax));
        o.push_str(&format!("  z range          {:.1} .. {:.1}\n", self.zmin, self.zmax));
        o.push_str(&format!(
            "  speed range      {:.1} .. {:.1} km/h (legend VMAX {})\n",
            self.vmin, self.vmax_data, self.vmax_legend
        ));
        // Seconds with a decimal, like every other time this project prints.
        // The payload itself keeps milliseconds: the page's own JavaScript
        // divides by 1000 to display them, so the number in the HTML is a wire
        // format, not a report.
        o.push_str(&format!(
            "  time range       {} .. {} s\n",
            crate::tick::secs(self.tmin),
            crate::tick::secs(self.tmax)
        ));
        o.push_str(&format!("  first/last run   {} / {}\n",
            self.names.first().cloned().unwrap_or_default(),
            self.names.last().cloned().unwrap_or_default()));
        o.push_str(&format!("  speed ramp       {}\n", self.speed_stops));
        o.push_str(&format!("  per-run colour   {}\n", self.run_hue));
        o.push_str(&format!("  checkpoints      {}\n", self.cps));
        o
    }
}
