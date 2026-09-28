//! `re15_edgecensus ROOT PLANE.dds[.gz] [--map BAKED.Map.Gbx] [--top N]` — the coverage (alpha) census of a captured
//! H-basis accumulation plane (RGBA16F 2048², the sweep's END before PS 25113), the value every texel takes through
//! the transcribed PS 25113 (LmSSNormOrGutterWithA_p, finalprep::resolve_ps25113_texel) and WHICH texels carry the
//! plane's maximum — partially covered (0.01 ≤ a ≤ 0.99) vs fully covered, inside a chart rect vs in the gutter
//! (RE 15, read 1: the record's hot texel; the game side of the question).
use lightmap::finalprep::resolve_ps25113_texel;
use lightmap::passdiff::{load_file, Buf};
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: re15_edgecensus ROOT PLANE [--map BAKED.Map.Gbx] [--top N]");
        std::process::exit(2);
    }
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let root = Path::new(&a[1]);
    let top: usize = f("--top").map(|v| v.parse().unwrap()).unwrap_or(12);
    let src: Buf = load_file(root, &a[2], "R16G16B16A16_FLOAT", 0, 0, 0).expect("plane");
    let (w, h) = (src.w, src.h);
    println!("{} : {w}×{h} ×{}", a[2], src.channels);

    if let Some(at) = f("--at") {
        // raw rgba + the PS 25113 output at x,y (and 2× f16-truncated, the PS 1109 source)
        let v: Vec<u32> = at.split(',').map(|s| s.parse().unwrap()).collect();
        let (x, y) = (v[0], v[1]);
        let raw = [src.get(x, y, 0), src.get(x, y, 1), src.get(x, y, 2), src.get(x, y, 3)];
        let o = resolve_ps25113_texel(&src, x, y, false);
        let t2 = |a: f32| lightmap::gpufmt::quantise_f16(lightmap::gpufmt::quantise_f16(a, lightmap::gpufmt::Rounding::Truncate) * 2.0, lightmap::gpufmt::Rounding::Truncate);
        println!("({x},{y}) raw {:?}  ps25113 {:?}  f16trunc×2 {:?}", raw, o, [t2(o[0]), t2(o[1]), t2(o[2])]);
        return;
    }
    // chart rects (2048-layout units = compute texels) when a baked map is given
    let mut rect_id = vec![u32::MAX; (w * h) as usize];
    let mut rects = Vec::new();
    if let Some(map) = f("--map") {
        let m = lightmap::passdiff::read_manifest("{}").unwrap_or_else(|_| panic!("manifest"));
        rects = lightmap::passdiff::chart_rects(&m, Some(&map));
        for (i, r) in rects.iter().enumerate() {
            for y in r.y.max(0)..(r.y + r.h).min(h as i32) {
                for x in r.x.max(0)..(r.x + r.w).min(w as i32) {
                    rect_id[(y as u32 * w + x as u32) as usize] = i as u32;
                }
            }
        }
        println!("{} chart rects from {map}", rects.len());
    }

    // alpha census
    let (mut a0, mut a_tiny, mut a_small, mut a_part, mut a_full) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut ninths = [0usize; 10]; // partial alpha rounded to k/9
    let mut part_other = 0usize;
    let mut full_max_a = 0f32;
    let mut full_min_a = f32::MAX;
    for y in 0..h {
        for x in 0..w {
            let al = src.get(x, y, 3);
            if al == 0.0 { a0 += 1; }
            else if al < 1e-4 { a_tiny += 1; }
            else if al < 0.01 { a_small += 1; }
            else if al <= 0.99 {
                a_part += 1;
                let k = (al * 9.0).round();
                if ((al * 9.0) - k).abs() < 0.02 && (1.0..=9.0).contains(&k) { ninths[k as usize] += 1; } else { part_other += 1; }
            } else { a_full += 1; full_max_a = full_max_a.max(al); full_min_a = full_min_a.min(al); }
        }
    }
    println!("alpha census: 0 → {a0}; (0,1e-4) → {a_tiny}; [1e-4,0.01) → {a_small}; [0.01,0.99] PARTIAL → {a_part} (k/9 bins {:?}, other {part_other}); >0.99 FULL → {a_full} (min {full_min_a:.6}, max {full_max_a:.6})", &ninths[1..]);

    if let Some(ri) = f("--rect") {
        for s in ri.split(',') { dump_rect(&src, &rects, s.parse().unwrap()); }
        return;
    }
    // exact alpha values: count + where (in a rect / gutter), top 24 by count
    {
        let mut hm: std::collections::HashMap<u32, (usize, usize, usize, std::collections::HashMap<u32, usize>)> = std::collections::HashMap::new();
        for y in 0..h {
            for x in 0..w {
                let al = src.get(x, y, 3);
                let e = hm.entry(al.to_bits()).or_insert((0, 0, 0, std::collections::HashMap::new()));
                e.0 += 1;
                let rid = rect_id[(y * w + x) as usize];
                if rid == u32::MAX { e.2 += 1 } else { e.1 += 1; *e.3.entry(if rects.is_empty() { 0 } else { rects[rid as usize].obj }).or_insert(0) += 1; }
            }
        }
        let mut v: Vec<_> = hm.into_iter().collect();
        v.sort_by(|p, q| q.1 .0.cmp(&p.1 .0));
        println!("distinct alpha values: {} — top 24 by count (value = ×128 → directions×fragments; in-rect / gutter):", v.len());
        for (bits, (n, inr, g, objs)) in v.iter().take(24) {
            let mut ov: Vec<_> = objs.iter().collect();
            ov.sort_by(|p, q| q.1.cmp(p.1));
            let objs_s: Vec<String> = ov.iter().take(4).map(|(o, c)| format!("obj{o}:{c}")).collect();
            let al = f32::from_bits(*bits);
            println!("  a {:.6} (= {:.2}/128) : {n} texels ({inr} in a rect, {g} gutter) {}", al, al * 128.0, objs_s.join(" "));
        }
    }
    // PS 25113 over the plane: the max |rgb| by class
    struct Hit { v: f32, x: u32, y: u32, a: f32, own: [f32; 3], out: [f32; 3], cls: &'static str, rect: u32 }
    let mut hits: Vec<Hit> = Vec::new();
    let (mut max_full, mut max_part_own, mut max_part_nb, mut max_raw_uncov) = (0f32, 0f32, 0f32, 0f32);
    for y in 0..h {
        for x in 0..w {
            let al = src.get(x, y, 3);
            let raw = [src.get(x, y, 0), src.get(x, y, 1), src.get(x, y, 2)];
            let o = resolve_ps25113_texel(&src, x, y, false);
            let v = o[0].abs().max(o[1].abs()).max(o[2].abs());
            let cls;
            if al < 0.01 {
                cls = "uncovered(a<0.01, unchanged)";
                max_raw_uncov = max_raw_uncov.max(v);
            } else if al > 0.99 {
                cls = "full";
                max_full = max_full.max(v);
            } else {
                let own = [raw[0] / al, raw[1] / al, raw[2] / al];
                let took_nb = (o[0] - own[0]).abs() > 1e-6 * own[0].abs().max(1e-3) || (o[1] - own[1]).abs() > 1e-6 * own[1].abs().max(1e-3);
                if took_nb { cls = "partial→neighbour"; max_part_nb = max_part_nb.max(v); } else { cls = "partial→own mean"; max_part_own = max_part_own.max(v); }
            }
            let own = if al > 0.0 { [raw[0] / al, raw[1] / al, raw[2] / al] } else { [0.0; 3] };
            hits.push(Hit { v, x, y, a: al, own, out: [o[0], o[1], o[2]], cls, rect: rect_id[(y * w + x) as usize] });
            if hits.len() > 4 * top {
                hits.sort_by(|p, q| q.v.partial_cmp(&p.v).unwrap());
                hits.truncate(top);
            }
        }
    }
    hits.sort_by(|p, q| q.v.partial_cmp(&p.v).unwrap());
    hits.truncate(top);
    println!("max |rgb| after PS 25113: full {max_full:.5}; partial→own {max_part_own:.5}; partial→neighbour {max_part_nb:.5}; uncovered raw {max_raw_uncov:.5}");
    println!("top {top} texels after PS 25113 (×2 = the plane the reduce sees; the 8 dilations never exceed a covered max):");
    for hh in &hits {
        let where_ = if hh.rect == u32::MAX { "GUTTER/no rect".to_string() } else {
            let r = &rects[hh.rect as usize];
            let ex = hh.x as i32 == r.x || hh.x as i32 == r.x + r.w - 1 || hh.y as i32 == r.y || hh.y as i32 == r.y + r.h - 1;
            format!("rect {} obj {} sub {} [{}..{})×[{}..{}) {}", hh.rect, r.obj, r.sub, r.x, r.x + r.w, r.y, r.y + r.h, if ex { "EDGE texel" } else { "interior" })
        };
        println!("  ({:4},{:4}) a {:.5} out ({:.4},{:.4},{:.4}) own-mean ({:.4},{:.4},{:.4}) {} {}", hh.x, hh.y, hh.a, hh.out[0], hh.out[1], hh.out[2], hh.own[0], hh.own[1], hh.own[2], hh.cls, where_);
    }
    // neighbourhood of the top texel
    if let Some(t) = hits.first() {
        println!("3×3 alpha / max-rgb around ({}, {}):", t.x, t.y);
        for dy in -2i64..=2 {
            let mut line = String::new();
            for dx in -2i64..=2 {
                let (x, y) = (t.x as i64 + dx, t.y as i64 + dy);
                if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { line += "   (out)      "; continue; }
                let al = src.get(x as u32, y as u32, 3);
                let mx = src.get(x as u32, y as u32, 0).max(src.get(x as u32, y as u32, 1)).max(src.get(x as u32, y as u32, 2));
                let rid = rect_id[(y as u32 * w + x as u32) as usize];
                line += &format!(" a{:.3}/m{:.3}{}", al, mx, if rid == u32::MAX { "g" } else { "r" });
            }
            println!("{line}");
        }
    }
}

/// `--rect I`: print alpha×128 of rect I with a 2-texel margin (called from main when given).
pub fn dump_rect(src: &Buf, rects: &[lightmap::passdump::ChartRect], i: usize) {
    let r = &rects[i];
    println!("rect {i} obj {} sub {} [{}..{})×[{}..{}) alpha×128 (g = gutter col/row):", r.obj, r.sub, r.x, r.x + r.w, r.y, r.y + r.h);
    for y in (r.y - 2)..(r.y + r.h + 2) {
        let mut line = format!("{y:5}:");
        for x in (r.x - 2)..(r.x + r.w + 2) {
            if x < 0 || y < 0 || x >= src.w as i32 || y >= src.h as i32 { line += "  ---"; continue; }
            let al = src.get(x as u32, y as u32, 3) * 128.0;
            let inside = x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
            line += &format!("{:4.0}{}", al, if inside { " " } else { "g" });
        }
        println!("{line}");
    }
}
