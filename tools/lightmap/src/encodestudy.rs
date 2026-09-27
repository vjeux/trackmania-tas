//! `lmtool encode-study DIR --against EDITOR.Map.Gbx [--tsv OUT]` — THE FRAME-0 ENCODE QUANTISATION (port engineer F,
//! 2026-09-27; the coordinator's cell after RE 14's 17:35Z read): the four accumulated H-basis images a bake wrote with
//! `LMTOOL_FINALS_OUT=DIR` are pushed through the transcribed finalisation tail (PS 1034 copy, 8 × PS 1332, the max
//! reduce) and the CS 23025 encode under EVERY UNORM store rounding the encoder knows (Trunc12 = today's default, read off
//! the WhiteStick capture's byte thresholds; NearestEven; HalfUp; Truncate) × fma on/off; then the client's CPU steps
//! (YCbCr_to_RGB_Down2x2, the per-chart max + truncating rescale) give the frame-0 RGB image and the chart bytes fb0
//! BEFORE the WebP encode. Each variant is scored against the editor's file: the fb0 bytes identical (an exact integer
//! test — V2's 240-vs-241 on 634 / 4 126 np-tk3 charts), the signed fb0 difference histogram, and the decoded colour
//! image against the editor's VP8-decoded frame 0 over the charts' own pixels (mean signed R/G/B byte difference — RE 14's
//! Cr-step hypothesis predicts a CONSTANT +1.6 in R and −0.8 in G on the editor's side if our Cr byte sits one step low;
//! the VP8 loss is zero-mean over 10⁵ texels).
use crate::passdiff::Buf;

pub fn run(a: &[String]) -> Result<(), String> {
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let dir = std::path::PathBuf::from(a.get(1).ok_or("usage: lmtool encode-study DIR --against EDITOR.Map.Gbx [--tsv OUT]")?);
    let editor = f("--against").ok_or("--against EDITOR.Map.Gbx")?;
    let mood_txt = std::fs::read_to_string(dir.join("mood.txt")).map_err(|e| format!("mood.txt: {e}"))?;
    let mut it = mood_txt.split_whitespace();
    let mood: f32 = it.next().and_then(|v| v.parse().ok()).ok_or("mood.txt: mood")?;
    let w: u32 = it.next().and_then(|v| v.parse().ok()).ok_or("mood.txt: w")?;
    let h: u32 = it.next().and_then(|v| v.parse().ok()).ok_or("mood.txt: h")?;
    let finals: Vec<Buf> = (0..4).map(|k| {
        let b = std::fs::read(dir.join(format!("final_{k}.f32"))).map_err(|e| format!("final_{k}.f32: {e}"))?;
        let data: Vec<f32> = b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        if data.len() != (w * h * 4) as usize { return Err(format!("final_{k}.f32: {} floats, expected {}", data.len(), w * h * 4)); }
        Ok(Buf { w, h, channels: 4, data })
    }).collect::<Result<_, String>>()?;
    let ed = crate::mapio::load(&editor)?;
    let d = ed.chunk.data.as_ref().ok_or("the editor file has no lightmap data")?;
    let mp = d.cache.mapping().ok_or("the editor file has no mapping")?;
    let charts: Vec<(u32, u32, u32, u32)> = (0..mp.count as usize).map(|i| (mp.pos[i].0 as u32, mp.pos[i].1 as u32, mp.size[i].0 as u32, mp.size[i].1 as u32)).collect();
    let fb_ed = mp.frame_bytes.first().ok_or("the editor file has no frame bytes")?;
    let img_ed = crate::img::decode_webp(&d.frames[0].images[0])?;
    let (ow, oh) = (w / 2, h / 2);
    if img_ed.w != ow || img_ed.h != oh { return Err(format!("the editor's frame 0 is {}×{}, the finals give {}×{}", img_ed.w, img_ed.h, ow, oh)); }
    println!("encode-study: {} charts, editor frame 0 {}×{}, mood cb {mood}; the editor's record MaxHDR {:?}", charts.len(), img_ed.w, img_ed.h, crate::classcmp::record_maxhdr(mp, 0));
    // --capture ROOT [--frame N]: our four finals against the capture's final_02 planes (the ×2-scaled MRTs, ids 24911/24914/24917/24920 in
    // frame 74490) — where the full bake's sweep-end planes differ from the editor's, and where each plane's PEAK sits on both sides
    if let Some(root) = f("--capture") {
        let root = std::path::PathBuf::from(root);
        let frame: u32 = f("--frame").and_then(|v| v.parse().ok()).unwrap_or(74490);
        let txt = std::fs::read_to_string(root.join("MANIFEST.json")).map_err(|e| format!("MANIFEST.json: {e}"))?;
        let m = crate::passdiff::read_manifest(&txt)?;
        const SCL: [&str; 4] = ["24911", "24914", "24917", "24920"];
        for k in 0..4 {
            let e = m.passes.iter().find(|e| e.pass == "final_02_scaled_x2_ps1109" && e.frame == Some(frame) && e.file.contains(&format!("_{}.dds", SCL[k]))).ok_or_else(|| format!("no final_02 entry for {}", SCL[k]))?;
            let cap = crate::passdiff::load_entry(&root, e)?;
            let ours = &finals[k];
            let r = crate::gpucmp::compare(ours, &cap, 4, crate::gpucmp::Fmt::F16);
            let peak = |b: &Buf| -> (f32, u32, u32, u32) { let (mut best, mut bx, mut by, mut bc) = (0.0f32, 0, 0, 0); for y in 0..b.h { for x in 0..b.w { for c in 0..3 { let v = b.get(x, y, c).abs(); if v > best { best = v; bx = x; by = y; bc = c; } } } } (best, bx, by, bc) };
            let (po, pc) = (peak(ours), peak(&cap));
            let at = |b: &Buf, x: u32, y: u32| -> [f32; 4] { [b.get(x, y, 0), b.get(x, y, 1), b.get(x, y, 2), b.get(x, y, 3)] };
            println!("plane {k} vs the capture's final_02 {}: {}", SCL[k], r.line());
            println!("   peak ours {:.5} at ({}, {}) ch {} [capture there {:?}]; peak capture {:.5} at ({}, {}) ch {} [ours there {:?}]", po.0, po.1, po.2, po.3, at(&cap, po.1, po.2), pc.0, pc.1, pc.2, pc.3, at(ours, pc.1, pc.2));
            // rgb only, over the texels the capture covers (alpha > 0): the relative difference histogram
            let (mut n, mut same, mut le1, mut le5, mut le20, mut gt20, mut lo, mut hi) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
            for y in 0..cap.h { for x in 0..cap.w { if cap.get(x, y, 3) <= 0.0 && ours.get(x, y, 3) <= 0.0 { continue; } n += 1; let mut worst = 0.0f32; let mut sgn = 0.0f32; for c in 0..3 { let (g, o) = (cap.get(x, y, c), ours.get(x, y, c)); let d = (o - g).abs() / g.abs().max(1e-3); if d > worst { worst = d; sgn = o.abs() - g.abs(); } } if worst == 0.0 { same += 1; } else if worst <= 0.01 { le1 += 1; } else if worst <= 0.05 { le5 += 1; } else if worst <= 0.2 { le20 += 1; } else { gt20 += 1; } if worst > 0.01 { if sgn < 0.0 { lo += 1; } else { hi += 1; } } } }
            println!("   rgb over {n} covered texels: identical {same}, ≤1 % {le1}, ≤5 % {le5}, ≤20 % {le20}, >20 % {gt20}; of those >1 %: ours darker {lo}, brighter {hi}");
            // the >1 % texels by owner: the chart (in the editor's mapping order) whose layout rect holds the texel — items named by obj, tiles pooled
            let mut by_owner = std::collections::BTreeMap::<String, (usize, usize, f64)>::new();
            for y in 0..cap.h { for x in 0..cap.w { if cap.get(x, y, 3) <= 0.0 && ours.get(x, y, 3) <= 0.0 { continue; } let mut worst = 0.0f32; let mut sd = 0.0f32; for c in 0..3 { let (g, o) = (cap.get(x, y, c), ours.get(x, y, c)); let d = (o - g).abs() / g.abs().max(1e-3); if d > worst { worst = d; sd = (o.abs() - g.abs()) / g.abs().max(1e-3); } } if worst <= 0.01 { continue; }
                let owner = charts.iter().enumerate().find(|(_, &(cx, cy, cw, ch))| x >= cx && x < cx + cw && y >= cy && y < cy + ch).map(|(i, &(_, _, cw, ch))| { let obj = mp.binds[i].obj_idx; if obj >= 4096 || cw >= 64 { format!("chart {i} obj {obj} ({}×{})", cw, ch) } else { "tiles".to_string() } }).unwrap_or_else(|| "no chart (pad/gutter)".to_string());
                let e = by_owner.entry(owner).or_default(); if sd < 0.0 { e.0 += 1; } else { e.1 += 1; } e.2 += sd as f64; } }
            let mut v: Vec<_> = by_owner.into_iter().collect(); v.sort_by_key(|(_, (a, b, _))| std::cmp::Reverse(a + b));
            for (k, (lo, hi, s)) in v.iter().take(8) { println!("      {k}: darker {lo}, brighter {hi}, mean signed rel Δ {:+.3}", s / (*lo + *hi).max(1) as f64); }
        }
    }
    use crate::gpuenc::{EncodeOpts, UnormRounding};
    let mut rows: Vec<String> = Vec::new();
    for unorm in [UnormRounding::Trunc12, UnormRounding::NearestEven, UnormRounding::HalfUp, UnormRounding::Truncate] {
        for fma in [false, true] {
            for f16_store in [false, true] {
                let o = EncodeOpts { fma, unorm, f16_store, f16_c: false };
                let (imgs, maxhdr_ours, enc_ours) = crate::e2e::finalise_tail_opts(&finals, mood, o);
                // --maxhdr-editor: the encode normalised by the EDITOR's record (MaxHDR / κ and the three tail scales / (√3·κ)) instead of our
                // own max — isolates the encode + CPU chain from the light's peak (pwc-day: our record 2.119 vs 2.135 shifts every chart byte)
                let (maxhdr, enc) = if a.iter().any(|x| x == "--maxhdr-editor") {
                    let r = 60usize; let hd = &mp.head; let rf = |o: usize| f32::from_le_bytes(hd[r + o..r + o + 4].try_into().unwrap());
                    let me = [rf(20) / 0.39894226f32, rf(54) / 0.6909883f32, rf(58) / 0.6909883f32, rf(62) / 0.6909883f32];
                    (me, crate::gpuenc::encode_ycbcr4([&imgs[0], &imgs[1], &imgs[2], &imgs[3]], me, mood, o))
                } else { (maxhdr_ours, enc_ours) };
                let plane = |t: &[[u8; 4]], c: usize| -> Vec<u8> { t.iter().map(|p| p[c]).collect() };
                let mut rgb = crate::filecheck::ycbcr_to_rgb_down2x2(&plane(&enc.y4, 0), &plane(&enc.cb4, 0), &plane(&enc.cr4, 0), w as usize, h as usize);
                let fb = crate::filecheck::chart_normalise(&mut rgb, ow, oh, &charts);
                // fb0: identical count and the signed difference histogram (ours − editor)
                let n = fb.len().min(fb_ed.len());
                let same = (0..n).filter(|&i| fb[i] == fb_ed[i]).count();
                let mut hist = std::collections::BTreeMap::<i32, usize>::new();
                for i in 0..n { *hist.entry(fb[i] as i32 - fb_ed[i] as i32).or_default() += 1; }
                let hist_s: Vec<String> = hist.iter().filter(|(k, _)| **k != 0).map(|(k, v)| format!("{k:+}:{v}")).collect();
                // the colour image vs the editor's decoded frame 0 over the charts' own pixels (fb0 ≠ 0 / 255 on both sides)
                let (mut sum, mut cnt) = ([0f64; 3], 0usize);
                let (mut absum, mut same_px) = ([0f64; 3], 0usize);
                for (i, &(x, y, cw, ch)) in charts.iter().enumerate() {
                    if i >= n || fb[i] == 0 || fb[i] == 255 || fb_ed[i] == 0 || fb_ed[i] == 255 { continue; }
                    let (x0, y0, pw, ph) = crate::filecheck::chart_px_shrunk(x, y, cw, ch);
                    for py in y0..(y0 + ph).min(oh) { for px in x0..(x0 + pw).min(ow) {
                        let o = ((py * ow + px) * 3) as usize;
                        let mut all = true;
                        for c in 0..3 { let dlt = rgb[o + c] as f64 - img_ed.px[o + c] as f64; sum[c] += dlt; absum[c] += dlt.abs(); if dlt != 0.0 { all = false; } }
                        if all { same_px += 1; }
                        cnt += 1;
                    } }
                }
                let c = cnt.max(1) as f64;
                let line = format!("{:?}\tfma {}\tf16store {}\tfb0 same {same}/{n}\tfb0 Δ {}\tMaxHdr {:?}\tpx {cnt}: mean Δ (ours−editor) R {:+.3} G {:+.3} B {:+.3}, mean |Δ| {:.3}/{:.3}/{:.3}, identical {:.2} %",
                    unorm, fma as u8, f16_store as u8, if hist_s.is_empty() { "—".to_string() } else { hist_s.join(" ") }, maxhdr, sum[0] / c, sum[1] / c, sum[2] / c, absum[0] / c, absum[1] / c, absum[2] / c, 100.0 * same_px as f64 / c);
                println!("{line}");
                rows.push(line);
            }
        }
    }
    if let Some(p) = f("--tsv") { std::fs::write(&p, rows.join("\n") + "\n").map_err(|e| format!("{p}: {e}"))?; }
    Ok(())
}
