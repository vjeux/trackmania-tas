//! `re7_trailer MAP…` — dump a saved map's probe trailer (Volume) block table verbatim (RE 7).
fn main() {
    for p in std::env::args().skip(1) {
        let m = match lightmap::mapio::load(&p) { Ok(m) => m, Err(e) => { println!("{p}: {e}"); continue } };
        let Some(d) = m.chunk.data.as_ref() else { println!("{p}: no lightmap data"); continue };
        let v = match lightmap::volume::Volume::parse(&d.cache.trailer) { Ok(v) => v, Err(e) => { println!("{p}: {e}"); continue } };
        println!("== {p}");
        println!("head_consts {:?} frame_info {:?} grid {:?}", v.head_consts, v.frame_info, v.grid);
        println!("slot_grid {:?} slot_tile {:?} block_size {:?} inv_scale {:?} ({:?}) unk_f {:?} ({:?}) counts {:?} tail {} B", v.slot_grid, v.slot_tile, v.block_size, v.inv_scale, v.inv_scale.map(|x| format!("{:#010x}", x.to_bits())), v.unk_f, v.unk_f.map(|x| format!("{:#010x}", x.to_bits())), v.counts, v.tail.len());
        println!("slots ({}): {:?}", v.slots.len(), v.slots);
        println!("cell4 ({}): first 16 {:?}", v.cell4.len(), &v.cell4[..v.cell4.len().min(16)]);
        for (i, b) in v.blocks.iter().enumerate() {
            println!("block {i}: origin {:?} min {:?} max {:?} cell {:?} pos {:?} ({:?}) slices {:?}", b.origin, b.min, b.max, b.cell, b.pos, b.pos.map(|x| format!("{:#010x}", x.to_bits())), b.slices);
        }
    }
}
