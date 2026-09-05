//! Print the crystal structure of one item (counts per layer) for debugging.
use mapgeom::crystal_model::{locate, CPlugCrystal, Rd, Crystal};
use tmmaps::gbx::Gbx;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&a[1]).unwrap();
    let g = Gbx::parse(&bytes);
    let loc = locate(&g.body).unwrap();
    println!("crystal at 0x{:x}, node {}, edition {}, lookback {:?}", loc.at, loc.node_index, loc.edition_index, loc.lookback.table);
    // step manually through chunks to survive a failure
    let mut r = Rd::new(&g.body, loc.at, loc.lookback.clone());
    let cid = r.u32().unwrap(); println!("chunk {cid:08x} tree gen {}", r.u32().unwrap());
    let cid = r.u32().unwrap(); println!("chunk {cid:08x} v{}", r.u32().unwrap());
    let nm = r.count().unwrap();
    let mut mats = Vec::new();
    for _ in 0..nm { let name = r.string().unwrap(); assert!(name.is_empty()); let n = r.noderef(|r, _| mapgeom::crystal_model::CPlugMaterialUserInst::parse(r)).unwrap(); mats.push(n.inline.unwrap().link().unwrap().to_string()); }
    println!("{nm} materials: {:?}", mats);
    let cid = r.u32().unwrap(); println!("chunk {cid:08x} at 0x{:x}", r.o); r.u32().unwrap(); let n = r.count().unwrap(); let _ = r.u8(); r.o += n - 1;
    let cid = r.u32().unwrap(); println!("chunk {cid:08x} v{} at 0x{:x}", r.u32().unwrap(), r.o);
    let nl = r.count().unwrap();
    for i in 0..nl {
        let start = r.o;
        let ty = r.u32().unwrap(); let lv = r.u32().unwrap(); let en = r.u32().unwrap(); let id = r.id().unwrap(); let name = r.string().unwrap(); let ie = if lv >= 1 { r.u32().unwrap() } else { 9 };
        println!("layer {i} at 0x{start:x}: type {ty} v{lv} crystalEnabled {en} id {id:?} name {name:?} isEnabled {ie}");
        if ty == 0 || ty == 14 {
            let gv = r.u32().unwrap();
            let o = r.o;
            let v = r.u32().unwrap(); r.o = o;
            println!("  type version {gv}, crystal version {v}, at 0x{o:x}");
            match Crystal::parse(&mut r, nm) {
                Ok(c) => {
                    let nidx: usize = c.faces.iter().map(|f| f.uv_index.len()).sum();
                    println!("  positions {} edge_count {} edges {} faces {} tex_coords {} corners {} groups {} u02 {} u03 {} u04 {} anchors {} vl {:?}", c.positions.len(), c.edge_count, c.edges.len(), c.faces.len(), c.tex_coords.len(), nidx, c.groups.len(), c.u02, c.u03, c.u04, c.anchor_infos.len(), c.visual_levels);
                    let maxv = c.faces.iter().flat_map(|f| f.verts.iter()).max(); let maxm = c.faces.iter().map(|f| f.material).max(); let maxg = c.faces.iter().map(|f| f.group).max();
                    println!("  max vert idx {maxv:?} max mat {maxm:?} max group {maxg:?}");
                    if ty == 0 { let u02 = r.array(|r| r.i32()).unwrap(); println!("  u02 {} ints, visible {} collidable {}", u02.len(), r.u32().unwrap(), r.u32().unwrap()); }
                    else { let u = r.array(|r| r.i32()).unwrap(); println!("  trigger ints {}", u.len()); }
                }
                Err(e) => { println!("  ERR {e} at 0x{:x}", r.o); 
                    // print header numbers manually
                    let mut r2 = Rd::new(&g.body, o, loc.lookback.clone());
                    let v = r2.u32().unwrap(); let u01 = r2.u32().unwrap(); let nvl = r2.count().unwrap(); r2.o += nvl*8; let na = r2.count().unwrap(); assert_eq!(na,0);
                    let ng = r2.count().unwrap(); for _ in 0..ng { r2.u32().unwrap(); r2.u8().unwrap(); r2.u32().unwrap(); r2.string().unwrap(); r2.u32().unwrap(); r2.array(|r| r.i32()).unwrap(); }
                    let emb = r2.u8().unwrap(); let u02 = r2.u32().unwrap(); let u03 = r2.u32().unwrap(); let npos = r2.count().unwrap(); r2.o += npos*12; let ec = r2.u32().unwrap(); let ne = r2.count().unwrap();
                    println!("  v{v} u01 {u01} nvl {nvl} groups {ng} emb {emb} u02 {u02} u03 {u03} npos {npos} edge_count {ec} edges {ne} at 0x{:x}", r2.o);
                    let w = mapgeom::crystal_model::opt_width(npos); r2.o += ne*2*w;
                    let nf = r2.count().unwrap(); let nt = r2.count().unwrap(); r2.o += nt*8; let ni = r2.count().unwrap();
                    println!("  faces {nf} tex {nt} nidx {ni} at 0x{:x}; bytes {:02x?}", r2.o, &g.body[r2.o..r2.o+16]);
                    return; }
            }
        } else { println!("  (modifier layer; stopping)"); return; }
    }
    let _ = CPlugCrystal::parse_with(&g.body, loc.at, loc.lookback.clone());

}
