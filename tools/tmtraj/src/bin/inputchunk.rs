// inputchunk -- read the REAL input archive, chunk 0x0309201D, and count events.
//
//   inputchunk GHOST.Ghost.Gbx [--dump]
//
// WHY THIS EXISTS
//
// Every input count this project has quoted came from CPlugEntRecordData
// telemetry on a 50 ms grid. That is the wrong source and it is known to be
// wrong: six README-stated counts were checked against it and all six
// mismatched (14->17, 23->16, 19->16, 15->59, 2->523, 3->1753). The telemetry
// is a RESAMPLING of what the car did; the input chunk is what the driver
// actually pressed, at 10 ms.
//
// A published README says the 203072 keyboard run is "3 steer values, 14
// events". The telemetry says 39 values and 83 changes. Neither settles it --
// only the chunk does, and that number is going in a video caption.
//
// FORMAT (CGameCtnGhost::Inputs, 0x0309201D):
//   u32 version, then per the version a small header, then
//   u32 n_entries, then n_entries records of { u32 time_ms, u32 packed }.
// The packed word is a union discriminated by its high bits. Rather than
// guessing the bitfield -- which is how wrong numbers get published -- this
// prints the distribution and only counts what it can name.
use std::env;
use tmtraj::entrec::load_body;
use tmtraj::gbx::all_skip_chunks;

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn main() {
    let a: Vec<String> = env::args().skip(1).collect();
    if a.is_empty() {
        eprintln!("usage: inputchunk GHOST.Ghost.Gbx [--dump]");
        std::process::exit(2);
    }
    let dump = a.iter().any(|x| x == "--dump");
    let path = &a[0];
    let body = match load_body(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let chunks = all_skip_chunks(&body);
    eprintln!("{} skippable chunks", chunks.len());
    let mut found = false;
    for (cid, _at, data, size) in &chunks {
        // 0x0309201D is the ghost input chunk; 0x0309202? siblings also carry
        // input data in some versions, so report every 0x030920xx we see.
        if (cid & 0xFFFFFF00) != 0x0309_2000 {
            continue;
        }
        println!("chunk 0x{:08X}  {} bytes", cid, size);
        if *cid != 0x0309_201D {
            continue;
        }
        found = true;
        let d = &body[*data..*data + *size];
        if d.len() < 8 {
            println!("  too short");
            continue;
        }
        let ver = u32le(d, 0);
        println!("  version {}", ver);

        // Find the entry count by scanning for a length that exactly fits the
        // remaining bytes as 8-byte records -- more honest than assuming an
        // offset that varies by version.
        let mut best: Option<(usize, u32)> = None;
        for off in (4..d.len().min(64)).step_by(4) {
            let n = u32le(d, off);
            if n > 0 && n < 200_000 {
                let need = off + 4 + n as usize * 8;
                if need == d.len() {
                    best = Some((off + 4, n));
                    break;
                }
            }
        }
        let Some((start, n)) = best else {
            println!("  could not locate an 8-byte-record table that fits the chunk exactly");
            println!("  first 64 bytes: {:02x?}", &d[..d.len().min(64)]);
            continue;
        };
        println!("  {} entries at offset {}", n, start);

        let mut rows: Vec<(u32, u32)> = Vec::with_capacity(n as usize);
        for i in 0..n as usize {
            let o = start + i * 8;
            rows.push((u32le(d, o), u32le(d, o + 4)));
        }

        // What kinds of event are in here? Group by the packed word's top byte,
        // which is where the discriminator lives in every version seen.
        use std::collections::BTreeMap;
        let mut kinds: BTreeMap<u32, usize> = BTreeMap::new();
        for (_, p) in &rows {
            *kinds.entry(p >> 24).or_insert(0) += 1;
        }
        println!("  event kinds (top byte of the packed word -> count):");
        for (k, c) in &kinds {
            println!("    0x{:02X}  {}", k, c);
        }
        let t0 = rows.first().map(|r| r.0).unwrap_or(0);
        let t1 = rows.last().map(|r| r.0).unwrap_or(0);
        println!("  time span {} .. {} ms", t0, t1);
        println!("  TOTAL RECORDED INPUT EVENTS: {}", n);

        if dump {
            for (t, p) in &rows {
                println!("    {:>7} 0x{:08X}", t, p);
            }
        }
    }
    if !found {
        println!("no 0x0309201D input chunk in this file");
    }
}
