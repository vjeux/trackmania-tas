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

/// Every field of the block, for a diff between two containers (the physics the
/// server runs can follow these: two carriers with the same tape diverged).
pub fn describe(g: &Gbx) -> Result<String, String> {
    let (p, size) = find(g)?;
    let b = &g.body[p..p + size];
    let mut o = 0usize;
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let mut out = String::new();
    out += &format!("  flag            {}\n", u(o));
    o += 4;
    let n = u(o) as usize;
    out += &format!("  exe_version     {:?}\n", String::from_utf8_lossy(&b[o + 4..o + 4 + n]));
    o += 4 + n;
    out += &format!("  exe_checksum    {:#010x}\n  os              {}\n  cpu             {}\n  walltime_start  {}\n  walltime_end    {}\n", u(o), u(o + 4), u(o + 8), u(o + 12), u(o + 16));
    o += 20;
    let n = u(o) as usize;
    out += &format!("  title_id        {:?}\n", String::from_utf8_lossy(&b[o + 4..o + 4 + n]));
    o += 4 + n;
    out += &format!("  title_checksum  {}\n", b[o..o + 32].iter().map(|x| format!("{x:02x}")).collect::<String>());
    o += 32;
    out += &format!("  settings_flags  {:#010x}\n  start_index     {}\n  seed            {}\n  u04             {}\n", u(o), u(o + 4), u(o + 8), u(o + 12));
    o += 16;
    if o + 4 <= b.len() {
        let n = u(o) as usize;
        if o + 4 + n <= b.len() {
            out += &format!("  race_settings   {:?}\n", String::from_utf8_lossy(&b[o + 4..o + 4 + n]));
        }
    }
    Ok(out)
}

/// The file's bytes with the validation SEED word replaced. The engine's
/// physics carries randomness seeded by this word (measured 2026-09-29: a
/// 300-km/h lap tape finished under seed 0 and DNF'd under 1, 2, 12345 and a
/// client's own seed; a 130-km/h cave tape was identical under all of them).
/// A deliverable must hold under several seeds -- `tmsearch validate --seeds N`.
pub fn with_seed(bytes: &[u8], seed: u32) -> Result<Vec<u8>, String> {
    let g = Gbx::parse(bytes);
    let (p, size) = find(&g)?;
    let o = start_index_offset(&g.body[p..p + size])? + 4; // seed follows the start index
    let mut body = g.body.clone();
    body[p + o..p + o + 4].copy_from_slice(&seed.to_le_bytes());
    Ok(if bytes.get(7) == Some(&b'C') { gbx::container::gbx_bytes_compressed(&g, &body) } else {
        let mut f = g.header_bytes_u();
        f.extend_from_slice(&body);
        f
    })
}

/// DST's bytes with SRC's whole validation block (then set the start index
/// with `with_start_index`, SRC's is its own map's).
pub fn copy_block(src: &[u8], dst: &[u8]) -> Result<Vec<u8>, String> {
    let gs = Gbx::parse(src);
    let gd = Gbx::parse(dst);
    let (sp, ss) = find(&gs)?;
    let (dp, ds) = find(&gd)?;
    let payload = &gs.body[sp..sp + ss];
    let mut body = Vec::with_capacity(gd.body.len() + payload.len());
    body.extend_from_slice(&gd.body[..dp - 12]);
    body.extend_from_slice(&CHUNK_VALIDATION.to_le_bytes());
    body.extend_from_slice(gbx::container::SKIP_MAGIC);
    body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    body.extend_from_slice(payload);
    body.extend_from_slice(&gd.body[dp + ds..]);
    Ok(if dst.get(7) == Some(&b'C') { gbx::container::gbx_bytes_compressed(&gd, &body) } else {
        let mut f = gd.header_bytes_u();
        f.extend_from_slice(&body);
        f
    })
}

pub fn with_start_index(bytes: &[u8], k: u32) -> Result<Vec<u8>, String> {
    let g = Gbx::parse(bytes);
    let (p, size) = find(&g)?;
    let o = start_index_offset(&g.body[p..p + size])?;
    let mut body = g.body.clone();
    body[p + o..p + o + 4].copy_from_slice(&k.to_le_bytes());
    Ok(if bytes.get(7) == Some(&b'C') { gbx::container::gbx_bytes_compressed(&g, &body) } else {
        let mut f = g.header_bytes_u();
        f.extend_from_slice(&body);
        f
    })
}

pub fn read_seed(g: &Gbx) -> Result<u32, String> {
    let (p, size) = find(g)?;
    let o = start_index_offset(&g.body[p..p + size])? + 4;
    Ok(u32::from_le_bytes(g.body[p + o..p + o + 4].try_into().unwrap()))
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
            if let Ok(d) = describe(&g) {
                print!("{d}");
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
        "copy" => {
            // DST's validation block := SRC's, whole payload (exe version, checksum,
            // title, flags, seed, race settings, start index). The physics the server
            // runs FOLLOWS this block: a 2024 game-written carrier and a 2026 synth
            // container ran the same tape to trajectories 1 m apart at 10 s and a
            // missed ramp at 17 s (2026-09-29). Set the start index again afterwards
            // if SRC's is not the map's.
            let src = a.get(1).unwrap_or_else(|| die("ghost validation copy SRC DST OUT"));
            let dst = a.get(2).unwrap_or_else(|| die("ghost validation copy SRC DST OUT"));
            let out = a.get(3).unwrap_or_else(|| die("ghost validation copy SRC DST OUT"));
            let gs = Gbx::parse(&std::fs::read(src).unwrap_or_else(|e| die(format!("{src}: {e}"))));
            let dbytes = std::fs::read(dst).unwrap_or_else(|e| die(format!("{dst}: {e}")));
            let gd = Gbx::parse(&dbytes);
            let (sp, ss) = find(&gs).unwrap_or_else(|e| die(format!("{src}: {e}")));
            let (dp, ds) = find(&gd).unwrap_or_else(|e| die(format!("{dst}: {e}")));
            let payload = gs.body[sp..sp + ss].to_vec();
            let mut body = Vec::with_capacity(gd.body.len() + payload.len());
            body.extend_from_slice(&gd.body[..dp - 12]);
            body.extend_from_slice(&CHUNK_VALIDATION.to_le_bytes());
            body.extend_from_slice(gbx::container::SKIP_MAGIC);
            body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            body.extend_from_slice(&payload);
            body.extend_from_slice(&gd.body[dp + ds..]);
            let compressed = dbytes.get(7) == Some(&b'C');
            let r = if compressed {
                gbx::container::write_gbx_compressed(&gd, body, out)
            } else {
                gbx::container::write_gbx(&gd, body, out)
            };
            r.unwrap_or_else(|e| die(e));
            let back = Gbx::parse(&std::fs::read(out).unwrap());
            let d = describe(&back).unwrap_or_else(|e| die(e));
            println!("wrote {out}: validation block is now {src}'s ({} -> {} bytes):\n{d}", ds, payload.len());
        }
        "edit" => {
            // Field-level surgery, for bisecting WHICH field changes the physics:
            // --exe-version S, --flags HEX, --race-settings S, --checksum HEX.
            let inp = a.get(1).unwrap_or_else(|| die("ghost validation edit IN OUT [--exe-version S] [--flags HEX] [--race-settings S] [--checksum HEX]"));
            let out = a.get(2).unwrap_or_else(|| die("ghost validation edit IN OUT ..."));
            let bytes = std::fs::read(inp).unwrap_or_else(|e| die(format!("{inp}: {e}")));
            let g = Gbx::parse(&bytes);
            let (p, size) = find(&g).unwrap_or_else(|e| die(e));
            let b = g.body[p..p + size].to_vec();
            let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
            // parse fields
            let mut o = 0usize;
            let flag_w = b[o..o + 4].to_vec();
            o += 4;
            let n = u(o);
            let mut exe = String::from_utf8_lossy(&b[o + 4..o + 4 + n]).to_string();
            o += 4 + n;
            let mut mid = b[o..o + 20].to_vec(); // checksum, os, cpu, walltimes
            o += 20;
            let n = u(o);
            let title = b[o..o + 4 + n].to_vec();
            o += 4 + n;
            let tchk = b[o..o + 32].to_vec();
            o += 32;
            let mut flags = b[o..o + 4].to_vec();
            let rest3 = b[o + 4..o + 16].to_vec(); // start index, seed, u04
            o += 16;
            let mut race = if o + 4 <= b.len() { let n = u(o); String::from_utf8_lossy(&b[o + 4..o + 4 + n]).to_string() } else { String::new() };
            let mut title = title;
            let mut tchk = tchk;
            let mut rest3 = rest3;
            if let Some(v) = flag(a, "--title") { title = Vec::new(); title.extend_from_slice(&(v.len() as u32).to_le_bytes()); title.extend_from_slice(v.as_bytes()); }
            if flag(a, "--zero-title-checksum").is_some() || a.iter().any(|x| x == "--zero-title-checksum") { tchk = vec![0u8; 32]; }
            if let Some(v) = flag(a, "--os") { let x: u32 = v.parse().unwrap_or_else(|_| die("--os N")); mid[4..8].copy_from_slice(&x.to_le_bytes()); }
            if let Some(v) = flag(a, "--cpu") { let x: u32 = v.parse().unwrap_or_else(|_| die("--cpu N")); mid[8..12].copy_from_slice(&x.to_le_bytes()); }
            if let Some(v) = flag(a, "--walltime") { let x: u32 = v.parse().unwrap_or_else(|_| die("--walltime N")); mid[12..16].copy_from_slice(&x.to_le_bytes()); mid[16..20].copy_from_slice(&(x + 35).to_le_bytes()); }
            // --walltime-span S: keep the START and move the END, so the pair spans
            // the run. THE SERVER CHECKS IT: a 44-second race whose block says the
            // wall clock advanced 5 seconds is refused with "wrong simu ...
            // unexcepted walltime (5s)" -- which reads exactly like a physics
            // mismatch and is not one (2026-09-29, a carrier's block copied from a
            // 5-second client run).
            if let Some(v) = flag(a, "--walltime-span") {
                let sp: u32 = v.parse().unwrap_or_else(|_| die("--walltime-span SECONDS"));
                let st = u32::from_le_bytes(mid[12..16].try_into().unwrap());
                mid[16..20].copy_from_slice(&(st + sp).to_le_bytes());
            }
            if let Some(v) = flag(a, "--seed") { let x: u32 = v.parse().unwrap_or_else(|_| die("--seed N")); rest3[4..8].copy_from_slice(&x.to_le_bytes()); }
            if let Some(v) = flag(a, "--u04") { let x: u32 = v.parse().unwrap_or_else(|_| die("--u04 N")); rest3[8..12].copy_from_slice(&x.to_le_bytes()); }
            if let Some(v) = flag(a, "--exe-version") { exe = v.to_string(); }
            if let Some(v) = flag(a, "--flags") { let x = u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or_else(|_| die("--flags HEX")); flags = x.to_le_bytes().to_vec(); }
            if let Some(v) = flag(a, "--checksum") { let x = u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or_else(|_| die("--checksum HEX")); mid[0..4].copy_from_slice(&x.to_le_bytes()); }
            if let Some(v) = flag(a, "--race-settings") { race = v.to_string(); }
            let mut nb = Vec::new();
            nb.extend_from_slice(&flag_w);
            nb.extend_from_slice(&(exe.len() as u32).to_le_bytes());
            nb.extend_from_slice(exe.as_bytes());
            nb.extend_from_slice(&mid);
            nb.extend_from_slice(&title);
            nb.extend_from_slice(&tchk);
            nb.extend_from_slice(&flags);
            nb.extend_from_slice(&rest3);
            nb.extend_from_slice(&(race.len() as u32).to_le_bytes());
            nb.extend_from_slice(race.as_bytes());
            let mut body = Vec::new();
            body.extend_from_slice(&g.body[..p - 12]);
            body.extend_from_slice(&CHUNK_VALIDATION.to_le_bytes());
            body.extend_from_slice(gbx::container::SKIP_MAGIC);
            body.extend_from_slice(&(nb.len() as u32).to_le_bytes());
            body.extend_from_slice(&nb);
            body.extend_from_slice(&g.body[p + size..]);
            let compressed = bytes.get(7) == Some(&b'C');
            let r = if compressed { gbx::container::write_gbx_compressed(&g, body, out) } else { gbx::container::write_gbx(&g, body, out) };
            r.unwrap_or_else(|e| die(e));
            let back = Gbx::parse(&std::fs::read(out).unwrap());
            print!("wrote {out}:\n{}", describe(&back).unwrap_or_default());
        }
        _ => die("ghost validation show FILE | set IN OUT --start-index N | copy SRC DST OUT | edit IN OUT [--exe-version S] [--flags HEX] [--race-settings S] [--checksum HEX]"),
    }
}
