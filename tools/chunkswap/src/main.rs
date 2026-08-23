// chunkswap -- copy skippable body chunks from one GBX file into another.
//
// Built as the bisect instrument for "a tape does not survive transplant into
// another run's container" (see OLDBUILD.md §6): the input tape is
// bit-identical in donor and transplant, so whatever the container binds is in
// one of the other chunks. Moving them one at a time named it in five runs --
// `0x0309202D`, the provenance block -- and moving `0x0309201D` + `0x0309202D`
// together is the recipe that puts any tape into any era's container.
//
//   chunkswap --into A.Ghost.Gbx --from B.Ghost.Gbx --id 0x0309201D --id 0x0309202D --out O.Ghost.Gbx
//   chunkswap --list FILE
//   chunkswap --show FILE 0x0309202D
//
// The body is written UNCOMPRESSED, like every other write path in this repo.
use gbx::{all_skip_chunks, container::write_gbx, Gbx};

fn usage() -> ! {
    eprintln!(
        "usage: chunkswap --into A --from B --id 0xNNNNNNNN [--id ...] --out O\n\
         \x20      chunkswap --list FILE\n\
         \x20      chunkswap --show FILE 0xNNNNNNNN"
    );
    std::process::exit(2)
}

fn parse_id(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16)
        .unwrap_or_else(|_| usage())
}

fn load(p: &str) -> Gbx {
    let data = std::fs::read(p).unwrap_or_else(|e| {
        eprintln!("chunkswap: {p}: {e}");
        std::process::exit(1)
    });
    Gbx::parse(&data)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (mut into, mut from, mut out) = (String::new(), String::new(), String::new());
    let mut ids: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--into" => {
                into = a.get(i + 1).cloned().unwrap_or_else(|| usage());
                i += 2;
            }
            "--from" => {
                from = a.get(i + 1).cloned().unwrap_or_else(|| usage());
                i += 2;
            }
            "--out" => {
                out = a.get(i + 1).cloned().unwrap_or_else(|| usage());
                i += 2;
            }
            "--id" => {
                ids.push(parse_id(a.get(i + 1).map(|s| s.as_str()).unwrap_or_else(|| usage())));
                i += 2;
            }
            "--list" => {
                let g = load(a.get(i + 1).map(|s| s.as_str()).unwrap_or_else(|| usage()));
                println!("body {} B, {} skippable chunks", g.body.len(), all_skip_chunks(&g.body).len());
                for c in all_skip_chunks(&g.body) {
                    println!("  0x{:08X} at {:>8} payload {:>8} size {:>8}", c.0, c.1, c.2, c.3);
                }
                return;
            }
            "--show" => {
                let g = load(a.get(i + 1).map(|s| s.as_str()).unwrap_or_else(|| usage()));
                let id = parse_id(a.get(i + 2).map(|s| s.as_str()).unwrap_or_else(|| usage()));
                let c = all_skip_chunks(&g.body)
                    .into_iter()
                    .find(|c| c.0 == id)
                    .unwrap_or_else(|| {
                        eprintln!("chunkswap: no chunk 0x{id:08X} in that file");
                        std::process::exit(1)
                    });
                for (n, ch) in g.body[c.2..c.2 + c.3].chunks(32).enumerate() {
                    let hex: String = ch.iter().map(|b| format!("{b:02x}")).collect();
                    let asc: String = ch
                        .iter()
                        .map(|&b| if (32..127).contains(&b) { b as char } else { '.' })
                        .collect();
                    println!("{:04x}  {hex}  {asc}", n * 32);
                }
                return;
            }
            _ => usage(),
        }
    }
    if into.is_empty() || from.is_empty() || out.is_empty() || ids.is_empty() {
        usage();
    }
    let ga = load(&into);
    let gb = load(&from);
    let mut body = ga.body.clone();
    // Collect the edits first, then apply them back-to-front so the offsets
    // taken before the first edit stay valid.
    let mut plan: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    for id in &ids {
        let ca = all_skip_chunks(&body)
            .into_iter()
            .find(|c| c.0 == *id)
            .unwrap_or_else(|| {
                eprintln!("chunkswap: no chunk 0x{id:08X} in {into}");
                std::process::exit(1)
            });
        let cb = all_skip_chunks(&gb.body)
            .into_iter()
            .find(|c| c.0 == *id)
            .unwrap_or_else(|| {
                eprintln!("chunkswap: no chunk 0x{id:08X} in {from}");
                std::process::exit(1)
            });
        println!("0x{:08X}: {} B -> {} B", id, ca.3, cb.3);
        plan.push((ca.2, ca.3, gb.body[cb.2..cb.2 + cb.3].to_vec()));
    }
    plan.sort_by_key(|p| std::cmp::Reverse(p.0));
    for (off, size, payload) in plan {
        let mut nb = Vec::with_capacity(body.len() + payload.len());
        nb.extend_from_slice(&body[..off - 4]); // the chunk's own size word
        nb.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        nb.extend_from_slice(&payload);
        nb.extend_from_slice(&body[off + size..]);
        body = nb;
    }
    if let Err(e) = write_gbx(&ga, body, &out) {
        eprintln!("chunkswap: {e}");
        std::process::exit(1);
    }
    println!("wrote {out}");
}
