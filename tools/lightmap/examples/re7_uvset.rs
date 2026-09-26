//! `re7_uvset --pak F:K PREFAB VSOUT.bin STRIDE sx sy ox oy [jx jy]` — which uv set the game's lightmapper raster uses: the
//! LOD-0 visuals' TexCoord0 and TexCoord1 through the instance ST vs the captured post-VS SV_Position xy (RE 7).
use mapgeom::static_item::vstream::{Elem, N_TEXCOORD0};
use mapgeom::static_item::Node;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let f = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let pak = f("--pak").unwrap(); let (pp, key) = pak.rsplit_once(':').unwrap();
    let mut store = mapgeom::store::DataStore::empty(); store.add_pak(pp, key).unwrap();
    let rest: Vec<&String> = a.iter().skip(1).filter(|x| !x.starts_with("--") && !x.contains(".pak:")).collect();
    let (path, vsout, stride) = (rest[0], rest[1], rest[2].parse::<usize>().unwrap());
    let st: Vec<f32> = rest[3..7].iter().map(|s| s.parse().unwrap()).collect();
    let jit: (f32, f32) = if rest.len() >= 9 { (rest[7].parse().unwrap(), rest[8].parse().unwrap()) } else { (0.0, 0.0) };
    let pm = store.load_model(path).unwrap();
    let pf = mapgeom::static_item::prefab::CPlugPrefab::from_model(&pm).unwrap();
    let out = std::fs::read(vsout).unwrap();
    let n = out.len() / stride;
    let clip: Vec<(f32, f32)> = (0..n).map(|i| (f32::from_le_bytes(out[i * stride..i * stride + 4].try_into().unwrap()), f32::from_le_bytes(out[i * stride + 4..i * stride + 8].try_into().unwrap()))).collect();
    println!("{n} post-VS vertices; clip x {:.4}..{:.4} y {:.4}..{:.4}", clip.iter().map(|c| c.0).fold(f32::MAX, f32::min), clip.iter().map(|c| c.0).fold(f32::MIN, f32::max), clip.iter().map(|c| c.1).fold(f32::MAX, f32::min), clip.iter().map(|c| c.1).fold(f32::MIN, f32::max));
    for e in &pf.ents {
        let Some(Node::StaticObject(so)) = e.model.inline.as_deref() else { continue };
        let Some(s2) = so.solid2() else { continue };
        for set in 0..2u32 {
            let mut pts: Vec<(f32, f32)> = Vec::new();
            for sg in &s2.shaded_geoms {
                if sg.lod_mask & 1 == 0 && sg.lod_mask != 0 { continue; }
                let Some(Node::Visual(v)) = s2.visuals.get(sg.visual_index as usize).and_then(|r| r.inline.as_deref()) else { continue };
                let Some(stm) = v.stream() else { continue };
                let get = |name: u32| stm.decls.iter().zip(stm.elems.iter()).find(|(d, _)| d.name() == name).map(|(_, e)| e);
                if let Some(Elem::Float2(uv)) = get(N_TEXCOORD0 + set) {
                    for q in uv { let ax = q[0] * st[0] + st[2]; let ay = q[1] * st[1] + st[3]; pts.push((2.0 * ax - 1.0 + jit.0, 1.0 - 2.0 * ay + jit.1)); }
                }
            }
            if pts.is_empty() { continue; }
            // nearest-neighbour distance from each predicted point to the captured set
            let mut sum = 0.0f64; let mut worst = 0.0f32;
            for p in &pts { let d = clip.iter().map(|c| ((c.0 - p.0).powi(2) + (c.1 - p.1).powi(2)).sqrt()).fold(f32::MAX, f32::min); sum += d as f64; worst = worst.max(d); }
            println!("TexCoord{set}: {} LOD-0 vertices → mean nearest distance to the captured clip xy {:.6}, worst {:.6} (one texel = {:.6} in clip x)", pts.len(), sum / pts.len() as f64, worst, 2.0 / 3072.0);
        }
    }
}
