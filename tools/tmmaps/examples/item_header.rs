use tmmaps::gbx::{Gbx, Reader};
fn lb(r: &mut Reader, table: &mut Vec<String>) -> String {
    let w = r.u32();
    if w == 0xFFFF_FFFF { return "<none>".into(); }
    if (w & 0x3FFF_FFFF) == 0 { let s = r.string(); table.push(s.clone()); return format!("NEW {s:?}"); }
    let idx = (w & 0x3FFF_FFFF) as usize;
    if w & 0xC000_0000 == 0 { return format!("collection#{w}"); }
    format!("REF[{idx}]={:?}", table.get(idx - 1))
}
fn main() {
    for f in std::env::args().skip(1) {
        let b = std::fs::read(&f).unwrap();
        let g = Gbx::parse(&b);
        let ud = &g.user_data;
        let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
        let mut off = 4 + n * 8;
        println!("### {f}: class {:08X}, {} header chunks", g.class_id, n);
        for i in 0..n {
            let id = u32::from_le_bytes(ud[4 + i * 8..8 + i * 8].try_into().unwrap());
            let size = (u32::from_le_bytes(ud[8 + i * 8..12 + i * 8].try_into().unwrap()) & 0x7FFF_FFFF) as usize;
            let d = &ud[off..off + size];
            print!("  chunk {id:08X} size {size}");
            if id == 0x2E001003 {
                let mut r = Reader::new(d);
                let mut t = Vec::new();
                let ver = r.u32();
                print!("  lbver {ver}");
                let name = lb(&mut r, &mut t); let coll = r.u32(); let author = lb(&mut r, &mut t);
                let v = r.u32();
                let page = r.string();
                print!("  ident=({name}, coll {coll}, {author}) v{v} page {page:?}");
                if v >= 4 { let _ = lb(&mut r, &mut t); }
                if v >= 3 { let _ = r.u32(); }
                if v >= 2 { let _ = r.u32(); }
                if v >= 5 { let _ = r.u32(); }
                if v >= 6 { let _ = r.u32(); }
                if v >= 7 { let disp = r.string(); print!(" name {disp:?}"); }
            }
            if id == 0x2E001006 { print!("  (light)"); }
            println!();
            off += size;
        }
    }
}
