//! `tinyctl mtrender` — a render copy of a map whose in-game MediaTracker group has a
//! clip that TRIGGERS where the ghost drives.
//!
//! ```text
//! tinyctl mtrender --map MAP --ghost G.Ghost.Gbx --out F [--pad 3]
//! ```
//!
//! WHY (2026-09-09, video session 2). On a map whose file has NO in-game clip group
//! (Summer 01, 02, 03, 05, 12 …) the map editor's in-game MediaTracker creates one
//! with an empty "Trigger 1" clip, `/ourclip` adds "GhostShooter", the ghost imports,
//! the camera block targets it — live playback follows the car — and the SHOOT
//! renders the whole clip from the map's thumbnail camera (chunk 0x03043036) with no
//! car: a static frame. Same map file rendered fine yesterday; the ghost, the uid,
//! the clip flags, the game session are all ruled out. The one thing the shoot does
//! that live playback does not is play the group AS A RACE WOULD: a clip runs when the
//! player enters its trigger zone, and a clip made by `CreateClip` has no zone the car
//! ever enters (where the editor's default zone lands depends on the editor camera).
//! Maps whose file carries an in-game group (04, 11, 15, 16) or whose default zone
//! happened to sit on the start (14, 17, 24) render.
//!
//! So the render copy gets a trigger the car cannot miss: the end-race group's node
//! (present in every Summer 2026 map, never used by a ghost render) is MOVED into the
//! in-game slot — no node is duplicated, so every node index stays unique — its first
//! clip's zone becomes every trigger cell the ghost's samples pass through (plus `--pad`
//! cells around the start), and the other clips lose their zones. The render then uses
//! clip 0 of the group (`shootctl setup --clip 0`: select it, empty it, import there).
//! The installed maps are untouched: this is the `_stage/vidNN.Map.Gbx` copy only.

use std::path::PathBuf;

use tmmaps::mediatracker::{Slot, Trigger};

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let map = PathBuf::from(f("--map").ok_or("mtrender needs --map MAP")?);
    let ghost = PathBuf::from(f("--ghost").ok_or("mtrender needs --ghost G.Ghost.Gbx")?);
    let out = PathBuf::from(f("--out").ok_or("mtrender needs --out F")?);
    let pad: i32 = f("--pad").map(|s| s.parse().map_err(|_| "--pad wants a cell count")).transpose()?.unwrap_or(3);

    let mut m = tmmaps::map::MapFile::load(&map);
    let mut mt = match m.mediatracker() {
        None => return Err(format!("{}: no MediaTracker chunk (0x03043049)", map.display())),
        Some(Err(e)) => return Err(format!("{}: MediaTracker: {e}", map.display())),
        Some(Ok(mt)) => mt,
    };
    let ts = mt.trigger_size.ok_or("the MediaTracker chunk has no trigger size")?;
    let collection = m.items.first().map(|it| it.collection_raw).unwrap_or(26);
    let ground = tmmaps::map::ground_y(collection);
    let unit = [32.0 / ts[0].max(1) as f32, 8.0 / ts[1].max(1) as f32, 32.0 / ts[2].max(1) as f32];

    // the cells the car passes through, from the ghost's samples
    let g = gbx::record::decode_ghost(ghost.to_str().ok_or("ghost path is not utf-8")?)?;
    if g.samples.is_empty() {
        return Err("the ghost has no samples".into());
    }
    let cell = |x: f32, y: f32, z: f32| -> [i32; 3] { [(x / unit[0]).floor() as i32, ((y - ground) / unit[1]).floor() as i32, (z / unit[2]).floor() as i32] };
    let mut cells: std::collections::BTreeSet<[i32; 3]> = Default::default();
    for s in &g.samples {
        let c = cell(s.x, s.y, s.z);
        // the car is ~2 m tall and the trigger row 4 m: the row above too
        cells.insert(c);
        cells.insert([c[0], c[1] + 1, c[2]]);
        cells.insert([c[0], c[1] - 1, c[2]]);
    }
    let s0 = &g.samples[0];
    let c0 = cell(s0.x, s0.y, s0.z);
    for dx in -pad..=pad {
        for dz in -pad..=pad {
            for dy in -1..=2 {
                cells.insert([c0[0] + dx, c0[1] + dy, c0[2] + dz]);
            }
        }
    }
    let coords: Vec<[i32; 3]> = cells.into_iter().filter(|c| c.iter().all(|v| *v >= 0)).collect();

    // the group: the in-game one when the file has it, else the end-race one moved over
    let (group, moved) = match (&mt.in_game, &mt.end_race) {
        (Slot::Group(g), _) => (g.clone(), false),
        (_, Slot::Group(g)) => (g.clone(), true),
        _ => return Err("the map has neither an in-game nor an end-race clip group to carry the render clip".into()),
    };
    let mut group = group;
    if group.clips.is_empty() {
        return Err("the clip group has no clip".into());
    }
    let template = group.triggers.first().cloned().unwrap_or(Trigger { u01: -1, u02: -1, u03: -1, u04: 0, condition: 0, condition_value: 0.0, coords: Vec::new() });
    let mut triggers = Vec::with_capacity(group.clips.len());
    for i in 0..group.clips.len() {
        let mut t = group.triggers.get(i).cloned().unwrap_or_else(|| template.clone());
        t.condition = 0;
        t.condition_value = 0.0;
        t.coords = if i == 0 { coords.clone() } else { Vec::new() };
        triggers.push(t);
    }
    group.triggers = triggers;
    let clip0 = group.clips[0].name.clone();
    mt.in_game = Slot::Group(group);
    if moved {
        mt.end_race = Slot::Null;
    }
    m.set_mediatracker(&mt);
    m.write_to(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    // read back
    let check = tmmaps::map::MapFile::load(&out);
    let mt2 = check.mediatracker().ok_or("written map: no MediaTracker chunk")?.map_err(|e| format!("written map: MediaTracker: {e}"))?;
    let (n_clips, n_cells) = match &mt2.in_game {
        Slot::Group(g) => (g.clips.len(), g.triggers.first().map(|t| t.coords.len()).unwrap_or(0)),
        _ => (0, 0),
    };
    println!(
        "{} -> {}: in-game group {} ({} clips, clip 0 {:?} triggers on {} cells: the ghost's {} samples + {}-cell pad around the start cell {:?}); end-race {}",
        map.display(),
        out.display(),
        if moved { "= the end-race group moved over" } else { "kept" },
        n_clips,
        clip0,
        n_cells,
        g.samples.len(),
        pad,
        c0,
        if moved { "now null" } else { "kept" }
    );
    if n_cells != coords.len() {
        return Err(format!("read-back: clip 0 has {n_cells} trigger cells, wrote {}", coords.len()));
    }
    Ok(())
}
