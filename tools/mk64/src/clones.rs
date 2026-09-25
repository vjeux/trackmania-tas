//! CLONES — the January 2026 Trackmania feature (vjeux, 2026-09-25: "in
//! solo play you can have ghosts along that have real collision … 7 other
//! players CPU driven"): a map flagged for clones spawns collidable copies of
//! its VALIDATION GHOST in solo play, each starting some seconds ahead and
//! replaying slower than real time, so the player has to pass them all.
//!
//! What the file carries (read off four Weekly Grands maps, TMX 354814 /
//! 354812 / 354801 / 355059, 2026-02-02 build, all `hasclones="1"`):
//!
//! * `0x0305B00F` — the validation ghost as a nested archive: `u32 0`,
//!   `u32 size`, then a `CGameCtnGhost` node (class id, chunks, FACADE01) with
//!   its OWN lookback-string table (the `03 00 00 00` marker inside) and its
//!   record-data node referenced as index 2. A `.Ghost.Gbx` body is the same
//!   node with the record at index 1.
//! * `0x03043044` — the ManiaScript metadata (`CScriptTraitsMetadata` v6) with
//!   `Nadeo_IsCloneEnabled` (Boolean), `Race_AuthorClones` (Int2[]: per clone
//!   the start offset in ms — negative, it started earlier — and its slowdown
//!   × 1 000 000; the editor writes 31 clones spread over the lap ahead, from
//!   −2.5 s at ×1.506 to one lap at ×1.15) and `Race_AuthorRaceWaypointTimes`
//!   (Integer[]: the ghost's waypoint crossings, laps × checkpoints).
//! * the header: `<times … hasclones="1">`, and `0x03043002`'s author/medal
//!   times, `isLapRace`, `nbLaps`, `nbCheckpoints`.
//!
//! Every clone is a copy of the ONE validation ghost, so they share its skin
//! (the ghost's `PackDesc`): one MK64 character for the whole field.

use tmmaps::map::MapFile;

/// One clone: start offset (ms, negative = ahead) and slowdown × 1e6.
#[derive(Clone, Copy, Debug)]
pub struct Clone {
    pub offset_ms: i32,
    pub slow_e6: i32,
}

/// `n` clones from `near_s` to `far_s` ahead, slowdown from `slow_near` (the
/// nearest, caught first) to `slow_far`.
pub fn schedule(n: usize, near_s: f32, far_s: f32, slow_near: f32, slow_far: f32) -> Vec<Clone> {
    (0..n)
        .map(|k| {
            let t = if n > 1 { k as f32 / (n - 1) as f32 } else { 0.0 };
            Clone { offset_ms: -((near_s + (far_s - near_s) * t) * 1000.0).round() as i32, slow_e6: ((slow_near + (slow_far - slow_near) * t) * 1e6).round() as i32 }
        })
        .collect()
}

fn varuint(out: &mut Vec<u8>, mut v: u32) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn trait_name(out: &mut Vec<u8>, name: &str) {
    varuint(out, name.len() as u32);
    out.extend_from_slice(name.as_bytes());
}

/// The `0x03043044` payload: version 0, size, then the metadata node with
/// four types (Boolean, Integer, Integer[], Int2[]) and the three clone traits
/// plus `LibMapType_MapTypeVersion = 1` (the order the editor writes).
pub fn metadata_payload(enabled: bool, waypoint_ms: &[i32], clones: &[Clone]) -> Vec<u8> {
    let mut node = Vec::new();
    node.extend_from_slice(&0x1100_2000u32.to_le_bytes());
    node.extend_from_slice(&6u32.to_le_bytes());
    // types: 0 Boolean, 1 Integer, 2 Integer[] (key Void), 3 Int2[] (key Void)
    node.push(4);
    node.push(1);
    node.push(2);
    node.extend_from_slice(&[7, 0, 2]);
    node.extend_from_slice(&[7, 0, 14]);
    // traits
    node.push(4);
    trait_name(&mut node, "Nadeo_IsCloneEnabled");
    node.push(0);
    node.push(enabled as u8);
    trait_name(&mut node, "LibMapType_MapTypeVersion");
    node.push(1);
    node.extend_from_slice(&1i32.to_le_bytes());
    trait_name(&mut node, "Race_AuthorRaceWaypointTimes");
    node.push(2);
    varuint(&mut node, waypoint_ms.len() as u32);
    for t in waypoint_ms {
        node.extend_from_slice(&t.to_le_bytes());
    }
    trait_name(&mut node, "Race_AuthorClones");
    node.push(3);
    varuint(&mut node, clones.len() as u32);
    for c in clones {
        node.extend_from_slice(&c.offset_ms.to_le_bytes());
        node.extend_from_slice(&c.slow_e6.to_le_bytes());
    }
    node.extend_from_slice(&0xFACA_DE01u32.to_le_bytes());
    let mut out = Vec::with_capacity(node.len() + 8);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(node.len() as u32).to_le_bytes());
    out.extend_from_slice(&node);
    out
}

/// A `.Ghost.Gbx` body as the validation-ghost blob: `u32 0`, `u32 size`,
/// the CGameCtnGhost class id, the body with its record-data node renumbered
/// from 1 to 2 (the nested archive counts its root as node 1).
pub fn ghost_blob(ghost_body: &[u8]) -> Result<Vec<u8>, String> {
    let mut body = ghost_body.to_vec();
    // the record-data node reference: index, class 0x0911F000, chunk 0x0911F000
    let pat: [u8; 8] = [0x00, 0xf0, 0x11, 0x09, 0x00, 0xf0, 0x11, 0x09];
    let at = body.windows(8).position(|w| w == pat).ok_or("ghost body: no CPlugEntRecordData node")?;
    if at < 4 {
        return Err("ghost body: record node at the very start".into());
    }
    let idx = u32::from_le_bytes(body[at - 4..at].try_into().unwrap());
    if idx != 1 && idx != 2 {
        return Err(format!("ghost body: record node index {idx}, expected 1"));
    }
    body[at - 4..at].copy_from_slice(&2u32.to_le_bytes());
    if !body.ends_with(&0xFACA_DE01u32.to_le_bytes()) {
        return Err("ghost body does not end with FACADE01".into());
    }
    let mut node = Vec::with_capacity(body.len() + 4);
    node.extend_from_slice(&0x0309_2000u32.to_le_bytes());
    node.extend_from_slice(&body);
    let mut out = Vec::with_capacity(node.len() + 8);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(node.len() as u32).to_le_bytes());
    out.extend_from_slice(&node);
    Ok(out)
}

/// Replace (or insert after `0x0305B00E`) the skippable body chunk `id`.
pub fn set_skip_chunk(m: &mut MapFile, id: u32, payload: &[u8]) -> Result<(), String> {
    let mut chunk = Vec::with_capacity(12 + payload.len());
    chunk.extend_from_slice(&id.to_le_bytes());
    chunk.extend_from_slice(b"PIKS");
    chunk.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    chunk.extend_from_slice(payload);
    let chunks = tmmaps::gbx::all_skip_chunks(&m.gbx.body);
    if let Some(&(_, off, p, s)) = chunks.iter().find(|c| c.0 == id) {
        m.raw_splices.push(((off, p + s), chunk));
        return Ok(());
    }
    // insert: after 0x0305B00E for the ghost, else after 0x03043043 for the metadata
    let anchor = if id == 0x0305_B00F { 0x0305_B00E } else { 0x0304_3043 };
    let (_, _, p, s) = chunks.iter().find(|c| c.0 == anchor).copied().ok_or_else(|| format!("map has neither chunk {id:#010x} nor its anchor {anchor:#010x}"))?;
    m.raw_splices.push(((p + s, p + s), chunk));
    Ok(())
}

/// The header description chunk `0x03043002` (v13) with the medal times,
/// lap race flag, lap count and checkpoint count set.
pub fn set_header_times(m: &mut MapFile, times_ms: [u32; 4], laps: u32, n_checkpoints: u32) -> bool {
    m.edit_header_chunk(0x0304_3002, &|c: &[u8]| {
        if c.len() < 57 || c[0] < 13 {
            return None;
        }
        let mut b = c.to_vec();
        // version u8, U01 u32, bronze, silver, gold, author, cost, isLapRace,
        // editorMode, U02, authorScore, editor, U03, nbCheckpoints, nbLaps
        let put = |b: &mut Vec<u8>, i: usize, v: u32| b[i..i + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut b, 5, times_ms[3]);
        put(&mut b, 9, times_ms[2]);
        put(&mut b, 13, times_ms[1]);
        put(&mut b, 17, times_ms[0]);
        put(&mut b, 25, (laps > 1) as u32);
        put(&mut b, 49, n_checkpoints);
        put(&mut b, 53, laps);
        Some(b)
    })
}
