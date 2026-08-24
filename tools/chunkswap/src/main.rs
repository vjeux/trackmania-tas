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
        "usage: chunkswap --into A --from B --id 0xNNNNNNNN [--id ...] [--insert-missing] --out O\n\
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
    let mut insert_missing = false;
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
            "--insert-missing" => {
                insert_missing = true;
                i += 1;
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
    // Collect whole-chunk replacements first, then apply them back-to-front so
    // the offsets taken before the first edit stay valid. With
    // --insert-missing this also serves as an explicit diagnostic instrument:
    // a donor chunk may be inserted into an otherwise from-scratch container,
    // but the resulting file is not publishable and the command says so.
    let into_chunks = all_skip_chunks(&body);
    let from_chunks = all_skip_chunks(&gb.body);
    let mut ordering: Vec<(u32, usize)> = into_chunks.iter().map(|c| (c.0, c.1)).collect();
    // Inline ghost chunks have no size marker and are therefore absent from
    // `all_skip_chunks`, but they still participate in numeric ordering.
    for inline_id in [0x0309_200Fu32, 0x0309_2010u32] {
        let pat = inline_id.to_le_bytes();
        if let Some(off) = (0..body.len().saturating_sub(3)).find(|off| {
            body[*off..*off + 4] == pat
                && !into_chunks.iter().any(|c| *off >= c.2 && *off < c.2 + c.3)
        }) {
            ordering.push((inline_id, off));
        }
    }
    let mut plan: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    for id in &ids {
        let cb = from_chunks.iter().find(|c| c.0 == *id).copied().unwrap_or_else(|| {
            eprintln!("chunkswap: no chunk 0x{id:08X} in {from}");
            std::process::exit(1)
        });
        let donor = gb.body[cb.1..cb.2 + cb.3].to_vec();
        match into_chunks.iter().find(|c| c.0 == *id).copied() {
            Some(ca) => {
                println!("0x{:08X}: {} B -> {} B", id, ca.3, cb.3);
                plan.push((ca.1, ca.2 + ca.3, donor));
            }
            None if insert_missing => {
                let at = ordering.iter()
                    .filter(|c| c.0 > *id)
                    .min_by_key(|c| c.1)
                    .map(|c| c.1)
                    .unwrap_or_else(|| body.len().saturating_sub(4));
                println!(
                    "0x{:08X}: MISSING -> insert {} B at {}  [DIAGNOSTIC DONOR CHUNK]",
                    id, donor.len(), at
                );
                plan.push((at, at, donor));
            }
            None => {
                eprintln!(
                    "chunkswap: no chunk 0x{id:08X} in {into}; pass --insert-missing only for a diagnostic ablation"
                );
                std::process::exit(1)
            }
        }
    }
    plan.sort_by_key(|p| std::cmp::Reverse(p.0));
    for (start, end, replacement) in plan {
        let mut nb = Vec::with_capacity(body.len() + replacement.len());
        nb.extend_from_slice(&body[..start]);
        nb.extend_from_slice(&replacement);
        nb.extend_from_slice(&body[end..]);
        body = nb;
    }
    if let Err(e) = write_gbx(&ga, body, &out) {
        eprintln!("chunkswap: {e}");
        std::process::exit(1);
    }
    println!("wrote {out}");
}
