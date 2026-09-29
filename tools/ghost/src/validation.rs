//! `ghost validation show FILE` / `ghost validation set IN OUT --start-index N`
//!
//! The validation block (chunk `0x0309202D`) carries the START INDEX the
//! dedicated server spawns the car from: an index into the engine's waypoint
//! array (every block waypoint, then every ITEM whose model has a waypoint
//! type, in item order). A container rebound to another map keeps its old
//! index and the car spawns wherever that entry points -- on the MK64 Koopa
//! cuts a human ghost's `2` put it in the void at (40, 0, 40) and every run
//! was a vacuous DNF (2026-09-29). `tmauto synth write` measures the right
//! index for a fresh container; this patches an EXISTING one (a game-written
//! replay used as the client-loadable carrier) in place, byte for byte
//! otherwise.
//!
//! Layout, from the engine's chunk handler (see `tmauto::synth::validation_payload`):
//! u32 flag, str exe_version, u32 exe_checksum, u32 os, u32 cpu,
//! u32 walltime_start, u32 walltime_end, str title_id, 32-byte title checksum,
//! u32 settings_flags, **u32 start_index**, u32 seed, u32 u04, str race_settings.

use crate::cli::{die, flag};
use gbx::container::Gbx;

pub const CHUNK_VALIDATION: u32 = 0x0309_202D;

/// Byte offset of the start-index word inside the chunk payload.
fn start_index_offset(payload: &[u8]) -> Result<usize, String> {
    let mut o = 0usize;
    let u32_at = |o: usize| -> Result<u32, String> {
        payload
            .get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| format!("validation chunk too short at {o}"))
    };
    o += 4; // flag
    let n = u32_at(o)? as usize; // exe_version
    o += 4 + n;
    o += 4 * 5; // exe_checksum, os, cpu, walltime_start, walltime_end
    let n = u32_at(o)? as usize; // title_id
    o += 4 + n;
    o += 32; // title checksum
    o += 4; // settings_flags
    if o + 4 > payload.len() {
        return Err(format!("validation chunk too short: start index would sit at {o} of {}", payload.len()));
    }
    Ok(o)
}

fn find(g: &Gbx) -> Result<(usize, usize), String> {
    gbx::container::all_skip_chunks(&g.body)
        .into_iter()
        .find(|(cid, _, _, _)| *cid == CHUNK_VALIDATION)
        .map(|(_, _, p, size)| (p, size))
        .ok_or_else(|| "no validation chunk 0x0309202D in this file".to_string())
}

pub fn read(g: &Gbx) -> Result<u32, String> {
    let (p, size) = find(g)?;
    let payload = &g.body[p..p + size];
    let o = start_index_offset(payload)?;
    Ok(u32::from_le_bytes(payload[o..o + 4].try_into().unwrap()))
}

pub fn cmd(a: &[String]) {
    let verb = a.first().map(|s| s.as_str()).unwrap_or("");
    match verb {
        "show" => {
            let f = a.get(1).unwrap_or_else(|| die("ghost validation show FILE"));
            let g = Gbx::parse(&std::fs::read(f).unwrap_or_else(|e| die(format!("{f}: {e}"))));
            match read(&g) {
                Ok(k) => println!("{f}: validation start index (u03) = {k}"),
                Err(e) => die(format!("{f}: {e}")),
            }
        }
        "set" => {
            let inp = a.get(1).unwrap_or_else(|| die("ghost validation set IN OUT --start-index N"));
            let out = a.get(2).unwrap_or_else(|| die("ghost validation set IN OUT --start-index N"));
            let k: u32 = flag(a, "--start-index")
                .unwrap_or_else(|| die("--start-index N"))
                .parse()
                .unwrap_or_else(|_| die("--start-index wants an integer"));
            let g = Gbx::parse(&std::fs::read(inp).unwrap_or_else(|e| die(format!("{inp}: {e}"))));
            let was = read(&g).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            let (p, size) = find(&g).unwrap();
            let o = start_index_offset(&g.body[p..p + size]).unwrap();
            let mut body = g.body.clone();
            body[p + o..p + o + 4].copy_from_slice(&k.to_le_bytes());
            // same compression as the input: a game-written carrier stays 'C'
            let bytes = std::fs::read(inp).unwrap();
            let compressed = bytes.get(7) == Some(&b'C');
            let r = if compressed {
                gbx::container::write_gbx_compressed(&g, body, out)
            } else {
                gbx::container::write_gbx(&g, body, out)
            };
            r.unwrap_or_else(|e| die(e));
            let back = Gbx::parse(&std::fs::read(out).unwrap());
            let now = read(&back).unwrap();
            assert_eq!(now, k, "read-back start index differs");
            let same = back.body.len() == g.body.len()
                && back.body.iter().zip(g.body.iter()).filter(|(x, y)| x != y).count() <= 4;
            println!(
                "wrote {out}: validation start index {was} -> {k} (read-back OK; body otherwise {}; {})",
                if same { "byte-identical" } else { "CHANGED ELSEWHERE" },
                if compressed { "'C' body kept" } else { "'U' body kept" }
            );
        }
        _ => die("ghost validation show FILE | ghost validation set IN OUT --start-index N"),
    }
}
